//! Expand harness channels before scalar net union. Harness types never name or short nets.
use super::*;
use crate::schematic_harness;
#[path = "schematic_harness_bus_compile.rs"]
mod bus_compile;

type Object = (Uuid, Uuid);
fn definition_channels(
    definitions: &BTreeMap<String, BTreeMap<String, String>>,
    kind: &str,
    stack: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let key = kind.to_uppercase();
    if stack.contains(&key) || stack.len() >= 32 {
        return Err(format!("Cyclic Harness Definition: {kind}"));
    }
    let entries = definitions
        .get(&key)
        .ok_or_else(|| format!("Unknown Harness Type: {kind}"))?;
    stack.push(key);
    let mut out = vec![];
    for (name, nested) in entries {
        if !nested.is_empty() {
            for leaf in definition_channels(definitions, nested, stack)? {
                out.push(format!("{name}.{leaf}"));
            }
        } else if let Some(bits) = crate::schematic_bus::members(name)? {
            out.extend(bits.into_iter().map(|n| n.to_uppercase()));
        } else {
            out.push(name.clone());
        }
        if out.len() > 100_000 {
            return Err("Harness definition expansion exceeds channel limit".into());
        }
    }
    stack.pop();
    Ok(out)
}
#[derive(Default)]
pub(crate) struct Expansion {
    pub bundles: HashSet<Object>,
    pub joins: Vec<(Object, Object)>,
    pub names: HashMap<Object, String>,
    pub types: HashMap<Object, String>,
}
fn path(p: &SchPrimitive) -> Option<&[Point]> {
    match &p.kind {
        K::Polyline {
            points, electrical, ..
        } if matches!(electrical.as_str(), "Signal Harness" | "Bus") => Some(points),
        _ => None,
    }
}
fn ends(p: &SchPrimitive) -> Vec<Point> {
    if let Some(points) = path(p) {
        points
            .first()
            .into_iter()
            .chain(points.last())
            .copied()
            .collect()
    } else if let Some(p) = schematic_harness::terminal(p) {
        vec![p]
    } else {
        schematic_edit::electrical_endpoints(p).collect()
    }
}
fn connected(a: &SchPrimitive, b: &SchPrimitive, tolerance: i64) -> bool {
    let ab = crate::schematic_bus::bus_path(a).is_some();
    let bb = crate::schematic_bus::bus_path(b).is_some();
    if path(a).is_some() && path(b).is_some() && ab != bb {
        return false;
    }
    if ab
        && (matches!(b.kind, K::HarnessConnector { .. }) || !b.presentation.harness_type.is_empty())
        || bb
            && (matches!(a.kind, K::HarnessConnector { .. })
                || !a.presentation.harness_type.is_empty())
    {
        return false;
    }
    let ae = ends(a);
    let be = ends(b);
    if let Some(w) = path(a)
        && be.iter().any(|p| {
            w.windows(2)
                .any(|e| near_segment(*p, e[0], e[1], tolerance))
        })
    {
        return true;
    }
    if let Some(w) = path(b)
        && ae.iter().any(|p| {
            w.windows(2)
                .any(|e| near_segment(*p, e[0], e[1], tolerance))
        })
    {
        return true;
    }
    path(a).is_none()
        && path(b).is_none()
        && ae
            .iter()
            .any(|p| be.iter().any(|q| near_segment(*p, *q, *q, tolerance)))
}
pub(crate) fn expand(
    sheets: &[&Document],
    options: &ProjectOptions,
    links: &[project::SheetLink],
    hierarchical: bool,
) -> Result<Expansion, String> {
    let mut nodes = vec![];
    let mut definitions: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for d in options
        .harness_definitions
        .iter()
        .cloned()
        .chain(sheets.iter().flat_map(|d| {
            schematic_harness::definitions(d.schematic.as_ref().unwrap()).unwrap_or_default()
        }))
    {
        let entries = d
            .entries
            .into_iter()
            .map(|(n, t)| (n.to_uppercase(), t.to_uppercase()))
            .collect();
        if let Some(old) = definitions.insert(d.name.to_uppercase(), entries)
            && old != definitions[&d.name.to_uppercase()]
        {
            return Err(format!("Conflicting Harness Definition for {}", d.name));
        }
    }
    for doc in sheets {
        let s = doc.schematic.as_ref().unwrap();
        schematic_harness::definitions(s)?;
        let excluded = crate::schematic_compile::exclusions(s);
        for p in s
            .primitives
            .iter()
            .filter(|p| !p.presentation.hidden && !excluded.primitives.contains(&p.id))
        {
            if path(p).is_some()
                || matches!(
                    p.kind,
                    K::HarnessConnector { .. }
                        | K::HarnessEntry { .. }
                        | K::Port { .. }
                        | K::SheetEntry { .. }
                )
                || p.text_role == Some(TextRole::NetLabel)
            {
                if path(p).is_some_and(|p| p.len() < 2) {
                    return Err("Bus or signal harness requires at least two vertices".into());
                }
                nodes.push((*doc, p));
            }
        }
    }
    let mut parents: Vec<_> = (0..nodes.len()).collect();
    let tolerance = options.connection_precision_nm.unwrap_or(0);
    // Spatial pruning applies to harness lines as well as port/connector hotspots.
    let mut trees: BTreeMap<Uuid, Vec<NodeBounds>> = BTreeMap::new();
    let envelope = |p: &SchPrimitive| {
        if let Some(w) = path(p) {
            AABB::from_points(&w.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>())
        } else {
            AABB::from_points(&ends(p).iter().map(|p| [p.x, p.y]).collect::<Vec<_>>())
        }
    };
    for (i, (d, p)) in nodes.iter().enumerate() {
        let e = envelope(p);
        trees.entry(d.id).or_default().push(NodeBounds {
            index: i,
            envelope: AABB::from_corners(
                e.lower().map(|x| x - tolerance),
                e.upper().map(|x| x + tolerance),
            ),
        });
    }
    let trees: BTreeMap<_, _> = trees
        .into_iter()
        .map(|(id, n)| (id, RTree::bulk_load(n)))
        .collect();
    for (i, (d, p)) in nodes.iter().enumerate() {
        for n in trees[&d.id].locate_in_envelope_intersecting(&envelope(p)) {
            if n.index < i && connected(p, nodes[n.index].1, tolerance) {
                join(&mut parents, i, n.index);
            }
        }
    }
    let mut active = HashSet::new();
    for (i, (_, p)) in nodes.iter().enumerate() {
        if path(p).is_some()
            || matches!(p.kind, K::HarnessConnector { .. })
            || !p.presentation.harness_type.is_empty()
        {
            active.insert(root(&mut parents, i));
        }
    }
    let is_active =
        |parents: &mut [usize], active: &HashSet<usize>, i| active.contains(&root(parents, i));
    // Same-name ports/labels obey project scope, separately from scalar wire names.
    let mut named: HashMap<(Option<Uuid>, u8, String), usize> = HashMap::new();
    for (i, (d, p)) in nodes.iter().enumerate() {
        if !is_active(&mut parents, &active, i) {
            continue;
        }
        if let Some((priority @ (1 | 2), name)) = label(p) {
            let local = if priority == 1 {
                options.scope != NetScope::Global
            } else {
                hierarchical
            };
            let key = (local.then_some(d.id), priority, name.to_uppercase());
            if let Some(&j) = named.get(&key) {
                join(&mut parents, i, j);
            } else {
                named.insert(key, i);
            }
        }
    }
    for link in links.iter().filter(|_| hierarchical) {
        for (i, (_, entry)) in nodes
            .iter()
            .enumerate()
            .filter(|(_, (d, p))| d.id == link.parent && p.presentation.parent == Some(link.symbol))
        {
            let K::SheetEntry { name, .. } = &entry.kind else {
                continue;
            };
            for (j, (_, port)) in nodes.iter().enumerate().filter(|(_, (d, p))| {
                d.id == link.child
                    && matches!(&p.kind,K::Port{name:n,..} if n.eq_ignore_ascii_case(name))
            }) {
                if is_active(&mut parents, &active, i) || is_active(&mut parents, &active, j) {
                    if !entry.presentation.harness_type.is_empty()
                        && !port.presentation.harness_type.is_empty()
                        && !entry
                            .presentation
                            .harness_type
                            .eq_ignore_ascii_case(&port.presentation.harness_type)
                    {
                        return Err(format!("Conflicting Harness Type on Sheet Entry {name}"));
                    }
                    join(&mut parents, i, j);
                }
            }
        }
    }
    // Recompute membership after scope/hierarchy unions.
    active.clear();
    for (i, (_, p)) in nodes.iter().enumerate() {
        if path(p).is_some()
            || matches!(p.kind, K::HarnessConnector { .. })
            || !p.presentation.harness_type.is_empty()
        {
            active.insert(root(&mut parents, i));
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut out = Expansion::default();
    for (i, (d, p)) in nodes.iter().enumerate() {
        let r = root(&mut parents, i);
        if active.contains(&r) {
            groups.entry(r).or_default().push(i);
            out.bundles.insert((d.id, p.id));
        }
    }
    let lookup: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, (d, p))| ((d.id, p.id), i))
        .collect();
    for members in groups.values() {
        let types: BTreeSet<_> = members
            .iter()
            .map(|i| &nodes[*i].1.presentation.harness_type)
            .filter(|s| !s.is_empty() && s.as_str() != "*")
            .map(|s| s.to_uppercase())
            .collect();
        if types.len() > 1 {
            return Err(format!(
                "Conflicting Harness Types: {}",
                types.into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        if let Some(kind) = types.first() {
            let display = members
                .iter()
                .map(|i| &nodes[*i].1.presentation.harness_type)
                .find(|n| n.eq_ignore_ascii_case(kind))
                .unwrap();
            for i in members {
                let (d, p) = nodes[*i];
                if matches!(
                    p.kind,
                    K::Port { .. } | K::SheetEntry { .. } | K::HarnessEntry { .. }
                ) {
                    out.types.insert((d.id, p.id), display.clone());
                }
            }
        }
        if types.is_empty() && members.iter().any(|i| matches!(&nodes[*i].1.kind,K::Polyline{electrical,..} if electrical=="Signal Harness")) {
            return Err("Missing Harness Type on Signal Harness".into());
        }
    }
    struct Flatten<'a> {
        nodes: &'a [(&'a Document, &'a SchPrimitive)],
        groups: &'a BTreeMap<usize, Vec<usize>>,
        lookup: &'a HashMap<Object, usize>,
        roots: &'a [usize],
        bus: &'a BTreeMap<usize, BTreeMap<String, Vec<Object>>>,
    }
    impl Flatten<'_> {
        fn group(
            &self,
            group: usize,
            prefix: &str,
            stack: &mut Vec<usize>,
            out: &mut BTreeMap<String, Vec<Object>>,
        ) -> Result<(), String> {
            if stack.contains(&group) || stack.len() > 32 {
                return Err("Cyclic or excessively nested signal harness".into());
            }
            stack.push(group);
            if let Some(channels) = self.bus.get(&group) {
                for (name, objects) in channels {
                    let key = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{prefix}.{name}")
                    };
                    out.entry(key).or_default().extend(objects);
                }
                stack.pop();
                return Ok(());
            }
            for i in self.groups.get(&group).into_iter().flatten() {
                let (d, p) = self.nodes[*i];
                if !matches!(p.kind, K::HarnessConnector { .. }) {
                    continue;
                }
                for entry in d
                    .schematic
                    .as_ref()
                    .unwrap()
                    .primitives
                    .iter()
                    .filter(|e| e.presentation.parent == Some(p.id) && !e.presentation.hidden)
                {
                    let K::HarnessEntry { name, .. } = &entry.kind else {
                        continue;
                    };
                    let channel = if crate::schematic_bus::members(name)?.is_some() {
                        prefix.to_owned()
                    } else if prefix.is_empty() {
                        name.to_uppercase()
                    } else {
                        format!("{prefix}.{}", name.to_uppercase())
                    };
                    let object = (d.id, entry.id);
                    let nested = self
                        .lookup
                        .get(&object)
                        .map(|i| self.roots[*i])
                        .filter(|r| self.groups.contains_key(r));
                    if let Some(nested) = nested {
                        self.group(nested, &channel, stack, out)?;
                    } else {
                        out.entry(channel).or_default().push(object);
                    }
                }
            }
            stack.pop();
            Ok(())
        }
    }
    let roots: Vec<_> = (0..nodes.len()).map(|i| root(&mut parents, i)).collect();
    let bus = bus_compile::channels(sheets, &nodes, &groups, &out.bundles, tolerance)?;
    let flatten = Flatten {
        nodes: &nodes,
        groups: &groups,
        lookup: &lookup,
        roots: &roots,
        bus: &bus,
    };
    for (&group, members) in &groups {
        let mut channels = BTreeMap::new();
        flatten.group(group, "", &mut vec![], &mut channels)?;
        if let Some(kind) = members
            .iter()
            .map(|i| &nodes[*i].1.presentation.harness_type)
            .find(|n| !n.is_empty() && n.as_str() != "*")
        {
            for channel in definition_channels(&definitions, kind, &mut vec![])? {
                channels.entry(channel).or_default();
            }
        }
        let labels: Vec<_> = members
            .iter()
            .filter_map(|i| {
                let (_, p) = nodes[*i];
                (p.text_role == Some(TextRole::NetLabel))
                    .then(|| {
                        label(p)
                            .filter(|(_, s)| !s.contains('['))
                            .map(|(_, s)| s.to_owned())
                    })
                    .flatten()
            })
            .collect();
        if labels.iter().any(|n| !n.eq_ignore_ascii_case(&labels[0])) {
            return Err("Conflicting net labels on signal harness".into());
        }
        for (channel, mut objects) in channels {
            // Connector-free harnesses expose qualified scalar names on any
            // sheet in this physical/hierarchical bundle. A type alone never
            // joins different instances of the harness.
            if let Some(prefix) = labels.first() {
                let qualified = format!("{prefix}.{channel}");
                let docs: BTreeSet<_> = members.iter().map(|i| nodes[*i].0.id).collect();
                for d in sheets.iter().filter(|d| docs.contains(&d.id)) {
                    for p in &d.schematic.as_ref().unwrap().primitives {
                        if !p.presentation.hidden
                            && !out.bundles.contains(&(d.id, p.id))
                            && p.text_role == Some(TextRole::NetLabel)
                            && label(p)
                                .is_some_and(|(_, name)| name.eq_ignore_ascii_case(&qualified))
                        {
                            objects.push((d.id, p.id));
                        }
                    }
                }
            }
            if let Some(&first) = objects.first() {
                out.joins
                    .extend(objects.iter().skip(1).map(|&o| (first, o)));
                if let Some(label) = labels.first() {
                    for object in objects {
                        out.names.insert(object, format!("{label}.{channel}"));
                    }
                }
            }
        }
    }
    Ok(out)
}
