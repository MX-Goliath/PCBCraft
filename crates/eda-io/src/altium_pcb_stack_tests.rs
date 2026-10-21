use super::writer::{Format, serialize};
use super::*;
fn pad() -> Item {
    Item::new(
        MULTI_LAYER,
        None,
        Shape::Pad {
            position: Point::mm(2, 2),
            size: Point::mm(3, 3),
            rotation: 20.0,
            shape: PadShape::Round,
            hole: 500_000,
            plated: true,
            name: "A1".into(),
            bottom_size: None,
            middle_size: None,
        },
    )
}
#[test]
fn native_full_stack_slot_offsets_masks_and_opaque_tail_roundtrip() {
    let b = Board::default();
    let mut p = pad();
    p.set_stack_mode(StackMode::Full, &b);
    let mut g = p.pad_geometry(BOTTOM).unwrap();
    g.size = Point::mm(4, 2);
    g.shape = PadShape::RoundedRectangle;
    g.corner_percent = 60;
    g.offset = Point::mm(1, 0);
    p.set_pad_geometry(BOTTOM, g.clone());
    let s = p.pad_stack.as_mut().unwrap();
    s.drill_shape = DrillShape::Slot;
    s.slot_length = 1_000_000;
    s.drill_rotation = 35.0;
    let mut m = MaskSettings::for_item(&p);
    m.solder[0].tented = true;
    for side in &mut m.solder {
        side.mode = MaskMode::Manual;
        side.expansion = 50_800;
    }
    p.masks = Some(Box::new(m));
    let mut d = Document::new(DocumentKind::Board, "stack");
    d.pcb = Some(Board {
        items: vec![p],
        ..b
    });
    let bytes = serialize(&d, Format::Binary).unwrap();
    let doc = import_pcb(&bytes, "stack.PcbDoc").unwrap();
    let b = doc.pcb.as_ref().unwrap();
    let p = &b.items[0];
    assert_eq!(p.stack_mode(), StackMode::Full);
    let parsed = p.pad_geometry(BOTTOM).unwrap();
    assert_eq!(parsed.shape, g.shape);
    assert_eq!(parsed.corner_percent, 60);
    assert!((parsed.size.x - g.size.x).abs() <= 2);
    assert!((parsed.offset.x - g.offset.x).abs() <= 2);
    let stack = p.pad_stack.as_ref().unwrap();
    assert_eq!(stack.drill_shape, DrillShape::Slot);
    assert_eq!(stack.drill_rotation, 35.0);
    assert!(p.masks.as_ref().unwrap().solder[0].tented);
    assert!(!p.masks.as_ref().unwrap().solder[1].tented);
    assert_eq!(p.masks.as_ref().unwrap().solder[0].expansion, 50_800);
    assert_eq!(serialize(&doc, Format::Preserve).unwrap(), bytes);
    // Patching native records must retain unknown trailers and wildcard rows.
    let mut blocks = vec![vec![], vec![], vec![], vec![], vec![0; 194], vec![0; 636]];
    blocks[5][628..632].copy_from_slice(&1u32.to_le_bytes());
    blocks[5][632..636].copy_from_slice(&15u32.to_le_bytes());
    let opaque = [4, 0, 128, 1, 2, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    blocks[5].extend(opaque);
    blocks[5].extend(b"unknown-tail");
    stack_io::patch(p, None, &mut blocks).unwrap();
    assert_eq!(&blocks[5][636..651], &opaque);
    assert!(blocks[5].ends_with(b"unknown-tail"));
}
#[test]
fn native_via_stack_diameters_and_hole_edge_roundtrip() {
    let mut b = Board::default();
    let mut v = Item::new(
        MULTI_LAYER,
        None,
        Shape::Via {
            position: Point::mm(1, 2),
            diameter: 1_000_000,
            hole: 300_000,
            from_layer: TOP,
            to_layer: BOTTOM,
        },
    );
    v.set_stack_mode(StackMode::Full, &b);
    let mut g = v.pad_geometry(BOTTOM).unwrap();
    g.size = Point::mm(2, 2);
    v.set_pad_geometry(BOTTOM, g);
    let mut m = MaskSettings::for_item(&v);
    for side in &mut m.solder {
        side.from_hole_edge = true;
    }
    v.masks = Some(Box::new(m));
    b.items.push(v);
    let mut d = Document::new(DocumentKind::Board, "via");
    d.pcb = Some(b);
    let bytes = serialize(&d, Format::Binary).unwrap();
    let d = import_pcb(&bytes, "via.PcbDoc").unwrap();
    let v = &d.pcb.as_ref().unwrap().items[0];
    assert!((v.pad_geometry(BOTTOM).unwrap().size.x - 2_000_000).abs() <= 2);
    assert!(v.masks.as_ref().unwrap().solder[0].from_hole_edge);
}
#[test]
fn full_stack_import_uses_board6_internal_layers_before_finish() {
    let mut b = Board::default();
    b.properties.insert("LAYER1NEXT".into(), "2".into());
    b.properties.insert("LAYER2NEXT".into(), "32".into());
    let mut p = pad();
    let mut blocks = vec![vec![], vec![], vec![], vec![], vec![0; 194], vec![]];
    p.set_stack_mode(StackMode::Full, &b);
    let mut g = p.pad_geometry(2).unwrap();
    g.size = Point::mm(5, 5);
    p.set_pad_geometry(2, g);
    stack_io::patch(&p, None, &mut blocks).unwrap();
    let mut read = pad();
    stack_io::read_pad(&mut read, &blocks[4], &blocks[5], &b).unwrap();
    assert!((read.pad_geometry(2).unwrap().size.x - 5_000_000).abs() <= 2);
}
#[test]
fn unsupported_native_masks_refuse_lossy_save_while_json_keeps_them() {
    let mut p = pad();
    let mut m = MaskSettings::for_item(&p);
    m.solder[0].mode = MaskMode::Manual;
    m.solder[0].expansion = 100_000;
    m.linked_solder = false;
    p.masks = Some(Box::new(m));
    let mut d = Document::new(DocumentKind::Board, "independent");
    d.pcb = Some(Board {
        items: vec![p],
        ..Default::default()
    });
    assert!(
        serialize(&d, Format::Binary)
            .unwrap_err()
            .contains("Independent")
    );
    let text = serde_json::to_string(&d).unwrap();
    assert_eq!(serde_json::from_str::<Document>(&text).unwrap(), d);
}
#[test]
fn native_mask_rule_edit_retains_prefix_unknown_fields_and_other_rules() {
    let mut d = Document::new(DocumentKind::Board, "rules");
    d.pcb = Some(Board::default());
    let bytes = serialize(&d, Format::Binary).unwrap();
    let mut c = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    c.create_storage_all("/Rules6").unwrap();
    let records = [b"|RULEKIND=SolderMaskExpansion|NAME=Mask|PRIORITY=1|SCOPE1EXPRESSION=All|EXPANSION=4mil|OPAQUE=keep\0".as_slice(),
        b"|RULEKIND=Clearance|NAME=clear|OPAQUE=untouched\0".as_slice()];
    let mut data = vec![];
    for r in records {
        data.extend([0x91, 0x02]);
        data.extend((r.len() as u32).to_le_bytes());
        data.extend(r);
    }
    c.create_stream("/Rules6/Data")
        .unwrap()
        .write_all(&data)
        .unwrap();
    c.create_stream("/Rules6/Header")
        .unwrap()
        .write_all(&2u32.to_le_bytes())
        .unwrap();
    let mut doc = import_pcb(&c.into_inner().into_inner(), "rules.PcbDoc").unwrap();
    let b = doc.pcb.as_ref().unwrap();
    let mut rule = b.rules[0].clone();
    rule.insert("EXPANSION".into(), "0.2mm".into());
    doc.pcb = Some(b.replace_mask_rule(0, rule).unwrap());
    let bytes = serialize(&doc, Format::Preserve).unwrap();
    let mut c = cfb::CompoundFile::open(Cursor::new(bytes.clone())).unwrap();
    let mut data = vec![];
    c.open_stream("/Rules6/Data")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(&data[..2], &[0x91, 0x02]);
    assert!(data.windows(records[1].len()).any(|w| w == records[1]));
    let parsed = import_pcb(&bytes, "edited.PcbDoc").unwrap().pcb.unwrap();
    assert_eq!(parsed.rules[0]["EXPANSION"], "0.2mm");
    assert_eq!(parsed.rules[0]["OPAQUE"], "keep");
    assert_eq!(parsed.rules[1], doc.pcb.as_ref().unwrap().rules[1]);
}
