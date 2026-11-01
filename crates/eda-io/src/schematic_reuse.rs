//! Local reusable schematic fragments. Native SchDoc files can also be placed directly.
use eda_core::{
    Document, DocumentKind,
    schematic_edit::{self as edit, Edit, Selection},
};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
#[derive(Clone, Debug)]
pub struct Snippet {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub metadata: Option<PathBuf>,
}
fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() < 120
        && !name.contains([
            '<', '>', ':', '"', '/', '\\', '|', '?', '*', '\n', '\r', '\0',
        ])
        && !name.ends_with(['.', ' '])
        && name != "."
        && name != ".."
}
pub fn save(
    folder: &Path,
    name: &str,
    description: &str,
    doc: &Document,
    selection: &Selection,
) -> Result<Snippet, String> {
    if !valid_name(name) {
        return Err("Enter a valid snippet name".into());
    }
    if description.len() > 8192 {
        return Err("Snippet description is too long".into());
    }
    let s = doc.schematic.as_ref().ok_or("Select schematic objects")?;
    let fragment = edit::extract_selection(s, selection)?.fresh_copy();
    let snapshot = edit::preview(
        &Document::new(DocumentKind::Schematic, name),
        &Edit::Insert(fragment),
    )?;
    let path = folder.join(format!("{name}.pcbcraft.json"));
    let metadata = folder.join(format!("{name}.snippet.json"));
    if path.exists() || metadata.exists() {
        return Err("A snippet with that name already exists".into());
    }
    let mut temp = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut temp, &snapshot).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    let mut meta = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({"version":1,"name":name,"description":description,"document":path.file_name().unwrap().to_string_lossy()});
    serde_json::to_writer_pretty(&mut meta, &manifest).map_err(|e| e.to_string())?;
    meta.write_all(b"\n").map_err(|e| e.to_string())?;
    meta.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist_noclobber(&path).map_err(|e| e.to_string())?;
    if let Err(e) = meta.persist_noclobber(&metadata) {
        let _ = std::fs::remove_file(&path);
        return Err(e.to_string());
    }
    Ok(Snippet {
        name: name.into(),
        description: description.into(),
        path,
        metadata: Some(metadata),
    })
}
pub fn scan(folders: &[PathBuf]) -> Result<Vec<Snippet>, String> {
    let mut snippets = vec![];
    for folder in folders {
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        let mut native = vec![];
        for entry in entries.take(5001) {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path
                .file_name()
                .is_some_and(|n| n.to_string_lossy().ends_with(".snippet.json"))
            {
                if entry.metadata().map_err(|e| e.to_string())?.len() > 64 * 1024 {
                    continue;
                }
                let mut data = vec![];
                std::fs::File::open(&path)
                    .map_err(|e| e.to_string())?
                    .take(64 * 1024 + 1)
                    .read_to_end(&mut data)
                    .map_err(|e| e.to_string())?;
                if data.len() > 64 * 1024 {
                    continue;
                }
                let value: serde_json::Value =
                    serde_json::from_slice(&data).map_err(|e| e.to_string())?;
                let Some(filename) = value["document"].as_str() else {
                    continue;
                };
                if Path::new(filename).components().count() != 1 || filename.contains(['\\', '/']) {
                    continue;
                }
                let document = folder.join(filename);
                if !document.is_file() {
                    continue;
                }
                snippets.push(Snippet {
                    name: value["name"].as_str().unwrap_or(filename).into(),
                    description: value["description"].as_str().unwrap_or_default().into(),
                    path: document,
                    metadata: Some(path),
                });
            } else if path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("SchDoc"))
            {
                native.push(path);
            }
            if snippets.len() > 5000 {
                return Err("Snippet folder contains too many files".into());
            }
        }
        for path in native {
            if !snippets.iter().any(|s| s.path == path) {
                snippets.push(Snippet {
                    name: path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    description: String::new(),
                    path,
                    metadata: None,
                });
            }
        }
    }
    snippets.sort_by_key(|s| s.name.to_lowercase());
    snippets.dedup_by(|a, b| a.path == b.path);
    Ok(snippets)
}
pub fn properties(snippet: &Snippet, name: &str, description: &str) -> Result<(), String> {
    if !valid_name(name) {
        return Err("Enter a valid snippet name".into());
    }
    if description.len() > 8192 {
        return Err("Snippet description is too long".into());
    }
    let path = snippet
        .metadata
        .as_ref()
        .ok_or("This schematic has no snippet properties file")?;
    let folder = path.parent().ok_or("Invalid snippet folder")?;
    let mut temp = tempfile::NamedTempFile::new_in(folder).map_err(|e| e.to_string())?;
    let value = serde_json::json!({"version":1,"name":name,"description":description,"document":snippet.path.file_name().unwrap_or_default().to_string_lossy()});
    serde_json::to_writer_pretty(&mut temp, &value).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eda_geom::Point;
    #[test]
    fn local_snippet_keeps_only_selected_objects_and_reopens_without_native_archive() {
        let folder = tempfile::tempdir().unwrap();
        let mut d = Document::new(DocumentKind::Schematic, "Private source");
        for x in [10, 40] {
            let f = edit::place(
                edit::Placement::VoltageSource,
                Point::mm(x, 10),
                0,
                &format!("V{x}"),
                "5",
                0,
            )
            .unwrap();
            d = edit::preview(&d, &Edit::Insert(f)).unwrap();
        }
        let s = d.schematic.as_ref().unwrap();
        let selected = s.components[0].id;
        let saved = save(
            folder.path(),
            "Supply",
            "5 V source",
            &d,
            &Selection {
                ids: vec![selected],
                segments: vec![],
            },
        )
        .unwrap();
        assert!(
            save(
                folder.path(),
                "Supply",
                "",
                &d,
                &Selection {
                    ids: vec![selected],
                    segments: vec![]
                }
            )
            .is_err()
        );
        let read = crate::load(&saved.path).unwrap();
        let s = read.schematic.as_ref().unwrap();
        assert_eq!(s.components.len(), 1);
        assert!(s.source.is_none());
        assert!(s.records.is_empty());
        assert_ne!(s.components[0].id, selected);
        properties(&saved, "Supply revision 2", "Verified").unwrap();
        let scanned = scan(&[folder.path().to_owned()]).unwrap();
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].name, "Supply revision 2");
    }
}
