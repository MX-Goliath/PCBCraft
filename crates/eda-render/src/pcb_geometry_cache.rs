//! Document-space display geometry, bounded to the current scene revision.
use eda_core::pcb::*;
use eda_geom::Point;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Default)]
pub(super) struct Cache {
    key: Option<(u64, u64)>,
    pub bounds: HashMap<Uuid, Bounds>,
    pub outline: Arc<Vec<Point>>,
    pub board_triangles: Vec<usize>,
    pads: HashMap<(Uuid, u16), Arc<Vec<Point>>>,
    drills: HashMap<Uuid, Arc<Vec<Point>>>,
    masks: HashMap<(Uuid, bool, bool), Arc<Vec<Point>>>,
}

impl Cache {
    pub fn prepare(&mut self, board: &Board, revision: u64, preview: u64) {
        if self.key == Some((revision, preview)) {
            return;
        }
        self.key = Some((revision, preview));
        self.bounds = board.items.iter().map(|i| (i.id, i.bounds())).collect();
        self.outline = Arc::new(board.outline_points(256));
        self.board_triangles.clear();
        self.pads.clear();
        self.drills.clear();
        self.masks.clear();
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
}
