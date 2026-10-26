//! Flat-sheet electrical compilation. Connectivity is physical, not text proximity.
use super::*;
use rstar::{AABB, RTree, RTreeObject};
use schematic::{SchPrimitive, SchPrimitiveKind as K, TextRole};

struct NodeBounds {
    envelope: AABB<[i64; 2]>,
    index: usize,
}
impl RTreeObject for NodeBounds {
    type Envelope = AABB<[i64; 2]>;
    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}
fn envelope(p: &SchPrimitive) -> AABB<[i64; 2]> {
    if let K::Polyline { points, .. } = &p.kind {
        let (min, max) = points
            .iter()
            .fold(([i64::MAX; 2], [i64::MIN; 2]), |(min, max), p| {
                (
                    [min[0].min(p.x), min[1].min(p.y)],
                    [max[0].max(p.x), max[1].max(p.y)],
                )
            });
        AABB::from_corners(min, max)
    } else {
        let p = schematic_edit::electrical_endpoint(p).unwrap();
        AABB::from_point([p.x, p.y])
    }
}

#[derive(Clone, Debug)]
pub struct Pin {
    pub designator: String,
    pub number: String,
    pub net: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Netlist {
    pub names: Vec<String>,
    pub pins: Vec<Pin>,
    pub(super) labels: BTreeMap<String, Vec<(Uuid, Uuid)>>,
}
fn label(p: &SchPrimitive) -> Option<(u8, &str)> {
    match &p.kind {
        K::PowerPort { text, .. } => Some((0, text)),
        K::Text { text, .. } if p.text_role == Some(TextRole::NetLabel) => Some((1, text)),
        K::Port { name, .. } => Some((2, name)),
        _ => None,
    }
    .filter(|(_, s)| !s.trim().is_empty())
    .map(|(p, s)| (p, s.as_str()))
}
fn electrical(p: &SchPrimitive) -> bool {
    matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Wire")
        || schematic_edit::electrical_endpoint(p).is_some()
}
fn root(parents: &mut [usize], n: usize) -> usize {
    let mut r = n;
    while parents[r] != r {
        r = parents[r];
    }
    let mut p = n;
    while parents[p] != r {
        let next = parents[p];
        parents[p] = r;
        p = next;
    }
    r
}
fn join(parents: &mut [usize], a: usize, b: usize) {
    let a = root(parents, a);
    let b = root(parents, b);
    if a != b {
        parents[b] = a;
    }
}
/// Compile a flat project. Power ports and same-name ports have project scope;
/// net labels are sheet-local. Single-pin islands remain No Net unless named.
/// Hierarchy, buses and harnesses are rejected, never silently flattened.
pub fn compile(sheets: &[&Document]) -> Result<Netlist, String> {
    let mut nodes = vec![];
    for doc in sheets {
        let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
        if s.primitives.iter().any(|p| {
            matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Bus")
                || matches!(p.kind, K::Port { .. })
        }) && sheets.len() > 1
        {
            return Err("Multi-sheet ports require project hierarchy/net scope settings; connectivity comparison is disabled".into());
        }
        if s.primitives
            .iter()
            .any(|p| matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Bus"))
            || s.records
                .iter()
                .any(|r| matches!(r.record_type, 15 | 16 | 26 | 37 | 215 | 216 | 217 | 218))
        {
            return Err(
                "Bus, sheet-entry or harness connectivity is not yet supported by the ECO compiler"
                    .into(),
            );
        }
        if s.records.iter().any(|r| {
            r.record_type == 2
                && field(&r.fields, "ISHIDDEN").is_some_and(|v| v.eq_ignore_ascii_case("TRUE"))
        }) {
            return Err("Native hidden-pin connectivity is not yet represented by the schematic importer; pin changes are disabled".into());
        }
        for p in s
            .primitives
            .iter()
            .chain(&s.parameter_objects)
            .filter(|p| electrical(p))
        {
            if p.component.is_some_and(|id| {
                s.components
                    .iter()
                    .find(|c| c.id == id)
                    .is_none_or(|c| matches!(c.settings.kind, 1 | 2 | 6))
            }) {
                continue;
            }
            nodes.push((*doc, p));
        }
    }
    let mut parents: Vec<_> = (0..nodes.len()).collect();
    let mut trees: BTreeMap<Uuid, Vec<NodeBounds>> = BTreeMap::new();
    for (n, (doc, p)) in nodes.iter().enumerate() {
        trees.entry(doc.id).or_default().push(NodeBounds {
            index: n,
            envelope: envelope(p),
        });
    }
    let trees: BTreeMap<_, _> = trees
        .into_iter()
        .map(|(id, nodes)| (id, RTree::bulk_load(nodes)))
        .collect();
    let mut names: HashMap<(Option<Uuid>, String), usize> = HashMap::new();
    let mut pins = HashMap::new();
    for (a, (doc, p)) in nodes.iter().enumerate() {
        if let Some((priority, name)) = label(p) {
            let scope = if priority == 1 { Some(doc.id) } else { None };
            if let Some(&b) = names.get(&(scope, name.to_uppercase())) {
                join(&mut parents, a, b);
            } else {
                names.insert((scope, name.to_uppercase()), a);
            }
        }
        if let K::Pin { number, .. } = &p.kind
            && let Some(id) = p.component
        {
            let c = doc
                .schematic
                .as_ref()
                .unwrap()
                .components
                .iter()
                .find(|c| c.id == id)
                .unwrap();
            if let Some(b) = pins.insert((c.designator.to_uppercase(), number.to_uppercase()), a) {
                join(&mut parents, a, b);
            }
        }
        for candidate in trees[&doc.id].locate_in_envelope_intersecting(&envelope(p)) {
            let b = candidate.index;
            if b < a && crate::cross_probe::touches(p, nodes[b].1) {
                join(&mut parents, a, b);
            }
        }
    }
    let mut islands: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for n in 0..nodes.len() {
        islands.entry(root(&mut parents, n)).or_default().push(n);
    }
    let mut out = Netlist::default();
    let mut assigned: HashMap<(String, String), Option<String>> = HashMap::new();
    let mut unique_names = HashSet::new();
    for indices in islands.values() {
        let mut pins = vec![];
        let mut labels = vec![];
        for &n in indices {
            let (doc, p) = nodes[n];
            if let Some((priority, name)) = label(p) {
                labels.push((priority, name.to_owned(), doc.id, p.id));
            }
            if let K::Pin { number, .. } = &p.kind
                && let Some(id) = p.component
            {
                let c = doc
                    .schematic
                    .as_ref()
                    .unwrap()
                    .components
                    .iter()
                    .find(|c| c.id == id)
                    .unwrap();
                pins.push((c.designator.clone(), number.clone()));
            }
        }
        if pins.is_empty() {
            continue;
        }
        pins.sort();
        pins.dedup();
        labels.sort();
        let net = if let Some((_, name, ..)) = labels.first() {
            Some(name.clone())
        } else if pins.len() > 1 {
            Some(format!("Net{}_{}", pins[0].0, pins[0].1))
        } else {
            None
        };
        if let Some(name) = &net {
            if !unique_names.insert(name.to_uppercase()) {
                return Err(format!(
                    "Different sheet-local nets share the name {name}; project net-scope compilation is required"
                ));
            }
            if name.contains(['[', ']', '{', '}']) {
                return Err(format!(
                    "Bus/harness net {name} requires expansion; connectivity comparison is disabled"
                ));
            }
            let aliases: Vec<_> = labels.iter().map(|(_, s, _, _)| s.to_uppercase()).collect();
            if aliases.iter().any(|s| s != &name.to_uppercase()) {
                return Err(format!(
                    "Conflicting labels on net {name}; resolve before synchronization"
                ));
            }
            out.names.push(name.clone());
            out.labels
                .entry(name.clone())
                .or_default()
                .extend(labels.iter().map(|(_, _, d, id)| (*d, *id)));
        }
        for (designator, number) in pins {
            let key = (designator.to_uppercase(), number.to_uppercase());
            if let Some(old) = assigned.insert(key, net.clone())
                && old != net
            {
                return Err(format!(
                    "Repeated pin {designator}-{number} belongs to different nets"
                ));
            }
            out.pins.push(Pin {
                designator,
                number,
                net: net.clone(),
            });
        }
    }
    out.names.sort();
    out.names.dedup();
    Ok(out)
}
