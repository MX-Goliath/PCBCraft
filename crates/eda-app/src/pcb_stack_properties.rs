//! AD 24.4.1 unified Copper / Hole / Paste / Solder stack table.
use super::*;
use pcb::{BOTTOM, DrillShape, MaskKind, MaskMode, MaskSettings, PadGeometry, StackMode, TOP};
pub(super) struct RuleEditor {
    index: usize,
    original: pcb::Fields,
    value: pcb::Fields,
}
fn cell<R>(ui: &mut egui::Ui, width: f32, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // Pixel rounding must not let five fractional columns widen the dock each frame.
    let width = width.floor().max(10.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::hover());
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(ui.clip_rect().intersect(rect));
    body(&mut child)
}
fn label(ui: &mut egui::Ui, width: f32, text: &str) {
    cell(ui, width, |ui| {
        ui.add(egui::Label::new(text).truncate())
            .on_hover_text(text);
    });
}
fn heading(ui: &mut egui::Ui, labels: [&str; 5]) {
    let w = ui.available_width();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, label) in labels.into_iter().enumerate() {
            cell(ui, w * [0.22, 0.2, 0.2, 0.25, 0.13][i], |ui| {
                ui.add(egui::Label::new(label).truncate());
            });
        }
    });
}
fn expanded(ui: &mut egui::Ui, id: &str, name: &str, color: Color32) -> (bool, bool) {
    let id = ui.make_persistent_id(id);
    let mut open = ui.data_mut(|d| d.get_persisted::<bool>(id).unwrap_or(false));
    let r = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::Button::new(format!("{} {name}", if open { "▾" } else { "▸" }))
            .frame(false)
            .wrap_mode(egui::TextWrapMode::Truncate),
    );
    ui.painter().rect_filled(
        egui::Rect::from_min_size(r.rect.min, egui::vec2(3.0, r.rect.height())),
        0.0,
        color,
    );
    if r.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(id, open));
    }
    (open, r.clicked())
}
fn combo_shape(ui: &mut egui::Ui, g: &mut PadGeometry) -> bool {
    let mut done = false;
    egui::ComboBox::from_id_salt("shape")
        .truncate()
        .width((ui.available_width() - 8.0).max(20.0))
        .selected_text(match g.shape {
            PadShape::Round => "● Round",
            PadShape::Rectangle => "■ Rectangular",
            PadShape::Octagonal => "◆ Octagonal",
            PadShape::RoundedRectangle => "▣ Rounded Rectangle",
        })
        .show_ui(ui, |ui| {
            for (v, s) in [
                (PadShape::Round, "● Round"),
                (PadShape::Rectangle, "■ Rectangular"),
                (PadShape::Octagonal, "◆ Octagonal"),
                (PadShape::RoundedRectangle, "▣ Rounded Rectangle"),
            ] {
                done |= ui.selectable_value(&mut g.shape, v, s).changed();
            }
        });
    done
}
fn copper(ui: &mut egui::Ui, item: &mut Item, layer: u16, name: &str, units: Units) -> bool {
    let Some(mut g) = item.pad_geometry(layer) else {
        return false;
    };
    let old = g.clone();
    let mut done = false;
    let w = ui.available_width();
    let mut open = false;
    let via = matches!(item.shape, Shape::Via { .. });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        cell(ui, w * 0.22, |ui| {
            open = expanded(ui, "open", name, eda_render::pcb::layer_color(layer)).0
        });
        cell(ui, w * 0.2, |ui| done |= numeric(ui, &mut g.size.x, units));
        if via {
            label(ui, w * 0.2, "");
        } else {
            cell(ui, w * 0.2, |ui| done |= numeric(ui, &mut g.size.y, units));
        }
        cell(ui, w * 0.25, |ui| {
            if via {
                ui.label("● Round");
            } else {
                done |= combo_shape(ui, &mut g);
            }
        });
        cell(ui, w * 0.13, |ui| {
            ui.add_enabled(false, egui::Checkbox::without_text(&mut false))
                .on_disabled_hover_text("Per-layer thermal-relief overrides are not implemented");
        });
    });
    if via {
        g.size.y = g.size.x;
    }
    if open {
        row(ui, "Shape", |ui| {
            ui.add_enabled_ui(!via, |ui| done |= combo_shape(ui, &mut g));
        });
        row(ui, if via { "Diameter" } else { "(X/Y)" }, |ui| {
            let width = (ui.available_width() - 4.0) / if via { 1.0 } else { 2.0 };
            cell(ui, width, |ui| done |= numeric(ui, &mut g.size.x, units));
            if !via {
                cell(ui, width, |ui| done |= numeric(ui, &mut g.size.y, units));
            } else {
                g.size.y = g.size.x;
            }
        });
        if g.shape == PadShape::RoundedRectangle {
            row(ui, "Corner Radius", |ui| {
                let before = g.corner_percent;
                let r = ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::DragValue::new(&mut g.corner_percent)
                        .range(0..=100)
                        .suffix("%"),
                );
                done |= completed(ui, &r, &mut g.corner_percent, before);
            });
        }
        row(ui, "Thermal Relief", |ui| {
            ui.add_enabled(false, egui::Checkbox::without_text(&mut false));
        });
    }
    if g != old {
        item.set_pad_geometry(layer, g);
    }
    done
}
fn drill_shape(ui: &mut egui::Ui, value: &mut DrillShape) -> bool {
    let mut done = false;
    egui::ComboBox::from_id_salt("drill_shape")
        .truncate()
        .width((ui.available_width() - 8.0).max(20.0))
        .selected_text(format!("{value:?}"))
        .show_ui(ui, |ui| {
            for (v, label) in [
                (DrillShape::Round, "● Round"),
                (DrillShape::Square, "■ Square"),
                (DrillShape::Slot, "▰ Slot"),
            ] {
                done |= ui.selectable_value(value, v, label).changed();
            }
        });
    done
}
fn hole(ui: &mut egui::Ui, item: &mut Item, units: Units) -> bool {
    let via = matches!(item.shape, Shape::Via { .. });
    let mut stack = item
        .pad_stack
        .as_deref()
        .cloned()
        .unwrap_or_else(|| pcb::PadStack {
            mode: item.stack_mode(),
            ..Default::default()
        });
    let old = stack.clone();
    let mut size = item.drill_size();
    let before_size = size;
    let mut plated = matches!(
        item.shape,
        Shape::Via { .. } | Shape::Pad { plated: true, .. }
    );
    let before_plated = plated;
    let mut done = false;
    let mut open = false;
    let w = ui.available_width();
    heading(ui, ["HOLE", "Size", "Length", "Shape", "Plated"]);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        cell(ui, w * 0.22, |ui| {
            open = expanded(
                ui,
                "hole_open",
                if via { "Via Hole" } else { "Pad Hole" },
                Color32::from_rgb(0, 170, 170),
            )
            .0
        });
        cell(ui, w * 0.2, |ui| done |= numeric(ui, &mut size, units));
        cell(ui, w * 0.2, |ui| {
            ui.add_enabled_ui(stack.drill_shape == DrillShape::Slot, |ui| {
                done |= numeric(ui, &mut stack.slot_length, units)
            });
        });
        cell(ui, w * 0.25, |ui| {
            ui.add_enabled_ui(!via, |ui| done |= drill_shape(ui, &mut stack.drill_shape));
        });
        cell(ui, w * 0.13, |ui| {
            done |= ui
                .add_enabled(!via, egui::Checkbox::without_text(&mut plated))
                .changed()
        });
    });
    if open {
        row(ui, "Shape", |ui| {
            ui.add_enabled_ui(!via, |ui| done |= drill_shape(ui, &mut stack.drill_shape));
        });
        done |= numeric_row(ui, "Size", &mut size, units);
        read_row(ui, "Tolerance", "N/A");
        ui.add_enabled_ui(stack.drill_shape == DrillShape::Slot, |ui| {
            done |= numeric_row(ui, "Length", &mut stack.slot_length, units);
            done |= angle(ui, "Rotation", &mut stack.drill_rotation);
        });
        let mut offset = item.pad_geometry(TOP).unwrap().offset;
        let previous = offset;
        ui.add_enabled_ui(!via, |ui| {
            done |= coordinates(
                ui,
                "Copper Offset (X/Y)",
                &mut offset,
                Point::default(),
                units,
            )
        });
        if previous != offset {
            let layers: Vec<_> = if stack.layers.is_empty() {
                vec![TOP]
            } else {
                stack.layers.keys().copied().collect()
            };
            for layer in layers {
                let mut g = item.pad_geometry(layer).unwrap();
                g.offset = offset;
                stack.layers.insert(layer, g);
            }
        }
        row(ui, "Plated", |ui| {
            done |= ui
                .add_enabled(!via, egui::Checkbox::without_text(&mut plated))
                .changed()
        });
    }
    if stack.drill_shape == DrillShape::Slot && old.drill_shape != DrillShape::Slot {
        stack.slot_length = stack.slot_length.max(size);
    }
    if size != before_size {
        match &mut item.shape {
            Shape::Via { hole, .. } | Shape::Pad { hole, .. } => *hole = size,
            _ => {}
        }
    }
    if plated != before_plated
        && let Shape::Pad { plated: p, .. } = &mut item.shape
    {
        *p = plated;
    }
    if stack != old {
        item.pad_stack = Some(Box::new(stack));
    }
    done
}
fn mask_shape(ui: &mut egui::Ui, mode: &mut MaskMode) -> bool {
    let mut done = false;
    egui::ComboBox::from_id_salt("mask_mode")
        .truncate()
        .width((ui.available_width() - 8.0).max(20.0))
        .selected_text(if *mode == MaskMode::Rule {
            "Rule Expansion"
        } else {
            "Manual Expansion"
        })
        .show_ui(ui, |ui| {
            done |= ui
                .selectable_value(mode, MaskMode::Rule, "Rule Expansion")
                .changed();
            done |= ui
                .selectable_value(mode, MaskMode::Manual, "Manual Expansion")
                .changed();
            for label in [
                "● Round",
                "■ Rectangular",
                "◆ Octagonal",
                "▣ Rounded Rectangle",
                "Chamfered Rectangle",
                "Custom Shape",
            ] {
                ui.add_enabled(false, egui::Button::new(label))
                    .on_disabled_hover_text("Custom mask aperture geometry is not implemented yet");
            }
        });
    done
}
fn mask_rows(
    ui: &mut egui::Ui,
    item: &mut Item,
    board: &Board,
    units: Units,
    kind: MaskKind,
    panel: &mut Panel,
) -> bool {
    let mut settings = item
        .masks
        .as_deref()
        .cloned()
        .unwrap_or_else(|| MaskSettings::for_item(item));
    let old = settings.clone();
    let mut done = false;
    let paste = kind == MaskKind::Paste;
    heading(
        ui,
        [
            if paste { "PASTE" } else { "SOLDER" },
            "Expansion",
            if paste { "%" } else { "" },
            "Shape",
            if paste { "Enable" } else { "Tented" },
        ],
    );
    let mut any_open = false;
    for bottom in [false, true] {
        ui.push_id((paste, bottom), |ui| {
            let resolved = board.resolve_mask(item, kind, bottom);
            let side = settings.side_mut(kind, bottom);
            let mut open = false;
            let w = ui.available_width();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                cell(ui, w * 0.22, |ui| {
                    open = expanded(
                        ui,
                        "mask_open",
                        if bottom { "Bottom" } else { "Top" },
                        eda_render::pcb::layer_color(match (paste, bottom) {
                            (true, false) => 35,
                            (true, true) => 36,
                            (false, false) => 37,
                            _ => 38,
                        }),
                    )
                    .0
                });
                cell(ui, w * 0.2, |ui| {
                    if side.mode == MaskMode::Manual {
                        ui.add_enabled_ui(side.enabled && !side.tented, |ui| {
                            done |= numeric(ui, &mut side.expansion, units)
                        });
                    } else {
                        ui.add(
                            egui::Label::new(
                                resolved
                                    .as_ref()
                                    .map(|r| {
                                        format!(
                                            "{:.3}{}",
                                            units.from_nm(r.expansion as f64),
                                            units.label()
                                        )
                                    })
                                    .unwrap_or_else(|_| "—".into()),
                            )
                            .truncate(),
                        );
                    }
                });
                cell(ui, w * 0.2, |ui| {
                    if paste {
                        let percentage = if side.mode == MaskMode::Rule {
                            resolved
                                .as_ref()
                                .map(|r| r.percentage)
                                .unwrap_or(side.percentage)
                        } else {
                            side.percentage
                        };
                        ui.add(egui::Label::new(format!("{percentage}%")).truncate());
                    }
                });
                cell(ui, w * 0.25, |ui| done |= mask_shape(ui, &mut side.mode));
                cell(ui, w * 0.13, |ui| {
                    done |= ui
                        .add(egui::Checkbox::without_text(if paste {
                            &mut side.enabled
                        } else {
                            &mut side.tented
                        }))
                        .changed()
                });
            });
            any_open |= open;
            if open {
                row(ui, "Shape", |ui| done |= mask_shape(ui, &mut side.mode));
                let mut expansion = if side.mode == MaskMode::Rule {
                    resolved
                        .as_ref()
                        .map(|r| r.expansion)
                        .unwrap_or(side.expansion)
                } else {
                    side.expansion
                };
                ui.add_enabled_ui(
                    side.mode == MaskMode::Manual && side.enabled && !side.tented,
                    |ui| {
                        done |= numeric_row(
                            ui,
                            if paste {
                                "Paste Expansion"
                            } else {
                                "Solder Expansion"
                            },
                            &mut expansion,
                            units,
                        )
                    },
                );
                if side.mode == MaskMode::Manual {
                    side.expansion = expansion;
                }
                if paste {
                    let mut percentage = if side.mode == MaskMode::Rule {
                        resolved
                            .as_ref()
                            .map(|r| r.percentage)
                            .unwrap_or(side.percentage)
                    } else {
                        side.percentage
                    };
                    ui.add_enabled_ui(side.mode == MaskMode::Manual && side.enabled, |ui| {
                        row(ui, "Percentage", |ui| {
                            let before = percentage;
                            let r = ui.add_sized(
                                [ui.available_width(), 22.0],
                                egui::DragValue::new(&mut percentage)
                                    .range(-100.0..=1000.0)
                                    .suffix("%"),
                            );
                            done |= completed(ui, &r, &mut percentage, before);
                        });
                    });
                    if side.mode == MaskMode::Manual {
                        side.percentage = percentage;
                    }
                }
                if !paste {
                    row(ui, "From Hole Edge", |ui| {
                        done |= ui.checkbox(&mut side.from_hole_edge, "").changed()
                    });
                }
                row(ui, if paste { "Enabled" } else { "Tented" }, |ui| {
                    done |= ui
                        .checkbox(
                            if paste {
                                &mut side.enabled
                            } else {
                                &mut side.tented
                            },
                            "",
                        )
                        .changed()
                });
                if side.mode == MaskMode::Rule {
                    match &resolved {
                        Ok(r) => row(ui, "Rule", |ui| {
                            let name = r
                                .rule
                                .and_then(|n| board.rules[n].get("NAME"))
                                .map(String::as_str)
                                .unwrap_or("Default");
                            label(ui, (ui.available_width() - 29.0).max(10.0), name);
                            if ui
                                .add_enabled(r.rule.is_some(), egui::Button::new("…"))
                                .clicked()
                            {
                                let index = r.rule.unwrap();
                                panel.rule_editor = Some(RuleEditor {
                                    index,
                                    original: board.rules[index].clone(),
                                    value: board.rules[index].clone(),
                                });
                            }
                        }),
                        Err(e) => {
                            ui.add(
                                egui::Label::new(RichText::new(e).color(Color32::LIGHT_RED)).wrap(),
                            );
                        }
                    }
                }
            }
        });
    }
    let linked = if paste {
        settings.linked_paste
    } else {
        settings.linked_solder
    };
    if linked {
        for bottom in [false, true] {
            let before = old.side(kind, bottom);
            let after = settings.side(kind, bottom).clone();
            if before.mode != after.mode
                || before.expansion != after.expansion
                || before.percentage != after.percentage
                || before.from_hole_edge != after.from_hole_edge
            {
                let other = settings.side_mut(kind, !bottom);
                other.mode = after.mode;
                other.expansion = after.expansion;
                other.percentage = after.percentage;
                other.from_hole_edge = after.from_hole_edge;
                break;
            }
        }
    }
    if any_open {
        row(ui, "", |ui| {
            let target = if paste {
                &mut settings.linked_paste
            } else {
                &mut settings.linked_solder
            };
            if ui
                .add(
                    egui::Button::new("↔")
                        .selected(*target)
                        .min_size(egui::vec2(22.0, 22.0)),
                )
                .on_hover_text("Link top and bottom expansion")
                .clicked()
            {
                *target = !*target;
                done = true;
            }
        });
    }
    if old != settings {
        item.masks = Some(Box::new(settings));
    }
    done
}
pub(super) fn show(
    ui: &mut egui::Ui,
    item: &mut Item,
    board: &Board,
    units: Units,
    query: &str,
    panel: &mut Panel,
) -> bool {
    let mut done = false;
    let via = matches!(item.shape, Shape::Via { .. });
    section(
        ui,
        if via { "Via Stack" } else { "Pad Stack" },
        query,
        |ui| {
            egui::Frame::NONE
                .fill(Color32::from_gray(32))
                .corner_radius(3)
                .inner_margin(8)
                .show(ui, |ui| {
                    if ui
                        .add(
                            egui::Button::new(if panel.hide_preview {
                                "Show preview"
                            } else {
                                "Hide preview"
                            })
                            .frame(false),
                        )
                        .clicked()
                    {
                        panel.hide_preview = !panel.hide_preview;
                    }
                    if !panel.hide_preview {
                        eda_render::pcb_stack::preview(ui, board, item, &mut panel.preview, 230.0);
                    }
                });
            ui.add_enabled_ui(!item.locked, |ui| {
                let mode = item.stack_mode();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let w = ui.available_width() / 3.0;
                    for (v, label) in [
                        (StackMode::Simple, "Simple"),
                        (StackMode::TopMiddleBottom, "Top-Middle-Bottom"),
                        (StackMode::Full, "Full Stack"),
                    ] {
                        if ui
                            .add_sized(
                                [w, 22.0],
                                egui::Button::new(label)
                                    .selected(mode == v)
                                    .wrap_mode(egui::TextWrapMode::Truncate),
                            )
                            .clicked()
                        {
                            item.set_stack_mode(v, board);
                            done = true;
                        }
                    }
                });
                egui::Frame::NONE
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(0)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        heading(
                            ui,
                            [
                                "COPPER",
                                if via { "Diameter" } else { "X Size" },
                                if via { "" } else { "Y Size" },
                                "Shape",
                                "Relief",
                            ],
                        );
                        let layers: Vec<(u16, String)> = match item.stack_mode() {
                            StackMode::Simple => vec![(TOP, "All Layers".into())],
                            StackMode::TopMiddleBottom => vec![
                                (TOP, "Top Layer".into()),
                                (2, "Mid-Layers".into()),
                                (BOTTOM, "Bottom Layer".into()),
                            ],
                            StackMode::Full => {
                                let mut l: Vec<_> =
                                    board.layers.iter().filter(|l| l.copper).collect();
                                l.sort_by_key(|l| l.order);
                                l.into_iter().map(|l| (l.id, l.name.clone())).collect()
                            }
                        };
                        for (layer, name) in layers {
                            ui.push_id(layer, |ui| done |= copper(ui, item, layer, &name, units));
                        }
                        done |= hole(ui, item, units);
                        if !via {
                            done |= mask_rows(ui, item, board, units, MaskKind::Paste, panel);
                        }
                        done |= mask_rows(ui, item, board, units, MaskKind::Solder, panel);
                    });
            });
        },
    );
    if !via {
        section(ui, "Pad Features", query, |ui| {
            for label in ["Top Side", "Bottom Side"] {
                row(ui, label, |ui| disabled(ui, "None"));
            }
        });
    }
    done
}
pub(super) fn rule_dialog(
    ctx: &egui::Context,
    board: &Board,
    units: Units,
    panel: &mut Panel,
) -> Option<Result<Board, String>> {
    let d = panel.rule_editor.as_mut()?;
    let mut result = None;
    let mut close = false;
    egui::Window::new("Mask Expansion Rule")
        .id(egui::Id::new("pcb_mask_rule"))
        .collapsible(false)
        .resizable(false)
        .default_width(450.0)
        .show(ctx, |ui| {
            for key in ["NAME", "SCOPE1EXPRESSION"] {
                let mut value = property(&d.value, key).to_owned();
                text_row(
                    ui,
                    if key == "NAME" {
                        "Name"
                    } else {
                        "Where The Object Matches"
                    },
                    &mut value,
                );
                d.value.insert(key.into(), value);
            }
            let mut enabled = !property(&d.value, "ENABLED").eq_ignore_ascii_case("FALSE");
            if ui.checkbox(&mut enabled, "Enabled").changed() {
                d.value.insert(
                    "ENABLED".into(),
                    if enabled { "TRUE" } else { "FALSE" }.into(),
                );
            }
            let mut priority = property(&d.value, "PRIORITY").parse::<u32>().unwrap_or(1);
            row(ui, "Priority", |ui| {
                if ui
                    .add(egui::DragValue::new(&mut priority).range(1..=10000))
                    .changed()
                {
                    d.value.insert("PRIORITY".into(), priority.to_string());
                }
            });
            let mut expansion =
                eda_io::altium_pcb::dimension(property(&d.value, "EXPANSION")).unwrap_or(0);
            if numeric_row(ui, "Expansion", &mut expansion, units) {
                d.value
                    .insert("EXPANSION".into(), format!("{}mm", expansion as f64 / 1e6));
            }
            for (key, label) in [
                ("ISTENTINGTOP", "Tented Top"),
                ("ISTENTINGBOTTOM", "Tented Bottom"),
            ] {
                let mut on = property(&d.value, key).eq_ignore_ascii_case("TRUE");
                if ui.checkbox(&mut on, label).changed() {
                    d.value
                        .insert(key.into(), if on { "TRUE" } else { "FALSE" }.into());
                }
            }
            if board.rules.get(d.index) != Some(&d.original) {
                ui.colored_label(Color32::LIGHT_RED, "The rule changed. Reopen the editor.");
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        board.rules.get(d.index) == Some(&d.original),
                        egui::Button::new("OK"),
                    )
                    .clicked()
                {
                    result = Some(board.replace_mask_rule(d.index, d.value.clone()));
                    close = result.as_ref().is_some_and(Result::is_ok);
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            });
        });
    if close {
        panel.rule_editor = None;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expanded_stack_rows_fit_the_native_dock_and_do_not_mutate_the_document() {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let b = Board::default();
        for mode in [
            StackMode::Simple,
            StackMode::TopMiddleBottom,
            StackMode::Full,
        ] {
            let mut item = Item::new(
                pcb::MULTI_LAYER,
                None,
                Shape::Pad {
                    position: Point::default(),
                    size: Point::mm(2, 2),
                    rotation: 0.0,
                    shape: PadShape::RoundedRectangle,
                    hole: 500_000,
                    plated: true,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            item.set_stack_mode(mode, &b);
            let mut masks = MaskSettings::for_item(&item);
            for side in &mut masks.solder {
                side.mode = MaskMode::Manual;
                side.expansion = 75_999;
            }
            item.masks = Some(Box::new(masks));
            let before = item.clone();
            for _ in 0..60 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1707.0, 2000.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        let p = egui::SidePanel::right("expanded_stack_width")
                            .default_width(316.0)
                            .show(ctx, |ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.data_mut(|d| {
                                    d.insert_persisted(ui.make_persistent_id("open"), true);
                                    d.insert_persisted(ui.make_persistent_id("hole_open"), true);
                                });
                                assert!(!copper(
                                    ui,
                                    &mut item,
                                    TOP,
                                    "Top Layer",
                                    Units::Millimetres
                                ));
                                assert!(!hole(ui, &mut item, Units::Millimetres));
                                for kind in [MaskKind::Paste, MaskKind::Solder] {
                                    for bottom in [false, true] {
                                        ui.push_id((kind == MaskKind::Paste, bottom), |ui| {
                                            ui.data_mut(|d| {
                                                d.insert_persisted(
                                                    ui.make_persistent_id("mask_open"),
                                                    true,
                                                )
                                            });
                                        });
                                    }
                                    assert!(!mask_rows(
                                        ui,
                                        &mut item,
                                        &b,
                                        Units::Millimetres,
                                        kind,
                                        &mut Panel::default()
                                    ));
                                }
                            });
                        assert!(
                            p.response.rect.width() <= 317.0,
                            "{}",
                            p.response.rect.width()
                        );
                    },
                );
            }
            assert_eq!(item, before);
        }
    }
}
