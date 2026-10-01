//! Original placed actors must reach instanced GPU draws with their assembled pose.
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
    renderer.render(&gpu, &camera);
    let (width, height, visible) = renderer.read_rgba().unwrap();
    tree.instances.clear();
    let empty = GpuScene::build(renderer.device(), renderer.queue(), &tree).unwrap();
    assert!(
        empty.draws.is_empty(),
        "unplaced components must not leak into the world"
    );
    renderer.set_scene(&empty);
    renderer.render(&empty, &camera);
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
}
