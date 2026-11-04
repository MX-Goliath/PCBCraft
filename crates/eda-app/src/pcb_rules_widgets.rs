//! Observed AD 24.4.1 dialog vocabulary and drawing; no document mutation.
use super::*;
use egui::{Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RulePage {
    Root,
    Category(String),
    Kind(String),
    Rule,
}

// AD24 tree order, distinct from the list of executable core constraints.
pub(super) const CATALOG: &[(&str, &str, &str)] = &[
    ("Electrical", "Clearance", "Clearance"),
    ("Electrical", "Short-Circuit", "ShortCircuit"),
    ("Electrical", "Un-Routed Net", "UnRoutedNet"),
    ("Electrical", "Un-Connected Pin", "UnConnectedPin"),
    ("Electrical", "Modified Polygon", "UnpouredPolygon"),
    ("Electrical", "Creepage Distance", "Creepage"),
    ("Routing", "Width", "Width"),
    ("Routing", "Routing Topology", "RoutingTopology"),
    ("Routing", "Routing Priority", "RoutingPriority"),
    ("Routing", "Routing Layers", "RoutingLayers"),
    ("Routing", "Routing Corners", "RoutingCorners"),
    ("Routing", "Routing Via Style", "RoutingVias"),
    ("Routing", "Fanout Control", "FanoutControl"),
    ("Routing", "Differential Pairs Routing", "DiffPairsRouting"),
    ("SMT", "SMD To Corner", "SMDToCorner"),
    ("SMT", "SMD To Plane", "SMDToPlane"),
    ("SMT", "SMD Neck-Down", "SMDNeckDown"),
    ("SMT", "SMD Entry", "SMDEntry"),
    ("Mask", "Solder Mask Expansion", "SolderMaskExpansion"),
    ("Mask", "Paste Mask Expansion", "PasteMaskExpansion"),
    ("Plane", "Power Plane Connect Style", "PlaneConnect"),
    ("Plane", "Power Plane Clearance", "PlaneClearance"),
    ("Plane", "Polygon Connect Style", "PolygonConnect"),
    (
        "Testpoint",
        "Fabrication Testpoint Style",
        "FabricationTestpoint",
    ),
    (
        "Testpoint",
        "Fabrication Testpoint Usage",
        "FabricationTestPointUsage",
    ),
    ("Testpoint", "Assembly Testpoint Style", "AssemblyTestpoint"),
    (
        "Testpoint",
        "Assembly Testpoint Usage",
        "AssemblyTestPointUsage",
    ),
    (
        "Manufacturing",
        "Minimum Annular Ring",
        "MinimumAnnularRing",
    ),
    ("Manufacturing", "Acute Angle", "AcuteAngle"),
    ("Manufacturing", "Hole Size", "HoleSize"),
    ("Manufacturing", "Layer Pairs", "LayerPairs"),
    (
        "Manufacturing",
        "Hole To Hole Clearance",
        "HoleToHoleClearance",
    ),
    (
        "Manufacturing",
        "Minimum Solder Mask Sliver",
        "MinimumSolderMaskSliver",
    ),
    (
        "Manufacturing",
        "Silk To Solder Mask Clearance",
        "SilkToSolderMaskClearance",
    ),
    (
        "Manufacturing",
        "Silk To Silk Clearance",
        "SilkToSilkClearance",
    ),
    ("Manufacturing", "Net Antennae", "NetAntennae"),
    (
        "Manufacturing",
        "Board Outline Clearance",
        "BoardOutlineClearance",
    ),
    ("High Speed", "Parallel Segment", "ParallelSegment"),
    ("High Speed", "Length", "Length"),
    ("High Speed", "Return Path", "ReturnPath"),
    ("High Speed", "Matched Lengths", "MatchedLengths"),
    (
        "High Speed",
        "Daisy Chain Stub Length",
        "DaisyChainStubLength",
    ),
    ("High Speed", "Vias Under SMD", "ViasUnderSMD"),
    ("High Speed", "Maximum Via Count", "MaximumViaCount"),
    (
        "High Speed",
        "Max Via Stub Length (Back Drilling)",
        "MaxViaStubLength",
    ),
    ("Placement", "Room Definition", "RoomDefinition"),
    ("Placement", "Component Clearance", "ComponentClearance"),
    (
        "Placement",
        "Component Orientations",
        "ComponentOrientations",
    ),
    ("Placement", "Permitted Layers", "PermittedLayers"),
    ("Placement", "Nets to Ignore", "NetsToIgnore"),
    ("Placement", "Height", "Height"),
    ("Signal Integrity", "Signal Stimulus", "SignalStimulus"),
    (
        "Signal Integrity",
        "Overshoot - Falling Edge",
        "OvershootFallingEdge",
    ),
    (
        "Signal Integrity",
        "Overshoot - Rising Edge",
        "OvershootRisingEdge",
    ),
    (
        "Signal Integrity",
        "Undershoot - Falling Edge",
        "UndershootFallingEdge",
    ),
    (
        "Signal Integrity",
        "Undershoot - Rising Edge",
        "UndershootRisingEdge",
    ),
    ("Signal Integrity", "Impedance", "Impedance"),
    ("Signal Integrity", "Signal Top Value", "SignalTopValue"),
    ("Signal Integrity", "Signal Base Value", "SignalBaseValue"),
    (
        "Signal Integrity",
        "Flight Time - Rising Edge",
        "FlightTimeRisingEdge",
    ),
    (
        "Signal Integrity",
        "Flight Time - Falling Edge",
        "FlightTimeFallingEdge",
    ),
    ("Signal Integrity", "Slope - Rising Edge", "SlopeRisingEdge"),
    (
        "Signal Integrity",
        "Slope - Falling Edge",
        "SlopeFallingEdge",
    ),
    ("Signal Integrity", "Supply Nets", "SupplyNets"),
];

pub(super) const BG: Color32 = Color32::from_gray(63);
pub(super) const LINE: Color32 = Color32::from_gray(43);
pub(super) const HEAD: Color32 = Color32::from_gray(77);
pub(super) const GOLD: Color32 = Color32::from_rgb(226, 184, 31);
pub(super) const DIM: Color32 = Color32::from_rgb(155, 209, 208);

/// Fixed dialog regions must not move the parent's layout cursor. In particular,
/// short content must never pull the bottom action row into the editing area.
pub(super) fn child(
    ui: &mut egui::Ui,
    mut builder: egui::UiBuilder,
    f: impl FnOnce(&mut egui::Ui),
) {
    if builder.id_salt.is_none() {
        builder = builder.id_salt(ui.next_auto_id());
    }
    let mut child = ui.new_child(builder.layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(ui.clip_rect().intersect(child.max_rect()));
    f(&mut child);
}

pub(super) fn style(ui: &mut egui::Ui) {
    let s = ui.style_mut();
    s.spacing.item_spacing = vec2(4.0, 3.0);
    s.spacing.button_padding = vec2(7.0, 3.0);
    s.spacing.interact_size = vec2(20.0, 21.0);
    s.spacing.icon_width = 18.0;
    s.spacing.icon_width_inner = 12.0;
    s.visuals.widgets.inactive.bg_fill = Color32::from_gray(77);
    s.visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(77);
    s.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    s.visuals.extreme_bg_color = Color32::from_gray(44);
    s.visuals.faint_bg_color = Color32::from_gray(61);
    s.visuals.selection.bg_fill = Color32::from_rgb(100, 137, 165);
    s.visuals.window_corner_radius = 0.into();
    s.visuals.window_shadow = egui::epaint::Shadow::NONE;
}

pub(super) fn section(ui: &mut egui::Ui, title: &str) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, HEAD);
    let family = egui::FontFamily::Name("ui-bold".into());
    let family = if ui.fonts(|f| f.families().contains(&family)) {
        family
    } else {
        egui::FontFamily::Proportional
    };
    cell(ui, r.shrink2(vec2(2.0, 0.0)), |ui| {
        ui.label(
            egui::RichText::new(title)
                .font(FontId::new(12.0, family))
                .color(Color32::from_gray(235)),
        );
    });
}

pub(super) fn position(ctx: &egui::Context, id: egui::Id, default: Pos2) -> Pos2 {
    ctx.data(|d| d.get_temp::<Pos2>(id.with("dialog_position")))
        .or_else(|| egui::AreaState::load(ctx, id).map(|s| s.left_top_pos()))
        .unwrap_or(default)
}
pub(super) fn title(ui: &mut egui::Ui, label: &str) -> bool {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 23.0), Sense::hover());
    ui.painter().text(
        r.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::from_gray(235),
    );
    let close = Rect::from_min_size(pos2(r.right() - 32.0, r.top() - 3.0), vec2(32.0, 23.0));
    // Native dialogs move only by their caption, never by clicks in the tree
    // or constraint illustration. A titleless egui Window otherwise drags its
    // whole interior, which competes with these custom child regions.
    let drag = ui.interact(
        Rect::from_min_max(r.min, close.left_bottom()),
        ui.id().with("dialog_drag"),
        Sense::drag(),
    );
    if drag.dragged() {
        let window = ui.layer_id().id;
        if let Some(rect) = ui.ctx().memory(|m| m.area_rect(window)) {
            let next = rect.min + drag.drag_delta();
            ui.ctx()
                .data_mut(|d| d.insert_temp(window.with("dialog_position"), next));
        }
    }
    let response = ui.interact(close, ui.id().with("dialog_close"), Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(close, 0.0, Color32::from_rgb(195, 0, 0));
    }
    let c = close.center();
    for sign in [-1.0, 1.0] {
        ui.painter().line_segment(
            [c + vec2(-4.0, -4.0 * sign), c + vec2(4.0, 4.0 * sign)],
            Stroke::new(1.0, Color32::WHITE),
        );
    }
    response.clicked()
}

pub(super) fn icon(p: &egui::Painter, rect: Rect, category: &str) {
    let c = rect.center();
    let gray = Color32::from_gray(183);
    let line = Stroke::new(1.2, gray);
    match category {
        "Design Rules" => {
            p.rect_filled(
                Rect::from_center_size(c, vec2(17.0, 14.0)),
                1.0,
                Color32::from_rgb(237, 219, 150),
            );
            for x in [-4.0, 4.0] {
                let tip = c + vec2(x, 1.0);
                p.line_segment(
                    [tip + vec2(-3.0, 0.0), tip + vec2(3.0, 0.0)],
                    Stroke::new(2.0, Color32::from_rgb(40, 161, 88)),
                );
                p.line_segment(
                    [tip + vec2(0.0, -3.0), tip + vec2(0.0, 3.0)],
                    Stroke::new(2.0, Color32::from_rgb(40, 161, 88)),
                );
            }
        }
        "Mask" => {
            let r = Rect::from_center_size(c, vec2(16.0, 11.0));
            p.rect_filled(r, 3.0, gray);
            p.rect_stroke(
                r,
                3.0,
                Stroke::new(1.0, Color32::from_gray(227)),
                StrokeKind::Inside,
            );
        }
        "SMT" => {
            for x in [-4.0, 4.0] {
                p.rect_filled(
                    Rect::from_center_size(c + vec2(x, 0.0), vec2(9.0, 9.0)),
                    4.0,
                    gray,
                );
            }
        }
        "Electrical" => {
            p.circle_stroke(
                c + vec2(0.0, 3.0),
                3.0,
                Stroke::new(1.4, Color32::from_rgb(65, 151, 113)),
            );
            p.circle_stroke(c + vec2(0.0, -5.0), 2.5, line);
            p.line_segment([c + vec2(-7.0, -1.0), c + vec2(7.0, -1.0)], line);
        }
        "Routing" | "High Speed" => {
            for (dy, dx) in [(-4.0, 0.0), (3.0, -3.0)] {
                p.line_segment([c + vec2(-8.0, dy), c + vec2(8.0, dy)], line);
                p.circle_filled(c + vec2(dx, dy), 2.5, BG);
                p.circle_stroke(c + vec2(dx, dy), 2.5, line);
            }
        }
        "Plane" => {
            p.rect_filled(rect.shrink(1.0), 0.0, gray);
            for a in [0.0_f32, 90.0] {
                let v = vec2(a.to_radians().cos(), a.to_radians().sin()) * 7.0;
                p.line_segment([c - v, c + v], Stroke::new(2.0, Color32::WHITE));
            }
            p.circle_filled(c, 2.0, BG);
        }
        "Manufacturing" => {
            p.line_segment(
                [c + vec2(-7.0, -5.0), c + vec2(7.0, -5.0)],
                Stroke::new(4.0, Color32::from_rgb(104, 174, 153)),
            );
            p.line_segment(
                [c + vec2(7.0, -5.0), c + vec2(-5.0, 7.0)],
                Stroke::new(4.0, Color32::from_rgb(237, 210, 114)),
            );
        }
        "Testpoint" => {
            p.line_segment(
                [c + vec2(-6.0, 6.0), c + vec2(6.0, -6.0)],
                Stroke::new(4.0, Color32::from_rgb(27, 136, 198)),
            );
            p.line_segment(
                [c + vec2(-7.0, -4.0), c + vec2(4.0, -4.0)],
                Stroke::new(2.0, Color32::GREEN),
            );
        }
        "Placement" => {
            for x in [-4.0, 4.0] {
                p.rect_filled(
                    Rect::from_center_size(c + vec2(x, 0.0), vec2(6.0, 14.0)),
                    0.0,
                    gray,
                );
            }
        }
        _ => {
            p.rect_filled(
                Rect::from_min_size(rect.min + vec2(0.0, 4.0), vec2(17.0, 9.0)),
                2.0,
                gray,
            );
            p.rect_filled(
                Rect::from_min_size(rect.min + vec2(1.0, 2.0), vec2(8.0, 5.0)),
                1.0,
                gray,
            );
        }
    }
}

pub(super) fn tree_row(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    level: usize,
    cat: &str,
    selected: bool,
    branch: Option<bool>,
) -> (bool, bool, egui::Response) {
    tree_row_impl(ui, id, label, level, cat, selected, (branch, true))
}

pub(super) fn checker_tree_row(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    level: usize,
    cat: &str,
    selected: bool,
) -> bool {
    tree_row_impl(ui, id, label, level, cat, selected, (None, false)).0
}

fn tree_row_impl(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    level: usize,
    cat: &str,
    selected: bool,
    branch_gutter: (Option<bool>, bool),
) -> (bool, bool, egui::Response) {
    let (branch, reserve_gutter) = branch_gutter;
    let (r, response) = ui.allocate_exact_size(
        vec2(
            ui.available_width(),
            if reserve_gutter { 21.3 } else { 21.0 },
        ),
        Sense::click(),
    );
    if selected {
        ui.painter()
            .rect_filled(r, 0.0, ui.visuals().selection.bg_fill);
    }
    let x = r.left() + 4.0 + level as f32 * 20.0 - if reserve_gutter { 0.0 } else { 18.0 };
    let arrow = Rect::from_center_size(pos2(x + 6.0, r.center().y), vec2(14.0, 19.0));
    let toggle = branch.is_some()
        && ui
            .interact(arrow, ui.id().with((id, "expand")), Sense::click())
            .clicked();
    if let Some(open) = branch {
        let c = arrow.center();
        let pts = if open {
            vec![c + vec2(-3.0, 2.0), c + vec2(3.0, 2.0), c + vec2(3.0, -4.0)]
        } else {
            vec![
                c + vec2(-2.0, -4.0),
                c + vec2(-2.0, 4.0),
                c + vec2(2.0, 0.0),
            ]
        };
        ui.painter().add(egui::Shape::convex_polygon(
            pts,
            Color32::from_gray(218),
            Stroke::NONE,
        ));
    }
    icon(
        ui.painter(),
        Rect::from_center_size(pos2(x + 23.0, r.center().y), vec2(18.0, 18.0)),
        cat,
    );
    let text_rect = Rect::from_min_max(pos2(x + 35.0, r.top()), r.max);
    ui.painter().with_clip_rect(text_rect).text(
        text_rect.left_center(),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::from_gray(220),
    );
    (response.clicked() && !toggle, toggle, response)
}

pub(super) fn attributes(r: &Fields) -> String {
    let kind = text(r, "RULEKIND");
    if kind == "LayerPairs" {
        return format!(
            "Enforce layer pairs settings = {}",
            if matches!(text(r, "ENFORCE"), "FALSE" | "0") {
                "False"
            } else {
                "True"
            }
        );
    }
    if kind == "PolygonConnect" {
        return if rules::advanced_connection(r) {
            "Advanced settings".into()
        } else {
            format!("Style = {} Connect", text(r, "CONNECTSTYLE"))
        };
    }
    if kind == "HoleSize" && text(r, "ABSOLUTEVALUES") == "FALSE" {
        return format!(
            "Min Hole Size = {}%   Max Hole Size = {}%",
            text(r, "MINPERCENT").trim_end_matches('%'),
            text(r, "MAXPERCENT").trim_end_matches('%')
        );
    }
    let fields = rules::fields(kind);
    if fields.is_empty() {
        return text(r, "COMMENT").into();
    }
    fields
        .iter()
        .map(|(key, label)| {
            let value = rules::dimension(r, key)
                .map(|n| {
                    format!(
                        "{}mm",
                        format!("{:.5}", n as f64 / 1e6)
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                    )
                })
                .unwrap_or_else(|_| text(r, key).into());
            format!("{label} = {value}")
        })
        .collect::<Vec<_>>()
        .join("   ")
}

/// A clipped native-looking table cell. UI child keeps editing IDs stable.
pub(super) fn cell(ui: &mut egui::Ui, rect: Rect, f: impl FnOnce(&mut egui::Ui)) {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(ui.next_auto_id())
            .max_rect(rect.shrink2(vec2(3.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    f(&mut child);
}

/// Keep partial numeric input (for example `0.`) while an editor owns focus.
pub(super) fn numeric_edit(
    ui: &mut egui::Ui,
    key: impl std::hash::Hash,
    value: &mut String,
    width: f32,
    frame: bool,
) -> egui::Response {
    let id = ui.id().with(key);
    let buffer = id.with("numeric_input");
    if ui.memory(|m| m.has_focus(id))
        && let Some(editing) = ui.ctx().data(|d| d.get_temp::<String>(buffer))
    {
        *value = editing;
    }
    let response = ui.add(
        egui::TextEdit::singleline(value)
            .id(id)
            .desired_width(width)
            .frame(frame)
            .margin(vec2(if frame { 3.0 } else { 0.0 }, 1.0))
            .text_color(if frame {
                Color32::from_gray(220)
            } else {
                Color32::from_rgb(156, 194, 222)
            }),
    );
    let focused = response.has_focus();
    ui.ctx().data_mut(|d| {
        if focused {
            d.insert_temp(buffer, value.clone());
        } else {
            d.remove::<String>(buffer);
        }
    });
    response
}
pub(super) fn table_header(ui: &mut egui::Ui, widths: &[f32], labels: &[&str]) -> Rect {
    let (r, _) = ui.allocate_exact_size(vec2(widths.iter().sum(), 20.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, HEAD);
    let mut x = r.left();
    for (w, label) in widths.iter().zip(labels) {
        let c = Rect::from_min_size(pos2(x, r.top()), vec2(*w, r.height()));
        cell(ui, c, |ui| {
            ui.add(egui::Label::new(*label).truncate());
        });
        ui.painter().vline(x, r.y_range(), Stroke::new(1.0, LINE));
        x += w;
    }
    ui.painter()
        .rect_stroke(r, 0.0, Stroke::new(1.0, LINE), StrokeKind::Inside);
    r
}

pub(super) fn dimension_arrow(p: &egui::Painter, a: Pos2, b: Pos2) {
    p.line_segment([a, b], Stroke::new(1.0, DIM));
    let v = (b - a).normalized();
    let n = vec2(-v.y, v.x);
    for (tip, dir) in [(a, v), (b, -v)] {
        for s in [-1.0, 1.0] {
            p.line_segment([tip, tip + dir * 4.0 + n * s * 2.0], Stroke::new(1.0, DIM));
        }
    }
}

/// Status messages must not change the size or move the footer of their owner.
pub(super) fn message(ctx: &egui::Context, id: impl std::hash::Hash, message: &mut String) {
    if message.is_empty() {
        return;
    }
    let mut dismiss = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    egui::Window::new("Information")
        .id(egui::Id::new(id))
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_max_width(480.0);
            ui.label(message.as_str());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                dismiss |= ui
                    .add_sized([75.0, 25.0], egui::Button::new("OK"))
                    .clicked();
            });
        });
    if dismiss {
        message.clear();
    }
}

pub(super) fn illustration(ui: &mut egui::Ui, kind: &str) -> Rect {
    let clearance = kind == "Clearance";
    let (r, _) = ui.allocate_exact_size(
        if clearance {
            vec2(215.0, 130.0)
        } else {
            vec2(280.0, 155.0)
        },
        Sense::hover(),
    );
    let p = ui.painter_at(r);
    let at = |x: f32, y: f32| r.min + vec2(x, y - if clearance { 49.0 } else { 0.0 });
    match kind {
        "Width" => {
            // Three widths and the centerline follow the native stepped-track illustration.
            for (a, b, w) in [
                (at(35.0, 111.0), at(88.0, 111.0), 10.0),
                (at(88.0, 111.0), at(153.0, 111.0), 20.0),
                (at(153.0, 111.0), at(205.0, 111.0), 38.0),
            ] {
                p.line_segment([a, b], Stroke::new(w, GOLD));
            }
            for (x, half) in [(24.0, 5.0), (121.0, 10.0), (215.0, 19.0)] {
                dimension_arrow(&p, at(x, 111.0 - half), at(x, 111.0 + half));
            }
        }
        "Clearance" | "BoardOutlineClearance" => {
            p.line_segment([at(30.0, 61.0), at(112.0, 61.0)], Stroke::new(19.0, GOLD));
            p.line_segment([at(112.0, 61.0), at(167.0, 113.0)], Stroke::new(19.0, GOLD));
            p.circle_filled(at(91.0, 119.0), 26.0, GOLD);
            p.circle_filled(at(91.0, 119.0), 10.0, BG);
            p.line_segment(
                [at(3.0, 72.0), at(104.0, 72.0)],
                Stroke::new(1.0, Color32::GRAY),
            );
            p.line_segment(
                [at(3.0, 94.0), at(69.0, 94.0)],
                Stroke::new(1.0, Color32::GRAY),
            );
            dimension_arrow(&p, at(14.0, 72.0), at(14.0, 94.0));
        }
        "RoutingVias" | "HoleSize" | "MinimumAnnularRing" | "HoleToHoleClearance" => {
            let c = at(115.0, 85.0);
            p.circle_filled(c, 50.0, GOLD);
            p.circle_filled(c, 25.0, BG);
            dimension_arrow(&p, at(65.0, 85.0), at(165.0, 85.0));
            dimension_arrow(&p, at(90.0, 142.0), at(140.0, 142.0));
            if kind == "HoleToHoleClearance" {
                p.circle_filled(at(228.0, 85.0), 25.0, GOLD);
                p.circle_filled(at(228.0, 85.0), 14.0, BG);
            }
        }
        "SolderMaskExpansion"
        | "PasteMaskExpansion"
        | "MinimumSolderMaskSliver"
        | "SilkToSolderMaskClearance" => {
            p.rect_filled(
                Rect::from_min_size(at(32.0, 27.0), vec2(114.0, 100.0)),
                0.0,
                Color32::from_rgb(45, 132, 92),
            );
            p.rect_filled(
                Rect::from_min_size(at(43.0, 38.0), vec2(92.0, 78.0)),
                0.0,
                GOLD,
            );
            dimension_arrow(&p, at(32.0, 15.0), at(43.0, 15.0));
            if kind == "MinimumSolderMaskSliver" {
                p.rect_filled(
                    Rect::from_min_size(at(174.0, 27.0), vec2(85.0, 100.0)),
                    0.0,
                    Color32::from_rgb(45, 132, 92),
                );
                dimension_arrow(&p, at(146.0, 78.0), at(174.0, 78.0));
            }
            if kind == "SilkToSolderMaskClearance" {
                p.line_segment(
                    [at(178.0, 30.0), at(178.0, 127.0)],
                    Stroke::new(8.0, Color32::from_gray(220)),
                );
                dimension_arrow(&p, at(146.0, 78.0), at(174.0, 78.0));
            }
        }
        _ => {}
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_child_regions_do_not_share_explicit_editor_ids() {
        let ctx = egui::Context::default();
        let mut ids = vec![];
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                for y in [10.0, 40.0] {
                    child(
                        ui,
                        egui::UiBuilder::new()
                            .max_rect(Rect::from_min_size(pos2(10.0, y), vec2(150.0, 25.0))),
                        |ui| {
                            let mut value = "0.1mm".to_owned();
                            ids.push(numeric_edit(ui, "EXPANSION", &mut value, 100.0, false).id);
                        },
                    );
                }
            });
        });
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }
    #[test]
    fn numeric_edit_keeps_the_decimal_point_when_the_model_formats_between_frames() {
        let ctx = egui::Context::default();
        let mut value = String::new();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.memory_mut(|m| m.request_focus(ui.id().with("dimension")));
                value = "0.".into();
                numeric_edit(ui, "dimension", &mut value, 100.0, false);
            });
        });
        let input = egui::RawInput {
            events: vec![egui::Event::Text("15".into())],
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                value = "0mm".into();
                numeric_edit(ui, "dimension", &mut value, 100.0, false);
            });
        });
        assert_eq!(value, "0.15");
    }
}
