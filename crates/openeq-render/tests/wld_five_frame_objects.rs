//! Native compressed five-frame playback must reach actual placed-object draws.
use glam::Vec3;
use openeq_assets::{Instance, SceneObject, loader, wld::Mesh};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

fn physical_vertices(parts: &[Mesh]) -> Vec<[u32; 3]> {
    parts
        .iter()
        .flat_map(|mesh| {
            mesh.polygons
                .iter()
                .filter(|p| p.collidable)
                .flat_map(|p| [p.a, p.b, p.c].map(|i| mesh.vertices[i as usize].map(f32::to_bits)))
        })
        .collect()
}

#[test]
#[ignore = "requires original Dreadlands assets and GPU"]
fn original_dreadlands_tree_moves_loops_and_preserves_its_trunk() {
    let base = loader::default_client_dir().unwrap();
    let scene = loader::load_zone(&base, "dreadlands").unwrap();
    let mut tree = scene.object_model("tree105").unwrap();
    let source = tree.wld_object_sources["tree105"].clone();
    assert_eq!(source.animation_period().unwrap(), Duration::from_secs(5));
    assert_eq!(tree.triangle_count(), 48);
    let initial = source.sample_animation(Duration::ZERO).unwrap();
    let physical = physical_vertices(&initial);
    assert_eq!(physical.len(), 18 * 3);
    tree.objects.push(SceneObject {
        name: "tree105".into(),
        meshes: (0..tree.meshes.len()).collect(),
        collision_meshes: (0..tree.collision_meshes.len()).collect(),
    });
    tree.instances = scene
        .instances
        .iter()
        .filter(|i| i.object == "tree105")
        .cloned()
        .collect();
    assert_eq!(tree.instances.len(), 34);
    let mut renderer = Renderer::new_headless(480, 480).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let placed = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
    assert_eq!(placed.placed_animation_count(), 1);
    assert_eq!(
        placed
            .draws
            .iter()
            .map(|d| d.index_count / 3 * d.instance_count)
            .sum::<u32>(),
        48 * 34
    );
    drop(placed);
    tree.instances = vec![Instance {
        object: "tree105".into(),
        position: [0.; 3],
        scale: [1.; 3],
        rotation: [0., 0., 0., 1.],
    }];
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in &initial {
        for position in &part.vertices {
            min = min.min(Vec3::from(*position));
            max = max.max(Vec3::from(*position));
        }
    }
    let center = (min + max) * 0.5;
    let distance = (max - min).max_element() * 1.8;
    let camera = Camera {
        position: (center - Vec3::Y * distance).to_array(),
        pitch: 0.,
        ..Default::default()
    };
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
    assert_eq!(gpu.placed_animation_count(), 1);
    renderer.set_scene(&gpu);
    renderer.render_at(&gpu, &camera, Duration::ZERO);
    let (_, _, initial_pixels) = renderer.read_rgba().unwrap();
    let directory = std::path::Path::new("/tmp/openeq-dreadlands-five-frame-tree");
    std::fs::create_dir_all(directory).unwrap();
    for millis in [0, 1250, 2500, 3750, 5000] {
        let time = Duration::from_millis(millis);
        let sampled = source.sample_animation(time).unwrap();
        assert_eq!(
            physical_vertices(&sampled),
            physical,
            "trunk moved at {millis}ms"
        );
        for part in &sampled {
            for point in &part.vertices {
                let point = Vec3::from(*point);
                assert!(gpu.bounds_min.cmple(point).all());
                assert!(gpu.bounds_max.cmpge(point).all());
            }
        }
        renderer.render_at(&gpu, &camera, time);
        let (_, _, pixels) = renderer.read_rgba().unwrap();
        let changed = pixels
            .chunks_exact(4)
            .zip(initial_pixels.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        if millis == 0 || millis == 5000 {
            assert_eq!(changed, 0, "native five-second loop must close exactly");
        } else {
            assert!(
                changed > 30,
                "no visible branch motion at {millis}ms: {changed}"
            );
        }
        image::save_buffer(
            directory.join(format!("{millis}.png")),
            &pixels,
            480,
            480,
            image::ColorType::Rgba8,
        )
        .unwrap();
        eprintln!(
            "Dreadlands TREE105 {millis}ms: {changed} changed pixels, stationary18-triangle trunk"
        );
    }
}
