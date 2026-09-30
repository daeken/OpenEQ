//! Rendered cached diagonals must match explicit triangles and preserve seam coverage.
use openeq_assets::{
    Scene,
    mesh::Geometry,
    pfs::Archive,
    terrain::{
        Ecosystems, Heightmap, TerrainOptions, TerrainTile, TerrainWaterMetadata, bake,
        parse_ecosystem,
    },
    texture::Texture,
};
use openeq_render::{
    Camera, GpuScene, Renderer, environment::EnvironmentSettings, terrain::TerrainMode,
};
use std::time::Duration;

fn draw(renderer: &mut Renderer, scene: &Scene, camera: &Camera, mode: TerrainMode) -> Vec<u8> {
    let gpu =
        GpuScene::build_with_terrain(renderer.device(), renderer.queue(), scene, mode).unwrap();
    assert_eq!(
        gpu.terrain_stats().materials,
        if mode == TerrainMode::Direct {
            scene.terrain_materials.len()
        } else {
            0
        }
    );
    renderer.set_scene(&gpu);
    renderer.render_at(&gpu, camera, Duration::ZERO);
    renderer.read_rgba().unwrap().2
}

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

fn changed(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(a, b)| a.abs_diff(*b) > 2))
        .count()
}

#[test]
#[ignore = "requires GPU"]
fn mixed_diagonal_ground_matches_explicit_triangles_and_correctly_occludes_a_marker() {
    let map = Heightmap {
        options: TerrainOptions {
            name: "topology".into(),
            min_lng: -1,
            max_lng: 0,
            min_lat: 0,
            max_lat: 0,
            units_per_vertex: 10.,
            quads_per_tile: 1,
        },
        header: [21, 0, 1],
        base_texture: "green".into(),
        tiles: [(-1, [0., 2., 3., 1.], 0), (0, [2., 0., 1., 4.], 0x80)]
            .into_iter()
            .map(|(longitude, heights, flag)| TerrainTile {
                longitude,
                latitude: 0,
                heights: heights.to_vec(),
                colors: vec![],
                secondary_colors: vec![],
                quad_flags: vec![flag],
                water_level: -1000.,
                water_metadata: TerrainWaterMetadata::default(),
                layers: vec![],
            })
            .collect(),
        placements: vec![],
        lights: vec![],
        groups: vec![],
        regions: vec![],
        region_count: 0,
    };
    let mut baked = bake(&map, &Ecosystems::new(), |name| {
        Some(Texture {
            name: name.into(),
            width: 1,
            height: 1,
            rgba: vec![40, 180, 60, 255],
        })
    })
    .unwrap();
    // This marker is above the cached negative diagonal (center z=0.5),
    // below the former positive diagonal (center z=3), and should be visible.
    let mut marker = baked.materials[0].clone();
    marker.textures = vec!["red".into()];
    marker.emissive = true;
    baked.materials.push(marker);
    baked.textures.push(Texture {
        name: "red".into(),
        width: 1,
        height: 1,
        rgba: vec![255, 0, 0, 255],
    });
    baked.meshes.push(Geometry {
        vertices: [[4.5, 4.5], [5.5, 4.5], [5.5, 5.5], [4.5, 5.5]]
            .into_iter()
            .flat_map(|[x, y]| [x, y, 1.5, 0., 0., 1., 0., 0.])
            .collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        material: 2,
        collidable: false,
    });
    let mut scene = Scene::from_geometry(
        "topology".into(),
        baked.materials,
        baked.meshes,
        baked.textures,
    );
    scene.terrain_materials = baked.terrain_materials;
    let camera = Camera {
        position: [0., -10., 16.],
        pitch: (-14_f32).atan2(15.),
        ..Default::default()
    };
    let mut renderer = renderer(320, 240);
    let authored = [
        scene.meshes[0].indices.clone(),
        scene.meshes[1].indices.clone(),
    ];
    for mode in [TerrainMode::Direct, TerrainMode::Baked] {
        scene.meshes[0].indices.clone_from(&authored[0]);
        scene.meshes[1].indices.clone_from(&authored[1]);
        let actual = draw(&mut renderer, &scene, &camera, mode);
        scene.meshes[0].indices = vec![0, 1, 3, 0, 3, 2];
        scene.meshes[1].indices = vec![0, 1, 2, 1, 3, 2];
        let reference = draw(&mut renderer, &scene, &camera, mode);
        assert_eq!(
            actual, reference,
            "cached surface must match independent explicit triangles"
        );
        let red = |rgba: &[u8]| {
            rgba.chunks_exact(4)
                .filter(|p| p[0] > 180 && p[1] < 20 && p[2] < 20)
                .count()
        };
        assert!(
            red(&actual) > 10,
            "marker must be visibly above the corrected terrain"
        );
        scene.meshes[1].indices = vec![0, 1, 3, 0, 3, 2];
        let wrong = draw(&mut renderer, &scene, &camera, mode);
        assert_eq!(red(&wrong), 0, "former diagonal must hide the marker");
        assert!(
            changed(&actual, &wrong) > 100,
            "fixture must expose the topology error"
        );
    }
}

#[test]
#[ignore = "requires original Feerrott2 assets and GPU; writes /tmp/openeq-terrain-topology"]
fn original_feerrott_seam_renders_the_corrected_nonplanar_cell() {
    let dir = openeq_assets::loader::default_client_dir().expect("original assets");
    let archive = Archive::open(dir.join("feerrott2.eqg")).unwrap();
    let options = TerrainOptions::parse(&archive.read("feerrott.zon").unwrap()).unwrap();
    let mut map = Heightmap::parse(options, &archive.read("feerrott.dat").unwrap()).unwrap();
    map.tiles
        .retain(|tile| tile.latitude == -4 && [-3, -2].contains(&tile.longitude));
    map.tiles.sort_by_key(|tile| tile.longitude);
    assert_eq!(map.tiles.len(), 2);
    assert_eq!(map.tiles[1].quad_flags[3 * 16], 0x80);
    let mut ecosystems = Ecosystems::new();
    for tile in &map.tiles {
        for layer in &tile.layers {
            let key = layer.ecosystem.to_ascii_lowercase();
            ecosystems.entry(key.clone()).or_insert_with(|| {
                parse_ecosystem(&archive.read(&format!("{key}.eco")).unwrap()).unwrap()
            });
        }
    }
    let baked = bake(&map, &ecosystems, |name| {
        Texture::decode(name, &archive.read(name).ok()?).ok()
    })
    .unwrap();
    let mut scene = Scene::from_geometry(
        "Feerrott seam".into(),
        baked.materials,
        baked.meshes,
        baked.textures,
    );
    scene.terrain_materials = baked.terrain_materials;
    let camera = Camera {
        position: [-504., -991., 79.],
        pitch: (-19_f32).atan2(23.),
        ..Default::default()
    };
    let mut renderer = renderer(640, 480);
    let output = std::path::Path::new("/tmp/openeq-terrain-topology");
    std::fs::create_dir_all(output).unwrap();
    // Isolate the topology of this one native cell. Materials, normals, vertex
    // coordinates, other cached diagonals, and adjoining tile remain identical.
    let start = scene.meshes[1]
        .indices
        .chunks_exact(6)
        .position(|tri| tri == [51, 52, 68, 52, 69, 68])
        .unwrap()
        * 6;
    for (mode, label) in [
        (TerrainMode::Direct, "direct"),
        (TerrainMode::Baked, "baked"),
    ] {
        scene.meshes[1].indices[start..start + 6].copy_from_slice(&[51, 52, 68, 52, 69, 68]);
        let after = draw(&mut renderer, &scene, &camera, mode);
        scene.meshes[1].indices[start..start + 6].copy_from_slice(&[51, 52, 69, 51, 69, 68]);
        let before = draw(&mut renderer, &scene, &camera, mode);
        let count = changed(&before, &after);
        assert!(
            count > 100,
            "original view must expose the changed ground: {count}"
        );
        assert!(
            count < 640 * 480 / 2,
            "single-cell change must stay local: {count}"
        );
        assert!(
            !after
                .chunks_exact(4)
                .any(|p| p[0] > 180 && p[1] < 20 && p[2] > 180)
        );
        eprintln!("Feerrott2 {label}: {count}/307200 pixels change for one corrected quad");
        for (name, rgba) in [("before", before), ("after", after)] {
            image::RgbaImage::from_raw(640, 480, rgba)
                .unwrap()
                .save(output.join(format!("feerrott-{label}-{name}.png")))
                .unwrap();
        }
    }
}
