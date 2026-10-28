use super::*;
use eda_render::SchematicInput;

pub(in crate::app) fn app() -> PcbCraft {
    PcbCraft {
        sheet_sync: Default::default(),
        document_loader: Default::default(),
        cross_probe: Default::default(),
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
        layer_stack: None,
        annotation: None,
        design_sync: None,
        allow_exit: false,
        cursor_mm: None,
        pad_edit: None,
        property_ui: Default::default(),
        sch_clipboard: None,
        pcb_clipboard: None,
        components_ui: Default::default(),
    }
}

#[test]
fn hierarchy_navigation_is_project_scoped_and_does_not_open_unrelated_sheets() {
    use eda_io::altium::{Project, ProjectEntry};
    let mut app = app();
    let base = std::env::temp_dir().join(Uuid::new_v4().to_string());
    let parent = base.join("Main.SchDoc");
    let child = base.join("child").join("Child.SchDoc");
    let other = base.join("other").join("Child.SchDoc");
    let symbol = edit::primitive(
        K::SheetSymbol {
            a: Point::mm(20, 40),
            b: Point::mm(40, 20),
            designator: "Child1".into(),
            filename: "child/Child.SchDoc".into(),
        },
        None,
    );
    let mut entry = edit::primitive(
        K::SheetEntry {
            position: Point::mm(20, 35),
            side: 0,
            io_type: 2,
            name: "SIGNAL".into(),
            font: 0,
            text_color: 0,
            arrow_kind: "Block & Triangle".into(),
        },
        None,
    );
    entry.presentation.parent = Some(symbol.id);
    let entry_id = entry.id;
    app.documents[0].source_path = Some(parent.clone());
    app.documents[0].model.name = "Main.SchDoc".into();
    app.documents[0]
        .model
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .extend([symbol, entry]);
    let mut child_doc = Document::new(DocumentKind::Schematic, "Child.SchDoc");
    let port = edit::primitive(
        K::Port {
            position: Point::mm(15, 20),
            width_nm: 5_000_000,
            height_nm: 2_540_000,
            style: 0,
            io_type: 2,
            name: "SIGNAL".into(),
            font: 0,
        },
        None,
    );
    let port_id = port.id;
    child_doc.schematic.as_mut().unwrap().primitives.push(port);
    app.documents
        .push(OpenDocument::new(child_doc.clone(), Some(child.clone())));
    child_doc.id = Uuid::new_v4();
    app.documents
        .push(OpenDocument::new(child_doc, Some(other.clone())));
    app.projects.push(Project {
        path: base.join("Test.PrjPcb"),
        name: "Test".into(),
        documents: [parent.clone(), child]
            .into_iter()
            .map(|path| ProjectEntry {
                original_path: path.to_string_lossy().into(),
                path,
                kind: "schdoc".into(),
                exists: true,
            })
            .collect(),
        generated_documents: vec![],
        variants: vec![],
        virtual_bom: false,
        sheet_children: Default::default(),
        sheet_numbers: Default::default(),
        netlist_options: Default::default(),
        warnings: vec![],
    });
    let before: Vec<_> = app.documents.iter().map(|d| d.model.clone()).collect();
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
            ..Default::default()
        },
        |_| {},
    );
    let mut click = SchematicInput {
        hit: Some(entry_id),
        clicked: true,
        ..Default::default()
    };
    assert!(app.hierarchy_click(0, &click, &ctx));
    assert_eq!(app.active, Some(0)); // First click must not cross-probe into PCB.
    click.double_clicked = true;
    assert!(app.hierarchy_click(0, &click, &ctx));
    assert_eq!(app.active, Some(1));
    assert_eq!(app.documents[1].view.selected, Some(port_id));
    assert!(app.navigate_hierarchy(1, port_id));
    assert_eq!(app.active, Some(0));
    assert_eq!(app.documents[0].view.selected, Some(entry_id));
    assert_eq!(app.documents.len(), 3);
    assert_eq!(
        app.documents
            .iter()
            .map(|d| d.model.clone())
            .collect::<Vec<_>>(),
        before
    );
}

fn pointer(point: Point, clicked: bool) -> SchematicInput {
    SchematicInput {
        point: Some(point),
        clicked,
        ..Default::default()
    }
}
fn focused_context() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = ctx.run(
        egui::RawInput {
            focused: true,
            ..Default::default()
        },
        |_| {},
    );
    ctx
}

fn wire_app() -> (PcbCraft, Uuid) {
    let mut app = app();
    let s = app.documents[0].model.schematic.as_mut().unwrap();
    s.sheet.options.snap_enabled = false;
    let p = edit::primitive(
        K::Polyline {
            points: vec![Point::mm(20, 40), Point::mm(60, 40)],
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    let id = p.id;
    s.primitives.push(p);
    (app, id)
}

#[test]
fn wire_drag_handle_preview_commit_pick_and_cancel_share_the_core_command() {
    for picked in [false, true] {
        let (mut app, id) = wire_app();
        let ctx = focused_context();
        let before = app.documents[0].model.clone();
        if picked {
            app.documents[0].view.selected = Some(id);
            app.schematic_action(SchAction::Drag, &ctx);
            app.schematic_input(
                0,
                SchematicInput {
                    hit: Some(id),
                    wire_handle: Some(edit::WireHandle::Segment(0)),
                    ..pointer(Point::mm(40, 40), true)
                },
                &ctx,
            );
        } else {
            app.schematic_input(
                0,
                SchematicInput {
                    hit: Some(id),
                    wire_handle: Some(edit::WireHandle::Segment(0)),
                    press_origin: Some(Point::mm(40, 40)),
                    drag_started: true,
                    ..pointer(Point::mm(40, 50), false)
                },
                &ctx,
            );
        }
        app.schematic_input(0, pointer(Point::mm(40, 50), false), &ctx);
        assert_eq!(app.documents[0].model, before);
        let expected = edit::preview(
            &before,
            &Edit::DragWire {
                id,
                handle: edit::WireHandle::Segment(0),
                delta: Point::mm(0, 10),
            },
        )
        .unwrap();
        let shown = &app.documents[0]
            .view
            .sch_preview
            .as_ref()
            .unwrap()
            .primitives[0]
            .kind;
        assert_eq!(
            *shown,
            expected
                .schematic
                .as_ref()
                .unwrap()
                .primitive(id)
                .unwrap()
                .kind
        );
        app.schematic_input(
            0,
            SchematicInput {
                clicked: picked,
                drag_stopped: !picked,
                ..pointer(Point::mm(40, 50), false)
            },
            &ctx,
        );
        assert_eq!(app.documents[0].model, expected);
        assert_eq!(app.documents[0].history.revision, 1);
        assert!(!app.documents[0].sch_editor.active());
        app.run(Command::Undo, &ctx);
        assert_eq!(app.documents[0].model, before);
        app.run(Command::Redo, &ctx);
        assert_eq!(app.documents[0].model, expected);
    }
    for cancel in 0..5 {
        let (mut app, id) = wire_app();
        let ctx = focused_context();
        app.schematic_input(
            0,
            SchematicInput {
                hit: Some(id),
                press_origin: Some(Point::mm(40, 40)),
                drag_started: true,
                ..pointer(Point::mm(40, 50), false)
            },
            &ctx,
        );
        let mut before = app.documents[0].model.clone();
        match cancel {
            0 => app.schematic_input(
                0,
                SchematicInput {
                    drag_stopped: true,
                    ..Default::default()
                },
                &ctx,
            ),
            1 => {
                app.schematic_action(SchAction::Select, &ctx);
            }
            2 => {
                let _ = ctx.run(
                    egui::RawInput {
                        focused: false,
                        ..Default::default()
                    },
                    |ctx| app.schematic_input(0, pointer(Point::mm(40, 50), true), ctx),
                );
            }
            3 => {
                app.add_document(Document::new(DocumentKind::Schematic, "Other"), None);
                app.finish_schematic_interaction();
            }
            _ => {
                app.documents[0]
                    .edit_schematic(
                        Edit::Width {
                            id,
                            width_nm: 500_000,
                        },
                        0,
                    )
                    .unwrap();
                before = app.documents[0].model.clone();
                app.schematic_input(
                    0,
                    SchematicInput {
                        drag_stopped: true,
                        ..pointer(Point::mm(40, 50), false)
                    },
                    &ctx,
                );
            }
        }
        assert_eq!(app.documents[0].model, before, "cancel case {cancel}");
        assert!(!app.documents[0].sch_editor.active());
    }
}

#[test]
fn wire_drawing_backtracks_without_deleting_selection_and_finishes_on_existing_wire() {
    let (mut app, id) = wire_app();
    let ctx = focused_context();
    app.documents[0].view.selected = Some(id);
    app.start_schematic_tool(Placement::Wire);
    for at in [Point::mm(30, 20), Point::mm(40, 20)] {
        app.schematic_input(0, pointer(at, true), &ctx);
    }
    let _ = ctx.run(
        egui::RawInput {
            focused: true,
            events: vec![egui::Event::Key {
                key: Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| {
            app.schematic_shortcuts(ctx);
        },
    );
    assert!(
        matches!(&app.documents[0].sch_editor.tool,Tool::Path {points,..} if points==&vec![Point::mm(30,20)])
    );
    assert_eq!(app.documents[0].history.revision, 0);
    app.schematic_input(0, pointer(Point::mm(30, 40), true), &ctx);
    assert_eq!(app.documents[0].history.revision, 1);
    let s = app.documents[0].model.schematic.as_ref().unwrap();
    assert_eq!(s.primitives.len(), 2);
    assert_eq!(edit::junctions(s.primitives.iter()).len(), 1);
    assert!(
        matches!(&app.documents[0].sch_editor.tool,Tool::Path {points,..} if points.is_empty())
    );
    app.schematic_input(0, pointer(Point::mm(70, 20), true), &ctx);
    app.schematic_action(SchAction::Select, &ctx);
    assert_eq!(app.documents[0].history.revision, 1);
    app.run(Command::Undo, &ctx);
    assert_eq!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .len(),
        1
    );
}

#[test]
fn actual_canvas_wire_gestures_resolve_handles_and_cancel_outside() {
    fn frame(app: &mut PcbCraft, ctx: &egui::Context, events: Vec<egui::Event>) {
        let _ = ctx.run(
            egui::RawInput {
                focused: true,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let doc = &mut app.documents[0];
                    let input = eda_render::canvas(ui, &doc.model, &mut doc.view)
                        .schematic_input
                        .unwrap();
                    app.schematic_input(0, input, ctx);
                });
            },
        );
    }
    for (handle, fast, outside) in [
        (edit::WireHandle::Segment(0), false, false),
        (edit::WireHandle::Segment(0), true, false),
        (edit::WireHandle::Vertex(0), false, false),
        (edit::WireHandle::Segment(0), false, true),
    ] {
        let (mut app, id) = wire_app();
        let ctx = focused_context();
        theme::apply(&ctx);
        let camera = &mut app.documents[0].view.camera;
        camera.centre_mm = [40.0, 40.0];
        camera.pixels_per_mm = 10.0;
        camera.fitted = true;
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![]);
        let from = egui::pos2(
            if matches!(handle, edit::WireHandle::Vertex(_)) {
                300.0
            } else {
                500.0
            },
            350.0,
        );
        let to = if outside {
            egui::pos2(1200.0, 250.0)
        } else {
            from + egui::vec2(0.0, -100.0)
        };
        let before = app.documents[0].model.clone();
        if fast {
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(from),
                    button(from, true),
                    egui::Event::PointerMoved(to),
                    button(to, false),
                ],
            );
        } else {
            frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(from), button(from, true)],
            );
            frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)]);
            assert!(
                matches!(app.documents[0].sch_editor.tool,Tool::WireDrag{handle:h,..} if h==handle)
            );
            assert_eq!(app.documents[0].model, before);
            frame(&mut app, &ctx, vec![button(to, false)]);
        }
        let expected = if outside {
            before.clone()
        } else {
            edit::preview(
                &before,
                &Edit::DragWire {
                    id,
                    handle,
                    delta: Point::mm(0, 10),
                },
            )
            .unwrap()
        };
        assert_eq!(
            app.documents[0].model, expected,
            "{handle:?}, fast={fast}, outside={outside}"
        );
        assert!(!app.documents[0].sch_editor.active());
    }
}

fn connected_app() -> (PcbCraft, Uuid, Uuid) {
    let mut app = app();
    let f = edit::place(Placement::Resistor, Point::mm(40, 40), 0, "R1", "10k", 0).unwrap();
    let id = f.components[0].id;
    let s = app.documents[0].model.schematic.as_mut().unwrap();
    s.sheet.options.snap_enabled = false;
    s.components = f.components;
    s.primitives = f.primitives;
    let p = edit::primitive(
        K::Polyline {
            points: vec![
                Point::mm(10, 40),
                Point {
                    x: 29_840_000,
                    y: 40_000_000,
                },
            ],
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    let wire = p.id;
    s.primitives.push(p);
    (app, id, wire)
}
#[test]
fn connected_canvas_drag_previews_wires_and_commits_once() {
    let (mut app, id, wire) = connected_app();
    let ctx = focused_context();
    let before = app.documents[0].model.clone();
    app.schematic_input(
        0,
        SchematicInput {
            hit: Some(id),
            press_origin: Some(Point::mm(40, 40)),
            drag_started: true,
            ..pointer(Point::mm(50, 50), false)
        },
        &ctx,
    );
    assert_eq!(app.documents[0].model, before);
    let shown = app.documents[0].view.sch_preview.as_ref().unwrap();
    assert!(shown.primitives.iter().any(|p|p.id==wire && matches!(&p.kind,K::Polyline{points,..} if points.last()==Some(&Point {x:39_840_000,y:50_000_000}))));
    app.schematic_input(
        0,
        SchematicInput {
            drag_stopped: true,
            ..pointer(Point::mm(50, 50), false)
        },
        &ctx,
    );
    let placed = app.documents[0].model.clone();
    assert_eq!(
        placed,
        edit::preview(
            &before,
            &Edit::Drag {
                ids: vec![id],
                pivot: Point::mm(40, 40),
                quarter_turns: 0,
                delta: Point::mm(10, 10)
            }
        )
        .unwrap()
    );
    assert_eq!(app.documents[0].history.revision, 1);
    assert!(!app.documents[0].sch_editor.active());
    app.run(Command::Undo, &ctx);
    assert_eq!(app.documents[0].model, before);
    app.run(Command::Redo, &ctx);
    assert_eq!(app.documents[0].model, placed);
}
#[test]
fn connected_drag_cancels_outside_on_escape_focus_tab_or_stale_revision() {
    for cancel in 0..5 {
        let (mut app, id, _) = connected_app();
        let ctx = focused_context();
        app.schematic_input(
            0,
            SchematicInput {
                hit: Some(id),
                press_origin: Some(Point::mm(40, 40)),
                drag_started: true,
                ..pointer(Point::mm(50, 50), false)
            },
            &ctx,
        );
        let mut before = app.documents[0].model.clone();
        match cancel {
            0 => app.schematic_input(
                0,
                SchematicInput {
                    drag_stopped: true,
                    ..Default::default()
                },
                &ctx,
            ),
            1 => {
                let _ = ctx.run(
                    egui::RawInput {
                        focused: true,
                        events: vec![egui::Event::Key {
                            key: Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        }],
                        ..Default::default()
                    },
                    |ctx| {
                        app.schematic_shortcuts(ctx);
                        app.schematic_input(
                            0,
                            SchematicInput {
                                hit: Some(id),
                                press_origin: Some(Point::mm(40, 40)),
                                drag_stopped: true,
                                ..pointer(Point::mm(50, 50), false)
                            },
                            ctx,
                        );
                    },
                );
            }
            2 => {
                let _ = ctx.run(
                    egui::RawInput {
                        focused: false,
                        ..Default::default()
                    },
                    |ctx| {
                        app.schematic_input(
                            0,
                            SchematicInput {
                                drag_stopped: true,
                                ..pointer(Point::mm(50, 50), false)
                            },
                            ctx,
                        )
                    },
                );
            }
            3 => {
                app.add_document(Document::new(DocumentKind::Schematic, "Other"), None);
                app.finish_schematic_interaction();
            }
            _ => {
                app.documents[0]
                    .edit_schematic(
                        Edit::ComponentProperties {
                            id,
                            designator: "R1".into(),
                            comment: "47k".into(),
                        },
                        0,
                    )
                    .unwrap();
                before = app.documents[0].model.clone();
                app.schematic_input(
                    0,
                    SchematicInput {
                        drag_stopped: true,
                        ..pointer(Point::mm(50, 50), false)
                    },
                    &ctx,
                );
                assert!(app.log.last().unwrap().0);
            }
        }
        assert_eq!(app.documents[0].model, before, "case {cancel}");
        assert!(!app.documents[0].sch_editor.active(), "case {cancel}");
    }
}
#[test]
fn m_d_and_m_m_pick_objects_with_distinct_connection_semantics() {
    for connected in [false, true] {
        let (mut app, id, wire) = connected_app();
        let ctx = focused_context();
        for key in [Key::M, if connected { Key::D } else { Key::M }] {
            let _ = ctx.run(
                egui::RawInput {
                    focused: true,
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    app.schematic_shortcuts(ctx);
                },
            );
            let _ = ctx.run(
                egui::RawInput {
                    focused: true,
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |_| {},
            );
        }
        assert!(matches!(
            app.documents[0].sch_editor.tool,
            Tool::Pick { .. }
        ));
        let original = app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitive(wire)
            .unwrap()
            .clone();
        app.schematic_input(
            0,
            SchematicInput {
                hit: Some(id),
                ..pointer(Point::mm(40, 40), true)
            },
            &ctx,
        );
        app.schematic_input(0, pointer(Point::mm(50, 50), true), &ctx);
        assert_eq!(app.documents[0].history.revision, 1);
        assert_eq!(
            app.documents[0]
                .model
                .schematic
                .as_ref()
                .unwrap()
                .primitive(wire)
                == Some(&original),
            !connected
        );
    }
}

#[test]
fn library_placement_preview_repeat_rotation_cancel_and_stale_revision() {
    use eda_core::library::{Symbol, SymbolVariant};
    let mut app = app();
    let ctx = focused_context();
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
    let ctx = focused_context();
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
    let ctx = focused_context();
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
    let ctx = focused_context();
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

fn drag_app() -> PcbCraft {
    use eda_core::library::{Library, Symbol, SymbolVariant};
    let mut app = app();
    let mut fragment = edit::place(
        Placement::Resistor,
        Point::default(),
        0,
        "R?",
        "=Resistance",
        0,
    )
    .unwrap();
    fragment.fonts = app.documents[0]
        .model
        .schematic
        .as_ref()
        .unwrap()
        .fonts
        .clone();
    fragment.components[0]
        .parameters
        .insert("Resistance".into(), "10k".into());
    fragment.components[0].comment = "10k".into();
    let symbol = Symbol {
        name: "DRAG_TEST".into(),
        description: "Test library symbol".into(),
        parts: 1,
        display_modes: 1,
        variants: vec![SymbolVariant {
            part: 1,
            display_mode: 0,
            fragment,
        }],
    };
    app.components_ui.libraries.push((
        PathBuf::from("fixture.SchLib"),
        Library {
            embedded_footprints: None,
            name: "fixture.SchLib".into(),
            symbols: vec![symbol],
            warnings: vec![],
        },
    ));
    app.settings.components = true;
    app.settings.right_components = true;
    app
}
fn drag_frame(
    app: &mut PcbCraft,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    focused: bool,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1707.0, 1027.0),
            )),
            events,
            focused,
            ..Default::default()
        },
        |ctx| {
            app.right_dock(ctx);
            app.workspace(ctx);
            app.finish_library_drag(ctx);
        },
    )
}
fn button(at: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}
fn row_origin(output: &egui::FullOutput) -> egui::Pos2 {
    output
        .shapes
        .iter()
        .filter_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                (t.galley.job.text == "DRAG_TEST" && t.pos.y < 150.0)
                    .then_some(t.pos + egui::vec2(8.0, 5.0))
            } else {
                None
            }
        })
        .next()
        .expect("library row text")
}
#[test]
fn components_real_drag_drop_is_one_atomic_insert_and_history_transaction() {
    let mut app = drag_app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut output = drag_frame(&mut app, &ctx, vec![], true);
    for _ in 0..3 {
        output = drag_frame(&mut app, &ctx, vec![], true);
    }
    let from = row_origin(&output);
    let to = egui::pos2(1000.0, 500.0);
    let original = app.documents[0].model.clone();
    drag_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
        true,
    );
    drag_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], true);
    assert_eq!(app.documents[0].model, original);
    assert!(app.documents[0].view.sch_preview.is_some());
    drag_frame(&mut app, &ctx, vec![button(to, false)], true);
    let placed = app.documents[0].model.clone();
    let doc = &mut app.documents[0];
    let s = doc.model.schematic.as_ref().unwrap();
    assert_eq!(s.components.len(), 1);
    assert_eq!(s.components[0].designator, "R1");
    assert_eq!(s.components[0].comment, "10k");
    assert_eq!(doc.history.revision, 1);
    assert!(!doc.sch_editor.active());
    assert!(doc.view.sch_preview.is_none());
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model, original);
    doc.history.redo(&mut doc.model);
    assert_eq!(doc.model, placed);
    drag_frame(
        &mut app,
        &ctx,
        vec![button(to, true), button(to, false)],
        true,
    );
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
}
#[test]
fn components_collapsed_details_remains_accessible_above_dock_tabs() {
    let mut app = drag_app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let mut output = drag_frame(&mut app, &ctx, vec![], true);
    for _ in 0..3 {
        output = drag_frame(&mut app, &ctx, vec![], true);
    }
    let title_position = |output: &egui::FullOutput, name: &str| {
        output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == name => Some(t.pos),
                _ => None,
            })
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .expect("panel heading")
    };
    let from = title_position(&output, "Details") + egui::vec2(10.0, 5.0);
    drag_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
        true,
    );
    drag_frame(&mut app, &ctx, vec![button(from, false)], true);
    output = drag_frame(&mut app, &ctx, vec![], true);
    let collapsed = title_position(&output, "Details");
    let tab = title_position(&output, "Components");
    assert!(collapsed.y > from.y);
    assert!(
        collapsed.y + 16.0 < tab.y,
        "heading {collapsed:?}, tab {tab:?}"
    );
    let at = collapsed + egui::vec2(10.0, 5.0);
    drag_frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, true)],
        true,
    );
    drag_frame(&mut app, &ctx, vec![button(at, false)], true);
    output = drag_frame(&mut app, &ctx, vec![], true);
    assert!(title_position(&output, "Details").y < collapsed.y);
}
#[test]
fn components_preview_card_can_be_dragged_to_canvas() {
    for fast in [false, true] {
        let mut app = drag_app();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut output = drag_frame(&mut app, &ctx, vec![], true);
        for _ in 0..3 {
            output = drag_frame(&mut app, &ctx, vec![], true);
        }
        let from = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r)
                    if r.fill == Color32::WHITE && (r.rect.height() - 184.0).abs() < 1.0 =>
                {
                    Some(r.rect.center())
                }
                _ => None,
            })
            .expect("preview card");
        let to = egui::pos2(1000.0, 500.0);
        if fast {
            drag_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(from),
                    button(from, true),
                    egui::Event::PointerMoved(to),
                    button(to, false),
                ],
                true,
            );
        } else {
            drag_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(from), button(from, true)],
                true,
            );
            drag_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], true);
            assert!(app.documents[0].view.sch_preview.is_some());
            drag_frame(&mut app, &ctx, vec![button(to, false)], true);
        }
        assert_eq!(
            app.documents[0]
                .model
                .schematic
                .as_ref()
                .unwrap()
                .components
                .len(),
            1,
            "fast: {fast}"
        );
    }
}
#[test]
fn components_drag_cancels_outside_canvas_on_escape_focus_loss_and_tab_switch() {
    for cancel in 0..4 {
        let mut app = drag_app();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut output = drag_frame(&mut app, &ctx, vec![], true);
        for _ in 0..3 {
            output = drag_frame(&mut app, &ctx, vec![], true);
        }
        let from = row_origin(&output);
        let to = egui::pos2(1000.0, 500.0);
        let original = app.documents[0].model.clone();
        drag_frame(
            &mut app,
            &ctx,
            vec![egui::Event::PointerMoved(from), button(from, true)],
            true,
        );
        drag_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], true);
        match cancel {
            0 => {
                let outside = egui::pos2(1500.0, 900.0);
                drag_frame(
                    &mut app,
                    &ctx,
                    vec![egui::Event::PointerMoved(outside), button(outside, false)],
                    true,
                );
            }
            1 => {
                drag_frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::Key {
                            key: Key::Escape,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        },
                        button(to, false),
                    ],
                    true,
                );
            }
            2 => {
                drag_frame(&mut app, &ctx, vec![], false);
            }
            _ => {
                app.add_document(Document::new(DocumentKind::Schematic, "Other"), None);
                drag_frame(&mut app, &ctx, vec![button(to, false)], true);
                assert!(
                    app.documents[1]
                        .model
                        .schematic
                        .as_ref()
                        .unwrap()
                        .components
                        .is_empty()
                );
            }
        }
        assert_eq!(app.documents[0].model, original, "cancel case {cancel}");
        assert!(!app.documents[0].sch_editor.active());
        assert!(app.components_ui.drag_document.is_none());
    }
}
#[test]
fn components_fast_drop_and_stale_drop_preserve_revision_rules() {
    for stale in [false, true] {
        let mut app = drag_app();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let mut output = drag_frame(&mut app, &ctx, vec![], true);
        for _ in 0..3 {
            output = drag_frame(&mut app, &ctx, vec![], true);
        }
        let from = row_origin(&output);
        let to = egui::pos2(1000.0, 500.0);
        if stale {
            drag_frame(
                &mut app,
                &ctx,
                vec![egui::Event::PointerMoved(from), button(from, true)],
                true,
            );
            drag_frame(&mut app, &ctx, vec![egui::Event::PointerMoved(to)], true);
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
                    0,
                )
                .unwrap();
            let confirmed = app.documents[0].model.clone();
            drag_frame(&mut app, &ctx, vec![button(to, false)], true);
            assert_eq!(app.documents[0].model, confirmed);
            assert!(app.log.last().unwrap().0);
        } else {
            drag_frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(from),
                    button(from, true),
                    egui::Event::PointerMoved(to),
                    button(to, false),
                ],
                true,
            );
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
        }
    }
}
#[test]
fn library_preview_uses_raw_gray_comment_without_mutating_symbol() {
    let app = drag_app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    let fragment = &app.components_ui.libraries[0].1.symbols[0].variants[0].fragment;
    let before = fragment.clone();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_max_width(248.0);
            eda_render::symbol_preview(ui, fragment, 184.0);
        });
    });
    assert_eq!(*fragment, before);
    assert!(output.shapes.iter().any(|s|matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text=="=Resistance" && t.fallback_color==Color32::from_gray(128))));
    let mut with_io = before;
    let mut types = [0, 2].into_iter();
    for primitive in &mut with_io.primitives {
        if let K::Pin { electrical, .. } = &mut primitive.kind {
            *electrical = types.next().unwrap();
        }
    }
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_max_width(248.0);
            eda_render::symbol_preview(ui, &with_io, 184.0);
        });
    });
    assert_eq!(output.shapes.iter().filter(|s| s.clip_rect.is_positive() && matches!(&s.shape, egui::Shape::Mesh(m) if m.vertices.len() == 3 && m.vertices.iter().all(|v| v.color == Color32::WHITE))).count(), 2);
}

#[test]
fn mixed_group_duplicate_cut_paste_and_delete_are_single_history_steps() {
    let (mut app, wire) = wire_app();
    let ctx = focused_context();
    let mut ids = vec![];
    for (n, x) in [(1, 30), (2, 70)] {
        let f = edit::place(
            Placement::Resistor,
            Point::mm(x, 60),
            0,
            &format!("R{n}"),
            "10k",
            0,
        )
        .unwrap();
        ids.push(f.components[0].id);
        app.documents[0]
            .edit_schematic(Edit::Insert(f), n - 1)
            .unwrap();
    }
    ids.push(wire);
    app.documents[0].view.sch_selection = ids.clone();
    app.documents[0].view.selected = Some(ids[0]);
    let original = app.documents[0].model.clone();
    app.schematic_action(SchAction::Duplicate, &ctx);
    app.schematic_input(0, pointer(Point::mm(100, 100), true), &ctx);
    let duplicated = app.documents[0].model.clone();
    let s = duplicated.schematic.as_ref().unwrap();
    assert_eq!(
        s.components
            .iter()
            .map(|c| c.designator.as_str())
            .collect::<Vec<_>>(),
        vec!["R1", "R2", "R3", "R4"]
    );
    for c in &s.components {
        assert!(s.primitives.iter().any(|p| p.component == Some(c.id)
            && p.text_role == Some(eda_core::schematic::TextRole::Designator)
            && matches!(&p.kind,K::Text{text,..} if text==&c.designator)));
    }
    assert_eq!(app.documents[0].history.revision, 3);
    app.run(Command::Undo, &ctx);
    assert_eq!(app.documents[0].model, original);
    app.run(Command::Redo, &ctx);
    assert_eq!(app.documents[0].model, duplicated);
    app.documents[0].view.sch_selection = ids.clone();
    app.documents[0].view.selected = Some(ids[0]);
    let revision = app.documents[0].history.revision;
    app.schematic_action(SchAction::Cut, &ctx);
    assert_eq!(app.documents[0].history.revision, revision + 1);
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
    let cut = app.documents[0].model.clone();
    app.schematic_action(SchAction::Paste, &ctx);
    app.schematic_input(0, pointer(Point::mm(30, 60), true), &ctx);
    assert_eq!(
        app.documents[0]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .components
            .len(),
        4
    );
    app.run(Command::Undo, &ctx);
    assert_eq!(app.documents[0].model, cut);
    app.run(Command::Undo, &ctx);
    assert_eq!(app.documents[0].model, duplicated);
    app.documents[0].view.sch_selection = ids;
    app.documents[0].view.selected = Some(wire);
    app.schematic_action(SchAction::Delete, &ctx);
    assert_eq!(app.documents[0].model, cut);
}

#[test]
fn moved_segment_remains_selected_and_deleting_it_preserves_the_other_runs() {
    let (mut app, id) = wire_app();
    let ctx = focused_context();
    let s = app.documents[0].model.schematic.as_mut().unwrap();
    if let K::Polyline { points, .. } = &mut s.primitives[0].kind {
        *points = vec![
            Point::mm(20, 20),
            Point::mm(40, 20),
            Point::mm(40, 40),
            Point::mm(60, 40),
        ];
    }
    let span = edit::wire_segment_at(s, id, Point::mm(40, 30), 1).unwrap();
    app.documents[0].view.selected = Some(id);
    app.documents[0].view.sch_segments = vec![span];
    app.schematic_action(SchAction::Move, &ctx);
    app.schematic_input(0, pointer(Point::mm(50, 20), true), &ctx);
    assert_eq!(app.documents[0].view.sch_segments.len(), 1);
    let selected = app.documents[0].view.sch_segments[0];
    assert_eq!(selected.a, Point::mm(50, 20));
    assert_eq!(selected.b, Point::mm(50, 40));
    assert_ne!(selected.id, id);
    let moved = app.documents[0].model.clone();
    app.schematic_action(SchAction::Delete, &ctx);
    assert!(app.documents[0].view.selected.is_none());
    assert!(app.documents[0].view.sch_segments.is_empty());
    let s = app.documents[0].model.schematic.as_ref().unwrap();
    assert_eq!(s.primitives.len(), 2);
    assert!(
        s.primitives
            .iter()
            .all(|p| matches!(&p.kind,K::Polyline{points,..} if points[0].y==points[1].y))
    );
    app.run(Command::Undo, &ctx);
    assert_eq!(app.documents[0].model, moved);
}

#[test]
fn mixed_component_wire_drag_stretches_neighbour_and_preview_matches_commit() {
    for connected in [false, true] {
        let (mut app, id) = wire_app();
        let ctx = focused_context();
        let f = edit::place(Placement::Resistor, Point::mm(40, 60), 0, "R1", "10k", 0).unwrap();
        let part = f.components[0].id;
        let s = app.documents[0].model.schematic.as_mut().unwrap();
        s.components = f.components;
        s.primitives.extend(f.primitives);
        let branch = edit::primitive(
            K::Polyline {
                points: vec![Point::mm(60, 40), Point::mm(60, 10)],
                closed: false,
                electrical: "Wire".into(),
            },
            None,
        );
        let branch_id = branch.id;
        s.primitives.push(branch);
        let before = app.documents[0].model.clone();
        let selection = Selection {
            ids: vec![part, id],
            segments: vec![],
        };
        app.documents[0].view.sch_selection = selection.ids.clone();
        app.documents[0].view.selected = Some(part);
        app.schematic_action(
            if connected {
                SchAction::Drag
            } else {
                SchAction::Move
            },
            &ctx,
        );
        let command = Edit::TransformSelection {
            selection,
            pivot: Point::mm(40, 60),
            quarter_turns: 0,
            delta: Point::mm(5, 10),
            connected,
        };
        app.schematic_input(0, pointer(Point::mm(45, 70), false), &ctx);
        let mut expected_preview = edit::preview_fragment(&before, &command)
            .unwrap()
            .primitives;
        for p in &mut expected_preview {
            p.color = 0xc08000;
        }
        assert_eq!(
            app.documents[0]
                .view
                .sch_preview
                .as_ref()
                .unwrap()
                .primitives,
            expected_preview
        );
        app.schematic_input(0, pointer(Point::mm(45, 70), true), &ctx);
        assert_eq!(
            app.documents[0].model,
            edit::preview(&before, &command).unwrap()
        );
        assert_eq!(app.documents[0].history.revision, 1);
        let s = app.documents[0].model.schematic.as_ref().unwrap();
        assert!(
            matches!(&s.primitive(branch_id).unwrap().kind,K::Polyline{points,..} if points[0]==if connected {Point::mm(65,50)}else{Point::mm(60,40)} && points.last()==Some(&Point::mm(60,10)))
        );
        app.run(Command::Undo, &ctx);
        assert_eq!(app.documents[0].model, before);
    }
}

#[test]
fn actual_canvas_repeated_shift_clicks_delete_only_selected_wire_spans() {
    let (mut app, id) = wire_app();
    let ctx = focused_context();
    theme::apply(&ctx);
    let branch = edit::primitive(
        K::Polyline {
            points: vec![Point::mm(40, 40), Point::mm(40, 20)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    let branch_id = branch.id;
    app.documents[0]
        .model
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(branch);
    let camera = &mut app.documents[0].view.camera;
    camera.centre_mm = [40.0, 40.0];
    camera.pixels_per_mm = 10.0;
    camera.fitted = true;
    let before = app.documents[0].model.clone();
    let frame = |app: &mut PcbCraft, events, shift| {
        let _ = ctx.run(
            egui::RawInput {
                focused: true,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 700.0),
                )),
                events,
                modifiers: egui::Modifiers {
                    shift,
                    ..Default::default()
                },
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    let doc = &mut app.documents[0];
                    let input = eda_render::canvas(ui, &doc.model, &mut doc.view)
                        .schematic_input
                        .unwrap();
                    app.schematic_input(0, input, ctx);
                });
            },
        );
    };
    frame(&mut app, vec![], false);
    frame(&mut app, vec![], false);
    let click = |app: &mut PcbCraft, at, shift| {
        let key = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers {
                shift,
                ..Default::default()
            },
        };
        frame(app, vec![egui::Event::PointerMoved(at), key(true)], shift);
        frame(app, vec![key(false)], shift);
    };
    click(&mut app, egui::pos2(400.0, 350.0), false);
    assert_eq!(app.documents[0].view.sch_selection, vec![id]);
    click(&mut app, egui::pos2(400.0, 350.0), false);
    assert_eq!(
        app.documents[0].view.sch_segments,
        vec![edit::WireSegment {
            id,
            a: Point::mm(20, 40),
            b: Point::mm(40, 40)
        }]
    );
    click(&mut app, egui::pos2(500.0, 450.0), true);
    click(&mut app, egui::pos2(500.0, 450.0), true);
    assert_eq!(app.documents[0].view.sch_segments.len(), 2);
    app.schematic_action(SchAction::Delete, &ctx);
    let s = app.documents[0].model.schematic.as_ref().unwrap();
    assert!(s.primitive(branch_id).is_none());
    assert_eq!(s.primitives.len(), 1);
    assert!(
        matches!(&s.primitive(id).unwrap().kind,K::Polyline{points,..} if points==&vec![Point::mm(40,40),Point::mm(60,40)])
    );
    assert_eq!(app.documents[0].history.revision, 1);
    app.run(Command::Undo, &ctx);
    assert_eq!(app.documents[0].model, before);
}
