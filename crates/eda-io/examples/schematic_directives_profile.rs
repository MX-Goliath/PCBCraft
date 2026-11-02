//! Read a native sheet, exercise directive edits, and write only an explicit QA copy.
use eda_core::{
    schematic::SchPrimitiveKind as K,
    schematic_directives as directives,
    schematic_edit::{self as edit, Edit, Fragment},
};
use eda_geom::Point;
fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input = std::path::PathBuf::from(
        args.next()
            .ok_or("Pass an input SchDoc and a new QA output SchDoc")?,
    );
    let output = std::path::PathBuf::from(args.next().ok_or("Pass a new QA output SchDoc")?);
    if output.exists() {
        return Err("QA output must not already exist".into());
    }
    let doc = eda_io::altium::load_schematic(&input)?;
    let s = doc.schematic.as_ref().unwrap();
    for (label, count) in [
        (
            "Blankets",
            s.primitives
                .iter()
                .filter(|p| matches!(p.kind, K::Blanket { .. }))
                .count(),
        ),
        (
            "Compile Masks",
            s.primitives
                .iter()
                .filter(|p| matches!(p.kind, K::CompileMask { .. }))
                .count(),
        ),
        (
            "Parameter Sets",
            s.primitives
                .iter()
                .filter(|p| matches!(p.kind, K::ParameterSet { .. }))
                .count(),
        ),
    ] {
        println!("{label}: {count}");
    }
    let masks = directives::primitive(K::CompileMask {
        a: Point::mm(145, 210),
        b: Point::mm(260, 270),
        collapsed: false,
    });
    let polygon = directives::primitive(K::Blanket {
        points: vec![
            Point::mm(300, 385),
            Point::mm(330, 385),
            Point::mm(325, 400),
            Point::mm(300, 400),
        ],
        collapsed: false,
    });
    let set = directives::primitive(K::ParameterSet {
        position: Point::mm(300, 390),
        orientation: 0,
        name: "QA_Class".into(),
        style: 0,
        parameters: std::collections::BTreeMap::from([("ClassName".into(), "QA_Class".into())]),
        rules: vec![],
    });
    let next = edit::preview(
        &doc,
        &Edit::Insert(Fragment {
            primitives: vec![masks, polygon, set],
            ..Default::default()
        }),
    )?;
    eda_io::altium::writer::save(&output, &next)?;
    let reopened = eda_io::altium::load_schematic(&output)?;
    let excluded = eda_core::schematic_compile::exclusions(reopened.schematic.as_ref().unwrap());
    println!(
        "QA save/reimport passed: {} masked components, {} masked primitives",
        excluded.components.len(),
        excluded.primitives.len()
    );
    Ok(())
}
