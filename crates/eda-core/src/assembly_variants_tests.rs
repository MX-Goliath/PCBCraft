use super::*;
use crate::{DocumentKind, schematic_edit::primitive};

fn fixture() -> Document {
    let mut d = Document::new(DocumentKind::Schematic, "Variant fixture");
    let s = d.schematic.as_mut().unwrap();
    let c = Component {
        settings: Default::default(),
        id: Uuid::new_v4(),
        source_index: edit::AUTHORED,
        position: Point::mm(20, 20),
        designator: "R1".into(),
        comment: "=Value".into(),
        library_reference: "Resistor".into(),
        description: String::new(),
        part: 1,
        display_mode: 0,
        parameters: BTreeMap::from([("Value".into(), "10k".into())]),
    };
    let id = c.id;
    s.components.push(c);
    s.primitives.push(primitive(
        SchPrimitiveKind::Rectangle {
            a: Point::mm(18, 19),
            b: Point::mm(22, 21),
            radius: Point::default(),
        },
        Some(id),
    ));
    s.primitives.push(primitive(
        SchPrimitiveKind::Pin {
            position: Point::mm(18, 20),
            length_nm: 2_000_000,
            orientation: 0,
            name: "A".into(),
            number: "1".into(),
            show_name: false,
            show_number: true,
            font: 0,
            electrical: 4,
            number_font: None,
            outer_symbol: 0,
            inner_symbol: 0,
        },
        Some(id),
    ));
    let mut p = primitive(
        SchPrimitiveKind::Text {
            position: Point::mm(18, 23),
            text: "10k".into(),
            font: 0,
            orientation: 0,
            justification: 0,
        },
        Some(id),
    );
    p.text_role = Some(TextRole::Comment);
    p.presentation.name = "Comment".into();
    p.presentation.raw_text = Some("=Value".into());
    s.primitives.push(p);
    s.primitives.push(primitive(
        SchPrimitiveKind::Polyline {
            points: vec![Point::mm(16, 20), Point::mm(10, 20)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    ));
    d
}
#[test]
fn assembly_projection_keeps_base_pins_wires_and_history_unchanged() {
    let d = fixture();
    let original = d.clone();
    let c = &d.schematic.as_ref().unwrap().components[0];
    let mut v = Variant::new("Economy".into());
    let change = v.variation_mut(&component_uid(d.schematic.as_ref().unwrap(), c, ""), "R1");
    change.kind = 1;
    change.parameters.insert("Value".into(), "22k".into());
    let projected = project(&d, Some(&v), "", &Default::default()).unwrap();
    let s = projected.schematic.as_ref().unwrap();
    assert_eq!(d, original);
    assert_eq!(
        s.primitives.len(),
        d.schematic.as_ref().unwrap().primitives.len() + 2
    );
    assert_eq!(
        pin_signature(&s.primitives, c.id),
        pin_signature(&d.schematic.as_ref().unwrap().primitives, c.id)
    );
    let text = s
        .primitives
        .iter()
        .find(|p| p.text_role == Some(TextRole::Comment))
        .unwrap();
    if let SchPrimitiveKind::Text { text, font, .. } = &text.kind {
        assert_eq!(text, "22k");
        assert!(s.fonts[*font].italic);
    } else {
        panic!()
    }
    assert_eq!(
        s.primitives.iter().find(|p| p.component.is_none()),
        d.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| p.component.is_none())
    );
    assert_eq!(project(&d, None, "", &Default::default()).unwrap(), d);
    let style = DisplayStyle {
        cross: false,
        ..Default::default()
    };
    assert_eq!(
        project(&d, Some(&v), "", &style)
            .unwrap()
            .schematic
            .unwrap()
            .primitives
            .len(),
        original.schematic.unwrap().primitives.len()
    );
}
#[test]
fn variants_are_uid_scoped_and_invalid_history_commits_are_atomic() {
    let mut v = Variant::new("A".into());
    v.variation_mut("\\LEFT\\RUID", "R1").kind = 1;
    assert!(v.variation("\\RIGHT\\RUID", "R1").is_none());
    assert_eq!(v.variation("left\\ruid", "R1").unwrap().kind, 1);
    let mut current = vec![];
    let mut h = History::default();
    h.commit(&mut current, vec![v.clone()]).unwrap();
    let revision = h.revision;
    assert!(h.commit(&mut current, vec![v.clone(), v.clone()]).is_err());
    assert_eq!(h.revision, revision);
    assert_eq!(current, vec![v.clone()]);
    h.undo(&mut current);
    assert!(current.is_empty());
    h.redo(&mut current);
    assert_eq!(current, vec![v]);
}
#[test]
fn alternate_graphics_can_change_but_physical_pins_cannot() {
    let d = fixture();
    let s = d.schematic.as_ref().unwrap();
    let c = &s.components[0];
    let mut a = edit::extract(s, c.id).unwrap().fresh_copy();
    a.components[0].library_reference = "Alternate".into();
    a.components[0].comment = "Alternate value".into();
    let p = a.components[0].position;
    a = a.transformed(p, 0, Point { x: -p.x, y: -p.y }).unwrap();
    assert!(alternate_for(s, c, &a).is_ok());
    let mut v = Variant::new("Alternate".into());
    let change = v.variation_mut(&component_uid(s, c, ""), "R1");
    change.kind = 2;
    change.alternate = Some(a.clone());
    let next = project(&d, Some(&v), "", &Default::default()).unwrap();
    assert_eq!(
        next.schematic.as_ref().unwrap().components[0].library_reference,
        "Alternate"
    );
    assert_eq!(
        pin_signature(&next.schematic.unwrap().primitives, c.id),
        pin_signature(&s.primitives, c.id)
    );
    if let SchPrimitiveKind::Pin { position, .. } = &mut a
        .primitives
        .iter_mut()
        .find(|p| matches!(p.kind, SchPrimitiveKind::Pin { .. }))
        .unwrap()
        .kind
    {
        position.x += 1;
    }
    assert!(alternate_for(s, c, &a).is_err());
}
#[test]
fn variant_parameters_override_sheet_special_strings_without_component_variation() {
    let mut d = fixture();
    let s = d.schematic.as_mut().unwrap();
    s.sheet.parameters.insert("Title".into(), "Base".into());
    let mut p = primitive(
        SchPrimitiveKind::Text {
            position: Point::mm(1, 1),
            text: "Base".into(),
            font: 0,
            orientation: 0,
            justification: 0,
        },
        None,
    );
    p.presentation.raw_text = Some("=Title".into());
    s.primitives.push(p);
    let mut v = Variant::new("B".into());
    v.parameters.insert("Title".into(), "Assembly B".into());
    let next = project(&d, Some(&v), "", &Default::default()).unwrap();
    assert!(
        matches!(&next.schematic.unwrap().primitives.last().unwrap().kind,SchPrimitiveKind::Text{text,..} if text=="Assembly B")
    );
    assert_eq!(d.schematic.unwrap().sheet.parameters["Title"], "Base");
}

#[test]
fn bulk_edits_are_atomic_and_history_restores_all_components() {
    let v = Variant::new("Assembly".into());
    let id = v.id.clone();
    let components = vec![
        ComponentRef {
            uid: "\\LEFT\\R".into(),
            designator: "R1".into(),
        },
        ComponentRef {
            uid: "\\RIGHT\\R".into(),
            designator: "R1".into(),
        },
    ];
    let mut current = vec![v];
    let original = current.clone();
    let next = edited(
        &current,
        Edit::SetKind {
            id: id.clone(),
            components: components.clone(),
            kind: 1,
        },
    )
    .unwrap();
    let mut history = History::default();
    history.commit(&mut current, next).unwrap();
    assert_eq!(current[0].variations.len(), 2);
    assert!(current[0].variations.iter().all(|v| v.kind == 1));
    let before = current.clone();
    assert!(
        edited(
            &current,
            Edit::SetKind {
                id: id.clone(),
                components: components.clone(),
                kind: 2
            }
        )
        .is_err()
    );
    assert_eq!(current, before);
    history.undo(&mut current);
    assert_eq!(current, original);
    history.redo(&mut current);
    assert_eq!(current, before);
    let next = edited(
        &current,
        Edit::SetParameter {
            id: id.clone(),
            components: components.clone(),
            name: "Value".into(),
            value: Some("22k".into()),
        },
    )
    .unwrap();
    let next = edited(
        &next,
        Edit::SetParameter {
            id: id.clone(),
            components: components.clone(),
            name: "value".into(),
            value: Some("33k".into()),
        },
    )
    .unwrap();
    assert!(
        next[0]
            .variations
            .iter()
            .all(|v| v.parameters.len() == 1 && v.parameters["value"] == "33k")
    );
    let next = edited(&next, Edit::Reset { id, components }).unwrap();
    assert!(next[0].variations.is_empty());
}

#[test]
fn exact_uid_wins_and_stale_links_require_explicit_unambiguous_rebinding() {
    let mut v = Variant::new("A".into());
    v.variations.push(Variation {
        designator: "R1".into(),
        kind: 1,
        ..Default::default()
    });
    v.variations.push(Variation {
        uid: "\\EXACT".into(),
        designator: "R1".into(),
        kind: 0,
        ..Default::default()
    });
    assert_eq!(v.variation("exact", "R1").unwrap().kind, 0);
    let components = vec![ComponentRef {
        uid: "\\NEW".into(),
        designator: "R1".into(),
    }];
    assert_eq!(unresolved(&v, &components), vec![1]);
    let id = v.id.clone();
    let next = edited(
        &[v],
        Edit::Rebind {
            id,
            index: 1,
            component: components[0].clone(),
        },
    )
    .unwrap();
    assert!(unresolved(&next[0], &components).is_empty());
    let mut bad = next.clone();
    bad[0].name = " [No Variations] ".into();
    assert!(validate(&bad).is_err());
    bad = next;
    bad[0].parameters = BTreeMap::from([("A".into(), "1".into()), ("a".into(), "2".into())]);
    assert!(validate(&bad).is_err());
}

#[test]
fn display_style_mesh_text_and_parameter_font_are_read_only_projections() {
    let d = fixture();
    let original = d.clone();
    let c = &d.schematic.as_ref().unwrap().components[0];
    let mut v = Variant::new("Assembly".into());
    let change = v.variation_mut(&component_uid(d.schematic.as_ref().unwrap(), c, ""), "R1");
    change.kind = 1;
    change.parameters.insert("Value".into(), "33k".into());
    let style = DisplayStyle {
        cross: false,
        mesh: true,
        dim: false,
        not_fitted_text: "DNP".into(),
        parameter_suffix: "*".into(),
        parameter_bold: true,
        ..Default::default()
    };
    let s = project(&d, Some(&v), "", &style)
        .unwrap()
        .schematic
        .unwrap();
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,SchPrimitiveKind::Text{text,..} if text=="DNP"))
    );
    assert!(s.primitives.iter().any(|p|matches!(&p.kind,SchPrimitiveKind::Text{text,font,..} if text=="33k*"&&s.fonts[*font].bold&&s.fonts[*font].italic)));
    let style = DisplayStyle {
        modify_parameters: false,
        cross: false,
        ..Default::default()
    };
    let s = project(&d, Some(&v), "", &style)
        .unwrap()
        .schematic
        .unwrap();
    assert!(s.primitives.iter().any(|p|matches!(&p.kind,SchPrimitiveKind::Text{text,font,..} if text=="33k"&&!s.fonts[*font].italic)));
    assert_eq!(d, original);
}

#[test]
fn assembly_metadata_uses_alternate_values_and_excludes_not_fitted_parts() {
    let d = fixture();
    let s = d.schematic.as_ref().unwrap();
    let c = &s.components[0];
    let mut v = Variant::new("BOM".into());
    let change = v.variation_mut(&component_uid(s, c, ""), "R1");
    change.parameters.insert("value".into(), "47k".into());
    assert_eq!(
        assembly_component(s, c, Some(&v), "")
            .unwrap()
            .unwrap()
            .comment,
        "47k"
    );
    v.variations[0].kind = 1;
    assert!(assembly_component(s, c, Some(&v), "").unwrap().is_none());
    let mut alternate = edit::extract(s, c.id).unwrap();
    alternate.components[0].library_reference = "Alternate".into();
    alternate.components[0]
        .parameters
        .insert("Value".into(), "100k".into());
    v.variations[0].alternate = Some(alternate);
    v.variations[0].kind = 2;
    v.variations[0].parameters.clear();
    let next = assembly_component(s, c, Some(&v), "").unwrap().unwrap();
    assert_eq!(next.library_reference, "Alternate");
    assert_eq!(next.comment, "100k");
    assert_eq!(next.id, c.id);
    assert_eq!(next.designator, "R1");
}
