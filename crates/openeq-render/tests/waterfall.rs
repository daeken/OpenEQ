//! Verified TER waterfall texture/alpha/depth contract on the GPU.
use openeq_assets::{
    Scene, loader,
    mesh::{Geometry, Material, UvEncoding},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

fn material(name: &str, rates: Option<[f32; 4]>) -> Material {
    Material {
        textures: vec![name.into()],
        normal_map: None,
        water: None,
        flags: 0,
        anim_speed: 0,
        alpha_mask: false,
        transparent: false,
        additive: false,
        emissive: rates.is_none(),
        clamp_uv: false,
        waterfall: rates,
        uv_encoding: UvEncoding::NativeTerShort2Sse2,
    }
}
fn plane(y: f32, material: usize, uv: [f32; 2]) -> Geometry {
    let vertices = [
        [-10., y, -10.],
        [10., y, -10.],
        [10., y, 10.],
        [-10., y, 10.],
    ]
    .into_iter()
    .flat_map(|p| [p[0], p[1], p[2], 0., -1., 0., uv[0], uv[1]])
    .collect();
    Geometry {
        vertices,
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        collidable: false,
    }
}
fn solid(name: &str, color: [u8; 4]) -> Texture {
    Texture {
        name: name.into(),
        width: 1,
        height: 1,
        rgba: color.to_vec(),
    }
}
fn scene(texture: Texture, rates: [f32; 4], uv: [f32; 2]) -> Scene {
    Scene::from_geometry(
        "waterfall".into(),
        vec![
            material("background", None),
            material(&texture.name, Some(rates)),
        ],
        vec![plane(4., 0, uv), plane(2., 1, uv)],
        vec![
            solid("background", [0, 0, 64, 255]),
            solid("far", [0, 128, 0, 255]),
            texture,
        ],
    )
}
fn pixel(renderer: &mut Renderer, source: &Scene, elapsed: Duration) -> [u8; 4] {
    let scene = GpuScene::build(renderer.device(), renderer.queue(), source).unwrap();
    assert!(
        scene
            .draws
            .iter()
            .filter(|d| d.waterfall)
            .all(|d| !d.transparent)
    );
    renderer.set_scene(&scene);
    renderer.render_at(
        &scene,
        &Camera {
            pitch: 0.,
            ..Default::default()
        },
        elapsed,
    );
    let (w, h, pixels) = renderer.read_rgba().unwrap();
    pixels[((h / 2 * w + w / 2) * 4) as usize..][..4]
        .try_into()
        .unwrap()
}
fn linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn source_alpha_cutoff_opaque_depth_and_no_waterfall_depth_write() {
    let mut renderer = Renderer::new_headless(64, 64).expect("GPU required");
    let elapsed = Duration::ZERO;
    let full = pixel(
        &mut renderer,
        &scene(solid("flow", [128, 0, 0, 255]), [0.; 4], [0.5; 2]),
        elapsed,
    );
    assert!(
        full[0] > 30 && full[0] < 128,
        "waterfalls must be lit: {full:?}"
    );
    for alpha in [0, 1, 15, 16, 17, 128, 254, 255] {
        let actual = pixel(
            &mut renderer,
            &scene(solid("flow", [128, 0, 0, alpha]), [0.; 4], [0.5; 2]),
            elapsed,
        );
        let opacity = if alpha < 16 {
            0.
        } else {
            f32::from(alpha) / 255.
        };
        for i in 0..3 {
            let expected = linear(full[i]) * opacity + linear([0, 0, 64][i]) * (1. - opacity);
            assert!(
                (linear(actual[i]) - expected).abs() < 0.008,
                "alpha={alpha}: {actual:?}, expected linear {expected}"
            );
        }
        assert_eq!(actual[3], 255);
    }
    let mut source = scene(solid("flow", [128, 0, 0, 255]), [0.; 4], [0.5; 2]);
    source.materials.push(material("far", Some([0.; 4])));

    source.meshes.push(plane(3., 2, [0.5; 2]));
    let two = pixel(&mut renderer, &source, elapsed);
    assert_eq!(two[0], 0, "near waterfall wrote depth");
    assert!(two[1] > 30);
    source.meshes[0] = plane(1., 0, [0.5; 2]);
    assert_eq!(pixel(&mut renderer, &source, elapsed), [0, 0, 64, 255]);
}

#[test]
fn color_and_opacity_scroll_independently_with_shared_clock_and_fog() {
    let mut renderer = Renderer::new_headless(64, 64).expect("GPU required");
    let texture = Texture {
        name: "flow".into(),
        width: 256,
        height: 256,
        rgba: (0..256)
            .flat_map(|_| {
                (0..256).flat_map(|x| match x / 64 {
                    0 => [160, 0, 0, 255],
                    1 => [0, 160, 0, 255],
                    2 => [0, 0, 160, 0],
                    _ => [160, 160, 0, 128],
                })
            })
            .collect(),
    };
    let source = scene(texture, [0.25, 0., 0.5, 0.], [0.125, 0.5]);
    let baseline = pixel(&mut renderer, &source, Duration::ZERO);
    assert!(baseline[0] > 40 && baseline[1] == 0);
    // At t=1 color is green, while opacity alone is fully transparent.
    assert_eq!(
        pixel(&mut renderer, &source, Duration::from_secs(1)),
        [0, 0, 64, 255]
    );
    // At t=2 color is blue, with opacity taken from the opaque red quarter.
    let blue = pixel(&mut renderer, &source, Duration::from_secs(2));
    assert_eq!(blue[0], 0);
    assert_eq!(blue[1], 0);
    assert!(blue[2] > 64);
    assert_eq!(
        pixel(&mut renderer, &source, Duration::from_secs(102)),
        blue
    );
    assert_eq!(
        pixel(
            &mut renderer,
            &source,
            Duration::from_millis((1_u64 << 32) + 2000)
        ),
        blue
    );
    renderer.set_environment(
        EnvironmentSettings {
            fog_enabled: true,
            fog_color: [1., 0., 1.],
            fog_start: 0.,
            fog_end: 1.,
            ..Default::default()
        },
        None,
    );
    assert_eq!(
        pixel(&mut renderer, &source, Duration::ZERO),
        [255, 0, 255, 255]
    );
}

#[test]
#[ignore = "requires original EverQuest assets and GPU"]
fn original_nest_authored_rates_and_independent_texture_reference() {
    let base = loader::default_client_dir().unwrap();
    let original = loader::load_zone(&base, "thenest").unwrap();
    let materials: Vec<_> = original
        .materials
        .iter()
        .filter(|m| m.waterfall.is_some())
        .collect();
    assert!(!materials.is_empty());
    let rates = [
        f32::from_bits(0xbdf5_c28f),
        f32::from_bits(0xbea3_d70a),
        0.,
        -0.5,
    ];
    for m in &materials {
        assert_eq!(
            m.waterfall.unwrap().map(f32::to_bits),
            rates.map(f32::to_bits)
        );
        assert_eq!(m.uv_encoding, UvEncoding::NativeTerShort2Sse2);
    }
    let triangles: usize = original
        .meshes
        .iter()
        .filter(|m| original.materials[m.material].waterfall.is_some())
        .map(|m| m.indices.len() / 3)
        .sum();
    assert_eq!(triangles, 916);
    let texture = original.texture("wtr_waterfall_tile.dds").unwrap().clone();
    assert_eq!((texture.width, texture.height), (256, 256));
    let mut renderer = Renderer::new_headless(64, 64).expect("GPU required");
    // Exact texel-aligned offsets at t=12.5: color=(-1.5,-4), alpha=(0,-6.25).
    // Build an independent source bitmap by wrapping whole source texels. The
    // two filtered source channels must reproduce the same GPU frame.
    let mut reference = texture.clone();
    for y in 0..256_usize {
        for x in 0..256_usize {
            let dst = (y * 256 + x) * 4;
            let color = (y * 256 + (x + 128) % 256) * 4;
            let alpha = (((y + 192) % 256) * 256 + x) * 4 + 3;
            reference.rgba[dst..dst + 3].copy_from_slice(&texture.rgba[color..color + 3]);
            reference.rgba[dst + 3] = texture.rgba[alpha];
        }
    }
    for uv in [[0.125, 0.25], [0.5, 0.5], [0.75, 0.875]] {
        let actual = pixel(
            &mut renderer,
            &scene(texture.clone(), rates, uv),
            Duration::from_millis(12500),
        );
        let expected = pixel(
            &mut renderer,
            &scene(reference.clone(), [0.; 4], uv),
            Duration::ZERO,
        );
        assert!(
            actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1),
            "{uv:?}: {actual:?} != {expected:?}"
        );
    }
}

#[test]
#[ignore = "requires original Nest geometry and GPU; writes captures under /tmp/openeq-waterfall-gpu"]
fn original_nest_waterfall_geometry_animates_and_closes_on_the_native_clock() {
    let base = loader::default_client_dir().unwrap();
    let source = loader::load_zone(&base, "thenest").unwrap();
    let material = source
        .materials
        .iter()
        .find(|m| m.waterfall.is_some())
        .unwrap()
        .clone();
    let texture = source.texture(&material.textures[0]).unwrap();
    let meshes = source
        .meshes
        .iter()
        .filter(|m| source.materials[m.material].waterfall.is_some())
        .cloned()
        .map(|mut m| {
            m.material = 0;
            m
        })
        .collect();
    let source = Scene::from_geometry(
        "Nest waterfall geometry".into(),
        vec![material],
        meshes,
        vec![texture],
    );
    assert_eq!(source.triangle_count(), 916);
    // Original polygon319110 center, viewed from its outward normal.
    let target = glam::Vec3::new(-165.84055, -94.167175, 227.47932);
    let normal = glam::Vec3::new(-0.46158066, -0.34836748, 0.8158329);
    let eye = target + normal * 70.;
    let direction = (target - eye).normalize();
    let camera = Camera {
        position: eye.to_array(),
        yaw: direction.x.atan2(direction.y),
        pitch: direction.z.asin(),
        fov_y: 70_f32.to_radians(),
    };
    let mut renderer = Renderer::new_headless(640, 360).expect("GPU required");
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    renderer.set_scene(&gpu);
    let mut frames = vec![];
    let path = std::path::Path::new("/tmp/openeq-waterfall-gpu");
    std::fs::create_dir_all(path).unwrap();
    for second in [0, 2, 100] {
        renderer.render_at(&gpu, &camera, Duration::from_secs(second));
        let (w, h, rgba) = renderer.read_rgba().unwrap();
        image::save_buffer(
            path.join(format!("nest-{second}.png")),
            &rgba,
            w,
            h,
            image::ColorType::Rgba8,
        )
        .unwrap();
        frames.push(rgba);
    }
    assert_eq!(
        frames[0], frames[2],
        "native effect period must close exactly"
    );
    let changed = frames[0]
        .chunks_exact(4)
        .zip(frames[1].chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 1000,
        "waterfall geometry did not visibly move: {changed} pixels"
    );
    eprintln!("Nest waterfall geometry: {changed} animated pixels, exact100-secondclosure");
}

#[test]
fn waterfall_pass_is_accounted_for_in_gpu_profiling() {
    let mut renderer = Renderer::new_headless(64, 64).expect("GPU required");
    if !renderer.enable_profiling(true) {
        return;
    }
    let source = scene(solid("flow", [128, 0, 0, 128]), [0.; 4], [0.5; 2]);
    pixel(&mut renderer, &source, Duration::ZERO);
    let mut stats = renderer.profiling_stats();
    for _ in 0..4 {
        if stats.in_flight == 0 {
            break;
        }
        renderer
            .device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        stats = renderer.profiling_stats();
    }
    assert_eq!(stats.failed, 0);
    assert_eq!(stats.in_flight, 0);
    let timing = stats.latest.unwrap();
    assert!(timing.raw_pass_ms[5] > 0.);
    let total = timing.shadow_ms
        + timing.gbuffer_ms
        + timing.lighting_ms
        + timing.transparency_ms
        + timing.waterfall_ms
        + timing.additive_ms
        + timing.particles_ms
        + timing.ui_ms;
    assert!((timing.total_ms - total).abs() < 1e-8);
    assert_eq!(timing.additive_ms, 0.);
}
