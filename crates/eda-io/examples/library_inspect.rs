fn main() {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        match eda_io::altium::library::load(std::path::Path::new(&path)) {
            Ok(library) => {
                println!(
                    "LIBRARY {}: {} symbols",
                    library.name,
                    library.symbols.len()
                );
                for symbol in library.symbols {
                    let f = &symbol.variants[0].fragment;
                    println!(
                        "  {} parts={} modes={} pins={} comment={} primitives={}",
                        symbol.name,
                        symbol.parts,
                        symbol.display_modes,
                        f.primitives
                            .iter()
                            .filter(|p| matches!(
                                p.kind,
                                eda_core::schematic::SchPrimitiveKind::Pin { .. }
                            ))
                            .count(),
                        f.components[0].comment,
                        f.primitives.len()
                    );
                }
                for warning in library.warnings {
                    println!("  WARNING {warning}");
                }
            }
            Err(error) => {
                eprintln!("ERROR {path}: {error}");
                failed = true;
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
