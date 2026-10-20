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
        components_ui: Default::default(),
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
fn library_placement_preview_repeat_rotation_cancel_and_stale_revision() {
    use eda_core::library::{Symbol, SymbolVariant};
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut fragment =
        edit::place(Placement::Resistor, Point::default(), 0, "R?", "10k", 0).unwrap();
    let mut hidden = edit::primitive(
        K::Text {
            position: Point::default(),
            text: "hidden library metadata".into(),
            font: 0,
            orientation: 0,
            justification: 0,
        },
        Some(fragment.components[0].id),
    );
    hidden.presentation.hidden = true;
    fragment.primitives.push(hidden);
    fragment.fonts = app.documents[0]
        .model
        .schematic
        .as_ref()
        .unwrap()
        .fonts
        .clone();
    let symbol = Symbol {
        name: "Library R".into(),
        description: "fixture".into(),
        parts: 1,
        display_modes: 1,
        variants: vec![SymbolVariant {
            part: 1,
            display_mode: 0,
            fragment,
        }],
    };
    let initial = app.documents[0].model.clone();
    app.start_library_tool(symbol.clone(), 1, 0);
    app.schematic_input(0, pointer(Point::mm(20, 20), false), &ctx);
    assert_eq!(app.documents[0].model, initial);
    assert!(app.documents[0].view.sch_preview.is_some());
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let doc = &mut app.documents[0];
            eda_render::canvas(ui, &doc.model, &mut doc.view);
        });
    });
    fn contains_text(shape: &egui::Shape, text: &str) -> bool {
        match shape {
            egui::Shape::Text(t) => t.galley.job.text.contains(text),
            egui::Shape::Vec(v) => v.iter().any(|s| contains_text(s, text)),
            _ => false,
        }
    }
    assert!(output.shapes.iter().any(|s| contains_text(&s.shape, "R1")));
    assert!(
        !output
            .shapes
            .iter()
            .any(|s| contains_text(&s.shape, "hidden library metadata"))
    );
    app.schematic_action(SchAction::Select, &ctx);
    assert!(!app.documents[0].sch_editor.active());
    assert_eq!(app.documents[0].history.revision, 0);
    app.start_library_tool(symbol.clone(), 1, 0);
    app.schematic_input(0, pointer(Point::mm(20, 20), true), &ctx);
    assert!(app.documents[0].sch_editor.active());
    if let Tool::Library { turns, .. } = &mut app.documents[0].sch_editor.tool {
        *turns = 1;
    }
    app.schematic_input(0, pointer(Point::mm(40, 30), true), &ctx);
    let scene = app.documents[0].model.schematic.as_ref().unwrap();
    assert_eq!(scene.components[0].designator, "R1");
    assert_eq!(scene.components[1].designator, "R2");
    assert_eq!(scene.components[1].settings.rotation, 1);
    assert_ne!(scene.components[0].id, scene.components[1].id);
    assert_eq!(app.documents[0].history.revision, 2);
    app.schematic_input(
        0,
        SchematicInput {
            secondary_clicked: true,
            ..Default::default()
        },
        &ctx,
    );
    assert!(!app.documents[0].sch_editor.active());
    app.run(Command::Undo, &ctx);
    assert_eq!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components
            .len(),
        1
    );
    app.run(Command::Redo, &ctx);
    assert_eq!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components
            .len(),
        2
    );
    app.start_library_tool(symbol, 1, 0);
    let revision = app.documents[0].history.revision;
    app.documents[0]
        .edit_schematic(
            Edit::Insert(Fragment {
                primitives: vec![edit::primitive(
                    K::Junction {
                        position: Point::mm(90, 90),
                        radius_nm: 254_000,
                    },
                    None,
                )],
                ..Default::default()
            }),
            revision,
        )
        .unwrap();
    app.schematic_input(0, pointer(Point::mm(60, 30), true), &ctx);
    assert_eq!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components
            .len(),
        2
    );
    assert!(!app.documents[0].sch_editor.active());
}

#[test]
fn properties_keeps_dock_width_with_long_parameter_names_and_font_families() {
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut f = edit::place(
        Placement::Resistor,
        Point::mm(20, 30),
        0,
        "R1234",
        "=LongParameterName",
        0,
    )
    .unwrap();
    let component = f.components[0].id;
    f.components[0]
        .settings
        .models
        .push(eda_core::schematic::ComponentModel {
            name: "Very long imported footprint model name".into(),
            kind: "PCBLIB".into(),
            current: true,
        });
    for name in ["Power", "Resistance"] {
        let value = "Long parameter value which must remain inside its column";
        f.components[0].parameters.insert(name.into(), value.into());
        let mut p = edit::primitive(
            K::Text {
                position: Point::default(),
                text: value.into(),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            Some(component),
        );
        p.presentation = eda_core::schematic::Presentation {
            name: name.into(),
            raw_text: Some(value.into()),
            hidden: true,
            ..Default::default()
        };
        f.primitives.push(p);
    }
    app.documents[0].model.schematic.as_mut().unwrap().fonts[0].name =
        "A very long schematic font family name".into();
    app.documents[0].edit_schematic(Edit::Insert(f), 0).unwrap();
    let mut contexts = vec![None, Some(component)];
    contexts.extend(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .map(|p| Some(p.id)),
    );
    for selected in contexts {
        app.documents[0].view.selected = selected;
        for _ in 0..12 {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1707.0, 1027.0),
                )),
                ..Default::default()
            };
            let _ = ctx.run(input, |ctx| {
                let panel = egui::SidePanel::right("properties_layout_regression")
                    .default_width(316.0)
                    .show(ctx, |ui| app.properties(ui));
                assert!(
                    panel.response.rect.width() <= 317.0,
                    "Properties expanded to {}",
                    panel.response.rect.width()
                );
            });
        }
    }
}

#[test]
fn duplicate_designator_keeps_display_and_property_value_in_sync() {
    let mut app = app();
    let ctx = egui::Context::default();
    let f = edit::place(Placement::Resistor, Point::mm(20, 20), 0, "R1", "10k", 0).unwrap();
    app.documents[0].view.selected = Some(f.components[0].id);
    app.documents[0].edit_schematic(Edit::Insert(f), 0).unwrap();
    app.schematic_action(SchAction::Duplicate, &ctx);
    app.schematic_input(0, pointer(Point::mm(60, 20), true), &ctx);
    let s = app.documents[0].model.schematic.as_ref().unwrap();
    let c = &s.components[1];
    assert_eq!(c.designator, "R2");
    let p = s
        .primitives
        .iter()
        .find(|p| {
            p.component == Some(c.id)
                && p.text_role == Some(eda_core::schematic::TextRole::Designator)
        })
        .unwrap();
    assert_eq!(p.presentation.raw_text.as_deref(), Some("R2"));
    assert!(matches!(&p.kind,K::Text{text,..}if text=="R2"));
}

#[test]
fn grid_shortcut_edits_the_same_sheet_setting_as_properties_and_is_undoable() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.documents[0].view.grid = false;
    assert!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .sheet
            .grid_visible
    );
    app.run(Command::ToggleGrid, &ctx);
    assert!(
        !app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .sheet
            .grid_visible
    );
    app.run(Command::ToggleGrid, &ctx);
    assert!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .sheet
            .grid_visible
    );
    app.run(Command::Undo, &ctx);
    assert!(
        !app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .sheet
            .grid_visible
    );
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
