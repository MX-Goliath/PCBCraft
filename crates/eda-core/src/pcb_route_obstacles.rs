//! Model-space clearance checks and bounded walkaround search. Copper geometry
//! follows the same polygon approximation as pours; this is not a rule compiler.
use super::*;
use crate::pcb_polygon::copper;
use geo::{Distance, Euclidean, Line, MultiPolygon};
use rstar::{AABB, RTree, RTreeObject};
#[cfg(test)]
#[path = "pcb_route_obstacles_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Conflict {
    Ignore,
    #[default]
    Walkaround,
    Stop,
}
impl Conflict {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ignore => "Ignore Obstacles",
            Self::Walkaround => "Walkaround Obstacles",
            Self::Stop => "Stop At First Obstacle",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Ignore => Self::Walkaround,
            Self::Walkaround => Self::Stop,
            Self::Stop => Self::Ignore,
        }
    }
}
#[derive(Clone)]
struct Obstacle {
    bounds: Bounds,
    gap: Result<Option<i64>, String>,
    gap_bound: i64,
    item: Option<usize>,
    copper: Geometry,
}
#[derive(Clone)]
enum Geometry {
    Track(Line<f64>, f64),
    Arc(Vec<Line<f64>>, f64),
    Polygon(MultiPolygon<f64>),
}
impl Geometry {
    fn distance(&self, line: &Line<f64>) -> f64 {
        match self {
            Self::Track(other, radius) => (Euclidean.distance(line, other) - radius).max(0.0),
            Self::Arc(segments, radius) => segments
                .iter()
                .map(|other| (Euclidean.distance(line, other) - radius).max(0.0))
                .fold(f64::INFINITY, f64::min),
            Self::Polygon(polygon) => Euclidean.distance(line, polygon),
        }
    }
}
#[derive(Clone)]
struct IndexedBounds {
    bounds: AABB<[f64; 2]>,
    index: usize,
}
impl RTreeObject for IndexedBounds {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}
#[derive(Clone)]
pub(super) struct Obstacles {
    objects: Vec<Obstacle>,
    spatial: bool,
    index: RTree<IndexedBounds>,
    maximum_gap: i64,
    first_error: Option<usize>,
}
impl Obstacles {
    fn indexed(objects: Vec<Obstacle>, spatial: bool) -> Self {
        let index = RTree::bulk_load(
            objects
                .iter()
                .enumerate()
                .map(|(index, o)| IndexedBounds {
                    bounds: envelope(o.bounds),
                    index,
                })
                .collect(),
        );
        let maximum_gap = objects
            .iter()
            .map(|o| {
                o.gap
                    .as_ref()
                    .ok()
                    .copied()
                    .flatten()
                    .unwrap_or(0)
                    .max(o.gap_bound)
            })
            .max()
            .unwrap_or(0);
        let first_error = objects.iter().position(|o| o.gap.is_err());
        Self {
            objects,
            spatial,
            index,
            maximum_gap,
            first_error,
        }
    }
    pub fn new(board: &Board, layer: u16, net: Option<u32>) -> Self {
        let probe = crate::pcb_rules::probe(layer, net, Point::default(), 1);
        Self::for_item(board, layer, &probe)
    }
    pub fn error(&self) -> Result<(), String> {
        if let Some(index) = self.first_error
            && let Err(e) = &self.objects[index].gap
        {
            return Err(e.clone());
        }
        Ok(())
    }
    pub fn for_item(board: &Board, layer: u16, probe: &Item) -> Self {
        let net = probe.net;
        // Room membership changes as the route moves. Cache the geometry, but
        // resolve these pair clearances against each proposed segment.
        let spatial = matches!(probe.shape, Shape::Track { .. })
            && board.rules.iter().any(|r| {
                crate::pcb_rules::enabled(r)
                    && matches!(
                        crate::pcb_rules::text(r, "RULEKIND"),
                        "Clearance" | "BoardOutlineClearance"
                    )
                    && ["SCOPE1EXPRESSION", "SCOPE2EXPRESSION"].iter().any(|key| {
                        crate::pcb_rules::text(r, key)
                            .to_ascii_uppercase()
                            .contains("WITHINROOM(")
                    })
            });
        let mut obstacles: Vec<Obstacle> = board
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                let board_cutout = i
                    .properties
                    .get("ISBOARDCUTOUT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"));
                if matches!(i.shape, Shape::Region { cutout: true, .. })
                    && !board_cutout
                    && !i.keepout
                {
                    return false;
                }
                // No-net objects are obstacles too; only an assigned common net
                // permits merging. Keepouts apply irrespective of their net.
                (i.keepout
                    || board_cutout
                    || i.layer == KEEP_OUT
                    || net.is_none()
                    || i.net != net
                    || i.is_drill_only())
                    && (board_cutout
                        || on_layer(board, i, layer)
                        || i.layer == KEEP_OUT
                        || i.is_drill_only())
            })
            .map(|(index, i)| Obstacle {
                bounds: i.bounds(),
                item: Some(index),
                gap_bound: if spatial {
                    maximum_gap(
                        board,
                        probe,
                        crate::pcb_rules::object_kind(i),
                        text_is_board_cutout(i),
                    )
                } else {
                    0
                },
                gap: if text_is_board_cutout(i) {
                    crate::pcb_rules::cutout_clearance(board, probe, i)
                } else {
                    crate::pcb_rules::clearance(board, probe, i)
                },
                copper: match i.shape {
                    Shape::Track { start, end, width } => Geometry::Track(
                        Line::new(
                            (start.x as f64 / 1e6, start.y as f64 / 1e6),
                            (end.x as f64 / 1e6, end.y as f64 / 1e6),
                        ),
                        width as f64 / 2e6,
                    ),
                    Shape::Arc {
                        centre,
                        radius,
                        start_angle,
                        end_angle,
                        width,
                    } => {
                        let points = arc_points(centre, radius, start_angle, end_angle, 256);
                        // Inflate by chord sagitta, keeping the polygonal arc
                        // approximation on the conservative side of clearance.
                        let step = arc_sweep(start_angle, end_angle).to_radians()
                            / (points.len() - 1) as f64;
                        let sagitta = radius as f64 * (1.0 - (step / 2.0).cos());
                        Geometry::Arc(
                            points
                                .windows(2)
                                .map(|e| {
                                    Line::new(
                                        (e[0].x as f64 / 1e6, e[0].y as f64 / 1e6),
                                        (e[1].x as f64 / 1e6, e[1].y as f64 / 1e6),
                                    )
                                })
                                .collect(),
                            (width as f64 / 2.0 + sagitta) / 1e6,
                        )
                    }
                    _ => Geometry::Polygon(copper(i, layer)),
                },
            })
            .collect();
        let outline = board.outline_points(256);
        if outline.len() >= 3 {
            let rule = crate::pcb_rules::resolve(board, "BoardOutlineClearance", probe, None);
            if !matches!(rule, Ok(None))
                || spatial
                    && board.rules.iter().any(|r| {
                        crate::pcb_rules::enabled(r)
                            && crate::pcb_rules::text(r, "RULEKIND") == "BoardOutlineClearance"
                    })
            {
                let gap = rule.and_then(|r| {
                    r.map(|(_, r)| {
                        crate::pcb_rules::gap_objects(
                            r,
                            crate::pcb_rules::object_kind(probe),
                            "OutlineEdge",
                        )
                    })
                    .transpose()
                });
                // An explicit zero edge clearance permits objects to cross that edge.
                if matches!(gap, Ok(Some(0))) && !spatial {
                    return Self::indexed(obstacles, spatial);
                }
                let exterior = vec![
                    (-100_001.0, -100_001.0),
                    (100_001.0, -100_001.0),
                    (100_001.0, 100_001.0),
                    (-100_001.0, 100_001.0),
                    (-100_001.0, -100_001.0),
                ];
                let interior = outline
                    .iter()
                    .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
                    .collect::<Vec<_>>();
                obstacles.push(Obstacle {
                    bounds: Bounds::points(&[
                        Point::mm(-100_001, -100_001),
                        Point::mm(100_001, 100_001),
                    ]),
                    gap,
                    item: None,
                    gap_bound: if spatial {
                        maximum_gap(board, probe, "OutlineEdge", true)
                    } else {
                        0
                    },
                    copper: Geometry::Polygon(MultiPolygon(vec![geo::Polygon::new(
                        exterior.into(),
                        vec![interior.into()],
                    )])),
                });
            }
        }
        Self::indexed(obstacles, spatial)
    }
    fn blocker(
        &self,
        a: Point,
        b: Point,
        radius: i64,
        clearance: i64,
        route: Option<&RouteSession>,
    ) -> Option<usize> {
        // Clearance may change during a session, and room/matrix rules can
        // enlarge it away from the initial probe. Query with a conservative
        // maximum, then resolve the exact pair rule in the narrow phase.
        // Include the 1 nm tolerance used by the geometry distance check.
        let search = envelope(
            Bounds::points(&[a, b]).expanded(radius + self.maximum_gap.max(clearance) + 1),
        );
        let mut candidates: Vec<_> = self
            .index
            .locate_in_envelope_intersecting(&search)
            .map(|o| o.index)
            .collect();
        // A rule error must block even if its geometry is outside the query.
        candidates.extend(self.first_error);
        candidates.sort_unstable();
        candidates.dedup();
        self.blocker_among(a, b, radius, clearance, route, candidates)
    }
    fn blocker_among(
        &self,
        a: Point,
        b: Point,
        radius: i64,
        clearance: i64,
        route: Option<&RouteSession>,
        candidates: impl IntoIterator<Item = usize>,
    ) -> Option<usize> {
        let bounds = Bounds::points(&[a, b]);
        let line = Line::new(
            (a.x as f64 / 1e6, a.y as f64 / 1e6),
            (b.x as f64 / 1e6, b.y as f64 / 1e6),
        );
        let probe = route.filter(|_| self.spatial).map(|r| {
            Item::new(
                r.layer,
                r.net,
                Shape::Track {
                    start: a,
                    end: b,
                    width: r.width,
                },
            )
        });
        // Keep original board order: choosing a different first obstruction
        // changes walkaround posture and the result of the bounded search.
        candidates.into_iter().find(|&index| {
            let o = &self.objects[index];
            let Ok(gap) = o.gap else {
                return true;
            };
            let broad_radius = radius
                + if probe.is_some() {
                    o.gap_bound.max(clearance)
                } else {
                    gap.unwrap_or(clearance)
                }
                + 1;
            let bounds = bounds.expanded(broad_radius);
            if !(bounds.min.x <= o.bounds.max.x
                && bounds.max.x >= o.bounds.min.x
                && bounds.min.y <= o.bounds.max.y
                && bounds.max.y >= o.bounds.min.y)
            {
                return false;
            }
            let gap = if let (Some(route), Some(probe)) = (route, &probe) {
                let board = &route.draft;
                let result = if let Some(index) = o.item {
                    let item = &board.items[index];
                    if text_is_board_cutout(item) {
                        crate::pcb_rules::cutout_clearance(board, probe, item)
                    } else {
                        crate::pcb_rules::clearance(board, probe, item)
                    }
                } else {
                    crate::pcb_rules::resolve(board, "BoardOutlineClearance", probe, None).and_then(
                        |r| {
                            r.map(|(_, r)| crate::pcb_rules::gap_objects(r, "Track", "OutlineEdge"))
                                .transpose()
                        },
                    )
                };
                match result {
                    Ok(gap) => gap,
                    Err(_) => return true,
                }
            } else {
                gap
            };
            if o.item.is_none() && gap.is_none_or(|gap| gap == 0) {
                return false; // No applicable edge constraint, or explicit crossing permission.
            }
            o.copper.distance(&line) < (radius + gap.unwrap_or(clearance)) as f64 / 1e6 + 0.000001
        })
    }
    pub fn clear(&self, a: Point, b: Point, radius: i64, clearance: i64) -> bool {
        self.blocker(a, b, radius, clearance, None).is_none()
    }
    pub(super) fn clear_for_route(&self, route: &RouteSession, a: Point, b: Point) -> bool {
        self.blocker(a, b, route.width / 2, route.clearance, Some(route))
            .is_none()
    }
    fn stop(&self, points: &[Point], route: &RouteSession) -> Vec<Point> {
        let clear = |a, b| {
            self.blocker(
                a,
                b,
                route.width.max(1) / 2,
                route.clearance.max(0),
                Some(route),
            )
            .is_none()
        };
        let mut result = vec![points[0]];
        for edge in points.windows(2) {
            let (a, b) = (edge[0], edge[1]);
            if clear(a, b) {
                result.push(b);
                continue;
            }
            if !clear(a, a) {
                break;
            }
            let interpolate = |t: f64| Point {
                x: a.x + ((b.x - a.x) as f64 * t).round() as i64,
                y: a.y + ((b.y - a.y) as f64 * t).round() as i64,
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..40 {
                let mid = (lo + hi) / 2.0;
                if clear(a, interpolate(mid)) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let p = interpolate(lo);
            if p != a && clear(a, p) {
                result.push(p);
            }
            break;
        }
        result
    }
    pub fn resolve(&self, route: &RouteSession, end: Point) -> Vec<Point> {
        let direct = route.free_points(route.anchor, end);
        let radius = route.width.max(1) / 2;
        let clearance = route.clearance.max(0);
        let clear = |a, b| self.blocker(a, b, radius, clearance, Some(route)).is_none();
        if route.conflict == Conflict::Ignore || direct.windows(2).all(|e| clear(e[0], e[1])) {
            return direct;
        }
        if route.conflict == Conflict::Stop || !clear(route.anchor, route.anchor) {
            return self.stop(&direct, route);
        }
        // Best-first detours around the first obstruction. Explicit budgets keep
        // mouse interaction bounded; on exhaustion retain the valid prefix.
        let mut queue = vec![vec![route.anchor]];
        let mut visited = std::collections::HashSet::new();
        for _ in 0..96 {
            let Some((index, _)) = queue
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| score(a, end).total_cmp(&score(b, end)))
            else {
                break;
            };
            let path = queue.swap_remove(index);
            let start = *path.last().unwrap();
            if !visited.insert((start.x, start.y)) {
                continue;
            }
            let leg = route.free_points(start, end);
            let obstruction = leg
                .windows(2)
                .find_map(|e| self.blocker(e[0], e[1], radius, clearance, Some(route)));
            let Some(index) = obstruction else {
                let mut result = path;
                result.extend_from_slice(&leg[1..]);
                return result;
            };
            if path.len() > 24 {
                continue;
            }
            let bounds = self.objects[index].bounds.expanded(
                radius
                    + self.objects[index]
                        .gap
                        .as_ref()
                        .ok()
                        .copied()
                        .flatten()
                        .unwrap_or(clearance)
                        .max(if self.spatial {
                            self.objects[index].gap_bound
                        } else {
                            0
                        })
                    + 10,
            );
            for point in [
                bounds.min,
                Point {
                    x: bounds.min.x,
                    y: bounds.max.y,
                },
                bounds.max,
                Point {
                    x: bounds.max.x,
                    y: bounds.min.y,
                },
            ] {
                if visited.contains(&(point.x, point.y)) {
                    continue;
                }
                let detour = route.free_points(start, point);
                if detour.windows(2).all(|e| clear(e[0], e[1])) {
                    let mut next = path.clone();
                    next.extend_from_slice(&detour[1..]);
                    queue.push(next);
                }
            }
        }
        self.stop(&direct, route)
    }
}
fn envelope(bounds: Bounds) -> AABB<[f64; 2]> {
    // Valid model nanometres fit exactly in f64. Floating tree arithmetic also
    // avoids overflowing i64 when multiplying board-sized envelope extents.
    AABB::from_corners(
        [bounds.min.x as f64, bounds.min.y as f64],
        [bounds.max.x as f64, bounds.max.y as f64],
    )
}
fn maximum_gap(board: &Board, probe: &Item, other_kind: &str, edge: bool) -> i64 {
    board
        .rules
        .iter()
        .filter(|r| {
            crate::pcb_rules::enabled(r)
                && (crate::pcb_rules::text(r, "RULEKIND") == "Clearance"
                    || edge && crate::pcb_rules::text(r, "RULEKIND") == "BoardOutlineClearance")
        })
        .map(|r| {
            let other_kind = if crate::pcb_rules::text(r, "RULEKIND") == "BoardOutlineClearance"
                && other_kind != "OutlineEdge"
            {
                "CutoutEdge"
            } else {
                other_kind
            };
            crate::pcb_rules::gap_objects(r, crate::pcb_rules::object_kind(probe), other_kind)
                .unwrap_or(100_000_000_000)
        })
        .max()
        .unwrap_or(0)
}
fn text_is_board_cutout(i: &Item) -> bool {
    matches!(i.shape, Shape::Region { cutout: true, .. })
        && i.properties
            .get("ISBOARDCUTOUT")
            .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
}
fn score(path: &[Point], end: Point) -> f64 {
    path.windows(2)
        .map(|e| distance_sq(e[0], e[1]).sqrt())
        .sum::<f64>()
        + distance_sq(*path.last().unwrap(), end).sqrt()
}
