use super::*;
use crate::{
    DocumentKind,
    pcb::{Board, Item, Shape, TOP},
};
#[test]
fn free_schematic_primitives_keep_exact_move_and_parameter_classification() {
    let mut a = Document::new(DocumentKind::Schematic, "Indexed anchors");
    for x in 0..256 {
        a.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(crate::schematic_edit::primitive(
                crate::schematic::SchPrimitiveKind::Rectangle {
                    a: Point::mm(x, 10),
                    b: Point::mm(x + 1, 11),
                    radius: Point::default(),
                },
                None,
            ));
    }
    let mut b = a.clone();
    if let crate::schematic::SchPrimitiveKind::Rectangle { a, b, .. } =
        &mut b.schematic.as_mut().unwrap().primitives[200].kind
    {
        a.x += 5_000_000;
        b.x += 5_000_000;
    }
    let diff = compare(Some(&a), Some(&b));
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].kind, Kind::Moved);
    b.schematic.as_mut().unwrap().primitives[200].width_nm += 1;
    let diff = compare(Some(&a), Some(&b));
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].kind, Kind::Modified);
    assert!(compare(Some(&a), Some(&a)).changes.is_empty());
}
#[test]
fn hidden_parameters_do_not_expand_schematic_focus_bounds() {
    let d = Document::new(DocumentKind::Schematic, "Bounds");
    let s = d.schematic.as_ref().unwrap();
    let hidden = crate::schematic_simulation::parameter(
        Uuid::new_v4(),
        "Hidden",
        "Metadata",
        Point::mm(200, 100),
        0,
        true,
    );
    let mut visible = hidden.clone();
    visible.presentation.hidden = false;
    let anchor = Point::mm(10, 10);
    let small = sch_bounds(s, &[&hidden], anchor);
    assert_eq!(small.min, anchor);
    assert_eq!(small.max, anchor);
    assert!(sch_bounds(s, &[&visible], anchor).max.x >= 200_000_000);
}
#[test]
fn native_coordinate_rounding_does_not_turn_translation_into_parameter_change() {
    let mut a = board();
    a.pcb.as_mut().unwrap().items[0]
        .properties
        .insert("UNIQUEID".into(), "MOVED-TRACK".into());
    let mut b = a.clone();
    if let Shape::Track { start, end, .. } = &mut b.pcb.as_mut().unwrap().items[0].shape {
        start.x += 5_000_001;
        end.x += 5_000_000;
    }
    assert_eq!(compare(Some(&a), Some(&b)).changes[0].kind, Kind::Moved);
    if let Shape::Track { width, .. } = &mut b.pcb.as_mut().unwrap().items[0].shape {
        *width += 1;
    }
    assert_eq!(compare(Some(&a), Some(&b)).changes[0].kind, Kind::Modified);
}
fn board() -> Document {
    let mut d = Document::new(DocumentKind::Board, "Review");
    d.pcb = Some(Board {
        items: vec![
            Item::new(
                TOP,
                None,
                Shape::Track {
                    start: Point::mm(5, 5),
                    end: Point::mm(15, 5),
                    width: 300_000,
                },
            ),
            Item::new(
                TOP,
                None,
                Shape::Via {
                    position: Point::mm(20, 10),
                    diameter: 1_000_000,
                    hole: 500_000,
                    from_layer: 1,
                    to_layer: 32,
                },
            ),
        ],
        ..Default::default()
    });
    d
}
#[test]
fn four_kinds_and_combined_movement_preserve_models() {
    let a = board();
    let original = a.clone();
    let mut b = a.clone();
    let pcb = b.pcb.as_mut().unwrap();
    pcb.items[0] = pcb.items[0].with_anchor(Point::mm(8, 8)).unwrap();
    pcb.items.remove(1);
    pcb.items.push(Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(1, 1),
            end: Point::mm(2, 1),
            width: 200_000,
        },
    ));
    let diff = compare(Some(&a), Some(&b));
    for kind in [Kind::Moved, Kind::Removed, Kind::Added] {
        assert!(
            diff.changes.iter().any(|c| c.kind == kind),
            "{kind:?}: {diff:?}"
        );
    }
    if let Shape::Track { width, .. } = &mut b.pcb.as_mut().unwrap().items[0].shape {
        *width = 800_000;
    }
    let diff = compare(Some(&a), Some(&b));
    assert!(
        diff.changes
            .iter()
            .any(|c| c.kind == Kind::Modified && c.moved && !c.fields.is_empty())
    );
    assert_eq!(a, original);
}
#[test]
fn native_uuid_churn_and_record_insertion_do_not_change_unchanged_geometry() {
    let mut a = board();
    a.pcb.as_mut().unwrap().source = Some(std::sync::Arc::new(crate::pcb::SourceArchive {
        filename: "x".into(),
        format: "test".into(),
        bytes_base64: "".into(),
    }));
    let mut b = a.clone();
    for i in &mut b.pcb.as_mut().unwrap().items {
        i.id = Uuid::new_v4();
        i.source = Some(crate::pcb::SourceRecord {
            stream: "Tracks6".into(),
            index: 99,
        });
    }
    assert!(compare(Some(&a), Some(&b)).changes.is_empty());
    let i = b.pcb.as_mut().unwrap().items[0].clone();
    b.pcb.as_mut().unwrap().items[0] = i.with_anchor(Point::mm(6, 6)).unwrap();
    assert_eq!(compare(Some(&a), Some(&b)).changes.len(), 1);
    assert_eq!(compare(Some(&a), Some(&b)).changes[0].kind, Kind::Moved);
}
#[test]
fn added_deleted_document_and_settings_are_visible() {
    let a = board();
    assert!(
        compare(None, Some(&a))
            .changes
            .iter()
            .all(|c| c.kind == Kind::Added)
    );
    assert!(
        compare(Some(&a), None)
            .changes
            .iter()
            .all(|c| c.kind == Kind::Removed)
    );
    let mut b = a.clone();
    b.pcb
        .as_mut()
        .unwrap()
        .rules
        .push(BTreeMap::from([("NAME".into(), "Clearance".into())]));
    let d = compare(Some(&a), Some(&b));
    assert_eq!(d.changes.len(), 1);
    assert!(d.changes[0].label.starts_with("Board"));
}

#[test]
fn parameter_names_that_resemble_internal_fields_are_preserved() {
    let mut a = Document::new(DocumentKind::Schematic, "Params");
    a.schematic
        .as_mut()
        .unwrap()
        .sheet
        .parameters
        .insert("id".into(), "old".into());
    let mut b = a.clone();
    b.schematic
        .as_mut()
        .unwrap()
        .sheet
        .parameters
        .insert("id".into(), "new".into());
    let diff = compare(Some(&a), Some(&b));
    assert_eq!(diff.changes.len(), 1);
    assert!(
        diff.changes[0]
            .fields
            .iter()
            .any(|f| f.name.ends_with("parameters.id") && f.after == "\"new\"")
    );
}
