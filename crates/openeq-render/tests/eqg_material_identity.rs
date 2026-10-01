//! Original Housegarden wall binding, compared with the former ID-keyed lookup.
use openeq_assets::{Scene, loader, pfs::Archive, texture::Texture};
use openeq_render::{Camera, GpuScene, Renderer, environment::EnvironmentSettings};

#[test]
#[ignore = "requires original Housegarden assets and GPU; writes /tmp/openeq-housegarden-materials"]
fn original_stone_wall_uses_its_ordinal_material_on_the_gpu() {
    let base = loader::default_client_dir().expect("original assets");
    let source = loader::load_zone(&base, "housegarden").unwrap();
    let mut renderer = Renderer::new_headless(640, 360).unwrap();
    renderer.set_environment(
        EnvironmentSettings {
            sky_enabled: false,
            ..Default::default()
        },
        None,
    );
    // Exercise the full original scene's atlas, instances and newly restored
    // batches before isolating a source-backed fixed-camera wall witness.
    let full = GpuScene::build(renderer.device(), renderer.queue(), &source).unwrap();
    assert!(full.bounds_min.is_finite() && full.bounds_max.is_finite());
    assert!(!full.draws.is_empty());
    drop(full);
    let batches: Vec<_> = source
        .meshes
        .iter()
        .filter(|mesh| {
            mesh.indices.len() == 2210 * 3
                && source.materials[mesh.material].textures == ["thule_stonestack_c.dds"]
        })
        .collect();
    assert_eq!(batches.len(), 1, "original stonewall ordinal 3 batch");
    let mut wall = batches[0].clone();
    let material = source.materials[wall.material].clone();
    assert!(!material.alpha_mask);
    // First four source polygons are a flat wall at Y=-321.49957, spanning
    // X[-376.75,-251.53598] and Z[-105.05767,-38.97163].
    wall.indices.truncate(12);
    wall.material = 0;
    let textures: Vec<_> = material
        .textures
        .iter()
        .chain(material.normal_map.iter())
        .map(|name| source.texture(name).expect("original wall texture"))
        .collect();
    let scene = Scene::from_geometry(
        "Housegarden stone wall".into(),
        vec![material.clone()],
        vec![wall.clone()],
        textures,
    );
    let camera = Camera {
        position: [-314., -420., -72.],
        pitch: 0.,
        ..Default::default()
    };
    let capture = |renderer: &mut Renderer, scene: &Scene| {
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), scene).unwrap();
        renderer.set_scene(&gpu);
        renderer.render(&gpu, &camera);
        renderer.read_rgba().unwrap().2
    };
    let corrected = capture(&mut renderer, &scene);
    let archive = Archive::open(base.join("housegarden.eqg")).unwrap();
    let branch =
        Texture::decode("branches01.dds", &archive.read("branches01.dds").unwrap()).unwrap();
    let mut old_material = material;
    old_material.textures = vec!["branches01.dds".into()];
    old_material.normal_map = None;
    old_material.alpha_mask = true;
    let legacy = Scene::from_geometry(
        "Former stored-ID 3 lookup".into(),
        vec![old_material],
        vec![wall],
        vec![branch],
    );
    let previous = capture(&mut renderer, &legacy);
    let empty = Scene::from_geometry("Empty reference".into(), vec![], vec![], vec![]);
    let background = capture(&mut renderer, &empty);
    let different = |a: &[u8], b: &[u8]| {
        a.chunks_exact(4)
            .zip(b.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count()
    };
    assert!(
        different(&corrected, &background) > 5000,
        "wall must be visible"
    );
    assert!(
        different(&corrected, &previous) > 5000,
        "corrected stone binding must change the captured wall"
    );
    let directory = std::path::Path::new("/tmp/openeq-housegarden-materials");
    std::fs::create_dir_all(directory).unwrap();
    for (name, pixels) in [
        ("corrected.png", corrected),
        ("former-id-lookup.png", previous),
    ] {
        image::save_buffer(
            directory.join(name),
            &pixels,
            640,
            360,
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
}
