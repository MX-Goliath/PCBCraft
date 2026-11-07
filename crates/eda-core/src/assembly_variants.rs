//! Assembly variants are project data. Projections never change the base design.
#[cfg(test)]
#[path = "assembly_variants_tests.rs"]
mod tests;
use crate::{
    Document, Point,
    schematic::*,
    schematic_edit::{self as edit, Fragment},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Variation {
    pub uid: String,
    pub designator: String,
    /// Native Kind: 0 fitted, 1 not fitted, 2 alternate part.
    pub kind: u32,
    pub parameters: BTreeMap<String, String>,
    pub alternate: Option<Fragment>,
    pub fields: BTreeMap<String, String>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    pub id: String,
    pub name: String,
    pub parameters: BTreeMap<String, String>,
    /// Opaque native parameter properties, keyed by case-folded parameter name.
    #[serde(default)]
    pub parameter_fields: BTreeMap<String, BTreeMap<String, String>>,
    pub variations: Vec<Variation>,
    pub fields: BTreeMap<String, String>,
}
impl Variant {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string().to_uppercase(),
            name,
            parameters: BTreeMap::new(),
            parameter_fields: BTreeMap::new(),
            variations: vec![],
            fields: BTreeMap::new(),
        }
    }
    pub fn variation(&self, uid: &str, designator: &str) -> Option<&Variation> {
        self.variations
            .iter()
            .find(|v| !v.uid.trim_matches('\\').is_empty() && same_uid(&v.uid, uid))
            .or_else(|| {
                self.variations.iter().find(|v| {
                    v.uid.trim_matches('\\').is_empty()
                        && v.designator.eq_ignore_ascii_case(designator)
                })
            })
    }
    pub fn variation_mut(&mut self, uid: &str, designator: &str) -> &mut Variation {
        let n = if uid.trim_matches('\\').is_empty() {
            self.variations
                .iter()
                .position(|v| v.designator.eq_ignore_ascii_case(designator))
        } else {
            self.variations
                .iter()
                .position(|v| !v.uid.trim_matches('\\').is_empty() && same_uid(&v.uid, uid))
                .or_else(|| {
                    self.variations.iter().position(|v| {
                        v.uid.trim_matches('\\').is_empty()
                            && v.designator.eq_ignore_ascii_case(designator)
                    })
                })
        };
        let n = n.unwrap_or_else(|| {
            self.variations.push(Variation {
                uid: uid.into(),
                designator: designator.into(),
                ..Default::default()
            });
            self.variations.len() - 1
        });
        &mut self.variations[n]
    }

    pub fn unique_name(variants: &[Variant], stem: &str) -> String {
        let stem = stem.trim();
        (0..)
            .map(|n| {
                if n == 0 {
                    stem.to_owned()
                } else {
                    format!("{stem} {n}")
                }
            })
            .find(|name| !variants.iter().any(|v| v.name.eq_ignore_ascii_case(name)))
            .unwrap()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentRef {
    pub uid: String,
    pub designator: String,
}

/// Commands are independent of the table selection and committed as one undo step.
#[derive(Clone, Debug)]
pub enum Edit {
    Add(Variant),
    Replace {
        id: String,
        value: Variant,
    },
    Remove(String),
    SetKind {
        id: String,
        components: Vec<ComponentRef>,
        kind: u32,
    },
    SetParameter {
        id: String,
        components: Vec<ComponentRef>,
        name: String,
        value: Option<String>,
    },
    Reset {
        id: String,
        components: Vec<ComponentRef>,
    },
    Rebind {
        id: String,
        index: usize,
        component: ComponentRef,
    },
}

pub fn edited(current: &[Variant], edit: Edit) -> Result<Vec<Variant>, String> {
    let mut next = current.to_vec();
    let id = match &edit {
        Edit::Add(_) => None,
        Edit::Replace { id, .. }
        | Edit::SetKind { id, .. }
        | Edit::SetParameter { id, .. }
        | Edit::Reset { id, .. }
        | Edit::Rebind { id, .. }
        | Edit::Remove(id) => Some(id),
    };
    let index = id
        .map(|id| {
            next.iter()
                .position(|v| &v.id == id)
                .ok_or("Variant no longer exists")
        })
        .transpose()?;
    match edit {
        Edit::Add(v) => next.push(v),
        Edit::Remove(_) => {
            next.remove(index.unwrap());
        }
        Edit::Replace { id, value } => {
            if id != value.id {
                return Err("Editing a variant cannot change its identity".into());
            }
            next[index.unwrap()] = value;
        }
        Edit::SetKind {
            components, kind, ..
        } => {
            if kind > 1 {
                return Err(
                    "Choose and validate an alternate component before using Alternate Part".into(),
                );
            }
            for c in components {
                let change = next[index.unwrap()].variation_mut(&c.uid, &c.designator);
                change.kind = kind;
                // Retain a selected alternate for a later switch, like the native manager.
            }
        }
        Edit::SetParameter {
            components,
            name,
            value,
            ..
        } => {
            let name = name.trim();
            if name.is_empty() {
                return Err("A parameter name is required".into());
            }
            for c in components {
                let values = &mut next[index.unwrap()]
                    .variation_mut(&c.uid, &c.designator)
                    .parameters;
                values.retain(|key, _| !key.eq_ignore_ascii_case(name));
                if let Some(value) = &value {
                    values.insert(name.into(), value.clone());
                }
            }
        }
        Edit::Reset { components, .. } => {
            next[index.unwrap()].variations.retain(|v| {
                !components.iter().any(|c| {
                    if v.uid.trim_matches('\\').is_empty() {
                        v.designator.eq_ignore_ascii_case(&c.designator)
                    } else {
                        same_uid(&v.uid, &c.uid)
                    }
                })
            });
        }
        Edit::Rebind {
            index: row,
            component,
            ..
        } => {
            let change = next[index.unwrap()]
                .variations
                .get_mut(row)
                .ok_or("Variation no longer exists")?;
            change.uid = component.uid;
            change.designator = component.designator;
        }
    }
    validate(&next)?;
    Ok(next)
}

/// A stale UID is never silently rebound by designator. Ambiguity needs a choice.
pub fn unresolved(variant: &Variant, components: &[ComponentRef]) -> Vec<usize> {
    variant
        .variations
        .iter()
        .enumerate()
        .filter_map(|(n, v)| {
            let count = components
                .iter()
                .filter(|c| {
                    if v.uid.trim_matches('\\').is_empty() {
                        v.designator.eq_ignore_ascii_case(&c.designator)
                    } else {
                        same_uid(&v.uid, &c.uid)
                    }
                })
                .count();
            (count != 1).then_some(n)
        })
        .collect()
}
pub fn same_uid(a: &str, b: &str) -> bool {
    a.trim_matches('\\')
        .eq_ignore_ascii_case(b.trim_matches('\\'))
}
pub fn component_uid(s: &Schematic, c: &Component, path: &str) -> String {
    let uid = s
        .records
        .iter()
        .find(|r| r.index == c.source_index && r.record_type == 1)
        .and_then(|r| r.fields.get("UNIQUEID"))
        .cloned()
        .unwrap_or_else(|| c.id.to_string());
    format!("{}\\{}", path.trim_end_matches('\\'), uid)
}
pub fn validate(variants: &[Variant]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    let mut ids = BTreeSet::new();
    if variants.len() > 1000 {
        return Err("Too many project variants".into());
    }
    for v in variants {
        if v.name.trim().is_empty()
            || v.name.trim().eq_ignore_ascii_case("[No Variations]")
            || v.name.contains(['\r', '\n'])
            || !names.insert(v.name.trim().to_lowercase())
        {
            return Err("Variant names must be nonempty and unique".into());
        }
        if v.id.is_empty() || !ids.insert(v.id.to_uppercase()) {
            return Err("Duplicate or empty variant identity".into());
        }
        let mut components = BTreeSet::new();
        for c in &v.variations {
            let key = if c.uid.trim_matches('\\').is_empty() {
                format!("designator:{}", c.designator.to_uppercase())
            } else {
                c.uid.trim_matches('\\').to_uppercase()
            };
            if !components.insert(key) {
                return Err(format!("Duplicate variation for {}", c.designator));
            }
            if c.kind > 2 {
                return Err(format!("Unsupported variation kind {}", c.kind));
            }
            validate_parameters(&c.parameters)?;
            if let Some(a) = &c.alternate {
                if a.components.len() != 1 {
                    return Err("An alternate must contain one component unit".into());
                }
                let mut d = Document::new(crate::DocumentKind::Schematic, "Alternate validation");
                if let Some(s) = &mut d.schematic {
                    s.fonts = a.fonts.clone();
                    s.components = a.components.clone();
                    s.primitives = a.primitives.clone();
                    s.images = a.images.clone();
                }
                d.validate()?;
            }
        }
        validate_parameters(&v.parameters)?;
    }
    Ok(())
}

fn validate_parameters(parameters: &BTreeMap<String, String>) -> Result<(), String> {
    let mut names = BTreeSet::new();
    for key in parameters.keys() {
        if key.trim().is_empty() || !names.insert(key.trim().to_lowercase()) {
            return Err("Parameter names must be nonempty and unique ignoring case".into());
        }
    }
    Ok(())
}

/// Each commit is validated before changing project state. Selection is view state.
#[derive(Default)]
pub struct History {
    past: Vec<Vec<Variant>>,
    future: Vec<Vec<Variant>>,
    pub revision: u64,
}
impl History {
    pub fn commit(
        &mut self,
        current: &mut Vec<Variant>,
        next: Vec<Variant>,
    ) -> Result<bool, String> {
        validate(&next)?;
        if *current == next {
            return Ok(false);
        }
        self.past.push(std::mem::replace(current, next));
        if self.past.len() > 128 {
            self.past.remove(0);
        }
        self.future.clear();
        self.revision += 1;
        Ok(true)
    }
    pub fn undo(&mut self, current: &mut Vec<Variant>) {
        if let Some(v) = self.past.pop() {
            self.future.push(std::mem::replace(current, v));
            self.revision += 1;
        }
    }
    pub fn redo(&mut self, current: &mut Vec<Variant>) {
        if let Some(v) = self.future.pop() {
            self.past.push(std::mem::replace(current, v));
            self.revision += 1;
        }
    }
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

fn assign(values: &mut BTreeMap<String, String>, key: &str, value: &str) {
    let key = values
        .keys()
        .find(|k| k.eq_ignore_ascii_case(key))
        .cloned()
        .unwrap_or_else(|| key.into());
    values.insert(key, value.into());
}

/// Resolve assembly metadata without graphics, for BOMs and reports. Electrical
/// connectivity deliberately continues to use the base document.
pub fn assembly_component(
    sheet: &Schematic,
    component: &Component,
    variant: Option<&Variant>,
    path: &str,
) -> Result<Option<Component>, String> {
    let change = variant.and_then(|v| {
        v.variation(
            &component_uid(sheet, component, path),
            &component.designator,
        )
    });
    if change.is_some_and(|c| c.kind == 1) {
        return Ok(None);
    }
    let mut result = if let Some(change) = change.filter(|c| c.kind == 2) {
        let alternate = change
            .alternate
            .as_ref()
            .and_then(|a| a.components.first())
            .ok_or_else(|| {
                format!(
                    "Alternate component unavailable for {}",
                    component.designator
                )
            })?;
        let mut c = alternate.clone();
        c.id = component.id;
        c.designator = component.designator.clone();
        c.position = component.position;
        c
    } else {
        component.clone()
    };
    if let Some(change) = change {
        for (key, value) in &change.parameters {
            if key.eq_ignore_ascii_case("Comment") {
                result.comment = value.clone();
            }
            if key.eq_ignore_ascii_case("Description") {
                result.description = value.clone();
            }
            assign(&mut result.parameters, key, value);
        }
    }
    let mut sheet_parameters = sheet.sheet.parameters.clone();
    if let Some(variant) = variant {
        for (key, value) in &variant.parameters {
            assign(&mut sheet_parameters, key, value);
        }
        assign(&mut sheet_parameters, "VariantName", &variant.name);
    }
    result.comment = parameter_text(&result.comment, &result.parameters, &sheet_parameters);
    Ok(Some(result))
}
fn pin_signature(primitives: &[SchPrimitive], owner: Uuid) -> Vec<(String, Point, u32)> {
    let mut p = primitives
        .iter()
        .filter(|p| p.component == Some(owner))
        .filter_map(|p| {
            if let SchPrimitiveKind::Pin {
                number, electrical, ..
            } = &p.kind
            {
                Some((number.clone(), edit::electrical_endpoint(p)?, *electrical))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    p.sort_by(|a, b| a.0.cmp(&b.0));
    p
}
/// Place a local library fragment without changing the physical pin connections.
pub fn alternate_for(s: &Schematic, c: &Component, local: &Fragment) -> Result<Fragment, String> {
    let original = local
        .components
        .first()
        .ok_or("Alternate has no component")?;
    let mut a = local.transformed(
        original.position,
        (c.settings.rotation + 4 - original.settings.rotation % 4) % 4,
        Point {
            x: c.position.x - original.position.x,
            y: c.position.y - original.position.y,
        },
    )?;
    let owner = a.components[0].id;
    if pin_signature(&s.primitives, c.id) != pin_signature(&a.primitives, owner) {
        return Err(
            "Alternate pins must have the same numbers, connection points and electrical types"
                .into(),
        );
    }
    a.components[0].id = c.id;
    a.components[0].designator = c.designator.clone();
    for p in &mut a.primitives {
        if p.component == Some(owner) {
            p.component = Some(c.id);
        }
        if p.text_role == Some(TextRole::Designator) {
            p.presentation.raw_text = Some(c.designator.clone());
            if let SchPrimitiveKind::Text { text, .. } = &mut p.kind {
                *text = c.designator.clone();
            }
        }
        if let SchPrimitiveKind::Pin { number, .. } = &p.kind
            && let Some(base) = s.primitives.iter().find(|b| {
                b.component == Some(c.id)
                    && matches!(&b.kind,SchPrimitiveKind::Pin{number:d,..} if d==number)
            })
        {
            p.id = base.id;
        }
    }
    Ok(a)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayStyle {
    pub cross: bool,
    pub dim: bool,
    pub color: u32,
    pub width_nm: i64,
    pub mesh: bool,
    pub not_fitted_text: String,
    pub modify_parameters: bool,
    pub parameter_color: u32,
    pub parameter_italic: bool,
    pub parameter_bold: bool,
    pub parameter_suffix: String,
    pub parameter_font_name: String,
    pub parameter_height_nm: i64,
}
impl Default for DisplayStyle {
    fn default() -> Self {
        Self {
            cross: true,
            dim: true,
            color: 255,
            width_nm: 508_000,
            mesh: false,
            not_fitted_text: String::new(),
            modify_parameters: true,
            parameter_color: 0x008000,
            parameter_italic: true,
            parameter_bold: true,
            parameter_suffix: String::new(),
            parameter_font_name: "Times New Roman".into(),
            parameter_height_nm: 2_540_000,
        }
    }
}
impl DisplayStyle {
    /// Upgrade the previous untouched defaults; retain customized appearances.
    pub fn upgrade_legacy_defaults(&mut self) {
        let mut migrated = self.clone();
        migrated.dim = true;
        migrated.parameter_bold = true;
        migrated.width_nm = 508_000;
        if self.width_nm == 200_000 && migrated == Self::default() {
            *self = Self::default();
        }
    }
}

fn parameter(p: &SchPrimitive) -> bool {
    matches!(p.kind, SchPrimitiveKind::Text { .. })
        && (p.text_role.is_some()
            || p.presentation.record_type == 41
            || !p.presentation.name.is_empty())
}

// The physical schematic masks electrical context and not-fitted symbols. This
// is independent of whether the optional cross/mesh overlay is enabled.
fn mask(p: &mut SchPrimitive) {
    p.color = 0xc0c0c0;
    p.fill = p.fill.map(|_| 0xe8e8e8);
    p.presentation.text_color = Some(p.color);
    match &mut p.kind {
        SchPrimitiveKind::HarnessEntry { text_color, .. }
        | SchPrimitiveKind::SheetEntry { text_color, .. } => *text_color = p.color,
        _ => {}
    }
}

/// Read-only assembly view. Base electrical objects, identities and wires survive.
pub fn project(
    document: &Document,
    variant: Option<&Variant>,
    path: &str,
    style: &DisplayStyle,
) -> Result<Document, String> {
    let mut d = document.clone();
    let s = d.schematic.as_mut().ok_or("Not a schematic")?;
    let base = s.clone();
    for p in s.primitives.iter_mut().chain(&mut s.parameter_objects) {
        if p.component.is_none()
            || variant.is_none() && parameter(p) && p.text_role != Some(TextRole::Designator)
        {
            mask(p);
        }
    }
    let Some(v) = variant else { return Ok(d) };
    for (key, value) in &v.parameters {
        assign(&mut s.sheet.parameters, key, value);
    }
    assign(&mut s.sheet.parameters, "VariantName", &v.name);
    for c in &base.components {
        let uid = component_uid(&base, c, path);
        let unchanged = Variation::default();
        let change = v.variation(&uid, &c.designator).unwrap_or(&unchanged);
        if change.kind == 2 {
            let local = change
                .alternate
                .as_ref()
                .ok_or_else(|| format!("Alternate symbol unavailable for {}", c.designator))?;
            let alt = alternate_for(&base, c, local)?;
            let font_offset = s.fonts.len();
            s.fonts.extend(alt.fonts.clone());
            // Library parameter coordinates/visibility are defaults for placing
            // a new part. A variant preserves the sheet's existing annotations.
            s.primitives
                .retain(|p| p.component != Some(c.id) || parameter(p));
            s.parameter_objects
                .retain(|p| p.component != Some(c.id) || parameter(p));
            let font_map = (0..alt.fonts.len())
                .map(|n| n + font_offset)
                .collect::<Vec<_>>();
            for mut p in alt.primitives.clone() {
                if parameter(&p) {
                    continue;
                }
                edit::remap_fonts(&mut p.kind, &font_map)?;
                s.primitives.push(p);
            }
            let target = s.components.iter_mut().find(|x| x.id == c.id).unwrap();
            *target = alt.components[0].clone();
            target.source_index = c.source_index;
            let images = std::sync::Arc::make_mut(&mut s.images);
            images.extend(alt.images.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        let target = s.components.iter_mut().find(|x| x.id == c.id).unwrap();
        for (key, value) in &change.parameters {
            if key.eq_ignore_ascii_case("Comment") {
                target.comment = value.clone();
            }
            if key.eq_ignore_ascii_case("Description") {
                target.description = value.clone();
            }
            assign(&mut target.parameters, key, value);
        }
        for p in s
            .primitives
            .iter_mut()
            .chain(&mut s.parameter_objects)
            .filter(|p| p.component == Some(c.id))
        {
            if change.kind == 1 {
                mask(p);
            }
            if let SchPrimitiveKind::Text { text, font, .. } = &mut p.kind {
                let name = if p.text_role == Some(TextRole::Comment) {
                    "Comment"
                } else {
                    p.presentation.name.as_str()
                };
                let changed = change
                    .parameters
                    .keys()
                    .any(|k| k.eq_ignore_ascii_case(name));
                let raw = change
                    .parameters
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(name))
                    .map(|(_, v)| v.as_str())
                    .or_else(|| {
                        (change.kind == 2)
                            .then(|| {
                                if p.text_role == Some(TextRole::Comment) {
                                    Some(target.comment.as_str())
                                } else {
                                    target
                                        .parameters
                                        .iter()
                                        .find(|(k, _)| k.eq_ignore_ascii_case(name))
                                        .map(|(_, v)| v.as_str())
                                }
                            })
                            .flatten()
                    })
                    .or(p.presentation.raw_text.as_deref())
                    .unwrap_or(text);
                let next = parameter_text(raw, &target.parameters, &s.sheet.parameters);
                let changed = changed || *text != next;
                *text = next;
                if changed && style.modify_parameters {
                    text.push_str(&style.parameter_suffix);
                    p.color = style.parameter_color;
                    p.presentation.text_color = Some(style.parameter_color);
                    if let Some(mut f) = s.fonts.get(*font).cloned() {
                        f.italic = style.parameter_italic;
                        f.bold = style.parameter_bold;
                        f.name = style.parameter_font_name.clone();
                        f.height_nm = style.parameter_height_nm.clamp(254_000, 25_400_000);
                        f.underline = false;
                        *font = s.fonts.iter().position(|old| old == &f).unwrap_or_else(|| {
                            s.fonts.push(f);
                            s.fonts.len() - 1
                        });
                    }
                }
            }
        }
        if change.kind == 1 && (style.cross || style.mesh || !style.not_fitted_text.is_empty()) {
            let bounds = base
                .primitives
                .iter()
                .filter(|p| {
                    p.component == Some(c.id)
                        && !p.presentation.hidden
                        && !matches!(p.kind, SchPrimitiveKind::Text { .. })
                })
                .filter_map(|p| crate::schematic_arrange::primitive_bounds(&base, p))
                .fold(None::<crate::schematic_arrange::Bounds>, |b, p| {
                    Some(b.map_or(p, |b| crate::schematic_arrange::Bounds {
                        min: Point {
                            x: b.min.x.min(p.min.x),
                            y: b.min.y.min(p.min.y),
                        },
                        max: Point {
                            x: b.max.x.max(p.max.x),
                            y: b.max.y.max(p.max.y),
                        },
                    }))
                });
            if let Some(mut b) = bounds {
                let x_margin = (b.max.x - b.min.x) / 20;
                let y_margin = (b.max.y - b.min.y) / 20;
                b.min.x -= x_margin;
                b.max.x += x_margin;
                b.min.y -= y_margin;
                b.max.y += y_margin;
                let mut lines = vec![];
                if style.cross {
                    lines.extend(
                        [
                            vec![b.min, b.max],
                            vec![
                                Point {
                                    x: b.min.x,
                                    y: b.max.y,
                                },
                                Point {
                                    x: b.max.x,
                                    y: b.min.y,
                                },
                            ],
                        ]
                        .map(|points| (points, style.color, style.width_nm, 0)),
                    );
                }
                if style.mesh {
                    lines.push((
                        vec![
                            b.min,
                            Point {
                                x: b.max.x,
                                y: b.min.y,
                            },
                            b.max,
                            Point {
                                x: b.min.x,
                                y: b.max.y,
                            },
                            b.min,
                        ],
                        0x808080,
                        25_400,
                        0,
                    ));
                    let height = b.max.y - b.min.y;
                    let width = b.max.x - b.min.x;
                    let step = 1_270_000.max((width + height) / 100);
                    let mut offset = step;
                    while offset < width + height {
                        let points = vec![
                            Point {
                                x: b.min.x + offset.min(width),
                                y: b.min.y + (offset - width).max(0),
                            },
                            Point {
                                x: b.min.x + (offset - height).max(0),
                                y: b.min.y + offset.min(height),
                            },
                        ];
                        let reflected = points
                            .iter()
                            .map(|p| Point {
                                x: b.min.x + b.max.x - p.x,
                                y: p.y,
                            })
                            .collect();
                        lines.push((points, 0x808080, 25_400, 0));
                        lines.push((reflected, 0x808080, 25_400, 2));
                        offset += step;
                    }
                }
                for (points, color, width, line_style) in lines {
                    let mut p = edit::primitive(
                        SchPrimitiveKind::Polyline {
                            points,
                            closed: false,
                            electrical: String::new(),
                        },
                        Some(c.id),
                    );
                    p.color = color;
                    p.width_nm = width;
                    p.line_style = line_style;
                    s.primitives.push(p);
                }
                if !style.not_fitted_text.is_empty() {
                    let mut p = edit::primitive(
                        SchPrimitiveKind::Text {
                            position: Point {
                                x: b.min.x + (b.max.x - b.min.x) / 2,
                                y: b.min.y + (b.max.y - b.min.y) / 2,
                            },
                            text: style.not_fitted_text.clone(),
                            font: 0,
                            orientation: 0,
                            justification: 4,
                        },
                        Some(c.id),
                    );
                    p.color = style.color;
                    s.primitives.push(p);
                }
            }
        }
    }
    // Resolve sheet special strings after variant parameter precedence is applied.
    for p in s.primitives.iter_mut().filter(|p| p.component.is_none()) {
        if let SchPrimitiveKind::Text { text, .. } = &mut p.kind {
            let raw = p.presentation.raw_text.as_deref().unwrap_or(text);
            *text = parameter_text(raw, &BTreeMap::new(), &s.sheet.parameters);
        }
    }
    Ok(d)
}
