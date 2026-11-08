//! AD 24.4.1 parameter matrix: components are rows, parameters are columns.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Matrix {
    selected: BTreeSet<(usize, String)>,
    anchor: Option<(usize, String)>,
    show_all: bool,
}
impl Default for Matrix {
    fn default() -> Self {
        Self {
            selected: BTreeSet::new(),
            anchor: None,
            show_all: true,
        }
    }
}
struct Cell {
    name: String,
    sheet: Option<String>,
    library: Option<String>,
    changed: bool,
    editable: bool,
    accepted: bool,
}
struct Part {
    row: usize,
    document: String,
    designator: String,
    cells: Vec<Cell>,
}
const SYSTEM: [&str; 4] = [
    "Comment",
    "Component Kind",
    "Description",
    "Library Reference",
];
fn value(c: &eda_core::schematic::Component, name: &str) -> Option<String> {
    match name {
        "Description" => Some(c.description.clone()),
        "Component Kind" => Some(c.settings.kind.to_string()),
        "Library Reference" => Some(c.library_reference.clone()),
        _ => c
            .parameters
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone()),
    }
}
fn parts(d: &Dialog) -> (Vec<String>, Vec<Part>) {
    let mut names = BTreeMap::new();
    for r in &d.rows {
        let s = &d.sheets[r.sheet];
        let Some(c) = s
            .model
            .schematic
            .as_ref()
            .and_then(|s| s.components.iter().find(|c| c.id == r.component))
        else {
            continue;
        };
        let Ok(f) = &r.definition else { continue };
        for name in c.parameters.keys().chain(f.components[0].parameters.keys()) {
            if !name.eq_ignore_ascii_case("Designator")
                && !SYSTEM.iter().any(|n| n.eq_ignore_ascii_case(name))
            {
                names
                    .entry(name.to_lowercase())
                    .or_insert_with(|| name.clone());
            }
        }
    }
    let names: Vec<_> = SYSTEM
        .iter()
        .map(|s| (*s).to_owned())
        .chain(names.into_values())
        .collect();
    let mut parts = vec![];
    for (index, r) in d.rows.iter().enumerate() {
        if !d.sheets[r.sheet].enabled || !r.enabled {
            continue;
        }
        let Ok(f) = &r.definition else { continue };
        let Some(c) = d.sheets[r.sheet]
            .model
            .schematic
            .as_ref()
            .and_then(|s| s.components.iter().find(|c| c.id == r.component))
        else {
            continue;
        };
        let changes = update::parameter_changes(c, f).unwrap_or_default();
        let cells = names
            .iter()
            .map(|name| {
                let sheet = value(c, name);
                let library = value(&f.components[0], name);
                let changed = changes.iter().any(|c| c.name.eq_ignore_ascii_case(name));
                let editable = changed
                    && !r.full
                    && r.options.parameters
                    && !r.protected.iter().any(|n| n.eq_ignore_ascii_case(name))
                    && (sheet.is_some() || r.options.add_parameters)
                    && (library.is_some() || r.options.remove_parameters);
                Cell {
                    name: name.clone(),
                    sheet,
                    library,
                    changed,
                    editable,
                    accepted: r.options.updates(name),
                }
            })
            .collect::<Vec<_>>();
        if d.parameter_matrix.show_all || cells.iter().any(|c| c.changed) {
            parts.push(Part {
                row: index,
                document: d.sheets[r.sheet].model.name.clone(),
                designator: r.designator.clone(),
                cells,
            });
        }
    }
    (names, parts)
}
fn select(
    matrix: &mut Matrix,
    row: usize,
    column: &str,
    rows: &[Part],
    columns: &[String],
    modifiers: egui::Modifiers,
) {
    let key = (row, column.to_lowercase());
    if modifiers.shift
        && let Some((anchor_row, anchor_column)) = &matrix.anchor
        && let (Some(a), Some(b), Some(x), Some(y)) = (
            rows.iter().position(|r| r.row == *anchor_row),
            rows.iter().position(|r| r.row == row),
            columns
                .iter()
                .position(|c| c.eq_ignore_ascii_case(anchor_column)),
            columns.iter().position(|c| c.eq_ignore_ascii_case(column)),
        )
    {
        if !modifiers.command {
            matrix.selected.clear();
        }
        for part in &rows[a.min(b)..=a.max(b)] {
            for name in &columns[x.min(y)..=x.max(y)] {
                matrix.selected.insert((part.row, name.to_lowercase()));
            }
        }
    } else {
        if !modifiers.command {
            matrix.selected.clear();
        }
        if !matrix.selected.remove(&key) {
            matrix.selected.insert(key.clone());
        }
        matrix.anchor = Some(key);
    }
}
fn apply(d: &mut Dialog, parts: &[Part], selected_only: bool, accept: bool) {
    for part in parts {
        for cell in &part.cells {
            if !cell.editable
                || selected_only
                    && !d
                        .parameter_matrix
                        .selected
                        .contains(&(part.row, cell.name.to_lowercase()))
            {
                continue;
            }
            let excluded = &mut d.rows[part.row].options.excluded_parameters;
            excluded.retain(|n| !n.eq_ignore_ascii_case(&cell.name));
            if !accept {
                excluded.insert(cell.name.clone());
            }
        }
    }
    d.validated = false;
    d.error.clear();
}
fn text_cell(ui: &mut egui::Ui, text: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 20.],
        egui::Label::new(text).truncate().halign(egui::Align::Min),
    )
    .on_hover_text(text)
}
fn parameter_cell(ui: &mut egui::Ui, text: &str, color: Color32, selected: bool) -> egui::Response {
    egui::Frame::NONE
        .fill(if selected {
            ui.visuals().selection.bg_fill
        } else {
            Color32::TRANSPARENT
        })
        .show(ui, |ui| {
            ui.add_sized(
                [60., 20.],
                egui::Label::new(egui::RichText::new(text).color(color))
                    .truncate()
                    .halign(egui::Align::Min)
                    .sense(egui::Sense::click()),
            )
        })
        .inner
}
fn display(cell: &Cell) -> String {
    let value = if cell.accepted {
        &cell.library
    } else {
        &cell.sheet
    };
    let text = value.as_deref().unwrap_or("-");
    if cell.name == "Component Kind" {
        text.parse()
            .map(crate::app::library_editor::component_type)
            .unwrap_or("Unknown")
            .to_owned()
    } else {
        text.to_owned()
    }
}
pub(super) fn show(ctx: &egui::Context, d: &mut Dialog) {
    let (columns, rows) = parts(d);
    let mut action = None;
    egui::Modal::new(egui::Id::new("library_update_parameter_changes"))
        .backdrop_color(Color32::TRANSPARENT)
        .frame(egui::Frame::window(&ctx.style()).corner_radius(0))
        .show(ctx, |ui| {
            ui.set_width((ctx.screen_rect().width() - 48.).clamp(600., 934.));
            ui.horizontal(|ui| {
                ui.strong("Select Parameter Changes");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("×").clicked() {
                        d.parameters = false;
                    }
                });
            });
            ui.add_space(8.);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt("library_parameter_matrix")
                    .min_scrolled_height(446.)
                    .max_height(446.)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0., 0.);
                        ui.horizontal(|ui| {
                            let selected = d
                                .parameter_matrix
                                .selected
                                .iter()
                                .map(|(row, _)| row)
                                .collect::<BTreeSet<_>>()
                                .len();
                            text_cell(
                                ui,
                                &format!(
                                    "{} Objects - {}",
                                    rows.len(),
                                    if selected == 0 {
                                        "None Selected".into()
                                    } else {
                                        format!("{selected} Selected")
                                    }
                                ),
                                338.,
                            );
                            text_cell(ui, "System Parameters", 240.);
                            text_cell(
                                ui,
                                "User Parameters",
                                (columns.len().saturating_sub(4) as f32 * 60.).max(120.),
                            );
                        });
                        egui::Grid::new("library_parameter_matrix_cells")
                            .striped(true)
                            .min_col_width(0.)
                            .spacing([0., 0.])
                            .show(ui, |ui| {
                                for (text, width) in [
                                    ("Object Type", 100.),
                                    ("Document", 138.),
                                    ("Identifier", 100.),
                                ] {
                                    text_cell(ui, text, width);
                                }
                                for name in &columns {
                                    text_cell(ui, name, 60.);
                                }
                                ui.end_row();
                                for part in &rows {
                                    text_cell(ui, "Part", 100.);
                                    text_cell(ui, &part.document, 138.);
                                    text_cell(ui, &part.designator, 100.);
                                    for cell in &part.cells {
                                        let key = (part.row, cell.name.to_lowercase());
                                        let selected = d.parameter_matrix.selected.contains(&key);
                                        let text = display(cell);
                                        let color = if cell.changed
                                            && cell.accepted
                                            && cell.sheet.is_none()
                                        {
                                            Color32::LIGHT_GREEN
                                        } else if cell.changed
                                            && cell.accepted
                                            && cell.library.is_none()
                                        {
                                            Color32::LIGHT_RED
                                        } else {
                                            ui.visuals().text_color()
                                        };
                                        let response = parameter_cell(ui, &text, color, selected)
                                            .on_hover_text(format!(
                                                "{}\nSheet: {}\nLibrary: {}{}",
                                                cell.name,
                                                cell.sheet.as_deref().unwrap_or("-"),
                                                cell.library.as_deref().unwrap_or("-"),
                                                if cell.changed && !cell.editable {
                                                    "\nProtected or excluded by update settings"
                                                } else {
                                                    ""
                                                }
                                            ));
                                        if cell.changed && cell.accepted {
                                            let p = response.rect.right_top();
                                            ui.painter().add(egui::Shape::convex_polygon(
                                                vec![
                                                    p - egui::vec2(7., 0.),
                                                    p,
                                                    p + egui::vec2(0., 7.),
                                                ],
                                                Color32::from_rgb(80, 140, 200),
                                                egui::Stroke::NONE,
                                            ));
                                        }
                                        if response.clicked() {
                                            select(
                                                &mut d.parameter_matrix,
                                                part.row,
                                                &cell.name,
                                                &rows,
                                                &columns,
                                                ui.input(|i| i.modifiers),
                                            );
                                        }
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            });
            ui.horizontal(|ui| {
                for (selected_only, accept, label) in [
                    (true, true, "Update Selected"),
                    (true, false, "Reject Selected"),
                    (false, true, "Update All"),
                    (false, false, "Reject All"),
                ] {
                    if !selected_only && accept {
                        ui.add_space(14.);
                    }
                    let enabled = rows.iter().any(|part| {
                        part.cells.iter().any(|cell| {
                            cell.editable
                                && cell.accepted != accept
                                && (!selected_only
                                    || d.parameter_matrix
                                        .selected
                                        .contains(&(part.row, cell.name.to_lowercase())))
                        })
                    });
                    if ui
                        .add_enabled(
                            enabled,
                            egui::Button::new(label).min_size(egui::vec2(96., 23.)),
                        )
                        .clicked()
                    {
                        action = Some((selected_only, accept));
                    }
                }
                ui.checkbox(&mut d.parameter_matrix.show_all, "Show all parts");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_sized([75., 23.], egui::Button::new("OK")).clicked() {
                        d.parameters = false;
                    }
                });
            });
        });
    if let Some((selected_only, accept)) = action {
        apply(d, &rows, selected_only, accept);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matrix_rejection_applies_to_selected_instance_and_keeps_protected_values() {
        let (mut docs, mut d) = super::super::tests::fixture();
        for r in &mut d.rows {
            r.full = false;
        }
        d.parameter_matrix
            .selected
            .insert((1, "description".into()));
        let (_, rows) = parts(&d);
        apply(&mut d, &rows, true, false);
        assert!(d.rows[0].options.updates("Description"));
        assert!(!d.rows[1].options.updates("Description"));
        d.rows[1].protected.insert("Description".into());
        let (_, rows) = parts(&d);
        apply(&mut d, &rows, false, true);
        assert!(!d.rows[1].options.updates("Description"));
        d.validated = true;
        d.execute(&mut docs).unwrap();
        assert_eq!(
            docs[0].model.schematic.as_ref().unwrap().components[0].description,
            "Updated"
        );
        assert!(
            docs[1].model.schematic.as_ref().unwrap().components[0]
                .description
                .is_empty()
        );
    }
}
