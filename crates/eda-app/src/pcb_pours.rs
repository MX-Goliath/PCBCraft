//! At most one background pour per document. Edits remain immediately usable;
//! only the matching revision may receive derived copper, without another Undo.
use super::OpenDocument;
use eda_core::pcb_polygon::{self, PreparedPour};
use std::sync::mpsc;

struct Job {
    revision: u64,
    receiver: mpsc::Receiver<Result<PreparedPour, String>>,
}
#[derive(Default)]
pub(super) struct State {
    job: Option<Job>,
    failed_revision: Option<u64>,
}
impl State {
    pub fn update(
        &mut self,
        doc: &mut OpenDocument,
        ctx: &egui::Context,
        allow_apply: bool,
    ) -> Result<(), String> {
        let revision = doc.history.revision;
        let editing = !allow_apply
            || doc.pcb_editor.has_pending()
            || doc
                .model
                .pcb
                .as_ref()
                .is_some_and(|b| doc.pcb_editor.properties_dirty(b, revision));
        let mut error = None;
        if let Some(job) = &self.job
            && (job.revision != revision || !editing)
        {
            match job.receiver.try_recv() {
                Ok(result) => {
                    if job.revision == revision {
                        match result {
                            Ok(result) => {
                                // Saving a stale JSON is safe: it retains the stale
                                // flag and is rebuilt on open. A cache refresh must
                                // not dirty a document saved during the calculation.
                                let clean = !doc.content_is_dirty();
                                match doc.history.apply_pcb_pour(&mut doc.model, result) {
                                    Ok(true) => {
                                        if clean {
                                            doc.mark_saved();
                                        }
                                        ctx.request_repaint();
                                    }
                                    Ok(false) => {}
                                    Err(message) => {
                                        self.failed_revision = Some(revision);
                                        error = Some(format!(
                                            "Could not apply polygon repour: {message}"
                                        ));
                                    }
                                }
                            }
                            Err(message) => {
                                self.failed_revision = Some(revision);
                                error = Some(format!("Polygon repour failed: {message}"));
                            }
                        }
                    }
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    if job.revision == revision {
                        self.failed_revision = Some(revision);
                        error = Some(
                            "Polygon repour worker stopped; use Repour All Polygons to retry"
                                .into(),
                        );
                    }
                    self.job = None;
                }
            }
        }
        if !editing
            && self.job.is_none()
            && self.failed_revision != Some(doc.history.revision)
            && let Some(board) = doc.model.pcb.as_ref().filter(|b| b.pours_stale)
        {
            let snapshot = board.clone();
            let document = doc.model.id;
            let revision = doc.history.revision;
            let (sender, receiver) = mpsc::sync_channel(1);
            self.job = Some(Job { revision, receiver });
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = pcb_polygon::prepare_repour(document, revision, &snapshot);
                let _ = sender.send(result);
                ctx.request_repaint();
            });
        }
        doc.view.pcb.repour_pending = self.job.is_some();
        if self.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(25));
        }
        error.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_core::{Document, DocumentKind, pcb::*, pcb_polygon::*};
    use eda_geom::Point;
    fn doc() -> OpenDocument {
        let mut model = Document::new(DocumentKind::Board, "Background.pcbcraft.json");
        let mut board = Board::from_legacy(&model);
        board.items.push(Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::mm(5, 5),
                diameter: 600_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        board.polygon_pours.push(Polygon {
            contour: straight_contour(&[
                Point::mm(0, 0),
                Point::mm(10, 0),
                Point::mm(10, 10),
                Point::mm(0, 10),
            ]),
            ..Default::default()
        });
        board.pours_stale = true;
        model.pcb = Some(board);
        OpenDocument::new(model, None)
    }
    fn prepared(doc: &OpenDocument) -> PreparedPour {
        prepare_repour(
            doc.model.id,
            doc.history.revision,
            doc.model.pcb.as_ref().unwrap(),
        )
        .unwrap()
    }
    fn waiting(revision: u64) -> (State, mpsc::SyncSender<Result<PreparedPour, String>>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        (
            State {
                job: Some(Job { revision, receiver }),
                ..Default::default()
            },
            sender,
        )
    }
    #[test]
    fn ready_result_waits_for_active_gesture_and_does_not_cancel_it() {
        let mut doc = doc();
        let (mut state, sender) = waiting(0);
        assert!(sender.send(Ok(prepared(&doc))).is_ok());
        doc.pcb_editor.start = Some(Point::mm(1, 1));
        let before = doc.model.clone();
        let ctx = egui::Context::default();
        state.update(&mut doc, &ctx, true).unwrap();
        assert_eq!(doc.model, before);
        assert_eq!(doc.history.revision, 0);
        assert!(state.job.is_some());
        doc.pcb_editor.start = None;
        state.update(&mut doc, &ctx, false).unwrap();
        assert_eq!(doc.history.revision, 0);
        state.update(&mut doc, &ctx, true).unwrap();
        assert!(!doc.model.pcb.as_ref().unwrap().pours_stale);
        assert!(state.job.is_none());
        assert!(!doc.history.can_undo());
    }
    #[test]
    fn newer_edits_coalesce_behind_one_worker_and_discard_old_result() {
        let mut doc = doc();
        let old = prepared(&doc);
        let (mut state, sender) = waiting(0);
        let mut next = doc.model.pcb.as_ref().unwrap().clone();
        let Shape::Via { position, .. } = &mut next.items[0].shape else {
            panic!()
        };
        *position = Point::mm(6, 5);
        super::super::commit(&mut doc, next).unwrap();
        let ctx = egui::Context::default();
        for _ in 0..20 {
            state.update(&mut doc, &ctx, true).unwrap();
        }
        assert_eq!(state.job.as_ref().unwrap().revision, 0);
        let newer = doc.model.clone();
        assert!(sender.send(Ok(old)).is_ok());
        state.update(&mut doc, &ctx, true).unwrap();
        assert_eq!(doc.model, newer);
        assert_eq!(state.job.as_ref().unwrap().revision, doc.history.revision);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while doc.model.pcb.as_ref().unwrap().pours_stale {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
            state.update(&mut doc, &ctx, true).unwrap();
        }
        assert_eq!(
            doc.model.pcb.as_ref().unwrap().items[0].anchor(),
            Point::mm(6, 5)
        );
        assert_eq!(doc.history.retained_steps(), 1);
    }
    #[test]
    fn saving_during_calculation_stays_clean_after_derived_refresh() {
        let mut doc = doc();
        doc.mark_saved();
        let (mut state, sender) = waiting(0);
        assert!(sender.send(Ok(prepared(&doc))).is_ok());
        state
            .update(&mut doc, &egui::Context::default(), true)
            .unwrap();
        assert!(!doc.dirty());
        assert_eq!(doc.saved.as_ref(), Some(&doc.model));
    }
    #[test]
    fn failed_worker_does_not_loop_or_clear_stale_copper() {
        let mut doc = doc();
        let before = doc.model.clone();
        let (mut state, sender) = waiting(0);
        assert!(sender.send(Err("Malformed polygon".into())).is_ok());
        let ctx = egui::Context::default();
        assert!(
            state
                .update(&mut doc, &ctx, true)
                .unwrap_err()
                .contains("Malformed polygon")
        );
        for _ in 0..20 {
            state.update(&mut doc, &ctx, true).unwrap();
        }
        assert!(state.job.is_none());
        assert_eq!(doc.model, before);
        assert!(!doc.view.pcb.repour_pending);
    }
}
