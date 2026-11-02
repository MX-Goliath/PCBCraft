use super::*;

#[test]
fn measured_electrical_boundary_does_not_relax_solder_mask_checks() {
    // AD 24.4.1, complete native DRC runs on independent parallel-track
    // fixtures: 19 native units (48.26 nm) pass; 20 (50.8 nm) fail.
    for (deficit, violations) in [(0, 0), (41, 0), (48, 0), (51, 1), (102, 1)] {
        let mut rule = default_rule("Clearance");
        rule.insert("GAP".into(), "0.2mm".into());
        let board = Board {
            rules: vec![rule],
            items: vec![
                track(TOP, Some(0), 0, 0),
                track(TOP, Some(1), 0, 300_000 - deficit),
            ],
            ..Default::default()
        };
        assert_eq!(check(&board).violations.len(), violations, "{deficit}");
    }
    // The same native application reports a one-unit mask deficit (2.54 nm).
    // Keep that manufacturing boundary independent from the copper margin.
    let board = Board {
        rules: vec![default_rule("MinimumSolderMaskSliver")],
        items: [0, 1_099_997]
            .into_iter()
            .map(|x| {
                Item::new(
                    37,
                    None,
                    Shape::Fill {
                        start: Point { x, y: 0 },
                        end: Point {
                            x: x + 1_000_000,
                            y: 1_000_000,
                        },
                        rotation: 0.,
                    },
                )
            })
            .collect(),
        ..Default::default()
    };
    assert_eq!(check(&board).violations.len(), 1);
}

#[test]
fn native_bad_connection_check_cannot_silently_pass_as_an_island_check() {
    let mut rule = default_rule("UnRoutedNet");
    rule.insert("CHECKBADCONNECTIONS".into(), "TRUE".into());
    let board = Board {
        rules: vec![rule],
        ..Default::default()
    };
    let report = check(&board);
    assert!(report.violations.is_empty());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|s| s.contains("CHECKBADCONNECTIONS"))
    );
    let report = check_with_options(
        &board,
        &CheckOptions {
            excluded: BTreeSet::from(["UnRoutedNet".into()]),
            ..Default::default()
        },
    );
    assert!(report.diagnostics.is_empty());
}
#[test]
fn net_tie_exemption_is_limited_to_copper_owned_by_that_footprint() {
    let mut board = Board {
        rules: vec![default_rule("ShortCircuit"), default_rule("Clearance")],
        items: vec![track(TOP, Some(0), 0, 0), track(TOP, Some(1), 0, 0)],
        ..Default::default()
    };
    let component = Component {
        id: Uuid::new_v4(),
        index: 7,
        designator: "NT1".into(),
        comment: String::new(),
        footprint: "NetTie".into(),
        parameters: None,
        position: Point::default(),
        rotation: 0.,
        layer: TOP,
        locked: false,
        properties: Fields::from([("COMPONENTKIND".into(), "4".into())]),
    };
    board.components.push(component);
    board.items[0].component = Some(7);
    board.items[1].component = Some(7);
    assert!(check(&board).violations.is_empty());
    assert_eq!(
        clearance(&board, &board.items[0], &board.items[1]),
        Ok(Some(0))
    );
    // Identical geometry outside the footprint must still report both checks.
    board.items[1].component = None;
    assert_eq!(check(&board).violations.len(), 2);
    board.items[1].component = Some(7);
    board.components[0]
        .properties
        .insert("COMPONENTKINDVERSION2".into(), "0".into());
    assert_eq!(check(&board).violations.len(), 2);
}
#[test]
fn circular_annular_ring_uses_native_dimensions_at_the_limit_and_each_stack_layer() {
    let mut via = Item::new(
        MULTI_LAYER,
        Some(0),
        Shape::Via {
            position: Point::mm(241, 125),
            diameter: 700_001,
            hole: 299_999,
            from_layer: TOP,
            to_layer: BOTTOM,
        },
    );
    let mut board = Board {
        rules: vec![default_rule("MinimumAnnularRing")],
        ..Default::default()
    };
    via.set_stack_mode(StackMode::Full, &board);
    board.items.push(via);
    assert!(
        check(&board).violations.is_empty(),
        "A true 0.200001 mm ring must not become 0.1998 mm through tessellation"
    );
    let mut bottom = board.items[0].pad_geometry(BOTTOM).unwrap();
    bottom.size = Point {
        x: 699_993,
        y: 699_993,
    };
    board.items[0].set_pad_geometry(BOTTOM, bottom);
    let report = check(&board);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].layer, BOTTOM);
    let mut top = board.items[0].pad_geometry(TOP).unwrap();
    top.offset.x = 100_000;
    board.items[0].set_pad_geometry(TOP, top);
    let report = check(&board);
    assert!(report.violations.iter().any(|v| v.layer == TOP));
}
#[test]
fn native_pour_fragments_inherit_net_for_short_circuit_checks() {
    let mut board = Board {
        rules: vec![default_rule("ShortCircuit")],
        items: vec![track(TOP, Some(7), 0, 0)],
        ..Default::default()
    };
    let mut fragment = Item::new(
        TOP,
        None,
        Shape::Region {
            contour: vec![
                Point::mm(0, -1),
                Point::mm(2, -1),
                Point::mm(2, 1),
                Point::mm(0, 1),
            ],
            holes: vec![],
            cutout: false,
        },
    );
    fragment.polygon = Some(2);
    board.items.push(fragment);
    board.polygon_pours.push(pcb_polygon::Polygon {
        index: 2,
        net: Some(7),
        ..Default::default()
    });
    assert!(check(&board).violations.is_empty());
    board.polygon_pours[0].net = Some(8);
    assert_eq!(
        check(&board).violations.len(),
        1,
        "A genuinely different inherited net must still be checked"
    );
    board.polygon_pours.clear();
    board.polygons.push(Fields::from([
        ("ID".into(), "2".into()),
        ("NET".into(), "7".into()),
    ]));
    assert!(check(&board).violations.is_empty());
    board.polygon_pours.push(pcb_polygon::Polygon {
        index: 2,
        net: None,
        ..Default::default()
    });
    assert_eq!(board.item_net(&board.items[1]), None);
    board.items[1].net = Some(9);
    assert_eq!(board.item_net(&board.items[1]), Some(9));
}

#[test]
fn large_gap_for_rare_scope_does_not_exhaust_budget_on_unrelated_copper() {
    let mut board = Board {
        rules: vec![default_rule("Clearance")],
        ..Default::default()
    };
    let mut rare = default_rule("Clearance");
    rare.insert("GAP".into(), "100mm".into());
    rare.insert("SCOPE1EXPRESSION".into(), "IsVia".into());
    rare.insert("SCOPE2EXPRESSION".into(), "IsVia".into());
    rare.insert("PRIORITY".into(), "0".into());
    board.rules.push(rare);
    board.items = (0..3000)
        .map(|n| track(TOP, Some(0), n * 2_000_000, 0))
        .collect();
    let report = check(&board);
    assert!(report.violations.is_empty());
    assert!(report.diagnostics.is_empty());
    assert!(!report.truncated);
}

fn track(layer: u16, net: Option<u32>, x: i64, y: i64) -> Item {
    Item::new(
        layer,
        net,
        Shape::Track {
            start: Point { x, y },
            end: Point {
                x: x + 1_000_000,
                y,
            },
            width: 100_000,
        },
    )
}
fn keys(report: &Report) -> Vec<(usize, Vec<Uuid>, String, u16)> {
    report
        .violations
        .iter()
        .map(|v| (v.rule, v.items.clone(), v.message.clone(), v.layer))
        .collect()
}
#[test]
fn prepared_rules_match_uncached_resolution_and_do_not_alias_duplicate_ids_or_probes() {
    let mut board = Board {
        items: vec![
            track(TOP, Some(0), 0, 0),
            track(BOTTOM, Some(1), 0, 0),
            track(TOP, None, 0, 0),
        ],
        ..Default::default()
    };
    board.items[1].id = board.items[0].id;
    for (priority, scope, net_scope) in [
        (2, "IsTrack", "DifferentNets"),
        (1, "OnBottomLayer", "SameNet"),
        (3, "All", ""),
    ] {
        let mut r = default_rule("Clearance");
        r.insert("PRIORITY".into(), priority.to_string());
        r.insert("SCOPE1EXPRESSION".into(), scope.into());
        r.insert("NETSCOPE".into(), net_scope.into());
        board.rules.push(r);
    }
    for expression in ["OnTopLayer", "Not SupportedQuery", "IsTrack"] {
        board.rules[0].insert("SCOPE2EXPRESSION".into(), expression.into());
        let prepared = Rules::new(&board);
        let mut synthetic = board.items[0].clone();
        synthetic.layer = BOTTOM;
        let items: Vec<_> = board
            .items
            .iter()
            .chain(std::iter::once(&synthetic))
            .collect();
        for _ in 0..2 {
            for a in &items {
                assert_eq!(
                    prepared.resolve("Clearance", a, None),
                    resolve(&board, "Clearance", a, None)
                );
                for b in &items {
                    assert_eq!(
                        prepared.resolve("Clearance", a, Some(b)),
                        resolve(&board, "Clearance", a, Some(b))
                    );
                }
            }
        }
    }
}
#[test]
fn copper_pairs_match_exhaustive_geometry_with_matrix_net_scopes_and_footprint_exemption() {
    let mut board = Board {
        rules: vec![default_rule("Clearance"), default_rule("ShortCircuit")],
        ..Default::default()
    };
    board.rules[0].insert("GAP".into(), "0.25mm".into());
    board.rules[0].insert(
        "OBJECTCLEARANCES".into(),
        "ClearanceObj_Track-ClearanceObj_Track:118110".into(),
    );
    for n in 0..55 {
        let mut item = track(
            TOP,
            Some(n % 3),
            (n as i64 * 7919 % 19) * 100_000,
            (n as i64 * 1543 % 17) * 100_000,
        );
        if n % 7 == 0 {
            item.net = None;
        }
        board.items.push(item);
    }
    let mut scoped = default_rule("Clearance");
    scoped.insert("PRIORITY".into(), "0".into());
    scoped.insert("NETSCOPE".into(), "SameNet".into());
    scoped.insert("GAP".into(), "0.12mm".into());
    scoped.insert("IGNOREPADTOPADCLEARANCEINFOOTPRINT".into(), "TRUE".into());
    board.rules.push(scoped);
    for x in [5_000_000, 5_100_000] {
        let mut item = Item::new(
            TOP,
            Some(0),
            Shape::Pad {
                position: Point { x, y: 5_000_000 },
                size: Point::mm(1, 1),
                rotation: 0.,
                shape: PadShape::Rectangle,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        item.component = Some(0);
        board.items.push(item);
    }
    let mut expected = Report::default();
    for (a, i) in board.items.iter().enumerate() {
        for j in &board.items[a + 1..] {
            let first = pcb_polygon::copper(i, TOP);
            let second = pcb_polygon::copper(j, TOP);
            if (i.net.is_none() || i.net != j.net) && first.intersects(&second) {
                let (n, r) = resolve(&board, "ShortCircuit", i, Some(j))
                    .unwrap()
                    .unwrap();
                expected.add(
                    n,
                    r,
                    "Copper objects on different nets intersect".into(),
                    &[i, j],
                    TOP,
                );
            }
            let (n, r) = resolve(&board, "Clearance", i, Some(j)).unwrap().unwrap();
            let required = clearance(&board, i, j).unwrap().unwrap();
            let distance = (Euclidean.distance(&first, &second) * 1e6).round() as i64;
            if distance + 50 < required {
                expected.add(
                    n,
                    r,
                    format!(
                        "Clearance {:.6} mm < {:.6} mm",
                        distance as f64 / 1e6,
                        required as f64 / 1e6
                    ),
                    &[i, j],
                    TOP,
                );
            }
        }
    }
    let canonical = |r: &Report| {
        r.violations
            .iter()
            .map(|v| {
                let mut ids = v.items.clone();
                ids.sort();
                (v.rule, ids, v.message.clone(), v.layer)
            })
            .collect::<BTreeSet<_>>()
    };
    let actual = check(&board);
    assert!(!expected.violations.is_empty());
    assert_eq!(canonical(&actual), canonical(&expected));
    assert!(!actual.truncated);
    let limited = check_with_options(
        &board,
        &CheckOptions {
            limit: 17,
            ..Default::default()
        },
    );
    assert_eq!(keys(&limited), keys(&actual)[..17]);
    assert!(limited.truncated);
}
#[test]
fn cancellation_discards_results_and_can_interrupt_between_phases() {
    let board = Board {
        rules: vec![default_rule("Width"), default_rule("Clearance")],
        items: vec![track(TOP, Some(0), 0, 0), track(TOP, Some(1), 0, 0)],
        ..Default::default()
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let expected = check(&board);
    let actual =
        check_owned_cancellable(board.clone(), &CheckOptions::default(), cancel.clone()).unwrap();
    assert_eq!(keys(&actual), keys(&expected));
    let report = check_inner(&board, 5000, Some(cancel.clone()), &mut |phase| {
        if phase == "item_rules" {
            cancel.store(true, Ordering::Relaxed);
        }
    });
    assert!(report.violations.iter().all(|v| v.kind == "Width"));
    assert!(check_owned_cancellable(board, &CheckOptions::default(), cancel).is_none());
}
#[test]
fn outline_pruning_matches_full_containment_and_distance_on_concave_and_convex_boards() {
    for points in [
        vec![(0, 0), (10, 0), (10, 10), (0, 10)],
        vec![(0, 0), (10, 0), (10, 4), (4, 4), (4, 10), (0, 10)],
    ] {
        let mut board = Board {
            rules: vec![default_rule("BoardOutlineClearance")],
            ..Default::default()
        };
        board.rules[0].insert("GAP".into(), "0.3mm".into());
        board.outline = points
            .into_iter()
            .map(|(x, y)| ContourVertex {
                position: Point::mm(x, y),
                arc: None,
            })
            .collect();
        for n in 0..100 {
            board.items.push(track(
                TOP,
                None,
                (n * 7919 % 130) * 100_000 - 1_000_000,
                (n * 1543 % 130) * 100_000 - 1_000_000,
            ));
        }
        let outline = polygon(&board.outline_points(256));
        let mut expected = Report::default();
        for item in &board.items {
            let copper = pcb_polygon::copper(item, TOP);
            let outside = !copper.0.iter().all(|p| outline.contains(p));
            let distance = (Euclidean.distance(&copper, outline.exterior()) * 1e6).round() as i64;
            if outside || distance + 2 < 300_000 {
                expected.add(
                    0,
                    &board.rules[0],
                    if outside {
                        "Copper crosses the board outline".into()
                    } else {
                        format!(
                            "Board outline clearance {:.6} mm < 0.300000 mm",
                            distance as f64 / 1e6
                        )
                    },
                    &[item],
                    TOP,
                );
            }
        }
        assert_eq!(keys(&check(&board)), keys(&expected));
    }
}
