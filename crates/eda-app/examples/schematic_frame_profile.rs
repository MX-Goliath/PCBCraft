//! Read-only pointer-frame probe, including egui tessellation but no GPU execution.
use egui::{Pos2, Rect, Vec2};
use std::time::Instant;

fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).ok_or("Pass a SchDoc")?;
    let reading = Instant::now();
    let doc = eda_io::load(std::path::Path::new(&path))?;
    println!("load_ms={:.3}", reading.elapsed().as_secs_f64() * 1000.);
    let preparing = Instant::now();
    let _saved_snapshot = std::hint::black_box(doc.clone());
    println!(
        "saved_snapshot_ms={:.3}",
        preparing.elapsed().as_secs_f64() * 1000.
    );
    let s = doc.schematic.as_ref().ok_or("Not a schematic")?;
    println!(
        "components={} primitives={} parameters={}",
        s.components.len(),
        s.primitives.len(),
        s.parameter_objects.len()
    );
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::default();
    for face in [
        "sch-sans",
        "sch-serif",
        "sch-mono",
        "sch-gost",
        "sch-altium-gost",
    ] {
        for suffix in ["", "-bold", "-italic", "-bold-italic"] {
            fonts.families.insert(
                egui::FontFamily::Name(format!("{face}{suffix}").into()),
                fonts.families[&egui::FontFamily::Proportional].clone(),
            );
        }
    }
    ctx.set_fonts(fonts);
    let mut view = eda_render::View::default();
    for zoom in [0., 10., 25.] {
        view.sch_opening = zoom == 0.;
        view.camera.fitted = false;
        if zoom != 0. {
            view.camera.centre_mm = [doc.size.x as f64 / 2e6, doc.size.y as f64 / 2e6];
            view.camera.pixels_per_mm = zoom;
            view.camera.fitted = true;
        }
        let mut times = vec![];
        let mut shapes = 0;
        let mut opening_ms = 0.;
        let mut opening_frames = 0;
        let mut opening_max_ms: f64 = 0.;
        let mut warmed = false;
        for n in 0..10_000 {
            let start = Instant::now();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1068., 900.))),
                    focused: true,
                    events: vec![egui::Event::PointerMoved(Pos2::new(
                        50. + (n % 20) as f32 * 48.,
                        150. + (n / 20) as f32 * 150.,
                    ))],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        eda_render::canvas(ui, &doc, &mut view);
                    });
                },
            );
            let paint = start.elapsed().as_secs_f64() * 1000.;
            shapes = output.shapes.len();
            std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
            let total = start.elapsed().as_secs_f64() * 1000.;
            if !warmed {
                opening_ms += total;
                opening_frames += 1;
                opening_max_ms = opening_max_ms.max(total);
                if !view.sch_opening {
                    warmed = true;
                    println!(
                        "cold_graphics_frames={opening_frames} cold_graphics_cpu_ms={opening_ms:.3} max_cold_frame_ms={opening_max_ms:.3}"
                    );
                }
            } else {
                times.push((paint, start.elapsed().as_secs_f64() * 1000.));
            }
            if times.len() == 60 {
                break;
            }
        }
        if times.len() != 60 {
            return Err("Schematic preparation did not finish within the probe frame limit".into());
        }
        times.sort_by(|a, b| a.1.total_cmp(&b.1));
        println!(
            "zoom={:.3} shapes={shapes} paint_mean_ms={:.3} total_mean_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            view.camera.pixels_per_mm,
            times.iter().map(|t| t.0).sum::<f64>() / times.len() as f64,
            times.iter().map(|t| t.1).sum::<f64>() / times.len() as f64,
            times[times.len() * 95 / 100].1,
            times.last().unwrap().1
        );
    }
    Ok(())
}
