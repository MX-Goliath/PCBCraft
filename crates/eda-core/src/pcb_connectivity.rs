//! Physical copper connectivity and a minimum spanning tree of disconnected islands.
//! Display visibility never affects this calculation. Coordinates in geo are millimetres.
use crate::{pcb::*, pcb_polygon};
use eda_geom::Point;
use geo::{BooleanOps, BoundingRect, Intersects, MultiPolygon, Polygon};
use std::collections::BTreeMap;

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
    let mut nets: BTreeMap<u32, Vec<&Item>> = BTreeMap::new();
    for item in &board.items {
        // Native Regions6/ShapeBasedRegions6 pour fragments may inherit NET
        // from their owning polygon instead of repeating it in the fragment.
        let net = item.net.or_else(|| {
            item.polygon.and_then(|index| {
                board
                    .polygon_pours
                    .iter()
                    .find(|p| p.index == index)
                    .and_then(|p| p.net)
            })
        });
        if let Some(net) = net
            && !item.keepout
            && !item.is_drill_only()
            && !matches!(
                item.shape,
                Shape::Text { .. } | Shape::Region { cutout: true, .. }
            )
            && !item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
            && item
                .properties
                .get("PCBCRAFT_LEGACY_REGION_DUPLICATE")
                .is_none_or(|v| v != "TRUE")
            && !(item.polygon.is_some() && board.pours_stale)
            && !item.polygon.is_some_and(|index| {
                board
                    .polygon_pours
                    .iter()
                    .any(|p| p.index == index && p.shelved)
            })
        {
            nets.entry(net).or_default().push(item);
        }
    }
    let mut result = Vec::new();
    for (net, items) in nets {
        let mut nodes: Vec<Copper> = Vec::new();
        let mut barrels = Vec::new();
        for item in items {
            let begin = nodes.len();
            for layer in board
                .layers
                .iter()
                .filter(|l| l.copper && pcb_polygon::on_layer(board, item, l.id))
            {
                let mut material = pcb_polygon::copper(item, layer.id);
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
            for &b in &order[pos + 1..] {
                if nodes[a].layer != nodes[b].layer
                    || nodes[b].bounds.min().x > nodes[a].bounds.max().x
                {
                    break;
                }
                if nodes[a].bounds.intersects(&nodes[b].bounds)
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
            used[current] = true;
            for next in 0..islands.len() {
                if used[next] {
                    continue;
                }
                for &a in &islands[current] {
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
    result
}

#[cfg(test)]
mod tests {
    use super::*;
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
