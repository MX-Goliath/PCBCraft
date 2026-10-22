use super::*;
use eda_core::annotation::{self as annotate, Operation, Options, Order, Plan, Sheet};
use std::collections::HashSet;

struct Scope {
    session: Uuid,
    revision: u64,
    name: String,
    enabled: bool,
    start: u32,
    selected: HashSet<Uuid>,
}
pub(super) struct Dialog {
    title: String,
    sheets: Vec<Scope>,
    options: Options,
    selected_only: bool,
    plan: Option<Plan>,
    error: String,
    eco: bool,
    validated: bool,
    executed: bool,
    changed_only: bool,
}
impl Dialog {
    fn documents<'a>(
        &self,
        documents: &'a [OpenDocument],
    ) -> Result<Vec<&'a OpenDocument>, String> {
        self.sheets
            .iter()
            .map(|s| {
                let d = documents
                    .iter()
                    .find(|d| d.session_id == s.session)
                    .ok_or("A schematic sheet was closed")?;
                if d.history.revision != s.revision
                    || d.sch_editor.active()
                    || d.view.sch_preview.is_some()
                {
                    return Err(format!(
                        "{} changed or has an unfinished edit; close and reopen Annotation",
                        d.model.name
                    ));
                }
                Ok(d)
            })
            .collect()
    }
    fn update(&mut self, documents: &[OpenDocument], op: Operation) -> Result<(), String> {
        let docs = self.documents(documents)?;
        let sheets: Vec<_> = self
            .sheets
            .iter()
            .zip(docs)
            .map(|(s, d)| Sheet {
                document: &d.model,
                enabled: s.enabled,
                start: s.start,
                selected: self.selected_only.then_some(&s.selected),
            })
            .collect();
        self.plan = Some(annotate::plan(&sheets, &self.options, op)?);
        self.validated = false;
        self.error.clear();
        Ok(())
    }
    fn validate(&self, documents: &[OpenDocument]) -> Result<Vec<Document>, String> {
        let docs = self.documents(documents)?;
        let models: Vec<_> = docs.iter().map(|d| &d.model).collect();
        self.plan
            .as_ref()
            .ok_or("Update the changes list first")?
            .prepare(&models)
    }
    fn execute(&mut self, documents: &mut [OpenDocument]) -> Result<usize, String> {
        if self.executed {
            return Err("Changes have already been executed".into());
        }
        if !self.validated {
            return Err("Validate Changes before Execute Changes".into());
        }
        let prepared = self.validate(documents)?;
        // Validate every target before the first mutation. The UI thread cannot interleave edits.
        let mut targets = vec![];
        for next in prepared {
            let index = documents
                .iter()
                .position(|d| {
                    d.model.id == next.id && self.sheets.iter().any(|s| s.session == d.session_id)
                })
                .ok_or("Target document missing")?;
            next.validate()?;
            targets.push((index, next));
        }
        let count = targets.len();
        for (i, next) in targets {
            documents[i].history.commit(&mut documents[i].model, next)?;
        }
        self.executed = true;
        Ok(count)
    }
}
impl PcbCraft {
    pub(super) fn open_annotation(&mut self) {
        if let Err(e) = self.annotation_scope() {
            self.message(true, e);
        }
    }
    fn annotation_scope(&mut self) -> Result<(), String> {
        let index = self.active.ok_or("Open a schematic first")?;
        let doc = &self.documents[index];
        if doc.model.schematic.is_none() {
            return Err("Annotation requires a schematic document".into());
        }
        let active = doc.session_id;
        let projects: Vec<_> = self
            .projects
            .iter()
            .filter(|p| {
                p.documents.iter().any(|e| {
                    doc.path.as_ref() == Some(&e.path) || doc.source_path.as_ref() == Some(&e.path)
                })
            })
            .collect();
        if projects.len() > 1 {
            return Err("This sheet belongs to multiple open projects. Close the other projects before project annotation".into());
        }
        let (title, paths) = if let Some(p) = projects.first() {
            (
                p.name.clone(),
                p.documents
                    .iter()
                    .filter(|e| e.kind == "schdoc")
                    .map(|e| e.path.clone())
                    .collect::<Vec<_>>(),
            )
        } else {
            (doc.model.name.clone(), vec![])
        };
        let mut sessions = vec![];
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            self.open_path(path.clone());
            let Some(d) = self
                .documents
                .iter()
                .find(|d| d.source_path.as_ref() == Some(&path) || d.path.as_ref() == Some(&path))
            else {
                self.active = self.documents.iter().position(|d| d.session_id == active);
                return Err(format!(
                    "Could not load all project sheets: {}. Annotation was not started.",
                    path.display()
                ));
            };
            if !sessions.contains(&d.session_id) {
                sessions.push(d.session_id);
            }
        }
        self.active = self.documents.iter().position(|d| d.session_id == active);
        if sessions.is_empty() {
            sessions.push(active);
        }
        let mut sheets = vec![];
        for session in sessions {
            let d = self
                .documents
                .iter()
                .find(|d| d.session_id == session)
                .unwrap();
            let s = d
                .model
                .schematic
                .as_ref()
                .ok_or("Project entry is not a schematic")?;
            if d.sch_editor.active() || d.view.sch_preview.is_some() {
                return Err(format!(
                    "Finish or cancel the edit on {} before annotation",
                    d.model.name
                ));
            }
            let selected: HashSet<_> = d
                .view
                .sch_selection
                .iter()
                .copied()
                .chain(d.view.selected)
                .filter_map(|id| {
                    if s.components.iter().any(|c| c.id == id) {
                        Some(id)
                    } else {
                        s.primitives
                            .iter()
                            .chain(&s.parameter_objects)
                            .find(|p| p.id == id)
                            .and_then(|p| p.component)
                    }
                })
                .collect();
            sheets.push(Scope {
                session,
                revision: d.history.revision,
                name: d.model.name.clone(),
                enabled: true,
                start: 1,
                selected,
            });
        }
        self.annotation = Some(Dialog {
            title,
            sheets,
            options: Options::default(),
            selected_only: false,
            plan: None,
            error: String::new(),
            eco: false,
            validated: false,
            executed: false,
            changed_only: false,
        });
        Ok(())
    }
    pub(super) fn annotation_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.annotation.take() else {
            return;
        };
        let mut close = false;
        let mut update = None;
        let mut validate = false;
        let mut execute = false;
        egui::Modal::new(egui::Id::new("schematic_annotation")).show(ctx,|ui|{
            ui.set_width((ctx.screen_rect().width()-60.0).clamp(500.0,1060.0));
            ui.heading(if d.eco{"Engineering Change Order"}else{"Annotate"});ui.label(&d.title);ui.separator();
            if !d.eco {
                let mut changed=false;
                ui.horizontal(|ui|{
                    ui.label("Order of Processing");egui::ComboBox::from_id_salt("annotation_order").selected_text(d.options.order.label()).show_ui(ui,|ui|{for o in Order::ALL{changed|=ui.selectable_value(&mut d.options.order,o,o.label()).changed();}});
                    changed|=ui.checkbox(&mut d.selected_only,"Selected Parts Only").changed();
                    changed|=ui.checkbox(&mut d.options.pack_parts,"Group multi-part components").changed();
                });
                ui.horizontal(|ui|{changed|=ui.checkbox(&mut d.options.reset_existing,"Renumber existing designators").changed();ui.label("Locked designators are preserved");});
                ui.label("Schematic Sheets to Annotate");
                let mut move_sheet=None;
                egui::ScrollArea::vertical().id_salt("annotation_sheets").max_height(130.0).show(ui,|ui|{
                    egui::Grid::new("annotation_sheet_grid").striped(true).show(ui,|ui|{
                        ui.strong("Enable");ui.strong("Schematic Document");ui.strong("Start Index");ui.strong("Order");ui.end_row();
                        for (i,s) in d.sheets.iter_mut().enumerate(){
                            changed|=ui.checkbox(&mut s.enabled,"").changed();ui.label(&s.name);changed|=ui.add(egui::DragValue::new(&mut s.start).range(1..=999_999)).changed();
                            ui.horizontal(|ui|{if ui.small_button("Up").clicked()&&i>0{move_sheet=Some((i,i-1));}
                            if ui.small_button("Down").clicked(){move_sheet=Some((i,i+1));}});ui.end_row();
                        }
                    });
                });
                if let Some((a,b))=move_sheet&&b<d.sheets.len(){d.sheets.swap(a,b);changed=true;}
                if changed{d.plan=None;d.validated=false;d.error.clear();}
                ui.horizontal(|ui|{
                    if ui.button("Reset All").clicked(){d.options.reset_existing=true;update=Some(Operation::ResetAll);}
                    if ui.button("Reset Duplicates").clicked(){d.options.reset_existing=false;update=Some(Operation::ResetDuplicates);}
                    if ui.button("Update Changes List").clicked(){update=Some(Operation::Update);}
                });
            }
            ui.separator();
            ui.horizontal(|ui|{ui.strong(if d.eco{"Change Component Designators"}else{"Proposed Changes"});ui.checkbox(&mut d.changed_only,"Show changes only");if let Some(p)=&d.plan{ui.label(format!("{} changes / {} components",p.changed(),p.changes.len()));}});
            egui::ScrollArea::both().id_salt("annotation_changes").max_height((ctx.screen_rect().height()-430.0).clamp(130.0,390.0)).auto_shrink([false,false]).show(ui,|ui|{
                egui::Grid::new("annotation_changes_header").min_col_width(175.0).max_col_width(175.0).show(ui,|ui|{for s in ["Schematic Document","Current Designator","New Designator","Component","Status"]{ui.strong(s);}ui.end_row();});
                if let Some(plan)=&mut d.plan {
                    let indices:Vec<_>=plan.changes.iter().enumerate().filter(|(_,r)|!d.changed_only||r.current!=r.proposed).map(|(i,_)|i).collect();
                    egui::ScrollArea::vertical().id_salt("annotation_change_rows").max_height(330.0).show_rows(ui,24.0,indices.len(),|ui,range|{
                        egui::Grid::new("annotation_rows_grid").striped(true).min_row_height(24.0).min_col_width(175.0).max_col_width(175.0).show(ui,|ui|{
                            for n in range {let r=&mut plan.changes[indices[n]];
                                ui.add(egui::Label::new(&r.sheet).truncate());ui.label(&r.current);
                                if ui.add_enabled(!d.eco&&r.eligible,egui::TextEdit::singleline(&mut r.proposed).id(egui::Id::new(("proposed_designator",r.document,r.component))).desired_width(130.0)).changed(){d.validated=false;d.error.clear();}
                                ui.add(egui::Label::new(&r.library).truncate());ui.label(if d.executed&&r.current!=r.proposed{"Done"}else if r.locked{"Locked"}else if !r.eligible{"Preserved"}else if d.validated{"Validated"}else if r.current!=r.proposed{"Change"}else{"Unchanged"});ui.end_row();
                            }
                        });
                    });
                }else{ui.label("Choose the scope and click Update Changes List.");}
            });
            if !d.error.is_empty(){ui.colored_label(Color32::from_rgb(245,160,110),&d.error);}
            ui.separator();
            ui.label("Logical schematic designators. Apply creates an Undo step on each changed sheet. Save edited schematics as PCBCraft JSON.");
            ui.horizontal(|ui|{
                if d.eco {
                    if ui.add_enabled(!d.executed,egui::Button::new("Validate Changes")).clicked(){validate=true;}
                    if ui.add_enabled(d.validated&&!d.executed,egui::Button::new("Execute Changes")).clicked(){execute=true;}
                    if ui.add_enabled(!d.executed,egui::Button::new("Back")).clicked(){d.eco=false;d.validated=false;}
                }else if ui.add_enabled(d.plan.as_ref().is_some_and(|p|p.changed()>0),egui::Button::new("Accept Changes (Create ECO)")).clicked(){d.eco=true;d.validated=false;}
                if ui.button(if d.executed{"Close"}else{"Cancel"}).clicked(){close=true;}
            });
            if d.executed{ui.label("Annotation executed. Changes are in the open documents; save the changed sheets when ready.");}
        });
        if let Some(op) = update
            && let Err(e) = d.update(&self.documents, op)
        {
            d.plan = None;
            d.error = e;
        }
        if validate {
            match d.validate(&self.documents) {
                Ok(_) => {
                    d.validated = true;
                    d.error.clear();
                }
                Err(e) => {
                    d.validated = false;
                    d.error = e;
                }
            }
        }
        if execute {
            match d.execute(&mut self.documents) {
                Ok(n) => self.message(false, format!("Annotation: updated {n} schematic sheets")),
                Err(e) => {
                    d.error = e;
                    d.validated = false;
                }
            }
        }
        if !close {
            self.annotation = Some(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_core::schematic::{Component, ComponentSettings};
    fn doc(name: &str) -> OpenDocument {
        let mut model = Document::new(DocumentKind::Schematic, name);
        model
            .schematic
            .as_mut()
            .unwrap()
            .components
            .push(Component {
                id: Uuid::new_v4(),
                settings: ComponentSettings::default(),
                source_index: u32::MAX,
                position: eda_geom::Point::mm(1, 1),
                designator: "R?".into(),
                comment: "10k".into(),
                library_reference: "R".into(),
                description: String::new(),
                part: 1,
                display_mode: 0,
                parameters: Default::default(),
            });
        let mut doc = OpenDocument::new(model, None);
        doc.saved = Some(doc.model.clone());
        doc
    }
    fn dialog(docs: &[OpenDocument]) -> Dialog {
        Dialog {
            title: "Project".into(),
            sheets: docs
                .iter()
                .map(|d| Scope {
                    session: d.session_id,
                    revision: d.history.revision,
                    name: d.model.name.clone(),
                    enabled: true,
                    start: 1,
                    selected: HashSet::new(),
                })
                .collect(),
            options: Options::default(),
            selected_only: false,
            plan: None,
            error: String::new(),
            eco: false,
            validated: false,
            executed: false,
            changed_only: false,
        }
    }
    #[test]
    fn preview_cancel_and_project_execute_have_expected_history() {
        let mut docs = vec![doc("a"), doc("b")];
        let before: Vec<_> = docs.iter().map(|d| d.model.clone()).collect();
        let mut d = dialog(&docs);
        d.update(&docs, Operation::Update).unwrap();
        assert!(docs.iter().all(|d| !d.dirty()));
        assert!(d.execute(&mut docs).is_err());
        d.validate(&docs).unwrap();
        d.validated = true;
        assert_eq!(d.execute(&mut docs).unwrap(), 2);
        assert_eq!(
            docs[0].model.schematic.as_ref().unwrap().components[0].designator,
            "R1"
        );
        assert_eq!(
            docs[1].model.schematic.as_ref().unwrap().components[0].designator,
            "R2"
        );
        assert!(d.execute(&mut docs).is_err());
        for (doc, old) in docs.iter_mut().zip(before) {
            assert_eq!(doc.history.revision, 1);
            doc.history.undo(&mut doc.model);
            assert_eq!(doc.model, old);
            assert!(!doc.dirty());
        }
    }
    #[test]
    fn stale_second_sheet_prevents_any_partial_execution() {
        let mut docs = vec![doc("a"), doc("b")];
        let mut d = dialog(&docs);
        d.update(&docs, Operation::Update).unwrap();
        d.validated = true;
        let mut next = docs[1].model.clone();
        next.name = "Changed".into();
        let second = &mut docs[1];
        second.history.commit(&mut second.model, next).unwrap();
        assert!(d.execute(&mut docs).is_err());
        assert!(!docs[0].dirty());
        assert_eq!(docs[0].history.revision, 0);
    }
}
