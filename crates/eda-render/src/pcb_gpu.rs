//! Resident 2D artwork buffers. Camera pan and zoom update a single small uniform.
use egui::{ColorImage, Context, Rect, Vec2, epaint::Mesh};
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor, wgpu};
use std::sync::{Arc, Mutex};
use wgpu::util::DeviceExt;

/// Enable the PCB callback for an egui-wgpu target using one MSAA sample.
/// Headless and other egui integrations keep the ordinary mesh painter.
pub fn enable_gpu(ctx: &Context, format: wgpu::TextureFormat) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("pcb-gpu-format"), format));
}
#[derive(Clone)]
struct Upload {
    device: wgpu::Device,
    queue: wgpu::Queue,
}
/// Also allow zoom refinement to prepare resident buffers off the UI thread.
pub fn enable_gpu_with_device(
    ctx: &Context,
    format: wgpu::TextureFormat,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) {
    enable_gpu(ctx, format);
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("pcb-gpu-upload"),
            Upload {
                device: device.clone(),
                queue: queue.clone(),
            },
        )
    });
}
pub(crate) fn copy_configuration(from: &Context, to: &Context) {
    if let Some(format) = format(from) {
        enable_gpu(to, format);
    }
    if let Some(upload) =
        from.data_mut(|data| data.get_temp::<Upload>(egui::Id::new("pcb-gpu-upload")))
    {
        to.data_mut(|data| data.insert_temp(egui::Id::new("pcb-gpu-upload"), upload));
    }
}
pub(crate) fn format(ctx: &Context) -> Option<wgpu::TextureFormat> {
    ctx.data_mut(|data| data.get_temp(egui::Id::new("pcb-gpu-format")))
}
pub(super) fn artwork_dither(ctx: &Context) -> bool {
    #[cfg(test)]
    {
        ctx.data_mut(|data| data.get_temp::<bool>(egui::Id::new("pcb-artwork-test-dither")))
            .unwrap_or(true)
    }
    #[cfg(not(test))]
    {
        let _ = ctx;
        true
    }
}
pub(crate) struct Scene {
    meshes: Vec<Arc<Mesh>>,
    image: ColorImage,
    format: wgpu::TextureFormat,
    prepared: Mutex<Option<Buffers>>,
}
struct Buffers {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    meshes: Vec<(wgpu::Buffer, wgpu::Buffer)>,
    camera: Option<[f32; 8]>,
}
impl Scene {
    pub fn new(
        meshes: Vec<Arc<Mesh>>,
        image: ColorImage,
        format: wgpu::TextureFormat,
    ) -> Arc<Self> {
        Arc::new(Self {
            meshes,
            image,
            format,
            prepared: Mutex::new(None),
        })
    }
    pub fn callback(
        self: &Arc<Self>,
        mesh: usize,
        rect: Rect,
        delta: Vec2,
        scale: f32,
        dither: bool,
    ) -> egui::Shape {
        egui::Shape::Callback(Callback::new_paint_callback(
            rect,
            Draw {
                scene: self.clone(),
                mesh,
                rect,
                delta,
                scale,
                dither,
            },
        ))
    }
    pub(crate) fn prepare_background(&self, ctx: &Context) {
        if let Some(upload) =
            ctx.data_mut(|data| data.get_temp::<Upload>(egui::Id::new("pcb-gpu-upload")))
        {
            *self.prepared.lock().unwrap() = Some(self.prepare(&upload.device, &upload.queue));
        }
    }
    fn prepare(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Buffers {
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PCB camera"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PCB artwork layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let size = wgpu::Extent3d {
            width: self.image.size[0] as u32,
            height: self.image.size[1] as u32,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PCB font atlas snapshot"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            bytemuck::cast_slice(&self.image.pixels),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size.width * 4),
                rows_per_image: None,
            },
            size,
        );
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("PCB atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PCB artwork bindings"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PCB cached artwork"),
            source: wgpu::ShaderSource::Wgsl(include_str!("pcb_gpu.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PCB artwork pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PCB artwork pipeline"), layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vertex"),
                buffers: &[wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<egui::epaint::Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Uint32] }],
                compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &shader,
                entry_point: Some(if self.format.is_srgb() { "linear_fragment" } else { "gamma_fragment" }),
                targets: &[Some(wgpu::ColorTargetState { format: self.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                        alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                            dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add },
                    }), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: Default::default(), depth_stencil: None, multisample: Default::default(), multiview: None, cache: None,
        });
        let meshes = self
            .meshes
            .iter()
            .map(|mesh| {
                let buffer = |label, contents, usage| {
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(label),
                        contents,
                        usage,
                    })
                };
                (
                    buffer(
                        "PCB vertices",
                        bytemuck::cast_slice(&mesh.vertices),
                        wgpu::BufferUsages::VERTEX,
                    ),
                    buffer(
                        "PCB indices",
                        bytemuck::cast_slice(&mesh.indices),
                        wgpu::BufferUsages::INDEX,
                    ),
                )
            })
            .collect();
        Buffers {
            pipeline,
            uniform,
            bindings,
            meshes,
            camera: None,
        }
    }
}
struct Draw {
    scene: Arc<Scene>,
    mesh: usize,
    rect: Rect,
    delta: Vec2,
    scale: f32,
    dither: bool,
}
impl CallbackTrait for Draw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &ScreenDescriptor,
        _: &mut wgpu::CommandEncoder,
        _: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        // Reused groups may omit mesh zero from an older scene. Every submitted
        // callback must therefore be able to prepare that scene's camera.
        {
            let mut prepared = self.scene.prepared.lock().unwrap();
            let buffers = prepared.get_or_insert_with(|| self.scene.prepare(device, queue));
            let viewport = egui::PaintCallbackInfo {
                viewport: self.rect,
                clip_rect: self.rect,
                pixels_per_point: screen.pixels_per_point,
                screen_size_px: screen.size_in_pixels,
            }
            .viewport_in_pixels();
            let dpi = screen.pixels_per_point;
            let shift =
                self.delta - Vec2::new(viewport.left_px as f32, viewport.top_px as f32) / dpi;
            let uniform = [
                viewport.width_px as f32 / dpi,
                viewport.height_px as f32 / dpi,
                shift.x,
                shift.y,
                if self.dither { 1. } else { 0. },
                self.scale,
                0.,
                0.,
            ];
            if buffers.camera != Some(uniform) {
                queue.write_buffer(&buffers.uniform, 0, bytemuck::cast_slice(&uniform));
                buffers.camera = Some(uniform);
            }
        }
        vec![]
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _: &CallbackResources,
    ) {
        let prepared = self.scene.prepared.lock().unwrap();
        let Some(buffers) = prepared.as_ref() else {
            return;
        };
        let (vertices, indices) = &buffers.meshes[self.mesh];
        pass.set_pipeline(&buffers.pipeline);
        pass.set_bind_group(0, &buffers.bindings, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(
            0..self.scene.meshes[self.mesh].indices.len() as u32,
            0,
            0..1,
        );
    }
}

#[cfg(test)]
mod tests {
    #[path = "pcb_route_gpu_tests.rs"]
    mod routing;
    use super::*;
    use egui::{
        Color32, Pos2,
        epaint::{ClippedPrimitive, Primitive},
    };

    fn image(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut egui_wgpu::Renderer,
        jobs: &[ClippedPrimitive],
        format: wgpu::TextureFormat,
        dpi: f32,
    ) -> Vec<u8> {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let screen = ScreenDescriptor {
            size_in_pixels: [64, 64],
            pixels_per_point: dpi,
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        let extra = renderer.update_buffers(device, queue, &mut encoder, jobs, &screen);
        {
            let view = target.create_view(&Default::default());
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.12,
                            g: 0.23,
                            b: 0.34,
                            a: 1.,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), jobs, &screen);
        }
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            target.size(),
        );
        queue.submit(extra.into_iter().chain([encoder.finish()]));
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::Wait).unwrap();
        receive.recv().unwrap().unwrap();
        readback.slice(..).get_mapped_range().to_vec()
    }
    #[test]
    fn moving_preview_matches_fresh_pixels_and_keeps_static_buffers() {
        use crate::{View, pcb};
        use eda_core::pcb::*;
        use eda_geom::Point;
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let mut renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
        let ctx = Context::default();
        enable_gpu(&ctx, format);
        let mut b = Board::default();
        b.items.push(Item::new(
            MULTI_LAYER,
            None,
            Shape::Via {
                position: Point::default(),
                diameter: 2_000_000,
                hole: 600_000,
                from_layer: TOP,
                to_layer: BOTTOM,
            },
        ));
        b.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(-3, 0),
                end: Point::mm(3, 0),
                width: 300_000,
            },
        ));
        let mut view = View::default();
        view.camera.fitted = true;
        view.camera.pixels_per_mm = 8.;
        view.grid = false;
        view.pcb.initialize(&b);
        view.pcb.display.connections = false;
        view.pcb.display.pad_numbers = false;
        view.pcb.display.pad_nets = false;
        view.pcb.display.via_nets = false;
        view.pcb.display.origin_visible = false;
        view.pcb.visible.extend([TOP, BOTTOM, MULTI_LAYER, 37, 38]);
        view.pcb.selected = vec![b.items[0].id];
        let mut render = |view: &mut View| {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::splat(64.))),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pcb::canvas(ui, &b, view);
                    });
                },
            );
            for (id, delta) in &output.textures_delta.set {
                renderer.update_texture(&device, &queue, *id, delta);
            }
            let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
            let result = image(&device, &queue, &mut renderer, &jobs, format, 1.);
            for id in &output.textures_delta.free {
                renderer.free_texture(id);
            }
            result
        };
        let mut built = None;
        for n in 1..5 {
            let selection = [b.items[0].id];
            let moved = b
                .edited(Edit::Transform {
                    selection: selection.to_vec(),
                    pivot: Point::default(),
                    translation: Point {
                        x: n * 125_000,
                        y: n * 250_000,
                    },
                    angle: 0.,
                })
                .unwrap();
            view.pcb.set_move_preview(&b, moved, &selection);
            let actual = render(&mut view);
            assert_eq!(
                *built.get_or_insert(view.pcb.paint_cache.builds),
                view.pcb.paint_cache.builds
            );
            let mut fresh = view.clone();
            fresh.pcb.preview_items = None;
            fresh.pcb.paint_cache = Default::default();
            let expected = render(&mut fresh);
            // Resident artwork applies the PCB shader's sub-LSB dithering;
            // live primitives use egui's pass. Allow only quantization noise.
            let error = actual
                .iter()
                .zip(&expected)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(
                error <= 2,
                "live vs full rebuild: step={n} maximum channel error={error}"
            );
        }
        view.pcb.set_preview(None);
        render(&mut view);
        assert!(view.pcb.paint_cache.builds > built.unwrap());
        let mut placement = InsertPreview::new(&b).unwrap();
        let mut resident = None;
        for n in 0..6 {
            view.pcb.set_preview(None);
            let position = Point {
                x: n * 125_000,
                y: 500_000,
            };
            let shape = if n % 2 == 0 {
                Shape::Pad {
                    position,
                    size: Point::mm(1, 2),
                    rotation: 30.,
                    shape: PadShape::Round,
                    hole: 300_000,
                    plated: true,
                    name: "1".into(),
                    bottom_size: None,
                    middle_size: None,
                }
            } else {
                Shape::Via {
                    position,
                    diameter: 900_000,
                    hole: 300_000,
                    from_layer: TOP,
                    to_layer: BOTTOM,
                }
            };
            let next = placement
                .update(vec![Item::new(MULTI_LAYER, None, shape)])
                .unwrap();
            view.pcb
                .set_insert_preview_shared(next, placement.source_len());
            let actual = render(&mut view);
            if let Some(scene) = &resident {
                assert!(Arc::ptr_eq(
                    scene,
                    view.pcb.paint_cache.resident_scene().as_ref().unwrap()
                ));
            } else {
                resident = view.pcb.paint_cache.resident_scene();
            }
            let mut fresh = view.clone();
            fresh.pcb.route_preview = None;
            fresh.pcb.preview_items = None;
            fresh.pcb.paint_cache = Default::default();
            let expected = render(&mut fresh);
            let error = actual
                .iter()
                .zip(expected)
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            assert!(error <= 2, "placement step {n} pixel error {error}");
        }
    }
    #[test]
    fn resident_artwork_matches_egui_meshes_during_pan_clip_dpi_and_blending() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("No graphics adapter; GPU image comparison skipped");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let mut mesh = Mesh::default();
        mesh.add_rect_with_uv(
            Rect::from_min_max(Pos2::new(5., 8.), Pos2::new(38., 34.)),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::from_rgba_unmultiplied(200, 70, 30, 160),
        );
        mesh.add_colored_rect(
            Rect::from_min_max(Pos2::new(15., 16.), Pos2::new(41., 40.)),
            Color32::from_rgba_unmultiplied(20, 170, 90, 140),
        );
        let atlas = ColorImage::new(
            [2, 2],
            vec![
                Color32::WHITE,
                Color32::from_gray(100),
                Color32::WHITE,
                Color32::from_gray(180),
            ],
        );
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            for dpi in [1., 1.5, 2.] {
                let mut renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
                renderer.update_texture(
                    &device,
                    &queue,
                    egui::TextureId::default(),
                    &egui::epaint::ImageDelta::full(atlas.clone(), egui::TextureOptions::LINEAR),
                );
                let scene = Scene::new(vec![Arc::new(mesh.clone())], atlas.clone(), format);
                let mut buffers = None;
                for (delta, scale) in [
                    (Vec2::ZERO, 1.),
                    (Vec2::new(7.5, -3.5), 0.75),
                    (Vec2::new(-2., 4.), 1.5),
                ] {
                    let rect = Rect::from_min_max(Pos2::new(3.5, 4.5), Pos2::new(55., 52.));
                    let mut moved = mesh.clone();
                    for vertex in &mut moved.vertices {
                        vertex.pos = (vertex.pos.to_vec2() * scale + delta).to_pos2();
                    }
                    let cpu = vec![ClippedPrimitive {
                        clip_rect: rect,
                        primitive: Primitive::Mesh(moved),
                    }];
                    let egui::Shape::Callback(callback) =
                        scene.callback(0, rect, delta, scale, false)
                    else {
                        unreachable!()
                    };
                    let gpu = vec![ClippedPrimitive {
                        clip_rect: rect,
                        primitive: Primitive::Callback(callback),
                    }];
                    let expected = image(&device, &queue, &mut renderer, &cpu, format, dpi);
                    let actual = image(&device, &queue, &mut renderer, &gpu, format, dpi);
                    let maximum = actual
                        .iter()
                        .zip(expected)
                        .map(|(a, b)| a.abs_diff(b))
                        .max()
                        .unwrap();
                    assert!(
                        maximum <= 1,
                        "GPU pixels differ: format={format:?} dpi={dpi} delta={delta:?} max={maximum}"
                    );
                    let prepared = scene.prepared.lock().unwrap();
                    let pointer =
                        &prepared.as_ref().unwrap().meshes[0].0 as *const wgpu::Buffer as usize;
                    if let Some(old) = buffers {
                        assert_eq!(pointer, old, "Pan uploaded vertices again");
                    }
                    buffers = Some(pointer);
                }
            }
        }
        // Exercise the full canvas/cache handoff too. The callback must receive
        // the meshes built this frame, including interleaved live hatch passes.
        use crate::{View, pcb};
        use eda_core::pcb::{Board, ContourVertex, Item, Shape, TOP, rectangle};
        use eda_geom::Point;
        let mut board = Board {
            outline: rectangle(Point::mm(100, 0), Point::mm(440, 20), 0.)
                .into_iter()
                .map(|position| ContourVertex {
                    position,
                    arc: None,
                })
                .collect(),
            ..Default::default()
        };
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(-6, 0),
                end: Point::mm(6, 0),
                width: 2_000_000,
            },
        ));
        let mut body = Item::new(
            57,
            None,
            Shape::Region {
                contour: rectangle(Point::default(), Point::mm(8, 8), 0.),
                holes: vec![],
                cutout: false,
            },
        );
        body.properties
            .insert("PCBCRAFT_BODY_PROJECTION".into(), "TRUE".into());
        board.items.push(body);
        board.items.push(Item::new(
            TOP,
            None,
            Shape::Track {
                start: Point::mm(194, 0),
                end: Point::mm(206, 0),
                width: 2_000_000,
            },
        ));
        board.properties.insert("ORIGINX".into(), "200mm".into());
        let mut pictures = vec![];
        for resident in [false, true] {
            let ctx = Context::default();
            if resident {
                enable_gpu(&ctx, wgpu::TextureFormat::Rgba8Unorm);
            }
            let mut renderer =
                egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, None, 1, true);
            let mut view = View::default();
            view.camera.fitted = true;
            view.camera.pixels_per_mm = 3.;
            view.pcb.initialize(&board);
            view.pcb.display.connections = false;
            view.pcb.visible.insert(57);
            let mut frames = vec![];
            for offset in [0., 0.5, 1., 200., -200., 0.] {
                if !resident {
                    // A fresh scene at the destination is the reference for a
                    // resident scene translated from far outside the viewport.
                    view.pcb.paint_cache = Default::default();
                }
                view.camera.centre_mm[0] = offset;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::splat(64.))),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            pcb::canvas(ui, &board, &mut view);
                        });
                    },
                );
                for (id, delta) in output.textures_delta.set {
                    renderer.update_texture(&device, &queue, id, &delta);
                }
                let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
                frames.push(image(
                    &device,
                    &queue,
                    &mut renderer,
                    &jobs,
                    wgpu::TextureFormat::Rgba8Unorm,
                    1.,
                ));
                if resident {
                    assert_eq!(view.pcb.paint_cache.builds, 1);
                }
            }
            pictures.push(frames);
        }
        for (index, (cpu, gpu)) in pictures[0].iter().zip(&pictures[1]).enumerate() {
            let maximum = cpu
                .iter()
                .zip(gpu)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(maximum <= 1, "frame={index}, maximum={maximum}");
        }
    }
}
