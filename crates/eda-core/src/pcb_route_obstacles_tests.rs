use super::*;

fn track(x: i64, y: i64) -> Item {
    Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(x, y),
            end: Point::mm(x + 1, y + 1),
            width: 250_000,
        },
    )
}

fn compare(route: &RouteSession) {
    let obstacles = &route.obstacles;
    for n in 0..400 {
        let a = Point {
            x: (n * 7919 % 101) * 1_000_000 - 50_000_000,
            y: (n * 3571 % 97) * 1_000_000 - 50_000_000,
        };
        let b = Point {
            x: a.x + (n * 2903 % 17) * 100_000,
            y: a.y + (n * 1871 % 19) * 100_000,
        };
        assert_eq!(
            obstacles.blocker(a, b, route.width / 2, route.clearance, Some(route), None),
            obstacles.blocker_among(
                [a, b],
                route.width / 2,
                route.clearance,
                Some(route),
                None,
                0..obstacles.objects.len()
            ),
            "segment {n}: {a:?} -> {b:?}",
        );
    }
}

#[test]
fn index_matches_exhaustive_search_for_mixed_geometry_and_live_clearance() {
    let mut board = Board::default();
    for y in -12..12 {
        for x in -12..12 {
            let mut item = track(x * 4, y * 4);
            item.shape = match (x + y).rem_euclid(4) {
                0 => Shape::Via {
                    position: Point::mm(x * 4, y * 4),
                    diameter: 800_000,
                    hole: 400_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                },
                1 => Shape::Arc {
                    centre: Point::mm(x * 4, y * 4),
                    radius: 800_000,
                    start_angle: 15.,
                    end_angle: 290.,
                    width: 200_000,
                },
                2 => Shape::Fill {
                    start: Point::mm(x * 4, y * 4),
                    end: Point::mm(x * 4 + 1, y * 4 + 1),
                    rotation: 35.,
                },
                _ => item.shape,
            };
            board.items.push(item);
        }
    }
    let mut route =
        RouteSession::begin(&board, 0, Point::mm(80, 80), TOP, None, 250_000, 0).unwrap();
    for clearance in [0, 200_000, 7_000_000, 100_000_000_000] {
        route.clearance = clearance;
        compare(&route);
    }
    let mut rule = crate::pcb_rules::default_rule("Clearance");
    rule.insert("GAP".into(), "12mm".into());
    board.rules.push(rule);
    let route = RouteSession::begin(&board, 0, Point::mm(80, 80), TOP, None, 250_000, 0).unwrap();
    compare(&route);
}

#[test]
fn spatial_rule_maximum_covers_room_and_matrix_clearance() {
    let mut board = Board {
        items: (-8..8).map(|x| track(x * 5, 5)).collect(),
        ..Default::default()
    };
    board.items.push(Item::new(
        MULTI_LAYER,
        None,
        Shape::Via {
            position: Point::mm(20, -20),
            diameter: 800_000,
            hole: 400_000,
            from_layer: TOP,
            to_layer: BOTTOM,
        },
    ));
    let mut cutout = Item::new(
        TOP,
        None,
        Shape::Region {
            contour: vec![
                Point::mm(-30, -30),
                Point::mm(-29, -30),
                Point::mm(-29, -29),
                Point::mm(-30, -29),
            ],
            holes: vec![],
            cutout: true,
        },
    );
    cutout
        .properties
        .insert("ISBOARDCUTOUT".into(), "TRUE".into());
    board.items.push(cutout);
    let mut room = Fields::from([
        ("RULEKIND".into(), "RoomDefinition".into()),
        ("NAME".into(), "DUT".into()),
    ]);
    for (n, (x, y)) in [(-40, -40), (40, -40), (40, 40), (-40, 40)]
        .into_iter()
        .enumerate()
    {
        room.insert(format!("VX{n}"), format!("{x}mm"));
        room.insert(format!("VY{n}"), format!("{y}mm"));
    }
    let mut rule = crate::pcb_rules::default_rule("Clearance");
    rule.insert("SCOPE1EXPRESSION".into(), "WithinRoom('DUT')".into());
    rule.insert("GAP".into(), "0.1mm".into());
    rule.insert(
        "OBJECTCLEARANCES".into(),
        "ClearanceObj_Track-ClearanceObj_Track:3937008;ClearanceObj_Track-ClearanceObj_Via:5905512;ClearanceObj_Track-ClearanceObj_Region:787402".into(),
    );
    let mut edge = crate::pcb_rules::default_rule("BoardOutlineClearance");
    edge.insert(
        "OBJECTCLEARANCES".into(),
        "ClearanceObj_Track-ClearanceObj_CutoutEdge:7874016".into(),
    );
    board.rules = vec![room, rule, edge];
    let route = RouteSession::begin(&board, 0, Point::mm(80, 80), TOP, None, 250_000, 0).unwrap();
    assert!(route.obstacles.spatial);
    assert!(route.obstacles.maximum_gap >= 9_999_000);
    assert!(
        !route
            .obstacles
            .clear_for_route(&route, Point::mm(0, 0), Point::mm(1, 0))
    );
    for (a, b) in [
        (Point::mm(20, -35), Point::mm(21, -35)),
        (Point::mm(-30, -45), Point::mm(-29, -45)),
    ] {
        assert!(!route.obstacles.clear_for_route(&route, a, b));
    }
    compare(&route);
}

#[test]
fn first_blocker_retains_board_order_and_errors_block_outside_query() {
    let obstacle = |x, gap| Obstacle {
        bounds: Bounds::points(&[Point::mm(x, 0)]),
        gap,
        gap_bound: 0,
        item: Some(0),
        copper: Geometry::Track(Line::new((x as f64, 0.), (x as f64, 1.)), 0.1),
    };
    let objects = vec![
        obstacle(8, Ok(None)),
        obstacle(1000, Err("Unsupported rule".into())),
        obstacle(1, Ok(None)),
    ];
    let obstacles = Obstacles::indexed(objects, false);
    assert_eq!(obstacles.error(), Err("Unsupported rule".into()));
    assert_eq!(
        obstacles.blocker(Point::mm(0, 0), Point::mm(10, 0), 100_000, 0, None, None),
        Some(0)
    );
    assert_eq!(
        obstacles.blocker(Point::mm(0, 0), Point::mm(2, 0), 100_000, 0, None, None),
        Some(1)
    );
}

#[test]
fn large_negative_coordinates_and_global_edge_envelopes_are_safe() {
    let mut board = Board {
        outline: crate::pcb_polygon::straight_contour(&[
            Point::mm(-99_999, -99_999),
            Point::mm(99_999, -99_999),
            Point::mm(99_999, 99_999),
            Point::mm(-99_999, 99_999),
        ]),
        ..Default::default()
    };
    board
        .rules
        .push(crate::pcb_rules::default_rule("BoardOutlineClearance"));
    board.items.push(track(-99_990, -99_990));
    let mut route = RouteSession::begin(
        &board,
        0,
        Point::mm(-99_995, -99_990),
        TOP,
        None,
        200_000,
        0,
    )
    .unwrap();
    route.conflict = Conflict::Stop;
    assert!(route.points(Point::mm(-99_985, -99_990)).last().unwrap().x < -99_990_000_000);
    assert_ne!(
        route.points(Point::mm(-100_000, -99_990)).last(),
        Some(&Point::mm(-100_000, -99_990))
    );
    compare(&route);
}
