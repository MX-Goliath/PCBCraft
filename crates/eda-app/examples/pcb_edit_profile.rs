//! Read-only measurement of a PCB move/repour/history transaction.
use eda_core::{
    History,
    pcb::{Edit, Shape},
    pcb_polygon,
};
use eda_geom::Point;
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: pcb_edit_profile INPUT.PcbDoc")?,
    );
    let mut doc = eda_io::load(&path)?;
    let original = doc.clone();
    let board = doc.pcb.as_ref().ok_or("Not a PCB document")?;
    let item = board
        .items
        .iter()
        .find(|i| !i.locked && matches!(i.shape, Shape::Via { .. }))
        .ok_or("No unlocked via")?;
    println!(
        "items={} pours={}",
        board.items.len(),
        board.polygon_pours.len()
    );
    let start = Instant::now();
    let moved = board.edited(Edit::Transform {
        selection: vec![item.id],
        pivot: item.anchor(),
        translation: Point::mm(1, 1),
        angle: 0.,
    })?;
    println!(
        "move_and_validate_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let start = Instant::now();
    let poured = pcb_polygon::repour(&moved)?;
    println!(
        "repour_ms={:.2} result_items={}",
        start.elapsed().as_secs_f64() * 1000.,
        poured.items.len()
    );
    let start = Instant::now();
    let mut next = doc.clone();
    next.pcb = Some(poured);
    println!(
        "document_clone_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let start = Instant::now();
    History::default().commit(&mut doc, next)?;
    println!("history_ms={:.2}", start.elapsed().as_secs_f64() * 1000.);
    let expected = doc.pcb.as_ref().unwrap().clone();
    let mut doc = original;
    let board = doc.pcb.as_ref().unwrap();
    let item = board
        .items
        .iter()
        .find(|i| !i.locked && matches!(i.shape, Shape::Via { .. }))
        .unwrap();
    let preview = board.edited(Edit::Transform {
        selection: vec![item.id],
        pivot: item.anchor(),
        translation: Point::mm(1, 1),
        angle: 0.,
    })?;
    let mut history = History::default();
    let start = Instant::now();
    history.commit_pcb(&mut doc, preview)?;
    println!(
        "release_commit_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let start = Instant::now();
    let snapshot = doc.pcb.as_ref().unwrap().clone();
    println!(
        "background_snapshot_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let start = Instant::now();
    let result = pcb_polygon::prepare_repour(doc.id, history.revision, &snapshot)?;
    println!(
        "background_repour_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let start = Instant::now();
    history.apply_pcb_pour(&mut doc, result)?;
    println!(
        "apply_result_ms={:.2} undo_steps={}",
        start.elapsed().as_secs_f64() * 1000.,
        history.retained_steps()
    );
    if doc.pcb.as_ref() != Some(&expected) {
        return Err("Background result differs from synchronous pour".into());
    }
    history.undo(&mut doc);
    history.redo(&mut doc);
    if doc.pcb.as_ref() != Some(&expected) {
        return Err("Move was not preserved through one Undo/Redo".into());
    }
    Ok(())
}
