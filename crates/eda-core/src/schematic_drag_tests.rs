use super::*;

#[test]
fn spatial_drag_keeps_shared_one_vertex_wire_endpoints_fixed() {
    let (mut doc, id, at, _) = fixture();
    let a = add_wire(&mut doc, vec![at]);
    let b = add_wire(&mut doc, vec![at]);
    let delta = Point::mm(5, 5);
    let next = preview(&doc, &command(vec![id], 0, delta)).unwrap();
    assert_eq!(path(&next, a), &[at]);
    assert_eq!(path(&next, b), &[at]);
    let to = transform_point(at, Point::default(), 0, delta).unwrap();
    assert!(next.schematic.as_ref().unwrap().primitives.iter().any(|p| {
        p.id != a
            && p.id != b
            && wire(p).is_some_and(|path| path.first() == Some(&at) && path.last() == Some(&to))
    }));
}

#[test]
fn port_far_end_connects_snaps_and_drags_without_using_io_direction() {
    for style in [0, 4] {
        for io_type in 0..4 {
            let mut doc = Document::new(DocumentKind::Schematic, "Ports");
            let at = Point::mm(40, 40);
            let port = primitive(
                K::Port {
                    position: at,
                    width_nm: 10_000_000,
                    height_nm: 2_500_000,
                    style,
                    io_type,
                    name: "SIGNAL".into(),
                    font: 0,
                },
                None,
            );
            let end = if style == 0 {
                Point::mm(50, 40)
            } else {
                Point::mm(40, 50)
            };
            let id = port.id;
            doc.schematic.as_mut().unwrap().primitives.push(port);
            let s = doc.schematic.as_ref().unwrap();
            assert!(connection_at(s, end));
            assert_eq!(
                snap_point(
                    s,
                    Point {
                        x: end.x + 100,
                        y: end.y + 100
                    },
                    1_000_000,
                    500
                ),
                end
            );
            let wire_id = add_wire(&mut doc, vec![end, Point::mm(70, 60)]);
            let s = doc.schematic.as_ref().unwrap();
            assert!(crate::cross_probe::touches(
                s.primitive(id).unwrap(),
                s.primitive(wire_id).unwrap()
            ));
            let moved = preview(&doc, &command(vec![id], 0, Point::mm(5, 5))).unwrap();
            let scene = moved.schematic.as_ref().unwrap();
            assert!(crate::cross_probe::touches(
                scene.primitive(id).unwrap(),
                scene.primitive(wire_id).unwrap()
            ));
            assert_eq!(path(&moved, wire_id).last(), Some(&Point::mm(70, 60)));
        }
    }
}

fn fixture() -> (Document, Uuid, Point, Point) {
    let mut doc = Document::new(DocumentKind::Schematic, "Connected drag");
    let f = place(Placement::Resistor, Point::mm(40, 40), 0, "R1", "10k", 0).unwrap();
    let id = f.components[0].id;
    let ends: Vec<_> = f.primitives.iter().filter_map(pin_end).collect();
    let s = doc.schematic.as_mut().unwrap();
    s.components = f.components;
    s.primitives = f.primitives;
    (doc, id, ends[0], ends[1])
}
fn add_wire(doc: &mut Document, points: Vec<Point>) -> Uuid {
    let mut p = primitive(
        K::Polyline {
            points,
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    p.width_nm = 203_200;
    p.color = 0x123456;
    let id = p.id;
    doc.schematic.as_mut().unwrap().primitives.push(p);
    id
}
fn path(doc: &Document, id: Uuid) -> Vec<Point> {
    wire(doc.schematic.as_ref().unwrap().primitive(id).unwrap())
        .unwrap()
        .to_vec()
}
fn command(ids: Vec<Uuid>, turns: u32, delta: Point) -> Edit {
    Edit::Drag {
        ids,
        pivot: Point::mm(40, 40),
        quarter_turns: turns,
        delta,
    }
}
#[test]
fn drag_stretches_wires_atomically_and_move_still_disconnects() {
    let (mut doc, id, left, right) = fixture();
    let a = Point {
        x: 10_000_000,
        y: left.y,
    };
    let b = Point {
        x: 80_000_000,
        y: right.y,
    };
    let l = add_wire(&mut doc, vec![a, left]);
    let r = add_wire(&mut doc, vec![right, b]);
    let original = doc.clone();
    let edit = command(vec![id], 0, Point::mm(5, -10));
    let shown = preview(&doc, &edit).unwrap();
    assert_eq!(doc, original);
    assert_eq!(
        path(&shown, l),
        vec![
            a,
            Point {
                x: a.x,
                y: a.y - 10_000_000
            },
            Point {
                x: left.x + 5_000_000,
                y: left.y - 10_000_000
            }
        ]
    );
    assert_eq!(
        path(&shown, r),
        vec![
            Point {
                x: right.x + 5_000_000,
                y: right.y - 10_000_000
            },
            Point {
                x: b.x,
                y: b.y - 10_000_000
            },
            b
        ]
    );
    for wire_id in [l, r] {
        let p = shown
            .schematic
            .as_ref()
            .unwrap()
            .primitive(wire_id)
            .unwrap();
        assert_eq!(p.color, 0x123456);
        assert_eq!(p.width_nm, 203_200);
    }
    let mut history = History::default();
    history
        .schematic_edit(
            &mut doc,
            Request {
                document: original.id,
                expected_revision: 0,
                edit,
            },
        )
        .unwrap();
    assert_eq!(doc, shown);
    assert_eq!(history.revision, 1);
    history.undo(&mut doc);
    assert_eq!(doc, original);
    history.redo(&mut doc);
    assert_eq!(doc, shown);
    let moved = preview(
        &original,
        &Edit::Transform {
            id,
            pivot: Point::mm(40, 40),
            quarter_turns: 0,
            delta: Point::mm(5, -10),
        },
    )
    .unwrap();
    assert_eq!(path(&moved, l), path(&original, l));
}
#[test]
fn drag_preserves_t_branch_label_and_unconnected_crossing() {
    let (mut doc, id, left, _) = fixture();
    let at = Point {
        x: left.x - 5_000_000,
        y: left.y,
    };
    let trunk = add_wire(&mut doc, vec![Point::mm(0, 40), left]);
    let branch = add_wire(
        &mut doc,
        vec![
            at,
            Point {
                x: at.x,
                y: 60_000_000,
            },
        ],
    );
    let crossing = add_wire(&mut doc, vec![Point::mm(15, 30), Point::mm(15, 50)]);
    let f = place(Placement::NetLabel, Point::mm(10, 40), 0, "", "SIGNAL", 0).unwrap();
    let label = f.primitives[0].clone();
    doc.schematic
        .as_mut()
        .unwrap()
        .primitives
        .extend(f.primitives);
    let shown = preview(&doc, &command(vec![id], 0, Point::mm(5, 10))).unwrap();
    assert_wire_coverage(shown.schematic.as_ref().unwrap(), &path(&doc, branch));
    assert_eq!(path(&shown, branch).last(), path(&doc, branch).last());
    assert_eq!(path(&shown, crossing), path(&doc, crossing));
    assert_eq!(
        shown.schematic.as_ref().unwrap().primitive(label.id),
        Some(&label)
    );
    let raw = connected_transform(
        doc.schematic.as_ref().unwrap(),
        &[id],
        Point::mm(40, 40),
        0,
        Point::mm(5, 10),
    )
    .unwrap();
    let p = wire(raw.primitives.iter().find(|p| p.id == trunk).unwrap()).unwrap();
    assert!(p.contains(&at));
    assert_wire_coverage(shown.schematic.as_ref().unwrap(), p);
    assert!(
        shown
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .filter_map(wire)
            .all(|p| p.windows(2).all(|e| e[0].x == e[1].x || e[0].y == e[1].y))
    );
}
#[test]
fn shared_terminal_keeps_stationary_pins_and_adds_one_stem() {
    let (mut doc, id, left, _) = fixture();
    let a = add_wire(&mut doc, vec![Point::mm(0, 40), left]);
    let b = add_wire(
        &mut doc,
        vec![
            left,
            Point {
                x: left.x,
                y: 60_000_000,
            },
        ],
    );
    let dot = primitive(
        K::Junction {
            position: left,
            radius_nm: 254_000,
        },
        None,
    );
    doc.schematic.as_mut().unwrap().primitives.push(dot.clone());
    let shown = preview(&doc, &command(vec![id], 0, Point::mm(5, 10))).unwrap();
    assert_eq!(path(&shown, a), path(&doc, a));
    assert_eq!(path(&shown, b), path(&doc, b));
    assert_eq!(
        shown.schematic.as_ref().unwrap().primitive(dot.id),
        Some(&dot)
    );
    let fresh: Vec<_> = shown
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter(|p| doc.schematic.as_ref().unwrap().primitive(p.id).is_none())
        .collect();
    assert_eq!(fresh.len(), 1);
    let stem = wire(fresh[0]).unwrap();
    assert_wire_coverage(
        shown.schematic.as_ref().unwrap(),
        &[
            left,
            Point {
                x: left.x,
                y: left.y + 10_000_000,
            },
            Point {
                x: left.x + 5_000_000,
                y: left.y + 10_000_000,
            },
        ],
    );
    assert!(stem.first().unwrap().on_segment(
        left,
        Point {
            x: left.x,
            y: 60_000_000
        }
    ));
    assert_eq!(
        stem.last(),
        Some(&Point {
            x: left.x + 5_000_000,
            y: left.y + 10_000_000
        })
    );
}
#[test]
fn group_drag_moves_internal_wire_and_keeps_external_anchor_fixed() {
    let (mut doc, id, _, right) = fixture();
    let f = place(Placement::Resistor, Point::mm(90, 40), 0, "R2", "1k", 0).unwrap();
    let other = f.components[0].id;
    let end = f.primitives.iter().filter_map(pin_end).next().unwrap();
    doc.schematic
        .as_mut()
        .unwrap()
        .components
        .extend(f.components);
    doc.schematic
        .as_mut()
        .unwrap()
        .primitives
        .extend(f.primitives);
    let internal = add_wire(&mut doc, vec![right, end]);
    let shown = preview(&doc, &command(vec![id, other], 1, Point::mm(5, 10))).unwrap();
    assert_eq!(
        path(&shown, internal),
        path(&doc, internal)
            .into_iter()
            .map(|p| transform_point(p, Point::mm(40, 40), 1, Point::mm(5, 10)).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(shown.schematic.as_ref().unwrap().components.len(), 2);
}
#[test]
fn rotated_drag_uses_electrical_pin_ends_and_stale_or_invalid_edit_is_atomic() {
    let (doc, id, left, right) = fixture();
    let mut doc = doc;
    let l = add_wire(&mut doc, vec![Point::mm(0, 40), left]);
    let r = add_wire(&mut doc, vec![right, Point::mm(80, 40)]);
    let shown = preview(&doc, &command(vec![id], 1, Point::mm(0, 10))).unwrap();
    let s = shown.schematic.as_ref().unwrap();
    let ends: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| p.component == Some(id))
        .filter_map(pin_end)
        .collect();
    assert_eq!(path(&shown, l).last(), Some(&ends[0]));
    assert_eq!(path(&shown, r).first(), Some(&ends[1]));
    let before = doc.clone();
    let mut history = History::default();
    assert!(
        history
            .schematic_edit(
                &mut doc,
                Request {
                    document: before.id,
                    expected_revision: 2,
                    edit: command(vec![id], 0, Point::mm(1, 1))
                }
            )
            .is_err()
    );
    assert!(preview(&doc, &command(vec![id], 0, Point::mm(100_000, 0))).is_err());
    assert_eq!(doc, before);
    assert_eq!(
        preview(&doc, &command(vec![id], 0, Point::default())).unwrap(),
        doc
    );
}
#[test]
fn explicit_junction_drag_preserves_all_incident_branches() {
    let (mut doc, _, _, _) = fixture();
    let at = Point::mm(10, 20);
    let dot = primitive(
        K::Junction {
            position: at,
            radius_nm: 254_000,
        },
        None,
    );
    let id = dot.id;
    doc.schematic.as_mut().unwrap().primitives.push(dot);
    let a = add_wire(&mut doc, vec![Point::mm(0, 20), at]);
    let b = add_wire(&mut doc, vec![at, Point::mm(10, 30)]);
    let shown = preview(&doc, &command(vec![id], 0, Point::mm(2, 3))).unwrap();
    assert_eq!(path(&shown, a).last(), Some(&Point::mm(12, 23)));
    assert_eq!(path(&shown, b).first(), Some(&Point::mm(12, 23)));
}
