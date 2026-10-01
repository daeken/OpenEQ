//! Recovered MaxLava layer/UV expression under current lighting and color policy.
use openeq_assets::{
    Scene,
    loader::{self, Light, TerLava},
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

fn material(name: &str) -> Material {
    Material {
        textures: vec![name.into()],
        normal_map: None,
        water: None,
        flags: 0,
        anim_speed: 0,
        alpha_mask: false,
        transparent: false,
        additive: false,
        emissive: false,
        clamp_uv: false,
        waterfall: None,
        uv_encoding: Default::default(),
    }
}
fn texture(name: &str, color: [u8; 4]) -> Texture {
    Texture {
        name: name.into(),
        width: 1,
        height: 1,
        rgba: color.to_vec(),
    }
}
fn plane(y: f32, material: usize, uv: [f32; 2]) -> Geometry {
    Geometry {
        vertices: [
            [-10., y, -10.],
            [10., y, -10.],
            [10., y, 10.],
            [-10., y, 10.],
        ]
        .into_iter()
        .flat_map(|p| [p[0], p[1], p[2], 0., 0., -1., uv[0], uv[1]])
        .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        collidable: false,
    }
}
fn scene(top: Texture, bottom: Texture, rates: [f32; 4]) -> Scene {
    let mut m = material("top");
    m.normal_map = Some("normal".into());
    let mut source = Scene::from_geometry(
        "MaxLava".into(),
        vec![m],
        vec![plane(2., 0, [0.125, 0.5])],
        vec![
            top,
            bottom,
            texture("normal", [128, 128, 255, 255]),
            texture("red", [128, 0, 0, 128]),
        ],
    );
    source.ter_lava.insert(
        0,
        TerLava {
            top: "top".into(),
            bottom: "bottom".into(),
            normal: "normal".into(),
            rates,
        },
    );
    source
}
fn renderer() -> Renderer {
    let mut r = Renderer::new_headless(64, 64).expect("GPU required");
    r.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    r
}
fn capture(r: &mut Renderer, source: &Scene, time: Duration) -> Vec<u8> {
    let gpu = GpuScene::build(r.device(), r.queue(), source).unwrap();
    r.set_scene(&gpu);
    r.render_at(
        &gpu,
        &Camera {
            pitch: 0.,
            ..Default::default()
        },
        time,
    );
    r.read_rgba().unwrap().2
}
fn center(pixels: &[u8]) -> [u8; 4] {
    pixels[(32 * 64 + 32) * 4..][..4].try_into().unwrap()
}
fn linear(v: u8) -> f32 {
    let v = f32::from(v) / 255.;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(v: f32) -> u8 {
    let v = if v <= 0.0031308 {
        12.92 * v
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    };
    (v.clamp(0., 1.) * 255.).round() as u8
}
fn near(actual: [u8; 4], expected: [u8; 4]) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn top_alpha_mixes_lit_top_with_twice_unlit_bottom_then_fogs_the_opaque_result() {
    let mut r = renderer();
    let top = [200, 80, 32];
    let bottom = [40, 100, 120];
    for alpha in [0, 1, 15, 16, 128, 254, 255] {
        for bottom_alpha in [0, 255] {
            let source = scene(
                texture("top", [top[0], top[1], top[2], alpha]),
                texture("bottom", [bottom[0], bottom[1], bottom[2], bottom_alpha]),
                [0.; 4],
            );
            let a = f32::from(alpha) / 255.;
            let expected = std::array::from_fn(|i| {
                if i == 3 {
                    255
                } else {
                    srgb(
                        a * linear(top[i]) * [0.22, 0.24, 0.30][i]
                            + (1. - a) * 2. * linear(bottom[i]),
                    )
                }
            });
            near(center(&capture(&mut r, &source, Duration::ZERO)), expected);
        }
    }
    let source = scene(
        texture("top", [255, 0, 0, 128]),
        texture("bottom", [0, 0, 255, 0]),
        [0.; 4],
    );
    r.set_environment(
        EnvironmentSettings {
            fog_enabled: true,
            fog_color: [0.2, 0.4, 0.6],
            fog_start: 0.,
            fog_end: 1.,
            ..Default::default()
        },
        None,
    );
    near(
        center(&capture(&mut r, &source, Duration::ZERO)),
        [51, 102, 153, 255],
    );
}

#[test]
fn each_color_uses_its_own_scroll_with_packed_primary_uv_and_native_clock_wrap() {
    let stripes = |name: &str, alpha| Texture {
        name: name.into(),
        width: 256,
        height: 256,
        rgba: (0..256 * 256)
            .flat_map(|i| match (i % 256) / 64 {
                0 => [128, 0, 0, alpha],
                1 => [0, 128, 0, alpha],
                2 => [0, 0, 128, alpha],
                _ => [128, 128, 0, alpha],
            })
            .collect(),
    };
    let mut r = renderer();
    for (alpha, expected) in [(255, 1), (0, 2)] {
        let mut source = scene(
            stripes("top", alpha),
            stripes("bottom", 255),
            [0.25, 0., 0.5, 0.],
        );
        let initial = capture(&mut r, &source, Duration::ZERO);
        let moved = capture(&mut r, &source, Duration::from_secs(1));
        assert!(center(&moved)[expected] > 60);
        assert_eq!(center(&moved)[0], 0);
        assert_ne!(initial, moved);
        assert_eq!(moved, capture(&mut r, &source, Duration::from_secs(101)));
        assert_eq!(
            moved,
            capture(&mut r, &source, Duration::from_millis((1_u64 << 32) + 1000))
        );
        for v in source.meshes[0].vertices.chunks_exact_mut(8) {
            v[6] = 128.1251;
            v[7] = -128.5;
        }
        assert_eq!(
            moved,
            capture(&mut r, &source, Duration::from_secs(1)),
            "native signed SHORT2 quantization/wrap"
        );
    }
}

#[test]
fn opaque_depth_blocks_later_lava_transparency_waterfalls_additive_and_particles() {
    let mut r = renderer();
    let mut source = scene(
        texture("top", [255, 0, 0, 0]),
        texture("bottom", [0, 80, 0, 0]),
        [0.; 4],
    );
    let base = capture(&mut r, &source, Duration::ZERO);
    let mut later = material("red");
    later.emissive = true;
    source.materials.push(later);
    source.meshes.push(plane(3., 1, [0.5; 2]));
    for mode in 0..4 {
        source.materials[1].transparent = mode == 0;
        source.materials[1].waterfall = (mode == 1).then_some([0.; 4]);
        source.materials[1].additive = mode == 2;
        if mode == 3 {
            source.materials[1] = source.materials[0].clone();
            source.materials[1].textures = vec!["red".into()];
            let mut recipe = source.ter_lava[&0].clone();
            recipe.top = "red".into();
            source.ter_lava.insert(1, recipe);
        }
        assert_eq!(
            base,
            capture(&mut r, &source, Duration::ZERO),
            "far pass {mode} leaked through lava depth"
        );
        source.meshes[1] = plane(1., 1, [0.5; 2]);
        assert_ne!(
            base,
            capture(&mut r, &source, Duration::ZERO),
            "near pass {mode} was not drawn"
        );
        source.meshes[1] = plane(3., 1, [0.5; 2]);
    }
    source.ter_lava.remove(&1);
    source.materials[1] = material("red");
    source.materials[1].emissive = true;
    source.meshes[1] = plane(1., 1, [0.5; 2]);
    near(
        center(&capture(&mut r, &source, Duration::ZERO)),
        [128, 0, 0, 255],
    );
    source.meshes.pop();
    r.set_particles(&openeq_render::particles::ParticleFrame {
        textures: std::sync::Arc::new(vec![texture("particle", [255, 0, 0, 255])]),
        instances: vec![openeq_render::particles::ParticleInstance {
            position: [0., 3., 0.],
            size: [4.; 2],
            ..Default::default()
        }],
    });
    assert_eq!(base, capture(&mut r, &source, Duration::ZERO));
    let rect = openeq_ui::Rect::new(0., 0., 64., 64.);
    r.set_ui(&openeq_ui::UiFrame {
        commands: vec![openeq_ui::DrawCommand::Fill {
            rect,
            clip: rect,
            color: [0, 0, 255, 255],
        }],
        ..Default::default()
    });
    assert_eq!(
        center(&capture(&mut r, &source, Duration::ZERO)),
        [0, 0, 255, 255]
    );
}

#[test]
fn invalid_or_rebound_recipes_keep_the_ordinary_fallback_and_lava_has_a_profile_slot() {
    let mut r = renderer();
    let mut source = scene(
        texture("top", [160, 0, 0, 0]),
        texture("bottom", [0, 80, 0, 0]),
        [0.; 4],
    );
    for mode in 0..5 {
        let mut recipe = source.ter_lava[&0].clone();
        match mode {
            0 => recipe.bottom = "missing".into(),
            1 => recipe.normal = "rebound".into(),
            2 => recipe.top = "rebound".into(),
            3 => recipe.rates[0] = f32::NAN,
            _ => source.materials[0].anim_speed = 1,
        }
        source.ter_lava.insert(0, recipe);
        let gpu = GpuScene::build(r.device(), r.queue(), &source).unwrap();
        assert!(!gpu.draws[0].lava);
        let rejected = capture(&mut r, &source, Duration::ZERO);
        let metadata = std::mem::take(&mut source.ter_lava);
        assert_eq!(rejected, capture(&mut r, &source, Duration::ZERO));
        source.ter_lava = metadata;
        source.ter_lava.insert(
            0,
            TerLava {
                top: "top".into(),
                bottom: "bottom".into(),
                normal: "normal".into(),
                rates: [0.; 4],
            },
        );
        source.materials[0].anim_speed = 0;
    }
    // Lava has shadow and forward work, but no G-buffer draw commands.
    if r.enable_profiling(true) {
        capture(&mut r, &source, Duration::ZERO);
        let mut stats = r.profiling_stats();
        for _ in 0..4 {
            if stats.in_flight == 0 {
                break;
            }
            r.device()
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            stats = r.profiling_stats();
        }
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.in_flight, 0);
        let timing = stats.latest.unwrap();
        assert!(timing.raw_pass_ms[3] > 0.);
        assert_eq!(timing.raw_pass_ms[1], 0.);
        let total = timing.shadow_ms
            + timing.gbuffer_ms
            + timing.lighting_ms
            + timing.lava_ms
            + timing.transparency_ms
            + timing.waterfall_ms
            + timing.additive_ms
            + timing.particles_ms
            + timing.ui_ms;
        assert!((timing.total_ms - total).abs() < 1e-8);
    }
}

#[test]
fn empty_or_zero_count_geometry_keeps_depth_clears_without_invalid_gpu_timestamps() {
    let mut r = renderer();
    r.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            fog_enabled: false,
            fog_color: [0.2, 0.4, 0.6],
            ..Default::default()
        },
        None,
    );
    let mut empty = scene(texture("top", [0; 4]), texture("bottom", [0; 4]), [0.; 4]);
    empty.meshes.clear();
    let fresh_empty = capture(&mut r, &empty, Duration::ZERO);
    near(center(&fresh_empty), [51, 102, 153, 255]);
    let mut dirty = scene(
        texture("top", [255, 0, 0, 255]),
        texture("bottom", [0; 4]),
        [0.; 4],
    );
    dirty.ter_lava.clear(); // Ordinary opaque geometry writes G-buffer and depth.
    if !r.enable_profiling(true) {
        return;
    }
    for mode in 0..3 {
        r.enable_profiling(false);
        let preceding = capture(&mut r, &dirty, Duration::ZERO);
        assert_ne!(preceding, fresh_empty, "fixture did not dirty the targets");
        assert!(center(&preceding)[0] > center(&fresh_empty)[0]);
        assert!(r.enable_profiling(true));
        let mut source = scene(texture("top", [0; 4]), texture("bottom", [0; 4]), [0.; 4]);
        if mode == 0 {
            source.meshes.clear();
        }
        let mut gpu = GpuScene::build(r.device(), r.queue(), &source).unwrap();
        if mode == 1 {
            gpu.draws[0].index_count = 0;
        }
        if mode == 2 {
            gpu.draws[0].instance_count = 0;
        }
        r.set_scene(&gpu);
        r.render_at(
            &gpu,
            &Camera {
                pitch: 0.,
                ..Default::default()
            },
            Duration::ZERO,
        );
        let pixels = r.read_rgba().unwrap().2;
        // A nonblack empty background also distinguishes stale opaque depth
        // from cleared depth when its old albedo has already been cleared.
        assert_eq!(
            pixels, fresh_empty,
            "stale color/depth after empty mode {mode}"
        );
        let mut stats = r.profiling_stats();
        for _ in 0..4 {
            if stats.in_flight == 0 {
                break;
            }
            r.device()
                .poll(wgpu::PollType::wait_indefinitely())
                .unwrap();
            stats = r.profiling_stats();
        }
        assert_eq!(stats.failed, 0, "empty mode {mode}");
        let timing = stats.latest.unwrap();
        assert_eq!(timing.raw_pass_ms[0], 0.);
        assert_eq!(timing.raw_pass_ms[1], 0.);
        assert_eq!(timing.raw_pass_ms[3], 0.);
        assert!(timing.raw_pass_ms[2] > 0.);
    }
}

#[test]
fn lava_top_receives_zone_lights_while_the_bottom_remains_unlit() {
    let mut r = renderer();
    for alpha in [0, 255] {
        let mut source = scene(
            texture("top", [128, 128, 128, alpha]),
            texture("bottom", [80, 80, 80, 255]),
            [0.; 4],
        );
        let unlit = capture(&mut r, &source, Duration::ZERO);
        source.lights.push(Light {
            position: [0., 2., -2.],
            color: [1., 0., 0.],
            radius: 20.,
            attenuation: 1.,
            eqg_source: None,
        });
        let lit = capture(&mut r, &source, Duration::ZERO);
        if alpha == 0 {
            assert_eq!(unlit, lit);
        } else {
            assert!(center(&lit)[0] > center(&unlit)[0] + 20);
        }
    }
}

#[test]
fn zero_alpha_lava_casts_the_same_opaque_shadow_as_solid_geometry() {
    let horizontal = |center: [f32; 3], half: f32, material| Geometry {
        vertices: [[-half, -half], [half, -half], [half, half], [-half, half]]
            .into_iter()
            .flat_map(|p| {
                [
                    center[0] + p[0],
                    center[1] + p[1],
                    center[2],
                    0.,
                    0.,
                    1.,
                    0.5,
                    0.5,
                ]
            })
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material,
        collidable: false,
    };
    let mut source = scene(texture("top", [0; 4]), texture("bottom", [0; 4]), [0.; 4]);
    source.meshes[0] = horizontal([-2.745, -2.135, 5.], 4., 0);
    source.materials.push(material("red"));
    source.meshes.push(horizontal([0.; 3], 40., 1));
    let mut r = renderer();
    let target = glam::Vec3::ZERO;
    let eye = glam::Vec3::new(0., -15., 12.);
    let direction = (target - eye).normalize();
    let camera = Camera {
        position: eye.to_array(),
        yaw: direction.x.atan2(direction.y),
        pitch: direction.z.asin(),
        ..Default::default()
    };
    let render = |r: &mut Renderer, gpu: &GpuScene| {
        r.set_scene(gpu);
        r.render_at(gpu, &camera, Duration::ZERO);
        r.read_rgba().unwrap().2
    };
    let mut gpu = GpuScene::build(r.device(), r.queue(), &source).unwrap();
    assert!(gpu.draws[0].lava);
    let lava = render(&mut r, &gpu);
    gpu.draws[0].instance_count = 0;
    let unobstructed = render(&mut r, &gpu);
    // Black caster pixels are excluded: these changed nonblack pixels belong
    // to the receiving floor, so merely drawing the caster cannot pass.
    let shadowed = unobstructed
        .chunks_exact(4)
        .zip(lava.chunks_exact(4))
        .filter(|(a, b)| b[0] > 0 && a[0] > b[0] + 5)
        .count();
    assert!(
        shadowed > 20,
        "fixture did not receive lava shadow: {shadowed}"
    );
    source.ter_lava.clear();
    let solid = GpuScene::build(r.device(), r.queue(), &source).unwrap();
    assert_eq!(
        lava,
        render(&mut r, &solid),
        "top alpha cut opaque shadow/depth"
    );
}

#[test]
#[ignore = "requires original Nest assets and GPU"]
fn original_nest_lava_keeps_exact_bindings_and_512_opaque_triangles() {
    let source = loader::load_zone(loader::default_client_dir().unwrap(), "thenest").unwrap();
    assert_eq!(source.ter_lava.len(), 4);
    for lava in source.ter_lava.values() {
        assert_eq!(
            (&*lava.top, &*lava.bottom, &*lava.normal),
            ("kl_lavaTop_c.dds", "kl_lavaBottom_c.dds", "kl_lava_n.dds")
        );
        assert_eq!(
            lava.rates.map(f32::to_bits),
            [0x3e99_999a, 0, 0x3e4c_cccd, 0]
        );
    }
    let r = renderer();
    let gpu = GpuScene::build(r.device(), r.queue(), &source).unwrap();
    let draws: Vec<_> = gpu.draws.iter().filter(|d| d.lava).collect();
    assert_eq!(draws.iter().map(|d| d.index_count / 3).sum::<u32>(), 512);
    assert!(
        draws
            .iter()
            .all(|d| !d.transparent && !d.waterfall && !d.additive)
    );
}
