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
        {
            return Some("Close editor dialogs and Layer Stack Manager before pulling.");
        }
        None
    }

    /// Called before any editor input, including shortcuts, imports and pours.
    /// A request is checked on the next frame, after the requesting frame's edits.
    pub(in crate::app) fn poll_git_pull(&mut self, ctx: &egui::Context) -> bool {
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
                false
            }
            Err(mpsc::TryRecvError::Empty) => {
                if ctx.input(|i| i.viewport().close_requested()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.);
                        ui.spinner();
                        ui.heading("Pulling changes…");
                        ui.label(
                            "Fetching upstream, updating files and reloading changed documents.",
                        );
                    });
                });
                ctx.request_repaint_after(Duration::from_millis(60));
                true
            }
        }
    }

    fn finish_git_pull(&mut self, outcome: Outcome) {
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
        self.git.feedback = Some(if errors.is_empty() {
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
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
