//! Observed AD 24.4.1 Update From Library wizard for file-based symbols.
use super::*;
use eda_core::{library_edit, library_update as update, schematic_edit};
use sha2::{Digest, Sha256};
use std::path::Path;

#[path = "library_update_parameters.rs"]
mod parameters;

fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
fn document_path_matches(doc: &OpenDocument, path: &Path) -> bool {
    doc.path
        .iter()
        .chain(&doc.source_path)
        .any(|p| canonical(p) == path)
}
fn fingerprint(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path)
        .map(|v| Sha256::digest(v).to_vec())
        .map_err(|e| format!("{}: {e}", path.display()))
}
struct Input {
    path: PathBuf,
    session: Option<(Uuid, u64)>,
    hash: Vec<u8>,
}
impl Input {
    fn check(&self, docs: &[OpenDocument]) -> Result<(), String> {
        if let Some((id, revision)) = self.session {
            let d = docs
                .iter()
                .find(|d| d.session_id == id)
                .ok_or("A compared document was closed")?;
            if d.history.revision != revision
                || d.sch_editor.active()
                || d.view.sch_preview.is_some()
                || d.path.is_some() && !document_path_matches(d, &self.path)
            {
                return Err(format!(
                    "{} changed or has an unfinished edit; reopen Update From Library",
                    d.model.name
                ));
            }
        } else {
            if docs.iter().any(|d| document_path_matches(d, &self.path)) {
                return Err("A compared file was opened; reopen Update From Library".into());
            }
            if fingerprint(&self.path)? != self.hash {
                return Err(format!(
                    "{} changed on disk; reopen Update From Library",
                    self.path.display()
                ));
            }
        }
        Ok(())
    }
}
struct Sheet {
    input: Input,
    model: Document,
    enabled: bool,
}
struct Source {
    input: Input,
    library: eda_core::library::Library,
}
struct Row {
    sheet: usize,
    component: Uuid,
    designator: String,
    comment: String,
    reference: String,
    source: String,
    definition: Result<schematic_edit::Fragment, String>,
    enabled: bool,
    full: bool,
    options: update::Options,
    pins_change: bool,
    eco_actions: [bool; 4],
    protected: std::collections::BTreeSet<String>,
}
pub(super) struct Dialog {
    sheets: Vec<Sheet>,
    sources: Vec<Source>,
    rows: Vec<Row>,
    page: u8,
    full: bool,
    options: update::Options,
    advanced: bool,
    parameters: bool,
    report: bool,
    selected_row: Option<usize>,
    parameter_matrix: parameters::Matrix,
    selected_scope: bool,
    validated: bool,
    done: bool,
    error: String,
}
fn source_from_document(doc: &OpenDocument) -> Result<Source, String> {
    let mut symbols = vec![];
    let mut names = std::collections::BTreeSet::new();
    for member in library_edit::members(&doc.model) {
        let name = library_edit::name(member);
        if !names.insert(name.to_owned()) {
            continue;
        }
        let c = &member
            .schematic
            .as_ref()
            .ok_or("Not a symbol library")?
            .components[0];
        let variants = library_edit::members(&doc.model)
            .filter(|m| library_edit::name(m) == name)
            .map(|m| {
                let s = m.schematic.as_ref().unwrap();
                Ok(eda_core::library::SymbolVariant {
                    part: s.components[0].part,
                    display_mode: s.components[0].display_mode,
                    fragment: schematic_edit::extract(s, s.components[0].id)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        symbols.push(eda_core::library::Symbol {
            name: name.into(),
            description: c.description.clone(),
            parts: c.settings.part_count,
            display_modes: variants.iter().map(|v| v.display_mode).max().unwrap_or(0) + 1,
            variants,
        });
    }
    Ok(Source {
        input: Input {
            path: canonical(
                &doc.path
                    .clone()
                    .unwrap_or_else(|| PathBuf::from(&doc.model.name)),
            ),
            session: Some((doc.session_id, doc.history.revision)),
            hash: vec![],
        },
        library: eda_core::library::Library {
            name: doc.model.name.clone(),
            embedded_footprints: None,
            symbols,
            warnings: vec![],
        },
    })
}
fn resolve(
    c: &eda_core::schematic::Component,
    sheet: &Path,
    sources: &[Source],
    forced: Option<usize>,
) -> Result<(schematic_edit::Fragment, String), String> {
    let source_path = PathBuf::from(&c.settings.source);
    let library_file = c
        .settings
        .source
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default();
    let exact = if source_path.is_absolute() {
        canonical(&source_path)
    } else {
        canonical(&sheet.parent().unwrap_or(Path::new(".")).join(&source_path))
    };
    let mut candidates: Vec<_> = sources
        .iter()
        .enumerate()
        .filter(|(i, s)| {
            forced.is_none_or(|n| n == *i)
                && (forced.is_some()
                    || c.settings.source.is_empty()
                    || s.input.path == exact
                    || s.input
                        .path
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(library_file)))
        })
        .flat_map(|(_, s)| {
            s.library
                .symbols
                .iter()
                .filter(|symbol| symbol.name.eq_ignore_ascii_case(&c.library_reference))
                .map(move |symbol| (s, symbol))
        })
        .collect();
    // An explicit source path wins over equally named available libraries.
    if forced.is_none() && candidates.iter().any(|(s, _)| s.input.path == exact) {
        candidates.retain(|(s, _)| s.input.path == exact);
    }
    if candidates.is_empty() {
        return Err(format!(
            "Not Found: {} in {}",
            c.library_reference, c.settings.source
        ));
    }
    if candidates.len() != 1 {
        return Err(format!(
            "Ambiguous library source for {}",
            c.library_reference
        ));
    }
    let (s, symbol) = candidates[0];
    let f = symbol
        .variant(c.part, c.display_mode)
        .ok_or("The part or display mode is absent from the library")?
        .fragment
        .clone();
    Ok((
        f,
        s.input
            .path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    ))
}
impl Dialog {
    fn check(&self, docs: &[OpenDocument]) -> Result<(), String> {
        for s in &self.sheets {
            s.input.check(docs)?;
        }
        for s in &self.sources {
            s.input.check(docs)?;
        }
        for s in self.sheets.iter().filter(|s| s.enabled) {
            if let Some((id, _)) = s.input.session {
                let d = docs.iter().find(|d| d.session_id == id).unwrap();
                if d.read_only || d.sch_editor.compiled_path.is_some() {
                    return Err("A target sheet is read-only".into());
                }
            }
        }
        Ok(())
    }
    fn prepare(&self, docs: &[OpenDocument]) -> Result<Vec<(usize, Document)>, String> {
        self.check(docs)?;
        let mut prepared = vec![];
        for (index, sheet) in self.sheets.iter().enumerate().filter(|(_, s)| s.enabled) {
            let changes = self
                .rows
                .iter()
                .filter(|r| r.sheet == index && r.enabled)
                .map(|r| {
                    Ok(update::Update {
                        component: r.component,
                        definition: r.definition.clone()?,
                        options: r.options.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            if !changes.is_empty() {
                prepared.push((index, update::preview(&sheet.model, &changes)?));
            }
        }
        if prepared.is_empty() {
            return Err("Select at least one available component to update".into());
        }
        Ok(prepared)
    }
    fn execute(&mut self, docs: &mut Vec<OpenDocument>) -> Result<usize, String> {
        if !self.validated {
            return Err("Validate Changes before Execute Changes".into());
        }
        let prepared = self.prepare(docs)?;
        // All scenes and identities are validated before the first history is changed.
        for (_, next) in &prepared {
            next.validate()?;
        }
        let mut count = 0;
        for (sheet, next) in prepared {
            let input = &self.sheets[sheet].input;
            let index = if let Some((id, _)) = input.session {
                docs.iter().position(|d| d.session_id == id).unwrap()
            } else {
                docs.push(OpenDocument::new(
                    self.sheets[sheet].model.clone(),
                    Some(input.path.clone()),
                ));
                docs.len() - 1
            };
            let d = &mut docs[index];
            if d.history.commit(&mut d.model, next)? {
                count += 1;
            }
        }
        self.done = true;
        self.validated = false;
        Ok(count)
    }
}
fn eco_group_matches(r: &Row, sheet: &Document, group: usize) -> bool {
    let Ok(f) = &r.definition else { return false };
    let Some(c) = sheet
        .schematic
        .as_ref()
        .and_then(|s| s.components.iter().find(|c| c.id == r.component))
    else {
        return false;
    };
    if group < 3 {
        return r.options.parameters
            && update::parameter_changes(c, f).is_ok_and(|changes| {
                changes.iter().any(|p| {
                    let kind = if p.sheet.is_none() {
                        1
                    } else if p.library.is_none() {
                        2
                    } else {
                        0
                    };
                    kind == group
                        && (kind != 1 || r.options.add_parameters)
                        && (kind != 2 || r.options.remove_parameters)
                })
            });
    }
    let source = &f.components[0];
    match group {
        3 => r.eco_actions[0],
        4 => r.eco_actions[1] && source.settings.models != c.settings.models,
        _ => {
            r.eco_actions[2] && source.settings.designator_locked != c.settings.designator_locked
                || r.eco_actions[3] && source.settings.part_locked != c.settings.part_locked
        }
    }
}
fn show_eco(ui: &mut egui::Ui, d: &mut Dialog, height: f32) {
    ui.label("Modifications");
    ui.horizontal(|ui| {
        for (label, width) in [
            ("Enable", 65.),
            ("Action", 160.),
            ("Affected Object", 365.),
            ("Affected Document", 210.),
            ("Check", 40.),
            ("Done", 40.),
        ] {
            ui.add_sized([width, 18.], egui::Label::new(label));
        }
    });
    let mut changed = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::ScrollArea::both()
            .id_salt("library_update_eco")
            .max_height((height - 480.).clamp(260., 365.))
            .min_scrolled_height(320.)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (group, title) in [
                    (0, "Change Parameter Value"),
                    (1, "Add Parameter"),
                    (2, "Remove Parameter"),
                    (3, "Update Part Symbol"),
                    (4, "Update Models"),
                    (5, "Update Lock Fields"),
                ] {
                    let count = d
                        .rows
                        .iter()
                        .filter(|r| r.enabled && d.sheets[r.sheet].enabled && r.definition.is_ok())
                        .filter(|r| eco_group_matches(r, &d.sheets[r.sheet].model, group))
                        .count();
                    if count == 0 {
                        continue;
                    }
                    egui::CollapsingHeader::new(title)
                        .id_salt(("library_eco_group", group))
                        .default_open(true)
                        .show(ui, |ui| {
                            for r in d
                                .rows
                                .iter_mut()
                                .filter(|r| r.enabled && d.sheets[r.sheet].enabled)
                            {
                                let Ok(f) = &r.definition else { continue };
                                let sheet = &d.sheets[r.sheet].model;
                                let c = sheet
                                    .schematic
                                    .as_ref()
                                    .unwrap()
                                    .components
                                    .iter()
                                    .find(|c| c.id == r.component)
                                    .unwrap();
                                let mut entries = vec![];
                                if group < 3 {
                                    if !r.options.parameters {
                                        continue;
                                    }
                                    if let Ok(changes) = update::parameter_changes(c, f) {
                                        for change in changes {
                                            let kind = if change.sheet.is_none() {
                                                1
                                            } else if change.library.is_none() {
                                                2
                                            } else {
                                                0
                                            };
                                            if kind != group
                                                || kind == 1 && !r.options.add_parameters
                                                || kind == 2 && !r.options.remove_parameters
                                            {
                                                continue;
                                            }
                                            let text = format!(
                                                "{}: {} [{} → {}]",
                                                r.designator,
                                                change.name,
                                                change.sheet.as_deref().unwrap_or("—"),
                                                change.library.as_deref().unwrap_or("—")
                                            );
                                            entries.push((Some(change.name), text));
                                        }
                                    }
                                } else {
                                    let show = eco_group_matches(r, sheet, group);
                                    if show {
                                        entries.push((
                                            None,
                                            if group == 3 && r.pins_change {
                                                format!("{} — pin connections change", r.designator)
                                            } else {
                                                r.designator.clone()
                                            },
                                        ));
                                    }
                                }
                                for (parameter, text) in entries {
                                    let mut enabled = parameter.as_ref().map_or_else(
                                        || match group {
                                            3 => r.options.graphics,
                                            4 => r.options.models,
                                            _ => r.options.designator_lock || r.options.part_lock,
                                        },
                                        |p| r.options.updates(p),
                                    );
                                    ui.horizontal(|ui| {
                                        ui.allocate_ui(egui::vec2(50., 18.), |ui| {
                                            changed |= ui
                                                .add_enabled(
                                                    !d.done
                                                        && parameter.as_ref().is_none_or(|name| {
                                                            !r.protected.iter().any(|p| {
                                                                p.eq_ignore_ascii_case(name)
                                                            })
                                                        }),
                                                    egui::Checkbox::without_text(&mut enabled),
                                                )
                                                .changed();
                                        });
                                        ui.add_sized(
                                            [160., 18.],
                                            egui::Label::new(match group {
                                                1 => "Add",
                                                2 => "Remove",
                                                _ => "Modify",
                                            }),
                                        );
                                        ui.add_sized(
                                            [365., 18.],
                                            egui::Label::new(&text).truncate(),
                                        )
                                        .on_hover_text(&text);
                                        ui.add_sized(
                                            [210., 18.],
                                            egui::Label::new(&sheet.name).truncate(),
                                        );
                                        ui.allocate_ui(egui::vec2(40., 18.), |ui| {
                                            design_sync::status_icon(
                                                ui,
                                                (enabled && (d.validated || d.done))
                                                    .then_some(true),
                                            )
                                        });
                                        ui.allocate_ui(egui::vec2(40., 18.), |ui| {
                                            design_sync::status_icon(
                                                ui,
                                                (enabled && d.done).then_some(true),
                                            )
                                        });
                                    });
                                    if let Some(name) = parameter {
                                        if enabled {
                                            r.options.excluded_parameters.remove(&name);
                                        } else {
                                            r.options.excluded_parameters.insert(name);
                                        }
                                    } else {
                                        match group {
                                            3 => r.options.graphics = enabled,
                                            4 => r.options.models = enabled,
                                            _ => {
                                                r.options.designator_lock =
                                                    enabled && r.eco_actions[2];
                                                r.options.part_lock = enabled && r.eco_actions[3];
                                            }
                                        }
                                    }
                                }
                            }
                        });
                }
            });
    });
    if changed {
        d.validated = false;
        d.error.clear();
    }
}

fn show_types(ui: &mut egui::Ui, d: &mut Dialog) {
    ui.strong("Component Types");
    egui::Grid::new("library_update_types")
        .striped(true)
        .min_col_width(0.)
        .spacing([0., 2.])
        .show(ui, |ui| {
            for (label, width) in [
                ("", 49.),
                ("Physical Component", 140.),
                ("Logical Symbol", 151.),
                ("Library", 208.),
                ("Count", 89.),
            ] {
                ui.add_sized(
                    [width, 18.],
                    egui::Label::new(label).truncate().halign(egui::Align::Min),
                );
            }
            ui.end_row();
            let mut groups = std::collections::BTreeSet::new();
            for n in 0..d.rows.len() {
                if !d.sheets[d.rows[n].sheet].enabled {
                    continue;
                }
                let key = (d.rows[n].reference.clone(), d.rows[n].source.clone());
                if !groups.insert(key.clone()) {
                    continue;
                }
                let matching = |r: &Row| {
                    d.sheets[r.sheet].enabled && (r.reference.clone(), r.source.clone()) == key
                };
                let count = d.rows.iter().filter(|r| matching(r)).count();
                let mut enabled = d.rows.iter().filter(|r| matching(r)).all(|r| r.enabled);
                let changed = ui
                    .allocate_ui_with_layout(
                        egui::vec2(49., 20.),
                        egui::Layout::top_down(egui::Align::Center),
                        |ui| ui.checkbox(&mut enabled, "").changed(),
                    )
                    .inner;
                if changed {
                    for r in &mut d.rows {
                        if (r.reference.clone(), r.source.clone()) == key {
                            r.enabled = enabled && r.definition.is_ok();
                        }
                    }
                }
                for (text, width) in [
                    (key.0.as_str(), 140.),
                    (key.0.as_str(), 151.),
                    (key.1.as_str(), 208.),
                    (&count.to_string(), 89.),
                ] {
                    ui.add_sized(
                        [width, 20.],
                        egui::Label::new(text).truncate().halign(egui::Align::Min),
                    )
                    .on_hover_text(text);
                }
                ui.end_row();
            }
        });
}

fn show_parts(ui: &mut egui::Ui, d: &mut Dialog, height: f32) {
    ui.strong("Use this page to control the changes to individual schematic parts");
    ui.label("Components unavailable in their specified library are flagged as Not Found.");
    ui.add_space(44.);
    let widths = [
        49., 74., 67., 94., 80., 86., 83., 176., 67., 60., 50., 50., 38.,
    ];
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.;
        ui.horizontal(|ui| {
            for (label, width) in [
                ("Schematic Parts", 364.),
                ("Library Components", 412.),
                ("Actions", 198.),
            ] {
                ui.add_sized(
                    [width, 18.],
                    egui::Label::new(label).halign(egui::Align::Min),
                );
            }
        });
        egui::ScrollArea::both()
            .id_salt("library_update_parts")
            .max_height((height - 280.).clamp(250., 582.))
            .min_scrolled_height(380.)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("library_update_part_grid")
                    .striped(true)
                    .min_col_width(0.)
                    .spacing([0., 2.])
                    .show(ui, |ui| {
                        for (label, width) in [
                            "Update",
                            "Designator",
                            "Comment",
                            "Physical Component",
                            "Logical Symbol",
                            "Physical Component",
                            "Logical Symbol",
                            "Library Name",
                            "Life Cycle State",
                            "Full Replace",
                            "Graphical",
                            "Parameters",
                            "Models",
                        ]
                        .into_iter()
                        .zip(widths)
                        {
                            ui.add_sized(
                                [width, 18.],
                                egui::Label::new(label).truncate().halign(egui::Align::Min),
                            )
                            .on_hover_text(label);
                        }
                        ui.end_row();
                        for (index, r) in d
                            .rows
                            .iter_mut()
                            .enumerate()
                            .filter(|(_, r)| d.sheets[r.sheet].enabled)
                        {
                            let check = |ui: &mut egui::Ui, value: &mut bool, width, enabled| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(width, 20.),
                                    egui::Layout::top_down(egui::Align::Center),
                                    |ui| {
                                        ui.add_enabled(enabled, egui::Checkbox::without_text(value))
                                            .changed()
                                    },
                                )
                                .inner
                            };
                            let mut changed = check(
                                ui,
                                &mut r.enabled,
                                widths[0],
                                !d.done && r.definition.is_ok(),
                            );
                            let response = ui.add_sized(
                                [widths[1], 20.],
                                egui::Button::selectable(
                                    d.selected_row == Some(index),
                                    &r.designator,
                                ),
                            );
                            if response.clicked() {
                                d.selected_row = Some(index);
                            }
                            if let Err(error) = &r.definition {
                                response.on_hover_text(error);
                            } else if r.pins_change {
                                response.on_hover_text(
                                    "Pin connections change; external wires keep their positions",
                                );
                            }
                            for (text, width) in [
                                &r.comment,
                                &r.reference,
                                &r.reference,
                                &r.reference,
                                &r.reference,
                                &r.source
                                    .rsplit(['/', '\\'])
                                    .next()
                                    .unwrap_or(&r.source)
                                    .to_owned(),
                                &String::new(),
                            ]
                            .into_iter()
                            .zip(widths[2..9].iter())
                            {
                                ui.add_sized(
                                    [*width, 20.],
                                    egui::Label::new(text).truncate().halign(egui::Align::Min),
                                )
                                .on_hover_text(text);
                            }
                            if check(ui, &mut r.full, widths[9], !d.done && r.enabled) {
                                changed = true;
                                if r.full {
                                    r.options.graphics = true;
                                    r.options.parameters = true;
                                    r.options.models = true;
                                    r.options.designator_lock = d.options.designator_lock;
                                    r.options.part_lock = d.options.part_lock;
                                } else {
                                    r.options.designator_lock = false;
                                    r.options.part_lock = false;
                                }
                            }
                            let enabled = !d.done && r.enabled && !r.full;
                            changed |= check(ui, &mut r.options.graphics, widths[10], enabled);
                            changed |= check(ui, &mut r.options.parameters, widths[11], enabled);
                            changed |= check(ui, &mut r.options.models, widths[12], enabled);
                            ui.end_row();
                            if changed {
                                d.validated = false;
                                d.error.clear();
                            }
                        }
                    });
            });
    });
    ui.horizontal(|ui| {
        ui.add_enabled(
            false,
            egui::Button::new("Return Selected to Default").min_size(egui::vec2(152., 23.)),
        );
        ui.add_enabled(
            false,
            egui::Button::new("Choose Component...").min_size(egui::vec2(128., 23.)),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    d.rows
                        .iter()
                        .any(|r| r.enabled && !r.full && r.options.parameters),
                    egui::Button::new("Parameters Changes...").min_size(egui::vec2(128., 23.)),
                )
                .clicked()
            {
                d.parameters = true;
            }
        });
    });
}

impl PcbCraft {
    pub(super) fn open_library_update(&mut self, selected_only: bool, from_library: bool) {
        match self.make_library_update(selected_only, from_library) {
            Ok(mut d) => {
                if from_library {
                    d.validated = true;
                    match d.execute(&mut self.documents) {
                        Ok(_) => d.page = 3,
                        Err(e) => {
                            self.message(true, e);
                            return;
                        }
                    }
                }
                self.library_update = Some(d);
            }
            Err(e) => self.message(true, e),
        }
    }
    fn make_library_update(
        &self,
        selected_only: bool,
        from_library: bool,
    ) -> Result<Dialog, String> {
        let active = self
            .active
            .ok_or("Open a schematic or symbol library first")?;
        let mut sheets = vec![];
        if !selected_only && !from_library {
            for (path, mut model) in self.schematic_project_snapshots()? {
                if model.library.is_some() {
                    continue;
                }
                let path = canonical(&path);
                let open = self.documents.iter().find(|d| {
                    document_path_matches(d, &path)
                        || d.path.is_none() && d.source_path.is_none() && d.model.id == model.id
                });
                let hash = if open.is_none() {
                    let hash = fingerprint(&path)?;
                    model = eda_io::load(&path)?;
                    if fingerprint(&path)? != hash {
                        return Err("A schematic changed while loading; try again".into());
                    }
                    hash
                } else {
                    vec![]
                };
                sheets.push(Sheet {
                    input: Input {
                        path: path.clone(),
                        session: open.map(|d| (d.session_id, d.history.revision)),
                        hash,
                    },
                    model,
                    enabled: true,
                });
            }
        }
        let candidates: Vec<_> = if from_library {
            self.documents.iter().collect()
        } else {
            vec![&self.documents[active]]
        };
        for d in candidates {
            if d.model.schematic.is_none()
                || d.model.library.is_some()
                || sheets
                    .iter()
                    .any(|s| s.input.session.is_some_and(|(id, _)| id == d.session_id))
            {
                continue;
            }
            sheets.push(Sheet {
                input: Input {
                    path: canonical(
                        &d.path
                            .clone()
                            .or(d.source_path.clone())
                            .unwrap_or_else(|| PathBuf::from(&d.model.name)),
                    ),
                    session: Some((d.session_id, d.history.revision)),
                    hash: vec![],
                },
                model: d.model.clone(),
                enabled: true,
            });
        }
        if sheets.is_empty() {
            return Err("Open a schematic sheet to update its components".into());
        }
        let mut sources = self
            .documents
            .iter()
            .filter(|d| d.model.library.is_some() && d.model.schematic.is_some())
            .map(source_from_document)
            .collect::<Result<Vec<_>, _>>()?;
        let forced = if from_library {
            Some(
                sources
                    .iter()
                    .position(|s| {
                        s.input
                            .session
                            .is_some_and(|(id, _)| id == self.documents[active].session_id)
                    })
                    .ok_or("Open a symbol library first")?,
            )
        } else {
            None
        };
        let mut paths = self.settings.libraries.clone();
        paths.extend(self.components_ui.libraries.iter().map(|(p, _)| p.clone()));
        for sheet in &sheets {
            for c in &sheet.model.schematic.as_ref().unwrap().components {
                if c.settings.source.is_empty() {
                    continue;
                }
                let path = PathBuf::from(&c.settings.source);
                paths.push(if path.is_absolute() {
                    path
                } else {
                    sheet
                        .input
                        .path
                        .parent()
                        .unwrap_or(Path::new("."))
                        .join(path)
                });
            }
        }
        for path in paths {
            let path = canonical(&path);
            if sources.iter().any(|s| s.input.path == path)
                || !path.is_file()
                || !path.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("SchLib") || e.eq_ignore_ascii_case("IntLib")
                })
            {
                continue;
            }
            let hash = fingerprint(&path)?;
            // Keep a failed lookup visible as Not Found; ambiguous sources never resolve silently.
            if let Ok(library) = eda_io::altium::library::load(&path) {
                if fingerprint(&path)? != hash {
                    return Err("A library changed while loading; try again".into());
                }
                sources.push(Source {
                    input: Input {
                        hash,
                        path,
                        session: None,
                    },
                    library,
                });
            }
        }
        let selected = &self.documents[active].view;
        let mut rows = vec![];
        for (sheet, s) in sheets.iter().enumerate() {
            for c in &s.model.schematic.as_ref().unwrap().components {
                if selected_only
                    && selected.selected != Some(c.id)
                    && !selected.sch_selection.contains(&c.id)
                {
                    continue;
                }
                if from_library
                    && !sources[forced.unwrap()]
                        .library
                        .symbols
                        .iter()
                        .any(|symbol| symbol.name.eq_ignore_ascii_case(&c.library_reference))
                {
                    continue;
                }
                let resolved = resolve(c, &s.input.path, &sources, forced);
                let source = if c.settings.source.is_empty() {
                    resolved
                        .as_ref()
                        .map(|(_, s)| s.clone())
                        .unwrap_or_default()
                } else {
                    c.settings.source.clone()
                };
                let definition = resolved.map(|(f, _)| f);
                let pins_change = definition
                    .as_ref()
                    .ok()
                    .and_then(|f| {
                        update::pin_connections_change(
                            &s.model,
                            &update::Update {
                                component: c.id,
                                definition: f.clone(),
                                options: Default::default(),
                            },
                        )
                        .ok()
                    })
                    .unwrap_or(false);
                let mut options = update::Options::default();
                for p in s
                    .model
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .chain(&s.model.schematic.as_ref().unwrap().parameter_objects)
                    .filter(|p| p.component == Some(c.id))
                {
                    if p.presentation.locked || !p.presentation.allow_sync {
                        options.excluded_parameters.insert(match p.text_role {
                            Some(eda_core::schematic::TextRole::Comment) => "Comment".into(),
                            Some(eda_core::schematic::TextRole::Designator) => "Designator".into(),
                            _ => p.presentation.name.clone(),
                        });
                    }
                }
                if let Some(n) = forced {
                    options.source_library = Some(
                        sources[n]
                            .input
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    );
                    options.preserve_locations = false;
                    options.preserve_visibility = false;
                }
                let protected = options.excluded_parameters.clone();
                rows.push(Row {
                    sheet,
                    component: c.id,
                    designator: c.designator.clone(),
                    comment: c.comment.clone(),
                    reference: c.library_reference.clone(),
                    source,
                    enabled: definition.is_ok(),
                    full: true,
                    definition,
                    options,
                    pins_change,
                    eco_actions: [true; 4],
                    protected,
                });
            }
        }
        if rows.is_empty() {
            return Err("No matching schematic components in the selected scope".into());
        }
        let d = Dialog {
            sheets,
            sources,
            rows,
            page: 0,
            full: true,
            options: Default::default(),
            advanced: false,
            parameters: false,
            report: false,
            selected_row: None,
            parameter_matrix: Default::default(),
            selected_scope: selected_only,
            validated: false,
            done: false,
            error: String::new(),
        };
        d.check(&self.documents)?;
        Ok(d)
    }
    pub(super) fn library_update_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.library_update.take() else {
            return;
        };
        if d.page == 3 {
            let mut close = false;
            egui::Modal::new(egui::Id::new("library_update_information")).show(ctx, |ui| {
                ui.set_width(240.);
                ui.strong("Information");
                ui.separator();
                ui.label(format!(
                    "Updated {} components",
                    d.rows.iter().filter(|r| r.enabled).count()
                ));
                ui.label(format!(
                    "In {} Schematic Documents",
                    d.rows
                        .iter()
                        .filter(|r| r.enabled && d.sheets[r.sheet].enabled)
                        .map(|r| r.sheet)
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        close = true;
                    }
                });
            });
            close |= ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, Key::Escape)
                    || i.consume_key(egui::Modifiers::NONE, Key::Enter)
            });
            if !close {
                self.library_update = Some(d);
            }
            return;
        }
        let mut close = false;
        let mut next = false;
        let mut finish = false;
        let mut validate = false;
        let mut execute = false;
        egui::Modal::new(egui::Id::new("library_update_wizard")).backdrop_color(Color32::TRANSPARENT).frame(egui::Frame::window(&ctx.style()).corner_radius(0)).show(ctx,|ui|{
            ui.set_width((ctx.screen_rect().width()-48.).clamp(600.,if d.page==2 {1065.} else {990.}));
            ui.horizontal(|ui|{ui.strong(if d.page==2{"Engineering Change Order"}else{"Update From Library"});ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.small_button("×").clicked(){close=true;}});});
            ui.separator();
            if d.page==0 {
                ui.strong("Choose the schematic documents and component types to update");
                ui.label("Updates use the library specified in each part. Click Next to configure individual parts.");
                ui.add_space(38.);
                ui.horizontal_top(|ui|{
                    ui.allocate_ui_with_layout(egui::vec2(342.,(ctx.screen_rect().height()-260.).clamp(350.,620.)),egui::Layout::top_down(egui::Align::Min),|ui|{
                        ui.set_min_height((ctx.screen_rect().height()-386.).clamp(350.,633.));
                        ui.strong("Schematic Sheets");
                        egui::Frame::group(ui.style()).show(ui,|ui|{ui.set_min_width(327.);egui::ScrollArea::vertical().id_salt("library_update_sheets").min_scrolled_height(202.).max_height(202.).auto_shrink([false,false]).show(ui,|ui|{ui.label("Schematic Sheet");for s in &mut d.sheets {ui.checkbox(&mut s.enabled,&s.model.name);}});});
                        ui.add_space(12.);ui.strong("Settings");ui.label("Actions");
                        ui.radio_value(&mut d.full,true,"Fully replace symbols on sheet with those from library");
                        ui.add_enabled_ui(d.full,|ui|{ui.checkbox(&mut d.options.designator_lock,"Update designator lock field");ui.checkbox(&mut d.options.part_lock,"Update part ID lock field");});
                        ui.checkbox(&mut d.options.preserve_locations,"Preserve parameter locations");
                        ui.checkbox(&mut d.options.preserve_visibility,"Preserve parameter visibility");
                        ui.radio_value(&mut d.full,false,"Replace selected attributes of symbols on sheets");
                        ui.add_enabled_ui(!d.full,|ui|{ui.checkbox(&mut d.options.graphics,"Update graphical attributes");ui.checkbox(&mut d.options.parameters,"Update parameters");ui.checkbox(&mut d.options.models,"Update models");if ui.button("Advanced...").clicked(){d.advanced=true;}});
                        ui.add_space(8.);ui.label("Options");
                        ui.add_enabled(false,egui::Checkbox::new(&mut d.selected_scope,"Selected Parts Only"));
                        ui.add_enabled(false,egui::Checkbox::new(&mut true,"Include Variants"));
                        ui.add_enabled(false,egui::Checkbox::new(&mut true,"Update To Latest Revision"));
                        ui.add_enabled_ui(false,|ui|{ui.horizontal(|ui|{ui.label("Show");egui::ComboBox::from_id_salt("library_update_show").selected_text("All Components").show_ui(ui,|_|{});});});
                    });
                    ui.vertical(|ui| show_types(ui, &mut d));
                });
            } else if d.page == 2 {
                show_eco(ui, &mut d, ctx.screen_rect().height());
            } else {
                show_parts(ui, &mut d, ctx.screen_rect().height());
            }
            if !d.error.is_empty(){ui.colored_label(Color32::from_rgb(240,135,120),&d.error);}
            ui.separator();
            ui.horizontal(|ui|{
                if d.page==2 {
                    if ui.add_enabled(!d.done,egui::Button::new("Validate Changes")).clicked(){validate=true;}
                    if ui.add_enabled(d.validated&&!d.done,egui::Button::new("Execute Changes")).clicked(){execute=true;}
                    if ui.button("Report Changes...").clicked(){d.report=true;}
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                    if d.page==2 {if ui.add_sized([75.,23.],egui::Button::new("Close")).clicked(){close=true;}}
                    else {
                        if ui.add_sized([75.,23.],egui::Button::new("Finish")).clicked(){finish=true;}
                        if ui.add_enabled(d.page==0,egui::Button::new("Next >").min_size(egui::vec2(75.,23.))).clicked(){next=true;}
                        if ui.add_enabled(d.page==1,egui::Button::new("< Back").min_size(egui::vec2(75.,23.))).clicked(){d.page=0;}
                        if ui.add_sized([75.,23.],egui::Button::new("Cancel")).clicked(){close=true;}
                    }
                });
            });
        });
        if d.advanced {
            egui::Modal::new(egui::Id::new("library_update_advanced")).show(ctx, |ui| {
                ui.set_width(670.);
                ui.strong("Library Update Settings");
                ui.separator();
                ui.columns(2, |columns| {
                    let ui = &mut columns[0];
                    ui.strong("Parameter Settings");
                    ui.add_enabled_ui(d.options.parameters, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Library parameters not in sheet:");
                            egui::ComboBox::from_id_salt("update_add_parameters")
                                .selected_text(if d.options.add_parameters {
                                    "Add"
                                } else {
                                    "Do not add"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut d.options.add_parameters, true, "Add");
                                    ui.selectable_value(
                                        &mut d.options.add_parameters,
                                        false,
                                        "Do not add",
                                    );
                                    ui.add_enabled(false, egui::Button::new("Add if not blank"));
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Sheet parameters not in library:");
                            egui::ComboBox::from_id_salt("update_remove_parameters")
                                .selected_text(if d.options.remove_parameters {
                                    "Remove"
                                } else {
                                    "Do not remove"
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut d.options.remove_parameters,
                                        true,
                                        "Remove",
                                    );
                                    ui.selectable_value(
                                        &mut d.options.remove_parameters,
                                        false,
                                        "Do not remove",
                                    );
                                });
                        });
                        ui.add_space(12.);
                        ui.label("Parameters to update:");
                        let mut names = std::collections::BTreeSet::new();
                        for r in &d.rows {
                            if let Ok(f) = &r.definition {
                                let c = d.sheets[r.sheet]
                                    .model
                                    .schematic
                                    .as_ref()
                                    .unwrap()
                                    .components
                                    .iter()
                                    .find(|c| c.id == r.component)
                                    .unwrap();
                                if let Ok(changes) = update::parameter_changes(c, f) {
                                    names.extend(changes.into_iter().map(|c| c.name));
                                }
                            }
                        }
                        egui::Grid::new("library_update_advanced_parameters")
                            .striped(true)
                            .min_col_width(120.)
                            .show(ui, |ui| {
                                ui.label("Parameter");
                                ui.label("Update");
                                ui.end_row();
                                for name in names {
                                    let mut enabled =
                                        !d.options.excluded_parameters.contains(&name);
                                    ui.label(&name);
                                    if ui.checkbox(&mut enabled, "").changed() {
                                        if enabled {
                                            d.options.excluded_parameters.remove(&name);
                                        } else {
                                            d.options.excluded_parameters.insert(name);
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    });
                    let ui = &mut columns[1];
                    ui.strong("Model Settings");
                    for label in [
                        "Add models found in library but not in sheet",
                        "Remove models found in sheet but not in library",
                        "Update which models are the current models",
                    ] {
                        ui.add_enabled(false, egui::Checkbox::new(&mut true, label));
                    }
                    ui.label("Models to update:");
                    egui::Grid::new("library_update_advanced_models")
                        .striped(true)
                        .min_col_width(120.)
                        .show(ui, |ui| {
                            ui.label("Model Name");
                            ui.label("Update");
                            ui.end_row();
                            let mut names = std::collections::BTreeSet::new();
                            for r in &d.rows {
                                if let Ok(f) = &r.definition {
                                    names.extend(
                                        f.components[0]
                                            .settings
                                            .models
                                            .iter()
                                            .map(|m| m.name.clone()),
                                    );
                                }
                            }
                            for name in names {
                                ui.label(name);
                                ui.add_enabled(false, egui::Checkbox::without_text(&mut true));
                                ui.end_row();
                            }
                        });
                });
                ui.add_space(250.);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("OK").clicked() {
                        d.advanced = false;
                    }
                });
            });
        }
        if d.parameters {
            parameters::show(ctx, &mut d);
        }
        if d.report {
            egui::Modal::new(egui::Id::new("library_update_report")).show(ctx, |ui| {
                ui.set_width(950.);
                ui.strong("Engineering Change Order Report");
                let mut report =
                    String::from("Library Update\nDocument\tDesignator\tChange\tSheet\tLibrary\n");
                for r in d
                    .rows
                    .iter()
                    .filter(|r| r.enabled && d.sheets[r.sheet].enabled)
                {
                    let sheet = &d.sheets[r.sheet].model;
                    if let Ok(f) = &r.definition {
                        let c = sheet
                            .schematic
                            .as_ref()
                            .unwrap()
                            .components
                            .iter()
                            .find(|c| c.id == r.component)
                            .unwrap();
                        if let Ok(changes) = update::parameter_changes(c, f) {
                            for change in changes {
                                if r.options.updates(&change.name)
                                    && (change.sheet.is_some() || r.options.add_parameters)
                                    && (change.library.is_some() || r.options.remove_parameters)
                                {
                                    report.push_str(&format!(
                                        "{}\t{}\t{}\t{}\t{}\n",
                                        sheet.name,
                                        r.designator,
                                        change.name,
                                        change.sheet.as_deref().unwrap_or("—"),
                                        change.library.as_deref().unwrap_or("—")
                                    ));
                                }
                            }
                        }
                        for (enabled, label) in [
                            (r.options.graphics, "Update Part Symbol"),
                            (r.options.models, "Update Models"),
                        ] {
                            if enabled {
                                report.push_str(&format!(
                                    "{}\t{}\t{}\n",
                                    sheet.name, r.designator, label
                                ));
                            }
                        }
                    }
                }
                if ui.button("Copy Report").clicked() {
                    ctx.copy_text(report.clone());
                }
                egui::ScrollArea::both().max_height(430.).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut report)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(1400.)
                            .interactive(false),
                    );
                });
                if ui.button("Close").clicked() {
                    d.report = false;
                }
            });
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            if d.report {
                d.report = false;
            } else if d.parameters {
                d.parameters = false;
            } else if d.advanced {
                d.advanced = false;
            } else {
                close = true;
            }
        }
        if (next || finish) && d.page == 0 {
            for r in &mut d.rows {
                r.full = d.full;
                let source_library = r.options.source_library.clone();
                r.options = d.options.clone();
                r.options.source_library = source_library;
                if d.full {
                    r.options.graphics = true;
                    r.options.parameters = true;
                    r.options.models = true;
                    r.options.excluded_parameters.clear();
                } else {
                    r.options.designator_lock = false;
                    r.options.part_lock = false;
                }
                r.options.excluded_parameters.extend(r.protected.clone());
            }
        }
        if next {
            d.page = 1;
        }
        if finish {
            for r in &mut d.rows {
                r.eco_actions = [
                    r.options.graphics,
                    r.options.models,
                    r.options.designator_lock,
                    r.options.part_lock,
                ];
            }
            d.page = 2;
            d.validated = false;
        }
        if validate {
            match d.prepare(&self.documents) {
                Ok(_) => {
                    d.validated = true;
                    d.error.clear();
                }
                Err(e) => {
                    d.validated = false;
                    d.error = e;
                }
            }
        }
        if execute {
            match d.execute(&mut self.documents) {
                Ok(n) => {
                    ctx.request_repaint();
                    self.message(
                        false,
                        format!("Updated {n} schematic sheet(s) from libraries"),
                    );
                    d.error.clear();
                }
                Err(e) => {
                    d.validated = false;
                    d.error = e;
                }
            }
        }
        if !close {
            self.library_update = Some(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn fixture() -> (Vec<OpenDocument>, Dialog) {
        let mut docs = vec![];
        let mut sheets = vec![];
        let mut rows = vec![];
        for n in 0..2 {
            let mut model = library_edit::new(DocumentKind::Schematic, &format!("Sheet{n}.SchDoc"));
            model.library = None;
            let c = &model.schematic.as_ref().unwrap().components[0];
            let id = c.id;
            let mut definition =
                schematic_edit::extract(model.schematic.as_ref().unwrap(), id).unwrap();
            definition.components[0].description = "Updated".into();
            let open = OpenDocument::new(model.clone(), None);
            sheets.push(Sheet {
                input: Input {
                    path: PathBuf::from(&model.name),
                    session: Some((open.session_id, 0)),
                    hash: vec![],
                },
                model,
                enabled: true,
            });
            rows.push(Row {
                sheet: n,
                component: id,
                designator: "U?".into(),
                comment: String::new(),
                reference: "Component_1".into(),
                source: "Parts.SchLib".into(),
                definition: Ok(definition),
                enabled: true,
                full: true,
                options: Default::default(),
                pins_change: false,
                eco_actions: [true; 4],
                protected: Default::default(),
            });
            docs.push(open);
        }
        let d = Dialog {
            sheets,
            sources: vec![],
            rows,
            page: 2,
            full: true,
            options: Default::default(),
            advanced: false,
            parameters: false,
            report: false,
            selected_row: None,
            parameter_matrix: Default::default(),
            selected_scope: false,
            validated: true,
            done: false,
            error: String::new(),
        };
        (docs, d)
    }
    #[test]
    fn one_invalid_target_prevents_all_updates_and_history_entries() {
        let (mut docs, mut d) = fixture();
        d.rows[1].definition = Err("Missing library variant".into());
        assert!(d.execute(&mut docs).is_err());
        assert!(docs.iter().all(|doc| {
            doc.history.revision == 0
                && doc.model.schematic.as_ref().unwrap().components[0]
                    .description
                    .is_empty()
        }));
    }
    #[test]
    fn eco_omits_unchanged_models_locks_and_empty_parameter_groups() {
        let (_, mut d) = fixture();
        let row = &mut d.rows[0];
        let sheet = &d.sheets[0].model;
        assert!(eco_group_matches(row, sheet, 0));
        assert!(!eco_group_matches(row, sheet, 1));
        assert!(!eco_group_matches(row, sheet, 2));
        assert!(!eco_group_matches(row, sheet, 4));
        assert!(!eco_group_matches(row, sheet, 5));
        let f = row.definition.as_mut().unwrap();
        f.components[0]
            .settings
            .models
            .push(eda_core::schematic::ComponentModel {
                name: "LAND".into(),
                kind: "PCBLIB".into(),
                current: true,
                parameters: Default::default(),
            });
        f.components[0].settings.designator_locked = true;
        assert!(eco_group_matches(row, sheet, 4));
        assert!(eco_group_matches(row, sheet, 5));
        row.eco_actions[2] = false;
        assert!(!eco_group_matches(row, sheet, 5));
    }
    #[test]
    fn stale_source_or_target_prevents_execution_and_each_sheet_has_undo() {
        let (mut docs, mut d) = fixture();
        docs[1].history.revision = 1;
        assert!(d.execute(&mut docs).is_err());
        assert_eq!(docs[0].history.revision, 0);
        docs[1].history.revision = 0;
        assert_eq!(d.execute(&mut docs).unwrap(), 2);
        assert!(docs.iter().all(|doc| {
            doc.model.schematic.as_ref().unwrap().components[0].description == "Updated"
        }));
        for doc in &mut docs {
            doc.history.undo(&mut doc.model);
            assert!(
                doc.model.schematic.as_ref().unwrap().components[0]
                    .description
                    .is_empty()
            );
        }
        assert!(d.execute(&mut docs).is_err());
    }
    #[test]
    fn resolving_identically_named_libraries_requires_an_unambiguous_source() {
        let mut a = OpenDocument::new(
            library_edit::new(DocumentKind::Schematic, "A.SchLib"),
            Some(PathBuf::from("A.SchLib")),
        );
        let b = OpenDocument::new(
            library_edit::new(DocumentKind::Schematic, "B.SchLib"),
            Some(PathBuf::from("B.SchLib")),
        );
        let sources = vec![
            source_from_document(&a).unwrap(),
            source_from_document(&b).unwrap(),
        ];
        let c = &mut a.model.schematic.as_mut().unwrap().components[0];
        c.settings.source.clear();
        assert!(resolve(c, Path::new("Sheet.SchDoc"), &sources, None).is_err());
        c.settings.source = "A.SchLib".into();
        assert!(resolve(c, Path::new("Sheet.SchDoc"), &sources, None).is_ok());
        c.settings.source = "Missing.SchLib".into();
        assert!(resolve(c, Path::new("Sheet.SchDoc"), &sources, None).is_err());
    }
    #[test]
    fn closed_sheet_updates_open_an_undoable_tab_without_writing_the_file() {
        let (mut docs, mut d) = fixture();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Closed.json");
        eda_io::save(&path, &docs[1].model).unwrap();
        let before = std::fs::read(&path).unwrap();
        docs.pop();
        d.sheets[1].input = Input {
            path: path.clone(),
            session: None,
            hash: fingerprint(&path).unwrap(),
        };
        assert_eq!(d.execute(&mut docs).unwrap(), 2);
        assert_eq!(docs.len(), 2);
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(docs[1].dirty());
        let doc = &mut docs[1];
        doc.history.undo(&mut doc.model);
        assert!(!doc.dirty());
    }
    #[test]
    fn stale_disk_snapshot_is_rejected_before_modifying_any_open_sheet() {
        let (mut docs, mut d) = fixture();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Closed.json");
        eda_io::save(&path, &docs[1].model).unwrap();
        docs.pop();
        d.sheets[1].input = Input {
            path: path.clone(),
            session: None,
            hash: fingerprint(&path).unwrap(),
        };
        std::fs::write(&path, b"changed externally").unwrap();
        assert!(d.execute(&mut docs).is_err());
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].history.revision, 0);
    }
}
