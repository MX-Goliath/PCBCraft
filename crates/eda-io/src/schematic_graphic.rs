//! Read bounded image resources before placing them in a schematic.
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::{
    schematic::SchPrimitiveKind as K,
    schematic_edit::{self as edit, Fragment},
};
use eda_geom::Point;
use std::{io::Read, path::Path};

pub fn load(path: &Path, embedded: bool) -> Result<Fragment, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 32 * 1024 * 1024 {
        return Err("Schematic graphic exceeds 32 MiB".into());
    }
    let mut bytes = vec![];
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Schematic graphic exceeds 32 MiB".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let pixels = reader
        .decode()
        .map_err(|e| format!("Cannot decode graphic: {e}"))?;
    let filename = path.to_string_lossy().into_owned();
    let key = filename.replace('\\', "/").to_lowercase();
    let height = 2_540_000;
    let width = (height as f64 * pixels.width() as f64 / pixels.height() as f64).round() as i64;
    let mut p = edit::primitive(
        K::Image {
            a: Point::default(),
            b: Point {
                x: width,
                y: height,
            },
            filename,
            keep_aspect: true,
            embedded,
        },
        None,
    );
    p.presentation.record_type = 30;
    p.presentation.allow_sync = true;
    p.width_nm = 101_600;
    p.color = 0;
    Ok(Fragment {
        images: std::collections::BTreeMap::from([(key, STANDARD.encode(bytes))]).into(),
        primitives: vec![p],
        ..Default::default()
    })
}

pub(crate) fn cache_external(doc: &mut eda_core::Document, source: &Path) {
    let Some(s) = &mut doc.schematic else { return };
    let mut budget = 64 * 1024 * 1024;
    for p in &s.primitives {
        let K::Image {
            filename,
            embedded: false,
            ..
        } = &p.kind
        else {
            continue;
        };
        let key = filename.replace('\\', "/").to_lowercase();
        if s.images.contains_key(&key) || key.starts_with("//") {
            continue;
        }
        let path = Path::new(filename);
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            source.parent().unwrap_or(Path::new(".")).join(filename)
        };
        let Ok(metadata) = path.metadata() else {
            continue;
        };
        if metadata.len() > budget {
            continue;
        }
        if let Ok(fragment) = load(&path, false)
            && let Some(encoded) = fragment.images.values().next()
        {
            budget = budget.saturating_sub(metadata.len());
            std::sync::Arc::make_mut(&mut s.images).insert(key, encoded.clone());
        }
    }
}
