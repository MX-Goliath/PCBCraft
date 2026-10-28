//! Read-only corpus comparison, with optional isolated native synchronization QA copies.
use eda_core::{
    schematic::SchPrimitiveKind as K,
    schematic_edit as edit,
    schematic_sync::{self as sync, Destination, Signal},
};
use eda_geom::Point;
use std::path::PathBuf;
use uuid::Uuid;
fn main() -> Result<(), String> {
    let project_path = std::env::args_os().nth(1).ok_or("Pass a project path")?;
    let project = eda_io::altium::load_project(std::path::Path::new(&project_path))?;
    let mut docs = vec![];
    for entry in project
        .documents
        .iter()
        .filter(|e| e.kind == "schdoc" && e.exists)
    {
        docs.push((entry.path.clone(), eda_io::load(&entry.path)?));
    }
    let mut count = 0;
    let mut connector = None;
    for (i, (path, parent)) in docs.iter().enumerate() {
        for p in &parent.schematic.as_ref().unwrap().primitives {
            if let K::SheetSymbol {
                designator,
                filename,
                ..
            } = &p.kind
            {
                let entry = project
                    .resolve_sheet(path, filename)
                    .ok_or("Unresolved child")?;
                let c = docs
                    .iter()
                    .position(|(p, _)| p == &entry.path)
                    .ok_or("Missing child")?;
                let compare = sync::compare(parent, &docs[c].1, p.id, &[])?;
                println!(
                    "{designator} -> {filename}: {} links, {} unmatched entries, {} unmatched ports",
                    compare.links.len(),
                    compare.entries.len(),
                    compare.ports.len()
                );
                count += compare.links.len();
                if filename.eq_ignore_ascii_case("Connector.SchDoc") {
                    connector = Some((i, c, p.id));
                }
            }
        }
    }
    println!("Total matching pairs: {count}");
    if let Some(out) = std::env::args_os().nth(2) {
        let out = PathBuf::from(out);
        if out.exists() {
            return Err("QA destination already exists; choose a fresh directory".into());
        }
        let (p, c, symbol) = connector.ok_or("Connector sheet not found for QA")?;
        for (destination, target, name, at) in [
            (
                Destination::Entries(symbol),
                p,
                "QA_ENTRY_ONLY",
                Point::mm(75, 220),
            ),
            (Destination::Ports, c, "QA_PORT_ONLY", Point::mm(30, 15)),
        ] {
            let signal = Signal {
                id: Uuid::new_v4(),
                name: name.into(),
                io: 2,
                harness: String::new(),
            };
            let plan = sync::transfer(&docs[target].1, destination, &[signal], &[])?;
            docs[target].1 = edit::preview(&docs[target].1, &plan.edit_at(&docs[target].1, at)?)?;
        }
        let port = docs[c]
            .1
            .schematic
            .as_ref()
            .unwrap()
            .primitives
            .iter()
            .find(|p| matches!(&p.kind,K::Port{name,..}if name=="KL30"))
            .ok_or("KL30 missing")?;
        let mut value = port.clone();
        if let K::Port { io_type, .. } = &mut value.kind {
            *io_type = 2;
        }
        docs[c].1 = edit::preview(
            &docs[c].1,
            &edit::Edit::PrimitiveProperties {
                id: port.id,
                value: Box::new(value),
            },
        )?;
        // Serialize every file before creating the isolated output directory.
        let bytes: Vec<_> = docs
            .iter()
            .map(|(_, d)| eda_io::altium::writer::serialize(d))
            .collect::<Result<_, _>>()?;
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let mut text = String::from("[Design]\nVersion=1.0\n");
        for (n, ((_, doc), bytes)) in docs.iter().zip(bytes).enumerate() {
            std::fs::write(out.join(&doc.name), bytes).map_err(|e| e.to_string())?;
            text.push_str(&format!("[Document{}]\nDocumentPath={}\n", n + 1, doc.name));
        }
        let path = out.join("SheetSync_QA.PrjPcb");
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
        println!("QA project: {}", path.display());
    }
    Ok(())
}
