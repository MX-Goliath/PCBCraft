use super::*;
use egui::{Event, Modifiers, PointerButton};
fn app() -> PcbCraft {
    let mut app = crate::app::schematic_editor::tests::app();
    let mut model = Document::new(DocumentKind::Board, "ActiveBar.PcbDoc");
    model.pcb = Some(Board::from_legacy(&model));
    let mut doc = OpenDocument::new(model, None);
    doc.model.pcb.as_mut().unwrap().items.clear();
    doc.view.pcb.initialize(doc.model.pcb.as_ref().unwrap());
    app.documents = vec![doc];
    app.active = Some(0);
    app
}
fn canvas() -> Rect {
    Rect::from_min_max(egui::pos2(334., 78.), egui::pos2(1390., 970.))
}
fn frame(
    app: &mut PcbCraft,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1707., 1026.))),
            time: Some(time),
            focused: true,
            events,
            ..Default::default()
        },
        |ctx| {
            app.pcb_bar_shortcuts(0, ctx);
            app.pcb_active_bar(ctx, canvas());
        },
    )
}
fn mouse(at: Pos2, button: PointerButton, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(at),
        Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        },
    ]
}
fn button(g: usize) -> Pos2 {
    egui::pos2(
        canvas().center().x - 238. + 4. + (g as f32 + 0.5) * 100. / 3.,
        canvas().top() + 21.,
    )
}
fn text(output: &egui::FullOutput, label: &str) -> Pos2 {
    output
        .shapes
        .iter()
        .find_map(|s| {
            if let egui::Shape::Text(t) = &s.shape {
                (t.galley.job.text == label).then_some(t.pos + t.galley.rect.center().to_vec2())
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("Missing {label}"))
}
fn click(x: i64, y: i64) -> Input {
    Input {
        point: Some(Point::mm(x, y)),
        raw_point: Some(Point::mm(x, y)),
        clicked: true,
        hovered: true,
        ..Default::default()
    }
}

#[test]
fn menus_hold_release_last_used_and_keyboard_are_real_events() {
    let mut app = app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    frame(&mut app, &ctx, 0., vec![]);
    frame(&mut app, &ctx, 0.02, vec![]);
    let r = ctx
        .memory(|m| m.area_rect(egui::Id::new("active_bar")))
        .unwrap();
    assert!(
        (r.width() - 476.).abs() < 1. && (r.height() - 100. / 3.).abs() < 1.,
        "{r:?}"
    );
    frame(
        &mut app,
        &ctx,
        0.1,
        mouse(button(8), PointerButton::Primary, true),
    );
    frame(&mut app, &ctx, 1.2, vec![]);
    let output = frame(
        &mut app,
        &ctx,
        1.3,
        mouse(button(8), PointerButton::Primary, false),
    );
    assert_eq!(
        app.documents[0].pcb_editor.tool,
        Tool::Select,
        "Release after a hold must not also place the default via"
    );
    assert_eq!(app.documents[0].pcb_editor.bar.menu.unwrap().0, 8);
    let pad = text(&output, "Pad");
    frame(
        &mut app,
        &ctx,
        1.4,
        mouse(pad, PointerButton::Primary, true),
    );
    frame(
        &mut app,
        &ctx,
        1.5,
        mouse(pad, PointerButton::Primary, false),
    );
    assert_eq!(app.documents[0].pcb_editor.tool, Tool::Pad);
    assert_eq!(app.documents[0].pcb_editor.bar.last[8], 1);
    assert_eq!(app.settings.pcb_active_bar_last[8], 1);
    app.documents[0].pcb_editor.bar = State::default();
    frame(&mut app, &ctx, 1.55, vec![]);
    assert_eq!(app.documents[0].pcb_editor.bar.last[8], 1);
    let doc = &mut app.documents[0];
    doc.pcb_editor.cancel(&mut doc.view);
    frame(
        &mut app,
        &ctx,
        1.6,
        mouse(button(8), PointerButton::Primary, true),
    );
    frame(
        &mut app,
        &ctx,
        1.7,
        mouse(button(8), PointerButton::Primary, false),
    );
    assert_eq!(
        app.documents[0].pcb_editor.tool,
        Tool::Pad,
        "Short click repeats Pad, not Via"
    );
    frame(
        &mut app,
        &ctx,
        1.8,
        mouse(button(13), PointerButton::Secondary, true),
    );
    frame(
        &mut app,
        &ctx,
        1.9,
        mouse(button(13), PointerButton::Secondary, false),
    );
    for (time, key) in [(2., Key::ArrowDown), (2.1, Key::Enter)] {
        frame(
            &mut app,
            &ctx,
            time,
            vec![Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
    }
    assert_eq!(app.documents[0].pcb_editor.tool, Tool::Line);
    assert!(
        app.documents[0]
            .model
            .pcb
            .as_ref()
            .unwrap()
            .items
            .is_empty()
    );
    assert_eq!(app.documents[0].history.revision, 0);
    app.documents[0].pcb_editor.bar.menu = Some((1, egui::pos2(628., 117.)));
    frame(&mut app, &ctx, 2.2, vec![]);
    app.documents[0].pcb_editor.bar.menu = Some((13, egui::pos2(1062., 117.)));
    frame(&mut app, &ctx, 2.3, vec![]);
    let output = frame(&mut app, &ctx, 2.4, vec![]);
    let menu = ctx
        .memory(|m| m.area_rect(egui::Id::new("pcb_active_bar_menu")))
        .unwrap();
    assert!((menu.height() - 8. * 88. / 3.).abs() < 1., "{menu:?}");
    assert!(
        menu.contains(text(&output, "Rectangle")),
        "A taller menu must grow after the snap palette"
    );
}

#[test]
fn keepout_placement_flags_repeat_and_undo_and_rectangle_are_atomic() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.pcb_bar_command(0, "PlaceKeepoutFill", &ctx).unwrap();
    app.pcb_input(0, click(1, 1), &ctx);
    app.pcb_input(0, click(3, 4), &ctx);
    let doc = &mut app.documents[0];
    let b = doc.model.pcb.as_ref().unwrap();
    assert_eq!(b.items.len(), 1);
    assert!(b.items[0].keepout);
    assert_eq!(
        b.items[0].properties.get("KEEPOUTRESTRICTIONS").unwrap(),
        "31"
    );
    assert_eq!(b.items[0].net, None);
    assert_eq!(doc.history.revision, 1);
    doc.history.undo(&mut doc.model);
    assert!(doc.model.pcb.as_ref().unwrap().items.is_empty());
    app.pcb_bar_command(0, "PlaceRectangle", &ctx).unwrap();
    app.pcb_input(0, click(4, 5), &ctx);
    app.pcb_input(0, click(7, 9), &ctx);
    let doc = &mut app.documents[0];
    let b = doc.model.pcb.as_ref().unwrap();
    assert_eq!(b.items.len(), 4);
    assert!(b.items.iter().all(|i| !i.keepout));
    doc.history.undo(&mut doc.model);
    assert!(doc.model.pcb.as_ref().unwrap().items.is_empty());
}

#[test]
fn region_selection_filters_and_cancel_never_change_history() {
    let mut app = app();
    let ctx = egui::Context::default();
    let a = Item::new(
        pcb::TOP,
        None,
        Shape::Fill {
            start: Point::mm(1, 1),
            end: Point::mm(2, 2),
            rotation: 0.,
        },
    );
    let b = Item::new(
        pcb::TOP,
        None,
        Shape::Fill {
            start: Point::mm(3, 3),
            end: Point::mm(4, 4),
            rotation: 0.,
        },
    );
    let id = a.id;
    app.documents[0].model.pcb.as_mut().unwrap().items = vec![a, b];
    app.pcb_bar_command(0, "SelectInsideArea", &ctx).unwrap();
    app.pcb_input(0, click(0, 0), &ctx);
    app.pcb_input(0, click(2, 2), &ctx);
    assert_eq!(app.documents[0].view.pcb.selected, vec![id]);
    assert_eq!(app.documents[0].history.revision, 0);
    let doc = &mut app.documents[0];
    doc.pcb_editor.cancel(&mut doc.view);
    assert!(doc.pcb_editor.bar.region.is_none());
    assert!(doc.view.pcb.selection_box.is_none());
    assert!(!commands::enabled("AdvancedDiffPairRoute", doc));
    assert!(!commands::enabled("PlaceDimensionLinear", doc));
}

#[test]
fn native_arc_anchor_counts_and_rotation_reference_preserve_cancel_and_history() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.pcb_bar_command(0, "PlaceArcCenter", &ctx).unwrap();
    for p in [(5, 5), (8, 5), (5, 8)] {
        app.pcb_input(0, click(p.0, p.1), &ctx);
    }
    assert!(
        app.documents[0]
            .model
            .pcb
            .as_ref()
            .unwrap()
            .items
            .is_empty(),
        "Centre/radius/start requires a separate end click"
    );
    app.pcb_input(0, click(2, 5), &ctx);
    assert_eq!(app.documents[0].model.pcb.as_ref().unwrap().items.len(), 1);
    let id = app.documents[0].model.pcb.as_ref().unwrap().items[0].id;
    let doc = &mut app.documents[0];
    doc.pcb_editor.cancel(&mut doc.view);
    doc.view.pcb.selected = vec![id];
    app.pcb_bar_command(0, "RotateSelectedObjects", &ctx)
        .unwrap();
    app.documents[0]
        .pcb_editor
        .bar
        .dialog
        .as_mut()
        .unwrap()
        .angle = "90".into();
    frame(&mut app, &ctx, 0., vec![]);
    frame(&mut app, &ctx, 0.05, vec![]);
    let bounds = ctx
        .memory(|m| m.area_rect(egui::Id::new("pcb_bar_transform")))
        .unwrap();
    assert!(
        (bounds.width() - 258.).abs() < 1. && (bounds.height() - 111.).abs() < 1.,
        "{bounds:?}"
    );
    frame(
        &mut app,
        &ctx,
        0.1,
        vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    assert_eq!(
        app.documents[0].history.revision, 1,
        "Angle entry must not rotate about an invented pivot"
    );
    assert!(app.documents[0].pcb_editor.bar.rotate_pick.is_some());
    app.pcb_input(0, click(0, 0), &ctx);
    assert_eq!(app.documents[0].history.revision, 2);
    let b = app.documents[0].model.pcb.as_ref().unwrap();
    assert_eq!(b.items[0].anchor(), Point::mm(-5, 5));
}

#[test]
fn align_dialog_commits_both_axes_once_and_rejects_a_stale_snapshot() {
    let mut app = app();
    let ctx = egui::Context::default();
    let items: Vec<_> = [(1, 2, 2, 3), (7, 9, 9, 12)]
        .into_iter()
        .map(|(x, y, right, top)| {
            Item::new(
                pcb::TOP,
                None,
                Shape::Fill {
                    start: Point::mm(x, y),
                    end: Point::mm(right, top),
                    rotation: 0.,
                },
            )
        })
        .collect();
    app.documents[0].view.pcb.selected = items.iter().map(|i| i.id).collect();
    app.documents[0].model.pcb.as_mut().unwrap().items = items.clone();
    app.pcb_bar_command(0, "AlignComponentsDialog", &ctx)
        .unwrap();
    let d = app.documents[0]
        .pcb_editor
        .bar
        .align_dialog
        .as_mut()
        .unwrap();
    d.h = 1;
    d.v = 1;
    frame(&mut app, &ctx, 0., vec![]);
    frame(&mut app, &ctx, 0.05, vec![]);
    let bounds = ctx
        .memory(|m| m.area_rect(egui::Id::new("pcb_bar_align")))
        .unwrap();
    assert!(
        (bounds.width() - 310.).abs() < 1. && (bounds.height() - 240.).abs() < 1.,
        "{bounds:?}"
    );
    let enter = || {
        vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]
    };
    frame(&mut app, &ctx, 0.1, enter());
    let doc = &mut app.documents[0];
    assert!(doc.pcb_editor.bar.align_dialog.is_none());
    assert_eq!(doc.history.revision, 1);
    assert!(
        doc.model
            .pcb
            .as_ref()
            .unwrap()
            .items
            .iter()
            .all(|i| { i.bounds().min.x == 1_000_000 && i.bounds().max.y == 12_000_000 })
    );
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model.pcb.as_ref().unwrap().items, items);
    app.pcb_bar_command(0, "AlignComponentsDialog", &ctx)
        .unwrap();
    let revision = app.documents[0].history.revision;
    app.documents[0]
        .pcb_editor
        .bar
        .align_dialog
        .as_mut()
        .unwrap()
        .revision = revision.wrapping_sub(1);
    frame(&mut app, &ctx, 0.2, vec![]);
    frame(&mut app, &ctx, 0.3, enter());
    let doc = &mut app.documents[0];
    assert!(
        doc.pcb_editor
            .bar
            .align_dialog
            .as_ref()
            .unwrap()
            .error
            .is_some()
    );
    assert_eq!(doc.history.revision, revision);
    assert_eq!(doc.model.pcb.as_ref().unwrap().items, items);
    doc.pcb_editor.cancel(&mut doc.view);
    assert!(doc.pcb_editor.bar.align_dialog.is_none());
}

#[test]
fn measured_offsets_require_acceptance_and_undo_as_one_edit() {
    let mut app = app();
    app.settings.units = Units::Millimetres;
    let ctx = egui::Context::default();
    let item = Item::new(
        pcb::TOP,
        None,
        Shape::Fill {
            start: Point::mm(1, 1),
            end: Point::mm(2, 2),
            rotation: 0.,
        },
    );
    app.documents[0].view.pcb.selected = vec![item.id];
    app.documents[0]
        .model
        .pcb
        .as_mut()
        .unwrap()
        .items
        .push(item.clone());
    app.pcb_bar_command(0, "OffsetSelection", &ctx).unwrap();
    frame(&mut app, &ctx, 0., vec![]);
    let output = frame(&mut app, &ctx, 0.05, vec![]);
    let bounds = ctx
        .memory(|m| m.area_rect(egui::Id::new("pcb_bar_transform")))
        .unwrap();
    assert!(
        (bounds.width() - 282.).abs() < 1. && (bounds.height() - 228.).abs() < 1.,
        "{bounds:?}"
    );
    let measure = text(&output, "Define X/Y Offsets Interactively...");
    frame(
        &mut app,
        &ctx,
        0.1,
        mouse(measure, PointerButton::Primary, true),
    );
    frame(
        &mut app,
        &ctx,
        0.2,
        mouse(measure, PointerButton::Primary, false),
    );
    assert!(app.documents[0].pcb_editor.bar.offset_pick.is_some());
    app.pcb_input(0, click(1, 1), &ctx);
    app.pcb_input(0, click(3, 4), &ctx);
    let doc = &app.documents[0];
    assert_eq!(doc.history.revision, 0);
    assert_eq!(doc.model.pcb.as_ref().unwrap().items[0], item);
    let dialog = doc.pcb_editor.bar.dialog.as_ref().unwrap();
    assert_eq!(dialog.x, "2.000000mm");
    assert_eq!(dialog.y, "3.000000mm");
    frame(&mut app, &ctx, 0.3, vec![]);
    frame(
        &mut app,
        &ctx,
        0.4,
        vec![Event::Key {
            key: Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
    );
    let doc = &mut app.documents[0];
    assert_eq!(doc.history.revision, 1);
    assert_eq!(
        doc.model.pcb.as_ref().unwrap().items[0].anchor(),
        Point::mm(3, 4)
    );
    doc.history.undo(&mut doc.model);
    assert_eq!(doc.model.pcb.as_ref().unwrap().items[0], item);
}

#[test]
fn center_alignment_asks_each_axis_and_cancelling_keeps_confirmed_axes() {
    for cancel_second in [false, true] {
        let mut app = app();
        let ctx = egui::Context::default();
        let items: Vec<_> = [(1, 2), (7, 9), (15, 18)]
            .into_iter()
            .map(|(x, y)| {
                Item::new(
                    pcb::TOP,
                    None,
                    Shape::Fill {
                        start: Point::mm(x, y),
                        end: Point::mm(x + 2, y + 2),
                        rotation: 0.,
                    },
                )
            })
            .collect();
        let original = items.clone();
        let ids: Vec<_> = items.iter().map(|i| i.id).collect();
        app.documents[0].view.pcb.selected = ids.clone();
        app.documents[0].model.pcb.as_mut().unwrap().items = items;
        app.pcb_bar_command(0, "AlignComponentsDialog", &ctx)
            .unwrap();
        let d = app.documents[0]
            .pcb_editor
            .bar
            .align_dialog
            .as_mut()
            .unwrap();
        d.h = 2;
        d.v = 2;
        frame(&mut app, &ctx, 0., vec![]);
        frame(
            &mut app,
            &ctx,
            0.1,
            vec![Event::Key {
                key: Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }],
        );
        assert_eq!(
            app.documents[0].pcb_editor.selection_status(),
            Some("Select a Primitive")
        );
        app.pcb_input(
            0,
            Input {
                hit: Some(ids[1]),
                ..click(8, 10)
            },
            &ctx,
        );
        let doc = &app.documents[0];
        assert_eq!(doc.history.revision, 0);
        assert_eq!(doc.model.pcb.as_ref().unwrap().items, original);
        assert!(
            doc.view
                .pcb
                .preview()
                .unwrap()
                .items
                .iter()
                .all(|i| i.bounds().centre().x == 8_000_000)
        );
        assert_eq!(
            doc.pcb_editor
                .bar
                .align_pick
                .as_ref()
                .unwrap()
                .operations
                .len(),
            1
        );
        // Hovering the second reference must not become a confirmed move on cancel.
        app.pcb_input(
            0,
            Input {
                hit: Some(ids[2]),
                hovered: true,
                ..Default::default()
            },
            &ctx,
        );
        assert_eq!(
            app.documents[0]
                .view
                .pcb
                .preview()
                .unwrap()
                .items
                .iter()
                .map(|i| i.bounds().centre().y)
                .collect::<Vec<_>>(),
            original
                .iter()
                .map(|i| i.bounds().centre().y)
                .collect::<Vec<_>>(),
            "Native alignment does not move the next axis before its reference click"
        );
        app.pcb_input(
            0,
            Input {
                hit: Some(ids[2]),
                clicked: !cancel_second,
                secondary_clicked: cancel_second,
                ..Default::default()
            },
            &ctx,
        );
        let doc = &mut app.documents[0];
        assert_eq!(doc.history.revision, 1);
        assert!(doc.pcb_editor.bar.align_pick.is_none());
        assert!(!doc.view.pcb.tool_active);
        let changed = &doc.model.pcb.as_ref().unwrap().items;
        for (i, before) in changed.iter().zip(&original) {
            assert_eq!(i.bounds().centre().x, 8_000_000);
            assert_eq!(
                i.bounds().centre().y,
                if cancel_second {
                    before.bounds().centre().y
                } else {
                    19_000_000
                }
            );
        }
        doc.history.undo(&mut doc.model);
        assert_eq!(doc.model.pcb.as_ref().unwrap().items, original);
    }
}

#[test]
fn spacing_commands_are_grid_steps_and_each_undo_restores_the_board() {
    let mut app = app();
    let ctx = egui::Context::default();
    let items: Vec<_> = [(1, 2), (7, 9), (15, 18)]
        .into_iter()
        .map(|(x, y)| {
            Item::new(
                pcb::TOP,
                None,
                Shape::Fill {
                    start: Point::mm(x, y),
                    end: Point::mm(x + 2, y + 2),
                    rotation: 0.,
                },
            )
        })
        .collect();
    app.documents[0].view.pcb.selected = items.iter().map(|i| i.id).collect();
    app.documents[0].view.grid_nm = 1_000_000;
    app.documents[0].model.pcb.as_mut().unwrap().items = items.clone();
    for (command, vertical, direction) in [
        ("AlignComponentsExpandHorizontal", false, 1),
        ("AlignComponentsContractHorizontal", false, -1),
        ("AlignComponentsExpandVertical", true, 1),
        ("AlignComponentsContractVertical", true, -1),
    ] {
        app.pcb_bar_command(0, command, &ctx).unwrap();
        let doc = &mut app.documents[0];
        for (n, (before, after)) in items
            .iter()
            .zip(&doc.model.pcb.as_ref().unwrap().items)
            .enumerate()
        {
            let delta = n as i64 * 1_000_000 * direction;
            let mut expected = before.anchor();
            if vertical {
                expected.y += delta;
            } else {
                expected.x += delta;
            }
            assert_eq!(after.anchor(), expected);
        }
        doc.history.undo(&mut doc.model);
        assert_eq!(doc.model.pcb.as_ref().unwrap().items, items);
    }
}

#[test]
fn connection_commands_use_their_seed_mode_and_never_edit_the_board() {
    let mut app = app();
    let ctx = egui::Context::default();
    let mut component = pcb::Component {
        id: Uuid::new_v4(),
        index: 0,
        designator: "U1".into(),
        comment: String::new(),
        footprint: String::new(),
        footprint_parameters: None,
        parameters: None,
        position: Point::default(),
        rotation: 0.,
        layer: pcb::TOP,
        locked: false,
        properties: pcb::Fields::new(),
    };
    let owner = component.id;
    let mut terminal = component.clone();
    terminal.id = Uuid::new_v4();
    terminal.index = 1;
    terminal.designator = "U2".into();
    component.position = Point::mm(0, 0);
    let board = app.documents[0].model.pcb.as_mut().unwrap();
    board.components = vec![component, terminal];
    board.nets = vec![pcb::Net {
        index: 0,
        name: "Signal".into(),
        properties: pcb::Fields::new(),
    }];
    board.items = [0, 10]
        .into_iter()
        .enumerate()
        .map(|(owner, x)| {
            let mut pad = Item::new(
                pcb::MULTI_LAYER,
                Some(0),
                Shape::Pad {
                    position: Point::mm(x, 0),
                    size: Point::mm(2, 2),
                    rotation: 0.,
                    shape: pcb::PadShape::Round,
                    hole: 600_000,
                    plated: true,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                },
            );
            pad.component = Some(owner as u32);
            pad
        })
        .collect();
    board.items.extend([
        Item::new(
            pcb::TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(0, 0),
                end: Point::mm(5, 0),
                width: 400_000,
            },
        ),
        Item::new(
            pcb::MULTI_LAYER,
            Some(0),
            Shape::Via {
                position: Point::mm(5, 0),
                diameter: 1_000_000,
                hole: 400_000,
                from_layer: pcb::TOP,
                to_layer: pcb::BOTTOM,
            },
        ),
        Item::new(
            pcb::BOTTOM,
            Some(0),
            Shape::Track {
                start: Point::mm(5, 0),
                end: Point::mm(10, 0),
                width: 400_000,
            },
        ),
    ]);
    let track = board.items[2].id;
    let expected_route: Vec<_> = board.items[2..].iter().map(|i| i.id).collect();
    let original = board.clone();
    for (command, seed, expected) in [
        ("SelectPhysicalConnection", track, expected_route.clone()),
        ("SelectPhysicalConnectionOnLayer", track, vec![track]),
        ("SelectComponentConnections", owner, expected_route),
        (
            "SelectComponentNets",
            owner,
            original.items.iter().map(|i| i.id).collect(),
        ),
    ] {
        app.pcb_bar_command(0, command, &ctx).unwrap();
        app.pcb_input(
            0,
            Input {
                clicked: true,
                hit: Some(Uuid::nil()),
                ..Default::default()
            },
            &ctx,
        );
        assert!(app.documents[0].pcb_editor.selection_pick.is_some());
        app.pcb_input(
            0,
            Input {
                clicked: true,
                hit: Some(seed),
                ..Default::default()
            },
            &ctx,
        );
        let doc = &app.documents[0];
        assert_eq!(doc.view.pcb.selected, expected);
        assert_eq!(doc.model.pcb.as_ref().unwrap(), &original);
        assert_eq!(doc.history.revision, 0);
        assert!(doc.view.pcb.tool_active);
        app.pcb_input(
            0,
            Input {
                secondary_clicked: true,
                ..Default::default()
            },
            &ctx,
        );
        assert!(app.documents[0].pcb_editor.selection_pick.is_none());
        assert!(!app.documents[0].view.pcb.tool_active);
    }
}
