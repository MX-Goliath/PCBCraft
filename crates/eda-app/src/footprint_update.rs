//! Direct PcbLib updates observed in AD 24.4.1: one options dialog, all open PCBs.
use super::*;
use eda_core::{design_sync::Footprint, footprint_update as update};

struct Snapshot {
    session: Uuid,
    revision: u64,
    path: Option<PathBuf>,
}
impl Snapshot {
    fn of(doc: &OpenDocument) -> Self {
        Self {
            session: doc.session_id,
            revision: doc.history.revision,
            path: doc.path.clone(),
        }
    }
    fn check<'a>(&self, docs: &'a [OpenDocument]) -> Result<&'a OpenDocument, String> {
        let d = docs
            .iter()
            .find(|d| d.session_id == self.session)
            .ok_or("A compared document was closed; reopen the footprint update")?;
        let pending = d.pcb_editor.has_pending()
            || d.model
                .pcb
                .as_ref()
                .is_some_and(|b| d.pcb_editor.properties_dirty(b, d.history.revision));
        if d.read_only || pending || d.history.revision != self.revision || d.path != self.path {
            return Err(format!(
                "{} changed, is read-only or has an unfinished edit; reopen the footprint update",
                d.model.name
            ));
        }
        Ok(d)
    }
}
pub(super) struct Dialog {
    source: Snapshot,
    targets: Vec<Snapshot>,
    footprints: Vec<Footprint>,
    options: update::Options,
}
fn matches(doc: &OpenDocument, footprints: &[Footprint]) -> bool {
    doc.model.library.is_none()
        && doc.model.pcb.as_ref().is_some_and(|b| {
            b.components.iter().any(|c| {
                footprints
                    .iter()
                    .any(|f| f.name().eq_ignore_ascii_case(&c.footprint))
            })
        })
}
impl Dialog {
    fn new(source: &OpenDocument, docs: &[OpenDocument], all: bool) -> Result<Self, String> {
        let members: Vec<_> = if all {
            edit::members(&source.model).collect()
        } else {
            vec![&source.model]
        };
        let footprints = members
            .into_iter()
            .map(|m| Footprint::from_library_board(m.pcb.clone().ok_or("Select a PCB library")?))
            .collect::<Result<Vec<_>, String>>()?;
        let targets = docs
            .iter()
            .filter(|d| matches(d, &footprints))
            .map(Snapshot::of)
            .collect();
        let dialog = Self {
            source: Snapshot::of(source),
            targets,
            footprints,
            options: Default::default(),
        };
        dialog.check(docs)?;
        Ok(dialog)
    }
    fn check(&self, docs: &[OpenDocument]) -> Result<(), String> {
        self.source.check(docs)?;
        for target in &self.targets {
            target.check(docs)?;
        }
        let actual: std::collections::BTreeSet<_> = docs
            .iter()
            .filter(|d| matches(d, &self.footprints))
            .map(|d| d.session_id)
            .collect();
        if actual != self.targets.iter().map(|s| s.session).collect() {
            return Err("The open PCB scope changed; reopen the footprint update".into());
        }
        Ok(())
    }
    fn prepare(&self, docs: &[OpenDocument]) -> Result<Vec<(Uuid, Document, usize)>, String> {
        self.check(docs)?;
        let source = self.source.check(docs)?;
        let path = source.path.as_ref().map(|p| {
            eda_io::project_files::native_path(p)
                .to_string_lossy()
                .into_owned()
        });
        self.targets
            .iter()
            .map(|t| {
                let d = t.check(docs)?;
                let changes: Vec<_> = d
                    .model
                    .pcb
                    .as_ref()
                    .unwrap()
                    .components
                    .iter()
                    .filter_map(|c| {
                        self.footprints
                            .iter()
                            .find(|f| f.name().eq_ignore_ascii_case(&c.footprint))
                            .map(|f| update::Update {
                                component: c.id,
                                footprint: f.clone(),
                                options: self.options.clone(),
                                source_library: path.clone(),
                            })
                    })
                    .collect();
                Ok((
                    t.session,
                    update::preview(&d.model, &changes)?,
                    changes.len(),
                ))
            })
            .collect()
    }
    fn execute(&self, docs: &mut [OpenDocument]) -> Result<(usize, usize), String> {
        let prepared = self.prepare(docs)?;
        // No document/history changes until every target has passed validation.
        let mut components = 0;
        let mut boards = 0;
        for (session, next, count) in prepared {
            let d = docs.iter_mut().find(|d| d.session_id == session).unwrap();
            if d.history.commit(&mut d.model, next)? {
                components += count;
                boards += 1;
                d.pcb_editor = Default::default();
                d.view.pcb.selected.clear();
            }
        }
        Ok((components, boards))
    }
}
impl PcbCraft {
    pub(super) fn open_footprint_update(&mut self, all: bool) {
        let Some(index) = self.active else { return };
        match Dialog::new(&self.documents[index], &self.documents, all) {
            Ok(mut dialog) => {
                dialog.options = self.library_editor.footprint_update_options.clone();
                self.library_editor.footprint_update = Some(dialog);
            }
            Err(error) => self.message(true, error),
        }
    }
    pub(super) fn footprint_update_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.library_editor.footprint_update.take() else {
            return;
        };
        let mut close = false;
        let mut execute = false;
        let response=egui::Modal::new(egui::Id::new("pcb_component_update_options"))
            .backdrop_color(Color32::TRANSPARENT)
            .frame(egui::Frame::window(&ctx.style()).corner_radius(0))
            .show(ctx,|ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(408.,442.),egui::Sense::hover());
                let origin=rect.min;
                let area=|x:f32,y:f32,w:f32,h:f32| egui::Rect::from_min_size(origin+vec2(x,y),vec2(w,h));
                ui.painter().text(origin+vec2(204.,7.),egui::Align2::CENTER_CENTER,"Component(s) Update Options",egui::FontId::proportional(12.),ui.visuals().text_color());
                // The native close box overlaps the frame margin. Register it
                // without a child layout, which would enlarge the modal bounds.
                let close_rect=area(374.,-7.,34.,23.);
                let close_response=ui.interact(close_rect,ui.id().with("close"),egui::Sense::click());
                ui.painter().rect_filled(close_rect,0.,if close_response.hovered() {Color32::from_rgb(220,0,0)} else {Color32::from_rgb(160,0,0)});
                ui.painter().text(close_rect.center(),egui::Align2::CENTER_CENTER,"\u{00d7}",egui::FontId::proportional(14.),Color32::WHITE);
                close=close_response.clicked();
                ui.painter().rect_stroke(area(0.,22.,408.,388.),0.,egui::Stroke::new(1.,ui.visuals().widgets.noninteractive.bg_stroke.color),egui::StrokeKind::Inside);
                ui.painter().text(origin+vec2(8.,39.),egui::Align2::LEFT_CENTER,"Primitives / Attributes to Update",egui::FontId::proportional(12.),ui.visuals().text_color());
                for (y,label) in [(54.,"Text"),(118.,"Other"),(230.,"Pad(s) Attributes"),(342.,"Parameter")] {
                    ui.painter().rect_filled(area(16.,y,376.,22.),0.,ui.visuals().widgets.noninteractive.bg_fill);
                    ui.painter().text(origin+vec2(22.,y+11.),egui::Align2::LEFT_CENTER,label,egui::FontId::proportional(12.),ui.visuals().strong_text_color());
                }
                for (y,label,value,enabled) in [
                    (86.,"On Any Layer",&mut dialog.options.text,true),
                    (150.,"On Silkscreen Layers",&mut dialog.options.silkscreen,true),
                    (174.,"On Mechanical Layers",&mut dialog.options.mechanical,true),
                    (198.,"On Other Layers",&mut dialog.options.other,true),
                    (262.,"Solder Mask Expansion",&mut dialog.options.solder_mask,true),
                    (286.,"Paste Mask Expansion",&mut dialog.options.paste_mask,true),
                    (310.,"Thermal Relief Overrides",&mut dialog.options.thermal_relief,false),
                    (374.,"Footprint Parameters",&mut dialog.options.parameters,true),
                ] {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(area(24.,y,350.,23.)),|ui| {
                        ui.add_enabled(enabled,egui::Checkbox::new(value,label)).on_disabled_hover_text("Copying native thermal overrides is not implemented. Matched pads retain the board's original override bytes.");
                    });
                }
                ui.scope_builder(egui::UiBuilder::new().max_rect(area(250.,417.,75.,23.)),|ui| {execute=ui.add_sized([75.,23.],egui::Button::new("OK")).clicked();});
                ui.scope_builder(egui::UiBuilder::new().max_rect(area(333.,417.,75.,23.)),|ui| {close|=ui.add_sized([75.,23.],egui::Button::new("Cancel")).clicked();});
            });
        close |= response.should_close();
        if execute {
            match dialog.execute(&mut self.documents) {
                Ok((components, boards)) => {
                    self.library_editor.footprint_update_options = dialog.options.clone();
                    self.message(
                        false,
                        format!("Updated {components} footprints in {boards} PCB documents"),
                    );
                    close = true;
                }
                Err(error) => self.message(true, error),
            }
        }
        if !close {
            self.library_editor.footprint_update = Some(dialog);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<OpenDocument> {
        let mut lib = edit::new(DocumentKind::Board, "Parts.PcbLib");
        let b = lib.pcb.as_mut().unwrap();
        let mut pad = eda_core::pcb::Item::new(
            eda_core::pcb::TOP,
            None,
            eda_core::pcb::Shape::Pad {
                position: Point::mm(1, 0),
                size: Point::mm(2, 1),
                rotation: 0.,
                shape: eda_core::pcb::PadShape::Rectangle,
                hole: 0,
                plated: true,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        pad.component = Some(b.components[0].index);
        b.items.push(pad);
        let mut board = Document::new(DocumentKind::Board, "One.PcbDoc");
        board.pcb = Some(b.clone());
        if let eda_core::pcb::Shape::Pad { size, .. } =
            &mut board.pcb.as_mut().unwrap().items[0].shape
        {
            *size = Point::mm(1, 1);
        }
        let mut second = board.clone();
        second.id = Uuid::new_v4();
        second.name = "Two.PcbDoc".into();
        vec![
            OpenDocument::new(lib, Some("Parts.PcbLib".into())),
            OpenDocument::new(board, Some("One.PcbDoc".into())),
            OpenDocument::new(second, Some("Two.PcbDoc".into())),
        ]
    }
    #[test]
    fn direct_update_all_open_boards_is_undoable_and_does_not_save() {
        let mut docs = fixture();
        docs[0].path = Some(PathBuf::from(r"\\?\C:\Parts.PcbLib"));
        let before: Vec<_> = docs.iter().map(|d| d.model.clone()).collect();
        let dialog = Dialog::new(&docs[0], &docs, false).unwrap();
        assert_eq!(dialog.execute(&mut docs).unwrap(), (2, 2));
        assert_eq!(docs[0].model, before[0]);
        for n in 1..=2 {
            let d = &mut docs[n];
            assert!(d.dirty());
            assert_eq!(
                d.model.pcb.as_ref().unwrap().components[0].properties["SOURCEFOOTPRINTLIBRARY"],
                r"C:\Parts.PcbLib"
            );
            assert_eq!(d.saved, Some(before[n].clone()));
            d.history.undo(&mut d.model);
            assert_eq!(d.model, before[n]);
        }
    }
    #[test]
    fn source_revision_scope_and_read_only_target_reject_the_entire_batch() {
        let mut docs = fixture();
        let dialog = Dialog::new(&docs[0], &docs, true).unwrap();
        docs[2].read_only = true;
        assert!(dialog.execute(&mut docs).is_err());
        assert_eq!(docs[1].history.revision, 0);
        docs[2].read_only = false;
        docs[0].history.revision += 1;
        assert!(dialog.execute(&mut docs).is_err());
        assert_eq!(docs[1].history.revision, 0);
        docs[0].history.revision = 0;
        docs.pop();
        assert!(dialog.execute(&mut docs).is_err());
        assert_eq!(docs[1].history.revision, 0);
    }
}
