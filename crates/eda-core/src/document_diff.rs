//! Read-only semantic comparison. Native source indexes and byte-derived UUIDs
//! are not identities: inserting a record must not recolor an entire board.
use crate::{Document, pcb::Bounds};
use eda_geom::Point;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Removed,
    Moved,
    Modified,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Removed => "Removed",
            Self::Moved => "Moved",
            Self::Modified => "Parameters",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Target {
    pub ids: Vec<Uuid>,
    pub bounds: Bounds,
}
#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub before: String,
    pub after: String,
}
#[derive(Clone, Debug)]
pub struct Change {
    pub label: String,
    pub kind: Kind,
    pub moved: bool,
    pub before: Option<Target>,
    pub after: Option<Target>,
    pub fields: Vec<Field>,
}
#[derive(Clone, Debug, Default)]
pub struct Diff {
    pub changes: Vec<Change>,
}
struct Entity {
    key: Option<String>,
    category: String,
    label: String,
    target: Target,
    value: Value,
    relative: Value,
    anchor: Point,
}
fn clean(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.contains_key("id") || map.contains_key("shape") {
                for key in ["id", "source_index", "source", "component", "index"] {
                    map.remove(key);
                }
            } else if map.contains_key("index") && map.contains_key("name") {
                map.remove("index");
            }
            if let Some(Value::Object(p)) = map.get_mut("presentation") {
                p.remove("parent");
            }
            if let Some(Value::Object(p)) = map.get_mut("properties") {
                // These native fields duplicate typed geometry/identity. Preserve
                // unknown parameters rather than claiming they did not change.
                for key in [
                    "RECORD",
                    "INDEX",
                    "UNIQUEID",
                    "OWNERINDEX",
                    "X",
                    "Y",
                    "X1",
                    "Y1",
                    "X2",
                    "Y2",
                    "LOCATION.X",
                    "LOCATION.Y",
                    "ROTATION",
                    "LAYER",
                    "NET",
                    "COMPONENT",
                    "PATTERN",
                    "SOURCEDESIGNATOR",
                    "SOURCEUNIQUEID",
                ] {
                    p.remove(key);
                }
            }
            for (key, v) in map.iter_mut() {
                if key != "parameters" && key != "properties" {
                    clean(v);
                }
            }
        }
        Value::Array(values) => {
            for v in values {
                clean(v);
            }
        }
        _ => {}
    }
}
fn relative(value: &mut Value, anchor: Point, coordinate: bool) {
    match value {
        Value::Object(map) => {
            if coordinate && map.len() == 2 && map.contains_key("x") && map.contains_key("y") {
                if let (Some(x), Some(y)) = (map["x"].as_i64(), map["y"].as_i64()) {
                    map.insert("x".into(), (x - anchor.x).into());
                    map.insert("y".into(), (y - anchor.y).into());
                }
            } else {
                for (k, v) in map {
                    relative(
                        v,
                        anchor,
                        matches!(
                            k.as_str(),
                            "position"
                                | "start"
                                | "end"
                                | "centre"
                                | "a"
                                | "b"
                                | "points"
                                | "contour"
                                | "holes"
                        ),
                    );
                    if k == "primitives"
                        && let Value::Array(values) = v
                    {
                        values.sort_by_key(Value::to_string);
                    }
                }
            }
        }
        Value::Array(values) => {
            for v in values {
                relative(v, anchor, coordinate);
            }
        }
        _ => {}
    }
}
fn entity(
    key: Option<String>,
    category: &str,
    label: String,
    ids: Vec<Uuid>,
    bounds: Bounds,
    mut value: Value,
    anchor: Point,
) -> Entity {
    clean(&mut value);
    let mut rel = value.clone();
    relative(&mut rel, anchor, false);
    Entity {
        key,
        category: category.into(),
        label,
        target: Target { ids, bounds },
        value,
        relative: rel,
        anchor,
    }
}
fn native_key(s: &crate::schematic::Schematic, index: u32) -> Option<String> {
    s.records
        .get(index as usize)
        .filter(|r| r.index == index)
        .and_then(|r| r.fields.get("UNIQUEID"))
        .filter(|id| !id.is_empty())
        .map(|id| format!("native:{id}"))
}
fn sch_bounds(
    s: &crate::schematic::Schematic,
    primitives: &[&crate::schematic::SchPrimitive],
    anchor: Point,
) -> Bounds {
    let mut b = Bounds::points(&[anchor]);
    for p in primitives.iter().filter(|p| !p.presentation.hidden) {
        if let Some(p) = crate::schematic_arrange::primitive_bounds(s, p) {
            b.union(Bounds::points(&[p.min, p.max]));
        }
    }
    b
}
fn category(v: &Value) -> String {
    v.get("type")
        .or_else(|| v.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| "Object".into())
}
fn entities(doc: &Document) -> Vec<Entity> {
    let mut out = Vec::new();
    if let Some(s) = &doc.schematic {
        let mut children = BTreeMap::<Uuid, Vec<_>>::new();
        for p in s.primitives.iter().chain(&s.parameter_objects) {
            if let Some(c) = p.component {
                children.entry(c).or_default().push(p);
            }
        }
        for c in &s.components {
            let ps = children.get(&c.id).cloned().unwrap_or_default();
            let mut primitives: Vec<_> = ps
                .iter()
                .map(|p| {
                    let mut v = serde_json::to_value(p).unwrap();
                    clean(&mut v);
                    v
                })
                .collect();
            primitives.sort_by_key(Value::to_string);
            let key = native_key(s, c.source_index)
                .or_else(|| Some(format!("component:{}:{}", c.designator, c.part)));
            out.push(entity(
                key,
                "Component",
                format!("{} · {}", c.designator, c.comment),
                ps.iter().map(|p| p.id).chain([c.id]).collect(),
                sch_bounds(s, &ps, c.position),
                serde_json::json!({"component_data":c,"primitives":primitives}),
                c.position,
            ));
        }
        for p in s
            .primitives
            .iter()
            .chain(&s.parameter_objects)
            .filter(|p| p.component.is_none())
        {
            let value = serde_json::to_value(p).unwrap();
            let cat = category(&value["kind"]);
            let anchor = crate::schematic_edit::anchor(s, p.id).unwrap_or_default();
            let key = native_key(s, p.source_index)
                .or_else(|| s.source.is_none().then(|| format!("id:{}", p.id)));
            let label = match &p.kind {
                crate::schematic::SchPrimitiveKind::Text { text, .. } => format!("{cat} · {text}"),
                crate::schematic::SchPrimitiveKind::Polyline { electrical, .. }
                    if !electrical.is_empty() =>
                {
                    format!(
                        "{electrical} · {:.2}, {:.2} mm",
                        anchor.x as f64 / 1e6,
                        anchor.y as f64 / 1e6
                    )
                }
                crate::schematic::SchPrimitiveKind::Port { name, .. }
                | crate::schematic::SchPrimitiveKind::SheetEntry { name, .. }
                | crate::schematic::SchPrimitiveKind::ParameterSet { name, .. } => {
                    format!("{cat} · {name}")
                }
                _ => format!(
                    "{cat} · {:.2}, {:.2} mm",
                    anchor.x as f64 / 1e6,
                    anchor.y as f64 / 1e6
                ),
            };
            out.push(entity(
                key,
                &cat,
                label,
                vec![p.id],
                sch_bounds(s, &[p], anchor),
                value,
                anchor,
            ));
        }
        out.push(entity(Some("sheet".into()),"Settings","Sheet / fonts / parameters".into(),vec![],Bounds::empty(),serde_json::json!({"sheet":s.sheet,"fonts":s.fonts,"size":doc.size,"compile_masks":s.compile_masks,"images":s.images}),Point::default()));
    }
    if let Some(b) = &doc.pcb {
        let components: BTreeMap<_, _> = b.components.iter().map(|c| (c.index, c)).collect();
        let nets: BTreeMap<_, _> = b.nets.iter().map(|n| (n.index, &n.name)).collect();
        let mut children = BTreeMap::<u32, (Vec<Uuid>, Bounds)>::new();
        for item in &b.items {
            if let Some(index) = item.component {
                let entry = children
                    .entry(index)
                    .or_insert_with(|| (vec![], Bounds::empty()));
                entry.0.push(item.id);
                entry.1.union(item.bounds());
            }
        }
        for item in &b.items {
            let mut value = serde_json::to_value(item).unwrap();
            // Resolve numeric references before comparing; net/component indices may shift.
            value["net"] = serde_json::to_value(item.net.and_then(|n| nets.get(&n))).unwrap();
            value["component_name"] = serde_json::to_value(
                item.component
                    .and_then(|i| components.get(&i))
                    .map(|c| &c.designator),
            )
            .unwrap();
            let key = item
                .properties
                .get("UNIQUEID")
                .filter(|s| !s.is_empty())
                .map(|s| format!("native:{s}"))
                .or_else(|| {
                    if let crate::pcb::Shape::Pad { name, .. } = &item.shape {
                        Some(format!(
                            "pad:{}:{name}",
                            item.component
                                .and_then(|i| components.get(&i))
                                .map_or("", |c| c.designator.as_str())
                        ))
                    } else {
                        b.source.is_none().then(|| format!("id:{}", item.id))
                    }
                });
            out.push(entity(
                key,
                item.kind(),
                format!(
                    "{}{}{}",
                    item.kind(),
                    item.component
                        .and_then(|i| components.get(&i))
                        .map_or(String::new(), |c| format!(" · {}", c.designator)),
                    match &item.shape {
                        crate::pcb::Shape::Text { text, .. } => format!(" · {text}"),
                        crate::pcb::Shape::Track { .. } => item
                            .net
                            .and_then(|n| nets.get(&n))
                            .map_or(String::new(), |net| format!(" · {net}")),
                        _ => String::new(),
                    }
                ),
                vec![item.id],
                item.bounds(),
                value,
                item.anchor(),
            ));
        }
        for c in &b.components {
            let (ids, mut bounds) = children
                .remove(&c.index)
                .unwrap_or_else(|| (vec![], Bounds::empty()));
            bounds.include(c.position);
            out.push(entity(
                c.properties
                    .get("UNIQUEID")
                    .or_else(|| c.properties.get("SOURCEUNIQUEID"))
                    .filter(|id| !id.is_empty())
                    .map(|id| format!("native:component:{id}"))
                    .or_else(|| Some(format!("component:{}", c.designator))),
                "Component",
                format!("{} · {}", c.designator, c.comment),
                ids,
                bounds,
                serde_json::to_value(c).unwrap(),
                c.position,
            ));
        }
        out.push(entity(Some("board".into()),"Settings","Board / layers / nets / rules / polygons".into(),vec![],Bounds::empty(),serde_json::json!({"outline":b.outline,"layers":b.layers,"nets":b.nets,"rules":b.rules,"classes":b.classes,"pairs":b.differential_pairs,"polygons":b.polygon_pours,"stack":b.layer_stack,"board_parameters":b.properties}),Point::default()));
    }
    // Legacy scenes are normalized to the typed PCB model by callers/importers.
    if doc.pcb.is_none() && doc.kind == crate::DocumentKind::Board {
        let mut typed = doc.clone();
        typed.pcb = Some(crate::pcb::Board::from_legacy(doc));
        typed.pads.clear();
        typed.tracks.clear();
        return entities(&typed);
    }
    out
}
fn signature(e: &Entity, pass: u8) -> Option<String> {
    match pass {
        0 => e.key.as_ref().map(|k| format!("{}:{k}", e.category)),
        1 => Some(format!("{}:{}", e.category, e.value)),
        2 => Some(format!("{}:{}", e.category, e.relative)),
        _ => Some(format!("{}:{}:{}", e.category, e.anchor.x, e.anchor.y)),
    }
}
fn fields(before: &Value, after: &Value, prefix: &str, out: &mut Vec<Field>) {
    if before == after {
        return;
    }
    if let (Some(a), Some(b)) = (before.as_object(), after.as_object()) {
        let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
        for key in keys {
            fields(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                &if prefix.is_empty() {
                    key.to_string()
                } else {
                    format!("{prefix}.{key}")
                },
                out,
            );
        }
    } else {
        out.push(Field {
            name: prefix.into(),
            before: before.to_string(),
            after: after.to_string(),
        });
    }
}
// Native SchDoc coordinates have 0.0001 mil (2.54 nm) resolution. Independent
// rounding of a translated anchor and its children can differ by 1–3 nm.
// Apply this tolerance only to coordinates when classifying a pure translation;
// parameter values, widths and ordinary change detection remain exact.
fn same_translation(a: &Value, b: &Value, coordinate: bool) -> bool {
    if a == b {
        return true;
    }
    match (a, b) {
        (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
            a.iter().all(|(key, value)| {
                b.get(key).is_some_and(|other| {
                    if key == "parameters" || key == "properties" {
                        value == other
                    } else {
                        same_translation(value, other, key == "x" || key == "y")
                    }
                })
            })
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            a.iter().zip(b).all(|(a, b)| same_translation(a, b, false))
        }
        (Value::Number(a), Value::Number(b)) if coordinate => a
            .as_i64()
            .zip(b.as_i64())
            .is_some_and(|(a, b)| a.abs_diff(b) <= 3),
        _ => false,
    }
}
pub fn compare(before: Option<&Document>, after: Option<&Document>) -> Diff {
    let a = before.map(entities).unwrap_or_default();
    let b = after.map(entities).unwrap_or_default();
    let mut pairs = BTreeMap::new();
    let mut used = BTreeSet::new();
    for pass in 0..4 {
        let mut aa = BTreeMap::<String, Vec<usize>>::new();
        let mut bb = BTreeMap::<String, Vec<usize>>::new();
        for (i, e) in a.iter().enumerate().filter(|(i, _)| !pairs.contains_key(i)) {
            if let Some(key) = signature(e, pass) {
                aa.entry(key).or_default().push(i);
            }
        }
        for (i, e) in b.iter().enumerate().filter(|(i, _)| !used.contains(i)) {
            if let Some(key) = signature(e, pass) {
                bb.entry(key).or_default().push(i);
            }
        }
        for (key, old) in aa {
            if let Some(new) = bb.get(&key)
                && old.len() == 1
                && new.len() == 1
            {
                let (i, j) = (old[0], new[0]);
                // Conflicting persistent identities must never be paired by location.
                let persistent = |key: &Option<String>| {
                    key.as_ref()
                        .is_some_and(|k| k.starts_with("native:") || k.starts_with("id:"))
                };
                if pass > 0
                    && persistent(&a[i].key)
                    && persistent(&b[j].key)
                    && a[i].key != b[j].key
                {
                    continue;
                }
                pairs.insert(i, j);
                used.insert(j);
            }
        }
    }
    let mut changes = Vec::new();
    for (i, old) in a.iter().enumerate() {
        if let Some(&j) = pairs.get(&i) {
            let new = &b[j];
            if old.value == new.value {
                continue;
            }
            let moved = old.anchor != new.anchor;
            let kind = if moved && same_translation(&old.relative, &new.relative, false) {
                Kind::Moved
            } else {
                Kind::Modified
            };
            let mut fs = Vec::new();
            fields(&old.value, &new.value, "", &mut fs);
            if kind == Kind::Moved && old.category == "Component" {
                fs.retain(|field| field.name != "primitives");
            }
            changes.push(Change {
                label: new.label.clone(),
                kind,
                moved,
                before: Some(old.target.clone()),
                after: Some(new.target.clone()),
                fields: fs,
            });
        } else {
            changes.push(Change {
                label: old.label.clone(),
                kind: Kind::Removed,
                moved: false,
                before: Some(old.target.clone()),
                after: None,
                fields: vec![],
            });
        }
    }
    for (_, new) in b.iter().enumerate().filter(|(j, _)| !used.contains(j)) {
        changes.push(Change {
            label: new.label.clone(),
            kind: Kind::Added,
            moved: false,
            before: None,
            after: Some(new.target.clone()),
            fields: vec![],
        });
    }
    Diff { changes }
}

#[cfg(test)]
#[path = "document_diff_tests.rs"]
mod tests;
