use super::*;
use crate::{
    Document, DocumentKind, History,
    schematic::{Component, SchPrimitiveKind as K},
    schematic_edit::{Edit, Request},
};
fn point(x: i64, y: i64) -> Point {
    Point {
        x: x * 2_540_000,
        y: y * 2_540_000,
    }
}
fn scene() -> Document {
    Document::new(DocumentKind::Schematic, "Harness.SchDoc")
}
fn add_connector(d: &mut Document, x: i64, side: u32) -> Uuid {
    let s = d.schematic.as_mut().unwrap();
    let mut f = connector(point(x, 20), point(x + 5, 0), 0, "Serial").unwrap();
    let id = f.primitives[0].id;
    if let K::HarnessConnector { side: v, .. } = &mut f.primitives[0].kind {
        *v = side;
    }
    s.primitives.extend(f.primitives);
    for (n, y) in [("RX", 15), ("TX", 5)] {
        let e = entry(s, id, point(x + 2, y), n).unwrap();
        s.primitives.push(e);
    }
    id
}
fn pin(d: &mut Document, position: Point, name: &str) -> Uuid {
    let s = d.schematic.as_mut().unwrap();
    let id = Uuid::new_v4();
    s.components.push(Component {
        id,
        source_index: edit::AUTHORED,
        position,
        designator: name.into(),
        comment: String::new(),
        library_reference: "Pin".into(),
        description: String::new(),
        part: 1,
        display_mode: 0,
        parameters: Default::default(),
        settings: Default::default(),
    });
    s.primitives.push(edit::primitive(
        K::Pin {
            position,
            length_nm: 0,
            orientation: 0,
            name: String::new(),
            number: "1".into(),
            show_name: false,
            show_number: false,
            font: 0,
            electrical: 4,
            number_font: None,
            outer_symbol: 0,
            inner_symbol: 0,
        },
        Some(id),
    ));
    id
}
fn harness(d: &mut Document, a: Point, b: Point) {
    d.schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(
            K::Polyline {
                points: vec![a, b],
                closed: false,
                electrical: "Signal Harness".into(),
            },
            None,
        ));
}
#[test]
fn channels_connect_by_entry_name_without_shorting_or_type_global_merging() {
    let mut d = scene();
    let a = add_connector(&mut d, 0, 1);
    let b = add_connector(&mut d, 20, 0);
    let c = add_connector(&mut d, 40, 1);
    let positions: Vec<_> = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::HarnessEntry { position, .. } = p.kind {
                Some(position)
            } else {
                None
            }
        })
        .collect();
    let ids: Vec<_> = positions
        .iter()
        .enumerate()
        .map(|(i, p)| pin(&mut d, *p, &format!("J{}", i + 1)))
        .collect();
    let s = d.schematic.as_ref().unwrap();
    let a = terminal(s.primitive(a).unwrap()).unwrap();
    let b = terminal(s.primitive(b).unwrap()).unwrap();
    harness(&mut d, a, b);
    let net = crate::design_sync::compile(&[&d]).unwrap();
    let name = |id| {
        net.pins
            .iter()
            .find(|p| p.component == id)
            .unwrap()
            .net
            .clone()
    };
    assert_eq!(name(ids[0]), name(ids[2]));
    assert_eq!(name(ids[1]), name(ids[3]));
    assert_ne!(name(ids[0]), name(ids[1]));
    assert_ne!(name(ids[0]), name(ids[4]));
    assert_ne!(c, Uuid::nil());
}
#[test]
fn entry_ownership_copy_undo_and_invalid_placement() {
    let mut d = scene();
    let id = add_connector(&mut d, 0, 1);
    d.validate().unwrap();
    let before = d.clone();
    let mut h = History::default();
    let edit = Edit::Transform {
        id,
        pivot: point(0, 0),
        quarter_turns: 1,
        delta: point(20, 0),
    };
    h.schematic_edit(
        &mut d,
        Request {
            document: before.id,
            expected_revision: 0,
            edit,
        },
    )
    .unwrap();
    d.validate().unwrap();
    let copied = edit::extract(d.schematic.as_ref().unwrap(), id)
        .unwrap()
        .fresh_copy();
    assert_eq!(copied.primitives.len(), 4);
    assert!(
        copied
            .primitives
            .iter()
            .skip(1)
            .all(|p| p.presentation.parent == Some(copied.primitives[0].id))
    );
    assert!(entry(d.schematic.as_ref().unwrap(), id, point(100, 100), "Bad").is_err());
    h.undo(&mut d);
    assert_eq!(d, before);
}
#[test]
fn harness_label_names_channels() {
    let mut d = scene();
    let a = add_connector(&mut d, 0, 1);
    let b = add_connector(&mut d, 20, 0);
    let sa = d.schematic.as_ref().unwrap();
    let a = terminal(sa.primitive(a).unwrap()).unwrap();
    let b = terminal(sa.primitive(b).unwrap()).unwrap();
    harness(&mut d, a, b);
    let positions: Vec<_> = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::HarnessEntry { position, .. } = p.kind {
                Some(position)
            } else {
                None
            }
        })
        .collect();
    for (n, p) in positions.iter().enumerate() {
        pin(&mut d, *p, &format!("J{n}"));
    }
    let mut label = edit::primitive(
        K::Text {
            position: a,
            text: "BUS1".into(),
            font: 0,
            orientation: 0,
            justification: 0,
        },
        None,
    );
    label.text_role = Some(crate::schematic::TextRole::NetLabel);
    d.schematic.as_mut().unwrap().primitives.push(label);
    let net = crate::design_sync::compile(&[&d]).unwrap();
    assert_eq!(net.names, vec!["BUS1.RX", "BUS1.TX"]);
}
#[test]
fn definition_syntax_and_conflicts_are_checked() {
    let text = "Locked;Serial=RX,TX,Control:Logic\nLogic=EN,RST\n";
    let definitions = parse_definitions(text).unwrap();
    assert!(definitions[0].locked);
    assert_eq!(
        parse_definitions(&format_definitions(&definitions)).unwrap(),
        definitions
    );
    assert!(parse_definitions("Serial=RX,rx").is_err());
    let mut d = scene();
    add_connector(&mut d, 0, 1);
    let id = add_connector(&mut d, 20, 1);
    let s = d.schematic.as_mut().unwrap();
    let p = s
        .primitives
        .iter_mut()
        .find(|p| p.presentation.parent == Some(id) && matches!(p.kind, K::HarnessEntry { .. }))
        .unwrap();
    if let K::HarnessEntry { name, .. } = &mut p.kind {
        *name = "Other".into();
    }
    assert!(super::definitions(s).is_err());
}

#[test]
fn harness_channels_cross_sheet_entry_and_port_without_becoming_one_net() {
    let mut parent = scene();
    parent.name = "Parent.SchDoc".into();
    let a = add_connector(&mut parent, 0, 1);
    let mut child = scene();
    child.name = "Child.SchDoc".into();
    let b = add_connector(&mut child, 20, 0);
    let symbol = edit::primitive(
        K::SheetSymbol {
            a: point(30, 20),
            b: point(35, 0),
            designator: "Child".into(),
            filename: child.name.clone(),
        },
        None,
    );
    let mut entry = edit::primitive(
        K::SheetEntry {
            position: point(30, 10),
            side: 0,
            io_type: 0,
            name: "DATA".into(),
            font: 0,
            text_color: 0,
            arrow_kind: "Block".into(),
        },
        None,
    );
    entry.presentation.parent = Some(symbol.id);
    parent
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .extend([symbol, entry]);
    let port = edit::primitive(
        K::Port {
            position: point(0, 10),
            width_nm: 2_540_000,
            height_nm: 2_540_000,
            style: 0,
            io_type: 0,
            name: "DATA".into(),
            font: 0,
        },
        None,
    );
    child.schematic.as_mut().unwrap().primitives.push(port);
    let a = terminal(parent.schematic.as_ref().unwrap().primitive(a).unwrap()).unwrap();
    let b = terminal(child.schematic.as_ref().unwrap().primitive(b).unwrap()).unwrap();
    harness(&mut parent, a, point(30, 10));
    harness(&mut child, point(1, 10), b);
    let mut ids = vec![];
    for d in [&mut parent, &mut child] {
        let positions: Vec<_> = d
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .filter_map(|p| {
                if let K::HarnessEntry { position, .. } = p.kind {
                    Some(position)
                } else {
                    None
                }
            })
            .collect();
        for (n, p) in positions.iter().enumerate() {
            let id = pin(d, *p, &format!("J{n}"));
            ids.push(id);
        }
    }
    let net = crate::design_sync::compile(&[&parent, &child]).unwrap();
    let name = |id| {
        net.pins
            .iter()
            .find(|p| p.component == id)
            .unwrap()
            .net
            .clone()
    };
    assert_eq!(name(ids[0]), name(ids[2]));
    assert_eq!(name(ids[1]), name(ids[3]));
    assert_ne!(name(ids[0]), name(ids[1]));
}

#[test]
fn nested_harness_channels_preserve_leaf_identity() {
    let mut d = scene();
    let left = add_connector(&mut d, 20, 0);
    let right = add_connector(&mut d, 40, 1);
    let positions: Vec<_> = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::HarnessEntry { position, .. } = p.kind {
                Some(position)
            } else {
                None
            }
        })
        .collect();
    let ids: Vec<_> = positions
        .iter()
        .enumerate()
        .map(|(n, p)| pin(&mut d, *p, &format!("J{n}")))
        .collect();
    for (x, side) in [(0, 1), (60, 0)] {
        let mut f = connector(point(x, 60), point(x + 5, 40), 0, "Outer").unwrap();
        if let K::HarnessConnector { side: v, .. } = &mut f.primitives[0].kind {
            *v = side;
        }
        let id = f.primitives[0].id;
        let s = d.schematic.as_mut().unwrap();
        s.primitives.extend(f.primitives);
        let mut e = entry(s, id, point(x + 2, 50), "Data").unwrap();
        e.presentation.harness_type = "Serial".into();
        s.primitives.push(e);
    }
    harness(&mut d, point(5, 50), point(60, 50));
    let s = d.schematic.as_ref().unwrap();
    let left = terminal(s.primitive(left).unwrap()).unwrap();
    let right = terminal(s.primitive(right).unwrap()).unwrap();
    for path in [
        vec![point(0, 50), point(-5, 50), point(-5, 10), left],
        vec![point(65, 50), point(70, 50), point(70, 10), right],
    ] {
        d.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(edit::primitive(
                K::Polyline {
                    points: path,
                    closed: false,
                    electrical: "Signal Harness".into(),
                },
                None,
            ));
    }
    let net = crate::design_sync::compile(&[&d]).unwrap();
    let name = |id| {
        net.pins
            .iter()
            .find(|p| p.component == id)
            .unwrap()
            .net
            .clone()
    };
    assert_eq!(name(ids[0]), name(ids[2]));
    assert_eq!(name(ids[1]), name(ids[3]));
    assert_ne!(name(ids[0]), name(ids[1]));
}

#[test]
fn bus_ranges_and_ports_expand_bits_without_shorting() {
    let mut docs = vec![];
    for n in 0..2 {
        let mut d = scene();
        d.name = format!("Bus{n}.SchDoc");
        let s = d.schematic.as_mut().unwrap();
        s.primitives.push(edit::primitive(
            K::Polyline {
                points: vec![point(0, 0), point(0, 20)],
                closed: false,
                electrical: "Bus".into(),
            },
            None,
        ));
        s.primitives.push(edit::primitive(
            K::Port {
                position: point(0, 0),
                width_nm: 2_540_000,
                height_nm: 2_540_000,
                style: 0,
                io_type: 0,
                name: "D[1..0]".into(),
                font: 0,
            },
            None,
        ));
        for bit in 0..2 {
            let y = 5 + 10 * bit;
            let mut label = edit::primitive(
                K::Text {
                    position: point(3, y),
                    text: format!("D{bit}"),
                    font: 0,
                    orientation: 0,
                    justification: 0,
                },
                None,
            );
            label.text_role = Some(crate::schematic::TextRole::NetLabel);
            let s = d.schematic.as_mut().unwrap();
            s.primitives.push(label);
            s.primitives.push(edit::primitive(
                K::Polyline {
                    points: vec![point(0, y), point(5, y)],
                    closed: false,
                    electrical: "Wire".into(),
                },
                None,
            ));
            pin(&mut d, point(5, y), &format!("J{n}{bit}"));
        }
        docs.push(d);
    }
    let net = crate::design_sync::compile(&docs.iter().collect::<Vec<_>>()).unwrap();
    assert_eq!(net.names, vec!["D0", "D1"]);
    for bit in 0..2 {
        assert_eq!(
            net.pins
                .iter()
                .filter(|p| p.net.as_deref() == Some(&format!("D{bit}")))
                .count(),
            2
        );
    }
}

#[test]
fn connector_drag_stretches_harness_and_entry_drag_remains_on_edge() {
    let mut d = scene();
    let id = add_connector(&mut d, 0, 1);
    let tip = terminal(d.schematic.as_ref().unwrap().primitive(id).unwrap()).unwrap();
    harness(&mut d, tip, point(20, 10));
    let moved = edit::preview(
        &d,
        &Edit::Drag {
            ids: vec![id],
            pivot: point(0, 0),
            quarter_turns: 0,
            delta: point(0, 5),
        },
    )
    .unwrap();
    let s = moved.schematic.as_ref().unwrap();
    let tip = terminal(s.primitive(id).unwrap()).unwrap();
    assert!(s.primitives.iter().any(|p|matches!(&p.kind,K::Polyline{points,electrical,..} if electrical=="Signal Harness"&&points.contains(&tip))));
    let entry = s
        .primitives
        .iter()
        .find(|p| p.presentation.parent == Some(id) && matches!(p.kind, K::HarnessEntry { .. }))
        .unwrap()
        .id;
    let moved = edit::preview(
        &moved,
        &Edit::Drag {
            ids: vec![entry],
            pivot: point(0, 0),
            quarter_turns: 0,
            delta: point(7, 1),
        },
    )
    .unwrap();
    moved.validate().unwrap();
    if let K::HarnessEntry { position, .. } = moved
        .schematic
        .as_ref()
        .unwrap()
        .primitive(entry)
        .unwrap()
        .kind
    {
        assert_eq!(position.x, 0);
    }
}

#[test]
fn connector_free_locked_definition_joins_qualified_channels_without_shorting() {
    let mut d = scene();
    harness(&mut d, point(0, 0), point(20, 0));
    let mut port = edit::place(edit::Placement::Port, point(0, 0), 0, "", "Bundle", 0)
        .unwrap()
        .primitives
        .remove(0);
    port.presentation.harness_type = "Serial".into();
    d.schematic.as_mut().unwrap().primitives.push(port);
    for (name, at) in [
        ("SOFT", point(10, 0)),
        ("SOFT.RX", point(0, 10)),
        ("SOFT.TX", point(10, 10)),
    ] {
        let f = edit::place(edit::Placement::NetLabel, at, 0, "", name, 0).unwrap();
        d.schematic
            .as_mut()
            .unwrap()
            .primitives
            .extend(f.primitives);
    }
    let rx = pin(&mut d, point(0, 10), "J1");
    let tx = pin(&mut d, point(10, 10), "J2");
    let options = crate::design_sync::ProjectOptions {
        harness_definitions: parse_definitions("LOCKED;Serial=RX,TX").unwrap(),
        ..Default::default()
    };
    let net = crate::design_sync::compile_project(&[&d], &options).unwrap();
    assert_eq!(
        net.pins
            .iter()
            .find(|p| p.component == rx)
            .unwrap()
            .net
            .as_deref(),
        Some("SOFT.RX")
    );
    assert_eq!(
        net.pins
            .iter()
            .find(|p| p.component == tx)
            .unwrap()
            .net
            .as_deref(),
        Some("SOFT.TX")
    );
    let mut cyclic = options;
    cyclic.harness_definitions = parse_definitions("LOCKED;Serial={Loop:Serial}").unwrap();
    assert!(
        crate::design_sync::compile_project(&[&d], &cyclic)
            .unwrap_err()
            .contains("Cyclic")
    );
}
