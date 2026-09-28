use openeq_assets::pfs::Archive;
use openeq_assets::terrain::{
    Heightmap, TerrainOptions, TerrainTile, parse_ecosystem, parse_object_group,
};

#[test]
fn real_heightmap_dat_streams_are_fully_understood() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    for (zone, tiles, objects, groups, lights) in [
        ("nektulos", 421, 2326, 67, 5),
        ("oldcommons", 1552, 3638, 10, 0),
        ("deadhills", 1233, 942, 5, 0),
    ] {
        let path = base.join(format!("{zone}.eqg"));
        if !path.is_file() {
            continue;
        }
        let archive = Archive::open(path).unwrap();
        let zon = archive
            .names()
            .iter()
            .find(|n| n.ends_with(".zon"))
            .unwrap();
        let options = TerrainOptions::parse(&archive.read(zon).unwrap()).unwrap();
        let data = archive.read(&format!("{}.dat", options.name)).unwrap();
        let map = Heightmap::parse(options, &data).unwrap_or_else(|e| panic!("{zone}: {e}"));
        assert_eq!(map.tiles.len(), tiles, "{zone}");
        assert_eq!(map.placements.len(), objects, "{zone}");
        assert_eq!(map.groups.len(), groups, "{zone}");
        assert_eq!(map.lights.len(), lights, "{zone}");
        assert!(map.placements.iter().all(|p| p.transform.is_finite()));
        assert!(map.tiles.iter().any(|t| t.layers.len() > 1));
        let mut truncated = data.clone();
        truncated.pop();
        assert!(Heightmap::parse(map.options.clone(), &truncated).is_err());
    }
}

#[test]
fn actual_heightmap_zones_bake_textured_terrain_and_objects() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    for zone in ["nektulos", "oldcommons", "deadhills"] {
        if !base.join(format!("{zone}.eqg")).is_file() {
            continue;
        }
        let scene = openeq_assets::load_zone(&base, zone).unwrap_or_else(|e| panic!("{zone}: {e}"));
        eprintln!(
            "{zone}: {} triangles, {} meshes, {} object instances, {} lights",
            scene.triangle_count(),
            scene.meshes.len(),
            scene.instances.len(),
            scene.lights.len()
        );
        assert!(scene.triangle_count() > 100_000);
        assert!(scene.instances.len() > 800);
        let terrain = scene
            .materials
            .iter()
            .filter_map(|m| m.textures.first())
            .filter(|n| n.starts_with("__terrain_"))
            .collect::<Vec<_>>();
        assert!(terrain.len() > 300);
        for name in terrain.iter().step_by(37) {
            let texture = scene.texture(name).unwrap();
            let magenta = texture
                .rgba
                .chunks_exact(4)
                .filter(|p| p[0] > 240 && p[1] < 15 && p[2] > 240)
                .count();
            assert!(
                magenta < texture.width as usize * texture.height as usize / 100,
                "{zone}: unresolved terrain material {name}"
            );
        }
        for mesh in &scene.meshes {
            assert!(
                mesh.vertices.iter().all(|v| v.is_finite()),
                "{zone}: non-finite vertices for {:?}",
                scene.materials[mesh.material].textures
            );
        }
        for mesh in &scene.meshes {
            assert!(
                mesh.indices
                    .iter()
                    .all(|i| (*i as usize) < mesh.vertex_count())
            );
        }
        if zone == "nektulos" {
            assert_eq!(scene.lights.len(), 5);
            assert!(scene.materials.iter().any(|m| m.water.is_some()));
            assert!(scene.materials.iter().any(|m| m.alpha_mask));
        }
    }
}

#[test]
fn version_two_binary_zone_preserves_placement_alignment() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("crescent.eqg").is_file() {
        return;
    }
    let scene = openeq_assets::load_zone(&base, "crescent").expect("v2 zone must load");
    assert!(scene.instances.len() > 1000);
    assert!(scene.triangle_count() > 10000);
    assert!(
        scene
            .instances
            .iter()
            .all(|i| i.position.iter().all(|v| v.is_finite())
                && i.position.iter().all(|v| v.abs() < 100000.0))
    );
}

#[test]
fn terrain_grid_and_object_group_transforms_are_consistent() {
    let options=TerrainOptions::parse(b"EQTZP\n*NAME test\n*MINLNG 0 *MAXLNG 0\n*MINLAT 0 *MAXLAT 0\n*UNITSPERVERT 10\n*QUADSPERTILE 1").unwrap();
    let tile = TerrainTile {
        longitude: 0,
        latitude: 0,
        heights: vec![0., 10., 20., 40.],
        colors: vec![],
        secondary_colors: vec![],
        quad_flags: vec![0],
        water_level: -1000.,
        layers: vec![],
    };
    assert_eq!(tile.height_at(&options, 0., 0.), 0.);
    assert_eq!(tile.height_at(&options, 10., 0.), 10.);
    assert_eq!(tile.height_at(&options, 0., 10.), 20.);
    assert_eq!(tile.height_at(&options, 10., 10.), 40.);
    assert_eq!(tile.height_at(&options, 5., 5.), 20.);
    let group=parse_object_group(b"*BEGIN_OBJECTGROUP\n*BEGIN_OBJECT\n*NAME TREE\n*POSITION 10 20 30\n*ROTATION 0 0 90\n*SCALE 2\n*END_OBJECT\n*END_OBJECTGROUP").unwrap();
    let p = group[0].transform.transform_point3(glam::Vec3::X);
    assert!((p - glam::Vec3::new(10., 22., 30.)).length() < 0.0001);
    let eco=parse_ecosystem(b"*TEXTUREPART\n*LAYER soil\n*DETAILMAP soil.dds\n*DETAILREPEAT 10\n*END_LAYER\n*END_TEXTUREPART\n*OBJECTPART\n*LAYER tree\n*DETAILMAP wrong.dds\n*END_LAYER").unwrap();
    assert_eq!(eco.len(), 1);
    assert_eq!(eco[0].detail_map, "soil.dds");
    assert!(TerrainOptions::parse(b"EQTZP *NAME bad *MINLNG 0 *MAXLNG 0 *MINLAT 0 *MAXLAT 0 *UNITSPERVERT 0 *QUADSPERTILE -1").is_err());
}

#[test]
fn eqg_v3_uses_primary_uvs_and_skips_unused_secondary_nan() {
    use openeq_assets::zone::{MOD_MAGIC, TerMod};
    let mut data = Vec::new();
    for word in [MOD_MAGIC, 3, 0, 0, 1, 0, 0] {
        data.extend(word.to_le_bytes());
    }
    for value in [1.0f32, 2.0, 3.0, 0.0, 0.0, 1.0] {
        data.extend(value.to_le_bytes());
    }
    data.extend(0xffff_ffffu32.to_le_bytes());
    for value in [0.25f32, 0.75, f32::NAN, f32::NAN] {
        data.extend(value.to_le_bytes());
    }
    let model = TerMod::parse(&data, false).unwrap();
    assert_eq!(model.positions, vec![[1.0, 2.0, 3.0]]);
    assert_eq!(model.tex_coords, vec![[0.25, 0.75]]);
}

#[test]
fn terrain_holes_are_not_drawn_and_visible_triangles_face_up() {
    use openeq_assets::terrain::{Ecosystems, bake};
    let options = TerrainOptions::parse(
        b"EQTZP *NAME test *MINLNG 0 *MAXLNG 0 *MINLAT 0 *MAXLAT 0 *UNITSPERVERT 1 *QUADSPERTILE 1",
    )
    .unwrap();
    let tile = TerrainTile {
        longitude: 0,
        latitude: 0,
        heights: vec![0.; 4],
        colors: vec![],
        secondary_colors: vec![],
        quad_flags: vec![1],
        water_level: -1000.,
        layers: vec![],
    };
    let mut map = Heightmap {
        options,
        header: [0; 3],
        base_texture: "solid".into(),
        tiles: vec![tile],
        placements: vec![],
        lights: vec![],
        groups: vec![],
        region_count: 0,
    };
    let texture = |_: &str| {
        Some(openeq_assets::texture::Texture {
            name: "solid".into(),
            width: 1,
            height: 1,
            rgba: vec![20, 80, 30, 255],
        })
    };
    assert!(
        bake(&map, &Ecosystems::new(), texture)
            .unwrap()
            .meshes
            .is_empty()
    );
    map.tiles[0].quad_flags[0] = 0;
    let baked = bake(&map, &Ecosystems::new(), texture).unwrap();
    let mesh = &baked.meshes[0];
    assert_eq!(mesh.indices.len(), 6);
    let p = |i: u32| glam::Vec3::from_slice(&mesh.vertices[i as usize * 8..i as usize * 8 + 3]);
    for tri in mesh.indices.chunks_exact(3) {
        assert!((p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0])).z > 0.0);
    }
}
