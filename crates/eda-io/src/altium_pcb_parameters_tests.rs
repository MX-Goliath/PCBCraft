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
fn component_link_urls_decode_native_equals_escaping_and_encode_on_edit() {
    let mut doc = native_document();
    let params = doc.pcb.as_mut().unwrap().components[0]
        .parameters
        .as_mut()
        .unwrap();
    params.insert(
        "ComponentLink1URL".into(),
        "https://example.test/?id=1&part=2".into(),
    );
    params.insert("Literal".into(), "a{}b=c".into());
    let bytes = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    let read = import_pcb(&bytes, "link.PcbDoc").unwrap();
    assert_eq!(
        read.pcb.as_ref().unwrap().components[0].parameters,
        doc.pcb.as_ref().unwrap().components[0].parameters
    );
    let mut cfb = cfb::CompoundFile::open(Cursor::new(&bytes)).unwrap();
    let mut raw = vec![];
    cfb.open_stream("/PrimitiveParameters/Data")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    let records = property_records(&raw, 0).unwrap();
    assert!(records.iter().any(|r| {
        r.get("VALUE")
            .is_some_and(|v| v == "https://example.test/?id{}1&part{}2")
    }));
    assert_eq!(
        writer::serialize(&read, writer::Format::Preserve).unwrap(),
        bytes
    );
    let mut edited = read;
    edited.pcb.as_mut().unwrap().components[0]
        .parameters
        .as_mut()
        .unwrap()
        .insert(
            "ComponentLink1URL".into(),
            "https://example.test/?id=3".into(),
        );
    let bytes = writer::serialize(&edited, writer::Format::Preserve).unwrap();
    let read = import_pcb(&bytes, "edited.PcbDoc").unwrap();
    assert_eq!(
        read.pcb.as_ref().unwrap().components[0].parameters,
        edited.pcb.as_ref().unwrap().components[0].parameters
    );
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
    let bytes = writer::serialize(&changed, writer::Format::Preserve).unwrap();
    let reread = import_pcb(&bytes, "changed.PcbDoc").unwrap();
    assert_eq!(
        reread.pcb.as_ref().unwrap().components[0]
            .parameters
            .as_ref()
            .unwrap()["ID"],
        "217"
    );
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let mut data = vec![];
    cfb.open_stream("/PrimitiveParameters/Data")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    let records = property_records(&data, 0).unwrap();
    assert_eq!(records[1]["VALUE"], "footprint-only");
    assert_eq!(records[6]["VALUE"], "variant-only");
    assert_eq!(records[3]["ISIMPORTED"], "TRUE");
    assert!(writer::serialize(&doc, writer::Format::Ascii).is_err());
}

#[test]
fn native_base_parameter_add_remove_unicode_and_source_links_roundtrip() {
    let mut doc = native_document();
    let c = &mut doc.pcb.as_mut().unwrap().components[0];
    c.parameters = Some(Fields::from([("Номинал".into(), "12 мкФ".into())]));
    c.properties
        .insert("SOURCEUNIQUEID".into(), "/SHEET/NEW_LINK".into());
    c.properties
        .insert("SOURCESHEET".into(), "Circuit.SchDoc".into());
    let bytes = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    let reread = import_pcb(&bytes, "sync.PcbDoc").unwrap();
    let c = &reread.pcb.as_ref().unwrap().components[0];
    assert_eq!(
        c.parameters.as_ref().unwrap(),
        &Fields::from([("Номинал".into(), "12 мкФ".into())])
    );
    assert_eq!(c.properties["SOURCEUNIQUEID"], "/SHEET/NEW_LINK");
    assert_eq!(c.properties["SOURCESHEET"], "Circuit.SchDoc");
    let mut empty = reread;
    empty.pcb.as_mut().unwrap().components[0].parameters = Some(Fields::new());
    let bytes = writer::serialize(&empty, writer::Format::Preserve).unwrap();
    assert!(
        import_pcb(&bytes, "empty.PcbDoc")
            .unwrap()
            .pcb
            .unwrap()
            .components[0]
            .parameters
            .as_ref()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn newly_authored_component_parameters_get_a_matching_native_owner() {
    let mut doc = Document::new(DocumentKind::Board, "new");
    let mut b = board();
    b.components[0].properties.remove("UNIQUEID");
    b.components[0].parameters = Some(Fields::from([("Value".into(), "42".into())]));
    doc.pcb = Some(b);
    let bytes = writer::serialize(&doc, writer::Format::Binary).unwrap();
    let reread = import_pcb(&bytes, "new.PcbDoc").unwrap();
    assert_eq!(
        reread.pcb.as_ref().unwrap().components[0]
            .parameters
            .as_ref()
            .unwrap()["Value"],
        "42"
    );
}

#[test]
fn legacy_uid_only_parameter_groups_roundtrip_and_edit_without_duplicate_groups() {
    let doc = native_document();
    let bytes = writer::serialize(&doc, writer::Format::Preserve).unwrap();
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let legacy = blocks(&[
        "|PRIMITIVEID=ABCDEFGH|VARIANTGUID=|COUNT=1",
        "|NAME=Value|VALUE=original",
        "|PRIMITIVEID=ABCDEFGH|VARIANTGUID=variant|COUNT=1",
        "|NAME=Value|VALUE=variant-value",
    ]);
    cfb.create_stream("/PrimitiveParameters/Data")
        .unwrap()
        .write_all(&legacy)
        .unwrap();
    cfb.create_stream("/PrimitiveParameters/Header")
        .unwrap()
        .write_all(&2u32.to_le_bytes())
        .unwrap();
    let original = cfb.into_inner().into_inner();
    let mut read = import_pcb(&original, "legacy.PcbDoc").unwrap();
    assert_eq!(
        read.pcb.as_ref().unwrap().components[0]
            .parameters
            .as_ref()
            .unwrap()["Value"],
        "original"
    );
    assert_eq!(
        writer::serialize(&read, writer::Format::Preserve).unwrap(),
        original
    );
    read.pcb.as_mut().unwrap().components[0]
        .parameters
        .as_mut()
        .unwrap()
        .insert("Value".into(), "edited".into());
    let saved = writer::serialize(&read, writer::Format::Preserve).unwrap();
    let reread = import_pcb(&saved, "edited.PcbDoc").unwrap();
    assert_eq!(
        reread.pcb.as_ref().unwrap().components[0]
            .parameters
            .as_ref()
            .unwrap()["Value"],
        "edited"
    );
    let mut cfb = cfb::CompoundFile::open(Cursor::new(saved)).unwrap();
    let mut raw = vec![];
    cfb.open_stream("/PrimitiveParameters/Data")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    let records = property_records(&raw, 0).unwrap();
    assert_eq!(records.len(), 4);
    assert!(!records[0].contains_key("ID"));
    assert_eq!(records[3]["VALUE"], "variant-value");
}

#[test]
fn uid_only_parameter_owner_must_be_unambiguous() {
    let mut b = board();
    let mut duplicate = b.components[0].clone();
    duplicate.index = 1;
    b.components.push(duplicate);
    let before = b.clone();
    let error = import(&mut b, &blocks(&["|PRIMITIVEID=ABCDEFGH|COUNT=0"])).unwrap_err();
    assert!(error.contains("Ambiguous"));
    assert_eq!(b, before);
}
