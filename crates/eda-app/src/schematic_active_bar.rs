//! Schematic Active Bar. Command order comes from AD24.4.1's public UI resources.
use super::*;
use eda_core::{
    schematic::SchPrimitiveKind as K,
    schematic_arrange::{self as arrange, Align, Region},
    schematic_edit::{self as edit, Edit, Fragment, Selection},
};
use eda_geom::Point;
use egui::{Pos2, Rect, Sense, Stroke, Vec2};
#[path = "schematic_active_bar_icons.rs"]
mod icons;
#[path = "schematic_resources.rs"]
mod resources;
#[cfg(test)]
#[path = "schematic_active_bar_tests.rs"]
mod tests;
use icons::Glyph;
pub(super) use resources::resolve_device;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    None,
    Place(Placement),
    Power(usize),
    Part,
    Graphic,
    SimulationLibrary,
    DeviceSheet,
    Reuse,
    Move(bool, bool),
    Rotate(bool),
    Offset,
    Select(Region),
    All,
    Connection,
    Toggle,
    Align(Align),
    AlignDialog,
    Stack(bool, bool),
    Entries(u8),
    Flip(bool),
}
#[derive(Clone, Copy)]
struct Item {
    label: &'static str,
    action: Action,
    glyph: Glyph,
}
const fn item(label: &'static str, action: Action, glyph: Glyph) -> Item {
    Item {
        label,
        action,
        glyph,
    }
}
#[derive(Clone, Copy)]
struct Group {
    name: &'static str,
    items: &'static [Item],
}
const GROUPS: [Group; 14] = [
    Group {
        name: "Selection Filter",
        items: &[],
    },
    Group {
        name: "Move",
        items: &[
            item("Drag", Action::Move(true, false), Glyph::Move),
            item("Move", Action::Move(false, false), Glyph::Move),
            item("Move Selection", Action::Move(false, true), Glyph::Move),
            item("Move Selection by X, Y...", Action::Offset, Glyph::Move),
            item("Drag Selection", Action::Move(true, true), Glyph::Move),
            item("Move To Front", Action::Stack(true, false), Glyph::Move),
            item("Rotate Selection", Action::Rotate(false), Glyph::Move),
            item(
                "Rotate Selection Clockwise",
                Action::Rotate(true),
                Glyph::Move,
            ),
            item("", Action::None, Glyph::Move),
            item("Bring To Front", Action::Stack(true, false), Glyph::Move),
            item("Send To Back", Action::Stack(false, false), Glyph::Move),
            item("Bring To Front Of", Action::Stack(true, true), Glyph::Move),
            item("Send To Back Of", Action::Stack(false, true), Glyph::Move),
            item("", Action::None, Glyph::Move),
            item(
                "Flip Selected Sheet Symbols Along X",
                Action::Flip(false),
                Glyph::Move,
            ),
            item(
                "Flip Selected Sheet Symbols Along Y",
                Action::Flip(true),
                Glyph::Move,
            ),
            item(
                "Toggle All Sheet Entry IO Type In Selected Sheet Symbols",
                Action::Entries(0),
                Glyph::Sheet,
            ),
            item(
                "Reverse Selected Sheet Entries Order",
                Action::Entries(1),
                Glyph::Sheet,
            ),
            item(
                "Toggle Selected Sheet Entries IO Type",
                Action::Entries(2),
                Glyph::Sheet,
            ),
            item(
                "Swap Selected Sheet Entries Sides",
                Action::Entries(3),
                Glyph::Sheet,
            ),
        ],
    },
    Group {
        name: "Select",
        items: &[
            item("Lasso Select", Action::Select(Region::Lasso), Glyph::Select),
            item("Inside Area", Action::Select(Region::Inside), Glyph::Select),
            item(
                "Outside Area",
                Action::Select(Region::Outside),
                Glyph::Select,
            ),
            item(
                "Touching Rectangle",
                Action::Select(Region::Touching),
                Glyph::Select,
            ),
            item("Touching Line", Action::Select(Region::Line), Glyph::Select),
            item("All", Action::All, Glyph::Select),
            item("Connection", Action::Connection, Glyph::Select),
            item("", Action::None, Glyph::Select),
            item("Toggle Selection", Action::Toggle, Glyph::Select),
        ],
    },
    Group {
        name: "Align",
        items: &[
            item("Align...", Action::AlignDialog, Glyph::Align),
            item("", Action::None, Glyph::Align),
            item("Align Left", Action::Align(Align::Left), Glyph::Align),
            item("Align Right", Action::Align(Align::Right), Glyph::Align),
            item(
                "Align Horizontal Centers",
                Action::Align(Align::HorizontalCenters),
                Glyph::Align,
            ),
            item(
                "Distribute Horizontally",
                Action::Align(Align::DistributeHorizontal),
                Glyph::Align,
            ),
            item("", Action::None, Glyph::Align),
            item("Align Top", Action::Align(Align::Top), Glyph::Align),
            item("Align Bottom", Action::Align(Align::Bottom), Glyph::Align),
            item(
                "Align Vertical Centers",
                Action::Align(Align::VerticalCenters),
                Glyph::Align,
            ),
            item(
                "Distribute Vertically",
                Action::Align(Align::DistributeVertical),
                Glyph::Align,
            ),
            item("", Action::None, Glyph::Align),
            item("Align To Grid", Action::Align(Align::Grid), Glyph::Align),
        ],
    },
    Group {
        name: "Part",
        items: &[item("Part...", Action::Part, Glyph::Part)],
    },
    Group {
        name: "Nets",
        items: &[
            item("Wire", Action::Place(Placement::Wire), Glyph::Wire),
            item("Bus", Action::Place(Placement::Bus), Glyph::Bus),
            item(
                "Bus Entry",
                Action::Place(Placement::BusEntry),
                Glyph::BusEntry,
            ),
            item(
                "Net Label",
                Action::Place(Placement::NetLabel),
                Glyph::NetLabel,
            ),
        ],
    },
    Group {
        name: "Power Ports",
        items: &[
            item(
                "Place GND power port",
                Action::Power(0),
                Glyph::PowerPreset(0),
            ),
            item(
                "Place VCC power port",
                Action::Power(1),
                Glyph::PowerPreset(1),
            ),
            item(
                "Place +12 power port",
                Action::Power(2),
                Glyph::PowerPreset(2),
            ),
            item(
                "Place +5 power port",
                Action::Power(3),
                Glyph::PowerPreset(3),
            ),
            item(
                "Place -5 power port",
                Action::Power(4),
                Glyph::PowerPreset(4),
            ),
            item(
                "Place Arrow style power port",
                Action::Power(5),
                Glyph::PowerPreset(5),
            ),
            item(
                "Place Wave style power port",
                Action::Power(6),
                Glyph::PowerPreset(6),
            ),
            item(
                "Place Bar style power port",
                Action::Power(7),
                Glyph::PowerPreset(7),
            ),
            item(
                "Place Circle style power port",
                Action::Power(8),
                Glyph::PowerPreset(8),
            ),
            item(
                "Place Signal Ground power port",
                Action::Power(9),
                Glyph::PowerPreset(9),
            ),
            item(
                "Place Earth power port",
                Action::Power(10),
                Glyph::PowerPreset(10),
            ),
        ],
    },
    Group {
        name: "Harnesses",
        items: &[
            item(
                "Signal Harness",
                Action::Place(Placement::SignalHarness),
                Glyph::Harness,
            ),
            item(
                "Harness Connector",
                Action::Place(Placement::HarnessConnector),
                Glyph::HarnessConnector,
            ),
            item(
                "Harness Entry",
                Action::Place(Placement::HarnessEntry),
                Glyph::HarnessEntry,
            ),
        ],
    },
    Group {
        name: "Sheet Symbols",
        items: &[
            item(
                "Sheet Symbol",
                Action::Place(Placement::SheetSymbol),
                Glyph::Sheet,
            ),
            item(
                "Sheet Entry",
                Action::Place(Placement::SheetEntry),
                Glyph::SheetEntry,
            ),
            item(
                "Device Sheet Symbol",
                Action::DeviceSheet,
                Glyph::DeviceSheet,
            ),
            item("Reuse Block...", Action::Reuse, Glyph::Reuse),
        ],
    },
    Group {
        name: "Connectors",
        items: &[
            item("Port", Action::Place(Placement::Port), Glyph::Port),
            item(
                "Off Sheet Connector",
                Action::Place(Placement::OffSheetConnector),
                Glyph::OffSheet,
            ),
        ],
    },
    Group {
        name: "Directives",
        items: &[
            item(
                "Parameter Set",
                Action::Place(Placement::ParameterSet),
                Glyph::Directive,
            ),
            item(
                "Generic No ERC",
                Action::Place(Placement::NoErc),
                Glyph::NoErc,
            ),
            item(
                "Differential Pair",
                Action::Place(Placement::DifferentialPair),
                Glyph::Differential,
            ),
            item("", Action::None, Glyph::Directive),
            item("Blanket", Action::Place(Placement::Blanket), Glyph::Blanket),
            item(
                "Compile Mask",
                Action::Place(Placement::CompileMask),
                Glyph::Mask,
            ),
        ],
    },
    Group {
        name: "Text Tools",
        items: &[
            item("Text String", Action::Place(Placement::Text), Glyph::Text),
            item(
                "Text Frame",
                Action::Place(Placement::TextFrame),
                Glyph::Frame,
            ),
            item("Note", Action::Place(Placement::Note), Glyph::Note),
        ],
    },
    Group {
        name: "Drawing Tools",
        items: &[
            item("Arc", Action::Place(Placement::Arc), Glyph::Arc),
            item(
                "Full Circle",
                Action::Place(Placement::FullCircle),
                Glyph::Circle,
            ),
            item(
                "Elliptical Arc",
                Action::Place(Placement::EllipticalArc),
                Glyph::EllipticalArc,
            ),
            item("Ellipse", Action::Place(Placement::Ellipse), Glyph::Ellipse),
            item("Line", Action::Place(Placement::Line), Glyph::Line),
            item(
                "Rectangle",
                Action::Place(Placement::Rectangle),
                Glyph::Rectangle,
            ),
            item(
                "Round Rectangle",
                Action::Place(Placement::RoundRectangle),
                Glyph::Rounded,
            ),
            item("Polygon", Action::Place(Placement::Polygon), Glyph::Polygon),
            item("Bezier", Action::Place(Placement::Bezier), Glyph::Bezier),
            item("Graphic...", Action::Graphic, Glyph::Graphic),
        ],
    },
    Group {
        name: "Simulation Sources",
        items: &[
            item(
                "Voltage Source",
                Action::Place(Placement::VoltageSource),
                Glyph::Source,
            ),
            item(
                "Current Source",
                Action::Place(Placement::CurrentSource),
                Glyph::Source,
            ),
            item(
                "Simulation Generic Component",
                Action::SimulationLibrary,
                Glyph::Initial,
            ),
            item(
                "Voltage Probe",
                Action::Place(Placement::VoltageProbe),
                Glyph::Probe(0),
            ),
            item(
                "Voltage Diff Probe",
                Action::Place(Placement::VoltageDiffProbe),
                Glyph::Probe(1),
            ),
            item(
                "Current Probe",
                Action::Place(Placement::CurrentProbe),
                Glyph::Probe(2),
            ),
            item(
                "Power Probe",
                Action::Place(Placement::PowerProbe),
                Glyph::Probe(3),
            ),
            item(
                "Initial Condition",
                Action::Place(Placement::InitialCondition),
                Glyph::Initial,
            ),
        ],
    },
];
const FILTERS: [&str; 12] = [
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
pub(super) const POWER: [(&str, u32, u32); 11] = [
    ("GND", 4, 3),
    ("VCC", 2, 1),
    ("+12", 2, 1),
    ("+5", 2, 1),
    ("-5", 2, 3),
    ("VCC", 1, 1),
    ("VCC", 3, 1),
    ("VCC", 2, 1),
    ("VCC", 0, 1),
    ("GND", 5, 3),
    ("GND", 6, 3),
];

pub(super) enum Interaction {
    Region {
        kind: Region,
        points: Vec<Point>,
    },
    Connection,
    Stack {
        front: bool,
        relative: bool,
        source: Option<Uuid>,
    },
    Shape {
        kind: Placement,
        points: Vec<Point>,
    },
}
pub(super) enum Dialog {
    Offset {
        delta: Point,
        revision: u64,
    },
    Align {
        horizontal: usize,
        vertical: usize,
        grid: bool,
        revision: u64,
    },
}
pub(super) struct State {
    pub(super) resources: resources::State,
    last: [usize; 14],
    menu: Option<(usize, Pos2)>,
    press: Option<(usize, f64)>,
    held: bool,
    menu_focus: Option<usize>,
    pub(super) dialog: Option<Dialog>,
    pub(super) interaction: Option<Interaction>,
    pub(super) power: Option<usize>,
    pub(super) shape_turns: u32,
}
impl Default for State {
    fn default() -> Self {
        let mut last = [0; 14];
        last[2] = 1;
        Self {
            resources: Default::default(),
            last,
            menu: None,
            press: None,
            held: false,
            menu_focus: None,
            dialog: None,
            interaction: None,
            power: None,
            shape_turns: 0,
        }
    }
}
fn enabled(action: Action, doc: &OpenDocument) -> bool {
    let ids = selected(doc);
    let s = doc.model.schematic.as_ref().unwrap();
    match action {
        Action::None => false,
        Action::Align(Align::Grid) => !ids.is_empty(),
        Action::Align(_) | Action::AlignDialog => ids.len() >= 2,
        Action::Offset | Action::Move(_, true) => !ids.is_empty(),
        Action::Flip(_) => ids.iter().any(|id| {
            s.primitive(*id)
                .is_some_and(|p| matches!(p.kind, K::SheetSymbol { .. }))
        }),
        Action::Entries(0) => ids.iter().any(|id| {
            s.primitive(*id)
                .is_some_and(|p| matches!(p.kind, K::SheetSymbol { .. }))
        }),
        Action::Entries(_) => ids.iter().any(|id| {
            s.primitive(*id)
                .is_some_and(|p| matches!(p.kind, K::SheetEntry { .. }))
        }),
        _ => true,
    }
}
fn selected(doc: &OpenDocument) -> Vec<Uuid> {
    let mut ids = doc.view.sch_selection.clone();
    if let Some(id) = doc.view.selected {
        ids.push(id);
    }
    eda_core::schematic_arrange::roots(doc.model.schematic.as_ref().unwrap(), &ids)
}
fn category(p: &eda_core::schematic::SchPrimitive) -> usize {
    if p.component.is_some() {
        0
    } else if let Some(role) = p.text_role {
        match role {
            eda_core::schematic::TextRole::NetLabel => 5,
            _ => 6,
        }
    } else {
        match &p.kind {
            K::Polyline { electrical, .. } if electrical == "Wire" => 1,
            K::Polyline { electrical, .. } if electrical == "Bus" => 2,
            K::SheetSymbol { .. } => 3,
            K::SheetEntry { .. } => 4,
            K::Port { .. } => 7,
            K::PowerPort { .. } => 8,
            K::Text { .. } | K::TextFrame { .. } => 9,
            K::Junction { .. }
            | K::NoErc { .. }
            | K::ParameterSet { .. }
            | K::Blanket { .. }
            | K::CompileMask { .. } => 11,
            _ => 10,
        }
    }
}
fn select(doc: &mut OpenDocument, ids: Vec<Uuid>) {
    let s = doc.model.schematic.as_ref().unwrap();
    let ids: Vec<_> = ids
        .into_iter()
        .filter(|id| {
            s.components.iter().any(|c| c.id == *id) && doc.view.sch_filters[0]
                || s.primitive(*id)
                    .is_some_and(|p| doc.view.sch_filters[category(p)])
        })
        .collect();
    doc.view.sch_selection = ids;
    doc.view.selected = doc.view.sch_selection.first().copied();
    doc.view.sch_segments.clear();
}

impl PcbCraft {
    pub(super) fn schematic_bar_shortcuts(&mut self, index: usize, ctx: &egui::Context) -> bool {
        let state = &mut self.documents[index].sch_editor.bar;
        if state.resources.modal() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                state.resources.escape();
            }
            return true;
        }
        if state.menu.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                state.menu = None;
            }
            return true;
        }
        if state.dialog.is_some() {
            return true;
        }
        if !ctx.wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::A))
        {
            self.schematic_bar_action(index, Action::All, ctx);
            return true;
        }
        if let Some(Interaction::Shape { points, .. }) = &mut state.interaction {
            if ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::NONE, Key::Backspace)
                    || i.consume_key(egui::Modifiers::NONE, Key::Delete)
            }) {
                points.pop();
                self.documents[index].view.sch_preview = None;
                return true;
            }
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
                state.shape_turns = (state.shape_turns + 1) % 4;
                return true;
            }
        } else if state.interaction.is_some()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space))
        {
            return true;
        }
        false
    }
    pub(super) fn schematic_active_bar(&mut self, ctx: &egui::Context, canvas: Rect) {
        let Some(index) = self.active else {
            return;
        };
        let mut chosen = None;
        let mut opening = None;
        let modal = self.documents[index].sch_editor.bar.dialog.is_some()
            || self.documents[index].sch_editor.bar.resources.modal();
        if let Some(last) =
            ctx.data(|d| d.get_temp::<[usize; 14]>(egui::Id::new("schematic_bar_last_commands")))
        {
            self.documents[index].sch_editor.bar.last = last;
        }
        let bar_width = 476.0;
        let origin = egui::pos2(
            (canvas.center().x - bar_width / 2.0).max(canvas.left()),
            canvas.top() + 4.0,
        );
        let mut rectangles = vec![];
        egui::Area::new(egui::Id::new("active_bar"))
            .fade_in(false)
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(Color32::from_gray(58))
                    .corner_radius(5)
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            for (group, definition) in GROUPS.iter().enumerate() {
                                let doc = &self.documents[index];
                                let state = &doc.sch_editor.bar;
                                let entry = definition.items.get(state.last[group]);
                                let is_enabled =
                                    group == 0 || entry.is_some_and(|e| enabled(e.action, doc));
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(100.0 / 3.0, 100.0 / 3.0),
                                    Sense::click(),
                                );
                                rectangles.push(rect);
                                if response.hovered() || state.menu.is_some_and(|(g, _)| g == group)
                                {
                                    ui.painter().rect_filled(
                                        rect.shrink2(egui::vec2(1.0, 2.0)),
                                        2.0,
                                        Color32::from_rgb(65, 99, 125),
                                    );
                                }
                                icons::paint(
                                    ui.painter(),
                                    Rect::from_center_size(
                                        rect.center() - egui::vec2(0.0, 1.0),
                                        egui::vec2(22.0, 22.0),
                                    ),
                                    entry.map_or(Glyph::Filter, |e| e.glyph),
                                    is_enabled,
                                );
                                if definition.items.len() > 1 {
                                    let at = rect.right_bottom() - egui::vec2(4.0, 3.0);
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![
                                            at,
                                            at - egui::vec2(5.0, 0.0),
                                            at - egui::vec2(0.0, 5.0),
                                        ],
                                        Color32::WHITE,
                                        Stroke::NONE,
                                    ));
                                }
                                if group == 3 {
                                    ui.painter().line_segment(
                                        [
                                            rect.right_top() + egui::vec2(0.0, 5.0),
                                            rect.right_bottom() - egui::vec2(0.0, 5.0),
                                        ],
                                        Stroke::new(1.0, Color32::from_gray(38)),
                                    );
                                }
                                let state = &mut self.documents[index].sch_editor.bar;
                                if !modal
                                    && ctx.input(|i| {
                                        i.pointer.primary_down()
                                            && i.pointer
                                                .press_origin()
                                                .is_some_and(|at| rect.contains(at))
                                    })
                                {
                                    let (pressed_group, start) =
                                        *state.press.get_or_insert((group, ctx.input(|i| i.time)));
                                    if pressed_group == group
                                        && definition.items.len() > 1
                                        && !state.held
                                        && ctx.input(|i| i.time) - start >= 1.0
                                    {
                                        opening = Some((
                                            group,
                                            rect.left_bottom() + egui::vec2(0.0, 2.0),
                                        ));
                                        state.held = true;
                                    } else {
                                        ctx.request_repaint_after(
                                            std::time::Duration::from_millis(50),
                                        );
                                    }
                                }
                                if !modal
                                    && response.secondary_clicked()
                                    && definition.items.len() > 1
                                {
                                    opening =
                                        Some((group, rect.left_bottom() + egui::vec2(0.0, 2.0)));
                                }
                                if !modal && response.clicked() {
                                    if group == 0 {
                                        opening = Some((
                                            group,
                                            rect.left_bottom() + egui::vec2(0.0, 2.0),
                                        ));
                                    } else if !state.held
                                        && let Some(e) = entry
                                        && is_enabled
                                    {
                                        chosen = Some((group, state.last[group], e.action));
                                    }
                                }
                                response.on_hover_text(entry.map_or(definition.name, |e| e.label));
                            }
                            ui.add_space(16.0 / 3.0);
                        });
                    });
            });
        if !ctx.input(|i| i.pointer.primary_down()) {
            let state = &mut self.documents[index].sch_editor.bar;
            state.press = None;
            state.held = false;
        }
        if let Some(open) = opening {
            let doc = &mut self.documents[index];
            if doc
                .sch_editor
                .bar
                .menu
                .is_some_and(|(group, _)| group == open.0)
            {
                doc.sch_editor.bar.menu = None;
            } else {
                doc.sch_editor.bar.menu = Some(open);
                doc.sch_editor.bar.menu_focus = None;
            }
        }
        if let Some((group, position)) = self.documents[index].sch_editor.bar.menu {
            let mut close = false;
            let result = egui::Area::new(egui::Id::new("schematic_active_bar_menu"))
                .fade_in(false)
                .order(egui::Order::Foreground)
                .fixed_pos(position)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(Color32::from_gray(61))
                        .corner_radius(if group == 0 { 0 } else { 5 })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            if group == 0 {
                                egui::Frame::NONE.inner_margin(8).show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(305.0, 117.0));
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    ui.spacing_mut().button_padding = egui::vec2(9.0, 3.0);
                                    let view = &mut self.documents[index].view;
                                    let all = view.sch_filters.iter().all(|v| *v);
                                    if ui
                                        .add_sized([76.0, 24.0], egui::Button::new("All - On"))
                                        .clicked()
                                    {
                                        view.sch_filters.fill(!all);
                                    }
                                    for row in 0..3 {
                                        ui.horizontal(|ui| {
                                            for (n, label) in
                                                FILTERS.iter().enumerate().skip(row * 4).take(4)
                                            {
                                                if ui
                                                    .add(
                                                        egui::Button::new(*label)
                                                            .selected(view.sch_filters[n])
                                                            .min_size(egui::vec2(0.0, 24.0)),
                                                    )
                                                    .clicked()
                                                {
                                                    view.sch_filters[n] = !view.sch_filters[n];
                                                }
                                            }
                                        });
                                    }
                                });
                            } else {
                                let width = [
                                    321., 368., 161., 187., 100., 109., 227., 159., 170., 167.,
                                    141., 115., 148., 224.,
                                ][group];
                                let eligible: Vec<_> = GROUPS[group]
                                    .items
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(n, e)| {
                                        enabled(e.action, &self.documents[index]).then_some(n)
                                    })
                                    .collect();
                                let direction = ctx.input_mut(|i| {
                                    if i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) {
                                        1
                                    } else if i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) {
                                        -1
                                    } else {
                                        0
                                    }
                                });
                                if direction != 0 && !eligible.is_empty() {
                                    let old = self.documents[index]
                                        .sch_editor
                                        .bar
                                        .menu_focus
                                        .and_then(|n| eligible.iter().position(|v| *v == n));
                                    let next = old.map_or(
                                        if direction > 0 { 0 } else { eligible.len() - 1 },
                                        |n| {
                                            (n as isize + direction)
                                                .rem_euclid(eligible.len() as isize)
                                                as usize
                                        },
                                    );
                                    self.documents[index].sch_editor.bar.menu_focus =
                                        Some(eligible[next]);
                                }
                                for (n, e) in GROUPS[group].items.iter().enumerate() {
                                    if e.label.is_empty() {
                                        let (rect, _) = ui.allocate_exact_size(
                                            egui::vec2(width, 4.0 / 3.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().line_segment(
                                            [
                                                rect.left_center() + egui::vec2(36.0, 0.0),
                                                rect.right_center() - egui::vec2(4.0, 0.0),
                                            ],
                                            Stroke::new(1.0, Color32::from_gray(40)),
                                        );
                                        continue;
                                    }
                                    let is_enabled = enabled(e.action, &self.documents[index]);
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(width, 88.0 / 3.0),
                                        if is_enabled {
                                            Sense::click()
                                        } else {
                                            Sense::hover()
                                        },
                                    );
                                    if is_enabled
                                        && response.hovered()
                                        && ctx.input(|i| i.pointer.delta() != Vec2::ZERO)
                                    {
                                        self.documents[index].sch_editor.bar.menu_focus = Some(n);
                                    }
                                    let focus =
                                        self.documents[index].sch_editor.bar.menu_focus == Some(n);
                                    if is_enabled && (response.hovered() || focus) {
                                        ui.painter().rect_filled(
                                            rect.shrink2(egui::vec2(2.0, 0.0)),
                                            1.0,
                                            Color32::from_rgb(75, 111, 137),
                                        );
                                    }
                                    if let Some(glyph) = menu_glyph(group, n, e.glyph) {
                                        icons::paint(
                                            ui.painter(),
                                            Rect::from_center_size(
                                                rect.left_center() + egui::vec2(16.0, 0.0),
                                                egui::vec2(22.0, 22.0),
                                            ),
                                            glyph,
                                            is_enabled,
                                        );
                                    }
                                    ui.painter().text(
                                        rect.left_center() + egui::vec2(36.0, 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        e.label,
                                        egui::FontId::proportional(12.0),
                                        if is_enabled {
                                            Color32::from_gray(238)
                                        } else {
                                            Color32::from_gray(95)
                                        },
                                    );
                                    if is_enabled
                                        && (response.clicked()
                                            || focus
                                                && ctx.input_mut(|i| {
                                                    i.consume_key(egui::Modifiers::NONE, Key::Enter)
                                                }))
                                    {
                                        chosen = Some((group, n, e.action));
                                        close = true;
                                    }
                                }
                            }
                        });
                });
            if ctx.input(|i| {
                i.key_pressed(Key::Escape)
                    || i.pointer.any_pressed()
                        && !result
                            .response
                            .rect
                            .contains(i.pointer.interact_pos().unwrap_or_default())
                        && !rectangles
                            .iter()
                            .any(|r| r.contains(i.pointer.interact_pos().unwrap_or_default()))
            }) {
                close = true;
            }
            if close {
                self.documents[index].sch_editor.bar.menu = None;
            }
        }
        if let Some((g, n, action)) = chosen {
            self.documents[index].sch_editor.bar.last[g] = n;
            ctx.data_mut(|d| {
                d.insert_temp(
                    egui::Id::new("schematic_bar_last_commands"),
                    self.documents[index].sch_editor.bar.last,
                )
            });
            self.documents[index].sch_editor.bar.menu = None;
            self.schematic_bar_action(index, action, ctx);
        }
        self.schematic_bar_dialog(ctx, index);
        self.schematic_resource_windows(ctx, index);
        if let Some(Interaction::Region { kind, points }) =
            &self.documents[index].sch_editor.bar.interaction
            && let Some(first) = points.first()
        {
            let camera = &self.documents[index].view.camera;
            let start = camera.screen(*first, canvas);
            let at = ctx.input(|i| i.pointer.hover_pos()).unwrap_or(start);
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("schematic_selection_region"),
            ));
            if *kind == Region::Lasso {
                let mut path: Vec<_> = points.iter().map(|p| camera.screen(*p, canvas)).collect();
                path.push(at);
                painter.add(egui::Shape::line(path, Stroke::new(1.0, theme::ACCENT)));
            } else if *kind == Region::Line {
                painter.line_segment([start, at], Stroke::new(1.0, theme::ACCENT));
            } else {
                painter.rect_stroke(
                    Rect::from_two_pos(start, at),
                    0.0,
                    Stroke::new(1.0, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
        }
    }

    pub(super) fn schematic_bar_action(
        &mut self,
        index: usize,
        action: Action,
        ctx: &egui::Context,
    ) {
        match action {
            Action::Part => {
                self.schematic_action(SchAction::Select, ctx);
                self.command = Some(Command::ShowComponents);
                return;
            }
            Action::Graphic => {
                self.schematic_graphic_tool(index);
                return;
            }
            Action::SimulationLibrary => {
                self.schematic_action(SchAction::Select, ctx);
                self.show_simulation_components();
                return;
            }
            Action::DeviceSheet => {
                self.schematic_action(SchAction::Select, ctx);
                self.documents[index].sch_editor.bar.resources.open_device();
                return;
            }
            Action::Reuse => {
                self.documents[index].sch_editor.bar.resources.open_reuse();
                return;
            }
            Action::Place(kind) => {
                self.start_schematic_tool(kind);
                return;
            }
            Action::Power(n) => {
                self.start_schematic_tool(Placement::Ground);
                let doc = &mut self.documents[index];
                doc.sch_editor.value = POWER[n].0.into();
                doc.sch_editor.bar.power = Some(n);
                return;
            }
            Action::Move(connected, selection) => {
                if !selection {
                    let view = &mut self.documents[index].view;
                    view.selected = None;
                    view.sch_selection.clear();
                    view.sch_segments.clear();
                }
                self.schematic_action(
                    if connected {
                        SchAction::Drag
                    } else {
                        SchAction::Move
                    },
                    ctx,
                );
                return;
            }
            _ => {}
        }
        let ids = selected(&self.documents[index]);
        let doc = &mut self.documents[index];
        let revision = doc.history.revision;
        let result = (|| -> Result<(), String> {
            match action {
                Action::None => {}
                Action::Part
                | Action::SimulationLibrary
                | Action::DeviceSheet
                | Action::Reuse
                | Action::Graphic
                | Action::Place(_)
                | Action::Power(_)
                | Action::Move(_, _) => {
                    unreachable!()
                }
                Action::Rotate(clockwise) => {
                    if ids.is_empty() {
                        return Ok(());
                    }
                    let s = doc.model.schematic.as_ref().unwrap();
                    let anchor = edit::selection_anchor(
                        s,
                        &Selection {
                            ids: ids.clone(),
                            segments: vec![],
                        },
                    )
                    .ok_or("Select objects to rotate")?;
                    doc.edit_schematic(
                        Edit::TransformSelection {
                            selection: Selection {
                                ids,
                                segments: vec![],
                            },
                            pivot: anchor,
                            quarter_turns: if clockwise { 3 } else { 1 },
                            delta: Point::default(),
                            connected: false,
                        },
                        revision,
                    )?;
                }
                Action::Offset => {
                    doc.sch_editor.bar.dialog = Some(Dialog::Offset {
                        delta: Point::default(),
                        revision,
                    })
                }
                Action::AlignDialog => {
                    doc.sch_editor.bar.dialog = Some(Dialog::Align {
                        horizontal: 0,
                        vertical: 0,
                        grid: false,
                        revision,
                    })
                }
                Action::Align(mode) => {
                    let command =
                        arrange::align(doc.model.schematic.as_ref().unwrap(), &ids, mode)?;
                    doc.edit_schematic(command, revision)?;
                }
                Action::Select(kind) => {
                    doc.sch_editor.cancel(&mut doc.view);
                    doc.sch_editor.bar.interaction = Some(Interaction::Region {
                        kind,
                        points: vec![],
                    });
                    doc.view.sch_tool_active = true;
                }
                Action::Connection => {
                    doc.sch_editor.cancel(&mut doc.view);
                    doc.sch_editor.bar.interaction = Some(Interaction::Connection);
                    doc.view.sch_tool_active = true;
                }
                Action::All | Action::Toggle => {
                    let s = doc.model.schematic.as_ref().unwrap();
                    let mut all = arrange::select_region(
                        s,
                        &[
                            Point {
                                x: -100_000_000_000,
                                y: -100_000_000_000,
                            },
                            Point {
                                x: 100_000_000_000,
                                y: 100_000_000_000,
                            },
                        ],
                        Region::Inside,
                    );
                    if action == Action::Toggle {
                        all.retain(|id| !ids.contains(id));
                    }
                    select(doc, all);
                }
                Action::Stack(front, relative) => {
                    doc.sch_editor.cancel(&mut doc.view);
                    doc.sch_editor.bar.interaction = Some(Interaction::Stack {
                        front,
                        relative,
                        source: None,
                    });
                    doc.view.sch_tool_active = true;
                }
                Action::Flip(vertical) => {
                    let command = arrange::flip_symbols(
                        doc.model.schematic.as_ref().unwrap(),
                        &ids,
                        vertical,
                    )?;
                    doc.edit_schematic(command, revision)?;
                }
                Action::Entries(op) => {
                    let command =
                        arrange::entry_actions(doc.model.schematic.as_ref().unwrap(), &ids, op)?;
                    doc.edit_schematic(command, revision)?;
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.message(true, e);
        }
    }

    pub(super) fn schematic_bar_input(
        &mut self,
        index: usize,
        input: &eda_render::SchematicInput,
        ctx: &egui::Context,
    ) -> bool {
        let doc = &mut self.documents[index];
        if doc.sch_editor.paused
            || doc.sch_editor.bar.resources.modal()
            || doc.sch_editor.bar.menu.is_some()
            || doc.sch_editor.bar.dialog.is_some()
        {
            return true;
        }
        let Some(mut interaction) = doc.sch_editor.bar.interaction.take() else {
            return false;
        };
        if input.secondary_clicked
            && let Interaction::Shape { kind, points } = &mut interaction
            && ((*kind == Placement::Polygon && points.len() >= 3)
                || (*kind == Placement::Bezier && points.len() >= 4))
        {
            let result = arrange::shape(
                doc.model.schematic.as_ref().unwrap(),
                *kind,
                points,
                &doc.sch_editor.value,
            )
            .and_then(|f| {
                f.transformed(points[0], doc.sch_editor.bar.shape_turns, Point::default())
            })
            .and_then(|fragment| doc.edit_schematic(Edit::Insert(fragment), doc.history.revision));
            points.clear();
            doc.view.sch_preview = None;
            doc.sch_editor.bar.interaction = Some(interaction);
            if let Err(e) = result {
                self.message(true, e);
            }
            return true;
        }
        if input.secondary_clicked || ctx.input(|i| !i.focused || i.key_pressed(Key::Escape)) {
            doc.sch_editor.cancel(&mut doc.view);
            return true;
        }
        let s = doc.model.schematic.as_ref().unwrap();
        let mut keep = true;
        let mut error = None;
        match &mut interaction {
            Interaction::Region { kind, points } => {
                if points.is_empty() {
                    if (input.clicked || input.drag_started)
                        && let Some(at) = input.press_origin.or(input.point)
                    {
                        points.push(at);
                    }
                } else if *kind == Region::Lasso
                    && ctx.input(|i| i.pointer.primary_down())
                    && let Some(at) = input.point
                    && points.last() != Some(&at)
                {
                    points.push(at);
                }
                if !points.is_empty()
                    && (input.drag_stopped
                        || input.clicked && input.point != points.first().copied())
                {
                    let mut path = points.clone();
                    if let Some(at) = input.point {
                        path.push(at);
                    }
                    let mut ids = arrange::select_region(s, &path, *kind);
                    if ctx.input(|i| i.modifiers.shift) {
                        ids.extend(selected(doc));
                    }
                    select(doc, ids);
                    keep = false;
                }
            }
            Interaction::Connection => {
                if input.clicked
                    && let Some(id) = input.hit
                {
                    let ids = eda_core::cross_probe::connected_objects(s, &[id]);
                    select(doc, ids);
                    keep = false;
                }
            }
            Interaction::Stack {
                front,
                relative,
                source,
            } => {
                if input.clicked
                    && let Some(id) = input.hit
                {
                    if *relative && source.is_none() {
                        *source = Some(id);
                    } else {
                        let selected = source.unwrap_or(id);
                        let target = relative.then_some(id);
                        if let Err(e) = doc.edit_schematic(
                            Edit::Reorder {
                                ids: vec![selected],
                                target,
                                front: *front,
                            },
                            doc.history.revision,
                        ) {
                            error = Some(e);
                        }
                        keep = false;
                    }
                }
            }
            Interaction::Shape { kind, points } => {
                if let Some(raw) = input.point {
                    let at =
                        edit::snap_point(s, raw, s.sheet.snap_nm, s.sheet.options.snap_distance_nm);
                    let enter = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                    let anchor = input.clicked || enter;
                    if anchor {
                        points.push(at);
                    }
                    let mut path = points.clone();
                    if !anchor {
                        path.push(at);
                    }
                    let complete = if *kind == Placement::Polygon {
                        points.len() >= 3 && enter
                    } else if *kind == Placement::Bezier {
                        points.len() >= 4 && enter
                    } else if *kind == Placement::Arc {
                        points.len() >= 4
                    } else if matches!(kind, Placement::Ellipse | Placement::EllipticalArc) {
                        points.len() >= 3
                    } else {
                        points.len() >= 2
                    };
                    if let Ok(fragment) = arrange::shape(s, *kind, &path, &doc.sch_editor.value)
                        .and_then(|f| {
                            f.transformed(path[0], doc.sch_editor.bar.shape_turns, Point::default())
                        })
                    {
                        let preview = SchematicPreview::from_fragment(s, &fragment);
                        doc.view.sch_preview = Some(preview.0);
                        if complete {
                            if let Err(e) =
                                doc.edit_schematic(Edit::Insert(fragment), doc.history.revision)
                            {
                                error = Some(e);
                            }
                            points.clear();
                            doc.view.sch_preview = None;
                        }
                    }
                }
            }
        }
        if keep {
            doc.sch_editor.bar.interaction = Some(interaction);
        } else {
            doc.sch_editor.cancel(&mut doc.view);
        }
        if let Some(e) = error {
            self.message(true, e);
        }
        true
    }

    fn schematic_bar_dialog(&mut self, ctx: &egui::Context, index: usize) {
        let Some(mut dialog) = self.documents[index].sch_editor.bar.dialog.take() else {
            return;
        };
        let mut cancel = false;
        let mut apply = false;
        egui::Window::new(match dialog {
            Dialog::Offset { .. } => "Move Selection by X, Y",
            Dialog::Align { .. } => "Align Objects",
        })
        .id(egui::Id::new("schematic_arrange_dialog"))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            match &mut dialog {
                Dialog::Offset { delta, .. } => {
                    for (label, value) in [("X", &mut delta.x), ("Y", &mut delta.y)] {
                        ui.horizontal(|ui| {
                            ui.label(label);
                            let mut number = *value as f64 / 1e6;
                            if ui
                                .add(egui::DragValue::new(&mut number).speed(0.1).suffix(" mm"))
                                .changed()
                            {
                                *value = (number * 1e6).round() as i64;
                            }
                        });
                    }
                }
                Dialog::Align {
                    horizontal,
                    vertical,
                    grid,
                    ..
                } => {
                    ui.horizontal_top(|ui| {
                        ui.vertical(|ui| {
                            ui.strong("Horizontal Alignment");
                            for (n, label) in
                                ["No Change", "Left", "Center", "Right", "Distribute equally"]
                                    .iter()
                                    .enumerate()
                            {
                                ui.radio_value(horizontal, n, *label);
                            }
                        });
                        ui.vertical(|ui| {
                            ui.strong("Vertical Alignment");
                            for (n, label) in
                                ["No Change", "Top", "Center", "Bottom", "Distribute equally"]
                                    .iter()
                                    .enumerate()
                            {
                                ui.radio_value(vertical, n, *label);
                            }
                        });
                    });
                    ui.checkbox(grid, "Move primitives to grid");
                }
            }
            ui.horizontal(|ui| {
                apply = ui.button("OK").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
        apply |= ctx.input(|i| i.key_pressed(Key::Enter));
        if apply {
            let doc = &mut self.documents[index];
            let ids = selected(doc);
            let result = (|| -> Result<(), String> {
                let s = doc.model.schematic.as_ref().unwrap();
                let (revision, command) = match dialog {
                    Dialog::Offset { delta, revision } => {
                        let pivot = edit::selection_anchor(
                            s,
                            &Selection {
                                ids: ids.clone(),
                                segments: vec![],
                            },
                        )
                        .ok_or("Selection no longer exists")?;
                        (
                            revision,
                            Edit::TransformSelection {
                                selection: Selection {
                                    ids,
                                    segments: vec![],
                                },
                                pivot,
                                quarter_turns: 0,
                                delta,
                                connected: false,
                            },
                        )
                    }
                    Dialog::Align {
                        horizontal,
                        vertical,
                        grid,
                        revision,
                    } => {
                        let mut scratch = doc.model.clone();
                        let mut commands = vec![];
                        for mode in [
                            [
                                None,
                                Some(Align::Left),
                                Some(Align::HorizontalCenters),
                                Some(Align::Right),
                                Some(Align::DistributeHorizontal),
                            ][horizontal],
                            [
                                None,
                                Some(Align::Top),
                                Some(Align::VerticalCenters),
                                Some(Align::Bottom),
                                Some(Align::DistributeVertical),
                            ][vertical],
                            grid.then_some(Align::Grid),
                        ]
                        .into_iter()
                        .flatten()
                        {
                            let command =
                                arrange::align(scratch.schematic.as_ref().unwrap(), &ids, mode)?;
                            scratch = edit::preview(&scratch, &command)?;
                            commands.push(command);
                        }
                        (revision, Edit::Batch(commands))
                    }
                };
                doc.edit_schematic(command, revision)?;
                Ok(())
            })();
            if let Err(e) = result {
                self.message(true, e);
            }
        } else if !cancel {
            self.documents[index].sch_editor.bar.dialog = Some(dialog);
        }
    }
}

struct SchematicPreview(eda_core::schematic::Schematic);
impl SchematicPreview {
    fn from_fragment(s: &eda_core::schematic::Schematic, f: &Fragment) -> Self {
        let mut preview = eda_core::schematic::Schematic::blank();
        preview.fonts = s.fonts.clone();
        preview.primitives = f.primitives.clone();
        Self(preview)
    }
}

fn menu_glyph(group: usize, n: usize, default: Glyph) -> Option<Glyph> {
    match group {
        1 => match n {
            2 | 3 => Some(Glyph::Move),
            9..=12 => Some(Glyph::Stack((n - 9) as u8)),
            _ => None,
        },
        2 => (n == 1).then_some(Glyph::Select),
        3 => match n {
            2..=5 => Some(Glyph::Alignment((n - 2) as u8)),
            7..=10 => Some(Glyph::Alignment((n - 3) as u8)),
            12 => Some(Glyph::Alignment(8)),
            _ => None,
        },
        _ => Some(default),
    }
}
