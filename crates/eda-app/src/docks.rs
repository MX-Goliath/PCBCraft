//! Workspace docking: tabs, recursive splits, popup rails and native floating windows.
use super::*;
#[path = "dock_layout.rs"]
mod layout;
use egui::{Align2, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};
pub(super) use layout::Layout;
use layout::{Node, Root, Target};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Side {
    Left,
    Right,
    Top,
    Bottom,
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
pub(super) struct Runtime {
    groups: Vec<GroupRect>,
    drag: Option<Drag>,
    popup: Option<u64>,
    popup_panel: Option<Panel>,
    popup_click: bool,
    popup_time: f64,
    main_rect: Rect,
    finished: bool,
    main_origin: Vec2,
    main_scale: f32,
    floating_hidden: bool,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            groups: vec![],
            drag: None,
            popup: None,
            popup_panel: None,
            popup_click: false,
            popup_time: 0.,
            main_rect: Rect::NOTHING,
            main_origin: Vec2::ZERO,
            main_scale: 1.,
            finished: false,
            floating_hidden: false,
        }
    }
}

struct GroupRect {
    id: u64,
    viewport: egui::ViewportId,
    rect: Rect,
    tabs: Vec<(Panel, Rect)>,
}
struct Drag {
    panels: Vec<Panel>,
    source: egui::ViewportId,
    pointer: Pos2,
    size: Vec2,
    offset: Vec2,
    released: bool,
    cancel: bool,
    ctrl: bool,
}
impl PcbCraft {
    pub(super) fn prepare_dock_input(&mut self, ctx: &egui::Context) {
        self.update_dock_drag(ctx);
    }
    pub(super) fn dock_drag_active(&self) -> bool {
        self.dock_runtime.drag.is_some()
    }
    pub(super) fn save_dock_visibility(&mut self) {
        self.settings.dock_visible = Some(
            Panel::ALL
                .into_iter()
                .filter(|p| self.panel_open(*p))
                .collect(),
        );
    }
    pub(super) fn restore_dock_visibility(&mut self) {
        if let Some(panels) = self.settings.dock_visible.clone() {
            for p in Panel::ALL {
                self.set_panel_open(p, panels.contains(&p));
            }
        }
    }
    fn ensure_docks(&mut self) {
        if self.settings.dock_layout.is_none() {
            let left = self.selected_panel(Side::Left);
            let right = self.selected_panel(Side::Right);
            let mut layout = Layout::default();
            for p in [left, right].into_iter().flatten() {
                layout.select(p);
            }
            self.settings.dock_layout = Some(layout);
        }
    }
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
        if let Some(layout) = &self.settings.dock_layout {
            return layout
                .roots
                .iter()
                .filter(|r| r.edge == Some(side) && !r.auto_hide)
                .find_map(|r| self.active_in_node(&r.tree));
        }
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
            _ => return None,
        };
        Some(selected).filter(|p| self.panel_open(*p)).or_else(|| {
            Panel::ALL
                .into_iter()
                .find(|p| p.side() == side && self.panel_open(*p))
        })
    }
    fn active_in_node(&self, node: &Node) -> Option<Panel> {
        match node {
            Node::Group { tabs, active, .. } => Some(*active)
                .filter(|p| self.panel_open(*p))
                .or_else(|| tabs.iter().rev().copied().find(|p| self.panel_open(*p))),
            Node::Split { first, second, .. } => self
                .active_in_node(first)
                .or_else(|| self.active_in_node(second)),
        }
    }
    pub(super) fn show_panel(&mut self, panel: Panel) {
        if panel == Panel::Git {
            self.git.follow_active_document();
        }
        self.set_panel_open(panel, true);
        self.ensure_docks();
        self.settings.dock_layout.as_mut().unwrap().select(panel);
        if let Some(root) = self.settings.dock_layout.as_ref().unwrap().root_for(panel) {
            if root.auto_hide {
                self.dock_runtime.popup = Some(root.id);
                self.dock_runtime.popup_panel = Some(panel);
                self.dock_runtime.popup_click = true;
            }
            if root.edge.is_none() {
                self.dock_runtime.floating_hidden = false;
            }
        }
        match panel.side() {
            Side::Left => self.settings.left_panel = Some(panel),
            _ => {
                self.settings.right_changes = panel == Panel::Changes;
                self.settings.right_components = panel == Panel::Components;
                self.settings.right_messages = panel == Panel::Messages;
            }
        }
    }
    pub(super) fn close_panel(&mut self, panel: Panel) {
        self.set_panel_open(panel, false);
        if let Some(layout) = &mut self.settings.dock_layout {
            for root in &mut layout.roots {
                choose_after_close(&mut root.tree, panel);
            }
        }
    }
    pub(super) fn toggle_panel(&mut self, panel: Panel) {
        if self.panel_selected(panel) {
            self.close_panel(panel);
        } else {
            self.show_panel(panel);
        }
    }
    pub(super) fn panel_selected(&self, panel: Panel) -> bool {
        fn selected(app: &PcbCraft, node: &Node, panel: Panel) -> bool {
            match node {
                Node::Group { tabs, .. } => {
                    tabs.contains(&panel) && app.active_in_node(node) == Some(panel)
                }
                Node::Split { first, second, .. } => {
                    selected(app, first, panel) || selected(app, second, panel)
                }
            }
        }
        self.settings
            .dock_layout
            .as_ref()
            .and_then(|l| l.root_for(panel))
            .map(|r| selected(self, &r.tree, panel))
            .unwrap_or_else(|| self.selected_panel(panel.side()) == Some(panel))
    }
    pub(super) fn restore_docks(&mut self) {
        self.settings.dock_layout = Some(Layout::default());
        self.dock_runtime = Runtime::default();
        for panel in Panel::ALL {
            self.set_panel_open(panel, false);
        }
        self.show_panel(Panel::Projects);
        self.show_panel(Panel::Properties);
        for doc in &mut self.documents {
            doc.view.pcb.show_layer_dialog = false;
        }
    }
    fn panel_title(&self, panel: Panel) -> &'static str {
        if panel == Panel::Library && self.library_active() {
            if self.documents[self.active.unwrap()].model.kind == eda_core::DocumentKind::Board {
                "PCB Library"
            } else {
                "SCH Library"
            }
        } else {
            panel.title()
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
                    egui::Button::new(self.panel_title(panel)).selected(self.panel_open(panel)),
                )
                .clicked()
            {
                self.command = Some(Command::ShowPanel(panel));
                ui.close();
            }
        }
    }
    pub(super) fn left_dock(&mut self, ctx: &egui::Context) {
        self.left_dock_with_properties(ctx, |app, ui| app.normal_properties(ui));
    }
    pub(super) fn left_dock_with_properties(
        &mut self,
        ctx: &egui::Context,
        mut properties: impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        self.ensure_docks();
        self.dock_runtime.finished = false;
        self.dock_runtime.groups.clear();
        self.dock_runtime.main_rect = ctx.available_rect();
        self.dock_runtime.main_origin = viewport_origin(ctx);
        self.dock_runtime.main_scale = ctx.pixels_per_point();
        if ctx.available_rect().width() < 600. || ctx.available_rect().height() < 100. {
            return;
        }
        self.update_dock_drag(ctx);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F4)) {
            self.dock_runtime.floating_hidden = !self.dock_runtime.floating_hidden;
        }
        if let Some(i) = self.active
            && std::mem::take(&mut self.documents[i].view.pcb.show_layer_dialog)
        {
            self.show_panel(Panel::ViewConfiguration);
        }
        for side in [Side::Top, Side::Bottom, Side::Left] {
            self.dock_edge(ctx, side, &mut properties);
        }
    }
    pub(super) fn right_dock(&mut self, ctx: &egui::Context) {
        self.right_dock_with_properties(ctx, |app, ui| app.normal_properties(ui));
    }
    pub(super) fn right_dock_with_properties(
        &mut self,
        ctx: &egui::Context,
        mut properties: impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        self.ensure_docks();
        self.dock_edge(ctx, Side::Right, &mut properties);
    }
    fn normal_properties(&mut self, ui: &mut egui::Ui) {
        let compiled = self
            .active
            .is_some_and(|i| self.documents[i].sch_editor.compiled_path.is_some());
        ui.add_enabled_ui(!compiled && !self.git_review_active(), |ui| {
            self.properties(ui)
        });
    }
    fn dock_edge(
        &mut self,
        ctx: &egui::Context,
        side: Side,
        properties: &mut impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        if ctx.available_rect().width() < 200. || ctx.available_rect().height() < 100. {
            return;
        }
        let roots: Vec<_> = self
            .settings
            .dock_layout
            .as_ref()
            .unwrap()
            .roots
            .iter()
            .filter(|r| r.edge == Some(side) && self.active_in_node(&r.tree).is_some())
            .cloned()
            .collect();
        let popup_roots: Vec<_> = roots.iter().filter(|r| r.auto_hide).cloned().collect();
        if !popup_roots.is_empty() {
            self.dock_rail(ctx, side, &popup_roots);
        }
        for root in roots.into_iter().filter(|r| !r.auto_hide) {
            let id = Id::new(("workspace_dock", root.id));
            let frame = egui::Frame::NONE.fill(theme::PANEL);
            let extent = root.extent;
            let response = match side {
                Side::Left | Side::Right => {
                    let max = (ctx.available_rect().width() - 250.).max(100.);
                    let panel = if side == Side::Left {
                        egui::SidePanel::left(id)
                    } else {
                        egui::SidePanel::right(id)
                    };
                    panel
                        .default_width(extent)
                        .width_range(100. ..=max)
                        .resizable(true)
                        .frame(frame)
                        .show(ctx, |ui| self.dock_node(ui, &root, &root.tree, properties))
                        .response
                }
                _ => {
                    let max = (ctx.available_rect().height() - 180.).max(80.);
                    let panel = if side == Side::Top {
                        egui::TopBottomPanel::top(id)
                    } else {
                        egui::TopBottomPanel::bottom(id)
                    };
                    panel
                        .default_height(extent)
                        .height_range(80. ..=max)
                        .resizable(true)
                        .frame(frame)
                        .show(ctx, |ui| self.dock_node(ui, &root, &root.tree, properties))
                        .response
                }
            };
            let size = if matches!(side, Side::Left | Side::Right) {
                response.rect.width()
            } else {
                response.rect.height()
            };
            if let Some(r) = self.root_mut(root.id) {
                r.extent = size;
            }
        }
    }
    fn root_mut(&mut self, id: u64) -> Option<&mut Root> {
        self.settings
            .dock_layout
            .as_mut()?
            .roots
            .iter_mut()
            .find(|r| r.id == id)
    }
    fn dock_node(
        &mut self,
        ui: &mut egui::Ui,
        root: &Root,
        node: &Node,
        properties: &mut impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        match node {
            Node::Group { id, tabs, .. } => {
                let Some(panel) = self.active_in_node(node) else {
                    return;
                };
                let tabs: Vec<_> = tabs
                    .iter()
                    .copied()
                    .filter(|p| self.panel_open(*p))
                    .collect();
                self.dock_group(ui, root, *id, &tabs, panel, properties);
            }
            Node::Split {
                horizontal,
                fraction,
                first,
                second,
            } => {
                if self.active_in_node(first).is_none() {
                    self.dock_node(ui, root, second, properties);
                    return;
                }
                if self.active_in_node(second).is_none() {
                    self.dock_node(ui, root, first, properties);
                    return;
                }
                let rect = ui.available_rect_before_wrap();
                let total = if *horizontal {
                    rect.width()
                } else {
                    rect.height()
                };
                let minimum = 45_f32.min(total * 0.5);
                let gap = 2_f32.min(total * 0.5);
                let cut = (total * fraction.clamp(0.1, 0.9)).clamp(minimum, total - minimum);
                let (a, handle, b) = if *horizontal {
                    (
                        Rect::from_min_max(rect.min, pos2(rect.left() + cut - gap, rect.bottom())),
                        Rect::from_min_max(
                            pos2(rect.left() + cut - gap, rect.top()),
                            pos2(rect.left() + cut + gap, rect.bottom()),
                        ),
                        Rect::from_min_max(pos2(rect.left() + cut + gap, rect.top()), rect.max),
                    )
                } else {
                    (
                        Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + cut - gap)),
                        Rect::from_min_max(
                            pos2(rect.left(), rect.top() + cut - gap),
                            pos2(rect.right(), rect.top() + cut + gap),
                        ),
                        Rect::from_min_max(pos2(rect.left(), rect.top() + cut + gap), rect.max),
                    )
                };
                let response = ui
                    .interact(
                        handle,
                        ui.id().with((root.id, first.panels(), "split")),
                        Sense::drag(),
                    )
                    .on_hover_cursor(if *horizontal {
                        egui::CursorIcon::ResizeHorizontal
                    } else {
                        egui::CursorIcon::ResizeVertical
                    });
                if response.dragged()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    let f = if *horizontal {
                        (p.x - rect.left()) / total
                    } else {
                        (p.y - rect.top()) / total
                    };
                    if let Some(r) = self.root_mut(root.id) {
                        set_fraction(&mut r.tree, &first.panels(), f.clamp(0.1, 0.9));
                    }
                }
                ui.painter().rect_filled(handle, 0., Color32::from_gray(45));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(a)
                        .id_salt((root.id, "first", first.panels())),
                    |ui| {
                        ui.set_clip_rect(a.intersect(ui.clip_rect()));
                        self.dock_node(ui, root, first, properties);
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(b).id_salt((
                        root.id,
                        "second",
                        second.panels(),
                    )),
                    |ui| {
                        ui.set_clip_rect(b.intersect(ui.clip_rect()));
                        self.dock_node(ui, root, second, properties);
                    },
                );
                ui.allocate_rect(rect, Sense::hover());
            }
        }
    }
    fn dock_group(
        &mut self,
        ui: &mut egui::Ui,
        root: &Root,
        group: u64,
        tabs: &[Panel],
        panel: Panel,
        properties: &mut impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        let rect = ui.available_rect_before_wrap();
        let scale = self.dock_scale(ui.ctx());
        let origin = viewport_origin(ui.ctx()) * scale;
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), 25.));
        let selected = ui.ctx().input(|i| {
            i.pointer.latest_pos().is_some_and(|p| rect.contains(p))
                && i.viewport().focused.unwrap_or(true)
        });
        ui.painter().rect_filled(
            header,
            0.,
            if selected {
                Color32::from_rgb(74, 117, 150)
            } else {
                Color32::from_gray(75)
            },
        );
        let title = Rect::from_min_max(
            header.min,
            pos2(
                (header.left() + 12. + self.panel_title(panel).len() as f32 * 7.)
                    .min(header.right() - 65.),
                header.bottom(),
            ),
        );
        ui.painter().text(
            header.left_center() + vec2(6., 0.),
            Align2::LEFT_CENTER,
            self.panel_title(panel),
            FontId::new(12., egui::FontFamily::Name("ui-bold".into())),
            Color32::from_gray(230),
        );
        let title_response = ui.interact(title, Id::new((group, "title")), Sense::click_and_drag());
        self.drag_handle(&title_response, panel, rect);
        if title_response.double_clicked() && root.edge.is_none() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                !ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)),
            ));
        }
        title_response.context_menu(|ui| self.dock_context(ui, root, panel));
        let blank = Rect::from_min_max(
            pos2(title.right(), header.top()),
            pos2(header.right() - 62., header.bottom()),
        );
        if blank.is_positive() {
            let response = ui.interact(blank, Id::new((group, "caption")), Sense::click_and_drag());
            self.drag_handle(&response, panel, rect);
            if response.double_clicked() && root.edge.is_none() {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                    !ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)),
                ));
            }
            response.context_menu(|ui| self.dock_context(ui, root, panel));
        }
        let close = Rect::from_center_size(header.right_center() - vec2(10., 0.), vec2(20., 23.));
        theme::draw_icon(ui.painter(), close.shrink(5.), theme::Icon::Close, true);
        if ui
            .interact(close, Id::new((group, "close")), Sense::click())
            .on_hover_text(format!("Close {} panel", self.panel_title(panel)))
            .clicked()
        {
            self.close_panel(panel);
        }
        let pin = close.translate(vec2(-19., 0.));
        draw_pin(ui.painter(), pin, root.auto_hide);
        if ui
            .add_enabled_ui(root.edge.is_some(), |ui| {
                ui.interact(pin, Id::new((group, "pin")), Sense::click())
            })
            .inner
            .on_hover_text(if root.auto_hide { "Dock" } else { "Auto Hide" })
            .clicked()
        {
            self.settings
                .dock_layout
                .as_mut()
                .unwrap()
                .set_auto_hide(panel, !root.auto_hide);
            self.dock_runtime.popup = None;
        }
        let menu = pin.translate(vec2(-19., 0.));
        draw_direction(ui.painter(), menu, Side::Bottom);
        egui::Popup::menu(&ui.interact(menu, Id::new((group, "menu")), Sense::click())).show(
            |ui| {
                for p in tabs {
                    if ui
                        .selectable_label(*p == panel, self.panel_title(*p))
                        .clicked()
                    {
                        self.show_panel(*p);
                        ui.close();
                    }
                }
                ui.separator();
                self.dock_context(ui, root, panel);
            },
        );
        let tab_rect = Rect::from_min_max(
            pos2(
                rect.left(),
                (rect.bottom() - if root.auto_hide { 0. } else { 25. }).max(header.bottom()),
            ),
            rect.max,
        );
        ui.painter()
            .rect_filled(tab_rect, 0., Color32::from_gray(51));
        let mut tab_regions = Vec::new();
        if !root.auto_hide {
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(tab_rect)
                    .id_salt((group, "tabs")),
                |ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt(group)
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 1.;
                                for p in tabs {
                                    let response = ui.add(
                                        egui::Button::new(self.panel_title(*p))
                                            .selected(*p == panel)
                                            .sense(Sense::click_and_drag()),
                                    );
                                    tab_regions
                                        .push((*p, (response.rect * scale).translate(origin)));
                                    if response.clicked() {
                                        self.show_panel(*p);
                                    }
                                    if response.clicked_by(egui::PointerButton::Middle) {
                                        self.close_panel(*p);
                                    }
                                    self.drag_handle(&response, *p, rect);
                                    response.context_menu(|ui| self.dock_context(ui, root, *p));
                                }
                            });
                        });
                },
            );
        }
        self.dock_runtime.groups.push(GroupRect {
            id: group,
            viewport: ui.ctx().viewport_id(),
            rect: (rect * scale).translate(origin),
            tabs: tab_regions,
        });
        let content = Rect::from_min_max(
            header.left_bottom() + vec2(8., 5.),
            tab_rect.right_top() - vec2(8., 0.),
        );
        if content.is_positive() {
            // Dock geometry belongs to the splitter/user, not to the minimum
            // width of a selected component's fields. scope_builder propagates
            // the child's min_rect back into the SidePanel even when clipped.
            // Allocate the dock rect below and keep content sizing local.
            let mut content_ui = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(content)
                    .id_salt((panel, "panel_content")),
            );
            content_ui.set_clip_rect(content.intersect(ui.clip_rect()));
            if panel == Panel::Properties {
                content_ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                if let Some(p) = ui.ctx().input(|i| i.pointer.press_origin()) {
                    self.property_ui.focused = content.contains(p);
                }
            }
            match panel {
                Panel::Projects => self.projects_content(&mut content_ui),
                Panel::Git => self.git_panel_content(&mut content_ui),
                Panel::ViewConfiguration => self.view_configuration_content(&mut content_ui),
                Panel::Library => {
                    self.library_panel_content(&mut content_ui);
                }
                Panel::Properties => properties(self, &mut content_ui),
                Panel::Components => self.components_panel(&mut content_ui),
                Panel::Messages => self.messages_panel(&mut content_ui),
                Panel::Changes => self.git_details_content(&mut content_ui),
            }
        }
        ui.allocate_rect(rect, Sense::hover());
    }
    fn dock_context(&mut self, ui: &mut egui::Ui, root: &Root, panel: Panel) {
        let panels = [panel];
        if ui
            .button(format!("Close '{}'", self.panel_title(panel)))
            .clicked()
        {
            self.close_panel(panel);
            ui.close();
        }
        ui.menu_button("Allow Dock", |ui| {
            let (mut horizontal, mut vertical) = self
                .settings
                .dock_layout
                .as_ref()
                .unwrap()
                .permissions(panel);
            if ui
                .add_enabled(
                    root.edge
                        .is_none_or(|e| !matches!(e, Side::Top | Side::Bottom)),
                    egui::Checkbox::new(&mut horizontal, "Horizontally"),
                )
                .changed()
            {
                self.settings
                    .dock_layout
                    .as_mut()
                    .unwrap()
                    .set_permissions(panel, horizontal, vertical);
            }
            if ui
                .add_enabled(
                    root.edge
                        .is_none_or(|e| !matches!(e, Side::Left | Side::Right)),
                    egui::Checkbox::new(&mut vertical, "Vertically"),
                )
                .changed()
            {
                self.settings
                    .dock_layout
                    .as_mut()
                    .unwrap()
                    .set_permissions(panel, horizontal, vertical);
            }
        });
        if root.edge.is_some() {
            if ui.button("Floating").clicked() {
                self.settings
                    .dock_layout
                    .as_mut()
                    .unwrap()
                    .move_panels(&panels, Target::Float(root.bounds));
                ui.close();
            }
            if ui.selectable_label(root.auto_hide, "Auto Hide").clicked() {
                self.settings
                    .dock_layout
                    .as_mut()
                    .unwrap()
                    .set_auto_hide(panel, !root.auto_hide);
                self.dock_runtime.popup = None;
                ui.close();
            }
        } else {
            if ui.button("Dock").clicked() {
                self.settings
                    .dock_layout
                    .as_mut()
                    .unwrap()
                    .move_panels(&panels, Target::Edge(panels[0].side()));
                ui.close();
            }
            if ui
                .button(
                    if ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)) {
                        "Restore"
                    } else {
                        "Maximize"
                    },
                )
                .clicked()
            {
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(
                    !ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)),
                ));
                ui.close();
            }
        }
    }
    fn drag_handle(&mut self, response: &egui::Response, panel: Panel, rect: Rect) {
        let scale = self.dock_scale(&response.ctx);
        if response.drag_started()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.dock_runtime.popup = None;
            self.dock_runtime.drag = Some(Drag {
                panels: vec![panel],
                source: response.ctx.viewport_id(),
                pointer: (p + viewport_origin(&response.ctx)) * self.dock_scale(&response.ctx),
                size: rect.size() * self.dock_scale(&response.ctx),
                offset: (response
                    .ctx
                    .input(|i| i.pointer.press_origin())
                    .unwrap_or(p)
                    - rect.min)
                    * scale,
                released: false,
                cancel: false,
                ctrl: false,
            });
        }
        self.update_dock_drag(&response.ctx);
    }
    fn dock_scale(&self, ctx: &egui::Context) -> f32 {
        ctx.pixels_per_point() / self.dock_runtime.main_scale
    }
    fn update_dock_drag(&mut self, ctx: &egui::Context) {
        let scale = self.dock_scale(ctx);
        if let Some(drag) = &mut self.dock_runtime.drag
            && drag.source == ctx.viewport_id()
        {
            ctx.input_mut(|i| {
                if let Some(p) = i.pointer.latest_pos() {
                    drag.pointer = (p + i
                        .viewport()
                        .inner_rect
                        .map(|r| r.min.to_vec2())
                        .unwrap_or_default())
                        * scale;
                }
                drag.released |= i.pointer.any_released();
                drag.ctrl = i.modifiers.ctrl;
                drag.cancel |= i.consume_key(egui::Modifiers::NONE, Key::Escape);
            });
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            ctx.request_repaint();
        }
    }

    fn dock_rail(&mut self, ctx: &egui::Context, side: Side, roots: &[Root]) {
        let id = Id::new(("autohide", side));
        let render = |ui: &mut egui::Ui| {
            for root in roots {
                for panel in root
                    .tree
                    .panels()
                    .into_iter()
                    .filter(|p| self.panel_open(*p))
                    .collect::<Vec<_>>()
                {
                    let vertical = matches!(side, Side::Left | Side::Right);
                    let size = if vertical {
                        vec2(24., self.panel_title(panel).len() as f32 * 7. + 14.)
                    } else {
                        vec2(self.panel_title(panel).len() as f32 * 7. + 14., 24.)
                    };
                    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                    #[cfg(test)]
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(Id::new(("dock_rail_rect", panel)), rect));
                    ui.painter().rect_filled(
                        rect.shrink(1.),
                        0.,
                        Color32::from_gray(if self.dock_runtime.popup == Some(root.id) {
                            80
                        } else {
                            60
                        }),
                    );
                    let galley = ui.painter().layout_no_wrap(
                        self.panel_title(panel).into(),
                        FontId::proportional(12.),
                        Color32::from_gray(230),
                    );
                    let mut text = egui::epaint::TextShape::new(
                        if vertical {
                            rect.left_bottom() + vec2(5., -6.)
                        } else {
                            rect.left_top() + vec2(6., 4.)
                        },
                        galley,
                        Color32::WHITE,
                    );
                    if vertical {
                        text.angle = -std::f32::consts::FRAC_PI_2;
                    }
                    ui.painter().add(text);
                    // Keep border buttons reachable while a foreground popup is open.
                    // They lie outside its painted clip, but egui's Area hit region can
                    // still cover them after content changes size.
                    let border_hover = ui.is_enabled()
                        && !egui::Popup::is_any_open(ui.ctx())
                        && ui.ctx().memory(|m| m.top_modal_layer().is_none())
                        && !ui.ctx().is_using_pointer()
                        && ui.input(|i| {
                            i.viewport().focused.unwrap_or(true)
                                && i.pointer
                                    .latest_pos()
                                    .is_some_and(|p| rect.intersect(ui.clip_rect()).contains(p))
                        });
                    let border_click = border_hover && ui.input(|i| i.pointer.primary_pressed());
                    if response.clicked() || response.hovered() || border_hover {
                        if self.dock_runtime.popup != Some(root.id) {
                            self.dock_runtime.popup_click = false;
                        }
                        self.dock_runtime.popup = Some(root.id);
                        self.dock_runtime.popup_panel = Some(panel);
                        self.dock_runtime.popup_time = ui.input(|i| i.time);
                        self.settings.dock_layout.as_mut().unwrap().select(panel);
                        self.dock_runtime.popup_click |= response.clicked() || border_click;
                    }
                    self.drag_handle(
                        &response,
                        panel,
                        Rect::from_min_size(rect.min, vec2(root.extent, 500.)),
                    );
                }
            }
        };
        let frame = egui::Frame::NONE.fill(Color32::from_gray(45));
        match side {
            Side::Left | Side::Right => {
                let panel = if side == Side::Left {
                    egui::SidePanel::left(id)
                } else {
                    egui::SidePanel::right(id)
                };
                panel
                    .exact_width(26.)
                    .resizable(false)
                    .frame(frame)
                    .show(ctx, render);
            }
            _ => {
                let panel = if side == Side::Top {
                    egui::TopBottomPanel::top(id)
                } else {
                    egui::TopBottomPanel::bottom(id)
                };
                panel
                    .exact_height(26.)
                    .resizable(false)
                    .frame(frame)
                    .show(ctx, |ui| {
                        ui.horizontal(render);
                    });
            }
        }
    }
    pub(super) fn finish_docks(&mut self, ctx: &egui::Context) {
        self.finish_docks_with_properties(ctx, |app, ui| app.normal_properties(ui));
    }
    pub(super) fn finish_docks_with_properties(
        &mut self,
        ctx: &egui::Context,
        mut properties: impl FnMut(&mut Self, &mut egui::Ui),
    ) {
        if self.dock_runtime.finished {
            return;
        }
        self.dock_runtime.finished = true;
        self.ensure_docks();
        let roots = self.settings.dock_layout.as_ref().unwrap().roots.clone();
        for root in &roots {
            if root.edge.is_none()
                && self.active_in_node(&root.tree).is_some()
                && !self.dock_runtime.floating_hidden
            {
                let panel = self.active_in_node(&root.tree).unwrap();
                let bounds = root.bounds;
                let viewport = egui::ViewportId::from_hash_of(("floating_dock", root.id));
                ctx.show_viewport_immediate(
                    viewport,
                    egui::ViewportBuilder::default()
                        .with_title(self.panel_title(panel))
                        .with_decorations(false)
                        .with_resizable(true)
                        .with_inner_size([bounds[2], bounds[3]])
                        .with_min_inner_size([160., 100.])
                        .with_position([bounds[0], bounds[1]]),
                    |ctx, class| {
                        if class == egui::ViewportClass::Embedded {
                            egui::Window::new(self.panel_title(panel))
                                .id(Id::new(("embedded_dock", root.id)))
                                .default_size([bounds[2], bounds[3]])
                                .show_stable(ctx, |ui| {
                                    self.dock_node(ui, root, &root.tree, &mut properties)
                                });
                        } else {
                            if ctx.input(|i| i.viewport().close_requested()) {
                                for panel in root.tree.panels() {
                                    self.close_panel(panel);
                                }
                            }
                            let main_scale = self.dock_runtime.main_scale;
                            let state = ctx.input(|i| {
                                (
                                    i.viewport().inner_rect,
                                    i.viewport().outer_rect,
                                    i.viewport().maximized.unwrap_or(false),
                                )
                            });
                            if !state.2
                                && let Some(rect) = state.0
                                && let Some(r) = self.root_mut(root.id)
                            {
                                let outer = state.1.unwrap_or(rect);
                                let scale = ctx.pixels_per_point() / main_scale;
                                r.bounds = [
                                    outer.left() * scale,
                                    outer.top() * scale,
                                    rect.width() * scale,
                                    rect.height() * scale,
                                ];
                            }
                            self.update_dock_drag(ctx);
                            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::F4)) {
                                self.dock_runtime.floating_hidden =
                                    !self.dock_runtime.floating_hidden;
                                ctx.request_repaint_of(egui::ViewportId::ROOT);
                            }
                            egui::CentralPanel::default()
                                .frame(egui::Frame::NONE.fill(theme::PANEL).inner_margin(3))
                                .show(ctx, |ui| {
                                    self.dock_node(ui, root, &root.tree, &mut properties)
                                });
                            floating_resize(ctx);
                        }
                        self.dock_overlay(ctx);
                    },
                );
            }
        }
        if let Some(root) = roots.iter().find(|r| {
            Some(r.id) == self.dock_runtime.popup
                && r.auto_hide
                && self.active_in_node(&r.tree).is_some()
        }) {
            let available = self.dock_runtime.main_rect;
            let extent = root.extent;
            let rect = match root.edge.unwrap_or(Side::Left) {
                Side::Left => Rect::from_min_max(
                    available.min + vec2(26., 0.),
                    pos2(available.left() + 26. + extent, available.bottom()),
                ),
                Side::Right => Rect::from_min_max(
                    pos2(available.right() - 26. - extent, available.top()),
                    available.max - vec2(26., 0.),
                ),
                Side::Top => Rect::from_min_size(
                    available.min + vec2(0., 26.),
                    vec2(available.width(), extent),
                ),
                Side::Bottom => Rect::from_min_max(
                    pos2(available.left(), available.bottom() - 26. - extent),
                    available.max - vec2(0., 26.),
                ),
            };
            let panel = self
                .dock_runtime
                .popup_panel
                .filter(|p| root.tree.panels().contains(p) && self.panel_open(*p))
                .or_else(|| self.active_in_node(&root.tree))
                .unwrap();
            let group = panel_group_id(&root.tree, panel).unwrap();
            let popup_node = Node::Group {
                id: group,
                tabs: vec![panel],
                active: panel,
            };
            egui::Area::new(Id::new(("popup_dock", root.id)))
                .order(egui::Order::Foreground)
                .fixed_pos(rect.min)
                .show(ctx, |ui| {
                    ui.set_min_size(rect.size());
                    ui.set_max_size(rect.size());
                    ui.set_clip_rect(rect);
                    ui.painter().rect_filled(rect, 0., theme::PANEL);
                    ui.painter().rect_stroke(
                        rect,
                        0.,
                        Stroke::new(1., Color32::from_gray(95)),
                        egui::StrokeKind::Inside,
                    );
                    let mut content = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .id_salt((root.id, "popup_content")),
                    );
                    content.set_clip_rect(rect);
                    self.dock_node(&mut content, root, &popup_node, &mut properties);
                    let side = root.edge.unwrap_or(Side::Left);
                    let handle = match side {
                        Side::Left => {
                            Rect::from_min_max(pos2(rect.right() - 4., rect.top()), rect.max)
                        }
                        Side::Right => {
                            Rect::from_min_max(rect.min, pos2(rect.left() + 4., rect.bottom()))
                        }
                        Side::Top => {
                            Rect::from_min_max(pos2(rect.left(), rect.bottom() - 4.), rect.max)
                        }
                        Side::Bottom => {
                            Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + 4.))
                        }
                    };
                    let response = ui
                        .interact(handle, Id::new((root.id, "popup_resize")), Sense::drag())
                        .on_hover_cursor(if matches!(side, Side::Left | Side::Right) {
                            egui::CursorIcon::ResizeHorizontal
                        } else {
                            egui::CursorIcon::ResizeVertical
                        });
                    if response.dragged()
                        && let Some(p) = response.interact_pointer_pos()
                    {
                        let extent = match side {
                            Side::Left => p.x - rect.left(),
                            Side::Right => rect.right() - p.x,
                            Side::Top => p.y - rect.top(),
                            Side::Bottom => rect.bottom() - p.y,
                        };
                        let max = if matches!(side, Side::Left | Side::Right) {
                            available.width() - 60.
                        } else {
                            available.height() - 60.
                        };
                        if let Some(r) = self.root_mut(root.id) {
                            r.extent = extent.clamp(100., max.max(100.));
                        }
                    }
                    ui.allocate_rect(rect, Sense::hover());
                });
            let (inside, press, time) = ctx.input(|i| {
                (
                    i.pointer
                        .latest_pos()
                        .is_some_and(|p| rect.expand(6.).contains(p)),
                    i.pointer.any_pressed(),
                    i.time,
                )
            });
            if inside {
                self.dock_runtime.popup_time = time;
            }
            if !inside
                && ((press && self.dock_runtime.popup_click)
                    || (!self.dock_runtime.popup_click
                        && time - self.dock_runtime.popup_time > 0.5))
            {
                self.dock_runtime.popup = None;
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.dock_overlay(ctx);
        if self.dock_runtime.drag.as_ref().is_some_and(|d| d.cancel) {
            self.dock_runtime.drag = None;
        }
        if self.dock_runtime.drag.as_ref().is_some_and(|d| d.released) {
            let target = self.dock_drop_target();
            let drag = self.dock_runtime.drag.take().unwrap();
            let target = target.map(|t| t.0).unwrap_or_else(|| {
                Target::Float(self.snap_floating_bounds(
                    [
                        drag.pointer.x - drag.offset.x,
                        drag.pointer.y - drag.offset.y,
                        drag.size.x.max(220.),
                        drag.size.y.max(160.),
                    ],
                    drag.ctrl,
                ))
            });
            self.settings
                .dock_layout
                .as_mut()
                .unwrap()
                .move_panels(&drag.panels, target);
            ctx.request_repaint();
        }
    }
    fn snap_floating_bounds(&self, mut bounds: [f32; 4], disabled: bool) -> [f32; 4] {
        if disabled {
            return bounds;
        }
        if let Some(layout) = &self.settings.dock_layout {
            for root in layout.roots.iter().filter(|r| r.edge.is_none()) {
                for x in [root.bounds[0] - bounds[2], root.bounds[0] + root.bounds[2]] {
                    if (bounds[0] - x).abs() < 12. {
                        bounds[0] = x;
                    }
                }
                for y in [root.bounds[1] - bounds[3], root.bounds[1] + root.bounds[3]] {
                    if (bounds[1] - y).abs() < 12. {
                        bounds[1] = y;
                    }
                }
            }
        }
        bounds
    }
    fn dock_drop_target(&self) -> Option<(Target, Rect)> {
        let drag = self.dock_runtime.drag.as_ref()?;
        if drag.ctrl {
            return None;
        }
        let permissions = self
            .settings
            .dock_layout
            .as_ref()?
            .permissions(drag.panels[0]);
        let point = drag.pointer;
        let main = self
            .dock_runtime
            .main_rect
            .translate(self.dock_runtime.main_origin);
        // A narrow application border has precedence over nested group targets.
        if main.expand(8.).contains(point) {
            for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
                if !allowed(permissions, side) {
                    continue;
                }
                let distance = match side {
                    Side::Left => (point.x - main.left()).abs(),
                    Side::Right => (point.x - main.right()).abs(),
                    Side::Top => (point.y - main.top()).abs(),
                    Side::Bottom => (point.y - main.bottom()).abs(),
                };
                if distance < 18. {
                    return Some((
                        Target::Edge(side),
                        side_rect(
                            main,
                            side,
                            if matches!(side, Side::Left | Side::Right) {
                                334.
                            } else {
                                220.
                            },
                        ),
                    ));
                }
            }
        }
        for group in self.dock_runtime.groups.iter().rev() {
            if !group.rect.contains(point) {
                continue;
            }
            let moving_all = self
                .settings
                .dock_layout
                .as_ref()?
                .group(group.id)?
                .panels()
                .iter()
                .all(|p| drag.panels.contains(p));
            if moving_all {
                continue;
            }
            let r = group.rect;
            for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
                if allowed(permissions, side) && arrow_rect(r, side).contains(point) {
                    return Some((
                        Target::Split(group.id, side),
                        side_rect(
                            r,
                            side,
                            if matches!(side, Side::Left | Side::Right) {
                                r.width() / 2.
                            } else {
                                r.height() / 2.
                            },
                        ),
                    ));
                }
            }
            let index = group
                .tabs
                .iter()
                .filter(|(p, r)| !drag.panels.contains(p) && point.x > r.center().x)
                .count();
            let node = self.settings.dock_layout.as_ref()?.group(group.id)?;
            let Node::Group { tabs, .. } = node else {
                continue;
            };
            // Map visible insertion position to the stored tab list, preserving hidden tabs.
            let visible: Vec<_> = tabs
                .iter()
                .filter(|p| !drag.panels.contains(p) && self.panel_open(**p))
                .collect();
            let stored: Vec<_> = tabs.iter().filter(|p| !drag.panels.contains(p)).collect();
            let index = visible
                .get(index)
                .and_then(|p| stored.iter().position(|q| p == q))
                .unwrap_or(stored.len());
            return Some((Target::Tab(group.id, index), r));
        }
        None
    }
    fn dock_overlay(&self, ctx: &egui::Context) {
        let Some(drag) = &self.dock_runtime.drag else {
            return;
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            Id::new("docking_overlay"),
        ));
        let scale = self.dock_scale(ctx);
        let origin = viewport_origin(ctx);
        let target = self.dock_drop_target();
        if let Some((_, rect)) = &target {
            painter.rect_filled(
                (*rect / scale).translate(-origin),
                0.,
                Color32::from_rgba_unmultiplied(58, 139, 212, 85),
            );
            painter.rect_stroke(
                (*rect / scale).translate(-origin),
                0.,
                Stroke::new(2., Color32::from_rgb(93, 173, 235)),
                egui::StrokeKind::Inside,
            );
        }
        if !drag.ctrl {
            for group in &self.dock_runtime.groups {
                if group.viewport != ctx.viewport_id() || !group.rect.contains(drag.pointer) {
                    continue;
                }
                for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
                    let rect = (arrow_rect(group.rect, side) / scale).translate(-origin);
                    painter.rect_filled(rect, 2., Color32::from_rgb(57, 83, 104));
                    painter.rect_stroke(
                        rect,
                        2.,
                        Stroke::new(1., Color32::from_rgb(160, 190, 214)),
                        egui::StrokeKind::Inside,
                    );
                    draw_direction(&painter, rect, side);
                }
                if let Some((Target::Tab(_, index), _)) = &target {
                    let x = group
                        .tabs
                        .get(*index)
                        .map(|(_, r)| r.left())
                        .or_else(|| group.tabs.last().map(|(_, r)| r.right()))
                        .unwrap_or(group.rect.left());
                    let p = pos2(x, group.rect.bottom() - 25.) / scale - origin;
                    painter.line_segment(
                        [p, p + vec2(0., 24.)],
                        Stroke::new(3., Color32::from_rgb(115, 191, 248)),
                    );
                }
            }
        }
        if target.is_none() {
            let rect = Rect::from_min_size(
                (drag.pointer - drag.offset) / scale - origin,
                drag.size / scale,
            );
            painter.rect_stroke(
                rect,
                0.,
                Stroke::new(2., Color32::from_rgb(100, 170, 220)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                rect.min + vec2(6., 6.),
                Align2::LEFT_TOP,
                self.panel_title(drag.panels[0]),
                FontId::proportional(12.),
                Color32::WHITE,
            );
        }
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

fn panel_group_id(node: &Node, panel: Panel) -> Option<u64> {
    match node {
        Node::Group { id, tabs, .. } => tabs.contains(&panel).then_some(*id),
        Node::Split { first, second, .. } => {
            panel_group_id(first, panel).or_else(|| panel_group_id(second, panel))
        }
    }
}
fn viewport_origin(ctx: &egui::Context) -> Vec2 {
    ctx.input(|i| {
        i.viewport()
            .inner_rect
            .map(|r| r.min.to_vec2())
            .unwrap_or_default()
    })
}
fn allowed(permissions: (bool, bool), side: Side) -> bool {
    if matches!(side, Side::Left | Side::Right) {
        permissions.1
    } else {
        permissions.0
    }
}
fn side_rect(rect: Rect, side: Side, extent: f32) -> Rect {
    match side {
        Side::Left => Rect::from_min_max(
            rect.min,
            pos2(rect.left() + extent.min(rect.width()), rect.bottom()),
        ),
        Side::Right => Rect::from_min_max(
            pos2(rect.right() - extent.min(rect.width()), rect.top()),
            rect.max,
        ),
        Side::Top => Rect::from_min_max(
            rect.min,
            pos2(rect.right(), rect.top() + extent.min(rect.height())),
        ),
        Side::Bottom => Rect::from_min_max(
            pos2(rect.left(), rect.bottom() - extent.min(rect.height())),
            rect.max,
        ),
    }
}
fn arrow_rect(rect: Rect, side: Side) -> Rect {
    Rect::from_center_size(
        match side {
            Side::Left => pos2(rect.left() + 17., rect.center().y),
            Side::Right => pos2(rect.right() - 17., rect.center().y),
            Side::Top => pos2(rect.center().x, rect.top() + 17.),
            Side::Bottom => pos2(rect.center().x, rect.bottom() - 45.),
        },
        vec2(24., 24.),
    )
}
fn choose_after_close(node: &mut Node, panel: Panel) {
    match node {
        Node::Group { tabs, active, .. } if *active == panel => {
            if let Some(i) = tabs.iter().position(|p| *p == panel)
                && let Some(next) = tabs[..i].last().or_else(|| tabs.get(i + 1))
            {
                *active = *next;
            }
        }
        Node::Split { first, second, .. } => {
            choose_after_close(first, panel);
            choose_after_close(second, panel);
        }
        _ => {}
    }
}
fn set_fraction(node: &mut Node, panels: &[Panel], value: f32) {
    if let Node::Split {
        fraction,
        first,
        second,
        ..
    } = node
    {
        if first.panels() == panels {
            *fraction = value;
        } else {
            set_fraction(first, panels, value);
            set_fraction(second, panels, value);
        }
    }
}
fn draw_direction(painter: &egui::Painter, rect: Rect, side: Side) {
    let c = rect.center();
    let direction = match side {
        Side::Left => vec2(-1., 0.),
        Side::Right => vec2(1., 0.),
        Side::Top => vec2(0., -1.),
        Side::Bottom => vec2(0., 1.),
    };
    let perpendicular = vec2(-direction.y, direction.x);
    painter.add(egui::Shape::convex_polygon(
        vec![
            c + direction * 3.,
            c - direction * 2. - perpendicular * 4.,
            c - direction * 2. + perpendicular * 4.,
        ],
        Color32::from_gray(235),
        Stroke::NONE,
    ));
}
fn draw_pin(p: &egui::Painter, r: Rect, popup: bool) {
    let c = r.center();
    let s = Stroke::new(1.2, Color32::from_gray(235));
    let transform = |v: Vec2| c + if popup { vec2(-v.y, v.x) } else { v };
    p.line_segment([transform(vec2(-4., -4.)), transform(vec2(4., -4.))], s);
    for x in [-2., 2.] {
        p.line_segment([transform(vec2(x, -4.)), transform(vec2(x, 1.))], s);
    }
    p.line_segment([transform(vec2(-4., 2.)), transform(vec2(4., 2.))], s);
    p.line_segment([transform(vec2(0., 2.)), transform(vec2(0., 7.))], s);
}
fn floating_resize(ctx: &egui::Context) {
    let rect = ctx.screen_rect();
    let Some(p) = ctx.pointer_latest_pos() else {
        return;
    };
    let left = p.x < rect.left() + 4.;
    let right = p.x > rect.right() - 4.;
    let top = p.y < rect.top() + 4.;
    let bottom = p.y > rect.bottom() - 4.;
    let (direction, cursor) = match (left, right, top, bottom) {
        (true, _, true, _) => (
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (_, true, true, _) => (
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (true, _, _, true) => (
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (_, true, _, true) => (
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
        (true, _, _, _) => (
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (_, true, _, _) => (
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (_, _, true, _) => (
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (_, _, _, true) => (
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
    }
}

#[cfg(test)]
#[path = "dock_tests.rs"]
mod tests;
