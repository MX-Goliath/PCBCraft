use super::*;

#[test]
fn validation_reads_closed_sheets_and_navigation_opens_only_its_target() {
    let mut app = schematic_editor::tests::app();
    let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Closed.json");
    let d = Document::new(DocumentKind::Schematic, "Closed.SchDoc");
    let fragment = eda_core::schematic_edit::place(
        Placement::Resistor,
        eda_geom::Point::mm(20, 20),
        0,
        "R1",
        "10k",
        0,
    )
    .unwrap();
    let mut d =
        eda_core::schematic_edit::preview(&d, &eda_core::schematic_edit::Edit::Insert(fragment))
            .unwrap();
    input_pins(&mut d);
    eda_io::save(&file, &d).unwrap();
    let project = dir.join("Test.PrjPcb");
    std::fs::write(
        &project,
        "[Design]\n[Document1]\nDocumentPath=Closed.json\n",
    )
    .unwrap();
    app.projects
        .push(eda_io::altium::load_project(&project).unwrap());
    app.documents[0].new_project = Some(project);
    let before = app.documents[0].model.clone();
    app.validate_schematic_project();
    assert_eq!(app.documents.len(), 1);
    let target = app
        .file_state
        .validation
        .rows
        .iter()
        .flat_map(|r| &r.targets)
        .find(|t| t.document == d.id)
        .unwrap()
        .clone();
    app.probe_validation_target(target.clone());
    assert_eq!(app.documents.len(), 2);
    assert_eq!(
        app.documents[app.active.unwrap()].view.selected,
        Some(target.object)
    );
    assert_eq!(app.documents[0].model, before);
    assert!(!app.documents[1].dirty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn validation_uses_unsaved_project_sheets_replaces_results_and_keeps_history_clean() {
    let mut app = schematic_editor::tests::app();
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/schematic/minimal.PrjPcb");
    let project = eda_io::altium::load_project(&root).unwrap();
    let path = project
        .documents
        .iter()
        .find(|e| e.kind == "schdoc")
        .unwrap()
        .path
        .clone();
    let mut doc = eda_io::load(&path).unwrap();
    doc.schematic.as_mut().unwrap().components[0].designator = "R?".into();
    app.documents = vec![OpenDocument::new(doc, Some(path))];
    app.projects = vec![project];
    app.active = Some(0);
    let before = app.documents[0].model.clone();
    let revision = app.documents[0].history.revision;
    app.validate_schematic_project();
    let count = app.file_state.validation.rows.len();
    assert!(
        app.file_state
            .validation
            .rows
            .iter()
            .any(|r| r.message.contains("unannotated designator"))
    );
    assert!(app.settings.right_messages);
    assert!(
        !app.file_state
            .validation
            .rows
            .iter()
            .any(|r| r.message == "Compile successful, no errors found.")
    );
    app.validate_schematic_project();
    assert_eq!(app.file_state.validation.rows.len(), count);
    app.next_validation_message(true);
    assert!(!app.documents[0].view.sch_selection.is_empty());
    assert_eq!(app.documents[0].model, before);
    assert_eq!(app.documents[0].history.revision, revision);
}
#[test]
fn broken_project_sheet_reports_fatal_without_success_or_opening_tabs() {
    let mut app = schematic_editor::tests::app();
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/schematic/minimal.PrjPcb");
    let mut project = eda_io::altium::load_project(&root).unwrap();
    project
        .documents
        .iter_mut()
        .find(|e| e.kind == "schdoc")
        .unwrap()
        .path = PathBuf::from("missing-validation-sheet.SchDoc");
    app.documents[0].new_project = Some(project.path.clone());
    app.projects = vec![project];
    let count = app.documents.len();
    app.validate_schematic_project();
    assert_eq!(app.documents.len(), count);
    assert!(
        app.file_state
            .validation
            .rows
            .iter()
            .any(|r| r.severity == Some(Severity::Fatal))
    );
    assert!(
        !app.file_state
            .validation
            .rows
            .iter()
            .any(|r| r.message == "Compile successful.")
    );
}
#[test]
fn warning_only_compilation_preserves_active_properties_and_message_navigation_wraps() {
    let mut app = schematic_editor::tests::app();
    let f = eda_core::schematic_edit::place(
        Placement::Resistor,
        eda_geom::Point::mm(20, 20),
        0,
        "R1",
        "10k",
        0,
    )
    .unwrap();
    app.documents[0].model = eda_core::schematic_edit::preview(
        &app.documents[0].model,
        &eda_core::schematic_edit::Edit::Insert(f),
    )
    .unwrap();
    input_pins(&mut app.documents[0].model);
    app.validate_schematic_project();
    assert!(!app.settings.right_messages);
    app.next_validation_message(false);
    let last = app.file_state.validation.selected;
    app.next_validation_message(true);
    app.next_validation_message(false);
    assert_eq!(app.file_state.validation.selected, last);
    let ctx = egui::Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| app.messages_panel(ui));
    });
}
fn input_pins(d: &mut Document) {
    for p in &mut d.schematic.as_mut().unwrap().primitives {
        if let eda_core::schematic::SchPrimitiveKind::Pin { electrical, .. } = &mut p.kind {
            *electrical = 0;
        }
    }
}

#[test]
fn later_application_messages_preserve_compiler_results_and_copy_selection() {
    let mut app = schematic_editor::tests::app();
    app.validate_schematic_project();
    let n = app.file_state.validation.rows.len();
    app.message(false, "Navigation complete");
    let state = &mut app.file_state.validation;
    assert!(state.has_run);
    assert_eq!(state.rows.len(), n + 1);
    assert!(
        state
            .rows
            .iter()
            .any(|r| r.message == "Compile successful, no errors found.")
    );
    state.selection.clear();
    state.selection.insert(n);
    let selected = state.text(true);
    assert!(selected.contains("WorkspaceManager\tNavigation complete"));
    assert!(!selected.contains("Compile successful"));
    assert_eq!(state.rows[n].number, n + 1);
}
