//! Read-only PCB painter, camera navigation, visibility, and picking.
use crate::{Camera, CanvasOutput, View};
use eda_core::pcb::*;
use eda_geom::Point;
use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2};
use std::{
    collections::{BTreeSet, HashMap},
    hash::{Hash, Hasher},
};
use uuid::Uuid;
#[derive(Clone, Default)]
pub struct BoardView {
    pub initialized: bool,
    pub selected: Vec<Uuid>,
    pub visible: BTreeSet<u16>,
    pub active_layer: u16,
    /// 0: all layers, 1: dim other layers, 2: hide other layers.
    pub single_layer: u8,
    pub highlight_net: Option<u32>,
    pub component_selection: bool,
    pub select_locked: bool,
    pub filters: [bool; 8],
    pub preview: Option<Board>,
    pub tool_active: bool,
    pub selection_box: Option<(Point, Point)>,
    pub draft_outline: Vec<Point>,
    pub input: Option<Input>,
    pub show_layer_dialog: bool,
    pub show_grid_dialog: bool,
    pub polygons_visible: bool,
    cache: HashMap<Uuid, (u64, Vec<usize>)>,
}
#[derive(Clone, Default)]
pub struct Input {
    pub point: Option<Point>,
    pub raw_point: Option<Point>,
    pub press_origin: Option<Point>,
    pub hit: Option<Uuid>,
    pub hits: Vec<Uuid>,
    pub clicked: bool,
    pub double_clicked: bool,
    pub secondary_clicked: bool,
    pub drag_started: bool,
    pub dragging: bool,
    pub drag_stopped: bool,
    pub hovered: bool,
}
pub fn layer_color(id: u16) -> Color32 {
    match id {
        TOP => Color32::from_rgb(255, 0, 0),
        BOTTOM => Color32::from_rgb(0, 0, 255),
        TOP_OVERLAY => Color32::from_rgb(255, 255, 0),
        BOTTOM_OVERLAY => Color32::from_rgb(160, 120, 0),
        35 | 36 => Color32::from_rgb(170, 170, 170),
        37 | 38 => Color32::from_rgb(128, 0, 128),
        MULTI_LAYER => Color32::from_rgb(190, 190, 190),
        KEEP_OUT => Color32::from_rgb(255, 0, 255),
        2..=31 => {
            const C: [[u8; 3]; 6] = [
                [0, 220, 0],
                [0, 220, 220],
                [220, 0, 220],
                [160, 110, 20],
                [180, 180, 0],
                [80, 130, 180],
            ];
            let c = C[(id as usize - 2) % 6];
            Color32::from_rgb(c[0], c[1], c[2])
        }
        39..=54 => Color32::from_rgb(150, 90, 30),
        _ => Color32::from_rgb(175, 190, 175),
    }
}
impl BoardView {
    pub fn initialize(&mut self, b: &Board) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.active_layer = TOP;
        self.visible = b.layers.iter().map(|l| l.id).collect();
        self.visible.insert(MULTI_LAYER);
        self.component_selection = true;
        self.filters = [true; 8];
        self.polygons_visible = true;
    }
    pub fn is_visible(&self, item: &Item) -> bool {
        self.visible.contains(&item.layer)
            && (self.single_layer != 2
                || item.layer == self.active_layer
                || item.layer == MULTI_LAYER)
            && (self.polygons_visible || item.polygon.is_none())
    }
    pub fn selectable(&self, i: &Item) -> bool {
        let idx = match i.shape {
            Shape::Track { .. } => 0,
            Shape::Arc { .. } => 1,
            Shape::Pad { .. } => 2,
            Shape::Via { .. } => 3,
            Shape::Text { .. } => 4,
            Shape::Fill { .. } => 5,
            Shape::Region { .. } => 6,
        };
        self.filters[if i.polygon.is_some() { 7 } else { idx }]
            && (self.select_locked || !i.locked)
            && self.is_visible(i)
    }
    pub fn clear_gesture(&mut self) {
        self.preview = None;
        self.tool_active = false;
        self.selection_box = None;
        self.draft_outline.clear();
        self.input = None;
    }
}
pub fn fit(camera: &mut Camera, b: &Board, rect: Rect) {
    let bounds = b.bounds();
    let w = ((bounds.max.x - bounds.min.x) as f64 / 1e6).max(0.1);
    let h = ((bounds.max.y - bounds.min.y) as f64 / 1e6).max(0.1);
    let c = bounds.centre();
    camera.centre_mm = [c.x as f64 / 1e6, c.y as f64 / 1e6];
    camera.pixels_per_mm = (((rect.width() - 60.0).max(10.0) as f64 / w)
        .min((rect.height() - 60.0).max(10.0) as f64 / h))
    .clamp(0.001, 100_000.0);
    camera.fitted = true;
}
fn world(camera: &Camera, p: Pos2, rect: Rect) -> Point {
    let p = camera.world_mm(p, rect);
    Point {
        x: (p[0] * 1e6).round() as i64,
        y: (p[1] * 1e6).round() as i64,
    }
}
fn dim(c: Color32, alpha: u8) -> Color32 {
    let [r, g, b, _] = c.to_array();
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}
/// PCB zoom uses the same bounds as PCB fit; shared schematic camera limits differ.
fn zoom_at(camera: &mut Camera, point: Pos2, rect: Rect, factor: f64) {
    let before = camera.world_mm(point, rect);
    camera.pixels_per_mm = (camera.pixels_per_mm * factor).clamp(0.001, 100_000.0);
    let after = camera.world_mm(point, rect);
    camera.centre_mm[0] += before[0] - after[0];
    camera.centre_mm[1] += before[1] - after[1];
}
fn navigate(ui: &egui::Ui, r: &Response, camera: &mut Camera, b: &Board, rect: Rect) {
    if !camera.fitted {
        fit(camera, b, rect);
    }
    if r.dragged_by(egui::PointerButton::Middle) || r.dragged_by(egui::PointerButton::Secondary) {
        let d = ui.input(|i| i.pointer.delta());
        camera.centre_mm[0] -= d.x as f64 / camera.pixels_per_mm;
        camera.centre_mm[1] += d.y as f64 / camera.pixels_per_mm;
    }
    if let Some(p) = r.hover_pos() {
        let (scroll, modifiers) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers));
        if modifiers.ctrl || modifiers.command {
            if scroll.y != 0.0 {
                zoom_at(camera, p, rect, (scroll.y as f64 * 0.003).exp());
            }
        } else if modifiers.shift {
            camera.centre_mm[0] -= (scroll.x + scroll.y) as f64 / camera.pixels_per_mm;
        } else {
            camera.centre_mm[0] -= scroll.x as f64 / camera.pixels_per_mm;
            camera.centre_mm[1] += scroll.y as f64 / camera.pixels_per_mm;
        }
        if !ui.ctx().wants_keyboard_input() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::PageDown) && (i.modifiers.ctrl || i.modifiers.command) {
                    fit(camera, b, rect);
                } else if i.key_pressed(egui::Key::PageUp) {
                    zoom_at(camera, p, rect, 1.5);
                } else if i.key_pressed(egui::Key::PageDown) {
                    zoom_at(camera, p, rect, 1.0 / 1.5);
                }
                if i.key_pressed(egui::Key::Home) {
                    camera.centre_mm = camera.world_mm(p, rect);
                }
            });
        }
    }
}
pub fn canvas(ui: &mut egui::Ui, b: &Board, view: &mut View) -> CanvasOutput {
    view.pcb.initialize(b);
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    navigate(ui, &response, &mut view.camera, b, rect);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(109, 106, 105));
    let scene = view.pcb.preview.as_ref().unwrap_or(b);
    // Placement previews use fresh IDs; do not retain abandoned triangulations forever.
    if view.pcb.cache.len() > scene.items.len().saturating_add(256) {
        let present: BTreeSet<_> = scene.items.iter().map(|i| i.id).collect();
        view.pcb.cache.retain(|id, _| present.contains(id));
    }
    let outline = scene.outline_points(256);
    if outline.len() >= 3 {
        polygon(
            &painter,
            &view.camera,
            rect,
            &outline,
            &[],
            Color32::BLACK,
            None,
        );
        painter.add(egui::Shape::closed_line(
            outline
                .iter()
                .map(|p| view.camera.screen(*p, rect))
                .collect(),
            Stroke::new(1.0, Color32::from_gray(180)),
        ));
    }
    if view.grid {
        let mut step = view.grid_nm.max(1) as f64 / 1e6;
        while step * view.camera.pixels_per_mm < 14.0 {
            step *= 5.0;
        }
        let min = view.camera.world_mm(rect.left_bottom(), rect);
        let max = view.camera.world_mm(rect.right_top(), rect);
        let sx = (min[0] / step).floor() * step;
        let sy = (min[1] / step).floor() * step;
        let cols = ((max[0] - min[0]) / step).ceil() as usize + 2;
        let rows = ((max[1] - min[1]) / step).ceil() as usize + 2;
        for x in 0..cols.min(500) {
            for y in 0..rows.min(500) {
                painter.circle_filled(
                    view.camera.screen(
                        Point {
                            x: ((sx + x as f64 * step) * 1e6) as i64,
                            y: ((sy + y as f64 * step) * 1e6) as i64,
                        },
                        rect,
                    ),
                    0.65,
                    Color32::from_gray(65),
                );
            }
        }
    }
    let selected = scene.resolve_selection(&view.pcb.selected);
    let bounds = Bounds::points(&[
        world(&view.camera, rect.left_bottom(), rect),
        world(&view.camera, rect.right_top(), rect),
    ]);
    let mut order: Vec<_> = scene
        .items
        .iter()
        .filter(|i| {
            view.pcb.is_visible(i)
                && i.bounds().intersects(bounds)
                && (view.pcb.single_layer == 0
                    || i.layer != MULTI_LAYER
                    || eda_core::pcb_polygon::on_layer(scene, i, view.pcb.active_layer))
        })
        .collect();
    order.sort_by_key(|i| {
        (
            if i.layer == view.pcb.active_layer {
                1
            } else if i.layer == MULTI_LAYER {
                2
            } else if i.layer == TOP_OVERLAY || i.layer == BOTTOM_OVERLAY {
                3
            } else {
                0
            },
            i.layer,
            match i.shape {
                Shape::Region { .. } | Shape::Fill { .. } => 0,
                _ => 1,
            },
        )
    });
    for item in &order {
        let mut color = layer_color(item.layer);
        if view.pcb.single_layer == 1
            && item.layer != view.pcb.active_layer
            && item.layer != MULTI_LAYER
        {
            color = dim(color, 50);
        }
        if view.pcb.highlight_net.is_some() && item.net != view.pcb.highlight_net {
            color = dim(color, 35);
        }
        if selected.contains(&item.id) {
            color = Color32::from_rgb(255, 255, 255);
        }
        if scene.pours_stale && item.polygon.is_some() {
            color = dim(color, 50);
        }
        paint_item(
            &painter,
            &view.camera,
            rect,
            item,
            view.pcb.active_layer,
            color,
            &mut view.pcb.cache,
        );
    }
    for poly in &scene.polygon_pours {
        if !view.pcb.visible.contains(&poly.layer)
            || (view.pcb.single_layer == 2 && poly.layer != view.pcb.active_layer)
        {
            continue;
        }
        let selected = view.pcb.selected.contains(&poly.id);
        let color = if selected {
            Color32::WHITE
        } else {
            dim(layer_color(poly.layer), if poly.shelved { 80 } else { 140 })
        };
        let points = poly.points();
        if points.len() > 2 {
            painter.add(egui::Shape::closed_line(
                points
                    .iter()
                    .map(|p| view.camera.screen(*p, rect))
                    .collect(),
                Stroke::new(if selected { 2.0 } else { 0.7 }, color),
            ));
        }
        if selected {
            for hole in &poly.holes {
                painter.add(egui::Shape::closed_line(
                    hole.iter().map(|p| view.camera.screen(*p, rect)).collect(),
                    Stroke::new(1.5, color),
                ));
            }
        }
    }
    for (_, point) in eda_core::pcb_tools::handles(scene, &view.pcb.selected) {
        let r = Rect::from_center_size(view.camera.screen(point, rect), Vec2::splat(7.0));
        painter.rect_filled(r, 0.0, Color32::BLACK);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
    }
    if view.pcb.draft_outline.len() > 1 {
        painter.add(egui::Shape::line(
            view.pcb
                .draft_outline
                .iter()
                .map(|p| view.camera.screen(*p, rect))
                .collect(),
            Stroke::new(1.5, Color32::WHITE),
        ));
    }
    if let Some((a, z)) = view.pcb.selection_box {
        let r = Rect::from_two_pos(view.camera.screen(a, rect), view.camera.screen(z, rect));
        let color = if z.x >= a.x {
            Color32::LIGHT_BLUE
        } else {
            Color32::LIGHT_GREEN
        };
        painter.rect_filled(r, 0.0, dim(color, 25));
        painter.rect_stroke(r, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
    }
    let pointer = response.hover_pos();
    let raw = pointer.map(|p| world(&view.camera, p, rect));
    let bypass = ui.input(|i| i.modifiers.ctrl && i.modifiers.shift);
    let tolerance = (6.0 / view.camera.pixels_per_mm * 1e6).round() as i64;
    let point = raw.map(|p| {
        if bypass {
            p
        } else {
            b.snap(
                p,
                view.grid_nm,
                tolerance.min(1_000_000),
                view.pcb.active_layer,
                &view.pcb.visible,
            )
        }
    });
    let mut hits = Vec::new();
    if let Some(p) = raw {
        let tol = 6.0 / view.camera.pixels_per_mm * 1e6;
        let mut candidates: Vec<_> = order
            .iter()
            .rev()
            .filter(|i| view.pcb.selectable(i))
            .filter_map(|i| {
                let d = hit_distance(i, p);
                (d <= tol).then_some((
                    i.id,
                    match i.shape {
                        Shape::Pad { .. } | Shape::Via { .. } => 0,
                        Shape::Track { .. } | Shape::Arc { .. } => 1,
                        Shape::Text { .. } => 2,
                        _ => 3,
                    },
                    d,
                ))
            })
            .collect();
        candidates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.total_cmp(&b.2)));
        hits.extend(candidates.into_iter().map(|c| c.0));
        if view.pcb.filters[7] {
            for poly in scene.polygon_pours.iter().rev() {
                if !view.pcb.visible.contains(&poly.layer)
                    || (poly.locked && !view.pcb.select_locked)
                    || (view.pcb.single_layer == 2 && poly.layer != view.pcb.active_layer)
                {
                    continue;
                }
                let points = poly.points();
                let edge = points
                    .iter()
                    .enumerate()
                    .any(|(n, a)| segment_distance(p, *a, points[(n + 1) % points.len()]) <= tol);
                let inside = point_in_polygon(p, &points)
                    && !poly.holes.iter().any(|h| point_in_polygon(p, h));
                if inside || edge {
                    hits.push(poly.id);
                }
            }
        }
    }
    let hit = hits.first().copied();
    if let Some(p) = point {
        let s = view.camera.screen(p, rect);
        if response.hovered() {
            painter.line_segment(
                [Pos2::new(rect.left(), s.y), Pos2::new(rect.right(), s.y)],
                Stroke::new(0.5, Color32::from_gray(100)),
            );
            painter.line_segment(
                [Pos2::new(s.x, rect.top()), Pos2::new(s.x, rect.bottom())],
                Stroke::new(0.5, Color32::from_gray(100)),
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }
    if scene.pours_stale {
        painter.text(
            rect.left_top() + Vec2::splat(8.0),
            egui::Align2::LEFT_TOP,
            "Polygon copper preview is stale; it is rebuilt when the edit is committed",
            egui::FontId::proportional(12.0),
            Color32::YELLOW,
        );
    }
    let origin = ui
        .input(|i| i.pointer.press_origin())
        .filter(|p| rect.contains(*p))
        .map(|p| world(&view.camera, p, rect));
    view.pcb.input = Some(Input {
        point,
        raw_point: raw,
        press_origin: origin,
        hit,
        hits,
        clicked: response.clicked(),
        double_clicked: response.double_clicked(),
        secondary_clicked: response.secondary_clicked(),
        drag_started: response.drag_started_by(egui::PointerButton::Primary),
        dragging: response.dragged_by(egui::PointerButton::Primary),
        drag_stopped: response.drag_stopped_by(egui::PointerButton::Primary),
        hovered: response.hovered(),
    });
    CanvasOutput {
        cursor_mm: point.map(|p| [p.x as f64 / 1e6, p.y as f64 / 1e6]),
        schematic_input: None,
    }
}
fn pad_points(position: Point, size: Point, angle: f64, shape: PadShape) -> Vec<Point> {
    eda_core::pcb_polygon::pad_outline(position, size, angle, shape)
}
fn paint_item(
    p: &egui::Painter,
    camera: &Camera,
    rect: Rect,
    item: &Item,
    active: u16,
    color: Color32,
    cache: &mut HashMap<Uuid, (u64, Vec<usize>)>,
) {
    let scale = (camera.pixels_per_mm / 1e6) as f32;
    let screen = |v| camera.screen(v, rect);
    let stroke = |w: i64| Stroke::new((w as f32 * scale).max(0.6), color);
    match &item.shape {
        Shape::Track { start, end, width } => {
            p.line_segment([screen(*start), screen(*end)], stroke(*width));
            p.circle_filled(
                screen(*start),
                (*width as f32 * scale / 2.0).max(0.3),
                color,
            );
            p.circle_filled(screen(*end), (*width as f32 * scale / 2.0).max(0.3), color);
        }
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            let n = ((radius.abs() as f64 * camera.pixels_per_mm / 1e6).sqrt() * 20.0) as usize;
            p.add(egui::Shape::line(
                arc_points(
                    *centre,
                    *radius,
                    *start_angle,
                    *end_angle,
                    n.clamp(32, 1024),
                )
                .into_iter()
                .map(screen)
                .collect(),
                stroke(*width),
            ));
        }
        Shape::Via {
            position,
            diameter,
            hole,
            ..
        } => {
            p.circle_filled(
                screen(*position),
                (*diameter as f32 * scale / 2.0).max(0.5),
                color,
            );
            p.circle_filled(
                screen(*position),
                *hole as f32 * scale / 2.0,
                Color32::BLACK,
            );
        }
        Shape::Pad {
            position,
            size,
            rotation,
            shape,
            hole,
            bottom_size,
            middle_size,
            ..
        } => {
            let sz = if item.layer == MULTI_LAYER {
                if active == BOTTOM {
                    bottom_size.unwrap_or(*size)
                } else if active != TOP {
                    middle_size.unwrap_or(*size)
                } else {
                    *size
                }
            } else {
                *size
            };
            p.add(egui::Shape::convex_polygon(
                pad_points(*position, sz, *rotation, *shape)
                    .into_iter()
                    .map(screen)
                    .collect(),
                color,
                Stroke::NONE,
            ));
            if *hole > 0 {
                p.circle_filled(
                    screen(*position),
                    *hole as f32 * scale / 2.0,
                    Color32::BLACK,
                );
            }
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => {
            p.add(egui::Shape::convex_polygon(
                rectangle(
                    midpoint(*start, *end),
                    Point {
                        x: (end.x - start.x).abs(),
                        y: (end.y - start.y).abs(),
                    },
                    *rotation,
                )
                .into_iter()
                .map(screen)
                .collect(),
                color,
                Stroke::NONE,
            ));
        }
        Shape::Region {
            contour,
            holes,
            cutout,
        } => {
            if *cutout || item.keepout {
                p.add(egui::Shape::closed_line(
                    contour.iter().copied().map(screen).collect(),
                    Stroke::new(1.0, if *cutout { Color32::WHITE } else { color }),
                ));
            } else {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                contour.len().hash(&mut h);
                for v in contour {
                    v.x.hash(&mut h);
                    v.y.hash(&mut h);
                }
                for hole in holes {
                    hole.len().hash(&mut h);
                    for v in hole {
                        v.x.hash(&mut h);
                        v.y.hash(&mut h);
                    }
                }
                let signature = h.finish();
                let e = cache.entry(item.id).or_insert((signature, vec![]));
                if e.0 != signature {
                    e.0 = signature;
                    e.1.clear();
                }
                polygon(p, camera, rect, contour, holes, color, Some(&mut e.1));
            }
        }
        Shape::Text {
            position,
            text,
            height,
            rotation,
            ..
        } => {
            let size = (*height as f32 * scale).clamp(1.0, 512.0);
            if size < 2.0 {
                return;
            }
            let galley = p.layout_no_wrap(text.clone(), egui::FontId::monospace(size), color);
            let mut shape = egui::epaint::TextShape::new(screen(*position), galley, color);
            shape.angle = -(*rotation as f32).to_radians();
            // Glyph mirroring needs a dedicated stroke/font renderer. It is not emulated by

            // reversing the string (which would be incorrect for asymmetric glyphs).

            p.add(shape);
        }
    }
}
fn polygon(
    p: &egui::Painter,
    camera: &Camera,
    rect: Rect,
    contour: &[Point],
    holes: &[Vec<Point>],
    color: Color32,
    cache: Option<&mut Vec<usize>>,
) {
    if contour.len() < 3 {
        return;
    }
    let mut vertices = contour.to_vec();
    let mut hole_indices = Vec::new();
    for h in holes {
        hole_indices.push(vertices.len());
        vertices.extend(h);
    }
    let coords: Vec<f64> = vertices
        .iter()
        .flat_map(|v| [v.x as f64 / 1e6, v.y as f64 / 1e6])
        .collect();
    let own;
    let indices = if let Some(c) = cache {
        if c.is_empty() {
            *c = earcutr::earcut(&coords, &hole_indices, 2).unwrap_or_default();
        }
        c.as_slice()
    } else {
        own = earcutr::earcut(&coords, &hole_indices, 2).unwrap_or_default();
        own.as_slice()
    };
    let mut mesh = egui::Mesh::default();
    for v in &vertices {
        mesh.colored_vertex(camera.screen(*v, rect), color);
    }
    mesh.indices.extend(indices.iter().map(|v| *v as u32));
    p.add(egui::Shape::mesh(mesh));
}
