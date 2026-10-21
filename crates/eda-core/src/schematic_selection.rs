//! Selection transactions, including logical wire spans between bends and junctions.
use super::drag::{clean, pin_end, wire};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "schematic_selection_tests.rs"]
mod tests;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Selection {
    pub ids: Vec<Uuid>,
    pub segments: Vec<WireSegment>,
}
impl Selection {
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty() && self.segments.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireSegment {
    pub id: Uuid,
    pub a: Point,
    pub b: Point,
}

fn roots(s: &Schematic, selection: &Selection) -> Result<Selection, String> {
    let mut ids = Vec::new();
    let mut seen = BTreeSet::new();
    for id in &selection.ids {
        if anchor(s, *id).is_none() {
            return Err("Selected object no longer exists".into());
        }
        if s.primitive(*id)
            .and_then(|p| p.component)
            .is_some_and(|id| selection.ids.contains(&id))
        {
            continue;
        }
        if seen.insert(*id) {
            ids.push(*id);
        }
    }
    let mut segments = Vec::new();
    for span in &selection.segments {
        if ids.contains(&span.id) {
            continue;
        }
        if span.a == span.b
            || !span.a.valid()
            || !span.b.valid()
            || !segment_edges(s, span.id).is_some_and(|edges| {
                edges
                    .into_iter()
                    .any(|(a, b)| (span.a == a && span.b == b) || (span.a == b && span.b == a))
            })
        {
            return Err("Selected wire segment changed; select it again".into());
        }
        if !segments.iter().any(|old: &WireSegment| {
            old.id == span.id
                && ((old.a == span.a && old.b == span.b) || (old.a == span.b && old.b == span.a))
        }) {
            segments.push(*span);
        }
    }
    if ids.is_empty() && segments.is_empty() {
        return Err("Nothing selected".into());
    }
    Ok(Selection { ids, segments })
}

pub fn selection_anchor(s: &Schematic, selection: &Selection) -> Option<Point> {
    selection
        .ids
        .first()
        .and_then(|id| anchor(s, *id))
        .or_else(|| selection.segments.first().map(|span| span.a))
}

pub(super) fn split_edges(path: &[Point], nodes: &[Point]) -> Vec<(Point, Point)> {
    let mut result = Vec::new();
    for e in path.windows(2).filter(|e| e[0] != e[1]) {
        let mut points = vec![e[0], e[1]];
        points.extend(nodes.iter().copied().filter(|p| p.on_segment(e[0], e[1])));
        points.sort_by_key(|p| {
            (p.x as i128 - e[0].x as i128).abs() + (p.y as i128 - e[0].y as i128).abs()
        });
        points.dedup();
        result.extend(points.windows(2).map(|e| (e[0], e[1])));
    }
    result
}

/// Pick a deletable span, stopping at bends and electrical attachments, not at a
/// bare crossing of wire interiors. Stored coordinates detect stale selections.
pub fn wire_segment_at(s: &Schematic, id: Uuid, at: Point, radius: i64) -> Option<WireSegment> {
    segment_edges(s, id)?
        .into_iter()
        .filter_map(|(a, b)| {
            let (dx, dy) = (b.x as f64 - a.x as f64, b.y as f64 - a.y as f64);
            let t = (((at.x as f64 - a.x as f64) * dx + (at.y as f64 - a.y as f64) * dy)
                / (dx * dx + dy * dy))
                .clamp(0.0, 1.0);
            let distance =
                (a.x as f64 + t * dx - at.x as f64).hypot(a.y as f64 + t * dy - at.y as f64);
            (distance <= radius.max(0) as f64).then_some((distance, WireSegment { id, a, b }))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, span)| span)
}

fn segment_edges(s: &Schematic, id: Uuid) -> Option<Vec<(Point, Point)>> {
    let path = wire(s.primitive(id)?)?;
    let mut nodes = Vec::new();
    for p in &s.primitives {
        nodes.extend(pin_end(p));
        if p.id != id
            && let Some(path) = wire(p)
        {
            nodes.extend(path.first().into_iter().chain(path.last()).copied());
        }
    }
    Some(split_edges(path, &nodes))
}

pub(super) fn piece(
    template: &SchPrimitive,
    points: Vec<Point>,
    keep_id: bool,
    occupied: &mut BTreeSet<Uuid>,
) -> SchPrimitive {
    let mut p = template.clone();
    if !keep_id {
        let mut salt = 0u32;
        loop {
            p.id = Uuid::new_v5(
                &template.id,
                format!("wire-piece:{points:?}:{salt}").as_bytes(),
            );
            if !occupied.contains(&p.id) {
                break;
            }
            salt += 1;
        }
        p.source_index = AUTHORED;
    }
    occupied.insert(p.id);
    if let K::Polyline { points: target, .. } = &mut p.kind {
        *target = points;
    }
    p
}

type Parts = Vec<(Uuid, Vec<SchPrimitive>, Vec<SchPrimitive>)>;
fn parts(s: &Schematic, selection: &Selection) -> Parts {
    let mut grouped = BTreeMap::<Uuid, Vec<WireSegment>>::new();
    for span in &selection.segments {
        grouped.entry(span.id).or_default().push(*span);
    }
    let mut occupied: BTreeSet<_> = s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .map(|p| p.id)
        .collect();
    let mut result = Vec::new();
    for (id, spans) in grouped {
        let p = s.primitive(id).unwrap();
        let nodes: Vec<_> = spans.iter().flat_map(|span| [span.a, span.b]).collect();
        let mut runs: Vec<(bool, Vec<Point>)> = Vec::new();
        for (a, b) in split_edges(wire(p).unwrap(), &nodes) {
            let selected = spans
                .iter()
                .any(|span| a.on_segment(span.a, span.b) && b.on_segment(span.a, span.b));
            if let Some((old, path)) = runs.last_mut()
                && *old == selected
                && path.last() == Some(&a)
            {
                path.push(b);
            } else {
                runs.push((selected, vec![a, b]));
            }
        }
        let keep = runs.iter().position(|(selected, _)| !selected).unwrap_or(0);
        let mut chosen = Vec::new();
        let mut remaining = Vec::new();
        for (i, (selected, mut path)) in runs.into_iter().enumerate() {
            clean(&mut path);
            let p = piece(p, path, i == keep, &mut occupied);
            if selected {
                chosen.push(p);
            } else {
                remaining.push(p);
            }
        }
        result.push((id, chosen, remaining));
    }
    result
}

pub fn extract_selection(s: &Schematic, selection: &Selection) -> Result<Fragment, String> {
    let selection = roots(s, selection)?;
    let mut result = Fragment {
        fonts: s.fonts.clone(),
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    for id in &selection.ids {
        let f = extract(s, *id)?;
        for c in f.components {
            if seen.insert(c.id) {
                result.components.push(c);
            }
        }
        for p in f.primitives {
            if seen.insert(p.id) {
                result.primitives.push(p);
            }
        }
        if !f.images.is_empty() {
            std::sync::Arc::make_mut(&mut result.images)
                .extend(f.images.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }
    for (_, chosen, _) in parts(s, &selection) {
        result.primitives.extend(chosen);
    }
    Ok(result)
}

pub(super) fn delete(s: &mut Schematic, selection: &Selection) -> Result<(), String> {
    let selection = roots(s, selection)?;
    let parts = parts(s, &selection);
    for id in selection.ids {
        apply(s, &mut Point::default(), &Edit::Delete(id), 0)?;
    }
    for (id, _, remaining) in parts {
        s.primitives.retain(|p| p.id != id);
        s.primitives.extend(remaining);
    }
    Ok(())
}

pub(super) fn transform(
    s: &mut Schematic,
    selection: &Selection,
    pivot: Point,
    turns: u32,
    delta: Point,
    connected: bool,
) -> Result<(), String> {
    let mut selection = roots(s, selection)?;
    if !pivot.valid() || !delta.valid() {
        return Err("Invalid selection transform coordinates".into());
    }
    if turns.is_multiple_of(4) && delta == Point::default() {
        return Ok(());
    }
    let parts = parts(s, &selection);
    for (id, chosen, remaining) in parts {
        s.primitives.retain(|p| p.id != id);
        selection.ids.extend(chosen.iter().map(|p| p.id));
        s.primitives.extend(remaining);
        s.primitives.extend(chosen);
    }
    selection.segments.clear();
    let patch = if connected {
        connected_transform(s, &selection.ids, pivot, turns, delta)?
    } else {
        extract_selection(s, &selection)?.transformed(pivot, turns, delta)?
    };
    apply_drag_patch(s, patch)
}

/// The renderer receives the same validated/optimized edit result used by history.
/// Empty wire tombstones hide originals removed by a split or overlap merge.
pub fn preview_fragment(doc: &Document, command: &Edit) -> Result<Fragment, String> {
    let next = preview(doc, command)?;
    let old = doc.schematic.as_ref().ok_or("Schematic unavailable")?;
    let s = next.schematic.as_ref().unwrap();
    let mut result = Fragment {
        fonts: s.fonts.clone(),
        images: s.images.clone(),
        ..Default::default()
    };
    result.components.extend(
        s.components
            .iter()
            .filter(|c| old.components.iter().find(|p| p.id == c.id) != Some(*c))
            .cloned(),
    );
    result.primitives.extend(
        s.primitives
            .iter()
            .chain(&s.parameter_objects)
            .filter(|p| old.primitive(p.id) != Some(*p))
            .cloned(),
    );
    for p in old
        .primitives
        .iter()
        .chain(&old.parameter_objects)
        .filter(|p| s.primitive(p.id).is_none())
    {
        let mut removed = p.clone();
        removed.kind = K::Polyline {
            points: vec![],
            electrical: "Wire".into(),
            closed: false,
        };
        result.primitives.push(removed);
    }
    Ok(result)
}
