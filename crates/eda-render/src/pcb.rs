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
#[path = "pcb_layer_catalog.rs"]
mod catalog;
#[path = "pcb_configuration.rs"]
mod configuration;
#[path = "pcb_connections.rs"]
mod connections;
#[path = "pcb_display.rs"]
mod display;
#[path = "pcb_geometry_cache.rs"]
mod geometry;
#[path = "pcb_gpu.rs"]
mod gpu;
#[path = "pcb_paint_cache.rs"]
mod paint_cache;
pub(crate) use gpu::format as gpu_format;
pub use gpu::{enable_gpu, enable_gpu_with_device};
#[path = "pcb_navigation.rs"]
mod navigation;
#[path = "pcb_text.rs"]
mod text;
pub use navigation::{CursorStyle, FitTarget, NavigationOptions};
#[derive(Clone, Default)]
pub struct BoardView {
    pub three_d: crate::pcb_3d::State,
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
    preview: Option<std::sync::Arc<Board>>,
    preview_generation: u64,
    pub tool_active: bool,
    pub selection_box: Option<(Point, Point)>,
    pub draft_outline: Vec<Point>,
    pub input: Option<Input>,
    pub show_layer_dialog: bool,
    pub show_grid_dialog: bool,
    pub polygons_visible: bool,
    pub repour_pending: bool,
    pub drc_markers: Vec<(Point, u16)>,
    cache: HashMap<Uuid, (u64, Vec<usize>, bool)>,
    geometry: geometry::Cache,
    display: display::Settings,
    grid_initialized: bool,
    connections: connections::Cache,
    paint_cache: paint_cache::Cache,
}
#[derive(Clone, Default)]
pub struct Input {
    /// Apply through the editor's layer-change command, including route vias.
    pub requested_layer: Option<u16>,
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
        35 => Color32::from_rgb(170, 170, 170),
        36 | 55 => Color32::RED,
        37 => Color32::from_rgb(128, 0, 128),
        38 => Color32::MAGENTA,
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
            if n == 16 {
                return Color32::BLACK;
            }
            if n == 17 {
                return Color32::from_rgb(128, 128, 0);
            }
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
    pub fn preview_generation(&self) -> u64 {
        self.preview_generation
    }
    pub fn set_preview(&mut self, preview: Option<Board>) {
        if self.preview.as_deref() == preview.as_ref() {
            return;
        }
        self.preview_generation = self.preview_generation.wrapping_add(1);
        self.preview = preview.map(std::sync::Arc::new);
    }
    pub fn preview(&self) -> Option<&Board> {
        self.preview.as_deref()
    }
    pub fn take_preview(&mut self) -> Option<Board> {
        self.preview.take().map(|board| {
            self.preview_generation = self.preview_generation.wrapping_add(1);
            std::sync::Arc::unwrap_or_clone(board)
        })
    }
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
        if let Some(range) = b
            .properties
            .get("ELECTRICALGRIDRANGE")
            .and_then(|v| dimension_nm(v).ok())
            .filter(|v| *v > 0)
        {
            self.snap.distance = range;
        }
        if b.properties
            .get("ELECTRICALGRIDENABLED")
            .is_some_and(|v| v.eq_ignore_ascii_case("FALSE"))
        {
            self.snap.mode = SnapLayerMode::Off;
        } else if let Some(all) = b.properties.get("ELECTRICALGRIDUSEALLLAYERS") {
            self.snap.mode = if all.eq_ignore_ascii_case("TRUE") {
                SnapLayerMode::All
            } else {
                SnapLayerMode::Current
            };
        }
        for (key, target) in [
            ("EGSNAPTOARCCENTERS", &mut self.snap.arc_centres),
            ("EGSNAPTOBOARDOUTLINE", &mut self.snap.board_vertices),
        ] {
            if let Some(value) = b.properties.get(key) {
                *target = value.eq_ignore_ascii_case("TRUE");
            }
        }
        self.three_d.read_configuration(b);
        if let Some(raw) = b.properties.get("2DCONFIGURATION") {
            self.single_layer = raw
                .split('`')
                .find_map(|s| s.strip_prefix("CFG2D.SINGLELAYERMODESTATE="))
                .and_then(|s| s.trim().parse::<u8>().ok())
                // AD 24.4.1 stores the normal multicolour view as state 3
                // (observed in ReferenceA). Clamping it to 2 hid every
                // inactive layer immediately after importing the board.
                .filter(|state| *state < 3)
                .unwrap_or(0);
        }
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
            && self.display.alpha(item) > 0
            && self.visible.contains(&item.layer)
            && (self.single_layer != 2
                || item.layer == self.active_layer
                || item.layer == MULTI_LAYER)
            && (self.polygons_visible || item.polygon.is_none())
    }
    fn polygon_visible(&self, layer: u16) -> bool {
        self.polygons_visible
            && !self.display.hidden[10]
            && self.visible.contains(&layer)
            && self
                .display
                .opacity
                .get(&layer)
                .is_none_or(|values| values[10] > 0)
            && (self.single_layer != 2 || layer == self.active_layer)
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
    /// Picking a footprint primitive promotes it to its owning component. Use
    /// the owner's mounting side, not the primitive's layer (a through-hole pad
    /// or body projection can otherwise select a component on the far side).
    pub fn selectable_on_board(&self, board: &Board, item: &Item) -> bool {
        if !self.selectable(item) {
            return false;
        }
        if self.component_selection
            && let Some(owner) = item
                .component
                .and_then(|index| board.components.iter().find(|c| c.index == index))
        {
            if owner.locked && !self.select_locked {
                return false;
            }
            if matches!(self.active_layer, TOP | BOTTOM)
                && matches!(owner.layer, TOP | BOTTOM)
                && owner.layer != self.active_layer
            {
                return false;
            }
        }
        true
    }
    pub fn pick_layer_rank(&self, item: &Item) -> u8 {
        if item.layer == self.active_layer {
            0
        } else if item.layer == MULTI_LAYER {
            1
        } else {
            2
        }
    }
    pub fn clear_gesture(&mut self) {
        self.clear_pointer();
        self.tool_active = false;
    }
    pub fn clear_pointer(&mut self) {
        self.suppress_primary_until_release = true;
        self.set_preview(None);
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
    // The first egui sizing pass can leave no canvas after the dock panels.
    // Defer fitting until a real viewport exists instead of retaining a
    // minimum-scale view for the rest of the document's lifetime.
    if !rect.is_finite() || rect.width() <= 60.0 || rect.height() <= 60.0 {
        return;
    }
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
    if alpha == 255 {
        return c;
    }
    if alpha == 0 {
        return Color32::TRANSPARENT;
    }
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, (u16::from(a) * u16::from(alpha) / 255) as u8)
}
pub fn canvas(ui: &mut egui::Ui, b: &Board, view: &mut View) -> CanvasOutput {
    view.pcb.initialize(b);
    if view.pcb.three_d.enabled {
        return crate::pcb_3d::canvas(ui, b, view);
    }
    if !view.pcb.grid_initialized {
        view.pcb.grid_initialized = true;
        if let Some(grid) = view.pcb.display.snap_grid_nm {
            view.grid_nm = grid;
        }
    }
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let panning = navigation::navigate(ui, &response, &mut view.camera, b, &mut view.pcb, rect);
    let painter = ui.painter_at(rect);
    let mut background = egui::Mesh::default();
    for (pos, color) in [
        (rect.left_top(), view.pcb.display.background),
        (rect.right_top(), view.pcb.display.background),
        (rect.right_bottom(), view.pcb.display.background_end),
        (rect.left_bottom(), view.pcb.display.background_end),
    ] {
        background.colored_vertex(pos, color);
    }
    background.add_triangle(0, 1, 2);
    background.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(background));
    // Release abandoned preview/deleted/hidden regions after one unused frame,
    // including large meshes that would remain below the old entry-count cap.
    view.pcb.begin_frame();
    if view.pcb.display.connections {
        view.pcb.connections.update(
            b,
            view.pcb.preview.as_deref(),
            view.pcb.three_d.revision,
            view.pcb.preview_generation,
            ui.ctx(),
        );
    }
    let preview = view.pcb.preview.clone();
    let scene = preview.as_deref().unwrap_or(b);
    view.pcb.geometry.prepare(
        scene,
        view.pcb.three_d.revision,
        view.pcb.preview_generation,
    );
    let key = paint_cache::key(scene, view, ui.ctx());
    let mut artwork = std::mem::take(&mut view.pcb.paint_cache);
    if !artwork.valid(key) {
        artwork.rebuild(&painter, scene, view, key);
    } else {
        artwork.refresh(view.camera.pixels_per_mm, ui.ctx());
    }
    artwork.paint(&painter, scene, view, rect);
    view.pcb.paint_cache = artwork;
    let pointer = response.hover_pos();
    let raw = pointer.map(|p| world(&view.camera, p, rect));
    let bypass = ui.input(|i| i.modifiers.ctrl && i.modifiers.shift);
    let point = raw.map(|p| {
        if bypass {
            p
        } else if preview.is_some() {
            b.snap_with_options(
                p,
                if view.grid { view.grid_nm } else { 0 },
                view.pcb.active_layer,
                &view.pcb.visible,
                &view.pcb.snap,
            )
        } else {
            let near = Bounds::points(&[p]).expanded(view.pcb.snap.distance.max(0));
            let candidates = view.pcb.geometry.query(near);
            b.snap_with_items(
                p,
                if view.grid { view.grid_nm } else { 0 },
                view.pcb.active_layer,
                &view.pcb.visible,
                &view.pcb.snap,
                candidates.into_iter().map(|index| &b.items[index]),
            )
        }
    });
    let mut hits = Vec::new();
    if let Some(p) = raw {
        let tol = 6.0 / view.camera.pixels_per_mm * 1e6;
        let near = Bounds::points(&[p]).expanded(tol.ceil() as i64);
        let mut near_items = view.pcb.geometry.query(near);
        near_items.retain(|&index| {
            let i = &scene.items[index];
            view.pcb.is_visible(i)
                && scene.text_visible(i)
                && (view.pcb.single_layer == 0
                    || i.layer != MULTI_LAYER
                    || i.is_drill_only()
                    || eda_core::pcb_polygon::on_layer(scene, i, view.pcb.active_layer))
        });
        near_items.sort_by_key(|&index| {
            (
                paint_order(&scene.items[index], view.pcb.active_layer),
                index,
            )
        });
        let mut candidates: Vec<_> = near_items
            .into_iter()
            .rev()
            .map(|index| &scene.items[index])
            .filter(|i| view.pcb.geometry.bounds[&i.id].intersects(near))
            .filter(|i| {
                if view.cross_probe {
                    view.pcb.is_visible(i)
                } else {
                    view.pcb.selectable_on_board(scene, i)
                }
            })
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
                    view.pcb.pick_layer_rank(i),
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
        candidates.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then(a.2.cmp(&b.2))
                .then_with(|| a.3.total_cmp(&b.3))
        });
        hits.extend(candidates.into_iter().map(|c| c.0));
        if view.pcb.filters[7] {
            for poly in scene.polygon_pours.iter().rev() {
                if !view.pcb.polygon_visible(poly.layer) || (poly.locked && !view.pcb.select_locked)
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
    if view.pcb.display.status_info
        && let Some(item) = hit.and_then(|id| b.item(id))
    {
        let kind = configuration::OBJECTS
            .iter()
            .find(|(kind, _)| *kind == display::Settings::kind(item))
            .map_or("Object", |(_, name)| *name);
        response.clone().on_hover_ui(|ui| {
            ui.label(format!("{} · {}", kind, layer_name(item.layer)));
            let net = b.net_name(item.net);
            if !net.is_empty() {
                ui.label(format!("Net: {net}"));
            }
        });
    }
    for (position, layer) in &view.pcb.drc_markers {
        if *layer == MULTI_LAYER || view.pcb.visible.contains(layer) {
            let p = view.camera.screen(*position, rect);
            if rect.contains(p) {
                let stroke = Stroke::new(1.5, Color32::from_rgb(0, 255, 80));
                painter.circle_stroke(p, 7.0, stroke);
                painter.line_segment([p - Vec2::splat(5.0), p + Vec2::splat(5.0)], stroke);
            }
        }
    }
    navigation::paint_cursor(ui, &painter, &response, pointer, view, rect, panning);
    if scene.pours_stale {
        painter.text(
            rect.left_top() + Vec2::splat(8.0),
            egui::Align2::LEFT_TOP,
            if view.pcb.repour_pending {
                "Repouring polygons…"
            } else {
                "Polygon copper is out of date; run Repour All Polygons"
            },
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
        requested_layer: navigation::requested_layer(ui, &response, &view.pcb),
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
fn paint_order(i: &Item, active: u16) -> (u8, u16, u8) {
    (
        if i.layer == active {
            1
        } else if i.layer == MULTI_LAYER {
            2
        } else if i.layer == TOP_OVERLAY || i.layer == BOTTOM_OVERLAY {
            3
        } else {
            0
        },
        i.layer,
        if matches!(i.shape, Shape::Region { .. } | Shape::Fill { .. }) {
            0
        } else {
            1
        },
    )
}
fn paint_scene(
    painter: &egui::Painter,
    scene: &Board,
    camera: &Camera,
    view: &mut View,
    rect: Rect,
    builder: &mut paint_cache::Builder,
) {
    let b = scene;
    let outline = view.pcb.geometry.outline.clone();
    if outline.len() >= 3 && view.pcb.display.board_visible {
        polygon(
            painter,
            camera,
            rect,
            &outline,
            &[],
            view.pcb.display.board_color,
            Some(&mut view.pcb.geometry.board_triangles),
        );
        // Native arc/line seams can backtrack by a few nanometres. A joined
        // stroked path can produce an unbounded antialias miter at that seam.
        for (a, z) in outline
            .iter()
            .zip(outline.iter().cycle().skip(1))
            .take(outline.len())
        {
            let a = camera.screen(*a, rect);
            let z = camera.screen(*z, rect);
            if a.distance(z) > 0.001 {
                painter.line_segment([a, z], Stroke::new(1.0, view.pcb.display.board_line));
            }
        }
    }
    builder.grid(painter);
    let selected = scene.resolve_selection(&view.pcb.selected);
    let mut order: Vec<_> = scene
        .items
        .iter()
        .filter(|i| {
            view.pcb.is_visible(i)
                && scene.text_visible(i)
                && (view.pcb.single_layer == 0
                    || i.layer != MULTI_LAYER
                    || i.is_drill_only()
                    || eda_core::pcb_polygon::on_layer(scene, i, view.pcb.active_layer))
        })
        .collect();
    order.sort_by_key(|i| paint_order(i, view.pcb.active_layer));
    paint_masks(painter, scene, camera, view, rect, false);
    for item in &order {
        let mut color = dim(view.pcb.color(item.layer), view.pcb.display.alpha(item));
        if view.pcb.single_layer == 1
            && item.layer != view.pcb.active_layer
            && item.layer != MULTI_LAYER
        {
            color = dim(color, view.pcb.display.dim);
        }
        if let Some(net) = view.pcb.highlight_net {
            color = if item.net == Some(net) {
                dim(
                    view.pcb.display.highlight_color,
                    view.pcb.display.highlight_dim,
                )
            } else {
                dim(color, view.pcb.display.masked_dim)
            };
        }
        if selected.contains(&item.id) {
            color = view.pcb.display.selection_color;
        }
        if scene.pours_stale && item.polygon.is_some() {
            color = dim(color, view.pcb.display.dim);
        }
        if view.pcb.display.draft[display::Settings::kind(item)]
            && paint_draft(painter, camera, rect, item, view.pcb.active_layer, color)
        {
            continue;
        }
        if matches!(item.shape, Shape::Region { cutout: true, .. })
            || (matches!(item.shape, Shape::Region { .. })
                && (item.keepout || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")))
        {
            builder.hatch(painter, item.id, color);
            continue;
        }
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
    if view.pcb.display.connections {
        let active_nets: BTreeSet<_> = scene
            .items
            .iter()
            .filter(|i| i.layer == view.pcb.active_layer || i.layer == MULTI_LAYER)
            .filter_map(|i| i.net)
            .collect();
        for connection in view.pcb.connections.lines.iter() {
            if view.pcb.single_layer != 0
                && !view.pcb.display.all_connections
                && !active_nets.contains(&connection.net)
            {
                continue;
            }
            let color = if view
                .pcb
                .highlight_net
                .is_some_and(|net| net != connection.net)
            {
                dim(view.pcb.display.connection_color, view.pcb.display.dim)
            } else if view.pcb.display.layer_connections {
                view.pcb.color(view.pcb.active_layer)
            } else {
                view.pcb.display.connection_color
            };
            painter.line_segment(
                [
                    camera.screen(connection.start, rect),
                    camera.screen(connection.end, rect),
                ],
                Stroke::new(1.0, color),
            );
        }
    }
    for poly in &scene.polygon_pours {
        if !view.pcb.polygon_visible(poly.layer) {
            continue;
        }
        let selected = view.pcb.selected.contains(&poly.id);
        let color = if selected {
            Color32::WHITE
        } else {
            dim(
                view.pcb.color(poly.layer),
                if poly.shelved { 80 } else { 140 },
            )
        };
        let points = poly.points();
        if points.len() > 2 {
            painter.add(egui::Shape::closed_line(
                points.iter().map(|p| camera.screen(*p, rect)).collect(),
                Stroke::new(if selected { 2.0 } else { 0.7 }, color),
            ));
        }
        if selected {
            for hole in &poly.holes {
                painter.add(egui::Shape::closed_line(
                    hole.iter().map(|p| camera.screen(*p, rect)).collect(),
                    Stroke::new(1.5, color),
                ));
            }
        }
    }
    builder.tools(painter);
    paint_masks(painter, scene, camera, view, rect, true);
    for item in &order {
        let drill = view.pcb.geometry.drill(item);
        let show_drill = if matches!(item.shape, Shape::Via { .. }) {
            view.pcb.display.via_holes_visible
        } else {
            view.pcb.display.pad_holes_visible
        };
        if drill.len() >= 3 && show_drill {
            painter.add(egui::Shape::convex_polygon(
                drill
                    .iter()
                    .copied()
                    .map(|p| camera.screen(p, rect))
                    .collect(),
                if matches!(item.shape, Shape::Via { .. }) {
                    view.pcb.display.via_hole_color
                } else {
                    view.pcb.display.hole_color
                },
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
                let height = (size.x.min(size.y) as f64 / 1e6 * camera.pixels_per_mm) as f32;
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
                let both = view.pcb.display.pad_numbers && !net.is_empty();
                let center = camera.screen(*position, rect);
                if both {
                    text::label(
                        painter,
                        center + Vec2::new(0.0, height * 0.23),
                        net,
                        height * 0.35,
                        height * 0.9,
                        Color32::from_rgb(255, 255, 200),
                    );
                }
                text::label(
                    painter,
                    center - Vec2::new(0.0, if both { height * 0.23 } else { 0.0 }),
                    value,
                    height * if both { 0.35 } else { 0.7 },
                    height * 0.9,
                    Color32::from_rgb(255, 255, 200),
                );
            }
            Shape::Via {
                position, diameter, ..
            } if view.pcb.display.via_nets => {
                let height = (*diameter as f64 / 1e6 * camera.pixels_per_mm) as f32;
                text::label(
                    painter,
                    camera.screen(*position, rect),
                    b.net_name(item.net),
                    height * 0.55,
                    height * 0.9,
                    Color32::from_rgb(255, 255, 200),
                );
            }
            Shape::Track { start, end, width }
                if view.pcb.display.track_names && item.net.is_some() =>
            {
                let a = camera.screen(*start, rect);
                let z = camera.screen(*end, rect);
                let height = (*width as f64 / 1e6 * camera.pixels_per_mm) as f32;
                if a.distance(z) > 45.0 && height > 4.0 {
                    text::track_labels(
                        painter,
                        a,
                        z,
                        b.net_name(item.net),
                        height * 0.6,
                        view.pcb.display.highlight_color,
                    );
                }
            }
            _ => {}
        }
    }
    builder.origin(painter);
}
fn paint_origin(painter: &egui::Painter, scene: &Board, view: &View, rect: Rect) {
    if view.pcb.display.origin_visible {
        let coordinate = |key| {
            scene
                .properties
                .get(key)
                .and_then(|v| dimension_nm(v).ok())
                .unwrap_or(0)
        };
        let c = view.camera.screen(
            Point {
                x: coordinate("ORIGINX"),
                y: coordinate("ORIGINY"),
            },
            rect,
        );
        if rect.expand(14.).contains(c) {
            painter.circle_stroke(c, 9.0, Stroke::new(0.7, view.pcb.display.origin_color));
            for d in [Vec2::new(14.0, 0.0), Vec2::new(0.0, 14.0)] {
                painter.line_segment(
                    [c - d, c + d],
                    Stroke::new(0.7, view.pcb.display.origin_color),
                );
            }
        }
    }
}
fn paint_grid(painter: &egui::Painter, view: &View, rect: Rect) {
    if !view.grid {
        return;
    }
    let mut step = view.grid_nm.max(1) as f64 / 1e6;
    while step * view.camera.pixels_per_mm < 14.0 {
        step *= 5.0;
    }
    let clip = painter.clip_rect().expand(1.);
    let a = view.camera.world_mm(clip.left_bottom(), rect);
    let b = view.camera.world_mm(clip.right_top(), rect);
    let min = [a[0].min(b[0]), a[1].min(b[1])];
    let max = [a[0].max(b[0]), a[1].max(b[1])];
    let sx = (min[0] / step).floor() * step;
    let sy = (min[1] / step).floor() * step;
    let cols = ((max[0] - min[0]) / step).ceil() as usize + 2;
    let rows = ((max[1] - min[1]) / step).ceil() as usize + 2;
    let outline = &view.pcb.geometry.outline;
    for row in 0..rows.min(500) {
        let y = ((sy + row as f64 * step) * 1e6) as i64;
        // Intersect the outline once per row, rather than scanning every edge
        // for every grid dot. Keep the same even/odd rule as point_in_polygon.
        let crossings = grid_crossings(outline, y);
        for col in 0..cols.min(500) {
            let x = ((sx + col as f64 * step) * 1e6) as i64;
            if crossings.partition_point(|crossing| *crossing <= x as f64) % 2 == 1 {
                painter.circle_filled(
                    view.camera.screen(Point { x, y }, rect),
                    0.65,
                    view.pcb.display.grid_color,
                );
            }
        }
    }
}
fn grid_crossings(outline: &[Point], y: i64) -> Vec<f64> {
    let mut crossings: Vec<_> = outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .take(outline.len())
        .filter(|(a, b)| (a.y > y) != (b.y > y))
        .map(|(a, b)| (b.x - a.x) as f64 * (y - a.y) as f64 / (b.y - a.y) as f64 + a.x as f64)
        .collect();
    crossings.sort_by(f64::total_cmp);
    crossings
}
fn paint_tools(painter: &egui::Painter, scene: &Board, view: &View, rect: Rect) {
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
}
fn paint_masks(
    painter: &egui::Painter,
    board: &Board,
    camera: &Camera,
    view: &mut View,
    rect: Rect,
    active: bool,
) {
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
            let alpha = view.pcb.display.alpha_on_layer(item, layer);
            if alpha == 0 {
                continue;
            }
            let outline = view.pcb.geometry.mask(board, item, kind, bottom);
            if outline.len() >= 3 {
                painter.add(egui::Shape::convex_polygon(
                    outline
                        .iter()
                        .copied()
                        .map(|p| camera.screen(p, rect))
                        .collect(),
                    dim(view.pcb.color(layer), alpha),
                    Stroke::NONE,
                ));
            }
        }
    }
}

fn paint_draft(
    p: &egui::Painter,
    camera: &Camera,
    rect: Rect,
    item: &Item,
    active: u16,
    color: Color32,
) -> bool {
    let screen = |point| camera.screen(point, rect);
    let ring = |points: Vec<Point>| {
        p.add(egui::Shape::closed_line(
            points.into_iter().map(screen).collect(),
            Stroke::new(1.0, color),
        ))
    };
    match &item.shape {
        Shape::Track { start, end, width } => {
            let length = distance_sq(*start, *end).sqrt() as i64;
            let angle = ((end.y - start.y) as f64)
                .atan2((end.x - start.x) as f64)
                .to_degrees();
            ring(eda_core::pcb_polygon::pad_outline_radius(
                midpoint(*start, *end),
                Point {
                    x: length + width,
                    y: *width,
                },
                angle,
                PadShape::Round,
                100,
            ));
        }
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            for r in [radius - width / 2, radius + width / 2] {
                if r > 0 {
                    p.add(egui::Shape::line(
                        arc_points(*centre, r, *start_angle, *end_angle, 128)
                            .into_iter()
                            .map(screen)
                            .collect(),
                        Stroke::new(1.0, color),
                    ));
                }
            }
        }
        Shape::Pad { .. } | Shape::Via { .. } => {
            let outline = item.pad_outline(if item.layer == MULTI_LAYER {
                active
            } else {
                item.layer
            });
            if outline.len() >= 3 {
                ring(outline);
            }
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => {
            ring(rectangle(
                midpoint(*start, *end),
                Point {
                    x: (end.x - start.x).abs(),
                    y: (end.y - start.y).abs(),
                },
                *rotation,
            ));
        }
        Shape::Region { contour, holes, .. } => {
            ring(contour.clone());
            for hole in holes {
                ring(hole.clone());
            }
        }
        Shape::Text { .. } => return false,
    }
    true
}
fn paint_item(
    p: &egui::Painter,
    camera: &Camera,
    rect: Rect,
    item: &Item,
    active: u16,
    color: Color32,
    view: &mut BoardView,
) {
    let cache = &mut view.cache;
    let geometry = &mut view.geometry;
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
            let outline = geometry.pad(item, layer);
            if outline.len() >= 3 {
                p.add(egui::Shape::convex_polygon(
                    outline.iter().copied().map(screen).collect(),
                    color,
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
    let triangulate = || {
        let coords: Vec<f64> = vertices
            .iter()
            .flat_map(|v| [v.x as f64 / 1e6, v.y as f64 / 1e6])
            .collect();
        earcutr::earcut(&coords, &hole_indices, 2).unwrap_or_default()
    };
    let own;
    let indices = if let Some(c) = cache {
        if c.is_empty() {
            *c = triangulate();
        }
        c.as_slice()
    } else {
        own = triangulate();
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
    fn component_selection_uses_mounting_side_even_for_multilayer_pads() {
        let mut board = Board::default();
        for (index, layer) in [(0, TOP), (1, BOTTOM)] {
            board.components.push(Component {
                id: Uuid::new_v4(),
                index,
                designator: format!("U{index}"),
                comment: String::new(),
                footprint: String::new(),
                position: Point::default(),
                rotation: 0.0,
                layer,
                locked: false,
                properties: Fields::new(),
            });
            let mut pad = Item::new(
                MULTI_LAYER,
                None,
                Shape::Pad {
                    position: Point::default(),
                    size: Point::mm(2, 2),
                    rotation: 0.0,
                    shape: PadShape::Round,
                    hole: 500_000,
                    plated: true,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            pad.component = Some(index);
            board.items.push(pad);
        }
        let mut view = BoardView::default();
        view.initialize(&board);
        for active in [TOP, BOTTOM] {
            view.active_layer = active;
            for item in &board.items {
                assert!(view.is_visible(item));
                assert_eq!(
                    view.selectable_on_board(&board, item),
                    board.components[item.component.unwrap() as usize].layer == active
                );
            }
        }
        let ctx = egui::Context::default();
        let mut canvas_view = View::default();
        canvas_view.camera.fitted = true;
        canvas_view.camera.pixels_per_mm = 100.0;
        canvas_view.pcb.initialize(&board);
        for active in [TOP, BOTTOM] {
            canvas_view.pcb.active_layer = active;
            let mut pointer = Pos2::ZERO;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pointer = canvas_view
                            .camera
                            .screen(Point::default(), ui.available_rect_before_wrap());
                        canvas(ui, &board, &mut canvas_view);
                    });
                },
            );
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    events: vec![egui::Event::PointerMoved(pointer)],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        canvas(ui, &board, &mut canvas_view);
                    });
                },
            );
            let expected = board.items[usize::from(active == BOTTOM)].id;
            assert_eq!(canvas_view.pcb.input.as_ref().unwrap().hits, vec![expected]);
        }
        view.component_selection = false;
        assert!(
            board
                .items
                .iter()
                .all(|i| view.selectable_on_board(&board, i))
        );
        view.component_selection = true;
        board.components[1].locked = true;
        view.select_locked = false;
        assert!(!view.selectable_on_board(&board, &board.items[1]));
    }
    #[test]
    fn startup_fit_waits_for_canvas_after_docks_are_measured() {
        let board = Board::default();
        let mut camera = Camera::default();
        fit(
            &mut camera,
            &board,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(0.0, 500.0)),
        );
        assert!(!camera.fitted);
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 600.0));
        fit(&mut camera, &board, viewport);
        assert!(camera.fitted);
        assert!(camera.pixels_per_mm > 1.0);
        let scale = camera.pixels_per_mm;
        fit(&mut camera, &board, Rect::NOTHING);
        assert_eq!(camera.pixels_per_mm, scale);
    }
    #[test]
    fn native_hotspot_range_and_layer_mode_apply_once_without_editing_board() {
        let mut board = Board::default();
        for (key, value) in [
            ("ELECTRICALGRIDRANGE", "1.9685mil"),
            ("ELECTRICALGRIDUSEALLLAYERS", "TRUE"),
            ("ELECTRICALGRIDENABLED", "TRUE"),
            ("EGSNAPTOBOARDOUTLINE", "TRUE"),
        ] {
            board.properties.insert(key.into(), value.into());
        }
        board.items.push(Item::new(
            BOTTOM,
            None,
            Shape::Track {
                start: Point::default(),
                end: Point::mm(1, 0),
                width: 100_000,
            },
        ));
        let before = board.clone();
        let mut view = BoardView::default();
        view.initialize(&board);
        assert_eq!(view.snap.distance, 50_000);
        assert_eq!(view.snap.mode, SnapLayerMode::All);
        assert!(view.snap.board_vertices);
        let raw = Point { x: 40_000, y: 0 };
        assert_eq!(
            board.snap_with_options(raw, 0, TOP, &view.visible, &view.snap),
            Point::default()
        );
        let outside = Point { x: 100_000, y: 0 };
        assert_eq!(
            board.snap_with_options(outside, 0, TOP, &view.visible, &view.snap),
            outside
        );
        view.snap.distance = 10_000;
        view.snap.mode = SnapLayerMode::Off;
        view.initialize(&board);
        assert_eq!(view.snap.distance, 10_000);
        assert_eq!(view.snap.mode, SnapLayerMode::Off);
        assert_eq!(board, before);
    }
    #[test]
    fn native_normal_view_does_not_hide_inactive_layers() {
        let mut board = Board::default();
        let bottom = Item::new(
            BOTTOM,
            None,
            Shape::Track {
                start: Point::default(),
                end: Point::mm(1, 0),
                width: 100_000,
            },
        );
        for (native, expected, bottom_visible) in [
            ("3", 0, true),
            ("2", 2, false),
            ("1", 1, true),
            ("0", 0, true),
            ("invalid", 0, true),
        ] {
            board.properties.insert(
                "2DCONFIGURATION".into(),
                format!("CFG2D.SINGLELAYERMODESTATE={native}"),
            );
            let before = board.clone();
            let mut view = BoardView::default();
            view.initialize(&board);
            assert_eq!(view.single_layer, expected);
            assert_eq!(view.is_visible(&bottom), bottom_visible);
            assert_eq!(board, before);
        }
    }
    #[test]
    fn dimming_composes_with_object_transparency_without_double_premultiplying_rgb() {
        let original = Color32::from_rgba_unmultiplied(150, 50, 20, 204);
        let dimmed = dim(original, 153);
        assert_eq!(dimmed.a(), 122);
        let rgb = dimmed.to_srgba_unmultiplied();
        assert!((i16::from(rgb[0]) - 150).abs() < 3);
        assert_eq!(dim(original, 0), Color32::TRANSPARENT);
    }
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
                    &mut view,
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
