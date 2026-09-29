//! Bounded, camera-facing spell billboards using original decoded textures.
use std::sync::Arc;

use crate::{Camera, DEPTH_FORMAT};
use bytemuck::{Pod, Zeroable};
use openeq_assets::texture::Texture;

/// Hard cap on live GPU instances. Additional submitted particles are dropped.
pub const MAX_PARTICLES: usize = 8192;
/// Two arrays support this many slots even with the standard 256-layer limit.
pub const MAX_PARTICLE_TEXTURES: usize = 512;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParticleBlend {
    #[default]
    Alpha,
    Additive,
}

#[derive(Debug, Clone, Copy)]
pub struct ParticleInstance {
    /// Center in EverQuest coordinates (Z up).
    pub position: [f32; 3],
    /// Full billboard width and height in world units.
    pub size: [f32; 2],
    /// Counterclockwise billboard roll, in radians.
    pub rotation: f32,
    /// Linear RGB tint and straight alpha, each in [0,1].
    pub color: [f32; 4],
    /// Index into ParticleFrame::textures.
    pub texture: u32,
    /// Normalized [left, top, right, bottom] in the original texture, supporting
    /// authored flipbook frames without uploading one texture per frame.
    pub uv_rect: [f32; 4],
    pub blend: ParticleBlend,
}

impl Default for ParticleInstance {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            size: [1.; 2],
            rotation: 0.,
            color: [1.; 4],
            texture: 0,
            uv_rect: [0., 0., 1., 1.],
            blend: ParticleBlend::Alpha,
        }
    }
}

/// Reuse the same Arc while its texture set is unchanged. Replacing it uploads
/// a new bounded atlas; the renderer releases its previous atlas immediately.
#[derive(Debug, Clone, Default)]
pub struct ParticleFrame {
    pub textures: Arc<Vec<Texture>>,
    pub instances: Vec<ParticleInstance>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParticleStats {
    pub submitted: usize,
    pub rendered: usize,
    pub invalid: usize,
    pub missing_texture: usize,
    pub over_capacity: usize,
}

const ATLAS_SIZE: u32 = 256;
const LAYERS_PER_ATLAS: usize = MAX_PARTICLE_TEXTURES / 2;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParticle {
    position: [f32; 3],
    rotation: f32,
    size: [f32; 2],
    texture: u32,
    blend: u32,
    color: [f32; 4],
    uv_rect: [f32; 4],
}

struct Atlas {
    _textures: [wgpu::Texture; 2],
    group: wgpu::BindGroup,
    valid: Vec<bool>,
}

pub(crate) struct ParticleRenderer {
    alpha: wgpu::RenderPipeline,
    additive: wgpu::RenderPipeline,
    atlas_layout: wgpu::BindGroupLayout,
    atlas: Option<Atlas>,
    source: Option<Arc<Vec<Texture>>>,
    instances: Vec<ParticleInstance>,
    staging: Vec<GpuParticle>,
    buffer: wgpu::Buffer,
    alpha_count: u32,
    stats: ParticleStats,
}

impl ParticleRenderer {
    pub(crate) fn new(
        device: &wgpu::Device,
        globals: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Self {
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("particle pipeline layout"),
            bind_group_layouts: &[Some(globals), Some(&atlas_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("spell billboards"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particles.wgsl").into()),
        });
        let make_pipeline = |label, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[particle_layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let alpha = make_pipeline(
            "alpha spell billboards",
            wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING,
        );
        let additive = make_pipeline(
            "additive spell billboards",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded spell instances"),
            size: (MAX_PARTICLES * std::mem::size_of::<GpuParticle>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            alpha,
            additive,
            atlas_layout,
            atlas: None,
            source: None,
            instances: Vec::new(),
            staging: Vec::new(),
            buffer,
            alpha_count: 0,
            stats: ParticleStats::default(),
        }
    }

    pub(crate) fn set_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame: &ParticleFrame,
    ) -> ParticleStats {
        if !self
            .source
            .as_ref()
            .is_some_and(|source| Arc::ptr_eq(source, &frame.textures))
        {
            self.atlas = build_atlas(device, queue, &self.atlas_layout, &frame.textures);
            self.source = Some(frame.textures.clone());
        }
        self.instances.clear();
        self.stats = ParticleStats {
            submitted: frame.instances.len(),
            over_capacity: frame.instances.len().saturating_sub(MAX_PARTICLES),
            ..Default::default()
        };
        for source in frame.instances.iter().take(MAX_PARTICLES) {
            if !valid(source) {
                self.stats.invalid += 1;
            } else if !self
                .atlas
                .as_ref()
                .and_then(|atlas| atlas.valid.get(source.texture as usize))
                .copied()
                .unwrap_or(false)
            {
                self.stats.missing_texture += 1;
            } else {
                let mut particle = *source;
                particle.color = particle.color.map(|value| value.clamp(0., 1.));
                particle.rotation = particle.rotation.rem_euclid(std::f32::consts::TAU);
                self.instances.push(particle);
            }
        }
        self.stats.rendered = self.instances.len();
        self.stats
    }

    pub(crate) fn clear(&mut self) {
        self.instances.clear();
        self.staging.clear();
        self.alpha_count = 0;
        self.stats = ParticleStats::default();
    }

    pub(crate) fn stats(&self) -> ParticleStats {
        self.stats
    }

    pub(crate) fn prepare(&mut self, queue: &wgpu::Queue, camera: &Camera) {
        let eye = Camera::to_world(camera.position);
        let forward = camera.forward();
        // Exact back-to-front ordering for ordinary alpha particles. Additive
        // particles commute and form a second draw without per-particle calls.
        self.instances.sort_by(|a, b| {
            let additive_a = a.blend == ParticleBlend::Additive;
            let additive_b = b.blend == ParticleBlend::Additive;
            additive_a.cmp(&additive_b).then_with(|| {
                if additive_a {
                    std::cmp::Ordering::Equal
                } else {
                    let depth =
                        |p: &ParticleInstance| (Camera::to_world(p.position) - eye).dot(forward);
                    depth(b).total_cmp(&depth(a))
                }
            })
        });
        self.alpha_count = self
            .instances
            .iter()
            .take_while(|p| p.blend == ParticleBlend::Alpha)
            .count() as u32;
        self.staging.clear();
        self.staging
            .extend(self.instances.iter().map(|p| GpuParticle {
                position: p.position,
                rotation: p.rotation,
                size: p.size,
                texture: p.texture,
                blend: u32::from(p.blend == ParticleBlend::Additive),
                color: p.color,
                uv_rect: p.uv_rect,
            }));
        if !self.staging.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.staging));
        }
    }

    pub(crate) fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        globals: &wgpu::BindGroup,
    ) {
        let Some(atlas) = &self.atlas else {
            return;
        };
        if self.staging.is_empty() {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("spell particles"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, globals, &[]);
        pass.set_bind_group(1, &atlas.group, &[]);
        pass.set_vertex_buffer(0, self.buffer.slice(..));
        if self.alpha_count > 0 {
            pass.set_pipeline(&self.alpha);
            pass.draw(0..6, 0..self.alpha_count);
        }
        let count = self.staging.len() as u32;
        if self.alpha_count < count {
            pass.set_pipeline(&self.additive);
            pass.draw(0..6, self.alpha_count..count);
        }
    }
}

fn valid(particle: &ParticleInstance) -> bool {
    particle
        .position
        .iter()
        .chain(&particle.size)
        .chain(&particle.color)
        .chain(&particle.uv_rect)
        .all(|x| x.is_finite())
        && particle.rotation.is_finite()
        && particle.size.iter().all(|x| *x > 0.)
        && particle.uv_rect.iter().all(|x| (0. ..=1.).contains(x))
        && particle.uv_rect[0] < particle.uv_rect[2]
        && particle.uv_rect[1] < particle.uv_rect[3]
}

fn particle_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        0 => Float32x3, 1 => Float32, 2 => Float32x2, 3 => Uint32,
        4 => Uint32, 5 => Float32x4, 6 => Float32x4,
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<GpuParticle>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}

fn build_atlas(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
    sources: &[Texture],
) -> Option<Atlas> {
    let count = sources.len().min(MAX_PARTICLE_TEXTURES);
    if count == 0 {
        return None;
    }
    // WebGPU's baseline 256 layers must cover every slot in the catalog, not
    // silently discard the alphabetically later textures. Both arrays stay
    // bound for the whole pass, preserving particle order and two draw calls.
    let textures = std::array::from_fn(|array: usize| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("original spell texture atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: count
                    .saturating_sub(array * LAYERS_PER_ATLAS)
                    .clamp(1, LAYERS_PER_ATLAS) as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    });
    let mut valid = vec![false; count];
    for (layer, source) in sources.iter().take(count).enumerate() {
        let expected = u64::from(source.width) * u64::from(source.height) * 4;
        if source.width == 0
            || source.height == 0
            || source.width > 4096
            || source.height > 4096
            || expected != source.rgba.len() as u64
        {
            continue;
        }
        let Some(image) =
            image::RgbaImage::from_raw(source.width, source.height, source.rgba.clone())
        else {
            continue;
        };
        let pixels = image::imageops::resize(
            &image,
            ATLAS_SIZE,
            ATLAS_SIZE,
            image::imageops::FilterType::Triangle,
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &textures[layer / LAYERS_PER_ATLAS],
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: (layer % LAYERS_PER_ATLAS) as u32,
                },
                aspect: wgpu::TextureAspect::All,
            },
            pixels.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_SIZE * 4),
                rows_per_image: Some(ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );
        valid[layer] = true;
    }
    let views = textures.each_ref().map(|texture| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("spell texture sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("spell texture atlas group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&views[0]),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&views[1]),
            },
        ],
    });
    Some(Atlas {
        _textures: textures,
        group,
        valid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_512_textures_fit_the_baseline_256_layer_device_limit() {
        let instance = wgpu::Instance::default();
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            return;
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .unwrap();
        assert_eq!(device.limits().max_texture_array_layers, 256);
        let globals = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("particle test globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let mut renderer = ParticleRenderer::new(&device, &globals, crate::ALBEDO_FORMAT);
        let frame = ParticleFrame {
            textures: Arc::new(
                (0..MAX_PARTICLE_TEXTURES)
                    .map(|i| Texture {
                        name: format!("slot{i}"),
                        width: 1,
                        height: 1,
                        rgba: vec![255; 4],
                    })
                    .collect(),
            ),
            instances: (0..MAX_PARTICLE_TEXTURES)
                .map(|i| ParticleInstance {
                    texture: i as u32,
                    ..Default::default()
                })
                .collect(),
        };
        let stats = renderer.set_frame(&device, &queue, &frame);
        assert_eq!(stats.rendered, MAX_PARTICLE_TEXTURES);
        assert_eq!(stats.missing_texture, 0);
    }
}
