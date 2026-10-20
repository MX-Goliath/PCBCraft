use super::*;
use eda_core::schematic::SchPrimitiveKind as K;

impl PcbCraft {
    pub(super) fn schematic_properties(&mut self, ui: &mut egui::Ui, index: usize) {
        let doc = &self.documents[index];
        let selected = doc.view.selected;
        let schematic = doc.model.schematic.as_ref();
        let component =
            schematic.and_then(|s| s.components.iter().find(|c| Some(c.id) == selected));
        let primitive =
            schematic.and_then(|s| s.primitives.iter().find(|p| Some(p.id) == selected));
        let has_selection = component.is_some() || primitive.is_some();
        let caption = if doc.sch_editor.active() {
            doc.sch_editor.caption()
        } else if component.is_some() {
            "Component"
        } else if let Some(p) = primitive {
            match &p.kind {
                K::Polyline { electrical, .. } if electrical == "Wire" => "Wire",
                K::Pin { .. } => "Pin",
                K::Text { .. } => "Text",
                K::PowerPort { .. } => "Power Port",
                K::Port { .. } => "Port",
                K::Junction { .. } => "Junction",
                _ => "Drawing Object",
            }
        } else {
            "Document Options"
        };
        ui.add_space(5.0);
        ui.label(caption);
        ui.add_space(8.0);
        theme::search_field(ui, &mut self.property_ui.search, ui.available_width());
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("General").color(Color32::WHITE));
            ui.separator();
            ui.label("Parameters");
        });
        ui.separator();
        let query = self.property_ui.search.to_lowercase();
        egui::TopBottomPanel::bottom("sch_selection_status")
            .exact_height(18.0)
            .show_inside(ui, |ui| {
                ui.label(if selected.is_some() {
                    "1 object selected"
                } else {
                    "Nothing selected"
                });
            });
        let doc = &mut self.documents[index];
        let mut edit_error = None;
        egui::ScrollArea::vertical()
            .id_salt(("sch_properties", doc.session_id, caption))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if doc.sch_editor.active() {
                    placement_options(ui,doc);
                } else if !has_selection {
                    document_options(ui, doc, &mut self.settings.units, &query);
                } else {
                    edit_error=editable_properties(ui,doc,self.settings.units);
                }
                if let Some(s) = &doc.model.schematic {
                    ui.add_space(10.0);
                    egui::CollapsingHeader::new("Import Information").default_open(true).show(ui, |ui| {
                        if let Some(source) = &s.source { sch_row(ui, "Source", &source.filename); }
                        ui.label(if s.source_modified {"Edited scene · native archive is the original"} else {"Schematic scene"});
                        ui.label(RichText::new("Edits save to .pcbcraft.json. The Altium source file is preserved.").small().color(MUTED));
                        for warning in &s.warnings {
                            ui.label(RichText::new(warning).color(Color32::from_rgb(225, 184, 105)));
                        }
                    });
                }
            });
        if let Some(error) = edit_error {
            self.message(true, error);
        }
    }
}

fn document_options(ui: &mut egui::Ui, doc: &mut OpenDocument, units: &mut Units, query: &str) {
    egui::CollapsingHeader::new("Selection Filter")
        .default_open(true)
        .show(ui, |ui| {
            let view = &mut doc.view;
            ui.horizontal_wrapped(|ui| {
                let filters = [
                    (&mut view.sch_select_components, "Components"),
                    (&mut view.sch_select_wires, "Wires / Buses"),
                    (&mut view.sch_select_text, "Texts / Net Labels"),
                    (&mut view.sch_select_graphics, "Drawing Objects"),
                ];
                for (value, label) in filters {
                    if ui.add(egui::Button::new(label).selected(*value)).clicked() {
                        *value = !*value;
                    }
                }
            });
        });
    egui::CollapsingHeader::new("General")
        .default_open(true)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Units");
                ui.selectable_value(units, Units::Millimetres, "mm");
                ui.selectable_value(units, Units::Mils, "mils");
            });
            ui.checkbox(&mut doc.view.grid, "Visible Grid");
            let mut grid = doc.view.grid_nm as f64 / 1e6;
            ui.horizontal(|ui| {
                ui.label("Grid Size");
                if ui
                    .add(
                        egui::DragValue::new(&mut grid)
                            .range(0.001..=100.0)
                            .suffix("mm"),
                    )
                    .changed()
                {
                    doc.view.grid_nm = (grid * 1e6).round() as i64;
                }
            });
            if let Some(s) = &doc.model.schematic {
                sch_row(
                    ui,
                    "Snap Grid",
                    &format!("{:.6} mm", s.sheet.snap_nm as f64 / 1e6),
                );
            }
        });
    egui::CollapsingHeader::new("Page Options")
        .default_open(true)
        .show(ui, |ui| {
            sch_row(
                ui,
                "Sheet Size",
                &format!(
                    "{:.2} × {:.2} mm",
                    doc.model.size.x as f64 / 1e6,
                    doc.model.size.y as f64 / 1e6
                ),
            );
            sch_row(
                ui,
                "Orientation",
                if doc.model.size.x >= doc.model.size.y {
                    "Landscape"
                } else {
                    "Portrait"
                },
            );
            if let Some(s) = &doc.model.schematic {
                sch_row(ui, "Style", &s.sheet.style);
                sch_row(ui, "Components", &s.components.len().to_string());
                sch_row(ui, "Drawing Objects", &s.primitives.len().to_string());
                sch_row(ui, "Source Records", &s.records.len().to_string());
                egui::CollapsingHeader::new("Parameters")
                    .show(ui, |ui| parameter_rows(ui, &s.sheet.parameters, query));
            }
        });
}

fn placement_options(ui: &mut egui::Ui, doc: &mut OpenDocument) {
    let editor = &mut doc.sch_editor;
    theme::heading(ui, editor.caption(), "");
    match editor.kind {
        Some(Placement::Resistor | Placement::Capacitor) => {
            property_field(ui, "Designator", &mut editor.designator);
            property_field(ui, "Comment", &mut editor.value);
        }
        Some(Placement::Text | Placement::NetLabel | Placement::Ground) => {
            property_field(ui, "Text", &mut editor.value);
        }
        _ => {}
    }
    ui.add_space(8.0);
    ui.label("Click to place. Space rotates. Esc cancels.");
    if matches!(editor.kind, Some(Placement::Wire | Placement::Line)) {
        ui.label("Click vertices; Enter or right-click finishes at the last vertex. Space changes the wire corner.");
    }
    if editor.kind.is_none() {
        ui.label("Move places the object without stretching attached wires.");
    }
}

fn property_field(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    ui.horizontal(|ui| {
        ui.add_sized([85.0, 21.0], egui::Label::new(label));
        let response = ui.add_sized(
            [ui.available_width(), 22.0],
            egui::TextEdit::singleline(value),
        );
        response.lost_focus() || (response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
    })
    .inner
}

fn editable_properties(ui: &mut egui::Ui, doc: &mut OpenDocument, units: Units) -> Option<String> {
    use eda_core::schematic_edit::{self as edit, Edit};
    let id = doc.view.selected?;
    let s = doc.model.schematic.as_ref()?;
    let component = s.components.iter().find(|c| c.id == id).cloned();
    let primitive = s.primitives.iter().find(|p| p.id == id).cloned();
    let position = edit::anchor(s, id)?;
    let text = primitive.as_ref().and_then(|p| match &p.kind {
        K::Text { text, .. } | K::TextFrame { text, .. } | K::PowerPort { text, .. } => {
            Some(text.clone())
        }
        K::Port { name, .. } => Some(name.clone()),
        _ => None,
    });
    let width = primitive
        .as_ref()
        .filter(|p| matches!(p.kind, K::Polyline { .. }))
        .map(|p| p.width_nm);
    if doc
        .sch_editor
        .properties
        .as_ref()
        .is_none_or(|p| p.id != id || p.revision != doc.history.revision)
    {
        doc.sch_editor.properties = Some(schematic_editor::PropertyDraft {
            id,
            revision: doc.history.revision,
            designator: component
                .as_ref()
                .map_or_else(String::new, |c| c.designator.clone()),
            value: component
                .as_ref()
                .map_or_else(|| text.clone().unwrap_or_default(), |c| c.comment.clone()),
            x: format!("{:.6}", units.from_nm(position.x as f64)),
            y: format!("{:.6}", units.from_nm(position.y as f64)),
            width: format!("{:.6}", units.from_nm(width.unwrap_or(254_000) as f64)),
        });
    }
    let draft = doc.sch_editor.properties.as_mut().unwrap();
    let mut commit = false;
    theme::heading(ui, "Properties", "");
    if component.is_some() {
        commit |= property_field(ui, "Designator", &mut draft.designator);
    }
    if component.is_some() || text.is_some() {
        commit |= property_field(
            ui,
            if component.is_some() {
                "Comment"
            } else {
                "Text"
            },
            &mut draft.value,
        );
    }
    commit |= property_field(ui, &format!("X ({})", units.label()), &mut draft.x);
    commit |= property_field(ui, &format!("Y ({})", units.label()), &mut draft.y);
    if width.is_some() {
        commit |= property_field(ui, &format!("Width ({})", units.label()), &mut draft.width);
    }
    if let Some(c) = &component {
        sch_row(ui, "Library Reference", &c.library_reference);
        egui::CollapsingHeader::new("Parameters")
            .show(ui, |ui| parameter_rows(ui, &c.parameters, ""));
    }
    if !commit {
        return None;
    }
    let draft = draft.clone();
    let parse = |text: &str| -> Result<i64, String> {
        let value = text
            .trim()
            .parse::<f64>()
            .map_err(|_| "Enter a numeric coordinate or width")?;
        let nm = value * if units == Units::Mils { 25_400.0 } else { 1e6 };
        if !nm.is_finite() || nm.abs() > 100_000_000_000.0 {
            return Err("Value exceeds the document range".into());
        }
        Ok(nm.round() as i64)
    };
    let result = (|| -> Result<(), String> {
        let mut edits = vec![];
        let next = eda_geom::Point {
            x: parse(&draft.x)?,
            y: parse(&draft.y)?,
        };
        if next != position {
            edits.push(Edit::Transform {
                id,
                pivot: position,
                quarter_turns: 0,
                delta: eda_geom::Point {
                    x: next.x - position.x,
                    y: next.y - position.y,
                },
            });
        }
        if let Some(c) = &component {
            if c.designator != draft.designator || c.comment != draft.value {
                edits.push(Edit::ComponentProperties {
                    id,
                    designator: draft.designator.clone(),
                    comment: draft.value.clone(),
                });
            }
        } else if text.as_ref().is_some_and(|old| old != &draft.value) {
            edits.push(Edit::Text {
                id,
                text: draft.value.clone(),
            });
        }
        if let Some(old) = width {
            let new = parse(&draft.width)?;
            if old != new {
                edits.push(Edit::Width { id, width_nm: new });
            }
        }
        let revision = draft.revision;
        if !edits.is_empty() {
            doc.edit_schematic(Edit::Batch(edits), revision)?;
        }
        Ok(())
    })();
    if result.is_err() {
        doc.sch_editor.properties = None;
    }
    result.err()
}

fn parameter_rows(
    ui: &mut egui::Ui,
    values: &std::collections::BTreeMap<String, String>,
    query: &str,
) {
    for (key, value) in values {
        filtered_row(ui, key, value, query);
    }
}

fn filtered_row(ui: &mut egui::Ui, label: &str, value: &str, query: &str) {
    if query.is_empty()
        || label.to_lowercase().contains(query)
        || value.to_lowercase().contains(query)
    {
        sch_row(ui, label, value);
    }
}

fn sch_row(ui: &mut egui::Ui, label: &str, value: &str) {
    egui::Frame::NONE
        .fill(theme::GROUP)
        .inner_margin(4)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.add_sized([95.0, 18.0], egui::Label::new(label));
                ui.label(value);
            });
        });
}
