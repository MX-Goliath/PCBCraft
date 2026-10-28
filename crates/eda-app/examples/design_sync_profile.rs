//! Read-only ECO diagnostic: cargo run -p eda-app --example design_sync_profile -- project.PrjPcb
use eda_core::design_sync::{self, Direction, Footprint};
fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PrjPcb path")?;
    let project = eda_io::altium::load_project(std::path::Path::new(&path))?;
    let sheets: Vec<_> = project
        .documents
        .iter()
        .filter(|e| e.kind == "schdoc")
        .map(|e| eda_io::altium::load_schematic(&e.path))
        .collect::<Result<_, _>>()?;
    let sheet_entries: Vec<_> = project
        .documents
        .iter()
        .filter(|e| e.kind == "schdoc")
        .collect();
    let compile_sheets: Vec<_> = sheet_entries
        .iter()
        .zip(&sheets)
        .map(|(e, d)| (e.path.as_path(), d))
        .collect();
    let options = project.compile_options(&compile_sheets)?;
    let sheets: Vec<_> = sheets.iter().collect();
    let mut available = vec![];
    let mut library_paths: Vec<_> = project
        .documents
        .iter()
        .filter(|e| matches!(e.kind.as_str(), "pcblib" | "intlib"))
        .map(|e| e.path.clone())
        .collect();
    library_paths.extend(
        std::env::args().filter_map(|a| a.strip_prefix("--library=").map(std::path::PathBuf::from)),
    );
    for path in library_paths {
        let library = eda_io::altium_pcb::library::load(&path)?;
        for warning in library.warnings {
            println!("Library warning: {warning}");
        }
        available.extend(library.footprints);
    }
    if let Some(name) =
        std::env::args().find_map(|a| a.strip_prefix("--component=").map(str::to_owned))
    {
        use eda_core::{schematic::SchPrimitiveKind as K, schematic_edit::electrical_endpoint};
        for d in &sheets {
            let s = d.schematic.as_ref().unwrap();
            for c in s.components.iter().filter(|c| c.designator == name) {
                for p in s
                    .primitives
                    .iter()
                    .filter(|p| p.component == Some(c.id) && matches!(p.kind, K::Pin { .. }))
                {
                    let at = electrical_endpoint(p).unwrap();
                    println!("{} {:?}, endpoint {:?}", c.designator, p.kind, at);
                    if let Some(r) = s.records.iter().find(|r| r.index == p.source_index) {
                        println!("Source {:?}", r.fields);
                    }
                    let mut nearest: Vec<_> = s
                        .primitives
                        .iter()
                        .filter(|q| q.id != p.id && q.component.is_none())
                        .filter_map(|q| {
                            let distance = if let K::Polyline {
                                points, electrical, ..
                            } = &q.kind
                            {
                                if electrical != "Wire" {
                                    return None;
                                }
                                points
                                    .windows(2)
                                    .map(|e| {
                                        let dx = (e[1].x - e[0].x) as f64;
                                        let dy = (e[1].y - e[0].y) as f64;
                                        let t = (((at.x - e[0].x) as f64 * dx
                                            + (at.y - e[0].y) as f64 * dy)
                                            / (dx * dx + dy * dy))
                                            .clamp(0., 1.);
                                        ((at.x - e[0].x) as f64 - t * dx)
                                            .hypot((at.y - e[0].y) as f64 - t * dy)
                                    })
                                    .reduce(f64::min)?
                            } else {
                                let b = electrical_endpoint(q)?;
                                ((at.x - b.x) as f64).hypot((at.y - b.y) as f64)
                            };
                            Some((distance, q))
                        })
                        .collect();
                    nearest.sort_by(|a, b| a.0.total_cmp(&b.0));
                    for (dist, q) in nearest.into_iter().take(3) {
                        println!("Nearest {dist} nm: {:?}", q.kind);
                    }
                }
            }
        }
    }
    for entry in project.documents.iter().filter(|e| e.kind == "pcbdoc") {
        let d = eda_io::load(&entry.path)?;
        let b = d.pcb.as_ref().ok_or("PCB missing")?;
        if let Some(name) =
            std::env::args().find_map(|a| a.strip_prefix("--params=").map(str::to_owned))
        {
            for doc in &sheets {
                for c in doc
                    .schematic
                    .as_ref()
                    .unwrap()
                    .components
                    .iter()
                    .filter(|c| c.designator == name)
                {
                    println!("SCH params {:?}", c.parameters);
                }
            }
            for c in b.components.iter().filter(|c| c.designator == name) {
                println!("PCB params {:?}", c.parameters);
            }
        }
        if std::env::args().any(|a| a == "--net-audit") {
            let nets = design_sync::compile_project(&sheets, &options)?;
            let mut differences = std::collections::BTreeMap::<String, usize>::new();
            let mut matched = 0;
            let mut missing = 0;
            for pin in &nets.pins {
                if std::env::args().any(|a| a == "--pin-table") {
                    println!(
                        "PINTAB\t{}\t{}\t{}",
                        pin.designator,
                        pin.number,
                        pin.net.as_deref().unwrap_or("")
                    );
                }
                let scene = sheets
                    .iter()
                    .find(|d| d.id == pin.sheet)
                    .unwrap()
                    .schematic
                    .as_ref()
                    .unwrap();
                let c = scene
                    .components
                    .iter()
                    .find(|c| c.id == pin.component)
                    .unwrap();
                let uid = scene
                    .records
                    .iter()
                    .find(|r| r.index == c.source_index)
                    .and_then(|r| r.fields.get("UNIQUEID"))
                    .cloned()
                    .unwrap_or_else(|| c.id.to_string());
                let targets: Vec<_> = b
                    .components
                    .iter()
                    .filter(|c| {
                        c.properties.get("SOURCEUNIQUEID").is_some_and(|v| {
                            v.rsplit(['/', '\\'])
                                .next()
                                .unwrap()
                                .eq_ignore_ascii_case(&uid)
                        })
                    })
                    .collect();
                if targets.len() != 1 {
                    missing += 1;
                    continue;
                }
                for pad in b.items.iter().filter(|i|i.component==Some(targets[0].index)&&matches!(&i.shape,eda_core::pcb::Shape::Pad{name,..}if name.eq_ignore_ascii_case(&pin.number))) {
                    let actual=pad.net.map(|n|b.net_name(Some(n)));
                    if actual.map(str::to_uppercase)==pin.net.as_ref().map(|n|n.to_uppercase()) {matched+=1;} else {
                        *differences.entry(format!("{:?} -> {:?}",actual,pin.net)).or_default()+=1;
                    }
                }
            }
            println!(
                "Pin audit: {matched} matching pad memberships, {missing} unmatched source pins, {} unresolved source pins",
                nets.unresolved_pins.len()
            );
            for (name, count) in differences {
                println!("{count} pins: {name}");
            }
        }
        let mut footprints = available.clone();
        if std::env::args().any(|a| a == "--reuse-board-footprints") {
            println!(
                "Diagnostic board-footprint reuse enabled; this differs from native library lookup"
            );
            footprints.extend(
                b.components
                    .iter()
                    .filter_map(|c| Footprint::from_board(b, c.id).ok()),
            );
        }
        let direction = if std::env::args().any(|a| a == "--reverse") {
            Direction::ToSchematic
        } else {
            Direction::ToPcb
        };
        let start = std::time::Instant::now();
        let mut p = design_sync::compare_project(&sheets, &d, direction, footprints, &options)?;
        p.restrict_native_back_annotation();
        println!(
            "{}: comparison prepared {} rows in {:?}",
            d.name,
            p.changes.len(),
            start.elapsed()
        );
        let mut groups = std::collections::BTreeMap::new();
        for c in &p.changes {
            *groups.entry(c.group).or_insert(0usize) += 1;
        }
        println!("Groups: {groups:?}");
        if std::env::args().any(|a| a == "--no-validate") {
            print!("{}", p.report());
            for warning in &p.warnings {
                println!("Warning: {warning}");
            }
            continue;
        }
        let mut docs = vec![&d];
        docs.extend(&sheets);
        let prepared = p.validate(&docs)?;
        println!(
            "{}: {} changes, {} applicable, {:?}",
            d.name,
            p.changes.len(),
            prepared.rows.len(),
            start.elapsed()
        );
        for w in &p.warnings {
            println!("Warning: {w}");
        }
        print!("{}", p.report());
    }
    Ok(())
}
