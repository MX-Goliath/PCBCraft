//! Altium schematic import. Generic CFB storage only; no Altium/KiCad runtime.
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::{Document, DocumentKind, schematic::*};
use eda_geom::Point;
use encoding_rs::{Encoding, WINDOWS_1251, WINDOWS_1252};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_FILE: usize = 128 * 1024 * 1024;
const MAX_RECORDS: usize = 500_000;

#[derive(Clone, Debug)]
pub struct ProjectEntry {
    pub original_path: String,
    pub path: PathBuf,
    pub kind: String,
    pub exists: bool,
}
#[derive(Clone, Debug)]
pub struct Project {
    pub path: PathBuf,
    pub name: String,
    pub documents: Vec<ProjectEntry>,
    pub warnings: Vec<String>,
}

pub fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE as u64 {
        return Err("File exceeds 128 MiB import limit".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("File exceeds 128 MiB import limit".into());
    }
    Ok(bytes)
}

fn resolve_path(base: &Path, name: &str) -> PathBuf {
    let normalized = name.replace('\\', "/");
    let p = PathBuf::from(&normalized);
    let p = if p.is_absolute() || normalized.as_bytes().get(1) == Some(&b':') {
        p
    } else {
        base.join(p)
    };
    if p.exists() {
        return p;
    }
    // Windows project paths may differ in case. Resolve one unique match at a time on Linux.
    let mut resolved = PathBuf::new();
    for part in p.components() {
        let exact = resolved.join(part);
        if exact.exists() {
            resolved = exact;
            continue;
        }
        let matches: Vec<_> = std::fs::read_dir(&resolved)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                e.file_name().to_string_lossy().to_lowercase()
                    == part.as_os_str().to_string_lossy().to_lowercase()
            })
            .collect();
        if matches.len() == 1 {
            resolved.push(matches[0].file_name());
        } else {
            return p;
        }
    }
    resolved
}

fn project_paths(text: &str) -> Vec<String> {
    let mut in_document = false;
    let mut result = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_document = section
                .to_ascii_lowercase()
                .strip_prefix("document")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        } else if in_document
            && let Some((k, v)) = line.split_once('=')
            && k.trim().eq_ignore_ascii_case("DocumentPath")
        {
            let v = v.trim().trim_matches('"');
            if !v.is_empty() {
                result.push(v.to_owned());
            }
        }
    }
    result
}

pub fn load_project(path: &Path) -> Result<Project, String> {
    let bytes = read_bounded(path)?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut warnings = vec![];
    let text = if bytes.starts_with(&[0xff, 0xfe]) {
        if (bytes.len() - 2) % 2 != 0 {
            return Err("Truncated UTF-16 project".into());
        }
        String::from_utf16(
            &bytes[2..]
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect::<Vec<_>>(),
        )
        .map_err(|e| e.to_string())?
    } else if let Ok(t) = std::str::from_utf8(&bytes) {
        t.trim_start_matches('\u{feff}').to_owned()
    } else {
        let a = WINDOWS_1251.decode(&bytes).0.into_owned();
        let b = WINDOWS_1252.decode(&bytes).0.into_owned();
        let score = |s: &str| {
            project_paths(s)
                .iter()
                .filter(|p| resolve_path(base, p).is_file())
                .count()
        };
        let (t, encoding) = if score(&a) > score(&b) {
            (a, "Windows-1251")
        } else {
            (b, "Windows-1252")
        };
        warnings.push(format!("Legacy project text decoded as {encoding}; encoding inferred from existing document paths"));
        t
    };
    if !text
        .lines()
        .any(|l| l.trim().eq_ignore_ascii_case("[Design]"))
    {
        return Err("Not an Altium project: [Design] section missing".into());
    }
    let paths = project_paths(&text);
    if paths.len() > 10_000 {
        return Err("Project document limit exceeded".into());
    }
    let mut documents = vec![];
    let mut seen = BTreeSet::new();
    for original_path in paths {
        let path = resolve_path(base, &original_path);
        let path = path.canonicalize().unwrap_or(path);
        if !seen.insert(path.clone()) {
            continue;
        }
        let kind = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let exists = path.is_file();
        if !exists {
            warnings.push(format!("Missing project document: {original_path}"));
        }
        documents.push(ProjectEntry {
            original_path,
            path,
            kind,
            exists,
        });
    }
    Ok(Project {
        path: path.to_owned(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        documents,
        warnings,
    })
}

type Fields = BTreeMap<String, String>;
pub(super) fn parameter_text(text: &str, local: &Fields, sheet: &Fields) -> String {
    let mut value = text.to_owned();
    let mut visited = BTreeSet::new();
    for _ in 0..32 {
        let Some(key) = value.strip_prefix('=') else {
            break;
        };
        let key = key.trim().trim_matches('"');
        if !visited.insert(key.to_lowercase()) {
            return text.to_owned();
        }
        let lookup = |values: &Fields| {
            values
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.clone())
        };
        let Some(next) = lookup(local).or_else(|| lookup(sheet)) else {
            return value;
        };
        value = next;
    }
    value
}
trait Values {
    fn n(&self, k: &str) -> i64;
    fn real(&self, k: &str) -> f64;
    fn yes(&self, k: &str) -> bool;
    fn text(&self, k: &str) -> String;
    fn coord(&self, k: &str) -> i64;
    fn point(&self, prefix: &str) -> Point;
}
impl Values for Fields {
    fn n(&self, k: &str) -> i64 {
        self.get(k).and_then(|s| s.parse().ok()).unwrap_or(0)
    }
    fn real(&self, k: &str) -> f64 {
        self.get(k).and_then(|s| s.parse().ok()).unwrap_or(0.0)
    }
    fn yes(&self, k: &str) -> bool {
        self.get(k)
            .is_some_and(|s| s.eq_ignore_ascii_case("T") || s.eq_ignore_ascii_case("TRUE"))
    }
    fn text(&self, k: &str) -> String {
        self.get(&format!("%UTF8%{k}"))
            .or_else(|| self.get(k))
            .cloned()
            .unwrap_or_default()
    }
    fn coord(&self, k: &str) -> i64 {
        let n = self.n(k) as i128 * 100_000 + self.n(&format!("{k}_FRAC")) as i128;
        // 1 file unit = 10 mil; _FRAC is 1/100000 of that unit. Round once to nm.
        let nm = n * 254_000;
        ((nm + if nm >= 0 { 50_000 } else { -50_000 }) / 100_000) as i64
    }
    fn point(&self, prefix: &str) -> Point {
        Point {
            x: self.coord(&format!("{prefix}.X")),
            y: self.coord(&format!("{prefix}.Y")),
        }
    }
}

fn segments(data: &[u8]) -> Vec<(&[u8], &[u8])> {
    data.split(|b| *b == b'|')
        .filter_map(|s| {
            s.iter()
                .position(|b| *b == b'=')
                .map(|p| (&s[..p], &s[p + 1..]))
        })
        .collect()
}

fn detect_encoding(records: &[(&str, u8, Vec<u8>)]) -> &'static Encoding {
    if records
        .iter()
        .filter(|(_, flag, _)| *flag == 0)
        .all(|(_, _, raw)| std::str::from_utf8(raw).is_ok())
    {
        return encoding_rs::UTF_8;
    }
    let mut scores = [0usize; 2];
    for (_, flag, raw) in records {
        if *flag != 0 {
            continue;
        }
        let pairs = segments(raw.strip_suffix(&[0]).unwrap_or(raw));
        for (key, value) in &pairs {
            let Ok(key) = std::str::from_utf8(key) else {
                continue;
            };
            let Some(plain) = key.strip_prefix("%UTF8%") else {
                continue;
            };
            let Ok(utf) = std::str::from_utf8(value) else {
                continue;
            };
            if utf.is_ascii() {
                continue;
            }
            if let Some((_, legacy)) = pairs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(plain.as_bytes()))
            {
                for (i, encoding) in [WINDOWS_1251, WINDOWS_1252].iter().enumerate() {
                    if encoding.decode(legacy).0 == utf {
                        scores[i] += 1;
                    }
                }
            }
        }
    }
    if scores[0] > scores[1] {
        WINDOWS_1251
    } else {
        WINDOWS_1252
    }
}

fn decode_fields(raw: &[u8], encoding: &'static Encoding) -> Result<Fields, String> {
    let mut fields = BTreeMap::new();
    for (key, value) in segments(raw.strip_suffix(&[0]).unwrap_or(raw)) {
        let key = std::str::from_utf8(key)
            .map_err(|_| "Invalid record property name")?
            .to_ascii_uppercase();
        let value = if key.starts_with("%UTF8%") {
            std::str::from_utf8(value)
                .map_err(|_| "Invalid explicit UTF-8 property")?
                .to_owned()
        } else {
            encoding.decode(value).0.into_owned()
        };
        fields.insert(key, value);
        if fields.len() > 4096 {
            return Err("Record property limit exceeded".into());
        }
    }
    validate_numeric(&fields)?;
    Ok(fields)
}

fn validate_numeric(f: &Fields) -> Result<(), String> {
    const INTS: &[&str] = &[
        "RECORD",
        "OWNERINDEX",
        "OWNERPARTID",
        "OWNERPARTDISPLAYMODE",
        "CURRENTPARTID",
        "PARTCOUNT",
        "DISPLAYMODE",
        "FONTID",
        "FONTIDCOUNT",
        "SYSTEMFONT",
        "LINEWIDTH",
        "LINESTYLE",
        "COLOR",
        "AREACOLOR",
        "TEXTCOLOR",
        "ORIENTATION",
        "JUSTIFICATION",
        "PINCONGLOMERATE",
        "ELECTRICAL",
        "SHEETSTYLE",
        "WORKSPACEORIENTATION",
        "STYLE",
        "IOTYPE",
        "LOCATIONCOUNT",
        "EXTRALOCATIONCOUNT",
        "SYMBOL_OUTEREDGE",
        "SYMBOL_INNEREDGE",
        "NAME_CUSTOMFONTID",
        "DESIGNATOR_CUSTOMFONTID",
        "CUSTOMXZONES",
        "CUSTOMYZONES",
    ];
    for (key, value) in f {
        let dimension = key.ends_with("_FRAC")
            || key.starts_with("LOCATION.")
            || key.starts_with("CORNER.")
            || ["X", "Y", "EX", "EY", "SIZE"].iter().any(|p| {
                key.strip_prefix(p)
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            })
            || [
                "RADIUS",
                "SECONDARYRADIUS",
                "CORNERXRADIUS",
                "CORNERYRADIUS",
                "PINLENGTH",
                "CUSTOMX",
                "CUSTOMY",
                "CUSTOMMARGINWIDTH",
                "SNAPGRIDSIZE",
                "VISIBLEGRIDSIZE",
                "WIDTH",
                "HEIGHT",
                "XSIZE",
                "YSIZE",
                "DISTANCEFROMTOP",
                "SCALEFACTOR",
            ]
            .contains(&key.as_str());
        if !value.is_empty()
            && (dimension || INTS.contains(&key.as_str()))
            && value.parse::<i64>().is_err()
        {
            return Err(format!("Invalid numeric property {key}={value}"));
        }
        if dimension
            && value
                .parse::<i64>()
                .is_ok_and(|n| n.unsigned_abs() > 1_000_000_000)
        {
            return Err(format!("Coordinate property out of range: {key}"));
        }
        if ["STARTANGLE", "ENDANGLE"].contains(&key.as_str())
            && !value.parse::<f64>().is_ok_and(f64::is_finite)
        {
            return Err(format!("Invalid angle {key}"));
        }
    }
    Ok(())
}

fn framed<'a>(
    stream: &'a str,
    data: &[u8],
    out: &mut Vec<(&'a str, u8, Vec<u8>)>,
) -> Result<(), String> {
    let mut offset = 0;
    while offset < data.len() {
        let header = data
            .get(offset..offset + 4)
            .ok_or_else(|| format!("Truncated {stream} record header at {offset}"))?;
        let word = u32::from_le_bytes(header.try_into().unwrap());
        let length = (word & 0xffffff) as usize;
        let flag = (word >> 24) as u8;
        offset += 4;
        if length == 0 {
            return Err(format!("Empty {stream} record at {offset}"));
        }
        let raw = data
            .get(offset..offset + length)
            .ok_or_else(|| format!("Truncated {stream} record payload at {offset}"))?;
        if flag == 0 && !raw.ends_with(&[0]) {
            return Err(format!("Unterminated property record in {stream}"));
        }
        out.push((stream, flag, raw.to_vec()));
        offset += length;
        if out.len() > MAX_RECORDS {
            return Err("Schematic record limit exceeded".into());
        }
    }
    Ok(())
}

pub fn load_schematic(path: &Path) -> Result<Document, String> {
    let bytes = read_bounded(path)?;
    import_schematic(
        &bytes,
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    )
}

pub fn import_schematic(bytes: &[u8], name: &str) -> Result<Document, String> {
    if bytes.len() > MAX_FILE {
        return Err("File exceeds import limit".into());
    }
    let mut raw = vec![];
    let mut warnings = vec![];
    let mut stored_images = vec![];
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
        let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes))
            .map_err(|e| format!("Invalid SchDoc CFB container: {e}"))?;
        let mut total = 0usize;
        for stream in ["FileHeader", "Additional"] {
            if stream == "Additional" && !cfb.is_stream(stream) {
                continue;
            }
            let mut data = Vec::new();
            cfb.open_stream(stream)
                .map_err(|e| format!("Missing or invalid {stream}: {e}"))?
                .take(MAX_FILE as u64 + 1)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            total += data.len();
            if total > MAX_FILE {
                return Err("Expanded schematic streams exceed limit".into());
            }
            framed(stream, &data, &mut raw)?;
        }
        if cfb.is_stream("Storage") {
            let mut data = vec![];
            cfb.open_stream("Storage")
                .map_err(|e| e.to_string())?
                .take(MAX_FILE as u64 + 1)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            if data.len() > MAX_FILE {
                return Err("Image storage exceeds limit".into());
            }
            let mut blocks = vec![];
            framed("Storage", &data, &mut blocks)?;
            let mut expanded = 0usize;
            for (_, flag, raw) in blocks {
                if flag == 0 {
                    continue;
                }
                if raw.first() != Some(&0xd0) || raw.len() < 2 {
                    warnings.push("Unsupported embedded image storage record retained".into());
                    continue;
                }
                let length = raw[1] as usize;
                let end = 2 + length;
                let filename = raw
                    .get(2..end)
                    .ok_or("Truncated embedded image filename")?
                    .to_vec();
                let size_bytes = raw
                    .get(end..end + 4)
                    .ok_or("Truncated embedded image length")?;
                let size = u32::from_le_bytes(size_bytes.try_into().unwrap()) as usize;
                let compressed = raw
                    .get(end + 4..end + 4 + size)
                    .ok_or("Truncated compressed image")?;
                let mut decoded = vec![];
                flate2::read::ZlibDecoder::new(compressed)
                    .take(32 * 1024 * 1024 + 1)
                    .read_to_end(&mut decoded)
                    .map_err(|e| format!("Invalid compressed image: {e}"))?;
                expanded += decoded.len();
                if decoded.len() > 32 * 1024 * 1024 || expanded > MAX_FILE {
                    return Err("Expanded image storage exceeds limit".into());
                }
                stored_images.push((filename, decoded));
            }
        }
    } else {
        let text = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
        for line in text.split(|b| *b == b'\n') {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            raw.push(("FileHeader", 0, line.to_vec()));
            if raw.len() > MAX_RECORDS {
                return Err("Schematic record limit exceeded".into());
            }
        }
    }
    let encoding = detect_encoding(&raw);
    let header = raw
        .first()
        .filter(|(_, flag, _)| *flag == 0)
        .ok_or("Missing schematic header")?;
    let header_fields = decode_fields(&header.2, encoding)?;
    let description = header_fields.text("HEADER");
    if !description.starts_with("Protel for Windows - Schematic Capture ")
        || !description.contains("File Version 5.0")
    {
        return Err("Unsupported file: expected Altium schematic Version 5.0 header".into());
    }
    let id = Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        if header_fields.text("UNIQUEID").is_empty() {
            bytes
        } else {
            header_fields.get("UNIQUEID").unwrap().as_bytes()
        },
    );
    let mut records = Vec::new();
    for (stream, flag, data) in &raw {
        let fields = if *flag == 0 {
            decode_fields(data, encoding)?
        } else {
            Fields::new()
        };
        if fields.contains_key("HEADER") {
            continue;
        }
        if *flag != 0 {
            warnings.push(format!(
                "Unsupported binary schematic record, flag {flag}; original preserved"
            ));
        }
        let record_type = if *flag == 0 {
            fields.n("RECORD") as u32
        } else {
            u32::MAX
        };
        if *stream == "Additional" {
            warnings.push(format!(
                "Additional-stream record {record_type} retained; rendering is not implemented"
            ));
        }
        records.push(SourceRecord {
            index: records.len() as u32,
            stream: (*stream).into(),
            record_type,
            fields,
        });
    }
    if records.first().is_none_or(|r| r.record_type != 31) {
        return Err("Missing schematic sheet record".into());
    }
    for (i, r) in records.iter().enumerate() {
        if r.stream == "FileHeader" && i > 0 && r.fields.n("OWNERINDEX") >= i as i64 {
            return Err(format!("Invalid forward/cyclic owner at record {i}"));
        }
        if r.record_type == 1 && !(0..=1024).contains(&r.fields.n("CURRENTPARTID")) {
            return Err(format!("Invalid component part at record {i}"));
        }
    }
    let images = stored_images
        .into_iter()
        .map(|(name, data)| {
            let name = std::str::from_utf8(&name)
                .map(str::to_owned)
                .unwrap_or_else(|_| encoding.decode(&name).0.into_owned())
                .replace('\\', "/")
                .to_lowercase();
            (name, STANDARD.encode(data))
        })
        .collect();
    build_document(name, id, records, warnings, bytes, images)
}

fn uuid(doc: Uuid, index: u32) -> Uuid {
    Uuid::new_v5(&doc, &index.to_le_bytes())
}
fn owner(f: &Fields) -> usize {
    f.n("OWNERINDEX").max(0) as usize
}
fn component_owner(records: &[SourceRecord], index: usize) -> Option<usize> {
    let mut i = index;
    for _ in 0..64 {
        if records.get(i)?.record_type == 1 {
            return Some(i);
        }
        let next = owner(&records[i].fields);
        if next >= i {
            return None;
        }
        i = next;
    }
    None
}

fn is_visible(records: &[SourceRecord], index: usize, show_template: bool) -> bool {
    let f = &records[index].fields;
    if f.yes("ISHIDDEN") || f.yes("IGNOREONLOAD") {
        return false;
    }
    let mut i = index;
    for _ in 0..64 {
        let record = &records[i];
        if record.record_type == 39 && !show_template {
            return false;
        }
        if [44, 45, 46, 47, 48].contains(&record.record_type) {
            return false;
        }
        if record.record_type == 1 {
            return (f.n("OWNERPARTID") <= 0
                || f.n("OWNERPARTID") == record.fields.n("CURRENTPARTID").max(1))
                && (([34, 41].contains(&records[index].record_type)
                    && records.get(owner(f)).is_some_and(|r| r.record_type == 1)
                    && !f.contains_key("OWNERPARTDISPLAYMODE"))
                    || f.n("OWNERPARTDISPLAYMODE") == record.fields.n("DISPLAYMODE"));
        }
        let next = owner(&record.fields);
        if next >= i {
            return next == 0 && i == 0;
        }
        i = next;
    }
    false
}

fn build_document(
    name: &str,
    id: Uuid,
    records: Vec<SourceRecord>,
    mut warnings: Vec<String>,
    bytes: &[u8],
    images: BTreeMap<String, String>,
) -> Result<Document, String> {
    let f = &records[0].fields;
    let custom = f.yes("USECUSTOMSHEET");
    let sizes = [
        (1150, 760),
        (1550, 1110),
        (2230, 1570),
        (3150, 2230),
        (4460, 3150),
        (950, 750),
        (1500, 950),
        (2000, 1500),
        (3200, 2000),
        (4200, 3200),
        (1100, 850),
        (1400, 850),
        (1700, 1100),
        (990, 790),
        (1540, 990),
        (2060, 1560),
        (3260, 2060),
        (4280, 3280),
    ];
    let (w, h) = sizes
        .get(f.n("SHEETSTYLE") as usize)
        .copied()
        .ok_or("Unsupported sheet size enumeration")?;
    let mut size = if custom {
        Point {
            x: f.coord("CUSTOMX"),
            y: f.coord("CUSTOMY"),
        }
    } else {
        Point {
            x: w * 254_000,
            y: h * 254_000,
        }
    };
    if !custom && f.n("WORKSPACEORIENTATION") == 1 {
        std::mem::swap(&mut size.x, &mut size.y);
    }
    let mut fonts = vec![];
    for n in 1..=f.n("FONTIDCOUNT").clamp(0, 128) {
        fonts.push(SchFont {
            name: f.text(&format!("FONTNAME{n}")),
            // Native SIZE uses the schematic text scale; treating it as physical
            // 1/72-inch points oversizes text (verified against TIDA in AD24).
            height_nm: f.n(&format!("SIZE{n}")).max(1) * 254_000,
            bold: f.yes(&format!("BOLD{n}")),
            italic: f.yes(&format!("ITALIC{n}")),
            underline: f.yes(&format!("UNDERLINE{n}")),
        });
    }
    if fonts.is_empty() {
        fonts.push(SchFont {
            name: "Times New Roman".into(),
            height_nm: 2_540_000,
            bold: false,
            italic: false,
            underline: false,
        });
    }
    let mut parameters = BTreeMap::new();
    for r in &records {
        if r.record_type == 41 && owner(&r.fields) == 0 {
            parameters.insert(r.fields.text("NAME"), r.fields.text("TEXT"));
        }
    }
    let mut components = vec![];
    let mut child_parameters: BTreeMap<usize, Vec<&SourceRecord>> = BTreeMap::new();
    for r in &records {
        if [34, 41].contains(&r.record_type) {
            child_parameters
                .entry(owner(&r.fields))
                .or_default()
                .push(r);
        }
    }
    for r in &records {
        if r.record_type != 1 || r.stream != "FileHeader" {
            continue;
        }
        let mut params = BTreeMap::new();
        let mut designator = String::new();
        for child in child_parameters
            .get(&(r.index as usize))
            .into_iter()
            .flatten()
        {
            if child.record_type == 34 {
                designator = child.fields.text("TEXT");
            }
            if child.record_type == 41 {
                params.insert(child.fields.text("NAME"), child.fields.text("TEXT"));
            }
        }
        let part = r.fields.n("CURRENTPARTID").max(1) as u32;
        if r.fields.n("PARTCOUNT") > 2 {
            designator.push(char::from_u32('A' as u32 + part - 1).unwrap_or('?'));
        }
        components.push(Component {
            id: uuid(id, r.index),
            source_index: r.index,
            position: r.fields.point("LOCATION"),
            designator,
            comment: parameter_text(
                &params
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("Comment"))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default(),
                &params,
                &parameters,
            ),
            library_reference: r.fields.text("LIBREFERENCE"),
            description: r.fields.text("COMPONENTDESCRIPTION"),
            part,
            display_mode: r.fields.n("DISPLAYMODE") as u32,
            parameters: params,
        });
    }
    let sheet = Sheet {
        background: if f.contains_key("AREACOLOR") {
            f.n("AREACOLOR") as u32
        } else {
            0xf8fcff
        },
        border: f.yes("BORDERON"),
        title_block: f.yes("TITLEBLOCKON"),
        zones: if custom {
            [
                f.n("CUSTOMXZONES").clamp(1, 64) as u32,
                f.n("CUSTOMYZONES").clamp(1, 64) as u32,
            ]
        } else {
            [4, 4]
        },
        margin_nm: if custom && f.contains_key("CUSTOMMARGINWIDTH") {
            f.coord("CUSTOMMARGINWIDTH").clamp(0, 100_000_000)
        } else {
            5_080_000
        },
        system_font: f.n("SYSTEMFONT").max(1) as usize - 1,
        zone_origin_lower_right: f.n("REFERENCEZONESTYLE") == 1,
        grid_nm: f.coord("VISIBLEGRIDSIZE").max(254_000),
        snap_nm: f.coord("SNAPGRIDSIZE").max(254_000),
        grid_visible: f.yes("VISIBLEGRIDON"),
        style: if custom {
            "Custom".into()
        } else {
            format!("Standard {}", f.n("SHEETSTYLE"))
        },
        parameters,
    };
    let mut primitives = vec![];
    let component_lookup: BTreeMap<u32, &Component> =
        components.iter().map(|c| (c.source_index, c)).collect();
    let mut unsupported = BTreeMap::<u32, usize>::new();
    let show_template = f.yes("SHOWTEMPLATEGRAPHICS");
    let default_font = f.n("SYSTEMFONT").max(1) as usize - 1;
    for (index, r) in records.iter().enumerate() {
        if r.stream != "FileHeader" || !is_visible(&records, index, show_template) {
            continue;
        }
        let f = &r.fields;
        let component = component_owner(&records, index).map(|i| uuid(id, i as u32));
        if let Some(kind) = primitive_kind(
            &records,
            r,
            default_font,
            &sheet.parameters,
            &component_lookup,
            &mut unsupported,
        )? {
            let color = if f.contains_key("COLOR") {
                f.n("COLOR") as u32
            } else {
                0
            };
            if let SchPrimitiveKind::Image { filename, .. } = &kind {
                let normalized = filename.replace('\\', "/").to_lowercase();
                if !images.contains_key(&normalized) {
                    warnings.push(format!("Image resource is unavailable: {filename}"));
                }
            }
            primitives.push(SchPrimitive {
                id: uuid(id, r.index),
                source_index: r.index,
                component,
                color,
                fill: if f.yes("ISSOLID") && !f.yes("TRANSPARENT") {
                    Some(f.n("AREACOLOR") as u32)
                } else {
                    None
                },
                width_nm: match f.n("LINEWIDTH") {
                    0 => 101_600,
                    1 => 254_000,
                    2 => 508_000,
                    3 => 1_016_000,
                    _ => 254_000,
                },
                line_style: f.n("LINESTYLE") as u32,
                start_shape: f.n("STARTLINESHAPE") as u32,
                end_shape: f.n("ENDLINESHAPE") as u32,
                line_shape_size: f.n("LINESHAPESIZE") as u32,
                kind,
            });
        }
    }
    for (kind, count) in unsupported {
        warnings.push(format!(
            "Record type {kind}: {count} visible object(s) not fully rendered; original retained"
        ));
    }
    warnings.sort();
    warnings.dedup();
    let mut doc = Document::new(DocumentKind::Schematic, name);
    doc.id = id;
    doc.size = size;
    doc.schematic = Some(Schematic {
        sheet,
        fonts,
        components,
        primitives,
        records,
        images,
        source: Some(NativeSource {
            format: "Altium SchDoc".into(),
            filename: name.into(),
            original_base64: STANDARD.encode(bytes),
        }),
        warnings,
    });
    doc.validate()?;
    Ok(doc)
}

fn primitive_kind(
    records: &[SourceRecord],
    r: &SourceRecord,
    default_font: usize,
    parameters: &Fields,
    components: &BTreeMap<u32, &Component>,
    unsupported: &mut BTreeMap<u32, usize>,
) -> Result<Option<SchPrimitiveKind>, String> {
    use SchPrimitiveKind as K;
    let f = &r.fields;
    // Pin-owned parameters (for example Net Name) are metadata for the pin,
    // not free-standing strings on the sheet. Pin name/number are drawn by Pin.
    if r.record_type == 41 && records.get(owner(f)).is_some_and(|r| r.record_type == 2) {
        return Ok(None);
    }
    let a = f.point("LOCATION");
    let b = f.point("CORNER");
    let orientation = f.n("ORIENTATION") as u32;
    let font = f
        .get("FONTID")
        .map_or(default_font, |_| f.n("FONTID").max(1) as usize - 1);
    let kind = match r.record_type {
        1 | 31 | 39 | 44..=48 => return Ok(None),
        5 | 6 | 7 | 26 | 27 => {
            let count = f.n("LOCATIONCOUNT");
            let extra = f.n("EXTRALOCATIONCOUNT");
            if !(0..=100_000).contains(&count) || !(0..=100_000 - count).contains(&extra) {
                return Err("Schematic point count exceeds limit".into());
            }
            let mut points = vec![];
            for n in 1..=count {
                points.push(Point {
                    x: f.coord(&format!("X{n}")),
                    y: f.coord(&format!("Y{n}")),
                });
            }
            for n in count + 1..=count + extra {
                points.push(Point {
                    x: f.coord(&format!("EX{n}")),
                    y: f.coord(&format!("EY{n}")),
                });
            }
            if r.record_type == 5 {
                if points.len() != 4 {
                    return Err("Bezier requires four points".into());
                }
                K::Bezier {
                    points: points.try_into().unwrap(),
                }
            } else {
                K::Polyline {
                    points,
                    closed: r.record_type == 7,
                    electrical: match r.record_type {
                        26 => "Bus",
                        27 => "Wire",
                        _ => "Graphic",
                    }
                    .into(),
                }
            }
        }
        10 | 14 => K::Rectangle {
            a,
            b,
            radius: Point {
                x: f.coord("CORNERXRADIUS"),
                y: f.coord("CORNERYRADIUS"),
            },
        },
        13 | 37 => K::Polyline {
            points: vec![a, b],
            closed: false,
            electrical: if r.record_type == 37 {
                "Bus Entry"
            } else {
                "Graphic"
            }
            .into(),
        },
        8 | 11 | 12 => K::Arc {
            centre: a,
            radii: Point {
                x: f.coord("RADIUS"),
                y: if r.record_type == 12 {
                    f.coord("RADIUS")
                } else {
                    f.coord("SECONDARYRADIUS")
                },
            },
            start: f.real("STARTANGLE"),
            end: if f.real("ENDANGLE") == f.real("STARTANGLE") {
                f.real("STARTANGLE") + 360.0
            } else {
                f.real("ENDANGLE")
            },
        },
        4 | 25 | 28 | 32 | 33 | 34 | 41 | 209 => {
            let mut text = f.text("TEXT").replace("~1", "\n");
            if text.starts_with('=') {
                let component = component_owner(records, r.index as usize)
                    .and_then(|i| components.get(&(i as u32)).copied());
                let values = component.map_or(parameters, |c| &c.parameters);
                text = parameter_text(&text, values, parameters);
            }
            if r.record_type == 34
                && let Some(c) = components.get(&(owner(f) as u32))
            {
                text = c.designator.clone();
            }
            if text.is_empty() {
                return Ok(None);
            }
            let justification = if f.contains_key("JUSTIFICATION") {
                f.n("JUSTIFICATION") as u32
            } else if orientation >= 2 {
                8
            } else {
                0
            };
            let position = if [28, 209].contains(&r.record_type) {
                Point {
                    x: a.x,
                    y: a.y.max(b.y),
                }
            } else {
                a
            };
            if [28, 209].contains(&r.record_type) {
                if orientation != 0 {
                    *unsupported.entry(r.record_type).or_default() += 1;
                }
                K::TextFrame {
                    a,
                    b,
                    text,
                    font,
                    alignment: f.n("ALIGNMENT") as u32,
                    word_wrap: f.yes("WORDWRAP"),
                    show_border: f.yes("SHOWBORDER"),
                }
            } else {
                K::Text {
                    position,
                    text,
                    font,
                    orientation,
                    justification,
                }
            }
        }
        2 => {
            let flags = f.n("PINCONGLOMERATE");
            K::Pin {
                position: a,
                length_nm: f.coord("PINLENGTH"),
                orientation: (flags & 3) as u32,
                name: f.text("NAME"),
                number: f.text("DESIGNATOR"),
                show_name: flags & 8 != 0,
                show_number: flags & 16 != 0,
                font: f.get("NAME_CUSTOMFONTID").map_or(default_font, |_| {
                    f.n("NAME_CUSTOMFONTID").max(1) as usize - 1
                }),
                number_font: Some(f.get("DESIGNATOR_CUSTOMFONTID").map_or(default_font, |_| {
                    f.n("DESIGNATOR_CUSTOMFONTID").max(1) as usize - 1
                })),
                electrical: f.n("ELECTRICAL") as u32,
                outer_symbol: f.n("SYMBOL_OUTEREDGE") as u32,
                inner_symbol: f.n("SYMBOL_INNEREDGE") as u32,
            }
        }
        17 => K::PowerPort {
            position: a,
            orientation,
            style: f.n("STYLE") as u32,
            text: f.text("TEXT"),
            show_name: f.yes("SHOWNETNAME"),
            font,
        },
        18 => K::Port {
            position: a,
            width_nm: f.coord("WIDTH"),
            height_nm: f.coord("HEIGHT").max(2_540_000),
            style: f.n("STYLE") as u32,
            io_type: f.n("IOTYPE") as u32,
            name: f.text("NAME"),
            font,
        },
        15 => K::Rectangle {
            a,
            b: Point {
                x: a.x + f.coord("XSIZE"),
                y: a.y - f.coord("YSIZE"),
            },
            radius: Point::default(),
        },
        16 => {
            let parent = &records
                .get(owner(f))
                .ok_or("Invalid sheet entry owner")?
                .fields;
            let loc = parent.point("LOCATION");
            let side = f.n("SIDE");
            let d = f.coord("DISTANCEFROMTOP");
            let p = match side {
                0 => Point {
                    x: loc.x,
                    y: loc.y - d,
                },
                1 => Point {
                    x: loc.x + parent.coord("XSIZE"),
                    y: loc.y - d,
                },
                2 => Point {
                    x: loc.x + d,
                    y: loc.y,
                },
                _ => Point {
                    x: loc.x + d,
                    y: loc.y - parent.coord("YSIZE"),
                },
            };
            *unsupported.entry(16).or_default() += 1;
            K::Text {
                position: p,
                text: f.text("NAME"),
                font,
                orientation: 0,
                justification: 3,
            }
        }
        29 => K::Junction {
            position: a,
            radius_nm: 508_000,
        },
        22 => K::NoErc { position: a },
        3 => {
            let symbol = f.n("SYMBOL") as u32;
            if ![0, 1, 3, 11].contains(&symbol) {
                *unsupported.entry(3).or_default() += 1;
            }
            K::Ieee {
                position: a,
                symbol,
                orientation,
                scale_nm: f.coord("SCALEFACTOR"),
                mirrored: f.yes("MIRROR"),
            }
        }
        30 => K::Image {
            a,
            b,
            filename: f.text("FILENAME"),
            keep_aspect: f.yes("KEEPASPECT"),
        },
        _ => {
            *unsupported.entry(r.record_type).or_default() += 1;
            return Ok(None);
        }
    };
    Ok(Some(kind))
}
