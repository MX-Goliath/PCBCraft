use super::altium::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::schematic::SchPrimitiveKind as K;
use eda_geom::Point;
use encoding_rs::WINDOWS_1251;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};

const ASCII: &[u8] = include_bytes!("../../../fixtures/schematic/minimal.SchDoc");

#[test]
fn contextual_properties_preserve_raw_parameters_visibility_fonts_models_and_history() {
    use eda_core::{
        History,
        schematic_edit::{Edit, Request},
    };
    let ascii = String::from_utf8(ASCII.to_vec())
        .unwrap()
        .replace("%UTF8%TEXT=Пример схемы", "%UTF8%TEXT=Первая~1Вторая")
        .replace("TEXT=100k|NAME=Comment", "TEXT==Resistance|NAME=Comment")
        + "\r\n|RECORD=41|OWNERINDEX=1|OWNERPARTID=-1|NAME=Resistance|TEXT=10k|ISHIDDEN=T|FONTID=1\r\n|RECORD=44|OWNERINDEX=1\r\n|RECORD=45|OWNERINDEX=16|MODELNAME=R0805|MODELTYPE=PCBLIB|ISCURRENT=T\r\n";
    let mut doc = import_schematic(ascii.as_bytes(), "properties.SchDoc").unwrap();
    let original = doc.clone();
    let s = doc.schematic.as_ref().unwrap();
    let owner = s.components[0].id;
    assert_eq!(s.components[0].comment, "10k");
    assert_eq!(s.components[0].settings.rotation, 1);
    assert_eq!(s.components[0].settings.models[0].name, "R0805");
    let hidden = s
        .parameter_objects
        .iter()
        .find(|p| p.presentation.name == "Resistance")
        .unwrap();
    let hidden_id = hidden.id;
    assert!(hidden.presentation.hidden);
    let mut shown = hidden.clone();
    shown.presentation.hidden = false;
    shown.presentation.show_name = true;
    let mut font = s.fonts[0].clone();
    font.bold = true;
    font.name = "Liberation Sans".into();
    let mut history = History::default();
    let document = doc.id;
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: 0,
                edit: Edit::Batch(vec![
                    Edit::PrimitiveProperties {
                        id: hidden_id,
                        value: Box::new(shown),
                    },
                    Edit::ParameterValue {
                        component: Some(owner),
                        name: "resistance".into(),
                        value: Some("22k".into()),
                    },
                    Edit::Font {
                        id: hidden_id,
                        font: font.clone(),
                    },
                ]),
            },
        )
        .unwrap();
    let s = doc.schematic.as_ref().unwrap();
    assert_eq!(s.components[0].comment, "22k");
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..}if text=="Первая\nВторая"))
    );
    assert_eq!(s.components[0].parameters.get("Resistance").unwrap(), "22k");
    assert!(!s.components[0].parameters.contains_key("resistance"));
    let p = s.primitive(hidden_id).unwrap();
    assert!(!p.presentation.hidden);
    assert!(matches!(&p.kind,K::Text{text,font:index,..} if text=="22k" && s.fonts[*index]==font));
    let comment = s
        .primitives
        .iter()
        .find(|p| p.text_role == Some(eda_core::schematic::TextRole::Comment))
        .unwrap();
    assert_eq!(
        comment.presentation.raw_text.as_deref(),
        Some("=Resistance")
    );
    assert!(matches!(&comment.kind,K::Text{text,..} if text=="22k"));
    assert_eq!(s.source, original.schematic.as_ref().unwrap().source);
    assert_eq!(s.records, original.schematic.as_ref().unwrap().records);
    let edited = doc.clone();
    history.undo(&mut doc);
    assert_eq!(doc, original);
    history.redo(&mut doc);
    assert_eq!(doc, edited);
    // Replacing an expression by its current displayed value still changes the raw property.
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: history.revision,
                edit: Edit::ComponentProperties {
                    id: owner,
                    designator: "R1".into(),
                    comment: "22k".into(),
                },
            },
        )
        .unwrap();
    assert_eq!(
        doc.schematic.as_ref().unwrap().components[0]
            .parameters
            .get("Comment")
            .unwrap(),
        "22k"
    );
    history.undo(&mut doc);
    assert_eq!(doc, edited);
    let mut invalid = doc.schematic.as_ref().unwrap().sheet.clone();
    invalid.options.snap_distance_nm = -1;
    assert!(
        history
            .schematic_edit(
                &mut doc,
                Request {
                    document,
                    expected_revision: history.revision,
                    edit: Edit::SheetProperties {
                        sheet: Box::new(invalid),
                        size: Point::mm(100, 100)
                    }
                }
            )
            .is_err()
    );
    assert_eq!(doc, edited);
    let encoded = serde_json::to_vec(&doc).unwrap();
    let restored: eda_core::Document = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(restored, doc);
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: history.revision,
                edit: Edit::ParameterValue {
                    component: Some(owner),
                    name: "RESISTANCE".into(),
                    value: None,
                },
            },
        )
        .unwrap();
    assert!(
        doc.schematic
            .as_ref()
            .unwrap()
            .primitive(hidden_id)
            .is_none()
    );
}

#[test]
fn edited_import_roundtrips_scene_without_changing_native_archive_or_source_records() {
    use eda_core::{
        History,
        schematic_edit::{Edit, Request, transform_point},
    };
    let mut doc = import_schematic(&compound(ASCII, true), "minimal.SchDoc").unwrap();
    let original = doc.clone();
    let s = doc.schematic.as_ref().unwrap();
    let id = s.components[0].id;
    let pivot = s.components[0].position;
    let delta = Point::mm(10, -5);
    let mut history = History::default();
    let document = doc.id;
    history
        .schematic_edit(
            &mut doc,
            Request {
                document,
                expected_revision: 0,
                edit: Edit::Batch(vec![
                    Edit::Transform {
                        id,
                        pivot,
                        quarter_turns: 1,
                        delta,
                    },
                    Edit::ComponentProperties {
                        id,
                        designator: "R42".into(),
                        comment: "47k".into(),
                    },
                ]),
            },
        )
        .unwrap();
    let s = doc.schematic.as_ref().unwrap();
    let before = original.schematic.as_ref().unwrap();
    assert!(s.source_modified);
    assert_eq!(s.source, before.source);
    assert_eq!(s.records, before.records);
    assert_eq!(
        s.components[0].position,
        transform_point(pivot, pivot, 1, delta).unwrap()
    );
    for p in before.primitives.iter().filter(|p| p.component == Some(id)) {
        if let K::Pin {
            position,
            orientation,
            ..
        } = p.kind
        {
            let next = s.primitives.iter().find(|v| v.id == p.id).unwrap();
            assert!(
                matches!(next.kind, K::Pin {position: actual, orientation: angle, ..}
                if actual == transform_point(position, pivot, 1, delta).unwrap() && angle == (orientation + 1) % 4)
            );
        }
    }
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind, K::Text{text,..} if text == "R42"))
    );
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind, K::Text{text,..} if text == "47k"))
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("edited.pcbcraft.json");
    crate::save(&path, &doc).unwrap();
    assert_eq!(crate::load(&path).unwrap(), doc);
    history.undo(&mut doc);
    assert_eq!(doc, original);
    history.redo(&mut doc);
    assert_eq!(crate::load(&path).unwrap(), doc);
}

#[test]
fn alternate_display_mode_keeps_shared_designator_and_comment_visible() {
    let mut text = std::str::from_utf8(ASCII)
        .unwrap()
        .replace("DISPLAYMODECOUNT=2", "DISPLAYMODECOUNT=2|DISPLAYMODE=1");
    text.push_str("\n|RECORD=41|OWNERINDEX=3|OWNERPARTID=-1|OWNERPARTDISPLAYMODE=1|NAME=Net Name|TEXT=Pin metadata\n");
    let doc = import_schematic(text.as_bytes(), "alternate.SchDoc").unwrap();
    let s = doc.schematic.unwrap();
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="R1"))
    );
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="100k"))
    );
    assert!(s.primitives.iter().any(|p| p.source_index == 7));
    assert!(!s.primitives.iter().any(|p| p.source_index == 2));
    assert!(
        !s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="Pin metadata"))
    );
}

#[test]
fn schematic_styles_keep_arrowheads_independent_pin_fonts_and_sheet_metrics() {
    let mut text = String::from_utf8(ASCII.to_vec())
        .unwrap()
        .replace("FONTIDCOUNT=1", "FONTIDCOUNT=2|SIZE2=14|FONTNAME2=Arial")
        .replace("SYSTEMFONT=1", "SYSTEMFONT=2");
    text.push_str("\n|RECORD=6|OWNERPARTID=-1|LOCATIONCOUNT=2|X1=10|Y1=20|X2=30|Y2=40|STARTLINESHAPE=1|ENDLINESHAPE=1|LINESHAPESIZE=2\n");
    text.push_str("|RECORD=2|OWNERPARTID=-1|LOCATION.X=20|LOCATION.Y=20|PINLENGTH=20|PINCONGLOMERATE=24|NAME=A|DESIGNATOR=1|NAME_CUSTOMFONTID=1|DESIGNATOR_CUSTOMFONTID=2\n");
    text.push_str("|RECORD=2|OWNERPARTID=-1|LOCATION.X=20|LOCATION.Y=30|PINLENGTH=20|PINCONGLOMERATE=24|NAME=B|DESIGNATOR=2\n");
    text.push_str(
        "|RECORD=3|OWNERPARTID=-1|LOCATION.X=10|LOCATION.Y=10|SYMBOL=11|SCALEFACTOR=10\n",
    );
    let doc = import_schematic(text.as_bytes(), "styles.SchDoc").unwrap();
    let s = doc.schematic.as_ref().unwrap();
    assert_eq!(s.sheet.margin_nm, 5_080_000);
    assert_eq!(s.sheet.system_font, 1);
    let arrow = s.primitives.iter().find(|p| p.end_shape == 1).unwrap();
    assert_eq!(
        (arrow.start_shape, arrow.end_shape, arrow.line_shape_size),
        (1, 1, 2)
    );
    assert!(
        matches!(&arrow.kind,K::Polyline{points,..} if points == &vec![Point{x:2_540_000,y:5_080_000},Point{x:7_620_000,y:10_160_000}])
    );
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Pin{name,font:0,number_font:Some(1),..} if name=="A"))
    );
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Pin{name,font:1,number_font:Some(1),..} if name=="B"))
    );
    assert!(!s.warnings.iter().any(|w| w.starts_with("Record type 3:")));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("styles.json");
    crate::save(&path, &doc).unwrap();
    assert_eq!(crate::load(&path).unwrap(), doc);
}

fn compound(text: &[u8], unknown_stream: bool) -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    let mut data = vec![];
    for line in text.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let length = (line.len() + 1) as u32;
        data.extend(length.to_le_bytes());
        data.extend(line);
        data.push(0);
    }
    file.create_stream("FileHeader")
        .unwrap()
        .write_all(&data)
        .unwrap();
    if unknown_stream {
        file.create_stream("FutureNativeData")
            .unwrap()
            .write_all(&[0, 0xff, 1, 2, 3, 0, 4])
            .unwrap();
    }
    file.into_inner().into_inner()
}

#[test]
fn ascii_and_cfb_import_preserve_ids_utf8_absolute_geometry_and_unknown_data() {
    let extended = [
        ASCII,
        b"\n|RECORD=999|OWNERPARTID=-1|OPAQUE=retained-future-data\n",
    ]
    .concat();
    let ascii = import_schematic(&extended, "minimal.SchDoc").unwrap();
    let bytes = compound(&extended, true);
    let binary = import_schematic(&bytes, "minimal.SchDoc").unwrap();
    assert_eq!(ascii.id, binary.id);
    let a = ascii.schematic.as_ref().unwrap();
    let b = binary.schematic.as_ref().unwrap();
    assert_eq!(a.components, b.components);
    assert_eq!(a.primitives, b.primitives);
    assert_eq!(b.components[0].designator, "R1");
    assert_eq!(b.components[0].comment, "100k");
    assert_eq!(b.fonts[0].height_nm, 2_540_000);
    assert_eq!(b.records.len(), 16);
    assert!(!b.primitives.iter().any(|p| p.source_index == 7)); // Other display mode.
    assert!(!b.primitives.iter().any(|p| p.source_index == 12)); // Hidden sheet parameter.
    assert!(
        b.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="Пример схемы"))
    );
    let pin = b
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::Pin { .. }))
        .unwrap();
    assert!(
        matches!(pin.kind,K::Pin{position,..} if position.x==27_940_000&&position.y==25_400_000)
    ); // Never rotate absolute child coordinates twice.
    assert!(b.warnings.iter().any(|w| w.contains("999")));
    assert_eq!(b.records[15].fields["OPAQUE"], "retained-future-data");
    assert_eq!(
        STANDARD
            .decode(&b.source.as_ref().unwrap().original_base64)
            .unwrap(),
        bytes
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("roundtrip.json");
    crate::save(&path, &binary).unwrap();
    assert_eq!(crate::load(&path).unwrap(), binary);
}

#[test]
fn project_resolves_relative_windows_paths_case_and_missing_references() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Sheets")).unwrap();
    std::fs::write(dir.path().join("Sheets").join("Схема.SchDoc"), ASCII).unwrap();
    let project = dir.path().join("test.PrjPcb");
    std::fs::write(&project,"[Design]\nVersion=1.0\n[Document1]\nDocumentPath=sheets\\Схема.schdoc\n[Document2]\nDocumentPath=missing.PcbDoc\n").unwrap();
    let p = load_project(&project).unwrap();
    assert_eq!(p.documents.len(), 2);
    assert!(p.documents[0].exists);
    assert!(!p.documents[1].exists);
    assert_eq!(p.documents[0].kind, "schdoc");
    assert!(p.warnings.iter().any(|w| w.contains("missing.PcbDoc")));
}

#[test]
fn damaged_framing_owners_numbers_and_coordinates_are_rejected_without_panics() {
    let bytes = compound(ASCII, false);
    assert!(import_schematic(&bytes[..100], "broken.SchDoc").is_err());
    for (a, b) in [
        ("OWNERINDEX=1", "OWNERINDEX=999999"),
        ("LOCATION.X=110", "LOCATION.X=invalid"),
        ("CURRENTPARTID=1", "CURRENTPARTID=9223372036854775807"),
        ("LOCATIONCOUNT=5", "LOCATIONCOUNT=100001"),
    ] {
        let text = std::str::from_utf8(ASCII).unwrap().replace(a, b);
        assert!(
            import_schematic(text.as_bytes(), "broken.SchDoc").is_err(),
            "{b}"
        );
    }
    let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
    file.create_stream("FileHeader")
        .unwrap()
        .write_all(&[100, 0, 0, 0, b'|'])
        .unwrap();
    assert!(
        import_schematic(&file.into_inner().into_inner(), "bad.SchDoc")
            .unwrap_err()
            .contains("Truncated")
    );
}

#[test]
fn native_source_can_never_be_overwritten_by_json_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.SchDoc");
    std::fs::write(&path, ASCII).unwrap();
    let doc = load_schematic(&path).unwrap();
    assert!(crate::save(&path, &doc).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), ASCII);
}

#[test]
fn quoted_parameter_indirection_is_recursive_case_insensitive_and_cycle_safe() {
    let local = BTreeMap::from([
        ("Comment".into(), "=\"VALUE\"".into()),
        ("Value".into(), "10 кОм".into()),
        ("Loop".into(), "=Loop".into()),
    ]);
    let sheet = BTreeMap::from([("Title".into(), "Sheet title".into())]);
    assert_eq!(parameter_text("=Comment", &local, &sheet), "10 кОм");
    assert_eq!(parameter_text("=title", &local, &sheet), "Sheet title");
    assert_eq!(parameter_text("=Loop", &local, &sheet), "=Loop");
    assert_eq!(parameter_text("=Undefined", &local, &sheet), "=Undefined");
}

#[test]
fn fractions_custom_large_sheet_and_legacy_utf8_override_survive_import() {
    let text = std::str::from_utf8(ASCII)
        .unwrap()
        .replace(
            "|RECORD=31|",
            "|RECORD=31|USECUSTOMSHEET=T|CUSTOMX=8000|CUSTOMY=5000|",
        )
        .replace(
            "LOCATION.X=110|LOCATION.Y=100",
            "LOCATION.X=-110|LOCATION.X_FRAC=-1|LOCATION.Y=100|LOCATION.Y_FRAC=1",
        );
    let (mut legacy, _, errors) = WINDOWS_1251.encode(&text);
    assert!(!errors);
    // Preserve an explicit UTF8 value alongside the legacy text in the same record.
    let marker = b"%UTF8%TEXT=";
    let pos = legacy
        .windows(marker.len())
        .position(|p| p == marker)
        .unwrap()
        + marker.len();
    let end = legacy[pos..].iter().position(|b| *b == b'\r').unwrap() + pos;
    legacy.to_mut().splice(pos..end, "Пример схемы".bytes());
    let doc = import_schematic(&legacy, "legacy.SchDoc").unwrap();
    assert_eq!(
        doc.size,
        Point {
            x: 2_032_000_000,
            y: 1_270_000_000
        }
    );
    let sch = doc.schematic.unwrap();
    assert!(
        sch.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Text{text,..} if text=="Пример схемы"))
    );
    assert!(sch.primitives.iter().any(
        |p| matches!(p.kind,K::Pin{position,..} if position==Point{x:-27_940_003,y:25_400_003})
    ));
}

#[test]
fn compressed_storage_image_is_preserved_and_truncation_is_rejected() {
    let png=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aRZkAAAAASUVORK5CYII=").unwrap();
    let mut compressor =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    compressor.write_all(&png).unwrap();
    let compressed = compressor.finish().unwrap();
    let name = b"C:\\images\\logo.png";
    let mut raw = vec![0xd0, name.len() as u8];
    raw.extend(name);
    raw.extend((compressed.len() as u32).to_le_bytes());
    raw.extend(compressed);
    let source=[ASCII,b"|RECORD=30|OWNERPARTID=-1|LOCATION.X=10|LOCATION.Y=10|CORNER.X=20|CORNER.Y=20|FILENAME=C:\\images\\logo.png\r\n"].concat();
    for truncated in [false, true] {
        let mut cfb = cfb::CompoundFile::open(Cursor::new(compound(&source, false))).unwrap();
        let mut storage = Vec::new();
        storage.extend(((raw.len() as u32) | 0x01000000).to_le_bytes());
        storage.extend(&raw);
        if truncated {
            storage.pop();
        }
        cfb.create_stream("Storage")
            .unwrap()
            .write_all(&storage)
            .unwrap();
        let result = import_schematic(&cfb.into_inner().into_inner(), "image.SchDoc");
        if truncated {
            assert!(result.is_err());
        } else {
            let sch = result.unwrap().schematic.unwrap();
            assert_eq!(
                STANDARD.decode(&sch.images["c:/images/logo.png"]).unwrap(),
                png
            );
            assert!(!sch.warnings.iter().any(|w| w.contains("unavailable")));
        }
    }
}

#[test]
fn utf16_project_document_paths_are_portable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("Схема.SchDoc"), ASCII).unwrap();
    let mut text = vec![0xff, 0xfe];
    text.extend(
        "[Design]\r\n[Document1]\r\nDocumentPath=Схема.SchDoc\r\n"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    let path = dir.path().join("unicode.PrjPcb");
    std::fs::write(&path, text).unwrap();
    assert!(load_project(&path).unwrap().documents[0].exists);
}
