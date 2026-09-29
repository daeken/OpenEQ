//! Indexed surfaces remain bounded drawing, independent of terrain and physics.
use glam::Vec3;
use openeq_assets::{
    Scene,
    collision::CollisionWorld,
    loader,
    mesh::{Geometry, VERTEX_STRIDE},
    pfs::Archive,
    terrain::{
        self, Heightmap, IndexedWaterResolution, TerrainOptions, TerrainTile,
        TerrainWaterExtension, TerrainWaterMetadata, WaterData, indexed_water,
    },
};

fn original(zone: &str, internal: &str) -> (Heightmap, WaterData) {
    let base = loader::default_client_dir().expect("original EverQuest assets");
    let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
    let options =
        TerrainOptions::parse(&archive.read(&format!("{internal}.zon")).unwrap()).unwrap();
    let map =
        Heightmap::parse(options, &archive.read(&format!("{internal}.dat")).unwrap()).unwrap();
    let water = terrain::parse_water_data(&archive.read("water.dat").unwrap()).unwrap();
    (map, water)
}
fn bounds(mesh: &Geometry) -> ([f32; 3], [f32; 3]) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for vertex in mesh.vertices.chunks_exact(VERTEX_STRIDE) {
        let p = Vec3::from_slice(&vertex[..3]);
        min = min.min(p);
        max = max.max(p);
    }
    (min.to_array(), max.to_array())
}
fn assert_two_sided(mesh: &Geometry, cells: usize) {
    assert!(!mesh.collidable);
    assert_eq!(mesh.indices.len(), cells * 12);
    let mut orientations = [0; 2];
    for triangle in mesh.indices.chunks_exact(3) {
        let vertices = triangle
            .iter()
            .map(|&i| &mesh.vertices[i as usize * 8..i as usize * 8 + 8])
            .collect::<Vec<_>>();
        let positions = vertices
            .iter()
            .map(|v| Vec3::from_slice(&v[..3]))
            .collect::<Vec<_>>();
        let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        assert!(normal.z != 0. && normal.x == 0. && normal.y == 0.);
        for vertex in vertices {
            assert_eq!(Vec3::from_slice(&vertex[3..6]), Vec3::Z * normal.z.signum());
        }
        orientations[usize::from(normal.z < 0.)] += 1;
    }
    assert_eq!(orientations, [cells * 2, cells * 2]);
}
fn indexed_meshes(scene: &Scene) -> Vec<&Geometry> {
    scene
        .meshes
        .iter()
        .filter(|m| {
            scene.materials[m.material]
                .water
                .as_ref()
                .is_some_and(|w| w.indexed_uv_scale.is_some())
        })
        .collect()
}
fn basename(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap()
        .to_ascii_lowercase()
}
fn assert_resolved_material(scene: &Scene, mesh: &Geometry, water: &WaterData, selector: i32) {
    let IndexedWaterResolution::Unique { definition, .. } = water.resolve_index(selector) else {
        panic!("unresolved authored selector {selector}");
    };
    let material = &scene.materials[mesh.material];
    let mut expected = definition.material.clone();
    expected.environment_map = expected.environment_map.as_deref().map(basename);
    expected.indexed_uv_scale = Some(definition.uv_scale);
    assert_eq!(material.water.as_ref(), Some(&expected));
    assert_eq!(material.normal_map, Some(basename(&definition.normal_map)));
    assert_eq!(material.textures, ["water_c.bmp"]);
    for name in material
        .textures
        .iter()
        .chain(material.normal_map.iter())
        .chain(expected.environment_map.iter())
    {
        let texture = scene
            .texture(name)
            .unwrap_or_else(|| panic!("indexed water texture {name} unresolved"));
        assert!(texture.width > 0 && texture.height > 0);
        assert!(
            texture
                .rgba
                .chunks_exact(4)
                .any(|p| p != [255, 0, 255, 255]),
            "placeholder texture {name}"
        );
    }
}

#[test]
fn nonaligned_q24_surface_preserves_quantized_uvs_and_ignores_terrain() {
    let water=terrain::parse_water_data(b"*WATERSHEETDATA\n*INDEX 2\n*UVSCALE 1.25\n*NORMALMAP normal.dds\n*ENVIRONMENTMAP environment.dds\n*WATERCOLOR1 0 0 0 1\n*WATERCOLOR2 1 1 1 1\n*REFLECTIONCOLOR 1 1 1 1\n*FRESNELBIAS 0.25\n*FRESNELPOWER 8\n*REFLECTIONAMOUNT 0.7\n*ENDWATERSHEETDATA\n").unwrap();
    let mut map = Heightmap {
        options: TerrainOptions {
            name: "q24".into(),
            min_lng: -3,
            max_lng: -3,
            min_lat: 2,
            max_lat: 2,
            units_per_vertex: 16.,
            quads_per_tile: 24,
        },
        header: [21, 0, 0],
        base_texture: String::new(),
        placements: vec![],
        lights: vec![],
        groups: vec![],
        regions: Vec::new(),
        region_count: 0,
        tiles: vec![TerrainTile {
            longitude: -3,
            latitude: 2,
            heights: vec![100.; 625],
            colors: vec![],
            secondary_colors: vec![],
            quad_flags: vec![1; 576],
            layers: vec![],
            water_level: -7.,
            water_metadata: TerrainWaterMetadata {
                word_bits: 2,
                extension: Some(TerrainWaterExtension {
                    tag: -128,
                    bounds: Some([8., 72., 16., 64.]),
                    trailing_value: f32::NAN,
                }),
            },
        }],
    };
    let baked = indexed_water::bake(&map, &water).unwrap();
    assert!(baked.diagnostics.is_empty());
    assert_eq!(baked.surfaces.len(), 1);
    let surface = &baked.surfaces[0];
    assert_eq!((surface.tile_index, surface.material_index), (0, 2));
    assert_eq!(
        bounds(&surface.geometry),
        ([-1144., 784., -7.], [-1080., 832., -7.])
    );
    assert_eq!(surface.geometry.vertex_count(), 40);
    assert_two_sided(&surface.geometry, 12);
    let mut u: Vec<_> = surface
        .geometry
        .vertices
        .chunks_exact(8)
        .map(|v| v[6])
        .collect();
    u.sort_by(f32::total_cmp);
    u.dedup();
    assert_eq!(u, [10., 21., 32., 42., 53.].map(|v| v / 256.));
    assert_eq!(water.indexed[0].uv_scale, 1.25);
    map.tiles[0].heights.fill(-1000.);
    map.tiles[0].quad_flags.fill(0);
    map.tiles[0]
        .water_metadata
        .extension
        .as_mut()
        .unwrap()
        .trailing_value = 5000.;
    let changed = indexed_water::bake(&map, &water).unwrap();
    assert_eq!(
        surface.geometry.vertices,
        changed.surfaces[0].geometry.vertices
    );
    assert_eq!(
        surface.geometry.indices,
        changed.surfaces[0].geometry.indices
    );
    let scene = Scene::from_geometry(
        "water only".into(),
        vec![],
        vec![surface.geometry.clone()],
        vec![],
    );
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 0);
}

#[test]
#[ignore = "requires original Feerrott2 assets"]
fn original_feerrott_pond_bounds_seams_occluded_tile_and_noncollision() {
    let (map, water) = original("feerrott2", "feerrott");
    let baked = indexed_water::bake(&map, &water).unwrap();
    assert!(baked.diagnostics.is_empty());
    assert_eq!(baked.surfaces.len(), 65);
    assert_eq!(
        baked
            .surfaces
            .iter()
            .map(|s| s.geometry.indices.len() / 3)
            .sum::<usize>(),
        34132
    );
    let find = |tile| {
        &baked
            .surfaces
            .iter()
            .find(|s| s.tile_index == tile)
            .unwrap()
            .geometry
    };
    let pond = find(84);
    assert_eq!(bounds(pond), ([-768., -2960., -50.], [-672., -2848., -50.]));
    assert_eq!((pond.vertex_count(), pond.indices.len()), (112, 504));
    assert_two_sided(pond, 42);
    let uv_bounds = pond.vertices.chunks_exact(8).fold(
        ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
        |(mut min, mut max), v| {
            for i in 0..2 {
                min[i] = min[i].min(v[6 + i]);
                max[i] = max[i].max(v[6 + i]);
            }
            (min, max)
        },
    );
    assert_eq!(uv_bounds, ([0., 0.4375], [0.375, 0.875]));
    assert!(!baked.surfaces.iter().any(|s| s.tile_index == 11));
    assert_eq!(map.tiles[11].water_level, -30.);
    let hidden = find(271);
    assert_eq!(bounds(hidden), ([-1552., 752., -30.], [-1536., 768., -30.]));
    assert_two_sided(hidden, 1);
    assert!(map.tiles[271].heights.iter().all(|&z| z > -30.));
    // Tile109 ends at U1 while tile84 starts at U0. Keep those endpoints in
    // the mesh and compare repeat phase at the authored shared grid vertices.
    let seam = find(109);
    for y in [-2944., -2928., -2912., -2896., -2880., -2864.] {
        let at = |mesh: &Geometry| -> [f32; 8] {
            mesh.vertices
                .chunks_exact(8)
                .find(|v| v[0] == -768. && v[1] == y && v[5] > 0.)
                .unwrap()
                .try_into()
                .unwrap()
        };
        let a = at(seam);
        let b = at(pond);
        assert_eq!((a[2], b[2]), (-50., -50.));
        assert_eq!((a[6], b[6]), (1., 0.));
        assert_eq!(a[7], b[7]);
        assert_eq!((a[6] * 2.).fract(), (b[6] * 2.).fract());
    }
    let base = loader::default_client_dir().unwrap();
    let mut scene = loader::load_zone(base, "feerrott2").unwrap();
    assert_eq!(scene.triangle_count(), 211245 + 34132);
    let meshes = indexed_meshes(&scene);
    assert_eq!(meshes.len(), 65);
    assert_eq!(
        scene
            .materials
            .iter()
            .filter(|m| m
                .water
                .as_ref()
                .is_some_and(|w| w.indexed_uv_scale.is_some()))
            .count(),
        1,
        "resolved selector material should be shared"
    );
    for surface in &baked.surfaces {
        let loaded = meshes
            .iter()
            .find(|m| {
                m.vertices == surface.geometry.vertices && m.indices == surface.geometry.indices
            })
            .expect("authored water surface missing or altered by loader");
        assert!(!loaded.collidable);
        assert_resolved_material(&scene, loaded, &water, surface.material_index);
    }
    assert_eq!(
        CollisionWorld::build(&scene).triangle_count(),
        824400,
        "new water must not add physical triangles"
    );
    let water_only = Scene::from_geometry(
        "indexed water".into(),
        scene.materials.clone(),
        meshes.into_iter().cloned().collect(),
        vec![],
    );
    let world = CollisionWorld::build(&water_only);
    assert_eq!(world.triangle_count(), 0);
    assert_eq!(world.ground_height(-720., -2900., -50., 0., 0.), None);
    assert_eq!(
        world.clip_camera([-720., -2900., -40.], [-720., -2900., -60.], 0.),
        [-720., -2900., -60.]
    );
    for mesh in &mut scene.meshes {
        if scene.materials[mesh.material]
            .water
            .as_ref()
            .is_some_and(|w| w.indexed_uv_scale.is_some())
        {
            mesh.indices.clear();
        }
    }
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 824400);
}

#[test]
#[ignore = "requires original Buried Sea assets"]
fn original_buried_sea_selector_two_keeps_its_gray_material() {
    let (map, water) = original("buriedsea", "buriedsea");
    let baked = indexed_water::bake(&map, &water).unwrap();
    assert!(baked.diagnostics.is_empty());
    assert_eq!(baked.surfaces.len(), 900);
    assert_eq!(
        baked
            .surfaces
            .iter()
            .filter(|s| s.material_index == 2)
            .count(),
        22
    );
    let scene = loader::load_zone(loader::default_client_dir().unwrap(), "buriedsea").unwrap();
    let meshes = indexed_meshes(&scene);
    assert_eq!(meshes.len(), 900);
    let surface = baked
        .surfaces
        .iter()
        .find(|s| s.material_index == 2)
        .unwrap();
    let mesh = meshes
        .iter()
        .find(|m| m.vertices == surface.geometry.vertices && m.indices == surface.geometry.indices)
        .unwrap();
    assert_resolved_material(&scene, mesh, &water, 2);
    let material = scene.materials[mesh.material].water.as_ref().unwrap();
    assert_eq!(material.color1, [0.2, 0.2, 0.2, 1.]);
    assert_eq!(
        (
            material.fresnel_bias,
            material.fresnel_power,
            material.reflection_amount
        ),
        (0.15, 8., 0.3)
    );
    assert_eq!(
        scene
            .materials
            .iter()
            .filter(|m| m
                .water
                .as_ref()
                .is_some_and(|w| w.indexed_uv_scale.is_some()))
            .count(),
        2
    );
    assert_eq!(
        scene.triangle_count(),
        663946
            + baked
                .surfaces
                .iter()
                .map(|s| s.geometry.indices.len() / 3)
                .sum::<usize>()
    );
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 1567093);
}

#[test]
#[ignore = "requires original Old Commonlands and Anguish assets"]
fn inactive_indexed_definitions_and_legacy_water_keep_existing_behavior() {
    let (map, water) = original("oldcommons", "commonlands");
    assert_eq!(water.indexed.len(), 83);
    let baked = indexed_water::bake(&map, &water).unwrap();
    assert!(baked.surfaces.is_empty() && baked.diagnostics.is_empty());
    let base = loader::default_client_dir().unwrap();
    let scene = loader::load_zone(&base, "oldcommons").unwrap();
    assert!(indexed_meshes(&scene).is_empty());
    assert_eq!(scene.triangle_count(), 834516);
    assert_eq!(CollisionWorld::build(&scene).triangle_count(), 841729);
    let finite = scene
        .meshes
        .iter()
        .filter(|m| scene.materials[m.material].water.is_some())
        .collect::<Vec<_>>();
    assert_eq!(finite.len(), 1);
    assert!(!finite[0].collidable);
    assert_eq!(
        scene.materials[finite[0].material].water.as_ref(),
        Some(&water.finite_sheets[0].material)
    );
    let scene = loader::load_zone(base, "anguish").unwrap();
    let eqg = scene
        .materials
        .iter()
        .filter_map(|m| m.water.as_ref())
        .collect::<Vec<_>>();
    assert!(!eqg.is_empty());
    assert!(eqg.iter().all(|w| w.indexed_uv_scale.is_none()));
    assert!(
        scene
            .meshes
            .iter()
            .filter(|m| scene.materials[m.material].water.is_some())
            .all(|m| !m.collidable)
    );
}
