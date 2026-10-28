use super::*;
use eda_core::schematic_edit::{self, Edit};

const ASCII: &[u8] = include_bytes!("../../../fixtures/schematic/minimal.SchDoc");

#[test]
fn modern_component_kind_overrides_legacy_and_round_trips_native_edits() {
    let text = String::from_utf8(ASCII.to_vec()).unwrap().replace(
        "|LIBREFERENCE=Resistor",
        "|COMPONENTKIND=0|COMPONENTKINDVERSION2=5|LIBREFERENCE=Resistor",
    );
    let doc = import_schematic(&binary(text.as_bytes()), "kind.SchDoc").unwrap();
    let c = &doc.schematic.as_ref().unwrap().components[0];
    assert_eq!(c.settings.kind, 5);
    let mut settings = c.settings.clone();
    settings.kind = 6;
    let next = edit(
        &doc,
        Edit::ComponentDetails {
            id: c.id,
            description: c.description.clone(),
            settings,
        },
    );
    let read = import_schematic(&serialize(&next).unwrap(), "kind.SchDoc").unwrap();
    assert_eq!(
        read.schematic.as_ref().unwrap().components[0].settings.kind,
        6
    );
    let record = read
        .schematic
        .as_ref()
        .unwrap()
        .records
        .iter()
        .find(|r| r.record_type == 1)
        .unwrap();
    assert_eq!(record.fields.get("COMPONENTKIND").unwrap(), "0");
    assert_eq!(record.fields.get("COMPONENTKINDVERSION2").unwrap(), "6");
}

fn hierarchy_source() -> Vec<u8> {
    let mut text = String::from_utf8(ASCII.to_vec())
        .unwrap()
        .lines()
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    text.push_str("\n|RECORD=15|LOCATION.X=100|LOCATION.Y=200|XSIZE=100|YSIZE=100|LINEWIDTH=1|COLOR=128|ISSOLID=T|AREACOLOR=8454016|UNIQUEID=SYMBOLAA\n|RECORD=16|OWNERINDEX=1|SIDE=0|DISTANCEFROMTOP=2|DISTANCEFROMTOP_FRAC1=500000|NAME=SIGNAL|IOTYPE=2|TEXTFONTID=1|TEXTCOLOR=128|COLOR=128|AREACOLOR=8454143|ARROWKIND=Block & Triangle|LINEWIDTH=1|UNIQUEID=ENTRYAAA\n|RECORD=32|OWNERINDEX=1|LOCATION.X=100|LOCATION.Y=210|TEXT=U_Child|FONTID=1\n|RECORD=33|OWNERINDEX=1|LOCATION.X=100|LOCATION.Y=220|TEXT=child.SchDoc|FONTID=1\n|RECORD=27|LOCATIONCOUNT=2|X1=70|Y1=175|X2=100|Y2=175|COLOR=8388608|LINEWIDTH=1\n");
    text.into_bytes()
}

#[test]
fn native_port_label_color_is_independent_of_border_and_survives_property_edits() {
    let text = [hierarchy_source(), b"\n|RECORD=18|LOCATION.X=50|LOCATION.Y=50|WIDTH=100|HEIGHT=10|NAME=P|IOTYPE=2|COLOR=128|TEXTCOLOR=16711680|ALIGNMENT=1|FONTID=1\n".to_vec()].concat();
    for bytes in [text.clone(), binary(&text)] {
        let original = import_schematic(&bytes, "port.SchDoc").unwrap();
        let port = original
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::Port { .. }))
            .unwrap();
        assert_eq!(port.color, 128);
        assert_eq!(port.presentation.text_color, Some(16711680));
        assert_eq!(port.presentation.alignment, Some(1));
        let mut replacement = port.clone();
        replacement.color = 255;
        replacement.presentation.text_color = Some(32768);
        replacement.presentation.alignment = Some(0);
        let changed = edit(
            &original,
            Edit::PrimitiveProperties {
                id: port.id,
                value: Box::new(replacement),
            },
        );
        let read = import_schematic(&serialize(&changed).unwrap(), "port.SchDoc").unwrap();
        let result = read
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::Port { .. }))
            .unwrap();
        assert_eq!(result.color, 255);
        assert_eq!(result.presentation.text_color, Some(32768));
        assert_eq!(result.presentation.alignment, Some(0));
    }
    let text = String::from_utf8(text)
        .unwrap()
        .replace("|TEXTCOLOR=16711680", "");
    let read = import_schematic(text.as_bytes(), "port.SchDoc").unwrap();
    assert_eq!(
        read.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::Port { .. }))
            .unwrap()
            .presentation
            .text_color,
        Some(0)
    );
}

#[test]
fn synchronization_preserves_additional_drawing_payloads_when_native_indices_shift() {
    use eda_core::schematic_sync::{self as sync, Destination, Signal};
    let mut file = cfb::CompoundFile::open(Cursor::new(binary(&hierarchy_source()))).unwrap();
    let payload =
        b"|RECORD=225|OWNERPARTID=-1|INDEXINSHEET=19|LOCATIONCOUNT=4|CustomCase=untouched\0";
    {
        let mut stream = file.create_stream("Additional").unwrap();
        stream
            .write_all(&(payload.len() as u32).to_le_bytes())
            .unwrap();
        stream.write_all(payload).unwrap();
    }
    let original = import_schematic(&file.into_inner().into_inner(), "sync.SchDoc").unwrap();
    let plan = sync::transfer(
        &original,
        Destination::Ports,
        &[Signal {
            id: Uuid::new_v4(),
            name: "NewPort".into(),
            io: 2,
            harness: String::new(),
        }],
        &[],
    )
    .unwrap();
    let changed = edit(&original, plan.edit_at(&original, Point::mm(1, 1)).unwrap());
    let saved = serialize(&changed).unwrap();
    let mut file = cfb::CompoundFile::open(Cursor::new(saved)).unwrap();
    let mut additional = vec![];
    file.open_stream("Additional")
        .unwrap()
        .read_to_end(&mut additional)
        .unwrap();
    assert_eq!(&additional[4..], payload);
    let reopened = import_schematic(&file.into_inner().into_inner(), "sync.SchDoc").unwrap();
    assert!(
        reopened
            .schematic
            .unwrap()
            .primitives
            .iter()
            .filter_map(Signal::read)
            .any(|s| s.name == "NewPort")
    );
}

#[test]
fn synchronized_entries_and_ports_can_be_inserted_updated_deleted_and_reopened_natively() {
    use eda_core::schematic_sync::{self as sync, Destination, Signal};
    for bytes in [hierarchy_source(), binary(&hierarchy_source())] {
        let original = import_schematic(&bytes, "sync.SchDoc").unwrap();
        let owner = original
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::SheetSymbol { .. }))
            .unwrap()
            .id;
        let signal = Signal {
            id: Uuid::new_v4(),
            name: "Новый_сигнал".into(),
            io: 3,
            harness: "Жгут".into(),
        };
        for destination in [Destination::Entries(owner), Destination::Ports] {
            let plan =
                sync::transfer(&original, destination, std::slice::from_ref(&signal), &[]).unwrap();
            let changed = edit(
                &original,
                plan.edit_at(&original, Point::mm(25, 35)).unwrap(),
            );
            let saved = serialize(&changed).unwrap();
            let read = import_schematic(&saved, "sync.SchDoc").unwrap();
            let created = read
                .schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .find(|p| sync::Signal::read(p).is_some_and(|s| s.name == signal.name))
                .unwrap();
            let info = Signal::read(created).unwrap();
            assert_eq!((info.io, info.harness.as_str()), (3, "Жгут"));
            if matches!(destination, Destination::Entries(_)) {
                let parent = created
                    .presentation
                    .parent
                    .and_then(|id| read.schematic.as_ref().unwrap().primitive(id))
                    .unwrap();
                assert!(matches!(parent.kind, K::SheetSymbol { .. }));
            }
            let mut renamed = info;
            renamed.name = "Changed".into();
            renamed.io = 1;
            renamed.harness = "Other".into();
            // The app keeps its original source archive after Save. A second
            // save must keep the authored object's native identity after rename.
            let authored = changed
                .schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .find(|p| Signal::read(p).is_some_and(|s| s.name == signal.name))
                .unwrap();
            let plan = sync::transfer(
                &changed,
                destination,
                std::slice::from_ref(&renamed),
                &[authored.id],
            )
            .unwrap();
            let saved_again = edit(&changed, plan.edit_at(&changed, Point::default()).unwrap());
            let again = import_schematic(&serialize(&saved_again).unwrap(), "sync.SchDoc").unwrap();
            assert_eq!(
                again
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .find(|p| Signal::read(p).is_some_and(|s| s.name == "Changed"))
                    .unwrap()
                    .id,
                created.id
            );
            let target = match destination {
                Destination::Entries(_) => {
                    Destination::Entries(created.presentation.parent.unwrap())
                }
                Destination::Ports => Destination::Ports,
            };
            let plan = sync::transfer(&read, target, &[renamed], &[created.id]).unwrap();
            let updated = edit(&read, plan.edit_at(&read, Point::default()).unwrap());
            let reopened = import_schematic(&serialize(&updated).unwrap(), "sync.SchDoc").unwrap();
            assert!(
                reopened
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .filter_map(Signal::read)
                    .any(|s| s.name == "Changed" && s.io == 1 && s.harness == "Other")
            );
            let deleted = edit(&read, Edit::Delete(created.id));
            let reopened = import_schematic(&serialize(&deleted).unwrap(), "sync.SchDoc").unwrap();
            assert_eq!(
                reopened.schematic.as_ref().unwrap().primitives.len(),
                original.schematic.as_ref().unwrap().primitives.len()
            );
        }
    }
}

#[test]
fn sheet_entry_native_units_ownership_and_binary_ascii_roundtrip() {
    for bytes in [hierarchy_source(), binary(&hierarchy_source())] {
        let doc = import_schematic(&bytes, "parent.SchDoc").unwrap();
        let s = doc.schematic.as_ref().unwrap();
        let symbol = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::SheetSymbol { .. }))
            .unwrap();
        let entry = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::SheetEntry { .. }))
            .unwrap();
        assert_eq!(entry.presentation.parent, Some(symbol.id));
        assert_eq!(
            schematic_edit::electrical_endpoint(entry),
            Some(Point {
                x: 25_400_000,
                y: 44_450_000
            })
        );
        assert_eq!(serialize(&doc).unwrap(), bytes);
        let moved = edit(
            &doc,
            Edit::Drag {
                ids: vec![symbol.id],
                pivot: Point::default(),
                quarter_turns: 0,
                delta: Point::mm(5, 5),
            },
        );
        let written = serialize(&moved).unwrap();
        let read = import_schematic(&written, "moved.SchDoc").unwrap();
        let s = read.schematic.as_ref().unwrap();
        let entry = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::SheetEntry { .. }))
            .unwrap();
        let endpoint = schematic_edit::electrical_endpoint(entry).unwrap();
        assert!(s.primitives.iter().any(|p|matches!(&p.kind,K::Polyline{electrical,points,..} if electrical=="Wire" && points.windows(2).any(|e|endpoint.on_segment(e[0],e[1])))));
        let deleted = edit(&doc, Edit::Delete(symbol.id));
        let read = import_schematic(&serialize(&deleted).unwrap(), "deleted.SchDoc").unwrap();
        assert!(
            !read
                .schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .any(
                    |p| matches!(p.kind, K::SheetSymbol { .. } | K::SheetEntry { .. })
                        || p.presentation.parent.is_some()
                )
        );
    }
}

#[test]
fn sheet_symbol_properties_and_clipboard_keep_owned_entries_and_names() {
    let doc = import_schematic(&hierarchy_source(), "parent.SchDoc").unwrap();
    let symbol = doc
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::SheetSymbol { .. }))
        .unwrap();
    let f = schematic_edit::extract(doc.schematic.as_ref().unwrap(), symbol.id).unwrap();
    assert_eq!(f.primitives.len(), 4);
    let copy = f.fresh_copy();
    let body = copy
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::SheetSymbol { .. }))
        .unwrap();
    assert_ne!(body.id, symbol.id);
    assert!(
        copy.primitives
            .iter()
            .filter(|p| p.id != body.id)
            .all(|p| p.presentation.parent == Some(body.id))
    );
    let mut value = symbol.clone();
    let K::SheetSymbol {
        designator,
        filename,
        ..
    } = &mut value.kind
    else {
        unreachable!()
    };
    *designator = "U_New".into();
    *filename = "новый.SchDoc".into();
    let next = edit(
        &doc,
        Edit::PrimitiveProperties {
            id: symbol.id,
            value: Box::new(value),
        },
    );
    let saved = serialize(&next).unwrap();
    let read = import_schematic(&saved, "edited.SchDoc").unwrap();
    assert!(read.schematic.as_ref().unwrap().primitives.iter().any(|p|matches!(&p.kind,K::SheetSymbol{filename,designator,..} if filename=="новый.SchDoc" && designator=="U_New")));
}

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
