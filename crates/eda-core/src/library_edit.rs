//! File-based library authoring. The active member is an ordinary document
//! scene, so existing checked geometry edits and bounded history are reused.
use crate::{Document, DocumentKind, History, pcb, schematic as sch, schematic_edit as edit};
use eda_geom::Point;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;
#[cfg(test)]
#[path = "library_edit_tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub others: Vec<Document>,
    /// Original native container, retained by the portable format.
    pub source: Option<pcb::SourceArchive>,
}

#[derive(Clone, Debug)]
pub enum Edit {
    FootprintMetadata {
        name: String,
        description: String,
        height_nm: i64,
        parameters: Vec<(String, String)>,
    },
    Metadata {
        name: String,
        designator: String,
        comment: String,
        description: String,
        parameters: Vec<(String, String)>,
    },
    Models(Vec<sch::ComponentModel>),
    Select(usize),
    Add(String),
    Duplicate(String),
    Rename(String),
    Remove,
    AddPart,
    RemovePart,
    AddDisplayMode,
    RemoveDisplayMode,
    RenameDisplayMode(String),
    ComponentKind(u32),
    Pin(Box<sch::SchPrimitive>),
    LinkFootprint {
        name: String,
        library: String,
        map: Vec<(String, Vec<String>)>,
    },
}

pub fn name(doc: &Document) -> &str {
    if let Some(s) = &doc.schematic {
        s.components
            .first()
            .map_or("", |c| c.library_reference.as_str())
    } else {
        doc.pcb
            .as_ref()
            .and_then(|b| b.components.first())
            .map_or("", |c| c.footprint.as_str())
    }
}
pub fn variant(doc: &Document) -> (u32, u32) {
    doc.schematic
        .as_ref()
        .and_then(|s| s.components.first())
        .map_or((1, 0), |c| (c.part, c.display_mode))
}
pub fn display_mode_name(doc: &Document, mode: u32) -> String {
    doc.schematic
        .as_ref()
        .and_then(|s| s.components.first())
        .and_then(|c| c.settings.display_mode_names.get(&mode))
        .cloned()
        .unwrap_or_else(|| {
            if mode == 0 {
                "Normal".into()
            } else {
                format!("Alternate {mode}")
            }
        })
}
pub fn members(doc: &Document) -> impl Iterator<Item = &Document> {
    std::iter::once(doc).chain(doc.library.iter().flat_map(|b| &b.others))
}

/// Selecting another member changes the editor view, not library contents.
pub fn same_contents(a: &Document, b: &Document) -> bool {
    let (Some(aa), Some(bb)) = (&a.library, &b.library) else {
        return a == b;
    };
    a.id == b.id
        && a.name == b.name
        && a.kind == b.kind
        && a.schema_version == b.schema_version
        && aa.source == bb.source
        && aa.others.len() == bb.others.len()
        && members(a).all(|m| {
            members(b).any(|n| {
                name(m) == name(n)
                    && variant(m) == variant(n)
                    && m.size == n.size
                    && m.pads == n.pads
                    && m.tracks == n.tracks
                    && m.schematic == n.schematic
                    && m.pcb == n.pcb
            })
        })
}

/// Capture independent symbol geometry at its local origin from a placed instance.
pub fn symbol_member(
    scene: &sch::Schematic,
    id: Uuid,
    item_name: &str,
) -> Result<Document, String> {
    let fragment = edit::extract(scene, id)?;
    let origin = fragment
        .components
        .first()
        .ok_or("Select a complete component")?
        .position;
    let rotation = fragment.components[0].settings.rotation;
    let fragment = fragment.transformed(
        origin,
        (4 - rotation) % 4,
        Point {
            x: -origin.x,
            y: -origin.y,
        },
    )?;
    let mut doc = member(DocumentKind::Schematic, item_name)?;
    let s = doc.schematic.as_mut().unwrap();
    s.components = fragment.components;
    s.primitives = fragment.primitives;
    s.fonts = fragment.fonts;
    s.images = fragment.images;
    s.components[0].library_reference = item_name.into();
    // One placed section alone cannot describe missing sections or display modes.
    s.components[0].settings.part_count = 1;
    s.components[0].part = 1;
    s.components[0].display_mode = 0;
    s.components[0].settings.display_mode_names.clear();
    Ok(doc)
}
fn check_name(value: &str) -> Result<(), String> {
    if value.trim().is_empty()
        || value.len() > 255
        || value.chars().any(|c| c.is_control() || c == '|')
    {
        return Err("A library item needs a nonempty name of at most 255 bytes without control characters or '|'".into());
    }
    Ok(())
}
pub fn validate(doc: &Document, book: &Book) -> Result<(), String> {
    if book.others.len() >= 10_000 {
        return Err("Library member limit exceeded".into());
    }
    let mut keys = BTreeSet::new();
    for member in members(doc) {
        if !std::ptr::eq(member, doc) {
            if member.library.is_some() || member.kind != doc.kind {
                return Err("Nested or mixed library documents are invalid".into());
            }
            member.validate()?;
        }
        check_name(name(member))?;
        if !keys.insert((name(member).to_lowercase(), variant(member))) {
            return Err("Duplicate library name/part/display mode".into());
        }
        match member.kind {
            DocumentKind::Schematic => {
                let s = member.schematic.as_ref().ok_or("Missing symbol scene")?;
                if s.components.len() != 1 {
                    return Err("A library symbol must contain exactly one component".into());
                }
                let c = &s.components[0];
                if c.position != Point::default()
                    || c.part == 0
                    || c.part > c.settings.part_count
                    || c.settings.part_count > 1024
                    || c.display_mode >= 32
                {
                    return Err("Invalid symbol origin, part or display mode".into());
                }
                if s.primitives
                    .iter()
                    .chain(&s.parameter_objects)
                    .any(|p| p.component != Some(c.id))
                {
                    return Err("Library graphics must belong to their symbol".into());
                }
            }
            DocumentKind::Board => {
                let b = member.pcb.as_ref().ok_or("Missing footprint scene")?;
                if b.components.len() != 1
                    || b.components[0].position != Point::default()
                    || b.components[0].rotation != 0.
                    || b.components[0].layer != pcb::TOP
                    || !b.nets.is_empty()
                    || !b.polygons.is_empty()
                    || b.items
                        .iter()
                        .any(|p| p.component != Some(b.components[0].index) || p.net.is_some())
                {
                    return Err("A footprint must have one top-side component at its local origin and no nets/polygons".into());
                }
            }
        }
    }
    Ok(())
}

pub fn member(kind: DocumentKind, item_name: &str) -> Result<Document, String> {
    check_name(item_name)?;
    let mut doc = Document::new(kind, item_name);
    doc.size = Point::mm(100, 100);
    match kind {
        DocumentKind::Schematic => {
            let s = doc.schematic.as_mut().unwrap();
            s.sheet.border = false;
            s.sheet.title_block = false;
            s.sheet.background = 0xf8fcff;
            s.sheet.grid_nm = 2_540_000;
            s.sheet.snap_nm = 2_540_000;
            s.components.push(sch::Component {
                settings: Default::default(),
                id: Uuid::new_v4(),
                source_index: edit::AUTHORED,
                position: Point::default(),
                designator: "U?".into(),
                comment: "*".into(),
                library_reference: item_name.into(),
                description: String::new(),
                part: 1,
                display_mode: 0,
                parameters: Default::default(),
            });
            let id = s.components[0].id;
            for (role, text, y) in [
                (sch::TextRole::Designator, "U?", 5_080_000),
                (sch::TextRole::Comment, "*", -5_080_000),
            ] {
                let mut p = edit::primitive(
                    sch::SchPrimitiveKind::Text {
                        position: Point { x: 0, y },
                        text: text.into(),
                        font: 0,
                        orientation: 0,
                        justification: 0,
                    },
                    Some(id),
                );
                p.text_role = Some(role);
                p.presentation.name = if role == sch::TextRole::Designator {
                    "Designator"
                } else {
                    "Comment"
                }
                .into();
                p.presentation.hidden = true;
                p.presentation.allow_sync = true;
                s.primitives.push(p);
            }
        }
        DocumentKind::Board => {
            let mut board = pcb::Board::default();
            board.outline.clear();
            board.components.push(pcb::Component {
                id: Uuid::new_v4(),
                index: 0,
                designator: ".Designator".into(),
                comment: ".Comment".into(),
                footprint: item_name.into(),
                footprint_parameters: None,
                parameters: Some(Default::default()),
                position: Point::default(),
                rotation: 0.,
                layer: pcb::TOP,
                locked: false,
                properties: Default::default(),
            });
            doc.pcb = Some(board);
        }
    }
    Ok(doc)
}
pub fn new(kind: DocumentKind, filename: &str) -> Document {
    let mut doc = member(
        kind,
        if kind == DocumentKind::Schematic {
            "Component_1"
        } else {
            "PCBCOMPONENT_1"
        },
    )
    .unwrap();
    doc.name = filename.into();
    doc.library = Some(Book {
        others: vec![],
        source: None,
    });
    doc
}
pub fn assemble(
    mut entries: Vec<Document>,
    filename: &str,
    source: Option<pcb::SourceArchive>,
) -> Result<Document, String> {
    if entries.is_empty() {
        return Err("Library has no editable members".into());
    }
    let mut doc = entries.remove(0);
    doc.name = filename.into();
    doc.library = Some(Book {
        others: entries,
        source,
    });
    doc.validate()?;
    Ok(doc)
}
fn select(doc: &mut Document, index: usize) -> Result<(), String> {
    if index == 0 {
        return Ok(());
    }
    let mut book = doc.library.take().ok_or("Not a library document")?;
    if index > book.others.len() {
        doc.library = Some(book);
        return Err("Library member no longer exists".into());
    }
    let other = &mut book.others[index - 1];
    std::mem::swap(&mut doc.schematic, &mut other.schematic);
    std::mem::swap(&mut doc.pcb, &mut other.pcb);
    std::mem::swap(&mut doc.size, &mut other.size);
    doc.library = Some(book);
    Ok(())
}
fn rename_member(doc: &mut Document, new_name: &str) {
    if let Some(s) = &mut doc.schematic {
        s.components[0].library_reference = new_name.into();
    }
    if let Some(b) = &mut doc.pcb {
        b.components[0].footprint = new_name.into();
        b.components[0]
            .properties
            .insert("PATTERN".into(), new_name.into());
    }
}
pub fn preview(doc: &Document, command: &Edit) -> Result<Document, String> {
    let book = doc.library.as_ref().ok_or("Not a library document")?;
    let old_name = name(doc).to_owned();
    let mut next = doc.clone();
    match command {
        Edit::FootprintMetadata {
            name,
            description,
            height_nm,
            parameters,
        } => {
            if !(0..=1_000_000_000).contains(height_nm) {
                return Err("Footprint height must be between zero and 1000 mm".into());
            }
            next = preview(doc, &Edit::Rename(name.clone()))?;
            let c = &mut next
                .pcb
                .as_mut()
                .ok_or("Footprint properties require a PCB library")?
                .components[0];
            let mut values = pcb::Fields::new();
            let mut keys = BTreeSet::new();
            for (key, value) in parameters {
                if key.trim().is_empty() || !keys.insert(key.to_lowercase()) {
                    return Err("Parameter names must be nonempty and unique".into());
                }
                values.insert(key.clone(), value.clone());
            }
            c.parameters = Some(values);
            c.properties
                .insert("DESCRIPTION".into(), description.clone());
            c.properties.insert(
                "HEIGHT".into(),
                format!("{:.9}mil", *height_nm as f64 / 25_400.),
            );
        }
        Edit::Models(models) => {
            let mut unique = BTreeSet::new();
            let mut current = BTreeSet::new();
            for model in models {
                check_name(&model.name)?;
                if model.kind.is_empty()
                    || !unique.insert((model.kind.to_uppercase(), model.name.to_uppercase()))
                    || model.current && !current.insert(model.kind.to_uppercase())
                {
                    return Err("Duplicate model or multiple current models of one type".into());
                }
            }
            next.schematic
                .as_mut()
                .ok_or("Component models require a schematic library")?
                .components[0]
                .settings
                .models = models.clone();
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == old_name {
                    m.schematic.as_mut().unwrap().components[0].settings.models = models.clone();
                }
            }
        }
        Edit::Metadata {
            name: new_name,
            designator,
            comment,
            description,
            parameters,
        } => {
            next = preview(doc, &Edit::Rename(new_name.clone()))?;
            if designator.trim().is_empty() {
                return Err("Default designator cannot be empty".into());
            }
            let mut values = pcb::Fields::new();
            let mut keys = BTreeSet::new();
            for (key, value) in parameters {
                if key.trim().is_empty() || !keys.insert(key.to_lowercase()) {
                    return Err("Parameter names must be nonempty and unique".into());
                }
                values.insert(key.clone(), value.clone());
            }
            values.retain(|k, _| !k.eq_ignore_ascii_case("Comment"));
            values.insert("Comment".into(), comment.clone());
            fn update(
                doc: &mut Document,
                designator: &str,
                comment: &str,
                description: &str,
                values: &pcb::Fields,
            ) -> Result<(), String> {
                let s = doc
                    .schematic
                    .as_mut()
                    .ok_or("Symbol metadata requires a schematic library")?;
                let c = &mut s.components[0];
                c.designator = designator.into();
                c.comment = sch::parameter_text(comment, values, &s.sheet.parameters);
                c.description = description.into();
                c.parameters = values.clone();
                for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
                    if let sch::SchPrimitiveKind::Text { text, .. } = &mut p.kind {
                        if p.text_role == Some(sch::TextRole::Designator) {
                            *text = designator.into();
                            p.presentation.raw_text = Some(designator.into());
                        }
                        if p.text_role == Some(sch::TextRole::Comment) {
                            *text = c.comment.clone();
                            p.presentation.raw_text = Some(comment.into());
                        }
                    }
                }
                Ok(())
            }
            update(&mut next, designator, comment, description, &values)?;
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == new_name {
                    update(m, designator, comment, description, &values)?;
                }
            }
        }
        Edit::Select(index) => select(&mut next, *index)?,
        Edit::ComponentKind(kind) => {
            if *kind > 6 || next.schematic.is_none() {
                return Err("Choose a supported schematic component type".into());
            }
            next.schematic.as_mut().unwrap().components[0].settings.kind = *kind;
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == old_name {
                    m.schematic.as_mut().unwrap().components[0].settings.kind = *kind;
                }
            }
        }
        Edit::RenameDisplayMode(value) => {
            check_name(value)?;
            let mode = variant(doc).1;
            if mode == 0 {
                return Err("Normal display mode cannot be renamed".into());
            }
            let mut seen = BTreeSet::new();
            for m in members(doc).filter(|m| name(m) == old_name) {
                let other = variant(m).1;
                if other != mode
                    && seen.insert(other)
                    && display_mode_name(m, other).eq_ignore_ascii_case(value)
                {
                    return Err("This display mode name already exists".into());
                }
            }
            next.schematic.as_mut().unwrap().components[0]
                .settings
                .display_mode_names
                .insert(mode, value.clone());
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == old_name {
                    m.schematic.as_mut().unwrap().components[0]
                        .settings
                        .display_mode_names
                        .insert(mode, value.clone());
                }
            }
        }
        Edit::RemovePart | Edit::RemoveDisplayMode => {
            let c = &doc
                .schematic
                .as_ref()
                .ok_or("Parts require a symbol library")?
                .components[0];
            let remove_part = matches!(command, Edit::RemovePart);
            if remove_part && c.settings.part_count <= 1 {
                return Err("The last part cannot be removed".into());
            }
            if !remove_part && c.display_mode == 0 {
                return Err("Normal display mode cannot be removed".into());
            }
            let removed = variant(doc);
            let matches_removed = |m: &Document| {
                name(m) == old_name
                    && if remove_part {
                        variant(m).0 == removed.0
                    } else {
                        variant(m).1 == removed.1
                    }
            };
            let replacement = members(doc)
                .enumerate()
                .skip(1)
                .filter(|(_, m)| name(m) == old_name && !matches_removed(m))
                .min_by_key(|(_, m)| {
                    let (p, mode) = variant(m);
                    (
                        u32::from(if remove_part {
                            mode != removed.1
                        } else {
                            p != removed.0
                        }),
                        if remove_part {
                            p.abs_diff(removed.0)
                        } else {
                            mode.abs_diff(removed.1)
                        },
                    )
                })
                .map(|(i, _)| i)
                .ok_or("No remaining symbol variant")?;
            select(&mut next, replacement)?;
            next.library
                .as_mut()
                .unwrap()
                .others
                .retain(|m| !matches_removed(m));
            let new_parts = c.settings.part_count - u32::from(remove_part);
            fn renumber(
                m: &mut Document,
                old_name: &str,
                removed: (u32, u32),
                remove_part: bool,
                parts: u32,
            ) {
                if name(m) == old_name {
                    let c = &mut m.schematic.as_mut().unwrap().components[0];
                    if remove_part && c.part > removed.0 {
                        c.part -= 1;
                    }
                    if !remove_part && c.display_mode > removed.1 {
                        c.display_mode -= 1;
                    }
                    if !remove_part {
                        c.settings.display_mode_names = c
                            .settings
                            .display_mode_names
                            .iter()
                            .filter(|(mode, _)| **mode != removed.1)
                            .map(|(mode, value)| {
                                (mode - u32::from(*mode > removed.1), value.clone())
                            })
                            .collect();
                    }
                    c.settings.part_count = parts;
                }
            }
            renumber(&mut next, &old_name, removed, remove_part, new_parts);
            for m in &mut next.library.as_mut().unwrap().others {
                renumber(m, &old_name, removed, remove_part, new_parts);
            }
        }
        Edit::Add(value) | Edit::Duplicate(value) => {
            check_name(value)?;
            if members(doc).any(|m| name(m).eq_ignore_ascii_case(value)) {
                return Err("This library name already exists".into());
            }
            let mut added = if matches!(command, Edit::Duplicate(_)) {
                members(doc)
                    .filter(|m| name(m) == old_name)
                    .cloned()
                    .map(|mut d| {
                        d.library = None;
                        d.id = Uuid::new_v4();
                        rename_member(&mut d, value);
                        d
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![member(doc.kind, value)?]
            };
            let index = book.others.len() + 1;
            next.library.as_mut().unwrap().others.append(&mut added);
            select(&mut next, index)?;
        }
        Edit::Rename(value) => {
            check_name(value)?;
            if members(doc).any(|m| {
                !name(m).eq_ignore_ascii_case(&old_name) && name(m).eq_ignore_ascii_case(value)
            }) {
                return Err("This library name already exists".into());
            }
            rename_member(&mut next, value);
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == old_name {
                    rename_member(m, value);
                }
            }
        }
        Edit::Remove => {
            next.library
                .as_mut()
                .unwrap()
                .others
                .retain(|m| name(m) != old_name);
            if next.library.as_ref().unwrap().others.is_empty() {
                return Err("The last library item cannot be removed".into());
            }
            select(&mut next, 1)?;
            next.library.as_mut().unwrap().others.remove(0);
        }
        Edit::AddPart | Edit::AddDisplayMode => {
            let s = doc
                .schematic
                .as_ref()
                .ok_or("Parts require a symbol library")?;
            let c = &s.components[0];
            let variants: Vec<_> = members(doc).filter(|m| name(m) == old_name).collect();
            let parts = c.settings.part_count;
            let modes = variants.iter().map(|m| variant(m).1).max().unwrap_or(0) + 1;
            let new_parts = parts + u32::from(matches!(command, Edit::AddPart));
            let new_modes = modes + u32::from(matches!(command, Edit::AddDisplayMode));
            if new_parts > 1024 || new_modes > 32 || new_parts * new_modes > 1024 {
                return Err("Symbol variant limit exceeded".into());
            }
            next.schematic.as_mut().unwrap().components[0]
                .settings
                .part_count = new_parts;
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == old_name {
                    m.schematic.as_mut().unwrap().components[0]
                        .settings
                        .part_count = new_parts;
                }
            }
            let keys: Vec<_> = if matches!(command, Edit::AddPart) {
                (0..modes).map(|mode| (new_parts, mode)).collect()
            } else {
                (1..=parts).map(|part| (part, modes)).collect()
            };
            let index = next.library.as_ref().unwrap().others.len() + 1;
            for (part, mode) in keys {
                let mut m = member(doc.kind, &old_name)?;
                let nc = &mut m.schematic.as_mut().unwrap().components[0];
                let id = nc.id;
                *nc = c.clone();
                nc.id = id;
                nc.part = part;
                nc.display_mode = mode;
                nc.settings.part_count = new_parts;
                for p in &mut m.schematic.as_mut().unwrap().primitives {
                    if let sch::SchPrimitiveKind::Text { text, .. } = &mut p.kind {
                        *text = if p.text_role == Some(sch::TextRole::Designator) {
                            c.designator.clone()
                        } else {
                            c.comment.clone()
                        };
                        p.presentation.raw_text = Some(text.clone());
                    }
                }
                next.library.as_mut().unwrap().others.push(m);
            }
            select(&mut next, index)?;
        }
        Edit::Pin(pin) => {
            let s = next
                .schematic
                .as_mut()
                .ok_or("Pins require a symbol library")?;
            if !matches!(&pin.kind, sch::SchPrimitiveKind::Pin { number, orientation, electrical, .. } if !number.trim().is_empty() && *orientation < 4 && *electrical < 8)
            {
                return Err(
                    "A pin needs a designator, a valid orientation and electrical type".into(),
                );
            }
            let mut p = pin.as_ref().clone();
            p.component = Some(s.components[0].id);
            p.source_index = edit::AUTHORED;
            if let sch::SchPrimitiveKind::Pin { name, .. } = &p.kind {
                p.presentation.name = name.clone();
            }
            if let Some(old) = s.primitives.iter_mut().find(|p| p.id == pin.id) {
                if !matches!(old.kind, sch::SchPrimitiveKind::Pin { .. }) {
                    return Err("The selected object is not a pin".into());
                }
                p.source_index = old.source_index;
                *old = p;
            } else {
                s.primitives.push(p);
            }
        }
        Edit::LinkFootprint {
            name: footprint,
            library,
            map,
        } => {
            check_name(footprint)?;
            let pins: BTreeSet<_> = members(doc)
                .filter(|m| name(m) == old_name)
                .filter_map(|m| m.schematic.as_ref())
                .flat_map(|s| &s.primitives)
                .filter_map(|p| {
                    if let sch::SchPrimitiveKind::Pin { number, .. } = &p.kind {
                        Some(number.clone())
                    } else {
                        None
                    }
                })
                .collect();
            let mut seen = BTreeSet::new();
            for (pin, pads) in map {
                if !pins.contains(pin)
                    || !seen.insert(pin)
                    || pads.is_empty()
                    || pads.iter().any(|s| s.trim().is_empty())
                {
                    return Err(
                        "Pin mapping needs existing unique pins and one or more pad designators"
                            .into(),
                    );
                }
            }
            let mut parameters =
                std::collections::BTreeMap::from([("LIBRARY".into(), library.clone())]);
            for (pin, pads) in map {
                parameters.insert(format!("PINMAP:{pin}"), pads.join(","));
            }
            let model = sch::ComponentModel {
                name: footprint.clone(),
                kind: "PCBLIB".into(),
                current: true,
                parameters,
            };
            fn link(doc: &mut Document, model: &sch::ComponentModel) {
                if let Some(s) = &mut doc.schematic {
                    for m in &mut s.components[0].settings.models {
                        if m.kind.eq_ignore_ascii_case("PCBLIB") {
                            m.current = false;
                        }
                    }
                    s.components[0].settings.models.retain(|m| {
                        !(m.kind.eq_ignore_ascii_case("PCBLIB") && m.name == model.name)
                    });
                    s.components[0].settings.models.push(model.clone());
                }
            }
            link(&mut next, &model);
            for m in &mut next.library.as_mut().unwrap().others {
                if name(m) == old_name {
                    link(m, &model);
                }
            }
        }
    }
    next.validate()?;
    Ok(next)
}
impl History {
    pub fn library_edit(
        &mut self,
        doc: &mut Document,
        revision: u64,
        command: &Edit,
    ) -> Result<bool, String> {
        if revision != self.revision {
            return Err("Library changed during the operation".into());
        }
        let next = preview(doc, command)?;
        self.commit(doc, next)
    }
}

/// Newly drawn library primitives belong to the active definition.
pub fn own_new_geometry(doc: &mut Document) {
    if doc.library.is_none() {
        return;
    }
    if let Some(s) = &mut doc.schematic
        && let Some(c) = s.components.first()
    {
        for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
            if p.component.is_none() {
                p.component = Some(c.id);
            }
        }
    }
    if let Some(b) = &mut doc.pcb
        && let Some(c) = b.components.first()
    {
        for p in &mut b.items {
            if p.component.is_none() {
                p.component = Some(c.index);
            }
        }
    }
}

pub fn issues(doc: &Document) -> Vec<String> {
    let mut result = vec![];
    if let Err(e) = doc.validate() {
        result.push(e);
        return result;
    }
    for member in members(doc) {
        if let Some(s) = &member.schematic {
            let mut pins = std::collections::BTreeSet::new();
            for p in &s.primitives {
                if let sch::SchPrimitiveKind::Pin { number, .. } = &p.kind
                    && (number.trim().is_empty() || !pins.insert(number))
                {
                    result.push(format!("{}: empty or duplicate pin {number}", name(member)));
                }
            }
        } else if let Some(b) = &member.pcb {
            let count = b
                .items
                .iter()
                .filter(|i| matches!(i.shape, pcb::Shape::Pad { .. }))
                .count();
            if count == 0 {
                result.push(format!("{}: footprint has no pads", name(member)));
            }
        }
    }
    result
}
