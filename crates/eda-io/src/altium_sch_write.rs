//! Lossless-container writing of schematic ECO properties. Geometry is verified,
//! not reconstructed from an incomplete native format description.
use super::*;
use eda_core::schematic::SchPrimitiveKind as K;
use std::io::Write;

#[cfg(test)]
#[path = "altium_sch_arrange_tests.rs"]
mod arrange_tests;
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

/// Rearrange complete native owner subtrees, never individual component records.
fn drawing_order(
    output: &mut [(u8, Vec<u8>, Option<u32>)],
    source: &Schematic,
    target: &Schematic,
    patch: &Patch,
) -> Result<(), String> {
    let old: Vec<_> = source
        .primitives
        .iter()
        .filter(|p| target.primitive(p.id).is_some())
        .map(|p| p.id)
        .collect();
    let desired: Vec<_> = target
        .primitives
        .iter()
        .filter(|p| source.primitive(p.id).is_some())
        .map(|p| p.id)
        .collect();
    if old == desired {
        return Ok(());
    }
    let additional = |id: Uuid| {
        source
            .primitive(id)
            .and_then(|p| source.records.get(p.source_index as usize))
            .is_some_and(|r| r.stream == "Additional")
    };
    let old_add: Vec<_> = old.iter().copied().filter(|id| additional(*id)).collect();
    let new_add: Vec<_> = desired
        .iter()
        .copied()
        .filter(|id| additional(*id))
        .collect();
    let mut seen_additional = false;
    if old_add != new_add
        || desired.iter().any(|id| {
            if additional(*id) {
                seen_additional = true;
                false
            } else {
                seen_additional
            }
        })
    {
        return Err(
            "Cross-stream schematic drawing order cannot be saved safely; choose PCBCraft JSON"
                .into(),
        );
    }
    let mut ranks = BTreeMap::new();
    for p in &target.primitives {
        let mut root = p.component.unwrap_or(p.id);
        let mut parent = p.presentation.parent;
        for _ in 0..128 {
            let Some(id) = parent else {
                break;
            };
            root = id;
            parent = target.primitive(id).and_then(|p| p.presentation.parent);
        }
        let record = target
            .components
            .iter()
            .find(|c| c.id == root)
            .map(|c| c.source_index)
            .or_else(|| target.primitive(root).map(|p| p.source_index));
        if let Some(record) = record {
            let record = if record == eda_core::schematic_edit::AUTHORED {
                patch
                    .authored
                    .iter()
                    .find_map(|(n, id)| (*id == root).then_some(*n))
            } else {
                Some(record)
            };
            if let Some(record) = record {
                let rank = ranks.len();
                ranks.entry(record).or_insert(rank);
            }
        }
    }
    let rank = |index: Option<u32>| {
        let mut index = index?;
        for _ in 0..64 {
            if let Some(rank) = ranks.get(&index) {
                return Some(*rank);
            }
            let f = if (index as usize) < patch.records.len() {
                patch.records[index as usize].as_ref()?
            } else {
                patch.added.get(index as usize - patch.records.len())?
            };
            let owner = f.n("OWNERINDEX");
            if owner <= 0 || owner == index as i64 {
                return None;
            }
            index = owner as u32;
        }
        None
    };
    output.sort_by_key(|(_, _, index)| rank(*index).map_or((0, 0), |rank| (1, rank)));
    Ok(())
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
        let point = if owner == 0 {
            header_count
        } else if owner < count {
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
    for old in source.primitives.iter().chain(&source.parameter_objects) {
        if !harness_write::harness(old)
            && target.primitive(old.id).is_none()
            && (matches!(
                old.kind,
                K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
            ) || old.presentation.parent.is_some()
                || old.component.is_none()
                    && matches!(
                        old.kind,
                        K::Polyline { .. }
                            | K::Text { .. }
                            | K::PowerPort { .. }
                            | K::Rectangle { .. }
                            | K::Arc { .. }
                            | K::TextFrame { .. }
                            | K::Bezier { .. }
                            | K::NoErc { .. }
                            | K::ParameterSet { .. }
                            | K::CompileMask { .. }
                            | K::Image { .. }
                    ))
        {
            patch.remove_tree(old.source_index as usize);
        }
    }
    for p in target.primitives.iter().chain(&target.parameter_objects) {
        if harness_write::harness(p) {
            continue;
        }
        let old = source.primitive(p.id);
        if old == Some(p) {
            continue;
        }
        let supported = p.component.is_some()
            || matches!(
                p.kind,
                K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
            )
            || p.presentation.parent.is_some()
            || p.component.is_none()
                && matches!(
                    p.kind,
                    K::Polyline { .. }
                        | K::Text { .. }
                        | K::PowerPort { .. }
                        | K::Rectangle { .. }
                        | K::Arc { .. }
                        | K::TextFrame { .. }
                        | K::Bezier { .. }
                        | K::NoErc { .. }
                        | K::ParameterSet { .. }
                        | K::CompileMask { .. }
                        | K::Image { .. }
                );
        if !supported {
            continue;
        }
        let index = if old.is_some() {
            p.source_index
        } else if (p.component.is_some()
            || matches!(
                p.kind,
                K::SheetSymbol { .. } | K::SheetEntry { .. } | K::Port { .. }
            )
            || p.component.is_none()
                && matches!(
                    p.kind,
                    K::Polyline { .. }
                        | K::Text { .. }
                        | K::PowerPort { .. }
                        | K::Rectangle { .. }
                        | K::Arc { .. }
                        | K::TextFrame { .. }
                        | K::Bezier { .. }
                        | K::NoErc { .. }
                        | K::ParameterSet { .. }
                        | K::CompileMask { .. }
                        | K::Image { .. }
                ))
            && p.source_index == eda_core::schematic_edit::AUTHORED
        {
            let owner = if let Some(id) = p.component {
                target
                    .components
                    .iter()
                    .find(|c| c.id == id)
                    .and_then(|c| {
                        if c.source_index != eda_core::schematic_edit::AUTHORED {
                            Some(c.source_index)
                        } else {
                            patch
                                .authored
                                .iter()
                                .find_map(|(index, object)| (*object == id).then_some(*index))
                        }
                    })
                    .ok_or("New component must be allocated before its geometry")?
            } else if let Some(id) = p.presentation.parent {
                target
                    .primitive(id)
                    .and_then(|p| {
                        if p.source_index != eda_core::schematic_edit::AUTHORED {
                            Some(p.source_index)
                        } else {
                            patch
                                .authored
                                .iter()
                                .find_map(|(index, id)| (*id == p.id).then_some(*index))
                        }
                    })
                    .ok_or("New child requires its native owner to be allocated first")?
            } else {
                0
            };
            let kind = match &p.kind {
                K::SheetSymbol { .. } => 15,
                K::ParameterSet { .. } => {
                    if p.presentation.record_type == 210 {
                        210
                    } else {
                        43
                    }
                }
                K::Image { .. } => 30,
                K::Pin { .. } => 2,
                K::CompileMask { .. } => 211,
                K::NoErc { .. } => 22,
                K::Rectangle { radius, .. } => {
                    if *radius == Point::default() {
                        14
                    } else {
                        10
                    }
                }
                K::Arc {
                    radii, start, end, ..
                } => {
                    if p.fill.is_some() && (end - start).abs() >= 359.9 {
                        8
                    } else if radii.x == radii.y {
                        12
                    } else if (end - start).abs() >= 359.9 {
                        8
                    } else {
                        11
                    }
                }
                K::TextFrame { .. } => {
                    if p.presentation.record_type == 209 {
                        209
                    } else {
                        28
                    }
                }
                K::Bezier { .. } => 5,
                K::SheetEntry { .. } => 16,
                K::Port { .. } => 18,
                K::Text { .. } => {
                    if p.text_role == Some(eda_core::schematic::TextRole::Designator) {
                        34
                    } else if p.text_role == Some(eda_core::schematic::TextRole::Comment) {
                        41
                    } else if matches!(p.presentation.record_type, 32 | 33 | 41) {
                        p.presentation.record_type
                    } else if p.text_role == Some(eda_core::schematic::TextRole::NetLabel) {
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
                    _ => {
                        if matches!(p.kind, K::Polyline { closed: true, .. }) {
                            7
                        } else {
                            6
                        }
                    }
                },
                _ => return Err("Unsupported native authored primitive".into()),
            };
            let uid = patch.fresh_uid(owner, kind, &p.id.to_string())?;
            let index = patch.add(kind, owner);
            patch.authored.insert(index, p.id);
            let f = patch.added.last_mut().unwrap();
            set(f, "UNIQUEID", uid);
            if let Some(id) = p.component {
                let component = target
                    .components
                    .iter()
                    .find(|c| c.id == id)
                    .ok_or("Missing component")?;
                set(f, "OWNERPARTID", component.part.to_string());
                set(
                    f,
                    "OWNERPARTDISPLAYMODE",
                    component.display_mode.to_string(),
                );
            }
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
            K::Image {
                a,
                b,
                filename,
                keep_aspect,
                embedded,
            } => {
                location(f, *a);
                coordinate_units(f, "CORNER.X", native_units(b.x));
                coordinate_units(f, "CORNER.Y", native_units(b.y));
                set(f, "FILENAME", filename.clone());
                set(f, "KEEPASPECT", boolean(*keep_aspect));
                set(f, "EMBEDIMAGE", boolean(*embedded));
            }
            K::Pin {
                position,
                length_nm,
                orientation,
                name,
                number,
                show_name,
                show_number,
                font,
                number_font,
                electrical,
                outer_symbol,
                inner_symbol,
            } => {
                location(f, *position);
                coordinate_units(f, "PINLENGTH", native_units(*length_nm));
                set(
                    f,
                    "PINCONGLOMERATE",
                    ((if f.contains_key("PINCONGLOMERATE") {
                        f.n("PINCONGLOMERATE") as u32 & !27
                    } else {
                        32
                    }) + orientation
                        + if *show_name { 8 } else { 0 }
                        + if *show_number { 16 } else { 0 })
                    .to_string(),
                );
                set(f, "NAME", name.clone());
                set(f, "DESIGNATOR", number.clone());
                set(f, "ELECTRICAL", electrical.to_string());
                set(f, "NAME_CUSTOMFONTID", (font + 1).to_string());
                set(
                    f,
                    "DESIGNATOR_CUSTOMFONTID",
                    (number_font.unwrap_or(*font) + 1).to_string(),
                );
                set(f, "SYMBOL_OUTEREDGE", outer_symbol.to_string());
                set(f, "SYMBOL_INNEREDGE", inner_symbol.to_string());
            }
            K::ParameterSet {
                position,
                orientation,
                label,
                style,
            } => {
                location(f, *position);
                set(f, "ORIENTATION", orientation.to_string());
                set(f, "NAME", label.clone());
                set(f, "STYLE", style.to_string());
            }
            K::CompileMask { a, b, collapsed } => {
                location(f, *a);
                coordinate_units(f, "CORNER.X", native_units(b.x));
                coordinate_units(f, "CORNER.Y", native_units(b.y));
                set(f, "COLLAPSED", boolean(*collapsed));
            }
            K::Rectangle { a, b, radius } => {
                location(f, *a);
                coordinate_units(f, "CORNER.X", native_units(b.x));
                coordinate_units(f, "CORNER.Y", native_units(b.y));
                coordinate_units(f, "CORNERXRADIUS", native_units(radius.x));
                coordinate_units(f, "CORNERYRADIUS", native_units(radius.y));
            }
            K::Arc {
                centre,
                radii,
                start,
                end,
            } => {
                location(f, *centre);
                coordinate_units(f, "RADIUS", native_units(radii.x));
                coordinate_units(f, "SECONDARYRADIUS", native_units(radii.y));
                set(f, "STARTANGLE", start.to_string());
                set(f, "ENDANGLE", end.to_string());
            }
            K::TextFrame {
                a,
                b,
                text,
                font,
                alignment,
                word_wrap,
                show_border,
            } => {
                location(f, *a);
                coordinate_units(f, "CORNER.X", native_units(b.x));
                coordinate_units(f, "CORNER.Y", native_units(b.y));
                set(f, "TEXT", text.replace('\n', "~1"));
                set(f, "FONTID", (font + 1).to_string());
                set(f, "ALIGNMENT", alignment.to_string());
                set(f, "WORDWRAP", boolean(*word_wrap));
                set(f, "SHOWBORDER", boolean(*show_border));
            }
            K::Bezier { points } => {
                set(f, "LOCATIONCOUNT", points.len().to_string());
                for (n, p) in points.iter().enumerate() {
                    coordinate_units(f, &format!("X{}", n + 1), native_units(p.x));
                    coordinate_units(f, &format!("Y{}", n + 1), native_units(p.y));
                }
            }
            K::NoErc { position } => location(f, *position),
            K::SheetSymbol { a, b, .. } => {
                set(f, "SYMBOLTYPE", p.presentation.symbol_type.clone());
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
                set(
                    f,
                    "TEXT",
                    p.presentation.raw_text.as_ref().unwrap_or(text).clone(),
                );
                set(f, "FONTID", (font + 1).to_string());
                set(f, "ORIENTATION", orientation.to_string());
                set(f, "JUSTIFICATION", justification.to_string());
                set(f, "ISHIDDEN", boolean(p.presentation.hidden));
                if matches!(f.n("RECORD"), 34 | 41) {
                    set(f, "NAME", p.presentation.name.clone());
                    set(f, "SHOWNAME", boolean(p.presentation.show_name));
                    set(f, "AUTOPOSITIONOFF", boolean(!p.presentation.auto_position));
                }
            }
            K::Polyline {
                points, electrical, ..
            } if matches!(electrical.as_str(), "Wire" | "Bus" | "Graphic") => {
                if f.n("RECORD") == 13 && points.len() == 2 {
                    location(f, points[0]);
                    coordinate_units(f, "CORNER.X", native_units(points[1].x));
                    coordinate_units(f, "CORNER.Y", native_units(points[1].y));
                } else {
                    set(f, "LOCATIONCOUNT", points.len().to_string());
                    for (n, p) in points.iter().enumerate() {
                        coordinate_units(f, &format!("X{}", n + 1), native_units(p.x));
                        coordinate_units(f, &format!("Y{}", n + 1), native_units(p.y));
                    }
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
                cross_sheet,
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
                set(f, "ISCROSSSHEETCONNECTOR", boolean(*cross_sheet));
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
            let parameter_owner = patch.add(48, model_index);
            parameters(
                patch,
                source,
                parameter_owner,
                &Fields::new(),
                &model.parameters,
            )?;
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
    for old in &source.components {
        if !target.components.iter().any(|c| c.id == old.id) {
            patch.remove_tree(old.source_index as usize);
        }
    }
    for c in target
        .components
        .iter()
        .filter(|c| !source.components.iter().any(|old| old.id == c.id))
    {
        if c.source_index != eda_core::schematic_edit::AUTHORED {
            return Err("New component has a native source identity".into());
        }
        let index = patch.add(1, 0);
        patch.authored.insert(index, c.id);
        let uid = patch.fresh_uid(0, 1, &c.id.to_string())?;
        let f = patch.added.last_mut().unwrap();
        set(f, "UNIQUEID", uid);
        location(f, c.position);
        for (name, value) in [
            ("LIBREFERENCE", c.library_reference.clone()),
            ("COMPONENTDESCRIPTION", c.description.clone()),
            ("PARTCOUNT", (c.settings.part_count + 1).to_string()),
            ("CURRENTPARTID", c.part.to_string()),
            ("DISPLAYMODECOUNT", (c.display_mode + 1).to_string()),
            ("DISPLAYMODE", c.display_mode.to_string()),
            ("ORIENTATION", c.settings.rotation.to_string()),
            ("COMPONENTKINDVERSION2", c.settings.kind.to_string()),
            ("SOURCELIBRARYNAME", c.settings.source.clone()),
            ("DESIGNITEMID", c.settings.design_item_id.clone()),
            ("DESIGNATORLOCKED", boolean(c.settings.designator_locked)),
            ("PARTIDLOCKED", boolean(c.settings.part_locked)),
        ] {
            set(f, name, value);
        }
    }
    for c in &target.components {
        let Some(old) = source.components.iter().find(|v| v.id == c.id) else {
            continue;
        };
        if (old.position != c.position || old.settings.rotation != c.settings.rotation)
            && source.records.iter().any(|r| {
                matches!(r.record_type,2..=18|25..=30|209)
                    && component_owner(&source.records, r.index as usize)
                        == Some(old.source_index as usize)
                    && !source
                        .primitives
                        .iter()
                        .chain(&source.parameter_objects)
                        .any(|p| p.source_index == r.index)
            })
        {
            return Err("Native symbol has unrepresented geometry; choose PCBCraft JSON".into());
        }
        let f = patch.record(old.source_index)?;
        if old.position != c.position || old.settings.rotation != c.settings.rotation {
            location(f, c.position);
            set(f, "ORIENTATION", c.settings.rotation.to_string());
        }
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
    for c in target
        .components
        .iter()
        .filter(|c| !source.components.iter().any(|old| old.id == c.id))
    {
        let index = patch
            .authored
            .iter()
            .find_map(|(index, id)| (*id == c.id).then_some(*index))
            .unwrap();
        let drawn: Fields = target
            .primitives
            .iter()
            .filter(|p| p.component == Some(c.id) && p.presentation.record_type == 41)
            .filter_map(|p| {
                if let K::Text { text, .. } = &p.kind {
                    Some((p.presentation.name.clone(), text.clone()))
                } else {
                    None
                }
            })
            .collect();
        parameters(&mut patch, source, index, &drawn, &c.parameters)?;
        let mut blank = c.clone();
        blank.source_index = index;
        blank.settings.models.clear();
        models(&mut patch, source, &blank, c)?;
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
    authored_indices: &BTreeMap<u32, Uuid>,
) -> Result<(), String> {
    let mut actual = import_schematic(bytes, &doc.name)?;
    let source = original.schematic.as_ref().unwrap();
    let scene = actual.schematic.as_mut().unwrap();
    let mut ids = BTreeMap::new();
    for c in &mut scene.components {
        let old =
            if let Some(index) = old_indices.get(c.source_index as usize).copied().flatten() {
                source.components.iter().find(|c| c.source_index == index)
            } else {
                authored_indices.get(&c.source_index).and_then(|id| {
                    doc.schematic
                        .as_ref()?
                        .components
                        .iter()
                        .find(|c| c.id == *id)
                })
            }
            .ok_or("Unexpected serialized component")?;
        ids.insert(c.id, old.id);
        c.id = old.id;
        c.source_index = old.source_index;
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
    // Hidden authored parameters may be read into the non-drawing collection.
    // Keep the caller's collection membership without dropping their metadata.
    let mut restored = vec![];
    scene.parameter_objects.retain(|p| {
        if expected_scene
            .primitives
            .iter()
            .any(|expected| expected.id == p.id)
        {
            restored.push(p.clone());
            false
        } else {
            true
        }
    });
    scene.primitives.extend(restored);
    let mut restored = vec![];
    scene.primitives.retain(|p| {
        if expected_scene
            .parameter_objects
            .iter()
            .any(|expected| expected.id == p.id)
        {
            restored.push(p.clone());
            false
        } else {
            true
        }
    });
    scene.parameter_objects.extend(restored);
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
                | K::PowerPort { .. }
                | K::Pin { .. }
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
    for c in &mut scene.components {
        if let Some(expected) = expected_scene.components.iter().find(|v| v.id == c.id)
            && near(c.position, expected.position)
        {
            c.position = expected.position;
        }
    }
    for read in scene
        .primitives
        .iter_mut()
        .chain(&mut scene.parameter_objects)
    {
        let Some(expected) = expected_scene.primitive(read.id) else {
            continue;
        };
        match (&mut read.kind, &expected.kind) {
            (
                K::Pin {
                    position,
                    length_nm,
                    ..
                },
                K::Pin {
                    position: at,
                    length_nm: length,
                    ..
                },
            ) if near(*position, *at) && (*length_nm - *length).abs() <= 2 => {
                *position = *at;
                *length_nm = *length;
            }
            (K::ParameterSet { position, .. }, K::ParameterSet { position: at, .. })
                if near(*position, *at) =>
            {
                *position = *at
            }
            (K::CompileMask { a, b, .. }, K::CompileMask { a: x, b: y, .. })
                if near(*a, *x) && near(*b, *y) =>
            {
                *a = *x;
                *b = *y;
            }
            (K::Blanket { points, .. }, K::Blanket { points: p, .. })
                if points.len() == p.len() && points.iter().zip(p).all(|(a, b)| near(*a, *b)) =>
            {
                points.clone_from(p)
            }
            (
                K::PowerPort { position, .. } | K::NoErc { position },
                K::PowerPort { position: p, .. } | K::NoErc { position: p },
            ) if near(*position, *p) => *position = *p,
            (
                K::Rectangle { a, b, radius },
                K::Rectangle {
                    a: x,
                    b: y,
                    radius: r,
                },
            ) if near(*a, *x) && near(*b, *y) && near(*radius, *r) => {
                *a = *x;
                *b = *y;
                *radius = *r;
            }
            (
                K::TextFrame { a, b, .. } | K::Image { a, b, .. },
                K::TextFrame { a: x, b: y, .. } | K::Image { a: x, b: y, .. },
            ) if near(*a, *x) && near(*b, *y) => {
                *a = *x;
                *b = *y;
            }
            (
                K::Arc { centre, radii, .. },
                K::Arc {
                    centre: c,
                    radii: r,
                    ..
                },
            ) if near(*centre, *c) && near(*radii, *r) => {
                *centre = *c;
                *radii = *r;
            }
            (K::Bezier { points }, K::Bezier { points: p })
                if points.len() == p.len() && points.iter().zip(p).all(|(a, b)| near(*a, *b)) =>
            {
                points.clone_from(p)
            }
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
                    || read.component.is_some()
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
                "Wire" | "Bus" | "Signal Harness" | "Bus Entry" | "Graphic"
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
        discard_external_cache(s);
        for p in &mut s.primitives {
            if matches!(p.kind, K::Port { .. }) && p.presentation.text_color.is_none() {
                p.presentation.text_color = Some(p.color);
            }
        }
    }
    if actual != expected {
        #[cfg(test)]
        {
            fn difference(
                a: &serde_json::Value,
                b: &serde_json::Value,
                path: String,
            ) -> Option<String> {
                if a == b {
                    return None;
                }
                match (a, b) {
                    (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
                        for key in a.keys().chain(b.keys()) {
                            if let Some(v) = difference(
                                &a.get(key).cloned().unwrap_or_default(),
                                &b.get(key).cloned().unwrap_or_default(),
                                format!("{path}.{key}"),
                            ) {
                                return Some(v);
                            }
                        }
                    }
                    (serde_json::Value::Array(a), serde_json::Value::Array(b))
                        if a.len() == b.len() =>
                    {
                        for (i, (a, b)) in a.iter().zip(b).enumerate() {
                            if let Some(v) = difference(a, b, format!("{path}[{i}]")) {
                                return Some(v);
                            }
                        }
                    }
                    _ => {}
                }
                Some(format!("{path}: actual={a}, expected={b}"))
            }
            eprintln!(
                "{}",
                difference(
                    &serde_json::to_value(&actual).unwrap(),
                    &serde_json::to_value(&expected).unwrap(),
                    String::new()
                )
                .unwrap()
            );
        }
        return Err("Native SchDoc verification found unsupported scene changes; choose PCBCraft JSON. The destination was not modified".into());
    }
    Ok(())
}

/// Serialize supported ECO edits while preserving the original container and all
/// untouched record payloads. Unsupported edits fail before touching a destination.
fn discard_external_cache(s: &mut Schematic) {
    let mut embedded = BTreeSet::new();
    let mut external = BTreeSet::new();
    for p in &s.primitives {
        if let K::Image {
            filename,
            embedded: stored,
            ..
        } = &p.kind
        {
            let key = filename.replace('\\', "/").to_lowercase();
            if *stored {
                embedded.insert(key);
            } else {
                external.insert(key);
            }
        }
    }
    std::sync::Arc::make_mut(&mut s.images)
        .retain(|key, _| !external.contains(key) || embedded.contains(key));
}

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
    let mut comparable = original.clone();
    discard_external_cache(unchanged.schematic.as_mut().unwrap());
    discard_external_cache(comparable.schematic.as_mut().unwrap());
    if unchanged == comparable {
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
                    if matches!(f.n("RECORD") as u32, 215..=218 | 225) {
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
        let stream = if matches!(f.n("RECORD") as u32, 215..=218 | 225) {
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
    drawing_order(
        output.get_mut("FileHeader").unwrap(),
        original.schematic.as_ref().unwrap(),
        s,
        &patch,
    )?;
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
                    || !matches!(r.record_type, 1..=18|22|25..=34|37|39|41|43..=48|209..=211|215..=218|225)
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
        let embedded: BTreeSet<_> = s
            .primitives
            .iter()
            .filter_map(|p| {
                if let K::Image {
                    filename,
                    embedded: true,
                    ..
                } = &p.kind
                {
                    Some(filename.replace('\\', "/").to_lowercase())
                } else {
                    None
                }
            })
            .collect();
        let desired: BTreeMap<_, _> = s
            .images
            .iter()
            .filter(|(key, _)| {
                embedded.contains(*key)
                    || original
                        .schematic
                        .as_ref()
                        .unwrap()
                        .images
                        .contains_key(*key)
            })
            .collect();
        if embedded.iter().any(|key| !s.images.contains_key(key)) {
            return Err("Embedded graphic data is unavailable".into());
        }
        if desired
            .iter()
            .map(|(k, v)| ((*k).clone(), (*v).clone()))
            .collect::<BTreeMap<_, _>>()
            != *original.schematic.as_ref().unwrap().images
        {
            let mut stream = file.create_stream("Storage").map_err(|e| e.to_string())?;
            let header = format!("|HEADER=Icon storage|Weight={}\0", desired.len()).into_bytes();
            stream
                .write_all(&(header.len() as u32).to_le_bytes())
                .map_err(|e| e.to_string())?;
            stream.write_all(&header).map_err(|e| e.to_string())?;
            for (key, encoded) in desired {
                let filename = s
                    .primitives
                    .iter()
                    .find_map(|p| {
                        if let K::Image { filename, .. } = &p.kind {
                            (filename.replace('\\', "/").to_lowercase() == *key)
                                .then_some(filename.as_str())
                        } else {
                            None
                        }
                    })
                    .unwrap_or(key);
                let native_name = encoding.encode(filename).0;
                if native_name.len() > 255 {
                    return Err("Embedded image filename exceeds native limit".into());
                }
                let data = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
                if data.len() > 32 * 1024 * 1024 {
                    return Err("Embedded image exceeds native limit".into());
                }
                let mut compressor =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                compressor.write_all(&data).map_err(|e| e.to_string())?;
                let compressed = compressor.finish().map_err(|e| e.to_string())?;
                let mut raw = vec![0xd0, native_name.len() as u8];
                raw.extend(native_name.as_ref());
                raw.extend((compressed.len() as u32).to_le_bytes());
                raw.extend(compressed);
                if raw.len() > 0xffffff {
                    return Err("Compressed image exceeds native record limit".into());
                }
                stream
                    .write_all(&(0x01000000 | raw.len() as u32).to_le_bytes())
                    .map_err(|e| e.to_string())?;
                stream.write_all(&raw).map_err(|e| e.to_string())?;
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
