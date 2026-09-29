//! Physical-only WLD triangles must not affect color, depth, shadows or bounds.
use openeq_assets::{
    Scene,
    collision::CollisionWorld,
    loader,
    mesh::{CollisionGeometry, Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};

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
    assert_eq!(a.bounds_min, b.bounds_min);
    assert_eq!(a.bounds_max, b.bounds_max);
    let draws = |gpu: &GpuScene| {
        gpu.draws
            .iter()
            .map(|draw| {
                (
                    draw.index_start,
                    draw.index_count,
                    draw.base_vertex,
                    draw.instance_start,
                    draw.instance_count,
                    draw.transparent,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(draws(a), draws(b));
}

#[test]
#[ignore = "requires GPU"]
fn invisible_wall_blocks_body_and_camera_without_changing_pixels_or_gpu_bounds() {
    let mut renderer = Renderer::new_headless(128, 128).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let mut scene = Scene::from_geometry(
        "visible backdrop".into(),
        vec![Material {
            textures: vec!["red".into()],
            normal_map: None,
            water: None,
            flags: 1,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            emissive: true,
            clamp_uv: false,
        }],
        vec![Geometry {
            vertices: vec![
                -10., 10., -10., 0., -1., 0., 0., 0., 10., 10., -10., 0., -1., 0., 1., 0., 10.,
                10., 10., 0., -1., 0., 1., 1., -10., 10., 10., 0., -1., 0., 0., 1.,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: true,
        }],
        vec![Texture {
            name: "red".into(),
            width: 1,
            height: 1,
            rgba: vec![255, 0, 0, 255],
        }],
    );
    let camera = Camera {
        pitch: 0.,
        ..Default::default()
    };
    let (before_gpu, before) = capture(&mut renderer, &scene, &camera);
    assert!(
        before
            .chunks_exact(4)
            .filter(|pixel| pixel[0] > 240 && pixel[1] < 10)
            .count()
            > 8000
    );
    scene.collision_meshes.push(CollisionGeometry {
        positions: vec![
            [-100., 3., -100.],
            [100., 3., -100.],
            [100., 3., 100.],
            [-100., 3., 100.],
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
    });
    let world = CollisionWorld::build(&scene);
    let blocked = world.move_player([0., 0., 0.], [0., 6., 0.], 1., 6., 0.);
    assert!(blocked[1] < 3.);
    assert!(world.clip_camera([0.; 3], [0., 6., 0.], 0.1)[1] < 3.);
    let (after_gpu, after) = capture(&mut renderer, &scene, &camera);
    assert_same_upload(&before_gpu, &after_gpu);
    assert_eq!(before, after, "invisible wall changed visible pixels");
}

#[test]
#[ignore = "requires original Timorous assets and GPU; writes /tmp/openeq-timorous-collision"]
fn original_timorous_collision_changes_leave_rendering_unchanged() {
    let base = loader::default_client_dir().expect("original client assets");
    let mut scene = loader::load_zone(base, "timorous").unwrap();
    assert_eq!(
        scene
            .collision_meshes
            .iter()
            .map(|mesh| mesh.indices.len() / 3)
            .sum::<usize>(),
        12_438
    );
    // Freeze texture animation so the A/B comparison isolates geometry.
    for material in &mut scene.materials {
        material.textures.truncate(1);
        material.anim_speed = 0;
    }
    let mut renderer = Renderer::new_headless(960, 540).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let camera = Camera {
        position: [-11326.276, -2283.372, 16.],
        yaw: (-9.65625f32).atan2(2.5981445),
        pitch: -0.12,
        ..Default::default()
    };
    let (after_gpu, after) = capture(&mut renderer, &scene, &camera);
    let restored_count = CollisionWorld::build(&scene).triangle_count();
    scene.collision_meshes.clear();
    for object in &mut scene.objects {
        object.collision_meshes.clear();
    }
    let visible_count = CollisionWorld::build(&scene).triangle_count();
    assert_eq!(restored_count - visible_count, 12_438);
    let (before_gpu, before) = capture(&mut renderer, &scene, &camera);
    assert_same_upload(&before_gpu, &after_gpu);
    assert_eq!(
        before, after,
        "original hidden faces changed rendered pixels"
    );
    let directory = std::path::Path::new("/tmp/openeq-timorous-collision");
    std::fs::create_dir_all(directory).unwrap();
    for (name, rgba) in [("before.png", before), ("after.png", after)] {
        image::RgbaImage::from_raw(960, 540, rgba)
            .unwrap()
            .save(directory.join(name))
            .unwrap();
    }
}
