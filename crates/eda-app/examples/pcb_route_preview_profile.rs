//! Read-only route preview CPU profile; GPU callbacks are submitted, not executed.
use eda_core::{
    pcb::{Shape, TOP},
    pcb_route::{self, RouteSession},
};
use eda_geom::Point;
use egui::{Pos2, Rect, Vec2};
use std::{path::PathBuf, time::Instant};

fn report(name: &str, mut times: Vec<f64>) {
    let first = times[0];
    times.sort_by(f64::total_cmp);
    println!(
        "{name}: median_ms={:.4} p95_ms={:.4} max_ms={:.4} first_ms={first:.4} over_16_7ms={} over_33ms={}",
        times[times.len() / 2],
        times[(times.len() - 1) * 95 / 100],
        times.last().unwrap(),
        times.iter().filter(|t| **t > 16.7).count(),
        times.iter().filter(|t| **t > 33.).count()
    );
}
fn main() -> Result<(), String> {
    let path = PathBuf::from(std::env::args_os().nth(1).ok_or("Pass INPUT.PcbDoc")?);
    let started = Instant::now();
    let doc = eda_io::load(&path)?;
    let board = doc.pcb.as_ref().ok_or("Not a PCB")?;
    println!(
        "load_ms={:.3} items={}",
        started.elapsed().as_secs_f64() * 1000.,
        board.items.len()
    );
    let pad = board
        .items
        .iter()
        .find(|i| i.net.is_some() && i.layer == TOP && matches!(i.shape, Shape::Pad { .. }))
        .ok_or("No top pad with net")?;
    let free_start = std::env::args().any(|arg| arg == "--free-start");
    let start = if free_start {
        let bounds = eda_core::pcb::Bounds::points(&board.outline_points(256));
        let candidates: Vec<_> = (1..10)
            .flat_map(|y| {
                (1..10).map(move |x| Point {
                    x: bounds.min.x + (bounds.max.x - bounds.min.x) * x / 10,
                    y: bounds.min.y + (bounds.max.y - bounds.min.y) * y / 10,
                })
            })
            .filter(|p| pcb_route::target(board, *p, TOP, 500_000).is_none())
            .collect();
        let mut probe = RouteSession::begin(
            board,
            0,
            *candidates
                .first()
                .ok_or("No free-space start in sampled board bounds")?,
            TOP,
            None,
            254_000,
            0,
        )?;
        candidates
            .into_iter()
            .find(|p| {
                probe.anchor = *p;
                let end = Point {
                    x: p.x + 1_000_000,
                    y: p.y,
                };
                probe.points(end).last() == Some(&end)
            })
            .ok_or("No sampled free-space start can escape by 1mm")?
    } else {
        pad.anchor()
    };
    // Sampling is diagnostic setup, outside the measured command startup.
    let started = Instant::now();
    let mut route = RouteSession::begin(
        board,
        0,
        start,
        TOP,
        if free_start { None } else { pad.net },
        254_000,
        0,
    )?;
    if free_start {
        assert_eq!(route.net, None);
    }
    println!("begin_ms={:.3}", started.elapsed().as_secs_f64() * 1000.);
    if std::env::args().any(|arg| arg == "--ignore") {
        route.conflict = pcb_route::Conflict::Ignore;
    }
    if std::env::args().any(|arg| arg == "--stop") {
        route.conflict = pcb_route::Conflict::Stop;
    }
    let full = std::env::args().any(|arg| arg == "--full-preview");
    let varied = std::env::args().any(|arg| arg == "--varied");
    if std::env::args().any(|arg| arg == "--verify-connectivity") {
        let started = Instant::now();
        let detached = eda_core::pcb_connectivity::Snapshot::new(board);
        assert_eq!(
            detached.connections(),
            eda_core::pcb_connectivity::connections(board)
        );
        println!(
            "verify_connectivity_ms={:.3}",
            started.elapsed().as_secs_f64() * 1000.
        );
    }
    let ctx = egui::Context::default();
    eda_render::pcb::enable_gpu(&ctx, eframe::wgpu::TextureFormat::Rgba8Unorm);
    let mut fonts = egui::FontDefinitions::default();
    for name in [
        "sch-sans",
        "sch-sans-bold",
        "sch-sans-italic",
        "sch-sans-bold-italic",
        "ui-bold",
    ] {
        fonts.families.insert(
            egui::FontFamily::Name(name.into()),
            fonts.families[&egui::FontFamily::Proportional].clone(),
        );
    }
    ctx.set_fonts(fonts);
    let mut view = eda_render::View::default();
    let centre = eda_core::pcb::Bounds::points(&board.outline_points(256)).centre();
    view.camera.centre_mm = [centre.x as f64 / 1e6, centre.y as f64 / 1e6];
    view.camera.pixels_per_mm = 15.;
    view.camera.fitted = true;
    let frame = |view: &mut eda_render::View| {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 800.))),
                focused: true,
                events: vec![egui::Event::PointerMoved(Pos2::new(500., 400.))],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    eda_render::pcb::canvas(ui, board, view);
                });
            },
        );
        std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
    };
    frame(&mut view);
    let mut target_times = vec![];
    let mut model = vec![];
    let mut publish = vec![];
    let mut paint = vec![];
    let mut total = vec![];
    let mut legs = 0;
    for n in 0..60 {
        let all = Instant::now();
        let end = Point {
            x: start.x + 3_000_000,
            y: start.y
                + if varied {
                    [0, 1_000_000, -1_000_000, 1_500_000, -1_500_000][n as usize % 5]
                } else {
                    n * 1_000
                },
        };
        let start = Instant::now();
        std::hint::black_box(if std::env::args().any(|arg| arg == "--linear-target") {
            pcb_route::target(board, end, TOP, 100_000)
        } else {
            route.source_target(board, end, 100_000)
        });
        target_times.push(start.elapsed().as_secs_f64() * 1000.);
        let start = Instant::now();
        if full {
            let preview = route.preview(end)?;
            legs += preview.items.len() - route.draft().items.len();
            model.push(start.elapsed().as_secs_f64() * 1000.);
            let start = Instant::now();
            view.pcb.set_preview(Some(preview));
            publish.push(start.elapsed().as_secs_f64() * 1000.);
        } else {
            view.pcb.set_preview(None);
            let preview = route.preview_shared(end)?;
            legs += preview.board.items.len() - route.draft().items.len();
            model.push(start.elapsed().as_secs_f64() * 1000.);
            let start = Instant::now();
            view.pcb.set_route_preview(preview);
            publish.push(start.elapsed().as_secs_f64() * 1000.);
        }
        let start = Instant::now();
        frame(&mut view);
        paint.push(start.elapsed().as_secs_f64() * 1000.);
        total.push(all.elapsed().as_secs_f64() * 1000.);
    }
    println!(
        "items={} samples=60 legs={legs} net={}",
        board.items.len(),
        board.net_name(route.net)
    );
    if free_start {
        assert!(legs > 0, "Free-space probe must preview actual copper");
    }
    report("target", target_times);
    report("model", model);
    report("publish", publish);
    report("canvas+tessellate", paint);
    report("combined", total);
    // Exercise placement and a complete History roundtrip without source writes.
    route.place(
        Point {
            x: start.x + 1_000_000,
            y: start.y,
        },
        None,
    )?;
    let mut edited = doc.clone();
    let mut history = eda_core::History::default();
    history.commit_pcb(&mut edited, route.finish(0)?)?;
    history.undo(&mut edited);
    assert_eq!(edited, doc);
    assert_eq!(eda_io::load(&path)?, doc);
    Ok(())
}
