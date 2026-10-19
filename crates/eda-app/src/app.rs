use crate::theme::{self, ACCENT, MUTED};
use eda_core::{Document, DocumentKind, History, Layer, Units};
use eda_render::{View, layer_color};
use egui::{Color32, Key, RichText};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

struct OpenDocument {
    session_id: Uuid,
    model: Document,
    path: Option<PathBuf>,
    saved: Option<Document>,
    history: History,
    view: View,
}

impl OpenDocument {
    fn new(model: Document, path: Option<PathBuf>) -> Self {
        let saved = path.as_ref().map(|_| model.clone());
        Self {
            session_id: Uuid::new_v4(),
            model,
            path,
            saved,
            history: History::default(),
            view: View::default(),
        }
    }
    fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.model)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    projects: bool,
    properties: bool,
    messages: bool,
    units: Units,
    recent: Vec<PathBuf>,
    show_grid: bool,
    grid_nm: i64,
    right_messages: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            projects: true,
            properties: true,
            messages: false,
            units: Units::Millimetres,
            recent: vec![],
            show_grid: true,
            grid_nm: 1_000_000,
            right_messages: false,
        }
    }
}

#[derive(Clone, Copy)]
enum Command {
    NewBoard,
    NewSchematic,
    Open,
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
}

pub struct PcbCraft {
    file_dialog: rfd::FileDialog,
    documents: Vec<OpenDocument>,
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
            file_dialog: rfd::FileDialog::new().set_parent(cc),
            documents: vec![],
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
        };
        let mut opened = false;
        if let Some(path) = std::env::args_os().nth(1) {
            app.open_path(path.into());
            opened = !app.documents.is_empty();
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

    fn add_document(&mut self, doc: Document, path: Option<PathBuf>) {
        let mut opened = OpenDocument::new(doc, path);
        opened.view.grid = self.settings.show_grid;
        opened.view.grid_nm = self.settings.grid_nm;
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
            .position(|d| d.path.as_ref() == Some(&path))
        {
            self.active = Some(index);
            return;
        }
        match eda_io::load(&path) {
            Ok(doc) => {
                self.message(false, format!("Opened {}", path.display()));
                self.recent(path.clone());
                self.add_document(doc, Some(path));
            }
            Err(error) => self.message(true, format!("Could not open {}: {error}", path.display())),
        }
    }

    fn save_index(&mut self, index: usize, save_as: bool) -> bool {
        let Some(doc) = self.documents.get(index) else {
            return false;
        };
        let path = if save_as || doc.path.is_none() {
            self.file_dialog
                .clone()
                .add_filter("PCBCraft document", &["json"])
                .set_file_name(&doc.model.name)
                .save_file()
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
        match command {
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
                    .add_filter("PCBCraft document", &["json"])
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
                    self.documents[i].view.camera.fitted = false;
                }
            }
            Command::Home => self.active = None,
            Command::ToggleProjects => self.settings.projects = !self.settings.projects,
            Command::ToggleProperties => {
                self.settings.properties = !self.settings.properties;
                self.settings.right_messages = false;
            }
            Command::ToggleMessages => {
                self.settings.messages = !self.settings.messages;
                self.settings.right_messages = true;
            }
            Command::ToggleGrid => {
                self.settings.show_grid = !self.settings.show_grid;
                for doc in &mut self.documents {
                    doc.view.grid = self.settings.show_grid;
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
            .exact_height(23.0)
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
                        ui.add_enabled(false, egui::Button::new("Open Project…"));
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
                            ui.separator();
                            self.action(ui, "Preferences…", "", Command::Preferences, true);
                        });
                    }
                    ui.menu_button("View", |ui| {
                        self.action(ui, "Fit Document", "Home", Command::Fit, has_doc);
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
                            ui.close();
                        }
                        if ui
                            .checkbox(&mut self.settings.messages, "Messages")
                            .clicked()
                        {
                            ui.close();
                        }
                        ui.separator();
                        self.action(ui, "Toggle Grid", "G", Command::ToggleGrid, has_doc);
                        self.action(ui, "Toggle Units", "Q", Command::ToggleUnits, true);
                    });
                    ui.menu_button("Project", |ui| {
                        ui.add_enabled(false, egui::Button::new("Project Options…"));
                    });
                    // The names mirror the observed editor. Unsupported tools are visibly disabled.
                    if has_doc {
                        for label in ["Place", "Design", "Tools", "Route", "Reports"] {
                            ui.menu_button(label, |ui| {
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
                            ctx.memory_mut(|m| *m = egui::Memory::default());
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
                        for (label, command) in [
                            ("Home", Command::Home),
                            ("Preferences", Command::Preferences),
                            ("Save", Command::Save),
                            ("Open", Command::Open),
                        ] {
                            if ui.small_button(label).clicked() {
                                self.command = Some(command);
                            }
                        }
                    });
                });
            });
    }

    fn projects(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("projects_altium_v2")
            .default_width(330.0)
            .width_range(190.0..=500.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Projects");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("×")
                            .on_hover_text("Hide Projects")
                            .clicked()
                        {
                            self.command = Some(Command::ToggleProjects);
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    for (label, command) in [
                        ("New", Command::NewBoard),
                        ("Open", Command::Open),
                        ("Save", Command::Save),
                        ("Preferences", Command::Preferences),
                    ] {
                        if ui.small_button(label).clicked() {
                            self.command = Some(command);
                        }
                    }
                });
                ui.add_space(5.0);
                ui.add_sized(
                    [ui.available_width(), 21.0],
                    egui::TextEdit::singleline(&mut self.search).hint_text("Search"),
                );
                ui.add_space(7.0);
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
                            egui::CollapsingHeader::new("Free Documents")
                                .default_open(true)
                                .show(ui, |ui| {
                                    egui::CollapsingHeader::new("Source Documents")
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            let query = self.search.to_lowercase();
                                            for (index, doc) in self.documents.iter().enumerate() {
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
                                                        .selectable_label(
                                                            self.active == Some(index),
                                                            title,
                                                        )
                                                        .clicked()
                                                    {
                                                        self.active = Some(index);
                                                        self.pad_edit = None;
                                                    }
                                                });
                                            }
                                            if self.documents.is_empty() {
                                                ui.label(
                                                    RichText::new("No documents open").color(MUTED),
                                                );
                                            }
                                        });
                                });
                        });
                });
            });
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.active else {
            ui.label("Select a document to view its properties.");
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            let doc = &self.documents[index];
            let selected = doc
                .view
                .selected
                .and_then(|id| doc.model.pads.iter().find(|p| p.id == id))
                .cloned();
            if let Some(pad) = selected {
                theme::heading(ui, "Pad", &pad.name);
                if self.pad_edit.as_ref().is_none_or(|e| e.id != pad.id) {
                    self.pad_edit = Some(PadEdit {
                        id: pad.id,
                        name: pad.name.clone(),
                        x: pad.position.x as f64 / 1e6,
                        y: pad.position.y as f64 / 1e6,
                        width: pad.size.x as f64 / 1e6,
                        height: pad.size.y as f64 / 1e6,
                    });
                }
                let edit = self.pad_edit.as_mut().unwrap();
                ui.label("Designator");
                ui.text_edit_singleline(&mut edit.name);
                ui.add_space(8.0);
                egui::Grid::new("pad_values")
                    .num_columns(2)
                    .spacing([12.0, 10.0])
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("X (mm)", &mut edit.x),
                            ("Y (mm)", &mut edit.y),
                            ("Width (mm)", &mut edit.width),
                            ("Height (mm)", &mut edit.height),
                        ] {
                            ui.label(label);
                            ui.add(egui::DragValue::new(value).speed(0.1).max_decimals(6));
                            ui.end_row();
                        }
                    });
                ui.add_space(10.0);
                if ui.button("Apply Pad Properties").clicked() {
                    if ![edit.x, edit.y, edit.width, edit.height]
                        .iter()
                        .all(|v| v.is_finite())
                    {
                        self.message(true, "Pad properties must contain finite numbers");
                        return;
                    }
                    let mut next = self.documents[index].model.clone();
                    if let Some(pad) = next.pads.iter_mut().find(|p| p.id == edit.id) {
                        pad.name = edit.name.clone();
                        pad.position = eda_geom::Point {
                            x: (edit.x * 1e6).round() as i64,
                            y: (edit.y * 1e6).round() as i64,
                        };
                        pad.size = eda_geom::Point {
                            x: (edit.width * 1e6).round() as i64,
                            y: (edit.height * 1e6).round() as i64,
                        };
                    }
                    let doc = &mut self.documents[index];
                    if let Err(error) = doc.history.commit(&mut doc.model, next) {
                        self.message(true, error);
                    }
                }
                ui.add_space(8.0);
                ui.label(
                    RichText::new("One Apply = one undo step")
                        .small()
                        .color(MUTED),
                );
            } else {
                self.pad_edit = None;
                theme::heading(
                    ui,
                    if doc.model.kind == DocumentKind::Board {
                        "Board"
                    } else {
                        "Schematic"
                    },
                    "Document properties",
                );
                ui.label(RichText::new(&doc.model.name).color(ACCENT));
                ui.add_space(10.0);
                theme::heading(ui, "Selection", "Click a pad in the workspace");
                ui.label(RichText::new("Nothing selected").color(MUTED));
                theme::heading(ui, "Document Information", "");
                let unit = self.settings.units;
                egui::Grid::new("doc_info")
                    .spacing([16.0, 9.0])
                    .show(ui, |ui| {
                        for (label, value) in [
                            (
                                "Width",
                                format!(
                                    "{:.3} {}",
                                    unit.from_nm(doc.model.size.x as f64),
                                    unit.label()
                                ),
                            ),
                            (
                                "Height",
                                format!(
                                    "{:.3} {}",
                                    unit.from_nm(doc.model.size.y as f64),
                                    unit.label()
                                ),
                            ),
                            ("Pads", doc.model.pads.len().to_string()),
                            ("Tracks", doc.model.tracks.len().to_string()),
                            ("Revision", doc.history.revision.to_string()),
                        ] {
                            ui.label(RichText::new(label).color(MUTED));
                            ui.label(value);
                            ui.end_row();
                        }
                    });
            }
            ui.add_space(15.0);
            theme::heading(ui, "View Configuration", "");
            if self.documents[index].model.kind == DocumentKind::Board {
                ui.label("Visible layers");
                for (i, layer) in Layer::ALL.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(
                            &mut self.documents[index].view.visible[i],
                            RichText::new("■").color(layer_color(*layer)),
                        );
                        ui.label(layer.label());
                    });
                }
            }
            ui.add_space(12.0);
            if ui.button("Fit Document").clicked() {
                self.command = Some(Command::Fit);
            }
            if ui.button("Preferences…").clicked() {
                self.command = Some(Command::Preferences);
            }
        });
    }

    fn status(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(23.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(RichText::new("●").color(Color32::from_rgb(124, 180, 146)));
                    ui.label("Ready");
                    ui.separator();
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
                    ui.label(RichText::new(coordinates).monospace().size(11.0));
                    ui.separator();
                    if ui.small_button(format!("Units: {}", u.label())).clicked() {
                        self.command = Some(Command::ToggleUnits);
                    }
                    ui.label(
                        RichText::new(format!("Grid: {} mm", self.settings.grid_nm as f64 / 1e6))
                            .small()
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("Panels", |ui| {
                            self.action(ui, "Projects", "", Command::ToggleProjects, true);
                            self.action(ui, "Properties", "", Command::ToggleProperties, true);
                            self.action(ui, "Messages", "", Command::ToggleMessages, true);
                        });
                        if ui
                            .selectable_label(self.settings.messages, "Messages")
                            .clicked()
                        {
                            self.command = Some(Command::ToggleMessages);
                        }
                        ui.label(RichText::new("2D Layout").small().color(MUTED));
                    });
                });
            });
    }

    fn right_dock(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("right_dock_altium_v2")
            .default_width(316.0)
            .width_range(230.0..=500.0)
            .resizable(true)
            .show(ctx, |ui| {
                let messages = self.settings.messages
                    && (self.settings.right_messages || !self.settings.properties);
                ui.horizontal(|ui| {
                    ui.strong(if messages { "Messages" } else { "Properties" });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("×").clicked() {
                            self.command = Some(if messages {
                                Command::ToggleMessages
                            } else {
                                Command::ToggleProperties
                            });
                        }
                    });
                });
                ui.separator();
                egui::TopBottomPanel::bottom("right_dock_tabs")
                    .exact_height(21.0)
                    .show_inside(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.selectable_label(messages, "Messages").clicked() {
                                self.settings.messages = true;
                                self.settings.right_messages = true;
                            }
                            if ui.selectable_label(!messages, "Properties").clicked() {
                                self.settings.properties = true;
                                self.settings.right_messages = false;
                            }
                        });
                    });
                if messages {
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
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(109, 106, 105)))
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
                    ui.horizontal(|ui| {
                        ui.add_space(((ui.available_width() - 260.0) / 2.0).max(0.0));
                        egui::Frame::NONE
                            .fill(theme::PANEL)
                            .inner_margin(3)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.selectable_label(true, "↖").on_hover_text("Select");
                                    if ui.small_button("Fit").clicked() {
                                        self.command = Some(Command::Fit);
                                    }
                                    if ui
                                        .selectable_label(self.settings.show_grid, "Grid")
                                        .clicked()
                                    {
                                        self.command = Some(Command::ToggleGrid);
                                    }
                                    for label in ["Track", "Pad", "Via"] {
                                        ui.add_enabled(false, egui::Button::new(label));
                                    }
                                });
                            });
                    });
                    if self.documents[index].model.kind == DocumentKind::Board {
                        egui::TopBottomPanel::bottom("layers")
                            .exact_height(22.0)
                            .show_inside(ui, |ui| {
                                ui.horizontal_centered(|ui| {
                                    ui.label(RichText::new("Layers").small().color(MUTED));
                                    ui.separator();
                                    for layer in Layer::ALL {
                                        let active =
                                            self.documents[index].view.active_layer == layer;
                                        if ui
                                            .selectable_label(
                                                active,
                                                RichText::new(format!("■  {}", layer.label()))
                                                    .color(layer_color(layer)),
                                            )
                                            .clicked()
                                        {
                                            self.documents[index].view.active_layer = layer;
                                        }
                                    }
                                });
                            });
                    }
                    let doc = &mut self.documents[index];
                    let output = eda_render::canvas(ui, &doc.model, &mut doc.view);
                    self.cursor_mm = output.cursor_mm;
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
            for doc in &mut self.documents {
                doc.view.grid = self.settings.show_grid;
                doc.view.grid_nm = self.settings.grid_nm;
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
                    ui.label("Current milestone: main window and document foundation.");
                    ui.label("Routing, ERC/DRC and Altium file import are not implemented.");
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
        self.menus(ctx);
        self.status(ctx);
        if self.settings.projects {
            self.projects(ctx);
        }
        if self.settings.properties || self.settings.messages {
            self.right_dock(ctx);
        }
        self.workspace(ctx);
        self.dialogs(ctx);
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
}
