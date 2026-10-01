//! Synthetic GPU checks for fractional character alpha, independent of client assets.
use openeq_assets::{
    Scene,
    loader::Light,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuActor, GpuScene, Renderer, environment::EnvironmentSettings};

fn plane(color: [u8; 4], y: f32, transparent: bool, emissive: bool) -> Scene {
    Scene::from_geometry(
        "alpha fixture".into(),
        vec![Material {
            textures: vec!["solid".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: transparent,
            transparent,
            additive: false,
            emissive,
            clamp_uv: false,
            uv_encoding: Default::default(),
        }],
        vec![Geometry {
            vertices: vec![
                -10., y, -10., 0., -1., 0., 0., 0., 10., y, -10., 0., -1., 0., 1., 0., 10., y, 10.,
                0., -1., 0., 1., 1., -10., y, 10., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "solid".into(),
            width: 1,
            height: 1,
            rgba: color.to_vec(),
        }],
    )
}

fn upload(renderer: &Renderer, scene: &Scene) -> GpuScene {
    GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap()
}

fn actor(renderer: &Renderer, color: [u8; 4], y: f32, emissive: bool) -> GpuActor {
    renderer.prepare_actor(upload(renderer, &plane(color, y, true, emissive)))
}

fn center(renderer: &mut Renderer, scene: &GpuScene, actors: &[&GpuActor]) -> [u8; 4] {
    renderer.set_scene(scene);
    renderer.render_with_actors(
        scene,
        &Camera {
            pitch: 0.,
            ..Default::default()
        },
        actors,
    );
    let (width, height, rgba) = renderer.read_rgba().unwrap();
    let offset = ((height / 2 * width + width / 2) * 4) as usize;
    rgba[offset..offset + 4].try_into().unwrap()
}

fn linear(value: u8) -> f32 {
    let value = f32::from(value) / 255.;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn fractional_alpha_is_visible_depth_tested_order_independent_and_below_ui() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let blue = upload(&renderer, &plane([0, 0, 255, 255], 4., false, true));
    for alpha in [0, 16, 64, 141, 192, 255] {
        let red = actor(&renderer, [255, 0, 0, alpha], 2., true);
        let pixel = center(&mut renderer, &blue, &[&red]);
        let expected = f32::from(alpha) / 255.;
        assert!(
            (linear(pixel[0]) - expected).abs() < 0.012,
            "alpha{alpha}: {pixel:?}"
        );
        assert!(
            (linear(pixel[2]) - (1. - expected)).abs() < 0.012,
            "alpha{alpha}: {pixel:?}"
        );
        assert_eq!(pixel[1], 0);
    }

    let red = actor(&renderer, [255, 0, 0, 64], 2., true);
    let green = actor(&renderer, [0, 255, 0, 96], 2.2, true);
    let a = center(&mut renderer, &blue, &[&red, &green]);
    let b = center(&mut renderer, &blue, &[&green, &red]);
    assert!(
        a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 1),
        "draw order changed alpha: {a:?} {b:?}"
    );
    assert!(
        a[0] > 50 && a[1] > 50 && a[2] > 50,
        "layers were lost: {a:?}"
    );

    let wall = upload(&renderer, &plane([0, 255, 0, 255], 1., false, true));
    assert_eq!(
        center(&mut renderer, &wall, &[&red]),
        [0, 255, 0, 255],
        "transparent surface leaked through opaque world"
    );
    let solid = actor(&renderer, [255, 0, 0, 255], 2., true);
    assert_eq!(
        center(&mut renderer, &blue, &[&green, &solid]),
        [255, 0, 0, 255],
        "opaque alpha interior failed to occlude rear layers"
    );

    renderer.resize(48, 48);
    let resized = center(&mut renderer, &blue, &[&red]);
    assert!((linear(resized[0]) - 64. / 255.).abs() < 0.012);
    let rect = openeq_ui::Rect::new(0., 0., 48., 48.);
    renderer.set_ui(&openeq_ui::UiFrame {
        commands: vec![openeq_ui::DrawCommand::Fill {
            rect,
            clip: rect,
            color: [0, 255, 0, 255],
        }],
        ..Default::default()
    });
    assert_eq!(
        center(&mut renderer, &blue, &[&red]),
        [0, 255, 0, 255],
        "transparency covered UI"
    );
}

#[test]
fn fractional_surfaces_share_opaque_lighting_emission_and_fog() {
    let Ok(mut renderer) = Renderer::new_headless(64, 64) else {
        return;
    };
    let mut background = plane([0, 0, 0, 255], 4., false, true);
    background.lights.push(Light {
        position: [0., 0., 0.],
        color: [0.6, 0.1, 0.05],
        radius: 24.,
        attenuation: 1.,
        eqg_source: None,
    });
    let world = upload(&renderer, &background);
    let opaque = actor(&renderer, [128, 128, 128, 255], 2., false);
    let partial = actor(&renderer, [128, 128, 128, 64], 2., false);
    let full = center(&mut renderer, &world, &[&opaque]);
    let alpha = center(&mut renderer, &world, &[&partial]);
    for i in 0..3 {
        assert!(
            (linear(alpha[i]) - linear(full[i]) * 64. / 255.).abs() < 0.012,
            "forward/deferred light mismatch: opaque{full:?} partial{alpha:?}"
        );
    }
    assert!(
        full[0] > full[2] + 10,
        "fixture did not exercise colored zone light: {full:?}"
    );
    renderer.set_environment(
        EnvironmentSettings {
            fog_enabled: true,
            fog_start: 0.,
            fog_end: 1.,
            fog_color: [0.2, 0.4, 0.6],
            ..Default::default()
        },
        None,
    );
    let emissive = actor(&renderer, [255, 0, 0, 64], 2., true);
    let fogged = center(&mut renderer, &world, &[&emissive]);
    assert!(
        fogged[..3]
            .iter()
            .zip([51, 102, 153])
            .all(|(a, b)| a.abs_diff(b) <= 2),
        "transparent emissive surface ignored zone fog: {fogged:?}"
    );
}
