//! Read-only CPU frame probe. Timings exclude the window system and GPU upload.
use eda_geom::Point;
use egui::{Pos2, Rect, Vec2};
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(args.next().ok_or(
        "Usage: pcb_frame_profile INPUT.PcbDoc [--load-only] [--3d] [--gpu] [--zoom PX_PER_MM]",
    )?);
    let args: Vec<_> = args.collect();
    let three_d = args.iter().any(|a| a == "--3d");
    let zoom = args
        .windows(2)
        .find(|pair| pair[0] == "--zoom")
        .map(|pair| pair[1].to_string_lossy().parse::<f64>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(15.);
    if !zoom.is_finite() || !(0.001..=100_000.).contains(&zoom) {
        return Err("--zoom must be finite and between 0.001 and 100000 px/mm".into());
    }
    let loading = Instant::now();
    let doc = eda_io::load(&path)?;
    let board = doc.pcb.as_ref().ok_or("Not a PCB document")?;
    println!("load_ms={:.2}", loading.elapsed().as_secs_f64() * 1000.);
    if args.iter().any(|a| a == "--load-only") {
        println!(
            "items={} components={} layers={} models={}",
            board.items.len(),
            board.components.len(),
            board.layers.len(),
            board.models.len()
        );
        for warning in &board.warnings {
            println!("{warning}");
        }
        return Ok(());
    }
    let saved = doc.clone();
    let compare = Instant::now();
    for _ in 0..20 {
        std::hint::black_box(std::hint::black_box(&doc) == std::hint::black_box(&saved));
    }
    println!(
        "saved_compare_ms={:.2}",
        compare.elapsed().as_secs_f64() * 1000. / 20.
    );
    for warning in board.warnings.iter().filter(|w| w.starts_with("3D")) {
        println!("{warning}");
    }
    let ctx = egui::Context::default();
    if args.iter().any(|a| a == "--gpu") {
        eda_render::pcb::enable_gpu(&ctx, eframe::wgpu::TextureFormat::Rgba8Unorm);
        println!("GPU callbacks: CPU submission timings only; no GPU execution in this probe");
    }
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
    let bounds = eda_core::pcb::Bounds::points(&board.outline_points(256));
    let centre = Point {
        x: (bounds.min.x + bounds.max.x) / 2,
        y: (bounds.min.y + bounds.max.y) / 2,
    };
    let mut view = eda_render::View::default();
    view.camera.centre_mm = [centre.x as f64 / 1e6, centre.y as f64 / 1e6];
    view.camera.pixels_per_mm = zoom;
    view.camera.fitted = true;
    println!("items={} layers={}", board.items.len(), board.layers.len());
    let frame = |view: &mut eda_render::View, point: Pos2| {
        let start = Instant::now();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 800.))),
                focused: true,
                events: vec![egui::Event::PointerMoved(point)],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    eda_render::pcb::canvas(ui, board, view);
                });
            },
        );
        let paint = start.elapsed();
        let shapes = output.shapes.len();
        let meshes = ctx.tessellate(output.shapes, output.pixels_per_point);
        if std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some() {
            let vertices: usize = meshes
                .iter()
                .map(|m| match &m.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                    _ => 0,
                })
                .sum();
            eprintln!(
                "shapes={shapes} meshes={} vertices={vertices}",
                meshes.len()
            );
        }
        (
            paint.as_secs_f64() * 1000.,
            start.elapsed().as_secs_f64() * 1000.,
        )
    };
    for (name, n) in [
        ("cold", 1),
        ("cursor", 10),
        ("pan", 10),
        ("long-pan", 360),
        ("zoom", 120),
    ] {
        let mut times = vec![];
        for i in 0..n {
            if name == "pan" {
                view.camera.centre_mm[0] += 0.1;
            } else if name == "long-pan" {
                // Sweep more than two view widths, then return. Small-pan means
                // hide the expensive frames at viewport-cache boundaries.
                let offset = (i as f64 / 359. * std::f64::consts::TAU).sin() * 1200. / zoom;
                view.camera.centre_mm[0] = centre.x as f64 / 1e6 + offset;
            }
            if name == "zoom" {
                view.camera.centre_mm = [centre.x as f64 / 1e6, centre.y as f64 / 1e6];
                view.camera.pixels_per_mm =
                    zoom * (i as f64 / 119. * std::f64::consts::TAU).sin().exp();
            }
            times.push(frame(&mut view, Pos2::new(500. + i as f32, 400.)));
        }
        let mean =
            |k: fn(&(f64, f64)) -> f64| times.iter().map(k).sum::<f64>() / times.len() as f64;
        let mut total: Vec<_> = times.iter().map(|t| t.1).collect();
        total.sort_by(f64::total_cmp);
        println!(
            "{name}: paint_ms={:.2} paint+tessellate_ms={:.2} p95_ms={:.2} max_ms={:.2} frames_over_33ms={}/{}",
            mean(|t| t.0),
            mean(|t| t.1),
            total[(total.len() - 1) * 95 / 100],
            total.last().unwrap(),
            total.iter().filter(|t| **t > 33.).count(),
            total.len()
        );
    }
    if args.iter().any(|a| a == "--drag") {
        use eda_core::pcb::Shape;
        let item = board
            .items
            .iter()
            .find(|i| !i.locked && i.component.is_none() && matches!(i.shape, Shape::Via { .. }))
            .ok_or("No unlocked via")?;
        view.pcb.selected = vec![item.id];
        let check = Instant::now();
        let copy = board.clone();
        println!(
            "board_clone_ms={:.2}",
            check.elapsed().as_secs_f64() * 1000.
        );
        let check = Instant::now();
        copy.validate()?;
        println!(
            "board_validate_ms={:.2}",
            check.elapsed().as_secs_f64() * 1000.
        );
        drop(copy);
        let mut prepared = eda_core::pcb::MovePreview::new(board, &[item.id])?;
        let mut edits = vec![];
        let mut frames = vec![];
        let mut publishes = vec![];
        for n in 0..60 {
            let start = Instant::now();
            view.pcb.set_preview(None);
            let next = prepared.update(
                item.anchor(),
                Point {
                    x: (n + 1) * 10_000,
                    y: 100_000,
                },
                0.,
            )?;
            edits.push(start.elapsed().as_secs_f64() * 1000.);
            let start = Instant::now();
            view.pcb.set_move_preview_shared(board, next, &[item.id]);
            publishes.push(start.elapsed().as_secs_f64() * 1000.);
            frames.push(frame(&mut view, Pos2::new(500., 400.)).1);
        }
        edits.sort_by(f64::total_cmp);
        publishes.sort_by(f64::total_cmp);
        frames.sort_by(f64::total_cmp);
        println!(
            "drag: edit_median_ms={:.2} publish_median_ms={:.2} frame_median_ms={:.2} frame_p95_ms={:.2} frame_max_ms={:.2}",
            edits[30], publishes[30], frames[30], frames[56], frames[59]
        );
        view.pcb.set_preview(None);
    }
    if three_d {
        view.pcb.three_d.enabled = true;
        for name in ["3d-request", "3d-ready", "3d-cached", "3d-orbit"] {
            if name == "3d-ready" {
                let start = Instant::now();
                let mut maximum: f64 = 0.;
                while view.pcb.three_d.preparing() || view.pcb.three_d.rendering() {
                    if start.elapsed().as_secs() > 120 {
                        return Err("3D worker timeout".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    maximum = maximum.max(frame(&mut view, Pos2::new(500., 400.)).1);
                }
                println!(
                    "3d-background: wall_ms={:.2} max_ui_frame_ms={maximum:.2}",
                    start.elapsed().as_secs_f64() * 1000.
                );
            }
            if name == "3d-orbit" {
                view.pcb.three_d.yaw += 0.05;
            }
            let (paint, total) = frame(&mut view, Pos2::new(500., 400.));
            println!("{name}: paint_ms={paint:.2} paint+tessellate_ms={total:.2}");
            if name == "3d-orbit" {
                let start = Instant::now();
                let mut maximum: f64 = 0.;
                while view.pcb.three_d.rendering() {
                    if start.elapsed().as_secs() > 120 {
                        return Err("3D image timeout".into());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    maximum = maximum.max(frame(&mut view, Pos2::new(500., 400.)).1);
                }
                println!(
                    "3d-orbit-background: wall_ms={:.2} max_ui_frame_ms={maximum:.2}",
                    start.elapsed().as_secs_f64() * 1000.
                );
            }
        }
    }
    Ok(())
}
