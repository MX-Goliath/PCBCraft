//! One bounded background calculation per view; coalesce edits while it runs.
use eda_core::{pcb::Board, pcb_connectivity::Connection};
use std::sync::{Arc, Mutex, mpsc};

struct Job {
    revision: u64,
    preview: Option<u64>,
    receiver: Mutex<mpsc::Receiver<Vec<Connection>>>,
}
#[derive(Clone, Default)]
pub(super) struct Cache {
    revision: Option<u64>,
    preview: Option<u64>,
    pub lines: Arc<Vec<Connection>>,
    job: Option<Arc<Job>>,
}
impl Cache {
    pub fn update(
        &mut self,
        board: &Board,
        preview: Option<&Board>,
        revision: u64,
        preview_generation: u64,
        ctx: &egui::Context,
    ) {
        let scene = preview.unwrap_or(board);
        let preview_key = preview.map(|_| preview_generation);
        if let Some(job) = &self.job {
            let received = job.receiver.lock().unwrap().try_recv();
            match received {
                Ok(lines) => {
                    if job.revision == revision && job.preview == preview_key {
                        self.lines = Arc::new(lines);
                        self.revision = Some(revision);
                        self.preview = preview_key;
                    }
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.job.is_none() && (self.revision != Some(revision) || self.preview != preview_key) {
            let snapshot = Arc::new(scene.clone());
            let (sender, receiver) = mpsc::sync_channel(1);
            let worker_board = snapshot.clone();
            let ctx = ctx.clone();
            self.job = Some(Arc::new(Job {
                revision,
                preview: preview_key,
                receiver: Mutex::new(receiver),
            }));
            std::thread::spawn(move || {
                let lines = eda_core::pcb_connectivity::connections(&worker_board);
                let _ = sender.send(lines);
                ctx.request_repaint();
            });
        }
    }
    pub fn calculating(&self) -> bool {
        self.job.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_geom::Point;
    fn completed(revision: u64) -> Cache {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(vec![Connection {
                net: 0,
                start: Point::default(),
                end: Point::mm(1, 0),
            }])
            .unwrap();
        Cache {
            job: Some(Arc::new(Job {
                revision,
                preview: None,
                receiver: Mutex::new(receiver),
            })),
            ..Default::default()
        }
    }
    #[test]
    fn completed_result_is_reused_until_document_revision_changes() {
        let mut cache = completed(4);
        let context = egui::Context::default();
        cache.update(&Board::default(), None, 4, 0, &context);
        assert_eq!(cache.lines.len(), 1);
        let lines = cache.lines.clone();
        for _ in 0..100 {
            cache.update(&Board::default(), None, 4, 0, &context);
        }
        assert!(Arc::ptr_eq(&lines, &cache.lines));
        assert!(!cache.calculating());
    }
    #[test]
    fn stale_job_cannot_replace_current_connections() {
        let mut cache = completed(4);
        cache.update(&Board::default(), None, 5, 0, &egui::Context::default());
        assert!(cache.lines.is_empty());
        assert_eq!(cache.job.as_ref().unwrap().revision, 5);
    }
    #[test]
    fn changing_preview_generation_discards_result_without_comparing_boards() {
        let mut cache = completed(4);
        Arc::get_mut(cache.job.as_mut().unwrap()).unwrap().preview = Some(10);
        cache.update(
            &Board::default(),
            Some(&Board::default()),
            4,
            11,
            &egui::Context::default(),
        );
        assert!(cache.lines.is_empty());
        assert_eq!(cache.job.as_ref().unwrap().preview, Some(11));
    }
}
