//! Native-style immediate Sheet Entry/Port synchronization and placement session.
use super::*;
use eda_core::{
    schematic::SchPrimitiveKind as K,
    schematic_edit as edit,
    schematic_sync::{self as sync, Destination, Signal, Transfer},
};
use eda_geom::Point;
use std::collections::BTreeSet;
#[cfg(test)]
#[path = "schematic_sync_tests.rs"]
mod tests;

#[derive(Default)]
pub(super) struct State {
    dialog: Option<Dialog>,
    popup: Option<(usize, Uuid, egui::Pos2)>,
}
impl State {
    pub fn busy(&self) -> bool {
        self.dialog.is_some()
    }
}
struct Tab {
    parent: Uuid,
    child: Uuid,
    symbol: Uuid,
    caption: String,
    file: String,
}
struct Dialog {
    title: String,
    tabs: Vec<Tab>,
    tab: usize,
    only_unmatched: bool,
    entries: Rows,
    ports: Rows,
    links: Rows,
    excluded: Vec<(Uuid, Uuid)>,
    pending: Option<PendingPlacement>,
    error: Option<String>,
}
struct PendingPlacement {
    target: Uuid,
    guards: Vec<(Uuid, u64)>,
    transfer: Transfer,
    relink: Vec<(Uuid, Uuid)>,
}
#[derive(Default)]
struct Rows {
    selected: BTreeSet<Uuid>,
    anchor: Option<usize>,
    dragging: bool,
    press_position: Option<egui::Pos2>,
    columns: Option<[f32; 3]>,
    column_drag: Option<(usize, f32)>,
}
impl Rows {
    fn clear(&mut self) {
        let columns = self.columns;
        *self = Self {
            columns,
            ..Self::default()
        };
    }
}
#[derive(Clone, Copy)]
enum Action {
    CopyEntries,
    CopyPorts,
    AddPorts,
    AddEntries,
    DeleteEntries,
    DeletePorts,
    Unlink,
}

fn signal_icon(painter: &egui::Painter, at: egui::Pos2, linked: bool) {
    let rect = |x, y, w, h| egui::Rect::from_min_size(at + egui::vec2(x, y), egui::vec2(w, h));
    if linked {
        painter.rect_filled(
            rect(0.0, 0.0, 10.0, 13.0),
            0.0,
            Color32::from_rgb(48, 139, 161),
        );
        painter.rect_filled(
            rect(2.0, 2.0, 6.0, 9.0),
            0.0,
            Color32::from_rgb(223, 235, 224),
        );
        painter.line_segment(
            [at + egui::vec2(3.0, 4.0), at + egui::vec2(7.0, 4.0)],
            egui::Stroke::new(1.0, Color32::from_rgb(71, 144, 114)),
        );
    }
    let points = [
        (4.0, 5.0),
        (12.0, 5.0),
        (15.0, 8.5),
        (12.0, 12.0),
        (4.0, 12.0),
    ]
    .map(|(x, y)| at + egui::vec2(x, y));
    painter.add(egui::Shape::convex_polygon(
        points.to_vec(),
        Color32::from_rgb(255, 239, 164),
        egui::Stroke::new(0.8, Color32::from_rgb(164, 148, 92)),
    ));
}

fn link_button(ui: &mut egui::Ui, enabled: bool, direction: Option<bool>) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("").min_size(egui::vec2(56.0, 24.0)),
    );
    let painter = ui.painter();
    let color = if enabled {
        Color32::from_gray(214)
    } else {
        Color32::from_gray(118)
    };
    let centre = response.rect.center();
    let chevron = |x: f32, reverse: bool| {
        let d = if reverse { -1.0 } else { 1.0 };
        painter.line_segment(
            [
                centre + egui::vec2(x - 2.0 * d, -4.0),
                centre + egui::vec2(x + 2.0 * d, 0.0),
            ],
            egui::Stroke::new(1.4, color),
        );
        painter.line_segment(
            [
                centre + egui::vec2(x + 2.0 * d, 0.0),
                centre + egui::vec2(x - 2.0 * d, 4.0),
            ],
            egui::Stroke::new(1.4, color),
        );
    };
    if let Some(to_ports) = direction {
        chevron(0.0, false);
        for (x, port) in [(-15.0, !to_ports), (15.0, to_ports)] {
            let r =
                egui::Rect::from_center_size(centre + egui::vec2(x, 0.0), egui::vec2(12.0, 9.0));
            if port {
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "D1",
                    egui::FontId::proportional(8.0),
                    color,
                );
                painter.rect_stroke(
                    r,
                    0.0,
                    egui::Stroke::new(0.7, color),
                    egui::StrokeKind::Inside,
                );
            } else {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        r.left_top(),
                        r.right_top() - egui::vec2(3.0, 0.0),
                        r.right_center(),
                        r.right_bottom() - egui::vec2(3.0, 0.0),
                        r.left_bottom(),
                    ],
                    color,
                    egui::Stroke::NONE,
                ));
            }
        }
    } else {
        chevron(0.0, true);
    }
    response
}

#[allow(clippy::too_many_arguments)]
fn table(
    ui: &mut egui::Ui,
    id: &str,
    title: &str,
    heading: &str,
    signals: &[Signal],
    selection: &mut Rows,
    width: f32,
    height: f32,
) {
    if !ui.input(|i| {
        i.pointer.primary_down() || i.pointer.button_released(egui::PointerButton::Primary)
    }) {
        selection.dragging = false;
        selection.press_position = None;
        selection.column_drag = None;
    }
    let press_position = ui.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                ..
            } => Some(*pos),
            _ => None,
        })
    });
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_width(width);
            ui.spacing_mut().item_spacing.y = 0.0;
            if !title.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.label(title);
                });
            }
            let (header, _) = ui.allocate_exact_size(egui::vec2(width, 17.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(header, 0.0, Color32::from_gray(66));
            let mut columns =
                selection
                    .columns
                    .unwrap_or([width * 0.50, width * 0.18, width * 0.32]);
            for n in 0..2 {
                let x = header.left() + columns[..=n].iter().sum::<f32>();
                let divider = egui::Rect::from_center_size(
                    egui::pos2(x, header.center().y),
                    egui::vec2(6.0, 17.0),
                );
                ui.interact(
                    divider,
                    ui.id().with((id, "column", n)),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
                if let Some(pos) = press_position
                    && divider.contains(pos)
                    && ui.clip_rect().contains(pos)
                    && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
                {
                    selection.column_drag = Some((n, pos.x));
                }
                if let Some((dragged, previous_x)) = selection.column_drag
                    && dragged == n
                {
                    let pointer =
                        ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down()));
                    if let Some(pos) = pointer.0 {
                        let delta =
                            (pos.x - previous_x).clamp(28.0 - columns[n], columns[n + 1] - 28.0);
                        columns[n] += delta;
                        columns[n + 1] -= delta;
                        selection.columns = Some(columns);
                        selection.column_drag = pointer.1.then_some((n, pos.x));
                    } else {
                        selection.column_drag = None;
                    }
                }
                ui.painter().line_segment(
                    [
                        egui::pos2(x, header.top() + 2.0),
                        egui::pos2(x, header.bottom() - 2.0),
                    ],
                    egui::Stroke::new(1.0, Color32::from_gray(48)),
                );
            }
            let paint = |ui: &mut egui::Ui,
                         rect: egui::Rect,
                         values: [&str; 3],
                         selected: bool,
                         row: bool| {
                if selected {
                    ui.painter()
                        .rect_filled(rect, 0.0, Color32::from_rgb(64, 98, 124));
                }
                let mut x = rect.left();
                for (n, value) in values.into_iter().enumerate() {
                    let clip = egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(columns[n], rect.height()),
                    );
                    if n == 0 && row {
                        signal_icon(
                            ui.painter(),
                            egui::pos2(x + 1.0, rect.center().y - 7.0),
                            heading == "Links",
                        );
                    }
                    ui.painter()
                        .with_clip_rect(clip.intersect(ui.clip_rect()))
                        .text(
                            clip.left_center()
                                + egui::vec2(if n == 0 && row { 18.0 } else { 3.0 }, 0.0),
                            egui::Align2::LEFT_CENTER,
                            value,
                            egui::FontId::proportional(11.0),
                            Color32::from_gray(218),
                        );
                    x += columns[n];
                }
            };
            paint(ui, header, [heading, "IO", "Harness Type"], false, false);
            let body_height = height - 17.0 - if title.is_empty() { 0.0 } else { 18.0 };
            let body = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(width, body_height));
            ui.painter().rect_filled(body, 0.0, Color32::from_gray(58));
            let area = egui::ScrollArea::vertical()
                .id_salt(id)
                .min_scrolled_height(body_height)
                .max_height(body_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for (n, signal) in signals.iter().enumerate() {
                        let (r, response) = ui.allocate_exact_size(
                            egui::vec2(width - 8.0, 17.0),
                            egui::Sense::click_and_drag(),
                        );
                        let io = ["Unspecified", "Output", "Input", "Bidirectional"]
                            .get(signal.io as usize)
                            .copied()
                            .unwrap_or("Custom");
                        paint(
                            ui,
                            r,
                            [&signal.name, io, &signal.harness],
                            selection.selected.contains(&signal.id),
                            true,
                        );
                        if let Some(pos) = press_position
                            .filter(|pos| r.contains(*pos) && ui.clip_rect().contains(*pos))
                            && ui.ctx().layer_id_at(pos) == Some(ui.layer_id())
                        {
                            selection.dragging = true;
                            selection.press_position = Some(pos);
                            if !ui.input(|i| i.modifiers.shift) {
                                selection.anchor = Some(n);
                            }
                        }
                        let range_drag = selection.dragging
                            && ui.input(|i| {
                                selection
                                    .press_position
                                    .zip(i.pointer.interact_pos())
                                    .is_some_and(|(a, b)| a.distance(b) > 6.0)
                            });
                        if response.clicked() && !range_drag {
                            let modifiers = ui.input(|i| i.modifiers);
                            if modifiers.shift {
                                let a = selection.anchor.unwrap_or(n).min(signals.len() - 1);
                                if !modifiers.ctrl {
                                    selection.selected.clear();
                                }
                                for s in &signals[a.min(n)..=a.max(n)] {
                                    selection.selected.insert(s.id);
                                }
                            } else {
                                if !modifiers.ctrl {
                                    selection.selected.clear();
                                }
                                if !selection.selected.insert(signal.id) {
                                    selection.selected.remove(&signal.id);
                                }
                                selection.anchor = Some(n);
                            }
                        }
                        if range_drag
                            && ui.input(|i| {
                                i.pointer
                                    .interact_pos()
                                    .is_some_and(|p| r.contains(p) && ui.clip_rect().contains(p))
                            })
                        {
                            let a = selection.anchor.unwrap_or(n).min(signals.len() - 1);
                            if !ui.input(|i| i.modifiers.ctrl) {
                                selection.selected.clear();
                            }
                            for s in &signals[a.min(n)..=a.max(n)] {
                                selection.selected.insert(s.id);
                            }
                        }
                        response.context_menu(|ui| {
                            if ui.button("Select All").clicked() {
                                selection.selected = signals.iter().map(|s| s.id).collect();
                                ui.close();
                            }
                            if ui.button("Clear Selection").clicked() {
                                selection.clear();
                                ui.close();
                            }
                        });
                        response.on_hover_text(format!(
                            "{}\nI/O Type: {io}\nHarness Type: {}",
                            signal.name, signal.harness
                        ));
                    }
                });
            ui.painter().rect_stroke(
                body,
                0.0,
                egui::Stroke::new(1.0, Color32::from_gray(47)),
                egui::StrokeKind::Inside,
            );
            if area
                .inner_rect
                .contains(ui.input(|i| i.pointer.hover_pos()).unwrap_or_default())
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::A))
            {
                selection.selected = signals.iter().map(|s| s.id).collect();
            }
        },
    );
}

impl PcbCraft {
    pub(super) fn start_sheet_sync(&mut self, selected: bool) {
        if let Err(e) = self.prepare_sheet_sync(selected) {
            self.message(true, e);
        }
    }
    fn prepare_sheet_sync(&mut self, selected: bool) -> Result<(), String> {
        let index = self.active.ok_or("Open a schematic project first")?;
        let source = self.documents[index]
            .source_path
            .as_ref()
            .or(self.documents[index].path.as_ref())
            .ok_or("Open the schematic project first")?;
        let project = self
            .projects
            .iter()
            .find(|p| p.documents.iter().any(|e| &e.path == source))
            .cloned()
            .ok_or("Open the schematic project first")?;
        let picked = if selected {
            let p = self.documents[index]
                .model
                .schematic
                .as_ref()
                .and_then(|s| s.primitive(self.documents[index].view.selected?))
                .ok_or("Select a Sheet Symbol first")?;
            Some(p.presentation.parent.unwrap_or(p.id))
        } else {
            None
        };
        let active_session = self.documents[index].session_id;
        for entry in project
            .documents
            .iter()
            .filter(|e| e.exists && e.kind == "schdoc")
        {
            if !self
                .documents
                .iter()
                .any(|d| d.source_path.as_ref().or(d.path.as_ref()) == Some(&entry.path))
            {
                self.open_path(entry.path.clone());
            }
        }
        let mut tabs = vec![];
        for doc in &self.documents {
            let Some(path) = doc.source_path.as_ref().or(doc.path.as_ref()) else {
                continue;
            };
            if !project.documents.iter().any(|e| &e.path == path) {
                continue;
            }
            let Some(scene) = &doc.model.schematic else {
                continue;
            };
            for p in &scene.primitives {
                let K::SheetSymbol {
                    designator,
                    filename,
                    ..
                } = &p.kind
                else {
                    continue;
                };
                if picked.is_some_and(|id| id != p.id || doc.session_id != active_session) {
                    continue;
                }
                let entry=project.resolve_sheet(path,filename).ok_or_else(||format!("Sheet Symbol {designator}: child file {filename} is missing or outside the project"))?;
                let child = self
                    .documents
                    .iter()
                    .find(|d| d.source_path.as_ref().or(d.path.as_ref()) == Some(&entry.path))
                    .ok_or_else(|| format!("Could not load child sheet {filename}"))?;
                tabs.push(Tab {
                    parent: doc.session_id,
                    child: child.session_id,
                    symbol: p.id,
                    caption: designator.clone(),
                    file: child.model.name.clone(),
                });
            }
        }
        if tabs.is_empty() {
            return Err("No Sheet Symbols found in this project".into());
        }
        self.active = self
            .documents
            .iter()
            .position(|d| d.session_id == active_session);
        self.sheet_sync.dialog = Some(Dialog {
            title: format!("Synchronize Ports To Sheet Entries In {}", project.name),
            tabs,
            tab: 0,
            only_unmatched: true,
            entries: Rows::default(),
            ports: Rows::default(),
            links: Rows::default(),
            excluded: vec![],
            pending: None,
            error: None,
        });
        Ok(())
    }
    fn sync_indices(&self, tab: &Tab) -> Result<(usize, usize), String> {
        Ok((
            self.documents
                .iter()
                .position(|d| d.session_id == tab.parent)
                .ok_or("Parent document was closed")?,
            self.documents
                .iter()
                .position(|d| d.session_id == tab.child)
                .ok_or("Child document was closed")?,
        ))
    }
    pub(super) fn sheet_sync_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.sheet_sync.dialog.take() else {
            return;
        };
        if dialog.pending.is_some() {
            self.sheet_sync.dialog = Some(dialog);
            return;
        }
        let comparisons: Vec<_> = dialog
            .tabs
            .iter()
            .map(|t| {
                self.sync_indices(t).and_then(|(p, c)| {
                    sync::compare(
                        &self.documents[p].model,
                        &self.documents[c].model,
                        t.symbol,
                        &dialog.excluded,
                    )
                })
            })
            .collect();
        let visible: Vec<_> = comparisons
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                !dialog.only_unmatched
                    || !c
                        .as_ref()
                        .is_ok_and(|c| c.entries.is_empty() && c.ports.is_empty())
            })
            .map(|(n, _)| n)
            .collect();
        if !visible.contains(&dialog.tab)
            && let Some(&n) = visible.first()
        {
            dialog.tab = n;
            dialog.entries.clear();
            dialog.ports.clear();
            dialog.links.clear();
        }
        let mut close = false;
        let mut action = None;
        egui::Modal::new(egui::Id::new("sheet_entry_port_sync")).show_stable(ctx, 856., |ui| {
            ui.set_width(856.0);
            ui.set_min_height(503.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            let (title, _) = ui.allocate_exact_size(egui::vec2(856.0, 24.0), egui::Sense::hover());
            ui.painter().text(title.center(), egui::Align2::CENTER_CENTER, &dialog.title, egui::FontId::proportional(11.0), Color32::from_gray(230));
            let close_rect=egui::Rect::from_min_size(title.right_top()-egui::vec2(36.0,0.0),egui::vec2(36.0,20.0));
            close=ui.put(close_rect,egui::Button::new("×").fill(Color32::from_rgb(161,24,24))).clicked();
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_space(13.0);
                ui.spacing_mut().item_spacing.x=2.0;
                ui.spacing_mut().button_padding=egui::vec2(4.0,0.0);
                ui.spacing_mut().interact_size.y=16.0;
                if visible.is_empty() {
                    ui.add(egui::Button::new(egui::RichText::new("All Sheet Symbols Are Matched").size(11.0)).min_size(egui::vec2(0.0,16.0)));
                }
                for &n in &visible {
                    let fill=if dialog.tab==n{Color32::from_gray(66)}else{Color32::from_gray(48)};
                    if ui.add(egui::Button::new(egui::RichText::new(&dialog.tabs[n].caption).size(11.0)).fill(fill).min_size(egui::vec2(0.0,16.0))).clicked() {
                        dialog.tab=n;
                        dialog.entries.clear(); dialog.ports.clear(); dialog.links.clear();
                    }
                }
            });
            ui.horizontal_top(|ui| {
                ui.add_space(13.0);
                egui::Frame::NONE.stroke(egui::Stroke::new(1.0,Color32::from_gray(43)))
                    .inner_margin(egui::Margin{left:8,right:6,top:8,bottom:8}).show(ui, |ui| {
                    ui.set_min_size(egui::vec2(814.0,388.0));
                    if visible.is_empty() {
                        ui.allocate_ui_with_layout(egui::vec2(814.0,388.0),egui::Layout::top_down(egui::Align::Center),|ui| {
                            ui.centered_and_justified(|ui|{ui.label("All Sheet symbols are matched");});
                        });
                    } else {
                        match &comparisons[dialog.tab] {
                            Err(e)=>{ui.colored_label(Color32::LIGHT_RED,e);}
                            Ok(c)=> {
                                let t=&dialog.tabs[dialog.tab];
                                ui.horizontal_top(|ui| {
                                    ui.vertical(|ui| {
                                        table(ui,"unmatched_entries",&t.caption,"Unmatched Sheet Entries",&c.entries,&mut dialog.entries,236.0,331.0);
                                        ui.add_space(1.0);
                                        if ui.add_enabled(!dialog.entries.selected.is_empty(),egui::Button::new("Add Ports").min_size(egui::vec2(236.0,24.0))).clicked(){action=Some(Action::AddPorts);}
                                        ui.add_space(8.0);
                                        if ui.add_enabled(!dialog.entries.selected.is_empty(),egui::Button::new("Delete Sheet Entries").min_size(egui::vec2(236.0,24.0))).clicked(){action=Some(Action::DeleteEntries);}
                                    });
                                    ui.add_space(14.0);
                                    ui.vertical(|ui| {
                                        table(ui,"unmatched_ports",&t.file,"Unmatched Ports",&c.ports,&mut dialog.ports,236.0,331.0);
                                        ui.add_space(1.0);
                                        if ui.add_enabled(!dialog.ports.selected.is_empty(),egui::Button::new("Add Sheet Entries").min_size(egui::vec2(236.0,24.0))).clicked(){action=Some(Action::AddEntries);}
                                        ui.add_space(8.0);
                                        if ui.add_enabled(!dialog.ports.selected.is_empty(),egui::Button::new("Delete Ports").min_size(egui::vec2(236.0,24.0))).clicked(){action=Some(Action::DeletePorts);}
                                    });
                                    ui.add_space(10.0);
                                    ui.vertical(|ui| {
                                        ui.set_width(56.0);ui.add_space(96.0);
                                        if link_button(ui,!dialog.entries.selected.is_empty(),Some(true)).on_hover_text("Use Sheet Entry properties to update selected Ports; create any missing Ports").clicked(){action=Some(Action::CopyEntries);}
                                        ui.add_space(4.0);
                                        if link_button(ui,!dialog.ports.selected.is_empty(),Some(false)).on_hover_text("Use Port properties to update selected Sheet Entries; create any missing Entries").clicked(){action=Some(Action::CopyPorts);}
                                        ui.add_space(4.0);
                                        if link_button(ui,!dialog.links.selected.is_empty(),None).on_hover_text("Unlink selected pairs").clicked(){action=Some(Action::Unlink);}
                                    });
                                    ui.add_space(10.0);
                                    let links:Vec<_>=c.links.iter().map(|(e,_)|e.clone()).collect();
                                    table(ui,"matched_links","","Links",&links,&mut dialog.links,252.0,387.0);
                                });
                            }
                        }
                    }
                });
            });
            if let Some(e)=&dialog.error {ui.colored_label(Color32::LIGHT_RED,e);}
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x=12.0;
                ui.allocate_ui_with_layout(egui::vec2(240.0,25.0),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                    ui.add_space(12.0);
                    ui.add(egui::Checkbox::new(&mut dialog.only_unmatched,egui::RichText::new("Only show unmatched sheet symbols").size(11.0)));
                });
                ui.add_sized(egui::vec2(514.0,25.0),egui::Label::new(egui::RichText::new("Changes made in this dialog occur immediately, use Undo in each affected document to undo them").size(10.0)).truncate());
                close|=ui.add_sized(egui::vec2(74.0,25.0),egui::Button::new("Close")).clicked();
            });
        });
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape)) {
            close = true;
        }
        if let Some(action) = action {
            dialog.error = self.sync_action(&mut dialog, action).err();
        }
        if !close {
            self.sheet_sync.dialog = Some(dialog);
        }
    }
    fn sync_action(&mut self, dialog: &mut Dialog, action: Action) -> Result<(), String> {
        let t = &dialog.tabs[dialog.tab];
        let (p, c) = self.sync_indices(t)?;
        let comparison = sync::compare(
            &self.documents[p].model,
            &self.documents[c].model,
            t.symbol,
            &dialog.excluded,
        )?;
        if matches!(action, Action::Unlink) {
            dialog.excluded.extend(
                comparison
                    .links
                    .iter()
                    .filter(|(e, _)| dialog.links.selected.contains(&e.id))
                    .map(|(e, p)| (e.id, p.id)),
            );
            dialog.links.clear();
            return Ok(());
        }
        if matches!(action, Action::DeleteEntries | Action::DeletePorts) {
            let (target, rows) = if matches!(action, Action::DeleteEntries) {
                (p, &dialog.entries)
            } else {
                (c, &dialog.ports)
            };
            let command = edit::Edit::Batch(
                rows.selected
                    .iter()
                    .copied()
                    .map(edit::Edit::Delete)
                    .collect(),
            );
            let revision = self.documents[target].history.revision;
            self.documents[target].edit_schematic(command, revision)?;
            dialog.entries.clear();
            dialog.ports.clear();
            return Ok(());
        }
        let to_ports = matches!(action, Action::CopyEntries | Action::AddPorts);
        let source: Vec<_> = if to_ports {
            comparison
                .entries
                .iter()
                .filter(|s| dialog.entries.selected.contains(&s.id))
                .cloned()
                .collect()
        } else {
            comparison
                .ports
                .iter()
                .filter(|s| dialog.ports.selected.contains(&s.id))
                .cloned()
                .collect()
        };
        let targets: Vec<_> = match action {
            Action::CopyEntries => comparison
                .ports
                .iter()
                .filter(|s| dialog.ports.selected.contains(&s.id))
                .map(|s| s.id)
                .collect(),
            Action::CopyPorts => comparison
                .entries
                .iter()
                .filter(|s| dialog.entries.selected.contains(&s.id))
                .map(|s| s.id)
                .collect(),
            _ => vec![],
        };
        let target = if to_ports { c } else { p };
        let transfer = sync::transfer(
            &self.documents[target].model,
            if to_ports {
                Destination::Ports
            } else {
                Destination::Entries(t.symbol)
            },
            &source,
            &targets,
        )?;
        let relink: Vec<_> = source
            .iter()
            .zip(&targets)
            .map(|(s, t)| if to_ports { (s.id, *t) } else { (*t, s.id) })
            .collect();
        if transfer.create.is_empty() {
            let command = transfer.edit_at(&self.documents[target].model, Point::default())?;
            let revision = self.documents[target].history.revision;
            self.documents[target].edit_schematic(command, revision)?;
            dialog.excluded.retain(|pair| !relink.contains(pair));
            dialog.entries.clear();
            dialog.ports.clear();
        } else {
            self.active = Some(target);
            let doc = &mut self.documents[target];
            doc.sch_editor.cancel(&mut doc.view);
            doc.view.sch_tool_active = true;
            if let Destination::Entries(owner) = transfer.destination
                && let Some(K::SheetSymbol { a, b, .. }) = doc
                    .model
                    .schematic
                    .as_ref()
                    .and_then(|s| s.primitive(owner))
                    .map(|p| &p.kind)
            {
                doc.view.camera.centre_mm = [
                    (a.x as f64 + b.x as f64) / 2e6,
                    (a.y as f64 + b.y as f64) / 2e6,
                ];
                doc.view.camera.pixels_per_mm =
                    (550e6 / ((b.x - a.x).max(a.y - b.y).max(1) as f64)).clamp(1.0, 12.0);
                doc.view.camera.fitted = true;
            }
            dialog.pending = Some(PendingPlacement {
                target: doc.session_id,
                guards: [p, c]
                    .into_iter()
                    .map(|i| {
                        (
                            self.documents[i].session_id,
                            self.documents[i].history.revision,
                        )
                    })
                    .collect(),
                transfer,
                relink,
            });
        }
        Ok(())
    }
    pub(super) fn sheet_sync_escape(&mut self) {
        if self
            .sheet_sync
            .dialog
            .as_ref()
            .is_some_and(|d| d.pending.is_some())
        {
            self.cancel_sheet_sync_placement();
        } else {
            self.sheet_sync.dialog = None;
        }
    }
    pub(super) fn cancel_sheet_sync_placement(&mut self) {
        if let Some(dialog) = &mut self.sheet_sync.dialog
            && let Some(pending) = dialog.pending.take()
            && let Some(doc) = self
                .documents
                .iter_mut()
                .find(|d| d.session_id == pending.target)
        {
            doc.sch_editor.cancel(&mut doc.view);
        }
    }
    pub(super) fn sheet_sync_input(
        &mut self,
        index: usize,
        input: &eda_render::SchematicInput,
        ctx: &egui::Context,
    ) -> bool {
        if self.documents[index].sch_editor.bar.resources.modal() {
            return true;
        }
        if self
            .sheet_sync
            .dialog
            .as_ref()
            .is_some_and(|d| d.pending.is_none())
        {
            return true;
        }
        if self
            .sheet_sync
            .dialog
            .as_ref()
            .is_some_and(|d| d.pending.is_some())
        {
            if input.secondary_clicked {
                self.cancel_sheet_sync_placement();
                return true;
            }
            let mut dialog = self.sheet_sync.dialog.take().unwrap();
            let pending = dialog.pending.as_ref().unwrap();
            if self.documents[index].session_id != pending.target {
                self.sheet_sync.dialog = Some(dialog);
                return true;
            }
            let stale = pending.guards.iter().any(|(id, rev)| {
                !self
                    .documents
                    .iter()
                    .any(|d| d.session_id == *id && d.history.revision == *rev)
            });
            let mut done = stale;
            if stale {
                dialog.error =
                    Some("A source or target sheet changed during placement; compare again".into());
            } else if let Some(at) = input.point {
                let doc = &mut self.documents[index];
                let s = doc.model.schematic.as_ref().unwrap();
                let at = edit::snap_point(s, at, s.sheet.snap_nm, s.sheet.options.snap_distance_nm);
                match pending.transfer.edit_at(&doc.model, at) {
                    Ok(command) => {
                        match edit::preview_fragment(&doc.model, &command) {
                            Ok(f) => {
                                let mut preview = eda_core::schematic::Schematic::blank();
                                preview.fonts = doc.model.schematic.as_ref().unwrap().fonts.clone();
                                preview.primitives = f.primitives;
                                doc.view.sch_preview = Some(preview);
                            }
                            Err(e) => dialog.error = Some(e),
                        }
                        if input.clicked
                            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter))
                        {
                            dialog.error = doc.edit_schematic(command, doc.history.revision).err();
                            if dialog.error.is_none() {
                                dialog
                                    .excluded
                                    .retain(|pair| !pending.relink.contains(pair));
                            }
                            done = true;
                            dialog.entries.clear();
                            dialog.ports.clear();
                        }
                    }
                    Err(e) => {
                        doc.view.sch_preview = None;
                        dialog.error = Some(e);
                    }
                }
            }
            if done {
                {
                    let doc = &mut self.documents[index];
                    doc.sch_editor.cancel(&mut doc.view);
                }
                dialog.pending = None;
            }
            self.sheet_sync.dialog = Some(dialog);
            ctx.request_repaint();
            return true;
        }
        if input.secondary_clicked
            && !self.documents[index].sch_editor.active()
            && let Some(id) = input.hit.or(self.documents[index].view.selected)
        {
            self.sheet_sync.popup = ctx
                .input(|i| i.pointer.hover_pos())
                .map(|pos| (index, id, pos));
            return true;
        }
        false
    }
    pub(super) fn sheet_sync_context_menu(&mut self, ctx: &egui::Context) {
        let Some((index, id, pos)) = self.sheet_sync.popup else {
            return;
        };
        let mut close = false;
        let mut start = false;
        let mut snippet = false;
        let is_sheet = self.documents[index]
            .model
            .schematic
            .as_ref()
            .and_then(|s| s.primitive(id))
            .is_some_and(|p| matches!(p.kind, K::SheetSymbol { .. }));
        let selected = self.documents[index].view.selected.is_some()
            || !self.documents[index].view.sch_selection.is_empty()
            || !self.documents[index].view.sch_segments.is_empty();
        let response = egui::Area::new(egui::Id::new("sheet_symbol_actions"))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    if is_sheet {
                        ui.menu_button("Sheet Symbol Actions", |ui| {
                            if ui.button("Synchronize Sheet Entries and Ports").clicked() {
                                start = true;
                                ui.close();
                            }
                        });
                    }
                    ui.menu_button("Snippets", |ui| {
                        if ui
                            .add_enabled(
                                selected,
                                egui::Button::new("Create Snippet from selected objects"),
                            )
                            .clicked()
                        {
                            snippet = true;
                            ui.close();
                        }
                    });
                });
            });
        let submenu_open = egui::Popup::is_any_open(ctx);
        if ctx.input(|i| {
            i.key_pressed(Key::Escape)
                || i.pointer.any_pressed()
                    && !submenu_open
                    && !response
                        .response
                        .rect
                        .contains(i.pointer.interact_pos().unwrap_or_default())
        }) {
            close = true;
        }
        if start {
            self.active = Some(index);
            self.documents[index].view.selected = Some(id);
            self.start_sheet_sync(true);
            close = true;
        }
        if snippet {
            self.active = Some(index);
            self.documents[index].sch_editor.bar.resources.open_create();
            close = true;
        }
        if close {
            self.sheet_sync.popup = None;
        }
    }
}
