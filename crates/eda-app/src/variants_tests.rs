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
