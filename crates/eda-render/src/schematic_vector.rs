//! Flatten the same read-only schematic painter to independent document-space faces.
//! Fonts become outlines, so DXF recipients need no installed schematic fonts.
use super::*;
use eda_geom::DrawingFace;
use egui::{
    Shape,
    epaint::{ClippedShape, TextShape},
};

const PIXELS_PER_MM: f64 = 20.;
const MAX_FACES: usize = 250_000;

pub fn drawing(ctx: &egui::Context, doc: &Document) -> Result<Vec<DrawingFace>, String> {
    doc.validate()?;
    let schematic = doc.schematic.as_ref().ok_or("Expected schematic")?;
    if schematic
        .primitives
        .iter()
        .chain(&schematic.parameter_objects)
        .any(|p| {
            !p.presentation.hidden
                && matches!(
                    p.kind,
                    eda_core::schematic::SchPrimitiveKind::Image { .. }
                        | eda_core::schematic::SchPrimitiveKind::Unsupported { .. }
                )
        })
    {
        return Err(
            "DXF drawing export cannot encode raster images or unsupported objects; use Smart PDF"
                .into(),
        );
    }
    let rect = Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(
            (doc.size.x as f64 / 1e6 * PIXELS_PER_MM) as f32,
            (doc.size.y as f64 / 1e6 * PIXELS_PER_MM) as f32,
        ),
    );
    ctx.tessellation_options_mut(|o| {
        o.feathering = false;
        o.prerasterized_discs = false;
        o.round_line_segments_to_pixels = false;
        o.round_rects_to_pixels = false;
    });
    let mut output = None;
    for _ in 0..2 {
        output = Some(ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| super::schematic_output::preview(ui, doc, rect));
            },
        ));
    }
    let fonts = ctx.fonts(|f| f.lock().fonts.definitions().clone());
    let mut collector = Collector {
        ctx,
        fonts,
        rect,
        faces: vec![],
    };
    for shape in output.unwrap().shapes {
        collector.shape(shape.shape, shape.clip_rect.intersect(rect))?;
    }
    Ok(collector.faces)
}
struct Collector<'a> {
    ctx: &'a egui::Context,
    fonts: egui::FontDefinitions,
    rect: Rect,
    faces: Vec<DrawingFace>,
}
impl Collector<'_> {
    fn triangle(&mut self, points: [Pos2; 3], color: Color32, clip: Rect) -> Result<(), String> {
        if color.a() == 0 || !clip.is_positive() {
            return Ok(());
        }
        // Clip faces rather than exporting drawing outside the selected page.
        let mut polygon = points.to_vec();
        for (axis, edge, greater) in [
            (0, clip.left(), true),
            (0, clip.right(), false),
            (1, clip.top(), true),
            (1, clip.bottom(), false),
        ] {
            let old = std::mem::take(&mut polygon);
            if old.is_empty() {
                return Ok(());
            }
            for i in 0..old.len() {
                let a = old[i];
                let b = old[(i + 1) % old.len()];
                let va = if axis == 0 { a.x } else { a.y };
                let vb = if axis == 0 { b.x } else { b.y };
                let inside = |v: f32| if greater { v >= edge } else { v <= edge };
                if inside(va) {
                    polygon.push(a);
                }
                if inside(va) != inside(vb) {
                    polygon.push(a + (b - a) * ((edge - va) / (vb - va)));
                }
            }
        }
        let convert = |p: Pos2| Point {
            x: (p.x as f64 / PIXELS_PER_MM * 1e6).round() as i64,
            y: ((self.rect.height() - p.y) as f64 / PIXELS_PER_MM * 1e6).round() as i64,
        };
        let rgb = color.to_array();
        for i in 1..polygon.len().saturating_sub(1) {
            let points = [
                convert(polygon[0]),
                convert(polygon[i]),
                convert(polygon[i + 1]),
            ];
            if points[2].on_segment(points[0], points[1]) {
                continue;
            }
            if self.faces.len() >= MAX_FACES {
                return Err(
                    "DXF drawing exceeds 250,000 faces; export a smaller drawing or use Smart PDF"
                        .into(),
                );
            }
            self.faces.push(DrawingFace {
                points,
                rgb: [rgb[0], rgb[1], rgb[2]],
            });
        }
        Ok(())
    }
    fn shape(&mut self, shape: Shape, clip: Rect) -> Result<(), String> {
        match shape {
            Shape::Noop => Ok(()),
            Shape::Vec(shapes) => {
                for shape in shapes {
                    self.shape(shape, clip)?;
                }
                Ok(())
            }
            Shape::Text(text) => self.text(text, clip),
            shape => {
                for primitive in self.ctx.tessellate(
                    vec![ClippedShape {
                        shape,
                        clip_rect: clip,
                    }],
                    1.,
                ) {
                    let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                        return Err("GPU callbacks cannot be exported to DXF".into());
                    };
                    if mesh.vertices.iter().any(|v| v.uv != egui::epaint::WHITE_UV) {
                        return Err("Textured drawing objects cannot be exported to DXF".into());
                    }
                    for face in mesh.indices.chunks_exact(3) {
                        let a = mesh.vertices[face[0] as usize];
                        let b = mesh.vertices[face[1] as usize];
                        let c = mesh.vertices[face[2] as usize];
                        if a.color != b.color || a.color != c.color || a.color.a() != 255 {
                            return Err(
                                "Translucent/gradient drawing objects cannot be exported to DXF"
                                    .into(),
                            );
                        }
                        self.triangle([a.pos, b.pos, c.pos], a.color, clip)?;
                    }
                }
                Ok(())
            }
        }
    }
    fn text(&mut self, text: TextShape, clip: Rect) -> Result<(), String> {
        let section = text
            .galley
            .job
            .sections
            .first()
            .ok_or("Missing drawing text format")?;
        if text.galley.job.sections.len() != 1 {
            return Err("Mixed-format drawing text cannot be exported".into());
        }
        let names = self
            .fonts
            .families
            .get(&section.format.font_id.family)
            .ok_or("Missing drawing font family")?
            .clone();
        let rotate = egui::emath::Rot2::from_angle(text.angle);
        let color = text.override_text_color.unwrap_or(section.format.color);
        let color = if color == Color32::PLACEHOLDER {
            text.fallback_color
        } else {
            color
        };
        for row in &text.galley.rows {
            let Some(last) = row.glyphs.last() else {
                continue;
            };
            let original_width = last.pos.x + last.advance_width;
            if original_width <= 0. {
                continue;
            }
            let scale = row.size.x / original_width;
            for glyph in &row.glyphs {
                if glyph.chr.is_whitespace() {
                    continue;
                }
                let (data, gid) = names
                    .iter()
                    .find_map(|name| {
                        let data = self.fonts.font_data.get(name)?;
                        let face = ttf_parser::Face::parse(&data.font, data.index).ok()?;
                        Some((data.clone(), face.glyph_index(glyph.chr)?))
                    })
                    .ok_or_else(|| {
                        format!("No outline font for drawing character {}", glyph.chr)
                    })?;
                let face = ttf_parser::Face::parse(&data.font, data.index)
                    .map_err(|_| "Invalid drawing font")?;
                let metrics = (face.ascender() as f32 - face.descender() as f32
                    + face.line_gap() as f32)
                    .max(1.);
                let font_scale = glyph.font_impl_height / metrics * data.tweak.scale;
                let mut outline = Outline::default();
                if face.outline_glyph(gid, &mut outline).is_none() {
                    return Err(format!("Character {} has no vector outline", glyph.chr));
                }
                let transform = |p: Pos2| {
                    let local = Vec2::new(
                        glyph.pos.x + p.x * font_scale,
                        glyph.pos.y - p.y * font_scale,
                    );
                    text.pos + rotate * (row.pos.to_vec2() + local * scale)
                };
                for triangle in outline.triangulate()? {
                    self.triangle(triangle.map(transform), color, clip)?;
                }
            }
            if !text.underline.is_empty() {
                let y = row.size.y;
                let a = text.pos + rotate * (row.pos.to_vec2() + Vec2::new(0., y));
                let b = text.pos + rotate * (row.pos.to_vec2() + Vec2::new(row.size.x, y));
                self.shape(Shape::line_segment([a, b], text.underline), clip)?;
            }
        }
        Ok(())
    }
}
#[derive(Default)]
struct Outline {
    contours: Vec<Vec<Pos2>>,
}
impl Outline {
    fn push(&mut self, p: Pos2) {
        self.contours.last_mut().unwrap().push(p);
    }
    fn triangulate(self) -> Result<Vec<[Pos2; 3]>, String> {
        let contours: Vec<_> = self.contours.into_iter().filter(|c| c.len() >= 3).collect();
        let contains = |c: &[Pos2], p: Pos2| {
            let mut inside = false;
            for i in 0..c.len() {
                let a = c[i];
                let b = c[(i + 1) % c.len()];
                if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x
                {
                    inside = !inside;
                }
            }
            inside
        };
        let depth: Vec<_> = contours
            .iter()
            .map(|c| {
                contours
                    .iter()
                    .filter(|other| !std::ptr::eq(*other, c) && contains(other, c[0]))
                    .count()
            })
            .collect();
        let mut result = vec![];
        for (i, outer) in contours
            .iter()
            .enumerate()
            .filter(|(i, _)| depth[*i] % 2 == 0)
        {
            let mut points = outer.clone();
            let mut holes = vec![];
            for (j, hole) in contours.iter().enumerate() {
                if depth[j] == depth[i] + 1 && contains(outer, hole[0]) {
                    holes.push(points.len());
                    points.extend(hole);
                }
            }
            let flat: Vec<_> = points
                .iter()
                .flat_map(|p| [p.x as f64, p.y as f64])
                .collect();
            let indices = earcutr::earcut(&flat, &holes, 2)
                .map_err(|_| "Cannot triangulate drawing glyph")?;
            for t in indices.chunks_exact(3) {
                result.push([points[t[0]], points[t[1]], points[t[2]]]);
            }
        }
        Ok(result)
    }
}
impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![Pos2::new(x, y)]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push(Pos2::new(x, y));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let a = *self.contours.last().unwrap().last().unwrap();
        let b = Pos2::new(x1, y1);
        let c = Pos2::new(x, y);
        for i in 1..=12 {
            let t = i as f32 / 12.;
            let u = 1. - t;
            self.push(
                (a.to_vec2() * u * u + b.to_vec2() * 2. * u * t + c.to_vec2() * t * t).to_pos2(),
            );
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let a = *self.contours.last().unwrap().last().unwrap();
        let b = Pos2::new(x1, y1);
        let c = Pos2::new(x2, y2);
        let d = Pos2::new(x, y);
        for i in 1..=16 {
            let t = i as f32 / 16.;
            let u = 1. - t;
            self.push(
                (a.to_vec2() * u * u * u
                    + b.to_vec2() * 3. * u * u * t
                    + c.to_vec2() * 3. * u * t * t
                    + d.to_vec2() * t * t * t)
                    .to_pos2(),
            );
        }
    }
    fn close(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn outlined_font_counters_remain_holes() {
        let outline = Outline {
            contours: vec![
                vec![
                    Pos2::new(0., 0.),
                    Pos2::new(20., 0.),
                    Pos2::new(20., 20.),
                    Pos2::new(0., 20.),
                ],
                vec![
                    Pos2::new(5., 5.),
                    Pos2::new(15., 5.),
                    Pos2::new(15., 15.),
                    Pos2::new(5., 15.),
                ],
            ],
        };
        let area: f32 = outline
            .triangulate()
            .unwrap()
            .into_iter()
            .map(|p| {
                ((p[1].x - p[0].x) * (p[2].y - p[0].y) - (p[1].y - p[0].y) * (p[2].x - p[0].x))
                    .abs()
                    / 2.
            })
            .sum();
        assert!((area - 300.).abs() < 0.01);
    }
}
