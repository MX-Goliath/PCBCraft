//! Read-only electrical selection. Net membership and physical copper are distinct.
use crate::{pcb::*, pcb_polygon};
use geo::{BooleanOps, BoundingRect, Intersects, MultiPolygon, Polygon};
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

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
    let mut nodes = vec![];
    for item in board
        .items
        .iter()
        .filter(|i| eligible(board, i) && (!same_net || nets.contains(&board.item_net(i))))
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
