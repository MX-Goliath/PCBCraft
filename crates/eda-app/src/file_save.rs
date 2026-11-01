use super::*;
use std::path::Path;

impl PcbCraft {
    fn choose_document_destination(&self, index: usize, copy: bool) -> Option<PathBuf> {
        let doc = self.documents.get(index)?;
        let native = doc
            .model
            .schematic
            .as_ref()
            .is_some_and(|s| s.source.is_some());
        let mut dialog =
            self.file_dialog
                .clone()
                .set_title(if copy { "Save Copy As" } else { "Save As" });
        if doc.model.pcb.is_some() {
            dialog = dialog.add_filter("PCB Binary File", &["PcbDoc"]);
        } else if native {
            dialog = dialog.add_filter("Schematic Document", &["SchDoc"]);
        }
        dialog = dialog.add_filter("PCBCraft document", &["json"]);
        let filename = doc
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| doc.model.name.clone());
        let filename = if native
            && !filename.to_ascii_lowercase().ends_with(".schdoc")
            && doc.path.is_none()
        {
            format!("{}.SchDoc", filename.trim_end_matches(".pcbcraft.json"))
        } else {
            filename
        };
        if let Some(parent) = doc.path.as_ref().and_then(|p| p.parent()) {
            dialog = dialog.set_directory(parent);
        }
        dialog.set_file_name(filename).save_file()
    }
    pub(super) fn save_index(&mut self, index: usize, save_as: bool) -> bool {
        let path = if save_as || self.documents.get(index).is_none_or(|d| d.path.is_none()) {
            self.choose_document_destination(index, false)
        } else {
            self.documents[index].path.clone()
        };
        path.is_some_and(|path| self.save_document_to(index, &path, false))
    }
    pub(super) fn save_copy_index(&mut self, index: usize) -> bool {
        self.choose_document_destination(index, true)
            .is_some_and(|path| self.save_document_to(index, &path, true))
    }
    pub(super) fn save_document_to(&mut self, index: usize, path: &Path, copy: bool) -> bool {
        let Some(doc) = self.documents.get_mut(index) else {
            return false;
        };
        if doc.model.pcb.is_some()
            && let Err(e) = pcb_editor::prepare_save(doc)
        {
            self.message(true, e);
            return false;
        }
        if !path.extension().is_some_and(|e| {
            ["SchDoc", "PcbDoc", "json"]
                .iter()
                .any(|s| e.eq_ignore_ascii_case(s))
        }) {
            self.message(
                true,
                "Choose a .SchDoc, .PcbDoc or .pcbcraft.json destination",
            );
            return false;
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| {
            eda_io::project_files::absolute(
                &std::env::current_dir().unwrap_or_default(),
                &path.to_string_lossy(),
            )
        });
        if self.documents.iter().enumerate().any(|(i, d)| {
            (i != index || copy)
                && (d.path.as_ref() == Some(&canonical)
                    || d.source_path.as_ref() == Some(&canonical))
        }) {
            self.message(true,"That destination belongs to an open document. Choose a separate file for the copy.");
            return false;
        }
        let mut model = self.documents[index].model.clone();
        let old_path = self.documents[index].path.clone();
        model.name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        match eda_io::save(path, &model) {
            Ok(()) => {
                let mut project_errors = Vec::new();
                if !copy {
                    let doc = &mut self.documents[index];
                    // Already validated by the writer before the destination was replaced.
                    doc.history
                        .rename_file(&mut doc.model, model.name)
                        .expect("saved document validated");
                    doc.path = Some(path.canonicalize().unwrap_or(canonical));
                    if doc.source_path.is_some() {
                        doc.source_path = doc.path.clone();
                    }
                    doc.mark_saved();
                    if let Some(old) = old_path
                        && Some(&old) != doc.path.as_ref()
                    {
                        let new = doc.path.clone().unwrap();
                        for p in &mut self.projects {
                            if p.documents.iter().any(|e| e.path == old) {
                                match eda_io::project_files::replace_document(p, &old, &new) {
                                    Ok(updated) => *p = updated,
                                    Err(e) => {
                                        project_errors.push(format!("Document saved, project reference could not be updated: {e}"));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(project) = doc.new_project.clone()
                        && let Some(p) = self.projects.iter().position(|p| p.path == project)
                    {
                        match eda_io::project_files::add_document(&self.projects[p],self.documents[index].path.as_ref().unwrap()){
                            Ok(updated)=>{self.projects[p]=updated;self.documents[index].new_project=None;},
                            Err(e)=>self.message(true,format!("Document saved, but project reference could not be updated: {e}")),
                        }
                    }
                    self.recent(path.to_owned());
                }
                for error in project_errors {
                    self.message(true, error);
                }
                self.message(
                    false,
                    format!(
                        "{} {}",
                        if copy { "Saved copy" } else { "Saved" },
                        path.display()
                    ),
                );
                true
            }
            Err(e) => {
                self.message(true, format!("Could not save {}: {e}", path.display()));
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_sheet_save_and_save_as_update_native_project_references() {
        let mut app = super::super::schematic_editor::tests::app();
        let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let project = eda_io::project_files::create_project(&dir.join("design.PrjPcb")).unwrap();
        app.file_state.selected_project = Some(project.path.clone());
        app.projects.push(project);
        app.active = None;
        app.run(Command::NewSchematic, &egui::Context::default());
        let i = app.active.unwrap();
        let first = dir.join("First.SchDoc");
        assert!(app.save_document_to(i, &first, false), "{:?}", app.log);
        assert_eq!(
            app.projects[0].documents[0].path,
            first.canonicalize().unwrap()
        );
        let second = dir.join("Second.pcbcraft.json");
        assert!(app.save_document_to(i, &second, false), "{:?}", app.log);
        assert_eq!(app.projects[0].documents.len(), 1);
        assert_eq!(
            app.projects[0].documents[0].path,
            second.canonicalize().unwrap()
        );
        assert_eq!(app.projects[0].documents[0].kind, "schdoc");
        assert!(first.is_file());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn authored_native_save_as_and_second_save_keep_source_archive_and_undo() {
        let mut app = super::super::schematic_editor::tests::app();
        app.documents[0] = OpenDocument::new(
            eda_io::altium::new_schematic("Sheet1.SchDoc").unwrap(),
            None,
        );
        let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Renamed.SchDoc");
        assert!(app.save_document_to(0, &path, false), "{:?}", app.log);
        assert!(!app.documents[0].dirty());
        let d = &mut app.documents[0];
        let mut sheet = d.model.schematic.as_ref().unwrap().sheet.clone();
        sheet.background = 0xffeedd;
        d.edit_schematic(
            eda_core::schematic_edit::Edit::SheetProperties {
                sheet: Box::new(sheet),
                size: d.model.size,
            },
            d.history.revision,
        )
        .unwrap();
        assert!(app.save_document_to(0, &path, false), "{:?}", app.log);
        assert_eq!(
            eda_io::load(&path)
                .unwrap()
                .schematic
                .unwrap()
                .sheet
                .background,
            0xffeedd
        );
        assert!(
            app.save_document_to(0, &dir.join("Copy.SchDoc"), true),
            "{:?}",
            app.log
        );
        let d = &mut app.documents[0];
        d.history.undo(&mut d.model);
        assert_eq!(d.model.name, "Renamed.SchDoc");
        assert!(d.dirty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn save_copy_preserves_dirty_identity_path_and_undo() {
        let mut app = super::super::schematic_editor::tests::app();
        let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let original = dir.join("original.json");
        let copy = dir.join("copy.json");
        assert!(app.save_document_to(0, &original, false));
        let d = &mut app.documents[0];
        let mut changed = d.model.clone();
        changed
            .schematic
            .as_mut()
            .unwrap()
            .sheet
            .parameters
            .insert("Title".into(), "Changed".into());
        d.history.commit(&mut d.model, changed).unwrap();
        let before = d.model.clone();
        let path = d.path.clone();
        let revision = d.history.revision;
        assert!(app.save_document_to(0, &copy, true));
        assert_eq!(app.documents[0].model, before);
        assert_eq!(app.documents[0].path, path);
        assert!(app.documents[0].dirty());
        assert_eq!(app.documents[0].history.revision, revision);
        assert!(!app.save_document_to(0, &original, true));
        assert_eq!(
            eda_io::load(&original)
                .unwrap()
                .schematic
                .unwrap()
                .sheet
                .parameters
                .get("Title"),
            None
        );
        assert!(app.save_document_to(0, &dir.join("renamed.json"), false));
        let name = app.documents[0].model.name.clone();
        let d = &mut app.documents[0];
        d.history.undo(&mut d.model);
        assert_eq!(d.model.name, name);
        assert!(d.dirty());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
