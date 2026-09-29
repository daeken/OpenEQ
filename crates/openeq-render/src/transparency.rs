//! Weighted blended order-independent transparency for fractional-alpha
//! surfaces. Fully opaque interiors stay in the G-buffer; water keeps its
//! authored shader. Accumulation is linear HDR, resolved before the UI.
use super::*;

const ACCUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const REVEAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

struct BlendTargets {
    accumulation: wgpu::TextureView,
    revealage: wgpu::TextureView,
    group: wgpu::BindGroup,
}

impl BlendTargets {
    fn new(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, width: u32, height: u32) -> Self {
        let make = |label, format| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: width.max(1),
                        height: height.max(1),
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let accumulation = make("transparent accumulation", ACCUM_FORMAT);
        let revealage = make("transparent revealage", REVEAL_FORMAT);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("transparency resolve"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&accumulation),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&revealage),
                },
            ],
        });
        Self {
            accumulation,
            revealage,
            group,
        }
    }
}

pub(super) struct Transparency {
    accumulate: wgpu::RenderPipeline,
    resolve: wgpu::RenderPipeline,
    resolve_layout: wgpu::BindGroupLayout,
    targets: BlendTargets,
}

impl Transparency {
    pub(super) fn new(
        device: &wgpu::Device,
        geometry_layout: &wgpu::PipelineLayout,
        output_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transparent surface lighting"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}",
                    include_str!("shaders/surface_lighting.wgsl"),
                    include_str!("shaders/transparency.wgsl")
                )
                .into(),
            ),
        });
        let additive = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let reveal = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        };
        let accumulate = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("transparent accumulation pipeline"),
            layout: Some(geometry_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &vertex_layouts(),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: ACCUM_FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: additive,
                            alpha: additive,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: REVEAL_FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: reveal,
                            alpha: reveal,
                        }),
                        write_mask: wgpu::ColorWrites::RED,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
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
        });
        let resolve_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("transparency resolve layout"),
            entries: &[0, 1].map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("transparency resolve pipeline layout"),
            bind_group_layouts: &[Some(&resolve_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("transparency resolve shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/transparency_resolve.wgsl").into(),
            ),
        });
        let resolve = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("transparency resolve pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let targets = BlendTargets::new(device, &resolve_layout, width, height);
        Self {
            accumulate,
            resolve,
            resolve_layout,
            targets,
        }
    }

    pub(super) fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.targets = BlendTargets::new(device, &self.resolve_layout, width, height);
    }

    pub(super) fn render(&self, encoder: &mut wgpu::CommandEncoder, inputs: BlendInputs<'_>) {
        let BlendInputs {
            depth,
            output,
            globals,
            lighting,
            zone,
            actors,
        } = inputs;
        let visible = |scene: &GpuScene| {
            scene
                .draws
                .iter()
                .any(|d| d.transparent && d.instance_count > 0)
        };
        if !visible(zone.0) && !actors.iter().any(|actor| visible(&actor.scene)) {
            return;
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("transparent accumulation"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.accumulation,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.revealage,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: None,
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.accumulate);
            pass.set_bind_group(0, globals, &[]);
            pass.set_bind_group(1, lighting, &[]);
            pass.set_bind_group(2, zone.1, &[]);
            draw_transparent(&mut pass, zone.0);
            for actor in actors {
                pass.set_bind_group(2, &actor.atlas, &[]);
                draw_transparent(&mut pass, &actor.scene);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("transparent resolve"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.resolve);
            pass.set_bind_group(0, &self.targets.group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

pub(super) struct BlendInputs<'a> {
    pub depth: &'a wgpu::TextureView,
    pub output: &'a wgpu::TextureView,
    pub globals: &'a wgpu::BindGroup,
    pub lighting: &'a wgpu::BindGroup,
    pub zone: (&'a GpuScene, &'a wgpu::BindGroup),
    pub actors: &'a [&'a GpuActor],
}

fn draw_transparent(pass: &mut wgpu::RenderPass<'_>, scene: &GpuScene) {
    pass.set_vertex_buffer(0, scene.vertices.slice(..));
    pass.set_vertex_buffer(1, scene.instances.slice(..));
    pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
    for draw in scene.draws.iter().filter(|draw| draw.transparent) {
        pass.draw_indexed(
            draw.index_start..draw.index_start + draw.index_count,
            draw.base_vertex,
            draw.instance_start..draw.instance_start + draw.instance_count,
        );
    }
}
