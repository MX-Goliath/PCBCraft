use super::*;
use egui::{Event, Modifiers, PointerButton};

struct Harness {
    ctx: egui::Context,
    doc: Document,
    view: View,
}
impl Harness {
    fn new() -> Self {
        let mut doc = Document::new(eda_core::DocumentKind::Schematic, "Selection");
        let s = doc.schematic.as_mut().unwrap();
        s.sheet.grid_visible = false;
        s.sheet.border = false;
        s.sheet.title_block = false;
        for x in [15, 35] {
            s.primitives.push(eda_core::schematic_edit::primitive(
                K::Rectangle {
                    a: Point::mm(x, 15),
                    b: Point::mm(x + 10, 25),
                    radius: Point::default(),
                },
                None,
            ));
        }
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.centre_mm = [30., 20.];
        view.camera.pixels_per_mm = 10.;
        let mut h = Self {
            ctx: egui::Context::default(),
            doc,
            view,
        };
        h.frame(vec![], Modifiers::NONE, true);
        h.frame(vec![], Modifiers::NONE, true);
        h
    }
    fn frame(&mut self, events: Vec<Event>, modifiers: Modifiers, focused: bool) {
        let _ = self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600., 400.))),
                events,
                modifiers,
                focused,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        crate::canvas(ui, &self.doc, &mut self.view);
                    });
            },
        );
    }
    fn drag(&mut self, a: Pos2, b: Pos2, mods: Modifiers, fast: bool) {
        let press = Event::PointerButton {
            pos: a,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: mods,
        };
        let release = Event::PointerButton {
            pos: b,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        };
        if fast {
            self.frame(
                vec![
                    Event::PointerMoved(a),
                    press,
                    Event::PointerMoved(b),
                    release,
                ],
                Modifiers::NONE,
                true,
            );
        } else {
            self.frame(vec![Event::PointerMoved(a), press], mods, true);
            self.frame(vec![Event::PointerMoved(b)], mods, true);
            self.frame(vec![release], Modifiers::NONE, true);
        }
    }
}

#[test]
fn rectangle_selection_is_directional_additive_and_survives_a_fast_release() {
    for fast in [false, true] {
        let mut h = Harness::new();
        let original = h.doc.clone();
        let ids: Vec<_> = h
            .doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .map(|p| p.id)
            .collect();
        h.drag(
            Pos2::new(200., 130.),
            Pos2::new(280., 160.),
            Modifiers::NONE,
            fast,
        );
        assert!(h.view.sch_selection.is_empty());
        h.drag(
            Pos2::new(280., 130.),
            Pos2::new(200., 160.),
            Modifiers::NONE,
            fast,
        );
        assert_eq!(h.view.sch_selection, vec![ids[0]]);
        h.drag(
            Pos2::new(340., 140.),
            Pos2::new(460., 260.),
            Modifiers::SHIFT,
            fast,
        );
        assert!(ids.iter().all(|id| h.view.sch_selection.contains(id)));
        assert!(h.view.sch_selection_box.is_none());
        assert_eq!(h.doc, original);
    }
}

#[test]
fn focus_loss_and_filter_disable_never_complete_a_stale_selection_rectangle() {
    let mut h = Harness::new();
    h.view.sch_filters[10] = false;
    h.drag(
        Pos2::new(140., 140.),
        Pos2::new(260., 260.),
        Modifiers::NONE,
        true,
    );
    assert!(h.view.sch_selection.is_empty());
    h.view.sch_filters[10] = true;
    h.frame(
        vec![
            Event::PointerMoved(Pos2::new(140., 140.)),
            Event::PointerButton {
                pos: Pos2::new(140., 140.),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ],
        Modifiers::NONE,
        true,
    );
    h.frame(
        vec![Event::PointerMoved(Pos2::new(260., 260.))],
        Modifiers::NONE,
        true,
    );
    assert!(h.view.sch_selection_box.is_some());
    h.frame(vec![], Modifiers::NONE, false);
    h.frame(
        vec![Event::PointerButton {
            pos: Pos2::new(260., 260.),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }],
        Modifiers::NONE,
        true,
    );
    assert!(h.view.sch_selection.is_empty());
    assert!(h.view.sch_selection_box.is_none());
}

#[test]
fn schematic_plain_wheel_scrolls_shift_wheel_pans_and_ctrl_wheel_anchors_zoom() {
    for modifiers in [Modifiers::NONE, Modifiers::SHIFT, Modifiers::CTRL] {
        let mut h = Harness::new();
        let at = Pos2::new(210., 170.);
        let before = h.view.camera.clone();
        let world = before.world_mm(at, Rect::from_min_size(Pos2::ZERO, Vec2::new(600., 400.)));
        h.frame(
            vec![
                Event::PointerMoved(at),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: Vec2::new(0., 1.),
                    modifiers,
                },
            ],
            modifiers,
            true,
        );
        if modifiers.ctrl {
            assert!(h.view.camera.pixels_per_mm > before.pixels_per_mm);
            let after = h
                .view
                .camera
                .world_mm(at, Rect::from_min_size(Pos2::ZERO, Vec2::new(600., 400.)));
            assert!((after[0] - world[0]).abs() < 1e-8 && (after[1] - world[1]).abs() < 1e-8);
        } else {
            assert_eq!(h.view.camera.pixels_per_mm, before.pixels_per_mm);
            if modifiers.shift {
                assert_ne!(h.view.camera.centre_mm[0], before.centre_mm[0]);
                assert_eq!(h.view.camera.centre_mm[1], before.centre_mm[1]);
            } else {
                assert_ne!(h.view.camera.centre_mm[1], before.centre_mm[1]);
                assert_eq!(h.view.camera.centre_mm[0], before.centre_mm[0]);
            }
        }
    }
}
