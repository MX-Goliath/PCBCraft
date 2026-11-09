//! Device sheets and local design reuse. The model changes only on canvas placement.
use super::*;
use eda_io::schematic_reuse::{self as reuse, Snippet};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub(in crate::app) struct State {
    device: Option<DeviceSelector>,
    reuse: bool,
    snippets: Vec<Snippet>,
    folders: Option<Folders>,
    create: Option<Create>,
    properties: Option<(Snippet, String, String)>,
    error: String,
    refresh: bool,
}
#[derive(Default)]
struct DeviceSelector {
    filter: String,
    folder: Option<usize>,
    files: Vec<PathBuf>,
    selected: Option<PathBuf>,
}
struct Create {
    name: String,
    description: String,
    folder: PathBuf,
    source: Option<(Document, Selection, u64)>,
}
struct Folders {
    device: bool,
    selected: Option<usize>,
}
impl State {
    pub(in crate::app) fn open_device(&mut self) {
        self.device = Some(DeviceSelector::default());
        self.refresh = true;
        self.error.clear();
    }
    pub(in crate::app) fn open_reuse(&mut self) {
        self.reuse = true;
        self.refresh = true;
        self.error.clear();
    }
    pub(in crate::app) fn open_create(&mut self) {
        self.create = Some(Create {
            name: String::new(),
            description: String::new(),
            folder: PathBuf::new(),
            source: None,
        });
        self.error.clear();
    }
    pub(in crate::app) fn modal(&self) -> bool {
        self.device.is_some()
            || self.folders.is_some()
            || self.create.is_some()
            || self.properties.is_some()
    }
    pub(super) fn escape(&mut self) {
        if self.folders.take().is_some()
            || self.properties.take().is_some()
            || self.create.take().is_some()
        {
            return;
        }
        self.device = None;
    }
}

fn selection(view: &View) -> Selection {
    let mut ids = view.sch_selection.clone();
    if ids.is_empty() && view.sch_segments.is_empty() {
        ids.extend(view.selected);
    }
    ids.retain(|id| !view.sch_segments.iter().any(|s| s.id == *id));
    Selection {
        ids,
        segments: view.sch_segments.clone(),
    }
}

// Read only registered directories; canonical paths prevent loops through junctions.
fn device_files(folders: &[(PathBuf, bool)], selected: Option<usize>) -> Vec<PathBuf> {
    let mut pending: Vec<_> = folders
        .iter()
        .enumerate()
        .filter(|(n, _)| selected.is_none_or(|selected| selected == *n))
        .map(|(_, (path, recursive))| (path.clone(), *recursive))
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut files = vec![];
    while let Some((folder, recursive)) = pending.pop() {
        let Ok(canonical) = folder.canonicalize() else {
            continue;
        };
        if !seen.insert(canonical) {
            continue;
        }
        if seen.len() > 1000 || files.len() > 5000 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.take(5001).flatten() {
            let path = entry.path();
            if path.is_dir() {
                if recursive {
                    pending.push((path, true));
                }
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
            {
                files.push(path);
            }
        }
    }
    files.sort_by_key(|p| {
        p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    });
    files.dedup();
    files
}

pub(in crate::app) fn resolve_device(folders: &[(PathBuf, bool)], name: &str) -> Option<PathBuf> {
    device_files(folders, None).into_iter().find(|p| {
        p.file_stem()
            .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case(name))
    })
}

enum ReuseAction {
    Place(PathBuf),
    Sheet(PathBuf, bool),
    Edit(PathBuf),
}
impl PcbCraft {
    pub(in crate::app) fn schematic_resource_windows(&mut self, ctx: &egui::Context, index: usize) {
        let mut state = std::mem::take(&mut self.documents[index].sch_editor.bar.resources);
        if state.refresh {
            if let Some(d) = &mut state.device {
                d.files = device_files(&self.settings.device_sheet_folders, d.folder);
            }
            match reuse::scan(&self.settings.snippet_folders) {
                Ok(snippets) => state.snippets = snippets,
                Err(e) => state.error = e,
            }
            state.refresh = false;
        }
        let mut action = None;
        if let Some(d) = &mut state.device {
            let mut open = true;
            let mut accept = false;
            let mut cancel = false;
            let mut folder_changed = false;
            egui::Window::new(egui::RichText::new("Select Device Sheet").size(12.))
                .id(egui::Id::new("select_device_sheet"))
                .order(egui::Order::Foreground)
                .fade_in(false)
                .default_pos(ctx.screen_rect().center() - egui::vec2(411.5, 319.5))
                .fixed_size([809., 613.])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show_stable(ctx, |ui| {
                    ui.add_enabled_ui(state.folders.is_none(), |ui| {
                        ui.horizontal(|ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2(260., 540.),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_min_size(egui::vec2(260., 554.));
                                    ui.set_max_width(260.);
                                    egui::ScrollArea::both()
                                        .id_salt("device_folders")
                                        .auto_shrink([false, false])
                                        .max_height(530.)
                                        .show(ui, |ui| {
                                            ui.style_mut().wrap_mode =
                                                Some(egui::TextWrapMode::Extend);
                                            folder_changed |= ui
                                                .selectable_value(
                                                    &mut d.folder,
                                                    None,
                                                    "All Device Sheets Folders",
                                                )
                                                .changed();
                                            for (n, (path, _)) in self
                                                .settings
                                                .device_sheet_folders
                                                .iter()
                                                .enumerate()
                                            {
                                                ui.indent("device_folder", |ui| {
                                                    folder_changed |= ui
                                                        .selectable_value(
                                                            &mut d.folder,
                                                            Some(n),
                                                            path.display().to_string(),
                                                        )
                                                        .changed();
                                                });
                                            }
                                        });
                                },
                            );
                            ui.separator();
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Filter:");
                                    ui.add_sized(
                                        [ui.available_width(), 22.],
                                        egui::TextEdit::singleline(&mut d.filter),
                                    );
                                });
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(ui.available_width(), 500.));
                                    egui::ScrollArea::vertical()
                                        .id_salt("device_files")
                                        .max_height(500.)
                                        .show(ui, |ui| {
                                            let filter = d.filter.to_lowercase();
                                            for path in &d.files {
                                                let name = path
                                                    .file_stem()
                                                    .unwrap_or_default()
                                                    .to_string_lossy();
                                                if !name.to_lowercase().contains(&filter) {
                                                    continue;
                                                }
                                                let r = ui.selectable_value(
                                                    &mut d.selected,
                                                    Some(path.clone()),
                                                    name.as_ref(),
                                                );
                                                accept |= r.double_clicked();
                                            }
                                        });
                                });
                            });
                        });
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Device Sheet Folders...").clicked() {
                                state.folders = Some(Folders {
                                    device: true,
                                    selected: None,
                                });
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    cancel = ui.button("Cancel").clicked();
                                    accept |= ui
                                        .add_enabled(
                                            d.selected.is_some(),
                                            egui::Button::new("OK").min_size(egui::vec2(75., 23.)),
                                        )
                                        .clicked();
                                },
                            );
                        });
                    });
                    error(ui, &state.error);
                });
            if folder_changed {
                d.files = device_files(&self.settings.device_sheet_folders, d.folder);
                d.selected = None;
            }
            if accept && let Some(path) = &d.selected {
                action = Some(ReuseAction::Sheet(path.clone(), true));
            }
            if !open || cancel || accept {
                state.device = None;
            }
        }
        if state.reuse {
            let mut open = true;
            egui::Window::new(egui::RichText::new("Design Reuse").size(12.))
                .id(egui::Id::new("design_reuse")).order(egui::Order::Foreground).fade_in(false).default_pos([73.,134.]).default_size([374., 368.])
                .collapsible(false).open(&mut open).show_stable(ctx, |ui| {
                    ui.set_min_size(egui::vec2(374.,368.));
                    ui.add_enabled_ui(!state.modal(), |ui| {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("reuse_scope").selected_text("Local").width(ui.available_width() - 65.).show_ui(ui, |ui| {
                                ui.selectable_label(true, "Local").clicked();
                                ui.add_enabled(false, egui::Button::new("Workspace"));
                            });
                            ui.add_enabled(false, egui::Button::new("+")).on_disabled_hover_text("A connected Workspace is required to create a Reuse Block");
                            ui.menu_button("≡", |ui| {
                                ui.add_enabled(false, egui::Button::new("✓ Local Snippets"));
                                if ui.button("Snippets Folders...").clicked() { state.folders = Some(Folders { device: false, selected: None }); ui.close(); }
                                if ui.button("Refresh").clicked() { state.refresh = true; ui.close(); }
                            });
                        });
                        ui.separator();
                        egui::ScrollArea::vertical().max_height(650.).show(ui, |ui| {
                            if state.snippets.is_empty() {
                                ui.add_space(105.);
                                ui.vertical_centered(|ui| { ui.label("Nothing to display. Create a new local snippet from Schematic or Pcb document."); });
                            }
                            for snippet in &state.snippets {
                                egui::Frame::group(ui.style()).show(ui, |ui| {
                                    ui.strong(&snippet.name);
                                    if !snippet.description.is_empty() { ui.label(&snippet.description); }
                                    ui.horizontal_wrapped(|ui| {
                                        if ui.button("Place").clicked() { action = Some(ReuseAction::Place(snippet.path.clone())); }
                                        if ui.button("Place as Sheet Symbol").clicked() { action = Some(ReuseAction::Sheet(snippet.path.clone(), false)); }
                                        ui.menu_button("…", |ui| {
                                            if ui.button("Edit").clicked() { action = Some(ReuseAction::Edit(snippet.path.clone())); ui.close(); }
                                            if ui.add_enabled(snippet.metadata.is_some(), egui::Button::new("Edit Properties...")).clicked() {
                                                state.properties = Some((snippet.clone(), snippet.name.clone(), snippet.description.clone())); ui.close();
                                            }
                                        });
                                    });
                                });
                            }
                        });
                    });
                    error(ui, &state.error);
                });
            state.reuse = open;
        }
        if let Some(folders) = &mut state.folders {
            let mut open = true;
            let mut close = false;
            egui::Window::new(
                egui::RichText::new(if folders.device {
                    "Device Sheet Folders"
                } else {
                    "Snippets Folders"
                })
                .size(12.),
            )
            .id(egui::Id::new("reuse_folders"))
            .order(egui::Order::Foreground)
            .fade_in(false)
            .default_pos(ctx.screen_rect().center() - egui::vec2(276.5, 244.))
            .fixed_size([539., 462.])
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show_stable(ctx, |ui| {
                egui::ScrollArea::both().max_height(350.).show(ui, |ui| {
                    ui.set_min_size(egui::vec2(510., 345.));
                    egui::Grid::new("reuse_folder_paths")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Path");
                            ui.label(if folders.device {
                                "Include Subfolders"
                            } else {
                                ""
                            });
                            ui.end_row();
                            if folders.device {
                                for (n, (path, recursive)) in
                                    self.settings.device_sheet_folders.iter_mut().enumerate()
                                {
                                    ui.selectable_value(
                                        &mut folders.selected,
                                        Some(n),
                                        path.display().to_string(),
                                    );
                                    state.refresh |= ui.checkbox(recursive, "").changed();
                                    ui.end_row();
                                }
                            } else {
                                for (n, path) in self.settings.snippet_folders.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut folders.selected,
                                        Some(n),
                                        path.display().to_string(),
                                    );
                                    ui.label("");
                                    ui.end_row();
                                }
                            }
                        });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    let len = if folders.device {
                        self.settings.device_sheet_folders.len()
                    } else {
                        self.settings.snippet_folders.len()
                    };
                    let selected = folders.selected.filter(|n| *n < len);
                    for (label, down) in [("Move Up", false), ("Move Down", true)] {
                        if ui
                            .add_enabled(
                                selected.is_some_and(|n| if down { n + 1 < len } else { n > 0 }),
                                egui::Button::new(label),
                            )
                            .clicked()
                        {
                            let n = selected.unwrap();
                            let other = if down { n + 1 } else { n - 1 };
                            if folders.device {
                                self.settings.device_sheet_folders.swap(n, other);
                            } else {
                                self.settings.snippet_folders.swap(n, other);
                            }
                            folders.selected = Some(other);
                            state.refresh = true;
                        }
                    }
                    if ui.button("Add...").clicked()
                        && let Some(path) = self
                            .file_dialog
                            .clone()
                            .set_title("Select Folder")
                            .pick_folder()
                    {
                        if folders.device {
                            if !self
                                .settings
                                .device_sheet_folders
                                .iter()
                                .any(|(p, _)| *p == path)
                            {
                                self.settings.device_sheet_folders.push((path, true));
                            }
                        } else if !self.settings.snippet_folders.contains(&path) {
                            self.settings.snippet_folders.push(path);
                        }
                        state.refresh = true;
                    }
                    if ui
                        .add_enabled(selected.is_some(), egui::Button::new("Remove"))
                        .on_hover_text("Unregister this folder; files remain on disk")
                        .clicked()
                    {
                        let n = selected.unwrap();
                        if folders.device {
                            self.settings.device_sheet_folders.remove(n);
                        } else {
                            self.settings.snippet_folders.remove(n);
                        }
                        folders.selected = None;
                        state.refresh = true;
                    }
                    close = ui.button("Close").clicked();
                });
            });
            if !open || close {
                state.folders = None;
            }
        }
        if let Some(create) = &mut state.create {
            if create.source.is_none() {
                let doc = &self.documents[index];
                create.source = Some((
                    doc.model.clone(),
                    selection(&doc.view),
                    doc.history.revision,
                ));
                create.folder = self
                    .settings
                    .snippet_folders
                    .first()
                    .cloned()
                    .unwrap_or_default();
            }
            let mut open = true;
            let mut accept = false;
            let mut cancel = false;
            egui::Window::new(egui::RichText::new("New Schematic Snippet").size(12.))
                .id(egui::Id::new("new_schematic_snippet"))
                .order(egui::Order::Foreground)
                .fade_in(false)
                .default_pos(ctx.screen_rect().center() - egui::vec2(192., 148.5))
                .fixed_size([370., 271.])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show_stable(ctx, |ui| {
                    ui.label("Name");
                    ui.add_sized(
                        [ui.available_width(), 22.],
                        egui::TextEdit::singleline(&mut create.name),
                    );
                    ui.label("Description");
                    ui.add_sized(
                        [ui.available_width(), 70.],
                        egui::TextEdit::multiline(&mut create.description),
                    );
                    ui.label("Path");
                    ui.horizontal(|ui| {
                        let mut path = create.folder.display().to_string();
                        ui.add_sized(
                            [ui.available_width() - 32., 22.],
                            egui::TextEdit::singleline(&mut path).interactive(false),
                        );
                        if ui.button("…").clicked()
                            && let Some(path) = self
                                .file_dialog
                                .clone()
                                .set_title("Choose Snippet Folder")
                                .pick_folder()
                        {
                            create.folder = path;
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        cancel = ui.button("Cancel").clicked();
                        accept = ui
                            .add_enabled(
                                !create.name.trim().is_empty() && create.folder.is_dir(),
                                egui::Button::new("Create"),
                            )
                            .clicked();
                    });
                    error(ui, &state.error);
                });
            if accept {
                let (source, selected, revision) = create.source.as_ref().unwrap();
                let current = &self.documents[index];
                let result =
                    if current.model.id != source.id || current.history.revision != *revision {
                        Err("The source document changed. Reopen Create Snippet.".into())
                    } else {
                        reuse::save(
                            &create.folder,
                            &create.name,
                            &create.description,
                            source,
                            selected,
                        )
                    };
                match result {
                    Ok(_) => {
                        if !self.settings.snippet_folders.contains(&create.folder) {
                            self.settings.snippet_folders.push(create.folder.clone());
                        }
                        state.refresh = true;
                        state.reuse = true;
                        state.error.clear();
                    }
                    Err(e) => {
                        state.error = e;
                        accept = false;
                    }
                }
            }
            if !open || cancel || accept {
                state.create = None;
            }
        }
        if let Some((snippet, name, description)) = &mut state.properties {
            let mut open = true;
            let mut accept = false;
            let mut cancel = false;
            egui::Window::new(egui::RichText::new("Snippet Properties").size(12.))
                .id(egui::Id::new("snippet_properties"))
                .order(egui::Order::Foreground)
                .fade_in(false)
                .fixed_size([368., 225.])
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show_stable(ctx, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(name);
                    ui.label("Description");
                    ui.add_sized(
                        [ui.available_width(), 90.],
                        egui::TextEdit::multiline(description),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        cancel = ui.button("Cancel").clicked();
                        accept = ui.button("OK").clicked();
                    });
                    error(ui, &state.error);
                });
            if accept {
                if let Err(e) = reuse::properties(snippet, name, description) {
                    state.error = e;
                    accept = false;
                } else {
                    state.refresh = true;
                    state.error.clear();
                }
            }
            if !open || cancel || accept {
                state.properties = None;
            }
        }
        if let Some(action) = action {
            let result = match action {
                ReuseAction::Edit(path) => {
                    self.request_open_path(path);
                    Ok(())
                }
                ReuseAction::Place(path) => self.place_local_snippet(ctx, index, &path),
                ReuseAction::Sheet(path, device) => self.place_resource_sheet(index, &path, device),
            };
            if let Err(e) = result {
                state.error = e.clone();
                self.message(true, e);
            }
        }
        let top = if state.properties.is_some() {
            Some("snippet_properties")
        } else if state.create.is_some() {
            Some("new_schematic_snippet")
        } else if state.folders.is_some() {
            Some("reuse_folders")
        } else if state.device.is_some() {
            Some("select_device_sheet")
        } else {
            None
        };
        if let Some(id) = top {
            ctx.move_to_top(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new(id),
            ));
        }
        self.documents[index].sch_editor.bar.resources = state;
    }

    fn place_local_snippet(
        &mut self,
        ctx: &egui::Context,
        index: usize,
        path: &Path,
    ) -> Result<(), String> {
        let source = eda_io::load(path)?;
        let s = source
            .schematic
            .as_ref()
            .ok_or("The selected snippet is not a schematic")?;
        let selected = Selection {
            ids: s
                .components
                .iter()
                .map(|c| c.id)
                .chain(
                    s.primitives
                        .iter()
                        .filter(|p| p.component.is_none() && p.presentation.parent.is_none())
                        .map(|p| p.id),
                )
                .collect(),
            segments: vec![],
        };
        let anchor = edit::selection_anchor(s, &selected).ok_or("This snippet is empty")?;
        let fragment = edit::extract_selection(s, &selected)?;
        // The paste path gives every object fresh IDs and reserves component designators.
        self.active = Some(index);
        self.sch_clipboard = Some((fragment, anchor));
        self.schematic_action(SchAction::Paste, ctx);
        Ok(())
    }

    fn place_resource_sheet(
        &mut self,
        index: usize,
        path: &Path,
        device: bool,
    ) -> Result<(), String> {
        let source = eda_io::load(path)?;
        let child = source
            .schematic
            .as_ref()
            .ok_or("The selected file is not a schematic")?;
        let s = self.documents[index]
            .model
            .schematic
            .as_ref()
            .ok_or("Open a schematic")?;
        let name = path.file_stem().unwrap_or_default().to_string_lossy();
        let mut fragment = arrange::device_sheet(s, child, &name)?;
        if !device {
            fragment.primitives[0].presentation.symbol_type.clear();
            fragment.primitives[0].fill = Some(11_194_200);
            if let K::SheetSymbol { filename, .. } = &mut fragment.primitives[0].kind {
                *filename = path.display().to_string();
            }
            for p in fragment.primitives.iter_mut().skip(1) {
                if p.presentation.record_type == 33
                    && let K::Text { text, .. } = &mut p.kind
                {
                    *text = path.display().to_string();
                    p.presentation.raw_text = Some(text.clone());
                }
            }
        }
        self.start_schematic_fragment(index, fragment, Point::default());
        Ok(())
    }
}
fn error(ui: &mut egui::Ui, error: &str) {
    if !error.is_empty() {
        ui.colored_label(egui::Color32::from_rgb(240, 110, 100), error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_folder_dialog_stays_above_its_disabled_parent_and_closes_read_only() {
        let mut app = crate::app::schematic_editor::tests::app();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let before = app.documents[0].model.clone();
        let state = &mut app.documents[0].sch_editor.bar.resources;
        state.open_device();
        state.folders = Some(Folders {
            device: true,
            selected: None,
        });
        let mut frame = |events| {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1707., 1026.),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| app.schematic_resource_windows(ctx, 0),
            );
        };
        for _ in 0..3 {
            frame(vec![]);
        }
        let parent = ctx
            .memory(|m| m.area_rect(egui::Id::new("select_device_sheet")))
            .unwrap();
        let child = ctx
            .memory(|m| m.area_rect(egui::Id::new("reuse_folders")))
            .unwrap();
        let at = parent.min + egui::vec2(20., 40.);
        assert!(!child.contains(at));
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
        }
        assert_eq!(
            ctx.layer_id_at(child.center()),
            Some(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("reuse_folders")
            ))
        );
        app.documents[0].sch_editor.bar.resources.escape();
        assert!(app.documents[0].sch_editor.bar.resources.folders.is_none());
        assert!(app.documents[0].sch_editor.bar.resources.device.is_some());
        assert_eq!(app.documents[0].model, before);
        assert_eq!(app.documents[0].history.revision, 0);
    }
}
