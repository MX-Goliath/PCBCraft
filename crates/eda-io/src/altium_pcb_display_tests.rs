use super::*;

fn header(n: usize) -> Vec<u8> {
    let mut p = vec![0; n];
    p[1] = 0x0c;
    p[3..13].fill(255);
    p
}
fn frame(tag: u8, blocks: &[&[u8]]) -> Vec<u8> {
    let mut out = vec![tag];
    for block in blocks {
        out.extend_from_slice(&(block.len() as u32).to_le_bytes());
        out.extend_from_slice(block);
    }
    out
}
fn region(extended: bool, body: bool) -> Vec<u8> {
    region_on_layer(
        extended,
        body,
        if body { "MECHANICAL20" } else { "MECHANICAL18" },
    )
}
fn region_on_layer(extended: bool, body: bool, layer: &str) -> Vec<u8> {
    let mut p = header(18);
    p[0] = 72;
    let f = format!("V7_LAYER={layer}|KIND=0|ARCRESOLUTION=0.5mil|MODEL.MODELTYPE=1\0");
    p.extend_from_slice(&(f.len() as u32).to_le_bytes());
    p.extend_from_slice(f.as_bytes());
    p.extend_from_slice(&4u32.to_le_bytes());
    let points: [(i32, i32); 4] = [
        (0, 0),
        (1_000_000, 0),
        (1_000_000, 1_000_000),
        (0, 1_000_000),
    ];
    for (x, y) in points
        .into_iter()
        .chain(points.into_iter().take(usize::from(extended)))
    {
        if extended {
            p.push(0);
            p.extend_from_slice(&x.to_le_bytes());
            p.extend_from_slice(&y.to_le_bytes());
            p.extend_from_slice(&[0; 28]);
        } else {
            p.extend_from_slice(&(x as f64).to_le_bytes());
            p.extend_from_slice(&(y as f64).to_le_bytes());
        }
    }
    p.extend_from_slice(b"opaque-region-tail");
    frame(if body { 12 } else { 11 }, &[&p])
}
fn fixture() -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    let board=b"|VX0=0mm|VY0=0mm|VX1=10mm|VY1=0mm|VX2=10mm|VY2=10mm|VX3=0mm|VY3=10mm|V9_CACHE_LAYER85_LAYERID=16908306|V9_CACHE_LAYER85_NAME=Carbon Mask\0";
    let board = [(board.len() as u32).to_le_bytes().as_slice(), board].concat();
    let mut track = header(49);
    track[0] = 72;
    track[21..25].copy_from_slice(&1_000_000i32.to_le_bytes());
    track[29..33].copy_from_slice(&100_000i32.to_le_bytes());
    track[41..45].copy_from_slice(&0x01020012u32.to_le_bytes());
    track[45..].copy_from_slice(b"tail");
    let track = frame(4, &[&track]);
    for (name, data) in [
        ("Board6", board),
        ("Tracks6", track),
        ("Regions6", region(false, false)),
        ("ShapeBasedRegions6", region(true, false)),
        ("ComponentBodies6", region(false, true)),
        ("ShapeBasedComponentBodies6", region(true, true)),
    ] {
        file.create_storage(format!("/{name}")).unwrap();
        file.create_stream(format!("/{name}/Data"))
            .unwrap()
            .write_all(&data)
            .unwrap();
        file.create_stream(format!("/{name}/Header"))
            .unwrap()
            .write_all(&1u32.to_le_bytes())
            .unwrap();
    }
    file.into_inner().into_inner()
}
#[test]
fn connect_layer_regions_do_not_prevent_binary_board_import() {
    let mut file = cfb::CompoundFile::open(Cursor::new(fixture())).unwrap();
    for (family, extended) in [("Regions6", false), ("ShapeBasedRegions6", true)] {
        file.create_stream(format!("/{family}/Data"))
            .unwrap()
            .write_all(&region_on_layer(extended, false, "CONNECT"))
            .unwrap();
    }
    let bytes = file.into_inner().into_inner();
    let doc = import_pcb(&bytes, "connect.PcbDoc").unwrap();
    let board = doc.pcb.as_ref().unwrap();
    assert_eq!(board.items.len(), 5);
    assert_eq!(
        board.items.iter().filter(|item| item.layer == 75).count(),
        2
    );
    assert!(
        !board
            .layers
            .iter()
            .find(|layer| layer.id == 75)
            .unwrap()
            .copper
    );
    assert_eq!(
        writer::serialize(&doc, writer::Format::Preserve).unwrap(),
        bytes
    );
}
#[test]
fn modern_layers_and_body_projections_keep_native_records_when_another_item_changes() {
    let bytes = fixture();
    let mut doc = import_pcb(&bytes, "layers.PcbDoc").unwrap();
    let b = doc.pcb.as_mut().unwrap();
    assert_eq!(b.items.len(), 5);
    assert_eq!(
        b.items
            .iter()
            .filter(|i| i
                .properties
                .contains_key("PCBCRAFT_LEGACY_REGION_DUPLICATE"))
            .count(),
        2
    );
    assert_eq!(
        b.layers.iter().find(|l| l.id == 84).unwrap().name,
        "Carbon Mask"
    );
    assert_eq!(
        b.items
            .iter()
            .filter(|i| i.properties.contains_key("PCBCRAFT_BODY_PROJECTION"))
            .count(),
        2
    );
    let item = b
        .items
        .iter_mut()
        .find(|i| matches!(i.shape, Shape::Track { .. }))
        .unwrap();
    assert_eq!(item.layer, 84);
    if let Shape::Track { width, .. } = &mut item.shape {
        *width += 25_400;
    }
    let output = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    let mut original = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let mut saved = cfb::CompoundFile::open(Cursor::new(output.clone())).unwrap();
    for family in [
        "ComponentBodies6",
        "ShapeBasedComponentBodies6",
        "Regions6",
        "ShapeBasedRegions6",
    ] {
        let mut a = vec![];
        let mut b = vec![];
        original
            .open_stream(format!("/{family}/Data"))
            .unwrap()
            .read_to_end(&mut a)
            .unwrap();
        saved
            .open_stream(format!("/{family}/Data"))
            .unwrap()
            .read_to_end(&mut b)
            .unwrap();
        assert_eq!(a, b);
    }
    let mut data = vec![];
    saved
        .open_stream("/Tracks6/Data")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(data[5], 72);
    assert_eq!(&data[46..50], &0x01020012u32.to_le_bytes());
    assert_eq!(&data[50..54], b"tail");
    assert_eq!(
        import_pcb(&output, "reopened.PcbDoc")
            .unwrap()
            .pcb
            .unwrap()
            .items
            .len(),
        5
    );
}
#[test]
fn moving_only_a_body_projection_cannot_silently_leave_a_stale_native_model() {
    let mut doc = import_pcb(&fixture(), "body.PcbDoc").unwrap();
    let b = doc.pcb.as_mut().unwrap();
    let i = b
        .items
        .iter_mut()
        .find(|i| i.properties.contains_key("PCBCRAFT_BODY_PROJECTION"))
        .unwrap();
    if let Shape::Region { contour, .. } = &mut i.shape {
        for p in contour {
            p.x += 25_400;
        }
    }
    assert!(
        writer::serialize(&doc, writer::Format::Preserve)
            .unwrap_err()
            .contains("3D")
    );
}
#[test]
fn text_font_kind_is_independent_of_bold_charset_and_barcode_kind() {
    let mut p = header(252);
    p[0] = 72;
    p[21..25].copy_from_slice(&100_000i32.to_le_bytes());
    p[25..27].copy_from_slice(&2u16.to_le_bytes());
    p[42] = 204;
    p[43] = 1;
    p[44] = 0;
    p[45] = 1;
    p[157] = 1;
    p[160] = 0;
    p[226..230].copy_from_slice(&0x01020012u32.to_le_bytes());
    let i = text_item(&p, b"\x01C", &BTreeMap::new(), &Board::default()).unwrap();
    assert_eq!(i.layer, 84);
    for (key, value) in [
        ("FONT_ID", "2"),
        ("TTF", "0"),
        ("BOLD", "0"),
        ("ITALIC", "1"),
        ("INVERTED", "0"),
    ] {
        assert_eq!(i.properties[&format!("PCBCRAFT_TEXT_{key}")], value);
    }
}

#[test]
fn literal_component_comment_survives_missing_sourcecomment_and_drives_properties() {
    let mut board = Board::default();
    board.components.push(Component {
        id: Uuid::new_v4(),
        index: 0,
        designator: "NM1".into(),
        comment: String::new(),
        footprint: "Code_2D".into(),
        parameters: Some(Default::default()),
        position: Point::default(),
        rotation: 0.0,
        layer: TOP,
        locked: false,
        properties: Fields::from([("COMMENTON".into(), "FALSE".into())]),
    });
    let mut p = header(252);
    p[0] = TOP_OVERLAY as u8;
    p[7..9].copy_from_slice(&0u16.to_le_bytes());
    p[21..25].copy_from_slice(&100_000i32.to_le_bytes());
    p[40] = 1;
    let item = text_item(&p, b"\x072D-code", &BTreeMap::new(), &board).unwrap();
    assert!(matches!(&item.shape,Shape::Text{text,..} if text=="2D-code"));
    let id = item.id;
    board.items.push(item);
    finish(&mut board).unwrap();
    assert_eq!(board.components[0].comment, "2D-code");
    assert!(!board.text_visible(board.item(id).unwrap()));
    let mut edited = board.components[0].clone();
    edited.comment = "New value".into();
    let next = eda_core::pcb_tools::edit_component(&board, edited).unwrap();
    assert!(matches!(&next.item(id).unwrap().shape,Shape::Text{text,..} if text=="New value"));
    assert_eq!(board.components[0].comment, "2D-code");
}
