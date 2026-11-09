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
#[test]
fn native_pin_symbol_width_roundtrips_independently_of_the_pin_line() {
    for width in [101_600, 254_000] {
        let mut doc = symbol();
        let mut pin = doc
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, sch::SchPrimitiveKind::Pin { .. }))
            .unwrap()
            .clone();
        pin.width_nm = 101_600;
        pin.presentation.pin_symbol_width_nm = Some(width);
        doc = lib::preview(&doc, &lib::Edit::Pin(Box::new(pin))).unwrap();
        let read = io::import(&io::serialize(&doc).unwrap(), "PinWidth.SchLib", false).unwrap();
        let pin = read
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(p.kind, sch::SchPrimitiveKind::Pin { .. }))
            .unwrap();
        assert_eq!(pin.width_nm, 101_600);
        assert_eq!(pin.presentation.pin_symbol_width_nm, Some(width));
    }
}

#[test]
fn library_update_roundtrips_rotated_native_instances_and_local_parameter_choices() {
    use eda_core::library_update as update;
    let folder = tempfile::tempdir().unwrap();
    let library_path = folder.path().join("Parts.SchLib");
    let mut library = symbol();
    let s = library.schematic.as_mut().unwrap();
    s.primitives.insert(
        0,
        edit::primitive(
            sch::SchPrimitiveKind::Rectangle {
                a: Point::mm(-2, -2),
                b: Point::mm(2, 2),
                radius: Point::default(),
            },
            Some(s.components[0].id),
        ),
    );
    crate::save(&library_path, &library).unwrap();
    let baseline = crate::altium::library::load(&library_path).unwrap();
    assert_eq!(
        baseline.symbols[0].variants[0]
            .fragment
            .primitives
            .iter()
            .filter(|p| p.text_role == Some(sch::TextRole::Comment))
            .count(),
        1,
        "An authored Comment must not also create a hidden duplicate parameter"
    );
    let mut sheet = crate::altium::new_schematic("Sheet.SchDoc").unwrap();
    for (x, turns) in [(30, 0), (60, 1)] {
        let mut f = baseline.symbols[0]
            .instantiate(1, 0, sheet.schematic.as_ref().unwrap())
            .unwrap();
        f.components[0].settings.source = "Parts.SchLib".into();
        f.components[0].settings.designator_locked = true;
        f.components[0].settings.part_locked = true;
        f = f
            .transformed(Point::default(), turns, Point::mm(x, 40))
            .unwrap();
        sheet = edit::preview(&sheet, &edit::Edit::Insert(f)).unwrap();
    }
    let path = folder.path().join("Sheet.SchDoc");
    crate::save(&path, &sheet).unwrap();
    sheet = crate::load(&path).unwrap();
    let old = sheet.schematic.as_ref().unwrap().clone();
    let definition = &baseline.symbols[0].variants[0].fragment;
    let mut definition = definition.clone();
    definition.components[0].description = "Updated definition".into();
    definition.components[0]
        .parameters
        .insert("Value".into(), "12k".into());
    definition.components[0]
        .parameters
        .insert("Vendor".into(), "Example".into());
    for p in &mut definition.primitives {
        if let sch::SchPrimitiveKind::Rectangle { b, .. } = &mut p.kind {
            b.x = 3_000_000;
        }
        if let sch::SchPrimitiveKind::Pin { name, .. } = &mut p.kind {
            *name = "Updated pin".into();
        }
    }
    let changes = old
        .components
        .iter()
        .enumerate()
        .map(|(n, c)| {
            let mut options = update::Options {
                source_library: Some("Updated.SchLib".into()),
                ..Default::default()
            };
            if n == 1 {
                options.excluded_parameters.insert("Value".into());
            }
            update::Update {
                component: c.id,
                definition: definition.clone(),
                options,
            }
        })
        .collect::<Vec<_>>();
    let next = update::preview(&sheet, &changes).unwrap();
    crate::save(&path, &next).unwrap();
    let loaded = crate::load(&path).unwrap();
    let s = loaded.schematic.as_ref().unwrap();
    for (n, c) in s.components.iter().enumerate() {
        assert_eq!(
            s.records
                .iter()
                .filter(|r| r.record_type == 41
                    && r.fields.get("OWNERINDEX") == Some(&c.source_index.to_string())
                    && r.fields
                        .get("NAME")
                        .is_some_and(|name| name.eq_ignore_ascii_case("Vendor")))
                .count(),
            1,
            "A copied hidden parameter presentation must not create another scalar record"
        );
        assert_eq!(
            s.records
                .iter()
                .filter(|r| r.record_type == 41
                    && r.fields.get("OWNERINDEX") == Some(&c.source_index.to_string())
                    && r.fields
                        .get("NAME")
                        .is_some_and(|name| name.eq_ignore_ascii_case("Comment")))
                .count(),
            1
        );
        let owned = s
            .primitives
            .iter()
            .filter(|p| p.component == Some(c.id))
            .collect::<Vec<_>>();
        let rectangle = owned
            .iter()
            .position(|p| matches!(p.kind, sch::SchPrimitiveKind::Rectangle { .. }))
            .unwrap();
        let pin = owned
            .iter()
            .position(|p| matches!(p.kind, sch::SchPrimitiveKind::Pin { .. }))
            .unwrap();
        assert!(
            rectangle < pin,
            "The filled body must stay behind the pin names in the native file"
        );
        assert_eq!(c.position, old.components[n].position);
        assert_eq!(c.designator, old.components[n].designator);
        assert_eq!(c.settings.rotation, old.components[n].settings.rotation);
        assert_eq!(c.settings.source, "Updated.SchLib");
        assert!(!c.settings.designator_locked);
        assert!(!c.settings.part_locked);
        assert_eq!(c.description, "Updated definition");
        assert_eq!(c.parameters["Value"], if n == 0 { "12k" } else { "10k" });
        assert_eq!(c.parameters["Vendor"], "Example");
        let uid = |scene: &sch::Schematic, index: u32| {
            scene.records[index as usize]
                .fields
                .get("UNIQUEID")
                .cloned()
        };
        assert_eq!(
            uid(s, c.source_index),
            uid(&old, old.components[n].source_index)
        );
    }
    assert_eq!(
        s.primitives
            .iter()
            .filter(
                |p| matches!(&p.kind,sch::SchPrimitiveKind::Pin { name,.. } if name=="Updated pin")
            )
            .count(),
        2
    );
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
fn footprint_update_preserves_native_identity_connectivity_and_annotation_records() {
    use eda_core::{design_sync::Footprint, footprint_update as update};
    let library = footprint();
    let mut board = Document::new(DocumentKind::Board, "Target.PcbDoc");
    board.pcb = library.pcb.clone();
    let b = board.pcb.as_mut().unwrap();
    let c = &mut b.components[0];
    c.designator = "U1".into();
    c.comment = "LOCAL".into();
    c.properties.insert("UNIQUEID".into(), "NATIVEID".into());
    c.properties.insert("HEIGHT".into(), "5.41mm".into());
    c.properties
        .insert("FOOTPRINTDESCRIPTION".into(), "Original footprint".into());
    c.parameters = Some(pcb::Fields::from([("Vendor".into(), "Schematic".into())]));
    c.footprint_parameters = Some(pcb::Fields::from([("Vendor".into(), "Old".into())]));
    b.nets.push(pcb::Net {
        index: 0,
        name: "GND".into(),
        properties: Default::default(),
    });
    b.items[0].net = Some(0);
    let mut annotation = pcb::Item::new(
        pcb::TOP_OVERLAY,
        None,
        pcb::Shape::Text {
            position: Point::mm(1, 3),
            text: "U1".into(),
            height: 1_000_000,
            stroke: 100_000,
            rotation: 90.,
            mirrored: false,
            font: String::new(),
        },
    );
    annotation.component = Some(c.index);
    annotation
        .properties
        .insert("DESIGNATOR".into(), "TRUE".into());
    b.items.push(annotation);
    let original =
        crate::altium_pcb::writer::serialize(&board, crate::altium_pcb::writer::Format::Binary)
            .unwrap();
    let imported = crate::altium_pcb::import_pcb(&original, "Target.PcbDoc").unwrap();
    let mut source = io::import(&io::serialize(&library).unwrap(), "Parts.PcbLib", true).unwrap();
    let b = source.pcb.as_mut().unwrap();
    assert!(b.warnings.is_empty(), "A footprint needs no Board6 outline");
    // Exercise the native deletion path independently of the library writer's
    // own explicit zero-height default for newly authored definitions.
    b.components[0].properties.remove("HEIGHT");
    b.components[0]
        .properties
        .insert("DESCRIPTION".into(), "Revised footprint".into());
    b.components[0].parameters = Some(pcb::Fields::from([("Vendor".into(), "New".into())]));
    if let pcb::Shape::Pad { size, .. } = &mut b.items[0].shape {
        size.x = 2_000_000;
    }
    let u = update::Update {
        component: imported.pcb.as_ref().unwrap().components[0].id,
        footprint: Footprint::from_library_board(b.clone()).unwrap(),
        options: Default::default(),
        source_library: Some("Parts.PcbLib".into()),
    };
    let mut with_height = u.clone();
    let mut definition = with_height.footprint.board().clone();
    definition.components[0]
        .properties
        .insert("HEIGHT".into(), "3mm".into());
    with_height.footprint = Footprint::from_library_board(definition).unwrap();
    let raised = update::preview(&imported, &[with_height]).unwrap();
    let raised_bytes =
        crate::altium_pcb::writer::serialize(&raised, crate::altium_pcb::writer::Format::Preserve)
            .unwrap();
    let raised = crate::altium_pcb::import_pcb(&raised_bytes, "Raised.PcbDoc").unwrap();
    assert_eq!(
        crate::altium_pcb::dimension(
            &raised.pcb.as_ref().unwrap().components[0].properties["HEIGHT"]
        )
        .unwrap(),
        3_000_000
    );
    let next = update::preview(&imported, &[u]).unwrap();
    let bytes =
        crate::altium_pcb::writer::serialize(&next, crate::altium_pcb::writer::Format::Preserve)
            .unwrap();
    let read = crate::altium_pcb::import_pcb(&bytes, "Saved.PcbDoc").unwrap();
    let b = read.pcb.as_ref().unwrap();
    assert_eq!(b.components[0].designator, "U1");
    assert_eq!(b.components[0].comment, "LOCAL");
    assert_eq!(b.components[0].properties["UNIQUEID"], "NATIVEID");
    assert!(!b.components[0].properties.contains_key("HEIGHT"));
    assert_eq!(
        b.components[0].properties["FOOTPRINTDESCRIPTION"],
        "Revised footprint"
    );
    assert_eq!(
        b.components[0].footprint_parameters.as_ref().unwrap()["Vendor"],
        "New"
    );
    assert_eq!(
        b.components[0].parameters.as_ref().unwrap()["Vendor"],
        "Schematic"
    );
    let pad = b
        .items
        .iter()
        .find(|i| matches!(&i.shape,pcb::Shape::Pad {name,..} if name=="A"))
        .unwrap();
    assert_eq!(pad.net, Some(0));
    assert!(matches!(pad.shape,pcb::Shape::Pad {size,..} if (size.x-2_000_000).abs()<10));
    let mut a = cfb::CompoundFile::open(std::io::Cursor::new(original)).unwrap();
    let mut z = cfb::CompoundFile::open(std::io::Cursor::new(bytes)).unwrap();
    use std::io::Read;
    let mut before = vec![];
    let mut after = vec![];
    a.open_stream("/Texts6/Data")
        .unwrap()
        .read_to_end(&mut before)
        .unwrap();
    z.open_stream("/Texts6/Data")
        .unwrap()
        .read_to_end(&mut after)
        .unwrap();
    assert_eq!(
        before, after,
        "Local annotation bytes must remain untouched"
    );
}
#[test]
fn imported_pin_edit_and_variant_removal_preserve_native_metadata_and_renumber_mode_names() {
    let mut original = symbol();
    for command in [
        lib::Edit::AddPart,
        lib::Edit::AddPart,
        lib::Edit::AddDisplayMode,
        lib::Edit::RenameDisplayMode("Diagnostic".into()),
        lib::Edit::AddDisplayMode,
        lib::Edit::RenameDisplayMode("Service".into()),
    ] {
        original = lib::preview(&original, &command).unwrap();
    }
    let mut loaded = io::import(&io::serialize(&original).unwrap(), "Parts.SchLib", false).unwrap();
    assert_eq!(lib::display_mode_name(&loaded, 1), "Diagnostic");
    assert_eq!(lib::display_mode_name(&loaded, 2), "Service");
    let n = lib::members(&loaded)
        .position(|m| lib::variant(m) == (1, 0))
        .unwrap();
    loaded = lib::preview(&loaded, &lib::Edit::Select(n)).unwrap();
    let mut pin = loaded
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find(|p| matches!(p.kind, sch::SchPrimitiveKind::Pin { .. }))
        .unwrap()
        .clone();
    let source_index = pin.source_index;
    let unique_id = loaded.schematic.as_ref().unwrap().records[source_index as usize]
        .fields
        .get("UNIQUEID")
        .cloned();
    pin.presentation.hidden = true;
    if let sch::SchPrimitiveKind::Pin {
        name, show_number, ..
    } = &mut pin.kind
    {
        *name = "Edited".into();
        *show_number = false;
    }
    loaded = lib::preview(&loaded, &lib::Edit::Pin(Box::new(pin))).unwrap();
    assert!(
        loaded
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .any(|p| p.source_index == source_index && p.presentation.hidden)
    );
    let n = lib::members(&loaded)
        .position(|m| lib::variant(m) == (2, 1))
        .unwrap();
    loaded = lib::preview(&loaded, &lib::Edit::Select(n)).unwrap();
    loaded = lib::preview(&loaded, &lib::Edit::RemovePart).unwrap();
    loaded = lib::preview(&loaded, &lib::Edit::RemoveDisplayMode).unwrap();
    let read = io::import(&io::serialize(&loaded).unwrap(), "Parts.SchLib", false).unwrap();
    assert_eq!(lib::members(&read).count(), 4);
    for m in lib::members(&read) {
        let c = &m.schematic.as_ref().unwrap().components[0];
        assert_eq!(c.settings.part_count, 2);
        assert_eq!(c.settings.display_mode_names.len(), 1);
        assert_eq!(lib::display_mode_name(m, 1), "Service");
    }
    let m = lib::members(&read)
        .find(|m| lib::variant(m) == (1, 0))
        .unwrap();
    let s = m.schematic.as_ref().unwrap();
    let p = s.primitives.iter().find(|p| matches!(&p.kind, sch::SchPrimitiveKind::Pin { name, show_number: false, .. } if name == "Edited")).unwrap();
    assert!(p.presentation.hidden);
    assert_eq!(
        s.records[p.source_index as usize]
            .fields
            .get("UNIQUEID")
            .cloned(),
        unique_id
    );
}
#[test]
fn repeated_multipart_native_saves_keep_the_definition_designator_without_section_suffixes() {
    let mut doc = lib::preview(&symbol(), &lib::Edit::AddPart).unwrap();
    for _ in 0..3 {
        doc = io::import(&io::serialize(&doc).unwrap(), "Parts.SchLib", false).unwrap();
        assert!(
            lib::members(&doc)
                .all(|m| m.schematic.as_ref().unwrap().components[0].designator == "U?")
        );
        doc = lib::preview(&doc, &lib::Edit::Rename("Edited".into())).unwrap();
    }
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
