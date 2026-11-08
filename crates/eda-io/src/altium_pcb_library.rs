//! PCB6 footprint libraries, including PCB libraries embedded in IntLib files.
use super::*;
use eda_core::design_sync::Footprint;

#[derive(Default)]
pub struct Library {
    pub footprints: Vec<Footprint>,
    pub warnings: Vec<String>,
}

pub fn load(path: &Path) -> Result<Library> {
    import(&crate::altium::read_bounded(path)?, &path.to_string_lossy())
}

fn streams(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut file = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let paths: Vec<_> = file
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_owned(), e.len()))
        .collect();
    let mut result = BTreeMap::new();
    let mut total = 0;
    for (path, len) in paths {
        if len > LIMIT as u64 {
            return Err("Library stream exceeds import limit".into());
        }
        let mut data = vec![];
        file.open_stream(&path)
            .map_err(|e| e.to_string())?
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        total += data.len();
        if total > LIMIT {
            return Err("Expanded PCB library exceeds import limit".into());
        }
        // Normalize the host Path representation to CFB's portable '/' keys.
        let name = path.to_string_lossy().replace('\\', "/");
        if result.insert(name, data).is_some() {
            return Err("Ambiguous PCB library stream paths".into());
        }
    }
    Ok(result)
}

pub fn import(bytes: &[u8], name: &str) -> Result<Library> {
    if bytes.len() > LIMIT {
        return Err("PCB library exceeds import limit".into());
    }
    let source = streams(bytes)?;
    if !source.contains_key("/Library/Data") {
        let mut result = Library::default();
        let mut total = 0;
        for (path, bytes) in source
            .iter()
            .filter(|(p, _)| p.to_ascii_lowercase().ends_with(".pcblib"))
        {
            let decoded = match bytes.first() {
                Some(2) => {
                    let mut decoder = flate2::read::ZlibDecoder::new(&bytes[1..]);
                    let mut decoded = vec![];
                    decoder
                        .by_ref()
                        .take(LIMIT as u64 + 1)
                        .read_to_end(&mut decoded)
                        .map_err(|e| e.to_string())?;
                    if decoder.total_in() != (bytes.len() - 1) as u64 {
                        return Err("Trailing or truncated IntLib PCB compression".into());
                    }
                    decoded
                }
                Some(0xd0) => bytes.clone(),
                _ => return Err(format!("Unsupported IntLib PCB compression in {path}")),
            };
            total += decoded.len();
            if total > LIMIT {
                return Err("Expanded IntLib PCB libraries exceed import limit".into());
            }
            let library = pcblib(
                &streams(&decoded)?,
                name,
                Uuid::new_v5(&Uuid::NAMESPACE_OID, &decoded),
            )?;
            result.footprints.extend(library.footprints);
            result.warnings.extend(library.warnings);
        }
        if result.footprints.is_empty() && result.warnings.is_empty() {
            return Err("No PCB library found in this container".into());
        }
        return Ok(result);
    }
    pcblib(&source, name, Uuid::new_v5(&Uuid::NAMESPACE_OID, bytes))
}

fn pcblib(source: &BTreeMap<String, Vec<u8>>, name: &str, ns: Uuid) -> Result<Library> {
    let header = source
        .get("/FileHeader")
        .ok_or("Missing PCB library FileHeader")?;
    if !header
        .windows(b"PCB 6.0 Binary Library File".len())
        .any(|b| b == b"PCB 6.0 Binary Library File")
    {
        return Err("Unsupported PCB library version".into());
    }
    let properties =
        fields(Bytes::new(source.get("/Library/Data").ok_or("Missing Library/Data")?).block()?)?;
    let model_streams: BTreeMap<_, _> = source
        .iter()
        .filter_map(|(p, b)| {
            p.strip_prefix("/Library/Models/")
                .map(|suffix| (format!("/Models/{suffix}"), b.clone()))
        })
        .collect();
    let mut model_board = Board::default();
    step_io::read_models(&model_streams, &mut model_board);
    let mut result = Library {
        footprints: vec![],
        warnings: model_board.warnings.clone(),
    };
    let paths: Vec<_> = source
        .keys()
        .filter(|p| {
            p.ends_with("/Data")
                && !p.starts_with("/Library/")
                && source.contains_key(&format!("{}/Parameters", p.trim_end_matches("/Data")))
        })
        .collect();
    if paths.is_empty() || paths.len() > 10_000 {
        return Err("Invalid PCB library footprint count".into());
    }
    for path in paths {
        let prefix = path.trim_end_matches("/Data");
        let loaded = footprint(source, prefix, &properties, &model_board, name, ns);
        match loaded {
            Ok(f) => {
                for warning in f
                    .warnings()
                    .iter()
                    .filter(|w| !w.starts_with("No valid Board6 outline"))
                {
                    result.warnings.push(format!("{}: {warning}", f.name()));
                }
                result.footprints.push(f);
            }
            Err(e) => result.warnings.push(format!(
                "{}: {e}; footprint unavailable",
                prefix.trim_start_matches('/')
            )),
        }
    }
    Ok(result)
}

fn footprint(
    source: &BTreeMap<String, Vec<u8>>,
    prefix: &str,
    properties: &Fields,
    models: &Board,
    library: &str,
    ns: Uuid,
) -> Result<Footprint> {
    let parameters = property_records(&source[&format!("{prefix}/Parameters")], 0)?;
    let mut props = parameters
        .first()
        .filter(|_| parameters.len() == 1)
        .ok_or("Invalid footprint Parameters")?
        .clone();
    let mut reader = Bytes::new(&source[&format!("{prefix}/Data")]);
    let record_name = short_string(reader.block()?)?;
    let name = props.text("PATTERN");
    if name.is_empty() || record_name != name {
        return Err("Footprint name/Parameters mismatch".into());
    }
    props.insert("X".into(), "0mil".into());
    props.insert("Y".into(), "0mil".into());
    props.insert("LAYER".into(), "TOP".into());
    props.insert("SOURCEFOOTPRINTLIBRARY".into(), library.into());
    let c = component(props, ns, 0)?;
    let mut board = Board {
        properties: properties.clone(),
        components: vec![c],
        models: models.models.clone(),
        ..Board::default()
    };
    board.components[0].parameters = Some(Fields::new());
    // In a definition the editable Parameters table already represents its
    // footprint values. The placed-board namespace is assigned on placement.
    board.components[0].footprint_parameters = None;
    if let Some(data) = source.get(&format!("{prefix}/PrimitiveParameters")) {
        let groups = u32_at(data, 0)?;
        let records = property_records(&data[4..], 0)?;
        let mut n = 0;
        let mut actual = 0;
        while n < records.len() {
            let h = &records[n];
            let count = h
                .text("COUNT")
                .parse::<usize>()
                .map_err(|_| "Invalid footprint parameter count")?;
            let end = n
                .checked_add(1)
                .and_then(|n| n.checked_add(count))
                .filter(|end| *end <= records.len())
                .ok_or("Truncated footprint parameters")?;
            if h.text("PRIMITIVEID") == "NoObject#0"
                && h.text("APPURTENANCE") == "Footprint"
                && h.text("VARIANTGUID") == "Footprint"
            {
                let params = board.components[0].parameters.as_mut().unwrap();
                for r in &records[n + 1..end] {
                    let name = r.text("NAME");
                    if name.is_empty() || params.keys().any(|k| k.eq_ignore_ascii_case(&name)) {
                        return Err("Empty or duplicate footprint parameter".into());
                    }
                    params.insert(name, r.text("VALUE"));
                }
            } else if count > 0 {
                return Err("Unrepresented footprint parameter namespace; native library remains available read-only".into());
            }
            actual += 1;
            n = end;
        }
        if actual != groups {
            return Err("Footprint parameter group count mismatch".into());
        }
    }
    let mut wide = BTreeMap::new();
    if let Some(data) = source.get(&format!("{prefix}/WideStrings")) {
        // Native PcbLib uses a property block here, unlike WideStrings6/Data.
        for f in property_records(data, 0)? {
            for (key, value) in f {
                let Some(index) = key
                    .strip_prefix("ENCODEDTEXT")
                    .and_then(|n| n.parse::<u32>().ok())
                else {
                    return Err("Unsupported footprint WideStrings property".into());
                };
                let words = value
                    .split(',')
                    .filter(|v| !v.trim().is_empty())
                    .map(|v| {
                        v.trim()
                            .parse::<u16>()
                            .map_err(|_| "Invalid encoded footprint text")
                    })
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let text = String::from_utf16(words.strip_suffix(&[0]).unwrap_or(&words))
                    .map_err(|e| e.to_string())?;
                wide.insert(index, text);
            }
        }
    }
    while reader.remaining() > 0 {
        let tag = reader.u8()?;
        let family = match tag {
            1 => "Arcs6",
            2 => "Pads6",
            3 => "Vias6",
            4 => "Tracks6",
            5 => "Texts6",
            6 => "Fills6",
            11 => "Regions6",
            12 => "ComponentBodies6",
            _ => return Err(format!("Unsupported primitive tag {tag}")),
        };
        let mut item = if tag == 2 {
            let mut blocks = vec![];
            for _ in 0..6 {
                blocks.push(reader.block()?);
            }
            pad(&blocks, &mut board)?
        } else {
            let payload = reader.block()?;
            if tag == 5 {
                text_item(payload, reader.block()?, &wide, &board)?
            } else if matches!(tag, 11 | 12) {
                let mut i = region_item(payload, false, &mut board)?;
                if tag == 12 {
                    i.properties
                        .insert("PCBCRAFT_BODY_PROJECTION".into(), "TRUE".into());
                }
                i
            } else {
                primitive(family, payload)?
            }
        };
        item.id = stable(ns, &name, board.items.len());
        item.component = Some(0);
        item.net = None;
        item.polygon = None;
        item.source = None;
        item.locked = false;
        board.items.push(item);
        if board.items.len() > MAX_ITEMS {
            return Err("Footprint primitive limit exceeded".into());
        }
    }
    let header = source
        .get(&format!("{prefix}/Header"))
        .ok_or("Missing footprint Header")?;
    if u32_at(header, 0)? as usize != board.items.len() {
        return Err("Footprint primitive count mismatch".into());
    }
    finish(&mut board)?;
    // A library definition intentionally has no Board6 outline. Do not pass
    // that board-only diagnostic to footprint consumers as lost geometry.
    board
        .warnings
        .retain(|w| !w.starts_with("No valid Board6 outline"));
    board.validate()?;
    Footprint::from_library_board(board)
}

#[cfg(test)]
#[path = "altium_pcb_library_tests.rs"]
mod tests;
