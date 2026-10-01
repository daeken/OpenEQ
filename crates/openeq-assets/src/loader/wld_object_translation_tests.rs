use super::*;
use translation::FiveFrameTranslations;

// Compact numeric witness from original native build/compression/GetSRT.
// See docs/WLD_OBJECT_TRANSLATION.md; no archive payload is embedded.
fn translating_source() -> ObjectSource {
    let mut source = short_animation_source(false);
    let skeleton = source.skeleton.as_mut().unwrap();
    skeleton.tracks[0].definition.frames[0] = frame([0.; 3], 1.);
    skeleton.tracks[1].speed = Some(333);
    skeleton.tracks[1].definition.frames = [8179, 9423, 11033, 11959, 10156]
        .map(|z| frame([0., 74. / 256., z as f32 / 256.], 1.))
        .to_vec();
    source
}

#[test]
fn translation_uses_its_own_native_removed_key_and_original_timestamps() {
    let source = translating_source();
    let frames = &source.skeleton.as_ref().unwrap().tracks[1]
        .definition
        .frames;
    let keys = FiveFrameTranslations::new(frames).unwrap();
    assert_eq!(keys.omitted, 4);
    assert_eq!(
        key_reduction::FiveFrameKeys::new(frames).unwrap().omitted,
        1
    );
    assert_eq!(
        source.animation_period().unwrap(),
        Duration::from_millis(1665)
    );
    for (time, z_bits) in [
        (0, 0x41ff9800),
        (333, 0x42133c00),
        (500, 0x421fd9ab),
        (666, 0x422c6400),
        (999, 0x423adc00),
        (1000, 0x423ac54c),
        (1332, 0x421d5400),
        (1660, 0x42003d83),
    ] {
        let translation = [0., 74. / 256., f32::from_bits(z_bits)];
        assert_eq!(keys.sample(time, 333), translation);
        let sampled = source
            .sample_animation(Duration::from_millis(time as u64))
            .unwrap();
        for (point, original) in sampled[0]
            .vertices
            .iter()
            .zip(&source.parts[0].mesh.vertices)
        {
            near(
                *point,
                (Vec3::from(*original) + Vec3::from(translation)).to_array(),
            );
        }
    }
    // The authored position at index 4 is not a retained native key.
    assert_eq!(
        frames[4].translation[2] - keys.sample(1332, 333)[2],
        0.33984375
    );
    assert_eq!(
        source.sample_animation(Duration::ZERO).unwrap()[0].vertices,
        source
            .sample_animation(Duration::from_millis(1665))
            .unwrap()[0]
            .vertices,
    );
}

#[test]
fn native_three_axis_controls_cover_each_removed_translation_key() {
    // Four of 128 generated controls executed through the original native
    // builder/compressor, with identical selections in scalar/SSE2 PC64/PC24.
    // These are synthetic packed-grid coordinates, not archive payloads.
    for (omitted, packed, midpoint) in [
        (
            1,
            [
                [-121, -136, -192],
                [-46, 47, -93],
                [-219, 26, 93],
                [155, -15, 62],
                [197, 129, -215],
            ],
            [-170., -55., -49.5],
        ),
        (
            2,
            [
                [-228, 3, -99],
                [-243, 153, 200],
                [-93, -60, -120],
                [138, 115, -185],
                [-194, -219, -91],
            ],
            [-52.5, 134., 7.5],
        ),
        (
            3,
            [
                [69, 88, -188],
                [118, -186, -91],
                [247, 256, -198],
                [48, 29, -30],
                [194, -58, -15],
            ],
            [220.5, 99., -106.5],
        ),
        (
            4,
            [
                [46, -180, -46],
                [-236, -71, 87],
                [39, 81, -238],
                [-38, -38, 162],
                [-187, -81, 0],
            ],
            [4., -109., 58.],
        ),
    ] {
        let frames = packed.map(|point| frame(point.map(|word| word as f32 / 256.), 1.));
        let keys = FiveFrameTranslations::new(&frames).unwrap();
        assert_eq!(keys.omitted, omitted);
        for (index, frame) in frames.iter().enumerate() {
            let expected = if index == omitted {
                midpoint.map(|word| word / 256.)
            } else {
                frame.translation
            };
            assert_eq!(keys.sample(index as u128 * 333, 333), expected);
        }
    }
}

#[test]
fn translation_admission_rejects_optimizer_runs_inexact_scores_and_other_motion() {
    let source = translating_source();
    let frames = &source.skeleton.as_ref().unwrap().tracks[1]
        .definition
        .frames;
    let mut cases = Vec::new();
    let mut repeated = frames.clone();
    repeated[2].translation = repeated[1].translation;
    cases.push(repeated);
    let mut closing_repeat = frames.clone();
    closing_repeat[4].translation = closing_repeat[0].translation;
    cases.push(closing_repeat);
    let mut nonpacked = frames.clone();
    nonpacked[1].translation[2] += 0.0001;
    cases.push(nonpacked);
    let mut rotating = frames.clone();
    rotating[1].rotation = [0., 0., 1., 0.];
    cases.push(rotating);
    let mut scaling = frames.clone();
    scaling[1].scale = 2.;
    cases.push(scaling);
    let mut inexact = frames.clone();
    inexact[0].translation[0] = -128.;
    inexact[1].translation[0] = 127.;
    inexact[2].translation[0] = 127.;
    assert!(
        FiveFrameTranslations::new(&inexact)
            .unwrap_err()
            .to_string()
            .contains("inexact")
    );
    cases.push(inexact);
    let mut tied = frames.clone();
    for (key, z) in tied.iter_mut().zip([0., 1., 2., 3., 4.]) {
        key.translation[2] = z;
    }
    cases.push(tied);
    cases.push(frames[..4].to_vec());
    for frames in cases {
        let mut rejected = source.clone();
        rejected.skeleton.as_mut().unwrap().tracks[1]
            .definition
            .frames = frames;
        assert!(rejected.animation_period().is_err());
        assert!(rejected.sample_animation(Duration::ZERO).is_err());
        assert!(posed_meshes(&rejected).is_ok());
    }
}

#[test]
fn translation_bounds_cover_the_full_motion_and_collision_stays_gated() {
    let mut source = translating_source();
    assert!(source.stationary_collision_animation_radius().is_err());
    for polygon in &mut source.parts[0].mesh.polygons {
        polygon.collidable = false;
    }
    source.parts[0].mesh.vertices.fill([0.; 3]);
    let radius = source.stationary_collision_animation_radius().unwrap();
    assert!(
        radius > 46.71 && radius < 46.75,
        "unexpected radius {radius}"
    );
    for millis in 0..=1665 {
        let sampled = source
            .sample_animation(Duration::from_millis(millis))
            .unwrap();
        assert!(
            sampled[0]
                .vertices
                .iter()
                .all(|point| Vec3::from(*point).length() < radius)
        );
    }
    source.parts[0].mesh.polygons[0].collidable = true;
    assert!(source.stationary_collision_animation_radius().is_err());
}

#[test]
#[ignore = "requires original electric-monument object archives in EQ_DIR"]
fn original_translating_monuments_preserve_all_physical_vertices() {
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    for (zone, key, period) in [
        ("eastwastes", "electmonu200", 1665),
        ("eastwastesshard", "electmonu200", 1665),
        ("westwastes", "electmonu200", 1665),
        ("necropolis", "electmonu201", 5000),
        ("sleeper", "electmonu201", 5000),
    ] {
        let scene = super::super::super::load_object_library(&base, zone).unwrap();
        let source = &scene.wld_object_sources[key];
        assert_eq!(source.parts.len(), 3);
        assert!(source.particle_attachments.is_empty());
        assert_eq!(
            source.animation_period().unwrap(),
            Duration::from_millis(period)
        );
        assert!(source.render_animation().is_some());
        let initial = source.sample_animation(Duration::ZERO).unwrap();
        assert_eq!(
            initial
                .iter()
                .flat_map(|m| &m.polygons)
                .filter(|p| p.collidable)
                .count(),
            200
        );
        let radius = source.stationary_collision_animation_radius().unwrap();
        for millis in [
            0,
            period / 5,
            period * 3 / 5,
            period * 4 / 5,
            period - 1,
            period,
        ] {
            let sampled = source
                .sample_animation(Duration::from_millis(millis))
                .unwrap();
            let mut moved = false;
            for (mesh, first) in sampled.iter().zip(&initial) {
                for polygon in mesh.polygons.iter().filter(|p| p.collidable) {
                    for vertex in [polygon.a, polygon.b, polygon.c] {
                        assert_eq!(
                            mesh.vertices[vertex as usize],
                            first.vertices[vertex as usize]
                        );
                    }
                }
                moved |= mesh.vertices != first.vertices;
                assert!(
                    mesh.vertices
                        .iter()
                        .all(|point| Vec3::from(*point).length() <= radius)
                );
            }
            assert_eq!(
                moved,
                millis != 0 && millis != period,
                "{zone}/{key} at {millis}"
            );
        }
    }
}
