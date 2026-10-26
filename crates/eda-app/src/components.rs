use super::*;
use eda_core::library::Library;

#[derive(Default)]
pub(super) struct Panel {
    pub(super) drag_document: Option<Uuid>,
    pub(super) libraries: Vec<(PathBuf, Library)>,
    pub(super) footprint_libraries: Vec<(PathBuf, eda_io::altium_pcb::library::Library)>,
    library: usize,
    selected: usize,
    query: String,
    part: u32,
    mode: u32,
    available: bool,
    details_more: bool,
    details_closed: bool,
    table_height: f32,
    scroll_x: f32,
    columns: [f32; 3],
    sort_column: usize,
    sort_reverse: bool,
}

impl PcbCraft {
    pub(super) fn import_library(&mut self, path: PathBuf, activate: bool) {
        let path = path.canonicalize().unwrap_or(path);
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("PcbLib"))
        {
            if !self
                .components_ui
                .footprint_libraries
                .iter()
                .any(|(p, _)| *p == path)
            {
                match eda_io::altium_pcb::library::load(&path) {
                    Ok(library) => {
                        self.message(
                            false,
                            format!(
                                "{}: {} footprints imported",
                                path.display(),
                                library.footprints.len()
                            ),
                        );
                        for warning in &library.warnings {
                            self.message(false, warning.clone());
                        }
                        if !self.settings.libraries.contains(&path) {
                            self.settings.libraries.push(path.clone());
                        }
                        self.components_ui.footprint_libraries.push((path, library));
                    }
                    Err(e) => self.message(true, format!("Library {}: {e}", path.display())),
                }
            }
            return;
        }
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
            .add_filter("Altium libraries", &["IntLib", "SchLib", "PcbLib"])
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
                        for (path, library) in &self.components_ui.footprint_libraries {
                            ui.label(format!(
                                "{} ({} footprints)",
                                path.file_name().unwrap_or_default().to_string_lossy(),
                                library.footprints.len()
                            ));
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
                    .on_hover_text("Add a local SchLib, PcbLib or IntLib to PCBCraft")
                    .clicked();
            });
        self.components_ui.available = open;
        if add {
            self.pick_library();
        }
    }
    pub(super) fn components_panel(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width((ui.available_width() - 4.0).max(1.0));
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        ui.spacing_mut().scroll.bar_width = 12.0;
        let can_place = self
            .active
            .is_some_and(|i| self.documents[i].model.kind == DocumentKind::Schematic);
        let mut install = false;
        let mut place = false;
        let mut drag = false;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size.y = 24.0;
            let filter = theme::icon_button(ui, theme::Icon::Filter, false, false);
            ui.painter().rect_stroke(
                filter.rect,
                2.0,
                egui::Stroke::new(1.0, Color32::from_gray(45)),
                egui::StrokeKind::Inside,
            );
            ui.add_space(8.0);
            let panel = &mut self.components_ui;
            let name = panel
                .libraries
                .get(panel.library)
                .map_or("File-based Libraries", |(_, l)| l.name.as_str());
            let combo = egui::ComboBox::from_id_salt("components_library")
                .width((ui.available_width() - 72.0).max(80.0))
                .selected_text(format!("     {name}"))
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
            theme::draw_icon(
                ui.painter(),
                egui::Rect::from_center_size(
                    combo.response.rect.left_center() + egui::vec2(11.0, 0.0),
                    egui::vec2(12.0, 12.0),
                ),
                theme::Icon::LibraryComponent,
                true,
            );
            ui.add_space(6.0);
            let (menu, _) = egui::containers::menu::MenuButton::from_button(
                egui::Button::new("").min_size(egui::vec2(27.0, 24.0)),
            )
            .ui(ui, |ui| {
                ui.add_enabled(false, egui::Button::new("Footprints"));
                if ui.button("Libraries Preferences…").clicked() {
                    self.components_ui.available = true;
                    ui.close();
                }
                ui.add_enabled(false, egui::Button::new("File-based Libraries Search…"));
                ui.separator();
                ui.add_enabled(false, egui::Button::new("Refresh                       F5"));
            });
            for y in [-3.0, 0.0, 3.0] {
                ui.painter().line_segment(
                    [
                        menu.rect.center() + egui::vec2(-5.0, y),
                        menu.rect.center() + egui::vec2(5.0, y),
                    ],
                    egui::Stroke::new(1.0, Color32::from_gray(216)),
                );
            }
            ui.add_space(4.0);
            let (r, _) = ui.allocate_exact_size(egui::vec2(27.0, 24.0), egui::Sense::hover());
            ui.painter().rect_filled(r, 2.0, Color32::from_gray(77));
            for x in [5.0, 17.0] {
                ui.painter().rect_stroke(
                    egui::Rect::from_min_size(r.min + egui::vec2(x, 5.0), egui::vec2(5.0, 14.0)),
                    0.0,
                    egui::Stroke::new(1.0, MUTED),
                    egui::StrokeKind::Inside,
                );
            }
        });
        ui.add_space(5.0);
        theme::search_field_height(
            ui,
            &mut self.components_ui.query,
            ui.available_width(),
            25.0,
        );
        ui.add_space(6.0);
        let filtered = !self.components_ui.query.trim().is_empty();
        if filtered {
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::from_gray(77));
            let galley = ui.painter().layout_no_wrap(
                self.components_ui.query.clone(),
                egui::FontId::proportional(11.0),
                Color32::from_gray(216),
            );
            let end = (rect.left() + 6.0 + galley.size().x).min(rect.right() - 18.0);
            ui.painter().with_clip_rect(rect).galley(
                rect.left_center() + egui::vec2(6.0, -galley.size().y * 0.5),
                galley,
                Color32::from_gray(216),
            );
            let close = egui::Rect::from_center_size(
                egui::pos2(end + 8.0, rect.center().y),
                egui::vec2(14.0, 14.0),
            );
            theme::draw_icon(ui.painter(), close.shrink(2.0), theme::Icon::Close, false);
            if ui
                .interact(
                    close,
                    ui.id().with("clear_components_filter"),
                    egui::Sense::click(),
                )
                .clicked()
            {
                self.components_ui.query.clear();
            }
            ui.add_space(6.0);
        }
        if self.components_ui.libraries.is_empty() {
            ui.weak("No file-based libraries available.");
            if ui.button("Install Library…").clicked() {
                install = true;
            }
            if install {
                self.pick_library();
            }
            return;
        }
        let panel = &mut self.components_ui;
        if panel.columns[0] == 0.0 {
            panel.columns = [120.0, 120.0, 120.0];
        }
        let (_, library) = &panel.libraries[panel.library];
        let query = panel.query.to_lowercase();
        let mut matches: Vec<_> = library
            .symbols
            .iter()
            .enumerate()
            .filter(|(_, s)| {
                query.split_whitespace().all(|term| {
                    s.name.to_lowercase().contains(term)
                        || s.description.to_lowercase().contains(term)
                        || s.variants[0].fragment.components[0]
                            .parameters
                            .iter()
                            .any(|(k, v)| {
                                k.to_lowercase().contains(term) || v.to_lowercase().contains(term)
                            })
                })
            })
            .map(|(i, _)| i)
            .collect();
        matches.sort_by_cached_key(|i| {
            let s = &library.symbols[*i];
            match panel.sort_column {
                1 => s.description.to_lowercase(),
                2 => symbol_value(s).to_lowercase(),
                _ => s.name.to_lowercase(),
            }
        });
        if panel.sort_reverse {
            matches.reverse();
        }
        if !matches.contains(&panel.selected)
            && let Some(i) = matches.first()
        {
            panel.selected = *i;
            panel.part = 1;
            panel.mode = 0;
        }
        let available = ui.available_height();
        let list_height = if panel.details_closed {
            (available - 58.0).max(80.0)
        } else {
            if panel.table_height == 0.0 {
                panel.table_height = 374.0;
            }
            (panel.table_height - if filtered { 29.0 } else { 0.0 })
                .clamp(100.0, (available - 90.0).max(100.0))
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let (header, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::hover());
            let mut x = header.left() - panel.scroll_x;
            for (col, title) in ["Design Item ID", "Description", "Value"]
                .iter()
                .enumerate()
            {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(x, header.top()),
                    egui::vec2(panel.columns[col], 20.0),
                );
                clipped_cell(
                    ui,
                    rect.shrink2(egui::vec2(7.0, 0.0)),
                    title,
                    Color32::from_gray(214),
                );
                let response = ui.interact(
                    rect.intersect(header),
                    ui.id().with(("column", col)),
                    egui::Sense::click(),
                );
                if response.clicked() {
                    if panel.sort_column == col {
                        panel.sort_reverse = !panel.sort_reverse;
                    } else {
                        panel.sort_column = col;
                        panel.sort_reverse = false;
                    }
                }
                let divider =
                    egui::Rect::from_center_size(rect.right_center(), egui::vec2(6.0, 20.0))
                        .intersect(header);
                let response =
                    ui.interact(divider, ui.id().with(("divider", col)), egui::Sense::drag());
                if response.dragged() {
                    panel.columns[col] =
                        (panel.columns[col] + response.drag_delta().x).clamp(60.0, 400.0);
                }
                response.on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                ui.painter().vline(
                    rect.right(),
                    header.y_range(),
                    egui::Stroke::new(1.0, Color32::from_gray(49)),
                );
                x += panel.columns[col];
            }
            ui.painter().rect_stroke(
                header,
                0.0,
                egui::Stroke::new(1.0, Color32::from_gray(45)),
                egui::StrokeKind::Inside,
            );
            let result = egui::ScrollArea::both()
                .id_salt("components_results_v2")
                .max_height(list_height - 42.0)
                .min_scrolled_height(list_height - 42.0)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    let width = panel.columns.iter().sum::<f32>().max(ui.available_width());
                    for index in &matches {
                        let symbol = &library.symbols[*index];
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(width, 18.0),
                            egui::Sense::click_and_drag(),
                        );
                        if *index == panel.selected {
                            ui.painter()
                                .rect_filled(rect, 0.0, Color32::from_rgb(62, 87, 104));
                        }
                        theme::draw_icon(
                            ui.painter(),
                            egui::Rect::from_center_size(
                                rect.left_center() + egui::vec2(10.0, 0.0),
                                egui::vec2(10.0, 14.0),
                            ),
                            theme::Icon::LibraryComponent,
                            true,
                        );
                        if response.clicked() || response.drag_started() {
                            if panel.selected != *index {
                                panel.part = 1;
                                panel.mode = 0;
                            }
                            panel.selected = *index;
                        }
                        if response.double_clicked() && can_place {
                            place = true;
                        }
                        if can_place
                            && panel.drag_document.is_none()
                            && (response.drag_started() || source_drag(ui, rect))
                        {
                            if panel.selected != *index {
                                panel.part = 1;
                                panel.mode = 0;
                            }
                            panel.selected = *index;
                            drag = true;
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
                        let mut x = rect.left();
                        for (col, text) in [
                            symbol.name.as_str(),
                            symbol.description.as_str(),
                            symbol_value(symbol),
                        ]
                        .iter()
                        .enumerate()
                        {
                            let pad = if col == 0 { 24.0 } else { 8.0 };
                            clipped_cell(
                                ui,
                                egui::Rect::from_min_max(
                                    egui::pos2(x + pad, rect.top()),
                                    egui::pos2(x + panel.columns[col] - 5.0, rect.bottom()),
                                ),
                                text,
                                Color32::from_gray(216),
                            );
                            x += panel.columns[col];
                        }
                    }
                    if filtered {
                        ui.add_space(20.0);
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.clip_rect().width(), 40.0),
                            egui::Sense::hover(),
                        );
                        ui.painter().text(
                            rect.center() - egui::vec2(0.0, 10.0),
                            egui::Align2::CENTER_CENTER,
                            "Need more components?",
                            egui::FontId::proportional(11.0),
                            Color32::from_gray(210),
                        );
                        ui.add_enabled_ui(false, |ui| {
                            ui.put(
                                egui::Rect::from_center_size(
                                    rect.center() + egui::vec2(0.0, 7.0),
                                    egui::vec2(rect.width(), 18.0),
                                ),
                                egui::Button::new(
                                    RichText::new("Find in Manufacturer Part Search …")
                                        .color(ACCENT),
                                )
                                .frame(false),
                            )
                            .on_disabled_hover_text("Manufacturer Part Search is not available");
                        });
                    }
                });
            panel.scroll_x = result.state.offset.x;
            let (r, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
            clipped_cell(
                ui,
                r.shrink2(egui::vec2(7.0, 0.0)),
                &format!("Results: {}", matches.len()),
                MUTED,
            );
            ui.painter().rect_stroke(
                r,
                0.0,
                egui::Stroke::new(1.0, Color32::from_gray(45)),
                egui::StrokeKind::Inside,
            );
        });
        let (split, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 8.0), egui::Sense::drag());
        if !panel.details_closed {
            if response.dragged() {
                panel.table_height = (panel.table_height + response.drag_delta().y)
                    .clamp(100.0, (available - 90.0).max(100.0));
            }
            response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        }
        ui.painter().hline(
            split.x_range(),
            split.center().y,
            egui::Stroke::new(1.0, Color32::from_gray(47)),
        );
        let name = library
            .symbols
            .get(panel.selected)
            .map_or("", |s| s.name.as_str());
        let mut details_open = !panel.details_closed;
        section_header(ui, "Details", Some(name), &mut details_open, true);
        panel.details_closed = !details_open;
        if details_open && !matches.is_empty() {
            let symbol = &library.symbols[panel.selected];
            egui::ScrollArea::vertical()
                .id_salt("components_details_v2")
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    egui::Frame::NONE
                        .inner_margin(egui::Margin {
                            left: 18,
                            right: 18,
                            top: 10,
                            bottom: 8,
                        })
                        .show(ui, |ui| {
                            let values = &symbol.variants[0].fragment.components[0].parameters;
                            let favorites_id = egui::Id::new("components_favorite_parameters");
                            let mut favorites = ui
                                .data_mut(|d| {
                                    d.get_persisted::<std::collections::BTreeSet<String>>(
                                        favorites_id,
                                    )
                                })
                                .unwrap_or_else(|| {
                                    [
                                        "Capacitance",
                                        "Capacitance Tolerance",
                                        "Manufacturer Package Code",
                                        "Package Code (M\\EIA)",
                                        "Package Code (mm\\inch)",
                                        "Package Type",
                                        "Power",
                                        "Resistance",
                                    ]
                                    .into_iter()
                                    .map(str::to_owned)
                                    .collect()
                                });
                            egui::Frame::NONE
                                .fill(Color32::from_gray(66))
                                .inner_margin(2)
                                .show(ui, |ui| {
                                    let rows: Vec<_> = detail_rows(values)
                                        .into_iter()
                                        .filter(|(n, _)| {
                                            panel.details_more || favorites.contains(n)
                                        })
                                        .collect();
                                    for (name, value) in rows {
                                        let (r, _) = ui.allocate_exact_size(
                                            egui::vec2(ui.available_width(), 21.3),
                                            egui::Sense::hover(),
                                        );
                                        let star_width =
                                            if panel.details_more { 18.0 } else { 0.0 };
                                        let column = r.left() + 120.0_f32.min(r.width() * 0.48);
                                        clipped_cell(
                                            ui,
                                            egui::Rect::from_min_max(
                                                r.min,
                                                egui::pos2(column - 4.0, r.bottom()),
                                            ),
                                            &name,
                                            Color32::from_gray(210),
                                        );
                                        clipped_cell(
                                            ui,
                                            egui::Rect::from_min_max(
                                                egui::pos2(column, r.top()),
                                                r.max - egui::vec2(star_width, 0.0),
                                            ),
                                            &value,
                                            Color32::from_gray(210),
                                        );
                                        if panel.details_more {
                                            let star_rect = egui::Rect::from_min_max(
                                                egui::pos2(r.right() - 18.0, r.top()),
                                                r.max,
                                            );
                                            let selected = favorites.contains(&name);
                                            let response = ui.interact(
                                                star_rect,
                                                ui.id().with((&name, "favorite")),
                                                egui::Sense::click(),
                                            );
                                            ui.painter().text(
                                                star_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "★",
                                                egui::FontId::proportional(12.0),
                                                if selected {
                                                    Color32::from_gray(220)
                                                } else {
                                                    Color32::from_gray(125)
                                                },
                                            );
                                            if response.clicked() {
                                                if selected {
                                                    favorites.remove(&name);
                                                } else {
                                                    favorites.insert(name);
                                                }
                                            }
                                        }
                                    }
                                    ui.add_space(5.0);
                                    let link = ui.link(if panel.details_more {
                                        "Show Less"
                                    } else {
                                        "Show More"
                                    });
                                    let at = link.rect.right_center() + egui::vec2(5.0, 0.0);
                                    let sign = if panel.details_more { -1.0 } else { 1.0 };
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![
                                            at + egui::vec2(-3.0, -sign),
                                            at + egui::vec2(3.0, -sign),
                                            at + egui::vec2(0.0, 3.0 * sign),
                                        ],
                                        ACCENT,
                                        egui::Stroke::NONE,
                                    ));
                                    if link.clicked() {
                                        panel.details_more = !panel.details_more;
                                    }
                                });
                            ui.data_mut(|d| d.insert_persisted(favorites_id, favorites));
                            ui.add_space(10.0);
                            let models_id = ui.id().with("models_open");
                            let mut models =
                                ui.data_mut(|d| d.get_persisted::<bool>(models_id).unwrap_or(true));
                            section_header(ui, "Models", None, &mut models, false);
                            ui.data_mut(|d| d.insert_persisted(models_id, models));
                            if models {
                                ui.add_space(11.0);
                                if let Some(variant) = symbol.variant(panel.part.max(1), panel.mode)
                                {
                                    let response =
                                        eda_render::symbol_preview(ui, &variant.fragment, 184.0);
                                    if can_place
                                        && panel.drag_document.is_none()
                                        && (response.drag_started() || source_drag(ui, response.rect))
                                    {
                                        drag = true;
                                    }
                                    if response.double_clicked() && can_place {
                                        place = true;
                                    }
                                    response.context_menu(|ui| {
                                        if ui
                                            .add_enabled(can_place, egui::Button::new("Place"))
                                            .clicked()
                                        {
                                            place = true;
                                            ui.close();
                                        }
                                        if symbol.parts > 1 {
                                            ui.horizontal(|ui| {
                                                ui.label("Part");
                                                ui.add(
                                                    egui::DragValue::new(&mut panel.part)
                                                        .range(1..=symbol.parts),
                                                );
                                            });
                                        }
                                        if symbol.display_modes > 1 {
                                            ui.horizontal(|ui| {
                                                ui.label("Display Mode");
                                                ui.add(
                                                    egui::DragValue::new(&mut panel.mode)
                                                        .range(0..=symbol.display_modes - 1),
                                                );
                                            });
                                        }
                                    });
                                }
                                ui.add_space(4.0);
                                ui.label(RichText::new(&symbol.name).size(10.0).color(MUTED));
                                ui.add_space(8.0);
                                ui.separator();
                                ui.add_space(8.0);
                                let (r, _) = ui.allocate_exact_size(
                                    egui::vec2(ui.available_width(), 150.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(r, 4.0, Color32::from_gray(232));
                                theme::draw_icon(
                                    ui.painter(),
                                    egui::Rect::from_center_size(
                                        r.center() - egui::vec2(0.0, 6.0),
                                        egui::vec2(28.0, 28.0),
                                    ),
                                    theme::Icon::Chip,
                                    false,
                                );
                                ui.painter().text(
                                    r.center() + egui::vec2(0.0, 26.0),
                                    egui::Align2::CENTER_CENTER,
                                    if library.embedded_footprints == Some(false) {
                                        "Footprint is not present"
                                    } else {
                                        "Footprint preview unavailable"
                                    },
                                    egui::FontId::proportional(11.0),
                                    Color32::GRAY,
                                );
                                for model in
                                    &symbol.variants[0].fragment.components[0].settings.models
                                {
                                    ui.label(RichText::new(&model.name).size(10.0).color(MUTED));
                                }
                            }
                            ui.add_space(8.0);
                            let mut references = ui.data_mut(|d| {
                                d.get_persisted::<bool>(ui.id().with("references"))
                                    .unwrap_or(true)
                            });
                            section_header(ui, "References", None, &mut references, false);
                            ui.data_mut(|d| {
                                d.insert_persisted(ui.id().with("references"), references)
                            });
                            if references {
                                reference_links(ui, values);
                            }
                            for (title, message) in [
                                ("Part Choices", "Get an accurate and up to date view of your supply chain"),
                                ("Where Used", "View projects that use this component and usage history with Workspace"),
                            ] {
                                workspace_banner(ui, title, message);
                            }
                        });
                });
        }
        if (place || drag)
            && let Some(symbol) = library.symbols.get(panel.selected)
        {
            let (symbol, part, mode) = (symbol.clone(), panel.part.max(1), panel.mode);
            if drag {
                self.start_library_drag(symbol, part, mode, ui.ctx());
            } else {
                self.start_library_tool(symbol, part, mode);
            }
        }
    }
}

fn symbol_value(symbol: &eda_core::library::Symbol) -> &str {
    symbol.variants[0].fragment.components[0]
        .parameters
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Value"))
        .map_or("", |(_, value)| value.as_str())
}

fn reference_links(ui: &mut egui::Ui, values: &std::collections::BTreeMap<String, String>) {
    egui::Frame::NONE
        .fill(Color32::from_gray(66))
        .inner_margin(4)
        .show(ui, |ui| {
            for (name, value) in values
                .iter()
                .filter(|(n, _)| n.starts_with("ComponentLink") && n.ends_with("Description"))
            {
                let key = name.trim_end_matches("Description").to_owned() + "URL";
                if let Some(url) = values.get(&key) {
                    ui.add_enabled(
                        false,
                        egui::Button::new(RichText::new(value).color(ACCENT)).frame(false),
                    )
                    .on_disabled_hover_text(url);
                }
            }
        });
}
fn workspace_banner(ui: &mut egui::Ui, title: &str, message: &str) {
    ui.add_space(8.0);
    let id = ui.id().with(title);
    let mut open = ui.data_mut(|d| d.get_persisted::<bool>(id).unwrap_or(true));
    section_header(ui, title, None, &mut open, false);
    ui.data_mut(|d| d.insert_persisted(id, open));
    if open {
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 78.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::from_gray(66));
        let painter = ui
            .painter()
            .with_clip_rect(rect.shrink(2.0).intersect(ui.clip_rect()));
        painter.text(
            rect.center() - egui::vec2(0.0, 7.0),
            egui::Align2::CENTER_CENTER,
            message,
            egui::FontId::proportional(11.0),
            Color32::from_gray(210),
        );
        ui.add_enabled_ui(false, |ui| {
            ui.put(
                egui::Rect::from_center_size(
                    rect.center() + egui::vec2(0.0, 9.0),
                    egui::vec2(70.0, 18.0),
                ),
                egui::Button::new(RichText::new("Learn More").color(ACCENT)).frame(false),
            )
            .on_disabled_hover_text("Workspace integration is not available");
        });
    }
}

fn clipped_cell(ui: &egui::Ui, rect: egui::Rect, text: &str, color: Color32) {
    if rect.width() <= 0.0 || !rect.intersects(ui.clip_rect()) {
        return;
    }
    let mut job = egui::text::LayoutJob::simple(
        text.into(),
        egui::FontId::proportional(11.0),
        color,
        rect.width(),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .galley(
            rect.left_center() - egui::vec2(0.0, galley.size().y * 0.5),
            galley,
            color,
        );
}
fn section_header(
    ui: &mut egui::Ui,
    title: &str,
    suffix: Option<&str>,
    open: &mut bool,
    outer: bool,
) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), if outer { 26.0 } else { 24.0 }),
        egui::Sense::hover(),
    );
    let response = ui.interact(
        if outer {
            rect
        } else {
            egui::Rect::from_min_max(rect.min - egui::vec2(12.0, 0.0), rect.max)
        },
        ui.id().with(("components_section", title)),
        egui::Sense::click(),
    );
    if response.clicked() {
        *open = !*open;
    }
    if outer {
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, Color32::from_gray(45)),
            egui::StrokeKind::Inside,
        );
    }
    let centre = rect.left_center() + egui::vec2(if outer { 10.0 } else { -5.0 }, 0.0);
    let points = if *open {
        vec![
            centre + egui::vec2(-3.0, -1.0),
            centre + egui::vec2(3.0, -1.0),
            centre + egui::vec2(0.0, 3.0),
        ]
    } else {
        vec![
            centre + egui::vec2(-2.0, -3.0),
            centre + egui::vec2(-2.0, 3.0),
            centre + egui::vec2(2.0, 0.0),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        points,
        Color32::from_gray(208),
        egui::Stroke::NONE,
    ));
    let galley = ui.painter().layout_no_wrap(
        title.into(),
        egui::FontId::new(11.0, egui::FontFamily::Name("ui-bold".into())),
        Color32::from_gray(226),
    );
    let width = galley.size().x;
    ui.painter().galley(
        rect.left_center() + egui::vec2(if outer { 18.0 } else { 1.0 }, -galley.size().y * 0.5),
        galley,
        Color32::from_gray(226),
    );
    if let Some(suffix) = suffix {
        ui.painter().text(
            rect.left_center() + egui::vec2(22.0 + width, 0.0),
            egui::Align2::LEFT_CENTER,
            suffix,
            egui::FontId::proportional(11.0),
            MUTED,
        );
    }
}
fn source_drag(ui: &egui::Ui, rect: egui::Rect) -> bool {
    let rect = rect.intersect(ui.clip_rect());
    let source = |p| rect.contains(p) && ui.ctx().layer_id_at(p) == Some(ui.layer_id());
    let (held, press_origin, buttons) = ui.input(|i| {
        (
            i.pointer.primary_down() && i.pointer.is_decidedly_dragging(),
            i.pointer.press_origin(),
            i.events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        ..
                    } => Some((*pos, *pressed)),
                    _ => None,
                })
                .collect::<Vec<_>>(),
        )
    });
    // A ScrollArea can own the drag response after a press in its contents.
    // Resolve layers outside the input lock so the card still owns placement.
    if held && press_origin.is_some_and(source) {
        return true;
    }
    let mut origin = None;
    for (pos, pressed) in buttons {
        if pressed {
            origin = source(pos).then_some(pos);
        } else if origin.is_some_and(|p| p.distance(pos) > 6.0) {
            return true;
        }
    }
    false
}
fn detail_rows(values: &std::collections::BTreeMap<String, String>) -> Vec<(String, String)> {
    let mut rows: Vec<_> = values
        .iter()
        .filter(|(n, _)| !n.eq_ignore_ascii_case("Comment") && !n.starts_with("ComponentLink"))
        .map(|(n, v)| (n.clone(), v.clone()))
        .collect();
    rows.sort_by_cached_key(|(n, _)| {
        n.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    });
    rows
}
