//! A single import worker. UI requests are queued, duplicate paths coalesced;
//! cancellation discards results and keeps the worker slot until it has stopped.
use super::*;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) struct PreparedDocument {
    pub model: Document,
    pub saved: Document,
}
impl PreparedDocument {
    pub fn new(mut model: Document) -> Self {
        eda_core::pcb::Board::ensure_document(&mut model);
        let saved = model.clone();
        Self { model, saved }
    }
}

struct Job {
    path: PathBuf,
    control: eda_io::altium_pcb::ImportControl,
    receiver: mpsc::Receiver<Result<PreparedDocument, String>>,
    preparing: Arc<AtomicBool>,
    started: Instant,
    cancelled: bool,
}
#[derive(Default)]
pub(super) struct State {
    job: Option<Job>,
    queued: VecDeque<PathBuf>,
}
impl State {
    fn enqueue(&mut self, path: PathBuf) -> Result<(), String> {
        if self
            .job
            .as_ref()
            .is_some_and(|j| j.path == path && !j.cancelled)
            || self.queued.contains(&path)
        {
            return Ok(());
        }
        if self.queued.len() >= 8 {
            return Err("The document opening queue is full; wait for the current import".into());
        }
        self.queued.push_back(path);
        Ok(())
    }
    pub fn busy(&self) -> bool {
        self.job.is_some() || !self.queued.is_empty()
    }
    fn cancel(&mut self) {
        self.queued.clear();
        if let Some(job) = &mut self.job {
            job.cancelled = true;
            job.control.cancel();
        }
    }
    fn start(&mut self, ctx: &egui::Context) {
        if self.job.is_some() {
            return;
        }
        let Some(path) = self.queued.pop_front() else {
            return;
        };
        let control = eda_io::altium_pcb::ImportControl::default();
        let worker_control = control.clone();
        let worker_path = path.clone();
        let worker_ctx = ctx.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let preparing = Arc::new(AtomicBool::new(false));
        let worker_preparing = preparing.clone();
        self.job = Some(Job {
            path,
            control,
            receiver,
            preparing,
            started: Instant::now(),
            cancelled: false,
        });
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let model = if worker_path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("PcbDoc"))
                {
                    eda_io::altium_pcb::load_pcb_controlled(&worker_path, &worker_control)
                } else {
                    eda_io::load(&worker_path)
                }?;
                worker_preparing.store(true, Ordering::Relaxed);
                worker_ctx.request_repaint();
                Ok(PreparedDocument::new(model))
            })
            .unwrap_or_else(|_| Err("Document import worker failed".into()));
            let _ = sender.send(result);
            worker_ctx.request_repaint();
        });
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl PcbCraft {
    pub(super) fn request_open_path(&mut self, path: PathBuf) {
        if let Some(manager) = &mut self.layer_stack {
            manager.visible = false;
        }
        let path = path.canonicalize().unwrap_or(path);
        if let Some(index) = self
            .documents
            .iter()
            .position(|d| d.path.as_ref() == Some(&path) || d.source_path.as_ref() == Some(&path))
        {
            self.active = Some(index);
        } else if path.extension().is_some_and(|s| {
            ["PrjPcb", "DsnWrk", "SchLib", "PcbLib", "IntLib"]
                .iter()
                .any(|ext| s.eq_ignore_ascii_case(ext))
        }) {
            self.open_path(path);
        } else if let Err(error) = self.document_loader.enqueue(path) {
            self.message(true, error);
        }
    }
    pub(super) fn poll_document_loader(&mut self, ctx: &egui::Context) {
        // Keep an ECO/annotation/dialog's active document stable until it closes.
        if self.design_sync.is_none()
            && self.library_update.is_none()
            && self.annotation.is_none()
            && self.pending.is_none()
            && !self.layer_stack_active()
        {
            let received = self
                .document_loader
                .job
                .as_ref()
                .map(|job| job.receiver.try_recv());
            if let Some(result) = received {
                match result {
                    Ok(result) => {
                        let job = self.document_loader.job.take().unwrap();
                        if !job.cancelled {
                            self.complete_prepared_open_path(job.path, result);
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        let job = self.document_loader.job.take().unwrap();
                        if !job.cancelled {
                            self.message(
                                true,
                                format!(
                                    "Could not open {}: import worker stopped",
                                    job.path.display()
                                ),
                            );
                        }
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
        }
        self.document_loader.start(ctx);
        if self.document_loader.busy() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }
    pub(super) fn document_loading_dialog(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.document_loader.job else {
            return;
        };
        let mut cancel = false;
        egui::Window::new("Opening document")
            .id(egui::Id::new("document_import_progress"))
            .collapsible(false)
            .resizable(false)
            .default_width(420.)
            .min_width(420.)
            .max_width(420.)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show_stable(ctx, |ui| {
                ui.add(
                    egui::Label::new(job.path.file_name().unwrap_or_default().to_string_lossy())
                        .truncate(),
                )
                .on_hover_text(job.path.display().to_string());
                ui.horizontal(|ui| {
                    ui.spinner();
                    let phase = if job.cancelled {
                        "Cancelling…".into()
                    } else if job.preparing.load(Ordering::Relaxed) {
                        "Preparing document".into()
                    } else if job
                        .path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
                    {
                        "Reading schematic".into()
                    } else {
                        job.control.phase()
                    };
                    ui.add(egui::Label::new(&phase).truncate())
                        .on_hover_text(phase);
                });
                ui.label(format!("{:.1} s", job.started.elapsed().as_secs_f64()));
                cancel = ui
                    .add_enabled(!job.cancelled, egui::Button::new("Cancel"))
                    .clicked();
            });
        if cancel {
            self.document_loader.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opening_dialog_keeps_its_rectangle_for_long_names_phases_and_elapsed_time() {
        for dpi in [1., 1.5, 2.] {
            let mut app = crate::app::schematic_editor::tests::app();
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            ctx.set_pixels_per_point(dpi);
            let (_sender, receiver) = mpsc::sync_channel(1);
            app.document_loader.job = Some(Job {
                path: "Board.PcbDoc".into(),
                control: Default::default(),
                receiver,
                preparing: Arc::new(AtomicBool::new(false)),
                started: Instant::now(),
                cancelled: false,
            });
            let mut before: Option<egui::Rect> = None;
            for (name, preparing, cancelled, seconds) in [
                ("Board.PcbDoc".into(), false, false, 0),
                (
                    format!("{}.PcbDoc", "VeryLongImportedBoardName".repeat(30)).into(),
                    false,
                    false,
                    10,
                ),
                ("Board.PcbDoc".into(), true, false, 100),
                ("Board.PcbDoc".into(), true, true, 10000),
            ] {
                let job = app.document_loader.job.as_mut().unwrap();
                job.path = name;
                job.preparing.store(preparing, Ordering::Relaxed);
                job.cancelled = cancelled;
                job.started = Instant::now()
                    .checked_sub(Duration::from_secs(seconds))
                    .unwrap_or(job.started);
                for _ in 0..3 {
                    let _ = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(1440., 900.),
                            )),
                            ..Default::default()
                        },
                        |ctx| app.document_loading_dialog(ctx),
                    );
                }
                let rect = ctx
                    .memory(|m| m.area_rect(egui::Id::new("document_import_progress")))
                    .unwrap();
                if let Some(before) = before {
                    assert!((rect.min - before.min).length() < 1.);
                    assert!((rect.size() - before.size()).length() < 1.);
                } else {
                    before = Some(rect);
                }
            }
        }
    }
    #[test]
    fn imported_snapshot_is_moved_into_the_ui_and_starts_clean() {
        let mut app = crate::app::schematic_editor::tests::app();
        let prepared =
            PreparedDocument::new(Document::new(DocumentKind::Schematic, "Loaded.SchDoc"));
        let model_name = prepared.model.name.as_ptr();
        let saved_name = prepared.saved.name.as_ptr();
        let path = PathBuf::from("Loaded.SchDoc");
        app.complete_prepared_open_path(path.clone(), Ok(prepared));
        let opened = &app.documents[app.active.unwrap()];
        assert_eq!(opened.model.name.as_ptr(), model_name);
        assert_eq!(opened.saved.as_ref().unwrap().name.as_ptr(), saved_name);
        assert_eq!(opened.source_path.as_ref(), Some(&path));
        assert!(!opened.dirty());
        assert!(opened.view.sch_opening);
    }

    #[test]
    fn project_first_schematic_is_queued_and_worker_prepares_its_saved_snapshot() {
        let folder = tempfile::tempdir().unwrap();
        let sheet = folder.path().join("Main.SchDoc");
        std::fs::write(
            &sheet,
            include_bytes!("../../../fixtures/schematic/minimal.SchDoc"),
        )
        .unwrap();
        let project = folder.path().join("Main.PrjPcb");
        std::fs::write(
            &project,
            "[Design]\n[Document1]\nDocumentPath=Main.SchDoc\n",
        )
        .unwrap();
        let mut app = crate::app::schematic_editor::tests::app();
        let count = app.documents.len();
        app.request_open_path(project.clone());
        app.request_open_path(project);
        assert_eq!(
            app.documents.len(),
            count,
            "Project opening must not import its sheet on the UI thread"
        );
        assert_eq!(app.document_loader.queued.len(), 1);
        app.document_loader.start(&egui::Context::default());
        let job = app.document_loader.job.as_ref().unwrap();
        let prepared = job
            .receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(job.preparing.load(Ordering::Relaxed));
        assert_eq!(prepared.model, prepared.saved);
        assert!(prepared.model.schematic.is_some());
    }

    #[test]
    fn cancelled_import_keeps_worker_slot_and_does_not_duplicate_queued_paths() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut state = State {
            job: Some(Job {
                path: "one.PcbDoc".into(),
                control: Default::default(),
                receiver,
                preparing: Arc::new(AtomicBool::new(false)),
                started: Instant::now(),
                cancelled: false,
            }),
            queued: Default::default(),
        };
        state.enqueue("one.PcbDoc".into()).unwrap();
        assert!(state.queued.is_empty());
        state.cancel();
        assert!(
            eda_io::altium_pcb::load_pcb_controlled(
                std::path::Path::new("unread.PcbDoc"),
                &state.job.as_ref().unwrap().control
            )
            .unwrap_err()
            .contains("cancelled")
        );
        state.enqueue("two.PcbDoc".into()).unwrap();
        state.enqueue("two.PcbDoc".into()).unwrap();
        state.start(&egui::Context::default());
        assert_eq!(state.queued.len(), 1);
        assert_eq!(
            state.job.as_ref().unwrap().path,
            PathBuf::from("one.PcbDoc")
        );
        sender
            .send(Ok(PreparedDocument::new(Document::example())))
            .unwrap();
        assert!(state.job.as_ref().unwrap().cancelled);
        state.job.take();
        state.cancel();
        assert!(!state.busy());
    }
    #[test]
    fn queue_is_bounded_and_worker_reports_open_errors() {
        let mut state = State::default();
        for n in 0..8 {
            state.enqueue(format!("missing-{n}.PcbDoc").into()).unwrap();
        }
        assert!(state.enqueue("ninth.PcbDoc".into()).is_err());
        state.start(&egui::Context::default());
        let result = state
            .job
            .as_ref()
            .unwrap()
            .receiver
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        assert!(result.is_err());
        assert_eq!(state.queued.len(), 7);
    }
}
