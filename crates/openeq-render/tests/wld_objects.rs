//! Original placed actors must reach instanced GPU draws with their assembled pose.
use glam::{Mat4, Quat, Vec3};
use openeq_assets::{Instance, SceneObject, loader};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};

#[test]
#[ignore = "requires original City of Mist assets and GPU; writes /tmp/openeq-citymist-objects"]
fn original_citymist_tree_assembles_visible_branches_and_all_fifty_instances() {
    let base = loader::default_client_dir().expect("original client assets");
    let scene = loader::load_zone(&base, "citymist").unwrap();
    let mut tree = scene.object_model("jntree103").unwrap();
    assert_eq!(tree.triangle_count(), 72);
    assert_eq!(tree.wld_object_sources["jntree103"].parts.len(), 7);
    tree.objects.push(SceneObject {
        name: "jntree103".into(),
        meshes: (0..tree.meshes.len()).collect(),
        collision_meshes: (0..tree.collision_meshes.len()).collect(),
    });
    tree.instances = scene
        .instances
        .iter()
        .filter(|instance| instance.object == "jntree103")
        .cloned()
        .collect();
    assert_eq!(tree.instances.len(), 50);
    let mut renderer = Renderer::new_headless(640, 480).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
    assert_eq!(
        gpu.draws
            .iter()
            .map(|draw| draw.index_count / 3 * draw.instance_count)
            .sum::<u32>(),
        72 * 50
    );
    assert!(gpu.draws.iter().all(|draw| draw.instance_count == 50));
    // Independently expand the actual original vertices into world space.
    // Definition-space bounds used to omit these placements entirely.
    let mut actual_min = Vec3::splat(f32::INFINITY);
    let mut actual_max = Vec3::splat(f32::NEG_INFINITY);
    for placement in &tree.instances {
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::from(placement.scale),
            Quat::from_array(placement.rotation),
            Vec3::from(placement.position),
        );
        for geometry in &tree.meshes {
            for &index in &geometry.indices {
                let start = index as usize * openeq_assets::mesh::VERTEX_STRIDE;
                let p = transform
                    .transform_point3(Vec3::from_slice(&geometry.vertices[start..start + 3]));
                actual_min = actual_min.min(p);
                actual_max = actual_max.max(p);
            }
        }
    }
    assert!(gpu.bounds_min.cmple(actual_min + Vec3::splat(0.001)).all());
    assert!(gpu.bounds_max.cmpge(actual_max - Vec3::splat(0.001)).all());
    assert!(
        (actual_max - actual_min).x > 1000.,
        "fixture covers dispersed original placements"
    );

    // Show one full assembled actor at a fixed camera; missing actor lookup
    // previously produced no draw at all, while raw components missed the pose.
    tree.instances = vec![Instance {
        object: "jntree103".into(),
        position: [0.; 3],
        scale: [1.; 3],
        rotation: [0., 0., 0., 1.],
    }];
    let camera = Camera {
        position: [0., -200., 75.],
        pitch: 0.,
        ..Default::default()
    };
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
    renderer.set_scene(&gpu);
    renderer.render_at(&gpu, &camera, std::time::Duration::ZERO);
    let (width, height, visible) = renderer.read_rgba().unwrap();
    tree.instances.clear();
    let empty = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
    assert!(
        empty.draws.is_empty(),
        "unplaced components must not leak into the world"
    );
    assert_eq!(empty.bounds_min, Vec3::ZERO);
    assert_eq!(empty.bounds_max, Vec3::ZERO);
    renderer.set_scene(&empty);
    renderer.render_at(&empty, &camera, std::time::Duration::ZERO);
    let (_, _, background) = renderer.read_rgba().unwrap();
    let changed = visible
        .chunks_exact(4)
        .zip(background.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        changed > 500,
        "assembled actor has too few visible pixels: {changed}"
    );
    let directory = std::path::Path::new("/tmp/openeq-citymist-objects");
    std::fs::create_dir_all(directory).unwrap();
    image::save_buffer(
        directory.join("assembled-tree.png"),
        &visible,
        width,
        height,
        image::ColorType::Rgba8,
    )
    .unwrap();

    // Exact authored poses are a diagnostic, not a native playback timeline.
    // Rebuild only this isolated test scene; production still uses frame zero.
    let source = tree.wld_object_sources["jntree103"].clone();
    let skeleton = source.skeleton.as_ref().unwrap();
    let archive = openeq_assets::pfs::Archive::open(base.join("citymist_obj.s3d")).unwrap();
    let wld = openeq_assets::wld::Wld::open(&archive, &source.wld_filename).unwrap();
    tree.instances.push(Instance {
        object: "jntree103".into(),
        position: [0.; 3],
        scale: [1.; 3],
        rotation: [0., 0., 0., 1.],
    });
    for frame in 0..4 {
        let selection = skeleton
            .tracks
            .iter()
            .map(|track| {
                if track.definition.frames.len() == 1 {
                    0
                } else {
                    frame
                }
            })
            .collect::<Vec<_>>();
        let parts = source.sample_authored_frames(&selection).unwrap();
        let (materials, mut meshes) = openeq_assets::mesh::bake_wld_meshes(&wld, &parts);
        // object_model preserves palette masking in decoded texture aliases.
        // Match the fresh bake back to those already extracted materials.
        let remap = materials
            .into_iter()
            .map(|mut material| {
                if material.alpha_mask {
                    for texture in &mut material.textures {
                        texture.push_str("#masked");
                    }
                }
                tree.materials
                    .iter()
                    .position(|existing| *existing == material)
                    .expect("sampling must retain every source material")
            })
            .collect::<Vec<_>>();
        for mesh in &mut meshes {
            mesh.material = remap[mesh.material];
        }
        tree.meshes = meshes;
        tree.objects[0].meshes = (0..tree.meshes.len()).collect();
        assert_eq!(
            openeq_assets::collision::CollisionWorld::build(&tree).triangle_count(),
            42
        );
        let sampled_gpu = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
        assert_eq!(
            sampled_gpu
                .draws
                .iter()
                .map(|draw| draw.index_count / 3 * draw.instance_count)
                .sum::<u32>(),
            72
        );
        renderer.set_scene(&sampled_gpu);
        renderer.render_at(&sampled_gpu, &camera, std::time::Duration::ZERO);
        let (_, _, pixels) = renderer.read_rgba().unwrap();
        image::save_buffer(
            directory.join(format!("authored-frame-{frame}.png")),
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .unwrap();
        let changed = pixels
            .chunks_exact(4)
            .zip(visible.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        if frame == 0 {
            assert!(
                pixels == visible,
                "diagnostic first pose must preserve the loaded actor: {changed} pixels changed"
            );
        } else {
            assert!(
                changed > 50,
                "authored frame {frame} must visibly change branches: {changed}"
            );
        }
    }
}
