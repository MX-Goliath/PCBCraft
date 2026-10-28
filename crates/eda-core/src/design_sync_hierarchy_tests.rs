use super::*;

#[test]
fn export_omission_cannot_silently_disconnect_existing_single_pin_nets_in_project_review() {
    let (sheet, board) = fixture();
    let options = ProjectOptions {
        single_pin_nets: false,
        review_annotation: true,
        ..Default::default()
    };
    let mut plan = compare_project(&[&sheet], &board, Direction::ToPcb, vec![], &options).unwrap();
    assert!(!plan.changes.is_empty());
    assert!(
        plan.changes
            .iter()
            .all(|row| !row.enabled && !row.supported())
    );
    let prepared = plan.validate(&[&sheet, &board]).unwrap();
    assert!(prepared.documents.is_empty());
    assert!(plan.warnings.iter().any(|w| w.contains("single-pin")));
}

#[test]
fn an_unconnected_schematic_pin_without_a_footprint_pad_needs_no_pad_net_operation() {
    let (mut sheet, mut board) = fixture();
    sheet
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .retain(|p| !matches!(&p.kind, K::Text { text, .. } if text == "N1"));
    board
        .pcb
        .as_mut()
        .unwrap()
        .items
        .retain(|p| !matches!(&p.shape, pcb::Shape::Pad { name, .. } if name == "1"));
    let mut plan = super::plan(&sheet, &board, Direction::ToPcb);
    assert!(
        !plan
            .changes
            .iter()
            .any(|row| matches!(&row.operation, Operation::PadNet { pin, .. } if pin == "1"))
    );
    plan.validate(&[&sheet, &board]).unwrap();
    assert!(
        plan.changes
            .iter()
            .all(|row| row.check.as_ref().is_none_or(Result::is_ok))
    );
}

#[test]
fn power_wired_to_a_port_is_localized_before_global_power_union() {
    let (mut a, _) = fixture();
    let (mut b, _) = fixture();
    for (n, doc) in [&mut a, &mut b].into_iter().enumerate() {
        let s = doc.schematic.as_mut().unwrap();
        let pin = s
            .primitives
            .iter()
            .find(|p| matches!(&p.kind, K::Pin { number, .. } if number == "1"))
            .unwrap();
        let at = edit::electrical_endpoint(pin).unwrap();
        s.primitives
            .retain(|p| p.text_role != Some(schematic::TextRole::NetLabel));
        s.primitives.push(edit::primitive(
            K::PowerPort {
                position: at,
                orientation: 0,
                style: 0,
                text: "VCC".into(),
                show_name: true,
                font: 0,
            },
            None,
        ));
        if n == 0 {
            s.primitives.push(edit::primitive(
                K::Port {
                    position: at,
                    width_nm: 1_000_000,
                    height_nm: 1_000_000,
                    style: 0,
                    io_type: 0,
                    name: "POWER".into(),
                    font: 0,
                },
                None,
            ));
            let mut label = edit::primitive(
                K::Text {
                    position: at,
                    text: "LOCAL".into(),
                    font: 0,
                    orientation: 0,
                    justification: 0,
                },
                None,
            );
            label.text_role = Some(schematic::TextRole::NetLabel);
            s.primitives.push(label);
        }
    }
    let mut options = ProjectOptions {
        scope: NetScope::Hierarchical,
        power_names_priority: false,
        hierarchical_net_names: false,
        ..Default::default()
    };
    let nets = compile_project(&[&a, &b], &options).unwrap();
    assert_eq!(
        nets.pins
            .iter()
            .find(|p| p.sheet == a.id && p.number == "1")
            .unwrap()
            .net
            .as_deref(),
        Some("LOCAL")
    );
    assert_eq!(
        nets.pins
            .iter()
            .find(|p| p.sheet == b.id && p.number == "1")
            .unwrap()
            .net
            .as_deref(),
        Some("VCC")
    );
    options.scope = NetScope::Flat;
    let nets = compile_project(&[&a, &b], &options).unwrap();
    assert!(
        nets.pins
            .iter()
            .filter(|p| p.number == "1")
            .all(|p| p.net.as_deref() == Some("LOCAL"))
    );
}

#[test]
fn naming_uses_lowercase_order_and_single_pin_setting_applies_to_named_pins() {
    let (mut doc, _) = fixture();
    let s = doc.schematic.as_mut().unwrap();
    let label = s
        .primitives
        .iter_mut()
        .find(|p| p.text_role == Some(schematic::TextRole::NetLabel))
        .unwrap();
    let mut alias = label.clone();
    alias.id = Uuid::new_v4();
    let K::Text { text, .. } = &mut label.kind else {
        unreachable!()
    };
    *text = "TR_WK".into();
    let K::Text { text, .. } = &mut alias.kind else {
        unreachable!()
    };
    *text = "TRANSP_WK".into();
    s.primitives.push(alias);
    let options = ProjectOptions {
        scope: NetScope::Hierarchical,
        ..Default::default()
    };
    let nets = compile_project(&[&doc], &options).unwrap();
    assert!(nets.names.iter().any(|n| n == "TR_WK"));
    let options = ProjectOptions {
        single_pin_nets: false,
        ..options
    };
    let nets = compile_project(&[&doc], &options).unwrap();
    assert!(nets.names.is_empty());
    assert!(nets.pins.iter().all(|p| p.net.is_none()));
}

#[test]
fn a_component_touching_compile_mask_boundary_is_retained() {
    let (mut doc, _) = fixture();
    let component = &doc.schematic.as_ref().unwrap().components[0];
    let x = component.position.x;
    doc.schematic
        .as_mut()
        .unwrap()
        .compile_masks
        .push(schematic::CompileMask {
            a: Point { x, y: 0 },
            b: Point::mm(40, 40),
            collapsed: false,
        });
    assert!(
        crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap())
            .components
            .is_empty()
    );
}

#[test]
fn pcb_legacy_component_kind_is_used_when_modern_field_is_absent() {
    let (mut sheet, mut board) = fixture();
    sheet.schematic.as_mut().unwrap().components[0]
        .settings
        .kind = 3;
    board.pcb.as_mut().unwrap().components[0]
        .properties
        .insert("COMPONENTKIND".into(), "3".into());
    assert!(
        super::plan(&sheet, &board, Direction::ToPcb)
            .changes
            .is_empty()
    );
}

#[test]
fn native_precision_connects_roundoff_without_moving_document_or_bridging_larger_gaps() {
    let (mut sheet, _) = fixture();
    let s = sheet.schematic.as_mut().unwrap();
    let pin = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::Pin { .. }))
        .unwrap();
    let id = pin.id;
    let at = edit::electrical_endpoint(pin).unwrap();
    let label = s
        .primitives
        .iter_mut()
        .find(|p| {
            p.text_role == Some(schematic::TextRole::NetLabel)
                && edit::electrical_endpoint(p) == Some(at)
        })
        .unwrap();
    let K::Text { position, .. } = &mut label.kind else {
        unreachable!()
    };
    position.x += 10;
    let original = sheet.clone();
    let options = ProjectOptions {
        connection_precision_nm: Some(1_000),
        ..Default::default()
    };
    assert!(
        compile(&[&sheet])
            .unwrap()
            .pins
            .iter()
            .any(|p| p.net.is_none())
    );
    let compiled = compile_project(&[&sheet], &options).unwrap();
    assert!(compiled.pins.iter().all(|p| p.net.is_some()));
    assert_eq!(sheet, original);
    let s = sheet.schematic.as_mut().unwrap();
    let label = s.primitives.iter_mut().find(|p| {
        p.text_role == Some(schematic::TextRole::NetLabel)
            && matches!(&p.kind, K::Text { position, .. } if position.x == at.x + 10 && position.y == at.y)
    }).unwrap();
    let K::Text { position, .. } = &mut label.kind else {
        unreachable!()
    };
    position.x = at.x + 2_000;
    let number = s
        .primitives
        .iter()
        .find_map(|p| {
            (p.id == id).then(|| match &p.kind {
                K::Pin { number, .. } => number.clone(),
                _ => unreachable!(),
            })
        })
        .unwrap();
    let compiled = compile_project(&[&sheet], &options).unwrap();
    assert!(
        compiled
            .pins
            .iter()
            .any(|p| p.number == number && p.net.is_none())
    );
}

#[test]
fn pcb_layer_metadata_is_preserved_and_never_back_annotated_as_a_schematic_parameter() {
    let (mut sheet, mut board) = fixture();
    let c = &mut board.pcb.as_mut().unwrap().components[0];
    c.parameters
        .as_mut()
        .unwrap()
        .insert("Layer".into(), "TOP".into());
    assert!(
        super::plan(&sheet, &board, Direction::ToPcb)
            .changes
            .is_empty()
    );
    sheet.schematic.as_mut().unwrap().components[0]
        .parameters
        .insert("Value".into(), "22k".into());
    let mut forward = super::plan(&sheet, &board, Direction::ToPcb);
    let prepared = forward.validate(&[&sheet, &board]).unwrap();
    let params = prepared.documents[0].pcb.as_ref().unwrap().components[0]
        .parameters
        .as_ref()
        .unwrap();
    assert_eq!(params.get("Layer").unwrap(), "TOP");
    assert_eq!(params.get("Value").unwrap(), "22k");
    let mut reverse = super::plan(&sheet, &board, Direction::ToSchematic);
    let prepared = reverse.validate(&[&sheet, &board]).unwrap();
    assert!(
        !prepared.documents[0].schematic.as_ref().unwrap().components[0]
            .parameters
            .contains_key("Layer")
    );
}

#[test]
fn expanded_compile_masks_remove_complete_components_and_wiring_but_collapsed_masks_do_not() {
    let (mut doc, board) = fixture();
    let base = doc.clone();
    doc.schematic
        .as_mut()
        .unwrap()
        .compile_masks
        .push(schematic::CompileMask {
            a: Point::mm(0, 0),
            b: Point::mm(40, 40),
            collapsed: false,
        });
    let excluded = crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap());
    assert!(
        excluded
            .components
            .contains(&doc.schematic.as_ref().unwrap().components[0].id)
    );
    assert!(compile(&[&doc]).unwrap().pins.is_empty());
    let plan = super::plan(&doc, &board, Direction::ToPcb);
    assert!(
        plan.changes
            .iter()
            .any(|c| matches!(c.operation, Operation::RemoveComponent(_)))
    );
    doc.schematic.as_mut().unwrap().compile_masks[0].collapsed = true;
    assert_eq!(
        compile(&[&doc]).unwrap().names,
        compile(&[&base]).unwrap().names
    );
    doc.schematic.as_mut().unwrap().compile_masks[0].collapsed = false;
    doc.schematic.as_mut().unwrap().compile_masks[0].b.x = 20_000_000;
    assert!(
        crate::schematic_compile::exclusions(doc.schematic.as_ref().unwrap())
            .components
            .is_empty(),
        "A mask covering only part of the symbol must not exclude it"
    );
}

fn project_fixture() -> (Vec<Document>, Document, ProjectOptions) {
    let mut main = Document::new(DocumentKind::Schematic, "Main.SchDoc");
    let (a, mut board) = fixture();
    let (mut b, other) = fixture();
    b.name = a.name.clone(); // Equal basenames in separate project folders.
    for p in &mut b.schematic.as_mut().unwrap().primitives {
        if p.text_role == Some(schematic::TextRole::NetLabel)
            && let K::Text { text, .. } = &mut p.kind
        {
            *text = text.replace('N', "B");
        }
    }
    let mut options = ProjectOptions {
        review_annotation: true,
        scope: NetScope::Hierarchical,
        ..Default::default()
    };
    let mut links = vec![];
    for (n, child) in [&a, &b].into_iter().enumerate() {
        let symbol = edit::primitive(
            K::SheetSymbol {
                a: Point::mm(n as i64 * 30, 30),
                b: Point::mm(n as i64 * 30 + 20, 0),
                designator: format!("S{n}"),
                filename: format!("{n}/Circuit.SchDoc"),
            },
            None,
        );
        links.push(SheetLink {
            parent: main.id,
            symbol: symbol.id,
            child: child.id,
        });
        let target = if n == 0 {
            &mut board.pcb.as_mut().unwrap().components[0]
        } else {
            let mut c = other.pcb.as_ref().unwrap().components[0].clone();
            c.index = 1;
            c.designator = "R2".into();
            board.pcb.as_mut().unwrap().components.push(c);
            &mut board.pcb.as_mut().unwrap().components[1]
        };
        target.properties.insert(
            "SOURCEUNIQUEID".into(),
            format!(
                "\\{}\\{}",
                symbol.id,
                child.schematic.as_ref().unwrap().components[0].id
            ),
        );
        main.schematic.as_mut().unwrap().primitives.push(symbol);
    }
    for mut net in other.pcb.as_ref().unwrap().nets.clone() {
        net.index += 2;
        net.name = net.name.replace('N', "B");
        board.pcb.as_mut().unwrap().nets.push(net);
    }
    for mut p in other.pcb.as_ref().unwrap().items.clone() {
        p.component = Some(1);
        p.net = p.net.map(|n| n + 2);
        board.pcb.as_mut().unwrap().items.push(p);
    }
    options.links = Some(links);
    (vec![main, a, b], board, options)
}

#[test]
fn hierarchical_eco_uses_sheet_paths_and_component_ids_in_both_directions() {
    let (mut sheets, board, options) = project_fixture();
    sheets[2].schematic.as_mut().unwrap().components[0].comment = "22k".into();
    let refs: Vec<_> = sheets.iter().collect();
    let mut plan = compare_project(&refs, &board, Direction::ToPcb, vec![], &options).unwrap();
    assert!(
        plan.warnings
            .iter()
            .any(|w| w.contains("Duplicate schematic designator"))
    );
    assert!(!plan.changes.iter().any(|c| matches!(
        c.operation,
        Operation::RemoveComponent(_) | Operation::PadNet { .. } | Operation::RenameNet(..)
    )));
    let mut all = refs;
    all.push(&board);
    let prepared = plan.validate(&all).unwrap();
    let changed = prepared
        .documents
        .iter()
        .find(|d| d.id == board.id)
        .unwrap();
    assert_eq!(changed.pcb.as_ref().unwrap().components[0].comment, "10k");
    assert_eq!(changed.pcb.as_ref().unwrap().components[1].comment, "22k");
    assert_eq!(changed.pcb.as_ref().unwrap().components[1].designator, "R2");
    let mut board = board;
    board.pcb.as_mut().unwrap().components[1].comment = "33k".into();
    let refs: Vec<_> = sheets.iter().collect();
    let mut reverse =
        compare_project(&refs, &board, Direction::ToSchematic, vec![], &options).unwrap();
    reverse.restrict_native_back_annotation();
    let mut all = refs;
    all.push(&board);
    let prepared = reverse.validate(&all).unwrap();
    assert_eq!(prepared.documents.len(), 1);
    assert_eq!(prepared.documents[0].id, sheets[2].id);
    assert_eq!(
        prepared.documents[0].schematic.as_ref().unwrap().components[0].designator,
        "R2"
    );
    assert_eq!(
        prepared.documents[0].schematic.as_ref().unwrap().components[0].comment,
        "33k"
    );
    let original = sheets[2].clone();
    let mut history = History::default();
    history
        .commit(&mut sheets[2], prepared.documents[0].clone())
        .unwrap();
    history.undo(&mut sheets[2]);
    assert_eq!(sheets[2], original);
}

#[test]
fn incomplete_annotation_keeps_ambiguous_pin_nets_and_all_existing_copper() {
    let (mut sheet, board) = fixture();
    let s = sheet.schematic.as_mut().unwrap();
    s.components[0].designator = "R?".into();
    s.components[0].comment = "changed".into();
    s.primitives
        .retain(|p| p.text_role != Some(schematic::TextRole::NetLabel));
    let options = ProjectOptions {
        review_annotation: true,
        ..Default::default()
    };
    let mut plan = compare_project(&[&sheet], &board, Direction::ToPcb, vec![], &options).unwrap();
    assert!(!plan.changes.iter().any(|c| matches!(
        c.operation,
        Operation::RemoveNet(_) | Operation::PadNet { .. } | Operation::RemoveComponent(_)
    )));
    assert!(plan.changes.iter().any(|c| c.group == "Annotation"));
    let prepared = plan.validate(&[&sheet, &board]).unwrap();
    let changed = &prepared.documents[0];
    assert_eq!(
        changed.pcb.as_ref().unwrap().nets,
        board.pcb.as_ref().unwrap().nets
    );
    assert_eq!(
        changed.pcb.as_ref().unwrap().items,
        board.pcb.as_ref().unwrap().items
    );
    assert_eq!(
        changed.pcb.as_ref().unwrap().components[0].comment,
        "changed"
    );
}

#[test]
fn net_scope_and_naming_preferences_are_separate_from_physical_connectivity() {
    let (mut docs, _, mut options) = project_fixture();
    for doc in &mut docs[1..] {
        let s = doc.schematic.as_mut().unwrap();
        s.primitives
            .retain(|p| p.text_role != Some(schematic::TextRole::NetLabel));
        let at = s
            .primitives
            .iter()
            .find_map(|p| {
                matches!(&p.kind,K::Pin{number,..}if number=="1")
                    .then(|| edit::electrical_endpoint(p).unwrap())
            })
            .unwrap();
        s.primitives.push(edit::primitive(
            K::Port {
                position: at,
                width_nm: 5_000_000,
                height_nm: 2_000_000,
                style: 0,
                io_type: 2,
                name: "SIGNAL".into(),
                font: 0,
            },
            None,
        ));
    }
    options.scope = NetScope::Flat;
    let refs: Vec<_> = docs.iter().collect();
    let nets = compile_project(&refs, &options).unwrap();
    let pins: Vec<_> = nets.pins.iter().filter(|p| p.number == "1").collect();
    assert_eq!(pins.len(), 2);
    assert!(pins.iter().all(|p| p.net.as_deref() == Some("SIGNAL")));
    options.allow_port_names = false;
    let unnamed = compile_project(&refs, &options).unwrap();
    assert!(!unnamed.names.iter().any(|n| n == "SIGNAL"));
    options.scope = NetScope::Hierarchical;
    options.allow_port_names = true;
    let scoped = compile_project(&refs, &options).unwrap();
    let pins: Vec<_> = scoped.pins.iter().filter(|p| p.number == "1").collect();
    assert_ne!(pins[0].net, pins[1].net);
}

#[test]
fn staged_metadata_matches_reference_operations_and_locked_rows_leave_no_partial_edit() {
    let (sheet, mut board) = fixture();
    board.pcb.as_mut().unwrap().components[0].comment = "old".into();
    let mut plan = plan(&sheet, &board, Direction::ToPcb);
    let mut reference = vec![board.clone(), sheet.clone()];
    for row in &plan.changes {
        if row.enabled {
            apply(&row.operation, board.id, &plan.footprints, &mut reference).unwrap();
        }
    }
    let prepared = plan.validate(&[&sheet, &board]).unwrap();
    assert_eq!(
        prepared
            .documents
            .iter()
            .find(|d| d.id == board.id)
            .unwrap(),
        &reference[0]
    );
    board.pcb.as_mut().unwrap().components[0].locked = true;
    let mut plan = super::plan(&sheet, &board, Direction::ToPcb);
    let result = plan.validate(&[&sheet, &board]).unwrap();
    assert!(result.documents.is_empty());
    assert!(
        plan.changes
            .iter()
            .any(|c| c.check.as_ref().is_some_and(Result::is_err))
    );
}
