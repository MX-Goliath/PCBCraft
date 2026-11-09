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
    pick_mode: Option<u8>,
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
    link_notice: Option<String>,
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
fn variant_glyph(p: &egui::Painter, r: egui::Rect, n: usize, enabled: bool) {
    let ink = |c: Color32| if enabled { c } else { Color32::from_gray(95) };
    let at = |x: f32, y: f32| r.min + egui::vec2(x, y);
    if n == 3 {
        for x in [4., 13.] {
            p.rect_filled(
                egui::Rect::from_min_max(at(x, 7.), at(x + 5., 16.)),
                0.,
                ink(Color32::from_rgb(154, 184, 139)),
            );
            for y in [5., 18.] {
                p.line_segment(
                    [at(x + 2.5, y), at(x + 2.5, y + 3.)],
                    egui::Stroke::new(1., ink(Color32::from_rgb(217, 189, 104))),
                );
            }
        }
        p.line_segment(
            [at(7., 3.), at(15., 3.)],
            egui::Stroke::new(1., ink(Color32::from_rgb(101, 149, 205))),
        );
    } else {
        p.rect_filled(
            egui::Rect::from_min_max(at(4., 5.), at(18., 17.)),
            1.,
            ink(Color32::from_rgb(68, 135, 114)),
        );
        for x in [7., 11., 15.] {
            for y in [2., 18.] {
                p.line_segment(
                    [at(x, y), at(x, y + 3.)],
                    egui::Stroke::new(1., ink(Color32::from_rgb(184, 213, 245))),
                );
            }
        }
        if n == 2 {
            let red = egui::Stroke::new(2., ink(Color32::from_rgb(238, 84, 84)));
            p.line_segment([at(2., 1.), at(20., 21.)], red);
            p.line_segment([at(2., 21.), at(20., 1.)], red);
        } else {
            p.rect_stroke(
                egui::Rect::from_min_max(at(7., 8.), at(15., 14.)),
                0.,
                egui::Stroke::new(1., ink(Color32::from_rgb(184, 213, 245))),
                egui::StrokeKind::Inside,
            );
        }
    }
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
        let selected = self
            .settings
            .active_variants
            .get(&self.projects[i].path)
            .cloned()
            .flatten();
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.variants.pick_mode = None;
        }
        use schematic_active_bar::icons::{self, Glyph};
        let width = 176.;
        egui::Area::new(egui::Id::new("variant_active_bar"))
            .fade_in(false)
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(
                canvas.center().x - width / 2.,
                canvas.top() + 4.,
            ))
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(Color32::from_gray(58))
                    .corner_radius(5)
                    .inner_margin(egui::Margin::ZERO)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                        ui.horizontal(|ui| {
                            ui.add_space(4.);
                            for (n, label) in [
                                "Selection Filter",
                                "Select",
                                "Toggle Fitted / Not Fitted",
                                "Choose Component Alternate Part",
                                "Edit Component Variation",
                            ]
                            .iter()
                            .enumerate()
                            {
                                let enabled = n < 2 || selected.is_some();
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(100. / 3., 100. / 3.),
                                    egui::Sense::click(),
                                );
                                let active =
                                    n >= 2 && self.variants.pick_mode == Some((n - 2) as u8);
                                if response.hovered() || active {
                                    ui.painter().rect_filled(
                                        rect.shrink2(egui::vec2(1., 2.)),
                                        2.,
                                        Color32::from_rgb(65, 99, 125),
                                    );
                                }
                                let glyph = egui::Rect::from_center_size(
                                    rect.center() - egui::vec2(0., 1.),
                                    egui::vec2(22., 22.),
                                );
                                if n < 2 {
                                    icons::paint(
                                        ui.painter(),
                                        glyph,
                                        if n == 0 { Glyph::Filter } else { Glyph::Select },
                                        true,
                                    );
                                } else {
                                    variant_glyph(ui.painter(), glyph, n, enabled);
                                }
                                if n == 1 {
                                    let at = rect.right_bottom() - egui::vec2(4., 3.);
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![at, at - egui::vec2(5., 0.), at - egui::vec2(0., 5.)],
                                        Color32::WHITE,
                                        egui::Stroke::NONE,
                                    ));
                                }
                                if n < 2 {
                                    egui::Popup::menu(&response).show(|ui| {
                                        if n == 0 {
                                            for (filter, label) in self.documents[index]
                                                .view
                                                .sch_filters
                                                .iter_mut()
                                                .zip([
                                                    "Components",
                                                    "Wires",
                                                    "Buses",
                                                    "Sheet Symbols",
                                                    "Sheet Entries",
                                                    "Net Labels",
                                                    "Parameters",
                                                    "Ports",
                                                    "Power Ports",
                                                    "Texts",
                                                    "Drawing Objects",
                                                    "Other",
                                                ])
                                            {
                                                ui.checkbox(filter, label);
                                            }
                                        } else {
                                            if ui.button("Select All").clicked() {
                                                self.documents[index].view.sch_selection = self
                                                    .documents[index]
                                                    .model
                                                    .schematic
                                                    .as_ref()
                                                    .unwrap()
                                                    .components
                                                    .iter()
                                                    .map(|c| c.id)
                                                    .collect();
                                                ui.close();
                                            }
                                            if ui.button("Deselect All").clicked() {
                                                self.documents[index].view.sch_selection.clear();
                                                self.documents[index].view.selected = None;
                                                ui.close();
                                            }
                                        }
                                    });
                                } else if enabled && response.clicked() {
                                    self.variants.pick_mode =
                                        if active { None } else { Some((n - 2) as u8) };
                                }
                                response.on_hover_text(*label);
                            }
                            ui.add_space(16. / 3.);
                        });
                    });
            });
        if self.variants.pick_mode.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        }
    }

    pub(super) fn variant_canvas_input(
        &mut self,
        index: usize,
        input: &eda_render::SchematicInput,
    ) {
        if input.cancelled || input.secondary_clicked {
            self.variants.pick_mode = None;
            return;
        }
        let Some(mode) = self.variants.pick_mode.filter(|_| input.clicked) else {
            return;
        };
        let Some(i) = self.file_project_index() else {
            return;
        };
        let Some(id) = self
            .settings
            .active_variants
            .get(&self.projects[i].path)
            .cloned()
            .flatten()
        else {
            return;
        };
        let d = &self.documents[index];
        let s = d.model.schematic.as_ref().unwrap();
        let owner = input
            .hit
            .and_then(|id| s.primitive(id).and_then(|p| p.component).or(Some(id)));
        let Some(c) = s.components.iter().find(|c| Some(c.id) == owner) else {
            return;
        };
        let uid = model::component_uid(
            s,
            c,
            d.sch_editor.compiled_path.as_deref().unwrap_or_default(),
        );
        let designator = c.designator.clone();
        if mode == 0 {
            let mut next = self.projects[i].variants.clone();
            if let Some(v) = next.iter_mut().find(|v| v.id == id) {
                let kind = if v.variation(&uid, &designator).is_some_and(|c| c.kind == 1) {
                    0
                } else {
                    1
                };
                let edit = model::Edit::SetKind {
                    id: id.clone(),
                    components: vec![model::ComponentRef { uid, designator }],
                    kind,
                };
                match model::edited(&next, edit).and_then(|next| self.apply_variants(i, next)) {
                    Ok(()) => {}
                    Err(e) => self.message(true, e),
                }
            }
        } else {
            self.variants.pick_mode = None;
            self.open_variants();
            if let Some(d) = &mut self.variants.dialog {
                d.variant = d.draft.iter().position(|v| v.id == id).unwrap_or(0);
                if let Some(n) = d
                    .rows
                    .iter()
                    .position(|r| r.uid == uid && r.designator == designator)
                {
                    d.row = n;
                    d.selected = BTreeSet::from([n]);
                    d.component_kind = Some(if mode == 1 {
                        2
                    } else {
                        d.draft[d.variant]
                            .variation(&uid, &designator)
                            .map_or(0, |c| c.kind)
                    });
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
        let references = rows.iter().map(Row::reference).collect::<Vec<_>>();
        let resolution = match model::resolve_links(&p.variants, &references) {
            Ok(resolution) => resolution,
            Err(error) => {
                self.message(true, error);
                return;
            }
        };
        let mut draft = p.variants.clone();
        let mut history = model::History::default();
        if let Err(error) = history.commit(&mut draft, resolution.variants) {
            self.message(true, error);
            return;
        }
        let link_notice = (!resolution.affected.is_empty()).then(|| format!(
            "Found incorrect link(s) between project variant and Schematic components: [{}].\nPlease review Variant definitions, then recompile and resolve warnings/errors and save the Project to avoid conflicts in future.", resolution.affected.join(", ")
        ));
        let mut link_errors = Vec::new();
        if !resolution.unresolved.is_empty() {
            link_errors.push(format!("Unresolved component links: {}. Review Component Links and validate the schematic.", resolution.unresolved.join(", ")));
        }
        if !resolution.duplicate_designators.is_empty() {
            link_errors.push(format!("Duplicate component designators: {}. Correct them in the schematic and validate the project.", resolution.duplicate_designators.join(", ")));
        }
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
            draft,
            history,
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
            link_notice,
            report: None,
            library: None,
            error: link_errors.join("\n"),
        });
    }
    pub(super) fn select_variant(&mut self, project: PathBuf, id: Option<String>) {
        self.variants.pick_mode = None;
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
