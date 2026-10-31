use super::*;
use eda_core::{
    History,
    schematic_edit::{self as edit, Edit, Request},
    schematic_harness,
};
fn source() -> Vec<u8> {
    let mut file = cfb::CompoundFile::create(Cursor::new(vec![])).unwrap();
    for (name, records) in [
        (
            "FileHeader",
            vec![
                "|HEADER=Protel for Windows - Schematic Capture Binary File Version 5.0|Weight=1|UniqueID=HARNESSTEST",
                "|RECORD=31|FontIdCount=1|Size1=10|FontName1=Times New Roman|SnapGridSize=10|VisibleGridSize=10|SnapGridOn=T|SystemFont=1",
            ],
        ),
        (
            "Additional",
            vec![
                "|HEADER=Protel for Windows - Schematic Capture Binary File Version 5.0|Weight=4",
                "|RECORD=215|OwnerPartId=-1|Location.X=400|Location.Y=540|XSize=130|YSize=160|LineWidth=1|Color=13213327|AreaColor=16511725|PrimaryConnectionPosition=80|HarnessConnectorSide=1|UniqueID=CONNECTR",
                "|RECORD=216|OwnerIndex=0|OwnerIndexAdditionalList=T|OwnerPartId=-1|DistanceFromTop=1|Color=7354880|AreaColor=8454143|TextColor=7354880|TextFontID=1|TextStyle=Full|Name=RX|UniqueID=ENTRYAAA",
                "|RECORD=217|OwnerIndex=0|OwnerIndexAdditionalList=T|OwnerPartId=-1|Location.X=400|Location.Y=540|Color=8388608|FontID=1|Text=Serial|NotAutoPosition=T|UniqueID=TYPEAAAA",
                "|RECORD=218|OwnerPartId=-1|LineWidth=2|Color=13213327|LocationCount=2|X1=530|Y1=460|X2=600|Y2=460|UniqueID=PATHAAAA",
            ],
        ),
    ] {
        let mut stream = file.create_stream(name).unwrap();
        for r in records {
            let b = [r.as_bytes(), &[0]].concat();
            stream.write_all(&(b.len() as u32).to_le_bytes()).unwrap();
            stream.write_all(&b).unwrap();
        }
    }
    file.into_inner().into_inner()
}
fn change(doc: &mut Document, edit: Edit) {
    let mut history = History::default();
    history
        .schematic_edit(
            doc,
            Request {
                document: doc.id,
                expected_revision: 0,
                edit,
            },
        )
        .unwrap();
}
#[test]
fn native_harness_additional_owners_geometry_and_unchanged_bytes() {
    let bytes = source();
    let d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    let s = d.schematic.as_ref().unwrap();
    assert_eq!(s.primitives.len(), 4);
    assert!(s.warnings.is_empty());
    let c = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::HarnessConnector { .. }))
        .unwrap();
    let e = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::HarnessEntry { .. }))
        .unwrap();
    assert_eq!(e.presentation.parent, Some(c.id));
    assert_eq!(c.presentation.harness_type, "Serial");
    assert_eq!(serialize(&d).unwrap(), bytes);
}
#[test]
fn native_harness_edit_insert_delete_reopen_and_repeat_save() {
    let bytes = source();
    let mut d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    let s = d.schematic.as_ref().unwrap();
    let mut c = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::HarnessConnector { .. }))
        .unwrap()
        .clone();
    let id = c.id;
    c.presentation.harness_type = "Serial2".into();
    if let K::HarnessConnector { b, .. } = &mut c.kind {
        b.y -= 2_540_000;
    }
    change(
        &mut d,
        Edit::PrimitiveProperties {
            id,
            value: Box::new(c),
        },
    );
    let bytes = serialize(&d).unwrap();
    let mut d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    assert_eq!(serialize(&d).unwrap(), bytes);
    let s = d.schematic.as_ref().unwrap();
    let c = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::HarnessConnector { .. }))
        .unwrap();
    assert_eq!(c.presentation.harness_type, "Serial2");
    let e = schematic_harness::entry(
        s,
        c.id,
        Point {
            x: 110_000_000,
            y: 130_000_000,
        },
        "TX",
    )
    .unwrap();
    change(
        &mut d,
        Edit::Insert(edit::Fragment {
            primitives: vec![e],
            ..Default::default()
        }),
    );
    let bytes = serialize(&d).unwrap();
    let mut d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    assert_eq!(
        d.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .filter(|p| matches!(p.kind, K::HarnessEntry { .. }))
            .count(),
        2
    );
    let id = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::HarnessConnector { .. }))
        .unwrap()
        .id;
    change(&mut d, Edit::Delete(id));
    let bytes = serialize(&d).unwrap();
    let d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    assert_eq!(d.schematic.as_ref().unwrap().primitives.len(), 1);
}
#[test]
fn newly_authored_connector_and_entries_save_to_additional() {
    let mut d = import_schematic(&source(), "Harness.SchDoc").unwrap();
    let mut f = schematic_harness::connector(
        Point {
            x: 10_160_000,
            y: 50_800_000,
        },
        Point {
            x: 35_560_000,
            y: 25_400_000,
        },
        0,
        "Other",
    )
    .unwrap();
    let mut scratch = d.schematic.clone().unwrap();
    scratch.primitives.extend(f.primitives.clone());
    let mut e = schematic_harness::entry(
        &scratch,
        f.primitives[0].id,
        Point {
            x: 12_700_000,
            y: 48_260_000,
        },
        "A",
    )
    .unwrap();
    // Properties/Add uses the general authored-object sync default.
    e.presentation.allow_sync = false;
    f.primitives.push(e);
    change(&mut d, Edit::Insert(f));
    let bytes = serialize(&d).unwrap();
    let d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    assert_eq!(
        d.schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .filter(|p| matches!(p.kind, K::HarnessConnector { .. }))
            .count(),
        2
    );
}

#[test]
fn native_harness_survives_fileheader_insertion_hidden_type_and_font_edit() {
    let mut d = import_schematic(&source(), "Harness.SchDoc").unwrap();
    let mut f = edit::place(
        edit::Placement::NetLabel,
        Point {
            x: 200_000_000,
            y: 100_000_000,
        },
        0,
        "",
        "SIGNAL",
        0,
    )
    .unwrap();
    f.primitives.push(edit::primitive(
        K::Polyline {
            points: vec![
                Point {
                    x: 200_000_000,
                    y: 100_000_000,
                },
                Point {
                    x: 210_000_000,
                    y: 100_000_000,
                },
            ],
            closed: false,
            electrical: "Wire".into(),
        },
        None,
    ));
    change(&mut d, Edit::Insert(f));
    let s = d.schematic.as_ref().unwrap();
    let entry = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::HarnessEntry { .. }))
        .unwrap()
        .id;
    let mut label = s
        .primitives
        .iter()
        .find(|p| p.presentation.record_type == 217)
        .unwrap()
        .clone();
    label.presentation.hidden = true;
    change(
        &mut d,
        Edit::Batch(vec![
            Edit::PrimitiveProperties {
                id: label.id,
                value: Box::new(label),
            },
            Edit::Font {
                id: entry,
                font: SchFont {
                    name: "Arial".into(),
                    height_nm: 3_048_000,
                    bold: true,
                    italic: false,
                    underline: false,
                },
            },
        ]),
    );
    let bytes = serialize(&d).unwrap();
    let d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    let s = d.schematic.as_ref().unwrap();
    assert!(
        s.primitives
            .iter()
            .any(|p| p.presentation.record_type == 217 && p.presentation.hidden)
    );
    assert_eq!(s.fonts.len(), 2);
    assert_eq!(serialize(&d).unwrap(), bytes);
}

#[test]
fn definition_sidecar_preserves_locked_entries_and_native_nested_syntax() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Harness.SchDoc");
    let sidecar = path.with_extension("Harness");
    std::fs::write(&sidecar, "LOCKED;Unused=A,{Child:Logic}\r\n").unwrap();
    let d = import_schematic(&source(), "Harness.SchDoc").unwrap();
    save(&path, &d).unwrap();
    let text = std::fs::read_to_string(sidecar).unwrap();
    assert!(text.contains("LOCKED;Unused=A,{Child:Logic}"));
    assert!(text.contains("Serial=RX"));
    assert_eq!(
        eda_io_load_for_test(&path)
            .schematic
            .unwrap()
            .primitives
            .len(),
        4
    );
}
fn eda_io_load_for_test(path: &Path) -> Document {
    crate::load(path).unwrap()
}

#[test]
fn predefined_harness_port_and_signal_save_and_reopen() {
    let mut d = import_schematic(&source(), "Harness.SchDoc").unwrap();
    let s = d.schematic.as_ref().unwrap();
    let at = Point {
        x: 152_400_000,
        y: 116_840_000,
    };
    let mut f = edit::place(
        edit::Placement::Port,
        at,
        0,
        "",
        "HarnessPort",
        s.sheet.system_font,
    )
    .unwrap();
    f.primitives[0].presentation.harness_type = "Serial".into();
    f.primitives.push(edit::primitive(
        K::Polyline {
            points: vec![
                at,
                Point {
                    x: 177_800_000,
                    y: 116_840_000,
                },
            ],
            closed: false,
            electrical: "Signal Harness".into(),
        },
        None,
    ));
    change(&mut d, Edit::Insert(f));
    let bytes = serialize(&d).unwrap();
    let d = import_schematic(&bytes, "Harness.SchDoc").unwrap();
    assert!(d.schematic.as_ref().unwrap().primitives.iter().any(
        |p| matches!(&p.kind,K::Port{name,..} if name=="HarnessPort")
            && p.presentation.harness_type == "Serial"
    ));
    assert_eq!(serialize(&d).unwrap(), bytes);
}
