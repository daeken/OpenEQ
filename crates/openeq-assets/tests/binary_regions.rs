use openeq_assets::{
    binary_regions::BinaryRegions,
    liquid_regions::{LiquidKind, LiquidRegions, LiquidSpan},
    pfs::Archive,
};

#[derive(Clone)]
struct Raw {
    name: &'static str,
    center: [f32; 3],
    orientation: [f32; 3],
    half: [f32; 3],
}

fn raw(name: &'static str) -> Raw {
    Raw {
        name,
        center: [0.; 3],
        orientation: [0.; 3],
        half: [10.; 3],
    }
}

fn word(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

/// Includes model names, two objects, differing v2 lighting-array lengths and
/// one trailing light. Region decoding must neither resolve nor parse meshes.
fn zon(version: u32, records: &[Raw]) -> Vec<u8> {
    let mut strings = b"unresolved_model.mod\0object\0".to_vec();
    let mut names = Vec::new();
    for record in records {
        names.push(strings.len() as u32);
        strings.extend(record.name.as_bytes());
        strings.push(0);
    }
    let mut bytes = b"EQGZ".to_vec();
    for value in [version, strings.len() as u32, 1, 2, records.len() as u32, 1] {
        word(&mut bytes, value);
    }
    bytes.extend(strings);
    word(&mut bytes, 0);
    for lighting in [&[17, 42, 0][..], &[][..]] {
        bytes.extend([0; 36]);
        if version == 2 {
            word(&mut bytes, lighting.len() as u32);
            for &value in lighting {
                word(&mut bytes, value);
            }
        }
    }
    for (name, record) in names.into_iter().zip(records) {
        word(&mut bytes, name);
        for value in record
            .center
            .into_iter()
            .chain(record.orientation)
            .chain(record.half)
        {
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes.extend([0; 32]);
    bytes
}

#[test]
fn both_versions_retain_signed_metadata_and_source_order_without_meshes() {
    let mut first = raw("AWT_Mixed_Case");
    first.center = [3., -4., 5.];
    first.orientation = [31.9, -73.8, 19.2];
    first.half = [-2., 7., 4.];
    for version in [1, 2] {
        let bytes = zon(version, &[first.clone(), raw("ATP_later")]);
        let set = BinaryRegions::parse(&bytes).unwrap();
        assert_eq!(set.version, version);
        assert_eq!(set.boxes.len(), 2);
        let volume = &set.boxes[0];
        assert_eq!(volume.name, first.name);
        assert_eq!(volume.center, first.center);
        assert_eq!(volume.orientation, first.orientation);
        assert_eq!(volume.half_extents, first.half);
        assert_eq!(volume.angle_units, [31, -73, 19]);
        assert_eq!(volume.raw_type, 0);
        assert_eq!(volume.effective_type, Some(5));
        assert_eq!(volume.record_index, 0);
        assert_eq!(set.boxes[1].record_index, 1);
        assert_eq!(set.boxes[1].source_offset, volume.source_offset + 40);
        assert_eq!(
            &bytes[volume.source_offset + 4..volume.source_offset + 8],
            &3_f32.to_le_bytes()
        );
        assert_eq!(
            set.at(first.center.map(f64::from), None).unwrap().name,
            "AWT_Mixed_Case"
        );
    }
    assert!(BinaryRegions::parse(&zon(2, &[])).unwrap().boxes.is_empty());
}

#[test]
fn native_first_match_preserves_dry_unknown_and_apv_precedence() {
    let data = zon(
        2,
        &[
            raw("APV_view"),
            raw("AXX_dry"),
            raw("AWT_water"),
            raw("ALV_lava"),
        ],
    );
    let set = BinaryRegions::parse(&data).unwrap();
    assert_eq!(set.at([0.; 3], None).unwrap().name, "AXX_dry");
    assert_eq!(set.at([0.; 3], Some(*b"AWT")).unwrap().name, "AWT_water");
    assert_eq!(set.at([0.; 3], Some(*b"APV")).unwrap().name, "APV_view");
    assert_eq!(set.at([0.; 3], Some(*b"XYZ")).unwrap().name, "AXX_dry");
    assert_eq!(set.at([0.; 3], Some(*b"awt")).unwrap().name, "AXX_dry");
    assert!(set.at([100.; 3], None).is_none());
    let unknown =
        BinaryRegions::parse(&zon(1, &[raw("legacy_unknown"), raw("AWT_water")])).unwrap();
    assert_eq!(unknown.at([0.; 3], None).unwrap().effective_type, None);
    let water_before_teleport =
        BinaryRegions::parse(&zon(1, &[raw("AWT_first"), raw("ATP_later")])).unwrap();
    assert_eq!(
        water_before_teleport.at([0.; 3], None).unwrap().name,
        "AWT_first"
    );
}

#[test]
fn malformed_headers_arrays_names_and_all_truncations_fail() {
    for version in [1, 2] {
        let bytes = zon(version, &[raw("AWT_water")]);
        for end in 0..bytes.len() {
            assert!(
                BinaryRegions::parse(&bytes[..end]).is_err(),
                "version {version}, length {end}"
            );
        }
        for offset in [8, 12, 16, 20, 24] {
            let mut bad = bytes.clone();
            bad[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
            assert!(BinaryRegions::parse(&bad).is_err());
        }
        let start = BinaryRegions::parse(&bytes).unwrap().boxes[0].source_offset;
        let size = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut bad = bytes.clone();
        bad[start..start + 4].copy_from_slice(&(size as u32).to_le_bytes());
        assert!(BinaryRegions::parse(&bad).is_err());
        let mut bad = bytes.clone();
        bad[28 + size - 1] = b'x';
        assert!(BinaryRegions::parse(&bad).is_err());
        let mut bad = bytes.clone();
        bad[28 + size - "AWT_water".len() - 1] = 255;
        assert!(BinaryRegions::parse(&bad).is_err());
        let mut bad = bytes;
        bad.push(0);
        assert!(BinaryRegions::parse(&bad).is_err());
    }
    let mut bad_lights = zon(2, &[raw("AWT_water")]);
    let size = u32::from_le_bytes(bad_lights[8..12].try_into().unwrap()) as usize;
    let offset = 28 + size + 4 + 36;
    bad_lights[offset..offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(BinaryRegions::parse(&bad_lights).is_err());
    assert!(BinaryRegions::parse(&zon(3, &[])).is_err());
    let mut bad_magic = zon(1, &[]);
    bad_magic[0] = b'x';
    assert!(BinaryRegions::parse(&bad_magic).is_err());
}

#[test]
fn unsupported_record_anywhere_rejects_the_complete_set() {
    for variant in 0..10 {
        let mut bad = raw("AXX_dry");
        match variant {
            0 => {
                bad.name = "AFG_singular";
                bad.half = [0., -2., 4.];
            }
            1 => bad.center[0] = f32::NAN,
            2 => bad.orientation[1] = f32::INFINITY,
            3 => bad.orientation[2] = f32::MAX,
            4 => bad.orientation[1] = -2_147_483_648.,
            5 => bad.half[0] = 0.,
            6 => bad.half[1] = -0.,
            7 => bad.half[2] = f32::from_bits(1),
            8 => bad.half[0] = f32::INFINITY,
            _ => bad.half = [f32::MAX; 3],
        }
        for records in [[bad.clone(), raw("AWT_water")], [raw("AWT_water"), bad]] {
            let bytes = zon(2, &records);
            assert!(BinaryRegions::parse(&bytes).is_err(), "variant {variant}");
            assert!(
                LiquidRegions::from_eqgz(&bytes).is_err(),
                "variant {variant}"
            );
        }
    }
}

#[test]
fn afg_signed_extents_are_equalized_before_cardinal_rotation() {
    for (half, registered) in [
        ([2., 7., 4.], [7., 7., 4.]),
        ([7., 2., 4.], [7., 7., 4.]),
        ([2., -7., -4.], [2., 2., -4.]),
        ([-7., 2., 4.], [2., 2., 4.]),
        ([-2., -7., -4.], [-2., -2., -4.]),
        ([-7., -2., 4.], [-2., -2., 4.]),
        ([0., 2., 4.], [2., 2., 4.]),
    ] {
        for (orientation, axes) in [
            ([0., 0., 0.], [0, 1, 2]),
            ([128., 0., 0.], [1, 0, 2]),
            ([0., 128., 0.], [2, 1, 0]),
            ([0., 0., -128.], [0, 2, 1]),
        ] {
            let mut record = raw("AFG_fog");
            record.center = [10., -20., 30.];
            record.half = half;
            record.orientation = orientation;
            let set = BinaryRegions::parse(&zon(2, &[record.clone()])).unwrap();
            let volume = &set.boxes[0];
            assert_eq!(volume.half_extents, half, "raw signed metadata");
            assert_eq!(volume.registered_half_extents, registered);
            assert_eq!(volume.orientation, orientation);
            assert_eq!(volume.effective_type, Some(0));
            for axis in 0..3 {
                for sign in [-1., 1.] {
                    let mut point = record.center.map(f64::from);
                    point[axis] += sign * f64::from(registered[axes[axis]].abs()) * 0.99;
                    assert!(volume.contains(point), "{half:?} {orientation:?} {point:?}");
                    point[axis] += sign * f64::from(registered[axes[axis]].abs()) * 0.02;
                    assert!(
                        !volume.contains(point),
                        "{half:?} {orientation:?} {point:?}"
                    );
                }
            }
        }
    }
    // Name comparison remains case-sensitive and uses exactly three bytes.
    for name in ["AFG", "AFG_fog", "Afg_fog", "afg_fog"] {
        let mut record = raw(name);
        record.half = [2., 7., 4.];
        let set = BinaryRegions::parse(&zon(1, &[record])).unwrap();
        assert_eq!(
            set.boxes[0].registered_half_extents,
            if name.starts_with("AFG") {
                [7., 7., 4.]
            } else {
                [2., 7., 4.]
            }
        );
    }
}

#[test]
fn afg_general_xyz_matches_independent_native_replay() {
    // Native 0x10021e3a..0x10021e65 signed comparison, followed by independent
    // instruction replay of 0x100c2889..0x100c2906. Neither a world-space square
    // nor max(abs(X),abs(Y)) contains these same boundary probes.
    for half in [[2., -7., 4.], [-2., -7., -4.]] {
        let mut record = raw("AFG_xyz");
        record.center = [10., -20., 30.];
        record.orientation = [31.9, -73.8, 19.2];
        record.half = half;
        let set = BinaryRegions::parse(&zon(1, &[record])).unwrap();
        let volume = &set.boxes[0];
        assert_eq!(volume.angle_units, [31, -73, 19]);
        for (point, expected) in [
            (
                [10.04443270802498, -18.84532758653164, 27.840102418661118],
                true,
            ),
            (
                [10.067640141248702, -18.8360467427969, 27.80887292981148],
                false,
            ),
            (
                [10.16484748840332, -17.95886244237423, 30.216755568683148],
                true,
            ),
            (
                [10.157096652984619, -17.920047854781153, 30.222530722916126],
                false,
            ),
        ] {
            assert_eq!(volume.contains(point), expected, "{half:?} {point:?}");
        }
        // The replay's registered local Y basis, used to cross both faces.
        let y = [-0.3875417709350586, 1.9407293796539307, 0.28875771164894104];
        let center = volume.center.map(f64::from);
        let from = std::array::from_fn(|i| center[i] - 2. * y[i]);
        let to = std::array::from_fn(|i| center[i] + 2. * y[i]);
        let [enter, exit] = volume.segment(from, to).unwrap();
        assert!((enter - 0.25).abs() < 1e-10 && (exit - 0.75).abs() < 1e-10);
    }
}

#[test]
fn afg_post_normalization_singular_regions_reject_the_complete_set() {
    for half in [
        [0., -2., 4.],
        [-2., -0., 4.],
        [f32::from_bits(1), -2., 4.],
        [2., 7., 0.],
        [2., 7., -0.],
    ] {
        let mut bad = raw("AFG_singular");
        bad.half = half;
        for records in [[bad.clone(), raw("AWT_pool")], [raw("AWT_pool"), bad]] {
            let data = zon(2, &records);
            assert!(BinaryRegions::parse(&data).is_err());
            assert!(LiquidRegions::from_eqgz(&data).is_err());
        }
    }
}

#[test]
fn afg_dry_regions_preserve_name_duplicates_and_liquid_precedence() {
    let mut first = raw("AFG_same_name");
    first.half = [3., 1., 2.];
    let mut second = first.clone();
    second.center = [8., 0., 0.];
    second.half = [1., 2., 2.];
    let bytes = zon(2, &[first.clone(), second, raw("AWT_pool")]);
    let set = BinaryRegions::parse(&bytes).unwrap();
    assert_eq!(set.boxes.len(), 3);
    assert_eq!(set.at([0., 2., 0.], None).unwrap().record_index, 0);
    assert_eq!(set.at([8., 0., 0.], None).unwrap().record_index, 1);
    assert_eq!(set.at([0., 2., 0.], Some(*b"AWT")).unwrap().record_index, 2);
    let regions = LiquidRegions::from_eqgz(&bytes).unwrap();
    assert_eq!(regions.at([0., 2., 0.]), None);
    assert_eq!(regions.at([8., 0., 0.]), None);
    assert_eq!(regions.at([5., 2., 0.]), Some(LiquidKind::Water));
    assert_eq!(
        regions.segment([0., -10., 0.], [0., 10., 0.]),
        vec![
            LiquidSpan {
                kind: LiquidKind::Water,
                enter: 0.,
                exit: 0.35
            },
            LiquidSpan {
                kind: LiquidKind::Water,
                enter: 0.65,
                exit: 1.
            },
        ]
    );
    let reversed = LiquidRegions::from_eqgz(&zon(1, &[raw("AWT_pool"), first])).unwrap();
    assert_eq!(reversed.at([0., 2., 0.]), Some(LiquidKind::Water));
}

#[test]
fn signed_cardinal_faces_and_segment_crossings_are_finite() {
    let mut record = raw("AWT_cardinal");
    record.center = [10., -20., 30.];
    record.orientation = [128., 0., 0.];
    record.half = [2., -8., 4.];
    let set = BinaryRegions::parse(&zon(1, &[record])).unwrap();
    let volume = &set.boxes[0];
    for axis in 0..3 {
        for sign in [-1., 1.] {
            let mut p = [10., -20., 30.];
            p[axis] += sign * [8., 2., 4.][axis];
            assert!(volume.contains(p));
            p[axis] += sign * 0.001;
            assert!(!volume.contains(p));
        }
    }
    assert_eq!(
        volume.segment([-6., -20., 30.], [26., -20., 30.]),
        Some([0.25, 0.75])
    );
    assert_eq!(
        volume.segment([26., -20., 30.], [-6., -20., 30.]),
        Some([0.25, 0.75])
    );
    assert!(volume.segment([0., 0., 0.], [0., 0., 0.]).is_none());
    assert!(volume.segment([f64::MAX; 3], [-f64::MAX; 3]).is_none());
    assert!(!volume.contains([f64::NAN; 3]));
}

#[test]
fn xyz_signed_transform_matches_independent_native_instruction_replay() {
    let mut record = raw("AWT_xyz");
    record.center = [10., -20., 30.];
    record.orientation = [31.9, -73.8, 19.2];
    record.half = [2., -7., 4.];
    let set = BinaryRegions::parse(&zon(1, &[record.clone()])).unwrap();
    let volume = &set.boxes[0];
    // Replayed x87 operations 0x100c2889..0x100c2906, with float32
    // stores and signed scaling, independently of this Rust implementation.
    for (point, expected) in [
        (
            [10.742007895708085, -22.33864051759243, 27.320338555574416],
            true,
        ),
        (
            [10.76521532893181, -22.329359673857688, 27.289109066724777],
            false,
        ),
        (
            [11.891346077919007, -26.604811946749688, 28.930340007543563],
            true,
        ),
        (
            [11.91847400188446, -26.74066300570965, 28.910126968622208],
            false,
        ),
    ] {
        assert_eq!(volume.contains(point), expected, "{point:?}");
    }
    record.orientation = [31.01 + 512., -73.01 - 512., 19.99];
    let wrapped = BinaryRegions::parse(&zon(2, &[record])).unwrap();
    assert_eq!(wrapped.boxes[0].angle_units, [543, -585, 19]);
    assert!(wrapped.boxes[0].contains([
        10.742007895708085,
        -22.33864051759243,
        27.320338555574416
    ]));
    assert!(!wrapped.boxes[0].contains([
        10.76521532893181,
        -22.329359673857688,
        27.289109066724777
    ]));
}

#[test]
fn liquid_point_and_swept_queries_keep_dry_gaps_and_file_precedence() {
    for dry_name in ["AXX_dry", "ATP_dry", "unknown"] {
        let mut dry = raw(dry_name);
        dry.half = [2.; 3];
        let regions = LiquidRegions::from_eqgz(&zon(2, &[dry.clone(), raw("AWT_water")])).unwrap();
        assert_eq!(regions.at([0.; 3]), None);
        assert_eq!(regions.at([5., 0., 0.]), Some(LiquidKind::Water));
        assert_eq!(
            regions.segment([-20., 0., 0.], [20., 0., 0.]),
            vec![
                LiquidSpan {
                    kind: LiquidKind::Water,
                    enter: 0.25,
                    exit: 0.45
                },
                LiquidSpan {
                    kind: LiquidKind::Water,
                    enter: 0.55,
                    exit: 0.75
                },
            ]
        );
        let reversed = LiquidRegions::from_eqgz(&zon(1, &[raw("AWT_water"), dry])).unwrap();
        assert_eq!(reversed.at([0.; 3]), Some(LiquidKind::Water));
        assert_eq!(
            reversed.segment([-20., 0., 0.], [20., 0., 0.]),
            vec![LiquidSpan {
                kind: LiquidKind::Water,
                enter: 0.25,
                exit: 0.75
            },]
        );
    }
    let apv = LiquidRegions::from_eqgz(&zon(2, &[raw("APV_view"), raw("ALV_lava")])).unwrap();
    assert_eq!(apv.at([0.; 3]), Some(LiquidKind::Lava));
    assert_eq!(
        apv.segment([-20., 0., 0.], [20., 0., 0.])[0].kind,
        LiquidKind::Lava
    );
}

fn f32s(values: [f64; 3]) -> [f32; 3] {
    values.map(|value| value as f32)
}

fn anguish_record() -> Raw {
    Raw {
        name: "AWT_water",
        center: f32s([700.9058837890625, 2.6773910522460938, -256.8711242675781]),
        orientation: [-std::f32::consts::FRAC_PI_2, -0., 0.],
        half: f32s([125.73063659667969, -123.33009338378906, 11.755911827087402]),
    }
}

fn crescent_record() -> Raw {
    Raw {
        name: "AWT_river30",
        center: f32s([-1091.171875, -2348.66162109375, -204.40826416015625]),
        orientation: [-std::f32::consts::FRAC_PI_2, 0., 0.],
        half: f32s([6.1612548828125, -117.21435546875, 35.02192687988281]),
    }
}

#[test]
fn original_numeric_fixtures_use_native_units_and_signed_half_extents() {
    // Original values and expected points come from static native instruction
    // replay; no proprietary data is needed for these portable fixtures.
    for (version, record, points) in [
        (
            1,
            anguish_record(),
            [
                [824.7644606113433, -48.17841153979302, -260.39789781570437],
                [827.2788839817047, -48.209269706010815, -260.39789781570437],
                [724.5518019664288, -119.71879093647003, -255.6955330848694],
                [724.5215329658985, -122.18520710468293, -255.6955330848694],
            ],
        ),
        (
            2,
            crescent_record(),
            [
                [-1085.6480521917342, -2395.6186843913792, -214.9148422241211],
                [-1085.5248363733292, -2395.6201965528726, -214.9148422241211],
                [-1091.363733317852, -2464.7102156853675, -200.90607147216798],
                [-1091.3925013279916, -2467.054326250553, -200.90607147216798],
            ],
        ),
    ] {
        let set = BinaryRegions::parse(&zon(version, &[record])).unwrap();
        let volume = &set.boxes[0];
        assert_eq!(volume.angle_units, [-1, 0, 0]);
        for (point, expected) in points.into_iter().zip([true, false, true, false]) {
            assert_eq!(volume.contains(point), expected, "{version}: {point:?}");
        }
    }
    let set = BinaryRegions::parse(&zon(2, &[crescent_record()])).unwrap();
    let c = set.boxes[0].center.map(f64::from);
    assert!(set.boxes[0].contains([c[0], c[1] + 100., c[2]]));
    assert!(!set.boxes[0].contains([c[0] + 100., c[1], c[2]]));
}

#[test]
#[ignore = "requires original Anguish and Crescent EQG/ZON files; CPU only"]
fn original_binary_files_and_runtime_load_match_finite_native_regions() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    for (zone, count, index, offset, expected) in [
        ("anguish", 2, 1, 48057, anguish_record()),
        ("crescent", 58, 31, 2726476, crescent_record()),
    ] {
        let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        let name = format!("{zone}.zon");
        let bytes = if archive.contains(&name) {
            archive.read(&name).unwrap()
        } else {
            std::fs::read(base.join(&name)).unwrap()
        };
        let set = BinaryRegions::parse(&bytes).unwrap();
        assert_eq!(set.boxes.len(), count);
        let volume = &set.boxes[index];
        assert_eq!(volume.source_offset, offset);
        assert_eq!(volume.name, expected.name);
        assert_eq!(volume.center, expected.center);
        assert_eq!(volume.orientation, expected.orientation);
        assert_eq!(volume.half_extents, expected.half);
        assert_eq!(volume.angle_units, [-1, 0, 0]);
        let regions = LiquidRegions::load(&base, zone).unwrap();
        assert!(!regions.is_empty());
        // Crescent's isolated northern region avoids neighboring river boxes
        // when checking dry faces through the complete runtime query.
        let probe = if zone == "crescent" {
            &set.boxes[0]
        } else {
            volume
        };
        let center = probe.center;
        assert_eq!(regions.at(center), Some(LiquidKind::Water));
        for sign in [-1., 1.] {
            let mut inside = center;
            inside[2] += sign * (probe.half_extents[2] - 0.1);
            let mut outside = center;
            outside[2] += sign * (probe.half_extents[2] + 0.1);
            assert_eq!(
                regions.at(inside),
                Some(LiquidKind::Water),
                "{zone} vertical {sign}"
            );
            assert_eq!(regions.at(outside), None, "{zone} vertical {sign}");
        }
        // Precomputed float32 cosine/sine for native table index 511.
        let cos = 0.9999247012138367;
        let sin = -0.012271538376808167;
        let half = f64::from(probe.half_extents[0]);
        for (distance, wet) in [(half - 0.1, true), (half + 0.1, false)] {
            let point = f32s([
                f64::from(center[0]) + cos * distance,
                f64::from(center[1]) + sin * distance,
                f64::from(center[2]),
            ]);
            assert_eq!(
                regions.at(point),
                wet.then_some(LiquidKind::Water),
                "{zone} side"
            );
        }
        let from = [center[0], center[1], center[2] - 2. * probe.half_extents[2]];
        let to = [center[0], center[1], center[2] + 2. * probe.half_extents[2]];
        let spans = regions.segment(from, to);
        assert_eq!(spans.len(), 1, "{zone}");
        assert_eq!(spans[0].kind, LiquidKind::Water);
        assert!((spans[0].enter - 0.25).abs() < 0.00001);
        assert!((spans[0].exit - 0.75).abs() < 0.00001);
    }
}

#[test]
#[ignore = "requires original Pohealth EQG/ZON files; CPU only"]
fn original_pohealth_afg_regions_parse_and_remain_dry() {
    let base = openeq_assets::loader::default_client_dir().expect("original client assets");
    let archive = Archive::open(base.join("pohealth.eqg")).unwrap();
    let bytes = if archive.contains("pohealth.zon") {
        archive.read("pohealth.zon").unwrap()
    } else {
        std::fs::read(base.join("pohealth.zon")).unwrap()
    };
    let set = BinaryRegions::parse(&bytes).unwrap();
    assert_eq!(set.version, 2);
    assert_eq!(set.boxes.len(), 19);
    assert_eq!(
        set.boxes
            .iter()
            .filter(|r| r.name.starts_with("AFG"))
            .count(),
        16
    );
    let volume = &set.boxes[3];
    assert_eq!(volume.name, "AFG_10");
    assert_eq!(volume.source_offset, 5_864_886);
    assert_eq!(
        volume.center,
        f32s([1495.302978515625, 2190.63232421875, 25.553619384765625])
    );
    assert_eq!(
        volume.half_extents,
        f32s([832.6297607421875, 487.094482421875, 171.72036743164062])
    );
    assert_eq!(
        volume.registered_half_extents,
        f32s([832.6297607421875, 832.6297607421875, 171.72036743164062])
    );
    assert_eq!(volume.angle_units, [-128, 0, 0]);
    // Native yaw maps local Y to world X. The authored shorter Y extent would
    // omit this point if the pre-rotation AFG expansion were not applied.
    let center = volume.center.map(f64::from);
    let expanded = [center[0] + 600., center[1], center[2]];
    assert!(volume.contains(expanded));
    assert_eq!(set.at(expanded, None).unwrap().record_index, 3);
    assert!(!volume.contains([center[0] + 833., center[1], center[2]]));
    assert!(!volume.contains([center[0], center[1], center[2] + 172.]));
    // Supported dry-only metadata stays empty in the public liquid API; it
    // does not invent a water volume merely because parsing now succeeds.
    assert!(LiquidRegions::from_eqgz(&bytes).unwrap().is_empty());
    assert!(LiquidRegions::load(&base, "pohealth").unwrap().is_empty());
    let metadata = openeq_assets::audit::metadata(&base, "pohealth").unwrap();
    assert_eq!(metadata.authored_regions, Some(19));
    assert_eq!(metadata.liquid_status, "no_supported_volumes");
    assert!(metadata.liquid_detail.is_none());
}
