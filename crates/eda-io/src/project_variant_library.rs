//! The native project alternate cache is a SchLib with ordinary symbol names.
use super::*;
use eda_core::{Document, DocumentKind, library_edit as edit, schematic_edit::Fragment};

fn member(fragment: &Fragment) -> Result<Document, String> {
    let c = fragment
        .components
        .first()
        .ok_or("Alternate has no component")?;
    if c.settings.part_count != 1 || c.part != 1 || c.display_mode != 0 {
        return Err(
            "Native alternate export requires the complete multipart/display-mode library".into(),
        );
    }
    let fragment = fragment.transformed(
        c.position,
        0,
        eda_geom::Point {
            x: -c.position.x,
            y: -c.position.y,
        },
    )?;
    let mut d = edit::member(DocumentKind::Schematic, &c.library_reference)?;
    let s = d.schematic.as_mut().unwrap();
    s.components = fragment.components;
    s.primitives = fragment.primitives;
    s.fonts = fragment.fonts;
    s.images = fragment.images;
    s.parameter_objects.clear();
    Ok(d)
}
fn signature(doc: &Document) -> Result<String, String> {
    let mut s = doc.schematic.as_ref().unwrap().clone();
    s.records = Default::default();
    s.source_modified = false;
    let c = &mut s.components[0];
    c.id = uuid::Uuid::nil();
    c.source_index = u32::MAX;
    c.designator = "?".into();
    for (n, p) in s
        .primitives
        .iter_mut()
        .chain(&mut s.parameter_objects)
        .enumerate()
    {
        p.id = uuid::Uuid::from_u128(n as u128 + 1);
        p.component = Some(uuid::Uuid::nil());
        p.source_index = u32::MAX;
        if p.text_role == Some(eda_core::schematic::TextRole::Designator) {
            p.presentation.raw_text = Some("?".into());
            if let eda_core::schematic::SchPrimitiveKind::Text { text, .. } = &mut p.kind {
                *text = "?".into();
            }
        }
    }
    serde_json::to_string(&s).map_err(|e| e.to_string())
}

pub(super) fn prepare(
    project: &altium::Project,
    destination: &Path,
) -> Result<(Vec<Variant>, Option<Vec<u8>>), String> {
    let mut variants = project.variants.clone();
    let mut members = BTreeMap::<String, Document>::new();
    let path = destination.with_extension("PrjPcbVariants");
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    for c in variants.iter_mut().flat_map(|v| &mut v.variations) {
        let Some(a) = &c.alternate else { continue };
        let d = member(a)?;
        let name = edit::name(&d).to_owned();
        let key = name.to_lowercase();
        if let Some(old) = members.get(&key) {
            if signature(old)? != signature(&d)? {
                return Err(format!(
                    "Alternates named {name} have different symbols; use distinct library references"
                ));
            }
        } else {
            members.insert(key, d);
        }
        set(&mut c.fields, "AlternatePart", String::new());
        set(
            &mut c.fields,
            "AltLibLink_LibIdentifierKind",
            "Library Full Path".into(),
        );
        set(
            &mut c.fields,
            "AltLibLink_LibraryIdentifier",
            path.to_string_lossy().into_owned(),
        );
        set(&mut c.fields, "AltLibLink_DesignItemID", name);
        set(
            &mut c.fields,
            "AltLibLink_SourceLibraryName",
            filename.clone(),
        );
        set(&mut c.fields, "AltLibLink_UseLibraryName", "True".into());
        set(&mut c.fields, "AltLibLink_UseDBTableName", "False".into());
    }
    if members.is_empty() {
        return Ok((
            variants,
            std::fs::read(project.path.with_extension("PrjPcbVariants")).ok(),
        ));
    }
    // Retain unused native symbols and opaque container streams. The existing
    // library writer refuses records/auxiliary streams it cannot safely preserve.
    let source_path = project.path.with_extension("PrjPcbVariants");
    let mut source = None;
    if source_path.exists() {
        let old = crate::library_documents::load(&source_path)?;
        source = old.library.as_ref().and_then(|b| b.source.clone());
        for m in edit::members(&old) {
            if !members.contains_key(&edit::name(m).to_lowercase()) {
                let mut m = m.clone();
                m.library = None;
                members.insert(
                    format!(
                        "{}:{}:{}",
                        edit::name(&m).to_lowercase(),
                        edit::variant(&m).0,
                        edit::variant(&m).1
                    ),
                    m,
                );
            }
        }
    }
    let library = edit::assemble(members.into_values().collect(), &filename, source)?;
    Ok((
        variants,
        Some(crate::library_documents::serialize(&library)?),
    ))
}
