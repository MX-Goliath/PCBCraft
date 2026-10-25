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

#[derive(Clone)]
enum Part {
    Mesh(Arc<Mesh>),
    Grid,
    Hatch(Uuid, Color32),
    Tools,
    Origin,
}
enum Capture {
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
        }
    }
    fn flush(&mut self, ctx: &egui::Context) {
        let shapes = ctx.graphics_mut(|g| std::mem::take(g.entry(self.layer)));
        if !shapes.is_empty() {
            self.captured.push(Capture::Shapes(shapes));
        }
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
            builds: 0,
        }
    }
}
impl Cache {
    pub fn rebuild(&mut self, painter: &egui::Painter, board: &Board, view: &mut View, key: u64) {
        let mut result = Self::build(painter, board, view, key, self.builds);
        result.refinement = self.refinement.clone();
        result.seed(board, view, painter.ctx());
        *self = result;
    }
    fn build(
        painter: &egui::Painter,
        board: &Board,
        view: &mut View,
        key: u64,
        builds: usize,
    ) -> Self {
        let start = Instant::now();
        let mut scene_painter = painter.clone();
        // Include offscreen objects so navigation never leaves cached artwork.
        scene_painter.set_clip_rect(Rect::EVERYTHING);
        let mut builder = Builder::new(&scene_painter);
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
        if std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some() {
            eprintln!(
                "pcb_artwork_build n={} cpu_ms={:.2} zoom={:.6} revision={} preview={}",
                result.builds,
                start.elapsed().as_secs_f64() * 1000.,
                camera.pixels_per_mm,
                view.pcb.three_d.revision,
                view.pcb.preview_generation,
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
                            for part in &mut result.parts {
                                if let Part::Mesh(mesh) = part {
                                    Arc::make_mut(mesh).texture_id = atlas.id();
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
        for captured in builder.captured {
            match captured {
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
                            self.parts.push(Part::Mesh(Arc::new(mesh)));
                        }
                    }
                }
            }
        }
        self.gpu = gpu::format(ctx).map(|format| {
            gpu::Scene::new(
                self.parts
                    .iter()
                    .filter_map(|part| match part {
                        Part::Mesh(mesh) => Some(mesh.clone()),
                        _ => None,
                    })
                    .collect(),
                image.clone(),
                format,
            )
        });
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
        let mut mesh_index = 0;
        for part in &self.parts {
            match part {
                Part::Mesh(mesh) => {
                    if let Some(gpu) = &self.gpu {
                        painter.add(gpu.callback(mesh_index, rect, delta, scale, true));
                        mesh_index += 1;
                        continue;
                    }
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

pub(super) fn key(board: &Board, view: &View, ctx: &egui::Context) -> u64 {
    let pcb = &view.pcb;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (
        board as *const Board as usize,
        pcb.three_d.revision,
        pcb.preview_generation,
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
        &pcb.selected,
        pcb.polygons_visible,
    )
        .hash(&mut hash);
    (Arc::as_ptr(&pcb.connections.lines) as usize).hash(&mut hash);
    pcb.display.paint_hash(&mut hash);
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
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
                        m.indices.clone(),
                        m.vertices
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
