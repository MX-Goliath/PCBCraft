//! Logical signal harness geometry and definition files; independent of GUI/native storage.
use crate::{
    Point,
    schematic::{SchPrimitive, SchPrimitiveKind as K, Schematic},
    schematic_edit::{self as edit, Fragment},
};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
#[cfg(test)]
#[path = "schematic_harness_tests.rs"]
mod tests;

pub const BORDER: u32 = 13_213_327;
pub const FILL: u32 = 16_511_725;
pub const ENTRY_COLOR: u32 = 7_354_880;
pub const SIGNAL_COLOR: u32 = 15_187_117;

pub fn edge(a: Point, b: Point, side: u32, distance: i64) -> Point {
    match side {
        0 => Point {
            x: a.x,
            y: a.y - distance.clamp(0, a.y - b.y),
        },
        1 => Point {
            x: b.x,
            y: a.y - distance.clamp(0, a.y - b.y),
        },
        2 => Point {
            x: a.x + distance.clamp(0, b.x - a.x),
            y: a.y,
        },
        _ => Point {
            x: a.x + distance.clamp(0, b.x - a.x),
            y: b.y,
        },
    }
}
pub fn terminal(p: &SchPrimitive) -> Option<Point> {
    if let K::HarnessConnector {
        a,
        b,
        side,
        primary_nm,
        ..
    } = p.kind
    {
        Some(edge(a, b, side, primary_nm))
    } else {
        None
    }
}
pub fn touches(a: &SchPrimitive, b: &SchPrimitive) -> bool {
    let path = |p: &SchPrimitive| match &p.kind {
        K::Polyline {
            points, electrical, ..
        } if matches!(electrical.as_str(), "Bus" | "Signal Harness") => {
            Some((electrical.clone(), points.clone()))
        }
        _ => None,
    };
    let endpoints = |p: &SchPrimitive| {
        terminal(p)
            .map(|p| vec![p])
            .unwrap_or_else(|| edit::electrical_endpoints(p).collect())
    };
    match (path(a), path(b)) {
        (Some((ak, a)), Some((bk, b))) => {
            ak == bk
                && (a
                    .first()
                    .into_iter()
                    .chain(a.last())
                    .any(|p| b.windows(2).any(|e| p.on_segment(e[0], e[1])))
                    || b.first()
                        .into_iter()
                        .chain(b.last())
                        .any(|p| a.windows(2).any(|e| p.on_segment(e[0], e[1]))))
        }
        (Some((_, path)), None) => endpoints(b)
            .iter()
            .any(|p| path.windows(2).any(|e| p.on_segment(e[0], e[1]))),
        (None, Some((_, path))) => endpoints(a)
            .iter()
            .any(|p| path.windows(2).any(|e| p.on_segment(e[0], e[1]))),
        _ => false,
    }
}
pub fn connection_at(s: &Schematic, at: Point) -> bool {
    s.primitives.iter().filter(|p|!p.presentation.hidden).any(|p| {
        terminal(p)==Some(at) || matches!(&p.kind,K::Polyline{points,electrical,..} if electrical=="Signal Harness" && points.windows(2).any(|e|at.on_segment(e[0],e[1])))
            || matches!(p.kind,K::Port{..}|K::SheetEntry{..}|K::HarnessEntry{..}) && edit::electrical_endpoints(p).any(|p|p==at)
    })
}
pub fn connector(a: Point, b: Point, font: usize, name: &str) -> Result<Fragment, String> {
    if a.x >= b.x || a.y <= b.y {
        return Err("Harness Connector width and height must be positive".into());
    }
    let mut p = edit::primitive(
        K::HarnessConnector {
            a,
            b,
            side: 1,
            primary_nm: (a.y - b.y) / 2,
            bus_text_style: "Full".into(),
        },
        None,
    );
    p.color = BORDER;
    p.fill = Some(FILL);
    p.presentation.record_type = 215;
    p.presentation.allow_sync = true;
    p.presentation.harness_type = name.into();
    let mut label = edit::primitive(
        K::Text {
            position: a,
            text: name.into(),
            font,
            orientation: 0,
            justification: 0,
        },
        None,
    );
    label.presentation.record_type = 217;
    label.presentation.parent = Some(p.id);
    label.presentation.raw_text = Some(name.into());
    label.presentation.allow_sync = true;
    label.color = 8_388_608;
    Ok(Fragment {
        primitives: vec![p, label],
        ..Default::default()
    })
}
pub fn entry(s: &Schematic, parent: Uuid, at: Point, name: &str) -> Result<SchPrimitive, String> {
    let p = s.primitive(parent).ok_or("Missing Harness Connector")?;
    let K::HarnessConnector { a, b, side, .. } = p.kind else {
        return Err("Choose a Harness Connector".into());
    };
    if at.x < a.x || at.x > b.x || at.y > b.y.max(a.y) || at.y < b.y {
        return Err("Harness Entry must be placed inside a Harness Connector".into());
    }
    let side = match side {
        0 => 1,
        1 => 0,
        2 => 3,
        _ => 2,
    };
    let distance = if side <= 1 { a.y - at.y } else { at.x - a.x };
    let mut e = edit::primitive(
        K::HarnessEntry {
            position: edge(a, b, side, distance),
            side,
            name: name.into(),
            font: s.sheet.system_font,
            text_color: ENTRY_COLOR,
        },
        None,
    );
    e.color = ENTRY_COLOR;
    e.fill = Some(8_454_143);
    e.width_nm = 101_600;
    e.presentation.parent = Some(parent);
    e.presentation.record_type = 216;
    e.presentation.allow_sync = true;
    Ok(e)
}
pub fn refresh(s: &mut Schematic, before: &Schematic) -> Result<(), String> {
    let mut resized = BTreeSet::new();
    let moved: Vec<_> = s
        .primitives
        .iter()
        .filter_map(|p| {
            if let K::HarnessEntry { position, side, .. } = p.kind {
                before
                    .primitive(p.id)
                    .filter(|old| old.kind != p.kind)
                    .and_then(|_| {
                        p.presentation
                            .parent
                            .map(|parent| (p.id, parent, position, side))
                    })
            } else {
                None
            }
        })
        .collect();
    for (id, parent, mut position, side) in moved {
        let Some(owner) = s.primitives.iter_mut().find(|p| p.id == parent) else {
            continue;
        };
        let owner_unchanged = before
            .primitive(parent)
            .is_some_and(|p| p.kind == owner.kind);
        if let K::HarnessConnector {
            a, b, primary_nm, ..
        } = &mut owner.kind
            && owner_unchanged
        {
            if side <= 1 {
                a.y = a.y.max(position.y);
                b.y = b.y.min(position.y);
                position.x = if side == 0 { a.x } else { b.x };
            } else {
                a.x = a.x.min(position.x);
                b.x = b.x.max(position.x);
                position.y = if side == 2 { a.y } else { b.y };
            }
            *primary_nm = (*primary_nm).min(if side <= 1 { a.y - b.y } else { b.x - a.x });
            resized.insert(parent);
        }
        if let K::HarnessEntry { position: p, .. } =
            &mut s.primitives.iter_mut().find(|p| p.id == id).unwrap().kind
        {
            *p = position;
        }
    }
    let connectors: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| matches!(p.kind, K::HarnessConnector { .. }))
        .cloned()
        .collect();
    for connector in connectors {
        let K::HarnessConnector { a, b, .. } = connector.kind else {
            unreachable!()
        };
        let old = before.primitive(connector.id);
        let old_rect = old.and_then(|p| {
            if let K::HarnessConnector { a, b, .. } = p.kind {
                Some((a, b))
            } else {
                None
            }
        });
        let mut names = BTreeSet::new();
        for child in s
            .primitives
            .iter_mut()
            .filter(|p| p.presentation.parent == Some(connector.id))
        {
            let previous = before.primitive(child.id);
            let unchanged = previous == Some(child);
            match &mut child.kind {
                K::HarnessEntry {
                    position,
                    side,
                    name,
                    ..
                } => {
                    if !names.insert(name.to_uppercase()) {
                        return Err(format!("Duplicate Harness Entry {name}"));
                    }
                    if let Some((x, y)) = old_rect
                        && !resized.contains(&connector.id) && previous.is_some_and(|p|matches!(p.kind,K::HarnessEntry{position:q,side:t,..} if q==*position && t==*side)) && (x,y)!=(a,b) {
                            let distance=if *side<=1 {x.y-position.y} else {position.x-x.x};
                            *position=edge(a,b,*side,distance);
                        }
                }
                K::Text { text, position, .. } if child.presentation.record_type == 217 => {
                    if old.is_some_and(|p| {
                        p.presentation.harness_type != connector.presentation.harness_type
                    }) {
                        *text = connector.presentation.harness_type.clone();
                        child.presentation.raw_text = Some(text.clone());
                    }
                    if let Some((x, _)) = old_rect
                        && unchanged
                    {
                        position.x += a.x - x.x;
                        position.y += a.y - x.y;
                    }
                }
                _ => {}
            }
        }
        if let Some(name) = s.primitives.iter().find_map(|p| {
            if p.presentation.parent == Some(connector.id) && p.presentation.record_type == 217 {
                if let K::Text { text, .. } = &p.kind {
                    Some(text.clone())
                } else {
                    None
                }
            } else {
                None
            }
        }) {
            s.primitives
                .iter_mut()
                .find(|p| p.id == connector.id)
                .unwrap()
                .presentation
                .harness_type = name;
        }
    }
    infer_types(s);
    Ok(())
}

pub fn infer_types(s: &mut Schematic) {
    if !s.primitives.iter().any(|p| {
        matches!(p.kind, K::HarnessConnector { .. })
            || matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Signal Harness")
    }) {
        return;
    }
    if let Ok(expansion) = crate::design_sync::local_harness(s) {
        for ((_, id), kind) in expansion.types {
            if let Some(p) = s.primitives.iter_mut().find(|p| p.id == id) {
                p.presentation.harness_type = kind;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub name: String,
    pub locked: bool,
    pub entries: BTreeMap<String, String>,
    pub order: Vec<String>,
}
pub fn parse_definitions(text: &str) -> Result<Vec<Definition>, String> {
    let mut out = vec![];
    for raw in text.trim_start_matches('\u{feff}').lines() {
        let raw = raw.split("//").next().unwrap_or("").trim();
        if raw.is_empty() {
            continue;
        }
        let (locked, raw) = if let Some((prefix, rest)) = raw.split_once(';') {
            if prefix.trim().eq_ignore_ascii_case("Locked") {
                (true, rest.trim())
            } else {
                (false, raw)
            }
        } else {
            (false, raw)
        };
        let (name, members) = raw
            .split_once('=')
            .ok_or("Harness definition requires Type=Entry,Entry")?;
        let name = name.trim();
        if name.is_empty() || name.contains(['[', ']', '{', '}', '.', ':', ';']) {
            return Err("Invalid Harness Connector Type".into());
        }
        let mut entries = BTreeMap::new();
        let mut order = vec![];
        for entry in members.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let entry = if entry.starts_with('{') {
                entry
                    .strip_prefix('{')
                    .and_then(|s| s.strip_suffix('}'))
                    .ok_or("Unclosed nested harness definition")?
            } else {
                entry
            };
            let (name, kind) = entry.split_once(':').unwrap_or((entry, ""));
            if entries
                .keys()
                .any(|k: &String| k.eq_ignore_ascii_case(name.trim()))
            {
                return Err("Duplicate harness definition entry".into());
            }
            entries.insert(name.trim().into(), kind.trim().into());
            order.push(name.trim().to_owned());
        }
        if out
            .iter()
            .any(|d: &Definition| d.name.eq_ignore_ascii_case(name))
        {
            return Err("Duplicate harness definition".into());
        }
        out.push(Definition {
            name: name.into(),
            locked,
            entries,
            order,
        });
    }
    Ok(out)
}
pub fn definitions(s: &Schematic) -> Result<Vec<Definition>, String> {
    let mut out: Vec<Definition> = vec![];
    for c in s
        .primitives
        .iter()
        .filter(|p| matches!(p.kind, K::HarnessConnector { .. }))
    {
        let entries = s
            .primitives
            .iter()
            .filter(|p| p.presentation.parent == Some(c.id))
            .filter_map(|p| {
                if let K::HarnessEntry { name, .. } = &p.kind {
                    Some((name.clone(), p.presentation.harness_type.clone()))
                } else {
                    None
                }
            })
            .collect();
        let d = Definition {
            name: c.presentation.harness_type.clone(),
            locked: false,
            entries,
            order: s
                .primitives
                .iter()
                .filter(|p| p.presentation.parent == Some(c.id))
                .filter_map(|p| {
                    if let K::HarnessEntry { name, .. } = &p.kind {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect(),
        };
        if d.name.is_empty() || d.name == "*" {
            continue;
        }
        if let Some(old) = out.iter().find(|v| v.name.eq_ignore_ascii_case(&d.name)) {
            if old.entries != d.entries {
                return Err(format!("Conflicting Harness Definition for {}", d.name));
            }
        } else {
            out.push(d);
        }
    }
    Ok(out)
}
pub fn format_definitions(definitions: &[Definition]) -> String {
    definitions
        .iter()
        .map(|d| {
            format!(
                "{}{}={}\r\n",
                if d.locked { "LOCKED;" } else { "" },
                d.name,
                d.order
                    .iter()
                    .filter_map(|n| d.entries.get(n).map(|t| (n, t)))
                    .map(|(n, t)| if t.is_empty() {
                        n.clone()
                    } else {
                        format!("{{{n}:{t}}}")
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect()
}
