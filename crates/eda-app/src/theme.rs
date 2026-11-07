use egui::{Color32, FontId, Stroke, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(100, 139, 167);
pub const MUTED: Color32 = Color32::from_rgb(166, 166, 166);
pub const PANEL: Color32 = Color32::from_rgb(58, 58, 58);
pub const GROUP: Color32 = Color32::from_rgb(65, 65, 65);
pub const FIELD: Color32 = Color32::from_rgb(43, 43, 43);
pub const BORDER: Color32 = Color32::from_rgb(35, 35, 35);

pub fn search_field(ui: &mut egui::Ui, text: &mut String, width: f32) -> egui::Response {
    search_field_height(ui, text, width, 22.0)
}

pub fn search_field_height(
    ui: &mut egui::Ui,
    text: &mut String,
    width: f32,
    height: f32,
) -> egui::Response {
    let response = ui.add_sized(
        [width, height],
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
    Validate,
    Print,
    Pdf,
    New,
    Open,
    Save,
    SaveAll,
    Undo,
    Redo,
    Settings,
    Home,
    Close,
    Folder,
    Minimize,
    Maximize,
    Search,
    Filter,
    LibraryComponent,
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
        Icon::Print => {
            p.rect_filled(
                egui::Rect::from_min_max(point(2., 5.), point(14., 12.)),
                0.,
                c,
            );
            p.rect_filled(
                egui::Rect::from_min_max(point(4., 0.), point(12., 6.)),
                0.,
                Color32::WHITE,
            );
            p.rect_filled(
                egui::Rect::from_min_max(point(4., 9.), point(12., 16.)),
                0.,
                Color32::WHITE,
            );
            for y in [11., 13., 15.] {
                line((5., y), (11., y), blue);
            }
        }
        Icon::Pdf => {
            p.rect_filled(rect, 0., Color32::WHITE);
            p.rect_filled(
                egui::Rect::from_min_max(point(0., 0.), point(13., 4.)),
                0.,
                Color32::from_rgb(190, 55, 55),
            );
            p.rect_filled(
                egui::Rect::from_min_max(point(2., 6.), point(8., 15.)),
                0.,
                Color32::from_rgb(100, 170, 135),
            );
            line((9., 6.), (15., 12.), blue);
            line((15., 12.), (9., 15.), blue);
        }
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
        Icon::Validate => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(0., 6.),
                    point(5., 6.),
                    point(7., 8.),
                    point(15., 8.),
                    point(13., 15.),
                    point(0., 15.),
                ],
                yellow,
                Stroke::new(0.7, c),
            ));
            p.rect_filled(
                egui::Rect::from_min_max(point(2., 0.), point(10., 10.)),
                0.,
                Color32::from_gray(235),
            );
            for y in [2., 4., 6.] {
                line((3., y), (9., y), Color32::from_rgb(110, 150, 178));
            }
            p.add(egui::Shape::convex_polygon(
                vec![
                    point(8., 3.),
                    point(12., 3.),
                    point(12., 0.),
                    point(17., 5.),
                    point(12., 10.),
                    point(12., 7.),
                    point(8., 7.),
                ],
                blue,
                Stroke::NONE,
            ));
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
        Icon::Chip | Icon::LibraryComponent => {
            let body = if matches!(icon, Icon::LibraryComponent) {
                yellow
            } else {
                blue
            };
            p.rect_filled(
                egui::Rect::from_min_max(point(4.0, 3.0), point(12.0, 13.0)),
                1.0,
                body,
            );
            for y in [4.0, 8.0, 12.0] {
                line((0.0, y), (4.0, y), c);
                line((12.0, y), (16.0, y), c);
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
    // Match family names, not filenames or a shared "GOST" substring. In
    // particular GOST Type AU is not Altium GOST Type A. Missing document
    // faces use Arial/Liberation, independently of UI fonts on both platforms.
    let mut system = fontdb::Database::new();
    system.load_system_fonts();
    let mut user = fontdb::Database::new();
    if let Some(dir) = std::env::var_os("PCBCRAFT_SCHEMATIC_FONT_DIR") {
        user.load_fonts_dir(dir);
    }
    for (i, bytes) in [
        include_bytes!("../../../assets/fonts/liberation/LiberationSans-Regular.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/liberation/LiberationSans-Bold.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/liberation/LiberationSans-Italic.ttf").as_slice(),
        include_bytes!("../../../assets/fonts/liberation/LiberationSans-BoldItalic.ttf").as_slice(),
    ]
    .into_iter()
    .enumerate()
    {
        fonts.font_data.insert(
            format!("sch-fallback-{i}"),
            std::sync::Arc::new(egui::FontData::from_static(bytes)),
        );
    }
    for (family, names) in [
        ("sch-serif", &["Times New Roman", "Liberation Serif"][..]),
        ("sch-sans", &["Arial", "Liberation Sans"][..]),
        ("sch-mono", &["Courier New", "Liberation Mono"][..]),
        (
            "sch-gost",
            &["GOST Type AU", "Arial", "Liberation Sans"][..],
        ),
        (
            "sch-altium-gost",
            &["Altium GOST Type A", "Arial", "Liberation Sans"][..],
        ),
    ] {
        for i in 0..4 {
            let key = format!(
                "{family}{}{}",
                if i % 2 == 1 { "-bold" } else { "" },
                if i >= 2 { "-italic" } else { "" }
            );
            let data = names.iter().find_map(|name| {
                let families = [fontdb::Family::Name(name)];
                let query = fontdb::Query {
                    families: &families,
                    weight: if i % 2 == 1 {
                        fontdb::Weight::BOLD
                    } else {
                        fontdb::Weight::NORMAL
                    },
                    style: if i >= 2 {
                        fontdb::Style::Italic
                    } else {
                        fontdb::Style::Normal
                    },
                    ..Default::default()
                };
                [&user, &system].into_iter().find_map(|db| {
                    db.query(&query).and_then(|id| {
                        db.with_face_data(id, |bytes, index| {
                            let mut data = egui::FontData::from_owned(bytes.to_vec());
                            data.index = index;
                            data
                        })
                    })
                })
            });
            let mut fallbacks = vec![];
            if let Some(data) = data {
                fonts
                    .font_data
                    .insert(key.clone(), std::sync::Arc::new(data));
                fallbacks.push(key.clone());
            }
            fallbacks.push(format!("sch-fallback-{i}"));
            fonts
                .families
                .insert(egui::FontFamily::Name(key.into()), fallbacks);
        }
    }
    ctx.set_fonts(fonts);
}

pub fn apply(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);
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
        style.spacing.menu_margin = egui::Margin {
            left: 3,
            right: 3,
            top: 1,
            bottom: 1,
        };
        style.visuals.menu_corner_radius = egui::CornerRadius::ZERO;
        style.visuals.popup_shadow = egui::epaint::Shadow {
            offset: [2, 2],
            blur: 3,
            spread: 0,
            color: Color32::from_black_alpha(80),
        };
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
