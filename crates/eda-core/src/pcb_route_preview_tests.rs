use super::*;

fn same_geometry(route: &mut RouteSession, end: Point) -> RoutePreview {
    let mut reference = route.preview(end).unwrap();
    let shared = route.preview_shared(end).unwrap();
    assert_eq!(reference.items.len(), shared.board.items.len());
    for (item, expected) in reference
        .items
        .iter_mut()
        .zip(&shared.board.items)
        .skip(route.draft.items.len())
    {
        item.id = expected.id;
    }
    assert_eq!(reference, *shared.board);
    shared.board.validate().unwrap();
    shared
}
#[test]
fn shared_preview_matches_fully_validated_path_across_settings_and_blocked_targets() {
    let mut board = Board::default();
    board.items.push(Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(4, -2),
            end: Point::mm(4, 2),
            width: 200_000,
        },
    ));
    let original = board.clone();
    let mut route =
        RouteSession::begin(&board, 0, Point::default(), TOP, None, 250_000, 0).unwrap();
    for mode in [Conflict::Ignore, Conflict::Stop, Conflict::Walkaround] {
        route.conflict = mode;
        for corner in [Corner::FortyFive, Corner::Orthogonal, Corner::AnyAngle] {
            route.corner = corner;
            for diagonal in [true, false] {
                route.diagonal_first = diagonal;
                for end in [
                    Point::default(),
                    Point::mm(2, 1),
                    Point::mm(8, 0),
                    Point::mm(-5, -3),
                ] {
                    same_geometry(&mut route, end);
                }
            }
        }
    }
    assert_eq!(board, original);
    assert_eq!(route.draft(), &board);
}

#[test]
fn storage_and_ids_are_reused_but_retained_snapshots_never_change() {
    let board = Board::default();
    let mut route =
        RouteSession::begin(&board, 0, Point::default(), TOP, None, 250_000, 0).unwrap();
    let first = same_geometry(&mut route, Point::mm(2, 1));
    let address = Arc::as_ptr(&first.board);
    let ids: Vec<_> = first.board.items.iter().map(|i| i.id).collect();
    let repeated = same_geometry(&mut route, Point::mm(2, 1));
    assert!(Arc::ptr_eq(&first.board, &repeated.board));
    assert!(route.preview_current(Point::mm(2, 1)));
    drop(first);
    drop(repeated);
    let changed = same_geometry(&mut route, Point::mm(3, 1));
    assert_eq!(Arc::as_ptr(&changed.board), address);
    assert_eq!(
        changed.board.items.iter().map(|i| i.id).collect::<Vec<_>>(),
        ids
    );
    let retained = changed.board.clone();
    let snapshot = (*retained).clone();
    drop(changed);
    let next = same_geometry(&mut route, Point::mm(4, 1));
    assert!(!Arc::ptr_eq(&retained, &next.board));
    assert_eq!(*retained, snapshot);
    drop(next);
    same_geometry(&mut route, Point::default());
    let again = same_geometry(&mut route, Point::mm(3, 1));
    assert_eq!(
        again.board.items.iter().map(|i| i.id).collect::<Vec<_>>(),
        ids
    );
}

#[test]
fn placement_via_backtrack_and_finish_invalidate_cached_prefixes_and_preserve_history() {
    let board = Board::default();
    let mut route =
        RouteSession::begin(&board, 7, Point::default(), TOP, None, 250_000, 0).unwrap();
    let retained = same_geometry(&mut route, Point::mm(3, 1));
    route.place(Point::mm(3, 1), None).unwrap();
    assert!(!route.preview_current(Point::mm(3, 1)));
    let placed = same_geometry(&mut route, Point::mm(3, 1));
    assert_eq!(placed.changed_items.len(), placed.board.items.len());
    route
        .via(Point::mm(4, 1), BOTTOM, 600_000, 300_000)
        .unwrap();
    let via = same_geometry(&mut route, Point::mm(5, 2));
    assert!(
        via.board
            .items
            .iter()
            .any(|i| matches!(i.shape, Shape::Via { .. }))
    );
    assert!(route.backtrack());
    assert_eq!(route.layer, TOP);
    same_geometry(&mut route, Point::mm(3, 1));
    assert!(route.backtrack());
    assert_eq!(route.draft(), &board);
    assert!(!retained.board.items.is_empty());
    same_geometry(&mut route, Point::default());
    assert!(route.finish(8).is_err());
    assert_eq!(route.finish(7).unwrap(), board);
}

#[test]
fn invalid_settings_or_new_geometry_never_poison_a_ready_preview() {
    let board = Board::default();
    let mut route =
        RouteSession::begin(&board, 0, Point::default(), TOP, None, 250_000, 0).unwrap();
    let valid = same_geometry(&mut route, Point::mm(2, 1));
    let original = (*valid.board).clone();
    route.width = 0;
    assert!(route.preview_shared(Point::mm(2, 1)).is_err());
    route.width = 250_000;
    route.clearance = i64::MAX;
    assert!(route.preview_shared(Point::mm(2, 1)).is_err());
    route.clearance = 200_000;
    route.net = Some(55);
    assert!(route.preview_shared(Point::mm(2, 1)).is_err());
    route.net = None;
    assert!(route.preview_shared(Point::mm(100_001, 0)).is_err());
    assert_eq!(*valid.board, original);
    assert_eq!(*same_geometry(&mut route, Point::mm(2, 1)).board, original);
}

#[test]
fn initial_track_split_is_marked_for_live_painting() {
    let mut board = Board::default();
    board.items.push(Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(0, 0),
            end: Point::mm(10, 0),
            width: 200_000,
        },
    ));
    let mut route = RouteSession::begin(&board, 0, Point::mm(5, 0), TOP, None, 200_000, 0).unwrap();
    route.conflict = Conflict::Ignore;
    let preview = same_geometry(&mut route, Point::mm(5, 3));
    assert_eq!(preview.source_len, 1);
    assert_eq!(preview.source_changes, [board.items[0].id].into());
    assert_eq!(preview.changed_items.len(), preview.board.items.len());
    assert_eq!(board.items.len(), 1);
}
