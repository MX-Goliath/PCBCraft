//! Cached full-board software 3D viewport with per-pixel depth and object picking.
use crate::{
    CanvasOutput, View,
    pcb::Input,
    pcb_stack::{Triangle, shade, surface, wall},
};
use eda_core::{pcb::*, pcb_3d};
use eda_geom::Point;
use egui::{Color32, Pos2, Rect, Sense, Vec2};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone)]
struct Face {
    triangle: Triangle,
    owner: Option<Uuid>,
}
#[derive(Default)]
struct Scene {
    parents: std::collections::HashMap<Uuid, Uuid>,
    faces: Vec<Face>,
    centre: [f32; 3],
    radius: f32,
}
#[derive(Clone)]
pub struct State {
    pub enabled: bool,
    pub revision: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: Vec2,
    focus: Option<[f32; 3]>,
    drag: Option<(Pos2, egui::PointerButton, bool)>,
    pub show_components: bool,
    scene: Arc<Scene>,
    built_revision: Option<u64>,
    texture: Option<egui::TextureHandle>,
    raster_key: Option<u64>,
    ids: Arc<Vec<u32>>,
    size: [usize; 2],
}
impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            revision: 0,
            yaw: -0.5,
            pitch: 0.85,
            zoom: 1.,
            pan: Vec2::ZERO,
            focus: None,
            drag: None,
            show_components: true,
            scene: Arc::default(),
            built_revision: None,
            texture: None,
            raster_key: None,
            ids: Arc::default(),
            size: [1, 1],
        }
    }
}
impl State {
    pub fn cancel_drag(&mut self) {
        self.drag = None;
    }
    pub fn fit(&mut self) {
        self.focus = None;
        self.zoom = 1.;
        self.pan = Vec2::ZERO;
    }
    pub fn top(&mut self) {
        self.yaw = 0.;
        self.pitch = std::f32::consts::FRAC_PI_2;
        self.fit();
    }
    pub fn bottom(&mut self) {
        self.yaw = 0.;
        self.pitch = -std::f32::consts::FRAC_PI_2;
        self.fit();
    }
}
fn rings(faces: &mut Vec<Triangle>, outer: &[Point], holes: &[Vec<Point>], z: f32, color: Color32) {
    let mut coords = vec![];
    let mut indices = vec![];
    for (n, ring) in std::iter::once(outer)
        .chain(holes.iter().map(Vec::as_slice))
        .enumerate()
    {
        if n > 0 {
            indices.push(coords.len() / 2);
        }
        for p in ring {
            coords.extend([p.x as f64 / 1e6, p.y as f64 / 1e6]);
        }
    }
    if let Ok(tris) = earcutr::earcut(&coords, &indices, 2) {
        for t in tris.chunks_exact(3) {
            faces.push(Triangle {
                v: [t[0], t[1], t[2]].map(|i| [coords[i * 2] as f32, coords[i * 2 + 1] as f32, z]),
                color,
            });
        }
    }
}
fn build(board: &Board, components: bool) -> Scene {
    let mut scene = Scene::default();
    let origin = Point::default();
    let board_outline = board.outline_points(256);
    let board_bounds = Bounds::points(&board_outline);
    let thickness = board.stack_thickness_nm() as f32 / 1e6;
    let mask = Color32::from_rgb(28, 106, 64);
    let cuts = pcb_3d::Cuts::new(board);
    let mut add = |triangles: Vec<Triangle>, owner| {
        scene.faces.extend(
            triangles
                .into_iter()
                .map(|triangle| Face { triangle, owner }),
        )
    };
    for region in pcb_3d::substrate(board) {
        let mut faces = vec![];
        rings(&mut faces, &region.outline, &region.holes, 0., mask);
        rings(&mut faces, &region.outline, &region.holes, -thickness, mask);
        wall(
            &mut faces,
            &region.outline,
            -thickness,
            0.,
            Color32::from_rgb(139, 128, 70),
            origin,
        );
        for hole in &region.holes {
            wall(
                &mut faces,
                hole,
                -thickness,
                0.,
                Color32::from_rgb(132, 128, 110),
                origin,
            );
        }
        add(faces, None);
    }
    for item in &board.items {
        if item.keepout
            || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
            || (item.component.is_none() && !point_in_polygon(item.anchor(), &board_outline))
        {
            continue;
        }
        for (layer, z) in [(TOP, 0.015), (BOTTOM, -thickness - 0.015)] {
            if item.layer != layer
                && item.layer != MULTI_LAYER
                && item.layer
                    != if layer == TOP {
                        TOP_OVERLAY
                    } else {
                        BOTTOM_OVERLAY
                    }
            {
                continue;
            }
            let mut faces = vec![];
            let copper = if item.layer == TOP_OVERLAY || item.layer == BOTTOM_OVERLAY {
                Color32::from_rgb(224, 222, 193)
            } else {
                Color32::from_rgb(44, 122, 72)
            };
            match &item.shape {
                Shape::Pad { .. } | Shape::Via { .. } => {
                    if !eda_core::pcb_polygon::on_layer(board, item, layer) {
                        continue;
                    }
                    let points = item.pad_outline(layer);
                    surface(
                        &mut faces,
                        &points,
                        &item.drill_outline(),
                        z,
                        Color32::from_rgb(193, 177, 113),
                        origin,
                    );
                }
                Shape::Track { start, end, width } => {
                    segment(&mut faces, *start, *end, *width, z, copper, &cuts)
                }
                Shape::Arc {
                    centre,
                    radius,
                    start_angle,
                    end_angle,
                    width,
                } => {
                    for edge in
                        arc_points(*centre, *radius, *start_angle, *end_angle, 64).windows(2)
                    {
                        segment(&mut faces, edge[0], edge[1], *width, z, copper, &cuts);
                    }
                }
                Shape::Fill {
                    start,
                    end,
                    rotation,
                } => planar(
                    &mut faces,
                    &rectangle(
                        midpoint(*start, *end),
                        Point {
                            x: (end.x - start.x).abs(),
                            y: (end.y - start.y).abs(),
                        },
                        *rotation,
                    ),
                    &[],
                    z,
                    copper,
                    &cuts,
                ),
                Shape::Region {
                    contour,
                    holes,
                    cutout: false,
                } => planar(&mut faces, contour, holes, z, copper, &cuts),
                _ => {}
            }
            if let Some(parent) = item
                .component
                .and_then(|i| board.components.iter().find(|c| c.index == i).map(|c| c.id))
            {
                scene.parents.insert(item.id, parent);
            }
            add(faces, Some(item.id));
        }
    }
    if components {
        for body in pcb_3d::bodies(board) {
            let mut faces = vec![];
            let color = Color32::from_rgb(65, 68, 73);
            let z0 = body.bottom_nm as f32 / 1e6;
            let z1 = body.top_nm as f32 / 1e6;
            rings(&mut faces, &body.outline, &body.holes, z0, color);
            rings(&mut faces, &body.outline, &body.holes, z1, color);
            wall(&mut faces, &body.outline, z0, z1, color, origin);
            for hole in &body.holes {
                wall(&mut faces, hole, z0, z1, color, origin);
            }
            add(faces, Some(body.component));
        }
    }
    let bounds = if board_bounds.empty {
        board.bounds()
    } else {
        board_bounds
    };
    let mut min = [
        bounds.min.x as f32 / 1e6,
        bounds.min.y as f32 / 1e6,
        -thickness,
    ];
    let mut max = [bounds.max.x as f32 / 1e6, bounds.max.y as f32 / 1e6, 0.];
    // Fabrication drawings outside the physical board must not shrink Fit Board.
    for face in &scene.faces {
        for p in face.triangle.v {
            if p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1] {
                min[2] = min[2].min(p[2]);
                max[2] = max[2].max(p[2]);
            }
        }
    }
    if !scene.faces.is_empty() {
        scene.centre = std::array::from_fn(|k| (min[k] + max[k]) * 0.5);
        scene.radius = (0..3)
            .map(|k| (max[k] - min[k]).powi(2))
            .sum::<f32>()
            .sqrt()
            * 0.5;
    }
    scene
}
fn planar(
    faces: &mut Vec<Triangle>,
    outer: &[Point],
    holes: &[Vec<Point>],
    z: f32,
    color: Color32,
    cuts: &pcb_3d::Cuts,
) {
    for face in cuts.surface(outer, holes) {
        rings(faces, &face.outline, &face.holes, z, color);
    }
}
fn segment(
    faces: &mut Vec<Triangle>,
    a: Point,
    b: Point,
    width: i64,
    z: f32,
    color: Color32,
    cuts: &pcb_3d::Cuts,
) {
    let length = ((b.x - a.x) as f64).hypot((b.y - a.y) as f64) as i64;
    let angle = ((b.y - a.y) as f64).atan2((b.x - a.x) as f64).to_degrees();
    planar(
        faces,
        &rectangle(
            midpoint(a, b),
            Point {
                x: length.max(width),
                y: width,
            },
            angle,
        ),
        &[],
        z,
        color,
        cuts,
    );
}
fn project(v: [f32; 3], state: &State) -> [f32; 3] {
    let v = std::array::from_fn::<_, 3, _>(|k| v[k] - state.focus.unwrap_or(state.scene.centre)[k]);
    let (s, c) = state.yaw.sin_cos();
    let x = v[0] * c - v[1] * s;
    let y = v[0] * s + v[1] * c;
    let (s, c) = state.pitch.sin_cos();
    [x, y * s + v[2] * c, -y * c + v[2] * s]
}
fn raster(
    state: &State,
    selected: &[Uuid],
    logical: Vec2,
    size: [usize; 2],
) -> (egui::ColorImage, Vec<u32>) {
    let [w, h] = size;
    let mut image = egui::ColorImage::filled(size, Color32::from_rgb(51, 55, 62));
    let mut depth = vec![f32::NEG_INFINITY; w * h];
    let mut ids = vec![0; w * h];
    let scale = logical.min_elem() * 0.43 / state.scene.radius.max(0.1) * state.zoom;
    let edge = |a: [f32; 3], b: [f32; 3], x: f32, y: f32| {
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    for (n, face) in state.scene.faces.iter().enumerate() {
        let projected = face.triangle.v.map(|v| project(v, state));
        let v = projected.map(|v| {
            [
                (v[0] * scale + logical.x / 2. + state.pan.x) * w as f32 / logical.x,
                (-v[1] * scale + logical.y / 2. + state.pan.y) * h as f32 / logical.y,
                v[2],
            ]
        });
        if !v.iter().flatten().all(|v| v.is_finite()) {
            continue;
        }
        let area = edge(v[0], v[1], v[2][0], v[2][1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let min = |k: usize, limit: usize| {
            v.iter()
                .map(|v| v[k])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .clamp(0., (limit - 1) as f32) as usize
        };
        let max = |k: usize, limit: usize| {
            v.iter()
                .map(|v| v[k])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .clamp(0., (limit - 1) as f32) as usize
        };
        let color = if face.owner.is_some_and(|id| {
            selected.contains(&id)
                || state
                    .scene
                    .parents
                    .get(&id)
                    .is_some_and(|p| selected.contains(p))
        }) {
            Color32::from_rgb(233, 177, 54)
        } else {
            face.triangle.color
        };
        let color = shade(&Triangle {
            v: projected,
            color,
        });
        for y in min(1, h)..=max(1, h) {
            for x in min(0, w)..=max(0, w) {
                let a = edge(v[1], v[2], x as f32 + 0.5, y as f32 + 0.5) / area;
                let b = edge(v[2], v[0], x as f32 + 0.5, y as f32 + 0.5) / area;
                let c = 1. - a - b;
                if a < -1e-5 || b < -1e-5 || c < -1e-5 {
                    continue;
                }
                let z = a * v[0][2] + b * v[1][2] + c * v[2][2];
                let index = y * w + x;
                if z > depth[index] {
                    depth[index] = z;
                    image.pixels[index] = color;
                    ids[index] = (n + 1) as u32;
                }
            }
        }
    }
    (image, ids)
}
fn drag_to(state: &mut State, pos: Pos2) -> bool {
    let Some((last, button, rotate)) = state.drag else {
        return false;
    };
    let delta = pos - last;
    if rotate {
        state.yaw += delta.x * 0.01;
        state.pitch = (state.pitch + delta.y * 0.01)
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    } else {
        state.pan += delta;
    }
    state.drag = Some((pos, button, rotate));
    delta != Vec2::ZERO
}
pub fn canvas(ui: &mut egui::Ui, board: &Board, view: &mut View) -> CanvasOutput {
    let state = &mut view.pcb.three_d;
    ui.horizontal(|ui| {
        ui.label("3D Layout Mode");
        if ui.small_button("Fit").clicked() {
            state.fit();
        }
        if ui.small_button("Top").clicked() {
            state.top();
        }
        if ui.small_button("Bottom").clicked() {
            state.bottom();
        }
        if ui
            .checkbox(&mut state.show_components, "Components")
            .changed()
        {
            state.built_revision = None;
        }
        ui.label("Shift + right drag: rotate · right drag: pan · wheel: zoom");
    });
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    if state.built_revision != Some(state.revision) {
        state.scene = Arc::new(build(board, state.show_components));
        state.built_revision = Some(state.revision);
        state.raster_key = None;
    }
    if let Some(target) = view.pcb.requested_fit.take() {
        state.fit();
        if matches!(target, crate::pcb::FitTarget::Selection) {
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for face in state.scene.faces.iter().filter(|f| {
                f.owner.is_some_and(|id| {
                    view.pcb.selected.contains(&id)
                        || state
                            .scene
                            .parents
                            .get(&id)
                            .is_some_and(|p| view.pcb.selected.contains(p))
                })
            }) {
                for p in face.triangle.v {
                    for k in 0..3 {
                        min[k] = min[k].min(p[k]);
                        max[k] = max[k].max(p[k]);
                    }
                }
            }
            if min[0].is_finite() {
                state.focus = Some(std::array::from_fn(|k| (min[k] + max[k]) * 0.5));
                let radius = (0..3)
                    .map(|k| (max[k] - min[k]).powi(2))
                    .sum::<f32>()
                    .sqrt()
                    * 0.5;
                state.zoom = (state.scene.radius / radius.max(1.)).clamp(0.1, 40.);
            }
        }
    }
    let focused = ui.input(|i| i.focused);
    let mut drag = state.drag.is_some();
    if !focused {
        state.drag = None;
    } else {
        for event in ui.input(|i| i.events.clone()) {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    modifiers,
                } if matches!(
                    button,
                    egui::PointerButton::Secondary | egui::PointerButton::Middle
                ) && rect.contains(pos)
                    && ui.clip_rect().contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id()) =>
                {
                    state.drag = Some((pos, button, modifiers.shift));
                }
                egui::Event::PointerMoved(pos) => {
                    drag |= drag_to(state, pos);
                }
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: false,
                    ..
                } if state.drag.is_some_and(|(_, b, _)| b == button) => {
                    drag |= drag_to(state, pos);
                    state.drag = None;
                }
                _ => {}
            }
        }
    }
    if drag {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if focused && response.hovered() {
        let factor = ui.input(|i| i.zoom_delta() * (i.smooth_scroll_delta.y * 0.003).exp());
        if factor.is_finite() && factor > 0. && factor != 1. {
            let old = state.zoom;
            state.zoom = (old * factor).clamp(0.1, 40.);
            if let Some(p) = response.hover_pos() {
                let p = p - rect.center();
                state.pan = p - (p - state.pan) * (state.zoom / old);
            }
        }
    }
    let size = [
        rect.width().ceil().clamp(1., 1200.) as usize,
        rect.height().ceil().clamp(1., 1000.) as usize,
    ];
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hash);
    rect.width().to_bits().hash(&mut hash);
    rect.height().to_bits().hash(&mut hash);
    for v in [state.yaw, state.pitch, state.zoom, state.pan.x, state.pan.y] {
        v.to_bits().hash(&mut hash);
    }
    state.focus.map(|p| p.map(f32::to_bits)).hash(&mut hash);
    view.pcb.selected.hash(&mut hash);
    state.revision.hash(&mut hash);
    state.show_components.hash(&mut hash);
    let key = hash.finish();
    if state.raster_key != Some(key) {
        let (image, ids) = raster(state, &view.pcb.selected, rect.size(), size);
        state.ids = Arc::new(ids);
        state.size = size;
        if let Some(texture) = &mut state.texture {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            state.texture = Some(ui.ctx().load_texture(
                format!("pcb-3d-{:?}", response.id),
                image,
                egui::TextureOptions::LINEAR,
            ));
        }
        state.raster_key = Some(key);
    }
    if let Some(texture) = &state.texture {
        ui.painter().image(
            texture.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
    }
    let hit = response.hover_pos().and_then(|p| {
        let x = (((p.x - rect.min.x) / rect.width()) * size[0] as f32) as usize;
        let y = (((p.y - rect.min.y) / rect.height()) * size[1] as f32) as usize;
        state
            .ids
            .get(y.min(size[1] - 1) * size[0] + x.min(size[0] - 1))
            .and_then(|n| n.checked_sub(1))
            .and_then(|n| state.scene.faces.get(n as usize))
            .and_then(|f| f.owner)
    });
    if hit.is_some() && !drag {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if let Some(c) = hit.and_then(|id| {
        board
            .components
            .iter()
            .find(|c| c.id == id || state.scene.parents.get(&id) == Some(&c.id))
    }) {
        response.clone().on_hover_text(format!(
            "{} · {}\n{}\nSimplified body · click to select, double-click for Properties",
            c.designator, c.comment, c.footprint
        ));
    }
    view.pcb.input = Some(Input {
        hit,
        hits: hit.into_iter().collect(),
        clicked: response.clicked(),
        double_clicked: response.double_clicked(),
        secondary_clicked: response.secondary_clicked(),
        hovered: response.hovered(),
        panning: drag,
        ..Default::default()
    });
    CanvasOutput {
        cursor_mm: None,
        schematic_input: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_picking_hides_back_components_and_selection_changes_only_pixels() {
        let id = Uuid::new_v4();
        let face = |z, owner| Face {
            triangle: Triangle {
                v: [[-1., -1., z], [1., -1., z], [0., 1., z]],
                color: Color32::GRAY,
            },
            owner,
        };
        let mut state = State {
            scene: Arc::new(Scene {
                faces: vec![face(-1., Some(id)), face(0., None)],
                centre: [0.; 3],
                radius: 2.,
                ..Default::default()
            }),
            ..Default::default()
        };
        state.top();
        let (normal, ids) = raster(&state, &[], Vec2::splat(100.), [100, 100]);
        assert_eq!(ids[50 * 100 + 50], 2);
        Arc::get_mut(&mut state.scene).unwrap().faces[1].owner = Some(id);
        let (highlight, ids) = raster(&state, &[id], Vec2::splat(100.), [100, 100]);
        assert_eq!(ids[50 * 100 + 50], 2);
        assert_ne!(normal.pixels[5050], highlight.pixels[5050]);
        state.bottom();
        let (_, ids) = raster(&state, &[], Vec2::splat(100.), [100, 100]);
        assert_eq!(ids[5050], 1);
    }
    #[test]
    fn viewport_caches_scene_texture_and_leaves_two_d_camera_untouched() {
        let board = Board::from_legacy(&eda_core::Document::new(
            eda_core::DocumentKind::Board,
            "3D",
        ));
        let ctx = egui::Context::default();
        let mut view = View::default();
        view.pcb.three_d.enabled = true;
        let before = view.camera.clone();
        let frame = |view: &mut View| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500., 400.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        canvas(ui, &board, view);
                    });
                },
            )
        };
        frame(&mut view);
        let scene = view.pcb.three_d.scene.clone();
        let texture = view.pcb.three_d.texture.as_ref().unwrap().id();
        let output = frame(&mut view);
        assert!(Arc::ptr_eq(&scene, &view.pcb.three_d.scene));
        assert_eq!(view.pcb.three_d.texture.as_ref().unwrap().id(), texture);
        assert!(output.textures_delta.set.is_empty());
        assert_eq!(view.camera.centre_mm, before.centre_mm);
        assert_eq!(view.camera.pixels_per_mm, before.pixels_per_mm);
        view.pcb.three_d.revision += 1;
        frame(&mut view);
        assert!(!Arc::ptr_eq(&scene, &view.pcb.three_d.scene));
        assert_eq!(view.pcb.three_d.texture.as_ref().unwrap().id(), texture);
    }
    #[test]
    fn fast_orbit_and_pan_use_press_modifiers_and_release_once() {
        let ctx = egui::Context::default();
        let board = Board::from_legacy(&eda_core::Document::new(
            eda_core::DocumentKind::Board,
            "Navigation",
        ));
        let mut view = View::default();
        let event = |pos, pressed, modifiers| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers,
        };
        let frame = |view: &mut View, events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500., 400.))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        canvas(ui, &board, view);
                    });
                },
            );
        };
        frame(&mut view, vec![]);
        let a = Pos2::new(250., 200.);
        let b = a + Vec2::new(20., 10.);
        let yaw = view.pcb.three_d.yaw;
        frame(
            &mut view,
            vec![
                egui::Event::PointerMoved(a),
                event(a, true, egui::Modifiers::SHIFT),
                egui::Event::PointerMoved(b),
                event(b, false, egui::Modifiers::NONE),
            ],
        );
        assert!((view.pcb.three_d.yaw - yaw - 0.2).abs() < 1e-5);
        assert!(view.pcb.three_d.drag.is_none());
        assert_eq!(view.pcb.three_d.pan, Vec2::ZERO);
        frame(
            &mut view,
            vec![
                event(a, true, egui::Modifiers::NONE),
                egui::Event::PointerMoved(b),
                event(b, false, egui::Modifiers::NONE),
            ],
        );
        assert_eq!(view.pcb.three_d.pan, Vec2::new(20., 10.));
        let yaw = view.pcb.three_d.yaw;
        frame(&mut view, vec![egui::Event::PointerMoved(a)]);
        assert_eq!(view.pcb.three_d.yaw, yaw);
    }
    #[test]
    fn off_board_fabrication_drawing_does_not_change_physical_fit() {
        let mut b = Board::from_legacy(&eda_core::Document::new(
            eda_core::DocumentKind::Board,
            "Fit",
        ));
        let before = build(&b, true);
        b.items.push(Item::new(
            TOP_OVERLAY,
            None,
            Shape::Track {
                start: Point::mm(1000, 1000),
                end: Point::mm(2000, 2000),
                width: 100_000,
            },
        ));
        let after = build(&b, true);
        assert_eq!(before.centre, after.centre);
        assert_eq!(before.radius, after.radius);
        assert_eq!(before.faces.len(), after.faces.len());
    }
}
