use super::*;
impl PcbCraft {
    pub(super) fn set_pcb_3d(&mut self, enabled: bool) {
        let Some(index) = self.active else { return };
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
            return true;
        }
        false
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
