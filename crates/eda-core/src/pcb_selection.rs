//! Read-only electrical selection. Net membership and physical copper are distinct.
use crate::{pcb::*, pcb_polygon};
use geo::{BooleanOps, BoundingRect, Intersects, MultiPolygon, Polygon};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSelection {
    Inside,
    Outside,
    Touching,
    Line,
    Lasso,
}

/// Select candidates with one scene scan. GUI visibility and selection filters
/// are applied by the caller before this geometry-only operation.
pub fn in_region(
    board: &Board,
    candidates: &BTreeSet<Uuid>,
    points: &[eda_geom::Point],
    mode: RegionSelection,
    layer: u16,
) -> Vec<Uuid> {
    use geo::Contains;
    use std::collections::BTreeMap;
    if points.len() < 2 {
        return vec![];
    }
    let bounds = Bounds::points(points);
    let line: geo::LineString<f64> = points
        .iter()
        .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
        .collect();
    let rectangle = |b: Bounds| {
        geo::Rect::new(
            geo::Coord {
                x: b.min.x as f64 / 1e6,
                y: b.min.y as f64 / 1e6,
            },
            geo::Coord {
                x: b.max.x as f64 / 1e6,
                y: b.max.y as f64 / 1e6,
            },
        )
        .to_polygon()
    };
    let area = if mode == RegionSelection::Lasso {
        let mut ring = line.0.clone();
        if let Some(first) = ring.first().copied() {
            ring.push(first);
        }
        Polygon::new(ring.into(), vec![])
    } else {
        rectangle(bounds)
    };
    let components: BTreeMap<_, _> = board.components.iter().map(|c| (c.index, c.id)).collect();
    let polygons: BTreeMap<_, _> = board
        .polygon_pours
        .iter()
        .map(|p| (p.index, p.id))
        .collect();
    let mut groups: BTreeMap<Uuid, Vec<&Item>> = BTreeMap::new();
    for item in &board.items {
        let mut owners = vec![item.id];
        if let Some(id) = item.component.and_then(|n| components.get(&n)) {
            owners.push(*id);
        }
        if let Some(id) = item.polygon.and_then(|n| polygons.get(&n)) {
            owners.push(*id);
        }
        for id in owners {
            if candidates.contains(&id) {
                groups.entry(id).or_default().push(item);
            }
        }
    }
    let definitions: BTreeMap<_, _> = board.polygon_pours.iter().map(|p| (p.id, p)).collect();
    let origins: BTreeMap<_, _> = board
        .components
        .iter()
        .map(|c| (c.id, c.position))
        .collect();
    let mut selected = vec![];
    for id in candidates {
        let items = groups.get(id).map_or(&[][..], Vec::as_slice);
        let definition = definitions.get(id);
        let mut object_bounds = Bounds::empty();
        for item in items {
            object_bounds.union(item.bounds());
        }
        if let Some(p) = definition {
            object_bounds.union(Bounds::points(&p.points()));
        }
        if let Some(p) = origins.get(id) {
            object_bounds.include(*p);
        }
        if object_bounds.empty {
            continue;
        }
        if !bounds.intersects(object_bounds) {
            if mode == RegionSelection::Outside {
                selected.push(*id);
            }
            continue;
        }
        let mut shapes = MultiPolygon(vec![]);
        for item in items {
            // Text selection uses its conservative document bounds, not glyph DRC.
            if matches!(item.shape, Shape::Text { .. }) {
                shapes.0.push(rectangle(item.bounds()));
            } else {
                shapes.0.extend(
                    pcb_polygon::copper(
                        item,
                        if item.layer == MULTI_LAYER {
                            layer
                        } else {
                            item.layer
                        },
                    )
                    .0,
                );
            }
        }
        if let Some(p) = definition {
            let mut ring: Vec<_> = p
                .points()
                .iter()
                .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
                .collect();
            if let Some(first) = ring.first().copied() {
                ring.push(first);
            }
            shapes.0.push(Polygon::new(ring.into(), vec![]));
        }
        if shapes.0.is_empty() {
            shapes.0.push(rectangle(object_bounds));
        }
        let matched = match mode {
            RegionSelection::Inside | RegionSelection::Lasso => area.contains(&shapes),
            RegionSelection::Outside => !area.intersects(&shapes),
            RegionSelection::Touching => area.intersects(&shapes),
            RegionSelection::Line => line.intersects(&shapes),
        };
        if matched {
            selected.push(*id);
        }
    }
    selected
}

pub fn net_objects(board: &Board, seeds: &[Uuid]) -> Vec<Uuid> {
    let nets: BTreeSet<_> = seeds
        .iter()
        .filter_map(|id| board.item(*id))
        .filter_map(|item| board.item_net(item))
        .collect();
    board
        .items
        .iter()
        .filter(|item| {
            eligible(board, item) && board.item_net(item).is_some_and(|n| nets.contains(&n))
        })
        .map(|item| item.id)
        .collect()
}

fn eligible(board: &Board, item: &Item) -> bool {
    if item.keepout
        || item.is_empty_region()
        || item.is_drill_only()
        || matches!(
            item.shape,
            Shape::Text { .. }
                | Shape::Region { cutout: true, .. }
                | Shape::Track { width: 0, .. }
                | Shape::Arc { width: 0, .. }
        )
        || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
        || item.polygon.is_some() && board.pours_stale
    {
        return false;
    }
    !item.polygon.is_some_and(|n| {
        board
            .polygon_pours
            .iter()
            .any(|p| p.index == n && p.shelved)
    })
}

/// Connected copper on one layer or through plated barrels. A bare crossing on
/// different layers and a drilled non-plated pad never bridge those layers.
pub fn connected(board: &Board, seeds: &[Uuid], layer: Option<u16>, same_net: bool) -> Vec<Uuid> {
    connected_impl(board, seeds, layer, same_net, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionSelection {
    Net,
    Copper,
    Physical,
    PhysicalOnLayer,
    ComponentConnections,
    ComponentNets,
}
impl ConnectionSelection {
    pub fn accepts(self, board: &Board, id: Uuid) -> bool {
        if matches!(self, Self::ComponentConnections | Self::ComponentNets) {
            return board.components.iter().any(|c| c.id == id)
                || board.item(id).is_some_and(|i| i.component.is_some());
        }
        board.item(id).is_some_and(|i| {
            eligible(board, i)
                && match self {
                    Self::Physical => matches!(
                        i.shape,
                        Shape::Pad { .. }
                            | Shape::Track { .. }
                            | Shape::Arc { .. }
                            | Shape::Via { .. }
                    ),
                    Self::PhysicalOnLayer => matches!(
                        i.shape,
                        Shape::Track { .. } | Shape::Arc { .. } | Shape::Via { .. }
                    ),
                    _ => true,
                }
        })
    }
}

/// Physical routes terminate at component pad lands; free pads remain connected.
/// This is not an unrestricted copper
/// flood fill. Component modes start at all of the chosen component's pads.
pub fn connection_selection(
    board: &Board,
    seed: Uuid,
    mode: ConnectionSelection,
    layer: u16,
) -> Vec<Uuid> {
    use ConnectionSelection as S;
    if !mode.accepts(board, seed) {
        return vec![];
    }
    let component_mode = matches!(mode, S::ComponentConnections | S::ComponentNets);
    let seeds = if component_mode {
        let owner = board
            .components
            .iter()
            .find(|c| c.id == seed)
            .map(|c| c.index)
            .or_else(|| board.item(seed).and_then(|i| i.component));
        board
            .items
            .iter()
            .filter(|i| i.component == owner && matches!(i.shape, Shape::Pad { .. }))
            .map(|i| i.id)
            .collect::<Vec<_>>()
    } else {
        vec![seed]
    };
    match mode {
        S::Net | S::ComponentNets => net_objects(board, &seeds),
        S::Copper => connected(board, &seeds, None, false),
        S::Physical | S::ComponentConnections | S::PhysicalOnLayer => {
            let single = mode == S::PhysicalOnLayer;
            let layer = board
                .item(seed)
                .filter(|i| matches!(i.shape, Shape::Track { .. } | Shape::Arc { .. }))
                .map_or(layer, |i| i.layer);
            connected_impl(board, &seeds, single.then_some(layer), false, true)
                .into_iter()
                .filter(|id| {
                    board.item(*id).is_some_and(|i| {
                        matches!(i.shape, Shape::Track { .. } | Shape::Arc { .. })
                            || !single && matches!(i.shape, Shape::Via { .. })
                            || i.component.is_none() && matches!(i.shape, Shape::Pad { .. })
                    })
                })
                .collect()
        }
    }
}

fn connected_impl(
    board: &Board,
    seeds: &[Uuid],
    layer: Option<u16>,
    same_net: bool,
    physical: bool,
) -> Vec<Uuid> {
    struct Node {
        id: Uuid,
        layer: u16,
        shape: Polygon<f64>,
        bounds: geo::Rect<f64>,
        barrel: bool,
    }
    let seeds: HashSet<_> = seeds.iter().copied().collect();
    let nets: BTreeSet<_> = board
        .items
        .iter()
        .filter(|i| seeds.contains(&i.id))
        .map(|i| board.item_net(i))
        .collect();
    struct PadLand {
        shape: MultiPolygon<f64>,
        envelope: rstar::AABB<[f64; 3]>,
    }
    impl rstar::RTreeObject for PadLand {
        type Envelope = rstar::AABB<[f64; 3]>;
        fn envelope(&self) -> Self::Envelope {
            self.envelope
        }
    }
    let envelope = |b: geo::Rect<f64>, layer: u16| {
        rstar::AABB::from_corners(
            [b.min().x, b.min().y, layer as f64],
            [b.max().x, b.max().y, layer as f64],
        )
    };
    let mut lands = vec![];
    if physical {
        for item in board.items.iter().filter(|i| {
            eligible(board, i) && i.component.is_some() && matches!(i.shape, Shape::Pad { .. })
        }) {
            for copper in board.layers.iter().filter(|l| {
                l.copper
                    && layer.is_none_or(|id| id == l.id)
                    && pcb_polygon::on_layer(board, item, l.id)
            }) {
                let shape = pcb_polygon::copper(item, copper.id);
                if let Some(b) = shape.bounding_rect() {
                    lands.push(PadLand {
                        shape,
                        envelope: envelope(b, copper.id),
                    });
                }
            }
        }
    }
    let lands = rstar::RTree::bulk_load(lands);
    let mut nodes = vec![];
    for item in board
        .items
        .iter()
        .filter(|i| eligible(board, i) && (!same_net || nets.contains(&board.item_net(i))))
        .filter(|i| {
            !physical
                || matches!(
                    i.shape,
                    Shape::Track { .. } | Shape::Arc { .. } | Shape::Via { .. }
                )
                || (seeds.contains(&i.id) || i.component.is_none())
                    && matches!(i.shape, Shape::Pad { .. })
        })
    {
        let barrel = matches!(
            item.shape,
            Shape::Via { .. }
                | Shape::Pad {
                    plated: true,
                    hole: 1..,
                    ..
                }
        );
        for copper in board.layers.iter().filter(|l| {
            l.copper
                && layer.is_none_or(|id| id == l.id)
                && pcb_polygon::on_layer(board, item, l.id)
        }) {
            let mut material = pcb_polygon::copper(item, copper.id);
            if physical
                && !matches!(item.shape, Shape::Pad { .. })
                && let Some(b) = material.bounding_rect()
            {
                for pad in lands.locate_in_envelope_intersecting(&envelope(b, copper.id)) {
                    material = material.difference(&pad.shape);
                }
            }
            if item.drill_size() > 0 {
                let ring: Vec<_> = item
                    .drill_outline()
                    .iter()
                    .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
                    .collect();
                material =
                    material.difference(&MultiPolygon(vec![Polygon::new(ring.into(), vec![])]));
            }
            for shape in material.0 {
                if let Some(bounds) = shape.bounding_rect() {
                    nodes.push(Node {
                        id: item.id,
                        layer: copper.id,
                        shape,
                        bounds,
                        barrel,
                    });
                }
            }
        }
    }
    let mut edges = vec![vec![]; nodes.len()];
    let mut order: Vec<_> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| {
        nodes[a]
            .layer
            .cmp(&nodes[b].layer)
            .then_with(|| nodes[a].bounds.min().x.total_cmp(&nodes[b].bounds.min().x))
    });
    for (pos, &a) in order.iter().enumerate() {
        for &b in &order[pos + 1..] {
            if nodes[a].layer != nodes[b].layer || nodes[b].bounds.min().x > nodes[a].bounds.max().x
            {
                break;
            }
            if (!same_net
                || board.item_net(board.item(nodes[a].id).unwrap())
                    == board.item_net(board.item(nodes[b].id).unwrap()))
                && nodes[a].bounds.intersects(&nodes[b].bounds)
                && nodes[a].shape.intersects(&nodes[b].shape)
            {
                edges[a].push(b);
                edges[b].push(a);
            }
        }
    }
    let mut barrels = std::collections::BTreeMap::<_, Vec<usize>>::new();
    for (n, node) in nodes.iter().enumerate().filter(|(_, n)| n.barrel) {
        barrels.entry(node.id).or_default().push(n);
    }
    for members in barrels.values() {
        for &a in members {
            for &b in members {
                if a != b {
                    edges[a].push(b);
                }
            }
        }
    }
    let mut queue: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter_map(|(n, v)| seeds.contains(&v.id).then_some(n))
        .collect();
    let mut seen: HashSet<_> = queue.iter().copied().collect();
    let mut cursor = 0;
    while cursor < queue.len() {
        for &next in &edges[queue[cursor]] {
            if seen.insert(next) {
                queue.push(next);
            }
        }
        cursor += 1;
    }
    let ids: HashSet<_> = queue.into_iter().map(|n| nodes[n].id).collect();
    board
        .items
        .iter()
        .filter(|i| ids.contains(&i.id))
        .map(|i| i.id)
        .collect()
}

/// Test the actual stroked/filled geometry, not only its bounding rectangle.
pub fn touches_rectangle(item: &Item, bounds: Bounds, active_layer: u16) -> bool {
    if matches!(item.shape, Shape::Text { .. }) {
        return item.bounds().intersects(bounds);
    }
    let layer = if item.layer == MULTI_LAYER {
        active_layer
    } else {
        item.layer
    };
    let area = geo::Rect::new(
        geo::Coord {
            x: bounds.min.x as f64 / 1e6,
            y: bounds.min.y as f64 / 1e6,
        },
        geo::Coord {
            x: bounds.max.x as f64 / 1e6,
            y: bounds.max.y as f64 / 1e6,
        },
    )
    .to_polygon();
    pcb_polygon::copper(item, layer).intersects(&area)
}

/// The empty interior of a pour definition is not rendered copper. Selecting
/// it would turn an ordinary empty-space rectangle into an invisible move.
pub fn polygon_boundary_hit(
    polygon: &pcb_polygon::Polygon,
    at: eda_geom::Point,
    tolerance: f64,
) -> bool {
    let points = polygon.points();
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
        .any(|(a, b)| segment_distance(at, *a, *b) <= tolerance)
}

pub fn polygon_boundary_touches_rectangle(polygon: &pcb_polygon::Polygon, bounds: Bounds) -> bool {
    let area = geo::Rect::new(
        geo::Coord {
            x: bounds.min.x as f64 / 1e6,
            y: bounds.min.y as f64 / 1e6,
        },
        geo::Coord {
            x: bounds.max.x as f64 / 1e6,
            y: bounds.max.y as f64 / 1e6,
        },
    )
    .to_polygon();
    std::iter::once(polygon.points())
        .chain(polygon.holes.iter().cloned())
        .any(|ring| {
            let mut points: Vec<_> = ring
                .iter()
                .map(|p| (p.x as f64 / 1e6, p.y as f64 / 1e6))
                .collect();
            if let Some(first) = points.first().copied() {
                points.push(first);
            }
            geo::LineString::from(points).intersects(&area)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_geom::Point;
    #[test]
    fn physical_routes_stop_at_pads_even_when_tracks_meet_under_the_land() {
        use ConnectionSelection as S;
        let mut b = Board::default();
        let owner = Uuid::new_v4();
        b.components.push(Component {
            id: owner,
            index: 7,
            designator: "U1".into(),
            comment: String::new(),
            footprint: String::new(),
            parameters: None,
            position: Point::default(),
            rotation: 0.,
            layer: TOP,
            locked: false,
            properties: Fields::new(),
        });
        let mut terminal = b.components[0].clone();
        terminal.id = Uuid::new_v4();
        terminal.index = 8;
        terminal.designator = "U2".into();
        b.components.push(terminal);
        let pad = |x, y, net, component, layer| {
            let mut i = Item::new(
                layer,
                Some(net),
                Shape::Pad {
                    position: Point::mm(x, y),
                    size: Point::mm(2, 2),
                    rotation: 0.,
                    shape: PadShape::Round,
                    hole: if layer == MULTI_LAYER { 600_000 } else { 0 },
                    plated: true,
                    name: String::new(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            i.component = component;
            i
        };
        b.items = vec![
            pad(0, 0, 0, Some(7), MULTI_LAYER),
            track(TOP, 0, Point::mm(0, 0), Point::mm(5, 0)),
            Item::new(
                MULTI_LAYER,
                Some(0),
                Shape::Via {
                    position: Point::mm(5, 0),
                    diameter: 1_000_000,
                    hole: 400_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                },
            ),
            track(BOTTOM, 0, Point::mm(5, 0), Point::mm(10, 0)),
            pad(10, 0, 0, Some(8), BOTTOM),
            track(BOTTOM, 0, Point::mm(10, 0), Point::mm(20, 0)),
            pad(20, 0, 0, Some(8), BOTTOM),
            pad(0, 5, 1, Some(7), TOP),
            track(TOP, 1, Point::mm(0, 5), Point::mm(20, 5)),
            pad(20, 5, 1, Some(8), TOP),
        ];
        let original = b.clone();
        let result = connection_selection(&b, b.items[1].id, S::Physical, TOP);
        assert_eq!(result, vec![b.items[1].id, b.items[2].id, b.items[3].id]);
        assert_eq!(
            connection_selection(&b, b.items[2].id, S::PhysicalOnLayer, TOP),
            vec![b.items[1].id]
        );
        assert_eq!(
            connection_selection(&b, b.items[2].id, S::PhysicalOnLayer, BOTTOM),
            vec![b.items[3].id]
        );
        assert!(!S::PhysicalOnLayer.accepts(&b, b.items[0].id));
        let from_component = connection_selection(&b, owner, S::ComponentConnections, TOP);
        assert_eq!(
            from_component,
            vec![b.items[1].id, b.items[2].id, b.items[3].id, b.items[8].id]
        );
        assert_eq!(
            connection_selection(&b, b.items[0].id, S::ComponentNets, TOP).len(),
            b.items.len()
        );
        assert!(connection_selection(&b, b.items[1].id, S::Copper, TOP).contains(&b.items[5].id));
        assert_eq!(b, original);
        // Observed in AD24: a free pad does not terminate the physical route.
        b.items[4].component = None;
        assert_eq!(
            connection_selection(&b, b.items[1].id, S::Physical, TOP),
            vec![
                b.items[1].id,
                b.items[2].id,
                b.items[3].id,
                b.items[4].id,
                b.items[5].id
            ]
        );
        assert_eq!(
            connection_selection(&b, b.items[2].id, S::PhysicalOnLayer, BOTTOM),
            vec![b.items[3].id, b.items[4].id, b.items[5].id]
        );
        assert_eq!(
            connection_selection(&b, b.items[3].id, S::PhysicalOnLayer, TOP),
            vec![b.items[3].id, b.items[4].id, b.items[5].id]
        );
    }
    #[test]
    fn region_selection_uses_real_copper_and_keeps_outside_distinct_from_crossing() {
        let mut b = Board::default();
        let diagonal = Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(10, 10),
                width: 200_000,
            },
        );
        let id = diagonal.id;
        b.items.push(diagonal);
        let candidates = [id].into_iter().collect();
        let empty_corner = [Point::mm(0, 8), Point::mm(2, 10)];
        assert!(
            in_region(
                &b,
                &candidates,
                &empty_corner,
                RegionSelection::Touching,
                TOP
            )
            .is_empty()
        );
        assert_eq!(
            in_region(
                &b,
                &candidates,
                &empty_corner,
                RegionSelection::Outside,
                TOP
            ),
            vec![id]
        );
        assert_eq!(
            in_region(
                &b,
                &candidates,
                &[Point::mm(4, 6), Point::mm(6, 4)],
                RegionSelection::Line,
                TOP
            ),
            vec![id]
        );
        assert!(
            in_region(
                &b,
                &candidates,
                &[Point::mm(4, 4), Point::mm(6, 6)],
                RegionSelection::Inside,
                TOP
            )
            .is_empty()
        );
    }
    fn track(layer: u16, net: u32, a: Point, b: Point) -> Item {
        Item::new(
            layer,
            Some(net),
            Shape::Track {
                start: a,
                end: b,
                width: 400_000,
            },
        )
    }
    #[test]
    fn physical_selection_does_not_jump_airwires_nets_or_unplated_layer_crossings() {
        let b = Board {
            items: vec![
                track(TOP, 0, Point::mm(0, 0), Point::mm(10, 0)),
                track(TOP, 0, Point::mm(10, 0), Point::mm(20, 0)),
                track(TOP, 0, Point::mm(30, 0), Point::mm(40, 0)),
                track(BOTTOM, 0, Point::mm(10, 0), Point::mm(10, 10)),
                track(TOP, 1, Point::mm(5, -5), Point::mm(5, 5)),
            ],
            ..Default::default()
        };
        let seed = b.items[0].id;
        assert_eq!(
            connected(&b, &[seed], None, true),
            vec![seed, b.items[1].id]
        );
        assert_eq!(
            connected(&b, &[seed], None, false),
            vec![seed, b.items[1].id, b.items[4].id]
        );
        assert_eq!(net_objects(&b, &[seed]).len(), 4);
        let original = b.clone();
        assert!(!connected(&b, &[seed], Some(TOP), true).contains(&b.items[3].id));
        assert_eq!(b, original);
    }
    #[test]
    fn plated_via_bridges_only_its_actual_span_and_tab_can_limit_to_current_layer() {
        let b = Board {
            items: vec![
                track(TOP, 0, Point::mm(0, 0), Point::mm(10, 0)),
                track(BOTTOM, 0, Point::mm(10, 0), Point::mm(20, 0)),
                Item::new(
                    MULTI_LAYER,
                    Some(0),
                    Shape::Via {
                        position: Point::mm(10, 0),
                        diameter: 1_000_000,
                        hole: 400_000,
                        from_layer: TOP,
                        to_layer: BOTTOM,
                    },
                ),
            ],
            ..Default::default()
        };
        let seed = b.items[0].id;
        assert_eq!(connected(&b, &[seed], None, true).len(), 3);
        assert_eq!(
            connected(&b, &[seed], Some(TOP), true),
            vec![seed, b.items[2].id]
        );
    }
}
