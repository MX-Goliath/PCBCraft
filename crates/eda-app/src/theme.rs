use egui::{Color32, FontId, Stroke, TextStyle};

pub const ACCENT: Color32 = Color32::from_rgb(100, 139, 167);
pub const MUTED: Color32 = Color32::from_rgb(166, 166, 166);
pub const PANEL: Color32 = Color32::from_rgb(58, 58, 58);

pub fn logo(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(23.0, 23.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_stroke(
        rect.shrink(3.0),
        2.0,
        Stroke::new(1.4, ACCENT),
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
        for y in [6.0, 17.0] {
            p.circle_filled(rect.min + egui::vec2(x, y), 2.0, ACCENT);
        }
    }
}

pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = Color32::from_rgb(65, 65, 65);
    visuals.extreme_bg_color = Color32::from_rgb(44, 44, 44);
    visuals.faint_bg_color = Color32::from_rgb(68, 68, 68);
    visuals.widgets.noninteractive.bg_fill = PANEL;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(212, 212, 212));
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(65, 69, 74));
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(60, 64, 69);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(53, 57, 62);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(70, 82, 94);
    visuals.widgets.active.bg_fill = Color32::from_rgb(62, 94, 119);
    visuals.selection.bg_fill = Color32::from_rgb(57, 88, 113);
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(4.0, 3.0);
        style.spacing.button_padding = egui::vec2(5.0, 2.0);
        style.spacing.interact_size.y = 20.0;
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
