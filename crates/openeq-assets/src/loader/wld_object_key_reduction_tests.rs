use super::*;
use key_reduction::FiveFrameKeys;

// Compact native-math witnesses, not copied archive payloads. Provenance:
// docs/WLD_LONG_OBJECT_ANIMATION.md and its frozen 250-sample result.
const BRANCHES: [([[i16; 4]; 5], usize, [[u32; 4]; 4]); 6] = [
    (
        // TR5MBR1_DAG
        [
            [-11399, 1619, 1639, -11540],
            [-11399, 1619, 1639, -11540],
            [-11418, 1479, 1498, -11559],
            [-11399, 1622, 1216, -11592],
            [-11388, 1692, 1287, -11584],
        ],
        1,
        [
            [0xbdc1a2de, 0xbdc412e7, 0x3f3478ac, 0x3f3244a3],
            [0xbdb8e203, 0xbdbb4209, 0x3f349df7, 0x3f3269f1],
            [0xbdcac161, 0xbd980109, 0x3f35213b, 0x3f321d36],
            [0xbdcef631, 0xbdb6e578, 0x3f34ad67, 0x3f320b53],
        ],
    ),
    (
        // TR5MBR2_DAG
        [
            [16382, 99, 102, 200],
            [16382, 99, 102, 200],
            [16381, 98, 202, 200],
            [16382, -2, 201, 201],
            [16381, -103, 200, 202],
        ],
        3,
        [
            [0xbbc60081, 0xbbcc0085, 0xbc480082, 0xbf7ff8a6],
            [0xbbc400bd, 0xbc4a00c3, 0xbc4800c1, 0xbf7ff4f6],
            [0x39200156, 0xbc4901ad, 0xbc4901ad, 0xbf7ff622],
            [0x39000124, 0xbc170159, 0xbc4901cb, 0xbf7ff848],
        ],
    ),
    (
        // TR5MBR6_DAG
        [
            [-11508, -1829, -1852, 11368],
            [-11508, -1829, -1852, 11368],
            [-11368, -1852, -1829, 11508],
            [-11438, -1840, -1840, 11438],
            [-11578, -1818, -1863, 11297],
        ],
        1,
        [
            [0x3de61089, 0x3de61089, 0xbf32b86a, 0x3f32b86a],
            [0x3de77f6e, 0x3de49f6f, 0xbf33cf8e, 0x3f319f90],
            [0x3de600f1, 0x3de600f1, 0xbf32b8bb, 0x3f32b8bb],
            [0x3de3ef4e, 0x3de82f4b, 0xbf311176, 0x3f345b73],
        ],
    ),
    (
        // TR5MBR5_DAG
        [
            [9728, -4355, 11447, -4875],
            [9728, -4355, 11447, -4875],
            [9788, -4496, 11393, -4755],
            [9845, -4635, 11337, -4635],
            [9817, -4565, 11365, -4695],
        ],
        2,
        [
            [0x3e881a56, 0xbf32df12, 0x3e985a9e, 0xbf18029c],
            [0x3e8c7c80, 0xbf3205b3, 0x3e949cc2, 0xbf18eee6],
            [0x3e90d934, 0xbf312578, 0x3e90d934, 0xbf19d547],
            [0x3e8b6342, 0xbf323c2b, 0x3e958b7f, 0xbf18b592],
        ],
    ),
    (
        // TR5MBR4_DAG
        [
            [10462, -10699, 5788, 3319],
            [10462, -10699, 5788, 3319],
            [10502, -10627, 5919, 3190],
            [10521, -10590, 5984, 3125],
            [10502, -10627, 5919, 3190],
        ],
        2,
        [
            [0x3f272bbd, 0xbeb4dfb7, 0xbe4f6fad, 0xbf2377be],
            [0x3f26548e, 0xbeb7f2d4, 0xbe496318, 0xbf23f085],
            [0x3f2579bd, 0xbebb01f7, 0xbe43520d, 0xbf2465ba],
            [0x3f269caa, 0xbeb6ecba, 0xbe4b68cf, 0xbf23c8a7],
        ],
    ),
    (
        // TR5MBR3_DAG
        [
            [16384, 0, 0, 0],
            [16384, 0, 0, 0],
            [16383, 0, -101, 0],
            [16382, 101, -201, 1],
            [16383, 101, -101, 1],
        ],
        1,
        [
            [0x0, 0x3b4a0155, 0x0, 0xbf7fffb1],
            [0x0, 0x3bca022c, 0x0, 0xbf7ffec1],
            [0xbbca0170, 0x3c49016e, 0xb88000e9, 0xbf7ff9d2],
            [0xbb4a0116, 0x3b4a0116, 0xb80000b0, 0xbf7fff61],
        ],
    ),
];

fn five_frame_source(branch: usize) -> ObjectSource {
    let mut source = short_animation_source(false);
    let skeleton = source.skeleton.as_mut().unwrap();
    skeleton.tracks[0].definition.frames[0] = frame([0.; 3], 1.);
    skeleton.tracks[1].definition.frames = BRANCHES[branch]
        .0
        .iter()
        .map(|&[w, x, y, z]| Frame {
            rotation: [x, y, z, w].map(|value| f32::from(value) / 16384.),
            ..frame([0.; 3], 1.)
        })
        .collect();
    source
}

#[test]
fn five_frame_native_reduction_preserves_key_times_and_selects_earliest_structural_tie() {
    for (branch, (_, omitted, samples)) in BRANCHES.iter().enumerate() {
        let source = five_frame_source(branch);
        let frames = &source.skeleton.as_ref().unwrap().tracks[1]
            .definition
            .frames;
        assert_eq!(FiveFrameKeys::new(frames).unwrap().omitted, *omitted);
        assert_eq!(source.animation_period().unwrap(), Duration::from_secs(5));
        for (&milliseconds, quaternion_bits) in [1000, 2000, 3000, 4500].iter().zip(samples) {
            let native = Mat4::from_quat(Quat::from_array(quaternion_bits.map(f32::from_bits)));
            let sampled = source
                .sample_animation(Duration::from_millis(milliseconds))
                .unwrap();
            for (original, actual) in source.parts[0]
                .mesh
                .vertices
                .iter()
                .zip(&sampled[0].vertices)
            {
                near(
                    *actual,
                    native
                        .transform_point3(Vec3::from_array(*original))
                        .to_array(),
                );
            }
        }
        let first = source.sample_animation(Duration::ZERO).unwrap();
        let closed = source.sample_animation(Duration::from_secs(5)).unwrap();
        assert_eq!(first[0].vertices, closed[0].vertices);
        assert_eq!(
            first[0].vertices,
            posed_meshes(&source).unwrap()[0].vertices
        );
        // Alternate authored quaternion signs are equivalent, including the
        // pair newly adjacent after reduction and the last-to-first segment.
        let mut flipped = source.clone();
        for (index, frame) in flipped.skeleton.as_mut().unwrap().tracks[1]
            .definition
            .frames
            .iter_mut()
            .enumerate()
        {
            if index % 2 == 1 {
                frame.rotation = frame.rotation.map(|value| -value);
            }
        }
        for milliseconds in [0, 250, 1250, 2750, 4750, 5000] {
            let elapsed = Duration::from_millis(milliseconds);
            let a = source.sample_animation(elapsed).unwrap();
            let b = flipped.sample_animation(elapsed).unwrap();
            for (a, b) in a[0].vertices.iter().zip(&b[0].vertices) {
                near(*a, *b);
            }
        }
    }
}

#[test]
fn five_frame_reduction_rejects_cpu_dependent_near_ties_and_unproven_inputs() {
    let mut source = five_frame_source(0);
    let first = source.skeleton.as_ref().unwrap().tracks[1]
        .definition
        .frames[0];
    // Independently replayed native counterexample: PC64 removes index3;
    // PC24 ties indices2/3 and removes index2. Every component is packed and
    // near-unit, so flags and magnitude checks alone are insufficient.
    source.skeleton.as_mut().unwrap().tracks[1]
        .definition
        .frames = [
        [20, -16, 4, 16383],
        [21, -14, 5, 16383],
        [21, -15, 5, 16383],
        [21, -16, 5, 16383],
        [20, -15, 5, 16383],
    ]
    .into_iter()
    .map(|rotation| Frame {
        rotation: rotation.map(|value| value as f32 / 16384.),
        ..first
    })
    .collect();
    assert!(
        source
            .animation_period()
            .unwrap_err()
            .to_string()
            .contains("numerically ambiguous")
    );
    assert!(source.sample_animation(Duration::ZERO).is_err());
    assert!(posed_meshes(&source).is_ok());

    let mut invalid_sources = Vec::new();
    for mutation in 0..7 {
        let mut bad = five_frame_source(0);
        let skeleton = bad.skeleton.as_mut().unwrap();
        match mutation {
            0 => skeleton.tracks[1].definition.frames[2].rotation[0] += 1. / 32768.,
            1 => skeleton.tracks[1].definition.frames[2].rotation = [0., 0., 0., 0.5],
            2 => skeleton.tracks[1].definition.frames[2].scale *= 2.,
            3 => skeleton.tracks[1].definition.frames[2].translation[0] += 1.,
            4 => {
                let extra = skeleton.tracks[1].definition.frames[0];
                skeleton.tracks[1].definition.frames.push(extra);
            }
            5 => skeleton.tracks[0].reference_flags = 0,
            6 => {
                let root = skeleton.tracks[0].definition.frames[0];
                skeleton.tracks[0].definition.frames = vec![root; 4];
                skeleton.tracks[0].reference_flags = 5;
                skeleton.tracks[0].speed = Some(1000);
            }
            _ => unreachable!(),
        }
        invalid_sources.push(bad);
    }
    for bad in invalid_sources {
        assert!(bad.animation_period().is_err());
        assert!(bad.sample_animation(Duration::ZERO).is_err());
        assert!(posed_meshes(&bad).is_ok());
    }
}

#[test]
fn five_frame_reduction_rejects_newly_adjacent_half_turns() {
    let mut source = five_frame_source(0);
    let first = source.skeleton.as_ref().unwrap().tracks[1]
        .definition
        .frames[0];
    source.skeleton.as_mut().unwrap().tracks[1]
        .definition
        .frames = [
        [0, 0, 0, 16384],
        [0, 0, 11585, 11585],
        [0, 0, 16384, 0],
        [0, 0, 11585, 11585],
        [0, 0, 0, 16384],
    ]
    .into_iter()
    .map(|rotation| Frame {
        rotation: rotation.map(|value| value as f32 / 16384.),
        ..first
    })
    .collect();
    assert!(
        source
            .animation_period()
            .unwrap_err()
            .to_string()
            .contains("ambiguous reduced")
    );
}

#[test]
fn five_frame_animation_does_not_relax_stationary_collision_policy() {
    let mut source = five_frame_source(0);
    assert!(source.animation_period().is_ok());
    assert!(source.stationary_collision_animation_radius().is_err());
    // Removing collidable faces admits the same motion; no animation test may
    // infer stationary collision from a few sampled or repeated poses.
    for polygon in &mut source.parts[0].mesh.polygons {
        polygon.collidable = false;
    }
    assert!(source.stationary_collision_animation_radius().is_ok());
}

#[test]
#[ignore = "requires original installed object archives in EQ_DIR; five-frame admission audit"]
fn original_five_frame_corpus_keeps_conservative_numeric_and_collision_gates() {
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    let zones: &[(&str, &[(&str, bool)])] = &[
        ("dreadlands", &[("tree105", true)]),
        (
            "firiona",
            &[
                ("swamptree100", false),
                ("swamptree101", true),
                ("swamptree102", false),
            ],
        ),
        (
            "growthplane",
            &[("purptree200", true), ("tree105", true), ("tree106", false)],
        ),
        (
            "mischiefplane",
            &[
                ("purptree200", true),
                ("swamptree100", false),
                ("tree105", true),
                ("tree106", false),
            ],
        ),
        (
            "pomischief",
            &[
                ("purptree200", true),
                ("swamptree100", false),
                ("tree105", true),
                ("tree106", false),
            ],
        ),
        (
            "swampofnohope",
            &[
                ("swamptree100", false),
                ("swamptree101", true),
                ("swamptree102", false),
            ],
        ),
    ];
    let mut admitted = 0;
    for &(zone, actors) in zones {
        let scene = super::super::super::load_object_library(&base, zone).unwrap();
        for &(key, expected) in actors {
            let source = &scene.wld_object_sources[key];
            assert_eq!(source.animation_period().is_ok(), expected, "{zone}/{key}");
            assert_eq!(
                source.render_animation().is_some(),
                expected,
                "{zone}/{key}"
            );
            if !expected {
                continue;
            }
            admitted += 1;
            let radius = source.stationary_collision_animation_radius().unwrap();
            let initial = source.sample_animation(Duration::ZERO).unwrap();
            for milliseconds in [0, 500, 1250, 2000, 3750, 4999, 5000] {
                let sampled = source
                    .sample_animation(Duration::from_millis(milliseconds))
                    .unwrap();
                for (mesh, first) in sampled.iter().zip(&initial) {
                    for polygon in mesh.polygons.iter().filter(|polygon| polygon.collidable) {
                        for vertex in [polygon.a, polygon.b, polygon.c] {
                            assert_eq!(
                                mesh.vertices[vertex as usize],
                                first.vertices[vertex as usize]
                            );
                        }
                    }
                    assert!(
                        mesh.vertices
                            .iter()
                            .all(|vertex| Vec3::from_array(*vertex).length() <= radius)
                    );
                }
            }
        }
    }
    assert_eq!(admitted, 9);
}
