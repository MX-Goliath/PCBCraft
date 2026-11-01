//! Read-only page rendering shared by Print and PDF. No editor overlays or grid.
use super::*;
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub struct Layout {
    pub paper_mm: [f64; 2],
    pub fit: bool,
    pub scale_percent: f64,
    pub offset_mm: [f64; 2],
    pub center: [bool; 2],
    pub margin_mm: f64,
}
impl Layout {
    pub fn document(doc: &Document) -> Self {
        Self {
            paper_mm: [doc.size.x as f64 / 1e6, doc.size.y as f64 / 1e6],
            fit: true,
            scale_percent: 100.,
            offset_mm: [0., 0.],
            center: [true, true],
            margin_mm: 0.,
        }
    }
    fn valid(self) -> bool {
        self.paper_mm
            .iter()
            .all(|v| v.is_finite() && *v > 0. && *v < 2000.)
            && self.scale_percent.is_finite()
            && self.scale_percent > 0.
            && self.scale_percent <= 1000.
            && self
                .offset_mm
                .iter()
                .all(|v| v.is_finite() && v.abs() < 2000.)
            && self.margin_mm.is_finite()
            && self.margin_mm >= 0.
            && self.paper_mm.iter().all(|v| 2. * self.margin_mm < *v)
    }
}

pub fn preview(ui: &mut egui::Ui, doc: &Document, rect: Rect) {
    preview_layout(ui, doc, rect, Layout::document(doc), 0);
}
pub fn preview_layout(ui: &mut egui::Ui, doc: &Document, rect: Rect, layout: Layout, color: u8) {
    if !layout.valid() {
        return;
    }
    let mut doc = doc.clone();
    if let Some(s) = &mut doc.schematic {
        s.sheet.grid_visible = false;
        s.sheet.background = 0xffffff;
        if color != 0 {
            for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
                p.color = gray(p.color, color);
                p.fill = p.fill.map(|c| gray(c, color));
            }
        }
    }
    let mut view = View::default();
    let width = doc.size.x as f64 / 1e6;
    let height = doc.size.y as f64 / 1e6;
    let scale = if layout.fit {
        ((layout.paper_mm[0] - 2. * layout.margin_mm) / width)
            .min((layout.paper_mm[1] - 2. * layout.margin_mm) / height)
    } else {
        layout.scale_percent / 100.
    };
    let pixels = rect.width() as f64 / layout.paper_mm[0];
    view.camera.pixels_per_mm = pixels * scale;
    let left = if layout.center[0] {
        (layout.paper_mm[0] - width * scale) / 2.
    } else {
        layout.margin_mm
    } + layout.offset_mm[0];
    let top = if layout.center[1] {
        (layout.paper_mm[1] - height * scale) / 2.
    } else {
        layout.margin_mm
    } + layout.offset_mm[1];
    view.camera.centre_mm = [
        (layout.paper_mm[0] / 2. - left) / scale,
        height - (layout.paper_mm[1] / 2. - top) / scale,
    ];
    view.camera.fitted = true;
    let response = ui.allocate_rect(rect, Sense::hover());
    super::schematic::paint_output(ui, &doc, &mut view, rect, &response);
}
fn gray(c: u32, mode: u8) -> u32 {
    let v = ((c & 255) * 299 + ((c >> 8) & 255) * 587 + ((c >> 16) & 255) * 114) / 1000;
    let v = if mode == 2 {
        if v < 230 { 0 } else { 255 }
    } else {
        v
    };
    v | (v << 8) | (v << 16)
}

pub struct Raster {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

/// Uses the same tessellated glyphs, primitive painter and image textures as the canvas.
/// The caller supplies an isolated egui Context configured with the application fonts.
pub fn raster(
    ctx: &egui::Context,
    doc: &Document,
    width: usize,
    height: usize,
) -> Result<Raster, String> {
    raster_layout(ctx, doc, width, height, Layout::document(doc))
}
pub fn raster_layout(
    ctx: &egui::Context,
    doc: &Document,
    width: usize,
    height: usize,
    layout: Layout,
) -> Result<Raster, String> {
    if !layout.valid() {
        return Err("Invalid print paper or scale".into());
    }
    if width < 128 || height < 128 || width.checked_mul(height).is_none_or(|n| n > 32_000_000) {
        return Err("Print raster exceeds 32 megapixels".into());
    }
    doc.validate()?;
    if doc.schematic.is_none() {
        return Err("Expected a schematic".into());
    }
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32));
    let mut textures = HashMap::<egui::TextureId, egui::ColorImage>::new();
    let mut last = None;
    // First pass installs lazily requested fonts/images; second uses the settled atlas.
    for _ in 0..2 {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(Color32::WHITE))
                    .show(ctx, |ui| preview_layout(ui, doc, rect, layout, 0));
            },
        );
        for (id, delta) in &output.textures_delta.set {
            let egui::ImageData::Color(image) = &delta.image;
            if let Some([x, y]) = delta.pos {
                let dest = textures.get_mut(id).ok_or("Missing print texture")?;
                for row in 0..image.size[1] {
                    let start = (y + row) * dest.size[0] + x;
                    dest.pixels[start..start + image.size[0]].copy_from_slice(
                        &image.pixels[row * image.size[0]..(row + 1) * image.size[0]],
                    );
                }
            } else {
                textures.insert(*id, (**image).clone());
            }
        }
        last = Some(output);
    }
    let output = last.unwrap();
    let meshes = ctx.tessellate(output.shapes, output.pixels_per_point);
    let mut rgb = vec![255; width * height * 3];
    for clipped in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = clipped.primitive else {
            return Err("Custom GPU callbacks cannot be printed".into());
        };
        let texture = textures
            .get(&mesh.texture_id)
            .ok_or("A print image texture is unavailable")?;
        let clip = clipped.clip_rect.intersect(rect);
        for triangle in mesh.indices.chunks_exact(3) {
            let v = [
                mesh.vertices[triangle[0] as usize],
                mesh.vertices[triangle[1] as usize],
                mesh.vertices[triangle[2] as usize],
            ];
            let edge =
                |a: Pos2, b: Pos2, p: Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let area = edge(v[0].pos, v[1].pos, v[2].pos);
            if area.abs() < 1e-7 {
                continue;
            }
            let min = v
                .iter()
                .fold(Pos2::new(f32::INFINITY, f32::INFINITY), |a, v| a.min(v.pos))
                .max(clip.min);
            let max = v
                .iter()
                .fold(Pos2::new(f32::NEG_INFINITY, f32::NEG_INFINITY), |a, v| {
                    a.max(v.pos)
                })
                .min(clip.max);
            let min_x = min.x.floor().max(0.) as usize;
            let min_y = min.y.floor().max(0.) as usize;
            let max_x = (max.x.ceil().max(0.) as usize).min(width);
            let max_y = (max.y.ceil().max(0.) as usize).min(height);
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                    let weights = [
                        edge(v[1].pos, v[2].pos, p) / area,
                        edge(v[2].pos, v[0].pos, p) / area,
                        edge(v[0].pos, v[1].pos, p) / area,
                    ];
                    if weights.iter().any(|w| *w < -1e-6) {
                        continue;
                    }
                    let mut uv = Vec2::ZERO;
                    let mut color = [0f32; 4];
                    for n in 0..3 {
                        uv += v[n].uv.to_vec2() * weights[n];
                        for (c, value) in color.iter_mut().enumerate() {
                            *value += v[n].color[c] as f32 * weights[n];
                        }
                    }
                    let tx = (uv.x * texture.size[0] as f32 - 0.5)
                        .clamp(0., texture.size[0] as f32 - 1.);
                    let ty = (uv.y * texture.size[1] as f32 - 0.5)
                        .clamp(0., texture.size[1] as f32 - 1.);
                    let (x0, y0) = (tx.floor() as usize, ty.floor() as usize);
                    let (fx, fy) = (tx - x0 as f32, ty - y0 as f32);
                    let mut sample = [0f32; 4];
                    for (xx, wx) in [(x0, 1. - fx), ((x0 + 1).min(texture.size[0] - 1), fx)] {
                        for (yy, wy) in [(y0, 1. - fy), ((y0 + 1).min(texture.size[1] - 1), fy)] {
                            let c = texture.pixels[yy * texture.size[0] + xx];
                            for n in 0..4 {
                                sample[n] += c[n] as f32 * wx * wy;
                            }
                        }
                    }
                    let alpha = color[3] * sample[3] / 65025.;
                    let pixel = (y * width + x) * 3;
                    for n in 0..3 {
                        let source = color[n] * sample[n] / 255.;
                        rgb[pixel + n] = (source + rgb[pixel + n] as f32 * (1. - alpha))
                            .clamp(0., 255.)
                            .round() as u8;
                    }
                }
            }
        }
    }
    Ok(Raster { width, height, rgb })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn print_raster_contains_sheet_and_text_without_mutating_document() {
        let mut doc = Document::new(DocumentKind::Schematic, "print");
        doc.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(eda_core::schematic_edit::primitive(
                eda_core::schematic::SchPrimitiveKind::Text {
                    position: Point::mm(40, 70),
                    text: "R1".into(),
                    font: 0,
                    orientation: 0,
                    justification: 0,
                },
                None,
            ));
        let before = doc.clone();
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        let family = fonts.families[&egui::FontFamily::Proportional].clone();
        for name in [
            "sch-sans",
            "sch-serif",
            "sch-mono",
            "sch-gost",
            "sch-altium-gost",
        ] {
            fonts
                .families
                .insert(egui::FontFamily::Name(name.into()), family.clone());
        }
        ctx.set_fonts(fonts);
        let image = raster(&ctx, &doc, 1000, 660).unwrap();
        assert_eq!(image.rgb.len(), 1000 * 660 * 3);
        assert!(image.rgb.iter().any(|v| *v < 30));
        assert_eq!(doc, before);
        assert!(raster(&egui::Context::default(), &doc, 100_000, 100_000).is_err());
    }
}
