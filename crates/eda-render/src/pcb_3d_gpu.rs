//! Resident triangle buffers, hardware depth and back-to-front transparency.
use super::*;
use egui_wgpu::{Callback, CallbackResources, CallbackTrait, ScreenDescriptor, wgpu};
use wgpu::util::DeviceExt;

/// Register the actual device limits. Eframe's OpenGL fallback requests no
/// vertex storage buffers; keep its software path instead of a validation panic.
pub fn enable_gpu(ctx: &egui::Context, device: &wgpu::Device) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("pcb-3d-gpu-limits"), device.limits()));
}
pub(super) fn supported(ctx: &egui::Context, scene: &Scene) -> bool {
    ctx.data(|data| data.get_temp::<wgpu::Limits>(egui::Id::new("pcb-3d-gpu-limits")))
        .is_some_and(|limits| {
            limits.max_storage_buffers_per_shader_stage > 0
                && scene.geometry.vertices.len() as u64 * 16 <= limits.max_buffer_size
                && scene.geometry.styles.len().max(4) as u64 * 4
                    <= u64::from(limits.max_storage_buffer_binding_size)
        })
}

#[derive(Default)]
pub(super) struct Geometry {
    vertices: Vec<[u32; 4]>,
    pub styles: Vec<usize>,
    face_styles: Vec<usize>,
}
impl Geometry {
    pub fn new(scene: &Scene) -> Self {
        let mut result = Self::default();
        let mut styles = std::collections::HashMap::new();
        result.vertices.reserve(scene.faces.len() * 3);
        for (index, face) in scene.faces.iter().enumerate() {
            let key = (face.material, face.layer, face.triangle.color, face.owner);
            let style = *styles.entry(key).or_insert_with(|| {
                let n = result.styles.len();
                result.styles.push(index);
                n
            });
            result.face_styles.push(style);
            for p in face.triangle.v {
                result.vertices.push([
                    p[0].to_bits(),
                    p[1].to_bits(),
                    p[2].to_bits(),
                    style as u32,
                ]);
            }
        }
        result
    }
}
#[derive(Default)]
pub(super) struct Renderer {
    buffers: Mutex<Option<Buffers>>,
}
struct Target {
    size: [usize; 2],
    color: wgpu::TextureView,
    depth: wgpu::TextureView,
    binding: wgpu::BindGroup,
    #[cfg(test)]
    texture: wgpu::Texture,
}
struct Buffers {
    scene: Arc<Scene>,
    vertices: wgpu::Buffer,
    colors: wgpu::Buffer,
    uniform: wgpu::Buffer,
    binding: wgpu::BindGroup,
    opaque: wgpu::Buffer,
    opaque_count: u32,
    transparent: wgpu::Buffer,
    transparent_faces: Vec<(usize, [f32; 4])>,
    transparent_count: u32,
    order: Vec<(usize, u32)>,
    sort_scratch: Vec<(usize, u32)>,
    indices: Vec<u32>,
    style: Option<u64>,
    rotation: Option<[u32; 3]>,
    rendered: Option<u64>,
    solid_pipeline: wgpu::RenderPipeline,
    transparent_pipeline: wgpu::RenderPipeline,
    background_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    image_layout: wgpu::BindGroupLayout,
    target: Option<Target>,
    format: wgpu::TextureFormat,
}
fn blend() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    }
}
impl Buffers {
    fn new(scene: Arc<Scene>, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let geometry = &scene.geometry;
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("PCB 3D positions"),
            contents: bytemuck::cast_slice(&geometry.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let buffer = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: size.max(16),
                usage,
                mapped_at_creation: false,
            })
        };
        let colors = buffer(
            "PCB 3D materials",
            geometry.styles.len() as u64 * 4,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        );
        let uniform = buffer(
            "PCB 3D camera",
            128,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let indices_size = geometry.vertices.len() as u64 * 4;
        let opaque = buffer(
            "PCB 3D opaque indices",
            indices_size,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );
        let transparent = buffer(
            "PCB 3D transparent indices",
            indices_size,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PCB 3D scene layout"),
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
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PCB 3D scene"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: colors.as_entire_binding(),
                },
            ],
        });
        let image_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PCB 3D image layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("PCB 3D"),
            source: wgpu::ShaderSource::Wgsl(include_str!("pcb_3d_gpu.wgsl").into()),
        });
        let scene_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let quad_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&layout, &image_layout],
            push_constant_ranges: &[],
        });
        let pipeline = |vertex,
                        fragment,
                        buffer_layout: &[wgpu::VertexBufferLayout<'_>],
                        pipeline_layout: &wgpu::PipelineLayout,
                        target_format,
                        depth: Option<bool>,
                        blending| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fragment),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    buffers: buffer_layout,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: blending,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: depth.map(|write| wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: write,
                    depth_compare: if vertex == "quad" {
                        wgpu::CompareFunction::Always
                    } else {
                        wgpu::CompareFunction::Greater
                    },
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let attributes = wgpu::vertex_attr_array![0=>Float32x3,1=>Uint32];
        let vertex_layout = [wgpu::VertexBufferLayout {
            array_stride: 16,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &attributes,
        }];
        let solid_pipeline = pipeline(
            "vertex",
            "fragment",
            &vertex_layout,
            &scene_layout,
            wgpu::TextureFormat::Rgba8Unorm,
            Some(true),
            None,
        );
        let transparent_pipeline = pipeline(
            "vertex",
            "fragment",
            &vertex_layout,
            &scene_layout,
            wgpu::TextureFormat::Rgba8Unorm,
            Some(false),
            Some(blend()),
        );
        let background_pipeline = pipeline(
            "quad",
            "background",
            &[],
            &scene_layout,
            wgpu::TextureFormat::Rgba8Unorm,
            Some(false),
            None,
        );
        let image_pipeline = pipeline(
            "quad",
            if format.is_srgb() {
                "linear_image"
            } else {
                "gamma_image"
            },
            &[],
            &quad_layout,
            format,
            None,
            None,
        );
        Self {
            scene,
            vertices,
            colors,
            uniform,
            binding,
            opaque,
            opaque_count: 0,
            transparent,
            transparent_faces: vec![],
            transparent_count: 0,
            order: vec![],
            sort_scratch: vec![],
            indices: vec![],
            style: None,
            rotation: None,
            rendered: None,
            solid_pipeline,
            transparent_pipeline,
            background_pipeline,
            image_pipeline,
            image_layout,
            target: None,
            format,
        }
    }
    fn target(&mut self, device: &wgpu::Device, size: [usize; 2]) {
        if self.target.as_ref().is_some_and(|t| t.size == size) {
            return;
        }
        let texture = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0] as u32,
                    height: size[1] as u32,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color_texture = texture(
            "PCB 3D color",
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
        );
        let color = color_texture.create_view(&Default::default());
        let depth = texture(
            "PCB 3D depth",
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        )
        .create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let binding = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("PCB 3D image"),
            layout: &self.image_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        self.target = Some(Target {
            size,
            color,
            depth,
            binding,
            #[cfg(test)]
            texture: color_texture,
        });
        self.rendered = None;
    }
    fn styles(&mut self, state: &State, selected: &[Uuid], key: u64, queue: &wgpu::Queue) {
        if self.style == Some(key) {
            return;
        }
        let selected: std::collections::HashSet<_> = selected.iter().copied().collect();
        let colors: Vec<_> = self
            .scene
            .geometry
            .styles
            .iter()
            .map(|&index| {
                let face = &self.scene.faces[index];
                let mut color = material_color(face, state);
                if color.a() > 0
                    && face.owner.is_some_and(|id| {
                        selected.contains(&id)
                            || self
                                .scene
                                .parents
                                .get(&id)
                                .is_some_and(|p| selected.contains(p))
                    })
                {
                    color = state.presentation.selection_color;
                }
                color
            })
            .collect();
        let packed: Vec<_> = colors
            .iter()
            .map(|c| u32::from_le_bytes(c.to_array()))
            .collect();
        if !packed.is_empty() {
            queue.write_buffer(&self.colors, 0, bytemuck::cast_slice(&packed));
        }
        self.indices.clear();
        self.transparent_faces.clear();
        for (index, &style) in self.scene.geometry.face_styles.iter().enumerate() {
            match colors[style].a() {
                0 => {}
                255 => self
                    .indices
                    .extend((index as u32 * 3)..(index as u32 * 3 + 3)),
                _ => {
                    let v = self.scene.faces[index].triangle.v;
                    self.transparent_faces.push((
                        index,
                        [
                            v.iter().map(|p| p[0]).sum(),
                            v.iter().map(|p| p[1]).sum(),
                            v.iter().map(|p| p[2]).sum(),
                            v.iter()
                                .map(|p| p[2].clamp(-self.scene.thickness, 0.))
                                .sum(),
                        ],
                    ));
                }
            }
        }
        self.opaque_count = self.indices.len() as u32;
        self.transparent_count = self.transparent_faces.len() as u32 * 3;
        if !self.indices.is_empty() {
            queue.write_buffer(&self.opaque, 0, bytemuck::cast_slice(&self.indices));
        }
        self.style = Some(key);
        self.rotation = None;
        self.rendered = None;
        if std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some() {
            eprintln!(
                "pcb_3d_gpu triangles={} opaque={} transparent={}",
                self.scene.faces.len(),
                self.opaque_count / 3,
                self.transparent_count / 3
            );
        }
    }
    fn order(&mut self, state: &State, queue: &wgpu::Queue) {
        let rotation = [
            state.yaw.to_bits(),
            state.pitch.to_bits(),
            state.presentation.thickness_scale.to_bits(),
        ];
        if self.rotation == Some(rotation) {
            return;
        }
        let (sy, cy) = state.yaw.sin_cos();
        let (sp, cp) = state.pitch.sin_cos();
        self.order.clear();
        for &(index, p) in &self.transparent_faces {
            let depth = -(p[0] * sy + p[1] * cy) * cp
                + (p[2] + p[3] * (state.presentation.thickness_scale - 1.)) * sp;
            let bits = depth.to_bits();
            let key = if bits & 0x8000_0000 == 0 {
                bits ^ 0x8000_0000
            } else {
                !bits
            };
            self.order.push((index, key));
        }
        radix_sort(&mut self.order, &mut self.sort_scratch);
        self.indices.clear();
        for &(index, _) in &self.order {
            self.indices
                .extend((index as u32 * 3)..(index as u32 * 3 + 3));
        }
        if !self.indices.is_empty() {
            queue.write_buffer(&self.transparent, 0, bytemuck::cast_slice(&self.indices));
        }
        self.rotation = Some(rotation);
    }
    fn render(
        &mut self,
        state: &State,
        logical: Vec2,
        key: u64,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        if self.rendered == Some(key) {
            return;
        }
        let centre = state.focus.unwrap_or(state.scene.centre);
        let (sy, cy) = state.yaw.sin_cos();
        let (sp, cp) = state.pitch.sin_cos();
        let mut uniform = [0f32; 32];
        uniform[..20].copy_from_slice(&[
            centre[0],
            centre[1],
            centre[2],
            state.scene.thickness,
            sy,
            cy,
            sp,
            cp,
            logical.x,
            logical.y,
            logical.min_elem() * 0.43 / state.scene.radius.max(0.1) * state.zoom,
            state.scene.radius.max(0.1) * 4.,
            state.pan.x,
            state.pan.y,
            state.presentation.thickness_scale,
            if state.presentation.perspective {
                1.
            } else {
                0.
            },
            (state.scene.radius.max(0.1) * (state.presentation.thickness_scale + 2.)
                + distance(centre, state.scene.centre))
                * 2.,
            0.,
            0.,
            0.,
        ]);
        for (i, color) in state.presentation.background.iter().enumerate() {
            for (k, v) in color.to_array().into_iter().enumerate() {
                uniform[20 + i * 4 + k] = v as f32 / 255.;
            }
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));
        let target = self.target.as_ref().unwrap();
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("PCB 3D"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.color,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &self.binding, &[]);
        pass.set_pipeline(&self.background_pipeline);
        pass.draw(0..3, 0..1);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        if self.opaque_count > 0 {
            pass.set_pipeline(&self.solid_pipeline);
            pass.set_index_buffer(self.opaque.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.opaque_count, 0, 0..1);
        }
        if self.transparent_count > 0 {
            pass.set_pipeline(&self.transparent_pipeline);
            pass.set_index_buffer(self.transparent.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.transparent_count, 0, 0..1);
        }
        self.rendered = Some(key);
    }
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.into_iter()
        .zip(b)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
}
// Stable numeric depth order in four linear passes. Equal depths retain source
// triangle order, as in the software renderer; camera translation needs no sort.
fn radix_sort(order: &mut Vec<(usize, u32)>, scratch: &mut Vec<(usize, u32)>) {
    scratch.resize(order.len(), (0, 0));
    for shift in [0, 8, 16, 24] {
        let mut offsets = [0usize; 256];
        for &(_, key) in order.iter() {
            offsets[((key >> shift) & 255) as usize] += 1;
        }
        let mut start = 0;
        for count in &mut offsets {
            let n = *count;
            *count = start;
            start += n;
        }
        for &entry in order.iter() {
            let slot = &mut offsets[((entry.1 >> shift) & 255) as usize];
            scratch[*slot] = entry;
            *slot += 1;
        }
        std::mem::swap(order, scratch);
    }
}
impl Renderer {
    pub fn callback(
        self: &Arc<Self>,
        state: &State,
        selected: &[Uuid],
        rect: Rect,
        size: [usize; 2],
        keys: [u64; 2],
        format: wgpu::TextureFormat,
    ) -> egui::Shape {
        egui::Shape::Callback(Callback::new_paint_callback(
            rect,
            Draw {
                renderer: self.clone(),
                state: state.clone(),
                selected: selected.to_vec(),
                rect,
                size,
                key: keys[0],
                style: keys[1],
                format,
            },
        ))
    }
}
struct Draw {
    renderer: Arc<Renderer>,
    state: State,
    selected: Vec<Uuid>,
    rect: Rect,
    size: [usize; 2],
    key: u64,
    style: u64,
    format: wgpu::TextureFormat,
}
impl CallbackTrait for Draw {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _: &ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        _: &mut CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let start = std::time::Instant::now();
        let mut buffers = self.renderer.buffers.lock().unwrap();
        if buffers
            .as_ref()
            .is_none_or(|b| !Arc::ptr_eq(&b.scene, &self.state.scene) || b.format != self.format)
        {
            *buffers = Some(Buffers::new(self.state.scene.clone(), device, self.format));
        }
        let b = buffers.as_mut().unwrap();
        let changed = b.rendered != Some(self.key);
        b.target(device, self.size);
        b.styles(&self.state, &self.selected, self.style, queue);
        b.order(&self.state, queue);
        b.render(&self.state, self.rect.size(), self.key, queue, encoder);
        if changed
            && std::env::var_os("PCBCRAFT_FRAME_PROFILE").is_some()
            && start.elapsed().as_millis() > 16
        {
            eprintln!(
                "pcb_3d_gpu_prepare cpu_ms={:.2}",
                start.elapsed().as_secs_f64() * 1000.
            );
        }
        vec![]
    }
    fn paint(
        &self,
        _: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        _: &CallbackResources,
    ) {
        let buffers = self.renderer.buffers.lock().unwrap();
        let Some(b) = buffers.as_ref() else { return };
        let Some(target) = &b.target else { return };
        pass.set_pipeline(&b.image_pipeline);
        pass.set_bind_group(0, &b.binding, &[]);
        pass.set_bind_group(1, &target.binding, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read_image(device: &wgpu::Device, queue: &wgpu::Queue, buffers: &Buffers) -> Vec<u8> {
        let target = buffers.target.as_ref().unwrap();
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 64 * 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            target.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            target.texture.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, receive) = mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::Wait).unwrap();
        receive.recv().unwrap().unwrap();
        readback.slice(..).get_mapped_range().to_vec()
    }
    #[test]
    fn gpu_depth_transparency_selection_and_camera_match_software_and_reuse_vertices() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("No graphics adapter; 3D image comparison skipped");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let owner = Uuid::new_v4();
        let mut scene = Scene {
            radius: 2.,
            thickness: 1.,
            ..Default::default()
        };
        for (z, color, material, id) in [
            (-0.8, Color32::BLUE, 8, None),
            (0., Color32::RED, 0, Some(owner)),
        ] {
            for v in [
                [[-1., -1., z], [1., -1., z], [1., 1., z]],
                [[-1., -1., z], [1., 1., z], [-1., 1., z]],
            ] {
                scene.faces.push(Face {
                    triangle: Triangle { v, color },
                    owner: id,
                    material,
                    layer: TOP,
                });
            }
        }
        scene.geometry = Geometry::new(&scene);
        let mut state = State {
            scene: Arc::new(scene),
            ..Default::default()
        };
        for format in [
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ] {
            let mut buffers = Buffers::new(state.scene.clone(), &device, format);
            buffers.target(&device, [64, 64]);
            let vertices = &buffers.vertices as *const wgpu::Buffer as usize;
            let mut key = 0;
            for perspective in [false, true] {
                for bottom in [false, true] {
                    for opacity in [0, 128, 255] {
                        for selected in [false, true] {
                            key += 1;
                            if bottom {
                                state.bottom();
                            } else {
                                state.top();
                            }
                            state.yaw += 0.3;
                            state.pitch *= 0.8;
                            state.pan = Vec2::new(1.5, -2.5);
                            state.zoom = 1.1;
                            state.presentation.perspective = perspective;
                            state.presentation.opacity[0] = opacity;
                            state.presentation.thickness_scale = 2.;
                            let selected = if selected { vec![owner] } else { vec![] };
                            buffers.styles(&state, &selected, key, &queue);
                            buffers.order(&state, &queue);
                            let mut encoder = device.create_command_encoder(&Default::default());
                            buffers.render(&state, Vec2::splat(64.), key, &queue, &mut encoder);
                            queue.submit([encoder.finish()]);
                            let actual = read_image(&device, &queue, &buffers);
                            let (expected, _) =
                                raster(&state, &selected, Vec2::splat(64.), [64, 64]);
                            // Sample interior pixels; hardware's subpixel edge coverage is
                            // allowed to differ from the software edge test at boundaries.
                            for y in (26..38).step_by(3) {
                                for x in (26..38).step_by(3) {
                                    let index = y * 64 + x;
                                    let maximum = actual[index * 4..index * 4 + 4]
                                        .iter()
                                        .zip(expected.pixels[index].to_array())
                                        .map(|(a, b)| a.abs_diff(b))
                                        .max()
                                        .unwrap();
                                    assert!(
                                        maximum <= 3,
                                        "max={maximum} pixel={x},{y} perspective={perspective} bottom={bottom} opacity={opacity} selected={selected:?}"
                                    );
                                }
                            }
                            assert_eq!(&buffers.vertices as *const wgpu::Buffer as usize, vertices);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn radix_depth_sort_matches_stable_numeric_order_and_ties() {
        let mut order: Vec<_> = (0..10000)
            .map(|n| {
                let v = ((n * 977) % 137) as f32 - 68.;
                let bits = v.to_bits();
                (
                    n,
                    if bits & 0x8000_0000 == 0 {
                        bits ^ 0x8000_0000
                    } else {
                        !bits
                    },
                )
            })
            .collect();
        let mut expected = order.clone();
        expected.sort_by_key(|v| v.1);
        radix_sort(&mut order, &mut vec![]);
        assert_eq!(order, expected);
    }
}
