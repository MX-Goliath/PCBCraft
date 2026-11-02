//! Native PCB serializer. Imported binary documents stay binary; imported ASCII
//! documents stay ASCII. Unknown CFB streams and unmodified record tails are retained.
//! Unsupported dependency-changing edits are rejected BEFORE touching the destination.
use super::*;
use eda_core::pcb_polygon::{self, Polygon};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Preserve,
    Binary,
    Ascii,
}
#[derive(Clone, Debug)]
struct Frame {
    tag: u8,
    blocks: Vec<Vec<u8>>,
    flags: Vec<u8>,
}
const FAMILIES: [(&str, u8, usize); 10] = [
    ("Tracks6", 4, 1),
    ("Arcs6", 1, 1),
    ("Vias6", 3, 1),
    ("Pads6", 2, 6),
    ("Fills6", 6, 1),
    ("Texts6", 5, 2),
    ("Regions6", 11, 1),
    ("ShapeBasedRegions6", 11, 1),
    ("ComponentBodies6", 12, 1),
    ("ShapeBasedComponentBodies6", 12, 1),
];
fn parse_frames(data: &[u8], tag: u8, count: usize) -> Result<Vec<Frame>> {
    let mut r = Bytes::new(data);
    let mut frames = Vec::new();
    while r.remaining() > 0 {
        if r.u8()? != tag {
            return Err("Unexpected primitive type while preparing native save".into());
        }
        let mut blocks = Vec::new();
        let mut flags = Vec::new();
        for _ in 0..count {
            let word = r.u32()?;
            let n = (word & 0xffffff) as usize;
            if n > MAX_BLOCK {
                return Err("Primitive subrecord exceeds limit".into());
            }
            blocks.push(r.take(n)?.to_vec());
            flags.push((word >> 24) as u8);
        }
        frames.push(Frame { tag, blocks, flags });
        if frames.len() > MAX_ITEMS {
            return Err("Primitive count exceeds limit".into());
        }
    }
    Ok(frames)
}
fn frame_bytes(frame: &Frame) -> Result<Vec<u8>> {
    let mut out = vec![frame.tag];
    for (n, b) in frame.blocks.iter().enumerate() {
        if b.len() > 0xffffff {
            return Err("Native subrecord exceeds its 24-bit length field".into());
        }
        out.extend_from_slice(
            &((b.len() as u32) | ((frame.flags.get(n).copied().unwrap_or(0) as u32) << 24))
                .to_le_bytes(),
        );
        out.extend_from_slice(b);
    }
    Ok(out)
}
fn property_blocks(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let mut r = Bytes::new(data);
    let mut out = Vec::new();
    while r.remaining() > 0 {
        out.push(r.block()?.to_vec());
        if out.len() > MAX_ITEMS {
            return Err("Property record limit exceeded".into());
        }
    }
    Ok(out)
}
fn encode_blocks(blocks: &[Vec<u8>]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for b in blocks {
        if b.len() > 0xffffff {
            return Err("Property block too large".into());
        }
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(b);
    }
    Ok(out)
}
fn safe_text(s: &str) -> Result<()> {
    if s.chars().any(|c| matches!(c, '|' | '\0' | '\r' | '\n')) {
        return Err("Native property values cannot contain '|', NUL or line breaks".into());
    }
    Ok(())
}
fn encoded_pair(k: &str, v: &str) -> Result<Vec<u8>> {
    safe_text(k)?;
    safe_text(v)?;
    let mut b = format!("|{k}=").into_bytes();
    let (bytes, _, bad) = WINDOWS_1252.encode(v);
    b.extend_from_slice(&bytes);
    if bad {
        b.extend_from_slice(format!("|%UTF8%{k}={v}").as_bytes());
    }
    if !v.is_ascii() {
        let units = v
            .encode_utf16()
            .map(|word| word.to_string())
            .collect::<Vec<_>>()
            .join(",");
        b.extend_from_slice(format!("|UNICODE=EXISTS|UNICODE__{k}={units}").as_bytes());
    }
    Ok(b)
}
/// Preserve unrelated fields, their order, duplicate keys, bytes and spelling.
fn patch_fields(raw: &[u8], changes: &BTreeMap<String, Option<String>>) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut written = BTreeSet::new();
    for segment in raw.strip_suffix(&[0]).unwrap_or(raw).split(|b| *b == b'|') {
        if segment.is_empty() {
            continue;
        }
        let key = segment
            .iter()
            .position(|b| *b == b'=')
            .and_then(|n| std::str::from_utf8(&segment[..n]).ok())
            .map(|s| s.to_ascii_uppercase());
        let lookup = key.as_deref().map(|s| {
            s.strip_prefix("%UTF8%")
                .or_else(|| s.strip_prefix("UNICODE__"))
                .unwrap_or(s)
        });
        if let Some(k) = lookup
            && let Some(value) = changes.get(k)
        {
            if written.insert(k.to_string())
                && let Some(v) = value
            {
                out.extend(encoded_pair(k, v)?);
            }
        } else {
            out.push(b'|');
            out.extend_from_slice(segment);
        }
    }
    for (k, v) in changes {
        if !written.contains(k)
            && let Some(v) = v
        {
            out.extend(encoded_pair(k, v)?);
        }
    }
    out.push(0);
    Ok(out)
}
fn rule_source(original: &[Fields], rule: &Fields, index: usize) -> Result<Option<usize>> {
    // Native files can contain duplicate UNIQUEIDs. An untouched record at its
    // original ordinal must keep its own opaque prefix, even in those files.
    if original.get(index) == Some(rule) {
        return Ok(Some(index));
    }
    if let Some(index) = original.iter().position(|r| r == rule) {
        return Ok(Some(index));
    }
    if let Some(id) = rule.get("UNIQUEID").filter(|s| !s.is_empty()) {
        let candidates: Vec<_> = original
            .iter()
            .enumerate()
            .filter(|(_, r)| {
                r.get("UNIQUEID") == Some(id) && r.get("RULEKIND") == rule.get("RULEKIND")
            })
            .map(|(i, _)| i)
            .collect();
        if candidates.len() <= 1 {
            return Ok(candidates.first().copied());
        }
        let named: Vec<_> = candidates
            .into_iter()
            .filter(|i| original[*i].get("NAME") == rule.get("NAME"))
            .collect();
        return if named.len() == 1 {
            Ok(Some(named[0]))
        } else {
            Err("Native rule identity is ambiguous: duplicate UNIQUEID and changed name; keep edits in JSON".into())
        };
    }
    Ok(original
        .iter()
        .position(|r| {
            r == rule
                || (r.get("NAME") == rule.get("NAME") && r.get("RULEKIND") == rule.get("RULEKIND"))
        })
        .or_else(|| {
            original
                .get(index)
                .filter(|r| {
                    r.get("UNIQUEID").is_none() && r.get("RULEKIND") == rule.get("RULEKIND")
                })
                .map(|_| index)
        }))
}
fn map_changes(fields: &Fields) -> BTreeMap<String, Option<String>> {
    fields
        .iter()
        .map(|(k, v)| (k.clone(), Some(v.clone())))
        .collect()
}
fn string(v: impl ToString) -> Option<String> {
    Some(v.to_string())
}
fn dim_value(n: i64) -> String {
    format!("{:.6}mm", n as f64 / 1e6)
}
fn boolean(v: bool) -> String {
    if v { "TRUE" } else { "FALSE" }.into()
}
fn vertex_key(k: &str) -> bool {
    ["VX", "VY", "CX", "CY", "KIND", "R", "SA", "EA"]
        .iter()
        .any(|p| {
            k.strip_prefix(p)
                .is_some_and(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit()))
        })
}
fn outline_changes(
    raw: &[u8],
    outline: &[ContourVertex],
) -> Result<BTreeMap<String, Option<String>>> {
    let mut changes = BTreeMap::new();
    for k in fields(raw)?.keys().filter(|k| vertex_key(k)) {
        changes.insert(k.clone(), None);
    }
    for (n, v) in outline.iter().enumerate() {
        changes.insert(format!("VX{n}"), string(dim_value(v.position.x)));
        changes.insert(format!("VY{n}"), string(dim_value(v.position.y)));
        changes.insert(
            format!("KIND{n}"),
            string(if v.arc.is_some() { 1 } else { 0 }),
        );
        if let Some(a) = &v.arc {
            changes.insert(format!("CX{n}"), string(dim_value(a.centre.x)));
            changes.insert(format!("CY{n}"), string(dim_value(a.centre.y)));
            changes.insert(format!("R{n}"), string(dim_value(a.radius)));
            changes.insert(format!("SA{n}"), string(a.start_angle));
            changes.insert(format!("EA{n}"), string(a.end_angle));
        }
    }
    Ok(changes)
}
fn hole_key(k: &str) -> bool {
    k == "HOLECOUNT"
        || k.strip_prefix("HOLE")
            .is_some_and(|v| v.bytes().next().is_some_and(|b| b.is_ascii_digit()))
}
fn prune_holes(raw: &[u8], changes: &mut BTreeMap<String, Option<String>>) -> Result<()> {
    for k in fields(raw)?.keys() {
        if hole_key(k) && !changes.contains_key(k) {
            changes.insert(k.clone(), None);
        }
    }
    Ok(())
}
fn hole_fields(f: &mut Fields, holes: &[Vec<Point>]) {
    f.retain(|k, _| !hole_key(k));
    f.insert("HOLECOUNT".into(), holes.len().to_string());
    for (n, h) in holes.iter().enumerate() {
        f.insert(format!("HOLE{n}COUNT"), h.len().to_string());
        for (j, p) in h.iter().enumerate() {
            f.insert(format!("HOLE{n}VX{j}"), dim_value(p.x));
            f.insert(format!("HOLE{n}VY{j}"), dim_value(p.y));
        }
    }
}
fn polygon_fields(p: &Polygon) -> Fields {
    let mut f = p.properties.clone();
    for (k, v) in [
        ("RECORD", "Polygon".into()),
        ("ID", p.index.to_string()),
        ("NAME", p.name.clone()),
        ("LAYER", layer_token(p.layer)),
        ("NET", p.net.map_or("-1".into(), |n| n.to_string())),
        ("POURINDEX", p.priority.to_string()),
        ("SHELVED", boolean(p.shelved)),
        ("LOCKED", boolean(p.locked)),
        ("REMOVEISLANDS", boolean(p.remove_islands)),
        ("PCBCRAFT_CLEARANCE", dim_value(p.clearance)),
        ("PCBCRAFT_CONNECTION", format!("{:?}", p.connection)),
        ("PCBCRAFT_THERMALGAP", dim_value(p.thermal_gap)),
        ("PCBCRAFT_SPOKEWIDTH", dim_value(p.spoke_width)),
        ("PCBCRAFT_SPOKEANGLE", p.spoke_angle.to_string()),
        ("PCBCRAFT_MINAREA", p.min_island_area_mm2.to_string()),
    ] {
        f.insert(k.into(), v);
    }
    f.entry("POLYGONTYPE".into())
        .or_insert_with(|| "Solid".into());
    hole_fields(&mut f, &p.holes);
    f
}
fn layer_token(id: u16) -> String {
    match id {
        1 => "TOP".into(),
        32 => "BOTTOM".into(),
        33 => "TOPOVERLAY".into(),
        34 => "BOTTOMOVERLAY".into(),
        35 => "TOPPASTE".into(),
        36 => "BOTTOMPASTE".into(),
        37 => "TOPSOLDER".into(),
        38 => "BOTTOMSOLDER".into(),
        56 => "KEEPOUT".into(),
        74 => "MULTILAYER".into(),
        75 => "CONNECT".into(),
        76 => "BACKGROUND".into(),
        77 => "DRCERROR".into(),
        78 => "HIGHLIGHT".into(),
        79 => "GRIDCOLOR1".into(),
        80 => "GRIDCOLOR10".into(),
        81 => "PADHOLE".into(),
        82 => "VIAHOLE".into(),
        2..=31 => format!("MID-LAYER{}", id - 1),
        39..=54 => format!("INTERNALPLANE{}", id - 38),
        57..=72 => format!("MECHANICAL{}", id - 56),
        83..=1090 => format!("MECHANICAL{}", id - 66),
        _ => format!("LAYER{id}"),
    }
}
fn internal(n: i64) -> Result<i32> {
    let sign = if n < 0 { -1i128 } else { 1 };
    let n = n as i128 * 100;
    let value = (n + sign * 127) / 254;
    i32::try_from(value)
        .map_err(|_| "Coordinate exceeds Altium's signed 32-bit internal range".into())
}
fn put(b: &mut [u8], at: usize, value: &[u8]) -> Result<()> {
    b.get_mut(at..at + value.len())
        .ok_or("Native record is too short for the edited field")?
        .copy_from_slice(value);
    Ok(())
}
fn put_dim(b: &mut [u8], at: usize, n: i64) -> Result<()> {
    put(b, at, &internal(n)?.to_le_bytes())
}
fn put_point(b: &mut [u8], at: usize, p: Point) -> Result<()> {
    put_dim(b, at, p.x)?;
    put_dim(b, at + 4, p.y)
}
fn put_real(b: &mut [u8], at: usize, n: f64) -> Result<()> {
    if !n.is_finite() {
        return Err("Non-finite native field".into());
    }
    put(b, at, &n.to_le_bytes())
}
fn idx(n: Option<u32>) -> Result<u16> {
    n.map_or(Ok(0xffff), |n| {
        if n < 65535 {
            Ok(n as u16)
        } else {
            Err("Native index exceeds 65534".into())
        }
    })
}
fn common_patch(b: &mut [u8], i: &Item) -> Result<()> {
    if b.len() < 13 || i.layer > 255 {
        return Err("Unsupported native primitive header/layer".into());
    }
    b[0] = i.layer as u8;
    b[1] = (b[1] & !4) | if i.locked { 0 } else { 4 };
    b[2] = (b[2] & !2) | if i.keepout { 2 } else { 0 };
    put(b, 3, &idx(i.net)?.to_le_bytes())?;
    put(
        b,
        5,
        &idx(
            if i.properties.contains_key("PCBCRAFT_POLYGON_CUTOUT_OWNER") {
                None
            } else {
                i.polygon
            },
        )?
        .to_le_bytes(),
    )?;
    put(b, 7, &idx(i.component)?.to_le_bytes())
}
fn new_common(n: usize) -> Vec<u8> {
    let mut b = vec![0; n];
    b[1] = 0x0c;
    b[3..13].fill(255);
    b
}
fn pascal(s: &str, lossy: bool) -> Result<Vec<u8>> {
    let (b, _, bad) = WINDOWS_1252.encode(s);
    if (bad || b.len() > 255) && !lossy {
        return Err("Pad name must fit the native Windows-1252 short string".into());
    }
    let n = b.len().min(255);
    let mut out = vec![n as u8];
    out.extend_from_slice(&b[..n]);
    Ok(out)
}
fn family(item: &Item) -> &'static str {
    if matches!(item.shape, Shape::Region { .. }) {
        if let Some(source) = &item.source {
            if source.stream == "ShapeBasedComponentBodies6" {
                return "ShapeBasedComponentBodies6";
            }
            if source.stream == "ComponentBodies6" {
                return "ComponentBodies6";
            }
            if source.stream == "ShapeBasedRegions6" {
                return "ShapeBasedRegions6";
            }
            if source.stream == "Regions6" {
                return "Regions6";
            }
        }
        return if item.polygon.is_some() {
            "Regions6"
        } else {
            "ShapeBasedRegions6"
        };
    }
    match item.shape {
        Shape::Track { .. } => "Tracks6",
        Shape::Arc { .. } => "Arcs6",
        Shape::Pad { .. } => "Pads6",
        Shape::Via { .. } => "Vias6",
        Shape::Text { .. } => "Texts6",
        Shape::Fill { .. } => "Fills6",
        Shape::Region { .. } => unreachable!(),
    }
}
fn new_frame(i: &Item) -> Frame {
    let (tag, n) = match i.shape {
        Shape::Track { .. } => (4, 36),
        Shape::Arc { .. } => (1, 48),
        Shape::Via { .. } => (3, 209),
        Shape::Pad { .. } => (2, 114),
        Shape::Text { .. } => (5, 137),
        Shape::Fill { .. } => (6, 37),
        Shape::Region { .. } => (11, 18),
    };
    let blocks = match i.shape {
        Shape::Pad { .. } => vec![vec![0], vec![0], vec![0], vec![0], new_common(n), vec![]],
        Shape::Text { .. } => vec![new_common(n), vec![0]],
        _ => vec![new_common(n)],
    };
    Frame {
        tag,
        flags: vec![0; blocks.len()],
        blocks,
    }
}
// Locate the opaque region extension without interpreting it as geometry.
fn region_tail(payload: &[u8], extended: bool) -> Result<&[u8]> {
    if payload.len() < 22 {
        return Err("Truncated original region".into());
    }
    let property_len = (u32_at(payload, 18)? & 0xffffff) as usize;
    let mut r = Bytes::new(payload.get(22..).ok_or("Truncated region properties")?);
    r.take(property_len)?;
    let stored = r.u32()? as usize;
    let count = stored
        .checked_add(usize::from(extended))
        .ok_or("Region vertex count overflow")?;
    r.take(
        count
            .checked_mul(if extended { 37 } else { 16 })
            .ok_or("Region geometry size overflow")?,
    )?;
    for _ in 0..u16_at(payload, 14)? {
        let count = r.u32()? as usize;
        r.take(count.checked_mul(16).ok_or("Region hole size overflow")?)?;
    }
    r.take(r.remaining())
}
fn modified_frame(
    item: &Item,
    old: Option<&Item>,
    original: Option<&Frame>,
    stream: &str,
    wide_index: u32,
) -> Result<Frame> {
    if item.properties.contains_key("PCBCRAFT_BODY_PROJECTION")
        && old.is_none_or(|o| {
            o.shape != item.shape || o.properties != item.properties || o.layer != item.layer
        })
    {
        return Err("Native saving of modified 3D body projections requires full 3D model transforms; keep this edit in PCBCraft JSON".into());
    }
    let mut f = original.cloned().unwrap_or_else(|| new_frame(item));
    let offset = if f.tag == 2 { 4 } else { 0 };
    let geometry_changed = old.is_none_or(|o| {
        o.shape != item.shape
            || o.pad_stack != item.pad_stack
            || o.masks != item.masks
            || (matches!(item.shape, Shape::Region { .. })
                && (o.polygon != item.polygon
                    || o.layer != item.layer
                    || o.properties != item.properties))
            || (matches!(item.shape, Shape::Text { .. }) && o.properties != item.properties)
    });
    let b = &mut f.blocks[offset];
    // Modern layer overrides cannot be guessed from a V6 layer byte.
    if let Some(old) = old
        && old.layer != item.layer
    {
        let at = match stream {
            "Tracks6" => Some(41),
            "Arcs6" => Some(56),
            "Fills6" => Some(42),
            "Texts6" => Some(226),
            _ => None,
        };
        if let Some(at) = at
            && b.len() >= at + 4
        {
            let v = u32_at(b, at)?;
            if v != 0 && v != old.layer as u32 {
                return Err(format!(
                    "{stream}: changing a V7/V8 layer override is not yet supported"
                ));
            }
            if v != 0 {
                put(b, at, &(item.layer as u32).to_le_bytes())?;
            }
        }
    }
    let legacy_layer = b[0];
    common_patch(b, item)?;
    if old.is_some_and(|old| old.layer == item.layer) {
        b[0] = legacy_layer;
    }
    if !geometry_changed {
        return Ok(f);
    }
    match &item.shape {
        Shape::Track { start, end, width } => {
            put_point(b, 13, *start)?;
            put_point(b, 21, *end)?;
            put_dim(b, 29, *width)?;
        }
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            put_point(b, 13, *centre)?;
            put_dim(b, 21, *radius)?;
            put_real(b, 25, *start_angle)?;
            put_real(b, 33, *end_angle)?;
            put_dim(b, 41, *width)?;
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => {
            put_point(b, 13, *start)?;
            put_point(b, 21, *end)?;
            put_real(b, 29, *rotation)?;
        }
        Shape::Via {
            position,
            diameter,
            hole,
            from_layer,
            to_layer,
        } => {
            if *from_layer > 255 || *to_layer > 255 {
                return Err("Unsupported via layer".into());
            }
            put_point(b, 13, *position)?;
            put_dim(b, 21, *diameter)?;
            put_dim(b, 25, *hole)?;
            b[29] = *from_layer as u8;
            b[30] = *to_layer as u8;
        }
        Shape::Pad {
            position,
            size,
            rotation,
            shape,
            hole,
            plated,
            name,
            bottom_size,
            middle_size,
        } => {
            put_point(b, 13, *position)?;
            put_point(b, 21, *size)?;
            put_point(b, 29, middle_size.unwrap_or(*size))?;
            put_point(b, 37, bottom_size.unwrap_or(*size))?;
            put_dim(b, 45, *hole)?;
            let shape_id = match shape {
                PadShape::Round => 1,
                PadShape::Rectangle | PadShape::RoundedRectangle => 2,
                PadShape::Octagonal => 3,
            };
            if old.is_none_or(|o| matches!(&o.shape,Shape::Pad{shape:sh,..} if sh!=shape)) {
                b[49..52].fill(shape_id);
            }
            put_real(b, 52, *rotation)?;
            b[60] = u8::from(*plated);
            if b[62] != 2 {
                b[62] = u8::from(
                    bottom_size.is_some_and(|s| s != *size)
                        || middle_size.is_some_and(|s| s != *size),
                );
            }
            f.blocks[0] = pascal(name, false)?;
            if *shape == PadShape::RoundedRectangle
                && old.is_none_or(
                    |o| matches!(o.shape,Shape::Pad{shape:old_shape,..} if old_shape!=*shape),
                )
            {
                let stack = &mut f.blocks[5];
                if stack.len() < 596 {
                    stack.resize(596, 0);
                }
                stack[532..564].fill(9);
                stack[564..596].fill(25);
            } else if *shape != PadShape::RoundedRectangle
                && old.is_some_and(|o| {
                    matches!(
                        o.shape,
                        Shape::Pad {
                            shape: PadShape::RoundedRectangle,
                            ..
                        }
                    )
                })
                && f.blocks[5].len() >= 596
            {
                f.blocks[5][532..564].fill(0);
                f.blocks[5][564..596].fill(0);
            }
        }
        Shape::Text {
            position,
            text,
            height,
            stroke,
            rotation,
            mirrored,
            font,
        } => {
            if b.len() < 137 {
                b.resize(137, 0);
            }
            put_point(b, 13, *position)?;
            put_dim(b, 21, *height)?;
            put_real(b, 27, *rotation)?;
            b[35] = u8::from(*mirrored);
            put_dim(b, 36, *stroke)?;
            b[40] = u8::from(
                item.properties
                    .get("COMMENT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE")),
            );
            b[41] = u8::from(
                item.properties
                    .get("DESIGNATOR")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE")),
            );
            if font.encode_utf16().count() > 31 {
                return Err("PCB text font name exceeds native field".into());
            }
            b[46..110].fill(0);
            for (n, w) in font.encode_utf16().enumerate() {
                put(b, 46 + 2 * n, &w.to_le_bytes())?;
            }
            put(b, 115, &wide_index.to_le_bytes())?;
            f.blocks[1] = pascal(text, true)?;
        }
        Shape::Region {
            contour,
            holes,
            cutout,
        } => {
            if holes.len() > 65535 {
                return Err("Too many region holes".into());
            }
            let mut properties = item.properties.clone();
            if old.is_some() && properties.get("KIND").is_some_and(|k| k != "0" && k != "1") {
                return Err(
                    "Editing a cavity/outline region requires its native region-kind semantics"
                        .into(),
                );
            }
            let board_cutout = *cutout
                && properties
                    .get("ISBOARDCUTOUT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"));
            properties.insert(
                "KIND".into(),
                if *cutout && !board_cutout { "1" } else { "0" }.into(),
            );
            if !cutout {
                properties.insert("ISBOARDCUTOUT".into(), "FALSE".into());
            }
            properties.insert("V7_LAYER".into(), layer_token(item.layer));
            let cutout_owner = item
                .properties
                .contains_key("PCBCRAFT_POLYGON_CUTOUT_OWNER");
            if cutout_owner {
                properties.insert(
                    "PCBCRAFT_POLYGON_CUTOUT_OWNER".into(),
                    item.polygon
                        .ok_or("Cutout has no polygon owner")?
                        .to_string(),
                );
            }
            properties.insert(
                "SUBPOLYINDEX".into(),
                if cutout_owner {
                    "-1".into()
                } else {
                    item.polygon.map_or("-1".into(), |x| x.to_string())
                },
            );
            properties
                .entry("ISSHAPEBASED".into())
                .or_insert_with(|| "FALSE".into());
            let old_properties = if b.len() >= 22 {
                let n = (u32_at(b, 18)? & 0xffffff) as usize;
                b.get(22..22 + n).unwrap_or(&[])
            } else {
                &[]
            };
            let prop = patch_fields(old_properties, &map_changes(&properties))?;
            let prop = prop.strip_prefix(b"|").unwrap_or(&prop);
            let mut out = b[..18].to_vec();
            put(&mut out, 14, &(holes.len() as u16).to_le_bytes())?;
            out.extend_from_slice(&(prop.len() as u32).to_le_bytes());
            out.extend_from_slice(prop);
            let extended = stream == "ShapeBasedRegions6";
            // Shape-based records contain N edges and N+1 vertices (closed ring).
            out.extend_from_slice(&(contour.len() as u32).to_le_bytes());
            if extended {
                for p in contour.iter().chain(contour.first()) {
                    out.push(0);
                    out.extend_from_slice(&internal(p.x)?.to_le_bytes());
                    out.extend_from_slice(&internal(p.y)?.to_le_bytes());
                    out.extend_from_slice(&[0; 28]);
                }
            } else {
                for p in contour {
                    out.extend_from_slice(&(p.x as f64 / 2.54).to_le_bytes());
                    out.extend_from_slice(&(p.y as f64 / 2.54).to_le_bytes());
                }
            }
            for hole in holes {
                out.extend_from_slice(&(hole.len() as u32).to_le_bytes());
                for p in hole {
                    out.extend_from_slice(&(p.x as f64 / 2.54).to_le_bytes());
                    out.extend_from_slice(&(p.y as f64 / 2.54).to_le_bytes());
                }
            }
            if original.is_some() {
                out.extend_from_slice(region_tail(b, extended)?);
            }
            f.blocks[0] = out;
        }
    }
    stack_io::patch(item, old, &mut f.blocks)?;
    Ok(f)
}
fn read_streams(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let mut c = cfb::CompoundFile::open(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let paths: Vec<_> = c
        .walk()
        .filter(|e| e.is_stream())
        .map(|e| e.path().to_owned())
        .collect();
    let mut out = BTreeMap::new();
    let mut total = 0;
    for path in paths {
        let mut b = Vec::new();
        c.open_stream(&path)
            .map_err(|e| e.to_string())?
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut b)
            .map_err(|e| e.to_string())?;
        total += b.len();
        if b.len() > LIMIT || total > 2 * LIMIT {
            return Err("Expanded native streams exceed limits".into());
        }
        out.insert(path.to_string_lossy().replace('\\', "/"), b);
    }
    Ok(out)
}
#[derive(Default)]
struct Maps {
    nets: BTreeMap<u32, u32>,
    components: BTreeMap<u32, u32>,
    polygons: BTreeMap<u32, u32>,
}
impl Maps {
    fn new(b: &Board) -> Result<Self> {
        let map = |mut keys: Vec<u32>| -> Result<BTreeMap<u32, u32>> {
            keys.sort();
            keys.dedup();
            if keys.len() > 65535 {
                return Err("Native object reference table exceeds 65535 records".into());
            }
            Ok(keys
                .into_iter()
                .enumerate()
                .map(|(n, k)| (k, n as u32))
                .collect())
        };
        Ok(Self {
            nets: map(b.nets.iter().map(|x| x.index).collect())?,
            components: map(b.components.iter().map(|x| x.index).collect())?,
            polygons: map(b.polygon_pours.iter().map(|x| x.index).collect())?,
        })
    }
    fn item(&self, item: &Item) -> Result<Item> {
        let mut i = item.clone();
        let remap = |map: &BTreeMap<u32, u32>, n: Option<u32>| -> Result<Option<u32>> {
            n.map(|n| {
                map.get(&n)
                    .copied()
                    .ok_or_else(|| format!("Dangling native reference {n}"))
            })
            .transpose()
        };
        i.net = remap(&self.nets, i.net)?;
        i.component = remap(&self.components, i.component)?;
        i.polygon = remap(&self.polygons, i.polygon)?;
        Ok(i)
    }
}
fn source_bytes(board: &Board) -> Result<Option<Vec<u8>>> {
    board
        .source
        .as_ref()
        .map(|s| {
            if s.bytes_base64.len() > LIMIT * 4 / 3 + 8 {
                return Err("Archived PCB source exceeds limit".into());
            }
            STANDARD.decode(&s.bytes_base64).map_err(|e| e.to_string())
        })
        .transpose()
}
fn write_stream(c: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, path: &str, b: &[u8]) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        c.create_storage_all(parent).map_err(|e| e.to_string())?;
    }
    c.create_stream(path)
        .map_err(|e| e.to_string())?
        .write_all(b)
        .map_err(|e| e.to_string())
}
fn section(
    c: &mut cfb::CompoundFile<Cursor<Vec<u8>>>,
    old: &BTreeMap<String, Vec<u8>>,
    name: &str,
    data: &[u8],
    count: usize,
) -> Result<()> {
    let data_path = format!("/{name}/Data");
    let header_path = format!("/{name}/Header");
    if old.get(&data_path).is_none_or(|b| b != data) {
        write_stream(c, &data_path, data)?;
    }
    let mut header = old.get(&header_path).cloned().unwrap_or_else(|| vec![0; 4]);
    if header.len() < 4 {
        return Err("Truncated native record-count header".into());
    }
    put(&mut header, 0, &(count as u32).to_le_bytes())?;
    if old.get(&header_path) != Some(&header) {
        write_stream(c, &header_path, &header)?;
    }
    Ok(())
}
fn component_changes(c: &Component) -> Fields {
    let mut f = Fields::new();
    for key in [
        "SOURCEUNIQUEID",
        "SOURCESHEET",
        "SOURCEDESCRIPTION",
        "SOURCELIBREFERENCE",
        "COMPONENTKINDVERSION2",
        "SOURCEFOOTPRINTLIBRARY",
        "SOURCECOMPONENTLIBRARY",
        "SOURCELIBRARYNAME",
        "UNIQUEID",
    ] {
        if let Some(value) = c.properties.get(key) {
            f.insert(key.into(), value.clone());
        }
    }
    if c.parameters.as_ref().is_some_and(|p| !p.is_empty()) {
        f.insert("UNIQUEID".into(), component_uid(c));
    }
    for (k, v) in [
        ("SOURCEDESIGNATOR", c.designator.clone()),
        ("SOURCECOMMENT", c.comment.clone()),
        ("PATTERN", c.footprint.clone()),
        ("X", dim_value(c.position.x)),
        ("Y", dim_value(c.position.y)),
        ("ROTATION", c.rotation.to_string()),
        ("LAYER", layer_token(c.layer)),
        ("LOCKED", boolean(c.locked)),
    ] {
        f.insert(k.into(), v);
    }
    f
}
fn component_uid(c: &Component) -> String {
    c.properties
        .get("UNIQUEID")
        .cloned()
        .unwrap_or_else(|| c.id.simple().to_string()[..8].to_ascii_uppercase())
}

/// Rewrite only the base component namespace. Variant/system/footprint groups
/// and unrelated record fields are retained byte-for-byte when unmodified.
fn parameter_section(
    board: &Board,
    streams: &BTreeMap<String, Vec<u8>>,
    maps: &Maps,
) -> Result<(Vec<u8>, usize)> {
    let raw = streams
        .get("/PrimitiveParameters/Data")
        .map(|b| property_blocks(b))
        .transpose()?
        .unwrap_or_default();
    let mut out = vec![];
    let mut groups = 0;
    let mut n = 0;
    let mut owners = BTreeSet::new();
    while n < raw.len() {
        let h = fields(&raw[n])?;
        let count = h
            .get("COUNT")
            .ok_or("Parameter group has no count")?
            .parse::<usize>()
            .map_err(|_| "Invalid parameter count")?;
        let end = n
            .checked_add(1)
            .and_then(|n| n.checked_add(count))
            .filter(|end| *end <= raw.len())
            .ok_or("Truncated parameter group")?;
        let base = h.get("VARIANTGUID").is_none_or(String::is_empty)
            && h.get("APPURTENANCE").is_none_or(String::is_empty);
        let owner = if base {
            parameters::component_owner(board, &h)?
        } else {
            None
        };
        if base && let Some(owner) = owner {
            let c = board
                .components
                .iter()
                .find(|c| c.index == owner)
                .ok_or("Parameter owner was removed; native reference remapping is required")?;
            if !owners.insert(owner) {
                return Err(
                    "Multiple base parameter groups for one component require consolidation".into(),
                );
            }
            if let Some(parameters) = &c.parameters {
                let old: Vec<_> = raw[n + 1..end]
                    .iter()
                    .map(|b| fields(b))
                    .collect::<Result<_>>()?;
                let mut records = vec![];
                let mut names = BTreeSet::new();
                for (block, record) in raw[n + 1..end].iter().zip(&old) {
                    let name = record.get("NAME").ok_or("Parameter has no name")?;
                    if let Some((key, value)) = parameters
                        .iter()
                        .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    {
                        if !names.insert(key.to_uppercase()) {
                            return Err("Duplicate component parameter".into());
                        }
                        if record
                            .get("VALUE")
                            .is_some_and(|v| parameters::decode_value(name, v) == *value)
                            && key == name
                        {
                            records.push(block.clone());
                        } else {
                            records.push(patch_fields(
                                block,
                                &map_changes(&Fields::from([
                                    ("NAME".into(), key.clone()),
                                    ("VALUE".into(), parameters::encode_value(key, value)),
                                ])),
                            )?);
                        }
                    }
                }
                for (name, value) in parameters {
                    if names.insert(name.to_uppercase()) {
                        records.push(patch_fields(
                            &[],
                            &map_changes(&Fields::from([
                                ("NAME".into(), name.clone()),
                                ("VALUE".into(), parameters::encode_value(name, value)),
                                ("ISIMPORTED".into(), "TRUE".into()),
                            ])),
                        )?);
                    }
                }
                let mut changes = Fields::from([
                    ("COUNT".into(), records.len().to_string()),
                    (
                        "ID".into(),
                        format!("Component#{}", maps.components[&owner]),
                    ),
                    ("PRIMITIVEID".into(), component_uid(c)),
                ]);
                if !h.contains_key("ID") {
                    changes.remove("ID");
                }
                let header = if changes.iter().all(|(k, v)| h.get(k) == Some(v)) {
                    raw[n].clone()
                } else {
                    patch_fields(&raw[n], &map_changes(&changes))?
                };
                out.push(header);
                out.extend(records);
            } else {
                out.extend_from_slice(&raw[n..end]);
            }
        } else {
            out.extend_from_slice(&raw[n..end]);
        }
        groups += 1;
        n = end;
    }
    for c in &board.components {
        if owners.contains(&c.index) {
            continue;
        }
        let Some(parameters) = &c.parameters else {
            continue;
        };
        if parameters.is_empty() {
            continue;
        }
        let header = Fields::from([
            ("PRIMITIVEID".into(), component_uid(c)),
            (
                "ID".into(),
                format!("Component#{}", maps.components[&c.index]),
            ),
            ("VARIANTGUID".into(), String::new()),
            ("COUNT".into(), parameters.len().to_string()),
        ]);
        out.push(patch_fields(&[], &map_changes(&header))?);
        let mut names = BTreeSet::new();
        for (name, value) in parameters {
            if name.is_empty() || !names.insert(name.to_uppercase()) {
                return Err("Invalid or duplicate component parameter name".into());
            }
            out.push(patch_fields(
                &[],
                &map_changes(&Fields::from([
                    ("NAME".into(), name.clone()),
                    ("VALUE".into(), parameters::encode_value(name, value)),
                    ("ISIMPORTED".into(), "TRUE".into()),
                ])),
            )?);
        }
        groups += 1;
    }
    Ok((encode_blocks(&out)?, groups))
}
fn supported_dependency_guard(
    old: &Board,
    now: &Board,
    streams: &BTreeMap<String, Vec<u8>>,
    reindexed: bool,
) -> Result<()> {
    let moved: Vec<_> = old
        .components
        .iter()
        .filter(|c| {
            now.components
                .iter()
                .find(|n| n.index == c.index)
                .is_none_or(|n| {
                    n.position != c.position || n.rotation != c.rotation || n.layer != c.layer
                })
        })
        .collect();
    if !moved.is_empty()
        && ["ComponentBodies6", "ShapeBasedComponentBodies6"]
            .iter()
            .any(|s| {
                streams
                    .get(&format!("/{s}/Data"))
                    .is_some_and(|b| !b.is_empty())
            })
    {
        return Err("Native save refused: moved/deleted components require writing native 3D-body transforms. Keep the edit in PCBCraft JSON; mesh viewing does not yet implement lossless native writing of this change.".into());
    }
    if reindexed {
        // These sections can refer to primitive/table indexes that cannot safely be
        // guessed. A future writer can lift this guard by implementing each remap.
        for family in [
            "UniqueIDPrimitiveInformation",
            "ExtendedPrimitiveInformation",
            "PrimitiveGuids",
            "PrimitiveParameters",
            "PinPairsSection",
            "Connections6",
            "FromTos6",
            "Dimensions6",
            "SmartUnions",
            "Classes6",
            "DifferentialPairs6",
            "BoardRegions",
        ] {
            if streams
                .get(&format!("/{family}/Data"))
                .is_some_and(|b| !b.is_empty())
            {
                return Err(format!(
                    "Native save refused: deletion/reindexing affects nonempty {family}; reference remapping for this stream is not implemented. Original and destination were not changed."
                ));
            }
        }
    }
    Ok(())
}
fn encode_binary(
    board: &Board,
    original: Option<&[u8]>,
    old_board: Option<&Board>,
) -> Result<Vec<u8>> {
    let streams = original.map(read_streams).transpose()?.unwrap_or_default();
    let mut c = if let Some(bytes) = original {
        cfb::CompoundFile::open(Cursor::new(bytes.to_vec()))
    } else {
        cfb::CompoundFile::create_with_version(cfb::Version::V3, Cursor::new(Vec::new()))
    }
    .map_err(|e| e.to_string())?;
    if original.is_none() {
        let name = "PCB 6.0 Binary File";
        let mut header = (name.len() as u32).to_le_bytes().to_vec();
        header.extend(pascal(name, false)?);
        header.extend_from_slice(&5.01f64.to_le_bytes());
        write_stream(&mut c, "/FileHeaderSix", &header)?;
        let name = "PCB 5.0 Binary File";
        let mut legacy = (name.len() as u32).to_le_bytes().to_vec();
        for w in "PCB 5.0 Bi".encode_utf16() {
            legacy.extend_from_slice(&w.to_le_bytes());
        }
        write_stream(&mut c, "/FileHeader", &legacy)?;
    }
    let maps = Maps::new(board)?;
    let mut reindexed = false;
    if let Some(old) = old_board {
        reindexed = old
            .nets
            .iter()
            .any(|n| maps.nets.get(&n.index) != Some(&n.index))
            || old
                .components
                .iter()
                .any(|n| maps.components.get(&n.index) != Some(&n.index))
            || old
                .polygon_pours
                .iter()
                .any(|n| maps.polygons.get(&n.index) != Some(&n.index));
        if old.polygons.len() > old.polygon_pours.len() && board.polygon_pours != old.polygon_pours
        {
            return Err(
                "An unsupported imported polygon prevents rebuilding Polygons6 without data loss"
                    .into(),
            );
        }
    }
    // Original record ordering is kept. New primitives are appended within a family.
    let mut selected: BTreeMap<&str, Vec<&Item>> = BTreeMap::new();
    for i in &board.items {
        selected.entry(family(i)).or_default().push(i);
    }
    for (name, items) in &mut selected {
        items.sort_by_key(|i| {
            i.source
                .as_ref()
                .filter(|s| s.stream == *name)
                .map_or((1, u32::MAX), |s| (0, s.index))
        });
        if old_board.is_some_and(|old| old.empty_region_records.iter().any(|s| s.stream == *name)) {
            // This entire family is retained verbatim below, including empty
            // slots; its editable source indices therefore do not move.
            continue;
        }
        let mut seen = BTreeSet::new();
        for (n, i) in items.iter().enumerate() {
            if let Some(src) = &i.source
                && src.stream == *name
            {
                if !seen.insert(src.index) {
                    return Err("Two objects claim the same native source record".into());
                }
                reindexed |= src.index != n as u32;
            }
        }
    }
    if let Some(old) = old_board {
        reindexed |= old.items.iter().any(|i| {
            i.source
                .as_ref()
                .is_some_and(|s| !board.items.iter().any(|n| n.source.as_ref() == Some(s)))
        });
        supported_dependency_guard(old, board, &streams, reindexed)?;
    }
    let mut wide = streams
        .get("/WideStrings6/Data")
        .cloned()
        .unwrap_or_default();
    let wide_map = if wide.is_empty() {
        BTreeMap::new()
    } else {
        wide_strings(&wide)?
    };
    let mut next_wide = wide_map
        .keys()
        .next_back()
        .copied()
        .map_or(Some(0), |n| n.checked_add(1))
        .ok_or("Wide-string index overflow")?;
    let mut expected = Vec::new();
    for (name, tag, block_count) in FAMILIES {
        if let Some(old) = old_board
            && old.empty_region_records.iter().any(|s| s.stream == name)
        {
            let before: Vec<_> = old.items.iter().filter(|i| family(i) == name).collect();
            let after: Vec<_> = board.items.iter().filter(|i| family(i) == name).collect();
            if reindexed || before != after {
                return Err(format!(
                    "Native {name} contains archived empty regions; changing this family or reindexing native records requires PCBCraft JSON"
                ));
            }
            continue;
        }
        let frames = streams
            .get(&format!("/{name}/Data"))
            .map(|b| parse_frames(b, tag, block_count))
            .transpose()?
            .unwrap_or_default();
        let items = selected.get(name).cloned().unwrap_or_default();
        let mut data = Vec::new();
        for (n, item) in items.iter().enumerate() {
            let src = item.source.as_ref().filter(|s| s.stream == name);
            let raw = src.and_then(|s| frames.get(s.index as usize));
            if src.is_some() && raw.is_none() {
                return Err("Native source record is missing from the original archive".into());
            }
            let old = old_board.and_then(|b| {
                src.and_then(|s| b.items.iter().find(|i| i.source.as_ref() == Some(s)))
            });
            let mapped = maps.item(item)?;
            let changed_text = matches!(item.shape, Shape::Text { .. })
                && old.is_none_or(|o| o.shape != item.shape || o.properties != item.properties);
            let frame = modified_frame(&mapped, old, raw, name, next_wide)?;
            if changed_text && let Shape::Text { text, .. } = &item.shape {
                wide.extend_from_slice(&next_wide.to_le_bytes());
                let mut bytes = Vec::new();
                for w in text.encode_utf16() {
                    bytes.extend_from_slice(&w.to_le_bytes());
                }
                bytes.extend_from_slice(&[0, 0]);
                wide.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                if bytes.len() > 2 {
                    wide.extend(bytes);
                }
                next_wide = next_wide
                    .checked_add(1)
                    .ok_or("Wide string index overflow")?;
            }
            data.extend(frame_bytes(&frame)?);
            expected.push((name, n as u32, mapped));
        }
        section(&mut c, &streams, name, &data, items.len())?;
    }
    if !wide.is_empty() {
        section(
            &mut c,
            &streams,
            "WideStrings6",
            &wide,
            wide_strings(&wide)?.len(),
        )?;
    }
    for (family, table, old_table) in [
        ("Classes6", &board.classes, old_board.map(|b| &b.classes)),
        (
            "DifferentialPairs6",
            &board.differential_pairs,
            old_board.map(|b| &b.differential_pairs),
        ),
    ] {
        if old_table == Some(table) {
            continue;
        }
        let raw = streams
            .get(&format!("/{family}/Data"))
            .map(|data| property_blocks(data))
            .transpose()?
            .unwrap_or_default();
        let original = raw.iter().map(|r| fields(r)).collect::<Result<Vec<_>>>()?;
        let mut written = vec![];
        for class in table {
            let identity = |f: &Fields| {
                f.get("NAME") == class.get("NAME") && f.get("KIND") == class.get("KIND")
            };
            let source = original.iter().position(identity);
            let before = source.map(|i| &original[i]);
            let mut changes = map_changes(class);
            if let Some(before) = before {
                changes.retain(|key, value| value.as_ref() != before.get(key));
                for key in before.keys().filter(|key| !class.contains_key(*key)) {
                    changes.insert(key.clone(), None);
                }
            }
            written.push(patch_fields(
                source.map_or(&[][..], |i| raw[i].as_slice()),
                &changes,
            )?);
        }
        section(
            &mut c,
            &streams,
            family,
            &encode_blocks(&written)?,
            written.len(),
        )?;
    }
    if old_board.map(|old| &old.rules) != Some(&board.rules)
        && (!board.rules.is_empty() || old_board.is_some())
    {
        let old_rules = old_board.map_or(&[][..], |b| b.rules.as_slice());
        let raw = streams.get("/Rules6/Data").map_or(&[][..], Vec::as_slice);
        let mut reader = Bytes::new(raw);
        let mut data = Vec::new();
        let mut records = Vec::new();
        while reader.remaining() > 0 {
            let prefix = reader.take(2)?.to_vec();
            records.push((prefix, reader.block()?.to_vec()));
        }
        if records.len() != old_rules.len() {
            return Err("Rule count does not match native stream".into());
        }
        for (index, rule) in board.rules.iter().enumerate() {
            let source = rule_source(old_rules, rule, index)?;
            let template = source.or_else(|| {
                old_rules
                    .iter()
                    .position(|r| r.get("RULEKIND") == rule.get("RULEKIND"))
            });
            let prefix = if let Some(n) = template {
                records[n].0.clone()
            } else {
                native_rule_tag(rule)?.to_le_bytes().to_vec()
            };
            let patched = if let Some(n) = source.filter(|n| old_rules[*n] == *rule) {
                records[n].1.clone()
            } else {
                let mut changes = map_changes(rule);
                if let Some(n) = source {
                    for key in old_rules[n].keys().filter(|k| !rule.contains_key(*k)) {
                        changes.insert(key.clone(), None);
                    }
                }
                patch_fields(
                    source.map_or(&[][..], |n| records[n].1.as_slice()),
                    &changes,
                )?
            };
            data.extend(prefix);
            data.extend_from_slice(&(patched.len() as u32).to_le_bytes());
            data.extend(patched);
        }
        section(&mut c, &streams, "Rules6", &data, board.rules.len())?;
    }
    let old_boards = streams
        .get("/Board6/Data")
        .map(|b| property_blocks(b))
        .transpose()?
        .unwrap_or_default();
    let mut boards = old_boards;
    if boards.is_empty() {
        boards.push(patch_fields(&[], &map_changes(&board.properties))?);
    }
    if old_board.is_none_or(|old| old.outline != board.outline) {
        let mut changes = outline_changes(&boards[0], &board.outline)?;
        changes.insert("RECORD".into(), string("Board"));
        boards[0] = patch_fields(&boards[0], &changes)?;
    }
    section(
        &mut c,
        &streams,
        "Board6",
        &encode_blocks(&boards)?,
        boards.len(),
    )?;
    for family in ["Nets6", "Components6", "Polygons6"] {
        let raw = streams
            .get(&format!("/{family}/Data"))
            .map(|b| property_blocks(b))
            .transpose()?
            .unwrap_or_default();
        let mut records = Vec::new();
        if family == "Nets6" {
            for (index, new_index) in &maps.nets {
                let n = board.nets.iter().find(|n| n.index == *index).unwrap();
                let fallback = patch_fields(&[], &map_changes(&n.properties))?;
                let base = raw.get(*index as usize).unwrap_or(&fallback);
                let mut f = Fields::new();
                f.insert("NAME".into(), n.name.clone());
                f.insert("ID".into(), new_index.to_string());
                records.push(patch_fields(base, &map_changes(&f))?);
            }
        } else if family == "Components6" {
            for (index, new_index) in &maps.components {
                let comp = board.components.iter().find(|n| n.index == *index).unwrap();
                let fallback = patch_fields(&[], &map_changes(&comp.properties))?;
                let base = raw.get(*index as usize).unwrap_or(&fallback);
                let mut f = component_changes(comp);
                // Re-encoding equal fields can change native legacy text and
                // numeric spellings even though our importer reads them alike.
                // A comment ECO must leave the footprint/library/transform bytes
                // intact, including their native Unicode shadow records.
                if let Some(original) =
                    old_board.and_then(|old| old.components.iter().find(|c| c.index == *index))
                {
                    let before = component_changes(original);
                    f.retain(|key, value| before.get(key) != Some(value));
                }
                if raw.get(*index as usize).is_none() || index != new_index {
                    f.insert("ID".into(), new_index.to_string());
                }
                records.push(patch_fields(base, &map_changes(&f))?);
            }
        } else if old_board.is_some_and(|old| old.polygons.len() > old.polygon_pours.len()) {
            records = raw;
        } else {
            for (index, new_index) in &maps.polygons {
                let p = board
                    .polygon_pours
                    .iter()
                    .find(|n| n.index == *index)
                    .unwrap();
                let base = raw.get(*index as usize).map_or(&[][..], Vec::as_slice);
                let mut fields = polygon_fields(p);
                fields.insert("ID".into(), new_index.to_string());
                fields.insert(
                    "NET".into(),
                    p.net
                        .and_then(|n| maps.nets.get(&n).copied())
                        .map_or("-1".into(), |n| n.to_string()),
                );
                let mut changes = map_changes(&fields);
                changes.extend(outline_changes(base, &p.contour)?);
                prune_holes(base, &mut changes)?;
                records.push(patch_fields(base, &changes)?);
            }
        }
        section(
            &mut c,
            &streams,
            family,
            &encode_blocks(&records)?,
            records.len(),
        )?;
    }
    let (parameters, count) = parameter_section(board, &streams, &maps)?;
    if count > 0 || streams.contains_key("/PrimitiveParameters/Data") {
        section(&mut c, &streams, "PrimitiveParameters", &parameters, count)?;
    }
    c.flush().map_err(|e| e.to_string())?;
    let bytes = c.into_inner().into_inner();
    if bytes.len() > LIMIT {
        return Err("Serialized PcbDoc exceeds the import/save limit".into());
    }
    let reread = import_pcb(&bytes, "native-save-validation.PcbDoc")?;
    let actual = reread.pcb.as_ref().unwrap();
    verify_tables(board, actual, Some(&maps))?;
    if actual.items.len() != board.items.len() {
        return Err("Native primitive count changed during save verification".into());
    }
    let actual_records: BTreeMap<_, _> = actual
        .items
        .iter()
        .filter_map(|i| i.source.as_ref().map(|s| ((s.stream.as_str(), s.index), i)))
        .collect();
    for (stream, n, item) in expected {
        let actual = actual_records
            .get(&(stream, n))
            .copied()
            .ok_or("Saved primitive was not readable")?;
        if !equivalent_item(&item, actual) {
            return Err(format!(
                "Native write/read validation failed for {stream} record {n}; destination was not changed"
            ));
        }
    }
    Ok(bytes)
}
fn put_pair(f: &mut Fields, x: &str, y: &str, p: Point) {
    f.insert(x.into(), dim_value(p.x));
    f.insert(y.into(), dim_value(p.y));
}
fn item_fields(i: &Item) -> Result<Fields> {
    let mut f = Fields::new();
    f.insert("RECORD".into(), i.kind().into());
    f.insert("LAYER".into(), layer_token(i.layer));
    f.insert("NET".into(), i.net.map_or("-1".into(), |n| n.to_string()));
    f.insert(
        "COMPONENT".into(),
        i.component.map_or("-1".into(), |n| n.to_string()),
    );
    f.insert(
        "POLYGON".into(),
        i.polygon.map_or("-1".into(), |n| n.to_string()),
    );
    f.insert("LOCKED".into(), boolean(i.locked));
    f.insert("KEEPOUT".into(), boolean(i.keepout));
    match &i.shape {
        Shape::Track { start, end, width } => {
            put_pair(&mut f, "X1", "Y1", *start);
            put_pair(&mut f, "X2", "Y2", *end);
            f.insert("WIDTH".into(), dim_value(*width));
        }
        Shape::Arc {
            centre,
            radius,
            start_angle,
            end_angle,
            width,
        } => {
            put_pair(&mut f, "LOCATION.X", "LOCATION.Y", *centre);
            put_pair(&mut f, "X", "Y", *centre);
            f.insert("RADIUS".into(), dim_value(*radius));
            f.insert("STARTANGLE".into(), start_angle.to_string());
            f.insert("ENDANGLE".into(), end_angle.to_string());
            f.insert("WIDTH".into(), dim_value(*width));
        }
        Shape::Pad {
            position,
            size,
            rotation,
            shape,
            hole,
            plated,
            name,
            bottom_size,
            middle_size,
        } => {
            put_pair(&mut f, "X", "Y", *position);
            put_pair(&mut f, "XSIZE", "YSIZE", *size);
            f.insert("ROTATION".into(), rotation.to_string());
            f.insert("HOLESIZE".into(), dim_value(*hole));
            f.insert("PLATED".into(), boolean(*plated));
            f.insert("NAME".into(), name.clone());
            f.insert(
                "SHAPE".into(),
                match shape {
                    PadShape::Round => "ROUND",
                    PadShape::Rectangle => "RECTANGLE",
                    PadShape::Octagonal => "OCTAGONAL",
                    PadShape::RoundedRectangle => "ROUNDRECT",
                }
                .into(),
            );
            if let Some(s) = bottom_size {
                put_pair(&mut f, "BOTTOMXSIZE", "BOTTOMYSIZE", *s);
            }
            if let Some(s) = middle_size {
                put_pair(&mut f, "MIDXSIZE", "MIDYSIZE", *s);
            }
        }
        Shape::Via {
            position,
            diameter,
            hole,
            from_layer,
            to_layer,
        } => {
            put_pair(&mut f, "X", "Y", *position);
            f.insert("DIAMETER".into(), dim_value(*diameter));
            f.insert("HOLESIZE".into(), dim_value(*hole));
            f.insert("STARTLAYER".into(), layer_token(*from_layer));
            f.insert("ENDLAYER".into(), layer_token(*to_layer));
        }
        Shape::Text {
            position,
            text,
            height,
            stroke,
            rotation,
            mirrored,
            font,
        } => {
            put_pair(&mut f, "X", "Y", *position);
            f.insert("TEXT".into(), text.clone());
            f.insert("HEIGHT".into(), dim_value(*height));
            f.insert("WIDTH".into(), dim_value(*stroke));
            f.insert("ROTATION".into(), rotation.to_string());
            f.insert("MIRROR".into(), boolean(*mirrored));
            f.insert("FONTNAME".into(), font.clone());
            if i.properties.contains_key("DESIGNATOR") {
                f.insert("DESIGNATOR".into(), i.properties["DESIGNATOR"].clone());
            }
            if i.properties.contains_key("COMMENT") {
                f.insert("COMMENT".into(), i.properties["COMMENT"].clone());
            }
        }
        Shape::Fill {
            start,
            end,
            rotation,
        } => {
            put_pair(&mut f, "X1", "Y1", *start);
            put_pair(&mut f, "X2", "Y2", *end);
            f.insert("ROTATION".into(), rotation.to_string());
        }
        Shape::Region {
            contour,
            holes,
            cutout,
        } => {
            if i.properties.contains_key("PCBCRAFT_POLYGON_CUTOUT_OWNER") {
                f.insert("POLYGON".into(), "-1".into());
                f.insert(
                    "PCBCRAFT_POLYGON_CUTOUT_OWNER".into(),
                    i.polygon.ok_or("Cutout has no polygon owner")?.to_string(),
                );
            }
            for (n, p) in contour.iter().enumerate() {
                put_pair(&mut f, &format!("VX{n}"), &format!("VY{n}"), *p);
                f.insert(format!("KIND{n}"), "0".into());
            }
            hole_fields(&mut f, holes);
            let board_cutout = *cutout
                && i.properties
                    .get("ISBOARDCUTOUT")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"));
            f.insert(
                "KIND".into(),
                if *cutout && !board_cutout { "1" } else { "0" }.into(),
            );
            f.insert(
                "ISBOARDCUTOUT".into(),
                boolean(
                    *cutout
                        && i.properties
                            .get("ISBOARDCUTOUT")
                            .is_some_and(|v| v.eq_ignore_ascii_case("TRUE")),
                ),
            );
        }
    }
    Ok(f)
}
fn ascii_item(raw: &[u8], item: &Item) -> Result<Vec<u8>> {
    let mut changes = map_changes(&item_fields(item)?);
    if matches!(item.shape, Shape::Region { .. }) {
        for k in fields(raw)?.keys() {
            if (vertex_key(k) || k.starts_with("HOLE")) && !changes.contains_key(k) {
                changes.insert(k.clone(), None);
            }
        }
    }
    let mut out = patch_fields(raw, &changes)?;
    if out.last() == Some(&0) {
        out.pop();
    }
    Ok(out)
}
fn unicode_record(b: &[u8]) -> String {
    b.split(|c| *c == b'|')
        .map(|s| {
            std::str::from_utf8(s)
                .map(str::to_owned)
                .unwrap_or_else(|_| WINDOWS_1252.decode(s).0.into_owned())
        })
        .collect::<Vec<_>>()
        .join("|")
}
fn encode_ascii(
    board: &Board,
    original: Option<&[u8]>,
    old_board: Option<&Board>,
) -> Result<Vec<u8>> {
    let original = original.unwrap_or(&[]);
    let utf16_source = original.starts_with(&[255, 254]);
    let utf8_bom = original.starts_with(&[239, 187, 191]);
    let source = if utf16_source {
        utf16(&original[2..])?.into_bytes()
    } else {
        original
            .strip_prefix(&[239, 187, 191])
            .unwrap_or(original)
            .to_vec()
    };
    let utf8_source = std::str::from_utf8(&source).is_ok();
    let newline = if source.windows(2).any(|w| w == b"\r\n") {
        b"\r\n".as_slice()
    } else {
        b"\n".as_slice()
    };
    let mut lines = Vec::new();
    let mut done_items = BTreeSet::new();
    let mut done_nets = BTreeSet::new();
    let mut done_comps = BTreeSet::new();
    let mut done_polys = BTreeSet::new();
    let mut logical = 0u32;
    let mut rule_ordinal = 0usize;
    let structural = old_board.is_some_and(|o| {
        o.items.len() != board.items.len()
            || o.items.iter().any(|i| {
                !board
                    .items
                    .iter()
                    .any(|n| n.id == i.id && n.source == i.source)
            })
            || o.components
                .iter()
                .map(|c| c.index)
                .collect::<BTreeSet<_>>()
                != board
                    .components
                    .iter()
                    .map(|c| c.index)
                    .collect::<BTreeSet<_>>()
            || o.nets.iter().map(|n| n.index).collect::<BTreeSet<_>>()
                != board.nets.iter().map(|n| n.index).collect::<BTreeSet<_>>()
            || o.polygon_pours
                .iter()
                .map(|p| p.index)
                .collect::<BTreeSet<_>>()
                != board
                    .polygon_pours
                    .iter()
                    .map(|p| p.index)
                    .collect::<BTreeSet<_>>()
    });
    let mut board_done = false;
    let mut comp_ordinal = 0u32;
    let mut net_ordinal = 0u32;
    let mut poly_ordinal = 0u32;
    for line in source.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let f = fields(line)?;
        if f.is_empty() {
            lines.push(line.to_vec());
            continue;
        }
        let n = logical;
        logical += 1;
        let kind = f.text("RECORD").to_ascii_uppercase();
        let mut changes = BTreeMap::new();
        match kind.as_str() {
            "BOARD" => {
                board_done = true;
                changes = outline_changes(line, &board.outline)?;
            }
            "NET" => {
                let id = unsigned_property(&f, "ID", net_ordinal)?;
                net_ordinal += 1;
                let Some(net) = board.nets.iter().find(|x| x.index == id) else {
                    continue;
                };
                done_nets.insert(id);
                changes.insert("NAME".into(), string(&net.name));
            }
            "COMPONENT" => {
                let id = unsigned_property(&f, "ID", comp_ordinal)?;
                comp_ordinal += 1;
                let Some(c) = board.components.iter().find(|x| x.index == id) else {
                    continue;
                };
                done_comps.insert(id);
                changes = map_changes(&component_changes(c));
                changes.insert("DESIGNATOR".into(), string(&c.designator));
            }
            "RULE" => {
                let rule = board.rules.get(rule_ordinal).ok_or("Rule count changed")?;
                rule_ordinal += 1;
                changes = map_changes(rule);
            }
            "POLYGON" => {
                let id = unsigned_property(&f, "ID", poly_ordinal)?;
                poly_ordinal += 1;
                if let Some(p) = board.polygon_pours.iter().find(|x| x.index == id) {
                    done_polys.insert(id);
                    changes = map_changes(&polygon_fields(p));
                    changes.extend(outline_changes(line, &p.contour)?);
                    prune_holes(line, &mut changes)?;
                } else if old_board.is_some_and(|b| b.polygon_pours.iter().any(|p| p.index == id)) {
                    continue;
                } else {
                    lines.push(line.to_vec());
                    continue;
                }
            }
            "TRACK" | "PAD" | "VIA" | "ARC" | "TEXT" | "FILL" | "REGION" => {
                let item = board.items.iter().find(|i| {
                    i.source
                        .as_ref()
                        .is_some_and(|s| s.stream == "ASCII" && s.index == n)
                });
                if let Some(i) = item {
                    done_items.insert(i.id);
                    lines.push(ascii_item(line, i)?);
                }
                continue;
            }
            _ => {
                if structural
                    && ["PRIMITIVEINDEX", "COMPONENT", "NET", "POLYGON"]
                        .iter()
                        .any(|k| f.contains_key(*k))
                {
                    return Err(format!(
                        "Cannot safely retain dependent ASCII record {kind} after structural edits"
                    ));
                }
                lines.push(line.to_vec());
                continue;
            }
        }
        let mut b = patch_fields(line, &changes)?;
        b.pop();
        lines.push(b);
    }
    if !board_done {
        let mut changes = map_changes(&board.properties);
        changes.insert("RECORD".into(), string("Board"));
        changes.insert("VERSION".into(), string("5.00"));
        changes.extend(outline_changes(&[], &board.outline)?);
        let mut b = patch_fields(&[], &changes)?;
        b.pop();
        lines.insert(0, b);
    }
    for n in &board.nets {
        if done_nets.contains(&n.index) {
            continue;
        }
        let mut f = n.properties.clone();
        f.insert("RECORD".into(), "Net".into());
        f.insert("ID".into(), n.index.to_string());
        f.insert("NAME".into(), n.name.clone());
        let mut b = patch_fields(&[], &map_changes(&f))?;
        b.pop();
        lines.push(b);
    }
    for c in &board.components {
        if done_comps.contains(&c.index) {
            continue;
        }
        let mut f = c.properties.clone();
        f.extend(component_changes(c));
        f.insert("RECORD".into(), "Component".into());
        f.insert("ID".into(), c.index.to_string());
        let mut b = patch_fields(&[], &map_changes(&f))?;
        b.pop();
        lines.push(b);
    }
    for p in &board.polygon_pours {
        if done_polys.contains(&p.index) {
            continue;
        }
        let mut changes = map_changes(&polygon_fields(p));
        changes.extend(outline_changes(&[], &p.contour)?);
        let mut b = patch_fields(&[], &changes)?;
        b.pop();
        lines.push(b);
    }
    for i in &board.items {
        if !done_items.contains(&i.id) {
            lines.push(ascii_item(&[], i)?);
        }
    }
    let mut output = Vec::new();
    for line in lines {
        if utf8_source || utf16_source {
            output.extend_from_slice(unicode_record(&line).as_bytes());
        } else {
            output.extend(line);
        }
        output.extend_from_slice(newline);
    }
    if utf16_source {
        let text = String::from_utf8(output).map_err(|e| e.to_string())?;
        output = vec![255, 254];
        for w in text.encode_utf16() {
            output.extend_from_slice(&w.to_le_bytes());
        }
    } else if utf8_bom {
        output.splice(0..0, [239, 187, 191]);
    }
    let reread = import_pcb(&output, "ascii-save-validation.PcbDoc")?;
    let actual = reread.pcb.as_ref().unwrap();
    if actual.items.len() != board.items.len()
        || actual.components.len() != board.components.len()
        || actual.nets.len() != board.nets.len()
    {
        return Err("ASCII write/read count validation failed".into());
    }
    verify_tables(board, actual, None)?;
    // Compare a multiset, because native stream/record ordering differs from scene ordering.
    let mut used = BTreeSet::new();
    for item in &board.items {
        let found = actual
            .items
            .iter()
            .enumerate()
            .find(|(n, i)| !used.contains(n) && equivalent_item(item, i))
            .map(|(n, _)| n);
        if let Some(n) = found {
            used.insert(n);
        } else {
            return Err(format!(
                "ASCII write/read validation failed for {}",
                item.kind()
            ));
        }
    }
    Ok(output)
}
fn verify_tables(expected: &Board, actual: &Board, maps: Option<&Maps>) -> Result<()> {
    if expected.rules != actual.rules
        || expected.classes != actual.classes
        || expected.differential_pairs != actual.differential_pairs
    {
        return Err("Rule/class table write/read verification failed".into());
    }
    if expected.nets.len() != actual.nets.len()
        || expected.components.len() != actual.components.len()
        || expected.polygon_pours.len() != actual.polygon_pours.len()
    {
        return Err("Native table counts changed during save verification".into());
    }
    let index = |kind: u8, n: u32| {
        maps.map_or(Some(n), |m| match kind {
            0 => m.nets.get(&n).copied(),
            1 => m.components.get(&n).copied(),
            _ => m.polygons.get(&n).copied(),
        })
    };
    for n in &expected.nets {
        if !actual
            .nets
            .iter()
            .any(|a| Some(a.index) == index(0, n.index) && a.name == n.name)
        {
            return Err("Net table write/read verification failed".into());
        }
    }
    for c in &expected.components {
        let a = actual
            .components
            .iter()
            .find(|a| Some(a.index) == index(1, c.index))
            .ok_or("Missing saved component")?;
        for key in [
            "SOURCEUNIQUEID",
            "SOURCESHEET",
            "SOURCEDESCRIPTION",
            "COMPONENTKINDVERSION2",
        ] {
            if let Some(value) = c.properties.get(key)
                && a.properties.get(key) != Some(value)
            {
                return Err(format!(
                    "Component {} field {key} write/read verification failed",
                    c.designator
                ));
            }
        }
        if c.designator != a.designator
            || c.parameters
                .as_ref()
                .is_some_and(|p| a.parameters.as_ref() != Some(p))
            || c.comment != a.comment
            || c.footprint != a.footprint
            || c.layer != a.layer
            || c.locked != a.locked
            || !nearp(c.position, a.position)
            || !angle(c.rotation, a.rotation)
        {
            return Err(format!(
                "Component {} write/read verification failed",
                c.designator
            ));
        }
    }
    if !contour_equivalent(&expected.outline, &actual.outline) {
        return Err("Board outline write/read verification failed".into());
    }
    for p in &expected.polygon_pours {
        let a = actual
            .polygon_pours
            .iter()
            .find(|a| Some(a.index) == index(2, p.index))
            .ok_or("Missing saved polygon")?;
        if p.net.and_then(|n| index(0, n)) != a.net
            || p.name != a.name
            || p.layer != a.layer
            || p.shelved != a.shelved
            || p.locked != a.locked
            || p.priority != a.priority
            || p.connection != a.connection
            || p.clearance != a.clearance
            || p.thermal_gap != a.thermal_gap
            || p.spoke_width != a.spoke_width
            || !angle(p.spoke_angle, a.spoke_angle)
            || p.remove_islands != a.remove_islands
            || (p.min_island_area_mm2 - a.min_island_area_mm2).abs() > 1e-9
            || !contour_equivalent(&p.contour, &a.contour)
            || p.holes.len() != a.holes.len()
            || !p
                .holes
                .iter()
                .zip(&a.holes)
                .all(|(x, y)| ring_equivalent(x, y))
        {
            return Err(format!("Polygon {} write/read verification failed", p.name));
        }
    }
    Ok(())
}
fn contour_equivalent(a: &[ContourVertex], b: &[ContourVertex]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| {
            nearp(a.position, b.position)
                && match (&a.arc, &b.arc) {
                    (None, None) => true,
                    (Some(a), Some(b)) => {
                        nearp(a.centre, b.centre)
                            && near(a.radius, b.radius)
                            && angle(a.start_angle, b.start_angle)
                            && angle(a.end_angle, b.end_angle)
                    }
                    _ => false,
                }
        })
}
fn near(a: i64, b: i64) -> bool {
    a.abs_diff(b) <= 3
}
fn nearp(a: Point, b: Point) -> bool {
    near(a.x, b.x) && near(a.y, b.y)
}
fn angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d < 1e-7 || (360.0 - d) < 1e-7
}
fn ring_equivalent(a: &[Point], b: &[Point]) -> bool {
    let b = if b.len() > 1 && b.first() == b.last() {
        &b[..b.len() - 1]
    } else {
        b
    };
    let a = if a.len() > 1 && a.first() == a.last() {
        &a[..a.len() - 1]
    } else {
        a
    };
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| nearp(*a, *b))
}
fn equivalent_item(a: &Item, b: &Item) -> bool {
    if (a.layer, a.net, a.component, a.polygon, a.locked, a.keepout)
        != (b.layer, b.net, b.component, b.polygon, b.locked, b.keepout)
    {
        return false;
    }
    match (&a.shape, &b.shape) {
        (
            Shape::Track {
                start: a,
                end: b,
                width: c,
            },
            Shape::Track {
                start: x,
                end: y,
                width: z,
            },
        ) => nearp(*a, *x) && nearp(*b, *y) && near(*c, *z),
        (
            Shape::Arc {
                centre: a,
                radius: b,
                start_angle: c,
                end_angle: d,
                width: e,
            },
            Shape::Arc {
                centre: x,
                radius: y,
                start_angle: z,
                end_angle: w,
                width: v,
            },
        ) => nearp(*a, *x) && near(*b, *y) && angle(*c, *z) && angle(*d, *w) && near(*e, *v),
        (
            Shape::Via {
                position: a,
                diameter: b,
                hole: c,
                from_layer: d,
                to_layer: e,
            },
            Shape::Via {
                position: x,
                diameter: y,
                hole: z,
                from_layer: w,
                to_layer: v,
            },
        ) => nearp(*a, *x) && near(*b, *y) && near(*c, *z) && d == w && e == v,
        (
            Shape::Fill {
                start: a,
                end: b,
                rotation: c,
            },
            Shape::Fill {
                start: x,
                end: y,
                rotation: z,
            },
        ) => nearp(*a, *x) && nearp(*b, *y) && angle(*c, *z),
        (
            Shape::Pad {
                position: a,
                size: b,
                rotation: c,
                shape: d,
                hole: e,
                plated: f,
                name: g,
                bottom_size: h,
                middle_size: j,
            },
            Shape::Pad {
                position: x,
                size: y,
                rotation: z,
                shape: w,
                hole: v,
                plated: u,
                name: t,
                bottom_size: s,
                middle_size: r,
            },
        ) => {
            nearp(*a, *x)
                && nearp(*b, *y)
                && angle(*c, *z)
                && d == w
                && near(*e, *v)
                && f == u
                && g == t
                && nearp(h.unwrap_or(*b), s.unwrap_or(*y))
                && nearp(j.unwrap_or(*b), r.unwrap_or(*y))
        }
        (
            Shape::Text {
                position: a,
                text: b,
                height: c,
                stroke: d,
                rotation: e,
                mirrored: f,
                font: g,
            },
            Shape::Text {
                position: x,
                text: y,
                height: z,
                stroke: w,
                rotation: v,
                mirrored: u,
                font: t,
            },
        ) => {
            nearp(*a, *x)
                && b == y
                && near(*c, *z)
                && near(*d, *w)
                && angle(*e, *v)
                && f == u
                && g == t
        }
        (
            Shape::Region {
                contour: a,
                holes: b,
                cutout: c,
            },
            Shape::Region {
                contour: x,
                holes: y,
                cutout: z,
            },
        ) => {
            c == z
                && ring_equivalent(a, x)
                && b.len() == y.len()
                && b.iter().zip(y).all(|(a, b)| ring_equivalent(a, b))
        }
        _ => false,
    }
}
/// Serialize in memory and re-import the result before returning any bytes.
/// Observed Rules6 little-endian family tags in the AD24.4.1 ReferenceG native corpus.
/// Unknown families require an existing native template; no guessed tag is emitted.
fn native_rule_tag(rule: &Fields) -> Result<u16> {
    Ok(
        match rule.get("RULEKIND").map(String::as_str).unwrap_or("") {
            "Clearance" => 0,
            "Width" => 2,
            "RoutingLayers" => 9,
            "RoutingVias" => 11,
            "SolderMaskExpansion" => 13,
            "PasteMaskExpansion" => 14,
            "ShortCircuit" => 15,
            "UnRoutedNet" => 16,
            "MinimumAnnularRing" => 19,
            "PolygonConnect" => 20,
            "HoleSize" => 42,
            "LayerPairs" => 48,
            "HoleToHoleClearance" => 52,
            "MinimumSolderMaskSliver" => 53,
            "SilkToSolderMaskClearance" => 54,
            "UnpouredPolygon" => 62,
            "BoardOutlineClearance" => 63,
            _ => {
                return Err(
                    "No observed native rule tag for this family; keep it in PCBCraft JSON".into(),
                );
            }
        },
    )
}
pub fn serialize(doc: &Document, format: Format) -> Result<Vec<u8>> {
    doc.validate()?;
    if doc.kind != DocumentKind::Board {
        return Err("Native PCB save requires a board document".into());
    }
    let legacy;
    let board = if let Some(board) = &doc.pcb {
        board
    } else {
        legacy = Board::from_legacy(doc);
        &legacy
    };
    if board.layer_stack.is_some() {
        return Err("Edited layer stacks, via types and impedance profiles currently require PCBCraft JSON (Save As); native Altium stack serialization is not implemented".into());
    }
    let original = source_bytes(board)?;
    let old = original
        .as_ref()
        .map(|bytes| {
            import_pcb(
                bytes,
                board
                    .source
                    .as_ref()
                    .map_or("original.PcbDoc", |s| s.filename.as_str()),
            )
        })
        .transpose()?;
    let old_board = old.as_ref().and_then(|d| d.pcb.as_ref());
    if !board.empty_region_records.is_empty() && original.is_none() {
        return Err("Empty native regions require their original source archive".into());
    }
    if old_board.is_some_and(|old| old.empty_region_records != board.empty_region_records) {
        return Err("Archived empty region references were changed; native save refused".into());
    }
    let binary_source = original.as_ref().is_some_and(|b| b.starts_with(MAGIC));
    let binary = match format {
        Format::Binary => true,
        Format::Ascii => false,
        Format::Preserve => original.is_none() || binary_source,
    };
    if original.is_some() && binary != binary_source {
        return Err("Cross-format conversion is disabled: use native Save to preserve the source representation".into());
    }
    if let Some(old) = old_board {
        let mut a = board.clone();
        let mut b = old.clone();
        a.source = None;
        b.source = None;
        a.warnings.clear();
        b.warnings.clear();
        if a == b {
            return Ok(original.unwrap());
        }
        if !binary && (a.classes != b.classes || a.differential_pairs != b.differential_pairs) {
            return Err("Class and differential pair edits currently require binary PcbDoc or PCBCraft JSON".into());
        }
        if a.rules != b.rules {
            if !binary && a.rules.len() != b.rules.len() {
                return Err("Adding/removing ASCII rules requires PCBCraft JSON".into());
            }
            for (n, rule) in a.rules.iter().enumerate() {
                if let Some(source) = rule_source(&b.rules, rule, n)? {
                    if rule != &b.rules[source] {
                        old.replace_design_rule(source, rule.clone())?;
                    }
                } else {
                    if !eda_core::pcb_rules::supported(eda_core::pcb_rules::text(rule, "RULEKIND"))
                    {
                        return Err("Creating this native rule kind is not supported".into());
                    }
                    eda_core::pcb_rules::validate_rule(board, rule)?;
                }
            }
        }
        if b.polygons.len() > b.polygon_pours.len() {
            return Err("Native save refused: at least one source polygon has unsupported geometry. Keep edits in JSON until that polygon can be modelled and repoured safely".into());
        }
        if b.warnings.iter().any(|w| {
            w.contains("unresolved component references") || w.contains("absent from Nets6")
        }) {
            return Err("Native save refused: original unresolved references require repair/interpretation; JSON remains available".into());
        }
    } else if !board.rules.is_empty() {
        if !binary {
            return Err("Creating ASCII rules requires PCBCraft JSON".into());
        }
        for rule in &board.rules {
            native_rule_tag(rule)?;
            eda_core::pcb_rules::validate_rule(board, rule)?;
        }
    }
    for component in &board.components {
        let original_parameters = old_board
            .and_then(|b| b.components.iter().find(|c| c.index == component.index))
            .and_then(|c| c.parameters.as_ref());
        if !binary
            && component.parameters.as_ref().is_some_and(|parameters| {
                !parameters.is_empty()
                    || parameters != original_parameters.unwrap_or(&Fields::new())
            })
        {
            return Err(
                "Changed component parameters require binary PcbDoc or PCBCraft JSON".into(),
            );
        }
    }
    if !binary {
        for item in &board.items {
            let old = old_board.and_then(|b| b.item(item.id));
            if (item.pad_stack.is_some() && old.is_none_or(|o| o.pad_stack != item.pad_stack))
                || (item.masks.is_some() && old.is_none_or(|o| o.masks != item.masks))
            {
                return Err("Advanced padstack/mask edits require binary PcbDoc or PCBCraft JSON; ASCII save refused to avoid data loss".into());
            }
        }
    }
    let rebuilt;
    let board = if board.pours_stale {
        rebuilt = pcb_polygon::repour(board)?;
        &rebuilt
    } else {
        board
    };
    if binary {
        encode_binary(board, original.as_deref(), old_board)
    } else {
        encode_ascii(board, original.as_deref(), old_board)
    }
}
/// A failed serialization never opens the destination for writing. Existing files
/// receive a non-clobbering .bak/.bak.N backup before atomic replacement.
pub fn save(path: &Path, doc: &Document, format: Format) -> Result<()> {
    let bytes = serialize(doc, format)?;
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err("Refusing to overwrite a symlink".into());
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let existing = if path.exists() {
        Some(crate::altium::read_bounded(path)?)
    } else {
        None
    };
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    if let Some(before) = &existing {
        let filename = path
            .file_name()
            .ok_or("Missing output filename")?
            .to_string_lossy();
        let mut saved = false;
        for n in 0..1000 {
            let backup = parent.join(if n == 0 {
                format!("{filename}.bak")
            } else {
                format!("{filename}.bak.{n}")
            });
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&backup)
            {
                Ok(mut f) => {
                    f.write_all(before).map_err(|e| e.to_string())?;
                    f.sync_all().map_err(|e| e.to_string())?;
                    saved = true;
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        if !saved {
            return Err("Cannot allocate a non-overwriting native backup filename".into());
        }
        if std::fs::read(path).map_err(|e| e.to_string())? != *before {
            return Err("Destination changed during save; the new document was not written".into());
        }
        temp.persist(path).map_err(|e| e.to_string())?;
    } else {
        temp.persist_noclobber(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_unicode_rule_comments_are_canonical_and_old_shadows_cannot_override_edits() {
        let raw = b"|COMMENT=legacy|UNICODE=EXISTS|UNICODE__COMMENT=1056,1072,1079,1084,1077,1088|OPAQUE=keep\0";
        let before = fields(raw).unwrap();
        assert_eq!(before["COMMENT"], "Размер");
        assert!(!before.contains_key("UNICODE__COMMENT"));
        let mut changes = BTreeMap::new();
        changes.insert("COMMENT".into(), Some("Отверстие 🟡".into()));
        let output = patch_fields(raw, &changes).unwrap();
        let after = fields(&output).unwrap();
        assert_eq!(after["COMMENT"], "Отверстие 🟡");
        assert_eq!(after["OPAQUE"], "keep");
        changes.insert("COMMENT".into(), Some("Hole".into()));
        let output = patch_fields(&output, &changes).unwrap();
        assert_eq!(fields(&output).unwrap()["COMMENT"], "Hole");
        assert!(!output.windows(16).any(|s| s == b"UNICODE__COMMENT"));
    }
    #[test]
    fn duplicate_native_rule_ids_preserve_untouched_and_named_sources() {
        let mut a = eda_core::pcb_rules::default_rule("Width");
        a.insert("UNIQUEID".into(), "DUPLICATE".into());
        let mut b = a.clone();
        b.insert("NAME".into(), "Another Width".into());
        b.insert("PREFEREDWIDTH".into(), "0.35mm".into());
        let original = vec![a.clone(), b.clone()];
        assert_eq!(rule_source(&original, &b, 1).unwrap(), Some(1));
        assert_eq!(rule_source(&original, &b, 0).unwrap(), Some(1));
        b.insert("COMMENT".into(), "Edited".into());
        assert_eq!(rule_source(&original, &b, 0).unwrap(), Some(1));
        b.insert("NAME".into(), "Renamed".into());
        assert!(rule_source(&original, &b, 1).is_err());
        let mut other = eda_core::pcb_rules::default_rule("Clearance");
        other.insert("UNIQUEID".into(), "DUPLICATE".into());
        other.insert("COMMENT".into(), "Edited".into());
        let original = vec![a, other.clone()];
        other.insert("GAP".into(), "0.3mm".into());
        assert_eq!(rule_source(&original, &other, 0).unwrap(), Some(1));
    }
    fn doc() -> Document {
        let mut d = Document::new(DocumentKind::Board, "test.PcbDoc");
        let mut b = Board::from_legacy(&d);
        b.nets.push(Net {
            index: 0,
            name: "GND".into(),
            properties: Fields::new(),
        });
        b.items.push(Item::new(
            TOP,
            Some(0),
            Shape::Track {
                start: Point::mm(1, 1),
                end: Point::mm(10, 1),
                width: 250_000,
            },
        ));
        d.pcb = Some(b);
        d
    }
    #[test]
    fn binary_directive_class_and_differential_pair_edits_round_trip() {
        let original = serialize(&doc(), Format::Binary).unwrap();
        let mut imported = import_pcb(&original, "classes.PcbDoc").unwrap();
        let b = imported.pcb.as_mut().unwrap();
        b.classes.push(Fields::from([
            ("NAME".into(), "Ground".into()),
            ("KIND".into(), "0".into()),
            ("M0".into(), "GND".into()),
            ("CUSTOM".into(), "retained".into()),
        ]));
        b.differential_pairs.push(Fields::from([
            ("NAME".into(), "DATA".into()),
            ("POSITIVENETNAME".into(), "DATA_P".into()),
            ("NEGATIVENETNAME".into(), "DATA_N".into()),
        ]));
        let classes = b.classes.clone();
        let pairs = b.differential_pairs.clone();
        let saved = serialize(&imported, Format::Preserve).unwrap();
        let mut read = import_pcb(&saved, "classes.PcbDoc").unwrap();
        assert_eq!(read.pcb.as_ref().unwrap().classes, classes);
        assert_eq!(read.pcb.as_ref().unwrap().differential_pairs, pairs);
        read.pcb.as_mut().unwrap().classes[0].remove("M0");
        let bytes = serialize(&read, Format::Preserve).unwrap();
        let read = import_pcb(&bytes, "classes.PcbDoc").unwrap();
        assert_eq!(read.pcb.as_ref().unwrap().classes[0]["CUSTOM"], "retained");
        assert!(!read.pcb.as_ref().unwrap().classes[0].contains_key("M0"));
    }
    #[test]
    fn authored_width_rule_needs_no_source_template_and_keeps_native_family_tag() {
        let mut d = doc();
        let mut rule = eda_core::pcb_rules::default_rule("Width");
        rule.insert("SCOPE1EXPRESSION".into(), "InNet('GND')".into());
        d.pcb.as_mut().unwrap().rules.push(rule.clone());
        let bytes = serialize(&d, Format::Binary).unwrap();
        let mut read = import_pcb(&bytes, "rules.PcbDoc").unwrap();
        assert_eq!(read.pcb.as_ref().unwrap().rules, vec![rule]);
        read.pcb
            .as_mut()
            .unwrap()
            .rules
            .push(eda_core::pcb_rules::default_rule("Clearance"));
        let bytes = serialize(&read, Format::Preserve).unwrap();
        let read = import_pcb(&bytes, "rules.PcbDoc").unwrap();
        assert_eq!(read.pcb.as_ref().unwrap().rules.len(), 2);
    }
    #[test]
    fn coordinate_roundtrip_is_bounded_and_no_wrapping() {
        for n in [-5_000_000_000, -25_400, -1, 0, 1, 25_400, 5_000_000_000] {
            let raw = internal(n).unwrap();
            assert!(near(coord(&raw.to_le_bytes(), 0).unwrap(), n));
        }
        assert!(internal(i64::MAX).is_err());
    }
    #[test]
    fn property_edit_preserves_unknown_bytes_and_duplicates() {
        let raw = b"|KNOWN=old|CUSTOM=one|CUSTOM=two|OPAQUE=\xff\0";
        let mut c = BTreeMap::new();
        c.insert("KNOWN".into(), string("new"));
        let b = patch_fields(raw, &c).unwrap();
        assert_eq!(b, b"|KNOWN=new|CUSTOM=one|CUSTOM=two|OPAQUE=\xff\0");
    }
    #[test]
    fn comment_eco_preserves_native_component_numeric_and_legacy_unicode_bytes() {
        let raw = b"|SOURCEDESIGNATOR=DD1|SOURCECOMMENT=old|UNICODE=EXISTS|PATTERN=\xd2\xc5\xd1\xd2|UNICODE__PATTERN=1058,1045,1057,1058|SOURCEFOOTPRINTLIBRARY=Y:\\lib\\\xcd.PcbLib|UNICODE__SOURCEFOOTPRINTLIBRARY=89,58,92,108,105,98,92,1053,46,80,99,98,76,105,98|ROTATION= 2.70000000000000E+0002|X=3679.6063mil|Y=3562.4803mil|HEIGHT=62.9921mil|LAYER=TOP|LOCKED=TRUE|CUSTOM=one|CUSTOM=two\0";
        let mut d = doc();
        d.pcb
            .as_mut()
            .unwrap()
            .components
            .push(component(fields(raw).unwrap(), d.id, 0).unwrap());
        let bytes = serialize(&d, Format::Binary).unwrap();
        let mut native = cfb::CompoundFile::open(Cursor::new(bytes)).unwrap();
        write_stream(
            &mut native,
            "/Components6/Data",
            &encode_blocks(&[raw.to_vec()]).unwrap(),
        )
        .unwrap();
        let bytes = native.into_inner().into_inner();
        let mut imported = import_pcb(&bytes, "native.PcbDoc").unwrap();
        imported.pcb.as_mut().unwrap().components[0].comment = "new comment".into();
        let edited = serialize(&imported, Format::Preserve).unwrap();
        let streams = read_streams(&edited).unwrap();
        let records = property_blocks(&streams["/Components6/Data"]).unwrap();
        assert_eq!(
            records[0],
            patch_fields(
                raw,
                &BTreeMap::from([("SOURCECOMMENT".into(), Some("new comment".into()))])
            )
            .unwrap()
        );
        // An unrelated geometry edit must not rewrite any component field.
        let mut imported = import_pcb(&bytes, "native.PcbDoc").unwrap();
        if let Shape::Track { end, .. } = &mut imported.pcb.as_mut().unwrap().items[0].shape {
            *end = Point::mm(15, 3);
        }
        let edited = serialize(&imported, Format::Preserve).unwrap();
        let streams = read_streams(&edited).unwrap();
        assert_eq!(
            property_blocks(&streams["/Components6/Data"]).unwrap()[0],
            raw
        );
    }
    #[test]
    fn binary_create_edit_save_reopen_and_untouched_exact() {
        let d = doc();
        let bytes = serialize(&d, Format::Binary).unwrap();
        assert!(bytes.starts_with(MAGIC));
        let mut imported = import_pcb(&bytes, "board.PcbDoc").unwrap();
        assert_eq!(serialize(&imported, Format::Preserve).unwrap(), bytes);
        if let Shape::Track { end, .. } = &mut imported.pcb.as_mut().unwrap().items[0].shape {
            *end = Point::mm(15, 3);
        }
        let changed = serialize(&imported, Format::Preserve).unwrap();
        assert_ne!(changed, bytes);
        let reread = import_pcb(&changed, "edited.PcbDoc").unwrap();
        assert!(equivalent_item(
            &imported.pcb.unwrap().items[0],
            &reread.pcb.unwrap().items[0]
        ));
    }
    #[test]
    fn ascii_remains_ascii_with_actual_edits() {
        let a = serialize(&doc(), Format::Ascii).unwrap();
        let mut d = import_pcb(&a, "ascii.PcbDoc").unwrap();
        d.pcb.as_mut().unwrap().items.clear();
        let b = serialize(&d, Format::Preserve).unwrap();
        assert!(!b.starts_with(MAGIC));
        assert!(
            import_pcb(&b, "b.PcbDoc")
                .unwrap()
                .pcb
                .unwrap()
                .items
                .is_empty()
        );
    }
    #[test]
    fn binary_unknown_streams_survive_edit() {
        let b = serialize(&doc(), Format::Binary).unwrap();
        let mut c = cfb::CompoundFile::open(Cursor::new(b)).unwrap();
        write_stream(&mut c, "/VendorData/Blob", b"opaque\0payload").unwrap();
        c.flush().unwrap();
        let bytes = c.into_inner().into_inner();
        let mut d = import_pcb(&bytes, "a.PcbDoc").unwrap();
        if let Shape::Track { width, .. } = &mut d.pcb.as_mut().unwrap().items[0].shape {
            *width = 400_000;
        }
        let out = serialize(&d, Format::Preserve).unwrap();
        assert_eq!(
            read_streams(&out).unwrap()["/VendorData/Blob"],
            b"opaque\0payload"
        );
    }
    #[test]
    fn overwrite_makes_backup_and_failure_is_atomic() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("board.PcbDoc");
        let d = doc();
        save(&p, &d, Format::Binary).unwrap();
        let first = std::fs::read(&p).unwrap();
        let mut d = import_pcb(&first, "board.PcbDoc").unwrap();
        d.pcb.as_mut().unwrap().items.clear();
        save(&p, &d, Format::Preserve).unwrap();
        assert_eq!(
            std::fs::read(t.path().join("board.PcbDoc.bak")).unwrap(),
            first
        );
        let current = std::fs::read(&p).unwrap();
        d.schema_version = 999;
        assert!(save(&p, &d, Format::Preserve).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), current);
    }
    #[test]
    fn mixed_primitives_and_polygon_roundtrip() {
        let mut d = doc();
        let b = d.pcb.as_mut().unwrap();
        b.items.push(Item::new(
            MULTI_LAYER,
            Some(0),
            Shape::Via {
                position: Point::mm(4, 4),
                diameter: 800_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        b.items.push(Item::new(
            TOP,
            Some(0),
            Shape::Pad {
                position: Point::mm(3, 3),
                size: Point::mm(2, 1),
                rotation: 90.0,
                shape: PadShape::Round,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        ));
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Region {
                contour: vec![Point::mm(1, 1), Point::mm(3, 1), Point::mm(3, 3)],
                holes: vec![],
                cutout: false,
            },
        ));
        b.polygon_pours.push(Polygon {
            net: Some(0),
            contour: b.outline.clone(),
            ..Default::default()
        });
        b.pours_stale = true;
        let data = serialize(&d, Format::Binary).unwrap();
        let imported = import_pcb(&data, "a.PcbDoc").unwrap();
        assert_eq!(imported.pcb.unwrap().polygon_pours.len(), 1);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    fn source() -> Document {
        import_pcb(b"|RECORD=Board|VX0=0mm|VY0=0mm|VX1=20mm|VY1=0mm|VX2=20mm|VY2=20mm|VX3=0mm|VY3=20mm\n|RECORD=Net|ID=0|NAME=GND\n|RECORD=Polygon|ID=0|NAME=GND pour|NET=0|LAYER=TOP|VX0=1mm|VY0=1mm|VX1=19mm|VY1=1mm|VX2=19mm|VY2=19mm|VX3=1mm|VY3=19mm\n","polygon.PcbDoc").unwrap()
    }
    #[test]
    fn removing_last_polygon_survives_json_reload() {
        let mut d = source();
        let b = d.pcb.as_ref().unwrap();
        let id = b.polygon_pours[0].id;
        d.pcb = Some(b.edited(Edit::Delete(vec![id])).unwrap());
        let t = tempfile::tempdir().unwrap();
        let path = t.path().join("edited.json");
        crate::save(&path, &d).unwrap();
        assert!(
            crate::load(&path)
                .unwrap()
                .pcb
                .unwrap()
                .polygon_pours
                .is_empty()
        );
    }
    #[test]
    fn cutout_is_native_region_and_deleting_it_removes_stale_fields() {
        let mut d = source();
        let b = d.pcb.as_mut().unwrap();
        b.polygon_pours[0].holes.push(vec![
            Point::mm(4, 4),
            Point::mm(7, 4),
            Point::mm(7, 7),
            Point::mm(4, 7),
        ]);
        b.pours_stale = true;
        let bytes = serialize(&d, Format::Preserve).unwrap();
        let mut d = import_pcb(&bytes, "p.PcbDoc").unwrap();
        let b = d.pcb.as_mut().unwrap();
        assert_eq!(b.polygon_pours[0].holes.len(), 1);
        assert!(
            b.items
                .iter()
                .any(|i| matches!(i.shape, Shape::Region { cutout: true, .. }))
        );
        b.polygon_pours[0].holes.clear();
        b.pours_stale = true;
        let bytes = serialize(&d, Format::Preserve).unwrap();
        let b = import_pcb(&bytes, "p.PcbDoc").unwrap().pcb.unwrap();
        assert!(b.polygon_pours[0].holes.is_empty());
        assert!(
            !b.items
                .iter()
                .any(|i| matches!(i.shape, Shape::Region { cutout: true, .. }))
        );
    }
    #[test]
    fn unsupported_reference_reindex_is_rejected() {
        let mut streams = BTreeMap::new();
        streams.insert("/ExtendedPrimitiveInformation/Data".into(), vec![1]);
        assert!(
            supported_dependency_guard(&Board::default(), &Board::default(), &streams, true)
                .is_err()
        );
    }
    #[test]
    fn nonascii_component_and_pad_geometry_survive_ascii_edit() {
        let source=b"|RECORD=Board|VX0=0mm|VY0=0mm|VX1=10mm|VY1=0mm|VX2=10mm|VY2=10mm\n|RECORD=Component|ID=0|SOURCEDESIGNATOR=U1|SOURCECOMMENT=old|X=1mm|Y=1mm|LAYER=TOP\n|RECORD=Pad|NAME=1|X=1mm|Y=1mm|XSIZE=2mm|YSIZE=1mm|COMPONENT=0|LAYER=MULTILAYER|HOLESIZE=0.5mm|MIDXSIZE=1mm|MIDYSIZE=1mm|BOTTOMXSIZE=1.5mm|BOTTOMYSIZE=1mm|SHAPE=ROUNDRECT\n";
        let mut d = import_pcb(source, "source.PcbDoc").unwrap();
        d.pcb.as_mut().unwrap().components[0].comment = "тест µ".into();
        let bytes = serialize(&d, Format::Preserve).unwrap();
        let b = import_pcb(&bytes, "edited.PcbDoc").unwrap().pcb.unwrap();
        assert_eq!(b.components[0].comment, "тест µ");
    }
    #[test]
    fn truncated_native_records_are_rejected_without_panic() {
        for n in 0..32 {
            let bytes = vec![4; n];
            assert!(parse_frames(&bytes, 4, 1).is_err() || n == 0);
        }
    }
    #[test]
    fn moving_rounded_pad_preserves_native_custom_radius() {
        let old = Item::new(
            TOP,
            None,
            Shape::Pad {
                position: Point::mm(1, 1),
                size: Point::mm(2, 1),
                rotation: 0.0,
                shape: PadShape::RoundedRectangle,
                hole: 0,
                plated: false,
                name: "1".into(),
                bottom_size: None,
                middle_size: None,
            },
        );
        let mut raw = modified_frame(&old, None, None, "Pads6", 0).unwrap();
        raw.blocks[5][564..596].fill(40);
        let mut edited = old.clone();
        if let Shape::Pad { position, .. } = &mut edited.shape {
            position.x += 1_000_000;
        }
        let out = modified_frame(&edited, Some(&old), Some(&raw), "Pads6", 0).unwrap();
        assert!(out.blocks[5][564..596].iter().all(|v| *v == 40));
    }
    #[test]
    fn modified_region_retains_opaque_tail() {
        let old = Item::new(
            TOP,
            None,
            Shape::Region {
                contour: vec![Point::mm(1, 1), Point::mm(5, 1), Point::mm(5, 5)],
                holes: vec![],
                cutout: false,
            },
        );
        let mut raw = modified_frame(&old, None, None, "ShapeBasedRegions6", 0).unwrap();
        raw.blocks[0].extend_from_slice(b"opaque-extension");
        let mut edited = old.clone();
        if let Shape::Region { contour, .. } = &mut edited.shape {
            for p in contour {
                p.x += 1_000_000;
            }
        }
        let out = modified_frame(&edited, Some(&old), Some(&raw), "ShapeBasedRegions6", 0).unwrap();
        assert!(out.blocks[0].ends_with(b"opaque-extension"));
    }
}
