use eda_core::Document;
use std::{io::Write, path::Path};

pub fn load(path: &Path) -> Result<Document, String> {
    // Bound reads before decoding untrusted input. Native Altium files are unsupported.
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
        return Err("Document exceeds the initial 16 MiB limit".into());
    }
    let doc: Document =
        serde_json::from_reader(file).map_err(|e| format!("Invalid PCBCraft document: {e}"))?;
    doc.validate()?;
    Ok(doc)
}

pub fn save(path: &Path, doc: &Document) -> Result<(), String> {
    doc.validate()?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, doc).map_err(|e| e.to_string())?;
    temp.write_all(b"\n").map_err(|e| e.to_string())?;
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
