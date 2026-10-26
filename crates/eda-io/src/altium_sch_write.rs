//! Lossless-container writing of schematic ECO properties. Geometry is verified,
//! not reconstructed from an incomplete native format description.
use super::*;
use eda_core::schematic::SchPrimitiveKind as K;
use std::io::Write;

#[cfg(test)]
#[path = "altium_sch_write_tests.rs"]
mod tests;

struct Patch {
    records: Vec<Option<Fields>>,
    added: Vec<Fields>,
    namespace: Uuid,
    used_uids: BTreeSet<String>,
}

fn insertion_points(source: &Schematic, patch: &Patch) -> Result<Vec<usize>, String> {
    let count = source.records.len();
    let header_count = source
        .records
        .iter()
        .take_while(|r| r.stream == "FileHeader")
        .count();
    let mut ends: Vec<_> = (1..=count).collect();
    for r in source.records.iter().rev() {
        if let Some(owner) = r
            .fields
            .get("OWNERINDEX")
            .and_then(|v| v.parse::<usize>().ok())
            && owner < r.index as usize
        {
            ends[owner] = ends[owner].max(ends[r.index as usize]);
        }
    }
    let mut points = vec![];
    for (i, f) in patch.added.iter().enumerate() {
        let owner = f
            .get("OWNERINDEX")
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or("Invalid authored owner")?;
        let point = if owner < count {
            ends[owner].min(header_count)
        } else {
            *points
                .get(owner - count)
                .filter(|_| owner - count < i)
                .ok_or("Forward authored owner")?
        };
        points.push(point);
    }
    Ok(points)
}
impl Patch {
    fn fresh_uid(&mut self, owner: u32, kind: u32, name: &str) -> Result<String, String> {
        for salt in 0..=MAX_RECORDS {
            let key = format!("{owner}/{kind}/{name}/{salt}");
            let uid = Uuid::new_v5(&self.namespace, key.as_bytes())
                .simple()
                .to_string()[..8]
                .to_uppercase();
            if self.used_uids.insert(uid.clone()) {
                return Ok(uid);
            }
        }
        Err("Native schematic identity allocation limit exceeded".into())
    }
    fn record(&mut self, index: u32) -> Result<&mut Fields, String> {
        self.records
            .get_mut(index as usize)
            .and_then(Option::as_mut)
            .ok_or_else(|| format!("Missing native schematic record {index}"))
    }
    fn add(&mut self, kind: u32, owner: u32) -> u32 {
        let index = (self.records.len() + self.added.len()) as u32;
        self.added.push(Fields::from([
            ("RECORD".into(), kind.to_string()),
            ("OWNERINDEX".into(), owner.to_string()),
            ("OWNERPARTID".into(), "-1".into()),
        ]));
        index
    }
    fn remove_tree(&mut self, index: usize) {
        let mut removed = BTreeSet::from([index]);
        for (i, f) in self.records.iter_mut().enumerate().skip(index) {
            if i == index
                || f.as_ref().is_some_and(|f| {
                    f.get("OWNERINDEX")
                        .and_then(|v| v.parse::<usize>().ok())
                        .is_some_and(|p| removed.contains(&p))
                })
            {
                *f = None;
                removed.insert(i);
            }
        }
    }
}

fn set(f: &mut Fields, key: &str, value: String) {
    f.remove(&format!("%UTF8%{key}"));
    f.insert(key.into(), value);
}
fn boolean(value: bool) -> String {
    if value { "T" } else { "F" }.into()
}

fn parameters(
    patch: &mut Patch,
    source: &Schematic,
    owner: u32,
    old: &Fields,
    next: &Fields,
) -> Result<(), String> {
    if next
        .iter()
        .any(|(name, value)| value.contains(['\r', '\n']) && old.get(name) != Some(value))
    {
        return Err(
            "Native multiline parameter writing is not verified; choose PCBCraft JSON".into(),
        );
    }
    for (name, value) in old {
        let target = next.iter().find(|(k, _)| k.eq_ignore_ascii_case(name));
        if target.is_some_and(|(k, v)| k == name && v == value) {
            continue;
        }
        for r in source.records.iter().filter(|r| {
            r.stream == "FileHeader"
                && r.record_type == 41
                && r.fields.n("OWNERINDEX") == owner as i64
                && r.fields.text("NAME").eq_ignore_ascii_case(name)
        }) {
            if let Some((key, value)) = target {
                let f = patch.record(r.index)?;
                set(f, "NAME", key.clone());
                set(f, "TEXT", value.replace('\n', "~1"));
            } else {
                patch.remove_tree(r.index as usize);
            }
        }
    }
    for (name, value) in next {
        if old.keys().any(|k| k.eq_ignore_ascii_case(name)) {
            continue;
        }
        patch.add(41, owner);
        let uid = patch.fresh_uid(owner, 41, name)?;
        let f = patch.added.last_mut().unwrap();
        set(f, "NAME", name.clone());
        set(f, "TEXT", value.replace('\n', "~1"));
        set(f, "ISHIDDEN", "T".into());
        set(f, "UNIQUEID", uid);
    }
    Ok(())
}

fn models(
    patch: &mut Patch,
    source: &Schematic,
    old: &Component,
    next: &Component,
) -> Result<(), String> {
    if old.settings.models == next.settings.models {
        return Ok(());
    }
    let originals: Vec<_> = source
        .records
        .iter()
        .filter(|r| {
            r.record_type == 45
                && component_owner(&source.records, r.index as usize)
                    == Some(old.source_index as usize)
        })
        .collect();
    // Keep the native model record, maps and library references when possible.
    let mut used = BTreeSet::new();
    let mut last = None;
    for model in &next.settings.models {
        let found = originals.iter().find(|r| {
            !used.contains(&r.index)
                && r.fields.text("MODELNAME") == model.name
                && r.fields.text("MODELTYPE") == model.kind
        });
        if let Some(r) = found {
            if last.is_some_and(|n| r.index < n) {
                return Err(
                    "Native model reordering is not supported; choose PCBCraft JSON".into(),
                );
            }
            last = Some(r.index);
            used.insert(r.index);
            set(patch.record(r.index)?, "ISCURRENT", boolean(model.current));
        } else {
            let list = source.records.iter().find(|r| {
                r.record_type == 44 && r.fields.n("OWNERINDEX") == old.source_index as i64
            });
            let owner =
                if let Some(r) = list {
                    r.index
                } else if let Some((i, _)) = patch.added.iter().enumerate().find(|(_, f)| {
                    f.n("RECORD") == 44 && f.n("OWNERINDEX") == old.source_index as i64
                }) {
                    (patch.records.len() + i) as u32
                } else {
                    patch.add(44, old.source_index)
                };
            let model_index = patch.add(45, owner);
            let uid = patch.fresh_uid(
                old.source_index,
                45,
                &format!("{}/{}", model.kind, model.name),
            )?;
            let f = patch.added.last_mut().unwrap();
            set(f, "MODELNAME", model.name.clone());
            set(f, "MODELTYPE", model.kind.clone());
            set(f, "ISCURRENT", boolean(model.current));
            set(f, "DATAFILECOUNT", "1".into());
            set(f, "MODELDATAFILEENTITY0", model.name.clone());
            set(
                f,
                "MODELDATAFILEKIND0",
                if model.kind.eq_ignore_ascii_case("PCBLIB") {
                    "PCBLib"
                } else {
                    &model.kind
                }
                .into(),
            );
            set(f, "UNIQUEID", uid);
            patch.add(46, model_index);
            patch.add(48, model_index);
        }
    }
    for r in originals {
        if !used.contains(&r.index) {
            patch.remove_tree(r.index as usize);
        }
    }
    Ok(())
}

fn changes(original: &Document, doc: &Document) -> Result<Patch, String> {
    let source = original.schematic.as_ref().unwrap();
    let target = doc.schematic.as_ref().ok_or("Expected a schematic")?;
    if target.records != source.records || target.source != source.source {
        return Err("Native schematic archive or record identities changed".into());
    }
    let mut patch = Patch {
        records: source
            .records
            .iter()
            .map(|r| Some(r.fields.clone()))
            .collect(),
        added: vec![],
        namespace: original.id,
        used_uids: source
            .records
            .iter()
            .map(|r| r.fields.text("UNIQUEID").to_uppercase())
            .filter(|v| !v.is_empty())
            .collect(),
    };
    if source.components.len() != target.components.len() {
        return Err(
            "Native symbol insertion/removal is not supported; choose PCBCraft JSON".into(),
        );
    }
    for c in &target.components {
        let old = source
            .components
            .iter()
            .find(|v| v.id == c.id)
            .ok_or("Native component identity changed")?;
        let f = patch.record(old.source_index)?;
        if old.description != c.description {
            set(f, "COMPONENTDESCRIPTION", c.description.clone());
        }
        if old.settings.kind != c.settings.kind {
            set(f, "COMPONENTKIND", c.settings.kind.to_string());
        }
        if old.designator != c.designator {
            let suffix = char::from_u32('A' as u32 + c.part - 1).ok_or("Invalid component part")?;
            let designator = if c.settings.part_count > 1 {
                c.designator
                    .strip_suffix(suffix)
                    .ok_or("Multipart designator has no unit suffix")?
            } else {
                &c.designator
            };
            let records: Vec<_> = source
                .records
                .iter()
                .filter(|r| {
                    r.record_type == 34 && r.fields.n("OWNERINDEX") == old.source_index as i64
                })
                .collect();
            if records.is_empty() {
                return Err("Native component has no designator object".into());
            }
            for r in records {
                set(patch.record(r.index)?, "TEXT", designator.into());
            }
        }
        parameters(
            &mut patch,
            source,
            old.source_index,
            &old.parameters,
            &c.parameters,
        )?;
        models(&mut patch, source, old, c)?;
    }
    parameters(
        &mut patch,
        source,
        0,
        &source.sheet.parameters,
        &target.sheet.parameters,
    )?;
    for p in &target.primitives {
        let Some(old) = source.primitives.iter().find(|v| v.id == p.id) else {
            continue;
        };
        if old.kind == p.kind || p.component.is_some() {
            continue;
        }
        let text = match (&old.kind, &p.kind) {
            (K::Text { .. }, K::Text { text, .. }) if p.presentation.record_type == 25 => text,
            (K::PowerPort { .. }, K::PowerPort { text, .. }) => text,
            (K::Port { .. }, K::Port { name, .. }) => name,
            _ => continue,
        };
        set(patch.record(p.source_index)?, "TEXT", text.clone());
    }
    Ok(patch)
}

fn properties(
    raw: &[u8],
    before: &Fields,
    after: &Fields,
    encoding: &'static Encoding,
) -> Result<Vec<u8>, String> {
    let keys: BTreeSet<_> = before
        .keys()
        .chain(after.keys())
        .filter(|k| before.get(*k) != after.get(*k))
        .cloned()
        .collect();
    if keys.is_empty() {
        return Ok(raw.to_vec());
    }
    let mut out = vec![];
    for segment in raw.strip_suffix(&[0]).unwrap_or(raw).split(|b| *b == b'|') {
        if segment.is_empty() {
            continue;
        }
        let key = segment.split(|b| *b == b'=').next().unwrap();
        let key = std::str::from_utf8(key)
            .map_err(|_| "Invalid property key")?
            .to_ascii_uppercase();
        if keys.contains(&key) || keys.contains(key.strip_prefix("%UTF8%").unwrap_or(&key)) {
            continue;
        }
        out.push(b'|');
        out.extend(segment);
    }
    // RECORD must lead a newly authored native record.
    let ordered = keys.iter().filter(|k| k.as_str() == "RECORD").chain(
        keys.iter()
            .filter(|k| k.as_str() != "RECORD" && !k.starts_with("%UTF8%")),
    );
    for key in ordered {
        let Some(value) = after.get(key) else {
            continue;
        };
        if key.contains(['|', '=', '\0', '\r', '\n']) || value.contains(['|', '\0', '\r', '\n']) {
            return Err(
                "Native schematic properties cannot contain separators or control characters"
                    .into(),
            );
        }
        out.push(b'|');
        out.extend(key.as_bytes());
        out.push(b'=');
        out.extend(encoding.encode(value).0.as_ref());
        if !value.is_ascii() {
            out.extend(b"|%UTF8%");
            out.extend(key.as_bytes());
            out.push(b'=');
            out.extend(value.as_bytes());
        }
    }
    out.push(0);
    Ok(out)
}

fn verify(
    original: &Document,
    doc: &Document,
    bytes: &[u8],
    old_indices: &[Option<u32>],
) -> Result<(), String> {
    let mut actual = import_schematic(bytes, &doc.name)?;
    let source = original.schematic.as_ref().unwrap();
    let scene = actual.schematic.as_mut().unwrap();
    let mut ids = BTreeMap::new();
    for c in &mut scene.components {
        let index = old_indices
            .get(c.source_index as usize)
            .copied()
            .flatten()
            .ok_or("Unexpected serialized component")?;
        let old = source
            .components
            .iter()
            .find(|c| c.source_index == index)
            .ok_or("Unexpected native component")?;
        ids.insert(c.id, old.id);
        c.id = old.id;
        c.source_index = index;
    }
    for primitives in [&mut scene.primitives, &mut scene.parameter_objects] {
        // Newly introduced base parameters are hidden native objects; the editable
        // model can hold their values without an existing presentation object.
        primitives.retain(|p| {
            old_indices
                .get(p.source_index as usize)
                .is_some_and(Option::is_some)
        });
        for p in primitives {
            let index = old_indices[p.source_index as usize].unwrap();
            let old = source
                .primitives
                .iter()
                .chain(&source.parameter_objects)
                .find(|p| p.source_index == index)
                .ok_or("Unexpected serialized primitive")?;
            p.id = old.id;
            p.source_index = index;
            p.component = p
                .component
                .map(|id| ids.get(&id).copied().ok_or("Missing component owner"))
                .transpose()?;
        }
    }
    actual.id = doc.id;
    let mut expected = doc.clone();
    for d in [&mut actual, &mut expected] {
        let s = d.schematic.as_mut().unwrap();
        s.source = None;
        s.records = Default::default();
        s.warnings.clear();
        s.source_modified = false;
    }
    if actual != expected {
        return Err("Native SchDoc verification found unsupported scene changes; choose PCBCraft JSON. The destination was not modified".into());
    }
    Ok(())
}

/// Serialize supported ECO edits while preserving the original container and all
/// untouched record payloads. Unsupported edits fail before touching a destination.
pub fn serialize(doc: &Document) -> Result<Vec<u8>, String> {
    doc.validate()?;
    let s = doc
        .schematic
        .as_ref()
        .ok_or("Expected a schematic document")?;
    let source = s
        .source
        .as_ref()
        .filter(|s| s.format == "Altium SchDoc")
        .ok_or("A native SchDoc source archive is required; choose PCBCraft JSON")?;
    let bytes = STANDARD
        .decode(&source.original_base64)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_FILE {
        return Err("Native schematic exceeds save limit".into());
    }
    let original = import_schematic(&bytes, &doc.name)?;
    if original.id != doc.id {
        return Err("Native schematic document identity changed".into());
    }
    let mut unchanged = doc.clone();
    unchanged.schematic.as_mut().unwrap().source_modified = false;
    if unchanged == original {
        return Ok(bytes);
    }
    let patch = changes(&original, doc)?;
    let binary = bytes.starts_with(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
    let mut raw = vec![];
    let mut compound = if binary {
        Some(cfb::CompoundFile::open(Cursor::new(bytes.clone())).map_err(|e| e.to_string())?)
    } else {
        None
    };
    if let Some(file) = &mut compound {
        for name in ["FileHeader", "Additional"] {
            if !file.is_stream(name) {
                continue;
            }
            let mut data = vec![];
            file.open_stream(name)
                .map_err(|e| e.to_string())?
                .read_to_end(&mut data)
                .map_err(|e| e.to_string())?;
            framed(name, &data, &mut raw)?;
        }
    } else {
        for line in bytes
            .strip_prefix(&[0xef, 0xbb, 0xbf])
            .unwrap_or(&bytes)
            .split(|b| *b == b'\n')
        {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if !line.is_empty() {
                raw.push(("FileHeader", 0, [line, &[0]].concat()));
            }
        }
    }
    let encoding = detect_encoding(&raw);
    let mut record = 0;
    let mut output = BTreeMap::<&str, Vec<(u8, Vec<u8>, Option<u32>)>>::new();
    for (stream, flag, data) in raw {
        let before = if flag == 0 {
            decode_fields(&data, encoding)?
        } else {
            Fields::new()
        };
        if before.contains_key("HEADER") {
            let mut after = before.clone();
            if stream == "FileHeader" {
                let retained = original
                    .schematic
                    .as_ref()
                    .unwrap()
                    .records
                    .iter()
                    .enumerate()
                    .filter(|(i, r)| r.stream == "FileHeader" && patch.records[*i].is_some())
                    .count();
                set(
                    &mut after,
                    "WEIGHT",
                    (retained + patch.added.len()).to_string(),
                );
            }
            output.entry(stream).or_default().push((
                flag,
                properties(&data, &before, &after, encoding)?,
                None,
            ));
            continue;
        }
        if let Some(after) = &patch.records[record] {
            output.entry(stream).or_default().push((
                flag,
                properties(&data, &before, after, encoding)?,
                Some(record as u32),
            ));
        }
        record += 1;
    }
    for (added, f) in patch.added.iter().enumerate() {
        let index = (record + added) as u32;
        output.entry("FileHeader").or_default().push((
            0,
            properties(&[], &Fields::new(), f, encoding)?,
            Some(index),
        ));
    }
    // Altium consumes component/model children in their native tree order.
    // OWNERINDEX alone is insufficient when new records are appended globally.
    let points = insertion_points(original.schematic.as_ref().unwrap(), &patch)?;
    output
        .entry("FileHeader")
        .or_default()
        .sort_by_key(|(_, _, index)| match index {
            None => (0, 0, 0),
            Some(i) if (*i as usize) < record => (*i as usize, 1, 0),
            Some(i) => (points[*i as usize - record], 0, *i),
        });
    let mut remap = BTreeMap::new();
    let mut old_indices = vec![];
    for stream in ["FileHeader", "Additional"] {
        for (_, _, index) in output.get(stream).into_iter().flatten() {
            if let Some(index) = index {
                remap.insert(*index, old_indices.len() as u32);
                old_indices.push((*index < record as u32).then_some(*index));
            }
        }
    }
    let shifted = remap.iter().any(|(a, b)| a != b);
    if shifted
        && original
            .schematic
            .as_ref()
            .unwrap()
            .records
            .iter()
            .any(|r| {
                r.stream != "FileHeader"
                    || r.record_type == u32::MAX
                    || !matches!(r.record_type, 1..=18|22|25..=34|37|39|41|43..=48)
            })
    {
        return Err(
            "Opaque schematic records prevent safe owner-index remapping; choose PCBCraft JSON"
                .into(),
        );
    }
    for stream in ["FileHeader", "Additional"] {
        for (flag, data, index) in output.get_mut(stream).into_iter().flatten() {
            if index.is_none() {
                continue;
            }
            if *flag != 0 {
                continue;
            }
            let before = decode_fields(data, encoding)?;
            let mut after = before.clone();
            if let Some(owner) = before.get("OWNERINDEX").and_then(|n| n.parse::<u32>().ok()) {
                set(
                    &mut after,
                    "OWNERINDEX",
                    remap
                        .get(&owner)
                        .ok_or("Native record references a removed owner")?
                        .to_string(),
                );
            }
            if shifted
                && let Some(key) = after.keys().find(|k| {
                    k.contains("INDEX")
                        && !matches!(k.as_str(), "OWNERINDEX" | "INDEXINSHEET" | "UNIONINDEX")
                })
            {
                return Err(format!(
                    "Opaque schematic index dependency {key}={} prevents native save; choose PCBCraft JSON",
                    after[key]
                ));
            }
            *data = properties(data, &before, &after, encoding)?;
        }
    }
    let mut result = vec![];
    if let Some(mut file) = compound {
        for (name, blocks) in output {
            let mut stream = file.create_stream(name).map_err(|e| e.to_string())?;
            for (flag, data, _) in blocks {
                if data.len() > 0xffffff {
                    return Err("Native record exceeds framing limit".into());
                }
                stream
                    .write_all(&((flag as u32) << 24 | data.len() as u32).to_le_bytes())
                    .map_err(|e| e.to_string())?;
                stream.write_all(&data).map_err(|e| e.to_string())?;
            }
        }
        result = file.into_inner().into_inner();
    } else {
        for (_, data, _) in output.remove("FileHeader").unwrap_or_default() {
            result.extend(data.strip_suffix(&[0]).unwrap_or(&data));
            result.extend(b"\r\n");
        }
    }
    if result.len() > MAX_FILE {
        return Err("Native schematic exceeds save limit".into());
    }
    verify(&original, doc, &result, &old_indices)?;
    Ok(result)
}

pub fn save(path: &Path, doc: &Document) -> Result<(), String> {
    let bytes = serialize(doc)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temp.write_all(&bytes).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
