use super::*;
use eda_core::schematic_edit::{self, Edit};

const ASCII: &[u8] = include_bytes!("../../../fixtures/schematic/minimal.SchDoc");

#[test]
fn omitted_text_justification_is_independent_of_rotation() {
    let mut text = ASCII.to_vec();
    for orientation in 0..4 {
        text.extend(format!("\n|RECORD=4|OWNERINDEX=0|LOCATION.X=400|LOCATION.Y=400|FONTID=1|TEXT=DefaultAnchor{orientation}|ORIENTATION={orientation}\n|RECORD=41|OWNERINDEX=0|LOCATION.X=500|LOCATION.Y=400|FONTID=1|NAME=ExplicitAnchor{orientation}|TEXT=Value|ORIENTATION={orientation}|JUSTIFICATION=8\n").as_bytes());
    }
    for bytes in [text.clone(), binary(&text)] {
        let doc = import_schematic(&bytes, "Anchors.SchDoc").unwrap();
        for p in &doc.schematic.as_ref().unwrap().primitives {
            if let K::Text {
                text,
                justification,
                ..
            } = &p.kind
            {
                if text.starts_with("DefaultAnchor") {
                    assert_eq!(*justification, 0);
                } else if p.presentation.name.starts_with("ExplicitAnchor") {
                    assert_eq!(*justification, 8);
                }
            }
        }
    }
}

#[test]
fn hidden_directive_parameters_keep_visibility_through_rotation_history_and_native_save() {
    use eda_core::schematic_edit::Request;
    // Same Small marker / hidden ClassName combination as the reported sheet.
    let text = [ASCII, b"\n|RECORD=43|OWNERPARTID=-1|LOCATION.X=400|LOCATION.Y=400|ORIENTATION=1|NAME=Parameter Set|STYLE=1\n"].concat();
    let owner_index = import_schematic(&text, "Hidden.SchDoc")
        .unwrap()
        .schematic
        .unwrap()
        .records
        .len()
        - 1;
    let children = format!(
        "|RECORD=41|OWNERINDEX={owner_index}|OWNERPARTID=-1|LOCATION.X=398|LOCATION.Y=390|FONTID=1|ISHIDDEN=T|NAME=ClassName|TEXT=ExamplePowerClass\n|RECORD=41|OWNERINDEX={owner_index}|OWNERPARTID=-1|LOCATION.X=410|LOCATION.Y=400|FONTID=1|NAME=Visible|TEXT=Shown\n"
    );
    let text = [text, children.into_bytes()].concat();
    for bytes in [text.clone(), binary(&text)] {
        let original = import_schematic(&bytes, "Hidden.SchDoc").unwrap();
        let s = original.schematic.as_ref().unwrap();
        let parent = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::ParameterSet { .. }))
            .unwrap();
        let parent_id = parent.id;
        let hidden = s
            .primitives
            .iter()
            .chain(&s.parameter_objects)
            .find(|p| {
                p.presentation.parent == Some(parent_id) && p.presentation.name == "ClassName"
            })
            .unwrap();
        assert!(hidden.presentation.hidden);
        let hidden_id = hidden.id;
        let mut doc = original.clone();
        let mut history = eda_core::History::default();
        for turn in 1..=4 {
            history
                .schematic_edit(
                    &mut doc,
                    Request {
                        document: original.id,
                        expected_revision: history.revision,
                        edit: Edit::Transform {
                            id: parent_id,
                            pivot: Point::mm(10, 10),
                            quarter_turns: 1,
                            delta: Point::default(),
                        },
                    },
                )
                .unwrap();
            let s = doc.schematic.as_ref().unwrap();
            assert!(s.primitive(hidden_id).unwrap().presentation.hidden);
            let copied = schematic_edit::extract(s, parent_id).unwrap();
            assert!(
                copied
                    .primitives
                    .iter()
                    .find(|p| p.id == hidden_id)
                    .unwrap()
                    .presentation
                    .hidden
            );
            let saved = serialize(&doc).unwrap();
            let reopened = import_schematic(&saved, "Hidden.SchDoc").unwrap();
            let s = reopened.schematic.as_ref().unwrap();
            let hidden = s
                .primitives
                .iter()
                .chain(&s.parameter_objects)
                .find(|p| p.presentation.name == "ClassName")
                .unwrap();
            assert!(hidden.presentation.hidden, "turn={turn}");
            assert!(matches!(hidden.kind, K::Text { orientation, .. } if orientation == turn % 4));
            let shown = s
                .primitives
                .iter()
                .find(|p| p.presentation.name == "Visible")
                .unwrap();
            assert!(!shown.presentation.hidden);
            // Copy/paste must retain visibility as well as typed parameter values.
            let duplicate = copied
                .fresh_copy()
                .transformed(Point::default(), 0, Point::mm(50, 0))
                .unwrap();
            let duplicate = edit(&doc, Edit::Insert(duplicate));
            assert_eq!(
                duplicate
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .chain(&duplicate.schematic.as_ref().unwrap().parameter_objects)
                    .filter(|p| p.presentation.name == "ClassName" && p.presentation.hidden)
                    .count(),
                2
            );
        }
        for _ in 0..4 {
            history.undo(&mut doc);
        }
        assert_eq!(doc, original);
        for _ in 0..4 {
            history.redo(&mut doc);
        }
        assert!(
            doc.schematic
                .as_ref()
                .unwrap()
                .primitive(hidden_id)
                .unwrap()
                .presentation
                .hidden
        );
    }
}

#[test]
fn new_native_sheet_saves_geometry_and_sheet_settings() {
    let doc = super::super::new_schematic("Sheet1.SchDoc").unwrap();
    let mut sheet = doc.schematic.as_ref().unwrap().sheet.clone();
    sheet.background = 0xfafafa;
    sheet.grid_visible = false;
    sheet.options.snap_enabled = false;
    let mut size = doc.size;
    sheet.set_standard_format(&mut size, 1);
    let next = edit(
        &doc,
        Edit::SheetProperties {
            sheet: Box::new(sheet),
            size,
        },
    );
    let next = edit(
        &next,
        Edit::Insert(schematic_edit::Fragment {
            primitives: vec![schematic_edit::primitive(
                K::Polyline {
                    points: vec![Point::mm(10, 10), Point::mm(30, 20)],
                    electrical: "Wire".into(),
                    closed: false,
                },
                None,
            )],
            ..Default::default()
        }),
    );
    let bytes = serialize(&next).unwrap();
    let read = import_schematic(&bytes, "Sheet1.SchDoc").unwrap();
    assert_eq!(read.size, next.size);
    assert_eq!(
        read.schematic.as_ref().unwrap().sheet,
        next.schematic.as_ref().unwrap().sheet
    );
    assert_eq!(read.schematic.as_ref().unwrap().primitives.len(), 1);
}

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
        b"|RECORD=225|OWNERPARTID=-1|INDEXINSHEET=19|LOCATIONCOUNT=4|X1=1|Y1=1|X2=1|Y2=2|X3=2|Y3=2|X4=2|Y4=1|CustomCase=untouched\0";
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
            parameters: Default::default(),
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
        parameters: Default::default(),
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
            .contains("unrepresented geometry")
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

#[test]
fn native_directives_round_trip_new_polygon_mask_parameters_rules_and_deletion() {
    use eda_core::schematic_directives::{primitive, rectangle};
    use eda_core::schematic_edit::Fragment;
    for bytes in [ASCII.to_vec(), binary(ASCII)] {
        let original = import_schematic(&bytes, "directives.SchDoc").unwrap();
        let points = vec![
            Point::mm(10, 10),
            Point::mm(20, 10),
            Point::mm(20, 20),
            Point::mm(15, 15),
            Point::mm(10, 20),
        ];
        let primitives = vec![
            primitive(K::Blanket {
                points: points.clone(),
                collapsed: false,
            }),
            primitive(K::CompileMask {
                a: Point::mm(30, 30),
                b: Point::mm(40, 40),
                collapsed: false,
            }),
            primitive(K::ParameterSet {
                position: Point::mm(10, 15),
                orientation: 0,
                name: "Clock".into(),
                style: 0,
                parameters: BTreeMap::from([
                    ("ClassName".into(), "CLK".into()),
                    ("CompClassName".into(), "Clock Components".into()),
                ]),
                rules: vec![BTreeMap::from([
                    ("RULEKIND".into(), "Width".into()),
                    ("MINLIMIT".into(), "10mil".into()),
                    ("MAXLIMIT".into(), "20mil".into()),
                    ("PREFEREDWIDTH".into(), "12mil".into()),
                ])],
            }),
        ];
        let inserted = edit(
            &original,
            Edit::Insert(Fragment {
                primitives,
                ..Default::default()
            }),
        );
        let saved = serialize(&inserted).unwrap();
        if saved.starts_with(&[0xd0, 0xcf]) {
            let mut file = cfb::CompoundFile::open(Cursor::new(saved.clone())).unwrap();
            let mut data = vec![];
            file.open_stream("Additional")
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            let len = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
            let text = String::from_utf8_lossy(&data[4..4 + len]);
            assert!(
                text.contains(
                    "HEADER=Protel for Windows - Schematic Capture Binary File Version 5.0"
                )
            );
            assert!(
                text.contains("WEIGHT=1"),
                "Additional must advertise the authored Blanket to Altium: {text}"
            );
        }

        let imported = import_schematic(&saved, "directives.SchDoc").unwrap();
        let s = imported.schematic.as_ref().unwrap();
        let blanket = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::Blanket { .. }))
            .unwrap();
        if let K::Blanket { points: read, .. } = &blanket.kind {
            assert!(
                read.iter()
                    .zip(&points)
                    .all(|(a, b)| (a.x - b.x).abs() <= 2 && (a.y - b.y).abs() <= 2)
            );
        }
        let mut replacement = blanket.clone();
        if let K::Blanket { points, collapsed } = &mut replacement.kind {
            *points = rectangle(Point::mm(5, 5), Point::mm(25, 25));
            *collapsed = true;
        }
        let resized = edit(
            &imported,
            Edit::PrimitiveProperties {
                id: blanket.id,
                value: Box::new(replacement),
            },
        );
        let resized = import_schematic(&serialize(&resized).unwrap(), "directives.SchDoc").unwrap();
        let mask = resized
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::CompileMask { .. }))
            .unwrap()
            .id;
        let removed = edit(&resized, Edit::Delete(mask));
        let removed = import_schematic(&serialize(&removed).unwrap(), "directives.SchDoc").unwrap();
        assert!(removed.schematic.as_ref().unwrap().compile_masks.is_empty());
        let set = removed
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::ParameterSet { .. }))
            .unwrap();
        let K::ParameterSet {
            parameters, rules, ..
        } = &set.kind
        else {
            panic!()
        };
        assert_eq!(parameters["ClassName"], "CLK");
        assert_eq!(rules[0]["PREFEREDWIDTH"], "12mil");
    }
}

#[test]
fn directive_typed_and_native_child_edits_save_without_duplicate_parameters() {
    use eda_core::{schematic_directives::primitive, schematic_edit::Fragment};
    for bytes in [ASCII.to_vec(), binary(ASCII)] {
        let original = import_schematic(&bytes, "fields.SchDoc").unwrap();
        let set = primitive(K::ParameterSet {
            position: Point::mm(10, 10),
            orientation: 0,
            name: "Constraints".into(),
            style: 0,
            parameters: BTreeMap::from([("ClassName".into(), "Clock".into())]),
            rules: vec![BTreeMap::from([
                ("RULEKIND".into(), "Width".into()),
                ("MINLIMIT".into(), "10mil".into()),
            ])],
        });
        let inserted = edit(
            &original,
            Edit::Insert(Fragment {
                primitives: vec![set],
                ..Default::default()
            }),
        );
        let mut doc = import_schematic(&serialize(&inserted).unwrap(), "fields.SchDoc").unwrap();
        let owner = doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::ParameterSet { .. }))
            .unwrap()
            .id;
        let mut child = doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| p.presentation.parent == Some(owner) && p.presentation.name == "ClassName")
            .unwrap()
            .clone();
        if let K::Text { text, .. } = &mut child.kind {
            *text = "Data".into();
        }
        child.presentation.raw_text = Some("Data".into());
        doc = edit(
            &doc,
            Edit::PrimitiveProperties {
                id: child.id,
                value: Box::new(child),
            },
        );
        let saved = serialize(&doc).unwrap();
        doc = import_schematic(&saved, "fields.SchDoc").unwrap();
        let mut set = doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::ParameterSet { .. }))
            .unwrap()
            .clone();
        if let K::ParameterSet {
            parameters, rules, ..
        } = &mut set.kind
        {
            assert_eq!(parameters["ClassName"], "Data");
            parameters.insert("CompClassName".into(), "Parts".into());
            rules[0].insert("MINLIMIT".into(), "12mil".into());
            rules.push(BTreeMap::from([
                ("RULEKIND".into(), "Clearance".into()),
                ("GAP".into(), "8mil".into()),
            ]));
        }
        doc = edit(
            &doc,
            Edit::PrimitiveProperties {
                id: set.id,
                value: Box::new(set),
            },
        );
        let saved = serialize(&doc).unwrap();
        let read = import_schematic(&saved, "fields.SchDoc").unwrap();
        let s = read.schematic.as_ref().unwrap();
        let set = s
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::ParameterSet { .. }))
            .unwrap();
        let K::ParameterSet {
            parameters, rules, ..
        } = &set.kind
        else {
            panic!()
        };
        assert_eq!(parameters.len(), 2);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["MINLIMIT"], "12mil");
        assert_eq!(
            s.records
                .iter()
                .filter(
                    |r| r.record_type == 41 && r.fields.n("OWNERINDEX") == set.source_index as i64
                )
                .count(),
            4
        );
        assert_eq!(serialize(&read).unwrap(), saved);
    }
}

#[test]
fn blanket_legacy_and_modern_border_widths_keep_all_four_choices() {
    let head = String::from_utf8(hierarchy_source()).unwrap();
    for (fields, width) in [
        ("|LINESTYLE=0", 254_000),
        ("|LINESTYLE=1", 508_000),
        ("|LINESTYLE=2", 1_016_000),
        ("", 101_600),
        ("|LINEWIDTH=0", 101_600),
        ("|LINEWIDTH=1", 254_000),
        ("|LINEWIDTH=2", 508_000),
        ("|LINEWIDTH=3", 1_016_000),
    ] {
        let text = format!(
            "{head}\n|RECORD=225|LOCATIONCOUNT=4|X1=1|Y1=1|X2=1|Y2=2|X3=2|Y3=2|X4=2|Y4=1{fields}\n"
        );
        let doc = import_schematic(&binary(text.as_bytes()), "width.SchDoc").unwrap();
        let blanket = doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::Blanket { .. }))
            .unwrap();
        assert_eq!(blanket.width_nm, width, "{fields}");
        for width in [101_600, 254_000, 508_000, 1_016_000] {
            let mut value = blanket.clone();
            value.width_nm = width;
            let next = edit(
                &doc,
                Edit::PrimitiveProperties {
                    id: blanket.id,
                    value: Box::new(value),
                },
            );
            let read = import_schematic(&serialize(&next).unwrap(), "width.SchDoc").unwrap();
            assert_eq!(
                read.schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .find(|p| matches!(p.kind, K::Blanket { .. }))
                    .unwrap()
                    .width_nm,
                width
            );
        }
    }
}

#[test]
fn authored_no_erc_and_differential_pair_directives_survive_native_copy() {
    for placement in [
        schematic_edit::Placement::NoErc,
        schematic_edit::Placement::DifferentialPair,
    ] {
        let doc = import_schematic(&binary(ASCII), "directive.SchDoc").unwrap();
        let f = schematic_edit::place(placement, Point::mm(100, 100), 1, "", "", 0).unwrap();
        let next = edit(&doc, Edit::Insert(f));
        let bytes = serialize(&next).unwrap();
        let read = import_schematic(&bytes, "directive.SchDoc").unwrap();
        assert_eq!(
            read.schematic.as_ref().unwrap().primitives.len(),
            next.schematic.as_ref().unwrap().primitives.len()
        );
    }
}

#[test]
fn blanket_line_style_ext_matches_native_solid_dashed_dotted_values() {
    for style in 0..=2 {
        let text = format!(
            "{}\n|RECORD=225|LOCATIONCOUNT=4|X1=1|Y1=1|X2=1|Y2=2|X3=2|Y3=2|X4=2|Y4=1|LINEWIDTH=2|LINESTYLEEXT={style}\n",
            String::from_utf8(hierarchy_source()).unwrap()
        );
        let doc = import_schematic(&binary(text.as_bytes()), "pattern.SchDoc").unwrap();
        let p = doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, K::Blanket { .. }))
            .unwrap();
        assert_eq!(p.line_style, style);
        for next in 0..=2 {
            let mut value = p.clone();
            value.line_style = next;
            let edited = edit(
                &doc,
                Edit::PrimitiveProperties {
                    id: p.id,
                    value: Box::new(value),
                },
            );
            let read = import_schematic(&serialize(&edited).unwrap(), "pattern.SchDoc").unwrap();
            let record = read
                .schematic
                .as_ref()
                .unwrap()
                .records
                .iter()
                .find(|r| r.record_type == 225)
                .unwrap();
            assert_eq!(record.fields.n("LINESTYLEEXT"), next as i64);
        }
    }
}
