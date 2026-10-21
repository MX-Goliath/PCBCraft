//! Inspect PCB import without opening a GUI. Does not claim DRC or native parity.
use eda_core::Document;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or(
        "Usage: pcb_inspect INPUT.PcbDoc [--save-json OUTPUT.pcbcraft.json] [--extract-original ORIGINAL-COPY.PcbDoc]",
    )?);
    let mut save_json = None;
    let mut extract = None;
    while let Some(flag) = args.next() {
        let destination = PathBuf::from(args.next().ok_or("Missing output filename")?);
        if flag == "--save-json" && save_json.is_none() {
            save_json = Some(destination);
        } else if flag == "--extract-original" && extract.is_none() {
            extract = Some(destination);
        } else {
            return Err(format!(
                "Unknown or repeated option: {}",
                flag.to_string_lossy()
            ));
        }
    }
    let doc: Document = if input
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pcbdoc"))
    {
        eda_io::altium_pcb::load_pcb(&input)?
    } else {
        eda_io::load(&input)?
    };
    let board = doc
        .pcb
        .as_ref()
        .ok_or("Document has no PCB scene; legacy JSON requires migration in the editor")?;
    let mut counts = BTreeMap::new();
    for item in &board.items {
        *counts.entry(item.kind()).or_insert(0usize) += 1;
    }
    let bounds = board.bounds();
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "file": input,
            "document_id": doc.id,
            "units": "nanometres",
            "items": board.items.len(),
            "counts": counts,
            "nets": board.nets.len(),
            "components": board.components.len(),
            "layers": board.layers,
            "outline_vertices": board.outline.len(),
            "bounds": { "min": bounds.min, "max": bounds.max },
            "polygons": board.polygon_pours.len(),
            "source_polygon_records": board.polygons.len(),
            "pours_stale": board.pours_stale,
            "archived_source": board.source.is_some(),
            "warnings": board.warnings,
            "native_modified_pcbdoc_writer": "experimental_subset",
            "altium_interoperability_verified": false,
            "drc_performed": false,
            "altium_visual_parity_verified": false
        }))
        .map_err(|error| error.to_string())?
    );
    if let Some(path) = save_json {
        if path.exists() {
            return Err(format!("Refusing to overwrite {}", path.display()));
        }
        eda_io::save(&path, &doc)?;
    }
    if let Some(path) = extract {
        // This extracts the original bytes; it NEVER serializes the modified board.
        eda_io::altium_pcb::restore_original(&path, &doc)?;
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PCB inspection failed: {error}");
        std::process::exit(1);
    }
}
