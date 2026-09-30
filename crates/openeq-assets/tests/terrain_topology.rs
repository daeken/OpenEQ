//! The DAT diagonal cache must agree across drawable ground, collision and anchors.
use glam::Vec3;
use openeq_assets::{
    Scene,
    collision::CollisionWorld,
    pfs::Archive,
    terrain::{
        Ecosystems, Heightmap, TerrainOptions, TerrainTile, TerrainWaterMetadata, bake,
        regions::native_anchor_height,
    },
    texture::Texture,
};

fn options() -> TerrainOptions {
    TerrainOptions {
        name: "topology".into(),
        min_lng: -2,
        max_lng: 2,
        min_lat: -3,
        max_lat: 3,
        units_per_vertex: 10.,
        quads_per_tile: 1,
    }
}

fn tile(longitude: i32, heights: [f32; 4], flags: u8) -> TerrainTile {
    TerrainTile {
        longitude,
        latitude: 0,
        heights: heights.to_vec(),
        colors: vec![],
        secondary_colors: vec![],
        quad_flags: vec![flags],
        water_level: -1000.,
        water_metadata: TerrainWaterMetadata::default(),
        layers: vec![],
    }
}

fn map(tiles: Vec<TerrainTile>) -> Heightmap {
    Heightmap {
        options: options(),
        header: [21, 0, 1],
        base_texture: "solid".into(),
        tiles,
        placements: vec![],
        lights: vec![],
        groups: vec![],
        regions: vec![],
        region_count: 0,
    }
}

fn scene(map: &Heightmap) -> Scene {
    let baked = bake(map, &Ecosystems::new(), |name| {
        Some(Texture {
            name: name.into(),
            width: 1,
            height: 1,
            rgba: vec![40, 150, 70, 255],
        })
    })
    .unwrap();
    Scene::from_geometry(
        "topology".into(),
        baked.materials,
        baked.meshes,
        baked.textures,
    )
}

fn near(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.0001,
        "actual {actual}, expected {expected}"
    );
}

#[test]
fn cached_diagonals_select_four_planes_and_matching_walkable_triangles() {
    // Row-major h00,h10,h01,h11. Shallow nonplanar surfaces are walkable,
    // and the center differs by two world units between the two diagonals.
    for (flags, indices, expected) in [
        (0, [0, 1, 3, 0, 3, 2], [0.25, 1.25, 1.75, 0.75, 0.5]),
        (0x80, [0, 1, 2, 1, 3, 2], [1.25, 2.25, 2.75, 1.75, 2.5]),
        (0x7e, [0, 1, 3, 0, 3, 2], [0.25, 1.25, 1.75, 0.75, 0.5]),
        (0xfe, [0, 1, 2, 1, 3, 2], [1.25, 2.25, 2.75, 1.75, 2.5]),
    ] {
        let map = map(vec![tile(0, [0., 2., 3., 1.], flags)]);
        let scene = scene(&map);
        assert_eq!(scene.meshes[0].indices, indices);
        let mesh = &scene.meshes[0];
        let p = |index: u32| Vec3::from_slice(&mesh.vertices[index as usize * 8..]);
        for tri in mesh.indices.chunks_exact(3) {
            assert!((p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0])).z > 0.);
        }
        let world = CollisionWorld::build(&scene);
        for ([x, y], expected) in [[2.5, 2.5], [7.5, 2.5], [2.5, 7.5], [7.5, 7.5], [5., 5.]]
            .into_iter()
            .zip(expected)
        {
            near(map.tiles[0].height_at(&map.options, x, y), expected);
            near(
                native_anchor_height(&map.options, &map.tiles[0], [x, y]).unwrap(),
                expected,
            );
            near(world.ground_height(x, y, 5., 0., 10.).unwrap(), expected);
        }
    }
}

#[test]
fn hole_flag_overrides_either_diagonal_for_drawing_and_collision() {
    for flags in [1, 0x81, 0xff] {
        let scene = scene(&map(vec![tile(0, [0., 2., 3., 1.], flags)]));
        assert!(scene.meshes.is_empty());
        assert_eq!(
            CollisionWorld::build(&scene).ground_height(5., 5., 5., 0., 10.),
            None
        );
    }
}

fn check_seam(map: &Heightmap, scene: &Scene, row: usize) {
    let q = map.options.quads_per_tile;
    let step = map.options.units_per_vertex;
    let [left, right] = [&map.tiles[0], &map.tiles[1]];
    let seam_x = right.longitude as f32 * map.options.tile_size();
    let base_y = left.latitude as f32 * map.options.tile_size();
    let world = CollisionWorld::build(scene);
    for r in row..=row + 1 {
        let a = &scene.meshes[0].vertices[(r * (q + 1) + q) * 8..][..6];
        let b = &scene.meshes[1].vertices[r * (q + 1) * 8..][..6];
        assert_eq!(a, b, "shared position and smoothed normal");
        assert!(a.iter().all(|v| v.is_finite()) && a[5] > 0.);
    }
    for fraction in [0.125, 0.25, 0.5, 0.75, 0.875] {
        let y = (row as f32 + fraction) * step;
        let edge = left.height_at(&map.options, map.options.tile_size(), y);
        near(right.height_at(&map.options, 0., y), edge);
        near(
            world
                .ground_height(seam_x, base_y + y, edge, 10., 10.)
                .unwrap(),
            edge,
        );
        for (offset, tile, local_x) in [
            (-0.001, left, map.options.tile_size() - 0.001),
            (0.001, right, 0.001),
        ] {
            let sampled = tile.height_at(&map.options, local_x, y);
            assert!((sampled - edge).abs() < 0.002, "surface cracked at seam");
            near(
                world
                    .ground_height(seam_x + offset, base_y + y, edge, 10., 10.)
                    .unwrap(),
                sampled,
            );
        }
    }
}

#[test]
fn mixed_diagonals_preserve_shared_tile_edges_normals_and_collision() {
    let map = map(vec![
        tile(0, [0., 2., 3., 1.], 0),
        tile(1, [2., 0., 1., 4.], 0x80),
    ]);
    check_seam(&map, &scene(&map), 0);
    // Edge clamping remains this public sampler's contract for either topology.
    for tile in &map.tiles {
        near(
            tile.height_at(&map.options, -5., 5.),
            tile.height_at(&map.options, 0., 5.),
        );
        near(
            tile.height_at(&map.options, 15., 5.),
            tile.height_at(&map.options, 10., 5.),
        );
    }
}

fn words(data: &mut Vec<u8>, values: &[u32]) {
    for value in values {
        data.extend(value.to_le_bytes());
    }
}

fn floats(data: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        data.extend(value.to_le_bytes());
    }
}

#[test]
fn dat_props_and_lights_use_the_cached_ground_plane_while_groups_keep_absolute_z() {
    for (flags, prop_z, light_z) in [(0, 5.25, 9.75), (0x80, 6.25, 10.75)] {
        let mut data = Vec::new();
        words(&mut data, &[21, 0, 1]);
        data.extend(b"solid\0");
        words(&mut data, &[1, 99998, 100003, 0]);
        floats(&mut data, &[0., 2., 3., 1.]);
        words(&mut data, &[0; 8]);
        data.push(flags);
        floats(&mut data, &[-1000.]);
        words(&mut data, &[u32::MAX]);
        data.push(0);
        floats(&mut data, &[0.]);
        words(&mut data, &[0, 1]); // No layers, one single placement.
        data.extend(b"prop.mod\0ecosystem\0");
        words(&mut data, &[99998, 100003]);
        floats(&mut data, &[2.5, 2.5, 5., 0., 0., 0., 1., 1., 1.]);
        data.push(0);
        words(&mut data, &[0, 1]); // No regions, one light.
        data.extend(b"point\0point.lit\0");
        data.push(0);
        words(&mut data, &[99998, 100003]);
        floats(&mut data, &[7.5, 7.5, 9., 0., 0., 0., 1., 1., 1., 25.]);
        words(&mut data, &[1]); // Group has absolute Z + scale.z * adjustment.
        data.extend(b"group.tog\0");
        words(&mut data, &[99998, 100003]);
        floats(&mut data, &[2.5, 2.5, 12., 0., 0., 0., 1., 1., 2., 3.]);
        let map = Heightmap::parse(options(), &data).unwrap();
        assert_eq!(
            map.placements[0]
                .transform
                .transform_point3(Vec3::ZERO)
                .to_array(),
            [-17.5, 32.5, prop_z]
        );
        assert_eq!(map.lights[0].position, [-12.5, 37.5, light_z]);
        assert_eq!(
            map.groups[0]
                .transform
                .transform_point3(Vec3::ZERO)
                .to_array(),
            [-17.5, 32.5, 18.]
        );
    }
}

#[test]
#[ignore = "requires original Feerrott2 assets"]
fn original_feerrott_cached_diagonal_changes_ground_and_keeps_a_walkable_seam() {
    let dir = openeq_assets::loader::default_client_dir().expect("original assets");
    let archive = Archive::open(dir.join("feerrott2.eqg")).unwrap();
    let options = TerrainOptions::parse(&archive.read("feerrott.zon").unwrap()).unwrap();
    let mut map = Heightmap::parse(options, &archive.read("feerrott.dat").unwrap()).unwrap();
    map.tiles
        .retain(|tile| tile.latitude == -4 && [-3, -2].contains(&tile.longitude));
    map.tiles.sort_by_key(|tile| tile.longitude);
    assert_eq!(map.tiles.len(), 2);
    assert_eq!(
        (map.options.quads_per_tile, map.options.units_per_vertex),
        (16, 16.)
    );
    assert_eq!(map.tiles[0].quad_flags[3 * 16 + 15], 0);
    assert_eq!(map.tiles[1].quad_flags[3 * 16], 0x80);
    let scene = scene(&map);
    check_seam(&map, &scene, 3);
    // Independent native x87 replay at local [8,56]: negative-diagonal center
    // uses h10/h01 and is 4.220995 units above the former positive diagonal.
    let corrected = map.tiles[1].height_at(&map.options, 8., 56.);
    near(corrected, 61.503_113);
    near(
        native_anchor_height(&map.options, &map.tiles[1], [8., 56.]).unwrap(),
        corrected,
    );
    near(
        CollisionWorld::build(&scene)
            .ground_height(-504., -968., 70., 0., 20.)
            .unwrap(),
        corrected,
    );
    let mut old = map.tiles[1].clone();
    old.quad_flags[3 * 16] = 0;
    near(old.height_at(&map.options, 8., 56.), 57.282_12);
}
