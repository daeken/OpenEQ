//! Proven EQG region AddAlpha_MaxCB1 surfaces: lit ONE/ONE RGB, alpha
//! cutoff, read-only opaque depth, and no fog. See EQG_ADDITIVE_SHADER.md.
use super::*;

pub(super) struct Additive {
    pipeline: wgpu::RenderPipeline,
}

impl Additive {
    pub(super) fn new(
        device: &wgpu::Device,
        geometry_layout: &wgpu::PipelineLayout,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("additive region surface lighting"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}\n{}",
                    include_str!("shaders/surface_lighting.wgsl"),
                    include_str!("shaders/forward_surface.wgsl"),
                    include_str!("shaders/additive.wgsl"),
                )
                .into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("additive region pipeline"),
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
                targets: &[Some(wgpu::ColorTargetState {
                    format: output_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        // The research establishes RGB, not destination alpha.
                        // Preserve the target's compositor alpha.
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
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
        Self { pipeline }
    }

    pub(super) fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        inputs: transparency::BlendInputs<'_>,
        profile: Option<&profiling::GpuProfiler>,
    ) {
        let transparency::BlendInputs {
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
                .any(|draw| draw.additive && draw.instance_count > 0)
        };
        if !visible(zone.0) && !actors.iter().any(|actor| visible(&actor.scene)) {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("additive region surfaces"),
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
            timestamp_writes: profile.and_then(|p| p.timestamps(profiling::Pass::Additive)),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_bind_group(1, lighting, &[]);
        pass.set_bind_group(2, zone.1, &[]);
        draw_additive(&mut pass, zone.0);
        for actor in actors {
            pass.set_bind_group(2, &actor.atlas, &[]);
            draw_additive(&mut pass, &actor.scene);
        }
    }
}

fn draw_additive(pass: &mut wgpu::RenderPass<'_>, scene: &GpuScene) {
    pass.set_vertex_buffer(0, scene.vertices.slice(..));
    pass.set_vertex_buffer(1, scene.instances.slice(..));
    pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
    for draw in scene.draws.iter().filter(|draw| draw.additive) {
        pass.draw_indexed(
            draw.index_start..draw.index_start + draw.index_count,
            draw.base_vertex,
            draw.instance_start..draw.instance_start + draw.instance_count,
        );
    }
}
