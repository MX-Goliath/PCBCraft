//! Electrical compilation with explicit Sheet Symbol/Entry/Port hierarchy.
use super::*;
use rstar::{AABB, RTree, RTreeObject};
use schematic::{SchPrimitive, SchPrimitiveKind as K, TextRole};
#[path = "schematic_harness_compile.rs"]
pub(crate) mod harness;

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
        let endpoints: Vec<_> = schematic_edit::electrical_endpoints(p)
            .map(|p| [p.x, p.y])
            .collect();
        AABB::from_points(&endpoints)
    }
}

#[derive(Clone, Debug)]
pub struct Pin {
    pub sheet: Uuid,
    pub designator: String,
    pub component: Uuid,
    pub number: String,
    pub net: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct Netlist {
    pub names: Vec<String>,
    pub pins: Vec<Pin>,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub unresolved_pins: BTreeSet<(Uuid, Uuid, String)>,
    /// Export omits these pins when single-pin nets are disabled. Omission alone
    /// does not establish that native ECO should disconnect an existing PCB net.
    pub omitted_single_pin_pins: BTreeSet<(Uuid, Uuid, String)>,
    pub(super) labels: BTreeMap<String, Vec<(Uuid, Uuid)>>,
}
fn label(p: &SchPrimitive) -> Option<(u8, &str)> {
    match &p.kind {
        K::PowerPort { text, .. } => Some((0, text)),
        K::Text { text, .. } if p.text_role == Some(TextRole::NetLabel) => Some((1, text)),
        K::Port { name, .. } => Some((2, name)),
        K::SheetEntry { name, .. } => Some((3, name)),
        _ => None,
    }
    .filter(|(_, s)| !s.trim().is_empty())
    .map(|(p, s)| (p, s.as_str()))
}
fn electrical(p: &SchPrimitive) -> bool {
    matches!(&p.kind,K::Polyline{electrical,..} if electrical=="Wire")
        || schematic_edit::electrical_endpoint(p).is_some()
}
fn naming_label<'a>(p: &'a SchPrimitive, options: &ProjectOptions) -> Option<(u8, &'a str)> {
    label(p).filter(|(kind, _)| match kind {
        2 => options.allow_port_names,
        3 => options.allow_entry_names,
        _ => true,
    })
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
// Native SCH coordinate conversions can leave sub-mil gaps. Apply the
// project compiler tolerance only to electrical hotspots, never to the drawing.
fn near_segment(p: Point, a: Point, b: Point, tolerance: i64) -> bool {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let x = (p.x - a.x) as f64;
    let y = (p.y - a.y) as f64;
    let length = dx * dx + dy * dy;
    let t = if length == 0. {
        0.
    } else {
        ((x * dx + y * dy) / length).clamp(0., 1.)
    };
    (x - t * dx).hypot(y - t * dy) <= tolerance as f64
}
fn touches(a: &SchPrimitive, b: &SchPrimitive, tolerance: i64) -> bool {
    if tolerance == 0 {
        return crate::cross_probe::touches(a, b);
    }
    let wire = |p: &SchPrimitive| match &p.kind {
        K::Polyline {
            points, electrical, ..
        } if electrical == "Wire" => Some(points.clone()),
        _ => None,
    };
    let aw = wire(a);
    let bw = wire(b);
    let endpoints = |p: &SchPrimitive, w: &Option<Vec<Point>>| -> Vec<Point> {
        if let Some(w) = w {
            w.first().into_iter().chain(w.last()).copied().collect()
        } else {
            schematic_edit::electrical_endpoints(p).collect()
        }
    };
    let ap = endpoints(a, &aw);
    let bp = endpoints(b, &bw);
    if let Some(w) = &bw
        && ap.iter().any(|p| {
            w.windows(2)
                .any(|e| near_segment(*p, e[0], e[1], tolerance))
        })
    {
        return true;
    }
    if let Some(w) = &aw
        && bp.iter().any(|p| {
            w.windows(2)
                .any(|e| near_segment(*p, e[0], e[1], tolerance))
        })
    {
        return true;
    }
    aw.is_none()
        && bw.is_none()
        && ap
            .iter()
            .any(|p| bp.iter().any(|q| near_segment(*p, *q, *q, tolerance)))
}
fn bounds(p: &SchPrimitive, tolerance: i64) -> AABB<[i64; 2]> {
    let e = envelope(p);
    AABB::from_corners(
        e.lower().map(|v| v - tolerance),
        e.upper().map(|v| v + tolerance),
    )
}
/// Compile physical connectivity and unique sheet instances. Hierarchical ports
/// and labels are sheet-local, power ports global, entries join matching child
/// ports. Multichannel annotation, buses and harnesses are rejected explicitly.
pub fn compile(sheets: &[&Document]) -> Result<Netlist, String> {
    compile_project(sheets, &ProjectOptions::default())
}

pub fn compile_project(sheets: &[&Document], options: &ProjectOptions) -> Result<Netlist, String> {
    if options
        .connection_precision_nm
        .is_some_and(|n| !(0..=2_540).contains(&n))
    {
        return Err("Unsupported schematic connection precision".into());
    }
    if let Some(reason) = &options.unsupported_connectivity {
        return Err(reason.clone());
    }
    let graph = project::hierarchy(sheets, options)?;
    let hierarchical = matches!(
        graph.scope,
        NetScope::Hierarchical | NetScope::StrictHierarchical
    );
    let depths = graph.depths;
    let links = graph.links;
    let harness = harness::expand(sheets, options, &links, hierarchical)?;
    let excluded: HashMap<_, _> = sheets
        .iter()
        .map(|d| {
            (
                d.id,
                crate::schematic_compile::exclusions(d.schematic.as_ref().unwrap()),
            )
        })
        .collect();
    let mut pin_owners = HashMap::new();
    for doc in sheets {
        let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
        let mut groups: HashMap<String, Vec<&schematic::Component>> = HashMap::new();
        for c in s.components.iter().filter(|c| {
            !matches!(c.settings.kind, 1 | 2 | 6) && !excluded[&doc.id].components.contains(&c.id)
        }) {
            groups
                .entry(project::logical_designator(c).to_uppercase())
                .or_default()
                .push(c);
        }
        for (name, cs) in groups {
            let first = cs[0];
            let mut parts = HashSet::new();
            let multipart = cs.len() == 1
                || cs.iter().all(|c| {
                    c.settings.part_count > 1
                        && c.settings.part_count == first.settings.part_count
                        && c.library_reference == first.library_reference
                        && c.settings.kind == first.settings.kind
                        && parts.insert(c.part)
                });
            if !multipart && !options.review_annotation {
                return Err(format!(
                    "Duplicate local designator {name} on {}; annotate distinct components before compiling connectivity",
                    doc.name
                ));
            }
            for c in cs {
                pin_owners.insert((doc.id, c.id), if multipart { first.id } else { c.id });
            }
        }
    }
    let mut nodes = vec![];
    for doc in sheets {
        let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
        if s.records.iter().any(|r| {
            r.record_type == 2
                && !excluded[&doc.id].records.contains(&r.index)
                && field(&r.fields, "ISHIDDEN")
                    .is_some_and(|v| v.eq_ignore_ascii_case("TRUE") || v.eq_ignore_ascii_case("T"))
        }) {
            return Err("Native hidden-pin connectivity is not yet represented by the schematic importer; pin changes are disabled".into());
        }
        for p in s
            .primitives
            .iter()
            .chain(&s.parameter_objects)
            .filter(|p| electrical(p) && !excluded[&doc.id].primitives.contains(&p.id))
        {
            if harness.bundles.contains(&(doc.id, p.id)) {
                continue;
            }
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
    let tolerance = options.connection_precision_nm.unwrap_or(0);
    let mut parents: Vec<_> = (0..nodes.len()).collect();
    let mut trees: BTreeMap<Uuid, Vec<NodeBounds>> = BTreeMap::new();
    for (n, (doc, p)) in nodes.iter().enumerate() {
        trees.entry(doc.id).or_default().push(NodeBounds {
            index: n,
            envelope: bounds(p, tolerance),
        });
    }
    let trees: BTreeMap<_, _> = trees
        .into_iter()
        .map(|(id, nodes)| (id, RTree::bulk_load(nodes)))
        .collect();
    let mut names: HashMap<(Option<Uuid>, u8, String), usize> = HashMap::new();
    let mut pins = HashMap::new();
    for (a, (doc, p)) in nodes.iter().enumerate() {
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
            if let Some(b) = pins.insert(
                (doc.id, pin_owners[&(doc.id, c.id)], number.to_uppercase()),
                a,
            ) {
                join(&mut parents, a, b);
            }
        }
        for candidate in trees[&doc.id].locate_in_envelope_intersecting(&bounds(p, 0)) {
            let b = candidate.index;
            if b < a && touches(p, nodes[b].1, tolerance) {
                join(&mut parents, a, b);
            }
        }
    }
    let node_indices: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, (d, p))| ((d.id, p.id), i))
        .collect();
    for (a, b) in &harness.joins {
        if let (Some(&a), Some(&b)) = (node_indices.get(a), node_indices.get(b)) {
            join(&mut parents, a, b);
        }
    }
    // Resolve sheet-local labels and ports first, so a wire separated into
    // named islands still localizes its power name when connected to a Port.
    for (a, (doc, p)) in nodes.iter().enumerate() {
        if let Some((priority @ (1 | 2), name)) = label(p) {
            let local = if priority == 1 {
                graph.scope != NetScope::Global
            } else {
                hierarchical
            };
            let key = (local.then_some(doc.id), priority, name.to_uppercase());
            if let Some(&b) = names.get(&key) {
                join(&mut parents, a, b);
            } else {
                names.insert(key, a);
            }
        }
    }
    let mut port_roots = HashSet::new();
    for (n, (d, p)) in nodes.iter().enumerate() {
        if matches!(p.kind, K::Port { .. }) {
            port_roots.insert((d.id, root(&mut parents, n)));
        }
    }
    let mut localized_power = HashSet::new();
    if hierarchical {
        for (n, (d, p)) in nodes.iter().enumerate() {
            if let K::PowerPort { text, .. } = &p.kind
                && port_roots.contains(&(d.id, root(&mut parents, n)))
            {
                localized_power.insert((d.id, text.to_uppercase()));
            }
        }
    }
    for (a, (doc, p)) in nodes.iter().enumerate() {
        if let K::PowerPort { text, .. } = &p.kind {
            let local = graph.scope == NetScope::StrictHierarchical
                || localized_power.contains(&(doc.id, text.to_uppercase()));
            let key = (local.then_some(doc.id), 0, text.to_uppercase());
            if let Some(&b) = names.get(&key) {
                join(&mut parents, a, b);
            } else {
                names.insert(key, a);
            }
        }
    }
    for project::SheetLink {
        parent,
        symbol,
        child,
    } in links.into_iter().filter(|_| hierarchical)
    {
        for (entry_index, (_, entry)) in nodes
            .iter()
            .enumerate()
            .filter(|(_, (d, p))| d.id == parent && p.presentation.parent == Some(symbol))
        {
            let K::SheetEntry { name, .. } = &entry.kind else {
                continue;
            };
            let ports: Vec<_> = nodes.iter().enumerate().filter(|(_, (d,p))| d.id == child && matches!(&p.kind,K::Port {name:port,..} if port.eq_ignore_ascii_case(name))).map(|(n,_)|n).collect();
            if ports.is_empty() {
                return Err(format!(
                    "Sheet Entry {name} has no matching port on child sheet"
                ));
            }
            for port in ports {
                join(&mut parents, entry_index, port);
            }
        }
    }
    let mut islands: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for n in 0..nodes.len() {
        islands.entry(root(&mut parents, n)).or_default().push(n);
    }
    let naming_key = |priority: u8, sheet: Uuid, name: &str| {
        let rank = if !options.power_names_priority && priority <= 1 {
            1 - priority
        } else {
            priority
        };
        (
            rank,
            if options.higher_level_names_priority {
                depths[&sheet]
            } else {
                0
            },
            name.to_lowercase(),
            depths[&sheet],
            name.to_owned(),
        )
    };
    let sheet_name = |id: Uuid| {
        let doc = sheets.iter().find(|d| d.id == id).unwrap();
        let name = options
            .sheet_paths
            .get(&id)
            .map(String::as_str)
            .unwrap_or(&doc.name);
        let name = name
            .rsplit_once('.')
            .filter(|(_, ext)| ext.eq_ignore_ascii_case("SchDoc"))
            .map_or(name, |(base, _)| base);
        if !options.sheet_paths.contains_key(&id)
            && sheets
                .iter()
                .filter(|d| d.name.eq_ignore_ascii_case(&doc.name))
                .count()
                > 1
        {
            format!("{name}{}", graph.paths[&id])
        } else {
            name.to_owned()
        }
    };
    let mut out = Netlist::default();
    let mut base_counts = HashMap::<String, usize>::new();
    if hierarchical {
        for indices in islands.values() {
            if !indices.iter().any(|n| {
                matches!(nodes[*n].1.kind, K::Pin { .. }) && nodes[*n].1.component.is_some()
            }) {
                continue;
            }
            if let Some((_, name, _)) = indices
                .iter()
                .filter_map(|n| {
                    naming_label(nodes[*n].1, options)
                        .map(|(priority, name)| (priority, name, nodes[*n].0.id))
                })
                .min_by_key(|a| naming_key(a.0, a.2, a.1))
            {
                *base_counts.entry(name.to_uppercase()).or_default() += 1;
            }
        }
    }
    let mut assigned: HashMap<(Uuid, Uuid, String), Option<String>> = HashMap::new();
    let mut unique_names = HashSet::new();
    for indices in islands.values() {
        let mut pins = vec![];
        let mut labels = vec![];
        let harness_names: BTreeSet<_> = indices
            .iter()
            .filter_map(|n| harness.names.get(&(nodes[*n].0.id, nodes[*n].1.id)))
            .cloned()
            .collect();
        if harness_names.len() > 1 {
            return Err("Conflicting named harness channels on one net".into());
        }
        for &n in indices {
            let (doc, p) = nodes[n];
            if let Some((priority, name)) = naming_label(p, options) {
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
                pins.push((
                    project::logical_designator(c).to_owned(),
                    number.clone(),
                    doc.id,
                    c.id,
                ));
            }
        }
        if pins.is_empty() {
            continue;
        }
        pins.sort_by(|a, b| {
            project::natural_order(&a.0, &b.0)
                .then_with(|| project::natural_order(&a.1, &b.1))
                .then_with(|| (a.2, a.3).cmp(&(b.2, b.3)))
        });
        pins.dedup();
        let physical_pin_count = pins
            .iter()
            .map(|p| (p.2, pin_owners[&(p.2, p.3)], p.1.to_uppercase()))
            .collect::<HashSet<_>>()
            .len();
        labels.sort_by_key(|a| naming_key(a.0, a.2, &a.1));
        let local_sheet = indices
            .iter()
            .map(|n| nodes[*n].0.id)
            .collect::<HashSet<_>>();
        let sheet_local = local_sheet.len() == 1
            && !labels
                .iter()
                .any(|l| l.0 == 0 && graph.scope != NetScope::StrictHierarchical);
        if options.review_annotation
            && labels.is_empty()
            && pins.iter().any(|p| p.0.is_empty() || p.0.contains('?'))
        {
            out.issues.push(format!("Unnamed net contains unannotated component {}; annotate before transferring this net",pins.iter().find(|p|p.0.is_empty()||p.0.contains('?')).unwrap().0));
            out.unresolved_pins
                .extend(pins.iter().map(|p| (p.2, p.3, p.1.to_uppercase())));
            continue;
        }
        let net = if !options.single_pin_nets && physical_pin_count <= 1 {
            out.omitted_single_pin_pins
                .extend(pins.iter().map(|p| (p.2, p.3, p.1.to_uppercase())));
            None
        } else if let Some(name) = harness_names.first() {
            Some(name.clone())
        } else if let Some((priority, name, doc, ..)) = labels.first() {
            if options.append_sheet_number && sheet_local {
                Some(format!(
                    "{}_{}",
                    name,
                    options.sheet_numbers.get(doc).copied().unwrap_or(1)
                ))
            } else if hierarchical
                && options.hierarchical_net_names
                && *priority != 0
                && base_counts
                    .get(&name.to_uppercase())
                    .is_some_and(|count| *count > 1)
            {
                let sheet = sheet_name(*doc);
                Some(format!("{sheet}/{name}"))
            } else {
                Some(name.clone())
            }
        } else if physical_pin_count > 1 {
            let local = format!("Net{}_{}", pins[0].0, pins[0].1);
            Some(if options.append_sheet_number && sheet_local {
                format!(
                    "{}_{}",
                    local,
                    options.sheet_numbers.get(&pins[0].2).copied().unwrap_or(1)
                )
            } else if hierarchical && options.hierarchical_net_names {
                format!("{}/{}", sheet_name(pins[0].2), local)
            } else {
                local
            })
        } else {
            None
        };
        if let Some(name) = &net {
            let duplicate = !unique_names.insert(name.to_uppercase());
            if duplicate && options.merge_duplicate_net_names {
                out.warnings.push(format!("Separate source islands named {name} export as one PCB net, following the project's native net naming"));
            }
            if duplicate && !options.merge_duplicate_net_names {
                if options.review_annotation {
                    out.issues.push(format!("Separate electrical nets share the name {name}; resolve annotation or enable unique local net names"));
                    out.unresolved_pins
                        .extend(pins.iter().map(|p| (p.2, p.3, p.1.to_uppercase())));
                    out.unresolved_pins.extend(
                        out.pins
                            .iter()
                            .filter(|p| {
                                p.net.as_ref().is_some_and(|n| n.eq_ignore_ascii_case(name))
                            })
                            .map(|p| (p.sheet, p.component, p.number.to_uppercase())),
                    );
                    out.pins
                        .retain(|p| !p.net.as_ref().is_some_and(|n| n.eq_ignore_ascii_case(name)));
                    out.names.retain(|n| !n.eq_ignore_ascii_case(name));
                    out.labels.remove(name);
                    continue;
                }
                return Err(format!(
                    "Different sheet-local nets share the name {name}; project net-scope compilation is required"
                ));
            }
            if name.contains(['[', ']', '{', '}']) {
                return Err(format!(
                    "Bus/harness net {name} requires expansion; connectivity comparison is disabled"
                ));
            }
            let aliases: Vec<_> = labels
                .iter()
                .filter(|(priority, _, _, _)| {
                    *priority == labels[0].0 && (!hierarchical || *priority == 0)
                })
                .map(|(_, s, _, _)| s.to_uppercase())
                .collect();
            if harness_names.is_empty()
                && aliases.iter().any(|s| s != &name.to_uppercase())
                && (!hierarchical || labels.first().is_some_and(|l| l.0 == 0))
            {
                return Err(format!(
                    "Conflicting labels on net {name}: {:?}; resolve before synchronization",
                    aliases.iter().take(12).collect::<Vec<_>>()
                ));
            }
            out.names.push(name.clone());
            out.labels
                .entry(name.clone())
                .or_default()
                .extend(labels.iter().map(|(_, _, d, id)| (*d, *id)));
        }
        for (designator, number, sheet, component) in pins {
            let key = (sheet, component, number.to_uppercase());
            if let Some(old) = assigned.insert(key, net.clone())
                && old != net
            {
                return Err(format!(
                    "Repeated pin {designator}-{number} belongs to different nets"
                ));
            }
            out.pins.push(Pin {
                sheet,
                component,
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
