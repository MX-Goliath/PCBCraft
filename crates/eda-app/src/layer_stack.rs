use super::*;
use eda_core::layer_stack::{Impedance, LayerKind, Stack, ViaKind, ViaType};
use eda_core::pcb::{BOTTOM, TOP};
#[path = "layer_stack_ui.rs"]
mod stack_ui;

pub(super) struct Manager {
    pub(super) session: Uuid,
    pub(super) visible: bool,
    revision: u64,
    stack: Stack,
    original: Stack,
    tab: usize,
    selected: usize,
    error: String,
    mils: bool,
    confirm_close: bool,
    undo: Vec<Stack>,
    redo: Vec<Stack>,
    symmetry_dialog: bool,
}
impl Manager {
    pub(super) fn dirty(&self) -> bool {
        self.stack != self.original
    }
    fn commit(&mut self, d: &mut OpenDocument) -> Result<(), String> {
        if d.session_id != self.session || d.history.revision != self.revision {
            return Err(
                "Board changed while manager was open; close and reopen the manager".into(),
            );
        }
        if self.stack != self.original {
            let board = d.model.pcb.as_ref().ok_or("Not a PCB document")?;
            let previous: std::collections::HashSet<_> =
                board.layers.iter().map(|l| l.id).collect();
            let next = self.stack.apply(board)?;
            pcb_editor::commit(d, next)?;
            let board = d.model.pcb.as_ref().unwrap();
            d.view
                .pcb
                .visible
                .retain(|id| board.layers.iter().any(|l| l.id == *id));
            for l in &board.layers {
                if !previous.contains(&l.id) {
                    d.view.pcb.visible.insert(l.id);
                }
            }
            if !board.layers.iter().any(|l| l.id == d.view.pcb.active_layer) {
                d.view.pcb.active_layer = TOP;
            }
        }
        self.revision = d.history.revision;
        self.original = self.stack.clone();
        self.error.clear();
        self.undo.clear();
        self.redo.clear();
        Ok(())
    }
    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.stack, previous));
            self.error.clear();
        }
    }
    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.stack, next));
            self.error.clear();
        }
    }
}
impl PcbCraft {
    pub(super) fn layer_stack_menus(&mut self, ctx: &egui::Context) {
        let undo = self
            .layer_stack
            .as_ref()
            .is_some_and(|m| !m.undo.is_empty());
        let redo = self
            .layer_stack
            .as_ref()
            .is_some_and(|m| !m.redo.is_empty());
        egui::TopBottomPanel::top("menu")
            .exact_height(29.0)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        self.action(ui, "Save", "Ctrl+S", Command::Save, true);
                        self.action(ui, "Close", "Ctrl+W", Command::Close, true);
                    });
                    ui.menu_button("Edit", |ui| {
                        self.action(ui, "Undo", "Ctrl+Z", Command::Undo, undo);
                        self.action(ui, "Redo", "Ctrl+Y", Command::Redo, redo);
                    });
                    ui.menu_button("View", |ui| self.panels_menu(ui));
                    ui.menu_button("Project", |ui| {
                        ui.add_enabled(false, egui::Button::new("Not implemented"));
                    });
                    ui.menu_button("Tools", |ui| {
                        self.action(
                            ui,
                            "Measurement Units",
                            "Ctrl+Q",
                            Command::ToggleUnits,
                            true,
                        );
                        for label in ["Material Library", "Layerstack Visualizer", "Features"] {
                            ui.add_enabled(false, egui::Button::new(label));
                        }
                    });
                    for label in ["Window", "Help"] {
                        ui.menu_button(label, |ui| {
                            ui.add_enabled(false, egui::Button::new("Not implemented"));
                        });
                    }
                });
            });
    }
    pub(super) fn layer_stack_active(&self) -> bool {
        self.layer_stack.as_ref().is_some_and(|m| m.visible)
    }
    pub(super) fn layer_stack_command(&mut self, command: Command) {
        let Some(mut manager) = self.layer_stack.take() else {
            return;
        };
        let mut close = false;
        match command {
            Command::Save => {
                let result = self
                    .documents
                    .iter_mut()
                    .find(|d| d.session_id == manager.session)
                    .ok_or_else(|| "Board document was closed".to_string())
                    .and_then(|d| manager.commit(d));
                if let Err(e) = result {
                    manager.error = e;
                }
            }
            Command::Undo => manager.undo(),
            Command::Redo => manager.redo(),
            Command::ToggleUnits => manager.mils = !manager.mils,
            Command::Close => {
                if manager.stack == manager.original {
                    close = true;
                } else {
                    manager.confirm_close = true;
                }
            }
            _ => self.message(
                true,
                "Apply or close Layer Stack Manager before another command",
            ),
        }
        if !close {
            self.layer_stack = Some(manager);
        }
    }
    pub(super) fn open_layer_stack(&mut self) {
        if let Some(manager) = &mut self.layer_stack {
            manager.visible = true;
            return;
        }
        let Some(i) = self.active else { return };
        let d = &self.documents[i];
        let Some(b) = &d.model.pcb else { return };
        if d.pcb_editor.has_pending() || d.pcb_editor.properties_dirty(b, d.history.revision) {
            self.message(
                true,
                "Finish or cancel PCB edits before opening Layer Stack Manager",
            );
            return;
        }
        let stack = Stack::from_board(b);
        self.layer_stack = Some(Manager {
            visible: true,
            session: d.session_id,
            revision: d.history.revision,
            original: stack.clone(),
            stack,
            tab: 0,
            selected: 0,
            error: String::new(),
            mils: false,
            confirm_close: false,
            undo: vec![],
            redo: vec![],
            symmetry_dialog: false,
        });
    }
    pub(super) fn layer_stack_workspace(&mut self, ctx: &egui::Context) {
        let Some(mut m) = self.layer_stack.take() else {
            return;
        };
        let mut apply = false;
        let mut close = false;
        let mut use_via = None;
        let mut use_width = None;
        if !ctx.wants_keyboard_input() && !m.confirm_close && !m.symmetry_dialog {
            ctx.input_mut(|input| {
                if input.consume_key(egui::Modifiers::CTRL, Key::S) {
                    apply = true;
                }
                if input.consume_key(egui::Modifiers::CTRL, Key::Q) {
                    m.mils = !m.mils;
                }
                if input.consume_key(egui::Modifiers::CTRL, Key::Z) {
                    m.undo();
                }
                if input.consume_key(egui::Modifiers::CTRL, Key::Y) {
                    m.redo();
                }
                if input.consume_key(egui::Modifiers::CTRL, Key::W) {
                    if m.stack == m.original {
                        close = true;
                    } else {
                        m.confirm_close = true;
                    }
                }
            });
        }
        let before_frame = m.stack.clone();
        let before_history = (m.undo.len(), m.redo.len());
        {
            let mut properties = |_app: &mut Self, ui: &mut egui::Ui| {
                ui.label("Layer Stack Manager");
                stack_ui::search(ui);
                egui::ScrollArea::vertical()
                    .id_salt("stack_properties_scroll")
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.add_space(22.0);
                        match m.tab {
                            0 => stack_ui::layer_properties(ui, &mut m),
                            1 => stack_ui::via_properties(ui, &mut m, &mut apply, &mut use_via),
                            _ => stack_ui::impedance_properties(
                                ui,
                                &mut m,
                                &mut apply,
                                &mut use_width,
                            ),
                        }
                        stack_ui::board_properties(ui, &mut m);
                        if m.tab != 1 {
                            stack_ui::other_properties(ui);
                        }
                    });
            };
            self.left_dock_with_properties(ctx, &mut properties);
            self.right_dock_with_properties(ctx, &mut properties);
            self.finish_docks_with_properties(ctx, &mut properties);
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_gray(58))
                    .inner_margin(egui::Margin::symmetric(5, 0)),
            )
            .show(ctx, |ui| {
                let name = self
                    .documents
                    .iter()
                    .find(|d| d.session_id == m.session)
                    .map(|d| d.model.name.as_str())
                    .unwrap_or("PCB");
                ui.horizontal(|ui| {
                    ui.set_height(26.0);
                    if ui.button(name).clicked() {
                        m.visible = false;
                        self.active = self
                            .documents
                            .iter()
                            .position(|d| d.session_id == m.session);
                    }
                    ui.add(
                        egui::Button::new(format!(
                            "{name} [Stackup]{}",
                            if m.stack != m.original { " *" } else { "" }
                        ))
                        .selected(true),
                    );
                    ui.add_enabled(false, egui::Button::new("Home Page"));
                });
                ui.separator();
                stack_ui::toolbar(ui, &mut m, &mut apply, &mut close);
                if !m.error.is_empty() {
                    ui.colored_label(Color32::from_rgb(245, 150, 110), &m.error);
                }
                if let Err(e) = m.stack.validate() {
                    ui.colored_label(Color32::YELLOW, e);
                }
                ui.separator();
                let height = (ui.available_height() - 24.0).max(150.0);
                egui::ScrollArea::both()
                    .max_height(height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        if m.tab == 0 {
                            stack_ui::table(ui, &mut m);
                        } else if m.tab == 1 {
                            stack_ui::via_table(ui, &mut m);
                        } else {
                            preview(ui, &m.stack, m.selected, m.tab);
                            egui::Grid::new(("stack_table", m.tab))
                                .striped(true)
                                .min_col_width(70.0)
                                .show(ui, |ui| match m.tab {
                                    0 => {
                                        for s in
                                            ["Layer", "Type", "Material", "Thickness", "Dk", "Df"]
                                        {
                                            ui.strong(s);
                                        }
                                        ui.end_row();
                                        for (i, l) in m.stack.layers.iter_mut().enumerate() {
                                            if ui
                                                .selectable_label(m.selected == i, &l.name)
                                                .clicked()
                                            {
                                                m.selected = i;
                                            }
                                            ui.label(format!("{:?}", l.kind));
                                            ui.label(&l.material);
                                            length(ui, &mut l.thickness, m.mils);
                                            ui.label(format!("{:.3}", l.er));
                                            ui.label(format!("{:.3}", l.loss_tangent));
                                            ui.end_row();
                                        }
                                    }
                                    1 => {
                                        let names = copper_names(&m.stack);
                                        for s in [
                                            "Name",
                                            "Type",
                                            "First Layer",
                                            "Last Layer",
                                            "Diameter",
                                            "Hole Size",
                                        ] {
                                            ui.strong(s);
                                        }
                                        ui.end_row();
                                        for (i, v) in m.stack.vias.iter().enumerate() {
                                            if ui
                                                .selectable_label(m.selected == i, &v.name)
                                                .clicked()
                                            {
                                                m.selected = i;
                                            }
                                            ui.label(format!("{:?}", v.kind));
                                            ui.label(layer_name(v.start, &names));
                                            ui.label(layer_name(v.end, &names));
                                            ui.label(dimension(v.diameter, m.mils));
                                            ui.label(dimension(v.hole, m.mils));
                                            ui.end_row();
                                        }
                                    }
                                    _ => {
                                        for s in [
                                            "Name",
                                            "Type",
                                            "Target Ω",
                                            "Tolerance %",
                                            "Width",
                                            "Gap",
                                            "Estimated Ω",
                                        ] {
                                            ui.strong(s);
                                        }
                                        ui.end_row();
                                        for (i, p) in m.stack.impedances.iter().enumerate() {
                                            if ui
                                                .selectable_label(m.selected == i, &p.name)
                                                .clicked()
                                            {
                                                m.selected = i;
                                            }
                                            ui.label(if p.differential {
                                                "Differential"
                                            } else {
                                                "Single"
                                            });
                                            ui.label(format!("{:.2}", p.target));
                                            ui.label(format!("{:.1}", p.tolerance));
                                            ui.label(dimension(p.width, m.mils));
                                            ui.label(dimension(p.gap, m.mils));
                                            ui.label(
                                                m.stack
                                                    .estimate(p)
                                                    .map(|z| format!("{z:.2}"))
                                                    .unwrap_or_else(|_| "—".into()),
                                            );
                                            ui.end_row();
                                        }
                                    }
                                });
                        }
                    });
                ui.separator();

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        for (tab, name) in [(0, "Stackup"), (2, "Impedance"), (1, "Via Types")] {
                            if ui.selectable_label(m.tab == tab, name).clicked() {
                                m.tab = tab;
                                m.selected = 0;
                                m.error.clear();
                            }
                        }
                    });
                });
            });
        if m.symmetry_dialog {
            egui::Window::new("Stack is not symmetric")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Choose which half supplies the paired layer properties.");
                    for (label, top_down) in [
                        ("Mirror top half down", true),
                        ("Mirror bottom half up", false),
                    ] {
                        if ui.button(label).clicked() {
                            match m.stack.mirror_half(top_down) {
                                Ok(()) => m.symmetry_dialog = false,
                                Err(e) => m.error = e,
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        m.symmetry_dialog = false;
                    }
                });
        }
        // Direct cell/property edits also maintain the physical partner. Structural
        // commands already validate their complete result atomically in eda-core.
        if before_frame.symmetric
            && m.stack.symmetric
            && before_frame.layers.len() == m.stack.layers.len()
        {
            let changed = before_frame
                .layers
                .iter()
                .zip(&m.stack.layers)
                .position(|(a, b)| a != b);
            if let Some(index) = changed {
                let layer = m.stack.layers[index].clone();
                if before_frame.layers[index].id == layer.id {
                    let mut edited = m.stack.clone();
                    if let Err(e) = edited.edit_layer(index, layer) {
                        m.stack = before_frame.clone();
                        m.error = e;
                    } else {
                        m.stack = edited;
                    }
                }
            }
        }
        if m.stack != before_frame && before_history == (m.undo.len(), m.redo.len()) {
            m.undo.push(before_frame);
            if m.undo.len() > 128 {
                m.undo.remove(0);
            }
            m.redo.clear();
        }
        if m.confirm_close {
            egui::Window::new("Unapplied stack changes")
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Apply changes before closing?");
                    ui.horizontal(|ui| {
                        if ui.button("Apply and close").clicked() {
                            apply = true;
                            close = true;
                        }
                        if ui.button("Discard").clicked() {
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            m.confirm_close = false;
                        }
                    });
                });
        }
        if apply {
            let result = self
                .documents
                .iter_mut()
                .find(|d| d.session_id == m.session)
                .ok_or("Board document was closed".to_string())
                .and_then(|d| {
                    m.commit(d)?;
                    if let Some(v) = use_via {
                        d.pcb_editor.via_from = v.start;
                        d.pcb_editor.via_to = v.end;
                        d.pcb_editor.diameter = v.diameter;
                        d.pcb_editor.drill = v.hole;
                    }
                    if let Some((layer, width)) = use_width {
                        d.pcb_editor.width = width;
                        d.view.pcb.active_layer = layer;
                    }
                    Ok(())
                });
            if let Err(e) = result {
                m.error = e;
                close = false;
            }
        }
        if !close {
            self.layer_stack = Some(m);
        }
    }
}
fn dimension(n: i64, mils: bool) -> String {
    let value = format!("{:.5}", n as f64 / if mils { 25_400.0 } else { 1e6 });
    format!(
        "{}{}",
        value.trim_end_matches('0').trim_end_matches('.'),
        if mils { "mil" } else { "mm" }
    )
}
fn length(ui: &mut egui::Ui, n: &mut i64, mils: bool) {
    let unit = if mils { 25_400.0 } else { 1e6 };
    let mut v = *n as f64 / unit;
    if ui
        .add(
            egui::DragValue::new(&mut v)
                .speed(if mils { 0.1 } else { 0.001 })
                .range(0.0001..=1000.0)
                .max_decimals(5)
                .suffix(if mils { " mil" } else { " mm" }),
        )
        .changed()
    {
        *n = (v * unit).round() as i64;
    }
}
fn copper_names(s: &Stack) -> Vec<(u16, String)> {
    s.copper()
        .iter()
        .map(|l| (l.copper_id.unwrap(), l.name.clone()))
        .collect()
}
fn layer_name(id: u16, names: &[(u16, String)]) -> String {
    names
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| format!("Missing layer {id}"))
}
fn preview(ui: &mut egui::Ui, stack: &Stack, selected: usize, tab: usize) {
    ui.label("Stack cross-section (thickness not to scale)");
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 150.0),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    let h = (140.0 / stack.layers.len().max(1) as f32).min(18.0);
    let mut y = rect.top();
    for (i, l) in stack.layers.iter().enumerate() {
        let color = if l.kind.copper() {
            Color32::from_rgb(180, 125, 50)
        } else if l.kind == LayerKind::SolderMask {
            Color32::from_rgb(45, 110, 65)
        } else {
            Color32::from_rgb(115, 125, 135)
        };
        let r = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 150.0, y),
            egui::vec2((rect.width() - 170.0).max(10.0), h - 1.0),
        );
        painter.rect_filled(r, 0.0, color);
        if tab == 0 && i == selected {
            painter.rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, Color32::WHITE),
                egui::StrokeKind::Inside,
            );
        }
        painter.text(
            egui::pos2(rect.left(), y),
            egui::Align2::LEFT_TOP,
            &l.name,
            egui::FontId::proportional(h.min(12.0)),
            Color32::LIGHT_GRAY,
        );
        y += h;
    }
    if tab == 1
        && let Some(v) = stack.vias.get(selected)
    {
        let a = stack
            .layers
            .iter()
            .position(|l| l.copper_id == Some(v.start));
        let b = stack.layers.iter().position(|l| l.copper_id == Some(v.end));
        if let (Some(a), Some(b)) = (a, b) {
            let x = rect.center().x;
            painter.line_segment(
                [
                    egui::pos2(x, rect.top() + a as f32 * h),
                    egui::pos2(x, rect.top() + (b + 1) as f32 * h),
                ],
                egui::Stroke::new(10.0, Color32::GOLD),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> OpenDocument {
        let mut d = Document::new(DocumentKind::Board, "stack");
        d.pcb = Some(eda_core::pcb::Board::default());
        OpenDocument::new(d, None)
    }
    fn manager(d: &OpenDocument) -> Manager {
        let stack = Stack::from_board(d.model.pcb.as_ref().unwrap());
        Manager {
            visible: true,
            session: d.session_id,
            revision: d.history.revision,
            original: stack.clone(),
            stack,
            tab: 0,
            selected: 0,
            error: String::new(),
            mils: false,
            confirm_close: false,
            undo: vec![],
            redo: vec![],
            symmetry_dialog: false,
        }
    }
    #[test]
    fn panel_navigation_preserves_pending_stack_draft() {
        let mut app = crate::app::schematic_editor::tests::app();
        let mut d = document();
        let path = std::env::temp_dir().join(format!("{}.PcbDoc", Uuid::new_v4()));
        d.source_path = Some(path.clone());
        let mut m = manager(&d);
        m.stack.add_copper(1).unwrap();
        let draft = m.stack.clone();
        let original = d.model.clone();
        app.documents.push(d);
        app.active = Some(app.documents.len() - 1);
        app.layer_stack = Some(m);
        let ctx = egui::Context::default();
        for panel in [
            Panel::Git,
            Panel::Components,
            Panel::Messages,
            Panel::Properties,
        ] {
            app.run(Command::ShowPanel(panel), &ctx);
            let side = if panel == Panel::Git {
                docks::Side::Left
            } else {
                docks::Side::Right
            };
            assert_eq!(app.selected_panel(side), Some(panel));
            assert!(app.layer_stack_active());
            assert_eq!(app.layer_stack.as_ref().unwrap().stack, draft);
            assert_eq!(app.documents.last().unwrap().model, original);
        }
        app.request_open_path(path);
        assert!(!app.layer_stack_active());
        assert_eq!(app.layer_stack.as_ref().unwrap().stack, draft);
        assert_eq!(app.active, Some(app.documents.len() - 1));
    }
    #[test]
    fn apply_is_atomic_preserves_hidden_layers_and_noop_stays_clean() {
        let mut d = document();
        let before = d.model.clone();
        let mut m = manager(&d);
        m.commit(&mut d).unwrap();
        assert_eq!(d.model, before);
        assert_eq!(d.history.revision, 0);
        d.view.pcb.visible.remove(&BOTTOM);
        m.stack.add_copper(1).unwrap();
        m.commit(&mut d).unwrap();
        assert_eq!(d.history.revision, 1);
        assert!(d.view.pcb.visible.contains(&2));
        assert!(!d.view.pcb.visible.contains(&BOTTOM));
        m.commit(&mut d).unwrap();
        assert_eq!(d.history.revision, 1);
        d.history.undo(&mut d.model);
        assert_eq!(d.model, before);
    }
    #[test]
    fn invalid_or_stale_draft_never_changes_document() {
        let mut d = document();
        let before = d.model.clone();
        let mut m = manager(&d);
        m.stack.layers[1].thickness = 0;
        assert!(m.commit(&mut d).is_err());
        assert_eq!(d.model, before);
        assert_eq!(d.history.revision, 0);
        m.stack = Stack::from_board(d.model.pcb.as_ref().unwrap());
        m.stack.layers[1].thickness += 1;
        m.revision = 99;
        assert!(m.commit(&mut d).is_err());
        assert_eq!(d.model, before);
    }

    #[test]
    fn draft_undo_redo_is_isolated_and_apply_is_one_board_transaction() {
        let mut d = document();
        let before = d.model.clone();
        let mut m = manager(&d);
        m.undo.push(m.stack.clone());
        m.stack.mirror_half(true).unwrap();
        m.stack.insert_copper(1, LayerKind::Signal).unwrap();
        m.stack.layers[1]
            .properties
            .insert("Manufacturer".into(), "Test laminate".into());
        let edited = m.stack.clone();
        let json = serde_json::to_string(&edited).unwrap();
        assert_eq!(serde_json::from_str::<Stack>(&json).unwrap(), edited);
        m.undo();
        assert_eq!(m.stack, m.original);
        assert_eq!(d.model, before);
        m.redo();
        assert_eq!(m.stack, edited);
        m.commit(&mut d).unwrap();
        assert_eq!(d.history.revision, 1);
        assert!(m.undo.is_empty());
        assert!(m.redo.is_empty());
        d.history.undo(&mut d.model);
        assert_eq!(d.model, before);
    }
}
