//! Read-only reproduction and isolation of connected track drag failures.
use eda_core::{
    pcb::*,
    pcb_route::{self, Conflict, DragOptions},
};
use eda_geom::Point;
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let path = PathBuf::from(
        args.first()
            .ok_or("Pass INPUT X_MM Y_MM DX_MM DY_MM [--isolate]")?,
    );
    let coordinate = |n: usize| -> Result<i64, String> {
        let value = args
            .get(n)
            .ok_or("Missing coordinate")?
            .to_string_lossy()
            .parse::<f64>()
            .map_err(|e| e.to_string())?;
        if !value.is_finite() || value.abs() > 100_000. {
            return Err("Invalid coordinate".into());
        }
        Ok((value * 1e6).round() as i64)
    };
    let at = Point {
        x: coordinate(1)?,
        y: coordinate(2)?,
    };
    let delta = Point {
        x: coordinate(3)?,
        y: coordinate(4)?,
    };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let doc = eda_io::load(&path)?;
    let board = doc.pcb.as_ref().ok_or("Not a PCB")?;
    let hit = pcb_route::target(board, at, TOP, 2_000).ok_or("No target at point")?;
    let item = board.item(hit.item).unwrap();
    if !matches!(item.shape, Shape::Track { .. }) {
        return Err("Target is not a track".into());
    }
    println!("selected={:?} at={at:?} delta={delta:?}", item.shape);
    let run = |b: &Board, conflict| {
        pcb_route::drag_tracks(
            b,
            &[item.id],
            at,
            delta,
            0.,
            DragOptions {
                conflict,
                ..Default::default()
            },
        )
    };
    let mut blocked = false;
    let mut expected = None;
    for conflict in [Conflict::Walkaround, Conflict::Ignore] {
        let started = Instant::now();
        match run(board, conflict) {
            Ok(next) => {
                let model_ms = started.elapsed().as_secs_f64() * 1000.;
                if conflict == Conflict::Walkaround {
                    expected = Some(next.clone());
                }
                let history_started = Instant::now();
                let mut edited = doc.clone();
                let mut history = eda_core::History::default();
                history.commit_pcb(&mut edited, next)?;
                history.undo(&mut edited);
                assert_eq!(edited, doc);
                println!(
                    "{conflict:?}: PASS model_ms={model_ms:.2} commit_undo_ms={:.2}",
                    history_started.elapsed().as_secs_f64() * 1000.
                );
            }
            Err(error) => {
                blocked |= conflict == Conflict::Walkaround;
                println!(
                    "{conflict:?}: FAIL {error}; elapsed_ms={:.2}",
                    started.elapsed().as_secs_f64() * 1000.
                );
            }
        }
    }
    let started = Instant::now();
    let mut prepared = pcb_route::TrackDragPreview::new(board, &[item.id])?;
    let first = prepared.update(at, delta, 0., DragOptions::default())?;
    println!(
        "prepared_first_ms={:.2} constrained={}",
        started.elapsed().as_secs_f64() * 1000.,
        first.constrained
    );
    drop(first);
    let mut samples = Vec::new();
    for frame in 1..=30 {
        let point = Point {
            x: delta.x * frame / 30,
            y: delta.y * frame / 30,
        };
        let started = Instant::now();
        let update = prepared.update(at, point, 0., DragOptions::default())?;
        samples.push(started.elapsed().as_secs_f64() * 1000.);
        drop(update);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "prepared_30_frames_p50_ms={:.2} p95_ms={:.2} max_ms={:.2}",
        samples[15], samples[28], samples[29]
    );
    let next = prepared.finish()?;
    if let Some(expected) = expected {
        let mut normalized = next.clone();
        assert_eq!(normalized.items.len(), expected.items.len());
        for (a, b) in normalized.items[board.items.len()..]
            .iter_mut()
            .zip(&expected.items[board.items.len()..])
        {
            a.id = b.id;
        }
        assert_eq!(normalized, expected);
        println!("prepared_complete_transaction_equivalence=PASS");
    }
    let mut edited = doc.clone();
    let mut history = eda_core::History::default();
    history.commit_pcb(&mut edited, next)?;
    history.undo(&mut edited);
    assert_eq!(edited, doc);
    println!("prepared_commit_undo=PASS");
    if blocked && args.iter().any(|a| a == "--isolate") {
        let mut candidates: Vec<_> = board
            .items
            .iter()
            .filter(|i| i.net != item.net || i.keepout || i.layer == KEEP_OUT || i.is_drill_only())
            .map(|i| i.id)
            .collect();
        let baseline: std::collections::HashSet<_> = candidates.iter().copied().collect();
        let subset = |ids: &[uuid::Uuid]| {
            let retained: std::collections::HashSet<_> = ids.iter().copied().collect();
            let mut b = board.clone();
            b.items
                .retain(|i| !baseline.contains(&i.id) || retained.contains(&i.id));
            b
        };
        if run(&subset(&[]), Conflict::Walkaround).is_ok() {
            while candidates.len() > 1 {
                let half = candidates.len() / 2;
                if run(&subset(&candidates[..half]), Conflict::Walkaround).is_err() {
                    candidates.truncate(half);
                } else if run(&subset(&candidates[half..]), Conflict::Walkaround).is_err() {
                    candidates.drain(..half);
                } else {
                    break;
                }
                println!("remaining_blocker_candidates={}", candidates.len());
            }
            for id in candidates.iter().take(16) {
                let i = board.item(*id).unwrap();
                println!(
                    "isolated kind={} layer={} net={:?} keepout={} polygon={:?} bounds={:?}",
                    i.kind(),
                    i.layer,
                    i.net,
                    i.keepout,
                    i.polygon,
                    i.bounds()
                );
                if matches!(
                    i.shape,
                    Shape::Track { .. } | Shape::Pad { .. } | Shape::Via { .. }
                ) {
                    println!("  shape={:?}", i.shape);
                }
            }
        } else {
            println!("Failure persists without foreign copper; inspect fixed anchors and rules");
        }
    }
    assert_eq!(bytes, std::fs::read(&path).map_err(|e| e.to_string())?);
    Ok(())
}
