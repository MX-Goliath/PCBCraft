//! Project / Validate and the compiler Messages panel observed in AD 24.4.1.
use super::*;
use eda_core::schematic_validation::{self as erc, Severity, Target};
use std::collections::BTreeMap;

fn project_row(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    icon: Option<theme::Icon>,
    enabled: bool,
    arrow: bool,
) -> egui::Response {
    file_menu::row_with_width(ui, label, shortcut, icon, enabled, arrow, 297.)
}

struct Row {
    severity: Option<Severity>,
    document: String,
    message: String,
    targets: Vec<Target>,
    source: &'static str,
    stamp: (String, String),
    number: usize,
}
pub(super) struct State {
    rows: Vec<Row>,
    paths: BTreeMap<Uuid, PathBuf>,
    selected: Option<usize>,
    time: String,
    date: String,
    has_run: bool,
    widths: [f32; 7],
    selection: std::collections::BTreeSet<usize>,
    sort: Option<(usize, bool)>,
    group: Option<usize>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            rows: vec![],
            paths: BTreeMap::new(),
            selected: None,
            time: String::new(),
            date: String::new(),
            has_run: false,
            widths: [39., 44., 27., 100., 26., 26., 25.],
            selection: Default::default(),
            sort: None,
            group: None,
        }
    }
}
impl State {
    pub(super) fn application_message(&mut self, error: bool, text: &str) {
        if !self.has_run {
            return;
        }
        let now = chrono::Local::now();
        self.rows.push(Row {
            severity: error.then_some(Severity::Error),
            document: String::new(),
            message: text.into(),
            targets: vec![],
            source: "WorkspaceManager",
            stamp: (
                now.format("%H:%M:%S").to_string(),
                now.format("%d.%m.%Y").to_string(),
            ),
            number: self.rows.last().map_or(1, |r| r.number + 1),
        });
    }
    fn value(&self, index: usize, column: usize) -> String {
        let row = &self.rows[index];
        match column {
            0 => row.severity.map_or("Info", Severity::label).into(),
            1 => row.document.clone(),
            2 => row.source.into(),
            3 => row.message.clone(),
            4 => row.stamp.0.clone(),
            5 => row.stamp.1.clone(),
            _ => row.number.to_string(),
        }
    }
    fn text(&self, selection: bool) -> String {
        let mut text = String::from("Class\tDocument\tSource\tMessage\tTime\tDate\tNo.\r\n");
        for i in 0..self.rows.len() {
            if !selection || self.selection.contains(&i) {
                text.push_str(
                    &(0..7)
                        .map(|c| self.value(i, c))
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
                text.push_str("\r\n");
            }
        }
        text
    }
}
impl PcbCraft {
    pub(super) fn project_menu(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 0.;
        // AD popup is 305 pixels at the reference desktop scale.
        ui.set_min_width(297.);
        let project = self.file_project_index();
        let label = project.map_or_else(
            || "Validate".into(),
            |i| format!("Validate PCB Project {}", self.projects[i].name),
        );
        let can_validate = project.is_some()
            || self
                .active
                .is_some_and(|i| self.documents[i].model.schematic.is_some());
        let mut row = |ui: &mut egui::Ui, name: &str, key: &str, icon, command, enabled| {
            if project_row(ui, name, key, icon, enabled, false).clicked() {
                self.command = command;
                ui.close();
            }
        };
        row(
            ui,
            &label,
            "",
            Some(theme::Icon::Validate),
            Some(Command::ValidateProject),
            can_validate,
        );
        let navigable = self
            .file_state
            .validation
            .rows
            .iter()
            .any(|r| !r.targets.is_empty());
        row(
            ui,
            "Cross Probe Next Message",
            "Shift+Ctrl+F3",
            None,
            Some(Command::ValidationMessage(true)),
            navigable,
        );
        row(
            ui,
            "Cross Probe Previous Message",
            "Shift+Ctrl+F4",
            None,
            Some(Command::ValidationMessage(false)),
            navigable,
        );
        file_menu::separator(ui);
        project_row(ui, "Add New to Project", "", None, false, true);
        project_row(
            ui,
            "Add Existing to Project…",
            "",
            Some(theme::Icon::New),
            false,
            false,
        );
        project_row(
            ui,
            "Remove from Project…",
            "",
            Some(theme::Icon::Folder),
            false,
            false,
        );
        file_menu::separator(ui);
        project_row(ui, "Project Documents…", "Ctrl+Alt+O", None, false, false);
        project_row(ui, "Close Project Documents", "", None, false, false);
        if project.is_some() {
            project_row(ui, "Close Project", "", None, false, false);
        }
        file_menu::separator(ui);
        if project_row(
            ui,
            "Show Differences…",
            "",
            Some(theme::Icon::New),
            self.sync_project_name().is_some(),
            false,
        )
        .clicked()
        {
            self.command = Some(Command::Synchronize(
                eda_core::design_sync::Direction::ToPcb,
                true,
            ));
            ui.close();
        }
        project_row(
            ui,
            "Show Physical Differences…",
            "",
            Some(theme::Icon::New),
            false,
            false,
        );
        if project.is_some() {
            project_row(ui, "Component Links…", "", None, false, false);
            if project_row(ui, "Variants…", "", None, true, false).clicked() {
                self.command = Some(Command::Variants);
                ui.close();
            }
        }
        file_menu::separator(ui);
        project_row(
            ui,
            "History & Version Control",
            "",
            Some(theme::Icon::Home),
            false,
            true,
        );
        file_menu::separator(ui);
        project_row(
            ui,
            "Project Packager…",
            "",
            Some(theme::Icon::LibraryComponent),
            false,
            false,
        );
        if project.is_some() {
            project_row(ui, "Project Releaser…", "", None, false, false);
            file_menu::separator(ui);
            project_row(ui, "Share…", "", None, false, false);
            file_menu::separator(ui);
        }
        project_row(
            ui,
            "Project Options…",
            "",
            Some(theme::Icon::Settings),
            false,
            false,
        );
    }
    pub(super) fn validate_schematic_project(&mut self) {
        let project = self.file_project_index();
        let name = project
            .map_or_else(
                || {
                    self.active
                        .map_or("", |i| self.documents[i].model.name.as_str())
                },
                |i| self.projects[i].name.as_str(),
            )
            .to_owned();
        let result = (|| {
            let snapshots = if project.is_some() {
                self.schematic_project_snapshots()?
            } else {
                let d = self
                    .documents
                    .get(self.active.ok_or("No active schematic")?)
                    .ok_or("No active schematic")?;
                vec![(
                    d.path.clone().or(d.source_path.clone()).unwrap_or_default(),
                    d.model.clone(),
                )]
            };
            let sheets: Vec<_> = snapshots.iter().map(|(p, d)| (p.as_path(), d)).collect();
            let options = if let Some(i) = project {
                self.projects[i].compile_options(&sheets)?
            } else {
                Default::default()
            };
            let validation = project
                .map(|i| self.projects[i].validation_options.clone())
                .unwrap_or_default();
            let docs: Vec<_> = snapshots.iter().map(|(_, d)| d).collect();
            let report = erc::validate(&docs, &options, &validation);
            Ok::<_, String>((snapshots, report))
        })();
        let state = &mut self.file_state.validation;
        state.has_run = true;
        state.rows.clear();
        state.paths.clear();
        state.selected = None;
        state.selection.clear();
        let now = chrono::Local::now();
        state.time = now.format("%H:%M:%S").to_string();
        state.date = now.format("%d.%m.%Y").to_string();
        match result {
            Ok((snapshots, report)) => {
                let has_errors = report
                    .diagnostics
                    .iter()
                    .any(|d| d.severity >= Severity::Error);
                let names: BTreeMap<_, _> = snapshots
                    .iter()
                    .map(|(_, d)| (d.id, d.name.clone()))
                    .collect();
                state.paths = snapshots
                    .iter()
                    .filter(|(p, _)| !p.as_os_str().is_empty())
                    .map(|(p, d)| (d.id, p.clone()))
                    .collect();
                for d in report.diagnostics {
                    state.rows.push(Row {
                        severity: Some(d.severity),
                        document: d
                            .targets
                            .first()
                            .and_then(|t| names.get(&t.document))
                            .cloned()
                            .unwrap_or_else(|| name.clone()),
                        message: d.message,
                        targets: d.targets,
                        source: "Compiler",
                        stamp: (state.time.clone(), state.date.clone()),
                        number: state.rows.len() + 1,
                    });
                }
                state.rows.push(Row {severity:Some(Severity::Warning), document:name.clone(),message:"Validation coverage is limited: advanced ERC rules and remaining native reporting levels are not implemented.".into(), targets:vec![],source:"PCBCraft",stamp:(state.time.clone(),state.date.clone()),number:state.rows.len()+1});
                if report.compiled && !has_errors {
                    state.rows.push(Row {
                        severity: None,
                        document: name,
                        message: "Compile successful, no errors found.".into(),
                        targets: vec![],
                        source: "Compiler",
                        stamp: (state.time.clone(), state.date.clone()),
                        number: state.rows.len() + 1,
                    });
                }
            }
            Err(error) => state.rows.push(Row {
                severity: Some(Severity::Fatal),
                document: name,
                message: error,
                targets: vec![],
                source: "Compiler",
                stamp: (state.time.clone(), state.date.clone()),
                number: state.rows.len() + 1,
            }),
        }
        state.selected = (!state.rows.is_empty()).then_some(0);
        state.selection.extend(state.selected);
        // The observed warning-only compile leaves Properties active.
        if state
            .rows
            .iter()
            .any(|r| matches!(r.severity, Some(Severity::Error | Severity::Fatal)))
        {
            self.settings.messages = true;
            self.settings.right_messages = true;
            self.settings.right_components = false;
        }
    }
    pub(super) fn next_validation_message(&mut self, next: bool) {
        let state = &mut self.file_state.validation;
        let indices: Vec<_> = state
            .rows
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.targets.is_empty())
            .map(|(i, _)| i)
            .collect();
        if indices.is_empty() {
            return;
        }
        let selected = state
            .selected
            .and_then(|s| indices.iter().position(|i| *i == s));
        let index = selected.map_or(if next { 0 } else { indices.len() - 1 }, |i| {
            if next {
                (i + 1) % indices.len()
            } else {
                (i + indices.len() - 1) % indices.len()
            }
        });
        state.selected = Some(indices[index]);
        state.selection.clear();
        state.selection.insert(indices[index]);
        let target = state.rows[indices[index]].targets[0].clone();
        self.probe_validation_target(target);
    }
    fn probe_validation_target(&mut self, target: Target) {
        let path = self
            .file_state
            .validation
            .paths
            .get(&target.document)
            .cloned();
        let mut index = self.documents.iter().position(|d| {
            d.model.id == target.document
                || path.as_ref().is_some_and(|p| {
                    d.path.as_ref() == Some(p) || d.source_path.as_ref() == Some(p)
                })
        });
        if index.is_none()
            && let Some(path) = &path
        {
            self.open_path(path.clone());
            index = self.documents.iter().position(|d| {
                d.path.as_ref() == Some(path) || d.source_path.as_ref() == Some(path)
            });
        }
        let Some(i) = index else {
            return;
        };
        let doc = &mut self.documents[i];
        let Some(scene) = doc.model.schematic.as_ref() else {
            return;
        };
        let Some(at) = eda_core::schematic_edit::anchor(scene, target.object) else {
            return;
        };
        doc.sch_editor.cancel(&mut doc.view);
        doc.view.selected = Some(target.object);
        doc.view.sch_selection = vec![target.object];
        doc.view.sch_segments.clear();
        doc.view.camera.centre_mm = [at.x as f64 / 1e6, at.y as f64 / 1e6];
        doc.view.camera.pixels_per_mm = doc.view.camera.pixels_per_mm.max(12.);
        doc.view.camera.fitted = true;
        self.active = Some(i);
    }
    pub(super) fn messages_panel(&mut self, ui: &mut egui::Ui) {
        if !self.file_state.validation.has_run {
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
                            RichText::new(if *error { "Error" } else { "Info" }).color(if *error {
                                Color32::LIGHT_RED
                            } else {
                                ACCENT
                            }),
                        );
                        ui.separator();
                        ui.label(text);
                    });
                }
            });
            return;
        }
        let mut probe = None;
        let mut action = None;
        let state = &mut self.file_state.validation;
        let height = ui.available_height();
        egui::TopBottomPanel::bottom("compiler_message_details")
            .default_height(72.)
            .height_range(48. ..=height.max(48.))
            .resizable(true)
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                let (title, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 18.),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(title, 0., Color32::from_gray(65));
                ui.painter().text(
                    title.left_center(),
                    egui::Align2::LEFT_CENTER,
                    "Details",
                    egui::FontId::proportional(12.),
                    Color32::from_gray(225),
                );
                egui::ScrollArea::both()
                    .id_salt("message_details_scroll")
                    .auto_shrink([false, false])
                    .min_scrolled_height(0.)
                    .show(ui, |ui| {
                        if let Some(row) = state.selected.and_then(|i| state.rows.get(i)) {
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(18., 18.),
                                    egui::Sense::hover(),
                                );
                                let painter = ui.painter();
                                painter.circle_filled(
                                    rect.center(),
                                    6.,
                                    Color32::from_rgb(205, 55, 55),
                                );
                                for sign in [-1., 1.] {
                                    painter.line_segment(
                                        [
                                            rect.center() + egui::vec2(-3., sign * 3.),
                                            rect.center() + egui::vec2(3., -sign * 3.),
                                        ],
                                        egui::Stroke::new(1., Color32::WHITE),
                                    );
                                }
                                ui.label(&row.message);
                            });
                            for target in &row.targets {
                                if ui
                                    .selectable_label(false, format!("    {}", target.description))
                                    .double_clicked()
                                {
                                    probe = Some(target.clone());
                                }
                            }
                        }
                    });
            });
        egui::ScrollArea::both()
            .id_salt("compiler_messages_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                let total = state.widths.iter().sum::<f32>().max(ui.available_width());
                let (header, _) =
                    ui.allocate_exact_size(egui::vec2(total, 19.), egui::Sense::hover());
                ui.painter().rect_filled(header, 0., Color32::from_gray(65));
                let mut x = header.left();
                for (i, label) in [
                    "Class", "Document", "Source", "Message", "Time", "Date", "No.",
                ]
                .iter()
                .enumerate()
                {
                    let rect = egui::Rect::from_min_size(
                        egui::pos2(x, header.top()),
                        egui::vec2(state.widths[i], 19.),
                    );
                    ui.painter().with_clip_rect(rect).text(
                        rect.left_center() + egui::vec2(2., 0.),
                        egui::Align2::LEFT_CENTER,
                        *label,
                        egui::FontId::proportional(12.),
                        Color32::from_gray(225),
                    );
                    ui.painter().vline(
                        rect.right(),
                        rect.y_range(),
                        egui::Stroke::new(1., Color32::from_gray(45)),
                    );
                    let drag = ui.interact(
                        egui::Rect::from_center_size(rect.right_center(), egui::vec2(5., 19.)),
                        ui.id().with(("message_column", i)),
                        egui::Sense::drag(),
                    );
                    if drag.dragged() {
                        state.widths[i] =
                            (state.widths[i] + ui.input(|i| i.pointer.delta().x)).max(24.);
                    }
                    drag.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                    let heading = ui.interact(
                        rect.shrink2(egui::vec2(3., 0.)),
                        ui.id().with(("message_sort", i)),
                        egui::Sense::click(),
                    );
                    if heading.clicked() {
                        state.sort = Some((i, !matches!(state.sort,Some((c,true)) if c==i)));
                    }
                    x = rect.right();
                }
                let mut indices: Vec<_> = (0..state.rows.len()).collect();
                indices.sort_by(|a, b| {
                    let group = state.group.map_or(std::cmp::Ordering::Equal, |c| {
                        state.value(*a, c).cmp(&state.value(*b, c))
                    });
                    group
                        .then_with(|| {
                            state
                                .sort
                                .map_or(std::cmp::Ordering::Equal, |(c, ascending)| {
                                    let order = if c == 6 {
                                        a.cmp(b)
                                    } else {
                                        state.value(*a, c).cmp(&state.value(*b, c))
                                    };
                                    if ascending { order } else { order.reverse() }
                                })
                        })
                        .then(a.cmp(b))
                });
                let mut counts = BTreeMap::<String, usize>::new();
                if let Some(c) = state.group {
                    for &i in &indices {
                        *counts.entry(state.value(i, c)).or_default() += 1;
                    }
                }
                let mut group = None;
                for &index in &indices {
                    if let Some(column) = state.group {
                        let value = state.value(index, column);
                        if group.as_ref() != Some(&value) {
                            ui.label(
                                RichText::new(format!("{value} ({})", counts[&value])).strong(),
                            );
                            group = Some(value);
                        }
                    }
                    let row = &state.rows[index];
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(total, 22.), egui::Sense::click());
                    if state.selection.contains(&index) {
                        ui.painter().rect_stroke(
                            rect,
                            0.,
                            egui::Stroke::new(1., Color32::from_gray(150)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let values = [
                        "".into(),
                        row.document.clone(),
                        row.source.into(),
                        row.message.clone(),
                        row.stamp.0.clone(),
                        row.stamp.1.clone(),
                        row.number.to_string(),
                    ];
                    let mut x = rect.left();
                    for (i, value) in values.iter().enumerate() {
                        let cell = egui::Rect::from_min_size(
                            egui::pos2(x, rect.top()),
                            egui::vec2(state.widths[i], 22.),
                        );
                        if i == 0 {
                            paint_class_icon(
                                ui.painter(),
                                cell.center() + egui::vec2(8., 0.),
                                row.severity,
                            );
                        } else {
                            ui.painter().with_clip_rect(cell).text(
                                cell.left_center() + egui::vec2(2., 0.),
                                egui::Align2::LEFT_CENTER,
                                value,
                                egui::FontId::proportional(12.),
                                Color32::from_gray(225),
                            );
                        }
                        x = cell.right();
                    }
                    if response.clicked() {
                        let modifiers = ui.input(|i| i.modifiers);
                        if modifiers.shift {
                            let anchor = state.selected.unwrap_or(index);
                            let a = indices.iter().position(|i| *i == anchor).unwrap_or(0);
                            let b = indices.iter().position(|i| *i == index).unwrap_or(0);
                            state
                                .selection
                                .extend(indices[a.min(b)..=a.max(b)].iter().copied());
                        } else if modifiers.command {
                            if !state.selection.insert(index) {
                                state.selection.remove(&index);
                            }
                        } else {
                            state.selection.clear();
                            state.selection.insert(index);
                        }
                        state.selected = Some(index);
                    }
                    if response.double_clicked() {
                        probe = row.targets.first().cloned();
                    }
                    let message = row.message.clone();
                    response.on_hover_text(message).context_menu(|ui| {
                        action = message_menu(ui, state, index).or(action.take());
                    });
                }
            });
        if let Some(action) = action {
            match action {
                MessageAction::Probe => {
                    probe = state
                        .selected
                        .and_then(|i| state.rows[i].targets.first())
                        .cloned()
                }
                MessageAction::Next(next) => {
                    self.next_validation_message(next);
                    return;
                }
                MessageAction::Copy => ui.ctx().copy_text(state.text(true)),
                MessageAction::Save(selection) => {
                    let text = state.text(selection);
                    if let Some(path) = self
                        .file_dialog
                        .clone()
                        .set_file_name("Messages.txt")
                        .add_filter("Text files", &["txt"])
                        .save_file()
                        && let Err(error) = std::fs::write(path, text)
                    {
                        self.message(true, format!("Save Messages: {error}"));
                    }
                }
                MessageAction::Clear(mode) => {
                    let class = state
                        .selected
                        .and_then(|i| state.rows.get(i))
                        .map(|r| r.severity);
                    let mut i = 0;
                    state.rows.retain(|r| {
                        let keep = match mode {
                            0 => false,
                            1 => !state.selection.contains(&i),
                            _ => Some(r.severity) != class,
                        };
                        i += 1;
                        keep
                    });
                    state.selected = None;
                    state.selection.clear();
                }
            }
        }
        if let Some(target) = probe {
            self.probe_validation_target(target);
        }
    }
}
enum MessageAction {
    Probe,
    Next(bool),
    Copy,
    Save(bool),
    Clear(u8),
}
fn message_menu(ui: &mut egui::Ui, state: &mut State, index: usize) -> Option<MessageAction> {
    if !state.selection.contains(&index) {
        state.selection.clear();
        state.selection.insert(index);
        state.selected = Some(index);
    }
    let mut action = None;
    ui.menu_button("Group By", |ui| {
        if ui
            .selectable_label(state.group.is_none(), "No Grouping")
            .clicked()
        {
            state.group = None;
            ui.close();
        }
        for (column, name) in [(0, "Class"), (1, "Document"), (2, "Source")] {
            if ui
                .selectable_label(state.group == Some(column), name)
                .clicked()
            {
                state.group = Some(column);
                ui.close();
            }
        }
    });
    let can_probe = state.rows.get(index).is_some_and(|r| !r.targets.is_empty());
    for (name, enabled, command) in [
        ("Cross Probe", can_probe, MessageAction::Probe),
        (
            "Cross Probe Next Message",
            state.rows.iter().any(|r| !r.targets.is_empty()),
            MessageAction::Next(true),
        ),
        (
            "Cross Probe Previous Message",
            state.rows.iter().any(|r| !r.targets.is_empty()),
            MessageAction::Next(false),
        ),
    ] {
        if ui.add_enabled(enabled, egui::Button::new(name)).clicked() {
            action = Some(command);
            ui.close();
        }
    }
    ui.add_enabled(
        false,
        egui::Button::new("Place Specific No ERC for this violation..."),
    );
    ui.separator();
    for (name, command) in [
        ("Clear All", MessageAction::Clear(0)),
        ("Clear Selected", MessageAction::Clear(1)),
        ("Clear Class", MessageAction::Clear(2)),
    ] {
        if ui.button(name).clicked() {
            action = Some(command);
            ui.close();
        }
    }
    ui.separator();
    for (name, command) in [
        ("Save...", MessageAction::Save(false)),
        ("Save Selection...", MessageAction::Save(true)),
    ] {
        if ui.button(name).clicked() {
            action = Some(command);
            ui.close();
        }
    }
    ui.add_enabled(false, egui::Button::new("Export To Report..."));
    if ui.button("Copy").clicked() {
        action = Some(MessageAction::Copy);
        ui.close();
    }
    ui.add_enabled(false, egui::Button::new("Print..."));
    action
}
fn paint_class_icon(painter: &egui::Painter, centre: egui::Pos2, severity: Option<Severity>) {
    let color = match severity {
        None => Color32::from_rgb(145, 187, 114),
        Some(Severity::Warning) => Color32::from_rgb(255, 239, 71),
        Some(Severity::Error) => Color32::from_rgb(255, 192, 64),
        Some(Severity::Fatal) => Color32::from_rgb(232, 100, 94),
    };
    painter.rect_filled(
        egui::Rect::from_center_size(centre, egui::vec2(18., 18.)),
        0.,
        color,
    );
}

#[cfg(test)]
#[path = "schematic_validation_tests.rs"]
mod tests;
