//! One graphics worker per comparison view, using immutable shared revisions.
use super::*;
use std::{collections::HashMap, sync::mpsc, time::Duration};

pub(super) struct Sources {
    pub before: Option<Arc<Document>>,
    pub after: Option<Arc<Document>>,
    pub diff: Arc<Diff>,
}
struct Completed {
    cache: Cache,
    textures: Vec<(egui::TextureId, egui::epaint::ImageDelta)>,
}
struct Job {
    key: u64,
    receiver: mpsc::Receiver<Completed>,
}
#[derive(Default)]
pub(super) struct State {
    job: Option<Job>,
    failed: Option<u64>,
}
pub(super) fn failed(cache: &Cache, key: u64) -> bool {
    cache.preparation.lock().unwrap().failed == Some(key)
}
pub(super) fn pending(cache: &Cache) -> bool {
    cache.preparation.lock().unwrap().job.is_some()
}
pub(super) fn has_failed(cache: &Cache) -> bool {
    cache.preparation.lock().unwrap().failed.is_some()
}

pub(super) fn poll(cache: &mut Cache, key: u64, ctx: &egui::Context) {
    let state = cache.preparation.clone();
    let completed = {
        let mut state = state.lock().unwrap();
        let Some(job) = &state.job else { return };
        match job.receiver.try_recv() {
            Ok(completed) => {
                let old_key = state.job.take().unwrap().key;
                (old_key == key).then_some(completed)
            }
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(25));
                None
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                state.failed = Some(state.job.take().unwrap().key);
                None
            }
        }
    };
    if let Some(mut completed) = completed {
        if publish_textures(&mut completed, ctx).is_err() {
            state.lock().unwrap().failed = Some(key);
            return;
        }
        completed.cache.preparation = state;
        *cache = completed.cache;
    }
}

fn publish_textures(completed: &mut Completed, ctx: &egui::Context) -> Result<(), ()> {
    let cache = &mut completed.cache;
    let mut remap = HashMap::new();
    for (id, delta) in std::mem::take(&mut completed.textures) {
        // Atlas meshes with native GPU buffers already own their image/bindings.
        // Ordinary image quads and the CPU fallback need UI-owned texture IDs.
        let needed = cache.meshes.iter().enumerate().any(|(slot, mesh)| {
            mesh.texture_id == id && (cache.gpu.is_none() || cache.gpu_slots[slot].is_none())
        });
        if !needed {
            continue;
        }
        if delta.pos.is_some() {
            return Err(());
        }
        let texture = ctx.load_texture("git-review-image", delta.image, delta.options);
        remap.insert(id, texture.id());
        cache.textures.push(texture);
    }
    for (slot, mesh) in cache.meshes.iter_mut().enumerate() {
        if cache.gpu.is_some() && cache.gpu_slots[slot].is_some() {
            continue;
        }
        let texture = remap.get(&mesh.texture_id).ok_or(())?;
        Arc::make_mut(mesh).texture_id = *texture;
    }
    cache.atlas = None;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn start(
    cache: &mut Cache,
    sources: Sources,
    view: &View,
    rect: Rect,
    visible: [bool; 4],
    key: u64,
    ctx: &egui::Context,
) {
    let mut state = cache.preparation.lock().unwrap();
    if state.failed == Some(key) {
        return;
    }
    ctx.request_repaint_after(Duration::from_millis(25));
    if state.job.is_some() {
        return;
    }
    let worker = egui::Context::default();
    worker.set_fonts(ctx.fonts(|f| f.lock().fonts.definitions().clone()));
    worker.set_style((*ctx.style()).clone());
    worker.options_mut(|o| *o = ctx.options(|o| o.clone()));
    worker.set_pixels_per_point(ctx.pixels_per_point());
    crate::pcb::gpu::copy_configuration(ctx, &worker);
    let mut view = view.clone();
    view.review_cache = Cache::default();
    let builds = cache.builds;
    let (sender, receiver) = mpsc::sync_channel(1);
    state.failed = None;
    state.job = Some(Job { key, receiver });
    let repaint = ctx.clone();
    std::thread::spawn(move || {
        let Some(doc) = sources.after.as_deref().or(sources.before.as_deref()) else {
            return;
        };
        let mut cache = None;
        let output = worker.run(egui::RawInput::default(), |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            cache = Some(build_cache(
                &painter,
                sources.before.as_deref(),
                sources.after.as_deref(),
                &sources.diff,
                doc,
                &mut view,
                rect,
                visible,
                key,
                builds,
            ));
        });
        let cache = cache.unwrap();
        if let Some(gpu) = &cache.gpu {
            gpu.prepare_background(&worker);
        }
        let _ = sender.send(Completed {
            cache,
            textures: output.textures_delta.set,
        });
        repaint.request_repaint();
    });
}

pub(super) fn status(ui: &mut egui::Ui, rect: Rect, cache: &Cache) {
    let failed = cache.preparation.lock().unwrap().failed.is_some();
    let status = Rect::from_center_size(rect.center(), Vec2::new(340_f32.min(rect.width()), 60.));
    if failed {
        let frame = egui::Frame::window(ui.style());
        let inner = frame.total_margin().shrink_rect(status);
        ui.painter().add(frame.paint(inner));
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        content.set_clip_rect(inner.intersect(ui.clip_rect()));
        content.add(egui::Label::new("Could not prepare comparison graphics.").truncate());
        if content.button("Retry").clicked() {
            cache.preparation.lock().unwrap().failed = None;
            content.ctx().request_repaint();
        }
    } else {
        ui.put(
            Rect::from_center_size(status.center(), Vec2::new(status.width(), 24.)),
            egui::ProgressBar::new(0.)
                .animate(true)
                .text("Preparing comparison graphics…"),
        );
    }
}

#[cfg(test)]
#[path = "document_diff_prepare_tests.rs"]
mod tests;
