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
    let before_primitives = before.primitive_index();
    let wires: Vec<_> = s.primitives.iter().filter(|p| eligible(p)).collect();
    let dirty: BTreeSet<_> = wires
        .iter()
        .enumerate()
        .filter(|(_, p)| before_primitives.get(&p.id).copied() != Some(**p))
        .map(|(i, _)| i)
        .collect();
    if dirty.is_empty() {
        return;
    }
    // The closure is limited to overlapping wires affected by this transaction.
    // Merely opening/importing or changing a component label does not rewrite nets.
    let index = super::wire_edit::index_wires(wires.iter().map(|p| wire(p).unwrap()));
    let mut affected = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut queue: Vec<_> = dirty.into_iter().collect();
    while let Some(i) = queue.pop() {
        if !visited.insert(i) {
            continue;
        }
        let a = wires[i];
        for (edge, e) in wire(a).unwrap().windows(2).enumerate() {
            let bounds = rstar::AABB::from_corners(
                [e[0].x.min(e[1].x), e[0].y.min(e[1].y)],
                [e[0].x.max(e[1].x), e[0].y.max(e[1].y)],
            );
            for candidate in index.locate_in_envelope_intersecting(&bounds) {
                let b = wires[candidate.wire];
                if (i != candidate.wire || edge != candidate.edge)
                    && overlap(e, &[candidate.a, candidate.b])
                    && (i == candidate.wire || compatible(a, b))
                {
                    affected.insert(a.id);
                    affected.insert(b.id);
                    if !visited.contains(&candidate.wire) {
                        queue.push(candidate.wire);
                    }
                }
            }
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
    ordered.sort_by_key(|p| !before_primitives.contains_key(&p.id));
    let mut kept: Vec<(Uuid, SchPrimitive)> = Vec::new();
    for p in ordered {
        let mut runs: Vec<Vec<Point>> = Vec::new();
        let mut current = Vec::new();
        let mut own_coverage = Vec::<[Point; 2]>::new();
        for e in wire(p).unwrap().windows(2) {
            let coverage: Vec<_> = kept
                .iter()
                .flat_map(|(_, old)| wire(old).unwrap().windows(2).map(move |e| (old, e)))
                .filter(|(_, old)| collinear(e[0], e[1], old[0]) && collinear(e[0], e[1], old[1]))
                .filter(|(old, _)| compatible(p, old))
                .map(|(_, e)| [e[0], e[1]])
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
                let pa = wire(&kept[i].1).unwrap();
                let pb = wire(&kept[j].1).unwrap();
                for reverse_a in [false, true] {
                    for reverse_b in [false, true] {
                        // Reject unrelated tips before cloning geometry or styles.
                        let (at, previous) = if reverse_a {
                            (pa[0], pa[1])
                        } else {
                            (pa[pa.len() - 1], pa[pa.len() - 2])
                        };
                        let (start, next) = if reverse_b {
                            (pb[pb.len() - 1], pb[pb.len() - 2])
                        } else {
                            (pb[0], pb[1])
                        };
                        if at != start
                            || nodes.contains(&at)
                            || !collinear(previous, at, next)
                            || !compatible(&kept[i].1, &kept[j].1)
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
                        let mut a = pa.to_vec();
                        let mut b = pb.to_vec();
                        if reverse_a {
                            a.reverse();
                        }
                        if reverse_b {
                            b.reverse();
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
