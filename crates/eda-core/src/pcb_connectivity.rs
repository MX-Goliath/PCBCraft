//! Electrical copper connectivity and a minimum spanning tree of disconnected islands.
//! Display visibility never affects this calculation. Coordinates in geo are millimetres.
use crate::{pcb::*, pcb_polygon};
use eda_geom::Point;
use geo::{BoundingRect, Intersects, Polygon};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub net: u32,
    pub start: Point,
    pub end: Point,
}
struct Copper {
    layer: u16,
    shape: Polygon<f64>,
    bounds: geo::Rect<f64>,
    anchors: Vec<Point>,
}
/// Geometry-only input for a background connectivity calculation. Native
/// archives, models, component parameters and unrelated item metadata are not
/// electrical geometry. Owning this input releases interactive preview storage.
pub struct Snapshot {
    board: Board,
    order: Vec<usize>,
    derived: std::collections::BTreeSet<Uuid>,
}
impl Snapshot {
    pub fn new(board: &Board) -> Self {
        Self::selected(board, |_| true)
    }
    fn selected(board: &Board, include: impl Fn(&Item) -> bool) -> Self {
        let mut order = vec![];
        let mut derived = std::collections::BTreeSet::new();
        let items = board
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if !include(item) {
                    return None;
                }
                let net = connection_net(board, item)?;
                order.push(index);
                if item.polygon.is_some() {
                    derived.insert(item.id);
                }
                Some(Item {
                    id: item.id,
                    layer: item.layer,
                    net: Some(net),
                    component: None,
                    locked: false,
                    keepout: false,
                    polygon: None,
                    shape: item.shape.clone(),
                    source: None,
                    // Legacy pad corner geometry can depend on native properties.
                    properties: if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
                        item.properties.clone()
                    } else {
                        Fields::new()
                    },
                    pad_stack: item.pad_stack.clone(),
                    masks: None,
                })
            })
            .collect();
        Self {
            order,
            derived,
            board: Board {
                items,
                layers: board.layers.clone(),
                ..Default::default()
            },
        }
    }
    pub fn connections(&self) -> Vec<Connection> {
        connections(&self.board)
    }
    pub fn connections_cancellable(&self, cancelled: &dyn Fn() -> bool) -> Option<Vec<Connection>> {
        connections_cancellable(&self.board, cancelled)
    }
    /// Source geometry stays immutable across route/move/placement frames.
    /// These previews preserve original source slots and append new primitives;
    /// commands that reorder the source require a fresh Snapshot instead.
    /// `changed` must include every replaced, removed and appended primitive.
    pub fn preview(
        self: &std::sync::Arc<Self>,
        board: &Board,
        changed: &std::collections::BTreeSet<Uuid>,
    ) -> PreviewSnapshot {
        PreviewSnapshot {
            source: self.clone(),
            changed: Self::selected(board, |i| changed.contains(&i.id)),
            excluded: changed.clone(),
            stale: board.pours_stale,
        }
    }
}
pub struct PreviewSnapshot {
    source: std::sync::Arc<Snapshot>,
    changed: Snapshot,
    excluded: std::collections::BTreeSet<Uuid>,
    stale: bool,
}
impl PreviewSnapshot {
    pub fn connections_cancellable(&self, cancelled: &dyn Fn() -> bool) -> Option<Vec<Connection>> {
        // Assemble references on the worker, retaining original document order
        // for stable island/MST tie breaking. No prefix shapes are cloned.
        let mut items: Vec<_> = self
            .source
            .board
            .items
            .iter()
            .zip(&self.source.order)
            .filter(|(i, _)| {
                !(self.excluded.contains(&i.id)
                    || self.stale && self.source.derived.contains(&i.id))
            })
            .chain(self.changed.board.items.iter().zip(&self.changed.order))
            .map(|(item, index)| (*index, item))
            .collect();
        items.sort_unstable_by_key(|(index, _)| *index);
        connections_from_items(
            &self.source.board,
            items.into_iter().map(|(_, item)| item),
            cancelled,
        )
    }
}
fn connection_net(board: &Board, item: &Item) -> Option<u32> {
    let net = board.item_net(item)?;
    if item.keepout
        || item.is_empty_region()
        || matches!(item.shape, Shape::Arc { width: 0, .. })
        || item.is_drill_only()
        || matches!(
            item.shape,
            Shape::Text { .. } | Shape::Region { cutout: true, .. }
        )
        || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
        || item
            .properties
            .get("PCBCRAFT_LEGACY_REGION_DUPLICATE")
            .is_some_and(|v| v == "TRUE")
        || item.polygon.is_some() && board.pours_stale
        || item.polygon.is_some_and(|index| {
            board
                .polygon_pours
                .iter()
                .any(|p| p.index == index && p.shelved)
        })
    {
        return None;
    }
    Some(net)
}
fn root(parent: &mut [usize], mut n: usize) -> usize {
    while parent[n] != n {
        parent[n] = parent[parent[n]];
        n = parent[n];
    }
    n
}
fn join(parent: &mut [usize], a: usize, b: usize) {
    let a = root(parent, a);
    let b = root(parent, b);
    parent[b] = a;
}
fn distance(a: Point, b: Point) -> f64 {
    (a.x as f64 - b.x as f64).powi(2) + (a.y as f64 - b.y as f64).powi(2)
}
/// O(n²) worst case, with a per-net/layer bounds sweep before exact intersection.
/// One nearest-island edge per iteration; no quadratic edge allocation.
pub fn connections(board: &Board) -> Vec<Connection> {
    connections_cancellable(board, &|| false).unwrap_or_default()
}
/// Cancellation discards the entire result; partial airwires are never published.
pub(crate) fn connections_cancellable(
    board: &Board,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Connection>> {
    connections_from_items(board, board.items.iter(), cancelled)
}
fn connections_from_items<'a>(
    board: &Board,
    items: impl Iterator<Item = &'a Item>,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Connection>> {
    let mut nets: BTreeMap<u32, Vec<&Item>> = BTreeMap::new();
    for item in items {
        if cancelled() {
            return None;
        }
        // Native Regions6/ShapeBasedRegions6 pour fragments may inherit NET
        // from their owning polygon instead of repeating it in the fragment.
        if let Some(net) = connection_net(board, item) {
            nets.entry(net).or_default().push(item);
        }
    }
    let mut result = Vec::new();
    for (net, items) in nets {
        let mut nodes: Vec<Copper> = Vec::new();
        let mut barrels = Vec::new();
        for item in items {
            if cancelled() {
                return None;
            }
            let begin = nodes.len();
            for layer in board
                .layers
                .iter()
                .filter(|l| l.copper && pcb_polygon::on_layer(board, item, l.id))
            {
                // Electrical connectivity uses pad lands, including their
                // centres. A test pad at the centre of a connector's plated
                // hole belongs to that pad; subtracting the drill invents an
                // isolated island. Drill clearance is checked separately.
                let material = pcb_polygon::copper(item, layer.id);
                for shape in material.0 {
                    let Some(bounds) = shape.bounding_rect() else {
                        continue;
                    };
                    let anchors = match item.shape {
                        Shape::Track { start, end, .. } => vec![start, end],
                        Shape::Pad { .. } | Shape::Via { .. } => vec![item.anchor()],
                        _ => shape
                            .exterior()
                            .0
                            .iter()
                            .step_by((shape.exterior().0.len() / 16).max(1))
                            .map(|p| Point {
                                x: (p.x * 1e6).round() as i64,
                                y: (p.y * 1e6).round() as i64,
                            })
                            .collect(),
                    };
                    nodes.push(Copper {
                        layer: layer.id,
                        shape,
                        bounds,
                        anchors,
                    });
                }
            }
            if matches!(
                item.shape,
                Shape::Via { .. }
                    | Shape::Pad {
                        plated: true,
                        hole: 1..,
                        ..
                    }
            ) {
                for i in begin + 1..nodes.len() {
                    barrels.push((begin, i));
                }
            }
        }
        let mut parents: Vec<_> = (0..nodes.len()).collect();
        for (a, b) in barrels {
            join(&mut parents, a, b);
        }
        let mut order: Vec<_> = (0..nodes.len()).collect();
        order.sort_by(|&a, &b| {
            nodes[a]
                .layer
                .cmp(&nodes[b].layer)
                .then_with(|| nodes[a].bounds.min().x.total_cmp(&nodes[b].bounds.min().x))
        });
        for (pos, &a) in order.iter().enumerate() {
            if cancelled() {
                return None;
            }
            for &b in &order[pos + 1..] {
                if nodes[a].layer != nodes[b].layer
                    || nodes[b].bounds.min().x > nodes[a].bounds.max().x
                {
                    break;
                }
                if root(&mut parents, a) != root(&mut parents, b)
                    && nodes[a].bounds.intersects(&nodes[b].bounds)
                    && nodes[a].shape.intersects(&nodes[b].shape)
                {
                    join(&mut parents, a, b);
                }
            }
        }
        let mut islands: BTreeMap<usize, Vec<Point>> = BTreeMap::new();
        for (i, node) in nodes.iter().enumerate() {
            islands
                .entry(root(&mut parents, i))
                .or_default()
                .extend(&node.anchors);
        }
        let islands: Vec<_> = islands.into_values().collect();
        if islands.len() < 2 {
            continue;
        }
        let mut used = vec![false; islands.len()];
        let mut best = vec![(f64::INFINITY, Point::default(), Point::default()); islands.len()];
        let mut current = 0;
        for _ in 1..islands.len() {
            if cancelled() {
                return None;
            }
            used[current] = true;
            for next in 0..islands.len() {
                if used[next] {
                    continue;
                }
                for &a in &islands[current] {
                    if cancelled() {
                        return None;
                    }
                    for &b in &islands[next] {
                        let d = distance(a, b);
                        if d < best[next].0 {
                            best[next] = (d, a, b);
                        }
                    }
                }
            }
            let Some(next) = (0..islands.len())
                .filter(|&i| !used[i])
                .min_by(|&a, &b| best[a].0.total_cmp(&best[b].0))
            else {
                break;
            };
            result.push(Connection {
                net,
                start: best[next].1,
                end: best[next].2,
            });
            current = next;
        }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delta_snapshots_match_complete_connectivity_for_stale_pours_edits_and_removed_copper() {
        let mut b = Board::default();
        b.nets.push(Net {
            index: 0,
            name: "N".into(),
            properties: Fields::new(),
        });
        for x in [0, 5, 10] {
            b.items.push(Item::new(
                MULTI_LAYER,
                Some(0),
                Shape::Via {
                    position: Point::mm(x, 0),
                    diameter: 800_000,
                    hole: 300_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                },
            ));
        }
        let mut fill = Item::new(
            TOP,
            Some(0),
            Shape::Fill {
                start: Point::mm(-1, -1),
                end: Point::mm(11, 1),
                rotation: 0.,
            },
        );
        fill.polygon = Some(0);
        b.items.push(fill);
        let source = std::sync::Arc::new(Snapshot::new(&b));
        let check = |board: &Board, changed: &std::collections::BTreeSet<Uuid>| {
            let delta = source.preview(board, changed);
            assert_eq!(
                delta.connections_cancellable(&|| false).unwrap(),
                connections(board)
            );
            assert!(delta.connections_cancellable(&|| true).is_none());
        };
        check(&b, &Default::default());
        let mut moved = b.clone();
        moved.pours_stale = true;
        check(&moved, &Default::default());
        let id = moved.items[0].id;
        if let Shape::Via { position, .. } = &mut moved.items[0].shape {
            *position = Point::mm(2, 3);
        }
        let mut changed = std::collections::BTreeSet::from([id]);
        check(&moved, &changed);
        let added = Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(2, 3),
                end: Point::mm(5, 0),
                width: 250_000,
            },
        );
        changed.insert(added.id);
        moved.items.push(added);
        check(&moved, &changed);
        changed.insert(moved.items[1].id);
        moved.items.remove(1);
        check(&moved, &changed);
        assert_eq!(source.connections(), connections(&b));
    }
    #[test]
    fn concentric_test_pad_connects_to_connector_land_and_only_plated_barrels_join_layers() {
        let connector = Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(2, 2),
                rotation: 0.,
                shape: PadShape::Round,
                hole: 1_300_000,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        let test_pad = Item::new(
            BOTTOM,
            Some(0),
            Shape::Pad {
                position: Point::default(),
                size: Point {
                    x: 900_000,
                    y: 900_000,
                },
                rotation: 0.,
                shape: PadShape::Round,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        let mut board = Board {
            items: vec![
                connector,
                test_pad,
                track(TOP, 0, Point::default(), Point::mm(3, 0)),
            ],
            ..Default::default()
        };
        assert!(connections(&board).is_empty());
        if let Shape::Pad { plated, .. } = &mut board.items[0].shape {
            *plated = false;
        }
        assert_eq!(connections(&board).len(), 1);
    }
    #[test]
    fn cancellation_during_connectivity_never_returns_partial_airwires() {
        let board = Board {
            items: (0..100)
                .map(|n| track(TOP, 0, Point::mm(n * 3, 0), Point::mm(n * 3 + 1, 0)))
                .collect(),
            ..Default::default()
        };
        let polls = std::cell::Cell::new(0);
        let result = connections_cancellable(&board, &|| {
            polls.set(polls.get() + 1);
            polls.get() > 150
        });
        assert!(result.is_none());
        assert_eq!(polls.get(), 151);
        assert_eq!(connections(&board).len(), 99);
    }
    #[test]
    fn point_line_regions_do_not_bridge_copper_islands() {
        let a = Point::mm(1, 0);
        let b = Point::mm(4, 0);
        let remnant = Item::new(
            TOP,
            Some(0),
            Shape::Region {
                contour: vec![a, b, b, b, a],
                holes: vec![],
                cutout: false,
            },
        );
        assert!(remnant.is_empty_region());
        let board = Board {
            nets: vec![Net {
                index: 0,
                name: "N".into(),
                properties: Fields::new(),
            }],
            items: vec![
                track(TOP, 0, Point::mm(0, 0), a),
                track(TOP, 0, b, Point::mm(5, 0)),
                remnant,
            ],
            ..Default::default()
        };
        board.validate().unwrap();
        assert!(pcb_polygon::copper(&board.items[2], TOP).0.is_empty());
        assert_eq!(connections(&board).len(), 1);
        let crossed = Item::new(
            TOP,
            None,
            Shape::Region {
                contour: vec![
                    Point::mm(0, 0),
                    Point::mm(2, 2),
                    Point::mm(0, 2),
                    Point::mm(2, 0),
                ],
                holes: vec![],
                cutout: false,
            },
        );
        assert!(
            !crossed.is_empty_region(),
            "Signed-area cancellation is not collinearity"
        );
    }
    #[test]
    fn detached_snapshot_matches_physical_geometry_and_is_independent_of_later_edits() {
        let mut board = Board {
            items: vec![
                track(TOP, 0, Point::mm(-3, 0), Point::mm(-1, 0)),
                track(TOP, 0, Point::mm(1, 0), Point::mm(3, 0)),
                track(BOTTOM, 0, Point::mm(-3, 0), Point::mm(3, 0)),
            ],
            ..Default::default()
        };
        let mut pad = Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(4, 2),
                rotation: 30.,
                shape: PadShape::RoundedRectangle,
                hole: 500_000,
                plated: true,
                name: "1".into(),
                bottom_size: Some(Point::mm(3, 2)),
                middle_size: None,
            },
        );
        pad.set_stack_mode(StackMode::TopMiddleBottom, &board);
        board.items.push(pad);
        let mut bridge = track(TOP, 0, Point::mm(-1, 0), Point::mm(1, 0));
        bridge.net = None;
        bridge.polygon = Some(7);
        board.items.push(bridge);
        board.polygon_pours.push(pcb_polygon::Polygon {
            index: 7,
            net: Some(0),
            ..Default::default()
        });
        for state in 0..5 {
            board.pours_stale = state == 1;
            board.polygon_pours[0].shelved = state == 2;
            board.items[3].keepout = state == 3;
            if state == 4 {
                board.items[3]
                    .properties
                    .insert("PCBCRAFT_BODY_PROJECTION".into(), "TRUE".into());
            }
            let expected = connections(&board);
            let snapshot = Snapshot::new(&board);
            let original = board.clone();
            board.items.clear();
            assert_eq!(snapshot.connections(), expected);
            assert!(
                snapshot.board.models.is_empty()
                    && snapshot.board.source.is_none()
                    && snapshot.board.components.is_empty()
                    && snapshot.board.rules.is_empty()
            );
            board = original;
        }
    }
    fn track(layer: u16, net: u32, a: Point, b: Point) -> Item {
        Item::new(
            layer,
            Some(net),
            Shape::Track {
                start: a,
                end: b,
                width: 100_000,
            },
        )
    }
    #[test]
    fn routes_and_separate_layers_and_nets() {
        let mut b = Board {
            items: vec![
                track(TOP, 0, Point::mm(0, 0), Point::mm(1, 0)),
                track(TOP, 0, Point::mm(3, 0), Point::mm(4, 0)),
            ],
            ..Board::default()
        };
        assert_eq!(connections(&b).len(), 1);
        b.items
            .push(track(TOP, 0, Point::mm(1, 0), Point::mm(3, 0)));
        assert!(connections(&b).is_empty());
        b.items[2].layer = BOTTOM;
        assert_eq!(connections(&b).len(), 2);
        b.items[2].net = Some(1);
        assert_eq!(connections(&b).len(), 1);
    }
    #[test]
    fn keepouts_and_stale_pours_do_not_connect() {
        let mut b = Board {
            items: vec![
                track(TOP, 0, Point::mm(0, 0), Point::mm(1, 0)),
                track(TOP, 0, Point::mm(3, 0), Point::mm(4, 0)),
                track(TOP, 0, Point::mm(1, 0), Point::mm(3, 0)),
            ],
            ..Board::default()
        };
        b.items[2].keepout = true;
        assert_eq!(connections(&b).len(), 1);
        b.items[2].keepout = false;
        b.items[2].polygon = Some(0);
        b.pours_stale = true;
        assert_eq!(connections(&b).len(), 1);
        b.pours_stale = false;
        assert!(connections(&b).is_empty());
    }
    #[test]
    fn plated_via_connects_layers_only_in_its_span() {
        let mut b = Board {
            items: vec![
                track(TOP, 0, Point::mm(0, 0), Point::mm(2, 0)),
                track(BOTTOM, 0, Point::mm(0, 0), Point::mm(2, 0)),
            ],
            ..Board::default()
        };
        assert_eq!(connections(&b).len(), 1);
        b.items.push(Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Via {
                position: Point::mm(1, 0),
                diameter: 600_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        assert!(connections(&b).is_empty());
        if let Shape::Via { to_layer, .. } = &mut b.items[2].shape {
            *to_layer = TOP;
        }
        assert_eq!(connections(&b).len(), 1);
    }
    #[test]
    fn holes_and_nonplated_lands_do_not_short_copper() {
        let square = |r: i64| {
            vec![
                Point::mm(-r, -r),
                Point::mm(r, -r),
                Point::mm(r, r),
                Point::mm(-r, r),
            ]
        };
        let mut board = Board {
            items: vec![
                Item::new(
                    TOP,
                    Some(0),
                    Shape::Region {
                        contour: square(4),
                        holes: vec![square(2)],
                        cutout: false,
                    },
                ),
                track(TOP, 0, Point::mm(-1, 0), Point::mm(1, 0)),
            ],
            ..Board::default()
        };
        assert_eq!(connections(&board).len(), 1);
        board
            .items
            .push(track(TOP, 0, Point::mm(1, 0), Point::mm(3, 0)));
        assert!(connections(&board).is_empty());
        board.items = vec![Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(2, 2),
                rotation: 0.0,
                shape: PadShape::Round,
                hole: 500_000,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        )];
        assert_eq!(connections(&board).len(), 1);
        if let Shape::Pad { plated, .. } = &mut board.items[0].shape {
            *plated = true;
        }
        assert!(connections(&board).is_empty());
    }
    #[test]
    fn islands_get_a_tree_without_redundant_connections() {
        let board = Board {
            items: (0..20)
                .map(|x| track(TOP, 4, Point::mm(x * 3, 0), Point::mm(x * 3 + 1, 0)))
                .collect(),
            ..Board::default()
        };
        let lines = connections(&board);
        assert_eq!(lines.len(), 19);
        assert!(
            lines
                .iter()
                .all(|c| c.net == 4 && distance(c.start, c.end) == 4e12)
        );
        assert_eq!(lines, connections(&board));
    }
    #[test]
    fn native_pour_fragments_inherit_net_and_respect_shelving() {
        let mut bridge = track(TOP, 0, Point::mm(1, 0), Point::mm(3, 0));
        bridge.net = None;
        bridge.polygon = Some(7);
        let mut board = Board {
            items: vec![
                track(TOP, 0, Point::mm(0, 0), Point::mm(1, 0)),
                track(TOP, 0, Point::mm(3, 0), Point::mm(4, 0)),
                bridge,
            ],
            polygon_pours: vec![pcb_polygon::Polygon {
                index: 7,
                net: Some(0),
                ..Default::default()
            }],
            ..Board::default()
        };
        assert!(connections(&board).is_empty());
        board.polygon_pours[0].shelved = true;
        assert_eq!(connections(&board).len(), 1);
    }
}
