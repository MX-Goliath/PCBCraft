//! Project-scoped navigation between Sheet Symbols, entries and child ports.
use super::*;
use eda_core::schematic::SchPrimitiveKind as K;

impl PcbCraft {
    pub(super) fn hierarchy_click(
        &mut self,
        index: usize,
        input: &eda_render::SchematicInput,
        ctx: &egui::Context,
    ) -> bool {
        if self.documents[index].sch_editor.active()
            || !ctx.input(|i| i.modifiers.ctrl)
            || !input.clicked
        {
            return false;
        }
        let Some(id) = input.hit else { return false };
        if !self.documents[index]
            .model
            .schematic
            .as_ref()
            .and_then(|s| s.primitive(id))
            .is_some_and(|p| {
                matches!(
                    p.kind,
                    K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
                )
            })
        {
            return false;
        }
        if input.double_clicked {
            self.navigate_hierarchy(index, id);
        }
        true
    }
    pub(super) fn start_hierarchy_navigation(&mut self) {
        if let Some(doc) = self.active.map(|i| &mut self.documents[i])
            && doc.model.schematic.is_some()
        {
            doc.sch_editor.cancel(&mut doc.view);
            doc.sch_editor.hierarchy_pick = true;
            doc.view.sch_tool_active = true;
        }
    }
    pub(super) fn navigate_hierarchy(&mut self, index: usize, id: Uuid) -> bool {
        let Some(p) = self.documents[index]
            .model
            .schematic
            .as_ref()
            .and_then(|s| s.primitive(id))
            .cloned()
        else {
            return false;
        };
        if !matches!(
            p.kind,
            K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
        ) {
            return false;
        }
        let Some(path) = self.documents[index]
            .source_path
            .as_ref()
            .or(self.documents[index].path.as_ref())
            .cloned()
        else {
            self.message(true, "Open the schematic project to navigate its hierarchy");
            return true;
        };
        let Some(project) = self
            .projects
            .iter()
            .find(|project| project.documents.iter().any(|e| e.path == path))
            .cloned()
        else {
            self.message(true, "Open the schematic project to navigate its hierarchy");
            return true;
        };
        let scene = self.documents[index].model.schematic.as_ref().unwrap();
        let filename = match &p.kind {
            K::SheetSymbol { filename, .. } => Some(filename.as_str()),
            K::SheetEntry { .. } => p
                .presentation
                .parent
                .and_then(|id| scene.primitive(id))
                .and_then(|p| {
                    if let K::SheetSymbol { filename, .. } = &p.kind {
                        Some(filename.as_str())
                    } else {
                        None
                    }
                }),
            _ => None,
        };
        let paths = if let Some(filename) = filename {
            project
                .resolve_sheet(&path, filename)
                .filter(|e| e.exists)
                .map(|e| vec![e.path.clone()])
                .unwrap_or_default()
        } else {
            // Only load known parent sheets; unrelated project documents stay closed.
            project.documents.iter().filter(|e| e.kind == "schdoc" && e.exists).filter(|e| {
                if let Some(doc) = self.documents.iter().find(|d| d.source_path.as_ref().or(d.path.as_ref()) == Some(&e.path)) {
                    doc.model.schematic.as_ref().is_some_and(|s| s.primitives.iter().any(|p| {
                        matches!(&p.kind,K::SheetSymbol{filename,..} if project.resolve_sheet(&e.path,filename).is_some_and(|e|e.path == path))
                    }))
                } else {
                    project.sheet_children.get(&e.path).is_some_and(|children|children.contains(&path))
                }
            }).map(|e|e.path.clone()).collect()
        };
        {
            let doc = &mut self.documents[index];
            doc.sch_editor.cancel(&mut doc.view);
        }
        for path in &paths {
            if !self
                .documents
                .iter()
                .any(|d| d.source_path.as_ref().or(d.path.as_ref()) == Some(path))
            {
                self.open_path(path.clone());
            }
        }
        let mut targets = vec![];
        for (i, doc) in self.documents.iter().enumerate().filter(|(_, d)| {
            paths
                .iter()
                .any(|p| Some(p) == d.source_path.as_ref().or(d.path.as_ref()))
        }) {
            let Some(s) = doc.model.schematic.as_ref() else {
                continue;
            };
            match &p.kind {
                K::SheetSymbol { .. } => targets.push((i, None)),
                K::SheetEntry { name, .. } => {
                    for port in &s.primitives {
                        if matches!(&port.kind,K::Port{name:n,..} if n.eq_ignore_ascii_case(name)) {
                            targets.push((i, Some(port.id)));
                        }
                    }
                }
                K::Port { name, .. } => {
                    let owner = doc.source_path.as_ref().or(doc.path.as_ref()).unwrap();
                    for entry in &s.primitives {
                        if matches!(&entry.kind,K::SheetEntry{name:n,..} if n.eq_ignore_ascii_case(name))
                            && entry.presentation.parent.and_then(|id|s.primitive(id)).is_some_and(|p| {
                                matches!(&p.kind,K::SheetSymbol{filename,..} if project.resolve_sheet(owner,filename).is_some_and(|e|e.path == path))
                            }) { targets.push((i,Some(entry.id))); }
                    }
                }
                _ => {}
            }
        }
        if targets.len() != 1 {
            self.message(true,format!("Hierarchy navigation found {} matching targets; select an unambiguous port/entry",targets.len()));
            return true;
        }
        let (i, target) = targets[0];
        self.active = Some(i);
        let doc = &mut self.documents[i];
        doc.view.selected = target;
        doc.view.sch_selection = target.into_iter().collect();
        doc.view.sch_segments.clear();
        if let Some(at) = target.and_then(|id| {
            eda_core::schematic_edit::anchor(doc.model.schematic.as_ref().unwrap(), id)
        }) {
            doc.view.camera.centre_mm = [at.x as f64 / 1e6, at.y as f64 / 1e6];
            doc.view.camera.pixels_per_mm = doc.view.camera.pixels_per_mm.max(12.0);
            doc.view.camera.fitted = true;
        } else {
            doc.view.camera.fitted = false;
        }
        true
    }
}
