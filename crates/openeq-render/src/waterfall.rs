//! TER waterfalls: independent scrolling color and alpha, source-alpha blend,
//! cutoff and read-only opaque depth. Lighting/fog use the current renderer.
use super::*;

pub(super) struct Waterfall {
    pipeline: wgpu::RenderPipeline,
}

impl Waterfall {
    pub(super) fn new(
        device: &wgpu::Device,
        geometry_layout: &wgpu::PipelineLayout,
        output_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("waterfall region surface lighting"),
            source: wgpu::ShaderSource::Wgsl(
                format!(
                    "{}\n{}\n{}",
                    include_str!("shaders/surface_lighting.wgsl"),
                    include_str!("shaders/forward_surface.wgsl"),
                    include_str!("shaders/waterfall.wgsl"),
                )
                .into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("waterfall region pipeline"),
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
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
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
                .any(|draw| draw.waterfall && draw.instance_count > 0)
        };
        if !visible(zone.0) && !actors.iter().any(|actor| visible(&actor.scene)) {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("waterfall region surfaces"),
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
            timestamp_writes: profile.and_then(|p| p.timestamps(profiling::Pass::Waterfall)),
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, globals, &[]);
        pass.set_bind_group(1, lighting, &[]);
        pass.set_bind_group(2, zone.1, &[]);
        draw_waterfall(&mut pass, zone.0);
        for actor in actors {
            pass.set_bind_group(2, &actor.atlas, &[]);
            draw_waterfall(&mut pass, &actor.scene);
        }
    }
}

fn draw_waterfall(pass: &mut wgpu::RenderPass<'_>, scene: &GpuScene) {
    pass.set_vertex_buffer(0, scene.vertices.slice(..));
    pass.set_vertex_buffer(1, scene.instances.slice(..));
    pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
    for draw in scene.draws.iter().filter(|draw| draw.waterfall) {
        pass.draw_indexed(
            draw.index_start..draw.index_start + draw.index_count,
            draw.base_vertex,
            draw.instance_start..draw.instance_start + draw.instance_count,
        );
    }
}

// D3DX30 executes the phase and rate multiply in qword temporaries, then
// converts offsets to f32. A single f32 time*rate disagrees for original rates.
pub(crate) fn scroll_offsets(elapsed: std::time::Duration, rates: [f32; 4]) -> [f32; 4] {
    let milliseconds = elapsed.as_millis() as u32;
    let time = f64::from((milliseconds % 100_000) as f32 * 0.001);
    let phase = (time * 0.01).fract() * 100.0;
    rates.map(|rate| (phase * f64::from(rate)) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_preshader_retains_double_intermediates_before_final_float_conversion() {
        // Original D3DX30 outputs at cached millisecond237; the shortcut
        // f32 time*rate rounds each of the first three one ULP higher.
        let rates = [0.75, 3.0, -1.5, 0.0];
        assert_eq!(
            scroll_offsets(std::time::Duration::from_millis(237), rates).map(f32::to_bits),
            [0x3e36_0419, 0x3f36_0419, 0xbeb6_0419, 0]
        );
        for ms in [0, 237, 433, 99999] {
            let reference = scroll_offsets(std::time::Duration::from_millis(ms), rates);
            assert_eq!(
                scroll_offsets(std::time::Duration::from_millis(ms + 100_000), rates),
                reference
            );
            assert_eq!(
                scroll_offsets(std::time::Duration::from_millis(ms + (1_u64 << 32)), rates),
                reference
            );
        }
    }
}
