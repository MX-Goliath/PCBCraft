use super::*;
impl PcbCraft {
    pub(super) fn set_pcb_3d(&mut self, enabled: bool) {
        let Some(index) = self.active else { return };
        if enabled && self.documents[index].read_only {
            self.git_enter_3d(index);
        }
        let doc = &mut self.documents[index];
        if doc.view.pcb.three_d.enabled == enabled {
            return;
        }
        if doc.model.pcb.is_none() {
            return;
        }
        if enabled
            && (doc.pcb_editor.has_pending()
                || doc
                    .model
                    .pcb
                    .as_ref()
                    .is_some_and(|b| doc.pcb_editor.properties_dirty(b, doc.history.revision)))
        {
            self.message(
                true,
                "Finish or cancel the current PCB edit before entering 3D Layout Mode",
            );
            return;
        }
        doc.pcb_editor.cancel(&mut doc.view);
        doc.view.pcb.three_d.cancel_drag();
        // A deferred Fit belongs to the mode in which it was requested.
        doc.view.pcb.requested_fit = None;
        if enabled {
            doc.view.pcb.three_d.enter_from_2d(&doc.view.camera);
        } else {
            doc.view.pcb.three_d.leave_to_2d(&mut doc.view.camera);
        }
        doc.view.pcb.three_d.enabled = enabled;
        doc.view.pcb.three_d.revision = doc.history.revision;
    }
    pub(super) fn pcb_view_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        if self
            .active
            .is_none_or(|i| self.documents[i].model.pcb.is_none())
        {
            return false;
        }
        if self.pcb_view_sequence_shortcuts(ctx) {
            return true;
        }
        let mode = ctx.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, Key::Num3) {
                Some(true)
            } else if i.consume_key(egui::Modifiers::NONE, Key::Num2) {
                Some(false)
            } else {
                None
            }
        });
        if let Some(enabled) = mode {
            self.set_pcb_3d(enabled);
            ctx.request_repaint();
            return true;
        }
        if let Some(index) = self.active
            && self.documents[index].view.pcb.three_d.enabled
        {
            use eda_render::pcb_3d::CameraCommand::*;
            let command = ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::CTRL, Key::Num0) {
                    Some(FlippedIsometric)
                } else if i.consume_key(egui::Modifiers::NONE, Key::Num0) {
                    Some(Isometric)
                } else if i.consume_key(egui::Modifiers::NONE, Key::Num9) {
                    Some(QuarterTurn)
                } else {
                    None
                }
            });
            if let Some(command) = command {
                self.documents[index]
                    .view
                    .pcb
                    .three_d
                    .camera_command(command);
                return true;
            }
        }
        let index = self.active.unwrap();
        let doc = &mut self.documents[index];
        let mut action = None;
        let handled = ctx.input_mut(|input| {
            let ctrl = egui::Modifiers::CTRL;
            let shift = egui::Modifiers::SHIFT;
            if input.consume_key(ctrl, Key::F) {
                if doc.view.pcb.three_d.enabled {
                    doc.view.pcb.three_d.flip();
                } else {
                    doc.view.camera.mirrored_x = !doc.view.camera.mirrored_x;
                }
                return true;
            }
            if input.consume_key(shift, Key::Num7) {
                doc.view.pcb.three_d.show_components = !doc.view.pcb.three_d.show_components;
                doc.view.pcb.three_d.invalidate();
                return true;
            }
            if input.consume_key(shift, Key::E) {
                doc.view.pcb.snap.mode = match doc.view.pcb.snap.mode {
                    eda_core::pcb::SnapLayerMode::All => eda_core::pcb::SnapLayerMode::Current,
                    eda_core::pcb::SnapLayerMode::Current => eda_core::pcb::SnapLayerMode::Off,
                    eda_core::pcb::SnapLayerMode::Off => eda_core::pcb::SnapLayerMode::All,
                };
                return true;
            }
            if input.consume_key(shift, Key::S) {
                if doc.view.pcb.three_d.enabled {
                    doc.view.pcb.three_d.presentation.single_layer =
                        !doc.view.pcb.three_d.presentation.single_layer;
                } else {
                    doc.view.pcb.single_layer = (doc.view.pcb.single_layer + 1) % 3;
                }
                return true;
            }
            if input.consume_key(shift, Key::C) {
                doc.view.pcb.highlight_net = None;
                doc.view.pcb.highlight_nets.clear();
                return true;
            }
            for (mods, key, candidate) in [
                (
                    ctrl,
                    Key::PageDown,
                    pcb_editor::Action::Fit(eda_render::pcb::FitTarget::Document),
                ),
                (
                    ctrl,
                    Key::Home,
                    pcb_editor::Action::Fit(eda_render::pcb::FitTarget::Selection),
                ),
                (egui::Modifiers::NONE, Key::L, pcb_editor::Action::Layers),
                (egui::Modifiers::NONE, Key::G, pcb_editor::Action::Grid),
            ] {
                if input.consume_key(mods, key) {
                    action = Some(candidate);
                    return true;
                }
            }
            false
        });
        if let Some(action) = action {
            self.pcb_action(action);
        }
        handled
    }
    pub(super) fn pcb_3d_input(
        &mut self,
        index: usize,
        input: &eda_render::pcb::Input,
        ctx: &egui::Context,
    ) -> bool {
        let doc = &mut self.documents[index];
        if !doc.view.pcb.three_d.enabled {
            return false;
        }
        if input.clicked || input.double_clicked {
            let hit = input.hit.map(|id| {
                doc.model
                    .pcb
                    .as_ref()
                    .and_then(|b| {
                        b.item(id).and_then(|p| p.component).and_then(|index| {
                            b.components.iter().find(|c| c.index == index).map(|c| c.id)
                        })
                    })
                    .unwrap_or(id)
            });
            if ctx.input(|i| i.modifiers.shift) {
                if let Some(id) = hit {
                    if doc.view.pcb.selected.contains(&id) {
                        doc.view.pcb.selected.retain(|v| *v != id);
                    } else {
                        doc.view.pcb.selected.push(id);
                    }
                }
            } else {
                doc.view.pcb.selected = hit.into_iter().collect();
            }
        }
        true
    }
}
