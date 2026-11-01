//! Read-only validation harness; shares exactly the GUI's project compiler/ERC.
use eda_core::schematic_validation;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("Pass a PrjPcb path")?);
    let project = eda_io::altium::load_project(&path)?;
    let snapshots: Vec<_> = project
        .documents
        .iter()
        .filter(|e| e.kind == "schdoc" || e.kind == "json")
        .map(|e| eda_io::load(&e.path).map(|d| (e.path.as_path(), d)))
        .collect::<Result<_, _>>()?;
    let sheets: Vec<_> = snapshots.iter().map(|(p, d)| (*p, d)).collect();
    let options = project.compile_options(&sheets)?;
    let documents: Vec<_> = sheets.iter().map(|(_, d)| *d).collect();
    let report = schematic_validation::validate(&documents, &options, &project.validation_options);
    println!(
        "Coverage: partial ERC; remaining native reporting levels and advanced checks are not implemented."
    );
    for d in &report.diagnostics {
        println!("{}: {}", d.severity.label(), d.message);
    }
    println!(
        "Compiled: {}; diagnostics: {}",
        report.compiled,
        report.diagnostics.len()
    );
    Ok(())
}
