use egui::{Color32, FontId, Stroke, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(100, 139, 167);
pub const MUTED: Color32 = Color32::from_rgb(166, 166, 166);
pub const PANEL: Color32 = Color32::from_rgb(58, 58, 58);
pub const GROUP: Color32 = Color32::from_rgb(65, 65, 65);
pub const FIELD: Color32 = Color32::from_rgb(43, 43, 43);
pub const BORDER: Color32 = Color32::from_rgb(35, 35, 35);

pub fn search_field(ui: &mut egui::Ui, text: &mut String, width: f32) -> egui::Response {
    let response = ui.add_sized(
        [width, 22.0],
        egui::TextEdit::singleline(text)
            .hint_text("Search")
            .margin(egui::Margin {
                left: 25,
                right: 4,
                top: 3,
                bottom: 3,
            }),
    );
    draw_icon(
        ui.painter(),
        egui::Rect::from_center_size(
            response.rect.left_center() + egui::vec2(12.0, 0.0),
            egui::vec2(11.0, 11.0),
        ),
        Icon::Search,
        true,
    );
    response
}

#[derive(Clone, Copy)]
pub enum Icon {
    New,
    Open,
    Save,
    SaveAll,
    Undo,
    Redo,
    Settings,
    Home,
    Pin,
    Close,
    Folder,
    Minimize,
    Maximize,
    Search,
    Filter,
    Select,
    Grid,
    Fit,
    Track,
    Pad,
    Via,
    Component,
    Polygon,
    Text,
    Line,
    Dimension,
    Chart,
    Chip,
}

pub fn icon_button(ui: &mut egui::Ui, icon: Icon, selected: bool, enabled: bool) -> egui::Response {
    let width = if matches!(icon, Icon::Minimize | Icon::Maximize | Icon::Close)
        && ui.max_rect().top() < 24.0
    {
        35.0
    } else {
        25.0
    };
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width, 23.0),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    if selected || (enabled && response.hovered()) {
        ui.painter().rect_filled(
            rect,
            1.0,
            if selected {
                ACCENT
            } else {
                Color32::from_gray(82)
            },
        );
    }
    draw_icon(
        ui.painter(),
        egui::Rect::from_center_size(rect.center(), egui::vec2(17.0, 15.0)),
        icon,
        enabled,
    );
    response
}

pub fn draw_icon(p: &egui::Painter, rect: egui::Rect, icon: Icon, enabled: bool) {
    let c = if enabled {
        Color32::from_gray(212)
    } else {
        Color32::from_gray(119)
    };
    let blue = if enabled {
        Color32::from_rgb(160, 199, 220)
    } else {
        Color32::from_gray(105)
    };
    let yellow = if enabled {
        Color32::from_rgb(216, 185, 111)
    } else {
        Color32::from_gray(107)
    };
    let point =
        |x: f32, y: f32| rect.min + egui::vec2(x / 16.0 * rect.width(), y / 16.0 * rect.height());
    let line = |a: (f32, f32), b: (f32, f32), color| {
        p.line_segment([point(a.0, a.1), point(b.0, b.1)], Stroke::new(1.3, color));
    };
    match icon {
        Icon::Close => {
            line((3.0, 3.0), (13.0, 13.0), c);
            line((13.0, 3.0), (3.0, 13.0), c);
        }
        Icon::Minimize => line((3.0, 11.0), (13.0, 11.0), c),
        Icon::Maximize => {
            p.rect_stroke(
                egui::Rect::from_min_max(point(3.0, 3.0), point(13.0, 13.0)),
                0.0,
                Stroke::new(1.0, c),
                egui::StrokeKind::Inside,
            );
        }
        Icon::Open | Icon::Folder => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(1.0, 4.0),
                    point(6.0, 4.0),
                    point(8.0, 6.0),
                    point(15.0, 6.0),
                    point(13.0, 14.0),
                    point(1.0, 14.0),
                ],
                if matches!(icon, Icon::Folder) {
                    Color32::from_rgb(155, 194, 136)
                } else {
                    yellow
                },
                Stroke::new(1.0, c),
            ));
        }
        Icon::Save | Icon::SaveAll => {
            p.rect_filled(
                egui::Rect::from_min_max(point(2.0, 1.0), point(14.0, 15.0)),
                1.0,
                blue,
            );
            p.rect_filled(
                egui::Rect::from_min_max(point(5.0, 2.0), point(11.0, 6.0)),
                0.0,
                FIELD,
            );
            p.rect_filled(
                egui::Rect::from_min_max(point(5.0, 9.0), point(12.0, 14.0)),
                0.0,
                c,
            );
            if matches!(icon, Icon::SaveAll) {
                line((0.0, 4.0), (0.0, 15.0), c);
            }
        }
        Icon::New => {
            p.rect_filled(
                egui::Rect::from_min_max(point(3.0, 1.0), point(13.0, 15.0)),
                0.0,
                c,
            );
            line((8.0, 5.0), (8.0, 11.0), blue);
            line((5.0, 8.0), (11.0, 8.0), blue);
        }
        Icon::Undo | Icon::Redo => {
            let flip = matches!(icon, Icon::Redo);
            let x = if flip { 13.0 } else { 3.0 };
            line((x, 5.0), (8.0, 2.0), c);
            line((x, 5.0), (8.0, 9.0), c);
            p.line(
                vec![
                    point(x, 5.0),
                    point(if flip { 4.0 } else { 12.0 }, 5.0),
                    point(if flip { 3.0 } else { 13.0 }, 11.0),
                ],
                Stroke::new(1.5, c),
            );
        }
        Icon::Search => {
            p.circle_stroke(point(6.0, 6.0), rect.width() * 0.27, Stroke::new(1.2, c));
            line((10.0, 10.0), (15.0, 15.0), c);
        }
        Icon::Settings => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let v = egui::Vec2::angled(a);
                p.line_segment(
                    [rect.center() + v * 4.0, rect.center() + v * 8.0],
                    Stroke::new(2.0, c),
                );
            }
            p.circle_stroke(rect.center(), 4.0, Stroke::new(2.0, blue));
        }
        Icon::Home => {
            p.add(egui::Shape::convex_polygon(
                vec![point(1.0, 8.0), point(8.0, 1.0), point(15.0, 8.0)],
                c,
                Stroke::NONE,
            ));
            p.rect_filled(
                egui::Rect::from_min_max(point(4.0, 7.0), point(12.0, 15.0)),
                0.0,
                c,
            );
        }
        Icon::Pin => {
            line((5.0, 2.0), (12.0, 2.0), c);
            line((6.0, 2.0), (6.0, 8.0), c);
            line((11.0, 2.0), (11.0, 8.0), c);
            line((3.0, 9.0), (13.0, 9.0), c);
            line((8.0, 9.0), (8.0, 15.0), c);
        }
        Icon::Filter => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(1.0, 2.0),
                    point(15.0, 2.0),
                    point(9.0, 8.0),
                    point(9.0, 14.0),
                    point(6.0, 14.0),
                    point(6.0, 8.0),
                ],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Select => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(3.0, 1.0),
                    point(13.0, 9.0),
                    point(8.0, 10.0),
                    point(5.0, 15.0),
                ],
                c,
                Stroke::new(1.0, FIELD),
            ));
        }
        Icon::Grid | Icon::Fit => {
            line((8.0, 0.0), (8.0, 16.0), blue);
            line((0.0, 8.0), (16.0, 8.0), blue);
            if matches!(icon, Icon::Grid) {
                for x in [3.0, 13.0] {
                    for y in [3.0, 13.0] {
                        p.circle_filled(point(x, y), 1.0, c);
                    }
                }
            }
        }
        Icon::Track | Icon::Line => {
            p.line(
                vec![
                    point(1.0, 14.0),
                    point(6.0, 14.0),
                    point(12.0, 3.0),
                    point(15.0, 3.0),
                ],
                Stroke::new(2.0, blue),
            );
        }
        Icon::Pad | Icon::Via => {
            p.circle_filled(rect.center(), 6.0, yellow);
            p.circle_filled(rect.center(), 2.0, FIELD);
        }
        Icon::Component | Icon::Chip => {
            p.rect_filled(
                egui::Rect::from_min_max(point(4.0, 3.0), point(12.0, 13.0)),
                1.0,
                blue,
            );
            for y in [4.0, 8.0, 12.0] {
                line((0.0, y), (4.0, y), c);
                line((12.0, y), (16.0, y), c);
            }
        }
        Icon::Polygon => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(1.0, 5.0),
                    point(10.0, 1.0),
                    point(15.0, 8.0),
                    point(12.0, 15.0),
                    point(2.0, 13.0),
                ],
                blue,
                Stroke::new(1.0, c),
            ));
        }
        Icon::Text => {
            p.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "A",
                FontId::proportional(18.0),
                blue,
            );
        }
        Icon::Dimension => {
            line((1.0, 3.0), (1.0, 15.0), yellow);
            line((15.0, 3.0), (15.0, 15.0), yellow);
            line((1.0, 8.0), (15.0, 8.0), blue);
        }
        Icon::Chart => {
            for (x, h) in [(2.0, 5.0), (6.0, 11.0), (10.0, 8.0)] {
                p.rect_filled(
                    egui::Rect::from_min_max(point(x, 15.0 - h), point(x + 3.0, 15.0)),
                    0.0,
                    blue,
                );
            }
        }
    }
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let override_path = std::env::var_os("PCBCRAFT_UI_FONT").map(std::path::PathBuf::from);
    #[cfg(windows)]
    let system_font =
        std::env::var_os("WINDIR").map(|p| std::path::PathBuf::from(p).join("Fonts/segoeui.ttf"));
    #[cfg(not(windows))]
    let system_font: Option<std::path::PathBuf> = None;
    let regular = override_path
        .or(system_font)
        .and_then(|p| std::fs::read(p).ok())
        .map(egui::FontData::from_owned)
        .unwrap_or_else(|| {
            egui::FontData::from_static(include_bytes!("../../../assets/fonts/Selawik-Regular.ttf"))
        });
    fonts
        .font_data
        .insert("ui".into(), std::sync::Arc::new(regular));
    let bold_override = std::env::var_os("PCBCRAFT_UI_FONT_SEMIBOLD").map(std::path::PathBuf::from);
    #[cfg(windows)]
    let bold_system =
        std::env::var_os("WINDIR").map(|p| std::path::PathBuf::from(p).join("Fonts/seguisb.ttf"));
    #[cfg(not(windows))]
    let bold_system: Option<std::path::PathBuf> = None;
    let bold = bold_override
        .or(bold_system)
        .and_then(|p| std::fs::read(p).ok())
        .map(egui::FontData::from_owned)
        .unwrap_or_else(|| {
            egui::FontData::from_static(include_bytes!(
                "../../../assets/fonts/Selawik-Semibold.ttf"
            ))
        });
    fonts
        .font_data
        .insert("ui-bold".into(), std::sync::Arc::new(bold));
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "ui".into());
    fonts.families.insert(
        egui::FontFamily::Name("ui-bold".into()),
        vec!["ui-bold".into(), "ui".into()],
    );
    ctx.set_fonts(fonts);
}

pub fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(19.0, 19.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect.shrink(2.0), 0.0, Color32::from_rgb(44, 130, 98));
    p.rect_stroke(
        rect.shrink(3.0),
        2.0,
        Stroke::new(1.0, Color32::from_rgb(157, 211, 193)),
        egui::StrokeKind::Inside,
    );
    p.line_segment(
        [
            rect.left_center() + egui::vec2(5.0, 0.0),
            rect.right_center() - egui::vec2(5.0, 0.0),
        ],
        Stroke::new(1.5, ACCENT),
    );
    for x in [6.0, 17.0] {
        for y in [6.0, 13.0] {
            p.circle_filled(rect.min + egui::vec2(x, y), 2.0, ACCENT);
        }
    }
}

pub fn apply(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = Color32::from_rgb(65, 65, 65);
    visuals.extreme_bg_color = Color32::from_rgb(44, 44, 44);
    visuals.faint_bg_color = Color32::from_rgb(68, 68, 68);
    visuals.disabled_alpha = 0.7;
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(212, 212, 212));
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(65, 69, 74));
    visuals.widgets.inactive.bg_fill = Color32::from_gray(68);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(49);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(70, 82, 94);
    visuals.widgets.active.bg_fill = Color32::from_rgb(62, 94, 119);
    visuals.selection.bg_fill = Color32::from_rgb(92, 121, 142);
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(4.0, 3.0);
        style.spacing.button_padding = egui::vec2(5.0, 2.0);
        style.spacing.interact_size.y = 20.0;
        style.spacing.window_margin = egui::Margin::same(6);
        style.spacing.menu_margin = egui::Margin::same(3);
        for widget in [
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.noninteractive,
        ] {
            widget.corner_radius = egui::CornerRadius::same(1);
        }
        style
            .text_styles
            .insert(TextStyle::Body, FontId::proportional(12.0));
        style
            .text_styles
            .insert(TextStyle::Button, FontId::proportional(12.0));
        style
            .text_styles
            .insert(TextStyle::Small, FontId::proportional(11.0));
        style
            .text_styles
            .insert(TextStyle::Heading, FontId::proportional(21.0));
    });
}

pub fn heading(ui: &mut egui::Ui, title: &str, detail: &str) {
    ui.add_space(5.0);
    ui.label(egui::RichText::new(title).strong().size(13.0));
    if !detail.is_empty() {
        ui.label(egui::RichText::new(detail).small().color(MUTED));
    }
    ui.add_space(3.0);
    ui.separator();
}
