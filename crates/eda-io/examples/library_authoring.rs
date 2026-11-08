//! Synthetic, shareable library compatibility probe. No private project inputs.
use eda_core::{DocumentKind, library_edit as lib, pcb, schematic as sch, schematic_edit as edit};
use eda_geom::Point;
fn main() -> Result<(), String> {
    let directory = std::env::args().nth(1).ok_or("Pass an output directory")?;
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let mut symbol = lib::new(DocumentKind::Schematic, "Authoring.SchLib");
    symbol = lib::preview(&symbol, &lib::Edit::Rename("PCBCraft_Test".into()))?;
    let s = symbol.schematic.as_mut().unwrap();
    let id = s.components[0].id;
    s.components[0].description = "Synthetic two-pin component".into();
    let mut body = edit::primitive(
        sch::SchPrimitiveKind::Rectangle {
            a: Point::mm(-5, -3),
            b: Point::mm(5, 3),
            radius: Point::default(),
        },
        Some(id),
    );
    body.fill = Some(0x80ffff);
    s.primitives.push(body);
    for (number, x, orientation) in [("1", -5, 2), ("2", 5, 0)] {
        s.primitives.push(edit::primitive(
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
            Some(id),
        ));
    }
    let mut footprint = lib::new(DocumentKind::Board, "Authoring.PcbLib");
    footprint = lib::preview(&footprint, &lib::Edit::Rename("TEST_2PAD".into()))?;
    let b = footprint.pcb.as_mut().unwrap();
    for (number, x) in [("1", -2), ("2", 2)] {
        let mut pad = pcb::Item::new(
            pcb::TOP,
            None,
            pcb::Shape::Pad {
                position: Point::mm(x, 0),
                size: Point::mm(2, 2),
                rotation: 0.,
                shape: pcb::PadShape::Rectangle,
                hole: 0,
                plated: true,
                name: number.into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        pad.component = Some(0);
        b.items.push(pad);
    }
    symbol = lib::preview(
        &symbol,
        &lib::Edit::LinkFootprint {
            name: "TEST_2PAD".into(),
            library: "Authoring.PcbLib".into(),
            map: vec![],
        },
    )?;
    for (name, doc) in [
        ("Authoring.SchLib", &symbol),
        ("Authoring.PcbLib", &footprint),
        ("symbol.pcbcraft.json", &symbol),
        ("footprint.pcbcraft.json", &footprint),
    ] {
        let path = directory.join(name);
        eda_io::save(&path, doc)?;
        let reopened = eda_io::load(&path)?;
        println!(
            "{}: {} member(s)",
            path.display(),
            lib::members(&reopened).count()
        );
    }
    eda_io::library_documents::compile_integrated(
        &directory.join("Authoring.IntLib"),
        &symbol,
        &[&footprint],
    )?;
    let mut variants = symbol.clone();
    for command in [
        lib::Edit::AddPart,
        lib::Edit::AddPart,
        lib::Edit::AddDisplayMode,
        lib::Edit::RenameDisplayMode("Diagnostic".into()),
        lib::Edit::AddDisplayMode,
        lib::Edit::RenameDisplayMode("Service".into()),
    ] {
        variants = lib::preview(&variants, &command)?;
    }
    eda_io::save(&directory.join("AuthoringVariants.SchLib"), &variants)?;
    let n = lib::members(&variants)
        .position(|m| lib::variant(m) == (2, 1))
        .unwrap();
    variants = lib::preview(&variants, &lib::Edit::Select(n))?;
    variants = lib::preview(&variants, &lib::Edit::RemovePart)?;
    variants = lib::preview(&variants, &lib::Edit::RemoveDisplayMode)?;
    eda_io::save(&directory.join("AuthoringReduced.SchLib"), &variants)?;
    Ok(())
}
