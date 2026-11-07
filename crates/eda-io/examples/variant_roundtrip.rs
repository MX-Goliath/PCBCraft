//! Inspect project variant interoperability without touching the source project.
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let source = std::path::PathBuf::from(
        args.next()
            .ok_or("usage: variant_roundtrip SOURCE.PrjPcb COPY.PrjPcb")?,
    );
    let destination =
        std::path::PathBuf::from(args.next().ok_or("A separate destination is required")?);
    if destination.exists() {
        return Err("Destination already exists".into());
    }
    let project = eda_io::altium::load_project(&source)?;
    for v in &project.variants {
        println!(
            "{}: {} variations, {} parameters, {} alternate symbols",
            v.name,
            v.variations.len(),
            v.parameters.len(),
            v.variations
                .iter()
                .filter(|c| c.alternate.is_some())
                .count()
        );
    }
    let saved = eda_io::project_files::save_project_as(&project, &destination)?;
    if saved.variants.len() != project.variants.len() {
        return Err("Variant count changed".into());
    }
    println!("Saved {}", saved.path.display());
    Ok(())
}
