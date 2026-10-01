//! Opaque TER MaxLava: independently scrolling lit top and luminous bottom.
//! Native bump/base/point-light parity is separate from current lighting/fog.
use super::*;

pub(super) struct Lava {
    pipeline: wgpu::RenderPipeline,
}

impl Lava {
    pub(super) fn new(
        device: &wgpu::Device,
        geometry_layout: &wgpu::PipelineLayout,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lava region surface lighting"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}\n{}",
                    include_str!("shaders/surface_lighting.wgsl"),
                    include_str!("shaders/forward_surface.wgsl"),
                    include_str!("shaders/lava.wgsl"),
                )
                .into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lava region pipeline"),
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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
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
                .any(|draw| draw.lava && draw.index_count > 0 && draw.instance_count > 0)
        };
        if !visible(zone.0) && !actors.iter().any(|actor| visible(&actor.scene)) {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("lava region surfaces"),
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
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: profile.and_then(|p| p.timestamps(profiling::Pass::Lava)),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_bind_group(1, lighting, &[]);
        pass.set_bind_group(2, zone.1, &[]);
        draw_lava(&mut pass, zone.0);
        for actor in actors {
            pass.set_bind_group(2, &actor.atlas, &[]);
            draw_lava(&mut pass, &actor.scene);
        }
    }
}

fn draw_lava(pass: &mut wgpu::RenderPass<'_>, scene: &GpuScene) {
    pass.set_vertex_buffer(0, scene.vertices.slice(..));
    pass.set_vertex_buffer(1, scene.instances.slice(..));
    pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
    for draw in scene
        .draws
        .iter()
        .filter(|draw| draw.lava && draw.index_count > 0 && draw.instance_count > 0)
    {
        pass.draw_indexed(
            draw.index_start..draw.index_start + draw.index_count,
            draw.base_vertex,
            draw.instance_start..draw.instance_start + draw.instance_count,
        );
    }
}
