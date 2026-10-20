use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::schematic::{SchFont, SchPrimitive, SchPrimitiveKind as K, Schematic};
use egui::{Align2, FontFamily, FontId, Painter, Shape, epaint::TextShape};
use std::collections::HashMap;

fn color(value: u32) -> Color32 {
    Color32::from_rgb(value as u8, (value >> 8) as u8, (value >> 16) as u8)
}
fn direction(orientation: u32) -> Vec2 {
    match orientation % 4 {
        0 => Vec2::new(1.0, 0.0),
        1 => Vec2::new(0.0, -1.0),
        2 => Vec2::new(-1.0, 0.0),
        _ => Vec2::new(0.0, 1.0),
    }
}
fn font_family(font: &SchFont) -> FontFamily {
    let name = font.name.to_ascii_lowercase();
    let face = if name == "altium gost type a" {
        "sch-altium-gost"
    } else if name.contains("gost") {
        "sch-gost"
    } else if name.contains("times") || name.contains("serif") {
        "sch-serif"
    } else if name.contains("courier") || name.contains("mono") {
        "sch-mono"
    } else {
        "sch-sans"
    };
    FontFamily::Name(
        format!(
            "{face}{}{}",
            if font.bold { "-bold" } else { "" },
            if font.italic { "-italic" } else { "" }
        )
        .into(),
    )
}

struct Scene<'a> {
    painter: Painter,
    camera: &'a Camera,
    rect: Rect,
    schematic: Option<&'a Schematic>,
    document_id: Uuid,
    library_preview: bool,
}

pub(super) fn symbol_preview(
    ui: &mut egui::Ui,
    fragment: &eda_core::schematic_edit::Fragment,
    height: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click_and_drag(),
    );
    let mut primitives = fragment.primitives.clone();
    for p in &mut primitives {
        if (p.text_role == Some(eda_core::schematic::TextRole::Comment)
            || p.presentation
                .raw_text
                .as_ref()
                .is_some_and(|s| s.starts_with('=')))
            && let K::Text { text, .. } = &mut p.kind
            && let Some(raw) = &p.presentation.raw_text
        {
            *text = raw.clone();
            p.color = 0x808080;
        }
    }
    let mut schematic = Schematic::blank();
    schematic.fonts = fragment.fonts.clone();
    schematic.images = fragment.images.clone();
    let id = fragment.components.first().map_or(Uuid::nil(), |c| c.id);
    let mut camera = Camera::default();
    let measure = Scene {
        painter: ui.painter().with_clip_rect(Rect::NOTHING),
        camera: &camera,
        rect,
        schematic: Some(&schematic),
        document_id: id,
        library_preview: true,
    };
    let mut bounds = Rect::NOTHING;
    for p in primitives.iter().filter(|p| !p.presentation.hidden) {
        bounds = bounds.union(measure.primitive(p, None).0);
    }
    if bounds.is_finite() && bounds.is_positive() {
        camera.centre_mm = camera.world_mm(bounds.center(), rect);
        camera.pixels_per_mm *= ((rect.width() - 30.0) / bounds.width())
            .min((rect.height() - 50.0) / bounds.height())
            .clamp(0.01, 20.0) as f64;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::WHITE);
    let scene = Scene {
        painter,
        camera: &camera,
        rect,
        schematic: Some(&schematic),
        document_id: id,
        library_preview: true,
    };
    for p in primitives.iter().filter(|p| !p.presentation.hidden) {
        scene.primitive(p, None);
    }
    response
}
impl Scene<'_> {
    fn screen(&self, p: Point) -> Pos2 {
        self.camera.screen(p, self.rect)
    }
    fn nm(&self, n: i64) -> f32 {
        (n as f64 / 1e6 * self.camera.pixels_per_mm) as f32
    }
    fn line(&self, a: Pos2, b: Pos2, stroke: Stroke) {
        // GDI-style cosmetic strokes: an integral number of physical pixels.
        // Keep topology/world coordinates intact when a view is zoomed.
        let ppp = self.painter.ctx().pixels_per_point();
        // Keep a visible one-point minimum at sheet fit.
        // A one-physical-pixel mesh can disappear when the desktop uses 150% DPI.
        let pixels = (stroke.width.max(1.0) * ppp).round().max(1.0);
        let width = pixels / ppp;
        if (a.x - b.x).abs() < 0.001 || (a.y - b.y).abs() < 0.001 {
            let phase = if pixels as u32 % 2 == 1 { 0.5 } else { 0.0 };
            let snap = |v: f32| ((v * ppp).round() + phase) / ppp;
            let (a, b) = if (a.x - b.x).abs() < 0.001 {
                (Pos2::new(snap(a.x), a.y), Pos2::new(snap(a.x), b.y))
            } else {
                (Pos2::new(a.x, snap(a.y)), Pos2::new(b.x, snap(a.y)))
            };
            self.painter
                .line_segment([a, b], Stroke::new(width, stroke.color));
        } else {
            self.painter
                .line_segment([a, b], Stroke::new(width, stroke.color));
        }
    }
    fn line_end(&self, tip: Pos2, previous: Pos2, shape: u32, size: u32, stroke: Stroke) {
        if shape != 1 || tip == previous {
            return;
        }
        let d = (tip - previous).normalized();
        let side = Vec2::new(-d.y, d.x);
        let length = self.nm(1_016_000 * (size.min(4) as i64 + 1));
        self.poly(
            vec![
                tip - d * length + side * length * 0.4,
                tip,
                tip - d * length - side * length * 0.4,
            ],
            false,
            None,
            stroke,
        );
    }
    fn text(
        &self,
        position: Pos2,
        text: &str,
        font: usize,
        orientation: u32,
        justification: u32,
        c: Color32,
    ) -> Rect {
        let fallback = SchFont {
            name: "Arial".into(),
            height_nm: 2_540_000,
            bold: false,
            italic: false,
            underline: false,
        };
        let font = self
            .schematic
            .and_then(|s| s.fonts.get(font))
            .unwrap_or(&fallback);
        let size = (self.nm(font.height_nm) * 0.875).clamp(1.0, 512.0);
        if size < 2.2 {
            return Rect::NOTHING;
        }
        let galley =
            self.painter
                .layout_no_wrap(text.to_owned(), FontId::new(size, font_family(font)), c);
        let justification = justification.min(8);
        let justification = if orientation % 4 >= 2 {
            8 - justification
        } else {
            justification
        };
        let h = justification % 3;
        let v = (justification / 3).min(2);
        let anchor = Align2([
            match h {
                1 => egui::Align::Center,
                2 => egui::Align::Max,
                _ => egui::Align::Min,
            },
            match v {
                1 => egui::Align::Center,
                2 => egui::Align::Min,
                _ => egui::Align::Max,
            },
        ]);
        // Altium text remains readable at 180/270: those orientations alter the anchor.
        let angle = if orientation % 2 == 1 {
            -std::f32::consts::FRAC_PI_2
        } else {
            0.0
        };
        let offset = anchor.anchor_size(Pos2::ZERO, galley.size()).min.to_vec2();
        let rotated = egui::emath::Rot2::from_angle(angle) * offset;
        let mut shape = TextShape::new(position + rotated, galley, c).with_angle(angle);
        if font.underline {
            shape.underline = Stroke::new((size / 14.0).max(0.4), c);
        }
        let bounds = shape.visual_bounding_rect();
        self.painter.add(shape);
        bounds
    }
    fn label(
        &self,
        position: Pos2,
        text: &str,
        font: usize,
        orientation: u32,
        justification: u32,
        c: Color32,
    ) -> Rect {
        if text.ends_with('\\') {
            let plain = text.replace('\\', "");
            let bounds = self.text(position, &plain, font, orientation, justification, c);
            if bounds.is_positive() {
                self.painter
                    .line_segment([bounds.left_top(), bounds.right_top()], Stroke::new(0.7, c));
            }
            bounds
        } else {
            self.text(position, text, font, orientation, justification, c)
        }
    }
    fn poly(&self, points: Vec<Pos2>, closed: bool, fill: Option<Color32>, stroke: Stroke) -> Rect {
        if points.is_empty() {
            return Rect::NOTHING;
        }
        let bounds = Rect::from_points(&points);
        if closed {
            // PathShape's fill is convex only. Triangulate native concave polygons first.
            if let Some(fill) = fill {
                let coordinates: Vec<f64> = points
                    .iter()
                    .flat_map(|p| [p.x as f64, p.y as f64])
                    .collect();
                if let Ok(indices) = earcutr::earcut(&coordinates, &[], 2) {
                    let mut mesh = egui::Mesh::default();
                    for point in &points {
                        mesh.colored_vertex(*point, fill);
                    }
                    mesh.indices = indices.into_iter().map(|i| i as u32).collect();
                    self.painter.add(Shape::mesh(mesh));
                }
            }
        }
        for pair in points.windows(2) {
            self.line(pair[0], pair[1], stroke);
        }
        if closed && points.len() > 2 {
            self.line(*points.last().unwrap(), points[0], stroke);
        }
        bounds
    }
    fn primitive(&self, p: &SchPrimitive, pointer: Option<Pos2>) -> (Rect, bool) {
        let c = color(p.color);
        let fill = p.fill.map(color);
        let stroke = Stroke::new(self.nm(p.width_nm).clamp(0.45, 100.0), c);
        let mut hit = false;
        let bounds = match &p.kind {
            K::Polyline { points, closed, .. } => {
                let points: Vec<_> = points.iter().map(|p| self.screen(*p)).collect();
                if !closed && points.len() >= 2 {
                    self.line_end(
                        points[0],
                        points[1],
                        p.start_shape,
                        p.line_shape_size,
                        stroke,
                    );
                    self.line_end(
                        points[points.len() - 1],
                        points[points.len() - 2],
                        p.end_shape,
                        p.line_shape_size,
                        stroke,
                    );
                }
                if let Some(pos) = pointer {
                    hit = points
                        .windows(2)
                        .any(|p| distance_segment(pos, p[0], p[1]) < 4.0);
                    if *closed && fill.is_some() {
                        hit |= inside_polygon(pos, &points);
                    }
                }
                if p.line_style > 0 {
                    for points in points.windows(2) {
                        let delta = points[1] - points[0];
                        let length = delta.length();
                        let step = (self.nm(1_270_000)).max(3.0);
                        let mut offset = 0.0;
                        while offset < length {
                            self.painter.line_segment(
                                [
                                    points[0] + delta * (offset / length),
                                    points[0]
                                        + delta * ((offset + step * 0.55).min(length) / length),
                                ],
                                stroke,
                            );
                            offset += step;
                        }
                    }
                    Rect::from_points(&points)
                } else {
                    self.poly(points, *closed, fill, stroke)
                }
            }
            K::Rectangle { a, b, radius } => {
                let rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                if radius.x == 0 && radius.y == 0 {
                    self.poly(
                        vec![
                            rect.left_top(),
                            rect.right_top(),
                            rect.right_bottom(),
                            rect.left_bottom(),
                        ],
                        true,
                        fill,
                        stroke,
                    );
                } else {
                    self.painter.rect(
                        rect,
                        self.nm(radius.x.min(radius.y))
                            .min(rect.height() / 2.0)
                            .clamp(0.0, 255.0) as u8,
                        fill.unwrap_or(Color32::TRANSPARENT),
                        stroke,
                        StrokeKind::Middle,
                    );
                }
                if let Some(pos) = pointer {
                    hit = rect.expand(3.0).contains(pos);
                }
                rect
            }
            K::Arc {
                centre,
                radii,
                start,
                end,
            } => {
                let centre = self.screen(*centre);
                let rx = self.nm(radii.x);
                let ry = self.nm(radii.y);
                let sweep = (end - start).rem_euclid(360.0);
                let sweep = if sweep < 1e-9 { 360.0 } else { sweep };
                let steps =
                    ((rx.max(ry) * sweep as f32 / 360.0).sqrt() * 8.0).clamp(8.0, 256.0) as usize;
                let points: Vec<_> = (0..=steps)
                    .map(|n| {
                        let a = (start + sweep * n as f64 / steps as f64).to_radians();
                        centre + Vec2::new(a.cos() as f32 * rx, -a.sin() as f32 * ry)
                    })
                    .collect();
                if let Some(pos) = pointer {
                    hit = points
                        .windows(2)
                        .any(|p| distance_segment(pos, p[0], p[1]) < 4.0);
                }
                self.poly(points, sweep >= 359.99 && fill.is_some(), fill, stroke)
            }
            K::Bezier { points } => {
                let p = points.map(|p| self.screen(p));
                let points: Vec<_> = (0..=32)
                    .map(|i| {
                        let t = i as f32 / 32.0;
                        let u = 1.0 - t;
                        Pos2::ZERO
                            + (p[0].to_vec2() * u * u * u
                                + p[1].to_vec2() * 3.0 * u * u * t
                                + p[2].to_vec2() * 3.0 * u * t * t
                                + p[3].to_vec2() * t * t * t)
                    })
                    .collect();
                if let Some(pos) = pointer {
                    hit = points
                        .windows(2)
                        .any(|p| distance_segment(pos, p[0], p[1]) < 4.0);
                }
                self.poly(points, false, None, stroke)
            }
            K::Text {
                position,
                text,
                font,
                orientation,
                justification,
            } => {
                let displayed = if p.presentation.show_name && !p.presentation.name.is_empty() {
                    format!("{} = {text}", p.presentation.name)
                } else {
                    text.clone()
                };
                let rect = self.label(
                    self.screen(*position),
                    &displayed,
                    *font,
                    *orientation,
                    *justification,
                    c,
                );
                hit = pointer.is_some_and(|p| rect.expand(2.0).contains(p));
                rect
            }
            K::TextFrame {
                a,
                b,
                text,
                font,
                alignment,
                word_wrap,
                show_border,
            } => {
                let rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                if let Some(fill) = fill {
                    self.painter.rect_filled(rect, 0.0, fill);
                }
                if *show_border {
                    self.painter
                        .rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
                }
                let fallback = SchFont {
                    name: "Arial".into(),
                    height_nm: 2_540_000,
                    bold: false,
                    italic: false,
                    underline: false,
                };
                let font = self
                    .schematic
                    .and_then(|s| s.fonts.get(*font))
                    .unwrap_or(&fallback);
                let size = (self.nm(font.height_nm) * 0.875).clamp(1.0, 512.0);
                let mut job = egui::text::LayoutJob::simple(
                    text.clone(),
                    FontId::new(size, font_family(font)),
                    c,
                    if *word_wrap {
                        rect.width().max(1.0)
                    } else {
                        f32::INFINITY
                    },
                );
                if font.underline {
                    for section in &mut job.sections {
                        section.format.underline = Stroke::new((size / 14.0).max(0.4), c);
                    }
                }
                job.halign = match alignment {
                    0 => egui::Align::Center,
                    2 => egui::Align::Max,
                    _ => egui::Align::Min,
                };
                let galley = self.painter.layout_job(job);
                let x = match alignment {
                    0 => rect.center().x,
                    2 => rect.right(),
                    _ => rect.left(),
                };
                self.painter
                    .with_clip_rect(rect.intersect(self.rect))
                    .galley(Pos2::new(x, rect.top()), galley, c);
                hit = pointer.is_some_and(|p| rect.contains(p));
                rect
            }
            K::Image {
                a,
                b,
                filename,
                keep_aspect,
            } => {
                let mut rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                let normalized = filename.replace('\\', "/").to_lowercase();
                let encoded = self.schematic.and_then(|s| s.images.get(&normalized));
                use std::hash::{Hash, Hasher};
                let mut fingerprint = std::collections::hash_map::DefaultHasher::new();
                encoded.hash(&mut fingerprint);
                let key = egui::Id::new((
                    "sch_image",
                    self.document_id,
                    filename,
                    fingerprint.finish(),
                ));
                let cached = self
                    .painter
                    .ctx()
                    .data_mut(|data| data.get_temp::<Option<egui::TextureHandle>>(key));
                let texture = if let Some(cached) = cached {
                    cached
                } else {
                    let texture = encoded
                        .and_then(|s| STANDARD.decode(s).ok())
                        .and_then(|bytes| {
                            let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
                                .with_guessed_format()
                                .ok()?;
                            let mut limits = image::Limits::default();
                            limits.max_image_width = Some(8192);
                            limits.max_image_height = Some(8192);
                            limits.max_alloc = Some(64 * 1024 * 1024);
                            reader.limits(limits);
                            let image = reader.decode().ok()?.into_rgba8();
                            let pixels = egui::ColorImage::from_rgba_unmultiplied(
                                [image.width() as usize, image.height() as usize],
                                image.as_raw(),
                            );
                            Some(self.painter.ctx().load_texture(
                                filename,
                                pixels,
                                egui::TextureOptions::LINEAR,
                            ))
                        });
                    self.painter
                        .ctx()
                        .data_mut(|data| data.insert_temp(key, texture.clone()));
                    texture
                };
                if let Some(texture) = texture {
                    if *keep_aspect {
                        let size = texture.size_vec2();
                        let scale = (rect.width() / size.x).min(rect.height() / size.y);
                        rect = Rect::from_center_size(rect.center(), size * scale);
                    }
                    self.painter.image(
                        texture.id(),
                        rect,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                } else {
                    self.painter.rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(1.0, Color32::DARK_RED),
                        StrokeKind::Inside,
                    );
                    self.text(
                        rect.center(),
                        "Image unavailable",
                        0,
                        0,
                        4,
                        Color32::DARK_RED,
                    );
                }
                hit = pointer.is_some_and(|p| rect.contains(p));
                rect
            }
            K::Pin {
                position,
                length_nm,
                orientation,
                name,
                number,
                show_name,
                show_number,
                font,
                number_font,
                outer_symbol,
                inner_symbol,
                electrical,
                ..
            } => {
                let a = self.screen(*position);
                let d = direction(*orientation);
                let b = a + d * self.nm(*length_nm);
                let perpendicular = Vec2::new(d.y, -d.x);
                let margin = self.nm(1_778_000);
                self.line(a, b, stroke);
                if self.library_preview && matches!(electrical, 0 | 2) {
                    let size = self.nm(1_270_000);
                    let (tip, base) = if *electrical == 0 {
                        (a, a + d * size)
                    } else {
                        (a + d * size, a)
                    };
                    self.poly(
                        vec![
                            tip,
                            base + perpendicular * size * 0.4,
                            base - perpendicular * size * 0.4,
                        ],
                        true,
                        Some(Color32::WHITE),
                        stroke,
                    );
                }
                if *outer_symbol == 1 {
                    self.painter
                        .circle(a, self.nm(508_000).max(1.0), Color32::WHITE, stroke);
                }
                if *inner_symbol == 3 {
                    let s = self.nm(1_016_000);
                    self.poly(
                        vec![
                            a + perpendicular * s / 2.0,
                            a - d * s,
                            a - perpendicular * s / 2.0,
                        ],
                        false,
                        None,
                        stroke,
                    );
                }
                let mut bounds = Rect::from_two_pos(a, b);
                if *show_name && !name.is_empty() {
                    bounds = bounds.union(self.label(
                        a - d * margin,
                        name,
                        *font,
                        orientation % 2,
                        if *orientation % 4 == 0 || *orientation % 4 == 3 {
                            5
                        } else {
                            3
                        },
                        c,
                    ));
                }
                if *show_number && !number.is_empty() {
                    bounds = bounds.union(self.label(
                        a + (b - a) * 0.5 + perpendicular * self.nm(762_000),
                        number,
                        number_font.unwrap_or(*font),
                        orientation % 2,
                        1,
                        c,
                    ));
                }
                hit = pointer.is_some_and(|p| distance_segment(p, a, b) < 4.0);
                bounds
            }
            K::PowerPort {
                position,
                orientation,
                style,
                text,
                show_name,
                font,
            } => {
                let a = self.screen(*position);
                let d = direction(*orientation);
                let across = Vec2::new(-d.y, d.x);
                let length = self.nm(2_540_000);
                let b = a + d * length;
                let stem_end = if *style == 1 { b - d * length * 0.5 } else { b };
                self.line(a, stem_end, stroke);
                match style {
                    4..=6 => {
                        for (n, scale) in [1.0, 0.65, 0.25].into_iter().enumerate() {
                            let c = b + d * (length * n as f32 * 0.2);
                            self.painter.line_segment(
                                [
                                    c - across * length * 0.5 * scale,
                                    c + across * length * 0.5 * scale,
                                ],
                                stroke,
                            );
                        }
                    }
                    1 | 7 => {
                        self.poly(
                            vec![
                                b - d * length * 0.5 + across * length * 0.4,
                                b,
                                b - d * length * 0.5 - across * length * 0.4,
                            ],
                            *style == 1,
                            None,
                            stroke,
                        );
                    }
                    _ => {
                        self.painter.line_segment(
                            [b - across * length * 0.5, b + across * length * 0.5],
                            stroke,
                        );
                    }
                }
                let mut bounds = Rect::from_two_pos(a, b).expand(length * 0.5);
                if *show_name {
                    bounds = bounds.union(self.label(
                        b + d * if orientation % 2 == 0 {
                            length * 0.5
                        } else {
                            0.0
                        },
                        text,
                        *font,
                        0,
                        match orientation % 4 {
                            0 => 3,
                            1 => 1,
                            2 => 5,
                            _ => 7,
                        },
                        c,
                    ));
                }
                hit = pointer.is_some_and(|p| bounds.contains(p));
                bounds
            }
            K::Port {
                position,
                width_nm,
                height_nm,
                style,
                io_type,
                name,
                font,
            } => {
                let a = self.screen(*position);
                let vertical = *style >= 4;
                let w = self.nm(*width_nm);
                let h = self.nm(*height_nm);
                let slope = h * 0.5;
                let offsets = vec![
                    Vec2::new(0.0, -h / 2.0),
                    Vec2::new(w - slope, -h / 2.0),
                    Vec2::new(w, 0.0),
                    Vec2::new(w - slope, h / 2.0),
                    Vec2::new(0.0, h / 2.0),
                    Vec2::new(if *io_type == 3 { -slope } else { 0.0 }, 0.0),
                ];
                let points: Vec<_> = offsets
                    .into_iter()
                    .map(|p| a + if vertical { Vec2::new(p.y, -p.x) } else { p })
                    .collect();
                let bounds = self.poly(
                    points,
                    true,
                    Some(fill.unwrap_or(Color32::from_rgb(255, 255, 128))),
                    stroke,
                );
                self.label(bounds.center(), name, *font, u32::from(vertical), 4, c);
                hit = pointer.is_some_and(|p| bounds.contains(p));
                bounds
            }
            K::Junction {
                position,
                radius_nm,
            } => {
                let c0 = self.screen(*position);
                let radius = self.nm(*radius_nm).max(1.2);
                // Native junctions adopt the adjoining wire's display colour.
                // This is a paint rule, not a compiled electrical netlist.
                let wire_color = self
                    .schematic
                    .and_then(|s| {
                        s.primitives.iter().find_map(|p| match &p.kind {
                            K::Polyline {
                                points, electrical, ..
                            } if electrical == "Wire"
                                && points
                                    .windows(2)
                                    .any(|pair| position.on_segment(pair[0], pair[1])) =>
                            {
                                Some(color(p.color))
                            }
                            _ => None,
                        })
                    })
                    .unwrap_or(c);
                self.painter.circle_filled(c0, radius, wire_color);
                let bounds = Rect::from_center_size(c0, Vec2::splat(radius * 2.0));
                hit = pointer.is_some_and(|p| bounds.expand(3.0).contains(p));
                bounds
            }
            K::NoErc { position } => {
                let c0 = self.screen(*position);
                let radius = self.nm(635_000);
                self.painter
                    .line_segment([c0 - Vec2::splat(radius), c0 + Vec2::splat(radius)], stroke);
                self.painter.line_segment(
                    [
                        c0 + Vec2::new(-radius, radius),
                        c0 + Vec2::new(radius, -radius),
                    ],
                    stroke,
                );
                let bounds = Rect::from_center_size(c0, Vec2::splat(radius * 2.0));
                hit = pointer.is_some_and(|p| bounds.contains(p));
                bounds
            }
            K::Ieee {
                position,
                symbol,
                scale_nm,
                orientation,
                mirrored,
            } => {
                let a = self.screen(*position);
                let radius = self.nm(*scale_nm) / 2.0;
                let d = direction(*orientation);
                let perpendicular = Vec2::new(-d.y, d.x) * if *mirrored { -1.0 } else { 1.0 };
                if *symbol == 1 {
                    self.painter.circle(a, radius * 0.5, Color32::WHITE, stroke);
                } else if *symbol == 3 {
                    self.poly(
                        vec![
                            a - d * radius + perpendicular * radius,
                            a,
                            a - d * radius - perpendicular * radius,
                        ],
                        false,
                        None,
                        stroke,
                    );
                } else if *symbol == 11 {
                    self.poly(
                        vec![
                            a,
                            a - perpendicular * radius * 1.5,
                            a + d * radius * 1.5 - perpendicular * radius * 0.75,
                        ],
                        true,
                        None,
                        stroke,
                    );
                } else if *symbol != 0 {
                    let placeholder = Rect::from_center_size(a, Vec2::splat(radius.max(3.0)));
                    self.painter.rect_stroke(
                        placeholder,
                        0.0,
                        Stroke::new(1.0, Color32::DARK_RED),
                        StrokeKind::Inside,
                    );
                    self.text(a, &format!("IEEE {symbol}"), 0, 0, 4, Color32::DARK_RED);
                }
                let bounds = Rect::from_center_size(a, Vec2::splat(radius * 2.0));
                hit = pointer.is_some_and(|p| bounds.expand(2.0).contains(p));
                bounds
            }
            K::Unsupported { a, b, description } => {
                let rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                self.painter.rect_stroke(
                    rect,
                    0.0,
                    Stroke::new(1.0, Color32::DARK_RED),
                    StrokeKind::Inside,
                );
                self.text(rect.center(), description, 0, 0, 4, Color32::DARK_RED);
                hit = pointer.is_some_and(|p| rect.contains(p));
                rect
            }
        };
        (bounds, hit)
    }
}

fn elide_path(path: &str) -> String {
    let path = path.strip_prefix("\\\\?\\").unwrap_or(path);
    let normalized = path.replace('\\', "/");
    let parts: Vec<_> = normalized.split('/').collect();
    if parts.len() > 3 {
        let separator = if path.contains('\\') { "\\" } else { "/" };
        format!(
            "{}{}{}{}...{}{}",
            parts[0],
            separator,
            parts[1],
            separator,
            separator,
            parts.last().unwrap()
        )
    } else {
        path.to_owned()
    }
}

fn distance_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let t = if d.length_sq() > 0.0 {
        ((p - a).dot(d) / d.length_sq()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(a + d * t)
}
fn inside_polygon(p: Pos2, points: &[Pos2]) -> bool {
    let mut inside = false;
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
    }
    inside
}

pub(super) fn paint(
    ui: &mut egui::Ui,
    doc: &Document,
    view: &mut View,
    rect: Rect,
    response: &egui::Response,
) -> CanvasOutput {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_gray(195));
    let scene = Scene {
        painter: painter.clone(),
        camera: &view.camera,
        rect,
        schematic: doc.schematic.as_ref(),
        document_id: doc.id,
        library_preview: false,
    };
    let sheet = doc.schematic.as_ref().map(|s| &s.sheet);
    if let Some(sheet) = sheet {
        view.grid = sheet.grid_visible;
        view.grid_nm = sheet.grid_nm;
    }
    let board = Rect::from_two_pos(
        scene.screen(Point {
            x: 0,
            y: doc.size.y,
        }),
        scene.screen(Point {
            x: doc.size.x,
            y: 0,
        }),
    );
    let background = sheet.map_or(Color32::from_rgb(255, 252, 248), |s| color(s.background));
    painter.rect_filled(board, 0.0, background);
    if view.grid && sheet.is_none_or(|s| s.grid_visible) {
        let mut step = sheet.map_or(view.grid_nm, |s| s.grid_nm) as f64 / 1e6;
        while step * view.camera.pixels_per_mm < 5.0 {
            step *= 10.0;
        }
        let count_x = (doc.size.x as f64 / 1e6 / step).ceil() as usize;
        let count_y = (doc.size.y as f64 / 1e6 / step).ceil() as usize;
        for x in 1..count_x.min(1000) {
            let major = (x as f64 * step / 10.0).fract().abs() < 0.001;
            let stroke = Stroke::new(0.4, Color32::from_gray(if major { 220 } else { 246 }));
            let x = board.left() + (x as f64 * step * view.camera.pixels_per_mm) as f32;
            scene.line(
                Pos2::new(x, board.top()),
                Pos2::new(x, board.bottom()),
                stroke,
            );
        }
        for y in 1..count_y.min(1000) {
            let major = (y as f64 * step / 10.0).fract().abs() < 0.001;
            let stroke = Stroke::new(0.4, Color32::from_gray(if major { 220 } else { 246 }));
            let y = board.bottom() - (y as f64 * step * view.camera.pixels_per_mm) as f32;
            scene.line(
                Pos2::new(board.left(), y),
                Pos2::new(board.right(), y),
                stroke,
            );
        }
    }
    if sheet.is_none_or(|s| s.border) {
        let margin = scene.nm(sheet.map_or(5_000_000, |s| s.margin_nm));
        let border = board.shrink(margin);
        scene.poly(
            vec![
                border.left_top(),
                border.right_top(),
                border.right_bottom(),
                border.left_bottom(),
            ],
            true,
            None,
            Stroke::new(0.75, color(sheet.map_or(0, |s| s.options.border_color))),
        );
        let zones = sheet.map_or([4, 4], |s| {
            if s.options.show_zones {
                s.zones
            } else {
                [0, 0]
            }
        });
        let system_font = sheet.map_or(0, |s| s.system_font);
        let lower_right = sheet.is_some_and(|s| s.zone_origin_lower_right);
        for x in 0..zones[0] {
            let centre = board.left() + (x as f32 + 0.5) * board.width() / zones[0] as f32;
            for y in [board.top() + margin * 0.5, board.bottom() - margin * 0.5] {
                scene.text(
                    Pos2::new(centre, y),
                    &(if lower_right { zones[0] - x } else { x + 1 }).to_string(),
                    system_font,
                    0,
                    4,
                    Color32::BLACK,
                );
            }
            if x > 0 {
                let x = board.left() + x as f32 * board.width() / zones[0] as f32;
                for (a, b) in [
                    (board.top(), border.top()),
                    (border.bottom(), board.bottom()),
                ] {
                    scene.line(
                        Pos2::new(x, a),
                        Pos2::new(x, b),
                        Stroke::new(0.75, Color32::BLACK),
                    );
                }
            }
        }
        for y in 0..zones[1] {
            let centre = board.bottom() - (y as f32 + 0.5) * board.height() / zones[1] as f32;
            for x in [board.left() + margin * 0.5, board.right() - margin * 0.5] {
                scene.text(
                    Pos2::new(x, centre),
                    &char::from_u32('A' as u32 + if lower_right { y } else { zones[1] - y - 1 })
                        .unwrap_or('?')
                        .to_string(),
                    system_font,
                    0,
                    4,
                    Color32::BLACK,
                );
            }
            if y > 0 {
                let y = board.bottom() - y as f32 * board.height() / zones[1] as f32;
                for (a, b) in [
                    (board.left(), border.left()),
                    (border.right(), board.right()),
                ] {
                    scene.line(
                        Pos2::new(a, y),
                        Pos2::new(b, y),
                        Stroke::new(0.75, Color32::BLACK),
                    );
                }
            }
        }
        if sheet.is_some_and(|s| s.title_block) {
            let title = Rect::from_min_max(
                border.right_bottom() - Vec2::new(scene.nm(88_900_000), scene.nm(20_320_000)),
                border.right_bottom(),
            );
            scene.poly(
                vec![
                    title.left_top(),
                    title.right_top(),
                    title.right_bottom(),
                    title.left_bottom(),
                ],
                true,
                None,
                Stroke::new(0.75, Color32::BLACK),
            );
            let params = &sheet.unwrap().parameters;
            let value = |name: &str| {
                params
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.as_str())
                    .filter(|v| *v != "*")
                    .unwrap_or("")
            };
            // The standard title block uses native 350 x 80 mil-grid units.
            let pos = |x: f32, y: f32| {
                title.left_top()
                    + Vec2::new(
                        scene.nm((x * 254_000.0) as i64),
                        scene.nm((y * 254_000.0) as i64),
                    )
            };
            let line = |a, b| scene.line(a, b, Stroke::new(0.75, Color32::BLACK));
            line(pos(0.0, 30.0), pos(350.0, 30.0));
            line(pos(0.0, 60.0), pos(350.0, 60.0));
            for x in [50.0, 250.0] {
                line(pos(x, 30.0), pos(x, 60.0));
            }
            let text = |x, y, s: &str| {
                scene.text(pos(x, y), s, system_font, 0, 6, Color32::BLACK);
            };
            text(5.0, 0.0, "Title");
            text(55.0, 0.0, value("Title"));
            text(5.0, 30.0, "Size");
            text(55.0, 30.0, "Number");
            text(255.0, 30.0, "Revision");
            let format = sheet.unwrap().format_name();
            text(10.0, 45.0, format);
            text(55.0, 45.0, value("DocumentNumber"));
            text(255.0, 45.0, value("Revision"));
            text(5.0, 60.0, "Date:");
            text(50.0, 60.0, &view.display_date);
            text(5.0, 70.0, "File:");
            let source = if view.source_display_path.is_empty() {
                doc.name.as_str()
            } else {
                &view.source_display_path
            };
            let short_path = elide_path(source);
            text(50.0, 70.0, &short_path);
            text(
                205.0,
                60.0,
                &format!("Sheet {} of {}", value("SheetNumber"), value("SheetTotal")),
            );
            text(205.0, 70.0, &format!("Drawn By: {}", value("DrawnBy")));
        }
    }
    let mut hit = None;
    // Retain the press location through release: egui clears pointer.press_origin
    // before a fast press/move/release sequence has necessarily been painted.
    let mut release = None;
    let pointer_buttons = ui.input(|i| {
        i.events
            .iter()
            .filter_map(|event| {
                if let egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } = event
                {
                    Some((*pos, *pressed))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    });
    for (pos, pressed) in pointer_buttons {
        if pressed {
            // Floating toolbars own a different layer and must not start canvas drags.
            view.sch_primary_down =
                rect.contains(pos) && ui.ctx().layer_id_at(pos) == Some(response.layer_id);
            if view.sch_primary_down {
                view.sch_press_origin = Some(pos);
            }
        } else if view.sch_primary_down {
            release = Some(pos);
            view.sch_primary_down = false;
        }
    }
    // A busy frame can receive the entire gesture before egui has started a drag.
    let drag_stopped = response.drag_stopped_by(egui::PointerButton::Primary)
        || release
            .zip(view.sch_press_origin)
            .is_some_and(|(end, start)| end.distance(start) > 6.0);
    let hit_pointer = if response.drag_started_by(egui::PointerButton::Primary) || drag_stopped {
        view.sch_press_origin
    } else {
        response.hover_pos()
    };
    let mut selected_bounds = Rect::NOTHING;
    let mut component_bounds = HashMap::<Uuid, Rect>::new();
    let preview_ids: std::collections::HashSet<_> = view
        .sch_preview
        .as_ref()
        .into_iter()
        .flat_map(|s| s.primitives.iter().map(|p| p.id))
        .collect();
    if let Some(schematic) = &doc.schematic {
        // Clip source primitives to the actual sheet; imported templates can extend beyond it.
        let scene = Scene {
            painter: painter.with_clip_rect(board.intersect(rect)),
            ..scene
        };
        for primitive in schematic
            .primitives
            .iter()
            .chain(&schematic.parameter_objects)
            .filter(|p| !p.presentation.hidden && !preview_ids.contains(&p.id))
        {
            let (bounds, is_hit) = scene.primitive(primitive, hit_pointer);
            if let Some(id) = primitive.component {
                component_bounds
                    .entry(id)
                    .and_modify(|r| *r = r.union(bounds))
                    .or_insert(bounds);
            }
            if (view.selected.is_some()
                && (view.selected == Some(primitive.id) || view.selected == primitive.component))
                || view.sch_selection.contains(&primitive.id)
                || primitive
                    .component
                    .is_some_and(|id| view.sch_selection.contains(&id))
            {
                selected_bounds = selected_bounds.union(bounds);
            }
            let category = if primitive.text_role == Some(eda_core::schematic::TextRole::NetLabel) {
                5
            } else if primitive.text_role.is_some()
                || (matches!(primitive.kind, K::Text { .. })
                    && !primitive.presentation.name.is_empty())
            {
                6
            } else if primitive.component.is_some() {
                0
            } else if primitive.presentation.record_type == 15 {
                3
            } else if primitive.presentation.record_type == 16 {
                4
            } else {
                match &primitive.kind {
                    K::Polyline { electrical, .. } if electrical == "Wire" => 1,
                    K::Polyline { electrical, .. } if electrical == "Bus" => 2,
                    K::Port { .. } => 7,
                    K::PowerPort { .. } => 8,
                    K::Text { .. } => 9,
                    K::Junction { .. } | K::NoErc { .. } => 11,
                    _ => 10,
                }
            };
            if is_hit && view.sch_filters[category] {
                let parameter = category == 5 || category == 6;
                hit = Some(
                    if parameter
                        || (matches!(primitive.kind, K::Pin { .. })
                            && ui.input(|i| i.modifiers.ctrl))
                    {
                        primitive.id
                    } else {
                        primitive.component.unwrap_or(primitive.id)
                    },
                );
            }
        }
        if hit.is_none()
            && view.sch_filters[0]
            && let Some(pointer) = hit_pointer
        {
            hit = component_bounds
                .iter()
                .filter(|(_, r)| r.contains(pointer))
                .min_by(|(_, a), (_, b)| a.area().total_cmp(&b.area()))
                .map(|(id, _)| *id);
        }
    }
    if selected_bounds.is_finite() && selected_bounds.is_positive() {
        painter.rect_stroke(
            selected_bounds.expand(3.0),
            0.0,
            Stroke::new(1.0, Color32::from_rgb(0, 130, 190)),
            StrokeKind::Outside,
        );
    }
    if let Some(preview) = &view.sch_preview {
        let scene = Scene {
            schematic: Some(preview),
            painter: painter.with_clip_rect(rect),
            ..scene
        };
        for p in preview.primitives.iter().filter(|p| !p.presentation.hidden) {
            scene.primitive(p, None);
        }
    }
    if response.clicked() && !view.sch_tool_active {
        if ui.input(|i| i.modifiers.shift) {
            if view.sch_selection.is_empty()
                && let Some(id) = view.selected
            {
                view.sch_selection.push(id);
            }
            if let Some(id) = hit {
                if view.sch_selection.contains(&id) {
                    view.sch_selection.retain(|v| *v != id);
                } else {
                    view.sch_selection.push(id);
                }
            }
            view.selected = view.sch_selection.first().copied();
        } else {
            view.selected = hit;
            view.sch_selection = hit.into_iter().collect();
        }
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    CanvasOutput {
        cursor_mm: response.hover_pos().map(|p| view.camera.world_mm(p, rect)),
        schematic_input: Some(SchematicInput {
            drop_point: ui
                .rect_contains_pointer(rect)
                .then(|| ui.ctx().pointer_latest_pos())
                .flatten()
                .map(|p| {
                    let mm = view.camera.world_mm(p, rect);
                    Point {
                        x: (mm[0] * 1e6).round() as i64,
                        y: (mm[1] * 1e6).round() as i64,
                    }
                }),
            point: release
                .filter(|p| rect.contains(*p) && ui.rect_contains_pointer(rect))
                .or_else(|| {
                    response
                        .hover_pos()
                        .or_else(|| response.interact_pointer_pos())
                })
                .map(|p| {
                    let mm = view.camera.world_mm(p, rect);
                    Point {
                        x: (mm[0] * 1e6).round() as i64,
                        y: (mm[1] * 1e6).round() as i64,
                    }
                }),
            press_origin: view
                .sch_press_origin
                .filter(|p| rect.contains(*p))
                .map(|p| {
                    let mm = view.camera.world_mm(p, rect);
                    Point {
                        x: (mm[0] * 1e6).round() as i64,
                        y: (mm[1] * 1e6).round() as i64,
                    }
                }),
            hit,
            clicked: response.clicked(),
            secondary_clicked: response.secondary_clicked(),
            drag_started: response.drag_started_by(egui::PointerButton::Primary),
            dragging: response.dragged_by(egui::PointerButton::Primary),
            drag_stopped,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drag_release_keeps_original_hit_location_after_pointer_origin_is_cleared() {
        let ctx = egui::Context::default();
        let mut doc = Document::new(DocumentKind::Schematic, "Drag");
        let s = doc.schematic.as_mut().unwrap();
        s.sheet.border = false;
        s.sheet.title_block = false;
        let p = eda_core::schematic_edit::primitive(
            K::Polyline {
                points: vec![Point::mm(40, 50), Point::mm(60, 50)],
                closed: false,
                electrical: "Wire".into(),
            },
            None,
        );
        let id = p.id;
        s.primitives.push(p);
        let mut view = View {
            grid: false,
            ..Default::default()
        };
        view.camera.fitted = true;
        view.camera.centre_mm = [50.0, 50.0];
        view.camera.pixels_per_mm = 3.0;
        let mut frame = |events| {
            let mut result = None;
            let mut origin = Pos2::ZERO;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        origin = view
                            .camera
                            .screen(Point::mm(50, 50), ui.available_rect_before_wrap());
                        result = canvas(ui, &doc, &mut view).schematic_input;
                    });
                },
            );
            (origin, result.unwrap())
        };
        let (origin, _) = frame(vec![]);
        frame(vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let end = origin + Vec2::new(100.0, 40.0);
        let (_, started) = frame(vec![egui::Event::PointerMoved(end)]);
        assert!(started.drag_started);
        assert_eq!(started.hit, Some(id));
        let (_, stopped) = frame(vec![egui::Event::PointerButton {
            pos: end,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        assert!(stopped.drag_stopped);
        assert_eq!(stopped.hit, Some(id));
        assert_eq!(stopped.press_origin, Some(Point::mm(50, 50)));
        assert_ne!(stopped.point, stopped.press_origin);
        let (_, coalesced) = frame(vec![
            egui::Event::PointerMoved(origin),
            egui::Event::PointerButton {
                pos: origin,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(end),
            egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        assert!(coalesced.drag_stopped);
        assert_eq!(coalesced.hit, Some(id));
        assert_eq!(coalesced.press_origin, Some(Point::mm(50, 50)));
        assert_eq!(doc.schematic.as_ref().unwrap().primitives[0].id, id);
    }
    #[test]
    fn title_path_hides_middle_directories_and_windows_device_prefix() {
        assert_eq!(
            elide_path(r"\\?\C:\Users\example\Design\Circuit.SchDoc"),
            r"C:\Users\...\Circuit.SchDoc"
        );
        assert_eq!(
            elide_path("/home/example/design/Circuit.SchDoc"),
            "/home/.../Circuit.SchDoc"
        );
        assert_eq!(elide_path("Circuit.SchDoc"), "Circuit.SchDoc");
    }
    #[test]
    fn wire_hit_testing_does_not_select_its_whole_bounding_rectangle() {
        assert!(distance_segment(Pos2::new(5.0, 0.2), Pos2::ZERO, Pos2::new(10.0, 0.0)) < 1.0);
        assert!(distance_segment(Pos2::new(5.0, 5.0), Pos2::ZERO, Pos2::new(10.0, 0.0)) > 4.0);
        assert!(!inside_polygon(
            Pos2::new(9.0, 9.0),
            &[Pos2::ZERO, Pos2::new(10.0, 0.0), Pos2::new(0.0, 10.0)]
        ));
    }
}
