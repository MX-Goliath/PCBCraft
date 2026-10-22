//! Document-space PCB lettering. Fixed-size layout avoids zoom-dependent metrics.
use eda_core::pcb::*;
use egui::{Color32, FontFamily, FontId, Pos2, Rect};
use std::sync::Arc;

pub(super) fn paint(
    p: &egui::Painter,
    item: &Item,
    position: Pos2,
    pixels_per_mm: f64,
    color: Color32,
) {
    let Shape::Text {
        text,
        height,
        rotation,
        mirrored,
        ..
    } = &item.shape
    else {
        return;
    };
    let screen_height = (*height as f64 / 1e6 * pixels_per_mm) as f32;
    if screen_height < 1.0 || text.is_empty() {
        return;
    }
    let bold = item
        .properties
        .get("PCBCRAFT_TEXT_BOLD")
        .is_some_and(|v| v == "1");
    let italic = item
        .properties
        .get("PCBCRAFT_TEXT_ITALIC")
        .is_some_and(|v| v == "1");
    let family = FontFamily::Name(
        format!(
            "sch-sans{}{}",
            if bold { "-bold" } else { "" },
            if italic { "-italic" } else { "" }
        )
        .into(),
    );
    let font = FontId::new(64.0, family);
    let mut galley = p.layout_no_wrap(text.clone(), font.clone(), color);
    let cap = p.layout_no_wrap("H".into(), font, color);
    let mut scale = screen_height / cap.mesh_bounds.height().max(1.0);
    let mut sx = 1.0;
    if item.properties.get("PCBCRAFT_TEXT_CACHE_VALUE") == Some(text)
        && item
            .properties
            .get("PCBCRAFT_TEXT_CACHE_SOURCE_HEIGHT")
            .is_some_and(|v| v == &height.to_string())
    {
        let parse = |key| {
            item.properties
                .get(key)
                .and_then(|v| v.parse::<i64>().ok())
                .filter(|v| *v > 0 && *v <= 100_000_000_000)
        };
        if let (Some(w), Some(h)) = (
            parse("PCBCRAFT_TEXT_CACHE_WIDTH"),
            parse("PCBCRAFT_TEXT_CACHE_HEIGHT"),
        ) {
            scale = (h as f64 / 1e6 * pixels_per_mm) as f32 / galley.size().y.max(1.0);
            sx = (w as f64 / 1e6 * pixels_per_mm) as f32 / (galley.size().x * scale).max(0.001);
        }
    }
    let bottom = galley.rect.max.y;
    let map = |v: Pos2| Pos2::new(v.x * sx * if *mirrored { -1.0 } else { 1.0 }, v.y - bottom);
    let bounds = |r: Rect| Rect::from_two_pos(map(r.min), map(r.max));
    let g = Arc::make_mut(&mut galley);
    g.rect = bounds(g.rect);
    g.mesh_bounds = bounds(g.mesh_bounds);
    for row in &mut g.rows {
        let row = Arc::make_mut(&mut row.row);
        for v in &mut row.visuals.mesh.vertices {
            v.pos = map(v.pos);
        }
        row.visuals.mesh_bounds = bounds(row.visuals.mesh_bounds);
    }
    let shape = egui::epaint::TextShape::new(Pos2::ZERO, galley, color)
        .with_angle(-(*rotation as f32).to_radians());
    p.add(crate::schematic::place_text(
        shape,
        scale,
        position,
        p.ctx().pixels_per_point(),
    ));
}

pub(super) fn label(
    p: &egui::Painter,
    position: Pos2,
    text: &str,
    height: f32,
    max_width: f32,
    color: Color32,
) {
    if height < 5.0 || max_width < 3.0 || text.is_empty() {
        return;
    }
    let galley = p.layout_no_wrap(
        text.into(),
        FontId::new(64.0, FontFamily::Name("sch-sans".into())),
        color,
    );
    let scale = (height / galley.size().y.max(1.0)).min(max_width / galley.size().x.max(1.0));
    let offset = -galley.size() * 0.5;
    let shape = egui::epaint::TextShape::new(offset.to_pos2(), galley, color);
    p.add(crate::schematic::place_text(
        shape,
        scale,
        position,
        p.ctx().pixels_per_point(),
    ));
}

pub(super) fn track_labels(
    p: &egui::Painter,
    a: Pos2,
    b: Pos2,
    value: &str,
    height: f32,
    color: Color32,
) {
    let length = a.distance(b);
    if height < 5. || length < 45. || value.is_empty() {
        return;
    }
    let galley = p.layout_no_wrap(
        value.into(),
        FontId::new(64., FontFamily::Name("sch-sans".into())),
        color,
    );
    let scale = (height / galley.size().y.max(1.)).min(length * 0.85 / galley.size().x.max(1.));
    let spacing = (galley.size().x * scale + height * 4.).max(50.);
    let count = ((length / spacing).floor() as usize).clamp(1, 128);
    let d = b - a;
    let mut angle = d.y.atan2(d.x);
    if angle > std::f32::consts::FRAC_PI_2 {
        angle -= std::f32::consts::PI;
    }
    if angle < -std::f32::consts::FRAC_PI_2 {
        angle += std::f32::consts::PI;
    }
    for n in 0..count {
        let shape =
            egui::epaint::TextShape::new((-galley.size() * 0.5).to_pos2(), galley.clone(), color)
                .with_angle_and_anchor(angle, egui::Align2::CENTER_CENTER);
        p.add(crate::schematic::place_text(
            shape,
            scale,
            a.lerp(b, (n as f32 + 0.5) / count as f32),
            p.ctx().pixels_per_point(),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_geom::Point;
    #[test]
    fn track_names_repeat_on_long_segments_and_follow_readable_track_direction() {
        let ctx = context(1.0);
        let output = ctx.run(Default::default(), |ctx| {
            let p = ctx.layer_painter(egui::LayerId::background());
            track_labels(
                &p,
                Pos2::new(100., 100.),
                Pos2::new(100., 900.),
                "GND",
                12.,
                Color32::WHITE,
            );
        });
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert!(labels.len() > 2);
        assert!(
            labels
                .iter()
                .all(|t| (t.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5)
        );
        assert!(labels.windows(2).all(|pair| pair[0].pos.y < pair[1].pos.y));
    }
    fn context(dpi: f32) -> egui::Context {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(dpi);
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "test-face".into(),
            Arc::new(egui::FontData::from_static(include_bytes!(
                "../../../assets/fonts/liberation/LiberationSans-Regular.ttf"
            ))),
        );
        fonts.families.insert(
            FontFamily::Name("sch-sans".into()),
            vec!["test-face".into()],
        );
        ctx.set_fonts(fonts);
        ctx
    }
    fn points(ctx: &egui::Context, zoom: f64, mirrored: bool, angle: f64) -> Vec<[f64; 2]> {
        let item = Item::new(
            TOP,
            None,
            Shape::Text {
                position: Point::default(),
                text: "Carbon T201".into(),
                height: 1_000_000,
                stroke: 100_000,
                rotation: angle,
                mirrored,
                font: "Arial".into(),
            },
        );
        let origin = Pos2::new(200.25, 200.75);
        let output = ctx.run(Default::default(), |ctx| {
            paint(
                &ctx.layer_painter(egui::LayerId::background()),
                &item,
                origin,
                zoom,
                Color32::YELLOW,
            )
        });
        ctx.tessellate(output.shapes, output.pixels_per_point)
            .into_iter()
            .flat_map(|s| match s.primitive {
                egui::epaint::Primitive::Mesh(m) => m
                    .vertices
                    .into_iter()
                    .map(|v| {
                        [
                            (v.pos.x - origin.x) as f64 / zoom,
                            (v.pos.y - origin.y) as f64 / zoom,
                        ]
                    })
                    .collect(),
                _ => Vec::new(),
            })
            .collect()
    }
    #[test]
    fn pcb_glyphs_stay_in_document_space_during_zoom_mirror_and_rotation() {
        for dpi in [1.0, 1.5, 2.0] {
            let ctx = context(dpi);
            let base = points(&ctx, 13.0, false, 0.0);
            assert!(!base.is_empty());
            for zoom in [5.81, 12.99, 13.01, 21.7, 50.1] {
                for (mirror, angle) in [(false, 0.0), (true, 0.0), (false, 90.0), (true, 270.0)] {
                    let result = points(&ctx, zoom, mirror, angle);
                    assert_eq!(base.len(), result.len());
                    let (s, c) = (-angle).to_radians().sin_cos();
                    for ([mut x, y], a) in base.iter().copied().zip(result) {
                        if mirror {
                            x = -x;
                        }
                        assert!(
                            (a[0] - (x * c - y * s)).abs() < 0.0001
                                && (a[1] - (x * s + y * c)).abs() < 0.0001,
                            "dpi={dpi} zoom={zoom} mirror={mirror} angle={angle}"
                        );
                    }
                }
            }
        }
    }
}
