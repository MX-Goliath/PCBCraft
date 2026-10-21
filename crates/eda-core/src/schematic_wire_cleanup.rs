//! Local overlap optimization. Preserve styles and electrical branch nodes; never
//! split two bare crossing interiors merely because their lines intersect.
use super::drag::{clean, pin_end, wire};
use super::selection::{piece, split_edges};
use super::*;
use std::collections::{BTreeMap, BTreeSet};

fn collinear(a: Point, b: Point, c: Point) -> bool {
    (b.x as i128 - a.x as i128) * (c.y as i128 - a.y as i128)
        == (b.y as i128 - a.y as i128) * (c.x as i128 - a.x as i128)
}
fn overlap(a: &[Point], b: &[Point]) -> bool {
    if a[0] == a[1] || b[0] == b[1] || !collinear(a[0], a[1], b[0]) || !collinear(a[0], a[1], b[1])
    {
        return false;
    }
    let (a0, a1, b0, b1) = if a[0].x != a[1].x {
        (a[0].x, a[1].x, b[0].x, b[1].x)
    } else {
        (a[0].y, a[1].y, b[0].y, b[1].y)
    };
    a0.min(a1).max(b0.min(b1)) < a0.max(a1).min(b0.max(b1))
}
fn eligible(p: &SchPrimitive) -> bool {
    wire(p).is_some()
        && !p.presentation.locked
        && !p.presentation.hidden
        && p.start_shape == 0
        && p.end_shape == 0
}
fn compatible(a: &SchPrimitive, b: &SchPrimitive) -> bool {
    let style = |p: &SchPrimitive| {
        let mut p = p.clone();
        p.id = Uuid::nil();
        p.source_index = AUTHORED;
        p.presentation.record_type = 0;
        p.kind = K::Polyline {
            points: vec![],
            electrical: "Wire".into(),
            closed: false,
        };
        p
    };
    style(a) == style(b)
}

pub(super) fn optimize(s: &mut Schematic, before: &Schematic) {
    let wires: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| eligible(p))
        .cloned()
        .collect();
    let dirty: BTreeSet<_> = wires
        .iter()
        .filter(|p| before.primitive(p.id) != Some(*p))
        .map(|p| p.id)
        .collect();
    if dirty.is_empty() {
        return;
    }
    // The closure is limited to overlapping wires affected by this transaction.
    // Merely opening/importing or changing a component label does not rewrite nets.
    let mut links = BTreeMap::<Uuid, BTreeSet<Uuid>>::new();
    for (i, a) in wires.iter().enumerate() {
        for b in &wires[i..] {
            if a.id != b.id && !compatible(a, b) {
                continue;
            }
            let pa = wire(a).unwrap();
            let pb = wire(b).unwrap();
            let same_wire = a.id == b.id;
            let intersects = pa.windows(2).enumerate().any(|(i, a)| {
                pb.windows(2)
                    .enumerate()
                    .any(|(j, b)| (i != j || !same_wire) && overlap(a, b))
            });
            if intersects {
                links.entry(a.id).or_default().insert(b.id);
                links.entry(b.id).or_default().insert(a.id);
            }
        }
    }
    let mut affected = BTreeSet::new();
    let mut queue: Vec<_> = dirty
        .into_iter()
        .filter(|id| links.contains_key(id))
        .collect();
    while let Some(id) = queue.pop() {
        if affected.insert(id)
            && let Some(neighbours) = links.get(&id)
        {
            queue.extend(neighbours);
        }
    }
    if affected.is_empty() {
        return;
    }
    let mut nodes: Vec<_> = junctions(s.primitives.iter())
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    nodes.extend(s.primitives.iter().filter_map(pin_end));
    let mut occupied: BTreeSet<_> = s
        .primitives
        .iter()
        .chain(&s.parameter_objects)
        .map(|p| p.id)
        .collect();
    let mut ordered: Vec<_> = wires.iter().filter(|p| affected.contains(&p.id)).collect();
    ordered.sort_by_key(|p| before.primitive(p.id).is_none());
    let mut kept: Vec<(Uuid, SchPrimitive)> = Vec::new();
    for p in ordered {
        let mut runs: Vec<Vec<Point>> = Vec::new();
        let mut current = Vec::new();
        let mut own_coverage = Vec::<[Point; 2]>::new();
        for e in wire(p).unwrap().windows(2) {
            let coverage: Vec<_> = kept
                .iter()
                .filter(|(_, old)| compatible(p, old))
                .flat_map(|(_, p)| wire(p).unwrap().windows(2).map(|e| [e[0], e[1]]))
                .chain(own_coverage.iter().copied())
                .filter(|old| collinear(e[0], e[1], old[0]) && collinear(e[0], e[1], old[1]))
                .collect();
            let cuts: Vec<_> = coverage.iter().flat_map(|e| e.iter().copied()).collect();
            for (a, b) in split_edges(e, &cuts) {
                if coverage
                    .iter()
                    .any(|old| a.on_segment(old[0], old[1]) && b.on_segment(old[0], old[1]))
                {
                    if current.len() > 1 {
                        runs.push(std::mem::take(&mut current));
                    }
                    current.clear();
                } else {
                    own_coverage.push([a, b]);
                    if current.last() == Some(&a) {
                        current.push(b);
                    } else {
                        if current.len() > 1 {
                            runs.push(std::mem::take(&mut current));
                        }
                        current = vec![a, b];
                    }
                }
            }
        }
        if current.len() > 1 {
            runs.push(current);
        }
        let mut first = true;
        for path in runs {
            // Preserve junction endpoints even if an overlapping wire is absorbed.
            let mut current = Vec::new();
            for (a, b) in split_edges(&path, &nodes) {
                if current.is_empty() {
                    current.push(a);
                }
                current.push(b);
                if nodes.contains(&b) {
                    let mut path = std::mem::take(&mut current);
                    clean(&mut path);
                    kept.push((p.id, piece(p, path, first, &mut occupied)));
                    first = false;
                }
            }
            if current.len() > 1 {
                clean(&mut current);
                kept.push((p.id, piece(p, current, first, &mut occupied)));
                first = false;
            }
        }
    }
    // Join contiguous collinear tips only. Branch nodes/pins/labels remain stops.
    loop {
        let mut joined = false;
        'pairs: for i in 0..kept.len() {
            for j in i + 1..kept.len() {
                if !compatible(&kept[i].1, &kept[j].1) {
                    continue;
                }
                let pa = wire(&kept[i].1).unwrap();
                let pb = wire(&kept[j].1).unwrap();
                for reverse_a in [false, true] {
                    for reverse_b in [false, true] {
                        let mut a = pa.to_vec();
                        let mut b = pb.to_vec();
                        if reverse_a {
                            a.reverse();
                        }
                        if reverse_b {
                            b.reverse();
                        }
                        let at = *a.last().unwrap();
                        if at != b[0] || nodes.contains(&at) || !collinear(a[a.len() - 2], at, b[1])
                        {
                            continue;
                        }
                        if kept.iter().enumerate().any(|(k, (_, p))| {
                            k != i
                                && k != j
                                && wire(p)
                                    .unwrap()
                                    .windows(2)
                                    .any(|e| at.on_segment(e[0], e[1]))
                        }) {
                            continue;
                        }
                        a.extend(b.into_iter().skip(1));
                        clean(&mut a);
                        if let K::Polyline { points, .. } = &mut kept[i].1.kind {
                            *points = a;
                        }
                        kept.remove(j);
                        joined = true;
                        break 'pairs;
                    }
                }
            }
        }
        if !joined {
            break;
        }
    }
    let mut replacements = BTreeMap::<Uuid, Vec<SchPrimitive>>::new();
    for (owner, p) in kept {
        replacements.entry(owner).or_default().push(p);
    }
    s.primitives = std::mem::take(&mut s.primitives)
        .into_iter()
        .flat_map(|p| {
            if affected.contains(&p.id) {
                replacements.remove(&p.id).unwrap_or_default()
            } else {
                vec![p]
            }
        })
        .collect();
}
