//! Read-only CPU frame probe. Timings exclude the window system and GPU upload.
use eda_geom::Point;
use egui::{Pos2, Rect, Vec2};
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or("Usage: pcb_frame_profile INPUT.PcbDoc [--3d]")?,
    );
    let three_d = args.any(|a| a == "--3d");
    let doc = eda_io::load(&path)?;
    let board = doc.pcb.as_ref().ok_or("Not a PCB document")?;
    let ctx = egui::Context::default();
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
    view.camera.pixels_per_mm = 15.;
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
        let _ = ctx.tessellate(output.shapes, output.pixels_per_point);
        (
            paint.as_secs_f64() * 1000.,
            start.elapsed().as_secs_f64() * 1000.,
        )
    };
    for (name, n) in [("cold", 1), ("cursor", 10), ("pan", 10)] {
        let mut times = vec![];
        for i in 0..n {
            if name == "pan" {
                view.camera.centre_mm[0] += 0.1;
            }
            times.push(frame(&mut view, Pos2::new(500. + i as f32, 400.)));
        }
        let mean =
            |k: fn(&(f64, f64)) -> f64| times.iter().map(k).sum::<f64>() / times.len() as f64;
        println!(
            "{name}: paint_ms={:.2} paint+tessellate_ms={:.2}",
            mean(|t| t.0),
            mean(|t| t.1)
        );
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
