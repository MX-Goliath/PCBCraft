//! Update a disposable board from a PcbLib, writing only a new output file.
use eda_core::footprint_update as update;
use std::path::PathBuf;
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 3 {
        return Err(
            "Usage: footprint_update_probe INPUT.PcbDoc SOURCE.PcbLib NEW_OUTPUT.PcbDoc".into(),
        );
    }
    if args[2].exists() {
        return Err("Output must be new".into());
    }
    let doc = eda_io::load(&args[0])?;
    let source = eda_io::altium_pcb::library::load(&args[1])?;
    if !source.warnings.is_empty() {
        return Err(format!("Library warnings: {:?}", source.warnings));
    }
    let changes: Vec<_> = doc
        .pcb
        .as_ref()
        .ok_or("A PCB is required")?
        .components
        .iter()
        .filter_map(|c| {
            source
                .footprints
                .iter()
                .find(|f| f.name().eq_ignore_ascii_case(&c.footprint))
                .map(|f| update::Update {
                    component: c.id,
                    footprint: f.clone(),
                    options: Default::default(),
                    source_library: Some(
                        eda_io::project_files::native_path(&args[1])
                            .to_string_lossy()
                            .into_owned(),
                    ),
                })
        })
        .collect();
    if changes.is_empty() {
        return Err("No matching footprints".into());
    }
    let next = update::preview(&doc, &changes)?;
    eda_io::save(&args[2], &next)?;
    eda_io::save(&args[2].with_extension("json"), &next)?;
    for c in &next.pcb.as_ref().unwrap().components {
        println!(
            "{} / {}: {:?}, rotation {}, native UID {:?}",
            c.designator,
            c.footprint,
            c.position,
            c.rotation,
            c.properties.get("UNIQUEID")
        );
    }
    println!(
        "Updated {} footprints; saved {}",
        changes.len(),
        args[2].display()
    );
    Ok(())
}
