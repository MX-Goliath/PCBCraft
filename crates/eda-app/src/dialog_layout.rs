//! Dialog width belongs to the form or the user's resize, not to changing text.
use egui::{Context, InnerResponse, Modal, Ui, Window};

pub trait StableWindow {
    fn show_stable<R>(
        self,
        ctx: &Context,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> Option<InnerResponse<Option<R>>>;
}

impl StableWindow for Window<'_> {
    fn show_stable<R>(
        self,
        ctx: &Context,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> Option<InnerResponse<Option<R>>> {
        self.show(ctx, |ui| {
            let width = ui.available_width();
            dialog_contents(ui, width, contents)
        })
    }
}

pub trait StableModal {
    fn show_stable<R>(
        self,
        ctx: &Context,
        width: f32,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::containers::modal::ModalResponse<R>;
}

impl StableModal for Modal {
    fn show_stable<R>(
        self,
        ctx: &Context,
        width: f32,
        contents: impl FnOnce(&mut Ui) -> R,
    ) -> egui::containers::modal::ModalResponse<R> {
        self.show(ctx, |ui| dialog_contents(ui, width, contents))
    }
}

pub fn dialog_contents<R>(ui: &mut Ui, width: f32, contents: impl FnOnce(&mut Ui) -> R) -> R {
    let width = width.clamp(1., (ui.ctx().screen_rect().width() - 40.).max(1.));
    ui.set_width(width);
    // Wrapping prevents status text, errors and combo selections from growing
    // the form. Intrinsically wide controls/tables remain reachable by scrolling;
    // their minimum rectangle must not propagate back to the window/area.
    egui::ScrollArea::horizontal()
        .id_salt("dialog_horizontal_overflow")
        .max_width(width)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.set_width(width);
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            contents(ui)
        })
        .inner
}

#[cfg(test)]
#[path = "dialog_layout_tests.rs"]
mod tests;
