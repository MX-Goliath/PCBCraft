use super::*;
use eda_core::{
    schematic_arrange,
    schematic_edit::{self, Edit, Placement},
};
fn document() -> Document {
    import_schematic(
        include_bytes!("../../../fixtures/schematic/minimal.SchDoc"),
        "ActiveBar.SchDoc",
    )
    .unwrap()
}

fn binary_document() -> Document {
    let mut file = cfb::CompoundFile::create(Cursor::new(vec![])).unwrap();
    {
        let mut stream = file.create_stream("FileHeader").unwrap();
        for line in include_bytes!("../../../fixtures/schematic/minimal.SchDoc")
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
        {
            let bytes = [line.strip_suffix(b"\r").unwrap_or(line), &[0]].concat();
            stream
                .write_all(&(bytes.len() as u32).to_le_bytes())
                .unwrap();
            stream.write_all(&bytes).unwrap();
        }
    }
    import_schematic(&file.into_inner().into_inner(), "Directives.SchDoc").unwrap()
}

#[test]
fn active_bar_directives_offsheet_and_mask_round_trip_and_keep_owners() {
    let mut d = binary_document();
    for (n, kind) in [
        Placement::OffSheetConnector,
        Placement::ParameterSet,
        Placement::DifferentialPair,
    ]
    .into_iter()
    .enumerate()
    {
        let f = schematic_edit::place(
            kind,
            Point::mm(50 + n as i64 * 20, 100),
            0,
            "",
            if kind == Placement::OffSheetConnector {
                "OffSheet"
            } else {
                "Parameter Set"
            },
            0,
        )
        .unwrap();
        d = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
    }
    for kind in [Placement::Blanket, Placement::CompileMask] {
        let f = schematic_arrange::shape(
            d.schematic.as_ref().unwrap(),
            kind,
            &[Point::mm(10, 50), Point::mm(40, 20)],
            "",
        )
        .unwrap();
        d = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
    }
    let bytes = serialize(&d).unwrap();
    let read = import_schematic(&bytes, "Directives.SchDoc").unwrap();
    let s = read.schematic.as_ref().unwrap();
    assert!(s.primitives.iter().any(|p| matches!(
        p.kind,
        K::PowerPort {
            cross_sheet: true,
            ..
        }
    )));
    let marker = s
        .primitives
        .iter()
        .find(|p| p.presentation.name == "DifferentialPair")
        .unwrap();
    let pair = s.primitive(marker.presentation.parent.unwrap()).unwrap();
    assert!(matches!(&pair.kind,K::ParameterSet{label,..}if label=="Parameter Set"));
    assert!(s.primitives.iter().any(
        |p| p.presentation.parent == Some(pair.id) && p.presentation.name == "DifferentialPair"
    ));
    assert!(
        s.primitives
            .iter()
            .any(|p| matches!(p.kind, K::Blanket { .. })
                && s.records[p.source_index as usize].stream == "Additional")
    );
    assert!(s.primitives.iter().any(|p| matches!(
        p.kind,
        K::CompileMask {
            collapsed: false,
            ..
        }
    )));
    assert_eq!(serialize(&read).unwrap(), bytes);
}
#[test]
fn device_sheet_reference_round_trip_keeps_source_kind_and_owned_harness_entries() {
    let d = binary_document();
    let mut child = Document::new(DocumentKind::Schematic, "Interface");
    let mut port =
        schematic_edit::place(Placement::Port, Point::mm(10, 10), 0, "", "Serial", 0).unwrap();
    port.primitives[0].presentation.harness_type = "SerialLink".into();
    child = schematic_edit::preview(&child, &Edit::Insert(port)).unwrap();
    let f = schematic_arrange::device_sheet(
        d.schematic.as_ref().unwrap(),
        child.schematic.as_ref().unwrap(),
        "Interface",
    )
    .unwrap();
    let owner = f.primitives[0].id;
    assert_eq!(
        f.primitives
            .iter()
            .filter(|p| p.presentation.parent == Some(owner))
            .count(),
        3
    );
    let changed = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
    let bytes = serialize(&changed).unwrap();
    let read = import_schematic(&bytes, "Devices.SchDoc").unwrap();
    let s = read.schematic.as_ref().unwrap();
    let sheet = s
        .primitives
        .iter()
        .find(|p| p.presentation.symbol_type == "Device Sheet")
        .unwrap();
    assert!(
        matches!(&sheet.kind,K::SheetSymbol{designator,filename,..} if designator=="U_Interface" && filename=="Interface")
    );
    assert!(
        s.primitives
            .iter()
            .any(|p| p.presentation.parent == Some(sheet.id)
                && p.presentation.harness_type == "SerialLink"
                && matches!(&p.kind,K::SheetEntry{name,..}if name=="Serial"))
    );
    assert_eq!(serialize(&read).unwrap(), bytes);
}
#[test]
fn active_bar_sources_probes_and_images_round_trip() {
    for kind in [
        Placement::Resistor,
        Placement::Capacitor,
        Placement::VoltageSource,
        Placement::CurrentSource,
        Placement::InitialCondition,
        Placement::VoltageProbe,
        Placement::VoltageDiffProbe,
        Placement::CurrentProbe,
        Placement::PowerProbe,
    ] {
        let d = binary_document();
        let f = schematic_edit::place(kind, Point::mm(50, 50), 1, "V1", "", 0).unwrap();
        let d = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
        let bytes = serialize(&d).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let read = import_schematic(&bytes, "Directives.SchDoc").unwrap();
        if matches!(
            kind,
            Placement::Resistor
                | Placement::Capacitor
                | Placement::VoltageSource
                | Placement::CurrentSource
                | Placement::InitialCondition
        ) {
            let c = read.schematic.as_ref().unwrap().components.last().unwrap();
            if kind == Placement::VoltageSource {
                let circle = read
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .find(|p| p.component == Some(c.id) && matches!(p.kind, K::Arc { .. }))
                    .unwrap();
                assert_eq!(
                    circle.presentation.record_type, 8,
                    "A filled source body must use a native ellipse, not an unfilled arc"
                );
            }
            let moved = schematic_edit::preview(
                &read,
                &Edit::Transform {
                    id: c.id,
                    pivot: c.position,
                    quarter_turns: 1,
                    delta: Point::mm(10, 10),
                },
            )
            .unwrap();
            serialize(&moved).unwrap_or_else(|e| panic!("Moving {kind:?}: {e}"));
        }
        assert_eq!(serialize(&read).unwrap(), bytes);
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("graphic.png");
    image::RgbaImage::from_pixel(4, 2, image::Rgba([0, 150, 250, 255]))
        .save(&path)
        .unwrap();
    for embedded in [false, true] {
        let d = binary_document();
        let f = crate::schematic_graphic::load(&path, embedded)
            .unwrap()
            .transformed(Point::default(), 0, Point::mm(50, 50))
            .unwrap();
        let d = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
        let bytes = serialize(&d).unwrap_or_else(|e| panic!("Image embedded={embedded}: {e}"));
        let read = import_schematic(&bytes, "Directives.SchDoc").unwrap();
        assert_eq!(
            !read.schematic.as_ref().unwrap().images.is_empty(),
            embedded
        );
        assert_eq!(serialize(&read).unwrap(), bytes);
        let native = directory.path().join(if embedded {
            "embedded.SchDoc"
        } else {
            "external.SchDoc"
        });
        std::fs::write(&native, &bytes).unwrap();
        let loaded = crate::load(&native).unwrap();
        assert!(!loaded.schematic.as_ref().unwrap().images.is_empty());
        assert!(
            serialize(&loaded).unwrap() == bytes,
            "External resource caching must preserve unchanged native bytes"
        );
    }
}
#[test]
fn authored_active_bar_graphics_and_owned_sheet_text_round_trip() {
    for kind in [
        Placement::Rectangle,
        Placement::RoundRectangle,
        Placement::FullCircle,
        Placement::Ellipse,
        Placement::Arc,
        Placement::EllipticalArc,
        Placement::Polygon,
        Placement::Bezier,
        Placement::TextFrame,
        Placement::Note,
        Placement::SheetSymbol,
    ] {
        let d = document();
        let points = [
            Point {
                x: 25_400_000,
                y: 50_800_000,
            },
            Point {
                x: 50_800_000,
                y: 25_400_000,
            },
            Point {
                x: 76_200_000,
                y: 25_400_000,
            },
            Point {
                x: 76_200_000,
                y: 50_800_000,
            },
        ];
        let count = match kind {
            Placement::Arc | Placement::Bezier | Placement::Polygon => 4,
            Placement::Ellipse | Placement::EllipticalArc => 3,
            _ => 2,
        };
        let f = schematic_arrange::shape(
            d.schematic.as_ref().unwrap(),
            kind,
            &points[..count],
            "Example",
        )
        .unwrap();
        let d = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
        let bytes = serialize(&d).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
        let read = import_schematic(&bytes, "ActiveBar.SchDoc").unwrap();
        assert_eq!(serialize(&read).unwrap(), bytes);
    }
}
#[test]
fn authored_no_erc_and_power_presets_keep_native_metadata() {
    let mut d = document();
    for (kind, at, value) in [
        (Placement::NoErc, Point::mm(100, 100), ""),
        (Placement::Ground, Point::mm(80, 100), "+12"),
    ] {
        let f = schematic_edit::place(kind, at, 0, "", value, 0).unwrap();
        d = schematic_edit::preview(&d, &Edit::Insert(f)).unwrap();
    }
    let bytes = serialize(&d).unwrap();
    assert_eq!(
        serialize(&import_schematic(&bytes, "ActiveBar.SchDoc").unwrap()).unwrap(),
        bytes
    );
}

#[test]
fn native_stack_order_is_written_instead_of_normalized_away() {
    let d = document();
    let power = d
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::PowerPort { .. }))
        .unwrap()
        .id;
    let d = schematic_edit::preview(
        &d,
        &Edit::Reorder {
            ids: vec![power],
            target: None,
            front: true,
        },
    )
    .unwrap();
    let bytes = serialize(&d).unwrap();
    let read = import_schematic(&bytes, "ActiveBar.SchDoc").unwrap();
    assert!(matches!(
        read.schematic
            .as_ref()
            .unwrap()
            .primitives
            .last()
            .unwrap()
            .kind,
        K::PowerPort { .. }
    ));
    assert_eq!(serialize(&read).unwrap(), bytes);
}
