//! Pull owns the editor while Git updates files and imports their replacements.
use super::*;

pub(super) struct Job {
    receiver: mpsc::Receiver<Outcome>,
}
struct Reload<T> {
    path: PathBuf,
    result: Result<T, String>,
}
struct Outcome {
    result: Result<Vec<String>, String>,
    snapshot: Result<Snapshot, String>,
    documents: Vec<(Uuid, Reload<document_loader::PreparedDocument>)>,
    projects: Vec<Reload<eda_io::altium::Project>>,
    symbols: Vec<Reload<eda_core::library::Library>>,
    footprints: Vec<Reload<eda_io::altium_pcb::library::Library>>,
    group_changed: bool,
}
impl PcbCraft {
    pub(in crate::app) fn git_pull_active(&self) -> bool {
        self.git.pull_job.is_some() || self.git.pull_request.is_some()
    }

    pub(in crate::app) fn defer_git_pull_command(&mut self, command: Command) -> bool {
        if !self.git_pull_active() {
            return false;
        }
        if matches!(command, Command::Quit) {
            self.git.pull_close_after = true;
        }
        true
    }

    pub(in crate::app) fn git_pull_frame(&mut self, ctx: &egui::Context) -> bool {
        if !self.poll_git_pull(ctx) {
            return false;
        }
        self.show_git_pull_ui(ctx);
        true
    }

    /// Paint the ordinary workspace, with editing disabled, throughout the job.
    /// Input finishers, shortcuts and asynchronous model publications stay paused.
    pub(in crate::app) fn show_git_pull_ui(&mut self, ctx: &egui::Context) {
        self.title_bar(ctx);
        self.menus(ctx);
        self.status(ctx);
        self.left_dock(ctx);
        self.right_dock(ctx);
        self.workspace(ctx);
        self.finish_docks(ctx);
        self.resize_edges(ctx);
        if let Some(command) = self.command.take() {
            self.defer_git_pull_command(command);
        }
        self.git_pull_progress(ctx);
    }

    /// Also drawn on the requesting frame, so the modal owns input before the
    /// first worker frame, including fast local refusals and slow fetches.
    pub(in crate::app) fn git_pull_progress(&mut self, ctx: &egui::Context) {
        if !self.git_pull_active() {
            return;
        }
        egui::Modal::new(egui::Id::new("git_pull_progress"))
            .backdrop_color(Color32::from_black_alpha(65))
            .show_stable(ctx, 410., |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.heading("Pulling changes…");
                });
                ui.label("Fetching upstream, updating files and reloading changed documents.");
                ui.label(if self.git.pull_close_after {
                    "PCBCraft will close after Pull finishes, using the normal unsaved-work dialog."
                } else {
                    "Editing is paused until Pull finishes. You can minimize the window."
                });
                ui.horizontal(|ui| {
                    if ui.button("Minimize").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    if ui
                        .add_enabled(
                            !self.git.pull_close_after,
                            egui::Button::new("Close after Pull"),
                        )
                        .clicked()
                    {
                        self.git.pull_close_after = true;
                    }
                });
            });
    }

    fn git_pull_blocker(&self, root: &std::path::Path) -> Option<&'static str> {
        if self.document_loader.busy()
            || self.git.job.is_some()
            || self.git.browsers.values().any(|b| b.job.is_some())
        {
            return Some("Wait for document imports and Git jobs before pulling.");
        }
        if self.documents.iter().any(|d| {
            !d.read_only
                && d.path
                    .as_ref()
                    .or(d.source_path.as_ref())
                    .or(d.new_project.as_ref())
                    .is_some_and(|p| p.starts_with(root))
                && (d.dirty() || d.pcb_editor.rules_open())
        }) || self
            .projects
            .iter()
            .any(|p| p.path.starts_with(root) && self.variants.dirty(p))
            || (self
                .file_state
                .group
                .as_ref()
                .is_some_and(|p| p.starts_with(root))
                && self.group_dirty())
        {
            return Some(
                "Save or discard unsaved document, project and editor changes before pulling.",
            );
        }
        if self.pending.is_some()
            || self.annotation.is_some()
            || self.design_sync.is_some()
            || self.library_update.is_some()
            || self.sheet_sync.busy()
            || self.file_state.modal()
            || self.library_editor.modal()
            || self.variants.dialog.is_some()
            || self.layer_stack.is_some()
            || self.pad_edit.is_some()
            || self.git.push_confirm
        {
            return Some("Close editor dialogs and Layer Stack Manager before pulling.");
        }
        None
    }

    /// Called before any editor input, including shortcuts, imports and pours.
    /// A request is checked on the next frame, after the requesting frame's edits.
    pub(in crate::app) fn poll_git_pull(&mut self, ctx: &egui::Context) -> bool {
        let busy = self.poll_git_pull_job(ctx);
        if !busy && std::mem::take(&mut self.git.pull_close_after) {
            self.command = Some(Command::Quit);
        }
        busy
    }

    fn poll_git_pull_job(&mut self, ctx: &egui::Context) -> bool {
        if let Some(repo) = self.git.pull_request.take() {
            if let Some(reason) = self.git_pull_blocker(&repo.root) {
                self.git.feedback = Some((true, reason.into()));
                return false;
            }
            let Some(head) = self.git.snapshot.as_ref().and_then(|s| s.head.clone()) else {
                self.git.feedback = Some((true, "Refresh the repository before pulling.".into()));
                return false;
            };
            let documents: Vec<_> = self
                .documents
                .iter()
                .filter(|d| !d.read_only)
                .filter_map(|d| Some((d.session_id, d.path.as_ref()?.clone())))
                .filter(|(_, p)| p.starts_with(&repo.root))
                .collect();
            let projects: Vec<_> = self
                .projects
                .iter()
                .map(|p| p.path.clone())
                .filter(|p| p.starts_with(&repo.root))
                .collect();
            let symbols: Vec<_> = self
                .components_ui
                .libraries
                .iter()
                .map(|(p, _)| p.clone())
                .filter(|p| p.starts_with(&repo.root))
                .collect();
            let footprints: Vec<_> = self
                .components_ui
                .footprint_libraries
                .iter()
                .map(|(p, _)| p.clone())
                .filter(|p| p.starts_with(&repo.root))
                .collect();
            let group = self.file_state.group.clone();
            let (sender, receiver) = mpsc::channel();
            self.git.pull_job = Some(Job { receiver });
            self.git.feedback = None;
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = repo.pull(&head);
                let snapshot = repo.snapshot("HEAD", 100);
                let changed: BTreeSet<_> = result
                    .as_ref()
                    .map(|paths| paths.iter().map(|p| repo.root.join(p)).collect())
                    .unwrap_or_default();
                // Even a post-merge hook failure may follow a successful update.
                let uncertain = result.is_err()
                    && !snapshot
                        .as_ref()
                        .is_ok_and(|s| s.head.as_deref() == Some(&head));
                let affected = |p: &PathBuf| uncertain || changed.contains(p);
                let group_changed = group.as_ref().is_some_and(affected);
                let documents = documents
                    .into_iter()
                    .filter(|(_, p)| affected(p))
                    .map(|(id, path)| {
                        let result =
                            eda_io::load(&path).map(document_loader::PreparedDocument::new);
                        (id, Reload { path, result })
                    })
                    .collect();
                let projects = projects
                    .into_iter()
                    .filter(|_| uncertain || !changed.is_empty())
                    .map(|path| {
                        let result = eda_io::altium::load_project(&path);
                        Reload { path, result }
                    })
                    .collect();
                let symbols = symbols
                    .into_iter()
                    .filter(affected)
                    .map(|path| {
                        let result = eda_io::altium::library::load(&path);
                        Reload { path, result }
                    })
                    .collect();
                let footprints = footprints
                    .into_iter()
                    .filter(affected)
                    .map(|path| {
                        let result = eda_io::altium_pcb::library::load(&path);
                        Reload { path, result }
                    })
                    .collect();
                let _ = sender.send(Outcome {
                    result,
                    snapshot,
                    documents,
                    projects,
                    symbols,
                    footprints,
                    group_changed,
                });
                ctx.request_repaint();
            });
        }
        let Some(job) = &self.git.pull_job else {
            return false;
        };
        match job.receiver.try_recv() {
            Ok(outcome) => {
                self.git.pull_job = None;
                self.finish_git_pull(outcome);
                false
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.git.pull_job = None;
                // Git's outcome is unknown. Do not allow a stale editor to save over it.
                self.git.feedback = Some((true, "Pull worker stopped. Check repository files in Git and reopen the project before editing.".into()));
                for d in &mut self.documents {
                    if d.path.as_ref().is_some_and(|p| {
                        self.git
                            .snapshot
                            .as_ref()
                            .is_some_and(|s| p.starts_with(&s.repository.root))
                    }) {
                        d.read_only = true;
                    }
                }
                self.message(true, "Pull worker stopped. Check repository files in Git and reopen the project before editing.");
                false
            }
            Err(mpsc::TryRecvError::Empty) => {
                if ctx.input(|i| i.viewport().close_requested()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.git.pull_close_after = true;
                }
                ctx.request_repaint_after(Duration::from_millis(60));
                true
            }
        }
    }

    fn finish_git_pull(&mut self, outcome: Outcome) {
        let active_id = self
            .active
            .and_then(|i| self.documents.get(i))
            .map(|d| d.session_id);
        let repository_root = outcome
            .snapshot
            .as_ref()
            .ok()
            .or(self.git.snapshot.as_ref())
            .map(|s| s.repository.root.clone());
        let mut errors = vec![];
        if outcome.group_changed {
            // Keep the current workspace, but never save its stale membership
            // over the updated group file without an explicit reopen/Save As.
            self.file_state.group = None;
            self.file_state.saved_projects.clear();
            errors.push("The project group changed on disk. Reopen its DsnWrk file to load the updated project list.".into());
        }
        for (id, reload) in outcome.documents {
            match reload.result {
                Ok(prepared) => {
                    if let Some(index) = self.documents.iter().position(|d| d.session_id == id) {
                        self.property_ui.schematic.close_document(id);
                        let mut opened = OpenDocument::with_saved(
                            prepared.model,
                            Some(reload.path.clone()),
                            Some(prepared.saved),
                        );
                        opened.session_id = id;
                        opened.source_path = self.documents[index].source_path.clone();
                        opened.new_project = self.documents[index].new_project.clone();
                        opened.view.grid = self.settings.show_grid;
                        opened.view.grid_nm = self.settings.grid_nm;
                        opened.view.sch_opening = opened.model.schematic.is_some();
                        if let Some(schematic) = &opened.model.schematic {
                            opened.view.grid = schematic.sheet.grid_visible;
                            opened.view.grid_nm = schematic.sheet.grid_nm;
                            opened.view.snap.distance_nm = schematic.sheet.snap_nm;
                        }
                        if let Some(settings) = self.settings.pcb_drc.get(&reload.path) {
                            opened.pcb_editor.restore_rule_settings(settings.clone());
                        }
                        self.documents[index] = opened;
                        if self.documents[index].model.library.is_some() {
                            self.publish_authored_library(index, false);
                        }
                    }
                }
                Err(error) => {
                    // Deleted/invalid files must not remain writable stale tabs.
                    self.remove_document(id);
                    errors.push(format!("{}: {error}", reload.path.display()));
                }
            }
        }
        for reload in outcome.projects {
            self.variants.project_reloaded(&reload.path);
            match reload.result {
                Ok(project) => {
                    if let Some(slot) = self.projects.iter_mut().find(|p| p.path == reload.path) {
                        *slot = project;
                    }
                }
                Err(error) => {
                    self.projects.retain(|p| p.path != reload.path);
                    errors.push(format!("{}: {error}", reload.path.display()));
                }
            }
        }
        if active_id.is_some_and(|id| !self.documents.iter().any(|d| d.session_id == id)) {
            // Removing the active tab must not switch Git to an unrelated document.
            // If no repository tab survives, Home retains the current Git browser.
            self.active = repository_root.as_ref().and_then(|root| {
                self.documents.iter().position(|d| {
                    d.path
                        .as_ref()
                        .or(d.source_path.as_ref())
                        .is_some_and(|p| p.starts_with(root))
                        || self
                            .git
                            .reviews
                            .get(&d.session_id)
                            .is_some_and(|r| &r.repository == root)
                })
            });
        }
        for reload in outcome.symbols {
            match reload.result {
                Ok(library) => {
                    if let Some((_, slot)) = self
                        .components_ui
                        .libraries
                        .iter_mut()
                        .find(|(p, _)| *p == reload.path)
                    {
                        *slot = library;
                    }
                }
                Err(error) => {
                    self.components_ui
                        .libraries
                        .retain(|(p, _)| *p != reload.path);
                    errors.push(format!("{}: {error}", reload.path.display()));
                }
            }
        }
        for reload in outcome.footprints {
            match reload.result {
                Ok(library) => {
                    if let Some((_, slot)) = self
                        .components_ui
                        .footprint_libraries
                        .iter_mut()
                        .find(|(p, _)| *p == reload.path)
                    {
                        *slot = library;
                    }
                }
                Err(error) => {
                    self.components_ui
                        .footprint_libraries
                        .retain(|(p, _)| *p != reload.path);
                    errors.push(format!("{}: {error}", reload.path.display()));
                }
            }
        }
        self.variants.cache = None;
        self.git.reference = "HEAD".into();
        self.git.history_file = None;
        self.git.commit = None;
        self.git.files.clear();
        self.git.selected.clear();
        let feedback = match outcome.result {
            Ok(paths) if paths.is_empty() => (false, "Already up to date.".into()),
            Ok(_) => (
                false,
                "Pull complete. Project files and open documents updated.".into(),
            ),
            Err(error) => (true, error),
        };
        match outcome.snapshot {
            Ok(snapshot) => self.git.snapshot = Some(snapshot),
            Err(error) => errors.push(format!("Repository refresh failed: {error}")),
        }
        let feedback = if errors.is_empty() {
            feedback
        } else {
            (
                true,
                format!(
                    "{}\nReload errors (reopen affected files):\n{}",
                    feedback.1,
                    errors.join("\n")
                ),
            )
        };
        // Browser refresh/context changes may clear local feedback. Application
        // errors persist in Messages, which is opened even if the active tab closed.
        if feedback.0 {
            self.message(true, feedback.1.clone());
        }
        self.git.feedback = Some(feedback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        app: &mut PcbCraft,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 900.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                assert!(app.git_pull_frame(ctx));
            },
        )
    }

    fn painted_text(output: &egui::FullOutput) -> Vec<(String, egui::Pos2)> {
        fn collect(shape: &egui::Shape, result: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(text) => result.push((
                    text.galley.job.text.clone(),
                    text.pos + text.galley.size() * 0.5,
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        collect(shape, result);
                    }
                }
                _ => {}
            }
        }
        let mut result = vec![];
        for shape in &output.shapes {
            collect(&shape.shape, &mut result);
        }
        result
    }

    fn failed_outcome() -> Outcome {
        Outcome {
            result: Err("Simulated local refusal".into()),
            snapshot: Err("Simulated unavailable snapshot".into()),
            documents: vec![],
            projects: vec![],
            symbols: vec![],
            footprints: vec![],
            group_changed: false,
        }
    }

    #[test]
    fn busy_pull_keeps_chrome_docks_tabs_canvas_and_minimize_control() {
        let mut app = crate::app::schematic_editor::tests::app();
        let original = app.documents[0].model.clone();
        let revision = app.documents[0].history.revision;
        app.show_panel(Panel::Git);
        app.show_panel(Panel::Properties);
        let (_sender, receiver) = mpsc::channel();
        app.git.pull_job = Some(Job { receiver });
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut output = frame(&mut app, &ctx, vec![]);
        for _ in 0..3 {
            output = frame(&mut app, &ctx, vec![]);
        }
        let text = painted_text(&output);
        for expected in [
            "File",
            "Git",
            "Properties",
            "Interaction test",
            "Pulling changes…",
            "Minimize",
            "Close after Pull",
        ] {
            assert!(
                text.iter().any(|(text, _)| text.contains(expected)),
                "Missing {expected}"
            );
        }
        app.run(Command::NewBoard, &ctx);
        app.run(Command::Close, &ctx);
        app.run(Command::SchEdit(SchAction::Delete), &ctx);
        app.shortcuts(&ctx);
        let position = text.iter().find(|(text, _)| text == "Minimize").unwrap().1;
        for pressed in [true, false] {
            output = frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::Minimized(true)))
        );
        assert_eq!(app.documents.len(), 1);
        assert_eq!(app.documents[0].model, original);
        assert_eq!(app.documents[0].history.revision, revision);
        assert!(app.pending.is_none());
        assert!(!app.allow_exit);
    }

    #[test]
    fn busy_pull_defers_native_close_until_normal_unsaved_work_prompt() {
        let mut app = crate::app::schematic_editor::tests::app();
        let original = app.documents[0].model.clone();
        let (sender, receiver) = mpsc::channel();
        app.git.pull_job = Some(Job { receiver });
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .events
            .push(egui::ViewportEvent::Close);
        let output = ctx.run(input, |ctx| {
            assert!(app.poll_git_pull(ctx));
        });
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
        );
        assert!(app.git.pull_close_after);
        assert!(app.command.is_none());
        assert!(!app.allow_exit);
        assert!(sender.send(failed_outcome()).is_ok());
        assert!(!app.poll_git_pull(&ctx));
        assert!(matches!(app.command, Some(Command::Quit)));
        let command = app.command.take().unwrap();
        app.run(command, &ctx);
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.allow_exit);
        assert_eq!(app.documents[0].model, original);
    }

    #[test]
    fn unreadable_active_tab_falls_back_within_repository_and_reports_global_error() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("working");
        std::fs::create_dir(&root).unwrap();
        let mut app = crate::app::schematic_editor::tests::app();
        app.documents[0].path = Some(temp.path().join("Controller.pcbcraft.json"));
        let unrelated = app.documents[0].model.clone();
        for name in ["Main.SchDoc", "Extra.SchDoc"] {
            app.documents.push(OpenDocument::new(
                Document::new(DocumentKind::Schematic, name),
                Some(root.join(name)),
            ));
        }
        app.active = Some(2);
        let removed_id = app.documents[2].session_id;
        app.git.directory = Some(root.clone());
        let snapshot = Snapshot {
            repository: Repository {
                root: root.clone(),
                scope: String::new(),
            },
            branch: "main".into(),
            upstream: Some("origin/main".into()),
            ahead: 0,
            behind: 0,
            head: Some("a".repeat(40)),
            files: vec![],
            commits: vec![],
            refs: vec![],
            remotes: vec!["origin".into()],
            operation: None,
        };
        app.finish_git_pull(Outcome {
            result: Ok(vec!["Extra.SchDoc".into()]),
            snapshot: Ok(snapshot),
            documents: vec![(
                removed_id,
                Reload {
                    path: root.join("Extra.SchDoc"),
                    result: Err("Invalid native schematic bytes".into()),
                },
            )],
            projects: vec![],
            symbols: vec![],
            footprints: vec![],
            group_changed: false,
        });
        assert_eq!(app.active, Some(1));
        assert_eq!(app.documents[0].model, unrelated);
        assert!(app.documents[0].dirty());
        assert!(!app.documents.iter().any(|d| d.session_id == removed_id));
        assert!(app.panel_open(Panel::Messages));
        let notice = app
            .log
            .iter()
            .find(|(error, text)| *error && text.contains("Extra.SchDoc"))
            .unwrap()
            .1
            .clone();
        assert!(notice.contains("Invalid native schematic bytes"));
        let ctx = egui::Context::default();
        app.active = Some(0);
        app.git.activate(&ctx, Some(temp.path().to_path_buf()));
        app.git.activate(&ctx, Some(root));
        assert!(
            app.git.feedback.is_none(),
            "Browser refresh should exercise feedback clearing"
        );
        assert!(
            app.log
                .iter()
                .any(|(error, text)| *error && text == &notice)
        );
    }

    #[test]
    fn pull_worker_fetches_native_document_and_installs_clean_import() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("working");
        let peer = temp.path().join("peer");
        let remote = temp.path().join("remote.git");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&remote).unwrap();
        let git = |path: &std::path::Path, args: &[&str]| {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&root, &["init", "-b", "main"]);
        git(&root, &["config", "user.name", "Pull test"]);
        git(&root, &["config", "user.email", "qa@example.invalid"]);
        git(&remote, &["init", "--bare"]);
        let mut native = eda_io::altium::new_schematic("test.SchDoc").unwrap();
        let path = root.join("test.SchDoc");
        std::fs::write(&path, eda_io::altium::writer::serialize(&native).unwrap()).unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-m", "Initial"]);
        git(
            &root,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        git(&root, &["push", "-u", "origin", "main"]);
        git(
            temp.path(),
            &[
                "clone",
                "--branch",
                "main",
                remote.to_str().unwrap(),
                peer.to_str().unwrap(),
            ],
        );
        git(&peer, &["config", "user.name", "Pull test"]);
        git(&peer, &["config", "user.email", "qa@example.invalid"]);
        native
            .schematic
            .as_mut()
            .unwrap()
            .sheet
            .parameters
            .insert("TITLE".into(), "Pulled title".into());
        std::fs::write(
            peer.join("test.SchDoc"),
            eda_io::altium::writer::serialize(&native).unwrap(),
        )
        .unwrap();
        git(&peer, &["commit", "-am", "Remote title"]);
        git(&peer, &["push"]);

        let mut app = crate::app::schematic_editor::tests::app();
        let path = path.canonicalize().unwrap();
        app.documents[0] = OpenDocument::new(eda_io::load(&path).unwrap(), Some(path));
        let id = app.documents[0].session_id;
        let repo = Repository::discover(&root).unwrap();
        app.git.snapshot = Some(repo.snapshot("HEAD", 100).unwrap());
        app.git.pull_request = Some(repo);
        let ctx = egui::Context::default();
        let started = std::time::Instant::now();
        loop {
            let mut busy = false;
            let _ = ctx.run(Default::default(), |ctx| busy = app.poll_git_pull(ctx));
            if !busy {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "Pull worker timed out"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !app.git.feedback.as_ref().unwrap().0,
            "{:?}",
            app.git.feedback
        );
        assert_eq!(app.documents[0].session_id, id);
        assert!(!app.documents[0].dirty());
        assert_eq!(
            app.documents[0]
                .model
                .schematic
                .as_ref()
                .unwrap()
                .sheet
                .parameters["TITLE"],
            "Pulled title"
        );
        assert_eq!(app.git.snapshot.as_ref().unwrap().behind, 0);
    }

    #[test]
    fn pull_rechecks_unsaved_editor_content_after_requesting_frame() {
        let mut app = crate::app::schematic_editor::tests::app();
        let root = PathBuf::from("pull-test");
        app.documents[0].path = Some(root.join("test.SchDoc"));
        app.documents[0].mark_saved();
        assert!(app.git_pull_blocker(&root).is_none());
        app.documents[0].model.name = "Unsaved edit".into();
        app.documents[0].history.revision += 1;
        app.git.pull_request = Some(Repository {
            root,
            scope: String::new(),
        });
        assert!(!app.poll_git_pull(&egui::Context::default()));
        assert!(app.git.pull_job.is_none());
        assert!(
            app.git
                .feedback
                .as_ref()
                .unwrap()
                .1
                .contains("Save or discard")
        );
        assert_eq!(app.documents[0].model.name, "Unsaved edit");
    }

    #[test]
    fn pull_reloads_working_tab_resets_history_and_keeps_review_snapshot() {
        let mut app = crate::app::schematic_editor::tests::app();
        let path = PathBuf::from("pull-test/test.SchDoc");
        app.documents[0].path = Some(path.clone());
        app.documents[0].mark_saved();
        app.documents[0].history.revision = 5;
        let id = app.documents[0].session_id;
        let old = app.documents[0].model.clone();
        let mut review = OpenDocument::new(old.clone(), None);
        review.read_only = true;
        let review_id = review.session_id;
        app.documents.push(review);
        let mut updated = old.clone();
        updated.name = "Updated upstream".into();
        app.finish_git_pull(Outcome {
            result: Ok(vec!["test.SchDoc".into()]),
            snapshot: Err("test snapshot unavailable".into()),
            documents: vec![(
                id,
                Reload {
                    path,
                    result: Ok(document_loader::PreparedDocument::new(updated)),
                },
            )],
            projects: vec![],
            symbols: vec![],
            footprints: vec![],
            group_changed: false,
        });
        assert_eq!(app.documents[0].session_id, id);
        assert_eq!(app.documents[0].model.name, "Updated upstream");
        assert_eq!(app.documents[0].history.revision, 0);
        assert!(!app.documents[0].dirty());
        assert_eq!(app.documents[1].session_id, review_id);
        assert_eq!(app.documents[1].model, old);
        assert!(app.documents[1].read_only);
    }

    #[test]
    fn pull_closes_deleted_or_unreadable_files_instead_of_leaving_stale_writable_tabs() {
        let mut app = crate::app::schematic_editor::tests::app();
        let id = app.documents[0].session_id;
        app.finish_git_pull(Outcome {
            result: Ok(vec!["deleted.SchDoc".into()]),
            snapshot: Err("test snapshot unavailable".into()),
            documents: vec![(
                id,
                Reload {
                    path: PathBuf::from("deleted.SchDoc"),
                    result: Err("File deleted".into()),
                },
            )],
            projects: vec![],
            symbols: vec![],
            footprints: vec![],
            group_changed: true,
        });
        assert!(app.documents.is_empty());
        assert!(app.active.is_none());
        assert!(app.file_state.group.is_none());
        assert!(app.git.feedback.as_ref().unwrap().0);
        assert!(
            app.git
                .feedback
                .as_ref()
                .unwrap()
                .1
                .contains("File deleted")
        );
    }
}
