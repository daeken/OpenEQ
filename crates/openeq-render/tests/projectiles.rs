//! Original item projectiles use the same world depth, lighting and material
//! passes as characters, without becoming selectable/nameplated actors.
use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer, actors::ActorRenderer, projectiles::ProjectileState,
};

fn wall(renderer: &Renderer, y: f32) -> GpuScene {
    let scene = Scene::from_geometry(
        "projectile backdrop".into(),
        vec![Material {
            textures: vec!["backdrop".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            additive: false,
            emissive: true,
            clamp_uv: false,
            uv_encoding: Default::default(),
        }],
        vec![Geometry {
            vertices: vec![
                -100., y, -100., 0., -1., 0., 0., 0., 100., y, -100., 0., -1., 0., 1., 0., 100., y,
                100., 0., -1., 0., 1., 1., -100., y, 100., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "backdrop".into(),
            width: 1,
            height: 1,
            rgba: vec![20, 28, 40, 255],
        }],
    );
    GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap()
}

#[test]
#[ignore = "requires original item assets and GPU; writes /tmp/openeq-original-projectiles.png"]
fn original_projectile_models_render_move_occlude_and_clear_without_actor_bounds() {
    let base = openeq_assets::loader::default_client_dir().expect("original client directory");
    let mut renderer = Renderer::new_headless(720, 400).unwrap();
    let mut actors = ActorRenderer::load(&base, "poknowledge").unwrap();
    let backdrop = wall(&renderer, 20.);
    let camera = Camera {
        position: [0., -9., 1.],
        pitch: 0.,
        ..Default::default()
    };
    renderer.set_scene(&backdrop);
    renderer.render_with_actors(&backdrop, &camera, &actors.draws());
    let baseline = renderer.read_rgba().unwrap().2;
    let mut states = vec![
        ProjectileState {
            id: 1,
            model_name: "IT10".into(),
            position: [-3., 0., 1.],
            direction: [1., 0., 0.],
        },
        ProjectileState {
            id: 2,
            model_name: "IT10".into(),
            position: [0., 0., 0.],
            direction: [0., 0., 1.],
        },
        ProjectileState {
            id: 3,
            model_name: "IT11504".into(),
            position: [3., 0., 1.],
            direction: [1., 0., 0.],
        },
    ];
    let stats = actors.update_projectiles(&renderer, &states);
    assert_eq!(stats.rendered, 3);
    assert_eq!(stats.cached_models, 2);
    assert_eq!(actors.rendered_instances, 0);
    assert!(actors.bounds().is_empty());
    assert!(actors.sockets().is_empty());
    assert_eq!(actors.draws().len(), 2);
    renderer.render_with_actors(&backdrop, &camera, &actors.draws());
    let (_, _, pixels) = renderer.read_rgba().unwrap();
    assert!(
        pixels
            .chunks_exact(4)
            .zip(baseline.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
            > 100
    );
    image::save_buffer(
        "/tmp/openeq-original-projectiles.png",
        &pixels,
        720,
        400,
        image::ColorType::Rgba8,
    )
    .unwrap();
    states[0].position[0] -= 1.;
    actors.update_projectiles(&renderer, &states);
    renderer.render_with_actors(&backdrop, &camera, &actors.draws());
    assert_ne!(renderer.read_rgba().unwrap().2, pixels);

    let occluder = wall(&renderer, -5.);
    renderer.set_scene(&occluder);
    renderer.render_with_actors(&occluder, &camera, &actors.draws());
    assert_eq!(
        renderer.read_rgba().unwrap().2,
        baseline,
        "projectile leaked through opaque geometry"
    );
    actors.update_projectiles(&renderer, &[]);
    assert_eq!(actors.projectile_stats().rendered, 0);
    assert!(actors.draws().is_empty());
    renderer.set_scene(&backdrop);
    renderer.render_with_actors(&backdrop, &camera, &actors.draws());
    assert_eq!(renderer.read_rgba().unwrap().2, baseline);
}
