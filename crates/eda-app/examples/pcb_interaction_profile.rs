//! Read-only comparison of complete and prepared PCB edit previews.
use eda_core::{pcb::*, pcb_tools};
use eda_geom::Point;
use std::{path::PathBuf, sync::Arc, time::Instant};

fn report(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    println!(
        "{label}: median_ms={:.4} p95_ms={:.4} max_ms={:.4}",
        samples[samples.len() / 2],
        samples[(samples.len() - 1) * 95 / 100],
        samples.last().unwrap()
    );
}
fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Pass INPUT.PcbDoc or INPUT.pcbcraft.json")?,
    );
    let doc = eda_io::load(&path)?;
    let b = doc.pcb.as_ref().ok_or("Not a PCB")?;
    let (id, pivot) = b
        .components
        .iter()
        .find(|c| !c.locked)
        .map(|c| (c.id, c.position))
        .or_else(|| {
            b.items
                .iter()
                .find(|i| {
                    !i.locked && i.component.is_none() && matches!(i.shape, Shape::Via { .. })
                })
                .map(|i| (i.id, i.anchor()))
        })
        .ok_or("No unlocked component/via")?;
    println!("items={} polygons={}", b.items.len(), b.polygon_pours.len());
    let delta = |n: i64| Point {
        x: (n + 1) * 10_000,
        y: 100_000,
    };
    for connected in [false, true] {
        let label = if connected { "connected-drag" } else { "move" };
        let mut times = vec![];
        let mut expected = None;
        for n in 0..12 {
            let started = Instant::now();
            let next = if connected {
                pcb_tools::drag_connected(b, &[id], pivot, delta(n), 0.)?
            } else {
                b.edited(Edit::Transform {
                    selection: vec![id],
                    pivot,
                    translation: delta(n),
                    angle: 0.,
                })?
            };
            times.push(started.elapsed().as_secs_f64() * 1000.);
            expected = Some(next);
        }
        report(&format!("{label}-full"), times);
        let started = Instant::now();
        let mut prepared = if connected {
            MovePreview::new_connected(b, &[id])?
        } else {
            MovePreview::new(b, &[id])?
        };
        println!(
            "{label}-prepare_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.
        );
        let mut held = None;
        let mut times = vec![];
        for n in 0..12 {
            drop(held.take());
            let started = Instant::now();
            let next = prepared.update(pivot, delta(n), 0.)?;
            times.push(started.elapsed().as_secs_f64() * 1000.);
            held = Some(next);
        }
        assert_eq!(held.as_deref(), expected.as_ref());
        report(&format!("{label}-prepared"), times);
    }
    let via = |n| {
        Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point {
                    x: pivot.x + delta(n).x,
                    y: pivot.y + delta(n).y,
                },
                diameter: 800_000,
                hole: 300_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        )
    };
    let mut times = vec![];
    for n in 0..12 {
        let started = Instant::now();
        let next = b.edited(Edit::Insert(vec![via(n)]))?;
        times.push(started.elapsed().as_secs_f64() * 1000.);
        std::hint::black_box(next);
    }
    report("placement-full", times);
    let started = Instant::now();
    let mut prepared = InsertPreview::new(b)?;
    println!(
        "placement-prepare_ms={:.3}",
        started.elapsed().as_secs_f64() * 1000.
    );
    let mut held: Option<Arc<Board>> = None;
    let mut times = vec![];
    for n in 0..12 {
        drop(held.take());
        let started = Instant::now();
        held = Some(prepared.update(vec![via(n)])?);
        times.push(started.elapsed().as_secs_f64() * 1000.);
    }
    let next = held.unwrap();
    let expected = b.edited(Edit::Insert(next.items[b.items.len()..].to_vec()))?;
    assert_eq!(*next, expected);
    report("placement-prepared", times);
    println!("geometry_matches_full_transactions=true source_not_written=true");
    Ok(())
}
