//! Reuse tessellated PCB artwork between pointer frames and short camera pans.
//! Screen-anchored hatching stays in its original draw order and is drawn live.
use super::*;
use egui::{
    LayerId, TextureHandle,
    epaint::{ClippedPrimitive, Mesh, Primitive},
};
use std::sync::Arc;

#[derive(Clone)]
enum Part {
    Mesh(Arc<Mesh>),
    Hatch(Uuid, Color32),
    Tools,
}
enum Capture {
    Shapes(egui::layers::PaintList),
    Hatch(Uuid, Color32),
    Tools,
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
}

#[derive(Clone)]
pub(super) struct Cache {
    key: Option<u64>,
    origin: Pos2,
    coverage: Rect,
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
            coverage: Rect::NOTHING,
            parts: vec![],
            atlas: None,
            gpu: None,
            builds: 0,
        }
    }
}
impl Cache {
    pub fn valid(&self, key: u64, camera: &Camera, rect: Rect) -> bool {
        let delta = camera.screen(Point::default(), rect) - self.origin;
        self.key == Some(key)
            && self
                .coverage
                .translate(delta)
                .contains_rect(rect.expand(1.))
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
                Capture::Hatch(item, color) => self.parts.push(Part::Hatch(item, color)),
                Capture::Tools => self.parts.push(Part::Tools),
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
                image,
                format,
            )
        });
        self.origin = camera.screen(Point::default(), rect);
        self.coverage = painter.clip_rect();
        self.key = Some(key);
        self.builds += 1;
    }
    pub fn paint(&self, painter: &egui::Painter, board: &Board, view: &mut View, rect: Rect) {
        let delta = view.camera.screen(Point::default(), rect) - self.origin;
        let mut mesh_index = 0;
        for part in &self.parts {
            match part {
                Part::Mesh(mesh) => {
                    if let Some(gpu) = &self.gpu {
                        painter.add(gpu.callback(mesh_index, rect, delta, true));
                        mesh_index += 1;
                        continue;
                    }
                    let mesh = if delta == Vec2::ZERO {
                        mesh.clone()
                    } else {
                        let mut moved = (**mesh).clone();
                        moved.translate(delta);
                        Arc::new(moved)
                    };
                    painter.add(egui::Shape::Mesh(mesh));
                }
                Part::Hatch(id, color) => {
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
        view.camera.pixels_per_mm.to_bits(),
        view.camera.mirrored_x,
        ctx.pixels_per_point().to_bits(),
    )
        .hash(&mut hash);
    ctx.fonts(|f| Arc::as_ptr(&f.texture_atlas()) as usize)
        .hash(&mut hash);
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
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800., 600.))),
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
        assert_eq!(view.pcb.paint_cache.builds, builds + 5);
        view.camera.centre_mm[0] += 100.;
        frame(&ctx, &board, &mut view, Pos2::new(400., 300.));
        assert_eq!(view.pcb.paint_cache.builds, builds + 6);
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
