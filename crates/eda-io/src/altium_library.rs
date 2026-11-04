//! SchLib and IntLib containers, decoded without the Altium runtime.
use super::*;
use eda_core::library::{Library, Symbol, SymbolVariant};
use eda_core::schematic_edit;

type Compound = cfb::CompoundFile<Cursor<Vec<u8>>>;

pub fn load(path: &Path) -> Result<Library, String> {
    import(
        &read_bounded(path)?,
        &path.file_name().unwrap_or_default().to_string_lossy(),
    )
}

pub fn import(bytes: &[u8], name: &str) -> Result<Library, String> {
    if bytes.len() > MAX_FILE {
        return Err("Library exceeds import limit".into());
    }
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes.to_vec()))
        .map_err(|e| format!("Invalid library CFB: {e}"))?;
    if file.is_stream("FileHeader") {
        return schlib(file, name, None);
    }
    let paths: Vec<_> = file
        .walk()
        .filter(|e| {
            e.is_stream()
                && e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("schlib"))
        })
        .map(|e| e.path().to_owned())
        .collect();
    if paths.is_empty() {
        return Err("No embedded SchLib found in IntLib".into());
    }
    let mut result = Library {
        embedded_footprints: Some(file.walk().any(|e| {
            e.is_stream()
                && e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("pcblib"))
        })),
        name: name.into(),
        symbols: vec![],
        warnings: vec![],
    };
    let mut total = 0;
    for path in paths {
        let raw = stream(&mut file, &path)?;
        let decoded = match raw.first() {
            Some(2) => inflate(&raw[1..])?,
            Some(0xd0) => raw,
            _ => {
                return Err(format!(
                    "Unsupported IntLib compression in {}",
                    path.display()
                ));
            }
        };
        total += decoded.len();
        if total > MAX_FILE {
            return Err("Expanded IntLib exceeds import limit".into());
        }
        let inner = cfb::CompoundFile::open(Cursor::new(decoded))
            .map_err(|e| format!("Invalid embedded SchLib: {e}"))?;
        let library = schlib(inner, name, None)?;
        result.symbols.extend(library.symbols);
        result.warnings.extend(library.warnings);
    }
    result.warnings.sort();
    result.warnings.dedup();
    Ok(result)
}

fn stream(file: &mut Compound, path: &Path) -> Result<Vec<u8>, String> {
    let mut data = vec![];
    file.open_stream(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .take(MAX_FILE as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > MAX_FILE {
        return Err("Library stream exceeds import limit".into());
    }
    Ok(data)
}
fn inflate(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut result = vec![];
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    decoder
        .by_ref()
        .take(MAX_FILE as u64 + 1)
        .read_to_end(&mut result)
        .map_err(|e| format!("Invalid library compression: {e}"))?;
    if result.len() > MAX_FILE || decoder.total_in() != data.len() as u64 {
        return Err("Truncated, oversized or trailing compressed library data".into());
    }
    Ok(result)
}
fn auxiliary(file: &mut Compound, path: &Path) -> Result<BTreeMap<usize, Vec<u8>>, String> {
    if !file.is_stream(path) {
        return Ok(BTreeMap::new());
    }
    let data = stream(file, path)?;
    let mut blocks = vec![];
    framed("Library auxiliary", &data, &mut blocks)?;
    let mut values = BTreeMap::new();
    let mut total = 0;
    for (_, flag, block) in blocks {
        if flag == 0 {
            continue;
        }
        if block.first() != Some(&0xd0) {
            return Err("Unsupported library auxiliary framing".into());
        }
        let end = 2 + *block.get(1).ok_or("Truncated auxiliary key")? as usize;
        let key = std::str::from_utf8(block.get(2..end).ok_or("Truncated auxiliary key")?)
            .map_err(|e| e.to_string())?
            .parse()
            .map_err(|_| "Invalid auxiliary pin index")?;
        let size = u32::from_le_bytes(
            block
                .get(end..end + 4)
                .ok_or("Truncated auxiliary length")?
                .try_into()
                .unwrap(),
        ) as usize;
        if block.len() != end + 4 + size {
            return Err("Invalid auxiliary compressed size".into());
        }
        let decoded = inflate(&block[end + 4..])?;
        total += decoded.len();
        if total > MAX_FILE {
            return Err("Expanded auxiliary data exceeds limit".into());
        }
        if values.insert(key, decoded).is_some() {
            return Err("Duplicate auxiliary pin index".into());
        }
    }
    Ok(values)
}

/// Retain complete decoded source records for native library authoring.
pub(crate) fn editable(bytes: &[u8], name: &str) -> Result<Vec<Document>, String> {
    if bytes.len() > MAX_FILE {
        return Err("Library exceeds import limit".into());
    }
    let file = cfb::CompoundFile::open(Cursor::new(bytes.to_vec())).map_err(|e| e.to_string())?;
    let mut members = vec![];
    let library = schlib(file, name, Some(&mut members))?;
    if !library.warnings.is_empty() {
        return Err(format!(
            "Library cannot be edited without losing unsupported data: {}",
            library.warnings.join("; ")
        ));
    }
    Ok(members)
}
fn schlib(
    mut file: Compound,
    name: &str,
    mut editable: Option<&mut Vec<Document>>,
) -> Result<Library, String> {
    let header_bytes = stream(&mut file, Path::new("FileHeader"))?;
    let mut header_blocks = vec![];
    framed("Library header", &header_bytes, &mut header_blocks)?;
    let encoding = detect_encoding(&header_blocks);
    let mut header = decode_fields(
        &header_blocks.first().ok_or("Missing library header")?.2,
        encoding,
    )?;
    if !header
        .text("HEADER")
        .starts_with("Protel for Windows - Schematic Library Editor Binary File Version 5.0")
    {
        return Err("Unsupported SchLib version".into());
    }
    header.insert("RECORD".into(), "31".into());
    header.insert("USECUSTOMSHEET".into(), "F".into());
    header.insert("SHEETSTYLE".into(), "0".into());
    // Symbol previews have no document parameters or page graphics.
    let paths: Vec<_> = file
        .walk()
        .filter(|e| e.is_stream() && e.path().file_name().is_some_and(|x| x == "Data"))
        .map(|e| e.path().to_owned())
        .collect();
    if paths.is_empty() || paths.len() > 10_000 {
        return Err("Invalid SchLib component count".into());
    }
    let mut library = Library {
        embedded_footprints: None,
        name: name.into(),
        symbols: vec![],
        warnings: vec![],
    };
    let images = if file.is_stream("Storage") {
        let data = stream(&mut file, Path::new("Storage"))?;
        decode_image_storage(&data, &mut library.warnings)?
            .into_iter()
            .map(|(name, data)| {
                (
                    encoding.decode(&name).0.replace('\\', "/").to_lowercase(),
                    STANDARD.encode(data),
                )
            })
            .collect()
    } else {
        BTreeMap::new()
    };
    let mut total = 0;
    let mut objects = 0;
    let mut expanded_records = 0usize;
    let mut expanded_resources = 0usize;
    for path in paths {
        let bytes = stream(&mut file, &path)?;
        total += bytes.len();
        if total > MAX_FILE {
            return Err("Expanded SchLib exceeds import limit".into());
        }
        let mut blocks = vec![];
        framed("Symbol Data", &bytes, &mut blocks)?;
        let parent = path.parent().ok_or("Invalid symbol path")?;
        let fractions = auxiliary(&mut file, &parent.join("PinFrac"))?;
        let text_data = auxiliary(&mut file, &parent.join("PinTextData"))?;
        let wide_text = auxiliary(&mut file, &parent.join("PinWideText"))?;
        let symbol_widths = auxiliary(&mut file, &parent.join("PinSymbolLineWidth"))?;
        total += [&fractions, &text_data, &wide_text, &symbol_widths]
            .iter()
            .flat_map(|m| m.values())
            .map(Vec::len)
            .sum::<usize>();
        if total > MAX_FILE {
            return Err("Expanded SchLib auxiliary data exceeds limit".into());
        }
        let mut records = vec![SourceRecord {
            index: 0,
            stream: "FileHeader".into(),
            record_type: 31,
            fields: header.clone(),
        }];
        let mut pin = 0;
        for (_, flag, data) in blocks {
            let mut fields = if flag == 0 {
                decode_fields(&data, encoding)?
            } else if flag == 1 {
                let mut fields =
                    binary_pin(&data, encoding, fractions.get(&pin), text_data.get(&pin))?;
                if let Some(wide) = wide_text.get(&pin) {
                    for (k, v) in wide_fields(wide)?.into_iter().filter(|(k, _)| {
                        matches!(k.as_str(), "NAME" | "DESIGNATOR" | "DESC" | "DESCRIPTION")
                    }) {
                        let key = if k == "DESC" { "DESCRIPTION" } else { &k };
                        fields.insert(key.into(), v);
                    }
                }
                if let Some(width) = symbol_widths.get(&pin)
                    && let Some(value) = wide_fields(width)?.remove("SYMBOL_LINEWIDTH")
                {
                    fields.insert("SYMBOL_LINEWIDTH".into(), value);
                }
                pin += 1;
                fields
            } else {
                return Err(format!("Unsupported SchLib binary flag {flag}"));
            };
            let record_type = fields.n("RECORD") as u32;
            if records.len() == 1 && record_type != 1 {
                return Err("Symbol does not begin with component record".into());
            }
            if records.len() > 1 {
                let owner = fields.n("OWNERINDEX");
                if owner < 0 || owner as usize >= records.len() - 1 {
                    return Err("Invalid library record owner".into());
                }
                fields.insert("OWNERINDEX".into(), (owner + 1).to_string());
            } else {
                fields.insert("OWNERINDEX".into(), "0".into());
                fields.insert("SOURCELIBRARYNAME".into(), name.into());
            }
            records.push(SourceRecord {
                index: records.len() as u32,
                stream: "FileHeader".into(),
                record_type,
                fields,
            });
        }
        let root = records.get(1).ok_or("Empty symbol")?;
        if [&fractions, &text_data, &wide_text, &symbol_widths]
            .iter()
            .any(|m| m.keys().any(|k| *k >= pin))
        {
            return Err("Auxiliary data refers to a missing pin".into());
        }
        let parts = root
            .fields
            .n("PARTCOUNT")
            .checked_sub(1)
            .ok_or("Invalid library part count")?;
        let modes = root.fields.get("DISPLAYMODECOUNT").map_or(Ok(1), |v| {
            v.parse::<i64>()
                .map_err(|_| "Invalid library display mode count")
        })?;
        if !(1..=1024).contains(&parts) || !(1..=32).contains(&modes) || parts * modes > 1024 {
            return Err("Library symbol variant count exceeds limit".into());
        }
        expanded_records =
            expanded_records.saturating_add(records.len().saturating_mul((parts * modes) as usize));
        expanded_resources = expanded_resources.saturating_add(
            images
                .values()
                .map(String::len)
                .sum::<usize>()
                .saturating_mul((parts * modes) as usize),
        );
        if expanded_records > MAX_RECORDS || expanded_resources > MAX_FILE {
            return Err("Expanded library variant records exceed limit".into());
        }
        let mut symbol = Symbol {
            name: root.fields.text("LIBREFERENCE"),
            description: root.fields.text("COMPONENTDESCRIPTION"),
            parts: parts as u32,
            display_modes: modes as u32,
            variants: vec![],
        };
        if symbol.name.is_empty() {
            return Err("Library symbol has no name".into());
        }
        let id = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            &[name.as_bytes(), path.to_string_lossy().as_bytes()].concat(),
        );
        for part in 1..=parts as u32 {
            for mode in 0..modes as u32 {
                records[1]
                    .fields
                    .insert("CURRENTPARTID".into(), part.to_string());
                records[1]
                    .fields
                    .insert("DISPLAYMODE".into(), mode.to_string());
                let mut doc = build_document(
                    &symbol.name,
                    id,
                    records.clone(),
                    vec![],
                    &[],
                    images.clone(),
                )?;
                let scene = doc.schematic.as_ref().unwrap();
                library.warnings.extend(scene.warnings.iter().cloned());
                let fragment = schematic_edit::extract(scene, scene.components[0].id)?;
                objects += fragment.primitives.len();
                if objects > MAX_RECORDS {
                    return Err("Library geometry limit exceeded".into());
                }
                symbol.variants.push(SymbolVariant {
                    part,
                    display_mode: mode,
                    fragment,
                });
                if let Some(out) = editable.as_deref_mut() {
                    let s = doc.schematic.as_mut().unwrap();
                    s.sheet.border = false;
                    s.sheet.title_block = false;
                    out.push(doc);
                }
            }
        }
        library.symbols.push(symbol);
    }
    if library.symbols.len() as i64 != header.n("COMPCOUNT") {
        library
            .warnings
            .push("SchLib header component count differs from Data streams".into());
    }
    library.symbols.sort_by_key(|s| s.name.to_lowercase());
    library.warnings.sort();
    library.warnings.dedup();
    Ok(library)
}

fn wide_fields(data: &[u8]) -> Result<Fields, String> {
    let length = u32::from_le_bytes(
        data.get(..4)
            .ok_or("Truncated wide pin properties")?
            .try_into()
            .unwrap(),
    ) as usize;
    if !length.is_multiple_of(2) || length + 4 != data.len() {
        return Err("Invalid wide pin property length".into());
    }
    let units: Vec<_> = data[4..]
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let text = String::from_utf16(&units).map_err(|_| "Invalid UTF-16 pin properties")?;
    decode_fields(text.as_bytes(), encoding_rs::UTF_8)
}

fn binary_pin(
    data: &[u8],
    encoding: &'static Encoding,
    fractions: Option<&Vec<u8>>,
    text_data: Option<&Vec<u8>>,
) -> Result<Fields, String> {
    let mut p = PinReader { data, offset: 0 };
    if p.u32()? != 2 {
        return Err("Unsupported binary symbol record (expected pin)".into());
    }
    p.byte()?;
    let part = p.i16()?;
    let mode = p.byte()?;
    let outer = p.byte()?;
    let inner = p.byte()?;
    p.byte()?;
    p.byte()?;
    let description = p.string(encoding)?;
    p.byte()?;
    let electrical = p.byte()?;
    let flags = p.byte()?;
    let length = p.i16()?;
    let x = p.i16()?;
    let y = p.i16()?;
    let color = p.u32()?;
    let name = p.string(encoding)?;
    let number = p.string(encoding)?;
    let mut f = Fields::new();
    for (key, value) in [
        ("RECORD", 2i64),
        ("OWNERPARTID", part as i64),
        ("OWNERPARTDISPLAYMODE", mode as i64),
        ("SYMBOL_OUTEREDGE", outer as i64),
        ("SYMBOL_INNEREDGE", inner as i64),
        ("ELECTRICAL", electrical as i64),
        ("PINCONGLOMERATE", flags as i64),
        ("PINLENGTH", length as i64),
        ("LOCATION.X", x as i64),
        ("LOCATION.Y", y as i64),
        ("COLOR", color as i64),
    ] {
        f.insert(key.into(), value.to_string());
    }
    f.insert("NAME".into(), name);
    f.insert("DESIGNATOR".into(), number);
    f.insert("DESCRIPTION".into(), description);
    if let Some(frac) = fractions {
        if frac.len() != 12 {
            return Err("Invalid PinFrac payload".into());
        }
        for (n, key) in ["LOCATION.X_FRAC", "LOCATION.Y_FRAC", "PINLENGTH_FRAC"]
            .iter()
            .enumerate()
        {
            f.insert(
                (*key).into(),
                i32::from_le_bytes(frac[n * 4..n * 4 + 4].try_into().unwrap()).to_string(),
            );
        }
    }
    if let Some(text) = text_data {
        if text.len() != 14 {
            return Err("Unsupported PinTextData payload".into());
        }
        for (o, key) in [(0, "NAME_CUSTOMFONTID"), (7, "DESIGNATOR_CUSTOMFONTID")] {
            if text[o] & 16 != 0 {
                f.insert(
                    key.into(),
                    u32::from_le_bytes(text[o + 1..o + 5].try_into().unwrap()).to_string(),
                );
            }
        }
    }
    Ok(f)
}
struct PinReader<'a> {
    data: &'a [u8],
    offset: usize,
}
impl PinReader<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.offset.checked_add(n).ok_or("Pin length overflow")?;
        let b = self
            .data
            .get(self.offset..end)
            .ok_or("Truncated binary pin")?;
        self.offset = end;
        Ok(b)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.bytes(1)?[0])
    }
    fn i16(&mut self) -> Result<i16, String> {
        Ok(i16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    fn string(&mut self, e: &'static Encoding) -> Result<String, String> {
        let n = self.byte()? as usize;
        Ok(e.decode(self.bytes(n)?).0.into_owned())
    }
}
