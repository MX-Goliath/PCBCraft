use super::*;
use crate::{DocumentKind, library_edit};

fn fixture(bottom: bool) -> (Document, Update) {
    let mut library = library_edit::member(DocumentKind::Board, "TEST").unwrap();
    let b = library.pcb.as_mut().unwrap();
    b.components[0].parameters = Some(Fields::from([("Vendor".into(), "New".into())]));
    b.components[0]
        .properties
        .insert("DESCRIPTION".into(), "Updated".into());
    for n in 1..=2 {
        let mut pad = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::mm(n, 0),
                size: Point::mm(2, 1),
                rotation: 0.,
                shape: PadShape::Rectangle,
                hole: 0,
                plated: true,
                name: n.to_string(),
                bottom_size: None,
                middle_size: None,
            },
        );
        pad.component = Some(b.components[0].index);
        b.items.push(pad);
    }
    let mut drawing = Item::new(
        TOP_OVERLAY,
        None,
        Shape::Track {
            start: Point::mm(-1, -1),
            end: Point::mm(3, -1),
            width: 100_000,
        },
    );
    drawing.component = Some(b.components[0].index);
    b.items.push(drawing);
    let definition = Footprint::from_library_board(b.clone()).unwrap();
    let id = b.components[0].id;
    let mut placed = b.clone();
    if bottom {
        placed = crate::pcb_tools::flip_component(&placed, id).unwrap();
    }
    let angle = 90. - placed.components[0].rotation;
    placed = placed
        .edited(Edit::Transform {
            selection: vec![id],
            pivot: Point::default(),
            translation: Point::mm(30, 40),
            angle,
        })
        .unwrap();
    placed.components[0].designator = "U1".into();
    placed.components[0].comment = "LOCAL".into();
    placed.components[0]
        .properties
        .insert("UNIQUEID".into(), "NATIVEID".into());
    placed.components[0]
        .properties
        .insert("SOURCEUNIQUEID".into(), "SCHLINK".into());
    placed.components[0].parameters = Some(Fields::from([("Vendor".into(), "Schematic".into())]));
    placed.components[0].footprint_parameters =
        Some(Fields::from([("Vendor".into(), "Old".into())]));
    placed.nets.push(Net {
        index: 0,
        name: "GND".into(),
        properties: Fields::new(),
    });
    placed.items[0].net = Some(0);
    placed.items[0].source = Some(SourceRecord {
        stream: "/Pads6/Data".into(),
        index: 0,
    });
    if let Shape::Pad { size, .. } = &mut placed.items[0].shape {
        *size = Point::mm(1, 1);
    }
    let mut masks = MaskSettings::default();
    masks.solder[0].mode = MaskMode::Manual;
    masks.solder[0].expansion = 123_456;
    placed.items[0].masks = Some(Box::new(masks));
    let mut text = Item::new(
        TOP_OVERLAY,
        None,
        Shape::Text {
            position: Point::mm(28, 41),
            text: "U1".into(),
            height: 1_000_000,
            stroke: 100_000,
            rotation: 17.,
            mirrored: false,
            font: "Arial".into(),
        },
    );
    text.component = Some(placed.components[0].index);
    text.properties.insert("DESIGNATOR".into(), "TRUE".into());
    placed.items.push(text);
    placed.items.push(Item::new(
        TOP,
        Some(0),
        Shape::Track {
            start: Point::mm(30, 41),
            end: Point::mm(35, 41),
            width: 250_000,
        },
    ));
    let mut doc = Document::new(DocumentKind::Board, "Target.PcbDoc");
    doc.pcb = Some(placed);
    (
        doc,
        Update {
            component: id,
            footprint: definition,
            options: Options::default(),
            source_library: Some("Parts.PcbLib".into()),
        },
    )
}
#[test]
fn rotated_both_sides_preserve_identity_nets_text_tracks_masks_and_history() {
    for bottom in [false, true] {
        let (mut doc, u) = fixture(bottom);
        let original = doc.clone();
        let old = doc.pcb.as_ref().unwrap();
        let target_pad = old.items[0].clone();
        let text = old.items[3].clone();
        let external = old.items[4].clone();
        let component = old.components[0].clone();
        let mut history = History::default();
        assert!(history.update_footprints(&mut doc, 0, &[u]).unwrap());
        let b = doc.pcb.as_ref().unwrap();
        let c = &b.components[0];
        assert_eq!(
            (
                c.id,
                c.index,
                c.position,
                c.rotation,
                c.layer,
                c.designator.as_str(),
                c.comment.as_str()
            ),
            (
                component.id,
                component.index,
                component.position,
                component.rotation,
                component.layer,
                "U1",
                "LOCAL"
            )
        );
        assert_eq!(c.properties["UNIQUEID"], "NATIVEID");
        assert_eq!(c.properties["SOURCEUNIQUEID"], "SCHLINK");
        assert_eq!(c.properties["FOOTPRINTDESCRIPTION"], "Updated");
        assert_eq!(c.footprint_parameters.as_ref().unwrap()["Vendor"], "New");
        assert_eq!(c.parameters.as_ref().unwrap()["Vendor"], "Schematic");
        let pad = b.items.iter().find(|i| i.id == target_pad.id).unwrap();
        assert_eq!(
            (pad.net, pad.source.clone(), pad.layer, pad.anchor()),
            (
                Some(0),
                target_pad.source.clone(),
                target_pad.layer,
                target_pad.anchor()
            )
        );
        assert!(matches!(pad.shape,Shape::Pad {size,..} if size == Point::mm(2,1)));
        assert_eq!(pad.masks, target_pad.masks);
        assert!(b.items.contains(&text));
        assert!(b.items.contains(&external));
        let after = doc.clone();
        history.undo(&mut doc);
        assert_eq!(doc, original);
        history.redo(&mut doc);
        assert_eq!(doc, after);
        assert!(history.update_footprints(&mut doc, 0, &[]).is_err());
    }
}
#[test]
fn options_preserve_excluded_geometry_parameters_and_allow_mask_replacement() {
    let (doc, mut u) = fixture(false);
    u.options.other = false;
    u.options.parameters = false;
    u.options.text = true;
    let next = preview(&doc, std::slice::from_ref(&u)).unwrap();
    let old = doc.pcb.as_ref().unwrap();
    let b = next.pcb.as_ref().unwrap();
    assert!(b.items.contains(&old.items[0]));
    assert!(b.items.contains(&old.items[3]));
    assert_eq!(b.components[0].parameters, old.components[0].parameters);
    assert_eq!(
        b.components[0].footprint_parameters,
        old.components[0].footprint_parameters
    );
    u.options.solder_mask = true;
    let next = preview(&doc, std::slice::from_ref(&u)).unwrap();
    let pad = next
        .pcb
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|p| p.id == old.items[0].id)
        .unwrap();
    assert_eq!(pad.shape, old.items[0].shape);
    assert_eq!(mask_settings(pad).solder[0].mode, MaskMode::Rule);
    u.options.other = true;
    u.options.solder_mask = true;
    let next = preview(&doc, &[u]).unwrap();
    let pad = next
        .pcb
        .as_ref()
        .unwrap()
        .items
        .iter()
        .find(|p| p.id == old.items[0].id)
        .unwrap();
    assert_eq!(mask_settings(pad).solder[0].mode, MaskMode::Rule);
    assert_eq!(mask_settings(pad).paste, mask_settings(&old.items[0]).paste);
}
#[test]
fn conflicting_pad_nets_removed_connected_pads_and_unsupported_updates_are_atomic() {
    let (mut doc, u) = fixture(false);
    let original = doc.clone();
    let mut invalid = u.clone();
    invalid.options.thermal_relief = true;
    let mut h = History::default();
    assert!(h.update_footprints(&mut doc, 0, &[invalid]).is_err());
    assert_eq!(doc, original);
    let b = doc.pcb.as_mut().unwrap();
    let mut duplicate = b.items[0].clone();
    duplicate.id = Uuid::new_v4();
    duplicate.net = None;
    b.items.push(duplicate);
    assert!(
        preview(&doc, std::slice::from_ref(&u))
            .unwrap_err()
            .contains("conflicting nets")
    );
    let (doc, mut u) = fixture(false);
    let mut source = u.footprint.board().clone();
    source.items.remove(0);
    u.footprint = Footprint::from_library_board(source).unwrap();
    assert!(
        preview(&doc, &[u])
            .unwrap_err()
            .contains("omits connected pad")
    );
}
#[test]
fn all_targets_validate_before_commit_and_locked_objects_are_rejected() {
    let (mut doc, u) = fixture(false);
    let original = doc.clone();
    let mut invalid = u.clone();
    invalid.component = Uuid::new_v4();
    let mut h = History::default();
    assert!(
        h.update_footprints(&mut doc, 0, &[u.clone(), invalid])
            .is_err()
    );
    assert_eq!(doc, original);
    assert_eq!(h.revision, 0);
    doc.pcb.as_mut().unwrap().components[0].locked = true;
    assert!(preview(&doc, &[u]).unwrap_err().contains("Unlock"));
}
