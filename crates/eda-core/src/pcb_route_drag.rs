//! Connected track edits use the routing solver, retaining fixed copper anchors.
use super::*;
use std::collections::BTreeSet;
#[cfg(test)]
#[path = "pcb_route_drag_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug)]
pub struct DragOptions {
    pub conflict: Conflict,
    pub clearance: i64,
    pub diagonal_first: bool,
}
impl Default for DragOptions {
    fn default() -> Self {
        Self {
            conflict: Conflict::Walkaround,
            clearance: 200_000,
            diagonal_first: true,
        }
    }
}

pub fn track_selection(board: &Board, selection: &[Uuid]) -> bool {
    !selection.is_empty()
        && selection.iter().all(|id| {
            board.item(*id).is_some_and(|i| {
                copper_layer(i.layer)
                    && i.component.is_none()
                    && i.polygon.is_none()
                    && !i.keepout
                    && matches!(i.shape, Shape::Track { .. })
            })
        })
}

/// Move tracks and reroute their neighbours in one validated transaction.
/// A blocked route fails as a whole rather than committing a disconnected leg.
pub fn drag_tracks(
    board: &Board,
    selection: &[Uuid],
    pivot: Point,
    translation: Point,
    angle: f64,
    options: DragOptions,
) -> Result<Board, String> {
    if !track_selection(board, selection) {
        return Err("Select free copper tracks for interactive dragging".into());
    }
    let next = board.edited(Edit::Transform {
        selection: selection.to_vec(),
        pivot,
        translation,
        angle,
    })?;
    reroute(board, next, selection, options)
}

pub fn drag_track_endpoint(
    board: &Board,
    id: Uuid,
    at_start: bool,
    at: Point,
    options: DragOptions,
) -> Result<Board, String> {
    if !track_selection(board, &[id]) {
        return Err("Select a free copper track endpoint".into());
    }
    let handle = if at_start {
        crate::pcb_tools::Handle::TrackStart(id)
    } else {
        crate::pcb_tools::Handle::TrackEnd(id)
    };
    let next = crate::pcb_tools::move_handle(board, handle, at, false)?;
    reroute(board, next, &[id], options)
}

struct MovedTrack {
    old: Item,
    new: Item,
}
impl MovedTrack {
    fn map(&self, point: Point) -> Option<Point> {
        let Shape::Track { start, end, .. } = self.old.shape else {
            return None;
        };
        if segment_distance(point, start, end) > 2.0 {
            return None;
        }
        let Shape::Track {
            start: a, end: b, ..
        } = self.new.shape
        else {
            return None;
        };
        let length = distance_sq(start, end);
        let t = if length == 0.0 {
            0.0
        } else {
            (((point.x - start.x) as f64 * (end.x - start.x) as f64
                + (point.y - start.y) as f64 * (end.y - start.y) as f64)
                / length)
                .clamp(0.0, 1.0)
        };
        Some(Point {
            x: (a.x as f64 + t * (b.x - a.x) as f64).round() as i64,
            y: (a.y as f64 + t * (b.y - a.y) as f64).round() as i64,
        })
    }
}

fn contacts(item: &Item, other: &Item) -> Vec<Point> {
    let Shape::Track { start, end, .. } = item.shape else {
        return vec![];
    };
    let mut points = vec![start, end];
    match other.shape {
        Shape::Track {
            start: a, end: b, ..
        } => {
            points.extend([a, b]);
            let line = |a: Point, b: Point| {
                geo::Line::new((a.x as f64, a.y as f64), (b.x as f64, b.y as f64))
            };
            if let Some(geo::line_intersection::LineIntersection::SinglePoint {
                intersection,
                ..
            }) = geo::line_intersection::line_intersection(line(start, end), line(a, b))
            {
                points.push(Point {
                    x: intersection.x.round() as i64,
                    y: intersection.y.round() as i64,
                });
            }
        }
        Shape::Pad { position, .. } | Shape::Via { position, .. } => {
            points.push(project(position, start, end));
        }
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            let arc = arc_points(centre, radius, start_angle, end_angle, 2);
            points.extend(arc.first().copied());
            points.extend(arc.last().copied());
        }
        _ => {}
    }
    points.retain(|p| {
        segment_distance(*p, start, end) <= 2.0
            && other.hit_distance_on_layer(*p, item.layer) <= 2.0
    });
    points.sort_by(|a, b| distance_sq(start, *a).total_cmp(&distance_sq(start, *b)));
    points.dedup();
    points
}

fn reroute(
    board: &Board,
    mut next: Board,
    selection: &[Uuid],
    options: DragOptions,
) -> Result<Board, String> {
    if !(0..=100_000_000_000).contains(&options.clearance) {
        return Err("Invalid drag clearance".into());
    }
    let moved: Vec<_> = selection
        .iter()
        .filter_map(|id| {
            let old = board.item(*id)?;
            let new = next.item(*id)?;
            (old.shape != new.shape).then(|| MovedTrack {
                old: old.clone(),
                new: new.clone(),
            })
        })
        .collect();
    if moved.is_empty() {
        return Ok(next);
    }
    let mut routes: Vec<_> = moved.iter().map(|m| m.new.clone()).collect();
    let mut bridges = BTreeSet::new();
    for candidate in &board.items {
        if selection.contains(&candidate.id) || candidate.keepout || candidate.polygon.is_some() {
            continue;
        }
        let attached: Vec<_> = moved
            .iter()
            .filter(|m| candidate.net == m.old.net && on_layer(board, candidate, m.old.layer))
            .collect();
        if attached.is_empty() {
            continue;
        }
        let mapped = |point| attached.iter().find_map(|m| m.map(point)).unwrap_or(point);
        if let Shape::Track { start, end, width } = candidate.shape
            && candidate.component.is_none()
        {
            let a = mapped(start);
            let b = mapped(end);
            if a != start || b != end {
                let mut item = candidate.clone();
                item.shape = Shape::Track {
                    start: a,
                    end: b,
                    width,
                };
                next.replace_item_unvalidated(item.clone())?;
                routes.push(item);
            }
        }
        // Pads, vias, arc ends and track interiors stay fixed. Bridge each
        // original contact to the moved track, including a T junction.
        for m in attached {
            let Shape::Track { width, .. } = m.old.shape else {
                continue;
            };
            for point in contacts(&m.old, candidate) {
                // Free track endpoints were already rerouted above.
                if candidate.component.is_none()
                    && let Shape::Track { start, end, .. } = candidate.shape
                    && (distance_sq(point, start) <= 4.0 || distance_sq(point, end) <= 4.0)
                {
                    continue;
                }
                let Some(to) = m.map(point) else { continue };
                if point == to || candidate.hit_distance_on_layer(point, m.old.layer) > 2.0 {
                    continue;
                }
                if bridges.insert((m.old.layer, m.old.net, point.x, point.y, to.x, to.y)) {
                    let mut bridge = m.old.clone();
                    bridge.id = Uuid::new_v4();
                    bridge.source = None;
                    bridge.shape = Shape::Track {
                        start: point,
                        end: to,
                        width,
                    };
                    routes.push(bridge);
                }
            }
        }
    }
    let replaced: BTreeSet<_> = routes.iter().map(|i| i.id).collect();
    // Only pre-existing no-net contacts may merge. A new endpoint landing on
    // unrelated no-net copper must still be rejected by clearance checking.
    let original_no_net_routes: Vec<_> = board
        .items
        .iter()
        .filter(|old| replaced.contains(&old.id) && old.net.is_none())
        .collect();
    let no_net_contacts: BTreeSet<_> = board
        .items
        .iter()
        .filter(|i| {
            i.net.is_none()
                && !i.keepout
                && !i.is_drill_only()
                && matches!(
                    i.shape,
                    Shape::Track { .. } | Shape::Pad { .. } | Shape::Via { .. } | Shape::Arc { .. }
                )
        })
        .filter(|i| {
            original_no_net_routes
                .iter()
                .any(|old| on_layer(board, i, old.layer) && !contacts(old, i).is_empty())
        })
        .map(|i| i.id)
        .collect();
    // Keep unaffected primitives in their original order. Reuse each edited
    // primitive's UUID and native metadata on its first resulting leg.
    let mut additions = Vec::new();
    let mut edited_ids = replaced.clone();
    for item in routes {
        let Shape::Track { start, end, width } = item.shape else {
            unreachable!()
        };
        let mut waypoints = vec![start];
        if let Some(old) = board.item(item.id) {
            let Shape::Track {
                start: old_start,
                end: old_end,
                ..
            } = old.shape
            else {
                unreachable!()
            };
            let mapping = MovedTrack {
                old: old.clone(),
                new: item.clone(),
            };
            let mut junctions: Vec<_> = board
                .items
                .iter()
                .filter(|other| {
                    other.id != old.id
                        && other.net == old.net
                        && !other.keepout
                        && other.polygon.is_none()
                        && on_layer(board, other, old.layer)
                })
                .flat_map(|other| contacts(old, other))
                .filter(|p| distance_sq(*p, old_start) > 4.0 && distance_sq(*p, old_end) > 4.0)
                .collect();
            junctions
                .sort_by(|a, b| distance_sq(old_start, *a).total_cmp(&distance_sq(old_start, *b)));
            junctions.dedup();
            waypoints.extend(junctions.into_iter().map(|p| {
                if selection.contains(&item.id) {
                    mapping.map(p).unwrap_or(p)
                } else {
                    p
                }
            }));
        }
        waypoints.push(end);
        waypoints.dedup();
        let mut environment = next.clone();
        environment.items.retain(|i| {
            (!replaced.contains(&i.id) || i.net != item.net)
                && !(item.net.is_none()
                    && i.net.is_none()
                    && !i.keepout
                    && no_net_contacts.contains(&i.id))
        });
        crate::pcb_rules::route_check(&next, &item)?;
        let obstacles = obstacles::Obstacles::for_item(&environment, item.layer, &item);
        obstacles.error()?;
        let mut route = RouteSession {
            revision: 0,
            draft: environment,
            anchor: start,
            layer: item.layer,
            net: item.net,
            width,
            corner: Corner::FortyFive,
            diagonal_first: options.diagonal_first,
            conflict: options.conflict,
            clearance: options.clearance,
            source_len: usize::MAX,
            contacts: BTreeSet::new(),
            obstacles,
            checkpoints: vec![],
            preview: Default::default(),
        };
        let mut points = vec![start];
        for pair in waypoints.windows(2) {
            route.anchor = pair[0];
            let leg = route.points(pair[1]);
            if leg.last() != Some(&pair[1]) {
                return Err(
                    "Cannot drag track without breaking a connection or violating clearance".into(),
                );
            }
            points.extend_from_slice(&leg[1..]);
        }
        // Keep an ID even when a neighbour collapses to a point. This also
        // retains the shared junction for subsequent edits and Undo.
        let edges: Vec<_> = if start == end {
            vec![[start, end]]
        } else {
            points.windows(2).map(|e| [e[0], e[1]]).collect()
        };
        for (n, [start, end]) in edges.into_iter().enumerate() {
            let mut leg = item.clone();
            if n > 0 {
                leg.id = Uuid::new_v4();
                leg.source = None;
            }
            edited_ids.insert(leg.id);
            leg.shape = Shape::Track { start, end, width };
            crate::pcb_rules::route_check(&next, &leg)?;
            if n == 0 && next.item(leg.id).is_some() {
                next.replace_item_unvalidated(leg)?;
            } else {
                additions.push(leg);
            }
        }
    }
    next.items.extend(additions);
    // Later replacements can introduce new bends. Check the final geometry too,
    // so a multi-net selection cannot route through another moved net.
    if options.conflict != Conflict::Ignore {
        for item in next.items.iter().filter(|i| edited_ids.contains(&i.id)) {
            let Shape::Track { start, end, width } = item.shape else {
                unreachable!()
            };
            let mut environment = next.clone();
            environment.items.retain(|i| {
                i.id != item.id
                    && !(item.net.is_none()
                        && i.net.is_none()
                        && !i.keepout
                        && (edited_ids.contains(&i.id) || no_net_contacts.contains(&i.id)))
            });
            let obstacles = obstacles::Obstacles::for_item(&environment, item.layer, item);
            obstacles.error()?;
            let route = RouteSession {
                revision: 0,
                draft: environment,
                anchor: start,
                layer: item.layer,
                net: item.net,
                width,
                corner: Corner::FortyFive,
                diagonal_first: options.diagonal_first,
                conflict: options.conflict,
                clearance: options.clearance,
                source_len: usize::MAX,
                contacts: BTreeSet::new(),
                obstacles,
                checkpoints: vec![],
                preview: Default::default(),
            };
            if !route.obstacles.clear_for_route(&route, start, end) {
                return Err("Dragged tracks violate copper clearance".into());
            }
        }
    }
    next.pours_stale |= !next.polygon_pours.is_empty();
    next.validate()?;
    Ok(next)
}
