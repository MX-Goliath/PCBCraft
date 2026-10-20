use super::*;

fn commit(history: &mut History, doc: &mut Document, edit: Edit) -> Result<bool, String> {
    let document = doc.id;
    let expected_revision = history.revision;
    history.schematic_edit(
        doc,
        Request {
            document,
            expected_revision,
            edit,
        },
    )
}

#[test]
fn component_lifecycle_preview_commit_rotate_properties_delete_and_history() {
    let mut doc = Document::new(DocumentKind::Schematic, "Editor test");
    let original = doc.clone();
    let mut history = History::default();
    let f = place(Placement::Resistor, Point::mm(20, 30), 0, "R1", "10k", 0).unwrap();
    let id = f.components[0].id;
    let request = Edit::Insert(f);
    let shown = preview(&doc, &request).unwrap();
    assert_eq!(doc, original);
    assert!(!history.can_undo());
    commit(&mut history, &mut doc, request).unwrap();
    assert_eq!(doc, shown);
    let before_move = doc.clone();
    commit(
        &mut history,
        &mut doc,
        Edit::Transform {
            id,
            pivot: Point::mm(20, 30),
            quarter_turns: 1,
            delta: Point::mm(4, -3),
        },
    )
    .unwrap();
    let s = doc.schematic.as_ref().unwrap();
    assert_eq!(s.components[0].position, Point::mm(24, 27));
    assert!(s.primitives.iter().any(|p|matches!(p.kind,K::Pin{position,orientation:3,..} if position==Point{x:24_000_000,y:21_920_000})));
    history.undo(&mut doc);
    assert_eq!(doc, before_move);
    history.redo(&mut doc);
    commit(
        &mut history,
        &mut doc,
        Edit::ComponentProperties {
            id,
            designator: "R17".into(),
            comment: "47k".into(),
        },
    )
    .unwrap();
    let s = doc.schematic.as_ref().unwrap();
    assert!(
        s.primitives
            .iter()
            .any(|p| p.text_role == Some(TextRole::Comment)
                && matches!(&p.kind,K::Text{text,..} if text=="47k"))
    );
    assert_eq!(s.components[0].id, id);
    let edited = doc.clone();
    commit(&mut history, &mut doc, Edit::Delete(id)).unwrap();
    assert!(doc.schematic.as_ref().unwrap().primitives.is_empty());
    history.undo(&mut doc);
    assert_eq!(doc, edited);
}

#[test]
fn stale_preview_and_failed_batch_never_overwrite_committed_work() {
    let mut doc = Document::new(DocumentKind::Schematic, "Atomic test");
    let mut history = History::default();
    let f = place(Placement::Text, Point::mm(10, 10), 0, "", "original", 0).unwrap();
    let id = f.primitives[0].id;
    commit(&mut history, &mut doc, Edit::Insert(f)).unwrap();
    let stale = Request {
        document: doc.id,
        expected_revision: history.revision,
        edit: Edit::Delete(id),
    };
    commit(
        &mut history,
        &mut doc,
        Edit::Text {
            id,
            text: "confirmed".into(),
        },
    )
    .unwrap();
    let confirmed = doc.clone();
    let revision = history.revision;
    assert!(history.schematic_edit(&mut doc, stale).is_err());
    assert!(
        commit(
            &mut history,
            &mut doc,
            Edit::Batch(vec![Edit::Delete(id), Edit::Delete(Uuid::new_v4())])
        )
        .is_err()
    );
    assert_eq!(doc, confirmed);
    assert_eq!(history.revision, revision);
    assert!(
        commit(
            &mut history,
            &mut doc,
            Edit::Transform {
                id,
                pivot: Point::default(),
                quarter_turns: 0,
                delta: Point { x: i64::MAX, y: 0 }
            }
        )
        .is_err()
    );
    assert_eq!(doc, confirmed);
    history.undo(&mut doc);
    assert!(
        doc.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="original"))
    );
}

#[test]
fn clipboard_remaps_fonts_and_ids_and_keeps_component_ownership() {
    let mut source = Document::new(DocumentKind::Schematic, "Source");
    let mut h = History::default();
    let s = source.schematic.as_mut().unwrap();
    let mut custom = s.fonts[0].clone();
    custom.height_nm = 5_080_000;
    s.fonts.push(custom.clone());
    let f = place(Placement::Capacitor, Point::mm(20, 20), 0, "C1", "1uF", 1).unwrap();
    let old = f.components[0].id;
    commit(&mut h, &mut source, Edit::Insert(f)).unwrap();
    let copied = extract(source.schematic.as_ref().unwrap(), old)
        .unwrap()
        .fresh_copy();
    let new = copied.components[0].id;
    assert_ne!(new, old);
    assert!(copied.primitives.iter().all(|p| p.component == Some(new)));
    let mut target = Document::new(DocumentKind::Schematic, "Target");
    let mut history = History::default();
    commit(&mut history, &mut target, Edit::Insert(copied)).unwrap();
    assert_eq!(target.schematic.as_ref().unwrap().fonts[1], custom);
    let before = target.clone();
    history.undo(&mut target);
    history.redo(&mut target);
    assert_eq!(target, before);
    assert_eq!(source.schematic.as_ref().unwrap().components[0].id, old);
}

#[test]
fn snapping_uses_electrical_pin_end_and_wire_interior_without_changing_model() {
    let mut s = Schematic::blank();
    s.primitives.push(primitive(
        K::Pin {
            position: Point::mm(10, 10),
            length_nm: 5_000_000,
            orientation: 2,
            name: "A".into(),
            number: "1".into(),
            show_name: true,
            show_number: true,
            font: 0,
            number_font: None,
            electrical: 4,
            outer_symbol: 0,
            inner_symbol: 0,
        },
        None,
    ));
    s.primitives.push(primitive(
        K::Polyline {
            points: vec![Point::mm(20, 20), Point::mm(40, 20)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    ));
    let original = s.clone();
    assert_eq!(
        snap_point(
            &s,
            Point {
                x: 5_100_000,
                y: 10_100_000
            },
            2_540_000,
            200_000
        ),
        Point::mm(5, 10)
    );
    assert_eq!(
        snap_point(
            &s,
            Point {
                x: 27_000_000,
                y: 20_100_000
            },
            2_540_000,
            200_000
        ),
        Point::mm(27, 20)
    );
    assert_eq!(
        snap_point(&s, Point::mm(-3, -3), 2_000_000, 0),
        Point::mm(-2, -2)
    );
    assert_eq!(s, original);
}
