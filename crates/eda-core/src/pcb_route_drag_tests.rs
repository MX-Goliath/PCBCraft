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
    assert_eq!(next.item(b.items[3].id).unwrap().anchor(), Point::mm(6, 5));
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
