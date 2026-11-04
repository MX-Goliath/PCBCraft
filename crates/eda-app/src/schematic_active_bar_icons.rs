//! Original vector drawings of the observed toolbar symbols, in a 22-point grid.
use egui::{Color32, Pos2, Rect, Shape, Stroke};
#[derive(Clone, Copy, Debug)]
pub(in crate::app) enum Glyph {
    Pin,
    Filter,
    Move,
    Select,
    Align,
    Alignment(u8),
    Part,
    Wire,
    Power,
    Harness,
    Sheet,
    Port,
    Directive,
    NoErc,
    Text,
    Arc,
    Polygon,
    Source,
    Probe(u8),
    Initial,
    Stack(u8),
    Bus,
    BusEntry,
    NetLabel,
    HarnessConnector,
    HarnessEntry,
    SheetEntry,
    DeviceSheet,
    Reuse,
    OffSheet,
    Differential,
    Blanket,
    Mask,
    Frame,
    Note,
    Circle,
    Ellipse,
    EllipticalArc,
    Line,
    Rectangle,
    Rounded,
    Bezier,
    Graphic,
    PowerPreset(u8),
}
pub(in crate::app) fn paint(p: &egui::Painter, r: Rect, g: Glyph, enabled: bool) {
    let ink = |c: Color32| if enabled { c } else { Color32::from_gray(95) };
    let blue = ink(Color32::from_rgb(101, 149, 205));
    let pale = ink(Color32::from_rgb(184, 213, 245));
    let gold = ink(Color32::from_rgb(217, 189, 104));
    let red = ink(Color32::from_rgb(238, 84, 84));
    let green = ink(Color32::from_rgb(126, 191, 135));
    let at = |x: f32, y: f32| {
        Pos2::new(
            r.left() + x * r.width() / 22.0,
            r.top() + y * r.height() / 22.0,
        )
    };
    let line = |a: (f32, f32), b: (f32, f32), c: Color32, w: f32| {
        p.line_segment(
            [at(a.0, a.1), at(b.0, b.1)],
            Stroke::new(w * r.width() / 22.0, c),
        );
    };
    let poly = |xy: &[(f32, f32)], fill: Color32, stroke: Color32| {
        p.add(Shape::convex_polygon(
            xy.iter().map(|(x, y)| at(*x, *y)).collect(),
            fill,
            Stroke::new(1.0, stroke),
        ));
    };
    match g {
        Glyph::Pin => {
            line((1., 11.), (14., 11.), blue, 1.4);
            line((14., 6.), (14., 16.), gold, 2.);
            line((17., 8.), (17., 14.), gold, 1.);
        }
        Glyph::Alignment(n) => {
            let cyan = ink(Color32::from_rgb(155, 211, 231));
            if n == 8 {
                for x in [4., 11., 18.] {
                    line((x, 2.), (x, 21.), pale, 0.7);
                }
                for y in [5., 11., 18.] {
                    line((1., y), (21., y), blue, 0.7);
                }
                poly(&[(6., 7.), (16., 7.), (16., 16.), (6., 16.)], cyan, blue);
            } else if n == 3 || n == 7 {
                let transpose = |x, y| if n == 3 { (x, y) } else { (y, x) };
                line(transpose(2., 11.), transpose(21., 11.), red, 1.);
                for (x, h) in [(3., 5.), (10., 10.), (18., 4.)] {
                    let xy = [
                        (x, 11. - h / 2.),
                        (x + 3., 11. - h / 2.),
                        (x + 3., 11. + h / 2.),
                        (x, 11. + h / 2.),
                    ]
                    .map(|(x, y)| transpose(x, y));
                    poly(&xy, cyan, blue);
                }
            } else {
                let vertical = n >= 4;
                let mode = if vertical { n - 4 } else { n };
                let boundary = [3., 19., 11.][mode as usize];
                let transpose = |x, y| if vertical { (y, x) } else { (x, y) };
                line(transpose(boundary, 1.), transpose(boundary, 21.), blue, 1.);
                for (y, w) in [(3., 8.), (9., 13.), (16., 6.)] {
                    let x = match mode {
                        0 => boundary + 2.,
                        1 => boundary - w - 2.,
                        _ => boundary - w / 2.,
                    };
                    let xy = [(x, y), (x + w, y), (x + w, y + 3.), (x, y + 3.)]
                        .map(|(x, y)| transpose(x, y));
                    poly(&xy, if y == 3. { red } else { cyan }, blue);
                }
            }
        }
        Glyph::Stack(n) => {
            let red_rect = [(2., 3.), (13., 3.), (13., 14.), (2., 14.)];
            let blue_rect = [(9., 10.), (20., 10.), (20., 21.), (9., 21.)];
            let pink = ink(Color32::from_rgb(240, 147, 150));
            if n % 2 == 0 {
                poly(&blue_rect, pale, blue);
                poly(&red_rect, pink, red);
            } else {
                poly(&red_rect, pink, red);
                poly(&blue_rect, pale, blue);
            }
            if n >= 2 {
                line((1., 18.), (1., 21.), pale, 1.);
                line((1., 21.), (5., 21.), pale, 1.);
            }
        }
        Glyph::Bus | Glyph::BusEntry => {
            poly(&[(1., 2.), (21., 2.), (21., 6.), (1., 6.)], gold, gold);
            for x in [4., 10., 16.] {
                line((x, 6.), (x, 20.), blue, 1.2);
                line(
                    (x, 10.),
                    (x + 3., 13.),
                    if matches!(g, Glyph::BusEntry) {
                        red
                    } else {
                        pale
                    },
                    1.,
                );
            }
        }
        Glyph::NetLabel => {
            p.text(
                at(11., 11.),
                egui::Align2::CENTER_CENTER,
                "Net",
                egui::FontId::proportional(13.),
                blue,
            );
            line((1., 18.), (21., 18.), blue, 1.);
        }
        Glyph::HarnessConnector | Glyph::HarnessEntry => {
            poly(&[(5., 2.), (16., 2.), (16., 19.), (5., 19.)], pale, pale);
            if matches!(g, Glyph::HarnessEntry) {
                poly(&[(5., 9.), (11., 7.), (11., 13.), (5., 11.)], blue, blue);
            } else {
                for y in [6., 11., 16.] {
                    line((0., y), (5., y), blue, 1.);
                    line((16., y), (21., y), blue, 1.);
                }
            }
        }
        Glyph::SheetEntry | Glyph::DeviceSheet | Glyph::Reuse => {
            paint(p, r, Glyph::Sheet, enabled);
            if matches!(g, Glyph::SheetEntry) {
                poly(
                    &[(3., 8.), (13., 8.), (17., 12.), (13., 16.), (3., 16.)],
                    gold,
                    gold,
                );
            } else if matches!(g, Glyph::DeviceSheet) {
                poly(
                    &[(12., 12.), (21., 12.), (21., 21.), (12., 21.)],
                    green,
                    blue,
                );
            } else {
                line((2., 2.), (19., 2.), gold, 2.);
                line((2., 2.), (2., 21.), gold, 2.);
            }
        }
        Glyph::OffSheet => {
            p.text(
                at(6., 11.),
                egui::Align2::CENTER_CENTER,
                "D1",
                egui::FontId::proportional(9.),
                blue,
            );
            for x in [12., 17.] {
                line((x, 7.), (x + 4., 11.), red, 1.);
                line((x + 4., 11.), (x, 15.), red, 1.);
            }
        }
        Glyph::Differential => {
            poly(
                &[
                    (6., 7.),
                    (16., 7.),
                    (20., 11.),
                    (16., 15.),
                    (6., 15.),
                    (2., 11.),
                ],
                Color32::TRANSPARENT,
                red,
            );
            line((0., 11.), (6., 11.), red, 1.);
            line((16., 11.), (22., 11.), red, 1.);
        }
        Glyph::Blanket | Glyph::Mask => {
            for (a, b) in [
                ((2., 3.), (21., 3.)),
                ((21., 3.), (21., 20.)),
                ((21., 20.), (2., 20.)),
                ((2., 20.), (2., 3.)),
            ] {
                if matches!(g, Glyph::Blanket) {
                    for t in [0., 0.3, 0.6, 0.9] {
                        line(
                            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t),
                            (a.0 + (b.0 - a.0) * (t + 0.1), a.1 + (b.1 - a.1) * (t + 0.1)),
                            red,
                            1.,
                        );
                    }
                } else {
                    line(a, b, red, 1.);
                }
            }
            poly(&[(4., 5.), (7., 5.), (5.5, 8.)], red, red);
        }
        Glyph::Frame | Glyph::Note => {
            poly(
                &[(2., 3.), (20., 3.), (20., 20.), (2., 20.)],
                if matches!(g, Glyph::Note) {
                    gold
                } else {
                    Color32::TRANSPARENT
                },
                blue,
            );
            for y in [7., 11., 15.] {
                line((5., y), (17., y), blue, 1.);
            }
        }
        Glyph::Circle | Glyph::Ellipse | Glyph::EllipticalArc => {
            let ry = if matches!(g, Glyph::Circle) { 9. } else { 6. };
            let sweep = if matches!(g, Glyph::EllipticalArc) {
                235.
            } else {
                360.
            };
            let points = (0..=40)
                .map(|n| {
                    let a = (n as f32 / 40. * sweep).to_radians();
                    at(11. + 9. * a.cos(), 11. - ry * a.sin())
                })
                .collect();
            if matches!(g, Glyph::Ellipse) {
                p.add(Shape::convex_polygon(points, pale, Stroke::new(1., blue)));
            } else {
                p.add(Shape::line(
                    points,
                    Stroke::new(
                        1.,
                        if matches!(g, Glyph::Circle) {
                            gold
                        } else {
                            blue
                        },
                    ),
                ));
                if matches!(g, Glyph::Circle) {
                    line((11., 11.), (5., 17.), gold, 1.);
                    p.circle_filled(at(11., 11.), 1., gold);
                } else {
                    line((11., 11.), (18., 8.), gold, 1.);
                    p.circle_filled(at(11., 11.), 1., gold);
                }
            }
        }
        Glyph::Line => line((3., 18.), (20., 3.), blue, 1.5),
        Glyph::Rectangle | Glyph::Rounded => {
            p.rect(
                r.shrink(3.),
                if matches!(g, Glyph::Rounded) { 3 } else { 0 },
                pale,
                Stroke::new(1., blue),
                egui::StrokeKind::Inside,
            );
        }
        Glyph::Bezier => {
            p.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [at(2., 18.), at(6., -4.), at(14., 27.), at(21., 4.)],
                false,
                Color32::TRANSPARENT,
                Stroke::new(1.5, blue),
            ));
        }
        Glyph::Graphic => {
            poly(&[(2., 2.), (20., 2.), (20., 21.), (2., 21.)], pale, blue);
            poly(
                &[(4., 18.), (10., 9.), (14., 14.), (18., 11.), (18., 18.)],
                green,
                green,
            );
            p.circle_filled(at(16., 6.), 2., gold);
        }
        Glyph::PowerPreset(n) => {
            if n == 0 {
                paint(p, r, Glyph::Power, enabled);
            } else if n <= 4 {
                line((11., 3.), (11., 21.), red, 1.);
                line((1., 13.), (21., 13.), red, 1.);
                p.text(
                    at(11., 7.),
                    egui::Align2::CENTER_CENTER,
                    ["", "Vcc", "+12", "+5", "-5"][n as usize],
                    egui::FontId::proportional(10.),
                    pale,
                );
            } else {
                line((11., 21.), (11., 10.), red, 1.);
                match n {
                    5 => poly(
                        &[(7., 11.), (11., 2.), (15., 11.)],
                        Color32::TRANSPARENT,
                        red,
                    ),
                    6 => {
                        let pts = (0..=20)
                            .map(|i| {
                                let x = i as f32;
                                at(1. + x, 8. + (x / 20. * std::f32::consts::TAU).sin() * 3.)
                            })
                            .collect();
                        p.add(Shape::line(pts, Stroke::new(1., red)));
                    }
                    7 => line((1., 9.), (21., 9.), red, 1.),
                    8 => {
                        p.circle_stroke(at(11., 6.), 4., Stroke::new(1., red));
                    }
                    9 => poly(
                        &[(3., 12.), (19., 12.), (11., 19.)],
                        Color32::TRANSPARENT,
                        red,
                    ),
                    _ => {
                        line((2., 14.), (20., 14.), red, 1.);
                        for x in [5., 11., 17.] {
                            line((x, 14.), (x - 3., 19.), red, 1.);
                        }
                    }
                }
            }
        }
        Glyph::Filter => {
            poly(
                &[
                    (2., 3.),
                    (20., 3.),
                    (14., 10.),
                    (14., 18.),
                    (9., 17.),
                    (9., 10.),
                ],
                ink(Color32::from_rgb(124, 177, 200)),
                ink(Color32::from_rgb(205, 229, 238)),
            );
            line((3., 4.), (18., 4.), pale, 1.);
        }
        Glyph::Move => {
            for (a, b) in [((11., 3.), (11., 19.)), ((3., 11.), (19., 11.))] {
                line(a, b, blue, 1.);
            }
            for (x, y, dx, dy) in [
                (11., 2., 0., 1.),
                (11., 20., 0., -1.),
                (2., 11., 1., 0.),
                (20., 11., -1., 0.),
            ] {
                line(
                    (x, y),
                    (x + dx * 3. - dy * 2., y + dy * 3. + dx * 2.),
                    blue,
                    1.,
                );
                line(
                    (x, y),
                    (x + dx * 3. + dy * 2., y + dy * 3. - dx * 2.),
                    blue,
                    1.,
                );
            }
        }
        Glyph::Select => {
            for i in (2..20).step_by(3) {
                let i = i as f32;
                line((i, 2.), ((i + 2.).min(20.), 2.), blue, 1.);
                line((i, 20.), ((i + 2.).min(20.), 20.), blue, 1.);
                line((2., i), (2., (i + 2.).min(20.)), blue, 1.);
                line((20., i), (20., (i + 2.).min(20.)), blue, 1.);
            }
        }
        Glyph::Align => {
            let c = ink(Color32::from_gray(164));
            line((3., 2.), (3., 21.), c, 1.);
            for (y, w) in [(4., 11.), (10., 15.), (16., 8.)] {
                poly(
                    &[(5., y), (w + 5., y), (w + 5., y + 3.), (5., y + 3.)],
                    ink(Color32::from_gray(96)),
                    c,
                );
            }
        }
        Glyph::Part => {
            poly(
                &[(6., 2.), (16., 2.), (16., 21.), (6., 21.)],
                ink(Color32::from_rgb(136, 120, 66)),
                gold,
            );
            poly(&[(8., 4.), (14., 4.), (14., 19.), (8., 19.)], gold, gold);
            for y in [5., 9., 13., 17.] {
                line((2., y), (6., y), gold, 1.);
                line((16., y), (20., y), gold, 1.);
            }
        }
        Glyph::Wire => {
            for (y, c) in [
                (5., pale),
                (11., blue),
                (17., ink(Color32::from_rgb(72, 115, 160))),
            ] {
                p.add(Shape::line(
                    vec![at(1., y), at(9., y), at(13., y + 4.), at(21., y + 4.)],
                    Stroke::new(2., c),
                ));
            }
        }
        Glyph::Power => {
            line((11., 1.), (11., 11.), red, 1.);
            for (y, width) in [(12., 21.), (15., 15.), (18., 8.)] {
                line((11. - width / 2., y), (11. + width / 2., y), red, 1.);
            }
        }
        Glyph::Harness => {
            poly(
                &[(2., 1.), (5., 1.), (6., 4.), (6., 20.), (2., 20.)],
                pale,
                pale,
            );
            line((6., 11.), (20., 11.), blue, 1.);
            for x in [10., 14., 18.] {
                line((x, 9.), (x - 1., 13.), blue, 1.);
            }
        }
        Glyph::Sheet => {
            poly(&[(2., 4.), (20., 4.), (20., 20.), (2., 20.)], green, blue);
            for y in [8., 15.] {
                poly(&[(0., y - 2.), (4., y), (0., y + 2.)], pale, pale);
                poly(&[(22., y - 2.), (18., y), (22., y + 2.)], pale, pale);
            }
            p.circle_filled(at(11., 12.), 1.1, pale);
        }
        Glyph::Port => {
            poly(
                &[(1., 6.), (17., 6.), (21., 11.), (17., 16.), (1., 16.)],
                gold,
                gold,
            );
            p.text(
                at(8., 11.),
                egui::Align2::CENTER_CENTER,
                "D1",
                egui::FontId::proportional(10.),
                ink(Color32::from_rgb(127, 105, 64)),
            );
        }
        Glyph::Directive => {
            p.circle_stroke(at(12., 10.), 8., Stroke::new(1.3, red));
            line((0., 10.), (4., 10.), red, 1.);
            p.text(
                at(12., 11.),
                egui::Align2::CENTER_CENTER,
                "i",
                egui::FontId::proportional(17.),
                red,
            );
        }
        Glyph::NoErc => {
            line((3., 3.), (19., 19.), red, 1.6);
            line((3., 19.), (19., 3.), red, 1.6);
        }
        Glyph::Text => {
            line((2., 21.), (10., 1.), blue, 1.3);
            line((10., 1.), (18., 21.), blue, 1.3);
            line((6., 14.), (15., 14.), blue, 1.);
            line((0., 21.), (7., 21.), blue, 1.);
            line((14., 21.), (21., 21.), blue, 1.);
            line((11., 6.), (16., 20.), pale, 1.);
        }
        Glyph::Arc => {
            let points = (0..=30)
                .map(|n| {
                    let a = (n as f32 / 30. * 290. - 55.).to_radians();
                    at(11. + 9. * a.cos(), 11. - 9. * a.sin())
                })
                .collect();
            p.add(Shape::line(points, Stroke::new(1.1, blue)));
        }
        Glyph::Polygon => {
            poly(
                &[(3., 9.), (9., 2.), (19., 8.), (17., 20.), (3., 18.)],
                pale,
                blue,
            );
        }
        Glyph::Probe(kind) => {
            poly(&[(5., 20.), (17., 5.), (20., 7.), (8., 21.)], pale, blue);
            line((3., 22.), (8., 17.), pale, 1.);
            p.text(
                at(1., 0.),
                egui::Align2::LEFT_TOP,
                ["V", "VD", "I", "P"][kind as usize],
                egui::FontId::proportional(if kind == 1 { 10. } else { 12. }),
                ink(Color32::WHITE),
            );
        }
        Glyph::Initial => {}
        Glyph::Source => {
            poly(
                &[
                    (4., 5.),
                    (16., 5.),
                    (21., 11.),
                    (16., 18.),
                    (4., 18.),
                    (0., 11.),
                ],
                gold,
                ink(Color32::from_rgb(211, 136, 44)),
            );
            line((0., 11.), (4., 5.), pale, 1.);
            line((4., 5.), (16., 5.), pale, 1.);
        }
    }
}
