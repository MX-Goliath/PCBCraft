//! Document-space display geometry, bounded to the current scene revision.
use eda_core::pcb::*;
use eda_geom::Point;
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Default)]
pub(super) struct Cache {
    key: Option<(u64, u64)>,
    source: bool,
    transformed: Option<BTreeSet<Uuid>>,
    routed: Option<(usize, Vec<Uuid>)>,
    pub bounds: HashMap<Uuid, Bounds>,
    pub indices: HashMap<Uuid, usize>,
    pub component_items: HashMap<u32, Vec<usize>>,
    pub component_bounds: HashMap<u32, Bounds>,
    pub outline: Arc<Vec<Point>>,
    pub board_triangles: Vec<usize>,
    pads: HashMap<(Uuid, u16), Arc<Vec<Point>>>,
    drills: HashMap<Uuid, Arc<Vec<Point>>>,
    masks: HashMap<(Uuid, bool, bool), Arc<Vec<Point>>>,
    spatial: Arc<Spatial>,
}

/// Original document candidates for snapping during an editing preview. Shares
/// only the immutable spatial tree, without copying display contours or model.
#[derive(Clone, Default)]
pub(super) struct SnapCache {
    revision: Option<u64>,
    spatial: Arc<Spatial>,
}
impl SnapCache {
    pub fn share_source(&mut self, source: &Cache, revision: u64) {
        if self.revision != Some(revision)
            && source.source
            && source.key.is_some_and(|key| key.0 == revision)
        {
            self.revision = Some(revision);
            self.spatial = source.spatial.clone();
        }
    }
    pub fn prepare(&mut self, board: &Board, revision: u64) {
        if self.revision == Some(revision) {
            return;
        }
        // A preview may arrive before a source frame has been displayed.
        let bounds = board.items.iter().map(|i| (i.id, i.bounds())).collect();
        self.spatial = Arc::new(Spatial::new(board, &bounds, board.items.len()));
        self.revision = Some(revision);
    }
    pub fn query(&self, bounds: Bounds) -> Vec<usize> {
        self.spatial.query(bounds)
    }
}

impl Cache {
    pub fn prepare_source(&mut self, board: &Board, revision: u64, preview: u64) {
        self.prepare(board, revision, preview);
        self.source = true;
    }
    pub fn prepare(&mut self, board: &Board, revision: u64, preview: u64) {
        self.source = false;
        if self.key == Some((revision, preview)) {
            return;
        }
        self.prepare_prefix(board, revision, preview, board.items.len());
    }
    fn prepare_prefix(&mut self, board: &Board, revision: u64, preview: u64, prefix: usize) {
        self.source = false;
        self.transformed = None;
        self.routed = None;
        self.key = Some((revision, preview));
        self.bounds = board.items[..prefix]
            .iter()
            .map(|i| (i.id, i.bounds()))
            .collect();
        self.indices = board
            .items
            .iter()
            .enumerate()
            .take(prefix)
            .map(|(index, i)| (i.id, index))
            .collect();
        self.spatial = Arc::new(Spatial::new(board, &self.bounds, prefix));
        self.component_items.clear();
        for (index, item) in board.items.iter().enumerate().take(prefix) {
            if let Some(owner) = item.component
                && !item.keepout
                && item.polygon.is_none()
                && !matches!(item.shape, Shape::Text { .. })
            {
                self.component_items.entry(owner).or_default().push(index);
            }
        }
        self.component_bounds = self
            .component_items
            .iter()
            .map(|(owner, indices)| {
                let mut bounds = Bounds::empty();
                for index in indices {
                    bounds.union(self.bounds[&board.items[*index].id]);
                }
                (*owner, bounds)
            })
            .collect();
        self.outline = Arc::new(board.outline_points(256));
        self.board_triangles.clear();
        self.pads.clear();
        self.drills.clear();
        self.masks.clear();
    }
    pub fn prepare_routed(
        &mut self,
        board: &Board,
        revision: u64,
        preview: u64,
        prefix: usize,
        changed: &BTreeSet<Uuid>,
    ) {
        if self.key == Some((revision, preview)) {
            return;
        }
        // A validated route preserves every source slot except the explicitly
        // reported splits. Keep the displayed source's tree and pad contours
        // when entering a session; rebuilding them adds a cold cursor stall.
        let source_matches = self.source
            && self.key.is_some_and(|key| key.0 == revision)
            && self.indices.len() == prefix;
        if !source_matches
            && (self.key.is_none_or(|key| key.0 != revision)
                || self.routed.as_ref().is_none_or(|r| r.0 != prefix))
        {
            self.prepare_prefix(board, revision, preview, prefix);
        }
        self.source = false;
        let old = self.routed.take().map_or_else(Vec::new, |r| r.1);
        for id in &old {
            self.bounds.remove(id);
            self.indices.remove(id);
        }
        for id in changed {
            let item = &board.items[self.indices[id]];
            self.bounds.insert(*id, item.bounds());
        }
        if !changed.is_empty() {
            Arc::make_mut(&mut self.spatial).refit(board, &self.bounds, changed);
        }
        let mut tail = vec![];
        for (index, item) in board.items.iter().enumerate().skip(prefix) {
            self.bounds.insert(item.id, item.bounds());
            self.indices.insert(item.id, index);
            tail.push(item.id);
        }
        let expire: BTreeSet<_> = old.into_iter().chain(changed.iter().copied()).collect();
        self.pads.retain(|(id, _), _| !expire.contains(id));
        self.drills.retain(|id, _| !expire.contains(id));
        self.masks.retain(|(id, _, _), _| !expire.contains(id));
        self.routed = Some((prefix, tail));
        self.key = Some((revision, preview));
    }
    pub fn prepare_transformed(
        &mut self,
        board: &Board,
        revision: u64,
        preview: u64,
        changed: &BTreeSet<Uuid>,
    ) {
        self.source = false;
        if self.key == Some((revision, preview)) {
            return;
        }
        if self.key.is_some_and(|key| key.0 == revision)
            && self.transformed.as_ref() == Some(changed)
        {
            for id in changed {
                let item = &board.items[self.indices[id]];
                self.bounds.insert(*id, item.bounds());
            }
            let owners: BTreeSet<_> = changed
                .iter()
                .filter_map(|id| board.items[self.indices[id]].component)
                .collect();
            for owner in owners {
                if let Some(indices) = self.component_items.get(&owner) {
                    let mut bounds = Bounds::empty();
                    for index in indices {
                        bounds.union(self.bounds[&board.items[*index].id]);
                    }
                    self.component_bounds.insert(owner, bounds);
                }
            }
            self.pads.retain(|(id, _), _| !changed.contains(id));
            self.drills.retain(|id, _| !changed.contains(id));
            self.masks.retain(|(id, _, _), _| !changed.contains(id));
            Arc::make_mut(&mut self.spatial).refit(board, &self.bounds, changed);
            self.key = Some((revision, preview));
        } else {
            self.prepare(board, revision, preview);
            self.transformed = Some(changed.clone());
        }
    }
    pub fn query(&self, bounds: Bounds) -> Vec<usize> {
        let mut indices = self.spatial.query(bounds);
        if let Some((_, tail)) = &self.routed {
            indices.extend(
                tail.iter()
                    .filter(|id| self.bounds[id].intersects(bounds))
                    .map(|id| self.indices[id]),
            );
        }
        indices
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
    fn new(board: &Board, bounds: &HashMap<Uuid, Bounds>, prefix: usize) -> Self {
        let mut result = Self {
            entries: board
                .items
                .iter()
                .enumerate()
                .take(prefix)
                .map(|(index, item)| {
                    let b = Self::item_bounds(board, item, bounds[&item.id]);
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
    fn item_bounds(board: &Board, item: &Item, mut b: Bounds) -> Bounds {
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
        b
    }
    fn refit(&mut self, board: &Board, bounds: &HashMap<Uuid, Bounds>, changed: &BTreeSet<Uuid>) {
        for (b, index) in &mut self.entries {
            let item = &board.items[*index];
            if changed.contains(&item.id) {
                *b = Self::item_bounds(board, item, bounds[&item.id]);
            }
        }
        // Children follow parents in storage, so a reverse pass repairs every
        // ancestor without sorting or rebuilding unaffected pad outlines.
        for index in (0..self.nodes.len()).rev() {
            let node = &self.nodes[index];
            let mut b = Bounds::empty();
            if let Some((left, right)) = node.children {
                b.union(self.nodes[left].bounds);
                b.union(self.nodes[right].bounds);
            } else {
                for (bounds, _) in &self.entries[node.range.clone()] {
                    b.union(*bounds);
                }
            }
            self.nodes[index].bounds = b;
        }
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
            self.entries[range.clone()].select_nth_unstable_by_key(range.len() / 2, |(b, _)| {
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
    #[test]
    fn snap_candidates_share_source_tree_and_stay_original_during_route_refits() {
        let mut board = board();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(4, 0),
                end: Point::mm(8, 0),
                width: 200_000,
            },
        ));
        let mut cache = Cache::default();
        cache.prepare_source(&board, 7, 20);
        let mut snap = SnapCache::default();
        snap.share_source(&cache, 7);
        assert!(Arc::ptr_eq(&snap.spatial, &cache.spatial));
        let retained = snap.clone();
        let mut route = eda_core::pcb_route::RouteSession::begin(
            &board,
            7,
            Point::mm(6, 0),
            TOP,
            None,
            200_000,
            0,
        )
        .unwrap();
        route.conflict = eda_core::pcb_route::Conflict::Ignore;
        let preview = route.preview_shared(Point::mm(6, 3)).unwrap();
        cache.prepare_routed(
            &preview.board,
            7,
            21,
            preview.source_len,
            &preview.source_changes,
        );
        assert!(!Arc::ptr_eq(&snap.spatial, &cache.spatial));
        let near_end = Bounds::points(&[Point::mm(8, 0)]).expanded(150_000);
        assert_eq!(snap.query(near_end), vec![1]);
        assert!(!cache.query(near_end).contains(&1));
        snap.share_source(&cache, 8);
        assert_eq!(snap.revision, Some(7));
        let mut original = Cache::default();
        original.prepare(&board, 7, 0);
        for x in -3..12 {
            for y in -2..5 {
                let bounds = Bounds::points(&[Point::mm(x, y)]).expanded(250_000);
                assert_eq!(snap.query(bounds), original.query(bounds));
            }
        }
        let mut moved = board.clone();
        moved.items[0] = moved.items[0].with_anchor(Point::mm(20, 20)).unwrap();
        snap.prepare(&moved, 8);
        assert!(!Arc::ptr_eq(&retained.spatial, &snap.spatial));
        assert!(snap.query(Bounds::points(&[Point::default()])).is_empty());
        assert_eq!(retained.query(near_end), vec![1]);
        cache.prepare_source(&board, 9, 0);
        snap.share_source(&cache, 9);
        assert!(Arc::ptr_eq(&snap.spatial, &cache.spatial));
        assert_eq!(snap.query(near_end), vec![1]);
    }
    #[test]
    fn canvas_preview_snaps_to_original_copper_and_preserves_grid_off_and_bypass() {
        use crate::{View, pcb::canvas};
        use egui::{Pos2, Rect, Vec2};
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(4, 0),
                end: Point::mm(8, 0),
                width: 200_000,
            },
        ));
        let original = board.clone();
        for warm_source in [false, true] {
            let ctx = egui::Context::default();
            let mut view = View::default();
            view.camera.fitted = true;
            view.camera.centre_mm = [6., 0.];
            view.camera.pixels_per_mm = 50.;
            view.pcb.initialize(&board);
            view.pcb.display.connections = false;
            let mut rect = Rect::NOTHING;
            let draw = |view: &mut View, point: Option<Pos2>, bypass, rect: &mut Rect| {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                        focused: true,
                        modifiers: egui::Modifiers {
                            ctrl: bypass,
                            shift: bypass,
                            ..Default::default()
                        },
                        events: point.map_or_else(Vec::new, |p| vec![egui::Event::PointerMoved(p)]),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            *rect = ui.available_rect_before_wrap();
                            canvas(ui, &board, view);
                        });
                    },
                );
            };
            if warm_source {
                draw(&mut view, None, false, &mut rect);
            }
            let mut route = eda_core::pcb_route::RouteSession::begin(
                &board,
                0,
                Point::mm(6, 0),
                TOP,
                None,
                200_000,
                0,
            )
            .unwrap();
            route.conflict = eda_core::pcb_route::Conflict::Ignore;
            view.pcb
                .set_route_preview(route.preview_shared(Point::mm(6, 3)).unwrap());
            view.pcb.snap.mode = SnapLayerMode::Off;
            draw(&mut view, None, false, &mut rect);
            let snapshot = view.pcb.snap_geometry.spatial.clone();
            for mode in [
                SnapLayerMode::Off,
                SnapLayerMode::Current,
                SnapLayerMode::All,
            ] {
                for grid in [false, true] {
                    for bypass in [false, true] {
                        view.pcb.snap.mode = mode;
                        view.grid = grid;
                        view.grid_nm = 100_000;
                        for raw in [
                            Point {
                                x: 8_080_000,
                                y: 80_000,
                            },
                            Point {
                                x: 6_080_000,
                                y: 3_080_000,
                            },
                        ] {
                            let pointer = view.camera.screen(raw, rect);
                            draw(&mut view, Some(pointer), bypass, &mut rect);
                            let expected = if bypass {
                                raw
                            } else {
                                board.snap_with_options(
                                    raw,
                                    if grid { view.grid_nm } else { 0 },
                                    view.pcb.active_layer,
                                    &view.pcb.visible,
                                    &view.pcb.snap,
                                )
                            };
                            assert_eq!(view.pcb.input.as_ref().unwrap().raw_point, Some(raw));
                            assert_eq!(view.pcb.input.as_ref().unwrap().point, Some(expected));
                            if mode == SnapLayerMode::Off {
                                assert!(Arc::ptr_eq(&snapshot, &view.pcb.snap_geometry.spatial));
                                assert_eq!(
                                    view.pcb.snap_geometry.revision,
                                    warm_source.then_some(0)
                                );
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(board, original);
    }
    #[test]
    fn entering_route_reuses_source_contours_and_refits_splits_but_rejects_other_previews() {
        let mut board = board();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(4, 0),
                end: Point::mm(8, 0),
                width: 200_000,
            },
        ));
        let mut route = eda_core::pcb_route::RouteSession::begin(
            &board,
            7,
            Point::mm(6, 0),
            TOP,
            None,
            200_000,
            0,
        )
        .unwrap();
        route.conflict = eda_core::pcb_route::Conflict::Ignore;
        let preview = route.preview_shared(Point::mm(6, 3)).unwrap();
        assert!(preview.source_changes.contains(&board.items[1].id));
        let mut cache = Cache::default();
        // A previous cleared preview can leave a nonzero generation.
        cache.prepare_source(&board, 7, 20);
        let pad = cache.pad(&board.items[0], TOP);
        let drill = cache.drill(&board.items[0]);
        let outline = cache.outline.clone();
        cache.prepare_routed(
            &preview.board,
            7,
            21,
            preview.source_len,
            &preview.source_changes,
        );
        assert!(Arc::ptr_eq(&pad, &cache.pad(&board.items[0], TOP)));
        assert!(Arc::ptr_eq(&drill, &cache.drill(&board.items[0])));
        assert!(Arc::ptr_eq(&outline, &cache.outline));
        let mut fresh = Cache::default();
        fresh.prepare(&preview.board, 7, 21);
        assert_eq!(cache.bounds, fresh.bounds);
        assert_eq!(cache.indices, fresh.indices);
        for x in 3..10 {
            let bounds = Bounds::points(&[Point::mm(x, 0)]).expanded(150_000);
            let mut actual = cache.query(bounds);
            actual.sort_unstable();
            assert_eq!(actual, fresh.query(bounds));
        }
        let mut other = board.clone();
        other.items[0] = other.items[0].with_anchor(Point::mm(20, 20)).unwrap();
        cache.prepare(&other, 7, 22);
        let other_pad = cache.pad(&other.items[0], TOP);
        cache.prepare_routed(
            &preview.board,
            7,
            23,
            preview.source_len,
            &preview.source_changes,
        );
        assert!(!Arc::ptr_eq(&other_pad, &cache.pad(&board.items[0], TOP)));
        assert_eq!(cache.bounds, fresh.bounds);
        cache.prepare_source(&board, 6, 24);
        let old_pad = cache.pad(&board.items[0], TOP);
        cache.prepare_routed(
            &preview.board,
            7,
            25,
            preview.source_len,
            &preview.source_changes,
        );
        assert!(!Arc::ptr_eq(&old_pad, &cache.pad(&board.items[0], TOP)));
        assert_eq!(cache.bounds, fresh.bounds);
    }
    #[test]
    fn routed_geometry_matches_fresh_queries_and_keeps_unaffected_pad_geometry() {
        let board = board();
        let mut route = eda_core::pcb_route::RouteSession::begin(
            &board,
            0,
            Point::default(),
            TOP,
            None,
            200_000,
            0,
        )
        .unwrap();
        route.conflict = eda_core::pcb_route::Conflict::Ignore;
        let mut cache = Cache::default();
        let mut retained_pad = None;
        for n in 0..15 {
            if n == 4 {
                route.place(Point::mm(3, 1), None).unwrap();
            }
            if n == 6 {
                route
                    .via(Point::mm(4, 1), BOTTOM, 600_000, 300_000)
                    .unwrap();
            }
            if n == 8 {
                assert!(route.backtrack());
            }
            if n == 10 {
                assert!(route.backtrack());
            }
            let end = if n % 3 == 0 {
                route.anchor
            } else {
                Point::mm(n, 2)
            };
            let preview = route.preview_shared(end).unwrap();
            cache.prepare_routed(
                &preview.board,
                0,
                n as u64 + 1,
                preview.source_len,
                &preview.source_changes,
            );
            if let Some(old) = &retained_pad {
                assert!(Arc::ptr_eq(old, &cache.pad(&board.items[0], TOP)));
            } else {
                retained_pad = Some(cache.pad(&board.items[0], TOP));
            }
            let mut fresh = Cache::default();
            fresh.prepare(&preview.board, 0, n as u64 + 1);
            for x in -3..20 {
                let bounds = Bounds::points(&[Point::mm(x, 2)]).expanded(1_000_000);
                let mut actual = cache.query(bounds);
                actual.sort_unstable();
                let mut expected = fresh.query(bounds);
                expected.sort_unstable();
                assert_eq!(actual, expected);
            }
            assert_eq!(cache.bounds.len(), preview.board.items.len());
            assert_eq!(cache.indices.len(), preview.board.items.len());
        }
        cache.prepare(&board, 1, 0);
        assert!(cache.routed.is_none());
        assert_eq!(cache.bounds.len(), board.items.len());
    }
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
    fn transformed_geometry_preserves_stationary_outlines_and_refits_moved_bounds() {
        let mut b = board();
        let moving = b.items[0].id;
        let mut stationary = b.items[0].clone();
        stationary.id = Uuid::new_v4();
        b.items.push(stationary);
        let changed = BTreeSet::from([moving]);
        let mut cache = Cache::default();
        cache.prepare_transformed(&b, 0, 1, &changed);
        let kept = cache.pad(&b.items[1], TOP);
        for n in 2..10 {
            b.items[0] = b.items[0].with_anchor(Point::mm(n as i64, 3)).unwrap();
            cache.prepare_transformed(&b, 0, n, &changed);
            assert!(Arc::ptr_eq(&kept, &cache.pad(&b.items[1], TOP)));
            assert_eq!(*cache.pad(&b.items[0], TOP), b.items[0].pad_outline(TOP));
            let mut fresh = Cache::default();
            fresh.prepare(&b, 0, n);
            for x in -3..15 {
                let bounds = Bounds::points(&[Point::mm(x, 3)]).expanded(500_000);
                assert_eq!(cache.query(bounds), fresh.query(bounds));
            }
        }
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
        cache.prepare_source(&board, 0, 0);
        let mut snap = SnapCache::default();
        snap.share_source(&cache, 0);
        assert!(Arc::ptr_eq(&snap.spatial, &cache.spatial));
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
                let near = Bounds::points(&[raw]).expanded(options.distance);
                let indices = snap.query(near);
                assert_eq!(indices, cache.query(near));
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
        let indices = snap.query(Bounds::points(&[raw]).expanded(options.distance));
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
        snap.prepare(&board, 1);
        assert_eq!(
            snap.query(Bounds::points(&[raw]).expanded(options.distance)),
            cache.query(Bounds::points(&[raw]).expanded(options.distance))
        );
        assert!(
            cache
                .query(Bounds::points(&[Point::mm(1, 5)]).expanded(1_000_000))
                .is_empty()
        );
    }
}
