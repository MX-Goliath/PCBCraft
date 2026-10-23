//! Layout measured in AD 24.4.1 on the ReferenceA board (1920x1030, Windows).

use super::*;

use egui::{Align2, FontId, Rect, Sense, Stroke, pos2, vec2};

pub(super) fn color(kind: LayerKind) -> Color32 {
    match kind {
        LayerKind::Signal | LayerKind::Plane => Color32::from_rgb(195, 126, 75),

        LayerKind::SolderMask | LayerKind::Overlay => Color32::from_rgb(39, 99, 56),

        _ => Color32::from_rgb(133, 129, 108),
    }
}

pub(super) fn toolbar(ui: &mut egui::Ui, m: &mut Manager, apply: &mut bool, close: &mut bool) {
    ui.horizontal(|ui| {
        ui.set_height(22.0);

        ui.menu_button("+ Add", |ui| {
            if m.tab == 0 {
                for (label, kind) in [("Signal", LayerKind::Signal), ("Plane", LayerKind::Plane)] {
                    if ui.button(label).clicked() {
                        match m.stack.insert_copper(m.selected, kind) {
                            Ok(i) => m.selected = i,

                            Err(e) => m.error = e,
                        }

                        ui.close();
                    }
                }

                if ui.button("Dielectric").clicked() {
                    match m.stack.insert_dielectric(m.selected) {
                        Ok(i) => m.selected = i,

                        Err(e) => m.error = e,
                    }

                    ui.close();
                }
            } else if m.tab == 1 {
                if ui.button("Via Type").clicked() {
                    let mut n = 1;

                    while m.stack.vias.iter().any(|v| v.name == format!("Via {n}")) {
                        n += 1;
                    }

                    m.stack.vias.push(ViaType {
                        id: Uuid::new_v4(),

                        name: format!("Via {n}"),

                        kind: ViaKind::Through,

                        start: TOP,

                        end: BOTTOM,

                        diameter: 600_000,

                        hole: 300_000,
                    });

                    m.selected = m.stack.vias.len() - 1;

                    ui.close();
                }
            } else if ui.button("Impedance Profile").clicked() {
                let mut n = 1;

                while m.stack.impedances.iter().any(|p| p.name == format!("S{n}")) {
                    n += 1;
                }

                m.stack.impedances.push(Impedance {
                    id: Uuid::new_v4(),

                    name: format!("S{n}"),

                    differential: false,

                    target: 50.0,

                    tolerance: 10.0,

                    signal: TOP,

                    reference: m
                        .stack
                        .copper()
                        .get(1)
                        .and_then(|l| l.copper_id)
                        .unwrap_or(BOTTOM),

                    reference2: None,

                    width: 200_000,

                    gap: 200_000,
                });

                m.selected = m.stack.impedances.len() - 1;

                ui.close();
            }
        });

        if m.tab == 0 && ui.button("Modify").clicked() {
            ui.ctx().memory_mut(|mem| {
                mem.request_focus(egui::Id::new(("stack_layer_name", m.session, m.selected)))
            });
        }

        let can_delete = match m.tab {
            0 => m
                .stack
                .layers
                .get(m.selected)
                .is_some_and(|l| !matches!(l.copper_id, Some(TOP | BOTTOM))),

            1 => m.selected < m.stack.vias.len(),

            _ => m.selected < m.stack.impedances.len(),
        };

        if ui
            .add_enabled(can_delete, egui::Button::new("Delete"))
            .clicked()
        {
            match m.tab {
                0 => {
                    if let Err(e) = m.stack.remove_layer(m.selected) {
                        m.error = e;
                    }
                }

                1 => {
                    m.stack.vias.remove(m.selected);
                }

                _ => {
                    m.stack.impedances.remove(m.selected);
                }
            }

            m.selected = m.selected.saturating_sub(1);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("≡", |ui| {
                if ui.button("Apply    Ctrl+S").clicked() {
                    *apply = true;

                    ui.close();
                }

                if ui.button("Close    Ctrl+W").clicked() {
                    if m.stack == m.original {
                        *close = true;
                    } else {
                        m.confirm_close = true;
                    }

                    ui.close();
                }

                ui.separator();

                if ui
                    .add_enabled(!m.undo.is_empty(), egui::Button::new("Undo    Ctrl+Z"))
                    .clicked()
                {
                    m.undo();

                    ui.close();
                }

                if ui
                    .add_enabled(!m.redo.is_empty(), egui::Button::new("Redo    Ctrl+Y"))
                    .clicked()
                {
                    m.redo();

                    ui.close();
                }

                ui.checkbox(&mut m.mils, "Display in mils");

                if m.tab == 0 {
                    for (label, up) in [("Move layer up", true), ("Move layer down", false)] {
                        if ui.button(label).clicked() {
                            match m.stack.move_layer(m.selected, up) {
                                Ok(i) => m.selected = i,

                                Err(e) => m.error = e,
                            }

                            ui.close();
                        }
                    }
                }
            });
        });
    });
}

fn kind_name(kind: LayerKind) -> &'static str {
    match kind {
        LayerKind::Signal => "Signal",

        LayerKind::Plane => "Plane",

        LayerKind::Core => "Core",

        LayerKind::Prepreg => "Prepreg",

        LayerKind::SolderMask => "Solder Mask",

        LayerKind::Overlay => "Overlay",
    }
}

pub(super) fn via_table(ui: &mut egui::Ui, m: &mut Manager) {
    let width = ui.available_width().max(700.0);

    let split = width * 0.56;

    let height = 20.0 + m.stack.layers.len() as f32 * 21.0;

    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());

    let p = ui.painter();

    p.rect_filled(
        Rect::from_min_size(rect.min, vec2(width, 20.0)),
        0.0,
        Color32::from_gray(72),
    );

    for (i, layer) in m.stack.layers.iter().enumerate() {
        let y = rect.top() + 20.0 + i as f32 * 21.0;

        let row = Rect::from_min_size(pos2(rect.left(), y), vec2(width, 21.0));

        p.rect_filled(row, 0.0, color(layer.kind));

        p.line_segment(
            [row.left_bottom(), row.right_bottom()],
            Stroke::new(1.0, Color32::from_gray(65)),
        );

        p.text(
            pos2(row.left() + 5.0, y + 10.5),
            Align2::LEFT_CENTER,
            &layer.name,
            FontId::proportional(11.0),
            Color32::LIGHT_GRAY,
        );
    }

    p.line_segment(
        [
            pos2(rect.left() + split, rect.top()),
            pos2(rect.left() + split, rect.bottom()),
        ],
        Stroke::new(4.0, Color32::from_gray(50)),
    );

    let fallback = ViaType {
        id: Uuid::nil(),

        name: format!("Thru 1:{}", m.stack.copper().len()),

        kind: ViaKind::Through,

        start: TOP,

        end: BOTTOM,

        diameter: 600_000,

        hole: 300_000,
    };

    let vias: Vec<_> = if m.stack.vias.is_empty() {
        vec![&fallback]
    } else {
        m.stack.vias.iter().collect()
    };

    for (i, via) in vias.iter().enumerate() {
        let x = rect.left() + split + 20.0 + i as f32 * 125.0;

        p.text(
            pos2(x + 50.0, rect.top() + 10.0),
            Align2::CENTER_CENTER,
            &via.name,
            FontId::proportional(11.0),
            Color32::LIGHT_GRAY,
        );

        let a = m
            .stack
            .layers
            .iter()
            .position(|l| l.copper_id == Some(via.start));

        let b = m
            .stack
            .layers
            .iter()
            .position(|l| l.copper_id == Some(via.end));

        if let (Some(a), Some(b)) = (a, b) {
            let span = Rect::from_min_max(
                pos2(x + 20.0, rect.top() + 20.0 + a as f32 * 21.0),
                pos2(x + 80.0, rect.top() + 20.0 + (b + 1) as f32 * 21.0),
            );

            p.rect_filled(span, 0.0, Color32::from_rgb(195, 143, 86));

            p.rect_stroke(
                span,
                0.0,
                Stroke::new(1.0, Color32::from_gray(40)),
                egui::StrokeKind::Inside,
            );

            p.rect_filled(
                Rect::from_min_max(pos2(x + 38.0, span.top()), pos2(x + 62.0, span.bottom())),
                0.0,
                Color32::from_rgb(112, 88, 59),
            );
        }

        if !m.stack.vias.is_empty()
            && ui
                .interact(
                    Rect::from_min_size(pos2(x, rect.top()), vec2(120.0, height)),
                    ui.id().with(("via_column", i)),
                    Sense::click(),
                )
                .clicked()
        {
            m.selected = i;
        }
    }
}

pub(super) fn table(ui: &mut egui::Ui, m: &mut Manager) {
    let widths = [25.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0];

    let width = ui.available_width().max(widths.iter().sum());

    let (header, _) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());

    ui.painter()
        .rect_filled(header, 0.0, Color32::from_gray(72));

    let mut x = header.left();

    for (label, w) in [
        "#",
        "Name",
        "Material",
        "Type",
        "Weight",
        "Thickness",
        "Dk",
        "Df",
    ]
    .into_iter()
    .zip(widths)
    {
        ui.painter().text(
            pos2(x + 4.0, header.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(11.0),
            Color32::LIGHT_GRAY,
        );

        x += w;
    }

    let mut number = 0;

    for (index, layer) in m.stack.layers.iter_mut().enumerate() {
        if layer.kind.copper() {
            number += 1;
        }

        let (rect, response) = ui.allocate_exact_size(vec2(width, 21.0), Sense::click());

        ui.painter().rect_filled(rect, 0.0, color(layer.kind));

        if response.clicked() {
            m.selected = index;
        }

        let dielectric = matches!(
            layer.kind,
            LayerKind::Core | LayerKind::Prepreg | LayerKind::SolderMask
        );

        let values = [
            if layer.kind.copper() {
                number.to_string()
            } else {
                String::new()
            },
            layer.name.clone(),
            if layer.kind.copper() {
                String::new()
            } else {
                layer.material.clone()
            },
            kind_name(layer.kind).into(),
            if layer.kind.copper() {
                format!("{:.3}oz", layer.thickness as f64 / 35_560.0).replace(".000", "")
            } else {
                String::new()
            },
            if layer.kind == LayerKind::Overlay {
                String::new()
            } else {
                dimension(layer.thickness, m.mils)
            },
            if dielectric {
                format!("{}", layer.er)
            } else {
                String::new()
            },
            if dielectric {
                format!("{}", layer.loss_tangent)
            } else {
                String::new()
            },
        ];

        let mut x = rect.left();

        for (value, w) in values.iter().zip(widths) {
            let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(w, rect.height()));

            ui.painter().with_clip_rect(cell.shrink(2.0)).text(
                pos2(x + 4.0, rect.center().y),
                Align2::LEFT_CENTER,
                value,
                FontId::proportional(11.0),
                Color32::from_gray(235),
            );

            ui.painter().line_segment(
                [cell.right_top(), cell.right_bottom()],
                Stroke::new(1.0, Color32::from_gray(65)),
            );

            x += w;
        }

        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            Stroke::new(1.0, Color32::from_gray(65)),
        );

        if m.selected == index {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                0.0,
                Stroke::new(1.0, Color32::from_rgb(125, 172, 206)),
                egui::StrokeKind::Inside,
            );
        }

        response.context_menu(|ui| {
            let kinds: &[LayerKind] = if layer.kind.copper() {
                &[LayerKind::Signal, LayerKind::Plane]
            } else if matches!(layer.kind, LayerKind::Core | LayerKind::Prepreg) {
                &[LayerKind::Core, LayerKind::Prepreg]
            } else {
                &[]
            };
            for kind in kinds {
                if ui
                    .selectable_value(&mut layer.kind, *kind, kind_name(*kind))
                    .clicked()
                {
                    ui.close();
                }
            }
        });
        response
            .on_hover_text("Select a layer to edit its properties; right-click to change its type");
    }
}

pub(super) fn search(ui: &mut egui::Ui) {
    let id = egui::Id::new("stack_property_search");
    let mut query = ui
        .ctx()
        .data_mut(|d| d.get_temp::<String>(id).unwrap_or_default());
    if ui
        .add_sized(
            [ui.available_width(), 20.0],
            egui::TextEdit::singleline(&mut query).hint_text("Search"),
        )
        .changed()
    {
        ui.ctx().data_mut(|d| d.insert_temp(id, query));
    }
}
fn row(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    let query = ui.ctx().data_mut(|d| {
        d.get_temp::<String>(egui::Id::new("stack_property_search"))
            .unwrap_or_default()
    });
    if !query.is_empty() && !label.to_lowercase().contains(&query.to_lowercase()) {
        return;
    }

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            vec2(160.0, 22.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(label);
            },
        );

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), 22.0),
            egui::Layout::left_to_right(egui::Align::Center),
            body,
        );
    });
}

fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) {
    row(ui, label, |ui| {
        ui.add_sized(
            [ui.available_width(), 20.0],
            egui::TextEdit::singleline(value),
        );
    });
}
fn metadata(ui: &mut egui::Ui, layer: &mut eda_core::layer_stack::Layer, key: &str) {
    let mut value = layer.properties.get(key).cloned().unwrap_or_default();
    let before = value.clone();
    text_row(ui, key, &mut value);
    if before != value {
        if value.is_empty() {
            layer.properties.remove(key);
        } else {
            layer.properties.insert(key.into(), value);
        }
    }
}
fn section(ui: &mut egui::Ui, name: &str, body: impl FnOnce(&mut egui::Ui)) {
    egui::CollapsingHeader::new(name)
        .default_open(true)
        .show(ui, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_gray(70))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    body(ui);
                });
        });
}
pub(super) fn layer_properties(ui: &mut egui::Ui, m: &mut Manager) {
    let Some(layer) = m.stack.layers.get_mut(m.selected) else {
        return;
    };
    section(ui, "Layer", |ui| {
        row(ui, "Name", |ui| {
            ui.add_sized(
                [ui.available_width(), 20.0],
                egui::TextEdit::singleline(&mut layer.name).id(egui::Id::new((
                    "stack_layer_name",
                    m.session,
                    m.selected,
                ))),
            );
        });
        if layer.kind != LayerKind::Overlay {
            metadata(ui, layer, "Manufacturer");
            text_row(ui, "Material", &mut layer.material);
            if layer.kind.copper() {
                metadata(ui, layer, "Process");
                row(ui, "Weight", |ui| {
                    let mut oz = layer.thickness as f64 / 35_560.0;
                    if ui
                        .add(
                            egui::DragValue::new(&mut oz)
                                .range(0.001..=100.0)
                                .speed(0.01)
                                .suffix("oz"),
                        )
                        .changed()
                    {
                        layer.thickness = (oz * 35_560.0).round() as i64;
                    }
                });
            }
            row(ui, "Thickness", |ui| {
                length(ui, &mut layer.thickness, m.mils)
            });
            if layer.kind.copper() {
                let bottom = layer.copper_id == Some(BOTTOM);
                for (label, value) in [
                    (
                        "Orientation",
                        if bottom {
                            "Bottom"
                        } else if layer.copper_id == Some(TOP) {
                            "Top"
                        } else {
                            "Internal"
                        },
                    ),
                    ("Copper Orientation", if bottom { "Below" } else { "Above" }),
                ] {
                    row(ui, label, |ui| {
                        ui.add_enabled(false, egui::Button::new(value));
                    });
                }
            } else {
                row(ui, "Dk", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut layer.er)
                            .speed(0.01)
                            .range(1.0..=100.0),
                    );
                });
                row(ui, "Df", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut layer.loss_tangent)
                            .speed(0.001)
                            .range(0.0..=1.0),
                    );
                });
            }
            metadata(ui, layer, "Description");
            if !layer.kind.copper() {
                if layer.kind == LayerKind::SolderMask {
                    row(ui, "Color", |ui| {
                        ui.add_enabled(false, egui::Button::new("Default"))
                            .on_hover_text("Fabrication color rendering is not implemented");
                    });
                    metadata(ui, layer, "Solid");
                } else {
                    metadata(ui, layer, "Constructions");
                    metadata(ui, layer, "Resin");
                }
                metadata(ui, layer, "Frequency");
                metadata(ui, layer, "GlassTransTemp");
            }
            metadata(ui, layer, "Note");
        }
        metadata(ui, layer, "Comment");
    });
}
fn copper_row(ui: &mut egui::Ui, label: &str, value: &mut u16, names: &[(u16, String)]) {
    row(ui, label, |ui| {
        egui::ComboBox::from_id_salt(label)
            .width(ui.available_width())
            .selected_text(layer_name(*value, names))
            .show_ui(ui, |ui| {
                for (id, name) in names {
                    ui.selectable_value(value, *id, name);
                }
            });
    });
}
pub(super) fn via_properties(
    ui: &mut egui::Ui,
    m: &mut Manager,
    apply: &mut bool,
    use_via: &mut Option<ViaType>,
) {
    let names = copper_names(&m.stack);
    section(ui, "Via Type", |ui| {
        if let Some(v) = m.stack.vias.get_mut(m.selected) {
            text_row(ui, "Name", &mut v.name);
            // Only offer ordered spans; microvias connect adjacent copper layers.
            let first: Vec<_> = names
                .iter()
                .take(names.len().saturating_sub(1))
                .cloned()
                .collect();
            ui.add_enabled_ui(v.kind != ViaKind::Through, |ui| {
                copper_row(ui, "First layer", &mut v.start, &first);
                let start = names.iter().position(|(id, _)| *id == v.start).unwrap_or(0);
                let last: Vec<_> = names
                    .iter()
                    .skip(start + 1)
                    .take(if v.kind == ViaKind::Micro {
                        1
                    } else {
                        names.len()
                    })
                    .cloned()
                    .collect();
                if !last.iter().any(|(id, _)| *id == v.end)
                    && let Some((id, _)) = last.first()
                {
                    v.end = *id;
                }
                copper_row(ui, "Last layer", &mut v.end, &last);
            });
            row(ui, "Type", |ui| {
                egui::ComboBox::from_id_salt("via_kind")
                    .selected_text(format!("{:?}", v.kind))
                    .show_ui(ui, |ui| {
                        for k in [ViaKind::Through, ViaKind::BlindBuried, ViaKind::Micro] {
                            if ui
                                .selectable_value(&mut v.kind, k, format!("{k:?}"))
                                .changed()
                            {
                                v.start = names.first().map_or(TOP, |n| n.0);
                                v.end = if k == ViaKind::Through {
                                    names.last().map_or(BOTTOM, |n| n.0)
                                } else {
                                    names.get(1).map_or(BOTTOM, |n| n.0)
                                };
                            }
                        }
                    });
            });
            row(ui, "Mirror", |ui| {
                ui.add_enabled(false, egui::Checkbox::new(&mut false, ""))
                    .on_hover_text("Mirrored via spans are not implemented");
            });
        } else {
            row(ui, "Name", |ui| {
                ui.label(format!("Thru 1:{}", names.len()));
            });
            ui.add_enabled_ui(false, |ui| {
                let mut top = TOP;
                let mut bottom = BOTTOM;
                copper_row(ui, "First layer", &mut top, &names);
                copper_row(ui, "Last layer", &mut bottom, &names);
                row(ui, "µVia", |ui| {
                    ui.checkbox(&mut false, "");
                });
                row(ui, "Mirror", |ui| {
                    ui.checkbox(&mut false, "");
                });
            });
        }
    });
    if let Some(v) = m.stack.vias.get_mut(m.selected) {
        section(ui, "Placement preset", |ui| {
            row(ui, "Diameter", |ui| length(ui, &mut v.diameter, m.mils));
            row(ui, "Hole size", |ui| length(ui, &mut v.hole, m.mils));
            if ui.button("Apply and use for placement").clicked() {
                *apply = true;
                *use_via = Some(v.clone());
            }
        });
    }
}
/// Maintain valid reference topology when a signal layer or geometry changes.
fn normalize_references(p: &mut Impedance, names: &[(u16, String)]) {
    let Some(i) = names.iter().position(|(id, _)| *id == p.signal) else {
        return;
    };
    if p.reference2.is_some() && i > 0 && i + 1 < names.len() {
        if !names[..i].iter().any(|(id, _)| *id == p.reference) {
            p.reference = names[i - 1].0;
        }
        if !names[i + 1..]
            .iter()
            .any(|(id, _)| Some(*id) == p.reference2)
        {
            p.reference2 = Some(names[i + 1].0);
        }
    } else {
        p.reference2 = None;
        if p.reference == p.signal || !names.iter().any(|(id, _)| *id == p.reference) {
            p.reference = names[if i == 0 {
                1.min(names.len() - 1)
            } else {
                i - 1
            }]
            .0;
        }
    }
}

pub(super) fn impedance_properties(
    ui: &mut egui::Ui,
    m: &mut Manager,
    apply: &mut bool,
    use_width: &mut Option<(u16, i64)>,
) {
    let names = copper_names(&m.stack);
    section(ui, "Impedance Profile", |ui| {
        let Some(p) = m.stack.impedances.get_mut(m.selected) else {
            ui.label("No item selected");
            return;
        };
        text_row(ui, "Name", &mut p.name);
        row(ui, "Type", |ui| {
            egui::ComboBox::from_id_salt("profile_type")
                .selected_text(if p.differential {
                    "Differential"
                } else {
                    "Single"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut p.differential, false, "Single");
                    ui.selectable_value(&mut p.differential, true, "Differential");
                });
        });
        row(ui, "Target Impedance", |ui| {
            ui.add(
                egui::DragValue::new(&mut p.target)
                    .range(1.0..=1000.0)
                    .suffix(" Ω"),
            );
        });
        row(ui, "Tolerance", |ui| {
            ui.add(
                egui::DragValue::new(&mut p.tolerance)
                    .range(0.0..=50.0)
                    .suffix(" %"),
            );
        });
        let old_signal = p.signal;
        copper_row(ui, "Signal layer", &mut p.signal, &names);
        if old_signal != p.signal {
            normalize_references(p, &names);
        }
        let i = names
            .iter()
            .position(|(id, _)| *id == p.signal)
            .unwrap_or(0);
        row(ui, "Stripline", |ui| {
            let mut two = p.reference2.is_some();
            if ui
                .add_enabled(
                    i > 0 && i + 1 < names.len(),
                    egui::Checkbox::new(&mut two, ""),
                )
                .changed()
            {
                p.reference2 = two.then_some(BOTTOM);
                normalize_references(p, &names);
            }
        });
        let refs: Vec<_> = names
            .iter()
            .enumerate()
            .filter(|(j, _)| {
                if p.reference2.is_some() {
                    *j < i
                } else {
                    *j != i
                }
            })
            .map(|(_, n)| n.clone())
            .collect();
        copper_row(ui, "Reference 1", &mut p.reference, &refs);
        if let Some(r) = &mut p.reference2 {
            copper_row(ui, "Reference 2", r, &names[i + 1..]);
        }
        row(ui, "Width", |ui| length(ui, &mut p.width, m.mils));
        if p.differential {
            row(ui, "Gap", |ui| length(ui, &mut p.gap, m.mils));
        }
    });
    if let Some(p) = m.stack.impedances.get(m.selected).cloned() {
        section(ui, "Calculation", |ui| {
            ui.label("Analytical estimate (not a field solver)");
            match m.stack.estimate(&p) {
                Ok(z) => {
                    ui.label(format!("Estimated impedance: {z:.2} Ω"));
                }
                Err(e) => {
                    ui.label(e);
                }
            }
            if ui.button("Calculate width for target").clicked() {
                match m.stack.solve_width(&p) {
                    Ok(w) => m.stack.impedances[m.selected].width = w,
                    Err(e) => m.error = e,
                }
            }
            if ui
                .add_enabled(
                    !p.differential,
                    egui::Button::new("Apply and use routing width"),
                )
                .clicked()
            {
                *apply = true;
                *use_width = Some((p.signal, p.width));
            }
        });
    }
}
pub(super) fn other_properties(ui: &mut egui::Ui) {
    ui.add_space(16.0);
    section(ui, "Other", |ui| {
        ui.label("Roughness");
        ui.add_enabled_ui(false, |ui| {
            row(ui, "Model Type", |ui| {
                let _ = ui.button("Flat Conductors");
            });
            row(ui, "Surface Roughness (SR)", |ui| {
                ui.label("0um");
            });
            row(ui, "Roughness Factor (RF)", |ui| {
                ui.label("1");
            });
        })
        .response
        .on_hover_text("Roughness models are not implemented by the analytical calculator");
    });
}
pub(super) fn board_properties(ui: &mut egui::Ui, m: &mut Manager) {
    ui.add_space(22.0);

    section(ui, "Board", |ui| {
        row(ui, "Stack Symmetry", |ui| {
            let mut symmetric = m.stack.symmetric;

            if ui.checkbox(&mut symmetric, "").changed() {
                if !symmetric || m.stack.is_symmetric() {
                    m.stack.symmetric = symmetric;
                } else {
                    m.symmetry_dialog = true;
                }
            }
        });

        row(ui, "Library Compliance", |ui| {
            ui.add_enabled(false, egui::Checkbox::new(&mut false, ""));
        });

        ui.separator();

        let copper: i64 = m
            .stack
            .layers
            .iter()
            .filter(|l| l.kind.copper())
            .map(|l| l.thickness)
            .sum();

        let dielectric: i64 = m
            .stack
            .layers
            .iter()
            .filter(|l| matches!(l.kind, LayerKind::Core | LayerKind::Prepreg))
            .map(|l| l.thickness)
            .sum();

        let total: i64 = m.stack.layers.iter().map(|l| l.thickness).sum();

        for (label, value) in [
            ("Layers", m.stack.copper().len().to_string()),
            (
                "Dielectrics",
                m.stack
                    .layers
                    .iter()
                    .filter(|l| matches!(l.kind, LayerKind::Core | LayerKind::Prepreg))
                    .count()
                    .to_string(),
            ),
            ("Conductive Thickness", dimension(copper, m.mils)),
            ("Dielectric Thickness", dimension(dielectric, m.mils)),
            ("Total Thickness", dimension(total, m.mils)),
        ] {
            row(ui, label, |ui| {
                ui.label(value);
            });
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changing_signal_keeps_references_on_valid_sides() {
        let names = vec![
            (TOP, "Top".into()),
            (2, "Inner".into()),
            (BOTTOM, "Bottom".into()),
        ];
        let mut p = Impedance {
            id: Uuid::new_v4(),
            name: "S1".into(),
            differential: false,
            target: 50.0,
            tolerance: 10.0,
            signal: 2,
            reference: 2,
            reference2: Some(TOP),
            width: 200_000,
            gap: 200_000,
        };
        normalize_references(&mut p, &names);
        assert_eq!((p.reference, p.reference2), (TOP, Some(BOTTOM)));
        p.signal = TOP;
        normalize_references(&mut p, &names);
        assert_eq!((p.reference, p.reference2), (2, None));
        p.signal = BOTTOM;
        p.reference = BOTTOM;
        normalize_references(&mut p, &names);
        assert_eq!((p.reference, p.reference2), (2, None));
    }
}
