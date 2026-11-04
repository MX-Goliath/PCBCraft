use crate::theme::{self, ACCENT, MUTED};
#[path = "schematic_hierarchy.rs"]
mod schematic_hierarchy;
#[path = "schematic_sync.rs"]
mod schematic_sync;
use eda_core::{Document, DocumentKind, History, Layer, Units};
use eda_render::{View, layer_color};
use egui::{Color32, Key, RichText};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;
#[path = "annotation.rs"]
mod annotation;
#[path = "chrome.rs"]
mod chrome;
#[path = "components.rs"]
mod components;
#[path = "cross_probe.rs"]
mod cross_probe;
#[path = "design_sync.rs"]
mod design_sync;
#[path = "document_bars.rs"]
mod document_bars;
#[path = "document_loader.rs"]
mod document_loader;
#[path = "file_menu.rs"]
mod file_menu;
#[path = "file_save.rs"]
mod file_save;
#[path = "layer_stack.rs"]
mod layer_stack;
#[path = "library_editor.rs"]
mod library_editor;
#[path = "pcb_3d.rs"]
mod pcb_3d;
#[path = "pcb_editor.rs"]
mod pcb_editor;
#[path = "projects.rs"]
mod projects;
#[path = "properties.rs"]
mod properties;
#[path = "schematic_active_bar.rs"]
mod schematic_active_bar;
#[path = "schematic_editor.rs"]
mod schematic_editor;
#[path = "schematic_netlist.rs"]
mod schematic_netlist;
#[path = "schematic_output.rs"]
mod schematic_output;
#[path = "schematic_properties.rs"]
mod schematic_properties;
#[path = "schematic_validation.rs"]
mod schematic_validation;
#[path = "selection_controls.rs"]
mod selection_controls;
#[path = "variants.rs"]
mod variants;
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
    new_project: Option<PathBuf>,
    session_id: Uuid,
    model: Document,
    path: Option<PathBuf>,
    source_path: Option<PathBuf>,
    saved: Option<Document>,
    content_dirty: std::cell::Cell<Option<(u64, bool)>>,
    history: History,
    view: View,
    sch_editor: schematic_editor::Editor,
    pcb_editor: pcb_editor::Editor,
}

impl OpenDocument {
    fn new(model: Document, path: Option<PathBuf>) -> Self {
        let saved = path.as_ref().map(|_| model.clone());
        Self {
            new_project: None,
            session_id: Uuid::new_v4(),
            model,
            path,
            source_path: None,
            content_dirty: std::cell::Cell::new(Some((0, saved.is_none()))),
            saved,
            history: History::default(),
            view: View::default(),
            sch_editor: Default::default(),
            pcb_editor: Default::default(),
        }
    }
    fn dirty(&self) -> bool {
        self.content_is_dirty()
            || self.pcb_editor.has_pending()
            || self
                .model
                .pcb
                .as_ref()
                .is_some_and(|b| self.pcb_editor.properties_dirty(b, self.history.revision))
    }
    fn content_is_dirty(&self) -> bool {
        if let Some((revision, dirty)) = self.content_dirty.get()
            && revision == self.history.revision
        {
            return dirty;
        }
        let dirty = self
            .saved
            .as_ref()
            .is_none_or(|s| !eda_core::library_edit::same_contents(s, &self.model));
        self.content_dirty.set(Some((self.history.revision, dirty)));
        dirty
    }
    fn mark_saved(&mut self) {
        self.saved = Some(self.model.clone());
        self.content_dirty.set(Some((self.history.revision, false)));
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    components: bool,
    right_components: bool,
    libraries: Vec<PathBuf>,
    device_sheet_folders: Vec<(PathBuf, bool)>,
    snippet_folders: Vec<PathBuf>,
    projects: bool,
    properties: bool,
    messages: bool,
    units: Units,
    recent: Vec<PathBuf>,
    recent_projects: Vec<PathBuf>,
    recent_groups: Vec<PathBuf>,
    show_grid: bool,
    grid_nm: i64,
    right_messages: bool,
    pcb_navigation: eda_render::pcb::NavigationOptions,
    pcb_active_bar_last: [usize; 14],
    pcb_drc: std::collections::BTreeMap<PathBuf, pcb_editor::CheckSettings>,
    active_variants: std::collections::BTreeMap<PathBuf, Option<String>>,
    variant_style: eda_core::assembly_variants::DisplayStyle,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            components: false,
            right_components: false,
            libraries: vec![],
            device_sheet_folders: default_altium_folder("Library/Device Sheets")
                .map(|p| (p, true))
                .into_iter()
                .collect(),
            snippet_folders: default_altium_folder("Examples/Snippets Examples")
                .into_iter()
                .collect(),
            projects: true,
            properties: true,
            messages: false,
            units: Units::Millimetres,
            recent: vec![],
            recent_projects: vec![],
            recent_groups: vec![],
            show_grid: true,
            grid_nm: 1_000_000,
            right_messages: false,
            pcb_navigation: Default::default(),
            pcb_active_bar_last: [0; 14],
            pcb_drc: Default::default(),
            active_variants: Default::default(),
            variant_style: Default::default(),
        }
    }
}

fn default_altium_folder(relative: &str) -> Option<PathBuf> {
    std::env::var_os("PUBLIC")
        .map(PathBuf::from)
        .map(|p| p.join("Documents/Altium/AD24").join(relative))
        .filter(|p| p.is_dir())
}

#[derive(Clone, Copy)]
enum Command {
    Library(library_editor::Action),
    Variants,
    ValidateProject,
    ValidationMessage(bool),
    File(file_menu::Action),
    SynchronizeEntries(bool),
    NavigateHierarchy,
    CrossProbe,
    Pcb3d(bool),
    Pcb3dCamera(eda_render::pcb_3d::CameraCommand),
    LayerStack,
    Annotate,
    Synchronize(eda_core::design_sync::Direction, bool),
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
    library_editor: library_editor::State,
    variants: variants::State,
    file_state: file_menu::State,
    sheet_sync: schematic_sync::State,
    document_loader: document_loader::State,
    annotation: Option<annotation::Dialog>,
    design_sync: Option<design_sync::Dialog>,
    layer_stack: Option<layer_stack::Manager>,
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
        if let Some(render) = &cc.wgpu_render_state {
            eda_render::pcb::enable_gpu_with_device(
                &cc.egui_ctx,
                render.target_format,
                &render.device,
                &render.queue,
            );
            eda_render::pcb_3d::enable_gpu(&cc.egui_ctx, &render.device);
        }
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, "pcbcraft.settings"))
            .unwrap_or_default();
        let mut documents = Vec::new();
        for path in std::mem::take(&mut settings.recent) {
            if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("PrjPcb"))
            {
                if !settings.recent_projects.contains(&path) {
                    settings.recent_projects.push(path);
                }
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("DsnWrk"))
            {
                if !settings.recent_groups.contains(&path) {
                    settings.recent_groups.push(path);
                }
            } else {
                documents.push(path);
            }
        }
        settings.recent = documents;
        if ![100_000, 250_000, 500_000, 1_000_000, 2_500_000].contains(&settings.grid_nm) {
            settings.grid_nm = 1_000_000;
        }
        let mut app = Self {
            library_editor: Default::default(),
            variants: Default::default(),
            file_state: Default::default(),
            sheet_sync: Default::default(),
            document_loader: Default::default(),
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
            layer_stack: None,
            annotation: None,
            design_sync: None,
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
            app.request_open_path(path.into());
            opened =
                !app.documents.is_empty() || !app.projects.is_empty() || app.document_loader.busy();
        }
        if !opened {
            app.add_document(Document::example(), None);
        }
        app
    }

    fn message(&mut self, error: bool, text: impl Into<String>) {
        let text = text.into();
        self.file_state.validation.application_message(error, &text);
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
        opened.new_project = self.file_state.project_for_new.take();
        if let Some(settings) = opened
            .path
            .as_ref()
            .and_then(|p| self.settings.pcb_drc.get(p))
        {
            opened.pcb_editor.restore_rule_settings(settings.clone());
        }
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
        let path = path.canonicalize().unwrap_or(path);
        let target = if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("PrjPcb"))
        {
            &mut self.settings.recent_projects
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("DsnWrk"))
        {
            &mut self.settings.recent_groups
        } else {
            &mut self.settings.recent
        };
        target.retain(|p| *p != path);
        target.insert(0, path);
        target.truncate(10);
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
        if extension == "dsnwrk" {
            self.open_group(path);
            return;
        }
        if matches!(extension.as_str(), "intlib") {
            self.import_library(path, true);
            return;
        }
        let loaded = if extension == "pcbdoc" {
            eda_io::altium_pcb::load_pcb(&path)
        } else if extension == "schdoc" {
            eda_io::altium::load_schematic(&path)
        } else {
            eda_io::load(&path)
        };
        self.complete_open_path(path, loaded);
    }

    fn complete_open_path(&mut self, path: PathBuf, loaded: Result<Document, String>) {
        // A synchronous dependency lookup may have opened a queued path first.
        if self
            .documents
            .iter()
            .any(|d| d.path.as_ref() == Some(&path) || d.source_path.as_ref() == Some(&path))
        {
            return;
        }
        let native = path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("SchDoc") || s.eq_ignore_ascii_case("PcbDoc"));
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
                    self.add_document(doc, Some(path.clone()));
                    let opened = self.documents.last_mut().unwrap();
                    opened.source_path = Some(path);
                    opened.mark_saved();
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
        self.file_state.selected_project = Some(path.clone());
        if let Some(project) = self.projects.iter().find(|p| p.path == path) {
            if let Some(first) = project
                .documents
                .iter()
                .filter(|d| d.kind == "schdoc" && d.exists)
                .min_by_key(|d| project.sheet_number(&d.path).unwrap_or(usize::MAX))
                .or_else(|| {
                    project
                        .documents
                        .iter()
                        .find(|d| d.kind == "pcbdoc" && d.exists)
                })
                .map(|d| d.path.clone())
            {
                if first
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("PcbDoc"))
                {
                    self.request_open_path(first);
                } else {
                    self.open_path(first);
                }
            }
            return;
        }
        match eda_io::altium::load_project(&path) {
            Ok(project) => {
                let first = project
                    .documents
                    .iter()
                    .filter(|d| d.kind == "schdoc" && d.exists)
                    .min_by_key(|d| project.sheet_number(&d.path).unwrap_or(usize::MAX))
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
                for entry in project.documents.iter().filter(|d| {
                    d.exists && matches!(d.kind.as_str(), "schlib" | "intlib" | "pcblib")
                }) {
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
                    if first
                        .extension()
                        .is_some_and(|s| s.eq_ignore_ascii_case("PcbDoc"))
                    {
                        self.request_open_path(first);
                    } else {
                        self.open_path(first);
                    }
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
            .any(|d| d.session_id == id && d.pcb_editor.rules_open())
        {
            self.message(true, "Apply or cancel Rules before closing the board");
            return;
        }
        if let Some(manager) = &mut self.layer_stack
            && manager.session == id
        {
            if manager.dirty() {
                manager.visible = true;
                self.message(
                    true,
                    "Apply or discard stack changes before closing the board",
                );
                return;
            }
            self.layer_stack = None;
        }
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
        if self.documents.iter().any(|d| d.pcb_editor.rules_open()) {
            self.message(true, "Apply or cancel Rules before another command");
            return;
        }
        if matches!(command, Command::Quit)
            && let Some(manager) = &mut self.layer_stack
            && manager.dirty()
        {
            manager.visible = true;
            self.message(true, "Apply or discard stack changes before quitting");
            return;
        }
        if self.annotation.is_some()
            || self.design_sync.is_some()
            || self.sheet_sync.busy()
            || self.file_state.modal()
            || self.library_editor.modal()
            || self.variants.dialog.is_some()
        {
            self.message(true, "Close the active dialog before another command");
            return;
        }
        if self.layer_stack_active() {
            self.layer_stack_command(command);
            return;
        }
        if matches!(
            command,
            Command::Save
                | Command::SaveAs
                | Command::SaveAll
                | Command::Close
                | Command::Quit
                | Command::NewBoard
                | Command::NewSchematic
                | Command::Open
        ) && let Err(error) = self.commit_library_properties()
        {
            self.message(true, error);
            return;
        }
        if !matches!(command, Command::CrossProbe | Command::Fit) {
            self.cross_probe.enabled = false;
        }
        match command {
            Command::Library(action) => self.library_command(action),
            Command::Variants => self.open_variants(),
            Command::File(action) => self.file_command(action, ctx),
            Command::SynchronizeEntries(selected) => self.start_sheet_sync(selected),
            Command::NavigateHierarchy => self.start_hierarchy_navigation(),
            Command::CrossProbe => self.start_cross_probe(),
            Command::Annotate => self.open_annotation(),
            Command::Synchronize(direction, differences) => {
                self.open_design_sync(direction, differences)
            }
            Command::ValidateProject => self.validate_schematic_project(),
            Command::ValidationMessage(next) => self.next_validation_message(next),
            Command::LayerStack => self.open_layer_stack(),
            Command::Pcb3d(enabled) => {
                self.set_pcb_3d(enabled);
                ctx.request_repaint();
            }
            Command::SchPlace(kind) => self.start_schematic_tool(kind),
            Command::SchEdit(action) => self.schematic_action(action, ctx),
            Command::Pcb(action) => self.pcb_action(action),
            Command::NewBoard | Command::NewSchematic => {
                let board = matches!(command, Command::NewBoard);
                let n = self.documents.len() + 1;
                self.file_state.project_for_new = self
                    .file_project_index()
                    .map(|i| self.projects[i].path.clone());
                if board {
                    self.add_document(
                        Document::new(DocumentKind::Board, format!("PCB{n}.PcbDoc")),
                        None,
                    );
                } else {
                    match eda_io::altium::new_schematic(&format!("Sheet{n}.SchDoc")) {
                        Ok(doc) => self.add_document(doc, None),
                        Err(e) => self.message(true, e),
                    }
                }
            }
            Command::Example => self.add_document(Document::example(), None),
            Command::Open => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .add_filter(
                        "Design documents and libraries",
                        &[
                            "SchDoc", "PcbDoc", "PrjPcb", "DsnWrk", "SchLib", "PcbLib", "IntLib",
                            "json",
                        ],
                    )
                    .add_filter("Altium libraries", &["SchLib", "PcbLib", "IntLib"])
                    .add_filter("Altium schematic", &["SchDoc"])
                    .add_filter("Altium PCB", &["PcbDoc"])
                    .add_filter("Altium project", &["PrjPcb"])
                    .pick_files()
                {
                    for path in path {
                        self.request_open_path(path);
                    }
                }
            }
            Command::OpenProject => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .add_filter("Altium PCB project", &["PrjPcb"])
                    .pick_file()
                {
                    self.request_open_path(path);
                }
            }
            Command::Save | Command::SaveAs => {
                if !self.save_variants() {
                    return;
                }
                if let Some(i) = self.active {
                    self.save_index(i, matches!(command, Command::SaveAs));
                }
            }
            Command::SaveAll => {
                if !self.save_variants() {
                    return;
                }
                let mut saved = true;
                for i in 0..self.documents.len() {
                    if self.documents[i].dirty() && !self.save_index(i, false) {
                        saved = false;
                        break;
                    }
                }
                if saved && self.group_dirty() {
                    self.save_project_group(false);
                }
            }
            Command::Close => {
                if let Some(i) = self.active {
                    self.request_close(self.documents[i].session_id);
                }
            }
            Command::Undo | Command::Redo => {
                if self.variant_undo(matches!(command, Command::Redo)) {
                    return;
                }
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
            Command::Pcb3dCamera(command) => {
                if let Some(index) = self.active
                    && self.documents[index].view.pcb.three_d.enabled
                {
                    self.documents[index]
                        .view
                        .pcb
                        .three_d
                        .camera_command(command);
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
                if self.documents.iter().any(OpenDocument::dirty)
                    || self.group_dirty()
                    || self.variants_dirty()
                {
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
        if self.library_active() {
            self.library_menus(ctx);
            return;
        }
        if self.layer_stack_active() {
            self.layer_stack_menus(ctx);
            return;
        }
        let has_doc = self.active.is_some();
        let undo = self
            .active
            .is_some_and(|i| self.documents[i].history.can_undo());
        let redo = self
            .active
            .is_some_and(|i| self.documents[i].history.can_redo());
        egui::TopBottomPanel::top("menu")
            .exact_height(29.0)
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_gray(58))
                    .inner_margin(egui::Margin {
                        left: 1,
                        right: 1,
                        top: 3,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.menu_button("File", |ui| self.file_menu(ui));
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
                            let three_d = self
                                .active
                                .is_some_and(|i| self.documents[i].view.pcb.three_d.enabled);
                            if three_d {
                                use eda_render::pcb_3d::CameraCommand::*;
                                self.action(
                                    ui,
                                    "Flip Board",
                                    "Ctrl+F",
                                    Command::Pcb3dCamera(Flip),
                                    true,
                                );
                                ui.menu_button("3D View Control", |ui| {
                                    for (label, shortcut, command) in [
                                        ("Top Side", "", Top),
                                        ("Bottom Side", "", Bottom),
                                        ("Front Side", "", Front),
                                        ("Back Side", "", Back),
                                        ("Left Edge", "", Left),
                                        ("Right Edge", "", Right),
                                        ("Rotate Left", "", RotateLeft),
                                        ("Rotate Right", "", RotateRight),
                                        ("Rotate Up", "", RotateUp),
                                        ("Rotate Down", "", RotateDown),
                                        ("Move Left", "", MoveLeft),
                                        ("Move Right", "", MoveRight),
                                        ("Move Up", "", MoveUp),
                                        ("Move Down", "", MoveDown),
                                        ("Isometric View", "0", Isometric),
                                        ("Flipped Isometric View", "Ctrl+0", FlippedIsometric),
                                    ] {
                                        self.action(
                                            ui,
                                            label,
                                            shortcut,
                                            Command::Pcb3dCamera(command),
                                            true,
                                        );
                                    }
                                    ui.add_enabled(false, egui::Button::new("Fold/Unfold"));
                                    self.action(
                                        ui,
                                        "90 Degree Rotation",
                                        "9",
                                        Command::Pcb3dCamera(QuarterTurn),
                                        true,
                                    );
                                });
                            }
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
                        self.project_menu(ui);
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
                                        "Make Library from Document…",
                                        "",
                                        Command::Library(library_editor::Action::FromDocument),
                                        true,
                                    );
                                    if schematic {
                                        self.action(
                                            ui,
                                            "Up/Down Hierarchy",
                                            "",
                                            Command::NavigateHierarchy,
                                            true,
                                        );
                                        ui.separator();
                                        for label in [
                                            "Parameter Manager...",
                                            "Footprint Manager...",
                                            "Update From Libraries...",
                                            "Update Selected From Libraries...",
                                            "Update Parameters From Database...",
                                            "Item Manager...",
                                        ] {
                                            ui.add_enabled(false, egui::Button::new(label));
                                        }
                                        ui.separator();
                                        ui.menu_button("Annotation", |ui| {
                                            self.action(
                                                ui,
                                                "Annotate Schematics...",
                                                "",
                                                Command::Annotate,
                                                true,
                                            );
                                        });
                                        ui.separator();
                                        ui.add_enabled(
                                            false,
                                            egui::Button::new("Signal Integrity..."),
                                        );
                                        ui.separator();
                                        ui.add_enabled(
                                            false,
                                            egui::Button::new("Convert                         ▸"),
                                        );
                                        ui.separator();
                                    }
                                    self.action(
                                        ui,
                                        "Cross Probe",
                                        "T, C",
                                        Command::CrossProbe,
                                        true,
                                    );
                                    if schematic {
                                        for label in [
                                            "Cross Select Mode     Shift+Ctrl+X",
                                            "Select PCB Components",
                                        ] {
                                            ui.add_enabled(false, egui::Button::new(label));
                                        }
                                        ui.separator();
                                        ui.add_enabled(
                                            false,
                                            egui::Button::new("Configure Pin Swapping..."),
                                        );
                                        ui.separator();
                                        self.action(
                                            ui,
                                            "Preferences...",
                                            "",
                                            Command::Preferences,
                                            true,
                                        );
                                        return;
                                    }
                                    ui.separator();
                                }
                                if label == "Design" && schematic {
                                    let project = self.sync_project_name();
                                    self.action(
                                        ui,
                                        "Update PCB Document…",
                                        "",
                                        Command::Synchronize(
                                            eda_core::design_sync::Direction::ToPcb,
                                            false,
                                        ),
                                        project.is_some(),
                                    );
                                    ui.separator();
                                    self.action(
                                        ui,
                                        "Synchronize Sheet Entries and Ports",
                                        "",
                                        Command::SynchronizeEntries(false),
                                        project.is_some(),
                                    );
                                    return;
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

    fn status(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(31.0)
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
                    if let Some(status) = self
                        .active
                        .and_then(|i| self.documents[i].pcb_editor.selection_status())
                    {
                        ui.separator();
                        ui.label(RichText::new(status).size(11.0));
                    }
                    if self.cross_probe.enabled {
                        ui.separator();
                        ui.label(RichText::new("Cross Probe · Esc exits").strong())
                            .on_hover_text(&self.cross_probe.status);
                    }
                    if let Some(status) = self
                        .active
                        .and_then(|i| self.documents[i].sch_editor.path_status())
                    {
                        ui.separator();
                        ui.label(RichText::new(status).size(11.0));
                    }
                    if let Some(status) = self.active.and_then(|i| {
                        self.documents[i]
                            .pcb_editor
                            .routing_status(&self.documents[i].view.pcb)
                    }) {
                        ui.separator();
                        ui.label(RichText::new(status).size(11.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("Panels", |ui| {
                            if self
                                .active
                                .is_some_and(|i| self.documents[i].model.pcb.is_some())
                            {
                                self.action(
                                    ui,
                                    "View Configuration",
                                    "L",
                                    Command::Pcb(pcb_editor::Action::Layers),
                                    true,
                                );
                            }
                            self.action(ui, "Components", "", Command::ToggleComponents, true);
                            self.action(ui, "Projects", "", Command::ToggleProjects, true);
                            self.action(ui, "Properties", "", Command::ToggleProperties, true);
                            self.action(ui, "Messages", "", Command::ToggleMessages, true);
                        });
                    });
                });
            });
    }

    fn view_configuration(&mut self, ctx: &egui::Context) {
        let Some(i) = self.active else {
            return;
        };
        let mut close = false;
        let mut projects = false;
        let mut requested_mode = None;
        egui::SidePanel::left("view_configuration_altium_v1")
            .default_width(330.0)
            .width_range(300.0..=600.0)
            .resizable(true)
            .frame(
                egui::Frame::NONE
                    .fill(theme::PANEL)
                    .inner_margin(egui::Margin {
                        left: 4,
                        right: 4,
                        top: 0,
                        bottom: 4,
                    }),
            )
            .show(ctx, |ui| {
                let (header, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 23.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(header, 0.0, Color32::from_rgb(92, 128, 155));
                ui.painter().text(
                    header.left_center() + egui::vec2(5.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "View Configuration",
                    egui::FontId::new(11.0, egui::FontFamily::Name("ui-bold".into())),
                    Color32::from_gray(240),
                );
                for (n, icon) in [theme::Icon::Close, theme::Icon::Pin]
                    .into_iter()
                    .enumerate()
                {
                    let r = egui::Rect::from_min_size(
                        egui::pos2(header.right() - 17.0 * (n + 1) as f32, header.top()),
                        egui::vec2(17.0, 23.0),
                    );
                    theme::draw_icon(
                        ui.painter(),
                        egui::Rect::from_center_size(r.center(), egui::vec2(16., 16.)),
                        icon,
                        n == 0,
                    );
                    if n == 0
                        && ui
                            .interact(r, ui.id().with("view_config_close"), egui::Sense::click())
                            .clicked()
                    {
                        close = true;
                    }
                }
                let c = egui::pos2(header.right() - 42., header.center().y);
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + egui::vec2(-3., -2.),
                        c + egui::vec2(3., -2.),
                        c + egui::vec2(0., 3.),
                    ],
                    Color32::GRAY,
                    egui::Stroke::NONE,
                ));
                egui::TopBottomPanel::bottom("view_configuration_tabs")
                    .exact_height(23.0)
                    .frame(egui::Frame::NONE)
                    .show_inside(ui, |ui| {
                        let (tabs, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 23.0),
                            egui::Sense::hover(),
                        );
                        let mut x = tabs.left();
                        for (n, name, width) in [
                            (0, "Projects", 52.0),
                            (1, "Navigator", 65.0),
                            (2, "PCB", 32.0),
                            (3, "PCB Filter", 62.0),
                            (4, "View Configuration", 110.0),
                        ] {
                            let r = egui::Rect::from_min_size(
                                egui::pos2(x, tabs.top()),
                                egui::vec2(width, 23.0),
                            );
                            if n == 4 {
                                ui.painter().rect_filled(r, 0.0, Color32::from_gray(76));
                            }
                            ui.painter().line_segment(
                                [r.right_top(), r.right_bottom()],
                                egui::Stroke::new(1.0, Color32::from_gray(40)),
                            );
                            ui.painter().text(
                                r.center(),
                                egui::Align2::CENTER_CENTER,
                                name,
                                egui::FontId::proportional(11.0),
                                if n == 0 || n == 4 {
                                    Color32::from_gray(212)
                                } else {
                                    Color32::GRAY
                                },
                            );
                            if n == 0
                                && ui
                                    .interact(
                                        r,
                                        ui.id().with("view_config_projects"),
                                        egui::Sense::click(),
                                    )
                                    .clicked()
                            {
                                projects = true;
                            }
                            x += width;
                        }
                    });
                egui::Frame::NONE
                    .inner_margin(egui::Margin {
                        left: 6,
                        right: 4,
                        top: 0,
                        bottom: 0,
                    })
                    .show(ui, |ui| {
                        let doc = &mut self.documents[i];
                        let previous_mode = doc.view.pcb.three_d.enabled;
                        let layer = doc.view.pcb.configuration_panel(
                            ui,
                            doc.model.pcb.as_ref().unwrap(),
                            &mut doc.view.grid,
                            &mut doc.view.camera,
                        );
                        if doc.view.pcb.three_d.enabled != previous_mode {
                            requested_mode = Some(doc.view.pcb.three_d.enabled);
                            doc.view.pcb.three_d.enabled = previous_mode;
                        }
                        if let Some(layer) = layer {
                            self.command =
                                Some(Command::Pcb(pcb_editor::Action::RouteLayer(layer)));
                        }
                    });
            });
        if let Some(enabled) = requested_mode {
            self.set_pcb_3d(enabled);
        }
        if close || projects {
            self.documents[i].view.pcb.show_layer_dialog = false;
            if projects {
                self.settings.projects = true;
            }
        }
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
                self.messages_panel(ui);
            } else {
                let compiled = self
                    .active
                    .is_some_and(|i| self.documents[i].sch_editor.compiled_path.is_some());
                ui.add_enabled_ui(!compiled, |ui| self.properties(ui));
            }
        });
    }

    fn workspace(&mut self, ctx: &egui::Context) {
        if self.layer_stack_active() {
            self.layer_stack_workspace(ctx);
            return;
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Color32::from_rgb(109, 106, 105))
                    .inner_margin(egui::Margin::symmetric(5, 0)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                self.document_tabs(ui);
                if let Some(index) = self.active {
                    self.library_model_table(ui, index);
                    if self.documents[index].model.kind == DocumentKind::Board {
                        self.pcb_layer_bar(ui, index);
                    }
                    if self.documents[index].model.kind == DocumentKind::Schematic {
                        self.schematic_bottom_bar(ui, index);
                    }
                    let canvas = ui.available_rect_before_wrap();
                    if self.documents[index].model.kind == DocumentKind::Schematic {
                        ui.painter()
                            .rect_filled(canvas, 0.0, Color32::from_gray(195));
                        ui.add_space(26.0);
                    }
                    let projection = self.variant_projection(index);
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
                    let output = eda_render::canvas(
                        ui,
                        projection.as_deref().unwrap_or(&doc.model),
                        &mut doc.view,
                    );
                    let pcb_input = doc.view.pcb.input.take();
                    self.cursor_mm = output.cursor_mm;
                    if let Some(input) = &output.schematic_input
                        && self.documents[index].sch_editor.compiled_path.is_some()
                        && input.clicked
                    {
                        self.documents[index].view.selected = input.hit;
                    }
                    if let Some(input) = output.schematic_input
                        && self.documents[index].sch_editor.compiled_path.is_none()
                        && !self.sheet_sync_input(index, &input, ctx)
                        && !self.hierarchy_click(index, &input, ctx)
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
                        self.schematic_harness_overlay(ctx, canvas, index);
                    }
                    if !self.documents[index].view.pcb.three_d.enabled
                        && !self.library_editor.modal()
                    {
                        if self.documents[index].sch_editor.compiled_path.is_some() {
                            self.variant_active_bar(ctx, canvas, index);
                        } else if !self.library_active_bar(ctx, canvas) {
                            self.active_bar(ctx, canvas);
                        }
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
                            self.request_open_path(path);
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
                let mut names: Vec<_> = self
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
                if matches!(pending, Pending::Quit) && self.group_dirty() {
                    names.push(self.file_state.group_name());
                }
                if matches!(pending, Pending::Quit) {
                    for p in &self.projects {
                        if self.variants.dirty(p) {
                            names.push(p.name.clone());
                        }
                    }
                }
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
                    if matches!(pending, Pending::Quit) && !self.save_variants() {
                        return;
                    }
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
                    if choice == 1
                        && matches!(pending, Pending::Quit)
                        && self.group_dirty()
                        && !self.save_project_group(false)
                    {
                        return;
                    }
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
        if let Some(index) = self.active
            && self.documents[index].sch_editor.bar.resources.modal()
        {
            self.schematic_bar_shortcuts(index, ctx);
            return;
        }
        if self.sheet_sync.busy() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                self.sheet_sync_escape();
            }
            return;
        }
        if self.annotation.is_some()
            || self.file_state.modal()
            || self.library_editor.modal()
            || self.variants.dialog.is_some()
            || self.design_sync.is_some()
            || self.layer_stack_active()
            || self.pending.is_some()
            || self
                .active
                .is_some_and(|i| self.documents[i].view.pcb.configuration_modal_open())
        {
            return;
        }
        if let Some(index) = self.active
            && self.documents[index].model.pcb.is_some()
            && self.documents[index].pcb_editor.bar.popup_open()
        {
            self.pcb_bar_shortcuts(index, ctx);
            return;
        }
        if ctx.wants_keyboard_input() {
            if let Some(index) = self.active
                && self.documents[index].sch_editor.paused
                && ctx.input(|i| i.key_pressed(Key::Escape))
            {
                self.documents[index].sch_editor.paused = false;
            }
            if let Some(index) = self.active
                && self.documents[index].pcb_editor.routing_paused()
                && ctx.input(|i| i.key_pressed(Key::Escape))
            {
                // Leave Escape available to the focused property field so it
                // can restore its original value while routing resumes.
                self.documents[index].pcb_editor.resume_routing(ctx);
            }
            return;
        }
        if self.pcb_view_shortcuts(ctx) {
            return;
        }
        if self.cross_probe.enabled {
            self.cross_probe_shortcuts(ctx);
            return;
        }
        if self.pcb_shortcuts(ctx) {
            return;
        }
        if self.cross_probe_shortcuts(ctx) {
            return;
        }
        if self.schematic_shortcuts(ctx) {
            return;
        }
        ctx.input_mut(|input| {
            let ctrl = egui::Modifiers::CTRL;
            let shift = egui::Modifiers::CTRL | egui::Modifiers::SHIFT;
            for (modifiers, key, command) in [
                (shift, Key::F3, Command::ValidationMessage(true)),
                (shift, Key::F4, Command::ValidationMessage(false)),
                (shift, Key::N, Command::NewSchematic),
                (shift, Key::S, Command::SaveAs),
                (ctrl, Key::N, Command::NewBoard),
                (ctrl, Key::O, Command::Open),
                (ctrl, Key::S, Command::Save),
                (ctrl, Key::P, Command::File(file_menu::Action::Print)),
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
        self.poll_document_loader(ctx);
        if std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some()
            && let Some(seconds) = _frame.info().cpu_usage
        {
            let report = ctx.data_mut(|data| {
                let samples =
                    data.get_temp_mut_or_default::<Vec<f64>>(egui::Id::new("frame-cpu-timings"));
                samples.push(seconds as f64 * 1000.);
                if samples.len() < 120 {
                    return None;
                }
                let mut samples = std::mem::take(samples);
                samples.sort_by(f64::total_cmp);
                Some((
                    samples[60],
                    samples[113],
                    samples.iter().sum::<f64>() / samples.len() as f64,
                    samples[119],
                ))
            });
            if let Some((median, p95, mean, maximum)) = report {
                eprintln!(
                    "frame_cpu_ms median={median:.2} p95={p95:.2} mean={mean:.2} max={maximum:.2} n=120"
                );
            }
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_exit {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.run(Command::Quit, ctx);
        }
        self.shortcuts(ctx);
        self.title_bar(ctx);
        self.menus(ctx);
        self.status(ctx);
        let view_configuration = !self.layer_stack_active()
            && self.active.is_some_and(|i| {
                self.documents[i].model.pcb.is_some()
                    && self.documents[i].view.pcb.show_layer_dialog
            });
        if view_configuration {
            self.view_configuration(ctx);
        } else if self.settings.projects {
            self.projects(ctx);
        }
        if !self.layer_stack_active()
            && (self.settings.properties || self.settings.messages || self.settings.components)
        {
            self.right_dock(ctx);
        }
        self.library_dialog(ctx);
        self.workspace(ctx);
        self.library_editor_dialogs(ctx);
        self.finish_schematic_interaction();
        self.finish_pcb_interaction(ctx);
        self.finish_library_drag(ctx);
        self.schematic_move_menu(ctx);
        self.pcb_dialogs(ctx);
        self.cross_probe_dialog(ctx);
        self.dialogs(ctx);
        self.annotation_dialog(ctx);
        self.design_sync_dialog(ctx);
        self.sheet_sync_dialog(ctx);
        self.sheet_sync_context_menu(ctx);
        self.selection_popup(ctx);
        self.document_loading_dialog(ctx);
        self.file_dialogs(ctx);
        self.variants_dialog(ctx);
        self.resize_edges(ctx);
        if let Some(command) = self.command.take() {
            self.run(command, ctx);
        }
        self.poll_pcb_pours(ctx);
        for doc in &self.documents {
            if doc.model.pcb.is_some()
                && let Some(path) = &doc.path
            {
                let settings = doc.pcb_editor.rule_settings();
                if self.settings.pcb_drc.get(path) != Some(settings) {
                    self.settings.pcb_drc.insert(path.clone(), settings.clone());
                }
            }
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
        let changed = ctx.data_mut(|data| {
            let id = egui::Id::new("pcbcraft-window-title");
            if data.get_temp::<String>(id).as_ref() == Some(&title) {
                false
            } else {
                data.insert_temp(id, title.clone());
                true
            }
        });
        if changed {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
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
        first.mark_saved();
        assert!(!first.dirty());
        first.history.undo(&mut first.model);
        assert!(first.dirty());
        first.history.redo(&mut first.model);
        assert!(!first.dirty());
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
