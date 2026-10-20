//! Local wire topology for schematic dragging; no screen-space proximity or net-name merging.
use super::*;
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
#[path = "schematic_drag_tests.rs"]
mod tests;

type Node = (i64, i64);
fn key(p: Point) -> Node {
    (p.x, p.y)
}
fn point(p: Node) -> Point {
    Point { x: p.0, y: p.1 }
}

fn pin_end(p: &SchPrimitive) -> Option<Point> {
    match p.kind {
        K::Pin {
            position,
            length_nm,
            orientation,
            ..
        } => {
            let (x, y) = match orientation % 4 {
                0 => (length_nm, 0),
                1 => (0, length_nm),
                2 => (-length_nm, 0),
                _ => (0, -length_nm),
            };
            Some(Point {
                x: position.x.checked_add(x)?,
                y: position.y.checked_add(y)?,
            })
        }
        K::PowerPort { position, .. } | K::Port { position, .. } | K::Junction { position, .. } => {
            Some(position)
        }
        K::Text { position, .. } if p.text_role == Some(TextRole::NetLabel) => Some(position),
        _ => None,
    }
}
fn wire(p: &SchPrimitive) -> Option<&[Point]> {
    match &p.kind {
        K::Polyline {
            points,
            electrical,
            closed: false,
        } if electrical == "Wire" && p.component.is_none() => Some(points),
        _ => None,
    }
}
fn clean(path: &mut Vec<Point>) {
    path.dedup();
    let mut i = 1;
    while i + 1 < path.len() {
        if path[i].on_segment(path[i - 1], path[i + 1]) {
            path.remove(i);
        } else {
            i += 1;
        }
    }
}
fn reroute(
    path: &[Point],
    start: Point,
    end: Point,
    pivot: Point,
    turns: u32,
    delta: Point,
) -> Result<Vec<Point>, String> {
    let mut p = path.to_vec();
    clean(&mut p);
    if p.len() < 2 {
        return Ok(vec![start, end]);
    }
    if start != p[0] && end != *p.last().unwrap() {
        for p in &mut p {
            *p = transform_point(*p, pivot, turns, delta)?;
        }
        return Ok(p);
    }
    if start == p[0] && end == *p.last().unwrap() {
        return Ok(p);
    }
    let reverse = start == p[0];
    if reverse {
        p.reverse();
    }
    let at = if reverse { end } else { start };
    let old = p[0];
    let next = p[1];
    if p.len() == 2 {
        let elbow = if old.y == next.y {
            Point { x: next.x, y: at.y }
        } else if old.x == next.x {
            Point { x: at.x, y: next.y }
        } else {
            next
        };
        p = vec![at, elbow, next];
    } else {
        p[0] = at;
        if old.y == next.y {
            p[1].y = at.y;
        } else if old.x == next.x {
            p[1].x = at.x;
        }
    }
    clean(&mut p);
    if reverse {
        p.reverse();
    }
    Ok(p)
}

/// Updated objects and incident wires only, with original UUIDs/presentation retained.
/// Every wire is split at electrical attachments before a free bend is adjusted.
/// Crossings of two wire interiors are not nodes unless a pin/junction is present.
pub fn connected_transform(
    s: &Schematic,
    ids: &[Uuid],
    pivot: Point,
    turns: u32,
    delta: Point,
) -> Result<Fragment, String> {
    let turns = turns % 4;
    if ids.is_empty() {
        return Err("Nothing selected".into());
    }
    if !pivot.valid() || !delta.valid() {
        return Err("Invalid drag coordinates".into());
    }
    let mut selected = Fragment {
        fonts: s.fonts.clone(),
        images: s.images.clone(),
        ..Default::default()
    };
    let mut seen = BTreeSet::new();
    for id in ids {
        let f = extract(s, *id)?;
        for c in f.components {
            if seen.insert(c.id) {
                selected.components.push(c);
            }
        }
        for p in f.primitives {
            if seen.insert(p.id) {
                selected.primitives.push(p);
            }
        }
    }
    if selected.primitives.iter().any(|p| wire(p).is_some()) {
        return Err("Drag a component, terminal, label or junction; wire segment dragging is not supported yet".into());
    }
    let mut moved = BTreeMap::<Node, (Point, Uuid)>::new();
    for p in &selected.primitives {
        if let Some(at) = pin_end(p) {
            let to = transform_point(at, pivot, turns, delta)?;
            if to != at {
                moved.insert(key(at), (to, p.id));
            }
        }
    }
    let mut result = selected.transformed(pivot, turns, delta)?;
    if moved.is_empty() {
        return Ok(result);
    }
    let mut anchors = BTreeSet::<Node>::new();
    let mut fixed = BTreeSet::<Node>::new();
    for p in &s.primitives {
        if seen.contains(&p.id) {
            continue;
        }
        if let Some(at) = pin_end(p) {
            anchors.insert(key(at));
            // A dot alone can accompany a pin. Shared net terminals remain fixed.
            if !matches!(p.kind, K::Junction { .. }) {
                fixed.insert(key(at));
            }
        }
        if let Some(points) = wire(p) {
            if let Some(at) = points.first() {
                anchors.insert(key(*at));
            }
            if let Some(at) = points.last() {
                anchors.insert(key(*at));
            }
        }
    }
    anchors.extend(moved.keys());
    // Multiple wire branches sharing a moving terminal keep their common node fixed.
    // Stretch a single stem from that node instead of splitting the branch apart.
    for node in moved.keys() {
        let at = point(*node);
        let branches = s
            .primitives
            .iter()
            .filter_map(wire)
            .map(|path| {
                if path.first() == Some(&at) || path.last() == Some(&at) {
                    1
                } else if path.windows(2).any(|e| at.on_segment(e[0], e[1])) {
                    2
                } else {
                    0
                }
            })
            .sum::<usize>();
        let explicit_junction = selected
            .primitives
            .iter()
            .any(|p| matches!(p.kind, K::Junction { position, .. } if key(position) == *node));
        if branches > 1 && !explicit_junction {
            fixed.insert(*node);
        }
    }
    for p in &s.primitives {
        let Some(path) = wire(p) else {
            continue;
        };
        if path.len() < 2 {
            continue;
        }
        if !moved.keys().any(|n| {
            !fixed.contains(n) && path.windows(2).any(|e| point(*n).on_segment(e[0], e[1]))
        }) {
            continue;
        }
        let mut split = vec![path[0]];
        for e in path.windows(2) {
            let mut interior: Vec<_> = anchors
                .iter()
                .copied()
                .map(point)
                .filter(|at| *at != e[0] && *at != e[1] && at.on_segment(e[0], e[1]))
                .collect();
            interior.sort_by_key(|at| {
                (at.x as i128 - e[0].x as i128).abs() + (at.y as i128 - e[0].y as i128).abs()
            });
            split.extend(interior);
            split.push(e[1]);
        }
        let destination = |p: Point| {
            if fixed.contains(&key(p)) {
                p
            } else {
                moved.get(&key(p)).map_or(p, |(to, _)| *to)
            }
        };
        let mut updated = Vec::new();
        let mut start = 0;
        for end in 1..split.len() {
            if end + 1 != split.len() && !anchors.contains(&key(split[end])) {
                continue;
            }
            let run = reroute(
                &split[start..=end],
                destination(split[start]),
                destination(split[end]),
                pivot,
                turns,
                delta,
            )?;
            if updated.is_empty() {
                updated.extend(run);
            } else {
                updated.extend(run.into_iter().skip(1));
            }
            start = end;
        }
        updated.dedup();
        if updated != path {
            let mut replacement = p.clone();
            if let K::Polyline { points, .. } = &mut replacement.kind {
                *points = updated;
            }
            result.primitives.push(replacement);
        }
    }
    for p in &s.primitives {
        if seen.contains(&p.id) {
            continue;
        }
        if let K::Junction { position, .. } | K::NoErc { position } = p.kind
            && !fixed.contains(&key(position))
            && let Some((to, _)) = moved.get(&key(position))
        {
            let mut replacement = p.clone();
            if let K::Junction { position, .. } | K::NoErc { position } = &mut replacement.kind {
                *position = *to;
            }
            result.primitives.push(replacement);
        }
    }
    for (node, (to, owner)) in &moved {
        if !fixed.contains(node) {
            continue;
        }
        let at = point(*node);
        let template = s.primitives.iter().find(|p| {
            wire(p).is_some_and(|path| path.windows(2).any(|e| at.on_segment(e[0], e[1])))
        });
        let mut p = template.cloned().unwrap_or_else(|| {
            primitive(
                K::Polyline {
                    points: vec![],
                    electrical: "Wire".into(),
                    closed: false,
                },
                None,
            )
        });
        let mut salt = 0u32;
        loop {
            p.id = Uuid::new_v5(owner, format!("drag-stem:{node:?}:{salt}").as_bytes());
            if s.primitive(p.id).is_none() {
                break;
            }
            salt += 1;
        }
        p.source_index = AUTHORED;
        let elbow = template
            .and_then(wire)
            .and_then(|path| path.windows(2).find(|e| at.on_segment(e[0], e[1])))
            .map(|e| {
                if e[0].y == e[1].y {
                    Point { x: at.x, y: to.y }
                } else {
                    Point { x: to.x, y: at.y }
                }
            })
            .unwrap_or(Point { x: at.x, y: to.y });
        let mut points = vec![at, elbow, *to];
        clean(&mut points);
        p.kind = K::Polyline {
            points,
            electrical: "Wire".into(),
            closed: false,
        };
        result.primitives.push(p);
    }
    Ok(result)
}
