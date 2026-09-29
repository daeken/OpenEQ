//! Direct terrain recipes retain authored source detail without changing the bake.
use std::collections::{BTreeMap, BTreeSet};

use openeq_assets::{
    Scene, SceneObject, loader,
    terrain::{
        EcoLayer, Ecosystems, Heightmap, TerrainLayer, TerrainOptions, TerrainTile,
        TerrainWaterMetadata, bake,
    },
    texture::Texture,
};

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

fn map() -> Heightmap {
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
    Heightmap {
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
    }
}

fn ecosystems() -> Ecosystems {
    Ecosystems::from([
        (
            "base".into(),
            vec![eco("soil.dds", 6.), eco("rock.dds", 12.)],
        ),
        ("overlay".into(), vec![eco("soil.dds", 30.)]),
    ])
}

fn texture(name: &str) -> Option<Texture> {
    if name == "none" || name == "missing.dds" {
        return None;
    }
    Some(Texture {
        name: name.into(),
        width: 1,
        height: 1,
        rgba: vec![20, 80, 30, 255],
    })
}

#[test]
fn recipes_follow_emitted_materials_after_unpainted_tiles_and_holes_are_skipped() {
    let map = map();
    let baked = bake(&map, &ecosystems(), texture).unwrap();
    assert_eq!(baked.materials.len(), 2);
    assert_eq!(baked.meshes.len(), 2);
    assert_eq!(
        baked.terrain_materials.keys().copied().collect::<Vec<_>>(),
        [0, 1]
    );
    for (material, source_tile) in [(0, 2), (1, 3)] {
        let recipe = &baked.terrain_materials[&material];
        let expected = format!("__terrain_recipecase_{source_tile}.rgba");
        assert_eq!(recipe.fallback_texture, expected);
        assert_eq!(
            baked.materials[material].textures.as_slice(),
            std::slice::from_ref(&expected)
        );
        assert_eq!(baked.meshes[material].material, material);
        assert!(baked.materials[material].clamp_uv);
        assert!(baked.meshes[material].collidable);
        assert_eq!(baked.meshes[material].indices, [0, 1, 3, 0, 3, 2]);
        assert_eq!(baked.meshes[material].vertices[0], source_tile as f32 * 10.);
        assert!(recipe.base_texture.is_none());
        assert_eq!(recipe.layers.len(), map.tiles[source_tile].layers.len());
        for (actual, original) in recipe.layers.iter().zip(&map.tiles[source_tile].layers) {
            assert_eq!(actual.mask_size, original.mask_size);
            assert_eq!(actual.mask, original.mask);
        }
        assert_eq!(
            recipe.layers[0]
                .layers
                .iter()
                .map(|l| l.detail_map.as_str())
                .collect::<Vec<_>>(),
            ["soil.dds", "rock.dds"]
        );
        assert_eq!(recipe.layers[0].layers[1].repeat, 12.);
        assert_eq!(recipe.layers[0].layers[1].max_slope, 120.);
        assert_eq!(recipe.layers[0].layers[1].min_height, -42.);
        assert_eq!(recipe.layers[0].layers[1].max_height, -42.);
        let fallback = baked.textures.iter().find(|t| t.name == expected).unwrap();
        assert_eq!([fallback.width, fallback.height], [128; 2]);
        assert!(
            fallback
                .rgba
                .chunks_exact(4)
                .all(|pixel| pixel == [20, 80, 30, 255])
        );
    }
    assert_eq!(baked.terrain_materials[&0].layers[1].layers[0].repeat, 30.);
}

#[test]
fn native_sources_decode_once_and_missing_details_remain_unresolved() {
    let mut map = map();
    map.base_texture = "SOIL.DDS".into();
    let mut ecosystems = ecosystems();
    ecosystems
        .get_mut("base")
        .unwrap()
        .push(eco("missing.dds", 1.));
    ecosystems
        .get_mut("overlay")
        .unwrap()
        .push(eco("missing.dds", 2.));
    let mut calls = BTreeMap::new();
    let baked = bake(&map, &ecosystems, |name| {
        *calls.entry(name.to_owned()).or_insert(0) += 1;
        texture(name)
    })
    .unwrap();
    assert_eq!(
        calls,
        BTreeMap::from([
            ("missing.dds".into(), 1),
            ("rock.dds".into(), 1),
            ("soil.dds".into(), 1)
        ])
    );
    assert_eq!(
        baked.terrain_materials[&0].base_texture.as_deref(),
        Some("soil.dds")
    );
    let mut scene = Scene::from_geometry(
        "fixture".into(),
        baked.materials,
        baked.meshes,
        baked.textures,
    );
    scene.terrain_materials = baked.terrain_materials;
    for name in ["soil.dds", "rock.dds"] {
        let source = scene.texture(name).unwrap();
        assert_eq!([source.width, source.height], [1; 2]);
        assert_eq!(source.rgba, [20, 80, 30, 255]);
    }
    assert!(scene.texture("missing.dds").is_none());
    assert!(scene.terrain_materials.values().any(|recipe| {
        recipe
            .layers
            .iter()
            .flat_map(|l| &l.layers)
            .any(|l| l.detail_map == "missing.dds")
    }));
}

#[test]
fn object_extraction_does_not_apply_old_recipe_indices_to_remapped_materials() {
    let baked = bake(&map(), &ecosystems(), texture).unwrap();
    let mut scene = Scene::from_geometry(
        "fixture".into(),
        baked.materials,
        baked.meshes,
        baked.textures,
    );
    scene.terrain_materials = baked.terrain_materials;
    scene.objects.push(SceneObject {
        name: "object".into(),
        meshes: vec![1],
        collision_meshes: vec![],
    });
    let object = scene.object_model("object").unwrap();
    assert_eq!(object.meshes[0].material, 0);
    assert_eq!(
        object.materials[0].textures,
        ["__terrain_recipecase_3.rgba"]
    );
    assert!(object.terrain_materials.is_empty());
    assert_eq!(scene.terrain_materials.len(), 2);
    scene.materials.swap(0, 1);
    assert_ne!(
        scene.materials[0].textures,
        [scene.terrain_materials[&0].fallback_texture.clone()],
        "identity must reveal stale recipes after public material reordering"
    );
}

#[test]
#[ignore = "requires six original heightmap zone archives; CPU only"]
fn original_recipes_preserve_masks_and_full_detail_dimensions() {
    let base = loader::default_client_dir().expect("original client assets");
    for (zone, count, source_count, dims, max_layers, max_eco, masks) in [
        ("maidensgrave", 154, 6, vec![(256, 6)], 2, 3, 28),
        ("feerrott2", 329, 6, vec![(256, 6)], 4, 2, 231),
        ("oldcommons", 1552, 5, vec![(256, 5)], 5, 3, 208),
        (
            "deadhills",
            1233,
            8,
            vec![(256, 3), (512, 4), (1024, 1)],
            5,
            3,
            1598,
        ),
        ("lopingplains", 1100, 7, vec![(256, 7)], 3, 3, 226),
        ("buriedsea", 956, 6, vec![(256, 6)], 2, 3, 47),
    ] {
        let scene = loader::load_zone(&base, zone).unwrap();
        assert_eq!(scene.terrain_materials.len(), count, "{zone}");
        let mut names = BTreeSet::new();
        let mut actual_masks = 0;
        for (&index, recipe) in &scene.terrain_materials {
            assert_eq!(
                scene.materials[index].textures.as_slice(),
                std::slice::from_ref(&recipe.fallback_texture)
            );
            assert_eq!(
                [
                    scene.texture(&recipe.fallback_texture).unwrap().width,
                    scene.texture(&recipe.fallback_texture).unwrap().height
                ],
                [128; 2]
            );
            names.extend(recipe.base_texture.iter().cloned());
            assert_eq!(recipe.base_texture.is_none(), zone == "deadhills");
            for (layer_index, layer) in recipe.layers.iter().enumerate() {
                assert_eq!(layer.mask_size, if layer_index == 0 { 0 } else { 64 });
                assert_eq!(layer.mask.len(), layer.mask_size * layer.mask_size);
                actual_masks += usize::from(layer_index > 0);
                for sublayer in &layer.layers {
                    assert!(sublayer.repeat.is_finite() && sublayer.repeat > 0.);
                    names.insert(sublayer.detail_map.clone());
                }
            }
        }
        assert_eq!(actual_masks, masks, "{zone}");
        assert_eq!(
            scene
                .terrain_materials
                .values()
                .map(|r| r.layers.len())
                .max(),
            Some(max_layers)
        );
        assert_eq!(
            scene
                .terrain_materials
                .values()
                .flat_map(|r| &r.layers)
                .map(|l| l.layers.len())
                .max(),
            Some(max_eco)
        );
        assert_eq!(names.len(), source_count, "{zone}");
        let mut actual_dims = BTreeMap::new();
        for name in names {
            let image = scene.texture(&name).unwrap();
            assert_eq!(image.width, image.height);
            assert_eq!(
                image.rgba.len(),
                image.width as usize * image.height as usize * 4
            );
            *actual_dims.entry(image.width).or_insert(0) += 1;
        }
        assert_eq!(actual_dims, dims.into_iter().collect(), "{zone}");
    }
}
