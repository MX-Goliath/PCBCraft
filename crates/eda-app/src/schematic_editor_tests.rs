use super::*;
use eda_render::SchematicInput;

fn app() -> PcbCraft {
    PcbCraft {
        file_dialog: rfd::FileDialog::new(),
        documents: vec![OpenDocument::new(
            Document::new(DocumentKind::Schematic, "Interaction test"),
            None,
        )],
        projects: vec![],
        active: Some(0),
        settings: Default::default(),
        search: String::new(),
        log: vec![],
        command: None,
        pending: None,
        preferences: false,
        about: false,
        allow_exit: false,
        cursor_mm: None,
        pad_edit: None,
        property_ui: Default::default(),
        sch_clipboard: None,
    }
}

fn pointer(point: Point, clicked: bool) -> SchematicInput {
    SchematicInput {
        point: Some(point),
        clicked,
        ..Default::default()
    }
}

#[test]
fn cancelled_placement_and_wire_do_not_commit_and_finished_wire_is_one_transaction() {
    let mut app = app();
    let ctx = egui::Context::default();
    let original = app.documents[0].model.clone();
    app.start_schematic_tool(Placement::Resistor);
    app.schematic_input(0, pointer(Point::mm(20, 20), false), &ctx);
    assert!(app.documents[0].view.sch_preview.is_some());
    assert_eq!(app.documents[0].model, original);
    app.schematic_action(SchAction::Select, &ctx);
    assert!(app.documents[0].view.sch_preview.is_none());
    app.start_schematic_tool(Placement::Wire);
    app.schematic_input(0, pointer(Point::mm(20, 20), true), &ctx);
    app.schematic_input(0, pointer(Point::mm(40, 30), true), &ctx);
    assert_eq!(app.documents[0].model, original);
    app.schematic_action(SchAction::Select, &ctx);
    assert_eq!(app.documents[0].history.revision, 0);
    app.start_schematic_tool(Placement::Wire);
    app.schematic_input(0, pointer(Point::mm(20, 20), true), &ctx);
    app.schematic_input(0, pointer(Point::mm(40, 30), true), &ctx);
    app.schematic_input(
        0,
        SchematicInput {
            secondary_clicked: true,
            ..pointer(Point::mm(80, 90), false)
        },
        &ctx,
    );
    let doc = &mut app.documents[0];
    let s = doc.model.schematic.as_ref().unwrap();
    assert_eq!(doc.history.revision, 1);
    assert!(
        matches!(&s.primitives[0].kind, K::Polyline{points,electrical,..}
        if points.len()==3 && electrical=="Wire" && points.last().unwrap().x < 50_000_000)
    );
    assert!(!doc.sch_editor.active());
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model, original);
}

#[test]
fn fast_drag_release_commits_group_once_and_stale_move_does_not_overwrite() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.start_schematic_tool(Placement::Capacitor);
    app.schematic_input(0, pointer(Point::mm(20, 20), true), &ctx);
    app.schematic_action(SchAction::Select, &ctx);
    let placed = app.documents[0].model.clone();
    let id = placed.schematic.as_ref().unwrap().components[0].id;
    app.schematic_input(
        0,
        SchematicInput {
            hit: Some(id),
            press_origin: Some(Point::mm(20, 20)),
            drag_stopped: true,
            ..pointer(Point::mm(40, 30), false)
        },
        &ctx,
    );
    assert_eq!(app.documents[0].history.revision, 2);
    assert_ne!(app.documents[0].model, placed);
    assert!(!app.documents[0].sch_editor.active());
    app.schematic_action(SchAction::Move, &ctx);
    let doc = &mut app.documents[0];
    doc.edit_schematic(
        Edit::ComponentProperties {
            id,
            designator: "C42".into(),
            comment: "1uF".into(),
        },
        2,
    )
    .unwrap();
    let confirmed = doc.model.clone();
    app.schematic_input(0, pointer(Point::mm(80, 90), true), &ctx);
    assert_eq!(app.documents[0].model, confirmed);
    assert_eq!(app.documents[0].history.revision, 3);
    assert!(app.log.last().unwrap().0);
}
