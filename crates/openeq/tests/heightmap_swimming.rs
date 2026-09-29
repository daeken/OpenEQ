//! Offline movement through native DAT liquid volumes; no server or GPU.
use openeq::{
    movement::{GroundMotion, MotionInput, MotionMode, MotionWorld},
    movement_rules::PlayerGravity,
};
use openeq_assets::{Scene, collision::CollisionWorld, liquid_regions::LiquidRegions, loader};

fn input(velocity: [f32; 3]) -> MotionInput {
    MotionInput {
        walk_velocity: [velocity[0], velocity[1]],
        volume_velocity: velocity,
        jump: false,
        gravity: PlayerGravity::Grounded,
    }
}

#[test]
#[ignore = "requires original Maiden's Grave assets; no GPU or server connection"]
fn maidensgrave_ocean_holds_depth_and_allows_swimming_to_its_authored_surface() {
    let base = loader::default_client_dir().expect("original client assets");
    let liquid = LiquidRegions::load(&base, "maidensgrave").unwrap();
    let scene = loader::load_zone(&base, "maidensgrave").unwrap();
    let collision = CollisionWorld::build(&scene);
    // Original tile [2,9], center of its grid: terrain is near Z29.33 while
    // the surface lies near Z300.04. This is actual open water above the floor.
    let start = [960., 3648., 280.];
    assert!(
        collision
            .ground_height(start[0], start[1], start[2], 0., 500.)
            .unwrap()
            < 250.
    );
    for fps in [10, 30, 120] {
        let mut motion = GroundMotion::default();
        let mut feet = start;
        let advance = |motion: &mut GroundMotion, feet, velocity| {
            motion.step_in_world(
                MotionWorld {
                    collision: &collision,
                    dynamic: None,
                    liquids: Some(&liquid),
                },
                feet,
                input(velocity),
                1. / fps as f32,
            )
        };
        for _ in 0..fps {
            feet = advance(&mut motion, feet, [0.; 3]);
        }
        assert_eq!(motion.mode, MotionMode::Swimming);
        assert_eq!(feet, start, "{fps} FPS: idle water drift");
        for _ in 0..fps {
            feet = advance(&mut motion, feet, [40., 0., 0.]);
        }
        assert!(
            (feet[0] - start[0] - 24.).abs() < 0.04,
            "{fps} FPS: {feet:?}"
        );
        assert_eq!(feet[2], start[2]);
        let mut surfaced = false;
        for _ in 0..fps * 2 {
            feet = advance(&mut motion, feet, [0., 0., 40.]);
            surfaced |= liquid.at([feet[0], feet[1], feet[2] + 6.]).is_none();
        }
        assert!(
            surfaced,
            "{fps} FPS: never reached the authored surface: {feet:?}"
        );
        assert!(
            (294.0..304.0).contains(&feet[2]),
            "{fps} FPS: invalid surface position: {feet:?}"
        );
    }
}

#[test]
#[ignore = "requires original Maiden's Grave liquid metadata; no GPU or server connection"]
fn maidensgrave_finite_side_boundary_changes_motion_at_every_frame_rate() {
    let base = loader::default_client_dir().expect("original client assets");
    let liquid = LiquidRegions::load(&base, "maidensgrave").unwrap();
    // Isolate the liquid boundary from terrain; the box center/size come from
    // the actual DAT, while this empty collision world adds no other behavior.
    let collision = CollisionWorld::build(&Scene::from_geometry(
        "liquid boundary fixture".into(),
        vec![],
        vec![],
        vec![],
    ));
    let min_x = 3011.3552 - 3500.;
    for entering in [false, true] {
        for fps in [10, 30, 120] {
            let sign = if entering { 1. } else { -1. };
            let start = [min_x - sign, 4048.7725, -202.96002];
            let mut feet = start;
            let mut motion = GroundMotion::default();
            for _ in 0..fps {
                feet = motion.step_in_world(
                    MotionWorld {
                        collision: &collision,
                        dynamic: None,
                        liquids: Some(&liquid),
                    },
                    feet,
                    input([sign * 40., 0., 0.]),
                    1. / fps as f32,
                );
            }
            let expected = if entering { 24.4 } else { 39.333332 };
            assert!(
                ((feet[0] - start[0]) * sign - expected).abs() < 0.04,
                "{fps} FPS, entering={entering}: {feet:?}"
            );
            assert_eq!(
                motion.mode,
                if entering {
                    MotionMode::Swimming
                } else {
                    MotionMode::Ground
                }
            );
        }
    }
}
