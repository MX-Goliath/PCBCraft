//! Read-only root-comparison frame probe; includes egui tessellation, no GPU/present.
use eda_core::document_diff;
use eda_render::{View, document_diff::Mode};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for file in std::env::args().skip(1) {
        let started = Instant::now();
        let doc = if file == "--synthetic-sch" {
            let mut doc =
                eda_core::Document::new(eda_core::DocumentKind::Schematic, "20000 additions");
            for i in 0..20_000 {
                doc.schematic.as_mut().unwrap().primitives.push(
                    eda_core::schematic_edit::primitive(
                        eda_core::schematic::SchPrimitiveKind::Rectangle {
                            a: eda_geom::Point::mm(i % 200, i / 200),
                            b: eda_geom::Point::mm(i % 200 + 1, i / 200 + 1),
                            radius: eda_geom::Point::default(),
                        },
                        None,
                    ),
                );
            }
            doc
        } else if file.ends_with(".json") {
            eda_io::load(Path::new(&file))?
        } else {
            let bytes = std::fs::read(&file)?;
            eda_io::git::import_document(&bytes, &file)?
        };
        let import_ms = started.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        let diff = Arc::new(document_diff::compare(None, Some(&doc)));
        let compare_ms = started.elapsed().as_secs_f64() * 1000.;
        let doc = Arc::new(doc);
        println!("{file} import_ms={import_ms:.2} compare_ms={compare_ms:.2}");
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        let fallback = fonts.families[&egui::FontFamily::Proportional].clone();
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
                    fallback.clone(),
                );
            }
        }
        ctx.set_fonts(fonts);
        let mut view = View::default();
        for mode in [Mode::Overlay, Mode::After] {
            let mut samples = Vec::new();
            let cold = Instant::now();
            let mut warm = 0;
            let mut waiting = 0;
            let mut cold_max_ms = 0_f64;
            loop {
                let start = Instant::now();
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1050., 850.),
                        )),
                        events: vec![egui::Event::PointerMoved(egui::pos2(
                            550. + warm as f32,
                            450.,
                        ))],
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            eda_render::document_diff::canvas_shared(
                                ui,
                                None,
                                Some(&doc),
                                &diff,
                                &mut view,
                                mode,
                                [true; 4],
                                None,
                                warm == 0 && waiting == 0,
                            );
                        });
                    },
                );
                std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
                let ms = start.elapsed().as_secs_f64() * 1000.;
                if eda_render::document_diff::preparation_failed(&view) {
                    return Err("Comparison graphics preparation failed".into());
                }
                if eda_render::document_diff::preparing(&view) {
                    waiting += 1;
                    cold_max_ms = cold_max_ms.max(ms);
                    if cold.elapsed() > Duration::from_secs(120) {
                        return Err("Comparison graphics preparation timed out".into());
                    }
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                if warm == 0 {
                    cold_max_ms = cold_max_ms.max(ms);
                    println!(
                        "{file} mode={} first_ready_ms={:.2} cold_ui_max_ms={cold_max_ms:.2} waiting_frames={waiting}",
                        if mode == Mode::Overlay {
                            "overlay"
                        } else {
                            "after"
                        },
                        cold.elapsed().as_secs_f64() * 1000.
                    );
                }
                samples.push(ms);
                warm += 1;
                if warm == 9 {
                    break;
                }
                view.camera.centre_mm[0] += 0.05;
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{} changes={} mode={} median_ms={:.2} max_ms={:.2}",
                Path::new(&file).file_name().unwrap().to_string_lossy(),
                diff.changes.len(),
                if mode == Mode::Overlay {
                    "overlay"
                } else {
                    "after"
                },
                samples[samples.len() / 2],
                samples.last().unwrap()
            );
        }
    }
    Ok(())
}
