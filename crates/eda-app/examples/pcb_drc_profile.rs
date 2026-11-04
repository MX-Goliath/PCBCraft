//! DRC benchmark with read-only source loading and exact result snapshots.
//! Explicit probe flags can author a separate, non-overwriting comparison copy.
use eda_core::pcb_rules::{self as rules, CheckOptions};
use std::{collections::BTreeSet, path::PathBuf, time::Instant};
#[derive(serde::Deserialize)]
struct ProbePatch {
    #[serde(default)]
    components: Vec<(u32, eda_core::pcb::Fields)>,
    #[serde(default)]
    items: Vec<(eda_core::pcb::SourceRecord, eda_core::pcb::Fields)>,
    #[serde(default)]
    shapes: Vec<(eda_core::pcb::SourceRecord, eda_core::pcb::Shape)>,
    #[serde(default)]
    rules: Vec<(usize, eda_core::pcb::Fields)>,
}
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().collect();
    let path = PathBuf::from(
        args.get(1)
            .ok_or("Pass a PCB or a prepared JSON snapshot")?,
    );
    // Prepared snapshots are already imported/validated. Deserialize buffered
    // bytes directly so upgrades cannot alter identities between measurements.
    let mut document: eda_core::Document = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?
    } else {
        eda_io::load(&path)?
    };
    if let Some(pair) = args.windows(2).find(|p| p[0] == "--probe-patch") {
        let patch: ProbePatch =
            serde_json::from_slice(&std::fs::read(&pair[1]).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let board = document.pcb.as_mut().ok_or("Not a PCB")?;
        for (index, values) in patch.components {
            board
                .components
                .iter_mut()
                .find(|c| c.index == index)
                .ok_or("Probe component index not found")?
                .properties
                .extend(values);
        }
        for (source, values) in patch.items {
            board
                .items
                .iter_mut()
                .find(|i| i.source.as_ref() == Some(&source))
                .ok_or("Probe source record not found")?
                .properties
                .extend(values);
        }
        for (source, shape) in patch.shapes {
            board
                .items
                .iter_mut()
                .find(|i| i.source.as_ref() == Some(&source))
                .ok_or("Probe shape source record not found")?
                .shape = shape;
        }
        for (index, values) in patch.rules {
            board
                .rules
                .get_mut(index)
                .ok_or("Probe rule index not found")?
                .extend(values);
        }
        board.validate()?;
    }
    if let Some(pair) = args.windows(2).find(|p| p[0] == "--append-probe") {
        let probe: eda_core::Document =
            serde_json::from_slice(&std::fs::read(&pair[1]).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let mut probe = probe.pcb.ok_or("Probe is not a PCB")?;
        let board = document.pcb.as_mut().ok_or("Not a PCB")?;
        let offset = board.nets.iter().map(|n| n.index).max().unwrap_or(0) + 1;
        for net in &mut probe.nets {
            net.index += offset;
            net.name = format!("DRC_PROBE_{}", net.name);
        }
        for item in &mut probe.items {
            if item.component.is_some() || item.polygon.is_some() {
                return Err("Probe must contain independent primitives".into());
            }
            item.net = item.net.map(|n| n + offset);
        }
        board.nets.extend(probe.nets);
        board.items.extend(probe.items);
    }
    if args.iter().any(|a| a == "--probe-any-net") {
        let board = document.pcb.as_mut().ok_or("Not a PCB")?;
        let rule = board
            .rules
            .iter_mut()
            .find(|r| {
                rules::text(r, "RULEKIND") == "Clearance"
                    && rules::text(r, "SCOPE1EXPRESSION") == "All"
                    && rules::text(r, "SCOPE2EXPRESSION") == "All"
            })
            .ok_or("No general clearance rule in comparison source")?;
        rule.insert("NETSCOPE".into(), "AnyNet".into());
    }
    // Author independent comparison fixtures; never overwrite an existing PCB.
    if let Some(pair) = args.windows(2).find(|p| p[0] == "--export-native-copy") {
        let output = PathBuf::from(&pair[1]);
        if output.exists() {
            return Err("Comparison fixture destination already exists".into());
        }
        return eda_io::save(&output, &document);
    }
    if let Some(pair) = args.windows(2).find(|p| p[0] == "--snapshot") {
        let board = document.pcb.as_mut().ok_or("Not a PCB")?;
        // DRC does not inspect 3D meshes or archived source bytes. Keep all
        // model-space geometry, identities, rules, classes and query metadata.
        board.models.clear();
        board.source = None;
        std::fs::write(
            &pair[1],
            serde_json::to_vec(&document).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!("Prepared DRC snapshot; native source not written");
        return Ok(());
    }
    let board = document.pcb.as_ref().ok_or("Not a PCB")?;
    let options = CheckOptions {
        excluded: if args.iter().any(|a| a == "--online") {
            BTreeSet::from(["UnRoutedNet".into()])
        } else {
            BTreeSet::new()
        },
        limit: args
            .windows(2)
            .find(|p| p[0] == "--limit")
            .map(|p| p[1].to_string_lossy().parse())
            .transpose()
            .map_err(|e| format!("Invalid limit: {e}"))?
            .unwrap_or(5000),
    };
    let start = Instant::now();
    let (report, phases) = rules::check_profiled(board, &options);
    let elapsed = start.elapsed().as_secs_f64() * 1000.;
    let output = serde_json::json!({
        "items":board.items.len(),"rules":board.rules.len(),"elapsed_ms":elapsed,
        "phases_ms":phases.into_iter().map(|(n,d)|(n,d.as_secs_f64()*1000.)).collect::<std::collections::BTreeMap<_,_>>(),
        "truncated":report.truncated,"diagnostics":report.diagnostics,
        "violations":report.violations.iter().map(|v|serde_json::json!({
            "rule":v.rule,"kind":v.kind,"message":v.message,"items":v.items,
            "position":[v.position.x,v.position.y],"layer":v.layer,
        })).collect::<Vec<_>>(),
    });
    if let Some(pair) = args.windows(2).find(|p| p[0] == "--output") {
        std::fs::write(
            &pair[1],
            serde_json::to_vec_pretty(&output).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    println!(
        "{}",
        serde_json::json!({"elapsed_ms":elapsed,"phases_ms":output["phases_ms"],"violations":report.violations.len(),"truncated":report.truncated,"diagnostics":report.diagnostics.len()})
    );
    Ok(())
}
