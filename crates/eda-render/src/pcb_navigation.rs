//! PCB viewport input. Camera changes never edit the board or its history.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorStyle {
    #[default]
    Small90,
    Small45,
    Large90,
}
impl CursorStyle {
    pub fn label(self) -> &'static str {
        match self {
            Self::Small90 => "Small 90",
            Self::Small45 => "Small 45",
            Self::Large90 => "Large 90",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NavigationOptions {
    pub wheel_zoom: bool,
    pub cursor: CursorStyle,
}
#[derive(Clone, Copy)]
pub enum FitTarget {
    Document,
    Board,
    Selection,
}

fn zoom_at(camera: &mut Camera, point: Pos2, rect: Rect, factor: f64) {
    if !factor.is_finite() || factor <= 0.0 {
        return;
    }
    let before = camera.world_mm(point, rect);
    camera.pixels_per_mm = (camera.pixels_per_mm * factor).clamp(0.001, 100_000.0);
    let after = camera.world_mm(point, rect);
    camera.centre_mm[0] += before[0] - after[0];
    camera.centre_mm[1] += before[1] - after[1];
}
fn fit_target(camera: &mut Camera, board: &Board, view: &BoardView, rect: Rect, target: FitTarget) {
    let bounds = match target {
        FitTarget::Document => Some(board.bounds()),
        FitTarget::Board => {
            let points = board.outline_points(256);
            Some(if points.len() >= 3 {
                Bounds::points(&points)
            } else {
                board.bounds()
            })
        }
        FitTarget::Selection => board.selected_bounds(&view.selected),
    };
    if let Some(bounds) = bounds.filter(|b| !b.empty) {
        fit_bounds(camera, bounds, rect);
    }
}
pub(super) fn navigate(
    ui: &egui::Ui,
    r: &Response,
    camera: &mut Camera,
    b: &Board,
    view: &mut BoardView,
    rect: Rect,
) -> bool {
    if !camera.fitted {
        fit(camera, b, rect);
    }
    if let Some(target) = view.requested_fit.take() {
        fit_target(camera, b, view, rect, target);
    }
    if !ui.input(|i| i.focused) {
        return false;
    }
    let panning =
        r.dragged_by(egui::PointerButton::Middle) || r.dragged_by(egui::PointerButton::Secondary);
    if panning {
        let d = ui.input(|i| i.pointer.delta());
        camera.centre_mm[0] -=
            d.x as f64 / camera.pixels_per_mm * if camera.mirrored_x { -1.0 } else { 1.0 };
        camera.centre_mm[1] += d.y as f64 / camera.pixels_per_mm;
    }
    if let Some(p) = r.hover_pos() {
        // egui routes Ctrl+wheel into zoom_delta, not smooth_scroll_delta.
        // Use this also for pinch and for the residual smoothed zoom frames
        // after Ctrl has been released. Do not apply wheel zoom a second time.
        let (scroll, zoom, modifiers) =
            ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers));
        if zoom != 1.0 {
            zoom_at(camera, p, rect, zoom as f64);
        }
        if view.navigation.wheel_zoom && !modifiers.shift {
            if scroll.y != 0.0 {
                zoom_at(camera, p, rect, (scroll.y as f64 * 0.003).exp());
            }
            camera.centre_mm[0] -= scroll.x as f64 / camera.pixels_per_mm;
        } else if modifiers.shift {
            camera.centre_mm[0] -= (scroll.x + scroll.y) as f64 / camera.pixels_per_mm;
        } else {
            camera.centre_mm[0] -= scroll.x as f64 / camera.pixels_per_mm;
            camera.centre_mm[1] += scroll.y as f64 / camera.pixels_per_mm;
        }
        if !ui.ctx().wants_keyboard_input() {
            ui.input_mut(|i| {
                let ctrl = egui::Modifiers::CTRL;
                let none = egui::Modifiers::NONE;
                if i.consume_key(ctrl, egui::Key::PageDown) {
                    fit_target(camera, b, view, rect, FitTarget::Document);
                } else if i.consume_key(ctrl, egui::Key::Home) {
                    fit_target(camera, b, view, rect, FitTarget::Selection);
                } else if i.consume_key(none, egui::Key::PageUp) {
                    zoom_at(camera, p, rect, 1.5);
                } else if i.consume_key(none, egui::Key::PageDown) {
                    zoom_at(camera, p, rect, 1.0 / 1.5);
                } else if i.consume_key(none, egui::Key::Home) {
                    camera.centre_mm = camera.world_mm(p, rect);
                }
            });
        }
    }
    panning
}
fn cursor_lines(style: CursorStyle, centre: Pos2, rect: Rect) -> [[Pos2; 2]; 2] {
    let d = 7.0;
    match style {
        CursorStyle::Small90 => [
            [centre - Vec2::new(d, 0.0), centre + Vec2::new(d, 0.0)],
            [centre - Vec2::new(0.0, d), centre + Vec2::new(0.0, d)],
        ],
        CursorStyle::Small45 => [
            [centre - Vec2::splat(d * 0.7), centre + Vec2::splat(d * 0.7)],
            [
                centre + Vec2::new(-d * 0.7, d * 0.7),
                centre + Vec2::new(d * 0.7, -d * 0.7),
            ],
        ],
        CursorStyle::Large90 => [
            [
                Pos2::new(rect.left(), centre.y),
                Pos2::new(rect.right(), centre.y),
            ],
            [
                Pos2::new(centre.x, rect.top()),
                Pos2::new(centre.x, rect.bottom()),
            ],
        ],
    }
}
pub(super) fn paint_cursor(
    ui: &egui::Ui,
    painter: &egui::Painter,
    r: &Response,
    point: Option<Point>,
    view: &View,
    rect: Rect,
    panning: bool,
) {
    if panning {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if r.hovered()
        && ui.input(|i| i.focused)
        && let Some(p) = point
    {
        let at = view.camera.screen(p, rect);
        if !rect.contains(at) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            return;
        }
        for line in cursor_lines(view.pcb.navigation.cursor, at, rect) {
            painter.line_segment(line, Stroke::new(1.0, Color32::WHITE));
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, MouseWheelUnit, PointerButton};
    struct Harness {
        ctx: egui::Context,
        board: Board,
        view: View,
        pointer: Pos2,
    }
    impl Harness {
        fn new() -> Self {
            let mut view = View::default();
            view.camera.fitted = true;
            view.camera.pixels_per_mm = 100.0;
            view.camera.centre_mm = [0.0, 0.0];
            let mut h = Self {
                ctx: egui::Context::default(),
                board: Board::default(),
                view,
                pointer: Pos2::new(250.0, 120.0),
            };
            h.frame(vec![], Modifiers::NONE);
            h
        }
        fn rect() -> Rect {
            Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))
        }
        fn frame(&mut self, mut events: Vec<Event>, modifiers: Modifiers) -> egui::FullOutput {
            events.insert(0, Event::PointerMoved(self.pointer));
            self.ctx.run(
                egui::RawInput {
                    screen_rect: Some(Self::rect()),
                    focused: true,
                    events,
                    modifiers,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(ctx, |ui| {
                            canvas(ui, &self.board, &mut self.view);
                        });
                },
            )
        }
        fn wheel(&mut self, mods: Modifiers) {
            self.frame(
                vec![Event::MouseWheel {
                    unit: MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, 1.0),
                    modifiers: mods,
                }],
                mods,
            );
        }
        fn key(&mut self, key: Key, modifiers: Modifiers) {
            self.frame(
                vec![
                    Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    },
                    Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    },
                ],
                modifiers,
            );
        }
    }
    fn close(a: [f64; 2], b: [f64; 2]) {
        assert!(
            (a[0] - b[0]).abs() < 1e-8 && (a[1] - b[1]).abs() < 1e-8,
            "{a:?} != {b:?}"
        );
    }
    #[test]
    fn ctrl_wheel_and_pinch_zoom_at_pointer_including_smoothed_tail() {
        let mut h = Harness::new();
        let before = h.view.camera.world_mm(h.pointer, Harness::rect());
        h.wheel(Modifiers::CTRL);
        assert!(
            h.view.camera.pixels_per_mm > 100.0,
            "Ctrl wheel was swallowed"
        );
        close(before, h.view.camera.world_mm(h.pointer, Harness::rect()));
        for _ in 0..40 {
            h.frame(vec![], Modifiers::NONE);
        }
        close(before, h.view.camera.world_mm(h.pointer, Harness::rect()));
        let scale = h.view.camera.pixels_per_mm;
        h.frame(vec![Event::Zoom(1.25)], Modifiers::NONE);
        assert!((h.view.camera.pixels_per_mm / scale - 1.25).abs() < 1e-5);
        close(before, h.view.camera.world_mm(h.pointer, Harness::rect()));
    }
    #[test]
    fn wheel_modes_pan_on_correct_axes_and_ignore_other_panels() {
        let mut h = Harness::new();
        h.wheel(Modifiers::NONE);
        assert_eq!(h.view.camera.pixels_per_mm, 100.0);
        assert_eq!(h.view.camera.centre_mm[0], 0.0);
        assert!(h.view.camera.centre_mm[1] > 0.0);
        let mut h = Harness::new();
        h.wheel(Modifiers::SHIFT);
        assert_eq!(h.view.camera.pixels_per_mm, 100.0);
        assert!(h.view.camera.centre_mm[0] < 0.0);
        assert_eq!(h.view.camera.centre_mm[1], 0.0);
        let mut h = Harness::new();
        h.view.pcb.navigation.wheel_zoom = true;
        h.wheel(Modifiers::NONE);
        assert!(h.view.camera.pixels_per_mm > 100.0);
        let mut h = Harness::new();
        h.pointer = Pos2::new(450.0, 150.0);
        h.wheel(Modifiers::CTRL);
        assert_eq!(h.view.camera.pixels_per_mm, 100.0);
        assert_eq!(h.view.camera.centre_mm, [0.0, 0.0]);
    }
    #[test]
    fn page_zoom_home_and_fit_targets_have_distinct_effects() {
        let mut h = Harness::new();
        let anchor = h.view.camera.world_mm(h.pointer, Harness::rect());
        h.key(Key::PageUp, Modifiers::NONE);
        assert_eq!(h.view.camera.pixels_per_mm, 150.0);
        close(anchor, h.view.camera.world_mm(h.pointer, Harness::rect()));
        h.key(Key::PageDown, Modifiers::NONE);
        assert_eq!(h.view.camera.pixels_per_mm, 100.0);
        h.key(Key::Home, Modifiers::NONE);
        close(h.view.camera.centre_mm, anchor);
        h.board.outline = eda_core::pcb_polygon::straight_contour(&[
            Point::mm(0, 0),
            Point::mm(10, 0),
            Point::mm(10, 10),
            Point::mm(0, 10),
        ]);
        let item = Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(100, 100),
                end: Point::mm(101, 100),
                width: 100_000,
            },
        );
        h.view.pcb.selected = vec![item.id];
        h.board.items.push(item);
        h.key(Key::PageDown, Modifiers::CTRL);
        let all = h.view.camera.pixels_per_mm;
        h.view.pcb.requested_fit = Some(FitTarget::Board);
        h.frame(vec![], Modifiers::NONE);
        assert!(h.view.camera.pixels_per_mm > all * 5.0);
        close(h.view.camera.centre_mm, [5.0, 5.0]);
        h.key(Key::Home, Modifiers::CTRL);
        close(h.view.camera.centre_mm, [100.5, 100.0]);
        assert!(h.view.camera.pixels_per_mm > 100.0);
    }
    #[test]
    fn right_and_middle_drags_pan_without_emitting_edit_or_cancel_clicks() {
        for button in [PointerButton::Secondary, PointerButton::Middle] {
            let mut h = Harness::new();
            let before = h.board.clone();
            let event = |pos, pressed| Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Modifiers::NONE,
            };
            h.frame(vec![event(h.pointer, true)], Modifiers::NONE);
            h.pointer += Vec2::new(40.0, 25.0);
            let output = h.frame(vec![], Modifiers::NONE);
            assert_eq!(
                output.platform_output.cursor_icon,
                egui::CursorIcon::Grabbing
            );
            let input = h.view.pcb.input.as_ref().unwrap();
            assert!(input.panning);
            assert!(!input.dragging && !input.clicked);
            h.frame(vec![event(h.pointer, false)], Modifiers::NONE);
            assert!(!h.view.pcb.input.as_ref().unwrap().secondary_clicked);
            close(h.view.camera.centre_mm, [-0.4, 0.25]);
            assert_eq!(h.board, before);
        }
    }
    #[test]
    fn default_cursor_is_small_and_large_cross_requires_explicit_choice() {
        let style = NavigationOptions::default().cursor;
        assert_eq!(style, CursorStyle::Small90);
        for style in [style, CursorStyle::Small45] {
            for [a, b] in cursor_lines(style, Pos2::new(200.0, 150.0), Harness::rect()) {
                assert!(a.distance(b) <= 15.0);
            }
        }
        let lines = cursor_lines(
            CursorStyle::Large90,
            Pos2::new(200.0, 150.0),
            Harness::rect(),
        );
        assert_eq!(lines[0][0].distance(lines[0][1]), 400.0);
        let mut h = Harness::new();
        let output = h.frame(vec![], Modifiers::NONE);
        assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::None);
        h.pointer = Pos2::new(-10.0, -10.0);
        let output = h.frame(vec![], Modifiers::NONE);
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Default
        );
    }
    #[test]
    fn zoom_limits_stay_finite_and_keep_pointer_anchor() {
        let mut h = Harness::new();
        for scale in [0.001, 100_000.0] {
            h.view.camera.pixels_per_mm = scale;
            let before = h.view.camera.world_mm(h.pointer, Harness::rect());
            h.frame(
                vec![Event::Zoom(if scale < 1.0 { 0.0001 } else { 10000.0 })],
                Modifiers::NONE,
            );
            assert_eq!(h.view.camera.pixels_per_mm, scale);
            close(h.view.camera.world_mm(h.pointer, Harness::rect()), before);
        }
    }
    #[test]
    fn primary_press_origin_survives_fast_drag_release() {
        let mut h = Harness::new();
        let origin = world(&h.view.camera, h.pointer, Harness::rect());
        h.frame(
            vec![Event::PointerButton {
                pos: h.pointer,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
        );
        h.pointer += Vec2::new(60.0, 0.0);
        h.frame(
            vec![Event::PointerButton {
                pos: h.pointer,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
        );
        let input = h.view.pcb.input.as_ref().unwrap();
        assert_eq!(input.press_origin, Some(origin));
        assert!(input.drag_stopped, "release was lost");
    }
    #[test]
    fn cancellation_before_canvas_rejects_same_frame_primary_events() {
        let mut h = Harness::new();
        let event = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        h.view.pcb.clear_pointer();
        let start = h.pointer;
        h.pointer += Vec2::new(60.0, 0.0);
        h.frame(
            vec![event(start, true), event(h.pointer, false)],
            Modifiers::NONE,
        );
        let input = h.view.pcb.input.as_ref().unwrap();
        assert!(!input.clicked && !input.drag_started && !input.drag_stopped);
        assert!(input.press_origin.is_none());
        h.frame(vec![event(h.pointer, true)], Modifiers::NONE);
        h.pointer += Vec2::new(60.0, 0.0);
        h.frame(vec![event(h.pointer, false)], Modifiers::NONE);
        assert!(h.view.pcb.input.as_ref().unwrap().drag_stopped);
    }
}
