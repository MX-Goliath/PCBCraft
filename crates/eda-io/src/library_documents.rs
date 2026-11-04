//! Editable library documents and native file-based library export.
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::{Document, DocumentKind, library_edit as edit, pcb, schematic as sch};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read, Write},
    path::Path,
};

const LIMIT: usize = 128 * 1024 * 1024;
type Result<T> = std::result::Result<T, String>;

pub fn load(path: &Path) -> Result<Document> {
    let bytes = crate::altium::read_bounded(path)?;
    import(
        &bytes,
        &path.file_name().unwrap_or_default().to_string_lossy(),
        path.extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("PcbLib")),
    )
}
pub fn import(bytes: &[u8], filename: &str, pcb_library: bool) -> Result<Document> {
    let mut entries = vec![];
    if pcb_library {
        let lib = crate::altium_pcb::library::import(bytes, filename)?;
        if !lib.warnings.is_empty() {
            return Err(format!(
                "Library cannot be edited without losing unsupported data: {}",
                lib.warnings.join("; ")
            ));
        }
        for footprint in lib.footprints {
            let mut doc = edit::member(DocumentKind::Board, footprint.name())?;
            doc.pcb = Some(footprint.board().clone());
            entries.push(doc);
        }
    } else {
        entries = crate::altium::library::editable(bytes, filename)?;
    }
    edit::assemble(
        entries,
        filename,
        Some(pcb::SourceArchive {
            filename: filename.into(),
            format: if pcb_library {
                "Altium PcbLib"
            } else {
                "Altium SchLib"
            }
            .into(),
            bytes_base64: STANDARD.encode(bytes),
        }),
    )
}

fn write_stream(
    file: &mut cfb::CompoundFile<Cursor<Vec<u8>>>,
    path: &str,
    bytes: &[u8],
) -> Result<()> {
    if let Some(parent) = Path::new(path).parent()
        && !parent.as_os_str().is_empty()
    {
        file.create_storage_all(parent).map_err(|e| e.to_string())?;
    }
    file.create_stream(path)
        .map_err(|e| e.to_string())?
        .write_all(bytes)
        .map_err(|e| e.to_string())
}
fn frame(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() > 0xffffff {
        return Err("Native library record exceeds 24-bit size limit".into());
    }
    Ok([&(data.len() as u32).to_le_bytes()[..], data].concat())
}
fn properties(fields: &pcb::Fields) -> Result<Vec<u8>> {
    frame(&crate::altium::writer::library_property_bytes(fields)?)
}
fn section_key(name: &str, used: &mut std::collections::BTreeSet<String>) -> String {
    let mut base = String::new();
    let mut units = 0;
    for ch in name.chars() {
        let ch = if matches!(ch, '/' | '\\' | ':' | '!') {
            '_'
        } else {
            ch
        };
        if units + ch.len_utf16() > 31 {
            break;
        }
        units += ch.len_utf16();
        base.push(ch);
    }
    (0..)
        .map(|n| {
            if n == 0 {
                base.clone()
            } else {
                format!("{}_{}", base.chars().take(16).collect::<String>(), n)
            }
        })
        .find(|s| {
            !["Library", "FileHeader", "Storage", "SectionKeys"]
                .iter()
                .any(|r| s.eq_ignore_ascii_case(r))
                && used.insert(s.to_lowercase())
        })
        .unwrap()
}
fn unchanged(doc: &Document) -> Result<Option<Vec<u8>>> {
    let Some(source) = doc.library.as_ref().and_then(|b| b.source.as_ref()) else {
        return Ok(None);
    };
    let bytes = STANDARD
        .decode(&source.bytes_base64)
        .map_err(|e| e.to_string())?;
    let original = import(&bytes, &source.filename, doc.kind == DocumentKind::Board)?;
    let equal = edit::members(doc).count() == edit::members(&original).count()
        && edit::members(doc).all(|m| {
            edit::members(&original).any(|o| {
                edit::name(m) == edit::name(o)
                    && edit::variant(m) == edit::variant(o)
                    && m.schematic == o.schematic
                    && m.pcb == o.pcb
            })
        });
    Ok(equal.then_some(bytes))
}

pub fn serialize(doc: &Document) -> Result<Vec<u8>> {
    doc.validate()?;
    if doc.library.is_none() {
        return Err("A library document is required".into());
    }
    if let Some(bytes) = unchanged(doc)? {
        return Ok(bytes);
    }
    match doc.kind {
        DocumentKind::Schematic => schlib(doc),
        DocumentKind::Board => pcblib(doc),
    }
}
fn schlib(doc: &Document) -> Result<Vec<u8>> {
    let mut groups = BTreeMap::<String, Vec<&Document>>::new();
    let mut fonts = vec![];
    for m in edit::members(doc) {
        groups.entry(edit::name(m).into()).or_default().push(m);
        for font in &m.schematic.as_ref().unwrap().fonts {
            if !fonts.contains(font) {
                fonts.push(font.clone());
            }
        }
        if !m.schematic.as_ref().unwrap().images.is_empty() {
            return Err("Embedded symbol images require PCBCraft JSON until native library image storage is verified".into());
        }
    }
    let mut header = doc
        .schematic
        .as_ref()
        .unwrap()
        .records
        .first()
        .filter(|r| r.record_type == 31)
        .map(|r| r.fields.clone())
        .unwrap_or_default();
    header.retain(|k, _| {
        k != "RECORD"
            && ![
                "LIBREF",
                "PARTCOUNT",
                "FONTNAME",
                "SIZE",
                "BOLD",
                "ITALIC",
                "UNDERLINE",
            ]
            .iter()
            .any(|prefix| {
                k.strip_prefix(prefix)
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            })
    });
    header.extend(pcb::Fields::from([
        (
            "HEADER".into(),
            "Protel for Windows - Schematic Library Editor Binary File Version 5.0".into(),
        ),
        ("MINORVERSION".into(), "9".into()),
        ("COMPCOUNT".into(), groups.len().to_string()),
        ("FONTIDCOUNT".into(), fonts.len().to_string()),
        ("USEMBCS".into(), "T".into()),
        ("USECUSTOMSHEET".into(), "T".into()),
        ("CUSTOMX".into(), "18000".into()),
        ("CUSTOMY".into(), "18000".into()),
        ("SNAPGRIDON".into(), "T".into()),
        ("SNAPGRIDSIZE".into(), "10".into()),
        ("VISIBLEGRIDON".into(), "T".into()),
        ("VISIBLEGRIDSIZE".into(), "10".into()),
        ("AREACOLOR".into(), "16317695".into()),
    ]));
    for (n, font) in fonts.iter().enumerate() {
        for (key, value) in [
            ("FONTNAME", font.name.clone()),
            ("SIZE", (font.height_nm / 254_000).to_string()),
            ("BOLD", if font.bold { "T" } else { "F" }.into()),
            ("ITALIC", if font.italic { "T" } else { "F" }.into()),
            ("UNDERLINE", if font.underline { "T" } else { "F" }.into()),
        ] {
            header.insert(format!("{key}{}", n + 1), value);
        }
    }
    let mut file = schematic_container(doc)?;
    let mut used = std::collections::BTreeSet::new();
    let mut sections = pcb::Fields::new();
    let mut section_count = 0;
    for (n, (name, mut variants)) in groups.into_iter().enumerate() {
        let section = section_key(&name, &mut used);
        if section != name {
            sections.insert(format!("LIBREF{section_count}"), name.clone());
            sections.insert(format!("SECTIONKEY{section_count}"), section.clone());
            section_count += 1;
        }
        variants.sort_by_key(|m| edit::variant(m));
        let parts = variants.iter().map(|m| edit::variant(m).0).max().unwrap();
        let modes = variants.iter().map(|m| edit::variant(m).1).max().unwrap() + 1;
        if variants.len() != (parts * modes) as usize {
            return Err(
                "All symbol parts and display modes must exist before native export".into(),
            );
        }
        header.insert(format!("LIBREF{n}"), name.clone());
        header.insert(format!("PARTCOUNT{n}"), (parts + 1).to_string());
        let mut output = vec![];
        for (variant_index, member) in variants.iter().enumerate() {
            let mut scene = member.schematic.as_ref().unwrap().clone();
            let remap: Vec<_> = scene
                .fonts
                .iter()
                .map(|f| fonts.iter().position(|v| v == f).unwrap())
                .collect();
            for p in scene
                .primitives
                .iter_mut()
                .chain(&mut scene.parameter_objects)
            {
                match &mut p.kind {
                    sch::SchPrimitiveKind::Text { font, .. }
                    | sch::SchPrimitiveKind::TextFrame { font, .. } => *font = remap[*font],
                    sch::SchPrimitiveKind::Pin {
                        font, number_font, ..
                    } => {
                        *font = remap[*font];
                        if let Some(f) = number_font {
                            *f = remap[*f];
                        }
                    }
                    _ => {}
                }
            }
            scene.fonts = fonts.clone();
            let mut records = crate::altium::writer::library_records(&scene)?;
            records[0].insert("PARTCOUNT".into(), (parts + 1).to_string());
            records[0].insert("DISPLAYMODECOUNT".into(), modes.to_string());
            let mut mapping = BTreeMap::from([(0i64, 0i64)]);
            for (index, f) in records.iter().enumerate() {
                let kind = f
                    .get("RECORD")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(0);
                if variant_index == 0 || !matches!(kind, 1 | 34 | 41 | 44..=48) {
                    mapping.insert(
                        index as i64,
                        (output.len() + mapping.len().saturating_sub(1)) as i64,
                    );
                }
            }
            let base = output.len();
            let selected: Vec<_> = records
                .into_iter()
                .enumerate()
                .filter(|(i, _)| variant_index == 0 || *i > 0 && mapping.contains_key(&(*i as i64)))
                .collect();
            let mut actual = BTreeMap::from([(0i64, 0i64)]);
            for (i, (old, _)) in selected.iter().enumerate() {
                actual.insert(*old as i64, (base + i) as i64);
            }
            for (old, mut f) in selected {
                if old > 0 {
                    let owner = f
                        .get("OWNERINDEX")
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);
                    f.insert(
                        "OWNERINDEX".into(),
                        actual
                            .get(&owner)
                            .ok_or("Library owner was omitted")?
                            .to_string(),
                    );
                }
                output.push(f);
            }
        }
        let mut data = vec![];
        for f in &output {
            data.extend(properties(f)?);
        }
        write_stream(&mut file, &format!("{section}/Data"), &data)?;
    }
    if section_count > 0 {
        sections.insert("KEYCOUNT".into(), section_count.to_string());
        write_stream(&mut file, "SectionKeys", &properties(&sections)?)?;
    }
    write_stream(&mut file, "FileHeader", &properties(&header)?)?;
    if !file.is_stream("Storage") {
        write_stream(
            &mut file,
            "Storage",
            &properties(&pcb::Fields::from([
                ("HEADER".into(), "Icon storage".into()),
                ("WEIGHT".into(), "0".into()),
            ]))?,
        )?;
    }
    file.flush().map_err(|e| e.to_string())?;
    let bytes = file.into_inner().into_inner();
    let read = import(&bytes, "export.SchLib", false)?;
    if edit::members(&read).count() != edit::members(doc).count() {
        return Err("SchLib verification changed the member count".into());
    }
    Ok(bytes)
}

/// Keep unrelated streams verbatim. Per-symbol auxiliary streams are decoded
/// into the text pin records; unfamiliar symbol data must never be discarded.
fn schematic_container(doc: &Document) -> Result<cfb::CompoundFile<Cursor<Vec<u8>>>> {
    let mut output = cfb::CompoundFile::create(Cursor::new(vec![])).map_err(|e| e.to_string())?;
    let Some(source) = doc.library.as_ref().and_then(|b| b.source.as_ref()) else {
        return Ok(output);
    };
    let bytes = STANDARD
        .decode(&source.bytes_base64)
        .map_err(|e| e.to_string())?;
    let mut source = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let paths: Vec<_> = source
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_owned())
        .collect();
    let symbols: Vec<_> = paths
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n == "Data"))
        .filter_map(|p| p.parent())
        .collect();
    for path in &paths {
        if let Some(parent) = symbols.iter().find(|p| path.starts_with(p)) {
            let relative = path.strip_prefix(parent).map_err(|e| e.to_string())?;
            if ![
                "Data",
                "PinFrac",
                "PinTextData",
                "PinWideText",
                "PinSymbolLineWidth",
            ]
            .iter()
            .any(|p| relative == Path::new(p))
            {
                return Err(format!(
                    "Unrepresented symbol stream {}; save the complete library in PCBCraft JSON",
                    path.display()
                ));
            }
            continue;
        }
        if path == Path::new("/FileHeader") || path == Path::new("/SectionKeys") {
            continue;
        }
        let bytes = read_stream(&mut source, &path.to_string_lossy())?;
        write_stream(&mut output, &path.to_string_lossy(), &bytes)?;
    }
    Ok(output)
}

fn read_stream(file: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, path: &str) -> Result<Vec<u8>> {
    let mut bytes = vec![];
    if file.is_stream(path) {
        file.open_stream(path)
            .map_err(|e| e.to_string())?
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
    }
    if bytes.len() > LIMIT {
        return Err("Library stream exceeds size limit".into());
    }
    Ok(bytes)
}
fn pcblib(doc: &Document) -> Result<Vec<u8>> {
    let mut file = pcb_container(doc)?;
    let title = "PCB 6.0 Binary Library File";
    let mut header = (title.len() as u32).to_le_bytes().to_vec();
    header.push(title.len() as u8);
    header.extend(title.as_bytes());
    header.extend(5.01f64.to_le_bytes());
    let uid = doc.id.simple().to_string()[..8].to_uppercase();
    header.extend(8u32.to_le_bytes());
    header.push(8);
    header.extend(uid.as_bytes());
    write_stream(&mut file, "FileHeader", &header)?;
    let mut settings = doc.pcb.as_ref().unwrap().properties.clone();
    settings.insert("KIND".into(), "Protel_Advanced_PCB_Library".into());
    settings.insert("VERSION".into(), "3.00".into());
    settings.insert("BOARDVERSION".into(), "5.01".into());
    for layer in &doc.pcb.as_ref().unwrap().layers {
        settings.insert(format!("LAYER{}NAME", layer.id), layer.name.clone());
    }
    let mut library = properties(&settings)?;
    let mut sections = vec![];
    let mut used = std::collections::BTreeSet::new();
    let mut toc = vec![];
    library.extend((edit::members(doc).count() as u32).to_le_bytes());
    for member in edit::members(doc) {
        let name = edit::name(member);
        let section = section_key(name, &mut used);
        if section != name {
            sections.push((name.to_owned(), section.clone()));
        }
        if name.len() > 255 || !name.is_ascii() {
            return Err(
                "Unicode footprint names require verified native name encoding; use PCBCraft JSON"
                    .into(),
            );
        }
        let mut board = member.pcb.as_ref().unwrap().clone();
        let mut index = footprint_index_parameters(member);
        index.insert("Name".into(), name.into());
        index.insert(
            "Description".into(),
            board.components[0]
                .properties
                .get("DESCRIPTION")
                .cloned()
                .unwrap_or_default(),
        );
        toc.extend(parameter_record(&index)?);
        board.source = None;
        for item in &mut board.items {
            item.source = None;
        }
        let mut tmp = Document::new(DocumentKind::Board, "footprint.PcbDoc");
        tmp.pcb = Some(board.clone());
        let bytes =
            crate::altium_pcb::writer::serialize(&tmp, crate::altium_pcb::writer::Format::Binary)?;
        let mut native = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let mut short = vec![name.len() as u8];
        short.extend(name.as_bytes());
        library.extend(frame(&short)?);
        let mut data = frame(&short)?;
        for family in [
            "Arcs6",
            "Pads6",
            "Vias6",
            "Tracks6",
            "Texts6",
            "Fills6",
            "Regions6",
            "ShapeBasedRegions6",
            "ComponentBodies6",
            "ShapeBasedComponentBodies6",
        ] {
            data.extend(read_stream(&mut native, &format!("{family}/Data"))?);
        }
        let mut params = board.components[0].properties.clone();
        params
            .entry("HEIGHT".into())
            .or_insert_with(|| "0mil".into());
        params.entry("DESCRIPTION".into()).or_default();
        params.insert("PATTERN".into(), name.into());
        write_stream(
            &mut file,
            &format!("{section}/Parameters"),
            &properties(&params)?,
        )?;
        write_stream(
            &mut file,
            &format!("{section}/Header"),
            &(board.items.len() as u32).to_le_bytes(),
        )?;
        write_stream(&mut file, &format!("{section}/Data"), &data)?;
        let wide_data = read_stream(&mut native, "WideStrings6/Data")?;
        write_stream(
            &mut file,
            &format!("{section}/PrimitiveParameters"),
            &footprint_parameters(&board.components[0])?,
        )?;
        let mut wide = pcb::Fields::new();
        let mut offset = 0;
        while offset + 8 <= wide_data.len() {
            let index = u32::from_le_bytes(wide_data[offset..offset + 4].try_into().unwrap());
            let size =
                u32::from_le_bytes(wide_data[offset + 4..offset + 8].try_into().unwrap()) as usize;
            offset += 8;
            let value = wide_data
                .get(offset..offset + size)
                .ok_or("Invalid wide-string size")?;
            wide.insert(
                format!("ENCODEDTEXT{index}"),
                value
                    .chunks_exact(2)
                    .map(|w| u16::from_le_bytes([w[0], w[1]]).to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            );
            offset += size;
        }
        if !wide.is_empty() {
            write_stream(
                &mut file,
                &format!("{section}/WideStrings"),
                &properties(&wide)?,
            )?;
        } else {
            write_stream(
                &mut file,
                &format!("{section}/WideStrings"),
                &[1, 0, 0, 0, 0],
            )?;
        }
        if !board.models.is_empty() {
            return Err("Native export of footprint 3D resources is not verified; keep this library in PCBCraft JSON".into());
        }
    }
    write_stream(&mut file, "Library/Data", &library)?;
    write_stream(&mut file, "Library/Header", &1u32.to_le_bytes())?;
    write_stream(&mut file, "Library/ComponentParamsTOC/Data", &toc)?;
    write_stream(
        &mut file,
        "Library/ComponentParamsTOC/Header",
        &(edit::members(doc).count() as u32).to_le_bytes(),
    )?;
    for family in ["Models", "ModelsNoEmbed", "Textures"] {
        write_stream(&mut file, &format!("Library/{family}/Data"), &[])?;
        write_stream(
            &mut file,
            &format!("Library/{family}/Header"),
            &0u32.to_le_bytes(),
        )?;
    }
    write_stream(&mut file, "Library/EmbeddedFonts", &0u32.to_le_bytes())?;
    if !sections.is_empty() {
        let mut data = (sections.len() as u32).to_le_bytes().to_vec();
        for (name, section) in sections {
            for value in [name, section] {
                let mut s = vec![value.len() as u8];
                s.extend(value.as_bytes());
                data.extend(frame(&s)?);
            }
        }
        write_stream(&mut file, "SectionKeys", &data)?;
    }
    file.flush().map_err(|e| e.to_string())?;
    let bytes = file.into_inner().into_inner();
    let read = import(&bytes, "export.PcbLib", true)?;
    if edit::members(&read).count() != edit::members(doc).count() {
        return Err("PcbLib verification changed the member count".into());
    }
    Ok(bytes)
}

fn pcb_container(doc: &Document) -> Result<cfb::CompoundFile<Cursor<Vec<u8>>>> {
    let mut output = cfb::CompoundFile::create(Cursor::new(vec![])).map_err(|e| e.to_string())?;
    let Some(source) = doc.library.as_ref().and_then(|b| b.source.as_ref()) else {
        return Ok(output);
    };
    let bytes = STANDARD
        .decode(&source.bytes_base64)
        .map_err(|e| e.to_string())?;
    let mut source = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let paths: Vec<_> = source
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_owned())
        .collect();
    let footprints: Vec<_> = paths
        .iter()
        .filter(|p| {
            p.file_name().is_some_and(|n| n == "Data")
                && p.parent()
                    .is_some_and(|parent| source.is_stream(parent.join("Parameters")))
        })
        .filter_map(|p| p.parent())
        .collect();
    for path in &paths {
        if let Some(parent) = footprints.iter().find(|p| path.starts_with(p)) {
            let relative = path.strip_prefix(parent).map_err(|e| e.to_string())?;
            if ![
                "Data",
                "Header",
                "Parameters",
                "WideStrings",
                "PrimitiveParameters",
            ]
            .iter()
            .any(|p| relative == Path::new(p))
            {
                return Err(format!(
                    "Native footprint auxiliary stream {} requires verified reference remapping; save the complete edited library in PCBCraft JSON",
                    path.display()
                ));
            }
            continue;
        }
        if path == Path::new("/FileHeader")
            || path == Path::new("/SectionKeys")
            || path.starts_with("/Library/ComponentParamsTOC")
        {
            continue;
        }
        let bytes = read_stream(&mut source, &path.to_string_lossy())?;
        write_stream(&mut output, &path.to_string_lossy(), &bytes)?;
    }
    Ok(output)
}

fn footprint_parameters(component: &pcb::Component) -> Result<Vec<u8>> {
    let values = component.parameters.as_ref().cloned().unwrap_or_default();
    let mut parameters = 3u32.to_le_bytes().to_vec();
    for (appurtenance, variant, fields) in [
        ("Footprint", "Footprint", values),
        ("System", "System", pcb::Fields::new()),
        ("", "", pcb::Fields::new()),
    ] {
        let mut header = pcb::Fields::from([
            ("PRIMITIVEID".into(), "NoObject#0".into()),
            ("VARIANTGUID".into(), variant.into()),
            ("COUNT".into(), fields.len().to_string()),
        ]);
        if !appurtenance.is_empty() {
            header.insert("APPURTENANCE".into(), appurtenance.into());
        }
        parameters.extend(properties(&header)?);
        for (name, value) in fields {
            parameters.extend(properties(&pcb::Fields::from([
                ("NAME".into(), name),
                ("VALUE".into(), value),
            ]))?);
        }
    }
    Ok(parameters)
}

pub fn save(path: &Path, doc: &Document) -> Result<()> {
    let pcb_file = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("PcbLib"));
    if pcb_file != (doc.kind == DocumentKind::Board) {
        return Err("Library type does not match the file extension".into());
    }
    atomic(path, &serialize(doc)?)
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if bytes.len() > LIMIT {
        return Err("Library exceeds save limit".into());
    }
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Refusing to overwrite a symlink".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    tmp.write_all(bytes).map_err(|e| e.to_string())?;
    tmp.as_file().sync_all().map_err(|e| e.to_string())?;
    if path.exists() {
        let mut n = 0u32;
        loop {
            let backup = path.with_file_name(format!(
                "{}.bak{}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                if n == 0 {
                    String::new()
                } else {
                    format!(".{n}")
                }
            ));
            match std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&backup)
            {
                Ok(mut backup) => {
                    std::io::copy(
                        &mut std::fs::File::open(path).map_err(|e| e.to_string())?,
                        &mut backup,
                    )
                    .map_err(|e| e.to_string())?;
                    backup.sync_all().map_err(|e| e.to_string())?;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    n = n.checked_add(1).ok_or("Backup counter exhausted")?;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }
    tmp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Compile portable native sources after checking every referenced footprint.
pub fn compile_integrated(
    path: &Path,
    schematic: &Document,
    footprints: &[&Document],
) -> Result<()> {
    if schematic.kind != DocumentKind::Schematic || schematic.library.is_none() {
        return Err("Select a schematic library to compile".into());
    }
    schematic.validate()?;
    for footprint in footprints {
        if footprint.kind != DocumentKind::Board || footprint.library.is_none() {
            return Err(
                "Integrated compilation accepts PCB library documents as footprint sources".into(),
            );
        }
        footprint.validate()?;
    }
    for member in edit::members(schematic) {
        let c = &member.schematic.as_ref().unwrap().components[0];
        for model in c
            .settings
            .models
            .iter()
            .filter(|m| m.kind.eq_ignore_ascii_case("PCBLIB"))
        {
            let candidates: Vec<_> = footprints
                .iter()
                .flat_map(|d| edit::members(d))
                .filter(|d| edit::name(d) == model.name)
                .collect();
            if candidates.len() != 1 {
                return Err(format!(
                    "{}: footprint '{}' must resolve to exactly one available definition",
                    c.library_reference, model.name
                ));
            }
            let pads: Vec<_> = candidates[0]
                .pcb
                .as_ref()
                .unwrap()
                .items
                .iter()
                .filter_map(|i| {
                    if let pcb::Shape::Pad { name, .. } = &i.shape {
                        Some(name.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            for p in &member.schematic.as_ref().unwrap().primitives {
                if let sch::SchPrimitiveKind::Pin { number, .. } = &p.kind {
                    let targets = model
                        .parameters
                        .get(&format!("PINMAP:{number}"))
                        .map_or(number.as_str(), String::as_str);
                    if targets.split(',').any(|pad| !pads.contains(&pad.trim())) {
                        return Err(format!(
                            "{} pin {number}: no matching pad in {}",
                            c.library_reference, model.name
                        ));
                    }
                }
            }
        }
    }
    let mut file = cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(vec![]))
        .map_err(|e| e.to_string())?;
    let mut symbols = BTreeMap::new();
    for m in edit::members(schematic) {
        symbols.entry(edit::name(m).to_owned()).or_insert(m);
    }
    let mut crossref = (symbols.len() as u32).to_le_bytes().to_vec();
    let mut parameter_data = vec![];
    for (name, member) in symbols {
        let c = &member.schematic.as_ref().unwrap().components[0];
        let source_name = source_filename(schematic, "Symbols.SchLib");
        crossref.extend(native_short(&name)?);
        crossref.extend(native_short(":\\SchLib\\0.schlib")?);
        crossref.extend(c.settings.part_count.to_le_bytes());
        crossref.extend(native_short(&c.description)?);
        crossref.extend(native_short(&source_name)?);
        crossref.extend((c.settings.models.len() as u32).to_le_bytes());
        let mut values = c.parameters.clone();
        values.insert("Comment".into(), c.comment.clone());
        values.insert("Description".into(), c.description.clone());
        values.insert("Designator".into(), c.designator.clone());
        values.insert("Library Reference".into(), name);
        values.insert("Component Kind".into(), "Standard".into());
        values.insert("Component Type".into(), "Standard".into());
        let pins: std::collections::BTreeSet<_> = edit::members(schematic)
            .filter(|m| edit::name(m) == c.library_reference)
            .filter_map(|m| m.schematic.as_ref())
            .flat_map(|s| &s.primitives)
            .filter_map(|p| {
                if let sch::SchPrimitiveKind::Pin { number, .. } = &p.kind {
                    Some(number)
                } else {
                    None
                }
            })
            .collect();
        values.insert("Pin Count".into(), pins.len().to_string());
        for model in &c.settings.models {
            if model.kind.eq_ignore_ascii_case("PCBLIB") && model.current {
                values.insert("Footprint".into(), model.name.clone());
            }
        }
        parameter_data.extend(parameter_record(&values)?);
        for model in &c.settings.models {
            if !model.kind.eq_ignore_ascii_case("PCBLIB") {
                return Err("Integrated export of simulation/other model resources is not verified; keep these references in SchLib/PCBCraft JSON".into());
            }
            let index = footprints
                .iter()
                .position(|d| edit::members(d).any(|m| edit::name(m) == model.name))
                .ok_or("Footprint library is unavailable")?;
            crossref.extend(native_short(&model.name)?);
            crossref.extend(native_short("PCBLIB")?);
            crossref.extend(1u32.to_le_bytes());
            crossref.extend(native_short(&format!(":\\PCBLib\\{index}.pcblib"))?);
            crossref.extend(native_short(&source_filename(
                footprints[index],
                &format!("Footprints{index}.PcbLib"),
            ))?);
            let footprint = edit::members(footprints[index])
                .find(|d| edit::name(d) == model.name)
                .unwrap();
            parameter_data.extend(parameter_record(&footprint_index_parameters(footprint))?);
        }
    }
    for library in footprints {
        for member in edit::members(library) {
            parameter_data.extend(parameter_record(&footprint_index_parameters(member))?);
        }
    }
    write_stream(&mut file, "Version.Txt", &[0, 2, 0, 0, 0])?;
    write_stream(&mut file, "LibCrossRef.Txt", &compressed(&crossref)?)?;
    write_stream(
        &mut file,
        "Parameters   .bin",
        &compressed(&parameter_data)?,
    )?;
    write_stream(
        &mut file,
        "SchLib/0.schlib",
        &compressed(&serialize(schematic)?)?,
    )?;
    for (n, doc) in footprints.iter().enumerate() {
        write_stream(
            &mut file,
            &format!("PCBLib/{n}.pcblib"),
            &compressed(&serialize(doc)?)?,
        )?;
    }
    file.flush().map_err(|e| e.to_string())?;
    atomic(path, &file.into_inner().into_inner())
}
fn source_filename(doc: &Document, fallback: &str) -> String {
    let name = Path::new(&doc.name)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let name = if name
        .to_ascii_lowercase()
        .ends_with(if doc.kind == DocumentKind::Board {
            ".pcblib"
        } else {
            ".schlib"
        }) {
        name.into_owned()
    } else {
        fallback.into()
    };
    // Altium requires a local Windows source identity even for extracted
    // embedded sources (a basename produces "Expecting object to be local").
    // This logical identity keeps exports portable and deterministic on Linux.
    format!("C:\\PCBCraft\\{name}")
}
fn footprint_index_parameters(doc: &Document) -> pcb::Fields {
    let board = doc.pcb.as_ref().unwrap();
    let height = board.components[0]
        .properties
        .get("HEIGHT")
        .and_then(|v| crate::altium_pcb::dimension(v).ok())
        .unwrap_or(0);
    pcb::Fields::from([
        ("Height".into(), format!("{}", height as f64 / 25_400.)),
        (
            "Pad Count".into(),
            board
                .items
                .iter()
                .filter(|i| matches!(i.shape, pcb::Shape::Pad { .. }))
                .count()
                .to_string(),
        ),
    ])
}
fn native_short(value: &str) -> Result<Vec<u8>> {
    let (value, _, bad) = encoding_rs::WINDOWS_1252.encode(value);
    if bad || value.len() > 255 {
        return Err(
            "Integrated library index strings must fit the verified native 255-byte encoding"
                .into(),
        );
    }
    let mut bytes = vec![value.len() as u8];
    bytes.extend_from_slice(&value);
    frame(&bytes)
}
fn compressed(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = flate2::write::ZlibEncoder::new(vec![], flate2::Compression::default());
    encoder.write_all(bytes).map_err(|e| e.to_string())?;
    let mut result = vec![2];
    result.extend(encoder.finish().map_err(|e| e.to_string())?);
    Ok(result)
}
fn parameter_record(values: &pcb::Fields) -> Result<Vec<u8>> {
    let mut bytes = crate::altium::writer::library_property_bytes(values)?;
    if bytes.first() == Some(&b'|') {
        bytes.remove(0);
    }
    if bytes.last() != Some(&0) {
        bytes.push(0);
    }
    frame(&bytes)
}
