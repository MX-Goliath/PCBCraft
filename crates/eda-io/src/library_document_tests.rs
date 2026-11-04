use crate::library_documents as io;
use eda_core::{
    Document, DocumentKind, library_edit as lib, pcb, schematic as sch, schematic_edit as edit,
};
use eda_geom::Point;

fn symbol() -> Document {
    let mut doc = lib::new(DocumentKind::Schematic, "Parts.SchLib");
    doc = lib::preview(
        &doc,
        &lib::Edit::Metadata {
            name: "Test".into(),
            designator: "U?".into(),
            comment: "=Value".into(),
            description: "Test symbol".into(),
            parameters: vec![("Value".into(), "10k".into())],
        },
    )
    .unwrap();
    let id = doc.schematic.as_ref().unwrap().components[0].id;
    let pin = edit::primitive(
        sch::SchPrimitiveKind::Pin {
            position: Point::mm(-2, 0),
            length_nm: 2_540_000,
            orientation: 2,
            name: "Вход".into(),
            number: "1".into(),
            show_name: true,
            show_number: true,
            font: 0,
            number_font: None,
            electrical: 0,
            outer_symbol: 0,
            inner_symbol: 0,
        },
        Some(id),
    );
    lib::preview(&doc, &lib::Edit::Pin(Box::new(pin))).unwrap()
}
fn footprint() -> Document {
    let mut doc = lib::new(DocumentKind::Board, "Parts.PcbLib");
    doc = lib::preview(&doc, &lib::Edit::Rename("LAND".into())).unwrap();
    let b = doc.pcb.as_mut().unwrap();
    for (n, x) in [("A", -1), ("B", 1)] {
        let mut pad = pcb::Item::new(
            pcb::TOP,
            None,
            pcb::Shape::Pad {
                position: Point::mm(x, 0),
                size: Point::mm(1, 1),
                rotation: 0.,
                shape: pcb::PadShape::Rectangle,
                hole: 0,
                plated: true,
                name: n.into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        pad.component = Some(0);
        b.items.push(pad);
    }
    doc
}
#[test]
fn authored_schlib_roundtrip_preserves_unicode_pins_parameters_and_all_variants() {
    let mut doc = symbol();
    doc = lib::preview(&doc, &lib::Edit::AddPart).unwrap();
    doc = lib::preview(&doc, &lib::Edit::AddDisplayMode).unwrap();
    doc = lib::preview(&doc, &lib::Edit::Duplicate("Second".into())).unwrap();
    let bytes = io::serialize(&doc).unwrap();
    let loaded = io::import(&bytes, "Parts.SchLib", false).unwrap();
    assert_eq!(lib::members(&loaded).count(), 8);
    for m in lib::members(&loaded) {
        let c = &m.schematic.as_ref().unwrap().components[0];
        assert_eq!(c.settings.part_count, 2);
        assert_eq!(c.comment, "10k");
        assert_eq!(c.description, "Test symbol");
        assert!(c.designator.starts_with("U?"));
    }
    let base = lib::members(&loaded)
        .find(|m| lib::name(m) == "Test" && lib::variant(m) == (1, 0))
        .unwrap();
    assert!(base.schematic.as_ref().unwrap().primitives.iter().any(|p|matches!(&p.kind,sch::SchPrimitiveKind::Pin{name,number,electrical:0,..}if name=="Вход"&&number=="1")));
    assert_eq!(
        io::serialize(&loaded).unwrap(),
        bytes,
        "unmodified imported container must be byte identical"
    );
}
#[test]
fn authored_pcblib_and_empty_footprints_reopen_with_local_geometry() {
    let footprint = lib::preview(
        &footprint(),
        &lib::Edit::FootprintMetadata {
            name: "LAND".into(),
            description: "Two terminals".into(),
            height_nm: 1_200_000,
            parameters: vec![("Package".into(), "Корпус".into())],
        },
    )
    .unwrap();
    for doc in [footprint, lib::new(DocumentKind::Board, "Empty.PcbLib")] {
        let bytes = io::serialize(&doc).unwrap();
        let loaded = io::import(&bytes, "Parts.PcbLib", true).unwrap();
        let actual = loaded.pcb.as_ref().unwrap();
        assert_eq!(actual.items.len(), doc.pcb.as_ref().unwrap().items.len());
        for (a, b) in actual.items.iter().zip(&doc.pcb.as_ref().unwrap().items) {
            // Native PCB coordinates use a 2.54 nm quantum; equal-size stacks
            // are expanded to explicit top/middle/bottom entries on import.
            let pcb::Shape::Pad {
                position: a,
                size: sa,
                name: na,
                bottom_size: ba,
                middle_size: ma,
                ..
            } = &a.shape
            else {
                panic!("Expected pad")
            };
            let pcb::Shape::Pad {
                position: b,
                size: sb,
                name: nb,
                ..
            } = &b.shape
            else {
                panic!("Expected pad")
            };
            assert!((a.x - b.x).abs() <= 2 && (a.y - b.y).abs() <= 2);
            assert!((sa.x - sb.x).abs() <= 2 && (sa.y - sb.y).abs() <= 2);
            assert_eq!(na, nb);
            assert_eq!(*ba, Some(*sa));
            assert_eq!(*ma, Some(*sa));
        }
        assert_eq!(actual.components[0].position, Point::default());
        assert_eq!(
            actual.components[0].parameters,
            doc.pcb.as_ref().unwrap().components[0].parameters
        );
        assert_eq!(io::serialize(&loaded).unwrap(), bytes);
    }
}

#[test]
fn edited_import_keeps_unknown_symbol_properties_and_unrelated_streams() {
    use std::io::{Cursor, Read, Write};
    let bytes = io::serialize(&symbol()).unwrap();
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    file.create_stream("VendorMetadata")
        .unwrap()
        .write_all(b"opaque metadata")
        .unwrap();
    let mut data = vec![];
    file.open_stream("Test/Data")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    let size = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    let mut root = data[4..4 + size - 1].to_vec();
    root.extend(b"|VENDOR_PROPERTY=keep me\0");
    let data = [
        (root.len() as u32).to_le_bytes().to_vec(),
        root,
        data[4 + size..].to_vec(),
    ]
    .concat();
    file.create_stream("Test/Data")
        .unwrap()
        .write_all(&data)
        .unwrap();
    file.flush().unwrap();
    let imported = io::import(&file.into_inner().into_inner(), "Parts.SchLib", false).unwrap();
    let changed = lib::preview(&imported, &lib::Edit::Rename("Renamed".into())).unwrap();
    let bytes = io::serialize(&changed).unwrap();
    let read = io::import(&bytes, "Parts.SchLib", false).unwrap();
    assert_eq!(
        read.schematic.as_ref().unwrap().records[1].fields["VENDOR_PROPERTY"],
        "keep me"
    );
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
    let mut vendor = vec![];
    file.open_stream("VendorMetadata")
        .unwrap()
        .read_to_end(&mut vendor)
        .unwrap();
    assert_eq!(vendor, b"opaque metadata");
}
#[test]
fn integrated_compile_checks_maps_and_failed_saves_keep_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("Parts.IntLib");
    let mut symbol = symbol();
    let footprint = footprint();
    symbol = lib::preview(
        &symbol,
        &lib::Edit::LinkFootprint {
            name: "LAND".into(),
            library: "Parts.PcbLib".into(),
            map: vec![("1".into(), vec!["A".into(), "B".into()])],
        },
    )
    .unwrap();
    io::compile_integrated(&path, &symbol, &[&footprint]).unwrap();
    let imported = crate::altium::library::load(&path).unwrap();
    assert_eq!(imported.symbols.len(), 1);
    assert_eq!(imported.embedded_footprints, Some(true));
    let model = &imported.symbols[0].variants[0].fragment.components[0]
        .settings
        .models[0];
    assert_eq!(model.parameters["PINMAP:1"], "A,B");
    let before = std::fs::read(&path).unwrap();
    assert!(io::compile_integrated(&path, &symbol, &[]).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let path = directory.path().join("Parts.SchLib");
    crate::save(&path, &symbol).unwrap();
    let before = std::fs::read(&path).unwrap();
    let mut bad = symbol.clone();
    let id = bad.schematic.as_ref().unwrap().components[0].id;
    bad.schematic
        .as_mut()
        .unwrap()
        .primitives
        .push(edit::primitive(
            sch::SchPrimitiveKind::Unsupported {
                a: Point::default(),
                b: Point::mm(1, 1),
                description: "Unrepresented native graphic".into(),
            },
            Some(id),
        ));
    assert!(crate::save(&path, &bad).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    crate::save(&path, &symbol).unwrap();
    assert_eq!(
        std::fs::read(directory.path().join("Parts.SchLib.bak")).unwrap(),
        before
    );
}
