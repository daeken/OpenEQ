use super::*;
use zero_reduction::SixteenFrameKeys;

fn packed_frames(axis: usize, words: [[i32; 2]; 16]) -> Vec<Frame> {
    words
        .map(|[value, w]| {
            let mut result = frame([0.; 3], 1.);
            result.rotation[axis] = value as f32 / 16384.;
            result.rotation[3] = w as f32 / 16384.;
            result
        })
        .to_vec()
}

fn lamp_frames() -> Vec<Frame> {
    // Compact original packed numeric witness, not an archive fixture.
    packed_frames(
        0,
        [
            [101, -16383],
            [0, 16384],
            [101, 16383],
            [302, 16381],
            [402, 16379],
            [503, 16376],
            [603, 16373],
            [603, 16373],
            [603, 16373],
            [603, 16373],
            [503, 16376],
            [402, 16379],
            [302, 16381],
            [101, 16383],
            [0, 16384],
            [101, -16383],
        ],
    )
}

fn lamp_source() -> ObjectSource {
    let mut source = short_animation_source(false);
    let skeleton = source.skeleton.as_mut().unwrap();
    skeleton.tracks[0].definition.frames[0] = frame([0.; 3], 1.);
    skeleton.tracks[1].definition.frames = lamp_frames();
    skeleton.tracks[1].speed = Some(100);
    for polygon in &mut source.parts[0].mesh.polygons {
        polygon.collidable = false;
    }
    source
}

#[test]
fn scalar_lamp_witness_keeps_native_heap_ties_and_authored_period() {
    let frames = lamp_frames();
    let keys = SixteenFrameKeys::new(&frames).unwrap();
    assert_eq!(keys.omitted, [1, 8]);
    let source = lamp_source();
    assert_eq!(
        source.animation_period().unwrap(),
        Duration::from_millis(1600)
    );
    // Executed original scalarPC64 GetSRT plus normalization, including the
    // first omitted key's extended span and the repeated plateau.
    for (time, x, w) in [
        (0, -0.0061648097, 0.999981),
        (37, -0.0038838747, 0.9999925),
        (74, -0.001602879, 0.99999875),
        (80, -0.0012329845, 0.9999992),
        (111, 0.0006781418, 0.99999976),
        (160, 0.0036989308, 0.99999315),
        (400, 0.024536233, 0.99969894),
        (800, 0.036803972, 0.9993225),
        (1440, -0.0024658733, 0.99999696),
        (1520, -0.0061648097, 0.999981),
    ] {
        let actual = Quat::from_array(keys.sample(time, 100)).normalize();
        let expected = Quat::from_xyzw(x, 0., 0., w);
        assert!(
            (actual - expected)
                .length()
                .min((actual + expected).length())
                < 2e-7,
            "{time}ms: {actual:?} != {expected:?}"
        );
    }
    assert_eq!(
        source.sample_animation(Duration::ZERO).unwrap()[0].vertices,
        source
            .sample_animation(Duration::from_millis(1600))
            .unwrap()[0]
            .vertices,
    );
    let mut flipped = frames;
    for index in [0, 3, 7, 15] {
        flipped[index].rotation = flipped[index].rotation.map(|v| -v);
    }
    assert_eq!(SixteenFrameKeys::new(&flipped).unwrap().omitted, [1, 8]);
}

#[test]
fn native_synthetic_controls_keep_consecutive_removed_key_spans() {
    for (axis, words, omitted) in [
        (
            0,
            [
                [702, 16369],
                [559, 16375],
                [559, 16375],
                [559, 16375],
                [-101, 16383],
                [0, 16384],
                [101, 16383],
                [423, 16380],
                [64, 16384],
                [-925, 16359],
                [188, 16383],
                [347, 16381],
                [-518, 16377],
                [613, 16373],
                [741, 16367],
                [-807, 16364],
            ],
            [1, 2],
        ),
        (
            2,
            [
                [-710, 16369],
                [-710, 16369],
                [334, 16382],
                [455, 16378],
                [-536, 16375],
                [-833, 16363],
                [-486, 16377],
                [-524, 16376],
                [851, 16362],
                [-50, 16384],
                [-708, 16369],
                [821, 16363],
                [-435, 16378],
                [-670, 16370],
                [-710, 16369],
                [-710, 16369],
            ],
            [14, 15],
        ),
    ] {
        let frames = packed_frames(axis, words);
        let keys = SixteenFrameKeys::new(&frames).unwrap();
        assert_eq!(keys.omitted, omitted);
        // Native retained timestamps include both original endpoints, even
        // when the last two authored keys are removed before loop closure.
        for index in (0..16).filter(|index| !omitted.contains(index)) {
            assert_eq!(
                keys.sample(index as u128 * 100, 100),
                frames[index].rotation
            );
        }
        let left = omitted[0] - 1;
        let right = omitted[1] + 1;
        for time in [omitted[0] * 100, omitted[1] * 100, right * 100 - 1] {
            let fraction = (time - left * 100) as f64 / 300.;
            let expected = std::array::from_fn(|component| {
                let a = f64::from(frames[left].rotation[component]);
                let b = f64::from(frames[right % 16].rotation[component]);
                (a + (b - a) * fraction) as f32
            });
            assert_eq!(keys.sample(time as u128, 100), expected);
        }
    }
}

#[test]
fn sixteen_frame_gate_rejects_unproven_motion_without_losing_first_pose() {
    let source = lamp_source();
    let frames = lamp_frames();
    let mut cases = Vec::new();
    for (component, delta, reason) in [(0, 1., "rounding boundary"), (0, 10., "clamp boundary")] {
        let mut changed = frames.clone();
        changed[0].rotation[component] += delta / 16384.;
        cases.push((changed, reason));
    }
    let mut multiple_axes = frames.clone();
    multiple_axes[3].rotation[1] = 1. / 16384.;
    cases.push((multiple_axes, "one axis"));
    let mut nonpacked = frames.clone();
    nonpacked[3].rotation[0] += 1e-7;
    cases.push((nonpacked, "encoding"));
    for scale in [false, true] {
        let mut changed = frames.clone();
        if scale {
            changed[3].scale = 2.;
        } else {
            changed[3].translation[0] = 1.;
        }
        cases.push((changed, "constant translation and scale"));
    }
    // Generated negative witnesses: losing a zero after the first removal,
    // and clips with fewer than the two required zero-error candidates.
    for (words, reason) in [
        (
            [
                [307, 16381],
                [-684, 16371],
                [-902, 16359],
                [834, 16364],
                [-950, 16356],
                [-346, 16380],
                [804, 16364],
                [-284, 16382],
                [-445, 16379],
                [-414, 16379],
                [-578, 16374],
                [604, 16374],
                [-932, 16357],
                [-932, 16357],
                [-932, 16357],
                [-932, 16357],
            ],
            "changes error category",
        ),
        (
            [
                [-528, 16375],
                [465, 16378],
                [-887, 16360],
                [-745, 16367],
                [-745, 16367],
                [-745, 16367],
                [-745, 16367],
                [-636, 16372],
                [493, 16377],
                [-936, 16357],
                [-700, 16369],
                [999, 16354],
                [-890, 16360],
                [446, 16378],
                [-128, 16383],
                [464, 16377],
            ],
            "no first zero",
        ),
        (
            [
                [-339, 16381],
                [-119, 16385],
                [882, 16360],
                [69, 16384],
                [-101, 16383],
                [0, 16384],
                [101, 16383],
                [-437, 16378],
                [-221, 16383],
                [602, 16374],
                [-69, 16385],
                [-39, 16384],
                [-781, 16365],
                [-781, 16365],
                [-781, 16365],
                [-781, 16365],
            ],
            "no second zero",
        ),
    ] {
        cases.push((packed_frames(0, words), reason));
    }
    cases.push((frames[..15].to_vec(), "constant translation and scale"));
    for (frames, reason) in cases {
        assert!(
            SixteenFrameKeys::new(&frames)
                .unwrap_err()
                .to_string()
                .contains(reason),
            "{reason}"
        );
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
fn sixteen_frame_animation_keeps_collision_ancestry_gate_and_bounds() {
    let mut source = lamp_source();
    let radius = source.stationary_collision_animation_radius().unwrap();
    for time in 0..=1600 {
        for mesh in source
            .sample_animation(Duration::from_millis(time))
            .unwrap()
        {
            assert!(
                mesh.vertices
                    .iter()
                    .all(|v| Vec3::from(*v).length() <= radius)
            );
        }
    }
    source.parts[0].mesh.polygons[0].collidable = true;
    assert!(
        source
            .stationary_collision_animation_radius()
            .unwrap_err()
            .to_string()
            .contains("moves collision ancestry")
    );
}

#[test]
#[ignore = "requires original long-actor object archives in EQ_DIR"]
fn original_lamp_is_admitted_but_mixed_motion_crate_remains_static() {
    let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
    let scene = super::super::super::load_object_library(&base, "swampofnohope").unwrap();
    let source = &scene.wld_object_sources["krlamp101"];
    assert_eq!(source.parts.len(), 2);
    assert!(source.particle_attachments.is_empty());
    assert!(source.render_animation().is_some());
    assert_eq!(
        source.animation_period().unwrap(),
        Duration::from_millis(1600)
    );
    let initial = source.sample_animation(Duration::ZERO).unwrap();
    assert_eq!(
        initial
            .iter()
            .map(|mesh| mesh.polygons.len())
            .sum::<usize>(),
        185
    );
    assert!(
        initial
            .iter()
            .flat_map(|mesh| &mesh.polygons)
            .all(|p| !p.collidable)
    );
    let moved = source.sample_animation(Duration::from_millis(800)).unwrap();
    assert_eq!(initial[0].vertices, moved[0].vertices);
    assert_ne!(initial[1].vertices, moved[1].vertices);
    assert_eq!(
        initial[1].vertices,
        source
            .sample_animation(Duration::from_millis(1600))
            .unwrap()[1]
            .vertices
    );
    let scene = super::super::super::load_object_library(&base, "overthere").unwrap();
    let source = &scene.wld_object_sources["vscrate103"];
    assert!(source.render_animation().is_none());
    assert!(source.animation_period().is_err());
    assert!(!posed_meshes(source).unwrap().is_empty());
}
