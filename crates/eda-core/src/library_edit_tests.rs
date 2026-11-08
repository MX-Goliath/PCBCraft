use super::*;

#[test]
fn removing_middle_part_and_mode_keeps_other_symbols_and_is_one_undo_step() {
    let mut doc = new(DocumentKind::Schematic, "Parts.SchLib");
    for command in [
        Edit::AddPart,
        Edit::AddPart,
        Edit::AddDisplayMode,
        Edit::AddDisplayMode,
        Edit::RenameDisplayMode("Service".into()),
        Edit::Add("Unrelated".into()),
    ] {
        doc = preview(&doc, &command).unwrap();
    }
    let unrelated = doc.schematic.clone();
    let select_middle = members(&doc)
        .position(|m| name(m) == "Component_1" && variant(m) == (2, 1))
        .unwrap();
    doc = preview(&doc, &Edit::Select(select_middle)).unwrap();
    let before = doc.clone();
    let mut history = History::default();
    history
        .library_edit(&mut doc, 0, &Edit::RemovePart)
        .unwrap();
    assert_eq!(
        members(&doc).filter(|m| name(m) == "Component_1").count(),
        6
    );
    for m in members(&doc).filter(|m| name(m) == "Component_1") {
        assert_eq!(
            m.schematic.as_ref().unwrap().components[0]
                .settings
                .part_count,
            2
        );
        assert!((1..=2).contains(&variant(m).0));
    }
    assert_eq!(variant(&doc).1, 1);
    history
        .library_edit(&mut doc, 1, &Edit::RemoveDisplayMode)
        .unwrap();
    assert_eq!(
        members(&doc).filter(|m| name(m) == "Component_1").count(),
        4
    );
    assert_eq!(display_mode_name(&doc, 1), "Service");
    assert_eq!(
        members(&doc)
            .find(|m| name(m) == "Unrelated")
            .unwrap()
            .schematic,
        unrelated
    );
    history.undo(&mut doc);
    history.undo(&mut doc);
    assert_eq!(doc, before);
    history.redo(&mut doc);
    history.redo(&mut doc);
    doc.validate().unwrap();
}

#[test]
fn normal_mode_last_part_and_conflicting_mode_names_are_rejected_atomically() {
    let doc = new(DocumentKind::Schematic, "Parts.SchLib");
    for command in [
        Edit::RemovePart,
        Edit::RemoveDisplayMode,
        Edit::RenameDisplayMode("X".into()),
    ] {
        assert!(preview(&doc, &command).is_err());
    }
    let doc = preview(&doc, &Edit::AddDisplayMode).unwrap();
    assert!(preview(&doc, &Edit::RenameDisplayMode("normal".into())).is_err());
    let doc = preview(&doc, &Edit::RenameDisplayMode("Service".into())).unwrap();
    let doc = preview(&doc, &Edit::AddDisplayMode).unwrap();
    assert!(preview(&doc, &Edit::RenameDisplayMode("service".into())).is_err());
    let doc = preview(&doc, &Edit::ComponentKind(6)).unwrap();
    assert!(members(&doc).all(|m| m.schematic.as_ref().unwrap().components[0].settings.kind == 6));
    assert!(preview(&doc, &Edit::ComponentKind(7)).is_err());
}

#[test]
fn footprint_metadata_is_checked_and_undo_keeps_geometry_and_parameters() {
    let mut doc = new(DocumentKind::Board, "Parts.PcbLib");
    let before = doc.clone();
    let mut history = History::default();
    let command = Edit::FootprintMetadata {
        name: "LAND".into(),
        description: "Two terminals".into(),
        height_nm: 1_200_000,
        parameters: vec![("Package".into(), "SMD".into())],
    };
    history.library_edit(&mut doc, 0, &command).unwrap();
    let c = &doc.pcb.as_ref().unwrap().components[0];
    assert_eq!(c.footprint, "LAND");
    assert_eq!(c.parameters.as_ref().unwrap()["Package"], "SMD");
    assert_eq!(c.properties["DESCRIPTION"], "Two terminals");
    let mut invalid = command;
    if let Edit::FootprintMetadata { height_nm, .. } = &mut invalid {
        *height_nm = -1;
    }
    let saved = doc.clone();
    assert!(history.library_edit(&mut doc, 1, &invalid).is_err());
    assert_eq!(doc, saved);
    history.undo(&mut doc);
    assert_eq!(doc, before);
}

#[test]
fn library_members_variants_and_history_are_atomic() {
    let mut doc = new(DocumentKind::Schematic, "Parts.SchLib");
    let mut history = History::default();
    let original = doc.clone();
    history
        .library_edit(&mut doc, 0, &Edit::Add("Other".into()))
        .unwrap();
    assert_eq!(name(&doc), "Other");
    let before = doc.clone();
    assert!(
        history
            .library_edit(&mut doc, 1, &Edit::Rename("Component_1".into()))
            .is_err()
    );
    assert_eq!(doc, before);
    assert!(
        history
            .library_edit(&mut doc, 0, &Edit::Rename("Stale".into()))
            .is_err()
    );
    history.undo(&mut doc);
    assert_eq!(doc, original);
    history.redo(&mut doc);
    assert_eq!(doc, before);
    doc = preview(&doc, &Edit::AddPart).unwrap();
    assert_eq!(variant(&doc), (2, 0));
    doc = preview(&doc, &Edit::AddDisplayMode).unwrap();
    assert_eq!(members(&doc).filter(|m| name(m) == "Other").count(), 4);
    doc = preview(&doc, &Edit::Duplicate("Copy".into())).unwrap();
    assert_eq!(members(&doc).filter(|m| name(m) == "Copy").count(), 4);
    doc = preview(&doc, &Edit::Remove).unwrap();
    assert!(!members(&doc).any(|m| name(m) == "Copy"));
    doc.validate().unwrap();
}

#[test]
fn member_navigation_does_not_change_saved_contents_or_add_an_undo_step() {
    let mut doc = new(DocumentKind::Schematic, "Parts.SchLib");
    doc = preview(&doc, &Edit::Add("Other".into())).unwrap();
    let saved = doc.clone();
    let mut history = History::default();
    history.select_library_member(&mut doc, 1).unwrap();
    assert!(same_contents(&saved, &doc));
    assert!(!history.can_undo());
    assert_ne!(name(&saved), name(&doc));
    let moved = preview(&doc, &Edit::Rename("Changed".into())).unwrap();
    assert!(!same_contents(&saved, &moved));
}
#[test]
fn library_graphics_have_one_owner_and_invalid_pin_mapping_is_rejected() {
    let mut doc = new(DocumentKind::Schematic, "Parts.SchLib");
    let id = doc.schematic.as_ref().unwrap().components[0].id;
    let graphic = edit::primitive(
        sch::SchPrimitiveKind::Rectangle {
            a: Point::mm(-2, -2),
            b: Point::mm(2, 2),
            radius: Point::default(),
        },
        None,
    );
    doc = edit::preview(
        &doc,
        &edit::Edit::Insert(edit::Fragment {
            primitives: vec![graphic],
            ..Default::default()
        }),
    )
    .unwrap();
    assert!(
        doc.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .all(|p| p.component == Some(id))
    );
    assert!(
        preview(
            &doc,
            &Edit::LinkFootprint {
                name: "SOT23".into(),
                library: "parts.PcbLib".into(),
                map: vec![("missing".into(), vec!["1".into()])]
            }
        )
        .is_err()
    );
    let nested = doc.clone();
    doc.library.as_mut().unwrap().others.push(nested);
    assert!(doc.validate().unwrap_err().contains("Nested"));
}
