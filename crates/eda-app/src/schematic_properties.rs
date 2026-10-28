//! Contextual Properties for the schematic editor; AD 24.4.1 is the visual reference.
use super::*;
use eda_core::schematic::{
    Component, SchFont, SchPrimitive, SchPrimitiveKind as K, Schematic, Sheet, TextRole,
};
use eda_core::schematic_edit::{self as edit, Edit};
use eda_geom::Point;
use egui::{Align, Sense, Stroke};

#[derive(Default)]
pub(super) struct Panel {
    document_parameters: bool,
    component_pins: bool,
    port_parameters: bool,
    filter: usize,
    selected_parameter: Option<String>,
    draft: Option<Draft>,
    parameter: Option<ParameterDialog>,
    font_dialog: bool,
}
struct ParameterDialog {
    owner: Option<Uuid>,
    original: Option<String>,
    name: String,
    value: String,
}
struct Draft {
    session: Uuid,
    revision: u64,
    selected: Option<Uuid>,
    component: Option<Component>,
    primitive: Option<SchPrimitive>,
    related: Vec<SchPrimitive>,
    sheet: Sheet,
    size: Point,
    font: SchFont,
    sheet_font: SchFont,
}
impl Panel {
    pub(super) fn close_document(&mut self, session: Uuid) {
        if self
            .draft
            .as_ref()
            .is_some_and(|draft| draft.session == session)
        {
            self.draft = None;
            self.parameter = None;
            self.selected_parameter = None;
            self.font_dialog = false;
        }
    }
}
fn font_index(p: &SchPrimitive) -> Option<usize> {
    match p.kind {
        K::Text { font, .. }
        | K::TextFrame { font, .. }
        | K::Pin { font, .. }
        | K::SheetEntry { font, .. }
        | K::Port { font, .. }
        | K::PowerPort { font, .. } => Some(font),
        _ => None,
    }
}
fn default_font() -> SchFont {
    SchFont {
        name: "Arial".into(),
        height_nm: 2_540_000,
        bold: false,
        italic: false,
        underline: false,
    }
}
impl Draft {
    fn new(doc: &OpenDocument) -> Self {
        let s = doc.model.schematic.as_ref().unwrap();
        let selected = doc.view.selected;
        let component = s
            .components
            .iter()
            .find(|c| Some(c.id) == selected)
            .cloned();
        let primitive = selected.and_then(|id| s.primitive(id)).cloned();
        let related = s
            .primitives
            .iter()
            .chain(&s.parameter_objects)
            .filter(|p| {
                component
                    .as_ref()
                    .is_some_and(|c| p.component == Some(c.id))
                    || primitive.as_ref().is_some_and(|owner| {
                        matches!(owner.kind, K::SheetSymbol { .. })
                            && p.presentation.parent == Some(owner.id)
                            && matches!(p.kind, K::Text { .. })
                    })
            })
            .cloned()
            .collect();
        let font = primitive
            .as_ref()
            .and_then(font_index)
            .and_then(|i| s.fonts.get(i))
            .cloned()
            .unwrap_or_else(default_font);
        Self {
            session: doc.session_id,
            revision: doc.history.revision,
            selected,
            component,
            primitive,
            related,
            sheet: s.sheet.clone(),
            size: doc.model.size,
            font,
            sheet_font: s
                .fonts
                .get(s.sheet.system_font)
                .cloned()
                .unwrap_or_else(default_font),
        }
    }
    fn commands(&self, doc: &OpenDocument) -> Vec<Edit> {
        let s = doc.model.schematic.as_ref().unwrap();
        let mut edits = vec![];
        if self.sheet != s.sheet || self.size != doc.model.size {
            edits.push(Edit::SheetProperties {
                sheet: Box::new(self.sheet.clone()),
                size: self.size,
            });
        }
        if s.fonts.get(s.sheet.system_font) != Some(&self.sheet_font) {
            edits.push(Edit::SheetFont(self.sheet_font.clone()));
        }
        if let Some(c) = &self.component {
            let old = s.components.iter().find(|v| v.id == c.id).unwrap();
            let mut settings = c.settings.clone();
            settings.rotation = old.settings.rotation;
            if settings != old.settings || c.description != old.description {
                edits.push(Edit::ComponentDetails {
                    id: c.id,
                    description: c.description.clone(),
                    settings,
                });
            }
            // Presentation changes precede the transform so parameter geometry rotates as well.
        }
        for p in self.primitive.iter().chain(&self.related) {
            if s.primitive(p.id) != Some(p) {
                edits.push(Edit::PrimitiveProperties {
                    id: p.id,
                    value: Box::new(p.clone()),
                });
            }
        }
        if let Some(c) = &self.component {
            let old = s.components.iter().find(|v| v.id == c.id).unwrap();
            let raw = |c: &Component| {
                c.parameters
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case("Comment"))
                    .map(|(_, value)| value)
                    .cloned()
                    .unwrap_or_else(|| c.comment.clone())
            };
            if c.designator != old.designator || raw(c) != raw(old) {
                edits.push(Edit::ComponentProperties {
                    id: c.id,
                    designator: c.designator.clone(),
                    comment: raw(c),
                });
            }
            if c.position != old.position || c.settings.rotation != old.settings.rotation {
                edits.push(Edit::Transform {
                    id: c.id,
                    pivot: old.position,
                    quarter_turns: (c.settings.rotation + 4 - old.settings.rotation) % 4,
                    delta: Point {
                        x: c.position.x - old.position.x,
                        y: c.position.y - old.position.y,
                    },
                });
            }
        }
        if let Some(p) = &self.primitive
            && font_index(p).is_some_and(|i| s.fonts.get(i) != Some(&self.font))
        {
            edits.push(Edit::Font {
                id: p.id,
                font: self.font.clone(),
            });
        }
        edits
    }
}
fn caption(p: &SchPrimitive) -> &'static str {
    match p.text_role {
        Some(TextRole::Designator) => return "Designator",
        Some(TextRole::Comment) => return "Parameter",
        Some(TextRole::NetLabel) => return "Net Label",
        _ => {}
    }
    match &p.kind {
        K::Text { .. } if p.presentation.record_type == 41 || !p.presentation.name.is_empty() => {
            "Parameter"
        }
        K::Text { .. } => "Text",
        K::TextFrame { .. } => "Text Frame",
        K::Pin { .. } => "Pin",
        K::PowerPort { .. } => "Power Port",
        K::SheetSymbol { .. } => "Sheet Symbol",
        K::SheetEntry { .. } => "Sheet Entry",
        K::Port { .. } => "Port",
        K::Polyline {
            electrical, closed, ..
        } => match electrical.as_str() {
            "Wire" => "Wire",
            "Bus" => "Bus",
            "BusEntry" => "Bus Entry",
            _ if *closed => "Polygon",
            _ => "Line",
        },
        K::Rectangle { .. } if p.presentation.record_type == 15 => "Sheet Symbol",
        K::Rectangle { radius, .. } if radius.x > 0 || radius.y > 0 => "Round Rectangle",
        K::Rectangle { .. } => "Rectangle",
        K::Arc {
            start, end, radii, ..
        } if (*end - *start).abs() >= 359.9 && radii.x != radii.y => "Ellipse",
        K::Arc { start, end, .. } if (*end - *start).abs() >= 359.9 => "Full Circle",
        K::Arc { .. } => "Arc",
        K::Bezier { .. } => "Bezier",
        K::Junction { .. } => "Junction",
        K::NoErc { .. } => "No ERC",
        K::Image { .. } => "Image",
        K::Ieee { .. } => "IEEE Symbol",
        K::Unsupported { .. } => "Drawing Object",
    }
}
impl PcbCraft {
    pub(super) fn schematic_properties(&mut self, ui: &mut egui::Ui, index: usize) {
        let doc = &mut self.documents[index];
        if doc.model.schematic.is_none() {
            return;
        }
        let panel = &mut self.property_ui.schematic;
        if panel.draft.as_ref().is_none_or(|d| {
            d.session != doc.session_id
                || d.selected != doc.view.selected
                || d.revision != doc.history.revision
        }) {
            panel.draft = Some(Draft::new(doc));
            panel.selected_parameter = None;
        }
        let draft = panel.draft.as_mut().unwrap();
        ui.set_width((ui.available_width() - 5.0).max(200.0));
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        let count = doc
            .view
            .sch_selection
            .len()
            .saturating_add(doc.view.sch_segments.len())
            .max(usize::from(doc.view.selected.is_some()));
        let context = if count > 1 {
            "Objects"
        } else if draft.component.is_some() {
            "Component"
        } else if let Some(p) = &draft.primitive {
            caption(p)
        } else {
            "Document Options"
        };
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        context_row(ui, context);
        ui.add_space(3.0);
        theme::search_field(ui, &mut self.property_ui.search, ui.available_width());
        ui.add_space(10.0);
        let parameters = if context == "Document Options" {
            tabs(ui, &mut panel.document_parameters, "General", "Parameters");
            panel.document_parameters
        } else if context == "Component" {
            tabs(ui, &mut panel.component_pins, "General", "Pins");
            false
        } else if context == "Port" {
            tabs(ui, &mut panel.port_parameters, "General", "Parameters");
            panel.port_parameters
        } else if context == "Sheet Symbol" {
            ui.horizontal(|ui| {
                ui.add(egui::Button::new("General").selected(true));
                ui.add_enabled(false, egui::Button::new("Parameters"))
                    .on_hover_text("Sheet Symbol parameters are not yet editable");
            });
            false
        } else {
            false
        };
        egui::TopBottomPanel::bottom("sch_selection_status")
            .exact_height(18.0)
            .show_inside(ui, |ui| {
                ui.label(match count {
                    0 => "Nothing selected".into(),
                    1 => "1 object is selected".into(),
                    n => format!("{n} objects are selected"),
                });
            });
        let mut commit = false;
        let mut commands = vec![];
        let mut choose = None;
        let mut dialog = None;
        let query = self.property_ui.search.to_lowercase();
        egui::ScrollArea::vertical()
            .id_salt((
                "schematic_properties_v3",
                doc.session_id,
                context,
                parameters,
                panel.component_pins,
            ))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                if doc.sch_editor.active() && !doc.sch_editor.hierarchy_pick {
                    placement_options(ui, doc);
                } else if count > 1 {
                    section(ui, "Properties", &query, |ui| {
                        ui.add_enabled(false, egui::Label::new("<Mixed>"));
                    });
                } else if context == "Document Options" {
                    if parameters {
                        parameter_table(
                            ui,
                            &draft.sheet.parameters,
                            None,
                            doc.model.schematic.as_ref().unwrap(),
                            &mut panel.filter,
                            &mut panel.selected_parameter,
                            &mut choose,
                            &mut dialog,
                            &mut commands,
                            &query,
                        );
                    } else {
                        commit |= document_options(
                            ui,
                            draft,
                            &mut doc.view,
                            &mut self.settings.units,
                            &mut panel.font_dialog,
                            &query,
                        );
                    }
                } else if let Some(c) = &mut draft.component {
                    if panel.component_pins {
                        pins(
                            ui,
                            c,
                            doc.model.schematic.as_ref().unwrap(),
                            &mut choose,
                            &query,
                        );
                    } else {
                        commit |= component_general(
                            ui,
                            c,
                            &mut draft.related,
                            self.settings.units,
                            &query,
                        );
                        parameter_table(
                            ui,
                            &c.parameters,
                            Some(c.id),
                            doc.model.schematic.as_ref().unwrap(),
                            &mut panel.filter,
                            &mut panel.selected_parameter,
                            &mut choose,
                            &mut dialog,
                            &mut commands,
                            &query,
                        );
                    }
                } else if let Some(p) = &mut draft.primitive {
                    if matches!(p.kind, K::SheetSymbol { .. }) {
                        commit |= sheet_symbol_properties(
                            ui,
                            p,
                            &mut draft.related,
                            doc.model.schematic.as_ref().unwrap(),
                            self.settings.units,
                            &query,
                            &mut choose,
                        );
                        return;
                    }
                    commit |= primitive_properties(
                        ui,
                        p,
                        &mut draft.font,
                        doc.model.schematic.as_ref().unwrap(),
                        self.settings.units,
                        &query,
                        parameters,
                        &mut choose,
                    );
                }
            });
        if panel.font_dialog {
            let mut open = true;
            egui::Window::new("Font")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    commit |= font_editor(ui, &mut draft.sheet_font, None);
                });
            panel.font_dialog = open;
        }
        if commit {
            commands.extend(draft.commands(doc));
        }
        if let Some(value) = dialog {
            panel.parameter = Some(value);
        }
        if let Some(parameter) = &mut panel.parameter {
            let mut open = true;
            let mut accept = false;
            egui::Window::new("Parameter Properties")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    text_row(ui, "Name", &mut parameter.name);
                    text_row(ui, "Value", &mut parameter.value);
                    accept = ui.button("OK").clicked();
                });
            if accept {
                if let Some(old) = &parameter.original
                    && old != &parameter.name
                {
                    commands.push(Edit::ParameterValue {
                        component: parameter.owner,
                        name: old.clone(),
                        value: None,
                    });
                }
                let scene = doc.model.schematic.as_ref().unwrap();
                if !scene
                    .primitives
                    .iter()
                    .chain(&scene.parameter_objects)
                    .any(|p| {
                        p.component == parameter.owner
                            && p.presentation.name.eq_ignore_ascii_case(&parameter.name)
                    })
                {
                    let position = parameter
                        .owner
                        .and_then(|id| scene.components.iter().find(|c| c.id == id))
                        .map_or(Point::default(), |c| c.position);
                    let mut p = edit::primitive(
                        K::Text {
                            position,
                            text: parameter.value.clone(),
                            font: scene.sheet.system_font,
                            orientation: 0,
                            justification: 0,
                        },
                        parameter.owner,
                    );
                    p.presentation = eda_core::schematic::Presentation {
                        record_type: 41,
                        name: parameter.name.clone(),
                        raw_text: Some(parameter.value.clone()),
                        hidden: true,
                        auto_position: parameter.owner.is_some(),
                        allow_sync: true,
                        ..Default::default()
                    };
                    p.color = 0x800000;
                    commands.push(Edit::Insert(edit::Fragment {
                        primitives: vec![p],
                        ..Default::default()
                    }));
                }
                commands.push(Edit::ParameterValue {
                    component: parameter.owner,
                    name: parameter.name.clone(),
                    value: Some(parameter.value.clone()),
                });
            }
            if accept || !open {
                panel.parameter = None;
            }
        }
        let revision = doc.history.revision;
        let result = if commands.is_empty() {
            Ok(false)
        } else {
            doc.edit_schematic(Edit::Batch(commands), revision)
        };
        if let Some(id) = choose {
            doc.view.selected = Some(id);
            doc.view.sch_selection = vec![id];
        }
        if let Err(error) = result {
            self.message(true, error);
            self.property_ui.schematic.draft = None;
        }
    }
}

fn context_row(ui: &mut egui::Ui, context: &str) {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 30.0), Sense::hover());
    let p = ui.painter();
    p.with_clip_rect(egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.left() + 104.0, rect.bottom()),
    ))
    .text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        context,
        egui::FontId::proportional(12.0),
        Color32::from_gray(220),
    );
    let filter = egui::Rect::from_min_size(
        egui::pos2(rect.right() - 42.0, rect.top() + 3.0),
        egui::vec2(28.0, 24.0),
    );
    p.rect_filled(filter, 1.0, ACCENT);
    theme::draw_icon(p, filter.shrink(5.0), theme::Icon::Filter, true);
    p.with_clip_rect(egui::Rect::from_min_max(
        egui::pos2(rect.left() + 106.0, rect.top()),
        filter.left_bottom(),
    ))
    .text(
        egui::pos2(rect.left() + 106.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Components (and 11 more)",
        egui::FontId::proportional(11.0),
        Color32::from_gray(212),
    );
    p.text(
        egui::pos2(rect.right() - 7.0, rect.center().y),
        egui::Align2::CENTER_CENTER,
        "▾",
        egui::FontId::proportional(12.0),
        Color32::GRAY,
    );
    response.on_hover_text("Object type context filtering is not supported yet");
}

fn section(ui: &mut egui::Ui, title: &str, query: &str, body: impl FnOnce(&mut egui::Ui)) {
    if !query.is_empty()
        && !title.to_lowercase().contains(query)
        && !matches!(
            title,
            "Properties" | "General" | "Parameters" | "Pins" | "Vertices"
        )
    {
        return;
    }
    super::properties::section(ui, title, body);
    ui.add_space(6.0);
}
pub(super) fn row(ui: &mut egui::Ui, label: &str, body: impl FnOnce(&mut egui::Ui)) {
    row_height(ui, label, 22.0, body);
}
pub(super) fn row_height(
    ui: &mut egui::Ui,
    label: &str,
    height: f32,
    body: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(107.0, height),
            egui::Layout::right_to_left(Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), height),
            egui::Layout::left_to_right(Align::Center),
            |ui| {
                ui.spacing_mut().interact_size.y = ui.spacing().interact_size.y.min(height);
                body(ui);
            },
        );
    });
}
fn tabs(ui: &mut egui::Ui, second: &mut bool, a: &str, b: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (value, label) in [(false, a), (true, b)] {
            if ui
                .add_sized(
                    [if label == "Parameters" { 70.0 } else { 54.0 }, 22.0],
                    egui::Button::new(label).selected(*second == value),
                )
                .clicked()
            {
                *second = value;
            }
        }
    });
    ui.add_space(4.0);
}
fn text(ui: &mut egui::Ui, value: &mut String) -> bool {
    let r = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::TextEdit::singleline(value),
    );
    r.lost_focus() || (r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
}
fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String) -> bool {
    let mut changed = false;
    row(ui, label, |ui| changed = text(ui, value));
    changed
}
pub(super) fn readonly(ui: &mut egui::Ui, value: &str) {
    let mut v = value.to_owned();
    ui.add_sized(
        [ui.available_width(), 22.0],
        egui::TextEdit::singleline(&mut v).interactive(false),
    );
}
fn numeric(ui: &mut egui::Ui, value: &mut i64, units: Units) -> bool {
    let mut n = units.from_nm(*value as f64);
    let r = ui.add_sized(
        [ui.available_width(), 22.0],
        egui::DragValue::new(&mut n)
            .speed(if units == Units::Mils { 1.0 } else { 0.01 })
            .max_decimals(6)
            .suffix(units.label()),
    );
    if r.changed() {
        *value = (n * if units == Units::Mils { 25_400.0 } else { 1e6 }).round() as i64;
    }
    r.lost_focus() || r.drag_stopped()
}
fn numeric_row(ui: &mut egui::Ui, label: &str, value: &mut i64, units: Units) -> bool {
    let mut changed = false;
    row(ui, label, |ui| changed = numeric(ui, value, units));
    changed
}
fn xy(ui: &mut egui::Ui, p: &mut Point, units: Units) -> bool {
    let mut changed = false;
    row(ui, "(X/Y)", |ui| {
        let w = (ui.available_width() - 4.0) / 2.0;
        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
            changed |= numeric(ui, &mut p.x, units)
        });
        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
            changed |= numeric(ui, &mut p.y, units)
        });
    });
    changed
}
fn combo(ui: &mut egui::Ui, id: impl std::hash::Hash, value: &mut u32, items: &[&str]) -> bool {
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .truncate()
        .width((ui.available_width() - 8.0).max(20.0))
        .height(240.0)
        .selected_text(items.get(*value as usize).copied().unwrap_or("Custom"))
        .show_ui(ui, |ui| {
            for (n, label) in items.iter().enumerate() {
                changed |= ui.selectable_value(value, n as u32, *label).changed();
            }
        });
    changed
}
fn rotation(ui: &mut egui::Ui, value: &mut u32) -> bool {
    let mut changed = false;
    row(ui, "Rotation", |ui| {
        changed = combo(
            ui,
            "rotation",
            value,
            &["0 Degrees", "90 Degrees", "180 Degrees", "270 Degrees"],
        )
    });
    changed
}
fn color(ui: &mut egui::Ui, value: &mut u32) -> bool {
    let old = ui.spacing().interact_size;
    ui.spacing_mut().interact_size = egui::vec2(22.0, 22.0);
    let mut rgb = [*value as u8, (*value >> 8) as u8, (*value >> 16) as u8];
    let changed = ui.color_edit_button_srgb(&mut rgb).changed();
    ui.spacing_mut().interact_size = old;
    if changed {
        *value = rgb[0] as u32 | (rgb[1] as u32) << 8 | (rgb[2] as u32) << 16;
    }
    changed
}
pub(super) fn small_icon(ui: &mut egui::Ui, kind: &str, value: &mut bool) -> bool {
    let (r, response) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(r, 1.0, Color32::from_gray(84));
    }
    let c = Color32::from_gray(195);
    let p = ui.painter();
    let centre = r.center();
    if kind == "eye" {
        p.add(egui::Shape::closed_line(
            vec![
                centre + egui::vec2(-8.0, 0.0),
                centre + egui::vec2(-4.0, -4.0),
                centre + egui::vec2(4.0, -4.0),
                centre + egui::vec2(8.0, 0.0),
                centre + egui::vec2(4.0, 4.0),
                centre + egui::vec2(-4.0, 4.0),
            ],
            Stroke::new(1.0, c),
        ));
        p.circle_filled(centre, 2.0, c);
        if !*value {
            p.line_segment(
                [
                    centre + egui::vec2(-8.0, 6.0),
                    centre + egui::vec2(8.0, -6.0),
                ],
                Stroke::new(1.0, c),
            );
        }
    } else {
        p.rect_stroke(
            egui::Rect::from_center_size(centre + egui::vec2(0.0, 3.0), egui::vec2(10.0, 8.0)),
            1.0,
            Stroke::new(1.0, c),
            egui::StrokeKind::Inside,
        );
        p.add(egui::Shape::line(
            vec![
                centre + egui::vec2(-3.0, 0.0),
                centre + egui::vec2(-3.0, -5.0),
                centre + egui::vec2(if *value { 3.0 } else { 6.0 }, -5.0),
                centre
                    + egui::vec2(
                        if *value { 3.0 } else { 6.0 },
                        if *value { 0.0 } else { -2.0 },
                    ),
            ],
            Stroke::new(1.0, c),
        ));
    }
    let clicked = response
        .on_hover_text(if kind == "eye" { "Visible" } else { "Locked" })
        .clicked();
    if clicked {
        *value = !*value;
    }
    clicked
}

fn tool_button(ui: &mut egui::Ui, kind: &str, enabled: bool) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(
        egui::vec2(27.0, 24.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let p = ui.painter();
    p.rect(
        r,
        2.0,
        Color32::from_gray(if enabled && response.hovered() {
            85
        } else {
            68
        }),
        Stroke::new(1.0, theme::BORDER),
        egui::StrokeKind::Inside,
    );
    let c = Color32::from_gray(if enabled { 210 } else { 116 });
    let s = Stroke::new(1.1, c);
    let q = r.center();
    if kind == "trash" {
        p.rect_stroke(
            egui::Rect::from_center_size(q + egui::vec2(0.0, 2.0), egui::vec2(9.0, 10.0)),
            0.0,
            s,
            egui::StrokeKind::Inside,
        );
        p.line_segment([q + egui::vec2(-6.0, -5.0), q + egui::vec2(6.0, -5.0)], s);
        p.line_segment([q + egui::vec2(-2.0, -7.0), q + egui::vec2(2.0, -7.0)], s);
        for x in [-2.0, 1.0] {
            p.line_segment([q + egui::vec2(x, -1.0), q + egui::vec2(x, 5.0)], s);
        }
    } else {
        p.add(egui::Shape::closed_line(
            vec![
                q + egui::vec2(-5.0, 6.0),
                q + egui::vec2(-4.0, 2.0),
                q + egui::vec2(4.0, -6.0),
                q + egui::vec2(7.0, -3.0),
                q + egui::vec2(-1.0, 5.0),
            ],
            s,
        ));
    }
    response.on_hover_text(if kind == "trash" { "Remove" } else { "Edit" })
}
fn divider(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
}
fn font_editor(ui: &mut egui::Ui, font: &mut SchFont, ink: Option<&mut u32>) -> bool {
    let mut changed = false;
    row(ui, "Font", |ui| {
        let w = (ui.available_width() - if ink.is_some() { 88.0 } else { 65.0 }).max(40.0);
        ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
            egui::ComboBox::from_id_salt("font_family")
                .truncate()
                .width(ui.available_width() - 8.0)
                .selected_text(&font.name)
                .show_ui(ui, |ui| {
                    for name in [
                        "Arial",
                        "Times New Roman",
                        "Segoe UI",
                        "Altium GOST Type A",
                        "DejaVu Sans",
                        "Liberation Sans",
                    ] {
                        changed |= ui
                            .selectable_value(&mut font.name, name.into(), name)
                            .changed();
                    }
                    changed |= text(ui, &mut font.name);
                });
        });
        let mut points = font.height_nm as f64 / 254_000.0;
        let r = ui.add_sized(
            [58.0, 22.0],
            egui::DragValue::new(&mut points)
                .range(0.1..=1000.0)
                .max_decimals(2),
        );
        if r.changed() {
            font.height_nm = (points * 254_000.0).round() as i64;
        }
        changed |= r.drag_stopped() || r.lost_focus();
        if let Some(ink) = ink {
            changed |= color(ui, ink);
        }
    });
    row(ui, "", |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let w = ui.available_width() / 4.0;
        for (label, value) in [
            ("B", &mut font.bold),
            ("I", &mut font.italic),
            ("U", &mut font.underline),
        ] {
            if ui
                .add_sized([w, 22.0], egui::Button::new(label).selected(*value))
                .clicked()
            {
                *value = !*value;
                changed = true;
            }
        }
        ui.add_enabled(false, egui::Button::new("T̶").min_size(egui::vec2(w, 22.0)));
    });
    changed
}
fn justification(ui: &mut egui::Ui, value: &mut u32) -> bool {
    let mut changed = false;
    row(ui, "Justification", |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            for y in (0..3).rev() {
                ui.horizontal(|ui| {
                    for x in 0..3 {
                        let n = y * 3 + x;
                        let icon = [["↙", "↓", "↘"], ["←", "•", "→"], ["↖", "↑", "↗"]][y as usize]
                            [x as usize];
                        if ui
                            .add_sized([21.0, 21.0], egui::Button::new(icon).selected(*value == n))
                            .clicked()
                        {
                            *value = n;
                            changed = true;
                        }
                    }
                });
            }
        });
    });
    changed
}

fn document_options(
    ui: &mut egui::Ui,
    d: &mut Draft,
    view: &mut View,
    units: &mut Units,
    font_dialog: &mut bool,
    query: &str,
) -> bool {
    let mut changed = false;
    let s = &mut d.sheet;
    section(ui, "Selection Filter", query, |ui| {
        if ui
            .add_sized(
                [73.0, 24.0],
                egui::Button::new(if view.sch_filters.iter().all(|v| *v) {
                    "All - On"
                } else {
                    "All - Off"
                })
                .selected(view.sch_filters.iter().all(|v| *v)),
            )
            .clicked()
        {
            let all = !view.sch_filters.iter().all(|v| *v);
            view.sch_filters.fill(all);
        }
        let labels = [
            "Components",
            "Wires",
            "Buses",
            "Sheet Symbols",
            "Sheet Entries",
            "Net Labels",
            "Parameters",
            "Ports",
            "Power Ports",
            "Texts",
            "Drawing Objects",
            "Other",
        ];
        for range in [0..3, 3..6, 6..10, 10..12] {
            ui.horizontal(|ui| {
                for n in range {
                    if ui
                        .add_sized(
                            [
                                match n {
                                    0 => 99.0,
                                    1 => 57.0,
                                    2 => 59.0,
                                    3 => 98.0,
                                    4 => 91.0,
                                    5 => 82.0,
                                    6 => 85.0,
                                    7 => 43.0,
                                    8 => 81.0,
                                    9 => 47.0,
                                    10 => 115.0,
                                    _ => 58.0,
                                },
                                24.0,
                            ],
                            egui::Button::new(labels[n]).selected(view.sch_filters[n]),
                        )
                        .clicked()
                    {
                        view.sch_filters[n] = !view.sch_filters[n];
                    }
                }
            });
        }
    });
    section(ui, "General", query, |ui| {
        ui.label("Units");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let w = ui.available_width() / 2.0;
            for (u, label) in [(Units::Millimetres, "mm"), (Units::Mils, "mils")] {
                if ui
                    .add_sized([w, 24.0], egui::Button::new(label).selected(*units == u))
                    .clicked()
                {
                    *units = u;
                }
            }
        });
        divider(ui);
        row(ui, "Visible Grid", |ui| {
            let w = ui.available_width() - 26.0;
            ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
                changed |= numeric(ui, &mut s.grid_nm, *units)
            });
            changed |= small_icon(ui, "eye", &mut s.grid_visible);
        });
        row(ui, "Snap Grid", |ui| {
            changed |= ui.checkbox(&mut s.options.snap_enabled, "").changed();
            ui.allocate_ui(egui::vec2(ui.available_width() - 28.0, 22.0), |ui| {
                changed |= numeric(ui, &mut s.snap_nm, *units)
            });
            ui.add_enabled(false, egui::Button::new("↔"));
        });
        row(ui, "", |ui| {
            changed |= ui
                .checkbox(
                    &mut s.options.electrical_snap,
                    "Snap to Electrical Object Hotspots",
                )
                .changed();
        });
        changed |= numeric_row(ui, "Snap Distance", &mut s.options.snap_distance_nm, *units);
        row(ui, "Document Font", |ui| {
            if ui
                .link(format!(
                    "{}, {}",
                    d.sheet_font.name,
                    d.sheet_font.height_nm as f64 / 254_000.0
                ))
                .clicked()
            {
                *font_dialog = true;
            }
        });
        row(ui, "Sheet Border", |ui| {
            changed |= ui.checkbox(&mut s.border, "").changed();
            changed |= color(ui, &mut s.options.border_color);
        });
        row(ui, "Sheet Color", |ui| {
            changed |= color(ui, &mut s.background)
        });
    });
    section(ui, "Page Options", query, |ui| {
        ui.label(RichText::new("Formatting and Size").strong());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let w = ui.available_width() / 3.0;
            ui.add_enabled(
                false,
                egui::Button::new("Template").min_size(egui::vec2(w, 24.0)),
            );
            for label in ["Standard", "Custom"] {
                if ui
                    .add_sized(
                        [w, 24.0],
                        egui::Button::new(label)
                            .selected((label == "Custom") == s.style.starts_with("Custom")),
                    )
                    .clicked()
                    && (label == "Custom") != s.style.starts_with("Custom")
                {
                    s.style = if label == "Standard" {
                        let portrait = d.size.y > d.size.x;
                        let (_, w, h) = eda_core::schematic::STANDARD_SHEETS[0];
                        d.size = if portrait {
                            Point { x: h, y: w }
                        } else {
                            Point { x: w, y: h }
                        };
                        "Standard 0".into()
                    } else {
                        label.into()
                    };
                    changed = true;
                }
            }
        });
        ui.add_space(10.0);
        row(ui, "Sheet Size", |ui| {
            egui::ComboBox::from_id_salt("sheet_size")
                .width(24.0)
                .selected_text("")
                .show_ui(ui, |ui| {
                    for (n, (name, w, h)) in
                        eda_core::schematic::STANDARD_SHEETS.into_iter().enumerate()
                    {
                        if ui.selectable_label(false, name).clicked() {
                            d.size = Point { x: w, y: h };
                            s.style = format!("Standard {n}");
                            changed = true;
                        }
                    }
                });
            ui.label(format!(
                "{}mm x {}mm",
                d.size.x as f64 / 1e6,
                d.size.y as f64 / 1e6
            ));
        });
        row(ui, "Orientation", |ui| {
            let mut v = u32::from(d.size.y > d.size.x);
            if combo(ui, "sheet_orientation", &mut v, &["Landscape", "Portrait"]) {
                std::mem::swap(&mut d.size.x, &mut d.size.y);
                changed = true;
            }
        });
        row(ui, "Title Block", |ui| {
            changed |= ui.checkbox(&mut s.title_block, "").changed();
            ui.add_enabled(false, egui::Button::new("Standard  ▾"));
        });
        if s.style == "Custom" {
            changed |= numeric_row(ui, "Width", &mut d.size.x, *units);
            changed |= numeric_row(ui, "Height", &mut d.size.y, *units);
        }
        divider(ui);
        ui.label(RichText::new("Margin and Zones").strong());
        row(ui, "Show Zones", |ui| {
            changed |= ui.checkbox(&mut s.options.show_zones, "").changed()
        });
        for (label, n) in [("Vertical", 1), ("Horizontal", 0)] {
            row(ui, label, |ui| {
                let r = ui.add_sized(
                    [ui.available_width(), 22.0],
                    egui::DragValue::new(&mut s.zones[n]).range(1..=64),
                );
                changed |= r.lost_focus() || r.drag_stopped();
            });
        }
        row(ui, "Origin", |ui| {
            let mut v = u32::from(s.zone_origin_lower_right);
            if combo(ui, "zone_origin", &mut v, &["Top Left", "Bottom Right"]) {
                s.zone_origin_lower_right = v == 1;
                changed = true;
            }
        });
        if s.style == "Custom" {
            changed |= numeric_row(ui, "Margin Width", &mut s.margin_nm, *units);
        }
    });
    changed
}
fn component_general(
    ui: &mut egui::Ui,
    c: &mut Component,
    related: &mut [SchPrimitive],
    units: Units,
    query: &str,
) -> bool {
    let mut changed = false;
    section(ui, "General", query, |ui| {
        ui.add_space(2.0);
        for (label, role) in [
            ("Designator", TextRole::Designator),
            ("Comment", TextRole::Comment),
        ] {
            row(ui, label, |ui| {
                ui.allocate_ui(egui::vec2(ui.available_width() - 52.0, 22.0), |ui| {
                    if role == TextRole::Designator {
                        changed |= text(ui, &mut c.designator);
                    } else {
                        let key = c
                            .parameters
                            .keys()
                            .find(|name| name.eq_ignore_ascii_case("Comment"))
                            .cloned()
                            .unwrap_or_else(|| "Comment".into());
                        let value = c.parameters.entry(key).or_insert_with(|| c.comment.clone());
                        changed |= text(ui, value);
                    }
                });
                if let Some(p) = related.iter_mut().find(|p| p.text_role == Some(role)) {
                    let mut visible = !p.presentation.hidden;
                    if small_icon(ui, "eye", &mut visible) {
                        p.presentation.hidden = !visible;
                        changed = true;
                    }
                    changed |= small_icon(ui, "lock", &mut p.presentation.locked);
                } else {
                    ui.add_enabled(false, egui::Button::new("◉"));
                    ui.add_enabled(false, egui::Button::new("♙"));
                }
            });
        }
        ui.add_space(2.0);
        row(ui, "Part", |ui| {
            let mut part = c.part.to_string();
            ui.add_enabled_ui(false, |ui| {
                ui.add_sized([49.0, 22.0], egui::TextEdit::singleline(&mut part));
            });
            ui.label("of Parts");
            ui.label(c.settings.part_count.to_string());
            changed |= small_icon(ui, "lock", &mut c.settings.part_locked);
        });
        ui.add_space(2.0);
        row(ui, "Description", |ui| {
            let r = ui.add_sized(
                [ui.available_width(), 57.0],
                egui::TextEdit::multiline(&mut c.description).desired_rows(3),
            );
            changed |= r.lost_focus();
        });
        ui.add_space(3.0);
        row(ui, "Type", |ui| {
            changed |= combo(
                ui,
                "component_type",
                &mut c.settings.kind,
                &[
                    "Standard",
                    "Mechanical",
                    "Graphical",
                    "Net Tie (In BOM)",
                    "Net Tie (No BOM)",
                    "Standard (No BOM)",
                    "Jumper",
                ],
            )
        });
        ui.add_space(1.0);
        row(ui, "Design Item ID", |ui| {
            readonly(
                ui,
                if c.settings.design_item_id.is_empty() {
                    &c.library_reference
                } else {
                    &c.settings.design_item_id
                },
            )
        });
        ui.add_space(1.0);
        row(ui, "Source", |ui| readonly(ui, &c.settings.source));
        ui.add_space(1.0);
    });
    section(ui, "Location", query, |ui| {
        changed |= xy(ui, &mut c.position, units);
        changed |= rotation(ui, &mut c.settings.rotation);
    });
    changed
}
#[allow(clippy::too_many_arguments)]
fn parameter_table(
    ui: &mut egui::Ui,
    values: &std::collections::BTreeMap<String, String>,
    owner: Option<Uuid>,
    s: &Schematic,
    filter: &mut usize,
    selected: &mut Option<String>,
    choose: &mut Option<Uuid>,
    dialog: &mut Option<ParameterDialog>,
    commands: &mut Vec<Edit>,
    query: &str,
) {
    section(ui, "Parameters", query, |ui| {
        let labels: &[&str] = if owner.is_some() {
            &[
                "All",
                "Footprints",
                "Models",
                "Parameters",
                "Links",
                "Rules",
            ]
        } else {
            &["All", "Parameters", "Rules"]
        };
        ui.horizontal_wrapped(|ui| {
            for (n, label) in labels.iter().enumerate() {
                if ui
                    .add_sized(
                        [
                            match *label {
                                "All" => 42.0,
                                "Parameters" => 74.0,
                                _ => 67.0,
                            },
                            24.0,
                        ],
                        egui::Button::new(*label).selected(if n == 0 {
                            *filter == 0
                        } else {
                            *filter & (1 << n) == 0
                        }),
                    )
                    .clicked()
                {
                    if n == 0 {
                        *filter = if *filter == 0 {
                            (1 << labels.len()) - 2
                        } else {
                            0
                        };
                    } else {
                        *filter ^= 1 << n;
                    }
                }
            }
        });
        egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                let more_id = ui.make_persistent_id(("parameter_show_more", owner));
                let mut more = ui.data_mut(|d| d.get_persisted::<bool>(more_id).unwrap_or(false));
                let favorites_id = egui::Id::new("schematic_property_favorites");
                let mut favorites = ui.data_mut(|d| {
                    d.get_persisted::<Vec<String>>(favorites_id)
                        .unwrap_or_else(|| vec!["Power".into(), "Resistance".into()])
                });
                let component = owner.and_then(|id| s.components.iter().find(|c| c.id == id));
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [ui.available_width() * 0.49, 20.0],
                        egui::Label::new("Name"),
                    );
                    ui.label("Value");
                });
                egui::ScrollArea::vertical()
                    .id_salt(("parameter_rows", owner))
                    .min_scrolled_height(if owner.is_some() {
                        if more { 220.0 } else { 120.0 }
                    } else {
                        270.0
                    })
                    .max_height(if owner.is_some() {
                        if more { 220.0 } else { 120.0 }
                    } else {
                        270.0
                    })
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        if let Some(c) = component {
                            for model in c.settings.models.iter().filter(|m| m.current) {
                                let footprint = model.kind.eq_ignore_ascii_case("PCBLIB");
                                if *filter & (1 << if footprint { 1 } else { 2 }) != 0 {
                                    continue;
                                }
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [ui.available_width() * 0.49, 26.0],
                                        egui::Label::new(if footprint {
                                            "Footprint"
                                        } else {
                                            "Simulation"
                                        }),
                                    );
                                    let value_width = (ui.available_width()
                                        - if footprint { 38.0 } else { 0.0 })
                                    .max(1.0);
                                    ui.add_sized(
                                        [value_width, 24.0],
                                        egui::Label::new(&model.name).truncate().halign(Align::Min),
                                    );
                                    if footprint {
                                        ui.add_enabled(
                                            false,
                                            egui::Button::new("Show").frame(false),
                                        );
                                    }
                                });
                            }
                        }
                        let show_parameters =
                            *filter & (1 << if owner.is_some() { 3 } else { 1 }) == 0;
                        if show_parameters {
                            let mut ordered: Vec<_> = values.iter().collect();
                            ordered.sort_by_key(|(name, _)| {
                                s.primitives
                                    .iter()
                                    .chain(&s.parameter_objects)
                                    .find(|p| {
                                        p.component == owner
                                            && p.presentation.name.eq_ignore_ascii_case(name)
                                    })
                                    .map_or(u32::MAX, |p| p.source_index)
                            });
                            for (name, value) in ordered {
                                if name == "Comment"
                                    || name.starts_with("ComponentLink")
                                    || (owner.is_some()
                                        && !more
                                        && query.is_empty()
                                        && !favorites.contains(name))
                                    || (!query.is_empty()
                                        && !name.to_lowercase().contains(query)
                                        && !value.to_lowercase().contains(query))
                                {
                                    continue;
                                }
                                let p = s.primitives.iter().chain(&s.parameter_objects).find(|p| {
                                    p.component == owner
                                        && p.presentation.name.eq_ignore_ascii_case(name)
                                });
                                ui.horizontal(|ui| {
                                    if let Some(p) = p {
                                        let mut visible = !p.presentation.hidden;
                                        let mut locked = p.presentation.locked;
                                        let mut new = p.clone();
                                        if small_icon(ui, "eye", &mut visible) {
                                            new.presentation.hidden = !visible;
                                        }
                                        if small_icon(ui, "lock", &mut locked) {
                                            new.presentation.locked = locked;
                                        }
                                        if new != *p {
                                            commands.push(Edit::PrimitiveProperties {
                                                id: p.id,
                                                value: Box::new(new),
                                            });
                                        }
                                    }
                                    let width = ui.available_width() * 0.49;
                                    let r = ui.add_sized(
                                        [width, 20.0],
                                        egui::Button::new(name)
                                            .truncate()
                                            .selected(selected.as_ref() == Some(name))
                                            .frame(false),
                                    );
                                    if r.clicked() {
                                        *selected = Some(name.clone());
                                    }
                                    if r.double_clicked() {
                                        if let Some(p) = p {
                                            *choose = Some(p.id);
                                        } else {
                                            *dialog = Some(ParameterDialog {
                                                owner,
                                                original: Some(name.clone()),
                                                name: name.clone(),
                                                value: value.clone(),
                                            });
                                        }
                                    }
                                    let value_width = (ui.available_width()
                                        - if owner.is_some() && more { 22.0 } else { 0.0 })
                                    .max(1.0);
                                    ui.add_sized(
                                        [value_width, 20.0],
                                        egui::Label::new(value).truncate().halign(Align::Min),
                                    );
                                    if owner.is_some() && more {
                                        let favorite = favorites.contains(name);
                                        if ui
                                            .small_button(if favorite { "★" } else { "☆" })
                                            .on_hover_text("Favorite parameter")
                                            .clicked()
                                        {
                                            if favorite {
                                                favorites.retain(|n| n != name);
                                            } else {
                                                favorites.push(name.clone());
                                            }
                                        }
                                    }
                                });
                            }
                            if owner.is_some()
                                && ui
                                    .link(if more { "Show Less" } else { "Show More" })
                                    .clicked()
                            {
                                more = !more;
                            }
                        } else {
                            ui.add_enabled(false, egui::Label::new("No entries"));
                        }
                    });
                ui.data_mut(|d| {
                    d.insert_persisted(more_id, more);
                    d.insert_persisted(favorites_id, favorites);
                });
                if owner.is_some() && *filter & (1 << 4) == 0 {
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt(("component_links", owner))
                        .max_height(92.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for (name, value) in values.iter().filter(|(n, _)| {
                                n.starts_with("ComponentLink") && n.ends_with("Description")
                            }) {
                                let key = name.trim_end_matches("Description").to_owned() + "URL";
                                ui.horizontal(|ui| {
                                    ui.add_sized(
                                        [ui.available_width() * 0.49, 20.0],
                                        egui::Label::new(value).truncate(),
                                    );
                                    ui.add(
                                        egui::Label::new(
                                            values.get(&key).map_or("", String::as_str),
                                        )
                                        .truncate(),
                                    );
                                });
                            }
                        });
                    ui.separator();
                }
                if owner.is_some() && *filter & (1 << 5) == 0 {
                    ui.vertical_centered(|ui| {
                        ui.add_enabled(false, egui::Label::new("No Rules"));
                    });
                }
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if tool_button(ui, "trash", selected.is_some())
                        .on_hover_text("Remove parameter")
                        .clicked()
                        && let Some(name) = selected.clone()
                    {
                        commands.push(Edit::ParameterValue {
                            component: owner,
                            name,
                            value: None,
                        });
                        *selected = None;
                    }
                    if tool_button(ui, "edit", selected.is_some())
                        .on_hover_text("Edit parameter")
                        .clicked()
                        && let Some(name) = selected.as_ref()
                    {
                        if let Some(p) = s.primitives.iter().chain(&s.parameter_objects).find(|p| {
                            p.component == owner && p.presentation.name.eq_ignore_ascii_case(name)
                        }) {
                            *choose = Some(p.id);
                        } else {
                            *dialog = Some(ParameterDialog {
                                owner,
                                original: Some(name.clone()),
                                name: name.clone(),
                                value: values.get(name).cloned().unwrap_or_default(),
                            });
                        }
                    }
                    ui.menu_button("Add…  ▾", |ui| {
                        if ui.button("Parameter").clicked() {
                            *dialog = Some(ParameterDialog {
                                owner,
                                original: None,
                                name: "Parameter".into(),
                                value: String::new(),
                            });
                            ui.close();
                        }
                        for label in ["Rule", "Footprint", "Model", "Link"] {
                            ui.add_enabled(false, egui::Button::new(label));
                        }
                    });
                });
            });
    });
}
fn pins(ui: &mut egui::Ui, c: &Component, s: &Schematic, choose: &mut Option<Uuid>, query: &str) {
    section(ui, "Pins", query, |ui| {
        egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([115.0, 20.0], egui::Label::new("Pins"));
                    ui.label("Name");
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_enabled(false, egui::Button::new("Show Full"));
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("component_pins")
                    .min_scrolled_height(475.0)
                    .max_height(475.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for p in s.primitives.iter().filter(|p| p.component == Some(c.id)) {
                            if let K::Pin { name, number, .. } = &p.kind {
                                ui.horizontal(|ui| {
                                    let r = ui.add_sized(
                                        [115.0, 20.0],
                                        egui::Button::new(format!("− Pin {number}")).frame(false),
                                    );
                                    ui.label(name);
                                    if r.double_clicked() {
                                        *choose = Some(p.id);
                                    }
                                });
                            }
                        }
                    });
            });
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            tool_button(ui, "trash", false);
            tool_button(ui, "edit", false);
            ui.add_enabled(false, egui::Button::new("Add"));
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn sheet_symbol_properties(
    ui: &mut egui::Ui,
    p: &mut SchPrimitive,
    related: &mut [SchPrimitive],
    s: &Schematic,
    units: Units,
    query: &str,
    choose: &mut Option<Uuid>,
) -> bool {
    let K::SheetSymbol {
        a,
        b,
        designator,
        filename,
    } = &mut p.kind
    else {
        return false;
    };
    let mut changed = false;
    section(ui, "Location", query, |ui| {
        let old = *a;
        changed |= xy(ui, a, units);
        b.x += a.x - old.x;
        b.y += a.y - old.y;
    });
    section(ui, "Properties", query, |ui| {
        for (label, value, record) in [
            ("Designator", designator, 32),
            ("File Name", &mut *filename, 33),
        ] {
            row(ui, label, |ui| {
                ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                    changed |= text(ui, value);
                });
                if let Some(child) = related
                    .iter_mut()
                    .find(|c| c.presentation.record_type == record)
                {
                    let mut visible = !child.presentation.hidden;
                    if small_icon(ui, "eye", &mut visible) {
                        child.presentation.hidden = !visible;
                        changed = true;
                    }
                }
            });
        }
        row(ui, "Bus Text Style", |ui| {
            ui.add_enabled(false, egui::Button::new("Full                 ▾"));
        });
        row(ui, "Width", |ui| {
            let mut width = b.x - a.x;
            ui.allocate_ui(
                egui::vec2((ui.available_width() - 52.0) / 2.0, 22.0),
                |ui| {
                    changed |= numeric(ui, &mut width, units);
                },
            );
            ui.label("Height");
            let mut height = a.y - b.y;
            changed |= numeric(ui, &mut height, units);
            if width > 0 && height > 0 {
                b.x = a.x + width;
                b.y = a.y - height;
            }
        });
        row(ui, "Line Style", |ui| {
            ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                changed |= width_combo(ui, &mut p.width_nm);
            });
            changed |= color(ui, &mut p.color);
        });
        changed |= fill_row(ui, &mut p.fill);
    });
    section(ui, "Source", query, |ui| {
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 8.0) / 3.0;
            ui.add_sized([width, 22.0], egui::Button::new("Local").selected(true));
            for name in ["Device", "Managed"] {
                ui.add_enabled_ui(false, |ui| {
                    ui.add_sized([width, 22.0], egui::Button::new(name));
                });
            }
        });
        row(ui, "File Name", |ui| readonly(ui, filename));
    });
    section(ui, "Sheet Entries", query, |ui| {
        let table_width = (ui.available_width() - 4.0).max(100.0);
        let column_width = (table_width - 12.0) / 2.0;
        egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.set_width(table_width);
                egui::ScrollArea::vertical()
                    .id_salt("symbol_entries")
                    .auto_shrink([false, false])
                    .min_scrolled_height(112.0)
                    .max_height(112.0)
                    .show(ui, |ui| {
                        egui::Grid::new("symbol_entry_rows")
                            .num_columns(2)
                            .min_col_width(column_width)
                            .max_col_width(column_width)
                            .spacing([8.0, 0.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.add_sized([column_width, 20.0], egui::Label::new("Name"));
                                ui.label("PortIO");
                                ui.end_row();
                                for entry in s
                                    .primitives
                                    .iter()
                                    .filter(|entry| entry.presentation.parent == Some(p.id))
                                {
                                    if let K::SheetEntry { name, io_type, .. } = &entry.kind {
                                        if ui
                                            .add_sized(
                                                [column_width, 20.0],
                                                egui::Button::new(name).frame(false),
                                            )
                                            .clicked()
                                        {
                                            *choose = Some(entry.id);
                                        }
                                        ui.label(
                                            ["Unspecified", "Output", "Input", "Bidirectional"]
                                                .get(*io_type as usize)
                                                .copied()
                                                .unwrap_or("Custom"),
                                        );
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                let entry_font = s.primitives.iter().find_map(|entry| {
                    if entry.presentation.parent == Some(p.id)
                        && let K::SheetEntry { font, .. } = entry.kind
                    {
                        s.fonts.get(font)
                    } else {
                        None
                    }
                });
                if let Some(font) = entry_font {
                    ui.label(format!(
                        "{}, {}",
                        font.name,
                        font.height_nm as f64 / 254_000.0
                    ));
                }
                ui.allocate_ui_with_layout(
                    egui::vec2(table_width, 24.0),
                    egui::Layout::right_to_left(Align::Center),
                    |ui| {
                        tool_button(ui, "trash", false);
                        ui.add_enabled(
                            false,
                            egui::Button::new("Add").min_size(egui::vec2(72.0, 22.0)),
                        );
                    },
                );
            });
    });
    changed
}

fn sheet_entry_properties(
    ui: &mut egui::Ui,
    p: &mut SchPrimitive,
    font: &mut SchFont,
    query: &str,
) -> bool {
    let K::SheetEntry {
        name,
        io_type,
        text_color,
        arrow_kind,
        ..
    } = &mut p.kind
    else {
        return false;
    };
    let mut changed = false;
    section(ui, "Properties", query, |ui| {
        changed |= text_row(ui, "Name", name);
        row(ui, "I/O Type", |ui| {
            changed |= combo(
                ui,
                "entry_io",
                io_type,
                &["Unspecified", "Output", "Input", "Bidirectional"],
            );
        });
        changed |= text_row(ui, "Harness Type", &mut p.presentation.harness_type);
        row(ui, "Cross Ref", |ui| {
            ui.add_enabled(false, egui::Button::new("◎"));
        });
        divider(ui);
        changed |= font_editor(ui, font, Some(text_color));
        divider(ui);
        row(ui, "Kind", |ui| {
            egui::ComboBox::from_id_salt("entry_kind")
                .width(ui.available_width() - 8.0)
                .selected_text(if arrow_kind.is_empty() {
                    "Block & Triangle"
                } else {
                    arrow_kind.as_str()
                })
                .show_ui(ui, |ui| {
                    for kind in ["Block & Triangle", "Triangle", "Arrow", "Arrow Tail"] {
                        // Other native forms still need a verified renderer.
                        ui.add_enabled_ui(kind == "Block & Triangle", |ui| {
                            changed |= ui.selectable_value(arrow_kind, kind.into(), kind).changed();
                        });
                    }
                });
        });
        row(ui, "Border Color", |ui| {
            changed |= color(ui, &mut p.color);
            ui.label("Fill Color");
            let mut fill = p.fill.unwrap_or(0x80ffff);
            if color(ui, &mut fill) {
                p.fill = Some(fill);
                changed = true;
            }
        });
    });
    net_sections(ui, query);
    changed
}

#[allow(clippy::too_many_arguments)]
fn primitive_properties(
    ui: &mut egui::Ui,
    p: &mut SchPrimitive,
    font: &mut SchFont,
    s: &Schematic,
    units: Units,
    query: &str,
    parameters: bool,
    choose: &mut Option<Uuid>,
) -> bool {
    let mut changed = false;
    let title = caption(p);
    if title == "Wire" {
        net_sections(ui, query);
        changed |= vertices(ui, p, units, query);
        return changed;
    }
    if matches!(p.kind, K::SheetEntry { .. }) {
        return sheet_entry_properties(ui, p, font, query);
    }
    let mut pos = primitive_anchor(p);
    let mut turns = match &p.kind {
        K::Text { orientation, .. }
        | K::PowerPort { orientation, .. }
        | K::Pin { orientation, .. }
        | K::Ieee { orientation, .. } => Some(*orientation),
        _ => None,
    };
    section(ui, "Location", query, |ui| {
        changed |= xy(ui, &mut pos, units);
        if let Some(v) = &mut turns {
            changed |= rotation(ui, v);
        }
    });
    let old = primitive_anchor(p);
    if pos != old {
        let _ = edit::transform_primitive(
            p,
            old,
            0,
            Point {
                x: pos.x - old.x,
                y: pos.y - old.y,
            },
        );
    }
    if let Some(turns) = turns {
        match &mut p.kind {
            K::Text { orientation, .. }
            | K::PowerPort { orientation, .. }
            | K::Pin { orientation, .. }
            | K::Ieee { orientation, .. } => *orientation = turns,
            _ => {}
        }
    }
    if parameters && matches!(p.kind, K::Port { .. }) {
        net_sections(ui, query);
        return changed;
    }
    section(ui, "Properties", query, |ui| match &mut p.kind {
        K::Text {
            text: value,
            justification: just,
            ..
        } => {
            if title == "Parameter" || title == "Designator" {
                row(ui, "Component", |ui| {
                    if let Some(c) = p
                        .component
                        .and_then(|id| s.components.iter().find(|c| c.id == id))
                        && ui.link(&c.designator).clicked()
                    {
                        *choose = Some(c.id);
                    }
                });
                row(ui, "Name", |ui| {
                    ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                        if matches!(p.text_role, Some(TextRole::Comment | TextRole::Designator)) {
                            readonly(ui, &p.presentation.name);
                        } else {
                            changed |= text(ui, &mut p.presentation.name);
                        }
                    });
                    changed |= small_icon(ui, "eye", &mut p.presentation.show_name);
                });
                row(ui, "Value", |ui| {
                    let w = ui.available_width() - if title == "Designator" { 0.0 } else { 26.0 };
                    ui.allocate_ui(egui::vec2(w, 22.0), |ui| {
                        changed |= text(ui, p.presentation.raw_text.as_mut().unwrap_or(value))
                    });
                    if title != "Designator" {
                        let mut visible = !p.presentation.hidden;
                        if small_icon(ui, "eye", &mut visible) {
                            p.presentation.hidden = !visible;
                            changed = true;
                        }
                    }
                });
                for (label, v) in [
                    ("Autoposition", &mut p.presentation.auto_position),
                    ("Lock Parameter", &mut p.presentation.locked),
                    (
                        "Allow Synchronization with Database",
                        &mut p.presentation.allow_sync,
                    ),
                ] {
                    row_height(ui, "", 17.0, |ui| {
                        changed |= ui.checkbox(v, label).changed()
                    });
                }
            } else {
                changed |= text_row(
                    ui,
                    if title == "Net Label" {
                        "Net Name"
                    } else {
                        "Text"
                    },
                    p.presentation.raw_text.as_mut().unwrap_or(value),
                );
                if title == "Text" {
                    changed |= text_row(ui, "URL", &mut p.presentation.url);
                }
            }
            divider(ui);
            changed |= font_editor(ui, font, Some(&mut p.color));
            changed |= justification(ui, just);
        }
        K::PowerPort {
            text: value,
            style,
            show_name,
            ..
        } => {
            row(ui, "Name", |ui| {
                ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                    changed |= text(ui, value)
                });
                changed |= small_icon(ui, "eye", show_name);
            });
            ui.add_space(10.0);
            row(ui, "Style", |ui| {
                ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                    changed |= combo(
                        ui,
                        "power_style",
                        style,
                        &[
                            "Circle",
                            "Arrow",
                            "Bar",
                            "Wave",
                            "Power Ground",
                            "Signal Ground",
                            "Earth",
                            "GOST Arrow",
                            "GOST Power Ground",
                            "GOST Earth",
                        ],
                    )
                });
                changed |= color(ui, &mut p.color);
            });
            divider(ui);
            changed |= font_editor(ui, font, Some(&mut p.color));
        }
        K::TextFrame {
            a,
            b,
            text: value,
            alignment,
            word_wrap,
            show_border,
            ..
        } => {
            row(ui, "Text", |ui| {
                let r = ui.add_sized(
                    [ui.available_width(), 111.0],
                    egui::TextEdit::multiline(value),
                );
                changed |= r.lost_focus();
            });
            row(ui, "", |ui| {
                changed |= ui.checkbox(word_wrap, "Word Wrap").changed()
            });
            row(ui, "", |ui| {
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Clip to Area"));
            });
            divider(ui);
            changed |= font_editor(ui, font, Some(&mut p.color));
            row(ui, "Alignment", |ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (n, label) in [(1, "≡"), (0, "☰"), (2, "≡")] {
                    if ui
                        .add_sized(
                            [39.0, 24.0],
                            egui::Button::new(label).selected(*alignment == n),
                        )
                        .clicked()
                    {
                        *alignment = n;
                        changed = true;
                    }
                }
            });
            divider(ui);
            row(ui, "Text Margin", |ui| readonly(ui, "0mil"));
            changed |= dimensions(ui, a, b, units);
            row(ui, "Border", |ui| {
                changed |= ui.checkbox(show_border, "").changed();
                changed |= width_combo(ui, &mut p.width_nm);
            });
            changed |= fill_row(ui, &mut p.fill);
        }
        K::Rectangle { a, b, radius } => {
            changed |= dimensions(ui, a, b, units);
            changed |= border_row(ui, &mut p.width_nm, &mut p.color);
            changed |= line_style(ui, &mut p.line_style);
            if radius.x != 0 || radius.y != 0 {
                changed |= numeric_row(ui, "Corner X", &mut radius.x, units);
                changed |= numeric_row(ui, "Corner Y", &mut radius.y, units);
            }
            divider(ui);
            changed |= fill_row(ui, &mut p.fill);
            row(ui, "Transparent", |ui| {
                ui.add_enabled(false, egui::Checkbox::new(&mut false, ""));
            });
        }
        K::Arc {
            radii, start, end, ..
        } => {
            changed |= numeric_row(ui, "Radius", &mut radii.x, units);
            if title == "Ellipse" {
                changed |= numeric_row(ui, "Secondary Radius", &mut radii.y, units);
            }
            for (label, v) in [("Start Angle", start), ("End Angle", end)] {
                row(ui, label, |ui| {
                    let r = ui.add_sized(
                        [ui.available_width(), 22.0],
                        egui::DragValue::new(v).suffix("°"),
                    );
                    changed |= r.drag_stopped() || r.lost_focus();
                });
            }
            changed |= border_row(ui, &mut p.width_nm, &mut p.color);
            changed |= line_style(ui, &mut p.line_style);
            changed |= fill_row(ui, &mut p.fill);
        }
        K::SheetSymbol { .. } | K::SheetEntry { .. } => {
            unreachable!("hierarchy properties have a dedicated form")
        }
        K::Port {
            name,
            width_nm,
            height_nm,
            io_type,
            ..
        } => {
            changed |= text_row(ui, "Name", name);
            row(ui, "I/O Type", |ui| {
                changed |= combo(
                    ui,
                    "port_io",
                    io_type,
                    &["Unspecified", "Output", "Input", "Bidirectional"],
                )
            });
            changed |= text_row(ui, "Harness Type", &mut p.presentation.harness_type);
            row(ui, "Cross Ref", |ui| {
                ui.add_enabled(false, egui::Button::new("◎"));
            });
            changed |= numeric_row(ui, "Width", width_nm, units);
            changed |= numeric_row(ui, "Height", height_nm, units);
            divider(ui);
            let mut text_color = p.presentation.text_color.unwrap_or(p.color);
            if font_editor(ui, font, Some(&mut text_color)) {
                p.presentation.text_color = Some(text_color);
                changed = true;
            }
            row(ui, "Alignment", |ui| {
                let alignment = p.presentation.alignment.unwrap_or(0);
                for (n, label) in [(1, "Left"), (0, "Center"), (2, "Right")] {
                    if ui.selectable_label(alignment == n, label).clicked() {
                        p.presentation.alignment = Some(n);
                        changed = true;
                    }
                }
            });
            divider(ui);
            changed |= border_row(ui, &mut p.width_nm, &mut p.color);
            changed |= fill_row(ui, &mut p.fill);
        }
        K::Pin {
            name,
            number,
            length_nm,
            show_name,
            show_number,
            electrical,
            outer_symbol,
            inner_symbol,
            ..
        } => {
            changed |= text_row(ui, "Name", name);
            changed |= text_row(ui, "Designator", number);
            row(ui, "", |ui| {
                changed |= ui.checkbox(show_name, "Show Name").changed()
            });
            row(ui, "", |ui| {
                changed |= ui.checkbox(show_number, "Show Designator").changed()
            });
            changed |= numeric_row(ui, "Length", length_nm, units);
            row(ui, "Electrical Type", |ui| {
                changed |= combo(
                    ui,
                    "pin_electrical",
                    electrical,
                    &[
                        "Input",
                        "I/O",
                        "Output",
                        "Open Collector",
                        "Passive",
                        "HiZ",
                        "Open Emitter",
                        "Power",
                    ],
                )
            });
            for (label, v) in [
                ("Outside Edge", outer_symbol),
                ("Inside Edge", inner_symbol),
            ] {
                row(ui, label, |ui| {
                    changed |= combo(
                        ui,
                        label,
                        v,
                        &[
                            "None",
                            "Dot",
                            "Clock",
                            "Dot Clock",
                            "Active Low Input",
                            "Active Low Output",
                        ],
                    )
                });
            }
            divider(ui);
            changed |= font_editor(ui, font, Some(&mut p.color));
        }
        K::Junction { radius_nm, .. } => {
            changed |= numeric_row(ui, "Size", radius_nm, units);
            row(ui, "Color", |ui| changed |= color(ui, &mut p.color));
        }
        K::Image {
            a,
            b,
            filename,
            keep_aspect,
        } => {
            changed |= dimensions(ui, a, b, units);
            row(ui, "File Name", |ui| readonly(ui, filename));
            row(ui, "", |ui| {
                changed |= ui.checkbox(keep_aspect, "Keep Aspect Ratio").changed()
            });
        }
        K::NoErc { .. } => {
            row(ui, "Color", |ui| changed |= color(ui, &mut p.color));
            row(ui, "Style", |ui| readonly(ui, "Cross"));
        }
        K::Ieee {
            scale_nm, mirrored, ..
        } => {
            changed |= numeric_row(ui, "Scale", scale_nm, units);
            changed |= ui.checkbox(mirrored, "Mirrored").changed();
        }
        K::Polyline { closed, .. } => {
            changed |= border_row(ui, &mut p.width_nm, &mut p.color);
            changed |= line_style(ui, &mut p.line_style);
            if *closed {
                changed |= fill_row(ui, &mut p.fill);
            }
        }
        K::Bezier { .. } => {
            changed |= border_row(ui, &mut p.width_nm, &mut p.color);
            changed |= line_style(ui, &mut p.line_style);
        }
        K::Unsupported { .. } => {
            ui.add_enabled(
                false,
                egui::Label::new("Properties are not supported for this record"),
            );
        }
    });
    if matches!(p.kind, K::PowerPort { .. } | K::Port { .. }) {
        net_sections(ui, query);
    }
    if matches!(p.kind, K::Polyline { .. } | K::Bezier { .. }) {
        changed |= vertices(ui, p, units, query);
    }
    changed
}
fn primitive_anchor(p: &SchPrimitive) -> Point {
    match &p.kind {
        K::Polyline { points, .. } => points.first().copied().unwrap_or_default(),
        K::Bezier { points } => points[0],
        K::Rectangle { a, .. }
        | K::SheetSymbol { a, .. }
        | K::TextFrame { a, .. }
        | K::Image { a, .. }
        | K::Unsupported { a, .. } => *a,
        K::Arc { centre, .. } => *centre,
        K::Text { position, .. }
        | K::SheetEntry { position, .. }
        | K::Pin { position, .. }
        | K::PowerPort { position, .. }
        | K::Port { position, .. }
        | K::Junction { position, .. }
        | K::NoErc { position }
        | K::Ieee { position, .. } => *position,
    }
}
fn dimensions(ui: &mut egui::Ui, a: &mut Point, b: &mut Point, units: Units) -> bool {
    let mut w = (b.x - a.x).abs();
    let mut h = (b.y - a.y).abs();
    let mut changed = false;
    changed |= numeric_row(ui, "Width", &mut w, units);
    changed |= numeric_row(ui, "Height", &mut h, units);
    if changed {
        b.x = a.x + w;
        b.y = a.y + h;
    }
    changed
}
fn width_combo(ui: &mut egui::Ui, width: &mut i64) -> bool {
    let values = [101_600, 254_000, 508_000, 1_016_000];
    let mut v = values.iter().position(|n| n == width).unwrap_or(4) as u32;
    let changed = combo(
        ui,
        "line_width",
        &mut v,
        &["Smallest", "Small", "Medium", "Large"],
    );
    if changed && v < 4 {
        *width = values[v as usize];
    }
    changed
}
fn border_row(ui: &mut egui::Ui, width: &mut i64, ink: &mut u32) -> bool {
    let mut changed = false;
    row(ui, "Border", |ui| {
        ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
            changed |= width_combo(ui, width)
        });
        changed |= color(ui, ink);
    });
    changed
}
fn line_style(ui: &mut egui::Ui, style: &mut u32) -> bool {
    let mut changed = false;
    row(ui, "Line Style", |ui| {
        changed |= combo(ui, "line_style", style, &["Solid", "Dashed", "Dotted"])
    });
    changed
}
fn fill_row(ui: &mut egui::Ui, fill: &mut Option<u32>) -> bool {
    let mut changed = false;
    row(ui, "Fill Color", |ui| {
        let mut on = fill.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *fill = if on { Some(0xa0ffff) } else { None };
            changed = true;
        }
        if let Some(value) = fill {
            changed |= color(ui, value);
        } else {
            ui.add_enabled_ui(false, |ui| {
                color(ui, &mut 0);
            });
        }
    });
    changed
}
fn vertices(ui: &mut egui::Ui, p: &mut SchPrimitive, units: Units, query: &str) -> bool {
    let mut changed = false;
    section(ui, "Vertices", query, |ui| {
        ui.horizontal(|ui| {
            ui.label("Width");
            ui.allocate_ui(egui::vec2(ui.available_width() - 26.0, 22.0), |ui| {
                changed |= width_combo(ui, &mut p.width_nm)
            });
            changed |= color(ui, &mut p.color);
        });
        ui.add_space(10.0);
        let can_add = matches!(p.kind, K::Polyline { .. });
        let mut remove = None;
        let points: &mut [Point] = match &mut p.kind {
            K::Polyline { points, .. } => points,
            K::Bezier { points } => points,
            _ => return,
        };
        egui::Grid::new("vertices")
            .num_columns(3)
            .spacing([4.0, 0.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Index");
                ui.label("X");
                ui.label("Y");
                ui.end_row();
                for (n, p) in points.iter_mut().enumerate() {
                    ui.add_sized([42.0, 20.0], egui::Label::new((n + 1).to_string()));
                    let w = (ui.available_width() - 4.0) / 2.0;
                    ui.allocate_ui(egui::vec2(w, 20.0), |ui| {
                        changed |= numeric(ui, &mut p.x, units)
                    });
                    ui.allocate_ui(egui::vec2(w, 20.0), |ui| {
                        changed |= numeric(ui, &mut p.y, units)
                    });
                    ui.end_row();
                }
            });
        let mut add = false;
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            if tool_button(ui, "trash", points.len() > 2 && can_add).clicked() {
                remove = Some(points.len() - 1);
            }
            add = ui
                .add_enabled(
                    can_add,
                    egui::Button::new("Add").min_size(egui::vec2(73.0, 24.0)),
                )
                .clicked();
        });
        if let K::Polyline { points, .. } = &mut p.kind {
            if let Some(n) = remove {
                points.remove(n);
                changed = true;
            }
            if add && let Some(last) = points.last().copied() {
                points.push(last);
                changed = true;
            }
        }
    });
    changed
}
fn net_sections(ui: &mut egui::Ui, query: &str) {
    // Compiled connectivity, net classes and rules are deliberately disabled until implemented.
    section(ui, "General (Net)", query, |ui| {
        row(ui, "Physical Name", |ui| {
            ui.add_enabled(false, egui::Label::new(""));
        });
        row(ui, "Net Name", |ui| {
            ui.add_enabled(false, egui::Label::new(""));
        });
        for (label, suffix) in [("Power Net", "V"), ("High Speed", "Hz")] {
            row(ui, label, |ui| {
                ui.add_enabled(false, egui::Checkbox::new(&mut false, ""));
                readonly(ui, "0.000");
                ui.label(suffix);
            });
        }
        row(ui, "Differential Pair", |ui| {
            ui.add_enabled(false, egui::Label::new("None"));
        });
    });
    section(ui, "Parameters (Net)", query, |ui| {
        ui.horizontal(|ui| {
            for label in ["All", "Parameters", "Rules", "Classes"] {
                ui.add_enabled(false, egui::Button::new(label).selected(true));
            }
        });
        egui::Frame::NONE
            .stroke(Stroke::new(1.0, theme::BORDER))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([ui.available_width() / 2.0, 20.0], egui::Label::new("Name"));
                    ui.label("Value");
                });
                for label in ["No Parameters", "No Rules", "No Classes"] {
                    ui.vertical_centered(|ui| {
                        ui.add_enabled(false, egui::Label::new(label));
                    });
                    ui.separator();
                }
                ui.add_space(157.0);
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    tool_button(ui, "trash", false);
                    tool_button(ui, "edit", false);
                    ui.add_enabled(false, egui::Button::new("Add…  ▾"));
                });
            });
    });
}
fn placement_options(ui: &mut egui::Ui, doc: &mut OpenDocument) {
    let editor = &mut doc.sch_editor;
    section(ui, "Properties", "", |ui| match editor.kind {
        Some(Placement::Resistor | Placement::Capacitor) => {
            text_row(ui, "Designator", &mut editor.designator);
            text_row(ui, "Comment", &mut editor.value);
        }
        Some(Placement::Text | Placement::NetLabel | Placement::Ground) => {
            text_row(ui, "Text", &mut editor.value);
        }
        _ => {}
    });
}
