use super::*;
use theme::{Icon, icon_button};

impl PcbCraft {
    pub(super) fn title_bar(&mut self, ctx: &egui::Context) {
        // AD24 keeps the same title strip in both document editors.
        let height = 23.;
        let title = self.git_review_title().unwrap_or_else(|| {
            self.active
                .map(|i| {
                    format!(
                        "{}{} - PCBCraft",
                        self.documents[i].model.name,
                        if self.documents[i].dirty() { " *" } else { "" }
                    )
                })
                .unwrap_or_else(|| "PCBCraft".into())
        });
        egui::TopBottomPanel::top("window_title")
            .exact_height(height)
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(74)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.horizontal(|ui| {
                    for (icon, command, tip) in [
                        (Icon::New, Command::NewBoard, "New PCB document"),
                        (Icon::Save, Command::Save, "Save"),
                        (Icon::SaveAll, Command::SaveAll, "Save All"),
                        (Icon::Open, Command::Open, "Open"),
                        (Icon::Undo, Command::Undo, "Undo"),
                        (Icon::Redo, Command::Redo, "Redo"),
                    ] {
                        let enabled = (!self.git_review_active()
                            || matches!(command, Command::Open))
                            && match command {
                                Command::Save => self.active.is_some(),
                                Command::Undo => self
                                    .active
                                    .is_some_and(|i| self.documents[i].history.can_undo()),
                                Command::Redo => self
                                    .active
                                    .is_some_and(|i| self.documents[i].history.can_redo()),
                                _ => true,
                            };
                        if icon_button(ui, icon, false, enabled)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            self.command = Some(command);
                        }
                    }
                    let width =
                        (ui.available_width() - 300.0 - crate::updates::CONTROL_WIDTH).max(10.0);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(width, height),
                        egui::Sense::click_and_drag(),
                    );
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        title,
                        egui::FontId::proportional(11.5),
                        egui::Color32::LIGHT_GRAY,
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if response.double_clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                            !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                        ));
                    }
                    self.updates.show(ui);
                    theme::draw_icon(
                        ui.painter(),
                        egui::Rect::from_center_size(
                            ui.cursor().min + egui::vec2(9.0, 11.0),
                            egui::vec2(11.0, 11.0),
                        ),
                        Icon::Search,
                        true,
                    );
                    ui.add_space(20.0);
                    ui.add_sized(
                        [170.0, 19.0],
                        egui::TextEdit::singleline(&mut self.search).hint_text("Search"),
                    );
                    if icon_button(ui, Icon::Minimize, false, true).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    if icon_button(ui, Icon::Maximize, false, true).clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                            !ctx.input(|i| i.viewport().maximized.unwrap_or(false)),
                        ));
                    }
                    if icon_button(ui, Icon::Close, false, true)
                        .on_hover_text("Close")
                        .clicked()
                    {
                        self.command = Some(Command::Quit);
                    }
                });
            });
    }
    fn routing_pause(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if self
            .active
            .is_some_and(|i| self.documents[i].pcb_editor.routing_paused())
        {
            egui::Area::new(egui::Id::new("routing_pause"))
                .fixed_pos(canvas.center() - egui::vec2(50.0, 50.0))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(100.0, 100.0), egui::Sense::click());
                    ui.painter()
                        .circle_filled(rect.center(), 50.0, Color32::from_white_alpha(55));
                    for x in [-11.0, 6.0] {
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                rect.center() + egui::vec2(x, -19.0),
                                egui::vec2(7.0, 38.0),
                            ),
                            0.0,
                            Color32::WHITE,
                        );
                    }
                    if response.clicked() {
                        let editor = &mut self.documents[self.active.unwrap()].pcb_editor;
                        editor.resume_routing(ctx);
                    }
                });
        }
    }

    pub(super) fn active_bar(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let schematic = self
            .active
            .is_some_and(|i| self.documents[i].model.kind == DocumentKind::Schematic);
        if schematic {
            self.schematic_active_bar(ctx, canvas);
            return;
        }
        self.pcb_active_bar(ctx, canvas);
        self.routing_pause(ctx, canvas);
    }

    pub(super) fn resize_edges(&self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
            return;
        }
        let bounds = ctx.screen_rect();
        let (w, h) = (bounds.width(), bounds.height());
        use egui::{CursorIcon as C, ResizeDirection as D};
        for (name, pos, size, direction, cursor) in [
            (
                "n",
                egui::pos2(5.0, 0.0),
                egui::vec2(w - 10.0, 3.0),
                D::North,
                C::ResizeVertical,
            ),
            (
                "s",
                egui::pos2(5.0, h - 3.0),
                egui::vec2(w - 10.0, 3.0),
                D::South,
                C::ResizeVertical,
            ),
            (
                "w",
                egui::pos2(0.0, 5.0),
                egui::vec2(3.0, h - 10.0),
                D::West,
                C::ResizeHorizontal,
            ),
            (
                "e",
                egui::pos2(w - 3.0, 5.0),
                egui::vec2(3.0, h - 10.0),
                D::East,
                C::ResizeHorizontal,
            ),
            (
                "nw",
                egui::pos2(0.0, 0.0),
                egui::vec2(5.0, 5.0),
                D::NorthWest,
                C::ResizeNwSe,
            ),
            (
                "ne",
                egui::pos2(w - 5.0, 0.0),
                egui::vec2(5.0, 5.0),
                D::NorthEast,
                C::ResizeNeSw,
            ),
            (
                "sw",
                egui::pos2(0.0, h - 5.0),
                egui::vec2(5.0, 5.0),
                D::SouthWest,
                C::ResizeNeSw,
            ),
            (
                "se",
                egui::pos2(w - 5.0, h - 5.0),
                egui::vec2(5.0, 5.0),
                D::SouthEast,
                C::ResizeNwSe,
            ),
        ] {
            egui::Area::new(egui::Id::new(("window_resize", name)))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    let (_, response) = ui.allocate_exact_size(size, egui::Sense::drag());
                    if response.hovered() {
                        ctx.set_cursor_icon(cursor);
                    }
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                    }
                });
        }
    }
}
