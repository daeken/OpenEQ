//! Original lamp rotation reaches the placed-actor GPU path.
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
                .filter(|polygon| polygon.collidable)
                .flat_map(|polygon| {
                    [polygon.a, polygon.b, polygon.c]
                        .map(|index| mesh.vertices[index as usize].map(f32::to_bits))
                })
        })
        .collect()
}

#[test]
#[ignore = "requires original Swamp of No Hope assets and GPU; no audio or live character"]
fn original_swamp_lamp_rotates_closes_and_preserves_static_stem() {
    let base = loader::default_client_dir().unwrap();
    let scene = loader::load_zone(&base, "swampofnohope").unwrap();
    let mut lamp = scene.object_model("krlamp101").unwrap();
    // Freeze textures so changed pixels prove that rotating vertices reach
    // the GPU independently of material animation.
    for material in &mut lamp.materials {
        material.textures.truncate(1);
    }
    let source = lamp.wld_object_sources["krlamp101"].clone();
    assert_eq!(
        source.animation_period().unwrap(),
        Duration::from_millis(1600)
    );
    assert!(source.particle_attachments.is_empty());
    assert_eq!(lamp.triangle_count(), 185);
    let initial = source.sample_animation(Duration::ZERO).unwrap();
    let physical = physical_vertices(&initial);
    assert_eq!(physical.len(), 0);
    lamp.objects.push(SceneObject {
        name: "krlamp101".into(),
        meshes: (0..lamp.meshes.len()).collect(),
        collision_meshes: (0..lamp.collision_meshes.len()).collect(),
    });
    lamp.instances = scene
        .instances
        .iter()
        .filter(|instance| instance.object == "krlamp101")
        .cloned()
        .collect();
    assert_eq!(lamp.instances.len(), 36);
    let mut renderer = Renderer::new_headless(480, 480).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let placed = GpuScene::build(renderer.device(), renderer.queue(), &lamp).unwrap();
    assert_eq!(placed.placed_animation_count(), 1);
    assert_eq!(
        placed
            .draws
            .iter()
            .map(|draw| draw.index_count / 3 * draw.instance_count)
            .sum::<u32>(),
        185 * 36
    );
    drop(placed);
    lamp.instances = vec![Instance {
        object: "krlamp101".into(),
        position: [0.; 3],
        scale: [1.; 3],
        rotation: [0., 0., 0., 1.],
    }];
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for part in &initial {
        for point in &part.vertices {
            min = min.min(Vec3::from(*point));
            max = max.max(Vec3::from(*point));
        }
    }
    let center = (min + max) * 0.5;
    let camera = Camera {
        position: (center - Vec3::X * (max - min).max_element() * 1.8).to_array(),
        yaw: std::f32::consts::FRAC_PI_2,
        pitch: 0.,
        ..Default::default()
    };
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &lamp).unwrap();
    assert_eq!(gpu.placed_animation_count(), 1);
    renderer.set_scene(&gpu);
    renderer.render_at(&gpu, &camera, Duration::ZERO);
    let (_, _, first_pixels) = renderer.read_rgba().unwrap();
    let directory = std::path::Path::new("/tmp/openeq-swampofnohope-animated-lamp");
    std::fs::create_dir_all(directory).unwrap();
    for millis in [0, 100, 400, 800, 1400, 1600] {
        let time = Duration::from_millis(millis);
        let sampled = source.sample_animation(time).unwrap();
        assert_eq!(
            sampled[0].vertices, initial[0].vertices,
            "stem moved at {millis}ms"
        );
        assert_eq!(
            physical_vertices(&sampled),
            physical,
            "collision moved at {millis}ms"
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
            .zip(first_pixels.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        if millis == 0 || millis == 1600 {
            assert_eq!(changed, 0, "native1600ms loop must close exactly");
        } else {
            assert!(
                changed > 30,
                "no visible lamp motion at {millis}ms: {changed}"
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
        eprintln!("KRLAMP101 {millis}ms: {changed} changed pixels,static stem unchanged");
    }
}
