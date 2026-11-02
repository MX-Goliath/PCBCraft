//! Native project/group file operations. Unknown INI sections survive Save As.
use crate::altium::{self, Project};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

pub(crate) fn decode(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(&[255, 254]) {
        if !bytes.len().is_multiple_of(2) {
            return Err("Truncated UTF-16 INI file".into());
        }
        String::from_utf16(
            &bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
        .map_err(|e| e.to_string())
    } else {
        Ok(std::str::from_utf8(bytes)
            .map(|s| s.trim_start_matches('\u{feff}').to_owned())
            .unwrap_or_else(|_| encoding_rs::WINDOWS_1251.decode(bytes).0.into_owned()))
    }
}

pub fn absolute(base: &Path, name: &str) -> PathBuf {
    let path = PathBuf::from(name.replace('\\', "/"));
    let path = if path.is_absolute() || name.as_bytes().get(1) == Some(&b':') {
        path
    } else {
        base.join(path)
    };
    path.canonicalize().unwrap_or(path)
}

fn reference(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('/', "\\")
}

pub fn create_project(path: &Path) -> Result<Project, String> {
    if path.exists() {
        return Err("A project already exists at this location".into());
    }
    write_atomic(path, b"[Design]\r\nVersion=1.0\r\nHierarchyMode=0\r\n")?;
    altium::load_project(path)
}

/// Rebase every document reference, preserving options, variants and unknown keys.
pub fn save_project_as(project: &Project, destination: &Path) -> Result<Project, String> {
    let text = crate::project_variants::rewrite(
        &decode(&altium::read_bounded(&project.path)?)?,
        &project.variants,
    )?;
    let old_base = project.path.parent().unwrap_or(Path::new("."));
    let new_base = destination.parent().unwrap_or(Path::new("."));
    let mut output = String::new();
    let mut document = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed[1..trimmed.len() - 1].to_ascii_lowercase();
            document = ["document", "generateddocument"].iter().any(|prefix| {
                section
                    .strip_prefix(prefix)
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            });
        }
        if document
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("DocumentPath")
        {
            let resolved = absolute(old_base, value.trim().trim_matches('"'));
            output.push_str(&format!("{key}={}\r\n", reference(new_base, &resolved)));
        } else {
            output.push_str(line);
            output.push_str("\r\n");
        }
    }
    // UTF-16LE is unambiguous for non-ASCII paths and accepted by the native reader.
    let bytes: Vec<_> = [255, 254]
        .into_iter()
        .chain(output.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    write_atomic(destination, &bytes)?;
    altium::load_project(destination)
}

pub fn add_document(project: &Project, path: &Path) -> Result<Project, String> {
    if project.documents.iter().any(|e| e.path == path) {
        return Ok(project.clone());
    }
    let text = decode(&altium::read_bounded(&project.path)?)?;
    let mut index = 1;
    let lower = text.to_ascii_lowercase();
    while lower
        .lines()
        .any(|l| l.trim() == format!("[document{index}]"))
    {
        index += 1;
    }
    let path = reference(project.path.parent().unwrap_or(Path::new(".")), path);
    let text =
        format!("{text}\r\n[Document{index}]\r\nDocumentPath={path}\r\nAnnotationEnabled=1\r\n");
    let bytes: Vec<_> = [255, 254]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    write_atomic(&project.path, &bytes)?;
    altium::load_project(&project.path)
}

pub fn replace_document(project: &Project, old: &Path, new: &Path) -> Result<Project, String> {
    let text = decode(&altium::read_bounded(&project.path)?)?;
    let base = project.path.parent().unwrap_or(Path::new("."));
    let mut output = String::new();
    let mut document = false;
    let mut found = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed[1..trimmed.len() - 1].to_ascii_lowercase();
            document = section
                .strip_prefix("document")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        }
        if document
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("DocumentPath")
            && absolute(base, value.trim().trim_matches('"')) == old
        {
            output.push_str(&format!("{key}={}\r\n", reference(base, new)));
            found = true;
        } else {
            output.push_str(line);
            output.push_str("\r\n");
        }
    }
    if !found {
        return Err("The project no longer contains the renamed document".into());
    }
    let bytes: Vec<_> = [255, 254]
        .into_iter()
        .chain(output.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    write_atomic(&project.path, &bytes)?;
    altium::load_project(&project.path)
}

pub fn load_group(path: &Path) -> Result<Vec<PathBuf>, String> {
    let text = decode(&altium::read_bounded(path)?)?;
    if !text
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case("[ProjectGroup]"))
    {
        return Err("Not an Altium project group: [ProjectGroup] missing".into());
    }
    let mut result = Vec::new();
    let mut section = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1]
                .to_ascii_lowercase()
                .strip_prefix("project")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        } else if section
            && let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("ProjectPath")
            && !value.trim().is_empty()
        {
            let p = absolute(
                path.parent().unwrap_or(Path::new(".")),
                value.trim().trim_matches('"'),
            );
            if !result.contains(&p) {
                result.push(p);
            }
            if result.len() > 1000 {
                return Err("Project group exceeds 1000 projects".into());
            }
        }
    }
    Ok(result)
}

pub fn save_group(path: &Path, projects: &[Project]) -> Result<(), String> {
    let mut text = String::from("[ProjectGroup]\r\nVersion=1.0\r\n");
    for (i, p) in projects.iter().enumerate() {
        text.push_str(&format!(
            "\r\n[Project{}]\r\nProjectPath={}\r\n",
            i + 1,
            reference(path.parent().unwrap_or(Path::new(".")), &p.path)
        ));
    }
    let bytes: Vec<_> = [255, 254]
        .into_iter()
        .chain(text.encode_utf16().flat_map(u16::to_le_bytes))
        .collect();
    write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_and_group_rebase_and_preserve_options() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let dest = dir.path().join("dest");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        let sheet = source.join("Схема.SchDoc");
        std::fs::write(&sheet, "fixture").unwrap();
        let p = source.join("board.PrjPcb");
        let project = create_project(&p).unwrap();
        let project = add_document(&project, &sheet.canonicalize().unwrap()).unwrap();
        let mut text = decode(&std::fs::read(&p).unwrap()).unwrap();
        text.push_str("\r\n[Unknown]\r\nFuture=42\r\n");
        std::fs::write(&p, text).unwrap();
        let saved = save_project_as(&project, &dest.join("copy.PrjPcb")).unwrap();
        assert_eq!(saved.documents[0].path, sheet.canonicalize().unwrap());
        assert!(saved.documents[0].exists);
        assert!(
            decode(&std::fs::read(&saved.path).unwrap())
                .unwrap()
                .contains("Future=42")
        );
        let group = dest.join("group.DsnWrk");
        save_group(&group, std::slice::from_ref(&saved)).unwrap();
        assert_eq!(
            load_group(&group).unwrap(),
            vec![saved.path.canonicalize().unwrap()]
        );
        assert!(create_project(&p).is_err());
        let json = dest.join("Renamed.pcbcraft.json");
        let document = eda_core::Document::new(eda_core::DocumentKind::Schematic, "Renamed");
        crate::save(&json, &document).unwrap();
        let renamed = replace_document(
            &saved,
            &sheet.canonicalize().unwrap(),
            &json.canonicalize().unwrap(),
        )
        .unwrap();
        assert_eq!(renamed.documents[0].kind, "schdoc");
        assert_eq!(renamed.documents[0].path, json.canonicalize().unwrap());
        assert!(
            decode(&std::fs::read(&renamed.path).unwrap())
                .unwrap()
                .contains("Future=42")
        );
    }
}
