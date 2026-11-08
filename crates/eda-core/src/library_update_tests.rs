use super::*;
use crate::{DocumentKind, library_edit};

fn fixture() -> (Document, Fragment) {
    let lib = library_edit::new(DocumentKind::Schematic, "Library");
    let mut definition = edit::extract(
        lib.schematic.as_ref().unwrap(),
        lib.schematic.as_ref().unwrap().components[0].id,
    )
    .unwrap();
    definition.components[0]
        .parameters
        .insert("Value".into(), "12k".into());
    definition.components[0].description = "Updated".into();
    let owner = definition.components[0].id;
    definition.primitives.push(edit::primitive(
        K::Rectangle {
            a: Point::mm(-2, -2),
            b: Point::mm(2, 2),
            radius: Point::default(),
        },
        Some(owner),
    ));
    let mut doc = Document::new(DocumentKind::Schematic, "Sheet");
    let mut instance = definition
        .fresh_copy()
        .transformed(Point::default(), 1, Point::mm(40, 50))
        .unwrap();
    instance.components[0].designator = "U42".into();
    instance.components[0].source_index = 17;
    instance.components[0].settings.source = "Parts.SchLib".into();
    instance.components[0].settings.designator_locked = true;
    doc = edit::preview(&doc, &edit::Edit::Insert(instance)).unwrap();
    let id = doc.schematic.as_ref().unwrap().components[0].id;
    doc = edit::preview(
        &doc,
        &edit::Edit::ParameterValue {
            component: Some(id),
            name: "Value".into(),
            value: Some("47k".into()),
        },
    )
    .unwrap();
    (doc, definition)
}
#[test]
fn update_preserves_placement_identity_annotation_and_parameter_exclusions_with_undo() {
    let (mut doc, definition) = fixture();
    let before = doc.clone();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    let mut options = Options {
        designator_lock: false,
        ..Default::default()
    };
    options.excluded_parameters.insert("vAlUe".into());
    let update = Update {
        component: c.id,
        definition,
        options,
    };
    let mut history = History::default();
    history
        .update_from_library(&mut doc, 0, std::slice::from_ref(&update))
        .unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    assert_eq!(c.id, before.schematic.as_ref().unwrap().components[0].id);
    assert_eq!(c.position, Point::mm(40, 50));
    assert_eq!(c.settings.rotation, 1);
    assert_eq!(c.designator, "U42");
    assert_eq!(c.source_index, 17);
    assert_eq!(c.settings.source, "Parts.SchLib");
    assert!(c.settings.designator_locked);
    assert_eq!(c.parameters["Value"], "47k");
    assert_eq!(c.description, "Updated");
    assert!(!c.parameters.contains_key("Description"));
    let after = doc.clone();
    assert!(history.update_from_library(&mut doc, 0, &[update]).is_err());
    assert_eq!(doc, after);
    history.undo(&mut doc);
    assert_eq!(doc, before);
    history.redo(&mut doc);
    assert_eq!(doc, after);
}
#[test]
fn batch_is_atomic_on_missing_variant_and_duplicate_targets() {
    let (doc, definition) = fixture();
    let component = doc.schematic.as_ref().unwrap().components[0].id;
    let update = Update {
        component,
        definition,
        options: Options::default(),
    };
    let mut invalid = update.clone();
    invalid.component = Uuid::new_v4();
    assert!(preview(&doc, &[update.clone(), invalid]).is_err());
    assert!(preview(&doc, &[update.clone(), update.clone()]).is_err());
    let mut invalid = update;
    invalid.definition.components[0].part = 2;
    assert!(preview(&doc, &[invalid]).is_err());
}
#[test]
fn remove_and_add_parameters_are_independent_and_do_not_duplicate_fonts() {
    let (mut doc, mut definition) = fixture();
    let c = &mut doc.schematic.as_mut().unwrap().components[0];
    c.parameters.insert("Local".into(), "Keep".into());
    definition.components[0]
        .parameters
        .insert("Vendor".into(), "Example".into());
    let update = Update {
        component: c.id,
        definition,
        options: Options {
            remove_parameters: false,
            ..Default::default()
        },
    };
    let count = doc.schematic.as_ref().unwrap().fonts.len();
    let next = preview(&doc, &[update]).unwrap();
    let s = next.schematic.as_ref().unwrap();
    assert_eq!(s.components[0].parameters["Value"], "12k");
    assert_eq!(s.components[0].parameters["Vendor"], "Example");
    assert_eq!(s.components[0].parameters["Local"], "Keep");
    assert_eq!(s.fonts.len(), count);
}
#[test]
fn annotation_locations_and_visibility_are_preserved_or_replaced_independently() {
    let (mut doc, mut definition) = fixture();
    let id = doc.schematic.as_ref().unwrap().components[0].id;
    let source = definition
        .primitives
        .iter_mut()
        .find(|p| p.text_role == Some(TextRole::Designator))
        .unwrap();
    source.presentation.hidden = false;
    source.presentation.allow_sync = false;
    let source_position = if let K::Text { position, .. } = source.kind {
        position
    } else {
        unreachable!()
    };
    let old = doc
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .iter_mut()
        .find(|p| p.text_role == Some(TextRole::Designator))
        .unwrap();
    old.presentation.hidden = true;
    old.presentation.allow_sync = true;
    old.presentation.auto_position = true;
    if let K::Text { position, .. } = &mut old.kind {
        *position = Point::mm(80, 90);
    }
    let annotation_id = old.id;
    let update = Update {
        component: id,
        definition,
        options: Options::default(),
    };
    let preserved = preview(&doc, std::slice::from_ref(&update)).unwrap();
    let s = preserved.schematic.as_ref().unwrap();
    let p = s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .find(|p| p.id == annotation_id)
        .unwrap();
    assert!(p.presentation.hidden);
    assert!(matches!(p.kind,K::Text{position,..} if position==Point::mm(80,90)));
    let mut update = update;
    update.options.preserve_locations = false;
    update.options.preserve_visibility = false;
    let replaced = preview(&doc, &[update]).unwrap();
    let s = replaced.schematic.as_ref().unwrap();
    let p = s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .find(|p| p.id == annotation_id)
        .unwrap();
    assert!(!p.presentation.hidden);
    assert!(p.presentation.allow_sync);
    assert!(p.presentation.auto_position);
    let expected =
        edit::transform_point(source_position, Point::default(), 1, Point::mm(40, 50)).unwrap();
    assert!(matches!(p.kind,K::Text{position,..} if position==expected));
}
#[test]
fn model_only_updates_leave_all_graphics_fonts_images_and_order_untouched() {
    let (doc, mut definition) = fixture();
    definition.components[0]
        .settings
        .models
        .push(crate::schematic::ComponentModel {
            name: "LAND".into(),
            kind: "PCBLIB".into(),
            current: true,
            parameters: Default::default(),
        });
    let update = Update {
        component: doc.schematic.as_ref().unwrap().components[0].id,
        definition,
        options: Options {
            graphics: false,
            parameters: false,
            designator_lock: false,
            part_lock: false,
            ..Default::default()
        },
    };
    let next = preview(&doc, &[update]).unwrap();
    let mut expected = doc;
    expected.schematic.as_mut().unwrap().components[0]
        .settings
        .models = next.schematic.as_ref().unwrap().components[0]
        .settings
        .models
        .clone();
    assert_eq!(next, expected);
}
#[test]
fn moved_library_pins_are_reported_without_moving_external_wires_or_changing_pin_ids() {
    let mut definition = edit::place(
        edit::Placement::Resistor,
        Point::default(),
        0,
        "R1",
        "10k",
        0,
    )
    .unwrap();
    let mut doc = Document::new(DocumentKind::Schematic, "Pins");
    doc = edit::preview(&doc, &edit::Edit::Insert(definition.clone())).unwrap();
    let old = doc.schematic.as_ref().unwrap().clone();
    for p in &mut definition.primitives {
        if let K::Pin { position, .. } = &mut p.kind {
            position.x += 1_000_000;
        }
    }
    let at = old
        .primitives
        .iter()
        .find_map(edit::electrical_endpoint)
        .unwrap();
    let wire = edit::primitive(
        K::Polyline {
            points: vec![
                at,
                Point {
                    x: at.x - 10_000_000,
                    y: at.y,
                },
            ],
            electrical: "Wire".into(),
            closed: false,
        },
        None,
    );
    let wire_id = wire.id;
    doc = edit::preview(
        &doc,
        &edit::Edit::Insert(Fragment {
            primitives: vec![wire],
            ..Default::default()
        }),
    )
    .unwrap();
    let wire = doc
        .schematic
        .as_ref()
        .unwrap()
        .primitive(wire_id)
        .unwrap()
        .clone();
    let update = Update {
        component: old.components[0].id,
        definition,
        options: Default::default(),
    };
    assert!(pin_connections_change(&doc, &update).unwrap());
    let next = preview(&doc, &[update]).unwrap();
    let s = next.schematic.as_ref().unwrap();
    assert_eq!(s.primitive(wire_id), Some(&wire));
    for p in old
        .primitives
        .iter()
        .filter(|p| matches!(p.kind, K::Pin { .. }))
    {
        assert!(s.primitive(p.id).is_some());
    }
}
#[test]
fn locked_or_unsynchronized_parameters_survive_an_explicit_update_request() {
    for locked in [false, true] {
        let (mut doc, definition) = fixture();
        let c = doc.schematic.as_ref().unwrap().components[0].id;
        let mut p = edit::primitive(
            K::Text {
                position: Point::mm(40, 55),
                text: "47k".into(),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            Some(c),
        );
        p.presentation.name = "Value".into();
        p.presentation.record_type = 41;
        p.presentation.raw_text = Some("47k".into());
        p.presentation.locked = locked;
        p.presentation.allow_sync = locked;
        let id = p.id;
        doc.schematic.as_mut().unwrap().primitives.push(p.clone());
        let options = Options {
            preserve_locations: false,
            preserve_visibility: false,
            ..Default::default()
        };
        let next = preview(
            &doc,
            &[Update {
                component: c,
                definition,
                options,
            }],
        )
        .unwrap();
        let s = next.schematic.as_ref().unwrap();
        assert_eq!(s.components[0].parameters["Value"], "47k");
        assert_eq!(s.primitive(id), Some(&p));
    }
}
#[test]
fn unrepresented_inactive_native_variants_reject_graphical_replacement() {
    let (mut doc, definition) = fixture();
    let c = doc.schematic.as_ref().unwrap().components[0].clone();
    doc.schematic.as_mut().unwrap().records =
        std::sync::Arc::new(vec![crate::schematic::SourceRecord {
            index: 18,
            stream: "FileHeader".into(),
            record_type: 14,
            fields: std::collections::BTreeMap::from([
                ("OWNERINDEX".into(), c.source_index.to_string()),
                ("OWNERPARTID".into(), "2".into()),
            ]),
        }]);
    let mut update = Update {
        component: c.id,
        definition,
        options: Default::default(),
    };
    assert!(preview(&doc, std::slice::from_ref(&update)).is_err());
    update.options.graphics = false;
    assert!(preview(&doc, &[update]).is_ok());
}
