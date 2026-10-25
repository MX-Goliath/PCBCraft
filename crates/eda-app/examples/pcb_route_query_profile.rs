//! Repeatable model-space routing workload, with no file writes or GUI timings.
use eda_core::{
    pcb::{Board, Item, Shape, TOP},
    pcb_route::{Conflict, RouteSession},
};
use eda_geom::Point;
use std::time::Instant;

fn main() -> Result<(), String> {
    let mut board = Board::default();
    // Remote copper is deliberately stored before the local obstacle, as on
    // boards whose file order is unrelated to the current editing viewport.
    for y in 0..200 {
        for x in 0..200 {
            board.items.push(Item::new(
                TOP,
                None,
                Shape::Track {
                    start: Point::mm(100 + x * 2, 100 + y * 2),
                    end: Point::mm(101 + x * 2, 100 + y * 2),
                    width: 200_000,
                },
            ));
        }
    }
    board.items.push(Item::new(
        TOP,
        None,
        Shape::Track {
            start: Point::mm(5, -2),
            end: Point::mm(5, 2),
            width: 400_000,
        },
    ));
    let start = Instant::now();
    let mut route = RouteSession::begin(&board, 0, Point::mm(0, 0), TOP, None, 250_000, 0)?;
    println!(
        "items={} prepare_ms={:.3}",
        board.items.len(),
        start.elapsed().as_secs_f64() * 1000.
    );
    for conflict in [Conflict::Stop, Conflict::Walkaround] {
        route.conflict = conflict;
        let mut samples = Vec::new();
        let mut legs = 0;
        for sample in 0..120 {
            let end = Point {
                x: 10_000_000,
                y: sample * 1_000 - 60_000,
            };
            let start = Instant::now();
            let points = std::hint::black_box(route.points(end));
            samples.push(start.elapsed().as_secs_f64() * 1000.);
            if conflict == Conflict::Walkaround && points.last() != Some(&end) {
                return Err("Walkaround failed to reach target".into());
            }
            if conflict == Conflict::Stop && points.last().is_none_or(|p| p.x >= 5_000_000) {
                return Err("Stop crossed obstacle".into());
            }
            legs += points.len() - 1;
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "mode={conflict:?} samples={} median_ms={:.4} p95_ms={:.4} max_ms={:.4} legs={legs}",
            samples.len(),
            samples[60],
            samples[114],
            samples[119]
        );
    }
    Ok(())
}
