use super::altium::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::schematic::SchPrimitiveKind as K;
use eda_geom::Point;
use encoding_rs::WINDOWS_1251;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};

const ASCII: &[u8] = include_bytes!("../../../fixtures/schematic/minimal.SchDoc");

#[test]
fn erc_reporting_levels_are_scoped_to_the_native_erc_section() {
    use eda_core::schematic_validation::Severity;
    let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Reporting.PrjPcb");
    std::fs::write(&path,"[Design]\n[Modification Levels]\nType10=3\n[Electrical Rules Check]\nType10=0\nType14=3\nType153=1\n[Difference Levels]\nType14=2\n").unwrap();
    let p = load_project(&path).unwrap();
    assert_eq!(p.validation_options.reporting_levels[&10], None);
    assert_eq!(
        p.validation_options.reporting_levels[&14],
        Some(Severity::Fatal)
    );
    assert_eq!(
        p.validation_options.reporting_levels[&153],
        Some(Severity::Warning)
    );
    std::fs::write(&path, "[Design]\n[Electrical Rules Check]\nType10=4\n").unwrap();
    assert!(
        load_project(&path)
            .unwrap_err()
            .contains("Invalid ERC reporting level")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_erc_pin_matrix_is_loaded_independently_of_output_settings() {
    use eda_core::schematic_validation::Severity;
    let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ERC.PrjPcb");
    std::fs::write(&path,"[Design]\nNetlistSinglePinNets=0\n[ERC Connection Matrix]\nL1=NNNNNNNNNNNWNNNWW\nL2=NNWNNNNWWWNWNWNWN\nL3=NNWNNNNNNNNNNNNNN\nL4=NNENNNWEENNWNENWN\nL5=NNNNNNNNNNNNNNNNN\nL6=NNENNNNEENNWNENWN\nL7=NNEWNNWEENNWNENWN\nL8=NWEENEENEEENNEENN\n").unwrap();
    let p = load_project(&path).unwrap();
    assert!(!p.netlist_options.single_pin_nets);
    assert_eq!(
        p.validation_options.connection_matrix[2][2],
        Some(Severity::Warning)
    );
    assert_eq!(p.validation_options.connection_matrix[4][4], None);
    std::fs::write(&path, "[Design]\n[ERC Connection Matrix]\nL1=N\n").unwrap();
    assert!(
        load_project(&path)
            .unwrap_err()
            .contains("Incomplete ERC connection matrix row")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_sheet_formats_match_editor_sizes_and_preserve_custom_dimensions() {
    for portrait in [false, true] {
        for index in 0..eda_core::schematic::STANDARD_SHEETS.len() {
            let text = String::from_utf8(ASCII.to_vec()).unwrap().replace(
                "|RECORD=31|",
                &format!(
                    "|RECORD=31|SHEETSTYLE={index}|WORKSPACEORIENTATION={}|",
                    u8::from(portrait)
                ),
            );
            let doc = import_schematic(text.as_bytes(), "Sheet.SchDoc").unwrap();
            let mut editor_sheet = eda_core::schematic::Schematic::blank().sheet;
            let mut editor_size = if portrait {
                Point::mm(100, 200)
            } else {
                Point::mm(200, 100)
            };
            assert!(editor_sheet.set_standard_format(&mut editor_size, index));
            assert_eq!(doc.size, editor_size);
            assert_eq!(
                doc.schematic.as_ref().unwrap().sheet.format_name(),
                editor_sheet.format_name()
            );
            assert_eq!(writer::serialize(&doc).unwrap(), text.as_bytes());
        }
        let text = String::from_utf8(ASCII.to_vec()).unwrap().replace(
            "|RECORD=31|",
            &format!(
                "|RECORD=31|USECUSTOMSHEET=T|CUSTOMX=800|CUSTOMY=500|WORKSPACEORIENTATION={}|",
                u8::from(portrait)
            ),
        );
        let doc = import_schematic(text.as_bytes(), "Custom.SchDoc").unwrap();
        assert_eq!(
            doc.size,
            Point {
                x: 203_200_000,
                y: 127_000_000
            }
        );
        assert_eq!(writer::serialize(&doc).unwrap(), text.as_bytes());
    }
}

#[test]
fn native_compile_masks_preserve_expanded_and_collapsed_state_and_archive_bytes() {
    let bytes=[ASCII,b"\n|RECORD=211|LOCATION.X=0|LOCATION.Y=0|CORNER.X=100|CORNER.Y=100\n|RECORD=211|LOCATION.X=200|LOCATION.Y=0|CORNER.X=300|CORNER.Y=100|COLLAPSED=T\n"].concat();
    let doc = import_schematic(&bytes, "Masks.SchDoc").unwrap();
    let masks: Vec<_> = doc
        .schematic
        .as_ref()
        .unwrap()
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::CompileMask { a, b, collapsed } = p.kind {
                Some(eda_core::schematic::CompileMask { a, b, collapsed })
            } else {
                None
            }
        })
        .collect();
    assert_eq!(masks.len(), 2);
    assert!(!masks[0].collapsed && masks[1].collapsed);
    assert_eq!(
        masks[0].b,
        Point {
            x: 25_400_000,
            y: 25_400_000
        }
    );
    assert_eq!(writer::serialize(&doc).unwrap(), bytes);
}

#[test]
fn project_compilation_resolves_current_symbols_relative_to_owner_and_imports_net_preferences() {
    use eda_core::{Document, DocumentKind, schematic_edit::primitive};
    let dir = tempfile::tempdir().unwrap();
    for folder in ["a", "b"] {
        std::fs::create_dir(dir.path().join(folder)).unwrap();
    }
    let paths = [
        dir.path().join("Main.SchDoc"),
        dir.path().join("a/Child.SchDoc"),
        dir.path().join("b/Child.SchDoc"),
    ];
    for path in &paths {
        std::fs::write(path, b"").unwrap();
    }
    let path = dir.path().join("Project.PrjPcb");
    std::fs::write(&path,"[Design]\nHierarchyMode=0\nAllowPortNetNames=0\nAllowSheetEntryNetNames=1\nPowerPortNamesTakePriority=0\nNameNetsHierarchically=0\nNetlistSinglePinNets=0\nAppendSheetNumberToLocalNets=1\n[Document1]\nDocumentPath=Main.SchDoc\n[Document2]\nDocumentPath=a/Child.SchDoc\n[Document3]\nDocumentPath=b/Child.SchDoc\n").unwrap();
    let project = load_project(&path).unwrap();
    let mut main = Document::new(DocumentKind::Schematic, "Main.SchDoc");
    let a = Document::new(DocumentKind::Schematic, "Child.SchDoc");
    let b = Document::new(DocumentKind::Schematic, "Child.SchDoc");
    for folder in ["a", "b"] {
        main.schematic.as_mut().unwrap().primitives.push(primitive(
            K::SheetSymbol {
                a: Point::mm(0, 10),
                b: Point::mm(10, 0),
                designator: folder.into(),
                filename: format!("{folder}/Child.SchDoc"),
            },
            None,
        ));
    }
    let sheets: Vec<_> = project
        .documents
        .iter()
        .zip([&main, &a, &b])
        .map(|(e, d)| (e.path.as_path(), d))
        .collect();
    let options = project.compile_options(&sheets).unwrap();
    let links = options.links.as_ref().unwrap();
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].child, a.id);
    assert_eq!(links[1].child, b.id);
    assert!(
        !options.allow_port_names && options.allow_entry_names && !options.power_names_priority
    );
    assert!(
        !options.hierarchical_net_names && !options.single_pin_nets && options.append_sheet_number
    );
    assert!(options.review_annotation);
    assert_eq!(options.sheet_paths[&b.id], "b/Child.SchDoc");
    if let K::SheetSymbol { filename, .. } =
        &mut main.schematic.as_mut().unwrap().primitives[0].kind
    {
        *filename = "missing.SchDoc".into();
    }
    let sheets: Vec<_> = project
        .documents
        .iter()
        .zip([&main, &a, &b])
        .map(|(e, d)| (e.path.as_path(), d))
        .collect();
    assert!(
        project
            .compile_options(&sheets)
            .unwrap_err()
            .contains("missing or outside")
    );
}

#[test]
fn native_port_preserves_alignment_fill_border_height_and_far_connection() {
    let header = std::str::from_utf8(ASCII)
        .unwrap()
        .lines()
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    let text = format!(
        "{header}\n|RECORD=18|LOCATION.X=442|LOCATION.X_FRAC=91340|LOCATION.Y=305|LOCATION.Y_FRAC=11812|WIDTH=49|WIDTH_FRAC=21260|HEIGHT=9|HEIGHT_FRAC=84252|IOTYPE=1|ALIGNMENT=2|BORDERWIDTH=1|AREACOLOR=8454143|COLOR=128|NAME=KL30|FONTID=1\n|RECORD=27|LOCATIONCOUNT=2|X1=492|X1_FRAC=12600|Y1=305|Y1_FRAC=11812|X2=600|Y2=305|Y2_FRAC=11812\n"
    );
    let doc = import_schematic(text.as_bytes(), "Ports.SchDoc").unwrap();
    let s = doc.schematic.as_ref().unwrap();
    let port = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::Port { .. }))
        .unwrap();
    assert_eq!(port.presentation.alignment, Some(2));
    assert_eq!(port.fill, Some(8454143));
    assert_eq!(port.width_nm, 254_000);
    assert!(matches!(
        port.kind,
        K::Port {
            height_nm: 2_500_000,
            ..
        }
    ));
    // A native 12.5 mm-wide KL30 Port connects at its right-hand edge.
    let wire = s
        .primitives
        .iter()
        .find(|p| matches!(p.kind, K::Polyline { .. }))
        .unwrap();
    assert!(eda_core::cross_probe::touches(port, wire));
    assert_eq!(writer::serialize(&doc).unwrap(), text.as_bytes());
}

#[test]
fn fractional_native_pin_endpoints_remain_connected_after_nm_conversion() {
    use eda_core::schematic_edit::electrical_endpoint;
    for sign in [1, -1] {
        for orientation in 0..4 {
            let backwards = orientation >= 2;
            let (end, end_frac, expected) = match (sign, backwards) {
                (1, false) => (1084, 64570, 275_500_008),
                (1, true) => (1072, 83468, 272_500_009),
                (-1, false) => (-1072, -83468, -272_500_009),
                _ => (-1084, -64570, -275_500_008),
            };
            let vertical = orientation % 2 == 1;
            let (axis, other) = if vertical { ("Y", "X") } else { ("X", "Y") };
            let header = String::from_utf8(ASCII.to_vec())
                .unwrap()
                .lines()
                .take(2)
                .collect::<Vec<_>>()
                .join("\r\n");
            let text = format!(
                "{header}\r\n|RECORD=1|LIBREFERENCE=Test|PARTCOUNT=1|CURRENTPARTID=1|UNIQUEID=TESTPART\r\n|RECORD=2|OWNERINDEX=1|OWNERPARTID=1|LOCATION.{axis}={}|LOCATION.{axis}_FRAC={}|LOCATION.{other}=100|PINLENGTH=5|PINLENGTH_FRAC=90551|PINCONGLOMERATE={}|DESIGNATOR=1\r\n|RECORD=34|OWNERINDEX=1|OWNERPARTID=-1|TEXT=U1|FONTID=1\r\n|RECORD=27|OWNERINDEX=0|LOCATIONCOUNT=2|{axis}1={end}|{axis}1_FRAC={end_frac}|{other}1=100|{axis}2={end}|{axis}2_FRAC={end_frac}|{other}2=90\r\n|RECORD=25|OWNERINDEX=0|LOCATION.{axis}={end}|LOCATION.{axis}_FRAC={end_frac}|LOCATION.{other}=90|TEXT=GND|FONTID=1\r\n",
                sign * 1078,
                sign * 74019,
                32 + orientation
            );
            let doc = import_schematic(text.as_bytes(), "fractional.SchDoc").unwrap();
            let s = doc.schematic.as_ref().unwrap();
            let pin = s
                .primitives
                .iter()
                .find(|p| matches!(p.kind, K::Pin { .. }))
                .unwrap();
            let at = electrical_endpoint(pin).unwrap();
            assert_eq!(
                if vertical { at.y } else { at.x },
                expected,
                "sign {sign}, direction {orientation}"
            );
            let nets = eda_core::design_sync::compile(&[&doc]).unwrap();
            assert_eq!(nets.pins[0].net.as_deref(), Some("GND"));
            // A deliberate one-nm gap in the authored model still stays disconnected.
            let mut gap = doc.clone();
            let pin = gap
                .schematic
                .as_mut()
                .unwrap()
                .primitives
                .iter_mut()
                .find(|p| matches!(p.kind, K::Pin { .. }))
                .unwrap();
            if let K::Pin { position, .. } = &mut pin.kind {
                if vertical {
                    position.x += 1;
                } else {
                    position.y += 1;
                }
            }
            assert!(
                eda_core::design_sync::compile(&[&gap]).unwrap().pins[0]
                    .net
                    .is_none()
            );
        }
    }
}

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
fn project_tree_metadata_keeps_sources_generated_and_variants_separate() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tree.PrjPcb");
    std::fs::write(&path, "[Design]\nIsVirtualBomDocumentRemoved=0\n[Document1]\nDocumentPath=main.SchDoc\n[Document2]\nDocumentPath=BOM.OutJob\n[GeneratedDocument1]\nDocumentPath=Outputs\\board.REP\n[GeneratedDocument2]\nDocumentPath=Outputs\\features\n[Variant1]\nDescription=Assembly A\n[Variant2]\nDescription=Assembly B\n[Configuration1]\nVariant=Assembly A\n").unwrap();
    let before = std::fs::read(&path).unwrap();
    let p = load_project(&path).unwrap();
    assert_eq!(p.documents.len(), 2);
    assert_eq!(p.documents[1].kind, "outjob");
    assert_eq!(p.generated_documents.len(), 2);
    assert_eq!(p.generated_documents[0].kind, "rep");
    assert_eq!(
        p.variants
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        ["Assembly A", "Assembly B"]
    );
    assert!(p.virtual_bom);
    assert_eq!(p.warnings.len(), 2); // Missing generated output is not a missing source warning.
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn project_sheet_tree_reads_references_without_importing_geometry() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("main.SchDoc"),
        b"|RECORD=15|\n|RECORD=33|OWNERINDEX=0|TEXT=child.SchDoc|\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("child.SchDoc"), b"|RECORD=31|\n").unwrap();
    let path = dir.path().join("tree.PrjPcb");
    std::fs::write(
        &path,
        "[Design]\n[Document1]\nDocumentPath=child.SchDoc\n[Document2]\nDocumentPath=main.SchDoc\n",
    )
    .unwrap();
    std::fs::write(path.with_extension("PrjPcbStructure"), "Record=TopLevelDocument|FileName=main.SchDoc|SheetNumber=1\nRecord=SheetSymbol|SourceDocument=main.SchDoc|FileName=child.SchDoc|SheetNumber=2\n").unwrap();
    let p = load_project(&path).unwrap();
    assert_eq!(
        p.sheet_children[&p.documents[1].path],
        [p.documents[0].path.clone()]
    );
    assert_eq!(p.sheet_number(&p.documents[1].path), Some(1));
    assert_eq!(p.sheet_number(&p.documents[0].path), Some(2));
    assert_eq!(
        p.resolve_sheet(&p.documents[1].path, "child.SchDoc")
            .unwrap()
            .path,
        p.documents[0].path
    );
    assert!(
        p.resolve_sheet(&p.documents[1].path, "other/child.SchDoc")
            .is_none()
    );
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
fn native_schematic_save_never_writes_json() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("original.SchDoc");
    std::fs::write(&path, ASCII).unwrap();
    let doc = load_schematic(&path).unwrap();
    crate::save(&path, &doc).unwrap();
    assert!(std::fs::read(&path).unwrap().starts_with(b"|HEADER="));
    assert_eq!(
        crate::load(&path).unwrap().schematic.unwrap().components,
        doc.schematic.unwrap().components
    );
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
