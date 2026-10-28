//! Read-only component identity and native hierarchy diagnostics.
use std::collections::BTreeMap;
fn main() -> Result<(), String> {
    let verbose = std::env::args().any(|a| a == "--verbose");
    let path = std::env::args_os().nth(1).ok_or("Pass a PrjPcb path")?;
    let project = eda_io::altium::load_project(std::path::Path::new(&path))?;
    for entry in &project.documents {
        if entry.kind == "schdoc" {
            let d = eda_io::load(&entry.path)?;
            let s = d.schematic.as_ref().unwrap();
            if let Some(dir) =
                std::env::args().find_map(|a| a.strip_prefix("--dump=").map(str::to_owned))
            {
                let dir = std::path::Path::new(&dir);
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                let file = std::fs::File::create(dir.join(format!("{}.json", d.name)))
                    .map_err(|e| e.to_string())?;
                serde_json::to_writer(std::io::BufWriter::new(file), &d)
                    .map_err(|e| e.to_string())?;
            }
            let excluded = eda_core::schematic_compile::exclusions(s);
            let versions: std::collections::BTreeSet<_> = s
                .records
                .iter()
                .filter(|r| r.record_type == 1)
                .filter_map(|r| r.fields.get("COMPONENTKINDVERSION2"))
                .collect();
            println!("ComponentKindVersion2: {versions:?}");
            let mut refs = BTreeMap::<String, Vec<_>>::new();
            for c in s
                .components
                .iter()
                .filter(|c| !excluded.components.contains(&c.id))
            {
                refs.entry(c.designator.clone()).or_default().push(c);
            }
            println!(
                "{}: {} components, {} excluded by compile masks, {} masks",
                d.name,
                s.components.len(),
                excluded.components.len(),
                s.compile_masks.len()
            );
            for r in s
                .records
                .iter()
                .filter(|r| verbose && matches!(r.record_type, 211))
            {
                println!("Directive {}: {:?}", r.record_type, r.fields);
            }
            for (name, cs) in refs.iter().filter(|(_, cs)| cs.len() > 1) {
                println!("Duplicate {name}:");
                for c in cs {
                    println!("  {} {} record {}", c.designator, c.id, c.source_index);
                    if !verbose {
                        continue;
                    }
                    println!(
                        "  {:?}",
                        s.records
                            .iter()
                            .find(|r| r.index == c.source_index)
                            .map(|r| &r.fields)
                    );
                }
            }
            for r in s.records.iter().filter(|r| verbose && r.record_type == 15) {
                println!("Symbol {:?}", r.fields);
            }
        } else if entry.kind == "pcbdoc" {
            let d = eda_io::load(&entry.path)?;
            let b = d.pcb.as_ref().unwrap();
            if let Some(dir) =
                std::env::args().find_map(|a| a.strip_prefix("--dump=").map(str::to_owned))
            {
                let file =
                    std::fs::File::create(std::path::Path::new(&dir).join("pcb-components.json"))
                        .map_err(|e| e.to_string())?;
                serde_json::to_writer(std::io::BufWriter::new(file), &b.components)
                    .map_err(|e| e.to_string())?;
            }
            println!(
                "PCB {}: {} components / {} nets",
                d.name,
                b.components.len(),
                b.nets.len()
            );
            for c in b.components.iter().take(6).chain(
                b.components
                    .iter()
                    .filter(|c| ["VD12", "C386"].contains(&c.designator.as_str())),
            ) {
                if verbose {
                    println!("PCB {} {:?}", c.designator, c.properties);
                }
            }
        }
    }
    Ok(())
}
