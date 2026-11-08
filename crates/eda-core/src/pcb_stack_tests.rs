use super::*;
fn pad() -> Item {
    Item::new(
        MULTI_LAYER,
        None,
        Shape::Pad {
            position: Point::default(),
            size: Point::mm(3, 3),
            rotation: 0.0,
            shape: PadShape::Round,
            hole: 500_000,
            plated: true,
            name: "1".into(),
            bottom_size: None,
            middle_size: None,
        },
    )
}
fn board() -> Board {
    let mut b = Board::default();
    b.layers.insert(
        1,
        PcbLayer {
            id: 2,
            name: "Mid".into(),
            copper: true,
            order: 1,
        },
    );
    b.layers.iter_mut().find(|l| l.id == BOTTOM).unwrap().order = 2;
    b
}
fn rule(priority: u32, scope: &str, expansion: &str) -> Fields {
    Fields::from([
        ("RULEKIND".into(), "SolderMaskExpansion".into()),
        ("PRIORITY".into(), priority.to_string()),
        ("SCOPE1EXPRESSION".into(), scope.into()),
        ("EXPANSION".into(), expansion.into()),
        ("NAME".into(), format!("rule{priority}")),
    ])
}
#[test]
fn stack_modes_preserve_layer_geometry_and_drive_selection_and_bounds() {
    let b = board();
    let mut p = pad();
    p.set_stack_mode(StackMode::TopMiddleBottom, &b);
    let mut bottom = p.pad_geometry(BOTTOM).unwrap();
    bottom.size = Point::mm(6, 2);
    bottom.shape = PadShape::Rectangle;
    bottom.offset = Point::mm(1, 0);
    p.set_pad_geometry(BOTTOM, bottom.clone());
    assert!(p.hit_distance_on_layer(Point::mm(3, 0), TOP) > 0.0);
    assert_eq!(p.hit_distance_on_layer(Point::mm(3, 0), BOTTOM), 0.0);
    assert_eq!(p.bounds().max.x, 4_000_000);
    p.set_stack_mode(StackMode::Full, &b);
    assert_eq!(p.pad_geometry(BOTTOM), Some(bottom));
    assert_eq!(p.pad_stack.as_ref().unwrap().layers.len(), 3);
    p.set_stack_mode(StackMode::Simple, &b);
    assert_eq!(p.pad_geometry(BOTTOM), p.pad_geometry(TOP));
}
#[test]
fn slot_rotation_and_offset_cutout_produce_real_surfaces() {
    let mut p = pad();
    p.set_stack_mode(StackMode::Simple, &board());
    let s = p.pad_stack.as_mut().unwrap();
    s.drill_shape = DrillShape::Slot;
    s.slot_length = 2_000_000;
    s.drill_rotation = 90.0;
    let bounds = Bounds::points(&p.drill_outline());
    assert_eq!(bounds.max.y - bounds.min.y, 2_000_000);
    assert_eq!(bounds.max.x - bounds.min.x, 500_000);
    let faces = cutout_surface(&p.pad_outline(TOP), &p.drill_outline());
    assert_eq!(faces.len(), 1);
    assert_eq!(faces[0].holes.len(), 1);
    // A drill crossing the land boundary produces a notch, not a closed hole.
    let mut g = p.pad_geometry(TOP).unwrap();
    g.offset = Point { x: 1_300_000, y: 0 };
    p.set_pad_geometry(TOP, g);
    let faces = cutout_surface(&p.pad_outline(TOP), &p.drill_outline());
    assert!(faces.iter().all(|f| f.holes.is_empty()));
}
#[test]
fn component_flip_swaps_stack_sides_offsets_and_mask_tenting() {
    let mut b = board();
    let id = Uuid::new_v4();
    b.components.push(Component {
        id,
        index: 0,
        designator: "U1".into(),
        comment: "".into(),
        footprint: "".into(),
        footprint_parameters: None,
        parameters: Some(Default::default()),
        position: Point::default(),
        rotation: 0.0,
        layer: TOP,
        locked: false,
        properties: Fields::new(),
    });
    let mut p = pad();
    p.component = Some(0);
    p.set_stack_mode(StackMode::Full, &b);
    let mut g = p.pad_geometry(BOTTOM).unwrap();
    g.size = Point::mm(4, 2);
    g.offset = Point::mm(1, 1);
    p.set_pad_geometry(BOTTOM, g);
    let mut m = MaskSettings::for_item(&p);
    m.solder[0].tented = true;
    p.masks = Some(Box::new(m));
    b.items.push(p);
    let next = crate::pcb_tools::flip_component(&b, id).unwrap();
    let g = next.items[0].pad_geometry(TOP).unwrap();
    assert_eq!(g.size, Point::mm(4, 2));
    assert_eq!(g.offset, Point::mm(1, -1));
    let m = next.items[0].masks.as_ref().unwrap();
    assert!(!m.solder[0].tented);
    assert!(m.solder[1].tented);
    let again = crate::pcb_tools::flip_component(&next, id).unwrap();
    assert_eq!(again.items[0].pad_stack, b.items[0].pad_stack);
    assert_eq!(again.items[0].masks, b.items[0].masks);
}
#[test]
fn invalid_stack_edit_is_atomic_and_legacy_size_edits_stay_in_sync() {
    let mut b = board();
    let mut p = pad();
    p.set_stack_mode(StackMode::Full, &b);
    b.items.push(p.clone());
    let before = b.clone();
    p.pad_stack
        .as_mut()
        .unwrap()
        .layers
        .get_mut(&BOTTOM)
        .unwrap()
        .size
        .x = -1;
    assert!(b.edited(Edit::Replace(p)).is_err());
    assert_eq!(b, before);
    let mut p = b.items[0].clone();
    if let Shape::Pad { size, .. } = &mut p.shape {
        *size = Point::mm(4, 4);
    }
    let next = b.edited(Edit::Replace(p)).unwrap();
    assert_eq!(
        next.items[0].pad_geometry(TOP).unwrap().size,
        Point::mm(4, 4)
    );
    assert_eq!(
        next.items[0].pad_geometry(BOTTOM).unwrap().size,
        Point::mm(3, 3)
    );
}
#[test]
fn rounded_mask_expansion_offsets_corner_radius_instead_of_reusing_percentage() {
    let b = board();
    let mut p = pad();
    p.set_pad_geometry(
        TOP,
        PadGeometry {
            size: Point::mm(1, 2),
            shape: PadShape::RoundedRectangle,
            corner_percent: 40,
            offset: Point::default(),
        },
    );
    let mut masks = MaskSettings::for_item(&p);
    masks.solder[0].mode = MaskMode::Manual;
    for expansion in [100_000, -100_000, -250_000] {
        masks.solder[0].expansion = expansion;
        p.masks = Some(Box::new(masks.clone()));
        let points = p.mask_outline(&b, MaskKind::Solder, false).unwrap();
        // Positive X corner starts at the top of its vertical straight edge.
        // r = max(0, 0.2 mm + expansion), so this junction stays at Y=0.8 mm
        // until the corner becomes square under a large negative expansion.
        let expected_radius = (200_000 + expansion).max(0);
        assert_eq!(
            points[0],
            Point {
                x: 500_000 + expansion,
                y: 1_000_000 + expansion - expected_radius,
            }
        );
        assert_eq!(
            Bounds::points(&points).max,
            Point {
                x: 500_000 + expansion,
                y: 1_000_000 + expansion,
            }
        );
    }
}
#[test]
fn mask_priority_scope_manual_override_and_tenting() {
    let mut b = board();
    let mut p = pad();
    b.rules = vec![
        rule(10, "All", "0.1mm"),
        rule(1, "IsPad AND NOT IsVia", "-2mil"),
    ];
    assert_eq!(
        b.resolve_mask(&p, MaskKind::Solder, false)
            .unwrap()
            .expansion,
        -50_800
    );
    b.rules[1].insert("ENABLED".into(), "FALSE".into());
    assert_eq!(
        b.resolve_mask(&p, MaskKind::Solder, false)
            .unwrap()
            .expansion,
        100_000
    );
    let mut m = MaskSettings::for_item(&p);
    m.solder[0].mode = MaskMode::Manual;
    m.solder[0].expansion = 250_000;
    m.solder[0].tented = true;
    p.masks = Some(Box::new(m));
    assert!(
        p.mask_outline(&b, MaskKind::Solder, false)
            .unwrap()
            .is_empty()
    );
    assert!(
        !p.mask_outline(&b, MaskKind::Solder, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        b.resolve_mask(&p, MaskKind::Solder, false).unwrap().rule,
        None
    );
}
#[test]
fn changing_only_masks_preserves_existing_copper_pours() {
    let mut b = board();
    let p = pad();
    b.items.push(p.clone());
    b.polygon_pours.push(crate::pcb_polygon::Polygon {
        contour: crate::pcb_polygon::straight_contour(&[
            Point::mm(0, 0),
            Point::mm(10, 0),
            Point::mm(10, 10),
        ]),
        ..Default::default()
    });
    let mut p = p;
    let mut m = MaskSettings::for_item(&p);
    m.solder[0].tented = true;
    p.masks = Some(Box::new(m));
    let next = b.edited(Edit::Replace(p)).unwrap();
    assert!(!next.pours_stale);
    assert_eq!(next.polygon_pours, b.polygon_pours);
    let mut p = next.items[0].clone();
    p.set_stack_mode(StackMode::Full, &next);
    let full = next.edited(Edit::Replace(p)).unwrap();
    assert!(!full.pours_stale);
    let mut p = full.items[0].clone();
    let mut g = p.pad_geometry(BOTTOM).unwrap();
    g.size = Point::mm(4, 4);
    p.set_pad_geometry(BOTTOM, g);
    assert!(full.edited(Edit::Replace(p)).unwrap().pours_stale);
}
#[test]
fn unknown_priority_scope_and_invalid_percentage_cannot_silently_fall_back() {
    let mut b = board();
    let p = pad();
    b.rules = vec![
        rule(10, "All", "0.1mm"),
        rule(1, "InUnknownClass('x')", "0.2mm"),
    ];
    assert!(b.resolve_mask(&p, MaskKind::Solder, false).is_err());
    b.rules[1].insert("SCOPE1EXPRESSION".into(), "All".into());
    b.rules[1].insert("PERCENTAGE".into(), "NaN".into());
    assert!(b.resolve_mask(&p, MaskKind::Solder, false).is_err());
    assert!(b.replace_mask_rule(1, b.rules[1].clone()).is_err());
}
#[test]
fn mask_rule_edit_preserves_unknown_fields_and_hole_edge_uses_slot() {
    let mut b = board();
    let mut p = pad();
    b.rules.push(rule(1, "All", "0.1mm"));
    b.rules[0].insert("OPAQUE".into(), "retained".into());
    let mut r = b.rules[0].clone();
    r.insert("EXPANSION".into(), "0.2mm".into());
    let next = b.replace_mask_rule(0, r).unwrap();
    assert_eq!(next.rules[0]["OPAQUE"], "retained");
    let mut r = next.rules[0].clone();
    r.insert("OPAQUE".into(), "changed".into());
    assert!(next.replace_mask_rule(0, r).is_err());
    p.set_stack_mode(StackMode::Simple, &b);
    let s = p.pad_stack.as_mut().unwrap();
    s.drill_shape = DrillShape::Slot;
    s.slot_length = 2_000_000;
    s.drill_rotation = 90.0;
    let mut m = MaskSettings::for_item(&p);
    m.solder[0].mode = MaskMode::Manual;
    m.solder[0].from_hole_edge = true;
    m.solder[0].expansion = 100_000;
    p.masks = Some(Box::new(m));
    let bounds = Bounds::points(&p.mask_outline(&b, MaskKind::Solder, false).unwrap());
    assert_eq!(bounds.max.y - bounds.min.y, 2_200_000);
    assert_eq!(bounds.max.x - bounds.min.x, 700_000);
}
