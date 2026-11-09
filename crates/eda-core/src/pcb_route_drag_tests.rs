use super::*;

fn track(a: Point, b: Point) -> Item {
    Item::new(
        TOP,
        Some(0),
        Shape::Track {
            start: a,
            end: b,
            width: 250_000,
        },
    )
}
fn pad(at: Point) -> Item {
    Item::new(
        TOP,
        Some(0),
        Shape::Pad {
            position: at,
            size: Point::mm(1, 1),
            rotation: 0.0,
            shape: PadShape::Round,
            hole: 0,
            plated: true,
            name: "1".into(),
            bottom_size: None,
            middle_size: None,
        },
    )
}
fn board() -> Board {
    Board {
        nets: vec![
            Net {
                index: 0,
                name: "SIGNAL".into(),
                properties: Fields::new(),
            },
            Net {
                index: 1,
                name: "OTHER".into(),
                properties: Fields::new(),
            },
        ],
        ..Default::default()
    }
}
fn assert_connected_45(board: &Board) {
    assert!(crate::pcb_connectivity::connections(board).is_empty());
    for item in &board.items {
        if let Shape::Track { start, end, .. } = item.shape {
            let dx = (start.x - end.x).abs();
            let dy = (start.y - end.y).abs();
            assert!(dx == 0 || dy == 0 || dx == dy, "{:?}", item.shape);
        }
    }
}
fn move_track(board: &Board, id: Uuid, delta: Point) -> Result<Board, String> {
    drag_tracks(
        board,
        &[id],
        Point::default(),
        delta,
        0.0,
        DragOptions::default(),
    )
}

#[test]
fn dragging_between_pads_keeps_both_anchors_and_native_track_metadata() {
    let mut b = board();
    let mut t = track(Point::mm(0, 0), Point::mm(10, 0));
    t.properties
        .insert("USER_PROPERTY".into(), "preserved".into());
    let id = t.id;
    b.items = vec![pad(Point::mm(0, 0)), pad(Point::mm(10, 0)), t];
    let original = b.clone();
    let next = move_track(&b, id, Point::mm(0, 3)).unwrap();
    assert_eq!(&next.items[..2], &b.items[..2]);
    assert_eq!(next.item(id).unwrap().anchor(), Point::mm(0, 3));
    assert!(next.items.len() > b.items.len());
    for item in next
        .items
        .iter()
        .filter(|i| matches!(i.shape, Shape::Track { .. }))
    {
        assert_eq!(
            item.properties.get("USER_PROPERTY").map(String::as_str),
            Some("preserved")
        );
    }
    assert_connected_45(&next);
    assert_eq!(b, original);
}

#[test]
fn neighbour_tracks_and_mid_segment_t_junction_reroute_to_fixed_pads() {
    let mut b = board();
    let t = track(Point::mm(2, 2), Point::mm(8, 2));
    let id = t.id;
    b.items = vec![
        t,
        track(Point::mm(0, 0), Point::mm(2, 2)),
        track(Point::mm(8, 2), Point::mm(10, 0)),
        track(Point::mm(5, 2), Point::mm(5, -2)),
        pad(Point::mm(0, 0)),
        pad(Point::mm(10, 0)),
        pad(Point::mm(5, -2)),
    ];
    let next = move_track(&b, id, Point::mm(1, 3)).unwrap();
    assert_eq!(&next.items[4..7], &b.items[4..7]);
    assert_connected_45(&next);
    assert_eq!(next.item(b.items[3].id).unwrap().anchor(), Point::mm(5, 5));
}

#[test]
fn rerouting_a_neighbour_retains_its_interior_pad_and_via() {
    let mut b = board();
    let t = track(Point::mm(8, 0), Point::mm(14, 0));
    let id = t.id;
    let via = Item::new(
        MULTI_LAYER,
        Some(0),
        Shape::Via {
            position: Point::mm(4, 0),
            diameter: 600_000,
            hole: 300_000,
            from_layer: TOP,
            to_layer: BOTTOM,
        },
    );
    b.items = vec![
        t,
        track(Point::mm(0, 0), Point::mm(8, 0)),
        pad(Point::mm(0, 0)),
        pad(Point::mm(14, 0)),
        via,
        pad(Point::mm(2, 0)),
    ];
    let next = move_track(&b, id, Point::mm(0, 3)).unwrap();
    assert_eq!(&next.items[2..6], &b.items[2..6]);
    assert_connected_45(&next);
}

#[test]
fn obstacle_walkaround_is_clear_and_stop_rejects_the_whole_drag() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    let mut obstacle = track(Point::mm(5, 2), Point::mm(5, 4));
    obstacle.net = Some(1);
    b.items = vec![
        t,
        pad(Point::mm(0, 0)),
        pad(Point::mm(10, 0)),
        obstacle.clone(),
    ];
    let next = move_track(&b, id, Point::mm(0, 3)).unwrap();
    assert_connected_45(&next);
    assert!(next.items.len() > 6);
    let environment = Board {
        items: vec![obstacle],
        ..b.clone()
    };
    let obstacles = obstacles::Obstacles::new(&environment, TOP, Some(0));
    for item in next.items.iter().filter(|i| i.net == Some(0)) {
        if let Shape::Track { start, end, width } = item.shape {
            assert!(obstacles.clear(start, end, width / 2, 200_000));
        }
    }
    let before = b.clone();
    assert!(
        drag_tracks(
            &b,
            &[id],
            Point::default(),
            Point::mm(0, 3),
            0.0,
            DragOptions {
                conflict: Conflict::Stop,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(b, before);
}

#[test]
fn locked_neighbour_rejects_drag_but_other_layer_track_stays_unchanged() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    let mut neighbour = track(Point::mm(10, 0), Point::mm(15, 0));
    neighbour.locked = true;
    b.items = vec![t, neighbour];
    assert!(move_track(&b, id, Point::mm(0, 3)).is_err());
    b.items[1].layer = BOTTOM;
    let next = move_track(&b, id, Point::mm(0, 3)).unwrap();
    assert_eq!(next.items[1], b.items[1]);
}

#[test]
fn endpoint_drag_keeps_pad_and_mid_track_branch_connected() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    b.items = vec![
        t,
        pad(Point::mm(0, 0)),
        pad(Point::mm(10, 0)),
        track(Point::mm(5, 0), Point::mm(5, -4)),
        pad(Point::mm(5, -4)),
    ];
    let next =
        drag_track_endpoint(&b, id, false, Point::mm(10, 4), DragOptions::default()).unwrap();
    assert_connected_45(&next);
    assert_eq!(next.items[1], b.items[1]);
    assert_eq!(next.items[2], b.items[2]);
}

#[test]
fn no_net_contacts_are_kept_but_unrelated_no_net_copper_blocks() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    b.items = vec![
        t,
        pad(Point::mm(0, 0)),
        pad(Point::mm(10, 0)),
        track(Point::mm(5, 2), Point::mm(5, 4)),
    ];
    for i in &mut b.items {
        i.net = None;
    }
    let next = move_track(&b, id, Point::mm(0, 3)).unwrap();
    assert!(next.items.len() > 6);
    assert_eq!(&next.items[1..4], &b.items[1..4]);
}

#[test]
fn fixed_crossing_track_retains_its_junction() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    b.items = vec![
        t,
        track(Point::mm(5, -5), Point::mm(5, 5)),
        pad(Point::mm(5, -5)),
        pad(Point::mm(5, 5)),
    ];
    let next = move_track(&b, id, Point::mm(3, 3)).unwrap();
    assert_eq!(next.items[1], b.items[1]);
    assert_connected_45(&next);
}

#[test]
fn no_motion_is_exact_and_locked_selected_track_cannot_move() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    b.items = vec![t];
    assert_eq!(move_track(&b, id, Point::default()).unwrap(), b);
    b.items[0].locked = true;
    assert!(move_track(&b, id, Point::mm(0, 3)).is_err());
}

#[test]
fn a_new_endpoint_cannot_merge_with_unrelated_no_net_copper() {
    let mut b = board();
    let t = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = t.id;
    b.items = vec![t, pad(Point::mm(10, 3))];
    for i in &mut b.items {
        i.net = None;
    }
    let original = b.clone();
    assert!(move_track(&b, id, Point::mm(0, 3)).is_err());
    assert_eq!(b, original);
}

#[test]
fn a_selected_chain_keeps_shared_nodes_and_fixed_terminal_pads() {
    let mut b = board();
    b.items = vec![
        track(Point::mm(0, 0), Point::mm(5, 0)),
        track(Point::mm(5, 0), Point::mm(8, 3)),
        pad(Point::mm(0, 0)),
        pad(Point::mm(8, 3)),
    ];
    let selected = vec![b.items[0].id, b.items[1].id];
    let next = drag_tracks(
        &b,
        &selected,
        Point::default(),
        Point::mm(0, 3),
        0.0,
        DragOptions::default(),
    )
    .unwrap();
    assert_eq!(&next.items[2..4], &b.items[2..4]);
    assert_connected_45(&next);
}

fn equivalent_drag(actual: &Board, expected: &Board, prefix: usize) {
    let mut normalized = actual.clone();
    assert_eq!(normalized.items.len(), expected.items.len());
    for (index, (a, b)) in normalized.items.iter_mut().zip(&expected.items).enumerate() {
        if index >= prefix {
            a.id = b.id;
        } else {
            assert_eq!(a.id, b.id);
        }
    }
    assert_eq!(&normalized, expected);
}

#[test]
fn prepared_drag_matches_complete_transactions_and_keeps_retained_frames_immutable() {
    use std::sync::Arc;
    let mut b = board();
    let selected = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = selected.id;
    b.items = vec![selected, pad(Point::mm(0, 0)), pad(Point::mm(10, 0))];
    let original = b.clone();
    let mut prepared = TrackDragPreview::new(&b, &[id]).unwrap();
    let first = prepared
        .update(
            Point::default(),
            Point::mm(0, 1),
            0.,
            DragOptions::default(),
        )
        .unwrap();
    let held = first.preview.board;
    let held_copy = (*held).clone();
    let second = prepared
        .update(
            Point::default(),
            Point::mm(0, 2),
            0.,
            DragOptions::default(),
        )
        .unwrap();
    assert_eq!(*held, held_copy);
    assert!(!Arc::ptr_eq(&held, &second.preview.board));
    assert!(!second.constrained);
    let repeat = prepared
        .update(
            Point::default(),
            Point::mm(0, 2),
            0.,
            DragOptions::default(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&second.preview.board, &repeat.preview.board));
    let expected = move_track(&b, id, Point::mm(0, 2)).unwrap();
    equivalent_drag(&second.preview.board, &expected, b.items.len());
    assert_connected_45(&second.preview.board);
    assert_eq!(b, original);
    drop(second);
    drop(repeat);
    let zero = prepared
        .update(
            Point::default(),
            Point::default(),
            0.,
            DragOptions::default(),
        )
        .unwrap();
    assert_eq!(*zero.preview.board, original);
    assert!(zero.preview.changed_items.is_empty());
    assert_eq!(prepared.finish().unwrap(), original);
}

#[test]
fn constrained_drag_stops_at_clearance_keeps_anchors_and_rechecks_conflict_changes() {
    let mut b = board();
    let selected = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = selected.id;
    let mut obstacle = track(Point::mm(5, 2), Point::mm(5, 4));
    obstacle.net = Some(1);
    b.items = vec![
        selected,
        pad(Point::mm(0, 0)),
        pad(Point::mm(10, 0)),
        obstacle.clone(),
    ];
    let mut prepared = TrackDragPreview::new(&b, &[id]).unwrap();
    let stop = DragOptions {
        conflict: Conflict::Stop,
        ..Default::default()
    };
    let check = |next: &Board| {
        let environment = Board {
            items: vec![obstacle.clone()],
            ..b.clone()
        };
        let obstacles = obstacles::Obstacles::new(&environment, TOP, Some(0));
        for i in next.items.iter().filter(|i| i.net == Some(0)) {
            if let Shape::Track { start, end, width } = i.shape {
                assert!(obstacles.clear(start, end, width / 2, 200_000));
            }
        }
        assert_eq!(&next.items[1..3], &b.items[1..3]);
        assert_connected_45(next);
    };
    let update = prepared
        .update(Point::default(), Point::mm(0, 3), 0., stop)
        .unwrap();
    assert!(update.constrained);
    let at = update.preview.board.item(id).unwrap().anchor();
    assert!(at.y > 0 && at.y < 2_000_000);
    check(&update.preview.board);
    drop(update);
    let ignored = prepared
        .update(
            Point::default(),
            Point::mm(0, 3),
            0.,
            DragOptions {
                conflict: Conflict::Ignore,
                ..stop
            },
        )
        .unwrap();
    assert!(!ignored.constrained);
    drop(ignored);
    let constrained = prepared
        .update(Point::default(), Point::mm(0, 3), 0., stop)
        .unwrap();
    assert!(constrained.constrained);
    check(&constrained.preview.board);
    drop(constrained);
    check(&prepared.finish().unwrap());
}

#[test]
fn prepared_endpoint_matches_the_full_connected_solver_and_rolls_back_errors() {
    let mut b = board();
    let selected = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = selected.id;
    b.items = vec![
        selected,
        pad(Point::mm(0, 0)),
        pad(Point::mm(10, 0)),
        track(Point::mm(5, 0), Point::mm(5, -4)),
        pad(Point::mm(5, -4)),
    ];
    let mut prepared = TrackDragPreview::new_endpoint(&b, id, false).unwrap();
    let at = Point::mm(10, 3);
    let update = prepared
        .update_endpoint(at, DragOptions::default())
        .unwrap();
    let expected = drag_track_endpoint(&b, id, false, at, DragOptions::default()).unwrap();
    equivalent_drag(&update.preview.board, &expected, b.items.len());
    drop(update);
    assert!(
        prepared
            .update_endpoint(
                at,
                DragOptions {
                    clearance: -1,
                    ..Default::default()
                }
            )
            .is_err()
    );
    let retry = prepared
        .update_endpoint(at, DragOptions::default())
        .unwrap();
    equivalent_drag(&retry.preview.board, &expected, b.items.len());
    assert_connected_45(&prepared.finish().unwrap());
    b.items[0].net = None;
    assert!(!TrackDragPreview::supports(&b, &[id]));
    assert!(TrackDragPreview::new(&b, &[id]).is_err());
}

#[test]
fn prepared_environments_preserve_room_rules_and_multiple_track_widths() {
    let mut b = board();
    let selected = track(Point::mm(0, 0), Point::mm(10, 0));
    let id = selected.id;
    let mut neighbour = track(Point::mm(10, 0), Point::mm(15, 5));
    if let Shape::Track { width, .. } = &mut neighbour.shape {
        *width = 150_000;
    }
    let mut obstacle = track(Point::mm(8, 3), Point::mm(8, 5));
    obstacle.net = Some(1);
    b.items = vec![
        selected,
        neighbour,
        pad(Point::mm(0, 0)),
        pad(Point::mm(15, 5)),
        obstacle,
    ];
    let mut room = Fields::from([
        ("RULEKIND".into(), "RoomDefinition".into()),
        ("NAME".into(), "TEST_ROOM".into()),
    ]);
    for (n, (x, y)) in [(-5, 2), (20, 2), (20, 20), (-5, 20)]
        .into_iter()
        .enumerate()
    {
        room.insert(format!("VX{n}"), format!("{x}mm"));
        room.insert(format!("VY{n}"), format!("{y}mm"));
    }
    let mut clearance = crate::pcb_rules::default_rule("Clearance");
    clearance.insert("SCOPE1EXPRESSION".into(), "WithinRoom('TEST_ROOM')".into());
    clearance.insert("GAP".into(), "0.8mm".into());
    b.rules = vec![room, clearance];
    let mut prepared = TrackDragPreview::new(&b, &[id]).unwrap();
    for delta in [Point::mm(0, -1), Point::mm(0, 1), Point::mm(1, -2)] {
        let expected = move_track(&b, id, delta).unwrap();
        let update = prepared
            .update(Point::default(), delta, 0., DragOptions::default())
            .unwrap();
        assert!(!update.constrained);
        equivalent_drag(&update.preview.board, &expected, b.items.len());
    }
}

fn sliding_chain() -> (Board, Uuid) {
    let mut b = board();
    let selected = track(Point::mm(4, 4), Point::mm(12, 4));
    let id = selected.id;
    b.items = vec![
        selected,
        track(Point::mm(0, 0), Point::mm(4, 4)),
        track(Point::mm(12, 4), Point::mm(16, 0)),
        pad(Point::mm(0, 0)),
        pad(Point::mm(16, 0)),
    ];
    (b, id)
}
#[test]
fn body_slide_preserves_adjacent_lines_and_ignores_tangential_pointer_motion() {
    for angle in [0., 90., 180., 270.] {
        let (mut b, id) = sliding_chain();
        for i in &mut b.items {
            crate::pcb::transform_item(i, Point::default(), Point::default(), angle, false)
                .unwrap();
        }
        let rotated = |p| {
            let mut i = track(p, p);
            crate::pcb::transform_item(&mut i, Point::default(), Point::default(), angle, false)
                .unwrap();
            i.anchor()
        };
        let original = b.clone();
        let next = move_track(&b, id, rotated(Point::mm(3, 2))).unwrap();
        assert_eq!(next.items.len(), b.items.len());
        assert!(
            matches!(next.item(id).unwrap().shape, Shape::Track {start,end,..}
            if start==rotated(Point::mm(6,6)) && end==rotated(Point::mm(10,6)))
        );
        assert_eq!(&next.items[3..], &b.items[3..]);
        assert_connected_45(&next);
        assert_eq!(move_track(&b, id, rotated(Point::mm(3, 0))).unwrap(), b);
        assert_eq!(b, original);
    }
}
#[test]
fn diagonal_slide_intersects_horizontal_and_vertical_neighbours_without_new_elbows() {
    let mut b = board();
    let selected = track(Point::mm(4, 0), Point::mm(8, 4));
    let id = selected.id;
    b.items = vec![
        selected,
        track(Point::mm(0, 0), Point::mm(4, 0)),
        track(Point::mm(8, 4), Point::mm(8, 8)),
        pad(Point::mm(0, 0)),
        pad(Point::mm(8, 8)),
    ];
    let next = move_track(&b, id, Point::mm(2, 0)).unwrap();
    assert_eq!(next.items.len(), b.items.len());
    assert!(
        matches!(next.item(id).unwrap().shape,Shape::Track{start,end,..}
        if start==Point::mm(6,0) && end==Point::mm(8,2))
    );
    assert_connected_45(&next);
    assert_eq!(move_track(&b, id, Point::mm(2, 2)).unwrap(), b);
}
#[test]
fn sliding_preview_matches_commit_restores_source_and_stops_before_corner_inversion() {
    let (b, id) = sliding_chain();
    let mut prepared = TrackDragPreview::new(&b, &[id]).unwrap();
    for delta in [Point::mm(3, 2), Point::mm(-2, 1), Point::default()] {
        let update = prepared
            .update(Point::default(), delta, 0., DragOptions::default())
            .unwrap();
        assert!(!update.constrained);
        equivalent_drag(
            &update.preview.board,
            &move_track(&b, id, delta).unwrap(),
            b.items.len(),
        );
    }
    let update = prepared
        .update(
            Point::default(),
            Point::mm(0, 10),
            0.,
            DragOptions::default(),
        )
        .unwrap();
    assert!(update.constrained);
    let Shape::Track { start, end, .. } = update.preview.board.item(id).unwrap().shape else {
        unreachable!()
    };
    assert!(start.x < end.x && start.y > 4_000_000 && start.y < 8_000_000);
    assert_eq!(update.preview.board.items.len(), b.items.len());
    assert_connected_45(&update.preview.board);
    let expected = (*update.preview.board).clone();
    drop(update);
    assert_eq!(prepared.finish().unwrap(), expected);
    assert!(move_track(&b, id, Point::mm(0, 10)).is_err());
}
