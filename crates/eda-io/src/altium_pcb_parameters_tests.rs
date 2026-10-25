use super::*;

fn board() -> Board {
    let mut board = Board::default();
    board.components.push(
        component(
            Fields::from([
                ("UNIQUEID".into(), "ABCDEFGH".into()),
                ("SOURCEDESIGNATOR".into(), "BM1".into()),
            ]),
            Uuid::nil(),
            0,
        )
        .unwrap(),
    );
    board
}
fn blocks(records: &[&str]) -> Vec<u8> {
    records
        .iter()
        .flat_map(|s| {
            let mut bytes = ((s.len() + 1) as u32).to_le_bytes().to_vec();
            bytes.extend_from_slice(s.as_bytes());
            bytes.push(0);
            bytes
        })
        .collect()
}
fn data() -> Vec<u8> {
    blocks(&[
        "|PRIMITIVEID=ABCDEFGH|ID=Component#0|APPURTENANCE=Footprint|VARIANTGUID=Footprint|COUNT=1",
        "|NAME=ID|VALUE=footprint-only",
        "|PRIMITIVEID=ABCDEFGH|ID=Component#0|VARIANTGUID=|COUNT=2",
        "|NAME=ID|VALUE=216|ISIMPORTED=TRUE",
        "|NAME=Empty|VALUE=",
        "|PRIMITIVEID=ABCDEFGH|ID=Component#0|VARIANTGUID=variant-guid|COUNT=1",
        "|NAME=ID|VALUE=variant-only",
    ])
}

#[test]
fn base_parameters_keep_names_values_and_ignore_other_namespaces() {
    let mut board = board();
    import(&mut board, &data()).unwrap();
    assert_eq!(
        board.components[0].parameters,
        Some(Fields::from([
            ("ID".into(), "216".into()),
            ("Empty".into(), "".into()),
        ]))
    );
    let json = serde_json::to_vec(&board).unwrap();
    assert_eq!(serde_json::from_slice::<Board>(&json).unwrap(), board);
}

#[test]
fn malformed_groups_and_wrong_owners_fail_atomically() {
    for data in [
        blocks(&[
            "|PRIMITIVEID=ABCDEFGH|ID=Component#0|COUNT=2",
            "|NAME=ID|VALUE=216",
        ]),
        blocks(&["|PRIMITIVEID=WRONG|ID=Component#0|COUNT=0"]),
        blocks(&["|PRIMITIVEID=ABCDEFGH|ID=Component#9|COUNT=0"]),
        blocks(&[
            "|PRIMITIVEID=ABCDEFGH|ID=Component#0|COUNT=2",
            "|NAME=ID|VALUE=216",
            "|NAME=id|VALUE=217",
        ]),
        blocks(&["|PRIMITIVEID=ABCDEFGH|ID=Component#0|COUNT=1", "|NAME=ID"]),
        blocks(&["|PRIMITIVEID=ABCDEFGH|ID=Component#0|COUNT=18446744073709551615"]),
    ] {
        let mut board = board();
        let old = board.clone();
        assert!(import(&mut board, &data).is_err());
        assert_eq!(board, old);
    }
}

fn native_document() -> Document {
    let mut doc = Document::new(DocumentKind::Board, "parameters");
    doc.pcb = Some(board());
    let bytes = writer::serialize(&doc, writer::Format::Binary).unwrap();
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    cfb.create_storage_all("/PrimitiveParameters").unwrap();
    cfb.create_stream("/PrimitiveParameters/Data")
        .unwrap()
        .write_all(&data())
        .unwrap();
    cfb.create_stream("/PrimitiveParameters/Header")
        .unwrap()
        .write_all(&3u32.to_le_bytes())
        .unwrap();
    import_pcb(&cfb.into_inner().into_inner(), "parameters.PcbDoc").unwrap()
}

#[test]
fn native_import_roundtrip_and_legacy_json_recovery_preserve_parameter_identity() {
    let doc = native_document();
    let board = doc.pcb.as_ref().unwrap();
    assert_eq!(
        board.components[0].parameters.as_ref().unwrap()["ID"],
        "216"
    );
    let saved = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    let mut compound = cfb::CompoundFile::open(Cursor::new(&saved)).unwrap();
    let mut preserved = vec![];
    compound
        .open_stream("/PrimitiveParameters/Data")
        .unwrap()
        .read_to_end(&mut preserved)
        .unwrap();
    assert_eq!(preserved, data());
    let reread = import_pcb(&saved, "saved.PcbDoc").unwrap();
    assert_eq!(
        reread.pcb.as_ref().unwrap().components[0].parameters,
        board.components[0].parameters
    );
    let mut legacy = board.clone();
    legacy.components[0].index = 77;
    legacy.components[0].position = Point::mm(12, 13);
    legacy.components[0].parameters = None;
    upgrade_component_parameters(&mut legacy).unwrap();
    assert_eq!(
        legacy.components[0].parameters,
        board.components[0].parameters
    );
    assert_eq!(legacy.components[0].position, Point::mm(12, 13));
    assert_eq!(legacy.components[0].index, 77);
    let mut changed = doc.clone();
    changed.pcb.as_mut().unwrap().components[0]
        .parameters
        .as_mut()
        .unwrap()
        .insert("ID".into(), "217".into());
    assert!(
        writer::serialize(&changed, writer::Format::Preserve)
            .unwrap_err()
            .contains("component parameters")
    );
    assert!(writer::serialize(&doc, writer::Format::Ascii).is_err());
}
