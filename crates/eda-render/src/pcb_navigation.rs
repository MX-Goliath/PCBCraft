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

use crate::interaction::zoom_at;
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
pub(super) fn requested_layer(ui: &egui::Ui, r: &Response, view: &BoardView) -> Option<u16> {
    if !ui.is_enabled() || !r.contains_pointer() {
        return None;
    }
    let step: isize = ui.input(|i| {
        if !i.focused {
            return 0;
        }
        i.events
            .iter()
            .filter_map(|event| match event {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed: true,
                    ..
                } if r.rect.contains(*pos) => match button {
                    egui::PointerButton::Extra1 => Some(-1), // Back
                    egui::PointerButton::Extra2 => Some(1),  // Forward
                    _ => None,
                },
                egui::Event::MouseWheel {
                    delta, modifiers, ..
                } if modifiers.ctrl && modifiers.shift && !modifiers.alt => {
                    (delta.y != 0.).then_some(if delta.y > 0. { 1 } else { -1 })
                }
                _ => None,
            })
            .sum()
    });
    if step == 0 {
        return None;
    }
    let layers = view.layer_tabs();
    if layers.is_empty() {
        return None;
    }
    let index = layers
        .iter()
        .position(|(id, _)| *id == view.active_layer)
        .map_or(if step > 0 { -1 } else { 0 }, |n| n as isize);
    let layer = layers[(index + step).rem_euclid(layers.len() as isize) as usize].0;
    (layer != view.active_layer).then_some(layer)
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
    let panning = crate::interaction::navigate(ui, r, camera, rect, view.navigation.wheel_zoom);
    if let Some(p) = r.hover_pos()
        && !ui.ctx().wants_keyboard_input()
    {
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
    pointer: Option<Pos2>,
    view: &View,
    rect: Rect,
    panning: bool,
) {
    if panning {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    } else if r.hovered()
        && ui.input(|i| i.focused)
        && let Some(at) = pointer
    {
        // The visual cursor follows the mouse; tool coordinates snap independently.
        if !rect.contains(at) || view.pcb.navigation.cursor == CursorStyle::Small90 {
            // The default crosshair must move with the native pointer even
            // while a board frame is expensive or waiting for presentation.
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
        focused: bool,
        modal: bool,
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
                focused: true,
                modal: false,
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
                    focused: self.focused,
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
                    if self.modal {
                        egui::Modal::new(egui::Id::new("navigation_test_modal")).show(ctx, |ui| {
                            ui.label("Modal dialog");
                        });
                    }
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
    fn layer_harness() -> Harness {
        let mut h = Harness::new();
        h.board.layers = [
            (1, "Top", true, 0),
            (2, "Hidden inner", true, 1),
            (32, "Bottom", true, 2),
            (35, "Paste", false, 3),
            (57, "Mechanical", false, 4),
            (75, "Connections", false, 5),
        ]
        .into_iter()
        .map(|(id, name, copper, order)| PcbLayer {
            id,
            name: name.into(),
            copper,
            order,
        })
        .collect();
        h.view.pcb = BoardView::default();
        h.frame(vec![], Modifiers::NONE);
        h.view.pcb.visible = [1, 32, 35, 57, 75].into();
        h
    }
    #[test]
    fn empty_polygon_definition_does_not_intercept_the_canvas_hit() {
        let mut h = layer_harness();
        h.board.polygon_pours.push(eda_core::pcb_polygon::Polygon {
            contour: eda_core::pcb_polygon::straight_contour(&[
                Point::mm(-10, -10),
                Point::mm(10, -10),
                Point::mm(10, 10),
                Point::mm(-10, 10),
            ]),
            ..Default::default()
        });
        h.frame(vec![], Modifiers::NONE);
        assert!(h.view.pcb.input.as_ref().unwrap().hit.is_none());
    }
    #[test]
    fn ctrl_shift_wheel_changes_layer_without_camera_motion_or_a_later_zoom_tail() {
        let mut h = layer_harness();
        let before = h.view.camera.clone();
        let mods = Modifiers {
            ctrl: true,
            shift: true,
            ..Default::default()
        };
        h.wheel(mods);
        assert_eq!(h.view.pcb.input.as_ref().unwrap().requested_layer, Some(32));
        for _ in 0..20 {
            h.frame(vec![], Modifiers::NONE);
        }
        assert_eq!(h.view.camera.pixels_per_mm, before.pixels_per_mm);
        close(h.view.camera.centre_mm, before.centre_mm);
        h.view.pcb.active_layer = 32;
        h.frame(
            vec![Event::MouseWheel {
                unit: MouseWheelUnit::Line,
                delta: Vec2::new(0., -1.),
                modifiers: mods,
            }],
            mods,
        );
        assert_eq!(h.view.pcb.input.as_ref().unwrap().requested_layer, Some(1));
    }
    fn side_button(pos: Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    #[test]
    fn side_buttons_request_visible_layers_in_strip_order_with_wraparound() {
        let mut h = layer_harness();
        let board = h.board.clone();
        let visible = h.view.pcb.visible.clone();
        for (active, button, expected) in [
            (1, PointerButton::Extra2, 32),
            (32, PointerButton::Extra2, 57),
            (57, PointerButton::Extra2, 35),
            (35, PointerButton::Extra2, 1),
            (1, PointerButton::Extra1, 35),
            (35, PointerButton::Extra1, 57),
            (57, PointerButton::Extra1, 32),
            (32, PointerButton::Extra1, 1),
        ] {
            h.view.pcb.active_layer = active;
            h.frame(
                vec![
                    side_button(h.pointer, button, true),
                    side_button(h.pointer, button, false),
                ],
                Modifiers::NONE,
            );
            let input = h.view.pcb.input.as_ref().unwrap();
            assert_eq!(input.requested_layer, Some(expected));
            assert!(!input.clicked && !input.secondary_clicked && !input.drag_started);
            assert_eq!(
                h.view.pcb.active_layer, active,
                "Editor must apply the request"
            );
            assert_eq!(h.view.pcb.visible, visible);
            assert_eq!(h.board, board);
        }
    }
    #[test]
    fn layer_cycle_reads_live_visibility_and_handles_hidden_active_or_empty_strip() {
        let mut h = layer_harness();
        for (visible, active, button, expected) in [
            (vec![1, 57, 35, 75], 1, PointerButton::Extra2, Some(57)),
            (vec![32, 35], 1, PointerButton::Extra2, Some(32)),
            (vec![32, 35], 1, PointerButton::Extra1, Some(35)),
            (vec![32], 32, PointerButton::Extra2, None),
            (vec![32], 1, PointerButton::Extra2, Some(32)),
            (vec![75], 1, PointerButton::Extra2, None),
            (vec![], 1, PointerButton::Extra1, None),
        ] {
            h.view.pcb.visible = visible.into_iter().collect();
            h.view.pcb.active_layer = active;
            h.frame(vec![side_button(h.pointer, button, true)], Modifiers::NONE);
            assert_eq!(h.view.pcb.input.as_ref().unwrap().requested_layer, expected);
        }
    }
    #[test]
    fn layer_cycle_uses_presses_once_and_keeps_multiple_presses_in_one_frame() {
        let mut h = layer_harness();
        h.frame(
            vec![side_button(h.pointer, PointerButton::Extra2, true)],
            Modifiers::NONE,
        );
        assert_eq!(h.view.pcb.input.as_ref().unwrap().requested_layer, Some(32));
        h.frame(vec![], Modifiers::NONE);
        assert!(h.view.pcb.input.as_ref().unwrap().requested_layer.is_none());
        h.frame(
            vec![side_button(h.pointer, PointerButton::Extra2, false)],
            Modifiers::NONE,
        );
        assert!(h.view.pcb.input.as_ref().unwrap().requested_layer.is_none());
        h.frame(
            vec![
                side_button(h.pointer, PointerButton::Extra2, true),
                side_button(h.pointer, PointerButton::Extra2, false),
                side_button(h.pointer, PointerButton::Extra2, true),
            ],
            Modifiers::NONE,
        );
        assert_eq!(h.view.pcb.input.as_ref().unwrap().requested_layer, Some(57));
    }
    #[test]
    fn side_buttons_ignore_outside_canvas_unfocused_window_and_modal_dialogs() {
        let mut h = layer_harness();
        h.pointer = Pos2::new(-10.0, -10.0);
        h.frame(
            vec![side_button(h.pointer, PointerButton::Extra2, true)],
            Modifiers::NONE,
        );
        assert!(h.view.pcb.input.as_ref().unwrap().requested_layer.is_none());
        h.pointer = Pos2::new(250.0, 120.0);
        h.focused = false;
        h.frame(
            vec![side_button(h.pointer, PointerButton::Extra2, true)],
            Modifiers::NONE,
        );
        assert!(h.view.pcb.input.as_ref().unwrap().requested_layer.is_none());
        h.focused = true;
        h.modal = true;
        h.frame(vec![], Modifiers::NONE);
        h.frame(
            vec![side_button(h.pointer, PointerButton::Extra2, true)],
            Modifiers::NONE,
        );
        assert!(h.view.pcb.input.as_ref().unwrap().requested_layer.is_none());
    }
    #[test]
    fn native_cursor_does_not_round_or_add_a_delayed_cross_inside_one_grid_cell() {
        let mut h = Harness::new();
        h.view.grid = true;
        h.view.grid_nm = 1_000_000;
        for mirrored in [false, true] {
            h.view.camera.mirrored_x = mirrored;
            for pointer in [Pos2::new(251.0, 121.0), Pos2::new(253.0, 123.0)] {
                h.pointer = pointer;
                let output = h.frame(vec![], Modifiers::NONE);
                assert_eq!(
                    output.platform_output.cursor_icon,
                    egui::CursorIcon::Crosshair
                );
                for expected in cursor_lines(CursorStyle::Small90, pointer, Harness::rect()) {
                    assert!(
                        !output.shapes.iter().any(|s| matches!(&s.shape,
                        egui::Shape::LineSegment { points, stroke }
                        if *points == expected && stroke.color == Color32::WHITE)),
                        "Native cursor must have no frame-rendered duplicate"
                    );
                }
                assert!(h.view.grid);
                assert_eq!(h.view.grid_nm, 1_000_000);
                let raw = world(&h.view.camera, pointer, Harness::rect());
                let input = h.view.pcb.input.as_ref().unwrap();
                assert_eq!(input.raw_point, Some(raw));
                assert_eq!(
                    input.point,
                    Some(Point {
                        x: eda_core::pcb::snap_coord(raw.x, h.view.grid_nm),
                        y: eda_core::pcb::snap_coord(raw.y, h.view.grid_nm),
                    })
                );
            }
        }
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
        h.view.pcb.three_d.revision += 1;
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
    fn right_drag_pans_and_middle_drag_zooms_without_emitting_edit_or_cancel_clicks() {
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
            if button == PointerButton::Secondary {
                close(h.view.camera.centre_mm, [-0.4, 0.25]);
            } else {
                assert!(h.view.camera.pixels_per_mm < 100.);
                close(
                    h.view
                        .camera
                        .world_mm(Pos2::new(250., 120.), Harness::rect()),
                    [0.5, 0.3],
                );
            }
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
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Crosshair
        );
        h.pointer = Pos2::new(-10.0, -10.0);
        let output = h.frame(vec![], Modifiers::NONE);
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Default
        );
    }
    #[test]
    fn explicitly_selected_software_cursors_keep_their_geometry() {
        let mut h = Harness::new();
        for style in [CursorStyle::Small45, CursorStyle::Large90] {
            h.view.pcb.navigation.cursor = style;
            for pointer in [Pos2::new(251.0, 121.0), Pos2::new(253.0, 123.0)] {
                h.pointer = pointer;
                let output = h.frame(vec![], Modifiers::NONE);
                assert_eq!(output.platform_output.cursor_icon, egui::CursorIcon::None);
                for expected in cursor_lines(style, pointer, Harness::rect()) {
                    assert!(output.shapes.iter().any(|s| matches!(&s.shape,
                        egui::Shape::LineSegment { points, stroke }
                        if *points == expected && stroke.color == Color32::WHITE)));
                }
            }
        }
    }
    #[test]
    fn native_cursor_is_released_when_canvas_loses_focus_or_a_modal_opens() {
        let mut h = Harness::new();
        h.view.pcb.tool_active = true;
        let output = h.frame(vec![], Modifiers::NONE);
        assert_eq!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Crosshair
        );
        h.focused = false;
        let output = h.frame(vec![], Modifiers::NONE);
        assert_ne!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Crosshair
        );
        h.focused = true;
        h.modal = true;
        h.frame(vec![], Modifiers::NONE);
        let output = h.frame(vec![], Modifiers::NONE);
        assert_ne!(
            output.platform_output.cursor_icon,
            egui::CursorIcon::Crosshair
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
