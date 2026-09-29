//! Atmosphere tests use synthetic scenes so they do not require client assets.
use openeq_assets::{
    Scene,
    environment::{SkyAssets, SkyColorMapLayout},
    liquid_regions::LiquidKind,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};

fn wall(y: f32) -> Scene {
    let material = Material {
        textures: vec!["red".into()],
        normal_map: None,
        water: None,
        flags: 0,
        anim_speed: 0,
        alpha_mask: false,
        transparent: false,
        emissive: true,
    };
    let mut vertices = Vec::new();
    for [x, z] in [[-100., -100.], [100., -100.], [100., 100.], [-100., 100.]] {
        vertices.extend_from_slice(&[x, y, z, 0., -1., 0., 0., 0.]);
    }
    Scene::from_geometry(
        "fog fixture".into(),
        vec![material],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "red".into(),
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        }],
    )
}
fn center(renderer: &mut Renderer) -> [u8; 3] {
    let (w, h, pixels) = renderer.read_rgba().unwrap();
    let offset = ((h / 2 * w + w / 2) * 4) as usize;
    pixels[offset..offset + 3].try_into().unwrap()
}

#[test]
fn server_fog_reaches_its_authored_color_even_on_emissive_geometry() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        eprintln!("no GPU; skipping fog pixel test");
        return;
    };
    let scene = wall(100.);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    let camera = Camera {
        pitch: 0.,
        ..Default::default()
    };
    renderer.render(&gpu, &camera);
    let near = center(&mut renderer);
    assert!(
        near[0] > 245 && near[1] < 5 && near[2] < 5,
        "fixture did not render red: {near:?}"
    );
    let settings = EnvironmentSettings {
        fog_color: [0., 0., 1.],
        fog_start: 40.,
        fog_end: 80.,
        fog_enabled: true,
        ..Default::default()
    };
    renderer.set_environment(settings, None);
    renderer.render(&gpu, &camera);
    let fogged = center(&mut renderer);
    assert!(
        fogged[0] < 5 && fogged[1] < 5 && fogged[2] > 245,
        "emissive geometry escaped fog: {fogged:?}"
    );
    // Moving closer than the start distance must reveal the original material.
    renderer.render(
        &gpu,
        &Camera {
            position: [0., 80., 0.],
            ..camera
        },
    );
    let close = center(&mut renderer);
    assert!(
        close[0] > 245 && close[2] < 5,
        "fog did not use world distance: {close:?}"
    );
}

#[test]
fn sky_is_attached_to_world_direction_not_screen_or_camera_position() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        eprintln!("no GPU; skipping sky pixel test");
        return;
    };
    let scene = wall(-100.);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    let assets = SkyAssets {
        weather: "test".into(),
        color_map: Texture {
            name: "sky gradient".into(),
            width: 2,
            height: 2,
            rgba: vec![
                255, 0, 0, 255, 255, 0, 0, 255, 0, 0, 255, 255, 0, 0, 255, 255,
            ],
        },
        color_map_layout: SkyColorMapLayout::FullTexture,
        cloud_texture: None,
        cloud_color_map: None,
        cloud_color_map_layout: SkyColorMapLayout::FullTexture,
        cloud_velocity: 0.,
    };
    renderer.set_environment(EnvironmentSettings::default(), Some(&assets));
    let camera = Camera {
        pitch: 0.35,
        ..Default::default()
    };
    renderer.render(&gpu, &camera);
    let original = center(&mut renderer);
    renderer.render(
        &gpu,
        &Camera {
            position: [450., 800., 75.],
            ..camera
        },
    );
    let translated = center(&mut renderer);
    assert!(
        original
            .iter()
            .zip(translated)
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "camera translation moved sky: {original:?} vs {translated:?}"
    );
    renderer.render(
        &gpu,
        &Camera {
            pitch: 1.1,
            ..camera
        },
    );
    let elevated = center(&mut renderer);
    assert!(
        elevated[2] > original[2] + 30 && elevated[0] + 30 < original[0],
        "camera pitch did not change sampled sky elevation: {original:?} vs {elevated:?}"
    );
}

#[test]
fn liquid_fog_replaces_distant_world_and_sky_then_restores_zone_settings() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        eprintln!("no GPU; skipping liquid fog pixel test");
        return;
    };
    let scene = wall(180.);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    let settings = EnvironmentSettings {
        fog_color: [0.7, 0.3, 0.5],
        fog_start: 90.,
        fog_end: 300.,
        fog_enabled: true,
        ..Default::default()
    };
    renderer.set_environment(settings, None);
    let camera = Camera {
        pitch: 0.,
        ..Default::default()
    };
    renderer.render(&gpu, &camera);
    let original_world = center(&mut renderer);
    let sky_camera = Camera {
        yaw: std::f32::consts::PI,
        pitch: 0.6,
        ..camera
    };
    renderer.render(&gpu, &sky_camera);
    let original_sky = center(&mut renderer);
    for (liquid, expected) in [
        (LiquidKind::Water, [20_u8, 56, 77]),
        (LiquidKind::FreezingWater, [51, 82, 102]),
        (LiquidKind::OpaqueWater, [20, 41, 23]),
        (LiquidKind::Lava, [128, 31, 4]),
    ] {
        renderer.set_view_liquid(Some(liquid));
        for view in [camera, sky_camera] {
            renderer.render(&gpu, &view);
            let actual = center(&mut renderer);
            assert!(
                actual.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
                "{liquid:?}: expected {expected:?}, got {actual:?}"
            );
        }
        renderer.set_view_liquid(None);
        renderer.render(&gpu, &camera);
        assert_eq!(
            center(&mut renderer),
            original_world,
            "zone fog failed to restore"
        );
        renderer.render(&gpu, &sky_camera);
        assert_eq!(
            center(&mut renderer),
            original_sky,
            "zone sky failed to restore"
        );
    }
}

#[test]
fn native_sky_helper_swatches_and_pole_ring_colors_never_reach_sky_pixels() {
    let Ok(mut renderer) = Renderer::new_headless(96, 96) else {
        return;
    };
    let scene = Scene::from_geometry("sky only".into(), vec![], vec![], vec![]);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    let expected = [40_u8, 90, 170];
    let mut rgba = Vec::new();
    for y in 0..32 {
        for x in 0..32 {
            let valid = x < 31 && y < 30 && ((y != 0 && y != 29) || x == 0);
            rgba.extend_from_slice(if valid {
                &[40, 90, 170, 255]
            } else {
                &[255, 0, 255, 255]
            });
        }
    }
    let table = Texture {
        name: "native sky color fixture".into(),
        width: 32,
        height: 32,
        rgba,
    };
    let mut assets = SkyAssets {
        weather: "fixture".into(),
        color_map: table.clone(),
        color_map_layout: SkyColorMapLayout::OriginalDome,
        cloud_texture: None,
        cloud_color_map: None,
        cloud_color_map_layout: SkyColorMapLayout::FullTexture,
        cloud_velocity: 0.,
    };
    for clouds in [false, true] {
        if clouds {
            assets.cloud_texture = Some(Texture {
                name: "opaque cloud".into(),
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            });
            assets.cloud_color_map = Some(table.clone());
            assets.cloud_color_map_layout = SkyColorMapLayout::OriginalDome;
        }
        renderer.set_environment(EnvironmentSettings::default(), Some(&assets));
        for pitch in [0.4_f32, 1.0, std::f32::consts::FRAC_PI_2 - 0.0001] {
            for yaw in [0_f32, 0.8, 2.0, 3.5, 5.0] {
                renderer.render(
                    &gpu,
                    &Camera {
                        pitch,
                        yaw,
                        ..Default::default()
                    },
                );
                let (_, _, pixels) = renderer.read_rgba().unwrap();
                for actual in pixels.chunks_exact(4) {
                    assert!(
                        actual[..3]
                            .iter()
                            .zip(expected)
                            .all(|(a, b)| a.abs_diff(b) <= 1),
                        "reserved sky color leaked at pitch={pitch} yaw={yaw} clouds={clouds}: {actual:?}"
                    );
                }
            }
        }
    }
    // Upload processing must preserve the original source asset for inspection.
    assert_eq!(&assets.color_map.rgba[31 * 4..32 * 4], &[255, 0, 255, 255]);
}

#[test]
#[ignore = "requires original Plane of Knowledge sky assets and GPU"]
fn original_pok_sky_has_no_rainbow_wedge_when_looking_up() {
    let base = openeq_assets::loader::default_client_dir().expect("original assets");
    let sky = openeq_assets::environment::load_sky(&base, "poknowledge", 0.5).unwrap();
    let mut renderer = Renderer::new_headless(160, 120).unwrap();
    let scene = Scene::from_geometry("sky only".into(), vec![], vec![], vec![]);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
    renderer.set_environment(EnvironmentSettings::for_zone("poknowledge"), Some(&sky));
    for pitch in [45_f32, 70., 89.99] {
        for yaw in [0_f32, 90., 180., 270.] {
            renderer.render(
                &gpu,
                &Camera {
                    pitch: pitch.to_radians(),
                    yaw: yaw.to_radians(),
                    ..Default::default()
                },
            );
            let (_, _, pixels) = renderer.read_rgba().unwrap();
            let rainbow = pixels
                .chunks_exact(4)
                .filter(|p| {
                    (p[1] > 180 && p[0] < 60 && p[2] < 90) || (p[0] > 180 && p[1] > 90 && p[2] < 50)
                })
                .count();
            assert_eq!(rainbow, 0, "PoK rainbow at pitch={pitch} yaw={yaw}");
        }
    }
}
