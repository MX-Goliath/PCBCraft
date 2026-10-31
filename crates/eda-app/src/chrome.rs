use super::*;
use theme::{Icon, icon_button};

impl PcbCraft {
    pub(super) fn title_bar(&mut self, ctx: &egui::Context) {
        let title = self
            .active
            .map(|i| {
                format!(
                    "{}{} - PCBCraft",
                    self.documents[i].model.name,
                    if self.documents[i].dirty() { " *" } else { "" }
                )
            })
            .unwrap_or_else(|| "PCBCraft".into());
        egui::TopBottomPanel::top("window_title")
            .exact_height(31.0)
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
                        let enabled = match command {
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
                    let width = (ui.available_width() - 300.0).max(10.0);
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(width, 31.0),
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
        egui::Area::new(egui::Id::new("active_bar"))
            .fixed_pos(egui::pos2(canvas.center().x - 237.0, canvas.top() + 7.0))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(4)
                    .corner_radius(2)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = if schematic { 11.5 } else { 9.0 };
                        ui.horizontal(|ui| {
                            let tools: &[(Icon, &str, Option<Command>)] = if schematic {
                                &[
                                    (
                                        Icon::Select,
                                        "Select / Cancel",
                                        Some(Command::SchEdit(SchAction::Select)),
                                    ),
                                    (Icon::Grid, "Grids", Some(Command::ToggleGrid)),
                                    (Icon::Fit, "Fit Document", Some(Command::Fit)),
                                    (
                                        Icon::Component,
                                        "Place Resistor",
                                        Some(Command::SchPlace(Placement::Resistor)),
                                    ),
                                    (
                                        Icon::Chip,
                                        "Place Capacitor",
                                        Some(Command::SchPlace(Placement::Capacitor)),
                                    ),
                                    (
                                        Icon::Track,
                                        "Place Wire (W)",
                                        Some(Command::SchPlace(Placement::Wire)),
                                    ),
                                    (
                                        Icon::Pad,
                                        "Place Junction (J)",
                                        Some(Command::SchPlace(Placement::Junction)),
                                    ),
                                    (
                                        Icon::Filter,
                                        "Place Power Port",
                                        Some(Command::SchPlace(Placement::Ground)),
                                    ),
                                    (
                                        Icon::Text,
                                        "Place Net Label (L)",
                                        Some(Command::SchPlace(Placement::NetLabel)),
                                    ),
                                    (
                                        Icon::Text,
                                        "Place Text String (T)",
                                        Some(Command::SchPlace(Placement::Text)),
                                    ),
                                    (
                                        Icon::Line,
                                        "Place Line",
                                        Some(Command::SchPlace(Placement::Line)),
                                    ),
                                    (
                                        Icon::Component,
                                        "Move selected (M)",
                                        Some(Command::SchEdit(SchAction::Move)),
                                    ),
                                    (
                                        Icon::Redo,
                                        "Rotate selected (Space)",
                                        Some(Command::SchEdit(SchAction::Rotate)),
                                    ),
                                ]
                            } else {
                                &[
                                    (Icon::Filter, "Selection Filter", None),
                                    (Icon::Magnet, "Object Snap", None),
                                    (Icon::Crosshair, "Set Origin", None),
                                    (
                                        Icon::Select,
                                        "Select / Cancel",
                                        Some(Command::Pcb(pcb_editor::Action::Tool(
                                            pcb_editor::Tool::Select,
                                        ))),
                                    ),
                                    (Icon::Chart, "Measure", None),
                                    (Icon::Chip, "Component", None),
                                    (
                                        Icon::Track,
                                        "Interactive Routing (Ctrl+W)",
                                        Some(Command::Pcb(pcb_editor::Action::Tool(
                                            pcb_editor::Tool::Route,
                                        ))),
                                    ),
                                    (Icon::MultiRoute, "Interactive Multi-Routing", None),
                                    (
                                        Icon::Via,
                                        "Place Via (P, V)",
                                        Some(Command::Pcb(pcb_editor::Action::Tool(
                                            pcb_editor::Tool::Via,
                                        ))),
                                    ),
                                    (
                                        Icon::Fill,
                                        "Place Fill (P, F)",
                                        Some(Command::Pcb(pcb_editor::Action::Tool(
                                            pcb_editor::Tool::Fill,
                                        ))),
                                    ),
                                    (
                                        Icon::Polygon,
                                        "Polygon Pour (P, G)",
                                        Some(Command::Pcb(pcb_editor::Action::Tool(
                                            pcb_editor::Tool::Polygon,
                                        ))),
                                    ),
                                    (Icon::Dimension, "Dimension", None),
                                    (
                                        Icon::Text,
                                        "Place String (P, S)",
                                        Some(Command::Pcb(pcb_editor::Action::Tool(
                                            pcb_editor::Tool::Text,
                                        ))),
                                    ),
                                    (Icon::Line, "Place Line", None),
                                ]
                            };
                            for &(icon, tip, command) in tools {
                                let needs_selection = matches!(
                                    command,
                                    Some(Command::SchEdit(SchAction::Move | SchAction::Rotate))
                                );
                                let enabled = command.is_some()
                                    && (!needs_selection
                                        || self.active.is_some_and(|i| {
                                            self.documents[i].view.selected.is_some()
                                        }));
                                let selected = match command {
                                    Some(Command::Pcb(pcb_editor::Action::Tool(tool))) => {
                                        self.active.is_some_and(|i| {
                                            self.documents[i].pcb_editor.tool == tool
                                                && tool != pcb_editor::Tool::Select
                                        })
                                    }
                                    _ => matches!(icon, Icon::Grid) && self.settings.show_grid,
                                };
                                if icon_button(ui, icon, selected, enabled)
                                    .on_hover_text(tip)
                                    .clicked()
                                    && let Some(command) = command
                                {
                                    self.command = Some(command);
                                }
                            }
                        });
                    });
            });
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
