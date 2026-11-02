//! Shared SCH/PCB viewport gestures. Only the camera and view state change here.
use crate::Camera;
use egui::{PointerButton as Button, Rect, Response, Ui};

pub(crate) fn navigate(
    ui: &Ui,
    response: &Response,
    camera: &mut Camera,
    rect: Rect,
    wheel_zoom: bool,
) -> bool {
    if !ui.is_enabled() || !ui.input(|i| i.focused) {
        return false;
    }
    let modifiers = ui.input(|i| i.modifiers);
    let right = response.dragged_by(Button::Secondary);
    let middle = response.dragged_by(Button::Middle);
    let zoom_drag = middle || right && ui.input(|i| i.modifiers.ctrl || i.pointer.primary_down());
    if right || middle {
        let delta = ui.input(|i| i.pointer.delta());
        if zoom_drag {
            let anchor = ui
                .input(|i| i.pointer.press_origin())
                .unwrap_or(rect.center());
            zoom_at(camera, anchor, rect, (-f64::from(delta.y) * 0.01).exp());
        } else {
            camera.centre_mm[0] -= f64::from(delta.x) / camera.pixels_per_mm
                * if camera.mirrored_x { -1.0 } else { 1.0 };
            camera.centre_mm[1] += f64::from(delta.y) / camera.pixels_per_mm;
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    if let Some(at) = response.hover_pos() {
        // Ctrl+Shift+wheel belongs to the editor (PCB layer / SCH channel).
        // egui otherwise turns it into a smoothed zoom, including later frames.
        let tail_key = response.id.with("editor_wheel_tail");
        let special=ui.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::MouseWheel{modifiers,..} if modifiers.ctrl&&modifiers.shift)));
        let ordinary=ui.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::MouseWheel{modifiers,..} if !(modifiers.ctrl&&modifiers.shift))||matches!(e,egui::Event::Zoom(_))));
        let suppress = ui.ctx().data_mut(|d| {
            let old = d.get_temp::<bool>(tail_key).unwrap_or(false);
            let next = special || old && !ordinary;
            d.insert_temp(tail_key, next);
            next
        });
        let (scroll, zoom) = if suppress {
            (egui::Vec2::ZERO, 1.)
        } else {
            ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()))
        };
        if zoom != 1.0 {
            zoom_at(camera, at, rect, f64::from(zoom));
        }
        let mirror = if camera.mirrored_x { -1.0 } else { 1.0 };
        if wheel_zoom && !modifiers.shift {
            zoom_at(camera, at, rect, (f64::from(scroll.y) * 0.003).exp());
            camera.centre_mm[0] -= f64::from(scroll.x) / camera.pixels_per_mm * mirror;
        } else if modifiers.shift {
            camera.centre_mm[0] -= f64::from(scroll.x + scroll.y) / camera.pixels_per_mm * mirror;
        } else {
            camera.centre_mm[0] -= f64::from(scroll.x) / camera.pixels_per_mm * mirror;
            camera.centre_mm[1] += f64::from(scroll.y) / camera.pixels_per_mm;
        }
    }
    right || middle
}

pub(crate) fn zoom_at(camera: &mut Camera, point: egui::Pos2, rect: Rect, factor: f64) {
    if !factor.is_finite() || factor <= 0.0 {
        return;
    }
    let before = camera.world_mm(point, rect);
    camera.pixels_per_mm = (camera.pixels_per_mm * factor).clamp(0.001, 100_000.0);
    let after = camera.world_mm(point, rect);
    camera.centre_mm[0] += before[0] - after[0];
    camera.centre_mm[1] += before[1] - after[1];
}
