//! Compare CPU DDS decoding with hardware BC1 at original texel coordinates.
//! This does not emulate native D3DX image transforms or generated mip levels.
use openeq_assets::{loader, pfs::Archive, texture::Texture};

fn compare(device: &wgpu::Device, queue: &wgpu::Queue, name: &str, dds: &[u8]) {
    assert_eq!(&dds[84..88], b"DXT1");
    let decoded = Texture::decode(name, dds).unwrap();
    let (width, height) = (decoded.width, decoded.height);
    assert!(width.is_multiple_of(4) && height.is_multiple_of(4));
    let source = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("original BC1 level zero"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bc1RgbaUnorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let block_bytes = (width / 4 * height / 4 * 8) as usize;
    queue.write_texture(
        source.as_image_copy(),
        &dds[128..128 + block_bytes],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width / 4 * 8),
            rows_per_image: Some(height / 4),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let size = u64::from(width) * u64::from(height) * 16;
    let output = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("BC1 decoded words"),
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("BC1 readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("unfiltered BC1 decode comparison"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<vec4<f32>>;
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) pixel: vec3<u32>) {
    let size = textureDimensions(source);
    if (pixel.x < size.x && pixel.y < size.y) {
        output[pixel.y*size.x+pixel.x] = textureLoad(source, vec2<i32>(pixel.xy), 0);
    }
}
"#
            .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("BC1 decode comparison"),
        layout: None,
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &source.create_view(&Default::default()),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, size);
    queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
        tx.send(r).unwrap();
    });
    device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = readback.slice(..).get_mapped_range();
    let actual: Vec<_> = mapped
        .chunks_exact(4)
        .map(|b| (f32::from_le_bytes(b.try_into().unwrap()) * 255.).round() as u8)
        .collect();
    assert_eq!(actual.len(), decoded.rgba.len());
    let max = actual
        .iter()
        .zip(&decoded.rgba)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    let differing = actual
        .iter()
        .zip(&decoded.rgba)
        .filter(|(a, b)| a != b)
        .count();
    eprintln!("{name}: {width}x{height}, {differing} differing channels, maximum delta {max}");
    // BC interpolation quantization can differ by two stored-byte steps across
    // implementations. Alpha and texel addressing must agree exactly.
    assert!(max <= 2, "{name}: GPU/CPU channel delta {max}");
    for (gpu, cpu) in actual.chunks_exact(4).zip(decoded.rgba.chunks_exact(4)) {
        assert_eq!(gpu[3], cpu[3], "{name}: alpha");
    }
}

fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok()?;
    if !adapter
        .features()
        .contains(wgpu::Features::TEXTURE_COMPRESSION_BC)
    {
        eprintln!("BC1 hardware comparison unavailable on this adapter");
        return None;
    }
    eprintln!("BC1 comparison adapter: {:?}", adapter.get_info());
    Some(
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
            ..Default::default()
        }))
        .unwrap(),
    )
}

#[test]
fn asymmetric_bc1_texels_preserve_rows_channels_and_transparent_code() {
    let Some((device, queue)) = device() else {
        return;
    };
    let mut dds = vec![0; 128];
    dds[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x81007),
        (12, 8),
        (16, 8),
        (20, 32),
        (76, 32),
        (80, 4),
        (108, 0x1000),
    ] {
        dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    dds[84..88].copy_from_slice(b"DXT1");
    // Four unlike blocks, including three-color/transparent mode, with every
    // row and column carrying a distinct selector pattern.
    for (first, second, selectors) in [
        (0xf800u16, 0x001fu16, 0x1be4_4eb1u32),
        (0x07e0, 0, 0xe41b_b14e),
        (0, 0xffff, 0x4eb1_1be4),
        (0xffff, 0x0000, 0xb14e_e41b),
    ] {
        dds.extend(first.to_le_bytes());
        dds.extend(second.to_le_bytes());
        dds.extend(selectors.to_le_bytes());
    }
    compare(&device, &queue, "asymmetric.dds", &dds);
}

#[test]
#[ignore = "requires original textures and a BC-capable GPU"]
fn original_particles_and_masked_textures_match_hardware_bc1() {
    let (device, queue) = device().expect("original BC1 check requires hardware support");
    let base = loader::default_client_dir().unwrap();
    for (archive, name, width, height, transparent) in [
        ("poknowledge_obj.s3d", "csmoke.dds", 16, 32, 0),
        ("poknowledge_obj.s3d", "geng00.dds", 64, 64, 0),
        ("cosul.eqg", "lamp_chainlink.dds", 64, 64, 2938),
        ("broodlands.eqg", "swmp_canopy_trim.dds", 256, 256, 22977),
    ] {
        let archive = Archive::open(base.join(archive)).unwrap();
        let bytes = archive.read(name).unwrap();
        let texture = Texture::decode(name, &bytes).unwrap();
        assert_eq!((texture.width, texture.height), (width, height));
        assert_eq!(
            texture
                .rgba
                .chunks_exact(4)
                .filter(|pixel| pixel[3] == 0)
                .count(),
            transparent
        );
        compare(&device, &queue, name, &bytes);
    }
}

#[test]
#[ignore = "requires original masked textures and GPU"]
fn original_bc1_mask_reveals_background_through_authored_holes() {
    use openeq_assets::{
        Scene,
        mesh::{Geometry, Material},
    };
    use openeq_render::{Camera, GpuScene, Renderer};
    let base = loader::default_client_dir().unwrap();
    let archive = Archive::open(base.join("cosul.eqg")).unwrap();
    let fixed = Texture::decode(
        "lamp_chainlink.dds",
        &archive.read("lamp_chainlink.dds").unwrap(),
    )
    .unwrap();
    let mut former = fixed.clone();
    for pixel in former.rgba.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    let plane = |y, material| Geometry {
        vertices: vec![
            -10., y, -10., 0., -1., 0., 0., 0., 10., y, -10., 0., -1., 0., 1., 0., 10., y, 10., 0.,
            -1., 0., 1., 1., -10., y, 10., 0., -1., 0., 0., 1.,
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        collidable: false,
    };
    let material = |name: &str, alpha_mask| Material {
        textures: vec![name.into()],
        normal_map: None,
        water: None,
        flags: 0,
        anim_speed: 0,
        alpha_mask,
        transparent: false,
        additive: false,
        emissive: true,
        clamp_uv: false,
        uv_encoding: Default::default(),
    };
    let mut renderer = Renderer::new_headless(256, 256).unwrap();
    let mut render = |texture: Texture| {
        let source = Scene::from_geometry(
            "BC1 mask witness".into(),
            vec![material(&texture.name, true), material("background", false)],
            vec![plane(12., 0), plane(14., 1)],
            vec![
                texture,
                Texture {
                    name: "background".into(),
                    width: 1,
                    height: 1,
                    rgba: vec![0, 0, 192, 255],
                },
            ],
        );
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
        renderer.set_scene(&gpu);
        renderer.render_at(
            &gpu,
            &Camera {
                pitch: 0.,
                ..Default::default()
            },
            std::time::Duration::ZERO,
        );
        renderer.read_rgba().unwrap().2
    };
    let before = render(former);
    let after = render(fixed);
    // Inspect only the central footprint where both quads cover the image.
    // Authored opaque chain texels remain unchanged; restored holes reveal blue.
    let mut revealed = 0;
    let mut unchanged = 0;
    for y in 64..192 {
        for x in 64..192 {
            let offset = (y * 256 + x) * 4;
            let old = &before[offset..offset + 4];
            let new = &after[offset..offset + 4];
            if new == [0, 0, 192, 255] && old != new {
                revealed += 1;
            }
            if new == old && new != [0, 0, 192, 255] {
                unchanged += 1;
            }
        }
    }
    assert!(
        revealed > 3000,
        "authored chain holes not visible: {revealed}"
    );
    assert!(unchanged > 1000, "opaque chain pixels changed: {unchanged}");
    let output = std::path::Path::new("/tmp/openeq-bc1-alpha-preview");
    std::fs::create_dir_all(output).unwrap();
    for (name, pixels) in [("before.png", before), ("after.png", after)] {
        image::save_buffer(
            output.join(name),
            &pixels,
            256,
            256,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
    eprintln!(
        "BC1 original chainlink: {revealed} holes reveal background; {unchanged} opaque pixels unchanged"
    );
}
