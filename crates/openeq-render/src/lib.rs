//! A custom `wgpu` renderer for EverQuest geometry.
//!
//! The pipeline is deferred: geometry is rendered once into a G-buffer
//! (albedo + normal + depth), then a fullscreen pass reconstructs world
//! positions and shades everything with a shadow-mapped directional sun plus
//! the zone's point lights.
//!
//! The same core renders to a window surface or to an offscreen texture, so the
//! whole pipeline can be exercised headlessly by tests and tools.

pub mod scene;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

pub use scene::{Camera, GpuScene};

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const SHADOW_SIZE: u32 = 2048;

/// Uniform block shared by every pass.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_projection: [[f32; 4]; 4],
    light_view_projection: [[f32; 4]; 4],
    inverse_view_projection: [[f32; 4]; 4],
    camera_position: [f32; 4],
    ambient: [f32; 4],
    sun_direction: [f32; 4],
    sun_color: [f32; 4],
    /// `x` = elapsed milliseconds, `y` = point light count.
    params: [f32; 4],
}

struct Targets {
    albedo_view: wgpu::TextureView,
    normal_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

impl Targets {
    fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let size = wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        };
        let make = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
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
        let albedo = make(
            "gbuffer albedo",
            ALBEDO_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let normal = make(
            "gbuffer normal",
            NORMAL_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        let depth = make(
            "gbuffer depth",
            DEPTH_FORMAT,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        );
        Self {
            albedo_view: albedo.create_view(&Default::default()),
            normal_view: normal.create_view(&Default::default()),
            depth_view: depth.create_view(&Default::default()),
        }
    }
}

enum Target {
    Surface(wgpu::Surface<'static>),
    Offscreen {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
    },
}

struct Pipelines {
    shadow: wgpu::RenderPipeline,
    geometry: wgpu::RenderPipeline,
    lighting: wgpu::RenderPipeline,
}

/// The renderer: device, targets and bind groups.
pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    target: Target,
    config: wgpu::SurfaceConfiguration,
    targets: Targets,
    shadow_view: wgpu::TextureView,
    /// A 1x1 depth texture bound during the passes that render *into* the shadow
    /// map; a texture cannot be both an attachment and a binding in one pass.
    placeholder_depth: wgpu::TextureView,
    shadow_sampler: wgpu::Sampler,
    globals: wgpu::Buffer,
    globals_bind_group: wgpu::BindGroup,
    scene_bind_group: Option<wgpu::BindGroup>,
    scene_bind_group_plain: Option<wgpu::BindGroup>,
    scene_layout: wgpu::BindGroupLayout,
    atlas_layout: wgpu::BindGroupLayout,
    atlas_bind_group: Option<wgpu::BindGroup>,
    gbuffer_layout: wgpu::BindGroupLayout,
    lighting_bind_group: Option<wgpu::BindGroup>,
    pipelines: Pipelines,
    start: std::time::Instant,
    width: u32,
    height: u32,
}

impl Renderer {
    /// Creates a renderer that draws into an offscreen texture.
    pub fn new_headless(width: u32, height: u32) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("openeq"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            }))?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: ALBEDO_FORMAT,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ALBEDO_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());

        Ok(Self::with_target(
            device,
            queue,
            Target::Offscreen { texture, view },
            config,
        ))
    }

    /// Creates a renderer that presents to a window surface.
    pub async fn new_surface(
        instance: &wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("openeq"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let capabilities = surface.get_capabilities(&adapter);
        let format = capabilities
            .formats
            .iter()
            .copied()
            .find(|format| format.is_srgb())
            .unwrap_or(capabilities.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: capabilities.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        Ok(Self::with_target(
            device,
            queue,
            Target::Surface(surface),
            config,
        ))
    }

    fn with_target(
        device: wgpu::Device,
        queue: wgpu::Queue,
        target: Target,
        config: wgpu::SurfaceConfiguration,
    ) -> Self {
        let width = config.width;
        let height = config.height;

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals layout"),
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
        let globals_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals bind group"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });

        let scene_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas layout"),
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
            ],
        });

        let gbuffer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gbuffer layout"),
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
                    // Non-filtering: this sampler also samples the depth texture.
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_geometry = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("geometry pipeline layout"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&scene_layout),
                Some(&atlas_layout),
            ],
            immediate_size: 0,
        });
        let pipeline_lighting = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lighting pipeline layout"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&scene_layout),
                Some(&gbuffer_layout),
            ],
            immediate_size: 0,
        });

        let shadow = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shadow map"),
            size: wgpu::Extent3d {
                width: SHADOW_SIZE,
                height: SHADOW_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let shadow_view = shadow.create_view(&Default::default());
        let placeholder = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("placeholder depth"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let placeholder_depth = placeholder.create_view(&Default::default());
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let layouts = vertex_layouts();
        let shadow_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shadow.wgsl").into()),
        });
        let geometry_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gbuffer"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/gbuffer.wgsl").into()),
        });
        let lighting_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lighting"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lighting.wgsl").into()),
        });

        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow pipeline"),
            layout: Some(&pipeline_geometry),
            vertex: wgpu::VertexState {
                module: &shadow_shader,
                entry_point: Some("vs_main"),
                buffers: &layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shadow_shader,
                entry_point: Some("fs_main"),
                targets: &[],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: 2,
                    slope_scale: 2.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let geometry_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("geometry pipeline"),
            layout: Some(&pipeline_geometry),
            vertex: wgpu::VertexState {
                module: &geometry_shader,
                entry_point: Some("vs_main"),
                buffers: &layouts,
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &geometry_shader,
                entry_point: Some("fs_main"),
                targets: &[
                    Some(wgpu::ColorTargetState {
                        format: ALBEDO_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    Some(wgpu::ColorTargetState {
                        format: NORMAL_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let lighting_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lighting pipeline"),
            layout: Some(&pipeline_lighting),
            vertex: wgpu::VertexState {
                module: &lighting_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &lighting_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
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

        Self {
            device: device.clone(),
            queue,
            target,
            config,
            targets: Targets::new(&device, width, height),
            shadow_view,
            placeholder_depth,
            shadow_sampler,
            globals,
            globals_bind_group,
            scene_bind_group: None,
            scene_bind_group_plain: None,
            scene_layout,
            atlas_layout,
            atlas_bind_group: None,
            gbuffer_layout,
            lighting_bind_group: None,
            pipelines: Pipelines {
                shadow: shadow_pipeline,
                geometry: geometry_pipeline,
                lighting: lighting_pipeline,
            },
            start: std::time::Instant::now(),
            width,
            height,
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// The device backing this renderer, for uploading scene data.
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// The queue backing this renderer, for uploading scene data.
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Builds the bind groups that depend on the scene (shadow map, lights, atlas).
    pub fn set_scene(&mut self, scene: &GpuScene) {
        let make_scene_group = |label: &str, shadow: &wgpu::TextureView| {
            self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &self.scene_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(shadow),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: scene.lights.as_entire_binding(),
                    },
                ],
            })
        };
        self.scene_bind_group = Some(make_scene_group("scene bind group", &self.shadow_view));
        self.scene_bind_group_plain = Some(make_scene_group(
            "scene bind group (no shadow)",
            &self.placeholder_depth,
        ));

        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        self.atlas_bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas bind group"),
            layout: &self.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&scene.atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        }));

        let gbuffer_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gbuffer sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        self.lighting_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("gbuffer bind group"),
                layout: &self.gbuffer_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.targets.albedo_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&gbuffer_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&self.targets.normal_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&self.targets.depth_view),
                    },
                ],
            }));
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;
        self.config.width = width;
        self.config.height = height;
        match &mut self.target {
            Target::Surface(surface) => surface.configure(&self.device, &self.config),
            Target::Offscreen { texture, view } => {
                *texture = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("offscreen target"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: ALBEDO_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });
                *view = texture.create_view(&Default::default());
            }
        }
        self.targets = Targets::new(&self.device, width, height);
        self.lighting_bind_group = None;
        self.scene_bind_group = None;
        self.scene_bind_group_plain = None;
    }

    /// Renders one frame of `scene` from `camera`.
    pub fn render(&mut self, scene: &GpuScene, camera: &Camera) {
        if self.scene_bind_group.is_none() || self.lighting_bind_group.is_none() {
            self.set_scene(scene);
        }

        let frame = match &self.target {
            Target::Surface(surface) => match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
                other => {
                    tracing::warn!(?other, "dropping frame: surface not ready");
                    return;
                }
            },
            Target::Offscreen { .. } => None,
        };

        let swapchain_view = frame
            .as_ref()
            .map(|frame| frame.texture.create_view(&Default::default()));
        let final_view = match (&swapchain_view, &self.target) {
            (Some(view), _) => view,
            (None, Target::Offscreen { view, .. }) => view,
            (None, Target::Surface(_)) => unreachable!("surface frames always yield a view"),
        };

        let aspect = self.width as f32 / self.height.max(1) as f32;
        let view_projection = camera.view_projection(aspect, 1.0, 20000.0);

        // A camera-anchored shadow frustum keeps the map sharp where it matters.
        let sun = Vec3::new(-0.45, 0.82, 0.35).normalize();
        let focus = Camera::to_world(camera.position) + camera.forward() * 150.0;
        let radius = 300.0;
        let distance = 1600.0;
        let light_view = Mat4::look_at_rh(focus - sun * distance, focus, Vec3::Y);
        let light_projection =
            Mat4::orthographic_rh(-radius, radius, -radius, radius, 1.0, distance * 2.0);
        let light_view_projection = light_projection * light_view;

        let elapsed = self.start.elapsed().as_secs_f32() * 1000.0;
        let globals = Globals {
            view_projection: view_projection.to_cols_array_2d(),
            light_view_projection: light_view_projection.to_cols_array_2d(),
            inverse_view_projection: view_projection.inverse().to_cols_array_2d(),
            camera_position: Camera::to_world(camera.position).extend(1.0).into(),
            ambient: Vec4::new(0.22, 0.24, 0.30, 1.0).into(),
            sun_direction: Vec4::new(sun.x, sun.y, sun.z, 1.0).into(),
            sun_color: Vec4::new(1.0, 0.96, 0.86, 1.0).into(),
            params: [elapsed, scene.light_count as f32, 0.0, 0.0],
        };
        self.queue
            .write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        let scene_bind_group = self.scene_bind_group.as_ref().unwrap();
        let plain_bind_group = self.scene_bind_group_plain.as_ref().unwrap();
        let atlas_bind_group = self.atlas_bind_group.as_ref().unwrap();
        let lighting_bind_group = self.lighting_bind_group.as_ref().unwrap();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });

        // 1. Shadow map.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow pass"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.shadow);
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_bind_group(1, plain_bind_group, &[]);
            // The shadow pipeline shares the geometry layout, so slot 2 must be
            // bound even though the shader does not read it.
            pass.set_bind_group(2, atlas_bind_group, &[]);
            draw_scene(&mut pass, scene);
        }

        // 2. G-buffer.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("geometry pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.albedo_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: &self.targets.normal_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    }),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.geometry);
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_bind_group(1, plain_bind_group, &[]);
            pass.set_bind_group(2, atlas_bind_group, &[]);
            draw_scene(&mut pass, scene);
        }

        // 3. Lighting, straight to the target.
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lighting pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: final_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipelines.lighting);
            pass.set_bind_group(0, &self.globals_bind_group, &[]);
            pass.set_bind_group(1, scene_bind_group, &[]);
            pass.set_bind_group(2, lighting_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        if let Some(frame) = frame {
            frame.present();
        }
    }

    /// Reads back the offscreen target as RGBA8. Only valid for headless use.
    pub fn read_rgba(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        let Target::Offscreen { texture, .. } = &self.target else {
            return None;
        };
        let width = self.width;
        let height = self.height;
        let bytes_per_row = (width * 4).next_multiple_of(256);
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        let slice = buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
        receiver.recv().ok()?.ok()?;

        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for row in 0..height {
            let start = (row * bytes_per_row) as usize;
            pixels.extend_from_slice(&data[start..start + (width * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        Some((width, height, pixels))
    }
}

fn draw_scene(pass: &mut wgpu::RenderPass<'_>, scene: &GpuScene) {
    pass.set_vertex_buffer(0, scene.vertices.slice(..));
    pass.set_vertex_buffer(1, scene.instances.slice(..));
    pass.set_index_buffer(scene.indices.slice(..), wgpu::IndexFormat::Uint32);
    for draw in &scene.draws {
        pass.draw_indexed(
            draw.index_start..draw.index_start + draw.index_count,
            draw.base_vertex,
            draw.instance_start..draw.instance_start + draw.instance_count,
        );
    }
}

fn vertex_layouts() -> [wgpu::VertexBufferLayout<'static>; 2] {
    use std::mem::size_of;
    const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        0 => Float32x3, // position
        1 => Float32x3, // normal
        2 => Float32x2, // uv
        3 => Uint32,    // atlas layer
        4 => Uint32,    // frame offset
        5 => Uint32,    // frame count
        6 => Uint32,    // flags
        7 => Uint32,    // milliseconds per frame
    ];
    const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        8 => Float32x4,
        9 => Float32x4,
        10 => Float32x4,
        11 => Float32x4,
    ];
    [
        wgpu::VertexBufferLayout {
            array_stride: size_of::<scene::Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &VERTEX_ATTRIBUTES,
        },
        wgpu::VertexBufferLayout {
            array_stride: size_of::<scene::Instance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &INSTANCE_ATTRIBUTES,
        },
    ]
}
