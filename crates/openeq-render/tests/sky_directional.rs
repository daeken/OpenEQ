//! Authored sun/moon RGB selection, retaining current fixed light direction.
use openeq_assets::{
    Scene,
    environment::{
        SkyAssets, SkyColorMapInputs, SkyColorMapLayout, SkyColorMapProvenance, SkyColorMapSource,
        load_sky,
    },
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer,
    environment::{DEFAULT_AMBIENT, DEFAULT_DIRECTIONAL, EnvironmentSettings},
};
use std::time::Duration;

fn sky(fraction: f32) -> SkyAssets {
    let mut rgba = vec![0; 4096];
    for (row, color) in [
        (0, [0, 40, 80, 0]),
        (1, [100, 20, 0, 255]),
        (3, [20, 20, 20, 0]),
    ] {
        let index = 4 * (row * 32 + 31);
        rgba[index..index + 4].copy_from_slice(&color);
    }
    SkyAssets {
        weather: "directional fixture".into(),
        color_map: Texture {
            name: "sky".into(),
            width: 32,
            height: 32,
            rgba,
        },
        color_map_layout: SkyColorMapLayout::OriginalDome,
        color_map_provenance: Some(SkyColorMapProvenance {
            color_set: "fixture".into(),
            day_tick: (fraction * 65536.) as u32,
            day_fraction_bits: fraction.to_bits(),
            inputs: SkyColorMapInputs::Single(SkyColorMapSource {
                key_index: 0,
                color_map: "fixture".into(),
                path: "fixture".into(),
                start_tick: 0,
                transition_ticks: 0,
            }),
        }),
        cloud_texture: None,
        cloud_color_map: None,
        cloud_color_map_layout: SkyColorMapLayout::FullTexture,
        cloud_color_map_provenance: None,
        cloud_velocity: 0.,
    }
}
fn settings() -> EnvironmentSettings {
    EnvironmentSettings {
        zone_type: Some(2),
        sky_enabled: false,
        ..Default::default()
    }
}

#[test]
fn exact_selector_boundaries_use_sample_fraction_not_truncated_day_tick() {
    let env = settings();
    for (bits, sun) in [
        (0, false),
        (0x3e7c_71c6, false),
        (0x3e7c_71c7, true),
        (0x3e7c_71c8, true),
        (0x3f00_0000, true),
        (0x3f43_8e38, true),
        (0x3f43_8e39, true),
        (0x3f43_8e3a, false),
        (0x3f80_0000, false),
    ] {
        let source = sky(f32::from_bits(bits));
        assert_eq!(
            env.directional_color(Some(&source)),
            if sun {
                [0., 40. / 255., 80. / 255.]
            } else {
                [100. / 255., 20. / 255., 0.]
            },
            "{bits:08x}"
        );
    }
    // The opposite selections really have identical truncated ticks.
    for (a, b) in [(0x3e7c_71c6, 0x3e7c_71c7), (0x3f43_8e39, 0x3f43_8e3a)] {
        assert_eq!(
            sky(f32::from_bits(a))
                .color_map_provenance
                .unwrap()
                .day_tick,
            sky(f32::from_bits(b))
                .color_map_provenance
                .unwrap()
                .day_tick
        );
    }
    for kind in 0..=255 {
        let env = EnvironmentSettings {
            zone_type: Some(kind),
            ..settings()
        };
        assert_eq!(
            env.directional_color(Some(&sky(0.5))),
            if matches!(kind, 1 | 2 | 5) {
                [0., 40. / 255., 80. / 255.]
            } else {
                DEFAULT_DIRECTIONAL
            }
        );
    }
}

#[test]
fn rgb_has_no_ambient_floor_or_alpha_multiplier_and_bad_inputs_fall_back() {
    let env = settings();
    let mut source = sky(0.5);
    for alpha in [0, 1, 127, 255] {
        source.color_map.rgba[4 * 31 + 3] = alpha;
        assert_eq!(
            env.directional_color(Some(&source)),
            [0., 40. / 255., 80. / 255.]
        );
    }
    assert_eq!(env.directional_color(None), DEFAULT_DIRECTIONAL);
    assert_eq!(
        EnvironmentSettings::default().directional_color(Some(&source)),
        DEFAULT_DIRECTIONAL
    );
    for fraction in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1, 1.1] {
        assert_eq!(
            env.directional_color(Some(&sky(fraction))),
            DEFAULT_DIRECTIONAL
        );
    }
    source.color_map_provenance = None;
    assert_eq!(env.directional_color(Some(&source)), DEFAULT_DIRECTIONAL);
    source = sky(0.5);
    source.color_map_layout = SkyColorMapLayout::FullTexture;
    assert_eq!(env.directional_color(Some(&source)), DEFAULT_DIRECTIONAL);
    source = sky(0.5);
    source.color_map.rgba.pop();
    assert_eq!(env.directional_color(Some(&source)), DEFAULT_DIRECTIONAL);
}

fn surface() -> Scene {
    Scene::from_geometry(
        "directional receiver".into(),
        vec![Material {
            textures: vec!["white".into()],
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
        }],
        vec![Geometry {
            vertices: [
                [-10., 2., -10.],
                [10., 2., -10.],
                [10., 2., 10.],
                [-10., 2., 10.],
            ]
            .into_iter()
            .flat_map(|p| [p[0], p[1], p[2], 0., 0., 1., 0.5, 0.5])
            .collect(),
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "white".into(),
            width: 1,
            height: 1,
            rgba: vec![255; 4],
        }],
    )
}
fn capture(r: &mut Renderer, gpu: &GpuScene) -> [u8; 4] {
    r.render_at(
        gpu,
        &Camera {
            position: [0.; 3],
            yaw: 0.,
            pitch: 0.,
            fov_y: 90f32.to_radians(),
        },
        Duration::ZERO,
    );
    let (w, h, pixels) = r.read_rgba().unwrap();
    let offset = ((h / 2 * w + w / 2) * 4) as usize;
    pixels[offset..offset + 4].try_into().unwrap()
}
fn near(pixel: [u8; 4], ambient: [f32; 3], directional: [f32; 3]) {
    let lambert = glam::Vec3::new(-0.45, 0.82, 0.35).normalize().y;
    for i in 0..3 {
        let linear = (ambient[i] + directional[i] * lambert).clamp(0., 1.);
        let srgb = if linear <= 0.0031308 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1. / 2.4) - 0.055
        };
        assert!(
            pixel[i].abs_diff((srgb * 255.).round() as u8) <= 2,
            "{pixel:?} channel{i} expected{srgb}"
        );
    }
}

#[test]
fn gpu_switches_sun_moon_color_and_resets_missing_zone_inputs() {
    let mut renderer = Renderer::new_headless(64, 64).unwrap();
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &surface()).unwrap();
    renderer.set_scene(&gpu);
    for (time, color) in [
        (0.5, [0., 40. / 255., 80. / 255.]),
        (0., [100. / 255., 20. / 255., 0.]),
    ] {
        renderer.set_environment(settings(), Some(&sky(time)));
        near(capture(&mut renderer, &gpu), [20. / 255.; 3], color);
    }
    let above = capture(&mut renderer, &gpu);
    renderer.set_view_liquid(Some(openeq_assets::liquid_regions::LiquidKind::Lava));
    assert_ne!(above, capture(&mut renderer, &gpu));
    renderer.set_view_liquid(None);
    assert_eq!(above, capture(&mut renderer, &gpu));
    renderer.set_environment(settings(), None);
    near(
        capture(&mut renderer, &gpu),
        DEFAULT_AMBIENT,
        DEFAULT_DIRECTIONAL,
    );
    renderer.set_environment(settings(), Some(&sky(0.)));
    renderer.set_environment(
        EnvironmentSettings {
            zone_type: Some(4),
            ..settings()
        },
        Some(&sky(0.5)),
    );
    near(
        capture(&mut renderer, &gpu),
        DEFAULT_AMBIENT,
        DEFAULT_DIRECTIONAL,
    );
}

#[test]
#[ignore = "requires original EverQuest assets and GPU; no live character"]
fn original_pok_night_surface_uses_moon_swatch() {
    let base = openeq_assets::loader::default_client_dir().expect("original assets");
    let mut renderer = Renderer::new_headless(64, 64).unwrap();
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &surface()).unwrap();
    renderer.set_scene(&gpu);
    let mut env = EnvironmentSettings::for_zone("poknowledge");
    env.sky_enabled = false;
    env.fog_enabled = false;
    let mut pixels = Vec::new();
    for fraction in [0., 0.5] {
        let sky = load_sky(&base, "poknowledge", fraction).unwrap();
        let row = if fraction == 0. { 1 } else { 0 };
        let offset = 4 * (row * 32 + 31);
        let directional = std::array::from_fn(|i| f32::from(sky.color_map.rgba[offset + i]) / 255.);
        let ambient = std::array::from_fn(|i| {
            f32::from(sky.color_map.rgba[4 * (3 * 32 + 31) + i].max(20)) / 255.
        });
        renderer.set_environment(env, Some(&sky));
        let pixel = capture(&mut renderer, &gpu);
        near(pixel, ambient, directional);
        pixels.push(pixel);
    }
    assert!(
        pixels[0][0] < pixels[1][0],
        "night must be darker: {pixels:?}"
    );
}

#[test]
fn directional_changes_do_not_recolor_emissive_surfaces_or_empty_sky() {
    let mut renderer = Renderer::new_headless(64, 64).unwrap();
    let mut source = surface();
    source.materials[0].emissive = true;
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    renderer.set_scene(&gpu);
    for fraction in [0., 0.5] {
        renderer.set_environment(settings(), Some(&sky(fraction)));
        assert_eq!(capture(&mut renderer, &gpu), [255; 4]);
    }
    let empty = Scene::from_geometry("sky".into(), vec![], vec![], vec![]);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &empty).unwrap();
    renderer.set_scene(&gpu);
    let env = EnvironmentSettings {
        sky_enabled: true,
        ..settings()
    };
    renderer.set_environment(env, Some(&sky(0.)));
    capture(&mut renderer, &gpu);
    let moon_frame = renderer.read_rgba().unwrap().2;
    renderer.set_environment(env, Some(&sky(0.5)));
    capture(&mut renderer, &gpu);
    assert_eq!(moon_frame, renderer.read_rgba().unwrap().2);
}
