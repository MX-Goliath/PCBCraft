use super::*;
use eda_core::design_sync::{self as sync, Direction, Footprint, Plan};

fn reference_order(a: &str, b: &str) -> std::cmp::Ordering {
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        let x = a[i..].chars().next().unwrap();
        let y = b[j..].chars().next().unwrap();
        if x.is_ascii_digit() && y.is_ascii_digit() {
            let end_a = a[i..]
                .find(|c: char| !c.is_ascii_digit())
                .map_or(a.len(), |n| i + n);
            let end_b = b[j..]
                .find(|c: char| !c.is_ascii_digit())
                .map_or(b.len(), |n| j + n);
            let x = a[i..end_a].trim_start_matches('0');
            let y = b[j..end_b].trim_start_matches('0');
            let order = x.len().cmp(&y.len()).then_with(|| x.cmp(y));
            if !order.is_eq() {
                return order;
            }
            i = end_a;
            j = end_b;
        } else {
            let order = x.cmp(&y);
            if !order.is_eq() {
                return order;
            }
            i += x.len_utf8();
            j += y.len_utf8();
        }
    }
    (a.len() - i).cmp(&(b.len() - j)).then_with(|| a.cmp(&b))
}

fn status_icon(ui: &mut egui::Ui, result: Option<bool>) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14., 18.), egui::Sense::hover());
    let Some(ok) = result else {
        return;
    };
    let center = rect.center();
    ui.painter().circle_filled(
        center,
        5.5,
        if ok {
            Color32::from_rgb(80, 160, 105)
        } else {
            Color32::from_rgb(208, 92, 78)
        },
    );
    let stroke = egui::Stroke::new(1.2, Color32::WHITE);
    if ok {
        ui.painter().line_segment(
            [center + egui::vec2(-3., 0.), center + egui::vec2(-0.5, 2.5)],
            stroke,
        );
        ui.painter().line_segment(
            [
                center + egui::vec2(-0.5, 2.5),
                center + egui::vec2(3., -2.5),
            ],
            stroke,
        );
    } else {
        for sign in [-1., 1.] {
            ui.painter().line_segment(
                [
                    center + egui::vec2(-2.5, sign * 2.5),
                    center + egui::vec2(2.5, -sign * 2.5),
                ],
                stroke,
            );
        }
    }
}

pub(super) struct Dialog {
    options: sync::ProjectOptions,
    project: String,
    sheets: Vec<Uuid>,
    boards: Vec<Uuid>,
    selected: usize,
    direction: Direction,
    plan: Option<Plan>,
    revisions: Vec<(Uuid, u64)>,
    choose: bool,
    differences: bool,
    validated: bool,
    only_errors: bool,
    error: String,
    report: bool,
    library_footprints: Vec<Footprint>,
    library_warnings: Vec<String>,
}
impl Dialog {
    fn check_scope(&self, docs: &[OpenDocument]) -> Result<(), String> {
        for (session, rev) in &self.revisions {
            let d = docs
                .iter()
                .find(|d| d.session_id == *session)
                .ok_or("A compared document was closed")?;
            if d.history.revision != *rev
                || d.sch_editor.active()
                || d.view.sch_preview.is_some()
                || d.pcb_editor.has_pending()
                || d.model
                    .pcb
                    .as_ref()
                    .is_some_and(|b| d.pcb_editor.properties_dirty(b, d.history.revision))
            {
                return Err(format!(
                    "{} changed or has an unfinished edit; compare again",
                    d.model.name
                ));
            }
        }
        Ok(())
    }
    fn compare(&mut self, docs: &[OpenDocument]) -> Result<(), String> {
        let board = docs
            .iter()
            .find(|d| d.session_id == self.boards[self.selected])
            .ok_or("PCB document missing")?;
        let sheets: Vec<_> = self
            .sheets
            .iter()
            .map(|id| {
                docs.iter()
                    .find(|d| d.session_id == *id)
                    .map(|d| &d.model)
                    .ok_or("Schematic sheet missing")
            })
            .collect::<Result<_, _>>()?;
        self.revisions = docs
            .iter()
            .filter(|d| self.sheets.contains(&d.session_id) || d.session_id == board.session_id)
            .map(|d| (d.session_id, d.history.revision))
            .collect();
        self.check_scope(docs)?;
        board.model.pcb.as_ref().ok_or("Not a PCB")?;
        // Like native ECO, new/replaced patterns must resolve in available
        // libraries. Existing board instances do not install a missing library.
        let footprints = self.library_footprints.clone();
        self.plan = Some(sync::compare_project(
            &sheets,
            &board.model,
            self.direction,
            footprints,
            &self.options,
        )?);
        self.plan
            .as_mut()
            .unwrap()
            .restrict_native_back_annotation();
        self.validated = false;
        self.error.clear();
        Ok(())
    }
    fn validate(&mut self, docs: &[OpenDocument]) -> Result<sync::Prepared, String> {
        self.check_scope(docs)?;
        let models: Vec<_> = docs.iter().map(|d| &d.model).collect();
        self.plan
            .as_mut()
            .ok_or("Compare documents first")?
            .validate(&models)
    }
    fn execute(&mut self, docs: &mut [OpenDocument]) -> Result<usize, String> {
        if !self.validated {
            return Err("Validate Changes before Execute Changes".into());
        }
        let prepared = self.validate(docs)?;
        let mut targets = vec![];
        for next in prepared.documents {
            let n = docs
                .iter()
                .position(|d| {
                    d.model.id == next.id
                        && self.revisions.iter().any(|(id, _)| *id == d.session_id)
                })
                .ok_or("Target document missing")?;
            next.validate()?;
            targets.push((n, next));
        }
        // Every document is prepared and validated before committing any of them.
        for (n, next) in targets {
            let d = &mut docs[n];
            d.history.commit(&mut d.model, next)?;
        }
        let count = prepared.rows.len();
        let models: Vec<_> = docs.iter().map(|d| &d.model).collect();
        self.plan
            .as_mut()
            .unwrap()
            .acknowledge(&prepared.rows, &models);
        for (id, rev) in &mut self.revisions {
            if let Some(d) = docs.iter().find(|d| d.session_id == *id) {
                *rev = d.history.revision;
            }
        }
        self.validated = false;
        Ok(count)
    }
}
impl PcbCraft {
    pub(super) fn sync_project_name(&self) -> Option<String> {
        let d = self.documents.get(self.active?)?;
        let ps: Vec<_> = self
            .projects
            .iter()
            .filter(|p| {
                p.documents.iter().any(|e| {
                    d.path.as_ref() == Some(&e.path) || d.source_path.as_ref() == Some(&e.path)
                })
            })
            .collect();
        (ps.len() == 1).then(|| ps[0].name.clone())
    }
    pub(super) fn open_design_sync(&mut self, direction: Direction, differences: bool) {
        match self.sync_scope(direction, differences) {
            Ok(d) => {
                for warning in &d.library_warnings {
                    self.message(false, format!("PCB library: {warning}"));
                }
                self.design_sync = Some(d);
            }
            Err(e) => self.message(true, e),
        }
    }
    fn sync_scope(&mut self, direction: Direction, differences: bool) -> Result<Dialog, String> {
        let d = self
            .documents
            .get(self.active.ok_or("Open a project document first")?)
            .ok_or("No document")?;
        let active = d.session_id;
        let ps: Vec<_> = self
            .projects
            .iter()
            .filter(|p| {
                p.documents.iter().any(|e| {
                    d.path.as_ref() == Some(&e.path) || d.source_path.as_ref() == Some(&e.path)
                })
            })
            .collect();
        if ps.len() != 1 {
            return Err(
                "Open one PrjPcb project containing the active document before synchronization"
                    .into(),
            );
        }
        let p = ps[0].clone();
        let project = p.name.clone();
        let mut library_paths: Vec<_> = p
            .documents
            .iter()
            .filter(|e| matches!(e.kind.as_str(), "pcblib" | "intlib"))
            .map(|e| e.path.clone())
            .collect();
        library_paths.extend(
            self.settings
                .libraries
                .iter()
                .filter(|p| {
                    p.extension().is_some_and(|e| {
                        e.eq_ignore_ascii_case("PcbLib") || e.eq_ignore_ascii_case("IntLib")
                    })
                })
                .cloned(),
        );
        let mut library_footprints = vec![];
        let mut library_warnings = vec![];
        // Open authored libraries take precedence over their last saved copy.
        // This also makes unsaved libraries immediately usable by ECO.
        for d in &self.documents {
            if d.model.library.is_some() && d.model.kind == DocumentKind::Board {
                for member in eda_core::library_edit::members(&d.model) {
                    library_footprints.push(eda_core::design_sync::Footprint::from_library_board(
                        member.pcb.as_ref().unwrap().clone(),
                    )?);
                }
            }
        }
        let mut visited = std::collections::HashSet::new();
        for d in &self.documents {
            if d.model.library.is_some()
                && d.model.kind == DocumentKind::Board
                && let Some(path) = &d.path
            {
                visited.insert(path.canonicalize().unwrap_or_else(|_| path.clone()));
            }
        }
        for path in library_paths {
            let path = path.canonicalize().unwrap_or(path);
            if !visited.insert(path.clone()) {
                continue;
            }
            if let Some((_, cached)) = self
                .components_ui
                .footprint_libraries
                .iter()
                .find(|(p, _)| *p == path)
            {
                library_footprints.extend(cached.footprints.clone());
                library_warnings.extend(cached.warnings.clone());
            } else {
                match eda_io::altium_pcb::library::load(&path) {
                    Ok(library) => {
                        library_footprints.extend(library.footprints);
                        library_warnings.extend(library.warnings);
                    }
                    Err(e) => library_warnings.push(format!("{}: {e}", path.display())),
                }
            }
        }
        let paths: Vec<_> = p
            .documents
            .iter()
            .filter(|e| matches!(e.kind.as_str(), "schdoc" | "pcbdoc"))
            .map(|e| (e.path.clone(), e.kind.clone()))
            .collect();
        let mut sheets = vec![];
        let mut boards = vec![];
        for (path, kind) in paths {
            let path = path.canonicalize().unwrap_or(path);
            self.open_path(path.clone());
            let loaded = self
                .documents
                .iter()
                .find(|d| d.path.as_ref() == Some(&path) || d.source_path.as_ref() == Some(&path))
                .map(|d| d.session_id);
            self.active = self.documents.iter().position(|d| d.session_id == active);
            let id = loaded.ok_or_else(|| {
                format!(
                    "Could not load {}. No partial project ECO was created",
                    path.display()
                )
            })?;
            let list = if kind == "schdoc" {
                &mut sheets
            } else {
                &mut boards
            };
            if !list.contains(&id) {
                list.push(id);
            }
        }
        if boards.is_empty() || sheets.is_empty() {
            return Err("The project needs a schematic and a PCB document".into());
        }
        let selected = boards.iter().position(|id| *id == active).unwrap_or(0);
        let choose = !boards.contains(&active) || differences;
        let compile_sheets: Vec<_> = self
            .documents
            .iter()
            .filter(|d| sheets.contains(&d.session_id))
            .map(|d| {
                (
                    d.source_path
                        .as_ref()
                        .or(d.path.as_ref())
                        .unwrap()
                        .as_path(),
                    &d.model,
                )
            })
            .collect();
        let options = p.compile_options(&compile_sheets)?;
        let mut dialog = Dialog {
            options,
            project,
            sheets,
            boards,
            selected,
            direction,
            plan: None,
            revisions: vec![],
            choose,
            differences,
            validated: false,
            only_errors: false,
            error: String::new(),
            report: false,
            library_footprints,
            library_warnings,
        };
        if !choose {
            dialog.compare(&self.documents)?;
        }
        Ok(dialog)
    }
    pub(super) fn design_sync_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.design_sync.take() else {
            return;
        };
        let mut close = false;
        let mut compare = false;
        let mut validate = false;
        let mut execute = false;
        egui::Modal::new(egui::Id::new("design_sync_eco")).backdrop_color(Color32::TRANSPARENT).frame(egui::Frame::window(&ctx.style()).corner_radius(0)).show(ctx,|ui|{
            ui.set_width((ctx.screen_rect().width()-48.).clamp(480.,if d.choose{540.}else{1065.}));
            ui.horizontal(|ui|{ui.strong(if d.choose{"Choose Documents To Compare"}else if d.differences{"Differences between"}else{"Engineering Change Order"});ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.small_button("×").clicked(){close=true;}});});ui.separator();
            if d.choose {
                ui.label(&d.project);ui.label("Compare the schematic project with:");
                egui::ComboBox::from_id_salt("eco_target_pcb").selected_text(self.documents.iter().find(|v|v.session_id==d.boards[d.selected]).map_or("PCB document",|v|v.model.name.as_str())).show_ui(ui,|ui|{
                    for (n,id) in d.boards.iter().enumerate(){if let Some(doc)=self.documents.iter().find(|v|v.session_id==*id){ui.selectable_value(&mut d.selected,n,&doc.model.name);}}
                });
                ui.horizontal(|ui|{ui.radio_value(&mut d.direction,Direction::ToPcb,"Update PCB");ui.radio_value(&mut d.direction,Direction::ToSchematic,"Update Schematics");});
                if ui.button("OK").clicked(){compare=true;}
                if ui.button("Cancel").clicked(){close=true;}
            } else {
                ui.spacing_mut().item_spacing=egui::vec2(3.,0.);
                ui.spacing_mut().interact_size.y=18.;
                let widths=[80.,185.,270.,207.,38.,40.,180.];
                let row=|ui:&mut egui::Ui,texts:[&str;7]|{
                    ui.horizontal(|ui|{for (text,width) in texts.into_iter().zip(widths){ui.allocate_ui_with_layout(egui::vec2(width,18.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(width,18.));ui.add(egui::Label::new(text).truncate());});}});
                };
                ui.horizontal(|ui|{ui.allocate_ui_with_layout(egui::vec2(751.,18.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(751.,18.));ui.label("Modifications");});ui.label("Status");});
                row(ui,["Enable","Action","Affected Object","Affected Document","Check","Done","Message"]);
                let mut changed=false;
                egui::Frame::new().stroke(egui::Stroke::new(1.,Color32::from_gray(30))).inner_margin(4.).show(ui,|ui|{
                    egui::ScrollArea::both().id_salt("eco_rows").max_height((ctx.screen_rect().height()-240.).clamp(160.,365.)).min_scrolled_height(320.).auto_shrink([false,false]).show(ui,|ui|{
                        if let Some(p)=&mut d.plan {
                            let mut groups=vec![];for c in &p.changes{if !groups.contains(&c.group){groups.push(c.group);}}
                            if p.changes.is_empty(){ui.label("No differences detected.");}
                            for group in groups {
                                let count=p.changes.iter().filter(|c|c.group==group).count();
                                egui::CollapsingHeader::new(format!("{group} ({count})")).id_salt(("eco_group",group)).default_open(true).show(ui,|ui|{
                                    let mut rows:Vec<_>=p.changes.iter_mut().filter(|c|c.group==group && (!d.only_errors||c.check.as_ref().is_some_and(Result::is_err)||!c.supported())).collect();
                                    rows.sort_by(|a,b|reference_order(&a.object,&b.object));
                                    for c in rows {
                                        ui.horizontal(|ui|{
                                            ui.allocate_ui_with_layout(egui::vec2(widths[0]-18.,18.),egui::Layout::right_to_left(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(widths[0]-18.,18.));changed|=ui.add_enabled(!c.done&&c.supported(),egui::Checkbox::without_text(&mut c.enabled)).changed();});
                                            for (text,width) in [(c.action,widths[1]),(c.object.as_str(),widths[2]),(c.document.as_str(),widths[3])]{ui.allocate_ui_with_layout(egui::vec2(width,18.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(width,18.));ui.add(egui::Label::new(text).truncate()).on_hover_text(text);});}
                                            for (result,width) in [(c.check.as_ref().map(Result::is_ok),widths[4]),(c.done.then_some(true),widths[5])]{ui.allocate_ui_with_layout(egui::vec2(width,18.),egui::Layout::left_to_right(egui::Align::Center),|ui|{ui.set_min_size(egui::vec2(width,18.));status_icon(ui,result);});}
                                            let message=c.check.as_ref().and_then(|s|s.as_ref().err()).map(String::as_str).or_else(||c.unavailable_reason()).unwrap_or("");
                                            ui.add(egui::Label::new(message).truncate()).on_hover_text(message);
                                        });
                                    }
                                });
                            }
                            if changed{p.reset_validation();d.validated=false;d.error.clear();}
                        }
                    });
                });
                ui.horizontal(|ui|{
                    if d.plan.as_ref().is_some_and(|p|p.changes.iter().any(|c|!c.done)) {
                        for (label,enabled) in [("Enable All",true),("Disable All",false)]{if ui.small_button(label).clicked(){if let Some(p)=&mut d.plan{for c in &mut p.changes{if !c.done{c.enabled=enabled&&c.supported();}}p.reset_validation();}d.validated=false;}}
                    }
                });
                if let Some(p)=&d.plan && !p.warnings.is_empty() {
                    egui::CollapsingHeader::new(format!("Compilation diagnostics ({})",p.warnings.len())).id_salt("eco_diagnostics").show(ui,|ui| {
                        egui::ScrollArea::vertical().id_salt("eco_diagnostics_rows").max_height(90.0).show(ui,|ui| {
                            for w in &p.warnings { ui.colored_label(Color32::from_rgb(245,170,110),w); }
                        });
                    });
                }
                ui.separator();
                ui.horizontal(|ui|{
                    if d.differences {if ui.button("Create Engineering Change Order").clicked(){d.differences=false;}}
                    else {
                        if ui.add_enabled(d.plan.as_ref().is_some_and(|p|p.changes.iter().any(|c|c.enabled&&!c.done)),egui::Button::new("Validate Changes")).clicked(){validate=true;}
                        if ui.add_enabled(d.validated,egui::Button::new("Execute Changes")).clicked(){execute=true;}
                    }
                    if ui.button("Report Changes…").clicked(){d.report=true;}
                    ui.checkbox(&mut d.only_errors,"Only Show Errors");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{if ui.button("Close").clicked(){close=true;}});
                });
            }
            if !d.error.is_empty(){ui.colored_label(Color32::from_rgb(245,170,110),&d.error);}
        });
        if compare {
            match d.compare(&self.documents) {
                Ok(()) => d.choose = false,
                Err(e) => d.error = e,
            }
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
                Ok(count) => {
                    self.message(false,format!("Engineering Change Order: {count} changes executed. Undo is available in each changed document."));
                }
                Err(e) => {
                    d.validated = false;
                    d.error = e;
                }
            }
        }
        if d.report {
            egui::Modal::new(egui::Id::new("design_sync_report")).show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() - 80.).clamp(480., 1100.));
                ui.horizontal(|ui| {
                    ui.strong("Engineering Change Order Report");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            d.report = false;
                        }
                    });
                });
                let mut report = d.plan.as_ref().map(Plan::report).unwrap_or_default();
                if ui.button("Copy Report").clicked() {
                    ui.ctx().copy_text(report.clone());
                }
                egui::ScrollArea::both().max_height(430.).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut report)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(1400.)
                            .interactive(false),
                    );
                });
            });
        }
        if !close {
            self.design_sync = Some(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_scope_accepts_hierarchy_and_commits_only_the_selected_board_with_undo() {
        use eda_io::altium::{Project, ProjectEntry};
        let (_, mut docs) = dialog();
        let base = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let child_path = base.join("Circuit.SchDoc");
        let board_path = base.join("Board.PcbDoc");
        let main_path = base.join("Main.SchDoc");
        docs[0].model.name = "Circuit.SchDoc".into();
        docs[0].source_path = Some(child_path.clone());
        docs[0].path = Some(child_path.clone());
        docs[1].source_path = Some(board_path.clone());
        docs[1].path = Some(board_path.clone());
        let mut main = Document::new(eda_core::DocumentKind::Schematic, "Main.SchDoc");
        main.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(eda_core::schematic_edit::primitive(
                eda_core::schematic::SchPrimitiveKind::SheetSymbol {
                    a: Point::mm(0, 10),
                    b: Point::mm(10, 0),
                    filename: "Circuit.SchDoc".into(),
                    designator: "U_Child".into(),
                },
                None,
            ));
        docs.push(OpenDocument::new(main, Some(main_path.clone())));
        let mut app = super::super::schematic_editor::tests::app();
        app.documents = docs;
        app.active = Some(3);
        app.projects = vec![Project {
            path: base.join("Test.PrjPcb"),
            name: "Test.PrjPcb".into(),
            documents: [
                (&main_path, "schdoc"),
                (&child_path, "schdoc"),
                (&board_path, "pcbdoc"),
            ]
            .into_iter()
            .map(|(p, k)| ProjectEntry {
                path: p.clone(),
                original_path: p.file_name().unwrap().to_string_lossy().into(),
                kind: k.into(),
                exists: true,
            })
            .collect(),
            generated_documents: vec![],
            variants: vec![],
            virtual_bom: false,
            sheet_children: std::collections::BTreeMap::from([(main_path, vec![child_path])]),
            sheet_numbers: Default::default(),
            netlist_options: Default::default(),
            validation_options: Default::default(),
            warnings: vec![],
        }];
        let source = app.documents[0].model.clone();
        let original = app.documents[1].model.clone();
        let mut dialog = app.sync_scope(Direction::ToPcb, false).unwrap();
        assert_eq!(dialog.sheets.len(), 2);
        dialog.compare(&app.documents).unwrap();
        dialog.validate(&app.documents).unwrap();
        dialog.validated = true;
        assert!(dialog.execute(&mut app.documents).unwrap() > 0);
        assert_eq!(app.documents[0].model, source);
        assert_ne!(app.documents[1].model, original);
        let pcb = &mut app.documents[1];
        pcb.history.undo(&mut pcb.model);
        assert_eq!(pcb.model, original);
    }
    #[test]
    fn eco_rows_follow_native_reference_order_without_integer_overflow() {
        let mut references = vec![
            "WA1", "TP27", "C9", "TP18", "TP9", "BQ1", "L5", "C6", "JVD1", "TP17", "L4", "C8",
        ];
        references.sort_by(|a, b| reference_order(a, b));
        assert_eq!(
            references,
            [
                "BQ1", "C6", "C8", "C9", "JVD1", "L4", "L5", "TP9", "TP17", "TP18", "TP27", "WA1"
            ]
        );
        assert!(
            reference_order(
                "R999999999999999999999999999999999999999",
                "R1000000000000000000000000000000000000000"
            )
            .is_lt()
        );
    }
    use eda_core::{
        pcb,
        schematic_edit::{self as edit, Placement},
    };
    use eda_geom::Point;

    fn dialog() -> (Dialog, Vec<OpenDocument>) {
        let s = Document::new(DocumentKind::Schematic, "Sheet");
        let f = edit::place(Placement::Resistor, Point::mm(10, 10), 0, "R1", "10k", 0).unwrap();
        let s = edit::preview(&s, &edit::Edit::Insert(f)).unwrap();
        let symbol = &s.schematic.as_ref().unwrap().components[0];
        let mut b = pcb::Board::default();
        b.components.push(pcb::Component {
            id: Uuid::new_v4(),
            index: 0,
            designator: "R1".into(),
            comment: "old".into(),
            footprint: String::new(),
            parameters: Some(Default::default()),
            position: Point::mm(1, 1),
            rotation: 0.,
            layer: pcb::TOP,
            locked: false,
            properties: std::collections::BTreeMap::from([
                ("SOURCEUNIQUEID".into(), symbol.id.to_string()),
                ("SOURCEDESCRIPTION".into(), symbol.description.clone()),
            ]),
        });
        for name in ["1", "2"] {
            let mut p = pcb::Item::new(
                pcb::TOP,
                None,
                pcb::Shape::Pad {
                    position: Point::mm(1, 1),
                    size: Point::mm(1, 1),
                    rotation: 0.,
                    shape: pcb::PadShape::Rectangle,
                    hole: 0,
                    plated: true,
                    name: name.into(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            p.component = Some(0);
            b.items.push(p);
        }
        let mut board = Document::new(DocumentKind::Board, "Board");
        board.pcb = Some(b);
        let mut docs = vec![
            OpenDocument::new(s, None),
            OpenDocument::new(board, None),
            OpenDocument::new(Document::example(), None),
        ];
        for d in &mut docs {
            d.mark_saved();
        }
        let d = Dialog {
            options: Default::default(),
            project: "Test".into(),
            sheets: vec![docs[0].session_id],
            boards: vec![docs[1].session_id],
            selected: 0,
            direction: Direction::ToPcb,
            plan: None,
            revisions: vec![],
            choose: false,
            differences: false,
            validated: false,
            only_errors: false,
            error: String::new(),
            report: false,
            library_footprints: vec![],
            library_warnings: vec![],
        };
        (d, docs)
    }
    #[test]
    fn eco_requires_available_library_geometry_instead_of_reusing_board_instances() {
        let (mut dialog, mut docs) = dialog();
        let fragment = eda_core::schematic_edit::place(
            eda_core::schematic_edit::Placement::Resistor,
            Point::mm(50, 50),
            0,
            "R2",
            "10k",
            0,
        )
        .unwrap();
        docs[0].model = eda_core::schematic_edit::preview(
            &docs[0].model,
            &eda_core::schematic_edit::Edit::Insert(fragment),
        )
        .unwrap();
        let s = docs[0].model.schematic.as_mut().unwrap();
        s.components[1].settings.models = s.components[0].settings.models.clone();
        dialog.compare(&docs).unwrap();
        dialog.validate(&docs).unwrap();
        assert!(dialog.plan.as_ref().unwrap().changes.iter().any(|c| {
            c.object.contains("R2")
                && c.check
                    .as_ref()
                    .is_some_and(|r| r.as_ref().is_err_and(|e| e.contains("Footprint Not Found")))
        }));
        let b = docs[1].model.pcb.as_ref().unwrap();
        dialog
            .library_footprints
            .push(Footprint::from_board(b, b.components[0].id).unwrap());
        dialog.compare(&docs).unwrap();
        let prepared = dialog.validate(&docs).unwrap();
        assert!(
            prepared
                .documents
                .iter()
                .filter_map(|d| d.pcb.as_ref())
                .any(|b| b.components.iter().any(|c| c.designator == "R2"))
        );
    }
    #[test]
    fn eco_execution_requires_validation_commits_only_scoped_documents_and_undo_invalidates_it() {
        let (mut d, mut docs) = dialog();
        d.compare(&docs).unwrap();
        assert!(d.execute(&mut docs).is_err());
        let unchanged = docs[2].model.clone();
        let original = docs[1].model.clone();
        let prepared = d.validate(&docs).unwrap();
        assert_eq!(prepared.rows.len(), 1);
        d.validated = true;
        assert_eq!(d.execute(&mut docs).unwrap(), 1);
        assert!(docs[1].dirty());
        assert_eq!(docs[0].history.revision, 0);
        assert_eq!(docs[1].history.revision, 1);
        assert_eq!(docs[2].model, unchanged);
        let p = d.plan.as_ref().unwrap();
        assert!(p.changes.iter().all(|c| c.done));
        assert!(d.validate(&docs).unwrap().rows.is_empty());
        let target = &mut docs[1];
        target.history.undo(&mut target.model);
        assert_eq!(target.model, original);
        assert!(!target.dirty());
        assert!(d.validate(&docs).is_err());
    }
    #[test]
    fn closing_a_compared_sheet_or_mutating_its_revision_rejects_the_eco() {
        let (mut d, mut docs) = dialog();
        d.compare(&docs).unwrap();
        docs[0].history.revision += 1;
        assert!(d.validate(&docs).is_err());
        docs.remove(0);
        assert!(d.validate(&docs).is_err());
        assert_eq!(docs[0].history.revision, 0);
    }
}
