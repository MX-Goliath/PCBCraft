//! Read-only footprint-container diagnostic.
use std::io::{Cursor, Read};
fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PcbLib path")?;
    let bytes = eda_io::altium::read_bounded(std::path::Path::new(&path))?;
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let paths: Vec<_> = file
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_owned(), e.len()))
        .collect();
    for (path, len) in paths
        .iter()
        .filter(|_| std::env::args().any(|a| a == "--inspect"))
    {
        let mut bytes = vec![];
        file.open_stream(path)
            .map_err(|e| e.to_string())?
            .take(130)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        println!("{} ({len}): {bytes:02x?}", path.display());
    }
    println!("{} streams", paths.len());
    let imported = eda_io::altium_pcb::library::load(std::path::Path::new(&path))?;
    println!("Decoded {} footprints", imported.footprints.len());
    for f in imported.footprints.iter().take(12) {
        println!("Footprint: {}", f.name());
    }
    for w in imported.warnings {
        println!("Warning: {w}");
    }
    Ok(())
}
