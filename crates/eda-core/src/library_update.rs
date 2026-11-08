//! Checked updates of placed schematic components from local library definitions.
//! Instances retain their identities, annotation, placement and external wires.
use crate::schematic::{Component, SchPrimitive, SchPrimitiveKind as K, TextRole};
use crate::schematic_edit::{self as edit, Fragment};
use crate::{Document, History};
use eda_geom::Point;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub graphics: bool,
    pub parameters: bool,
    pub models: bool,
    pub designator_lock: bool,
    pub part_lock: bool,
    pub preserve_locations: bool,
    pub preserve_visibility: bool,
    pub add_parameters: bool,
    pub remove_parameters: bool,
    /// Case-insensitive parameter names, including Comment, Description and Component Kind.
    pub excluded_parameters: BTreeSet<String>,
    /// Direct SchLib updates relink the instance to this source library.
    pub source_library: Option<String>,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            graphics: true,
            parameters: true,
            models: true,
            designator_lock: true,
            part_lock: true,
            preserve_locations: true,
            preserve_visibility: true,
            add_parameters: true,
            remove_parameters: true,
            excluded_parameters: BTreeSet::new(),
            source_library: None,
        }
    }
}
impl Options {
    pub fn updates(&self, name: &str) -> bool {
        self.parameters
            && !name.eq_ignore_ascii_case("Designator")
            && !self
                .excluded_parameters
                .iter()
                .any(|n| n.eq_ignore_ascii_case(name))
    }
}
#[derive(Clone, Debug)]
pub struct Update {
    pub component: Uuid,
    pub definition: Fragment,
    pub options: Options,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterChange {
    pub name: String,
    pub sheet: Option<String>,
    pub library: Option<String>,
}
pub fn parameter_changes(
    c: &Component,
    definition: &Fragment,
) -> Result<Vec<ParameterChange>, String> {
    let source = definition
        .components
        .first()
        .ok_or("Library component is missing")?;
    let mut changes = vec![];
    let mut names = BTreeSet::new();
    for name in c.parameters.keys().chain(source.parameters.keys()) {
        if name.eq_ignore_ascii_case("Designator") || !names.insert(name.to_lowercase()) {
            continue;
        }
        let value = |component: &Component| {
            component
                .parameters
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.clone())
        };
        let (sheet, library) = (value(c), value(source));
        if sheet != library {
            changes.push(ParameterChange {
                name: name.clone(),
                sheet,
                library,
            });
        }
    }
    for (name, sheet, library) in [
        (
            "Description",
            c.description.clone(),
            source.description.clone(),
        ),
        (
            "Component Kind",
            c.settings.kind.to_string(),
            source.settings.kind.to_string(),
        ),
    ] {
        if sheet != library {
            changes.push(ParameterChange {
                name: name.into(),
                sheet: Some(sheet),
                library: Some(library),
            });
        }
    }
    Ok(changes)
}
fn annotation(p: &SchPrimitive) -> bool {
    matches!(p.kind, K::Text { .. })
        && (p.text_role.is_some()
            || p.presentation.record_type == 41
            || !p.presentation.name.is_empty())
}
fn annotation_name(p: &SchPrimitive) -> &str {
    match p.text_role {
        Some(TextRole::Designator) => "Designator",
        Some(TextRole::Comment) => "Comment",
        _ => &p.presentation.name,
    }
}
fn pins(primitives: &[SchPrimitive], owner: Uuid) -> Vec<(String, Point)> {
    let mut pins: Vec<_> = primitives
        .iter()
        .filter(|p| p.component == Some(owner))
        .filter_map(|p| {
            if let K::Pin { number, .. } = &p.kind {
                Some((number.clone(), edit::electrical_endpoint(p)?))
            } else {
                None
            }
        })
        .collect();
    pins.sort_by_key(|(n, p)| (n.clone(), p.x, p.y));
    pins
}
fn placed(c: &Component, definition: &Fragment) -> Result<Fragment, String> {
    if definition.components.len() != 1 {
        return Err("A library definition must contain exactly one component".into());
    }
    let source = &definition.components[0];
    if source.part != c.part || source.display_mode != c.display_mode {
        return Err("The library does not contain the instance part and display mode".into());
    }
    if definition
        .primitives
        .iter()
        .any(|p| p.component != Some(source.id) || p.presentation.parent.is_some())
    {
        return Err("Nested or unowned library primitives cannot be updated yet".into());
    }
    let mut f = definition.fresh_copy();
    f = f.transformed(
        source.position,
        (c.settings.rotation + 4 - source.settings.rotation % 4) % 4,
        Point {
            x: c.position.x - source.position.x,
            y: c.position.y - source.position.y,
        },
    )?;
    let owner = f.components[0].id;
    f.components[0].id = c.id;
    for p in &mut f.primitives {
        if p.component == Some(owner) {
            p.component = Some(c.id);
        }
    }
    Ok(f)
}
/// Warn before applying updates that move, add or remove electrical connection points.
pub fn pin_connections_change(doc: &Document, update: &Update) -> Result<bool, String> {
    let s = doc
        .schematic
        .as_ref()
        .ok_or("A schematic document is required")?;
    let c = s
        .components
        .iter()
        .find(|c| c.id == update.component)
        .ok_or("Component is missing")?;
    let f = placed(c, &update.definition)?;
    Ok(pins(&s.primitives, c.id) != pins(&f.primitives, c.id))
}
/// Prepare all updates before committing. Any invalid definition leaves the document untouched.
pub fn preview(doc: &Document, updates: &[Update]) -> Result<Document, String> {
    if doc.library.is_some() {
        return Err("Update placed components in a schematic sheet".into());
    }
    let mut next = doc.clone();
    let mut seen = BTreeSet::new();
    for update in updates {
        if !seen.insert(update.component) {
            return Err("A component is listed more than once".into());
        }
        apply(&mut next, update)?;
    }
    if next != *doc
        && let Some(s) = &mut next.schematic
        && s.source.is_some()
    {
        s.source_modified = true;
    }
    edit::preview(&next, &edit::Edit::Batch(vec![]))
}
impl History {
    pub fn update_from_library(
        &mut self,
        doc: &mut Document,
        expected_revision: u64,
        updates: &[Update],
    ) -> Result<bool, String> {
        if self.revision != expected_revision {
            return Err("The schematic changed; compare with the library again".into());
        }
        let next = preview(doc, updates)?;
        self.commit(doc, next)
    }
}
fn apply(doc: &mut Document, update: &Update) -> Result<(), String> {
    let s = doc
        .schematic
        .as_mut()
        .ok_or("A schematic document is required")?;
    let index = s
        .components
        .iter()
        .position(|c| c.id == update.component)
        .ok_or("Component is missing")?;
    let old = s.components[index].clone();
    let mut effective = update.options.clone();
    for p in s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .filter(|p| p.component == Some(old.id) && annotation(p))
    {
        if p.presentation.locked || !p.presentation.allow_sync {
            effective
                .excluded_parameters
                .insert(annotation_name(p).into());
        }
    }
    let o = &effective;
    // Native mirror state is retained in the archive but is not yet a model transform.
    if o.graphics
        && s.records
            .iter()
            .find(|r| r.index == old.source_index)
            .is_some_and(|r| {
                r.fields.iter().any(|(k, v)| {
                    k.eq_ignore_ascii_case("ISMIRRORED")
                        && (v.eq_ignore_ascii_case("T")
                            || v == "1"
                            || v.eq_ignore_ascii_case("true"))
                })
            })
    {
        return Err("Graphical update of a mirrored native symbol is not supported yet".into());
    }
    let mut definition = update.definition.clone();
    if !o.graphics {
        definition.primitives.retain(annotation);
    }
    let mut f = placed(&old, &definition)?;
    let source = &f.components[0];
    let mut target = old.clone();
    if let Some(path) = &o.source_library {
        target.settings.source = path.clone();
        target.settings.design_item_id = target.library_reference.clone();
    }
    if o.updates("Description") {
        target.description = source.description.clone();
    }
    if o.updates("Component Kind") {
        target.settings.kind = source.settings.kind;
    }
    if o.models {
        target.settings.models = source.settings.models.clone();
    }
    if o.designator_lock {
        target.settings.designator_locked = source.settings.designator_locked;
    }
    if o.part_lock {
        target.settings.part_locked = source.settings.part_locked;
    }
    if !o.graphics && !o.parameters {
        s.components[index] = target;
        return Ok(());
    }
    if o.graphics {
        let inactive = s.records.iter().any(|r| {
            let number = |name: &str| r.fields.get(name).and_then(|v| v.parse::<i64>().ok());
            number("OWNERINDEX") == Some(old.source_index as i64)
                && (number("OWNERPARTID").is_some_and(|n| n > 0 && n != old.part as i64)
                    || number("OWNERPARTDISPLAYMODE")
                        .is_some_and(|n| n >= 0 && n != old.display_mode as i64))
                && matches!(r.record_type, 2..=18 | 34 | 41)
        });
        if inactive {
            return Err("This native component retains inactive parts or display modes; their graphical update is not supported yet. Update parameters/models separately".into());
        }
        target.settings.part_count = source.settings.part_count;
        target.settings.display_mode_names = source.settings.display_mode_names.clone();
        if s.primitives
            .iter()
            .filter(|p| p.component == Some(old.id))
            .chain(&f.primitives)
            .any(|p| matches!(p.kind, K::Unsupported { .. }))
        {
            return Err(
                "The symbol contains unsupported graphics; update parameters or models separately"
                    .into(),
            );
        }
    }
    for change in parameter_changes(&old, &f)? {
        if change.name == "Description" || change.name == "Component Kind" {
            continue;
        }
        if !o.updates(&change.name)
            || change.library.is_none() && !o.remove_parameters
            || change.sheet.is_none() && !o.add_parameters
        {
            continue;
        }
        target
            .parameters
            .retain(|k, _| !k.eq_ignore_ascii_case(&change.name));
        if let Some(value) = change.library {
            target.parameters.insert(change.name, value);
        }
    }
    // Do not allocate another copy of fonts already present on the sheet.
    let mut font_map = vec![];
    for font in &f.fonts {
        let index = s.fonts.iter().position(|v| v == font).unwrap_or_else(|| {
            s.fonts.push(font.clone());
            s.fonts.len() - 1
        });
        font_map.push(index);
    }
    for p in &mut f.primitives {
        edit::remap_fonts(&mut p.kind, &font_map)?;
    }
    let old_primitives: Vec<_> = s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .filter(|p| p.component == Some(old.id))
        .cloned()
        .collect();
    let mut replacements = vec![];
    let mut used_pins = BTreeSet::new();
    if o.graphics {
        for mut p in f.primitives.iter().filter(|p| !annotation(p)).cloned() {
            if let K::Pin { number, .. } = &p.kind {
                let candidates: Vec<_> = old_primitives
                    .iter()
                    .filter(|b| {
                        !used_pins.contains(&b.id)
                            && matches!(&b.kind, K::Pin { number: n, .. } if n == number)
                    })
                    .collect();
                let existing = candidates
                    .iter()
                    .find(|b| edit::electrical_endpoint(b) == edit::electrical_endpoint(&p))
                    .copied()
                    .or_else(|| (candidates.len() == 1).then(|| candidates[0]));
                if let Some(b) = existing {
                    p.id = b.id;
                    p.source_index = b.source_index;
                    used_pins.insert(b.id);
                }
            }
            replacements.push(p);
        }
    } else {
        replacements.extend(old_primitives.iter().filter(|p| !annotation(p)).cloned());
    }
    let mut annotation_names = BTreeSet::new();
    for old_p in old_primitives.iter().filter(|p| annotation(p)) {
        let name = annotation_name(old_p);
        annotation_names.insert(name.to_lowercase());
        if old_p.presentation.locked || !old_p.presentation.allow_sync {
            replacements.push(old_p.clone());
            continue;
        }
        let designator = name.eq_ignore_ascii_case("Designator");
        let value = target
            .parameters
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.clone());
        if !designator && value.is_none() && o.updates(name) && o.remove_parameters {
            continue;
        }
        let library_p = f
            .primitives
            .iter()
            .find(|p| annotation(p) && annotation_name(p).eq_ignore_ascii_case(name));
        let mut p = old_p.clone();
        if let Some(lib) = library_p {
            if !o.preserve_locations {
                let id = p.id;
                let source_index = p.source_index;
                let hidden = p.presentation.hidden;
                p = lib.clone();
                p.id = id;
                p.source_index = source_index;
                if o.preserve_visibility {
                    p.presentation.hidden = hidden;
                }
            }
            if !o.preserve_visibility {
                p.presentation.hidden = lib.presentation.hidden;
            }
        }
        // Synchronization and locking belong to the instance. Copying library
        // defaults here would silently disable later updates or change its locks.
        p.presentation.locked = old_p.presentation.locked;
        p.presentation.allow_sync = old_p.presentation.allow_sync;
        p.presentation.auto_position = old_p.presentation.auto_position;
        if let K::Text { text, .. } = &mut p.kind {
            let value = if designator {
                Some(old.designator.clone())
            } else {
                value
            };
            if let Some(value) = value {
                p.presentation.raw_text = Some(value.clone());
                *text = value;
            }
        }
        replacements.push(p);
    }
    if o.parameters && o.add_parameters {
        for p in f.primitives.iter().filter(|p| annotation(p)) {
            let name = annotation_name(p);
            if annotation_names.insert(name.to_lowercase())
                && o.updates(name)
                && target
                    .parameters
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case(name))
            {
                replacements.push(p.clone());
            }
        }
    }
    if !o.graphics {
        let order: std::collections::HashMap<_, _> = old_primitives
            .iter()
            .enumerate()
            .map(|(n, p)| (p.id, n))
            .collect();
        replacements.sort_by_key(|p| order.get(&p.id).copied().unwrap_or(usize::MAX));
    }
    let insert_at = s
        .primitives
        .iter()
        .position(|p| p.component == Some(old.id))
        .unwrap_or(s.primitives.len());
    s.primitives.retain(|p| p.component != Some(old.id));
    s.parameter_objects.retain(|p| p.component != Some(old.id));
    let mut visible = vec![];
    for p in replacements {
        if annotation(&p) && p.presentation.hidden {
            s.parameter_objects.push(p);
        } else {
            visible.push(p);
        }
    }
    s.primitives.splice(insert_at..insert_at, visible);
    let images = std::sync::Arc::make_mut(&mut s.images);
    for (key, data) in f.images.iter() {
        if images.get(key).is_some_and(|v| v != data) {
            return Err(format!("Conflicting library image resource: {key}"));
        }
        images.insert(key.clone(), data.clone());
    }
    s.components[index] = target;
    Ok(())
}

#[cfg(test)]
#[path = "library_update_tests.rs"]
mod tests;
