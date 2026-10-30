//! Draws a painting's layers under a light. Sticks to WebGPU's baseline limits and
//! features, so it runs the same on Metal, Vulkan and DX12.

use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use las_core::{MapKind, Mip, Project, load_mips};
use wgpu::util::DeviceExt;

pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Lit = 0,
    Normals = 1,
    Original = 2,
}

#[derive(Debug, Clone, Copy)]
pub struct Frame<'a> {
    pub transform: [f32; 4],
    pub light: [f32; 3],
    pub ambient: f32,
    pub strength: f32,
    pub sheen: f32,
    pub mode: Mode,
    pub linear: bool,
    pub background: [f64; 3],
    /// Per layer, back to front. Layers past the end are drawn.
    pub visible: &'a [bool],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    xf: [f32; 4],
    light: [f32; 4],
    p: [f32; 4],
    q: [f32; 4],
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    group0: wgpu::BindGroup,
    layer_layout: wgpu::BindGroupLayout,
    layers: Vec<wgpu::BindGroup>,
}

impl Renderer {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let entry = |binding, visibility, ty| wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        let uniform = wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        };
        let texture = wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        };
        let both = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let frag = wgpu::ShaderStages::FRAGMENT;
        let global_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[
                entry(0, both, uniform),
                entry(1, frag, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)),
            ],
        });
        let layer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("layer"),
            entries: &[entry(0, both, uniform), entry(1, frag, texture), entry(2, frag, texture)],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("layers"),
            bind_group_layouts: &[Some(&global_layout), Some(&layer_layout)],
            immediate_size: 0,
        });
        let over = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("layers"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: TARGET_FORMAT,
                    blend: Some(wgpu::BlendState { color: over, alpha: over }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let group0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &global_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        Self { pipeline, globals, group0, layer_layout, layers: Vec::new() }
    }

    /// Replace the loaded painting.
    pub fn load(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, project: &Project) -> Result<()> {
        let mut layers = Vec::with_capacity(project.layers.len());
        for layer in &project.layers {
            let color = upload(device, queue, &load_mips(&project.dir.join(&layer.color), MapKind::Color)?);
            let normal = upload(device, queue, &load_mips(&project.dir.join(&layer.normal), MapKind::Normal)?);
            let rect = [layer.x, layer.y, layer.w as f32, layer.h as f32];
            let quad = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("quad"),
                contents: bytemuck::cast_slice(&rect),
                usage: wgpu::BufferUsages::UNIFORM,
            });
            layers.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&layer.name),
                layout: &self.layer_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: quad.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&color) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&normal) },
                ],
            }));
        }
        self.layers = layers;
        Ok(())
    }

    pub fn render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        frame: &Frame,
    ) {
        let [lx, ly, lz] = frame.light;
        let globals = Globals {
            xf: frame.transform,
            light: [lx, ly, lz, 0.0],
            p: [frame.ambient, frame.strength, frame.sheen, frame.mode as u32 as f32],
            q: [frame.linear as u32 as f32, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let [r, g, b] = frame.background;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("layers"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.group0, &[]);
        for (i, layer) in self.layers.iter().enumerate() {
            if frame.visible.get(i).copied().unwrap_or(true) {
                pass.set_bind_group(1, layer, &[]);
                pass.draw(0..6, 0..1);
            }
        }
    }
}

fn upload(device: &wgpu::Device, queue: &wgpu::Queue, mips: &[Mip]) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: mips[0].width, height: mips[0].height, depth_or_array_layers: 1 },
        mip_level_count: mips.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, mip) in mips.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &mip.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(mip.width * 4), rows_per_image: None },
            wgpu::Extent3d { width: mip.width, height: mip.height, depth_or_array_layers: 1 },
        );
    }
    texture.create_view(&Default::default())
}
