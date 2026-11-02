//! Shared selection memory and keyboard transforms for the two document editors.
use super::*;
use eda_geom::Point;

impl PcbCraft {
    pub(super) fn selection_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if ctx.wants_keyboard_input() || ctx.input(|i| !i.focused) {
            return false;
        }
        let Some(index) = self.active else {
            return false;
        };
        let popup_key = egui::Id::new("selection_popup");
        if let Some((session, _, deselect)) =
            ctx.data(|d| d.get_temp::<(Uuid, egui::Pos2, bool)>(popup_key))
            && session == self.documents[index].session_id
        {
            let escape = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
            let all = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::A));
            if escape || all {
                ctx.data_mut(|d| d.remove::<(Uuid, egui::Pos2, bool)>(popup_key));
            }
            if all {
                if deselect {
                    let d = &mut self.documents[index];
                    d.view.selected = None;
                    d.view.sch_selection.clear();
                    d.view.sch_segments.clear();
                    d.view.pcb.selected.clear();
                    d.view.pcb.selected_connections.clear();
                } else if self.documents[index].model.schematic.is_some() {
                    self.schematic_bar_action(index, schematic_active_bar::Action::All, ctx);
                } else {
                    self.pcb_action(pcb_editor::Action::SelectAll);
                }
            }
            return true;
        }
        if !self.documents[index].sch_editor.active()
            && !self.documents[index].pcb_editor.has_pending()
        {
            for (key, deselect) in [(Key::S, false), (Key::X, true)] {
                if consume_exact_key(ctx, egui::Modifiers::NONE, key) {
                    let at = ctx.pointer_latest_pos().unwrap_or(egui::pos2(400., 250.));
                    let session = self.documents[index].session_id;
                    ctx.data_mut(|d| {
                        d.insert_temp(egui::Id::new("selection_popup"), (session, at, deselect))
                    });
                    return true;
                }
            }
        }
        let modifiers = ctx.input(|i| {
            i.events
                .iter()
                .find_map(|e| match e {
                    egui::Event::Key {
                        pressed: true,
                        modifiers,
                        ..
                    } => Some(*modifiers),
                    _ => None,
                })
                .unwrap_or(i.modifiers)
        });
        if modifiers.shift
            && !modifiers.ctrl
            && !modifiers.alt
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, Key::C))
        {
            for doc in &mut self.documents {
                doc.view.sch_highlight.clear();
                doc.view.pcb.highlight_net = None;
                doc.view.pcb.highlight_nets.clear();
            }
            return true;
        }
        let doc = &mut self.documents[index];
        for (slot, key) in [
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
        ]
        .into_iter()
        .enumerate()
        {
            if !(modifiers.ctrl || modifiers.shift || modifiers.alt)
                || modifiers.ctrl && (modifiers.shift || modifiers.alt)
                || !ctx.input_mut(|i| i.consume_key(modifiers, key))
            {
                continue;
            }
            let schematic = doc.model.schematic.is_some();
            let view = &mut doc.view;
            let ids = if schematic {
                let mut ids = view.sch_selection.clone();
                if let Some(id) = view.selected
                    && !ids.contains(&id)
                {
                    ids.push(id);
                }
                ids
            } else {
                view.pcb.selected.clone()
            };
            if modifiers.ctrl {
                view.selection_memory[slot] = ids;
                view.segment_memory[slot] = view.sch_segments.clone();
            } else if modifiers.alt {
                let mut remembered = view.selection_memory[slot].clone();
                remembered.retain(|id| {
                    if let Some(s) = &doc.model.schematic {
                        s.primitive(*id).is_some() || s.components.iter().any(|c| c.id == *id)
                    } else if let Some(b) = &doc.model.pcb {
                        b.item(*id).is_some()
                            || b.components.iter().any(|c| c.id == *id)
                            || b.polygon_pours.iter().any(|p| p.id == *id)
                    } else {
                        false
                    }
                });
                let selected = if schematic {
                    &mut view.sch_selection
                } else {
                    &mut view.pcb.selected
                };
                if !modifiers.shift {
                    selected.clear();
                }
                for id in remembered {
                    if !selected.contains(&id) {
                        selected.push(id);
                    }
                }
                if schematic {
                    if !modifiers.shift {
                        view.sch_segments.clear();
                    }
                    for span in &view.segment_memory[slot] {
                        if doc
                            .model
                            .schematic
                            .as_ref()
                            .is_some_and(|s| s.primitive(span.id).is_some())
                            && !view.sch_segments.contains(span)
                        {
                            view.sch_segments.push(*span);
                        }
                    }
                    view.selected = view
                        .sch_selection
                        .first()
                        .copied()
                        .or_else(|| view.sch_segments.first().map(|s| s.id));
                }
            } else {
                for id in ids {
                    if !view.selection_memory[slot].contains(&id) {
                        view.selection_memory[slot].push(id);
                    }
                }
                for span in &view.sch_segments {
                    if !view.segment_memory[slot].contains(span) {
                        view.segment_memory[slot].push(*span);
                    }
                }
            }
            return true;
        }
        if modifiers.ctrl && !modifiers.alt {
            for (key, x, y) in [
                (Key::ArrowLeft, -1, 0),
                (Key::ArrowRight, 1, 0),
                (Key::ArrowUp, 0, 1),
                (Key::ArrowDown, 0, -1),
            ] {
                if !ctx.input_mut(|i| i.consume_key(modifiers, key)) {
                    continue;
                }
                let step = doc
                    .model
                    .schematic
                    .as_ref()
                    .map_or(doc.view.grid_nm, |s| s.sheet.snap_nm)
                    * if modifiers.shift { 10 } else { 1 };
                let delta = Point {
                    x: x * step,
                    y: y * step,
                };
                let result = if let Some(s) = &doc.model.schematic {
                    let mut ids = doc.view.sch_selection.clone();
                    if let Some(id) = doc.view.selected
                        && !ids.contains(&id)
                    {
                        ids.push(id);
                    }
                    let selection = eda_core::schematic_edit::Selection {
                        ids,
                        segments: doc.view.sch_segments.clone(),
                    };
                    if selection.is_empty() {
                        return true;
                    }
                    let pivot = eda_core::schematic_edit::selection_anchor(s, &selection)
                        .unwrap_or_default();
                    doc.edit_schematic(
                        eda_core::schematic_edit::Edit::TransformSelection {
                            selection,
                            pivot,
                            quarter_turns: 0,
                            delta,
                            connected: false,
                        },
                        doc.history.revision,
                    )
                } else if let Some(board) = &doc.model.pcb {
                    if doc.view.pcb.selected.is_empty() {
                        return true;
                    }
                    board
                        .edited(eda_core::pcb::Edit::Transform {
                            selection: doc.view.pcb.selected.clone(),
                            pivot: Point::default(),
                            translation: delta,
                            angle: 0.,
                        })
                        .and_then(|board| pcb_editor::commit(doc, board))
                } else {
                    return false;
                };
                if let Err(error) = result {
                    self.message(true, error);
                }
                return true;
            }
        }
        false
    }
}

impl PcbCraft {
    pub(super) fn selection_popup(&mut self, ctx: &egui::Context) {
        use eda_core::schematic_arrange::Region;
        use schematic_active_bar::Action as Sch;
        let key = egui::Id::new("selection_popup");
        let Some((session, at, deselect)) =
            ctx.data(|d| d.get_temp::<(Uuid, egui::Pos2, bool)>(key))
        else {
            return;
        };
        let Some(index) = self
            .active
            .filter(|n| self.documents[*n].session_id == session)
        else {
            ctx.data_mut(|d| d.remove::<(Uuid, egui::Pos2, bool)>(key));
            return;
        };
        let schematic = self.documents[index].model.schematic.is_some();
        let mut sch = None;
        let mut pcb = None;
        let mut clear = false;
        let mut invert = false;
        let mut close = false;
        let area = egui::Area::new(key)
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .corner_radius(0)
                    .inner_margin(2)
                    .show(ui, |ui| {
                        ui.set_min_width(234.);
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        if deselect {
                            if selection_menu_row(ui, "All", true).clicked() {
                                clear = true;
                            }
                            for label in ["Inside Area", "Outside Area"] {
                                selection_menu_row(ui, label, false);
                            }
                        } else {
                            if schematic {
                                for (label, action) in [
                                    ("Lasso Select", Sch::Select(Region::Lasso)),
                                    ("Inside Area", Sch::Select(Region::Inside)),
                                    ("Outside Area", Sch::Select(Region::Outside)),
                                    ("Touching Rectangle", Sch::Select(Region::Touching)),
                                    ("Touching Line", Sch::Select(Region::Line)),
                                    ("All", Sch::All),
                                    ("Connection", Sch::Connection),
                                ] {
                                    if selection_menu_row(ui, label, true).clicked() {
                                        sch = Some(action);
                                    }
                                }
                            } else {
                                for (label, action) in [
                                    ("All", pcb_editor::Action::SelectAll),
                                    ("Board", pcb_editor::Action::SelectBoard),
                                    ("Net", pcb_editor::Action::SelectNet),
                                    ("Connected Copper", pcb_editor::Action::SelectCopper),
                                ] {
                                    if selection_menu_row(ui, label, true).clicked() {
                                        pcb = Some(action);
                                    }
                                }
                            }
                            ui.separator();
                            if selection_menu_row(ui, "Toggle Selection", true).clicked() {
                                invert = true;
                            }
                        }
                    });
            });
        close |= clear
            || invert
            || sch.is_some()
            || pcb.is_some()
            || ctx.input(|i| {
                i.key_pressed(Key::Escape)
                    || i.pointer.any_pressed()
                        && !area
                            .response
                            .rect
                            .contains(i.pointer.latest_pos().unwrap_or(at))
            });
        if clear {
            let d = &mut self.documents[index];
            d.view.selected = None;
            d.view.sch_selection.clear();
            d.view.sch_segments.clear();
            d.view.pcb.selected.clear();
            d.view.pcb.selected_connections.clear();
        }
        if invert {
            if schematic {
                sch = Some(Sch::Toggle);
            } else {
                let d = &mut self.documents[index];
                let b = d.model.pcb.as_ref().unwrap();
                let previous = d.view.pcb.selected.clone();
                d.view.pcb.selected = b
                    .items
                    .iter()
                    .filter(|i| {
                        d.view.pcb.selectable_on_board(b, i)
                            && !previous.contains(&i.id)
                            && !i.component.is_some_and(|n| {
                                b.components
                                    .iter()
                                    .any(|c| c.index == n && previous.contains(&c.id))
                            })
                    })
                    .map(|i| i.id)
                    .collect();
            }
        }
        if let Some(action) = sch {
            self.schematic_bar_action(index, action, ctx);
        }
        if let Some(action) = pcb {
            self.pcb_action(action);
        }
        if close {
            ctx.data_mut(|d| d.remove::<(Uuid, egui::Pos2, bool)>(key));
        }
    }
}

fn selection_menu_row(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(234., 24.),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    if enabled && response.hovered() {
        ui.painter()
            .rect_filled(rect, 0., Color32::from_rgb(75, 111, 137));
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(30., 0.),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.),
        if enabled {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        },
    );
    response
}

// egui's logical matching deliberately ignores extra Alt/Shift. Select's S
// must not consume PCB Shift+S or an Alt menu accelerator.
fn consume_exact_key(ctx: &egui::Context, modifiers: egui::Modifiers, key: Key) -> bool {
    ctx.input_mut(|i|{
        let mut consumed=false;
        i.events.retain(|e|{
            let matches=matches!(e,egui::Event::Key{key:actual,pressed:true,modifiers:actual_mods,..} if *actual==key&&actual_mods.matches_exact(modifiers));
            consumed|=matches;!matches
        });consumed
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_core::schematic::SchPrimitiveKind as K;
    use eda_core::schematic_edit::{Selection, primitive};

    #[test]
    fn select_popup_does_not_consume_shift_s_or_alt_menu_accelerators() {
        let mut app = schematic_editor::tests::app();
        let ctx = egui::Context::default();
        for modifiers in [egui::Modifiers::SHIFT, egui::Modifiers::ALT] {
            let _ = ctx.run(
                egui::RawInput {
                    modifiers,
                    events: vec![egui::Event::Key {
                        key: Key::S,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    assert!(!app.selection_shortcuts(ctx));
                    assert!(ctx.input(|i| i.key_pressed(Key::S)));
                },
            );
        }
    }

    fn key(app: &mut PcbCraft, ctx: &egui::Context, key: Key, modifiers: egui::Modifiers) {
        let _ = ctx.run(
            egui::RawInput {
                focused: true,
                modifiers,
                events: vec![
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    },
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers,
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                assert!(app.selection_shortcuts(ctx));
            },
        );
    }
    #[test]
    fn memory_and_keyboard_nudge_preserve_history_and_ignore_deleted_objects() {
        let mut app = schematic_editor::tests::app();
        let ctx = egui::Context::default();
        let p = primitive(
            K::Polyline {
                points: vec![Point::mm(10, 10), Point::mm(20, 10)],
                electrical: "Wire".into(),
                closed: false,
            },
            None,
        );
        let id = p.id;
        app.documents[0]
            .model
            .schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(p);
        app.documents[0].view.sch_selection = vec![id];
        app.documents[0].view.selected = Some(id);
        let original = app.documents[0].model.clone();
        key(&mut app, &ctx, Key::Num1, egui::Modifiers::CTRL);
        key(&mut app, &ctx, Key::ArrowRight, egui::Modifiers::CTRL);
        assert_eq!(app.documents[0].history.revision, 1);
        assert_ne!(app.documents[0].model, original);
        let d = &mut app.documents[0];
        d.history.undo(&mut d.model);
        assert_eq!(d.model, original);
        d.view.sch_selection.clear();
        d.view.selected = None;
        key(&mut app, &ctx, Key::Num1, egui::Modifiers::ALT);
        assert_eq!(app.documents[0].view.sch_selection, vec![id]);
        let d = &mut app.documents[0];
        d.edit_schematic(
            eda_core::schematic_edit::Edit::DeleteSelection(Selection {
                ids: vec![id],
                segments: vec![],
            }),
            d.history.revision,
        )
        .unwrap();
        key(&mut app, &ctx, Key::Num1, egui::Modifiers::ALT);
        assert!(app.documents[0].view.sch_selection.is_empty());
    }
    #[test]
    fn alt_net_highlight_is_not_a_group_edit_selection_and_clear_does_not_edit() {
        let mut app = schematic_editor::tests::app();
        let mut ids = vec![];
        for x in [10, 40] {
            let wire = primitive(
                K::Polyline {
                    points: vec![Point::mm(x, 10), Point::mm(x + 10, 10)],
                    electrical: "Wire".into(),
                    closed: false,
                },
                None,
            );
            ids.push(wire.id);
            let mut label = primitive(
                K::Text {
                    position: Point::mm(x, 10),
                    text: "SIGNAL".into(),
                    font: 0,
                    orientation: 0,
                    justification: 0,
                },
                None,
            );
            label.text_role = Some(eda_core::schematic::TextRole::NetLabel);
            app.documents[0]
                .model
                .schematic
                .as_mut()
                .unwrap()
                .primitives
                .extend([wire, label]);
        }
        let original = app.documents[0].model.clone();
        app.highlight_schematic_net(0, Some(ids[0]));
        assert!(
            ids.iter()
                .all(|id| app.documents[0].view.sch_highlight.contains(id))
        );
        assert_eq!(app.documents[0].view.sch_selection, vec![ids[0]]);
        let ctx = egui::Context::default();
        key(&mut app, &ctx, Key::C, egui::Modifiers::SHIFT);
        assert!(app.documents[0].view.sch_highlight.is_empty());
        assert_eq!(app.documents[0].model, original);
        assert_eq!(app.documents[0].history.revision, 0);
    }
}
