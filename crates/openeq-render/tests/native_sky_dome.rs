//! Opt-in/default-state GPU diagnostic, not the live sky implementation.
//! Explicit UNORM target, no texture/lighting/fog/blend/culling, native topology.
//! Camera matrices are diagnostic inputs; no native host orientation is inferred.
use glam::{Mat4, Vec3};
use openeq_assets::{
    environment::{self, SkyAssets, SkyColorMapLayout, dome::NativeSkyDome},
    loader,
    texture::Texture,
};
use openeq_render::Renderer;
use wgpu::util::DeviceExt;

const SIZE: u32 = 128;
fn palette(rgba: [u8; 4]) -> SkyAssets {
    SkyAssets {
        weather: "diagnostic".into(),
        color_map: Texture {
            name: "diagnostic".into(),
            width: 32,
            height: 32,
            rgba: rgba.repeat(1024),
        },
        color_map_layout: SkyColorMapLayout::OriginalDome,
        color_map_provenance: None,
        cloud_texture: None,
        cloud_color_map: None,
        cloud_color_map_layout: SkyColorMapLayout::FullTexture,
        cloud_color_map_provenance: None,
        cloud_velocity: 0.,
    }
}
fn render(renderer: &Renderer, sky: &SkyAssets, direction: Vec3, fov: f32) -> Vec<u8> {
    let device = renderer.device();
    let queue = renderer.queue();
    let dome = NativeSkyDome::build(80., sky).unwrap();
    let data: Vec<u8> = dome
        .vertices
        .iter()
        .flat_map(|v| {
            v.position
                .map(f32::to_bits)
                .into_iter()
                .chain([v.diffuse])
                .flat_map(u32::to_le_bytes)
        })
        .collect();
    let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("native dome16-bytevertices"),
        contents: &data,
        usage: wgpu::BufferUsages::VERTEX,
    });
    let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("native dome indices"),
        contents: bytemuck::cast_slice(&dome.indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let up = if direction.z.abs() > 0.99 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let matrix =
        Mat4::perspective_rh(fov, 1., 1., 200.) * Mat4::look_at_rh(Vec3::ZERO, direction, up);
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(&matrix.to_cols_array()),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let shader=device.create_shader_module(wgpu::ShaderModuleDescriptor{label:Some("unlit diffuse dome diagnostic"),source:wgpu::ShaderSource::Wgsl(r#"
@group(0) @binding(0) var<uniform> matrix: mat4x4<f32>;
struct Out { @builtin(position) clip: vec4<f32>, @location(0) diffuse: vec4<f32> };
@vertex fn vs_main(@location(0) position:vec3<f32>, @location(1) argb:u32)->Out {
    var out:Out;out.clip=matrix*vec4<f32>(position,1.0);
    out.diffuse=vec4<f32>(f32((argb>>16u)&255u),f32((argb>>8u)&255u),f32(argb&255u),f32(argb>>24u))/255.0;
    return out;
}
@fragment fn fs_main(in:Out)->@location(0) vec4<f32> { return in.diffuse; }
"#.into())});
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("native dome explicit default-state diagnostic"),
        layout: None,
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: 16,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0=>Float32x3,1=>Uint32],
            }],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
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
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::LessEqual),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform.as_entire_binding(),
        }],
    });
    let texture = |format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: SIZE,
                height: SIZE,
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
    let output = texture(
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let view = output.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(SIZE * SIZE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.,
                        g: 0.,
                        b: 1.,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..dome.indices.len() as u32, 0, 0..1);
    }
    encoder.copy_texture_to_buffer(
        output.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let result = readback.slice(..).get_mapped_range().to_vec();
    readback.unmap();
    result
}

#[test]
fn native_dome_default_state_preserves_unorm_rgb_and_zero_alpha_in_six_views() {
    let renderer = Renderer::new_headless(8, 8).expect("GPU required");
    let sky = palette([51, 102, 153, 0]);
    for direction in [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ] {
        let pixels = render(&renderer, &sky, direction, 60_f32.to_radians());
        assert!(
            pixels.chunks_exact(4).all(|p| p == [51, 102, 153, 0]),
            "constant diffuse changed or dome left holes toward {direction:?}"
        );
    }
}

#[test]
fn native_near_pole_colors_survive_and_unused_table_words_never_become_pixels() {
    let renderer = Renderer::new_headless(8, 8).expect("GPU required");
    let mut sky = palette([0, 0, 0, 255]);
    let before = render(&renderer, &sky, Vec3::Z, 1_f32.to_radians());
    sky.color_map.rgba[4..8].copy_from_slice(&[255, 32, 0, 255]);
    let after = render(&renderer, &sky, Vec3::Z, 1_f32.to_radians());
    assert!(
        before.iter().zip(&after).filter(|(a, b)| a != b).count() > 100,
        "valid first-ring source color disappeared"
    );
    let used: std::collections::BTreeSet<_> = NativeSkyDome::build(80., &sky)
        .unwrap()
        .source_color_indices
        .into_iter()
        .collect();
    for i in 0..1024 {
        if !used.contains(&i) {
            sky.color_map.rgba[i as usize * 4..i as usize * 4 + 4]
                .copy_from_slice(&[255, 0, 255, 255]);
        }
    }
    assert_eq!(render(&renderer, &sky, Vec3::Z, 1_f32.to_radians()), after);
}

#[test]
#[ignore = "requires original PoK sky tables and GPU; diagnostic captures under /tmp/openeq-native-sky-gpu"]
fn original_pok_dome_palettes_render_without_sampling_auxiliary_table_entries() {
    let base = loader::default_client_dir().unwrap();
    let renderer = Renderer::new_headless(8, 8).expect("GPU required");
    let path = std::path::Path::new("/tmp/openeq-native-sky-gpu");
    std::fs::create_dir_all(path).unwrap();
    let mut images = vec![];
    for (name, time) in [("night", 0.), ("dawn", 0.25), ("day", 0.5), ("dusk", 0.75)] {
        let sky = environment::load_sky(&base, "poknowledge", time).unwrap();
        let image = render(
            &renderer,
            &sky,
            Vec3::new(1., 0., 0.5).normalize(),
            70_f32.to_radians(),
        );
        assert!(image.chunks_exact(4).all(|p| p != [255, 0, 255, 255]));
        image::save_buffer(
            path.join(format!("pok-{name}.png")),
            &image,
            SIZE,
            SIZE,
            image::ColorType::Rgba8,
        )
        .unwrap();
        images.push(image);
    }
    assert!(images.windows(2).all(|pair| pair[0] != pair[1]));
}
