//! Query authoring drafts. Cancel never writes into a rule; accepted queries
//! are checked by the same GUI-independent evaluator that routing/DRC use.
use super::*;
use egui::{Rect, Stroke, UiBuilder, pos2, vec2};

const PREDICATES: &[(&str, &str)] = &[
    ("Belongs to Net", "InNet"),
    ("Belongs to Component", "InComponent"),
    ("Associated with Footprint", "HasFootprint"),
    ("Belongs to Component Class", "InComponentClass"),
    ("Belongs to Net Class", "InNetClass"),
    ("Belongs to Pad Class", "InPadClass"),
    ("Exists on Layer", "OnLayer"),
    ("Belongs to Polygon", "InNamedPolygon"),
    ("Object Kind is", "ObjectKind"),
];
const OBJECTS: &[&str] = &[
    "All",
    "IsTrack",
    "IsArc",
    "IsPad",
    "IsVia",
    "IsFill",
    "IsRegion",
    "IsPolygon",
    "IsThruPad",
    "IsSMDPad",
    "OnCopper",
    "OnTop",
    "OnBottom",
];

pub(super) struct QueryDraft {
    pub key: String,
    pub expression: String,
    builder: bool,
    terms: Vec<(String, String)>,
    selected: usize,
    category: String,
    mask: String,
    message: String,
}
impl QueryDraft {
    pub(super) fn new(key: &str, expression: &str, builder: bool) -> Self {
        Self {
            key: key.into(),
            expression: expression.into(),
            builder,
            terms: vec![],
            selected: 0,
            category: "PCB Functions".into(),
            mask: String::new(),
            message: String::new(),
        }
    }
    fn preview(&self) -> String {
        if !self.builder {
            return self.expression.clone();
        }
        if self.terms.is_empty() {
            return "All".into();
        }
        self.terms
            .iter()
            .map(|(function, value)| {
                if function == "ObjectKind" {
                    value.clone()
                } else {
                    format!("{function}('{}')", value.replace('\'', "''"))
                }
            })
            .collect::<Vec<_>>()
            .join(" And ")
    }
}
pub(super) fn values(board: &Board, function: &str) -> Vec<String> {
    let mut values: Vec<String> = match function {
        "InNet" => board.nets.iter().map(|n| n.name.clone()).collect(),
        "InComponent" => board
            .components
            .iter()
            .map(|c| c.designator.clone())
            .collect(),
        "HasFootprint" => board
            .components
            .iter()
            .map(|c| c.footprint.clone())
            .collect(),
        "OnLayer" => board.layers.iter().map(|l| l.name.clone()).collect(),
        "InNamedPolygon" => board.polygon_pours.iter().map(|p| p.name.clone()).collect(),
        "ObjectKind" => OBJECTS.iter().map(|s| (*s).into()).collect(),
        other @ ("InNetClass" | "InComponentClass" | "InPadClass") => {
            let kind = match other {
                "InNetClass" => "0",
                "InComponentClass" => "1",
                _ => "3",
            };
            board
                .classes
                .iter()
                .filter(|c| text(c, "KIND") == kind)
                .map(|c| text(c, "NAME").into())
                .collect()
        }
        _ => vec![],
    };
    values.sort();
    values.dedup();
    values
}
pub(super) fn check(board: &Board, expression: &str) -> Result<usize, String> {
    // Validate even on empty boards (an empty iterator is not a syntax check).
    let sample = Item::new(
        pcb::TOP,
        None,
        Shape::Track {
            start: eda_geom::Point::default(),
            end: eda_geom::Point::mm(1, 0),
            width: 100_000,
        },
    );
    rules::query(board, &sample, expression)?;
    board.items.iter().try_fold(0, |count, i| {
        rules::query(board, i, expression).map(|matched| count + usize::from(matched))
    })
}
fn builder(ui: &mut egui::Ui, q: &mut QueryDraft, board: &Board) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("query_levels")
            .width(120.0)
            .selected_text("Show All Levels")
            .show_ui(ui, |ui| {
                ui.label("Show All Levels");
            });
        ui.add_space(12.0);
        for label in ["◀", "▶"] {
            ui.add_enabled(false, egui::Button::new(label));
        }
        for (label, delta) in [("▲", -1isize), ("▼", 1)] {
            let target = q.selected as isize + delta;
            let enabled = !q.terms.is_empty() && (0..q.terms.len() as isize).contains(&target);
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                q.terms.swap(q.selected, target as usize);
                q.selected = target as usize;
            }
        }
        if ui.button("＋").on_hover_text("Add condition").clicked() {
            q.terms.push((
                "InNet".into(),
                values(board, "InNet").first().cloned().unwrap_or_default(),
            ));
            q.selected = q.terms.len() - 1;
        }
        if ui
            .add_enabled(!q.terms.is_empty(), egui::Button::new("−"))
            .on_hover_text("Remove condition")
            .clicked()
        {
            q.terms.remove(q.selected);
            q.selected = q.selected.saturating_sub(1);
        }
        if ui.button("×").on_hover_text("Clear conditions").clicked() {
            q.terms.clear();
            q.selected = 0;
        }
    });
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, 316.0), egui::Sense::hover());
    let left = Rect::from_min_size(rect.min, vec2(310.0, rect.height()));
    let right = Rect::from_min_max(pos2(left.right() + 4.0, rect.top()), rect.max);
    for r in [left, right] {
        ui.painter().rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, widgets::LINE),
            egui::StrokeKind::Inside,
        );
    }
    widgets::child(ui, UiBuilder::new().max_rect(left), |ui| {
        widgets::table_header(
            ui,
            &[155.0, 155.0],
            &["Condition Type / Operator", "Condition Value"],
        );
        egui::ScrollArea::vertical()
            .id_salt("builder_conditions")
            .max_height(290.0)
            .show(ui, |ui| {
                for i in 0..=q.terms.len() {
                    ui.push_id(i, |ui| {
                        ui.horizontal(|ui| {
                            let label = q
                                .terms
                                .get(i)
                                .and_then(|(function, _)| {
                                    PREDICATES.iter().find(|(_, f)| f == function)
                                })
                                .map_or("Add first condition…", |(label, _)| *label);
                            egui::ComboBox::from_id_salt("condition")
                                .width(148.0)
                                .selected_text(label)
                                .show_ui(ui, |ui| {
                                    for (label, function) in PREDICATES {
                                        if ui.selectable_label(false, *label).clicked() {
                                            let v = values(board, function)
                                                .first()
                                                .cloned()
                                                .unwrap_or_default();
                                            if i == q.terms.len() {
                                                q.terms.push(((*function).into(), v));
                                            } else {
                                                q.terms[i] = ((*function).into(), v);
                                            }
                                            q.selected = i;
                                        }
                                    }
                                    ui.add_enabled(
                                        false,
                                        egui::Button::new("Belongs to Differential Pair Class"),
                                    );
                                });
                            if let Some((function, value)) = q.terms.get_mut(i) {
                                egui::ComboBox::from_id_salt("condition_value")
                                    .width(145.0)
                                    .selected_text(value.as_str())
                                    .show_ui(ui, |ui| {
                                        for option in values(board, function) {
                                            ui.selectable_value(value, option.clone(), option);
                                        }
                                    });
                            }
                        });
                    });
                }
            });
    });
    widgets::child(ui, UiBuilder::new().max_rect(right), |ui| {
        widgets::section(ui, "Query Preview");
        ui.add(egui::Label::new(q.preview()).wrap());
    });
}
fn insert_at_cursor(ctx: &egui::Context, id: egui::Id, expression: &mut String, inserted: &str) {
    let mut state = egui::text_edit::TextEditState::load(ctx, id).unwrap_or_default();
    let end = expression.chars().count();
    let range = state
        .cursor
        .char_range()
        .map_or(end..end, |r| r.as_sorted_char_range());
    let byte_index = |n| {
        expression
            .char_indices()
            .nth(n)
            .map_or(expression.len(), |(i, _)| i)
    };
    expression.replace_range(byte_index(range.start)..byte_index(range.end), inserted);
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(range.start + inserted.chars().count()),
        )));
    state.store(ctx, id);
    ctx.memory_mut(|m| m.request_focus(id));
}
fn helper(ui: &mut egui::Ui, q: &mut QueryDraft, board: &Board) {
    ui.label("Query");
    let editor = ui.add_sized(
        [ui.available_width(), 180.0],
        egui::TextEdit::multiline(&mut q.expression)
            .id_salt("helper_expression")
            .font(egui::TextStyle::Monospace),
    );
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for label in [
            "+", "-", "*", "/", "Div", "Mod", "Not", "And", "Or", "Xor", "<", "<=", ">=", ">",
            "<>", "=", "Like", "''",
        ] {
            let supported = matches!(label, "Not" | "And" | "Or" | "''");
            if ui
                .add_enabled(
                    supported,
                    egui::Button::new(label).min_size(vec2(30.0, 20.0)),
                )
                .clicked()
            {
                insert_at_cursor(
                    ui.ctx(),
                    editor.id,
                    &mut q.expression,
                    &format!(" {label} "),
                );
            }
        }
    });
    ui.horizontal_top(|ui| {
        egui::Frame::new()
            .fill(Color32::from_gray(58))
            .stroke(Stroke::new(1.0, widgets::LINE))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_width(162.0);
                    widgets::section(ui, "Categories");
                    egui::ScrollArea::vertical()
                        .id_salt("query_categories")
                        .max_height(182.0)
                        .show(ui, |ui| {
                            for label in [
                                "PCB Functions",
                                "Object Type Checks",
                                "Fields",
                                "Membership Checks",
                                "Attribute Checks",
                                "Parameters",
                                "Layer Checks",
                                "PCB Object Lists",
                                "Nets",
                                "Polygons",
                                "Pads",
                                "Components",
                                "Layers",
                                "Net Classes",
                                "Component Classes",
                                "Pad Classes",
                            ] {
                                let available = !matches!(
                                    label,
                                    "Fields" | "Attribute Checks" | "Parameters" | "Pads"
                                );
                                ui.add_enabled_ui(available, |ui| {
                                    ui.selectable_value(&mut q.category, label.into(), label);
                                });
                            }
                        });
                });
            });
        ui.vertical(|ui| {
            ui.set_width(420.0);
            widgets::table_header(ui, &[145.0, 275.0], &["Name", "Description"]);
            egui::Frame::new()
                .fill(Color32::from_gray(58))
                .stroke(Stroke::new(1.0, widgets::LINE))
                .show(ui, |ui| {
                    ui.set_min_size(vec2(420.0, 151.0));
                    let candidates: Vec<(String, String)> = match q.category.as_str() {
                        "PCB Functions" => OBJECTS
                            .iter()
                            .map(|s| ((*s).into(), format!("Test {s}")))
                            .chain(
                                PREDICATES
                                    .iter()
                                    .filter(|(_, f)| *f != "ObjectKind")
                                    .map(|(label, f)| (format!("{f}('')"), (*label).into())),
                            )
                            .collect(),
                        "Object Type Checks" => OBJECTS
                            .iter()
                            .map(|s| ((*s).into(), format!("Test {s}")))
                            .collect(),
                        "Layer Checks" => ["OnCopper", "OnTop", "OnBottom"]
                            .iter()
                            .map(|s| ((*s).into(), "Layer check".into()))
                            .collect(),
                        "Membership Checks" => PREDICATES
                            .iter()
                            .filter(|(_, f)| *f != "ObjectKind")
                            .map(|(label, f)| (format!("{f}('')"), (*label).into()))
                            .collect(),
                        cat => {
                            let function = match cat {
                                "Nets" => "InNet",
                                "Polygons" => "InNamedPolygon",
                                "Components" => "InComponent",
                                "Layers" => "OnLayer",
                                "Net Classes" => "InNetClass",
                                "Component Classes" => "InComponentClass",
                                "Pad Classes" => "InPadClass",
                                _ => "",
                            };
                            values(board, function)
                                .into_iter()
                                .map(|s| (format!("{function}('{}')", s.replace('\'', "''")), s))
                                .collect()
                        }
                    };
                    egui::ScrollArea::vertical()
                        .id_salt("query_keywords")
                        .max_height(150.0)
                        .show(ui, |ui| {
                            for (value, description) in candidates {
                                if !q.mask.is_empty()
                                    && !value.to_lowercase().contains(&q.mask.to_lowercase())
                                {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_sized(
                                            [143.0, 20.0],
                                            egui::Button::selectable(false, &value),
                                        )
                                        .double_clicked()
                                    {
                                        insert_at_cursor(
                                            ui.ctx(),
                                            editor.id,
                                            &mut q.expression,
                                            &value,
                                        );
                                    }
                                    ui.add(egui::Label::new(description).truncate());
                                });
                            }
                        });
                });
            ui.horizontal(|ui| {
                ui.label("Mask");
                ui.add(egui::TextEdit::singleline(&mut q.mask).desired_width(ui.available_width()));
            });
        });
    });
}
/// Returns true when the nested dialog is dismissed.
pub(super) fn show(
    ctx: &egui::Context,
    board: &Board,
    q: &mut QueryDraft,
    rule: &mut Fields,
) -> bool {
    let mut close = q.message.is_empty()
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    let mut accept = false;
    let size = if q.builder {
        vec2(576.0, 417.0)
    } else {
        vec2(618.0, 510.0)
    };
    let title = if q.builder {
        "Building Query from Board"
    } else {
        "Query Helper"
    };
    egui::Window::new(title)
        .id(egui::Id::new(("rules_query_draft", q.builder)))
        .title_bar(false)
        .fixed_size(size)
        .resizable(false)
        .collapsible(false)
        .interactable(q.message.is_empty())
        .movable(false)
        .current_pos(widgets::position(
            ctx,
            egui::Id::new(("rules_query_draft", q.builder)),
            ctx.screen_rect().center() - (size + vec2(14.0, 14.0)) * 0.5,
        ))
        .frame(
            egui::Frame::new()
                .fill(Color32::from_gray(76))
                .inner_margin(7.0)
                .stroke(Stroke::new(1.0, widgets::LINE)),
        )
        .show(ctx, |ui| {
            widgets::style(ui);
            close |= widgets::title(ui, title);
            egui::Frame::new()
                .fill(widgets::BG)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    if q.builder {
                        builder(ui, q, board);
                    } else {
                        helper(ui, q, board);
                    }
                });
            ui.horizontal(|ui| {
                if !q.builder && ui.button("Check Syntax").clicked() {
                    q.message = match check(board, &q.expression) {
                        Ok(n) => format!("Syntax is valid. {n} objects match."),
                        Err(e) => e,
                    };
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized([75.0, 25.0], egui::Button::new("Cancel"))
                        .clicked()
                    {
                        close = true;
                    }
                    if ui
                        .add_sized([75.0, 25.0], egui::Button::new("OK"))
                        .clicked()
                    {
                        accept = true;
                    }
                });
            });
        });
    if accept {
        let expression = q.preview();
        match check(board, &expression) {
            Ok(_) => {
                rule.insert(q.key.clone(), expression);
                close = true;
            }
            Err(e) => q.message = e,
        }
    }
    widgets::message(ctx, "query_syntax_message", &mut q.message);
    close
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helper_insertion_replaces_the_selection_and_uses_character_indices() {
        let ctx = egui::Context::default();
        let id = egui::Id::new("query");
        let mut state = egui::text_edit::TextEditState::default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(7),
                egui::text::CCursor::new(10),
            )));
        state.store(&ctx, id);
        let mut expression = "InNet('СЕТЬ')".to_owned();
        insert_at_cursor(&ctx, id, &mut expression, "GND");
        assert_eq!(expression, "InNet('GNDЬ')");
        insert_at_cursor(&ctx, id, &mut expression, "_1");
        assert_eq!(expression, "InNet('GND_1Ь')");
    }
    #[test]
    fn both_query_dialogs_fit_the_observed_reference_bounds_and_cancel_is_read_only() {
        for builder in [false, true] {
            let ctx = egui::Context::default();
            let board = Board::default();
            let mut q = QueryDraft::new("SCOPE1EXPRESSION", "IsPad", builder);
            let mut rule = rules::default_rule("Width");
            let before = rule.clone();
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1707.0, 1019.0))),
                ..Default::default()
            };
            for _ in 0..2 {
                let _ = ctx.run(input.clone(), |ctx| {
                    show(ctx, &board, &mut q, &mut rule);
                });
            }
            let rect = ctx
                .memory(|m| m.area_rect(egui::Id::new(("rules_query_draft", builder))))
                .unwrap();
            assert!(
                rect.width() <= 650.0 && rect.height() <= 560.0,
                "{builder}: {rect:?}"
            );
            assert_eq!(rule, before);
            let input = egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..input
            };
            let _ = ctx.run(input, |ctx| {
                assert!(show(ctx, &board, &mut q, &mut rule));
            });
            assert_eq!(rule, before);
        }
    }
    #[test]
    fn builder_quotes_board_names_and_validates_empty_boards() {
        let mut q = QueryDraft::new("SCOPE1EXPRESSION", "UnknownPredicate", true);
        q.terms = vec![
            ("InNet".into(), "A'B".into()),
            ("ObjectKind".into(), "IsTrack".into()),
        ];
        assert_eq!(q.preview(), "InNet('A''B') And IsTrack");
        assert_eq!(check(&Board::default(), &q.preview()), Ok(0));
        assert!(check(&Board::default(), "UnknownPredicate").is_err());
    }
}
