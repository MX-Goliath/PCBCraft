//! Model-space clearance checks and bounded walkaround search. Copper geometry
//! follows the same polygon approximation as pours; this is not a rule compiler.
use super::*;
use crate::pcb_polygon::copper;
use geo::{Distance, Euclidean, Line, MultiPolygon};

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
pub(super) struct Obstacles(Vec<Obstacle>);
impl Obstacles {
    pub fn new(board: &Board, layer: u16, net: Option<u32>) -> Self {
        Self(
            board
                .items
                .iter()
                .filter(|i| {
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
                .map(|i| Obstacle {
                    bounds: i.bounds(),
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
                .collect(),
        )
    }
    fn blocker(&self, a: Point, b: Point, radius: i64) -> Option<usize> {
        let bounds = Bounds::points(&[a, b]).expanded(radius);
        let line = Line::new(
            (a.x as f64 / 1e6, a.y as f64 / 1e6),
            (b.x as f64 / 1e6, b.y as f64 / 1e6),
        );
        self.0.iter().position(|o| {
            bounds.min.x <= o.bounds.max.x
                && bounds.max.x >= o.bounds.min.x
                && bounds.min.y <= o.bounds.max.y
                && bounds.max.y >= o.bounds.min.y
                && o.copper.distance(&line) < radius as f64 / 1e6 + 0.000001
        })
    }
    pub fn clear(&self, a: Point, b: Point, radius: i64) -> bool {
        self.blocker(a, b, radius).is_none()
    }
    fn stop(&self, points: &[Point], radius: i64) -> Vec<Point> {
        let mut result = vec![points[0]];
        for edge in points.windows(2) {
            let (a, b) = (edge[0], edge[1]);
            if self.clear(a, b, radius) {
                result.push(b);
                continue;
            }
            if !self.clear(a, a, radius) {
                break;
            }
            let interpolate = |t: f64| Point {
                x: a.x + ((b.x - a.x) as f64 * t).round() as i64,
                y: a.y + ((b.y - a.y) as f64 * t).round() as i64,
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..40 {
                let mid = (lo + hi) / 2.0;
                if self.clear(a, interpolate(mid), radius) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let p = interpolate(lo);
            if p != a && self.clear(a, p, radius) {
                result.push(p);
            }
            break;
        }
        result
    }
    pub fn resolve(&self, route: &RouteSession, end: Point) -> Vec<Point> {
        let direct = route.free_points(route.anchor, end);
        let radius = route.width.max(1) / 2 + route.clearance.max(0);
        if route.conflict == Conflict::Ignore
            || direct.windows(2).all(|e| self.clear(e[0], e[1], radius))
        {
            return direct;
        }
        if route.conflict == Conflict::Stop || !self.clear(route.anchor, route.anchor, radius) {
            return self.stop(&direct, radius);
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
                .find_map(|e| self.blocker(e[0], e[1], radius));
            let Some(index) = obstruction else {
                let mut result = path;
                result.extend_from_slice(&leg[1..]);
                return result;
            };
            if path.len() > 24 {
                continue;
            }
            let bounds = self.0[index].bounds.expanded(radius + 10);
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
                if detour.windows(2).all(|e| self.clear(e[0], e[1], radius)) {
                    let mut next = path.clone();
                    next.extend_from_slice(&detour[1..]);
                    queue.push(next);
                }
            }
        }
        self.stop(&direct, radius)
    }
}
fn score(path: &[Point], end: Point) -> f64 {
    path.windows(2)
        .map(|e| distance_sq(e[0], e[1]).sqrt())
        .sum::<f64>()
        + distance_sq(*path.last().unwrap(), end).sqrt()
}
