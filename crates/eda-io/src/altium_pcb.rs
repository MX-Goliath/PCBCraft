//! Bounded Altium PCB6 CFB and ASCII importer.
//! Binary layout facts were cross-checked with the public altiumts format reader.
//! See docs/pcb-update/IMPLEMENTATION.md for the deliberately explicit compatibility limits.
use base64::{Engine, engine::general_purpose::STANDARD};
use eda_core::{Document, DocumentKind, pcb::*};
use eda_geom::Point;
use encoding_rs::WINDOWS_1252;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read, Write},
    path::Path,
    sync::Arc,
};
use uuid::Uuid;
#[cfg(test)]
#[path = "altium_pcb_display_tests.rs"]
mod display_tests;
#[cfg(test)]
#[path = "altium_pcb_stack_tests.rs"]
mod stack_tests;
#[path = "altium_pcb_write.rs"]
pub mod writer;

#[path = "altium_pcb_stack.rs"]
mod stack_io;

const LIMIT: usize = 128 * 1024 * 1024;
const MAX_BLOCK: usize = 16 * 1024 * 1024;
const MAGIC: &[u8] = &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
type Result<T> = std::result::Result<T, String>;
pub fn load_pcb(path: &Path) -> Result<Document> {
    let bytes = crate::altium::read_bounded(path)?;
    import_pcb(
        &bytes,
        &path.file_name().unwrap_or_default().to_string_lossy(),
    )
}
pub fn import_pcb(bytes: &[u8], name: &str) -> Result<Document> {
    if bytes.len() > LIMIT {
        return Err("PCB file exceeds 128 MiB import limit".into());
    }
    let namespace = Uuid::new_v5(&Uuid::NAMESPACE_OID, bytes);
    let mut board = if bytes.starts_with(MAGIC) {
        binary(bytes, namespace)?
    } else {
        ascii(bytes, namespace)?
    };
    finish(&mut board)?;
    upgrade_polygons(&mut board, namespace)?;
    board.source = Some(Arc::new(SourceArchive {
        filename: name.to_owned(),
        format: if bytes.starts_with(MAGIC) {
            "Altium PCB6 CFB"
        } else {
            "Altium PCB ASCII"
        }
        .into(),
        bytes_base64: STANDARD.encode(bytes),
    }));
    warn(
        &mut board,
        "Compatibility is limited to the documented PCB6 fields and supported modern layer identities; stack regions, native design rules and full Altium interactions are not implemented",
    );
    board.validate()?;
    let bounds = board.bounds();
    let mut doc = Document::new(DocumentKind::Board, name);
    doc.id = namespace;
    doc.size = Point {
        x: (bounds.max.x - bounds.min.x).max(1_000_000),
        y: (bounds.max.y - bounds.min.y).max(1_000_000),
    };
    doc.pcb = Some(board);
    doc.validate()?;
    Ok(doc)
}
/// Extract the exact ORIGINAL file, not a serialization of the edited board.
/// Callers must expose this distinction and must not use this as ordinary Save.
pub fn restore_original(path: &Path, doc: &Document) -> Result<()> {
    let source = doc
        .pcb
        .as_ref()
        .and_then(|b| b.source.as_ref())
        .ok_or("No archived source file")?;
    if source.bytes_base64.len() > LIMIT * 4 / 3 + 8 {
        return Err("Archived source exceeds limit".into());
    }
    let bytes = STANDARD
        .decode(&source.bytes_base64)
        .map_err(|e| e.to_string())?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist_noclobber(path).map_err(|e| e.to_string())?;
    Ok(())
}
fn stable(ns: Uuid, family: &str, index: usize) -> Uuid {
    Uuid::new_v5(&ns, format!("{family}:{index}").as_bytes())
}
fn warn(b: &mut Board, s: impl Into<String>) {
    let s = s.into();
    if !b.warnings.contains(&s) && b.warnings.len() < 200 {
        b.warnings.push(s);
    }
}
struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Bytes<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).ok_or("Record length overflow")?;
        let v = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| format!("Truncated record at byte {} (need {n} bytes)", self.pos))?;
        self.pos = end;
        Ok(v)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn block(&mut self) -> Result<&'a [u8]> {
        let n = (self.u32()? & 0x00ff_ffff) as usize;
        if n > MAX_BLOCK {
            return Err("PCB subrecord exceeds 16 MiB limit".into());
        }
        self.take(n)
    }
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }
}
fn at<const N: usize>(b: &[u8], p: usize) -> Result<[u8; N]> {
    b.get(p..p + N)
        .ok_or_else(|| format!("Truncated PCB field at offset {p}"))?
        .try_into()
        .map_err(|_| "Field width mismatch".into())
}
fn u16_at(b: &[u8], p: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(at(b, p)?))
}
fn u32_at(b: &[u8], p: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(at(b, p)?))
}
fn f64_at(b: &[u8], p: usize) -> Result<f64> {
    let n = f64::from_le_bytes(at(b, p)?);
    if n.is_finite() {
        Ok(n)
    } else {
        Err(format!("Non-finite PCB numeric field at {p}"))
    }
}
fn coord(b: &[u8], p: usize) -> Result<i64> {
    let n = i32::from_le_bytes(at(b, p)?) as i64;
    let scaled = n * 254;
    Ok((scaled + if scaled < 0 { -50 } else { 50 }) / 100)
}
fn float_coord(b: &[u8], p: usize) -> Result<i64> {
    let n = f64_at(b, p)? * 2.54;
    if n.abs() > 100_000_000_000.0 {
        return Err("PCB coordinate exceeds model limits".into());
    }
    Ok(n.round() as i64)
}
fn point(b: &[u8], p: usize) -> Result<Point> {
    Ok(Point {
        x: coord(b, p)?,
        y: coord(b, p + 4)?,
    })
}
fn index(n: u16) -> Option<u32> {
    (n != 0xffff).then_some(n as u32)
}
fn short_string(b: &[u8]) -> Result<String> {
    if b.is_empty() {
        return Ok(String::new());
    }
    let n = b[0] as usize;
    let v = b.get(1..1 + n).ok_or("Truncated PCB short string")?;
    Ok(WINDOWS_1252.decode(v).0.into_owned())
}
fn utf16(b: &[u8]) -> Result<String> {
    if !b.len().is_multiple_of(2) {
        return Err("Truncated UTF-16 PCB text".into());
    }
    let words: Vec<_> = b
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|c| *c != 0)
        .collect();
    String::from_utf16(&words).map_err(|e| e.to_string())
}
fn fields(b: &[u8]) -> Result<Fields> {
    let b = b.strip_suffix(&[0]).unwrap_or(b);
    let mut f = Fields::new();
    for pair in b.split(|c| *c == b'|') {
        if let Some(eq) = pair.iter().position(|c| *c == b'=') {
            let k = std::str::from_utf8(&pair[..eq])
                .map_err(|_| "Invalid PCB property key")?
                .trim()
                .to_ascii_uppercase();
            let value = &pair[eq + 1..];
            let s = if k.starts_with("%UTF8%") {
                std::str::from_utf8(value)
                    .map_err(|_| "Invalid explicit UTF-8 PCB property")?
                    .to_owned()
            } else {
                std::str::from_utf8(value)
                    .map(str::to_owned)
                    .unwrap_or_else(|_| WINDOWS_1252.decode(value).0.into_owned())
            };
            if f.len() >= 100_000 {
                return Err("PCB property limit exceeded".into());
            }
            f.insert(k, s);
        }
    }
    let utf8: Vec<_> = f
        .iter()
        .filter_map(|(k, v)| k.strip_prefix("%UTF8%").map(|k| (k.to_owned(), v.clone())))
        .collect();
    let unicode: Vec<_> = f
        .iter()
        .filter_map(|(k, v)| {
            k.strip_prefix("UNICODE__")
                .map(|key| (k.clone(), key.to_owned(), v.clone()))
        })
        .collect();
    for (shadow, key, value) in unicode {
        let words: Vec<u16> = value
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                s.trim()
                    .parse::<u16>()
                    .map_err(|_| "Invalid PCB Unicode property code unit".to_owned())
            })
            .collect::<Result<_>>()?;
        let end = words.iter().position(|w| *w == 0).unwrap_or(words.len());
        let decoded =
            String::from_utf16(&words[..end]).map_err(|_| "Invalid UTF-16 PCB property")?;
        f.insert(key, decoded);
        f.remove(&shadow);
    }
    for (k, v) in utf8 {
        f.remove(&format!("%UTF8%{k}"));
        f.insert(k, v);
    }
    // Encoding shadows remain in the archived source, not in the semantic map.
    if f.get("UNICODE").is_some_and(|v| v == "EXISTS") {
        f.remove("UNICODE");
    }
    Ok(f)
}
trait Values {
    fn text(&self, k: &str) -> String;
    fn num(&self, k: &str, default: f64) -> Result<f64>;
    fn dim(&self, k: &str, default: i64) -> Result<i64>;
    fn yes(&self, k: &str) -> bool;
    fn pos(&self, x: &str, y: &str) -> Result<Point>;
}
impl Values for Fields {
    fn text(&self, k: &str) -> String {
        self.get(k).cloned().unwrap_or_default()
    }
    fn num(&self, k: &str, default: f64) -> Result<f64> {
        match self.get(k) {
            None => Ok(default),
            Some(s) => s
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("Invalid numeric property {k}={s}")),
        }
    }
    fn dim(&self, k: &str, default: i64) -> Result<i64> {
        self.get(k).map_or(Ok(default), |s| dimension(s))
    }
    fn yes(&self, k: &str) -> bool {
        self.get(k)
            .is_some_and(|s| matches!(s.to_ascii_uppercase().as_str(), "TRUE" | "T" | "1"))
    }
    fn pos(&self, x: &str, y: &str) -> Result<Point> {
        Ok(Point {
            x: self.dim(x, 0)?,
            y: self.dim(y, 0)?,
        })
    }
}
pub fn dimension(value: &str) -> Result<i64> {
    let v = value.trim().to_ascii_lowercase();
    let (n, scale) = if let Some(s) = v.strip_suffix("mil") {
        (s, 25_400.0)
    } else if let Some(s) = v.strip_suffix("mm") {
        (s, 1_000_000.0)
    } else if let Some(s) = v.strip_suffix("inch") {
        (s, 25_400_000.0)
    } else {
        (v.as_str(), 25_400.0)
    };
    let n = n
        .trim()
        .parse::<f64>()
        .map_err(|_| format!("Invalid PCB dimension: {value}"))?
        * scale;
    if !n.is_finite() || n.abs() > 100_000_000_000.0 {
        return Err(format!("Out-of-range PCB dimension: {value}"));
    }
    Ok(n.round() as i64)
}
pub fn layer_id(name: &str) -> Result<u16> {
    let s = name.to_ascii_uppercase().replace(' ', "");
    let id = match s.as_str() {
        "TOP" | "TOPLAYER" => TOP,
        "BOTTOM" | "BOTTOMLAYER" => BOTTOM,
        "TOPOVERLAY" => 33,
        "BOTTOMOVERLAY" => 34,
        "TOPPASTE" => 35,
        "BOTTOMPASTE" => 36,
        "TOPSOLDER" => 37,
        "BOTTOMSOLDER" => 38,
        "DRILLGUIDE" => 55,
        "KEEPOUT" | "KEEPOUTLAYER" => KEEP_OUT,
        "DRILLDRAWING" => 73,
        "MULTILAYER" | "MULTI-LAYER" => MULTI_LAYER,
        "CONNECT" | "CONNECTLAYER" => 75,
        "BACKGROUND" | "BACKGROUNDLAYER" => 76,
        "DRCERROR" | "DRCERRORLAYER" => 77,
        "HIGHLIGHT" | "HIGHLIGHTLAYER" => 78,
        "GRIDCOLOR1" => 79,
        "GRIDCOLOR10" => 80,
        "PADHOLE" | "PADHOLELAYER" => 81,
        "VIAHOLE" | "VIAHOLELAYER" => 82,
        _ => {
            let invalid = || format!("Unrecognized PCB layer {name}");
            if let Some(n) = s.strip_prefix("MECHANICAL") {
                let n = n.parse::<u16>().map_err(|_| invalid())?;
                match n {
                    1..=1024 => mechanical_layer(n).unwrap(),
                    _ => return Err(invalid()),
                }
            } else if let Some(n) = s
                .strip_prefix("MID-LAYER")
                .or_else(|| s.strip_prefix("MIDLAYER"))
                .or_else(|| s.strip_prefix("MID"))
            {
                let n = n.parse::<u16>().map_err(|_| invalid())?;
                if !(1..=30).contains(&n) {
                    return Err(invalid());
                }
                1 + n
            } else if let Some(n) = s.strip_prefix("INTERNALPLANE") {
                let n = n.parse::<u16>().map_err(|_| invalid())?;
                if !(1..=16).contains(&n) {
                    return Err(invalid());
                }
                38 + n
            } else {
                s.strip_prefix("LAYER")
                    .unwrap_or(&s)
                    .parse::<u16>()
                    .map_err(|_| invalid())?
            }
        }
    };
    Ok(id)
}
fn unsigned_property(f: &Fields, key: &str, default: u32) -> Result<u32> {
    let v = f.num(key, default as f64)?;
    if v < 0.0 || v.fract() != 0.0 || v > u32::MAX as f64 {
        return Err(format!("Invalid unsigned PCB index {key}={v}"));
    }
    Ok(v as u32)
}
fn contour(f: &Fields) -> Result<Vec<ContourVertex>> {
    let mut out = Vec::new();
    for i in 0..MAX_ITEMS {
        if !f.contains_key(&format!("VX{i}")) {
            break;
        }
        let position = f.pos(&format!("VX{i}"), &format!("VY{i}"))?;
        let arc = if f.num(&format!("KIND{i}"), 0.0)? == 1.0 {
            Some(ContourArc {
                centre: f.pos(&format!("CX{i}"), &format!("CY{i}"))?,
                radius: f.dim(&format!("R{i}"), 0)?,
                start_angle: f.num(&format!("SA{i}"), 0.0)?,
                end_angle: f.num(&format!("EA{i}"), 360.0)?,
            })
        } else {
            None
        };
        out.push(ContourVertex { position, arc });
    }
    if out.len() > 1 && out.first().map(|v| v.position) == out.last().map(|v| v.position) {
        out.pop();
    }
    Ok(out)
}
fn component(f: Fields, ns: Uuid, n: usize) -> Result<Component> {
    let layer = layer_id(f.get("LAYER").map_or("TOP", String::as_str))?;
    Ok(Component {
        id: stable(ns, "Components6", n),
        index: n as u32,
        designator: f.text("SOURCEDESIGNATOR"),
        comment: f.text("SOURCECOMMENT"),
        footprint: f.text("PATTERN"),
        position: f.pos("X", "Y")?,
        rotation: f.num("ROTATION", 0.0)?,
        layer,
        locked: f.yes("LOCKED"),
        properties: f,
    })
}
fn property_records(bytes: &[u8], prefix: usize) -> Result<Vec<Fields>> {
    let mut reader = Bytes::new(bytes);
    let mut records = vec![];
    while reader.remaining() > 0 {
        reader.take(prefix)?;
        let block = reader.block()?;
        if block.is_empty() {
            return Err("Empty PCB property block".into());
        }
        records.push(fields(block)?);
        if records.len() > MAX_ITEMS {
            return Err("PCB record limit exceeded".into());
        }
    }
    Ok(records)
}
fn read_count(streams: &BTreeMap<String, Vec<u8>>, family: &str) -> Result<Option<usize>> {
    streams
        .get(&format!("/{family}/Header"))
        .map(|b| u32_at(b, 0).map(|n| n as usize))
        .transpose()
}
fn check_count(streams: &BTreeMap<String, Vec<u8>>, family: &str, actual: usize) -> Result<()> {
    if let Some(n) = read_count(streams, family)?
        && n != actual
    {
        return Err(format!(
            "{family}: header declares {n} records, decoded {actual}"
        ));
    }
    Ok(())
}
fn binary(bytes: &[u8], ns: Uuid) -> Result<Board> {
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|e| format!("Invalid PCB compound file: {e}"))?;
    if !cfb.is_stream("/Board6/Data") {
        return Err("CFB file has no Board6/Data; not a supported PCB6 document".into());
    }
    let paths: Vec<_> = cfb
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| (e.path().to_string_lossy().replace('\\', "/"), e.len()))
        .collect();
    let known = [
        "Classes6",
        "Board6",
        "Nets6",
        "Components6",
        "Rules6",
        "Polygons6",
        "Tracks6",
        "Arcs6",
        "Vias6",
        "Pads6",
        "Fills6",
        "Texts6",
        "Regions6",
        "ShapeBasedRegions6",
        "ComponentBodies6",
        "ShapeBasedComponentBodies6",
        "WideStrings6",
    ];
    let mut streams = BTreeMap::new();
    let mut expanded = 0usize;
    let mut board = Board {
        polygon_model_version: 0,
        ..Board::default()
    };
    for (path, len) in paths {
        let family = path.trim_start_matches('/').split('/').next().unwrap_or("");
        if known.contains(&family) {
            if len > LIMIT as u64 {
                return Err(format!("PCB stream {path} exceeds limit"));
            }
            let mut data = vec![];
            cfb.open_stream(&path)
                .map_err(|e| e.to_string())?
                .take(LIMIT as u64 + 1)
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            expanded = expanded
                .checked_add(data.len())
                .ok_or("Expanded stream overflow")?;
            if data.len() > LIMIT || expanded > 2 * LIMIT {
                return Err("Expanded PCB streams exceed limit".into());
            }
            streams.insert(path, data);
        } else if path.ends_with("/Data") && len > 0 {
            warn(
                &mut board,
                format!(
                    "{family}: retained in the original archive, not interpreted by this importer"
                ),
            );
        }
    }
    for family in [
        "Board6",
        "Nets6",
        "Components6",
        "Rules6",
        "Polygons6",
        "Classes6",
    ] {
        let Some(data) = streams.get(&format!("/{family}/Data")) else {
            continue;
        };
        let records = property_records(data, if family == "Rules6" { 2 } else { 0 })?;
        check_count(&streams, family, records.len())?;
        for (n, f) in records.into_iter().enumerate() {
            match family {
                "Board6" => {
                    if n == 0 {
                        board.outline = contour(&f)?;
                        board.properties = f;
                    } else {
                        warn(
                            &mut board,
                            "Additional Board6 records retained in source archive",
                        );
                    }
                }
                "Nets6" => board.nets.push(Net {
                    index: n as u32,
                    name: f.text("NAME"),
                    properties: f,
                }),
                "Components6" => board.components.push(component(f, ns, n)?),
                "Rules6" => board.rules.push(f),
                "Classes6" => board.classes.push(f),
                "Polygons6" => board.polygons.push(f),
                _ => {}
            }
        }
    }
    let wide = streams
        .get("/WideStrings6/Data")
        .map(|b| wide_strings(b))
        .transpose()?
        .unwrap_or_default();
    for (family, tag) in [
        ("Tracks6", 4),
        ("Arcs6", 1),
        ("Vias6", 3),
        ("Pads6", 2),
        ("Fills6", 6),
        ("Texts6", 5),
        ("Regions6", 11),
        ("ShapeBasedRegions6", 11),
        ("ComponentBodies6", 12),
        ("ShapeBasedComponentBodies6", 12),
    ] {
        let Some(data) = streams.get(&format!("/{family}/Data")) else {
            continue;
        };
        let mut reader = Bytes::new(data);
        let mut n = 0;
        while reader.remaining() > 0 {
            if reader.u8()? != tag {
                return Err(format!("{family}: invalid primitive tag in record {n}"));
            }
            let mut item = if family == "Pads6" {
                let mut blocks = vec![];
                for _ in 0..6 {
                    blocks.push(reader.block()?);
                }
                pad(blocks.as_slice(), &mut board)?
            } else {
                let payload = reader.block()?;
                if family == "Texts6" {
                    let text = reader.block()?;
                    text_item(payload, text, &wide, &board)?
                } else if matches!(tag, 11 | 12) {
                    let mut item =
                        region_item(payload, family.starts_with("ShapeBased"), &mut board)?;
                    if tag == 12 {
                        item.properties
                            .insert("PCBCRAFT_BODY_PROJECTION".into(), "TRUE".into());
                        item.net = None;
                        item.polygon = None;
                    }
                    item
                } else {
                    primitive(family, payload)?
                }
            };
            item.id = stable(ns, family, n);
            item.source = Some(SourceRecord {
                stream: family.into(),
                index: n as u32,
            });
            board.items.push(item);
            n += 1;
            if board.items.len() > MAX_ITEMS {
                return Err("PCB primitive limit exceeded".into());
            }
        }
        check_count(&streams, family, n)?;
    }
    let modern: Vec<_> = board
        .items
        .iter()
        .filter(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| s.stream.starts_with("ShapeBased"))
        })
        .cloned()
        .collect();
    for item in &mut board.items {
        if item
            .source
            .as_ref()
            .is_some_and(|s| matches!(s.stream.as_str(), "Regions6" | "ComponentBodies6"))
            && modern.iter().any(|m| {
                m.layer == item.layer
                    && m.source
                        .as_ref()
                        .zip(item.source.as_ref())
                        .is_some_and(|(a, b)| {
                            a.index == b.index && a.stream == format!("ShapeBased{}", b.stream)
                        })
                    && m.properties == item.properties
                    && m.properties.get("PCBCRAFT_BODY_PROJECTION")
                        == item.properties.get("PCBCRAFT_BODY_PROJECTION")
                    && m.net == item.net
                    && m.component == item.component
                    && m.polygon == item.polygon
                    && region_equivalent(
                        &m.shape,
                        &item.shape,
                        item.properties
                            .get("ARCRESOLUTION")
                            .and_then(|v| dimension(v).ok())
                            .unwrap_or(2)
                            .clamp(2, 12_700),
                    )
            })
        {
            item.properties
                .insert("PCBCRAFT_LEGACY_REGION_DUPLICATE".into(), "TRUE".into());
        }
    }
    if !board.rules.is_empty() {
        warn(
            &mut board,
            "Design rules imported; PCB Rules/DRC supports a subset of constraints and queries. See unchecked conditions in the DRC report",
        );
    }
    if !board.polygons.is_empty() {
        warn(
            &mut board,
            "Imported pour results are retained until edited or explicitly repoured; PCBCraft does not evaluate the full Altium rule engine",
        );
    }
    Ok(board)
}
fn region_equivalent(a: &Shape, b: &Shape, tolerance: i64) -> bool {
    let (
        Shape::Region {
            contour: a,
            holes: ah,
            cutout: ac,
        },
        Shape::Region {
            contour: b,
            holes: bh,
            cutout: bc,
        },
    ) = (a, b)
    else {
        return false;
    };
    let same = |a: &[Point], b: &[Point]| {
        (a.len() == b.len()
            && a.iter()
                .zip(b)
                .all(|(a, b)| (a.x - b.x).abs() <= 2 && (a.y - b.y).abs() <= 2))
            || (!a.is_empty()
                && !b.is_empty()
                && [(a, b), (b, a)].into_iter().all(|(a, b)| {
                    a.iter().all(|p| {
                        b.iter()
                            .zip(b.iter().cycle().skip(1))
                            .take(b.len())
                            .any(|(a, b)| segment_distance(*p, *a, *b) <= tolerance as f64)
                    })
                }))
    };
    ac == bc && same(a, b) && ah.len() == bh.len() && ah.iter().zip(bh).all(|(a, b)| same(a, b))
}
fn common(p: &[u8], shape: Shape) -> Result<Item> {
    if p.len() < 13 {
        return Err("Truncated primitive common header".into());
    }
    let mut i = Item::new(p[0] as u16, index(u16_at(p, 3)?), shape);
    i.component = index(u16_at(p, 7)?);
    i.polygon = index(u16_at(p, 5)?);
    i.locked = p[1] & 4 == 0;
    i.keepout = u16_at(p, 1)? & 0x200 != 0;
    Ok(i)
}
fn primitive(family: &str, p: &[u8]) -> Result<Item> {
    let shape = match family {
        "Tracks6" => Shape::Track {
            start: point(p, 13)?,
            end: point(p, 21)?,
            width: coord(p, 29)?,
        },
        "Arcs6" => Shape::Arc {
            centre: point(p, 13)?,
            radius: coord(p, 21)?,
            start_angle: f64_at(p, 25)?,
            end_angle: f64_at(p, 33)?,
            width: coord(p, 41)?,
        },
        "Fills6" => Shape::Fill {
            start: point(p, 13)?,
            end: point(p, 21)?,
            rotation: f64_at(p, 29)?,
        },
        "Vias6" => Shape::Via {
            position: point(p, 13)?,
            diameter: coord(p, 21)?,
            hole: coord(p, 25)?,
            from_layer: *p.get(29).ok_or("Truncated via start layer")? as u16,
            to_layer: *p.get(30).ok_or("Truncated via end layer")? as u16,
        },
        _ => return Err("Unsupported primitive family".into()),
    };
    let mut i = common(p, shape)?;
    let modern_offset = match family {
        "Tracks6" => Some(41),
        "Arcs6" => Some(56),
        "Fills6" => Some(42),
        _ => None,
    };
    if let Some(at) = modern_offset
        && p.len() >= at + 4
        && let Some(layer) = native_layer(u32_at(p, at)?)
    {
        i.layer = layer;
    }
    if family == "Vias6" {
        stack_io::read_via(&mut i, p)?;
        i.layer = MULTI_LAYER;
        i.component = None;
        i.polygon = None;
    }
    Ok(i)
}
fn pad(blocks: &[&[u8]], board: &mut Board) -> Result<Item> {
    let p = blocks[4];
    if p.len() < 110 {
        return Err("Pad geometry is shorter than 110 bytes".into());
    }
    let mut shape = match p[49] {
        1 => PadShape::Round,
        2 => PadShape::Rectangle,
        3 => PadShape::Octagonal,
        n => {
            warn(
                board,
                format!("Pad shape {n} displayed as rectangle; original preserved"),
            );
            PadShape::Rectangle
        }
    };
    if blocks[5].len() >= 596 && blocks[5][532] == 9 {
        shape = PadShape::RoundedRectangle;
    }
    let optional_size = |offset| -> Result<Option<Point>> {
        let s = point(p, offset)?;
        Ok((s.x > 0 && s.y > 0).then_some(s))
    };
    let mut item = common(
        p,
        Shape::Pad {
            position: point(p, 13)?,
            size: point(p, 21)?,
            rotation: f64_at(p, 52)?,
            shape,
            hole: coord(p, 45)?,
            plated: p[60] != 0,
            name: short_string(blocks[0])?,
            bottom_size: optional_size(37)?,
            middle_size: optional_size(29)?,
        },
    )?;
    if blocks[5].len() >= 596 {
        for layer in 1..=32 {
            item.properties.insert(
                format!("PCBCRAFT_PAD_RADIUS_{layer}"),
                blocks[5][563 + layer].to_string(),
            );
        }
    }
    stack_io::read_pad(&mut item, p, blocks[5], board)?;
    item.polygon = None;
    Ok(item)
}
fn text_item(p: &[u8], legacy: &[u8], wide: &BTreeMap<u32, String>, board: &Board) -> Result<Item> {
    if p.len() < 40 {
        return Err("Text geometry is shorter than 40 bytes".into());
    }
    let wide_text = if p.len() >= 119 {
        wide.get(&u32_at(p, 115)?).cloned()
    } else {
        None
    };
    let mut text = match wide_text {
        Some(value) => value,
        None => short_string(legacy)?,
    };
    let designator = p.get(41) == Some(&1) || text.eq_ignore_ascii_case(".Designator");
    let comment = p.get(40) == Some(&1) || text.eq_ignore_ascii_case(".Comment");
    if let Some(c) =
        index(u16_at(p, 7)?).and_then(|idx| board.components.iter().find(|c| c.index == idx))
    {
        if designator {
            text = c.designator.clone();
        }
        if comment && (text.is_empty() || text.eq_ignore_ascii_case(".Comment")) {
            text = c.comment.clone();
        }
    }
    let font = if p.len() >= 110 {
        utf16(&p[46..110])?
    } else {
        String::new()
    };
    let mut item = common(
        p,
        Shape::Text {
            position: point(p, 13)?,
            text,
            height: coord(p, 21)?.max(1),
            stroke: coord(p, 36)?.max(0),
            rotation: f64_at(p, 27)?,
            mirrored: p[35] != 0,
            font,
        },
    )?;
    item.net = None;
    if p.len() >= 230
        && let Some(layer) = native_layer(u32_at(p, 226)?)
    {
        item.layer = layer;
    }
    item.properties
        .insert("PCBCRAFT_TEXT_FONT_ID".into(), u16_at(p, 25)?.to_string());
    for (key, at) in [
        ("PCBCRAFT_TEXT_TTF", if p.len() > 160 { 160 } else { 43 }),
        ("PCBCRAFT_TEXT_BOLD", 44),
        ("PCBCRAFT_TEXT_ITALIC", 45),
        ("PCBCRAFT_TEXT_INVERTED", 110),
    ] {
        if let Some(value) = p.get(at) {
            item.properties.insert(key.into(), value.to_string());
        }
    }
    if p.len() >= 132 {
        let width = coord(p, 124)?;
        let height = coord(p, 128)?;
        if width > 0 && height > 0 {
            item.properties
                .insert("PCBCRAFT_TEXT_CACHE_WIDTH".into(), width.to_string());
            item.properties
                .insert("PCBCRAFT_TEXT_CACHE_HEIGHT".into(), height.to_string());
            if let Shape::Text { text, height, .. } = &item.shape {
                item.properties
                    .insert("PCBCRAFT_TEXT_CACHE_VALUE".into(), text.clone());
                item.properties.insert(
                    "PCBCRAFT_TEXT_CACHE_SOURCE_HEIGHT".into(),
                    height.to_string(),
                );
            }
        }
    }
    if designator {
        item.properties.insert("DESIGNATOR".into(), "TRUE".into());
    }
    if comment {
        item.properties.insert("COMMENT".into(), "TRUE".into());
    }
    item.polygon = None;
    Ok(item)
}
fn wide_strings(b: &[u8]) -> Result<BTreeMap<u32, String>> {
    let mut r = Bytes::new(b);
    let mut out = BTreeMap::new();
    while r.remaining() > 0 {
        let id = r.u32()?;
        let len = r.u32()? as usize;
        if len > MAX_BLOCK {
            return Err("PCB wide string exceeds limit".into());
        }
        // The legacy empty-string marker has no following payload.

        let value = if len <= 2 {
            String::new()
        } else {
            utf16(r.take(len)?)?
        };
        out.insert(id, value);
        if out.len() > MAX_ITEMS {
            return Err("Wide string count exceeds limit".into());
        }
    }
    Ok(out)
}
fn region_item(p: &[u8], extended: bool, board: &mut Board) -> Result<Item> {
    if p.len() < 26 {
        return Err("Truncated region header".into());
    }
    let n = (u32_at(p, 18)? & 0xffffff) as usize;
    let end = 22usize
        .checked_add(n)
        .ok_or("Region property length overflow")?;
    let f = fields(p.get(22..end).ok_or("Truncated region properties")?)?;
    let mut r = Bytes::new(p.get(end..).ok_or("Truncated region geometry")?);
    let count = (r.u32()? as usize)
        .checked_add(usize::from(extended))
        .ok_or("Region count overflow")?;
    if count > MAX_ITEMS || count > r.remaining() / (if extended { 37 } else { 16 }) {
        return Err("Region outline exceeds payload limits".into());
    }
    let mut contour = Vec::new();
    for _ in 0..count {
        if extended {
            let v = r.take(37)?;
            if v[0] != 0 {
                contour.extend(contour_arc_points(
                    point(v, 1)?,
                    &ContourArc {
                        centre: point(v, 9)?,
                        radius: coord(v, 17)?,
                        start_angle: f64_at(v, 21)?,
                        end_angle: f64_at(v, 29)?,
                    },
                    128,
                ));
            } else {
                contour.push(point(v, 1)?);
            }
        } else {
            let v = r.take(16)?;
            contour.push(Point {
                x: float_coord(v, 0)?,
                y: float_coord(v, 8)?,
            });
        }
        if contour.len() > MAX_ITEMS {
            return Err("Expanded region contour exceeds limit".into());
        }
    }
    let holes_count = u16_at(p, 14)? as usize;
    if holes_count > 10_000 {
        return Err("Region hole count exceeds limit".into());
    }
    let mut holes = Vec::new();
    for _ in 0..holes_count {
        let count = r.u32()? as usize;
        if count > MAX_ITEMS || count > r.remaining() / 16 {
            return Err("Region hole exceeds payload limits".into());
        }
        let mut hole = Vec::new();
        for _ in 0..count {
            let v = r.take(16)?;
            hole.push(Point {
                x: float_coord(v, 0)?,
                y: float_coord(v, 8)?,
            });
        }
        holes.push(hole);
    }
    contour.dedup();
    if contour.len() > 1 && contour.first() == contour.last() {
        contour.pop();
    }
    for hole in &mut holes {
        hole.dedup();
        if hole.len() > 1 && hole.first() == hole.last() {
            hole.pop();
        }
    }
    let cutout = f.yes("ISBOARDCUTOUT") || f.num("KIND", 0.0)? != 0.0;
    let _ = board;
    let mut i = common(
        p,
        Shape::Region {
            contour,
            holes,
            cutout,
        },
    )?;
    i.polygon = index(u16_at(p, 5)?);
    if let Some(owner) = f.get("PCBCRAFT_POLYGON_CUTOUT_OWNER") {
        i.polygon = Some(
            owner
                .parse::<u32>()
                .map_err(|_| "Invalid polygon cutout owner")?,
        );
    }
    if let Some(layer) = f.get("V7_LAYER") {
        i.layer = layer_id(layer)?;
    }
    i.properties = f;
    Ok(i)
}
fn finish(b: &mut Board) -> Result<()> {
    // Native component comments commonly live in the owned Texts6 record,
    // while Components6 contains no SOURCECOMMENT at all.
    for component in &mut b.components {
        if let Some(text) = b.items.iter().find_map(|item| {
            if item.component == Some(component.index)
                && item
                    .properties
                    .get("COMMENT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
                && let Shape::Text { text, .. } = &item.shape
            {
                Some(text)
            } else {
                None
            }
        }) {
            component.comment = text.clone();
        }
    }
    let mut missing_nets = BTreeSet::new();
    let known_nets: BTreeSet<_> = b.nets.iter().map(|n| n.index).collect();
    let known_components: BTreeSet<_> = b.components.iter().map(|c| c.index).collect();
    let mut dangling_components = 0;
    for i in &mut b.items {
        if let Some(n) = i.net
            && !known_nets.contains(&n)
        {
            missing_nets.insert(n);
        }
        if i.component.is_some_and(|c| !known_components.contains(&c)) {
            i.component = None;
            dangling_components += 1;
        }
    }
    for n in missing_nets {
        b.nets.push(Net {
            index: n,
            name: format!("Unresolved net #{n}"),
            properties: Fields::new(),
        });
        warn(
            b,
            "Some primitive net references were absent from Nets6; explicit unresolved-net placeholders were retained",
        );
    }
    if dangling_components > 0 {
        warn(
            b,
            format!(
                "{dangling_components} primitives had unresolved component references and remain standalone; original references are archived"
            ),
        );
    }
    let mut used: BTreeSet<_> = b.items.iter().map(|i| i.layer).collect();
    used.extend([TOP, BOTTOM, TOP_OVERLAY, BOTTOM_OVERLAY, MULTI_LAYER]);
    for item in &b.items {
        if let Shape::Via {
            from_layer,
            to_layer,
            ..
        } = item.shape
        {
            used.extend([from_layer, to_layer]);
        }
    }
    for id in 1..=98 {
        if !b.properties.text(&format!("LAYER{id}NAME")).is_empty()
            && b.properties.num(&format!("LAYER{id}NEXT"), 0.0)? != 0.0
        {
            used.insert(id);
        }
    }
    let native_names: BTreeMap<_, _> = b
        .properties
        .iter()
        .filter_map(|(key, value)| {
            let prefix = key.strip_suffix("_LAYERID")?;
            if !prefix.starts_with("V9_CACHE_LAYER") {
                return None;
            }
            let id = native_layer(value.parse().ok()?)?;
            let name = b.properties.get(&format!("{prefix}_NAME"))?;
            Some((id, name.clone()))
        })
        .collect();
    b.layers = used
        .into_iter()
        .enumerate()
        .map(|(order, id)| {
            let raw = native_names
                .get(&id)
                .cloned()
                .unwrap_or_else(|| b.properties.text(&format!("LAYER{id}NAME")));
            PcbLayer {
                id,
                name: if raw.is_empty() { layer_name(id) } else { raw },
                copper: copper_layer(id),
                order: order as u16,
            }
        })
        .collect();
    let mut current = TOP;
    let mut ordered = BTreeSet::new();
    let mut order = 0;
    while ordered.insert(current) {
        if let Some(layer) = b.layers.iter_mut().find(|l| l.id == current) {
            layer.order = order;
            order += 1;
        }
        let next = b.properties.num(&format!("LAYER{current}NEXT"), 0.0)? as u16;
        if next == 0 {
            break;
        }
        current = next;
    }
    if ordered.len() > 1 {
        for layer in &mut b.layers {
            if !ordered.contains(&layer.id) {
                layer.order = order;
                order += 1;
            }
        }
    }
    if b.outline.len() < 3 {
        warn(
            b,
            "No valid Board6 outline was found; fit uses object extents, no rectangular board shape was invented",
        );
    }
    if b.items
        .iter()
        .any(|i| matches!(i.shape, Shape::Text { .. }))
    {
        warn(
            b,
            "PCB text preserves native size, rotation and mirror; proprietary Altium stroke glyphs, embedded TrueType faces and special text styles are not fully reproduced",
        );
    }
    Ok(())
}
fn ascii(bytes: &[u8], ns: Uuid) -> Result<Board> {
    let owned;
    let input = if bytes.starts_with(&[0xff, 0xfe]) {
        owned = utf16(&bytes[2..])?;
        owned.as_bytes()
    } else {
        bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
    };
    let mut board = Board {
        polygon_model_version: 0,
        ..Board::default()
    };
    let mut records = Vec::new();
    for line in input.split(|b| *b == b'\n') {
        if line.len() > MAX_BLOCK {
            return Err("ASCII PCB line exceeds limit".into());
        }
        let f = fields(line.strip_suffix(b"\r").unwrap_or(line))?;
        if !f.is_empty() {
            records.push(f);
        }
        if records.len() > MAX_ITEMS {
            return Err("ASCII PCB record count exceeds limit".into());
        }
    }
    if !records
        .iter()
        .any(|f| f.text("RECORD").eq_ignore_ascii_case("Board"))
    {
        return Err("Not an Altium ASCII PCB: Board record missing".into());
    }
    for f in &records {
        match f.text("RECORD").to_ascii_uppercase().as_str() {
            "BOARD" => {
                board.properties = f.clone();
                board.outline = contour(f)?;
            }
            "NET" => board.nets.push(Net {
                index: unsigned_property(f, "ID", board.nets.len() as u32)?,
                name: f.text("NAME"),
                properties: f.clone(),
            }),
            "COMPONENT" => {
                let mut c = component(f.clone(), ns, board.components.len())?;
                c.index = unsigned_property(f, "ID", c.index)?;
                if c.designator.is_empty() {
                    c.designator = f.text("DESIGNATOR");
                }
                board.components.push(c);
            }
            "RULE" => board.rules.push(f.clone()),
            "POLYGON" => board.polygons.push(f.clone()),
            _ => {}
        }
    }
    for (n, f) in records.into_iter().enumerate() {
        let kind = f.text("RECORD").to_ascii_uppercase();
        let shape = match kind.as_str() {
            "TRACK" => Shape::Track {
                start: f.pos("X1", "Y1")?,
                end: f.pos("X2", "Y2")?,
                width: f.dim("WIDTH", 250_000)?,
            },
            "ARC" => Shape::Arc {
                centre: if f.contains_key("LOCATION.X") {
                    f.pos("LOCATION.X", "LOCATION.Y")?
                } else {
                    f.pos("X", "Y")?
                },
                radius: f.dim("RADIUS", 0)?,
                start_angle: f.num("STARTANGLE", 0.0)?,
                end_angle: f.num("ENDANGLE", 360.0)?,
                width: f.dim("WIDTH", 250_000)?,
            },
            "VIA" => Shape::Via {
                position: f.pos("X", "Y")?,
                diameter: f.dim("DIAMETER", 600_000)?,
                hole: f.dim("HOLESIZE", 300_000)?,
                from_layer: layer_id(f.get("STARTLAYER").map_or("TOP", String::as_str))?,
                to_layer: layer_id(f.get("ENDLAYER").map_or("BOTTOM", String::as_str))?,
            },
            "PAD" => Shape::Pad {
                position: f.pos("X", "Y")?,
                size: Point {
                    x: f.dim("XSIZE", 1_000_000)?,
                    y: f.dim("YSIZE", 1_000_000)?,
                },
                rotation: f.num("ROTATION", 0.0)?,
                shape: match f.text("SHAPE").to_ascii_uppercase().as_str() {
                    "ROUND" | "1" => PadShape::Round,
                    "OCTAGONAL" | "3" => PadShape::Octagonal,
                    "ROUNDRECT" | "9" => PadShape::RoundedRectangle,
                    _ => PadShape::Rectangle,
                },
                hole: f.dim("HOLESIZE", 0)?,
                plated: !f
                    .get("PLATED")
                    .is_some_and(|s| s.eq_ignore_ascii_case("FALSE")),
                name: f.text("NAME"),
                bottom_size: if f.contains_key("BOTTOMXSIZE") {
                    Some(f.pos("BOTTOMXSIZE", "BOTTOMYSIZE")?)
                } else {
                    None
                },
                middle_size: if f.contains_key("MIDXSIZE") {
                    Some(f.pos("MIDXSIZE", "MIDYSIZE")?)
                } else {
                    None
                },
            },
            "TEXT" => Shape::Text {
                position: f.pos("X", "Y")?,
                text: f.text("TEXT"),
                height: f.dim("HEIGHT", 1_000_000)?,
                stroke: f.dim("WIDTH", 150_000)?,
                rotation: f.num("ROTATION", 0.0)?,
                mirrored: f.yes("MIRROR"),
                font: f.text("FONTNAME"),
            },
            "FILL" => Shape::Fill {
                start: f.pos("X1", "Y1")?,
                end: f.pos("X2", "Y2")?,
                rotation: f.num("ROTATION", 0.0)?,
            },
            "REGION" => {
                let vertices = contour(&f)?;
                let mut points = Vec::new();
                for v in vertices {
                    if let Some(a) = v.arc {
                        points.extend(arc_points(
                            a.centre,
                            a.radius,
                            a.start_angle,
                            a.end_angle,
                            128,
                        ));
                    } else {
                        points.push(v.position);
                    }
                }
                Shape::Region {
                    contour: points,
                    holes: holes(&f)?,
                    cutout: f.yes("ISBOARDCUTOUT") || f.num("KIND", 0.0)? != 0.0,
                }
            }
            "BOARD" | "NET" | "COMPONENT" | "RULE" | "POLYGON" => continue,
            _ => {
                warn(
                    &mut board,
                    format!("ASCII record {kind}: retained in original archive, not interpreted"),
                );
                continue;
            }
        };
        let read_index = |key: &str| -> Result<Option<u32>> {
            let v = f.num(key, -1.0)?;
            if v == -1.0 || v == 65535.0 {
                Ok(None)
            } else if v < 0.0 || v.fract() != 0.0 || v > u32::MAX as f64 {
                Err(format!("Invalid PCB index {key}"))
            } else {
                Ok(Some(v as u32))
            }
        };
        let layer = if kind == "VIA" {
            MULTI_LAYER
        } else {
            layer_id(f.get("LAYER").map_or("TOP", String::as_str))?
        };
        let mut item = Item::new(layer, read_index("NET")?, shape);
        item.id = stable(ns, "ASCII", n);
        item.component = read_index("COMPONENT")?;
        item.polygon = read_index("POLYGON")?;
        item.locked = f.yes("LOCKED");
        item.keepout = f.yes("KEEPOUT");
        item.source = Some(SourceRecord {
            stream: "ASCII".into(),
            index: n as u32,
        });
        if let Some(owner) = f.get("PCBCRAFT_POLYGON_CUTOUT_OWNER") {
            item.polygon = Some(
                owner
                    .parse::<u32>()
                    .map_err(|_| "Invalid polygon cutout owner")?,
            );
        }
        item.properties = f;
        board.items.push(item);
    }
    if !board.polygons.is_empty() {
        warn(
            &mut board,
            "Use Tools / Polygon Pours / Repour All to fill imported unpoured polygons",
        );
    }
    Ok(board)
}
fn holes(f: &Fields) -> Result<Vec<Vec<Point>>> {
    let mut holes = Vec::new();
    for n in 0..10000 {
        let Some(count) = f.get(&format!("HOLE{n}COUNT")) else {
            break;
        };
        let count = count
            .parse::<usize>()
            .map_err(|_| "Invalid hole vertex count")?;
        if !(3..=20000).contains(&count) {
            return Err("Hole vertex count out of range".into());
        }
        let mut points = Vec::with_capacity(count);
        for j in 0..count {
            points.push(f.pos(&format!("HOLE{n}VX{j}"), &format!("HOLE{n}VY{j}"))?);
        }
        if points.first() == points.last() {
            points.pop();
        }
        holes.push(points);
    }
    Ok(holes)
}
/// Migrate the v1 metadata-only polygons without changing legacy source archives.
pub fn upgrade_polygons(board: &mut Board, namespace: Uuid) -> Result<()> {
    if board.polygon_model_version >= 1 {
        return Ok(());
    }
    if !board.polygon_pours.is_empty() {
        board.polygon_model_version = 1;
        return Ok(());
    }
    use eda_core::pcb_polygon::{Connection, Polygon};
    for (n, f) in board.polygons.iter().enumerate() {
        let net = f.num("NET", -1.0)?;
        let index = unsigned_property(f, "ID", n as u32)?;
        let p = Polygon {
            id: stable(namespace, "Polygons6", n),
            index,
            name: f
                .get("NAME")
                .cloned()
                .unwrap_or_else(|| format!("Polygon {}", n + 1)),
            layer: layer_id(f.get("LAYER").map_or("TOP", String::as_str))?,
            net: if net < 0.0 || net == 65535.0 {
                None
            } else {
                Some(net as u32)
            },
            contour: contour(f)?,
            holes: holes(f)?,
            clearance: f.dim("PCBCRAFT_CLEARANCE", 200_000)?,
            connection: match f.text("PCBCRAFT_CONNECTION").as_str() {
                "Thermal" => Connection::Thermal,
                "None" => Connection::None,
                _ => Connection::Direct,
            },
            thermal_gap: f.dim("PCBCRAFT_THERMALGAP", 250_000)?,
            spoke_width: f.dim("PCBCRAFT_SPOKEWIDTH", 300_000)?,
            spoke_angle: f.num("PCBCRAFT_SPOKEANGLE", 45.0)?,
            priority: unsigned_property(f, "POURINDEX", n as u32)?,
            remove_islands: f.yes("REMOVEISLANDS"),
            min_island_area_mm2: f.num("PCBCRAFT_MINAREA", 0.0)?,
            shelved: f.yes("SHELVED"),
            locked: f.yes("LOCKED"),
            properties: f.clone(),
        };
        // Unsupported outlines stay visible as a warning, and native writing refuses
        // to rebuild Polygons6 rather than deleting these records silently.
        match p.validate() {
            Ok(()) => board.polygon_pours.push(p),
            Err(e) => board
                .warnings
                .push(format!("Polygon {index} not editable: {e}")),
        }
    }
    board.polygon_model_version = 1;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn board() -> &'static [u8] {
        b"|RECORD=Board|VX0=0mil|VY0=0mil|VX1=1000mil|VY1=0mil|VX2=1000mil|VY2=500mil|VX3=0mil|VY3=500mil\n|RECORD=Net|ID=0|NAME=GND\n|RECORD=Track|LAYER=TOP|NET=0|X1=100mil|Y1=100mil|X2=200mil|Y2=200mil|WIDTH=10mil\n|RECORD=Pad|LAYER=TOP|NET=0|X=100mil|Y=100mil|XSIZE=40mil|YSIZE=20mil|SHAPE=ROUND|NAME=1|ROTATION=90\n"
    }
    #[test]
    fn ascii_import_preserves_units_and_source() {
        let doc = import_pcb(board(), "example.PcbDoc").unwrap();
        let b = doc.pcb.unwrap();
        assert_eq!(b.items.len(), 2);
        assert_eq!(b.nets[0].name, "GND");
        assert_eq!(b.outline[1].position.x, 25_400_000);
        assert_eq!(
            STANDARD.decode(&b.source.unwrap().bytes_base64).unwrap(),
            board()
        );
    }
    #[test]
    fn imports_are_deterministic() {
        assert_eq!(
            import_pcb(board(), "test.PcbDoc").unwrap(),
            import_pcb(board(), "test.PcbDoc").unwrap()
        );
    }
    #[test]
    fn truncations_fail_without_panicking() {
        for n in 0..33 {
            assert!(primitive("Tracks6", &vec![0; n]).is_err());
        }
    }
    #[test]
    fn negative_fixed_point_rounds_symmetrically() {
        assert_eq!(coord(&(-1i32).to_le_bytes(), 0).unwrap(), -3);
        assert_eq!(coord(&(1i32).to_le_bytes(), 0).unwrap(), 3);
        assert_eq!(coord(&(10_000i32).to_le_bytes(), 0).unwrap(), 25_400);
    }
    #[test]
    fn dimensions_and_bad_values() {
        assert_eq!(dimension("1mm").unwrap(), 1_000_000);
        assert_eq!(dimension("-10mil").unwrap(), -254_000);
        assert!(dimension("NaNmil").is_err());
        assert!(dimension("1e100mm").is_err());
    }
    #[test]
    fn bounded_framing() {
        let mut b = Bytes::new(&[0xff, 0xff, 0xff, 0, 1]);
        assert!(b.block().is_err());
    }
    #[test]
    fn track_record() {
        let mut b = vec![0; 33];
        b[0] = 1;
        b[1] = 4;
        b[3..13].fill(255);
        b[21..25].copy_from_slice(&10_000i32.to_le_bytes());
        b[29..33].copy_from_slice(&1000i32.to_le_bytes());
        let i = primitive("Tracks6", &b).unwrap();
        assert!(!i.locked);
        assert_eq!(i.net, None);
        assert_eq!(i.component, None);
        if let Shape::Track { end, width, .. } = i.shape {
            assert_eq!(end.x, 25400);
            assert_eq!(width, 2540);
        } else {
            panic!();
        }
    }
    #[test]
    fn json_roundtrip_preserves_archive_and_geometry() {
        let doc = import_pcb(board(), "example.PcbDoc").unwrap();
        let json = serde_json::to_vec(&doc).unwrap();
        let same: Document = serde_json::from_slice(&json).unwrap();
        assert_eq!(doc, same);
    }
    #[test]
    fn source_restore_never_overwrites() {
        let doc = import_pcb(board(), "a.PcbDoc").unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("original.PcbDoc");
        restore_original(&path, &doc).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), board());
        assert!(restore_original(&path, &doc).is_err());
    }
    #[test]
    fn synthetic_fixture_counts() {
        let bytes = include_bytes!("../../../fixtures/pcb-update/synthetic.PcbDoc");
        let doc = import_pcb(bytes, "synthetic.PcbDoc").unwrap();
        let board = doc.pcb.unwrap();
        assert_eq!(board.items.len(), 9);
        assert_eq!(board.nets.len(), 2);
        assert_eq!(board.components.len(), 1);
        assert_eq!(board.outline.len(), 4);
        assert_eq!(
            board.outline[2].position,
            Point {
                x: 60_000_000,
                y: 40_000_000
            }
        );
    }
    #[test]
    fn mechanical_extended_layers_are_not_confused_with_legacy_ones() {
        assert_eq!(layer_id("Mechanical16").unwrap(), 72);
        assert_eq!(layer_id("Mechanical17").unwrap(), 83);
        assert_eq!(layer_id("Mechanical32").unwrap(), 98);
        assert_eq!(layer_id("Mechanical33").unwrap(), 99);
        assert!(layer_id("Mechanical1025").is_err());
        assert!(layer_id("MID-LAYER31").is_err());
    }
    #[test]
    fn system_layer_names_remain_non_copper_and_distinct_from_mechanical_layers() {
        for (name, id) in [
            ("CONNECT", 75),
            ("BACKGROUND", 76),
            ("DRCERROR", 77),
            ("HIGHLIGHT", 78),
            ("GRIDCOLOR1", 79),
            ("GRIDCOLOR10", 80),
            ("PADHOLE", 81),
            ("VIAHOLE", 82),
        ] {
            assert_eq!(layer_id(name).unwrap(), id);
            assert_eq!(layer_id(&layer_name(id)).unwrap(), id);
            assert!(!copper_layer(id));
            assert_ne!(mechanical_layer(17), Some(id));
        }
        assert!(layer_id("CONNECT_UNKNOWN").is_err());
    }
    #[test]
    fn internal_signal_layer_aliases_respect_native_range() {
        for n in 1..=30 {
            for name in [
                format!("MID{n}"),
                format!("mid{n}"),
                format!("Mid-Layer {n}"),
                format!("MidLayer{n}"),
            ] {
                assert_eq!(layer_id(&name).unwrap(), n + 1, "{name}");
            }
        }
        for name in ["MID0", "MID31", "MID65535", "MID", "MID-1", "MID1.5"] {
            assert!(layer_id(name).is_err(), "{name}");
        }
    }
    #[test]
    fn binary_zero_copper_npth_imports_and_survives_json_and_native_move() {
        let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        file.create_storage("/Board6").unwrap();
        let board = b"|VX0=0mm|VY0=0mm|VX1=20mm|VY1=0mm|VX2=20mm|VY2=20mm|VX3=0mm|VY3=20mm\0";
        let mut data = (board.len() as u32).to_le_bytes().to_vec();
        data.extend_from_slice(board);
        file.create_stream("/Board6/Data")
            .unwrap()
            .write_all(&data)
            .unwrap();
        file.create_storage("/Pads6").unwrap();
        // Observed PCB6 layout: zero top/middle/bottom land, 0.45 mm NPTH.
        let mut pad = vec![0u8; 194];
        pad[0] = MULTI_LAYER as u8;
        pad[1] = 4;
        pad[3..13].fill(255);
        pad[13..17].copy_from_slice(&1_000_000i32.to_le_bytes());
        pad[17..21].copy_from_slice(&2_000_000i32.to_le_bytes());
        pad[45..49].copy_from_slice(&177_165i32.to_le_bytes());
        pad[49..52].fill(1);
        pad[52..60].copy_from_slice(&270f64.to_le_bytes());
        pad[180..194].copy_from_slice(b"opaque-pad-end");
        let mut data = vec![2];
        for block in [b"\x010".as_slice(), &[], &[], &[], &pad, &[]] {
            data.extend_from_slice(&(block.len() as u32).to_le_bytes());
            data.extend_from_slice(block);
        }
        file.create_stream("/Pads6/Data")
            .unwrap()
            .write_all(&data)
            .unwrap();
        file.create_stream("/Pads6/Header")
            .unwrap()
            .write_all(&1u32.to_le_bytes())
            .unwrap();
        let bytes = file.into_inner().into_inner();
        let mut doc = import_pcb(&bytes, "hole.PcbDoc").unwrap();
        let b = doc.pcb.as_ref().unwrap();
        assert_eq!(b.items.len(), 1);
        assert!(b.items[0].is_drill_only());
        assert!(
            matches!(&b.items[0].shape,Shape::Pad{size,hole:449_999,plated:false,..}if *size==Point::default())
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hole.pcbcraft.json");
        crate::save(&path, &doc).unwrap();
        assert_eq!(crate::load(&path).unwrap(), doc);
        let id = b.items[0].id;
        let old = b.items[0].anchor();
        doc.pcb = Some(
            b.edited(Edit::Transform {
                selection: vec![id],
                pivot: old,
                translation: Point {
                    x: 2_540_000,
                    y: 5_080_000,
                },
                angle: 0.0,
            })
            .unwrap(),
        );
        let output = writer::serialize(&doc, writer::Format::Preserve).unwrap();
        let moved = import_pcb(&output, "moved.PcbDoc").unwrap();
        let item = &moved.pcb.as_ref().unwrap().items[0];
        assert!(item.is_drill_only());
        assert_eq!(
            item.anchor(),
            Point {
                x: old.x + 2_540_000,
                y: old.y + 5_080_000
            }
        );
        let mut cfb = cfb::CompoundFile::open(Cursor::new(output)).unwrap();
        let mut data = Vec::new();
        cfb.open_stream("/Pads6/Data")
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        assert!(data.windows(14).any(|s| s == b"opaque-pad-end"));
        assert_eq!(
            STANDARD
                .decode(
                    &doc.pcb
                        .as_ref()
                        .unwrap()
                        .source
                        .as_ref()
                        .unwrap()
                        .bytes_base64
                )
                .unwrap(),
            bytes
        );
    }
    #[test]
    fn binary_polygons_import_short_internal_layer_names() {
        let mut file = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        file.create_storage("/Board6").unwrap();
        let outline = b"|VX0=0mm|VY0=0mm|VX1=20mm|VY1=0mm|VX2=20mm|VY2=20mm|VX3=0mm|VY3=20mm\0";
        let mut board_data = (outline.len() as u32).to_le_bytes().to_vec();
        board_data.extend_from_slice(outline);
        file.create_stream("/Board6/Data")
            .unwrap()
            .write_all(&board_data)
            .unwrap();
        file.create_storage("/Polygons6").unwrap();
        let mut data = Vec::new();
        for (index, layer) in ["MID1", "MID2", "MID30"].into_iter().enumerate() {
            let record = format!(
                "|ID={index}|LAYER={layer}|POURINDEX={index}|VX0=0mm|VY0=0mm|VX1=10mm|VY1=0mm|VX2=10mm|VY2=10mm|VX3=0mm|VY3=10mm\0"
            );
            data.extend_from_slice(&(record.len() as u32).to_le_bytes());
            data.extend_from_slice(record.as_bytes());
        }
        file.create_stream("/Polygons6/Data")
            .unwrap()
            .write_all(&data)
            .unwrap();
        file.create_stream("/Polygons6/Header")
            .unwrap()
            .write_all(&3u32.to_le_bytes())
            .unwrap();
        let bytes = file.into_inner().into_inner();
        let doc = import_pcb(&bytes, "internal-layers.PcbDoc").unwrap();
        let board = doc.pcb.unwrap();
        assert_eq!(board.polygons.len(), 3);
        assert_eq!(
            board
                .polygon_pours
                .iter()
                .map(|p| p.layer)
                .collect::<Vec<_>>(),
            [2, 3, 31]
        );
        assert_eq!(board.polygons[0].get("LAYER").unwrap(), "MID1");
        assert_eq!(
            STANDARD
                .decode(&board.source.unwrap().bytes_base64)
                .unwrap(),
            bytes
        );
    }
    #[test]
    fn nonintegral_and_negative_indices_are_rejected() {
        for value in ["-2", "1.5", "4294967296"] {
            let f = fields(format!("|ID={value}").as_bytes()).unwrap();
            assert!(unsigned_property(&f, "ID", 0).is_err());
        }
    }
}
