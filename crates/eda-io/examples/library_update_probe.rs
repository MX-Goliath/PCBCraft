//! Disposable native library-update reference; no user project inputs.
use eda_core::{DocumentKind, library_edit as lib, schematic as sch, schematic_edit as edit};
use eda_geom::Point;

fn main() -> Result<(), String> {
    let folder = std::env::args()
        .nth(1)
        .ok_or("Pass a new output directory")?;
    let folder = std::path::Path::new(&folder);
    if folder.exists() {
        return Err("Use a new directory to keep earlier observations intact".into());
    }
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    let mut source = lib::new(DocumentKind::Schematic, "Parts.SchLib");
    source = lib::preview(
        &source,
        &lib::Edit::Metadata {
            name: "UPDATE_TEST".into(),
            designator: "U?".into(),
            comment: "=Value".into(),
            description: "Original definition".into(),
            parameters: vec![("Value".into(), "10k".into())],
        },
    )?;
    let scene = source.schematic.as_mut().unwrap();
    let owner = scene.components[0].id;
    let mut body = edit::primitive(
        sch::SchPrimitiveKind::Rectangle {
            a: Point::mm(-5, -3),
            b: Point::mm(5, 3),
            radius: Point::default(),
        },
        Some(owner),
    );
    body.fill = Some(0x80ffff);
    scene.primitives.push(body);
    for (number, x, orientation) in [("1", -5, 2), ("2", 5, 0)] {
        scene.primitives.push(edit::primitive(
            sch::SchPrimitiveKind::Pin {
                position: Point::mm(x, 0),
                length_nm: 2_540_000,
                orientation,
                name: format!("P{number}"),
                number: number.into(),
                show_name: true,
                show_number: true,
                font: 0,
                number_font: None,
                electrical: 4,
                outer_symbol: 0,
                inner_symbol: 0,
            },
            Some(owner),
        ));
    }
    for p in &mut scene.primitives {
        if p.text_role.is_some() {
            p.presentation.hidden = false;
        }
    }
    eda_io::save(&folder.join("PartsBaseline.SchLib"), &source)?;
    eda_io::save(&folder.join("Parts.SchLib"), &source)?;
    let library = eda_io::altium::library::load(&folder.join("PartsBaseline.SchLib"))?;
    let symbol = &library.symbols[0];
    let mut sheet = eda_io::altium::new_schematic("Sheet.SchDoc")?;
    let library_path = folder
        .join("Parts.SchLib")
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .to_owned();
    for (n, position, rotation) in [(1, Point::mm(50, 50), 0), (2, Point::mm(80, 50), 1)] {
        let mut fragment = symbol.instantiate(1, 0, sheet.schematic.as_ref().unwrap())?;
        fragment.components[0].settings.source = library_path.clone();
        fragment.components[0].settings.design_item_id = "UPDATE_TEST".into();
        let component = fragment.components[0].id;
        let fragment = fragment.transformed(Point::default(), rotation, position)?;
        sheet = edit::preview(&sheet, &edit::Edit::Insert(fragment))?;
        if n == 2 {
            sheet = edit::preview(
                &sheet,
                &edit::Edit::ParameterValue {
                    component: Some(component),
                    name: "Value".into(),
                    value: Some("47k".into()),
                },
            )?;
        }
    }
    eda_io::save(&folder.join("Sheet.SchDoc"), &sheet)?;
    std::fs::write(folder.join("UpdateReference.PrjPcb"), "[Design]\r\nVersion=1.0\r\nName=UpdateReference\r\n[Document1]\r\nDocumentPath=Sheet.SchDoc\r\n[Document2]\r\nDocumentPath=Parts.SchLib\r\n").map_err(|e| e.to_string())?;
    source = lib::preview(
        &source,
        &lib::Edit::Metadata {
            name: "UPDATE_TEST".into(),
            designator: "U?".into(),
            comment: "=Value".into(),
            description: "Updated definition".into(),
            parameters: vec![
                ("Value".into(), "12k".into()),
                ("Vendor".into(), "Example".into()),
            ],
        },
    )?;
    for p in &mut source.schematic.as_mut().unwrap().primitives {
        if let sch::SchPrimitiveKind::Rectangle { b, .. } = &mut p.kind {
            b.y = 4_000_000;
        }
    }
    eda_io::save(&folder.join("Parts.SchLib"), &source)?;
    let updated = eda_io::altium::library::load(&folder.join("Parts.SchLib"))?;
    let baseline = eda_io::load(&folder.join("Sheet.SchDoc"))?;
    let updates = baseline
        .schematic
        .as_ref()
        .unwrap()
        .components
        .iter()
        .enumerate()
        .map(|(n, c)| {
            let mut options = eda_core::library_update::Options::default();
            if n == 1 {
                options.excluded_parameters.insert("Value".into());
            }
            eda_core::library_update::Update {
                component: c.id,
                definition: updated.symbols[0]
                    .variant(c.part, c.display_mode)
                    .unwrap()
                    .fragment
                    .clone(),
                options,
            }
        })
        .collect::<Vec<_>>();
    let result = eda_core::library_update::preview(&baseline, &updates)?;
    eda_io::save(&folder.join("RustUpdated.SchDoc"), &result)?;
    println!("Prepared synthetic project in {}", folder.display());
    Ok(())
}
