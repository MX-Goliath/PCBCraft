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
        let caption = if component.is_some() {
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
        egui::ScrollArea::vertical()
            .id_salt(("sch_properties", doc.session_id, caption))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if !has_selection {
                    document_options(ui, doc, &mut self.settings.units, &query);
                } else if let Some(s) = &doc.model.schematic {
                    if let Some(c) = s.components.iter().find(|c| Some(c.id) == selected) {
                        component_properties(ui, c, &query);
                    } else if let Some(p) = s.primitives.iter().find(|p| Some(p.id) == selected) {
                        egui::CollapsingHeader::new("Properties").default_open(true).show(ui, |ui| {
                            if let Some(record) = s.records.get(p.source_index as usize) {
                                parameter_rows(ui, &record.fields, &query);
                            }
                        });
                    }
                }
                if let Some(s) = &doc.model.schematic {
                    ui.add_space(10.0);
                    egui::CollapsingHeader::new("Import Information").default_open(true).show(ui, |ui| {
                        if let Some(source) = &s.source { sch_row(ui, "Source", &source.filename); }
                        ui.label("Imported schematic · read only");
                        ui.label(RichText::new("Save As creates a PCBCraft document with the original data preserved.").small().color(MUTED));
                        for warning in &s.warnings {
                            ui.label(RichText::new(warning).color(Color32::from_rgb(225, 184, 105)));
                        }
                    });
                }
            });
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

fn component_properties(ui: &mut egui::Ui, c: &eda_core::schematic::Component, query: &str) {
    egui::CollapsingHeader::new("Properties")
        .default_open(true)
        .show(ui, |ui| {
            let rows = [
                ("Designator", c.designator.clone()),
                ("Comment", c.comment.clone()),
                ("Library Reference", c.library_reference.clone()),
                ("Description", c.description.clone()),
                ("Part", c.part.to_string()),
                ("Display Mode", c.display_mode.to_string()),
                ("X", format!("{:.6} mm", c.position.x as f64 / 1e6)),
                ("Y", format!("{:.6} mm", c.position.y as f64 / 1e6)),
            ];
            for (label, value) in rows {
                filtered_row(ui, label, &value, query);
            }
        });
    egui::CollapsingHeader::new("Parameters")
        .default_open(true)
        .show(ui, |ui| parameter_rows(ui, &c.parameters, query));
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
