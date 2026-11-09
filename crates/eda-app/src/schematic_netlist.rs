//! The observed File / Export / Netlist Schematic flow and shared sheet snapshots.
use super::*;
use eda_core::design_sync::ProjectOptions;
use std::path::Path;

pub(super) struct Dialog {
    path: PathBuf,
    project: bool,
    format: usize,
    error: String,
}
const FORMATS: [&str; 20] = [
    "Cadnetix netlist",
    "Calay netlist",
    "EDIF Netlist",
    "EESof netlist",
    "Intergraph netlist",
    "Mentor BoardStation netlist",
    "MultiWire Netlist",
    "OrCad/PCB2 netlist",
    "PADS ASCII netlist",
    "Pcad Netlist",
    "PCAD netlist",
    "PCADnlt netlist",
    "Protel Netlist",
    "Protel2 netlist",
    "Racal netlist",
    "RINF netlist",
    "SciCards netlist",
    "Tango netlist",
    "Telesis Netlist",
    "WireList netlist",
];
pub(super) fn export_destination(path: &Path, extension: &str) -> Result<(), String> {
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(extension))
    {
        return Err(format!("Choose a .{extension} destination for this export"));
    }
    Ok(())
}
impl PcbCraft {
    /// Prefer current unsaved models over disk, without opening extra editor tabs.
    pub(super) fn schematic_project_snapshots(&self) -> Result<Vec<(PathBuf, Document)>, String> {
        let Some(index) = self.file_project_index() else {
            return Ok(vec![]);
        };
        let project = &self.projects[index];
        let mut result = vec![];
        for entry in project
            .documents
            .iter()
            .filter(|e| e.kind == "schdoc" || e.kind == "json")
        {
            let loaded = self.documents.iter().find(|d| {
                d.path.as_ref() == Some(&entry.path) || d.source_path.as_ref() == Some(&entry.path)
            });
            let doc = if let Some(open) = loaded {
                open.model.clone()
            } else {
                eda_io::load(&entry.path)?
            };
            if doc.schematic.is_some() {
                result.push((entry.path.clone(), doc));
            }
        }
        for open in self.documents.iter().filter(|d| {
            d.new_project.as_ref() == Some(&project.path) && d.model.schematic.is_some()
        }) {
            let path = open
                .path
                .clone()
                .unwrap_or_else(|| project.path.with_file_name(&open.model.name));
            if !result.iter().any(|(_, doc)| doc.id == open.model.id) {
                result.push((path, open.model.clone()));
            }
        }
        Ok(result)
    }
    pub(super) fn open_netlist_export(&mut self) {
        let Some(index) = self.active else { return };
        let name = Path::new(&self.documents[index].model.name).with_extension("NET");
        if let Some(path) = self
            .file_dialog
            .clone()
            .set_title("Export File")
            .add_filter("NetList Files", &["NET"])
            .set_file_name(name.to_string_lossy())
            .save_file()
        {
            self.file_state.netlist = Some(Dialog {
                path,
                project: self.file_project_index().is_some(),
                format: 12,
                error: String::new(),
            });
        }
    }
    fn export_netlist_to(&self, d: &Dialog) -> Result<Vec<String>, String> {
        export_destination(&d.path, "NET")?;
        if d.format != 12 {
            return Err("This netlist format is not implemented".into());
        }
        if d.project {
            let index = self
                .file_project_index()
                .ok_or("The project is no longer open")?;
            let snapshots = self.schematic_project_snapshots()?;
            let sheets: Vec<_> = snapshots.iter().map(|(p, d)| (p.as_path(), d)).collect();
            let options = self.projects[index].compile_options(&sheets)?;
            let docs: Vec<_> = sheets.iter().map(|(_, d)| *d).collect();
            eda_io::schematic_netlist::save_protel(&d.path, &docs, &options)
        } else {
            let doc = &self.documents[self.active.ok_or("No active schematic")?].model;
            eda_io::schematic_netlist::save_protel(&d.path, &[doc], &ProjectOptions::default())
        }
    }
    pub(super) fn netlist_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut d) = self.file_state.netlist.take() else {
            return;
        };
        let mut keep = true;
        let mut export = false;
        egui::Modal::new(egui::Id::new("export_netlist")).show_stable(ctx, 341., |ui| {
            // AD 24.4.1: 349 x 221 outer pixels at this DPI, including the modal margin.
            let (area, _) = ui.allocate_exact_size(egui::vec2(341., 213.), egui::Sense::hover());
            let at = |x, y| area.min + egui::vec2(x, y);
            let rect = |x, y, w, h| egui::Rect::from_min_size(at(x, y), egui::vec2(w, h));
            let font = egui::FontId::proportional(12.);
            let foreground = ui.visuals().text_color();
            ui.painter().text(
                at(170.5, 10.5),
                egui::Align2::CENTER_CENTER,
                "Export NetList",
                font.clone(),
                foreground,
            );
            let close_rect = rect(307., -3., 35., 20.);
            let close = ui.interact(
                close_rect,
                ui.id().with("netlist_close"),
                egui::Sense::click(),
            );
            if close.hovered() {
                ui.painter()
                    .rect_filled(close_rect, 0., Color32::from_rgb(170, 20, 20));
            }
            ui.painter().text(
                close_rect.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                font.clone(),
                foreground,
            );
            if close.clicked() {
                keep = false;
            }
            ui.painter().rect_stroke(
                rect(4., 26., 333., 151.),
                0.,
                egui::Stroke::new(1., Color32::from_gray(58)),
                egui::StrokeKind::Inside,
            );
            ui.painter()
                .rect_filled(rect(28., 51., 285., 21.), 0., Color32::from_gray(78));
            ui.painter().text(
                at(33., 61.5),
                egui::Align2::LEFT_CENTER,
                "Scope",
                font.clone(),
                foreground,
            );
            let mut project_ui =
                ui.new_child(egui::UiBuilder::new().max_rect(rect(41., 82., 94., 28.)));
            project_ui.add_enabled_ui(self.file_project_index().is_some(), |ui| {
                if ui
                    .add(egui::RadioButton::new(d.project, "Project"))
                    .clicked()
                {
                    d.project = true;
                }
            });
            let mut document_ui =
                ui.new_child(egui::UiBuilder::new().max_rect(rect(173., 82., 110., 28.)));
            if document_ui
                .add(egui::RadioButton::new(!d.project, "Document"))
                .clicked()
            {
                d.project = false;
            }
            ui.painter().text(
                at(28., 140.),
                egui::Align2::LEFT_CENTER,
                "Netlist format",
                font,
                foreground,
            );
            let mut format_ui =
                ui.new_child(egui::UiBuilder::new().max_rect(rect(139., 130., 174., 22.)));
            egui::ComboBox::from_id_salt("netlist_format")
                .width(174.)
                .selected_text(FORMATS[d.format])
                .show_ui(&mut format_ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.;
                    for (index, label) in FORMATS.iter().enumerate() {
                        ui.add_enabled_ui(index == 12, |ui| {
                            ui.selectable_value(&mut d.format, index, *label);
                        });
                    }
                });
            if ui
                .put(rect(170., 184., 75., 24.), egui::Button::new("OK"))
                .clicked()
            {
                export = true;
            }
            if ui
                .put(rect(252., 184., 75., 24.), egui::Button::new("Cancel"))
                .clicked()
            {
                keep = false;
            }
            if !d.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &d.error);
            }
        });
        if !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            keep = false;
        }
        if export {
            match self.export_netlist_to(&d) {
                Ok(warnings) => {
                    self.message(false, format!("Exported {}", d.path.display()));
                    for warning in warnings {
                        self.message(false, warning);
                    }
                    keep = false;
                }
                Err(error) => d.error = error,
            }
        }
        if keep {
            self.file_state.netlist = Some(d);
        }
    }
}

impl PcbCraft {
    pub(super) fn highlight_schematic_net(&mut self, index: usize, hit: Option<Uuid>) {
        let active_id = self.documents[index].model.id;
        let project = self.file_project_index();
        let result = (|| {
            let snapshots = if project.is_some() {
                self.schematic_project_snapshots()?
            } else {
                vec![]
            };
            let sheets: Vec<_> = snapshots.iter().map(|(p, d)| (p.as_path(), d)).collect();
            let mut options = if let Some(p) = project {
                self.projects[p].compile_options(&sheets)?
            } else {
                ProjectOptions::default()
            };
            let docs: Vec<_> = if sheets.is_empty() {
                vec![&self.documents[index].model]
            } else {
                sheets.iter().map(|(_, d)| *d).collect()
            };
            options.report_label_conflicts = true;
            options.review_annotation = true;
            let compiled = eda_core::design_sync::compile_project(&docs, &options)?;
            let objects = hit
                .and_then(|id| {
                    compiled
                        .electrical_islands
                        .into_iter()
                        .find(|island| island.contains(&(active_id, id)))
                })
                .unwrap_or_default();
            Ok::<_, String>(objects)
        })();
        match result {
            Ok(objects) => {
                for doc in &mut self.documents {
                    doc.view.sch_highlight = objects
                        .iter()
                        .filter(|(sheet, _)| *sheet == doc.model.id)
                        .map(|(_, id)| *id)
                        .collect();
                }
                let view = &mut self.documents[index].view;
                view.sch_selection = hit.into_iter().collect();
                view.selected = hit;
                view.sch_segments.clear();
            }
            Err(e) => self.message(true, e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_dxf_contains_component_graphics_and_outlined_fonts() {
        let doc = eda_io::altium::import_schematic(
            include_bytes!("../../../fixtures/schematic/minimal.SchDoc"),
            "DxfReference.SchDoc",
        )
        .unwrap();
        let before = doc.clone();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let faces = eda_render::schematic_vector::drawing(&ctx, &doc).unwrap();
        assert!(faces.len() > 100);
        let data = eda_io::schematic_dxf::export_drawing(&faces).unwrap();
        assert_eq!(data.matches("0\nSOLID\n").count(), faces.len());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drawing.dxf");
        std::fs::write(&path, &data).unwrap();
        let fragment = eda_io::schematic_dxf::import(&path).unwrap();
        assert_eq!(fragment.primitives.len(), faces.len());
        assert!(
            fragment
                .primitives
                .iter()
                .all(|p| p.fill.is_some() && p.width_nm == 0)
        );
        assert_eq!(doc, before);
        if let Some(path) = std::env::var_os("PCBCRAFT_FILE_QA_DXF") {
            std::fs::write(&path, data).unwrap();
            let raster_ctx = egui::Context::default();
            theme::apply(&raster_ctx);
            let raster =
                eda_render::schematic_output::raster(&raster_ctx, &doc, 2400, 1590).unwrap();
            image::RgbImage::from_raw(raster.width as u32, raster.height as u32, raster.rgb)
                .unwrap()
                .save(Path::new(&path).with_extension("png"))
                .unwrap();
        }
    }
    #[test]
    fn project_export_uses_unsaved_scene_and_does_not_open_tabs() {
        let dir = tempfile::tempdir().unwrap();
        let project =
            eda_io::project_files::create_project(&dir.path().join("Circuit.PrjPcb")).unwrap();
        let path = dir.path().join("Circuit.SchDoc");
        let doc = eda_io::altium::new_schematic("Circuit.SchDoc").unwrap();
        eda_io::save(&path, &doc).unwrap();
        let project = eda_io::project_files::add_document(&project, &path).unwrap();
        let mut app = super::super::schematic_editor::tests::app();
        app.projects.push(project);
        app.open_path(path.clone());
        let index = app.active.unwrap();
        let fragment = eda_core::schematic_edit::place(
            Placement::Resistor,
            eda_geom::Point::mm(20, 20),
            0,
            "R7",
            "2k",
            0,
        )
        .unwrap();
        let open = &mut app.documents[index];
        open.edit_schematic(
            eda_core::schematic_edit::Edit::Insert(fragment),
            open.history.revision,
        )
        .unwrap();
        let count = app.documents.len();
        let output = dir.path().join("Circuit.NET");
        app.export_netlist_to(&Dialog {
            path: output.clone(),
            project: true,
            format: 12,
            error: String::new(),
        })
        .unwrap();
        let text = std::fs::read_to_string(output).unwrap();
        assert!(text.contains("\r\nR7\r\n"));
        assert_eq!(app.documents.len(), count);
        assert!(app.documents[index].dirty());
        assert!(
            eda_io::load(&path)
                .unwrap()
                .schematic
                .unwrap()
                .components
                .is_empty()
        );
        let mut unsaved = OpenDocument::new(
            eda_io::altium::new_schematic("Unsaved.SchDoc").unwrap(),
            None,
        );
        unsaved.new_project = Some(app.projects[0].path.clone());
        app.documents.push(unsaved);
        app.active = Some(app.documents.len() - 1);
        assert_eq!(app.file_project_index(), Some(0));
        assert_eq!(app.schematic_project_snapshots().unwrap().len(), 2);
    }
}
