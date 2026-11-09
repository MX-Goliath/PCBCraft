//! Parameter Set properties share the PCB constraint editor and native fields.
use super::*;
#[derive(Clone)]
struct RuleDraft {
    index: Option<usize>,
    fields: eda_core::pcb::Fields,
    error: String,
}
#[derive(Clone, PartialEq)]
enum Selected {
    Parameter(String),
    Rule(usize),
}
#[derive(Clone)]
struct ParameterDraft {
    original: Option<String>,
    name: String,
    value: String,
    error: String,
}
#[derive(Clone, Default)]
struct State {
    tab: usize,
    selected: Option<Selected>,
    parameter: Option<ParameterDraft>,
    rule: Option<RuleDraft>,
}
fn label(name: &str) -> &str {
    if name.eq_ignore_ascii_case("ClassName") {
        "Net Class Name"
    } else if name.eq_ignore_ascii_case("CompClassName") {
        "Component Class Name"
    } else {
        name
    }
}
pub(super) fn properties(ui: &mut egui::Ui, p: &mut SchPrimitive, query: &str) -> bool {
    let mut changed = false;
    let id = ui.id().with(("directive", p.id));
    let mut state = ui.data_mut(|d| d.get_temp::<State>(id).unwrap_or_default());
    let K::ParameterSet {
        name,
        style,
        parameters,
        rules,
        ..
    } = &mut p.kind
    else {
        return false;
    };
    section(ui, "Properties", query, |ui| {
        changed |= text_row(ui, "Label", name);
        row(ui, "Style", |ui| {
            ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                changed |= combo(ui, "directive_style", style, &["Large", "Small"])
            });
            changed |= color(ui, &mut p.color);
        });
    });
    p.presentation.name = name.clone();
    section(ui, "Parameters", query, |ui| {
        ui.horizontal(|ui| {
            for (n, title) in ["All", "Parameters", "Rules", "Classes"].iter().enumerate() {
                if ui.selectable_label(state.tab == n, *title).clicked() {
                    state.tab = n;
                }
            }
        });
        let available = ui.available_width();
        let mut remove = None;
        egui::Grid::new(id.with("table"))
            .num_columns(2)
            .min_col_width(available * 0.40)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Name");
                ui.label("Value");
                ui.end_row();
                if state.tab != 2 {
                    for (key, value) in parameters.iter_mut() {
                        let class = key.eq_ignore_ascii_case("ClassName")
                            || key.eq_ignore_ascii_case("CompClassName");
                        if (state.tab == 1 && class) || (state.tab == 3 && !class) {
                            continue;
                        }
                        if ui
                            .selectable_label(
                                state.selected == Some(Selected::Parameter(key.clone())),
                                label(key),
                            )
                            .clicked()
                        {
                            state.selected = Some(Selected::Parameter(key.clone()));
                        }
                        let response = ui.add_sized(
                            [ui.available_width(), 22.0],
                            egui::TextEdit::singleline(value),
                        );
                        // Draft values persist while typing; commit on Enter or focus loss.
                        changed |= response.lost_focus();
                        ui.end_row();
                    }
                }
                if state.tab == 0 || state.tab == 2 {
                    for (n, rule) in rules.iter().enumerate() {
                        let kind = eda_core::pcb_rules::text(rule, "RULEKIND");
                        if ui
                            .selectable_label(
                                state.selected == Some(Selected::Rule(n)),
                                format!("{kind} Constraint"),
                            )
                            .clicked()
                        {
                            state.selected = Some(Selected::Rule(n));
                        }
                        if ui.link("Edit…").clicked() {
                            state.rule = Some(RuleDraft {
                                index: Some(n),
                                fields: rule.clone(),
                                error: String::new(),
                            });
                        }
                        ui.end_row();
                    }
                }
            });
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.menu_button("Add…", |ui| {
                for (title, key) in [
                    ("Net Class", "ClassName"),
                    ("Component Class", "CompClassName"),
                    ("Parameter", "Parameter"),
                ] {
                    if ui.button(title).clicked() {
                        state.parameter = Some(ParameterDraft {
                            original: parameters
                                .keys()
                                .find(|n| n.eq_ignore_ascii_case(key))
                                .cloned(),
                            name: key.into(),
                            value: parameters.get(key).cloned().unwrap_or_default(),
                            error: String::new(),
                        });
                        ui.close();
                    }
                }
                ui.menu_button("Rule", |ui| {
                    for (_, title, kind) in
                        crate::app::pcb_editor::rules_ui::schematic_rule_catalog()
                    {
                        if ui
                            .add_enabled(
                                eda_core::pcb_rules::supported(kind),
                                egui::Button::new(*title),
                            )
                            .clicked()
                        {
                            state.rule = Some(RuleDraft {
                                index: None,
                                fields: eda_core::pcb_rules::default_rule(kind),
                                error: String::new(),
                            });
                            ui.close();
                        }
                    }
                });
            });
            if ui
                .add_enabled(state.selected.is_some(), egui::Button::new("Remove"))
                .clicked()
            {
                remove = state.selected.take();
            }
        });
        if let Some(selected) = remove {
            match selected {
                Selected::Parameter(key) => {
                    parameters.remove(&key);
                }
                Selected::Rule(n) => {
                    if n < rules.len() {
                        rules.remove(n);
                    }
                }
            }
            changed = true;
        }
        if let Some(Selected::Parameter(key)) = &state.selected
            && ui.button("Edit…").clicked()
            && let Some(value) = parameters.get(key)
        {
            state.parameter = Some(ParameterDraft {
                original: Some(key.clone()),
                name: key.clone(),
                value: value.clone(),
                error: String::new(),
            });
        }
    });
    if let Some(draft) = &mut state.parameter {
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new("Parameter Properties")
            .id(id.with("parameter_editor"))
            .open(&mut open)
            .collapsible(false)
            .show_stable(ui.ctx(), |ui| {
                let class = draft.name.eq_ignore_ascii_case("ClassName")
                    || draft.name.eq_ignore_ascii_case("CompClassName");
                ui.add_enabled_ui(!class, |ui| {
                    text_row(ui, "Name", &mut draft.name);
                });
                text_row(ui, "Value", &mut draft.value);
                if !draft.error.is_empty() {
                    ui.colored_label(egui::Color32::LIGHT_RED, &draft.error);
                }
                ui.horizontal(|ui| {
                    accept = ui.button("OK").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if accept {
            if draft.name.trim().is_empty() || draft.name.eq_ignore_ascii_case("Rule") {
                draft.error = "Enter a parameter name other than the reserved Rule name".into();
            } else if parameters
                .keys()
                .any(|n| n.eq_ignore_ascii_case(&draft.name) && draft.original.as_ref() != Some(n))
            {
                draft.error = "A parameter with this name already exists".into();
            } else {
                if let Some(key) = &draft.original {
                    parameters.remove(key);
                }
                parameters.insert(draft.name.clone(), draft.value.clone());
                state.selected = Some(Selected::Parameter(draft.name.clone()));
                state.parameter = None;
                changed = true;
            }
        } else if cancel || !open {
            state.parameter = None;
        }
    }
    if let Some(draft) = &mut state.rule {
        let kind = eda_core::pcb_rules::text(&draft.fields, "RULEKIND").to_owned();
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        egui::Window::new(format!("Edit PCB Rule (From Schematic) - {kind} Rule"))
            .id(id.with("rule_editor"))
            .open(&mut open)
            .default_width(640.0)
            .show_stable(ui.ctx(), |ui| {
                if eda_core::pcb_rules::supported(&kind) {
                    crate::app::pcb_editor::rules_ui::schematic_rule_constraints(
                        ui,
                        &mut draft.fields,
                    );
                } else {
                    ui.label("This rule is preserved. Editing this rule family is unavailable.");
                }
                if !draft.error.is_empty() {
                    ui.colored_label(egui::Color32::LIGHT_RED, &draft.error);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    accept = ui
                        .add_enabled(
                            eda_core::pcb_rules::supported(&kind),
                            egui::Button::new("OK"),
                        )
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if accept {
            match eda_core::pcb_rules::validate_rule(
                &eda_core::pcb::Board::default(),
                &draft.fields,
            ) {
                Ok(()) => {
                    if let Some(n) = draft.index.filter(|n| *n < rules.len()) {
                        rules[n] = draft.fields.clone();
                    } else {
                        rules.push(draft.fields.clone());
                    }
                    changed = true;
                    state.rule = None;
                }
                Err(e) => draft.error = e,
            }
        } else if cancel || !open {
            state.rule = None;
        }
    }
    ui.data_mut(|d| d.insert_temp(id, state));
    changed
}
