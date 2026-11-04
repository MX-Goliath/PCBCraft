//! Original vector renditions of the observed PCB toolbar symbols.
use egui::{Color32, Pos2, Rect, Shape, Stroke, StrokeKind};
pub(super) fn menu_icon(id: &str, group: usize) -> bool {
    group >= 5
        || group == 4
            && !matches!(
                id,
                "AutopositionComponentTexts"
                    | "AlignComponentsLeftSideBySide"
                    | "AlignComponentsRightSideBySide"
                    | "AlignComponentsTopSideBySide"
                    | "AlignComponentsBottomSideBySide"
            )
        || id == "SelectInsideArea"
        || matches!(id, "MoveSelection" | "OffsetSelection")
}
pub(super) fn paint(p: &egui::Painter, r: Rect, group: usize, id: &str, enabled: bool) {
    let ink = |c| if enabled { c } else { Color32::from_gray(101) };
    let blue = ink(Color32::from_rgb(123, 181, 222));
    let pale = ink(Color32::from_rgb(187, 226, 242));
    let gold = ink(Color32::from_rgb(228, 190, 68));
    let red = ink(Color32::from_rgb(214, 101, 119));
    let at = |x: f32, y: f32| {
        Pos2::new(
            r.left() + x * r.width() / 22.,
            r.top() + y * r.height() / 22.,
        )
    };
    let line = |a: (f32, f32), b: (f32, f32), color, w| {
        p.line_segment(
            [at(a.0, a.1), at(b.0, b.1)],
            Stroke::new(w * r.width() / 22., color),
        );
    };
    let rect = |a: (f32, f32), b: (f32, f32), fill, stroke| {
        p.rect(
            Rect::from_min_max(at(a.0, a.1), at(b.0, b.1)),
            0.,
            fill,
            Stroke::new(1., stroke),
            StrokeKind::Inside,
        );
    };
    let ring = |x, y, radius| {
        p.circle_stroke(at(x, y), radius * r.width() / 22., Stroke::new(1.1, gold));
    };
    match group {
        0 => {
            p.add(Shape::convex_polygon(
                vec![
                    at(3., 3.),
                    at(19., 3.),
                    at(13., 11.),
                    at(13., 19.),
                    at(9., 17.),
                    at(9., 11.),
                ],
                pale,
                Stroke::NONE,
            ));
            p.circle_filled(
                at(17., 17.),
                3.5 * r.width() / 22.,
                ink(Color32::from_rgb(149, 212, 60)),
            );
        }
        1 => {
            line((4., 6.), (15., 6.), pale, 3.);
            line((15., 6.), (19., 9.), red, 3.);
            line((19., 9.), (15., 14.), red, 3.);
            line((15., 14.), (4., 14.), red, 3.);
            line((4., 5.), (4., 9.), pale, 2.);
            line((4., 12.), (4., 16.), pale, 2.);
        }
        2 => {
            line((11., 2.), (11., 21.), blue, 1.);
            line((2., 11.), (21., 11.), blue, 1.);
            line((8., 5.), (11., 2.), blue, 1.);
            line((11., 2.), (14., 5.), blue, 1.);
        }
        3 => {
            for n in 0..5 {
                let t = 3. + n as f32 * 3.5;
                line((t, 3.), (t + 1.5, 3.), blue, 1.);
                line((t, 19.), (t + 1.5, 19.), blue, 1.);
                line((3., t), (3., t + 1.5), blue, 1.);
                line((19., t), (19., t + 1.5), blue, 1.);
            }
        }
        4 => {
            for (n, h) in [7., 14., 10.].into_iter().enumerate() {
                let x = 3. + n as f32 * 6.;
                rect(
                    (x, 19. - h),
                    (x + 4., 19.),
                    if n == 1 { red } else { pale },
                    blue,
                );
            }
            line((1., 21.), (21., 21.), blue, 1.2);
            if id.contains("Left") {
                line((1., 2.), (1., 21.), blue, 1.2);
            } else if id.contains("Right") {
                line((21., 2.), (21., 21.), blue, 1.2);
            } else if id.contains("Top") {
                line((1., 2.), (21., 2.), blue, 1.2);
            }
        }
        5 => {
            rect((5., 5.), (18., 18.), blue, pale);
            for n in 0..5 {
                let t = 6. + n as f32 * 2.5;
                line((t, 2.), (t, 5.), pale, 1.);
                line((t, 18.), (t, 21.), pale, 1.);
                line((2., t), (5., t), pale, 1.);
                line((18., t), (21., t), pale, 1.);
            }
        }
        6 => {
            ring(4., 18., 2.);
            ring(17., 4., 2.);
            line((4., 15.), (4., 6.), gold, 1.4);
            line((4., 6.), (10., 6.), gold, 1.4);
            line((10., 6.), (10., 13.), gold, 1.4);
            line((10., 13.), (16., 7.), blue, 1.4);
            if id.contains("Diff") || id.contains("Multi") {
                line((7., 18.), (7., 9.), gold, 1.);
                line((7., 9.), (13., 9.), gold, 1.);
            }
        }
        7 => {
            for n in 0..4 {
                let x = 2. + 5. * n as f32;
                line(
                    (x, if n % 2 == 0 { 16. } else { 4. }),
                    (x + 5., if n % 2 == 0 { 4. } else { 16. }),
                    blue,
                    1.5,
                );
            }
            ring(2., 16., 2.);
            ring(21., 16., 2.);
        }
        8 => {
            p.circle_filled(at(11., 10.), 7. * r.width() / 22., gold);
            p.circle_filled(
                at(11., 10.),
                3.2 * r.width() / 22.,
                ink(Color32::from_gray(61)),
            );
            if id != "PlacePad" {
                line((11., 17.), (11., 22.), blue, 2.5);
            }
        }
        9 => {
            if id == "PlaceCutoutRegion" {
                rect((2., 3.), (20., 19.), pale, blue);
                rect((6., 7.), (16., 15.), ink(Color32::from_gray(61)), blue);
            } else {
                p.add(Shape::convex_polygon(
                    vec![
                        at(2., 13.),
                        at(9., 13.),
                        at(9., 4.),
                        at(20., 4.),
                        at(20., 19.),
                        at(2., 19.),
                    ],
                    pale,
                    Stroke::new(1., blue),
                ));
            }
        }
        10 => {
            rect(
                (1., 1.),
                (21., 21.),
                Color32::TRANSPARENT,
                ink(Color32::from_rgb(196, 146, 218)),
            );
            if id.contains("Fill") {
                rect((4., 4.), (18., 18.), pale, blue);
            } else if id.contains("Region") {
                rect((4., 9.), (18., 18.), pale, blue);
                rect((10., 4.), (18., 12.), pale, blue);
            } else {
                ring(6., 16., 1.5);
                ring(16., 6., 1.5);
                line((7., 15.), (15., 7.), gold, 1.);
            }
        }
        11 => {
            line((4., 4.), (4., 19.), blue, 1.);
            line((18., 4.), (18., 19.), blue, 1.);
            line((4., 18.), (18., 18.), blue, 1.);
            ring(4., 3., 2.);
            ring(18., 3., 2.);
            p.text(
                at(11., 10.),
                egui::Align2::CENTER_CENTER,
                "10",
                egui::FontId::proportional(r.height() * 0.43),
                pale,
            );
        }
        12 => {
            p.text(
                at(11., 11.),
                egui::Align2::CENTER_CENTER,
                "A",
                egui::FontId::proportional(r.height() * 1.1),
                blue,
            );
            if id == "PlaceFrame" {
                rect((1., 1.), (21., 21.), Color32::TRANSPARENT, pale);
            }
        }
        _ => {
            if id.contains("Arc") || id == "PlaceFullCircle" {
                let end = if id == "PlaceFullCircle" {
                    std::f32::consts::TAU
                } else {
                    std::f32::consts::PI * 1.45
                };
                let points = (0..=32)
                    .map(|n| {
                        let a = 0.2 + end * n as f32 / 32.;
                        at(11. + 8. * a.cos(), 11. + 8. * a.sin())
                    })
                    .collect();
                p.add(Shape::line(points, Stroke::new(1., gold)));
            } else if id == "PlaceFill" {
                rect((2., 4.), (20., 18.), pale, blue);
            } else if id == "PlaceRectangle" {
                rect((2., 4.), (20., 18.), Color32::TRANSPARENT, pale);
            } else if id == "PlaceSolidRegion" {
                p.add(Shape::convex_polygon(
                    vec![
                        at(2., 13.),
                        at(9., 13.),
                        at(9., 4.),
                        at(20., 4.),
                        at(20., 19.),
                        at(2., 19.),
                    ],
                    pale,
                    Stroke::new(1., blue),
                ));
            } else {
                line((3., 19.), (19., 3.), blue, 2.5);
            }
        }
    }
}
