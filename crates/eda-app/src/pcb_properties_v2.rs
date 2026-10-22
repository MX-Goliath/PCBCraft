//! Contextual PCB Properties, observed in Altium Designer 24.4.1.
//! Drafts commit on field completion; all model edits validate through History.
use super::super::schematic_properties::{readonly, row, row_height, small_icon};
use super::*;
use egui::{Align2, FontId, Sense, Stroke, Vec2};

#[path = "pcb_stack_properties.rs"]
mod stacks;

#[derive(Default)]
pub(super) struct Panel {
    search: String,
    parameters: bool,
    parameter_group: usize,
    context: Option<Uuid>,
    hide_preview: bool,
    preview: eda_render::pcb_stack::PreviewState,
    rule_editor: Option<stacks::RuleEditor>,
}
fn section(ui: &mut egui::Ui, title: &str, query: &str, body: impl FnOnce(&mut egui::Ui)) {
    if query.is_empty()
        || title.to_lowercase().contains(query)
        || matches!(title, "Properties" | "Definition" | "Parameters")
    {
        properties::section(ui, title, body);
        ui.add_space(6.0);
    }
}
fn disabled(ui: &mut egui::Ui, label: &str) {
    ui.add_enabled(
        false,
        egui::Button::new(label).min_size(Vec2::new(ui.available_width(), 22.0)),
    )
    .on_disabled_hover_text("Not supported yet");
}
fn read_row(ui: &mut egui::Ui, label: &str, value: &str) {
    row(ui, label, |ui| readonly(ui, value));
}
fn completed<T: Clone + Default + Send + Sync + 'static>(
    ui: &mut egui::Ui,
    r: &egui::Response,
    value: &mut T,
    before: T,
) -> bool {
    let id = r.id.with("pcb_field_original");
    if r.gained_focus()
        || ((r.has_focus() || r.clicked()) && ui.data(|d| d.get_temp::<T>(id).is_none()))
    {
        ui.data_mut(|d| d.insert_temp(id, before));
    }
    if (r.has_focus() || r.lost_focus()) && ui.input(|i| i.key_pressed(Key::Escape)) {
        if let Some(original) = ui.data_mut(|d| d.remove_temp::<T>(id)) {
            *value = original;
        }
        r.surrender_focus();
        return false;
    }
    let done = r.lost_focus()
        || r.drag_stopped()
        || (r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
    if done {
        ui.data_mut(|d| d.remove::<T>(id));
    }
    done
}
fn text(ui: &mut egui::Ui, value: &mut String) -> bool {
    let before = value.clone();
    let r = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::TextEdit::singleline(value),
    );
    completed(ui, &r, value, before)
}
fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let mut done = false;
    row(ui, label, |ui| done = text(ui, value));
    done
}

fn angle(ui: &mut egui::Ui, label: &str, value: &mut f64) -> bool {
    let mut done = false;
    row(ui, label, |ui| {
        let before = *value;
        let r = ui.add_sized(
            [ui.available_width(), 22.0],
            egui::DragValue::new(&mut *value).speed(1.0).max_decimals(3),
        );
        done = completed(ui, &r, value, before);
    });
    done
}
fn numeric(ui: &mut egui::Ui, value: &mut i64, units: Units) -> bool {
    let before = *value;
    let scale = if units == Units::Mils { 25_400.0 } else { 1e6 };
    let mut n = *value as f64 / scale;
    let r = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::DragValue::new(&mut n)
            .range(-100_000_000_000.0 / scale..=100_000_000_000.0 / scale)
            .speed(if units == Units::Mils { 1.0 } else { 0.01 })
            .max_decimals(3)
            .suffix(units.label()),
    );
    if r.changed() && n.is_finite() {
        *value = (n * scale).round() as i64;
    }
    completed(ui, &r, value, before)
}
fn numeric_row(ui: &mut egui::Ui, label: &str, value: &mut i64, units: Units) -> bool {
    let mut done = false;
    row(ui, label, |ui| done = numeric(ui, value, units));
    done
}

fn tabs(ui: &mut egui::Ui, second: &mut bool, _a: &str, _b: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (value, label, width) in [(false, "General", 54.0), (true, "Parameters", 70.0)] {
            if ui
                .add_sized(
                    [width, 22.0],
                    egui::Button::new(label).fill(Color32::from_gray(if *second == value {
                        78
                    } else {
                        58
                    })),
                )
                .clicked()
            {
                *second = value;
            }
        }
    });
}
fn description(ui: &mut egui::Ui, id: &str, value: &str) {
    row_height(ui, "Description", 60.0, |ui| {
        egui::Frame::NONE
            .fill(Color32::from_gray(45))
            .inner_margin(3)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt(("description", id))
                    .max_height(54.0)
                    .min_scrolled_height(54.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(egui::Label::new(value).wrap());
                    });
            });
    });
}

fn origin(board: &Board) -> Point {
    let dimension = |key: &str| {
        board
            .properties
            .get(key)
            .and_then(|v| eda_io::altium_pcb::dimension(v).ok())
            .unwrap_or(0)
    };
    Point {
        x: dimension("ORIGINX"),
        y: dimension("ORIGINY"),
    }
}
fn coordinates(ui: &mut egui::Ui, label: &str, p: &mut Point, offset: Point, units: Units) -> bool {
    let mut local = Point {
        x: p.x - offset.x,
        y: p.y - offset.y,
    };
    let mut done = false;
    row(ui, label, |ui| {
        let w = (ui.available_width() - 4.0) / 2.0;
        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
            done |= numeric(ui, &mut local.x, units)
        });
        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
            done |= numeric(ui, &mut local.y, units)
        });
    });
    *p = Point {
        x: local.x + offset.x,
        y: local.y + offset.y,
    };
    done
}
fn location(
    ui: &mut egui::Ui,
    p: &mut Point,
    rotation: Option<&mut f64>,
    locked: &mut bool,
    offset: Point,
    units: Units,
) -> bool {
    let mut done = false;
    ui.horizontal_top(|ui| {
        let width = ui.available_width() - 30.0;
        ui.allocate_ui_with_layout(
            egui::vec2(width, if rotation.is_some() { 48.0 } else { 22.0 }),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.add_enabled_ui(!*locked, |ui| {
                    done |= coordinates(ui, "(X/Y)", p, offset, units);
                    if let Some(r) = rotation {
                        done |= angle(ui, "Rotation", r);
                    }
                });
            },
        );
        done |= small_icon(ui, "lock", locked);
    });
    done
}
fn layer_combo(
    ui: &mut egui::Ui,
    board: &Board,
    value: &mut u16,
    copper: bool,
    component: bool,
) -> bool {
    let mut changed = false;
    let name = board
        .layers
        .iter()
        .find(|l| l.id == *value)
        .map(|l| l.name.clone())
        .unwrap_or_else(|| pcb::layer_name(*value));
    let response = egui::ComboBox::from_id_salt("layer")
        .truncate()
        .width((ui.available_width() - 8.0).max(20.0))
        .selected_text(format!("   {name}"))
        .show_ui(ui, |ui| {
            for l in board.layers.iter().filter(|l| {
                (!copper || l.copper) && (!component || matches!(l.id, pcb::TOP | pcb::BOTTOM))
            }) {
                changed |= ui.selectable_value(value, l.id, &l.name).changed();
            }
        })
        .response;
    let r = response.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            r.min + egui::vec2(2.0, 1.0),
            egui::vec2(8.0, r.height() - 2.0),
        ),
        1.0,
        eda_render::pcb::layer_color(*value),
    );
    changed
}
fn layer(ui: &mut egui::Ui, board: &Board, value: &mut u16, copper: bool) -> bool {
    let mut changed = false;
    row(ui, "Layer", |ui| {
        changed = layer_combo(ui, board, value, copper, false)
    });
    changed
}

fn net(ui: &mut egui::Ui, board: &Board, value: &mut Option<u32>) -> bool {
    let mut changed = false;
    row(ui, "Net", |ui| {
        egui::ComboBox::from_id_salt("net")
            .truncate()
            .width((ui.available_width() - 8.0).max(20.0))
            .selected_text(if value.is_none() {
                "No Net"
            } else {
                board.net_name(*value)
            })
            .show_ui(ui, |ui| {
                changed |= ui.selectable_value(value, None, "No Net").changed();
                for n in &board.nets {
                    changed |= ui.selectable_value(value, Some(n.index), &n.name).changed();
                }
            });
    });
    changed
}
fn context_row(ui: &mut egui::Ui, context: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::hover());
    let p = ui.painter();
    p.with_clip_rect(egui::Rect::from_min_size(r.min, egui::vec2(104.0, 30.0)))
        .text(
            r.left_center(),
            Align2::LEFT_CENTER,
            context,
            FontId::proportional(12.0),
            Color32::from_gray(220),
        );
    let f = egui::Rect::from_min_size(
        egui::pos2(r.right() - 42.0, r.top() + 3.0),
        egui::vec2(28.0, 24.0),
    );
    p.rect_filled(f, 1.0, ACCENT);
    theme::draw_icon(p, f.shrink(5.0), theme::Icon::Filter, true);
    p.with_clip_rect(egui::Rect::from_min_max(
        egui::pos2(r.left() + 106.0, r.top()),
        f.left_bottom(),
    ))
    .text(
        egui::pos2(r.left() + 106.0, r.center().y),
        Align2::LEFT_CENTER,
        "Components (and 12 more)",
        FontId::proportional(11.0),
        Color32::from_gray(212),
    );
    let filter_response = ui.interact(f, ui.id().with("context_filter"), Sense::hover());
    filter_response
        .on_hover_text("Context filtering menu is not supported yet; use Board > Selection Filter");
    p.text(
        egui::pos2(r.right() - 7.0, r.center().y),
        Align2::CENTER_CENTER,
        "▾",
        FontId::proportional(12.0),
        Color32::GRAY,
    );
}
fn length(item: &Item) -> f64 {
    match item.shape {
        Shape::Track { start, end, .. } => pcb::distance_sq(start, end).sqrt(),
        Shape::Arc {
            radius,
            start_angle,
            end_angle,
            ..
        } => radius as f64 * pcb::arc_sweep(start_angle, end_angle).to_radians(),
        _ => 0.0,
    }
}
fn net_information(ui: &mut egui::Ui, board: &Board, item: &Item, units: Units, query: &str) {
    section(ui, "Net Information", query, |ui| {
        row(ui, "", |ui| {
            ui.add_sized([ui.available_width() * 0.5, 22.0], egui::Label::new("Name"));
            ui.label("Class");
        });
        row(ui, "Net", |ui| {
            ui.add_sized(
                [ui.available_width() * 0.5, 22.0],
                egui::Label::new(if item.net.is_none() {
                    "N/A"
                } else {
                    board.net_name(item.net)
                })
                .truncate(),
            );
            ui.add_enabled(false, egui::Label::new("N/A"));
        });
        ui.add_space(14.0);
        row(ui, "", |ui| {
            ui.add_sized(
                [ui.available_width() * 0.5, 22.0],
                egui::Label::new("Selected"),
            );
            ui.label("Total");
        });
        row(ui, "Delay", |ui| {
            ui.add_enabled(false, egui::Label::new("N/A"));
        });
        row(ui, "Length", |ui| {
            let value = format!("{:.3}{}", units.from_nm(length(item)), units.label());
            ui.add_sized(
                [ui.available_width() * 0.5, 22.0],
                egui::Label::new(value).truncate(),
            );
            ui.add_enabled(false, egui::Label::new("N/A"));
        });
        ui.add_enabled(false, egui::Button::new("Show More ▾").frame(false));
    });
}
fn property<'a>(fields: &'a pcb::Fields, key: &str) -> &'a str {
    fields.get(key).map(String::as_str).unwrap_or("")
}
fn source_row(ui: &mut egui::Ui, label: &str, fields: &pcb::Fields, key: &str) {
    read_row(ui, label, property(fields, key));
}
fn visibility(ui: &mut egui::Ui, key: &str, fields: &mut pcb::Fields) -> bool {
    let mut visible = fields
        .get(key)
        .is_none_or(|v| !v.eq_ignore_ascii_case("FALSE"));
    let changed = small_icon(ui, "eye", &mut visible);
    if changed {
        fields.insert(key.into(), if visible { "TRUE" } else { "FALSE" }.into());
    }
    changed
}
fn component(
    ui: &mut egui::Ui,
    c: &mut pcb::Component,
    board: &Board,
    units: Units,
    query: &str,
    action: &mut Option<Action>,
) -> bool {
    let mut done = false;
    section(ui, "Location", query, |ui| {
        done |= location(
            ui,
            &mut c.position,
            Some(&mut c.rotation),
            &mut c.locked,
            origin(board),
            units,
        )
    });
    ui.add_enabled_ui(!c.locked, |ui| {
        section(ui, "Properties", query, |ui| {
            let mut side = c.layer;
            row(ui, "Layer", |ui| {
                if layer_combo(ui, board, &mut side, true, true) {
                    *action = Some(Action::Flip);
                }
            });
            for (label, value, key) in [
                ("Designator", &mut c.designator, "NAMEON"),
                ("Comment", &mut c.comment, "COMMENTON"),
            ] {
                row(ui, label, |ui| {
                    let w = ui.available_width() - 27.0;
                    ui.allocate_ui(egui::vec2(w, 22.0), |ui| done |= text(ui, value));
                    done |= visibility(ui, key, &mut c.properties);
                });
            }
            read_row(ui, "Area", "—");
            ui.separator();
            description(ui, "source", property(&c.properties, "SOURCEDESCRIPTION"));
            let kind = match property(&c.properties, "COMPONENTKINDVERSION2") {
                "5" => "Standard (No BOM)",
                "1" => "Mechanical",
                "2" => "Graphical",
                _ => "Standard",
            };
            read_row(ui, "Type", kind);
            source_row(ui, "Design Item ID", &c.properties, "SOURCELIBREFERENCE");
            source_row(ui, "Source", &c.properties, "SOURCECOMPONENTLIBRARY");
            ui.add_space(12.0);
            let height = board
                .items
                .iter()
                .filter(|i| {
                    i.component == Some(c.index)
                        && i.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
                })
                .filter_map(|i| i.properties.get("OVERALLHEIGHT"))
                .filter_map(|v| eda_io::altium_pcb::dimension(v).ok())
                .max()
                .unwrap_or(0);
            read_row(
                ui,
                "Height",
                &format!("{:.3}{}", units.from_nm(height as f64), units.label()),
            );
            row(ui, "3D Body Opacity", |ui| disabled(ui, "100%"));
            ui.add_space(12.0);
            ui.separator();
            row(ui, "Primitives", |ui| {
                let mut locked =
                    property(&c.properties, "PRIMITIVELOCK").eq_ignore_ascii_case("TRUE");
                ui.add_enabled_ui(false, |ui| {
                    small_icon(ui, "lock", &mut locked);
                });
                ui.label("Strings");
                ui.add_enabled_ui(false, |ui| {
                    small_icon(ui, "lock", &mut locked);
                });
            });
        });
        section(ui, "Footprint", query, |ui| {
            read_row(ui, "Footprint Name", &c.footprint);
            source_row(ui, "Library", &c.properties, "SOURCEFOOTPRINTLIBRARY");
            description(
                ui,
                "footprint",
                property(&c.properties, "FOOTPRINTDESCRIPTION"),
            );
        });
        section(ui, "Swapping Options", query, |ui| {
            row(ui, "", |ui| {
                ui.add_enabled(
                    false,
                    egui::Checkbox::new(&mut false, "Enable Pin Swapping"),
                );
            });
            row(ui, "", |ui| {
                ui.add_enabled(
                    false,
                    egui::Checkbox::new(&mut false, "Enable Part Swapping"),
                );
            });
        });
    });
    done
}
fn shape_combo(ui: &mut egui::Ui, value: &mut PadShape) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt("pad_shape")
        .truncate()
        .width((ui.available_width() - 8.0).max(20.0))
        .selected_text(match value {
            PadShape::Round => "Round",
            PadShape::Rectangle => "Rectangular",
            PadShape::Octagonal => "Octagonal",
            PadShape::RoundedRectangle => "Rounded Rectangle",
        })
        .show_ui(ui, |ui| {
            for (v, label) in [
                (PadShape::Round, "Round"),
                (PadShape::Rectangle, "Rectangular"),
                (PadShape::Octagonal, "Octagonal"),
                (PadShape::RoundedRectangle, "Rounded Rectangle"),
            ] {
                changed |= ui.selectable_value(value, v, label).changed();
            }
        });
    changed
}

fn vertices(ui: &mut egui::Ui, points: &mut [Point], offset: Point, units: Units) -> bool {
    let mut done = false;
    ui.horizontal(|ui| {
        for label in ["Index", "X", "Y"] {
            ui.add_sized(
                [
                    if label == "Index" {
                        37.0
                    } else {
                        (ui.available_width() - 8.0) / 2.0
                    },
                    18.0,
                ],
                egui::Label::new(label),
            );
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("vertices")
        .max_height(140.0)
        .show(ui, |ui| {
            for (n, p) in points.iter_mut().enumerate() {
                ui.push_id(n, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized([37.0, 22.0], egui::Label::new(n.to_string()));
                        let w = (ui.available_width() - 4.0) / 2.0;
                        let mut local = Point {
                            x: p.x - offset.x,
                            y: p.y - offset.y,
                        };
                        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
                            done |= numeric(ui, &mut local.x, units)
                        });
                        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
                            done |= numeric(ui, &mut local.y, units)
                        });
                        *p = Point {
                            x: local.x + offset.x,
                            y: local.y + offset.y,
                        };
                    })
                });
            }
        });
    done
}

fn mask(ui: &mut egui::Ui, title: &str, query: &str) {
    section(ui, title, query, |ui| {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 4.0) / 2.0;
            ui.add_enabled(
                false,
                egui::Button::new("Rule").min_size(egui::vec2(w, 22.0)),
            );
            ui.add_enabled(
                false,
                egui::Button::new("Manual").min_size(egui::vec2(w, 22.0)),
            );
        });
        row(ui, "", |ui| disabled(ui, "—"));
    });
}
fn item(
    ui: &mut egui::Ui,
    item: &mut Item,
    board: &Board,
    units: Units,
    query: &str,
    panel: &mut Panel,
) -> bool {
    let mut done = false;
    if !matches!(
        item.shape,
        Shape::Text { .. } | Shape::Fill { .. } | Shape::Region { .. }
    ) {
        net_information(ui, board, item, units, query);
    }
    let offset = origin(board);
    if !matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
        section(ui, "Location", query, |ui| {
            let mut p = item.anchor();
            let mut rotation = match item.shape {
                Shape::Text { rotation, .. } | Shape::Fill { rotation, .. } => Some(rotation),
                _ => None,
            };
            done |= location(
                ui,
                &mut p,
                rotation.as_mut(),
                &mut item.locked,
                offset,
                units,
            );
            if p != item.anchor() {
                match item.with_anchor(p) {
                    Ok(next) => *item = next,
                    Err(e) => {
                        ui.colored_label(Color32::LIGHT_RED, e);
                    }
                }
            }
            match &mut item.shape {
                Shape::Text { rotation: r, .. } | Shape::Fill { rotation: r, .. } => {
                    *r = rotation.unwrap()
                }
                _ => {}
            }
        });
    }
    let locked = item.locked;
    ui.add_enabled_ui(!locked, |ui| {
        section(
            ui,
            if matches!(item.shape, Shape::Via { .. }) {
                "Definition"
            } else {
                "Properties"
            },
            query,
            |ui| {
                if let Shape::Pad { name, .. } = &mut item.shape {
                    done |= text_row(ui, "Designator", name);
                }
                if matches!(item.shape, Shape::Pad { .. }) {
                    done |= layer(ui, board, &mut item.layer, false);
                    done |= net(ui, board, &mut item.net);
                    row(ui, "Electrical Type", |ui| disabled(ui, "—"));
                    read_row(ui, "Propagation Delay", "—");
                    read_row(ui, "Pin Package Length", "—");
                    row(ui, "Jumper", |ui| disabled(ui, "0"));
                } else {
                    if let Shape::Text { text: value, .. } = &mut item.shape {
                        ui.horizontal(|ui| {
                            let w = (ui.available_width() - 4.0) / 2.0;
                            ui.add_sized([w, 22.0], egui::Button::new("String").selected(true));
                            ui.add_enabled(
                                false,
                                egui::Button::new("Frame").min_size(egui::vec2(w, 22.0)),
                            );
                        });
                        done |= text_row(ui, "Text", value);
                    }
                    if !matches!(item.shape, Shape::Text { .. }) {
                        done |= net(ui, board, &mut item.net);
                    }
                    if !matches!(item.shape, Shape::Via { .. }) {
                        done |= layer(ui, board, &mut item.layer, false);
                    }
                }
                match &mut item.shape {
                    Shape::Track { start, end, width } => {
                        ui.separator();
                        done |= numeric_row(ui, "Width", width, units);
                        done |= coordinates(ui, "Start (X/Y)", start, offset, units);
                        done |= coordinates(ui, "End (X/Y)", end, offset, units);
                        read_row(
                            ui,
                            "Length",
                            &format!(
                                "{:.3}{}",
                                units.from_nm(pcb::distance_sq(*start, *end).sqrt()),
                                units.label()
                            ),
                        );
                    }
                    Shape::Arc {
                        centre,
                        radius,
                        start_angle,
                        end_angle,
                        width,
                    } => {
                        done |= numeric_row(ui, "Width", width, units);
                        done |= coordinates(ui, "Center (X/Y)", centre, offset, units);
                        done |= numeric_row(ui, "Radius", radius, units);
                        done |= angle(ui, "Start Angle", start_angle);
                        done |= angle(ui, "End Angle", end_angle);
                    }
                    Shape::Pad { .. } | Shape::Via { .. } => {
                        if let Shape::Via {
                            from_layer,
                            to_layer,
                            ..
                        } = item.shape
                        {
                            let mut stack: Vec<_> =
                                board.layers.iter().filter(|l| l.copper).collect();
                            stack.sort_by_key(|l| l.order);
                            let first =
                                stack.iter().position(|l| l.id == from_layer).map(|n| n + 1);
                            let last = stack.iter().position(|l| l.id == to_layer).map(|n| n + 1);
                            let name = match (first, last) {
                                (Some(a), Some(b)) => format!(
                                    "{} {}:{}",
                                    if a.min(b) == 1 && a.max(b) == stack.len() {
                                        "Thru"
                                    } else if a.min(b) == 1 || a.max(b) == stack.len() {
                                        "Blind"
                                    } else {
                                        "Buried"
                                    },
                                    a,
                                    b
                                ),
                                _ => "—".into(),
                            };
                            read_row(ui, "Name", &name);
                            read_row(ui, "Propagation Delay", "—");
                        }
                        ui.separator();
                        row(ui, "Template", |ui| {
                            disabled(
                                ui,
                                if property(&item.properties, "TEMPLATENAME").is_empty() {
                                    "<None>"
                                } else {
                                    property(&item.properties, "TEMPLATENAME")
                                },
                            )
                        });
                        read_row(ui, "Library", "<Local>");
                        ui.separator();
                    }
                    Shape::Text {
                        height,
                        stroke,
                        mirrored,
                        font,
                        ..
                    } => {
                        row(ui, "", |ui| {
                            done |= ui.checkbox(mirrored, "Mirror").changed()
                        });
                        done |= numeric_row(ui, "Text Height", height, units);
                        ui.separator();
                        ui.label("Font Type");
                        let ttf = property(&item.properties, "PCBCRAFT_TEXT_TTF") == "1";
                        ui.horizontal(|ui| {
                            let w = (ui.available_width() - 8.0) / 3.0;
                            for (label, selected) in
                                [("TrueType", ttf), ("Stroke", !ttf), ("BarCode", false)]
                            {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(label)
                                        .selected(selected)
                                        .min_size(egui::vec2(w, 22.0)),
                                );
                            }
                        });
                        row(ui, "Justification", |ui| {
                            for label in ["≡", "≡", "≡"] {
                                ui.add_enabled(false, egui::Button::new(label));
                            }
                        });
                        if ttf {
                            done |= text_row(ui, "Font", font);
                        } else {
                            read_row(ui, "Font", "Default");
                        }
                        done |= numeric_row(ui, "Stroke Width", stroke, units);
                        ui.add_space(20.0);
                        ui.label("Border Mode");
                        ui.separator();
                        ui.horizontal(|ui| {
                            let w = (ui.available_width() - 4.0) / 2.0;
                            for label in ["Margin", "Offset"] {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(label).min_size(egui::vec2(w, 22.0)),
                                );
                            }
                        });
                        read_row(ui, "Text Offset", "—");
                        read_row(ui, "Margin Border", "—");
                    }
                    Shape::Fill {
                        start,
                        end,
                        rotation,
                    } => {
                        done |= coordinates(ui, "Corner 1 (X/Y)", start, offset, units);
                        done |= coordinates(ui, "Corner 2 (X/Y)", end, offset, units);
                        done |= angle(ui, "Rotation", rotation);
                    }
                    Shape::Region { cutout, .. } => {
                        row(ui, "Kind", |ui| {
                            egui::ComboBox::from_id_salt("region_kind")
                                .truncate()
                                .width((ui.available_width() - 8.0).max(20.0))
                                .selected_text(if *cutout { "Polygon Cutout" } else { "Copper" })
                                .show_ui(ui, |ui| {
                                    done |= ui.selectable_value(cutout, false, "Copper").changed();
                                    done |= ui
                                        .selectable_value(cutout, true, "Polygon Cutout")
                                        .changed();
                                });
                        });
                    }
                }
            },
        );
    });
    // Keep the lock reachable even when all editable fields are locked.
    match &mut item.shape {
        Shape::Pad {
            position, rotation, ..
        } => {
            done |= location(
                ui,
                position,
                Some(rotation),
                &mut item.locked,
                offset,
                units,
            );
        }
        Shape::Via { position, .. } => {
            done |= location(ui, position, None, &mut item.locked, offset, units);
        }
        _ => {}
    }
    if matches!(item.shape, Shape::Pad { .. } | Shape::Via { .. }) {
        done |= stacks::show(ui, item, board, units, query, panel);
    }
    ui.add_enabled_ui(!item.locked, |ui| match &mut item.shape {
        Shape::Region { contour, holes, .. } => {
            section(ui, "Outline Vertices", query, |ui| {
                done |= vertices(ui, contour, offset, units)
            });
            for (n, hole) in holes.iter_mut().enumerate() {
                ui.push_id(n, |ui| {
                    section(ui, &format!("Cutout {}", n + 1), query, |ui| {
                        done |= vertices(ui, hole, offset, units)
                    })
                });
            }
        }
        Shape::Track { .. } | Shape::Arc { .. } | Shape::Fill { .. } => {
            mask(ui, "Paste Mask Expansion", query);
            mask(ui, "Solder Mask Expansion", query);
        }
        _ => {}
    });
    done
}
fn polygon(
    ui: &mut egui::Ui,
    p: &mut Polygon,
    board: &Board,
    units: Units,
    query: &str,
    action: &mut Option<Action>,
) -> bool {
    let mut done = false;
    section(ui, "Actions", query, |ui| {
        ui.horizontal(|ui| {
            if ui.button("Repour").clicked() {
                *action = Some(Action::Repour);
            }
            if ui
                .button(if p.shelved { "Restore" } else { "Shelve" })
                .clicked()
            {
                p.shelved = !p.shelved;
                done = true;
            }
            ui.add_enabled(false, egui::Button::new("Assign net"));
        });
        ui.horizontal(|ui| {
            for label in ["Bring to front", "Send to back", "Modify"] {
                ui.add_enabled(false, egui::Button::new(label));
            }
        });
    });
    let proxy = Item::new(
        p.layer,
        p.net,
        Shape::Region {
            contour: p.points(),
            holes: p.holes.clone(),
            cutout: false,
        },
    );
    net_information(ui, board, &proxy, units, query);
    section(ui, "Properties", query, |ui| {
        row(ui, "Locked", |ui| {
            done |= small_icon(ui, "lock", &mut p.locked)
        });
        ui.add_enabled_ui(!p.locked, |ui| {
            done |= text_row(ui, "Name", &mut p.name);
            done |= layer(ui, board, &mut p.layer, true);
            done |= net(ui, board, &mut p.net);
            done |= numeric_row(ui, "Clearance", &mut p.clearance, units);
            row(ui, "Connect Style", |ui| {
                egui::ComboBox::from_id_salt("connect")
                    .truncate()
                    .width((ui.available_width() - 8.0).max(20.0))
                    .selected_text(format!("{:?}", p.connection))
                    .show_ui(ui, |ui| {
                        for v in [Connection::Direct, Connection::Thermal, Connection::None] {
                            done |= ui
                                .selectable_value(&mut p.connection, v, format!("{v:?}"))
                                .changed();
                        }
                    });
            });
            if p.connection == Connection::Thermal {
                done |= numeric_row(ui, "Air Gap Width", &mut p.thermal_gap, units);
                done |= numeric_row(ui, "Conductor Width", &mut p.spoke_width, units);
                done |= angle(ui, "Conductors Angle", &mut p.spoke_angle);
            }
            row(ui, "Pour Order", |ui| {
                let r = ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::DragValue::new(&mut p.priority),
                );
                done |= r.lost_focus() || r.drag_stopped();
            });
            done |= ui
                .checkbox(&mut p.remove_islands, "Remove Dead Copper")
                .changed();
            if p.remove_islands {
                row(ui, "Area Less Than", |ui| {
                    let r = ui.add_sized(
                        [ui.available_width(), 22.0],
                        egui::DragValue::new(&mut p.min_island_area_mm2)
                            .range(0.0..=1e6)
                            .suffix(" sq.mm"),
                    );
                    done |= r.lost_focus() || r.drag_stopped();
                });
            }
        });
    });
    section(ui, "Outline Vertices", query, |ui| {
        let mut points: Vec<_> = p.contour.iter().map(|v| v.position).collect();
        ui.add_enabled_ui(
            !p.locked && !p.contour.iter().any(|v| v.arc.is_some()),
            |ui| {
                if vertices(ui, &mut points, origin(board), units) {
                    done = true;
                }
                for (v, point) in p.contour.iter_mut().zip(points) {
                    v.position = point;
                }
            },
        );
        // Editing arc endpoints without their centres would corrupt the outline.
        if p.contour.iter().any(|v| v.arc.is_some()) {
            ui.add_enabled(false,egui::Label::new("Arc vertices")).on_disabled_hover_text("Edit curved outlines on the canvas; Properties arc editing is not supported yet");
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!p.locked, egui::Button::new("Add"))
                .clicked()
                && let Some(last) = p.contour.last().cloned()
            {
                let first = p.contour[0].position;
                p.contour.push(ContourVertex {
                    position: pcb::midpoint(last.position, first),
                    arc: None,
                });
                done = true;
            }
            ui.add_enabled(false, egui::Button::new("Delete"));
        });
    });
    done
}

fn board_panel(
    ui: &mut egui::Ui,
    board: &Board,
    view: &mut eda_render::View,
    units: Units,
    query: &str,
) {
    section(ui, "Selection Filter", query, |ui| {
        egui::ComboBox::from_id_salt("selection_filter")
            .width(73.0)
            .selected_text("Custom")
            .show_ui(ui, |ui| {
                if ui.selectable_label(false, "All Objects").clicked() {
                    view.pcb.filters.fill(true);
                    view.pcb.component_selection = true;
                    view.pcb.select_bodies = true;
                    view.pcb.select_keepouts = true;
                    ui.close();
                }
                if ui.selectable_label(false, "Clear").clicked() {
                    view.pcb.filters.fill(false);
                    view.pcb.component_selection = false;
                    view.pcb.select_bodies = false;
                    view.pcb.select_keepouts = false;
                    ui.close();
                }
            });
        for labels in [
            &["Components", "3D Bodies", "Keepouts"][..],
            &["Tracks", "Arcs", "Pads", "Vias", "Regions"],
            &["Polygons", "Fills", "Texts", "Rooms", "Other"],
        ] {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for label in labels {
                    let value = match *label {
                        "Components" => Some(&mut view.pcb.component_selection),
                        "3D Bodies" => Some(&mut view.pcb.select_bodies),
                        "Keepouts" => Some(&mut view.pcb.select_keepouts),
                        "Tracks" => Some(&mut view.pcb.filters[0]),
                        "Arcs" => Some(&mut view.pcb.filters[1]),
                        "Pads" => Some(&mut view.pcb.filters[2]),
                        "Vias" => Some(&mut view.pcb.filters[3]),
                        "Texts" => Some(&mut view.pcb.filters[4]),
                        "Fills" => Some(&mut view.pcb.filters[5]),
                        "Regions" => Some(&mut view.pcb.filters[6]),
                        "Polygons" => Some(&mut view.pcb.filters[7]),
                        _ => None,
                    };
                    if let Some(v) = value {
                        if ui
                            .add(
                                egui::Button::new(*label)
                                    .selected(*v)
                                    .min_size(egui::vec2(0.0, 24.0)),
                            )
                            .clicked()
                        {
                            *v = !*v;
                        }
                    } else {
                        ui.add_enabled(
                            false,
                            egui::Button::new(*label).min_size(egui::vec2(0.0, 24.0)),
                        );
                    }
                }
            });
        }
        ui.add_space(7.0);
    });
    section(ui, "Snap Options", query, |ui| {
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new("Grids")
                        .selected(view.grid)
                        .min_size(egui::vec2(0.0, 24.0)),
                )
                .clicked()
            {
                view.grid = !view.grid;
            }
            ui.add_enabled(
                false,
                egui::Button::new("Guides").min_size(egui::vec2(0.0, 24.0)),
            );
            ui.add_enabled(
                false,
                egui::Button::new("Axes").min_size(egui::vec2(0.0, 24.0)),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Snapping");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("Shift+E").color(MUTED));
            });
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let w = ui.available_width() / 3.0;
            for (v, label) in [
                (pcb::SnapLayerMode::All, "All Layers"),
                (pcb::SnapLayerMode::Current, "Current Layer"),
                (pcb::SnapLayerMode::Off, "Off"),
            ] {
                if ui
                    .add_sized(
                        [w, 22.0],
                        egui::Button::new(label).selected(view.pcb.snap.mode == v),
                    )
                    .clicked()
                {
                    view.pcb.snap.mode = v;
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
                egui::ScrollArea::vertical()
                    .id_salt("pcb_snap_objects")
                    .max_height(252.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let snap = &mut view.pcb.snap;
                        for (n, label) in [
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
                            "3D Body Snap Points",
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let value = match n {
                                0 => Some(&mut snap.track_vertices),
                                2 => Some(&mut snap.arc_centres),
                                4 => Some(&mut snap.pad_centres),
                                6 => Some(&mut snap.pad_edges),
                                7 => Some(&mut snap.via_centres),
                                8 => Some(&mut snap.region_vertices),
                                9 => Some(&mut snap.board_vertices),
                                10 => Some(&mut snap.component_origins),
                                _ => None,
                            };
                            properties::snap_checkbox(ui, label, value);
                        }
                    });
            });
        ui.add_space(5.0);
        numeric_row(ui, "Snap Distance", &mut view.pcb.snap.distance, units);
        view.pcb.snap.distance = view.pcb.snap.distance.clamp(0, 100_000_000);
        row(ui, "Axis Snap Range", |ui| disabled(ui, "—"));
    });
    section(ui, "Board Information", query, |ui| {
        ui.label(RichText::new("Board Size").family(egui::FontFamily::Name("ui-bold".into())));
        let points = board.outline_points(512);
        let bounds = Bounds::points(&points);
        let area = if points.len() > 2 {
            points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .take(points.len())
                .map(|(a, b)| a.x as f64 * b.y as f64 - b.x as f64 * a.y as f64)
                .sum::<f64>()
                .abs()
                / 2e12
        } else {
            0.0
        };
        for (label, value) in [
            (
                "Horizontal",
                format!(
                    "{:.3}{}",
                    units.from_nm((bounds.max.x - bounds.min.x) as f64),
                    units.label()
                ),
            ),
            (
                "Vertical",
                format!(
                    "{:.3}{}",
                    units.from_nm((bounds.max.y - bounds.min.y) as f64),
                    units.label()
                ),
            ),
            ("Area", format!("{area:.3} sq.mm")),
            ("Components Area", "—".into()),
        ] {
            row(ui, label, |ui| {
                ui.add(egui::Label::new(value).truncate());
            });
        }
        ui.add_space(12.0);
        ui.label(RichText::new("Components").strong());
        row(ui, "Total", |ui| {
            ui.label(board.components.len().to_string());
        });
        row(ui, "Nets", |ui| {
            ui.label(board.nets.len().to_string());
        });
    });
}
fn parameters(
    ui: &mut egui::Ui,
    board: &Board,
    owner: Option<Uuid>,
    group: &mut usize,
    query: &str,
    units: Units,
) {
    section(ui, "Parameters", query, |ui| {
        if owner.is_none() {
            for labels in [
                &[
                    (0, "All"),
                    (1, "System"),
                    (2, "Version Control"),
                    (3, "Board"),
                ][..],
                &[
                    (4, "Components"),
                    (5, "Primitives"),
                    (6, "FAB"),
                    (7, "Project"),
                ],
            ] {
                ui.horizontal_wrapped(|ui| {
                    for (value, label) in labels {
                        if ui
                            .add(
                                egui::Button::new(*label)
                                    .selected(*group == *value)
                                    .min_size(egui::vec2(0.0, 24.0)),
                            )
                            .clicked()
                        {
                            *group = *value;
                        }
                    }
                });
            }
        }
        let mut entries = vec![];
        let mut add = |category: usize, name: &str, value: String| {
            if (owner.is_some() || *group == 0 || *group == category)
                && (query.is_empty() || format!("{name} {value}").to_lowercase().contains(query))
            {
                entries.push((name.to_owned(), value));
            }
        };
        if let Some(c) = owner.and_then(|id| board.components.iter().find(|c| c.id == id)) {
            for (key, value) in &c.properties {
                if key.starts_with("PARAMETER.") {
                    add(4, key.trim_start_matches("PARAMETER."), value.clone());
                }
            }
        } else if owner.is_none() {
            add(
                1,
                "Application Build Number",
                env!("CARGO_PKG_VERSION").into(),
            );
            let bounds = Bounds::points(&board.outline_points(512));
            add(
                3,
                "Board Height",
                format!(
                    "{:.3}{}",
                    units.from_nm((bounds.max.y - bounds.min.y) as f64),
                    units.label()
                ),
            );
            add(
                3,
                "Board Width",
                format!(
                    "{:.3}{}",
                    units.from_nm((bounds.max.x - bounds.min.x) as f64),
                    units.label()
                ),
            );
            add(4, "Component Count", board.components.len().to_string());
            for (label, predicate) in [
                ("Arc Count", 0),
                ("Fill Count", 1),
                ("Hole Count", 2),
                ("Pad Count", 3),
                ("Track Count", 4),
                ("Via Count", 5),
            ] {
                let count = board
                    .items
                    .iter()
                    .filter(|i| match (&i.shape, predicate) {
                        (Shape::Arc { .. }, 0)
                        | (Shape::Fill { .. }, 1)
                        | (Shape::Pad { .. }, 3)
                        | (Shape::Track { .. }, 4)
                        | (Shape::Via { .. }, 5) => true,
                        (Shape::Pad { hole, .. } | Shape::Via { hole, .. }, 2) => *hole > 0,
                        _ => false,
                    })
                    .count();
                add(5, label, count.to_string());
            }
            add(3, "Net Count", board.nets.len().to_string());
            for (key, value) in &board.properties {
                if key.starts_with("PARAMETER.") {
                    add(7, key.trim_start_matches("PARAMETER."), value.clone());
                }
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme::BORDER))
            .inner_margin(4)
            .show(ui, |ui| {
                let w = (ui.available_width() - 6.0) / 2.0;
                ui.horizontal(|ui| {
                    ui.add_sized([w, 20.0], egui::Label::new("Name"));
                    ui.label("Value");
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("parameters")
                    .max_height(400.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (name, value) in entries {
                            ui.horizontal(|ui| {
                                ui.add_sized([w, 20.0], egui::Label::new(&name).truncate())
                                    .on_hover_text(name);
                                ui.add_sized([w, 20.0], egui::Label::new(&value).truncate())
                                    .on_hover_text(value);
                            });
                        }
                    });
                ui.add_enabled(false, egui::Button::new("Place"));
            });
    });
}
fn interactive(
    ui: &mut egui::Ui,
    ed: &mut Editor,
    board: &Board,
    units: Units,
    action: &mut Option<Action>,
) {
    properties::section(ui, "Properties", |ui| {
        if matches!(ed.tool, Tool::Route | Tool::Arc) {
            numeric_row(ui, "Width", &mut ed.width, units);
        }
        if matches!(ed.tool, Tool::Via | Tool::Route) {
            numeric_row(ui, "Diameter", &mut ed.diameter, units);
            numeric_row(ui, "Hole Size", &mut ed.drill, units);
        }
        if ed.tool == Tool::Pad {
            text_row(ui, "Designator", &mut ed.pad_name);
            numeric_row(ui, "X-Size", &mut ed.pad_size.x, units);
            numeric_row(ui, "Y-Size", &mut ed.pad_size.y, units);
            row(ui, "Shape", |ui| {
                shape_combo(ui, &mut ed.pad_shape);
            });
            numeric_row(ui, "Hole Size", &mut ed.pad_hole, units);
            ui.checkbox(&mut ed.pad_plated, "Plated");
            angle(ui, "Rotation", &mut ed.rotation);
        }
        if ed.tool == Tool::Text {
            text_row(ui, "Text", &mut ed.text);
            numeric_row(ui, "Height", &mut ed.text_height, units);
        }
        ui.add_enabled_ui(ed.route_session.is_none(), |ui| {
            net(ui, board, &mut ed.net);
        });
        if ed.route_session.is_some() && ui.button("Finish route").clicked() {
            *action = Some(Action::FinishRoute);
        }
        if let Some(error) = &ed.last_error {
            ui.add(egui::Label::new(RichText::new(error).color(Color32::LIGHT_RED)).wrap());
        }
    });
}
pub(super) fn flush_pending(
    doc: &mut OpenDocument,
    ed: &mut Editor,
    force: bool,
) -> Result<(), String> {
    let selected = &doc.view.pcb.selected;
    let board = doc.model.pcb.as_ref().unwrap();
    let revision = doc.history.revision;
    let next =
        if let Some(d) = ed.draft.as_ref().filter(|d| {
            (force || selected.len() != 1 || selected[0] != d.id) && d.revision == revision
        }) {
            board
                .item(d.id)
                .filter(|i| *i != &d.item)
                .map(|_| board.edited(Edit::Replace(d.item.clone())))
        } else if let Some(d) = ed.component_draft.as_ref().filter(|d| {
            (force || selected.len() != 1 || selected[0] != d.value.id) && d.revision == revision
        }) {
            board
                .components
                .iter()
                .find(|c| c.id == d.value.id)
                .filter(|c| *c != &d.value)
                .map(|_| pcb_tools::edit_component(board, d.value.clone()))
        } else if let Some(d) = ed.polygon_draft.as_ref().filter(|d| {
            (force || selected.len() != 1 || selected[0] != d.value.id) && d.revision == revision
        }) {
            board
                .polygon_pours
                .iter()
                .find(|p| p.id == d.value.id)
                .filter(|p| *p != &d.value)
                .map(|_| pcb_tools::replace_polygon(board, d.value.clone()))
        } else {
            None
        };
    if let Some(result) = next {
        commit(doc, result?)?;
    }
    Ok(())
}
pub(super) fn show(app: &mut PcbCraft, ui: &mut egui::Ui) -> bool {
    let Some(index) = app.active else {
        return false;
    };
    if app.documents[index].model.pcb.is_none() {
        return false;
    }
    ui.spacing_mut().scroll.floating = false;
    ui.spacing_mut().scroll.bar_width = 6.0;
    ui.spacing_mut().scroll.foreground_color = false;
    ui.spacing_mut().scroll.bar_inner_margin = 8.0;
    ui.spacing_mut().scroll.bar_outer_margin = 2.0;
    let units = app.settings.units;
    let doc = &mut app.documents[index];
    let mut ed = std::mem::take(&mut doc.pcb_editor);
    let mut error = flush_pending(doc, &mut ed, false).err();
    let selected = doc.view.pcb.selected.clone();
    let board = doc.model.pcb.as_ref().unwrap();
    let selected_id = (selected.len() == 1).then(|| selected[0]);
    if ed.properties_panel.context != selected_id {
        ed.properties_panel.context = selected_id;
        ed.properties_panel.parameters = false;
    }
    let context = if ed.tool != Tool::Select {
        match ed.tool {
            Tool::Route => "Interactive Routing",
            Tool::Pad => "Pad",
            Tool::Via => "Via",
            Tool::Text => "Text",
            Tool::Fill => "Fill",
            Tool::Region => "Region",
            Tool::Polygon => "Polygon Pour",
            Tool::Arc => "Arc",
            _ => "Board",
        }
    } else if selected.len() > 1 {
        "Multiple Objects"
    } else if let Some(id) = selected_id {
        if board.components.iter().any(|c| c.id == id) {
            "Component"
        } else if board.polygon_pours.iter().any(|p| p.id == id) {
            "Polygon Pour"
        } else {
            board.item(id).map(Item::kind).unwrap_or("Board")
        }
    } else {
        "Board"
    };
    context_row(ui, context);
    ui.add_space(3.0);
    theme::search_field(ui, &mut ed.properties_panel.search, ui.available_width());
    ui.add_space(10.0);
    if matches!(context, "Board" | "Component") && ed.tool == Tool::Select {
        ui.horizontal(|ui| {
            tabs(
                ui,
                &mut ed.properties_panel.parameters,
                "General",
                "Parameters",
            );
            if context == "Board" {
                ui.add_enabled(
                    false,
                    egui::Button::new("Health Check ⚠").min_size(egui::vec2(98.0, 22.0)),
                )
                .on_disabled_hover_text("Engineering health checks are not implemented");
            }
        });
    }
    egui::TopBottomPanel::bottom("pcb_properties_status")
        .exact_height(18.0)
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            ui.label(
                RichText::new(match selected.len() {
                    0 => "Nothing selected".into(),
                    1 => "1 object is selected".into(),
                    n => format!("{n} objects are selected"),
                })
                .size(11.0),
            );
        });
    let query = ed.properties_panel.search.trim().to_lowercase();
    let mut edit = None;
    let mut action = None;
    egui::ScrollArea::vertical()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .id_salt((
            "pcb_properties_v3",
            context,
            selected_id,
            ed.properties_panel.parameters,
        ))
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            if ed.tool != Tool::Select {
                interactive(ui, &mut ed, board, units, &mut action);
                return;
            }
            if ed.properties_panel.parameters && matches!(context, "Board" | "Component") {
                parameters(
                    ui,
                    board,
                    selected_id,
                    &mut ed.properties_panel.parameter_group,
                    &query,
                    units,
                );
                return;
            }
            ui.add_enabled_ui(!ed.has_pending(), |ui| {
                if let Some(id) = selected_id {
                    if let Some(original) = board.item(id) {
                        ed.component_draft = None;
                        ed.polygon_draft = None;
                        if ed
                            .draft
                            .as_ref()
                            .is_none_or(|d| d.id != id || d.revision != doc.history.revision)
                        {
                            ed.draft = Some(Draft {
                                id,
                                revision: doc.history.revision,
                                item: original.clone(),
                            });
                        }
                        let draft = &mut ed.draft.as_mut().unwrap().item;
                        if item(ui, draft, board, units, &query, &mut ed.properties_panel)
                            && draft != original
                        {
                            edit = Some(board.edited(Edit::Replace(draft.clone())));
                        }
                    } else if let Some(original) = board.components.iter().find(|c| c.id == id) {
                        ed.draft = None;
                        ed.polygon_draft = None;
                        if ed
                            .component_draft
                            .as_ref()
                            .is_none_or(|d| d.value.id != id || d.revision != doc.history.revision)
                        {
                            ed.component_draft = Some(ComponentDraft {
                                revision: doc.history.revision,
                                value: original.clone(),
                            });
                        }
                        let draft = &mut ed.component_draft.as_mut().unwrap().value;
                        if component(ui, draft, board, units, &query, &mut action)
                            && draft != original
                        {
                            edit = Some(pcb_tools::edit_component(board, draft.clone()));
                        }
                    } else if let Some(original) = board.polygon_pours.iter().find(|p| p.id == id) {
                        ed.draft = None;
                        ed.component_draft = None;
                        if ed
                            .polygon_draft
                            .as_ref()
                            .is_none_or(|d| d.value.id != id || d.revision != doc.history.revision)
                        {
                            ed.polygon_draft = Some(PolygonDraft {
                                revision: doc.history.revision,
                                value: original.clone(),
                            });
                        }
                        let draft = &mut ed.polygon_draft.as_mut().unwrap().value;
                        if polygon(ui, draft, board, units, &query, &mut action)
                            && draft != original
                        {
                            edit = Some(pcb_tools::replace_polygon(board, draft.clone()));
                        }
                    }
                } else if selected.is_empty() {
                    ed.draft = None;
                    ed.component_draft = None;
                    ed.polygon_draft = None;
                    board_panel(ui, board, &mut doc.view, units, &query);
                } else {
                    section(ui, "Selection", &query, |ui| {
                        for kind in [
                            "Component",
                            "Track",
                            "Arc",
                            "Pad",
                            "Via",
                            "Text",
                            "Fill",
                            "Region",
                            "Polygon Pour",
                        ] {
                            let n = selected
                                .iter()
                                .filter(|id| {
                                    board.item(**id).is_some_and(|i| i.kind() == kind)
                                        || (kind == "Component"
                                            && board.components.iter().any(|c| c.id == **id))
                                        || (kind == "Polygon Pour"
                                            && board.polygon_pours.iter().any(|p| p.id == **id))
                                })
                                .count();
                            if n > 0 {
                                read_row(ui, kind, &n.to_string());
                            }
                        }
                    });
                    section(ui, "Properties", &query, |ui| {
                        disabled(ui, "Mixed selection editing")
                    });
                }
            });
        });
    if let Some(result) = stacks::rule_dialog(ui.ctx(), board, units, &mut ed.properties_panel) {
        edit = Some(result);
    }
    if let Some(result) = edit
        && let Err(e) = result.and_then(|next| commit(doc, next).map(|_| ()))
    {
        error = Some(e);
        ed.draft = None;
        ed.component_draft = None;
        ed.polygon_draft = None;
    }
    if error.is_some() {
        ed.draft = None;
        ed.component_draft = None;
        ed.polygon_draft = None;
    }
    doc.pcb_editor = ed;
    if let Some(e) = error {
        app.message(true, e);
    }
    if let Some(action) = action {
        app.command = Some(Command::Pcb(action));
    }
    true
}

#[cfg(test)]
#[path = "pcb_properties_tests.rs"]
mod tests;
