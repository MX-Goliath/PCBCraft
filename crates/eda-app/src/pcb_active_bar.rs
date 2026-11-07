//! PCB Active Bar, independently implemented from observed AD24 interactions.
use super::*;
use eda_core::pcb_selection::RegionSelection;
use egui::{Pos2, Rect, Sense, Stroke, Vec2};
#[path = "pcb_active_bar_catalog.rs"]
mod catalog;
#[path = "pcb_active_bar_commands.rs"]
mod commands;
#[path = "pcb_active_bar_icons.rs"]
mod icons;
#[cfg(test)]
#[path = "pcb_active_bar_tests.rs"]
mod tests;
use catalog::{Entry, GROUPS};

struct Group {
    name: &'static str,
    width: f32,
    items: &'static [Entry],
}
#[derive(Default)]
pub(in crate::app) struct State {
    menu: Option<(usize, Pos2)>,
    last: [usize; 14],
    focus: Option<usize>,
    press: Option<(usize, f64)>,
    held: bool,
    region: Option<SelectionGesture>,
    dialog: Option<TransformDialog>,
    component_pick: bool,
    offset_pick: Option<(TransformDialog, Option<Point>)>,
    rotate_pick: Option<(Vec<Uuid>, u64, f64)>,
    snap_custom: Option<[bool; 8]>,
    align_dialog: Option<AlignDialog>,
    align_pick: Option<AlignmentPick>,
}
#[derive(Clone)]
struct AlignmentPick {
    selection: Vec<Uuid>,
    revision: u64,
    operations: Vec<eda_core::pcb_arrange::Align>,
    draft: std::sync::Arc<pcb::Board>,
}
struct SelectionGesture {
    mode: RegionSelection,
    points: Vec<Point>,
    revision: u64,
}
struct TransformDialog {
    rotate: bool,
    selection: Vec<Uuid>,
    revision: u64,
    x: String,
    y: String,
    angle: String,
    error: Option<String>,
    units: Units,
    focused: bool,
}
struct AlignDialog {
    selection: Vec<Uuid>,
    revision: u64,
    h: u8,
    v: u8,
    error: Option<String>,
}
impl State {
    pub(super) fn instruction(&self) -> Option<&'static str> {
        self.align_pick.as_ref().map(|_| "Select a Primitive")
    }
    pub(in crate::app) fn popup_open(&self) -> bool {
        self.menu.is_some()
    }
    pub(super) fn cancel_interaction(&mut self) {
        self.region = None;
        self.dialog = None;
        self.component_pick = false;
        self.offset_pick = None;
        self.rotate_pick = None;
        self.align_dialog = None;
        self.align_pick = None;
    }
}
pub(super) use commands::input;

fn enabled(id: &str, doc: &OpenDocument) -> bool {
    commands::enabled(id, doc)
}
fn frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(Color32::from_gray(61))
        .corner_radius(5)
        .inner_margin(0)
}
fn popup_position(ctx: &egui::Context, at: Pos2, size: Vec2) -> Pos2 {
    let screen = ctx.screen_rect();
    egui::pos2(
        at.x.clamp(screen.left(), (screen.right() - size.x).max(screen.left())),
        at.y.clamp(screen.top(), (screen.bottom() - size.y).max(screen.top())),
    )
}
impl PcbCraft {
    pub(in crate::app) fn pcb_bar_shortcuts(&mut self, index: usize, ctx: &egui::Context) -> bool {
        let state = &mut self.documents[index].pcb_editor.bar;
        if state.menu.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                state.menu = None;
                state.focus = None;
            }
            return true;
        }
        if state.dialog.is_some() || state.align_dialog.is_some() {
            return true;
        }
        if state.align_pick.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
                && let Err(error) = commands::cancel_alignment(&mut self.documents[index])
            {
                self.message(true, error);
            }
            return true;
        }
        if state.offset_pick.is_some() || state.rotate_pick.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                if let Some((dialog, _)) = state.offset_pick.take() {
                    state.dialog = Some(dialog);
                }
                state.rotate_pick = None;
                self.documents[index].view.pcb.clear_gesture();
            }
            return true;
        }
        if state.region.is_some() {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                state.region = None;
                self.documents[index].view.pcb.clear_gesture();
            }
            return true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::E)) {
            state.menu = Some((1, ctx.screen_rect().center()));
            return true;
        }
        false
    }
    pub(in crate::app) fn pcb_active_bar(&mut self, ctx: &egui::Context, canvas: Rect) {
        let Some(index) = self.active else {
            return;
        };
        if self.documents[index].model.pcb.is_none() {
            return;
        }
        let blocked = self.documents[index].read_only
            || self.documents[index].pcb_editor.rules_open()
            || self.file_state.modal()
            || self.pending.is_some();
        if blocked {
            self.documents[index].pcb_editor.bar.menu = None;
        }
        self.documents[index].pcb_editor.bar.last = self.settings.pcb_active_bar_last;
        let origin = egui::pos2(
            (canvas.center().x - 238.).max(canvas.left()),
            canvas.top() + 4.,
        );
        let mut choose = None;
        let mut opening = None;
        let bar = egui::Area::new(egui::Id::new("active_bar"))
            .fade_in(false)
            .order(egui::Order::Foreground)
            .fixed_pos(origin)
            .show(ctx, |ui| {
                frame().fill(Color32::from_gray(58)).show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                    ui.horizontal(|ui| {
                        ui.add_space(4.);
                        for (g, group) in GROUPS.iter().enumerate() {
                            let entry = group
                                .items
                                .get(self.documents[index].pcb_editor.bar.last[g]);
                            let available = !blocked
                                && (g < 2
                                    || entry
                                        .is_some_and(|e| enabled(e.id, &self.documents[index])));
                            let (r, response) = ui.allocate_exact_size(
                                egui::vec2(100. / 3., 100. / 3.),
                                Sense::click(),
                            );
                            let state = &self.documents[index].pcb_editor.bar;
                            if !blocked
                                && (response.hovered() || state.menu.is_some_and(|(n, _)| n == g))
                            {
                                ui.painter().rect_filled(
                                    r.shrink2(egui::vec2(1., 2.)),
                                    2.,
                                    Color32::from_rgb(65, 99, 125),
                                );
                            }
                            icons::paint(
                                ui.painter(),
                                Rect::from_center_size(
                                    r.center() - egui::vec2(0., 1.),
                                    egui::vec2(22., 22.),
                                ),
                                g,
                                entry.map_or("", |e| e.id),
                                available,
                            );
                            if g >= 2 {
                                let p = r.right_bottom() - egui::vec2(4., 3.);
                                ui.painter().add(egui::Shape::convex_polygon(
                                    vec![p, p - egui::vec2(5., 0.), p - egui::vec2(0., 5.)],
                                    Color32::LIGHT_GRAY,
                                    Stroke::NONE,
                                ));
                            }
                            if g == 4 {
                                ui.painter().line_segment(
                                    [
                                        r.right_top() + egui::vec2(0., 5.),
                                        r.right_bottom() - egui::vec2(0., 5.),
                                    ],
                                    Stroke::new(1., Color32::from_gray(38)),
                                );
                            }
                            let state = &mut self.documents[index].pcb_editor.bar;
                            if !blocked
                                && ctx.input(|i| {
                                    i.pointer.primary_down()
                                        && i.pointer.press_origin().is_some_and(|p| r.contains(p))
                                })
                            {
                                let (pressed, time) =
                                    *state.press.get_or_insert((g, ctx.input(|i| i.time)));
                                if pressed == g
                                    && g >= 2
                                    && !state.held
                                    && ctx.input(|i| i.time) - time >= 1.
                                {
                                    opening = Some((g, r.left_bottom() + egui::vec2(0., 2.)));
                                    state.held = true;
                                } else {
                                    ctx.request_repaint_after(std::time::Duration::from_millis(50));
                                }
                            }
                            if !blocked && response.secondary_clicked() {
                                opening = Some((
                                    g,
                                    if g < 2 {
                                        origin + egui::vec2(4., 35.333)
                                    } else {
                                        r.left_bottom() + egui::vec2(0., 2.)
                                    },
                                ));
                            }
                            if !blocked && response.clicked() && !state.held {
                                if g < 2 {
                                    opening = Some((g, origin + egui::vec2(4., 35.333)));
                                } else if available {
                                    choose = Some((g, state.last[g]));
                                }
                            }
                            response.on_hover_text(entry.map_or(group.name, |e| e.label));
                        }
                        ui.add_space(16. / 3.);
                    });
                });
            });
        if !ctx.input(|i| i.pointer.primary_down()) {
            let state = &mut self.documents[index].pcb_editor.bar;
            state.press = None;
            state.held = false;
        }
        if let Some(open) = opening {
            let state = &mut self.documents[index].pcb_editor.bar;
            state.menu = if state.menu.is_some_and(|(g, _)| g == open.0) {
                None
            } else {
                Some(open)
            };
            state.focus = None;
        }
        if let Some((g, at)) = self.documents[index].pcb_editor.bar.menu {
            let group = &GROUPS[g];
            let height = if g == 0 {
                134.
            } else if g == 1 {
                177.
            } else {
                group
                    .items
                    .iter()
                    .map(|e| if e.id.is_empty() { 4. / 3. } else { 88. / 3. })
                    .sum()
            };
            let at = popup_position(ctx, at, egui::vec2(group.width, height));
            let area = egui::Area::new(egui::Id::new("pcb_active_bar_menu"))
                .fade_in(false)
                .order(egui::Order::Foreground)
                .fixed_pos(at)
                .show(ctx, |ui| {
                    // A ScrollArea otherwise inherits the previous group's cached
                    // height and can permanently hide the end of a longer menu.
                    ui.set_width(group.width);
                    ui.set_height(height.min((ctx.screen_rect().height() - 12.).max(40.)));
                    frame().corner_radius(if g < 2 { 0 } else { 5 }).show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::ZERO;
                        if g == 0 {
                            filter(ui, &mut self.documents[index].view.pcb);
                        } else if g == 1 {
                            let doc = &mut self.documents[index];
                            snapping(ui, &mut doc.view, &mut doc.pcb_editor.bar.snap_custom);
                        } else {
                            let eligible: Vec<_> = group.items.iter().enumerate()
                                .filter_map(|(n, e)| enabled(e.id, &self.documents[index]).then_some(n))
                                .collect();
                            let direction = ctx.input_mut(|i| {
                                if i.consume_key(egui::Modifiers::NONE, Key::ArrowDown) { 1 }
                                else if i.consume_key(egui::Modifiers::NONE, Key::ArrowUp) { -1 }
                                else { 0 }
                            });
                            if direction != 0 && !eligible.is_empty() {
                                let state = &mut self.documents[index].pcb_editor.bar;
                                let old = state.focus.and_then(|n| eligible.iter().position(|v| *v == n));
                                let next = old.map_or(
                                    if direction > 0 { 0 } else { eligible.len() - 1 },
                                    |n| (n as isize + direction).rem_euclid(eligible.len() as isize) as usize,
                                );
                                state.focus = Some(eligible[next]);
                            }
                            egui::ScrollArea::vertical()
                                .id_salt(("pcb_bar_scroll", g))
                                .max_height((ctx.screen_rect().height() - 12.).max(40.))
                                .show(ui, |ui| {
                                    for (n, e) in group.items.iter().enumerate() {
                                        if e.id.is_empty() {
                                            let (r, _) = ui.allocate_exact_size(
                                                egui::vec2(group.width, 4. / 3.), Sense::hover());
                                            ui.painter().line_segment([
                                                r.left_center() + egui::vec2(36., 0.),
                                                r.right_center() - egui::vec2(4., 0.),
                                            ], Stroke::new(1., Color32::from_gray(42)));
                                            continue;
                                        }
                                        let available = enabled(e.id, &self.documents[index]);
                                        let (r, response) = ui.allocate_exact_size(
                                            egui::vec2(group.width, 88. / 3.),
                                            if available { Sense::click() } else { Sense::hover() },
                                        );
                                        let focus = self.documents[index].pcb_editor.bar.focus == Some(n);
                                        if available && (response.hovered() || focus) {
                                            ui.painter().rect_filled(r.shrink2(egui::vec2(1., 0.)),
                                                0., Color32::from_rgb(65, 99, 125));
                                        }
                                        if icons::menu_icon(e.id, g) {
                                            icons::paint(ui.painter(), Rect::from_center_size(
                                                r.left_center() + egui::vec2(17., 0.), egui::vec2(22., 22.)),
                                                g, e.id, available);
                                        }
                                        ui.painter().text(r.left_center() + egui::vec2(36., 0.),
                                            egui::Align2::LEFT_CENTER, e.label, egui::FontId::proportional(12.),
                                            Color32::from_gray(if available { 224 } else { 119 }));
                                        if response.clicked() || available && focus
                                            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)) {
                                            choose = Some((g, n));
                                        }
                                        if !available {
                                            response.on_hover_text("This command is not implemented or is unavailable for the current selection");
                                        }
                                    }
                                });
                        }
                    });
                });
            if ctx.input(|i| {
                i.pointer.any_pressed()
                    && !area
                        .response
                        .rect
                        .contains(i.pointer.latest_pos().unwrap_or(at))
                    && !bar
                        .response
                        .rect
                        .contains(i.pointer.latest_pos().unwrap_or(at))
            }) {
                self.documents[index].pcb_editor.bar.menu = None;
            }
        }
        if let Some((g, n)) = choose {
            let id = GROUPS[g].items[n].id;
            let state = &mut self.documents[index].pcb_editor.bar;
            state.last[g] = n;
            state.menu = None;
            self.settings.pcb_active_bar_last = state.last;
            if let Err(error) = self.pcb_bar_command(index, id, ctx) {
                self.message(true, error);
            }
        }
        self.pcb_bar_dialog(index, ctx);
    }
}

fn chip(ui: &mut egui::Ui, label: &str, value: &mut bool, width: f32) {
    if ui
        .add_sized([width, 24.], egui::Button::new(label).selected(*value))
        .clicked()
    {
        *value = !*value;
    }
}
fn disabled_chip(ui: &mut egui::Ui, label: &str, width: f32) {
    ui.add_enabled_ui(false, |ui| {
        ui.add_sized([width, 24.], egui::Button::new(label));
    });
}
fn filter(ui: &mut egui::Ui, view: &mut eda_render::pcb::BoardView) {
    egui::Frame::NONE.inner_margin(8).show(ui, |ui| {
        ui.set_min_width(347.);
        ui.spacing_mut().item_spacing = egui::vec2(6., 22. / 3.);
        let all = view.filters.iter().all(|v| *v)
            && view.component_selection
            && view.select_bodies
            && view.select_keepouts;
        if ui
            .add_sized([76., 24.], egui::Button::new("All - On").selected(all))
            .clicked()
        {
            view.filters.fill(!all);
            view.component_selection = !all;
            view.select_bodies = !all;
            view.select_keepouts = !all;
        }
        ui.horizontal(|ui| {
            chip(ui, "Components", &mut view.component_selection, 86.);
            chip(ui, "3D Bodies", &mut view.select_bodies, 72.);
            chip(ui, "Keepouts", &mut view.select_keepouts, 68.);
            chip(ui, "Tracks", &mut view.filters[0], 51.);
            chip(ui, "Arcs", &mut view.filters[1], 42.);
        });
        ui.horizontal(|ui| {
            chip(ui, "Pads", &mut view.filters[2], 43.);
            chip(ui, "Vias", &mut view.filters[3], 40.);
            chip(ui, "Regions", &mut view.filters[6], 61.);
            chip(ui, "Polygons", &mut view.filters[7], 66.);
            chip(ui, "Fills", &mut view.filters[5], 38.);
            chip(ui, "Texts", &mut view.filters[4], 46.);
        });
        ui.horizontal(|ui| {
            disabled_chip(ui, "Rooms", 55.);
            disabled_chip(ui, "Other", 50.);
        });
    });
}
fn snapping(ui: &mut egui::Ui, view: &mut View, custom: &mut Option<[bool; 8]>) {
    egui::Frame::NONE.inner_margin(8).show(ui, |ui| {
        ui.set_min_width(409.);
        ui.set_min_height(161.);
        ui.spacing_mut().item_spacing = egui::vec2(6., 6.);
        ui.horizontal(|ui| {
            chip(ui, "Grids", &mut view.grid, 46.);
            disabled_chip(ui, "Guides", 55.);
            disabled_chip(ui, "Axes", 45.);
        });
        ui.add_space(2.);
        ui.separator();
        ui.add_space(1.);
        let snap = &mut view.pcb.snap;
        let values = [
            snap.track_vertices,
            snap.arc_centres,
            snap.pad_centres,
            snap.pad_edges,
            snap.via_centres,
            snap.region_vertices,
            snap.board_vertices,
            snap.component_origins,
        ];
        let all = values.iter().all(|v| *v);
        let off = values.iter().all(|v| !*v);
        let label = if all {
            "All - On"
        } else if off {
            "All - Off"
        } else {
            "Custom"
        };
        if ui.add_sized([74., 24.], egui::Button::new(label)).clicked() {
            let next = if all {
                [false; 8]
            } else if off {
                custom.unwrap_or([true, true, true, true, true, true, false, true])
            } else {
                *custom = Some(values);
                [true; 8]
            };
            [
                snap.track_vertices,
                snap.arc_centres,
                snap.pad_centres,
                snap.pad_edges,
                snap.via_centres,
                snap.region_vertices,
                snap.board_vertices,
                snap.component_origins,
            ] = next;
        }
        ui.horizontal(|ui| {
            chip(ui, "Track Vertices", &mut snap.track_vertices, 89.);
            disabled_chip(ui, "Track Lines", 75.);
            chip(ui, "Arc Centers", &mut snap.arc_centres, 79.);
            disabled_chip(ui, "Intersections", 85.);
        });
        ui.horizontal(|ui| {
            chip(ui, "Pad Centers", &mut snap.pad_centres, 80.);
            disabled_chip(ui, "Pad Vertices", 82.);
            chip(ui, "Pad Edges", &mut snap.pad_edges, 72.);
            chip(ui, "Via Centers", &mut snap.via_centres, 78.);
            disabled_chip(ui, "Texts", 45.);
        });
        ui.horizontal(|ui| {
            chip(ui, "Regions", &mut snap.region_vertices, 60.);
            chip(ui, "Board Shape", &mut snap.board_vertices, 85.);
            chip(ui, "Footprint Origins", &mut snap.component_origins, 106.);
            disabled_chip(ui, "3D Body Snap Points", 126.);
        });
    });
}
