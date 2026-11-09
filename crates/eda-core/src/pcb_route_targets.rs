//! Conservative source endpoint lookup, prepared once per routing revision.
use super::*;
use rstar::{AABB, RTree, RTreeObject};

#[derive(Clone)]
struct Entry {
    bounds: AABB<[f64; 2]>,
    index: usize,
}
impl RTreeObject for Entry {
    type Envelope = AABB<[f64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.bounds
    }
}
fn envelope(bounds: Bounds) -> AABB<[f64; 2]> {
    AABB::from_corners(
        [bounds.min.x as f64, bounds.min.y as f64],
        [bounds.max.x as f64, bounds.max.y as f64],
    )
}
#[derive(Clone)]
pub(super) struct Index(RTree<Entry>);
impl Index {
    pub fn new(board: &Board) -> Self {
        Self(RTree::bulk_load(
            board
                .items
                .iter()
                .enumerate()
                .filter(|(_, i)| {
                    !i.keepout
                        && !i.is_drill_only()
                        && i.polygon.is_none()
                        && matches!(
                            i.shape,
                            Shape::Pad { .. } | Shape::Via { .. } | Shape::Track { .. }
                        )
                })
                .map(|(index, item)| Entry {
                    index,
                    bounds: envelope(item.bounds()),
                })
                .collect(),
        ))
    }
    pub fn target(&self, board: &Board, p: Point, layer: u16, tolerance: i64) -> Option<Target> {
        self.draft_target(board, p, layer, tolerance, board.items.len())
    }
    /// Session source tracks only shrink when split. Their original bounds
    /// remain conservative; appended copper is checked in document order too.
    pub fn draft_target(
        &self,
        board: &Board,
        p: Point,
        layer: u16,
        tolerance: i64,
        source_len: usize,
    ) -> Option<Target> {
        if tolerance < 0 {
            return None;
        }
        // Include odd-width half-nanometre edges and the two-nanometre
        // split-at quantization tolerance conservatively. The narrow phase
        // retains the original exact distance, pad precedence and ties.
        let search = envelope(
            Bounds::points(&[p]).expanded(tolerance.saturating_add(3).min(200_000_000_003)),
        );
        let mut candidates: Vec<_> = self
            .0
            .locate_in_envelope_intersecting(&search)
            .map(|e| e.index)
            .filter(|i| *i < source_len)
            .collect();
        candidates.sort_unstable();
        candidates.extend(source_len..board.items.len());
        target_from_items(
            board,
            p,
            layer,
            tolerance,
            candidates.into_iter().filter_map(|i| board.items.get(i)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn draft_contacts_match_exhaustive_search_after_splits_placement_vias_and_backtrack() {
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(-5, 0),
                end: Point::mm(5, 0),
                width: 250_000,
            },
        ));
        let mut route =
            RouteSession::begin(&board, 0, Point::default(), TOP, None, 250_000, 0).unwrap();
        route.conflict = Conflict::Ignore;
        let result = |v: Option<Target>| v.map(|t| (t.item, t.point, t.net));
        for step in 0..5 {
            match step {
                1 => route.place(Point::mm(10, 3), None).unwrap(),
                2 => route
                    .via(Point::mm(10, 6), BOTTOM, 600_000, 300_000)
                    .unwrap(),
                3 | 4 => assert!(route.backtrack()),
                _ => {}
            }
            let index = route.targets.as_ref().unwrap();
            for layer in [TOP, BOTTOM, 2] {
                for n in 0..=200 {
                    let p = Point {
                        x: -6_000_000 + n * 90_000,
                        y: n * 47_031 % 8_000_000 - 1_000_000,
                    };
                    for tolerance in [0, 300_000, 2_000_000] {
                        assert_eq!(
                            result(index.draft_target(
                                &route.draft,
                                p,
                                layer,
                                tolerance,
                                route.source_len
                            )),
                            result(target(&route.draft, p, layer, tolerance))
                        );
                    }
                }
            }
            for point in [Point::mm(3, 0), Point::mm(10, 3), Point::mm(10, 6)] {
                assert_eq!(
                    route.endpoint_contact(point),
                    target(&route.draft, point, route.layer, 0)
                        .filter(|t| t.net.is_none() && t.point == point)
                        .map(|t| t.item)
                );
            }
        }
    }
    #[test]
    fn indexed_endpoints_match_exhaustive_picking_with_layer_spans_and_ties() {
        let mut b = Board::default();
        for n in -20..20 {
            b.items.push(Item::new(
                TOP,
                None,
                Shape::Track {
                    start: Point::mm(n, -3),
                    end: Point::mm(n + 2, 3),
                    width: 250_001,
                },
            ));
            b.items.push(Item::new(
                MULTI_LAYER,
                None,
                Shape::Pad {
                    position: Point::mm(n, 0),
                    size: Point::mm(1, 2),
                    rotation: 35.,
                    shape: PadShape::Round,
                    hole: 300_000,
                    plated: true,
                    name: "1".into(),
                    bottom_size: Some(Point::mm(3, 4)),
                    middle_size: None,
                },
            ));
        }
        let index = Index::new(&b);
        let result = |v: Option<Target>| v.map(|t| (t.item, t.point, t.net));
        for layer in [TOP, BOTTOM, 2] {
            for n in 0..500 {
                let p = Point {
                    x: n * 97_003 - 24_000_000,
                    y: n * 71_021 % 10_000_000 - 5_000_000,
                };
                for tolerance in [0, 300_000, 2_000_000] {
                    assert_eq!(
                        result(index.target(&b, p, layer, tolerance)),
                        result(target(&b, p, layer, tolerance))
                    );
                }
            }
        }
    }
}
