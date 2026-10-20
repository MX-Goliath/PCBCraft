use super::*;
use eda_render::SnapMode;
use egui::{Align2, FontFamily, FontId, Sense, Stroke, Vec2};

#[derive(Default)]
pub(super) struct PropertiesUi {
    pub(super) search: String,
    parameters: bool,
    pub focused: bool,
    pub(super) schematic: super::schematic_properties::Panel,
}

pub(super) fn section(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let id = ui.make_persistent_id(title);
    let mut open = ui.data_mut(|d| d.get_persisted::<bool>(id).unwrap_or(true));
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::click());
    if response.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    let centre = rect.left_center() + Vec2::new(4.0, 0.0);
    let vertices = if open {
        vec![
            centre + Vec2::new(-2.0, 0.0),
            centre + Vec2::new(2.0, 0.0),
            centre + Vec2::new(0.0, 3.0),
        ]
    } else {
        vec![
            centre + Vec2::new(-1.0, -2.0),
            centre + Vec2::new(2.0, 0.0),
            centre + Vec2::new(-1.0, 2.0),
        ]
    };
    ui.painter().add(egui::Shape::convex_polygon(
        vertices,
        Color32::LIGHT_GRAY,
        Stroke::NONE,
    ));
    ui.painter().text(
        rect.left_center() + Vec2::new(10.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        FontId::new(11.5, FontFamily::Name("ui-bold".into())),
        Color32::from_gray(230),
    );
    if open {
        egui::Frame::NONE
            .fill(theme::GROUP)
            .inner_margin(6)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                contents(ui);
            });
    }
    ui.add_space(6.0);
}

fn row(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(104.0, 23.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 23.0),
            egui::Layout::left_to_right(egui::Align::Center),
            content,
        );
    });
}

fn unsupported(ui: &mut egui::Ui, text: &str) {
    ui.add_enabled(
        false,
        egui::Button::new(text).min_size(Vec2::new(ui.available_width(), 22.0)),
    )
    .on_disabled_hover_text("Not supported in this version");
}

fn snap_checkbox(ui: &mut egui::Ui, label: &str, value: Option<&mut bool>) {
    let enabled = value.is_some();
    let checked = value.as_ref().is_some_and(|v| **v);
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 21.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if response.clicked()
        && let Some(value) = value
    {
        *value = !*value;
    }
    let square =
        egui::Rect::from_center_size(rect.left_center() + Vec2::new(16.0, 0.0), Vec2::splat(14.0));
    ui.painter().rect(
        square,
        2.0,
        Color32::from_gray(72),
        Stroke::new(1.0, Color32::from_gray(32)),
        egui::StrokeKind::Inside,
    );
    if checked {
        ui.painter().line(
            vec![
                square.min + Vec2::new(3.0, 7.0),
                square.min + Vec2::new(6.0, 10.0),
                square.min + Vec2::new(11.0, 3.0),
            ],
            Stroke::new(1.2, Color32::LIGHT_GRAY),
        );
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(52.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::from_gray(if enabled { 212 } else { 150 }),
    );
    if !enabled {
        response.on_hover_text("Not supported in this version");
    }
}

fn field(ui: &mut egui::Ui, value: &mut f64) -> bool {
    let response = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::DragValue::new(value)
            .speed(0.01)
            .max_decimals(6)
            .suffix("mm"),
    );
    response.lost_focus() || response.drag_stopped()
}

impl PcbCraft {
    pub(super) fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.active else {
            ui.label("No document selected");
            return;
        };
        if self.documents[index].model.kind == DocumentKind::Schematic {
            self.schematic_properties(ui, index);
            return;
        }
        let pad = self.documents[index]
            .view
            .selected
            .and_then(|id| self.documents[index].model.pads.iter().find(|p| p.id == id))
            .cloned();
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(98.0, 27.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(if pad.is_some() {
                        "Pad"
                    } else if self.documents[index].model.kind == DocumentKind::Board {
                        "Board"
                    } else {
                        "Schematic"
                    });
                },
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                theme::icon_button(ui, theme::Icon::Filter, false, false)
                    .on_disabled_hover_text("Selection context menu is not available yet");
            });
        });
        ui.add_space(3.0);
        theme::search_field(ui, &mut self.property_ui.search, ui.available_width());
        ui.add_space(10.0);
        if pad.is_none() {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(!self.property_ui.parameters, "General")
                    .clicked()
                {
                    self.property_ui.parameters = false;
                }
                if ui
                    .selectable_label(self.property_ui.parameters, "Parameters")
                    .clicked()
                {
                    self.property_ui.parameters = true;
                }
                ui.add_enabled(false, egui::Button::new("Health Check").frame(true))
                    .on_disabled_hover_text("Engineering health checks are not implemented");
            });
            ui.separator();
        }
        let query = self.property_ui.search.to_lowercase();
        egui::TopBottomPanel::bottom("properties_selection_status")
            .exact_height(18.0)
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| {
                ui.label(
                    egui::RichText::new(if pad.is_some() {
                        "1 object selected"
                    } else {
                        "Nothing selected"
                    })
                    .size(11.0),
                );
            });
        egui::ScrollArea::vertical()
            .id_salt(if pad.is_some() {
                "pad_properties"
            } else {
                "board_properties"
            })
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if let Some(pad) = pad {
                    self.pad_properties(ui, index, &pad, &query);
                } else if self.property_ui.parameters {
                    section(ui, "Parameters", |ui| {
                        let doc = &self.documents[index].model;
                        row(ui, "Name", |ui| {
                            ui.label(&doc.name);
                        });
                        row(ui, "Document ID", |ui| {
                            ui.label(doc.id.to_string());
                        });
                        row(ui, "Schema Version", |ui| {
                            ui.label(doc.schema_version.to_string());
                        });
                    });
                } else {
                    self.board_properties(ui, index, &query);
                }
            });
    }

    fn board_properties(&mut self, ui: &mut egui::Ui, index: usize, query: &str) {
        self.pad_edit = None;
        if query.is_empty() || "selection filter pads custom".contains(query) {
            section(ui, "Selection Filter", |ui| {
                egui::ComboBox::from_id_salt("selection_filter_mode")
                    .selected_text("Custom")
                    .width(75.0)
                    .show_ui(ui, |ui| {
                        if ui.button("All supported objects").clicked() {
                            self.documents[index].view.select_pads = true;
                            ui.close();
                        }
                        if ui.button("Clear").clicked() {
                            self.documents[index].view.select_pads = false;
                            self.documents[index].view.selected = None;
                            ui.close();
                        }
                    });
                ui.add_space(3.0);
                for labels in [
                    &["Components", "3D Bodies", "Keepouts"][..],
                    &["Tracks", "Arcs", "Pads", "Vias", "Regions"][..],
                    &["Polygons", "Fills", "Texts", "Rooms", "Other"][..],
                ] {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 3.0;
                        ui.spacing_mut().button_padding = Vec2::new(7.0, 5.0);
                        for label in labels {
                            if *label == "Pads" {
                                let enabled = self.documents[index].view.select_pads;
                                if ui
                                    .add(egui::Button::new(*label).selected(enabled))
                                    .clicked()
                                {
                                    self.documents[index].view.select_pads = !enabled;
                                    if enabled {
                                        self.documents[index].view.selected = None;
                                    }
                                }
                            } else {
                                ui.add_enabled(false, egui::Button::new(*label))
                                    .on_disabled_hover_text(
                                        "Selection of this object type is not supported yet",
                                    );
                            }
                        }
                    });
                }
                ui.add_space(7.0);
            });
        }
        if query.is_empty() || "snap options grids guides axes objects distance".contains(query) {
            section(ui, "Snap Options", |ui| {
                ui.spacing_mut().interact_size.y = 24.0;
                ui.horizontal(|ui| {
                    if ui
                        .add(egui::Button::new("Grids").selected(self.settings.show_grid))
                        .clicked()
                    {
                        self.command = Some(Command::ToggleGrid);
                    }
                    ui.add_enabled(false, egui::Button::new("Guides"));
                    let view = &mut self.documents[index].view;
                    if ui
                        .add(egui::Button::new("Axes").selected(view.show_axes))
                        .clicked()
                    {
                        view.show_axes = !view.show_axes;
                    }
                });
                ui.label("Snapping");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let width = ui.available_width() / 3.0;
                    let snap = &mut self.documents[index].view.snap;
                    for (mode, label) in [
                        (SnapMode::AllLayers, "All Layers"),
                        (SnapMode::CurrentLayer, "Current Layer"),
                        (SnapMode::Off, "Off"),
                    ] {
                        if ui
                            .add_sized(
                                [width, 24.0],
                                egui::Button::new(label).selected(snap.mode == mode),
                            )
                            .clicked()
                        {
                            snap.mode = mode;
                        }
                    }
                });
                ui.label("Objects for snapping");
                egui::Frame::NONE
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized([48.0, 18.0], egui::Label::new("On/Off"));
                            ui.label("Objects");
                        });
                        let snap = &mut self.documents[index].view.snap;
                        let rows = [
                            "Track/Arcs Vertices",
                            "Track/Arcs Lines",
                            "Arc Centers",
                            "Intersections",
                            "Pad Centers",
                            "Pad Vertices",
                            "Pad Edges",
                            "Via Centers",
                            "Regions/Polygons/Fills",
                            "Board Shape",
                            "Footprint Origins",
                            "Text Origins",
                            "3D Body Snap Points",
                        ];
                        egui::ScrollArea::vertical()
                            .id_salt("snap_objects")
                            .max_height(245.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for (number, label) in rows.iter().enumerate() {
                                    egui::Frame::NONE
                                        .fill(if number % 2 == 0 {
                                            Color32::from_gray(63)
                                        } else {
                                            Color32::from_gray(59)
                                        })
                                        .inner_margin(0)
                                        .show(ui, |ui| {
                                            ui.set_min_width(ui.available_width());
                                            let option = match number {
                                                0 => Some(&mut snap.track_vertices),
                                                4 => Some(&mut snap.pad_centres),
                                                5 => Some(&mut snap.pad_vertices),
                                                6 => Some(&mut snap.pad_edges),
                                                _ => None,
                                            };
                                            snap_checkbox(ui, label, option);
                                        });
                                }
                            });
                    });
                ui.add_space(5.0);
                row(ui, "Snap Distance", |ui| {
                    let snap = &mut self.documents[index].view.snap;
                    let mut mm = snap.distance_nm as f64 / 1e6;
                    if ui
                        .add_sized(
                            [ui.available_width(), 22.0],
                            egui::DragValue::new(&mut mm)
                                .speed(0.01)
                                .range(0.0..=100.0)
                                .suffix("mm"),
                        )
                        .changed()
                    {
                        snap.distance_nm = (mm * 1e6).round() as i64;
                    }
                });
                row(ui, "Axis Snap Range", |ui| {
                    unsupported(ui, "N/A");
                });
            });
        }
        if query.is_empty()
            || "board information size horizontal vertical area components".contains(query)
        {
            section(ui, "Board Information", |ui| {
                ui.label(RichText::new("Board Size").family(FontFamily::Name("ui-bold".into())));
                let doc = &self.documents[index].model;
                let units = self.settings.units;
                for (label, value) in [
                    (
                        "Horizontal",
                        format!("{:.3}{}", units.from_nm(doc.size.x as f64), units.label()),
                    ),
                    (
                        "Vertical",
                        format!("{:.3}{}", units.from_nm(doc.size.y as f64), units.label()),
                    ),
                    (
                        "Area",
                        format!(
                            "{:.3} sq.mm",
                            doc.size.x as f64 / 1e6 * (doc.size.y as f64 / 1e6)
                        ),
                    ),
                ] {
                    row(ui, label, |ui| {
                        ui.label(value);
                    });
                }
                row(ui, "Components Area", |ui| {
                    ui.add_enabled(false, egui::Label::new("N/A"));
                });
            });
        }
    }

    fn pad_properties(
        &mut self,
        ui: &mut egui::Ui,
        index: usize,
        pad: &eda_core::Pad,
        query: &str,
    ) {
        if self.pad_edit.as_ref().is_none_or(|e| e.id != pad.id) {
            self.pad_edit = Some(PadEdit {
                id: pad.id,
                name: pad.name.clone(),
                x: pad.position.x as f64 / 1e6,
                y: pad.position.y as f64 / 1e6,
                width: pad.size.x as f64 / 1e6,
                height: pad.size.y as f64 / 1e6,
                layer: pad.layer,
            });
        }
        if query.is_empty() || "net information delay length class".contains(query) {
            section(ui, "Net Information", |ui| {
                ui.horizontal(|ui| {
                    ui.add_space(90.0);
                    ui.label("Name");
                    ui.add_space(50.0);
                    ui.label("Class");
                });
                row(ui, "Net", |ui| {
                    ui.label("N/A");
                    ui.add_space(45.0);
                    ui.label("N/A");
                });
                ui.add_space(14.0);
                row(ui, "", |ui| {
                    ui.label("Selected");
                    ui.add_space(30.0);
                    ui.label("Total");
                });
                for label in ["Delay", "Length"] {
                    row(ui, label, |ui| {
                        ui.add_enabled(false, egui::Label::new("N/A"));
                    });
                }
            });
        }
        let mut commit = false;
        let edit = self.pad_edit.as_mut().unwrap();
        if query.is_empty() || "properties designator layer net electrical type propagation pin package length jumper template library position rotation".contains(query) {
            section(ui,"Properties",|ui| {
                row(ui,"Designator",|ui| {
                    let response=ui.add_sized([ui.available_width(),22.0],egui::TextEdit::singleline(&mut edit.name));
                    commit |= response.lost_focus();
                });
                row(ui,"Layer",|ui| {
                    egui::ComboBox::from_id_salt("pad_layer").width(ui.available_width()-8.0).selected_text(edit.layer.label()).show_ui(ui,|ui| {
                        for layer in [Layer::Top,Layer::Bottom] {if ui.selectable_value(&mut edit.layer,layer,layer.label()).changed(){commit=true;}}
                    });
                });
                for (label,value) in [("Net","No Net"),("Electrical Type","N/A"),("Propagation Delay","N/A"),("Pin Package Length","N/A"),("Jumper","N/A")] {row(ui,label,|ui|{unsupported(ui,value);});}
                ui.add_space(7.0);ui.separator();ui.add_space(6.0);
                row(ui,"Template",|ui|{unsupported(ui,"N/A");});
                row(ui,"Library",|ui|{ui.label("<Local>");});
                ui.add_space(7.0);ui.separator();ui.add_space(6.0);
                row(ui,"(X/Y)",|ui| {
                    let width=(ui.available_width()-4.0)/2.0;
                    ui.allocate_ui(Vec2::new(width,23.0),|ui|{commit |= field(ui,&mut edit.x);});
                    ui.allocate_ui(Vec2::new(width,23.0),|ui|{commit |= field(ui,&mut edit.y);});
                });
                row(ui,"Rotation",|ui|{unsupported(ui,"0.000");});
            });
        }
        if query.is_empty() || "pad stack copper size shape".contains(query) {
            section(ui, "Pad Stack", |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 265.0), Sense::hover());
                ui.painter()
                    .rect_filled(rect, 3.0, Color32::from_rgb(29, 29, 29));
                let scale = (rect.width() - 65.0).min(rect.height() - 65.0)
                    / (edit.width.max(edit.height).max(0.001) as f32);
                let size = Vec2::new(edit.width as f32, edit.height as f32) * scale;
                let centre = rect.center();
                let points = vec![
                    centre + Vec2::new(-size.x / 2.0, 0.0),
                    centre + Vec2::new(0.0, -size.y / 3.0),
                    centre + Vec2::new(size.x / 2.0, 0.0),
                    centre + Vec2::new(0.0, size.y / 3.0),
                ];
                ui.painter().add(egui::Shape::convex_polygon(
                    points,
                    layer_color(edit.layer),
                    Stroke::new(1.0, Color32::from_gray(130)),
                ));
                ui.painter().text(
                    rect.left_top() + Vec2::new(9.0, 9.0),
                    Align2::LEFT_TOP,
                    "Pad preview",
                    FontId::proportional(11.0),
                    ACCENT,
                );
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let width = ui.available_width() / 3.0;
                    let _ = ui.add_sized([width, 22.0], egui::Button::new("Simple").selected(true));
                    ui.add_enabled_ui(false, |ui| {
                        ui.add_sized(
                            [width, 22.0],
                            egui::Button::new("Top-Middle-Bottom").truncate(),
                        );
                        ui.add_sized([width, 22.0], egui::Button::new("Full Stack"));
                    });
                });
                ui.add_space(3.0);
                egui::Grid::new("pad_stack_grid")
                    .num_columns(4)
                    .striped(true)
                    .spacing([7.0, 4.0])
                    .show(ui, |ui| {
                        for header in ["COPPER", "X-Size", "Y-Size", "Shape"] {
                            ui.label(header);
                        }
                        ui.end_row();
                        ui.label(edit.layer.label());
                        let x = ui.add(
                            egui::DragValue::new(&mut edit.width)
                                .speed(0.01)
                                .range(0.000001..=1000.0)
                                .suffix("mm"),
                        );
                        commit |= x.lost_focus() || x.drag_stopped();
                        let y = ui.add(
                            egui::DragValue::new(&mut edit.height)
                                .speed(0.01)
                                .range(0.000001..=1000.0)
                                .suffix("mm"),
                        );
                        commit |= y.lost_focus() || y.drag_stopped();
                        ui.label("Rectangular");
                        ui.end_row();
                    });
            });
        }
        if commit {
            let edit = self.pad_edit.as_ref().unwrap();
            if ![edit.x, edit.y, edit.width, edit.height]
                .iter()
                .all(|v| v.is_finite())
            {
                self.message(true, "Pad values must be finite");
                return;
            }
            let mut next = self.documents[index].model.clone();
            let target = next.pads.iter_mut().find(|p| p.id == edit.id).unwrap();
            target.name = edit.name.clone();
            target.layer = edit.layer;
            target.position = eda_geom::Point {
                x: (edit.x * 1e6).round() as i64,
                y: (edit.y * 1e6).round() as i64,
            };
            target.size = eda_geom::Point {
                x: (edit.width * 1e6).round() as i64,
                y: (edit.height * 1e6).round() as i64,
            };
            let doc = &mut self.documents[index];
            if let Err(error) = doc.history.commit(&mut doc.model, next) {
                self.message(true, error);
            }
        }
    }
}
