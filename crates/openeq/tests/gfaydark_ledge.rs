//! Offline original-asset regression for the reported Kelethin ramp-end snag.
//! Run: cargo test -p openeq --test gfaydark_ledge -- --ignored --nocapture
use openeq::{coordinates, movement::GroundMotion};
use openeq_assets::{Scene, collision::CollisionWorld, loader, mesh::Geometry};

/// Minimal authored ramp/deck overlap: both center-plane heights are below
/// the feet when the front of the body's circle touches the raised end cap.
fn isolated_cap(low_ceiling: bool) -> CollisionWorld {
    let mut quads = vec![
        [
            [-20., -6., -1.71875],
            [20., -6., -1.71875],
            [20., 0., 0.09375],
            [-20., 0., 0.09375],
        ],
        [
            [-20., 0., -3.],
            [20., 0., -3.],
            [20., 0., 0.09375],
            [-20., 0., 0.09375],
        ],
        [
            [-20., -0.5625, 0.],
            [20., -0.5625, 0.],
            [20., 100., 0.],
            [-20., 100., 0.],
        ],
    ];
    if low_ceiling {
        // Standing at deck height fits; rising onto the 0.09375 cap does not.
        quads.push([
            [-20., -10., 6.04],
            [20., -10., 6.04],
            [20., 100., 6.04],
            [-20., 100., 6.04],
        ]);
    }
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for points in quads {
        let offset = (vertices.len() / 8) as u32;
        for [x, y, z] in points {
            vertices.extend_from_slice(&[x, y, z, 0., 0., 1., 0., 0.]);
        }
        indices.extend([0, 1, 2, 0, 2, 3].map(|index| index + offset));
    }
    CollisionWorld::build(&Scene::from_geometry(
        "isolated raised ramp cap".into(),
        vec![],
        vec![Geometry {
            vertices,
            indices,
            material: 0,
            collidable: true,
        }],
        vec![],
    ))
}

#[test]
fn raised_ramp_cap_is_crossed_in_both_directions_without_an_extrapolated_floor() {
    let world = isolated_cap(false);
    for reverse in [false, true] {
        for fps in [10, 30, 60, 120] {
            let mut position = [0., if reverse { 5. } else { -1. }, 0.];
            let mut motion = GroundMotion::default();
            assert!(world.supports_player(position, 1., 0.05));
            for _ in 0..fps {
                position = motion.step(
                    &world,
                    position,
                    [0., if reverse { -6. } else { 6. }],
                    false,
                    1. / fps as f32,
                );
                assert!(
                    position[2] <= 0.10375,
                    "invented support above the actual cap: {position:?}"
                );
            }
            let expected_y = if reverse { -1. } else { 5. };
            assert!(
                (position[1] - expected_y).abs() < 0.025,
                "{fps} FPS, reverse={reverse}: snagged at {position:?}"
            );
            assert!(position[0].abs() < 0.01);
            assert!(world.supports_player(position, 1., 0.05));
        }
    }
}

#[test]
fn raised_ramp_cap_respects_head_clearance() {
    let world = isolated_cap(true);
    let mut motion = GroundMotion::default();
    let mut position = [0., -1., 0.];
    for _ in 0..120 {
        position = motion.step(&world, position, [0., 6.], false, 1. / 120.);
    }
    assert!(
        position[1] <= -0.99,
        "stepped through the low ceiling: {position:?}"
    );
    assert!(
        position[2].abs() < 0.01,
        "rose into the ceiling: {position:?}"
    );
    assert!(world.supports_player(position, 1., 0.05));
}

#[test]
#[ignore = "requires original Greater Faydark assets; no GPU or server connection"]
fn reported_kelethin_ramp_end_is_walkable_without_jumping_at_all_frame_rates() {
    let base = loader::default_client_dir().expect("set OPENEQ_CLIENT_DIR to original assets");
    let scene = loader::load_zone(&base, "gfaydark").unwrap();
    let world = CollisionWorld::build(&scene);

    // The reported server pose is XYZ=(41.50,-96.93,76.97), heading138.75.
    // Server XYZ swaps to asset XY. Main uses camera=server-center+3, then
    // feet=camera-6, so the reported center Z needs a further -3 for physics.
    // Unswapped XY has no platform at this elevation and misses the regression.
    let reported_feet = coordinates::server_point_to_scene([41.50, -96.93, 76.97 - 3.]);
    let heading = coordinates::server_heading_to_scene(138.75);
    assert_eq!(heading, 501.25);
    assert!(world.supports_player(reported_feet, 1., 0.05));
    let angle = heading * std::f32::consts::TAU / 512.;
    let forward = [angle.sin(), angle.cos()];

    // FAYFLOOR.BMP's slope reaches y42.5,z74.0625; the deck behind the lip
    // lies at z73.96875 and begins near y41.93. The cylindrical footprint
    // overlaps that deck while its center is still above the lower ramp.
    // Thus both walkable planes evaluated at the body's center are BELOW its
    // current feet, but the ramp's vertical end cap rises 0.09375 above deck.
    // Before the fix, looking only for higher center support planes rejected
    // every stair candidate and left the player stuck at y41.499.
    // The face's top triangle is:
    // [-80.5,42.5,74.0625],[-100.5,42.5,73.78125],[-100.5,42.5,74.0625].
    let mut results = Vec::new();
    for speed in [40., 60.] {
        for reverse in [false, true] {
            let start = if reverse {
                [
                    reported_feet[0] + forward[0] * speed,
                    reported_feet[1] + forward[1] * speed,
                    73.96875,
                ]
            } else {
                reported_feet
            };
            let direction = forward.map(|value| if reverse { -value } else { value });
            let velocity = direction.map(|value| value * speed);
            for fps in [10, 30, 60, 120] {
                let mut motion = GroundMotion::default();
                let mut position = start;
                let mut highest = start[2];
                for _ in 0..fps {
                    position = motion.step(&world, position, velocity, false, 1. / fps as f32);
                    highest = highest.max(position[2]);
                }
                let delta = [position[0] - start[0], position[1] - start[1]];
                let progress = delta[0] * direction[0] + delta[1] * direction[1];
                let lateral = delta[0] * direction[1] - delta[1] * direction[0];
                eprintln!(
                    "{fps} FPS, speed{speed}, reverse={reverse}: {start:?} -> {position:?}, forward={progress}, lateral={lateral}, highest={highest}"
                );
                results.push((fps, speed, reverse, position, progress, lateral, highest));
            }
        }
    }
    // Check after collecting all rates so a regression prints every witness.
    for (fps, speed, reverse, position, progress, lateral, highest) in results {
        let case = format!("{fps} FPS, speed{speed}, reverse={reverse}");
        assert!(
            (progress - speed).abs() < 0.05,
            "{case} snagged on the tiny ramp lip: {position:?}, progress={progress}"
        );
        assert!(
            lateral.abs() < 0.05,
            "{case} was pushed sideways: {lateral}"
        );
        // The reverse endpoint's footprint is tangent to the raised cap, so
        // either the deck or the actual lip can still provide edge support.
        let highest_support = if reverse { 74.0625 } else { 73.96875 };
        assert!(
            (73.94375..=highest_support + 0.025).contains(&position[2]),
            "{case} did not finish on an authored support: {position:?}"
        );
        assert!(
            highest < 74.2,
            "{case} introduced an unnecessary upward jump: {highest}"
        );
        assert!(
            world.supports_player(position, 1., 0.05),
            "{case} finished without ground support"
        );
    }
}
