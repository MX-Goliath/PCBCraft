//! Read-only hierarchy/endpoint/netlist probe for a native schematic project.
use eda_core::schematic::SchPrimitiveKind as K;

fn trace_power_conflict(doc: &eda_core::Document) {
    use eda_core::schematic::{SchPrimitive, TextRole};
    let s = doc.schematic.as_ref().unwrap();
    let nodes: Vec<_> = s
        .primitives
        .iter()
        .filter(|p| {
            eda_core::schematic_edit::electrical_endpoint(p).is_some()
                || matches!(&p.kind,K::Polyline{electrical,..} if electrical == "Wire")
        })
        .collect();
    let name = |p: &SchPrimitive| match &p.kind {
        K::PowerPort { text, .. } => Some((true, text.to_uppercase())),
        K::Port { name, .. } => Some((false, name.to_uppercase())),
        K::Text { text, .. } if p.text_role == Some(TextRole::NetLabel) => {
            Some((false, text.to_uppercase()))
        }
        _ => None,
    };
    let pin = |p: &SchPrimitive| {
        if let K::Pin { number, .. } = &p.kind {
            p.component
                .and_then(|id| s.components.iter().find(|c| c.id == id))
                .map(|c| (c.designator.to_uppercase(), number.to_uppercase()))
        } else {
            None
        }
    };
    let mut visited = vec![false; nodes.len()];
    for start in 0..nodes.len() {
        let K::PowerPort { text: power, .. } = &nodes[start].kind else {
            continue;
        };
        if visited[start] {
            continue;
        }
        let mut queue = std::collections::VecDeque::from([start]);
        let mut previous = vec![None; nodes.len()];
        visited[start] = true;
        while let Some(a) = queue.pop_front() {
            if let K::PowerPort { text, .. } = &nodes[a].kind
                && !text.eq_ignore_ascii_case(power)
            {
                let mut chain = vec![a];
                while let Some(p) = previous[*chain.last().unwrap()] {
                    chain.push(p);
                }
                chain.reverse();
                println!("power conflict path on {}:", doc.name);
                for n in chain {
                    println!(
                        "  record {} {:?}, pin={:?}, endpoint={:?}",
                        nodes[n].source_index,
                        name(nodes[n]),
                        pin(nodes[n]),
                        eda_core::schematic_edit::electrical_endpoint(nodes[n])
                    );
                }
                return;
            }
            let label = name(nodes[a]);
            let terminal = pin(nodes[a]);
            for b in 0..nodes.len() {
                if !visited[b]
                    && (eda_core::cross_probe::touches(nodes[a], nodes[b])
                        || label
                            .as_ref()
                            .zip(name(nodes[b]).as_ref())
                            .is_some_and(|(a, b)| a == b)
                        || terminal
                            .as_ref()
                            .zip(pin(nodes[b]).as_ref())
                            .is_some_and(|(a, b)| a == b))
                {
                    visited[b] = true;
                    previous[b] = Some(a);
                    queue.push_back(b);
                }
            }
        }
    }
}
fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PrjPcb path")?;
    let project = eda_io::altium::load_project(std::path::Path::new(&path))?;
    let mut docs = vec![];
    for entry in project.documents.iter().filter(|e| e.kind == "schdoc") {
        let d = eda_io::load(&entry.path)?;
        let s = d.schematic.as_ref().unwrap();
        let entries: Vec<_> = s
            .primitives
            .iter()
            .filter(|p| matches!(p.kind, K::SheetEntry { .. }))
            .collect();
        let connected = entries.iter().filter(|p| {
            let at = eda_core::schematic_edit::electrical_endpoint(p).unwrap();
            s.primitives.iter().any(|p| matches!(&p.kind,K::Polyline{points,electrical,..} if electrical=="Wire" && points.windows(2).any(|e|at.on_segment(e[0],e[1]))))
        }).count();
        println!(
            "{}: {} symbols, {} entries, {} connected endpoints, {} ports",
            d.name,
            s.primitives
                .iter()
                .filter(|p| matches!(p.kind, K::SheetSymbol { .. }))
                .count(),
            entries.len(),
            connected,
            s.primitives
                .iter()
                .filter(|p| matches!(p.kind, K::Port { .. }))
                .count()
        );
        for p in entries.iter().filter(|p| {
            let at=eda_core::schematic_edit::electrical_endpoint(p).unwrap();
            !s.primitives.iter().any(|p|matches!(&p.kind,K::Polyline{points,electrical,..} if electrical=="Wire" && points.windows(2).any(|e|at.on_segment(e[0],e[1]))))
        }).take(8) {
            let at=eda_core::schematic_edit::electrical_endpoint(p).unwrap();
            let nearest=s.primitives.iter().filter_map(|p|if let K::Polyline{points,electrical,..}=&p.kind { (electrical=="Wire").then_some(points)}else{None}).flatten().min_by_key(|q|(q.x-at.x).abs()+(q.y-at.y).abs());
            println!("unconnected {:?}, nearest={nearest:?}",p.kind);
        }
        if std::env::args().any(|a| a == "--local")
            && !s
                .primitives
                .iter()
                .any(|p| matches!(p.kind, K::SheetSymbol { .. }))
        {
            println!(
                "local {}: {:?}; blank component refs={}",
                d.name,
                eda_core::design_sync::compile(&[&d]).map(|n| (n.names.len(), n.pins.len())),
                s.components
                    .iter()
                    .filter(|c| c.designator.trim().is_empty())
                    .count()
            );
            let mut parent =
                eda_core::Document::new(eda_core::DocumentKind::Schematic, "ProbeParent.SchDoc");
            parent.schematic.as_mut().unwrap().primitives.push(
                eda_core::schematic_edit::primitive(
                    K::SheetSymbol {
                        a: eda_geom::Point::mm(0, 10),
                        b: eda_geom::Point::mm(10, 0),
                        designator: "Probe".into(),
                        filename: d.name.clone(),
                    },
                    None,
                ),
            );
            println!(
                "scoped child {}: {:?}",
                d.name,
                eda_core::design_sync::compile(&[&parent, &d])
                    .map(|n| (n.names.len(), n.pins.len()))
            );
            if std::env::args().any(|a| a == "--trace") {
                trace_power_conflict(&d);
            }
        }
        docs.push(d);
    }
    let refs: Vec<_> = docs.iter().collect();
    let nets = eda_core::design_sync::compile(&refs)?;
    println!(
        "compiled: {} nets, {} pin assignments",
        nets.names.len(),
        nets.pins.len()
    );
    Ok(())
}
