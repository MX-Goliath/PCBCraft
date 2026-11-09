use super::*;
use egui::{Event, Id, Pos2, Rect, vec2};

fn input(events: Vec<Event>) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440., 900.))),
        events,
        ..Default::default()
    }
}

fn window(ctx: &Context, wide: bool, events: Vec<Event>) -> Rect {
    let mut rect = Rect::NOTHING;
    let _ = ctx.run(input(events), |ctx| {
        rect = Window::new("Changing status")
            .id(Id::new("stable_status_window"))
            .default_size([360., 100.])
            .fixed_pos([60., 60.])
            .resizable([true, false])
            .fade_in(false)
            .show_stable(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(if wide {
                        "A long imported path, changing status or error message ".repeat(30)
                    } else {
                        "Reading".into()
                    });
                });
                if wide {
                    ui.allocate_space(vec2(2000., 24.));
                }
                let _ = ui.button("Cancel");
            })
            .unwrap()
            .response
            .rect;
    });
    rect
}

#[test]
fn window_width_survives_status_changes_wide_content_and_user_resize() {
    for dpi in [1., 1.5, 2.] {
        let ctx = Context::default();
        ctx.set_pixels_per_point(dpi);
        let mut before = Rect::NOTHING;
        for _ in 0..3 {
            before = window(&ctx, false, vec![]);
        }
        for wide in [true, false, true, false] {
            for _ in 0..3 {
                let rect = window(&ctx, wide, vec![]);
                assert!((rect.width() - before.width()).abs() < 1.);
            }
        }
        let from = before.right_center() - vec2(1., 0.);
        let to = from + vec2(100., 0.);
        window(&ctx, false, vec![Event::PointerMoved(from)]);
        for events in [
            vec![Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
            vec![Event::PointerMoved(to)],
            vec![Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            window(&ctx, false, events);
        }
        let resized = window(&ctx, false, vec![]);
        assert!(resized.width() > before.width() + 70., "Resize was lost");
        for wide in [true, false, true, false] {
            let rect = window(&ctx, wide, vec![]);
            assert!((rect.width() - resized.width()).abs() < 1.);
        }
    }
}

#[test]
fn modal_keeps_its_declared_width_and_cancel_remains_interactive() {
    for dpi in [1., 1.5, 2.] {
        let ctx = Context::default();
        ctx.set_pixels_per_point(dpi);
        let mut before: Option<f32> = None;
        let mut cancel = Rect::NOTHING;
        let mut clicked = false;
        let mut render = |wide, events| {
            let _ = ctx.run(input(events), |ctx| {
                let response = Modal::new(Id::new("stable_modal")).show_stable(ctx, 480., |ui| {
                    ui.horizontal(|ui| {
                        ui.label(if wide {
                            "Value".repeat(300)
                        } else {
                            "Value".into()
                        });
                    });
                    if wide {
                        ui.allocate_space(vec2(2400., 22.));
                    }
                    let response = ui.button("Cancel");
                    cancel = response.rect;
                    clicked |= response.clicked();
                });
                if let Some(before) = before {
                    assert!((response.response.rect.width() - before).abs() < 1.);
                } else {
                    before = Some(response.response.rect.width());
                }
            });
            cancel.center()
        };
        for wide in [false, false, true, false, true, false, false] {
            render(wide, vec![]);
        }
        let at = render(false, vec![]);
        render(
            false,
            vec![
                Event::PointerMoved(at),
                Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        render(
            false,
            vec![Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(clicked);
    }
}
