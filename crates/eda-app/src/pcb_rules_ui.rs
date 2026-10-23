//! Rule drafts and asynchronous DRC belong to the current document session.
use super::*;
use eda_core::pcb::Fields;
use eda_core::pcb_rules::{self as rules, Report, text};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
pub(super) struct Draft {
    revision: u64,
    original: Vec<Fields>,
    rules: Vec<Fields>,
    selected: usize,
    search: String,
    error: String,
    priorities: bool,
}
pub(super) struct State {
    pub editor: Option<Draft>,
    pub show_report: bool,
    online: bool,
    markers: bool,
    report: Option<(Version, Report)>,
    pending: Option<mpsc::Receiver<(Version, Report)>>,
    pub(super) settings: CheckSettings,
    pub setup: Option<CheckSettings>,
    settings_page: String,
    generation: u64,
    last_start: Instant,
    force: bool,
    filter: String,
}
type Version = (u64, u64, u64);
const CATEGORIES: &[&str] = &[
    "Electrical",
    "Routing",
    "SMT",
    "Mask",
    "Plane",
    "Testpoint",
    "Manufacturing",
    "High Speed",
    "Placement",
    "Signal Integrity",
];
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::app) struct CheckSettings {
    online_excluded: BTreeSet<String>,
    batch_excluded: BTreeSet<String>,
    limit: usize,
    create_violations: bool,
}
impl Default for CheckSettings {
    fn default() -> Self {
        Self {
            online_excluded: BTreeSet::from(["UnRoutedNet".into()]),
            batch_excluded: BTreeSet::new(),
            limit: 500,
            create_violations: true,
        }
    }
}
impl Default for State {
    fn default() -> Self {
        Self {
            editor: None,
            show_report: false,
            online: true,
            markers: true,
            report: None,
            pending: None,
            settings: CheckSettings::default(),
            setup: None,
            settings_page: "Report Options".into(),
            generation: 0,
            last_start: Instant::now() - Duration::from_secs(1),
            force: false,
            filter: String::new(),
        }
    }
}
pub(super) fn open(doc: &mut OpenDocument) {
    if doc.pcb_editor.has_pending() {
        doc.pcb_editor.last_error =
            Some("Finish or cancel the current PCB operation before editing rules".into());
        return;
    }
    let Some(board) = &doc.model.pcb else {
        return;
    };
    if doc.pcb_editor.rules.editor.is_none() {
        doc.pcb_editor.rules.editor = Some(Draft {
            revision: doc.history.revision,
            original: board.rules.clone(),
            rules: board.rules.clone(),
            selected: board
                .rules
                .iter()
                .position(|r| text(r, "RULEKIND") == "Clearance")
                .unwrap_or(0),
            search: String::new(),
            error: String::new(),
            priorities: false,
        });
    }
}
pub(super) fn request_check(doc: &mut OpenDocument) {
    let state = &mut doc.pcb_editor.rules;
    state.setup = Some(state.settings.clone());
}
pub(super) fn browse(doc: &mut OpenDocument) {
    doc.pcb_editor.rules.show_report = true;
}
pub(super) fn reset_markers(doc: &mut OpenDocument) {
    doc.pcb_editor.rules.markers = false;
    doc.view.pcb.drc_markers.clear();
}
fn category(kind: &str) -> &str {
    if let Some((cat, _, _)) = rules::KINDS.iter().find(|(_, _, k)| *k == kind) {
        return cat;
    }
    match kind {
        "AcuteAngle"
        | "NetAntennae"
        | "MinimumAnnularRing"
        | "MinimumSolderMaskSliver"
        | "SilkToSilkClearance"
        | "SilkToSolderMaskClearance"
        | "BoardOutlineClearance"
        | "SilkToBoardRegionClearance" => "Manufacturing",
        "RoutingTopology" | "RoutingPriority" | "RoutingCorners" | "FanoutControl"
        | "DiffPairsRouting" => "Routing",
        "SolderMaskExpansion" | "PasteMaskExpansion" => "Mask",
        "PlaneClearance" | "PlaneConnect" | "PolygonConnect" => "Plane",
        "SMDToCorner" | "SMDNeckDown" | "SMDToPlane" => "SMT",
        "Height" | "ComponentClearance" | "RoomDefinition" => "Placement",
        "FabricationTestpoint"
        | "AssemblyTestpoint"
        | "FabricationTestPointUsage"
        | "AssemblyTestPointUsage" => "Testpoint",
        "MatchedLengths" | "Length" | "DaisyChainStubLength" => "High Speed",
        "UnpouredPolygon" | "UnConnectedPin" | "Creepage" => "Electrical",
        _ => "Signal Integrity",
    }
}
fn checker_settings(doc: &OpenDocument, ctx: &egui::Context, state: &mut State) {
    let Some(mut draft) = state.setup.take() else {
        return;
    };
    let mut save = false;
    let mut cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut run = false;
    egui::Window::new(egui::RichText::new("Design Rule Checker [mm]").size(13.0)).id(egui::Id::new(("drc_setup_v2",doc.session_id)))
        .fixed_size([760.0,660.0]).default_pos(ctx.screen_rect().center()-egui::vec2(380.0,350.0)).collapsible(false).show(ctx,|ui| {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(200.0,600.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_min_size(egui::vec2(200.0,600.0));
                ui.selectable_value(&mut state.settings_page,"Report Options".into(),"Report Options");
                ui.selectable_value(&mut state.settings_page,"Rules To Check".into(),"Rules To Check");
                ui.indent("check_categories",|ui| {for cat in CATEGORIES.iter().filter(|c|!matches!(**c,"Mask"|"Plane")) {
                    ui.selectable_value(&mut state.settings_page,(*cat).into(),*cat);
                }});
            });
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),600.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.set_min_height(600.0);
                if state.settings_page=="Report Options" {
                    ui.strong("DRC Report Options");ui.separator();
                    let mut unavailable=false;
                    for label in ["Create Report File","Report PCB Health Issues"] {ui.add_enabled(false,egui::Checkbox::new(&mut unavailable,label));}
                    ui.checkbox(&mut draft.create_violations,"Create Violations");
                    for label in ["Sub-Net Details","Verify Shorting Copper","Report Drilled SMT Pads","Report Multilayer Pads with 0 size Hole"] {ui.add_enabled(false,egui::Checkbox::new(&mut unavailable,label));}
                    ui.horizontal(|ui| {ui.label("Stop when");ui.add(egui::DragValue::new(&mut draft.limit).range(1..=50_000));ui.label("violations found");});
                    ui.add_space(16.0);ui.strong("Split Plane DRC Report Options");ui.separator();
                    for label in ["Report Broken Planes","Report Dead Copper larger than","Report Starved Thermals with less than"] {ui.add_enabled(false,egui::Checkbox::new(&mut unavailable,label));}
                    ui.add_space(20.0);ui.label("Online DRC checks the board while you work. Batch checks run with Run Design Rule Check. Unimplemented checks are disabled and listed in the results.");
                } else {
                    let board=doc.model.pcb.as_ref().unwrap();
                    let mut kinds:BTreeSet<&str>=rules::KINDS.iter().map(|(_,_,k)|*k).collect();
                    kinds.extend(board.rules.iter().map(|r|text(r,"RULEKIND")));
                    egui::ScrollArea::vertical().id_salt("check_types").max_height(565.0).show(ui,|ui| {
                    egui::Grid::new("online_batch_rules").striped(true).min_col_width(60.0).show(ui,|ui| {
                        for label in ["Rule","Category","Online","Batch"] {ui.strong(label);}ui.end_row();
                        for kind in kinds {
                            if matches!(category(kind),"Mask"|"Plane") || (state.settings_page!="Rules To Check"&&category(kind)!=state.settings_page) {continue;}
                            let available=rules::supported(kind)&&board.rules.iter().any(|r|text(r,"RULEKIND")==kind);
                            ui.label(name(kind));ui.label(category(kind));
                            for (is_online,excluded) in [(true,&mut draft.online_excluded),(false,&mut draft.batch_excluded)] {
                                let mut checked=available&&!excluded.contains(kind);
                                if ui.add_enabled(available&&(!is_online||kind!="UnRoutedNet"),egui::Checkbox::without_text(&mut checked)).changed() {
                                    if checked {excluded.remove(kind);}else{excluded.insert(kind.into());}
                                }
                            }
                            ui.end_row();
                        }
                    });});
                }
            });
        });
        ui.separator();ui.horizontal(|ui| {
            if ui.button("Run Design Rule Check…").clicked(){save=true;run=true;}
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                if ui.button("Cancel").clicked(){cancel=true;}
                if ui.button("OK").clicked(){save=true;}
            });
        });
    });
    if save {
        state.settings = draft;
        state.generation = state.generation.wrapping_add(1);
        state.report = None;
        state.markers = true;
        if run {
            state.force = true;
            state.show_report = true;
        }
    } else if !cancel {
        state.setup = Some(draft);
    }
}
fn name(kind: &str) -> &str {
    rules::KINDS
        .iter()
        .find(|(_, _, k)| *k == kind)
        .map_or(kind, |(_, label, _)| *label)
}
fn insert_rule(d: &mut Draft, kind: &str, source: Option<usize>) {
    let mut indices: Vec<_> = d
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| text(r, "RULEKIND") == kind)
        .map(|(i, _)| i)
        .collect();
    indices.sort_by_key(|i| {
        text(&d.rules[*i], "PRIORITY")
            .parse::<u32>()
            .unwrap_or(u32::MAX)
    });
    let position = source
        .and_then(|s| indices.iter().position(|i| *i == s))
        .map_or(0, |n| n + 1);
    let mut rule = source
        .map(|i| d.rules[i].clone())
        .unwrap_or_else(|| rules::default_rule(kind));
    let base = text(&rule, "NAME").to_owned();
    let mut name = base.clone();
    let mut n = 1;
    while d
        .rules
        .iter()
        .any(|r| text(r, "RULEKIND") == kind && text(r, "NAME").eq_ignore_ascii_case(&name))
    {
        name = format!("{base}_{n}");
        n += 1;
    }
    rule.insert("NAME".into(), name);
    rule.insert("UNIQUEID".into(), Uuid::new_v4().to_string());
    let index = d.rules.len();
    d.rules.push(rule);
    indices.insert(position, index);
    for (priority, index) in indices.iter().enumerate() {
        d.rules[*index].insert("PRIORITY".into(), (priority + 1).to_string());
    }
    d.selected = index;
}
fn field(ui: &mut egui::Ui, r: &mut Fields, key: &str, label: &str) {
    let mut value = text(r, key).to_owned();
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 22.0], egui::Label::new(label));
        if ui
            .add_sized(
                [ui.available_width(), 22.0],
                egui::TextEdit::singleline(&mut value).id_salt(key),
            )
            .changed()
        {
            r.insert(key.into(), value);
        }
    });
}
fn dimension_field(ui: &mut egui::Ui, r: &mut Fields, key: &str, label: &str) {
    let raw = text(r, key);
    let mut value = if let Some(mm) = raw.strip_suffix("mm") {
        mm.to_owned()
    } else {
        rules::dimension(r, key)
            .map(|n| {
                format!("{:.6}", n as f64 / 1e6)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_owned()
            })
            .unwrap_or_else(|_| raw.to_owned())
    };
    ui.horizontal(|ui| {
        ui.add_sized([150.0, 22.0], egui::Label::new(label));
        if ui
            .add_sized(
                [180.0, 22.0],
                egui::TextEdit::singleline(&mut value).id_salt(key),
            )
            .changed()
        {
            let value = value.trim();
            let stored = if value
                .chars()
                .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
            {
                value.to_owned()
            } else {
                format!("{value}mm")
            };
            r.insert(key.into(), stored);
        }
        ui.label("mm");
    });
}
fn flag(ui: &mut egui::Ui, r: &mut Fields, key: &str, label: &str, default: bool) {
    let mut value = r.get(key).map_or(default, |v| v == "TRUE");
    if ui.checkbox(&mut value, label).changed() {
        r.insert(key.into(), if value { "TRUE" } else { "FALSE" }.into());
    }
}
fn scope_field(ui: &mut egui::Ui, r: &mut Fields, key: &str, label: &str, board: &Board) {
    ui.push_id(key, |ui| {
        egui::Frame::new()
            .fill(ui.visuals().widgets.inactive.bg_fill)
            .inner_margin(4.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.strong(label);
            });
        let expression = text(r, key).to_owned();
        let (inferred, value) = if expression.eq_ignore_ascii_case("All") {
            ("All", String::new())
        } else if let Some(v) = expression
            .strip_prefix("InNet('")
            .and_then(|s| s.strip_suffix("')"))
        {
            ("Net", v.replace("''", "'"))
        } else if let Some(v) = expression
            .strip_prefix("OnLayer('")
            .and_then(|s| s.strip_suffix("')"))
        {
            ("Layer", v.replace("''", "'"))
        } else {
            ("Custom Query", String::new())
        };
        let id = ui.id().with((text(r, "UNIQUEID"), "scope_mode"));
        let mut mode = ui
            .ctx()
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| inferred.into());
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(105.0);
                egui::ComboBox::from_id_salt("scope_type")
                    .width(100.0)
                    .selected_text(&mode)
                    .show_ui(ui, |ui| {
                        for m in ["All", "Net", "Layer", "Custom Query"] {
                            if ui.selectable_label(mode == m, m).clicked() {
                                mode = m.into();
                                let expression = match m {
                                    "All" => Some("All".into()),
                                    "Net" => board.nets.first().map(|n| {
                                        format!("InNet('{}')", n.name.replace('\'', "''"))
                                    }),
                                    "Layer" => board.layers.first().map(|l| {
                                        format!("OnLayer('{}')", l.name.replace('\'', "''"))
                                    }),
                                    _ => None,
                                };
                                if let Some(expression) = expression {
                                    r.insert(key.into(), expression);
                                }
                                ui.ctx().data_mut(|d| d.insert_temp(id, mode.clone()));
                            }
                        }
                    });
                if mode == "Custom Query" {
                    ui.add_enabled(false, egui::Button::new("Query Helper…"));
                    ui.add_enabled(false, egui::Button::new("Query Builder…"));
                }
            });
            if mode == "Custom Query" {
                let mut expression = text(r, key).to_owned();
                if ui
                    .add_sized(
                        [ui.available_width(), 80.0],
                        egui::TextEdit::multiline(&mut expression).font(egui::TextStyle::Monospace),
                    )
                    .changed()
                {
                    r.insert(key.into(), expression);
                }
            } else if mode == "Net" || mode == "Layer" {
                let options: Vec<_> = if mode == "Net" {
                    board.nets.iter().map(|n| n.name.as_str()).collect()
                } else {
                    board.layers.iter().map(|l| l.name.as_str()).collect()
                };
                egui::ComboBox::from_id_salt("scope_value")
                    .width(180.0)
                    .selected_text(value)
                    .show_ui(ui, |ui| {
                        for option in options {
                            if ui.selectable_label(false, option).clicked() {
                                r.insert(
                                    key.into(),
                                    format!(
                                        "{}('{}')",
                                        if mode == "Net" { "InNet" } else { "OnLayer" },
                                        option.replace('\'', "''")
                                    ),
                                );
                            }
                        }
                    });
            }
        });
        ui.add_space(9.0);
    });
}
fn rule_heading(ui: &mut egui::Ui, r: &mut Fields, board: &Board, status: &mut String) {
    ui.horizontal(|ui| {
        ui.label("Name");
        let mut name = text(r, "NAME").to_owned();
        if ui
            .add(egui::TextEdit::singleline(&mut name).desired_width(170.0))
            .changed()
        {
            r.insert("NAME".into(), name);
        }
        ui.label("Comment");
        let mut comment = text(r, "COMMENT").to_owned();
        if ui
            .add(
                egui::TextEdit::singleline(&mut comment)
                    .desired_width((ui.available_width() - 260.0).max(90.0)),
            )
            .changed()
        {
            r.insert("COMMENT".into(), comment);
        }
        ui.label("Unique ID");
        ui.add_sized(
            [92.0, 22.0],
            egui::Label::new(text(r, "UNIQUEID")).truncate(),
        );
        if ui.button("Test Queries").clicked() {
            let mut counts = Vec::new();
            for key in ["SCOPE1EXPRESSION", "SCOPE2EXPRESSION"] {
                let expression = text(r, key);
                let result: Result<Vec<_>, _> = board
                    .items
                    .iter()
                    .map(|i| {
                        rules::query(
                            board,
                            i,
                            if expression.is_empty() {
                                "All"
                            } else {
                                expression
                            },
                        )
                    })
                    .collect();
                match result {
                    Ok(v) => counts.push(v.iter().filter(|v| **v).count()),
                    Err(e) => {
                        *status = e;
                        return;
                    }
                }
            }
            *status = format!(
                "First query: {} objects. Second query: {} objects.",
                counts[0], counts[1]
            );
        }
    });
    ui.add_space(8.0);
}

fn matrix(ui: &mut egui::Ui, r: &mut Fields) {
    const GROUPS: &[(&str, &[&str])] = &[
        ("Track", &["Track", "Arc"]),
        ("SMD Pad", &["SMDPad"]),
        ("TH Pad", &["THPad"]),
        ("Via", &["Via"]),
        ("Copper", &["Fill", "Poly", "Region"]),
        ("Text", &["Text"]),
        ("Hole", &["Hole"]),
    ];
    ui.label("Object clearances [mm]");
    let mut values: BTreeMap<String, String> = text(r, "OBJECTCLEARANCES")
        .split(';')
        .filter_map(|s| s.split_once(':').map(|(k, v)| (k.into(), v.into())))
        .collect();
    let mut changed = false;
    egui::ScrollArea::horizontal()
        .id_salt("clearance_matrix_scroll")
        .show(ui, |ui| {
            egui::Grid::new("clearance_matrix")
                .striped(true)
                .show(ui, |ui| {
                    ui.label("");
                    for (label, _) in GROUPS {
                        ui.label(*label);
                    }
                    ui.end_row();
                    for (row, (label, a)) in GROUPS.iter().enumerate() {
                        ui.label(*label);
                        for (col, (_, b)) in GROUPS.iter().enumerate() {
                            if col > row {
                                ui.label("");
                                continue;
                            }
                            let key = format!("ClearanceObj_{}-ClearanceObj_{}", a[0], b[0]);
                            let rev = format!("ClearanceObj_{}-ClearanceObj_{}", b[0], a[0]);
                            let mut value = values
                                .get(&key)
                                .or_else(|| values.get(&rev))
                                .and_then(|v| v.parse::<f64>().ok())
                                .map_or_else(
                                    || rules::dimension(r, "GAP").unwrap_or(0) as f64 / 1e6,
                                    |n| n * 2.54 / 1e6,
                                );
                            if ui
                                .add(
                                    egui::DragValue::new(&mut value)
                                        .speed(0.01)
                                        .range(0.0..=100.0)
                                        .max_decimals(5),
                                )
                                .changed()
                            {
                                changed = true;
                                for x in *a {
                                    for y in *b {
                                        let key = format!("ClearanceObj_{x}-ClearanceObj_{y}");
                                        let rev = format!("ClearanceObj_{y}-ClearanceObj_{x}");
                                        values.remove(&rev);
                                        values
                                            .insert(key, (value * 1e6 / 2.54).round().to_string());
                                    }
                                }
                            }
                        }
                        ui.end_row();
                    }
                });
        });
    if changed {
        r.insert(
            "OBJECTCLEARANCES".into(),
            values
                .into_iter()
                .map(|(k, v)| format!("{k}:{v}"))
                .collect::<Vec<_>>()
                .join(";"),
        );
    }
}
fn apply(doc: &mut OpenDocument, d: &mut Draft) -> Result<(), String> {
    if doc.history.revision != d.revision || doc.pcb_editor.has_pending() {
        return Err("Board changed while Rules was open; cancel and reopen the editor".into());
    }
    let next = doc
        .model
        .pcb
        .as_ref()
        .ok_or("Not a PCB")?
        .with_rules(d.rules.clone())?;
    commit(doc, next)?;
    d.revision = doc.history.revision;
    d.original = d.rules.clone();
    d.error.clear();
    Ok(())
}
fn outline_matrix(ui: &mut egui::Ui, r: &mut Fields) {
    const OBJECTS: &[(&str, &[&str])] = &[
        ("Track", &["Track", "Arc"]),
        ("SMD Pad", &["SMDPad"]),
        ("TH Pad", &["THPad"]),
        ("Via", &["Via"]),
        ("Copper", &["Fill", "Poly", "Region"]),
        ("Text", &["Text"]),
        ("Hole", &["Hole"]),
    ];
    const EDGES: &[(&str, &str)] = &[
        ("Outline Edge", "OutlineEdge"),
        ("Cavity Edge", "CavityEdge"),
        ("Cutout Edge", "CutoutEdge"),
        ("Split Barrier", "SplitBarrier"),
        ("Split Continuation", "SplitContinuation"),
    ];
    let mut values: BTreeMap<String, String> = text(r, "OBJECTCLEARANCES")
        .split(';')
        .filter_map(|e| e.split_once(':').map(|(k, v)| (k.into(), v.into())))
        .collect();
    let mut changed = false;
    egui::Grid::new("outline_clearances")
        .striped(true)
        .show(ui, |ui| {
            ui.label("");
            for (label, _) in EDGES {
                ui.label(*label);
            }
            ui.end_row();
            for (label, objects) in OBJECTS {
                ui.label(*label);
                for (_, edge) in EDGES {
                    let mut value =
                        rules::gap_objects(r, objects[0], edge).unwrap_or(0) as f64 / 1e6;
                    let implemented = matches!(*edge, "OutlineEdge" | "CutoutEdge");
                    if ui
                        .add_enabled(
                            implemented,
                            egui::DragValue::new(&mut value)
                                .speed(0.01)
                                .range(0.0..=100.0)
                                .max_decimals(6),
                        )
                        .changed()
                    {
                        changed = true;
                        for object in *objects {
                            values.insert(
                                format!("ClearanceObj_{object}-ClearanceObj_{edge}"),
                                (value * 1e6 / 2.54).round().to_string(),
                            );
                        }
                    }
                }
                ui.end_row();
            }
        });
    if changed {
        r.insert(
            "OBJECTCLEARANCES".into(),
            values
                .into_iter()
                .map(|(k, v)| format!("{k}:{v}"))
                .collect::<Vec<_>>()
                .join(";"),
        );
    }
}
fn connection_preview(ui: &mut egui::Ui, r: &Fields, prefix: &str) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(150.0, 140.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let copper = Color32::from_rgb(225, 183, 32);
    let void = ui.visuals().panel_fill;
    let c = rect.center();
    let solid = text(r, &format!("{prefix}CONNECTSTYLE")) == "Direct";
    let relief = text(r, &format!("{prefix}CONNECTSTYLE")) == "Relief";
    painter.rect_filled(rect.shrink(5.0), 0.0, copper);
    let smd = prefix == "SMDPAD.";
    if !solid {
        if smd {
            painter.rect_filled(
                egui::Rect::from_center_size(c, egui::vec2(84.0, 84.0)),
                0.0,
                void,
            );
        } else {
            painter.circle_filled(c, 44.0, void);
        }
        if relief {
            let angle = if text(r, &format!("{prefix}POLYGONRELIEFANGLE")) == "45 Angle" {
                45.0_f32
            } else {
                90.0_f32
            };
            let count = if text(r, &format!("{prefix}RELIEFENTRIES")) == "2" {
                1
            } else {
                2
            };
            for n in 0..count {
                let a = (angle + n as f32 * 90.0).to_radians();
                let v = egui::vec2(a.cos(), a.sin()) * 48.0;
                painter.line_segment([c - v, c + v], egui::Stroke::new(12.0, copper));
            }
        }
        if smd {
            painter.rect_filled(
                egui::Rect::from_center_size(c, egui::vec2(48.0, 48.0)),
                0.0,
                copper,
            );
        } else {
            painter.circle_filled(c, 25.0, copper);
        }
    }
    if !smd {
        painter.circle_filled(c, 8.0, void);
    }
}
fn connection_column(ui: &mut egui::Ui, r: &mut Fields, prefix: &str, title: &str) {
    ui.push_id(prefix, |ui| {
        ui.strong(title);
        ui.add_space(8.0);
        let key = format!("{prefix}CONNECTSTYLE");
        ui.horizontal(|ui| {
            ui.label("Connect Style");
            let label = match text(r, &key) {
                "Direct" => "Direct Connect",
                "NoConnect" => "No Connect",
                _ => "Relief Connect",
            };
            egui::ComboBox::from_id_salt("style")
                .width(126.0)
                .selected_text(label)
                .show_ui(ui, |ui| {
                    for (value, label) in [
                        ("Relief", "Relief Connect"),
                        ("Direct", "Direct Connect"),
                        ("NoConnect", "No Connect"),
                    ] {
                        if ui.selectable_label(text(r, &key) == value, label).clicked() {
                            r.insert(key.clone(), value.into());
                        }
                    }
                });
        });
        ui.add_space(10.0);
        connection_preview(ui, r, prefix);
        ui.add_enabled_ui(text(r, &key) == "Relief", |ui| {
            // Widths fit inside each of the three native advanced columns.
            for (key, label) in [
                ("AIRGAPWIDTH", "Air Gap Width"),
                ("RELIEFCONDUCTORWIDTH", "Conductor Width"),
            ] {
                let key = format!("{prefix}{key}");
                ui.label(label);
                ui.horizontal(|ui| {
                    let mut value = rules::dimension(r, &key)
                        .map(|n| {
                            format!("{:.6}", n as f64 / 1e6)
                                .trim_end_matches('0')
                                .trim_end_matches('.')
                                .to_owned()
                        })
                        .unwrap_or_else(|_| text(r, &key).into());
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut value)
                                .id_salt(&key)
                                .desired_width(90.0),
                        )
                        .changed()
                    {
                        let value = if value.chars().any(|c| c.is_ascii_alphabetic()) {
                            value
                        } else {
                            format!("{value}mm")
                        };
                        r.insert(key, value);
                    }
                    ui.label("mm");
                });
            }
            ui.horizontal(|ui| {
                ui.label("Conductors");
                let key = format!("{prefix}RELIEFENTRIES");
                for value in ["2", "4"] {
                    if ui.radio(text(r, &key) == value, value).clicked() {
                        r.insert(key.clone(), value.into());
                    }
                }
                ui.add_enabled(
                    false,
                    egui::RadioButton::new(!matches!(text(r, &key), "2" | "4"), "Auto"),
                )
                .on_hover_text("Automatic placement of multiple conductors is not implemented");
            });
            ui.horizontal(|ui| {
                ui.label("Rotation");
                let key = format!("{prefix}POLYGONRELIEFANGLE");
                egui::ComboBox::from_id_salt("rotation")
                    .width(110.0)
                    .selected_text(text(r, &key))
                    .show_ui(ui, |ui| {
                        for value in ["45 Angle", "90 Angle"] {
                            if ui.selectable_label(text(r, &key) == value, value).clicked() {
                                r.insert(key.clone(), value.into());
                            }
                        }
                    });
            });
        });
    });
}
fn polygon_constraints(ui: &mut egui::Ui, r: &mut Fields) {
    let mut advanced = rules::advanced_connection(r);
    ui.horizontal(|ui| {
        ui.radio_value(&mut advanced, false, "Simple");
        ui.radio_value(&mut advanced, true, "Advanced");
    });
    rules::set_advanced_connection(r, advanced);
    if advanced {
        ui.columns(3, |columns| {
            for (ui, (prefix, title)) in columns.iter_mut().zip([
                ("THPAD.", "Through Hole Pad Connection"),
                ("SMDPAD.", "SMD Pad Connection"),
                ("VIA.", "Via Connection"),
            ]) {
                ui.group(|ui| connection_column(ui, r, prefix, title));
            }
        });
    } else {
        ui.group(|ui| connection_column(ui, r, "", "Polygon Connection"));
    }
}
fn editor(doc: &mut OpenDocument, ctx: &egui::Context, d: &mut Draft) -> bool {
    let board = doc.model.pcb.as_ref().unwrap();
    let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut save = false;
    let mut ok = false;
    egui::Window::new(egui::RichText::new("PCB Rules and Constraints Editor [mm]").size(13.0)).id(egui::Id::new(("pcb_rules_v2",doc.session_id)))
        .fixed_size([1280.0,760.0]).default_pos(ctx.screen_rect().center()-egui::vec2(640.0,400.0)).min_width(800.0).collapsible(false).show(ctx,|ui| {
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(egui::vec2(250.0,690.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                ui.add(egui::TextEdit::singleline(&mut d.search).hint_text("Search"));
                egui::ScrollArea::vertical().id_salt("rules_tree").max_height(650.0).show(ui,|ui| {
                    ui.strong("Design Rules");
                    for category_name in ["Electrical","Routing","SMT","Mask","Plane","Testpoint","Manufacturing","High Speed","Placement","Signal Integrity"] {
                        egui::CollapsingHeader::new(category_name).default_open(category_name=="Electrical").open((!d.search.is_empty()).then_some(true)).show(ui,|ui| {
                            let mut kinds:BTreeSet<String>=d.rules.iter().filter(|r|category(text(r,"RULEKIND"))==category_name).map(|r|text(r,"RULEKIND").into()).collect();
                            kinds.extend(rules::KINDS.iter().filter(|(c,_,_)|*c==category_name).map(|(_,_,k)|k.to_string()));
                            for kind in kinds {
                                egui::CollapsingHeader::new(name(&kind)).id_salt(&kind).default_open(kind=="Clearance").open((!d.search.is_empty()).then_some(true)).show(ui,|ui| {
                                    let mut rows:Vec<_>=d.rules.iter().enumerate().filter(|(_,r)|text(r,"RULEKIND")==kind).collect();
                                    rows.sort_by_key(|(i,r)|(text(r,"PRIORITY").parse::<u32>().unwrap_or(u32::MAX),*i));
                                    for (i,r) in rows {if text(r,"NAME").to_lowercase().contains(&d.search.to_lowercase())&&ui.selectable_label(d.selected==i,format!("{}{}",if rules::enabled(r){""}else{"[off] "},text(r,"NAME"))).clicked(){d.selected=i;}}
                                    if rules::supported(&kind)&&ui.small_button("New Rule").clicked(){insert_rule(d,&kind,None);}

                                });
                            }
                        });
                    }
                });
            });
            ui.separator();
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(),690.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                egui::ScrollArea::vertical().id_salt("rule_constraints").max_height(680.0).show(ui,|ui| {
                    if let Some(r)=d.rules.get_mut(d.selected) {
                        let kind=text(r,"RULEKIND").to_owned();let supported=rules::supported(&kind);
                        if !supported{ui.colored_label(Color32::YELLOW,"This imported rule type is preserved, but its editor/checker is not implemented.");}
                        ui.add_enabled_ui(supported,|ui| {
                            rule_heading(ui,r,board,&mut d.error);
                            scope_field(ui,r,"SCOPE1EXPRESSION","Where The First Object Matches",board);
                            if matches!(kind.as_str(),"Clearance"|"ShortCircuit"|"HoleToHoleClearance"|"PolygonConnect"|"MinimumSolderMaskSliver"|"SilkToSolderMaskClearance"){scope_field(ui,r,"SCOPE2EXPRESSION","Where The Second Object Matches",board);}
                            ui.separator();ui.horizontal(|ui|{ui.strong("Constraints");flag(ui,r,"ENABLED","Enabled",true);});
                            if kind=="PolygonConnect" {polygon_constraints(ui,r);}
                            if kind=="BoardOutlineClearance" {outline_matrix(ui,r);}
                            if kind=="UnpouredPolygon"{flag(ui,r,"ALLOWSHELVED","Allow Shelved Polygons",false);flag(ui,r,"ALLOWUNPOURED","Allow Unpoured Polygons",false);}

                            let previous_gap=text(r,"GAP").to_owned();
                            for (key,label) in rules::fields(&kind){dimension_field(ui,r,key,label);}
                            if matches!(kind.as_str(),"Clearance"|"BoardOutlineClearance") && text(r,"GAP")!=previous_gap && let Ok(gap)=rules::dimension(r,"GAP") {
                                let value=(gap as f64/2.54).round().to_string();
                                let matrix=text(r,"OBJECTCLEARANCES").split(';').filter_map(|entry|entry.split_once(':').map(|(k,_)|format!("{k}:{value}"))).collect::<Vec<_>>().join(";");
                                if !matrix.is_empty(){r.insert("OBJECTCLEARANCES".into(),matrix);}
                            }
                            if kind=="PasteMaskExpansion"{field(ui,r,"PERCENTAGE","Percentage");}
                            if kind=="SolderMaskExpansion"{flag(ui,r,"ISTENTINGTOP","Tented - Top",false);flag(ui,r,"ISTENTINGBOTTOM","Tented - Bottom",false);}
                            if kind=="ShortCircuit"{flag(ui,r,"ALLOWED","Allow Short Circuit",false);}
                            if kind=="SilkToSolderMaskClearance"{
                                let mut exposed=text(r,"CLEARANCETOEXPOSEDCOPPER")=="TRUE";
                                let before=exposed;
                                ui.radio_value(&mut exposed,true,"Check Clearance To Exposed Copper");
                                ui.radio_value(&mut exposed,false,"Check Clearance To Solder Mask Openings");
                                if exposed!=before {r.insert("CLEARANCETOEXPOSEDCOPPER".into(),if exposed{"TRUE"}else{"FALSE"}.into());}
                                ui.label("Text glyph outlines are not yet checked; the report lists this limitation.");
                            }
                            if kind=="Clearance"{
                                flag(ui,r,"IGNOREPADTOPADCLEARANCEINFOOTPRINT","Ignore Pad to Pad clearances within a footprint",false);
                                ui.horizontal(|ui|{ui.label("Net scope");egui::ComboBox::from_id_salt("net_scope").selected_text(text(r,"NETSCOPE")).show_ui(ui,|ui|{for s in ["DifferentNets","SameNet","AnyNet"]{if ui.selectable_label(text(r,"NETSCOPE")==s,s).clicked(){r.insert("NETSCOPE".into(),s.into());}}});});
                                matrix(ui,r);
                            }
                            if kind=="RoutingLayers"{for layer in board.layers.iter().filter(|l|l.copper){flag(ui,r,&format!("{}_V5",pcb::layer_name(layer.id).to_ascii_uppercase()),&layer.name,true);}}
                            if kind=="Width"{for layer in board.layers.iter().filter(|l|l.copper){let prefix=match layer.id{pcb::TOP=>"TOPLAYER".into(),pcb::BOTTOM=>"BOTTOMLAYER".into(),n=>format!("MIDLAYER{}",n-1)};for suffix in ["MINWIDTH","MAXWIDTH"]{let key=format!("{prefix}_{suffix}");if r.contains_key(&key){dimension_field(ui,r,&key,&format!("{} {suffix}",layer.name));}}}}
                        });
                        if !supported{for (k,v) in r.iter(){ui.horizontal_wrapped(|ui|{ui.label(k);ui.monospace(v);});}}
                    } else {ui.label("Select a rule or create a rule in the tree.");}
                });
            });
        });
        if !d.error.is_empty(){ui.colored_label(Color32::YELLOW,&d.error);}
        ui.separator();ui.horizontal(|ui| {
            ui.add_enabled(false,egui::Button::new("Switch to Document View"));ui.add_enabled(false,egui::Button::new("Rule Wizard…"));
            if ui.button("Priorities…").clicked(){d.priorities=true;}
            if ui.button("Create Default Rules").clicked(){for (_,_,kind) in rules::KINDS{if !d.rules.iter().any(|r|text(r,"RULEKIND")==*kind){d.rules.push(rules::default_rule(kind));}}}
            let editable=d.rules.get(d.selected).is_some_and(|r|rules::supported(text(r,"RULEKIND")));
            if ui.add_enabled(editable,egui::Button::new("Duplicate")).clicked(){let kind=text(&d.rules[d.selected],"RULEKIND").to_owned();insert_rule(d,&kind,Some(d.selected));}

            if ui.add_enabled(editable,egui::Button::new("Delete")).clicked(){d.rules.remove(d.selected);d.selected=d.selected.saturating_sub(1);}
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button("Apply").clicked(){save=true;}
if ui.button("Cancel").clicked(){close=true;}
if ui.button("OK").clicked(){save=true;ok=true;}});
        });
    });
    if d.priorities {
        egui::Window::new("Edit Rule Priorities")
            .collapsible(false)
            .show(ctx, |ui| {
                if let Some(selected) = d.rules.get(d.selected) {
                    let kind = text(selected, "RULEKIND").to_owned();
                    let mut indices: Vec<_> = d
                        .rules
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| text(r, "RULEKIND") == kind)
                        .map(|(i, _)| i)
                        .collect();
                    indices.sort_by_key(|i| {
                        text(&d.rules[*i], "PRIORITY")
                            .parse::<u32>()
                            .unwrap_or(u32::MAX)
                    });
                    for i in &indices {
                        if ui
                            .selectable_label(
                                d.selected == *i,
                                format!(
                                    "{}  {}",
                                    text(&d.rules[*i], "PRIORITY"),
                                    text(&d.rules[*i], "NAME")
                                ),
                            )
                            .clicked()
                        {
                            d.selected = *i;
                        }
                    }
                    ui.horizontal(|ui| {
                        for (label, delta) in
                            [("Increase Priority", -1isize), ("Decrease Priority", 1)]
                        {
                            if ui.button(label).clicked()
                                && let Some(n) = indices.iter().position(|i| *i == d.selected)
                            {
                                let target = n as isize + delta;
                                if (0..indices.len() as isize).contains(&target) {
                                    indices.swap(n, target as usize);
                                    for (priority, i) in indices.iter().enumerate() {
                                        d.rules[*i]
                                            .insert("PRIORITY".into(), (priority + 1).to_string());
                                    }
                                }
                            }
                        }
                    });
                }
                if ui.button("Close").clicked() {
                    d.priorities = false;
                }
            });
    }
    if save {
        match apply(doc, d) {
            Ok(()) => close |= ok,
            Err(e) => d.error = e,
        }
    }
    close
}
pub(super) fn show(doc: &mut OpenDocument, ctx: &egui::Context) {
    let mut state = std::mem::take(&mut doc.pcb_editor.rules);
    checker_settings(doc, ctx, &mut state);
    if let Some(mut draft) = state.editor.take()
        && !editor(doc, ctx, &mut draft)
    {
        state.editor = Some(draft);
    }
    let token = (
        doc.history.revision,
        doc.view.pcb.preview_generation(),
        state.generation,
    );
    if let Some(receiver) = &state.pending {
        match receiver.try_recv() {
            Ok((version, report)) => {
                if version == token {
                    state.report = Some((version, report));
                }
                state.pending = None;
            }
            Err(mpsc::TryRecvError::Disconnected) => state.pending = None,
            Err(mpsc::TryRecvError::Empty) => {}
        }
    }
    let board = doc.view.pcb.preview().or(doc.model.pcb.as_ref()).unwrap();
    if state.pending.is_none()
        && (state.force
            || (state.online
                && !board.rules.is_empty()
                && state.report.as_ref().is_none_or(|(v, _)| *v != token)
                && state.last_start.elapsed() > Duration::from_millis(500)))
    {
        let board = board.clone();
        let options = rules::CheckOptions {
            excluded: if state.force {
                state.settings.batch_excluded.clone()
            } else {
                state.settings.online_excluded.clone()
            },
            limit: state.settings.limit,
        };
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let report = rules::check_with_options(&board, &options);
            let _ = tx.send((token, report));
            ctx.request_repaint();
        });
        state.pending = Some(rx);
        state.last_start = Instant::now();
        state.force = false;
    }
    doc.view.pcb.drc_markers = if state.markers && state.settings.create_violations {
        state
            .report
            .as_ref()
            .filter(|(v, _)| *v == token)
            .map(|(_, r)| r.violations.iter().map(|v| (v.position, v.layer)).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    if state.pending.is_some() || state.online {
        ctx.request_repaint_after(Duration::from_millis(250));
    }
    if state.show_report {
        egui::Window::new("PCB Rules And Violations")
            .default_size([850.0, 500.0])
            .min_width(700.0)
            .open(&mut state.show_report)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Run Design Rule Check").clicked() {
                        state.force = true;
                    }
                    ui.checkbox(&mut state.online, "Online DRC");
                    ui.checkbox(&mut state.markers, "Show violations");
                    if ui
                        .add_enabled(
                            state.report.as_ref().is_some_and(|(v, _)| *v == token),
                            egui::Button::new("Export Report…"),
                        )
                        .clicked()
                        && let Some((_, report)) = &state.report
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Design Rule Verification Report", &["html"])
                            .set_file_name("Design Rule Check.html")
                            .save_file()
                        && let Err(e) = std::fs::write(
                            path,
                            report.html(
                                doc.view.pcb.preview().or(doc.model.pcb.as_ref()).unwrap(),
                                &doc.model.name,
                            ),
                        )
                    {
                        doc.pcb_editor.last_error =
                            Some(format!("Could not export DRC report: {e}"));
                    }
                });
                if state.pending.is_some() {
                    ui.label("Checking…");
                }
                if let Some((version, report)) = &state.report {
                    if *version != token {
                        ui.colored_label(
                            Color32::YELLOW,
                            "Results are stale; board or preview changed.",
                        );
                    }
                    ui.label(format!(
                        "{} violations; {} unchecked conditions{}",
                        report.violations.len(),
                        report.diagnostics.len(),
                        if report.truncated {
                            "; report incomplete"
                        } else {
                            ""
                        }
                    ));
                    ui.add(
                        egui::TextEdit::singleline(&mut state.filter)
                            .hint_text("Filter violations"),
                    );
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .show(ui, |ui| {
                            egui::CollapsingHeader::new(format!(
                                "Unchecked conditions ({})",
                                report.diagnostics.len()
                            ))
                            .default_open(false)
                            .show(ui, |ui| {
                                for message in &report.diagnostics {
                                    ui.colored_label(Color32::YELLOW, message);
                                }
                            });
                            for v in &report.violations {
                                let rule = doc
                                    .model
                                    .pcb
                                    .as_ref()
                                    .and_then(|b| b.rules.get(v.rule))
                                    .map_or(v.kind.as_str(), |r| text(r, "NAME"));
                                let label = format!("{rule}: {}", v.message);
                                if label.to_lowercase().contains(&state.filter.to_lowercase())
                                    && ui
                                        .add_enabled(
                                            *version == token,
                                            egui::Button::selectable(false, label),
                                        )
                                        .clicked()
                                {
                                    doc.view.pcb.selected = v.items.clone();
                                    doc.view.camera.centre_mm =
                                        [v.position.x as f64 / 1e6, v.position.y as f64 / 1e6];
                                    if v.layer != pcb::MULTI_LAYER {
                                        doc.view.pcb.active_layer = v.layer;
                                        doc.view.pcb.visible.insert(v.layer);
                                    }
                                }
                            }
                        });
                } else {
                    ui.label("No current check results.");
                }
            });
    }
    doc.pcb_editor.rules = state;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> OpenDocument {
        let mut model = Document::new(DocumentKind::Board, "rules.PcbDoc");
        model.pcb = Some(Board {
            rules: vec![rules::default_rule("Width")],
            ..Default::default()
        });
        OpenDocument::new(model, None)
    }
    #[test]
    fn viewing_rules_is_read_only_and_apply_is_one_undoable_edit() {
        let mut doc = document();
        let before = doc.model.clone();
        open(&mut doc);
        let mut draft = doc.pcb_editor.rules.editor.take().unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(!editor(&mut doc, ctx, &mut draft));
        });
        assert_eq!(draft.rules, draft.original);
        assert_eq!(doc.model, before);
        draft.rules[0].insert("PREFEREDWIDTH".into(), "0.5mm".into());
        apply(&mut doc, &mut draft).unwrap();
        assert_eq!(doc.history.revision, 1);
        doc.history.undo(&mut doc.model);
        assert_eq!(doc.model, before);
    }
    #[test]
    fn stale_rule_drafts_cannot_replace_newer_board_edits() {
        let mut doc = document();
        open(&mut doc);
        let mut draft = doc.pcb_editor.rules.editor.take().unwrap();
        draft.rules[0].insert("PREFEREDWIDTH".into(), "0.5mm".into());
        let mut board = doc.model.pcb.clone().unwrap();
        board.rules[0].insert("PREFEREDWIDTH".into(), "0.6mm".into());
        doc.history.commit_pcb(&mut doc.model, board).unwrap();
        let before = doc.model.clone();
        assert!(apply(&mut doc, &mut draft).is_err());
        assert_eq!(before, doc.model);
    }
    #[test]
    fn checker_settings_roundtrip_keeps_independent_online_and_batch_selections() {
        let settings = CheckSettings {
            online_excluded: BTreeSet::from(["Width".into()]),
            batch_excluded: BTreeSet::from(["Clearance".into()]),
            limit: 37,
            create_violations: false,
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(
            serde_json::from_str::<CheckSettings>(&json).unwrap(),
            settings
        );
        assert_eq!(
            serde_json::from_str::<CheckSettings>("{}").unwrap(),
            CheckSettings::default()
        );
    }
    #[test]
    fn new_rules_take_highest_priority_and_copies_follow_source() {
        let mut doc = document();
        open(&mut doc);
        let mut d = doc.pcb_editor.rules.editor.take().unwrap();
        insert_rule(&mut d, "Width", None);
        let new = d.selected;
        assert_eq!(text(&d.rules[new], "PRIORITY"), "1");
        assert_eq!(text(&d.rules[0], "PRIORITY"), "2");
        insert_rule(&mut d, "Width", Some(new));
        assert_eq!(text(&d.rules[d.selected], "PRIORITY"), "2");
        assert_eq!(text(&d.rules[0], "PRIORITY"), "3");
        assert_ne!(
            text(&d.rules[d.selected], "UNIQUEID"),
            text(&d.rules[new], "UNIQUEID")
        );
        apply(&mut doc, &mut d).unwrap();
        assert_eq!(doc.history.revision, 1);
    }
    #[test]
    fn delayed_drc_results_for_old_settings_are_discarded() {
        let mut doc = document();
        doc.pcb_editor.rules.online = false;
        let (tx, rx) = mpsc::channel();
        doc.pcb_editor.rules.pending = Some(rx);
        doc.pcb_editor.rules.generation = 1;
        tx.send(((0, doc.view.pcb.preview_generation(), 0), Report::default()))
            .unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show(&mut doc, ctx));
        assert!(doc.pcb_editor.rules.report.is_none());
        assert!(doc.view.pcb.drc_markers.is_empty());
    }
    #[test]
    fn rules_and_checker_windows_fit_screen_with_all_rule_categories() {
        let mut doc = document();
        doc.model.pcb.as_mut().unwrap().rules = rules::KINDS
            .iter()
            .map(|(_, _, k)| rules::default_rule(k))
            .collect();
        doc.pcb_editor.rules.online = false;
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1920.0, 1030.0),
            )),
            ..Default::default()
        };
        open(&mut doc);
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| show(&mut doc, ctx));
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::new(("pcb_rules_v2", doc.session_id))))
            .unwrap();
        assert!(rect.width() <= 1320.0 && rect.height() <= 850.0, "{rect:?}");
        // Exercise the advanced three-column layout with the tall custom scopes.
        let draft = doc.pcb_editor.rules.editor.as_mut().unwrap();
        draft.selected = draft
            .rules
            .iter()
            .position(|r| text(r, "RULEKIND") == "PolygonConnect")
            .unwrap();
        let r = &mut draft.rules[draft.selected];
        rules::set_advanced_connection(r, true);
        r.insert("SCOPE1EXPRESSION".into(), "HasFootprint('QFN')".into());
        r.insert("SCOPE2EXPRESSION".into(), "IsNamedPolygon('GND')".into());
        let original = draft.rules.clone();
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| show(&mut doc, ctx));
        }
        assert_eq!(
            doc.pcb_editor.rules.editor.as_ref().unwrap().rules,
            original
        );
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::new(("pcb_rules_v2", doc.session_id))))
            .unwrap();
        assert!(rect.width() <= 1320.0 && rect.height() <= 850.0, "{rect:?}");
        doc.pcb_editor.rules.editor = None;
        request_check(&mut doc);
        for _ in 0..2 {
            let _ = ctx.run(input.clone(), |ctx| show(&mut doc, ctx));
        }
        let rect = ctx
            .memory(|m| m.area_rect(egui::Id::new(("drc_setup_v2", doc.session_id))))
            .unwrap();
        assert!(rect.width() <= 850.0 && rect.height() <= 750.0, "{rect:?}");
    }
}
