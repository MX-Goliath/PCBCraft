//! A read-only view of the model. Selection and camera are view state.
use eda_core::{Document, DocumentKind, Layer};
use eda_geom::Point;
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn layer_color(layer: Layer) -> Color32 {
    match layer {
        Layer::Top => Color32::from_rgb(219, 77, 72),
        Layer::Bottom => Color32::from_rgb(82, 139, 221),
        Layer::Overlay => Color32::from_rgb(219, 206, 133),
        Layer::Outline => Color32::from_rgb(160, 176, 161),
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Camera {
    pub centre_mm: [f64; 2],
    pub pixels_per_mm: f64,
    pub fitted: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            centre_mm: [0.0, 0.0],
            pixels_per_mm: 10.0,
            fitted: false,
        }
    }
}

impl Camera {
    pub fn fit(&mut self, doc: &Document, rect: Rect) {
        let width = doc.size.x as f64 / 1e6;
        let height = doc.size.y as f64 / 1e6;
        self.centre_mm = [width / 2.0, height / 2.0];
        self.pixels_per_mm = ((rect.width() as f64 - 120.0) / width)
            .min((rect.height() as f64 - 120.0) / height)
            .clamp(0.05, 500.0);
        self.fitted = true;
    }
    pub fn screen(&self, point: Point, rect: Rect) -> Pos2 {
        self.screen_mm([point.x as f64 / 1e6, point.y as f64 / 1e6], rect)
    }
    fn screen_mm(&self, point: [f64; 2], rect: Rect) -> Pos2 {
        rect.center()
            + Vec2::new(
                ((point[0] - self.centre_mm[0]) * self.pixels_per_mm) as f32,
                (-(point[1] - self.centre_mm[1]) * self.pixels_per_mm) as f32,
            )
    }
    pub fn world_mm(&self, point: Pos2, rect: Rect) -> [f64; 2] {
        [
            self.centre_mm[0] + (point.x - rect.center().x) as f64 / self.pixels_per_mm,
            self.centre_mm[1] - (point.y - rect.center().y) as f64 / self.pixels_per_mm,
        ]
    }
    /// Anchor zoom at the pointer, so the world point under the mouse stays fixed.
    pub fn zoom_at(&mut self, point: Pos2, rect: Rect, factor: f64) {
        let before = self.world_mm(point, rect);
        self.pixels_per_mm = (self.pixels_per_mm * factor).clamp(0.05, 500.0);
        let after = self.world_mm(point, rect);
        self.centre_mm[0] += before[0] - after[0];
        self.centre_mm[1] += before[1] - after[1];
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct View {
    pub camera: Camera,
    pub grid: bool,
    pub grid_nm: i64,
    pub visible: [bool; 4],
    pub active_layer: Layer,
    pub selected: Option<Uuid>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            grid: true,
            grid_nm: 1_000_000,
            visible: [true; 4],
            active_layer: Layer::Top,
            selected: None,
        }
    }
}

pub struct CanvasOutput {
    pub cursor_mm: Option<[f64; 2]>,
}

pub fn canvas(ui: &mut egui::Ui, doc: &Document, view: &mut View) -> CanvasOutput {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(109, 106, 105));
    if !view.camera.fitted {
        view.camera.fit(doc, rect);
    }
    if response.dragged_by(egui::PointerButton::Middle)
        || response.dragged_by(egui::PointerButton::Secondary)
    {
        let delta = ui.input(|i| i.pointer.delta());
        view.camera.centre_mm[0] -= delta.x as f64 / view.camera.pixels_per_mm;
        view.camera.centre_mm[1] += delta.y as f64 / view.camera.pixels_per_mm;
    }
    let pointer = response.hover_pos();
    if let Some(pointer) = pointer {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            view.camera
                .zoom_at(pointer, rect, (scroll as f64 * 0.003).exp());
        }
    }
    if response.double_clicked() {
        view.camera.fit(doc, rect);
    }
    let top_left = view.camera.screen(
        Point {
            x: 0,
            y: doc.size.y,
        },
        rect,
    );
    let bottom_right = view.camera.screen(
        Point {
            x: doc.size.x,
            y: 0,
        },
        rect,
    );
    let board = Rect::from_two_pos(top_left, bottom_right);
    painter.rect_filled(
        board.translate(Vec2::splat(5.0)),
        2.0,
        Color32::from_black_alpha(55),
    );
    painter.rect_filled(
        board,
        1.0,
        if doc.kind == DocumentKind::Board {
            Color32::from_rgb(22, 26, 25)
        } else {
            Color32::from_rgb(232, 230, 218)
        },
    );

    if view.grid {
        // Decimate display only; the document/grid step is never changed by zoom.
        let mut step = view.grid_nm as f64 / 1e6;
        while step * view.camera.pixels_per_mm < 12.0 {
            step *= 5.0;
        }
        let min = view.camera.world_mm(rect.left_bottom(), rect);
        let max = view.camera.world_mm(rect.right_top(), rect);
        let cols = ((max[0] - min[0]) / step).ceil() as i32 + 2;
        let rows = ((max[1] - min[1]) / step).ceil() as i32 + 2;
        let start = [
            (min[0] / step).floor() * step,
            (min[1] / step).floor() * step,
        ];
        for x in 0..cols.min(300) {
            for y in 0..rows.min(300) {
                let point = view.camera.screen_mm(
                    [start[0] + x as f64 * step, start[1] + y as f64 * step],
                    rect,
                );
                painter.circle_filled(
                    point,
                    0.7,
                    Color32::from_gray(if board.contains(point) { 69 } else { 62 }),
                );
            }
        }
    }
    if view.visible[3] {
        painter.rect_stroke(
            board,
            1.0,
            Stroke::new(1.5, layer_color(Layer::Outline)),
            StrokeKind::Inside,
        );
    }
    for track in &doc.tracks {
        if !view.visible[layer_index(track.layer)] {
            continue;
        }
        painter.line_segment(
            [
                view.camera.screen(track.start, rect),
                view.camera.screen(track.end, rect),
            ],
            Stroke::new(
                (track.width as f64 / 1e6 * view.camera.pixels_per_mm) as f32,
                layer_color(track.layer),
            ),
        );
    }
    let mut hit = None;
    for pad in &doc.pads {
        if !view.visible[layer_index(pad.layer)] {
            continue;
        }
        let centre = view.camera.screen(pad.position, rect);
        let size = Vec2::new(pad.size.x as f32, pad.size.y as f32)
            * (view.camera.pixels_per_mm / 1e6) as f32;
        let pad_rect = Rect::from_center_size(centre, size);
        painter.rect_filled(pad_rect, 2.0, layer_color(pad.layer));
        if view.selected == Some(pad.id) {
            painter.rect_stroke(
                pad_rect.expand(4.0),
                2.0,
                Stroke::new(1.5, Color32::from_rgb(112, 198, 255)),
                StrokeKind::Outside,
            );
        }
        if view.visible[2] && view.camera.pixels_per_mm > 4.0 {
            painter.text(
                centre + Vec2::new(0.0, -size.y / 2.0 - 9.0),
                egui::Align2::CENTER_BOTTOM,
                &pad.name,
                egui::FontId::monospace(11.0),
                layer_color(Layer::Overlay),
            );
        }
        if pointer.is_some_and(|pos| pad_rect.expand(4.0).contains(pos)) {
            hit = Some(pad.id);
        }
    }
    if response.clicked() {
        view.selected = hit;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    if view.visible[2] && doc.kind == DocumentKind::Board {
        painter.text(
            board.left_top() + Vec2::new(14.0, 14.0),
            egui::Align2::LEFT_TOP,
            format!("PCBCraft  /  {}", doc.name),
            egui::FontId::monospace(12.0),
            layer_color(Layer::Overlay),
        );
    }
    let origin = view.camera.screen(Point::default(), rect);
    painter.line_segment(
        [origin - Vec2::new(8.0, 0.0), origin + Vec2::new(8.0, 0.0)],
        Stroke::new(1.0, Color32::GRAY),
    );
    painter.line_segment(
        [origin - Vec2::new(0.0, 8.0), origin + Vec2::new(0.0, 8.0)],
        Stroke::new(1.0, Color32::GRAY),
    );
    painter.text(
        rect.left_bottom() + Vec2::new(16.0, -16.0),
        egui::Align2::LEFT_BOTTOM,
        "Wheel: zoom   ·   Middle / right drag: pan   ·   Double-click: fit",
        egui::FontId::proportional(11.0),
        Color32::from_gray(125),
    );
    CanvasOutput {
        cursor_mm: pointer.map(|p| view.camera.world_mm(p, rect)),
    }
}

pub fn layer_index(layer: Layer) -> usize {
    Layer::ALL.iter().position(|l| *l == layer).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_roundtrip_and_zoom_anchor() {
        let rect = Rect::from_min_size(Pos2::new(300.0, 80.0), Vec2::new(900.0, 600.0));
        let mut camera = Camera::default();
        camera.fit(&Document::example(), rect);
        let point = Point::mm(17, 23);
        let pixel = camera.screen(point, rect);
        let world = camera.world_mm(pixel, rect);
        assert!((world[0] - 17.0).abs() < 1e-4 && (world[1] - 23.0).abs() < 1e-4);
        camera.zoom_at(pixel, rect, 2.7);
        let after = camera.world_mm(pixel, rect);
        assert!((world[0] - after[0]).abs() < 1e-9 && (world[1] - after[1]).abs() < 1e-9);
    }
}
