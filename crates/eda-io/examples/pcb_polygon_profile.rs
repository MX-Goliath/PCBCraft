//! Read-only polygon regression/performance probe. Optional output is explicit.
use eda_core::{History, pcb::Edit, pcb_polygon};
use eda_geom::Point;
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(
        args.next()
            .ok_or("Usage: pcb_polygon_profile INPUT [OUTPUT.pcbcraft.json]")?,
    );
    let mut output = None;
    let mut move_component = false;
    for arg in args {
        if arg == "--move-component" {
            move_component = true;
        } else if output.is_none() {
            output = Some(PathBuf::from(arg));
        } else {
            return Err("Unexpected argument".into());
        }
    }
    let start = Instant::now();
    let mut doc = eda_io::load(&input)?;
    let mut history = History::default();
    let original = move_component.then(|| doc.clone());
    if move_component {
        let board = doc.pcb.as_ref().ok_or("Not a PCB document")?;
        let component = board
            .components
            .iter()
            .find(|c| !c.locked)
            .ok_or("No unlocked component")?;
        let start = Instant::now();
        let moved = board.edited(Edit::Transform {
            selection: vec![component.id],
            pivot: component.position,
            translation: Point::mm(1, 1),
            angle: 0.0,
        })?;
        history.commit_pcb(&mut doc, moved)?;
        println!(
            "component_move_ms={:.1} stale={}",
            start.elapsed().as_secs_f64() * 1000.,
            doc.pcb.as_ref().unwrap().pours_stale
        );
    }
    let board = doc.pcb.as_ref().ok_or("Not a PCB document")?;
    println!(
        "load_ms={:.1} items={} polygons={} archived_polygons={} warnings={}",
        start.elapsed().as_secs_f64() * 1000.,
        board.items.len(),
        board.polygon_pours.len(),
        board.polygons.len(),
        board.warnings.len()
    );
    for warning in board.warnings.iter().filter(|s| s.contains("Polygon")) {
        println!("{warning}");
    }
    let start = Instant::now();
    let prepared = pcb_polygon::prepare_repour(doc.id, history.revision, board)?;
    history.apply_pcb_pour(&mut doc, prepared)?;
    let poured = doc.pcb.as_ref().unwrap();
    println!(
        "repour_ms={:.1} generated={} stale={}",
        start.elapsed().as_secs_f64() * 1000.,
        poured.items.iter().filter(|i| i.polygon.is_some()).count(),
        poured.pours_stale
    );
    let start = Instant::now();
    let again = pcb_polygon::repour(poured)?;
    if poured != &again {
        return Err("Repour is not deterministic".into());
    }
    println!(
        "repeat_ms={:.1} deterministic=true",
        start.elapsed().as_secs_f64() * 1000.
    );
    if let Some(original) = original {
        history.undo(&mut doc);
        if doc != original {
            return Err("One Undo did not restore the original component and copper".into());
        }
        history.redo(&mut doc);
        if doc.pcb.as_ref() != Some(&again) {
            return Err("Redo did not restore moved component and derived copper".into());
        }
        println!("component_move_repour_undo_redo=true");
    }
    if let Some(output) = output {
        if output == input {
            return Err("Use a separate output path".into());
        }
        eda_io::save(&output, &doc)?;
        println!("Saved explicit research output; source not written");
    }
    Ok(())
}
