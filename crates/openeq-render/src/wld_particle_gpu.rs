//! Opt-in, standalone WebGPU diagnostics for captured WLD particle quads.
//!
//! Both source and target are RGBA8Unorm, explicitly assuming native D3D defaults
//! for inherited sRGB states. No scene, camera, sampler, fog or spell integration.
//! Original pixel parity, mip generation and D3D9 rasterization are unverified.
//! See `docs/WLD_PARTICLE_GPU_DIAGNOSTIC.md` and `WLD_PARTICLE_GPU_CONTRACT.md`.

use anyhow::{Result, ensure};
use bytemuck::{Pod, Zeroable};
use openeq_assets::texture::Texture;
use wgpu::util::DeviceExt;

pub const MAX_TARGET_EDGE: u32 = 2048;
pub const MAX_TEXTURE_EDGE: u32 = 2048;
pub const MAX_TEXTURES: usize = 32;
pub const MAX_TEXTURE_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_QUADS: usize = 1024;

/// Explicit experiment choice; neither option certifies D3D9 edge coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelCenterConvention {
    /// Use captured numeric screen coordinates on WebGPU without translation.
    AsCaptured,
    /// Add +0.5 to both screen coordinates, testing integer-center translation.
    ShiftByHalfPixel,
}

#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Native 32-byte XYZRHW record; diffuse is the packed word 0xAARRGGBB.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ProjectedVertex {
    pub xyzrhw: [f32; 4],
    pub diffuse: u32,
    /// The bounded native witnesses have zero specular; other values are rejected.
    pub specular: u32,
    pub uv: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectedQuad {
    /// Native corner order: top-left, top-right, bottom-right, bottom-left.
    pub vertices: [ProjectedVertex; 4],
    pub texture: usize,
}

pub struct DiagnosticFrame<'a> {
    pub target_size: [u32; 2],
    pub viewport: Viewport,
    pub pixel_centers: PixelCenterConvention,
    pub clear_rgba: [f64; 4],
    /// Uniform initial depth; the particle pass never writes depth.
    pub clear_depth: f32,
    /// Decoded, original-sized, top-left-origin RGBA8. No resizing or atlas.
    pub textures: &'a [Texture],
    /// Already projected and selected by the caller. No guessed camera/culling.
    pub quads: &'a [ProjectedQuad],
}

impl DiagnosticFrame<'_> {
    /// Validate the entire frame before GPU allocation or submission.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.target_size
                .iter()
                .all(|&n| n > 0 && n <= MAX_TARGET_EDGE),
            "diagnostic target dimensions must be 1..={MAX_TARGET_EDGE}"
        );
        let v = self.viewport;
        ensure!(
            v.width > 0
                && v.height > 0
                && u64::from(v.x) + u64::from(v.width) <= u64::from(self.target_size[0])
                && u64::from(v.y) + u64::from(v.height) <= u64::from(self.target_size[1]),
            "diagnostic viewport must fit the target"
        );
        ensure!(
            self.clear_rgba
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
                && self.clear_depth.is_finite()
                && (0.0..=1.0).contains(&self.clear_depth),
            "invalid diagnostic clear value"
        );
        ensure!(
            self.textures.len() <= MAX_TEXTURES && self.quads.len() <= MAX_QUADS,
            "diagnostic texture/quad count exceeds its cap"
        );
        let mut total = 0;
        for texture in self.textures {
            ensure!(
                texture.width > 0
                    && texture.height > 0
                    && texture.width <= MAX_TEXTURE_EDGE
                    && texture.height <= MAX_TEXTURE_EDGE,
                "invalid diagnostic texture dimensions"
            );
            let bytes = u64::from(texture.width) * u64::from(texture.height) * 4;
            ensure!(
                bytes == texture.rgba.len() as u64,
                "malformed diagnostic RGBA8 texture"
            );
            total += bytes;
            ensure!(
                total <= MAX_TEXTURE_BYTES,
                "diagnostic texture byte budget exceeded"
            );
        }
        let offset = self.pixel_offset();
        for quad in self.quads {
            ensure!(
                quad.texture < self.textures.len(),
                "missing diagnostic texture"
            );
            for vertex in quad.vertices {
                ensure!(
                    vertex
                        .xyzrhw
                        .iter()
                        .chain(vertex.uv.iter())
                        .all(|v| v.is_finite())
                        && vertex.xyzrhw[3] > 0.0
                        && vertex.xyzrhw[3].is_normal()
                        && vertex.specular == 0,
                    "unsupported diagnostic vertex: finite coordinates, positive normal RHW and zero specular required"
                );
                // Reject numeric overflow in the shader transform. Finite depth outside
                // [0,1] remains intact for WebGPU clipping, rather than being clamped.
                let [x, y, z, rhw] = vertex.xyzrhw;
                let w = 1.0 / rhw;
                let nx = ((x + offset - v.x as f32) / v.width as f32 * 2.0 - 1.0) * w;
                let ny = ((y + offset - v.y as f32) / v.height as f32 * 2.0 - 1.0) * w;
                ensure!(
                    [w, nx, ny, z * w].iter().all(|v| v.is_finite()) && w.is_normal(),
                    "diagnostic clip-coordinate overflow or subnormal reciprocal RHW"
                );
            }
        }
        Ok(())
    }

    fn pixel_offset(&self) -> f32 {
        match self.pixel_centers {
            PixelCenterConvention::AsCaptured => 0.0,
            PixelCenterConvention::ShiftByHalfPixel => 0.5,
        }
    }
}

pub struct DiagnosticImage {
    pub width: u32,
    pub height: u32,
    /// Stored RGBA8Unorm target bytes, without a display transfer conversion.
    pub rgba: Vec<u8>,
    /// Actual GPU upload dimensions, in the frame's texture order.
    pub uploaded_texture_sizes: Vec<[u32; 2]>,
}

/// Owns an independent headless device and never attaches to the scene renderer.
pub struct DiagnosticRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    screen_layout: wgpu::BindGroupLayout,
    source_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
}

impl DiagnosticRenderer {
    pub fn new_headless() -> Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("standalone WLD particle diagnostic"),
                ..Default::default()
            }))?;
        let screen_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("diagnostic screen"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let source_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("diagnostic original texture"),
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("diagnostic wrap-linear level zero"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            ..Default::default()
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("diagnostic WLD layout"),
            bind_group_layouts: &[Some(&screen_layout), Some(&source_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("diagnostic WLD projected quads"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/wld_particle_diagnostic.wgsl").into(),
            ),
        });
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("diagnostic unorm source-alpha additive"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 32,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Uint32,
                            offset: 16,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: blend,
                        alpha: blend,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Ok(Self {
            device,
            queue,
            screen_layout,
            source_layout,
            sampler,
            pipeline,
        })
    }

    /// Synchronously render and read back one bounded standalone experiment.
    /// No native draw gates, sorting, camera projection or mip generation are inferred.
    pub fn render(&self, frame: &DiagnosticFrame<'_>) -> Result<DiagnosticImage> {
        frame.validate()?;
        let [width, height] = frame.target_size;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let make_target = |label, format, usage| {
            self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let target = make_target(
            "diagnostic unorm target",
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = make_target(
            "diagnostic read-only depth",
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let target_view = target.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let v = frame.viewport;
        let screen = [
            v.x as f32,
            v.y as f32,
            v.width as f32,
            v.height as f32,
            frame.pixel_offset(),
            frame.pixel_offset(),
            0.0,
            0.0,
        ];
        let screen_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("diagnostic viewport"),
                contents: bytemuck::cast_slice(&screen),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let screen_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("diagnostic viewport"),
            layout: &self.screen_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: screen_buffer.as_entire_binding(),
            }],
        });
        let mut sources = Vec::with_capacity(frame.textures.len());
        let mut groups = Vec::with_capacity(frame.textures.len());
        for texture in frame.textures {
            let gpu = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("diagnostic original-sized unorm source"),
                size: wgpu::Extent3d {
                    width: texture.width,
                    height: texture.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.queue.write_texture(
                gpu.as_image_copy(),
                &texture.rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(texture.width * 4),
                    rows_per_image: Some(texture.height),
                },
                gpu.size(),
            );
            let view = gpu.create_view(&Default::default());
            groups.push(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("diagnostic source"),
                layout: &self.source_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            }));
            sources.push(gpu);
        }
        // Native shared index builder 0x100b0690 uses the TR-to-BL diagonal.
        // See WLD_PARTICLE_TEXTURE_LOAD.md for the executed full-buffer witness.
        let vertices: Vec<_> = frame
            .quads
            .iter()
            .flat_map(|q| [0, 1, 3, 1, 2, 3].map(|i| q.vertices[i]))
            .collect();
        let vertex_buffer = (!vertices.is_empty()).then(|| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("diagnostic captured vertices"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let [r, g, b, a] = frame.clear_rgba;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("standalone WLD particle diagnostic"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(frame.clear_depth),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_viewport(
                v.x as f32,
                v.y as f32,
                v.width as f32,
                v.height as f32,
                0.0,
                1.0,
            );
            pass.set_scissor_rect(v.x, v.y, v.width, v.height);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &screen_group, &[]);
            if let Some(buffer) = &vertex_buffer {
                pass.set_vertex_buffer(0, buffer.slice(..));
                for (i, quad) in frame.quads.iter().enumerate() {
                    pass.set_bind_group(1, &groups[quad.texture], &[]);
                    pass.draw(i as u32 * 6..i as u32 * 6 + 6, 0..1);
                }
            }
        }
        let stride = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("diagnostic readback"),
            size: u64::from(stride) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            size,
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        receiver.recv()??;
        let bytes = slice.get_mapped_range();
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for row in bytes.chunks_exact(stride as usize) {
            rgba.extend_from_slice(&row[..width as usize * 4]);
        }
        drop(bytes);
        readback.unmap();
        Ok(DiagnosticImage {
            width,
            height,
            rgba,
            uploaded_texture_sizes: sources.iter().map(|t| [t.width(), t.height()]).collect(),
        })
    }
}
