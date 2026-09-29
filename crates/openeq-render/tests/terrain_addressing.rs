//! Baked tiles have distinct opposite edges; sampling must not wrap between them.
use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};
use std::time::Duration;

fn edge_scene(axis: usize, high: bool, alpha: u8, solid: bool) -> Scene {
    let edge = if high { 0.9999 } else { 0.0001 };
    let mut uv = [0.5; 2];
    uv[axis] = edge;
    let mut vertices = Vec::new();
    for [x, z] in [[-10., -10.], [10., -10.], [10., 10.], [-10., 10.]] {
        vertices.extend([x, 4., z, 0., -1., 0., uv[0], uv[1]]);
    }
    let mut rgba = Vec::new();
    for y in 0..128 {
        for x in 0..128 {
            let at_high_edge = [x, y][axis] >= 64;
            rgba.extend(if solid || at_high_edge == high {
                [0, 255, 0, alpha]
            } else {
                [255, 0, 0, alpha]
            });
        }
    }
    Scene::from_geometry(
        "tile edge".into(),
        vec![Material {
            textures: vec!["tile".into()],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: alpha < 255,
            emissive: true,
            clamp_uv: true,
        }],
        vec![Geometry {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "tile".into(),
            width: 128,
            height: 128,
            rgba,
        }],
    )
}

fn capture(renderer: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap();
    renderer.set_scene(&gpu);
    renderer.render_at(
        &gpu,
        &Camera {
            pitch: 0.,
            ..Default::default()
        },
        Duration::ZERO,
    );
    renderer.read_rgba().unwrap().2
}

#[test]
#[ignore = "requires GPU"]
fn tile_edges_do_not_sample_the_opposite_side_in_opaque_or_blended_passes() {
    let mut renderer = Renderer::new_headless(64, 64).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    for alpha in [255, 128] {
        for axis in 0..2 {
            for high in [false, true] {
                let actual = capture(&mut renderer, &edge_scene(axis, high, alpha, false));
                let expected = capture(&mut renderer, &edge_scene(axis, high, alpha, true));
                assert!(
                    actual == expected,
                    "opposite edge bled across axis{axis}, high{high}, alpha{alpha}"
                );
                let mut repeating = edge_scene(axis, high, alpha, false);
                repeating.materials[0].clamp_uv = false;
                let wrapped = capture(&mut renderer, &repeating);
                let changed = actual
                    .iter()
                    .zip(&wrapped)
                    .filter(|(a, b)| a.abs_diff(**b) > 10)
                    .count();
                assert!(
                    changed > 1000,
                    "ordinary repeating material must still blend opposite edges"
                );
            }
        }
    }
}

#[test]
#[ignore = "requires original Feerrott2 assets and GPU; writes /tmp/openeq-terrain-addressing"]
fn original_feerrott_tile_edges_change_without_moving_geometry_or_interior_uvs() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    let mut scene = openeq_assets::load_zone(base, "feerrott2").unwrap();
    assert_eq!(scene.materials.iter().filter(|m| m.clamp_uv).count(), 329);
    // Match the terrain-only diagnostic view that exposed the grid lines.
    scene.instances.clear();
    let mut renderer = Renderer::new_headless(960, 540).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let target = glam::Vec3::new(-1544., 760., -30.);
    let position = glam::Vec3::new(-1544., 744., 60.);
    let direction = target - position;
    let camera = Camera {
        position: position.to_array(),
        yaw: direction.x.atan2(direction.y),
        pitch: direction.z.atan2(direction.truncate().length()),
        ..Default::default()
    };
    let mut draw = |scene: &Scene| {
        let gpu = GpuScene::build_with_terrain(
            renderer.device(),
            renderer.queue(),
            scene,
            openeq_render::terrain::TerrainMode::Baked,
        )
        .unwrap();
        renderer.set_scene(&gpu);
        renderer.render_at(&gpu, &camera, Duration::from_secs(12));
        (gpu, renderer.read_rgba().unwrap().2)
    };
    let (after_gpu, after) = draw(&scene);
    for material in &mut scene.materials {
        material.clamp_uv = false;
    }
    let (before_gpu, before) = draw(&scene);
    assert_eq!(before_gpu.vertices.size(), after_gpu.vertices.size());
    assert_eq!(before_gpu.indices.size(), after_gpu.indices.size());
    assert_eq!(before_gpu.bounds_min, after_gpu.bounds_min);
    assert_eq!(before_gpu.bounds_max, after_gpu.bounds_max);
    let changed = before
        .chunks_exact(4)
        .zip(after.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 2))
        .count();
    eprintln!(
        "Feerrott2 terrain edge addressing changes {changed}/{} pixels",
        before.len() / 4
    );
    assert!(
        changed > 100,
        "fixture must expose tile edge color wrapping"
    );
    assert!(
        changed < before.len() / 4 / 10,
        "interior tile colors must remain unchanged"
    );
    let dir = std::path::Path::new("/tmp/openeq-terrain-addressing");
    std::fs::create_dir_all(dir).unwrap();
    for (name, rgba) in [("before.png", before), ("after.png", after)] {
        image::RgbaImage::from_raw(960, 540, rgba)
            .unwrap()
            .save(dir.join(name))
            .unwrap();
    }
}
