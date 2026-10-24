//! Flat, physical lettering. Font contours retain counters at any camera angle.
use eda_core::pcb::*;
use eda_geom::Point;
use ttf_parser::{Face, OutlineBuilder};

#[derive(Default)]
struct Contours {
    rings: Vec<Vec<[f64; 2]>>,
    current: Vec<[f64; 2]>,
}
impl OutlineBuilder for Contours {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.current.push([x as f64, y as f64]);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.current.push([x as f64, y as f64]);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let Some(&a) = self.current.last() else {
            return;
        };
        for i in 1..=12 {
            let t = i as f64 / 12.;
            let s = 1. - t;
            self.current.push([
                s * s * a[0] + 2. * s * t * x1 as f64 + t * t * x as f64,
                s * s * a[1] + 2. * s * t * y1 as f64 + t * t * y as f64,
            ]);
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let Some(&a) = self.current.last() else {
            return;
        };
        for i in 1..=16 {
            let t = i as f64 / 16.;
            let s = 1. - t;
            self.current.push([
                s * s * s * a[0]
                    + 3. * s * s * t * x1 as f64
                    + 3. * s * t * t * x2 as f64
                    + t * t * t * x as f64,
                s * s * s * a[1]
                    + 3. * s * s * t * y1 as f64
                    + 3. * s * t * t * y2 as f64
                    + t * t * t * y as f64,
            ]);
        }
    }
    fn close(&mut self) {
        if self.current.len() >= 3 {
            self.rings.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
}
pub(super) fn outlines(item: &Item) -> Vec<StackSurface> {
    let Shape::Text {
        position,
        text,
        height,
        rotation,
        mirrored,
        ..
    } = &item.shape
    else {
        return vec![];
    };
    let bold = item
        .properties
        .get("PCBCRAFT_TEXT_BOLD")
        .is_some_and(|v| v == "1");
    let italic = item
        .properties
        .get("PCBCRAFT_TEXT_ITALIC")
        .is_some_and(|v| v == "1");
    let font: &[u8] = match (bold, italic) {
        (true, true) => {
            include_bytes!("../../../assets/fonts/liberation/LiberationSans-BoldItalic.ttf")
        }
        (true, false) => include_bytes!("../../../assets/fonts/liberation/LiberationSans-Bold.ttf"),
        (false, true) => {
            include_bytes!("../../../assets/fonts/liberation/LiberationSans-Italic.ttf")
        }
        _ => include_bytes!("../../../assets/fonts/liberation/LiberationSans-Regular.ttf"),
    };
    let Ok(face) = Face::parse(font, 0) else {
        return vec![];
    };
    let scale = *height as f64 / f64::from(face.capital_height().unwrap_or(1400));
    let mut x = 0.;
    let mut y = 0.;
    let mut rings = Vec::<Vec<Point>>::new();
    let (s, c) = rotation.to_radians().sin_cos();
    for ch in text.chars().take(16_384) {
        if ch == '\n' {
            x = 0.;
            y -= f64::from(face.height());
            continue;
        }
        let Some(id) = face.glyph_index(ch) else {
            continue;
        };
        let mut contours = Contours::default();
        face.outline_glyph(id, &mut contours);
        contours.close();
        for ring in contours.rings {
            rings.push(
                ring.into_iter()
                    .map(|p| {
                        let px = (p[0] + x) * scale * if *mirrored { -1. } else { 1. };
                        let py = (p[1] + y) * scale;
                        Point {
                            x: position.x + (px * c - py * s).round() as i64,
                            y: position.y + (px * s + py * c).round() as i64,
                        }
                    })
                    .collect(),
            );
        }
        x += f64::from(face.glyph_hor_advance(id).unwrap_or(0));
    }
    let depth: Vec<_> = rings
        .iter()
        .enumerate()
        .map(|(i, r)| {
            rings
                .iter()
                .enumerate()
                .filter(|(j, p)| *j != i && point_in_polygon(r[0], p))
                .count()
        })
        .collect();
    rings
        .iter()
        .enumerate()
        .filter(|(i, _)| depth[*i] % 2 == 0)
        .map(|(i, r)| StackSurface {
            outline: r.clone(),
            holes: rings
                .iter()
                .enumerate()
                .filter(|(j, h)| depth[*j] == depth[i] + 1 && point_in_polygon(h[0], r))
                .map(|(_, h)| h.clone())
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lettering_has_real_glyph_counters_and_tracks_document_mirroring() {
        let mut item = Item::new(
            TOP_OVERLAY,
            None,
            Shape::Text {
                position: Point::mm(10, 10),
                text: "O8".into(),
                height: 2_000_000,
                stroke: 100_000,
                rotation: 0.,
                mirrored: false,
                font: String::new(),
            },
        );
        let glyphs = outlines(&item);
        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[0].holes.len(), 1);
        assert_eq!(glyphs[1].holes.len(), 2);
        if let Shape::Text {
            mirrored, rotation, ..
        } = &mut item.shape
        {
            *mirrored = true;
            *rotation = 90.;
        }
        let changed = outlines(&item);
        let a = glyphs[0].outline[0];
        let b = changed[0].outline[0];
        assert_eq!(
            b,
            Point {
                x: 20_000_000 - a.y,
                y: 20_000_000 - a.x
            }
        );
    }
}
