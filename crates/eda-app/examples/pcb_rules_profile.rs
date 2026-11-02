//! Read-only rule inventory for compatibility research.
fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a PCB document")?;
    let document = eda_io::load(std::path::Path::new(&path))?;
    let layer_width_only = std::env::args().any(|a| a == "--roundtrip-layer-width");
    if layer_width_only || std::env::args().any(|a| a == "--roundtrip") {
        let mut edited = document.clone();
        let board = edited.pcb.as_mut().ok_or("Not a PCB")?;
        let mut rules = board.rules.clone();
        for rule in &mut rules {
            let kind = eda_core::pcb_rules::text(rule, "RULEKIND");
            if !layer_width_only
                && matches!(
                    kind,
                    "Width"
                        | "PolygonConnect"
                        | "MinimumSolderMaskSliver"
                        | "SilkToSolderMaskClearance"
                )
            {
                rule.insert(
                    "COMMENT".into(),
                    "PCBCraft in-memory roundtrip check".into(),
                );
            }
        }
        if !layer_width_only
            && let Some(rule) = rules
                .iter_mut()
                .find(|r| eda_core::pcb_rules::text(r, "RULEKIND") == "HoleSize")
        {
            rule.insert("ABSOLUTEVALUES".into(), "FALSE".into());
            rule.insert("MINPERCENT".into(), "25.5".into());
            rule.insert("MAXPERCENT".into(), "82.25".into());
        }
        if !layer_width_only
            && let Some(rule) = rules
                .iter_mut()
                .find(|r| eda_core::pcb_rules::text(r, "RULEKIND") == "LayerPairs")
        {
            rule.insert("ENFORCE".into(), "FALSE".into());
        }
        // Exercise the field edited by the per-layer Width grid, in addition
        // to metadata. The source PCB is never written by this probe.
        if let Some(rule) = rules.iter_mut().find(|r| {
            eda_core::pcb_rules::text(r, "RULEKIND") == "Width"
                && eda_core::pcb_rules::text(r, "SCOPE1EXPRESSION").eq_ignore_ascii_case("All")
        }) {
            let (minimum, _) = eda_core::pcb_rules::width_limits(rule, eda_core::pcb::TOP)?;
            rule.insert(
                "TOPLAYER_PREFWIDTH".into(),
                format!("{}mm", minimum as f64 / 1e6),
            );
        }
        *board = board.with_rules(rules)?;
        let bytes = eda_io::altium_pcb::writer::serialize(
            &edited,
            eda_io::altium_pcb::writer::Format::Preserve,
        )?;
        let parsed = eda_io::altium_pcb::import_pcb(&bytes, "roundtrip.PcbDoc")?;
        if parsed.pcb.as_ref().unwrap().rules != edited.pcb.as_ref().unwrap().rules {
            return Err("Rules differ after roundtrip".into());
        }
        println!(
            "Native rules roundtrip verified in memory: {} bytes; source not written",
            bytes.len()
        );
        return Ok(());
    }
    let board = document.pcb.ok_or("Not a PCB")?;
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|a| a == "--pairs") {
        println!(
            "{}",
            serde_json::json!({
                "pairs":board.differential_pairs,
            "classes":board.classes.iter().filter(|c| eda_core::pcb_rules::text(c,"KIND") == "6").collect::<Vec<_>>(),
            })
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--audit-queries") {
        let mut scopes =
            std::collections::BTreeMap::<String, std::collections::BTreeSet<String>>::new();
        for rule in board
            .rules
            .iter()
            .filter(|r| eda_core::pcb_rules::enabled(r))
        {
            for key in ["SCOPE1EXPRESSION", "SCOPE2EXPRESSION"] {
                let expression = eda_core::pcb_rules::text(rule, key);
                if !expression.trim().is_empty() {
                    scopes.entry(expression.into()).or_default().insert(format!(
                        "{}: {} ({key})",
                        eda_core::pcb_rules::text(rule, "RULEKIND"),
                        eda_core::pcb_rules::text(rule, "NAME"),
                    ));
                }
            }
        }
        let probe = eda_core::pcb_rules::probe(
            eda_core::pcb::TOP,
            None,
            eda_geom::Point::default(),
            200_000,
        );
        let mut unsupported = Vec::new();
        for (expression, rules) in &scopes {
            if let Some(error) = std::iter::once(&probe)
                .chain(board.items.iter())
                .find_map(|item| eda_core::pcb_rules::query(&board, item, expression).err())
            {
                unsupported.push(serde_json::json!({
                    "expression": expression, "error": error, "rules": rules,
                }));
            }
        }
        println!(
            "{}",
            serde_json::json!({
                "items":board.items.len(), "expressions":scopes.len(),
                "unsupported":unsupported, "source_written":false,
            })
        );
        return Ok(());
    }
    if args.iter().any(|a| a == "--stack") {
        for (key, value) in &board.properties {
            if key.contains("VIA") || key.contains("DRILL") || key.contains("LAYERPAIR") {
                println!("{key}={value}");
            }
        }
        println!("typed_stack={:?}", board.layer_stack);
        return Ok(());
    }
    if args.iter().any(|a| a == "--arcs") {
        for item in &board.items {
            if matches!(item.shape, eda_core::pcb::Shape::Arc { .. }) {
                println!("{} layer={} {:?}", item.id, item.layer, item.shape);
            }
        }
        return Ok(());
    }
    if let Some(pair) = args.windows(2).find(|pair| pair[0] == "--query") {
        let mut matches = std::collections::BTreeMap::<String, usize>::new();
        let mut owners = std::collections::BTreeSet::new();
        for item in &board.items {
            if eda_core::pcb_rules::query(&board, item, &pair[1])? {
                *matches.entry(item.kind().into()).or_default() += 1;
                if let Some(owner) = item
                    .component
                    .and_then(|index| board.components.iter().find(|c| c.index == index))
                {
                    owners.insert(owner.designator.clone());
                }
            }
        }
        println!("matches={matches:?} component_owners={owners:?}; source not written");
        return Ok(());
    }
    if std::env::args().any(|a| a == "--repour") {
        let start = std::time::Instant::now();
        let poured = eda_core::pcb_polygon::repour(&board)?;
        println!(
            "Repour in memory: {} polygons, {} generated objects, elapsed={:?}; source not written",
            poured.polygon_pours.len(),
            poured.items.iter().filter(|i| i.polygon.is_some()).count(),
            start.elapsed()
        );
        return Ok(());
    }
    if std::env::args().any(|a| a == "--check") {
        let start = std::time::Instant::now();
        let report = eda_core::pcb_rules::check(&board);
        println!(
            "{} items, {} rules, {} violations, truncated={}, elapsed={:?}",
            board.items.len(),
            board.rules.len(),
            report.violations.len(),
            report.truncated,
            start.elapsed()
        );
        let mut counts = std::collections::BTreeMap::new();
        for v in report.violations {
            *counts.entry(v.kind).or_insert(0) += 1;
        }
        println!("{counts:?}");
        for d in report.diagnostics {
            println!("Unchecked: {d}");
        }
        return Ok(());
    }
    eprintln!(
        "Classes: {}",
        serde_json::to_string_pretty(&board.classes).map_err(|e| e.to_string())?
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&board.rules).map_err(|e| e.to_string())?
    );
    Ok(())
}
