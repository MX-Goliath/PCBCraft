//! The wizard owns an isolated rule draft until Finish.
use super::*;
use egui::{Rect, Sense, Stroke, UiBuilder, pos2, vec2};
pub(super) struct Wizard {
    step: u8,
    kind: String,
    name: String,
    comment: String,
    scope: usize,
    nets: BTreeSet<String>,
    layer: String,
    component: String,
    custom: String,
    priority: usize,
    launch: bool,
}
impl Default for Wizard {
    fn default() -> Self {
        Self {
            step: 0,
            kind: "Clearance".into(),
            name: String::new(),
            comment: String::new(),
            scope: 0,
            nets: BTreeSet::new(),
            layer: String::new(),
            component: String::new(),
            custom: "All".into(),
            priority: 0,
            launch: true,
        }
    }
}
fn expression(w: &Wizard) -> String {
    let quote = |v: &str| v.replace('\'', "''");
    let nets = if w.nets.is_empty() {
        "All".into()
    } else {
        format!(
            "({})",
            w.nets
                .iter()
                .map(|n| format!("InNet('{}')", quote(n)))
                .collect::<Vec<_>>()
                .join(" Or ")
        )
    };
    match w.scope {
        0 => "All".into(),
        1 | 2 => nets,
        3 => format!("{nets} And OnLayer('{}')", quote(&w.layer)),
        4 => format!("{nets} And InComponent('{}')", quote(&w.component)),
        _ => w.custom.clone(),
    }
}
fn illustration(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, 0.0, Color32::from_gray(47));
    // Original vector artwork, independent of Altium's bundled raster artwork.
    for (i, color) in [
        Color32::from_rgb(68, 88, 102),
        Color32::from_rgb(111, 100, 68),
        Color32::from_rgb(50, 86, 69),
    ]
    .into_iter()
    .enumerate()
    {
        let c = rect.center() + vec2(0.0, (i as f32 - 1.0) * 45.0);
        let pts = vec![
            c + vec2(-90.0, -35.0),
            c + vec2(25.0, -75.0),
            c + vec2(100.0, 25.0),
            c + vec2(-15.0, 65.0),
        ];
        p.add(egui::Shape::convex_polygon(
            pts,
            color,
            Stroke::new(1.0, Color32::GRAY),
        ));
        for n in 0..8 {
            let a = c + vec2(-54.0 + n as f32 * 13.0, -12.0 + n as f32 * 2.0);
            p.circle_stroke(a, 3.0, Stroke::new(1.5, widgets::GOLD));
            p.line_segment([a, a + vec2(21.0, 25.0)], Stroke::new(1.0, widgets::GOLD));
        }
    }
}
fn finish(d: &mut Draft, w: &Wizard) {
    insert_rule(d, &w.kind, None);
    let selected = d.selected;
    let r = &mut d.rules[selected];
    if !w.name.trim().is_empty() {
        r.insert("NAME".into(), w.name.trim().into());
    }
    r.insert("COMMENT".into(), w.comment.clone());
    r.insert("SCOPE1EXPRESSION".into(), expression(w));
    let mut indices = sorted_rules(d, &w.kind);
    indices.retain(|i| *i != selected);
    indices.insert(w.priority.min(indices.len()), selected);
    for (p, i) in indices.iter().enumerate() {
        d.rules[*i].insert("PRIORITY".into(), (p + 1).to_string());
    }
    d.page = if w.launch {
        RulePage::Rule
    } else {
        RulePage::Kind(w.kind.clone())
    };
}
pub(super) fn show(ctx: &egui::Context, board: &Board, d: &mut Draft, w: &mut Wizard) -> bool {
    let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut complete = false;
    let valid_name = w.name.trim().is_empty()
        || !d.rules.iter().any(|r| {
            text(r, "RULEKIND") == w.kind && text(r, "NAME").eq_ignore_ascii_case(w.name.trim())
        });
    let valid_scope = match w.scope {
        0 => true,
        1 | 2 => !w.nets.is_empty(),
        3 => !w.nets.is_empty() && !w.layer.is_empty(),
        4 => !w.nets.is_empty() && !w.component.is_empty(),
        _ => query_ui::check(board, &w.custom).is_ok(),
    };
    let titles = [
        "Design Rule Wizard",
        "Choose the Rule Type",
        "Choose the Rule Scope",
        "Choose the Rule Priority",
        "The New Rule is Complete",
    ];
    let descriptions = [
        "This wizard will guide you through the steps of creating a new Design Rule",
        "Choose the type of rule you wish to create below. Give it a name and description.",
        "Choose from the options below the one that best describes the objects that you wish this rule to apply to. You will be able to refine this choice later.",
        "Set the rule priority in the list below. Priority one is highest. The rule with the highest priority whose scope matches the checked objects will be used.",
        "This new rule is complete. Check the box below to edit the rule in the main design rule editor dialog.",
    ];
    egui::Window::new("New Rule Wizard")
        .id(egui::Id::new("new_rule_wizard_v3"))
        .title_bar(false)
        .fixed_size([644.0, 490.0])
        .resizable(false)
        .collapsible(false)
        .movable(false)
        .current_pos(widgets::position(
            ctx,
            egui::Id::new("new_rule_wizard_v3"),
            ctx.screen_rect().center() - vec2(329.0, 252.0),
        ))
        .frame(
            egui::Frame::new()
                .fill(Color32::from_gray(76))
                .inner_margin(7.0)
                .stroke(Stroke::new(1.0, widgets::LINE)),
        )
        .show(ctx, |ui| {
            widgets::style(ui);
            close |= widgets::title(ui, "New Rule Wizard");
            let (body, _) = ui.allocate_exact_size(vec2(644.0, 435.0), Sense::hover());
            ui.painter().rect_filled(body, 0.0, widgets::BG);
            if w.step == 0 {
                let art = Rect::from_min_size(body.min, vec2(130.0, body.height()));
                illustration(&ui.painter_at(art), art);
                widgets::child(
                    ui,
                    UiBuilder::new().max_rect(Rect::from_min_max(
                        body.min + vec2(146.0, 95.0),
                        body.max - vec2(15.0, 15.0),
                    )),
                    |ui| {
                        ui.label(egui::RichText::new(titles[0]).size(22.0).strong());
                        ui.add_space(25.0);
                        ui.label(egui::RichText::new(descriptions[0]).size(16.0));
                    },
                );
            } else {
                let header = Rect::from_min_size(body.min, vec2(body.width(), 80.0));
                ui.painter()
                    .rect_filled(header, 0.0, Color32::from_gray(46));
                let art = Rect::from_min_size(
                    pos2(header.right() - 240.0, header.top()),
                    vec2(240.0, 80.0),
                );
                illustration(&ui.painter_at(art), art);
                let text_rect =
                    Rect::from_min_size(header.min + vec2(14.0, 13.0), vec2(385.0, 64.0));
                widgets::child(ui, UiBuilder::new().max_rect(text_rect), |ui| {
                    ui.strong(titles[w.step as usize]);
                    ui.label(descriptions[w.step as usize]);
                });
                let content =
                    Rect::from_min_max(body.min + vec2(12.0, 91.0), body.max - vec2(12.0, 12.0));
                widgets::child(ui, UiBuilder::new().max_rect(content), |ui| match w.step {
                    1 => {
                        ui.horizontal(|ui| {
                            ui.label("Name");
                            ui.add(egui::TextEdit::singleline(&mut w.name).desired_width(120.0));
                            ui.label("Comment");
                            ui.add(
                                egui::TextEdit::singleline(&mut w.comment)
                                    .desired_width(ui.available_width()),
                            );
                        });
                        ui.add_space(5.0);
                        egui::ScrollArea::vertical()
                            .id_salt("wizard_types")
                            .max_height(290.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for cat in CATEGORIES {
                                    egui::CollapsingHeader::new(*cat)
                                        .default_open(matches!(*cat, "Electrical" | "Routing"))
                                        .show(ui, |ui| {
                                            for (_, label, kind) in
                                                CATALOG.iter().filter(|(c, _, _)| c == cat)
                                            {
                                                if ui
                                                    .add_enabled(
                                                        rules::supported(kind),
                                                        egui::Button::selectable(
                                                            w.kind == *kind,
                                                            *label,
                                                        ),
                                                    )
                                                    .clicked()
                                                {
                                                    w.kind = (*kind).into();
                                                    w.priority = 0;
                                                }
                                            }
                                        });
                                }
                            });
                    }
                    2 => {
                        widgets::section(ui, "What this rule should apply to");
                        ui.add_space(7.0);
                        for (i, label) in [
                            "Whole Board",
                            "1 Net",
                            "A Few Nets",
                            "A Net on a Particular Layer",
                            "A Net in a Particular Component",
                            "Advanced (Start With a Blank Query)",
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            ui.radio_value(&mut w.scope, i, label);
                        }
                        ui.add_space(15.0);
                        if (1..=4).contains(&w.scope) {
                            ui.horizontal(|ui| {
                                ui.label("Net");
                                egui::ComboBox::from_id_salt("wizard_net")
                                    .width(250.0)
                                    .selected_text(
                                        w.nets.iter().cloned().collect::<Vec<_>>().join(", "),
                                    )
                                    .show_ui(ui, |ui| {
                                        for n in &board.nets {
                                            let mut selected = w.nets.contains(&n.name);
                                            if ui.checkbox(&mut selected, &n.name).changed() {
                                                if selected {
                                                    if w.scope != 2 {
                                                        w.nets.clear();
                                                    }
                                                    w.nets.insert(n.name.clone());
                                                } else {
                                                    w.nets.remove(&n.name);
                                                }
                                            }
                                        }
                                    });
                            });
                        }
                        if w.scope == 3 || w.scope == 4 {
                            let (function, label, value) = if w.scope == 3 {
                                ("OnLayer", "Layer", &mut w.layer)
                            } else {
                                ("InComponent", "Component", &mut w.component)
                            };
                            ui.horizontal(|ui| {
                                ui.label(label);
                                egui::ComboBox::from_id_salt("wizard_scope_value")
                                    .width(250.0)
                                    .selected_text(value.as_str())
                                    .show_ui(ui, |ui| {
                                        for option in query_ui::values(board, function) {
                                            ui.selectable_value(value, option.clone(), option);
                                        }
                                    });
                            });
                        }
                        if w.scope == 5 {
                            ui.add_sized(
                                [ui.available_width(), 100.0],
                                egui::TextEdit::multiline(&mut w.custom)
                                    .font(egui::TextStyle::Monospace),
                            );
                        }
                    }
                    3 => {
                        let indices = sorted_rules(d, &w.kind);
                        let mut rows: Vec<_> =
                            indices.iter().map(|i| d.rules[*i].clone()).collect();
                        let mut new = rules::default_rule(&w.kind);
                        new.insert(
                            "NAME".into(),
                            if w.name.is_empty() {
                                format!("{}_1", name(&w.kind))
                            } else {
                                w.name.clone()
                            },
                        );
                        new.insert("SCOPE1EXPRESSION".into(), expression(w));
                        rows.insert(w.priority.min(rows.len()), new);
                        egui::Frame::new()
                            .fill(Color32::from_gray(58))
                            .show(ui, |ui| {
                                widgets::table_header(
                                    ui,
                                    &[49.0, 48.0, 110.0, 112.0, 290.0],
                                    &["Priority", "Enabled", "Name", "Scope", "Attributes"],
                                );
                                egui::ScrollArea::both()
                                    .id_salt("wizard_priorities")
                                    .max_height(265.0)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.set_min_height(265.0);
                                        for (p, r) in rows.iter().enumerate() {
                                            ui.horizontal(|ui| {
                                                ui.add_sized(
                                                    [45.0, 21.0],
                                                    egui::Label::new((p + 1).to_string()),
                                                );
                                                let mut enabled = rules::enabled(r);
                                                ui.add_enabled(
                                                    false,
                                                    egui::Checkbox::without_text(&mut enabled),
                                                );
                                                ui.add_sized(
                                                    [110.0, 21.0],
                                                    egui::Label::new(text(r, "NAME")).truncate(),
                                                );
                                                ui.add_sized(
                                                    [112.0, 21.0],
                                                    egui::Label::new(text(r, "SCOPE1EXPRESSION"))
                                                        .truncate(),
                                                );
                                                ui.add(
                                                    egui::Label::new(widgets::attributes(r))
                                                        .truncate(),
                                                );
                                            });
                                        }
                                    });
                            });
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(w.priority > 0, egui::Button::new("Increase Priority"))
                                .clicked()
                            {
                                w.priority -= 1;
                            }
                            if ui
                                .add_enabled(
                                    w.priority < indices.len(),
                                    egui::Button::new("Decrease Priority"),
                                )
                                .clicked()
                            {
                                w.priority += 1;
                            }
                        });
                    }
                    _ => {
                        ui.checkbox(&mut w.launch, "Launch main design rules dialog");
                        ui.add_space(10.0);
                        ui.label(name(&w.kind));
                        ui.label(widgets::attributes(&rules::default_rule(&w.kind)));
                        ui.add_space(10.0);
                        ui.label(expression(w));
                    }
                });
            }
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        w.step == 4,
                        egui::Button::new("Finish").min_size(vec2(75.0, 25.0)),
                    )
                    .clicked()
                {
                    complete = true;
                }
                if ui
                    .add_enabled(
                        w.step < 4
                            && rules::supported(&w.kind)
                            && (w.step != 1 || valid_name)
                            && (w.step != 2 || valid_scope),
                        egui::Button::new("Next").min_size(vec2(75.0, 25.0)),
                    )
                    .clicked()
                {
                    w.step += 1;
                }
                if ui
                    .add_enabled(
                        w.step > 0,
                        egui::Button::new("Back").min_size(vec2(75.0, 25.0)),
                    )
                    .clicked()
                {
                    w.step -= 1;
                }
                if ui
                    .add_sized([75.0, 25.0], egui::Button::new("Cancel"))
                    .clicked()
                {
                    close = true;
                }
            });
        });
    if complete {
        finish(d, w);
        close = true;
    }
    close
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wizard_actions_stay_below_every_page_at_the_reference_window_size() {
        let mut model = Document::new(DocumentKind::Board, "test");
        model.pcb = Some(Board {
            rules: vec![rules::default_rule("Width")],
            ..Default::default()
        });
        let mut doc = OpenDocument::new(model, None);
        open(&mut doc);
        let mut draft = doc.pcb_editor.rules.editor.take().unwrap();
        for step in 0..=4 {
            let ctx = egui::Context::default();
            let mut wizard = Wizard {
                step,
                ..Default::default()
            };
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1707.0, 1019.0))),
                ..Default::default()
            };
            let _ = ctx.run(input.clone(), |ctx| {
                show(
                    ctx,
                    doc.model.pcb.as_ref().unwrap(),
                    &mut draft,
                    &mut wizard,
                );
            });
            let output = ctx.run(input, |ctx| {
                show(
                    ctx,
                    doc.model.pcb.as_ref().unwrap(),
                    &mut draft,
                    &mut wizard,
                );
            });
            let rect = ctx
                .memory(|m| m.area_rect(egui::Id::new("new_rule_wizard_v3")))
                .unwrap();
            assert!(
                rect.width() <= 665.0 && rect.height() <= 520.0,
                "{step}: {rect:?}"
            );
            let finish = output
                .shapes
                .iter()
                .find_map(|s| {
                    if let egui::Shape::Text(t) = &s.shape {
                        (t.galley.text() == "Finish").then_some(t.pos)
                    } else {
                        None
                    }
                })
                .unwrap();
            assert!(
                finish.y > rect.bottom() - 40.0,
                "Page {step}: Finish {finish:?}, window {rect:?}"
            );
        }
    }
    #[test]
    fn wizard_finish_inserts_at_requested_priority_and_cancel_draft_is_isolated() {
        let mut model = Document::new(DocumentKind::Board, "test");
        model.pcb = Some(Board {
            rules: vec![rules::default_rule("Width")],
            ..Default::default()
        });
        let mut doc = OpenDocument::new(model, None);
        open(&mut doc);
        let mut d = doc.pcb_editor.rules.editor.take().unwrap();
        let before = d.rules.clone();
        let w = Wizard {
            kind: "Width".into(),
            name: "Scoped Width".into(),
            scope: 1,
            nets: BTreeSet::from(["A'B".into()]),
            priority: 1,
            ..Default::default()
        };
        assert_eq!(d.rules, before);
        finish(&mut d, &w);
        assert_eq!(text(&d.rules[0], "PRIORITY"), "1");
        assert_eq!(text(&d.rules[d.selected], "PRIORITY"), "2");
        assert_eq!(
            text(&d.rules[d.selected], "SCOPE1EXPRESSION"),
            "(InNet('A''B'))"
        );
        assert_eq!(doc.history.revision, 0);
    }
}
