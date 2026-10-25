//! Read-only offscreen 3D benchmark, including actual GPU execution and waiting.
//! It measures completed offscreen frames, not window presentation FPS.
use eframe::{egui_wgpu, wgpu};
use egui::{Pos2, Rect, Vec2};
use std::{path::PathBuf, time::Instant};

fn main() -> Result<(), String> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("Usage: pcb_3d_frame_profile INPUT.PcbDoc")?,
    );
    let two_d = std::env::args_os().any(|a| a == "--2d");
    let document = eda_io::load(&path)?;
    let board = document.pcb.as_ref().ok_or("Not a PCB document")?;
    if std::env::args_os().any(|a| a == "--window") {
        let args: Vec<_> = std::env::args_os().collect();
        let capture = args
            .windows(2)
            .find(|pair| pair[0] == "--capture")
            .map(|pair| PathBuf::from(&pair[1]));
        return window(
            board.clone(),
            capture,
            two_d,
            args.iter().any(|a| a == "--drag"),
        );
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .map_err(|e| e.to_string())?;
    println!("adapter={:?}", adapter.get_info());
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))
        .map_err(|e| e.to_string())?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    eda_render::pcb::enable_gpu_with_device(&ctx, format, &device, &queue);
    eda_render::pcb_3d::enable_gpu(&ctx, &device);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("3D frame probe"),
        size: wgpu::Extent3d {
            width: 1000,
            height: 800,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let target = target.create_view(&Default::default());
    let mut view = eda_render::View::default();
    view.pcb.three_d.enabled = !two_d;
    if two_d {
        initialize_2d(&mut view, board);
    }
    let mut frame = |view: &mut eda_render::View, board: &eda_core::pcb::Board| {
        let start = Instant::now();
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
        for (id, delta) in &output.textures_delta.set {
            renderer.update_texture(&device, &queue, *id, delta);
        }
        let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [1000, 800],
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        let extra = renderer.update_buffers(&device, &queue, &mut encoder, &jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        let cpu = start.elapsed().as_secs_f64() * 1000.;
        queue.submit(extra.into_iter().chain([encoder.finish()]));
        device.poll(wgpu::PollType::Wait).unwrap();
        for id in &output.textures_delta.free {
            renderer.free_texture(id);
        }
        (cpu, start.elapsed().as_secs_f64() * 1000.)
    };
    let start = Instant::now();
    frame(&mut view, board);
    while !two_d && view.pcb.three_d.preparing() {
        if start.elapsed().as_secs() > 120 {
            return Err("Scene preparation timed out".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        frame(&mut view, board);
    }
    println!(
        "scene_and_first_gpu_frame_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    let base_scale = view.camera.pixels_per_mm;
    let base_centre = view.camera.centre_mm;
    let phases: &[&str] = if two_d {
        &["cached", "pan", "zoom", "settle"]
    } else {
        &["cached", "orbit", "pan", "zoom"]
    };
    for &phase in phases {
        let mut cpu = vec![];
        let mut completed = vec![];
        for n in 0..120 {
            if two_d {
                match phase {
                    "pan" => {
                        view.camera.centre_mm[0] = base_centre[0] + (n as f64 * 0.05).sin() * 40.
                    }
                    "zoom" => {
                        view.camera.centre_mm = base_centre;
                        view.camera.pixels_per_mm = base_scale * (n as f64 * 0.05).sin().exp();
                    }
                    "settle" => std::thread::sleep(std::time::Duration::from_millis(8)),
                    _ => {}
                }
            } else {
                match phase {
                    "orbit" => {
                        view.pcb.three_d.yaw += 0.015;
                        view.pcb.three_d.pitch = 0.7 + (n as f32 * 0.03).sin() * 0.3;
                    }
                    "pan" => {
                        view.pcb.three_d.pan = Vec2::new(
                            (n as f32 * 0.05).sin() * 200.,
                            (n as f32 * 0.03).sin() * 100.,
                        )
                    }
                    "zoom" => view.pcb.three_d.zoom = 1. + (n as f32 * 0.03).sin().abs() * 2.,
                    _ => {}
                }
            }
            let (a, b) = frame(&mut view, board);
            cpu.push(a);
            completed.push(b);
        }
        cpu.sort_by(f64::total_cmp);
        completed.sort_by(f64::total_cmp);
        println!(
            "{phase}: cpu_median_ms={:.2} completed_median_ms={:.2} completed_p95_ms={:.2} completed_max_ms={:.2}",
            cpu[60], completed[60], completed[113], completed[119]
        );
    }
    if !two_d && std::env::args_os().any(|a| a == "--edits") {
        use eda_core::pcb::{Edit, Shape};
        let mut current = board.clone();
        for kind in ["track", "via", "component"] {
            let board = &current;
            let target = if kind == "component" {
                board
                    .components
                    .iter()
                    .find(|c| {
                        !c.locked
                            && !board
                                .items
                                .iter()
                                .any(|i| i.component == Some(c.index) && i.locked)
                    })
                    .map(|c| (c.id, c.position))
            } else {
                board
                    .items
                    .iter()
                    .find(|i| {
                        !i.locked
                            && i.component.is_none()
                            && i.polygon.is_none()
                            && match kind {
                                "via" => matches!(i.shape, Shape::Via { .. }),
                                _ => matches!(i.shape, Shape::Track { .. }),
                            }
                    })
                    .map(|i| (i.id, i.anchor()))
            };
            let Some((id, pivot)) = target else {
                continue;
            };
            let edited = board.edited(Edit::Transform {
                selection: vec![id],
                pivot,
                translation: eda_geom::Point::mm(1, 1),
                angle: 0.,
            })?;
            view.pcb.three_d.revision += 1;
            let start = Instant::now();
            frame(&mut view, &edited);
            while view.pcb.three_d.preparing() {
                if start.elapsed().as_secs() > 120 {
                    return Err("Edited scene timed out".into());
                }
                std::thread::sleep(std::time::Duration::from_millis(2));
                frame(&mut view, &edited);
            }
            println!(
                "after_{kind}_edit_scene_ms={:.2}",
                start.elapsed().as_secs_f64() * 1000.
            );
            current = edited;
        }
    }
    Ok(())
}

// Native-window QA without desktop input injection. The same canvas and GPU
// callback run inside eframe; only this read-only probe animates the camera.
fn window(
    board: eda_core::pcb::Board,
    capture: Option<PathBuf>,
    two_d: bool,
    drag: bool,
) -> Result<(), String> {
    struct Probe {
        board: eda_core::pcb::Board,
        view: eda_render::View,
        ready: Option<Instant>,
        capture: Option<PathBuf>,
        requested_capture: bool,
        two_d: bool,
        drag: bool,
        move_preview: Option<eda_core::pcb::MovePreview>,
        base_scale: Option<f64>,
        cpu_times: Vec<f32>,
    }
    impl eframe::App for Probe {
        fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
            for event in ctx.input(|i| i.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event
                    && let Some(path) = self.capture.take()
                {
                    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
                    image::save_buffer(
                        &path,
                        &rgba,
                        image.size[0] as u32,
                        image.size[1] as u32,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                    eprintln!("Captured native frame: {}", path.display());
                }
            }
            if self.two_d || !self.view.pcb.three_d.preparing() {
                let t = self
                    .ready
                    .get_or_insert_with(Instant::now)
                    .elapsed()
                    .as_secs_f32();
                if t > if self.two_d { 7. } else { 3. }
                    && self.capture.is_some()
                    && !self.requested_capture
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    self.requested_capture = true;
                }
                if self.two_d {
                    let base = *self
                        .base_scale
                        .get_or_insert(self.view.camera.pixels_per_mm);
                    // Camera zoom, then idle frames that let the
                    // exact-detail worker complete and publish its buffers.
                    if self.drag {
                        if t < 7. {
                            use eda_core::pcb::Shape;
                            if let Some(item) = self.board.items.iter().find(|i| {
                                !i.locked
                                    && i.component.is_none()
                                    && matches!(i.shape, Shape::Via { .. })
                            }) {
                                let selection = vec![item.id];
                                self.view.pcb.selected = selection.clone();
                                if self.move_preview.is_none() {
                                    self.move_preview = Some(
                                        eda_core::pcb::MovePreview::new(&self.board, &selection)
                                            .unwrap(),
                                    );
                                }
                                self.view.pcb.set_preview(None);
                                let next = self
                                    .move_preview
                                    .as_mut()
                                    .unwrap()
                                    .update(
                                        item.anchor(),
                                        eda_geom::Point {
                                            x: (f64::from(t).sin() * 1e6) as i64,
                                            y: (f64::from(t).cos() * 1e6) as i64,
                                        },
                                        0.,
                                    )
                                    .unwrap();
                                self.view.pcb.set_move_preview_shared(
                                    &self.board,
                                    next,
                                    &selection,
                                );
                            }
                        }
                        if t > 14. {
                            self.cpu_times.sort_by(f32::total_cmp);
                            let n = self.cpu_times.len();
                            if n > 0 {
                                eprintln!(
                                    "native_drag_cpu frames={n} median_ms={:.2} p95_ms={:.2} max_ms={:.2}",
                                    self.cpu_times[n / 2] * 1000.,
                                    self.cpu_times[(n - 1) * 95 / 100] * 1000.,
                                    self.cpu_times[n - 1] * 1000.
                                );
                            }
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    } else if t < 5. {
                        self.view.camera.pixels_per_mm = base * (f64::from(t) * 2.).sin().exp();
                    } else if t < 8. {
                        self.view.camera.pixels_per_mm = base * 2.;
                    } else if t < 11. {
                        self.view.camera.pixels_per_mm = base * 0.6;
                    } else if t > 14. {
                        self.cpu_times.sort_by(f32::total_cmp);
                        let n = self.cpu_times.len();
                        if n > 0 {
                            eprintln!(
                                "native_2d_cpu frames={} median_ms={:.2} p95_ms={:.2} max_ms={:.2} frames_over_33ms={}",
                                n,
                                self.cpu_times[n / 2] * 1000.,
                                self.cpu_times[(n - 1) * 95 / 100] * 1000.,
                                self.cpu_times[n - 1] * 1000.,
                                self.cpu_times.iter().filter(|t| **t > 0.033).count()
                            );
                        }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                } else if t < 8. {
                    self.view.pcb.three_d.yaw = t * 0.6;
                    self.view.pcb.three_d.pitch = 0.7 + (t * 0.7).sin() * 0.3;
                } else if t < 12. {
                    self.view.pcb.three_d.pan =
                        Vec2::new((t * 2.).sin() * 100., (t * 1.5).sin() * 70.);
                } else if t < 16. {
                    self.view.pcb.three_d.zoom = 1. + (t * 1.5).sin().abs() * 2.;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if self.capture.is_some() && !self.two_d {
                    // A reproducible oblique pose for before/after image QA.
                    self.view.pcb.three_d.yaw = 0.6;
                    self.view.pcb.three_d.pitch = 0.55;
                    self.view.pcb.three_d.zoom = 1.4;
                    self.view.pcb.three_d.pan = Vec2::ZERO;
                }
                if let Some(cpu) = frame.info().cpu_usage
                    && t > 1.
                    && (!self.drag || t < 7.)
                {
                    self.cpu_times.push(cpu);
                }
                if let Some(cpu) = frame.info().cpu_usage
                    && cpu > 0.033
                {
                    eprintln!("native_app_cpu_ms={:.2}", cpu * 1000.);
                }
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                eda_render::pcb::canvas(ui, &self.board, &mut self.view);
            });
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
    eframe::run_native(
        "PCBCraft 3D performance probe",
        eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default().with_inner_size([1000., 800.]),
            ..Default::default()
        },
        Box::new(move |cc| {
            let render = cc
                .wgpu_render_state
                .as_ref()
                .ok_or_else(|| std::io::Error::other("wgpu is required"))?;
            eda_render::pcb::enable_gpu_with_device(
                &cc.egui_ctx,
                render.target_format,
                &render.device,
                &render.queue,
            );
            eda_render::pcb_3d::enable_gpu(&cc.egui_ctx, &render.device);
            install_fonts(&cc.egui_ctx);
            let mut view = eda_render::View::default();
            view.pcb.three_d.enabled = !two_d;
            if two_d {
                initialize_2d(&mut view, &board);
            }
            Ok(Box::new(Probe {
                board,
                view,
                ready: None,
                capture,
                requested_capture: false,
                two_d,
                drag,
                move_preview: None,
                base_scale: None,
                cpu_times: vec![],
            }))
        }),
    )
    .map_err(|e| e.to_string())
}

fn install_fonts(ctx: &egui::Context) {
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
}

fn initialize_2d(view: &mut eda_render::View, board: &eda_core::pcb::Board) {
    let centre = eda_core::pcb::Bounds::points(&board.outline_points(256)).centre();
    view.camera.centre_mm = [centre.x as f64 / 1e6, centre.y as f64 / 1e6];
    view.camera.pixels_per_mm = 15.;
    view.camera.fitted = true;
}
