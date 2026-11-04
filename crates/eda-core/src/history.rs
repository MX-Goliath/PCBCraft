//! Bounded document history. Historical native data is shared by the model;
//! the byte budget estimates editable snapshot data, not allocator RSS.
//! User edits use commit; derived PCB pours use a revision-checked typed result.
use crate::Document;
use serde::Serialize;
use std::collections::VecDeque;

pub const DEFAULT_MAX_STEPS: usize = 128;
pub const DEFAULT_MAX_BYTES: usize = 64 * 1024 * 1024;

struct Snapshot {
    document: Document,
    bytes: usize,
}

pub struct History {
    undo: VecDeque<Snapshot>,
    redo: Vec<Snapshot>,
    retained_bytes: usize,
    max_steps: usize,
    max_bytes: usize,
    // Swapping known snapshots during Undo/Redo does not need another traversal
    // of the entire document just to account for its retained memory.
    current_bytes: Option<usize>,
    pub revision: u64,
}

impl Default for History {
    fn default() -> Self {
        Self::with_limits(DEFAULT_MAX_STEPS, DEFAULT_MAX_BYTES)
    }
}

impl History {
    pub fn select_library_member(
        &mut self,
        doc: &mut Document,
        index: usize,
    ) -> Result<bool, String> {
        if index == 0 {
            return Ok(false);
        }
        let next = crate::library_edit::preview(doc, &crate::library_edit::Edit::Select(index))?;
        *doc = next;
        self.current_bytes = None;
        self.revision += 1;
        Ok(true)
    }
    /// Saving under a new filename changes file metadata in every snapshot;
    /// Undo must continue to address the same document and destination.
    pub fn rename_file(&mut self, current: &mut Document, name: String) -> Result<(), String> {
        let mut checked = current.clone();
        checked.name = name.clone();
        checked.validate()?;
        current.name = name.clone();
        for snapshot in self.undo.iter_mut().chain(&mut self.redo) {
            self.retained_bytes = self.retained_bytes.saturating_sub(snapshot.bytes);
            snapshot.document.name = name.clone();
            snapshot.bytes = snapshot_bytes(&snapshot.document);
            self.retained_bytes += snapshot.bytes;
        }
        self.current_bytes = None;
        Ok(())
    }
    /// Keep at least the nearest snapshot even if one large edit exceeds the
    /// byte budget. Dropping older snapshots must not make that edit irreversible.
    pub fn with_limits(max_steps: usize, max_bytes: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            retained_bytes: 0,
            max_steps: max_steps.max(1),
            max_bytes: max_bytes.max(1),
            current_bytes: None,
            revision: 0,
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn retained_steps(&self) -> usize {
        self.undo.len() + self.redo.len()
    }
    pub fn estimated_retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    fn trim(&mut self, keep_undo: bool) {
        while self.retained_steps() > 1
            && (self.retained_steps() > self.max_steps || self.retained_bytes > self.max_bytes)
        {
            // The closest undo/redo entries are at the back. Evict remote
            // history first, without changing the monotonic document revision.
            let oldest = if self.undo.len() > usize::from(keep_undo) {
                self.undo.pop_front().unwrap()
            } else {
                self.redo.remove(0)
            };
            self.retained_bytes = self.retained_bytes.saturating_sub(oldest.bytes);
        }
    }
    pub fn commit(&mut self, current: &mut Document, mut next: Document) -> Result<bool, String> {
        if current.id != next.id || current.kind != next.kind {
            return Err("An edit cannot replace document identity or kind".into());
        }
        crate::library_edit::own_new_geometry(&mut next);
        next.validate()?;
        if *current == next {
            return Ok(false);
        }
        let bytes = self
            .current_bytes
            .take()
            .unwrap_or_else(|| snapshot_bytes(current));
        self.retained_bytes = self
            .retained_bytes
            .saturating_sub(self.redo.iter().map(|s| s.bytes).sum::<usize>());
        self.redo.clear();
        self.undo.push_back(Snapshot {
            document: std::mem::replace(current, next),
            bytes,
        });
        self.retained_bytes = self.retained_bytes.saturating_add(bytes);
        self.trim(true);
        self.revision += 1;
        Ok(true)
    }
    /// PCB controllers already own the validated replacement board. Copy only
    /// the document envelope, rather than cloning and then discarding its PCB.
    pub fn commit_pcb(
        &mut self,
        current: &mut Document,
        board: crate::pcb::Board,
    ) -> Result<bool, String> {
        if current.kind != crate::DocumentKind::Board {
            return Err("Not a PCB document".into());
        }
        let next = Document {
            library: current.library.clone(),
            schema_version: current.schema_version,
            id: current.id,
            name: current.name.clone(),
            kind: current.kind,
            size: current.size,
            pads: current.pads.clone(),
            tracks: current.tracks.clone(),
            schematic: current.schematic.clone(),
            pcb: Some(board),
        };
        self.commit(current, next)
    }
    pub fn undo(&mut self, doc: &mut Document) {
        if let Some(previous) = self.undo.pop_back() {
            let bytes = self
                .current_bytes
                .take()
                .unwrap_or_else(|| snapshot_bytes(doc));
            self.current_bytes = Some(previous.bytes);
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(previous.bytes)
                .saturating_add(bytes);
            self.redo.push(Snapshot {
                document: std::mem::replace(doc, previous.document),
                bytes,
            });
            self.trim(false);
            self.revision += 1;
        }
    }
    /// Refresh only geometry produced by the pure polygon generator. Preserve
    /// the edit's Undo entry and the remaining Redo chain; expire view caches.
    pub fn apply_pcb_pour(
        &mut self,
        doc: &mut Document,
        result: crate::pcb_polygon::PreparedPour,
    ) -> Result<bool, String> {
        if result.document != doc.id || doc.kind != crate::DocumentKind::Board || doc.pcb.is_none()
        {
            return Err("Polygon result belongs to another PCB document".into());
        }
        if result.revision != self.revision {
            return Ok(false);
        }
        result.board.validate()?;
        if doc.pcb.as_ref() == Some(&result.board) {
            return Ok(false);
        }
        doc.pcb = Some(result.board);
        self.current_bytes = None;
        self.revision += 1;
        Ok(true)
    }
    pub fn redo(&mut self, doc: &mut Document) {
        if let Some(next) = self.redo.pop() {
            let bytes = self
                .current_bytes
                .take()
                .unwrap_or_else(|| snapshot_bytes(doc));
            self.current_bytes = Some(next.bytes);
            self.retained_bytes = self
                .retained_bytes
                .saturating_sub(next.bytes)
                .saturating_add(bytes);
            self.undo.push_back(Snapshot {
                document: std::mem::replace(doc, next.document),
                bytes,
            });
            self.trim(true);
            self.revision += 1;
        }
    }
}

/// Count serialization without allocating a JSON string. A conservative factor
/// allows for collection/string overhead. Native archives/records are immutable
/// Arcs shared across history, so do not charge those again for every edit.
fn snapshot_bytes(doc: &Document) -> usize {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(data.len());
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    fn count(value: &impl Serialize) -> usize {
        let mut counter = Counter(0);
        if serde_json::to_writer(&mut counter, value).is_err() {
            return usize::MAX / 4;
        }
        counter.0
    }
    let mut bytes = count(&(&doc.name, &doc.pads, &doc.tracks));
    if let Some(s) = &doc.schematic {
        bytes = bytes.saturating_add(count(&(
            &s.sheet,
            &s.fonts,
            &s.components,
            &s.primitives,
            &s.parameter_objects,
            &s.images,
            &s.warnings,
        )));
    }
    if let Some(b) = &doc.pcb {
        bytes = bytes.saturating_add(count(&(
            &b.layer_stack,
            &b.properties,
            &b.layers,
            &b.nets,
            &b.components,
            &b.items,
            &b.outline,
            &b.rules,
            &b.classes,
            &b.polygons,
            &b.polygon_pours,
            &b.warnings,
        )));
    }
    bytes
        .saturating_mul(2)
        .saturating_add(std::mem::size_of::<Document>())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pcb_doc() -> Document {
        use crate::{pcb::*, pcb_polygon::*};
        let mut doc = Document::new(crate::DocumentKind::Board, "Background pour");
        let mut board = Board::from_legacy(&doc);
        board.items.push(Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: eda_geom::Point::mm(5, 5),
                diameter: 600_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        board.polygon_pours.push(Polygon {
            contour: straight_contour(&[
                eda_geom::Point::mm(0, 0),
                eda_geom::Point::mm(10, 0),
                eda_geom::Point::mm(10, 10),
                eda_geom::Point::mm(0, 10),
            ]),
            ..Default::default()
        });
        doc.pcb = Some(repour(&board).unwrap());
        doc
    }
    fn move_via(doc: &mut Document, history: &mut History, dx: i64) {
        use crate::pcb::*;
        let board = doc.pcb.as_ref().unwrap();
        let via = board
            .items
            .iter()
            .find(|i| matches!(i.shape, Shape::Via { .. }))
            .unwrap();
        let next = board
            .edited(Edit::Transform {
                selection: vec![via.id],
                pivot: via.anchor(),
                translation: eda_geom::Point::mm(dx, 0),
                angle: 0.,
            })
            .unwrap();
        assert!(history.commit_pcb(doc, next).unwrap());
    }
    fn prepare(doc: &Document, history: &History) -> crate::pcb_polygon::PreparedPour {
        crate::pcb_polygon::prepare_repour(doc.id, history.revision, doc.pcb.as_ref().unwrap())
            .unwrap()
    }
    #[test]
    fn derived_pour_is_part_of_the_move_single_undo_and_redo() {
        let mut doc = pcb_doc();
        let original = doc.clone();
        let mut history = History::default();
        move_via(&mut doc, &mut history, 1);
        assert!(doc.pcb.as_ref().unwrap().pours_stale);
        let prepared = prepare(&doc, &history);
        assert!(history.apply_pcb_pour(&mut doc, prepared).unwrap());
        assert!(!doc.pcb.as_ref().unwrap().pours_stale);
        let moved = doc.clone();
        assert_eq!(history.retained_steps(), 1);
        assert_eq!(history.revision, 2);
        history.undo(&mut doc);
        assert_eq!(doc, original);
        assert!(!history.can_undo());
        history.redo(&mut doc);
        assert_eq!(doc, moved);
    }
    #[test]
    fn delayed_pour_after_undo_or_new_edit_cannot_overwrite_the_model() {
        let mut doc = pcb_doc();
        let original = doc.clone();
        let mut history = History::default();
        move_via(&mut doc, &mut history, 1);
        let late = prepare(&doc, &history);
        history.undo(&mut doc);
        assert!(!history.apply_pcb_pour(&mut doc, late).unwrap());
        assert_eq!(doc, original);
        assert!(history.can_redo());
        history.redo(&mut doc);
        let late = prepare(&doc, &history);
        move_via(&mut doc, &mut history, 1);
        let newer = doc.clone();
        assert!(!history.apply_pcb_pour(&mut doc, late).unwrap());
        assert_eq!(doc, newer);
    }
    #[test]
    fn derived_refresh_keeps_redo_chain_and_rejects_another_document() {
        let mut doc = pcb_doc();
        let mut history = History::default();
        move_via(&mut doc, &mut history, 1);
        move_via(&mut doc, &mut history, 1);
        history.undo(&mut doc);
        let prepared = prepare(&doc, &history);
        assert!(history.apply_pcb_pour(&mut doc, prepared).unwrap());
        let first_move = doc.clone();
        assert!(history.can_redo());
        history.redo(&mut doc);
        assert!(doc.pcb.as_ref().unwrap().pours_stale);
        let prepared = prepare(&doc, &history);
        history.apply_pcb_pour(&mut doc, prepared).unwrap();
        history.undo(&mut doc);
        assert_eq!(doc, first_move);
        let prepared = prepare(&doc, &history);
        let mut other = pcb_doc();
        let before = other.clone();
        assert!(history.apply_pcb_pour(&mut other, prepared).is_err());
        assert_eq!(other, before);
    }
    #[test]
    fn long_edit_session_is_bounded_and_preserves_nearest_undo_redo() {
        let mut doc = Document::example();
        let mut history = History::with_limits(5, usize::MAX);
        for n in 1..=100 {
            let mut next = doc.clone();
            next.name = n.to_string();
            history.commit(&mut doc, next).unwrap();
        }
        assert_eq!(history.retained_steps(), 5);
        for expected in (95..100).rev() {
            history.undo(&mut doc);
            assert_eq!(doc.name, expected.to_string());
        }
        assert!(!history.can_undo());
        assert_eq!(history.retained_steps(), 5);
        for expected in 96..=100 {
            history.redo(&mut doc);
            assert_eq!(doc.name, expected.to_string());
        }
        assert_eq!(history.revision, 110);
    }
    #[test]
    fn byte_budget_noops_and_branching_release_remote_snapshots() {
        let mut doc = Document::example();
        let mut history = History::with_limits(100, snapshot_bytes(&doc) * 3);
        for n in 0..100 {
            let mut next = doc.clone();
            next.name = format!("Edit {n}");
            history.commit(&mut doc, next).unwrap();
            assert!(history.estimated_retained_bytes() <= history.max_bytes);
        }
        assert!(history.retained_steps() < 100);
        let bytes = history.estimated_retained_bytes();
        let unchanged = doc.clone();
        assert!(!history.commit(&mut doc, unchanged).unwrap());
        assert_eq!(bytes, history.estimated_retained_bytes());
        history.undo(&mut doc);
        let mut branch = doc.clone();
        branch.name = "Branch".into();
        history.commit(&mut doc, branch).unwrap();
        assert!(!history.can_redo());
        assert_eq!(
            history.retained_bytes,
            history.undo.iter().map(|s| s.bytes).sum::<usize>()
        );
    }
    #[test]
    fn one_oversized_snapshot_can_still_be_undone() {
        let mut doc = Document::example();
        let original = doc.clone();
        let mut history = History::with_limits(5, 1);
        let mut next = doc.clone();
        next.name = "Changed".into();
        history.commit(&mut doc, next).unwrap();
        assert_eq!(history.retained_steps(), 1);
        history.undo(&mut doc);
        assert_eq!(doc, original);
        history.redo(&mut doc);
        assert_eq!(doc.name, "Changed");
    }

    #[test]
    fn visibility_history_shares_native_assets_and_releases_them_on_close() {
        use crate::{
            DocumentKind, Point,
            schematic::*,
            schematic_edit::{self, Edit, Request},
        };
        use std::{collections::BTreeMap, sync::Arc};
        let mut doc = Document::new(DocumentKind::Schematic, "Memory regression");
        let s = doc.schematic.as_mut().unwrap();
        s.source = Some(Arc::new(NativeSource {
            format: "Altium SchDoc".into(),
            filename: "original.SchDoc".into(),
            original_base64: "A".repeat(512 * 1024),
        }));
        s.records = Arc::new(vec![SourceRecord {
            index: 0,
            stream: "FileHeader".into(),
            record_type: 4,
            fields: BTreeMap::from([("TEXT".into(), "original".repeat(1024))]),
        }]);
        s.images = Arc::new(BTreeMap::from([("logo.png".into(), "A".repeat(4096))]));
        let p = schematic_edit::primitive(
            SchPrimitiveKind::Text {
                position: Point::mm(1, 1),
                text: "Label".into(),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            None,
        );
        let id = p.id;
        s.primitives.push(p);
        let source = s.source.as_ref().unwrap().clone();
        let records = s.records.clone();
        let images = s.images.clone();
        let saved = doc.clone();
        let mut history = History::default();
        for _ in 0..300 {
            let mut value = doc
                .schematic
                .as_ref()
                .unwrap()
                .primitive(id)
                .unwrap()
                .clone();
            value.presentation.hidden = !value.presentation.hidden;
            let request = Request {
                document: doc.id,
                expected_revision: history.revision,
                edit: Edit::PrimitiveProperties {
                    id,
                    value: Box::new(value),
                },
            };
            history.schematic_edit(&mut doc, request).unwrap();
        }
        assert_eq!(history.retained_steps(), DEFAULT_MAX_STEPS);
        for snapshot in &history.undo {
            let s = snapshot.document.schematic.as_ref().unwrap();
            assert!(Arc::ptr_eq(s.source.as_ref().unwrap(), &source));
            assert!(Arc::ptr_eq(&s.records, &records));
            assert!(Arc::ptr_eq(&s.images, &images));
        }
        drop((history, doc, saved));
        assert_eq!(Arc::strong_count(&source), 1);
        assert_eq!(Arc::strong_count(&records), 1);
        assert_eq!(Arc::strong_count(&images), 1);
    }
}
