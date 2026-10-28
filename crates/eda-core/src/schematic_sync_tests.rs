use super::*;
use crate::{DocumentKind, History, schematic_edit::Request};
fn fixture() -> (Document, Document, Uuid, Uuid, Uuid) {
    let mut parent = Document::new(DocumentKind::Schematic, "Parent.SchDoc");
    let child = Document::new(DocumentKind::Schematic, "Child.SchDoc");
    let symbol = edit::primitive(
        K::SheetSymbol {
            a: Point::mm(20, 80),
            b: Point::mm(70, 20),
            designator: "Child".into(),
            filename: child.name.clone(),
        },
        None,
    );
    let owner = symbol.id;
    parent.schematic.as_mut().unwrap().primitives.push(symbol);
    let signal = Signal {
        id: Uuid::new_v4(),
        name: "SIGNAL".into(),
        io: 2,
        harness: "Harness".into(),
    };
    let plan = transfer(&parent, Destination::Entries(owner), &[signal], &[]).unwrap();
    parent = edit::preview(&parent, &plan.edit_at(&parent, Point::mm(20, 70)).unwrap()).unwrap();
    let entry = parent
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find_map(Signal::read)
        .unwrap();
    let plan = transfer(
        &child,
        Destination::Ports,
        std::slice::from_ref(&entry),
        &[],
    )
    .unwrap();
    let child = edit::preview(&child, &plan.edit_at(&child, Point::mm(30, 40)).unwrap()).unwrap();
    let port = child
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find_map(Signal::read)
        .unwrap();
    (parent, child, owner, entry.id, port.id)
}
#[test]
fn comparison_checks_name_io_harness_and_explicit_unlink() {
    let (parent, child, owner, entry, port) = fixture();
    let c = compare(&parent, &child, owner, &[]).unwrap();
    assert_eq!(c.links.len(), 1);
    assert!(c.entries.is_empty());
    assert!(c.ports.is_empty());
    let c = compare(&parent, &child, owner, &[(entry, port)]).unwrap();
    assert_eq!((c.entries.len(), c.ports.len(), c.links.len()), (1, 1, 0));
    for field in 0..3 {
        let mut other = child.clone();
        let p = other
            .schematic
            .as_mut()
            .unwrap()
            .primitives
            .iter_mut()
            .find(|p| p.id == port)
            .unwrap();
        match field {
            0 => {
                if let K::Port { name, .. } = &mut p.kind {
                    *name = "Other".into()
                }
            }
            1 => {
                if let K::Port { io_type, .. } = &mut p.kind {
                    *io_type = 1
                }
            }
            _ => p.presentation.harness_type = "OtherHarness".into(),
        }
        let c = compare(&parent, &other, owner, &[]).unwrap();
        assert_eq!((c.entries.len(), c.ports.len(), c.links.len()), (1, 1, 0));
    }
}
#[test]
fn transfer_updates_only_counterpart_and_history_can_undo_delete_and_create() {
    let (parent, mut child, owner, entry, port) = fixture();
    let before = child.clone();
    let p = parent.schematic.as_ref().unwrap().primitive(entry).unwrap();
    let mut source = Signal::read(p).unwrap();
    source.name = "Renamed".into();
    source.io = 3;
    source.harness = "NewHarness".into();
    let plan = transfer(&child, Destination::Ports, &[source], &[port]).unwrap();
    assert!(plan.create.is_empty());
    let mut history = History::default();
    let document = child.id;
    history
        .schematic_edit(
            &mut child,
            Request {
                document,
                expected_revision: 0,
                edit: plan.edit_at(&before, Point::default()).unwrap(),
            },
        )
        .unwrap();
    let result = child.schematic.as_ref().unwrap().primitive(port).unwrap();
    let signal = Signal::read(result).unwrap();
    assert_eq!(
        (&*signal.name, signal.io, &*signal.harness),
        ("Renamed", 3, "NewHarness")
    );
    assert_eq!(
        edit::electrical_endpoint(result),
        edit::electrical_endpoint(before.schematic.as_ref().unwrap().primitive(port).unwrap())
    );
    history.undo(&mut child);
    assert_eq!(child, before);
    let c = compare(&parent, &child, owner, &[]).unwrap();
    let plan = transfer(&child, Destination::Ports, &[c.links[0].0.clone()], &[]).unwrap();
    let next = edit::preview(&child, &plan.edit_at(&child, Point::mm(60, 50)).unwrap()).unwrap();
    assert_eq!(next.schematic.as_ref().unwrap().primitives.len(), 2);
    let deleted = edit::preview(&parent, &Edit::Delete(entry)).unwrap();
    assert!(
        deleted
            .schematic
            .as_ref()
            .unwrap()
            .primitive(owner)
            .is_some()
    );
    assert_eq!(
        compare(&deleted, &child, owner, &[]).unwrap().ports.len(),
        1
    );
}
#[test]
fn entry_group_projects_to_each_edge_and_refuses_an_oversized_group() {
    let (parent, _, owner, _, _) = fixture();
    let source: Vec<_> = (0..3)
        .map(|n| Signal {
            id: Uuid::new_v4(),
            name: format!("P{n}"),
            io: 2,
            harness: String::new(),
        })
        .collect();
    let plan = transfer(&parent, Destination::Entries(owner), &source, &[]).unwrap();
    for (side, at) in [
        (0, Point::mm(20, 60)),
        (1, Point::mm(70, 60)),
        (2, Point::mm(40, 80)),
        (3, Point::mm(40, 20)),
    ] {
        let result = edit::preview(&parent, &plan.edit_at(&parent, at).unwrap()).unwrap();
        for p in result.schematic.as_ref().unwrap().primitives.iter().skip(2) {
            assert!(matches!(p.kind,K::SheetEntry{side:s,..}if s==side));
            assert_eq!(p.presentation.parent, Some(owner));
        }
    }
    let many = vec![source[0].clone(); 1000];
    let plan = transfer(&parent, Destination::Entries(owner), &many, &[]).unwrap();
    assert!(plan.edit_at(&parent, Point::default()).is_err());
}
