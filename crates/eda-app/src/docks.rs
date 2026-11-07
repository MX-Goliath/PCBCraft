//! Dock membership and selection belong to the workspace, never to a document.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Panel {
    Projects,
    Git,
    ViewConfiguration,
    Library,
    Properties,
    Components,
    Messages,
    Changes,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Side {
    Left,
    Right,
}
impl Panel {
    const ALL: [Self; 8] = [
        Self::Projects,
        Self::Git,
        Self::ViewConfiguration,
        Self::Library,
        Self::Properties,
        Self::Components,
        Self::Messages,
        Self::Changes,
    ];
    fn side(self) -> Side {
        match self {
            Self::Projects | Self::Git | Self::ViewConfiguration | Self::Library => Side::Left,
            _ => Side::Right,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Projects => "Projects",
            Self::Git => "Git",
            Self::ViewConfiguration => "View Configuration",
            Self::Library => "Library",
            Self::Properties => "Properties",
            Self::Components => "Components",
            Self::Messages => "Messages",
            Self::Changes => "Changes",
        }
    }
}
impl PcbCraft {
    pub(super) fn panel_open(&self, panel: Panel) -> bool {
        match panel {
            Panel::Projects => self.settings.projects,
            Panel::Git => self.settings.git,
            Panel::ViewConfiguration => self.settings.view_configuration,
            Panel::Library => self.settings.library_panel,
            Panel::Properties => self.settings.properties,
            Panel::Components => self.settings.components,
            Panel::Messages => self.settings.messages,
            Panel::Changes => self.git.details_open,
        }
    }
    fn set_panel_open(&mut self, panel: Panel, open: bool) {
        match panel {
            Panel::Projects => self.settings.projects = open,
            Panel::Git => self.settings.git = open,
            Panel::ViewConfiguration => self.settings.view_configuration = open,
            Panel::Library => self.settings.library_panel = open,
            Panel::Properties => self.settings.properties = open,
            Panel::Components => self.settings.components = open,
            Panel::Messages => self.settings.messages = open,
            Panel::Changes => self.git.details_open = open,
        }
    }
    pub(super) fn selected_panel(&self, side: Side) -> Option<Panel> {
        let selected = match side {
            Side::Left => self.settings.left_panel.unwrap_or(if self.settings.git {
                Panel::Git
            } else {
                Panel::Projects
            }),
            Side::Right if self.settings.right_changes => Panel::Changes,
            Side::Right if self.settings.right_components => Panel::Components,
            Side::Right if self.settings.right_messages => Panel::Messages,
            Side::Right => Panel::Properties,
        };
        Some(selected)
            .filter(|p| p.side() == side && self.panel_open(*p))
            .or_else(|| {
                Panel::ALL
                    .into_iter()
                    .find(|p| p.side() == side && self.panel_open(*p))
            })
    }
    pub(super) fn show_panel(&mut self, panel: Panel) {
        self.set_panel_open(panel, true);
        match panel.side() {
            Side::Left => self.settings.left_panel = Some(panel),
            Side::Right => {
                self.settings.right_changes = panel == Panel::Changes;
                self.settings.right_components = panel == Panel::Components;
                self.settings.right_messages = panel == Panel::Messages;
            }
        }
    }
    pub(super) fn close_panel(&mut self, panel: Panel) {
        let active = self.selected_panel(panel.side());
        self.set_panel_open(panel, false);
        if active != Some(panel) {
            return;
        }
        let position = Panel::ALL.iter().position(|p| *p == panel).unwrap();
        let next = Panel::ALL[..position]
            .iter()
            .rev()
            .chain(Panel::ALL[position + 1..].iter())
            .copied()
            .find(|p| p.side() == panel.side() && self.panel_open(*p));
        if let Some(next) = next {
            self.show_panel(next);
        }
    }
    pub(super) fn toggle_panel(&mut self, panel: Panel) {
        if self.selected_panel(panel.side()) == Some(panel) {
            self.close_panel(panel);
        } else {
            self.show_panel(panel);
        }
    }
    pub(super) fn restore_docks(&mut self) {
        for panel in Panel::ALL {
            self.set_panel_open(panel, false);
        }
        self.show_panel(Panel::Projects);
        self.show_panel(Panel::Properties);
        for doc in &mut self.documents {
            doc.view.pcb.show_layer_dialog = false;
        }
    }
    pub(super) fn panels_menu(&mut self, ui: &mut egui::Ui) {
        for panel in Panel::ALL {
            let available = self.panel_open(panel)
                || match panel {
                    Panel::ViewConfiguration => self
                        .active
                        .is_some_and(|i| self.documents[i].model.pcb.is_some()),
                    Panel::Library => self.library_active(),
                    Panel::Changes => !self.git.reviews.is_empty(),
                    _ => true,
                };
            if ui
                .add_enabled(
                    available,
                    egui::Button::new(panel.title()).selected(self.panel_open(panel)),
                )
                .clicked()
            {
                self.command = Some(Command::ShowPanel(panel));
                ui.close();
            }
        }
    }
    fn dock_chrome(&mut self, ui: &mut egui::Ui, panel: Panel) {
        let side = panel.side();
        ui.set_min_width(ui.available_width());
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 25.), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0., Color32::from_gray(75));
        ui.painter().text(
            rect.left_center() + egui::vec2(6., 0.),
            egui::Align2::LEFT_CENTER,
            panel.title(),
            egui::FontId::new(12., egui::FontFamily::Name("ui-bold".into())),
            Color32::from_gray(230),
        );
        let close = egui::Rect::from_center_size(
            rect.right_center() - egui::vec2(11., 0.),
            egui::vec2(22., 23.),
        );
        theme::draw_icon(ui.painter(), close.shrink(5.), theme::Icon::Close, true);
        if ui
            .interact(close, ui.id().with("dock_close"), egui::Sense::click())
            .on_hover_text(format!("Close {} panel", panel.title()))
            .clicked()
        {
            self.close_panel(panel);
        }
        egui::TopBottomPanel::bottom(if side == Side::Left {
            "left_dock_tabs"
        } else {
            "right_dock_tabs"
        })
        .exact_height(25.)
        .frame(egui::Frame::NONE.fill(Color32::from_gray(51)))
        .show_inside(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("panel_tabs_scroll")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.;
                        for tab in Panel::ALL {
                            if tab.side() != side || !self.panel_open(tab) {
                                continue;
                            }
                            let response = ui.selectable_label(panel == tab, tab.title());
                            if response.clicked() {
                                self.show_panel(tab);
                            }
                            if response.clicked_by(egui::PointerButton::Middle) {
                                self.close_panel(tab);
                            }
                            response.context_menu(|ui| {
                                if ui.button(format!("Close {}", tab.title())).clicked() {
                                    self.close_panel(tab);
                                    ui.close();
                                }
                            });
                        }
                    });
                });
        });
        ui.add_space(5.);
    }
    pub(super) fn left_dock(&mut self, ctx: &egui::Context) {
        if ctx.available_rect().width() < 600. || ctx.available_rect().height() < 100. {
            return;
        }
        // Existing layer-bar requests are one-shot commands, not panel visibility.
        if let Some(i) = self.active
            && std::mem::take(&mut self.documents[i].view.pcb.show_layer_dialog)
        {
            self.show_panel(Panel::ViewConfiguration);
        }
        let Some(panel) = self.selected_panel(Side::Left) else {
            return;
        };
        egui::SidePanel::left("workspace_left_dock_v1")
            .default_width(334.)
            .width_range(280. ..=650.)
            .resizable(true)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                self.dock_chrome(ui, panel);
                match panel {
                    Panel::Projects => self.projects_content(ui),
                    Panel::Git => self.git_panel_content(ui),
                    Panel::ViewConfiguration => self.view_configuration_content(ui),
                    Panel::Library => {
                        self.library_panel_content(ui);
                    }
                    _ => unreachable!(),
                }
            });
    }
    pub(super) fn right_dock(&mut self, ctx: &egui::Context) {
        self.right_dock_with_properties(ctx, |app, ui| {
            let compiled = app
                .active
                .is_some_and(|i| app.documents[i].sch_editor.compiled_path.is_some());
            ui.add_enabled_ui(!compiled && !app.git_review_active(), |ui| {
                app.properties(ui)
            });
        });
    }
    pub(super) fn right_dock_with_properties(
        &mut self,
        ctx: &egui::Context,
        properties: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let Some(panel) = self.selected_panel(Side::Right) else {
            return;
        };
        egui::SidePanel::right("workspace_right_dock_v1")
            .default_width(316.)
            .width_range(260. ..=550.)
            .resizable(true)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ctx, |ui| {
                if let Some(position) = ctx.input(|i| i.pointer.press_origin()) {
                    self.property_ui.focused = ui.max_rect().contains(position);
                }
                self.dock_chrome(ui, panel);
                match panel {
                    Panel::Changes => self.git_details_content(ui),
                    Panel::Messages => self.messages_panel(ui),
                    Panel::Components => self.components_panel(ui),
                    Panel::Properties => properties(self, ui),
                    _ => unreachable!(),
                }
            });
    }
    fn view_configuration_content(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self
            .active
            .filter(|i| self.documents[*i].model.pcb.is_some())
        else {
            ui.label("Select a PCB document or PCB comparison to configure its view.");
            return;
        };
        let doc = &mut self.documents[i];
        let previous_mode = doc.view.pcb.three_d.enabled;
        let layer = doc.view.pcb.configuration_panel(
            ui,
            doc.model.pcb.as_ref().unwrap(),
            &mut doc.view.grid,
            &mut doc.view.camera,
        );
        let mode = doc.view.pcb.three_d.enabled;
        doc.view.pcb.three_d.enabled = previous_mode;
        if previous_mode != mode {
            self.set_pcb_3d(mode);
        }
        if let Some(layer) = layer {
            self.command = Some(Command::Pcb(pcb_editor::Action::RouteLayer(layer)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_switching_never_replaces_or_closes_open_panels() {
        let mut app = crate::app::schematic_editor::tests::app();
        app.add_document(Document::new(DocumentKind::Board, "Board"), None);
        app.show_panel(Panel::Git);
        app.show_panel(Panel::ViewConfiguration);
        app.show_panel(Panel::Changes);
        for active in [Some(0), Some(1), None, Some(1)] {
            app.active = active;
            assert_eq!(
                app.selected_panel(Side::Left),
                Some(Panel::ViewConfiguration)
            );
            assert_eq!(app.selected_panel(Side::Right), Some(Panel::Changes));
            assert!(app.panel_open(Panel::Git));
            assert!(app.panel_open(Panel::Projects));
        }
        // Showing an already open tab never toggles it off or closes its peers.
        app.run(Command::ShowPanel(Panel::Git), &egui::Context::default());
        app.run(Command::ShowPanel(Panel::Git), &egui::Context::default());
        assert_eq!(app.selected_panel(Side::Left), Some(Panel::Git));
        assert!(app.panel_open(Panel::ViewConfiguration));
        app.show_panel(Panel::ViewConfiguration);
        app.close_panel(Panel::ViewConfiguration);
        assert_eq!(app.selected_panel(Side::Left), Some(Panel::Git));
        app.show_panel(Panel::Properties);
        app.close_panel(Panel::Changes);
        assert_eq!(app.selected_panel(Side::Right), Some(Panel::Properties));
        assert!(app.panel_open(Panel::Git));
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&app.settings).unwrap()).unwrap();
        app.settings = restored;
        assert_eq!(app.selected_panel(Side::Left), Some(Panel::Git));
        app.restore_docks();
        assert_eq!(app.selected_panel(Side::Left), Some(Panel::Projects));
        assert_eq!(app.selected_panel(Side::Right), Some(Panel::Properties));
        assert!(!app.panel_open(Panel::Git));
        assert!(!app.panel_open(Panel::ViewConfiguration));
    }

    #[test]
    fn dock_layout_keeps_tabs_and_widths_on_sch_pcb_and_home() {
        let mut app = crate::app::schematic_editor::tests::app();
        app.add_document(Document::new(DocumentKind::Board, "Board"), None);
        app.show_panel(Panel::Git);
        app.show_panel(Panel::ViewConfiguration);
        app.show_panel(Panel::Changes);
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        for active in [Some(0), Some(1), None, Some(0)] {
            app.active = active;
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1440., 900.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    app.left_dock(ctx);
                    app.right_dock(ctx);
                    assert!((ctx.available_rect().left() - 334.).abs() < 1.);
                    assert!((ctx.available_rect().right() - 1124.).abs() < 1.);
                },
            );
            fn labels(shape: &egui::Shape, text: &mut Vec<String>) {
                match shape {
                    egui::Shape::Text(t) => text.push(t.galley.job.text.clone()),
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            labels(shape, text);
                        }
                    }
                    _ => {}
                }
            }
            let mut text = Vec::new();
            for shape in output.shapes {
                labels(&shape.shape, &mut text);
            }
            for title in [
                "Git",
                "Projects",
                "View Configuration",
                "Properties",
                "Changes",
            ] {
                assert!(
                    text.iter().any(|label| label == title),
                    "Missing {title}: {text:?}"
                );
            }
            for stub in ["Navigator", "PCB Filter", "SCH Filter", "PCB"] {
                assert!(!text.iter().any(|label| label == stub));
            }
        }
    }
}
