use super::*;
use eda_core::cross_probe::{self as probe, Target};

#[derive(Default)]
pub(super) struct State {
    pub enabled: bool,
    prefix: Option<f64>,
    choices: Vec<(Uuid, Target)>,
    pub status: String,
}
impl PcbCraft {
    fn probe_project(&self, index: usize) -> Option<usize> {
        let d = &self.documents[index];
        self.projects.iter().position(|p| {
            p.documents.iter().any(|e| {
                d.source_path.as_ref() == Some(&e.path) || d.path.as_ref() == Some(&e.path)
            })
        })
    }
    pub(super) fn start_cross_probe(&mut self) {
        if self.cross_probe.enabled {
            self.cross_probe = State::default();
            return;
        }
        let Some(index) = self.active else { return };
        let d = &mut self.documents[index];
        if d.pcb_editor.has_pending()
            || d.model
                .pcb
                .as_ref()
                .is_some_and(|b| d.pcb_editor.properties_dirty(b, d.history.revision))
        {
            self.message(true, "Finish or cancel the PCB edit before Cross Probe");
            return;
        }
        d.sch_editor.cancel(&mut d.view);
        d.pcb_editor.cancel(&mut d.view);
        self.cross_probe.enabled = true;
        self.cross_probe.status = "Click a component, pin or net · Esc / right click exits".into();
    }
    pub(super) fn cross_probe_shortcuts(&mut self, ctx: &egui::Context) -> bool {
        let mut toggle = false;
        let mut cancel = false;
        let mut handled = false;
        ctx.input_mut(|i| {
            if self.cross_probe.enabled && i.consume_key(egui::Modifiers::NONE, Key::Escape) {
                cancel = true;
                handled = true;
                return;
            }
            if self.cross_probe.prefix.is_some_and(|t| i.time - t >= 1.5) {
                self.cross_probe.prefix = None;
            }
            if i.consume_key(egui::Modifiers::NONE, Key::T) {
                self.cross_probe.prefix = Some(i.time);
                handled = true;
            }
            if self.cross_probe.prefix.is_some() && i.consume_key(egui::Modifiers::NONE, Key::C) {
                self.cross_probe.prefix = None;
                toggle = true;
                handled = true;
            }
        });
        if cancel {
            self.cross_probe = State::default();
        }
        if toggle {
            self.start_cross_probe();
        }
        handled
    }
    pub(super) fn cross_probe_input(
        &mut self,
        index: usize,
        hit: Option<Uuid>,
        clicked: bool,
        right: bool,
        ctx: &egui::Context,
    ) -> bool {
        if self.cross_probe.enabled && right {
            self.cross_probe = State::default();
            return true;
        }
        let d = &self.documents[index];
        let idle = !d.sch_editor.active()
            && !d.pcb_editor.has_pending()
            && d.pcb_editor.tool == pcb_editor::Tool::Select;
        let quick = idle && ctx.input(|i| i.modifiers.ctrl && !i.modifiers.shift);
        if !(self.cross_probe.enabled || (quick && clicked)) {
            return false;
        }
        if clicked && let Some(id) = hit {
            self.probe_object(index, id);
        }
        true
    }
    fn probe_object(&mut self, index: usize, id: Uuid) {
        self.cross_probe.choices.clear();
        if self.documents[index].pcb_editor.has_pending() {
            self.probe_message("Finish or cancel the current edit first".into());
            return;
        }
        let source_id = self.documents[index].session_id;
        let pcb = self.documents[index].model.pcb.is_some();
        let key = match probe::source(&self.documents[index].model, id) {
            Ok(k) => k,
            Err(e) => {
                self.probe_message(e);
                return;
            }
        };
        let project = self.probe_project(index);
        if let Some(p) = project {
            let paths: Vec<_> = self.projects[p]
                .documents
                .iter()
                .filter(|e| e.exists && e.kind == if pcb { "schdoc" } else { "pcbdoc" })
                .map(|e| e.path.clone())
                .collect();
            for path in paths {
                self.open_path(path);
            }
            self.active = self
                .documents
                .iter()
                .position(|d| d.session_id == source_id);
        }
        let mut choices = Vec::new();
        for (i, d) in self.documents.iter().enumerate() {
            if d.model.pcb.is_some() == pcb || self.probe_project(i) != project {
                continue;
            }
            match probe::destinations(&key, &d.model) {
                Ok(targets) => choices.extend(targets.into_iter().map(|t| (d.session_id, t))),
                Err(e) => {
                    self.probe_message(e);
                    return;
                }
            }
        }
        match choices.len() {
            0=>self.probe_message("No corresponding object in the project. For free documents, open both schematic and PCB.".into()),
            1=>{let (id,target)=choices.remove(0);self.show_probe(id,target);},
            _=>{self.cross_probe.choices=choices;self.cross_probe.status="Choose the destination document".into();}
        }
    }
    fn probe_message(&mut self, text: String) {
        self.cross_probe.status = text.clone();
        self.settings.messages = true;
        self.settings.right_messages = true;
        self.message(false, format!("Cross Probe: {text}"));
    }
    fn show_probe(&mut self, id: Uuid, target: Target) {
        let Some(index) = self.documents.iter().position(|d| d.session_id == id) else {
            return;
        };
        let doc = &mut self.documents[index];
        if doc.pcb_editor.has_pending()
            || doc
                .model
                .pcb
                .as_ref()
                .is_some_and(|b| doc.pcb_editor.properties_dirty(b, doc.history.revision))
        {
            self.probe_message(
                "Finish or cancel the edit in the destination document first".into(),
            );
            return;
        }
        doc.sch_editor.cancel(&mut doc.view);
        doc.pcb_editor.cancel(&mut doc.view);
        if let Some(board) = &doc.model.pcb {
            doc.view.pcb.selected = target.ids;
            doc.view.pcb.highlight_net = target.net;
            for item in board.items.iter().filter(|p| {
                doc.view.pcb.selected.contains(&p.id)
                    || p.component.is_some_and(|c| {
                        board
                            .components
                            .iter()
                            .any(|b| b.index == c && doc.view.pcb.selected.contains(&b.id))
                    })
            }) {
                doc.view.pcb.visible.insert(item.layer);
            }
            doc.view.pcb.single_layer = 0;
            doc.view.pcb.requested_fit = Some(eda_render::pcb::FitTarget::Selection);
        } else {
            doc.view.sch_selection = target.ids;
            doc.view.sch_segments.clear();
            doc.view.selected = doc.view.sch_selection.first().copied();
            doc.view.camera.centre_mm =
                [target.centre.x as f64 / 1e6, target.centre.y as f64 / 1e6];
            doc.view.camera.pixels_per_mm = doc.view.camera.pixels_per_mm.max(12.0);
            doc.view.camera.fitted = true;
        }
        self.active = Some(index);
        self.settings.properties = true;
        self.settings.right_components = false;
        self.settings.right_messages = false;
        self.cross_probe.status = format!("Located in {} · Esc exits Cross Probe", doc.model.name);
    }
    pub(super) fn cross_probe_dialog(&mut self, ctx: &egui::Context) {
        if self.cross_probe.choices.is_empty() {
            return;
        }
        let mut open = true;
        let mut picked = None;
        egui::Window::new("Cross Probe — destination")
            .collapsible(false)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.label("Choose the destination document or physical instance:");
                for (n, (id, target)) in self.cross_probe.choices.iter().enumerate() {
                    if let Some(d) = self.documents.iter().find(|d| d.session_id == *id)
                        && ui
                            .push_id(n, |ui| {
                                ui.button(format!("{} · {}", d.model.name, target.label))
                            })
                            .inner
                            .clicked()
                    {
                        picked = Some(n);
                    }
                }
            });
        if let Some(n) = picked {
            let (id, target) = self.cross_probe.choices.remove(n);
            self.cross_probe.choices.clear();
            self.show_probe(id, target);
        }
        if !open {
            self.cross_probe.choices.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_core::{pcb::Shape, schematic::SchPrimitiveKind as K};
    fn app() -> PcbCraft {
        let mut app = PcbCraft {
            cross_probe: Default::default(),
            file_dialog: rfd::FileDialog::new(),
            documents: vec![OpenDocument::new(
                Document::new(DocumentKind::Schematic, "Interaction test"),
                None,
            )],
            projects: vec![],
            active: Some(0),
            settings: Default::default(),
            search: String::new(),
            log: vec![],
            command: None,
            pending: None,
            preferences: false,
            about: false,
            layer_stack: None,
            allow_exit: false,
            cursor_mm: None,
            pad_edit: None,
            property_ui: Default::default(),
            sch_clipboard: None,
            pcb_clipboard: None,
            components_ui: Default::default(),
        };
        app.documents.clear();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        let sch = eda_io::altium::load_schematic(&root.join("schematic/minimal.SchDoc")).unwrap();
        let pcb = eda_io::altium_pcb::load_pcb(&root.join("pcb-update/editor-v2.PcbDoc")).unwrap();
        app.documents.push(OpenDocument::new(sch, None));
        app.documents.push(OpenDocument::new(pcb, None));
        for d in &mut app.documents {
            d.saved = Some(d.model.clone());
        }
        app.active = Some(0);
        app
    }
    #[test]
    fn bidirectional_component_pin_and_net_probe_never_edits_documents() {
        let mut app = app();
        let component = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components[0]
            .id;
        app.probe_object(0, component);
        assert_eq!(app.active, Some(1));
        let selected = app.documents[1].view.pcb.selected[0];
        app.probe_object(1, selected);
        assert_eq!(app.active, Some(0));
        assert_eq!(app.documents[0].view.sch_selection, vec![component]);
        let pin = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(&p.kind,K::Pin{number,..} if number=="1"))
            .unwrap()
            .id;
        app.probe_object(0, pin);
        assert_eq!(app.active, Some(1));
        let pad = app.documents[1].view.pcb.selected[0];
        assert!(
            matches!(&app.documents[1].model.pcb.as_ref().unwrap().item(pad).unwrap().shape,Shape::Pad{name,..} if name=="1")
        );
        app.probe_object(1, pad);
        assert_eq!(app.active, Some(0));
        assert_eq!(app.documents[0].view.sch_selection, vec![pin]);
        let wire = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Wire"))
            .unwrap()
            .id;
        app.probe_object(0, wire);
        assert_eq!(app.active, Some(1));
        assert_eq!(app.documents[1].view.pcb.highlight_net, Some(1));
        let track = app.documents[1]
            .model
            .pcb
            .as_ref()
            .unwrap()
            .items
            .iter()
            .find(|p| matches!(p.shape, Shape::Track { .. }))
            .unwrap()
            .id;
        app.probe_object(1, track);
        assert_eq!(app.active, Some(0));
        assert!(app.documents[0].view.sch_selection.contains(&wire));
        assert!(
            app.documents
                .iter()
                .all(|d| !d.dirty() && d.history.revision == 0)
        );
    }
    #[test]
    fn source_unique_id_wins_over_renamed_designator_and_mismatch_is_not_guessed() {
        let mut app = app();
        let sch = app.documents[0].model.clone();
        let id = sch.schematic.as_ref().unwrap().components[0].id;
        let key = probe::source(&sch, id).unwrap();
        let c = &mut app.documents[1].model.pcb.as_mut().unwrap().components[0];
        c.designator = "R999".into();
        c.properties
            .insert("SOURCEUNIQUEID".into(), r"\SHEET\RESISTOR".into());
        assert!(
            probe::resolve(&key, &app.documents[1].model)
                .unwrap()
                .is_some()
        );
        let c = &mut app.documents[1].model.pcb.as_mut().unwrap().components[0];
        c.designator = "R1".into();
        c.properties
            .insert("SOURCEUNIQUEID".into(), "DIFFERENT".into());
        assert!(
            probe::resolve(&key, &app.documents[1].model)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn duplicate_documents_prompt_for_destination_and_missing_target_preserves_view() {
        let mut app = app();
        let pcb = app.documents[1].model.clone();
        app.documents.push(OpenDocument::new(pcb, None));
        let id = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components[0]
            .id;
        app.probe_object(0, id);
        assert_eq!(app.active, Some(0));
        assert_eq!(app.cross_probe.choices.len(), 2);
        app.documents.truncate(1);
        app.probe_object(0, id);
        assert_eq!(app.active, Some(0));
        assert!(app.cross_probe.choices.is_empty());
        assert!(app.cross_probe.status.contains("No corresponding"));
    }
    #[test]
    fn project_scope_does_not_probe_another_projects_identical_designators() {
        let mut app = app();
        for (i, d) in app.documents.iter_mut().enumerate() {
            d.path = Some(PathBuf::from(format!("/project-{i}/design")));
        }
        app.projects = (0..2)
            .map(|i| eda_io::altium::Project {
                path: PathBuf::from(format!("/project-{i}/project.PrjPcb")),
                name: format!("Project {i}"),
                warnings: vec![],
                documents: vec![eda_io::altium::ProjectEntry {
                    path: app.documents[i].path.clone().unwrap(),
                    original_path: "design".into(),
                    kind: if i == 0 { "schdoc" } else { "pcbdoc" }.into(),
                    exists: false,
                }],
            })
            .collect();
        let id = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components[0]
            .id;
        app.probe_object(0, id);
        assert_eq!(app.active, Some(0));
        assert!(app.cross_probe.status.contains("No corresponding"));
    }
    #[test]
    fn menu_sequence_survives_frames_and_escape_restores_editing() {
        let mut app = app();
        let ctx = egui::Context::default();
        let event = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        for (time, events) in [
            (1., vec![event(Key::T)]),
            (1.1, vec![]),
            (1.2, vec![event(Key::C)]),
        ] {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    app.cross_probe_shortcuts(ctx);
                },
            );
        }
        assert!(app.cross_probe.enabled);
        let _ = ctx.run(
            egui::RawInput {
                events: vec![event(Key::Escape)],
                ..Default::default()
            },
            |ctx| {
                app.cross_probe_shortcuts(ctx);
            },
        );
        assert!(!app.cross_probe.enabled);
    }
    #[test]
    fn unopened_project_counterpart_loads_once_and_keeps_source_clean() {
        let mut app = app();
        app.documents.clear();
        app.active = None;
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/cross-probe/demo.PrjPcb");
        app.open_path(path);
        assert_eq!(app.documents.len(), 1);
        let id = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components[0]
            .id;
        app.probe_object(0, id);
        assert_eq!(app.documents.len(), 2);
        assert_eq!(app.active, Some(1));
        app.probe_object(0, id);
        assert_eq!(app.documents.len(), 2);
        assert!(app.documents.iter().all(|d| !d.dirty()));
    }
    #[test]
    fn repeated_channels_prompt_for_physical_instance_in_same_board() {
        let mut app = app();
        let b = app.documents[1].model.pcb.as_mut().unwrap();
        b.components[0]
            .properties
            .insert("SOURCEUNIQUEID".into(), r"\CHANNEL1\RESISTOR".into());
        let mut copy = b.components[0].clone();
        copy.id = Uuid::new_v4();
        copy.index = 100;
        copy.designator = "R101".into();
        copy.properties
            .insert("SOURCEUNIQUEID".into(), r"\CHANNEL2\RESISTOR".into());
        b.components.push(copy);
        let id = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components[0]
            .id;
        app.probe_object(0, id);
        assert_eq!(app.active, Some(0));
        assert_eq!(app.cross_probe.choices.len(), 2);
        assert_ne!(
            app.cross_probe.choices[0].1.ids,
            app.cross_probe.choices[1].1.ids
        );
        assert_ne!(
            app.cross_probe.choices[0].1.label,
            app.cross_probe.choices[1].1.label
        );
    }
}
