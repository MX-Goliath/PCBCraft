//! Conservative replacement of an unbranched existing, same-layer route.
//! Only the newly closed loop is considered; pre-existing loops are retained.
use super::*;
use std::collections::{HashMap, HashSet};

pub(super) fn remove(route: &RouteSession, board: &mut Board) {
    // Consider all placed contacts, so rejoining copper and then continuing
    // (automatic termination disabled) still replaces the closed old section.
    let mut anchors: Vec<_> = route
        .checkpoints
        .iter()
        .map(|c| (c.anchor, c.layer))
        .collect();
    anchors.push((route.anchor, route.layer));
    // Altium stores the per-net exception in the native Net record. Retain
    // intentional power/ground loops when that imported preference is disabled.
    if board
        .nets
        .iter()
        .find(|n| Some(n.index) == route.net)
        .and_then(|n| n.properties.get("LOOPREMOVAL"))
        .is_some_and(|v| v.eq_ignore_ascii_case("FALSE") || v == "0")
    {
        return;
    }
    if route.net.is_none() || route.origin.is_none() {
        return;
    }
    // Layer-changing loops are deliberately retained until a multilayer graph
    // can prove that removing their vias will preserve all other connections.
    if route.route_items.iter().any(|id| {
        board
            .item(*id)
            .is_some_and(|i| !matches!(i.shape, Shape::Track { .. }) || i.layer != route.layer)
    }) {
        return;
    }
    let old_endpoints: HashSet<_> = board
        .items
        .iter()
        .filter(|i| {
            i.net == route.net
                && !i.keepout
                && i.polygon.is_none()
                && !route.route_items.contains(&i.id)
        })
        .flat_map(|i| match i.shape {
            Shape::Track { start, end, .. } => {
                vec![(start.x, start.y, i.layer), (end.x, end.y, i.layer)]
            }
            _ => vec![],
        })
        .collect();
    anchors.retain(|(p, layer)| old_endpoints.contains(&(p.x, p.y, *layer)));
    for end_index in 1..anchors.len() {
        let (end, layer) = anchors[end_index];
        for &(origin, from_layer) in anchors[..end_index].iter().rev() {
            if from_layer == layer && origin != end {
                remove_between(route, board, origin, end, layer);
            }
        }
    }
}

fn remove_between(
    route: &RouteSession,
    board: &mut Board,
    origin: Point,
    finish: Point,
    layer: u16,
) {
    let tracks: Vec<_> = board
        .items
        .iter()
        .filter(|i| {
            i.layer == layer
                && i.net == route.net
                && !i.keepout
                && i.polygon.is_none()
                && !route.route_items.contains(&i.id)
                && matches!(i.shape, Shape::Track { .. })
        })
        .collect();
    let mut graph: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (n, i) in tracks.iter().enumerate() {
        if let Shape::Track { start, end, .. } = i.shape {
            graph.entry((start.x, start.y)).or_default().push(n);
            graph.entry((end.x, end.y)).or_default().push(n);
        }
    }
    let mut paths = Vec::new();
    for first in graph.get(&(origin.x, origin.y)).into_iter().flatten() {
        let mut at = origin;
        let mut edge = *first;
        let mut path = Vec::new();
        let mut seen = HashSet::new();
        loop {
            if !seen.insert(edge) {
                break;
            }
            let item = tracks[edge];
            let Shape::Track { start, end, .. } = item.shape else {
                unreachable!()
            };
            let next = if at == start { end } else { start };
            path.push(item.id);
            if next == finish {
                paths.push(path);
                break;
            }
            let Some(edges) = graph.get(&(next.x, next.y)).filter(|v| v.len() == 2) else {
                break;
            };
            at = next;
            edge = if edges[0] == edge { edges[1] } else { edges[0] };
        }
    }
    // Two old paths already form a loop: choosing either would destroy an
    // intentional topology. A free-space route has no old path at all.
    if paths.len() == 1 {
        let removed: HashSet<_> = paths.pop().unwrap().into_iter().collect();
        // Validate every old primitive only after topology has established a
        // unique path. A locked alternative must not make an existing loop look
        // like a unique path that can safely be deleted.
        for item in tracks.iter().filter(|i| removed.contains(&i.id)) {
            if item.locked || item.component.is_some() {
                return;
            }
            // Do not orphan a branch, pad, via, arc or free copper touching the
            // interior of the replaced section. Check geometric contacts too,
            // because imported T-junctions need not have split track records.
            let unsafe_contact = board.items.iter().any(|other| {
                if other.id == item.id
                    || route.route_items.contains(&other.id)
                    || other.keepout
                    || other.polygon.is_some()
                    || other.net != route.net
                    || !on_layer(board, other, layer)
                {
                    return false;
                }
                // Arbitrary arc/region/fill copper can contact the middle of a
                // track without sharing an endpoint. Keep the path whenever
                // conservative bounds overlap; this graph cannot prove it safe.
                if !matches!(other.shape, Shape::Track { .. } | Shape::Pad { .. } | Shape::Via { .. })
                    && item.bounds().intersects(other.bounds()) {
                    return true;
                }
                drag::contacts(item, other).into_iter().any(|p| {
                    p != origin
                        && p != finish
                        && (!matches!(item.shape, Shape::Track { start, end, .. } if p == start || p == end) || !matches!(other.shape, Shape::Track { .. }))
                })
            });
            if unsafe_contact {
                return;
            }
        }
        board.items.retain(|i| !removed.contains(&i.id));
        board.pours_stale |= !board.polygon_pours.is_empty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Board {
        let mut b = Board::default();
        b.nets.push(Net {
            index: 0,
            name: "Signal".into(),
            properties: Fields::new(),
        });
        b.items.push(Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(10, 0),
                width: 250_000,
            },
        ));
        b
    }
    fn reroute(b: &Board) -> RouteSession {
        let mut r = RouteSession::begin(b, 0, Point::mm(2, 0), TOP, None, 250_000, 0).unwrap();
        r.conflict = Conflict::Ignore;
        r.place(Point::mm(2, 3), None).unwrap();
        r.place(Point::mm(8, 3), None).unwrap();
        let hit = target(b, Point::mm(8, 0), TOP, 0);
        r.place(Point::mm(8, 0), hit).unwrap();
        r
    }
    #[test]
    fn replaces_only_old_section_between_interior_track_points_and_is_undoable() {
        let b = fixture();
        let mut r = reroute(&b);
        let finished = r.finish(0).unwrap();
        assert!(
            !finished
                .items
                .iter()
                .any(|i| matches!(i.shape, Shape::Track { start, end, .. }
            if start == Point::mm(2, 0) && end == Point::mm(8, 0)))
        );
        assert_eq!(finished.items.len(), 5);
        let mut history = crate::history::History::default();
        let mut doc = crate::Document::new(crate::DocumentKind::Board, "Reroute");
        doc.pcb = Some(b.clone());
        history.commit_pcb(&mut doc, finished.clone()).unwrap();
        history.undo(&mut doc);
        assert_eq!(doc.pcb.as_ref(), Some(&b));
        history.redo(&mut doc);
        assert_eq!(doc.pcb.as_ref(), Some(&finished));
        assert!(r.backtrack());
        assert_eq!(r.finish(0).unwrap().items.len(), 4);
    }
    #[test]
    fn closed_section_is_replaced_even_when_routing_continues_past_rejoin() {
        let b = fixture();
        let mut r = reroute(&b);
        r.place(Point::mm(8, -3), None).unwrap();
        assert_eq!(r.finish(0).unwrap().items.len(), 6);
        // Finishing and previewing are read-only; Cancel leaves source intact.
        assert_eq!(b.items.len(), 1);
        assert_eq!(r.draft().items.len(), 7);
        assert!(r.finish(1).is_err());
    }
    #[test]
    fn interior_terminals_and_locked_old_segments_survive_replacement() {
        for via in [false, true] {
            let mut b = fixture();
            b.items.push(Item::new(
                TOP,
                Some(0),
                if via {
                    Shape::Via {
                        position: Point::mm(5, 0),
                        diameter: 800_000,
                        hole: 400_000,
                        from_layer: TOP,
                        to_layer: BOTTOM,
                    }
                } else {
                    Shape::Pad {
                        position: Point::mm(5, 0),
                        size: Point::mm(1, 1),
                        shape: PadShape::Round,
                        rotation: 0.,
                        hole: 0,
                        plated: true,
                        name: "1".into(),
                        bottom_size: None,
                        middle_size: None,
                    }
                },
            ));
            let r = reroute(&b);
            assert_eq!(r.finish(0).unwrap(), *r.draft());
        }
        let mut b = fixture();
        b.items[0].shape = Shape::Track {
            start: Point::mm(0, 0),
            end: Point::mm(5, 0),
            width: 250_000,
        };
        let mut locked = Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(5, 0),
                end: Point::mm(10, 0),
                width: 250_000,
            },
        );
        locked.locked = true;
        b.items.push(locked);
        let mut r = RouteSession::begin(&b, 0, Point::mm(0, 0), TOP, None, 250_000, 0).unwrap();
        r.conflict = Conflict::Ignore;
        r.place(Point::mm(0, 3), None).unwrap();
        r.place(Point::mm(10, 3), None).unwrap();
        r.place(Point::mm(10, 0), None).unwrap();
        assert_eq!(r.finish(0).unwrap(), *r.draft());
    }
    #[test]
    fn interior_arc_or_fill_contact_is_not_disconnected() {
        for shape in [
            Shape::Arc {
                centre: Point::mm(5, 1),
                radius: 1_000_000,
                start_angle: 180.,
                end_angle: 360.,
                width: 250_000,
            },
            Shape::Fill {
                start: Point::mm(4, -1),
                end: Point::mm(6, 1),
                rotation: 0.,
            },
        ] {
            let mut b = fixture();
            b.items.push(Item::new(TOP, Some(0), shape));
            let r = reroute(&b);
            assert_eq!(r.finish(0).unwrap(), *r.draft());
        }
    }
    #[test]
    fn disabled_locked_branched_and_existing_loops_are_retained() {
        let mut disabled_net = fixture();
        disabled_net.nets[0]
            .properties
            .insert("LOOPREMOVAL".into(), "FALSE".into());
        let r = reroute(&disabled_net);
        assert_eq!(r.finish(0).unwrap(), *r.draft());
        let b = fixture();
        let mut r = reroute(&b);
        r.remove_loops = false;
        assert_eq!(r.finish(0).unwrap(), *r.draft());
        for mode in 0..3 {
            let mut b = b.clone();
            if mode == 0 {
                b.items[0].locked = true;
            } else {
                b.items.push(Item::new(
                    TOP,
                    Some(0),
                    Shape::Track {
                        start: Point::mm(if mode == 1 { 5 } else { 0 }, 0),
                        end: Point::mm(
                            if mode == 1 { 5 } else { 10 },
                            if mode == 1 { 2 } else { 0 },
                        ),
                        width: 250_000,
                    },
                ));
            }
            if mode == 0 {
                assert!(
                    RouteSession::begin(&b, 0, Point::mm(2, 0), TOP, None, 250_000, 0).is_err()
                );
            } else {
                let r = reroute(&b);
                assert_eq!(r.finish(0).unwrap(), *r.draft());
            }
        }
    }
}
