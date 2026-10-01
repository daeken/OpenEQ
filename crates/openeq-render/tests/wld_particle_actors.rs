//! Original mesh siblings of unsupported particle attachments still draw.
use glam::Vec3;
use openeq_assets::{Instance, SceneObject, loader};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

#[test]
#[ignore = "requires original PoK assets and GPU; no live connection or particle playback"]
fn original_pok_particle_linked_bodies_restore_all_366_placements() {
    let base = loader::default_client_dir().unwrap();
    let scene = loader::load_zone(&base, "poknowledge").unwrap();
    let mut renderer = Renderer::new_headless(640, 480).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            fog_enabled: false,
            ..Default::default()
        },
        None,
    );
    let captures = std::path::Path::new("/tmp/openeq-pok-particle-bodies");
    std::fs::create_dir_all(captures).unwrap();
    let mut placements = 0;
    for (name, count, polygons) in [
        ("ftorch301", 60, 168),
        ("ftorch302", 13, 128),
        ("ftorch304", 17, 160),
        ("poklamp500", 16, 482),
        ("poklamp501", 13, 1000),
        ("poklamp502", 30, 918),
        ("poksconce500", 49, 90),
        ("poktorch500", 168, 184),
    ] {
        let mut model = scene.object_model(name).unwrap();
        assert_eq!(model.triangle_count(), polygons as usize, "{name}");
        assert!(model.meshes.iter().all(|mesh| mesh.collidable));
        model.objects.push(SceneObject {
            name: name.into(),
            meshes: (0..model.meshes.len()).collect(),
            collision_meshes: (0..model.collision_meshes.len()).collect(),
        });
        model.instances = scene
            .instances
            .iter()
            .filter(|i| i.object == name)
            .cloned()
            .collect();
        assert_eq!(model.instances.len(), count as usize, "{name}");
        placements += count;
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), &model).unwrap();
        assert_eq!(
            gpu.placed_animation_count(),
            0,
            "partial static actor has no controller"
        );
        assert_eq!(
            gpu.draws
                .iter()
                .map(|d| d.index_count / 3 * d.instance_count)
                .sum::<u32>(),
            polygons * count
        );
        assert!(gpu.draws.iter().all(|draw| draw.instance_count == count));

        // Isolate the supported geometry at identity for a stable visual check.
        // Unsupported emitters must not be fabricated to make this body visible.
        model.instances = vec![Instance {
            object: name.into(),
            position: [0.; 3],
            scale: [1.; 3],
            rotation: [0., 0., 0., 1.],
        }];
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), &model).unwrap();
        let center = (gpu.bounds_min + gpu.bounds_max) * 0.5;
        let distance = (gpu.bounds_max - gpu.bounds_min).max_element().max(10.) * 1.5;
        let camera = Camera {
            position: (center - Vec3::Y * distance).to_array(),
            pitch: 0.,
            ..Default::default()
        };
        renderer.set_scene(&gpu);
        renderer.render_at(&gpu, &camera, Duration::ZERO);
        let (width, height, pixels) = renderer.read_rgba().unwrap();
        renderer.render_at(&gpu, &camera, Duration::from_secs(5));
        let later = renderer.read_rgba().unwrap().2;
        if model
            .materials
            .iter()
            .all(|material| material.textures.len() <= 1)
        {
            assert_eq!(
                pixels, later,
                "static single-frame body changes over time: {name}"
            );
        }
        model.instances.clear();
        let empty = GpuScene::build(renderer.device(), renderer.queue(), &model).unwrap();
        assert!(empty.draws.is_empty());
        renderer.set_scene(&empty);
        renderer.render_at(&empty, &camera, Duration::ZERO);
        let background = renderer.read_rgba().unwrap().2;
        let changed = pixels
            .chunks_exact(4)
            .zip(background.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        assert!(
            changed > 100,
            "too little visible {name} geometry: {changed}"
        );
        image::save_buffer(
            captures.join(format!("{name}.png")),
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
    assert_eq!(placements, 366);
}
