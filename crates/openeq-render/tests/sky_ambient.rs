//! Recovered packed ambient input under the existing normal-vision lighting policy.
use openeq_assets::{
    Scene,
    environment::{SkyAssets, SkyColorMapLayout, load_sky},
    liquid_regions::LiquidKind,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer,
    environment::{DEFAULT_AMBIENT, EnvironmentSettings},
};
use std::time::Duration;

fn sky(rgb: [u8; 4]) -> SkyAssets {
    let mut rgba = vec![0; 4096];
    let pixel = 4 * (3 * 32 + 31);
    rgba[pixel..pixel + 4].copy_from_slice(&rgb);
    SkyAssets {
        weather: "ambient fixture".into(),
        color_map: Texture {
            name: "sky".into(),
            width: 32,
            height: 32,
            rgba,
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
fn settings(zone_type: u8) -> EnvironmentSettings {
    EnvironmentSettings {
        zone_type: Some(zone_type),
        sky_enabled: false,
        ..Default::default()
    }
}

#[test]
fn exact_native_type_gate_and_byte_floor_ignore_alpha_and_sky_visibility() {
    let authored = sky([0, 40, 120, 0]);
    let expected = [20. / 255., 40. / 255., 120. / 255.];
    for kind in 0..=255 {
        let env = settings(kind);
        assert_eq!(
            env.normal_vision_ambient(Some(&authored)),
            if matches!(kind, 1 | 2 | 5) {
                expected
            } else {
                DEFAULT_AMBIENT
            }
        );
    }
    assert_eq!(
        EnvironmentSettings::default().normal_vision_ambient(Some(&authored)),
        DEFAULT_AMBIENT
    );
    let env = settings(2);
    let opaque_alpha = sky([0, 40, 120, 255]);
    assert_eq!(env.normal_vision_ambient(Some(&opaque_alpha)), expected);
    assert_eq!(
        env.with_view_liquid(Some(LiquidKind::Water))
            .normal_vision_ambient(Some(&authored)),
        expected
    );
    for byte in 0..=255 {
        let map = sky([byte, byte, byte, byte]);
        assert_eq!(
            env.normal_vision_ambient(Some(&map)),
            [f32::from(byte.max(20)) / 255.; 3]
        );
    }
    for zone in [
        "poknowledge",
        "gfaydark",
        "thenest",
        "bazaar",
        "thundercrest",
    ] {
        let snapshot = openeq_assets::environment::load_zone_environment(zone).unwrap();
        assert_eq!(
            EnvironmentSettings::for_zone(zone).zone_type,
            Some(snapshot.time_type)
        );
    }
}

#[test]
fn unsupported_or_missing_table_uses_current_fallback() {
    let env = settings(1);
    assert_eq!(env.normal_vision_ambient(None), DEFAULT_AMBIENT);
    let original = sky([128; 4]);
    let mut invalid = original.clone();
    invalid.color_map_layout = SkyColorMapLayout::FullTexture;
    assert_eq!(env.normal_vision_ambient(Some(&invalid)), DEFAULT_AMBIENT);
    invalid = original.clone();
    invalid.color_map.width = 31;
    assert_eq!(env.normal_vision_ambient(Some(&invalid)), DEFAULT_AMBIENT);
    invalid = original;
    invalid.color_map.rgba.pop();
    assert_eq!(env.normal_vision_ambient(Some(&invalid)), DEFAULT_AMBIENT);
}

fn surface() -> Scene {
    Scene::from_geometry(
        "ambient receiver".into(),
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
            // Downward normal excludes the fixed sun from this isolated receiver.
            .flat_map(|p| [p[0], p[1], p[2], 0., 0., -1., 0.5, 0.5])
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
fn srgb(v: f32) -> u8 {
    ((if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1. / 2.4) - 0.055
    }) * 255.)
        .round() as u8
}
fn near(pixel: [u8; 4], linear: [f32; 3]) {
    for i in 0..3 {
        assert!(
            pixel[i].abs_diff(srgb(linear[i])) <= 2,
            "pixel={pixel:?}, expected={linear:?}"
        );
    }
    assert_eq!(pixel[3], 255);
}

#[test]
fn gpu_ambient_refresh_and_zone_fallback_never_retain_stale_color() {
    let mut r = Renderer::new_headless(64, 64).unwrap();
    let source = surface();
    let gpu = GpuScene::build(r.device(), r.queue(), &source).unwrap();
    r.set_scene(&gpu);
    let env = settings(2);
    r.set_environment(env, Some(&sky([0, 40, 120, 0])));
    near(capture(&mut r, &gpu), [20. / 255., 40. / 255., 120. / 255.]);
    r.set_environment(env, Some(&sky([80, 0, 255, 255])));
    near(capture(&mut r, &gpu), [80. / 255., 20. / 255., 1.]);
    r.set_environment(env, None);
    near(capture(&mut r, &gpu), DEFAULT_AMBIENT);
    r.set_environment(env, Some(&sky([80, 0, 255, 255])));
    let mut unsupported = sky([255; 4]);
    unsupported.color_map_layout = SkyColorMapLayout::FullTexture;
    r.set_environment(env, Some(&unsupported));
    near(capture(&mut r, &gpu), DEFAULT_AMBIENT);
    r.set_environment(env, Some(&sky([80, 0, 255, 255])));
    r.set_environment(settings(4), Some(&sky([0, 40, 120, 0])));
    near(capture(&mut r, &gpu), DEFAULT_AMBIENT);
    // Camera-medium presentation must restore lighting as well as fog on exit.
    r.set_environment(env, Some(&sky([0, 40, 120, 0])));
    let above = capture(&mut r, &gpu);
    r.set_view_liquid(Some(LiquidKind::Lava));
    assert_ne!(above, capture(&mut r, &gpu));
    r.set_view_liquid(None);
    assert_eq!(above, capture(&mut r, &gpu));
}

#[test]
#[ignore = "requires original EverQuest assets and GPU; no live character"]
fn original_pok_day_keys_drive_surface_ambient() {
    let base = openeq_assets::loader::default_client_dir().expect("original assets");
    let mut r = Renderer::new_headless(64, 64).unwrap();
    let gpu = GpuScene::build(r.device(), r.queue(), &surface()).unwrap();
    r.set_scene(&gpu);
    let mut env = EnvironmentSettings::for_zone("poknowledge");
    env.sky_enabled = false;
    env.fog_enabled = false;
    let mut pixels = Vec::new();
    for fraction in [0., 0.25, 0.5, 0.75] {
        let assets = load_sky(&base, "poknowledge", fraction).unwrap();
        let index = 4 * (3 * 32 + 31);
        let expected = std::array::from_fn(|channel| {
            f32::from(assets.color_map.rgba[index + channel].max(20)) / 255.
        });
        r.set_environment(env, Some(&assets));
        let pixel = capture(&mut r, &gpu);
        near(pixel, expected);
        pixels.push(pixel);
    }
    assert_ne!(
        pixels[0], pixels[2],
        "original day and night lighting must differ"
    );
    assert_ne!(pixels[1], pixels[0], "sampled dawn must affect the surface");
}

#[test]
fn emissive_surfaces_and_empty_sky_do_not_inherit_ambient_swatches() {
    let mut renderer = Renderer::new_headless(64, 64).unwrap();
    let mut source = surface();
    source.materials[0].emissive = true;
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    renderer.set_scene(&gpu);
    for rgba in [[0; 4], [255; 4]] {
        renderer.set_environment(settings(2), Some(&sky(rgba)));
        assert_eq!(capture(&mut renderer, &gpu), [255; 4]);
    }
    let empty = Scene::from_geometry("empty sky".into(), vec![], vec![], vec![]);
    let empty_gpu = GpuScene::build(renderer.device(), renderer.queue(), &empty).unwrap();
    renderer.set_scene(&empty_gpu);
    let env = EnvironmentSettings {
        sky_enabled: true,
        ..settings(2)
    };
    renderer.set_environment(env, Some(&sky([0; 4])));
    capture(&mut renderer, &empty_gpu);
    let dark = renderer.read_rgba().unwrap().2;
    renderer.set_environment(env, Some(&sky([255; 4])));
    capture(&mut renderer, &empty_gpu);
    assert_eq!(dark, renderer.read_rgba().unwrap().2);
}
