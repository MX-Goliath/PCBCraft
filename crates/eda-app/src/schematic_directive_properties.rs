//! Observed directive/probe tables and schematic rule editing.
use super::*;
pub(super) fn is_pair(p: &SchPrimitive, related: &[SchPrimitive]) -> bool {
    matches!(p.kind, K::ParameterSet { .. })
        && related.iter().any(|v| {
            v.presentation.parent == Some(p.id)
                && v.presentation.name.eq_ignore_ascii_case("DifferentialPair")
                && matches!(&v.kind,K::Text{text,..} if text.eq_ignore_ascii_case("True"))
        })
}
fn class(name: &str) -> usize {
    if name.eq_ignore_ascii_case("Rule") {
        2
    } else if ["ClassName", "CompClassName"]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
    {
        3
    } else {
        1
    }
}
fn rule_fields(p: &SchPrimitive) -> eda_core::pcb::Fields {
    if let K::Text { text, .. } = &p.kind {
        text.split('\u{a6}')
            .filter_map(|f| f.split_once('='))
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    } else {
        Default::default()
    }
}
pub(super) fn directive_parameters(
    ui: &mut egui::Ui,
    parent: &SchPrimitive,
    related: &mut Vec<SchPrimitive>,
    s: &Schematic,
    query: &str,
    controls: (
        &mut usize,
        &mut Option<String>,
        &mut Option<DirectiveRuleDialog>,
    ),
) -> bool {
    let (filter, selected, rule) = controls;
    let mut changed = false;
    section(ui, "Parameters", query, |ui| {
        ui.horizontal(|ui| {
            for (n, label) in ["All", "Parameters", "Rules", "Classes"].iter().enumerate() {
                if ui
                    .add_sized(
                        [[42., 74., 44., 54.][n], 24.],
                        egui::Button::new(*label).selected(*filter == 0 || *filter == n),
                    )
                    .clicked()
                {
                    *filter = if n == 0 { 0 } else { n };
                }
            }
        });
        let mut remove = None;
        egui::Frame::NONE
            .stroke(Stroke::new(1., theme::BORDER))
            .show(ui, |ui| {
                ui.set_min_height(270.);
                let width = (ui.available_width() - 60.).max(100.) / 2.;
                egui::Grid::new(("directive_parameters", parent.id))
                    .num_columns(4)
                    .spacing(egui::vec2(3., 2.))
                    .show(ui, |ui| {
                        ui.label("");
                        ui.label("Name");
                        ui.label("Value");
                        ui.label("");
                        ui.end_row();
                        for category in 1..=3 {
                            if *filter != 0 && *filter != category {
                                continue;
                            }
                            let mut count = 0;
                            for child in related.iter_mut().filter(|p| {
                                !p.presentation.name.eq_ignore_ascii_case("DifferentialPair")
                                    && class(&p.presentation.name) == category
                            }) {
                                count += 1;
                                let mut visible = !child.presentation.hidden;
                                if small_icon(ui, "eye", &mut visible) {
                                    child.presentation.hidden = !visible;
                                    changed = true;
                                }
                                let id = child.id;
                                ui.push_id(id, |ui| {
                                    ui.allocate_ui(egui::vec2(width, 22.), |ui| {
                                        match child.presentation.name.as_str() {
                                            "ClassName" => {
                                                ui.label("Net Class Name");
                                            }
                                            "CompClassName" => {
                                                ui.label("Component Class Name");
                                            }
                                            "Rule" => {
                                                let fields = rule_fields(child);
                                                let label = fields
                                                    .get("RULEKIND")
                                                    .map(String::as_str)
                                                    .unwrap_or("Rule");
                                                if ui
                                                    .selectable_label(
                                                        selected.as_deref()
                                                            == Some(&id.to_string()),
                                                        format!("{label} Constraint"),
                                                    )
                                                    .double_clicked()
                                                {
                                                    *rule = Some(DirectiveRuleDialog {
                                                        child: Some(id),
                                                        selected: label.into(),
                                                        fields: Some(fields),
                                                    });
                                                }
                                            }
                                            _ => {
                                                changed |= text(ui, &mut child.presentation.name);
                                            }
                                        }
                                        if ui.rect_contains_pointer(ui.min_rect()) {
                                            *selected = Some(id.to_string());
                                        }
                                    });
                                });
                                if category == 2 {
                                    let fields = rule_fields(child);
                                    if ui
                                        .add_sized(
                                            [width, 22.],
                                            egui::Button::new(
                                                super::super::pcb_editor::rules_ui::schematic_rule_summary(
                                                    &fields,
                                                ),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        *rule = Some(DirectiveRuleDialog {
                                            child: Some(id),
                                            selected: fields
                                                .get("RULEKIND")
                                                .cloned()
                                                .unwrap_or_default(),
                                            fields: Some(fields),
                                        });
                                    }
                                } else if let K::Text { text: value, .. } = &mut child.kind {
                                    ui.push_id((id, "value"), |ui| {
                                        ui.allocate_ui(egui::vec2(width, 22.), |ui| {
                                            if text(ui, value) {
                                                child.presentation.raw_text = Some(value.clone());
                                                changed = true;
                                            }
                                        });
                                    });
                                } else {
                                    ui.label("");
                                }
                                if ui.small_button("-").clicked() {
                                    remove = Some(id);
                                }
                                ui.end_row();
                            }
                            if count == 0 {
                                ui.label("");
                                ui.label(["", "No Parameters", "No Rules", "No Classes"][category]);
                                ui.label("");
                                ui.label("");
                                ui.end_row();
                            }
                        }
                    });
            });
        if let Some(id) = remove {
            related.retain(|p| p.id != id);
            changed = true;
        }
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            let chosen = selected
                .as_ref()
                .and_then(|id| related.iter().find(|p| p.id.to_string() == *id));
            if tool_button(
                ui,
                "edit",
                chosen.is_some_and(|p| class(&p.presentation.name) == 2),
            )
            .clicked()
                && let Some(p) = chosen
            {
                *rule = Some(DirectiveRuleDialog {
                    child: Some(p.id),
                    selected: String::new(),
                    fields: Some(rule_fields(p)),
                });
            }
            ui.menu_button("Add...", |ui| {
                for (label, name) in [
                    ("Net Class", "ClassName"),
                    ("Component Class", "CompClassName"),
                    ("Parameter", ""),
                    ("Rule", "Rule"),
                ] {
                    if ui.button(label).clicked() {
                        if name == "Rule" {
                            *rule = Some(DirectiveRuleDialog {
                                child: None,
                                selected: "Clearance".into(),
                                fields: None,
                            });
                        } else {
                            let name = if name.is_empty() {
                                (1..)
                                    .map(|n| format!("Parameter {n}"))
                                    .find(|name| {
                                        !related
                                            .iter()
                                            .any(|p| p.presentation.name.eq_ignore_ascii_case(name))
                                    })
                                    .unwrap()
                            } else {
                                name.into()
                            };
                            let at = primitive_anchor(parent);
                            related.push(eda_core::schematic_simulation::parameter(
                                parent.id,
                                &name,
                                "*",
                                Point {
                                    x: at.x,
                                    y: at.y - 4_064_000,
                                },
                                s.sheet.system_font,
                                true,
                            ));
                            changed = true;
                        }
                        ui.close();
                    }
                }
            });
        });
    });
    changed
}
pub(super) fn directive_rule_dialog(
    ctx: &egui::Context,
    dialog: &mut DirectiveRuleDialog,
) -> (bool, bool) {
    let mut open = true;
    let mut accept = false;
    let mut choose = false;
    let mut cancel = false;
    if let Some(fields) = &mut dialog.fields {
        let kind = fields.get("RULEKIND").cloned().unwrap_or_default();
        egui::Window::new(format!("Edit PCB Rule (From Schematic) - {kind} Rule"))
            .id(egui::Id::new("schematic_rule_editor"))
            .fixed_size([700., 420.])
            .resizable(false)
            .collapsible(false)
            .open(&mut open)
            .show_stable(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(365.)
                    .show(ui, |ui| {
                        super::super::pcb_editor::rules_ui::schematic_constraints(ui, fields);
                    });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        choose = true;
                    }
                    accept = ui.button("OK").clicked();
                });
            });
    } else {
        egui::Window::new("Choose Design Rule Type")
            .id(egui::Id::new("schematic_rule_type"))
            .fixed_size([444., 604.])
            .resizable(false)
            .collapsible(false)
            .open(&mut open)
            .show_stable(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(555.)
                    .show(ui, |ui| {
                        let mut category = "";
                        for (group, label, kind) in
                            super::super::pcb_editor::rules_ui::schematic_catalog()
                        {
                            if *group != category {
                                category = group;
                                ui.label(egui::RichText::new(*group).strong());
                            }
                            ui.indent(group, |ui| {
                                let response = ui.add_enabled(
                                    eda_core::pcb_rules::supported(kind),
                                    egui::Button::new(format!("{label} Constraint"))
                                        .selected(dialog.selected == *kind)
                                        .frame(false),
                                );
                                if response.clicked() {
                                    dialog.selected = (*kind).into();
                                }
                                choose |= response.double_clicked();
                            });
                        }
                    });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                    choose |= ui.button("OK").clicked();
                });
            });
        if choose {
            dialog.fields = Some(eda_core::pcb_rules::default_rule(&dialog.selected));
            return (false, false);
        }
    }
    (accept, !open || choose || cancel)
}
pub(super) fn simulation_source_properties(
    ui: &mut egui::Ui,
    c: &mut Component,
    related: &mut [SchPrimitive],
    query: &str,
) -> bool {
    let mut changed = false;
    section(ui, "General", query, |ui| {
        changed |= text_row(ui, "Designator", &mut c.designator);
        row(ui, "Stimulus Name", |ui| {
            readonly(ui, "NONE");
        });
        row(ui, "Stimulus Type", |ui| {
            readonly(ui, "DC Source");
        });
        waveform(ui);
    });
    section(ui, "Parameters", query, |ui| {
        let width = (ui.available_width() - 4.) / 2.;
        egui::Grid::new(("simulation_parameters", c.id))
            .num_columns(2)
            .min_col_width(width)
            .max_col_width(width)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Name");
                ui.label("Value");
                ui.end_row();
                for (key, label) in [
                    ("Value", "DC Magnitude"),
                    ("AC Magnitude", "AC Magnitude"),
                    ("AC Phase", "AC Phase"),
                ] {
                    ui.label(label);
                    let value = c.parameters.entry(key.into()).or_default();
                    let r = ui.add_sized([width, 22.], egui::TextEdit::singleline(value));
                    changed |= r.lost_focus();
                    ui.end_row();
                }
            });
    });
    if changed {
        let value = c.parameters.get("Value").cloned().unwrap_or_default();
        let comment = format!("DC {value}");
        c.parameters.insert("Comment".into(), comment.clone());
        if let Some(p) = related
            .iter_mut()
            .find(|p| p.text_role == Some(TextRole::Comment))
            && let K::Text { text, .. } = &mut p.kind
        {
            *text = comment.clone();
            p.presentation.raw_text = Some(comment);
        }
    }
    changed
}
fn waveform(ui: &mut egui::Ui) {
    plot(ui, true);
}
fn plot(ui: &mut egui::Ui, stimulus: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 300.), Sense::hover());
    let plot = rect.shrink2(egui::vec2(30., 24.));
    ui.painter().rect_stroke(
        plot,
        0.,
        Stroke::new(1., Color32::from_gray(170)),
        egui::StrokeKind::Inside,
    );
    if stimulus {
        ui.painter().line_segment(
            [
                egui::pos2(plot.left(), plot.center().y),
                egui::pos2(plot.right(), plot.center().y),
            ],
            Stroke::new(1., Color32::from_rgb(102, 148, 195)),
        );
    }
}
pub(super) fn simulation_probe_properties(
    ui: &mut egui::Ui,
    p: &mut SchPrimitive,
    related: &mut [SchPrimitive],
    units: Units,
    query: &str,
) -> bool {
    let mut changed = false;
    section(ui, "Properties", query, |ui| {
        if let Some(child) = related
            .iter_mut()
            .find(|p| p.presentation.name == "ProbeCustomName")
            && let K::Text { text: value, .. } = &mut child.kind
        {
            row(ui, "Name", |ui| {
                let response = ui.add_sized(
                    [ui.available_width() - 26., 22.],
                    egui::TextEdit::singleline(value).hint_text("Empty Probe"),
                );
                if response.lost_focus() {
                    child.presentation.raw_text = Some(value.clone());
                    changed = true;
                }
                let mut visible = !child.presentation.hidden;
                if small_icon(ui, "eye", &mut visible) {
                    child.presentation.hidden = !visible;
                    changed = true;
                }
            });
        }
        row(ui, "Value", |ui| readonly(ui, "N/A"));
    });
    if let K::ParameterSet {
        position,
        orientation,
        ..
    } = &mut p.kind
    {
        let old = *position;
        let old_turns = *orientation;
        section(ui, "Location", query, |ui| {
            changed |= xy(ui, position, units);
            changed |= rotation(ui, orientation);
        });
        for child in related {
            let _ = edit::transform_primitive(
                child,
                old,
                (*orientation + 4 - old_turns) % 4,
                Point {
                    x: position.x - old.x,
                    y: position.y - old.y,
                },
            );
        }
    }
    section(ui, "Plot", query, |ui| plot(ui, false));
    changed
}
