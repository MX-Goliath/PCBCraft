use super::*;
use crate::schematic_bus as bus;

type Channels = BTreeMap<usize, BTreeMap<String, Vec<Object>>>;
pub(super) fn channels(
    sheets: &[&Document],
    nodes: &[(&Document, &SchPrimitive)],
    groups: &BTreeMap<usize, Vec<usize>>,
    bundles: &HashSet<Object>,
    tolerance: i64,
) -> Result<Channels, String> {
    let mut result = Channels::new();
    if !nodes.iter().any(|(_, p)| bus::bus_path(p).is_some()) {
        return Ok(result);
    }
    let scalar: Vec<_> = sheets
        .iter()
        .flat_map(|d| {
            d.schematic
                .as_ref()
                .unwrap()
                .primitives
                .iter()
                .filter(|p| electrical(p) && !bundles.contains(&(d.id, p.id)))
                .map(|p| (*d, p))
        })
        .collect();
    let mut parents: Vec<_> = (0..scalar.len()).collect();
    let mut trees: BTreeMap<Uuid, Vec<NodeBounds>> = BTreeMap::new();
    for (i, (d, p)) in scalar.iter().enumerate() {
        trees.entry(d.id).or_default().push(NodeBounds {
            index: i,
            envelope: bounds(p, tolerance),
        });
    }
    let trees: BTreeMap<_, _> = trees
        .into_iter()
        .map(|(id, n)| (id, RTree::bulk_load(n)))
        .collect();
    for (i, (d, p)) in scalar.iter().enumerate() {
        for n in trees[&d.id].locate_in_envelope_intersecting(&bounds(p, 0)) {
            if n.index < i && touches(p, scalar[n.index].1, tolerance) {
                join(&mut parents, i, n.index);
            }
        }
    }
    let mut names: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();
    for (i, (_, p)) in scalar.iter().enumerate() {
        if let Some((_, name)) = label(p) {
            names
                .entry(root(&mut parents, i))
                .or_default()
                .insert(name.to_uppercase());
        }
    }
    for (&group, members) in groups {
        let paths: Vec<_> = members
            .iter()
            .filter_map(|i| bus::bus_path(nodes[*i].1).map(|p| (nodes[*i].0.id, p)))
            .collect();
        if paths.is_empty() {
            continue;
        }
        let channels = result.entry(group).or_default();
        let mut valid = BTreeSet::new();
        for i in members {
            let p = nodes[*i].1;
            let name = label(p).map(|(_, n)| n).or({
                if let K::HarnessEntry { name, .. } = &p.kind {
                    Some(name.as_str())
                } else {
                    None
                }
            });
            if let Some(name) = name
                && let Some(bits) = bus::members(name)?
            {
                valid.extend(bits.into_iter().map(|s| s.to_uppercase()));
            }
        }
        if valid.is_empty() {
            return Err("Bus requires a numeric range label, Port or Harness Entry".into());
        }
        let on_bus = |doc: Uuid, at: Point| {
            paths.iter().filter(|(d, _)| *d == doc).any(|(_, p)| {
                p.windows(2)
                    .any(|e| near_segment(at, e[0], e[1], tolerance))
            })
        };
        let entries: Vec<_> = sheets
            .iter()
            .flat_map(|d| {
                d.schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .filter(|p| bus::is_entry(p))
                    .map(|p| (*d, p))
            })
            .filter_map(|(d, p)| {
                let points = bus::entry_ends(p);
                (points.len() == 2).then_some((d.id, points))
            })
            .collect();
        for (i, (d, p)) in scalar.iter().enumerate() {
            let wire_ends = if let K::Polyline {
                points, electrical, ..
            } = &p.kind
            {
                if electrical != "Wire" {
                    continue;
                }
                points
                    .first()
                    .into_iter()
                    .chain(points.last())
                    .copied()
                    .collect::<Vec<_>>()
            } else {
                continue;
            };
            let attached = wire_ends.iter().any(|at| {
                on_bus(d.id, *at)
                    || entries.iter().filter(|(id, _)| *id == d.id).any(|(_, e)| {
                        (on_bus(d.id, e[0]) && near_segment(*at, e[1], e[1], tolerance))
                            || (on_bus(d.id, e[1]) && near_segment(*at, e[0], e[0], tolerance))
                    })
            });
            if !attached {
                continue;
            }
            let names = names
                .get(&root(&mut parents, i))
                .ok_or("Bus member wire requires a net label")?;
            let matches: Vec<_> = names.intersection(&valid).collect();
            if matches.len() != 1 {
                return Err("Bus member must identify exactly one bit in the bus range".into());
            }
            channels
                .entry(matches[0].clone())
                .or_default()
                .push((d.id, p.id));
        }
    }
    Ok(result)
}
