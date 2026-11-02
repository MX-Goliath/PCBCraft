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
