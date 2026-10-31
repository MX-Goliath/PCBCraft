use super::*;
use eda_core::schematic_harness::{self as harness, Definition};

impl PcbCraft {
    pub(crate) fn schematic_harness_overlay(
        &mut self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        index: usize,
    ) {
        if self.documents[index].sch_editor.paused {
            egui::Area::new(egui::Id::new("schematic_placement_pause"))
                .order(egui::Order::Foreground)
                .fixed_pos(canvas.center() - egui::vec2(32.0, 32.0))
                .show(ctx, |ui| {
                    if ui
                        .add_sized(
                            [64.0, 64.0],
                            egui::Button::new(egui::RichText::new("II").size(34.0))
                                .corner_radius(32.0),
                        )
                        .clicked()
                    {
                        self.documents[index].sch_editor.paused = false;
                    }
                });
        }
        if !self.documents[index].sch_editor.predefined {
            return;
        }
        let mut definitions: Vec<Definition> = vec![];
        let mut error = None;
        for doc in &self.documents {
            if let Some(s) = &doc.model.schematic {
                match harness::definitions(s) {
                    Ok(ds) => {
                        for d in ds {
                            if !definitions
                                .iter()
                                .any(|v| v.name.eq_ignore_ascii_case(&d.name))
                            {
                                definitions.push(d);
                            }
                        }
                    }
                    Err(e) => error = Some(e),
                }
            }
        }
        let mut files = std::collections::BTreeSet::new();
        for project in &self.projects {
            for entry in &project.documents {
                if entry
                    .path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("Harness"))
                {
                    files.insert(entry.path.clone());
                }
            }
        }
        for doc in &self.documents {
            if let Some(path) = doc.source_path.as_ref().or(doc.path.as_ref()) {
                let path = path.with_extension("Harness");
                if path.is_file() {
                    files.insert(path);
                }
            }
        }
        for file in files {
            if std::fs::metadata(&file).is_ok_and(|m| m.len() <= 1024 * 1024) {
                match eda_io::altium::load_harness_definitions(&file) {
                    Ok(ds) => {
                        for d in ds {
                            if let Some(old) = definitions
                                .iter_mut()
                                .find(|v| v.name.eq_ignore_ascii_case(&d.name))
                            {
                                if d.locked && old.entries != d.entries {
                                    error = Some(format!(
                                        "Conflicting Harness Definition for {}",
                                        d.name
                                    ));
                                } else {
                                    // A dirty sheet is newer than its generated,
                                    // unlocked sidecar. Reuse the editable shape.
                                    old.locked |= d.locked;
                                }
                            } else {
                                definitions.push(d);
                            }
                        }
                    }
                    Err(e) => error = Some(format!("{}: {e}", file.display())),
                }
            }
        }
        definitions.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        let editor = &mut self.documents[index].sch_editor;
        let mut open = true;
        let mut place = None;
        egui::Window::new("Place Predefined Harness Connector")
            .id(egui::Id::new("predefined_harness_dialog"))
            .collapsible(false)
            .resizable(false)
            .fixed_size([592.0, 342.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(307.0, 302.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(egui::RichText::new("Harness connectors").strong());
                            egui::Frame::NONE
                                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                                .show(ui, |ui| {
                                    ui.set_min_width(291.0);
                                    egui::ScrollArea::vertical()
                                        .min_scrolled_height(228.0)
                                        .max_height(228.0)
                                        .auto_shrink([false, false])
                                        .show(ui, |ui| {
                                            egui::CollapsingHeader::new("Free Documents")
                                                .default_open(true)
                                                .show(ui, |ui| {
                                                    for (i, d) in definitions
                                                        .iter()
                                                        .enumerate()
                                                        .filter(|(_, d)| {
                                                            d.name.to_lowercase().contains(
                                                                &editor
                                                                    .predefined_filter
                                                                    .to_lowercase(),
                                                            )
                                                        })
                                                    {
                                                        let response = ui.selectable_label(
                                                            editor.predefined_selected == i,
                                                            &d.name,
                                                        );
                                                        if response.clicked() {
                                                            editor.predefined_selected = i;
                                                        }
                                                        if editor.predefined_selected == i {
                                                            ui.indent(d.name.as_str(), |ui| {
                                                                for entry in &d.order {
                                                                    ui.label(entry);
                                                                }
                                                            });
                                                        }
                                                    }
                                                });
                                        });
                                });
                            ui.horizontal(|ui| {
                                ui.label("Filter");
                                ui.add_sized(
                                    [255.0, 20.0],
                                    egui::TextEdit::singleline(&mut editor.predefined_filter),
                                );
                            });
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(261.0, 302.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(egui::RichText::new("Additional Objects").strong());
                            ui.checkbox(&mut editor.predefined_port, "Add Port");
                            ui.add_enabled_ui(editor.predefined_port, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized([82.0, 22.0], egui::Label::new("Name"));
                                    ui.add_sized(
                                        [136.0, 22.0],
                                        egui::TextEdit::singleline(&mut editor.predefined_name),
                                    );
                                });
                                ui.horizontal(|ui| {
                                    ui.add_sized([82.0, 22.0], egui::Label::new("Width"));
                                    ui.add_enabled_ui(!editor.predefined_auto, |ui| {
                                        ui.add_sized(
                                            [76.0, 22.0],
                                            egui::DragValue::new(&mut editor.predefined_width)
                                                .custom_formatter(|n, _| {
                                                    format!("{}mil", n / 25_400.0)
                                                })
                                                .custom_parser(|s| {
                                                    s.trim_end_matches("mil")
                                                        .trim()
                                                        .parse::<f64>()
                                                        .ok()
                                                        .map(|n| n * 25_400.0)
                                                })
                                                .speed(254_000.0)
                                                .range(254_000..=1_000_000_000),
                                        );
                                    });
                                    ui.checkbox(&mut editor.predefined_auto, "Auto");
                                });
                            });
                            ui.add_space(22.0);
                            ui.checkbox(&mut editor.predefined_line, "Add Signal Harness");
                            ui.add_enabled_ui(editor.predefined_line, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add_sized([110.0, 22.0], egui::Label::new("Harness Length"));
                                    ui.add_sized(
                                        [136.0, 22.0],
                                        egui::DragValue::new(&mut editor.predefined_length)
                                            .custom_formatter(|n, _| format!("{}mil", n / 25_400.0))
                                            .custom_parser(|s| {
                                                s.trim_end_matches("mil")
                                                    .trim()
                                                    .parse::<f64>()
                                                    .ok()
                                                    .map(|n| n * 25_400.0)
                                            })
                                            .speed(254_000.0)
                                            .range(254_000..=1_000_000_000),
                                    );
                                });
                            });
                            ui.add_space(22.0);
                            ui.checkbox(&mut editor.predefined_sort, "Sort Harness Entries");
                        },
                    );
                });
                if let Some(e) = &error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized([74.0, 24.0], egui::Button::new("Cancel"))
                        .clicked()
                    {
                        editor.predefined = false;
                    }
                    if ui
                        .add_enabled(
                            error.is_none()
                                && definitions.get(editor.predefined_selected).is_some(),
                            egui::Button::new("OK").min_size(egui::vec2(74.0, 24.0)),
                        )
                        .clicked()
                    {
                        place = definitions.get(editor.predefined_selected).cloned();
                    }
                });
            });
        if !open {
            self.documents[index].sch_editor.predefined = false;
        }
        if let Some(d) = place {
            let doc = &mut self.documents[index];
            let s = doc.model.schematic.as_ref().unwrap();
            let height = ((d.entries.len() + 1) as i64 * 2_540_000).max(10_160_000);
            let a = Point::default();
            let b = Point {
                x: 12_700_000,
                y: -height,
            };
            let result = (|| -> Result<Fragment, String> {
                let mut f = harness::connector(a, b, s.sheet.system_font, &d.name)?;
                let id = f.primitives[0].id;
                let mut scratch = s.clone();
                scratch.primitives.extend(f.primitives.clone());
                let names = if doc.sch_editor.predefined_sort {
                    d.entries.keys().collect::<Vec<_>>()
                } else {
                    d.order.iter().collect()
                };
                for (n, name) in names.into_iter().enumerate() {
                    let kind = &d.entries[name];
                    let mut p = harness::entry(
                        &scratch,
                        id,
                        Point {
                            x: 0,
                            y: -(n as i64 + 1) * 2_540_000,
                        },
                        name,
                    )?;
                    p.presentation.harness_type = kind.clone();
                    f.primitives.push(p);
                }
                let start = harness::terminal(&f.primitives[0]).unwrap();
                let end = Point {
                    x: start.x
                        + if doc.sch_editor.predefined_line {
                            doc.sch_editor.predefined_length
                        } else {
                            0
                        },
                    y: start.y,
                };
                if doc.sch_editor.predefined_line {
                    let mut p = edit::primitive(
                        K::Polyline {
                            points: vec![start, end],
                            closed: false,
                            electrical: "Signal Harness".into(),
                        },
                        None,
                    );
                    p.color = harness::SIGNAL_COLOR;
                    p.width_nm = 508_000;
                    p.presentation.record_type = 218;
                    p.presentation.allow_sync = true;
                    f.primitives.push(p);
                }
                if doc.sch_editor.predefined_port {
                    let mut p = edit::place(
                        Placement::Port,
                        end,
                        0,
                        "",
                        &doc.sch_editor.predefined_name,
                        s.sheet.system_font,
                    )?
                    .primitives
                    .remove(0);
                    if let K::Port { width_nm, .. } = &mut p.kind {
                        *width_nm = if doc.sch_editor.predefined_auto {
                            ((doc.sch_editor.predefined_name.chars().count() as i64) * 1_524_000
                                + 2_540_000)
                                .max(12_700_000)
                        } else {
                            doc.sch_editor.predefined_width
                        };
                    }
                    p.presentation.harness_type = d.name.clone();
                    f.primitives.push(p);
                }
                Ok(f)
            })();
            match result {
                Ok(fragment) => {
                    doc.sch_editor.cancel(&mut doc.view);
                    doc.sch_editor.predefined = false;
                    doc.sch_editor.tool = Tool::Move {
                        fragment,
                        selection: Selection::default(),
                        connected: false,
                        anchor: Point::default(),
                        turns: 0,
                        revision: doc.history.revision,
                        drag: false,
                    };
                    doc.view.sch_tool_active = true;
                }
                Err(e) => self.message(true, e),
            }
        }
    }
}
