use super::*;
use eda_io::altium::{Project, ProjectEntry};

fn fixture() -> (PcbCraft, Uuid, Uuid) {
    let mut app = super::super::schematic_editor::tests::app();
    let base = std::env::temp_dir().join(Uuid::new_v4().to_string());
    let paths = [base.join("Main.SchDoc"), base.join("Child.SchDoc")];
    app.documents[0].model.name = "Main.SchDoc".into();
    app.documents[0].source_path = Some(paths[0].clone());
    let mut child = Document::new(DocumentKind::Schematic, "Child.SchDoc");
    let port = edit::primitive(
        K::Port {
            position: Point::mm(20, 20),
            width_nm: 10_000_000,
            height_nm: 2_540_000,
            io_type: 1,
            style: 0,
            name: "Old".into(),
            font: 0,
        },
        None,
    );
    let port_id = port.id;
    child.schematic.as_mut().unwrap().primitives.push(port);
    app.documents
        .push(OpenDocument::new(child, Some(paths[1].clone())));
    let symbol = edit::primitive(
        K::SheetSymbol {
            a: Point::mm(20, 80),
            b: Point::mm(80, 20),
            filename: "Child.SchDoc".into(),
            designator: "U_Child".into(),
        },
        None,
    );
    let owner = symbol.id;
    app.documents[0]
        .model
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(symbol);
    let plan = sync::transfer(
        &app.documents[0].model,
        Destination::Entries(owner),
        &[Signal {
            id: Uuid::new_v4(),
            name: "Signal".into(),
            io: 2,
            harness: "Harness".into(),
        }],
        &[],
    )
    .unwrap();
    app.documents[0].model = edit::preview(
        &app.documents[0].model,
        &plan
            .edit_at(&app.documents[0].model, Point::mm(20, 70))
            .unwrap(),
    )
    .unwrap();
    let entry = app.documents[0]
        .model
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find_map(Signal::read)
        .unwrap()
        .id;
    app.projects.push(Project {
        path: base.join("Test.PrjPcb"),
        name: "Test.PrjPcb".into(),
        documents: paths
            .into_iter()
            .map(|path| ProjectEntry {
                original_path: path.to_string_lossy().into(),
                path,
                exists: true,
                kind: "schdoc".into(),
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
    app.prepare_sheet_sync(false).unwrap();
    (app, entry, port_id)
}

#[test]
fn sync_dialog_updates_counterparts_and_undo_restores_them() {
    let (mut app, entry, port) = fixture();
    let before = app.documents[1].model.clone();
    let mut dialog = app.sheet_sync.dialog.take().unwrap();
    dialog.entries.selected.insert(entry);
    dialog.ports.selected.insert(port);
    app.sync_action(&mut dialog, Action::CopyEntries).unwrap();
    let indices = app.sync_indices(&dialog.tabs[0]).unwrap();
    assert_eq!(indices, (0, 1));
    assert_eq!(
        sync::compare(
            &app.documents[0].model,
            &app.documents[1].model,
            dialog.tabs[0].symbol,
            &[]
        )
        .unwrap()
        .links
        .len(),
        1
    );
    assert_eq!(app.documents[1].history.revision, 1);
    let doc = &mut app.documents[1];
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model, before);
    dialog.ports.selected.insert(port);
    app.sync_action(&mut dialog, Action::DeletePorts).unwrap();
    assert!(
        app.documents[1]
            .model
            .schematic
            .as_ref()
            .unwrap()
            .primitive(port)
            .is_none()
    );
    let doc = &mut app.documents[1];
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model, before);
}

#[test]
fn table_drag_selects_the_range_from_pointer_press_origin() {
    for batched in [false, true] {
        let ctx = egui::Context::default();
        let signals: Vec<_> = (0..8)
            .map(|n| Signal {
                id: Uuid::new_v4(),
                name: format!("Row{n}"),
                io: 2,
                harness: String::new(),
            })
            .collect();
        let mut rows = Rows::default();
        let mut frame = |events| {
            ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(600.0, 600.0),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        table(ui, "rows", "", "Entries", &signals, &mut rows, 238.0, 324.0)
                    });
                },
            )
        };
        let mut output = frame(vec![]);
        for _ in 0..2 {
            output = frame(vec![]);
        }
        let at = |name: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.job.text == name => {
                        Some(t.pos + t.galley.rect.center().to_vec2())
                    }
                    _ => None,
                })
                .unwrap()
        };
        let first = at("Row0");
        let last = at("Row3");
        let event = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        if batched {
            let _ = frame(vec![
                egui::Event::PointerMoved(first),
                event(first, true),
                egui::Event::PointerMoved(last),
                event(last, false),
            ]);
        } else {
            let _ = frame(vec![egui::Event::PointerMoved(first), event(first, true)]);
            let _ = frame(vec![egui::Event::PointerMoved(last)]);
            let _ = frame(vec![event(last, false)]);
        }
        let divider = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.job.text == "IO" => Some(egui::pos2(
                    t.pos.x - 3.0,
                    (t.pos + t.galley.rect.center().to_vec2()).y,
                )),
                _ => None,
            })
            .unwrap();
        let moved = divider + egui::vec2(20.0, 0.0);
        if batched {
            let _ = frame(vec![
                egui::Event::PointerMoved(divider),
                event(divider, true),
                egui::Event::PointerMoved(moved),
                event(moved, false),
            ]);
        } else {
            let _ = frame(vec![
                egui::Event::PointerMoved(divider),
                event(divider, true),
            ]);
            let _ = frame(vec![egui::Event::PointerMoved(moved)]);
            let _ = frame(vec![event(moved, false)]);
        }
        assert_eq!(rows.selected, signals[..4].iter().map(|s| s.id).collect());
        assert!(rows.columns.unwrap()[0] > 238.0 * 0.5);
    }
}

#[test]
fn sheet_symbol_context_menu_opens_synchronization_without_losing_submenu_clicks() {
    let (mut app, _, _) = fixture();
    let owner = app.sheet_sync.dialog.as_ref().unwrap().tabs[0].symbol;
    app.sheet_sync.dialog = None;
    app.sheet_sync.popup = Some((0, owner, egui::pos2(20.0, 20.0)));
    let ctx = egui::Context::default();
    let mut frame = |events: Vec<egui::Event>| {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1920.0, 1080.0),
                )),
                focused: true,
                events,
                ..Default::default()
            },
            |ctx| app.sheet_sync_context_menu(ctx),
        )
    };
    for _ in 0..3 {
        let _ = frame(vec![]);
    }
    let event = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let at = egui::pos2(65.0, 32.0);
    let _ = frame(vec![egui::Event::PointerMoved(at), event(at, true)]);
    let _ = frame(vec![event(at, false)]);
    assert!(egui::Popup::is_any_open(&ctx));
    let output = frame(vec![]);
    let command = output
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t)
                if t.galley.job.text == "Synchronize Sheet Entries and Ports" =>
            {
                Some(t.pos + t.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .expect("synchronization submenu item is rendered");
    let _ = frame(vec![
        egui::Event::PointerMoved(command),
        event(command, true),
    ]);
    let _ = frame(vec![event(command, false)]);
    assert_eq!(app.sheet_sync.dialog.as_ref().unwrap().tabs.len(), 1);
    assert!(app.sheet_sync.popup.is_none());
}

#[test]
fn reverse_sync_preserves_entry_geometry_and_unlink_is_reversible_inside_dialog() {
    let (mut app, entry, port) = fixture();
    let before = app.documents[0].model.clone();
    let mut dialog = app.sheet_sync.dialog.take().unwrap();
    dialog.entries.selected.insert(entry);
    dialog.ports.selected.insert(port);
    app.sync_action(&mut dialog, Action::CopyPorts).unwrap();
    let value = app.documents[0]
        .model
        .schematic
        .as_ref()
        .unwrap()
        .primitive(entry)
        .unwrap();
    assert_eq!(Signal::read(value).unwrap().name, "Old");
    assert_eq!(Signal::read(value).unwrap().io, 1);
    assert!(Signal::read(value).unwrap().harness.is_empty());
    assert_eq!(
        edit::electrical_endpoint(value),
        edit::electrical_endpoint(before.schematic.as_ref().unwrap().primitive(entry).unwrap())
    );
    let revision = app.documents[0].history.revision;
    dialog.links.selected.insert(entry);
    app.sync_action(&mut dialog, Action::Unlink).unwrap();
    assert_eq!(dialog.excluded, vec![(entry, port)]);
    assert_eq!(app.documents[0].history.revision, revision);
    dialog.entries.selected.insert(entry);
    dialog.ports.selected.insert(port);
    app.sync_action(&mut dialog, Action::CopyPorts).unwrap();
    assert!(dialog.excluded.is_empty());
    dialog.links.selected.insert(entry);
    app.sync_action(&mut dialog, Action::Unlink).unwrap();
    dialog.ports.selected.insert(port);
    app.sync_action(&mut dialog, Action::AddEntries).unwrap();
    app.sheet_sync.dialog = Some(dialog);
    app.sheet_sync_escape();
    assert_eq!(
        app.sheet_sync.dialog.as_ref().unwrap().excluded,
        vec![(entry, port)]
    );
}

#[test]
fn synchronization_dialog_remains_bounded_with_many_links_and_escape_closes_it() {
    let (mut app, _, port) = fixture();
    let mut dialog = app.sheet_sync.dialog.take().unwrap();
    dialog.ports.selected.insert(port);
    app.sync_action(&mut dialog, Action::CopyPorts).unwrap();
    // Cancel the pending new Entry, then display the full matching list.
    app.sheet_sync.dialog = Some(dialog);
    app.sheet_sync_escape();
    assert!(app.sheet_sync.dialog.as_ref().unwrap().pending.is_none());
    let owner = app.sheet_sync.dialog.as_ref().unwrap().tabs[0].symbol;
    let signals: Vec<_> = (0..20)
        .map(|n| Signal {
            id: Uuid::new_v4(),
            name: format!("Link{n}"),
            io: 2,
            harness: String::new(),
        })
        .collect();
    for (index, destination, at) in [
        (0, Destination::Entries(owner), Point::mm(20, 75)),
        (1, Destination::Ports, Point::mm(100, 100)),
    ] {
        let model = &app.documents[index].model;
        let plan = sync::transfer(model, destination, &signals, &[]).unwrap();
        app.documents[index].model =
            edit::preview(model, &plan.edit_at(model, at).unwrap()).unwrap();
    }
    app.sheet_sync.dialog.as_mut().unwrap().only_unmatched = false;
    let ctx = egui::Context::default();
    for _ in 0..3 {
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1920.0, 1080.0),
                )),
                ..Default::default()
            },
            |ctx| app.sheet_sync_dialog(ctx),
        );
    }
    let rect = ctx
        .memory(|m| m.area_rect(egui::Id::new("sheet_entry_port_sync")))
        .unwrap();
    assert!(rect.width() <= 870.0 && rect.height() <= 540.0, "{rect:?}");
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
        |ctx| app.shortcuts(ctx),
    );
    assert!(!app.sheet_sync.busy());
}

#[test]
fn missing_counterparts_are_previewed_then_placed_or_cancelled_and_stale_sources_rejected() {
    let (mut app, entry, _) = fixture();
    let original = app.documents[1].model.clone();
    let ctx = egui::Context::default();
    let mut dialog = app.sheet_sync.dialog.take().unwrap();
    dialog.entries.selected.insert(entry);
    app.sync_action(&mut dialog, Action::AddPorts).unwrap();
    assert!(dialog.pending.is_some());
    app.sheet_sync.dialog = Some(dialog);
    let input = eda_render::SchematicInput {
        point: Some(Point::mm(50, 60)),
        ..Default::default()
    };
    assert!(app.sheet_sync_input(1, &input, &ctx));
    assert_eq!(app.documents[1].model, original);
    assert!(app.documents[1].view.sch_preview.is_some());
    app.cancel_sheet_sync_placement();
    assert_eq!(app.documents[1].model, original);
    assert!(app.documents[1].view.sch_preview.is_none());
    let mut dialog = app.sheet_sync.dialog.take().unwrap();
    app.sync_action(&mut dialog, Action::AddPorts).unwrap();
    app.sheet_sync.dialog = Some(dialog);
    let at = eda_render::SchematicInput {
        point: input.point,
        ..Default::default()
    };
    let _ = ctx.run(
        egui::RawInput {
            focused: true,
            events: vec![egui::Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        },
        |ctx| assert!(app.sheet_sync_input(1, &at, ctx)),
    );
    assert!(app.sheet_sync.dialog.as_ref().unwrap().pending.is_none());
    assert_eq!(app.documents[1].history.revision, 1);
    let ports: Vec<_> = app.documents[1]
        .model
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter_map(Signal::read)
        .collect();
    assert!(
        ports
            .iter()
            .any(|p| p.name == "Signal" && p.io == 2 && p.harness == "Harness")
    );
    let mut dialog = app.sheet_sync.dialog.take().unwrap();
    let doc = &mut app.documents[1];
    doc.history.undo(&mut doc.model);
    dialog.entries.selected.insert(entry);
    app.sync_action(&mut dialog, Action::AddPorts).unwrap();
    app.sheet_sync.dialog = Some(dialog);
    let revision = app.documents[0].history.revision;
    app.documents[0]
        .edit_schematic(
            edit::Edit::Text {
                id: entry,
                text: "Changed".into(),
            },
            revision,
        )
        .unwrap();
    let before = app.documents[1].model.clone();
    app.sheet_sync_input(1, &at, &ctx);
    assert_eq!(app.documents[1].model, before);
    assert!(
        app.sheet_sync
            .dialog
            .as_ref()
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("changed")
    );
}
