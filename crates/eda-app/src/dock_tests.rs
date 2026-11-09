use super::*;

fn frame(app: &mut PcbCraft, ctx: &egui::Context, events: Vec<egui::Event>) {
    frame_with_properties(app, ctx, events, |app, ui| app.normal_properties(ui));
}
fn frame_with_properties(
    app: &mut PcbCraft,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    mut properties: impl FnMut(&mut PcbCraft, &mut egui::Ui),
) {
    let _ = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1440., 900.))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.prepare_dock_input(ctx);
            app.left_dock_with_properties(ctx, &mut properties);
            app.right_dock_with_properties(ctx, &mut properties);
            egui::CentralPanel::default().show(ctx, |_| {});
            app.finish_docks_with_properties(ctx, &mut properties);
        },
    );
}

#[test]
fn oversized_properties_do_not_enlarge_the_dock_or_undo_a_pointer_resize() {
    fn oversized(_: &mut PcbCraft, ui: &mut egui::Ui) {
        // A selected component can have long fields or a wide parameter table.
        // Even explicitly oversized content must not negotiate the dock width.
        ui.set_min_width(2400.);
        ui.label("Component parameter value".repeat(100));
    }
    for dpi in [1., 1.5, 2.] {
        let mut app = crate::app::schematic_editor::tests::app();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        ctx.set_pixels_per_point(dpi);
        for _ in 0..4 {
            frame_with_properties(&mut app, &ctx, vec![], oversized);
            let root = app
                .settings
                .dock_layout
                .as_ref()
                .unwrap()
                .root_for(Panel::Properties)
                .unwrap();
            assert!(
                (root.extent - 316.).abs() < 1.,
                "Content expanded the dock: {}",
                root.extent
            );
        }
        let rect = app
            .dock_runtime
            .groups
            .iter()
            .find(|g| g.id == 4)
            .unwrap()
            .rect;
        let from = rect.left_center();
        let to = from + vec2(110., 0.);
        for events in [
            vec![
                egui::Event::PointerMoved(from),
                egui::Event::PointerButton {
                    pos: from,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            vec![egui::Event::PointerMoved(to)],
            vec![egui::Event::PointerButton {
                pos: to,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ] {
            frame_with_properties(&mut app, &ctx, events, oversized);
        }
        for _ in 0..8 {
            frame_with_properties(&mut app, &ctx, vec![], oversized);
            let root = app
                .settings
                .dock_layout
                .as_ref()
                .unwrap()
                .root_for(Panel::Properties)
                .unwrap();
            assert!(
                (root.extent - 206.).abs() < 2.,
                "Resize sprang back: {}",
                root.extent
            );
        }
    }
}

#[test]
fn long_component_fields_keep_properties_width_when_switching_schematic_and_board() {
    let mut app = crate::app::schematic_editor::tests::app();
    let long = "VeryLongImportedComponentValueOrLibraryPath_".repeat(100);
    let id = Uuid::new_v4();
    app.documents[0]
        .model
        .schematic
        .as_mut()
        .unwrap()
        .components
        .push(eda_core::schematic::Component {
            id,
            source_index: 0,
            position: Default::default(),
            designator: "U1".into(),
            comment: long.clone(),
            library_reference: long.clone(),
            description: long.clone(),
            part: 1,
            display_mode: 0,
            parameters: [
                ("Comment".into(), long.clone()),
                (long.clone(), long.clone()),
            ]
            .into(),
            settings: Default::default(),
        });
    app.documents[0].view.selected = Some(id);
    app.add_document(Document::new(DocumentKind::Board, "Wide component"), None);
    let id = Uuid::new_v4();
    app.documents[1]
        .model
        .pcb
        .as_mut()
        .unwrap()
        .components
        .push(eda_core::pcb::Component {
            id,
            index: 0,
            designator: "U1".into(),
            comment: long.clone(),
            footprint: long.clone(),
            footprint_parameters: None,
            parameters: Some(Default::default()),
            position: Default::default(),
            rotation: 0.,
            layer: eda_core::pcb::TOP,
            locked: false,
            properties: [
                ("SOURCEFOOTPRINTLIBRARY".into(), long.clone()),
                ("FOOTPRINTDESCRIPTION".into(), long),
            ]
            .into(),
        });
    app.documents[1].view.pcb.selected = vec![id];
    let models: Vec<_> = app.documents.iter().map(|d| d.model.clone()).collect();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    for active in [0, 1, 0, 1] {
        app.active = Some(active);
        for _ in 0..4 {
            frame(&mut app, &ctx, vec![]);
            let root = app
                .settings
                .dock_layout
                .as_ref()
                .unwrap()
                .root_for(Panel::Properties)
                .unwrap();
            assert!(
                (root.extent - 316.).abs() < 1.,
                "Document {active} expanded Properties: {}",
                root.extent
            );
        }
    }
    for (doc, before) in app.documents.iter().zip(models) {
        assert_eq!(
            doc.model, before,
            "Rendering properties edited the document"
        );
    }
}
fn drag(app: &mut PcbCraft, ctx: &egui::Context, from: Pos2, to: Pos2) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(from),
            egui::Event::PointerButton {
                pos: from,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerMoved(from + vec2(8., 0.))],
    );
    frame(app, ctx, vec![egui::Event::PointerMoved(to)]);
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    frame(app, ctx, vec![]);
}
fn assert_membership(layout: &Layout) {
    let all: Vec<_> = layout.roots.iter().flat_map(|r| r.tree.panels()).collect();
    assert_eq!(all.len(), Panel::ALL.len());
    for p in Panel::ALL {
        assert_eq!(all.iter().filter(|q| **q == p).count(), 1, "{p:?}");
    }
}
#[test]
fn pointer_drag_merges_splits_and_docks_on_all_four_edges() {
    let mut app = crate::app::schematic_editor::tests::app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    frame(&mut app, &ctx, vec![]);
    let left = app
        .dock_runtime
        .groups
        .iter()
        .find(|g| g.id == 2)
        .unwrap()
        .rect;
    let right = app
        .dock_runtime
        .groups
        .iter()
        .find(|g| g.id == 4)
        .unwrap()
        .rect;
    drag(&mut app, &ctx, left.min + vec2(30., 12.), right.center());
    assert_eq!(
        app.settings
            .dock_layout
            .as_ref()
            .unwrap()
            .root_for(Panel::Projects)
            .unwrap()
            .edge,
        Some(Side::Right)
    );
    assert!(
        matches!(app.settings.dock_layout.as_ref().unwrap().group(4),Some(Node::Group{tabs,..}) if tabs.contains(&Panel::Projects))
    );
    // Tear one tab into a recursive split, then move it to each application edge.
    let group = app.dock_runtime.groups.iter().find(|g| g.id == 4).unwrap();
    let from = group
        .tabs
        .iter()
        .find(|(p, _)| *p == Panel::Projects)
        .unwrap()
        .1
        .center();
    let to = arrow_rect(group.rect, Side::Bottom).center();
    drag(&mut app, &ctx, from, to);
    assert!(matches!(
        app.settings
            .dock_layout
            .as_ref()
            .unwrap()
            .root_for(Panel::Projects)
            .unwrap()
            .tree,
        Node::Split { .. }
    ));
    for side in [Side::Top, Side::Bottom, Side::Left, Side::Right] {
        let group = app
            .dock_runtime
            .groups
            .iter()
            .find(|g| {
                app.settings
                    .dock_layout
                    .as_ref()
                    .unwrap()
                    .group(g.id)
                    .unwrap()
                    .panels()
                    .contains(&Panel::Projects)
            })
            .unwrap();
        let from = group.rect.min + vec2(30., 12.);
        let main = app.dock_runtime.main_rect;
        let to = match side {
            Side::Left => main.left_center() + vec2(3., 0.),
            Side::Right => main.right_center() - vec2(3., 0.),
            Side::Top => main.center_top() + vec2(0., 3.),
            Side::Bottom => main.center_bottom() - vec2(0., 3.),
        };
        drag(&mut app, &ctx, from, to);
        let layout = app.settings.dock_layout.as_ref().unwrap();
        assert_eq!(layout.root_for(Panel::Projects).unwrap().edge, Some(side));
        assert_membership(layout);
    }
}
#[test]
fn native_bounds_hidden_members_and_order_survive_settings_roundtrip() {
    let mut app = crate::app::schematic_editor::tests::app();
    app.show_panel(Panel::Git);
    app.show_panel(Panel::Changes);
    let layout = app.settings.dock_layout.as_mut().unwrap();
    layout.move_panels(
        &[Panel::Git, Panel::Changes],
        Target::Float([-180., 220., 420., 510.]),
    );
    let root = layout.root_for(Panel::Git).unwrap();
    let root_id = root.id;
    app.close_panel(Panel::Git);
    app.save_dock_visibility();
    app.settings = serde_json::from_str(&serde_json::to_string(&app.settings).unwrap()).unwrap();
    app.git.details_open = false;
    app.restore_dock_visibility();
    assert!(!app.panel_open(Panel::Git));
    assert!(app.panel_open(Panel::Changes));
    app.show_panel(Panel::Git);
    let layout = app.settings.dock_layout.as_ref().unwrap();
    let root = layout.root_for(Panel::Git).unwrap();
    assert_eq!(root.id, root_id);
    assert_eq!(root.bounds, [-180., 220., 420., 510.]);
    assert_eq!(root.tree.panels(), vec![Panel::Git, Panel::Changes]);
    assert_membership(layout);
    app.restore_docks();
    assert_membership(app.settings.dock_layout.as_ref().unwrap());
    assert_eq!(app.settings.dock_layout.as_ref().unwrap().roots.len(), 2);
}
#[test]
fn ctrl_and_escape_prevent_docking_and_do_not_mutate_layout() {
    let mut app = crate::app::schematic_editor::tests::app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    frame(&mut app, &ctx, vec![]);
    let original = serde_json::to_string(&app.settings.dock_layout).unwrap();
    app.dock_runtime.drag = Some(Drag {
        panels: vec![Panel::Projects],
        source: egui::ViewportId::ROOT,
        pointer: pos2(1300., 300.),
        size: vec2(334., 600.),
        offset: vec2(30., 12.),
        released: false,
        cancel: false,
        ctrl: true,
    });
    assert!(app.dock_drop_target().is_none());
    app.dock_runtime.drag.as_mut().unwrap().cancel = true;
    app.dock_runtime.finished = false;
    app.finish_docks(&ctx);
    assert!(app.dock_runtime.drag.is_none());
    assert_eq!(
        original,
        serde_json::to_string(&app.settings.dock_layout).unwrap()
    );
}
#[test]
fn tab_reorder_and_nested_group_removal_do_not_lose_other_panels() {
    let mut l = Layout::default();
    l.move_panels(&[Panel::Git], Target::Tab(2, 0));
    assert_eq!(l.group(2).unwrap().panels()[0], Panel::Git);
    l.move_panels(&[Panel::Git], Target::Split(4, Side::Left));
    let g = match &l.root_for(Panel::Git).unwrap().tree {
        Node::Split { first, .. } => match **first {
            Node::Group { id, .. } => id,
            _ => panic!(),
        },
        _ => panic!(),
    };
    l.move_panels(&[Panel::Changes], Target::Split(g, Side::Bottom));
    assert_membership(&l);
    l.move_panels(&[Panel::Git, Panel::Changes], Target::Tab(2, 0));
    assert_membership(&l);
    assert!(matches!(
        l.root_for(Panel::Properties).unwrap().tree,
        Node::Group { .. }
    ));
}
#[test]
fn document_switching_never_replaces_or_closes_open_panels() {
    let mut app = crate::app::schematic_editor::tests::app();
    app.add_document(Document::new(DocumentKind::Board, "Board"), None);
    app.show_panel(Panel::Git);
    app.show_panel(Panel::ViewConfiguration);
    app.show_panel(Panel::Changes);
    for active in [Some(0), Some(1), None, Some(1)] {
        app.active = active;
        assert_eq!(
            app.selected_panel(Side::Left),
            Some(Panel::ViewConfiguration)
        );
        assert_eq!(app.selected_panel(Side::Right), Some(Panel::Changes));
        assert!(app.panel_open(Panel::Git));
        assert!(app.panel_open(Panel::Projects));
    }
    // Showing an already open tab never toggles it off or closes its peers.
    app.run(Command::ShowPanel(Panel::Git), &egui::Context::default());
    app.run(Command::ShowPanel(Panel::Git), &egui::Context::default());
    assert_eq!(app.selected_panel(Side::Left), Some(Panel::Git));
    assert!(app.panel_open(Panel::ViewConfiguration));
    app.show_panel(Panel::ViewConfiguration);
    app.close_panel(Panel::ViewConfiguration);
    assert_eq!(app.selected_panel(Side::Left), Some(Panel::Git));
    app.show_panel(Panel::Properties);
    app.close_panel(Panel::Changes);
    assert_eq!(app.selected_panel(Side::Right), Some(Panel::Properties));
    assert!(app.panel_open(Panel::Git));
    let restored: Settings =
        serde_json::from_str(&serde_json::to_string(&app.settings).unwrap()).unwrap();
    app.settings = restored;
    assert_eq!(app.selected_panel(Side::Left), Some(Panel::Git));
    app.restore_docks();
    assert_eq!(app.selected_panel(Side::Left), Some(Panel::Projects));
    assert_eq!(app.selected_panel(Side::Right), Some(Panel::Properties));
    assert!(!app.panel_open(Panel::Git));
    assert!(!app.panel_open(Panel::ViewConfiguration));
}

#[test]
fn dock_layout_keeps_tabs_and_widths_on_sch_pcb_and_home() {
    let mut app = crate::app::schematic_editor::tests::app();
    app.add_document(Document::new(DocumentKind::Board, "Board"), None);
    app.show_panel(Panel::Git);
    app.show_panel(Panel::ViewConfiguration);
    app.show_panel(Panel::Changes);
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    for active in [Some(0), Some(1), None, Some(0)] {
        app.active = active;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1440., 900.),
                )),
                ..Default::default()
            },
            |ctx| {
                app.left_dock(ctx);
                app.right_dock(ctx);
                assert!((ctx.available_rect().left() - 334.).abs() < 1.);
                assert!((ctx.available_rect().right() - 1124.).abs() < 1.);
            },
        );
        fn labels(shape: &egui::Shape, text: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => text.push(t.galley.job.text.clone()),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, text);
                    }
                }
                _ => {}
            }
        }
        let mut text = Vec::new();
        for shape in output.shapes {
            labels(&shape.shape, &mut text);
        }
        for title in [
            "Git",
            "Projects",
            "View Configuration",
            "Properties",
            "Changes",
        ] {
            assert!(
                text.iter().any(|label| label == title),
                "Missing {title}: {text:?}"
            );
        }
        for stub in ["Navigator", "PCB Filter", "SCH Filter", "PCB"] {
            assert!(!text.iter().any(|label| label == stub));
        }
    }
}

#[test]
fn popup_rail_opens_only_the_requested_panel_without_a_second_tab_strip() {
    let mut app = crate::app::schematic_editor::tests::app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    frame(&mut app, &ctx, vec![]);
    app.show_panel(Panel::Messages);
    app.settings
        .dock_layout
        .as_mut()
        .unwrap()
        .roots
        .iter_mut()
        .find(|r| r.edge == Some(Side::Right))
        .unwrap()
        .auto_hide = true;
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(pos2(1425., 25.))],
    );
    frame(&mut app, &ctx, vec![]);
    assert_eq!(app.dock_runtime.popup_panel, Some(Panel::Properties));
    assert!(
        app.dock_runtime
            .groups
            .iter()
            .any(|g| g.id == 4 && g.tabs.is_empty())
    );
    let target = ctx
        .data(|d| d.get_temp::<Rect>(Id::new(("dock_rail_rect", Panel::Messages))))
        .unwrap();
    frame(
        &mut app,
        &ctx,
        vec![egui::Event::PointerMoved(target.center())],
    );
    assert_eq!(
        app.dock_runtime.popup_panel,
        Some(Panel::Messages),
        "rail={target:?}"
    );
    assert!(app.panel_open(Panel::Properties));
    assert!(app.panel_open(Panel::Messages));
}
#[test]
fn floating_pointer_coordinates_use_the_main_window_dpi_scale() {
    let mut app = crate::app::schematic_editor::tests::app();
    let ctx = egui::Context::default();
    app.dock_runtime.main_scale = 1.;
    app.dock_runtime.drag = Some(Drag {
        panels: vec![Panel::Projects],
        source: egui::ViewportId::ROOT,
        pointer: Pos2::ZERO,
        size: vec2(300., 400.),
        offset: vec2(30., 12.),
        released: false,
        cancel: false,
        ctrl: false,
    });
    let mut input = egui::RawInput {
        events: vec![egui::Event::PointerMoved(pos2(30., 40.))],
        ..Default::default()
    };
    let info = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
    info.native_pixels_per_point = Some(2.);
    info.inner_rect = Some(Rect::from_min_size(pos2(100., 100.), vec2(300., 400.)));
    let _ = ctx.run(input, |ctx| app.update_dock_drag(ctx));
    assert_eq!(app.dock_runtime.drag.unwrap().pointer, pos2(260., 280.));
}

#[test]
fn dragging_any_caption_moves_only_its_active_panel() {
    for selected in Panel::ALL {
        let mut app = crate::app::schematic_editor::tests::app();
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        for p in Panel::ALL {
            app.show_panel(p);
        }
        app.show_panel(selected);
        frame(&mut app, &ctx, vec![]);
        let source_group = if selected.side() == Side::Left { 2 } else { 4 };
        let target_group = if source_group == 2 { 4 } else { 2 };
        let before: Vec<_> = Panel::ALL
            .into_iter()
            .filter(|p| *p != selected)
            .map(|p| {
                let root = app
                    .settings
                    .dock_layout
                    .as_ref()
                    .unwrap()
                    .root_for(p)
                    .unwrap();
                (p, root.id, panel_group_id(&root.tree, p).unwrap())
            })
            .collect();
        let document = app.documents[0].model.clone();
        let revision = app.documents[0].history.revision;
        let source = app
            .dock_runtime
            .groups
            .iter()
            .find(|g| g.id == source_group)
            .unwrap()
            .rect;
        let target = app
            .dock_runtime
            .groups
            .iter()
            .find(|g| g.id == target_group)
            .unwrap()
            .rect;
        // Reproduce grabbing the empty caption, rather than its text or bottom tab.
        drag(
            &mut app,
            &ctx,
            pos2(source.right() - 90., source.top() + 12.),
            target.center(),
        );
        assert!(
            app.settings
                .dock_layout
                .as_ref()
                .unwrap()
                .group(target_group)
                .unwrap()
                .panels()
                .contains(&selected),
            "{selected:?}"
        );
        let source = app
            .dock_runtime
            .groups
            .iter()
            .find(|g| g.id == target_group)
            .unwrap()
            .rect;
        drag(
            &mut app,
            &ctx,
            pos2(source.right() - 90., source.top() + 12.),
            pos2(720., 350.),
        );
        let layout = app.settings.dock_layout.as_ref().unwrap();
        let floating = layout.root_for(selected).unwrap();
        assert!(floating.edge.is_none(), "{selected:?}");
        assert_eq!(floating.tree.panels(), vec![selected]);
        for (p, id, group) in before {
            let root = layout.root_for(p).unwrap();
            assert_eq!(
                (root.id, panel_group_id(&root.tree, p).unwrap()),
                (id, group),
                "Moving {selected:?} affected {p:?}"
            );
            assert!(app.panel_open(p));
        }
        assert_membership(layout);
        assert_eq!(app.documents[0].model, document);
        assert_eq!(app.documents[0].history.revision, revision);
    }
}
fn click(app: &mut PcbCraft, ctx: &egui::Context, pos: Pos2) {
    frame(
        app,
        ctx,
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    frame(
        app,
        ctx,
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    frame(app, ctx, vec![]);
}
#[test]
fn header_close_and_pin_do_not_act_on_other_tabs() {
    let mut app = crate::app::schematic_editor::tests::app();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    app.show_panel(Panel::Git);
    frame(&mut app, &ctx, vec![]);
    let rect = app
        .dock_runtime
        .groups
        .iter()
        .find(|g| g.id == 2)
        .unwrap()
        .rect;
    click(&mut app, &ctx, pos2(rect.right() - 10., rect.top() + 12.));
    assert!(!app.panel_open(Panel::Git));
    assert!(app.panel_open(Panel::Projects));
    app.show_panel(Panel::Git);
    frame(&mut app, &ctx, vec![]);
    let rect = app
        .dock_runtime
        .groups
        .iter()
        .find(|g| g.id == 2)
        .unwrap()
        .rect;
    click(&mut app, &ctx, pos2(rect.right() - 29., rect.top() + 12.));
    let layout = app.settings.dock_layout.as_ref().unwrap();
    assert!(layout.root_for(Panel::Git).unwrap().auto_hide);
    assert!(!layout.root_for(Panel::Projects).unwrap().auto_hide);
    assert_eq!(
        layout.root_for(Panel::Git).unwrap().tree.panels(),
        vec![Panel::Git]
    );
    assert!(app.panel_open(Panel::Git));
    assert!(app.panel_open(Panel::Projects));
    assert_membership(layout);
}
#[test]
fn pin_and_dock_permissions_are_independent_and_persist() {
    let mut layout = Layout::default();
    layout.move_panels(&[Panel::Git], Target::Split(2, Side::Bottom));
    layout.set_permissions(Panel::Git, false, false);
    layout.set_auto_hide(Panel::Git, true);
    assert!(!layout.root_for(Panel::Projects).unwrap().auto_hide);
    layout.set_auto_hide(Panel::Git, false);
    layout.move_panels(&[Panel::Git], Target::Tab(2, 0));
    let restored: Layout = serde_json::from_str(&serde_json::to_string(&layout).unwrap()).unwrap();
    assert_eq!(restored.permissions(Panel::Git), (false, false));
    assert_eq!(restored.permissions(Panel::Projects), (true, true));
    assert_membership(&restored);
}
