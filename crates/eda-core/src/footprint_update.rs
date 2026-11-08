//! Checked replacement of placed footprint geometry from a local PCB library.
use crate::{Document, History, design_sync::Footprint, pcb::*};
use eda_geom::Point;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub text: bool,
    pub silkscreen: bool,
    pub mechanical: bool,
    pub other: bool,
    pub solder_mask: bool,
    pub paste_mask: bool,
    pub parameters: bool,
    /// Opaque native thermal overrides cannot yet be copied from another archive.
    /// Matched pads retain their original record and its unmodelled bytes.
    pub thermal_relief: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            text: false,
            silkscreen: true,
            mechanical: true,
            other: true,
            solder_mask: false,
            paste_mask: false,
            parameters: true,
            thermal_relief: false,
        }
    }
}
impl Options {
    fn includes(&self, item: &Item) -> bool {
        if matches!(item.shape, Shape::Text { .. }) {
            // Component annotation is separate from footprint text. Native
            // Update Text keeps designator/comment records and their positions.
            return self.text && !flag(item, "DESIGNATOR") && !flag(item, "COMMENT");
        }
        match item.layer {
            TOP_OVERLAY | BOTTOM_OVERLAY => self.silkscreen,
            57..=72 | 83..=1090 => self.mechanical,
            _ => self.other,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Update {
    pub component: Uuid,
    pub footprint: Footprint,
    pub options: Options,
    pub source_library: Option<String>,
}

pub fn preview(doc: &Document, updates: &[Update]) -> Result<Document, String> {
    if doc.library.is_some() {
        return Err("Update a placed PCB, not a library definition".into());
    }
    let mut next = doc.clone();
    let board = next.pcb.as_mut().ok_or("A PCB scene is required")?;
    let mut seen = BTreeSet::new();
    for update in updates {
        if !seen.insert(update.component) {
            return Err("A component is listed more than once".into());
        }
        apply(board, update)?;
    }
    if doc.pcb.as_ref() != Some(board) {
        board.pours_stale |= !board.polygon_pours.is_empty();
    }
    next.validate()?;
    Ok(next)
}
impl History {
    pub fn update_footprints(
        &mut self,
        doc: &mut Document,
        expected_revision: u64,
        updates: &[Update],
    ) -> Result<bool, String> {
        if self.revision != expected_revision {
            return Err("The PCB changed; reopen the footprint update".into());
        }
        let next = preview(doc, updates)?;
        self.commit(doc, next)
    }
}
fn pad_name(item: &Item) -> Option<String> {
    if let Shape::Pad { name, .. } = &item.shape {
        Some(name.to_uppercase())
    } else {
        None
    }
}
fn same_kind(a: &Item, b: &Item) -> bool {
    std::mem::discriminant(&a.shape) == std::mem::discriminant(&b.shape) && a.layer == b.layer
}
fn flag(item: &Item, key: &str) -> bool {
    item.properties
        .get(key)
        .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
}
fn mask_settings(item: &Item) -> MaskSettings {
    item.masks
        .as_deref()
        .cloned()
        .unwrap_or_else(|| MaskSettings::for_item(item))
}
fn updated_masks(source: &Item, target: &Item, options: &Options) -> Box<MaskSettings> {
    let original = mask_settings(target);
    let mut masks = mask_settings(source);
    if !options.solder_mask {
        masks.solder = original.solder;
        masks.linked_solder = original.linked_solder;
    }
    if !options.paste_mask {
        masks.paste = original.paste;
        masks.linked_paste = original.linked_paste;
    }
    Box::new(masks)
}
fn apply(board: &mut Board, update: &Update) -> Result<(), String> {
    let old = board
        .components
        .iter()
        .find(|c| c.id == update.component)
        .cloned()
        .ok_or("Component no longer exists")?;
    if old.locked {
        return Err(format!(
            "Unlock {} before updating its footprint",
            old.designator
        ));
    }
    if update.options.thermal_relief {
        return Err("Copying native pad thermal relief overrides is not implemented".into());
    }
    let mut part = update.footprint.board().clone();
    part.validate()?;
    if part.components.len() != 1 {
        return Err("A footprint requires exactly one component".into());
    }
    if !part.warnings.is_empty() || !part.empty_region_records.is_empty() {
        return Err(
            "The library footprint contains unsupported or archived objects; update refused".into(),
        );
    }
    let source = part.components[0].clone();
    if !source.footprint.eq_ignore_ascii_case(&old.footprint) {
        return Err(format!(
            "Footprint {} does not match {}",
            source.footprint, old.footprint
        ));
    }
    part.source = None;
    part.items.retain(|i| i.component == Some(source.index));
    part.components[0].locked = false;
    part.nets.clear();
    part.polygons.clear();
    part.polygon_pours.clear();
    for i in &mut part.items {
        i.locked = false;
        i.net = None;
        i.polygon = None;
        i.source = None;
    }
    // Normalize the library origin first, then mirror/rotate into the placed pose.
    if part.components[0].layer == BOTTOM {
        part = crate::pcb_tools::flip_component(&part, source.id)?;
    }
    let origin = part.components[0].position;
    let angle = part.components[0].rotation;
    part = part.edited(Edit::Transform {
        selection: vec![source.id],
        pivot: origin,
        translation: Point {
            x: -origin.x,
            y: -origin.y,
        },
        angle: -angle,
    })?;
    if old.layer == BOTTOM {
        part = crate::pcb_tools::flip_component(&part, source.id)?;
    }
    let angle = old.rotation - part.components[0].rotation;
    part = part.edited(Edit::Transform {
        selection: vec![source.id],
        pivot: Point::default(),
        translation: old.position,
        angle,
    })?;
    let existing: Vec<_> = board
        .items
        .iter()
        .filter(|i| i.component == Some(old.index))
        .cloned()
        .collect();
    let mask_update = update.options.solder_mask || update.options.paste_mask;
    if existing
        .iter()
        .any(|i| i.locked && (update.options.includes(i) || mask_update && pad_name(i).is_some()))
    {
        return Err(format!(
            "Unlock the primitives of {} before updating",
            old.designator
        ));
    }
    let mut nets = BTreeMap::new();
    for item in &existing {
        if let Some(name) = pad_name(item)
            && let Some(net) = nets.insert(name.clone(), item.net)
            && net != item.net
        {
            return Err(format!(
                "{} has conflicting nets on pad {name}",
                old.designator
            ));
        }
    }
    if update.options.other {
        let names: BTreeSet<_> = part.items.iter().filter_map(pad_name).collect();
        if let Some((name, _)) = nets
            .iter()
            .find(|(name, net)| net.is_some() && !names.contains(*name))
        {
            return Err(format!(
                "The library omits connected pad {}-{name}; update refused",
                old.designator
            ));
        }
    }
    let mut used = BTreeSet::new();
    let mut replacements = vec![];
    for mut item in part
        .items
        .iter()
        .filter(|i| update.options.includes(i))
        .cloned()
    {
        let name = pad_name(&item);
        let matched = existing.iter().enumerate().find(|(n, i)| {
            !used.contains(n)
                && update.options.includes(i)
                && if name.is_some() {
                    pad_name(i) == name
                } else {
                    same_kind(i, &item)
                        && flag(i, "DESIGNATOR") == flag(&item, "DESIGNATOR")
                        && flag(i, "COMMENT") == flag(&item, "COMMENT")
                }
        });
        item.id = Uuid::new_v4();
        item.component = Some(old.index);
        item.properties.remove("UNIQUEID");
        item.source = None;
        if let Some((n, previous)) = matched {
            used.insert(n);
            item.id = previous.id;
            item.source = previous.source.clone();
            item.locked = previous.locked;
            if let Some(uid) = previous.properties.get("UNIQUEID") {
                item.properties.insert("UNIQUEID".into(), uid.clone());
            }
            if name.is_some() {
                item.masks = Some(updated_masks(&item, previous, &update.options));
            }
        }
        if let Some(name) = name {
            item.net = nets.get(&name).copied().flatten();
        }
        if let Shape::Text { text, .. } = &mut item.shape {
            if item
                .properties
                .get("DESIGNATOR")
                .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            {
                *text = old.designator.clone();
            }
            if item
                .properties
                .get("COMMENT")
                .is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
            {
                *text = old.comment.clone();
            }
        }
        replacements.push(item);
    }
    // Pad attributes are an independent native option; they can be updated
    // while On Other Layers is off and the pad geometry stays local.
    if mask_update && !update.options.other {
        let mut used = BTreeSet::new();
        for item in board
            .items
            .iter_mut()
            .filter(|i| i.component == Some(old.index))
        {
            let Some(name) = pad_name(item) else { continue };
            if let Some((n, source)) = part
                .items
                .iter()
                .enumerate()
                .find(|(n, p)| !used.contains(n) && pad_name(p).as_ref() == Some(&name))
            {
                used.insert(n);
                item.masks = Some(updated_masks(source, item, &update.options));
            }
        }
    }
    // Preserve unselected categories byte for byte, including local annotations.
    board
        .items
        .retain(|i| i.component != Some(old.index) || !update.options.includes(i));
    board.items.extend(replacements);
    for layer in part.layers {
        if !board.layers.iter().any(|l| l.id == layer.id) {
            board.layers.push(layer);
        }
    }
    for (key, model) in part.models {
        if board.models.get(&key).is_some_and(|m| m != &model) {
            return Err(format!("Conflicting 3D model {key}"));
        }
        board.models.insert(key, model);
    }
    let target = board
        .components
        .iter_mut()
        .find(|c| c.id == old.id)
        .unwrap();
    // Library metadata is distinct from schematic source description/linkage.
    for key in ["HEIGHT", "FOOTPRINTDESCRIPTION"] {
        target.properties.remove(key);
        let value = source.properties.get(key).or_else(|| {
            (key == "FOOTPRINTDESCRIPTION")
                .then(|| source.properties.get("DESCRIPTION"))
                .flatten()
        });
        if let Some(value) = value {
            target.properties.insert(key.into(), value.clone());
        }
    }
    if update.options.parameters {
        let values = source
            .parameters
            .ok_or("The source footprint parameters are unavailable")?;
        let mut names = BTreeSet::new();
        if values
            .keys()
            .any(|key| key.trim().is_empty() || !names.insert(key.to_uppercase()))
        {
            return Err("Footprint parameter names must be nonempty and unique".into());
        }
        target.footprint_parameters = Some(values);
    }
    if let Some(path) = &update.source_library {
        target
            .properties
            .insert("SOURCEFOOTPRINTLIBRARY".into(), path.clone());
    }
    Ok(())
}

#[cfg(test)]
#[path = "footprint_update_tests.rs"]
mod tests;
