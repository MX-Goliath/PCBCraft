use super::*;
use crate::{
    DocumentKind,
    schematic_edit::{self as edit, Placement},
};
use eda_geom::Point;

fn circuit() -> Document {
    let d = Document::new(DocumentKind::Schematic, "Circuit.SchDoc");
    let f = edit::place(Placement::Resistor, Point::mm(20, 20), 0, "R1", "10k", 0).unwrap();
    edit::preview(&d, &edit::Edit::Insert(f)).unwrap()
}
fn report(d: &Document) -> Report {
    validate(&[d], &Default::default(), &Default::default())
}
fn short_pins(d: &mut Document) {
    let s = d.schematic.as_mut().unwrap();
    let points = s
        .primitives
        .iter()
        .filter_map(edit::electrical_endpoint)
        .collect();
    s.primitives.push(edit::primitive(
        K::Polyline {
            points,
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    ));
}
#[test]
fn unconnected_pins_and_passive_net_have_navigation_targets_without_mutation() {
    let mut d = circuit();
    let before = d.clone();
    let r = report(&d);
    assert!(r.compiled);
    assert!(r.diagnostics.is_empty(), "Passive/Unconnected is No Report");
    assert!(r.diagnostics.iter().all(|m| !m.targets.is_empty()));
    assert_eq!(d, before);
    short_pins(&mut d);
    let r = report(&d);
    assert!(
        !r.diagnostics
            .iter()
            .any(|m| m.message.contains("has no driving source"))
    );
    if let K::Pin { electrical, .. } = &mut d
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .iter_mut()
        .find(|p| matches!(p.kind, K::Pin { .. }))
        .unwrap()
        .kind
    {
        *electrical = 0;
    }
    assert!(
        report(&d)
            .diagnostics
            .iter()
            .any(|m| m.message.contains("has no driving source"))
    );
    assert!(
        !r.diagnostics
            .iter()
            .any(|m| m.message.starts_with("Unconnected"))
    );
}
#[test]
fn pin_matrix_and_no_erc_are_electrical_and_compile_masks_exclude_objects() {
    let mut d = circuit();
    short_pins(&mut d);
    for p in &mut d.schematic.as_mut().unwrap().primitives {
        if let K::Pin { electrical, .. } = &mut p.kind {
            *electrical = 2;
        }
    }
    assert!(
        report(&d)
            .diagnostics
            .iter()
            .any(|m| m.severity == Severity::Error && m.message.contains("Output Pin"))
    );
    let points: Vec<_> = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter_map(edit::electrical_endpoint)
        .collect();
    for position in points {
        d.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(edit::primitive(K::NoErc { position }, None));
    }
    assert!(report(&d).diagnostics.is_empty());
    d.schematic
        .as_mut()
        .unwrap()
        .primitives
        .retain(|p| !matches!(p.kind, K::NoErc { .. }));
    d.schematic
        .as_mut()
        .unwrap()
        .compile_masks
        .push(crate::schematic::CompileMask {
            a: Point::mm(1, 1),
            b: Point::mm(100, 100),
            collapsed: false,
        });
    assert!(report(&d).diagnostics.is_empty());
}

#[test]
fn no_erc_on_wire_suppresses_net_diagnostics_but_not_unrelated_nets() {
    let mut d = circuit();
    short_pins(&mut d);
    let wire = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find_map(|p| {
            if let K::Polyline {
                points, electrical, ..
            } = &p.kind
            {
                (electrical == "Wire").then_some(points.clone())
            } else {
                None
            }
        })
        .unwrap();
    let position = Point {
        x: (wire[0].x + wire[1].x) / 2,
        y: (wire[0].y + wire[1].y) / 2,
    };
    d.schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(K::NoErc { position }, None));
    assert!(report(&d).diagnostics.is_empty());
    let f = edit::place(Placement::Resistor, Point::mm(80, 20), 0, "R2", "10k", 0).unwrap();
    d = edit::preview(&d, &edit::Edit::Insert(f)).unwrap();
    for p in &mut d.schematic.as_mut().unwrap().primitives {
        if let K::Pin { electrical, .. } = &mut p.kind {
            *electrical = 0;
        }
    }
    assert_eq!(
        report(&d)
            .diagnostics
            .iter()
            .filter(|m| m.message.starts_with("Unconnected Pin R2-"))
            .count(),
        2
    );
}
#[test]
fn native_matrix_overrides_severity_and_rejects_truncated_values() {
    let mut d = circuit();
    short_pins(&mut d);
    let rows = [
        "NNNNNNNN", "NNNNNNNN", "NNNNNNNN", "NNNNNNNN", "NNNNFNNN", "NNNNNNNN", "NNNNNNNN",
        "NNNNNNNN",
    ];
    let options = Options::from_native_matrix(&rows).unwrap();
    let r = validate(&[&d], &Default::default(), &options);
    assert!(!r.compiled);
    assert!(r.diagnostics.iter().any(|m| m.severity == Severity::Fatal));
    assert!(Options::from_native_matrix(&["N"; 8]).is_err());
    assert!(Options::from_native_matrix(&["XXXXXXXX"; 8]).is_err());
}

#[test]
fn repeated_pin_of_a_multipart_component_is_one_physical_pin() {
    let mut d = circuit();
    let f = edit::place(Placement::Resistor, Point::mm(50, 20), 0, "U1", "10k", 0).unwrap();
    d = edit::preview(&d, &edit::Edit::Insert(f)).unwrap();
    let s = d.schematic.as_mut().unwrap();
    for (i, c) in s.components.iter_mut().enumerate() {
        c.designator = "U1".into();
        c.settings.part_count = 2;
        c.part = i as u32 + 1;
    }
    for p in &mut s.primitives {
        if let K::Pin { electrical, .. } = &mut p.kind {
            *electrical = 2;
        }
    }
    let mut options = Options::default();
    options.connection_matrix[2][16] = Some(Severity::Warning);
    let r = validate(&[&d], &Default::default(), &options);
    assert!(
        !r.diagnostics
            .iter()
            .any(|m| m.message.contains("Output Pin and Output Pin"))
    );
    assert_eq!(
        r.diagnostics
            .iter()
            .filter(|m| m.message.starts_with("Unconnected Pin"))
            .count(),
        2
    );
}

#[test]
fn validation_reports_net_aliases_and_continues_while_eco_refuses_them() {
    let mut d = circuit();
    short_pins(&mut d);
    if let K::Pin { electrical, .. } = &mut d
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .iter_mut()
        .find(|p| matches!(p.kind, K::Pin { .. }))
        .unwrap()
        .kind
    {
        *electrical = 0;
    }
    let position = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find_map(edit::electrical_endpoint)
        .unwrap();
    for text in ["FIRST", "SECOND"] {
        let mut p = edit::primitive(
            K::Text {
                position,
                text: text.into(),
                font: 0,
                orientation: 0,
                justification: 0,
            },
            None,
        );
        p.text_role = Some(crate::schematic::TextRole::NetLabel);
        d.schematic.as_mut().unwrap().primitives.push(p);
    }
    assert!(
        design_sync::compile(&[&d])
            .unwrap_err()
            .contains("Conflicting labels")
    );
    let r = report(&d);
    assert!(r.compiled);
    assert!(
        r.diagnostics
            .iter()
            .any(|m| m.message.contains("has multiple names") && !m.targets.is_empty())
    );
    assert!(
        r.diagnostics
            .iter()
            .any(|m| m.message.contains("has no driving source"))
    );
    let mut options = Options::default();
    options.set_native_level(10, 0).unwrap();
    options.set_native_level(14, 3).unwrap();
    let changed = validate(&[&d], &Default::default(), &options);
    assert!(!changed.compiled);
    assert_eq!(changed.diagnostics.len(), 1);
    assert_eq!(changed.diagnostics[0].severity, Severity::Fatal);
    assert!(
        changed.diagnostics[0]
            .message
            .contains("has multiple names")
    );
    options.set_native_level(10, 3).unwrap();
    options.set_native_level(14, 0).unwrap();
    let changed = validate(&[&d], &Default::default(), &options);
    assert_eq!(changed.diagnostics.len(), 1);
    assert!(
        changed.diagnostics[0]
            .message
            .contains("has no driving source")
    );
    d.schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(K::NoErc { position }, None));
    assert!(report(&d).diagnostics.is_empty());
}
#[test]
fn duplicate_annotation_does_not_abort_other_checks_and_multipart_is_distinct() {
    let mut d = circuit();
    let f = edit::place(Placement::Resistor, Point::mm(50, 20), 0, "R1", "10k", 0).unwrap();
    d = edit::preview(&d, &edit::Edit::Insert(f)).unwrap();
    let r = report(&d);
    assert!(
        r.diagnostics
            .iter()
            .any(|m| m.message == "Duplicate Component Designators R1")
    );
    assert!(r.compiled);
    for (i, c) in d
        .schematic
        .as_mut()
        .unwrap()
        .components
        .iter_mut()
        .enumerate()
    {
        c.settings.part_count = 2;
        c.part = i as u32 + 1;
    }
    assert!(
        !report(&d)
            .diagnostics
            .iter()
            .any(|m| m.message.starts_with("Duplicate Component"))
    );
}
#[test]
fn project_global_power_joins_sheets_and_unsupported_compilation_never_succeeds() {
    let mut a = circuit();
    let mut b = circuit();
    b.name = "Second.SchDoc".into();
    b.schematic.as_mut().unwrap().components[0].designator = "R2".into();
    for d in [&mut a, &mut b] {
        if let K::Pin { electrical, .. } = &mut d
            .schematic
            .as_mut()
            .unwrap()
            .primitives
            .iter_mut()
            .find(|p| matches!(p.kind, K::Pin { .. }))
            .unwrap()
            .kind
        {
            *electrical = 0;
        }
        let position = d
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find_map(edit::electrical_endpoint)
            .unwrap();
        d.schematic
            .as_mut()
            .unwrap()
            .primitives
            .push(edit::primitive(
                K::PowerPort {
                    cross_sheet: false,
                    position,
                    orientation: 0,
                    style: 0,
                    text: "VCC".into(),
                    show_name: true,
                    font: 0,
                },
                None,
            ));
    }
    let r = validate(&[&a, &b], &Default::default(), &Default::default());
    assert!(
        r.diagnostics
            .iter()
            .any(|m| m.message.contains("Net VCC has no driving source")
                && m.targets.iter().any(|t| t.document == a.id)
                && m.targets.iter().any(|t| t.document == b.id))
    );
    let options = design_sync::ProjectOptions {
        unsupported_connectivity: Some("Unsupported channel".into()),
        ..Default::default()
    };
    let r = validate(&[&a, &b], &options, &Default::default());
    assert!(!r.compiled);
    assert!(
        r.diagnostics
            .iter()
            .any(|m| m.message == "Unsupported channel")
    );
}

#[test]
fn native_full_matrix_checks_ports_and_unconnected_and_no_erc() {
    let mut d = circuit();
    short_pins(&mut d);
    for p in &mut d.schematic.as_mut().unwrap().primitives {
        if let K::Pin { electrical, .. } = &mut p.kind {
            *electrical = 2;
        }
    }
    let position = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find_map(edit::electrical_endpoint)
        .unwrap();
    let port = edit::primitive(
        K::Port {
            position,
            width_nm: 0,
            height_nm: 0,
            style: 0,
            io_type: 2,
            name: "INPUT".into(),
            font: 0,
        },
        None,
    );
    let port_id = port.id;
    d.schematic.as_mut().unwrap().primitives.push(port);
    let mut rows = vec!["NNNNNNNNNNNNNNNNN".to_owned(); 17];
    rows[2].replace_range(8..9, "F");
    let options =
        Options::from_native_matrix(&rows.iter().map(String::as_str).collect::<Vec<_>>()).unwrap();
    let r = validate(&[&d], &Default::default(), &options);
    assert!(!r.compiled);
    assert!(r.diagnostics.iter().any(|r|r.severity==Severity::Fatal && r.targets.iter().any(|t|t.object==port_id)));
    d.schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(K::NoErc { position }, None));
    assert!(
        validate(&[&d], &Default::default(), &options)
            .diagnostics
            .is_empty()
    );
    let mut isolated = Document::new(DocumentKind::Schematic, "Port.SchDoc");
    isolated
        .schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(
            K::Port {
                position,
                width_nm: 0,
                height_nm: 0,
                style: 0,
                io_type: 2,
                name: "INPUT".into(),
                font: 0,
            },
            None,
        ));
    assert!(
        report(&isolated)
            .diagnostics
            .iter()
            .any(|r| r.message == "Unconnected Port INPUT")
    );
}
