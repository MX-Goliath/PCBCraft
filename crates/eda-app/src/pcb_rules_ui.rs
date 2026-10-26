//! Rule drafts and asynchronous DRC belong to the current document session.
use super::*;
use eda_core::pcb::Fields;
use eda_core::pcb_rules::{self as rules, Report, text};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
#[path = "pcb_rules_query.rs"]
mod query_ui;
#[path = "pcb_rules_widgets.rs"]
mod widgets;
#[path = "pcb_rules_wizard.rs"]
mod wizard_ui;
use widgets::{CATALOG, RulePage};
pub(super) struct Draft {
    revision: u64,
    original: Vec<Fields>,
    rules: Vec<Fields>,
    selected: usize,
    search: String,
    error: String,
    priorities: bool,
    page: RulePage,
    expanded: BTreeSet<String>,
    priority_kind: String,
    query: Option<query_ui::QueryDraft>,
    wizard: Option<wizard_ui::Wizard>,
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
            page: RulePage::Rule,
            expanded: BTreeSet::from([
                "Design Rules".into(),
                "Electrical".into(),
                "Clearance".into(),
            ]),
            priority_kind: "Clearance".into(),
            query: None,
            wizard: None,
        });
    }
}
pub(super) fn request_check(doc: &mut OpenDocument) {
    let state = &mut doc.pcb_editor.rules;
    state.setup = Some(state.settings.clone());
}
pub(super) fn open_wizard(doc: &mut OpenDocument) {
    if let Some(draft) = &mut doc.pcb_editor.rules.editor {
        draft.wizard = Some(wizard_ui::Wizard::default());
    }
}
pub(super) fn browse(doc: &mut OpenDocument) {
    doc.pcb_editor.rules.show_report = true;
}
pub(super) fn reset_markers(doc: &mut OpenDocument) {
    doc.pcb_editor.rules.markers = false;
    doc.view.pcb.drc_markers.clear();
}
fn category(kind: &str) -> &str {
    if let Some((cat, _, _)) = CATALOG.iter().find(|(_, _, k)| *k == kind) {
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

fn disabled_check(ui: &mut egui::Ui, label: &str, value: bool) {
    let mut checked = value;
    ui.add_enabled(false, egui::Checkbox::new(&mut checked, label))
        .on_hover_text("This check is not implemented");
}
fn checker_settings(doc: &OpenDocument, ctx: &egui::Context, state: &mut State) {
    let Some(mut draft) = state.setup.take() else {
        return;
    };
    let mut save = false;
    let mut cancel = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut run = false;
    egui::Window::new("Design Rule Checker [mm]").id(egui::Id::new(("drc_setup_v2",doc.session_id))).title_bar(false).fixed_size([752.0,684.0]).resizable(false).collapsible(false)
        .movable(false).current_pos(widgets::position(ctx,egui::Id::new(("drc_setup_v2",doc.session_id)),ctx.screen_rect().center()-egui::vec2(384.0,350.0)))
        .frame(egui::Frame::new().fill(Color32::from_gray(76)).inner_margin(7.0).stroke(egui::Stroke::new(1.0,widgets::LINE))).show(ctx,|ui|{
            widgets::style(ui);cancel|=widgets::title(ui,"Design Rule Checker [mm]");
            let(body,_)=ui.allocate_exact_size(egui::vec2(ui.available_width(),613.0),egui::Sense::hover());ui.painter().rect_filled(body,0.0,widgets::BG);
            let left=egui::Rect::from_min_size(body.min+egui::vec2(15.0,15.0),egui::vec2(181.0,590.0));
            widgets::child(ui, egui::UiBuilder::new().max_rect(left),|ui|{
                for label in ["Report Options","Rules To Check"]{let(click,_,_)=widgets::tree_row(ui,label,label,0,"Manufacturing",state.settings_page==label,None);
if click{state.settings_page=label.into();}}
                for cat in CATEGORIES.iter().filter(|c|!matches!(**c,"Mask"|"Plane")){let(click,_,_)=widgets::tree_row(ui,cat,cat,1,cat,state.settings_page==*cat,None);
if click{state.settings_page=(*cat).into();}}
            });
            let right=egui::Rect::from_min_max(body.min+egui::vec2(209.0,15.0),body.max-egui::vec2(18.0,8.0));
            widgets::child(ui, egui::UiBuilder::new().max_rect(right),|ui|{
                if state.settings_page=="Report Options"{
                    widgets::section(ui,"DRC Report Options");ui.add_space(11.0);
                    ui.indent("report_checkboxes",|ui|{
                        disabled_check(ui,"Create Report File",true);
                        ui.indent("health",|ui|disabled_check(ui,"Report PCB Health Issues",false));
                        ui.checkbox(&mut draft.create_violations,"Create Violations");
                        for(label,value)in[("Sub-Net Details",true),("Verify Shorting Copper",true),("Report Drilled SMT Pads",false),("Report Multilayer Pads with 0 size Hole",true)]{disabled_check(ui,label,value);}
                        ui.horizontal(|ui|{ui.label("Stop when");ui.add(egui::DragValue::new(&mut draft.limit).range(1..=50_000));ui.label("violations found");});
                    });
                    ui.add_space(10.0);widgets::section(ui,"Split Plane DRC Report Options");ui.add_space(10.0);
                    ui.indent("split_options",|ui|{
                        disabled_check(ui,"Report Broken Planes",true);
                        for(label,value,suffix)in[("Report Dead Copper larger than","0.065 sq. mm",""),("Report Starved Thermals with less than","50%","available copper")]{ui.horizontal(|ui|{disabled_check(ui,label,true);let mut value=value.to_owned();ui.add_enabled(false,egui::TextEdit::singleline(&mut value).desired_width(110.0));ui.label(suffix);});}
                        ui.add_space(10.0);ui.label("Note: To generate Report File you must save your PCB document first.\nTo speed the process of rule checking enable only the rules that are required for the task being performed.");
                        ui.add_space(14.0);ui.label("Note: Options are only enabled when corresponding rules have been defined.\nOn line DRC tests for design rule violations as you work. Include a Design Rule in the Design-Rules dialog to be able to test for a particular rule type.");
                    });
                }else{
                    let board=doc.model.pcb.as_ref().unwrap();
                    let mut kinds:Vec<String>=CATALOG.iter().filter(|(cat,_,_)|!matches!(*cat,"Mask"|"Plane")&&(state.settings_page=="Rules To Check"||state.settings_page==*cat)).map(|(_,_,k)|(*k).into()).collect();
                    for r in &board.rules{let k=text(r,"RULEKIND");
if !matches!(category(k),"Mask"|"Plane")&&(state.settings_page=="Rules To Check"||state.settings_page==category(k))&&!kinds.iter().any(|v|v==k){kinds.push(k.into());}}
                    kinds.sort_by_key(|k|name(k).to_owned());
                    let widths=[right.width()*0.38,right.width()*0.38,right.width()*0.13,right.width()*0.11];
                    egui::Frame::new().fill(Color32::from_gray(58)).stroke(egui::Stroke::new(1.0,widgets::LINE)).show(ui,|ui|{
                        widgets::table_header(ui,&widths,&["Rule","Category","Online","Batch"]);
                        egui::ScrollArea::vertical().id_salt("check_types").max_height(570.0).auto_shrink([false,false]).show(ui,|ui|{ui.spacing_mut().item_spacing.y=0.0;ui.set_min_height(570.0);
                            for kind in kinds{let available=rules::supported(&kind)&&board.rules.iter().any(|r|text(r,"RULEKIND")==kind);
                                let(rect,response)=ui.allocate_exact_size(egui::vec2(right.width(),21.3),egui::Sense::click());let mut x=rect.left();
                                response.context_menu(|ui|{for (label,value) in [("Enable All",true),("Disable All",false)]{if ui.button(label).clicked(){for(_,_,k)in CATALOG.iter().filter(|(_,_,k)|rules::supported(k)){for excluded in [&mut draft.online_excluded,&mut draft.batch_excluded]{if value{excluded.remove(*k);}else{excluded.insert((*k).into());}}}ui.close();}}});
                                for(col,w)in widths.iter().enumerate(){let cell=egui::Rect::from_min_size(egui::pos2(x,rect.top()),egui::vec2(*w,21.3));widgets::cell(ui,cell,|ui|{if col<2{if col==0{let(icon,_)=ui.allocate_exact_size(egui::vec2(17.0,17.0),egui::Sense::hover());widgets::icon(ui.painter(),icon,category(&kind));}ui.add(egui::Label::new(if col==0{name(&kind)}else{category(&kind)}).truncate());}else{let online=col==2;
if online&&kind=="UnRoutedNet"{return;}let excluded=if online{&mut draft.online_excluded}else{&mut draft.batch_excluded};let mut checked=available&&!excluded.contains(&kind);
if ui.add_enabled(available,egui::Checkbox::without_text(&mut checked)).on_hover_text(if available{""}else{"This check is not implemented or has no rule"}).changed(){if checked{excluded.remove(&kind);}else{excluded.insert(kind.clone());}}}});x+=w;}
                            }
                        });
                    });
                }
            });
            ui.add_space(4.0);ui.horizontal(|ui|{
                if ui.add_sized([158.0,25.0],egui::Button::new("Run Design Rule Check…")).clicked(){save=true;run=true;}
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.add_sized([74.0,25.0],egui::Button::new("Cancel")).clicked(){cancel=true;}
if ui.add_sized([74.0,25.0],egui::Button::new("OK")).clicked(){save=true;}});
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
    CATALOG
        .iter()
        .find(|(_, _, k)| *k == kind)
        .map_or(kind, |(_, label, _)| *label)
}
fn insert_rule(d: &mut Draft, kind: &str, source: Option<usize>) {
    let source = source.filter(|i| d.rules.get(*i).is_some_and(|r| text(r, "RULEKIND") == kind));
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
    d.expanded.insert(category(kind).into());
    d.expanded.insert(kind.into());
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
                format!("{:.5}", n as f64 / 1e6)
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

const SCOPE_MODES: &[(&str, &str)] = &[
    ("All", ""),
    ("Net", "InNet"),
    ("Net Class", "InNetClass"),
    ("Layer", "OnLayer"),
    ("Component", "InComponent"),
    ("Component Class", "InComponentClass"),
    ("Pad Class", "InPadClass"),
    ("Custom Query", ""),
];
fn scope_modes(kind: &str) -> &'static [(&'static str, &'static str)] {
    if matches!(
        kind,
        "HoleSize" | "MinimumAnnularRing" | "HoleToHoleClearance"
    ) {
        &[
            ("All", ""),
            ("Pad", "IsPad"),
            ("Via", "IsVia"),
            ("Layer Stack", ""),
            ("Drill Pair", ""),
            ("Custom Query", ""),
        ]
    } else {
        SCOPE_MODES
    }
}
fn scope_field(
    ui: &mut egui::Ui,
    r: &mut Fields,
    key: &str,
    label: &str,
    board: &Board,
    query: &mut Option<query_ui::QueryDraft>,
) {
    ui.push_id(key, |ui| {
        widgets::section(ui, label);
        ui.add_space(5.0);
        let expression = text(r, key).to_owned();
        let modes = scope_modes(text(r, "RULEKIND"));
        let (inferred, value) = if expression.eq_ignore_ascii_case("All") {
            ("All", String::new())
        } else {
            modes
                .iter()
                .filter(|(_, f)| !f.is_empty())
                .find_map(|(mode, f)| {
                    if f.starts_with("Is") && expression.eq_ignore_ascii_case(f) {
                        return Some((*mode, String::new()));
                    }
                    expression
                        .strip_prefix(&format!("{f}('"))
                        .and_then(|v| v.strip_suffix("')"))
                        .map(|v| (*mode, v.replace("''", "'")))
                })
                .unwrap_or(("Custom Query", String::new()))
        };
        let id = ui.id().with((text(r, "UNIQUEID"), "scope_mode"));
        let mut mode = ui
            .ctx()
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| inferred.into());
        ui.horizontal_top(|ui| {
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.set_width(105.0);
                egui::ComboBox::from_id_salt("scope_type")
                    .width(105.0)
                    .selected_text(&mode)
                    .show_ui(ui, |ui| {
                        for (m, function) in modes {
                            if matches!(*m, "Layer Stack" | "Drill Pair") {
                                ui.add_enabled(false, egui::Button::new(*m));
                                continue;
                            }
                            if ui.selectable_label(mode == *m, *m).clicked() {
                                mode = (*m).into();
                                let expression = match *m {
                                    "All" => Some("All".into()),
                                    "Custom Query" => None,
                                    "Pad" | "Via" => Some((*function).into()),
                                    _ => query_ui::values(board, function).first().map(|v| {
                                        format!("{function}('{}')", v.replace('\'', "''"))
                                    }),
                                };
                                if let Some(expression) = expression {
                                    r.insert(key.into(), expression);
                                }
                                ui.ctx().data_mut(|d| d.insert_temp(id, mode.clone()));
                            }
                        }
                    });
                if mode == "Custom Query" {
                    for (label, builder) in [("Query Helper…", false), ("Query Builder…", true)]
                    {
                        if ui
                            .add_sized([105.0, 25.0], egui::Button::new(label))
                            .clicked()
                        {
                            *query = Some(query_ui::QueryDraft::new(key, text(r, key), builder));
                        }
                    }
                }
            });
            if mode == "Custom Query" {
                let mut expression = text(r, key).to_owned();
                if ui
                    .add_sized(
                        [ui.available_width(), 86.0],
                        egui::TextEdit::multiline(&mut expression).font(egui::TextStyle::Monospace),
                    )
                    .changed()
                {
                    r.insert(key.into(), expression);
                }
            } else if let Some((_, function)) = SCOPE_MODES.iter().find(|(m, _)| *m == mode)
                && !function.is_empty()
                && !function.starts_with("Is")
            {
                egui::ComboBox::from_id_salt("scope_value")
                    .width(130.0)
                    .selected_text(value)
                    .show_ui(ui, |ui| {
                        for option in query_ui::values(board, function) {
                            if ui.selectable_label(false, &option).clicked() {
                                r.insert(
                                    key.into(),
                                    format!("{function}('{}')", option.replace('\'', "''")),
                                );
                            }
                        }
                    });
            }
        });
        ui.add_space(2.0);
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

fn compact_dimension(ui: &mut egui::Ui, r: &mut Fields, key: &str, width: f32) -> egui::Response {
    let mut value = rules::dimension(r, key)
        .map(|n| {
            format!(
                "{}mm",
                format!("{:.5}", n as f64 / 1e6)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
            )
        })
        .unwrap_or_else(|_| text(r, key).into());
    let response = widgets::numeric_edit(ui, key, &mut value, width, true);
    if response.changed() {
        let value = value.trim();
        r.insert(
            key.into(),
            if value
                .chars()
                .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
            {
                value.into()
            } else {
                format!("{value}mm")
            },
        );
    }
    response
}
const CLEARANCE_OBJECTS: &[(&str, &str)] = &[
    ("Arc", "Arc"),
    ("Track", "Track"),
    ("SMD Pad", "SMDPad"),
    ("TH Pad", "THPad"),
    ("Via", "Via"),
    ("Fill", "Fill"),
    ("Poly", "Poly"),
    ("Region", "Region"),
    ("Text", "Text"),
    ("Hole", "Hole"),
];
fn set_gap(r: &mut Fields, value: &str) {
    r.insert("GAP".into(), value.into());
    if let Ok(gap) = rules::dimension(r, "GAP") {
        let native = (gap as f64 / 2.54).round().to_string();
        let matrix = text(r, "OBJECTCLEARANCES")
            .split(';')
            .filter_map(|entry| entry.split_once(':').map(|(k, _)| format!("{k}:{native}")))
            .collect::<Vec<_>>()
            .join(";");
        if !matrix.is_empty() {
            r.insert("OBJECTCLEARANCES".into(), matrix);
        }
    }
}
fn mixed_clearances(r: &Fields) -> bool {
    let gap = rules::dimension(r, "GAP").unwrap_or(0) as f64 / 2.54;
    text(r, "OBJECTCLEARANCES")
        .split(';')
        .filter_map(|e| e.split_once(':')?.1.parse::<f64>().ok())
        .any(|v| (v - gap).abs() > 1.0)
}
fn matrix(ui: &mut egui::Ui, r: &mut Fields, advanced: bool) {
    let objects: Vec<(&str, Vec<&str>)> = if advanced {
        CLEARANCE_OBJECTS
            .iter()
            .map(|(label, key)| (*label, vec![*key]))
            .collect()
    } else {
        vec![
            ("Track", vec!["Track", "Arc"]),
            ("SMD Pad", vec!["SMDPad"]),
            ("TH Pad", vec!["THPad"]),
            ("Via", vec!["Via"]),
            ("Copper", vec!["Fill", "Poly", "Region"]),
            ("Text", vec!["Text"]),
            ("Hole", vec!["Hole"]),
        ]
    };
    let mut values: BTreeMap<String, String> = text(r, "OBJECTCLEARANCES")
        .split(';')
        .filter_map(|s| s.split_once(':').map(|(k, v)| (k.into(), v.into())))
        .collect();
    let mut changed = false;
    let width = ui.available_width();
    let label_width = 164.0_f32.min(width * 0.2);
    let column = (width - label_width - 10.0) / objects.len() as f32;
    let mut widths = vec![label_width];
    widths.extend(std::iter::repeat_n(column, objects.len()));
    let mut headers = vec![""];
    headers.extend(objects.iter().map(|(label, _)| *label));
    egui::Frame::new()
        .fill(widgets::BG)
        .stroke(egui::Stroke::new(1.0, widgets::LINE))
        .show(ui, |ui| {
            widgets::table_header(ui, &widths, &headers);
            egui::ScrollArea::vertical()
                .id_salt("clearance_matrix_scroll")
                .max_height(174.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (row, (label, a)) in objects.iter().enumerate() {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(width - 10.0, 19.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(rect.min, egui::vec2(label_width, 19.0)),
                            0.0,
                            widgets::HEAD,
                        );
                        widgets::cell(
                            ui,
                            egui::Rect::from_min_size(rect.min, egui::vec2(label_width, 19.0)),
                            |ui| {
                                ui.label(*label);
                            },
                        );
                        for (col, (_, b)) in objects.iter().enumerate() {
                            let cell = egui::Rect::from_min_size(
                                rect.min + egui::vec2(label_width + col as f32 * column, 0.0),
                                egui::vec2(column, 19.0),
                            );
                            if col > row {
                                ui.painter().rect_filled(cell, 0.0, Color32::from_gray(48));
                                continue;
                            }
                            if (row + col) % 2 == 0 {
                                ui.painter().rect_filled(cell, 0.0, Color32::from_gray(68));
                            }
                            let mut samples = Vec::new();
                            for x in a {
                                for y in b {
                                    let key = format!("ClearanceObj_{x}-ClearanceObj_{y}");
                                    let rev = format!("ClearanceObj_{y}-ClearanceObj_{x}");
                                    samples.push(
                                        values
                                            .get(&key)
                                            .or_else(|| values.get(&rev))
                                            .and_then(|v| v.parse::<f64>().ok())
                                            .map_or_else(
                                                || {
                                                    rules::dimension(r, "GAP").unwrap_or(0) as f64
                                                        / 1e6
                                                },
                                                |v| v * 2.54 / 1e6,
                                            ),
                                    );
                                }
                            }
                            let first = samples[0];
                            let mut value = if samples.iter().any(|v| (v - first).abs() > 0.000001)
                            {
                                "N/A".into()
                            } else {
                                let value = format!("{first:.5}")
                                    .trim_end_matches('0')
                                    .trim_end_matches('.')
                                    .to_owned();
                                if value.is_empty() { "0".into() } else { value }
                            };
                            let key = format!("matrix_{row}_{col}");
                            widgets::cell(ui, cell, |ui| {
                                if widgets::numeric_edit(
                                    ui,
                                    (&key, "matrix"),
                                    &mut value,
                                    column - 6.0,
                                    false,
                                )
                                .changed()
                                    && let Ok(n) = value.parse::<f64>()
                                    && n.is_finite()
                                    && n >= 0.0
                                {
                                    changed = true;
                                    for x in a {
                                        for y in b {
                                            let key = format!("ClearanceObj_{x}-ClearanceObj_{y}");
                                            let rev = format!("ClearanceObj_{y}-ClearanceObj_{x}");
                                            values.remove(&rev);
                                            values
                                                .insert(key, (n * 1e6 / 2.54).round().to_string());
                                        }
                                    }
                                }
                            });
                        }
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
fn clearance_constraints(ui: &mut egui::Ui, r: &mut Fields) {
    ui.horizontal(|ui| {
        ui.add_space(29.0);
        egui::ComboBox::from_id_salt("net_scope")
            .width(125.0)
            .selected_text(match text(r, "NETSCOPE") {
                "SameNet" => "Same Net",
                "AnyNet" => "Any Net",
                _ => "Different Nets",
            })
            .show_ui(ui, |ui| {
                for (value, label) in [
                    ("DifferentNets", "Different Nets"),
                    ("SameNet", "Same Net"),
                    ("AnyNet", "Any Net"),
                ] {
                    if ui
                        .selectable_label(text(r, "NETSCOPE") == value, label)
                        .clicked()
                    {
                        r.insert("NETSCOPE".into(), value.into());
                    }
                }
            });
    });
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(215.0);
            ui.horizontal(|ui| {
                ui.add_space(35.0);
                ui.label("Minimum Clearance");
                if mixed_clearances(r) {
                    let mut value = "N/A".to_owned();
                    if widgets::numeric_edit(ui, "GAP", &mut value, 70.0, false).changed() {
                        let value = value.trim();
                        let value = if value.chars().any(|c| c.is_ascii_alphabetic()) {
                            value.to_owned()
                        } else {
                            format!("{value}mm")
                        };
                        set_gap(r, &value);
                    }
                } else if compact_dimension(ui, r, "GAP", 70.0).changed() {
                    let value = text(r, "GAP").to_owned();
                    set_gap(r, &value);
                }
            });
            widgets::illustration(ui, "Clearance");
        });
        ui.add_space(5.0);
        ui.vertical(|ui| {
            ui.add_space(85.0);
            flag(
                ui,
                r,
                "IGNOREPADTOPADCLEARANCEINFOOTPRINT",
                "Ignore Pad to Pad clearances within a footprint",
                false,
            );
        });
    });
    ui.add_space(12.0);
    let id = ui.id().with((text(r, "UNIQUEID"), "clearance_advanced"));
    let mut advanced = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(id))
        .unwrap_or(!text(r, "OBJECTCLEARANCES").is_empty());
    ui.horizontal(|ui| {
        ui.add_space(35.0);
        ui.radio_value(&mut advanced, false, "Simple");
        ui.add_space(65.0);
        ui.radio_value(&mut advanced, true, "Advanced");
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, advanced));
    ui.add_space(8.0);
    matrix(ui, r, advanced);
    ui.add_space(8.0);
    ui.label("Required clearances between electrical objects and Board Cutouts / Board Cavities are determined using the largest of Electrical Clearance rule’s Region-to-object settings and Board Outline Clearance rule’s settings.");
}

fn link_dimension(ui: &mut egui::Ui, r: &mut Fields, key: &str, fallback: &str, width: f32) {
    let source = if r.contains_key(key) { key } else { fallback };
    let mut value = rules::dimension(r, source)
        .map(|n| {
            format!(
                "{}mm",
                format!("{:.5}", n as f64 / 1e6)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
            )
        })
        .unwrap_or_else(|_| text(r, source).into());
    if widgets::numeric_edit(ui, key, &mut value, width, false).changed() {
        let value = value.trim();
        r.insert(
            key.into(),
            if value
                .chars()
                .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
            {
                value.into()
            } else {
                format!("{value}mm")
            },
        );
    }
}

fn hole_size_constraints(ui: &mut egui::Ui, r: &mut Fields) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(380.0, 180.0), egui::Sense::hover());
    let at = |x, y| rect.min + egui::vec2(x, y);
    let p = ui.painter_at(rect);
    let c = at(242.0, 94.0);
    p.circle_filled(c, 35.0, widgets::GOLD);
    p.circle_filled(c, 17.0, widgets::BG);
    let diagonal = egui::vec2(-1.0, -1.0).normalized();
    for radius in [17.0, 35.0] {
        let from = c + diagonal * radius;
        p.line_segment(
            [from, from + egui::vec2(-32.0, -32.0)],
            egui::Stroke::new(1.0, Color32::GRAY),
        );
    }
    widgets::dimension_arrow(
        &p,
        c + diagonal * 17.0 + egui::vec2(-26.0, -26.0),
        c + diagonal * 35.0 + egui::vec2(-26.0, -26.0),
    );
    widgets::child(
        ui,
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            at(32.0, 3.0),
            egui::vec2(280.0, 24.0),
        )),
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Measurement Method");
                ui.add_space(15.0);
                let absolute = text(r, "ABSOLUTEVALUES") != "FALSE";
                egui::ComboBox::from_id_salt("hole_measurement")
                    .width(95.0)
                    .selected_text(if absolute { "Absolute" } else { "Percent" })
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(absolute, "Absolute").clicked() {
                            r.insert("ABSOLUTEVALUES".into(), "TRUE".into());
                            r.entry("MINLIMIT".into()).or_insert_with(|| "1mil".into());
                            r.entry("MAXLIMIT".into())
                                .or_insert_with(|| "100mil".into());
                        }
                        if ui.selectable_label(!absolute, "Percent").clicked() {
                            r.insert("ABSOLUTEVALUES".into(), "FALSE".into());
                            r.entry("MINPERCENT".into()).or_insert_with(|| "20".into());
                            r.entry("MAXPERCENT".into()).or_insert_with(|| "80".into());
                        }
                    });
            });
        },
    );
    let percent = text(r, "ABSOLUTEVALUES") == "FALSE";
    let keys = if percent {
        ["MINPERCENT", "MAXPERCENT"]
    } else {
        ["MINLIMIT", "MAXLIMIT"]
    };
    for (y, label, key) in [(34.0, "Minimum", keys[0]), (59.0, "Maximum", keys[1])] {
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                at(32.0, y),
                egui::vec2(140.0, 24.0),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([65.0, 20.0], egui::Label::new(label));
                    if percent {
                        let mut value = format!("{}%", text(r, key).trim_end_matches('%'));
                        if widgets::numeric_edit(ui, key, &mut value, 75.0, false).changed() {
                            r.insert(key.into(), value.trim().trim_end_matches('%').trim().into());
                        }
                    } else {
                        link_dimension(ui, r, key, key, 75.0);
                    }
                });
            },
        );
    }
}
fn annular_ring_constraints(ui: &mut egui::Ui, r: &mut Fields) {
    let height = (ui.clip_rect().bottom() - ui.cursor().top() - 12.0).max(180.0);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let at = |x, y| rect.min + egui::vec2(x, y);
    widgets::child(
        ui,
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            at(32.0, 3.0),
            egui::vec2(310.0, 24.0),
        )),
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Minimum Annular Ring (x - y)");
                ui.add_space(12.0);
                link_dimension(ui, r, "MINIMUMRING", "MINIMUMRING", 90.0);
            });
        },
    );
    let p = ui.painter_at(rect);
    let c = at(133.0, 56.0);
    p.circle_filled(c, 30.0, widgets::GOLD);
    p.circle_filled(c, 11.0, widgets::BG);
    for y in [67.0, 86.0] {
        p.line_segment(
            [at(128.0, y), at(194.0, y)],
            egui::Stroke::new(1.0, Color32::GRAY),
        );
    }
    widgets::dimension_arrow(&p, at(188.0, 67.0), at(188.0, 86.0));
}
fn hole_clearance_constraints(ui: &mut egui::Ui, r: &mut Fields) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(430.0, 230.0), egui::Sense::hover());
    let at = |x, y| rect.min + egui::vec2(x, y);
    widgets::child(
        ui,
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            at(32.0, 3.0),
            egui::vec2(430.0, 24.0),
        )),
        |ui| {
            ui.horizontal(|ui| {
                disabled_check(ui, "Allow Stacked Micro Vias", false);
                ui.add_space(16.0);
                ui.label("Hole to Hole Clearance");
                ui.add_space(16.0);
                link_dimension(ui, r, "GAP", "GAP", 80.0);
            });
        },
    );
    let p = ui.painter_at(rect);
    let green = Color32::from_rgb(52, 126, 8);
    p.rect_filled(
        egui::Rect::from_min_size(at(36.0, 32.0), egui::vec2(220.0, 182.0)),
        0.0,
        green,
    );
    for y in [85.0, 179.0] {
        for (n, x) in [72.0, 121.0, 170.0, 220.0].into_iter().enumerate() {
            let c = at(x, y);
            if y > 100.0 && n == 0 {
                p.rect_filled(
                    egui::Rect::from_center_size(c, egui::vec2(34.0, 34.0)),
                    0.0,
                    widgets::GOLD,
                );
            } else {
                p.circle_filled(c, 17.0, widgets::GOLD);
            }
            p.circle_filled(c, 7.0, green);
        }
    }
    p.circle_filled(at(170.0, 144.0), 9.0, widgets::GOLD);
    p.circle_filled(at(170.0, 144.0), 5.0, green);
    for y in [51.0, 145.0] {
        for x in [79.0, 114.0] {
            p.line_segment(
                [at(x, y - 8.0), at(x, y + 38.0)],
                egui::Stroke::new(1.0, Color32::GRAY),
            );
        }
        widgets::dimension_arrow(&p, at(79.0, y), at(114.0, y));
    }
    widgets::dimension_arrow(&p, at(196.0, 151.0), at(196.0, 162.0));
}
fn mask_expansion_constraints(ui: &mut egui::Ui, r: &mut Fields, paste: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(650.0, 245.0), egui::Sense::hover());
    let at = |x, y| rect.min + egui::vec2(x, y);
    if paste {
        for (x, label, checks) in [
            (40.0, "SMD Pads:", vec![("Use Paste", true)]),
            (
                140.0,
                "TH Pads:",
                vec![("Use Top Paste", false), ("Use Bottom Paste", false)],
            ),
        ] {
            widgets::child(
                ui,
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    at(x, 8.0),
                    egui::vec2(150.0, 85.0),
                )),
                |ui| {
                    ui.label(label);
                    ui.add_space(5.0);
                    for (label, checked) in checks {
                        disabled_check(ui, label, checked);
                    }
                },
            );
        }
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                at(40.0, 90.0),
                egui::vec2(220.0, 45.0),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label("Measurement Method");
                    ui.add_space(5.0);
                    ui.add_enabled(false, egui::Link::new("Absolute"));
                });
                ui.horizontal(|ui| {
                    ui.label("Expansion");
                    ui.label("⊕")
                        .on_hover_text("Expansion on each edge of the paste aperture");
                    link_dimension(ui, r, "EXPANSION", "EXPANSION", 90.0);
                });
            },
        );
        let p = ui.painter_at(rect);
        p.rect_filled(
            egui::Rect::from_min_size(at(36.0, 170.0), egui::vec2(96.0, 43.0)),
            10.0,
            widgets::GOLD,
        );
        p.rect_filled(
            egui::Rect::from_min_size(at(42.0, 176.0), egui::vec2(84.0, 31.0)),
            6.0,
            Color32::from_rgb(190, 145, 42),
        );
        p.line_segment(
            [at(128.0, 191.0), at(178.0, 191.0)],
            egui::Stroke::new(15.0, widgets::GOLD),
        );
        p.line_segment(
            [at(178.0, 191.0), at(191.0, 204.0)],
            egui::Stroke::new(15.0, widgets::GOLD),
        );
        widgets::dimension_arrow(&p, at(126.0, 147.0), at(132.0, 147.0));
    } else {
        let p = ui.painter_at(rect);
        let green = Color32::from_rgb(52, 126, 8);
        p.rect_filled(
            egui::Rect::from_min_size(at(32.0, 3.0), egui::vec2(160.0, 122.0)),
            0.0,
            green,
        );
        p.circle_filled(at(120.0, 61.0), 36.0, Color32::from_rgb(103, 130, 13));
        p.line_segment(
            [at(32.0, 61.0), at(120.0, 61.0)],
            egui::Stroke::new(16.0, widgets::GOLD),
        );
        p.circle_filled(at(120.0, 61.0), 26.0, widgets::GOLD);
        p.circle_filled(at(120.0, 61.0), 10.0, widgets::BG);
        for y in [25.0, 37.0] {
            p.line_segment(
                [at(105.0, y), at(241.0, y)],
                egui::Stroke::new(1.0, Color32::GRAY),
            );
        }
        widgets::dimension_arrow(&p, at(231.0, 25.0), at(231.0, 37.0));
        for (y, label, bottom) in [
            (11.0, "Expansion top:", false),
            (37.0, "Expansion bottom:", true),
        ] {
            widgets::child(
                ui,
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    at(273.0, y),
                    egui::vec2(183.0, 23.0),
                )),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized([102.0, 20.0], egui::Label::new(label));
                        ui.add_enabled_ui(!bottom, |ui| {
                            link_dimension(ui, r, "EXPANSION", "EXPANSION", 75.0)
                        });
                    });
                },
            );
            widgets::child(
                ui,
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    at(506.0, y + 4.0),
                    egui::vec2(120.0, 24.0),
                )),
                |ui| {
                    flag(
                        ui,
                        r,
                        if bottom {
                            "ISTENTINGBOTTOM"
                        } else {
                            "ISTENTINGTOP"
                        },
                        "Tented",
                        false,
                    );
                },
            );
        }
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                at(460.0, 9.0),
                egui::vec2(24.0, 52.0),
            )),
            |ui| {
                ui.add_enabled(
                    false,
                    egui::Button::new("⇔").min_size(egui::vec2(20.0, 50.0)),
                )
                .on_hover_text("Independent top and bottom expansion is not implemented");
            },
        );
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                at(273.0, 77.0),
                egui::vec2(330.0, 24.0),
            )),
            |ui| disabled_check(ui, "Solder Mask From The Hole Edge", false),
        );
    }
}
fn via_constraints(ui: &mut egui::Ui, r: &mut Fields) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(460.0, 230.0), egui::Sense::hover());
    let at = |x, y| rect.min + egui::vec2(x, y);
    let p = ui.painter_at(rect);
    let c = at(210.0, 78.0);
    p.circle_filled(c, 30.0, widgets::GOLD);
    p.circle_filled(c, 11.0, widgets::BG);
    for x in [179.0, 241.0] {
        p.line_segment(
            [at(x, 32.0), at(x, 112.0)],
            egui::Stroke::new(1.0, Color32::GRAY),
        );
    }
    for x in [199.0, 221.0] {
        p.line_segment(
            [at(x, 80.0), at(x, 133.0)],
            egui::Stroke::new(1.0, Color32::GRAY),
        );
    }
    widgets::dimension_arrow(&p, at(153.0, 48.0), at(178.0, 48.0));
    widgets::dimension_arrow(&p, at(267.0, 48.0), at(242.0, 48.0));
    widgets::dimension_arrow(&p, at(163.0, 122.0), at(198.0, 122.0));
    widgets::dimension_arrow(&p, at(280.0, 122.0), at(222.0, 122.0));
    widgets::child(
        ui,
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
            at(33.0, 3.0),
            egui::vec2(170.0, 22.0),
        )),
        |ui| {
            ui.colored_label(Color32::from_rgb(156, 194, 222), "Min/Max preferred");
        },
    );
    for (x, y, title, fields) in [
        (
            33.0,
            40.0,
            "Via Diameter",
            [
                ("MINWIDTH", "Minimum"),
                ("MAXWIDTH", "Maximum"),
                ("WIDTH", "Preferred"),
            ],
        ),
        (
            293.0,
            115.0,
            "Via Hole Size",
            [
                ("MINHOLEWIDTH", "Minimum"),
                ("MAXHOLEWIDTH", "Maximum"),
                ("HOLEWIDTH", "Preferred"),
            ],
        ),
    ] {
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                at(x, y),
                egui::vec2(150.0, 100.0),
            )),
            |ui| {
                ui.strong(title);
                for (key, label) in fields {
                    ui.horizontal(|ui| {
                        ui.add_sized([53.0, 20.0], egui::Label::new(label));
                        link_dimension(ui, r, key, key, 90.0);
                    });
                }
            },
        );
    }
}
fn width_constraints(ui: &mut egui::Ui, r: &mut Fields, board: &Board) {
    let (upper, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 146.0),
        egui::Sense::hover(),
    );
    for (x, y, label, key) in [
        (81.0, 3.0, "Preferred Width", "PREFEREDWIDTH"),
        (32.0, 33.0, "Min Width", "MINLIMIT"),
        (190.0, 33.0, "Max Width", "MAXLIMIT"),
    ] {
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                upper.min + egui::vec2(x, y),
                egui::vec2(170.0, 23.0),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    ui.label(label);
                    link_dimension(ui, r, key, key, 75.0);
                });
            },
        );
    }
    let p = ui.painter_at(upper);
    let c = upper.min + egui::vec2(95.0, 99.0);
    for (a, b, w) in [
        (c, c + egui::vec2(53.0, 0.0), 10.0),
        (c + egui::vec2(53.0, 0.0), c + egui::vec2(118.0, 0.0), 20.0),
        (c + egui::vec2(118.0, 0.0), c + egui::vec2(170.0, 0.0), 38.0),
    ] {
        p.line_segment([a, b], egui::Stroke::new(w, widgets::GOLD));
    }
    for (x, half) in [(78.0, 5.0), (175.0, 10.0), (269.0, 19.0)] {
        widgets::dimension_arrow(
            &p,
            upper.min + egui::vec2(x, 99.0 - half),
            upper.min + egui::vec2(x, 99.0 + half),
        );
    }
    widgets::child(
        ui,
        egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
            upper.min + egui::vec2(335.0, 0.0),
            upper.max,
        )),
        |ui| {
            let individual = true;
            ui.add_enabled(
                false,
                egui::RadioButton::new(individual, "Check Tracks/Arcs Min/Max Width Individually"),
            );
            ui.add_enabled(
                false,
                egui::RadioButton::new(!individual, "Check Min/Max Width for Physically Connected"),
            );
            let mut impedance = false;
            ui.add_enabled(
                false,
                egui::Checkbox::new(&mut impedance, "Use Impedance Profile"),
            );
            ui.add_enabled(false, egui::Button::new("50Ω                         ▾"));
        },
    );
    ui.add_space(7.0);
    let width = ui.available_width();
    let widths = [width * 0.2, width * 0.2, width * 0.2, width * 0.4];
    let height = (ui.clip_rect().bottom() - ui.cursor().top() - 15.0).clamp(80.0, 425.0);
    egui::Frame::new()
        .fill(Color32::from_gray(58))
        .stroke(egui::Stroke::new(1.0, widgets::LINE))
        .show(ui, |ui| {
            ui.set_min_height(height);
            ui.spacing_mut().item_spacing.y = 0.0;
            widgets::table_header(
                ui,
                &widths,
                &["Min Width", "Preferred Width", "Max Width", "Layer Name"],
            );
            for (ordinal, layer) in board.layers.iter().filter(|l| l.copper).enumerate() {
                let prefix = match layer.id {
                    pcb::TOP => "TOPLAYER".into(),
                    pcb::BOTTOM => "BOTTOMLAYER".into(),
                    n => format!("MIDLAYER{}", n - 1),
                };
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, 19.0), egui::Sense::hover());
                let mut x = rect.left();
                for (col, w) in widths.iter().enumerate() {
                    let cell =
                        egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(*w, 19.0));
                    widgets::cell(ui, cell, |ui| {
                        if col < 3 {
                            let (suffix, fallback) = match col {
                                0 => ("MINWIDTH", "MINLIMIT"),
                                1 => ("PREFWIDTH", "PREFEREDWIDTH"),
                                _ => ("MAXWIDTH", "MAXLIMIT"),
                            };
                            link_dimension(
                                ui,
                                r,
                                &format!("{prefix}_{suffix}"),
                                fallback,
                                *w - 7.0,
                            );
                        } else {
                            let (swatch, _) = ui
                                .allocate_exact_size(egui::vec2(25.0, 17.0), egui::Sense::hover());
                            ui.painter().rect_filled(
                                swatch,
                                0.0,
                                eda_render::pcb::layer_color(layer.id),
                            );
                            ui.label(format!("{} - {}", ordinal + 1, layer.name));
                        }
                    });
                    x += w;
                }
            }
        });
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

fn connection_preview(ui: &mut egui::Ui, rect: egui::Rect, r: &Fields, prefix: &str) {
    let p = ui.painter_at(rect);
    let at = |x, y| rect.min + egui::vec2(x, y);
    let c = at(72.0, 64.0);
    let style = text(r, &format!("{prefix}CONNECTSTYLE"));
    let smd = prefix == "SMDPAD.";
    if style == "Direct" {
        // Native illustration uses a cropped copper island rather than a full square.
        let pts = [
            (0.0, 38.0),
            (121.0, 38.0),
            (121.0, 0.0),
            (180.0, 0.0),
            (180.0, 103.0),
            (36.0, 103.0),
            (36.0, 78.0),
            (0.0, 78.0),
        ]
        .map(|(x, y)| at(x, y));
        // Concave L island drawn as three rectangles.
        for (x, y, w, h) in [
            (0.0, 38.0, 180.0, 40.0),
            (121.0, 0.0, 59.0, 38.0),
            (36.0, 78.0, 144.0, 25.0),
        ] {
            p.rect_filled(
                egui::Rect::from_min_size(at(x, y), egui::vec2(w, h)),
                0.0,
                widgets::GOLD,
            );
        }
        if !smd {
            p.circle_filled(pts[0] + egui::vec2(114.0, 26.0), 8.0, widgets::BG);
        }
        return;
    }
    p.rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(145.0, 130.0)),
        0.0,
        widgets::GOLD,
    );
    if smd {
        p.rect_filled(
            egui::Rect::from_center_size(c, egui::vec2(78.0, 81.0)),
            2.0,
            widgets::BG,
        );
    } else {
        p.circle_filled(c, 40.0, widgets::BG);
    }
    if style == "Relief" {
        let angle = if text(r, &format!("{prefix}POLYGONRELIEFANGLE")) == "45 Angle" {
            45.0_f32
        } else {
            0.0
        };
        let count = if text(r, &format!("{prefix}RELIEFENTRIES")) == "2" {
            1
        } else {
            2
        };
        for n in 0..count {
            let a = (angle + n as f32 * 90.0).to_radians();
            let v = egui::vec2(a.cos(), a.sin()) * 45.0;
            p.line_segment([c - v, c + v], egui::Stroke::new(14.0, widgets::GOLD));
        }
    }
    if smd {
        p.rect_filled(
            egui::Rect::from_center_size(c, egui::vec2(49.0, 53.0)),
            0.0,
            widgets::GOLD,
        );
    } else {
        p.circle_filled(c, 25.0, widgets::GOLD);
        p.circle_filled(c, 8.0, widgets::BG);
    }
    // Truncated adjacent copper makes the reference boundary recognizable.
    for (x, y) in [
        (0.0, 4.0),
        (0.0, 20.0),
        (145.0, 75.0),
        (145.0, 91.0),
        (98.0, 0.0),
        (114.0, 0.0),
        (97.0, 130.0),
        (114.0, 130.0),
    ] {
        p.circle_filled(at(x, y), 3.0, widgets::BG);
    }
}
fn connection_column(ui: &mut egui::Ui, r: &mut Fields, prefix: &str, title: Option<&str>) {
    ui.push_id(prefix, |ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(310.0, 366.0), egui::Sense::hover());
        let at = |x, y| rect.min + egui::vec2(x, y);
        let cell = |ui: &mut egui::Ui, x, y, w, h, f: &mut dyn FnMut(&mut egui::Ui)| {
            widgets::child(
                ui,
                egui::UiBuilder::new()
                    .max_rect(egui::Rect::from_min_size(at(x, y), egui::vec2(w, h))),
                |ui| f(ui),
            );
        };
        if let Some(title) = title {
            cell(ui, 8.0, 7.0, 294.0, 22.0, &mut |ui| {
                ui.strong(title);
            });
        }
        let key = format!("{prefix}CONNECTSTYLE");
        cell(ui, 20.0, 35.0, 92.0, 21.0, &mut |ui| {
            ui.label("Connect Style");
        });
        cell(ui, 110.0, 32.0, 105.0, 23.0, &mut |ui| {
            egui::ComboBox::from_id_salt("style")
                .width(105.0)
                .selected_text(match text(r, &key) {
                    "Direct" => "Direct Connect",
                    "NoConnect" => "No Connect",
                    _ => "Relief Connect",
                })
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
        connection_preview(
            ui,
            egui::Rect::from_min_size(at(16.0, 70.0), egui::vec2(180.0, 140.0)),
            r,
            prefix,
        );
        if text(r, &key) != "Relief" {
            return;
        }
        let p = ui.painter();
        for (a, b) in [
            (at(178.0, 70.0), at(178.0, 91.0)),
            (at(178.0, 127.0), at(178.0, 109.0)),
        ] {
            widgets::dimension_arrow(p, a, b);
        }
        for x in [81.0, 97.0] {
            p.line_segment(
                [at(x, 164.0), at(x, 229.0)],
                egui::Stroke::new(1.0, Color32::GRAY),
            );
        }
        widgets::dimension_arrow(p, at(81.0, 216.0), at(97.0, 216.0));
        for (x, y, key, label) in [
            (212.0, 82.0, "AIRGAPWIDTH", "Air Gap Width"),
            (88.0, 238.0, "RELIEFCONDUCTORWIDTH", "Conductor Width"),
        ] {
            cell(ui, x, y, 100.0, 21.0, &mut |ui| {
                ui.label(label);
            });
            cell(ui, x, y + 21.0, 94.0, 21.0, &mut |ui| {
                let key = format!("{prefix}{key}");
                let mut value = rules::dimension(r, &key)
                    .map(|n| {
                        format!(
                            "{}mm",
                            format!("{:.5}", n as f64 / 1e6)
                                .trim_end_matches('0')
                                .trim_end_matches('.')
                        )
                    })
                    .unwrap_or_else(|_| text(r, &key).into());
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut value)
                            .id_salt(&key)
                            .desired_width(90.0)
                            .frame(false)
                            .text_color(Color32::from_rgb(156, 194, 222)),
                    )
                    .changed()
                {
                    let value = value.trim();
                    r.insert(
                        key,
                        if value.chars().any(|c| c.is_ascii_alphabetic()) {
                            value.into()
                        } else {
                            format!("{value}mm")
                        },
                    );
                }
            });
        }
        cell(ui, 16.0, 282.0, 290.0, 23.0, &mut |ui| {
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
                .on_hover_text("Automatic conductor placement is not implemented");
            });
        });
        cell(ui, 16.0, 309.0, 290.0, 23.0, &mut |ui| {
            ui.horizontal(|ui| {
                ui.label("Rotation");
                ui.add_space(19.0);
                let key = format!("{prefix}POLYGONRELIEFANGLE");
                egui::ComboBox::from_id_salt("rotation")
                    .width(105.0)
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
    let previous = advanced;
    ui.horizontal(|ui| {
        ui.add_space(30.0);
        ui.radio_value(&mut advanced, false, "Simple");
        ui.add_space(60.0);
        ui.radio_value(&mut advanced, true, "Advanced");
    });
    if previous != advanced {
        rules::set_advanced_connection(r, advanced);
    }
    ui.add_space(11.0);
    if advanced {
        egui::ScrollArea::horizontal()
            .id_salt("connection_columns")
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    for (prefix, title) in [
                        ("THPAD.", "Through Hole Pad Connection"),
                        ("SMDPAD.", "SMD Pad Connection"),
                        ("VIA.", "Via Connection"),
                    ] {
                        egui::Frame::new()
                            .stroke(egui::Stroke::new(1.0, widgets::LINE))
                            .show(ui, |ui| connection_column(ui, r, prefix, Some(title)));
                    }
                });
            });
    } else {
        ui.horizontal(|ui| {
            ui.add_space(30.0);
            connection_column(ui, r, "", None);
        });
    }
}
fn active_kind(d: &Draft) -> String {
    if let RulePage::Kind(kind) = &d.page {
        return kind.clone();
    }
    d.rules
        .get(d.selected)
        .map(|r| text(r, "RULEKIND").to_owned())
        .unwrap_or_else(|| "Clearance".into())
}
fn rule_menu(ui: &mut egui::Ui, d: &mut Draft, kind: &str, index: Option<usize>) {
    let available = rules::supported(kind);
    if ui
        .add_enabled(available, egui::Button::new("New Rule"))
        .clicked()
    {
        insert_rule(d, kind, None);
        d.page = RulePage::Rule;
        ui.close();
    }
    if ui
        .add_enabled(
            available && index.is_some(),
            egui::Button::new("Duplicate Rule"),
        )
        .clicked()
    {
        insert_rule(d, kind, index);
        d.page = RulePage::Rule;
        ui.close();
    }
    if ui
        .add_enabled(
            available && index.is_some(),
            egui::Button::new("Delete Rule"),
        )
        .clicked()
    {
        if let Some(i) = index {
            d.rules.remove(i);
            d.selected = d.selected.saturating_sub(1);
            d.page = RulePage::Kind(kind.into());
        }
        ui.close();
    }
    ui.separator();
    for label in ["Export Rules…", "Import Rules…"] {
        ui.add_enabled(false, egui::Button::new(label));
    }
}
fn tree(ui: &mut egui::Ui, d: &mut Draft) {
    let filter = d.search.to_lowercase();
    let (click, toggle, _) = widgets::tree_row(
        ui,
        "root",
        "Design Rules",
        0,
        "Design Rules",
        d.page == RulePage::Root,
        Some(d.expanded.contains("Design Rules")),
    );
    if click {
        d.page = RulePage::Root;
    }
    if toggle && !d.expanded.remove("Design Rules") {
        d.expanded.insert("Design Rules".into());
    }
    if !d.expanded.contains("Design Rules") {
        return;
    }
    for cat in CATEGORIES {
        if !filter.is_empty()
            && !cat.to_lowercase().contains(&filter)
            && !CATALOG.iter().any(|(c, label, kind)| {
                c == cat
                    && (label.to_lowercase().contains(&filter)
                        || kind.to_lowercase().contains(&filter))
            })
            && !d.rules.iter().any(|r| {
                category(text(r, "RULEKIND")) == *cat
                    && text(r, "NAME").to_lowercase().contains(&filter)
            })
        {
            continue;
        }
        let cat_selected = d.page == RulePage::Category((*cat).into());
        let (click, toggle, _) = widgets::tree_row(
            ui,
            cat,
            cat,
            1,
            cat,
            cat_selected,
            Some(d.expanded.contains(*cat)),
        );
        if click {
            d.page = RulePage::Category((*cat).into());
        }
        if toggle && !d.expanded.remove(*cat) {
            d.expanded.insert((*cat).into());
        }
        if !d.expanded.contains(*cat) {
            continue;
        }
        let mut kinds: Vec<String> = CATALOG
            .iter()
            .filter(|(c, _, _)| c == cat)
            .map(|(_, _, k)| (*k).into())
            .collect();
        for r in &d.rules {
            let k = text(r, "RULEKIND");
            if category(k) == *cat && !kinds.iter().any(|s| s == k) {
                kinds.push(k.into());
            }
        }
        for kind in kinds {
            // The native tree retains file order; the separate priority dialog
            // sorts by PRIORITY. These orders differ on the reference board.
            let rows: Vec<_> = d
                .rules
                .iter()
                .enumerate()
                .filter(|(_, r)| text(r, "RULEKIND") == kind)
                .map(|(i, _)| i)
                .collect();
            if !filter.is_empty()
                && !name(&kind).to_lowercase().contains(&filter)
                && !rows
                    .iter()
                    .any(|i| text(&d.rules[*i], "NAME").to_lowercase().contains(&filter))
            {
                continue;
            }
            let (click, toggle, response) = widgets::tree_row(
                ui,
                &kind,
                name(&kind),
                2,
                cat,
                d.page == RulePage::Kind(kind.clone()),
                Some(d.expanded.contains(&kind)),
            );
            if click {
                d.page = RulePage::Kind(kind.clone());
            }
            if toggle && !d.expanded.remove(&kind) {
                d.expanded.insert(kind.clone());
            }
            response.context_menu(|ui| rule_menu(ui, d, &kind, None));
            if !d.expanded.contains(&kind) {
                continue;
            }
            for i in rows {
                if i >= d.rules.len() {
                    continue;
                }
                let label = text(&d.rules[i], "NAME").to_owned();
                if !filter.is_empty()
                    && !label.to_lowercase().contains(&filter)
                    && !name(&kind).to_lowercase().contains(&filter)
                {
                    continue;
                }
                let (click, _, response) = widgets::tree_row(
                    ui,
                    &format!("instance_{i}"),
                    &label,
                    3,
                    cat,
                    d.page == RulePage::Rule && d.selected == i,
                    None,
                );
                if click {
                    d.selected = i;
                    d.page = RulePage::Rule;
                }
                response.context_menu(|ui| rule_menu(ui, d, &kind, Some(i)));
            }
        }
    }
}
fn sorted_rules(d: &Draft, kind: &str) -> Vec<usize> {
    let mut rows: Vec<_> = d
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| text(r, "RULEKIND") == kind)
        .map(|(i, _)| i)
        .collect();
    rows.sort_by_key(|i| {
        (
            text(&d.rules[*i], "PRIORITY")
                .parse::<u32>()
                .unwrap_or(u32::MAX),
            *i,
        )
    });
    rows
}
fn summary(ui: &mut egui::Ui, d: &mut Draft, height: f32) {
    let mut rows: Vec<_> = d
        .rules
        .iter()
        .enumerate()
        .filter(|(_, r)| match &d.page {
            RulePage::Root => true,
            RulePage::Category(c) => category(text(r, "RULEKIND")) == c,
            RulePage::Kind(k) => text(r, "RULEKIND") == k,
            _ => false,
        })
        .map(|(i, _)| i)
        .collect();
    rows.sort_by_key(|i| text(&d.rules[*i], "NAME").to_ascii_lowercase());
    let width = ui.available_width();
    let widths = [
        width * 0.19,
        65.0,
        49.0,
        width * 0.19,
        width * 0.115,
        width * 0.19,
        (width * 0.315 - 114.0).max(90.0),
    ];
    egui::Frame::new()
        .fill(widgets::BG)
        .stroke(egui::Stroke::new(1.0, widgets::LINE))
        .show(ui, |ui| {
            ui.set_min_height(height - 35.0);
            ui.set_max_width(width);
            widgets::table_header(
                ui,
                &widths,
                &[
                    "Name",
                    "Priority",
                    "Enabled",
                    "Type",
                    "Category",
                    "Scope",
                    "Attributes",
                ],
            );
            egui::ScrollArea::both()
                .id_salt("rules_summary")
                .max_height(height - 59.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for i in rows {
                        let kind = text(&d.rules[i], "RULEKIND").to_owned();
                        let (r, response) = ui.allocate_exact_size(
                            egui::vec2(widths.iter().sum(), 21.3),
                            egui::Sense::click(),
                        );
                        if d.selected == i {
                            ui.painter().rect_filled(r, 0.0, Color32::from_gray(78));
                        }
                        let mut x = r.left();
                        for (col, w) in widths.iter().enumerate() {
                            let c = egui::Rect::from_min_size(
                                egui::pos2(x, r.top()),
                                egui::vec2(*w, r.height()),
                            );
                            widgets::cell(ui, c, |ui| match col {
                                0 => {
                                    let (icon, _) = ui.allocate_exact_size(
                                        egui::vec2(18.0, 18.0),
                                        egui::Sense::hover(),
                                    );
                                    widgets::icon(ui.painter(), icon, category(&kind));
                                    ui.add(egui::Label::new(text(&d.rules[i], "NAME")).truncate());
                                }
                                2 => {
                                    let mut enabled = rules::enabled(&d.rules[i]);
                                    if ui
                                        .add_enabled(
                                            rules::supported(&kind),
                                            egui::Checkbox::without_text(&mut enabled),
                                        )
                                        .changed()
                                    {
                                        d.rules[i].insert(
                                            "ENABLED".into(),
                                            if enabled { "TRUE" } else { "FALSE" }.into(),
                                        );
                                    }
                                }
                                _ => {
                                    let value = match col {
                                        1 => text(&d.rules[i], "PRIORITY").to_owned(),
                                        3 => name(&kind).into(),
                                        4 => category(&kind).into(),
                                        5 => format!(
                                            "{}   {}",
                                            text(&d.rules[i], "SCOPE1EXPRESSION"),
                                            text(&d.rules[i], "SCOPE2EXPRESSION")
                                        ),
                                        _ => widgets::attributes(&d.rules[i]),
                                    };
                                    ui.add(egui::Label::new(&value).truncate())
                                        .on_hover_text(value);
                                }
                            });
                            x += w;
                        }
                        if response.clicked() {
                            d.selected = i;
                        }
                        if response.double_clicked() {
                            d.selected = i;
                            d.page = RulePage::Rule;
                        }
                        response.context_menu(|ui| rule_menu(ui, d, &kind, Some(i)));
                    }
                });
        });
    ui.add_space(5.0);
    ui.horizontal(|ui| {
        let kind = active_kind(d);
        let available = rules::supported(&kind);
        let selected_visible = d.rules.get(d.selected).is_some_and(|r| match &d.page {
            RulePage::Root => true,
            RulePage::Category(c) => category(text(r, "RULEKIND")) == c,
            RulePage::Kind(k) => text(r, "RULEKIND") == k,
            RulePage::Rule => true,
        });
        let editable_selection = selected_visible
            && d.rules
                .get(d.selected)
                .is_some_and(|r| rules::supported(text(r, "RULEKIND")));
        if ui
            .add_enabled(
                available,
                egui::Button::new("New Rule").min_size(egui::vec2(90.0, 25.0)),
            )
            .clicked()
        {
            insert_rule(d, &kind, None);
            d.page = RulePage::Rule;
        }
        if ui
            .add_enabled(
                editable_selection,
                egui::Button::new("Delete Rule(s)…").min_size(egui::vec2(90.0, 25.0)),
            )
            .clicked()
        {
            d.rules.remove(d.selected);
            d.selected = d.selected.saturating_sub(1);
        }
        if ui
            .add_enabled(
                editable_selection,
                egui::Button::new("Duplicate Rule").min_size(egui::vec2(90.0, 25.0)),
            )
            .clicked()
        {
            insert_rule(d, &kind, Some(d.selected));
            d.page = RulePage::Rule;
        }
        ui.add_enabled(
            false,
            egui::Button::new("Report…").min_size(egui::vec2(90.0, 25.0)),
        );
    });
}
fn priorities(ctx: &egui::Context, d: &mut Draft) {
    let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    egui::Window::new("Edit Rule Priorities")
        .id(egui::Id::new("rule_priorities_v3"))
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .fixed_size([600.0, 344.0])
        .movable(false)
        .current_pos(widgets::position(
            ctx,
            egui::Id::new("rule_priorities_v3"),
            ctx.screen_rect().center() - egui::vec2(308.0, 180.0),
        ))
        .frame(
            egui::Frame::new()
                .fill(Color32::from_gray(76))
                .inner_margin(7.0)
                .stroke(egui::Stroke::new(1.0, widgets::LINE)),
        )
        .show(ctx, |ui| {
            widgets::style(ui);
            if widgets::title(ui, "Edit Rule Priorities") || escape {
                d.priorities = false;
            }
            ui.horizontal(|ui| {
                ui.label("Rule Type");
                egui::ComboBox::from_id_salt("priority_type")
                    .width(ui.available_width())
                    .selected_text(name(&d.priority_kind))
                    .show_ui(ui, |ui| {
                        for (_, label, kind) in CATALOG {
                            if ui
                                .selectable_label(d.priority_kind == *kind, *label)
                                .clicked()
                            {
                                d.priority_kind = (*kind).into();
                                if let Some(i) = sorted_rules(d, kind).first() {
                                    d.selected = *i;
                                }
                            }
                        }
                    });
            });
            let indices = sorted_rules(d, &d.priority_kind);
            let widths = [44.0, 48.0, 100.0, 100.0, 198.0];
            egui::Frame::new()
                .fill(widgets::BG)
                .stroke(egui::Stroke::new(1.0, widgets::LINE))
                .show(ui, |ui| {
                    widgets::table_header(
                        ui,
                        &widths,
                        &["Priority", "Enabled", "Name", "Scope", "Attributes"],
                    );
                    egui::ScrollArea::both()
                        .id_salt("priority_rows")
                        .max_height(239.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.set_min_height(239.0);
                            for i in &indices {
                                let (r, response) = ui.allocate_exact_size(
                                    egui::vec2(widths.iter().sum(), 21.0),
                                    egui::Sense::click(),
                                );
                                if d.selected == *i {
                                    ui.painter().rect_filled(
                                        r,
                                        0.0,
                                        ui.visuals().selection.bg_fill,
                                    );
                                }
                                let mut x = r.left();
                                for (col, w) in widths.iter().enumerate() {
                                    let cell = egui::Rect::from_min_size(
                                        egui::pos2(x, r.top()),
                                        egui::vec2(*w, 21.0),
                                    );
                                    widgets::cell(ui, cell, |ui| {
                                        if col == 1 {
                                            let mut enabled = rules::enabled(&d.rules[*i]);
                                            if ui
                                                .add_enabled(
                                                    rules::supported(&d.priority_kind),
                                                    egui::Checkbox::without_text(&mut enabled),
                                                )
                                                .changed()
                                            {
                                                d.rules[*i].insert(
                                                    "ENABLED".into(),
                                                    if enabled { "TRUE" } else { "FALSE" }.into(),
                                                );
                                            }
                                        } else {
                                            let value = match col {
                                                0 => text(&d.rules[*i], "PRIORITY").into(),
                                                2 => text(&d.rules[*i], "NAME").into(),
                                                3 => text(&d.rules[*i], "SCOPE1EXPRESSION").into(),
                                                _ => widgets::attributes(&d.rules[*i]),
                                            };
                                            ui.add(egui::Label::new(value).truncate());
                                        }
                                    });
                                    x += w;
                                }
                                if response.clicked() {
                                    d.selected = *i;
                                }
                            }
                        });
                });
            ui.horizontal(|ui| {
                let pos = indices.iter().position(|i| *i == d.selected);
                for (label, delta) in [("Increase Priority", -1isize), ("Decrease Priority", 1)] {
                    let target = pos.map(|n| n as isize + delta);
                    let available = rules::supported(&d.priority_kind)
                        && target.is_some_and(|t| (0..indices.len() as isize).contains(&t));
                    if ui
                        .add_enabled(
                            available,
                            egui::Button::new(label).min_size(egui::vec2(106.0, 25.0)),
                        )
                        .clicked()
                    {
                        let mut ordered = indices.clone();
                        ordered.swap(pos.unwrap(), target.unwrap() as usize);
                        for (p, i) in ordered.iter().enumerate() {
                            d.rules[*i].insert("PRIORITY".into(), (p + 1).to_string());
                        }
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized([75.0, 25.0], egui::Button::new("Close"))
                        .clicked()
                    {
                        d.priorities = false;
                    }
                });
            });
        });
}
fn constraints(ui: &mut egui::Ui, r: &mut Fields, board: &Board) {
    let kind = text(r, "RULEKIND").to_owned();
    widgets::section(ui, "Constraints");
    ui.add_space(if kind == "LayerPairs" { 3.0 } else { 12.0 });
    if kind == "PolygonConnect" {
        polygon_constraints(ui, r);
        return;
    }
    if kind == "Clearance" {
        clearance_constraints(ui, r);
        return;
    }
    if kind == "Width" {
        width_constraints(ui, r, board);
        return;
    }
    if kind == "RoutingVias" {
        via_constraints(ui, r);
        return;
    }
    if kind == "HoleSize" {
        hole_size_constraints(ui, r);
        return;
    }
    if kind == "LayerPairs" {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(280.0, 26.0), egui::Sense::hover());
        widgets::child(
            ui,
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                rect.min + egui::vec2(16.0, 0.0),
                egui::vec2(260.0, 24.0),
            )),
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.spacing_mut().icon_width = 16.0;
                    ui.spacing_mut().icon_width_inner = 10.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(184.0, 20.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(egui::vec2(184.0, 20.0));
                            ui.label("Enforce layer pairs settings");
                        },
                    );
                    let mut value = !matches!(text(r, "ENFORCE"), "FALSE" | "0");
                    if ui.checkbox(&mut value, "").changed() {
                        r.insert(
                            "ENFORCE".into(),
                            if value { "TRUE" } else { "FALSE" }.into(),
                        );
                    }
                });
            },
        );
        return;
    }
    if kind == "MinimumAnnularRing" {
        annular_ring_constraints(ui, r);
        return;
    }
    if kind == "HoleToHoleClearance" {
        hole_clearance_constraints(ui, r);
        return;
    }
    if matches!(kind.as_str(), "SolderMaskExpansion" | "PasteMaskExpansion") {
        mask_expansion_constraints(ui, r, kind == "PasteMaskExpansion");
        return;
    }
    if kind == "BoardOutlineClearance" {
        widgets::illustration(ui, &kind);
        dimension_field(ui, r, "GAP", "Minimum Clearance");
        outline_matrix(ui, r);
        return;
    }
    if matches!(
        kind.as_str(),
        "HoleSize"
            | "RoutingVias"
            | "MinimumAnnularRing"
            | "HoleToHoleClearance"
            | "SolderMaskExpansion"
            | "PasteMaskExpansion"
            | "MinimumSolderMaskSliver"
            | "SilkToSolderMaskClearance"
    ) {
        widgets::illustration(ui, &kind);
        ui.add_space(10.0);
    }
    if kind == "UnpouredPolygon" {
        flag(ui, r, "ALLOWSHELVED", "Allow Shelved Polygons", false);
        flag(ui, r, "ALLOWUNPOURED", "Allow Unpoured Polygons", false);
    }
    for (key, label) in rules::fields(&kind) {
        dimension_field(ui, r, key, label);
    }
    if kind == "PasteMaskExpansion" {
        field(ui, r, "PERCENTAGE", "Percentage");
    }
    if kind == "SolderMaskExpansion" {
        flag(ui, r, "ISTENTINGTOP", "Tented - Top", false);
        flag(ui, r, "ISTENTINGBOTTOM", "Tented - Bottom", false);
    }
    if kind == "ShortCircuit" {
        flag(ui, r, "ALLOWED", "Allow Short Circuit", false);
    }
    if kind == "SilkToSolderMaskClearance" {
        let mut exposed = text(r, "CLEARANCETOEXPOSEDCOPPER") == "TRUE";
        let before = exposed;
        ui.radio_value(&mut exposed, true, "Check Clearance To Exposed Copper");
        ui.radio_value(
            &mut exposed,
            false,
            "Check Clearance To Solder Mask Openings",
        );
        if exposed != before {
            r.insert(
                "CLEARANCETOEXPOSEDCOPPER".into(),
                if exposed { "TRUE" } else { "FALSE" }.into(),
            );
        }
    }
    if kind == "RoutingLayers" {
        let width = ui.available_width();
        widgets::table_header(ui, &[width - 110.0, 110.0], &["Layer Name", "Allowed"]);
        for layer in board.layers.iter().filter(|l| l.copper) {
            ui.horizontal(|ui| {
                ui.add_sized([width - 120.0, 21.0], egui::Label::new(&layer.name));
                flag(
                    ui,
                    r,
                    &format!("{}_V5", pcb::layer_name(layer.id).to_ascii_uppercase()),
                    "",
                    true,
                );
            });
        }
    }
}
fn editor(doc: &mut OpenDocument, ctx: &egui::Context, d: &mut Draft) -> bool {
    let board = doc.model.pcb.as_ref().unwrap();
    if let Some(mut wizard) = d.wizard.take() {
        if !wizard_ui::show(ctx, board, d, &mut wizard) {
            d.wizard = Some(wizard);
        }
        return false;
    }
    let mut close = !d.priorities
        && d.query.is_none()
        && d.error.is_empty()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut save = false;
    let mut ok = false;
    let size = egui::vec2(
        1300.0_f32.min(ctx.screen_rect().width() - 32.0),
        823.0_f32.min(ctx.screen_rect().height() - 40.0),
    );
    egui::Window::new("PCB Rules and Constraints Editor [mm]").id(egui::Id::new(("pcb_rules_v2",doc.session_id))).title_bar(false).fixed_size(size).resizable(false).collapsible(false).interactable(!d.priorities && d.query.is_none() && d.error.is_empty())
        .movable(false).current_pos(widgets::position(ctx,egui::Id::new(("pcb_rules_v2",doc.session_id)),ctx.screen_rect().center()-(size+egui::vec2(16.0,16.0))*0.5))
        .frame(egui::Frame::new().fill(Color32::from_gray(76)).inner_margin(7.0).stroke(egui::Stroke::new(1.0,widgets::LINE)))
        .show(ctx,|ui|{
            widgets::style(ui);ui.scope(|ui|{
            close|=widgets::title(ui,"PCB Rules and Constraints Editor [mm]");
            let body_height=size.y-64.0;let (body,_)=ui.allocate_exact_size(egui::vec2(ui.available_width(),body_height),egui::Sense::hover());
            ui.painter().rect_filled(body,0.0,widgets::BG);
            let left=egui::Rect::from_min_size(body.min+egui::vec2(7.0,7.0),egui::vec2(250.0,body.height()-14.0));
            widgets::child(ui, egui::UiBuilder::new().max_rect(left),|ui|{
                theme::search_field_height(ui,&mut d.search,250.0,21.0);ui.add_space(4.0);
                egui::Frame::new().fill(Color32::from_gray(58)).stroke(egui::Stroke::new(1.0,widgets::LINE)).show(ui,|ui|{
                    egui::ScrollArea::vertical().id_salt("rules_tree_v3").max_height(left.height()-30.0).auto_shrink([false,false]).show(ui,|ui|{ui.set_min_width(238.0);ui.spacing_mut().item_spacing.y=0.0;tree(ui,d);});
                });
            });
            let right=egui::Rect::from_min_max(body.min+egui::vec2(265.0,7.0),body.max-egui::vec2(7.0,7.0));
            widgets::child(ui, egui::UiBuilder::new().max_rect(right),|ui|{
                if d.page!=RulePage::Rule{summary(ui,d,right.height());}else{
                    let detail=right.shrink2(egui::vec2(17.0,0.0));
                    widgets::child(ui, egui::UiBuilder::new().max_rect(detail),|ui|{
                        ui.add_space(20.0);
                        egui::ScrollArea::vertical().id_salt("rule_constraints").max_height(right.height()-24.0).auto_shrink([false,false]).show(ui,|ui|{
                            if let Some(r)=d.rules.get_mut(d.selected){let kind=text(r,"RULEKIND").to_owned();let supported=rules::supported(&kind);
                                ui.add_enabled_ui(supported,|ui|{rule_heading(ui,r,board,&mut d.error);scope_field(ui,r,"SCOPE1EXPRESSION",if matches!(kind.as_str(),"Clearance"|"ShortCircuit"|"HoleToHoleClearance"|"PolygonConnect"|"MinimumSolderMaskSliver"|"SilkToSolderMaskClearance"){"Where The First Object Matches"}else{"Where The Object Matches"},board,&mut d.query);
                                    if matches!(kind.as_str(),"Clearance"|"ShortCircuit"|"HoleToHoleClearance"|"PolygonConnect"|"MinimumSolderMaskSliver"|"SilkToSolderMaskClearance"){scope_field(ui,r,"SCOPE2EXPRESSION","Where The Second Object Matches",board,&mut d.query);}
                                    constraints(ui,r,board);
                                });
                                if !supported{ui.add_space(15.0);ui.colored_label(Color32::from_rgb(225,192,107),"This rule type is preserved. Its constraints and checker are not implemented.");for(k,v)in r.iter(){ui.horizontal_wrapped(|ui|{ui.label(k);ui.monospace(v);});}}
                            }else{ui.label("Select a rule in the tree.");}
                        });
                    });
                }
            });
            ui.add_space(4.0);ui.horizontal(|ui|{
                ui.add_enabled(false,egui::Button::new("Switch to Document View").min_size(egui::vec2(158.0,25.0)));
                if ui.add_sized([96.0,25.0],egui::Button::new("Rule Wizard…")).clicked(){d.wizard=Some(wizard_ui::Wizard::default());}
                if ui.add_sized([96.0,25.0],egui::Button::new("Priorities…")).clicked(){d.priority_kind=active_kind(d);d.priorities=true;}
                if ui.add_sized([124.0,25.0],egui::Button::new("Create Default Rules")).clicked(){for(_,_,kind)in rules::KINDS{if !d.rules.iter().any(|r|text(r,"RULEKIND")==*kind){d.rules.push(rules::default_rule(kind));}}}
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if ui.add_sized([74.0,25.0],egui::Button::new("Apply")).clicked(){save=true;}
                    if ui.add_sized([74.0,25.0],egui::Button::new("Cancel")).clicked(){close=true;}
                    if ui.add_sized([74.0,25.0],egui::Button::new("OK")).clicked(){save=true;ok=true;}
                });
            });
            });
        });
    if d.priorities {
        priorities(ctx, d);
    }
    if let Some(mut query) = d.query.take()
        && let Some(rule) = d.rules.get_mut(d.selected)
        && !query_ui::show(ctx, board, &mut query, rule)
    {
        d.query = Some(query);
    }

    if save {
        match apply(doc, d) {
            Ok(()) => close |= ok,
            Err(e) => d.error = e,
        }
    }
    widgets::message(ctx, ("rules_status", doc.session_id), &mut d.error);
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
    fn browsing_every_rule_and_summary_page_keeps_native_fields_unchanged() {
        let mut doc = document();
        doc.model.pcb.as_mut().unwrap().rules = CATALOG
            .iter()
            .map(|(_, _, kind)| rules::default_rule(kind))
            .collect();
        let mut percent = rules::default_rule("HoleSize");
        percent.insert("ABSOLUTEVALUES".into(), "FALSE".into());
        percent.insert("MINPERCENT".into(), "25.5".into());
        doc.model.pcb.as_mut().unwrap().rules.push(percent);
        doc.pcb_editor.rules.online = false;
        let before = doc.model.clone();
        open(&mut doc);
        let mut draft = doc.pcb_editor.rules.editor.take().unwrap();
        let original = draft.rules.clone();
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1707.0, 1019.0),
            )),
            ..Default::default()
        };
        for selected in 0..draft.rules.len() {
            draft.selected = selected;
            draft.page = RulePage::Rule;
            let _ = ctx.run(input.clone(), |ctx| {
                editor(&mut doc, ctx, &mut draft);
            });
            assert_eq!(
                draft.rules,
                original,
                "Viewing {} changed native fields",
                text(&original[selected], "RULEKIND")
            );
        }
        for page in std::iter::once(RulePage::Root)
            .chain(CATEGORIES.iter().map(|c| RulePage::Category((*c).into())))
            .chain(CATALOG.iter().map(|(_, _, k)| RulePage::Kind((*k).into())))
        {
            draft.page = page;
            let _ = ctx.run(input.clone(), |ctx| {
                editor(&mut doc, ctx, &mut draft);
            });
            assert_eq!(draft.rules, original);
        }
        assert_eq!(doc.model, before);
        assert_eq!(doc.history.revision, 0);
    }
    #[test]
    fn inserting_another_type_cannot_clone_the_previous_type() {
        let mut doc = document();
        open(&mut doc);
        let mut draft = doc.pcb_editor.rules.editor.take().unwrap();
        insert_rule(&mut draft, "Clearance", Some(0));
        assert_eq!(text(&draft.rules[draft.selected], "RULEKIND"), "Clearance");
        assert_eq!(text(&draft.rules[0], "PRIORITY"), "1");
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
    fn percent_hole_rule_apply_undo_and_summary_keep_both_measurement_modes() {
        let mut doc = document();
        doc.model.pcb.as_mut().unwrap().rules = vec![rules::default_rule("HoleSize")];
        let before = doc.model.clone();
        open(&mut doc);
        let mut draft = doc.pcb_editor.rules.editor.take().unwrap();
        draft.rules[0].insert("ABSOLUTEVALUES".into(), "FALSE".into());
        draft.rules[0].insert("MINPERCENT".into(), "25.5".into());
        draft.rules[0].insert("MAXPERCENT".into(), "82.25".into());
        assert!(widgets::attributes(&draft.rules[0]).contains("25.5%"));
        apply(&mut doc, &mut draft).unwrap();
        assert_eq!(doc.history.revision, 1);
        let r = &doc.model.pcb.as_ref().unwrap().rules[0];
        assert_eq!(
            r["MINLIMIT"],
            before.pcb.as_ref().unwrap().rules[0]["MINLIMIT"]
        );
        assert_eq!(r["MAXPERCENT"], "82.25");
        doc.history.undo(&mut doc.model);
        assert_eq!(doc.model, before);
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
