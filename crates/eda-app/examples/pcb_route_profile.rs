//! Read-only native routing probe. Does not save or modify the input file.
use eda_core::pcb::{Shape, TOP};
use eda_core::pcb_route::RouteSession;
use eda_geom::Point;
use std::{path::PathBuf, time::Instant};
fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: pcb_route_profile INPUT.PcbDoc")?,
    );
    let doc = eda_io::load(&path)?;
    let board = doc.pcb.as_ref().ok_or("Not a PCB")?;
    let show_rules = std::env::args_os().any(|arg| arg == "--rules");
    for rule in board.rules.iter().filter(|_| show_rules) {
        if rule
            .get("RULEKIND")
            .is_some_and(|k| ["Width", "Clearance", "RoutingVias"].contains(&k.as_str()))
        {
            println!("rule={rule:?}");
        }
    }
    let pad = board
        .items
        .iter()
        .find(|i| i.net.is_some() && i.layer == TOP && matches!(i.shape, Shape::Pad { .. }))
        .ok_or("No pad")?;
    let start = Instant::now();
    let mut route = RouteSession::begin(board, 0, pad.anchor(), TOP, pad.net, 254_000, 0)?;
    println!(
        "items={} net={} start={:?} width_nm={} prepare_ms={:.2}",
        board.items.len(),
        board.net_name(route.net),
        route.anchor,
        route.width,
        start.elapsed().as_secs_f64() * 1000.
    );
    let mut best = route.anchor;
    for (x, y) in [(1, 1), (3, 0), (-2, 3)] {
        let end = Point {
            x: pad.anchor().x + x * 1_000_000,
            y: pad.anchor().y + y * 1_000_000,
        };
        let start = Instant::now();
        let points = route.points(end);
        println!(
            "offset=({x},{y}) legs={} reached={} solve_ms={:.2}",
            points.len() - 1,
            points.last() == Some(&end),
            start.elapsed().as_secs_f64() * 1000.
        );
        if let Some(point) = points.last()
            && eda_core::pcb::distance_sq(*point, route.anchor)
                > eda_core::pcb::distance_sq(best, route.anchor)
        {
            best = *point;
        }
    }
    if best == route.anchor {
        return Err("All sampled route directions are blocked at the pad".into());
    }
    let start = Instant::now();
    let preview = route.preview(best)?;
    route.place(best, None)?;
    assert!(route.has_segments());
    let finished = route.finish(0)?;
    // Preview and placement allocate different UUIDs, but must draw identical copper.
    assert_eq!(preview.items.len(), finished.items.len());
    assert_eq!(
        preview.items.last().map(|i| &i.shape),
        finished.items.last().map(|i| &i.shape)
    );
    let mut edited = doc.clone();
    let mut history = eda_core::History::default();
    history.commit_pcb(&mut edited, finished)?;
    assert_eq!(history.revision, 1);
    history.undo(&mut edited);
    assert_eq!(edited, doc);
    println!(
        "preview_place_finish_undo_ms={:.2} end={best:?}; source not written",
        start.elapsed().as_secs_f64() * 1000.
    );
    assert_eq!(doc, eda_io::load(&path)?);
    Ok(())
}
