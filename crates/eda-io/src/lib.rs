use eda_core::Document;
pub mod altium;
pub mod altium_pcb;
#[cfg(test)]
mod altium_tests;
#[cfg(test)]
mod library_tests;
pub mod project_files;
pub mod schematic_dxf;
pub mod schematic_graphic;
pub mod schematic_netlist;
pub mod schematic_pdf;
pub mod schematic_reuse;
use std::{io::Write, path::Path};

pub fn load(path: &Path) -> Result<Document, String> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
    {
        return altium::load_schematic(path);
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("PcbDoc"))
    {
        return altium_pcb::load_pcb(path);
    }
    // Own JSON is independent of native import; preserve source archives on Save As.
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024 {
        return Err("Document exceeds the 256 MiB limit".into());
    }
    let mut doc: Document =
        serde_json::from_reader(file).map_err(|e| format!("Invalid PCBCraft document: {e}"))?;
    if let Some(board) = &mut doc.pcb {
        altium_pcb::upgrade_polygons(board, doc.id)?;
        altium_pcb::upgrade_models(board);
        altium_pcb::upgrade_component_parameters(board)?;
    }
    doc.validate()?;
    Ok(doc)
}

pub fn save(path: &Path, doc: &Document) -> Result<(), String> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
    {
        return altium::writer::save(path, doc);
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("PcbDoc"))
    {
        return altium_pcb::writer::save(path, doc, altium_pcb::writer::Format::Preserve);
    }
    if path.extension().is_some_and(|e| {
        ["PrjPcb", "SchLib", "SchDot", "IntLib", "PcbLib", "DbLib"]
            .iter()
            .any(|suffix| e.eq_ignore_ascii_case(suffix))
    }) {
        return Err("Native writing for this Altium document type is not implemented. Choose a .pcbcraft.json destination; the original cannot be overwritten.".into());
    }
    doc.validate()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, doc).map_err(|e| e.to_string())?;
    temp.write_all(b"\n").map_err(|e| e.to_string())?;
    if temp.as_file().metadata().map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024 {
        return Err("Document exceeds the 256 MiB save limit".into());
    }
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn save_reopen_replace_and_reject_future_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("board.json");
        let mut doc = Document::example();
        save(&path, &doc).unwrap();
        assert_eq!(load(&path).unwrap(), doc);
        doc.pads[0].position.x += 17;
        save(&path, &doc).unwrap();
        assert_eq!(load(&path).unwrap(), doc);
        let original = std::fs::read(&path).unwrap();
        doc.schema_version = 2;
        assert!(save(&path, &doc).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        assert!(
            load(&path)
                .unwrap_err()
                .contains("Unsupported document version")
        );
        doc.schema_version = 1;
        let mut extended = serde_json::to_value(&doc).unwrap();
        extended["future_feature"] = serde_json::json!({"data":42});
        std::fs::write(&path, serde_json::to_vec(&extended).unwrap()).unwrap();
        assert!(load(&path).unwrap_err().contains("unknown field"));
    }
}

#[cfg(test)]
mod layer_stack_tests {
    use super::*;
    use eda_core::{
        DocumentKind,
        layer_stack::{Impedance, Stack, ViaKind, ViaType},
        pcb::Board,
    };
    #[test]
    fn edited_stack_json_roundtrip_and_native_refusal_preserve_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stack.pcbcraft.json");
        let mut doc = Document::new(DocumentKind::Board, "stack");
        let b = Board::default();
        let mut s = Stack::from_board(&b);
        s.add_copper(1).unwrap();
        s.vias.push(ViaType {
            id: uuid::Uuid::new_v4(),
            name: "Laser".into(),
            kind: ViaKind::Micro,
            start: 1,
            end: 2,
            diameter: 300_000,
            hole: 100_000,
        });
        s.impedances.push(Impedance {
            id: uuid::Uuid::new_v4(),
            name: "USB".into(),
            differential: true,
            target: 90.,
            tolerance: 10.,
            signal: 1,
            reference: 2,
            reference2: None,
            width: 200_000,
            gap: 150_000,
        });
        doc.pcb = Some(s.apply(&b).unwrap());
        save(&path, &doc).unwrap();
        let read = load(&path).unwrap();
        assert_eq!(read, doc);
        let native = dir.path().join("original.PcbDoc");
        std::fs::write(&native, b"untouched").unwrap();
        assert!(save(&native, &doc).unwrap_err().contains("PCBCraft JSON"));
        assert_eq!(std::fs::read(native).unwrap(), b"untouched");
    }
}
