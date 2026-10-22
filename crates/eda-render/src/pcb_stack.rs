//! Interactive 3D padstack preview with a portable software depth buffer.
use eda_core::{pcb::*, pcb_polygon};
use eda_geom::Point;
use egui::{Color32, Rect, Sense, Vec2};
#[derive(Clone)]
pub struct PreviewState {
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    drag_position: Option<egui::Pos2>,
    texture: Option<egui::TextureHandle>,
    raster_key: u64,
}
impl Default for PreviewState {
    fn default() -> Self {
        Self {
            yaw: 0.74,
            pitch: 0.56,
            zoom: 1.0,
            drag_position: None,
            texture: None,
            raster_key: 0,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct Triangle {
    pub v: [[f32; 3]; 3],
    pub color: Color32,
}
pub(crate) fn surface(
    faces: &mut Vec<Triangle>,
    outer: &[Point],
    hole: &[Point],
    z: f32,
    color: Color32,
    origin: Point,
) {
    for face in cutout_surface(outer, hole) {
        let mut coords = vec![];
        let mut holes = vec![];
        for (n, ring) in std::iter::once(&face.outline)
            .chain(&face.holes)
            .enumerate()
        {
            if n > 0 {
                holes.push(coords.len() / 2);
            }
            for p in ring {
                coords.extend([(p.x - origin.x) as f64 / 1e6, (p.y - origin.y) as f64 / 1e6]);
            }
        }
        let indices = earcutr::earcut(&coords, &holes, 2).unwrap_or_default();
        for t in indices.chunks_exact(3) {
            faces.push(Triangle {
                v: t.try_into()
                    .map(|a: [usize; 3]| {
                        a.map(|i| [coords[i * 2] as f32, coords[i * 2 + 1] as f32, z])
                    })
                    .unwrap(),
                color,
            });
        }
    }
}
pub(crate) fn wall(
    faces: &mut Vec<Triangle>,
    points: &[Point],
    z0: f32,
    z1: f32,
    color: Color32,
    origin: Point,
) {
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        let p = |p: &Point, z| {
            [
                (p.x - origin.x) as f32 / 1e6,
                (p.y - origin.y) as f32 / 1e6,
                z,
            ]
        };
        faces.push(Triangle {
            v: [p(a, z0), p(b, z0), p(b, z1)],
            color,
        });
        faces.push(Triangle {
            v: [p(a, z0), p(b, z1), p(a, z1)],
            color,
        });
    }
}
fn geometry(board: &Board, item: &Item) -> Vec<Triangle> {
    let mut faces = vec![];
    let origin = item.anchor();
    let hole = item.drill_outline();
    let mut layers: Vec<_> = board
        .layers
        .iter()
        .filter(|l| l.copper && pcb_polygon::on_layer(board, item, l.id))
        .collect();
    layers.sort_by_key(|l| l.order);
    if layers.is_empty() {
        return faces;
    }
    let height = board.stack_thickness_nm() as f32 / 1e6;
    let mut all: Vec<_> = board.layers.iter().filter(|l| l.copper).collect();
    all.sort_by_key(|l| l.order);
    let z_for = |n: usize| {
        let index = all.iter().position(|l| l.id == layers[n].id).unwrap_or(n);
        height / 2.0 - height * index as f32 / (all.len() - 1).max(1) as f32
    };
    for (n, l) in layers.iter().enumerate() {
        let z = z_for(n);
        let outline = item.pad_outline(l.id);
        let color = match l.id {
            TOP => Color32::from_rgb(155, 4, 15),
            BOTTOM => Color32::from_rgb(12, 57, 116),
            _ => Color32::from_rgb(158, 114, 12),
        };
        surface(&mut faces, &outline, &hole, z, color, origin);
        surface(&mut faces, &outline, &hole, z - 0.035, color, origin);
        wall(
            &mut faces,
            &outline,
            z - 0.035,
            z,
            color.gamma_multiply(0.65),
            origin,
        );
    }
    if hole.len() >= 3 {
        let plated = matches!(
            item.shape,
            Shape::Via { .. } | Shape::Pad { plated: true, .. }
        );
        wall(
            &mut faces,
            &hole,
            z_for(layers.len() - 1),
            z_for(0),
            if plated {
                Color32::from_gray(135)
            } else {
                Color32::from_gray(22)
            },
            origin,
        );
    }
    // Native preview includes colored outer layer patches around the land.
    let mut bounds = Bounds::points(&hole);
    for l in &layers {
        bounds.union(Bounds::points(&item.pad_outline(l.id)));
    }
    let size = Point {
        x: ((bounds.max.x - bounds.min.x) as f64 * 1.65).round() as i64,
        y: ((bounds.max.y - bounds.min.y) as f64 * 1.65).round() as i64,
    };
    let context = rectangle(bounds.centre(), size, 0.0);
    for (bottom, layer, z) in [
        (false, TOP, height / 2.0 + 0.04),
        (true, BOTTOM, -height / 2.0 - 0.04),
    ] {
        if !layers.iter().any(|l| l.id == layer) {
            continue;
        }
        if let Ok(aperture) = item.mask_outline(board, MaskKind::Solder, bottom) {
            let color = if matches!(item.shape, Shape::Pad { plated: false, .. }) {
                Color32::from_rgb(87, 0, 103)
            } else if bottom {
                Color32::from_rgb(8, 45, 108)
            } else {
                Color32::from_rgb(145, 0, 22)
            };
            surface(&mut faces, &context, &aperture, z, color, origin);
            wall(
                &mut faces,
                &aperture,
                z - 0.018,
                z,
                color.gamma_multiply(0.6),
                origin,
            );
        }
        if let Ok(paste) = item.mask_outline(board, MaskKind::Paste, bottom)
            && !paste.is_empty()
        {
            surface(
                &mut faces,
                &paste,
                &hole,
                z + if bottom { -0.025 } else { 0.025 },
                Color32::from_gray(170),
                origin,
            );
        }
    }
    faces
}
fn project(v: [f32; 3], state: &PreviewState) -> [f32; 3] {
    let (s, c) = state.yaw.sin_cos();
    let x = v[0] * c - v[1] * s;
    let y = v[0] * s + v[1] * c;
    let (sp, cp) = state.pitch.sin_cos();
    [x, y * sp - v[2] * cp, y * cp + v[2] * sp]
}
pub(crate) fn shade(face: &Triangle) -> Color32 {
    let [a, b, c] = face.v;
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let norm = n.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-8);
    let light =
        (0.35 + 0.65 * ((n[0] * 0.3 - n[1] * 0.45 + n[2] * 0.84) / norm).abs()).clamp(0.25, 1.0);
    face.color.gamma_multiply(light)
}
fn rasterize(
    faces: &[Triangle],
    centre: egui::Pos2,
    scale: f32,
    logical: Vec2,
    size: [usize; 2],
) -> egui::ColorImage {
    let [width, height] = size;
    let mut image = egui::ColorImage::filled(size, Color32::from_gray(32));
    let mut depth = vec![f32::NEG_INFINITY; width * height];
    let edge = |a: [f32; 3], b: [f32; 3], x: f32, y: f32| {
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    for face in faces {
        let v = face.v.map(|p| {
            [
                ((p[0] - centre.x) * scale + logical.x / 2.0) * width as f32 / logical.x,
                ((p[1] - centre.y) * scale + logical.y / 2.0) * height as f32 / logical.y,
                p[2],
            ]
        });
        if !v.iter().flatten().all(|v| v.is_finite()) {
            continue;
        }
        let area = edge(v[0], v[1], v[2][0], v[2][1]);
        if area.abs() < 1e-6 {
            continue;
        }
        let min = |n: usize, limit: usize| {
            v.iter()
                .map(|p| p[n])
                .fold(f32::INFINITY, f32::min)
                .floor()
                .clamp(0.0, (limit - 1) as f32) as usize
        };
        let max = |n: usize, limit: usize| {
            v.iter()
                .map(|p| p[n])
                .fold(f32::NEG_INFINITY, f32::max)
                .ceil()
                .clamp(0.0, (limit - 1) as f32) as usize
        };
        let color = shade(face);
        for y in min(1, height)..=max(1, height) {
            for x in min(0, width)..=max(0, width) {
                let px = x as f32 + 0.5;
                let py = y as f32 + 0.5;
                let a = edge(v[1], v[2], px, py) / area;
                let b = edge(v[2], v[0], px, py) / area;
                let c = 1.0 - a - b;
                if a < -1e-5 || b < -1e-5 || c < -1e-5 {
                    continue;
                }
                let z = a * v[0][2] + b * v[1][2] + c * v[2][2];
                let index = y * width + x;
                if z > depth[index] {
                    depth[index] = z;
                    image.pixels[index] = color;
                }
            }
        }
    }
    image
}
pub fn preview(
    ui: &mut egui::Ui,
    board: &Board,
    item: &Item,
    state: &mut PreviewState,
    height: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    ui.painter().rect_filled(rect, 3.0, Color32::from_gray(32));
    let (press, down, pos, focused) = ui.input(|i| {
        (
            i.events.iter().find_map(|event| {
                if let egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } = event
                {
                    Some(*pos)
                } else {
                    None
                }
            }),
            i.pointer.primary_down(),
            i.pointer.interact_pos(),
            i.focused,
        )
    });
    if let Some(press) = press
        && rect.contains(press)
        && ui.clip_rect().contains(press)
        && ui.ctx().layer_id_at(press) == Some(ui.layer_id())
    {
        state.drag_position = Some(press);
    }
    if let Some(previous) = state.drag_position {
        if let Some(pos) = pos {
            let delta = pos - previous;
            state.yaw += delta.x * 0.012;
            state.pitch = (state.pitch + delta.y * 0.012).clamp(-1.45, 1.45);
            state.drag_position = Some(pos);
        }
        if !down || !focused {
            state.drag_position = None;
        }
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            state.zoom = (state.zoom * (scroll * 0.003).exp()).clamp(0.3, 3.0);
            ui.input_mut(|i| {
                i.smooth_scroll_delta = Vec2::ZERO;
                i.raw_scroll_delta = Vec2::ZERO;
            });
        }
    }
    if response.double_clicked() {
        *state = Default::default();
    }
    let faces: Vec<_> = geometry(board, item)
        .into_iter()
        .map(|t| Triangle {
            v: t.v.map(|v| project(v, state)),
            color: t.color,
        })
        .collect();
    let mut fit = Rect::NOTHING;
    for face in &faces {
        for v in face.v {
            fit.extend_with(egui::pos2(v[0], v[1]));
        }
    }
    if !fit.is_finite() || fit.width() <= 0.0 {
        return response;
    }
    let scale = ((rect.width() - 20.0) / fit.width()).min((rect.height() - 20.0) / fit.height())
        * state.zoom;
    let centre = fit.center();
    let pixels = (ui.ctx().pixels_per_point() * 2.0).clamp(1.0, 4.0);
    let size = [
        (rect.width() * pixels).ceil().clamp(1.0, 1024.0) as usize,
        (rect.height() * pixels).ceil().clamp(1.0, 1024.0) as usize,
    ];
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    size.hash(&mut hash);
    scale.to_bits().hash(&mut hash);
    for t in &faces {
        for v in t.v {
            for n in v {
                n.to_bits().hash(&mut hash);
            }
        }
        t.color.to_array().hash(&mut hash);
    }
    let key = hash.finish();
    if state.texture.is_none() || state.raster_key != key {
        let image = rasterize(&faces, centre, scale, rect.size(), size);
        if let Some(texture) = &mut state.texture {
            texture.set(image, egui::TextureOptions::LINEAR);
        } else {
            state.texture = Some(ui.ctx().load_texture(
                format!("pcb-stack-{:?}", response.id),
                image,
                egui::TextureOptions::LINEAR,
            ));
        }
        state.raster_key = key;
    }
    ui.painter().with_clip_rect(rect).image(
        state.texture.as_ref().unwrap().id(),
        rect,
        Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    response.on_hover_text("Drag to rotate · Wheel to zoom · Double-click to reset")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depth_buffer_hides_rear_faces_independently_of_triangle_order() {
        let front = Triangle {
            v: [[-1.0, -1.0, 2.0], [1.0, -1.0, 2.0], [-1.0, 1.0, 2.0]],
            color: Color32::BLUE,
        };
        let back = Triangle {
            v: [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [-1.0, 1.0, 0.0]],
            color: Color32::RED,
        };
        let render = |faces: &[Triangle]| {
            rasterize(faces, egui::Pos2::ZERO, 8.0, Vec2::splat(32.0), [32, 32])
        };
        let a = render(&[front.clone(), back.clone()]);
        let b = render(&[back, front.clone()]);
        assert_eq!(a.pixels, b.pixels);
        assert_eq!(a.pixels[12 * 32 + 12], shade(&front));
    }
    fn via() -> Item {
        Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::default(),
                diameter: 1_000_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        )
    }
    #[test]
    fn fast_pointer_drag_rotates_preview_without_a_model_transaction() {
        let ctx = egui::Context::default();
        let b = Board::default();
        let v = via();
        let before = v.clone();
        let mut state = PreviewState::default();
        let initial = state.yaw;
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(316.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        preview(ui, &b, &v, &mut state, 230.0);
                    });
                },
            );
        };
        frame(vec![]);
        frame(vec![]);
        let from = egui::pos2(80.0, 100.0);
        let to = egui::pos2(160.0, 120.0);
        frame(vec![
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ]);
        frame(vec![
            egui::Event::PointerMoved(to),
            egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]);
        assert_ne!(state.yaw, initial);
        assert_eq!(v, before);
    }
    #[test]
    fn meshes_include_open_hole_walls_and_tenting_changes_top_surface() {
        let b = Board::default();
        let mut v = via();
        let open = geometry(&b, &v);
        assert!(!open.is_empty());
        assert!(open.iter().flat_map(|t| t.v).flatten().all(f32::is_finite));
        assert!(open.iter().any(|t| t.v[0][2] != t.v[2][2]));
        let mut m = MaskSettings::for_item(&v);
        m.solder[0].tented = true;
        v.masks = Some(Box::new(m));
        let tented = geometry(&b, &v);
        assert_ne!(open.len(), tented.len());
    }
    #[test]
    fn blind_via_barrel_stops_at_internal_layer_and_projection_rotates() {
        let mut b = Board::default();
        b.layers.insert(
            1,
            PcbLayer {
                id: 2,
                name: "Mid".into(),
                copper: true,
                order: 1,
            },
        );
        b.layers.iter_mut().find(|l| l.id == BOTTOM).unwrap().order = 2;
        let mut v = via();
        if let Shape::Via { to_layer, .. } = &mut v.shape {
            *to_layer = 2;
        }
        let faces = geometry(&b, &v);
        assert!(faces.iter().flat_map(|t| t.v).all(|v| v[2] >= -0.0351));
        let a = project([1.0, 0.0, 0.0], &PreviewState::default());
        let z = project(
            [1.0, 0.0, 0.0],
            &PreviewState {
                yaw: 0.0,
                ..Default::default()
            },
        );
        assert_ne!(a, z);
    }
    #[test]
    fn preview_rotation_only_changes_view_and_all_emitted_vertices_are_finite() {
        let ctx = egui::Context::default();
        let board = Board::default();
        let item = via();
        let before = item.clone();
        let mut state = PreviewState::default();
        let out = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                preview(ui, &board, &item, &mut state, 230.0);
            });
        });
        assert_eq!(item, before);
        for primitive in ctx.tessellate(out.shapes, out.pixels_per_point) {
            if let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive {
                assert!(mesh.vertices.iter().all(|v| v.pos.is_finite()));
            }
        }
    }
}
