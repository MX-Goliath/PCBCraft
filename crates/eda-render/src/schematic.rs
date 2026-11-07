use super::*;
use eda_core::schematic::{SchFont, SchPrimitive, SchPrimitiveKind as K, Schematic};
use egui::{Align2, FontFamily, FontId, Painter, Shape, epaint::TextShape};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// Layout in a fixed font space: rasterizer rounding must not change glyph
// advances, anchors or line breaks whenever the schematic camera zooms.
const TEXT_LAYOUT_SIZE: f32 = 64.0;

pub(super) fn place_text(
    mut shape: TextShape,
    scale: f32,
    position: Pos2,
    pixels_per_point: f32,
) -> TextShape {
    shape.transform(egui::emath::TSTransform::from_scaling(scale));
    shape.pos += position.to_vec2();
    // epaint snaps TextShape.pos to physical pixels. Keep that origin snapped,
    // but retain its fractional part in the rows so only this document text
    // moves continuously (ordinary UI text keeps its normal pixel alignment).
    let snapped = Pos2::new(
        (shape.pos.x * pixels_per_point).round() / pixels_per_point,
        (shape.pos.y * pixels_per_point).round() / pixels_per_point,
    );
    let residual = egui::emath::Rot2::from_angle(-shape.angle) * (shape.pos - snapped);
    shape.pos = snapped;
    let galley = std::sync::Arc::make_mut(&mut shape.galley);
    galley.rect = galley.rect.translate(residual);
    galley.mesh_bounds = galley.mesh_bounds.translate(residual);
    for row in &mut galley.rows {
        row.pos += residual;
    }
    shape
}

fn color(value: u32) -> Color32 {
    Color32::from_rgb(value as u8, (value >> 8) as u8, (value >> 16) as u8)
}

pub(super) type JunctionCache = Option<(u64, Vec<(Point, u32)>)>;
fn auto_junctions(cache: &mut JunctionCache, primitives: Vec<&SchPrimitive>) -> Vec<(Point, u32)> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for p in &primitives {
        p.id.hash(&mut hash);
        p.color.hash(&mut hash);
        p.presentation.hidden.hash(&mut hash);
        p.component.hash(&mut hash);
        std::mem::discriminant(&p.kind).hash(&mut hash);
        match &p.kind {
            K::Polyline {
                points,
                electrical,
                closed,
            } => {
                electrical.hash(&mut hash);
                closed.hash(&mut hash);
                for p in points {
                    p.x.hash(&mut hash);
                    p.y.hash(&mut hash);
                }
            }
            K::Pin {
                position,
                length_nm,
                orientation,
                ..
            } => {
                position.x.hash(&mut hash);
                position.y.hash(&mut hash);
                length_nm.hash(&mut hash);
                orientation.hash(&mut hash);
            }
            K::Junction { position, .. }
            | K::PowerPort { position, .. }
            | K::Port { position, .. }
            | K::SheetEntry { position, .. }
            | K::Text { position, .. } => {
                position.x.hash(&mut hash);
                position.y.hash(&mut hash);
                (p.text_role == Some(eda_core::schematic::TextRole::NetLabel)).hash(&mut hash);
            }
            _ => {}
        }
    }
    let signature = hash.finish();
    if let Some((old, dots)) = cache
        && *old == signature
    {
        return dots.clone();
    }
    let dots = eda_core::schematic_edit::junctions(primitives.into_iter());
    *cache = Some((signature, dots.clone()));
    dots
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

pub(super) fn diff_overlay(
    painter: &egui::Painter,
    doc: &Document,
    camera: &Camera,
    rect: Rect,
    ids: &std::collections::HashSet<Uuid>,
    tint: Color32,
) {
    let Some(s) = &doc.schematic else {
        return;
    };
    let scene = Scene {
        painter: painter.clone(),
        camera,
        rect,
        schematic: Some(s),
        document_id: doc.id,
        library_preview: doc.library.is_some(),
    };
    let rgb = tint.r() as u32 | ((tint.g() as u32) << 8) | ((tint.b() as u32) << 16);
    let primitives: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| {
            !p.presentation.hidden
                && (ids.contains(&p.id) || p.component.is_some_and(|id| ids.contains(&id)))
        })
        .collect();
    for (position, _) in auto_junctions(&mut None, primitives) {
        scene
            .painter
            .circle_filled(scene.screen(position), scene.nm(508_000).max(1.4), tint);
    }
    for p in s.primitives.iter().chain(&s.parameter_objects).filter(|p| {
        !p.presentation.hidden
            && (ids.contains(&p.id) || p.component.is_some_and(|id| ids.contains(&id)))
    }) {
        let mut p = p.clone();
        p.color = rgb;
        p.fill = p.fill.map(|_| rgb);
        p.presentation.text_color = Some(rgb);
        match &mut p.kind {
            K::HarnessEntry { text_color, .. } | K::SheetEntry { text_color, .. } => {
                *text_color = rgb
            }
            _ => {}
        }
        if let K::Junction {
            position,
            radius_nm,
        } = &p.kind
        {
            scene.painter.circle_filled(
                scene.screen(*position),
                scene.nm(*radius_nm).max(1.2),
                tint,
            );
        } else {
            scene.primitive(&p, None);
        }
    }
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
        if stroke.is_empty() {
            return;
        }
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
        let size = self.nm(font.height_nm) * 0.875;
        if size < 2.2 {
            return Rect::NOTHING;
        }
        let galley = self.painter.layout_no_wrap(
            text.to_owned(),
            FontId::new(TEXT_LAYOUT_SIZE, font_family(font)),
            c,
        );
        let scale = size / TEXT_LAYOUT_SIZE;
        let justification = justification.min(8);
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
        // Document angles are counterclockwise; screen Y points down. Preserve
        // both the text orientation and its local anchor (Altium's Display
        // Strings As Rotated mode), including upside-down/left-reading text.
        let angle = -((orientation % 4) as f32) * std::f32::consts::FRAC_PI_2;
        let offset = anchor.anchor_size(Pos2::ZERO, galley.size()).min.to_vec2();
        let rotated = egui::emath::Rot2::from_angle(angle) * offset;
        let mut shape = TextShape::new(rotated.to_pos2(), galley, c).with_angle(angle);
        if font.underline {
            shape.underline = Stroke::new(TEXT_LAYOUT_SIZE / 14.0, c);
        }
        let shape = place_text(
            shape,
            scale,
            position,
            self.painter.ctx().pixels_per_point(),
        );
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
    fn patterned_line(&self, a: Pos2, b: Pos2, style: u32, stroke: Stroke) {
        if style == 0 {
            self.line(a, b, stroke);
            return;
        }
        let delta = b - a;
        let length = delta.length();
        if length <= 0.0 {
            return;
        }
        let step = self
            .nm(if style == 2 { 508_000 } else { 1_270_000 })
            .max(if style == 2 {
                stroke.width.max(1.0) * 2.5
            } else {
                3.0
            });
        let mut offset = 0.0;
        while offset < length {
            if style == 2 {
                self.painter.circle_filled(
                    a + delta * (offset / length),
                    stroke.width.max(1.0) * 0.5,
                    stroke.color,
                );
            } else {
                self.line(
                    a + delta * (offset / length),
                    a + delta * ((offset + step * 0.55).min(length) / length),
                    stroke,
                );
            }
            offset += step;
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
        if p.presentation.hidden {
            return (Rect::NOTHING, false);
        }
        let bundle_terminal = !p.presentation.harness_type.is_empty()
            && matches!(p.kind, K::Port { .. } | K::SheetEntry { .. });
        let c = color(if bundle_terminal {
            eda_core::schematic_harness::BORDER
        } else {
            p.color
        });
        let fill = if bundle_terminal {
            Some(color(eda_core::schematic_harness::FILL))
        } else {
            p.fill.map(color)
        };
        let stroke = if p.width_nm == 0 && p.fill.is_some() {
            Stroke::NONE
        } else {
            Stroke::new(self.nm(p.width_nm).clamp(0.45, 100.0), c)
        };
        if p.presentation.record_type == 210
            && let K::ParameterSet {
                position,
                orientation,
                ..
            } = p.kind
        {
            let point = |x, y| {
                self.screen(
                    eda_core::schematic_edit::transform_point(
                        Point { x, y },
                        Point::default(),
                        orientation,
                        position,
                    )
                    .unwrap_or(position),
                )
            };
            let a = point(0, 0);
            let b = point(2_540_000, 2_540_000);
            self.line(a, b, stroke);
            self.poly(
                vec![
                    point(635_000, 635_000),
                    point(1_905_000, 2_540_000),
                    point(2_540_000, 1_905_000),
                ],
                true,
                Some(c),
                stroke,
            );
            let bounds = Rect::from_two_pos(a, b).expand(3.);
            return (bounds, pointer.is_some_and(|at| bounds.contains(at)));
        }
        let mut hit = false;
        let bounds = match &p.kind {
            K::Blanket { points, collapsed } => {
                let pts: Vec<_> = points.iter().map(|p| self.screen(*p)).collect();
                let rect = Rect::from_points(&pts);
                let toggle =
                    Rect::from_min_size(rect.left_top(), Vec2::splat(self.nm(1_524_000).max(3.0)));
                if !collapsed {
                    self.poly(
                        pts.clone(),
                        true,
                        Some(fill.unwrap_or(Color32::WHITE).gamma_multiply(0.5)),
                        if p.line_style == 0 {
                            stroke
                        } else {
                            Stroke::NONE
                        },
                    );
                    if p.line_style > 0 {
                        for (a, b) in pts
                            .iter()
                            .copied()
                            .zip(pts.iter().copied().cycle().skip(1))
                            .take(pts.len())
                        {
                            self.patterned_line(a, b, p.line_style, stroke);
                        }
                    }
                    hit = pointer.is_some_and(|p| {
                        pts.iter()
                            .copied()
                            .zip(pts.iter().copied().cycle().skip(1))
                            .take(pts.len())
                            .any(|(a, b)| distance_segment(p, a, b) < 4.0)
                    });
                }
                self.painter.add(Shape::convex_polygon(
                    vec![
                        toggle.left_top(),
                        toggle.right_top(),
                        if *collapsed {
                            toggle.left_bottom()
                        } else {
                            toggle.center_bottom()
                        },
                    ],
                    c,
                    Stroke::NONE,
                ));
                hit |= pointer.is_some_and(|p| toggle.expand(2.0).contains(p));
                if *collapsed { toggle } else { rect }
            }
            K::CompileMask { a, b, collapsed } => {
                let rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                let toggle =
                    Rect::from_min_size(rect.left_top(), Vec2::splat(self.nm(1_524_000).max(3.0)));
                if !collapsed {
                    self.painter.rect_filled(
                        rect,
                        0.0,
                        fill.unwrap_or(Color32::WHITE).gamma_multiply(0.25),
                    );
                    self.painter
                        .rect_stroke(rect, 0.0, stroke, StrokeKind::Middle);
                    hit = pointer.is_some_and(|p| {
                        rect.expand(4.0).contains(p) && !rect.shrink(4.0).contains(p)
                    });
                }
                self.painter
                    .rect_stroke(toggle, 0.0, stroke, StrokeKind::Middle);
                self.painter.add(Shape::convex_polygon(
                    vec![
                        toggle.left_top(),
                        toggle.right_top(),
                        if *collapsed {
                            toggle.left_bottom()
                        } else {
                            toggle.center_bottom()
                        },
                    ],
                    c,
                    Stroke::NONE,
                ));
                hit |= pointer.is_some_and(|p| toggle.expand(2.0).contains(p));
                if *collapsed { toggle } else { rect }
            }
            K::ParameterSet {
                position,
                orientation,
                name: label,
                parameters,
                style,
                ..
            } => {
                let a = self.screen(*position);
                let d = direction(*orientation);
                let across = Vec2::new(-d.y, d.x);
                let size = self.nm(2_540_000);
                let b = a + d * size;
                let pair=parameters.iter().any(|(k,v)| k.eq_ignore_ascii_case("DifferentialPair") && v.eq_ignore_ascii_case("True")) || self.schematic.is_some_and(|s|s.primitives.iter().chain(&s.parameter_objects).any(|child|child.presentation.parent==Some(p.id)&&child.presentation.name.eq_ignore_ascii_case("DifferentialPair")&&matches!(&child.kind,K::Text{text,..} if text.eq_ignore_ascii_case("True"))));
                self.line(a, b - d * size * 0.5, stroke);
                if pair {
                    self.poly(
                        vec![
                            b - d * size * 0.5,
                            b - d * size * 0.2 + across * size * 0.35,
                            b + d * size * 0.4 + across * size * 0.35,
                            b + d * size * 0.7,
                            b + d * size * 0.4 - across * size * 0.35,
                            b - d * size * 0.2 - across * size * 0.35,
                        ],
                        true,
                        None,
                        stroke,
                    );
                } else {
                    self.painter.circle_stroke(
                        b,
                        size * if *style == 0 { 0.5 } else { 0.25 },
                        stroke,
                    );
                    if *style == 0 {
                        self.line(b - across * size * 0.15, b + across * size * 0.15, stroke);
                        self.painter
                            .circle_filled(b - across * size * 0.3, stroke.width, c);
                    }
                }
                let mut bounds =
                    Rect::from_two_pos(a - across * size * 0.5, b + d * size + across * size * 0.5);
                // Small/Tiny directives are just markers. Their stored label is
                // still editable metadata, not another visible parameter.
                if *style == 0 && !label.is_empty() && !pair {
                    bounds = bounds.union(self.label(
                        b + d * size * 0.75,
                        label,
                        self.schematic.map_or(0, |s| s.sheet.system_font),
                        *orientation,
                        3,
                        c,
                    ));
                }
                hit = pointer.is_some_and(|at| bounds.expand(3.0).contains(at));
                bounds
            }
            K::HarnessConnector {
                a,
                b,
                side,
                primary_nm,
                ..
            } => {
                let rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                self.poly(
                    vec![
                        rect.left_top(),
                        rect.right_top(),
                        rect.right_bottom(),
                        rect.left_bottom(),
                    ],
                    true,
                    fill,
                    Stroke::NONE,
                );
                let terminal = eda_core::schematic_harness::edge(*a, *b, *side, *primary_nm);
                let inset = 2_540_000.min(if *side <= 1 {
                    (b.x - a.x) / 3
                } else {
                    (a.y - b.y) / 3
                });
                let half = inset / 2;
                let local = if *side <= 1 {
                    let sign = if *side == 1 { -1 } else { 1 };
                    vec![
                        Point {
                            x: terminal.x + sign * inset,
                            y: a.y,
                        },
                        Point {
                            x: terminal.x + sign * half,
                            y: a.y - half,
                        },
                        Point {
                            x: terminal.x + sign * half,
                            y: terminal.y + half,
                        },
                        terminal,
                        Point {
                            x: terminal.x + sign * half,
                            y: terminal.y - half,
                        },
                        Point {
                            x: terminal.x + sign * half,
                            y: b.y + half,
                        },
                        Point {
                            x: terminal.x + sign * inset,
                            y: b.y,
                        },
                    ]
                } else {
                    let sign = if *side == 2 { -1 } else { 1 };
                    vec![
                        Point {
                            x: a.x,
                            y: terminal.y + sign * inset,
                        },
                        Point {
                            x: a.x + half,
                            y: terminal.y + sign * half,
                        },
                        Point {
                            x: terminal.x - half,
                            y: terminal.y + sign * half,
                        },
                        terminal,
                        Point {
                            x: terminal.x + half,
                            y: terminal.y + sign * half,
                        },
                        Point {
                            x: b.x - half,
                            y: terminal.y + sign * half,
                        },
                        Point {
                            x: b.x,
                            y: terminal.y + sign * inset,
                        },
                    ]
                };
                self.poly(
                    local.into_iter().map(|p| self.screen(p)).collect(),
                    false,
                    None,
                    stroke,
                );
                hit = pointer.is_some_and(|p| rect.expand(3.0).contains(p));
                rect
            }
            K::HarnessEntry {
                position,
                side,
                name,
                font,
                text_color,
            } => {
                let prefix = self.schematic.and_then(|s| p.presentation.parent.and_then(|id| s.primitive(id))).is_some_and(|parent| matches!(&parent.kind, K::HarnessConnector { bus_text_style, .. } if bus_text_style == "Prefix"));
                let name = if prefix {
                    name.split('[').next().unwrap_or(name)
                } else {
                    name
                };
                let at = self.screen(*position);
                let inward = match side {
                    0 => Vec2::RIGHT,
                    1 => Vec2::LEFT,
                    2 => Vec2::DOWN,
                    _ => Vec2::UP,
                };
                self.painter.circle_filled(at, self.nm(127_000).max(0.8), c);
                let bounds = self.label(
                    at + inward * self.nm(508_000),
                    name,
                    *font,
                    if *side >= 2 { 1 } else { 0 },
                    if matches!(side, 1 | 2) { 5 } else { 3 },
                    color(*text_color),
                );
                hit = pointer.is_some_and(|p| {
                    bounds
                        .union(Rect::from_center_size(at, Vec2::splat(6.0)))
                        .contains(p)
                });
                bounds.union(Rect::from_center_size(at, Vec2::splat(3.0)))
            }
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
            K::SheetSymbol { a, b, .. } => {
                let rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
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
                if p.presentation.symbol_type == "Device Sheet" {
                    let centre = rect.center();
                    let size = (rect.height() * 0.34).min(rect.width() * 0.2);
                    for n in 0..3 {
                        let angle = n as f32 * std::f32::consts::TAU / 3.;
                        let rotate = |x: f32, y: f32| {
                            centre
                                + Vec2::new(
                                    x * angle.cos() - y * angle.sin(),
                                    x * angle.sin() + y * angle.cos(),
                                ) * size
                        };
                        self.poly(
                            vec![
                                rotate(-0.9, -0.35),
                                rotate(-0.3, -0.95),
                                rotate(0.3, -0.95),
                                rotate(0.15, -1.2),
                                rotate(0.95, -0.8),
                                rotate(0.45, -0.05),
                                rotate(0.35, -0.35),
                                rotate(-0.1, -0.35),
                                rotate(-0.5, 0.1),
                            ],
                            true,
                            Some(Color32::from_rgb(126, 179, 113)),
                            Stroke::NONE,
                        );
                    }
                }
                hit = pointer.is_some_and(|p| rect.expand(3.0).contains(p));
                rect
            }
            K::SheetEntry {
                position,
                side,
                io_type,
                name,
                font,
                text_color,
                arrow_kind,
            } => {
                let at = self.screen(*position);
                let inward = match side {
                    0 => Vec2::RIGHT,
                    1 => Vec2::LEFT,
                    2 => Vec2::DOWN,
                    _ => Vec2::UP,
                };
                let normal = Vec2::new(-inward.y, inward.x);
                let length = self.nm(2_540_000);
                let half = self.nm(635_000);
                let arrows = !arrow_kind.eq_ignore_ascii_case("Block");
                let input = arrows && matches!(io_type, 2 | 3);
                let output = arrows && matches!(io_type, 1 | 3);
                let local = [
                    (if output { half } else { 0.0 }, -half),
                    (length - if input { half } else { 0.0 }, -half),
                    (length, 0.0),
                    (length - if input { half } else { 0.0 }, half),
                    (if output { half } else { 0.0 }, half),
                    (0.0, 0.0),
                ];
                let shape = self.poly(
                    local
                        .into_iter()
                        .map(|(x, y)| at + inward * x + normal * y)
                        .collect(),
                    true,
                    Some(fill.unwrap_or(Color32::from_rgb(255, 255, 128))),
                    stroke,
                );
                let label = at + inward * (length + self.nm(508_000));
                let text = self.label(
                    label,
                    name,
                    *font,
                    if *side >= 2 { 1 } else { 0 },
                    if matches!(side, 1 | 2) { 5 } else { 3 },
                    color(*text_color),
                );
                let bounds = shape.union(text);
                hit = pointer.is_some_and(|p| bounds.expand(2.0).contains(p));
                bounds
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
                    hit = rect.expand(3.0).contains(pos)
                        && (fill.is_some() || !rect.shrink(3.0).contains(pos));
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
                let points: Vec<_> = points
                    .windows(4)
                    .step_by(3)
                    .flat_map(|control| {
                        let p: Vec<_> = control.iter().map(|p| self.screen(*p)).collect();
                        (0..=32).map(move |i| {
                            let t = i as f32 / 32.0;
                            let u = 1.0 - t;
                            Pos2::ZERO
                                + (p[0].to_vec2() * u * u * u
                                    + p[1].to_vec2() * 3.0 * u * u * t
                                    + p[2].to_vec2() * 3.0 * u * t * t
                                    + p[3].to_vec2() * t * t * t)
                        })
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
                if p.presentation.record_type == 217 && text == "*" {
                    return (Rect::NOTHING, false);
                }
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
                let size = self.nm(font.height_nm) * 0.875;
                if size < 2.2 {
                    return (rect, pointer.is_some_and(|p| rect.contains(p)));
                }
                let scale = size / TEXT_LAYOUT_SIZE;
                let mut job = egui::text::LayoutJob::simple(
                    text.clone(),
                    FontId::new(TEXT_LAYOUT_SIZE, font_family(font)),
                    c,
                    if *word_wrap {
                        // Derive wrap width from document coordinates as well;
                        // subtracting screen positions loses precision as we pan/zoom.
                        (((b.x as f64 - a.x as f64).abs() / font.height_nm as f64)
                            * (TEXT_LAYOUT_SIZE as f64 / 0.875)) as f32
                    } else {
                        f32::INFINITY
                    },
                );
                if font.underline {
                    for section in &mut job.sections {
                        section.format.underline = Stroke::new(TEXT_LAYOUT_SIZE / 14.0, c);
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
                let shape = place_text(
                    TextShape::new(Pos2::ZERO, galley, c),
                    scale,
                    Pos2::new(x, rect.top()),
                    self.painter.ctx().pixels_per_point(),
                );
                self.painter
                    .with_clip_rect(rect.intersect(self.rect))
                    .add(shape);
                hit = pointer.is_some_and(|p| rect.contains(p));
                rect
            }
            K::Image {
                a,
                b,
                filename,
                keep_aspect,
                ..
            } => {
                let mut rect = Rect::from_two_pos(self.screen(*a), self.screen(*b));
                let normalized = filename.replace('\\', "/").to_lowercase();
                let encoded = self.schematic.and_then(|s| s.images.get(&normalized));
                let texture = crate::image_cache::texture(
                    self.painter.ctx(),
                    self.document_id,
                    filename,
                    encoded.map(String::as_str),
                );
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
                cross_sheet,
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
                if *cross_sheet {
                    for n in 0..2 {
                        let x = n as f32 * length * 0.4;
                        let (tip, back) = if *style == 0 {
                            (x, x + length * 0.35)
                        } else {
                            (x + length * 0.35, x)
                        };
                        self.poly(
                            vec![
                                a + d * back - across * length * 0.4,
                                a + d * tip,
                                a + d * back + across * length * 0.4,
                            ],
                            false,
                            None,
                            stroke,
                        );
                    }
                    let mut bounds = Rect::from_two_pos(
                        a - across * length * 0.4,
                        a + d * length + across * length * 0.4,
                    );
                    if *show_name {
                        bounds = bounds.union(self.label(
                            a + d * length,
                            text,
                            *font,
                            orientation % 2,
                            match orientation % 4 {
                                0 => 3,
                                1 => 1,
                                2 => 5,
                                _ => 7,
                            },
                            c,
                        ));
                    }
                    hit = pointer.is_some_and(|p| bounds.expand(3.0).contains(p));
                    bounds
                } else {
                    let stem_end = if *style == 1 {
                        b - d * length * 0.5
                    } else if *style == 0 {
                        b - d * length * 0.25
                    } else {
                        b
                    };
                    self.line(a, stem_end, stroke);
                    match style {
                        0 => {
                            self.painter.circle_stroke(b, length * 0.25, stroke);
                        }
                        3 => {
                            let points = (0..=24)
                                .map(|n| {
                                    let t = n as f32 / 24.;
                                    b + across * ((t - 0.5) * length)
                                        + d * ((t * std::f32::consts::TAU).sin() * length * 0.16)
                                })
                                .collect();
                            self.poly(points, false, None, stroke);
                        }
                        5 => {
                            self.poly(
                                vec![
                                    b - across * length * 0.5,
                                    b + across * length * 0.5,
                                    b + d * length * 0.7,
                                ],
                                true,
                                None,
                                stroke,
                            );
                        }
                        6 => {
                            self.line(b - across * length * 0.5, b + across * length * 0.5, stroke);
                            for n in [-1., 0., 1.] {
                                let base = b + across * length * n * 0.35;
                                self.line(
                                    base,
                                    base + d * length * 0.35 - across * length * 0.25,
                                    stroke,
                                );
                            }
                        }
                        4 | 8 | 9 => {
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
                let left_arrow = matches!(io_type, 1 | 3);
                let right_arrow = matches!(io_type, 2 | 3);
                let offsets = vec![
                    Vec2::new(if left_arrow { slope } else { 0.0 }, -h / 2.0),
                    Vec2::new(w - if right_arrow { slope } else { 0.0 }, -h / 2.0),
                    Vec2::new(w, 0.0),
                    Vec2::new(w - if right_arrow { slope } else { 0.0 }, h / 2.0),
                    Vec2::new(if left_arrow { slope } else { 0.0 }, h / 2.0),
                    Vec2::ZERO,
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
                let along = if vertical { Vec2::UP } else { Vec2::RIGHT };
                let (offset, justification) = match p.presentation.alignment {
                    Some(1) => (self.nm(508_000) + if left_arrow { slope } else { 0.0 }, 3),
                    Some(2) => (
                        w - self.nm(508_000) - if right_arrow { slope } else { 0.0 },
                        5,
                    ),
                    _ => (w / 2.0, 4),
                };
                self.label(
                    a + along * offset,
                    name,
                    *font,
                    u32::from(vertical),
                    justification,
                    color(p.presentation.text_color.unwrap_or(p.color)),
                );
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
    panning: bool,
) -> CanvasOutput {
    paint_mode(ui, doc, view, rect, response, false, panning)
}

pub(super) fn paint_output(
    ui: &mut egui::Ui,
    doc: &Document,
    view: &mut View,
    rect: Rect,
    response: &egui::Response,
) -> CanvasOutput {
    paint_mode(ui, doc, view, rect, response, true, false)
}

fn paint_mode(
    ui: &mut egui::Ui,
    doc: &Document,
    view: &mut View,
    rect: Rect,
    response: &egui::Response,
    output: bool,
    panning: bool,
) -> CanvasOutput {
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        0.0,
        if output {
            Color32::WHITE
        } else {
            Color32::from_gray(195)
        },
    );
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
        view.grid = view.sch_grid_override.unwrap_or(sheet.grid_visible);
        view.grid_nm = sheet.grid_nm;
    }
    let mut board = Rect::from_two_pos(
        scene.screen(Point {
            x: 0,
            y: doc.size.y,
        }),
        scene.screen(Point {
            x: doc.size.x,
            y: 0,
        }),
    );
    let library = doc.library.is_some();
    if library {
        board = rect;
    }
    let background = sheet.map_or(Color32::from_rgb(255, 252, 248), |s| color(s.background));
    painter.rect_filled(board, 0.0, background);
    if library {
        if view.grid {
            let mut step = view.grid_nm as f64 / 1e6;
            while step * view.camera.pixels_per_mm < 8. {
                step *= 10.;
            }
            let min = view.camera.world_mm(rect.left_bottom(), rect);
            let max = view.camera.world_mm(rect.right_top(), rect);
            for x in (min[0] / step).floor() as i64..=(max[0] / step).ceil() as i64 {
                let x = view
                    .camera
                    .screen(
                        Point {
                            x: (x as f64 * step * 1e6) as i64,
                            y: 0,
                        },
                        rect,
                    )
                    .x;
                painter.line_segment(
                    [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                    Stroke::new(0.4, Color32::from_gray(238)),
                );
            }
            for y in (min[1] / step).floor() as i64..=(max[1] / step).ceil() as i64 {
                let y = view
                    .camera
                    .screen(
                        Point {
                            x: 0,
                            y: (y as f64 * step * 1e6) as i64,
                        },
                        rect,
                    )
                    .y;
                painter.line_segment(
                    [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                    Stroke::new(0.4, Color32::from_gray(238)),
                );
            }
        }
        let origin = scene.screen(Point::default());
        painter.line_segment(
            [
                Pos2::new(origin.x, rect.top()),
                Pos2::new(origin.x, rect.bottom()),
            ],
            Stroke::new(0.7, Color32::BLACK),
        );
        painter.line_segment(
            [
                Pos2::new(rect.left(), origin.y),
                Pos2::new(rect.right(), origin.y),
            ],
            Stroke::new(0.7, Color32::BLACK),
        );
    } else if view.grid
        && view
            .sch_grid_override
            .unwrap_or_else(|| sheet.is_none_or(|s| s.grid_visible))
    {
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
    let focused = ui.is_enabled() && ui.input(|i| i.focused);
    let suppressed = view.sch_suppress_primary;
    for (pos, pressed, modifiers) in ui.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                } => Some((*pos, *pressed, *modifiers)),
                _ => None,
            })
            .collect::<Vec<_>>()
    }) {
        if pressed {
            view.sch_primary_down = !suppressed
                && focused
                && rect.contains(pos)
                && ui.ctx().layer_id_at(pos) == Some(response.layer_id);
            if view.sch_primary_down {
                view.sch_press_origin = Some(pos);
                view.sch_press_modifiers = modifiers;
                view.sch_primary_dragging = false;
            }
        } else if view.sch_primary_down {
            release = Some(pos);
            view.sch_primary_down = false;
        }
    }
    let was_dragging = view.sch_primary_dragging;
    let pointer = release.or_else(|| ui.ctx().pointer_latest_pos());
    if view
        .sch_press_origin
        .zip(pointer)
        .is_some_and(|(a, b)| a.distance(b) > 6.)
    {
        view.sch_primary_dragging = true;
    }
    let interrupted = !focused || panning;
    let dragging = view.sch_primary_dragging && !interrupted;
    let drag_started = dragging && !was_dragging;
    let drag_stopped = dragging && release.is_some();
    let origin = view.sch_press_origin;
    let modifiers = if origin.is_some() {
        view.sch_press_modifiers
    } else {
        ui.input(|i| i.modifiers)
    };
    let hit_pointer = if dragging {
        origin
    } else {
        response.hover_pos()
    };
    if interrupted {
        view.sch_primary_down = false;
        view.sch_primary_dragging = false;
        view.sch_press_origin = None;
        view.sch_selection_box = None;
        view.sch_suppress_primary = true;
    }
    if !ui.input(|i| i.pointer.primary_down()) {
        view.sch_suppress_primary = false;
        view.sch_press_origin = None;
        view.sch_primary_dragging = false;
    }
    let mut selected_bounds = Rect::NOTHING;
    if view.review_overlay {
        return CanvasOutput::default();
    }
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
        let excluded = eda_core::schematic_compile::exclusions(schematic);
        let mut ordered: Vec<_> = schematic
            .primitives
            .iter()
            .chain(&schematic.parameter_objects)
            .collect();
        ordered.sort_by_key(|p| !matches!(p.kind, K::Blanket { .. } | K::CompileMask { .. }));
        for primitive in ordered
            .into_iter()
            .filter(|p| !p.presentation.hidden && !preview_ids.contains(&p.id))
        {
            let masked = excluded.primitives.contains(&primitive.id)
                || primitive
                    .component
                    .is_some_and(|c| excluded.components.contains(&c));
            let highlighted = view.sch_highlight.contains(&primitive.id);
            let net_dimmed = !view.sch_highlight.is_empty() && !highlighted;
            let faded = (masked || net_dimmed || highlighted).then(|| {
                let mut p = primitive.clone();
                p.color = if highlighted { 0x00b000 } else { 0xa0a0a0 };
                p.fill = p
                    .fill
                    .map(|_| if highlighted { 0xb0ffb0 } else { 0xe0e0e0 });
                p.presentation.text_color = Some(p.color);
                p
            });
            let (bounds, is_hit) =
                scene.primitive(faded.as_ref().unwrap_or(primitive), hit_pointer);
            if let Some(id) = primitive.component {
                component_bounds
                    .entry(id)
                    .and_modify(|r| *r = r.union(bounds))
                    .or_insert(bounds);
            }
            if !matches!(primitive.kind, K::Blanket { .. } | K::CompileMask { .. })
                && !matches!(&primitive.kind,K::Polyline{electrical,..} if electrical=="Wire")
                && !view.sch_segments.iter().any(|span| span.id == primitive.id)
                && ((view.selected.is_some()
                    && (view.selected == Some(primitive.id)
                        || view.selected == primitive.component))
                    || primitive.presentation.parent.is_some_and(|id| {
                        view.selected == Some(id) || view.sch_selection.contains(&id)
                    })
                    || view.sch_selection.contains(&primitive.id)
                    || primitive
                        .component
                        .is_some_and(|id| view.sch_selection.contains(&id)))
            {
                selected_bounds = selected_bounds.union(bounds);
            }
            let category = selection_category(primitive);
            if is_hit && view.sch_filters[category] {
                let parameter = category == 5 || category == 6;
                hit = Some(
                    if library
                        || parameter
                        || (matches!(primitive.kind, K::Pin { .. })
                            && (view.cross_probe || modifiers.ctrl || modifiers.alt))
                    {
                        primitive.id
                    } else {
                        primitive.component.unwrap_or(primitive.id)
                    },
                );
            }
        }
        if !library
            && hit.is_none()
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
    if !view.sch_tool_active
        && let Some(s) = &doc.schematic
    {
        for primitive in &s.primitives {
            if primitive.component.is_some()
                || view.sch_segments.iter().any(|span| span.id == primitive.id)
                || !(view.sch_selection.contains(&primitive.id)
                    || view.selected == Some(primitive.id))
            {
                continue;
            }
            for at in eda_core::schematic_directives::handles(&primitive.kind) {
                painter.rect_filled(
                    Rect::from_center_size(scene.screen(at), Vec2::splat(6.0)),
                    0.0,
                    Color32::GREEN,
                );
            }
            if let K::Polyline {
                points,
                electrical,
                closed: false,
            } = &primitive.kind
                && electrical == "Wire"
            {
                for p in points
                    .iter()
                    .copied()
                    .chain(points.windows(2).map(|e| Point {
                        x: e[0].x + (e[1].x - e[0].x) / 2,
                        y: e[0].y + (e[1].y - e[0].y) / 2,
                    }))
                {
                    painter.rect_filled(
                        Rect::from_center_size(scene.screen(p), Vec2::splat(5.0)),
                        0.0,
                        Color32::from_rgb(0, 160, 0),
                    );
                }
            }
        }
    }
    if !view.sch_tool_active {
        for span in &view.sch_segments {
            let a = scene.screen(span.a);
            let b = scene.screen(span.b);
            for at in [a, b] {
                painter.rect_filled(
                    Rect::from_center_size(at, Vec2::splat(5.0)),
                    0.0,
                    Color32::RED,
                );
            }
        }
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
    if let Some(s) = &doc.schematic {
        let primitives = s
            .primitives
            .iter()
            .filter(|p| !preview_ids.contains(&p.id))
            .chain(view.sch_preview.iter().flat_map(|s| &s.primitives))
            .collect();
        for (at, ink) in auto_junctions(&mut view.sch_junction_cache, primitives) {
            painter.with_clip_rect(board.intersect(rect)).circle_filled(
                scene.screen(at),
                scene.nm(508_000).max(1.4),
                color(ink),
            );
        }
    }
    if output {
        return CanvasOutput {
            cursor_mm: None,
            schematic_input: None,
        };
    }
    if dragging
        && !view.sch_tool_active
        && (hit.is_none() || modifiers.shift)
        && !modifiers.alt
        && let (Some(a), Some(z)) = (origin, pointer)
    {
        let world = |p| {
            let mm = view.camera.world_mm(p, rect);
            Point {
                x: (mm[0] * 1e6).round() as i64,
                y: (mm[1] * 1e6).round() as i64,
            }
        };
        let (a, z) = (world(a), world(z));
        view.sch_selection_box = Some((a, z));
        if drag_stopped {
            if let Some(s) = &doc.schematic {
                let mode = if (z.x < a.x) != view.camera.mirrored_x {
                    eda_core::schematic_arrange::Region::Touching
                } else {
                    eda_core::schematic_arrange::Region::Inside
                };
                let ids = eda_core::schematic_arrange::select_region(s, &[a, z], mode)
                    .into_iter()
                    .filter(|id| {
                        s.components.iter().any(|c| c.id == *id) && view.sch_filters[0]
                            || s.primitive(*id)
                                .is_some_and(|p| view.sch_filters[selection_category(p)])
                    })
                    .collect::<Vec<_>>();
                if !modifiers.shift {
                    view.sch_selection.clear();
                    view.sch_segments.clear();
                    view.selected = None;
                }
                for id in ids {
                    if !view.sch_selection.contains(&id) {
                        view.sch_selection.push(id);
                    }
                }
                view.selected = view.sch_selection.first().copied();
            }
            view.sch_selection_box = None;
        }
    }
    if let Some((a, z)) = view.sch_selection_box {
        let a = scene.screen(a);
        let z = scene.screen(z);
        let ink = if z.x >= a.x {
            Color32::LIGHT_BLUE
        } else {
            Color32::LIGHT_GREEN
        };
        let r = Rect::from_two_pos(a, z);
        painter.rect_filled(r, 0., ink.linear_multiply(0.12));
        painter.rect_stroke(r, 0., Stroke::new(1., ink), StrokeKind::Inside);
    }
    if response.clicked()
        && !view.sch_tool_active
        && !dragging
        && !interrupted
        && !suppressed
        && !modifiers.alt
    {
        let span = hit.and_then(|id| {
            let mm = view.camera.world_mm(hit_pointer?, rect);
            eda_core::schematic_edit::wire_segment_at(
                doc.schematic.as_ref()?,
                id,
                Point {
                    x: (mm[0] * 1e6).round() as i64,
                    y: (mm[1] * 1e6).round() as i64,
                },
                (6.0 / view.camera.pixels_per_mm * 1e6) as i64,
            )
        });
        select_hit(view, hit, span, modifiers.shift);
    }
    if (response.hovered() || response.dragged())
        && ui.input(|i| i.focused)
        && ui.rect_contains_pointer(rect)
        && let Some(at) = ui.ctx().pointer_latest_pos()
    {
        // Windows crosshair cursors can be white on a white sheet. Draw a
        // constant-size black cross with a white halo, also visible on dark ink.
        // Follow the actual pointer; placement coordinates snap independently.
        let lines = [
            [at - Vec2::new(7.0, 0.0), at + Vec2::new(7.0, 0.0)],
            [at - Vec2::new(0.0, 7.0), at + Vec2::new(0.0, 7.0)],
        ];
        for (width, ink) in [(3.0, Color32::WHITE), (1.0, Color32::BLACK)] {
            for line in lines {
                painter.line_segment(line, Stroke::new(width, ink));
            }
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }
    CanvasOutput {
        cursor_mm: response.hover_pos().map(|p| view.camera.world_mm(p, rect)),
        schematic_input: Some(SchematicInput {
            panning,
            cancelled: interrupted,
            modifiers: Some(modifiers),
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
                .filter(|p| rect.contains(*p) && ui.rect_contains_pointer(rect))
                .map(|p| {
                    let mm = view.camera.world_mm(p, rect);
                    Point {
                        x: (mm[0] * 1e6).round() as i64,
                        y: (mm[1] * 1e6).round() as i64,
                    }
                }),
            press_origin: origin.filter(|p| rect.contains(*p)).map(|p| {
                let mm = view.camera.world_mm(p, rect);
                Point {
                    x: (mm[0] * 1e6).round() as i64,
                    y: (mm[1] * 1e6).round() as i64,
                }
            }),
            hit,
            wire_handle: hit.and_then(|id| {
                let pointer = hit_pointer?;
                let mm = view.camera.world_mm(pointer, rect);
                let at = Point {
                    x: (mm[0] * 1e6).round() as i64,
                    y: (mm[1] * 1e6).round() as i64,
                };
                eda_core::schematic_edit::wire_handle_at(
                    doc.schematic.as_ref()?,
                    id,
                    at,
                    (6.0 / view.camera.pixels_per_mm * 1e6) as i64,
                )
            }),
            clicked: response.clicked() && !dragging && !interrupted && !suppressed,
            double_clicked: response.double_clicked() && !dragging && !interrupted && !suppressed,
            secondary_clicked: response.secondary_clicked(),
            drag_started,
            dragging: dragging && ui.input(|i| i.pointer.primary_down()),
            drag_stopped,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schematic_cursor_has_contrast_on_light_and_dark_sheets_and_stays_in_canvas() {
        for dpi in [1.0, 1.5, 2.0] {
            for background in [0xffffff, 0] {
                let ctx = egui::Context::default();
                ctx.set_pixels_per_point(dpi);
                let mut doc = Document::new(eda_core::DocumentKind::Schematic, "Cursor");
                let sheet = &mut doc.schematic.as_mut().unwrap().sheet;
                sheet.background = background;
                sheet.border = false;
                sheet.grid_visible = false;
                let mut view = View::default();
                let mut frame = |pointer: Pos2, focused, modal, tool_active| {
                    view.sch_tool_active = tool_active;
                    ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(600.0, 400.0),
                            )),
                            focused,
                            events: vec![egui::Event::PointerMoved(pointer)],
                            ..Default::default()
                        },
                        |ctx| {
                            egui::SidePanel::left("cursor_properties")
                                .exact_width(100.0)
                                .show(ctx, |ui| {
                                    ui.label("Properties");
                                });
                            egui::CentralPanel::default().frame(egui::Frame::NONE).show(
                                ctx,
                                |ui| {
                                    crate::canvas(ui, &doc, &mut view);
                                },
                            );
                            if modal {
                                egui::Modal::new(egui::Id::new("cursor_modal")).show(ctx, |ui| {
                                    ui.label("Dialog");
                                });
                            }
                        },
                    )
                };
                let pointer = Pos2::new(237.25, 149.75);
                for tool_active in [false, true] {
                    frame(pointer, true, false, tool_active);
                    let output = frame(pointer, true, false, tool_active);
                    assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::None);
                    let strokes: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|s| {
                            if let egui::Shape::LineSegment { points, stroke } = &s.shape {
                                (points[0].lerp(points[1], 0.5) == pointer)
                                    .then_some((*points, *stroke))
                            } else {
                                None
                            }
                        })
                        .collect();
                    assert_eq!(strokes.len(), 4);
                    for expected in [
                        Stroke::new(1.0, Color32::BLACK),
                        Stroke::new(3.0, Color32::WHITE),
                    ] {
                        assert_eq!(
                            strokes
                                .iter()
                                .filter(|(_, stroke)| *stroke == expected)
                                .count(),
                            2
                        );
                    }
                    for (points, stroke) in strokes {
                        assert_eq!(points[0].lerp(points[1], 0.5), pointer);
                        assert_eq!(points[0].distance(points[1]), 14.0);
                        assert!(
                            stroke == Stroke::new(1.0, Color32::BLACK)
                                || stroke == Stroke::new(3.0, Color32::WHITE)
                        );
                    }
                }
                for (at, focused, modal) in [
                    (pointer, false, false),
                    (Pos2::new(40.0, 120.0), true, false),
                    (pointer, true, true),
                ] {
                    frame(at, focused, modal, false);
                    let output = frame(at, focused, modal, false);
                    assert_ne!(output.platform_output.cursor_icon, egui::CursorIcon::None);
                }
            }
        }
    }

    #[test]
    fn auto_junction_cache_replaces_geometry_and_expires_with_view() {
        use eda_core::schematic_edit::{Placement, place, primitive};
        let mut view = View::default();
        let wire = |points| {
            primitive(
                K::Polyline {
                    points,
                    electrical: "Wire".into(),
                    closed: false,
                },
                None,
            )
        };
        let mut primitives = vec![
            wire(vec![Point::mm(0, 10), Point::mm(20, 10)]),
            wire(vec![Point::mm(10, 10), Point::mm(10, 20)]),
        ];
        assert_eq!(
            auto_junctions(&mut view.sch_junction_cache, primitives.iter().collect()).len(),
            1
        );
        primitives.pop();
        assert!(
            auto_junctions(&mut view.sch_junction_cache, primitives.iter().collect()).is_empty()
        );
        let mut power = place(Placement::Ground, Point::mm(10, 10), 0, "", "", 0)
            .unwrap()
            .primitives;
        primitives.append(&mut power);
        assert_eq!(
            auto_junctions(&mut view.sch_junction_cache, primitives.iter().collect()).len(),
            1
        );
        primitives[1].presentation.hidden = true;
        assert!(
            auto_junctions(&mut view.sch_junction_cache, primitives.iter().collect()).is_empty()
        );
        assert!(view.sch_junction_cache.as_ref().unwrap().1.is_empty());
    }

    fn text_context(pixels_per_point: f32) -> egui::Context {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        fonts.families.insert(
            FontFamily::Name("sch-sans".into()),
            fonts.families[&FontFamily::Proportional].clone(),
        );
        ctx.set_fonts(fonts);
        ctx.set_pixels_per_point(pixels_per_point);
        ctx
    }

    #[test]
    fn port_native_alignment_and_label_ink_render_independently_of_outline() {
        let ctx = text_context(1.0);
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0));
        let camera = Camera {
            pixels_per_mm: 10.0,
            ..Default::default()
        };
        let mut schematic = Schematic::blank();
        schematic.fonts[0].name = "Arial".into();
        let mut origins = vec![];
        for alignment in [1, 0, 2] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(rect),
                    ..Default::default()
                },
                |ctx| {
                    let scene = Scene {
                        painter: ctx.layer_painter(egui::LayerId::background()),
                        camera: &camera,
                        rect,
                        schematic: Some(&schematic),
                        document_id: Uuid::nil(),
                        library_preview: false,
                    };
                    let mut p = eda_core::schematic_edit::primitive(
                        K::Port {
                            position: Point::mm(0, 0),
                            width_nm: 30_000_000,
                            height_nm: 2_540_000,
                            style: 0,
                            io_type: 0,
                            name: "Signal".into(),
                            font: 0,
                        },
                        None,
                    );
                    p.color = 128;
                    p.presentation.text_color = Some(0xff0000);
                    p.presentation.alignment = Some(alignment);
                    scene.primitive(&p, None);
                },
            );
            let label = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    Shape::Text(t) => Some(t),
                    _ => None,
                })
                .unwrap();
            assert_eq!(label.fallback_color, Color32::BLUE);
            origins.push(label.pos.x + label.galley.rect.left());
        }
        assert!(
            origins[0] < origins[1] && origins[1] < origins[2],
            "Left, Center, Right: {origins:?}"
        );
        assert!(
            (origins[0] - 405.08).abs() < 0.1,
            "Left uses the native 20 mil inset"
        );
    }

    fn rendered_text(
        ctx: &egui::Context,
        zoom: f64,
        orientation: u32,
        justification: u32,
        frame: bool,
    ) -> (Vec<[f64; 2]>, usize) {
        let rect = Rect::from_min_size(Pos2::new(0.13, 0.17), Vec2::new(800.0, 600.0));
        let camera = Camera {
            centre_mm: [13.1, 21.7],
            pixels_per_mm: zoom,
            ..Default::default()
        };
        let mut schematic = Schematic::blank();
        schematic.fonts = vec![SchFont {
            name: "Arial".into(),
            height_nm: 2_540_000,
            bold: false,
            italic: false,
            underline: false,
        }];
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(rect),
                ..Default::default()
            },
            |ctx| {
                let scene = Scene {
                    painter: ctx.layer_painter(egui::LayerId::background()),
                    camera: &camera,
                    rect,
                    schematic: Some(&schematic),
                    document_id: Uuid::nil(),
                    library_preview: false,
                };
                if frame {
                    let p = eda_core::schematic_edit::primitive(
                        K::TextFrame {
                            a: Point::mm(10, 25),
                            b: Point::mm(29, 10),
                            text: "R1 Resistance 47k capacitor C12 output signal".into(),
                            font: 0,
                            alignment: justification % 3,
                            word_wrap: true,
                            show_border: false,
                        },
                        None,
                    );
                    scene.primitive(&p, None);
                } else {
                    scene.text(
                        scene.screen(Point::mm(10, 25)),
                        "R1 Resistance 47k C12",
                        0,
                        orientation,
                        justification,
                        Color32::BLACK,
                    );
                }
            },
        );
        let rows = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                Shape::Text(text) => Some(text.galley.rows.len()),
                _ => None,
            })
            .sum();
        let points = ctx
            .tessellate(output.shapes, output.pixels_per_point)
            .into_iter()
            .flat_map(|p| match p.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh
                    .vertices
                    .into_iter()
                    .map(|v| camera.world_mm(v.pos, rect))
                    .collect(),
                _ => Vec::new(),
            })
            .collect();
        (points, rows)
    }

    #[test]
    fn text_rotation_matches_document_geometry_for_every_angle_and_anchor() {
        let ctx = text_context(1.5);
        for justification in 0..9 {
            let (reference, _) = rendered_text(&ctx, 12., 0, justification, false);
            for turns in 0..4 {
                let (actual, _) = rendered_text(&ctx, 12., turns, justification, false);
                assert_eq!(reference.len(), actual.len());
                for (p, q) in reference.iter().zip(actual) {
                    let (x, y) = (p[0] - 10., p[1] - 25.);
                    let (x, y) = match turns {
                        0 => (x, y),
                        1 => (-y, x),
                        2 => (-x, -y),
                        _ => (y, -x),
                    };
                    assert!(
                        (q[0] - (10. + x)).abs() < 0.0002 && (q[1] - (25. + y)).abs() < 0.0002,
                        "turns={turns}, anchor={justification}: {p:?} -> {q:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn tiny_parameter_sets_omit_the_label_and_hidden_children_never_paint_or_hit() {
        let ctx = text_context(1.);
        let camera = Camera::default();
        for style in 0..2 {
            for orientation in 0..4 {
                let mut schematic = Schematic::blank();
                let parent = eda_core::schematic_directives::primitive(K::ParameterSet {
                    position: Point::mm(10, 10),
                    orientation,
                    name: "Parameter Set".into(),
                    style,
                    parameters: std::collections::BTreeMap::from([
                        ("ClassName".into(), "SecretClass".into()),
                        ("Visible".into(), "VisibleValue".into()),
                    ]),
                    rules: vec![],
                });
                let hidden = eda_core::schematic_simulation::parameter(
                    parent.id,
                    "ClassName",
                    "SecretClass",
                    Point::mm(12, 12),
                    0,
                    true,
                );
                let visible = eda_core::schematic_simulation::parameter(
                    parent.id,
                    "Visible",
                    "VisibleValue",
                    Point::mm(15, 15),
                    0,
                    false,
                );
                schematic.primitives = vec![parent, visible];
                schematic.parameter_objects.push(hidden);
                let output = ctx.run(egui::RawInput::default(), |ctx| {
                    let scene = Scene {
                        painter: ctx.layer_painter(egui::LayerId::background()),
                        camera: &camera,
                        rect: Rect::from_min_size(Pos2::ZERO, Vec2::splat(600.)),
                        schematic: Some(&schematic),
                        document_id: Uuid::nil(),
                        library_preview: false,
                    };
                    for p in schematic
                        .primitives
                        .iter()
                        .chain(&schematic.parameter_objects)
                    {
                        let (bounds, hit) =
                            scene.primitive(p, Some(scene.screen(Point::mm(12, 12))));
                        if p.presentation.hidden {
                            assert_eq!(bounds, Rect::NOTHING);
                            assert!(!hit);
                        }
                    }
                });
                let texts: Vec<_> = output
                    .shapes
                    .iter()
                    .filter_map(|s| match &s.shape {
                        Shape::Text(t) => Some(t.galley.job.text.as_str()),
                        _ => None,
                    })
                    .collect();
                assert!(texts.contains(&"VisibleValue"));
                assert!(!texts.contains(&"SecretClass"));
                assert_eq!(texts.contains(&"Parameter Set"), style == 0);
            }
        }
    }

    #[test]
    fn schematic_text_glyphs_and_anchors_stay_fixed_in_document_during_zoom() {
        for dpi in [1.0, 1.5, 2.0] {
            let ctx = text_context(dpi);
            for orientation in 0..4 {
                for justification in 0..9 {
                    let (reference, _) =
                        rendered_text(&ctx, 6.0, orientation, justification, false);
                    assert!(!reference.is_empty());
                    for zoom in [5.81, 5.99, 6.01, 6.19, 7.3, 12.4] {
                        let (actual, _) =
                            rendered_text(&ctx, zoom, orientation, justification, false);
                        assert_eq!(reference.len(), actual.len());
                        for (a, b) in reference.iter().zip(actual) {
                            assert!(
                                (a[0] - b[0]).abs() < 0.0002 && (a[1] - b[1]).abs() < 0.0002,
                                "Glyph moved in document: dpi={dpi} orientation={orientation} anchor={justification} zoom={zoom}: {a:?} -> {b:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn schematic_text_frame_wrap_and_glyphs_do_not_change_with_zoom() {
        for dpi in [1.0, 1.5, 2.0] {
            let ctx = text_context(dpi);
            for alignment in 0..3 {
                let (reference, rows) = rendered_text(&ctx, 6.0, 0, alignment, true);
                assert!(rows > 1);
                assert!(!reference.is_empty());
                for zoom in [5.81, 5.99, 6.01, 6.19, 7.3, 12.4] {
                    let (actual, actual_rows) = rendered_text(&ctx, zoom, 0, alignment, true);
                    assert_eq!(rows, actual_rows);
                    assert_eq!(reference.len(), actual.len());
                    for (a, b) in reference.iter().zip(actual) {
                        assert!((a[0] - b[0]).abs() < 0.0002 && (a[1] - b[1]).abs() < 0.0002);
                    }
                }
            }
        }
    }

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

fn select_hit(
    view: &mut View,
    hit: Option<Uuid>,
    span: Option<eda_core::schematic_edit::WireSegment>,
    additive: bool,
) {
    if view.sch_selection.is_empty() && view.sch_segments.is_empty() {
        view.sch_selection.extend(view.selected);
    }
    let repeated = hit.is_some_and(|id| {
        view.sch_selection.contains(&id) || view.sch_segments.iter().any(|s| s.id == id)
    });
    if !additive {
        view.sch_selection.clear();
        view.sch_segments.clear();
    }
    if let Some(id) = hit {
        if let Some(span) = span.filter(|_| repeated) {
            view.sch_selection.retain(|old| *old != id);
            if let Some(index) = view.sch_segments.iter().position(|old| *old == span) {
                view.sch_segments.remove(index);
            } else {
                view.sch_segments.push(span);
            }
        } else if additive && view.sch_selection.contains(&id) {
            view.sch_selection.retain(|old| *old != id);
        } else if !repeated || additive {
            view.sch_selection.push(id);
        }
    }
    view.selected = view
        .sch_selection
        .first()
        .copied()
        .or_else(|| view.sch_segments.first().map(|s| s.id));
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    #[test]
    fn repeated_wire_click_selects_span_and_shift_keeps_mixed_group() {
        let part = Uuid::new_v4();
        let wire = Uuid::new_v4();
        let span = eda_core::schematic_edit::WireSegment {
            id: wire,
            a: Point::mm(0, 0),
            b: Point::mm(10, 0),
        };
        let mut v = View::default();
        select_hit(&mut v, Some(part), None, false);
        select_hit(&mut v, Some(wire), Some(span), true);
        assert_eq!(v.sch_selection, vec![part, wire]);
        assert!(v.sch_segments.is_empty());
        select_hit(&mut v, Some(wire), Some(span), true);
        assert_eq!(v.sch_selection, vec![part]);
        assert_eq!(v.sch_segments, vec![span]);
        select_hit(&mut v, Some(wire), Some(span), true);
        assert!(v.sch_segments.is_empty());
        assert_eq!(v.sch_selection, vec![part]);
        select_hit(&mut v, None, None, false);
        assert!(v.selected.is_none());
        assert!(v.sch_selection.is_empty());
    }
}

fn selection_category(p: &SchPrimitive) -> usize {
    use eda_core::schematic::TextRole;
    if p.text_role == Some(TextRole::NetLabel) {
        5
    } else if p.text_role.is_some()
        || matches!(p.kind, K::Text { .. }) && !p.presentation.name.is_empty()
    {
        6
    } else if p.component.is_some() {
        0
    } else {
        match &p.kind {
            K::Polyline { electrical, .. } if electrical == "Wire" => 1,
            K::Polyline { electrical, .. } if electrical == "Bus" => 2,
            K::SheetSymbol { .. } => 3,
            K::SheetEntry { .. } => 4,
            K::Port { .. } => 7,
            K::PowerPort { .. } => 8,
            K::Text { .. } | K::TextFrame { .. } => 9,
            K::Junction { .. }
            | K::NoErc { .. }
            | K::ParameterSet { .. }
            | K::Blanket { .. }
            | K::CompileMask { .. } => 11,
            _ => 10,
        }
    }
}

#[cfg(test)]
#[path = "schematic_interaction_tests.rs"]
mod interaction_tests;
