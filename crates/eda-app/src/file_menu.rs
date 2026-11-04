//! File commands and the observed AD 24.4.1 schematic menu.
use super::*;
use eda_io::project_files as files;
use theme::Icon;

#[derive(Clone, Copy, Debug)]
pub(super) enum Action {
    NewProject,
    NewGroup,
    OpenGroup,
    SaveCopy,
    SaveProjectAs,
    SaveGroupAs,
    ImportDxf,
    ExportDxf,
    ExportNetlist,
    ImportWizard,
    RunScript,
    Print,
    SmartPdf,
}

#[derive(Default)]
pub(super) struct State {
    pub validation: super::schematic_validation::State,
    pub group: Option<PathBuf>,
    pub selected_project: Option<PathBuf>,
    pub project_for_new: Option<PathBuf>,
    pub saved_projects: Vec<PathBuf>,
    pub output: Option<super::schematic_output::Dialog>,
    pub netlist: Option<super::schematic_netlist::Dialog>,
    wizard: Option<ImportWizard>,
    script: Option<ScriptDialog>,
}
#[derive(Default)]
struct ImportWizard {
    step: usize,
    paths: Vec<PathBuf>,
    error: String,
}
#[derive(Default)]
struct ScriptDialog {
    path: Option<PathBuf>,
    script: Option<eda_core::schematic_script::Script>,
    error: String,
}

impl State {
    pub fn modal(&self) -> bool {
        self.output.is_some()
            || self.netlist.is_some()
            || self.wizard.is_some()
            || self.script.is_some()
    }
    pub fn group_name(&self) -> String {
        self.group.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "Project Group 1.DsnWrk".into(),
            |n| n.to_string_lossy().into_owned(),
        )
    }
}

pub(super) fn row(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    icon: Option<Icon>,
    enabled: bool,
    arrow: bool,
) -> egui::Response {
    row_with_width(ui, label, shortcut, icon, enabled, arrow, 174.)
}
pub(super) fn row_with_width(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    icon: Option<Icon>,
    enabled: bool,
    arrow: bool,
    minimum_width: f32,
) -> egui::Response {
    let font = egui::FontId::proportional(12.);
    let label_width = ui
        .painter()
        .layout_no_wrap(label.into(), font.clone(), Color32::WHITE)
        .size()
        .x;
    let shortcut_width = ui
        .painter()
        .layout_no_wrap(shortcut.into(), font, Color32::WHITE)
        .size()
        .x;
    let width =
        (27. + label_width + 14. + shortcut_width + if shortcut.is_empty() { 0. } else { 20. })
            .max(minimum_width);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 22.),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    if response.hovered() && enabled {
        ui.painter().rect_filled(rect, 0., theme::ACCENT);
    }
    let color = if enabled {
        Color32::from_gray(225)
    } else {
        Color32::from_gray(128)
    };
    ui.painter().text(
        rect.left_center() + egui::vec2(27., 0.),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.),
        color,
    );
    ui.painter().text(
        rect.right_center() - egui::vec2(14., 0.),
        egui::Align2::RIGHT_CENTER,
        shortcut,
        egui::FontId::proportional(12.),
        color,
    );
    if let Some(icon) = icon {
        theme::draw_icon(
            ui.painter(),
            egui::Rect::from_center_size(
                rect.left_center() + egui::vec2(10., 0.),
                egui::vec2(19., 18.),
            ),
            icon,
            enabled,
        );
    }
    if arrow {
        let c = rect.right_center() - egui::vec2(6., 0.);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                c + egui::vec2(-2., -4.),
                c + egui::vec2(2., 0.),
                c + egui::vec2(-2., 4.),
            ],
            color,
            egui::Stroke::NONE,
        ));
    }
    response
}
fn submenu<R>(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) {
    let response = row(ui, label, "", None, enabled, true);
    if enabled {
        egui::containers::menu::SubMenu::new().show(ui, &response, |ui| {
            ui.spacing_mut().item_spacing.y = 0.;
            contents(ui)
        });
    }
}
pub(super) fn separator(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(
        egui::vec2(ui.min_rect().width().max(174.), 5.),
        egui::Sense::hover(),
    );
    ui.painter().line_segment(
        [
            r.left_center() + egui::vec2(27., 0.),
            r.right_center() - egui::vec2(6., 0.),
        ],
        egui::Stroke::new(1., Color32::from_gray(45)),
    );
}

impl PcbCraft {
    pub(super) fn group_dirty(&self) -> bool {
        self.projects
            .iter()
            .map(|p| &p.path)
            .ne(self.file_state.saved_projects.iter())
    }
    pub(super) fn save_project_group(&mut self, save_as: bool) -> bool {
        let path = if save_as || self.file_state.group.is_none() {
            self.file_dialog
                .clone()
                .add_filter("Design Project Group", &["DsnWrk"])
                .set_file_name(self.file_state.group_name())
                .save_file()
        } else {
            self.file_state.group.clone()
        };
        let Some(path) = path else { return false };
        match files::save_group(&path, &self.projects) {
            Ok(()) => {
                self.file_state.group = Some(path.clone());
                self.file_state.saved_projects =
                    self.projects.iter().map(|p| p.path.clone()).collect();
                self.recent(path);
                true
            }
            Err(e) => {
                self.message(true, e);
                false
            }
        }
    }
    fn confirm_group_replacement(&mut self) -> bool {
        if self.variants_dirty() {
            match rfd::MessageDialog::new()
                .set_title("Save project variants?")
                .set_description(
                    "Modified assembly variants will be closed with the project group.",
                )
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show()
            {
                rfd::MessageDialogResult::Yes => {
                    if !self.save_variants() {
                        return false;
                    }
                }
                rfd::MessageDialogResult::No => {}
                _ => return false,
            }
        }
        if !self.group_dirty() {
            return true;
        }
        match rfd::MessageDialog::new()
            .set_title("Save changes?")
            .set_description(format!("Save changes to {}?", self.file_state.group_name()))
            .set_buttons(rfd::MessageButtons::YesNoCancel)
            .show()
        {
            rfd::MessageDialogResult::Yes => self.save_project_group(false),
            rfd::MessageDialogResult::No => true,
            _ => false,
        }
    }
    fn file_item(
        &mut self,
        ui: &mut egui::Ui,
        label: &str,
        shortcut: &str,
        icon: Option<Icon>,
        command: Option<Command>,
        enabled: bool,
    ) {
        if row(
            ui,
            label,
            shortcut,
            icon,
            enabled && command.is_some(),
            false,
        )
        .clicked()
            && let Some(command) = command
        {
            self.command = Some(command);
            ui.close();
        }
    }
    pub(super) fn file_menu(&mut self, ui: &mut egui::Ui) {
        use Action::*;
        ui.set_min_width(174.);
        ui.spacing_mut().item_spacing.y = 0.;
        let has_doc = self.active.is_some();
        let sch = self
            .active
            .is_some_and(|i| self.documents[i].model.schematic.is_some());
        submenu(ui, "New", true, |ui| {
            self.file_item(
                ui,
                "Project…",
                "",
                None,
                Some(Command::File(NewProject)),
                true,
            );
            separator(ui);
            self.file_item(
                ui,
                "Schematic",
                "",
                Some(Icon::New),
                Some(Command::NewSchematic),
                true,
            );
            self.file_item(
                ui,
                "PCB",
                "",
                Some(Icon::Chip),
                Some(Command::NewBoard),
                true,
            );
            for label in [
                "Draftsman Document",
                "CAM Document",
                "Output Job File",
                "Component…",
            ] {
                self.file_item(ui, label, "", None, None, false);
            }
            separator(ui);
            self.file_item(
                ui,
                "Library…",
                "",
                Some(Icon::Folder),
                Some(Command::Library(library_editor::Action::New)),
                true,
            );
            submenu(ui, "Script", true, |ui| {
                for label in ["Script Project", "Script Unit", "Script Form"] {
                    self.file_item(ui, label, "", None, None, false);
                }
            });
            submenu(ui, "Mixed-Signal Simulation", true, |ui| {
                for label in [
                    "AdvancedSim Model",
                    "AdvancedSim Netlist",
                    "AdvancedSim Sub-Circuit",
                ] {
                    self.file_item(ui, label, "", None, None, false);
                }
            });
            separator(ui);
            self.file_item(
                ui,
                "Design Project Group",
                "",
                Some(Icon::Folder),
                Some(Command::File(NewGroup)),
                true,
            );
        });
        self.file_item(
            ui,
            "Open…",
            "Ctrl+O",
            Some(Icon::Open),
            Some(Command::Open),
            true,
        );
        self.file_item(ui, "Close", "Ctrl+F4", None, Some(Command::Close), has_doc);
        separator(ui);
        self.file_item(
            ui,
            "Open Project…",
            "",
            Some(Icon::Open),
            Some(Command::OpenProject),
            true,
        );
        self.file_item(
            ui,
            "Open Project Group…",
            "",
            None,
            Some(Command::File(OpenGroup)),
            true,
        );
        separator(ui);
        self.file_item(
            ui,
            "Save",
            "Ctrl+S",
            Some(Icon::Save),
            Some(Command::Save),
            has_doc,
        );
        self.file_item(ui, "Save As…", "", None, Some(Command::SaveAs), has_doc);
        self.file_item(
            ui,
            "Save Copy As…",
            "",
            None,
            Some(Command::File(SaveCopy)),
            has_doc,
        );
        self.file_item(
            ui,
            "Save All",
            "",
            None,
            Some(Command::SaveAll),
            has_doc || !self.projects.is_empty(),
        );
        separator(ui);
        self.file_item(
            ui,
            "Save Project As…",
            "",
            None,
            Some(Command::File(SaveProjectAs)),
            self.file_project_index().is_some(),
        );
        self.file_item(
            ui,
            "Save Project Group As…",
            "",
            None,
            Some(Command::File(SaveGroupAs)),
            true,
        );
        separator(ui);
        submenu(ui, "Import", sch, |ui| {
            self.file_item(
                ui,
                "DXF/DWG",
                "",
                None,
                Some(Command::File(ImportDxf)),
                true,
            );
            self.file_item(ui, "Protel DOS Schematic", "", None, None, false);
            separator(ui);
            self.file_item(ui, "Add Importer…", "", None, None, false);
        });
        submenu(ui, "Export", sch, |ui| {
            self.file_item(
                ui,
                "DXF/DWG",
                "",
                None,
                Some(Command::File(ExportDxf)),
                true,
            );
            self.file_item(
                ui,
                "Netlist Schematic",
                "",
                None,
                Some(Command::File(ExportNetlist)),
                true,
            );
            for label in [
                "Orcad SDT Schematic",
                "Orcad v7 Capture Design",
                "P-CAD V16 Schematic Design",
            ] {
                self.file_item(ui, label, "", None, None, false);
            }
            separator(ui);
            self.file_item(ui, "Add Exporter…", "", None, None, false);
        });
        self.file_item(
            ui,
            "Import Wizard",
            "",
            None,
            Some(Command::File(ImportWizard)),
            true,
        );
        self.file_item(
            ui,
            "Run Script…",
            "",
            None,
            Some(Command::File(RunScript)),
            sch,
        );
        separator(ui);
        self.file_item(
            ui,
            "Print…",
            "Ctrl+P",
            Some(Icon::Print),
            Some(Command::File(Print)),
            sch,
        );
        separator(ui);
        self.file_item(
            ui,
            "Smart PDF…",
            "",
            Some(Icon::Pdf),
            Some(Command::File(SmartPdf)),
            sch,
        );
        separator(ui);
        self.file_recent(ui, "Recent Documents", self.settings.recent.clone());
        self.file_recent(ui, "Recent Projects", self.settings.recent_projects.clone());
        self.file_recent(
            ui,
            "Recent Project Groups",
            self.settings.recent_groups.clone(),
        );
        separator(ui);
        self.file_item(ui, "Exit", "Alt+F4", None, Some(Command::Quit), true);
    }
    fn file_recent(&mut self, ui: &mut egui::Ui, label: &str, paths: Vec<PathBuf>) {
        submenu(ui, label, true, |ui| {
            if paths.is_empty() {
                row(ui, "(No recent files)", "", None, false, false);
            }
            for (n, path) in paths.iter().enumerate() {
                let label = format!(
                    "{} {}",
                    n + 1,
                    path.file_name().unwrap_or_default().to_string_lossy()
                );
                if ui
                    .button(label)
                    .on_hover_text(path.display().to_string())
                    .clicked()
                {
                    self.request_open_path(path.clone());
                    ui.close();
                }
            }
        });
    }
    pub(super) fn file_project_index(&self) -> Option<usize> {
        self.active
            .and_then(|i| {
                let doc = &self.documents[i];
                self.projects.iter().position(|p| {
                    doc.new_project.as_ref() == Some(&p.path)
                        || p.documents.iter().any(|e| {
                            doc.path.as_ref() == Some(&e.path)
                                || doc.source_path.as_ref() == Some(&e.path)
                        })
                })
            })
            .or_else(|| {
                if self.active.is_some() {
                    return None;
                }
                self.file_state
                    .selected_project
                    .as_ref()
                    .and_then(|path| self.projects.iter().position(|p| &p.path == path))
            })
    }
    pub(super) fn open_group(&mut self, path: PathBuf) {
        // Validate the complete group before changing the workspace.
        let result = files::load_group(&path).and_then(|paths| {
            paths
                .iter()
                .map(|p| eda_io::altium::load_project(p))
                .collect::<Result<Vec<_>, _>>()
        });
        match result {
            Ok(projects) => {
                if !self.confirm_group_replacement() {
                    return;
                }
                self.projects = projects;
                self.file_state.saved_projects =
                    self.projects.iter().map(|p| p.path.clone()).collect();
                self.file_state.group = Some(path.clone());
                self.file_state.selected_project = self.projects.first().map(|p| p.path.clone());
                self.recent(path.clone());
                self.settings.projects = true;
                let first = self
                    .projects
                    .iter()
                    .flat_map(|p| &p.documents)
                    .find(|e| e.exists && e.kind == "schdoc")
                    .map(|e| e.path.clone());
                if let Some(first) = first {
                    self.request_open_path(first);
                }
                self.message(false, format!("Opened project group {}", path.display()));
            }
            Err(e) => self.message(true, format!("Could not open project group: {e}")),
        }
    }
    pub(super) fn file_command(&mut self, action: Action, ctx: &egui::Context) {
        use Action::*;
        match action {
            NewProject => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .set_title("Create Project")
                    .add_filter("PCB Project", &["PrjPcb"])
                    .set_file_name("PCB Project1.PrjPcb")
                    .save_file()
                {
                    match files::create_project(&path) {
                        Ok(p) => {
                            self.file_state.selected_project = Some(p.path.clone());
                            self.projects.push(p);
                            self.recent(path);
                            self.settings.projects = true;
                        }
                        Err(e) => self.message(true, e),
                    }
                }
            }
            NewGroup => {
                if !self.confirm_group_replacement() {
                    return;
                }
                self.projects.clear();
                self.file_state.saved_projects.clear();
                self.file_state.group = None;
                self.file_state.selected_project = None;
                self.settings.projects = true;
            }
            OpenGroup => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .add_filter("Design Project Group", &["DsnWrk"])
                    .pick_file()
                {
                    self.open_group(path);
                }
            }
            SaveCopy => {
                if let Some(i) = self.active {
                    self.save_copy_index(i);
                }
            }
            SaveProjectAs => {
                if let Some(i) = self.file_project_index()
                    && let Some(path) = self
                        .file_dialog
                        .clone()
                        .add_filter("PCB Project", &["PrjPcb"])
                        .set_file_name(&self.projects[i].name)
                        .save_file()
                {
                    match files::save_project_as(&self.projects[i], &path) {
                        Ok(p) => {
                            let old = self.projects[i].path.clone();
                            for doc in &mut self.documents {
                                if doc.new_project.as_ref() == Some(&old) {
                                    doc.new_project = Some(p.path.clone());
                                }
                            }
                            self.file_state.selected_project = Some(p.path.clone());
                            self.projects[i] = p;
                            self.variants.project_updated(&self.projects[i]);
                            self.recent(path);
                        }
                        Err(e) => self.message(true, e),
                    }
                }
            }
            SaveGroupAs => {
                self.save_project_group(true);
            }
            ImportDxf => {
                if let Some(path) = self
                    .file_dialog
                    .clone()
                    .add_filter("AutoCAD ASCII DXF", &["dxf"])
                    .pick_file()
                {
                    self.import_dxf(&path);
                }
            }
            ExportDxf => {
                if let Some(i) = self.active
                    && let Some(path) = self
                        .file_dialog
                        .clone()
                        .add_filter("AutoCAD ASCII DXF", &["dxf"])
                        .set_file_name("Schematic.dxf")
                        .save_file()
                {
                    match super::schematic_netlist::export_destination(&path, "dxf")
                        .and_then(|()| {
                            let ctx = egui::Context::default();
                            theme::apply(&ctx);
                            eda_render::schematic_vector::drawing(&ctx, &self.documents[i].model)
                        })
                        .and_then(|drawing| eda_io::schematic_dxf::export_drawing(&drawing))
                        .and_then(|data| files::write_atomic(&path, data.as_bytes()))
                    {
                        Ok(()) => self.message(false, format!("Exported {}", path.display())),
                        Err(e) => self.message(true, e),
                    }
                }
            }
            ExportNetlist => self.open_netlist_export(),
            ImportWizard => self.file_state.wizard = Some(Default::default()),
            RunScript => self.file_state.script = Some(ScriptDialog::default()),
            Print | SmartPdf => self.open_schematic_output(matches!(action, SmartPdf), ctx),
        }
    }
    fn import_dxf(&mut self, path: &std::path::Path) {
        let Some(i) = self.active else { return };
        let result = eda_io::schematic_dxf::import(path).and_then(|fragment| {
            let d = &mut self.documents[i];
            d.edit_schematic(
                eda_core::schematic_edit::Edit::Insert(fragment),
                d.history.revision,
            )
            .map(|_| ())
        });
        match result {
            Ok(()) => self.message(false, format!("Imported {}", path.display())),
            Err(e) => self.message(true, e),
        }
    }
    pub(super) fn file_dialogs(&mut self, ctx: &egui::Context) {
        self.schematic_output_dialog(ctx);
        self.netlist_export_dialog(ctx);
        if let Some(mut wizard) = self.file_state.wizard.take() {
            let mut keep = true;
            let mut import = false;
            egui::Modal::new(egui::Id::new("file_import_wizard")).show(ctx, |ui| {
                ui.set_min_size(egui::vec2(650., 420.));
                ui.heading("Import Wizard");
                ui.separator();
                if wizard.step == 0 {
                    ui.heading("Welcome to the Import Wizard");
                    ui.label("Import design documents and libraries into PCBCraft.");
                    ui.add_space(25.);
                    ui.label("Select Type of Files to Import");
                    ui.label("Altium Designer: SchDoc, PcbDoc, PrjPcb, SchLib, PcbLib, IntLib");
                    ui.label("AutoCAD ASCII DXF → active schematic");
                } else {
                    ui.label("Select Files to Import");
                    if ui.button("Add…").clicked()
                        && let Some(paths) = self
                            .file_dialog
                            .clone()
                            .add_filter(
                                "Design documents",
                                &[
                                    "SchDoc", "PcbDoc", "PrjPcb", "SchLib", "PcbLib", "IntLib",
                                    "json", "dxf",
                                ],
                            )
                            .pick_files()
                    {
                        for p in paths {
                            if !wizard.paths.contains(&p) {
                                wizard.paths.push(p);
                            }
                        }
                    }
                    egui::ScrollArea::vertical()
                        .max_height(240.)
                        .show(ui, |ui| {
                            for p in &wizard.paths {
                                ui.label(p.display().to_string());
                            }
                        });
                    if ui.button("Clear List").clicked() {
                        wizard.paths.clear();
                    }
                }
                ui.colored_label(Color32::LIGHT_RED, &wizard.error);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space((ui.available_width() - 210.).max(0.));
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                        if ui
                            .add_enabled(wizard.step > 0, egui::Button::new("Back"))
                            .clicked()
                        {
                            wizard.step -= 1;
                        }
                        if ui
                            .add_enabled(wizard.step == 0, egui::Button::new("Next"))
                            .clicked()
                        {
                            wizard.step += 1;
                        }
                        if ui
                            .add_enabled(
                                wizard.step > 0 && !wizard.paths.is_empty(),
                                egui::Button::new("Finish"),
                            )
                            .clicked()
                        {
                            import = true;
                        }
                    });
                });
            });
            if import {
                if wizard
                    .paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dxf")))
                    && self
                        .active
                        .is_none_or(|i| self.documents[i].model.schematic.is_none())
                {
                    wizard.error = "Open a schematic before importing DXF".into();
                } else {
                    for path in &wizard.paths {
                        if path
                            .extension()
                            .is_some_and(|e| e.eq_ignore_ascii_case("dxf"))
                        {
                            self.import_dxf(path);
                        } else {
                            self.request_open_path(path.clone());
                        }
                    }
                    keep = false;
                }
            }
            if !egui::Popup::is_any_open(ctx)
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
            {
                keep = false;
            }
            if keep {
                self.file_state.wizard = Some(wizard);
            }
        }
        if let Some(mut dialog) = self.file_state.script.take() {
            let mut keep = true;
            let mut run = false;
            egui::Modal::new(egui::Id::new("run_script")).show(ctx, |ui| {
                ui.set_width(570.);
                ui.heading("Run Script");
                ui.separator();
                ui.label("PCBCraft schematic command scripts (*.pcbscript.json)");
                if ui.button("Browse…").clicked()
                    && let Some(path) = self
                        .file_dialog
                        .clone()
                        .add_filter("PCBCraft command script", &["json"])
                        .pick_file()
                {
                    dialog.path = Some(path.clone());
                    dialog.script = None;
                    match eda_io::altium::read_bounded(&path)
                        .and_then(|bytes| eda_core::schematic_script::Script::parse(&bytes))
                    {
                        Ok(script) => {
                            dialog.script = Some(script);
                            dialog.error.clear();
                        }
                        Err(e) => dialog.error = e,
                    }
                }
                if let Some(path) = &dialog.path {
                    ui.label(path.display().to_string());
                }
                if let Some(script) = &dialog.script {
                    ui.label(format!("{} — {} commands", script.name, script.edits.len()));
                    ui.label("The commands are validated together and create one Undo step.");
                }
                ui.colored_label(Color32::LIGHT_RED, &dialog.error);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(dialog.script.is_some(), egui::Button::new("Run"))
                        .clicked()
                    {
                        run = true;
                    }
                    if ui.button("Close").clicked() {
                        keep = false;
                    }
                });
            });
            if run
                && let Some(i) = self.active
                && let Some(script) = &dialog.script
            {
                let d = &mut self.documents[i];
                match script.execute(&mut d.model, &mut d.history) {
                    Ok(_) => {
                        d.content_dirty.set(None);
                        self.message(false, format!("Script '{}' completed", script.name));
                        keep = false;
                    }
                    Err(e) => dialog.error = e,
                }
            }
            if !egui::Popup::is_any_open(ctx)
                && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
            {
                keep = false;
            }
            if keep {
                self.file_state.script = Some(dialog);
            }
        }
    }
}
