//! Independent Git dock and read-only revision workspace.
#[path = "git_pull.rs"]
mod pull;
use super::*;
use eda_core::document_diff::{self, Diff, Kind};
use eda_io::git::{Commit, File, Repository, Snapshot};
use eda_render::document_diff::{self as render, Mode};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, mpsc},
    time::Duration,
};

pub(super) struct Page {
    pub name: String,
    before: Option<Arc<Document>>,
    after: Option<Arc<Document>>,
    diff: Arc<Diff>,
}
pub(super) struct Review {
    path: String,
    base: String,
    result: String,
    pages: Vec<Page>,
    page: usize,
    text: Option<(String, String)>,
    repository: PathBuf,
    directory: PathBuf,
    displayed: Option<(usize, Mode)>,
    mode: Mode,
    visible: [bool; 4],
    selected: Option<usize>,
    fit: bool,
    initial_model: Option<Document>,
}
impl Review {
    fn scene(&self) -> Option<&Document> {
        let page = self.pages.get(self.page)?;
        if self.mode == Mode::Before {
            page.before.as_deref().or(page.after.as_deref())
        } else {
            page.after.as_deref().or(page.before.as_deref())
        }
    }
}

enum Output {
    Snapshot(Box<Snapshot>),
    Files(Vec<File>),
    Review(Box<Review>),
    Done(String, Box<Snapshot>),
}
struct Job {
    receiver: mpsc::Receiver<Result<Output, String>>,
    label: String,
}
pub(super) struct Browser {
    snapshot: Option<Snapshot>,
    directory: Option<PathBuf>,
    job: Option<Job>,
    history: bool,
    reference: String,
    history_file: Option<String>,
    count: usize,
    query: String,
    only_changes: bool,
    include_unsaved: bool,
    selected: BTreeSet<String>,
    commit: Option<Commit>,
    parent: usize,
    files: Vec<File>,
    message: String,
    feedback: Option<(bool, String)>,
    pending_review: Option<Review>,
    push_confirm: bool,
    push_remote: String,
    push_branch: String,
}
impl Default for Browser {
    fn default() -> Self {
        Self {
            snapshot: None,
            directory: None,
            job: None,
            history: false,
            reference: "HEAD".into(),
            history_file: None,
            count: 100,
            query: String::new(),
            only_changes: false,
            include_unsaved: false,
            selected: BTreeSet::new(),
            commit: None,
            parent: 0,
            files: vec![],
            message: String::new(),
            feedback: None,
            pending_review: None,
            push_confirm: false,
            push_remote: String::new(),
            push_branch: String::new(),
        }
    }
}
#[derive(Default)]
pub(super) struct State {
    pull_request: Option<Repository>,
    pull_job: Option<pull::Job>,
    browser: Browser,
    browsers: BTreeMap<Option<PathBuf>, Browser>,
    followed: Option<(Option<Uuid>, Option<PathBuf>)>,
    pub(super) reviews: BTreeMap<Uuid, Review>,
    change_query: String,
    pub(super) details_open: bool,
}
impl std::ops::Deref for State {
    type Target = Browser;
    fn deref(&self) -> &Browser {
        &self.browser
    }
}
impl std::ops::DerefMut for State {
    fn deref_mut(&mut self) -> &mut Browser {
        &mut self.browser
    }
}
impl State {
    pub(super) fn follow_active_document(&mut self) {
        self.followed = None;
    }
    fn active_review_path(&self, id: Option<Uuid>) -> Option<String> {
        let review = self.reviews.get(&id?)?;
        if self.directory.as_ref() != Some(&review.directory) {
            return None;
        }
        let same = if self.history {
            self.commit.as_ref().is_some_and(|c| {
                c.id == review.result
                    && c.parents.get(self.parent).map_or("Empty", String::as_str) == review.base
            })
        } else {
            self.snapshot
                .as_ref()
                .is_some_and(|s| s.head.as_deref().unwrap_or("Empty") == review.base)
                && (review.result.starts_with("Working tree")
                    || review.result.starts_with("Editor buffer"))
        };
        same.then(|| review.path.clone())
    }
    fn activate(&mut self, ctx: &egui::Context, directory: Option<PathBuf>) {
        if self.directory == directory {
            return;
        }
        let browser = self.browsers.remove(&directory).unwrap_or_else(|| Browser {
            directory: directory.clone(),
            ..Browser::default()
        });
        let old = std::mem::replace(&mut self.browser, browser);
        self.browsers.insert(old.directory.clone(), old);
        if self.job.is_none()
            && let Some(directory) = directory
        {
            self.refresh(ctx, directory);
        }
    }
}
impl Browser {
    fn start(
        &mut self,
        ctx: &egui::Context,
        label: &str,
        work: impl FnOnce() -> Result<Output, String> + Send + 'static,
    ) {
        if self.job.is_some() {
            return;
        }
        self.feedback = None;
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = work();
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        self.job = Some(Job {
            receiver,
            label: label.into(),
        });
    }
    fn refresh(&mut self, ctx: &egui::Context, directory: PathBuf) {
        self.directory = Some(directory.clone());
        let reference = self.reference.clone();
        let count = self.count;
        let file = self.history_file.clone();
        self.start(ctx, "Reading repository…", move || {
            Ok(Output::Snapshot(Box::new(snapshot(
                &Repository::discover(&directory)?,
                &reference,
                count,
                file.as_deref(),
            )?)))
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        let result = self.job.as_ref().map(|j| j.receiver.try_recv());
        match result {
            Some(Ok(result)) => {
                self.job = None;
                match result {
                    Err(error) => self.feedback = Some((true, error)),
                    Ok(Output::Snapshot(snapshot)) => {
                        self.selected
                            .retain(|p| snapshot.files.iter().any(|f| &f.path == p && f.changed()));
                        self.snapshot = Some(*snapshot);
                    }
                    Ok(Output::Files(files)) => self.files = files,
                    Ok(Output::Review(review)) => {
                        self.pending_review = Some(*review);
                    }
                    Ok(Output::Done(message, snapshot)) => {
                        if message.starts_with("Created commit ") {
                            self.message.clear();
                            self.selected.clear();
                        } else {
                            self.selected.retain(|p| {
                                snapshot.files.iter().any(|f| f.path == *p && f.changed())
                            });
                        }
                        self.feedback = Some((false, message));
                        self.snapshot = Some(*snapshot);
                    }
                }
            }
            Some(Err(mpsc::TryRecvError::Disconnected)) => {
                self.job = None;
                self.feedback = Some((true, "Git worker stopped. Refresh to retry.".into()));
            }
            _ => {}
        }
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        }
    }
    fn load_files(&mut self, ctx: &egui::Context) {
        let (Some(snapshot), Some(commit)) = (&self.snapshot, &self.commit) else {
            return;
        };
        let repo = snapshot.repository.clone();
        let commit = commit.clone();
        let parent = commit.parents.get(self.parent).cloned();
        self.files.clear();
        self.start(ctx, "Reading commit files…", move || {
            Ok(Output::Files(
                repo.commit_files(&commit.id, parent.as_deref())?,
            ))
        });
    }
    fn open_diff(&mut self, ctx: &egui::Context, file: File, buffer: Option<Document>) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let repo = snapshot.repository.clone();
        let (base, result) = if self.history {
            let Some(commit) = &self.commit else {
                return;
            };
            (
                commit.parents.get(self.parent).cloned(),
                Some(commit.id.clone()),
            )
        } else {
            (snapshot.head.clone(), None)
        };
        self.start(ctx, "Comparing Altium documents…", move || {
            let old_path = file.old_path.as_deref().unwrap_or(&file.path);
            let before = if let Some(base) = &base {
                repo.blob(base, old_path)?
            } else {
                None
            };
            let after = if buffer.is_some() {
                None
            } else if let Some(result) = &result {
                repo.blob(result, &file.path)?
            } else {
                repo.working_blob(&file.path)?
            };
            let mut review = review(
                &file.path,
                base.as_deref().unwrap_or("Empty"),
                result.as_deref().unwrap_or(if buffer.is_some() {
                    "Editor buffer (unsaved applied edits)"
                } else {
                    "Working tree (saved files)"
                }),
                before,
                after,
                buffer,
            )?;
            review.repository = repo.root.clone();
            review.directory = repo.root.join(&repo.scope);
            Ok(Output::Review(Box::new(review)))
        });
    }
}
fn snapshot(
    repo: &Repository,
    reference: &str,
    count: usize,
    file: Option<&str>,
) -> Result<Snapshot, String> {
    let mut snapshot = repo.snapshot(reference, count)?;
    if let Some(file) = file
        && snapshot.head.is_some()
    {
        snapshot.commits = repo.file_history(reference, count, file)?;
    }
    Ok(snapshot)
}
fn short(id: &str) -> &str {
    if id.len() >= 12 && id.bytes().all(|c| c.is_ascii_hexdigit()) {
        &id[..8]
    } else {
        id
    }
}
fn git_directory(path: &std::path::Path) -> PathBuf {
    let directory = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("PrjPcb") || e.eq_ignore_ascii_case("DsnWrk"))
    {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_path_buf())
}
fn scenes(mut document: Document) -> BTreeMap<String, Document> {
    if let Some(book) = document.library.take() {
        let others = book.others;
        std::iter::once(document)
            .chain(others)
            .map(|mut d| {
                d.library = Some(eda_core::library_edit::Book {
                    others: vec![],
                    source: None,
                });
                let (part, mode) = eda_core::library_edit::variant(&d);
                (
                    format!(
                        "{} · part {part} / mode {mode}",
                        eda_core::library_edit::name(&d)
                    ),
                    d,
                )
            })
            .collect()
    } else {
        BTreeMap::from([("Document".into(), document)])
    }
}
fn review(
    path: &str,
    base: &str,
    result: &str,
    before: Option<Vec<u8>>,
    after: Option<Vec<u8>>,
    buffer: Option<Document>,
) -> Result<Review, String> {
    let mut pages = Vec::new();
    let text = if PathBuf::from(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("PrjPcb") || e.eq_ignore_ascii_case("DsnWrk"))
    {
        let decode = |bytes: Option<Vec<u8>>| {
            bytes
                .map(|b| eda_io::git::project_text(&b))
                .transpose()
                .map(|s| s.unwrap_or_default())
        };
        Some((decode(before)?, decode(after)?))
    } else {
        let import = |bytes: Option<Vec<u8>>| {
            bytes
                .map(|b| eda_io::git::import_document(&b, path).map(scenes))
                .transpose()
                .map(|v| v.unwrap_or_default())
        };
        let identical = buffer.is_none() && before.is_some() && before == after;
        let mut a: BTreeMap<_, _> = import(before)?
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v)))
            .collect();
        let mut b: BTreeMap<_, _> = if identical {
            a.clone()
        } else if let Some(buffer) = buffer {
            scenes(buffer)
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect()
        } else {
            import(after)?
                .into_iter()
                .map(|(k, v)| (k, Arc::new(v)))
                .collect()
        };
        let names: BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
        for name in names {
            let before = a.remove(&name);
            let after = b.remove(&name);
            let diff = Arc::new(document_diff::compare(before.as_deref(), after.as_deref()));
            pages.push(Page {
                name,
                before,
                after,
                diff,
            });
        }
        None
    };
    let mut review = Review {
        path: path.into(),
        base: base.into(),
        result: result.into(),
        pages,
        page: 0,
        text,
        repository: PathBuf::new(),
        directory: PathBuf::new(),
        displayed: None,
        mode: Mode::Overlay,
        visible: [true; 4],
        selected: None,
        fit: false,
        initial_model: None,
    };
    // This function runs on the import/comparison worker. The read-only tab
    // needs one display model for its property panels, but no saved snapshot.
    review.initial_model = review.scene().cloned();
    Ok(review)
}

impl PcbCraft {
    pub(super) fn git_working_path(&self, id: Uuid) -> Option<PathBuf> {
        let review = self.git.reviews.get(&id)?;
        Some(review.repository.join(&review.path))
    }
    pub(super) fn open_git_working_path(&mut self, path: PathBuf) {
        if !path.is_file() {
            self.message(true, "This file does not exist in the working tree");
            return;
        }
        self.request_open_path(path);
    }
    pub(super) fn git_review_active(&self) -> bool {
        self.active.is_some_and(|i| self.documents[i].read_only)
    }
    pub(super) fn git_review_title(&self) -> Option<String> {
        let id = self.documents.get(self.active?)?.session_id;
        let r = self.git.reviews.get(&id)?;
        Some(format!(
            "Git · {} · {} → {} — PCBCraft",
            r.path,
            short(&r.base),
            short(&r.result)
        ))
    }
    pub(super) fn git_tab_title(&self, id: Uuid) -> Option<String> {
        let r = self.git.reviews.get(&id)?;
        Some(format!(
            "{} [Diff {}]",
            r.path.rsplit('/').next().unwrap_or(&r.path),
            short(&r.result)
        ))
    }
    fn activate_existing_committed_review(&mut self, file: &File) -> bool {
        if !self.git.history {
            return false;
        }
        let (Some(snapshot), Some(commit)) = (&self.git.snapshot, &self.git.commit) else {
            return false;
        };
        let base = commit
            .parents
            .get(self.git.parent)
            .map_or("Empty", String::as_str);
        let id = self.git.reviews.iter().find_map(|(id, review)| {
            (review.repository == snapshot.repository.root
                && review.path == file.path
                && review.base == base
                && review.result == commit.id)
                .then_some(*id)
        });
        let Some(index) = id.and_then(|id| self.documents.iter().position(|d| d.session_id == id))
        else {
            return false;
        };
        self.active = Some(index);
        true
    }
    fn install_git_review(&mut self, mut review: Review) {
        if let Some(manager) = &mut self.layer_stack {
            manager.visible = false;
        }
        let existing = self.git.reviews.iter().find_map(|(id, r)| {
            (r.repository == review.repository
                && r.path == review.path
                && r.base == review.base
                && r.result == review.result)
                .then_some(*id)
        });
        if let Some(id) = existing {
            self.active = self.documents.iter().position(|d| d.session_id == id);
            // Saved/buffer comparisons are snapshots; reopening refreshes their data.
            if review.result.starts_with("Working tree")
                || review.result.starts_with("Editor buffer")
            {
                if let Some(index) = self.active {
                    let doc = &mut self.documents[index];
                    if let Some(model) = review
                        .initial_model
                        .take()
                        .or_else(|| review.scene().cloned())
                    {
                        doc.model = model;
                        doc.view.sch_opening = doc.model.schematic.is_some();
                    }
                    doc.content_dirty.set(Some((doc.history.revision, false)));
                    doc.view.pcb.three_d.revision = doc.view.pcb.three_d.revision.wrapping_add(1);
                    doc.view.pcb.three_d.invalidate();
                    review.displayed = Some((review.page, review.mode));
                }
                self.git.reviews.insert(id, review);
                self.sync_git_scene();
            }
            return;
        }
        let model = review
            .initial_model
            .take()
            .or_else(|| review.scene().cloned())
            .unwrap_or_else(|| Document::new(DocumentKind::Schematic, &review.path));
        let mut doc = OpenDocument::new(model, None);
        doc.read_only = true;
        doc.view.sch_opening = doc.model.schematic.is_some();
        doc.content_dirty.set(Some((0, false)));
        review.displayed = Some((review.page, review.mode));
        let id = doc.session_id;
        self.documents.push(doc);
        self.active = Some(self.documents.len() - 1);
        self.git.reviews.insert(id, review);
        self.show_panel(Panel::Changes);
        self.pad_edit = None;
    }
    pub(super) fn sync_git_scene(&mut self) {
        let Some(index) = self.active else {
            return;
        };
        let doc = &mut self.documents[index];
        let Some(review) = self.git.reviews.get_mut(&doc.session_id) else {
            return;
        };
        let displayed = (review.page, review.mode);
        if review.displayed.is_some_and(|(page, mode)| {
            page == review.page && (mode == Mode::Before) == (review.mode == Mode::Before)
        }) {
            // Overlay and After expose the same property/3D model.
            review.displayed = Some(displayed);
            return;
        }
        if let Some(scene) = review.scene() {
            doc.model = scene.clone();
            doc.view.sch_opening = doc.model.schematic.is_some();
            doc.content_dirty.set(Some((doc.history.revision, false)));
            doc.view.pcb.three_d.revision = doc.view.pcb.three_d.revision.wrapping_add(1);
            doc.view.pcb.three_d.invalidate();
        }
        review.displayed = Some(displayed);
    }
    pub(super) fn git_enter_3d(&mut self, index: usize) {
        if let Some(review) = self.git.reviews.get_mut(&self.documents[index].session_id)
            && review.mode == Mode::Overlay
        {
            review.mode = Mode::After;
        }
        self.sync_git_scene();
    }
    pub(super) fn git_view_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input()
            || self.pending.is_some()
            || self.file_state.modal()
            || self.library_update.is_some()
            || self
                .active
                .is_some_and(|i| self.documents[i].view.pcb.configuration_modal_open())
        {
            return;
        }
        if self.pcb_view_shortcuts(ctx) {
            return;
        }
        ctx.input_mut(|input| {
            for (mods, key, command) in [
                (egui::Modifiers::CTRL, Key::W, Command::Close),
                (egui::Modifiers::CTRL, Key::F4, Command::Close),
                (egui::Modifiers::CTRL, Key::O, Command::Open),
                (egui::Modifiers::NONE, Key::Home, Command::Fit),
                (egui::Modifiers::NONE, Key::G, Command::ToggleGrid),
                (egui::Modifiers::NONE, Key::Q, Command::ToggleUnits),
            ] {
                if input.consume_key(mods, key) {
                    self.command = Some(command);
                    return;
                }
            }
            if input.consume_key(egui::Modifiers::NONE, Key::Escape)
                && let Some(i) = self.active
                && let Some(review) = self.git.reviews.get_mut(&self.documents[i].session_id)
            {
                review.selected = None;
            }
        });
    }
    pub(super) fn git_command_allowed(command: Command) -> bool {
        matches!(
            command,
            Command::ShowPanel(_)
                | Command::ShowComponents
                | Command::Open
                | Command::OpenProject
                | Command::Home
                | Command::Close
                | Command::Quit
                | Command::Preferences
                | Command::About
                | Command::Fit
                | Command::ToggleGrid
                | Command::ToggleUnits
                | Command::Pcb3d(_)
                | Command::Pcb3dCamera(_)
                | Command::Pcb(
                    pcb_editor::Action::Layers
                        | pcb_editor::Action::Grid
                        | pcb_editor::Action::Fit(_)
                        | pcb_editor::Action::RouteLayer(_)
                )
        )
    }
    pub(super) fn poll_git(&mut self, ctx: &egui::Context) {
        if self.settings.git || !self.git.reviews.is_empty() {
            let context = (
                self.active.map(|i| self.documents[i].session_id),
                self.active_git_directory(),
            );
            if self.git.followed.as_ref() != Some(&context) {
                if context.0.is_some() {
                    self.git.activate(ctx, context.1.clone());
                } else if self.git.directory.is_none() {
                    let directory = self
                        .settings
                        .git_directory
                        .as_ref()
                        .map(|p| git_directory(p));
                    self.git.activate(ctx, directory);
                }
                self.git.followed = Some(context);
            }
        }
        self.git.poll(ctx);
        for browser in self.git.browsers.values_mut() {
            browser.poll(ctx);
        }
        if let Some(review) = self.git.pending_review.take() {
            self.git.change_query.clear();
            self.install_git_review(review);
        }
    }
    fn active_git_directory(&self) -> Option<PathBuf> {
        let doc = self.documents.get(self.active?)?;
        if let Some(review) = self.git.reviews.get(&doc.session_id) {
            return Some(if review.directory.as_os_str().is_empty() {
                review.repository.clone()
            } else {
                review.directory.clone()
            });
        }
        let source = doc.source_path.as_ref().or(doc.path.as_ref());
        let owns = |project: &&eda_io::altium::Project| {
            doc.new_project.as_ref() == Some(&project.path)
                || source.is_some_and(|path| {
                    path == &project.path
                        || project
                            .documents
                            .iter()
                            .chain(&project.generated_documents)
                            .any(|entry| &entry.path == path)
                })
        };
        let project = self
            .projects
            .iter()
            .filter(owns)
            .find(|p| self.file_state.selected_project.as_ref() == Some(&p.path))
            .or_else(|| self.projects.iter().find(owns));
        project
            .map(|p| git_directory(&p.path))
            .or_else(|| doc.new_project.as_ref().map(|p| git_directory(p)))
            .or_else(|| source.and_then(|p| p.parent()).map(git_directory))
    }
    pub(super) fn git_panel_content(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let unsaved: BTreeSet<_> = self
            .documents
            .iter()
            .filter(|d| d.dirty())
            .filter_map(|d| d.path.as_ref())
            .filter_map(|p| {
                self.git
                    .snapshot
                    .as_ref()
                    .and_then(|s| p.strip_prefix(&s.repository.root).ok())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        let active_review = self
            .git
            .active_review_path(self.active.map(|i| self.documents[i].session_id));
        let state = &mut self.git.browser;
        let busy = state.job.is_some();
        let mut open = None;
        let mut choose = false;
        let mut refresh = false;
        let mut fetch = false;
        let mut pull = false;
        let mut commit = false;
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(4., 6.);
            ui.horizontal(|ui| {
                choose = theme::icon_button(ui, theme::Icon::Folder, false, !busy)
                    .on_hover_text("Open project")
                    .clicked();
                refresh = tool_button(ui, "↻", "Refresh repository", !busy).clicked();
                fetch = ui
                    .add_enabled(
                        !busy && state.snapshot.is_some(),
                        egui::Button::new("Fetch").frame(false),
                    )
                    .on_hover_text("Fetch remote references. Project files stay unchanged; use Pull to update files.")
                    .clicked();
                pull = ui
                    .add_enabled(
                        !busy
                            && state.snapshot.as_ref().is_some_and(|s| {
                                s.head.is_some() && s.upstream.is_some() && s.operation.is_none()
                            }),
                        egui::Button::new("Pull").frame(false),
                    )
                    .on_hover_text("Fetch and update project files from this branch's upstream (fast-forward only). Save editor changes and commit or stash local files first.")
                    .on_disabled_hover_text(state.snapshot.as_ref().map_or(
                        "Open a project repository first.",
                        |s| if s.branch == "Detached HEAD" {
                            "Cannot pull a detached HEAD. Check out a branch in Git first."
                        } else if s.upstream.is_none() {
                            "No upstream configured. Set this branch's upstream in Git, then refresh."
                        } else if s.operation.is_some() {
                            "Finish the current Git operation before pulling."
                        } else {
                            "Fetch and update project files from this branch's upstream (fast-forward only). Save editor changes and commit or stash local files first."
                        },
                    ))
                    .clicked();
                if let Some(job) = &state.job {
                    ui.add(egui::Spinner::new().size(14.));
                    ui.add(
                        egui::Label::new(RichText::new(&job.label).size(11.).color(MUTED))
                            .truncate(),
                    );
                } else {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                state.snapshot.as_ref().is_some_and(|s| {
                                    (s.upstream.is_some() || !s.remotes.is_empty())
                                        && s.head.is_some()
                                        && s.branch != "Detached HEAD"
                                }),
                                egui::Button::new("Push…").frame(false),
                            )
                            .clicked()
                        {
                            state.push_confirm = true;
                            let s = state.snapshot.as_ref().unwrap();
                            state.push_remote = s
                                .remotes
                                .iter()
                                .find(|r| r.as_str() == "origin")
                                .or(s.remotes.first())
                                .cloned()
                                .unwrap_or_default();
                            state.push_branch = s.branch.clone();
                        }
                    });
                }
            });
            if let Some((error, text)) = &state.feedback {
                egui::Frame::NONE
                    .fill(Color32::from_gray(47))
                    .inner_margin(6)
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("git_feedback")
                            .max_height(64.)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::Label::new(RichText::new(text).size(11.).color(
                                        if *error {
                                            render::color(Kind::Removed)
                                        } else {
                                            MUTED
                                        },
                                    ))
                                    .wrap(),
                                );
                            });
                    });
            }
            let Some(snapshot) = &state.snapshot else {
                ui.add_space(20.);
                ui.strong("Open a project");
                ui.label(RichText::new("Open an Altium PCB project (*.PrjPcb).").color(MUTED));
                choose |= ui.button("Open project…").clicked();
                return;
            };
            let root = snapshot.repository.root.clone();
            let project = state
                .directory
                .as_ref()
                .and_then(|p| p.file_name())
                .map_or_else(|| "Project".into(), |p| p.to_string_lossy().into_owned());
            ui.add(egui::Label::new(RichText::new(project).size(13.).strong()).truncate())
                .on_hover_text(snapshot.repository.root.display().to_string());
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(&snapshot.branch).color(MUTED)).truncate())
                    .on_hover_text(
                        snapshot
                            .upstream
                            .as_deref()
                            .unwrap_or("No upstream configured"),
                    );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("↑ {}   ↓ {}", snapshot.ahead, snapshot.behind))
                            .size(11.)
                            .color(MUTED),
                    )
                    .on_hover_text("Outgoing / incoming commits");
                });
            });
            if let Some(operation) = &snapshot.operation {
                ui.colored_label(render::color(Kind::Modified), operation);
            }
            let old_history = state.history;
            ui.horizontal(|ui| {
                let width = (ui.available_width() - 4.) / 2.;
                if nav_tab(ui, "Changes", !state.history, width).clicked() {
                    state.history = false;
                }
                if nav_tab(ui, "History", state.history, width).clicked() {
                    state.history = true;
                }
            });
            if old_history != state.history {
                state.query.clear();
            }
            theme::search_field_height(ui, &mut state.query, ui.available_width(), 25.);
            if state.history {
                if let Some(file) = state.history_file.clone() {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!("History: {file}"))
                                    .size(11.)
                                    .color(MUTED),
                            )
                            .truncate(),
                        )
                        .on_hover_text(file);
                        if ui.small_button("×").clicked() {
                            state.history_file = None;
                            refresh = true;
                        }
                    });
                }
                let mut reference = state.reference.clone();
                egui::ComboBox::from_id_salt("git_branch")
                    .selected_text(format!("Branch: {reference}"))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut reference, "HEAD".into(), "Current branch (HEAD)");
                        for name in &snapshot.refs {
                            ui.selectable_value(&mut reference, name.clone(), name);
                        }
                    });
                if reference != state.reference && !busy {
                    state.reference = reference;
                    state.commit = None;
                    state.files.clear();
                    refresh = true;
                }
                let mut selected = None;
                egui::ScrollArea::vertical()
                    .id_salt("git_history")
                    .max_height((ui.available_height() * 0.46).max(90.))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 0.;
                        for c in snapshot.commits.iter().filter(|c| {
                            format!("{} {} {}", c.subject, c.author, c.id)
                                .to_lowercase()
                                .contains(&state.query.to_lowercase())
                        }) {
                            let response = commit_row(
                                ui,
                                c,
                                state.commit.as_ref().is_some_and(|v| v.id == c.id),
                                !busy,
                            );
                            if response.clicked() {
                                selected = Some(c.clone());
                            }
                        }
                        if snapshot.commits.is_empty() {
                            ui.label("No commits in this project");
                        }
                    });
                if snapshot.commits.len() >= state.count
                    && ui
                        .add_enabled(!busy, egui::Button::new("Load more history").frame(false))
                        .clicked()
                {
                    state.count += 100;
                    refresh = true;
                }
                if let Some(c) = selected {
                    state.commit = Some(c);
                    state.parent = 0;
                    state.load_files(&ctx);
                }
                section_header(
                    ui,
                    "Changed files",
                    &state
                        .commit
                        .as_ref()
                        .map_or(String::new(), |c| short(&c.id).to_string()),
                );
                if let Some(c) = &state.commit {
                    let old_parent = state.parent;
                    if c.parents.len() > 1 {
                        egui::ComboBox::from_id_salt("git_parent")
                            .selected_text(format!("Parent {}", state.parent + 1))
                            .show_ui(ui, |ui| {
                                for (i, p) in c.parents.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut state.parent,
                                        i,
                                        format!("Parent {} · {}", i + 1, short(p)),
                                    );
                                }
                            });
                    }
                    if old_parent != state.parent && !busy {
                        state.load_files(&ctx);
                    }
                } else {
                    ui.label(RichText::new("Select a commit to compare").color(MUTED));
                }
                egui::ScrollArea::vertical()
                    .id_salt("git_commit_files")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        tree(
                            ui,
                            &root,
                            &state.files,
                            active_review.as_deref(),
                            "",
                            false,
                            &mut state.selected,
                            &unsaved,
                            &mut open,
                            !busy,
                        );
                        if state.commit.is_some() && state.files.is_empty() && !busy {
                            ui.label(RichText::new("No supported files changed").color(MUTED));
                        }
                    });
            } else {
                let changed = snapshot
                    .files
                    .iter()
                    .filter(|f| f.changed() || state.include_unsaved && unsaved.contains(&f.path))
                    .count();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{changed} changed")).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.menu_button("View ▾", |ui| {
                            ui.checkbox(&mut state.only_changes, "Changed files only");
                            ui.checkbox(&mut state.include_unsaved, "Include unsaved editor edits")
                                .on_hover_text(
                                    "Compare open document edits. Commits use saved files.",
                                );
                        });
                        if ui
                            .add_enabled(!busy, egui::Button::new("Select all").frame(false))
                            .on_hover_text("Include all changed saved files")
                            .clicked()
                        {
                            state.selected = snapshot
                                .files
                                .iter()
                                .filter(|f| f.changed() && !f.conflicted())
                                .map(|f| f.path.clone())
                                .collect();
                        }
                        if !state.selected.is_empty() && ui.button("Clear").clicked() {
                            state.selected.clear();
                        }
                    });
                });
                egui::TopBottomPanel::bottom("git_commit_footer")
                    .resizable(false)
                    .frame(
                        egui::Frame::NONE
                            .fill(theme::PANEL)
                            .inner_margin(egui::Margin {
                                left: 0,
                                right: 0,
                                top: 8,
                                bottom: 10,
                            }),
                    )
                    .show_inside(ui, |ui| {
                        section_header(ui, "Commit", &format!("{} selected", state.selected.len()));
                        ui.add_sized(
                            [ui.available_width(), 62.],
                            egui::TextEdit::multiline(&mut state.message)
                                .hint_text("Summary of changes")
                                .desired_rows(3)
                                .margin(8),
                        );
                        let dirty = state.selected.iter().any(|p| unsaved.contains(p));
                        if dirty {
                            ui.colored_label(
                                render::color(Kind::Modified),
                                "Save selected documents before committing.",
                            );
                        }
                        commit = ui
                            .add_enabled(
                                !busy
                                    && !dirty
                                    && !state.selected.is_empty()
                                    && !state.message.trim().is_empty()
                                    && snapshot.operation.is_none(),
                                egui::Button::new("Commit selected")
                                    .fill(theme::ACCENT)
                                    .min_size(egui::vec2(ui.available_width(), 28.)),
                            )
                            .on_hover_text(
                                "Commit selected saved files. Other staged files are preserved.",
                            )
                            .clicked();
                    });
                let files: Vec<_> = snapshot
                    .files
                    .iter()
                    .filter(|f| {
                        !state.only_changes
                            || f.changed()
                            || state.include_unsaved && unsaved.contains(&f.path)
                    })
                    .cloned()
                    .collect();
                egui::ScrollArea::vertical()
                    .id_salt("git_worktree")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let matches = tree(
                            ui,
                            &root,
                            &files,
                            active_review.as_deref(),
                            &state.query,
                            true,
                            &mut state.selected,
                            &unsaved,
                            &mut open,
                            !busy,
                        );
                        if matches == 0 {
                            ui.label(
                                RichText::new(if !state.query.is_empty() {
                                    "No files match this search"
                                } else if snapshot.files.is_empty() {
                                    "No supported Altium files"
                                } else if changed == 0 {
                                    "Working tree clean"
                                } else {
                                    "No matching files"
                                })
                                .color(MUTED),
                            );
                        }
                    });
            }
        });
        if choose && !busy {
            self.command = Some(Command::OpenProject);
        }
        if pull && let Some(s) = &self.git.snapshot {
            self.git.pull_request = Some(s.repository.clone());
        }
        if refresh
            && !busy
            && let Some(directory) = self.git.directory.clone()
        {
            self.git.refresh(&ctx, directory);
        }
        if let Some(OpenFile::Working(file)) = &open
            && let Some(snapshot) = &self.git.snapshot
        {
            self.open_git_working_path(snapshot.repository.root.join(&file.path));
        }
        if let Some(OpenFile::Diff(file)) = open
            && !self.activate_existing_committed_review(&file)
        {
            let buffer = (!self.git.history && self.git.include_unsaved)
                .then(|| {
                    let root = &self.git.snapshot.as_ref()?.repository.root;
                    self.documents
                        .iter()
                        .find(|d| {
                            d.dirty()
                                && d.path.as_ref().is_some_and(|p| {
                                    p.strip_prefix(root).is_ok_and(|p| {
                                        p.to_string_lossy().replace('\\', "/") == file.path
                                    })
                                })
                        })
                        .map(|d| d.model.clone())
                })
                .flatten();
            self.git.open_diff(&ctx, file, buffer);
        }
        if fetch && let Some(s) = &self.git.snapshot {
            let repo = s.repository.clone();
            let reference = self.git.reference.clone();
            let count = self.git.count;
            let file = self.git.history_file.clone();
            self.git.start(&ctx, "Fetching…", move || {
                repo.fetch()?;
                Ok(Output::Done(
                    "Fetch complete".into(),
                    Box::new(snapshot(&repo, &reference, count, file.as_deref())?),
                ))
            });
        }
        if commit && let Some(s) = &self.git.snapshot {
            let repo = s.repository.clone();
            let head = s.head.clone();
            let mut paths = self.git.selected.iter().cloned().collect::<Vec<_>>();
            for file in &s.files {
                if self.git.selected.contains(&file.path)
                    && let Some(old) = &file.old_path
                {
                    paths.push(old.clone());
                }
            }
            let message = self.git.message.clone();
            let reference = self.git.reference.clone();
            let count = self.git.count;
            let file = self.git.history_file.clone();
            self.git
                .start(&ctx, "Committing selected saved files…", move || {
                    let id = repo.commit(&paths, &message, head.as_deref())?;
                    Ok(Output::Done(
                        format!("Created commit {}", short(&id)),
                        Box::new(snapshot(&repo, &reference, count, file.as_deref())?),
                    ))
                });
        }
    }
    pub(super) fn git_dialogs(&mut self, ctx: &egui::Context) {
        self.git_push_dialog(ctx);
    }
    pub(super) fn switch_git_repository(&mut self, ctx: &egui::Context, directory: PathBuf) {
        let directory = git_directory(&directory);
        self.settings.git_directory = Some(directory.clone());
        self.git.activate(ctx, Some(directory));
    }
    fn git_push_dialog(&mut self, ctx: &egui::Context) {
        if !self.git.push_confirm {
            return;
        }
        let Some(s) = self.git.snapshot.clone() else {
            return;
        };
        let mut push = false;
        let mut cancel = false;
        egui::Window::new("Push commits")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
            .show_stable(ctx, |ui| {
                if s.upstream.is_none() {
                    ui.label("First push · create upstream tracking");
                    egui::ComboBox::from_id_salt("git_push_remote")
                        .selected_text(&self.git.push_remote)
                        .show_ui(ui, |ui| {
                            for remote in &s.remotes {
                                ui.selectable_value(
                                    &mut self.git.push_remote,
                                    remote.clone(),
                                    remote,
                                );
                            }
                        });
                    ui.horizontal(|ui| {
                        ui.label("Remote branch");
                        ui.text_edit_singleline(&mut self.git.push_branch);
                    });
                }
                ui.label(format!(
                    "{} → {}",
                    s.branch,
                    s.upstream.as_deref().unwrap_or("No upstream")
                ));
                ui.label(format!(
                    "{} outgoing commits · {} incoming",
                    s.ahead, s.behind
                ));
                ui.label(
                    "Push sends complete commits, including files outside this panel's filter.",
                );
                ui.horizontal(|ui| {
                    push = ui
                        .add_enabled(self.git.job.is_none(), egui::Button::new("Push now"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if push {
            let repo = s.repository.clone();
            let first_push = s.upstream.is_none();
            let remote = self.git.push_remote.clone();
            let branch = self.git.push_branch.clone();
            let reference = self.git.reference.clone();
            let count = self.git.count;
            let file = self.git.history_file.clone();
            self.git.push_confirm = false;
            self.git.start(ctx, "Pushing commits…", move || {
                let result = if first_push {
                    repo.first_push(&remote, &branch)?
                } else {
                    repo.push()?
                };
                Ok(Output::Done(
                    if result.is_empty() {
                        "Push complete".into()
                    } else {
                        result
                    },
                    Box::new(snapshot(&repo, &reference, count, file.as_deref())?),
                ))
            });
        }
        if cancel {
            self.git.push_confirm = false;
        }
    }
    pub(super) fn git_workspace(&mut self, ui: &mut egui::Ui, index: usize) {
        let id = self.documents[index].session_id;
        let working_path = self.git_working_path(id);
        let mut open_working = false;
        let mut changes_action = false;
        let changes_selected = self.panel_selected(Panel::Changes);
        let Some(review) = self.git.reviews.get_mut(&id) else {
            return;
        };
        let mut file_history = None;
        ui.spacing_mut().item_spacing = egui::vec2(5., 0.);
        egui::Frame::NONE
            .fill(theme::PANEL)
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    for (mode, label) in [
                        (Mode::Overlay, "Overlay"),
                        (Mode::Before, "Before"),
                        (Mode::After, "After"),
                    ] {
                        if ui
                            .add_enabled_ui(
                                mode != Mode::Overlay
                                    || !self.documents[index].view.pcb.three_d.enabled,
                                |ui| nav_tab(ui, label, review.mode == mode, 55.),
                            )
                            .inner
                            .clicked()
                        {
                            review.mode = mode;
                        }
                    }
                    if ui.selectable_label(changes_selected, "Changes").clicked() {
                        changes_action = true;
                    }
                    open_working = ui
                        .add_enabled(
                            working_path.as_ref().is_some_and(|p| p.is_file()),
                            egui::Button::new("Open working file"),
                        )
                        .on_hover_text("Open the editable file alongside this comparison")
                        .on_disabled_hover_text("This file does not exist in the working tree")
                        .clicked();
                    if self.documents[index].model.pcb.is_some()
                        && ui.button("View Configuration").clicked()
                    {
                        self.documents[index].view.pcb.show_layer_dialog = true;
                    }
                    ui.label(RichText::new("Snapshot · Read only").small().color(MUTED));
                    ui.menu_button("•••", |ui| {
                        if ui.button("File history").clicked() {
                            file_history = Some((review.repository.clone(), review.path.clone()));
                            ui.close();
                        }
                    });
                    ui.add_space(8.);
                    if ui
                        .add(egui::Button::new("Fit").frame(false))
                        .on_hover_text("Fit all changes")
                        .clicked()
                    {
                        review.selected = None;
                        review.fit = true;
                    }
                    if tool_button(ui, "‹", "Previous change", true).clicked() {
                        step(review, false);
                    }
                    if tool_button(ui, "›", "Next change", true).clicked() {
                        step(review, true);
                    }
                });
                ui.add_space(4.);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}  →  {}",
                            short(&review.base),
                            short(&review.result)
                        ))
                        .size(11.)
                        .color(MUTED),
                    )
                    .on_hover_text(format!("Before: {}\nAfter: {}", review.base, review.result));
                });
                ui.add_space(3.);
            });
        if changes_action {
            self.toggle_panel(Panel::Changes);
        }
        if open_working && let Some(path) = working_path {
            self.open_git_working_path(path);
        }
        if let Some((directory, file)) = file_history {
            self.git.history_file = Some(file);
            self.git.history = true;
            self.show_panel(Panel::Git);
            self.git.query.clear();
            self.git.refresh(ui.ctx(), directory);
        }
        let review = self.git.reviews.get_mut(&id).unwrap();
        if review.pages.len() > 1 {
            let old = review.page;
            egui::ComboBox::from_id_salt("git_library_member")
                .selected_text(&review.pages[review.page].name)
                .show_ui(ui, |ui| {
                    for (i, page) in review.pages.iter().enumerate() {
                        ui.selectable_value(
                            &mut review.page,
                            i,
                            format!("{} ({})", page.name, page.diff.changes.len()),
                        );
                    }
                });
            if old != review.page {
                review.selected = None;
                self.documents[index].view = View::default();
                review.fit = true;
            }
        }
        if let Some((before, after)) = &review.text {
            ui.separator();
            ui.label("Project configuration · removed / added lines");
            egui::ScrollArea::both()
                .id_salt("git_text_diff")
                .show(ui, |ui| {
                    for line in before
                        .lines()
                        .filter(|line| !after.lines().any(|l| l == *line))
                    {
                        ui.label(
                            RichText::new(format!("− {line}"))
                                .monospace()
                                .color(render::color(Kind::Removed)),
                        );
                    }
                    for line in after
                        .lines()
                        .filter(|line| !before.lines().any(|l| l == *line))
                    {
                        ui.label(
                            RichText::new(format!("+ {line}"))
                                .monospace()
                                .color(render::color(Kind::Added)),
                        );
                    }
                    ui.separator();
                    ui.collapsing("Before", |ui| {
                        ui.label(RichText::new(before).monospace());
                    });
                    ui.collapsing("After", |ui| {
                        ui.label(RichText::new(after).monospace());
                    });
                });
            return;
        }
        self.sync_git_scene();
        let review = self.git.reviews.get_mut(&id).unwrap();
        let Some(page) = review.pages.get(review.page) else {
            ui.label("No document data in either revision");
            return;
        };
        if page.diff.changes.is_empty() {
            ui.label("No semantic changes in supported document data");
        }
        let fit = std::mem::take(&mut review.fit);
        let canvas = ui.available_rect_before_wrap();
        if self.documents[index].model.kind == DocumentKind::Schematic {
            ui.add_space(26.);
        }
        let scene_rect = ui.available_rect_before_wrap();
        if let Some(i) = render::canvas_shared(
            ui,
            page.before.as_ref(),
            page.after.as_ref(),
            &page.diff,
            &mut self.documents[index].view,
            review.mode,
            review.visible,
            review.selected,
            fit,
        ) {
            review.selected = Some(i);
        }
        self.cursor_mm = ui
            .input(|input| input.pointer.hover_pos())
            .filter(|p| scene_rect.contains(*p))
            .map(|p| self.documents[index].view.camera.world_mm(p, scene_rect));
        if !self.documents[index].view.pcb.three_d.enabled {
            self.active_bar(ui.ctx(), canvas);
        }
    }
    pub(super) fn git_details_content(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.active.map(|i| self.documents[i].session_id) else {
            ui.label("Select a comparison tab to inspect its changes.");
            return;
        };
        let Some(review) = self.git.reviews.get_mut(&id) else {
            ui.label("Select a comparison tab to inspect its changes.");
            return;
        };
        let Some(page) = review.pages.get(review.page) else {
            ui.label("Select a comparison tab to inspect its changes.");
            return;
        };
        ui.scope(|ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = egui::vec2(4., 6.);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} objects", page.diff.changes.len())).color(MUTED),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(
                            review.selected.is_some(),
                            egui::Button::new("Focus selected").frame(false),
                        )
                        .clicked()
                    {
                        review.fit = true;
                    }
                });
            });
            egui::Grid::new("diff_filters")
                .num_columns(2)
                .spacing([8., 3.])
                .show(ui, |ui| {
                    for (i, kind) in [Kind::Removed, Kind::Moved, Kind::Modified, Kind::Added]
                        .into_iter()
                        .enumerate()
                    {
                        let count = page.diff.changes.iter().filter(|c| c.kind == kind).count();
                        ui.checkbox(
                            &mut review.visible[i],
                            RichText::new(format!("{}  {count}", kind.label()))
                                .size(11.)
                                .color(render::color(kind)),
                        );
                        if i % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
            theme::search_field_height(ui, &mut self.git.change_query, ui.available_width(), 25.);
            let query = self.git.change_query.to_lowercase();
            let mut selected = None;
            let visible_changes: Vec<_> = page
                .diff
                .changes
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    let kind_index = match c.kind {
                        Kind::Removed => 0,
                        Kind::Moved => 1,
                        Kind::Modified => 2,
                        Kind::Added => 3,
                    };
                    review.visible[kind_index]
                        && (query.is_empty() || c.label.to_lowercase().contains(&query))
                })
                .collect();
            ui.spacing_mut().item_spacing.y = 0.;
            egui::ScrollArea::vertical()
                .id_salt("git_changes")
                .max_height((ui.available_height() * 0.49).max(100.))
                .auto_shrink([false, false])
                .show_rows(ui, 44., visible_changes.len(), |ui, range| {
                    for &(i, c) in &visible_changes[range] {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 44.),
                            egui::Sense::click(),
                        );
                        if review.selected == Some(i) || response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                0.,
                                if review.selected == Some(i) {
                                    Color32::from_rgb(64, 86, 102)
                                } else {
                                    theme::GROUP
                                },
                            );
                        }
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(
                                rect.min + egui::vec2(3., 9.),
                                egui::vec2(3., 26.),
                            ),
                            1.,
                            render::color(c.kind),
                        );
                        let painter = ui
                            .painter()
                            .with_clip_rect(rect.shrink(3.).intersect(ui.clip_rect()));
                        painter.text(
                            rect.min + egui::vec2(13., 6.),
                            egui::Align2::LEFT_TOP,
                            &c.label,
                            egui::FontId::proportional(12.),
                            Color32::from_gray(230),
                        );
                        painter.text(
                            rect.min + egui::vec2(13., 25.),
                            egui::Align2::LEFT_TOP,
                            format!(
                                "{}{}",
                                c.kind.label(),
                                if c.moved && c.kind != Kind::Moved {
                                    " · moved"
                                } else {
                                    ""
                                }
                            ),
                            egui::FontId::proportional(11.),
                            MUTED,
                        );
                        if response.on_hover_text(&c.label).clicked() {
                            selected = Some(i);
                        }
                    }
                });
            ui.spacing_mut().item_spacing.y = 6.;
            if let Some(i) = selected {
                review.selected = Some(i);
                review.fit = true;
            }
            section_header(
                ui,
                "Properties",
                &review.selected.map_or(String::new(), |i| {
                    format!("{} / {}", i + 1, page.diff.changes.len())
                }),
            );
            if let Some(c) = review.selected.and_then(|i| page.diff.changes.get(i)) {
                ui.add(egui::Label::new(RichText::new(&c.label).strong()).truncate())
                    .on_hover_text(&c.label);
                egui::ScrollArea::vertical()
                    .id_salt("git_fields")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (i, field) in c.fields.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                ui.add_space(5.);
                                ui.label(
                                    RichText::new(property_name(&field.name))
                                        .size(11.)
                                        .color(MUTED),
                                );
                                property_value(
                                    ui,
                                    "Before",
                                    &property_display(&field.name, &field.before),
                                    render::color(Kind::Removed),
                                );
                                property_value(
                                    ui,
                                    "After",
                                    &property_display(&field.name, &field.after),
                                    render::color(Kind::Added),
                                );
                                ui.separator();
                            });
                        }
                        if c.fields.is_empty() {
                            ui.label(
                                RichText::new("Use Before / After to inspect this object.")
                                    .color(MUTED),
                            );
                        }
                    });
            } else {
                ui.add_space(10.);
                ui.label(
                    RichText::new(if page.diff.changes.is_empty() {
                        "No semantic changes"
                    } else {
                        "Select an object to inspect its changes."
                    })
                    .color(MUTED),
                );
            }
        });
    }
}

fn section_header(ui: &mut egui::Ui, title: &str, detail: &str) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 26.), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0., theme::GROUP);
    ui.painter().text(
        rect.left_center() + egui::vec2(6., 0.),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(12.),
        Color32::from_gray(225),
    );
    ui.painter().text(
        rect.right_center() - egui::vec2(6., 0.),
        egui::Align2::RIGHT_CENTER,
        detail,
        egui::FontId::proportional(11.),
        MUTED,
    );
}
fn tool_button(ui: &mut egui::Ui, label: &str, tooltip: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(label).size(17.))
            .frame(false)
            .min_size(egui::vec2(26., 26.)),
    )
    .on_hover_text(tooltip)
}
fn nav_tab(ui: &mut egui::Ui, label: &str, selected: bool, width: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 30.), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(rect, 0., theme::GROUP);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(12.),
        if selected { Color32::WHITE } else { MUTED },
    );
    if selected {
        ui.painter().line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(2., theme::ACCENT),
        );
    }
    response
}
fn step(review: &mut Review, next: bool) {
    let count = review
        .pages
        .get(review.page)
        .map_or(0, |p| p.diff.changes.len());
    if count > 0 {
        review.selected = Some(match review.selected {
            None => 0,
            Some(i) if next => (i + 1) % count,
            Some(i) => (i + count - 1) % count,
        });
        review.fit = true;
    }
}
enum OpenFile {
    Diff(File),
    Working(File),
}
#[derive(Default)]
struct Tree {
    folders: BTreeMap<String, Tree>,
    files: Vec<File>,
}
#[allow(clippy::too_many_arguments)]
fn tree(
    ui: &mut egui::Ui,
    root: &std::path::Path,
    files: &[File],
    active: Option<&str>,
    query: &str,
    checkboxes: bool,
    selected: &mut BTreeSet<String>,
    unsaved: &BTreeSet<String>,
    open: &mut Option<OpenFile>,
    enabled: bool,
) -> usize {
    let mut tree = Tree::default();
    let mut count = 0;
    for file in files
        .iter()
        .filter(|f| f.path.to_lowercase().contains(&query.to_lowercase()))
    {
        count += 1;
        let mut parts = file.path.split('/').collect::<Vec<_>>();
        parts.pop();
        let mut node = &mut tree;
        for p in parts {
            node = node.folders.entry(p.into()).or_default();
        }
        node.files.push(file.clone());
    }
    #[allow(clippy::too_many_arguments)]
    fn show(
        ui: &mut egui::Ui,
        root: &std::path::Path,
        tree: &Tree,
        checkboxes: bool,
        selected: &mut BTreeSet<String>,
        unsaved: &BTreeSet<String>,
        open: &mut Option<OpenFile>,
        enabled: bool,
        active: Option<&str>,
    ) {
        for (folder, node) in &tree.folders {
            egui::CollapsingHeader::new(folder)
                .default_open(true)
                .show(ui, |ui| {
                    show(
                        ui, root, node, checkboxes, selected, unsaved, open, enabled, active,
                    )
                });
        }
        ui.spacing_mut().item_spacing.y = 0.;
        for file in &tree.files {
            let (row, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 27.),
                if enabled {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            let active = active == Some(file.path.as_str());
            if active || response.hovered() {
                ui.painter().rect_filled(
                    row,
                    0.,
                    if active {
                        Color32::from_rgb(64, 86, 102)
                    } else {
                        theme::GROUP
                    },
                );
            }
            let mut x = row.left() + 3.;
            let mut checkbox_hit = false;
            if checkboxes {
                let mut checked = selected.contains(&file.path);
                let check_rect =
                    egui::Rect::from_min_size(egui::pos2(x, row.top() + 4.), egui::vec2(19., 19.));
                let check = ui
                    .scope_builder(egui::UiBuilder::new().max_rect(check_rect), |ui| {
                        ui.add_enabled(
                            enabled && file.changed() && !file.conflicted(),
                            egui::Checkbox::without_text(&mut checked),
                        )
                    })
                    .inner
                    .on_hover_text("Include saved file in next commit");
                checkbox_hit = check.hovered();
                if check.changed() {
                    if checked {
                        selected.insert(file.path.clone());
                    } else {
                        selected.remove(&file.path);
                    }
                }
                x += 23.;
            }
            let ext = file.path.rsplit('.').next().unwrap_or("").to_lowercase();
            let icon =
                egui::Rect::from_min_size(egui::pos2(x, row.top() + 5.), egui::vec2(22., 16.));
            super::document_bars::document_icon(
                ui,
                icon,
                if ext.starts_with("sch") {
                    DocumentKind::Schematic
                } else {
                    DocumentKind::Board
                },
            );
            x += 24.;
            let name = file.path.rsplit('/').next().unwrap_or(&file.path);
            let label = format!(
                "{name}{}",
                if unsaved.contains(&file.path) {
                    " *"
                } else {
                    ""
                }
            );
            let text_rect = egui::Rect::from_min_max(
                egui::pos2(x, row.top()),
                row.right_bottom() - egui::vec2(25., 0.),
            );
            ui.painter()
                .with_clip_rect(text_rect.intersect(ui.clip_rect()))
                .text(
                    egui::pos2(x, row.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(12.),
                    Color32::from_gray(if enabled { 225 } else { 145 }),
                );
            if file.changed() {
                let color = if file.conflicted() || file.status.contains('D') {
                    render::color(Kind::Removed)
                } else if file.status.contains(['A', '?']) {
                    render::color(Kind::Added)
                } else {
                    render::color(Kind::Modified)
                };
                let status = if file.status.contains('?') {
                    "A"
                } else {
                    file.status.trim()
                };
                ui.painter().text(
                    row.right_center() - egui::vec2(8., 0.),
                    egui::Align2::RIGHT_CENTER,
                    status,
                    egui::FontId::proportional(11.),
                    color,
                );
            }
            let response = response.on_hover_text(format!(
                "{}\nClick to open; right-click for working file or comparison",
                file.path
            ));
            response.context_menu(|ui| {
                if ui
                    .add_enabled(
                        root.join(&file.path).is_file(),
                        egui::Button::new("Open working file"),
                    )
                    .on_disabled_hover_text("This file does not exist in the working tree")
                    .clicked()
                {
                    *open = Some(OpenFile::Working(file.clone()));
                    ui.close();
                }
                if ui
                    .add_enabled(enabled, egui::Button::new("Open diff"))
                    .clicked()
                {
                    *open = Some(OpenFile::Diff(file.clone()));
                    ui.close();
                }
            });
            if response.clicked() && !checkbox_hit {
                *open = Some(
                    if checkboxes && !file.changed() && !unsaved.contains(&file.path) {
                        OpenFile::Working(file.clone())
                    } else {
                        OpenFile::Diff(file.clone())
                    },
                );
            }
        }
    }

    show(
        ui, root, &tree, checkboxes, selected, unsaved, open, enabled, active,
    );
    count
}

fn commit_row(ui: &mut egui::Ui, c: &Commit, selected: bool, enabled: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 49.),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            0.,
            if selected {
                Color32::from_rgb(64, 86, 102)
            } else {
                theme::GROUP
            },
        );
    }
    let dot = egui::pos2(rect.left() + 10., rect.top() + 16.);
    ui.painter().line_segment(
        [
            egui::pos2(dot.x, rect.top()),
            egui::pos2(dot.x, rect.bottom()),
        ],
        egui::Stroke::new(1., Color32::from_gray(95)),
    );
    ui.painter().circle_filled(
        dot,
        3.,
        if selected {
            Color32::from_rgb(157, 199, 226)
        } else {
            MUTED
        },
    );
    let clip = rect.shrink2(egui::vec2(5., 0.)).intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    painter.text(
        egui::pos2(rect.left() + 23., rect.top() + 7.),
        egui::Align2::LEFT_TOP,
        &c.subject,
        egui::FontId::proportional(12.),
        Color32::from_gray(232),
    );
    painter.text(
        egui::pos2(rect.left() + 23., rect.top() + 27.),
        egui::Align2::LEFT_TOP,
        format!(
            "{} · {} · {}",
            short(&c.id),
            c.author,
            c.date.get(..10).unwrap_or(&c.date)
        ),
        egui::FontId::proportional(11.),
        Color32::from_gray(174),
    );
    response.on_hover_text(format!("{}\n{}\n{}\n{}", c.subject, c.id, c.refs, c.body))
}
fn property_value(ui: &mut egui::Ui, label: &str, value: &str, color: Color32) {
    ui.horizontal_top(|ui| {
        ui.add_sized(
            [43., 18.],
            egui::Label::new(RichText::new(label).size(11.).color(MUTED)),
        );
        let response = if value.chars().count() > 180 {
            let preview: String = value.chars().take(48).collect();
            egui::CollapsingHeader::new(RichText::new(format!("{preview}…")).size(12.).color(color))
                .id_salt(label)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(value).size(12.).color(color)).wrap());
                })
                .header_response
        } else {
            ui.add(egui::Label::new(RichText::new(value).size(12.).color(color)).wrap())
        };
        response.context_menu(|ui| {
            if ui.button("Copy value").clicked() {
                ui.ctx().copy_text(value.into());
                ui.close();
            }
        });
    });
}

fn property_name(name: &str) -> String {
    match name {
        "kind.a.x" => "First corner · X".into(),
        "kind.a.y" => "First corner · Y".into(),
        "kind.b.x" => "Second corner · X".into(),
        "kind.b.y" => "Second corner · Y".into(),
        "shape.start.x" => "Start · X".into(),
        "shape.start.y" => "Start · Y".into(),
        "shape.end.x" => "End · X".into(),
        "shape.end.y" => "End · Y".into(),
        "shape.position.x" | "kind.position.x" => "Position · X".into(),
        "shape.position.y" | "kind.position.y" => "Position · Y".into(),
        "width_nm" | "shape.width" => "Width".into(),
        _ => name
            .trim_start_matches("component_data.")
            .trim_start_matches("kind.")
            .trim_start_matches("shape.")
            .replace("_nm", "")
            .replace('_', " ")
            .replace('.', " / "),
    }
}
fn property_display<'a>(name: &str, value: &'a str) -> std::borrow::Cow<'a, str> {
    let dimensions = name.ends_with(".x")
        || name.ends_with(".y")
        || name.ends_with("_nm")
        || matches!(
            name,
            "shape.width" | "shape.height" | "shape.radius" | "shape.hole" | "shape.diameter"
        );
    if dimensions
        && !name.contains("parameters.")
        && !name.contains("properties.")
        && let Ok(nm) = value.parse::<i64>()
    {
        let number = format!("{:.6}", nm as f64 / 1e6);
        return format!("{} mm", number.trim_end_matches('0').trim_end_matches('.')).into();
    }
    if value.len() < 180
        && value.starts_with('"')
        && let Ok(text) = serde_json::from_str::<String>(value)
    {
        return text.into();
    }
    value.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_geom::Point;
    #[test]
    fn follows_project_membership_source_paths_and_unsaved_project_documents() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let project = root.join("Main.PrjPcb");
        std::fs::write(
            &project,
            "[Design]\n[Document1]\nDocumentPath=Sheets/Child.SchDoc\n",
        )
        .unwrap();
        let mut app = crate::app::schematic_editor::tests::app();
        app.projects
            .push(eda_io::altium::load_project(&project).unwrap());
        app.documents[0].source_path = Some(root.join("Sheets/Child.SchDoc"));
        assert_eq!(app.active_git_directory(), Some(root.clone()));
        app.documents[0].source_path = None;
        app.documents[0].new_project = Some(project);
        assert_eq!(app.active_git_directory(), Some(root.clone()));
        app.documents[0].new_project = None;
        app.documents[0].path = Some(root.join("Loose/Other.PcbDoc"));
        assert_eq!(app.active_git_directory(), Some(root.join("Loose")));
        app.documents[0].path = None;
        app.file_state.selected_project = Some(app.projects[0].path.clone());
        assert_eq!(app.active_git_directory(), None);
    }
    fn context_snapshot(directory: &std::path::Path) -> Snapshot {
        Snapshot {
            repository: Repository {
                root: directory.to_path_buf(),
                scope: String::new(),
            },
            branch: "main".into(),
            upstream: None,
            ahead: 0,
            behind: 0,
            head: Some("baseline".into()),
            files: vec![],
            commits: vec![],
            refs: vec![],
            remotes: vec![],
            operation: None,
        }
    }
    #[test]
    fn active_working_and_deleted_revision_tabs_restore_their_own_browser_and_jobs() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let a = root.join("A");
        let b = root.join("B");
        let mut app = crate::app::schematic_editor::tests::app();
        app.documents[0].path = Some(a.join("Same.SchDoc"));
        app.add_document(
            Document::new(DocumentKind::Schematic, "B"),
            Some(b.join("Same.SchDoc")),
        );
        app.show_panel(Panel::Git);
        app.settings.git_directory = Some(b.clone()); // stale persisted choice must lose to active A
        app.active = Some(0);
        let ctx = egui::Context::default();
        app.poll_git(&ctx);
        assert_eq!(app.git.directory, Some(a.clone()));
        let (sender, receiver) = mpsc::channel();
        app.git.job = Some(Job {
            receiver,
            label: "A operation".into(),
        });
        app.git.message = "Draft A".into();
        app.git.query = "A filter".into();
        app.active = Some(1);
        app.poll_git(&ctx);
        assert_eq!(app.git.directory, Some(b.clone()));
        app.git.job = None;
        app.git.snapshot = Some(context_snapshot(&b));
        app.git.message = "Draft B".into();
        sender
            .send(Ok(Output::Snapshot(Box::new(context_snapshot(&a)))))
            .unwrap();
        app.poll_git(&ctx);
        assert_eq!(app.git.snapshot.as_ref().unwrap().repository.root, b);
        let mut comparison = fixture_review("A/Deleted.PcbDoc", "commit-a");
        comparison.repository = root.clone();
        comparison.directory = a.clone(); // scope inside one repo must not become repository root
        app.install_git_review(comparison);
        let id = app.documents[app.active.unwrap()].session_id;
        let title = app.git_tab_title(id);
        app.poll_git(&ctx);
        assert_eq!(app.git.directory, Some(a.clone()));
        assert_eq!(app.git.message, "Draft A");
        assert_eq!(app.git.query, "A filter");
        app.git.job = None;
        assert_eq!(app.git.snapshot.as_ref().unwrap().repository.root, a);
        app.active = Some(1);
        app.poll_git(&ctx);
        assert_eq!(app.git.message, "Draft B");
        assert_eq!(app.git.directory, Some(b));
        assert_eq!(app.git_tab_title(id), title);
        assert_eq!(
            app.git_working_path(id),
            Some(root.join("A/Deleted.PcbDoc"))
        );
        app.active = Some(0);
        app.poll_git(&ctx);
        assert_eq!(app.git.directory, Some(a.clone()));
        app.switch_git_repository(&ctx, root.join("B"));
        app.show_panel(Panel::Git);
        app.poll_git(&ctx);
        assert_eq!(app.git.directory, Some(a));
        app.documents[0].path = None;
        app.poll_git(&ctx);
        assert!(app.git.directory.is_none());
        assert!(app.git.snapshot.is_none());
    }
    #[test]
    fn late_comparison_does_not_activate_a_tab_in_another_project() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let a = root.join("A");
        let b = root.join("B");
        let mut app = crate::app::schematic_editor::tests::app();
        app.documents[0].path = Some(a.join("Review.PcbDoc"));
        app.add_document(
            Document::new(DocumentKind::Board, "B"),
            Some(b.join("Review.PcbDoc")),
        );
        app.show_panel(Panel::Git);
        let ctx = egui::Context::default();
        app.active = Some(0);
        app.poll_git(&ctx);
        let (sender, receiver) = mpsc::channel();
        app.git.job = Some(Job {
            receiver,
            label: "Comparing A".into(),
        });
        let mut comparison = fixture_review("A/Review.PcbDoc", "commit-a");
        comparison.repository = root;
        comparison.directory = a.clone();
        sender
            .send(Ok(Output::Review(Box::new(comparison))))
            .unwrap();
        app.active = Some(1);
        app.poll_git(&ctx);
        assert_eq!(app.active, Some(1));
        assert_eq!(app.documents.len(), 2);
        assert_eq!(app.git.directory, Some(b));
        app.active = Some(0);
        app.poll_git(&ctx);
        assert_eq!(app.documents.len(), 3);
        assert!(app.git_review_active());
        assert_eq!(app.active_git_directory(), Some(a));
    }
    #[test]
    fn working_file_opens_beside_snapshot_and_reuses_unsaved_editor() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let path = root.join("Parallel.SchDoc");
        let model = eda_io::altium::new_schematic("Parallel.SchDoc").unwrap();
        let bytes = eda_io::altium::writer::serialize(&model).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        let mut app = crate::app::schematic_editor::tests::app();
        let mut comparison = review(
            "Parallel.SchDoc",
            "before",
            "after",
            Some(bytes.clone()),
            Some(bytes.clone()),
            None,
        )
        .unwrap();
        comparison.directory = root.clone();
        comparison.repository = root;
        app.install_git_review(comparison);
        let revision_index = app.active.unwrap();
        let revision_id = app.documents[revision_index].session_id;
        app.documents[revision_index].view.camera.pixels_per_mm = 31.;
        app.open_git_working_path(app.git_working_path(revision_id).unwrap());
        let ctx = egui::Context::default();
        let started = std::time::Instant::now();
        while app.document_loader.busy() && started.elapsed().as_secs() < 5 {
            app.poll_document_loader(&ctx);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!app.document_loader.busy());
        assert_eq!(app.documents.len(), 3);
        let working = app.active.unwrap();
        assert_ne!(working, revision_index);
        assert_eq!(app.documents[working].path.as_ref(), Some(&path));
        assert!(!app.documents[working].read_only);
        app.documents[working].model.name = "Unsaved local edit".into();
        app.documents[working].content_dirty.set(None);
        let unsaved = app.documents[working].model.clone();
        app.active = Some(revision_index);
        app.open_git_working_path(path.clone());
        assert_eq!(app.active, Some(working));
        assert_eq!(app.documents.len(), 3);
        assert_eq!(app.documents[working].model, unsaved);
        assert!(app.documents[working].dirty());
        assert_eq!(app.documents[revision_index].view.camera.pixels_per_mm, 31.);
        assert!(app.git.reviews.contains_key(&revision_id));
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    #[test]
    fn changing_repository_preserves_existing_revision_tabs() {
        let mut app = crate::app::schematic_editor::tests::app();
        app.install_git_review(fixture_review("Old.PcbDoc", "old-commit"));
        let id = app.documents[app.active.unwrap()].session_id;
        let title = app.git_tab_title(id);
        let directory = tempfile::tempdir().unwrap();
        app.switch_git_repository(&egui::Context::default(), directory.path().to_path_buf());
        assert_eq!(app.git_tab_title(id), title);
        assert!(app.git.reviews.contains_key(&id));
        assert!(app.git.details_open);
        assert_eq!(app.documents.len(), 2);
        assert_eq!(
            app.git_working_path(id),
            Some(PathBuf::from("fixture").join("Old.PcbDoc"))
        );
    }
    fn fixture_review(name: &str, result: &str) -> Review {
        let mut before = Document::new(DocumentKind::Board, name);
        before.pcb = Some(eda_core::pcb::Board::default());
        let mut after = before.clone();
        after
            .pcb
            .as_mut()
            .unwrap()
            .items
            .push(eda_core::pcb::Item::new(
                eda_core::pcb::TOP,
                None,
                eda_core::pcb::Shape::Track {
                    start: Point::mm(1, 1),
                    end: Point::mm(8, 1),
                    width: 300_000,
                },
            ));
        let diff = document_diff::compare(Some(&before), Some(&after));
        Review {
            path: name.into(),
            base: "baseline".into(),
            result: result.into(),
            pages: vec![Page {
                name: name.into(),
                before: Some(Arc::new(before)),
                after: Some(Arc::new(after)),
                diff: Arc::new(diff),
            }],
            page: 0,
            text: None,
            repository: PathBuf::from("fixture"),
            directory: PathBuf::from("fixture"),
            displayed: None,
            mode: Mode::Overlay,
            visible: [true; 4],
            selected: None,
            fit: false,
            initial_model: None,
        }
    }
    #[test]
    fn worker_prepared_display_moves_into_read_only_tab_without_saved_or_first_frame_copy() {
        let doc = eda_io::altium::new_schematic("Prepared.SchDoc").unwrap();
        let bytes = eda_io::altium::writer::serialize(&doc).unwrap();
        let review = review(
            "Prepared.SchDoc",
            "before",
            "after",
            Some(bytes.clone()),
            Some(bytes),
            None,
        )
        .unwrap();
        let page = &review.pages[0];
        assert!(Arc::ptr_eq(
            page.before.as_ref().unwrap(),
            page.after.as_ref().unwrap()
        ));
        assert!(page.diff.changes.is_empty());
        let name = review.initial_model.as_ref().unwrap().name.as_ptr();
        let mut app = crate::app::schematic_editor::tests::app();
        app.install_git_review(review);
        let index = app.active.unwrap();
        app.sync_git_scene();
        assert_eq!(app.documents[index].model.name.as_ptr(), name);
        assert!(app.documents[index].saved.is_none());
        assert!(!app.documents[index].dirty());
        let id = app.documents[index].session_id;
        app.git.reviews.get_mut(&id).unwrap().mode = Mode::After;
        app.sync_git_scene();
        assert_eq!(app.documents[index].model.name.as_ptr(), name);
    }
    #[test]
    fn reopening_an_open_commit_activates_it_without_a_git_job_but_working_tree_still_refreshes() {
        let mut app = crate::app::schematic_editor::tests::app();
        app.install_git_review(fixture_review("a.PcbDoc", "commit-a"));
        let index = app.active.unwrap();
        app.documents[index].view.camera.pixels_per_mm = 23.;
        app.git.snapshot = Some(Snapshot {
            repository: Repository {
                root: "fixture".into(),
                scope: String::new(),
            },
            branch: String::new(),
            upstream: None,
            ahead: 0,
            behind: 0,
            head: None,
            files: vec![],
            commits: vec![],
            refs: vec![],
            remotes: vec![],
            operation: None,
        });
        app.git.commit = Some(Commit {
            id: "commit-a".into(),
            parents: vec!["baseline".into()],
            author: String::new(),
            date: String::new(),
            subject: String::new(),
            body: String::new(),
            refs: String::new(),
        });
        app.git.history = true;
        app.active = Some(0);
        let file = File {
            path: "a.PcbDoc".into(),
            old_path: None,
            status: "M".into(),
        };
        assert!(app.activate_existing_committed_review(&file));
        assert_eq!(app.active, Some(index));
        assert_eq!(app.documents[index].view.camera.pixels_per_mm, 23.);
        assert!(app.git.job.is_none());
        app.git.parent = 1;
        assert!(!app.activate_existing_committed_review(&file));
        app.git.parent = 0;
        app.git.history = false;
        assert!(!app.activate_existing_committed_review(&file));
    }
    #[test]
    fn review_tabs_keep_independent_views_reuse_identity_and_close_individually() {
        let mut app = crate::app::schematic_editor::tests::app();
        let original = app.documents[0].model.clone();
        app.install_git_review(fixture_review("a.PcbDoc", "commit-a"));
        let first = app.active.unwrap();
        let first_id = app.documents[first].session_id;
        app.documents[first].view.camera.pixels_per_mm = 23.;
        app.documents[first].view.pcb.single_layer = 2;
        app.install_git_review(fixture_review("b.PcbDoc", "commit-a"));
        let second_id = app.documents[app.active.unwrap()].session_id;
        app.install_git_review(fixture_review("a.PcbDoc", "commit-a"));
        assert_eq!(app.active, Some(first));
        assert_eq!(app.documents.len(), 3);
        assert_eq!(app.documents[first].view.camera.pixels_per_mm, 23.);
        assert_eq!(app.documents[first].view.pcb.single_layer, 2);
        app.install_git_review(fixture_review("a.PcbDoc", "commit-b"));
        assert_eq!(app.documents.len(), 4);
        assert!(
            app.documents
                .iter()
                .skip(1)
                .all(|d| d.read_only && !d.dirty() && d.path.is_none())
        );
        app.request_close(first_id);
        assert!(!app.git.reviews.contains_key(&first_id));
        assert!(app.git.reviews.contains_key(&second_id));
        assert_eq!(app.documents[0].model, original);
        assert!(app.pending.is_none());
    }
    #[test]
    fn review_native_view_commands_and_revision_switch_never_edit_working_document() {
        let mut app = crate::app::schematic_editor::tests::app();
        let original = app.documents[0].model.clone();
        app.install_git_review(fixture_review("board.PcbDoc", "commit"));
        let index = app.active.unwrap();
        let id = app.documents[index].session_id;
        let ctx = egui::Context::default();
        app.run(Command::Pcb(pcb_editor::Action::Layers), &ctx);
        assert!(app.documents[index].view.pcb.show_layer_dialog);
        app.run(Command::Pcb(pcb_editor::Action::RouteLayer(32)), &ctx);
        assert_eq!(app.documents[index].view.pcb.active_layer, 32);
        app.git.reviews.get_mut(&id).unwrap().mode = Mode::Before;
        app.sync_git_scene();
        assert!(
            app.documents[index]
                .model
                .pcb
                .as_ref()
                .unwrap()
                .items
                .is_empty()
        );
        let generation = app.documents[index].view.pcb.three_d.revision;
        app.git.reviews.get_mut(&id).unwrap().mode = Mode::After;
        app.sync_git_scene();
        assert_eq!(
            app.documents[index].model.pcb.as_ref().unwrap().items.len(),
            1
        );
        assert!(app.documents[index].view.pcb.three_d.revision > generation);
        assert!(!app.save_index(index, true));
        app.run(Command::Pcb(pcb_editor::Action::Delete), &ctx);
        assert_eq!(
            app.documents[index].model.pcb.as_ref().unwrap().items.len(),
            1
        );
        assert!(!app.documents[index].dirty());
        assert_eq!(app.documents[0].model, original);
        app.run(Command::Close, &ctx);
        assert_eq!(app.documents.len(), 1);
    }
    #[test]
    fn review_blocks_working_document_edit_save_and_history_commands() {
        let mut app = crate::app::schematic_editor::tests::app();
        let before = app.documents[0].model.clone();
        let revision = app.documents[0].history.revision;
        let count = app.documents.len();
        app.documents[0].read_only = true;
        let ctx = egui::Context::default();
        for command in [
            Command::Save,
            Command::SaveAll,
            Command::Undo,
            Command::Redo,
            Command::NewBoard,
            Command::SchEdit(SchAction::Delete),
            Command::SchPlace(Placement::Wire),
        ] {
            app.run(command, &ctx);
        }
        assert!(app.git_review_active());
        assert_eq!(app.documents.len(), count);
        assert_eq!(app.documents[0].model, before);
        assert_eq!(app.documents[0].history.revision, revision);
        app.settings.git = false;
        app.run(Command::ShowPanel(Panel::Git), &ctx);
        assert!(app.settings.git);
    }
    #[test]
    fn library_review_includes_added_and_removed_members_not_just_active_definition() {
        use eda_core::library_edit as lib;
        let a = lib::new(DocumentKind::Schematic, "Review.SchLib");
        let b = lib::preview(&a, &lib::Edit::Add("Second".into())).unwrap();
        let before = eda_io::library_documents::serialize(&a).unwrap();
        let after = eda_io::library_documents::serialize(&b).unwrap();
        let review = review(
            "Review.SchLib",
            "base",
            "result",
            Some(before),
            Some(after),
            None,
        )
        .unwrap();
        assert_eq!(review.pages.len(), 2);
        assert!(
            review
                .pages
                .iter()
                .any(|p| p.before.is_none() && p.after.is_some())
        );
    }
    #[test]
    fn unsaved_editor_snapshot_is_compared_without_saving_or_mutating_it() {
        let before = eda_io::altium::new_schematic("Review.SchDoc").unwrap();
        let bytes = eda_io::altium::writer::serialize(&before).unwrap();
        let mut buffer = before.clone();
        buffer
            .schematic
            .as_mut()
            .unwrap()
            .sheet
            .parameters
            .insert("TITLE".into(), "Unsaved title".into());
        let original = buffer.clone();
        let review = review(
            "Review.SchDoc",
            "HEAD",
            "Editor buffer",
            Some(bytes.clone()),
            Some(bytes),
            Some(buffer.clone()),
        )
        .unwrap();
        assert_eq!(review.pages[0].diff.changes.len(), 1);
        assert_eq!(buffer, original);
    }
}
