//! Project Variant Management and read-only schematic assembly views.
#[path = "variants_manager.rs"]
mod manager;
#[cfg(test)]
#[path = "variants_tests.rs"]
mod tests;
use super::*;
use eda_core::assembly_variants::{self as model, Variant};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Default)]
pub(super) struct State {
    pub(super) request_project: Option<PathBuf>,
    pub dialog: Option<Dialog>,
    saved: BTreeMap<PathBuf, (Vec<Variant>, Vec<u8>)>,
    alternate_files: BTreeMap<PathBuf, Option<Vec<u8>>>,
    pub(super) cache: Option<(String, Arc<Document>)>,
    catalog: std::cell::RefCell<Option<(String, Arc<Vec<Row>>)>>,
    histories: BTreeMap<PathBuf, model::History>,
}
#[derive(Clone)]
pub(super) struct Row {
    document: Arc<Document>,
    component: Uuid,
    uid: String,
    path: String,
    designator: String,
}
pub(super) struct Dialog {
    project: PathBuf,
    original: Vec<Variant>,
    draft: Vec<Variant>,
    history: model::History,
    rows: Arc<Vec<Row>>,
    row: usize,
    selected: BTreeSet<usize>,
    variant: usize,
    filter: String,
    only_varied: bool,
    parameter: String,
    value: String,
    editor: Option<manager::VariantEditor>,
    component_kind: Option<u32>,
    style: model::DisplayStyle,
    style_editor: Option<model::DisplayStyle>,
    fit_width: bool,
    links: bool,
    report: Option<String>,
    library: Option<eda_core::library::Library>,
    error: String,
}
impl State {
    pub fn project_updated(&mut self, p: &eda_io::altium::Project) {
        if let Some((saved, bytes)) = self.saved.get_mut(&p.path)
            && *saved == p.variants
            && let Ok(next) = std::fs::read(&p.path)
        {
            *bytes = next;
        }
        self.catalog.borrow_mut().take();
        self.cache = None;
    }
    pub fn dirty(&self, p: &eda_io::altium::Project) -> bool {
        self.saved
            .get(&p.path)
            .is_some_and(|(v, _)| *v != p.variants)
    }
}
fn natural(s: &str) -> (String, u64, String) {
    let start = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
    let end = s[start..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(s.len(), |n| n + start);
    (
        s[..start].to_lowercase(),
        s[start..end].parse().unwrap_or(0),
        s[end..].to_lowercase(),
    )
}
impl PcbCraft {
    fn apply_variants(&mut self, index: usize, next: Vec<Variant>) -> Result<(), String> {
        let p = &mut self.projects[index];
        if !self.variants.saved.contains_key(&p.path) {
            let bytes = std::fs::read(&p.path).map_err(|e| e.to_string())?;
            self.variants
                .saved
                .insert(p.path.clone(), (p.variants.clone(), bytes));
            self.variants.alternate_files.insert(
                p.path.clone(),
                std::fs::read(p.path.with_extension("PrjPcbVariants")).ok(),
            );
        }
        self.variants
            .histories
            .entry(p.path.clone())
            .or_default()
            .commit(&mut p.variants, next)?;
        if self
            .settings
            .active_variants
            .get(&p.path)
            .and_then(|id| id.as_ref())
            .is_some_and(|id| !p.variants.iter().any(|v| &v.id == id))
        {
            self.settings.active_variants.insert(p.path.clone(), None);
        }
        self.variants.cache = None;
        Ok(())
    }
    pub(super) fn variant_undo(&mut self, redo: bool) -> bool {
        if self
            .active
            .is_none_or(|i| self.documents[i].sch_editor.compiled_path.is_none())
        {
            return false;
        }
        if let Some(i) = self.file_project_index() {
            let p = &mut self.projects[i];
            if let Some(h) = self.variants.histories.get_mut(&p.path) {
                if redo {
                    h.redo(&mut p.variants)
                } else {
                    h.undo(&mut p.variants)
                }
            }
            if self
                .settings
                .active_variants
                .get(&p.path)
                .and_then(|v| v.as_ref())
                .is_some_and(|id| !p.variants.iter().any(|v| &v.id == id))
            {
                self.settings.active_variants.insert(p.path.clone(), None);
            }
        }
        self.variants.cache = None;
        true
    }
    pub(super) fn variant_active_bar(
        &mut self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        index: usize,
    ) {
        let Some(i) = self.file_project_index() else {
            return;
        };
        let project = self.projects[i].path.clone();
        let mut selected = self
            .settings
            .active_variants
            .get(&project)
            .cloned()
            .flatten();
        let old = selected.clone();
        let choices = self.projects[i]
            .variants
            .iter()
            .map(|v| (v.id.clone(), v.name.clone()))
            .collect::<Vec<_>>();
        let mut toggle = false;
        egui::Area::new(egui::Id::new("variant_active_bar"))
            .fixed_pos(egui::pos2(canvas.center().x - 210., canvas.top() + 5.))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(Color32::from_gray(73))
                    .inner_margin(5)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("current_assembly_variant")
                                .selected_text(
                                    choices
                                        .iter()
                                        .find(|(id, _)| Some(id) == selected.as_ref())
                                        .map_or("[No Variations]", |(_, n)| n.as_str()),
                                )
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut selected, None, "[No Variations]");
                                    for (id, name) in &choices {
                                        ui.selectable_value(&mut selected, Some(id.clone()), name);
                                    }
                                });
                            if ui.button("Variant Management…").clicked() {
                                self.command = Some(Command::Variants);
                            }
                            toggle = ui
                                .add_enabled(
                                    selected.is_some()
                                        && self.documents[index].view.selected.is_some(),
                                    egui::Button::new("Toggle Fitted"),
                                )
                                .clicked();
                        });
                    });
            });
        if selected != old {
            self.select_variant(project.clone(), selected.clone());
        }
        if toggle && let Some(id) = selected {
            let d = &self.documents[index];
            let s = d.model.schematic.as_ref().unwrap();
            let hit = d.view.selected;
            let owner = hit.and_then(|id| s.primitive(id).and_then(|p| p.component).or(Some(id)));
            if let Some(c) = s.components.iter().find(|c| Some(c.id) == owner) {
                let uid = model::component_uid(
                    s,
                    c,
                    d.sch_editor.compiled_path.as_deref().unwrap_or_default(),
                );
                let mut next = self.projects[i].variants.clone();
                if let Some(v) = next.iter_mut().find(|v| v.id == id) {
                    let change = v.variation_mut(&uid, &c.designator);
                    change.kind = if change.kind == 1 { 0 } else { 1 };
                }
                if let Err(e) = self.apply_variants(i, next) {
                    self.message(true, e);
                }
            }
        }
    }
    pub(super) fn variants_dirty(&self) -> bool {
        self.projects.iter().any(|p| self.variants.dirty(p))
    }
    pub(super) fn variant_output_snapshots(
        &self,
        snapshots: &mut [(PathBuf, Document)],
        index: usize,
    ) -> Result<(), String> {
        if self.documents[index].sch_editor.compiled_path.is_none() {
            return Ok(());
        }
        let Some(p) = self.file_project_index() else {
            return Ok(());
        };
        let project = &self.projects[p];
        let selected = self
            .settings
            .active_variants
            .get(&project.path)
            .and_then(|v| v.as_ref());
        let variant = selected.and_then(|id| project.variants.iter().find(|v| &v.id == id));
        let rows = self.variant_rows(p)?;
        if let Some(v) = variant {
            let refs = rows
                .iter()
                .map(|r| model::ComponentRef {
                    uid: r.uid.clone(),
                    designator: r.designator.clone(),
                })
                .collect::<Vec<_>>();
            if !model::unresolved(v, &refs).is_empty() {
                return Err(
                    "Resolve variant component links before generating project output".into(),
                );
            }
        }
        for (_, d) in snapshots {
            let mut paths = rows
                .iter()
                .filter(|r| r.document.id == d.id)
                .map(|r| r.path.clone())
                .collect::<Vec<_>>();
            paths.sort();
            paths.dedup();
            if paths.len() > 1 {
                return Err(
                    "Project PDF for repeated physical sheets requires explicit channel pagination"
                        .into(),
                );
            }
            *d = model::project(
                d,
                variant,
                paths.first().map_or("", String::as_str),
                &self.settings.variant_style,
            )?;
        }
        Ok(())
    }
    pub(super) fn save_variants(&mut self) -> bool {
        for p in &self.projects {
            if !self.variants.dirty(p) {
                continue;
            }
            let expected = &self.variants.saved[&p.path].1;
            let expected_library = self
                .variants
                .alternate_files
                .get(&p.path)
                .and_then(|b| b.as_deref());
            match eda_io::project_variants::save_with_alternates(p, expected, expected_library) {
                Ok(bytes) => {
                    self.variants
                        .saved
                        .insert(p.path.clone(), (p.variants.clone(), bytes));
                    self.variants.alternate_files.insert(
                        p.path.clone(),
                        std::fs::read(p.path.with_extension("PrjPcbVariants")).ok(),
                    );
                }
                Err(e) => {
                    self.message(true, e);
                    return false;
                }
            }
        }
        true
    }
    pub(super) fn variant_rows(&self, index: usize) -> Result<Arc<Vec<Row>>, String> {
        let key = format!(
            "{}:{:?}",
            self.projects[index].path.display(),
            self.documents
                .iter()
                .map(|d| (d.model.id, d.history.revision))
                .collect::<Vec<_>>()
        );
        if let Some((old, rows)) = &*self.variants.catalog.borrow()
            && old == &key
        {
            return Ok(rows.clone());
        }
        let p = &self.projects[index];
        let mut sheets = vec![];
        for e in p.documents.iter().filter(|e| e.kind == "schdoc") {
            let d = if let Some(d) = self.documents.iter().find(|d| {
                d.path.as_ref() == Some(&e.path) || d.source_path.as_ref() == Some(&e.path)
            }) {
                if d.sch_editor.active() {
                    return Err(format!("Finish the edit on {} first", d.model.name));
                }
                d.model.clone()
            } else {
                eda_io::altium::load_schematic(&e.path)?
            };
            sheets.push((e.path.clone(), d));
        }
        let refs = sheets
            .iter()
            .map(|(p, d)| (p.as_path(), d))
            .collect::<Vec<_>>();
        let options = p.compile_options(&refs)?;
        let links = options.links.unwrap_or_default();
        fn paths(
            id: Uuid,
            sheets: &[(PathBuf, Document)],
            links: &[eda_core::design_sync::SheetLink],
            seen: &mut Vec<Uuid>,
        ) -> Result<Vec<String>, String> {
            if seen.contains(&id) || seen.len() > 64 {
                return Err("Cyclic or excessive project hierarchy".into());
            }
            seen.push(id);
            let parents = links.iter().filter(|l| l.child == id).collect::<Vec<_>>();
            let mut result = vec![];
            if parents.is_empty() {
                result.push(String::new());
            }
            for l in parents {
                let d = &sheets
                    .iter()
                    .find(|(_, d)| d.id == l.parent)
                    .ok_or("Missing parent sheet")?
                    .1;
                let s = d.schematic.as_ref().unwrap();
                let symbol = s.primitive(l.symbol).ok_or("Missing Sheet Symbol")?;
                let uid = s
                    .records
                    .iter()
                    .find(|r| r.index == symbol.source_index)
                    .and_then(|r| r.fields.get("UNIQUEID"))
                    .cloned()
                    .unwrap_or_else(|| symbol.id.to_string());
                for path in paths(l.parent, sheets, links, seen)? {
                    result.push(format!("{path}\\{uid}"));
                }
                if result.len() > 1024 {
                    return Err("Too many repeated sheet instances".into());
                }
            }
            seen.pop();
            Ok(result)
        }
        let mut rows = vec![];
        for (_, d) in &sheets {
            let document = Arc::new(d.clone());
            for path in paths(d.id, &sheets, &links, &mut vec![])? {
                let s = d.schematic.as_ref().unwrap();
                for c in &s.components {
                    rows.push(Row {
                        document: document.clone(),
                        component: c.id,
                        uid: model::component_uid(s, c, &path),
                        path: path.clone(),
                        designator: c.designator.clone(),
                    });
                }
            }
        }
        rows.sort_by_key(|r| (natural(&r.designator), r.path.clone()));
        let rows = Arc::new(rows);
        *self.variants.catalog.borrow_mut() = Some((key, rows.clone()));
        Ok(rows)
    }
    pub(super) fn open_variants(&mut self) {
        let target = self.variants.request_project.take();
        let Some(i) = target
            .and_then(|p| self.projects.iter().position(|v| v.path == p))
            .or_else(|| self.file_project_index())
        else {
            return;
        };
        let p = &self.projects[i];
        let path = p.path.clone();
        self.variants.catalog.borrow_mut().take();
        let rows = match self.variant_rows(i) {
            Ok(r) => r,
            Err(e) => {
                self.message(true, e);
                return;
            }
        };
        if !self.variants.saved.contains_key(&path) {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    self.variants
                        .saved
                        .insert(path.clone(), (p.variants.clone(), bytes));
                    self.variants.alternate_files.insert(
                        path.clone(),
                        std::fs::read(path.with_extension("PrjPcbVariants")).ok(),
                    );
                }
                Err(e) => {
                    self.message(true, e.to_string());
                    return;
                }
            }
        }
        self.variants.dialog = Some(Dialog {
            project: path,
            original: p.variants.clone(),
            draft: p.variants.clone(),
            history: Default::default(),
            rows,
            row: 0,
            selected: BTreeSet::from([0]),
            variant: 0,
            filter: String::new(),
            only_varied: false,
            parameter: String::new(),
            value: String::new(),
            editor: None,
            component_kind: None,
            style: self.settings.variant_style.clone(),
            style_editor: None,
            fit_width: true,
            links: false,
            report: None,
            library: None,
            error: String::new(),
        });
        if let Some(d) = &mut self.variants.dialog {
            let refs = d
                .rows
                .iter()
                .map(|r| model::ComponentRef {
                    uid: r.uid.clone(),
                    designator: r.designator.clone(),
                })
                .collect::<Vec<_>>();
            let count = d
                .draft
                .iter()
                .map(|v| model::unresolved(v, &refs).len())
                .sum::<usize>();
            if count > 0 {
                d.error = format!(
                    "{count} unresolved component link(s). Use Component Links to review and relink them."
                );
            }
        }
    }
    pub(super) fn select_variant(&mut self, project: PathBuf, id: Option<String>) {
        self.settings.active_variants.insert(project.clone(), id);
        self.file_state.selected_project = Some(project.clone());
        if let Some(p) = self.projects.iter().find(|p| p.path == project)
            && let Some(e) = p.documents.iter().find(|e| e.kind == "schdoc")
        {
            let same = self.active.is_some_and(|i| {
                p.documents.iter().any(|e| {
                    self.documents[i].path.as_ref() == Some(&e.path)
                        || self.documents[i].source_path.as_ref() == Some(&e.path)
                })
            });
            if !same {
                let path = e.path.clone();
                self.request_open_path(path);
            }
        }
        self.variants.cache = None;
    }
    pub(super) fn variant_projection(&mut self, index: usize) -> Option<Arc<Document>> {
        let path = self.documents[index].sch_editor.compiled_path.clone()?;
        let p = self.file_project_index()?;
        let project = &self.projects[p];
        let selected = self
            .settings
            .active_variants
            .get(&project.path)
            .and_then(|id| id.as_ref());
        let variant = selected.and_then(|id| project.variants.iter().find(|v| &v.id == id));
        if let Some(v) = variant {
            let result=self.variant_rows(p).and_then(|rows| {
                let refs=rows.iter().map(|r|model::ComponentRef{uid:r.uid.clone(),designator:r.designator.clone()}).collect::<Vec<_>>();
                if model::unresolved(v,&refs).is_empty(){Ok(())}else{Err("Resolve variant component links in Variant Management before viewing this assembly".into())}
            });
            if let Err(e) = result {
                self.documents[index].sch_editor.compiled_path = None;
                self.message(true, e);
                return None;
            }
        }
        let doc = &self.documents[index];
        let key = format!(
            "{}:{}:{}:{}:{}:{}",
            doc.session_id,
            doc.history.revision,
            path,
            variant.map_or("", |v| v.id.as_str()),
            self.variants
                .histories
                .get(&project.path)
                .map_or(0, |h| h.revision),
            serde_json::to_string(&self.settings.variant_style).unwrap_or_default()
        );
        if let Some((old, d)) = &self.variants.cache
            && old == &key
        {
            return Some(d.clone());
        }
        match model::project(&doc.model, variant, &path, &self.settings.variant_style) {
            Ok(d) => {
                let d = Arc::new(d);
                self.variants.cache = Some((key, d.clone()));
                Some(d)
            }
            Err(e) => {
                self.documents[index].sch_editor.compiled_path = None;
                self.message(true, format!("Cannot display variant: {e}"));
                None
            }
        }
    }
    pub(super) fn variant_paths(&self, index: usize) -> Vec<String> {
        let Some(p) = self.file_project_index() else {
            return vec![];
        };
        match self.variant_rows(p) {
            Ok(rows) => {
                let mut paths = rows
                    .iter()
                    .filter(|r| r.document.id == self.documents[index].model.id)
                    .map(|r| r.path.clone())
                    .collect::<Vec<_>>();
                paths.sort();
                paths.dedup();
                if paths.is_empty() {
                    paths.push(String::new());
                }
                paths
            }
            Err(_) => vec![String::new()],
        }
    }
}
