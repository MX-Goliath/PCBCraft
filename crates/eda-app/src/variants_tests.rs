use super::*;

fn fixture() -> (tempfile::TempDir, PcbCraft) {
    let dir = tempfile::tempdir().unwrap();
    let sch = dir.path().join("Main.SchDoc");
    let project = dir.path().join("Test.PrjPcb");
    let source = include_str!("../../../fixtures/schematic/minimal.SchDoc");
    std::fs::write(&sch, source.lines().take(8).collect::<Vec<_>>().join("\n")).unwrap();
    std::fs::write(&project,"[Design]\n[Document1]\nDocumentPath=Main.SchDoc\n[ProjectVariant1]\nUniqueID=ASSEMBLY_A\nDescription=Assembly A\nVariationCount=1\nVariation1=Designator=R1|UniqueId=\\RESISTOR|Kind=1|AlternatePart=\n").unwrap();
    let mut app = schematic_editor::tests::app();
    app.projects = vec![eda_io::altium::load_project(&project).unwrap()];
    app.documents = vec![OpenDocument::new(
        eda_io::altium::load_schematic(&sch).unwrap(),
        Some(sch.canonicalize().unwrap()),
    )];
    app.active = Some(0);
    (dir, app)
}
#[test]
fn physical_variant_view_is_cached_read_only_and_leaves_base_history_clean() {
    let (_dir, mut app) = fixture();
    let before = app.documents[0].model.clone();
    let p = app.projects[0].path.clone();
    app.select_variant(p, Some("ASSEMBLY_A".into()));
    app.documents[0].sch_editor.compiled_path = Some(String::new());
    let a = app.variant_projection(0).unwrap();
    let b = app.variant_projection(0).unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(app.documents[0].model, before);
    assert!(!app.documents[0].dirty());
    assert_eq!(app.documents[0].history.revision, 0);
    let edit = eda_core::schematic_edit::Edit::ParameterValue {
        component: None,
        name: "Title".into(),
        value: Some("Forbidden".into()),
    };
    assert!(app.documents[0].edit_schematic(edit, 0).is_err());
    assert_eq!(app.documents[0].model, before);
    assert_eq!(
        a.schematic.as_ref().unwrap().primitives.len(),
        before.schematic.unwrap().primitives.len() + 2
    );
}
#[test]
fn project_variant_edits_save_reopen_and_undo_without_saving_the_schematic() {
    let (_dir, mut app) = fixture();
    let source = std::fs::read(app.documents[0].path.as_ref().unwrap()).unwrap();
    let mut next = app.projects[0].variants.clone();
    next[0].name = "Changed assembly".into();
    app.apply_variants(0, next).unwrap();
    assert!(app.variants_dirty());
    assert!(app.save_variants());
    assert!(!app.variants_dirty());
    let loaded = eda_io::altium::load_project(&app.projects[0].path).unwrap();
    assert_eq!(loaded.variants[0].name, "Changed assembly");
    app.documents[0].sch_editor.compiled_path = Some(String::new());
    assert!(app.variant_undo(false));
    assert_eq!(app.projects[0].variants[0].name, "Assembly A");
    assert!(app.variants_dirty());
    assert_eq!(
        std::fs::read(app.documents[0].path.as_ref().unwrap()).unwrap(),
        source
    );
}

fn frame(app: &mut PcbCraft, ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1707., 1019.),
            )),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| app.variants_dialog(ctx),
    )
}
fn button(output: &egui::FullOutput, label: &str) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::epaint::Shape::Text(t) if t.galley.job.text == label => {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("Missing {label} button"))
}
fn click(app: &mut PcbCraft, ctx: &egui::Context, at: egui::Pos2) {
    for pressed in [true, false] {
        let _ = frame(
            app,
            ctx,
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}

#[test]
fn manager_cancel_discards_definitions_and_drawing_style() {
    let (_dir, mut app) = fixture();
    app.open_variants();
    let original = app.projects[0].variants.clone();
    let style = app.settings.variant_style.clone();
    let d = app.variants.dialog.as_mut().unwrap();
    d.draft[0].name = "Cancelled".into();
    d.style.mesh = true;
    d.style.cross = false;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = frame(&mut app, &ctx, vec![]);
    }
    let output = frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, button(&output, "Cancel"));
    assert!(app.variants.dialog.is_none());
    assert_eq!(app.projects[0].variants, original);
    assert_eq!(app.settings.variant_style, style);
    assert!(!app.variants_dirty());
}

#[test]
fn manager_ok_commits_project_history_and_style_without_editing_source_sheet() {
    let (_dir, mut app) = fixture();
    app.open_variants();
    let original = app.documents[0].model.clone();
    let d = app.variants.dialog.as_mut().unwrap();
    d.draft[0].name = "Accepted".into();
    d.style.mesh = true;
    d.style.cross = false;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = frame(&mut app, &ctx, vec![]);
    }
    let output = frame(&mut app, &ctx, vec![]);
    click(&mut app, &ctx, button(&output, "OK"));
    assert!(app.variants.dialog.is_none());
    assert_eq!(app.projects[0].variants[0].name, "Accepted");
    assert!(app.settings.variant_style.mesh);
    assert!(app.variants_dirty());
    assert_eq!(app.documents[0].model, original);
    assert!(!app.documents[0].dirty());
    app.documents[0].sch_editor.compiled_path = Some(String::new());
    app.variant_undo(false);
    assert_eq!(app.projects[0].variants[0].name, "Assembly A");
}
