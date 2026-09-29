use openeq_assets::pfs::Archive;
use openeq_assets::terrain::{
    Heightmap, IndexedWaterResolution, TerrainOptions, TerrainTile, TerrainWaterExtension,
    TerrainWaterMetadata, parse_ecosystem, parse_object_group, parse_water, parse_water_data,
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
        for material in &scene.materials {
            assert_eq!(
                material.clamp_uv,
                material
                    .textures
                    .first()
                    .is_some_and(|name| name.starts_with("__terrain_")),
                "{zone}: only baked tile images should clamp their edges"
            );
        }
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
        water_metadata: TerrainWaterMetadata::default(),
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
        water_metadata: TerrainWaterMetadata::default(),
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

fn water_metadata(word_bits: u32, tag: Option<i8>) -> TerrainWaterMetadata {
    TerrainWaterMetadata {
        word_bits,
        extension: tag.map(|tag| TerrainWaterExtension {
            tag,
            bounds: (tag != 0).then_some([0.0, 96.0, 112.0, 224.0]),
            trailing_value: -1000.0,
        }),
    }
}

fn water_dat_fixture(
    version: u32,
    records: &[TerrainWaterMetadata],
) -> (TerrainOptions, Vec<u8>, Vec<std::ops::Range<usize>>) {
    let options = TerrainOptions::parse(
        b"EQTZP *NAME water *MINLNG 0 *MAXLNG 1 *MINLAT 0 *MAXLAT 1 *UNITSPERVERT 16 *QUADSPERTILE 1",
    )
    .unwrap();
    let mut data = Vec::new();
    for word in [version, 0, 1] {
        data.extend(word.to_le_bytes());
    }
    data.extend(b"base\0");
    data.extend((records.len() as u32).to_le_bytes());
    let mut ranges = Vec::new();
    for (index, record) in records.iter().enumerate() {
        assert_eq!(record.extension.is_some(), version >= 21);
        for word in [100000u32 + index as u32, 100000, 0] {
            data.extend(word.to_le_bytes());
        }
        data.extend([0; 4 * 4 + 2 * 4 * 4 + 1]); // Heights, colors, quad flags.
        let start = data.len();
        data.extend((-50.0f32).to_le_bytes());
        data.extend(record.word_bits.to_le_bytes());
        if let Some(extension) = &record.extension {
            assert_eq!(extension.bounds.is_some(), extension.tag != 0);
            data.push(extension.tag as u8);
            if let Some(bounds) = extension.bounds {
                for value in bounds {
                    data.extend(value.to_le_bytes());
                }
            }
            data.extend(extension.trailing_value.to_le_bytes());
        }
        ranges.push(start..data.len());
        data.extend([0; 5 * 4]); // Layers, placements, regions, lights, groups.
    }
    (options, data, ranges)
}

#[test]
fn modern_tile_water_metadata_preserves_all_selectors_tags_and_exact_float_bits() {
    let mut unusual = water_metadata(2, Some(127));
    let extension = unusual.extension.as_mut().unwrap();
    extension.bounds = Some([
        f32::from_bits(0x8000_0000),
        f32::from_bits(0x7fc0_1234),
        112.0,
        224.0,
    ]);
    extension.trailing_value = f32::from_bits(0xffc0_5678);
    let records = [
        water_metadata(1, Some(1)),
        water_metadata(2, Some(0)),
        water_metadata(0, Some(0)),
        water_metadata(0xffff_ffff, Some(-128)),
        water_metadata(0x8000_0000, Some(-1)),
        unusual,
    ];
    let (options, data, ranges) = water_dat_fixture(21, &records);
    let map = Heightmap::parse(options, &data).unwrap();
    assert_eq!(map.tiles.len(), records.len());
    assert_eq!(
        ranges.iter().map(|r| r.len()).collect::<Vec<_>>(),
        [29, 13, 13, 29, 29, 29]
    );
    for (tile, expected) in map.tiles.iter().zip(&records) {
        assert_eq!(tile.water_level, -50.0);
        let actual = &tile.water_metadata;
        assert_eq!(actual.word_bits, expected.word_bits);
        assert_eq!(actual.material_index(), expected.material_index());
        match (&actual.extension, &expected.extension) {
            (Some(actual), Some(expected)) => {
                assert_eq!(actual.tag, expected.tag);
                assert_eq!(
                    actual.bounds.map(|b| b.map(f32::to_bits)),
                    expected.bounds.map(|b| b.map(f32::to_bits))
                );
                assert_eq!(
                    actual.trailing_value.to_bits(),
                    expected.trailing_value.to_bits()
                );
            }
            (None, None) => {}
            _ => panic!("water extension presence changed"),
        }
    }
    assert_eq!(
        map.tiles
            .iter()
            .map(|tile| tile.water_metadata.material_index())
            .collect::<Vec<_>>(),
        [Some(1), Some(2), Some(0), Some(-1), Some(i32::MIN), Some(2)]
    );
}

#[test]
fn legacy_tile_water_metadata_preserves_second_float_without_material_selector() {
    let records = [
        water_metadata(1000.0f32.to_bits(), None),
        water_metadata((-1000.0f32).to_bits(), None),
        water_metadata(0, None),
        water_metadata(0x8000_0000, None),
        water_metadata(1, None),
    ];
    let (options, data, ranges) = water_dat_fixture(20, &records);
    let map = Heightmap::parse(options, &data).unwrap();
    assert_eq!(map.tiles.len(), records.len());
    assert!(ranges.iter().all(|range| range.len() == 8));
    for (tile, expected) in map.tiles.iter().zip(&records) {
        assert_eq!(tile.water_level, -50.0);
        assert_eq!(tile.water_metadata.word_bits, expected.word_bits);
        assert!(tile.water_metadata.extension.is_none());
        assert_eq!(tile.water_metadata.material_index(), None);
    }
}

#[test]
fn tile_water_metadata_rejects_every_truncated_optional_field() {
    for (version, record) in [
        (21, water_metadata(0, Some(0))),
        (21, water_metadata(1, Some(1))),
        (21, water_metadata(0xffff_ffff, Some(-128))),
        (21, water_metadata(0x8000_0000, Some(-1))),
        (21, water_metadata(2, Some(127))),
        (20, water_metadata(1000.0f32.to_bits(), None)),
        (20, water_metadata((-1000.0f32).to_bits(), None)),
    ] {
        let (options, data, ranges) = water_dat_fixture(version, &[record]);
        let water = &ranges[0];
        for end in water.start..water.end {
            assert!(
                Heightmap::parse(options.clone(), &data[..end]).is_err(),
                "version {version}, end {end}"
            );
        }
        assert!(Heightmap::parse(options.clone(), &data[..data.len() - 1]).is_err());
        let mut trailing = data.clone();
        trailing.push(0);
        assert!(Heightmap::parse(options, &trailing).is_err());
    }
}

fn indexed_water(index: i32) -> String {
    format!(
        "*WATERSHEETDATA\n*INDEX {index}\n*FRESNELBIAS 0.250\n*FRESNELPOWER 8.000\n*REFLECTIONAMOUNT 0.700\n*UVSCALE 1.000\n*REFLECTIONCOLOR 0.700 1.000 1.000 1.000\n*WATERCOLOR1 0.000 0.040 0.110 1.000\n*WATERCOLOR2 0.000 0.230 0.170 1.000\n*NORMALMAP Resources\\WaterSwap\\water_n.dds\n*ENVIRONMENTMAP Resources\\WaterSwap\\water_e.dds\n*ENDWATERSHEETDATA\n"
    )
}

#[test]
fn indexed_water_preserves_records_fields_paths_and_finite_behavior() {
    let finite = "*WATERSHEET\n*MINX -2\n*MAXX 5\n*MINY 3\n*MAXY 8\n*ZHEIGHT -13\n*NORMALMAP Resources\\WaterSwap\\WATER_N.dds\n*END_SHEET\n";
    let first = indexed_water(1).replace(
        "*ENDWATERSHEETDATA",
        "*UNKNOWN A B\n*UNKNOWN C\n*ENDWATERSHEETDATA",
    );
    let text = format!(
        "{finite}*BEGIN_WATERSHEETDATA\n{first}{}*END_WATERSHEETDATA\n",
        indexed_water(2)
    );
    let data = parse_water_data(text.as_bytes()).unwrap();
    assert_eq!(data.finite_sheets, parse_water(finite.as_bytes()).unwrap());
    assert_eq!(parse_water(text.as_bytes()).unwrap(), data.finite_sheets);
    assert_eq!(
        data.indexed.iter().map(|d| d.index).collect::<Vec<_>>(),
        [1, 2]
    );
    let material = &data.indexed[0];
    assert_eq!(material.normal_map, "Resources\\WaterSwap\\water_n.dds");
    assert_eq!(
        material.material.environment_map.as_deref(),
        Some("Resources\\WaterSwap\\water_e.dds")
    );
    assert_eq!(material.material.fresnel_bias, 0.25);
    assert_eq!(material.material.reflection_color, [0.7, 1.0, 1.0, 1.0]);
    assert_eq!(material.fields[1].values, ["0.250"]);
    let unknown: Vec<_> = material
        .fields
        .iter()
        .filter(|field| field.key == "*UNKNOWN")
        .collect();
    assert_eq!(unknown.len(), 2);
    assert_eq!(unknown[0].values, ["A", "B"]);
    assert_eq!(unknown[1].values, ["C"]);
    assert_eq!(data.finite_sheets[0].normal_map, "water_n.dds");
}

#[test]
fn indexed_water_lookup_distinguishes_missing_identical_and_conflicting_definitions() {
    let one = indexed_water(1);
    let text = format!("{}{}{}", indexed_water(0), one, one);
    let data = parse_water_data(text.as_bytes()).unwrap();
    assert_eq!(data.indexed.len(), 3);
    assert!(matches!(
        data.resolve_index(2),
        IndexedWaterResolution::Missing
    ));
    match data.resolve_index(1) {
        IndexedWaterResolution::Unique {
            definition,
            occurrences,
        } => {
            assert_eq!(definition.index, 1);
            assert_eq!(occurrences, 2);
        }
        other => panic!("unexpected resolution: {other:?}"),
    }
    for changed in [
        one.replace("*FRESNELBIAS 0.250", "*FRESNELBIAS 0.150"),
        one.replace("*ENDWATERSHEETDATA", "*UNKNOWN kept\n*ENDWATERSHEETDATA"),
        one.replace("*UVSCALE 1.000", "*UVSCALE -0.000"),
    ] {
        let data = parse_water_data(format!("{one}{changed}").as_bytes()).unwrap();
        match data.resolve_index(1) {
            IndexedWaterResolution::Ambiguous { definitions } => assert_eq!(definitions.len(), 2),
            other => panic!("unexpected resolution: {other:?}"),
        }
    }
    let zero = parse_water_data(indexed_water(0).as_bytes()).unwrap();
    assert!(matches!(
        zero.resolve_index(1),
        IndexedWaterResolution::Missing
    ));
}

#[test]
fn indexed_water_rejects_malformed_required_fields_and_block_boundaries() {
    let valid = indexed_water(1);
    for invalid in [
        valid.replace("*INDEX 1\n", ""),
        valid.replace("*INDEX 1", "*INDEX invalid"),
        valid.replace("*INDEX 1", "*INDEX 2147483648"),
        valid.replace("*INDEX 1", "*INDEX 1\n*INDEX 2"),
        valid.replace("*FRESNELBIAS 0.250", "*FRESNELBIAS NaN"),
        valid.replace("*UVSCALE 1.000", "*UVSCALE 1 2"),
        valid.replace(
            "*REFLECTIONCOLOR 0.700 1.000 1.000 1.000",
            "*REFLECTIONCOLOR 0.7 1 1",
        ),
        valid.replace("*ENDWATERSHEETDATA\n", ""),
        valid.replace("*ENDWATERSHEETDATA", "*END_WATERSHEETDATA"),
        valid.replace("*ENDWATERSHEETDATA", "*END_SHEET"),
        format!("*WATERSHEETDATA\n{valid}"),
        "*ENDWATERSHEETDATA\n".to_owned(),
    ] {
        assert!(
            parse_water_data(invalid.as_bytes()).is_err(),
            "accepted {invalid}"
        );
        // The existing finite-only API continues to ignore indexed blocks.
        assert!(parse_water(invalid.as_bytes()).unwrap().is_empty());
    }
    assert!(parse_water_data(&[0xff]).is_err());
    let finite = "*WATERSHEET\n*UVSCALE 9\n*NORMALMAP outer.dds\n*END_SHEET\n";
    let nested = finite.replace("*END_SHEET", &format!("{valid}*END_SHEET"));
    assert!(parse_water_data(nested.as_bytes()).is_err());
    let nested = valid.replace("*ENDWATERSHEETDATA", &format!("{finite}*ENDWATERSHEETDATA"));
    assert!(parse_water_data(nested.as_bytes()).is_err());
    assert_eq!(
        parse_water_data(finite.as_bytes()).unwrap().finite_sheets,
        parse_water(finite.as_bytes()).unwrap()
    );
    assert_eq!(
        parse_water_data(indexed_water(-1).as_bytes())
            .unwrap()
            .indexed[0]
            .index,
        -1
    );
}

#[test]
#[ignore = "requires all six original client heightmap archives"]
fn original_six_zones_preserve_indexed_water_metadata_and_lookup() {
    use std::collections::BTreeMap;

    let base = openeq_assets::loader::default_client_dir().expect("original client directory");
    for (zone, internal, bytes, header, tiles, rectangles, word_counts, indices, finite) in [
        (
            "feerrott2",
            "feerrott",
            2_928_591,
            [21, 0, 12],
            329,
            65,
            vec![(1, 329)],
            vec![1],
            0,
        ),
        (
            "deadhills",
            "deadhills",
            7_985_399,
            [21, 0, 1],
            1233,
            204,
            vec![(1, 1233)],
            vec![1],
            0,
        ),
        (
            "lopingplains",
            "lopingplains",
            5_203_995,
            [21, 0, 1],
            1100,
            278,
            vec![(1, 1057), (2, 43)],
            vec![1, 2],
            0,
        ),
        (
            "buriedsea",
            "buriedsea",
            4_261_992,
            [21, 0, 20],
            961,
            900,
            vec![(1, 938), (2, 23)],
            vec![1, 2],
            0,
        ),
        (
            "oldcommons",
            "commonlands",
            6_948_252,
            [21, 0, 1],
            1552,
            0,
            vec![(1, 1552)],
            vec![0; 83],
            1,
        ),
        (
            "nektulos",
            "nektulos",
            6_735_104,
            [20, 0, 10],
            421,
            0,
            vec![(0xc47a_0000, 421)],
            vec![],
            1,
        ),
    ] {
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let options =
            TerrainOptions::parse(&archive.read(&format!("{internal}.zon")).unwrap()).unwrap();
        assert!(options.name.eq_ignore_ascii_case(internal), "{zone}");
        let dat = archive.read(&format!("{}.dat", options.name)).unwrap();
        assert_eq!(dat.len(), bytes, "{zone}");
        let map = Heightmap::parse(options, &dat).unwrap(); // Also requires exact EOF.
        assert_eq!(map.header, header, "{zone}");
        assert_eq!(map.tiles.len(), tiles, "{zone}");
        let mut actual_words = BTreeMap::new();
        let mut actual_rectangles = 0;
        let mut active_indices = BTreeMap::new();
        for tile in &map.tiles {
            *actual_words
                .entry(tile.water_metadata.word_bits)
                .or_insert(0) += 1;
            if let Some(extension) = &tile.water_metadata.extension {
                assert_eq!(
                    extension.trailing_value.to_bits(),
                    (-1000.0f32).to_bits(),
                    "{zone}"
                );
                assert!(matches!(extension.tag, 0 | 1), "{zone}");
                if extension.bounds.is_some() {
                    assert_eq!(extension.tag, 1, "{zone}");
                    actual_rectangles += 1;
                    *active_indices
                        .entry(tile.water_metadata.material_index().unwrap())
                        .or_insert(0) += 1;
                } else {
                    assert_eq!(extension.tag, 0, "{zone}");
                }
            } else {
                assert_eq!(zone, "nektulos");
                assert_eq!(tile.water_metadata.material_index(), None);
            }
        }
        assert_eq!(actual_words, word_counts.into_iter().collect(), "{zone}");
        assert_eq!(actual_rectangles, rectangles, "{zone}");
        let water_bytes = archive.read("water.dat").unwrap();
        let water = parse_water_data(&water_bytes).unwrap();
        assert_eq!(water.finite_sheets.len(), finite, "{zone}");
        assert_eq!(
            water.finite_sheets,
            parse_water(&water_bytes).unwrap(),
            "{zone}"
        );
        assert_eq!(
            water.indexed.iter().map(|d| d.index).collect::<Vec<_>>(),
            indices,
            "{zone}"
        );
        match zone {
            "feerrott2" => {
                let pond = &map.tiles[84];
                assert_eq!((pond.longitude, pond.latitude), (-3, -12));
                assert_eq!(pond.water_level, -50.0);
                assert_eq!(pond.water_metadata.word_bits, 1);
                let extension = pond.water_metadata.extension.as_ref().unwrap();
                assert_eq!(extension.bounds, Some([0.0, 96.0, 112.0, 224.0]));
                assert_eq!(dat[732_517..732_521], (-50.0f32).to_le_bytes());
                assert_eq!(dat[732_521..732_525], 1u32.to_le_bytes());
                assert_eq!(dat[732_525], 1);
                assert_eq!(
                    map.tiles[11].water_metadata.extension.as_ref().unwrap().tag,
                    0
                );
                assert_eq!(map.tiles[11].water_level, -30.0);
            }
            "lopingplains" => assert_eq!(active_indices, BTreeMap::from([(1, 236), (2, 42)])),
            "buriedsea" => {
                assert_eq!(active_indices, BTreeMap::from([(1, 878), (2, 22)]));
                let IndexedWaterResolution::Unique {
                    definition,
                    occurrences: 1,
                } = water.resolve_index(2)
                else {
                    panic!("Buried Sea index 2 must resolve uniquely");
                };
                assert_eq!(definition.material.fresnel_bias, 0.15);
                assert_eq!(definition.material.fresnel_power, 8.0);
                assert_eq!(definition.material.reflection_amount, 0.3);
                assert_eq!(definition.material.color1, [0.2, 0.2, 0.2, 1.0]);
            }
            "oldcommons" => {
                assert!(matches!(
                    water.resolve_index(1),
                    IndexedWaterResolution::Missing
                ));
                assert!(matches!(
                    water.resolve_index(0),
                    IndexedWaterResolution::Unique {
                        occurrences: 83,
                        ..
                    }
                ));
            }
            "nektulos" => assert!(matches!(
                water.resolve_index(1),
                IndexedWaterResolution::Missing
            )),
            _ => {}
        }
        if matches!(
            zone,
            "feerrott2" | "deadhills" | "lopingplains" | "buriedsea"
        ) {
            for tile in &map.tiles {
                assert!(
                    matches!(
                        water.resolve_index(tile.water_metadata.material_index().unwrap()),
                        IndexedWaterResolution::Unique { occurrences: 1, .. }
                    ),
                    "{zone}"
                );
            }
        }
        assert!(
            Heightmap::parse(map.options.clone(), &dat[..dat.len() - 1]).is_err(),
            "{zone}"
        );
        eprintln!(
            "{zone}: {tiles} tiles, {rectangles} quartets, {} indexed definitions; exact EOF {bytes}",
            water.indexed.len()
        );
    }
}
