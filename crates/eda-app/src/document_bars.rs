//! Editor chrome observed in AD 24.4.1. This module changes view state only.
use super::*;
use egui::{Align2, FontId, Rect, Sense, Stroke, pos2, vec2};

const BACKGROUND: Color32 = Color32::from_gray(51);

fn tab(
    ui: &mut egui::Ui,
    title: &str,
    selected: bool,
    height: f32,
    icon: Option<Color32>,
) -> egui::Response {
    let font = if selected && height < 26.0 {
        FontId::new(11.0, egui::FontFamily::Name("ui-bold".into()))
    } else {
        FontId::proportional(11.0)
    };
    let galley = ui
        .painter()
        .layout_no_wrap(title.into(), font, Color32::from_gray(225));
    let width = galley.size().x + if icon.is_some() { 26.0 } else { 12.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    let fill = if selected {
        Color32::from_gray(81)
    } else if response.hovered() {
        Color32::from_gray(68)
    } else {
        Color32::from_gray(59)
    };
    ui.painter()
        .rect_filled(rect.shrink2(vec2(0.5, 1.0)), 2.0, fill);
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, Color32::from_gray(38)),
    );
    let mut x = rect.left() + 6.0;
    if let Some(color) = icon {
        let swatch = Rect::from_center_size(pos2(x + 4.0, rect.center().y), vec2(9.0, 9.0));
        ui.painter().rect_filled(swatch, 0.0, color);
        ui.painter().rect_stroke(
            swatch,
            0.0,
            Stroke::new(0.5, Color32::from_gray(166)),
            egui::StrokeKind::Inside,
        );
        x += 15.0;
    }
    ui.painter().galley(
        pos2(x, rect.center().y - galley.size().y / 2.0),
        galley,
        Color32::from_gray(225),
    );
    response
}

pub(super) fn document_icon(ui: &egui::Ui, rect: Rect, kind: DocumentKind) {
    let p = ui.painter();
    let r = Rect::from_center_size(pos2(rect.left() + 11.0, rect.center().y), vec2(14.0, 11.0));
    let board = kind == DocumentKind::Board;
    p.rect_filled(
        r,
        0.0,
        if board {
            Color32::from_rgb(19, 125, 94)
        } else {
            Color32::from_rgb(205, 181, 96)
        },
    );
    let ink = Color32::from_rgb(231, 237, 204);
    if board {
        p.rect_stroke(
            r.shrink(3.0),
            0.0,
            Stroke::new(1.0, ink),
            egui::StrokeKind::Inside,
        );
        for y in [2.0, 5.0, 8.0] {
            for x in [1.0, 11.0] {
                p.line_segment(
                    [r.min + vec2(x, y), r.min + vec2(x + 2.0, y)],
                    Stroke::new(1.0, ink),
                );
            }
        }
    } else {
        for y in [3.0, 5.0, 7.0] {
            p.line_segment(
                [r.min + vec2(3.0, y), r.min + vec2(11.0, y)],
                Stroke::new(1.0, ink),
            );
        }
    }
}

impl PcbCraft {
    pub(super) fn document_tabs(&mut self, ui: &mut egui::Ui) {
        let mut close = None;
        let active_id = self.active.map(|i| self.documents[i].session_id);
        let selection_key = ui.id().with("document_tab_selection");
        let reveal = ui.ctx().data_mut(|d| {
            let previous = d.get_temp::<Option<Uuid>>(selection_key);
            d.insert_temp(selection_key, active_id);
            previous != Some(active_id)
        });
        egui::Frame::NONE.fill(BACKGROUND).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(26.0);
            egui::ScrollArea::horizontal()
                .id_salt("document_tabs")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    ui.horizontal(|ui| {
                        for (index, doc) in self.documents.iter().enumerate() {
                            let sheet_number = (doc.model.kind == DocumentKind::Schematic)
                                .then(|| {
                                    let path = doc.source_path.as_ref().or(doc.path.as_ref());
                                    self.projects.iter().find_map(|project| {
                                        path.and_then(|path| project.sheet_number(path))
                                    })
                                })
                                .flatten();
                            let prefix =
                                sheet_number.map_or_else(String::new, |n| format!("[{n}] "));
                            let title = format!(
                                "{prefix}{}{}",
                                doc.model.name,
                                if doc.dirty() { " *" } else { "" }
                            );
                            let response = tab(
                                ui,
                                &title,
                                self.active == Some(index),
                                26.0,
                                Some(Color32::TRANSPARENT),
                            );
                            document_icon(ui, response.rect, doc.model.kind);
                            if reveal && self.active == Some(index) {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            if response.clicked() {
                                self.active = Some(index);
                                self.pad_edit = None;
                            }
                            if response.clicked_by(egui::PointerButton::Middle) {
                                close = Some(doc.session_id);
                            }
                            response.context_menu(|ui| {
                                if ui.button(format!("Close {title}")).clicked() {
                                    close = Some(doc.session_id);
                                    ui.close();
                                }
                                for name in [
                                    if doc.model.kind == DocumentKind::Board {
                                        "Close PCB Documents"
                                    } else {
                                        "Close Schematic Documents"
                                    },
                                    "Close All Other Documents",
                                    "Close All Documents",
                                ] {
                                    ui.add_enabled(false, egui::Button::new(name));
                                }
                                ui.separator();
                                for name in [
                                    "Split Vertical",
                                    "Split Horizontal",
                                    "Tile All",
                                    "Merge All",
                                    "Open In New Window",
                                ] {
                                    ui.add_enabled(false, egui::Button::new(name));
                                }
                            });
                        }
                        if let Some(manager) = &mut self.layer_stack {
                            let name = self
                                .documents
                                .iter()
                                .find(|d| d.session_id == manager.session)
                                .map(|d| d.model.name.as_str())
                                .unwrap_or("PCB");
                            let title = format!(
                                "{name} [Stackup]{}",
                                if manager.dirty() { " *" } else { "" }
                            );
                            if tab(
                                ui,
                                &title,
                                false,
                                26.0,
                                Some(Color32::from_rgb(195, 126, 75)),
                            )
                            .clicked()
                            {
                                manager.visible = true;
                            }
                        }
                        let home = tab(
                            ui,
                            "Home Page",
                            self.active.is_none(),
                            26.0,
                            Some(Color32::TRANSPARENT),
                        );
                        theme::draw_icon(
                            ui.painter(),
                            Rect::from_center_size(
                                pos2(home.rect.left() + 11.0, home.rect.center().y),
                                vec2(16.0, 16.0),
                            ),
                            theme::Icon::Home,
                            true,
                        );
                        if home.clicked() {
                            self.command = Some(Command::Home);
                        }
                    });
                });
        });
        if let Some(id) = close {
            self.request_close(id);
        }
    }

    pub(super) fn schematic_bottom_bar(&mut self, ui: &mut egui::Ui, index: usize) {
        egui::TopBottomPanel::bottom("schematic_editor")
            .exact_height(22.0)
            .frame(egui::Frame::NONE.fill(BACKGROUND))
            .show_inside(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.horizontal(|ui| {
                    tab(ui, "Editor", true, 20.0, None);
                    let name = self.documents[index].model.name.trim_end_matches(".SchDoc");
                    ui.add_enabled_ui(false, |ui| {
                        tab(ui, name, false, 20.0, None)
                            .on_hover_text("Compiled sheet view is not implemented");
                    });
                });
            });
    }

    pub(super) fn pcb_bottom_bar(&mut self, ui: &mut egui::Ui, index: usize) {
        let doc = &mut self.documents[index];
        let Some(board) = &doc.model.pcb else {
            return;
        };
        doc.view.pcb.initialize(board);
        let view = &mut doc.view.pcb;
        let tabs = view.layer_tabs();
        let mut chosen = None;
        egui::TopBottomPanel::bottom("pcb_layers_v1")
            .exact_height(22.0)
            .frame(egui::Frame::NONE.fill(BACKGROUND))
            .show_inside(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                ui.horizontal(|ui| {
                    let (r, response) = ui.allocate_exact_size(vec2(50.0, 20.0), Sense::click());
                    ui.painter().rect_filled(
                        Rect::from_min_size(r.min + vec2(3.0, 5.0), vec2(25.0, 12.0)),
                        0.0,
                        view.color(view.active_layer),
                    );
                    ui.painter().text(
                        r.min + vec2(32.0, 11.0),
                        Align2::LEFT_CENTER,
                        "LS",
                        FontId::proportional(11.0),
                        Color32::from_gray(225),
                    );
                    egui::Popup::menu(&response)
                        .width(160.0)
                        .frame(
                            egui::Frame::popup(ui.style())
                                .corner_radius(0)
                                .inner_margin(egui::Margin {
                                    left: 28,
                                    right: 8,
                                    top: 3,
                                    bottom: 3,
                                }),
                        )
                        .show(|ui| {
                            for (set, name) in [
                                "All Layers",
                                "Signal Layers",
                                "Plane Layers",
                                "NonSignal Layers",
                                "Mechanical Layers",
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                if ui.button(name).clicked() {
                                    view.apply_layer_set(set);
                                    ui.close();
                                }
                            }
                            ui.separator();
                            ui.add_enabled(false, egui::Button::new("Manage Layer Sets"));
                        });
                    let scroll_id = ui.id().with(("layer_scroll", doc.session_id));
                    let mut offset = ui
                        .ctx()
                        .data_mut(|d| d.get_temp::<f32>(scroll_id).unwrap_or(0.0));
                    for delta in [-120.0, 120.0] {
                        let (rect, response) =
                            ui.allocate_exact_size(vec2(18.0, 20.0), Sense::click());
                        ui.painter().rect_filled(
                            rect.shrink2(vec2(1.0, 3.0)),
                            1.0,
                            Color32::from_gray(if response.hovered() { 81 } else { 63 }),
                        );
                        let c = rect.center();
                        let direction = if delta < 0.0 { -1.0 } else { 1.0 };
                        ui.painter().add(egui::Shape::convex_polygon(
                            vec![
                                c + vec2(2.0 * direction, 0.0),
                                c + vec2(-2.0 * direction, -3.0),
                                c + vec2(-2.0 * direction, 3.0),
                            ],
                            Color32::from_gray(190),
                            Stroke::NONE,
                        ));
                        if response.clicked() {
                            offset = (offset + delta).max(0.0);
                        }
                    }
                    let output = egui::ScrollArea::horizontal()
                        .id_salt(scroll_id)
                        .horizontal_scroll_offset(offset)
                        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (id, name) in &tabs {
                                    let response = tab(
                                        ui,
                                        name,
                                        view.active_layer == *id,
                                        20.0,
                                        Some(view.color(*id)),
                                    );
                                    if response.clicked() {
                                        chosen = Some(*id);
                                    }
                                    if response.double_clicked() {
                                        view.show_layer_dialog = true;
                                    }
                                }
                            });
                        });
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(scroll_id, output.state.offset.x));
                });
            });
        if let Some(id) = chosen.filter(|id| *id != view.active_layer) {
            self.command = Some(Command::Pcb(pcb_editor::Action::RouteLayer(id)));
        }
    }
}
