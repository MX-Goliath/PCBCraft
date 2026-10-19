use eda_core::schematic::SchPrimitiveKind;
use std::path::Path;

fn main() {
    let mut failed = false;
    for arg in std::env::args().skip(1) {
        let path = Path::new(&arg);
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("PrjPcb"))
        {
            match eda_io::altium::load_project(path) {
                Ok(p) => {
                    println!("PROJECT {}: {} documents", p.name, p.documents.len());
                    for d in p.documents {
                        println!("  {} [{}] exists={}", d.original_path, d.kind, d.exists);
                    }
                    for w in p.warnings {
                        println!("  WARNING: {w}");
                    }
                }
                Err(e) => {
                    eprintln!("ERROR {}: {e}", path.display());
                    failed = true;
                }
            }
        } else {
            match eda_io::altium::load_schematic(path) {
                Ok(doc) => {
                    let s = doc.schematic.unwrap();
                    println!("SCHEMATIC {}: {}x{} nm, {} records, {} components, {} primitives, {} wires, {} pins",doc.name,doc.size.x,doc.size.y,s.records.len(),s.components.len(),s.primitives.len(),s.primitives.iter().filter(|p|matches!(&p.kind,SchPrimitiveKind::Polyline{electrical,..} if electrical=="Wire")).count(),s.primitives.iter().filter(|p|matches!(p.kind,SchPrimitiveKind::Pin{..})).count());
                    for w in s.warnings {
                        println!("  WARNING: {w}");
                    }
                }
                Err(e) => {
                    eprintln!("ERROR {}: {e}", path.display());
                    failed = true;
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
