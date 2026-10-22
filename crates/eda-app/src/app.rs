use crate::theme::{self, ACCENT, MUTED};
use eda_core::{Document, DocumentKind, History, Layer, Units};
use eda_render::{View, layer_color};
use egui::{Color32, Key, RichText};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;
#[path = "chrome.rs"]
mod chrome;
#[path = "components.rs"]
mod components;
#[path = "cross_probe.rs"]
mod cross_probe;
#[path = "pcb_3d.rs"]
mod pcb_3d;
#[path = "pcb_editor.rs"]
mod pcb_editor;
#[path = "properties.rs"]
mod properties;
#[path = "schematic_editor.rs"]
mod schematic_editor;
#[path = "schematic_properties.rs"]
mod schematic_properties;
use eda_core::schematic_edit::Placement;

#[derive(Clone, Copy)]
enum SchAction {
    Select,
    Move,
    Drag,
    Rotate,
    Delete,
    Copy,
    Cut,
    Paste,
    Duplicate,
}

struct OpenDocument {
    session_id: Uuid,
    model: Document,
    path: Option<PathBuf>,
    source_path: Option<PathBuf>,
    saved: Option<Document>,
    history: History,
    view: View,
    sch_editor: schematic_editor::Editor,
    pcb_editor: pcb_editor::Editor,
}

impl OpenDocument {
    fn new(model: Document, path: Option<PathBuf>) -> Self {
        let saved = path.as_ref().map(|_| model.clone());
        Self {
            session_id: Uuid::new_v4(),
            model,
            path,
            source_path: None,
            saved,
            history: History::default(),
            view: View::default(),
            sch_editor: Default::default(),
            pcb_editor: Default::default(),
        }
    }
    fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.model)
            || self.pcb_editor.has_pending()
            || self
                .model
                .pcb
                .as_ref()
                .is_some_and(|b| self.pcb_editor.properties_dirty(b, self.history.revision))
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    components: bool,
    right_components: bool,
    libraries: Vec<PathBuf>,
    projects: bool,
    properties: bool,
    messages: bool,
    units: Units,
    recent: Vec<PathBuf>,
    show_grid: bool,
    grid_nm: i64,
    right_messages: bool,
    pcb_navigation: eda_render::pcb::NavigationOptions,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            components: false,
            right_components: false,
            libraries: vec![],
            projects: true,
            properties: true,
            messages: false,
            units: Units::Millimetres,
            recent: vec![],
            show_grid: true,
            grid_nm: 1_000_000,
            right_messages: false,
            pcb_navigation: Default::default(),
        }
    }
}

#[derive(Clone, Copy)]
enum Command {
    CrossProbe,
    Pcb3d(bool),
    ShowComponents,
    ToggleComponents,
    SchPlace(Placement),
    SchEdit(SchAction),
    Pcb(pcb_editor::Action),
    NewBoard,
    NewSchematic,
    Open,
    OpenProject,
    Save,
    SaveAs,
    SaveAll,
    Close,
    Undo,
    Redo,
    Fit,
    Home,
    ToggleProjects,
    ToggleProperties,
    ToggleMessages,
    ToggleGrid,
    ToggleUnits,
    Preferences,
    About,
    Quit,
    Example,
}

#[derive(Clone, Copy)]
enum Pending {
    Close(Uuid),
    Quit,
}

struct PadEdit {
    id: Uuid,
    name: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    layer: Layer,
}

pub struct PcbCraft {
    cross_probe: cross_probe::State,
    file_dialog: rfd::FileDialog,
    documents: Vec<OpenDocument>,
    projects: Vec<eda_io::altium::Project>,
    active: Option<usize>,
    settings: Settings,
    search: String,
    log: Vec<(bool, String)>,
    command: Option<Command>,
    pending: Option<Pending>,
    preferences: bool,
    about: bool,
    allow_exit: bool,
    cursor_mm: Option<[f64; 2]>,
    pad_edit: Option<PadEdit>,
    property_ui: properties::PropertiesUi,
    sch_clipboard: Option<(eda_core::schematic_edit::Fragment, eda_geom::Point)>,
    components_ui: components::Panel,
    pcb_clipboard: Option<pcb_editor::Clipboard>,
}

impl PcbCraft {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, "pcbcraft.settings"))
            .unwrap_or_default();
        if ![100_000, 250_000, 500_000, 1_000_000, 2_500_000].contains(&settings.grid_nm) {
            settings.grid_nm = 1_000_000;
        }
        let mut app = Self {
            cross_probe: Default::default(),
            file_dialog: rfd::FileDialog::new().set_parent(cc),
            documents: vec![],
            projects: vec![],
            active: None,
            settings,
            search: String::new(),
            log: vec![(false, "PCBCraft 0.1.0 · Rust / wgpu · Ready".into())],
            command: None,
            pending: None,
            preferences: false,
            about: false,
            allow_exit: false,
            cursor_mm: None,
            pad_edit: None,
            property_ui: properties::PropertiesUi::default(),
            sch_clipboard: None,
            pcb_clipboard: None,
            components_ui: Default::default(),
        };
        for path in app.settings.libraries.clone() {
            app.import_library(path, false);
        }
        let mut opened = false;
        if let Some(path) = std::env::args_os().nth(1) {
            app.open_path(path.into());
            opened = !app.documents.is_empty() || !app.projects.is_empty();
        }
        if !opened {
            app.add_document(Document::example(), None);
        }
        app
    }

    fn message(&mut self, error: bool, text: impl Into<String>) {
        let text = text.into();
        if error {
            tracing::error!("{text}");
            self.settings.messages = true;
            self.settings.right_messages = true;
        } else {
            tracing::info!("{text}");
        }
        self.log.push((error, text));
        if self.log.len() > 250 {
            self.log.remove(0);
        }
    }

    fn add_document(&mut self, mut doc: Document, path: Option<PathBuf>) {
        eda_core::pcb::Board::ensure_document(&mut doc);
        let mut opened = OpenDocument::new(doc, path);
        opened.view.grid = self.settings.show_grid;
        opened.view.grid_nm = self.settings.grid_nm;
        if let Some(s) = &opened.model.schematic {
            opened.view.grid = s.sheet.grid_visible;
            opened.view.grid_nm = s.sheet.grid_nm;
            opened.view.snap.distance_nm = s.sheet.snap_nm;
        }
        self.documents.push(opened);
        self.active = Some(self.documents.len() - 1);
        self.pad_edit = None;
    }

    fn recent(&mut self, path: PathBuf) {
        self.settings.recent.retain(|p| *p != path);
        self.settings.recent.insert(0, path);
        self.settings.recent.truncate(10);
    }

    fn open_path(&mut self, path: PathBuf) {
        let path = path.canonicalize().unwrap_or(path);
        if let Some(index) = self
            .documents
            .iter()
            .position(|d| d.path.as_ref() == Some(&path) || d.source_path.as_ref() == Some(&path))
        {
            self.active = Some(index);
            return;
        }
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        if extension == "prjpcb" {
            self.open_project(path);
            return;
        }
        if matches!(extension.as_str(), "schlib" | "intlib") {
            self.import_library(path, true);
            return;
        }
        let native = matches!(extension.as_str(), "schdoc" | "pcbdoc");
        let loaded = if extension == "pcbdoc" {
            eda_io::altium_pcb::load_pcb(&path)
        } else if extension == "schdoc" {
            eda_io::altium::load_schematic(&path)
        } else {
            eda_io::load(&path)
        };
        match loaded {
            Ok(doc) => {
                self.message(false, format!("Opened {}", path.display()));
                self.recent(path.clone());
                let warnings = doc
                    .schematic
                    .as_ref()
                    .map(|s| s.warnings.clone())
                    .or_else(|| doc.pcb.as_ref().map(|b| b.warnings.clone()))
                    .unwrap_or_default();
                if native {
                    let native_path = (extension == "pcbdoc").then(|| path.clone());
                    self.add_document(doc, native_path);
                    let opened = self.documents.last_mut().unwrap();
                    opened.source_path = Some(path);
                    opened.saved = Some(opened.model.clone());
                } else {
                    self.add_document(doc, Some(path));
                }
                for warning in warnings {
                    self.message(false, format!("Import warning: {warning}"));
                }
            }
            Err(error) => self.message(true, format!("Could not open {}: {error}", path.display())),
        }
    }

    fn open_project(&mut self, path: PathBuf) {
        if let Some(project) = self.projects.iter().find(|p| p.path == path) {
            if let Some(first) = project
                .documents
                .iter()
                .find(|d| d.kind == "schdoc" && d.exists)
                .or_else(|| {
                    project
                        .documents
                        .iter()
                        .find(|d| d.kind == "pcbdoc" && d.exists)
                })
                .map(|d| d.path.clone())
            {
                self.open_path(first);
            }
            return;
        }
        match eda_io::altium::load_project(&path) {
            Ok(project) => {
                let first = project
                    .documents
                    .iter()
                    .find(|d| d.kind == "schdoc" && d.exists)
                    .or_else(|| {
                        project
                            .documents
                            .iter()
                            .find(|d| d.kind == "pcbdoc" && d.exists)
                    })
                    .map(|d| d.path.clone());
                for warning in &project.warnings {
                    self.message(false, warning.clone());
                }
                for entry in project
                    .documents
                    .iter()
                    .filter(|d| d.exists && matches!(d.kind.as_str(), "schlib" | "intlib"))
                {
                    self.import_library(entry.path.clone(), false);
                }
                self.message(
                    false,
                    format!(
                        "Opened project {}: {} document references",
                        project.name,
                        project.documents.len()
                    ),
                );
                self.projects.push(project);
                self.recent(path);
                if let Some(first) = first {
                    self.open_path(first);
                } else {
                    self.active = None;
                }
            }
            Err(e) => self.message(
                true,
                format!("Could not open project {}: {e}", path.display()),
            ),
        }
    }

    fn save_index(&mut self, index: usize, save_as: bool) -> bool {
        if let Some(doc) = self.documents.get_mut(index)
            && doc.model.pcb.is_some()
            && let Err(error) = pcb_editor::prepare_save(doc)
        {
            self.message(true, error);
            return false;
        }
        let Some(doc) = self.documents.get(index) else {
            return false;
        };
        let path = if save_as || doc.path.is_none() {
            let is_pcb = doc.model.kind == DocumentKind::Board;
            let mut dialog = self.file_dialog.clone();
            if is_pcb {
                dialog = dialog.add_filter("Altium PCB document", &["PcbDoc"]);
            }
            dialog = dialog.add_filter("PCBCraft document", &["json"]);
            let filename = if let Some(path) = &doc.path {
                path.file_name().map_or_else(
                    || doc.model.name.clone(),
                    |n| n.to_string_lossy().into_owned(),
                )
            } else if is_pcb {
                let name = &doc.model.name;
                if name.to_ascii_lowercase().ends_with(".pcbdoc") {
                    name.clone()
                } else {
                    format!(
                        "{}.PcbDoc",
                        name.trim_end_matches(".pcbcraft.json")
                            .trim_end_matches(".json")
                    )
                }
            } else if doc.model.name.to_ascii_lowercase().ends_with(".schdoc") {
                format!(
                    "{}.pcbcraft.json",
                    &doc.model.name[..doc.model.name.len() - 7]
                )
            } else {
                doc.model.name.clone()
            };
            dialog.set_file_name(filename).save_file()
        } else {
            doc.path.clone()
        };
        let Some(path) = path else {
            return false;
        };
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if self
            .documents
            .iter()
            .enumerate()
            .any(|(i, d)| i != index && d.path.as_ref() == Some(&canonical))
        {
            self.message(
                true,
                "That file is open in another tab. Choose another destination.",
            );
            return false;
        }
        match eda_io::save(&path, &self.documents[index].model) {
            Ok(()) => {
                self.documents[index].saved = Some(self.documents[index].model.clone());
                self.documents[index].path = Some(path.canonicalize().unwrap_or(path.clone()));
                self.recent(path.clone());
                self.message(false, format!("Saved {}", path.display()));
                true
            }
            Err(error) => {
                self.message(true, format!("Could not save {}: {error}", path.display()));
                false
            }
        }
    }

    fn remove_document(&mut self, id: Uuid) {
        self.property_ui.schematic.close_document(id);
        let active_id = self
            .active
            .and_then(|i| self.documents.get(i))
            .map(|d| d.session_id);
        self.documents.retain(|d| d.session_id != id);
        self.active = active_id
            .and_then(|id| self.documents.iter().position(|d| d.session_id == id))
            .or_else(|| (!self.documents.is_empty()).then_some(0));
        self.pad_edit = None;
    }

    fn request_close(&mut self, id: Uuid) {
        if self
            .documents
            .iter()
            .any(|d| d.session_id == id && d.dirty())
        {
            self.pending = Some(Pending::Close(id));
        } else {
            self.remove_document(id);
        }
    }

    fn run(&mut self, command: Command, ctx: &egui::Context) {
        if !matches!(command, Command::CrossProbe | Command::Fit) {
            self.cross_probe.enabled = false;
        }
        match command {
            Command::CrossProbe => self.start_cross_probe(),
            Command::Pcb3d(enabled) => self.set_pcb_3d(enabled),
            Command::SchPlace(kind) => self.start_schematic_tool(kind),
            Command::SchEdit(action) => self.schematic_action(action, ctx),
            Command::Pcb(action) => self.pcb_action(action),
            Command::NewBoard | Command::NewSchematic => {
                let board = matches!(command, Command::NewBoard);
                let n = self.documents.len() + 1;
                self.add_document(
                    Document::new(
                        if board {
                            DocumentKind::Board
                        } else {
                            DocumentKind::Schematic
                        },
                        format!(
                            "{}{}.pcbcraft.json",
                            if board { "Board" } else { "Schematic" },
                            n
                        ),
                    ),
                    None,
                );
            }
            Command::Example => self.add_document(Document::example(), None),
            Command::Open => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .add_filter(
                        "Design documents and libraries",
                        &["SchDoc", "PcbDoc", "PrjPcb", "SchLib", "IntLib", "json"],
                    )
                    .add_filter("Altium libraries", &["SchLib", "IntLib"])
                    .add_filter("Altium schematic", &["SchDoc"])
                    .add_filter("Altium PCB", &["PcbDoc"])
                    .add_filter("Altium project", &["PrjPcb"])
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Command::OpenProject => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .add_filter("Altium PCB project", &["PrjPcb"])
                    .pick_file()
                {
                    self.open_path(path);
                }
            }
            Command::Save | Command::SaveAs => {
                if let Some(i) = self.active {
                    self.save_index(i, matches!(command, Command::SaveAs));
                }
            }
            Command::SaveAll => {
                for i in 0..self.documents.len() {
                    if self.documents[i].dirty() && !self.save_index(i, false) {
                        break;
                    }
                }
            }
            Command::Close => {
                if let Some(i) = self.active {
                    self.request_close(self.documents[i].session_id);
                }
            }
            Command::Undo | Command::Redo => {
                if let Some(i) = self.active {
                    let doc = &mut self.documents[i];
                    doc.sch_editor.cancel(&mut doc.view);
                    doc.pcb_editor.cancel(&mut doc.view);
                    doc.view.pcb.selected.clear();
                    if !doc.view.sch_segments.is_empty() {
                        doc.view.selected = None;
                        doc.view.sch_segments.clear();
                    }
                    doc.view.sch_selection.clear();
                    if matches!(command, Command::Undo) {
                        doc.history.undo(&mut doc.model);
                    } else {
                        doc.history.redo(&mut doc.model);
                    }
                    self.pad_edit = None;
                }
            }
            Command::Fit => {
                if let Some(i) = self.active {
                    if self.documents[i].view.pcb.three_d.enabled {
                        self.documents[i].view.pcb.three_d.fit();
                    } else {
                        self.documents[i].view.camera.fitted = false;
                    }
                }
            }
            Command::Home => self.active = None,
            Command::ToggleProjects => self.settings.projects = !self.settings.projects,
            Command::ToggleProperties => {
                self.settings.properties = !self.settings.properties;
                self.settings.right_messages = false;
                self.settings.right_components = false;
            }
            Command::ToggleComponents => {
                if self.settings.right_components && self.settings.components {
                    self.settings.components = false;
                    self.settings.right_components = false;
                } else {
                    self.settings.components = true;
                    self.settings.right_components = true;
                }
            }
            Command::ShowComponents => {
                self.settings.components = true;
                self.settings.right_components = true;
            }
            Command::ToggleMessages => {
                self.settings.messages = !self.settings.messages;
                self.settings.right_messages = true;
                self.settings.right_components = false;
            }
            Command::ToggleGrid => {
                if let Some(index) = self.active {
                    let doc = &mut self.documents[index];
                    if let Some(s) = &doc.model.schematic {
                        let mut sheet = s.sheet.clone();
                        sheet.grid_visible = !sheet.grid_visible;
                        if let Err(error) = doc.edit_schematic(
                            eda_core::schematic_edit::Edit::SheetProperties {
                                sheet: Box::new(sheet),
                                size: doc.model.size,
                            },
                            doc.history.revision,
                        ) {
                            self.message(true, error);
                        }
                    } else {
                        doc.view.grid = !doc.view.grid;
                        self.settings.show_grid = doc.view.grid;
                    }
                }
            }
            Command::ToggleUnits => {
                self.settings.units = match self.settings.units {
                    Units::Millimetres => Units::Mils,
                    Units::Mils => Units::Millimetres,
                }
            }
            Command::Preferences => self.preferences = true,
            Command::About => self.about = true,
            Command::Quit => {
                if self.documents.iter().any(OpenDocument::dirty) {
                    self.pending = Some(Pending::Quit);
                } else {
                    self.allow_exit = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn action(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        shortcut: &str,
        command: Command,
        enabled: bool,
    ) {
        if ui
            .add_enabled(enabled, egui::Button::new(label).shortcut_text(shortcut))
            .clicked()
        {
            self.command = Some(command);
            ui.close();
        }
    }

    fn menus(&mut self, ctx: &egui::Context) {
        let has_doc = self.active.is_some();
        let undo = self
            .active
            .is_some_and(|i| self.documents[i].history.can_undo());
        let redo = self
            .active
            .is_some_and(|i| self.documents[i].history.can_redo());
        egui::TopBottomPanel::top("menu")
            .exact_height(29.0)
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        ui.menu_button("New", |ui| {
                            self.action(ui, "PCB Document", "Ctrl+N", Command::NewBoard, true);
                            self.action(
                                ui,
                                "Schematic Document",
                                "Ctrl+Shift+N",
                                Command::NewSchematic,
                                true,
                            );
                        });
                        self.action(ui, "Open…", "Ctrl+O", Command::Open, true);
                        self.action(ui, "Close", "Ctrl+F4", Command::Close, has_doc);
                        ui.separator();
                        self.action(ui, "Open Project…", "", Command::OpenProject, true);
                        ui.add_enabled(false, egui::Button::new("Open Project Group…"));
                        ui.separator();
                        self.action(ui, "Save", "Ctrl+S", Command::Save, has_doc);
                        self.action(ui, "Save As…", "Ctrl+Shift+S", Command::SaveAs, has_doc);
                        ui.add_enabled(false, egui::Button::new("Save Copy As…"));
                        self.action(
                            ui,
                            "Save All",
                            "",
                            Command::SaveAll,
                            !self.documents.is_empty(),
                        );
                        ui.separator();
                        for label in [
                            "Save Project As…",
                            "Save Project Group As…",
                            "Import",
                            "Export",
                            "Import Wizard",
                            "Run Script…",
                            "Fabrication Outputs",
                            "Assembly Outputs",
                            "Print…",
                            "Smart PDF…",
                        ] {
                            ui.add_enabled(false, egui::Button::new(label));
                        }
                        ui.separator();
                        let recent = self.settings.recent.clone();
                        ui.add_enabled_ui(!recent.is_empty(), |ui| {
                            ui.menu_button("Recent Documents", |ui| {
                                for path in recent {
                                    if ui.button(path.display().to_string()).clicked() {
                                        self.open_path(path);
                                        ui.close();
                                    }
                                }
                            });
                        });
                        ui.separator();
                        self.action(ui, "Exit", "Alt+F4", Command::Quit, true);
                    });
                    if has_doc {
                        ui.menu_button("Edit", |ui| {
                            self.action(ui, "Undo", "Ctrl+Z", Command::Undo, undo);
                            self.action(ui, "Redo", "Ctrl+Y", Command::Redo, redo);
                            let schematic = self.active.is_some_and(|i| {
                                self.documents[i].model.kind == DocumentKind::Schematic
                            });
                            if schematic {
                                let selected = self
                                    .active
                                    .is_some_and(|i| self.documents[i].view.selected.is_some());
                                ui.separator();
                                for (label, shortcut, action, enabled) in [
                                    ("Cut", "Ctrl+X", SchAction::Cut, selected),
                                    ("Copy", "Ctrl+C", SchAction::Copy, selected),
                                    (
                                        "Paste",
                                        "Ctrl+V",
                                        SchAction::Paste,
                                        self.sch_clipboard.is_some(),
                                    ),
                                    ("Duplicate", "Ctrl+D", SchAction::Duplicate, selected),
                                    ("Delete", "Del", SchAction::Delete, selected),
                                    ("Rotate 90°", "Space", SchAction::Rotate, selected),
                                ] {
                                    self.action(
                                        ui,
                                        label,
                                        shortcut,
                                        Command::SchEdit(action),
                                        enabled,
                                    );
                                }
                                ui.menu_button("Move", |ui| self.move_menu_entries(ui));
                            }
                            ui.separator();
                            if !schematic {
                                self.pcb_edit_menu(ui);
                            }
                            self.action(ui, "Preferences…", "", Command::Preferences, true);
                        });
                    }
                    ui.menu_button("View", |ui| {
                        self.action(ui, "Components", "", Command::ToggleComponents, true);
                        let pcb_active = self
                            .active
                            .is_some_and(|i| self.documents[i].model.pcb.is_some());
                        self.action(
                            ui,
                            "Fit Document",
                            if pcb_active { "Ctrl+PgDn" } else { "Home" },
                            Command::Fit,
                            has_doc,
                        );
                        if pcb_active {
                            self.action(ui, "2D Layout Mode", "2", Command::Pcb3d(false), true);
                            self.action(ui, "3D Layout Mode", "3", Command::Pcb3d(true), true);
                            ui.separator();
                            self.action(
                                ui,
                                "Fit Board",
                                "V, F",
                                Command::Pcb(pcb_editor::Action::Fit(
                                    eda_render::pcb::FitTarget::Board,
                                )),
                                true,
                            );
                            self.action(
                                ui,
                                "Selected Objects",
                                "Ctrl+Home",
                                Command::Pcb(pcb_editor::Action::Fit(
                                    eda_render::pcb::FitTarget::Selection,
                                )),
                                true,
                            );
                        }
                        self.action(ui, "Home Page", "", Command::Home, true);
                        ui.separator();
                        if ui
                            .checkbox(&mut self.settings.projects, "Projects")
                            .clicked()
                        {
                            ui.close();
                        }
                        if ui
                            .checkbox(&mut self.settings.properties, "Properties")
                            .clicked()
                        {
                            self.settings.right_components = false;
                            self.settings.right_messages = false;
                            ui.close();
                        }
                        if ui
                            .checkbox(&mut self.settings.messages, "Messages")
                            .clicked()
                        {
                            self.settings.right_components = false;
                            self.settings.right_messages = true;
                            ui.close();
                        }
                        ui.separator();
                        self.action(
                            ui,
                            "Toggle Grid",
                            if pcb_active { "" } else { "G" },
                            Command::ToggleGrid,
                            has_doc,
                        );
                        if pcb_active {
                            self.action(
                                ui,
                                "Snap Grid…",
                                "G",
                                Command::Pcb(pcb_editor::Action::Grid),
                                true,
                            );
                        }
                        self.action(ui, "Toggle Units", "Q", Command::ToggleUnits, true);
                        if let Some(index) = self.active {
                            if self.documents[index].model.pcb.is_some() {
                                self.action(
                                    ui,
                                    "Layers and Colors…",
                                    "L",
                                    Command::Pcb(pcb_editor::Action::Layers),
                                    true,
                                );
                            } else {
                                ui.menu_button("Layers", |ui| {
                                    for layer in Layer::ALL {
                                        let visible = &mut self.documents[index].view.visible
                                            [eda_render::layer_index(layer)];
                                        ui.checkbox(visible, layer.label());
                                    }
                                });
                            }
                        }
                    });
                    ui.menu_button("Project", |ui| {
                        ui.add_enabled(false, egui::Button::new("Project Options…"));
                    });
                    // The names mirror the observed editor. Unsupported tools are visibly disabled.
                    if has_doc {
                        let schematic = self.active.is_some_and(|i| {
                            self.documents[i].model.kind == DocumentKind::Schematic
                        });
                        for label in [
                            "Place",
                            "Design",
                            "Tools",
                            if schematic { "Simulate" } else { "Route" },
                            "Reports",
                        ] {
                            ui.menu_button(label, |ui| {
                                if label == "Tools" {
                                    self.action(
                                        ui,
                                        "Cross Probe",
                                        "T, C",
                                        Command::CrossProbe,
                                        true,
                                    );
                                    if schematic {
                                        return;
                                    }
                                    ui.separator();
                                }
                                if label == "Place" && schematic {
                                    self.schematic_place_menu(ui);
                                    return;
                                }
                                if !schematic && self.pcb_menu(ui, label) {
                                    return;
                                }
                                ui.add_enabled(
                                    false,
                                    egui::Label::new("Not available in this version"),
                                );
                            });
                        }
                    }
                    ui.menu_button("Window", |ui| {
                        if ui.button("Restore Default Layout").clicked() {
                            self.settings.projects = true;
                            self.settings.properties = true;
                            self.settings.messages = false;
                            self.settings.components = false;
                            self.settings.right_components = false;
                            ctx.memory_mut(|m| *m = egui::Memory::default());
                            theme::apply(ctx);
                            for doc in &mut self.documents {
                                doc.view.camera.fitted = false;
                            }
                            ui.close();
                        }
                        ui.separator();
                        for i in 0..self.documents.len() {
                            if ui
                                .selectable_label(
                                    self.active == Some(i),
                                    &self.documents[i].model.name,
                                )
                                .clicked()
                            {
                                self.active = Some(i);
                                ui.close();
                            }
                        }
                    });
                    ui.menu_button("Help", |ui| {
                        self.action(ui, "About PCBCraft", "", Command::About, true);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(RichText::new("Not Connected").small().color(MUTED));
                        for (icon, command) in [
                            (theme::Icon::Home, Command::Home),
                            (theme::Icon::Settings, Command::Preferences),
                        ] {
                            if theme::icon_button(ui, icon, false, true).clicked() {
                                self.command = Some(command);
                            }
                        }
                    });
                });
            });
    }

    fn projects(&mut self, ctx: &egui::Context) {
        let mut open_path = None;
        let schematic = self
            .active
            .is_some_and(|i| self.documents[i].model.kind == DocumentKind::Schematic);
        egui::SidePanel::left(if schematic {
            "projects_schematic_v3"
        } else {
            "projects_altium_v2"
        })
        .default_width(330.0)
        .width_range(190.0..=500.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Projects");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::icon_button(ui, theme::Icon::Close, false, true)
                        .on_hover_text("Hide Projects")
                        .clicked()
                    {
                        self.command = Some(Command::ToggleProjects);
                    }
                    theme::icon_button(ui, theme::Icon::Pin, false, false);
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 9.0;
                ui.add_space(7.0);
                for (icon, label, command) in [
                    (theme::Icon::Save, "Save document", Some(Command::Save)),
                    (theme::Icon::SaveAll, "Save All", Some(Command::SaveAll)),
                    (theme::Icon::Open, "Open document", Some(Command::Open)),
                    (
                        theme::Icon::Folder,
                        "Open project",
                        Some(Command::OpenProject),
                    ),
                    (
                        theme::Icon::Settings,
                        "Preferences",
                        Some(Command::Preferences),
                    ),
                ] {
                    if theme::icon_button(ui, icon, false, command.is_some())
                        .on_hover_text(label)
                        .clicked()
                    {
                        self.command = command;
                    }
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                theme::search_field(ui, &mut self.search, ui.available_width() - 8.0);
            });
            ui.add_space(13.0);
            egui::TopBottomPanel::bottom("project_panel_tabs")
                .exact_height(21.0)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_label(true, "Projects")
                            .on_hover_text("Projects panel");
                        ui.add_enabled(false, egui::Button::new("Navigator").frame(false));
                    });
                });
            egui::ScrollArea::vertical().show(ui, |ui| {
                egui::CollapsingHeader::new(RichText::new("Project Group 1").strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for project in &self.projects {
                            egui::CollapsingHeader::new(RichText::new(&project.name).strong())
                                .id_salt(&project.path)
                                .default_open(true)
                                .show(ui, |ui| {
                                    egui::CollapsingHeader::new("Source Documents")
                                        .id_salt((&project.path, "sources"))
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            for entry in &project.documents {
                                                let filename = entry
                                                    .path
                                                    .file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy();
                                                if !filename
                                                    .to_lowercase()
                                                    .contains(&self.search.to_lowercase())
                                                {
                                                    continue;
                                                }
                                                let selected = self.active.is_some_and(|i| {
                                                    self.documents[i].source_path.as_ref()
                                                        == Some(&entry.path)
                                                });
                                                let label = format!(
                                                    "{} {}{}",
                                                    if entry.kind == "schdoc" {
                                                        "SCH"
                                                    } else if entry.kind == "pcbdoc" {
                                                        "PCB"
                                                    } else {
                                                        "LIB"
                                                    },
                                                    filename,
                                                    if entry.exists { "" } else { " (missing)" }
                                                );
                                                let response = ui.add_enabled(
                                                    entry.exists
                                                        && matches!(
                                                            entry.kind.as_str(),
                                                            "schdoc"
                                                                | "pcbdoc"
                                                                | "schlib"
                                                                | "intlib"
                                                        ),
                                                    egui::Button::new(label)
                                                        .frame(selected)
                                                        .selected(selected),
                                                );
                                                if response.clicked() {
                                                    open_path = Some(entry.path.clone());
                                                }
                                                response.on_disabled_hover_text(if !entry.exists {
                                                    "Referenced file was not found"
                                                } else {
                                                    "This document type is not supported"
                                                });
                                            }
                                        });
                                });
                        }
                        let folder_header = |ui: &mut egui::Ui, label: &str| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                            theme::draw_icon(ui.painter(), rect, theme::Icon::Folder, true);
                            ui.label(label);
                        };
                        egui::collapsing_header::CollapsingState::load_with_default_open(
                            ui.ctx(),
                            ui.make_persistent_id("free_documents"),
                            true,
                        )
                        .show_header(ui, |ui| folder_header(ui, "Free Documents"))
                        .body(|ui| {
                            egui::collapsing_header::CollapsingState::load_with_default_open(
                                ui.ctx(),
                                ui.make_persistent_id("source_documents"),
                                true,
                            )
                            .show_header(ui, |ui| folder_header(ui, "Source Documents"))
                            .body(|ui| {
                                let query = self.search.to_lowercase();
                                for (index, doc) in self.documents.iter().enumerate() {
                                    if doc.source_path.as_ref().is_some_and(|path| {
                                        self.projects
                                            .iter()
                                            .any(|p| p.documents.iter().any(|d| &d.path == path))
                                    }) {
                                        continue;
                                    }
                                    if !doc.model.name.to_lowercase().contains(&query) {
                                        continue;
                                    }
                                    ui.horizontal(|ui| {
                                        theme::logo(ui);
                                        let title = format!(
                                            "{}{}",
                                            doc.model.name,
                                            if doc.dirty() { " *" } else { "" }
                                        );
                                        if ui
                                            .selectable_label(self.active == Some(index), title)
                                            .clicked()
                                        {
                                            self.active = Some(index);
                                            self.pad_edit = None;
                                        }
                                    });
                                }
                                if self.documents.is_empty() {
                                    ui.label(RichText::new("No documents open").color(MUTED));
                                }
                            });
                        });
                    });
            });
        });
        if let Some(path) = open_path {
            self.open_path(path);
        }
    }

    fn status(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(23.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let u = self.settings.units;
                    let coordinates = self
                        .cursor_mm
                        .map(|p| {
                            format!(
                                "X: {:>8.3}   Y: {:>8.3} {}",
                                u.from_nm(p[0] * 1e6),
                                u.from_nm(p[1] * 1e6),
                                u.label()
                            )
                        })
                        .unwrap_or_else(|| format!("X: —    Y: —  {}", u.label()));
                    ui.label(RichText::new(coordinates).size(11.0));
                    ui.separator();
                    if ui.small_button(format!("Units: {}", u.label())).clicked() {
                        self.command = Some(Command::ToggleUnits);
                    }
                    ui.label(
                        RichText::new(format!(
                            "Grid: {:.3} mm",
                            self.active
                                .map_or(self.settings.grid_nm, |i| self.documents[i].view.grid_nm)
                                as f64
                                / 1e6
                        ))
                        .small()
                        .color(MUTED),
                    );
                    if self.cross_probe.enabled {
                        ui.separator();
                        ui.label(RichText::new("Cross Probe · Esc exits").strong())
                            .on_hover_text(&self.cross_probe.status);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("Panels", |ui| {
                            self.action(ui, "Components", "", Command::ToggleComponents, true);
                            self.action(ui, "Projects", "", Command::ToggleProjects, true);
                            self.action(ui, "Properties", "", Command::ToggleProperties, true);
                            self.action(ui, "Messages", "", Command::ToggleMessages, true);
                        });
                    });
                });
            });
    }

    fn right_dock(&mut self, ctx: &egui::Context) {
        let schematic = self
            .active
            .is_some_and(|i| self.documents[i].model.kind == DocumentKind::Schematic);
        egui::SidePanel::right(if schematic {
            "right_dock_schematic_v3"
        } else {
            "right_dock_pcb_v4"
        })
        .default_width(316.0)
        .width_range(230.0..=500.0)
        .resizable(true)
        .show(ctx, |ui| {
            if let Some(position) = ctx.input(|i| i.pointer.press_origin()) {
                self.property_ui.focused = ui.max_rect().contains(position);
            }
            let components = self.settings.components
                && (self.settings.right_components
                    || (!self.settings.properties && !self.settings.messages));
            let messages = !components
                && self.settings.messages
                && (self.settings.right_messages || !self.settings.properties);
            egui::Frame::NONE
                .fill(if self.property_ui.focused {
                    Color32::from_rgb(57, 91, 112)
                } else {
                    Color32::from_gray(66)
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(if components {
                            "Components"
                        } else if messages {
                            "Messages"
                        } else {
                            "Properties"
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::icon_button(ui, theme::Icon::Close, false, true).clicked() {
                                self.command = Some(if components {
                                    Command::ToggleComponents
                                } else if messages {
                                    Command::ToggleMessages
                                } else {
                                    Command::ToggleProperties
                                });
                            }
                            theme::icon_button(ui, theme::Icon::Pin, false, false);
                        });
                    });
                });
            ui.add_space(4.0);
            egui::TopBottomPanel::bottom("right_dock_tabs")
                .exact_height(21.0)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(messages, "Messages").clicked() {
                            self.settings.messages = true;
                            self.settings.right_messages = true;
                            self.settings.right_components = false;
                        }
                        for name in if schematic {
                            ["Properties", "Components"]
                        } else {
                            ["Components", "Properties"]
                        } {
                            if name == "Properties" {
                                if ui
                                    .selectable_label(!messages && !components, name)
                                    .clicked()
                                {
                                    self.settings.properties = true;
                                    self.settings.right_messages = false;
                                    self.settings.right_components = false;
                                }
                            } else if self.settings.components
                                && ui.selectable_label(components, name).clicked()
                            {
                                self.settings.right_components = true;
                            }
                        }
                    });
                });
            if components {
                self.components_panel(ui);
            } else if messages {
                ui.horizontal(|ui| {
                    ui.label("Class");
                    ui.separator();
                    ui.label("Document");
                    ui.separator();
                    ui.label("Message");
                });
                ui.separator();
                egui::ScrollArea::both().show(ui, |ui| {
                    for (error, text) in &self.log {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if *error { "Error" } else { "Info" })
                                    .color(if *error { Color32::LIGHT_RED } else { ACCENT }),
                            );
                            ui.separator();
                            ui.label(text);
                        });
                    }
                });
            } else {
                self.properties(ui);
            }
        });
    }

    fn workspace(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_rgb(109, 106, 105))
                    .inner_margin(egui::Margin::symmetric(5, 0)),
            )
            .show(ctx, |ui| {
                let mut close = None;
                egui::Frame::NONE
                    .fill(Color32::from_rgb(54, 54, 54))
                    .inner_margin(egui::Margin::symmetric(3, 1))
                    .show(ui, |ui| {
                        egui::ScrollArea::horizontal()
                            .id_salt("document_tabs")
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.selectable_label(self.active.is_none(), "Home").clicked()
                                    {
                                        self.command = Some(Command::Home);
                                    }
                                    ui.separator();
                                    for (index, doc) in self.documents.iter().enumerate() {
                                        let title = format!(
                                            "{}  {}{}",
                                            if doc.model.kind == DocumentKind::Board {
                                                "PCB"
                                            } else {
                                                "SCH"
                                            },
                                            doc.model.name,
                                            if doc.dirty() { " *" } else { "" }
                                        );
                                        if ui
                                            .selectable_label(self.active == Some(index), title)
                                            .clicked()
                                        {
                                            self.active = Some(index);
                                            self.pad_edit = None;
                                        }
                                        if ui
                                            .small_button("×")
                                            .on_hover_text("Close document")
                                            .clicked()
                                        {
                                            close = Some(doc.session_id);
                                        }
                                        ui.separator();
                                    }
                                });
                            });
                    });
                if let Some(id) = close {
                    self.request_close(id);
                }
                if let Some(index) = self.active {
                    if self.documents[index].model.kind == DocumentKind::Board {
                        self.pcb_layer_bar(ui, index);
                    }
                    if self.documents[index].model.kind == DocumentKind::Schematic {
                        egui::TopBottomPanel::bottom("schematic_editor")
                            .exact_height(24.0)
                            .frame(egui::Frame::NONE.fill(Color32::from_gray(54)))
                            .show_inside(ui, |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.selectable_label(true, "Editor")
                                        .on_hover_text("Schematic editor");
                                    ui.label(
                                        self.documents[index]
                                            .model
                                            .name
                                            .trim_end_matches(".SchDoc"),
                                    );
                                });
                            });
                    }
                    let canvas = ui.available_rect_before_wrap();
                    if self.documents[index].model.kind == DocumentKind::Schematic {
                        ui.painter()
                            .rect_filled(canvas, 0.0, Color32::from_gray(195));
                        ui.add_space(26.0);
                    }
                    let doc = &mut self.documents[index];
                    doc.view.source_display_path = doc
                        .source_path
                        .as_ref()
                        .or(doc.path.as_ref())
                        .map_or_else(|| doc.model.name.clone(), |p| p.display().to_string());
                    doc.view.display_date = chrono::Local::now().format("%m.%d.%Y").to_string();
                    doc.view.pcb.navigation = self.settings.pcb_navigation;
                    doc.view.pcb.three_d.revision = doc.history.revision;
                    doc.view.cross_probe = self.cross_probe.enabled;
                    let output = eda_render::canvas(ui, &doc.model, &mut doc.view);
                    let pcb_input = doc.view.pcb.input.take();
                    self.cursor_mm = output.cursor_mm;
                    if let Some(input) = output.schematic_input
                        && !self.cross_probe_input(
                            index,
                            input.hit,
                            input.clicked,
                            input.secondary_clicked,
                            ctx,
                        )
                    {
                        self.schematic_input(index, input, ctx);
                    }
                    if let Some(input) = pcb_input
                        && !self.cross_probe_input(
                            index,
                            input.hit,
                            input.clicked,
                            input.secondary_clicked,
                            ctx,
                        )
                    {
                        self.pcb_input(index, input, ctx);
                    }
                    if self.documents[index].model.kind == DocumentKind::Schematic {
                        self.active_bar(ctx, canvas);
                    }
                } else {
                    self.home(ui);
                    self.cursor_mm = None;
                }
            });
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(65.0);
            ui.horizontal(|ui| {
                ui.add_space(45.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("PCBCraft").size(38.0).strong());
                    ui.label(
                        RichText::new("Electronic design workspace")
                            .size(18.0)
                            .color(MUTED),
                    );
                    ui.add_space(30.0);
                    ui.label(RichText::new("Start designing").size(17.0).strong());
                    ui.add_space(12.0);
                    for (label, command) in [
                        ("+  New PCB Document", Command::NewBoard),
                        ("+  New Schematic Document", Command::NewSchematic),
                        ("Open Document…", Command::Open),
                        ("Open Example Board", Command::Example),
                    ] {
                        if ui
                            .add_sized([260.0, 38.0], egui::Button::new(label))
                            .clicked()
                        {
                            self.command = Some(command);
                        }
                    }
                    ui.add_space(30.0);
                    ui.label(RichText::new("Recent documents").strong());
                    let recent = self.settings.recent.clone();
                    if recent.is_empty() {
                        ui.label(RichText::new("Saved documents will appear here.").color(MUTED));
                    }
                    for path in recent {
                        if ui.link(path.display().to_string()).clicked() {
                            self.open_path(path);
                        }
                    }
                    ui.add_space(35.0);
                    ui.label(
                        RichText::new("0.1.0  /  Main window and document foundation")
                            .small()
                            .color(MUTED),
                    );
                });
            });
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.preferences {
            let old_grid = (self.settings.show_grid, self.settings.grid_nm);
            let mut open = true;
            egui::Window::new("Preferences")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(380.0)
                .show(ctx, |ui| {
                    theme::heading(ui, "PCB Editor / General", "Application preferences");
                    ui.horizontal(|ui| {
                        ui.label("Display units");
                        ui.selectable_value(&mut self.settings.units, Units::Millimetres, "mm");
                        ui.selectable_value(&mut self.settings.units, Units::Mils, "mil");
                    });
                    ui.horizontal(|ui| {
                        ui.label("Cursor Type");
                        egui::ComboBox::from_id_salt("pcb_cursor_type")
                            .selected_text(self.settings.pcb_navigation.cursor.label())
                            .show_ui(ui, |ui| {
                                for style in [eda_render::pcb::CursorStyle::Small90,
                                    eda_render::pcb::CursorStyle::Small45, eda_render::pcb::CursorStyle::Large90] {
                                    ui.selectable_value(&mut self.settings.pcb_navigation.cursor,style,style.label());
                                }
                            });
                    });
                    ui.checkbox(&mut self.settings.pcb_navigation.wheel_zoom, "Zoom with mouse wheel")
                        .on_hover_text("Ctrl+wheel always zooms. When disabled, the wheel scrolls vertically; Shift+wheel scrolls horizontally.");
                    ui.checkbox(&mut self.settings.show_grid, "Show grid");
                    ui.horizontal(|ui| {
                        ui.label("Grid step");
                        egui::ComboBox::from_id_salt("grid_step")
                            .selected_text(format!("{} mm", self.settings.grid_nm as f64 / 1e6))
                            .show_ui(ui, |ui| {
                                for step in [100_000, 250_000, 500_000, 1_000_000, 2_500_000] {
                                    ui.selectable_value(
                                        &mut self.settings.grid_nm,
                                        step,
                                        format!("{} mm", step as f64 / 1e6),
                                    );
                                }
                            });
                    });
                    ui.separator();
                    ui.label(
                        RichText::new("Preferences apply immediately and are saved on exit.")
                            .small()
                            .color(MUTED),
                    );
                });
            self.preferences = open;
            if old_grid != (self.settings.show_grid, self.settings.grid_nm) {
                for doc in &mut self.documents {
                    doc.view.grid = self.settings.show_grid;
                    doc.view.grid_nm = self.settings.grid_nm;
                }
            }
        }
        if self.about {
            egui::Window::new("About PCBCraft")
                .open(&mut self.about)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("PCBCraft 0.1.0");
                    ui.label("Independent EDA application written in Rust.");
                    ui.label("egui / eframe · wgpu");
                    ui.separator();
                    ui.label("Altium schematic/PCB import and experimental PCB editing.");
                    ui.label("Native .PcbDoc Save and polygon repour are supported for the documented subset; DRC/3D and exact Altium parity are not verified or implemented.");
                });
        }
        if let Some(pending) = self.pending {
            let mut choice = 0;
            egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
                ui.set_width(410.0);
                ui.heading("Save changes?");
                ui.add_space(8.0);
                let names: Vec<_> = self
                    .documents
                    .iter()
                    .filter(|d| {
                        d.dirty()
                            && match pending {
                                Pending::Close(id) => d.session_id == id,
                                Pending::Quit => true,
                            }
                    })
                    .map(|d| d.model.name.clone())
                    .collect();
                ui.label("The following documents have unsaved changes:");
                for name in names {
                    ui.label(RichText::new(name).color(ACCENT));
                }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = 1;
                    }
                    if ui.button("Discard Changes").clicked() {
                        choice = 2;
                    }
                    if ui.button("Cancel").clicked() {
                        choice = 3;
                    }
                });
            });
            if choice == 3 {
                self.pending = None;
            }
            if choice == 1 || choice == 2 {
                let mut proceed = true;
                if choice == 1 {
                    for i in 0..self.documents.len() {
                        let target = match pending {
                            Pending::Close(id) => self.documents[i].session_id == id,
                            Pending::Quit => true,
                        };
                        if target && self.documents[i].dirty() && !self.save_index(i, false) {
                            proceed = false;
                            break;
                        }
                    }
                }
                if proceed {
                    self.pending = None;
                    match pending {
                        Pending::Close(id) => self.remove_document(id),
                        Pending::Quit => {
                            self.allow_exit = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    }
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if self.pending.is_some() || ctx.wants_keyboard_input() {
            return;
        }
        if self.pcb_view_shortcuts(ctx) {
            return;
        }
        if self.cross_probe_shortcuts(ctx) {
            return;
        }
        if self.cross_probe.enabled {
            return;
        }
        if self.pcb_shortcuts(ctx) {
            return;
        }
        if self.schematic_shortcuts(ctx) {
            return;
        }
        ctx.input_mut(|input| {
            let ctrl = egui::Modifiers::CTRL;
            let shift = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
            for (modifiers, key, command) in [
                (shift, Key::N, Command::NewSchematic),
                (shift, Key::S, Command::SaveAs),
                (ctrl, Key::N, Command::NewBoard),
                (ctrl, Key::O, Command::Open),
                (ctrl, Key::S, Command::Save),
                (ctrl, Key::W, Command::Close),
                (ctrl, Key::F4, Command::Close),
                (ctrl, Key::Z, Command::Undo),
                (ctrl, Key::Y, Command::Redo),
                (egui::Modifiers::NONE, Key::Home, Command::Fit),
                (egui::Modifiers::NONE, Key::G, Command::ToggleGrid),
                (egui::Modifiers::NONE, Key::Q, Command::ToggleUnits),
            ] {
                if input.consume_key(modifiers, key) {
                    self.command = Some(command);
                    break;
                }
            }
            if input.key_pressed(Key::Escape)
                && let Some(i) = self.active
            {
                self.documents[i].view.selected = None;
                self.pad_edit = None;
            }
        });
    }
}

impl eframe::App for PcbCraft {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.run(Command::Quit, ctx);
        }
        self.shortcuts(ctx);
        self.title_bar(ctx);
        self.menus(ctx);
        self.status(ctx);
        if self.settings.projects {
            self.projects(ctx);
        }
        if self.settings.properties || self.settings.messages || self.settings.components {
            self.right_dock(ctx);
        }
        self.library_dialog(ctx);
        self.workspace(ctx);
        self.finish_schematic_interaction();
        self.finish_pcb_interaction(ctx);
        self.finish_library_drag(ctx);
        self.schematic_move_menu(ctx);
        self.pcb_dialogs(ctx);
        self.cross_probe_dialog(ctx);
        self.dialogs(ctx);
        self.resize_edges(ctx);
        if let Some(command) = self.command.take() {
            self.run(command, ctx);
        }
        let title = self
            .active
            .map(|i| {
                format!(
                    "{}{} — PCBCraft",
                    self.documents[i].model.name,
                    if self.documents[i].dirty() { " *" } else { "" }
                )
            })
            .unwrap_or_else(|| "Home — PCBCraft".into());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "pcbcraft.settings", &self.settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_state_follows_saved_content_and_copies_have_independent_tabs() {
        let model = Document::example();
        let mut first = OpenDocument::new(model.clone(), Some(PathBuf::from("one.json")));
        let second = OpenDocument::new(model, Some(PathBuf::from("two.json")));
        assert_ne!(first.session_id, second.session_id);
        assert!(!first.dirty());
        let mut edited = first.model.clone();
        edited.pads[0].position.x += 1_000_000;
        first.history.commit(&mut first.model, edited).unwrap();
        assert!(first.dirty());
        first.history.undo(&mut first.model);
        assert!(!first.dirty());
        first.history.redo(&mut first.model);
        assert!(first.dirty());
        assert!(!second.dirty());
    }
    #[test]
    fn pcb_navigation_preferences_restore_and_old_settings_keep_small_cursor() {
        let old: Settings = serde_json::from_str("{}").unwrap();
        assert!(!old.pcb_navigation.wheel_zoom);
        assert_eq!(
            old.pcb_navigation.cursor,
            eda_render::pcb::CursorStyle::Small90
        );
        let mut settings = Settings::default();
        settings.pcb_navigation.wheel_zoom = true;
        settings.pcb_navigation.cursor = eda_render::pcb::CursorStyle::Small45;
        let restored: Settings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert!(restored.pcb_navigation.wheel_zoom);
        assert_eq!(
            restored.pcb_navigation.cursor,
            eda_render::pcb::CursorStyle::Small45
        );
    }
}
