//! Resolve supported indexed surfaces without changing finite-sheet handling.
use std::collections::BTreeMap;

use super::{Scene, register_texture};
use crate::{mesh::Material, terrain};

pub(super) fn append(scene: &mut Scene, map: &terrain::Heightmap, bytes: &[u8]) {
    let built = terrain::parse_water_data(bytes)
        .and_then(|water| terrain::indexed_water::bake(map, &water).map(|baked| (water, baked)));
    let (water, baked) = match built {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(zone = %scene.name, %error, "indexed water unavailable; retaining terrain and finite sheets");
            return;
        }
    };
    if !baked.diagnostics.is_empty() || baked.omitted_diagnostics != 0 {
        let details: Vec<_> = baked
            .diagnostics
            .iter()
            .map(|diagnostic| {
                let tile = &map.tiles[diagnostic.tile_index];
                (
                    tile.longitude,
                    tile.latitude,
                    diagnostic.material_index,
                    diagnostic.reason.as_str(),
                )
            })
            .collect();
        tracing::warn!(zone = %scene.name, ?details, omitted = baked.omitted_diagnostics, "skipped unsupported indexed water records");
    }
    let mut materials = BTreeMap::new();
    for surface in baked.surfaces {
        let material = *materials.entry(surface.material_index).or_insert_with(|| {
            let terrain::IndexedWaterResolution::Unique { definition, .. } =
                water.resolve_index(surface.material_index)
            else {
                return None;
            };
            append_material(scene, definition)
        });
        if let Some(material) = material {
            let mut geometry = surface.geometry;
            geometry.material = material;
            scene.meshes.push(geometry);
        }
    }
}

fn append_material(
    scene: &mut Scene,
    definition: &terrain::IndexedWaterDefinition,
) -> Option<usize> {
    // Authored Windows build paths are asset references. Resolve only their
    // basenames through the existing archive/known client texture index, never
    // as arbitrary host filesystem paths. Metadata retains the original text.
    let basename = |name: &str| name.rsplit(['\\', '/']).next().unwrap_or("").to_owned();
    let normal = basename(&definition.normal_map);
    let environment = definition
        .material
        .environment_map
        .as_deref()
        .map(basename)?;
    for name in ["water_c.bmp", normal.as_str(), environment.as_str()] {
        register_texture(scene, 0, name);
    }
    // This path has no invented substitute planes: unresolved or undecodable
    // maps omit the indexed material, with one diagnostic per selector.
    let valid_texture = |name: &str| {
        scene.texture(name).is_some_and(|texture| {
            texture.width != 0
                && texture.height != 0
                && texture.rgba.as_slice() != [255, 0, 255, 255]
        })
    };
    if !valid_texture("water_c.bmp")
        || !valid_texture(&normal)
        || scene.texture_cube(&environment).is_none()
    {
        tracing::warn!(zone = %scene.name, selector = definition.index, %normal, %environment, "indexed water maps unavailable; omitting this selector's surfaces");
        return None;
    }
    let mut water = definition.material.clone();
    water.indexed_uv_scale = Some(definition.uv_scale);
    water.environment_map = Some(environment);
    let material = scene.materials.len();
    scene.materials.push(Material {
        textures: vec!["water_c.bmp".into()],
        normal_map: Some(normal),
        water: Some(water),
        flags: 0,
        anim_speed: 0,
        alpha_mask: false,
        transparent: false,
        emissive: false,
    });
    Some(material)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mesh::Geometry,
        terrain::{
            Heightmap, TerrainOptions, TerrainTile, TerrainWaterExtension, TerrainWaterMetadata,
        },
    };

    const WATER: &str = "*WATERSHEETDATA\n*INDEX 1\n*UVSCALE 1\n*NORMALMAP missing-normal.dds\n*ENVIRONMENTMAP missing-cube.dds\n*WATERCOLOR1 0 0 0 1\n*WATERCOLOR2 0 0 0 1\n*REFLECTIONCOLOR 1 1 1 1\n*FRESNELBIAS 0.25\n*FRESNELPOWER 8\n*REFLECTIONAMOUNT 0.5\n*ENDWATERSHEETDATA\n";

    fn fixture() -> (Scene, Heightmap) {
        let scene = Scene::from_geometry(
            "preserved scene".into(),
            vec![Material {
                textures: vec!["kept.dds".into()],
                normal_map: None,
                water: None,
                flags: 0,
                anim_speed: 0,
                alpha_mask: false,
                transparent: false,
                emissive: false,
            }],
            vec![Geometry {
                vertices: vec![
                    123., 0., 0., 0., 0., 1., 0., 0., 124., 0., 0., 0., 0., 1., 1., 0., 123., 1.,
                    0., 0., 0., 1., 0., 1.,
                ],
                indices: vec![0, 1, 2],
                material: 0,
                collidable: true,
            }],
            vec![],
        );
        let map = Heightmap {
            options: TerrainOptions {
                name: "synthetic".into(),
                min_lng: 0,
                max_lng: 0,
                min_lat: 0,
                max_lat: 0,
                units_per_vertex: 1.,
                quads_per_tile: 1,
            },
            header: [21, 0, 0],
            base_texture: String::new(),
            tiles: vec![TerrainTile {
                longitude: 0,
                latitude: 0,
                heights: vec![],
                colors: vec![],
                secondary_colors: vec![],
                quad_flags: vec![],
                layers: vec![],
                water_level: -3.,
                water_metadata: TerrainWaterMetadata {
                    word_bits: 1,
                    extension: Some(TerrainWaterExtension {
                        tag: 1,
                        bounds: Some([0., 1., 0., 1.]),
                        trailing_value: 0.,
                    }),
                },
            }],
            placements: vec![],
            lights: vec![],
            groups: vec![],
            region_count: 0,
        };
        (scene, map)
    }

    fn assert_preserved(scene: &Scene) {
        assert_eq!(scene.meshes.len(), 1);
        assert_eq!(scene.meshes[0].vertices.len(), 24);
        assert_eq!(scene.meshes[0].vertices[0], 123.);
        assert_eq!(scene.meshes[0].indices, [0, 1, 2]);
        assert!(scene.meshes[0].collidable);
        assert_eq!(scene.materials.len(), 1);
        assert_eq!(scene.materials[0].textures, ["kept.dds"]);
        assert!(scene.collision_meshes.is_empty());
    }

    #[test]
    fn malformed_indexed_metadata_keeps_existing_scene() {
        let (mut scene, map) = fixture();
        append(
            &mut scene,
            &map,
            WATER.replace("*UVSCALE 1", "*UVSCALE invalid").as_bytes(),
        );
        assert_preserved(&scene);
    }

    #[test]
    fn excessive_indexed_geometry_keeps_existing_scene_without_partial_materials() {
        let (mut scene, mut map) = fixture();
        map.options.quads_per_tile = 512;
        map.tiles[0]
            .water_metadata
            .extension
            .as_mut()
            .unwrap()
            .bounds = Some([0., 512., 0., 512.]);
        map.tiles.resize(5, map.tiles[0].clone());
        append(&mut scene, &map, WATER.as_bytes());
        assert_preserved(&scene);
    }

    #[test]
    #[ignore = "requires original Feerrott2 archive as a real texture resolver"]
    fn missing_indexed_texture_does_not_create_placeholder_planes() {
        let (mut scene, map) = fixture();
        let base = super::super::default_client_dir().expect("original client assets required");
        scene
            .archives
            .push(crate::pfs::Archive::open(base.join("feerrott2.eqg")).unwrap());
        scene.loose_textures = super::super::loose_textures(&base);
        append(&mut scene, &map, WATER.as_bytes());
        assert_preserved(&scene);
    }
}
