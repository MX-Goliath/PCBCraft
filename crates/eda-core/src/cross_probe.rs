//! Read-only schematic/PCB correspondence. Never modifies documents or history.
use crate::{
    Document, pcb,
    schematic::{SchPrimitive, SchPrimitiveKind as K, Schematic, TextRole},
};
use eda_geom::Point;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ComponentKey {
    pub designator: String,
    pub unique_id: Option<String>,
}
#[derive(Clone, Debug)]
pub struct PinKey {
    pub component: ComponentKey,
    pub number: String,
}
#[derive(Clone, Debug)]
pub enum Probe {
    Component(ComponentKey),
    Pin(PinKey),
    Net {
        names: Vec<String>,
        pins: Vec<PinKey>,
    },
}
#[derive(Clone, Debug)]
pub struct Target {
    pub ids: Vec<Uuid>,
    pub centre: Point,
    pub net: Option<u32>,
    pub label: String,
}
fn field(fields: &pcb::Fields, name: &str) -> Option<String> {
    fields
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}
fn sch_key(s: &Schematic, id: Uuid) -> Option<ComponentKey> {
    let c = s.components.iter().find(|c| c.id == id)?;
    let unique_id = s
        .records
        .iter()
        .find(|r| r.index == c.source_index && r.record_type == 1)
        .and_then(|r| field(&r.fields, "UNIQUEID"));
    Some(ComponentKey {
        designator: c.designator.clone(),
        unique_id,
    })
}
fn pcb_key(c: &pcb::Component) -> ComponentKey {
    ComponentKey {
        designator: c.designator.clone(),
        unique_id: field(&c.properties, "SOURCEUNIQUEID"),
    }
}
fn matches(a: &ComponentKey, b: &ComponentKey) -> bool {
    match (&a.unique_id, &b.unique_id) {
        (Some(a), Some(b)) => {
            a.eq_ignore_ascii_case(b)
                || a.rsplit(['/', '\\'])
                    .next()
                    .unwrap()
                    .eq_ignore_ascii_case(b.rsplit(['/', '\\']).next().unwrap())
        }
        _ => {
            !a.designator.trim().is_empty()
                && !a.designator.contains('?')
                && a.designator.eq_ignore_ascii_case(&b.designator)
        }
    }
}
fn pin(s: &Schematic, p: &SchPrimitive) -> Option<PinKey> {
    if let K::Pin { number, .. } = &p.kind {
        Some(PinKey {
            component: sch_key(s, p.component?)?,
            number: number.clone(),
        })
    } else {
        None
    }
}
fn pad(b: &pcb::Board, p: &pcb::Item) -> Option<PinKey> {
    if let pcb::Shape::Pad { name, .. } = &p.shape {
        Some(PinKey {
            component: pcb_key(b.components.iter().find(|c| Some(c.index) == p.component)?),
            number: name.clone(),
        })
    } else {
        None
    }
}
fn pin_matches(a: &PinKey, b: &PinKey) -> bool {
    matches(&a.component, &b.component) && a.number.eq_ignore_ascii_case(&b.number)
}
fn name(p: &SchPrimitive) -> Option<&str> {
    match &p.kind {
        K::Text { text, .. } if p.text_role == Some(TextRole::NetLabel) => Some(text),
        K::PowerPort { text, .. } => Some(text),
        K::Port { name, .. } => Some(name),
        _ => None,
    }
    .filter(|s| !s.trim().is_empty())
    .map(String::as_str)
}
fn node(p: &SchPrimitive) -> Option<Point> {
    crate::schematic_edit::electrical_endpoint(p)
}
fn wire(p: &SchPrimitive) -> Option<&[Point]> {
    match &p.kind {
        K::Polyline {
            points, electrical, ..
        } if electrical == "Wire" && p.component.is_none() => Some(points),
        _ => None,
    }
}
pub(crate) fn touches(a: &SchPrimitive, b: &SchPrimitive) -> bool {
    match (wire(a), wire(b)) {
        (Some(a), Some(b)) => {
            a.first()
                .into_iter()
                .chain(a.last())
                .any(|p| b.windows(2).any(|e| p.on_segment(e[0], e[1])))
                || b.first()
                    .into_iter()
                    .chain(b.last())
                    .any(|p| a.windows(2).any(|e| p.on_segment(e[0], e[1])))
        }
        (Some(w), None) => node(b).is_some_and(|p| w.windows(2).any(|e| p.on_segment(e[0], e[1]))),
        (None, Some(w)) => node(a).is_some_and(|p| w.windows(2).any(|e| p.on_segment(e[0], e[1]))),
        _ => node(a).is_some() && node(a) == node(b),
    }
}
/// Local electrical topology: T junctions connect; bare interior crossings do not.
/// Named labels/ports on the same sheet connect disconnected wire islands.
fn connected(s: &Schematic, seeds: &[Uuid]) -> Vec<Uuid> {
    let nodes: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| wire(p).is_some() || node(p).is_some())
        .collect();
    let mut found = seeds.to_vec();
    let mut seen: std::collections::HashSet<_> = seeds.iter().copied().collect();
    let mut cursor = 0;
    while cursor < found.len() {
        let Some(a) = s.primitive(found[cursor]) else {
            cursor += 1;
            continue;
        };
        for b in &nodes {
            if !seen.contains(&b.id)
                && (touches(a, b)
                    || name(a)
                        .zip(name(b))
                        .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b)))
            {
                seen.insert(b.id);
                found.push(b.id);
            }
        }
        cursor += 1;
    }
    found
}
pub fn source(doc: &Document, id: Uuid) -> Result<Probe, String> {
    if let Some(b) = &doc.pcb {
        if let Some(c) = b.components.iter().find(|c| c.id == id) {
            return Ok(Probe::Component(pcb_key(c)));
        }
        let p = b.item(id).ok_or("Object no longer exists")?;
        if let Some(key) = pad(b, p) {
            return Ok(Probe::Pin(key));
        }
        if let Some(c) = b.components.iter().find(|c| Some(c.index) == p.component) {
            return Ok(Probe::Component(pcb_key(c)));
        }
        let net = p.net.ok_or("Object has no electrical net")?;
        return Ok(Probe::Net {
            names: b
                .nets
                .iter()
                .filter(|n| n.index == net)
                .map(|n| n.name.clone())
                .collect(),
            pins: b
                .items
                .iter()
                .filter(|p| p.net == Some(net))
                .filter_map(|p| pad(b, p))
                .collect(),
        });
    }
    let s = doc.schematic.as_ref().ok_or("No schematic scene")?;
    if let Some(key) = sch_key(s, id) {
        return Ok(Probe::Component(key));
    }
    let p = s.primitive(id).ok_or("Object no longer exists")?;
    if let Some(key) = pin(s, p) {
        return Ok(Probe::Pin(key));
    }
    if let Some(key) = p.component.and_then(|id| sch_key(s, id)) {
        return Ok(Probe::Component(key));
    }
    if wire(p).is_none() && node(p).is_none() {
        return Err("Choose a component, pin, wire or net label".into());
    }
    let ids = connected(s, &[id]);
    Ok(Probe::Net {
        names: s
            .primitives
            .iter()
            .filter(|p| ids.contains(&p.id))
            .filter_map(name)
            .map(str::to_owned)
            .collect(),
        pins: s
            .primitives
            .iter()
            .filter(|p| ids.contains(&p.id))
            .filter_map(|p| pin(s, p))
            .collect(),
    })
}
pub fn resolve(probe: &Probe, doc: &Document) -> Result<Option<Target>, String> {
    let (ids, net, centre) = if let Some(b) = &doc.pcb {
        let (ids, net): (Vec<Uuid>, Option<u32>) = match probe {
            Probe::Component(key) => {
                let cs: Vec<_> = b
                    .components
                    .iter()
                    .filter(|c| matches(key, &pcb_key(c)))
                    .collect();
                if cs.len() > 1 {
                    return Err("Ambiguous component link on PCB".into());
                }
                (cs.iter().map(|c| c.id).collect(), None)
            }
            Probe::Pin(key) => (
                b.items
                    .iter()
                    .filter(|p| pad(b, p).is_some_and(|p| pin_matches(key, &p)))
                    .map(|p| p.id)
                    .collect(),
                None,
            ),
            Probe::Net { names, pins } => {
                let mut nets: Vec<_> = b
                    .items
                    .iter()
                    .filter(|p| pad(b, p).is_some_and(|p| pins.iter().any(|k| pin_matches(k, &p))))
                    .filter_map(|p| p.net)
                    .collect();
                if nets.is_empty() {
                    nets.extend(
                        b.nets
                            .iter()
                            .filter(|n| names.iter().any(|v| v.eq_ignore_ascii_case(&n.name)))
                            .map(|n| n.index),
                    );
                }
                nets.sort_unstable();
                nets.dedup();
                if nets.len() > 1 {
                    return Err(
                        "Schematic net maps to multiple PCB nets; update connectivity first".into(),
                    );
                }
                let net = nets.first().copied();
                (
                    b.items
                        .iter()
                        .filter(|p| net.is_some() && p.net == net)
                        .map(|p| p.id)
                        .collect(),
                    net,
                )
            }
        };
        let bounds = b.selected_bounds(&ids);
        let centre = bounds.map(|b| Point {
            x: b.min.x + (b.max.x - b.min.x) / 2,
            y: b.min.y + (b.max.y - b.min.y) / 2,
        });
        (ids, net, centre)
    } else if let Some(s) = &doc.schematic {
        let ids: Vec<_> = match probe {
            Probe::Component(key) => {
                let components: Vec<_> = s
                    .components
                    .iter()
                    .filter(|c| sch_key(s, c.id).is_some_and(|k| matches(key, &k)))
                    .collect();
                let parts: std::collections::HashSet<_> =
                    components.iter().map(|c| c.part).collect();
                if parts.len() != components.len() {
                    return Err("Ambiguous component link on schematic".into());
                }
                components.iter().map(|c| c.id).collect()
            }
            Probe::Pin(key) => s
                .primitives
                .iter()
                .filter(|p| pin(s, p).is_some_and(|p| pin_matches(key, &p)))
                .map(|p| p.id)
                .collect(),
            Probe::Net { names, pins } => {
                let seeds: Vec<_> = s
                    .primitives
                    .iter()
                    .filter(|p| {
                        pin(s, p).is_some_and(|p| pins.iter().any(|k| pin_matches(k, &p)))
                            || (pins.is_empty()
                                && name(p).is_some_and(|n| {
                                    names.iter().any(|v| v.eq_ignore_ascii_case(n))
                                }))
                    })
                    .map(|p| p.id)
                    .collect();
                connected(s, &seeds)
            }
        };
        let points: Vec<_> = ids
            .iter()
            .filter_map(|id| crate::schematic_edit::anchor(s, *id))
            .collect();
        let centre = points.first().copied();
        (ids, None, centre)
    } else {
        return Ok(None);
    };
    Ok(centre.filter(|_| !ids.is_empty()).map(|centre| Target {
        ids,
        net,
        centre,
        label: String::new(),
    }))
}

/// Keep repeated physical instances distinct so the user can choose the channel.
pub fn destinations(probe: &Probe, doc: &Document) -> Result<Vec<Target>, String> {
    if let Some(board) = &doc.pcb {
        let key = match probe {
            Probe::Component(k) => Some(k),
            Probe::Pin(k) => Some(&k.component),
            _ => None,
        };
        if let Some(key) = key {
            let mut targets = Vec::new();
            for c in board
                .components
                .iter()
                .filter(|c| matches(key, &pcb_key(c)))
            {
                let ids=match probe {
                    Probe::Pin(k)=>board.items.iter().filter(|p|p.component==Some(c.index) && matches!(&p.shape,pcb::Shape::Pad{name,..} if name.eq_ignore_ascii_case(&k.number))).map(|p|p.id).collect(),
                    _=>vec![c.id],
                };
                if let Some(b) = board.selected_bounds(&ids) {
                    targets.push(Target {
                        ids,
                        net: None,
                        centre: Point {
                            x: b.min.x + (b.max.x - b.min.x) / 2,
                            y: b.min.y + (b.max.y - b.min.y) / 2,
                        },
                        label: format!(
                            "{} · {}",
                            c.designator,
                            field(&c.properties, "SOURCEHIERARCHICALPATH").unwrap_or_default()
                        ),
                    });
                }
            }
            return Ok(targets);
        }
    }
    Ok(resolve(probe, doc)?.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn primitive(kind: K) -> SchPrimitive {
        SchPrimitive {
            id: Uuid::new_v4(),
            source_index: u32::MAX,
            component: None,
            color: 0,
            fill: None,
            width_nm: 0,
            line_style: 0,
            start_shape: 0,
            end_shape: 0,
            line_shape_size: 0,
            presentation: Default::default(),
            text_role: None,
            kind,
        }
    }
    fn line(points: Vec<Point>) -> SchPrimitive {
        primitive(K::Polyline {
            points,
            closed: false,
            electrical: "Wire".into(),
        })
    }
    #[test]
    fn crossings_are_not_connections_but_junctions_and_t_branches_are() {
        let mut s = Schematic::blank();
        let a = line(vec![Point::mm(0, 5), Point::mm(10, 5)]);
        let b = line(vec![Point::mm(5, 0), Point::mm(5, 10)]);
        let seed = a.id;
        s.primitives = vec![a, b];
        assert_eq!(connected(&s, &[seed]), vec![seed]);
        let branch = line(vec![Point::mm(3, 5), Point::mm(3, 8)]);
        let bid = branch.id;
        s.primitives.push(branch);
        assert!(connected(&s, &[seed]).contains(&bid));
        let j = primitive(K::Junction {
            position: Point::mm(5, 5),
            radius_nm: 100_000,
        });
        s.primitives.push(j);
        assert_eq!(connected(&s, &[seed]).len(), 4);
    }
    #[test]
    fn named_disconnected_islands_join_but_plain_text_does_not() {
        let mut s = Schematic::blank();
        let a = line(vec![Point::mm(0, 0), Point::mm(10, 0)]);
        let b = line(vec![Point::mm(20, 0), Point::mm(30, 0)]);
        let seed = a.id;
        let bid = b.id;
        let mut label = primitive(K::Text {
            position: Point::mm(0, 0),
            text: "SIGNAL".into(),
            font: 0,
            orientation: 0,
            justification: 0,
        });
        label.text_role = Some(TextRole::NetLabel);
        let mut other = label.clone();
        other.id = Uuid::new_v4();
        if let K::Text { position, .. } = &mut other.kind {
            *position = Point::mm(20, 0);
        }
        s.primitives = vec![a, b, label, other];
        assert!(connected(&s, &[seed]).contains(&bid));
        s.primitives[3].text_role = None;
        assert!(!connected(&s, &[seed]).contains(&bid));
    }
}
