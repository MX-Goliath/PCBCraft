use super::*;
use eda_core::schematic_edit::{self, Edit};

const ASCII: &[u8] = include_bytes!("../../../fixtures/schematic/minimal.SchDoc");

fn binary(text: &[u8]) -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    let mut records = vec![];
    for line in text.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        records.extend(((line.len() + 1) as u32).to_le_bytes());
        records.extend(line);
        records.push(0);
    }
    file.create_stream("FileHeader")
        .unwrap()
        .write_all(&records)
        .unwrap();
    file.create_stream("FutureData")
        .unwrap()
        .write_all(b"\0\xffopaque\0\x01")
        .unwrap();
    file.into_inner().into_inner()
}
fn edit(doc: &Document, edit: Edit) -> Document {
    schematic_edit::preview(doc, &edit).unwrap()
}

#[test]
fn unchanged_native_archives_are_returned_byte_for_byte() {
    for bytes in [ASCII.to_vec(), binary(ASCII)] {
        let mut doc = import_schematic(&bytes, "unchanged.SchDoc").unwrap();
        assert_eq!(serialize(&doc).unwrap(), bytes);
        doc.schematic.as_mut().unwrap().source_modified = true;
        assert_eq!(serialize(&doc).unwrap(), bytes);
    }
}

#[test]
fn deleting_a_parameter_remaps_later_component_and_model_owners() {
    let text = [ASCII, b"\n|RECORD=41|OWNERINDEX=1|NAME=Remove|TEXT=Old|ISHIDDEN=T\n|RECORD=1|LIBREFERENCE=Resistor|PARTCOUNT=2|CURRENTPARTID=1|LOCATION.X=300|LOCATION.Y=100\n|RECORD=34|OWNERINDEX=16|OWNERPARTID=-1|TEXT=R2\n|RECORD=41|OWNERINDEX=16|OWNERPARTID=-1|NAME=Comment|TEXT=22k|ISHIDDEN=T\n|RECORD=44|OWNERINDEX=16\n|RECORD=45|OWNERINDEX=19|MODELNAME=0805|MODELTYPE=PCBLIB|ISCURRENT=T\n"].concat();
    let doc = import_schematic(&binary(&text), "owners.SchDoc").unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    let next = edit(
        &doc,
        Edit::ParameterValue {
            component: Some(c.id),
            name: "Remove".into(),
            value: None,
        },
    );
    let data = serialize(&next).unwrap();
    let read = import_schematic(&data, "owners.SchDoc").unwrap();
    let s = read.schematic.unwrap();
    assert_eq!(s.components[1].designator, "R2");
    assert_eq!(s.components[1].comment, "22k");
    assert_eq!(s.components[1].source_index, 15);
    assert_eq!(s.components[1].settings.models[0].name, "0805");
    assert!(
        s.primitives
            .iter()
            .any(|p| p.component == Some(s.components[1].id))
    );
}

#[test]
fn legacy_encoding_and_utf8_overrides_roundtrip_together() {
    let text = String::from_utf8(ASCII.to_vec())
        .unwrap()
        .replace("Пример схемы", "Legacy example");
    let text = format!(
        "{text}\n|RECORD=41|OWNERINDEX=1|OWNERPARTID=-1|NAME=Value|TEXT=Резистор|%UTF8%TEXT=Резистор|ISHIDDEN=T\n"
    );
    // The legacy part is Windows-1251, explicit overrides remain UTF-8.
    let mut legacy = vec![];
    for token in text.split('|') {
        if !legacy.is_empty() {
            legacy.push(b'|');
        }
        if token.starts_with("%UTF8%") {
            legacy.extend(token.as_bytes());
        } else {
            legacy.extend(WINDOWS_1251.encode(token).0.as_ref());
        }
    }
    let doc = import_schematic(&binary(&legacy), "legacy.SchDoc").unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    let next = edit(
        &doc,
        Edit::ComponentProperties {
            id: c.id,
            designator: "R42".into(),
            comment: "日本語 µΩ".into(),
        },
    );
    let data = serialize(&next).unwrap();
    let read = import_schematic(&data, "legacy.SchDoc").unwrap();
    let s = read.schematic.unwrap();
    assert_eq!(s.components[0].comment, "日本語 µΩ");
    assert_eq!(s.components[0].parameters["Value"], "Резистор");
}

#[test]
fn eco_metadata_and_unicode_parameters_roundtrip_ascii_and_binary() {
    let extra = [
        ASCII,
        b"\n|RECORD=41|OWNERINDEX=1|OWNERPARTID=-1|NAME=Remove|TEXT=Old|ISHIDDEN=T\n",
    ]
    .concat();
    for bytes in [extra.clone(), binary(&extra)] {
        let doc = import_schematic(&bytes, "eco.SchDoc").unwrap();
        let c = &doc.schematic.as_ref().unwrap().components[0];
        let mut settings = c.settings.clone();
        settings.kind = 5;
        settings.models = vec![ComponentModel {
            name: "Корпус 0603".into(),
            kind: "PCBLIB".into(),
            current: true,
        }];
        let next = edit(
            &doc,
            Edit::Batch(vec![
                Edit::ComponentProperties {
                    id: c.id,
                    designator: "R42".into(),
                    comment: "10 кОм".into(),
                },
                Edit::ComponentDetails {
                    id: c.id,
                    description: "Резистор".into(),
                    settings,
                },
                Edit::ParameterValue {
                    component: Some(c.id),
                    name: "Remove".into(),
                    value: None,
                },
                Edit::ParameterValue {
                    component: Some(c.id),
                    name: "Допуск".into(),
                    value: Some("±1%; 125 °C".into()),
                },
            ]),
        );
        let data = serialize(&next).unwrap();
        assert_eq!(
            serialize(&next).unwrap(),
            data,
            "repeated saves must retain new native identities"
        );
        let read = import_schematic(&data, "eco.SchDoc").unwrap();
        if data.starts_with(&[0xd0, 0xcf]) {
            let mut file = cfb::CompoundFile::open(Cursor::new(data.clone())).unwrap();
            let mut stream = vec![];
            file.open_stream("FileHeader")
                .unwrap()
                .read_to_end(&mut stream)
                .unwrap();
            let mut blocks = vec![];
            framed("FileHeader", &stream, &mut blocks).unwrap();
            let header = decode_fields(&blocks[0].2, detect_encoding(&blocks)).unwrap();
            assert_eq!(
                header.n("WEIGHT"),
                read.schematic.as_ref().unwrap().records.len() as i64
            );
        }
        let result = &read.schematic.as_ref().unwrap().components[0];
        assert_eq!(result.designator, "R42");
        assert_eq!(result.comment, "10 кОм");
        assert_eq!(result.description, "Резистор");
        assert_eq!(result.settings.kind, 5);
        assert_eq!(result.settings.models[0].name, "Корпус 0603");
        assert_eq!(result.parameters["Допуск"], "±1%; 125 °C");
        assert!(!result.parameters.contains_key("Remove"));
        if data.starts_with(&[0xd0, 0xcf]) {
            let mut file = cfb::CompoundFile::open(Cursor::new(data)).unwrap();
            let mut opaque = vec![];
            file.open_stream("FutureData")
                .unwrap()
                .read_to_end(&mut opaque)
                .unwrap();
            assert_eq!(opaque, b"\0\xffopaque\0\x01");
        }
    }
}

#[test]
fn unchanged_records_and_unknown_streams_keep_their_payloads() {
    let text = [
        ASCII,
        b"\n|RECORD=999|OWNERPARTID=-1|OpaqueCase=Some Unknown DATA\n",
    ]
    .concat();
    let text = String::from_utf8(text)
        .unwrap()
        .replace("WEIGHT=15", "WEIGHT=16");
    let bytes = binary(text.as_bytes());
    let doc = import_schematic(&bytes, "eco.SchDoc").unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    let next = edit(
        &doc,
        Edit::ComponentProperties {
            id: c.id,
            designator: "R42".into(),
            comment: c.comment.clone(),
        },
    );
    let data = serialize(&next).unwrap();
    let mut before = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let mut after = cfb::CompoundFile::open(Cursor::new(data)).unwrap();
    let mut a = vec![];
    let mut b = vec![];
    before
        .open_stream("FileHeader")
        .unwrap()
        .read_to_end(&mut a)
        .unwrap();
    after
        .open_stream("FileHeader")
        .unwrap()
        .read_to_end(&mut b)
        .unwrap();
    let mut ra = vec![];
    let mut rb = vec![];
    framed("FileHeader", &a, &mut ra).unwrap();
    framed("FileHeader", &b, &mut rb).unwrap();
    assert_eq!(ra.len(), rb.len());
    for (i, (a, b)) in ra.iter().zip(&rb).enumerate() {
        if i != 6 {
            assert_eq!(a, b, "unexpected rewritten record {i}");
        }
    }
}

#[test]
fn native_net_names_and_existing_model_maps_are_preserved() {
    let text = [ASCII, b"\n|RECORD=44|OWNERINDEX=1\n|RECORD=45|OWNERINDEX=15|MODELNAME=0603|MODELTYPE=PCBLIB|ISCURRENT=T|DATAFILEENTITY0=original.PcbLib\n|RECORD=46|OWNERINDEX=16|MAP=retain\n"].concat();
    let doc = import_schematic(&binary(&text), "eco.SchDoc").unwrap();
    let s = doc.schematic.as_ref().unwrap();
    let c = &s.components[0];
    let power = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::PowerPort { .. }))
        .unwrap();
    let mut settings = c.settings.clone();
    settings.models[0].current = false;
    settings.models.push(ComponentModel {
        name: "0805".into(),
        kind: "PCBLIB".into(),
        current: true,
    });
    let next = edit(
        &doc,
        Edit::Batch(vec![
            Edit::Text {
                id: power.id,
                text: "DGND".into(),
            },
            Edit::ComponentDetails {
                id: c.id,
                description: c.description.clone(),
                settings,
            },
        ]),
    );
    let data = serialize(&next).unwrap();
    let read = import_schematic(&data, "eco.SchDoc").unwrap();
    let s = read.schematic.unwrap();
    assert_eq!(s.components[0].settings.models.len(), 2);
    assert!(!s.components[0].settings.models[0].current);
    assert!(s.components[0].settings.models[1].current);
    assert!(
        s.records
            .iter()
            .any(|r| r.fields.get("MAP").is_some_and(|v| v == "retain"))
    );
    assert!(s.records.iter().any(|r| {
        r.fields
            .get("DATAFILEENTITY0")
            .is_some_and(|v| v == "original.PcbLib")
    }));
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind, K::PowerPort{text,..} if text=="DGND"))
    );
}

#[test]
fn unsupported_edits_and_opaque_reindexing_leave_destination_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("target.SchDoc");
    std::fs::write(&destination, b"untouched").unwrap();
    let doc = import_schematic(&binary(ASCII), "eco.SchDoc").unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    let moved = edit(
        &doc,
        Edit::Transform {
            id: c.id,
            pivot: c.position,
            quarter_turns: 0,
            delta: Point::mm(1, 0),
        },
    );
    assert!(
        save(&destination, &moved)
            .unwrap_err()
            .contains("unsupported scene changes")
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"untouched");
    let multiline = edit(
        &doc,
        Edit::ParameterValue {
            component: Some(c.id),
            name: "Value".into(),
            value: Some("first\nsecond".into()),
        },
    );
    assert!(
        save(&destination, &multiline)
            .unwrap_err()
            .contains("multiline parameter")
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"untouched");
    let invalid = edit(
        &doc,
        Edit::ParameterValue {
            component: Some(c.id),
            name: "Value".into(),
            value: Some("bad|RECORD=1".into()),
        },
    );
    assert!(
        save(&destination, &invalid)
            .unwrap_err()
            .contains("separators")
    );
    assert_eq!(std::fs::read(&destination).unwrap(), b"untouched");
    let text = [
        ASCII,
        b"\n|RECORD=41|OWNERINDEX=1|NAME=Remove|TEXT=Old|ISHIDDEN=T\n|RECORD=999|UNKNOWNINDEX=17\n",
    ]
    .concat();
    let doc = import_schematic(&binary(&text), "eco.SchDoc").unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    let next = edit(
        &doc,
        Edit::ParameterValue {
            component: Some(c.id),
            name: "Remove".into(),
            value: None,
        },
    );
    assert!(save(&destination, &next).unwrap_err().contains("Opaque"));
    assert_eq!(std::fs::read(&destination).unwrap(), b"untouched");
}
