//! Dump an independent logical/assembly scene for native visual comparisons.
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let source = args
        .next()
        .ok_or("usage: variant_scene PROJECT OUTPUT.json")?;
    let output = args.next().ok_or("An output JSON path is required")?;
    let project = eda_io::altium::load_project(std::path::Path::new(&source))?;
    let sheet = project
        .documents
        .iter()
        .find(|d| d.kind == "schdoc")
        .ok_or("No schematic")?;
    let logical = eda_io::altium::load_schematic(&sheet.path)?;
    let assembly = eda_core::assembly_variants::project(
        &logical,
        project.variants.first(),
        "",
        &Default::default(),
    )?;
    let json = serde_json::to_vec_pretty(&serde_json::json!({
        "logical": logical,
        "assembly": assembly,
    }))
    .map_err(|e| e.to_string())?;
    eda_io::project_files::write_atomic(std::path::Path::new(&output), &json)?;
    println!("Wrote {output}");
    Ok(())
}
