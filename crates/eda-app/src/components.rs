use super::*;
use eda_core::library::Library;

#[derive(Default)]
pub(super) struct Panel {
    libraries: Vec<(PathBuf, Library)>,
    library: usize,
    selected: usize,
    query: String,
    part: u32,
    mode: u32,
    available: bool,
    details_less: bool,
}

impl PcbCraft {
    pub(super) fn import_library(&mut self, path: PathBuf, activate: bool) {
        let path = path.canonicalize().unwrap_or(path);
        let existing = self
            .components_ui
            .libraries
            .iter()
            .position(|(p, _)| *p == path);
        let index = if let Some(index) = existing {
            index
        } else {
            match eda_io::altium::library::load(&path) {
                Ok(library) => {
                    self.message(
                        false,
                        format!(
                            "{}: {} components imported",
                            library.name,
                            library.symbols.len()
                        ),
                    );
                    for warning in &library.warnings {
                        self.message(false, format!("{}: {warning}", library.name));
                    }
                    if !self.settings.libraries.contains(&path) {
                        self.settings.libraries.push(path.clone());
                    }
                    self.components_ui.libraries.push((path, library));
                    self.components_ui.libraries.len() - 1
                }
                Err(e) => {
                    self.message(true, format!("Library {}: {e}", path.display()));
                    return;
                }
            }
        };
        if activate {
            self.components_ui.library = index;
            self.components_ui.selected = 0;
            self.components_ui.part = 1;
            self.components_ui.mode = 0;
            self.components_ui.query.clear();
            self.settings.components = true;
            self.settings.right_components = true;
        }
    }
    fn pick_library(&mut self) {
        if let Some(path) = self
            .file_dialog
            .clone()
            .add_filter("Altium libraries", &["IntLib", "SchLib"])
            .pick_file()
        {
            self.import_library(path, true);
        }
    }
    pub(super) fn library_dialog(&mut self, ctx: &egui::Context) {
        if !self.components_ui.available {
            return;
        }
        let mut open = true;
        let mut add = false;
        egui::Window::new("Available File-based Libraries")
            .open(&mut open)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.label("Installed");
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(300.0)
                    .show(ui, |ui| {
                        for (path, library) in &self.components_ui.libraries {
                            ui.label(&library.name);
                            ui.label(
                                RichText::new(path.display().to_string())
                                    .small()
                                    .color(MUTED),
                            );
                        }
                    });
                ui.separator();
                add = ui
                    .button("Install…")
                    .on_hover_text("Add a local SchLib or IntLib to PCBCraft")
                    .clicked();
            });
        self.components_ui.available = open;
        if add {
            self.pick_library();
        }
    }
    pub(super) fn components_panel(&mut self, ui: &mut egui::Ui) {
        let mut install = false;
        let mut place = false;
        let can_place = self
            .active
            .is_some_and(|i| self.documents[i].model.kind == DocumentKind::Schematic);
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size.y = 24.0;
            theme::icon_button(ui, theme::Icon::Filter, false, false)
                .on_hover_text("Categories are not implemented");
            let panel = &mut self.components_ui;
            let name = panel
                .libraries
                .get(panel.library)
                .map_or("File-based Libraries", |(_, l)| l.name.as_str());
            egui::ComboBox::from_id_salt("components_library")
                .width((ui.available_width() - 64.0).max(80.0))
                .selected_text(name)
                .show_ui(ui, |ui| {
                    ui.weak("FILE-BASED LIBRARIES");
                    for (index, (_, library)) in panel.libraries.iter().enumerate() {
                        if ui
                            .selectable_value(&mut panel.library, index, &library.name)
                            .changed()
                        {
                            panel.selected = 0;
                            panel.part = 1;
                            panel.mode = 0;
                        }
                    }
                });
            ui.menu_button("☰", |ui| {
                ui.add_enabled(false, egui::Button::new("Footprints"));
                if ui.button("Libraries Preferences…").clicked() {
                    self.components_ui.available = true;
                    ui.close();
                }
                if ui.button("Install Library…").clicked() {
                    install = true;
                    ui.close();
                }
                ui.add_enabled(false, egui::Button::new("File-based Libraries Search…"));
                ui.add_enabled(false, egui::Button::new("Refresh"));
            });
            ui.add_enabled(false, egui::Button::new("◫"));
        });
        ui.add_space(8.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.components_ui.query)
                .hint_text("⌕ Search")
                .min_size(egui::vec2(0.0, 24.0))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(5.0);
        if install {
            self.pick_library();
        }
        let panel = &mut self.components_ui;
        let Some((path, library)) = panel.libraries.get(panel.library) else {
            ui.add_space(16.0);
            ui.label("No file-based libraries available.");
            if ui.button("Install Library…").clicked() {
                self.pick_library();
            }
            return;
        };
        let query = panel.query.to_lowercase();
        let matches: Vec<_> = library
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query.split_whitespace().all(|term| {
                    s.name.to_lowercase().contains(term)
                        || s.description.to_lowercase().contains(term)
                        || s.variants.first().is_some_and(|v| {
                            v.fragment.components[0].parameters.iter().any(|(k, v)| {
                                k.to_lowercase().contains(term) || v.to_lowercase().contains(term)
                            })
                        })
                })
            })
            .collect();
        if !matches.iter().any(|(i, _)| *i == panel.selected)
            && let Some((i, _)) = matches.first()
        {
            panel.selected = *i;
            panel.part = 1;
            panel.mode = 0;
        }
        let table_height = (ui.available_height() * 0.49).clamp(100.0, 370.0);
        egui::Frame::NONE
            .stroke(egui::Stroke::new(1.0, Color32::from_gray(48)))
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("components_results")
                    .max_height(table_height)
                    .min_scrolled_height(table_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let width = ui.available_width().max(340.0);
                        let (header, _) =
                            ui.allocate_exact_size(egui::vec2(width, 19.0), egui::Sense::hover());
                        for (x, text) in [
                            (0.0, "Design Item ID"),
                            (116.0, "Description"),
                            (242.0, "Value"),
                        ] {
                            ui.painter().text(
                                header.left_top() + egui::vec2(x + 4.0, 9.0),
                                egui::Align2::LEFT_CENTER,
                                text,
                                egui::FontId::proportional(11.0),
                                Color32::from_gray(210),
                            );
                        }
                        ui.painter().hline(
                            header.x_range(),
                            header.bottom(),
                            egui::Stroke::new(1.0, Color32::from_gray(70)),
                        );
                        for (index, symbol) in &matches {
                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(width, 18.0), egui::Sense::click());
                            if *index == panel.selected {
                                ui.painter()
                                    .rect_filled(rect, 0.0, Color32::from_rgb(62, 87, 104));
                            }
                            theme::draw_icon(
                                ui.painter(),
                                egui::Rect::from_center_size(
                                    rect.left_center() + egui::vec2(8.0, 0.0),
                                    egui::vec2(12.0, 14.0),
                                ),
                                theme::Icon::LibraryComponent,
                                true,
                            );
                            if response.clicked() {
                                panel.selected = *index;
                                panel.part = 1;
                                panel.mode = 0;
                            }
                            if response.double_clicked() && can_place {
                                place = true;
                            }
                            response.context_menu(|ui| {
                                if ui
                                    .add_enabled(can_place, egui::Button::new("Place"))
                                    .clicked()
                                {
                                    if panel.selected != *index {
                                        panel.part = 1;
                                        panel.mode = 0;
                                    }
                                    panel.selected = *index;
                                    place = true;
                                    ui.close();
                                }
                            });
                            let comment = &symbol.variants[0].fragment.components[0].comment;
                            for (x, w, text) in [
                                (15.0, 101.0, symbol.name.as_str()),
                                (116.0, 126.0, symbol.description.as_str()),
                                (242.0, 98.0, comment.as_str()),
                            ] {
                                let clip = egui::Rect::from_min_size(
                                    rect.left_top() + egui::vec2(x + 3.0, 0.0),
                                    egui::vec2(w - 6.0, 18.0),
                                )
                                .intersect(ui.clip_rect());
                                ui.painter().with_clip_rect(clip).text(
                                    rect.left_center() + egui::vec2(x + 3.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    text,
                                    egui::FontId::proportional(11.0),
                                    Color32::from_gray(215),
                                );
                            }
                        }
                    });
                ui.weak(format!("Results: {}", matches.len()));
            });
        ui.add_space(14.0);
        if !matches.is_empty()
            && let Some(symbol) = library.symbols.get(panel.selected)
        {
            egui::ScrollArea::vertical()
                .id_salt("components_details")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    egui::CollapsingHeader::new(format!("Details  {}", symbol.name))
                        .id_salt("component_details")
                        .default_open(true)
                        .show(ui, |ui| {
                            let component = &symbol.variants[0].fragment.components[0];
                            let mut rows = detail_rows(&component.parameters);
                            if panel.details_less {
                                rows.truncate(3);
                            }
                            egui::Grid::new("library_parameter_values")
                                .num_columns(2)
                                .min_col_width(60.0)
                                .max_col_width((ui.available_width() - 8.0) / 2.0)
                                .show(ui, |ui| {
                                    for (name, value) in rows {
                                        ui.add(egui::Label::new(&name).truncate())
                                            .on_hover_text(&name);
                                        ui.add(egui::Label::new(&value).truncate())
                                            .on_hover_text(&value);
                                        ui.end_row();
                                    }
                                });
                            if ui
                                .link(if panel.details_less {
                                    "Show more ▾"
                                } else {
                                    "Show less ▴"
                                })
                                .clicked()
                            {
                                panel.details_less = !panel.details_less;
                            }
                        });
                    egui::CollapsingHeader::new("Symbol")
                        .id_salt("component_symbol")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label("Part");
                                ui.add(
                                    egui::DragValue::new(&mut panel.part).range(1..=symbol.parts),
                                );
                                ui.label("Mode");
                                ui.add(
                                    egui::DragValue::new(&mut panel.mode)
                                        .range(0..=symbol.display_modes - 1),
                                );
                            });
                            if let Some(variant) = symbol.variant(panel.part.max(1), panel.mode) {
                                eda_render::symbol_preview(ui, &variant.fragment, 160.0);
                            }
                            if ui
                                .add_enabled(can_place, egui::Button::new("Place"))
                                .clicked()
                            {
                                place = true;
                            }
                        });
                    egui::CollapsingHeader::new("Model")
                        .id_salt("component_model")
                        .show(ui, |ui| {
                            for model in &symbol.variants[0].fragment.components[0].settings.models
                            {
                                ui.label(format!("{}  {}", model.kind, model.name));
                            }
                            ui.add_enabled(false, egui::Button::new("3D preview"));
                        });
                    ui.label(
                        RichText::new(path.display().to_string())
                            .small()
                            .color(MUTED),
                    );
                });
        }
        if place && let Some(symbol) = library.symbols.get(panel.selected) {
            let (symbol, part, mode) = (symbol.clone(), panel.part.max(1), panel.mode);
            self.start_library_tool(symbol, part, mode);
        }
    }
}

fn detail_rows(values: &std::collections::BTreeMap<String, String>) -> Vec<(String, String)> {
    let mut rows = vec![];
    for (name, value) in values {
        if name.eq_ignore_ascii_case("Comment") {
            continue;
        }
        if name.starts_with("ComponentLink") {
            if name.ends_with("Description") {
                let key = name.trim_end_matches("Description").to_owned() + "URL";
                if let Some(url) = values.get(&key) {
                    rows.push((value.clone(), url.clone()));
                }
            }
        } else {
            rows.push((name.clone(), value.clone()));
        }
    }
    rows.sort_by_cached_key(|(name, _)| {
        name.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    });
    rows
}
