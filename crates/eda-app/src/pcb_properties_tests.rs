use super::*;

fn app() -> PcbCraft {
    let model = Document::new(DocumentKind::Board, "Properties test");
    let board = Board::from_legacy(&model);
    let mut model = model;
    model.pcb = Some(board);
    PcbCraft {
        cross_probe: Default::default(),
        file_dialog: rfd::FileDialog::new(),
        documents: vec![OpenDocument::new(model, None)],
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
        allow_exit: false,
        cursor_mm: None,
        pad_edit: None,
        property_ui: Default::default(),
        sch_clipboard: None,
        pcb_clipboard: None,
        components_ui: Default::default(),
    }
}
fn track() -> Item {
    Item::new(
        pcb::TOP,
        None,
        Shape::Track {
            start: Point::mm(1, 2),
            end: Point::mm(5, 2),
            width: 200_000,
        },
    )
}
#[test]
fn leaving_a_field_for_board_or_mixed_selection_commits_once_and_undo_restores() {
    for mixed in [false, true] {
        let mut app = app();
        let doc = &mut app.documents[0];
        let item = track();
        let id = item.id;
        doc.model.pcb.as_mut().unwrap().items.push(item.clone());
        let before = doc.model.clone();
        let mut changed = item.clone();
        if let Shape::Track { width, .. } = &mut changed.shape {
            *width = 350_000;
        }
        doc.pcb_editor.draft = Some(Draft {
            id,
            revision: 0,
            item: changed.clone(),
        });
        doc.view.pcb.selected = if mixed {
            vec![id, Uuid::new_v4()]
        } else {
            vec![]
        };
        let mut ed = std::mem::take(&mut doc.pcb_editor);
        flush_pending(doc, &mut ed, false).unwrap();
        flush_pending(doc, &mut ed, false).unwrap();
        assert_eq!(doc.history.revision, 1);
        assert_eq!(doc.model.pcb.as_ref().unwrap().item(id), Some(&changed));
        doc.pcb_editor = ed;
        app.run(Command::Undo, &egui::Context::default());
        assert_eq!(app.documents[0].model, before);
        app.run(Command::Redo, &egui::Context::default());
        assert_eq!(
            app.documents[0].model.pcb.as_ref().unwrap().item(id),
            Some(&changed)
        );
    }
}
#[test]
fn invalid_and_stale_drafts_cannot_change_model_or_history() {
    let mut app = app();
    let doc = &mut app.documents[0];
    let item = track();
    let id = item.id;
    doc.model.pcb.as_mut().unwrap().items.push(item.clone());
    let before = doc.model.clone();
    let mut bad = item.clone();
    if let Shape::Track { width, .. } = &mut bad.shape {
        *width = -1;
    }
    let mut ed = Editor {
        draft: Some(Draft {
            id,
            revision: 0,
            item: bad,
        }),
        ..Default::default()
    };
    assert!(flush_pending(doc, &mut ed, false).is_err());
    assert_eq!(doc.model, before);
    assert_eq!(doc.history.revision, 0);
    ed.draft = Some(Draft {
        id,
        revision: 99,
        item: item.with_anchor(Point::mm(7, 8)).unwrap(),
    });
    flush_pending(doc, &mut ed, false).unwrap();
    assert_eq!(doc.model, before);
    assert_eq!(doc.history.revision, 0);
}

#[test]
fn save_commits_focused_properties_and_pending_values_protect_close() {
    let mut app = app();
    let doc = &mut app.documents[0];
    let item = track();
    let id = item.id;
    doc.model.pcb.as_mut().unwrap().items.push(item.clone());
    doc.saved = Some(doc.model.clone());
    doc.view.pcb.selected = vec![id];
    assert!(!doc.dirty());
    let mut changed = item;
    if let Shape::Track { width, .. } = &mut changed.shape {
        *width = 400_000;
    }
    doc.pcb_editor.draft = Some(Draft {
        id,
        revision: 0,
        item: changed.clone(),
    });
    assert!(doc.dirty());
    let session = doc.session_id;
    app.request_close(session);
    assert!(matches!(app.pending,Some(Pending::Close(id)) if id==session));
    assert_eq!(app.documents.len(), 1);
    let doc = &mut app.documents[0];
    prepare_save(doc).unwrap();
    assert_eq!(doc.history.revision, 1);
    assert_eq!(doc.model.pcb.as_ref().unwrap().item(id), Some(&changed));
    prepare_save(doc).unwrap();
    assert_eq!(doc.history.revision, 1);
    doc.saved = Some(doc.model.clone());
    assert!(!doc.dirty());
}
#[test]
fn relative_origin_is_unit_aware_and_location_preserves_track_dimensions() {
    let mut board = Board::default();
    board.properties.insert("ORIGINX".into(), "1000mil".into());
    board.properties.insert("ORIGINY".into(), "12.5mm".into());
    assert_eq!(
        origin(&board),
        Point {
            x: 25_400_000,
            y: 12_500_000
        }
    );
    let item = track();
    let moved = item.with_anchor(Point::mm(10, 20)).unwrap();
    assert_eq!(length(&item), length(&moved));
    assert!(item.with_anchor(Point { x: i64::MAX, y: 0 }).is_err());
    assert!(
        matches!(moved.shape,Shape::Track{start,end,..} if start==Point::mm(10,20)&&end==Point::mm(14,20))
    );
}
#[test]
fn every_properties_context_stays_within_native_dock_width() {
    let mut app = app();
    let b = app.documents[0].model.pcb.as_mut().unwrap();
    let long =
        "Very long imported field name or library path which must stay inside Properties".repeat(3);
    b.nets.push(pcb::Net {
        index: 0,
        name: long.clone(),
        properties: Default::default(),
    });
    let mut t = track();
    t.net = Some(0);
    b.items.push(t);
    for shape in [
        Shape::Pad {
            position: Point::mm(1, 1),
            size: Point::mm(2, 2),
            rotation: 0.0,
            shape: PadShape::Round,
            hole: 500_000,
            plated: true,
            name: long.clone(),
            bottom_size: None,
            middle_size: None,
        },
        Shape::Via {
            position: Point::mm(3, 3),
            diameter: 600_000,
            hole: 300_000,
            from_layer: pcb::TOP,
            to_layer: pcb::BOTTOM,
        },
        Shape::Text {
            position: Point::default(),
            text: long.clone(),
            height: 1_000_000,
            stroke: 200_000,
            rotation: 0.0,
            mirrored: false,
            font: long.clone(),
        },
        Shape::Arc {
            centre: Point::mm(2, 2),
            radius: 1_000_000,
            start_angle: 0.0,
            end_angle: 90.0,
            width: 200_000,
        },
        Shape::Fill {
            start: Point::mm(1, 1),
            end: Point::mm(2, 2),
            rotation: 0.0,
        },
        Shape::Region {
            contour: vec![Point::mm(1, 1), Point::mm(2, 1), Point::mm(1, 2)],
            holes: vec![],
            cutout: false,
        },
    ] {
        b.items.push(Item::new(pcb::TOP, Some(0), shape));
    }
    b.components.push(pcb::Component {
        id: Uuid::new_v4(),
        index: 0,
        designator: long.clone(),
        comment: long.clone(),
        footprint: long.clone(),
        position: Point::default(),
        rotation: 0.0,
        layer: pcb::TOP,
        locked: false,
        properties: pcb::Fields::from([
            ("SOURCEFOOTPRINTLIBRARY".into(), long.clone()),
            ("FOOTPRINTDESCRIPTION".into(), long.clone()),
        ]),
    });
    b.polygon_pours.push(Polygon {
        name: long,
        contour: pcb_polygon::straight_contour(&[
            Point::mm(0, 0),
            Point::mm(10, 0),
            Point::mm(0, 10),
        ]),
        ..Default::default()
    });
    let mut contexts = vec![vec![]];
    contexts.extend(b.items.iter().map(|i| vec![i.id]));
    contexts.extend(b.components.iter().map(|c| vec![c.id]));
    contexts.extend(b.polygon_pours.iter().map(|p| vec![p.id]));
    contexts.push(b.items.iter().map(|i| i.id).collect());
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    for selected in contexts {
        app.documents[0].view.pcb.selected = selected.clone();
        for _ in 0..60 {
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1707.0, 1027.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    let panel = egui::SidePanel::right("pcb_properties_width")
                        .default_width(316.0)
                        .show(ctx, |ui| {
                            show(&mut app, ui);
                        });
                    assert!(
                        panel.response.rect.width() <= 317.0,
                        "Context {selected:?} expanded to {}",
                        panel.response.rect.width()
                    );
                },
            );
        }
    }
}

#[test]
fn numeric_fields_commit_on_enter_and_escape_reverts_in_mm_and_mils() {
    for (units, typed, expected) in [
        (Units::Millimetres, "0.8", 800_000),
        (Units::Mils, "40", 1_016_000),
    ] {
        for cancel in [false, true] {
            let ctx = egui::Context::default();
            theme::apply(&ctx);
            let mut value = 700_000;
            let mut done = false;
            let mut frame = |events: Vec<egui::Event>| {
                let _ = ctx.run(
                    egui::RawInput {
                        focused: true,
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(320.0, 100.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .show(ctx, |ui| done |= numeric(ui, &mut value, units));
                    },
                );
            };
            frame(vec![]);
            frame(vec![]);
            let pos = egui::pos2(50.0, 18.0);
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Default::default(),
                },
            ]);
            frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }]);
            frame(vec![]);
            frame(vec![egui::Event::Text(typed.into())]);
            frame(vec![egui::Event::Key {
                key: if cancel { Key::Escape } else { Key::Enter },
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }]);
            frame(vec![]);
            frame(vec![]);
            assert_eq!(
                value,
                if cancel { 700_000 } else { expected },
                "units={units:?}, cancel={cancel}"
            );
            assert_eq!(done, !cancel, "units={units:?}, cancel={cancel}");
        }
    }
}

#[test]
fn pcb_shortcut_sequences_and_inactive_drag_cleanup_are_routed_to_active_document() {
    let mut app = app();
    app.documents[0].saved = Some(app.documents[0].model.clone());
    let ctx = egui::Context::default();
    fn press(app: &mut PcbCraft, ctx: &egui::Context, key: Key, mods: egui::Modifiers) {
        let _ = ctx.run(
            egui::RawInput {
                focused: true,
                events: vec![
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: mods,
                    },
                    egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: false,
                        repeat: false,
                        modifiers: mods,
                    },
                ],
                modifiers: mods,
                ..Default::default()
            },
            |ctx| app.shortcuts(ctx),
        );
    }
    for (key, tool) in [(Key::M, Tool::Move), (Key::D, Tool::Drag)] {
        press(&mut app, &ctx, Key::M, egui::Modifiers::NONE);
        press(&mut app, &ctx, key, egui::Modifiers::NONE);
        assert_eq!(app.documents[0].pcb_editor.tool, tool);
        press(&mut app, &ctx, Key::Escape, egui::Modifiers::NONE);
        assert_eq!(app.documents[0].pcb_editor.tool, Tool::Select);
    }
    press(&mut app, &ctx, Key::V, egui::Modifiers::NONE);
    press(&mut app, &ctx, Key::F, egui::Modifiers::NONE);
    assert!(matches!(
        app.documents[0].view.pcb.requested_fit,
        Some(eda_render::pcb::FitTarget::Board)
    ));
    press(&mut app, &ctx, Key::Home, egui::Modifiers::CTRL);
    assert!(matches!(
        app.documents[0].view.pcb.requested_fit,
        Some(eda_render::pcb::FitTarget::Selection)
    ));
    press(&mut app, &ctx, Key::PageDown, egui::Modifiers::CTRL);
    assert!(matches!(
        app.documents[0].view.pcb.requested_fit,
        Some(eda_render::pcb::FitTarget::Document)
    ));
    let doc = &mut app.documents[0];
    let before = doc.model.clone();
    doc.pcb_editor.gesture = Some(Gesture::Box {
        origin: Point::default(),
        add: false,
    });
    doc.view.pcb.selection_box = Some((Point::default(), Point::mm(5, 5)));
    doc.view.pcb.set_preview(doc.model.pcb.clone());
    app.active = None;
    app.finish_pcb_interaction(&ctx);
    let doc = &app.documents[0];
    assert!(doc.pcb_editor.gesture.is_none());
    assert!(doc.view.pcb.selection_box.is_none());
    assert!(doc.view.pcb.preview().is_none());
    assert_eq!(doc.model, before);
    assert!(!doc.dirty());
}

#[test]
fn three_d_mode_shortcuts_selection_rotation_and_return_preserve_history_and_camera() {
    let mut app = app();
    let ctx = egui::Context::default();
    let id = Uuid::new_v4();
    let d = &mut app.documents[0];
    d.model
        .pcb
        .as_mut()
        .unwrap()
        .components
        .push(pcb::Component {
            id,
            index: 0,
            designator: "U1".into(),
            comment: String::new(),
            footprint: String::new(),
            position: Point::mm(10, 10),
            rotation: 0.,
            layer: pcb::TOP,
            locked: false,
            properties: Default::default(),
        });
    d.saved = Some(d.model.clone());
    d.view.camera.centre_mm = [12., 34.];
    d.view.camera.pixels_per_mm = 7.;
    let event = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let _ = ctx.run(
        egui::RawInput {
            events: vec![event(Key::Num3)],
            ..Default::default()
        },
        |ctx| {
            assert!(app.pcb_view_shortcuts(ctx));
        },
    );
    assert!(app.documents[0].view.pcb.three_d.enabled);
    app.pcb_input(
        0,
        Input {
            hit: Some(id),
            clicked: true,
            double_clicked: true,
            ..Default::default()
        },
        &ctx,
    );
    assert_eq!(app.documents[0].view.pcb.selected, vec![id]);
    assert!(!app.documents[0].dirty());
    assert_eq!(app.documents[0].history.revision, 0);
    app.pcb_action(Action::Rotate);
    assert_eq!(
        app.documents[0].model.pcb.as_ref().unwrap().components[0].rotation,
        90.
    );
    assert_eq!(
        app.documents[0].model.pcb.as_ref().unwrap().components[0].position,
        Point::mm(10, 10)
    );
    assert_eq!(app.documents[0].history.revision, 1);
    assert!(app.documents[0].view.pcb.three_d.enabled);
    app.run(Command::Undo, &ctx);
    assert_eq!(
        app.documents[0].model.pcb.as_ref().unwrap().components[0].rotation,
        0.
    );
    let _ = ctx.run(
        egui::RawInput {
            events: vec![event(Key::Num2)],
            ..Default::default()
        },
        |ctx| {
            assert!(app.pcb_view_shortcuts(ctx));
        },
    );
    assert!(!app.documents[0].view.pcb.three_d.enabled);
    assert_eq!(app.documents[0].view.camera.centre_mm, [12., 34.]);
    assert_eq!(app.documents[0].view.camera.pixels_per_mm, 7.);
    assert!(!app.documents[0].dirty());
}

#[test]
fn entering_three_d_rejects_pending_gesture_and_two_d_noop_keeps_it() {
    let mut app = app();
    app.documents[0].pcb_editor.gesture = Some(Gesture::Box {
        origin: Point::mm(1, 1),
        add: false,
    });
    app.set_pcb_3d(true);
    assert!(!app.documents[0].view.pcb.three_d.enabled);
    assert!(app.documents[0].pcb_editor.gesture.is_some());
    app.set_pcb_3d(false);
    assert!(app.documents[0].pcb_editor.gesture.is_some());
    assert_eq!(app.documents[0].history.revision, 0);
}
