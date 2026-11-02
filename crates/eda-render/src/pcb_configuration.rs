//! Observed AD 24.4.1 View Configuration layout. All changes are view-local.
use super::{
    BoardView,
    catalog::{Catalog, Layer},
    display::Settings,
};
use eda_core::pcb::*;
use egui::{Align2, Color32, FontId, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Vec2};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
const TEXT: Color32 = Color32::from_gray(212);
const BORDER: Color32 = Color32::from_gray(38);
const SELECTED: Color32 = Color32::from_rgb(92, 128, 155);
pub(super) const OBJECTS: [(usize, &str); 14] = [
    (0, "All Objects"),
    (2, "Pads"),
    (3, "Vias"),
    (1, "Arcs"),
    (4, "Tracks"),
    (10, "Polygons"),
    (11, "Regions"),
    (6, "Fills"),
    (5, "Texts"),
    (8, "Rooms"),
    (9, "Dimensions"),
    (13, "Keepouts"),
    (14, "Coordinates"),
    (12, "3D Body"),
];
const MATRIX_WIDTHS: [f32; 13] = [
    34., 34., 34., 36., 50., 42., 30., 36., 40., 54., 52., 62., 44.,
];
fn matrix_cell(rect: Rect, n: usize, indent: f32) -> Rect {
    part(
        rect,
        120.0 - indent + MATRIX_WIDTHS[..n].iter().sum::<f32>(),
        MATRIX_WIDTHS[n],
    )
}
fn supported(kind: usize) -> bool {
    !matches!(kind, 8 | 9 | 14)
}
#[derive(Clone)]
struct Advanced {
    opacity: BTreeMap<u16, [u8; 23]>,
    selected: BTreeSet<(u16, usize)>,
    used_only: bool,
}
#[derive(Clone, Default)]
pub(super) struct Panel {
    pub catalog: Arc<Catalog>,
    catalog_revision: u64,
    search: String,
    section: bool,
    collapsed: BTreeSet<String>,
    selected_object: usize,
    advanced: Option<Advanced>,
}
fn text(ui: &egui::Ui, rect: Rect, value: &str, align: Align2, color: Color32) {
    let anchor = match align {
        Align2::RIGHT_CENTER => rect.right_center(),
        Align2::CENTER_CENTER => rect.center(),
        _ => rect.left_center(),
    };
    let p = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let mut job =
        egui::text::LayoutJob::simple_singleline(value.into(), FontId::proportional(11.0), color);
    job.wrap.max_width = rect.width().max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = p.layout_job(job);
    let at = align.anchor_size(anchor, galley.size()).min;
    p.galley(at, galley, color);
}
fn part(rect: Rect, x: f32, width: f32) -> Rect {
    Rect::from_min_size(
        rect.min + Vec2::new(x, 0.0),
        Vec2::new(width, rect.height()),
    )
}
fn child<R>(
    ui: &mut egui::Ui,
    rect: Rect,
    salt: impl std::hash::Hash,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut c = ui.new_child(
        egui::UiBuilder::new()
            .id_salt((salt, rect.left().to_bits(), rect.top().to_bits()))
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    c.set_clip_rect(rect.intersect(ui.clip_rect()));
    body(&mut c)
}
fn line(ui: &mut egui::Ui, h: f32) -> Rect {
    ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover())
        .0
}
fn eye(ui: &mut egui::Ui, rect: Rect, on: &mut bool, enabled: bool) -> Response {
    let response = ui.interact(
        rect,
        ui.id()
            .with(("eye", rect.left().to_bits(), rect.top().to_bits())),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let c = if enabled {
        if *on { TEXT } else { Color32::from_gray(116) }
    } else {
        Color32::from_gray(100)
    };
    let p = ui.painter();
    let center = rect.center();
    p.add(egui::Shape::closed_line(
        vec![
            center + Vec2::new(-5.0, 0.0),
            center + Vec2::new(-2.0, -3.0),
            center + Vec2::new(2.0, -3.0),
            center + Vec2::new(5.0, 0.0),
            center + Vec2::new(2.0, 3.0),
            center + Vec2::new(-2.0, 3.0),
        ],
        Stroke::new(1.0, c),
    ));
    p.circle_filled(center, 1.5, c);
    if !*on {
        p.line_segment(
            [center - Vec2::splat(5.0), center + Vec2::splat(5.0)],
            Stroke::new(1.0, c),
        );
    }
    if response.clicked() {
        *on = !*on;
    }
    response
}
fn arrow(ui: &egui::Ui, rect: Rect, open: bool) {
    let c = rect.center();
    let pts = if open {
        vec![
            c + Vec2::new(-3.0, -2.0),
            c + Vec2::new(3.0, -2.0),
            c + Vec2::new(0.0, 2.0),
        ]
    } else {
        vec![
            c + Vec2::new(-2.0, -3.0),
            c + Vec2::new(-2.0, 3.0),
            c + Vec2::new(2.0, 0.0),
        ]
    };
    ui.painter()
        .add(egui::Shape::convex_polygon(pts, TEXT, Stroke::NONE));
}
fn color(ui: &mut egui::Ui, rect: Rect, value: &mut Color32, enabled: bool) -> bool {
    let swatch = Rect::from_center_size(rect.center(), Vec2::splat(14.0));
    ui.painter().rect_filled(swatch, 0.0, *value);
    ui.painter().rect_stroke(
        swatch,
        0.0,
        Stroke::new(1.0, Color32::from_gray(90)),
        StrokeKind::Inside,
    );
    let response = ui.interact(
        rect,
        ui.id()
            .with(("color", rect.left().to_bits(), rect.top().to_bits())),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    egui::Popup::from_toggle_button_response(&response)
        .show(|ui| {
            egui::color_picker::color_picker_color32(ui, value, egui::color_picker::Alpha::Opaque)
        })
        .is_some_and(|r| r.inner)
}
fn tab(ui: &mut egui::Ui, rect: Rect, label: &str, selected: bool) -> bool {
    ui.painter().rect_filled(
        rect,
        0.0,
        Color32::from_gray(if selected { 57 } else { 65 }),
    );
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, Color32::from_gray(82)),
    );
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, BORDER),
    );
    text(
        ui,
        rect.shrink2(Vec2::new(4.0, 0.0)),
        label,
        Align2::CENTER_CENTER,
        TEXT,
    );
    ui.interact(rect, ui.id().with(("tab", label)), Sense::click())
        .clicked()
}
fn profile_icons(ui: &mut egui::Ui, row: Rect, positions: [f32; 3]) {
    for (n, x) in positions.into_iter().enumerate() {
        let r = part(row, x, 30.0).shrink2(Vec2::new(1.0, 2.0));
        ui.painter().rect_filled(r, 1.0, Color32::from_gray(59));
        ui.painter().rect_stroke(
            r,
            1.0,
            Stroke::new(1.0, Color32::from_gray(46)),
            StrokeKind::Inside,
        );
        let c = r.center();
        let stroke = Stroke::new(1.0, Color32::from_gray(112));
        if n == 0 {
            ui.painter().rect_stroke(
                Rect::from_center_size(c - Vec2::new(1., 1.), Vec2::new(12., 9.)),
                0.,
                stroke,
                StrokeKind::Inside,
            );
            ui.painter()
                .line_segment([c + Vec2::new(3., -1.), c + Vec2::new(3., 7.)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(-1., 3.), c + Vec2::new(7., 3.)], stroke);
        } else if n == 1 {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, Vec2::splat(11.)),
                0.,
                stroke,
                StrokeKind::Inside,
            );
            ui.painter().rect_stroke(
                Rect::from_center_size(c + Vec2::new(0., 2.), Vec2::new(7., 5.)),
                0.,
                stroke,
                StrokeKind::Inside,
            );
        } else {
            ui.painter().rect_stroke(
                Rect::from_center_size(c + Vec2::new(0., 1.), Vec2::new(8., 10.)),
                0.,
                stroke,
                StrokeKind::Inside,
            );
            ui.painter()
                .line_segment([c + Vec2::new(-5., -5.), c + Vec2::new(5., -5.)], stroke);
        }
        ui.interact(
            r,
            ui.id().with(("profile", n, r.top().to_bits())),
            Sense::hover(),
        )
        .on_hover_text("Configuration profiles are not implemented yet");
    }
}
fn checkbox(ui: &mut egui::Ui, rect: Rect, value: &mut bool, enabled: bool) -> bool {
    let r = Rect::from_center_size(rect.center() - Vec2::new(1.0, 0.0), Vec2::splat(14.0));
    ui.painter()
        .rect_filled(r, 2.0, Color32::from_gray(if enabled { 88 } else { 68 }));
    ui.painter()
        .rect_stroke(r, 2.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    if *value {
        let c = r.center();
        ui.painter().add(egui::Shape::line(
            vec![
                c + Vec2::new(-4., 0.),
                c + Vec2::new(-1., 3.),
                c + Vec2::new(4., -4.),
            ],
            Stroke::new(1.0, if enabled { TEXT } else { Color32::GRAY }),
        ));
    }
    if enabled
        && ui
            .interact(
                rect,
                ui.id()
                    .with(("checkbox", rect.left().to_bits(), rect.top().to_bits())),
                Sense::click(),
            )
            .clicked()
    {
        *value = !*value;
        true
    } else {
        false
    }
}
fn button(ui: &mut egui::Ui, rect: Rect, label: &str, selected: bool, enabled: bool) -> bool {
    let h = match rect.height() as u32 {
        27 | 29 => 22.0,
        28 => 24.0,
        _ => rect.height(),
    };
    let rect = Rect::from_center_size(rect.center(), Vec2::new(rect.width(), h));
    let response = ui.interact(
        rect,
        ui.id()
            .with(("button", label, rect.left().to_bits(), rect.top().to_bits())),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    let fill = if selected {
        if enabled {
            SELECTED
        } else {
            Color32::from_gray(84)
        }
    } else {
        Color32::from_gray(if rect.height() >= 24. { 74 } else { 65 })
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        2.0,
        Stroke::new(
            1.0,
            if enabled && response.hovered() {
                Color32::from_rgb(156, 190, 219)
            } else {
                BORDER
            },
        ),
        StrokeKind::Inside,
    );
    text(
        ui,
        rect.shrink2(Vec2::new(3., 0.)),
        label,
        Align2::CENTER_CENTER,
        if enabled {
            TEXT
        } else {
            Color32::from_gray(154)
        },
    );
    enabled && response.clicked()
}
fn slider(ui: &mut egui::Ui, rect: Rect, value: &mut f32, enabled: bool) -> bool {
    let id = ui
        .id()
        .with(("slider", rect.left().to_bits(), rect.top().to_bits()));
    ui.interact(
        rect,
        id,
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    let track =
        Rect::from_center_size(rect.center(), Vec2::new((rect.width() - 6.0).max(1.0), 7.0));
    ui.painter().rect_filled(track, 1.0, Color32::from_gray(43));
    ui.painter().rect_stroke(
        track,
        1.0,
        Stroke::new(1.0, Color32::from_gray(50)),
        StrokeKind::Inside,
    );
    let x = egui::lerp(track.x_range(), value.clamp(0.0, 100.0) / 100.0);
    ui.painter().rect_filled(
        Rect::from_center_size(Pos2::new(x, track.center().y), Vec2::new(4.0, 10.0)),
        1.0,
        if enabled { TEXT } else { Color32::GRAY },
    );
    // Preserve a press/move/release delivered in one frame. egui's completed
    // drag response no longer retains its press position in that case.
    let mut active = ui.ctx().data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let mut pointer = None;
    if enabled && ui.input(|i| i.focused) {
        for event in ui.input(|i| i.events.clone()) {
            match event {
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    ..
                } if rect.contains(pos)
                    && ui.clip_rect().contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id()) =>
                {
                    active = true;
                    pointer = Some(pos);
                }
                egui::Event::PointerMoved(pos) if active => pointer = Some(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                } if active => {
                    pointer = Some(pos);
                    active = false;
                }
                egui::Event::PointerGone => active = false,
                _ => {}
            }
        }
    } else {
        active = false;
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, active));
    if let Some(pos) = pointer {
        *value = ((pos.x - track.left()) / track.width() * 100.0).clamp(0.0, 100.0);
        return true;
    }
    false
}
fn group(
    ui: &mut egui::Ui,
    panel: &mut Panel,
    name: &str,
    default_open: bool,
    body: impl FnOnce(&mut egui::Ui, &mut Panel),
) {
    let key = format!("group:{name}");
    let mut open = if default_open {
        !panel.collapsed.contains(&key)
    } else {
        panel.collapsed.contains(&key)
    };
    let rect = line(ui, 24.0);
    arrow(ui, part(rect, 0.0, 8.0), open);
    child(
        ui,
        part(rect, 8.0, rect.width() - 8.0),
        ("heading", name),
        |ui| {
            let bold = egui::FontFamily::Name("ui-bold".into());
            let family = if ui.fonts(|f| f.families().contains(&bold)) {
                bold
            } else {
                egui::FontFamily::Proportional
            };
            ui.label(
                egui::RichText::new(name)
                    .family(family)
                    .size(11.0)
                    .color(TEXT),
            );
        },
    );
    if ui
        .interact(rect, ui.id().with(&key), Sense::click())
        .clicked()
    {
        open = !open;
        if !panel.collapsed.remove(&key) {
            panel.collapsed.insert(key);
        }
    }
    if open {
        egui::Frame::NONE
            .fill(Color32::from_gray(65))
            .inner_margin(egui::Margin {
                left: 7,
                right: 9,
                top: 5,
                bottom: 5,
            })
            .show(ui, |ui| body(ui, panel));
    }
    ui.add_space(9.0);
}
fn border(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::NONE
        .stroke(Stroke::new(1.0, BORDER))
        .inner_margin(0)
        .show(ui, body);
}
fn matches(query: &str, value: &str) -> bool {
    query.is_empty() || value.to_lowercase().contains(query)
}
fn short(id: u16) -> String {
    mechanical_label(id).unwrap_or_else(|| match id {
        TOP => "T".into(),
        BOTTOM => "B".into(),
        _ => String::new(),
    })
}
fn mechanical_label(id: u16) -> Option<String> {
    super::catalog::mechanical(id).map(|n| format!("M{n}"))
}
fn layer_color(view: &mut BoardView, ui: &mut egui::Ui, rect: Rect, id: u16) {
    let mut c = view.color(id);
    if color(ui, rect, &mut c, true) {
        view.display.colors.insert(id, c);
    }
}
fn layer_eye(view: &mut BoardView, ui: &mut egui::Ui, rect: Rect, id: u16) {
    let mut on = view.visible.contains(&id);
    if eye(ui, rect, &mut on, true).clicked() {
        if on {
            view.visible.insert(id);
        } else {
            view.visible.remove(&id);
        }
    }
}
fn used_on(view: &mut BoardView, ui: &mut egui::Ui, rect: Rect, ids: &[u16], catalog: &Catalog) {
    let rect = part(rect, rect.width() - 58.0, 51.0);
    text(
        ui,
        rect,
        "Used On",
        Align2::RIGHT_CENTER,
        Color32::from_rgb(156, 190, 219),
    );
    if ui
        .interact(
            rect,
            ui.id().with(("used", rect.top().to_bits())),
            Sense::click(),
        )
        .clicked()
    {
        for id in ids {
            if catalog.layer(*id).is_some_and(|l| l.used) {
                view.visible.insert(*id);
            } else {
                view.visible.remove(id);
            }
        }
    }
}
fn layer_row(
    view: &mut BoardView,
    ui: &mut egui::Ui,
    l: &Layer,
    ordinal: usize,
    requested: &mut Option<u16>,
) {
    let rect = line(ui, 20.0);
    let w = rect.width();
    if view.active_layer == l.id {
        ui.painter().rect_filled(rect, 0.0, Color32::from_gray(70));
    }
    layer_eye(view, ui, part(rect, 25.0, 18.0), l.id);
    layer_color(view, ui, part(rect, 48.0, 17.0), l.id);
    let label = if l.copper {
        format!("[{}] {} ({})", ordinal, l.name, short(l.id))
    } else {
        l.name.clone()
    };
    let name_rect = part(rect, 70.0, (w - 115.0).max(20.0));
    text(ui, name_rect, &label, Align2::LEFT_CENTER, TEXT);
    let r = ui.interact(name_rect, ui.id().with(("activate", l.id)), Sense::click());
    r.clone().on_hover_text(&l.name);
    if r.clicked() {
        *requested = Some(l.id);
        view.visible.insert(l.id);
    }
    let short = short(l.id);
    text(
        ui,
        part(rect, w - 45.0, 38.0),
        if l.copper {
            if (39..=54).contains(&l.id) {
                "Plane"
            } else {
                "Signal"
            }
        } else {
            &short
        },
        Align2::RIGHT_CENTER,
        TEXT,
    );
}
fn layer_group(
    view: &mut BoardView,
    ui: &mut egui::Ui,
    panel: &mut Panel,
    n: usize,
    title: &str,
    query: &str,
    requested: &mut Option<u16>,
) {
    let catalog = panel.catalog.clone();
    let ids: Vec<_> = if n == 1 {
        catalog
            .pairs
            .iter()
            .flat_map(|p| [p.top, p.bottom])
            .collect()
    } else {
        catalog.group(n).iter().map(|l| l.id).collect()
    };
    if ids.is_empty() {
        return;
    }
    if !query.is_empty()
        && !ids
            .iter()
            .any(|id| catalog.layer(*id).is_some_and(|l| matches(query, &l.name)))
        && !matches(query, title)
    {
        return;
    }
    let rect = line(ui, 20.0);
    let w = rect.width();
    let key = format!("layers:{n}");
    let open = !panel.collapsed.contains(&key);
    let mut on = ids.iter().any(|id| view.visible.contains(id));
    if eye(ui, part(rect, 5.0, 18.0), &mut on, true).clicked() {
        for id in &ids {
            if on {
                view.visible.insert(*id);
            } else {
                view.visible.remove(id);
            }
        }
    }
    arrow(ui, part(rect, 27.0, 14.0), open);
    text(
        ui,
        part(rect, 42.0, w - 105.0),
        title,
        Align2::LEFT_CENTER,
        TEXT,
    );
    if ui
        .interact(
            part(rect, 25.0, w - 95.0),
            ui.id().with(&key),
            Sense::click(),
        )
        .clicked()
        && !panel.collapsed.remove(&key)
    {
        panel.collapsed.insert(key);
    }
    used_on(view, ui, rect, &ids, &catalog);
    if !open {
        return;
    }
    if n == 1 {
        let rect = line(ui, 20.0);
        text(ui, part(rect, 49.0, 55.0), "Top", Align2::LEFT_CENTER, TEXT);
        text(
            ui,
            part(rect, 117.0, 90.0),
            "Layer",
            Align2::LEFT_CENTER,
            TEXT,
        );
        text(
            ui,
            part(rect, w - 65.0, 55.0),
            "Bottom",
            Align2::LEFT_CENTER,
            TEXT,
        );
        for p in &catalog.pairs {
            if !query.is_empty()
                && !matches(query, &p.name)
                && ![p.top, p.bottom]
                    .iter()
                    .any(|id| catalog.layer(*id).is_some_and(|l| matches(query, &l.name)))
            {
                continue;
            }
            let rect = line(ui, 20.0);
            layer_eye(view, ui, part(rect, 36.0, 18.0), p.top);
            layer_color(view, ui, part(rect, 61.0, 17.0), p.top);
            text(
                ui,
                part(rect, 83.0, 31.0),
                &short(p.top),
                Align2::LEFT_CENTER,
                TEXT,
            );
            let mid = part(rect, 117.0, (w - 195.0).max(20.0));
            text(ui, mid, &p.name, Align2::LEFT_CENTER, TEXT);
            ui.interact(mid, ui.id().with(("pair", p.top)), Sense::hover())
                .on_hover_text(&p.name);
            layer_eye(view, ui, part(rect, w - 79.0, 18.0), p.bottom);
            layer_color(view, ui, part(rect, w - 56.0, 17.0), p.bottom);
            text(
                ui,
                part(rect, w - 37.0, 35.0),
                &short(p.bottom),
                Align2::LEFT_CENTER,
                TEXT,
            );
        }
    } else {
        for l in catalog.group(n) {
            if matches(query, &l.name) || matches(query, title) {
                let ordinal = catalog
                    .group(0)
                    .iter()
                    .position(|a| a.id == l.id)
                    .map_or(0, |i| i + 1);
                ui.push_id(l.id, |ui| layer_row(view, ui, l, ordinal, requested));
            }
        }
    }
}
fn layers(
    view: &mut BoardView,
    ui: &mut egui::Ui,
    panel: &mut Panel,
    camera: &mut crate::Camera,
    requested: &mut Option<u16>,
    query: &str,
) {
    group(ui, panel, "Layers", true, |ui, panel| {
        border(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let rect = line(ui, 20.0);
            let mut on = !view.visible.is_empty();
            if eye(ui, part(rect, 5.0, 18.0), &mut on, true).clicked() {
                if on {
                    view.visible
                        .extend(panel.catalog.layers.iter().map(|l| l.id));
                } else {
                    view.visible.clear();
                }
            }
            text(
                ui,
                part(rect, 25.0, 150.0),
                "All Layers",
                Align2::LEFT_CENTER,
                TEXT,
            );
            let ids: Vec<_> = panel.catalog.layers.iter().map(|l| l.id).collect();
            used_on(view, ui, rect, &ids, &panel.catalog);
            for (n, title) in [
                (0, "Signal And Plane Layers (S)"),
                (1, "Component Layer Pairs (C)"),
                (2, "Mechanical Layers (M)"),
                (3, "Other Layers (O)"),
            ] {
                layer_group(view, ui, panel, n, title, query, requested);
            }
        });
        ui.add_space(4.0);
        let rect = line(ui, 27.0);
        text(
            ui,
            part(rect, 0.0, 100.0),
            "Layer Sets",
            Align2::RIGHT_CENTER,
            TEXT,
        );
        child(ui, part(rect, 105.0, 75.0), "layer_sets", |ui| {
            egui::ComboBox::from_id_salt("set")
                .width(68.0)
                .selected_text("All Layers")
                .show_ui(ui, |ui| {
                    if ui.selectable_label(true, "All Layers").clicked() {
                        view.visible
                            .extend(panel.catalog.layers.iter().map(|l| l.id));
                    }
                });
        });
        profile_icons(ui, rect, [184.0, 218.0, 252.0]);
        let rect = line(ui, 27.0);
        text(
            ui,
            part(rect, 0.0, 100.0),
            "Active Layer",
            Align2::RIGHT_CENTER,
            TEXT,
        );
        child(
            ui,
            part(rect, 105.0, (rect.width() - 105.0).max(50.0)),
            "active_layer",
            |ui| {
                egui::ComboBox::from_id_salt("active")
                    .width(ui.available_width() - 8.0)
                    .truncate()
                    .selected_text(
                        panel
                            .catalog
                            .layer(view.active_layer)
                            .map_or("Top Layer", |l| l.name.as_str()),
                    )
                    .show_ui(ui, |ui| {
                        for l in &panel.catalog.layers {
                            if ui
                                .selectable_label(view.active_layer == l.id, &l.name)
                                .clicked()
                            {
                                *requested = Some(l.id);
                                view.visible.insert(l.id);
                            }
                        }
                    });
            },
        );
        let rect = line(ui, 23.0);
        checkbox(ui, part(rect, 105.0, 20.0), &mut camera.mirrored_x, true);
        text(
            ui,
            part(rect, 126.0, rect.width() - 126.0),
            "View From Bottom...  (Ctrl + F)",
            Align2::LEFT_CENTER,
            TEXT,
        );
        let rect = line(ui, 27.0);
        button(ui, part(rect, 105.0, 83.0), "Import", false, false);
        button(
            ui,
            part(rect, 193.0, (rect.width() - 193.0).max(40.0)),
            "Export",
            false,
            false,
        );
    });
}
fn aggregate(settings: &Settings, catalog: &Catalog, kind: usize) -> Option<u8> {
    let mut values = catalog.layers.iter().flat_map(|l| {
        let a = settings.opacity.get(&l.id).copied().unwrap_or([255; 23]);
        OBJECTS
            .iter()
            .filter(move |(k, _)| *k != 0 && (kind == 0 || *k == kind))
            .map(move |(k, _)| a[*k])
    });
    let first = values.next()?;
    values.all(|v| v == first).then_some(first)
}
fn set_opacity(settings: &mut Settings, catalog: &Catalog, kind: usize, alpha: u8) {
    for l in &catalog.layers {
        let a = settings.opacity.entry(l.id).or_insert([255; 23]);
        for (k, _) in OBJECTS {
            if k != 0 && (kind == 0 || kind == k) && supported(k) {
                a[k] = alpha;
            }
        }
    }
}
fn objects(view: &mut BoardView, ui: &mut egui::Ui, panel: &mut Panel) {
    group(ui, panel, "Object Visibility", true, |ui, panel| {
        border(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let rect = line(ui, 20.0);
            let w = rect.width();
            text(ui, part(rect, 8.0, 92.0), "Name", Align2::LEFT_CENTER, TEXT);
            text(
                ui,
                part(rect, 106.0, 57.0),
                "Draft",
                Align2::LEFT_CENTER,
                TEXT,
            );
            text(
                ui,
                part(rect, 168.0, w - 168.0),
                "Transparency",
                Align2::LEFT_CENTER,
                TEXT,
            );
            for (kind, name) in OBJECTS {
                ui.push_id(kind, |ui| {
                    let rect = line(ui, 20.0);
                    if panel.selected_object == kind {
                        ui.painter().rect_filled(rect, 0.0, SELECTED);
                    }
                    let enabled = supported(kind);
                    let mut visible = if kind == 0 {
                        !view.display.hidden.iter().all(|v| *v)
                    } else {
                        !view.display.hidden[kind]
                    };
                    if eye(ui, part(rect, 4.0, 19.0), &mut visible, enabled).clicked() {
                        if kind == 0 {
                            view.display.hidden.fill(!visible);
                        } else {
                            view.display.hidden[kind] = !visible;
                        }
                    }
                    text(
                        ui,
                        part(rect, 24.0, 78.0),
                        name,
                        Align2::LEFT_CENTER,
                        if enabled { TEXT } else { Color32::GRAY },
                    );
                    if ui
                        .interact(part(rect, 23.0, 78.0), ui.id().with("name"), Sense::click())
                        .clicked()
                    {
                        panel.selected_object = kind;
                    }
                    let mut draft = if kind == 0 {
                        OBJECTS
                            .iter()
                            .filter(|(k, _)| *k != 0 && supported(*k))
                            .all(|(k, _)| view.display.draft[*k])
                    } else {
                        view.display.draft[kind]
                    };
                    if checkbox(
                        ui,
                        part(rect, 128.0, 20.0),
                        &mut draft,
                        enabled && kind != 5,
                    ) {
                        if kind == 0 {
                            for (k, _) in OBJECTS {
                                if supported(k) && k != 5 {
                                    view.display.draft[k] = draft;
                                }
                            }
                        } else {
                            view.display.draft[kind] = draft;
                        }
                    }
                    let a = aggregate(&view.display, &panel.catalog, kind);
                    let mut transparency = 100.0 - f32::from(a.unwrap_or(255)) * 100.0 / 255.0;
                    if slider(ui, part(rect, 172.0, 43.0), &mut transparency, enabled) {
                        set_opacity(
                            &mut view.display,
                            &panel.catalog,
                            kind,
                            ((100.0 - transparency) * 2.55).round() as u8,
                        );
                    }
                    text(
                        ui,
                        part(rect, w - 59.0, 52.0),
                        &a.map(|a| format!("{:.0}%", 100.0 - f32::from(a) * 100.0 / 255.0))
                            .unwrap_or_else(|| "Mixed".into()),
                        Align2::RIGHT_CENTER,
                        TEXT,
                    );
                });
            }
            let rect = line(ui, 47.0);
            if button(
                ui,
                Rect::from_min_size(
                    rect.right_bottom() - Vec2::new(87.0, 33.0),
                    Vec2::new(79.0, 26.0),
                ),
                "Advanced...",
                false,
                true,
            ) {
                panel.advanced = Some(Advanced {
                    opacity: view.display.opacity.clone(),
                    selected: BTreeSet::new(),
                    used_only: true,
                });
            }
        });
    });
}
fn mask_dim(view: &mut BoardView, ui: &mut egui::Ui, panel: &mut Panel) {
    group(ui, panel, "Mask and Dim Settings", true, |ui, _| {
        for (label, value) in [
            ("Dimmed Objects", &mut view.display.dim),
            ("Highlighted Objects", &mut view.display.highlight_dim),
            ("Masked Objects", &mut view.display.masked_dim),
        ] {
            let rect = line(ui, 24.0);
            text(
                ui,
                part(rect, 0.0, 111.0),
                label,
                Align2::RIGHT_CENTER,
                TEXT,
            );
            let mut n = f32::from(*value) * 100.0 / 255.0;
            if slider(ui, part(rect, 124.0, rect.width() - 124.0), &mut n, true) {
                *value = (n * 2.55).round() as u8;
            }
        }
    });
}
fn options(
    view: &mut BoardView,
    ui: &mut egui::Ui,
    panel: &mut Panel,
    grid: &mut bool,
    query: &str,
) {
    if query.is_empty() || matches(query, "General Settings") {
        group(ui, panel, "General Settings", true, |ui, _| {
            let rect = line(ui, 28.0);
            text(
                ui,
                part(rect, 0.0, 99.0),
                "Configuration",
                Align2::RIGHT_CENTER,
                TEXT,
            );
            child(ui, part(rect, 113.0, 65.0), "configuration", |ui| {
                egui::ComboBox::from_id_salt("config")
                    .width(65.0)
                    .truncate()
                    .selected_text(if view.three_d.enabled {
                        "Custom Configuration"
                    } else {
                        "Custom"
                    })
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "Custom");
                    });
            });
            profile_icons(ui, rect, [184.0, 218.0, 252.0]);
            let rect = line(ui, 27.0);
            text(ui, part(rect, 0.0, 99.0), "3D", Align2::RIGHT_CENTER, TEXT);
            for (on, label, x) in [(false, "Off", 113.0), (true, "On", 153.0)] {
                if button(
                    ui,
                    part(rect, x, 39.0),
                    label,
                    view.three_d.enabled == on,
                    true,
                ) {
                    view.three_d.enabled = on;
                }
            }
            text(
                ui,
                part(rect, 218.0, rect.width() - 218.0),
                "2 or 3",
                Align2::RIGHT_CENTER,
                TEXT,
            );
            let rect = line(ui, 27.0);
            text(
                ui,
                part(rect, 0.0, 99.0),
                "Single Layer Mode",
                Align2::RIGHT_CENTER,
                TEXT,
            );
            let single = if view.three_d.enabled {
                view.three_d.presentation.single_layer
            } else {
                view.single_layer != 0
            };
            if button(ui, part(rect, 113.0, 39.0), "Off", !single, true) {
                if view.three_d.enabled {
                    view.three_d.presentation.single_layer = false;
                } else {
                    view.single_layer = 0;
                }
            }
            if button(ui, part(rect, 153.0, 39.0), "On", single, true) {
                if view.three_d.enabled {
                    view.three_d.presentation.single_layer = true;
                } else {
                    view.single_layer = 1;
                }
            }
            if !view.three_d.enabled {
                child(ui, part(rect, 193.0, 44.0), "single_layer_mode", |ui| {
                    ui.menu_button("Mono", |ui| {
                        for (n, s) in [(1, "Dim Other Layers"), (2, "Hide Other Layers")] {
                            ui.selectable_value(&mut view.single_layer, n, s);
                        }
                    });
                });
            }
            text(
                ui,
                part(rect, 238.0, (rect.width() - 238.0).max(35.0)),
                "Shift + S",
                Align2::RIGHT_CENTER,
                TEXT,
            );
            ui.add_space(5.0);
            let divider = line(ui, 6.0);
            ui.painter().line_segment(
                [divider.left_center(), divider.right_center()],
                Stroke::new(1.0, BORDER),
            );
            ui.add_space(6.0);
            if view.three_d.enabled {
                let rect = line(ui, 29.0);
                text(
                    ui,
                    part(rect, 0.0, 99.0),
                    "Projection",
                    Align2::RIGHT_CENTER,
                    TEXT,
                );
                if button(
                    ui,
                    part(rect, 113.0, 91.0),
                    "Orthographic",
                    !view.three_d.presentation.perspective,
                    true,
                ) {
                    view.three_d.presentation.perspective = false;
                }
                if button(
                    ui,
                    part(rect, 205.0, 91.0),
                    "Perspective",
                    view.three_d.presentation.perspective,
                    true,
                ) {
                    view.three_d.presentation.perspective = true;
                }
                let rect = line(ui, 29.0);
                text(
                    ui,
                    part(rect, 0.0, 99.0),
                    "Show 3D Bodies",
                    Align2::RIGHT_CENTER,
                    TEXT,
                );
                for (on, label, x) in [(false, "Off", 113.0), (true, "On", 153.0)] {
                    if button(
                        ui,
                        part(rect, x, 39.0),
                        label,
                        view.three_d.show_components == on,
                        true,
                    ) {
                        view.three_d.show_components = on;
                        view.three_d.invalidate();
                    }
                }
                text(
                    ui,
                    part(rect, 228.0, rect.width() - 228.0),
                    "Shift + 7",
                    Align2::RIGHT_CENTER,
                    TEXT,
                );
            } else {
                let rect = line(ui, 29.0);
                text(
                    ui,
                    part(rect, 0.0, 87.0),
                    "Show Grid",
                    Align2::RIGHT_CENTER,
                    TEXT,
                );
                checkbox(ui, part(rect, 100.0, 20.0), grid, true);
                color(
                    ui,
                    part(rect, 120.0, 20.0),
                    &mut view.display.grid_color,
                    true,
                );
            }
        });
    }
    if view.three_d.enabled {
        three_d(view, ui, panel);
    } else if query.is_empty() || matches(query, "Object Visibility") {
        objects(view, ui, panel);
    }
    if query.is_empty() || matches(query, "Mask and Dim Settings") {
        mask_dim(view, ui, panel);
    }
    if !view.three_d.enabled && (query.is_empty() || matches(query, "Additional Options")) {
        group(ui, panel, "Additional Options", true, |ui, _| {
            let rect = line(ui, 28.0);
            button(ui, part(rect, 0.0, 70.0), "Test Points", false, false);
            if button(
                ui,
                part(rect, 75.0, 70.0),
                "Status Info",
                view.display.status_info,
                true,
            ) {
                view.display.status_info = !view.display.status_info;
            }
            if button(
                ui,
                part(rect, 150.0, 62.0),
                "Pad Nets",
                view.display.pad_nets,
                true,
            ) {
                view.display.pad_nets = !view.display.pad_nets;
            }
            let rect = line(ui, 28.0);
            if button(
                ui,
                part(rect, 0.0, 85.0),
                "Pad Numbers",
                view.display.pad_numbers,
                true,
            ) {
                view.display.pad_numbers = !view.display.pad_numbers;
            }
            if button(
                ui,
                part(rect, 90.0, 61.0),
                "Via Nets",
                view.display.via_nets,
                true,
            ) {
                view.display.via_nets = !view.display.via_nets;
            }
            button(
                ui,
                part(rect, 156.0, 61.0),
                "Via Span",
                view.display.via_span,
                false,
            );
            let rect = line(ui, 28.0);
            if button(
                ui,
                part(rect, 0.0, 206.0),
                "All Connections in Single Layer Mode",
                view.display.all_connections,
                true,
            ) {
                view.display.all_connections = !view.display.all_connections;
            }
            let rect = line(ui, 28.0);
            button(
                ui,
                part(rect, 0.0, 131.0),
                "  Net Color Override",
                false,
                false,
            );
            let rect = line(ui, 28.0);
            if button(
                ui,
                part(rect, 0.0, 218.0),
                "Use Layer Colors for Connection Drawing",
                view.display.layer_connections,
                true,
            ) {
                view.display.layer_connections = !view.display.layer_connections;
            }
            let rect = line(ui, 28.0);
            if button(
                ui,
                part(rect, 0.0, 176.0),
                "Repeated Net Names on Tracks",
                view.display.track_names,
                true,
            ) {
                view.display.track_names = !view.display.track_names;
            }
            button(ui, part(rect, 181.0, 91.0), "Special Strings", false, false);
        });
    }
}
fn three_d(view: &mut BoardView, ui: &mut egui::Ui, panel: &mut Panel) {
    group(ui, panel, "3D Settings", true, |ui, panel| {
        let p = &mut view.three_d.presentation;
        let rect = line(ui, 27.0);
        text(
            ui,
            part(rect, 0.0, 123.0),
            "Board thickness (Scale)",
            Align2::RIGHT_CENTER,
            TEXT,
        );
        let mut scale = (p.thickness_scale - 1.0) / 19.0 * 100.0;
        if slider(
            ui,
            part(rect, 130.0, rect.width() - 130.0),
            &mut scale,
            true,
        ) {
            p.thickness_scale = 1.0 + scale * 19.0 / 100.0;
        }
        let rect = line(ui, 27.0);
        text(
            ui,
            part(rect, 0.0, 123.0),
            "Colors",
            Align2::RIGHT_CENTER,
            TEXT,
        );
        if button(ui, part(rect, 130.0, 85.0), "Realistic", !p.by_layer, true) {
            p.by_layer = false;
        }
        if button(ui, part(rect, 216.0, 83.0), "By Layer", p.by_layer, true) {
            p.by_layer = true;
        }
        ui.add_space(5.0);
        border(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let rect = line(ui, 20.0);
            text(
                ui,
                part(rect, 8.0, 188.0),
                "Name",
                Align2::LEFT_CENTER,
                TEXT,
            );
            text(
                ui,
                part(rect, 211.0, 100.0),
                "Transparency",
                Align2::LEFT_CENTER,
                TEXT,
            );
            for (n, name) in [
                "Copper",
                "Core",
                "Top Silkscreen",
                "Bottom Silkscreen",
                "Top Solder Mask",
                "Bottom Solder Mask",
                "Top Paste Mask",
                "Bottom Paste Mask",
            ]
            .into_iter()
            .enumerate()
            {
                let rect = line(ui, 20.0);
                color(ui, part(rect, 9.0, 17.0), &mut p.colors[n], !p.by_layer);
                text(ui, part(rect, 28.0, 170.0), name, Align2::LEFT_CENTER, TEXT);
                let mut transparency = 100.0 - f32::from(p.opacity[n]) * 100.0 / 255.0;
                if slider(
                    ui,
                    part(rect, 210.0, rect.width() - 214.0),
                    &mut transparency,
                    true,
                ) {
                    p.opacity[n] = ((100.0 - transparency) * 2.55).round() as u8;
                }
            }
        });
        p.layer_colors = view.display.colors.clone();
        for l in &panel.catalog.layers {
            p.layer_colors
                .entry(l.id)
                .or_insert_with(|| super::layer_color(l.id));
        }
        ui.add_space(6.0);
    });
}

fn system_colors(view: &mut BoardView, ui: &mut egui::Ui, panel: &mut Panel) {
    group(ui, panel, "System Colors", false, |ui, _| {
        border(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let rect = line(ui, 20.0);
            let mut on = view.display.connections
                || view.display.pad_holes_visible
                || view.display.via_holes_visible
                || view.display.origin_visible
                || view.display.board_visible;
            if eye(ui, part(rect, 5.0, 18.0), &mut on, true).clicked() {
                view.display.connections = on;
                view.display.pad_holes_visible = on;
                view.display.via_holes_visible = on;
                view.display.origin_visible = on;
                view.display.board_visible = on;
            }
            text(
                ui,
                part(rect, 25.0, 200.0),
                "System Colors",
                Align2::LEFT_CENTER,
                TEXT,
            );
            for (n, name) in [
                "Connection Lines",
                "Selection / Highlight",
                "Pad Holes",
                "Via Holes",
                "Origin Marker",
                "Component Reference Point",
                "3D Body Reference Point",
                "Custom Snap Points",
                "DRC Error / Waived DRC Error Markers",
                "Violation / Waived Violation Markers",
                "Board Line / Area",
                "Sheet Line / Area Color",
                "Workspace in 2D Mode Start / End",
                "Workspace in 3D Mode Start / End",
                "First / Second Dimension Line",
                "Area / Touch Rectangle Selection",
                "Not Changed / Modified Comparison...",
                "Added / Removed Comparison Colors",
            ]
            .into_iter()
            .enumerate()
            {
                ui.push_id(name, |ui| {
                    let rect = line(ui, 20.0);
                    let enabled = matches!(n, 0..=4 | 10 | 12 | 13);
                    if n == 0 && view.connections.calculating() {
                        ui.interact(rect, ui.id().with("connections_status"), Sense::hover())
                            .on_hover_text("Updating connections…");
                    }
                    let mut on = match n {
                        0 => view.display.connections,
                        2 => view.display.pad_holes_visible,
                        3 => view.display.via_holes_visible,
                        4 => view.display.origin_visible,
                        10 => view.display.board_visible,
                        _ => true,
                    };
                    if eye(
                        ui,
                        part(rect, 4.0, 18.0),
                        &mut on,
                        enabled && !matches!(n, 1 | 12 | 13),
                    )
                    .clicked()
                    {
                        match n {
                            0 => view.display.connections = on,
                            2 => view.display.pad_holes_visible = on,
                            3 => view.display.via_holes_visible = on,
                            4 => view.display.origin_visible = on,
                            10 => view.display.board_visible = on,
                            _ => {}
                        }
                    }
                    let d = &mut view.display;
                    let (a, b) = match n {
                        0 => (&mut d.connection_color, None),
                        1 => (&mut d.selection_color, Some(&mut d.highlight_color)),
                        2 => (&mut d.hole_color, None),
                        3 => (&mut d.via_hole_color, None),
                        4 => (&mut d.origin_color, None),
                        10 => (&mut d.board_line, Some(&mut d.board_color)),
                        12 => (&mut d.background, Some(&mut d.background_end)),
                        13 => {
                            let [a, b] = &mut view.three_d.presentation.background;
                            (a, Some(b))
                        }
                        _ => (&mut d.highlight_color, None),
                    };
                    if let Some(b) = b {
                        color(ui, part(rect, 24.0, 17.0), a, enabled);
                        color(ui, part(rect, 44.0, 17.0), b, enabled);
                    } else {
                        color(ui, part(rect, 44.0, 17.0), a, enabled);
                    }
                    text(
                        ui,
                        part(rect, 64.0, rect.width() - 66.0),
                        name,
                        Align2::LEFT_CENTER,
                        if enabled { TEXT } else { Color32::GRAY },
                    );
                });
            }
        });
    });
}
fn advanced(view: &mut BoardView, ctx: &egui::Context, panel: &mut Panel) {
    let Some(editor) = panel.advanced.as_mut() else {
        return;
    };
    let mut close = false;
    let mut apply = false;
    let modal = egui::Modal::new(egui::Id::new("pcb_object_visibility"))
        .backdrop_color(Color32::TRANSPARENT)
        .frame(
            egui::Frame::NONE
                .fill(Color32::from_gray(76))
                .stroke(Stroke::new(1.0, BORDER)),
        )
        .show(ctx, |ui| {
            ui.set_width(708.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            let title = line(ui, 30.0);
            text(ui, title, "Object Visibility", Align2::CENTER_CENTER, TEXT);
            let x = Rect::from_min_size(
                title.right_top() - Vec2::new(36.0, 0.0),
                Vec2::new(36.0, 23.0),
            );
            ui.painter()
                .rect_filled(x, 0.0, Color32::from_rgb(180, 0, 0));
            text(ui, x, "×", Align2::CENTER_CENTER, Color32::WHITE);
            close |= ui
                .interact(x, ui.id().with("close"), Sense::click())
                .clicked();
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 18,
                    right: 18,
                    top: 8,
                    bottom: 10,
                })
                .show(ui, |ui| {
                    egui::Frame::NONE
                        .fill(Color32::from_gray(57))
                        .stroke(Stroke::new(1.0, BORDER))
                        .show(ui, |ui| {
                            egui::ScrollArea::both()
                                .max_height(385.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.spacing_mut().interact_size.y = 17.0;
                                    ui.spacing_mut().button_padding.y = 1.0;
                                    let width = 672.0;
                                    ui.set_min_width(width);
                                    let rect = line(ui, 20.0);
                                    text(
                                        ui,
                                        part(rect, 0.0, 120.0),
                                        "Layer",
                                        Align2::LEFT_CENTER,
                                        TEXT,
                                    );
                                    for (n, (_, name)) in OBJECTS.iter().skip(1).enumerate() {
                                        text(
                                            ui,
                                            matrix_cell(rect, n, 0.0),
                                            name,
                                            Align2::CENTER_CENTER,
                                            TEXT,
                                        );
                                    }
                                    for (group, label) in [
                                        (0, "Signal Layers"),
                                        (3, "Other Layers"),
                                        (4, "Silkscreen Layers"),
                                        (5, "Mask Layers"),
                                        (2, "Mechanical Layers"),
                                    ] {
                                        let bold = egui::FontFamily::Name("ui-bold".into());
                                        let family = if ui.fonts(|f| f.families().contains(&bold)) {
                                            bold
                                        } else {
                                            egui::FontFamily::Proportional
                                        };
                                        egui::CollapsingHeader::new(
                                            egui::RichText::new(label).family(family).size(11.0),
                                        )
                                        .default_open(false)
                                        .show(ui, |ui| {
                                            for l in &panel.catalog.layers {
                                                let g = if l.copper {
                                                    0
                                                } else if super::catalog::mechanical(l.id).is_some()
                                                {
                                                    2
                                                } else if matches!(l.id, 33 | 34) {
                                                    4
                                                } else if (35..=38).contains(&l.id) {
                                                    5
                                                } else {
                                                    3
                                                };
                                                if g != group || editor.used_only && !l.used {
                                                    continue;
                                                }
                                                let rect = line(ui, 17.0);
                                                ui.painter().rect_filled(
                                                    Rect::from_min_size(
                                                        rect.min + Vec2::new(2., 3.),
                                                        Vec2::new(10., 11.),
                                                    ),
                                                    0.,
                                                    view.color(l.id),
                                                );
                                                text(
                                                    ui,
                                                    part(rect, 17.0, 85.0),
                                                    &l.name,
                                                    Align2::LEFT_CENTER,
                                                    TEXT,
                                                );
                                                for (n, (kind, _)) in
                                                    OBJECTS.iter().skip(1).enumerate()
                                                {
                                                    let cell = matrix_cell(rect, n, 18.0);
                                                    ui.painter().rect_stroke(
                                                        cell,
                                                        0.0,
                                                        Stroke::new(0.5, Color32::from_gray(51)),
                                                        StrokeKind::Inside,
                                                    );
                                                    if editor.selected.contains(&(l.id, *kind)) {
                                                        ui.painter()
                                                            .rect_filled(cell, 0.0, SELECTED);
                                                    }
                                                    let alpha = editor
                                                        .opacity
                                                        .get(&l.id)
                                                        .map_or(255, |v| v[*kind]);
                                                    text(
                                                        ui,
                                                        cell,
                                                        &format!(
                                                            "{:.0}%",
                                                            100.0
                                                                - f32::from(alpha) * 100.0 / 255.0
                                                        ),
                                                        Align2::CENTER_CENTER,
                                                        TEXT,
                                                    );
                                                    if supported(*kind)
                                                        && ui
                                                            .interact(
                                                                cell,
                                                                ui.id().with((l.id, *kind)),
                                                                Sense::click(),
                                                            )
                                                            .clicked()
                                                    {
                                                        if !ui.input(|i| i.modifiers.ctrl) {
                                                            editor.selected.clear();
                                                        }
                                                        editor.selected.insert((l.id, *kind));
                                                    }
                                                }
                                            }
                                        });
                                    }
                                });
                        });
                    let row = line(ui, 40.0);
                    child(ui, part(row, 4.0, 230.0), "used_only", |ui| {
                        ui.checkbox(
                            &mut editor.used_only,
                            egui::RichText::new("Only show used layers").size(11.0),
                        );
                    });
                    text(
                        ui,
                        part(row, 260.0, 148.0),
                        "Transparency for selected:",
                        Align2::RIGHT_CENTER,
                        if editor.selected.is_empty() {
                            Color32::GRAY
                        } else {
                            TEXT
                        },
                    );
                    let mut n = editor
                        .selected
                        .iter()
                        .next()
                        .map(|(id, k)| {
                            100.0
                                - f32::from(editor.opacity.get(id).map_or(255, |a| a[*k])) * 100.0
                                    / 255.0
                        })
                        .unwrap_or(0.0);
                    let rect = part(row, 420.0, 180.0);
                    if slider(ui, rect, &mut n, !editor.selected.is_empty()) {
                        for (id, k) in &editor.selected {
                            editor.opacity.entry(*id).or_insert([255; 23])[*k] =
                                ((100.0 - n) * 2.55).round() as u8;
                        }
                    }
                    child(ui, part(row, 606.0, 52.0), "transparency_value", |ui| {
                        if ui
                            .add_enabled(
                                !editor.selected.is_empty(),
                                egui::DragValue::new(&mut n)
                                    .range(0.0..=100.0)
                                    .suffix("%")
                                    .speed(1.),
                            )
                            .changed()
                        {
                            for (id, k) in &editor.selected {
                                editor.opacity.entry(*id).or_insert([255; 23])[*k] =
                                    ((100.0 - n) * 2.55).round() as u8;
                            }
                        }
                    });
                });
            let row = line(ui, 40.0);
            apply |= button(
                ui,
                Rect::from_center_size(
                    row.right_center() - Vec2::new(128.0, 0.0),
                    Vec2::new(75.0, 24.0),
                ),
                "Apply",
                false,
                true,
            );
            close |= button(
                ui,
                Rect::from_center_size(
                    row.right_center() - Vec2::new(46.0, 0.0),
                    Vec2::new(75.0, 24.0),
                ),
                "Cancel",
                false,
                true,
            );
        });
    close |= modal.is_top_modal
        && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
    if apply {
        view.display.opacity = editor.opacity.clone();
        close = true;
    }
    if close {
        panel.advanced = None;
    }
}
impl BoardView {
    pub fn configuration_modal_open(&self) -> bool {
        self.display.panel.advanced.is_some()
    }
    pub(crate) fn sync_3d_display(&mut self, board: &Board) {
        let p = &mut self.three_d.presentation;
        p.active_layer = self.active_layer;
        p.dim = self.display.dim;
        p.highlighted_dim = self.display.highlight_dim;
        p.masked_dim = self.display.masked_dim;
        p.selection_color = self.display.selection_color;
        p.highlight_color = self.display.highlight_color;
        self.three_d.highlighted =
            (self.highlight_net.is_some() || !self.highlight_nets.is_empty()).then(|| {
                board
                    .items
                    .iter()
                    .filter(|i| {
                        board.item_net(i).is_some_and(|net| {
                            self.highlight_net == Some(net) || self.highlight_nets.contains(&net)
                        })
                    })
                    .map(|i| i.id)
                    .collect()
            });
    }
    pub fn configuration_panel(
        &mut self,
        ui: &mut egui::Ui,
        board: &Board,
        grid: &mut bool,
        camera: &mut crate::Camera,
    ) -> Option<u16> {
        self.initialize(board);
        let mut panel = std::mem::take(&mut self.display.panel);
        if panel.catalog_revision != self.three_d.revision {
            panel.catalog = Arc::new(Catalog::from_board(board));
            panel.catalog_revision = self.three_d.revision;
        }
        let mut requested = None;
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add_space(8.0);
        let rect = line(ui, 24.0);
        child(ui, rect, "search", |ui| {
            ui.add_sized(
                rect.size(),
                egui::TextEdit::singleline(&mut panel.search)
                    .hint_text("Search")
                    .margin(egui::Margin {
                        left: 24,
                        right: 4,
                        top: 3,
                        bottom: 3,
                    }),
            );
        });
        let c = rect.left_center() + Vec2::new(10.0, -1.0);
        ui.painter().circle_stroke(c, 3.0, Stroke::new(1.0, TEXT));
        ui.painter().line_segment(
            [c + Vec2::splat(2.0), c + Vec2::splat(5.0)],
            Stroke::new(1.0, TEXT),
        );
        ui.add_space(12.0);
        let rect = line(ui, 22.0);
        for (index, label, x, w) in [
            (0, "Layers & Colors", 7.0, 94.0),
            (1, "View Options", 101.0, 80.0),
        ] {
            if tab(
                ui,
                part(rect, x, w),
                label,
                usize::from(self.display.options_tab) == index && !panel.section,
            ) {
                self.display.options_tab = index == 1;
                panel.section = false;
            }
        }
        if self.three_d.enabled && tab(ui, part(rect, 181.0, 79.0), "Section View", panel.section) {
            panel.section = true;
        }
        let query = panel.search.to_lowercase();
        ui.spacing_mut().scroll = egui::style::ScrollStyle {
            floating: false,
            bar_width: 6.0,
            bar_inner_margin: 6.0,
            bar_outer_margin: 0.0,
            ..egui::style::ScrollStyle::solid()
        };
        egui::ScrollArea::vertical()
            .id_salt((
                "view_configuration",
                self.display.options_tab,
                panel.section,
            ))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(11.0);
                if panel.section {
                    group(ui, &mut panel, "General Settings", true, |ui, _| {
                        let rect = line(ui, 28.0);
                        text(
                            ui,
                            part(rect, 0.0, 99.0),
                            "Section View",
                            Align2::RIGHT_CENTER,
                            TEXT,
                        );
                        for (label, x) in [("Edit", 106.0), ("On", 167.0), ("Off", 228.0)] {
                            button(ui, part(rect, x, 60.0), label, label == "Off", false);
                        }
                        let rect = line(ui, 28.0);
                        text(
                            ui,
                            part(rect, 0.0, 99.0),
                            "Invert Cutout",
                            Align2::RIGHT_CENTER,
                            TEXT,
                        );
                        button(ui, part(rect, 106.0, 91.0), "On", false, false);
                        button(ui, part(rect, 198.0, 91.0), "Off", true, false);
                        for (name, c) in [
                            ("Plane X", Color32::RED),
                            ("Plane Y", Color32::GREEN),
                            ("Plane Z", Color32::BLUE),
                        ] {
                            let rect = line(ui, 30.0);
                            let mut on = true;
                            checkbox(ui, part(rect, 105.0, 20.0), &mut on, false);
                            text(
                                ui,
                                part(rect, 126.0, 50.0),
                                name,
                                Align2::LEFT_CENTER,
                                Color32::GRAY,
                            );
                            let mut c = c;
                            color(ui, part(rect, 177.0, 20.0), &mut c, false);
                            button(ui, part(rect, 202.0, 23.0), "↔", false, false);
                        }
                    });
                } else if self.display.options_tab {
                    options(self, ui, &mut panel, grid, &query);
                } else {
                    if query.is_empty()
                        || matches(&query, "Layers")
                        || panel
                            .catalog
                            .layers
                            .iter()
                            .any(|l| matches(&query, &l.name))
                    {
                        layers(self, ui, &mut panel, camera, &mut requested, &query);
                    }
                    if query.is_empty() || matches(&query, "System Colors") {
                        system_colors(self, ui, &mut panel);
                    }
                }
            });
        advanced(self, ui.ctx(), &mut panel);
        self.display.panel = panel;
        requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn texts(shapes: &[egui::epaint::ClippedShape], value: &str) -> Vec<Pos2> {
        fn collect(shape: &egui::Shape, value: &str, out: &mut Vec<Pos2>) {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == value => {
                    out.push(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        collect(s, value, out);
                    }
                }
                _ => {}
            }
        }
        let mut result = vec![];
        for s in shapes {
            collect(&s.shape, value, &mut result);
        }
        result.sort_by(|a, b| a.y.total_cmp(&b.y));
        result
    }
    fn click(pos: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    }
    #[test]
    fn general_buttons_with_same_labels_change_only_their_own_setting() {
        let ctx = egui::Context::default();
        let board = Board::default();
        let original = board.clone();
        let mut view = BoardView::default();
        view.initialize(&board);
        let mut panel = view.display.panel.clone();
        let mut grid = true;
        let mut frame = |view: &mut BoardView, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(330., 1100.))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| options(view, ui, &mut panel, &mut grid, ""));
                },
            )
        };
        let out = frame(&mut view, vec![]);
        let ons = texts(&out.shapes, "On");
        assert_eq!(ons.len(), 2);
        frame(&mut view, click(ons[1]));
        assert_eq!(view.single_layer, 1);
        assert!(!view.three_d.enabled);
        frame(&mut view, click(ons[0]));
        assert!(view.three_d.enabled);
        assert_eq!(view.single_layer, 1);
        let out = frame(&mut view, vec![]);
        frame(&mut view, click(texts(&out.shapes, "On")[1]));
        assert!(view.three_d.presentation.single_layer);
        assert_eq!(view.single_layer, 1);
        let out = frame(&mut view, vec![]);
        let offs = texts(&out.shapes, "Off");
        frame(&mut view, click(offs[0]));
        assert!(!view.three_d.enabled);
        assert_eq!(view.single_layer, 1);
        assert_eq!(board, original);
    }
    #[test]
    fn eye_and_transparency_slider_in_same_row_have_independent_pointer_interaction() {
        let ctx = egui::Context::default();
        let mut visible = true;
        let mut alpha = 0.;
        let mut frame = |events| {
            let out = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(330., 100.))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let r = line(ui, 20.);
                        eye(ui, part(r, 0., 20.), &mut visible, true);
                        slider(ui, part(r, 170., 43.), &mut alpha, true);
                    });
                },
            );
            (out, visible, alpha)
        };
        frame(vec![]);
        let (_, on, value) = frame(click(Pos2::new(215., 18.)));
        assert!(on);
        assert!(value > 80.);
        let (_, on, same) = frame(click(Pos2::new(18., 18.)));
        assert!(!on);
        assert_eq!(same, value);
        let a = Pos2::new(184., 18.);
        let b = Pos2::new(215., 18.);
        assert!(frame(click(a)).2 < 20.);
        let (_, on, dragged) = frame(vec![
            egui::Event::PointerMoved(a),
            egui::Event::PointerButton {
                pos: a,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerMoved(b),
            egui::Event::PointerButton {
                pos: b,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        assert!(!on);
        assert!(dragged > 80.);
    }
    #[test]
    fn aggregate_transparency_spans_layers_and_hidden_kind_keeps_opacity() {
        let board = Board::default();
        let catalog = Catalog::from_board(&board);
        let mut settings = Settings::from_board(&board);
        settings.opacity.insert(TOP, [204; 23]);
        assert_eq!(aggregate(&settings, &catalog, 2), None);
        set_opacity(&mut settings, &catalog, 2, 127);
        assert_eq!(aggregate(&settings, &catalog, 2), Some(127));
        settings.hidden[2] = true;
        let pad = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: eda_geom::Point::default(),
                size: eda_geom::Point::mm(1, 1),
                hole: 0,
                rotation: 0.,
                shape: PadShape::Round,
                plated: true,
                bottom_size: None,
                middle_size: None,
                name: "1".into(),
            },
        );
        assert_eq!(settings.alpha(&pad), 0);
        settings.hidden[2] = false;
        assert_eq!(settings.alpha(&pad), 127);
        assert_eq!(settings.opacity[&TOP][4], 204);
    }
    #[test]
    fn advanced_cancel_discards_changes_and_apply_commits_only_view_opacity() {
        let ctx = egui::Context::default();
        let board = Board::default();
        let before = board.clone();
        let mut view = BoardView::default();
        view.initialize(&board);
        let original = view.display.opacity.clone();
        let mut panel = view.display.panel.clone();
        let editor = || Advanced {
            opacity: BTreeMap::from([(TOP, [100; 23])]),
            selected: BTreeSet::new(),
            used_only: true,
        };
        panel.advanced = Some(editor());
        let frame = |view: &mut BoardView, panel: &mut Panel, events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1707., 1027.))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| advanced(view, ctx, panel),
            )
        };
        for _ in 0..4 {
            frame(&mut view, &mut panel, vec![]);
        }
        let out = frame(&mut view, &mut panel, vec![]);
        frame(
            &mut view,
            &mut panel,
            click(texts(&out.shapes, "Cancel")[0]),
        );
        assert!(panel.advanced.is_none());
        assert_eq!(view.display.opacity, original);
        panel.advanced = Some(editor());
        for _ in 0..4 {
            frame(&mut view, &mut panel, vec![]);
        }
        let out = frame(&mut view, &mut panel, vec![]);
        frame(&mut view, &mut panel, click(texts(&out.shapes, "Apply")[0]));
        assert!(panel.advanced.is_none());
        assert_eq!(view.display.opacity[&TOP], [100; 23]);
        assert_eq!(board, before);
    }
}
