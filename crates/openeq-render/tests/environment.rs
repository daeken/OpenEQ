//! Atmosphere tests use synthetic scenes so they do not require client assets.
use openeq_assets::{
    Scene,
    environment::SkyAssets,
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
        cloud_texture: None,
        cloud_color_map: None,
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
