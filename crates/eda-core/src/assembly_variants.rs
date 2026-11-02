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
    pub variations: Vec<Variation>,
    pub fields: BTreeMap<String, String>,
}
impl Variant {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string().to_uppercase(),
            name,
            parameters: BTreeMap::new(),
            variations: vec![],
            fields: BTreeMap::new(),
        }
    }
    pub fn variation(&self, uid: &str, designator: &str) -> Option<&Variation> {
        self.variations.iter().find(|v| {
            if v.uid.trim_matches('\\').is_empty() {
                v.designator.eq_ignore_ascii_case(designator)
            } else {
                same_uid(&v.uid, uid)
            }
        })
    }
    pub fn variation_mut(&mut self, uid: &str, designator: &str) -> &mut Variation {
        let n = self.variations.iter().position(|v| {
            if uid.is_empty() || v.uid.trim_matches('\\').is_empty() {
                v.designator.eq_ignore_ascii_case(designator)
            } else {
                same_uid(&v.uid, uid)
            }
        });
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
            || v.name.eq_ignore_ascii_case("[No Variations]")
            || v.name.contains(['\r', '\n'])
            || !names.insert(v.name.to_lowercase())
        {
            return Err("Variant names must be nonempty and unique".into());
        }
        if v.id.is_empty() || !ids.insert(v.id.to_uppercase()) {
            return Err("Duplicate or empty variant identity".into());
        }
        let mut components = BTreeSet::new();
        for c in &v.variations {
            let key = if c.uid.is_empty() {
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
pub struct DisplayStyle {
    pub cross: bool,
    pub dim: bool,
    pub color: u32,
    pub width_nm: i64,
}
impl Default for DisplayStyle {
    fn default() -> Self {
        Self {
            cross: true,
            dim: false,
            color: 255,
            width_nm: 200_000,
        }
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
    let Some(v) = variant else { return Ok(d) };
    let base = s.clone();
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
            s.primitives.retain(|p| p.component != Some(c.id));
            s.parameter_objects.retain(|p| p.component != Some(c.id));
            let font_map = (0..alt.fonts.len())
                .map(|n| n + font_offset)
                .collect::<Vec<_>>();
            for mut p in alt.primitives.clone() {
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
            assign(&mut target.parameters, key, value);
        }
        for p in s
            .primitives
            .iter_mut()
            .chain(&mut s.parameter_objects)
            .filter(|p| p.component == Some(c.id))
        {
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
                    .or(p.presentation.raw_text.as_deref())
                    .unwrap_or(text);
                let next = parameter_text(raw, &target.parameters, &s.sheet.parameters);
                let changed = changed || *text != next;
                *text = next;
                if changed && let Some(mut f) = s.fonts.get(*font).cloned() {
                    f.italic = true;
                    s.fonts.push(f);
                    *font = s.fonts.len() - 1;
                }
            }
            if change.kind == 1 && style.dim {
                p.color = 0xa0a0a0;
                p.fill = p.fill.map(|_| 0xe0e0e0);
            }
        }
        if change.kind == 1 && style.cross {
            let bounds = base
                .primitives
                .iter()
                .filter(|p| {
                    p.component == Some(c.id) && !p.presentation.hidden && p.text_role.is_none()
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
            if let Some(b) = bounds {
                for points in [
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
                ] {
                    let mut p = edit::primitive(
                        SchPrimitiveKind::Polyline {
                            points,
                            closed: false,
                            electrical: String::new(),
                        },
                        Some(c.id),
                    );
                    p.color = style.color;
                    p.width_nm = style.width_nm;
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
