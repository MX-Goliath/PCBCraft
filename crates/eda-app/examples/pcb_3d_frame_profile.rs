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
    let document = eda_io::load(&path)?;
    let board = document.pcb.as_ref().ok_or("Not a PCB document")?;
    if std::env::args_os().any(|a| a == "--window") {
        let args: Vec<_> = std::env::args_os().collect();
        let capture = args
            .windows(2)
            .find(|pair| pair[0] == "--capture")
            .map(|pair| PathBuf::from(&pair[1]));
        return window(board.clone(), capture);
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
    eda_render::pcb::enable_gpu(&ctx, format);
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
    view.pcb.three_d.enabled = true;
    let mut frame = |view: &mut eda_render::View| {
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
    frame(&mut view);
    while view.pcb.three_d.preparing() {
        if start.elapsed().as_secs() > 120 {
            return Err("Scene preparation timed out".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        frame(&mut view);
    }
    println!(
        "scene_and_first_gpu_frame_ms={:.2}",
        start.elapsed().as_secs_f64() * 1000.
    );
    for phase in ["cached", "orbit", "pan", "zoom"] {
        let mut cpu = vec![];
        let mut completed = vec![];
        for n in 0..120 {
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
            let (a, b) = frame(&mut view);
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
    Ok(())
}

// Native-window QA without desktop input injection. The same canvas and GPU
// callback run inside eframe; only this read-only probe animates the camera.
fn window(board: eda_core::pcb::Board, capture: Option<PathBuf>) -> Result<(), String> {
    struct Probe {
        board: eda_core::pcb::Board,
        view: eda_render::View,
        ready: Option<Instant>,
        capture: Option<PathBuf>,
        requested_capture: bool,
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
                    eprintln!("Captured native 3D frame: {}", path.display());
                }
            }
            if !self.view.pcb.three_d.preparing() {
                let t = self
                    .ready
                    .get_or_insert_with(Instant::now)
                    .elapsed()
                    .as_secs_f32();
                let state = &mut self.view.pcb.three_d;
                if t > 3. && self.capture.is_some() && !self.requested_capture {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
                    self.requested_capture = true;
                }
                if t < 8. {
                    state.yaw = t * 0.6;
                    state.pitch = 0.7 + (t * 0.7).sin() * 0.3;
                } else if t < 12. {
                    state.pan = Vec2::new((t * 2.).sin() * 100., (t * 1.5).sin() * 70.);
                } else if t < 16. {
                    state.zoom = 1. + (t * 1.5).sin().abs() * 2.;
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
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
            eda_render::pcb::enable_gpu(&cc.egui_ctx, render.target_format);
            eda_render::pcb_3d::enable_gpu(&cc.egui_ctx, &render.device);
            let mut view = eda_render::View::default();
            view.pcb.three_d.enabled = true;
            Ok(Box::new(Probe {
                board,
                view,
                ready: None,
                capture,
                requested_capture: false,
            }))
        }),
    )
    .map_err(|e| e.to_string())
}
