//! Lossless-container writing of schematic ECO properties. Geometry is verified,
//! not reconstructed from an incomplete native format description.
use super::*;
use eda_core::schematic::SchPrimitiveKind as K;
use std::io::Write;

#[cfg(test)]
#[path = "altium_sch_harness_tests.rs"]
mod harness_tests;
#[cfg(test)]
#[path = "altium_sch_write_tests.rs"]
mod tests;

#[path = "altium_sch_harness_write.rs"]
mod harness_write;

struct Patch {
    records: Vec<Option<Fields>>,
    added: Vec<Fields>,
    namespace: Uuid,
    used_uids: BTreeSet<String>,
    authored: BTreeMap<u32, Uuid>,
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

fn native_units(nm: i64) -> i128 {
    let scaled = nm as i128 * 100_000;
    (scaled + if scaled < 0 { -127_000 } else { 127_000 }) / 254_000
}
fn coordinate_units(f: &mut Fields, key: &str, units: i128) {
    set(f, key, (units / 100_000).to_string());
    set(f, &format!("{key}_FRAC"), (units % 100_000).to_string());
}
fn location(f: &mut Fields, p: Point) {
    coordinate_units(f, "LOCATION.X", native_units(p.x));
    coordinate_units(f, "LOCATION.Y", native_units(p.y));
}
fn hierarchy_changes(
    patch: &mut Patch,
    source: &Schematic,
    target: &Schematic,
) -> Result<(), String> {
    for old in &source.primitives {
        if !harness_write::harness(old)
            && target.primitive(old.id).is_none()
            && (matches!(
                old.kind,
                K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
            ) || old.presentation.parent.is_some()
                || old.component.is_none()
                    && matches!(
                        old.kind,
                        K::Polyline { .. } | K::Text { .. } | K::PowerPort { .. }
                    ))
        {
            patch.remove_tree(old.source_index as usize);
        }
    }
    for p in &target.primitives {
        if harness_write::harness(p) {
            continue;
        }
        let old = source.primitive(p.id);
        if old == Some(p) {
            continue;
        }
        let supported = matches!(
            p.kind,
            K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
        ) || p.presentation.parent.is_some()
            || p.component.is_none()
                && matches!(
                    p.kind,
                    K::Polyline { .. } | K::Text { .. } | K::PowerPort { .. }
                );
        if !supported {
            continue;
        }
        let index = if old.is_some() {
            p.source_index
        } else if (matches!(p.kind, K::SheetEntry { .. } | K::Port { .. })
            || p.component.is_none()
                && matches!(
                    p.kind,
                    K::Polyline { .. } | K::Text { .. } | K::PowerPort { .. }
                ))
            && p.source_index == eda_core::schematic_edit::AUTHORED
        {
            let owner = if let Some(id) = p.presentation.parent {
                target
                    .primitive(id)
                    .filter(|p| p.source_index != eda_core::schematic_edit::AUTHORED)
                    .ok_or("New Sheet Entry requires an existing native Sheet Symbol")?
                    .source_index
            } else {
                0
            };
            let kind = match &p.kind {
                K::SheetEntry { .. } => 16,
                K::Port { .. } => 18,
                K::Text { .. } => {
                    if p.text_role == Some(eda_core::schematic::TextRole::NetLabel) {
                        25
                    } else {
                        4
                    }
                }
                K::PowerPort { .. } => 17,
                K::Polyline { electrical, .. } => match electrical.as_str() {
                    "Wire" => 27,
                    "Bus" => 26,
                    "Bus Entry" => 37,
                    _ => 6,
                },
                _ => return Err("Unsupported native authored primitive".into()),
            };
            let uid = patch.fresh_uid(owner, kind, &p.id.to_string())?;
            let index = patch.add(kind, owner);
            patch.authored.insert(index, p.id);
            let f = patch.added.last_mut().unwrap();
            set(f, "UNIQUEID", uid);
            index
        } else {
            continue;
        };
        let f = if index as usize >= patch.records.len() {
            let n = index as usize - patch.records.len();
            &mut patch.added[n]
        } else {
            patch.record(index)?
        };
        match &p.kind {
            K::SheetSymbol { a, b, .. } => {
                location(f, *a);
                coordinate_units(f, "XSIZE", native_units(b.x) - native_units(a.x));
                coordinate_units(f, "YSIZE", native_units(a.y) - native_units(b.y));
            }
            K::SheetEntry {
                position,
                side,
                io_type,
                name,
                font,
                text_color,
                arrow_kind,
            } => {
                let parent = target
                    .primitive(p.presentation.parent.ok_or("Missing entry parent")?)
                    .ok_or("Missing Sheet Symbol")?;
                let K::SheetSymbol { a, .. } = parent.kind else {
                    return Err("Invalid entry parent".into());
                };
                let distance = if *side <= 1 {
                    native_units(a.y) - native_units(position.y)
                } else {
                    native_units(position.x) - native_units(a.x)
                };
                set(f, "SIDE", side.to_string());
                set(f, "IOTYPE", io_type.to_string());
                set(f, "NAME", name.clone());
                set(f, "TEXTFONTID", (font + 1).to_string());
                set(f, "TEXTCOLOR", text_color.to_string());
                set(f, "ARROWKIND", arrow_kind.clone());
                set(f, "HARNESSTYPE", p.presentation.harness_type.clone());
                set(f, "DISTANCEFROMTOP", (distance / 1_000_000).to_string());
                set(
                    f,
                    "DISTANCEFROMTOP_FRAC1",
                    (distance % 1_000_000).to_string(),
                );
            }
            K::Port {
                position,
                width_nm,
                height_nm,
                style,
                io_type,
                name,
                font,
            } => {
                location(f, *position);
                let (origin, end) = if *style >= 4 {
                    (position.y, position.y + width_nm)
                } else {
                    (position.x, position.x + width_nm)
                };
                coordinate_units(f, "WIDTH", native_units(end) - native_units(origin));
                coordinate_units(f, "HEIGHT", native_units(*height_nm));
                set(f, "STYLE", style.to_string());
                set(f, "IOTYPE", io_type.to_string());
                set(f, "NAME", name.clone());
                set(f, "HARNESSTYPE", p.presentation.harness_type.clone());
                set(
                    f,
                    "TEXTCOLOR",
                    p.presentation.text_color.unwrap_or(p.color).to_string(),
                );
                set(f, "FONTID", (font + 1).to_string());
                set(
                    f,
                    "ALIGNMENT",
                    p.presentation.alignment.unwrap_or(0).to_string(),
                );
            }
            K::Text {
                position,
                text,
                font,
                orientation,
                justification,
            } => {
                location(f, *position);
                set(f, "TEXT", text.clone());
                set(f, "FONTID", (font + 1).to_string());
                set(f, "ORIENTATION", orientation.to_string());
                set(f, "JUSTIFICATION", justification.to_string());
                set(f, "ISHIDDEN", boolean(p.presentation.hidden));
            }
            K::Polyline {
                points, electrical, ..
            } if matches!(electrical.as_str(), "Wire" | "Bus" | "Graphic") => {
                set(f, "LOCATIONCOUNT", points.len().to_string());
                for (n, p) in points.iter().enumerate() {
                    coordinate_units(f, &format!("X{}", n + 1), native_units(p.x));
                    coordinate_units(f, &format!("Y{}", n + 1), native_units(p.y));
                }
            }
            K::Polyline {
                points, electrical, ..
            } if electrical == "Bus Entry" && points.len() == 2 => {
                location(f, points[0]);
                coordinate_units(f, "CORNER.X", native_units(points[1].x));
                coordinate_units(f, "CORNER.Y", native_units(points[1].y));
            }
            K::PowerPort {
                position,
                orientation,
                style,
                text,
                show_name,
                font,
            } => {
                location(f, *position);
                set(f, "ORIENTATION", orientation.to_string());
                set(f, "STYLE", style.to_string());
                set(f, "TEXT", text.clone());
                set(f, "SHOWNETNAME", boolean(*show_name));
                set(f, "FONTID", (font + 1).to_string());
            }
            _ => continue,
        }
        set(f, "DISABLESYNC", boolean(!p.presentation.allow_sync));
        set(f, "COLOR", p.color.to_string());
        set(f, "ISSOLID", boolean(p.fill.is_some()));
        if let Some(color) = p.fill {
            set(f, "AREACOLOR", color.to_string());
        }
        set(
            f,
            if matches!(p.kind, K::Port { .. }) {
                "BORDERWIDTH"
            } else {
                "LINEWIDTH"
            },
            match p.width_nm {
                101_600 => "0",
                254_000 => "1",
                508_000 => "2",
                1_016_000 => "3",
                _ => return Err("Unsupported native schematic stroke width".into()),
            }
            .into(),
        );
    }
    Ok(())
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
        authored: BTreeMap::new(),
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
            if f.contains_key("COMPONENTKINDVERSION2") || c.settings.kind >= 5 {
                set(f, "COMPONENTKINDVERSION2", c.settings.kind.to_string());
                set(
                    f,
                    "COMPONENTKIND",
                    if c.settings.kind >= 5 {
                        "0".into()
                    } else {
                        c.settings.kind.to_string()
                    },
                );
            } else {
                set(f, "COMPONENTKIND", c.settings.kind.to_string());
            }
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
    if target.fonts != source.fonts {
        let f = patch.record(0)?;
        set(f, "FONTIDCOUNT", target.fonts.len().to_string());
        for (i, font) in target.fonts.iter().enumerate() {
            let n = i + 1;
            set(f, &format!("FONTNAME{n}"), font.name.clone());
            set(
                f,
                &format!("SIZE{n}"),
                (font.height_nm / 254_000).to_string(),
            );
            set(f, &format!("BOLD{n}"), boolean(font.bold));
            set(f, &format!("ITALIC{n}"), boolean(font.italic));
            set(f, &format!("UNDERLINE{n}"), boolean(font.underline));
        }
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
            _ => continue,
        };
        set(patch.record(p.source_index)?, "TEXT", text.clone());
    }
    harness_write::changes(&mut patch, source, target)?;
    hierarchy_changes(&mut patch, source, target)?;
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
    authored_indices: &BTreeMap<u32, Uuid>,
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
                || authored_indices.contains_key(&p.source_index)
        });
        for p in primitives {
            let old = if let Some(index) = old_indices[p.source_index as usize] {
                source
                    .primitives
                    .iter()
                    .chain(&source.parameter_objects)
                    .find(|p| p.source_index == index)
            } else {
                authored_indices
                    .get(&p.source_index)
                    .and_then(|id| doc.schematic.as_ref()?.primitive(*id))
            }
            .ok_or("Unexpected serialized primitive")?;
            if authored_indices.contains_key(&p.source_index) {
                p.presentation.record_type = old.presentation.record_type;
            }
            ids.insert(p.id, old.id);
            p.id = old.id;
            p.source_index = old.source_index;
            p.component = p
                .component
                .map(|id| ids.get(&id).copied().ok_or("Missing component owner"))
                .transpose()?;
            p.presentation.parent = p
                .presentation
                .parent
                .map(|id| ids.get(&id).copied().ok_or("Missing Sheet Symbol owner"))
                .transpose()?;
        }
    }
    let expected_scene = doc.schematic.as_ref().unwrap();
    for expected in &expected_scene.primitives {
        if source.primitive(expected.id) == Some(expected) {
            continue;
        }
        let Some(read) = scene.primitive(expected.id) else {
            continue;
        };
        if matches!(
            expected.kind,
            K::SheetEntry { .. }
                | K::Port { .. }
                | K::HarnessEntry { .. }
                | K::HarnessConnector { .. }
        ) || expected.text_role == Some(eda_core::schematic::TextRole::NetLabel)
            || matches!(&expected.kind,K::Polyline{electrical,..} if matches!(electrical.as_str(),"Wire"|"Signal Harness"|"Bus"|"Bus Entry"))
        {
            let a: BTreeSet<_> = expected_scene
                .primitives
                .iter()
                .filter(|p| {
                    p.id != expected.id
                        && (eda_core::cross_probe::touches(expected, p)
                            || eda_core::schematic_harness::touches(expected, p))
                })
                .map(|p| p.id)
                .collect();
            let b: BTreeSet<_> = scene
                .primitives
                .iter()
                .filter(|p| {
                    p.id != read.id
                        && (eda_core::cross_probe::touches(read, p)
                            || eda_core::schematic_harness::touches(read, p))
                })
                .map(|p| p.id)
                .collect();
            if a != b {
                return Err("Native SchDoc coordinate quantization changed electrical attachments; choose PCBCraft JSON".into());
            }
        }
    }
    let near = |a: Point, b: Point| (a.x - b.x).abs() <= 2 && (a.y - b.y).abs() <= 2;
    for read in &mut scene.primitives {
        let Some(expected) = expected_scene.primitive(read.id) else {
            continue;
        };
        match (&mut read.kind, &expected.kind) {
            (
                K::HarnessConnector {
                    a, b, primary_nm, ..
                },
                K::HarnessConnector {
                    a: x,
                    b: y,
                    primary_nm: z,
                    ..
                },
            ) if near(*a, *x) && near(*b, *y) && (*primary_nm - *z).abs() <= 2 => {
                *a = *x;
                *b = *y;
                *primary_nm = *z;
            }
            (K::HarnessEntry { position, .. }, K::HarnessEntry { position: p, .. })
                if near(*position, *p) =>
            {
                *position = *p;
            }
            (K::SheetSymbol { a, b, .. }, K::SheetSymbol { a: x, b: y, .. })
                if near(*a, *x) && near(*b, *y) =>
            {
                *a = *x;
                *b = *y;
            }
            (K::SheetEntry { position, .. }, K::SheetEntry { position: p, .. })
                if near(*position, *p) =>
            {
                *position = *p
            }
            (
                K::Port {
                    position,
                    width_nm,
                    height_nm,
                    ..
                },
                K::Port {
                    position: p,
                    width_nm: w,
                    height_nm: h,
                    ..
                },
            ) if near(*position, *p)
                && (*width_nm - *w).abs() <= 2
                && (*height_nm - *h).abs() <= 2 =>
            {
                *position = *p;
                *width_nm = *w;
                *height_nm = *h;
            }
            (K::Text { position, .. }, K::Text { position: p, .. })
                if (read.presentation.parent.is_some()
                    || read.text_role == Some(eda_core::schematic::TextRole::NetLabel))
                    && near(*position, *p) =>
            {
                *position = *p
            }
            (
                K::Polyline {
                    points, electrical, ..
                },
                K::Polyline { points: p, .. },
            ) if matches!(
                electrical.as_str(),
                "Wire" | "Bus" | "Signal Harness" | "Bus Entry"
            ) && points.len() == p.len()
                && points.iter().zip(p).all(|(a, b)| near(*a, *b)) =>
            {
                *points = p.clone()
            }
            _ => {}
        }
    }
    // New entries are serialized inside their owner's native subtree. Match
    // their model order only after validating identity, ownership and topology.
    if !authored_indices.is_empty() {
        let order: BTreeMap<_, _> = expected_scene
            .primitives
            .iter()
            .enumerate()
            .map(|(n, p)| (p.id, n))
            .collect();
        scene
            .primitives
            .sort_by_key(|p| order.get(&p.id).copied().unwrap_or(usize::MAX));
    }
    actual.id = doc.id;
    let mut expected = doc.clone();
    for d in [&mut actual, &mut expected] {
        let s = d.schematic.as_mut().unwrap();
        s.source = None;
        s.records = Default::default();
        s.warnings.clear();
        s.source_modified = false;
        for p in &mut s.primitives {
            if matches!(p.kind, K::Port { .. }) && p.presentation.text_color.is_none() {
                p.presentation.text_color = Some(p.color);
            }
        }
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
            let retained = original
                .schematic
                .as_ref()
                .unwrap()
                .records
                .iter()
                .enumerate()
                .filter(|(i, r)| r.stream == stream && patch.records[*i].is_some())
                .count();
            let added = patch
                .added
                .iter()
                .filter(|f| {
                    if (215..=218).contains(&(f.n("RECORD") as u32)) {
                        stream == "Additional"
                    } else {
                        stream == "FileHeader"
                    }
                })
                .count();
            set(&mut after, "WEIGHT", (retained + added).to_string());
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
        let stream = if (215..=218).contains(&(f.n("RECORD") as u32)) {
            "Additional"
        } else {
            "FileHeader"
        };
        output.entry(stream).or_default().push((
            0,
            properties(&[], &Fields::new(), f, encoding)?,
            Some(index),
        ));
    }
    if !original
        .schematic
        .as_ref()
        .unwrap()
        .records
        .iter()
        .any(|r| r.stream == "Additional")
        && let Some(additional) = output.get_mut("Additional")
        && additional.first().is_some_and(|(_, _, i)| i.is_some())
    {
        let f = Fields::from([
            (
                "HEADER".into(),
                "Protel for Windows - Schematic Capture Binary File Version 5.0".into(),
            ),
            ("WEIGHT".into(), additional.len().to_string()),
        ]);
        additional.insert(0, (0, properties(&[], &Fields::new(), &f, encoding)?, None));
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
    let mut authored_indices = BTreeMap::new();
    for stream in ["FileHeader", "Additional"] {
        for (_, _, index) in output.get(stream).into_iter().flatten() {
            if let Some(index) = index {
                if let Some(id) = patch.authored.get(index) {
                    authored_indices.insert(old_indices.len() as u32, *id);
                }
                remap.insert(*index, old_indices.len() as u32);
                old_indices.push((*index < record as u32).then_some(*index));
            }
        }
    }
    let shifted = remap.iter().any(|(a, b)| a != b);
    // Text frames (209) and Additional-stream drawing regions (225) have
    // ordinary property payloads. Preserve them byte-for-byte unless their
    // explicit OWNERINDEX changes; unknown index dependencies still fail below.
    if shifted
        && original
            .schematic
            .as_ref()
            .unwrap()
            .records
            .iter()
            .any(|r| {
                !matches!(r.stream.as_str(), "FileHeader" | "Additional")
                    || !matches!(r.record_type, 1..=18|22|25..=34|37|39|41|43..=48|209|215..=218|225)
            })
    {
        return Err(
            "Opaque schematic records prevent safe owner-index remapping; choose PCBCraft JSON"
                .into(),
        );
    }
    let output_header_count = output
        .get("FileHeader")
        .into_iter()
        .flatten()
        .filter(|(_, _, i)| i.is_some())
        .count() as u32;
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
                let original_index = index.unwrap();
                let local = before.yes("OWNERINDEXADDITIONALLIST");
                let old_base = original
                    .schematic
                    .as_ref()
                    .unwrap()
                    .records
                    .iter()
                    .position(|r| r.stream == "Additional")
                    .unwrap_or(record) as u32;
                let absolute = if local && original_index < record as u32 {
                    old_base + owner
                } else {
                    owner
                };
                let new_base = output_header_count;
                let mapped = *remap
                    .get(&absolute)
                    .ok_or("Native record references a removed owner")?;
                set(
                    &mut after,
                    "OWNERINDEX",
                    if local {
                        mapped
                            .checked_sub(new_base)
                            .ok_or("Invalid Additional owner")?
                    } else {
                        mapped
                    }
                    .to_string(),
                );
            }
            if shifted
                && let Some(key) = after.keys().find(|k| {
                    k.contains("INDEX")
                        && !matches!(
                            k.as_str(),
                            "OWNERINDEX"
                                | "OWNERINDEXADDITIONALLIST"
                                | "INDEXINSHEET"
                                | "UNIONINDEX"
                        )
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
    verify(&original, doc, &result, &old_indices, &authored_indices)?;
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
    let harness_path = path.with_extension("Harness");
    let generated = eda_core::schematic_harness::definitions(
        doc.schematic.as_ref().ok_or("Expected schematic")?,
    )?;
    let definition_temp = if !generated.is_empty() || harness_path.is_file() {
        let mut definitions = if harness_path.is_file() {
            let bytes = read_bounded(&harness_path)?;
            let text = std::str::from_utf8(&bytes)
                .map(std::borrow::Cow::Borrowed)
                .unwrap_or_else(|_| WINDOWS_1251.decode(&bytes).0);
            eda_core::schematic_harness::parse_definitions(&text)?
        } else {
            vec![]
        };
        definitions.retain(|d| d.locked);
        for definition in generated {
            if !definitions
                .iter()
                .any(|d| d.name.eq_ignore_ascii_case(&definition.name))
            {
                definitions.push(definition);
            }
        }
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(eda_core::schematic_harness::format_definitions(&definitions).as_bytes())
            .map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        Some(file)
    } else {
        None
    };
    temp.persist(path).map_err(|e| e.to_string())?;
    if let Some(file) = definition_temp {
        file.persist(&harness_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
