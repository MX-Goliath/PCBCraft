use super::*;

fn add(s: &mut Schematic, points: &[(i64, i64)]) -> Uuid {
    let mut p = primitive(
        K::Polyline {
            points: points.iter().map(|(x, y)| Point::mm(*x, *y)).collect(),
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    p.color = 0x123456;
    p.width_nm = 203_200;
    let id = p.id;
    s.primitives.push(p);
    id
}
fn doc() -> Document {
    Document::new(DocumentKind::Schematic, "Wire editing")
}
fn path(d: &Document, id: Uuid) -> &[Point] {
    wire(d.schematic.as_ref().unwrap().primitive(id).unwrap()).unwrap()
}
fn drag(id: Uuid, handle: WireHandle, delta: Point) -> Edit {
    Edit::DragWire { id, handle, delta }
}

#[test]
fn segment_drag_anchors_endpoints_and_preserves_style_history_and_serialization() {
    let mut d = doc();
    let id = add(d.schematic.as_mut().unwrap(), &[(10, 20), (30, 20)]);
    let before = d.clone();
    let command = drag(id, WireHandle::Segment(0), Point::mm(7, 5));
    let shown = preview(&d, &command).unwrap();
    assert_eq!(
        path(&shown, id),
        &[
            Point::mm(10, 20),
            Point::mm(10, 25),
            Point::mm(30, 25),
            Point::mm(30, 20)
        ]
    );
    assert_eq!(
        shown
            .schematic
            .as_ref()
            .unwrap()
            .primitive(id)
            .unwrap()
            .color,
        0x123456
    );
    assert_eq!(
        shown
            .schematic
            .as_ref()
            .unwrap()
            .primitive(id)
            .unwrap()
            .width_nm,
        203_200
    );
    assert_eq!(d, before);
    let mut h = History::default();
    h.schematic_edit(
        &mut d,
        Request {
            document: before.id,
            expected_revision: 0,
            edit: command,
        },
    )
    .unwrap();
    assert_eq!(d, shown);
    assert_eq!(h.revision, 1);
    let reopened: Document = serde_json::from_str(&serde_json::to_string(&d).unwrap()).unwrap();
    assert_eq!(reopened, d);
    h.undo(&mut d);
    assert_eq!(d, before);
    h.redo(&mut d);
    assert_eq!(d, shown);
    assert_eq!(
        preview(&d, &drag(id, WireHandle::Segment(1), Point::mm(4, 0))).unwrap(),
        d
    );
}

#[test]
fn middle_segment_stretches_t_branch_dot_but_leaves_crossing_and_other_bends_fixed() {
    let mut d = doc();
    let s = d.schematic.as_mut().unwrap();
    let trunk = add(s, &[(0, 0), (10, 0), (10, 20), (30, 20)]);
    let branch = add(s, &[(10, 10), (20, 10)]);
    let crossing = add(s, &[(5, 15), (25, 15)]);
    let dot = primitive(
        K::Junction {
            position: Point::mm(10, 10),
            radius_nm: 254_000,
        },
        None,
    );
    let dot_id = dot.id;
    s.primitives.push(dot);
    let shown = preview(&d, &drag(trunk, WireHandle::Segment(1), Point::mm(5, 9))).unwrap();
    assert_eq!(
        path(&shown, trunk),
        &[
            Point::mm(0, 0),
            Point::mm(15, 0),
            Point::mm(15, 20),
            Point::mm(30, 20)
        ]
    );
    assert_eq!(
        path(&shown, branch),
        &[Point::mm(15, 10), Point::mm(20, 10)]
    );
    assert_eq!(path(&shown, crossing), path(&d, crossing));
    assert!(
        matches!(shown.schematic.as_ref().unwrap().primitive(dot_id).unwrap().kind,K::Junction { position, .. } if position==Point::mm(15,10))
    );
}

#[test]
fn vertex_drag_preserves_other_vertices_and_fixed_component_hotspot() {
    let mut d = doc();
    let s = d.schematic.as_mut().unwrap();
    let id = add(s, &[(0, 0), (10, 0), (10, 10), (20, 10)]);
    let pin = primitive(
        K::PowerPort {
            cross_sheet: false,
            position: Point::mm(10, 0),
            style: 2,
            text: "GND".into(),
            orientation: 0,
            show_name: true,
            font: 0,
        },
        None,
    );
    let pin_id = pin.id;
    s.primitives.push(pin);
    let shown = preview(&d, &drag(id, WireHandle::Vertex(1), Point::mm(2, 3))).unwrap();
    let raw = wire_drag(
        d.schematic.as_ref().unwrap(),
        id,
        WireHandle::Vertex(1),
        Point::mm(2, 3),
    )
    .unwrap();
    let after = shown.schematic.as_ref().unwrap();
    // Optimization may split/absorb paths, but must preserve their exact union.
    for p in &raw.primitives {
        if let Some(path) = wire(p) {
            assert_wire_coverage(after, path);
        }
    }
    let mut expected = d.schematic.as_ref().unwrap().clone();
    expected.primitives = raw.primitives.clone();
    for p in after.primitives.iter().filter_map(wire) {
        assert_wire_coverage(&expected, p);
        assert!(p.windows(2).all(|e| e[0].x == e[1].x || e[0].y == e[1].y));
    }
    assert_eq!(
        after.primitive(pin_id),
        d.schematic.as_ref().unwrap().primitive(pin_id)
    );
    assert_wire_coverage(
        after,
        &[Point::mm(10, 0), Point::mm(10, 3), Point::mm(12, 3)],
    );
}

#[test]
fn wire_handle_selection_invalid_stale_and_noop_do_not_change_scene() {
    let mut d = doc();
    let id = add(d.schematic.as_mut().unwrap(), &[(0, 0), (10, 0), (10, 20)]);
    let s = d.schematic.as_ref().unwrap();
    assert_eq!(
        wire_handle_at(s, id, Point::mm(10, 0), 1_000_000),
        Some(WireHandle::Vertex(1))
    );
    assert_eq!(
        wire_handle_at(s, id, Point::mm(5, 0), 1_000_000),
        Some(WireHandle::Segment(0))
    );
    assert_eq!(wire_handle_at(s, id, Point::mm(5, 7), 1_000_000), None);
    assert_eq!(
        preview(&d, &drag(id, WireHandle::Vertex(1), Point::default())).unwrap(),
        d
    );
    for (handle, delta) in [
        (WireHandle::Segment(usize::MAX), Point::mm(1, 1)),
        (WireHandle::Vertex(40), Point::mm(1, 1)),
        (WireHandle::Vertex(0), Point::mm(100_001, 0)),
    ] {
        assert!(preview(&d, &drag(id, handle, delta)).is_err());
    }
    let mut history = History::default();
    let before = d.clone();
    assert!(
        history
            .schematic_edit(
                &mut d,
                Request {
                    document: before.id,
                    expected_revision: 1,
                    edit: drag(id, WireHandle::Vertex(0), Point::mm(1, 1))
                }
            )
            .is_err()
    );
    assert_eq!(d, before);
}

#[test]
fn derived_junctions_cover_t_pin_and_overlap_without_connecting_bare_x_crossings() {
    let mut d = doc();
    let s = d.schematic.as_mut().unwrap();
    add(s, &[(0, 10), (30, 10)]);
    let branch = add(s, &[(10, 10), (10, 20)]);
    add(s, &[(20, 0), (20, 20)]);
    add(s, &[(0, 10), (15, 10)]); // Overlap does not introduce an extra branch at 15.
    let dot = primitive(
        K::Junction {
            position: Point::mm(20, 10),
            radius_nm: 254_000,
        },
        None,
    );
    s.primitives.push(dot);
    assert_eq!(
        junctions(s.primitives.iter()),
        vec![(Point::mm(10, 10), 0x123456)]
    );
    s.primitives.retain(|p| p.id != branch);
    assert!(junctions(s.primitives.iter()).is_empty());
    assert!(connection_at(s, Point::mm(7, 10)));
    assert!(!connection_at(s, Point::mm(7, 11)));
}

#[test]
fn indexed_junctions_match_exhaustive_geometry_and_last_wire_color() {
    let mut s = Schematic::blank();
    // Crossings, T branches, diagonals, reversed/zero edges, negative coordinates
    // and overlapping colours. The reference inspects every wire at every node.
    for y in -8..8 {
        for x in -8..8 {
            let id = add(&mut s, &[(x * 5, y * 5), (x * 5 + 10, y * 5)]);
            s.primitives.iter_mut().find(|p| p.id == id).unwrap().color = (x + 8) as u32;
            add(&mut s, &[(x * 5, y * 5), (x * 5, y * 5 + 10)]);
            add(
                &mut s,
                &[(x * 5 + 10, y * 5 + 10), (x * 5, y * 5), (x * 5, y * 5)],
            );
        }
    }
    s.primitives.push(primitive(
        K::Junction {
            position: Point::default(),
            radius_nm: 254_000,
        },
        None,
    ));
    s.primitives.last_mut().unwrap().presentation.hidden = true;
    s.primitives.push(primitive(
        K::Junction {
            position: Point::mm(5, 5),
            radius_nm: 254_000,
        },
        None,
    ));
    s.primitives.push(primitive(
        K::PowerPort {
            position: Point::mm(3, 0),
            orientation: 0,
            style: 2,
            text: "GND".into(),
            show_name: false,
            font: 0,
            cross_sheet: false,
        },
        None,
    ));
    for p in s.primitives.iter_mut().step_by(17) {
        p.presentation.hidden = true;
    }
    let visible: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| !p.presentation.hidden)
        .collect();
    let mut nodes = BTreeSet::new();
    let mut manual = BTreeSet::new();
    let mut terminals = BTreeSet::new();
    for p in &visible {
        if let K::Junction { position, .. } = p.kind {
            manual.insert(key(position));
        } else {
            for at in pin_ends(p) {
                nodes.insert(key(at));
                if !matches!(p.kind, K::Text { .. }) {
                    terminals.insert(key(at));
                }
            }
        }
        if let Some(path) = wire(p) {
            nodes.extend(
                path.first()
                    .into_iter()
                    .chain(path.last())
                    .copied()
                    .map(key),
            );
        }
    }
    let expected: Vec<_> = nodes
        .difference(&manual)
        .filter_map(|node| {
            let at = point(*node);
            let mut rays = BTreeSet::new();
            let mut color = 0;
            for p in &visible {
                if let Some(path) = wire(p) {
                    for e in path.windows(2).filter(|e| at.on_segment(e[0], e[1])) {
                        color = p.color;
                        for end in e {
                            let dx = end.x - at.x;
                            let dy = end.y - at.y;
                            let (mut a, mut b) = (dx.unsigned_abs(), dy.unsigned_abs());
                            if a == 0 && b == 0 {
                                continue;
                            }
                            while b != 0 {
                                (a, b) = (b, a % b);
                            }
                            rays.insert((dx / a as i64, dy / a as i64));
                        }
                    }
                }
            }
            (rays.len() >= 3 || (rays.len() >= 2 && terminals.contains(node)))
                .then_some((at, color))
        })
        .collect();
    assert!(!expected.is_empty());
    assert_eq!(junctions(s.primitives.iter()), expected);
}
