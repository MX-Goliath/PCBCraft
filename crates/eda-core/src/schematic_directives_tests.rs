use super::*;
use crate::{
    Document, DocumentKind, History,
    schematic_edit::{self as edit, Edit, Fragment, Request},
};
use std::collections::BTreeMap;
fn at(x: i64, y: i64) -> Point {
    Point::mm(x, y)
}
fn insert(doc: &Document, p: crate::schematic::SchPrimitive) -> Document {
    edit::preview(
        doc,
        &Edit::Insert(Fragment {
            primitives: vec![p],
            ..Default::default()
        }),
    )
    .unwrap()
}
#[test]
fn native_child_parameter_edits_keep_typed_directive_fields_and_undo_in_sync() {
    let mut doc = Document::new(DocumentKind::Schematic, "Children.SchDoc");
    let owner = primitive(K::ParameterSet {
        position: at(10, 10),
        orientation: 0,
        name: "Constraints".into(),
        style: 0,
        parameters: Default::default(),
        rules: vec![],
    });
    let id = owner.id;
    doc = insert(&doc, owner);
    let child =
        crate::schematic_simulation::parameter(id, "ClassName", "Clock", at(10, 8), 0, true);
    let child_id = child.id;
    doc = insert(&doc, child);
    let rule = crate::schematic_simulation::parameter(
        id,
        "Rule",
        "RULEKIND=Width\u{a6}MINLIMIT=10mil",
        at(10, 6),
        0,
        true,
    );
    let rule_id = rule.id;
    doc = insert(&doc, rule);
    let typed = |doc: &Document| match &doc.schematic.as_ref().unwrap().primitive(id).unwrap().kind
    {
        K::ParameterSet {
            parameters, rules, ..
        } => (parameters.clone(), rules.clone()),
        _ => panic!(),
    };
    assert_eq!(typed(&doc).0["ClassName"], "Clock");
    assert_eq!(typed(&doc).1[0]["MINLIMIT"], "10mil");
    let before = doc.clone();
    let mut child = doc
        .schematic
        .as_ref()
        .unwrap()
        .primitive(child_id)
        .unwrap()
        .clone();
    if let K::Text { text, .. } = &mut child.kind {
        *text = "Data".into();
    }
    child.presentation.raw_text = Some("Data".into());
    let mut history = History::default();
    let document = doc.id;
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: 0,
                edit: Edit::PrimitiveProperties {
                    id: child_id,
                    value: Box::new(child),
                },
            },
        )
        .unwrap();
    assert_eq!(typed(&doc).0["ClassName"], "Data");
    history.undo(&mut doc);
    assert_eq!(doc, before);
    doc = edit::preview(&doc, &Edit::Delete(rule_id)).unwrap();
    assert!(typed(&doc).1.is_empty());
    assert_eq!(typed(&doc).0["ClassName"], "Clock");
}
#[test]
fn reflected_masks_recompute_scope_atomically_without_moving_covered_circuitry() {
    let mut doc = Document::new(DocumentKind::Schematic, "Reflect.SchDoc");
    let mask = primitive(K::CompileMask {
        a: at(10, 30),
        b: at(30, 10),
        collapsed: false,
    });
    let id = mask.id;
    doc = insert(&doc, mask);
    let wire = edit::primitive(
        K::Polyline {
            points: vec![at(15, 15), at(20, 20)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    let wid = wire.id;
    doc = insert(&doc, wire.clone());
    let before = doc.clone();
    let fragment = edit::extract(doc.schematic.as_ref().unwrap(), id).unwrap();
    let changed = transform_areas(&fragment, at(10, 30), 0, true, at(0, 0)).unwrap();
    assert!(
        matches!(changed.primitives[0].kind, K::CompileMask{a,b,collapsed:false}
        if a==at(-10,30) && b==at(10,10))
    );
    let mut history = History::default();
    let document = doc.id;
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: 0,
                edit: Edit::PrimitiveProperties {
                    id,
                    value: Box::new(changed.primitives[0].clone()),
                },
            },
        )
        .unwrap();
    let scene = doc.schematic.as_ref().unwrap();
    assert_eq!(scene.primitive(wid), Some(&wire));
    assert!(
        !crate::schematic_compile::exclusions(scene)
            .primitives
            .contains(&wid)
    );
    assert_eq!(scene.compile_masks[0].a, at(-10, 30));
    history.undo(&mut doc);
    assert_eq!(doc, before);
    assert!(
        crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap())
            .primitives
            .contains(&wid)
    );
    let mut invalid = fragment;
    invalid.primitives[0].kind = K::Blanket {
        points: vec![Point { x: i64::MAX, y: 0 }, at(0, 0), at(1, 1)],
        collapsed: false,
    };
    assert!(
        transform_areas(
            &invalid,
            Point { x: i64::MIN, y: 0 },
            0,
            true,
            Point::default()
        )
        .is_err()
    );
    invalid.primitives.push(wire);
    assert!(!area_fragment(&invalid));
}
#[test]
fn polygon_scope_handles_concavity_edges_and_rejects_crossings() {
    let points = vec![at(0, 0), at(10, 0), at(10, 10), at(5, 5), at(0, 10)];
    assert!(valid_polygon(&points));
    assert!(contains(&points, at(2, 2), false));
    assert!(!contains(&points, at(5, 8), true));
    assert!(contains(&points, at(5, 5), true));
    assert!(!contains(&points, at(5, 5), false));
    assert!(!valid_polygon(&[
        at(0, 0),
        at(10, 10),
        at(0, 10),
        at(10, 0)
    ]));
    assert!(!valid_polygon(&[at(0, 0), at(10, 0), at(10, 0)]));
}
#[test]
fn masks_follow_history_copy_move_resize_delete_and_disabled_state() {
    let mut doc = Document::new(DocumentKind::Schematic, "Mask.SchDoc");
    let mask = primitive(K::CompileMask {
        a: at(0, 0),
        b: at(10, 10),
        collapsed: false,
    });
    let id = mask.id;
    doc = insert(&doc, mask);
    let wire = edit::primitive(
        K::Polyline {
            points: vec![at(1, 1), at(2, 2)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    let wid = wire.id;
    doc = insert(&doc, wire);
    assert!(
        crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap())
            .primitives
            .contains(&wid)
    );
    let mut history = History::default();
    let document = doc.id;
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: 0,
                edit: Edit::Delete(id),
            },
        )
        .unwrap();
    assert!(doc.schematic.as_ref().unwrap().compile_masks.is_empty());
    assert!(
        crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap())
            .primitives
            .is_empty()
    );
    history.undo(&mut doc);
    assert_eq!(doc.schematic.as_ref().unwrap().compile_masks.len(), 1);
    let mut mask = doc
        .schematic
        .as_ref()
        .unwrap()
        .primitive(id)
        .unwrap()
        .clone();
    if let K::CompileMask { collapsed, .. } = &mut mask.kind {
        *collapsed = true;
    }
    doc = edit::preview(
        &doc,
        &Edit::PrimitiveProperties {
            id,
            value: Box::new(mask),
        },
    )
    .unwrap();
    assert!(
        crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap())
            .primitives
            .is_empty()
    );
    let original = doc.schematic.as_ref().unwrap().primitive(id).unwrap();
    let copy = edit::extract(doc.schematic.as_ref().unwrap(), id)
        .unwrap()
        .fresh_copy()
        .transformed(at(0, 0), 0, at(20, 20))
        .unwrap();
    assert_ne!(copy.primitives[0].id, id);
    assert_ne!(copy.primitives[0].kind, original.kind);
    let mut rect = K::Blanket {
        points: rectangle(at(0, 0), at(10, 10)),
        collapsed: false,
    };
    resize(&mut rect, 0, at(-2, -3)).unwrap();
    let K::Blanket { points, .. } = rect else {
        panic!()
    };
    assert_eq!(points, rectangle(at(-2, -3), at(10, 10)));
}
#[test]
fn blanket_attachment_unions_net_classes_and_disable_removes_scope() {
    let mut doc = Document::new(DocumentKind::Schematic, "Blanket.SchDoc");
    let blanket = primitive(K::Blanket {
        points: rectangle(at(0, 0), at(10, 10)),
        collapsed: false,
    });
    let bid = blanket.id;
    doc = insert(&doc, blanket);
    let wire = edit::primitive(
        K::Polyline {
            points: vec![at(2, 2), at(5, 2)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    let wid = wire.id;
    doc = insert(&doc, wire);
    // A crossing with neither vertex covered must not pull in an unrelated net.
    let crossing = edit::primitive(
        K::Polyline {
            points: vec![at(-5, 3), at(15, 3)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    let xid = crossing.id;
    doc = insert(&doc, crossing);
    let parameters = BTreeMap::from([
        ("ClassName".into(), "CLK".into()),
        ("DifferentialPair".into(), "False".into()),
    ]);
    doc = insert(
        &doc,
        primitive(K::ParameterSet {
            position: at(0, 4),
            orientation: 0,
            name: "Class".into(),
            style: 0,
            parameters,
            rules: vec![],
        }),
    );
    let mut nets = crate::design_sync::Netlist::default();
    nets.primitive_nets.insert((doc.id, wid), "CLK_P".into());
    nets.primitive_nets.insert((doc.id, xid), "OTHER".into());
    let compiled = compile(&[&doc], &nets, 0);
    assert_eq!(
        compiled.classes[&(0, "CLK".into())],
        BTreeSet::from(["CLK_P".into()])
    );
    assert!(compiled.differential_nets.is_empty());
    let mut blanket = doc
        .schematic
        .as_ref()
        .unwrap()
        .primitive(bid)
        .unwrap()
        .clone();
    if let K::Blanket { collapsed, .. } = &mut blanket.kind {
        *collapsed = true;
    }
    doc = edit::preview(
        &doc,
        &Edit::PrimitiveProperties {
            id: bid,
            value: Box::new(blanket),
        },
    )
    .unwrap();
    assert!(compile(&[&doc], &nets, 0).classes[&(0, "CLK".into())].is_empty());
}

#[test]
fn a_component_segment_crossing_a_concave_notch_is_not_fully_covered() {
    let points = vec![at(0, 0), at(10, 0), at(10, 10), at(5, 5), at(0, 10)];
    assert!(!covers_segment(&points, at(1, 8), at(9, 8)));
    assert!(covers_segment(&points, at(1, 2), at(9, 2)));
}

#[test]
fn component_classes_use_physical_designators_for_multipart_symbols() {
    let mut doc = Document::new(DocumentKind::Schematic, "parts.SchDoc");
    let part = edit::place(edit::Placement::Resistor, at(20, 20), 0, "U1A", "logic", 0).unwrap();
    doc = edit::preview(&doc, &Edit::Insert(part)).unwrap();
    let c = &mut doc.schematic.as_mut().unwrap().components[0];
    c.part = 1;
    c.settings.part_count = 2;
    doc = insert(
        &doc,
        primitive(K::Blanket {
            points: rectangle(at(0, 0), at(40, 40)),
            collapsed: false,
        }),
    );
    doc = insert(
        &doc,
        primitive(K::ParameterSet {
            position: at(0, 10),
            orientation: 0,
            name: "Class".into(),
            style: 0,
            parameters: BTreeMap::from([("CompClassName".into(), "Logic".into())]),
            rules: vec![],
        }),
    );
    let result = compile(&[&doc], &crate::design_sync::Netlist::default(), 0);
    assert_eq!(
        result.classes[&(1, "Logic".into())],
        BTreeSet::from(["U1".into()])
    );
}
