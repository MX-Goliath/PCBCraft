//! A read-only view of the model. Selection and camera are view state.
use eda_core::{Document, DocumentKind, Layer};
use eda_geom::Point;
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod image_cache;
pub mod pcb;
pub mod pcb_3d;
pub mod pcb_stack;
mod schematic;

/// Read-only preview of a library fragment, fitted to its visible geometry.
pub fn symbol_preview(
    ui: &mut egui::Ui,
    fragment: &eda_core::schematic_edit::Fragment,
    height: f32,
) -> egui::Response {
    schematic::symbol_preview(ui, fragment, height)
}

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
    #[serde(default)]
    pub mirrored_x: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            centre_mm: [0.0, 0.0],
            pixels_per_mm: 10.0,
            fitted: false,
            mirrored_x: false,
        }
    }
}

impl Camera {
    pub fn fit(&mut self, doc: &Document, rect: Rect) {
        if !rect.is_finite() || rect.width() <= 120.0 || rect.height() <= 120.0 {
            return;
        }
        let width = doc.size.x as f64 / 1e6;
        let height = doc.size.y as f64 / 1e6;
        self.centre_mm = [width / 2.0, height / 2.0];
        let padding = if doc.kind == DocumentKind::Schematic {
            0.0
        } else {
            120.0
        };
        self.pixels_per_mm = ((rect.width() as f64 - padding) / width)
            .min((rect.height() as f64 - padding) / height)
            .clamp(0.05, 500.0);
        self.fitted = true;
    }
    pub fn screen(&self, point: Point, rect: Rect) -> Pos2 {
        self.screen_mm([point.x as f64 / 1e6, point.y as f64 / 1e6], rect)
    }
    fn screen_mm(&self, point: [f64; 2], rect: Rect) -> Pos2 {
        rect.center()
            + Vec2::new(
                ((point[0] - self.centre_mm[0])
                    * self.pixels_per_mm
                    * if self.mirrored_x { -1.0 } else { 1.0 }) as f32,
                (-(point[1] - self.centre_mm[1]) * self.pixels_per_mm) as f32,
            )
    }
    pub fn world_mm(&self, point: Pos2, rect: Rect) -> [f64; 2] {
        [
            self.centre_mm[0]
                + (point.x - rect.center().x) as f64 / self.pixels_per_mm
                    * if self.mirrored_x { -1.0 } else { 1.0 },
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
#[serde(default)]
pub struct View {
    #[serde(skip)]
    pub cross_probe: bool,
    #[serde(skip)]
    pub pcb: pcb::BoardView,
    pub camera: Camera,
    pub grid: bool,
    pub grid_nm: i64,
    pub visible: [bool; 4],
    pub active_layer: Layer,
    pub selected: Option<Uuid>,
    pub select_pads: bool,
    pub show_axes: bool,
    pub snap: SnapOptions,
    pub sch_select_components: bool,
    pub sch_select_wires: bool,
    pub sch_select_text: bool,
    pub sch_select_graphics: bool,
    pub sch_filters: [bool; 12],
    pub sch_selection: Vec<Uuid>,
    #[serde(skip)]
    pub sch_segments: Vec<eda_core::schematic_edit::WireSegment>,
    /// Ephemeral display strings supplied by the shell, never document geometry.
    #[serde(skip)]
    pub source_display_path: String,
    #[serde(skip)]
    pub display_date: String,
    #[serde(skip)]
    pub sch_tool_active: bool,
    #[serde(skip)]
    pub sch_preview: Option<eda_core::schematic::Schematic>,
    #[serde(skip)]
    sch_junction_cache: schematic::JunctionCache,
    #[serde(skip)]
    sch_press_origin: Option<Pos2>,
    #[serde(skip)]
    sch_primary_down: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SnapMode {
    AllLayers,
    #[default]
    CurrentLayer,
    Off,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SnapOptions {
    pub mode: SnapMode,
    pub track_vertices: bool,
    pub pad_centres: bool,
    pub pad_vertices: bool,
    pub pad_edges: bool,
    pub distance_nm: i64,
}

impl Default for SnapOptions {
    fn default() -> Self {
        Self {
            mode: SnapMode::CurrentLayer,
            track_vertices: true,
            pad_centres: true,
            pad_vertices: false,
            pad_edges: false,
            distance_nm: 250_000,
        }
    }
}

impl Default for View {
    fn default() -> Self {
        Self {
            pcb: pcb::BoardView::default(),
            camera: Camera::default(),
            grid: true,
            grid_nm: 1_000_000,
            visible: [true; 4],
            active_layer: Layer::Top,
            cross_probe: false,
            selected: None,
            select_pads: true,
            show_axes: true,
            snap: SnapOptions::default(),
            sch_select_components: true,
            sch_select_wires: true,
            sch_select_text: true,
            sch_select_graphics: true,
            sch_filters: [true; 12],
            sch_selection: vec![],
            sch_segments: vec![],
            source_display_path: String::new(),
            display_date: String::new(),
            sch_tool_active: false,
            sch_preview: None,
            sch_junction_cache: None,
            sch_press_origin: None,
            sch_primary_down: false,
        }
    }
}

impl View {
    pub fn cancel_schematic_gesture(&mut self) {
        self.sch_press_origin = None;
        self.sch_primary_down = false;
    }
    /// Candidate search reads the actual visible geometry; it never edits document coordinates.
    pub fn snap_point(&self, doc: &Document, raw: Point) -> Point {
        if self.snap.mode == SnapMode::Off {
            return raw;
        }
        let mut result = raw;
        let mut best = self.snap.distance_nm.max(0) as i128;
        best *= best;
        let mut consider = |point: Point| {
            let dx = point.x as i128 - raw.x as i128;
            let dy = point.y as i128 - raw.y as i128;
            let distance = dx * dx + dy * dy;
            if distance <= best {
                best = distance;
                result = point;
            }
        };
        let eligible = |layer: Layer| {
            self.visible[layer_index(layer)]
                && (self.snap.mode == SnapMode::AllLayers || layer == self.active_layer)
        };
        if self.snap.track_vertices {
            for track in &doc.tracks {
                if eligible(track.layer) {
                    consider(track.start);
                    consider(track.end);
                }
            }
        }
        for pad in &doc.pads {
            if !eligible(pad.layer) {
                continue;
            }
            if self.snap.pad_centres {
                consider(pad.position);
            }
            let left = pad.position.x - pad.size.x / 2;
            let right = pad.position.x + pad.size.x / 2;
            let bottom = pad.position.y - pad.size.y / 2;
            let top = pad.position.y + pad.size.y / 2;
            if self.snap.pad_vertices {
                for (x, y) in [(left, bottom), (left, top), (right, bottom), (right, top)] {
                    consider(Point { x, y });
                }
            }
            if self.snap.pad_edges {
                for point in [
                    Point {
                        x: raw.x.clamp(left, right),
                        y: bottom,
                    },
                    Point {
                        x: raw.x.clamp(left, right),
                        y: top,
                    },
                    Point {
                        x: left,
                        y: raw.y.clamp(bottom, top),
                    },
                    Point {
                        x: right,
                        y: raw.y.clamp(bottom, top),
                    },
                ] {
                    consider(point);
                }
            }
        }
        result
    }
}

pub struct CanvasOutput {
    pub cursor_mm: Option<[f64; 2]>,
    pub schematic_input: Option<SchematicInput>,
}

#[derive(Default)]
pub struct SchematicInput {
    /// Pointer over the unobscured canvas, including a drag originating in another panel.
    pub drop_point: Option<Point>,
    pub point: Option<Point>,
    pub press_origin: Option<Point>,
    pub hit: Option<Uuid>,
    pub wire_handle: Option<eda_core::schematic_edit::WireHandle>,
    pub clicked: bool,
    pub secondary_clicked: bool,
    pub drag_started: bool,
    pub dragging: bool,
    pub drag_stopped: bool,
}

pub fn canvas(ui: &mut egui::Ui, doc: &Document, view: &mut View) -> CanvasOutput {
    if let Some(board) = &doc.pcb {
        return pcb::canvas(ui, board, view);
    }
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
        if doc.kind == DocumentKind::Schematic && !ui.ctx().wants_keyboard_input() {
            let factor = ui.input(|i| {
                if i.key_pressed(egui::Key::PageUp) {
                    1.5
                } else if i.key_pressed(egui::Key::PageDown) {
                    1.0 / 1.5
                } else {
                    1.0
                }
            });
            if factor != 1.0 {
                view.camera.zoom_at(pointer, rect, factor);
            }
        }
    }
    // Schematic repeated clicks refine wire selection; fitting here changes
    // document coordinates before the second click can select a segment.
    if response.double_clicked() && doc.kind != DocumentKind::Schematic && !view.sch_tool_active {
        view.camera.fit(doc, rect);
    }
    if doc.kind == DocumentKind::Schematic {
        return schematic::paint(ui, doc, view, rect, &response);
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
        board,
        1.0,
        if doc.kind == DocumentKind::Board {
            Color32::BLACK
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
        if view.select_pads && pointer.is_some_and(|pos| pad_rect.expand(4.0).contains(pos)) {
            hit = Some(pad.id);
        }
    }
    if response.clicked() {
        view.selected = hit;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let origin = view.camera.screen(Point::default(), rect);
    if view.show_axes {
        painter.line_segment(
            [origin - Vec2::new(8.0, 0.0), origin + Vec2::new(8.0, 0.0)],
            Stroke::new(1.0, Color32::GRAY),
        );
        painter.line_segment(
            [origin - Vec2::new(0.0, 8.0), origin + Vec2::new(0.0, 8.0)],
            Stroke::new(1.0, Color32::GRAY),
        );
    }
    let cursor_mm = pointer.map(|p| {
        let raw = view.camera.world_mm(p, rect);
        let snapped = view.snap_point(
            doc,
            Point {
                x: (raw[0] * 1e6).round() as i64,
                y: (raw[1] * 1e6).round() as i64,
            },
        );
        if (snapped.x as f64 / 1e6 - raw[0]).abs() > 1e-9
            || (snapped.y as f64 / 1e6 - raw[1]).abs() > 1e-9
        {
            painter.circle_stroke(
                view.camera.screen(snapped, rect),
                4.0,
                Stroke::new(1.0, Color32::YELLOW),
            );
        }
        [snapped.x as f64 / 1e6, snapped.y as f64 / 1e6]
    });
    CanvasOutput {
        cursor_mm,
        schematic_input: None,
    }
}

pub fn layer_index(layer: Layer) -> usize {
    Layer::ALL.iter().position(|l| *l == layer).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapping_respects_radius_layer_visibility_and_off() {
        let mut doc = Document::example();
        doc.tracks.clear();
        doc.pads.truncate(1);
        doc.pads[0].layer = Layer::Bottom;
        let original = doc.clone();
        let target = doc.pads[0].position;
        let raw = Point {
            x: target.x + 100_000,
            y: target.y,
        };
        let mut view = View::default();
        assert_eq!(view.snap_point(&doc, raw), raw);
        view.snap.mode = SnapMode::AllLayers;
        assert_eq!(view.snap_point(&doc, raw), target);
        view.visible[layer_index(Layer::Bottom)] = false;
        assert_eq!(view.snap_point(&doc, raw), raw);
        view.visible[layer_index(Layer::Bottom)] = true;
        view.snap.distance_nm = 99_999;
        assert_eq!(view.snap_point(&doc, raw), raw);
        view.snap.distance_nm = 100_000;
        assert_eq!(view.snap_point(&doc, raw), target);
        view.snap.mode = SnapMode::Off;
        assert_eq!(view.snap_point(&doc, raw), raw);
        assert_eq!(doc, original);
    }

    #[test]
    fn edge_snapping_projects_onto_finite_rectangular_pad_edges() {
        let mut doc = Document::example();
        doc.tracks.clear();
        doc.pads.truncate(1);
        let pad = &doc.pads[0];
        let corner = Point {
            x: pad.position.x + pad.size.x / 2,
            y: pad.position.y + pad.size.y / 2,
        };
        let raw = Point {
            x: corner.x + 100_000,
            y: corner.y + 100_000,
        };
        let mut view = View::default();
        view.snap.pad_centres = false;
        view.snap.track_vertices = false;
        assert_eq!(view.snap_point(&doc, raw), raw);
        view.snap.pad_edges = true;
        assert_eq!(view.snap_point(&doc, raw), corner);
        let side = Point {
            x: corner.x + 100_000,
            y: pad.position.y,
        };
        assert_eq!(
            view.snap_point(&doc, side),
            Point {
                x: corner.x,
                y: side.y
            }
        );
    }
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
    #[test]
    fn initial_document_fit_waits_for_nonempty_layout() {
        let mut camera = Camera::default();
        let doc = Document::example();
        camera.fit(&doc, Rect::from_min_size(Pos2::ZERO, Vec2::ZERO));
        assert!(!camera.fitted);
        camera.fit(
            &doc,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0)),
        );
        assert!(camera.fitted);
        assert!(camera.pixels_per_mm > 0.05);
    }
    #[test]
    fn bottom_view_mirrors_coordinates_and_keeps_zoom_anchored() {
        let rect = Rect::from_min_size(Pos2::new(330., 90.), Vec2::new(800., 600.));
        let mut camera = Camera {
            centre_mm: [15., 25.],
            mirrored_x: true,
            ..Default::default()
        };
        let point = Point::mm(20, 31);
        let pixel = camera.screen(point, rect);
        assert!(pixel.x < rect.center().x);
        let before = camera.world_mm(pixel, rect);
        assert_eq!(before, [20., 31.]);
        camera.zoom_at(pixel, rect, 3.2);
        let after = camera.world_mm(pixel, rect);
        for n in 0..2 {
            assert!((after[n] - before[n]).abs() < 1e-9);
        }
    }
}
