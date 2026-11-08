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

#[path = "altium_library.rs"]
pub mod library;
#[path = "altium_sch_write.rs"]
pub mod writer;

/// An independently authored ASCII 5.0 sheet, with a native source identity.
/// Starting from a native sheet lets the verified writer handle subsequent edits.
pub fn new_schematic(name: &str) -> Result<Document, String> {
    let bytes = format!(
        "|HEADER=Protel for Windows - Schematic Capture Ascii File Version 5.0|WEIGHT=1|MINORVERSION=9|UNIQUEID={}\r\n|RECORD=31|FONTIDCOUNT=1|SIZE1=10|FONTNAME1=Arial|SYSTEMFONT=1|SHEETSTYLE=0|BORDERON=T|TITLEBLOCKON=T|AREACOLOR=16317695|VISIBLEGRIDON=T|VISIBLEGRIDSIZE=10|SNAPGRIDSIZE=10\r\n",
        Uuid::new_v4()
    );
    import_schematic(bytes.as_bytes(), name)
}

/// Read the native textual harness definitions, including legacy Cyrillic files.
pub fn load_harness_definitions(
    path: &Path,
) -> Result<Vec<eda_core::schematic_harness::Definition>, String> {
    let bytes = read_bounded(path)?;
    let text = std::str::from_utf8(&bytes)
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|_| WINDOWS_1251.decode(&bytes).0);
    eda_core::schematic_harness::parse_definitions(&text)
}

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
    pub generated_documents: Vec<ProjectEntry>,
    pub variants: Vec<eda_core::assembly_variants::Variant>,
    pub virtual_bom: bool,
    pub sheet_children: BTreeMap<PathBuf, Vec<PathBuf>>,
    pub sheet_numbers: BTreeMap<PathBuf, usize>,
    pub netlist_options: eda_core::design_sync::ProjectOptions,
    pub validation_options: eda_core::schematic_validation::Options,
    pub warnings: Vec<String>,
}

impl Project {
    /// Resolve the current editable sheets, not the cached project-tree snapshot.
    pub fn compile_options(
        &self,
        sheets: &[(&Path, &Document)],
    ) -> Result<eda_core::design_sync::ProjectOptions, String> {
        use eda_core::design_sync::SheetLink;
        let mut options = self.netlist_options.clone();
        options.harness_definitions.clear();
        let mut harness_files: BTreeSet<_> = self
            .documents
            .iter()
            .filter(|e| e.kind.eq_ignore_ascii_case("harness"))
            .map(|e| e.path.clone())
            .collect();
        for (path, _) in sheets {
            let file = path.with_extension("Harness");
            if file.is_file() {
                harness_files.insert(file);
            }
        }
        for file in harness_files {
            options
                .harness_definitions
                .extend(load_harness_definitions(&file)?);
        }
        let mut links = vec![];
        for (path, doc) in sheets {
            if let Some(entry) = self.documents.iter().find(|e| e.path == *path) {
                options
                    .sheet_paths
                    .insert(doc.id, entry.original_path.replace('\\', "/"));
            }
            options
                .sheet_numbers
                .insert(doc.id, self.sheet_number(path).unwrap_or(1));
            for p in &doc
                .schematic
                .as_ref()
                .ok_or("Expected schematic")?
                .primitives
            {
                if let SchPrimitiveKind::SheetSymbol { filename, .. } = &p.kind {
                    let entry = self.resolve_sheet(path, filename).ok_or_else(|| {
                        format!(
                            "{}: child sheet {filename} is missing or outside the project",
                            doc.name
                        )
                    })?;
                    let child = sheets
                        .iter()
                        .find(|(path, _)| *path == entry.path)
                        .ok_or_else(|| format!("Child sheet {filename} was not loaded"))?;
                    links.push(SheetLink {
                        parent: doc.id,
                        symbol: p.id,
                        child: child.1.id,
                    });
                }
            }
        }
        options.links = Some(links);
        Ok(options)
    }
    pub fn sheet_number(&self, path: &Path) -> Option<usize> {
        self.sheet_numbers.get(path).copied().or_else(|| {
            self.documents
                .iter()
                .filter(|e| e.kind == "schdoc")
                .position(|e| e.path == path)
                .map(|n| n + 1)
        })
    }

    /// Resolve relative to the owning schematic, never by an ambiguous basename.
    pub fn resolve_sheet(&self, owner: &Path, filename: &str) -> Option<&ProjectEntry> {
        let path = resolve_path(owner.parent()?, filename);
        let path = path.canonicalize().unwrap_or(path);
        self.documents
            .iter()
            .find(|e| e.kind == "schdoc" && e.path == path)
    }
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
    section_paths(text, "document")
}

fn section_paths(text: &str, prefix: &str) -> Vec<String> {
    let mut in_document = false;
    let mut result = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            let section = &line[1..line.len() - 1];
            in_document = section
                .to_ascii_lowercase()
                .strip_prefix(prefix)
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
        let cyrillic_word = a
            .chars()
            .scan(0usize, |run, ch| {
                *run = if ('\u{0400}'..='\u{04ff}').contains(&ch) {
                    *run + 1
                } else {
                    0
                };
                Some(*run)
            })
            .any(|run| run >= 3);
        let (t, encoding) = if score(&a) > score(&b) || score(&a) == score(&b) && cyrillic_word {
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
    let mut generated_documents = vec![];
    let mut seen = BTreeSet::new();
    let generated = section_paths(&text, "generateddocument");
    if paths.len() + generated.len() > 10_000 {
        return Err("Project document limit exceeded".into());
    }
    for (original_path, is_generated) in paths
        .into_iter()
        .map(|p| (p, false))
        .chain(generated.into_iter().map(|p| (p, true)))
    {
        let path = resolve_path(base, &original_path);
        let path = path.canonicalize().unwrap_or(path);
        if !seen.insert(path.clone()) {
            continue;
        }
        let mut kind = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let exists = path.is_file();
        if kind == "json"
            && exists
            && let Ok(doc) = crate::load(&path)
        {
            kind = if doc.library.is_some() {
                if doc.kind == DocumentKind::Schematic {
                    "schlib"
                } else {
                    "pcblib"
                }
            } else if doc.kind == DocumentKind::Schematic {
                "schdoc"
            } else {
                "pcbdoc"
            }
            .into();
        }
        if !exists && !is_generated {
            warnings.push(format!("Missing project document: {original_path}"));
        }
        let entries = if is_generated {
            &mut generated_documents
        } else {
            &mut documents
        };
        entries.push(ProjectEntry {
            original_path,
            path,
            kind,
            exists,
        });
    }
    let mut variants = crate::project_variants::parse(&text)?;
    crate::project_variants::resolve_alternates(&mut variants, path, &mut warnings);
    let mut netlist_options = eda_core::design_sync::ProjectOptions {
        allow_port_names: false,
        allow_entry_names: true,
        power_names_priority: false,
        higher_level_names_priority: false,
        hierarchical_net_names: false,
        single_pin_nets: false,
        merge_duplicate_net_names: true,
        connection_precision_nm: Some(2_540),
        review_annotation: true,
        ..Default::default()
    };
    let mut section = String::new();
    let mut erc_matrix = BTreeMap::<usize, String>::new();
    let mut erc_levels = BTreeMap::<u16, u8>::new();
    let mut virtual_bom = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_ascii_lowercase();
        } else if let Some((key, value)) = line.split_once('=') {
            if section == "electrical rules check"
                && let Some(index) = key.trim().to_ascii_lowercase().strip_prefix("type")
            {
                let rule = index
                    .parse::<u16>()
                    .map_err(|_| "Invalid ERC rule number")?;
                let level = value
                    .trim()
                    .parse::<u8>()
                    .map_err(|_| "Invalid ERC reporting level")?;
                if level > 3 {
                    return Err(format!(
                        "Invalid ERC reporting level {level} for Type{rule}"
                    ));
                }
                erc_levels.insert(rule, level);
            }
            if section == "erc connection matrix"
                && let Some(index) = key
                    .trim()
                    .strip_prefix('L')
                    .and_then(|n| n.parse::<usize>().ok())
            {
                erc_matrix.insert(index, value.trim().to_owned());
            }
            if section == "design" {
                let flag = value.trim() == "1";
                match key.trim().to_ascii_lowercase().as_str() {
                    "allowportnetnames" => netlist_options.allow_port_names = flag,
                    "allowsheetentrynetnames" => netlist_options.allow_entry_names = flag,
                    "powerportnamestakepriority" => netlist_options.power_names_priority = flag,
                    "higherlevelnamestakepriority" => {
                        netlist_options.higher_level_names_priority = flag
                    }
                    "namenetshierarchically" if flag => {
                        netlist_options.unsupported_connectivity = Some("Native NameNetsHierarchically=1 naming semantics are not yet verified; connectivity changes are disabled".into());
                    }
                    "appendsheetnumbertolocalnets" => netlist_options.append_sheet_number = flag,
                    "netlistsinglepinnets" => netlist_options.single_pin_nets = flag,
                    "hierarchymode" if value.trim() != "0" => {
                        netlist_options.unsupported_connectivity = Some(format!(
                            "Native HierarchyMode={} is not yet verified; connectivity changes are disabled",
                            value.trim()
                        ))
                    }
                    _ => {}
                }
            }
            if section == "design" && key.eq_ignore_ascii_case("IsVirtualBomDocumentRemoved") {
                virtual_bom = value.trim() == "0";
            }
            if section
                .strip_prefix("variant")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
                && key.eq_ignore_ascii_case("Description")
                && !value.trim().is_empty()
            {
                // Full ProjectVariant/legacy Variant definitions are parsed above.
            }
        }
    }
    let mut sheet_children = BTreeMap::new();
    for entry in documents.iter().filter(|e| e.kind == "schdoc" && e.exists) {
        if let Ok(names) = sheet_references(&entry.path) {
            let children: Vec<_> = names
                .into_iter()
                .filter_map(|name| {
                    let child = resolve_path(entry.path.parent().unwrap_or(base), &name);
                    let child = child.canonicalize().unwrap_or(child);
                    documents
                        .iter()
                        .find(|e| e.kind == "schdoc" && e.path == child)
                        .map(|e| e.path.clone())
                })
                .collect();
            if !children.is_empty() {
                sheet_children.insert(entry.path.clone(), children);
            }
        }
    }
    let mut sheet_numbers = BTreeMap::new();
    if let Ok(bytes) = read_bounded(&path.with_extension("PrjPcbStructure")) {
        let mut candidates = vec![
            WINDOWS_1251.decode(&bytes).0.into_owned(),
            WINDOWS_1252.decode(&bytes).0.into_owned(),
        ];
        if let Ok(text) = std::str::from_utf8(&bytes) {
            candidates.push(text.to_owned());
        }
        if bytes.starts_with(&[0xff, 0xfe])
            && bytes.len().is_multiple_of(2)
            && let Ok(text) = String::from_utf16(
                &bytes[2..]
                    .chunks_exact(2)
                    .map(|b| u16::from_le_bytes([b[0], b[1]]))
                    .collect::<Vec<_>>(),
            )
        {
            candidates.push(text);
        }
        for text in candidates {
            let mut numbers = BTreeMap::new();
            let mut used = BTreeSet::new();
            for line in text.lines() {
                let fields: BTreeMap<_, _> = line
                    .trim_start_matches('\u{feff}')
                    .split('|')
                    .filter_map(|s| s.split_once('='))
                    .map(|(k, v)| (k.to_ascii_uppercase(), v))
                    .collect();
                if let (Some(file), Some(number)) = (
                    fields.get("FILENAME"),
                    fields
                        .get("SHEETNUMBER")
                        .and_then(|v| v.parse::<usize>().ok()),
                ) && (1..=10_000).contains(&number)
                {
                    let sheet = resolve_path(base, file);
                    let sheet = sheet.canonicalize().unwrap_or(sheet);
                    if documents
                        .iter()
                        .any(|e| e.kind == "schdoc" && e.path == sheet)
                        && used.insert(number)
                    {
                        numbers.entry(sheet).or_insert(number);
                    }
                }
            }
            if numbers.len() > sheet_numbers.len() {
                sheet_numbers = numbers;
            }
        }
    }
    for children in sheet_children.values_mut() {
        children.sort_by_key(|path| sheet_numbers.get(path).copied().unwrap_or(usize::MAX));
    }
    Ok(Project {
        path: path.to_owned(),
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        documents,
        generated_documents,
        variants,
        virtual_bom,
        sheet_children,
        sheet_numbers,
        netlist_options,
        validation_options: {
            let mut options = if erc_matrix.is_empty() {
                Default::default()
            } else {
                let size = if erc_matrix.keys().any(|n| *n > 8) {
                    17
                } else {
                    8
                };
                let rows: Vec<_> = (1..=size)
                    .map(|n| erc_matrix.get(&n).map_or("", String::as_str))
                    .collect();
                eda_core::schematic_validation::Options::from_native_matrix(&rows)?
            };
            for (rule, level) in erc_levels {
                options.set_native_level(rule, level)?;
            }
            options
        },
        warnings,
    })
}

// Read only the schematic record stream: no images, geometry, or editable model.
fn sheet_references(path: &Path) -> Result<Vec<String>, String> {
    let bytes = read_bounded(path)?;
    let mut raw = Vec::new();
    if bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]) {
        let mut file = cfb::CompoundFile::open(Cursor::new(&bytes)).map_err(|e| e.to_string())?;
        let mut data = Vec::new();
        file.open_stream("FileHeader")
            .map_err(|e| e.to_string())?
            .take(MAX_FILE as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        if data.len() > MAX_FILE {
            return Err("Schematic header exceeds limit".into());
        }
        framed("FileHeader", &data, &mut raw)?;
    } else {
        for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
            raw.push(("FileHeader", 0, line.to_vec()));
            if raw.len() > MAX_RECORDS {
                return Err("Schematic record limit exceeded".into());
            }
        }
    }
    let encoding = detect_encoding(&raw);
    let mut result = Vec::new();
    for (_, flag, bytes) in raw {
        if flag != 0 {
            continue;
        }
        let fields = decode_fields(&bytes, encoding)?;
        if fields.get("RECORD").is_some_and(|r| r == "33") {
            let name = fields.text("TEXT");
            if !name.is_empty() {
                result.push(name);
            }
        }
    }
    Ok(result)
}

type Fields = BTreeMap<String, String>;
pub(super) fn parameter_text(text: &str, local: &Fields, sheet: &Fields) -> String {
    eda_core::schematic::parameter_text(text, local, sheet)
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
        coord_nm(coord_units(self, k))
    }
    fn point(&self, prefix: &str) -> Point {
        Point {
            x: self.coord(&format!("{prefix}.X")),
            y: self.coord(&format!("{prefix}.Y")),
        }
    }
}

fn coord_units(fields: &Fields, key: &str) -> i128 {
    fields.n(key) as i128 * 100_000 + fields.n(&format!("{key}_FRAC")) as i128
}
fn coord_nm(units: i128) -> i64 {
    let nm = units * 254_000;
    ((nm + if nm >= 0 { 50_000 } else { -50_000 }) / 100_000) as i64
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
        "COMPONENTKIND",
        "COMPONENTKINDVERSION2",
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
    let mut doc = import_schematic(
        &bytes,
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .as_ref(),
    )?;
    crate::schematic_graphic::cache_external(&mut doc, path);
    Ok(doc)
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
            stored_images = decode_image_storage(&data, &mut warnings)?;
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
        if *stream == "Additional" && !matches!(record_type, 215..=218 | 225) {
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
fn harness_owner(records: &[SourceRecord], r: &SourceRecord) -> usize {
    let base = if r.stream == "Additional" && r.fields.yes("OWNERINDEXADDITIONALLIST") {
        records
            .iter()
            .position(|p| p.stream == "Additional")
            .unwrap_or(0)
    } else {
        0
    };
    base + owner(&r.fields)
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
    in_active_variant(records, index, show_template, false)
}
fn in_active_variant(
    records: &[SourceRecord],
    index: usize,
    show_template: bool,
    hidden: bool,
) -> bool {
    let f = &records[index].fields;
    if (!hidden
        && (f.yes("ISHIDDEN")
            || records[index].record_type == 2 && f.n("PINCONGLOMERATE") & 4 != 0))
        || f.yes("IGNOREONLOAD")
    {
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
    let standard_size = eda_core::schematic::standard_sheet_size(
        f.n("SHEETSTYLE") as usize,
        f.n("WORKSPACEORIENTATION") == 1,
    )
    .ok_or("Unsupported sheet size enumeration")?;
    let size = if custom {
        Point {
            x: f.coord("CUSTOMX"),
            y: f.coord("CUSTOMY"),
        }
    } else {
        standard_size
    };
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
            parameters.insert(
                r.fields.text("NAME"),
                r.fields.text("TEXT").replace("~1", "\n"),
            );
        }
    }
    let mut components = vec![];
    let mut models: BTreeMap<usize, Vec<eda_core::schematic::ComponentModel>> = BTreeMap::new();
    for (index, r) in records
        .iter()
        .enumerate()
        .filter(|(_, r)| r.record_type == 45)
    {
        if let Some(component_index) = component_owner(&records, index) {
            models
                .entry(component_index)
                .or_default()
                .push(eda_core::schematic::ComponentModel {
                    parameters: {
                        let mut values: BTreeMap<String, String> = records
                            .iter()
                            .filter(|p| {
                                p.record_type == 41
                                    && records.get(owner(&p.fields)).is_some_and(|list| {
                                        list.record_type == 48 && owner(&list.fields) == index
                                    })
                            })
                            .map(|p| {
                                (
                                    p.fields.text("NAME"),
                                    p.fields.text("TEXT").replace("~1", "\n"),
                                )
                            })
                            .collect();
                        if r.fields.text("MODELTYPE").eq_ignore_ascii_case("PCBLIB") {
                            if !r.fields.text("MODELDATAFILE0").is_empty() {
                                values.insert("LIBRARY".into(), r.fields.text("MODELDATAFILE0"));
                            }
                            for map in records.iter().filter(|p| {
                                p.record_type == 47
                                    && records.get(owner(&p.fields)).is_some_and(|list| {
                                        list.record_type == 46 && owner(&list.fields) == index
                                    })
                            }) {
                                let count = map.fields.n("DESIMPCOUNT");
                                if !(1..=1024).contains(&count) {
                                    return Err("Invalid footprint pin-map count".into());
                                }
                                let pin = map.fields.text("DESINTF");
                                let pads: Vec<_> = (0..count)
                                    .map(|n| map.fields.text(&format!("DESIMP{n}")))
                                    .collect();
                                if pin.is_empty()
                                    || pads.iter().any(String::is_empty)
                                    || values
                                        .insert(format!("PINMAP:{pin}"), pads.join(","))
                                        .is_some()
                                {
                                    return Err("Invalid or duplicate footprint pin map".into());
                                }
                            }
                        }
                        values
                    },
                    name: r.fields.text("MODELNAME"),
                    kind: r.fields.text("MODELTYPE"),
                    current: r.fields.yes("ISCURRENT"),
                });
        }
    }
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
                params.insert(
                    child.fields.text("NAME"),
                    child.fields.text("TEXT").replace("~1", "\n"),
                );
            }
        }
        let part = r.fields.n("CURRENTPARTID").max(1) as u32;
        if r.fields.n("PARTCOUNT") > 2 {
            designator.push(char::from_u32('A' as u32 + part - 1).unwrap_or('?'));
        }
        components.push(Component {
            settings: eda_core::schematic::ComponentSettings {
                display_mode_names: (1..32)
                    .filter_map(|mode| {
                        r.fields
                            .get(&format!("CUSTOMDISPLAYMODENAME{mode}"))
                            .map(|name| (mode, name.clone()))
                    })
                    .collect(),
                models: models.remove(&(r.index as usize)).unwrap_or_default(),
                rotation: r.fields.n("ORIENTATION") as u32 % 4,
                part_count: (r.fields.n("PARTCOUNT") - 1).clamp(1, 1024) as u32,
                kind: r
                    .fields
                    .n(if r.fields.contains_key("COMPONENTKINDVERSION2") {
                        "COMPONENTKINDVERSION2"
                    } else {
                        "COMPONENTKIND"
                    }) as u32,
                source: r.fields.text("SOURCELIBRARYNAME"),
                design_item_id: r.fields.text("DESIGNITEMID"),
                designator_locked: r.fields.yes("DESIGNATORLOCKED"),
                part_locked: r.fields.yes("PARTIDLOCKED"),
            },
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
        options: eda_core::schematic::SheetOptions {
            border_color: f.n("BORDERCOLOR") as u32,
            show_zones: !f.yes("REFERENCEZONESOFF"),
            snap_enabled: !f.contains_key("SNAPGRIDON") || f.yes("SNAPGRIDON"),
            electrical_snap: !f.contains_key("HOTSPOTGRIDON") || f.yes("HOTSPOTGRIDON"),
            snap_distance_nm: if f.contains_key("HOTSPOTGRIDSIZE") {
                f.coord("HOTSPOTGRIDSIZE")
            } else {
                2_540_000
            },
        },
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
    let mut parameter_objects = vec![];
    let component_lookup: BTreeMap<u32, &Component> =
        components.iter().map(|c| (c.source_index, c)).collect();
    let mut unsupported = BTreeMap::<u32, usize>::new();
    let show_template = f.yes("SHOWTEMPLATEGRAPHICS");
    let default_font = f.n("SYSTEMFONT").max(1) as usize - 1;
    for (index, r) in records.iter().enumerate() {
        let visible = if matches!(r.record_type, 215..=218) {
            !r.fields.yes("ISHIDDEN")
        } else {
            is_visible(&records, index, show_template)
        };
        let hidden_parameter = r.record_type == 41
            && r.fields.yes("ISHIDDEN")
            && (owner(&r.fields) == 0
                || records
                    .get(owner(&r.fields))
                    .is_some_and(|p| matches!(p.record_type, 1 | 43 | 210)));
        let hidden_symbol_object = matches!(r.record_type, 2 | 34)
            && in_active_variant(&records, index, show_template, true);
        if (r.stream != "FileHeader" && !matches!(r.record_type, 215..=218 | 225))
            || (!visible && !hidden_parameter && !hidden_symbol_object && r.record_type != 217)
        {
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
            let p = SchPrimitive {
                presentation: eda_core::schematic::Presentation {
                    symbol_type: f.text("SYMBOLTYPE"),
                    parent: (matches!(r.record_type, 16 | 32 | 33 | 216 | 217)
                        || r.record_type == 41
                            && records
                                .get(owner(f))
                                .is_some_and(|p| matches!(p.record_type, 43 | 210)))
                    .then(|| uuid(id, harness_owner(&records, r) as u32)),
                    alignment: (r.record_type == 18).then(|| f.n("ALIGNMENT") as u32),
                    text_color: (r.record_type == 18).then(|| f.n("TEXTCOLOR") as u32),
                    harness_type: if matches!(r.record_type, 16 | 18 | 216) {
                        f.text("HARNESSTYPE")
                    } else {
                        String::new()
                    },
                    record_type: r.record_type,
                    url: f.text("URL"),
                    name: if r.record_type == 216 {
                        String::new()
                    } else if r.record_type == 34 {
                        "Designator".into()
                    } else {
                        f.text("NAME")
                    },
                    raw_text: matches!(r.record_type, 4 | 25 | 34 | 41 | 217)
                        .then(|| f.text("TEXT").replace("~1", "\n")),
                    hidden: !visible,
                    show_name: f.yes("SHOWNAME"),
                    auto_position: matches!(r.record_type, 34 | 41)
                        && !f.yes("AUTOPOSITIONOFF")
                        && !f.yes("NOTAUTOPOSITION"),
                    locked: f.yes("ISREADONLY"),
                    allow_sync: !f.yes("DISABLESYNC"),
                },
                text_role: match r.record_type {
                    34 => Some(eda_core::schematic::TextRole::Designator),
                    41 if f.text("NAME").eq_ignore_ascii_case("Comment") => {
                        Some(eda_core::schematic::TextRole::Comment)
                    }
                    25 => Some(eda_core::schematic::TextRole::NetLabel),
                    _ => None,
                },
                id: uuid(id, r.index),
                source_index: r.index,
                component,
                color,
                fill: if (f.yes("ISSOLID")
                    || matches!(r.record_type, 16 | 18 | 211 | 215 | 216 | 225))
                    && !f.yes("TRANSPARENT")
                {
                    Some(f.n("AREACOLOR") as u32)
                } else {
                    None
                },
                width_nm: match if r.record_type == 225
                    && !f.contains_key("LINEWIDTH")
                    && f.contains_key("LINESTYLE")
                {
                    f.n("LINESTYLE") + 1
                } else {
                    f.n(if r.record_type == 18 {
                        "BORDERWIDTH"
                    } else {
                        "LINEWIDTH"
                    })
                } {
                    0 => 101_600,
                    1 => 254_000,
                    2 => 508_000,
                    3 => 1_016_000,
                    _ => 254_000,
                },
                line_style: if r.record_type == 225 {
                    f.n("LINESTYLEEXT").max(0) as u32
                } else {
                    f.n("LINESTYLE") as u32
                },
                start_shape: f.n("STARTLINESHAPE") as u32,
                end_shape: f.n("ENDLINESHAPE") as u32,
                line_shape_size: f.n("LINESHAPESIZE") as u32,
                kind,
            };
            if visible
                || hidden_symbol_object
                || r.record_type == 217
                || r.record_type == 41
                    && records
                        .get(owner(f))
                        .is_some_and(|p| matches!(p.record_type, 43 | 210))
            {
                primitives.push(p);
            } else {
                parameter_objects.push(p);
            }
        }
    }
    for c in primitives
        .clone()
        .iter()
        .filter(|p| matches!(p.kind, SchPrimitiveKind::HarnessConnector { .. }))
    {
        let name = primitives
            .iter()
            .chain(&parameter_objects)
            .find_map(|p| {
                if p.presentation.parent == Some(c.id) {
                    if let SchPrimitiveKind::Text { text, .. } = &p.kind {
                        Some(text.clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap_or_default();
        primitives
            .iter_mut()
            .find(|p| p.id == c.id)
            .unwrap()
            .presentation
            .harness_type = name;
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
        source_modified: false,
        sheet,
        fonts,
        components,
        compile_masks: vec![],
        primitives,
        parameter_objects,
        records: records.into(),
        images: images.into(),
        source: Some(std::sync::Arc::new(NativeSource {
            format: "Altium SchDoc".into(),
            filename: name.into(),
            original_base64: STANDARD.encode(bytes),
        })),
        warnings,
    });
    if let Some(s) = &mut doc.schematic {
        eda_core::schematic_harness::infer_types(s);
    }
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
        5 | 6 | 7 | 26 | 27 | 218 => {
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
                if points.len() < 4 || !(points.len() - 1).is_multiple_of(3) {
                    return Err("Bezier requires a 3n+1 control-point chain".into());
                }
                K::Bezier { points }
            } else {
                K::Polyline {
                    points,
                    closed: r.record_type == 7,
                    electrical: match r.record_type {
                        26 => "Bus",
                        27 => "Wire",
                        218 => "Signal Harness",
                        _ => "Graphic",
                    }
                    .into(),
                }
            }
        }
        215 => K::HarnessConnector {
            a,
            b: Point {
                x: a.x + f.coord("XSIZE"),
                y: a.y - f.coord("YSIZE"),
            },
            side: f.n("HARNESSCONNECTORSIDE") as u32,
            primary_nm: f.coord("PRIMARYCONNECTIONPOSITION"),
            bus_text_style: if f.text("BUSTEXTSTYLE").is_empty() {
                "Full".into()
            } else {
                f.text("BUSTEXTSTYLE")
            },
        },
        216 => {
            let parent = records
                .get(harness_owner(records, r))
                .filter(|p| p.record_type == 215)
                .ok_or("Harness Entry has no Harness Connector owner")?;
            let a = parent.fields.point("LOCATION");
            let b = Point {
                x: a.x + parent.fields.coord("XSIZE"),
                y: a.y - parent.fields.coord("YSIZE"),
            };
            let distance =
                f.n("DISTANCEFROMTOP") as i128 * 1_000_000 + f.n("DISTANCEFROMTOP_FRAC1") as i128;
            let distance = i64::try_from(distance * 2_540_000 / 1_000_000)
                .map_err(|_| "Harness Entry distance overflow")?;
            let side = f.n("SIDE") as u32;
            K::HarnessEntry {
                position: eda_core::schematic_harness::edge(a, b, side, distance),
                side,
                name: f.text("NAME"),
                font: f.n("TEXTFONTID").max(1) as usize - 1,
                text_color: f.n("TEXTCOLOR") as u32,
            }
        }
        211 => K::CompileMask {
            a,
            b,
            collapsed: f.yes("COLLAPSED"),
        },
        225 => {
            let count = f.n("LOCATIONCOUNT");
            if !(0..=10000).contains(&count) {
                return Err("Invalid Blanket vertex count".into());
            }
            let mut points: Vec<_> = (1..=count)
                .map(|n| Point {
                    x: f.coord(&format!("X{n}")),
                    y: f.coord(&format!("Y{n}")),
                })
                .collect();
            if points.len() > 1 && points.first() == points.last() {
                points.pop();
            }
            if points.is_empty() {
                points = eda_core::schematic_directives::rectangle(a, b);
            }
            K::Blanket {
                points,
                collapsed: f.yes("COLLAPSED"),
            }
        }
        43 | 210 => K::ParameterSet {
            position: a,
            orientation,
            name: f.text("NAME"),
            style: f.n("STYLE") as u32,
            parameters: records
                .iter()
                .filter(|p| {
                    p.record_type == 41
                        && owner(&p.fields) == r.index as usize
                        && !p.fields.text("NAME").eq_ignore_ascii_case("Rule")
                })
                .map(|p| (p.fields.text("NAME"), p.fields.text("TEXT")))
                .collect(),
            rules: records
                .iter()
                .filter(|p| {
                    p.record_type == 41
                        && owner(&p.fields) == r.index as usize
                        && p.fields.text("NAME").eq_ignore_ascii_case("Rule")
                })
                .map(|p| {
                    p.fields
                        .text("TEXT")
                        .split('¦')
                        .filter_map(|s| s.split_once('='))
                        .map(|(k, v)| (k.to_ascii_uppercase(), v.into()))
                        .collect()
                })
                .collect(),
        },
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
        4 | 25 | 28 | 32 | 33 | 34 | 41 | 209 | 217 => {
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
            if text.is_empty() && r.record_type != 41 {
                return Ok(None);
            }
            // Omitted enum fields use the native zero value (Bottom Left).
            // Reading direction is a display preference, not a different
            // document anchor for 180/270-degree strings.
            let justification = f.n("JUSTIFICATION") as u32;
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
            // Quantize the absolute electrical endpoint once. Rounding position
            // and length independently can sever a native wire by one nm.
            let axis = if flags & 1 == 0 {
                "LOCATION.X"
            } else {
                "LOCATION.Y"
            };
            let position = coord_units(f, axis);
            let length = coord_units(f, "PINLENGTH");
            if length < 0 {
                return Err("Negative schematic pin length".into());
            }
            let endpoint = coord_nm(position + if flags & 2 == 0 { length } else { -length });
            let length_nm = (endpoint - coord_nm(position)).abs();
            K::Pin {
                position: a,
                length_nm,
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
            cross_sheet: f.yes("ISCROSSSHEETCONNECTOR"),
            position: a,
            orientation,
            style: f.n("STYLE") as u32,
            text: f.text("TEXT"),
            show_name: f.yes("SHOWNETNAME"),
            font,
        },
        18 => K::Port {
            position: a,
            width_nm: {
                let (axis, origin) = if f.n("STYLE") >= 4 {
                    ("LOCATION.Y", a.y)
                } else {
                    ("LOCATION.X", a.x)
                };
                coord_nm(coord_units(f, axis) + coord_units(f, "WIDTH")) - origin
            },
            height_nm: if f.contains_key("HEIGHT") {
                f.coord("HEIGHT")
            } else {
                2_540_000
            },
            style: f.n("STYLE") as u32,
            io_type: f.n("IOTYPE") as u32,
            name: f.text("NAME"),
            font,
        },
        15 => K::SheetSymbol {
            a,
            b: Point {
                x: coord_nm(coord_units(f, "LOCATION.X") + coord_units(f, "XSIZE")),
                y: coord_nm(coord_units(f, "LOCATION.Y") - coord_units(f, "YSIZE")),
            },
            designator: records
                .iter()
                .find(|child| child.record_type == 32 && owner(&child.fields) == r.index as usize)
                .map_or(String::new(), |child| child.fields.text("TEXT")),
            filename: records
                .iter()
                .find(|child| child.record_type == 33 && owner(&child.fields) == r.index as usize)
                .map_or(String::new(), |child| child.fields.text("TEXT")),
        },
        16 => {
            let parent = &records
                .get(owner(f))
                .ok_or("Invalid sheet entry owner")?
                .fields;
            let loc = parent.point("LOCATION");
            let side = f.n("SIDE");
            // Unlike schematic LOCATION fields, this is measured in 100 mil
            // units, with a millionth fractional field named _FRAC1.
            let units =
                f.n("DISTANCEFROMTOP") as i128 * 1_000_000 + f.n("DISTANCEFROMTOP_FRAC1") as i128;
            let d_units = units * 10;
            let absolute = |origin: &str, size: Option<&str>, sign: i128| {
                let units = (coord_units(parent, origin)
                    + size.map_or(0, |key| coord_units(parent, key)))
                    * 10
                    + sign * d_units;
                let scaled = units * 254_000;
                ((scaled + if scaled < 0 { -500_000 } else { 500_000 }) / 1_000_000) as i64
            };
            let p = match side {
                0 => Point {
                    x: loc.x,
                    y: absolute("LOCATION.Y", None, -1),
                },
                1 => Point {
                    x: absolute("LOCATION.X", Some("XSIZE"), 0),
                    y: absolute("LOCATION.Y", None, -1),
                },
                2 => Point {
                    x: absolute("LOCATION.X", None, 1),
                    y: loc.y,
                },
                _ => Point {
                    x: absolute("LOCATION.X", None, 1),
                    y: coord_nm(coord_units(parent, "LOCATION.Y") - coord_units(parent, "YSIZE")),
                },
            };
            K::SheetEntry {
                position: p,
                side: side as u32,
                io_type: f.n("IOTYPE") as u32,
                name: f.text("NAME"),
                font: f
                    .get("TEXTFONTID")
                    .map_or(font, |_| f.n("TEXTFONTID").max(1) as usize - 1),
                text_color: f.n("TEXTCOLOR") as u32,
                arrow_kind: f.text("ARROWKIND"),
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
            embedded: f.yes("EMBEDIMAGE"),
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

type StoredImages = Vec<(Vec<u8>, Vec<u8>)>;

fn decode_image_storage(data: &[u8], warnings: &mut Vec<String>) -> Result<StoredImages, String> {
    let mut stored_images = vec![];
    let mut blocks = vec![];
    framed("Storage", data, &mut blocks)?;
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
    Ok(stored_images)
}
