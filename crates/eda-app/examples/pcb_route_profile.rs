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
    for rule in &board.rules {
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
    let route = RouteSession::begin(board, 0, pad.anchor(), TOP, pad.net, 254_000, 0)?;
    println!(
        "items={} prepare_ms={:.2}",
        board.items.len(),
        start.elapsed().as_secs_f64() * 1000.
    );
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
    }
    assert_eq!(doc, eda_io::load(&path)?);
    Ok(())
}
