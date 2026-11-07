//! A single import worker. UI requests are queued, duplicate paths coalesced;
//! cancellation discards results and keeps the worker slot until it has stopped.
use super::*;
use std::{
    collections::VecDeque,
    sync::mpsc,
    time::{Duration, Instant},
};

struct Job {
    path: PathBuf,
    control: eda_io::altium_pcb::ImportControl,
    receiver: mpsc::Receiver<Result<Document, String>>,
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
        self.job = Some(Job {
            path,
            control,
            receiver,
            started: Instant::now(),
            cancelled: false,
        });
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                if worker_path
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("PcbDoc"))
                {
                    eda_io::altium_pcb::load_pcb_controlled(&worker_path, &worker_control)
                } else {
                    eda_io::load(&worker_path)
                }
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
                            self.complete_open_path(job.path, result);
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
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(job.path.file_name().unwrap_or_default().to_string_lossy());
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(if job.cancelled {
                        "Cancelling…".into()
                    } else {
                        job.control.phase()
                    });
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
    fn cancelled_import_keeps_worker_slot_and_does_not_duplicate_queued_paths() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut state = State {
            job: Some(Job {
                path: "one.PcbDoc".into(),
                control: Default::default(),
                receiver,
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
        sender.send(Ok(Document::example())).unwrap();
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
