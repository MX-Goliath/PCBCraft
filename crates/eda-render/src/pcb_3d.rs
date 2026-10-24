//! Full-board 3D viewport with resident GPU geometry and a software fallback.
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
    sync::{Arc, Mutex, mpsc},
};
use uuid::Uuid;
#[path = "pcb_3d_gpu.rs"]
mod gpu;
pub use gpu::enable_gpu;
#[path = "pcb_3d_text.rs"]
mod lettering;
#[path = "pcb_3d_pick.rs"]
mod picking;

#[derive(Clone)]
struct Face {
    triangle: Triangle,
    owner: Option<Uuid>,
    material: usize,
    layer: u16,
}
#[derive(Default)]
struct Scene {
    parents: std::collections::HashMap<Uuid, Uuid>,
    faces: Vec<Face>,
    centre: [f32; 3],
    radius: f32,
    thickness: f32,
    geometry: gpu::Geometry,
    picking: picking::Index,
}
#[derive(Clone, PartialEq)]
pub struct Presentation {
    pub perspective: bool,
    pub thickness_scale: f32,
    pub by_layer: bool,
    pub colors: [Color32; 8],
    pub opacity: [u8; 8],
    pub layer_colors: std::collections::BTreeMap<u16, Color32>,
    pub background: [Color32; 2],
    pub single_layer: bool,
    pub active_layer: u16,
    pub dim: u8,
    pub highlighted_dim: u8,
    pub masked_dim: u8,
    pub selection_color: Color32,
    pub highlight_color: Color32,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            perspective: false,
            thickness_scale: 1.0,
            by_layer: false,
            colors: [
                Color32::from_rgb(193, 177, 113),
                Color32::from_rgb(139, 128, 70),
                Color32::from_rgb(224, 222, 193),
                Color32::from_rgb(224, 222, 193),
                Color32::from_rgb(28, 106, 64),
                Color32::from_rgb(28, 106, 64),
                Color32::GRAY,
                Color32::GRAY,
            ],
            opacity: [255, 255, 255, 255, 255, 255, 0, 0],
            layer_colors: Default::default(),
            background: [Color32::from_rgb(51, 55, 62); 2],
            single_layer: false,
            active_layer: TOP,
            dim: 153,
            highlighted_dim: 102,
            masked_dim: 128,
            selection_color: Color32::WHITE,
            highlight_color: Color32::from_rgb(255, 255, 200),
        }
    }
}
type SceneKey = (u64, bool, u64);
#[derive(Clone, Copy)]
pub enum CameraCommand {
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
    RotateLeft,
    RotateRight,
    RotateUp,
    RotateDown,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Isometric,
    FlippedIsometric,
    QuarterTurn,
    Flip,
}
struct SceneJob {
    key: SceneKey,
    receiver: Mutex<mpsc::Receiver<Scene>>,
}
struct RasterJob {
    scene: SceneKey,
    key: u64,
    size: [usize; 2],
    receiver: Mutex<mpsc::Receiver<(egui::ColorImage, Vec<u32>)>>,
}
#[derive(Clone)]
pub struct State {
    pub enabled: bool,
    pub presentation: Presentation,
    pub revision: u64,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    pub pan: Vec2,
    pub(crate) highlighted: Option<std::collections::BTreeSet<Uuid>>,
    focus: Option<[f32; 3]>,
    drag: Option<(Pos2, egui::PointerButton, bool)>,
    pub show_components: bool,
    scene: Arc<Scene>,
    built_key: Option<SceneKey>,
    generation: u64,
    scene_job: Option<Arc<SceneJob>>,
    failed_key: Option<SceneKey>,
    raster_job: Option<Arc<RasterJob>>,
    failed_raster: Option<u64>,
    texture: Option<egui::TextureHandle>,
    raster_key: Option<u64>,
    ids: Arc<Vec<u32>>,
    size: [usize; 2],
    gpu: Option<Arc<gpu::Renderer>>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            enabled: false,
            presentation: Default::default(),
            revision: 0,
            yaw: 0.,
            pitch: std::f32::consts::FRAC_PI_2,
            zoom: 1.,
            pan: Vec2::ZERO,
            highlighted: None,
            focus: None,
            drag: None,
            show_components: true,
            scene: Arc::default(),
            built_key: None,
            generation: 0,
            scene_job: None,
            failed_key: None,
            raster_job: None,
            failed_raster: None,
            texture: None,
            raster_key: None,
            ids: Arc::default(),
            size: [1, 1],
            gpu: None,
        }
    }
}
impl State {
    pub fn camera_command(&mut self, command: CameraCommand) {
        use CameraCommand::*;
        let step = std::f32::consts::PI / 18.;
        match command {
            Top => self.top(),
            Bottom => self.bottom(),
            Flip => self.flip(),
            Front | Back | Left | Right => {
                self.pitch = 0.;
                self.yaw = match command {
                    Back => std::f32::consts::PI,
                    Left => -std::f32::consts::FRAC_PI_2,
                    Right => std::f32::consts::FRAC_PI_2,
                    _ => 0.,
                };
                self.fit();
            }
            Isometric | FlippedIsometric => {
                self.yaw = std::f32::consts::FRAC_PI_4;
                self.pitch = std::f32::consts::FRAC_PI_4;
                self.fit();
                if matches!(command, FlippedIsometric) {
                    self.flip();
                }
            }
            RotateLeft => self.yaw += step,
            RotateRight => self.yaw -= step,
            RotateUp => self.pitch = (self.pitch + step).min(std::f32::consts::FRAC_PI_2),
            RotateDown => self.pitch = (self.pitch - step).max(-std::f32::consts::FRAC_PI_2),
            MoveLeft => self.pan.x -= 20.,
            MoveRight => self.pan.x += 20.,
            MoveUp => self.pan.y -= 20.,
            MoveDown => self.pan.y += 20.,
            QuarterTurn => self.yaw += std::f32::consts::FRAC_PI_2,
        }
    }
    pub fn read_configuration(&mut self, board: &Board) {
        let fields: std::collections::BTreeMap<_, _> = board
            .properties
            .get("3DCONFIGURATION")
            .map_or("", String::as_str)
            .split('`')
            .filter_map(|s| s.split_once('='))
            .collect();
        let color = |key: &str| {
            fields
                .get(key)
                .and_then(|v| v.trim().parse::<u32>().ok())
                .map(|v| Color32::from_rgb(v as u8, (v >> 8) as u8, (v >> 16) as u8))
        };
        self.show_components = fields.get("CFG3D.SHOWCOMPONENTBODIES") != Some(&"FALSE");
        self.presentation.perspective = true;
        self.presentation.by_layer = fields.get("CFG3D.USESYSCOLORSFOR3D") == Some(&"TRUE");
        for (n, name) in [
            "COPPER",
            "BOARDCORE",
            "TOPSILKSCREEN",
            "BOTSILKSCREEN",
            "TOPSOLDERMASK",
            "BOTSOLDERMASK",
            "TOPPASTEMASK",
            "BOTPASTEMASK",
        ]
        .into_iter()
        .enumerate()
        {
            let key = format!("CFG3D.{name}COLOR");
            if let Some(c) = color(&key) {
                self.presentation.colors[n] = c;
            }
            if let Some(a) = fields
                .get(format!("{key}OPACITY").as_str())
                .and_then(|v| v.trim().parse::<f32>().ok())
                .filter(|a| a.is_finite())
            {
                self.presentation.opacity[n] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        for (n, name) in ["CFG3D.WORKSPACEBEGINCOLOR", "CFG3D.WORKSPACEENDCOLOR"]
            .into_iter()
            .enumerate()
        {
            if let Some(c) = color(name) {
                self.presentation.background[n] = c;
            }
        }
        if let Some(scale) = fields
            .get("CFG3D.BOARDTHICKNESSSCALING")
            .and_then(|v| v.trim().parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.0)
        {
            self.presentation.thickness_scale = scale.clamp(1.0, 20.0);
        }
    }
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.built_key = None;
        self.raster_key = None;
    }
    pub fn preparing(&self) -> bool {
        self.built_key != Some(self.scene_key())
    }
    pub fn rendering(&self) -> bool {
        self.raster_job.is_some()
    }
    fn render(
        &mut self,
        key: u64,
        selected: &[Uuid],
        logical: Vec2,
        size: [usize; 2],
        ctx: &egui::Context,
    ) {
        if let Some(job) = &self.raster_job {
            let result = job.receiver.lock().unwrap().try_recv();
            match result {
                Ok((image, ids)) => {
                    if job.scene == self.scene_key() {
                        self.ids = Arc::new(ids);
                        self.size = job.size;
                        if let Some(texture) = &mut self.texture {
                            texture.set(image, egui::TextureOptions::LINEAR);
                        } else {
                            self.texture = Some(ctx.load_texture(
                                "pcb-3d",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                        self.raster_key = Some(job.key);
                    }
                    self.raster_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.failed_raster = Some(job.key);
                    self.raster_job = None;
                }
            }
        }
        if self.raster_key != Some(key)
            && self.raster_job.is_none()
            && self.failed_raster != Some(key)
        {
            let mut snapshot = self.clone();
            snapshot.scene_job = None;
            snapshot.raster_job = None;
            snapshot.texture = None;
            let selected = selected.to_vec();
            let (sender, receiver) = mpsc::sync_channel(1);
            self.raster_job = Some(Arc::new(RasterJob {
                scene: self.scene_key(),
                key,
                size,
                receiver: Mutex::new(receiver),
            }));
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = raster(&snapshot, &selected, logical, size);
                let _ = sender.send(result);
                ctx.request_repaint();
            });
        }
        if self.raster_job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(25));
        }
    }
    fn scene_key(&self) -> SceneKey {
        (self.revision, self.show_components, self.generation)
    }
    fn prepare(&mut self, board: &Board, ctx: &egui::Context) {
        let key = self.scene_key();
        if let Some(job) = &self.scene_job {
            let received = job.receiver.lock().unwrap().try_recv();
            match received {
                Ok(scene) => {
                    if job.key == key {
                        if std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some() {
                            eprintln!(
                                "pcb_3d_scene triangles={} styles={}",
                                scene.faces.len(),
                                scene.geometry.styles.len()
                            );
                        }
                        self.scene = Arc::new(scene);
                        self.built_key = Some(key);
                        self.raster_key = None;
                    }
                    self.scene_job = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.failed_key = Some(job.key);
                    self.scene_job = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.preparing() && self.scene_job.is_none() && self.failed_key != Some(key) {
            let snapshot = board.clone();
            let (sender, receiver) = mpsc::sync_channel(1);
            self.scene_job = Some(Arc::new(SceneJob {
                key,
                receiver: Mutex::new(receiver),
            }));
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let scene = build(&snapshot, key.1);
                let _ = sender.send(scene);
                ctx.request_repaint();
            });
        }
    }
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
        self.yaw = std::f32::consts::PI;
        self.pitch = -std::f32::consts::FRAC_PI_2;
        self.fit();
    }
    pub fn flip(&mut self) {
        self.yaw = (std::f32::consts::PI - self.yaw).rem_euclid(std::f32::consts::TAU);
        self.pitch = -self.pitch;
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
    scene.thickness = thickness;
    let mask = Color32::from_rgb(28, 106, 64);
    let cuts = pcb_3d::Cuts::new(board);
    let mut add = |triangles: Vec<Triangle>, owner, material, layer| {
        scene
            .faces
            .extend(triangles.into_iter().map(|triangle| Face {
                triangle,
                owner,
                material,
                layer,
            }))
    };
    for region in pcb_3d::substrate(board) {
        let mut faces = vec![];
        rings(
            &mut faces,
            &region.outline,
            &region.holes,
            -0.005,
            Color32::from_rgb(139, 128, 70),
        );
        rings(
            &mut faces,
            &region.outline,
            &region.holes,
            -thickness + 0.005,
            Color32::from_rgb(139, 128, 70),
        );
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
        add(faces, None, 1, MULTI_LAYER);
    }
    for (bottom, z, material, layer) in
        [(false, 0.04, 4, TOP), (true, -thickness - 0.04, 5, BOTTOM)]
    {
        for region in pcb_3d::solder_mask(board, bottom) {
            let mut faces = vec![];
            rings(&mut faces, &region.outline, &region.holes, z, mask);
            add(faces, None, material, layer);
        }
    }
    for item in &board.items {
        if item.keepout
            || item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
            || item
                .properties
                .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
            || !board.text_visible(item)
        {
            continue;
        }
        for (layer, mut z) in [(TOP, 0.015), (BOTTOM, -thickness - 0.015)] {
            let overlay = if layer == TOP {
                TOP_OVERLAY
            } else {
                BOTTOM_OVERLAY
            };
            let paste_layer = if layer == TOP { 35 } else { 36 };
            if item.layer != layer
                && item.layer != MULTI_LAYER
                && item.layer != overlay
                && item.layer != paste_layer
            {
                continue;
            }
            if item.layer == overlay || item.layer == paste_layer {
                z += if layer == TOP { 0.06 } else { -0.06 };
            }
            let mut faces = vec![];
            let copper = if item.layer == TOP_OVERLAY || item.layer == BOTTOM_OVERLAY {
                Color32::from_rgb(224, 222, 193)
            } else if item.layer == 35 || item.layer == 36 {
                Color32::GRAY
            } else {
                Color32::from_rgb(193, 177, 113)
            };
            match &item.shape {
                Shape::Pad { .. } | Shape::Via { .. } => {
                    if !eda_core::pcb_polygon::on_layer(board, item, layer) {
                        continue;
                    }
                    let points = item.pad_outline(layer);
                    planar(
                        &mut faces,
                        &points,
                        &[],
                        z,
                        Color32::from_rgb(193, 177, 113),
                        &cuts,
                    );
                }
                Shape::Track { start, end, width } => {
                    segment(&mut faces, *start, *end, *width, z, copper, &cuts)
                }
                Shape::Arc { .. } => {
                    for shape in pcb_3d::planar_outline(item) {
                        planar(&mut faces, &shape.outline, &shape.holes, z, copper, &cuts);
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
                Shape::Text { .. } => {
                    for outline in lettering::outlines(item) {
                        planar(
                            &mut faces,
                            &outline.outline,
                            &outline.holes,
                            z,
                            copper,
                            &cuts,
                        );
                    }
                }
                _ => {}
            }
            if let Some(parent) = item
                .component
                .and_then(|i| board.components.iter().find(|c| c.index == i).map(|c| c.id))
            {
                scene.parents.insert(item.id, parent);
            }
            add(
                faces,
                Some(item.id),
                if item.layer == TOP_OVERLAY {
                    2
                } else if item.layer == BOTTOM_OVERLAY {
                    3
                } else if item.layer == 35 {
                    6
                } else if item.layer == 36 {
                    7
                } else {
                    0
                },
                item.layer,
            );
            if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
                let bottom = layer == BOTTOM;
                if let Ok(outline) = item.mask_outline(board, MaskKind::Paste, bottom) {
                    let mut paste = vec![];
                    surface(
                        &mut paste,
                        &outline,
                        &item.drill_outline(),
                        z + if bottom { -0.05 } else { 0.05 },
                        Color32::GRAY,
                        origin,
                    );
                    add(
                        paste,
                        Some(item.id),
                        if bottom { 7 } else { 6 },
                        if bottom { 36 } else { 35 },
                    );
                }
            }
        }
        if matches!(
            item.shape,
            Shape::Via {
                from_layer: TOP,
                to_layer: BOTTOM,
                ..
            } | Shape::Via {
                from_layer: BOTTOM,
                to_layer: TOP,
                ..
            } | Shape::Pad {
                plated: true,
                hole: 1..,
                ..
            }
        ) {
            let mut faces = vec![];
            wall(
                &mut faces,
                &item.drill_outline(),
                -thickness,
                0.,
                Color32::from_rgb(193, 177, 113),
                origin,
            );
            add(faces, Some(item.id), 0, MULTI_LAYER);
        }
    }
    if components {
        for item in board.items.iter().filter(|i| {
            i.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
                && !i
                    .properties
                    .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE")
        }) {
            let Some(model) = item
                .properties
                .get("MODELID")
                .and_then(|id| board.models.get(id))
            else {
                continue;
            };
            let owner = item
                .component
                .and_then(|i| board.components.iter().find(|c| c.index == i))
                .map(|c| c.id);
            let opacity = item
                .properties
                .get("BODYOPACITY3D")
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(1.)
                .clamp(0., 1.);
            let faces = model
                .triangles
                .iter()
                .filter_map(|t| {
                    let mut v = [[0.; 3]; 3];
                    for (vertex, p) in v.iter_mut().zip(t.vertices) {
                        *vertex = pcb_3d::model_point(board, item, p)?.map(|x| x as f32);
                    }
                    Some(Triangle {
                        v,
                        color: Color32::from_rgba_unmultiplied(
                            t.color[0],
                            t.color[1],
                            t.color[2],
                            (f32::from(t.color[3]) * opacity) as u8,
                        ),
                    })
                })
                .collect();
            add(faces, owner, 8, MULTI_LAYER);
        }
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
            add(faces, Some(body.component), 8, MULTI_LAYER);
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
    scene.geometry = gpu::Geometry::new(&scene);
    scene.picking = picking::Index::new(&scene.faces);
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
    for shape in pcb_3d::planar_outline(&Item::new(
        TOP,
        None,
        Shape::Track {
            start: a,
            end: b,
            width,
        },
    )) {
        planar(faces, &shape.outline, &shape.holes, z, color, cuts);
    }
}
fn camera_point(v: [f32; 3], state: &State) -> [f32; 3] {
    let centre = state.focus.unwrap_or(state.scene.centre);
    let scale_z = |z: f32| {
        z + z.clamp(-state.scene.thickness, 0.0) * (state.presentation.thickness_scale - 1.0)
    };
    let v = [
        v[0] - centre[0],
        v[1] - centre[1],
        scale_z(v[2]) - scale_z(centre[2]),
    ];
    let (s, c) = state.yaw.sin_cos();
    let x = v[0] * c - v[1] * s;
    let y = v[0] * s + v[1] * c;
    let (s, c) = state.pitch.sin_cos();
    let y = y * s + v[2] * c;
    let depth = -(v[0] * state.yaw.sin() + v[1] * state.yaw.cos()) * c + v[2] * s;
    [x, y, depth]
}
fn project(v: [f32; 3], state: &State) -> [f32; 3] {
    let [x, y, depth] = camera_point(v, state);
    let perspective = if state.presentation.perspective {
        let d = state.scene.radius.max(0.1) * 4.0;
        d / (d - depth).max(d * 0.1)
    } else {
        1.0
    };
    [x * perspective, y * perspective, depth]
}
fn material_color(face: &Face, state: &State) -> Color32 {
    let p = &state.presentation;
    let defaults = Presentation::default();
    let mut alpha = if face.material < 8 {
        p.opacity[face.material]
    } else {
        face.triangle.color.a()
    };
    let mut color = if face.material >= 8 {
        face.triangle.color
    } else if p.by_layer {
        p.layer_colors
            .get(&face.layer)
            .copied()
            .unwrap_or(p.colors[face.material])
    } else if p.colors[face.material] == defaults.colors[face.material] {
        face.triangle.color
    } else {
        p.colors[face.material]
    };
    if p.single_layer && face.layer != p.active_layer && face.layer != MULTI_LAYER {
        alpha = (u16::from(alpha) * u16::from(p.dim) / 255) as u8;
    }
    if let Some(highlighted) = &state.highlighted {
        if face.owner.is_some_and(|id| highlighted.contains(&id)) {
            let c = p.highlight_color;
            let channel = |v: u8| (u16::from(v) * u16::from(p.highlighted_dim) / 255) as u8;
            color = Color32::from_rgb(channel(c.r()), channel(c.g()), channel(c.b()));
        } else {
            alpha = (u16::from(alpha) * u16::from(p.masked_dim) / 255) as u8;
        }
    }
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn raster(
    state: &State,
    selected: &[Uuid],
    logical: Vec2,
    size: [usize; 2],
) -> (egui::ColorImage, Vec<u32>) {
    let selected: std::collections::HashSet<_> = selected.iter().copied().collect();
    let [w, h] = size;
    let mut image = egui::ColorImage::filled(size, state.presentation.background[0]);
    for y in 0..h {
        let t = y as f32 / h.saturating_sub(1).max(1) as f32;
        let [a, b] = state.presentation.background;
        let c = Color32::from_rgb(
            egui::lerp(f32::from(a.r())..=f32::from(b.r()), t).round() as u8,
            egui::lerp(f32::from(a.g())..=f32::from(b.g()), t).round() as u8,
            egui::lerp(f32::from(a.b())..=f32::from(b.b()), t).round() as u8,
        );
        image.pixels[y * w..(y + 1) * w].fill(c);
    }
    let mut depth = vec![f32::NEG_INFINITY; w * h];
    let mut ids = vec![0; w * h];
    let scale = logical.min_elem() * 0.43 / state.scene.radius.max(0.1) * state.zoom;
    let edge = |a: [f32; 3], b: [f32; 3], x: f32, y: f32| {
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    let mut order: Vec<_> = state
        .scene
        .faces
        .iter()
        .enumerate()
        .filter_map(|(n, face)| {
            let color = material_color(face, state);
            if color.a() == 0 {
                return None;
            }
            let projected = face.triangle.v.map(|v| project(v, state));
            let depth = projected.iter().map(|v| v[2]).sum::<f32>();
            Some((n, face, color, projected, depth))
        })
        .collect();
    order.sort_by(|a, b| {
        let aa = a.2.a() == 255;
        let bb = b.2.a() == 255;
        bb.cmp(&aa).then_with(|| a.4.total_cmp(&b.4))
    });
    for (n, face, material, projected, _) in order {
        let mut v = projected.map(|v| {
            [
                (v[0] * scale + logical.x / 2. + state.pan.x) * w as f32 / logical.x,
                (-v[1] * scale + logical.y / 2. + state.pan.y) * h as f32 / logical.y,
                v[2],
            ]
        });
        if !v.iter().flatten().all(|v| v.is_finite()) {
            continue;
        }
        let mut area = edge(v[0], v[1], v[2][0], v[2][1]);
        if area.abs() < 1e-6 {
            continue;
        }
        if area < 0. {
            v.swap(1, 2);
            area = -area;
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
            state.presentation.selection_color
        } else {
            material
        };
        let color = shade(&Triangle {
            v: face.triangle.v.map(|v| camera_point(v, state)),
            color,
        });
        // Half-open triangle edges: a shared diagonal belongs to one triangle.
        // Inclusive barycentric tolerances blend translucent surfaces twice.
        let inclusive = |a: [f32; 3], b: [f32; 3]| b[1] < a[1] || (b[1] == a[1] && b[0] > a[0]);
        let covers = |value: f32, a, b| value > 0. || (value == 0. && inclusive(a, b));
        for y in min(1, h)..=max(1, h) {
            for x in min(0, w)..=max(0, w) {
                let a = edge(v[1], v[2], x as f32 + 0.5, y as f32 + 0.5);
                let b = edge(v[2], v[0], x as f32 + 0.5, y as f32 + 0.5);
                let c = edge(v[0], v[1], x as f32 + 0.5, y as f32 + 0.5);
                if !covers(a, v[1], v[2]) || !covers(b, v[2], v[0]) || !covers(c, v[0], v[1]) {
                    continue;
                }
                let z = if state.presentation.perspective {
                    let d = state.scene.radius.max(0.1) * 4.;
                    d - area
                        / (a / (d - v[0][2]).max(d * 0.1)
                            + b / (d - v[1][2]).max(d * 0.1)
                            + c / (d - v[2][2]).max(d * 0.1))
                } else {
                    (a * v[0][2] + b * v[1][2] + c * v[2][2]) / area
                };
                let index = y * w + x;
                if z > depth[index] {
                    if color.a() == 255 {
                        depth[index] = z;
                        image.pixels[index] = color;
                    } else {
                        let old = image.pixels[index];
                        let inv = 255 - u16::from(color.a());
                        let channel =
                            |a: u8, b: u8| (u16::from(a) + u16::from(b) * inv / 255).min(255) as u8;
                        image.pixels[index] = Color32::from_rgb(
                            channel(color.r(), old.r()),
                            channel(color.g(), old.g()),
                            channel(color.b(), old.b()),
                        );
                    }
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
    view.pcb.sync_3d_display(board);
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
            state.invalidate();
        }
        ui.label("Shift + right drag: rotate · right drag: pan · wheel: zoom");
    });
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    state.prepare(board, ui.ctx());
    if !state.preparing()
        && let Some(target) = view.pcb.requested_fit.take()
    {
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
    if state.preparing() {
        view.pcb.input = None;
        ui.painter()
            .rect_filled(rect, 0., state.presentation.background[0]);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if state.failed_key == Some(state.scene_key()) {
                "Could not prepare 3D view"
            } else {
                "Preparing 3D view…"
            },
            egui::FontId::proportional(14.),
            Color32::WHITE,
        );
        if state.scene_job.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
        return CanvasOutput {
            cursor_mm: None,
            schematic_input: None,
        };
    }
    let dpi = ui.ctx().pixels_per_point();
    let size = [
        (rect.width() * dpi).ceil().clamp(1., 1800.) as usize,
        (rect.height() * dpi).ceil().clamp(1., 1400.) as usize,
    ];
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    view.pcb.selected.hash(&mut hash);
    state.revision.hash(&mut hash);
    state.show_components.hash(&mut hash);
    state.presentation.perspective.hash(&mut hash);
    state.presentation.thickness_scale.to_bits().hash(&mut hash);
    state.presentation.background.hash(&mut hash);
    state.presentation.single_layer.hash(&mut hash);
    state.presentation.active_layer.hash(&mut hash);
    state.presentation.dim.hash(&mut hash);
    state.presentation.highlighted_dim.hash(&mut hash);
    state.presentation.masked_dim.hash(&mut hash);
    state.presentation.selection_color.hash(&mut hash);
    state.presentation.highlight_color.hash(&mut hash);
    state.highlighted.hash(&mut hash);
    state.presentation.by_layer.hash(&mut hash);
    state.presentation.opacity.hash(&mut hash);
    state
        .presentation
        .colors
        .map(|c| c.to_array())
        .hash(&mut hash);
    for (k, c) in &state.presentation.layer_colors {
        k.hash(&mut hash);
        c.to_array().hash(&mut hash);
    }
    let style_key = hash.finish();
    size.hash(&mut hash);
    rect.width().to_bits().hash(&mut hash);
    rect.height().to_bits().hash(&mut hash);
    for v in [state.yaw, state.pitch, state.zoom, state.pan.x, state.pan.y] {
        v.to_bits().hash(&mut hash);
    }
    state.focus.map(|p| p.map(f32::to_bits)).hash(&mut hash);
    let key = hash.finish();
    let interactive;
    let hit;
    if let Some(format) =
        crate::pcb::gpu_format(ui.ctx()).filter(|_| gpu::supported(ui.ctx(), &state.scene))
    {
        let renderer = state
            .gpu
            .get_or_insert_with(|| Arc::new(gpu::Renderer::default()))
            .clone();
        ui.painter().add(renderer.callback(
            state,
            &view.pcb.selected,
            rect,
            size,
            [key, style_key],
            format,
        ));
        interactive = true;
        hit = response.hover_pos().filter(|_| !drag).and_then(|p| {
            state
                .scene
                .picking
                .hit(state, p - rect.min, rect.size())
                .and_then(|n| state.scene.faces[n].owner)
        });
    } else {
        state.render(key, &view.pcb.selected, rect.size(), size, ui.ctx());
        if let Some(texture) = &state.texture {
            ui.painter().image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        } else {
            ui.painter()
                .rect_filled(rect, 0., state.presentation.background[0]);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                if state.failed_raster == Some(key) {
                    "Could not render 3D view"
                } else {
                    "Rendering 3D view…"
                },
                egui::FontId::proportional(14.),
                Color32::WHITE,
            );
        }
        // The displayed image may lag while orbiting. Never pick using a depth
        // buffer belonging to a different camera, selection, size or document.
        interactive = state.raster_key == Some(key);
        hit = response.hover_pos().filter(|_| interactive).and_then(|p| {
            let x = (((p.x - rect.min.x) / rect.width()) * size[0] as f32) as usize;
            let y = (((p.y - rect.min.y) / rect.height()) * size[1] as f32) as usize;
            state
                .ids
                .get(y.min(size[1] - 1) * size[0] + x.min(size[0] - 1))
                .and_then(|n| n.checked_sub(1))
                .and_then(|n| state.scene.faces.get(n as usize))
                .and_then(|f| f.owner)
        });
    }
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
            "{} · {}\n{}\n{} · click to select, double-click for Properties",
            c.designator,
            c.comment,
            c.footprint,
            if board.items.iter().any(|i| i.component == Some(c.index)
                && i.properties
                    .get("MODELID")
                    .is_some_and(|id| board.models.contains_key(id)))
            {
                "STEP model"
            } else {
                "Simplified body"
            }
        ));
    }
    view.pcb.input = Some(Input {
        hit,
        hits: hit.into_iter().collect(),
        clicked: response.clicked() && interactive,
        double_clicked: response.double_clicked() && interactive,
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
    fn bottom_view_mirrors_x_and_preserves_y_like_the_native_flip() {
        let mut s = State::default();
        s.top();
        let top = project([1., 2., 0.], &s);
        s.bottom();
        let bottom = project([1., 2., 0.], &s);
        assert!((top[0] + bottom[0]).abs() < 1e-5);
        assert!((top[1] - bottom[1]).abs() < 1e-5);
        s.flip();
        let back = project([1., 2., 0.], &s);
        assert!((back[0] - top[0]).abs() < 1e-5);
        assert!((back[1] - top[1]).abs() < 1e-5);
    }
    #[test]
    fn transparent_quad_has_no_double_blended_diagonal() {
        let face = |v| Face {
            triangle: Triangle {
                v,
                color: Color32::RED,
            },
            owner: None,
            material: 0,
            layer: TOP,
        };
        let mut s = State {
            scene: Arc::new(Scene {
                radius: 2.,
                faces: vec![
                    face([[-1., -1., 0.], [1., -1., 0.], [1., 1., 0.]]),
                    face([[-1., -1., 0.], [1., 1., 0.], [-1., 1., 0.]]),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        s.top();
        s.presentation.opacity[0] = 128;
        s.presentation.background = [Color32::from_rgb(51, 55, 62); 2];
        let (image, _) = raster(&s, &[], Vec2::splat(100.), [100, 100]);
        let color = image.pixels[5050];
        for y in 35..65 {
            for x in 35..65 {
                assert_eq!(image.pixels[y * 100 + x], color, "pixel {x},{y}");
            }
        }
    }
    #[test]
    fn single_layer_and_highlight_mask_compose_with_material_opacity() {
        let owner = Uuid::new_v4();
        let face = Face {
            triangle: Triangle {
                v: [[0.; 3]; 3],
                color: Color32::BLUE,
            },
            owner: Some(owner),
            material: 0,
            layer: TOP,
        };
        let mut state = State::default();
        state.presentation.single_layer = true;
        state.presentation.active_layer = BOTTOM;
        state.presentation.dim = 128;
        state.presentation.opacity[0] = 128;
        assert_eq!(material_color(&face, &state).a(), 64);
        state.highlighted = Some(std::collections::BTreeSet::from([owner]));
        state.presentation.highlight_color = Color32::RED;
        state.presentation.highlighted_dim = 255;
        let color = material_color(&face, &state);
        assert_eq!(color.a(), 64);
        assert!(color.r() > 0 && color.b() == 0);
        state.highlighted = Some(Default::default());
        state.presentation.masked_dim = 128;
        assert_eq!(material_color(&face, &state).a(), 32);
    }
    #[test]
    fn native_material_colors_opacity_background_and_body_visibility_are_read() {
        let mut board = Board::default();
        board.properties.insert("3DCONFIGURATION".into(),
            "CFG3D.COPPERCOLOR=3323360`CFG3D.BOARDCORECOLOROPACITY=0.86`CFG3D.TOPSOLDERMASKCOLOROPACITY=0.83`CFG3D.SHOWCOMPONENTBODIES=FALSE`CFG3D.BOARDTHICKNESSSCALING=2`CFG3D.WORKSPACEBEGINCOLOR=11908806`CFG3D.BOTSOLDERMASKCOLOROPACITY=NaN".into());
        let before = board.clone();
        let mut state = State::default();
        state.read_configuration(&board);
        assert_eq!(
            state.presentation.colors[0],
            Color32::from_rgb(224, 181, 50)
        );
        assert_eq!(state.presentation.opacity[1], 219);
        assert_eq!(state.presentation.opacity[4], 212);
        assert_eq!(state.presentation.opacity[5], 255);
        assert_eq!(
            state.presentation.background[0],
            Color32::from_rgb(198, 182, 181)
        );
        assert!(!state.show_components);
        assert_eq!(state.presentation.thickness_scale, 2.);
        assert_eq!(before, board);
    }
    #[test]
    fn thickness_scaling_keeps_bottom_component_height_and_projection_changes_depth_scale() {
        let mut state = State {
            scene: Arc::new(Scene {
                thickness: 1.,
                radius: 3.,
                ..Default::default()
            }),
            ..Default::default()
        };
        state.presentation.thickness_scale = 4.;
        let distance = |a: [f32; 3], b: [f32; 3]| {
            a.iter()
                .zip(b)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                .sqrt()
        };
        assert!(
            (distance(
                project([0., 0., 0.], &state),
                project([0., 0., -1.], &state)
            ) - 4.)
                .abs()
                < 1e-5
        );
        assert!(
            (distance(
                project([0., 0., -1.], &state),
                project([0., 0., -2.], &state)
            ) - 1.)
                .abs()
                < 1e-5
        );
        let ortho = project([1., 1., 1.], &state);
        state.presentation.perspective = true;
        let perspective = project([1., 1., 1.], &state);
        assert_ne!(ortho[..2], perspective[..2]);
        assert_eq!(ortho[2], perspective[2]);
    }
    #[test]
    fn material_opacity_blends_and_fully_hidden_material_is_not_pickable() {
        let face = |z, material, color| Face {
            triangle: Triangle {
                v: [[-1., -1., z], [1., -1., z], [0., 1., z]],
                color,
            },
            owner: Some(Uuid::new_v4()),
            material,
            layer: TOP,
        };
        let mut state = State {
            scene: Arc::new(Scene {
                radius: 2.,
                faces: vec![face(-1., 1, Color32::BLUE), face(0., 0, Color32::RED)],
                ..Default::default()
            }),
            ..Default::default()
        };
        state.top();
        let (opaque, ids) = raster(&state, &[], Vec2::splat(100.), [100, 100]);
        assert_eq!(ids[5050], 2);
        state.presentation.opacity[0] = 128;
        let (blend, ids) = raster(&state, &[], Vec2::splat(100.), [100, 100]);
        assert_eq!(ids[5050], 2);
        assert_ne!(opaque.pixels[5050], blend.pixels[5050]);
        assert!(blend.pixels[5050].r() > 0 && blend.pixels[5050].b() > 0);
        state.presentation.opacity[0] = 0;
        let (_, ids) = raster(&state, &[], Vec2::splat(100.), [100, 100]);
        assert_eq!(ids[5050], 1);
        state.presentation.by_layer = true;
        state.presentation.layer_colors.insert(TOP, Color32::GREEN);
        let (layer, _) = raster(&state, &[], Vec2::splat(100.), [100, 100]);
        assert!(layer.pixels[5050].g() > 0 && layer.pixels[5050].b() == 0);
    }
    #[test]
    fn depth_picking_hides_back_components_and_selection_changes_only_pixels() {
        let id = Uuid::new_v4();
        let face = |z, owner| Face {
            triangle: Triangle {
                v: [[-1., -1., z], [1., -1., z], [0., 1., z]],
                color: Color32::GRAY,
            },
            owner,
            material: 8,
            layer: MULTI_LAYER,
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
    fn gpu_canvas_reuses_scene_and_renderer_during_navigation_without_raster_jobs() {
        let board = Board::from_legacy(&eda_core::Document::new(
            eda_core::DocumentKind::Board,
            "GPU 3D",
        ));
        let ctx = egui::Context::default();
        crate::pcb::enable_gpu(&ctx, egui_wgpu::wgpu::TextureFormat::Rgba8Unorm);
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("pcb-3d-gpu-limits"),
                egui_wgpu::wgpu::Limits::default(),
            )
        });
        let mut view = View::default();
        view.pcb.three_d.enabled = true;
        let frame = |view: &mut View| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500., 400.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        crate::pcb::canvas(ui, &board, view);
                    });
                },
            )
        };
        frame(&mut view);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while view.pcb.three_d.preparing() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(1));
            frame(&mut view);
        }
        let scene = view.pcb.three_d.scene.clone();
        let renderer = view.pcb.three_d.gpu.clone().unwrap();
        for n in 0..10 {
            view.pcb.three_d.yaw += 0.1;
            view.pcb.three_d.pan.x += 1.;
            view.pcb.three_d.zoom = 1. + n as f32 * 0.1;
            let output = frame(&mut view);
            assert!(
                output
                    .shapes
                    .iter()
                    .any(|s| matches!(s.shape, egui::Shape::Callback(_)))
            );
            assert!(Arc::ptr_eq(&scene, &view.pcb.three_d.scene));
            assert!(Arc::ptr_eq(
                &renderer,
                view.pcb.three_d.gpu.as_ref().unwrap()
            ));
            assert!(view.pcb.three_d.raster_job.is_none());
            assert!(view.pcb.three_d.texture.is_none());
        }
        let small = egui_wgpu::wgpu::Limits {
            max_buffer_size: 16,
            ..Default::default()
        };
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("pcb-3d-gpu-limits"), small));
        assert!(!gpu::supported(&ctx, &view.pcb.three_d.scene));
        ctx.data_mut(|data| {
            data.insert_temp(
                egui::Id::new("pcb-3d-gpu-limits"),
                egui_wgpu::wgpu::Limits::downlevel_webgl2_defaults(),
            )
        });
        frame(&mut view);
        assert!(
            view.pcb.three_d.raster_job.is_some(),
            "Unsupported device must retain software rendering"
        );
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
        let wait = |view: &mut View| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while view.pcb.three_d.preparing() || view.pcb.three_d.rendering() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "3D worker did not finish"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
                frame(view);
            }
        };
        wait(&mut view);
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
        wait(&mut view);
        assert!(!Arc::ptr_eq(&scene, &view.pcb.three_d.scene));
        assert_eq!(view.pcb.three_d.texture.as_ref().unwrap().id(), texture);
    }
    #[test]
    fn scene_worker_is_bounded_and_discards_stale_revisions_and_body_settings() {
        for change_bodies in [false, true] {
            let (sender, receiver) = mpsc::sync_channel(1);
            let mut state = State::default();
            let old_key = state.scene_key();
            state.scene_job = Some(Arc::new(SceneJob {
                key: old_key,
                receiver: Mutex::new(receiver),
            }));
            let job = state.scene_job.clone().unwrap();
            let original = state.scene.clone();
            let board = Board::default();
            let ctx = egui::Context::default();
            if change_bodies {
                state.show_components = false;
            } else {
                state.revision += 1;
            }
            for _ in 0..20 {
                state.prepare(&board, &ctx);
            }
            assert!(Arc::ptr_eq(&job, state.scene_job.as_ref().unwrap()));
            assert!(sender.send(Scene::default()).is_ok());
            state.prepare(&board, &ctx);
            assert!(Arc::ptr_eq(&original, &state.scene));
            assert!(state.preparing());
            assert_eq!(state.scene_job.as_ref().unwrap().key, state.scene_key());
        }
    }
    #[test]
    fn invalidation_expires_in_flight_scene_even_at_the_same_revision() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut state = State::default();
        let old_key = state.scene_key();
        state.scene_job = Some(Arc::new(SceneJob {
            key: old_key,
            receiver: Mutex::new(receiver),
        }));
        state.invalidate();
        assert!(sender.send(Scene::default()).is_ok());
        state.prepare(&Board::default(), &egui::Context::default());
        assert_ne!(state.built_key, Some(old_key));
        assert!(state.preparing());
    }
    #[test]
    fn raster_worker_coalesces_camera_changes_and_drops_old_document_pixels() {
        let ctx = egui::Context::default();
        let mut state = State::default();
        let (sender, receiver) = mpsc::sync_channel(1);
        state.raster_job = Some(Arc::new(RasterJob {
            scene: state.scene_key(),
            key: 1,
            size: [1, 1],
            receiver: Mutex::new(receiver),
        }));
        let job = state.raster_job.clone().unwrap();
        for key in 2..20 {
            state.render(key, &[], Vec2::splat(1.), [1, 1], &ctx);
            assert!(Arc::ptr_eq(&job, state.raster_job.as_ref().unwrap()));
        }
        let old_ids = state.ids.clone();
        state.revision += 1;
        assert!(
            sender
                .send((egui::ColorImage::filled([1, 1], Color32::RED), vec![42]))
                .is_ok()
        );
        state.render(20, &[], Vec2::splat(1.), [1, 1], &ctx);
        assert!(Arc::ptr_eq(&old_ids, &state.ids));
        assert!(state.texture.is_none());
        assert_ne!(state.raster_key, Some(1));
        assert_eq!(state.raster_job.as_ref().unwrap().scene, state.scene_key());
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
