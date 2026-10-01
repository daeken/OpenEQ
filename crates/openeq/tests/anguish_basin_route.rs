//! Original Anguish's traversed overhead route stays dry. Offline only.
use openeq::{
    movement::{GroundMotion, MotionInput, MotionMode, MotionWorld},
    movement_rules::PlayerGravity,
};
use openeq_assets::{
    collision::CollisionWorld,
    liquid_regions::{LiquidKind, LiquidRegions},
    loader,
};

#[test]
#[ignore = "requires original Anguish assets; no GPU, audio device or server"]
fn original_anguish_entrance_route_reaches_dry_floor_above_registered_water() {
    let base = loader::default_client_dir().expect("original client assets");
    let scene = loader::load_zone(&base, "anguish").unwrap();
    let collision = CollisionWorld::build(&scene);
    let liquids = LiquidRegions::load(&base, "anguish").unwrap();
    // Keep the verified authored box distinct from the floor and visible sheet.
    // A successful dry route must not result from dropping liquid metadata.
    assert_eq!(
        liquids.at([700., -10., -256.87112]),
        Some(LiquidKind::Water)
    );
    assert_eq!(liquids.at([700., -10., -288.19308]), None);
    let floor = collision
        .ground_height(700., -10., -180., 0., 60.)
        .expect("original physical prison floor");
    assert!((floor + 198.0401).abs() < 0.01);
    assert_eq!(liquids.at([700., -10., floor + 3.]), None);

    // XY targets follow the actual curved bridge, lower entrance ramp and
    // central room. Initial feet are on the original entrance-side terrain;
    // no teleport, flight, recovery, dynamic geometry or synthetic floor is used.
    let waypoints = [
        [-1900., 10.],
        [-1850., 40.],
        [-1750., 90.],
        [-1600., 110.],
        [-1500., 130.],
        [-1350., 70.],
        [-1200., 50.],
        [-1050., 10.],
        [-900., -35.],
        [-750., -80.],
        [-650., -90.],
        [-500., -75.],
        [-350., -35.],
        [-200., -25.],
        [0., 0.],
        [200., 0.],
        [350., 0.],
        [550., 0.],
        [700., -10.],
    ];
    let start = [-2000., 0., -90.230705];
    assert!(collision.supports_player(start, 1., 0.02));
    for fps in [10, 30, 120] {
        let mut motion = GroundMotion::default();
        let mut feet = start;
        for (index, target) in waypoints.into_iter().enumerate() {
            let mut reached = false;
            let mut stationary = 0.;
            let mut jumped_at_stall = false;
            let mut jumps = 0;
            for _ in 0..fps * 40 {
                let delta = [target[0] - feet[0], target[1] - feet[1]];
                let distance = delta[0].hypot(delta[1]);
                if distance < 1. {
                    reached = true;
                    break;
                }
                // Slow only the final frame of each waypoint to avoid a
                // controller overshoot at low FPS. Physics remains unchanged.
                let speed = 40f32.min(distance * fps as f32);
                let velocity = [speed * delta[0] / distance, speed * delta[1] / distance, 0.];
                let jump = stationary >= 0.25 && !jumped_at_stall && jumps < 3;
                if jump {
                    jumped_at_stall = true;
                    jumps += 1;
                }
                let previous = feet;
                feet = motion.step_in_world(
                    MotionWorld {
                        collision: &collision,
                        dynamic: None,
                        liquids: Some(&liquids),
                    },
                    feet,
                    MotionInput {
                        walk_velocity: [velocity[0], velocity[1]],
                        volume_velocity: velocity,
                        jump,
                        gravity: PlayerGravity::Grounded,
                    },
                    1. / fps as f32,
                );
                assert_eq!(
                    motion.mode,
                    MotionMode::Ground,
                    "{fps} FPS, waypoint {index}: {feet:?}"
                );
                assert!(feet.iter().all(|coordinate| coordinate.is_finite()));
                let horizontal = (feet[0] - previous[0]).abs() + (feet[1] - previous[1]).abs();
                if horizontal < 0.001 {
                    stationary += 1. / fps as f32;
                } else {
                    stationary = 0.;
                    jumped_at_stall = false;
                }
                if stationary >= 2. {
                    break;
                }
            }
            assert!(
                reached,
                "{fps} FPS, waypoint {index} {target:?}: stopped at {feet:?}"
            );
        }
        assert!((feet[0] - 700.).abs() < 1. && (feet[1] + 10.).abs() < 1.);
        assert!((feet[2] - floor).abs() < 0.01, "{fps} FPS: {feet:?}");
        assert!(collision.supports_player(feet, 1., 0.02));
    }
}
