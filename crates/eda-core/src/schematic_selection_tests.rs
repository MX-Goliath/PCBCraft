use super::*;
use crate::schematic_edit::electrical_endpoint as pin_end;

fn add(s: &mut Schematic, points: &[(i64, i64)]) -> Uuid {
    let p = primitive(
        K::Polyline {
            points: points.iter().map(|(x, y)| Point::mm(*x, *y)).collect(),
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    let id = p.id;
    s.primitives.push(p);
    id
}
fn document() -> Document {
    Document::new(DocumentKind::Schematic, "Group editing")
}

#[test]
fn grouped_extraction_preserves_order_hidden_parameters_images_and_native_text_roles() {
    let mut s = Schematic::blank();
    let mut ids = Vec::new();
    for (name, x) in [("R1", 10), ("R2", 30)] {
        let f = place(Placement::Resistor, Point::mm(x, 10), 0, name, "10k", 0).unwrap();
        ids.push(f.components[0].id);
        s.components.extend(f.components);
        s.primitives.extend(f.primitives);
    }
    let mut parameter = s
        .primitives
        .iter()
        .find(|p| p.component == Some(ids[0]) && matches!(p.kind, K::Text { .. }))
        .unwrap()
        .clone();
    parameter.id = Uuid::new_v4();
    parameter.source_index = 0;
    parameter.text_role = None;
    parameter.presentation.hidden = true;
    let parameter_id = parameter.id;
    s.parameter_objects.push(parameter);
    s.records = std::sync::Arc::new(vec![crate::schematic::SourceRecord {
        index: 0,
        stream: "FileHeader".into(),
        record_type: 34,
        fields: Default::default(),
    }]);
    let image = primitive(
        K::Image {
            a: Point::mm(1, 1),
            b: Point::mm(2, 2),
            filename: "selected.png".into(),
            keep_aspect: true,
            embedded: true,
        },
        Some(ids[0]),
    );
    let image_id = image.id;
    s.primitives.push(image);
    s.images = std::sync::Arc::new(BTreeMap::from([
        ("selected.png".into(), "selected bytes".into()),
        ("unused.png".into(), "unused bytes".into()),
    ]));
    let selection = [ids[1], parameter_id, ids[0], image_id, ids[1]];
    let actual = extract_many(&s, &selection).unwrap();
    let mut expected = Fragment {
        fonts: s.fonts.clone(),
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    for id in selection {
        let part = extract(&s, id).unwrap();
        expected
            .components
            .extend(part.components.into_iter().filter(|c| seen.insert(c.id)));
        expected
            .primitives
            .extend(part.primitives.into_iter().filter(|p| seen.insert(p.id)));
        std::sync::Arc::make_mut(&mut expected.images)
            .extend(part.images.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    assert_eq!(actual, expected);
    assert_eq!(
        actual.components.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![ids[1], ids[0]]
    );
    let hidden = actual
        .primitives
        .iter()
        .find(|p| p.id == parameter_id)
        .unwrap();
    assert!(hidden.presentation.hidden);
    assert_eq!(hidden.text_role, Some(TextRole::Designator));
    assert_eq!(actual.images.len(), 1);
    assert!(actual.images.contains_key("selected.png"));
    assert!(extract_many(&s, &[ids[0], Uuid::new_v4()]).is_err());
}
fn path(d: &Document, id: Uuid) -> Vec<Point> {
    wire(d.schematic.as_ref().unwrap().primitive(id).unwrap())
        .unwrap()
        .to_vec()
}

#[test]
fn indexed_overlap_follows_transitive_chain_and_preserves_remote_duplicates() {
    for (dx, dy) in [(1, 0), (0, 1), (1, 1), (-1, 1)] {
        let mut d = document();
        let s = d.schematic.as_mut().unwrap();
        for (a, b) in [(-20, 0), (-5, 15), (10, 30)] {
            add(s, &[(a * dx, a * dy), (b * dx, b * dy)]);
        }
        let remote = add(s, &[(100, 100), (110, 110), (100, 100)]);
        let incoming = primitive(
            K::Polyline {
                points: vec![Point::mm(25 * dx, 25 * dy), Point::mm(40 * dx, 40 * dy)],
                electrical: "Wire".into(),
                closed: false,
            },
            None,
        );
        let command = Edit::Insert(Fragment {
            primitives: vec![incoming],
            ..Default::default()
        });
        let before = d.clone();
        let shown = preview(&d, &command).unwrap();
        let s = shown.schematic.as_ref().unwrap();
        assert_eq!(s.primitives.len(), 2);
        assert_eq!(
            wire(&s.primitives[0]).unwrap(),
            &[Point::mm(-20 * dx, -20 * dy), Point::mm(40 * dx, 40 * dy)]
        );
        assert_eq!(path(&shown, remote), path(&before, remote));
        let mut history = History::default();
        history
            .schematic_edit(
                &mut d,
                Request {
                    document: before.id,
                    expected_revision: 0,
                    edit: command,
                },
            )
            .unwrap();
        assert_eq!(d, shown);
        history.undo(&mut d);
        assert_eq!(d, before);
        history.redo(&mut d);
        assert_eq!(d, shown);
    }
}
fn tx(selection: Selection, connected: bool) -> Edit {
    Edit::TransformSelection {
        selection,
        pivot: Point::default(),
        quarter_turns: 0,
        delta: Point::mm(5, 10),
        connected,
    }
}

#[test]
fn mixed_group_transforms_owned_text_once_and_stretches_external_wires() {
    let mut d = document();
    let f = place(Placement::Resistor, Point::mm(40, 40), 0, "R1", "10k", 0).unwrap();
    let id = f.components[0].id;
    let text = f
        .primitives
        .iter()
        .find(|p| p.text_role == Some(TextRole::Designator))
        .unwrap()
        .id;
    let pin = pin_end(
        f.primitives
            .iter()
            .find(|p| matches!(p.kind, K::Pin { .. }))
            .unwrap(),
    )
    .unwrap();
    let s = d.schematic.as_mut().unwrap();
    s.components = f.components;
    s.primitives = f.primitives;
    let selected_wire = add(s, &[(0, 40), (10, 40)]);
    let external = add(s, &[(10, 40), (10, 20)]);
    let attached = add(s, &[(10, 40), (20, 40)]);
    if let K::Polyline { points, .. } = &mut s
        .primitives
        .iter_mut()
        .find(|p| p.id == attached)
        .unwrap()
        .kind
    {
        points[1] = pin;
    }
    let selection = Selection {
        ids: vec![id, text, selected_wire, id],
        segments: vec![],
    };
    let shown = preview(&d, &tx(selection.clone(), true)).unwrap();
    let extracted = extract_selection(d.schematic.as_ref().unwrap(), &selection).unwrap();
    assert_eq!(extracted.components.len(), 1);
    assert_eq!(
        extracted.primitives.iter().filter(|p| p.id == text).count(),
        1
    );
    let original = d.schematic.as_ref().unwrap().primitive(text).unwrap();
    let expected = Fragment {
        primitives: vec![original.clone()],
        ..Default::default()
    }
    .transformed(Point::default(), 0, Point::mm(5, 10))
    .unwrap();
    assert_eq!(
        shown.schematic.as_ref().unwrap().primitive(text),
        Some(&expected.primitives[0])
    );
    assert_eq!(
        path(&shown, selected_wire),
        vec![Point::mm(5, 50), Point::mm(15, 50)]
    );
    assert_eq!(path(&shown, external).first(), Some(&Point::mm(15, 50)));
    assert_eq!(path(&shown, attached).first(), Some(&Point::mm(15, 50)));
    assert_eq!(
        path(&shown, attached).last(),
        Some(&Point {
            x: pin.x + 5_000_000,
            y: pin.y + 10_000_000
        })
    );
    let moved = preview(&d, &tx(selection, false)).unwrap();
    assert_eq!(path(&moved, external), path(&d, external));
}

#[test]
fn segment_deletion_stops_at_t_and_preserves_remaining_runs_and_undo() {
    let mut d = document();
    let s = d.schematic.as_mut().unwrap();
    let trunk = add(s, &[(0, 10), (30, 10), (30, 30)]);
    let branch = add(s, &[(10, 10), (10, 20)]);
    let span = wire_segment_at(s, trunk, Point::mm(5, 10), 1).unwrap();
    assert_eq!((span.a, span.b), (Point::mm(0, 10), Point::mm(10, 10)));
    let selection = Selection {
        ids: vec![],
        segments: vec![span, span],
    };
    let original = d.clone();
    let mut history = History::default();
    history
        .schematic_edit(
            &mut d,
            Request {
                document: original.id,
                expected_revision: 0,
                edit: Edit::DeleteSelection(selection),
            },
        )
        .unwrap();
    assert_eq!(
        path(&d, trunk),
        vec![Point::mm(10, 10), Point::mm(30, 10), Point::mm(30, 30)]
    );
    assert_eq!(path(&d, branch), path(&original, branch));
    assert!(junctions(d.schematic.as_ref().unwrap().primitives.iter()).is_empty());
    let changed = d.clone();
    history.undo(&mut d);
    assert_eq!(d, original);
    history.redo(&mut d);
    assert_eq!(d, changed);
    let s = d.schematic.as_ref().unwrap();
    let span = wire_segment_at(s, trunk, Point::mm(30, 20), 1).unwrap();
    assert_eq!(
        extract_selection(
            s,
            &Selection {
                ids: vec![],
                segments: vec![span]
            }
        )
        .unwrap()
        .primitives
        .len(),
        1
    );
}

#[test]
fn middle_span_cut_copy_and_mixed_deletion_are_atomic_and_validate_stale_geometry() {
    let mut d = document();
    let s = d.schematic.as_mut().unwrap();
    let id = add(s, &[(0, 0), (10, 0), (10, 10), (20, 10)]);
    let other = add(s, &[(40, 0), (50, 0)]);
    let span = WireSegment {
        id,
        a: Point::mm(10, 0),
        b: Point::mm(10, 10),
    };
    let selection = Selection {
        ids: vec![other],
        segments: vec![span],
    };
    let copied = extract_selection(s, &selection).unwrap().fresh_copy();
    assert_eq!(copied.primitives.len(), 2);
    let cut = preview(&d, &Edit::DeleteSelection(selection.clone())).unwrap();
    assert_eq!(cut.schematic.as_ref().unwrap().primitives.len(), 2);
    assert!(cut.schematic.as_ref().unwrap().primitive(other).is_none());
    let moved = preview(
        &d,
        &tx(
            Selection {
                ids: vec![],
                segments: vec![span],
            },
            false,
        ),
    )
    .unwrap();
    assert_eq!(moved.schematic.as_ref().unwrap().primitives.len(), 4);
    assert!(
        preview(
            &d,
            &Edit::DeleteSelection(Selection {
                ids: vec![other],
                segments: vec![WireSegment {
                    b: Point::mm(50, 50),
                    ..span
                }]
            })
        )
        .is_err()
    );
    assert_eq!(d.schematic.as_ref().unwrap().primitives.len(), 2);
}

#[test]
fn overlapping_wires_merge_without_duplicate_coverage_or_connecting_bare_crossings() {
    let mut d = document();
    let s = d.schematic.as_mut().unwrap();
    let id = add(s, &[(10, 10), (20, 10)]);
    let crossing = add(s, &[(15, 0), (15, 20)]);
    let incoming = primitive(
        K::Polyline {
            points: vec![Point::mm(0, 10), Point::mm(30, 10)],
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    let command = Edit::Insert(Fragment {
        primitives: vec![incoming],
        ..Default::default()
    });
    let shown = preview(&d, &command).unwrap();
    assert_eq!(shown.schematic.as_ref().unwrap().primitives.len(), 2);
    assert_eq!(path(&shown, id), vec![Point::mm(0, 10), Point::mm(30, 10)]);
    assert_eq!(path(&shown, crossing), path(&d, crossing));
    assert!(junctions(shown.schematic.as_ref().unwrap().primitives.iter()).is_empty());
    let f = preview_fragment(&d, &command).unwrap();
    assert!(f.primitives.iter().any(|p| p.id == id));
}

#[test]
fn overlap_optimization_preserves_junctions_styles_and_unrelated_native_wires() {
    let mut d = document();
    let s = d.schematic.as_mut().unwrap();
    let id = add(s, &[(10, 10), (20, 10)]);
    add(s, &[(10, 0), (10, 20)]);
    let unrelated = add(s, &[(0, 50), (20, 50), (20, 50), (10, 50)]);
    let duplicate = add(s, &[(0, 60), (20, 60)]);
    let mut incoming = primitive(
        K::Polyline {
            points: vec![Point::mm(0, 10), Point::mm(30, 10)],
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    incoming.width_nm = 254_000;
    let shown = preview(
        &d,
        &Edit::Insert(Fragment {
            primitives: vec![incoming],
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(
        junctions(shown.schematic.as_ref().unwrap().primitives.iter())
            .iter()
            .any(|(p, _)| *p == Point::mm(10, 10))
    );
    assert_eq!(path(&shown, unrelated), path(&d, unrelated));
    assert_eq!(path(&shown, duplicate), path(&d, duplicate));
    assert!(shown.schematic.as_ref().unwrap().primitive(id).is_some());
    let mut different = d.schematic.as_ref().unwrap().primitive(id).unwrap().clone();
    different.id = Uuid::new_v4();
    different.width_nm = 500_000;
    let shown = preview(
        &d,
        &Edit::Insert(Fragment {
            primitives: vec![different],
            ..Default::default()
        }),
    )
    .unwrap();
    assert_eq!(
        shown.schematic.as_ref().unwrap().primitives.len(),
        d.schematic.as_ref().unwrap().primitives.len() + 1
    );
}

#[test]
fn duplicate_reversed_and_backtracking_wires_are_absorbed_with_undo_and_stable_preview() {
    for path in [
        vec![(0, 10), (20, 10)],
        vec![(30, 10), (10, 10)],
        vec![(0, 10), (30, 10), (10, 10), (40, 10)],
    ] {
        let mut d = document();
        let old = add(d.schematic.as_mut().unwrap(), &[(0, 10), (20, 10)]);
        let mut source = document();
        add(source.schematic.as_mut().unwrap(), &path);
        let command = Edit::Insert(Fragment {
            primitives: source.schematic.unwrap().primitives,
            ..Default::default()
        });
        let expected = preview(&d, &command).unwrap();
        assert_eq!(preview(&d, &command).unwrap(), expected);
        let s = expected.schematic.as_ref().unwrap();
        assert_eq!(s.primitives.len(), 1);
        assert!(s.primitive(old).is_some());
        assert_wire_coverage(
            s,
            &[
                Point::mm(0, 10),
                Point::mm(path.iter().map(|p| p.0).max().unwrap().max(20), 10),
            ],
        );
        let before = d.clone();
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
        assert_eq!(d, expected);
        h.undo(&mut d);
        assert_eq!(d, before);
        h.redo(&mut d);
        assert_eq!(d, expected);
    }
}
#[test]
fn selected_span_noop_and_group_rotation_preserve_geometry_and_ownership() {
    let mut d = document();
    let id = add(
        d.schematic.as_mut().unwrap(),
        &[(0, 0), (10, 0), (10, 10), (20, 10)],
    );
    let span = WireSegment {
        id,
        a: Point::mm(10, 0),
        b: Point::mm(10, 10),
    };
    let selection = Selection {
        ids: vec![],
        segments: vec![span],
    };
    let cmd = Edit::TransformSelection {
        selection: selection.clone(),
        pivot: Point::default(),
        quarter_turns: 4,
        delta: Point::default(),
        connected: false,
    };
    assert_eq!(preview(&d, &cmd).unwrap(), d);
    let rotated = preview(
        &d,
        &Edit::TransformSelection {
            selection,
            pivot: Point::default(),
            quarter_turns: 1,
            delta: Point::default(),
            connected: false,
        },
    )
    .unwrap();
    assert_wire_coverage(
        rotated.schematic.as_ref().unwrap(),
        &[Point::mm(0, 10), Point::mm(-10, 10)],
    );
    assert_wire_coverage(
        rotated.schematic.as_ref().unwrap(),
        &[Point::mm(0, 0), Point::mm(10, 0)],
    );
    assert_wire_coverage(
        rotated.schematic.as_ref().unwrap(),
        &[Point::mm(10, 10), Point::mm(20, 10)],
    );
}

#[test]
fn overlap_union_is_exact_for_horizontal_vertical_diagonal_and_reversed_intervals() {
    for axis in 0..3 {
        for reverse in [false, true] {
            for a in 0..4 {
                for len_a in 1..4 {
                    for b in 0..4 {
                        for len_b in 1..4 {
                            let coord = |t| match axis {
                                0 => (t, 10),
                                1 => (10, t),
                                _ => (t, t),
                            };
                            let mut d = document();
                            let old =
                                add(d.schematic.as_mut().unwrap(), &[coord(a), coord(a + len_a)]);
                            let mut input = document();
                            let mut path = vec![coord(b), coord(b + len_b)];
                            if reverse {
                                path.reverse();
                            }
                            add(input.schematic.as_mut().unwrap(), &path);
                            let shown = preview(
                                &d,
                                &Edit::Insert(Fragment {
                                    primitives: input.schematic.unwrap().primitives,
                                    ..Default::default()
                                }),
                            )
                            .unwrap();
                            let s = shown.schematic.as_ref().unwrap();
                            let overlap = a.max(b) < (a + len_a).min(b + len_b);
                            assert_eq!(
                                s.primitives.len(),
                                if overlap { 1 } else { 2 },
                                "axis {axis}, intervals {a}/{len_a} and {b}/{len_b}"
                            );
                            assert!(s.primitive(old).is_some());
                            if overlap {
                                let (x, y) = coord(a.min(b));
                                let (u, v) = coord((a + len_a).max(b + len_b));
                                assert_wire_coverage(s, &[Point::mm(x, y), Point::mm(u, v)]);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_new_branch_invalidates_a_previously_selected_span_atomically() {
    let mut d = document();
    let s = d.schematic.as_mut().unwrap();
    let id = add(s, &[(0, 10), (30, 10)]);
    let span = wire_segment_at(s, id, Point::mm(15, 10), 0).unwrap();
    let other = add(s, &[(10, 10), (10, 20)]);
    let before = d.clone();
    let mut h = History::default();
    assert!(
        h.schematic_edit(
            &mut d,
            Request {
                document: before.id,
                expected_revision: 0,
                edit: Edit::DeleteSelection(Selection {
                    ids: vec![other],
                    segments: vec![span]
                })
            }
        )
        .is_err()
    );
    assert_eq!(d, before);
    assert_eq!(h.revision, 0);
}
