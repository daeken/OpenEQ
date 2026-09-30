//! Lazy terrain must preserve fallback pixels and isolate each tile's cache.
use super::*;
use crate::terrain::{
    EcoLayer, Ecosystems, Heightmap, TerrainLayer, TerrainOptions, TerrainTile,
    TerrainWaterMetadata,
};
use std::sync::Barrier;

fn layer(name: &str) -> TerrainLayer {
    TerrainLayer {
        ecosystem: name.into(),
        mask_size: 0,
        mask: vec![],
    }
}

fn eco(name: &str, repeat: f32) -> EcoLayer {
    EcoLayer {
        detail_map: name.into(),
        repeat,
        min_height: -42.,
        max_height: -42.,
        height_tolerance: 10.,
        min_slope: 22.,
        max_slope: 120.,
        slope_tolerance: 0.,
    }
}

// Same nonuniform fixture used to capture the pre-refactor pixel hashes.
fn fixture() -> (Heightmap, Ecosystems) {
    let mut tiles = Vec::new();
    for index in 0..4 {
        tiles.push(TerrainTile {
            longitude: index,
            latitude: 0,
            heights: vec![0.; 4],
            colors: vec![],
            secondary_colors: vec![],
            quad_flags: vec![u8::from(index == 1)],
            water_level: -1000.,
            water_metadata: TerrainWaterMetadata::default(),
            layers: if index == 0 {
                vec![]
            } else {
                vec![layer("base")]
            },
        });
    }
    tiles[2].layers.push(TerrainLayer {
        ecosystem: "overlay".into(),
        mask_size: 2,
        mask: vec![0, 64, 128, 255],
    });
    tiles[2].heights = vec![-60., -30., -10., 10.];
    tiles[3].heights = vec![-30., 20., 10., 45.];
    let map = Heightmap {
        options: TerrainOptions {
            name: "RecipeCase".into(),
            min_lng: 0,
            max_lng: 3,
            min_lat: 0,
            max_lat: 0,
            units_per_vertex: 10.,
            quads_per_tile: 1,
        },
        header: [21, 0, 1],
        base_texture: "None".into(),
        tiles,
        placements: vec![],
        lights: vec![],
        groups: vec![],
        regions: vec![],
        region_count: 0,
    };
    let mut ecosystems = Ecosystems::from([
        (
            "base".into(),
            vec![eco("soil.dds", 6.), eco("rock.dds", 12.)],
        ),
        ("overlay".into(), vec![eco("snow.dds", 30.)]),
    ]);
    ecosystems.get_mut("base").unwrap()[1].height_tolerance = 55.;
    ecosystems.get_mut("base").unwrap()[1].min_slope = 0.;
    ecosystems.get_mut("base").unwrap()[1].slope_tolerance = 12.;
    (map, ecosystems)
}

fn texture(name: &str) -> Option<Texture> {
    if name == "none" || name == "missing.dds" {
        return None;
    }
    let seed = match name {
        "soil.dds" => 19_u8,
        "rock.dds" => 101,
        _ => 211,
    };
    Some(Texture {
        name: name.into(),
        width: 3,
        height: 2,
        rgba: (0..6_u8)
            .flat_map(|i| {
                [
                    seed.wrapping_add(i * 7),
                    i * 39,
                    seed.wrapping_sub(i * 17),
                    40 + i * 31,
                ]
            })
            .collect(),
    })
}

fn hash(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

fn cached_count(scene: &Scene) -> usize {
    scene
        .textures
        .values()
        .filter(|source| matches!(source, TextureSource::DeferredTerrain(t) if t.is_cached()))
        .count()
}

#[test]
fn preparation_preserves_geometry_sources_and_prechange_fallback_pixels() {
    for (base, expected) in [
        (
            "None",
            vec![(2, 0x6aa5406763cbc62e), (3, 0x5a2efdd6ee0d4e29)],
        ),
        (
            "SOIL.DDS",
            vec![
                (0, 0xb93f144efefec325),
                (2, 0x6aa5406763cbc62e),
                (3, 0xe054d7c994d95f8b),
            ],
        ),
    ] {
        let (mut map, ecosystems) = fixture();
        map.base_texture = base.into();
        let eager = terrain::bake(&map, &ecosystems, texture).unwrap();
        let mut calls = BTreeMap::new();
        let prepared = terrain::prepare(&map, &ecosystems, |name| {
            *calls.entry(name.to_owned()).or_insert(0) += 1;
            texture(name)
        })
        .unwrap();
        assert!(calls.values().all(|count| *count == 1));
        let scene = Scene::from_prepared_terrain("fixture".into(), prepared);
        assert_eq!(cached_count(&scene), 0);
        assert_eq!(scene.materials, eager.materials);
        assert_eq!(scene.meshes.len(), eager.meshes.len());
        assert_eq!(scene.terrain_materials.len(), eager.terrain_materials.len());
        for (actual, previous) in scene.meshes.iter().zip(&eager.meshes) {
            assert_eq!(actual.vertices, previous.vertices);
            assert_eq!(actual.indices, previous.indices);
            assert_eq!(actual.material, previous.material);
            assert_eq!(actual.collidable, previous.collidable);
        }
        for (&index, recipe) in &scene.terrain_materials {
            let previous = &eager.terrain_materials[&index];
            assert_eq!(recipe.fallback_texture, previous.fallback_texture);
            assert_eq!(recipe.base_texture, previous.base_texture);
            for (a, b) in recipe.layers.iter().zip(&previous.layers) {
                assert_eq!(a.mask, b.mask);
                assert_eq!(a.mask_size, b.mask_size);
                assert_eq!(a.layers.len(), b.layers.len());
                for (a, b) in a.layers.iter().zip(&b.layers) {
                    assert_eq!(a.detail_map, b.detail_map);
                    assert_eq!(
                        [
                            a.repeat,
                            a.min_height,
                            a.max_height,
                            a.height_tolerance,
                            a.min_slope,
                            a.max_slope,
                            a.slope_tolerance
                        ],
                        [
                            b.repeat,
                            b.min_height,
                            b.max_height,
                            b.height_tolerance,
                            b.min_slope,
                            b.max_slope,
                            b.slope_tolerance
                        ]
                    );
                }
            }
        }
        for name in ["rock.dds", "snow.dds", "soil.dds"] {
            let TextureSource::Decoded(shared) = &scene.textures[name] else {
                panic!("source missing")
            };
            assert!(
                Arc::strong_count(shared) > 1,
                "source pixels must be shared with painter"
            );
            assert_eq!(
                scene.texture(name).unwrap().rgba,
                texture(name).unwrap().rgba
            );
        }
        // Enumeration and direct-source requests must not paint fallbacks.
        assert_eq!(cached_count(&scene), 0);
        let mut eager_names: Vec<_> = eager.textures.iter().map(|t| t.name.clone()).collect();
        eager_names.sort();
        assert_eq!(scene.texture_names(), eager_names);
        for (tile, golden) in expected {
            let name = format!("__terrain_recipecase_{tile}.rgba");
            let previous = eager.textures.iter().find(|t| t.name == name).unwrap();
            let actual = scene.texture(&name.to_ascii_uppercase()).unwrap();
            assert_eq!(hash(&actual.rgba), golden, "{base}: {name}");
            assert_eq!(actual.rgba, previous.rgba);
            assert_eq!([actual.width, actual.height], [128; 2]);
        }
    }
}

#[test]
fn concurrent_requests_initialize_only_one_tile_and_reuse_its_cache() {
    let (map, ecosystems) = fixture();
    let scene = Scene::from_prepared_terrain(
        "fixture".into(),
        terrain::prepare(&map, &ecosystems, texture).unwrap(),
    );
    let name = "__terrain_recipecase_2.rgba";
    let barrier = Barrier::new(8);
    let pointers = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let scene = &scene;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    assert_eq!(hash(&scene.texture(name).unwrap().rgba), 0x6aa5406763cbc62e);
                    let TextureSource::DeferredTerrain(source) = &scene.textures[name] else {
                        panic!("not deferred")
                    };
                    source.texture().rgba.as_ptr() as usize
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(pointers.iter().all(|pointer| *pointer == pointers[0]));
    assert_eq!(cached_count(&scene), 1);
    assert!(scene.texture("unknown.rgba").is_none());
    assert!(scene.texture_cube(name).is_none());
    // Public texture results are owned: callers cannot alter the shared cache.
    scene.texture(name).unwrap().rgba.fill(0);
    assert_eq!(hash(&scene.texture(name).unwrap().rgba), 0x6aa5406763cbc62e);
    assert_eq!(cached_count(&scene), 1);
}

#[test]
fn deferred_inputs_survive_source_mutation_and_object_remapping() {
    let (mut map, mut ecosystems) = fixture();
    let prepared = terrain::prepare(&map, &ecosystems, texture).unwrap();
    map.tiles.clear();
    map.options.quads_per_tile = 0;
    ecosystems.clear();
    let mut scene = Scene::from_prepared_terrain("fixture".into(), prepared);
    scene.objects.push(SceneObject {
        name: "object".into(),
        meshes: vec![1],
        collision_meshes: vec![],
    });
    let object = scene.object_model("object").unwrap();
    assert!(object.terrain_materials.is_empty());
    assert_eq!(object.meshes[0].material, 0);
    assert_eq!(
        object.materials[0].textures,
        ["__terrain_recipecase_3.rgba"]
    );
    assert_eq!(
        hash(&object.texture("__terrain_recipecase_3.rgba").unwrap().rgba),
        0x5a2efdd6ee0d4e29
    );
    assert_eq!(cached_count(&scene), 1);
}

#[test]
fn missing_or_malformed_sources_and_unsupported_recipes_keep_the_eager_fallback() {
    for case in 0..4 {
        let (mut map, mut ecosystems) = fixture();
        match case {
            0 => ecosystems.clear(),
            1 => ecosystems.get_mut("base").unwrap()[1].repeat = f32::NAN,
            2 => {
                ecosystems.get_mut("base").unwrap()[0].detail_map = "missing.dds".into();
                map.tiles[2].layers[1].mask_size = 0;
                map.tiles[2].layers[1].mask.clear();
            }
            _ => {}
        }
        let resolve = |name: &str| {
            if case == 3 {
                Texture::decode(name, b"not a valid image").ok()
            } else {
                texture(name)
            }
        };
        let eager = terrain::bake(&map, &ecosystems, resolve).unwrap();
        let prepared = terrain::prepare(&map, &ecosystems, resolve).unwrap();
        let scene = Scene::from_prepared_terrain("fixture".into(), prepared);
        assert_eq!(cached_count(&scene), 0);
        assert_eq!(scene.materials, eager.materials);
        assert_eq!(scene.meshes.len(), 2);
        for previous in &eager.textures {
            let actual = scene.texture(&previous.name).unwrap();
            assert_eq!(actual.rgba, previous.rgba, "case {case}: {}", previous.name);
            if case == 0 || case == 3 {
                assert!(
                    actual
                        .rgba
                        .chunks_exact(4)
                        .all(|pixel| pixel == [255, 0, 255, 255])
                );
            }
        }
    }
}

#[test]
fn native_source_name_collision_preserves_eager_registration_order() {
    let (mut map, ecosystems) = fixture();
    map.base_texture = "__terrain_recipecase_2.rgba".into();
    let eager = terrain::bake(&map, &ecosystems, texture).unwrap();
    let eager = Scene::from_geometry(
        "eager".into(),
        eager.materials,
        eager.meshes,
        eager.textures,
    );
    let lazy = Scene::from_prepared_terrain(
        "lazy".into(),
        terrain::prepare(&map, &ecosystems, texture).unwrap(),
    );
    let name = &map.base_texture;
    let expected = eager.texture(name).unwrap();
    let actual = lazy.texture(name).unwrap();
    assert_eq!([actual.width, actual.height], [3, 2]);
    assert_eq!(actual.rgba, expected.rgba);
    assert_eq!(cached_count(&lazy), 0);
}

#[test]
#[ignore = "requires original OldCommons archive; CPU only"]
fn original_loader_defers_tiles_and_limits_paint_changes_to_corrected_diagonals() {
    let base = default_client_dir().expect("original client assets");
    let scene = load_zone(base, "oldcommons").unwrap();
    assert_eq!(scene.terrain_materials.len(), 1552);
    assert_eq!(cached_count(&scene), 0);
    assert_eq!(
        scene
            .textures
            .values()
            .filter(|s| matches!(s, TextureSource::DeferredTerrain(_)))
            .count(),
        1552
    );
    let archive = &scene.archives[0];
    let map = read_heightmap(archive, &archive.read("commonlands.zon").unwrap()).unwrap();
    let mut ecosystems = Ecosystems::new();
    for tile in &map.tiles {
        for layer in &tile.layers {
            let key = layer.ecosystem.to_ascii_lowercase();
            if !ecosystems.contains_key(&key) {
                ecosystems.insert(
                    key.clone(),
                    terrain::parse_ecosystem(&archive.read(&format!("{key}.eco")).unwrap())
                        .unwrap(),
                );
            }
        }
    }
    let mut legacy_map = map.clone();
    for tile in &mut legacy_map.tiles {
        for flag in &mut tile.quad_flags {
            *flag &= !0x80;
        }
    }
    let legacy = Scene::from_prepared_terrain(
        "legacy diagonals".into(),
        terrain::prepare(&legacy_map, &ecosystems, |name| scene.texture(name)).unwrap(),
    );
    // Retain the pre-refactor eager hashes with the former all-positive grid.
    // Corrected hashes may change only height-dependent paint in 0x80 cells.
    // Includes every original tile with more than three ecosystem applications.
    let goldens = [
        (0, 0x2b4838a245432da5, 0x2b4838a245432da5),
        (137, 0x83fb1f4685410ee2, 0x83fb1f4685410ee2),
        (233, 0xf43905c3e16c0002, 0xc28f006b619c746e),
        (274, 0xca46b05604eaacb6, 0xca46b05604eaacb6),
        (411, 0xc371656da5b56bac, 0xc371656da5b56bac),
        (430, 0x336b329dbe5b9f27, 0x2327b121c618e471),
        (548, 0x9a4d978b592445a3, 0x012e42978075e695),
        (685, 0xf807359c4ec89dd8, 0xf807359c4ec89dd8),
        (822, 0x21d0b4558a00b89e, 0x21d0b4558a00b89e),
        (959, 0x91ecfa0b1f42e895, 0xe38a0116b3bb9771),
        (1096, 0x4923c11327050685, 0xa7324e3c0488f909),
        (1158, 0x0def58099db7a2ef, 0x622fbb0d9b401a9b),
        (1172, 0x8c7971afd5460a95, 0x8c7971afd5460a95),
        (1205, 0x9eeea475d63d3456, 0x92799e30148a6221),
        (1219, 0xcc624b8c71d479d8, 0x4e1cf295cecf6e97),
        (1233, 0x07fec6aba978d5b6, 0x8947ef492dca9fb5),
        (1370, 0xbc312b73cb765170, 0xbc312b73cb765170),
        (1507, 0xfc12c2b75de6e996, 0xb5d6363c15f07fba),
    ];
    for (tile, before, after) in goldens {
        let name = format!("__terrain_commonlands_{tile}.rgba");
        let old = legacy.texture(&name).unwrap();
        let actual = scene.texture(&name).unwrap();
        assert_eq!(hash(&old.rgba), before, "legacy {name}");
        assert_eq!(hash(&actual.rgba), after, "corrected {name}");
        let q = map.options.quads_per_tile;
        for (pixel, (a, b)) in old
            .rgba
            .chunks_exact(4)
            .zip(actual.rgba.chunks_exact(4))
            .enumerate()
        {
            if a != b {
                let col = (pixel % 128) * q / 128;
                let row = (pixel / 128) * q / 128;
                assert_ne!(
                    map.tiles[tile].quad_flags[row * q + col] & 0x80,
                    0,
                    "paint changed outside a corrected cell in {name}"
                );
            }
        }
    }
    assert_eq!(cached_count(&scene), goldens.len());
}
