use super::*;

#[test]
fn manager_open_repairs_native_stale_uids_preserving_all_variation_data() {
    let mut variant = Variant::new("Assembly".into());
    let mut components = Vec::new();
    for (designator, old, new) in [
        ("R112", "\\TIHJCEVI\\WXMVVAEY", "\\TIHJCEVI\\KVRCDJEI"),
        ("R90", "\\TIHJCEVI\\YRMNYXPC", "\\TIHJCEVI\\XCEMIHVS"),
        ("R91", "\\TIHJCEVI\\OQKDWTJD", "\\TIHJCEVI\\HPIOPMMP"),
        ("ZQ1", "\\MNKFAVNF\\WUEKOWZF", "\\MNKFAVNF\\BBCVTPWC"),
    ] {
        variant.variations.push(Variation {
            uid: old.into(),
            designator: designator.into(),
            kind: 1,
            parameters: BTreeMap::from([("Comment".into(), "keep expression".into())]),
            fields: BTreeMap::from([("FutureField".into(), "keep opaque".into())]),
            ..Default::default()
        });
        components.push(ComponentRef {
            uid: new.into(),
            designator: designator.into(),
        });
    }
    let document = fixture();
    let sheet = document.schematic.as_ref().unwrap();
    variant.variations[3].kind = 2;
    variant.variations[3].alternate = Some(edit::extract(sheet, sheet.components[0].id).unwrap());
    let original = vec![variant];
    let result = resolve_links(&original, &components).unwrap();
    assert_eq!(result.affected, ["R112", "R90", "R91", "ZQ1"]);
    assert!(result.unresolved.is_empty());
    for ((old, new), target) in original[0]
        .variations
        .iter()
        .zip(&result.variants[0].variations)
        .zip(&components)
    {
        assert_eq!(new.uid, target.uid);
        let mut expected = old.clone();
        expected.uid = target.uid.clone();
        assert_eq!(*new, expected);
    }
    let again = resolve_links(&result.variants, &components).unwrap();
    assert_eq!(again.variants, result.variants);
    assert!(again.affected.is_empty());
    let mut current = original.clone();
    let mut history = History::default();
    history
        .commit(&mut current, result.variants.clone())
        .unwrap();
    history.undo(&mut current);
    assert_eq!(current, original);
    history.redo(&mut current);
    assert_eq!(current, result.variants);
}

#[test]
fn exact_uid_wins_over_a_conflicting_designator_and_is_reserved_before_fallbacks() {
    let mut variant = Variant::new("A".into());
    variant.variations = vec![
        Variation {
            uid: "\\STALE".into(),
            designator: "R2".into(),
            kind: 1,
            ..Default::default()
        },
        Variation {
            uid: "\\EXACT".into(),
            designator: "R1".into(),
            kind: 0,
            ..Default::default()
        },
    ];
    let components = vec![
        ComponentRef {
            uid: "\\OTHER".into(),
            designator: "R1".into(),
        },
        ComponentRef {
            uid: "\\EXACT".into(),
            designator: "R2".into(),
        },
    ];
    let result = resolve_links(&[variant.clone()], &components).unwrap();
    assert_eq!(result.variants[0].variations[1].uid, "\\EXACT");
    assert_eq!(result.variants[0].variations[1].designator, "R2");
    assert_eq!(result.variants[0].variations[0], variant.variations[0]);
    assert_eq!(unresolved(&result.variants[0], &components), vec![0]);
}

#[test]
fn automatic_resolution_does_not_guess_between_duplicate_names_uids_or_claimants() {
    let mut variant = Variant::new("A".into());
    variant.variations.push(Variation {
        uid: "\\OLD".into(),
        designator: "R1".into(),
        kind: 1,
        ..Default::default()
    });
    let duplicate_names = vec![
        ComponentRef {
            uid: "\\LEFT\\R".into(),
            designator: "R1".into(),
        },
        ComponentRef {
            uid: "\\RIGHT\\R".into(),
            designator: "R1".into(),
        },
    ];
    let result = resolve_links(&[variant.clone()], &duplicate_names).unwrap();
    assert_eq!(result.variants[0], variant);
    assert_eq!(result.duplicate_designators, ["R1"]);
    let mut duplicate_uids = duplicate_names.clone();
    duplicate_uids[0].uid = "\\OLD".into();
    duplicate_uids[1].uid = "\\OLD".into();
    let result = resolve_links(&[variant.clone()], &duplicate_uids).unwrap();
    assert_eq!(result.variants[0], variant);
    assert_eq!(result.unresolved, ["R1"]);
    variant.variations.push(Variation {
        uid: "\\SECOND_OLD".into(),
        designator: "R1".into(),
        kind: 0,
        ..Default::default()
    });
    let result = resolve_links(&[variant.clone()], &duplicate_names[..1]).unwrap();
    assert_eq!(result.variants[0], variant);
    assert_eq!(
        unresolved(&result.variants[0], &duplicate_names[..1]),
        vec![0, 1]
    );
    let missing = resolve_links(&[variant.clone()], &[]).unwrap();
    assert_eq!(missing.variants, vec![variant]);
    assert_eq!(missing.unresolved, ["R1"]);
}

#[test]
fn correct_case_insensitive_uids_and_legacy_parameter_rows_do_not_dirty_the_project() {
    let mut variant = Variant::new("A".into());
    variant.variations = vec![
        Variation {
            uid: "left\\uid".into(),
            designator: "r1".into(),
            kind: 1,
            ..Default::default()
        },
        Variation {
            designator: "R2".into(),
            parameters: BTreeMap::from([("Comment".into(), "22k".into())]),
            ..Default::default()
        },
    ];
    let result = resolve_links(
        &[variant.clone()],
        &[
            ComponentRef {
                uid: "\\LEFT\\UID\\".into(),
                designator: "R1".into(),
            },
            ComponentRef {
                uid: "\\R2_UID".into(),
                designator: "R2".into(),
            },
        ],
    )
    .unwrap();
    assert_eq!(result.variants, vec![variant]);
    assert!(result.affected.is_empty());
    assert!(result.unresolved.is_empty());
}
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
    let wire = s.primitives.iter().find(|p| p.component.is_none()).unwrap();
    let base_wire = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find(|p| p.component.is_none())
        .unwrap();
    assert_eq!(wire.kind, base_wire.kind);
    assert_eq!(wire.color, 0xc0c0c0);
    let no_variations = project(&d, None, "", &Default::default()).unwrap();
    assert_eq!(
        no_variations
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| p.component.is_none())
            .unwrap()
            .color,
        0xc0c0c0
    );
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
fn assembly_annotations_keep_sheet_layout_and_compare_against_the_sheet_value() {
    let mut d = fixture();
    let s = d.schematic.as_mut().unwrap();
    let c = s.components[0].clone();
    let mut alternate = edit::extract(s, c.id).unwrap().fresh_copy();
    alternate.components[0]
        .parameters
        .insert("Value".into(), "47k".into());
    for p in &mut alternate.primitives {
        if let SchPrimitiveKind::Text { position, .. } = &mut p.kind {
            *position = Point::mm(100, 100);
            p.presentation.hidden = true;
        }
    }
    let mut v = Variant::new("Alternate".into());
    let change = v.variation_mut(&component_uid(s, &c, ""), "R1");
    change.kind = 2;
    change.alternate = Some(alternate);
    let original = d.clone();
    let assembly = project(&d, Some(&v), "", &Default::default()).unwrap();
    let next = assembly.schematic.as_ref().unwrap();
    let before = snapshot_comment(&d);
    let after = snapshot_comment(&assembly);
    assert_eq!(before.id, after.id);
    assert!(!after.presentation.hidden);
    if let (
        SchPrimitiveKind::Text {
            position: a,
            orientation: oa,
            justification: ja,
            ..
        },
        SchPrimitiveKind::Text {
            position: b,
            orientation: ob,
            justification: jb,
            text,
            font,
        },
    ) = (&before.kind, &after.kind)
    {
        assert_eq!((a, oa, ja), (b, ob, jb));
        assert_eq!(text, "47k");
        assert_eq!(next.fonts[*font].name, "Times New Roman");
        assert_eq!(next.fonts[*font].height_nm, 2_540_000);
        assert!(next.fonts[*font].bold && next.fonts[*font].italic);
    } else {
        panic!()
    }
    assert_eq!(after.color, 0x008000);
    assert_eq!(d, original);
}

fn snapshot_comment(d: &Document) -> &SchPrimitive {
    d.schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find(|p| p.text_role == Some(TextRole::Comment))
        .unwrap()
}

#[test]
fn not_fitted_bounds_exclude_parameter_labels_and_masking_is_automatic() {
    let d = fixture();
    let mut distant = d.clone();
    let s = distant.schematic.as_mut().unwrap();
    let mut label = snapshot_comment(&d).clone();
    label.id = Uuid::new_v4();
    label.text_role = None;
    label.presentation.name = "Package".into();
    label.presentation.raw_text = Some("A very long package name".into());
    if let SchPrimitiveKind::Text { position, text, .. } = &mut label.kind {
        *position = Point::mm(200, 200);
        *text = "A very long package name".into();
    }
    s.primitives.push(label);
    let c = &s.components[0];
    let mut v = Variant::new("DNP".into());
    v.variation_mut(&component_uid(s, c, ""), "R1").kind = 1;
    let style = DisplayStyle {
        dim: false,
        ..Default::default()
    };
    let a = project(&d, Some(&v), "", &style)
        .unwrap()
        .schematic
        .unwrap();
    let b = project(&distant, Some(&v), "", &style)
        .unwrap()
        .schematic
        .unwrap();
    let crosses = |s: Schematic| {
        s.primitives
            .into_iter()
            .filter(|p| p.color == 255)
            .map(|p| p.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(crosses(a), crosses(b.clone()));
    assert!(
        b.primitives
            .iter()
            .filter(|p| !matches!(p.kind, SchPrimitiveKind::Polyline { .. }))
            .all(|p| p.color == 0xc0c0c0)
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
    let mut legacy: DisplayStyle =
        serde_json::from_str(r#"{"cross":true,"dim":false,"color":255,"width_nm":200000}"#)
            .unwrap();
    legacy.upgrade_legacy_defaults();
    assert_eq!(legacy, DisplayStyle::default());
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
