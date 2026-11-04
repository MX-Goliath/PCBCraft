//! File-based library editor, based on the observed AD 24.4.1 library workspace.
use super::*;
use eda_core::{library_edit as edit, schematic as sch, schematic_edit};
use eda_geom::Point;
use egui::vec2;

#[derive(Clone, Copy)]
pub(super) enum Action {
    FromDocument,
    Panel,
    New,
    Add,
    Duplicate,
    Rename,
    Remove,
    Part,
    Mode,
    Pin,
    Link,
    EditModel,
    RemoveModel,
    Validate,
    Compile,
    Install,
    Place,
}
#[derive(Default)]
pub(super) struct State {
    new: bool,
    pcb: bool,
    query: String,
    prompt: Option<(Uuid, u64, Action, String)>,
    link: Option<Link>,
    details: Option<Details>,
    footprint_details: Option<FootprintDetails>,
    pins_tab: bool,
    show_projects: bool,
    selected_model: Option<usize>,
    pcb_panel_flags: Option<[bool; 3]>,
    applied_mask: String,
}
struct Link {
    session: Uuid,
    revision: u64,
    name: String,
    library: String,
    mapping: String,
    error: String,
}
struct Details {
    session: Uuid,
    revision: u64,
    name: String,
    designator: String,
    comment: String,
    description: String,
    parameters: Vec<(String, String)>,
}
struct FootprintDetails {
    session: Uuid,
    revision: u64,
    name: String,
    description: String,
    height_nm: i64,
    parameters: Vec<(String, String)>,
}
impl State {
    pub(super) fn modal(&self) -> bool {
        self.new || self.prompt.is_some() || self.link.is_some()
    }
}

impl PcbCraft {
    fn make_document_library(&mut self) {
        let Some(index) = self.active else {
            return;
        };
        let doc = &self.documents[index].model;
        if doc.library.is_some() {
            return;
        }
        let mut entries = vec![];
        let mut names = std::collections::BTreeSet::new();
        let result = (|| -> Result<Document, String> {
            if let Some(s) = &doc.schematic {
                for c in &s.components {
                    let base = if c.library_reference.is_empty() {
                        &c.designator
                    } else {
                        &c.library_reference
                    };
                    let name = if names.insert(base.to_lowercase()) {
                        base.clone()
                    } else {
                        (1..)
                            .map(|n| format!("{base}_{n}"))
                            .find(|name| names.insert(name.to_lowercase()))
                            .unwrap()
                    };
                    entries.push(edit::symbol_member(s, c.id, &name)?);
                }
            } else if let Some(b) = &doc.pcb {
                for c in &b.components {
                    let base = if c.footprint.is_empty() {
                        &c.designator
                    } else {
                        &c.footprint
                    };
                    let name = if names.insert(base.to_lowercase()) {
                        base.clone()
                    } else {
                        (1..)
                            .map(|n| format!("{base}_{n}"))
                            .find(|name| names.insert(name.to_lowercase()))
                            .unwrap()
                    };
                    let footprint = eda_core::design_sync::Footprint::from_board(b, c.id)?;
                    let mut member = edit::member(DocumentKind::Board, &name)?;
                    member.pcb = Some(footprint.board().clone());
                    member.pcb.as_mut().unwrap().components[0].footprint = name;
                    entries.push(member);
                }
            }
            edit::assemble(
                entries,
                if doc.kind == DocumentKind::Schematic {
                    "Extracted.SchLib"
                } else {
                    "Extracted.PcbLib"
                },
                None,
            )
        })();
        match result {
            Ok(doc) => {
                self.add_document(doc, None);
                self.settings.projects = true;
                self.library_editor.show_projects = false;
            }
            Err(e) => self.message(true, e),
        }
    }
    pub(super) fn library_active(&self) -> bool {
        self.active
            .is_some_and(|i| self.documents[i].model.library.is_some())
    }
    fn edit_library(&mut self, command: edit::Edit) {
        if !matches!(
            command,
            edit::Edit::Metadata { .. } | edit::Edit::FootprintMetadata { .. }
        ) && let Err(e) = self.commit_library_properties()
        {
            self.message(true, e);
            return;
        }
        let Some(index) = self.active else {
            return;
        };
        let d = &mut self.documents[index];
        let revision = d.history.revision;
        let result = if let edit::Edit::Select(index) = command {
            d.history.select_library_member(&mut d.model, index)
        } else {
            d.history.library_edit(&mut d.model, revision, &command)
        };
        match result {
            Ok(true) => {
                d.sch_editor.cancel(&mut d.view);
                d.pcb_editor = Default::default();
                d.view.selected = None;
                d.view.sch_selection.clear();
                d.view.sch_segments.clear();
                d.view.pcb.selected.clear();
                if matches!(
                    command,
                    edit::Edit::Select(_)
                        | edit::Edit::Add(_)
                        | edit::Edit::Duplicate(_)
                        | edit::Edit::Remove
                        | edit::Edit::AddPart
                        | edit::Edit::AddDisplayMode
                ) {
                    d.view.camera.fitted = false;
                }
                self.library_editor.details = None;
                self.library_editor.footprint_details = None;
            }
            Err(e) => self.message(true, e),
            _ => {}
        }
    }
    pub(super) fn library_command(&mut self, action: Action) {
        if let Err(e) = self.commit_library_properties() {
            self.message(true, e);
            return;
        }
        if matches!(action, Action::FromDocument) {
            self.make_document_library();
            return;
        }
        if matches!(action, Action::Panel) {
            self.library_editor.show_projects = false;
            self.settings.projects = true;
            return;
        }
        if matches!(action, Action::New) {
            self.library_editor.new = true;
            self.library_editor.show_projects = false;
            return;
        }
        let Some(index) = self
            .active
            .filter(|i| self.documents[*i].model.library.is_some())
        else {
            return;
        };
        let doc = &self.documents[index];
        match action {
            Action::New | Action::FromDocument | Action::Panel => {}
            Action::Add | Action::Duplicate | Action::Rename => {
                let old = edit::name(&doc.model);
                let value = if matches!(action, Action::Rename) {
                    old.into()
                } else {
                    let prefix = if matches!(action, Action::Duplicate) {
                        format!("{old}_")
                    } else if doc.model.kind == DocumentKind::Board {
                        "PCBCOMPONENT_".into()
                    } else {
                        "Component_".into()
                    };
                    (1..)
                        .map(|n| format!("{prefix}{n}"))
                        .find(|s| {
                            !edit::members(&doc.model)
                                .any(|m| edit::name(m).eq_ignore_ascii_case(s))
                        })
                        .unwrap()
                };
                self.library_editor.prompt =
                    Some((doc.session_id, doc.history.revision, action, value));
            }
            Action::Remove => self.edit_library(edit::Edit::Remove),
            Action::Part => self.edit_library(edit::Edit::AddPart),
            Action::Mode => self.edit_library(edit::Edit::AddDisplayMode),
            Action::Pin => {
                let Some(s) = doc.model.schematic.as_ref() else {
                    return;
                };
                let number = (1..).map(|n| n.to_string()).find(|n| !s.primitives.iter().any(|p| matches!(&p.kind, sch::SchPrimitiveKind::Pin { number, .. } if number == n))).unwrap();
                let mut pin = schematic_edit::primitive(
                    sch::SchPrimitiveKind::Pin {
                        position: Point::default(),
                        length_nm: 7_620_000,
                        orientation: 0,
                        name: "Pin".into(),
                        number,
                        show_name: true,
                        show_number: true,
                        font: s.sheet.system_font,
                        number_font: None,
                        electrical: 4,
                        outer_symbol: 0,
                        inner_symbol: 0,
                    },
                    Some(s.components[0].id),
                );
                pin.width_nm = 101_600;
                pin.color = 0;
                self.start_schematic_tool(Placement::Text);
                self.documents[index].sch_editor.directive = Some(pin);
            }
            Action::Link => {
                self.library_editor.link = Some(Link {
                    session: doc.session_id,
                    revision: doc.history.revision,
                    name: String::new(),
                    library: String::new(),
                    mapping: String::new(),
                    error: String::new(),
                });
            }
            Action::EditModel => {
                let selected = self.library_editor.selected_model;
                let model = doc
                    .model
                    .schematic
                    .as_ref()
                    .and_then(|s| selected.and_then(|n| s.components[0].settings.models.get(n)))
                    .cloned();
                if let Some(model) = model {
                    if !model.kind.eq_ignore_ascii_case("PCBLIB") {
                        self.message(
                            true,
                            "This model type is retained but has no verified editor yet",
                        );
                        return;
                    }
                    self.library_editor.link = Some(Link {
                        session: doc.session_id,
                        revision: doc.history.revision,
                        name: model.name,
                        library: model.parameters.get("LIBRARY").cloned().unwrap_or_default(),
                        mapping: model
                            .parameters
                            .iter()
                            .filter_map(|(k, v)| {
                                k.strip_prefix("PINMAP:").map(|pin| format!("{pin}={v}"))
                            })
                            .collect::<Vec<_>>()
                            .join("\n"),
                        error: String::new(),
                    });
                }
            }
            Action::RemoveModel => {
                if let Some(s) = &doc.model.schematic
                    && let Some(n) = self.library_editor.selected_model
                    && n < s.components[0].settings.models.len()
                {
                    let mut models = s.components[0].settings.models.clone();
                    models.remove(n);
                    self.edit_library(edit::Edit::Models(models));
                    self.library_editor.selected_model = None;
                }
            }
            Action::Validate => {
                let errors = edit::issues(&doc.model);
                if errors.is_empty() {
                    self.message(
                        false,
                        "Library validation passed: names, pins, variants and local geometry",
                    );
                } else {
                    for error in errors {
                        self.message(true, error);
                    }
                }
            }
            Action::Compile => {
                let Some(path) = self
                    .file_dialog
                    .clone()
                    .set_title("Compile Integrated Library")
                    .add_filter("Integrated Library", &["IntLib"])
                    .set_file_name("Library.IntLib")
                    .save_file()
                else {
                    return;
                };
                let footprints: Vec<_> = self
                    .documents
                    .iter()
                    .filter(|d| d.model.library.is_some() && d.model.kind == DocumentKind::Board)
                    .map(|d| &d.model)
                    .collect();
                match eda_io::library_documents::compile_integrated(
                    &path,
                    &self.documents[index].model,
                    &footprints,
                ) {
                    Ok(()) => {
                        self.message(false, format!("Compiled {}", path.display()));
                        self.import_library(path, true);
                    }
                    Err(e) => self.message(true, e),
                }
            }
            Action::Install => self.install_authored_library(index),
            Action::Place => {
                let symbol = self.authored_symbol(index);
                if let Some(target) = self.documents.iter().position(|d| {
                    d.model.library.is_none() && d.model.kind == DocumentKind::Schematic
                }) {
                    let (part, mode) = edit::variant(&self.documents[index].model);
                    self.active = Some(target);
                    if let Some(symbol) = symbol {
                        self.start_library_tool(symbol, part, mode);
                    }
                } else {
                    self.message(
                        true,
                        "Open a schematic document before placing a library component",
                    );
                }
            }
        }
    }
    fn authored_symbol(&self, index: usize) -> Option<eda_core::library::Symbol> {
        let doc = &self.documents[index].model;
        let s = doc.schematic.as_ref()?;
        let c = &s.components[0];
        let variants = edit::members(doc)
            .filter(|m| edit::name(m) == c.library_reference)
            .map(|m| {
                let s = m.schematic.as_ref().unwrap();
                Ok(eda_core::library::SymbolVariant {
                    part: s.components[0].part,
                    display_mode: s.components[0].display_mode,
                    fragment: schematic_edit::extract(s, s.components[0].id)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()
            .ok()?;
        Some(eda_core::library::Symbol {
            name: c.library_reference.clone(),
            description: c.description.clone(),
            parts: c.settings.part_count,
            display_modes: variants.iter().map(|v| v.display_mode).max().unwrap_or(0) + 1,
            variants,
        })
    }
    fn install_authored_library(&mut self, index: usize) {
        let model = self.documents[index].model.clone();
        let doc = &self.documents[index];
        let path = doc
            .path
            .clone()
            .unwrap_or_else(|| PathBuf::from(&model.name));
        if model.kind == DocumentKind::Schematic {
            let mut symbols = vec![];
            let mut names = std::collections::BTreeSet::new();
            for member in edit::members(&model) {
                let name = edit::name(member);
                if names.insert(name.to_owned()) {
                    let s = member.schematic.as_ref().unwrap();
                    let variants = edit::members(&model)
                        .filter(|m| edit::name(m) == name)
                        .map(|m| {
                            let s = m.schematic.as_ref().unwrap();
                            Ok(eda_core::library::SymbolVariant {
                                part: edit::variant(m).0,
                                display_mode: edit::variant(m).1,
                                fragment: schematic_edit::extract(s, s.components[0].id)?,
                            })
                        })
                        .collect::<Result<Vec<_>, String>>();
                    match variants {
                        Ok(variants) => symbols.push(eda_core::library::Symbol {
                            name: name.into(),
                            description: s.components[0].description.clone(),
                            parts: s.components[0].settings.part_count,
                            display_modes: variants
                                .iter()
                                .map(|v| v.display_mode)
                                .max()
                                .unwrap_or(0)
                                + 1,
                            variants,
                        }),
                        Err(e) => {
                            self.message(true, e);
                            return;
                        }
                    }
                }
            }
            self.components_ui.libraries.retain(|(p, _)| *p != path);
            self.components_ui.libraries.push((
                path,
                eda_core::library::Library {
                    name: model.name.clone(),
                    embedded_footprints: None,
                    symbols,
                    warnings: vec![],
                },
            ));
        } else {
            let footprints = edit::members(&model)
                .map(|m| {
                    eda_core::design_sync::Footprint::from_library_board(
                        m.pcb.as_ref().unwrap().clone(),
                    )
                })
                .collect::<Result<Vec<_>, _>>();
            match footprints {
                Ok(footprints) => {
                    self.components_ui
                        .footprint_libraries
                        .retain(|(p, _)| *p != path);
                    self.components_ui.footprint_libraries.push((
                        path,
                        eda_io::altium_pcb::library::Library {
                            footprints,
                            warnings: vec![],
                        },
                    ));
                }
                Err(e) => {
                    self.message(true, e);
                    return;
                }
            }
        }
        self.message(
            false,
            "Library definitions made available to Components and design synchronization",
        );
    }
    pub(super) fn library_panel(&mut self, ctx: &egui::Context) -> bool {
        let Some(index) = self
            .active
            .filter(|i| self.documents[*i].model.library.is_some())
        else {
            return false;
        };
        if self.library_editor.show_projects {
            return false;
        }
        let pcb = self.documents[index].model.kind == DocumentKind::Board;
        if pcb {
            self.pcb_library_panel(ctx, index);
            return true;
        }
        let mut command = None;
        let mut action = None;
        egui::SidePanel::left("library_panel")
            .default_width(334.)
            .width_range(190. ..=900.)
            .show(ctx, |ui| {
                ui.set_min_width(ui.available_width());
                egui::Frame::NONE
                    .fill(Color32::from_gray(75))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(if pcb { "PCB Library" } else { "SCH Library" })
                                .strong(),
                        );
                    });
                theme::search_field(ui, &mut self.library_editor.query, ui.available_width());
                egui::TopBottomPanel::bottom("library_panel_buttons")
                    .exact_height(62.)
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (label, a) in [
                                ("Place", Action::Place),
                                ("Add", Action::Add),
                                ("Delete", Action::Remove),
                                ("Edit", Action::Rename),
                            ] {
                                if ui
                                    .add_enabled(!pcb || label != "Place", egui::Button::new(label))
                                    .clicked()
                                {
                                    action = Some(a);
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.selectable_label(false, "Projects").clicked() {
                                self.library_editor.show_projects = true;
                            }
                            let _ = ui.selectable_label(
                                true,
                                if pcb { "PCB Library" } else { "SCH Library" },
                            );
                        });
                    });
                ui.horizontal(|ui| {
                    ui.label(if pcb { "Name" } else { "Design Item ID" });
                    ui.separator();
                    ui.label(if pcb { "Pads" } else { "Description" });
                });
                ui.separator();
                let q = self.library_editor.query.to_lowercase();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (n, member) in edit::members(&self.documents[index].model).enumerate() {
                        let name = edit::name(member);
                        if !name.to_lowercase().contains(&q) {
                            continue;
                        }
                        let (part, mode) = edit::variant(member);
                        let label = if member.kind == DocumentKind::Schematic
                            && member.schematic.as_ref().unwrap().components[0]
                                .settings
                                .part_count
                                > 1
                            || mode > 0
                        {
                            format!("{name}  Part {part}  Mode {}", mode + 1)
                        } else {
                            name.into()
                        };
                        let response = ui.selectable_label(n == 0, label);
                        if response.clicked() && n != 0 {
                            command = Some(edit::Edit::Select(n));
                        }
                        response.context_menu(|ui| {
                            for (label, a) in [
                                ("New Component", Action::Add),
                                ("Copy Component", Action::Duplicate),
                                ("Rename", Action::Rename),
                                ("Remove", Action::Remove),
                            ] {
                                if ui.button(label).clicked() {
                                    if n != 0 {
                                        command = Some(edit::Edit::Select(n));
                                    }
                                    action = Some(a);
                                    ui.close();
                                }
                            }
                        });
                    }
                });
            });
        if let Some(c) = command {
            self.edit_library(c);
        }
        if let Some(a) = action {
            self.library_command(a);
        }
        true
    }
    fn pcb_library_panel(&mut self, ctx: &egui::Context, index: usize) {
        let mut member = None;
        let mut action = None;
        let mut primitive = None;
        let mut magnify = false;
        let flags = self.library_editor.pcb_panel_flags.get_or_insert([true; 3]);
        egui::SidePanel::left("library_panel")
            .default_width(334.)
            .width_range(190. ..=900.)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(Color32::from_gray(75))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.strong("PCB Library");
                    });
                ui.horizontal(|ui| {
                    ui.label("Mask");
                    ui.text_edit_singleline(&mut self.library_editor.query);
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_sized([145., 24.], egui::Button::new("Apply"))
                        .clicked()
                    {
                        self.library_editor.applied_mask = self.library_editor.query.clone();
                    }
                    if ui.button("Clear").clicked() {
                        self.library_editor.query.clear();
                        self.library_editor.applied_mask.clear();
                    }
                    magnify = ui.button("Magnify").clicked();
                });
                ui.add_enabled(
                    false,
                    egui::Button::new("Normal     ▾").min_size(vec2(ui.available_width(), 22.)),
                );
                ui.horizontal(|ui| {
                    ui.checkbox(&mut flags[0], "Select");
                    ui.checkbox(&mut flags[1], "Zoom");
                    ui.checkbox(&mut flags[2], "Clear Existing");
                });
                ui.label("Footprints");
                egui::Grid::new("footprint_members_header")
                    .min_col_width(60.)
                    .show(ui, |ui| {
                        ui.label("Name");
                        ui.label("Pads");
                        ui.label("Primitives");
                        ui.end_row();
                    });
                egui::ScrollArea::vertical()
                    .id_salt("footprints")
                    .auto_shrink([false, false])
                    .max_height(178.)
                    .min_scrolled_height(178.)
                    .show(ui, |ui| {
                        egui::Grid::new("footprint_members")
                            .min_col_width(60.)
                            .show(ui, |ui| {
                                for (n, m) in
                                    edit::members(&self.documents[index].model).enumerate()
                                {
                                    if !edit::name(m)
                                        .to_lowercase()
                                        .contains(&self.library_editor.applied_mask.to_lowercase())
                                    {
                                        continue;
                                    }
                                    let b = m.pcb.as_ref().unwrap();
                                    if ui.selectable_label(n == 0, edit::name(m)).clicked()
                                        && n != 0
                                    {
                                        member = Some(n);
                                    }
                                    ui.label(
                                        b.items
                                            .iter()
                                            .filter(|i| {
                                                matches!(i.shape, eda_core::pcb::Shape::Pad { .. })
                                            })
                                            .count()
                                            .to_string(),
                                    );
                                    ui.label(b.items.len().to_string());
                                    ui.end_row();
                                }
                            });
                    });
                ui.horizontal(|ui| {
                    ui.add_enabled(false, egui::Button::new("Place"));
                    for (label, a) in [
                        ("Add", Action::Add),
                        ("Delete", Action::Remove),
                        ("Edit", Action::Rename),
                    ] {
                        if ui.button(label).clicked() {
                            action = Some(a);
                        }
                    }
                });
                ui.separator();
                ui.strong("Footprint Primitives");
                egui::ScrollArea::both()
                    .id_salt("footprint_primitives")
                    .auto_shrink([false, false])
                    .max_height(210.)
                    .min_scrolled_height(210.)
                    .show(ui, |ui| {
                        egui::Grid::new("footprint_primitive_rows")
                            .striped(true)
                            .show(ui, |ui| {
                                for label in ["Type", "Name", "X Size", "Y Size", "Layer"] {
                                    ui.label(label);
                                }
                                ui.end_row();
                                let d = &self.documents[index];
                                let b = d.model.pcb.as_ref().unwrap();
                                for item in &b.items {
                                    let bounds = item.bounds();
                                    if ui
                                        .selectable_label(
                                            d.view.pcb.selected.contains(&item.id),
                                            item.kind(),
                                        )
                                        .clicked()
                                    {
                                        primitive = Some(item.id);
                                    }
                                    ui.label(
                                        if let eda_core::pcb::Shape::Pad { name, .. } = &item.shape
                                        {
                                            name.as_str()
                                        } else {
                                            ""
                                        },
                                    );
                                    ui.label(format!(
                                        "{:.3}",
                                        (bounds.max.x - bounds.min.x) as f64 / 1e6
                                    ));
                                    ui.label(format!(
                                        "{:.3}",
                                        (bounds.max.y - bounds.min.y) as f64 / 1e6
                                    ));
                                    ui.label(eda_core::pcb::layer_name(item.layer));
                                    ui.end_row();
                                }
                            });
                    });
                ui.separator();
                ui.strong("Other");
                let available = ui.available_size() - vec2(0., 30.);
                if available.x > 30. && available.y > 30. {
                    ui.push_id("footprint_preview", |ui| {
                        ui.allocate_ui(available, |ui| {
                            let d = &self.documents[index];
                            let bounds = d.model.pcb.as_ref().unwrap().bounds();
                            let mut view = eda_render::View::default();
                            if !bounds.empty {
                                view.camera.centre_mm = [
                                    (bounds.min.x as f64 + bounds.max.x as f64) / 2e6,
                                    (bounds.min.y as f64 + bounds.max.y as f64) / 2e6,
                                ];
                                view.camera.pixels_per_mm = ((available.x as f64 - 20.)
                                    / ((bounds.max.x - bounds.min.x).max(1) as f64 / 1e6))
                                    .min(
                                        (available.y as f64 - 20.)
                                            / ((bounds.max.y - bounds.min.y).max(1) as f64 / 1e6),
                                    )
                                    .clamp(0.01, 500.);
                                view.camera.fitted = true;
                            }
                            view.grid = false;
                            if d.model.pcb.as_ref().unwrap().items.is_empty() {
                                ui.painter().rect_filled(
                                    ui.available_rect_before_wrap(),
                                    0.,
                                    Color32::BLACK,
                                );
                            } else {
                                ui.add_enabled_ui(false, |ui| {
                                    let _ = eda_render::pcb::canvas(
                                        ui,
                                        d.model.pcb.as_ref().unwrap(),
                                        &mut view,
                                    );
                                });
                            }
                        });
                    });
                }
                ui.horizontal(|ui| {
                    if ui.selectable_label(false, "Projects").clicked() {
                        self.library_editor.show_projects = true;
                    }
                    let _ = ui.selectable_label(true, "PCB Library");
                });
            });
        let flags = *self.library_editor.pcb_panel_flags.as_ref().unwrap();
        if let Some(id) = primitive {
            let view = &mut self.documents[index].view.pcb;
            if flags[0] {
                if flags[2] {
                    view.selected.clear();
                }
                if !view.selected.contains(&id) {
                    view.selected.push(id);
                }
            }
            if flags[1] {
                view.requested_fit = Some(eda_render::pcb::FitTarget::Selection);
            }
        }
        if magnify {
            self.documents[index].view.pcb.requested_fit =
                Some(eda_render::pcb::FitTarget::Document);
        }
        if let Some(n) = member {
            self.edit_library(edit::Edit::Select(n));
        }
        if let Some(a) = action {
            self.library_command(a);
        }
    }
    pub(super) fn library_menus(&mut self, ctx: &egui::Context) {
        let pcb = self
            .active
            .is_some_and(|i| self.documents[i].model.kind == DocumentKind::Board);
        egui::TopBottomPanel::top("menu")
            .exact_height(29.)
            .frame(egui::Frame::NONE.fill(Color32::from_gray(58)))
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| self.file_menu(ui));
                    ui.menu_button("Edit", |ui| {
                        let i = self.active.unwrap();
                        self.action(
                            ui,
                            "Undo",
                            "Ctrl+Z",
                            Command::Undo,
                            self.documents[i].history.can_undo(),
                        );
                        self.action(
                            ui,
                            "Redo",
                            "Ctrl+Y",
                            Command::Redo,
                            self.documents[i].history.can_redo(),
                        );
                        ui.separator();
                        if pcb {
                            self.pcb_edit_menu(ui);
                        } else {
                            for (label, key, a) in [
                                ("Cut", "Ctrl+X", SchAction::Cut),
                                ("Copy", "Ctrl+C", SchAction::Copy),
                                ("Paste", "Ctrl+V", SchAction::Paste),
                                ("Delete", "Del", SchAction::Delete),
                                ("Move", "M, M", SchAction::Move),
                                ("Rotate 90°", "Space", SchAction::Rotate),
                            ] {
                                self.action(ui, label, key, Command::SchEdit(a), true);
                            }
                        }
                        ui.separator();
                        for (label, a) in [
                            ("New Component", Action::Add),
                            ("Copy Component", Action::Duplicate),
                            ("Rename Component", Action::Rename),
                            ("Remove Component", Action::Remove),
                        ] {
                            self.action(ui, label, "", Command::Library(a), true);
                        }
                    });
                    ui.menu_button("View", |ui| {
                        self.action(
                            ui,
                            if pcb { "PCB Library" } else { "SCH Library" },
                            "",
                            Command::Library(Action::Panel),
                            true,
                        );
                        self.action(ui, "Fit Document", "Ctrl+PgDn", Command::Fit, true);
                        self.action(ui, "Toggle Grid", "G", Command::ToggleGrid, true);
                        if pcb {
                            self.action(ui, "3D Layout Mode", "3", Command::Pcb3d(true), true);
                            self.action(ui, "2D Layout Mode", "2", Command::Pcb3d(false), true);
                        }
                    });
                    ui.menu_button("Project", |ui| self.project_menu(ui));
                    ui.menu_button("Place", |ui| {
                        if pcb {
                            self.pcb_menu(ui, "Place");
                        } else {
                            self.action(ui, "Pin", "P, P", Command::Library(Action::Pin), true);
                            for (label, kind) in [
                                ("Arc", Placement::Arc),
                                ("Full Circle", Placement::FullCircle),
                                ("Elliptical Arc", Placement::EllipticalArc),
                                ("Ellipse", Placement::Ellipse),
                                ("Line", Placement::Line),
                                ("Rectangle", Placement::Rectangle),
                                ("Round Rectangle", Placement::RoundRectangle),
                                ("Polygon", Placement::Polygon),
                                ("Bezier", Placement::Bezier),
                                ("Text String", Placement::Text),
                                ("Text Frame", Placement::TextFrame),
                            ] {
                                self.action(ui, label, "", Command::SchPlace(kind), true);
                            }
                        }
                    });
                    ui.menu_button("Tools", |ui| {
                        for (label, a, enabled) in [
                            ("New Part", Action::Part, !pcb),
                            ("New Display Mode", Action::Mode, !pcb),
                            ("Add Footprint…", Action::Link, !pcb),
                            ("Component Rule Check", Action::Validate, true),
                            ("Make Available", Action::Install, true),
                            ("Compile Integrated Library…", Action::Compile, !pcb),
                        ] {
                            self.action(ui, label, "", Command::Library(a), enabled);
                        }
                    });
                    ui.menu_button("Reports", |ui| {
                        self.action(
                            ui,
                            "Component Rule Check",
                            "",
                            Command::Library(Action::Validate),
                            true,
                        );
                    });
                    ui.menu_button("Window", |ui| {
                        for n in 0..self.documents.len() {
                            if ui
                                .selectable_label(
                                    self.active == Some(n),
                                    &self.documents[n].model.name,
                                )
                                .clicked()
                            {
                                self.active = Some(n);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        self.action(ui, "About PCBCraft", "", Command::About, true)
                    });
                });
            });
    }
    pub(super) fn library_active_bar(&mut self, ctx: &egui::Context, canvas: egui::Rect) -> bool {
        if !self.library_active()
            || self.documents[self.active.unwrap()].model.kind != DocumentKind::Schematic
        {
            return false;
        }
        let mut action = None;
        egui::Area::new(egui::Id::new("library_active_bar"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(canvas.center().x - 154., canvas.top() + 3.))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(Color32::from_gray(58))
                    .corner_radius(5)
                    .inner_margin(4)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            use super::schematic_active_bar::icons::{self, Glyph};
                            for (glyph, tip, command) in [
                                (Glyph::Filter, "Selection Filter", None),
                                (Glyph::Move, "Move", Some(Command::SchEdit(SchAction::Move))),
                                (
                                    Glyph::Select,
                                    "Select",
                                    Some(Command::SchEdit(SchAction::Select)),
                                ),
                                (Glyph::Align, "Alignment", None),
                                (Glyph::Pin, "Pin", Some(Command::Library(Action::Pin))),
                                (Glyph::Directive, "IEEE Symbols", None),
                                (
                                    Glyph::Line,
                                    "Line",
                                    Some(Command::SchPlace(Placement::Line)),
                                ),
                                (
                                    Glyph::Text,
                                    "Text String",
                                    Some(Command::SchPlace(Placement::Text)),
                                ),
                                (
                                    Glyph::Part,
                                    "New Component",
                                    Some(Command::Library(Action::Add)),
                                ),
                            ] {
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(32., 26.),
                                    if command.is_some() {
                                        egui::Sense::click()
                                    } else {
                                        egui::Sense::hover()
                                    },
                                );
                                if response.hovered() && command.is_some() {
                                    ui.painter().rect_filled(rect, 0., theme::ACCENT);
                                }
                                icons::paint(
                                    ui.painter(),
                                    rect.shrink(2.),
                                    glyph,
                                    command.is_some(),
                                );
                                if response.on_hover_text(tip).clicked() {
                                    action = command;
                                }
                            }
                        });
                    });
            });
        if let Some(a) = action {
            self.command = Some(a);
        }
        true
    }
    pub(super) fn library_model_table(&mut self, ui: &mut egui::Ui, index: usize) {
        if self.documents[index].model.library.is_none()
            || self.documents[index].model.schematic.is_none()
        {
            return;
        }
        let mut action = None;
        egui::TopBottomPanel::bottom("library_models")
            .resizable(true)
            .default_height(320.)
            .height_range(80. ..=400.)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    for (label, w) in [
                        ("Model", 160.),
                        ("Type", 80.),
                        ("Location", 160.),
                        ("Description", 150.),
                    ] {
                        ui.add_sized([w, 20.], egui::Label::new(label));
                    }
                });
                ui.separator();
                let c = &self.documents[index]
                    .model
                    .schematic
                    .as_ref()
                    .unwrap()
                    .components[0];
                for (n, model) in c.settings.models.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .add_sized(
                                [160., 20.],
                                egui::Button::new(&model.name)
                                    .selected(self.library_editor.selected_model == Some(n)),
                            )
                            .clicked()
                        {
                            self.library_editor.selected_model = Some(n);
                        }
                        ui.add_sized([80., 20.], egui::Label::new(&model.kind));
                        ui.label(
                            model
                                .parameters
                                .get("LIBRARY")
                                .map_or("Any", String::as_str),
                        );
                    });
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Add Footprint").clicked() {
                            action = Some(Action::Link);
                        }
                        let selected = self
                            .library_editor
                            .selected_model
                            .is_some_and(|n| n < c.settings.models.len());
                        if ui
                            .add_enabled(selected, egui::Button::new("Remove"))
                            .clicked()
                        {
                            action = Some(Action::RemoveModel);
                        }
                        if ui
                            .add_enabled(
                                self.library_editor
                                    .selected_model
                                    .and_then(|n| c.settings.models.get(n))
                                    .is_some_and(|m| m.kind.eq_ignore_ascii_case("PCBLIB")),
                                egui::Button::new("Edit..."),
                            )
                            .clicked()
                        {
                            action = Some(Action::EditModel);
                        }
                    });
                });
            });
        if let Some(action) = action {
            self.library_command(action);
        }
    }
    pub(super) fn library_properties(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(index) = self
            .active
            .filter(|i| self.documents[*i].model.library.is_some())
        else {
            return false;
        };
        let d = &self.documents[index];
        if d.model.kind == DocumentKind::Board {
            return self.footprint_library_properties(ui, index);
        }
        if d.view.selected.is_some_and(|id| {
            d.model
                .schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .any(|p| p.id == id)
        }) {
            return false;
        }
        let c = &d.model.schematic.as_ref().unwrap().components[0];
        if d.sch_editor.active()
            && d.sch_editor
                .directive
                .as_ref()
                .is_some_and(|p| matches!(p.kind, sch::SchPrimitiveKind::Pin { .. }))
        {
            ui.label("Pin");
            ui.separator();
            let pin = self.documents[index].sch_editor.directive.as_mut().unwrap();
            if let sch::SchPrimitiveKind::Pin {
                name,
                number,
                length_nm,
                show_name,
                show_number,
                electrical,
                ..
            } = &mut pin.kind
            {
                ui.label("Designator");
                ui.text_edit_singleline(number);
                ui.label("Name");
                ui.text_edit_singleline(name);
                ui.label("Length (mil)");
                let mut length = *length_nm as f64 / 25_400.;
                if ui
                    .add(egui::DragValue::new(&mut length).range(1. ..=10000.))
                    .changed()
                {
                    *length_nm = (length * 25_400.) as i64;
                }
                ui.checkbox(show_name, "Show Name");
                ui.checkbox(show_number, "Show Designator");
                electrical_type(ui, electrical);
            }
            ui.label("Click to place · Space to rotate · Esc to finish");
            return true;
        }
        if self
            .library_editor
            .details
            .as_ref()
            .is_none_or(|v| v.session != d.session_id || v.revision != d.history.revision)
        {
            self.library_editor.details = Some(Details {
                session: d.session_id,
                revision: d.history.revision,
                name: c.library_reference.clone(),
                designator: c.designator.clone(),
                comment: c
                    .parameters
                    .get("Comment")
                    .cloned()
                    .unwrap_or_else(|| c.comment.clone()),
                description: c.description.clone(),
                parameters: c
                    .parameters
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            });
        }
        let (part, mode) = edit::variant(&d.model);
        let mut apply = false;
        let mut action = None;
        let draft = self.library_editor.details.as_mut().unwrap();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label("Component");
            ui.separator();
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.library_editor.pins_tab, false, "General");
                ui.selectable_value(&mut self.library_editor.pins_tab, true, "Pins");
            });
            if !self.library_editor.pins_tab {
                egui::CollapsingHeader::new("General")
                    .default_open(true)
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("Design Item ID", &mut draft.name),
                            ("Designator", &mut draft.designator),
                            ("Comment", &mut draft.comment),
                        ] {
                            ui.label(label);
                            apply |= ui.text_edit_singleline(value).lost_focus();
                        }
                        ui.label(format!(
                            "Part {part} of {}   Mode {}",
                            c.settings.part_count,
                            mode + 1
                        ));
                        ui.label("Description");
                        apply |= ui.text_edit_multiline(&mut draft.description).lost_focus();
                        apply |= ui.button("Apply").clicked();
                    });
                ui.collapsing("Parameters", |ui| {
                    let mut remove = None;
                    for (n, (key, value)) in draft.parameters.iter_mut().enumerate() {
                        ui.push_id(n, |ui| {
                            ui.horizontal(|ui| {
                                apply |= ui
                                    .add_sized([95., 22.], egui::TextEdit::singleline(key))
                                    .lost_focus();
                                apply |= ui
                                    .add_sized([95., 22.], egui::TextEdit::singleline(value))
                                    .lost_focus();
                                if ui.small_button("×").clicked() {
                                    remove = Some(n);
                                }
                            });
                        });
                    }
                    if let Some(n) = remove {
                        draft.parameters.remove(n);
                    }
                    if ui.button("Add Parameter").clicked() {
                        draft.parameters.push(("Parameter".into(), String::new()));
                    }
                    apply |= ui.button("Apply Parameters").clicked();
                });
            }
            if self.library_editor.pins_tab {
                egui::CollapsingHeader::new("Pins")
                    .default_open(true)
                    .show(ui, |ui| {
                        for p in &d.model.schematic.as_ref().unwrap().primitives {
                            if let sch::SchPrimitiveKind::Pin { number, name, .. } = &p.kind
                                && ui.button(format!("{number}   {name}")).clicked()
                            {
                                action = Some(p.id);
                            }
                        }
                    });
            }
            ui.horizontal(|ui| {
                if ui.button("Add Pin").clicked() {
                    self.command = Some(Command::Library(Action::Pin));
                }
                if ui.button("Add Footprint…").clicked() {
                    self.command = Some(Command::Library(Action::Link));
                }
            });
        });
        if let Some(id) = action {
            self.documents[index].view.selected = Some(id);
        }
        if apply {
            self.apply_library_details();
        }
        true
    }
    fn footprint_library_properties(&mut self, ui: &mut egui::Ui, index: usize) -> bool {
        let d = &self.documents[index];
        if !d.view.pcb.selected.is_empty() || d.pcb_editor.tool != super::pcb_editor::Tool::Select {
            return false;
        }
        let c = &d.model.pcb.as_ref().unwrap().components[0];
        if self
            .library_editor
            .footprint_details
            .as_ref()
            .is_none_or(|v| v.session != d.session_id || v.revision != d.history.revision)
        {
            self.library_editor.footprint_details = Some(FootprintDetails {
                session: d.session_id,
                revision: d.history.revision,
                name: c.footprint.clone(),
                description: c.properties.get("DESCRIPTION").cloned().unwrap_or_default(),
                height_nm: c
                    .properties
                    .get("HEIGHT")
                    .and_then(|v| eda_io::altium_pcb::dimension(v).ok())
                    .unwrap_or(0),
                parameters: c
                    .parameters
                    .as_ref()
                    .map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default(),
            });
        }
        let draft = self.library_editor.footprint_details.as_mut().unwrap();
        let mut apply = false;
        ui.label("Library Options");
        ui.separator();
        ui.horizontal(|ui| {
            let _ = ui.selectable_label(true, "General");
            ui.add_enabled(false, egui::Button::new("Footprint"));
        });
        egui::CollapsingHeader::new("Properties")
            .default_open(true)
            .show(ui, |ui| {
                ui.label("Name");
                apply |= ui.text_edit_singleline(&mut draft.name).lost_focus();
                ui.label("Description");
                apply |= ui
                    .add_sized(
                        [ui.available_width(), 70.],
                        egui::TextEdit::multiline(&mut draft.description),
                    )
                    .lost_focus();
                ui.horizontal(|ui| {
                    ui.label("Type");
                    ui.add_enabled(false, egui::Button::new("Standard"));
                });
                let mut height = draft.height_nm as f64 / 1e6;
                ui.horizontal(|ui| {
                    ui.label("Height");
                    let response = ui.add(
                        egui::DragValue::new(&mut height)
                            .range(0. ..=1000.)
                            .suffix(" mm"),
                    );
                    if response.changed() {
                        draft.height_nm = (height * 1e6).round() as i64;
                    }
                    apply |= response.lost_focus() || response.drag_stopped();
                });
                apply |= ui.button("Apply").clicked();
            });
        egui::CollapsingHeader::new("Parameters")
            .default_open(true)
            .show(ui, |ui| {
                let mut remove = None;
                for (n, (k, v)) in draft.parameters.iter_mut().enumerate() {
                    ui.push_id(n, |ui| {
                        ui.horizontal(|ui| {
                            apply |= ui
                                .add_sized([100., 22.], egui::TextEdit::singleline(k))
                                .lost_focus();
                            apply |= ui
                                .add_sized([100., 22.], egui::TextEdit::singleline(v))
                                .lost_focus();
                            if ui.small_button("×").clicked() {
                                remove = Some(n);
                            }
                        });
                    });
                }
                if let Some(n) = remove {
                    draft.parameters.remove(n);
                }
                if ui.button("Add").clicked() {
                    draft.parameters.push(("Parameter".into(), String::new()));
                }
                apply |= ui.button("Apply Parameters").clicked();
            });
        if apply {
            self.apply_library_details();
        }
        true
    }
    fn apply_library_details(&mut self) {
        if let Err(e) = self.commit_library_properties() {
            self.message(true, e);
        }
    }
    pub(super) fn commit_library_properties(&mut self) -> Result<(), String> {
        let Some(index) = self
            .active
            .filter(|i| self.documents[*i].model.library.is_some())
        else {
            return Ok(());
        };
        let d = &mut self.documents[index];
        let command = if let Some(v) = self
            .library_editor
            .details
            .as_ref()
            .filter(|v| v.session == d.session_id)
        {
            if v.revision != d.history.revision {
                self.library_editor.details = None;
                return Ok(());
            }
            let c = &d.model.schematic.as_ref().unwrap().components[0];
            let current: Vec<_> = c
                .parameters
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let unchanged = v.name == c.library_reference
                && v.designator == c.designator
                && v.comment == *c.parameters.get("Comment").unwrap_or(&c.comment)
                && v.description == c.description
                && v.parameters == current;
            (!unchanged).then(|| edit::Edit::Metadata {
                name: v.name.clone(),
                designator: v.designator.clone(),
                comment: v.comment.clone(),
                description: v.description.clone(),
                parameters: v.parameters.clone(),
            })
        } else if let Some(v) = self
            .library_editor
            .footprint_details
            .as_ref()
            .filter(|v| v.session == d.session_id)
        {
            if v.revision != d.history.revision {
                self.library_editor.footprint_details = None;
                return Ok(());
            }
            let c = &d.model.pcb.as_ref().unwrap().components[0];
            let current: Vec<_> = c
                .parameters
                .iter()
                .flat_map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())))
                .collect();
            let height = c
                .properties
                .get("HEIGHT")
                .and_then(|v| eda_io::altium_pcb::dimension(v).ok())
                .unwrap_or(0);
            let unchanged = v.name == c.footprint
                && v.description == c.properties.get("DESCRIPTION").cloned().unwrap_or_default()
                && v.height_nm == height
                && v.parameters == current;
            (!unchanged).then(|| edit::Edit::FootprintMetadata {
                name: v.name.clone(),
                description: v.description.clone(),
                height_nm: v.height_nm,
                parameters: v.parameters.clone(),
            })
        } else {
            None
        };
        if let Some(command) = command {
            d.history
                .library_edit(&mut d.model, d.history.revision, &command)?;
            self.library_editor.details = None;
            self.library_editor.footprint_details = None;
        }
        Ok(())
    }
    pub(super) fn library_editor_dialogs(&mut self, ctx: &egui::Context) {
        if self.library_editor.new {
            let mut open = true;
            let mut create = false;
            let response = egui::Modal::new(egui::Id::new("new_file_library"))
                .backdrop_color(Color32::TRANSPARENT)
                .frame(egui::Frame::window(&ctx.style()).corner_radius(0))
                .show(ctx, |ui| {
                    ui.set_min_size(vec2(684., 534.));
                    ui.set_max_size(vec2(684., 534.));
                    ui.horizontal(|ui| {
                        ui.label("New Library");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("×").clicked() {
                                open = false;
                            }
                        });
                    });
                    ui.separator();
                    let content = ui.available_rect_before_wrap();
                    let left = egui::Rect::from_min_size(content.min, vec2(180., content.height()));
                    let right = egui::Rect::from_min_max(content.min + vec2(196., 0.), content.max);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| {
                        ui.add_space(10.);
                        ui.label("LIBRARY TYPE");
                        ui.add_space(22.);
                        ui.add_enabled(
                            false,
                            egui::Button::new("Workspace").min_size(vec2(176., 32.)),
                        );
                        ui.add_space(8.);
                        ui.add_enabled(
                            false,
                            egui::Button::new("Database").min_size(vec2(176., 32.)),
                        );
                        ui.add_space(8.);
                        ui.add_sized([176., 32.], egui::Button::new("File").selected(true));
                    });
                    ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| {
                        ui.add_space(10.);
                        ui.heading("Simple File-based Library");
                        ui.label("Components stored in local files");
                        ui.add_space(40.);
                        ui.add_enabled(false, egui::RadioButton::new(false, "Integrated Library"));
                        ui.radio_value(&mut self.library_editor.pcb, false, "Schematic Library");
                        ui.radio_value(&mut self.library_editor.pcb, true, "PCB Library");
                        ui.add_enabled(false, egui::RadioButton::new(false, "Pad Via Library"));
                        ui.add_space(45.);
                        ui.label(if self.library_editor.pcb {
                            "PCB Library contains PCB footprint models."
                        } else {
                            "Schematic Library contains symbols, parameters and model references."
                        });
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                            create = ui
                                .add_sized([112., 28.], egui::Button::new("Create"))
                                .clicked();
                        });
                    });
                });
            if response.should_close() {
                open = false;
            }
            if create {
                let kind = if self.library_editor.pcb {
                    DocumentKind::Board
                } else {
                    DocumentKind::Schematic
                };
                let n = self.documents.len() + 1;
                let filename = format!(
                    "{}{}.{}",
                    if self.library_editor.pcb {
                        "PcbLib"
                    } else {
                        "SchLib"
                    },
                    n,
                    if self.library_editor.pcb {
                        "PcbLib"
                    } else {
                        "SchLib"
                    }
                );
                self.add_document(edit::new(kind, &filename), None);
                self.settings.projects = true;
                self.settings.properties = true;
                self.settings.right_components = false;
                self.settings.right_messages = false;
                open = false;
            }
            self.library_editor.new = open;
        }
        if let Some((session, revision, action, mut value)) = self.library_editor.prompt.take() {
            let mut open = true;
            let mut ok = false;
            egui::Window::new(match action {
                Action::Rename => "Rename Component",
                Action::Duplicate => "Copy Component",
                _ => "New Component",
            })
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label("Design Item ID / Name");
                let response = ui.text_edit_singleline(&mut value);
                response.request_focus();
                ok = ui.button("OK").clicked() || ui.input(|i| i.key_pressed(Key::Enter));
            });
            if ok {
                if self.active.is_some_and(|i| {
                    self.documents[i].session_id == session
                        && self.documents[i].history.revision == revision
                }) {
                    self.edit_library(match action {
                        Action::Rename => edit::Edit::Rename(value),
                        Action::Duplicate => edit::Edit::Duplicate(value),
                        _ => edit::Edit::Add(value),
                    });
                } else {
                    self.message(true, "Library changed; repeat the operation");
                }
            } else if open {
                self.library_editor.prompt = Some((session, revision, action, value));
            }
        }
        if let Some(mut link) = self.library_editor.link.take() {
            let mut open = true;
            let mut ok = false;
            let choices: Vec<_> = self
                .documents
                .iter()
                .filter(|d| d.model.library.is_some() && d.model.kind == DocumentKind::Board)
                .flat_map(|d| {
                    edit::members(&d.model).map(move |m| {
                        (
                            edit::name(m).to_owned(),
                            d.path
                                .as_ref()
                                .map_or_else(|| d.model.name.clone(), |p| p.display().to_string()),
                        )
                    })
                })
                .collect();
            egui::Window::new("PCB Model").open(&mut open).collapsible(false).default_width(460.).show(ctx,|ui| {
                ui.label("Name"); ui.text_edit_singleline(&mut link.name);
                egui::ComboBox::from_id_salt("open_footprints").selected_text("Browse open PCB libraries").show_ui(ui,|ui| {for(name,path)in &choices {if ui.selectable_label(false,name).clicked(){link.name=name.clone();link.library=path.clone();}}});
                ui.label("Library"); ui.text_edit_singleline(&mut link.library);
                ui.label("Pin Map"); ui.label("One mapping per line, e.g. 1=1 or EP=9,10. Empty uses matching designators."); ui.add(egui::TextEdit::multiline(&mut link.mapping).desired_rows(6));
                if !link.error.is_empty(){ui.colored_label(Color32::LIGHT_RED,&link.error);}
                ok=ui.button("OK").clicked();
            });
            if ok {
                let map = link
                    .mapping
                    .lines()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| {
                        let (pin, pads) = s
                            .split_once('=')
                            .ok_or("Use pin=pad[,pad] for each mapping")?;
                        Ok((
                            pin.trim().into(),
                            pads.split(',').map(|s| s.trim().into()).collect(),
                        ))
                    })
                    .collect::<Result<Vec<_>, String>>();
                if self.active.is_none_or(|i| {
                    self.documents[i].session_id != link.session
                        || self.documents[i].history.revision != link.revision
                }) {
                    link.error = "Library changed; reopen PCB Model".into();
                } else {
                    match map {
                        Ok(map) => {
                            let index = self.active.unwrap();
                            let command = edit::Edit::LinkFootprint {
                                name: link.name.clone(),
                                library: link.library.clone(),
                                map,
                            };
                            match edit::preview(&self.documents[index].model, &command) {
                                Ok(_) => {
                                    self.edit_library(command);
                                    open = false;
                                }
                                Err(e) => link.error = e,
                            }
                        }
                        Err(e) => link.error = e,
                    }
                }
            }
            if open {
                self.library_editor.link = Some(link);
            }
        }
    }
}
fn electrical_type(ui: &mut egui::Ui, value: &mut u32) {
    egui::ComboBox::from_id_salt("library_pin_electrical")
        .selected_text(
            *[
                "Input",
                "I/O",
                "Output",
                "Open Collector",
                "Passive",
                "HiZ",
                "Open Emitter",
                "Power",
            ]
            .get(*value as usize)
            .unwrap_or(&"Unknown"),
        )
        .show_ui(ui, |ui| {
            for (n, name) in [
                "Input",
                "I/O",
                "Output",
                "Open Collector",
                "Passive",
                "HiZ",
                "Open Emitter",
                "Power",
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(value, n as u32, *name);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_property_inspection_stays_clean_and_pending_values_commit_before_navigation() {
        let mut app = super::super::schematic_editor::tests::app();
        app.add_document(edit::new(DocumentKind::Schematic, "Parts.SchLib"), None);
        let i = app.active.unwrap();
        app.documents[i].mark_saved();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(app.library_properties(ui));
            });
        });
        app.commit_library_properties().unwrap();
        assert_eq!(app.documents[i].history.revision, 0);
        assert!(!app.documents[i].dirty());
        app.library_editor.details.as_mut().unwrap().description = "Edited without Apply".into();
        app.edit_library(edit::Edit::Add("Other".into()));
        let original = edit::members(&app.documents[i].model)
            .find(|m| edit::name(m) == "Component_1")
            .unwrap();
        assert_eq!(
            original.schematic.as_ref().unwrap().components[0].description,
            "Edited without Apply"
        );
        assert_eq!(app.documents[i].history.revision, 2);
    }
    #[test]
    fn repeated_library_pin_placement_undo_save_and_navigation_keep_all_members() {
        let mut app = super::super::schematic_editor::tests::app();
        app.add_document(edit::new(DocumentKind::Schematic, "Parts.SchLib"), None);
        let index = app.active.unwrap();
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(
                egui::RawInput {
                    focused: true,
                    events: vec![egui::Event::Key {
                        key: Key::P,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    assert!(app.schematic_shortcuts(ctx));
                },
            );
        }
        for y in [0, 3] {
            let _ = ctx.run(
                egui::RawInput {
                    focused: true,
                    ..Default::default()
                },
                |ctx| {
                    app.schematic_input(
                        index,
                        eda_render::SchematicInput {
                            point: Some(Point::mm(-5, y)),
                            clicked: true,
                            ..Default::default()
                        },
                        ctx,
                    );
                },
            );
        }
        let d = &mut app.documents[index];
        d.sch_editor.cancel(&mut d.view);
        let pins = |doc: &Document| {
            doc.schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .filter_map(|p| {
                    if let sch::SchPrimitiveKind::Pin { number, .. } = &p.kind {
                        Some(number.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(pins(&app.documents[index].model), ["1", "2"]);
        app.run(Command::Undo, &ctx);
        assert_eq!(pins(&app.documents[index].model), ["1"]);
        app.run(Command::Redo, &ctx);
        assert_eq!(pins(&app.documents[index].model), ["1", "2"]);
        app.edit_library(edit::Edit::Add("Other".into()));
        app.documents[index].mark_saved();
        app.edit_library(edit::Edit::Select(1));
        assert!(!app.documents[index].dirty());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Parts.SchLib");
        eda_io::save(&path, &app.documents[index].model).unwrap();
        let reopened = eda_io::load(&path).unwrap();
        assert_eq!(edit::members(&reopened).count(), 2);
        let original = edit::members(&reopened)
            .find(|d| edit::name(d) == "Component_1")
            .unwrap();
        assert_eq!(pins(original), ["1", "2"]);
    }
}
