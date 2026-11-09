//! One bounded background calculation per view; coalesce edits while it runs.
use eda_core::{pcb::Board, pcb_connectivity::Connection};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use uuid::Uuid;

struct Job {
    revision: u64,
    preview: Option<u64>,
    receiver: Mutex<mpsc::Receiver<Option<Vec<Connection>>>>,
    cancelled: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}
#[derive(Clone, Default)]
pub(super) struct Cache {
    revision: Option<u64>,
    preview: Option<u64>,
    pub lines: Arc<Vec<Connection>>,
    job: Option<Arc<Job>>,
    source: Option<(u64, Arc<eda_core::pcb_connectivity::Snapshot>)>,
}
impl Cache {
    pub fn update(
        &mut self,
        board: &Board,
        preview: Option<&Arc<Board>>,
        changed: Option<&BTreeSet<Uuid>>,
        revision: u64,
        preview_generation: u64,
        ctx: &egui::Context,
    ) {
        let preview_key = preview.map(|_| preview_generation);
        if let Some(job) = &self.job {
            if job.revision != revision || job.preview != preview_key {
                job.cancelled.store(true, Ordering::Relaxed);
            }
            let received = job.receiver.lock().unwrap().try_recv();
            match received {
                Ok(lines) => {
                    if job.revision == revision
                        && job.preview == preview_key
                        && let Some(lines) = lines
                    {
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
            if self.source.as_ref().is_none_or(|(old, _)| *old != revision) {
                self.source = Some((
                    revision,
                    Arc::new(eda_core::pcb_connectivity::Snapshot::new(board)),
                ));
            }
            let source = self.source.as_ref().unwrap().1.clone();
            let delta = match (preview, changed) {
                (Some(board), Some(changed)) => Some(source.preview(board, changed)),
                _ => None,
            };
            let full = match (preview, &delta) {
                (Some(board), None) => Arc::new(eda_core::pcb_connectivity::Snapshot::new(board)),
                _ => source,
            };
            let (sender, receiver) = mpsc::sync_channel(1);
            let ctx = ctx.clone();
            let cancelled = Arc::new(AtomicBool::new(false));
            self.job = Some(Arc::new(Job {
                revision,
                preview: preview_key,
                receiver: Mutex::new(receiver),
                cancelled: cancelled.clone(),
            }));
            std::thread::spawn(move || {
                let stop = || cancelled.load(Ordering::Relaxed);
                let lines = delta.map_or_else(
                    || full.connections_cancellable(&stop),
                    |snapshot| snapshot.connections_cancellable(&stop),
                );
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
    #[test]
    fn background_job_does_not_retain_the_editors_preview_buffer() {
        let preview = Arc::new(Board::default());
        let mut cache = Cache::default();
        cache.update(
            &Board::default(),
            Some(&preview),
            Some(&BTreeSet::new()),
            0,
            1,
            &egui::Context::default(),
        );
        assert_eq!(Arc::strong_count(&preview), 1);
    }
    fn completed(revision: u64) -> Cache {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(Some(vec![Connection {
                net: 0,
                start: Point::default(),
                end: Point::mm(1, 0),
            }]))
            .unwrap();
        Cache {
            job: Some(Arc::new(Job {
                revision,
                preview: None,
                receiver: Mutex::new(receiver),
                cancelled: Arc::new(AtomicBool::new(false)),
            })),
            ..Default::default()
        }
    }
    #[test]
    fn completed_result_is_reused_until_document_revision_changes() {
        let mut cache = completed(4);
        let context = egui::Context::default();
        cache.update(&Board::default(), None, None, 4, 0, &context);
        assert_eq!(cache.lines.len(), 1);
        let lines = cache.lines.clone();
        for _ in 0..100 {
            cache.update(&Board::default(), None, None, 4, 0, &context);
        }
        assert!(Arc::ptr_eq(&lines, &cache.lines));
        assert!(!cache.calculating());
    }
    #[test]
    fn stale_job_cannot_replace_current_connections() {
        let mut cache = completed(4);
        let cancelled = cache.job.as_ref().unwrap().cancelled.clone();
        cache.update(
            &Board::default(),
            None,
            None,
            5,
            0,
            &egui::Context::default(),
        );
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(cache.lines.is_empty());
        assert_eq!(cache.job.as_ref().unwrap().revision, 5);
    }
    #[test]
    fn changing_preview_generation_discards_result_without_comparing_boards() {
        let mut cache = completed(4);
        Arc::get_mut(cache.job.as_mut().unwrap()).unwrap().preview = Some(10);
        cache.update(
            &Board::default(),
            Some(&Arc::new(Board::default())),
            Some(&BTreeSet::new()),
            4,
            11,
            &egui::Context::default(),
        );
        assert!(cache.lines.is_empty());
        assert_eq!(cache.job.as_ref().unwrap().preview, Some(11));
    }
}
