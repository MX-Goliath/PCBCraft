use super::*;
use geo::Contains;

fn rect(x: i64, y: i64, w: i64, h: i64) -> Vec<Point> {
    vec![
        Point::mm(x, y),
        Point::mm(x + w, y),
        Point::mm(x + w, y + h),
        Point::mm(x, y + h),
    ]
}
fn board() -> Board {
    Board {
        outline: straight_contour(&rect(0, 0, 20, 20)),
        nets: vec![Net {
            index: 0,
            name: "GND".into(),
            properties: Fields::new(),
        }],
        polygon_pours: vec![Polygon {
            contour: straight_contour(&rect(0, 0, 20, 20)),
            net: Some(0),
            clearance: 0,
            ..Default::default()
        }],
        ..Default::default()
    }
}
fn material(b: &Board, index: u32) -> MultiPolygon<f64> {
    MultiPolygon(
        b.items
            .iter()
            .filter(|i| {
                i.polygon == Some(index) && !matches!(i.shape, Shape::Region { cutout: true, .. })
            })
            .flat_map(|i| copper(i, i.layer).0)
            .collect(),
    )
}
fn region(points: Vec<Point>, net: Option<u32>) -> Item {
    Item::new(
        TOP,
        net,
        Shape::Region {
            contour: points,
            holes: vec![],
            cutout: false,
        },
    )
}

#[test]
fn batched_obstacles_union_mixed_winding_and_preserve_real_holes() {
    let mut b = board();
    b.items.push(region(rect(2, 2, 6, 6), None));
    let mut clockwise = rect(5, 2, 6, 6);
    clockwise.reverse();
    b.items.push(region(clockwise, None));
    let mut ring = region(rect(12, 2, 6, 6), None);
    if let Shape::Region { holes, .. } = &mut ring.shape {
        holes.push(rect(14, 4, 2, 2));
    }
    b.items.push(ring);
    let p = repour(&b).unwrap();
    let g = material(&p, 0);
    assert!((g.unsigned_area() - (400. - 54. - 32.)).abs() < 1e-5);
    assert!(g.contains(&geo::Point::new(15., 5.)));
    assert!(!g.contains(&geo::Point::new(6., 5.)));
    assert_eq!(p, repour(&p).unwrap());
}

#[test]
fn same_net_modes_apply_to_tracks_but_pad_connection_remains_rule_driven() {
    for mode in [SameNet::All, SameNet::PolygonsOnly, SameNet::None] {
        let mut b = board();
        b.polygon_pours[0].same_net = mode;
        b.items.push(region(rect(2, 2, 4, 4), Some(0)));
        b.items.push(Item::new(
            TOP,
            Some(0),
            Shape::Via {
                position: Point::mm(10, 10),
                diameter: 1_000_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        let g = material(&repour(&b).unwrap(), 0);
        assert_eq!(g.contains(&geo::Point::new(4., 4.)), mode == SameNet::All);
        assert!(g.contains(&geo::Point::new(10., 10.)));
    }
}

#[test]
fn same_net_polygon_precedence_respects_do_not_pour_over() {
    for mode in [SameNet::All, SameNet::PolygonsOnly, SameNet::None] {
        let mut b = board();
        b.polygon_pours[0].contour = straight_contour(&rect(2, 2, 4, 4));
        b.polygon_pours.push(Polygon {
            index: 1,
            priority: 1,
            net: Some(0),
            clearance: 0,
            same_net: mode,
            contour: straight_contour(&rect(0, 0, 20, 20)),
            ..Default::default()
        });
        let p = repour(&b).unwrap();
        assert_eq!(
            material(&p, 1).contains(&geo::Point::new(4., 4.)),
            mode != SameNet::None
        );
    }
}

#[test]
fn nonsignal_polygon_ignores_electrical_objects_and_dead_copper_but_obeys_cutouts() {
    let mut b = board();
    let p = &mut b.polygon_pours[0];
    p.layer = 37;
    p.net = None;
    p.remove_islands = true;
    p.contour = straight_contour(&rect(0, 0, 25, 25));
    let mut item = region(rect(0, 0, 20, 20), None);
    item.layer = 37;
    b.items.push(item);
    let mut cut = region(rect(3, 3, 2, 2), None);
    cut.layer = 37;
    if let Shape::Region { cutout, .. } = &mut cut.shape {
        *cutout = true;
    }
    b.items.push(cut);
    assert!((material(&repour(&b).unwrap(), 0).unsigned_area() - 621.).abs() < 1e-5);
}

#[test]
fn selective_track_keepout_does_not_remove_polygon_copper() {
    let mut b = board();
    let mut item = region(rect(5, 5, 10, 10), None);
    item.keepout = true;
    item.properties
        .insert("KEEPOUTRESTRICTIONS".into(), "2".into());
    b.items.push(item);
    assert!((material(&repour(&b).unwrap(), 0).unsigned_area() - 400.).abs() < 1e-5);
    b.items[0]
        .properties
        .insert("KEEPOUTRESTRICTIONS".into(), "4".into());
    assert!((material(&repour(&b).unwrap(), 0).unsigned_area() - 300.).abs() < 1e-5);
}

#[test]
fn narrow_necks_split_before_dead_copper_filtering() {
    let mut b = board();
    b.polygon_pours[0].contour = straight_contour(&[
        Point::mm(1, 1),
        Point::mm(8, 1),
        Point {
            x: 8_000_000,
            y: 4_900_000,
        },
        Point {
            x: 12_000_000,
            y: 4_900_000,
        },
        Point::mm(12, 1),
        Point::mm(19, 1),
        Point::mm(19, 9),
        Point::mm(12, 9),
        Point {
            x: 12_000_000,
            y: 5_100_000,
        },
        Point {
            x: 8_000_000,
            y: 5_100_000,
        },
        Point::mm(8, 9),
        Point::mm(1, 9),
    ]);
    b.polygon_pours[0].neck_width = 500_000;
    let g = material(&repour(&b).unwrap(), 0);
    assert_eq!(g.0.len(), 2);
    assert!(!g.contains(&geo::Point::new(10., 5.)));
}

#[test]
fn spatial_pruning_includes_remote_obstacles_with_large_matrix_clearance() {
    let mut b = board();
    b.items.push(region(rect(25, 8, 1, 4), None));
    let mut rule = crate::pcb_rules::default_rule("Clearance");
    rule.insert("GAP".into(), "0.1mm".into());
    rule.insert(
        "OBJECTCLEARANCES".into(),
        "ClearanceObj_Poly-ClearanceObj_Region:3149606".into(),
    );
    b.rules.push(rule);
    let g = material(&repour(&b).unwrap(), 0);
    assert!(!g.contains(&geo::Point::new(19., 10.)));
    assert!(g.contains(&geo::Point::new(1., 10.)));
}

#[test]
fn spatial_pruning_includes_larger_bottom_lands_in_legacy_pad_sizes() {
    let mut b = board();
    b.polygon_pours[0].layer = BOTTOM;
    b.items.push(Item::new(
        MULTI_LAYER,
        None,
        Shape::Pad {
            position: Point::mm(25, 10),
            size: Point::mm(1, 1),
            rotation: 0.,
            shape: PadShape::Rectangle,
            hole: 200_000,
            plated: true,
            name: "1".into(),
            bottom_size: Some(Point::mm(14, 2)),
            middle_size: None,
        },
    ));
    let g = material(&repour(&b).unwrap(), 0);
    assert!(!g.contains(&geo::Point::new(19., 10.)));
    assert!(g.contains(&geo::Point::new(1., 10.)));
}

#[test]
fn spatial_pruning_includes_remote_pads_with_advanced_thermal_gap() {
    let mut b = board();
    b.items.push(Item::new(
        TOP,
        Some(0),
        Shape::Pad {
            position: Point::mm(25, 10),
            size: Point::mm(2, 2),
            rotation: 0.,
            shape: PadShape::Round,
            hole: 0,
            plated: true,
            name: "1".into(),
            bottom_size: None,
            middle_size: None,
        },
    ));
    let mut rule = crate::pcb_rules::default_rule("PolygonConnect");
    crate::pcb_rules::set_advanced_connection(&mut rule, true);
    rule.insert("SMDPAD.AIRGAPWIDTH".into(), "8mm".into());
    b.rules.push(rule);
    let g = material(&repour(&b).unwrap(), 0);
    // Away from the horizontal/diagonal thermal spokes, the remote pad's void
    // reaches the polygon even though its actual copper lies outside the outline.
    assert!(!g.contains(&geo::Point::new(19., 12.)));
    assert!(g.contains(&geo::Point::new(1., 10.)));
}

#[test]
fn cancelled_and_unsupported_pours_leave_the_input_and_existing_copper_intact() {
    let mut b = repour(&board()).unwrap();
    let original = b.clone();
    let control = PourControl::default();
    control.cancel();
    assert!(prepare_repour_controlled(Uuid::new_v4(), 0, &b, &control).is_err());
    assert_eq!(b, original);
    b.polygon_pours[0].fill_mode = FillMode::Hatched;
    assert!(repour(&b).unwrap_err().contains("hatched"));
    assert_eq!(b.items, original.items);
    b.polygon_pours[0].fill_mode = FillMode::None;
    assert!(material(&repour(&b).unwrap(), 0).0.is_empty());
}

#[test]
fn shelving_removes_derived_copper_before_any_worker_finishes() {
    let b = repour(&board()).unwrap();
    let mut p = b.polygon_pours[0].clone();
    p.shelved = true;
    let shelved = crate::pcb_tools::replace_polygon(&b, p).unwrap();
    assert!(material(&shelved, 0).0.is_empty());
    assert!(shelved.pours_stale);
}
