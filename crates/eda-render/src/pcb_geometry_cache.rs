//! Document-space display geometry, bounded to the current scene revision.
use eda_core::pcb::*;
use eda_geom::Point;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Default)]
pub(super) struct Cache {
    key: Option<(u64, u64)>,
    pub bounds: HashMap<Uuid, Bounds>,
    pub indices: HashMap<Uuid, usize>,
    pub outline: Arc<Vec<Point>>,
    pub board_triangles: Vec<usize>,
    pads: HashMap<(Uuid, u16), Arc<Vec<Point>>>,
    drills: HashMap<Uuid, Arc<Vec<Point>>>,
    masks: HashMap<(Uuid, bool, bool), Arc<Vec<Point>>>,
    spatial: Spatial,
}

impl Cache {
    pub fn prepare(&mut self, board: &Board, revision: u64, preview: u64) {
        if self.key == Some((revision, preview)) {
            return;
        }
        self.key = Some((revision, preview));
        self.bounds = board.items.iter().map(|i| (i.id, i.bounds())).collect();
        self.indices = board
            .items
            .iter()
            .enumerate()
            .map(|(index, i)| (i.id, index))
            .collect();
        self.spatial = Spatial::new(board, &self.bounds);
        self.outline = Arc::new(board.outline_points(256));
        self.board_triangles.clear();
        self.pads.clear();
        self.drills.clear();
        self.masks.clear();
    }
    pub fn query(&self, bounds: Bounds) -> Vec<usize> {
        self.spatial.query(bounds)
    }
    pub fn pad(&mut self, item: &Item, layer: u16) -> Arc<Vec<Point>> {
        self.pads
            .entry((item.id, layer))
            .or_insert_with(|| Arc::new(item.pad_outline(layer)))
            .clone()
    }
    pub fn drill(&mut self, item: &Item) -> Arc<Vec<Point>> {
        self.drills
            .entry(item.id)
            .or_insert_with(|| Arc::new(item.drill_outline()))
            .clone()
    }
    pub fn mask(
        &mut self,
        board: &Board,
        item: &Item,
        kind: MaskKind,
        bottom: bool,
    ) -> Arc<Vec<Point>> {
        self.masks
            .entry((item.id, kind == MaskKind::Paste, bottom))
            .or_insert_with(|| Arc::new(item.mask_outline(board, kind, bottom).unwrap_or_default()))
            .clone()
    }
}

#[derive(Clone, Default)]
struct Spatial {
    entries: Vec<(Bounds, usize)>,
    nodes: Vec<Node>,
}
#[derive(Clone)]
struct Node {
    bounds: Bounds,
    range: std::ops::Range<usize>,
    children: Option<(usize, usize)>,
}
impl Spatial {
    fn new(board: &Board, bounds: &HashMap<Uuid, Bounds>) -> Self {
        let mut result = Self {
            entries: board
                .items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let mut b = bounds[&item.id];
                    b.include(item.anchor());
                    if matches!(
                        item.shape,
                        Shape::Pad {
                            bottom_size: Some(_),
                            ..
                        } | Shape::Pad {
                            middle_size: Some(_),
                            ..
                        }
                    ) || item.pad_stack.is_some()
                    {
                        for layer in board.layers.iter().filter(|l| l.copper) {
                            b.union(Bounds::points(&item.pad_outline(layer.id)));
                        }
                    }
                    (b, index)
                })
                .collect(),
            nodes: vec![],
        };
        if !result.entries.is_empty() {
            result.build(0..result.entries.len());
        }
        result
    }
    fn build(&mut self, range: std::ops::Range<usize>) -> usize {
        let mut bounds = Bounds::empty();
        for (b, _) in &self.entries[range.clone()] {
            bounds.union(*b);
        }
        let index = self.nodes.len();
        self.nodes.push(Node {
            bounds,
            range: range.clone(),
            children: None,
        });
        if range.len() > 8 {
            let x = bounds.max.x - bounds.min.x >= bounds.max.y - bounds.min.y;
            self.entries[range.clone()].sort_unstable_by_key(|(b, _)| {
                if x {
                    b.min.x / 2 + b.max.x / 2
                } else {
                    b.min.y / 2 + b.max.y / 2
                }
            });
            let mid = range.start + range.len() / 2;
            let left = self.build(range.start..mid);
            let right = self.build(mid..range.end);
            self.nodes[index].children = Some((left, right));
        }
        index
    }
    fn query(&self, bounds: Bounds) -> Vec<usize> {
        let mut found = vec![];
        if self.nodes.is_empty() {
            return found;
        }
        let mut pending = vec![0];
        while let Some(index) = pending.pop() {
            let node = &self.nodes[index];
            if !node.bounds.intersects(bounds) {
                continue;
            }
            if let Some((left, right)) = node.children {
                pending.extend([left, right]);
            } else {
                found.extend(
                    self.entries[node.range.clone()]
                        .iter()
                        .filter(|(b, _)| b.intersects(bounds))
                        .map(|(_, i)| *i),
                );
            }
        }
        found.sort_unstable();
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn board() -> Board {
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(2, 1),
                rotation: 30.,
                shape: PadShape::Round,
                hole: 300_000,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        ));
        board
    }
    #[test]
    fn cached_geometry_matches_model_and_is_reused_between_frames() {
        let board = board();
        let before = board.clone();
        let item = &board.items[0];
        let mut cache = Cache::default();
        cache.prepare(&board, 0, 0);
        let pad = cache.pad(item, TOP);
        let hole = cache.drill(item);
        let mask = cache.mask(&board, item, MaskKind::Solder, false);
        assert_eq!(*pad, item.pad_outline(TOP));
        assert_eq!(*hole, item.drill_outline());
        assert_eq!(
            *mask,
            item.mask_outline(&board, MaskKind::Solder, false).unwrap()
        );
        cache.prepare(&board, 0, 0);
        assert!(Arc::ptr_eq(&pad, &cache.pad(item, TOP)));
        assert!(Arc::ptr_eq(&hole, &cache.drill(item)));
        assert!(Arc::ptr_eq(
            &mask,
            &cache.mask(&board, item, MaskKind::Solder, false)
        ));
        assert_eq!(board, before);
    }
    #[test]
    fn edits_previews_undo_and_deletion_expire_geometry() {
        let mut board = board();
        let mut cache = Cache::default();
        cache.prepare(&board, 0, 0);
        let original = cache.pad(&board.items[0], TOP);
        if let Shape::Pad { position, .. } = &mut board.items[0].shape {
            *position = Point::mm(5, 4);
        }
        for (revision, preview) in [(0, 1), (1, 2), (2, 2)] {
            cache.prepare(&board, revision, preview);
            let changed = cache.pad(&board.items[0], TOP);
            assert!(!Arc::ptr_eq(&original, &changed));
            assert_eq!(*changed, board.items[0].pad_outline(TOP));
        }
        board.items.clear();
        cache.prepare(&board, 3, 2);
        assert!(
            cache.bounds.is_empty()
                && cache.pads.is_empty()
                && cache.drills.is_empty()
                && cache.masks.is_empty()
        );
    }
    #[test]
    fn spatial_candidates_match_full_snapping_and_expire_after_geometry_changes() {
        let mut board = board();
        board.layers.push(PcbLayer {
            id: 2,
            name: "Mid1".into(),
            copper: true,
            order: 1,
        });
        board.items[0].layer = MULTI_LAYER;
        if let Shape::Pad { middle_size, .. } = &mut board.items[0].shape {
            *middle_size = Some(Point::mm(6, 6));
        }
        for n in 0..500 {
            board.items.push(Item::new(
                TOP,
                None,
                Shape::Track {
                    start: Point::mm(n * 3, 5),
                    end: Point::mm(n * 3 + 1, 5),
                    width: 200_000,
                },
            ));
        }
        let mut cache = Cache::default();
        cache.prepare(&board, 0, 0);
        let visible = [TOP, 2, BOTTOM, MULTI_LAYER].into();
        for mode in [
            SnapLayerMode::All,
            SnapLayerMode::Current,
            SnapLayerMode::Off,
        ] {
            let options = SnapOptions {
                mode,
                ..Default::default()
            };
            for n in 0..100 {
                let raw = Point {
                    x: n * 3_000_000 + 70_000,
                    y: 5_080_000,
                };
                let indices = cache.query(Bounds::points(&[raw]).expanded(options.distance));
                assert!(indices.len() < 10);
                assert_eq!(
                    board.snap_with_items(
                        raw,
                        100_000,
                        TOP,
                        &visible,
                        &options,
                        indices.into_iter().map(|i| &board.items[i])
                    ),
                    board.snap_with_options(raw, 100_000, TOP, &visible, &options)
                );
            }
        }
        let raw = Point {
            x: 3_080_000,
            y: 400_000,
        };
        let options = SnapOptions {
            mode: SnapLayerMode::All,
            ..Default::default()
        };
        let indices = cache.query(Bounds::points(&[raw]).expanded(options.distance));
        assert_ne!(board.snap_with_options(raw, 0, 2, &visible, &options), raw);
        assert_eq!(
            board.snap_with_items(
                raw,
                0,
                2,
                &visible,
                &options,
                indices.into_iter().map(|i| &board.items[i])
            ),
            board.snap_with_options(raw, 0, 2, &visible, &options)
        );
        board.items.clear();
        cache.prepare(&board, 1, 0);
        assert!(
            cache
                .query(Bounds::points(&[Point::mm(1, 5)]).expanded(1_000_000))
                .is_empty()
        );
    }
}
