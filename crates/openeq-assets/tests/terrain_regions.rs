use openeq_assets::{
    liquid_regions::{LiquidKind, LiquidRegions, LiquidSpan},
    pfs::Archive,
    terrain::{
        Heightmap, TerrainOptions, TerrainPlacement, TerrainRegion,
        regions::{
            NativeRegionBox, NativeTopLevelRegions, native_anchor_height, native_type_word,
            top_level_registration_order,
        },
    },
};

fn record(name: &str, raw_type: u32) -> TerrainRegion {
    TerrainRegion {
        source_offset: 0,
        tile_index: 0,
        index_in_tile: 0,
        enclosing_grid: [0, 0],
        name: name.into(),
        type_id: raw_type,
        alternate_name: "Authored Alternate Name".into(),
        grid: [100000, 100000],
        position: [5., 5., 0.],
        rotation_degrees: [0.; 3],
        scale: [1.; 3],
        full_size: [100.; 3],
    }
}

fn options() -> TerrainOptions {
    TerrainOptions {
        name: "fixture".into(),
        min_lng: -2,
        max_lng: 2,
        min_lat: -2,
        max_lat: 2,
        units_per_vertex: 10.,
        quads_per_tile: 1,
    }
}

fn word(data: &mut Vec<u8>, value: u32) {
    data.extend(value.to_le_bytes());
}

fn string(data: &mut Vec<u8>, value: &str) {
    data.extend(value.as_bytes());
    data.push(0);
}

fn floats(data: &mut Vec<u8>, values: &[f32]) {
    for value in values {
        data.extend(value.to_le_bytes());
    }
}

/// Emits the original version-21 grammar, not an alternative JSON fixture.
fn dat(tiles: Vec<([i32; 2], Vec<TerrainRegion>)>) -> Vec<u8> {
    let mut data = Vec::new();
    for value in [21, 0, 1] {
        word(&mut data, value);
    }
    string(&mut data, "fixture.dds");
    word(&mut data, tiles.len() as u32);
    for (grid, regions) in tiles {
        for value in grid {
            word(&mut data, (value + 100000) as u32);
        }
        word(&mut data, 0); // Editor identifier.
        floats(&mut data, &[0.; 4]);
        for _ in 0..8 {
            word(&mut data, 0); // Both vertex-color arrays.
        }
        data.push(0); // Quad flags.
        floats(&mut data, &[-1000.]);
        word(&mut data, u32::MAX);
        data.push(0); // Version-21 water tag.
        floats(&mut data, &[0.]);
        word(&mut data, 0); // Layers.
        word(&mut data, 0); // Single-object placements.
        word(&mut data, regions.len() as u32);
        for region in regions {
            string(&mut data, &region.name);
            word(&mut data, region.type_id);
            string(&mut data, &region.alternate_name);
            for value in grid {
                word(&mut data, (value + 100000) as u32);
            }
            floats(&mut data, &region.position);
            floats(&mut data, &region.rotation_degrees);
            floats(&mut data, &region.scale);
            floats(&mut data, &region.full_size);
        }
        word(&mut data, 0); // Lights.
        word(&mut data, 0); // Groups.
    }
    data
}

fn fixture(regions: Vec<TerrainRegion>) -> Heightmap {
    Heightmap::parse(options(), &dat(vec![([0, 0], regions)])).unwrap()
}

#[test]
fn dat_retains_region_fields_spelling_and_source_order() {
    let mut first = record("AWT_Mixed_Case", 10);
    first.position = [2.5, 3.5, -12.];
    first.rotation_degrees = [11., 22., -35.];
    first.scale = [2., 3., 4.];
    first.full_size = [-10., 20., 30.];
    let bytes = dat(vec![
        ([1, -2], vec![first.clone(), record("unknown_name", 0x1234)]),
        ([-1, 2], vec![record("ATP_1", 0)]),
    ]);
    let map = Heightmap::parse(options(), &bytes).unwrap();
    assert_eq!(map.region_count, 3);
    assert_eq!(map.regions.len(), 3);
    let actual = &map.regions[0];
    assert_eq!(actual.tile_index, 0);
    assert_eq!(actual.index_in_tile, 0);
    assert_eq!(actual.enclosing_grid, [1, -2]);
    assert_eq!(actual.grid, [100001, 99998]);
    assert_eq!(actual.name, first.name);
    assert_eq!(actual.alternate_name, first.alternate_name);
    assert_eq!(actual.type_id, 10);
    assert_eq!(actual.position, first.position);
    assert_eq!(actual.rotation_degrees, first.rotation_degrees);
    assert_eq!(actual.scale, first.scale);
    assert_eq!(actual.full_size, first.full_size);
    assert_eq!(
        &bytes[actual.source_offset..actual.source_offset + actual.name.len()],
        actual.name.as_bytes()
    );
    assert_eq!(map.regions[1].index_in_tile, 1);
    assert_eq!(map.regions[2].tile_index, 1);
    assert_eq!(map.regions[2].index_in_tile, 0);
    assert!(
        map.regions
            .windows(2)
            .all(|r| r[0].source_offset < r[1].source_offset)
    );
    // Retention does not imply the nonunit/tilted/signed transform is supported.
    assert!(NativeRegionBox::from_record(&map, 0).is_err());
}

#[test]
fn truncated_and_nonfinite_region_records_fail_before_queries() {
    let bytes = dat(vec![([0, 0], vec![record("AWT_pool", 0)])]);
    let map = Heightmap::parse(options(), &bytes).unwrap();
    for length in map.regions[0].source_offset..bytes.len() {
        assert!(Heightmap::parse(options(), &bytes[..length]).is_err());
    }
    for field in 0..4 {
        let mut bad = record("AWT_pool", 0);
        match field {
            0 => bad.position[2] = f32::NAN,
            1 => bad.rotation_degrees[0] = f32::INFINITY,
            2 => bad.scale[1] = f32::NEG_INFINITY,
            _ => bad.full_size[0] = f32::NAN,
        }
        assert!(Heightmap::parse(options(), &dat(vec![([0, 0], vec![bad])])).is_err());
    }
}

#[test]
fn authored_diagonal_selects_all_four_native_height_planes() {
    let mut map = fixture(vec![]);
    let tile = &mut map.tiles[0];
    // h00=0, h10=10, h01=20, h11=40. Bilinear height would
    // differ from either piecewise-planar surface away from the edges.
    tile.heights = vec![0., 10., 20., 40.];
    assert_eq!(
        native_anchor_height(&map.options, tile, [7.5, 2.5]).unwrap(),
        15.
    );
    assert_eq!(
        native_anchor_height(&map.options, tile, [2.5, 7.5]).unwrap(),
        20.
    );
    assert_eq!(
        native_anchor_height(&map.options, tile, [5., 5.]).unwrap(),
        20.
    );
    tile.quad_flags[0] = 0x80;
    assert_eq!(
        native_anchor_height(&map.options, tile, [2.5, 2.5]).unwrap(),
        7.5
    );
    assert_eq!(
        native_anchor_height(&map.options, tile, [7.5, 7.5]).unwrap(),
        27.5
    );
    assert_eq!(
        native_anchor_height(&map.options, tile, [5., 5.]).unwrap(),
        15.
    );
    // A hidden render quad still has an authored anchor height.
    tile.quad_flags[0] = 0x81;
    assert_eq!(
        native_anchor_height(&map.options, tile, [5., 5.]).unwrap(),
        15.
    );
    // Render/object anchoring now follows the same cached diagonal.
    assert_eq!(tile.height_at(&map.options, 5., 5.), 15.);
    for point in [[0., 5.], [10., 5.], [-1., 5.], [5., 11.], [f32::NAN, 5.]] {
        assert!(native_anchor_height(&map.options, tile, point).is_err());
    }
    tile.quad_flags.clear();
    assert!(native_anchor_height(&map.options, tile, [5., 5.]).is_err());
}

#[test]
fn native_classifier_overwrites_liquid_byte_but_preserves_modifier_types() {
    for (prefix, kind) in [("AWT", 5), ("ALV", 7), ("AVW", 8)] {
        for raw in [0, 1, 5, 9, 10, 255, 0x1234_5678] {
            assert_eq!(
                native_type_word(&format!("{prefix}_pool"), raw),
                Some((raw & 0xffff_ff00) | kind)
            );
        }
    }
    for (prefix, modifier) in [
        ("APK", 0x4000_0000),
        ("ATP", 0x8000_0000),
        ("ASL", 0x1000_0000),
    ] {
        assert_eq!(
            native_type_word(&format!("{prefix}_area"), 0),
            Some(modifier)
        );
        assert_eq!(
            native_type_word(&format!("{prefix}_area"), 5),
            Some(modifier | 5)
        );
    }
    assert_eq!(native_type_word("APV_generic", 7), Some(7));
    assert_eq!(native_type_word("AXX_unknown", 5), Some(5));
    assert_eq!(native_type_word("AWT", 0), Some(0));
    assert_eq!(native_type_word("", 11), Some(11));
    assert_eq!(native_type_word("Awt_mixed", 0), Some(0));
    assert_eq!(native_type_word("awt_lower", 0), None);
    assert_eq!(native_type_word("WT_classic", 0), None);
}

#[test]
fn registration_uses_atp_priority_grid_order_and_tile_source_order() {
    let map = Heightmap::parse(
        options(),
        &dat(vec![
            (
                [1, 0],
                vec![record("AWT_first_disk", 0), record("ATP_east", 0)],
            ),
            ([0, 1], vec![record("AWT_north", 0)]),
            ([-1, 0], vec![record("AWT_west", 0), record("ATP_west", 0)]),
            (
                [0, 0],
                vec![
                    record("APV_generic", 0),
                    record("APK_dry", 0),
                    record("AWT_center1", 0),
                    record("AWT_center2", 0),
                ],
            ),
        ]),
    )
    .unwrap();
    let order = top_level_registration_order(&map);
    let names: Vec<_> = order
        .iter()
        .map(|&i| map.regions[i].name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "ATP_west",
            "ATP_east",
            "AWT_west",
            "APV_generic",
            "APK_dry",
            "AWT_center1",
            "AWT_center2",
            "AWT_north",
            "AWT_first_disk"
        ]
    );
    assert_eq!(map.regions[0].name, "AWT_first_disk");
    let set = NativeTopLevelRegions::from_heightmap(&map).unwrap();
    let dry = set.at([0.; 3], None).unwrap();
    assert_eq!(dry.name, "ATP_west");
    assert_eq!(dry.effective_type, Some(0x8000_0000));
    assert_eq!(set.at([0.; 3], Some(*b"AWT")).unwrap().name, "AWT_west");
    assert_eq!(set.at([0.; 3], Some(*b"APV")).unwrap().name, "APV_generic");
    assert_eq!(set.at([0.; 3], Some(*b"ALV")).unwrap().name, "ATP_west");
    assert!(set.at([1000.; 3], None).is_none());
    assert!(set.at([f64::NAN, 0., 0.], None).is_none());
}

#[test]
fn generic_apv_is_skipped_but_dry_and_unknown_winners_are_retained() {
    for first in [
        record("APK_dry", 0),
        record("ASL_dry", 0),
        record("AXX_unknown", 0),
        record("other_unknown", 0),
    ] {
        let map = fixture(vec![
            record("APV_generic", 5),
            first.clone(),
            record("AWT_wet", 0),
        ]);
        let set = NativeTopLevelRegions::from_heightmap(&map).unwrap();
        let selected = set.at([5., 5., 0.], None).unwrap();
        assert_eq!(selected.name, first.name);
        assert_eq!(
            selected.effective_type,
            native_type_word(&first.name, first.type_id)
        );
        assert_eq!(
            set.at([5., 5., 0.], Some(*b"AWT")).unwrap().effective_type,
            Some(5)
        );
    }
    let map = fixture(vec![record("APV_only", 5)]);
    let set = NativeTopLevelRegions::from_heightmap(&map).unwrap();
    assert!(set.at([5., 5., 0.], None).is_none());
    assert_eq!(
        set.at([5., 5., 0.], Some(*b"APV")).unwrap().effective_type,
        Some(5)
    );
}

#[test]
fn finite_box_has_six_inclusive_faces_and_swept_crossings() {
    let mut region = record("AWT_pool", 0);
    region.full_size = [4., 6., 8.];
    region.position[2] = -10.;
    let map = fixture(vec![region]);
    let volume = NativeRegionBox::from_record(&map, 0).unwrap();
    let center = volume.center.map(f64::from);
    assert_eq!(center, [5., 5., -10.]);
    for axis in 0..3 {
        for sign in [-1., 1.] {
            let mut face = center;
            face[axis] += sign * f64::from(volume.half_extents[axis]);
            assert!(volume.contains(face));
            face[axis] += sign * 0.001;
            assert!(!volume.contains(face));
        }
    }
    let from = [-10., 5., -10.];
    let to = [20., 5., -10.];
    assert!(!volume.contains(from) && !volume.contains(to));
    let [enter, exit] = volume.segment(from, to).unwrap();
    assert!((enter - 13. / 30.).abs() < 1e-12);
    assert!((exit - 17. / 30.).abs() < 1e-12);
    assert_eq!(
        volume.segment([5., 5., -20.], [5., 5., 0.]),
        Some([0.3, 0.7])
    );
    assert!(volume.segment([-10., 5., 100.], [20., 5., 100.]).is_none());
    assert!(volume.segment(center, center).is_none());
    assert!(volume.segment([f64::INFINITY, 0., 0.], to).is_none());
    let mut tiny = record("AWT_small", 0);
    tiny.full_size = [0.01; 3];
    let tiny = NativeRegionBox::from_record(&fixture(vec![tiny]), 0).unwrap();
    assert!(tiny.segment([f64::MAX, 5., 0.], [5., 5., 0.]).is_none());
}

#[test]
fn native_yaw_quantizes_toward_zero_and_rotates_nonsquare_boxes() {
    for (degrees, units) in [
        (-35., -49),
        (-20., -28),
        (-15., -21),
        (-0.5, 0),
        (45., 64),
        (90., 128),
        (360., 512),
    ] {
        let mut region = record("AWT_rotated", 0);
        region.rotation_degrees[2] = degrees;
        region.full_size = [4., 20., 8.];
        let volume = NativeRegionBox::from_record(&fixture(vec![region]), 0).unwrap();
        assert_eq!(volume.yaw_units, units);
        let angle = f64::from(units) * std::f64::consts::TAU / 512.;
        let (sin, cos) = angle.sin_cos();
        let center = volume.center.map(f64::from);
        assert!(volume.contains([center[0] - sin * 9.9, center[1] + cos * 9.9, center[2]]));
        assert!(!volume.contains([center[0] - sin * 10.1, center[1] + cos * 10.1, center[2]]));
        assert!(!volume.contains([center[0] + cos * 2.1, center[1] + sin * 2.1, center[2]]));
    }
}

#[test]
fn unsupported_records_fail_the_set_instead_of_exposing_water_behind_them() {
    let original = fixture(vec![record("ATP_dry", 0), record("AWT_wet", 0)]);
    for variant in 0..10 {
        let mut map = original.clone();
        match variant {
            0 => map.regions[0].scale[0] = 2.,
            1 => map.regions[0].rotation_degrees[1] = 10.,
            2 => map.regions[0].grid[0] += 1,
            3 => map.regions[0].position[0] = 10.,
            4 => map.regions[0].full_size[0] = -10.,
            5 => map.regions[0].position[2] = f32::NAN,
            6 => map.regions[0].tile_index = 9,
            7 => map.regions[0].rotation_degrees[2] = f32::MAX,
            8 => map.regions[0].full_size[0] = f32::from_bits(1),
            _ => map.header[0] = 22,
        }
        assert!(
            NativeTopLevelRegions::from_heightmap(&map).is_err(),
            "variant {variant}"
        );
    }
    let mut groups = original.clone();
    groups.groups.push(TerrainPlacement {
        model: "unresolved.tog".into(),
        transform: glam::Mat4::IDENTITY,
    });
    assert!(NativeTopLevelRegions::from_heightmap(&groups).is_err());
    assert!(NativeRegionBox::from_record(&groups, 0).is_ok());
    let mut duplicate = original.clone();
    duplicate.tiles.push(duplicate.tiles[0].clone());
    assert!(NativeTopLevelRegions::from_heightmap(&duplicate).is_err());
    let mut missing = original;
    missing.regions.remove(0);
    assert!(NativeTopLevelRegions::from_heightmap(&missing).is_err());
}

#[test]
fn afg_special_constructor_rejects_the_whole_set_before_exposing_later_water() {
    let mut water = record("AWT_pool", 0);
    water.full_size = [10.; 3];
    let point = [5., 7., 0.];
    let water_only = LiquidRegions::from_heightmap(&fixture(vec![water.clone()])).unwrap();
    assert_eq!(water_only.at(point), Some(LiquidKind::Water));

    // Native AFG [6,2,2] expands to [6,6,2]. Its dry area therefore includes
    // this point, which would fall through to water with an ordinary box.
    let mut native_shape = record("AXX_native_afg_shape", 0);
    native_shape.full_size = [6., 6., 2.];
    let native_precedence =
        LiquidRegions::from_heightmap(&fixture(vec![native_shape, water.clone()])).unwrap();
    assert_eq!(native_precedence.at(point), None);

    // Square AFG records also remain outside the supported constructor subset.
    for dimensions in [[6., 2., 2.], [6., 6., 2.]] {
        let mut fog = record("AFG_fog", 0);
        fog.full_size = dimensions;
        let mut map = fixture(vec![fog, water.clone()]);
        assert!(NativeRegionBox::from_record(&map, 0).is_err());
        assert!(NativeTopLevelRegions::from_heightmap(&map).is_err());
        assert!(LiquidRegions::from_heightmap(&map).is_err());
        map.groups.push(TerrainPlacement {
            model: "trees".into(),
            transform: glam::Mat4::IDENTITY,
        });
        assert!(
            LiquidRegions::from_heightmap_with_groups(&map, |_| {
                Ok(REGION_FREE_GROUP.as_bytes().to_vec())
            })
            .is_err()
        );
    }
}

fn original_zone(zone: &str) -> Option<Heightmap> {
    let base = openeq_assets::loader::default_client_dir()?;
    let path = base.join(format!("{zone}.eqg"));
    if !path.is_file() {
        return None;
    }
    let archive = Archive::open(path).unwrap();
    let zon = archive
        .names()
        .iter()
        .find(|name| name.ends_with(".zon"))
        .unwrap();
    let options = TerrainOptions::parse(&archive.read(zon).unwrap()).unwrap();
    let data = archive.read(&format!("{}.dat", options.name)).unwrap();
    Some(Heightmap::parse(options, &data).unwrap())
}

#[test]
fn original_dat_regions_retain_all_66_audited_transforms() {
    for (zone, count) in [
        ("feerrott2", 11),
        ("deadhills", 24),
        ("lopingplains", 12),
        ("buriedsea", 8),
        ("oldcommons", 6),
        ("nektulos", 5),
    ] {
        let Some(map) = original_zone(zone) else {
            continue;
        };
        assert_eq!(map.regions.len(), count, "{zone}");
        for (index, region) in map.regions.iter().enumerate() {
            let volume = NativeRegionBox::from_record(&map, index)
                .unwrap_or_else(|error| panic!("{zone}/{}: {error}", region.name));
            assert_eq!(region.scale, [1.; 3]);
            assert!(volume.contains(volume.center.map(f64::from)));
            if region.name.starts_with("AWT") {
                assert_eq!(volume.effective_type, Some(5));
            } else {
                assert!(region.name.starts_with("ATP"));
                assert_eq!(region.type_id, 0);
                assert_eq!(volume.effective_type, Some(0x8000_0000));
            }
        }
        if !map.groups.is_empty() {
            assert!(NativeTopLevelRegions::from_heightmap(&map).is_err());
        }
    }
}

#[test]
fn original_negative_diagonals_match_native_height_replay() {
    // Expected values come from independent replay of the native x87 plane
    // expressions, not from TerrainTile::height_at or this decoder.
    for (zone, name, expected) in [
        ("deadhills", "AWT_dh_pools_3B", -81.94614347993775),
        ("lopingplains", "AWT_river7", 91.04486062283831),
        ("oldcommons", "AWT_lake", -131.98532605387481),
    ] {
        let Some(map) = original_zone(zone) else {
            continue;
        };
        let region = map.regions.iter().find(|r| r.name == name).unwrap();
        let tile = &map.tiles[region.tile_index];
        let [x, y, _] = region.position;
        let height = native_anchor_height(&map.options, tile, [x, y]).unwrap();
        assert!((f64::from(height) - expected).abs() < 0.00002, "{zone}");
        assert!((height - tile.height_at(&map.options, x, y)).abs() < 0.0001);
        // Preserve the evidence that these original records distinguish the
        // cached diagonal from the former all-positive grid well beyond roundoff.
        let mut legacy = tile.clone();
        for flag in &mut legacy.quad_flags {
            *flag &= !0x80;
        }
        assert!((height - legacy.height_at(&map.options, x, y)).abs() > 0.5);
    }
}

#[test]
fn original_pond_and_rotated_pool_match_finite_native_fixtures() {
    for (zone, name, offset, center, wet, dry) in [
        (
            "feerrott2",
            "AWT_small_pond",
            738595,
            [-740.842966, -2896.214996, -55.802547],
            [-665.942966, -2896.214996, -55.802547],
            [-665.742966, -2896.214996, -55.802547],
        ),
        (
            "deadhills",
            "AWT_dh_pools_3C",
            611587,
            [-1233.036144, -137.548630, -93.582024],
            [-1182.176854, -63.418051, -93.582024],
            [-1182.063708, -63.253134, -93.582024],
        ),
    ] {
        let Some(map) = original_zone(zone) else {
            continue;
        };
        let index = map.regions.iter().position(|r| r.name == name).unwrap();
        assert_eq!(map.regions[index].source_offset, offset);
        let volume = NativeRegionBox::from_record(&map, index).unwrap();
        for (actual, expected) in volume.center.into_iter().zip(center) {
            assert!(
                (f64::from(actual) - expected).abs() < 0.0002,
                "{zone}: {actual} vs {expected}"
            );
        }
        assert!(volume.contains(wet));
        assert!(!volume.contains(dry));
        for sign in [-1., 1.] {
            let mut point = center;
            point[2] += sign * (f64::from(volume.half_extents[2]) + 0.1);
            assert!(!volume.contains(point));
        }
        if zone == "deadhills" {
            assert_eq!(volume.yaw_units, -49);
            assert_eq!(map.regions[0].name, "AWT_dh_pools_3F");
            let first = top_level_registration_order(&map)[0];
            assert_eq!(map.regions[first].name, "AWT_dh_ocean_sidebay");
            // Adjacent 3D overlaps 3C's +X face. Leaving 3C alone is not a
            // whole-zone dry assertion; the neighboring authored box matters.
            let adjacent = map
                .regions
                .iter()
                .position(|r| r.name == "AWT_dh_pools_3D")
                .unwrap();
            let adjacent = NativeRegionBox::from_record(&map, adjacent).unwrap();
            let angle = -49. * std::f64::consts::TAU / 512.;
            let point = [
                center[0] + angle.cos() * 40.1,
                center[1] + angle.sin() * 40.1,
                center[2],
            ];
            assert!(!volume.contains(point));
            assert!(adjacent.contains(point));
        }
        // Check only decoded top-level boxes explicitly. This is not proof
        // about groups or a claim that EQG gameplay volumes are enabled.
        assert!(
            map.regions
                .iter()
                .enumerate()
                .all(|(i, _)| !NativeRegionBox::from_record(&map, i).unwrap().contains(dry))
        );
    }
}

#[test]
fn unsupported_eqg_zones_keep_gameplay_volumes_empty() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    // Missing groups, embedded areas and Arelis's AFG constructor still
    // cannot provide a complete supported region set.
    for zone in ["deadhills", "oceangreenhills", "shardslanding", "arelis"] {
        if base.join(format!("{zone}.eqg")).is_file() {
            assert!(LiquidRegions::load(&base, zone).unwrap().is_empty());
        }
    }
}

const REGION_FREE_GROUP: &str = "*BEGIN_OBJECTGROUP\n\
    *BEGIN_OBJECT\n\
    *NAME tree\n\
    *POSITION 1 2 3\n\
    *ROTATION 0 0 -35\n\
    *SCALE 1\n\
    *FILE LIT tree.lit\n\
    *END_OBJECT\n\
    *END_OBJECTGROUP\n";

#[test]
fn verified_region_free_groups_preserve_dry_precedence_and_finite_water() {
    let mut blocker = record("ATP_dry", 0);
    blocker.full_size = [2., 4., 4.];
    let mut water = record("AWT_pool", 0);
    water.full_size = [8., 4., 4.];
    let mut map = fixture(vec![water, blocker]);
    let baseline = LiquidRegions::from_heightmap(&map).unwrap();
    for _ in 0..2 {
        map.groups.push(TerrainPlacement {
            model: "trees".into(),
            transform: glam::Mat4::IDENTITY,
        });
    }
    assert!(LiquidRegions::from_heightmap(&map).is_err());
    let mut calls = 0;
    let regions = LiquidRegions::from_heightmap_with_groups(&map, |name| {
        assert_eq!(name, "trees");
        calls += 1;
        Ok(REGION_FREE_GROUP.as_bytes().to_vec())
    })
    .unwrap();
    assert_eq!(calls, 1, "repeated placements share one completeness check");
    assert_eq!(regions.at([5., 5., 0.]), None);
    assert_eq!(regions.at([2., 5., 0.]), Some(LiquidKind::Water));
    assert_eq!(regions.at([2., 5., 3.]), None);
    assert_eq!(
        regions.segment([0., 5., 0.], [10., 5., 0.]),
        baseline.segment([0., 5., 0.], [10., 5., 0.])
    );
}

#[test]
fn missing_unknown_truncated_and_area_bearing_groups_cannot_expose_top_level_water() {
    let mut map = fixture(vec![record("AWT_pool", 0)]);
    map.groups.push(TerrainPlacement {
        model: "fixture".into(),
        transform: glam::Mat4::IDENTITY,
    });
    assert!(
        LiquidRegions::from_heightmap_with_groups(&map, |_| Err(openeq_assets::Error::NotFound(
            "fixture.tog".into()
        )))
        .is_err()
    );
    let variants = [
        String::new(),
        REGION_FREE_GROUP.replace("*END_OBJECTGROUP", ""),
        REGION_FREE_GROUP.replace("*END_OBJECT\n", ""),
        REGION_FREE_GROUP.replace("*POSITION 1 2 3", "*POSITION 1 2"),
        REGION_FREE_GROUP.replace("*POSITION 1 2 3", "*POSITION NaN 2 3"),
        REGION_FREE_GROUP.replace("*NAME tree", "*NAME tree\n*NAME other"),
        REGION_FREE_GROUP.replace("*FILE LIT", "*FILE TOG"),
        REGION_FREE_GROUP.replace(
            "*END_OBJECTGROUP",
            "*BEGIN_AREA\n*NAME ATP_dry\n*END_AREA\n*END_OBJECTGROUP",
        ),
        REGION_FREE_GROUP.replace(
            "*END_OBJECTGROUP",
            "*BEGIN_AREA\n*NAME AWT_hidden\n*END_AREA\n*END_OBJECTGROUP",
        ),
        REGION_FREE_GROUP.replace("*END_OBJECTGROUP", "*BEGIN_UNKNOWN\n*END_OBJECTGROUP"),
        format!("{REGION_FREE_GROUP}*BEGIN_AREA\n"),
        format!("{REGION_FREE_GROUP}\0"),
    ];
    for (index, text) in variants.into_iter().enumerate() {
        assert!(
            LiquidRegions::from_heightmap_with_groups(&map, |_| Ok(text.as_bytes().to_vec()))
                .is_err(),
            "variant {index}"
        );
    }
    assert!(LiquidRegions::from_heightmap_with_groups(&map, |_| Ok(vec![0xff])).is_err());
}

#[test]
#[ignore = "requires original Feerrott2 and Loping Plains groups; CPU only"]
fn original_grouped_terrain_enables_verified_pond_and_river_volumes() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    let regions = LiquidRegions::load(&base, "feerrott2").unwrap();
    assert!(!regions.is_empty());
    // Independent native-reader fixtures recorded before group support.
    let center = [-740.84296, -2896.215, -55.802547];
    assert_eq!(regions.at(center), Some(LiquidKind::Water));
    assert_eq!(
        regions.at([-665.94296, center[1], center[2]]),
        Some(LiquidKind::Water)
    );
    for point in [
        [-665.743, center[1], center[2]],
        [center[0], center[1], -50.702547],
        [center[0], center[1], -60.902547],
    ] {
        assert_eq!(regions.at(point), None);
    }
    let spans = regions.segment(
        [center[0], center[1], center[2] - 10.],
        [center[0], center[1], center[2] + 10.],
    );
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, LiquidKind::Water);
    assert!((spans[0].enter - 0.25).abs() < 0.0001 && (spans[0].exit - 0.75).abs() < 0.0001);
    let map = original_zone("lopingplains").expect("original Loping Plains");
    assert!(!map.groups.is_empty());
    let region_index = map
        .regions
        .iter()
        .position(|region| region.name == "AWT_river7")
        .unwrap();
    let volume = NativeRegionBox::from_record(&map, region_index).unwrap();
    let regions = LiquidRegions::load(&base, "lopingplains").unwrap();
    assert!(!regions.is_empty());
    assert_eq!(regions.at(volume.center), Some(LiquidKind::Water));
}

#[test]
fn runtime_heightmap_known_names_override_raw_liquid_byte() {
    for (name, kind) in [
        ("AWT_pool", LiquidKind::Water),
        ("ALV_pool", LiquidKind::Lava),
        ("AVW_pool", LiquidKind::FreezingWater),
    ] {
        for raw in [0, 1, 10, 0x1234_5678] {
            let regions = LiquidRegions::from_heightmap(&fixture(vec![record(name, raw)])).unwrap();
            assert!(!regions.is_empty());
            assert_eq!(regions.at([5., 5., 0.]), Some(kind));
        }
    }
}

#[test]
fn runtime_dry_and_unsupported_winners_mask_swept_water() {
    for name in [
        "APK_dry",
        "ASL_area",
        "ATP_area",
        "AXX_unknown",
        "other",
        "WT_classic",
        "awt_lower",
        "AWT",
        "",
    ] {
        // Raw 5 is deliberately not enough to infer an unnamed/unknown liquid.
        let mut blocker = record(name, 5);
        blocker.full_size = [2., 4., 4.];
        let mut wet = record("AWT_pool", 0);
        wet.full_size = [8., 4., 4.];
        let mut records = vec![record("APV_generic", 5), blocker, wet];
        if name == "ATP_area" {
            // ATP's registration pass wins even though it is last in the DAT.
            records.swap(1, 2);
        }
        let regions = LiquidRegions::from_heightmap(&fixture(records)).unwrap();
        assert_eq!(regions.at([5., 5., 0.]), None, "{name}");
        assert_eq!(
            regions.at([4., 5., 0.]),
            None,
            "inclusive blocker face {name}"
        );
        assert_eq!(regions.at([2., 5., 0.]), Some(LiquidKind::Water));
        for (from, to) in [([0., 5., 0.], [10., 5., 0.]), ([10., 5., 0.], [0., 5., 0.])] {
            assert_eq!(
                regions.segment(from, to),
                vec![
                    LiquidSpan {
                        kind: LiquidKind::Water,
                        enter: 0.1,
                        exit: 0.4
                    },
                    LiquidSpan {
                        kind: LiquidKind::Water,
                        enter: 0.6,
                        exit: 0.9
                    },
                ],
                "{name}"
            );
        }
    }
}

#[test]
fn runtime_liquid_overlap_and_grid_order_match_point_queries() {
    let mut lava = record("ALV_pool", 0);
    lava.position[0] = 4.;
    lava.full_size = [4., 4., 4.];
    let mut water = record("AWT_pool", 0);
    water.position[0] = 6.;
    water.full_size = [6., 4., 4.];
    let regions = LiquidRegions::from_heightmap(&fixture(vec![lava, water])).unwrap();
    assert_eq!(regions.at([5., 5., 0.]), Some(LiquidKind::Lava));
    assert_eq!(regions.at([8., 5., 0.]), Some(LiquidKind::Water));
    assert_eq!(
        regions.segment([0., 5., 0.], [10., 5., 0.]),
        vec![
            LiquidSpan {
                kind: LiquidKind::Lava,
                enter: 0.2,
                exit: 0.6
            },
            LiquidSpan {
                kind: LiquidKind::Water,
                enter: 0.6,
                exit: 0.9
            },
        ]
    );
    let map = Heightmap::parse(
        options(),
        &dat(vec![
            ([1, 0], vec![record("AWT_first_on_disk", 0)]),
            ([0, 0], vec![record("AXX_first_in_grid", 0)]),
        ]),
    )
    .unwrap();
    let regions = LiquidRegions::from_heightmap(&map).unwrap();
    assert_eq!(regions.at([5., 5., 0.]), None);
    assert!(regions.segment([0., 5., 0.], [10., 5., 0.]).is_empty());
}

#[test]
fn runtime_quantized_rotation_keeps_nonsquare_bounds_and_swept_crossings() {
    let mut region = record("AVW_rotated", 0);
    region.rotation_degrees[2] = 35.6; // 50 units after native truncation.
    region.full_size = [4., 1., 2.];
    let regions = LiquidRegions::from_heightmap(&fixture(vec![region])).unwrap();
    let (sin, cos) = (50_f64 * std::f64::consts::TAU / 512.).sin_cos();
    let along = |x: f64| [(5. + x * cos) as f32, (5. + x * sin) as f32, 0.];
    assert_eq!(regions.at(along(1.8)), Some(LiquidKind::FreezingWater));
    assert_eq!(regions.at([5. - sin as f32, 5. + cos as f32, 0.]), None);
    let spans = regions.segment(along(-4.), along(4.));
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, LiquidKind::FreezingWater);
    assert!((spans[0].enter - 0.25).abs() < 1e-6);
    assert!((spans[0].exit - 0.75).abs() < 1e-6);
    assert!(regions.segment(along(0.), along(0.)).is_empty());
    assert_eq!(regions.at([f32::NAN; 3]), None);
    assert!(regions.segment([f32::INFINITY; 3], [0.; 3]).is_empty());
}

#[test]
fn runtime_supported_set_rejects_any_unsupported_record_or_group() {
    let mut map = fixture(vec![record("AWT_pool", 0), record("AXX_unknown", 0)]);
    map.regions[1].rotation_degrees[0] = 15.;
    assert!(LiquidRegions::from_heightmap(&map).is_err());
    map.regions[1].rotation_degrees[0] = 0.;
    map.groups.push(TerrainPlacement {
        model: "unresolved".into(),
        transform: glam::Mat4::IDENTITY,
    });
    assert!(LiquidRegions::from_heightmap(&map).is_err());
}

#[test]
#[ignore = "requires original Maiden's Grave assets"]
fn runtime_maidensgrave_uses_authored_center_faces_and_finite_depth() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    let regions = LiquidRegions::load(&base, "maidensgrave").unwrap();
    assert!(!regions.is_empty());
    let center = [3011.3552, 4048.7725, -199.96002];
    let half = [3500., 3000., 500.];
    assert_eq!(regions.at(center), Some(LiquidKind::Water));
    for axis in 0..3 {
        for sign in [-1., 1.] {
            let mut point = center;
            point[axis] += sign * half[axis];
            // Public query coordinates are f32: vertical face sums can round
            // outwards. One representable point inside still must be wet.
            point[axis] -= sign * 0.001;
            assert_eq!(regions.at(point), Some(LiquidKind::Water), "{axis} {sign}");
            point[axis] += sign * 0.101;
            assert_eq!(regions.at(point), None, "{axis} {sign}");
        }
    }
    let from = [center[0], center[1], center[2] - 1000.];
    let to = [center[0], center[1], center[2] + 1000.];
    assert_eq!(regions.at(from), None);
    assert_eq!(regions.at(to), None);
    let spans = regions.segment(from, to);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, LiquidKind::Water);
    assert!((spans[0].enter - 0.25).abs() < 1e-6);
    assert!((spans[0].exit - 0.75).abs() < 1e-6);
}

#[path = "support/liquid_heightmap_loader.rs"]
mod runtime_loader;
