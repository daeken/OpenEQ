//! EQG physical classification must leave render inputs, pixels and bounds intact.
#[path = "../../openeq-assets/tests/support/eqg_collision.rs"]
mod support;
use openeq_assets::{Scene, collision::CollisionWorld, loader};
use openeq_render::{
    Camera, GpuScene, Renderer,
    doors::{DoorRenderer, DoorState},
    environment::EnvironmentSettings,
};
use support::*;

fn renderer(width: u32, height: u32) -> Renderer {
    let mut renderer = Renderer::new_headless(width, height).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    renderer
}
fn capture(renderer: &mut Renderer, scene: &Scene, camera: &Camera) -> (GpuScene, Vec<u8>) {
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap();
    renderer.set_scene(&gpu);
    renderer.render(&gpu, camera);
    let (_, _, rgba) = renderer.read_rgba().unwrap();
    (gpu, rgba)
}
fn assert_same_upload(a: &GpuScene, b: &GpuScene) {
    assert_eq!(a.vertices.size(), b.vertices.size());
    assert_eq!(a.indices.size(), b.indices.size());
    assert_eq!(a.instances.size(), b.instances.size());
    assert_eq!(a.water_materials.size(), b.water_materials.size());
    assert_eq!(a.light_count, b.light_count);
    assert_eq!(a.bounds_min, b.bounds_min);
    assert_eq!(a.bounds_max, b.bounds_max);
    let draws = |gpu: &GpuScene| {
        gpu.draws
            .iter()
            .map(|d| {
                (
                    d.index_start,
                    d.index_count,
                    d.base_vertex,
                    d.instance_start,
                    d.instance_count,
                    d.transparent,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(draws(a), draws(b));
}

#[test]
#[ignore = "requires GPU"]
fn loaded_eqg_collision_changes_leave_pixels_and_gpu_bounds_unchanged() {
    let mut source = model(true);
    source.materials.insert(0, material("Opaque.fx", None));
    quad(
        &mut source,
        [
            [-10., 10., -10.],
            [10., 10., -10.],
            [10., 10., 10.],
            [-10., 10., 10.],
        ],
        0,
        0x8000_0000,
    );
    // A large hidden wall would change bounds and cover the backdrop if it
    // accidentally entered the draw channel. A drawn bit-0 floor stays visible.
    quad(
        &mut source,
        [
            [-100., 3., -100.],
            [100., 3., -100.],
            [100., 3., 100.],
            [-100., 3., 100.],
        ],
        u32::MAX,
        2,
    );
    quad(&mut source, floor(-5.), 0, 0x8000_0001);
    let fixture = Fixture::new(&[("ground.ter", &source)], &[]);
    let mut scene = fixture.scene();
    let fingerprint = draw_fingerprints(&scene);
    let world = CollisionWorld::build(&scene);
    assert_eq!(world.triangle_count(), 4);
    assert!(world.move_player([0.; 3], [0., 6., 0.], 1., 6., 0.)[1] < 3.);
    assert!(world.clip_camera([0.; 3], [0., 6., 0.], 0.1)[1] < 3.);
    let mut renderer = renderer(128, 128);
    let camera = Camera {
        pitch: 0.,
        ..Default::default()
    };
    let (after_gpu, after) = capture(&mut renderer, &scene, &camera);
    assert!(
        after
            .chunks_exact(4)
            .filter(|p| p[0] > p[1].saturating_add(30) && p[2] > p[1].saturating_add(30))
            .count()
            > 8000,
        "missing-texture backdrop must visibly fill the capture"
    );
    restore_legacy_collision(&mut scene);
    assert_eq!(draw_fingerprints(&scene), fingerprint);
    let before = CollisionWorld::build(&scene);
    assert_eq!(before.clip_camera([0.; 3], [0., 6., 0.], 0.), [0., 6., 0.]);
    let (before_gpu, before) = capture(&mut renderer, &scene, &camera);
    assert_same_upload(&before_gpu, &after_gpu);
    assert_eq!(
        before, after,
        "physical-only faces or passability changed rendered pixels"
    );
}

#[test]
#[ignore = "requires GPU"]
fn loaded_hidden_eqg_door_and_lift_move_collision_without_rendering() {
    let source = hidden_door();
    let fixture = Fixture::new(&[("hidden.mod", &source)], &[]);
    let renderer = renderer(64, 64);
    let mut doors = DoorRenderer::load(&fixture.0, "fixture").unwrap();
    let mut state = DoorState {
        id: 1,
        name: "hidden".into(),
        position: [30., 40., 50.],
        heading: 128.,
        size: 150,
        open_type: 59,
        parameter: 50,
        ..Default::default()
    };
    doors.update(&renderer, &[state.clone()], 0.);
    assert!(doors.draws().is_empty());
    assert_eq!(doors.rendered_instances, 0);
    assert_eq!(doors.collision_world().triangle_count(), 4);
    assert_eq!(
        doors
            .collision_world()
            .ground_height(36., 40., 50., 0.1, 0.1),
        Some(50.)
    );
    assert!(
        doors
            .collision_world()
            .move_player([27., 40., 50.], [6., 0., 0.], 1., 6., 0.)[0]
            < 30.
    );
    state.state = 1;
    doors.update(&renderer, &[state.clone()], 1.);
    doors.update(&renderer, &[state.clone()], 2.);
    assert_eq!(
        doors
            .collision_world()
            .ground_height(36., 40., 75., 0.1, 0.1),
        Some(75.)
    );
    assert_eq!(
        doors
            .collision_world()
            .ground_height(36., 40., 50., 0.1, 0.1),
        None
    );
    assert_eq!(
        doors.take_platform_displacement([36., 40., 50.], true),
        [0., 0., 25.]
    );
    assert_eq!(
        doors.take_platform_displacement([36., 40., 50.], true),
        [0.; 3]
    );
    assert!(
        (doors
            .collision_world()
            .move_player([27., 40., 50.], [6., 0., 0.], 1., 6., 0.)[0]
            - 33.)
            .abs()
            < 0.01
    );
    // Sliding travel exposes the bounds computed from valid hidden triangles:
    // authored extent20 × size1.5 × existing travel factor4 =120. Invalid far,
    // NaN and out-of-range faces in this archive must not inflate that extent.
    state.open_type = 25;
    doors.update(&renderer, &[state], 4.);
    assert_eq!(
        doors
            .collision_world()
            .ground_height(156., 40., 50., 0.1, 0.1),
        Some(50.)
    );
    assert_eq!(
        doors
            .collision_world()
            .ground_height(36., 40., 50., 0.1, 0.1),
        None
    );
    assert!(doors.draws().is_empty());
    doors.update(&renderer, &[], 5.);
    assert_eq!(doors.collision_world().triangle_count(), 0);
}

#[test]
#[ignore = "requires original Bloodfields archive and GPU; writes /tmp/openeq-bloodfields-collision"]
fn original_bloodfields_collision_changes_leave_frozen_rendering_unchanged() {
    let base = loader::default_client_dir().expect("original EverQuest assets");
    assert!(
        base.join("bloodfields.eqg").is_file(),
        "original Bloodfields archive required"
    );
    let mut scene = loader::load_zone(base, "bloodfields").unwrap();
    assert_eq!(draw_fingerprints(&scene), BLOODFIELDS_DRAW_FINGERPRINTS);
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 324800);
    // EQG Bloodfields has no water materials. Freeze any texture animation and
    // disable the animated sky; keep the actual lights and normal shadow pass.
    assert!(scene.materials.iter().all(|m| m.water.is_none()));
    for material in &mut scene.materials {
        material.textures.truncate(1);
        material.anim_speed = 0;
    }
    let mut renderer = renderer(960, 540);
    let camera = Camera {
        position: [-780.9877, -1255.4523, -926.1253],
        yaw: 55.069f32.to_radians(),
        pitch: -5f32.to_radians(),
        ..Default::default()
    };
    let (after_gpu, after) = capture(&mut renderer, &scene, &camera);
    restore_legacy_collision(&mut scene);
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 320817);
    let (before_gpu, before) = capture(&mut renderer, &scene, &camera);
    assert_same_upload(&before_gpu, &after_gpu);
    assert_eq!(
        before, after,
        "EQG collision changes altered original Bloodfields pixels"
    );
    let path = std::path::Path::new("/tmp/openeq-bloodfields-collision");
    std::fs::create_dir_all(path).unwrap();
    for (name, rgba) in [("before.png", before), ("after.png", after)] {
        image::RgbaImage::from_raw(960, 540, rgba)
            .unwrap()
            .save(path.join(name))
            .unwrap();
    }
}
