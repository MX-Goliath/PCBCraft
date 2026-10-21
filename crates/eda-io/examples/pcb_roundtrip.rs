//! Edited native round-trip smoke test. Always writes a NEW output file.
//! This tests PCBCraft against its own reader, not against Altium Designer.
use eda_core::{pcb::*, pcb_polygon, pcb_tools};
use eda_geom::Point;
use eda_io::altium_pcb::{
    self,
    writer::{self, Format},
};
use std::{io::Write, path::PathBuf};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&args.len()) {
        return Err("Usage: cargo run -p eda-io --example pcb_roundtrip -- INPUT.PcbDoc NEW_OUTPUT.PcbDoc [--exercise]".into());
    }
    let source = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let exercise = if let Some(flag) = args.get(2) {
        if flag.as_os_str() != std::ffi::OsStr::new("--exercise") {
            return Err("Unknown flag; expected --exercise".into());
        }
        true
    } else {
        false
    };
    if output.exists() || std::fs::symlink_metadata(&output).is_ok() {
        return Err("Output must not exist; this tool never overwrites a project".into());
    }
    let original = std::fs::read(&source).map_err(|e| e.to_string())?;
    let mut doc = altium_pcb::import_pcb(
        &original,
        &source.file_name().unwrap_or_default().to_string_lossy(),
    )?;
    let mut edits = Vec::new();
    if exercise {
        let mut b = doc.pcb.clone().ok_or("Not a PCB scene")?;
        if let Some(c) = b
            .components
            .iter()
            .find(|c| {
                !c.locked
                    && !b
                        .items
                        .iter()
                        .any(|i| i.component == Some(c.index) && i.locked)
            })
            .cloned()
        {
            let mut edited = c;
            edited.position.x += 250_000;
            b = pcb_tools::edit_component(&b, edited)?;
            edits.push("component translation");
        }
        for kind in ["Track", "Pad", "Via"] {
            if let Some(i) = b
                .items
                .iter()
                .find(|i| {
                    i.kind() == kind && !i.locked && i.polygon.is_none() && i.component.is_none()
                })
                .cloned()
            {
                let mut edited = i;
                match &mut edited.shape {
                    Shape::Track { width, .. } => *width += 25_400,
                    Shape::Pad { size, .. } => size.x += 25_400,
                    Shape::Via { diameter, .. } => *diameter += 25_400,
                    _ => {}
                }
                b = b.edited(Edit::Replace(edited))?;
                edits.push(kind);
            }
        }
        let bounds = b.bounds();
        let a = Point {
            x: bounds.min.x + 1_000_000,
            y: bounds.min.y + 1_000_000,
        };
        let end = Point {
            x: a.x + 2_000_000,
            y: a.y + 1_000_000,
        };
        let net = b.nets.first().map(|n| n.index);
        b = b.edited(Edit::Insert(vec![Item::new(
            TOP,
            net,
            Shape::Track {
                start: a,
                end,
                width: 250_000,
            },
        )]))?;
        edits.push("new track");
        if let Some(p) = b.polygon_pours.iter().find(|p| !p.locked).cloned() {
            let mut p = p;
            p.clearance += 25_400;
            b = pcb_tools::replace_polygon(&b, p)?;
            edits.push("polygon clearance and repour");
        }
        if !b.polygon_pours.is_empty() {
            b = pcb_polygon::repour(&b)?;
        }
        doc.pcb = Some(b);
    }
    let bytes = writer::serialize(&doc, Format::Preserve)?;
    if exercise && original == bytes {
        return Err("Exercise did not change serialized bytes".into());
    }
    let reread = altium_pcb::import_pcb(&bytes, "roundtrip.PcbDoc")?;
    let b = reread.pcb.ok_or("Serialized document has no PCB model")?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(std::path::Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist_noclobber(&output).map_err(|e| e.to_string())?;
    println!(
        "{}",
        serde_json::json!({"status":"own_reader_roundtrip_passed","source":source,"output":output,
        "exercise":exercise,"edits":edits,"bytes":bytes.len(),"primitives":b.items.len(),"components":b.components.len(),
        "nets":b.nets.len(),"polygons":b.polygon_pours.len(),"altium_oracle_tested":false,"warnings":b.warnings})
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
