use super::*;
use eda_core::layer_stack::{Impedance, Layer, LayerKind, Stack, ViaKind, ViaType};
use eda_core::pcb::{BOTTOM, TOP};

pub(super) struct Manager {
    session: Uuid,
    revision: u64,
    stack: Stack,
    original: Stack,
    tab: usize,
    selected: usize,
    error: String,
    mils: bool,
    confirm_close: bool,
}
impl Manager {
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
        Ok(())
    }
}
impl PcbCraft {
    pub(super) fn open_layer_stack(&mut self) {
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
            session: d.session_id,
            revision: d.history.revision,
            original: stack.clone(),
            stack,
            tab: 0,
            selected: 0,
            error: String::new(),
            mils: false,
            confirm_close: false,
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
        egui::SidePanel::right("layer_stack_properties")
            .default_width(316.0)
            .width_range(260.0..=460.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                ui.label("Layer Stack Manager");
                ui.label("Edited stacks are saved with Save As → PCBCraft JSON.");
                ui.label(format!("{} copper layers", m.stack.copper().len()));
                ui.label(format!(
                    "Board thickness: {:.4} mm",
                    m.stack.thickness() as f64 / 1e6
                ));
                ui.checkbox(&mut m.mils, "Display in mils");
                ui.separator();
                match m.tab {
                    0 => {
                        if let Some(l) = m.stack.layers.get_mut(m.selected) {
                            ui.label("Layer");
                            ui.text_edit_singleline(&mut l.name);
                            let kinds = if l.kind.copper() {
                                vec![LayerKind::Signal, LayerKind::Plane]
                            } else {
                                vec![LayerKind::Core, LayerKind::Prepreg, LayerKind::SolderMask]
                            };
                            egui::ComboBox::from_id_salt("stack_kind")
                                .selected_text(format!("{:?}", l.kind))
                                .show_ui(ui, |ui| {
                                    for kind in kinds {
                                        ui.selectable_value(&mut l.kind, kind, format!("{kind:?}"));
                                    }
                                });
                            ui.label("Material");
                            ui.text_edit_singleline(&mut l.material);
                            ui.horizontal(|ui| {
                                ui.label("Thickness");
                                length(ui, &mut l.thickness, m.mils);
                            });
                            if !l.kind.copper() {
                                ui.horizontal(|ui| {
                                    ui.label("Dk");
                                    ui.add(
                                        egui::DragValue::new(&mut l.er)
                                            .speed(0.01)
                                            .range(1.0..=100.0),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.label("Df");
                                    ui.add(
                                        egui::DragValue::new(&mut l.loss_tangent)
                                            .speed(0.001)
                                            .range(0.0..=1.0),
                                    );
                                });
                            }
                        }
                    }
                    1 => {
                        let copper = copper_names(&m.stack);
                        if let Some(v) = m.stack.vias.get_mut(m.selected) {
                            ui.label("Via Type");
                            ui.text_edit_singleline(&mut v.name);
                            egui::ComboBox::from_id_salt("via_kind")
                                .selected_text(format!("{:?}", v.kind))
                                .show_ui(ui, |ui| {
                                    for k in
                                        [ViaKind::Through, ViaKind::BlindBuried, ViaKind::Micro]
                                    {
                                        if ui
                                            .selectable_value(&mut v.kind, k, format!("{k:?}"))
                                            .changed()
                                            && k == ViaKind::Through
                                        {
                                            v.start = TOP;
                                            v.end = BOTTOM;
                                        }
                                    }
                                });
                            ui.add_enabled_ui(v.kind != ViaKind::Through, |ui| {
                                layer_combo(ui, "First layer", &mut v.start, &copper);
                                layer_combo(ui, "Last layer", &mut v.end, &copper);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Diameter");
                                length(ui, &mut v.diameter, m.mils);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Hole size");
                                length(ui, &mut v.hole, m.mils);
                            });
                            if ui.button("Apply and use for placement").clicked() {
                                apply = true;
                                use_via = Some(v.clone());
                            }
                        }
                    }
                    _ => {
                        let copper = copper_names(&m.stack);
                        if let Some(p) = m.stack.impedances.get_mut(m.selected) {
                            ui.label("Impedance Profile");
                            ui.text_edit_singleline(&mut p.name);
                            ui.checkbox(&mut p.differential, "Differential pair");
                            ui.horizontal(|ui| {
                                ui.label("Target (Ω)");
                                ui.add(egui::DragValue::new(&mut p.target).range(1.0..=1000.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label("Tolerance (%)");
                                ui.add(egui::DragValue::new(&mut p.tolerance).range(0.0..=50.0));
                            });
                            layer_combo(ui, "Signal layer", &mut p.signal, &copper);
                            layer_combo(ui, "Reference 1", &mut p.reference, &copper);
                            let mut two = p.reference2.is_some();
                            if ui
                                .checkbox(&mut two, "Stripline (two references)")
                                .changed()
                            {
                                p.reference2 = two.then_some(BOTTOM);
                            }
                            if let Some(r) = &mut p.reference2 {
                                layer_combo(ui, "Reference 2", r, &copper);
                            }
                            ui.horizontal(|ui| {
                                ui.label("Width");
                                length(ui, &mut p.width, m.mils);
                            });
                            if p.differential {
                                ui.horizontal(|ui| {
                                    ui.label("Gap");
                                    length(ui, &mut p.gap, m.mils);
                                });
                            }
                        }
                        if let Some(p) = m.stack.impedances.get(m.selected).cloned() {
                            ui.separator();
                            ui.label("Analytical estimate (not a field solver)");
                            match m.stack.estimate(&p) {
                                Ok(z) => {
                                    ui.label(format!("Estimated impedance: {z:.2} Ω"));
                                }
                                Err(e) => {
                                    ui.label(e);
                                }
                            }
                            if ui.button("Calculate width for target").clicked() {
                                match m.stack.solve_width(&p) {
                                    Ok(w) => m.stack.impedances[m.selected].width = w,
                                    Err(e) => m.error = e,
                                }
                            }
                            if ui
                                .add_enabled(
                                    !p.differential,
                                    egui::Button::new("Apply and use routing width"),
                                )
                                .clicked()
                            {
                                apply = true;
                                use_width = Some((p.signal, p.width));
                            }
                        }
                    }
                }
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.strong(format!(
                    "Layer Stack Manager{}",
                    if m.stack != m.original { " *" } else { "" }
                ));
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    apply = true;
                }
                if ui.button("Close").clicked() {
                    if m.stack == m.original {
                        close = true;
                    } else {
                        m.confirm_close = true;
                    }
                }
                ui.separator();
                match m.tab {
                    0 => {
                        if ui.button("Add Layer").clicked()
                            && let Err(e) = m.stack.add_copper(m.selected)
                        {
                            m.error = e;
                        }
                        if ui.button("Add Dielectric").clicked() {
                            let at = (m.selected + 1).min(m.stack.layers.len());
                            m.stack.layers.insert(at, Layer::dielectric());
                            m.selected = at;
                        }
                        if ui.button("Move Up").clicked()
                            && m.selected > 0
                            && m.selected < m.stack.layers.len()
                        {
                            m.stack.layers.swap(m.selected, m.selected - 1);
                            m.selected -= 1;
                        }
                        if ui.button("Move Down").clicked() && m.selected + 1 < m.stack.layers.len()
                        {
                            m.stack.layers.swap(m.selected, m.selected + 1);
                            m.selected += 1;
                        }
                    }
                    1 => {
                        if ui.button("Add Via Type").clicked() {
                            let n = m.stack.vias.len() + 1;
                            m.stack.vias.push(ViaType {
                                id: Uuid::new_v4(),
                                name: format!("Via {n}"),
                                kind: ViaKind::Through,
                                start: TOP,
                                end: BOTTOM,
                                diameter: 600_000,
                                hole: 300_000,
                            });
                            m.selected = n - 1;
                        }
                    }
                    _ => {
                        if ui.button("Add Impedance Profile").clicked() {
                            let n = m.stack.impedances.len() + 1;
                            let reference = m
                                .stack
                                .copper()
                                .get(1)
                                .and_then(|l| l.copper_id)
                                .unwrap_or(BOTTOM);
                            m.stack.impedances.push(Impedance {
                                id: Uuid::new_v4(),
                                name: format!("S{n}"),
                                differential: false,
                                target: 50.0,
                                tolerance: 10.0,
                                signal: TOP,
                                reference,
                                reference2: None,
                                width: 200_000,
                                gap: 200_000,
                            });
                            m.selected = n - 1;
                        }
                    }
                }
                if ui.button("Delete").clicked() {
                    match m.tab {
                        0 => {
                            if let Some(l) = m.stack.layers.get(m.selected) {
                                if l.copper_id == Some(TOP) || l.copper_id == Some(BOTTOM) {
                                    m.error = "Outer copper layers cannot be deleted".into();
                                } else {
                                    m.stack.layers.remove(m.selected);
                                }
                            }
                        }
                        1 => {
                            if m.selected < m.stack.vias.len() {
                                m.stack.vias.remove(m.selected);
                            }
                        }
                        _ => {
                            if m.selected < m.stack.impedances.len() {
                                m.stack.impedances.remove(m.selected);
                            }
                        }
                    }
                    m.selected = m.selected.saturating_sub(1);
                }
            });
            if !m.error.is_empty() {
                ui.colored_label(Color32::from_rgb(245, 150, 110), &m.error);
            }
            if let Err(e) = m.stack.validate() {
                ui.colored_label(Color32::YELLOW, e);
            }
            ui.separator();
            let height = (ui.available_height() - 220.0).max(150.0);
            egui::ScrollArea::both()
                .max_height(height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::Grid::new(("stack_table", m.tab))
                        .striped(true)
                        .min_col_width(70.0)
                        .show(ui, |ui| match m.tab {
                            0 => {
                                for s in ["Layer", "Type", "Material", "Thickness", "Dk", "Df"] {
                                    ui.strong(s);
                                }
                                ui.end_row();
                                for (i, l) in m.stack.layers.iter_mut().enumerate() {
                                    if ui.selectable_label(m.selected == i, &l.name).clicked() {
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
                                    if ui.selectable_label(m.selected == i, &v.name).clicked() {
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
                                    if ui.selectable_label(m.selected == i, &p.name).clicked() {
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
                });
            ui.separator();
            preview(ui, &m.stack, m.selected, m.tab);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    for (tab, name) in ["Stackup", "Via Types", "Impedance"].iter().enumerate() {
                        if ui.selectable_label(m.tab == tab, *name).clicked() {
                            m.tab = tab;
                            m.selected = 0;
                            m.error.clear();
                        }
                    }
                });
            });
        });
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
    format!(
        "{:.4} {}",
        n as f64 / if mils { 25_400.0 } else { 1e6 },
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
fn layer_combo(ui: &mut egui::Ui, label: &str, value: &mut u16, names: &[(u16, String)]) {
    ui.label(label);
    egui::ComboBox::from_id_salt(label)
        .selected_text(layer_name(*value, names))
        .show_ui(ui, |ui| {
            for (id, n) in names {
                ui.selectable_value(value, *id, n);
            }
        });
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
            session: d.session_id,
            revision: d.history.revision,
            original: stack.clone(),
            stack,
            tab: 0,
            selected: 0,
            error: String::new(),
            mils: false,
            confirm_close: false,
        }
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
}
