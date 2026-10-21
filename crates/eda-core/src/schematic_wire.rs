//! Wire handles and derived local junctions. All geometry is in document nanometres.
use super::drag::{clean, key, pin_end, point, reroute, stretch_wires, wire};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "schematic_wire_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WireHandle {
    Vertex(usize),
    Segment(usize),
}

/// Resolve a handle using a document-space picking radius supplied by the view.
pub fn wire_handle_at(s: &Schematic, id: Uuid, at: Point, radius: i64) -> Option<WireHandle> {
    let path = wire(s.primitive(id)?)?;
    let distance = |p: Point| (p.x as f64 - at.x as f64).hypot(p.y as f64 - at.y as f64);
    if let Some((i, _)) = path
        .iter()
        .enumerate()
        .map(|(i, p)| (i, distance(*p)))
        .filter(|(_, d)| *d <= radius.max(0) as f64)
        .min_by(|a, b| a.1.total_cmp(&b.1))
    {
        return Some(WireHandle::Vertex(i));
    }
    path.windows(2)
        .enumerate()
        .filter_map(|(i, e)| {
            let dx = e[1].x as f64 - e[0].x as f64;
            let dy = e[1].y as f64 - e[0].y as f64;
            let length = dx * dx + dy * dy;
            if length == 0.0 {
                return None;
            }
            let t = (((at.x as f64 - e[0].x as f64) * dx + (at.y as f64 - e[0].y as f64) * dy)
                / length)
                .clamp(0.0, 1.0);
            let d =
                (e[0].x as f64 + t * dx - at.x as f64).hypot(e[0].y as f64 + t * dy - at.y as f64);
            (d <= radius.max(0) as f64).then_some((i, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| WireHandle::Segment(i))
}

pub fn connection_at(s: &Schematic, at: Point) -> bool {
    s.primitives
        .iter()
        .filter(|p| !p.presentation.hidden)
        .any(|p| {
            pin_end(p) == Some(at)
                || wire(p).is_some_and(|path| path.windows(2).any(|e| at.on_segment(e[0], e[1])))
        })
}

/// Compiler-like local dots, derived rather than authored objects. A bare X crossing
/// has no candidate node. Collinear overlaps count as one direction, not a branch.
pub fn junctions<'a>(primitives: impl Iterator<Item = &'a SchPrimitive>) -> Vec<(Point, u32)> {
    let primitives: Vec<_> = primitives.filter(|p| !p.presentation.hidden).collect();
    let mut candidates = BTreeSet::new();
    let mut manual = BTreeSet::new();
    let mut terminals = BTreeSet::new();
    let wires: Vec<_> = primitives
        .iter()
        .filter_map(|p| wire(p).map(|w| (*p, w)))
        .collect();
    for p in &primitives {
        if let K::Junction { position, .. } = p.kind {
            manual.insert(key(position));
        } else if let Some(at) = pin_end(p) {
            if !matches!(p.kind, K::Text { .. }) {
                terminals.insert(key(at));
            }
            candidates.insert(key(at));
        }
    }
    for (_, path) in &wires {
        candidates.extend(
            path.first()
                .into_iter()
                .chain(path.last())
                .copied()
                .map(key),
        );
    }
    let mut dots = Vec::new();
    for node in candidates.difference(&manual) {
        let at = point(*node);
        let mut rays = BTreeSet::new();
        let mut color = None;
        for (p, path) in &wires {
            for e in path.windows(2).filter(|e| at.on_segment(e[0], e[1])) {
                color = Some(p.color);
                for end in e {
                    let (dx, dy) = (end.x - at.x, end.y - at.y);
                    if dx != 0 || dy != 0 {
                        let mut a = dx.unsigned_abs();
                        let mut b = dy.unsigned_abs();
                        while b != 0 {
                            (a, b) = (b, a % b);
                        }
                        rays.insert((dx / a as i64, dy / a as i64));
                    }
                }
            }
        }
        if rays.len() >= 3 || (rays.len() >= 2 && terminals.contains(node)) {
            dots.push((at, color.unwrap()));
        }
    }
    dots
}

/// Stretch one selected segment or vertex; source wire end points remain anchored
/// for segment edits. Incident T branches follow; unrelated crossings do not.
pub fn wire_drag(
    s: &Schematic,
    id: Uuid,
    handle: WireHandle,
    delta: Point,
) -> Result<Fragment, String> {
    if !delta.valid() {
        return Err("Invalid wire drag coordinates".into());
    }
    let primitive = s.primitive(id).ok_or("Selected wire no longer exists")?;
    let path = wire(primitive).ok_or("Select a standalone wire")?;
    if path.len() < 2 {
        return Err("A wire requires at least two vertices".into());
    }
    let mut shifted = path.to_vec();
    let mut moved = BTreeMap::new();
    let mut fixed = BTreeSet::new();
    let offset = match handle {
        WireHandle::Segment(i) => {
            let e = path
                .get(i..i.checked_add(2).ok_or("Invalid segment")?)
                .ok_or("Wire segment no longer exists")?;
            if e[0] == e[1] {
                return Err("Cannot drag a zero-length segment".into());
            }
            let offset = if e[0].y == e[1].y {
                Point { x: 0, y: delta.y }
            } else if e[0].x == e[1].x {
                Point { x: delta.x, y: 0 }
            } else {
                return Err(
                    "Orthogonal segment dragging requires a horizontal or vertical wire".into(),
                );
            };
            for j in [i, i + 1] {
                shifted[j] = transform_point(path[j], Point::default(), 0, offset)?;
            }
            for p in &s.primitives {
                if p.id == id {
                    continue;
                }
                if let Some(at) = pin_end(p).filter(|at| at.on_segment(e[0], e[1])) {
                    moved.insert(
                        key(at),
                        (transform_point(at, Point::default(), 0, offset)?, id),
                    );
                }
                if let Some(other) = wire(p) {
                    for at in other
                        .first()
                        .into_iter()
                        .chain(other.last())
                        .filter(|at| at.on_segment(e[0], e[1]))
                    {
                        moved.insert(
                            key(*at),
                            (transform_point(*at, Point::default(), 0, offset)?, id),
                        );
                    }
                }
            }
            for j in [i, i + 1] {
                moved.insert(key(path[j]), (shifted[j], id));
            }
            // Segment editing keeps both ends of the original wire anchored, even
            // if one happens to be a free end rather than a component terminal.
            fixed.extend([key(path[0]), key(*path.last().unwrap())]);
            offset
        }
        WireHandle::Vertex(i) => {
            let at = *path.get(i).ok_or("Wire vertex no longer exists")?;
            shifted[i] = transform_point(at, Point::default(), 0, delta)?;
            moved.insert(key(at), (shifted[i], id));
            delta
        }
    };
    if offset == Point::default() {
        return Ok(Fragment {
            fonts: s.fonts.clone(),
            images: s.images.clone(),
            primitives: vec![primitive.clone()],
            ..Default::default()
        });
    }
    // Labels, ports and component pin hotspots are anchors, not movable decorations.
    // A stem retains a connection when a selected handle leaves one of these points.
    for p in &s.primitives {
        if !matches!(p.kind, K::Junction { .. } | K::NoErc { .. })
            && let Some(at) = pin_end(p)
        {
            fixed.insert(key(at));
        }
    }
    moved.retain(|node, (to, _)| *to != point(*node));
    let mut updated = Vec::new();
    for (i, e) in path.windows(2).enumerate() {
        let run = if handle == WireHandle::Segment(i) {
            let mut run = Vec::new();
            if i == 0 {
                run.push(e[0]);
            }
            run.extend([shifted[i], shifted[i + 1]]);
            if i + 2 == path.len() {
                run.push(e[1]);
            }
            run
        } else {
            reroute(e, shifted[i], shifted[i + 1], Point::default(), 0, offset)?
        };
        if updated.is_empty() {
            updated.extend(run);
        } else {
            updated.extend(run.into_iter().skip(1));
        }
    }
    clean(&mut updated);
    let mut replacement = primitive.clone();
    if let K::Polyline { points, .. } = &mut replacement.kind {
        *points = updated;
    }
    let mut result = Fragment {
        fonts: s.fonts.clone(),
        images: s.images.clone(),
        primitives: vec![replacement],
        ..Default::default()
    };
    // Endpoints already retained in the selected wire must not also get a stem.
    let mut anchors = fixed.clone();
    if matches!(handle, WireHandle::Segment(_)) {
        moved.remove(&key(path[0]));
        moved.remove(&key(*path.last().unwrap()));
    } else {
        // A vertex attached to a fixed pin retains the pin through a connector.
        anchors.retain(|node| moved.contains_key(node));
    }
    stretch_wires(
        s,
        &BTreeSet::from([id]),
        &moved,
        &anchors,
        (Point::default(), 0, offset),
        &mut result,
    )?;
    Ok(result)
}
