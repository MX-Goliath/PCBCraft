//! Reuse the complete tessellated PCB artwork across camera translations and zoom gestures.
//! Screen-anchored hatching stays in its original draw order and is drawn live.
use super::*;
use egui::{
    LayerId, TextureHandle,
    epaint::{ClippedPrimitive, Mesh, Primitive},
};
use std::{
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

const ZOOM_SETTLE: Duration = Duration::from_millis(120);

// Match stationary artwork's document anchor when egui rounds stroke/glyph
// coordinates. Tessellating at the moving viewport can shift a line by a whole
// pixel (rounding half pixels on opposite sides of zero differently).
pub(super) fn paint_live(
    painter: &egui::Painter,
    view: &mut View,
    rect: Rect,
    draw: impl FnOnce(&egui::Painter, &Camera, &mut View, Rect),
) {
    let anchor = Bounds::points(&view.pcb.geometry.outline).centre();
    let camera = Camera {
        centre_mm: [anchor.x as f64 / 1e6, anchor.y as f64 / 1e6],
        ..view.camera.clone()
    };
    let fixed_rect = Rect::from_center_size(Pos2::ZERO, Vec2::splat(2.));
    let shift = view.camera.screen(anchor, rect).to_vec2();
    let layer = painter.layer_id();
    let saved = painter
        .ctx()
        .graphics_mut(|g| std::mem::take(g.entry(layer)));
    let mut fixed = painter.clone();
    fixed.set_clip_rect(Rect::EVERYTHING);
    draw(&fixed, &camera, view, fixed_rect);
    let shapes = painter.ctx().graphics_mut(|g| {
        let shapes = std::mem::replace(g.entry(layer), saved);
        shapes.all_entries().cloned().collect()
    });
    for primitive in painter
        .ctx()
        .tessellate(shapes, painter.ctx().pixels_per_point())
    {
        if let Primitive::Mesh(mut mesh) = primitive.primitive {
            for vertex in &mut mesh.vertices {
                vertex.pos += shift;
            }
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}

// One owned snapshot and at most one worker per view. The worker has its own
// egui context: running a second frame on the UI context would race its fonts
// and paint lists. Content/display changes replace its render snapshot.
struct Snapshot {
    board: Board,
    view: View,
    ctx: egui::Context,
}
struct ZoomJob {
    key: u64,
    receiver: mpsc::Receiver<(Snapshot, Cache)>,
}
#[derive(Default)]
struct Refinement {
    snapshot: Option<Snapshot>,
    job: Option<ZoomJob>,
    failed: bool,
    target: f64,
    changed: Option<Instant>,
}

#[derive(Clone, Copy)]
pub(super) enum Live {
    Artwork(Color32),
    Mask(u16, MaskKind, bool),
    Details,
}
#[derive(Clone, Copy)]
pub(super) enum Routes {
    Artwork(u16),
    Details(u16),
    Mask(u16, MaskKind, bool),
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    Board,
    Artwork(u16, u8),
    Details(u16, u8),
    Mask(u16),
    Polygons,
}
#[derive(Clone)]
struct GroupCache {
    signature: u64,
    parts: Vec<Part>,
}
#[derive(Clone)]
struct CachedMesh {
    mesh: Arc<Mesh>,
    gpu: Option<(Arc<gpu::Scene>, usize)>,
    _atlas: TextureHandle,
}
#[derive(Clone)]
enum Part {
    Routes(Routes),
    Live(Uuid, Live),
    Connections,
    Mesh(CachedMesh),
    Grid,
    Hatch(Uuid, Color32),
    Tools,
    Origin,
}
enum Capture {
    Start(Group, u64),
    End,
    Reuse(Group, Arc<GroupCache>),
    Routes(Routes),
    Live(Uuid, Live),
    Connections,
    Shapes(egui::layers::PaintList),
    Grid,
    Hatch(Uuid, Color32),
    Tools,
    Origin,
}
pub(super) struct Builder {
    layer: LayerId,
    saved: egui::layers::PaintList,
    captured: Vec<Capture>,
    previous: BTreeMap<Group, Arc<GroupCache>>,
    active: bool,
}
impl Builder {
    pub fn new(painter: &egui::Painter) -> Self {
        let layer = painter.layer_id();
        Self {
            layer,
            saved: painter
                .ctx()
                .graphics_mut(|g| std::mem::take(g.entry(layer))),
            captured: vec![],
            previous: BTreeMap::new(),
            active: false,
        }
    }
    pub fn group(&mut self, painter: &egui::Painter, group: Group, signature: u64) -> bool {
        self.end_group(painter);
        self.flush(painter.ctx());
        if let Some(old) = self.previous.get(&group)
            && old.signature == signature
        {
            self.captured.push(Capture::Reuse(group, old.clone()));
            return false;
        }
        self.captured.push(Capture::Start(group, signature));
        self.active = true;
        true
    }
    pub fn end_group(&mut self, painter: &egui::Painter) {
        if self.active {
            self.flush(painter.ctx());
            self.captured.push(Capture::End);
            self.active = false;
        }
    }
    fn flush(&mut self, ctx: &egui::Context) {
        let shapes = ctx.graphics_mut(|g| std::mem::take(g.entry(self.layer)));
        if !shapes.is_empty() {
            self.captured.push(Capture::Shapes(shapes));
        }
    }
    pub fn live(&mut self, painter: &egui::Painter, id: Uuid, live: Live) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Live(id, live));
    }
    pub fn routes(&mut self, painter: &egui::Painter, pass: Routes) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Routes(pass));
    }
    pub fn connections(&mut self, painter: &egui::Painter) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Connections);
    }
    pub fn hatch(&mut self, painter: &egui::Painter, item: Uuid, color: Color32) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Hatch(item, color));
    }
    pub fn tools(&mut self, painter: &egui::Painter) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Tools);
    }
    pub fn grid(&mut self, painter: &egui::Painter) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Grid);
    }
    pub fn origin(&mut self, painter: &egui::Painter) {
        self.flush(painter.ctx());
        self.captured.push(Capture::Origin);
    }
}

#[derive(Clone)]
pub(super) struct Cache {
    key: Option<u64>,
    origin: Pos2,
    scale: f64,
    image: Option<egui::ColorImage>,
    refinement: Arc<Mutex<Refinement>>,
    parts: Vec<Part>,
    atlas: Option<TextureHandle>,
    gpu: Option<Arc<gpu::Scene>>,
    shared_key: Option<u64>,
    groups: BTreeMap<Group, Arc<GroupCache>>,
    pub builds: usize,
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            key: None,
            origin: Pos2::ZERO,
            scale: 1.,
            image: None,
            refinement: Arc::default(),
            parts: vec![],
            atlas: None,
            gpu: None,
            shared_key: None,
            groups: BTreeMap::new(),
            builds: 0,
        }
    }
}
impl Cache {
    #[cfg(test)]
    pub(super) fn resident_scene(&self) -> Option<Arc<gpu::Scene>> {
        self.gpu.clone()
    }
    pub fn rebuild(&mut self, painter: &egui::Painter, board: &Board, view: &mut View, key: u64) {
        let shared_key = shared_key(board, view, painter.ctx());
        let previous = (self.shared_key == shared_key
            && shared_key.is_some()
            && self.scale == view.camera.pixels_per_mm)
            .then_some(&*self);
        let mut result = Self::build_reusing(painter, board, view, key, self.builds, previous);
        result.refinement = self.refinement.clone();
        if view.pcb.preview.is_none() {
            result.seed(board, view, painter.ctx());
        } else {
            let mut state = result.refinement.lock().unwrap();
            state.snapshot = None;
            state.changed = None;
        }
        *self = result;
    }
    fn build(
        painter: &egui::Painter,
        board: &Board,
        view: &mut View,
        key: u64,
        builds: usize,
    ) -> Self {
        Self::build_reusing(painter, board, view, key, builds, None)
    }
    fn build_reusing(
        painter: &egui::Painter,
        board: &Board,
        view: &mut View,
        key: u64,
        builds: usize,
        previous: Option<&Self>,
    ) -> Self {
        let start = Instant::now();
        let mut scene_painter = painter.clone();
        // Include offscreen objects so navigation never leaves cached artwork.
        scene_painter.set_clip_rect(Rect::EVERYTHING);
        let mut builder = Builder::new(&scene_painter);
        if let Some(previous) = previous {
            builder.previous = previous.groups.clone();
        }
        // Keep pixel rounding independent of the pan at capture time.
        let anchor = Bounds::points(&view.pcb.geometry.outline).centre();
        let camera = Camera {
            centre_mm: [anchor.x as f64 / 1e6, anchor.y as f64 / 1e6],
            ..view.camera.clone()
        };
        let rect = Rect::from_center_size(Pos2::ZERO, Vec2::splat(2.));
        paint_scene(&scene_painter, board, &camera, view, rect, &mut builder);
        let mut result = Self {
            builds,
            ..Default::default()
        };
        result.finish(builder, &scene_painter, key, &camera, rect);
        result.shared_key = shared_key(board, view, painter.ctx());
        if result.gpu.is_none() {
            result.gpu = previous.and_then(|cache| cache.gpu.clone());
        }
        if std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some() {
            let reused = previous.map_or(0, |old| {
                result
                    .groups
                    .iter()
                    .filter(|(id, group)| {
                        old.groups
                            .get(id)
                            .is_some_and(|old| Arc::ptr_eq(old, group))
                    })
                    .count()
            });
            eprintln!(
                "pcb_artwork_build n={} cpu_ms={:.2} zoom={:.6} revision={} preview={} groups={} reused={reused}",
                result.builds,
                start.elapsed().as_secs_f64() * 1000.,
                camera.pixels_per_mm,
                view.pcb.three_d.revision,
                view.pcb.preview_generation,
                result.groups.len(),
            );
        }
        result
    }
    fn seed(&mut self, board: &Board, view: &View, ctx: &egui::Context) {
        let worker = egui::Context::default();
        worker.set_fonts(ctx.fonts(|f| f.lock().fonts.definitions().clone()));
        worker.options_mut(|options| *options = ctx.options(|o| o.clone()));
        worker.set_pixels_per_point(ctx.pixels_per_point());
        gpu::copy_configuration(ctx, &worker);
        // paint_cache is taken out of the live view before this snapshot.
        let mut view = view.clone();
        view.pcb.paint_cache = Self::default();
        view.pcb.preview = None;
        let mut state = self.refinement.lock().unwrap();
        state.snapshot = Some(Snapshot {
            board: board.clone(),
            view,
            ctx: worker,
        });
        state.target = self.scale;
        state.changed = None;
        state.failed = false;
        // Keep an older running job until it exits. Its key rejects its result,
        // and prevents a second worker from being launched in the meantime.
    }
    pub fn refresh(&mut self, scale: f64, ctx: &egui::Context) {
        let shared = self.refinement.clone();
        let mut state = shared.lock().unwrap();
        if state.target != scale {
            state.target = scale;
            state.changed = Some(Instant::now());
        }
        if let Some(job) = &state.job {
            match job.receiver.try_recv() {
                Ok((snapshot, mut result)) => {
                    let current = self.key == Some(job.key);
                    if current {
                        state.snapshot = Some(snapshot);
                    }
                    state.job = None;
                    if current && result.scale == scale {
                        // Worker texture IDs belong to its own context. GPU
                        // callbacks own the atlas; the CPU fallback needs a UI
                        // texture and remapped IDs before publishing the result.
                        result.atlas = None;
                        if result.gpu.is_none() {
                            let atlas = ctx.load_texture(
                                "pcb-2d-artwork",
                                result.image.as_ref().unwrap().clone(),
                                egui::TextureOptions::LINEAR,
                            );
                            let mut remapped = HashMap::new();
                            let mut remap = |part: &mut Part| {
                                if let Part::Mesh(cached) = part {
                                    let id = Arc::as_ptr(&cached.mesh) as usize;
                                    cached.mesh = remapped
                                        .entry(id)
                                        .or_insert_with(|| {
                                            let mut mesh = (*cached.mesh).clone();
                                            mesh.texture_id = atlas.id();
                                            Arc::new(mesh)
                                        })
                                        .clone();
                                    cached._atlas = atlas.clone();
                                }
                            };
                            for part in &mut result.parts {
                                remap(part);
                            }
                            for group in result.groups.values_mut() {
                                for part in &mut Arc::make_mut(group).parts {
                                    remap(part);
                                }
                            }
                            result.atlas = Some(atlas);
                        }
                        result.builds = self.builds + 1;
                        result.refinement = shared.clone();
                        *self = result;
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    let current = self.key == Some(job.key);
                    state.job = None;
                    state.failed = current;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.scale == scale || state.failed {
            return;
        }
        let remaining =
            ZOOM_SETTLE.saturating_sub(state.changed.map_or(ZOOM_SETTLE, |t| t.elapsed()));
        if !remaining.is_zero() {
            ctx.request_repaint_after(remaining);
            return;
        }
        if state.job.is_none()
            && let Some(mut snapshot) = state.snapshot.take()
        {
            let (sender, receiver) = mpsc::sync_channel(1);
            let key = self.key.unwrap();
            state.job = Some(ZoomJob { key, receiver });
            let repaint = ctx.clone();
            std::thread::spawn(move || {
                snapshot.view.camera.pixels_per_mm = scale;
                let mut result = None;
                let _ = snapshot.ctx.run(egui::RawInput::default(), |ctx| {
                    let painter = ctx.layer_painter(egui::LayerId::background());
                    result = Some(Self::build(
                        &painter,
                        &snapshot.board,
                        &mut snapshot.view,
                        key,
                        0,
                    ));
                });
                let result = result.unwrap();
                if let Some(gpu) = &result.gpu {
                    gpu.prepare_background(&snapshot.ctx);
                }
                let _ = sender.send((snapshot, result));
                repaint.request_repaint();
            });
        }
    }
    pub fn valid(&self, key: u64) -> bool {
        self.key == Some(key)
    }
    pub fn finish(
        &mut self,
        mut builder: Builder,
        painter: &egui::Painter,
        key: u64,
        camera: &Camera,
        rect: Rect,
    ) {
        let ctx = painter.ctx();
        builder.end_group(painter);
        builder.flush(ctx);
        ctx.graphics_mut(|g| *g.entry(builder.layer) = builder.saved);
        self.parts.clear();
        // Use a private atlas snapshot: ordinary UI text may grow/reset egui's
        // atlas after this scene was cached. Its UVs must not invalidate artwork.
        let image = ctx.fonts(|f| f.image());
        if let Some(atlas) = &mut self.atlas {
            atlas.set(image.clone(), egui::TextureOptions::LINEAR);
        } else {
            self.atlas = Some(ctx.load_texture(
                "pcb-2d-artwork",
                image.clone(),
                egui::TextureOptions::LINEAR,
            ));
        }
        let texture = self.atlas.as_ref().unwrap().id();
        let mut new_meshes = vec![];
        let mut ranges = vec![];
        let mut group_start = None;
        for captured in builder.captured {
            match captured {
                Capture::Start(group, signature) => {
                    group_start = Some((group, signature, self.parts.len()));
                }
                Capture::End => {
                    let (group, signature, start) = group_start.take().unwrap();
                    ranges.push((group, signature, start..self.parts.len()));
                }
                Capture::Reuse(group, cached) => {
                    self.parts.extend(cached.parts.iter().cloned());
                    self.groups.insert(group, cached);
                }
                Capture::Routes(pass) => self.parts.push(Part::Routes(pass)),
                Capture::Live(id, live) => self.parts.push(Part::Live(id, live)),
                Capture::Connections => self.parts.push(Part::Connections),
                Capture::Grid => self.parts.push(Part::Grid),
                Capture::Hatch(item, color) => self.parts.push(Part::Hatch(item, color)),
                Capture::Tools => self.parts.push(Part::Tools),
                Capture::Origin => self.parts.push(Part::Origin),
                Capture::Shapes(shapes) => {
                    let shapes = shapes.all_entries().cloned().collect();
                    for ClippedPrimitive { primitive, .. } in
                        ctx.tessellate(shapes, ctx.pixels_per_point())
                    {
                        if let Primitive::Mesh(mut mesh) = primitive {
                            debug_assert_eq!(mesh.texture_id, egui::TextureId::default());
                            mesh.texture_id = texture;
                            new_meshes.push(self.parts.len());
                            self.parts.push(Part::Mesh(CachedMesh {
                                mesh: Arc::new(mesh),
                                gpu: None,
                                _atlas: self.atlas.as_ref().unwrap().clone(),
                            }));
                        }
                    }
                }
            }
        }
        self.gpu = gpu::format(ctx)
            .filter(|_| !new_meshes.is_empty())
            .map(|format| {
                gpu::Scene::new(
                    new_meshes
                        .iter()
                        .filter_map(|index| match &self.parts[*index] {
                            Part::Mesh(mesh) => Some(mesh.mesh.clone()),
                            _ => None,
                        })
                        .collect(),
                    image.clone(),
                    format,
                )
            });
        if let Some(gpu) = &self.gpu {
            for (slot, index) in new_meshes.into_iter().enumerate() {
                if let Part::Mesh(mesh) = &mut self.parts[index] {
                    mesh.gpu = Some((gpu.clone(), slot));
                }
            }
        }
        for (group, signature, range) in ranges {
            self.groups.insert(
                group,
                Arc::new(GroupCache {
                    signature,
                    parts: self.parts[range].to_vec(),
                }),
            );
        }
        self.image = Some(image);
        self.scale = camera.pixels_per_mm;
        self.origin = camera.screen(Point::default(), rect);
        self.key = Some(key);
        self.builds += 1;
    }
    pub fn paint(&self, painter: &egui::Painter, board: &Board, view: &mut View, rect: Rect) {
        let scale = (view.camera.pixels_per_mm / self.scale) as f32;
        let delta =
            view.camera.screen(Point::default(), rect).to_vec2() - self.origin.to_vec2() * scale;
        let visible = Bounds::points(&[
            world(&view.camera, rect.expand(2.).left_bottom(), rect),
            world(&view.camera, rect.expand(2.).right_top(), rect),
        ]);
        for part in &self.parts {
            match part {
                Part::Routes(pass) => paint_route_items(painter, board, view, rect, *pass),
                Part::Connections => paint_connections(painter, board, &view.camera, view, rect),
                Part::Live(id, live) => {
                    let item = &board.items[view.pcb.geometry.indices[id]];
                    if let Live::Artwork(color) = *live
                        && matches!(item.shape, Shape::Region { .. })
                        && (matches!(item.shape, Shape::Region { cutout: true, .. })
                            || item.keepout
                            || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION"))
                        && !view.pcb.display.draft[display::Settings::kind(item)]
                    {
                        paint_item(
                            painter,
                            &view.camera,
                            rect,
                            item,
                            view.pcb.active_layer,
                            color,
                            &mut view.pcb,
                        );
                        continue;
                    }
                    paint_live(
                        painter,
                        view,
                        rect,
                        |painter, camera, view, rect| match *live {
                            Live::Artwork(color) => {
                                if !(view.pcb.display.draft[display::Settings::kind(item)]
                                    && paint_draft(
                                        painter,
                                        camera,
                                        rect,
                                        item,
                                        view.pcb.active_layer,
                                        color,
                                    ))
                                {
                                    paint_item(
                                        painter,
                                        camera,
                                        rect,
                                        item,
                                        view.pcb.active_layer,
                                        color,
                                        &mut view.pcb,
                                    );
                                }
                            }
                            Live::Mask(layer, kind, bottom) => paint_mask(
                                painter,
                                board,
                                camera,
                                view,
                                rect,
                                item,
                                (layer, kind, bottom),
                            ),
                            Live::Details => {
                                paint_details(painter, board, camera, view, rect, item)
                            }
                        },
                    );
                }
                Part::Mesh(cached) => {
                    if let Some((gpu, mesh_index)) = &cached.gpu {
                        painter.add(gpu.callback(
                            *mesh_index,
                            rect,
                            delta,
                            scale,
                            super::gpu::artwork_dither(painter.ctx()),
                        ));
                        continue;
                    }
                    let mesh = &cached.mesh;
                    let mesh = if delta == Vec2::ZERO && scale == 1. {
                        mesh.clone()
                    } else {
                        let mut moved = (**mesh).clone();
                        for vertex in &mut moved.vertices {
                            vertex.pos = (vertex.pos.to_vec2() * scale + delta).to_pos2();
                        }
                        Arc::new(moved)
                    };
                    painter.add(egui::Shape::Mesh(mesh));
                }
                Part::Hatch(id, color) => {
                    if !view.pcb.geometry.bounds[id].intersects(visible) {
                        continue;
                    }
                    let item = &board.items[view.pcb.geometry.indices[id]];
                    paint_item(
                        painter,
                        &view.camera,
                        rect,
                        item,
                        view.pcb.active_layer,
                        *color,
                        &mut view.pcb,
                    );
                }
                Part::Tools => paint_tools(painter, board, view, rect),
                Part::Grid => paint_grid(painter, view, rect),
                Part::Origin => paint_origin(painter, board, view, rect),
            }
        }
    }
}

fn common_hash(view: &View, ctx: &egui::Context) -> std::collections::hash_map::DefaultHasher {
    let pcb = &view.pcb;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (
        pcb.three_d.revision,
        view.camera.mirrored_x,
        ctx.pixels_per_point().to_bits(),
    )
        .hash(&mut hash);
    // A full UI atlas is periodically replaced by egui. Artwork owns its atlas,
    // so that replacement must not trigger a board rebuild. Font definitions,
    // unlike atlas allocations, do affect the cached glyphs.
    ctx.fonts(|f| {
        let fonts = f.lock();
        let definitions = fonts.fonts.definitions();
        for (name, data) in &definitions.font_data {
            name.hash(&mut hash);
            (Arc::as_ptr(data) as usize).hash(&mut hash);
        }
        definitions.families.hash(&mut hash);
    });
    gpu::format(ctx).hash(&mut hash);
    (
        view.grid,
        view.grid_nm,
        &pcb.visible,
        pcb.active_layer,
        pcb.single_layer,
        pcb.highlight_net,
        &pcb.highlight_nets,
        &pcb.selected,
        pcb.polygons_visible,
    )
        .hash(&mut hash);
    pcb.display.paint_hash(&mut hash);
    hash
}
fn shared_key(board: &Board, view: &View, ctx: &egui::Context) -> Option<u64> {
    if view.pcb.preview.is_some() && view.pcb.route_preview.is_none() {
        return None;
    }
    let mut hash = common_hash(view, ctx);
    view.pcb
        .route_preview
        .as_ref()
        .map_or(board.items.len(), |r| r.prefix)
        .hash(&mut hash);
    Some(hash.finish())
}
pub(super) fn group_signature<'a>(
    board: &Board,
    view: &BoardView,
    items: impl IntoIterator<Item = &'a Item>,
) -> u64 {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut stale = false;
    for item in items {
        if view
            .preview_items
            .as_ref()
            .is_some_and(|ids| ids.contains(&item.id))
        {
            item.id.hash(&mut hash);
        }
        stale |= board.pours_stale && item.polygon.is_some();
    }
    stale.hash(&mut hash);
    hash.finish()
}
pub(super) fn key(board: &Board, view: &View, ctx: &egui::Context) -> u64 {
    let pcb = &view.pcb;
    let mut hash = common_hash(view, ctx);
    (
        if pcb.preview_items.is_some() {
            0
        } else {
            board as *const Board as usize
        },
        if pcb.preview_items.is_some() {
            0
        } else {
            pcb.preview_generation
        },
    )
        .hash(&mut hash);
    pcb.route_preview.is_some().hash(&mut hash);
    if let Some(route) = &pcb.route_preview {
        route.prefix.hash(&mut hash);
        route.source_changes.hash(&mut hash);
    } else {
        pcb.preview_items.hash(&mut hash);
        if let Some(ids) = &pcb.preview_items {
            for id in ids {
                let item = &board.items[pcb.geometry.indices[id]];
                // A route can change layer/net or add a via. Those affect cached
                // ordering, visibility and live colors even with stable identities.
                (
                    item.layer,
                    item.net,
                    item.component,
                    item.keepout,
                    item.polygon,
                    std::mem::discriminant(&item.shape),
                )
                    .hash(&mut hash);
            }
        }
    }
    board.pours_stale.hash(&mut hash);
    pcb.display.paint_hash(&mut hash);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_entry_reuses_stationary_groups_and_rebuilds_only_stale_copper() {
        let ctx = egui::Context::default();
        gpu::enable_gpu(&ctx, egui_wgpu::wgpu::TextureFormat::Rgba8Unorm);
        let mut board = Board::default();
        board.polygon_pours.push(eda_core::pcb_polygon::Polygon {
            contour: [
                Point::mm(-4, -4),
                Point::mm(4, -4),
                Point::mm(4, 4),
                Point::mm(-4, 4),
            ]
            .map(|position| ContourVertex {
                position,
                arc: None,
            })
            .into(),
            ..Default::default()
        });
        let mut copper = Item::new(
            TOP,
            None,
            Shape::Fill {
                start: Point::mm(-4, -4),
                end: Point::mm(4, 4),
                rotation: 0.,
            },
        );
        copper.polygon = Some(0);
        board.items.push(copper);
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(-2, 0),
                end: Point::mm(2, 0),
                width: 250_000,
            },
        ));
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 20.;
        view.pcb.initialize(&board);
        view.pcb.display.connections = false;
        view.pcb.visible.insert(37);
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        let original = view.pcb.paint_cache.groups.clone();
        let source_gpu = Arc::downgrade(view.pcb.paint_cache.gpu.as_ref().unwrap());
        let mut route = eda_core::pcb_route::RouteSession::begin(
            &board,
            0,
            Point::mm(5, 5),
            TOP,
            None,
            250_000,
            0,
        )
        .unwrap();
        route.conflict = eda_core::pcb_route::Conflict::Ignore;
        for end in [Point::mm(6, 5), route.anchor, Point::mm(6, 6)] {
            view.pcb.set_preview(None);
            let preview = route.preview_shared(end).unwrap();
            let stale = preview.board.pours_stale;
            view.pcb.set_route_preview(preview);
            frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
            for group in [
                Group::Board,
                Group::Artwork(TOP, 1),
                Group::Details(TOP, 1),
                Group::Mask(37),
                Group::Polygons,
            ] {
                assert!(Arc::ptr_eq(
                    &original[&group],
                    &view.pcb.paint_cache.groups[&group]
                ));
            }
            if stale {
                assert!(!Arc::ptr_eq(
                    &original[&Group::Artwork(TOP, 0)],
                    &view.pcb.paint_cache.groups[&Group::Artwork(TOP, 0)]
                ));
            }
        }
        let before = view.pcb.paint_cache.groups.clone();
        view.pcb.highlight_net = Some(0);
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert!(
            before
                .iter()
                .all(|(id, old)| !Arc::ptr_eq(old, &view.pcb.paint_cache.groups[id]))
        );
        drop(original);
        drop(before);
        assert!(
            source_gpu.upgrade().is_none(),
            "expired groups retained old GPU resources"
        );
        let before = view.pcb.paint_cache.groups.clone();
        view.pcb.three_d.revision += 1;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert!(
            before
                .iter()
                .all(|(id, old)| !Arc::ptr_eq(old, &view.pcb.paint_cache.groups[id]))
        );
    }
    fn frame(ctx: &egui::Context, board: &Board, view: &mut View, point: Pos2) {
        frame_with_texture_limit(ctx, board, view, point, 2048);
    }
    fn frame_with_texture_limit(
        ctx: &egui::Context,
        board: &Board,
        view: &mut View,
        point: Pos2,
        limit: usize,
    ) {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
                max_texture_side: Some(limit),
                focused: true,
                events: vec![egui::Event::PointerMoved(point)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    canvas(ui, board, view);
                });
            },
        );
        let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
    }
    #[test]
    fn cursor_pan_and_live_tools_reuse_artwork_but_content_and_display_changes_expire_it() {
        let ctx = egui::Context::default();
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(-2, 0),
                end: Point::mm(2, 0),
                width: 500_000,
            },
        ));
        let original = board.clone();
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 20.;
        view.pcb.initialize(&board);
        view.pcb.display.connections = false;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        let builds = view.pcb.paint_cache.builds;
        assert_eq!(
            view.pcb.input.as_ref().unwrap().hit,
            Some(board.items[0].id)
        );
        for n in 0..10 {
            view.camera.centre_mm[0] += 0.1;
            view.pcb.selection_box = Some((Point::default(), Point::mm(n, 1)));
            view.pcb.draft_outline = vec![Point::default(), Point::mm(n, 2)];
            frame(&ctx, &board, &mut view, Pos2::new(400. + n as f32, 300.));
            assert_eq!(view.pcb.paint_cache.builds, builds);
        }
        assert_eq!(board, original);
        view.pcb.display.hidden[4] = true;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 1);
        assert!(view.pcb.input.as_ref().unwrap().hits.is_empty());
        view.pcb.display.hidden[4] = false;
        view.pcb.active_layer = BOTTOM;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 2);
        view.pcb.selected.push(board.items[0].id);
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 3);
        view.pcb.three_d.revision += 1;
        board.items[0] = board.items[0].with_anchor(Point::mm(5, 5)).unwrap();
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 4);
        view.camera.pixels_per_mm *= 2.;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 4);
        view.camera.centre_mm[0] += 100.;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 4);
    }
    fn settle(ctx: &egui::Context, board: &Board, view: &mut View) {
        view.pcb.paint_cache.refinement.lock().unwrap().changed =
            Some(Instant::now() - ZOOM_SETTLE);
        let deadline = Instant::now() + Duration::from_secs(10);
        while view.pcb.paint_cache.scale != view.camera.pixels_per_mm {
            assert!(Instant::now() < deadline, "Zoom refinement timed out");
            frame(ctx, board, view, Pos2::new(400., 300.));
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn continuous_zoom_reuses_artwork_and_refines_to_fresh_geometry_after_settling() {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.families.insert(
            egui::FontFamily::Name("sch-sans".into()),
            fonts.families[&egui::FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(-2, 0),
                end: Point::mm(2, 0),
                width: 10_000,
            },
        ));
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Text {
                position: Point::mm(1, 1),
                text: "PCB zoom".into(),
                height: 1_000_000,
                stroke: 100_000,
                rotation: 30.,
                mirrored: false,
                font: "Arial".into(),
            },
        ));
        let original = board.clone();
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 20.;
        view.pcb.initialize(&board);
        view.pcb.display.connections = false;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        let builds = view.pcb.paint_cache.builds;
        for scale in [0.5, 0.75, 1.5, 3., 10., 1.25] {
            let pointer = Pos2::new(330., 270.);
            view.camera.zoom_at(
                pointer,
                Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.)),
                scale,
            );
            frame(&ctx, &board, &mut view, pointer);
            assert_eq!(view.pcb.paint_cache.builds, builds);
        }
        settle(&ctx, &board, &mut view);
        assert_eq!(view.pcb.paint_cache.builds, builds + 1);
        let refined = view.pcb.paint_cache.clone();
        view.pcb.paint_cache = Cache::default();
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        let meshes = |cache: &Cache| {
            cache
                .parts
                .iter()
                .filter_map(|p| match p {
                    Part::Mesh(m) => Some((
                        m.mesh.indices.clone(),
                        m.mesh
                            .vertices
                            .iter()
                            .map(|v| (v.pos, v.color))
                            .collect::<Vec<_>>(),
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(meshes(&refined), meshes(&view.pcb.paint_cache));
        assert_eq!(board, original);
    }
    #[test]
    fn stale_zoom_result_and_disconnected_worker_never_replace_current_artwork() {
        let ctx = egui::Context::default();
        let board = Board::default();
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 20.;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        let mut cache = std::mem::take(&mut view.pcb.paint_cache);
        let (sender, receiver) = mpsc::sync_channel(1);
        let snapshot = cache.refinement.lock().unwrap().snapshot.take().unwrap();
        sender
            .send((
                snapshot,
                Cache {
                    scale: 30.,
                    ..Default::default()
                },
            ))
            .unwrap_or_else(|_| panic!("send failed"));
        cache.refinement.lock().unwrap().job = Some(ZoomJob {
            key: cache.key.unwrap(),
            receiver,
        });
        cache.refresh(40., &ctx);
        assert_eq!(cache.scale, 20.);
        assert!(cache.refinement.lock().unwrap().snapshot.is_some());
        assert!(cache.refinement.lock().unwrap().job.is_none());
        let (sender, receiver) = mpsc::sync_channel(1);
        drop(sender);
        cache.refinement.lock().unwrap().job = Some(ZoomJob {
            key: cache.key.unwrap(),
            receiver,
        });
        cache.refresh(40., &ctx);
        assert_eq!(cache.scale, 20.);
        assert!(cache.refinement.lock().unwrap().failed);
    }
    #[test]
    fn content_change_keeps_one_running_worker_and_discards_its_old_snapshot() {
        let ctx = egui::Context::default();
        let mut board = Board::default();
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 20.;
        view.pcb.initialize(&board);
        view.pcb.display.connections = false;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        let shared = view.pcb.paint_cache.refinement.clone();
        let snapshot = shared.lock().unwrap().snapshot.take().unwrap();
        let (sender, receiver) = mpsc::sync_channel(1);
        shared.lock().unwrap().job = Some(ZoomJob {
            key: view.pcb.paint_cache.key.unwrap(),
            receiver,
        });
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(1, 1),
                end: Point::mm(2, 1),
                width: 200_000,
            },
        ));
        view.pcb.three_d.revision += 1;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert!(Arc::ptr_eq(&shared, &view.pcb.paint_cache.refinement));
        assert!(shared.lock().unwrap().job.is_some());
        sender
            .send((
                snapshot,
                Cache {
                    scale: 40.,
                    ..Default::default()
                },
            ))
            .unwrap_or_else(|_| panic!("send failed"));
        view.camera.pixels_per_mm = 40.;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.scale, 20.);
        assert!(shared.lock().unwrap().job.is_none());
        assert_eq!(
            shared.lock().unwrap().snapshot.as_ref().unwrap().board,
            board
        );
        settle(&ctx, &board, &mut view);
        assert_eq!(view.pcb.paint_cache.scale, 40.);
    }
    #[test]
    fn replacing_ui_atlas_reuses_artwork_but_changing_font_definitions_expires_it() {
        let ctx = egui::Context::default();
        let board = Board::default();
        let mut view = View::default();
        view.camera.fitted = true;
        view.pcb.initialize(&board);
        view.pcb.display.connections = false;
        let point = Pos2::new(400., 300.);
        frame_with_texture_limit(&ctx, &board, &mut view, point, 2048);
        let builds = view.pcb.paint_cache.builds;
        let atlas = ctx.fonts(|f| f.texture_atlas());
        frame_with_texture_limit(&ctx, &board, &mut view, point, 4096);
        assert!(!Arc::ptr_eq(&atlas, &ctx.fonts(|f| f.texture_atlas())));
        assert_eq!(view.pcb.paint_cache.builds, builds);
        let mut fonts = ctx.fonts(|f| f.lock().fonts.definitions().clone());
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .reverse();
        ctx.set_fonts(fonts);
        frame_with_texture_limit(&ctx, &board, &mut view, point, 4096);
        assert_eq!(view.pcb.paint_cache.builds, builds + 1);
    }
    #[test]
    fn grid_scanlines_match_point_in_polygon_at_vertices_edges_and_concave_rows() {
        let outline = [
            Point::mm(-5, -4),
            Point::mm(5, -4),
            Point::mm(5, 4),
            Point::mm(1, 1),
            Point::mm(-1, 4),
            Point::mm(-5, 4),
        ];
        for y in -10..=10 {
            let y = y * 500_000;
            let crossings = grid_crossings(&outline, y);
            for x in -12..=12 {
                let x = x * 500_000;
                assert_eq!(
                    crossings.partition_point(|crossing| *crossing <= x as f64) % 2 == 1,
                    point_in_polygon(Point { x, y }, &outline),
                    "x={x}, y={y}"
                );
            }
        }
    }
    #[test]
    fn preview_with_inserted_items_keeps_snapping_to_the_original_board() {
        let ctx = egui::Context::default();
        let board = Board::default();
        let mut preview = board.clone();
        preview.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::default(),
                end: Point::mm(1, 0),
                width: 500_000,
            },
        ));
        let mut view = View::default();
        view.camera.fitted = true;
        view.pcb.set_preview(Some(preview));
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(board.items.len(), 0);
        assert_eq!(
            view.pcb.input.as_ref().unwrap().raw_point,
            view.pcb.input.as_ref().unwrap().point
        );
    }
}
