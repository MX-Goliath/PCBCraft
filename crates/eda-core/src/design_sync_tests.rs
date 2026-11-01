use super::*;
#[path = "design_sync_hierarchy_tests.rs"]
mod hierarchy;
use crate::{
    DocumentKind, History,
    schematic::SchPrimitiveKind as K,
    schematic_edit::{self as edit, Placement},
};

fn fixture() -> (Document, Document) {
    let mut sch = Document::new(DocumentKind::Schematic, "Circuit.SchDoc");
    let f = edit::place(Placement::Resistor, Point::mm(20, 20), 0, "R1", "10k", 0).unwrap();
    sch = edit::preview(&sch, &edit::Edit::Insert(f)).unwrap();
    let s = sch.schematic.as_mut().unwrap();
    let c = &mut s.components[0];
    c.settings.models.push(schematic::ComponentModel {
        parameters: Default::default(),
        name: "R0603".into(),
        kind: "PCBLIB".into(),
        current: true,
    });
    let cid = c.id;
    let uid = c.id.to_string();
    let description = c.description.clone();
    let pins: Vec<_> = s
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::Pin { number, .. } = &p.kind {
                Some((number.clone(), edit::electrical_endpoint(p).unwrap()))
            } else {
                None
            }
        })
        .collect();
    for (name, position) in &pins {
        let mut p = edit::primitive(
            K::Text {
                position: *position,
                text: format!("N{name}"),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            None,
        );
        p.text_role = Some(schematic::TextRole::NetLabel);
        s.primitives.push(p);
    }
    let mut board = Document::new(DocumentKind::Board, "Board.PcbDoc");
    let mut b = pcb::Board::default();
    b.components.push(pcb::Component {
        id: Uuid::new_v4(),
        index: 0,
        designator: "R1".into(),
        comment: "10k".into(),
        footprint: "R0603".into(),
        parameters: Some(Default::default()),
        position: Point::mm(30, 30),
        rotation: 0.,
        layer: pcb::TOP,
        locked: false,
        properties: BTreeMap::from([
            ("SOURCEUNIQUEID".into(), uid),
            ("SOURCEDESCRIPTION".into(), description),
        ]),
    });
    for (n, (name, _)) in pins.iter().enumerate() {
        b.nets.push(pcb::Net {
            index: n as u32,
            name: format!("N{name}"),
            properties: Default::default(),
        });
        let mut pad = pcb::Item::new(
            pcb::TOP,
            Some(n as u32),
            pcb::Shape::Pad {
                position: Point::mm(30 + n as i64, 30),
                size: Point::mm(1, 1),
                rotation: 0.,
                shape: pcb::PadShape::Rectangle,
                hole: 0,
                plated: true,
                name: name.clone(),
                bottom_size: None,
                middle_size: None,
            },
        );
        pad.component = Some(0);
        b.items.push(pad);
    }
    assert_eq!(s.components[0].id, cid);
    board.pcb = Some(b);
    (sch, board)
}
fn plan(s: &Document, b: &Document, dir: Direction) -> Plan {
    let board = b.pcb.as_ref().unwrap();
    let footprints = board
        .components
        .iter()
        .filter_map(|c| Footprint::from_board(board, c.id).ok())
        .collect();
    compare(&[s], b, dir, footprints).unwrap()
}
#[test]
fn identical_documents_produce_no_changes() {
    let (s, b) = fixture();
    let p = plan(&s, &b, Direction::ToPcb);
    assert!(p.changes.is_empty(), "{}", p.report());
}

#[test]
fn duplicate_document_identity_is_rejected_before_any_transaction() {
    let (mut s, b) = fixture();
    s.id = b.id;
    assert!(
        compare(&[&s], &b, Direction::ToSchematic, vec![])
            .unwrap_err()
            .contains("duplicate document identities")
    );
}
#[test]
fn stable_link_renames_component_and_supports_undo_without_moving_copper() {
    let (mut s, mut b) = fixture();
    s.schematic.as_mut().unwrap().components[0].designator = "R12".into();
    let before = b.clone();
    let items = b.pcb.as_ref().unwrap().items.clone();
    let mut p = plan(&s, &b, Direction::ToPcb);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 1);
    let mut h = History::default();
    h.commit(&mut b, prepared.documents.into_iter().next().unwrap())
        .unwrap();
    assert_eq!(b.pcb.as_ref().unwrap().components[0].designator, "R12");
    assert_eq!(b.pcb.as_ref().unwrap().items, items);
    assert!(plan(&s, &b, Direction::ToPcb).changes.is_empty());
    h.undo(&mut b);
    assert_eq!(b, before);
}
#[test]
fn native_reverse_policy_enables_only_supported_ordinary_component_properties() {
    let (s, mut b) = fixture();
    let c = &mut b.pcb.as_mut().unwrap().components[0];
    c.comment = "22k".into();
    c.footprint = "R0805".into();
    c.parameters = Some(BTreeMap::from([("Tolerance".into(), "1%".into())]));
    c.properties
        .insert("COMPONENTKINDVERSION2".into(), "5".into());
    c.properties
        .insert("SOURCEDESCRIPTION".into(), "Changed PCB description".into());
    b.pcb.as_mut().unwrap().nets[0].name = "RENAMED".into();
    let mut p = plan(&s, &b, Direction::ToSchematic);
    p.restrict_native_back_annotation();
    assert_eq!(p.changes.iter().filter(|c| c.enabled).count(), 2);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 2);
    let result = prepared.documents[0].schematic.as_ref().unwrap();
    assert_eq!(result.components[0].comment, "22k");
    assert_eq!(footprint(&result.components[0]), "R0805");
    assert_eq!(result.components[0].settings.kind, 0);
    assert_eq!(
        result.components[0].description,
        s.schematic.as_ref().unwrap().components[0].description
    );
    assert!(!result.components[0].parameters.contains_key("Tolerance"));
    assert!(
        p.changes
            .iter()
            .filter(|c| !c.supported())
            .all(|c| !c.enabled && !c.done)
    );
}
#[test]
fn forward_and_reverse_comments_parameters_and_models_use_real_edits() {
    let (s, mut b) = fixture();
    let pcb = b.pcb.as_mut().unwrap();
    pcb.components[0].comment = "22k".into();
    pcb.components[0].parameters = Some(BTreeMap::from([("Tolerance".into(), "1%".into())]));
    pcb.components[0].footprint = "R0805".into();
    let mut p = plan(&s, &b, Direction::ToSchematic);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 3);
    let sch = &prepared.documents[0];
    let c = &sch.schematic.as_ref().unwrap().components[0];
    assert_eq!(c.comment, "22k");
    assert_eq!(c.parameters["Tolerance"], "1%");
    assert_eq!(footprint(c), "R0805");
    assert!(plan(sch, &b, Direction::ToSchematic).changes.is_empty());
}
#[test]
fn renamed_net_preserves_index_tracks_and_round_trip() {
    let (mut s, mut b) = fixture();
    let pcb = b.pcb.as_mut().unwrap();
    pcb.items.push(pcb::Item::new(
        pcb::TOP,
        Some(0),
        pcb::Shape::Track {
            start: Point::mm(30, 30),
            end: Point::mm(40, 30),
            width: 200_000,
        },
    ));
    let sref = s.schematic.as_mut().unwrap();
    let label = sref
        .primitives
        .iter_mut()
        .find(|p| matches!(&p.kind,K::Text{text,..} if text=="N1"))
        .unwrap();
    if let K::Text { text, .. } = &mut label.kind {
        *text = "SUPPLY".into();
    }
    let mut p = plan(&s, &b, Direction::ToPcb);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 1, "{}", p.report());
    let new = &prepared.documents[0];
    assert_eq!(
        new.pcb.as_ref().unwrap().items,
        b.pcb.as_ref().unwrap().items
    );
    assert_eq!(new.pcb.as_ref().unwrap().nets[0].name, "SUPPLY");
    assert!(plan(&s, new, Direction::ToPcb).changes.is_empty());
}
#[test]
fn reverse_net_rename_updates_labels_and_rejects_rewiring() {
    let (mut s, mut b) = fixture();
    // Native labels retain their raw expression. Refresh must not restore it.
    for p in &mut s.schematic.as_mut().unwrap().primitives {
        if p.text_role == Some(schematic::TextRole::NetLabel)
            && let K::Text { text, .. } = &p.kind
        {
            p.presentation.raw_text = Some(text.clone());
        }
    }
    b.pcb.as_mut().unwrap().nets[0].name = "RENAMED".into();
    let mut p = plan(&s, &b, Direction::ToSchematic);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 1, "{}", p.report());
    let next = &prepared.documents[0];
    assert!(plan(next, &b, Direction::ToSchematic).changes.is_empty());
    assert!(
        next.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="RENAMED"))
    );
    b.pcb.as_mut().unwrap().items[0].net = Some(1);
    let p = plan(&s, &b, Direction::ToSchematic);
    assert!(p.changes.iter().any(|c| !c.supported()));
}
#[test]
fn missing_footprint_and_dependent_pins_fail_without_phantom_components() {
    let (s, mut b) = fixture();
    b.pcb.as_mut().unwrap().components.clear();
    b.pcb.as_mut().unwrap().items.clear();
    let mut p = plan(&s, &b, Direction::ToPcb);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert!(prepared.documents.is_empty());
    assert!(p.changes.iter().any(|c| {
        c.check
            .as_ref()
            .is_some_and(|r| r.as_ref().is_err_and(|e| e.contains("Footprint Not Found")))
    }));
    assert!(p.changes.iter().any(|c| {
        c.check
            .as_ref()
            .is_some_and(|r| r.as_ref().is_err_and(|e| e.contains("Unknown Pin")))
    }));
}
#[test]
fn available_footprint_adds_geometry_assigns_pins_and_converges() {
    let (s, mut b) = fixture();
    let original = b.clone();
    let footprints = vec![
        Footprint::from_board(
            b.pcb.as_ref().unwrap(),
            b.pcb.as_ref().unwrap().components[0].id,
        )
        .unwrap(),
    ];
    b.pcb.as_mut().unwrap().components.clear();
    b.pcb.as_mut().unwrap().items.clear();
    let mut p = compare(&[&s], &b, Direction::ToPcb, footprints).unwrap();
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 3, "{}", p.report());
    let next = &prepared.documents[0];
    let new = next.pcb.as_ref().unwrap();
    assert_eq!(new.components.len(), 1);
    assert_eq!(new.items.len(), 2);
    assert!(new.items.iter().all(|i| i.net.is_some()));
    assert_ne!(new.items[0].id, original.pcb.as_ref().unwrap().items[0].id);
    assert!(plan(&s, next, Direction::ToPcb).changes.is_empty());
}
#[test]
fn stale_sources_closed_documents_and_disabled_dependencies_are_detected() {
    let (mut s, mut b) = fixture();
    b.pcb.as_mut().unwrap().components[0].comment = "old".into();
    let mut p = plan(&s, &b, Direction::ToPcb);
    assert!(p.validate(&[&b]).is_err());
    s.name.push('x');
    assert!(p.validate(&[&s, &b]).is_err());
    let (s, mut b) = fixture();
    b.pcb.as_mut().unwrap().nets.clear();
    for pad in &mut b.pcb.as_mut().unwrap().items {
        pad.net = None;
    }
    let mut p = plan(&s, &b, Direction::ToPcb);
    for c in &mut p.changes {
        if matches!(c.operation, Operation::AddNet(_)) {
            c.enabled = false;
        }
    }
    assert!(p.validate(&[&s, &b]).unwrap().documents.is_empty());
    assert!(
        p.changes
            .iter()
            .filter(|c| c.enabled)
            .all(|c| c.check.as_ref().unwrap().is_err())
    );
}
#[test]
fn ambiguous_links_and_duplicate_designators_do_not_guess() {
    let (s, mut b) = fixture();
    let mut c = b.pcb.as_ref().unwrap().components[0].clone();
    c.id = Uuid::new_v4();
    c.index = 1;
    b.pcb.as_mut().unwrap().components.push(c);
    assert!(
        compare(&[&s], &b, Direction::ToPcb, vec![])
            .unwrap_err()
            .contains("Ambiguous")
    );
    let (mut s, b) = fixture();
    let mut c = s.schematic.as_ref().unwrap().components[0].clone();
    c.id = Uuid::new_v4();
    s.schematic.as_mut().unwrap().components.push(c);
    assert!(
        compare(&[&s], &b, Direction::ToPcb, vec![])
            .unwrap_err()
            .contains("Ambiguous")
    );
}
#[test]
fn topology_connects_t_junction_but_not_bare_crossing() {
    let mut s = Document::new(DocumentKind::Schematic, "Wires");
    let a = edit::primitive(
        K::Polyline {
            points: vec![Point::mm(0, 0), Point::mm(10, 0)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    let b = edit::primitive(
        K::Polyline {
            points: vec![Point::mm(5, -5), Point::mm(5, 5)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    assert!(!crate::cross_probe::touches(&a, &b));
    let t = edit::primitive(
        K::Polyline {
            points: vec![Point::mm(5, 0), Point::mm(5, 5)],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    );
    assert!(crate::cross_probe::touches(&a, &t));
    s.schematic.as_mut().unwrap().primitives.extend([a, b, t]);
    assert!(compile(&[&s]).is_ok());
}
#[test]
fn buses_and_hierarchy_do_not_produce_destructive_flat_netlists() {
    let (mut s, b) = fixture();
    s.schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(
            K::Polyline {
                points: vec![Point::mm(0, 0), Point::mm(10, 0)],
                closed: false,
                electrical: "Bus".into(),
            },
            None,
        ));
    let p = plan(&s, &b, Direction::ToPcb);
    assert!(!p.warnings.is_empty());
    assert!(
        p.changes
            .iter()
            .all(|c| !matches!(c.operation, Operation::RemoveNet(_)))
    );
}

#[test]
fn hierarchical_entries_join_child_ports_without_merging_duplicate_local_designators() {
    let mut main = Document::new(DocumentKind::Schematic, "Main.SchDoc");
    let (mut child, _) = fixture();
    child.name = "Child.SchDoc".into();
    let (mut other, _) = fixture();
    other.name = "Other.SchDoc".into();
    let mut children = vec![child, other];
    for (n, child) in children.iter_mut().enumerate() {
        let endpoint = child
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find_map(|p| {
                matches!(&p.kind,K::Pin{number,..} if number=="1")
                    .then(|| edit::electrical_endpoint(p).unwrap())
            })
            .unwrap();
        child
            .schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(edit::primitive(
                K::Port {
                    position: endpoint,
                    width_nm: 5_000_000,
                    height_nm: 2_000_000,
                    style: 0,
                    io_type: 2,
                    name: "SIGNAL".into(),
                    font: 0,
                },
                None,
            ));
        let s = main.schematic.as_mut().unwrap();
        let a = Point::mm(10 + n as i64 * 30, 50);
        let b = Point::mm(30 + n as i64 * 30, 20);
        let mut symbol = edit::primitive(
            K::SheetSymbol {
                a,
                b,
                designator: format!("S{n}"),
                filename: child.name.clone(),
            },
            None,
        );
        symbol.presentation.record_type = 15;
        let id = symbol.id;
        s.primitives.push(symbol);
        let at = Point {
            x: a.x,
            y: a.y - 5_000_000,
        };
        let mut entry = edit::primitive(
            K::SheetEntry {
                position: at,
                side: 0,
                io_type: 2,
                name: "SIGNAL".into(),
                font: 0,
                text_color: 128,
                arrow_kind: "Block & Triangle".into(),
            },
            None,
        );
        entry.presentation.record_type = 16;
        entry.presentation.parent = Some(id);
        s.primitives.push(entry);
        let mut label = edit::primitive(
            K::Text {
                position: at,
                text: format!("PARENT_{n}"),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            None,
        );
        label.text_role = Some(schematic::TextRole::NetLabel);
        s.primitives.push(label);
    }
    let nets = compile(&[&main, &children[0], &children[1]]).unwrap();
    for (n, child) in children.iter().enumerate() {
        let pin = nets
            .pins
            .iter()
            .find(|p| p.sheet == child.id && p.designator == "R1" && p.number == "1")
            .unwrap();
        assert_eq!(pin.net.as_deref(), Some(format!("PARENT_{n}").as_str()));
    }
    let entry = main
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .iter_mut()
        .find(|p| matches!(p.kind, K::SheetEntry { .. }))
        .unwrap();
    if let K::SheetEntry { name, .. } = &mut entry.kind {
        *name = "MISSING".into();
    }
    assert!(
        compile(&[&main, &children[0], &children[1]])
            .unwrap_err()
            .contains("no matching port")
    );
}

#[test]
fn non_electrical_components_are_preserved_and_no_bom_parts_sync() {
    for kind in [1, 2] {
        let (mut s, b) = fixture();
        s.schematic.as_mut().unwrap().components[0].settings.kind = kind;
        let p = plan(&s, &b, Direction::ToPcb);
        assert!(
            !p.changes
                .iter()
                .any(|c| matches!(c.operation, Operation::RemoveComponent(_)))
        );
    }
    let (mut s, mut b) = fixture();
    s.schematic.as_mut().unwrap().components[0].settings.kind = 5;
    b.pcb.as_mut().unwrap().components[0]
        .properties
        .insert("COMPONENTKINDVERSION2".into(), "5".into());
    assert!(plan(&s, &b, Direction::ToPcb).changes.is_empty());
}

#[test]
fn duplicate_local_references_are_rejected_before_pin_union() {
    let (mut doc, _) = fixture();
    let s = doc.schematic.as_mut().unwrap();
    let mut duplicate = s.components[0].clone();
    duplicate.id = Uuid::new_v4();
    duplicate.source_index = u32::MAX;
    s.components.push(duplicate);
    let error = compile(&[&doc]).unwrap_err();
    assert!(error.contains("Duplicate local designator R1"), "{error}");
    let s = doc.schematic.as_mut().unwrap();
    for (n, c) in s.components.iter_mut().enumerate() {
        c.settings.part_count = 2;
        c.part = n as u32 + 1;
    }
    assert!(compile(&[&doc]).is_ok());
}

#[test]
fn partial_execution_can_resume_without_reapplying_done_rows() {
    let (s, mut b) = fixture();
    b.pcb.as_mut().unwrap().components[0].comment = "outdated".into();
    b.pcb.as_mut().unwrap().items[0].net = None;
    let mut p = plan(&s, &b, Direction::ToPcb);
    let skipped = p
        .changes
        .iter()
        .position(|c| matches!(c.operation, Operation::PadNet { .. }))
        .unwrap();
    p.changes[skipped].enabled = false;
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows.len(), 1);
    b = prepared.documents.into_iter().next().unwrap();
    p.acknowledge(&prepared.rows, &[&s, &b]);
    p.changes[skipped].enabled = true;
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(prepared.rows, vec![skipped]);
    b = prepared.documents.into_iter().next().unwrap();
    p.acknowledge(&prepared.rows, &[&s, &b]);
    assert!(p.validate(&[&s, &b]).unwrap().rows.is_empty());
    assert!(plan(&s, &b, Direction::ToPcb).changes.is_empty());
}

#[test]
fn unsynchronized_parameters_are_preserved_in_both_directions() {
    let (mut s, mut b) = fixture();
    let id = s.schematic.as_ref().unwrap().components[0].id;
    s = edit::preview(
        &s,
        &edit::Edit::ParameterValue {
            component: Some(id),
            name: "Private".into(),
            value: Some("schematic-only".into()),
        },
    )
    .unwrap();
    let scene = s.schematic.as_mut().unwrap();
    let mut p = edit::primitive(
        K::Text {
            position: Point::default(),
            text: "schematic-only".into(),
            font: 0,
            orientation: 0,
            justification: 0,
        },
        Some(id),
    );
    p.presentation.name = "Private".into();
    p.presentation.record_type = 41;
    p.presentation.allow_sync = false;
    p.presentation.hidden = true;
    p.presentation.raw_text = Some("schematic-only".into());
    scene.parameter_objects.push(p);
    b.pcb.as_mut().unwrap().components[0].parameters =
        Some(BTreeMap::from([("Private".into(), "pcb-only".into())]));
    assert!(plan(&s, &b, Direction::ToPcb).changes.is_empty());
    assert!(plan(&s, &b, Direction::ToSchematic).changes.is_empty());
    b.pcb.as_mut().unwrap().components[0]
        .parameters
        .as_mut()
        .unwrap()
        .insert("Other".into(), "new".into());
    let mut p = plan(&s, &b, Direction::ToSchematic);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert_eq!(
        prepared.documents[0].schematic.as_ref().unwrap().components[0].parameters["Private"],
        "schematic-only"
    );
}

#[test]
fn identical_local_names_on_different_sheets_do_not_merge_accidentally() {
    let (s, _) = fixture();
    let (mut other, _) = fixture();
    other.schematic.as_mut().unwrap().components[0].designator = "R2".into();
    assert!(compile(&[&s, &other]).unwrap_err().contains("sheet-local"));
    for doc in [&mut other] {
        for p in &mut doc.schematic.as_mut().unwrap().primitives {
            if let K::Text { text, .. } = &p.kind
                && p.text_role == Some(schematic::TextRole::NetLabel)
            {
                let position = edit::electrical_endpoint(p).unwrap();
                p.kind = K::PowerPort {
                    cross_sheet: false,
                    position,
                    orientation: 0,
                    style: 0,
                    text: text.clone(),
                    show_name: true,
                    font: 0,
                };
            }
        }
    }
    let mut first = s.clone();
    for p in &mut first.schematic.as_mut().unwrap().primitives {
        if let K::Text { text, .. } = &p.kind
            && p.text_role == Some(schematic::TextRole::NetLabel)
        {
            let position = edit::electrical_endpoint(p).unwrap();
            p.kind = K::PowerPort {
                cross_sheet: false,
                position,
                orientation: 0,
                style: 0,
                text: text.clone(),
                show_name: true,
                font: 0,
            };
        }
    }
    let nets = compile(&[&first, &other]).unwrap();
    assert_eq!(nets.names.len(), 2);
    assert_eq!(nets.pins.len(), 4);
}

#[test]
fn repeated_pin_numbers_and_multi_part_components_fold_to_one_physical_part() {
    let (mut s, b) = fixture();
    let scene = s.schematic.as_mut().unwrap();
    let mut part = scene.components[0].clone();
    scene.components[0].settings.part_count = 2;
    part.settings.part_count = 2;
    part.part = 2;
    part.id = Uuid::new_v4();
    let owner = scene.components[0].id;
    let mut shared = scene
        .primitives
        .iter()
        .find(|p| p.component == Some(owner) && matches!(p.kind, K::Pin { .. }))
        .unwrap()
        .clone();
    shared.id = Uuid::new_v4();
    shared.component = Some(part.id);
    if let K::Pin { position, .. } = &mut shared.kind {
        position.x += 50_000_000;
    }
    scene.components.push(part);
    scene.primitives.push(shared);
    let p = plan(&s, &b, Direction::ToPcb);
    assert!(p.changes.is_empty(), "{}", p.report());
}

#[test]
fn missing_required_pad_is_reported_after_a_footprint_change() {
    let (mut s, b) = fixture();
    s.schematic.as_mut().unwrap().components[0].settings.models[0].name = "ONE_PAD".into();
    let mut template = b.pcb.as_ref().unwrap().clone();
    template.components[0].footprint = "ONE_PAD".into();
    template.items.remove(0);
    let f = Footprint::from_board(&template, template.components[0].id).unwrap();
    let mut p = compare(&[&s], &b, Direction::ToPcb, vec![f]).unwrap();
    let _ = p.validate(&[&s, &b]).unwrap();
    assert!(p.changes.iter().any(|c| {
        c.check
            .as_ref()
            .is_some_and(|r| r.as_ref().is_err_and(|e| e.contains("Unknown Pin")))
    }));
}

#[test]
fn locked_component_and_stale_undo_state_cannot_be_applied_silently() {
    let (s, mut b) = fixture();
    b.pcb.as_mut().unwrap().components[0].locked = true;
    b.pcb.as_mut().unwrap().components[0].comment = "outdated".into();
    let mut p = plan(&s, &b, Direction::ToPcb);
    let prepared = p.validate(&[&s, &b]).unwrap();
    assert!(prepared.documents.is_empty());
    assert!(p.report().contains("locked"));
}

#[test]
fn placement_of_a_new_component_does_not_depend_on_existing_schematic_count() {
    let (mut s, mut b) = fixture();
    let symbol = s.schematic.as_ref().unwrap().components[0].clone();
    let component = b.pcb.as_ref().unwrap().components[0].clone();
    for n in 2..=50 {
        let mut c = symbol.clone();
        c.id = Uuid::new_v4();
        c.designator = format!("R{n}");
        let mut p = component.clone();
        p.id = Uuid::new_v4();
        p.index = n;
        p.designator = c.designator.clone();
        p.properties
            .insert("SOURCEUNIQUEID".into(), c.id.to_string());
        s.schematic.as_mut().unwrap().components.push(c);
        b.pcb.as_mut().unwrap().components.push(p);
    }
    let mut added = symbol;
    added.id = Uuid::new_v4();
    added.designator = "R51".into();
    s.schematic.as_mut().unwrap().components.push(added);
    let bounds = b.pcb.as_ref().unwrap().bounds();
    let mut p = plan(&s, &b, Direction::ToPcb);
    let prepared = p.validate(&[&s, &b]).unwrap();
    let next = prepared.documents[0].pcb.as_ref().unwrap();
    let added = next
        .components
        .iter()
        .find(|c| c.designator == "R51")
        .unwrap();
    assert!(added.position.x < bounds.max.x + 10_000_000);
    assert!(added.position.y < bounds.min.y + 10_000_000);
    assert!(
        next.items
            .iter()
            .filter(|i| i.component == Some(added.index))
            .all(|i| i.bounds().min.x > bounds.max.x)
    );
}
