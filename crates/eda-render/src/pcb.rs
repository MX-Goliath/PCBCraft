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
#[path = "pcb_display.rs"]
mod display;
#[path = "pcb_navigation.rs"]
mod navigation;
#[path = "pcb_text.rs"]
mod text;
pub use navigation::{CursorStyle, FitTarget, NavigationOptions};
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
    pub select_bodies: bool,
    pub select_keepouts: bool,
    pub snap: SnapOptions,
    pub navigation: NavigationOptions,
    pub requested_fit: Option<FitTarget>,
    primary_origin: Option<Point>,
    primary_press: Option<Pos2>,
    primary_dragging: bool,
    suppress_primary_until_release: bool,
    pub preview: Option<Board>,
    pub tool_active: bool,
    pub selection_box: Option<(Point, Point)>,
    pub draft_outline: Vec<Point>,
    pub input: Option<Input>,
    pub show_layer_dialog: bool,
    pub show_grid_dialog: bool,
    pub polygons_visible: bool,
    cache: HashMap<Uuid, (u64, Vec<usize>, bool)>,
    display: display::Settings,
    grid_initialized: bool,
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
    pub cancelled: bool,
    pub panning: bool,
}
pub fn layer_color(id: u16) -> Color32 {
    match id {
        TOP => Color32::from_rgb(153, 0, 0),
        BOTTOM => Color32::from_rgb(11, 83, 148),
        TOP_OVERLAY => Color32::from_rgb(255, 255, 0),
        BOTTOM_OVERLAY => Color32::from_rgb(128, 128, 0),
        35 | 36 => Color32::from_rgb(170, 170, 170),
        37 | 38 => Color32::from_rgb(128, 0, 128),
        MULTI_LAYER => Color32::from_rgb(204, 204, 204),
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
        73 => Color32::from_rgb(255, 0, 0),
        57..=72 | 83..=1090 => {
            let n = if id <= 72 { id - 56 } else { id - 66 };
            match (n - 1) % 4 {
                0 => Color32::from_rgb(255, 0, 255),
                1 => Color32::from_rgb(128, 0, 128),
                2 => Color32::from_rgb(0, 128, 0),
                _ => Color32::from_rgb(128, 128, 0),
            }
        }
        _ => Color32::from_rgb(175, 190, 175),
    }
}
impl BoardView {
    fn begin_frame(&mut self) {
        self.cache.retain(|_, (_, _, used)| std::mem::take(used));
    }
    pub fn initialize(&mut self, b: &Board) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.active_layer = TOP;
        self.display = display::Settings::from_board(b);
        self.visible = self
            .display
            .visible
            .clone()
            .unwrap_or_else(|| b.layers.iter().map(|l| l.id).collect());
        self.component_selection = true;
        self.filters = [true; 8];
        self.select_bodies = true;
        self.select_keepouts = true;
        self.polygons_visible = true;
    }
    pub fn is_visible(&self, item: &Item) -> bool {
        item.properties
            .get("PCBCRAFT_LEGACY_REGION_DUPLICATE")
            .is_none_or(|v| v != "TRUE")
            && self.visible.contains(&item.layer)
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
            && (self.select_bodies || !i.properties.contains_key("PCBCRAFT_BODY_PROJECTION"))
            && (self.select_keepouts || !i.keepout)
            && (self.select_locked || !i.locked)
            && self.is_visible(i)
    }
    pub fn clear_gesture(&mut self) {
        self.clear_pointer();
        self.tool_active = false;
    }
    pub fn clear_pointer(&mut self) {
        self.suppress_primary_until_release = true;
        self.preview = None;
        self.selection_box = None;
        self.draft_outline.clear();
        self.input = None;
        self.primary_origin = None;
        self.primary_press = None;
        self.primary_dragging = false;
    }
}
pub fn fit(camera: &mut Camera, b: &Board, rect: Rect) {
    fit_bounds(camera, b.bounds(), rect);
}
fn fit_bounds(camera: &mut Camera, bounds: Bounds, rect: Rect) {
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
pub fn canvas(ui: &mut egui::Ui, b: &Board, view: &mut View) -> CanvasOutput {
    view.pcb.initialize(b);
    if !view.pcb.grid_initialized {
        view.pcb.grid_initialized = true;
        if let Some(grid) = view.pcb.display.snap_grid_nm {
            view.grid_nm = grid;
        }
    }
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let panning = navigation::navigate(ui, &response, &mut view.camera, b, &mut view.pcb, rect);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(109, 106, 105));
    // Release abandoned preview/deleted/hidden regions after one unused frame,
    // including large meshes that would remain below the old entry-count cap.
    view.pcb.begin_frame();
    let scene = view.pcb.preview.as_ref().unwrap_or(b);
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
        // Native arc/line seams can backtrack by a few nanometres. A joined
        // stroked path can produce an unbounded antialias miter at that seam.
        for (a, z) in outline
            .iter()
            .zip(outline.iter().cycle().skip(1))
            .take(outline.len())
        {
            let a = view.camera.screen(*a, rect);
            let z = view.camera.screen(*z, rect);
            if a.distance(z) > 0.001 {
                painter.line_segment([a, z], Stroke::new(1.0, Color32::from_gray(180)));
            }
        }
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
                let at = Point {
                    x: ((sx + x as f64 * step) * 1e6) as i64,
                    y: ((sy + y as f64 * step) * 1e6) as i64,
                };
                if !point_in_polygon(at, &outline) {
                    continue;
                }
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
                && scene.text_visible(i)
                && i.bounds().intersects(bounds)
                && (view.pcb.single_layer == 0
                    || i.layer != MULTI_LAYER
                    || i.is_drill_only()
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
    paint_masks(&painter, scene, view, rect, false);
    for item in &order {
        let mut color = dim(layer_color(item.layer), view.pcb.display.alpha(item));
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
    paint_masks(&painter, scene, view, rect, true);
    let pointer = response.hover_pos();
    let raw = pointer.map(|p| world(&view.camera, p, rect));
    let bypass = ui.input(|i| i.modifiers.ctrl && i.modifiers.shift);
    let point = raw.map(|p| {
        if bypass {
            p
        } else {
            b.snap_with_options(
                p,
                if view.grid { view.grid_nm } else { 0 },
                view.pcb.active_layer,
                &view.pcb.visible,
                &view.pcb.snap,
            )
        }
    });
    for item in &order {
        let drill = item.drill_outline();
        if drill.len() >= 3 {
            painter.add(egui::Shape::convex_polygon(
                drill
                    .into_iter()
                    .map(|p| view.camera.screen(p, rect))
                    .collect(),
                Color32::from_rgb(210, 210, 190),
                Stroke::NONE,
            ));
        }
        match &item.shape {
            Shape::Pad {
                position,
                size,
                name,
                ..
            } if (view.pcb.display.pad_numbers || view.pcb.display.pad_nets)
                && (item.layer == view.pcb.active_layer || item.layer == MULTI_LAYER) =>
            {
                let height = (size.x.min(size.y) as f64 / 1e6 * view.camera.pixels_per_mm) as f32;
                let net = if view.pcb.display.pad_nets {
                    b.net_name(item.net)
                } else {
                    ""
                };
                let value = if view.pcb.display.pad_numbers {
                    name.as_str()
                } else {
                    net
                };
                text::label(
                    &painter,
                    view.camera.screen(*position, rect),
                    value,
                    height * 0.7,
                    height * 0.9,
                    Color32::from_rgb(255, 255, 200),
                );
            }
            Shape::Via {
                position, diameter, ..
            } if view.pcb.display.via_nets => {
                let height = (*diameter as f64 / 1e6 * view.camera.pixels_per_mm) as f32;
                text::label(
                    &painter,
                    view.camera.screen(*position, rect),
                    b.net_name(item.net),
                    height * 0.55,
                    height * 0.9,
                    Color32::from_rgb(255, 255, 200),
                );
            }
            _ => {}
        }
    }
    let mut hits = Vec::new();
    if let Some(p) = raw {
        let tol = 6.0 / view.camera.pixels_per_mm * 1e6;
        let mut candidates: Vec<_> = order
            .iter()
            .rev()
            .filter(|i| view.pcb.selectable(i))
            .filter_map(|i| {
                let d = i.hit_distance_on_layer(
                    p,
                    if i.layer == MULTI_LAYER {
                        view.pcb.active_layer
                    } else {
                        i.layer
                    },
                );
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
    navigation::paint_cursor(ui, &painter, &response, point, view, rect, panning);
    if scene.pours_stale {
        painter.text(
            rect.left_top() + Vec2::splat(8.0),
            egui::Align2::LEFT_TOP,
            "Polygon copper preview is stale; it is rebuilt when the edit is committed",
            egui::FontId::proportional(12.0),
            Color32::YELLOW,
        );
    }
    // Keep the press location across release, including fast drags whose
    // threshold and release arrive in one frame (egui clears press_origin then).
    let pressed = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => Some(*pos),
            _ => None,
        })
    });
    let suppressed = view.pcb.suppress_primary_until_release;
    if let Some(pos) =
        pressed.filter(|pos| !suppressed && rect.contains(*pos) && response.contains_pointer())
    {
        view.pcb.primary_origin = Some(world(&view.camera, pos, rect));
        view.pcb.primary_press = Some(pos);
        view.pcb.primary_dragging = false;
    }
    let origin = view.pcb.primary_origin;
    let was_dragging = view.pcb.primary_dragging;
    let (down, released, focused, pointer_pos) = ui.input(|i| {
        (
            i.pointer.primary_down(),
            i.pointer.button_released(egui::PointerButton::Primary),
            i.focused,
            i.pointer.latest_pos(),
        )
    });
    if let (Some(start), Some(end)) = (view.pcb.primary_press, pointer_pos)
        && start.distance(end) > 6.0
    {
        view.pcb.primary_dragging = true;
    }
    let dragging = view.pcb.primary_dragging;
    if !down {
        view.pcb.suppress_primary_until_release = false;
    }
    let interrupted = !focused || (panning && origin.is_some());
    if !down || interrupted {
        view.pcb.primary_origin = None;
        view.pcb.primary_press = None;
        view.pcb.primary_dragging = false;
    }
    view.pcb.input = Some(Input {
        point,
        raw_point: raw,
        press_origin: origin,
        hit,
        hits,
        clicked: response.clicked() && !dragging && !suppressed,
        double_clicked: response.double_clicked() && !dragging && !suppressed,
        secondary_clicked: response.secondary_clicked(),
        drag_started: dragging && !was_dragging && !interrupted,
        dragging: dragging && down && !interrupted,
        drag_stopped: dragging && released && !interrupted,
        hovered: response.hovered(),
        cancelled: interrupted,
        panning,
    });
    CanvasOutput {
        cursor_mm: point.map(|p| [p.x as f64 / 1e6, p.y as f64 / 1e6]),
        schematic_input: None,
    }
}
fn paint_masks(painter: &egui::Painter, board: &Board, view: &View, rect: Rect, active: bool) {
    for (layer, kind, bottom) in [
        (35, MaskKind::Paste, false),
        (36, MaskKind::Paste, true),
        (37, MaskKind::Solder, false),
        (38, MaskKind::Solder, true),
    ] {
        if !view.pcb.visible.contains(&layer)
            || (view.pcb.active_layer == layer) != active
            || (view.pcb.single_layer == 2 && view.pcb.active_layer != layer)
        {
            continue;
        }
        for item in board
            .items
            .iter()
            .filter(|i| matches!(i.shape, Shape::Pad { .. } | Shape::Via { .. }))
        {
            let mut appearance = item.clone();
            appearance.layer = layer;
            let alpha = view.pcb.display.alpha(&appearance);
            if alpha == 0 {
                continue;
            }
            if let Ok(outline) = item.mask_outline(board, kind, bottom)
                && outline.len() >= 3
            {
                painter.add(egui::Shape::convex_polygon(
                    outline
                        .into_iter()
                        .map(|p| view.camera.screen(p, rect))
                        .collect(),
                    dim(layer_color(layer), alpha),
                    Stroke::NONE,
                ));
            }
        }
    }
}

fn paint_item(
    p: &egui::Painter,
    camera: &Camera,
    rect: Rect,
    item: &Item,
    active: u16,
    color: Color32,
    cache: &mut HashMap<Uuid, (u64, Vec<usize>, bool)>,
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
        Shape::Via { .. } | Shape::Pad { .. } => {
            let layer = if item.layer == MULTI_LAYER {
                active
            } else {
                item.layer
            };
            let outline = item.pad_outline(layer);
            if outline.len() >= 3 {
                p.add(egui::Shape::convex_polygon(
                    outline.into_iter().map(screen).collect(),
                    color,
                    Stroke::NONE,
                ));
            }
            let drill = item.drill_outline();
            if drill.len() >= 3 {
                p.add(egui::Shape::convex_polygon(
                    drill.into_iter().map(screen).collect(),
                    Color32::BLACK,
                    Stroke::NONE,
                ));
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
            if *cutout || item.keepout || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION") {
                let outline: Vec<_> = contour.iter().copied().map(screen).collect();
                let holes: Vec<Vec<_>> = holes
                    .iter()
                    .map(|h| h.iter().copied().map(screen).collect())
                    .collect();
                for segment in hatch_segments(
                    &outline,
                    &holes,
                    p.clip_rect(),
                    item.properties.contains_key("PCBCRAFT_BODY_PROJECTION"),
                ) {
                    p.line_segment(segment, Stroke::new(0.55, color));
                }
                p.add(egui::Shape::closed_line(outline, Stroke::new(0.7, color)));
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
                let e = cache.entry(item.id).or_insert((signature, vec![], true));
                e.2 = true;
                if e.0 != signature {
                    e.0 = signature;
                    e.1.clear();
                }
                polygon(p, camera, rect, contour, holes, color, Some(&mut e.1));
            }
        }
        Shape::Text { position, .. } => {
            text::paint(p, item, screen(*position), camera.pixels_per_mm, color)
        }
    }
}
/// Screen-space hatch pattern clipped by even/odd intersections, including holes.
fn hatch_segments(
    contour: &[Pos2],
    holes: &[Vec<Pos2>],
    clip: Rect,
    cross: bool,
) -> Vec<[Pos2; 2]> {
    if contour.len() < 3 {
        return vec![];
    }
    let bounds = Rect::from_points(contour).intersect(clip);
    if !bounds.is_positive() {
        return vec![];
    }
    let mut out = Vec::new();
    for sign in [1.0, -1.0].into_iter().take(if cross { 2 } else { 1 }) {
        let u = |p: Pos2| p.x + sign * p.y;
        let uv = [
            bounds.left_top(),
            bounds.right_top(),
            bounds.right_bottom(),
            bounds.left_bottom(),
        ]
        .map(u);
        let low = uv.into_iter().fold(f32::INFINITY, f32::min);
        let high = uv.into_iter().fold(f32::NEG_INFINITY, f32::max);
        let step = 7.0;
        let mut at = (low / step).floor() * step;
        while at <= high {
            let mut hits = Vec::new();
            for ring in std::iter::once(contour).chain(holes.iter().map(Vec::as_slice)) {
                for (a, b) in ring
                    .iter()
                    .zip(ring.iter().cycle().skip(1))
                    .take(ring.len())
                {
                    let ua = u(*a);
                    let ub = u(*b);
                    if (ua <= at && at < ub) || (ub <= at && at < ua) {
                        let p = a.lerp(*b, (at - ua) / (ub - ua));
                        hits.push(p.x - sign * p.y);
                    }
                }
            }
            hits.sort_by(f32::total_cmp);
            for pair in hits.chunks_exact(2) {
                if pair[1] - pair[0] < 0.001 {
                    continue;
                }
                let point = |v: f32| Pos2::new((at + v) * 0.5, sign * (at - v) * 0.5);
                out.push([point(pair[0]), point(pair[1])]);
            }
            at += step;
        }
    }
    out
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_passes_pad_edge_snap_and_bypass_to_editor_without_editing_board() {
        let mut board = Board::default();
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::default(),
                size: Point::mm(2, 2),
                rotation: 0.0,
                shape: PadShape::Rectangle,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        ));
        let original = board.clone();
        let ctx = egui::Context::default();
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 100.0;
        view.camera.centre_mm = [0.0, 0.0];
        view.grid = false;
        let raw = Point {
            x: 1_100_000,
            y: 400_000,
        };
        let mut canvas_rect = Rect::NOTHING;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    canvas_rect = ui.available_rect_before_wrap();
                    canvas(ui, &board, &mut view);
                });
            },
        );
        let pointer = view.camera.screen(raw, canvas_rect);
        for (enabled, bypass) in [(true, false), (false, false), (true, true), (true, false)] {
            view.pcb.snap.pad_edges = enabled;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    focused: true,
                    modifiers: egui::Modifiers {
                        ctrl: bypass,
                        shift: bypass,
                        ..Default::default()
                    },
                    events: vec![egui::Event::PointerMoved(pointer)],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        canvas(ui, &board, &mut view);
                    });
                },
            );
            let input = view.pcb.input.as_ref().unwrap();
            assert_eq!(input.raw_point, Some(raw));
            assert_eq!(
                input.point,
                Some(if enabled && !bypass {
                    Point {
                        x: 1_000_000,
                        y: 400_000,
                    }
                } else {
                    raw
                })
            );
        }
        assert_eq!(board, original);
    }
    #[test]
    fn source_grid_applies_after_layer_bar_initialization_without_overriding_later_edits() {
        let mut board = Board::default();
        board
            .properties
            .insert("SNAPGRIDSIZE".into(), "10000.000000".into());
        let mut view = View::default();
        view.pcb.initialize(&board);
        let ctx = egui::Context::default();
        let draw = |view: &mut View| {
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    canvas(ui, &board, view);
                });
            });
        };
        draw(&mut view);
        assert_eq!(view.grid_nm, 25_400);
        view.grid_nm = 500_000;
        draw(&mut view);
        assert_eq!(view.grid_nm, 500_000);
    }
    #[test]
    fn native_arc_seams_never_create_unbounded_outline_triangles() {
        let board = Board {
            outline: vec![
                ContourVertex {
                    position: Point { x: 0, y: 2_000_000 },
                    arc: None,
                },
                ContourVertex {
                    position: Point {
                        x: 1_000_000,
                        y: 2_000_000,
                    },
                    arc: None,
                },
                ContourVertex {
                    position: Point {
                        x: 1_000_000,
                        y: 2_000_076,
                    },
                    arc: Some(ContourArc {
                        centre: Point {
                            x: 1_000_000,
                            y: 1_000_076,
                        },
                        radius: 1_000_000,
                        start_angle: 0.0,
                        end_angle: 90.0,
                    }),
                },
                ContourVertex {
                    position: Point {
                        x: 2_000_000,
                        y: 1_000_076,
                    },
                    arc: None,
                },
                ContourVertex {
                    position: Point {
                        x: 1_999_997,
                        y: 1_000_079,
                    },
                    arc: None,
                },
                ContourVertex {
                    position: Point { x: 2_000_000, y: 0 },
                    arc: None,
                },
                ContourVertex {
                    position: Point::default(),
                    arc: None,
                },
            ],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        for zoom in [6.73, 13.0, 15.31, 70.1] {
            let mut view = View::default();
            view.camera.fitted = true;
            view.camera.pixels_per_mm = zoom;
            view.camera.centre_mm = [1.0, 1.0];
            let o = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(500.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        canvas(ui, &board, &mut view);
                    });
                },
            );
            for p in ctx.tessellate(o.shapes, o.pixels_per_point) {
                if let egui::epaint::Primitive::Mesh(m) = p.primitive {
                    assert!(
                        m.vertices.iter().all(|v| v.pos.is_finite()
                            && v.pos.x.abs() < 1000.0
                            && v.pos.y.abs() < 1000.0),
                        "unbounded miter at zoom {zoom}"
                    );
                }
            }
        }
    }
    #[test]
    fn hatching_respects_concave_contours_and_holes() {
        let outline = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(100.0, 0.0),
            Pos2::new(100.0, 40.0),
            Pos2::new(40.0, 40.0),
            Pos2::new(40.0, 100.0),
            Pos2::new(0.0, 100.0),
        ];
        let hole = vec![
            Pos2::new(10.0, 10.0),
            Pos2::new(30.0, 10.0),
            Pos2::new(30.0, 30.0),
            Pos2::new(10.0, 30.0),
        ];
        let lines = hatch_segments(
            &outline,
            &[hole],
            Rect::from_min_max(Pos2::ZERO, Pos2::new(100.0, 100.0)),
            true,
        );
        assert!(!lines.is_empty());
        for [a, b] in lines {
            let mid = a.lerp(b, 0.5);
            assert!(!(mid.x > 40.0 && mid.y > 40.0));
            assert!(!(mid.x > 10.0 && mid.x < 30.0 && mid.y > 10.0 && mid.y < 30.0));
            assert!(a.distance(b) > 0.0);
        }
    }
    #[test]
    fn abandoned_region_previews_do_not_accumulate_cached_meshes() {
        let ctx = egui::Context::default();
        let mut view = BoardView::default();
        let camera = Camera::default();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(400.0));
        for _ in 0..100 {
            view.begin_frame();
            let item = Item::new(
                TOP,
                None,
                Shape::Region {
                    contour: vec![Point::mm(0, 0), Point::mm(1, 0), Point::mm(1, 1)],
                    holes: vec![],
                    cutout: false,
                },
            );
            let _ = ctx.run(Default::default(), |ctx| {
                paint_item(
                    &ctx.layer_painter(egui::LayerId::background()),
                    &camera,
                    rect,
                    &item,
                    TOP,
                    Color32::RED,
                    &mut view.cache,
                );
            });
            assert!(view.cache.len() <= 2);
            assert!(!view.cache[&item.id].1.is_empty());
        }
        view.begin_frame();
        view.begin_frame();
        assert!(view.cache.is_empty());
    }
}
