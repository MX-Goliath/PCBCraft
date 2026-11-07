//! Read-only root-comparison frame probe; includes egui tessellation, no GPU/present.
use eda_core::document_diff;
use eda_render::{View, document_diff::Mode};
use std::{path::Path, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for file in std::env::args().skip(1) {
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
        } else {
            let bytes = std::fs::read(&file)?;
            eda_io::git::import_document(&bytes, &file)?
        };
        let diff = document_diff::compare(None, Some(&doc));
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
            for frame in 0..9 {
                let start = Instant::now();
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(1050., 850.),
                        )),
                        events: vec![egui::Event::PointerMoved(egui::pos2(
                            550. + frame as f32,
                            450.,
                        ))],
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            eda_render::document_diff::canvas(
                                ui,
                                None,
                                Some(&doc),
                                &diff,
                                &mut view,
                                mode,
                                [true; 4],
                                None,
                                frame == 0,
                            );
                        });
                    },
                );
                std::hint::black_box(ctx.tessellate(output.shapes, output.pixels_per_point));
                if frame > 1 {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
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
