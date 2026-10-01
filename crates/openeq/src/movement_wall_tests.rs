//! Wall-prefix witnesses that a future deflected-medium certificate must cover.
//! These deliberately retain the existing whole-tick fallback: a correction
//! segment and a monotone tangent-time search are both insufficient.

use super::*;
use openeq_assets::{
    Scene,
    liquid_regions::{LiquidBox, LiquidKind},
    mesh::Geometry,
};

const FLOOR: [[f32; 3]; 4] = [
    [-50., -50., 0.],
    [50., -50., 0.],
    [50., 50., 0.],
    [-50., 50., 0.],
];
const WALL: [[f32; 3]; 4] = [
    [1.1, -20., 0.],
    [1.1, 20., 0.],
    [1.1, 20., 20.],
    [1.1, -20., 20.],
];

fn geometry(quads: &[[[f32; 3]; 4]]) -> CollisionWorld {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for quad in quads {
        let offset = (vertices.len() / 8) as u32;
        for point in quad {
            vertices.extend_from_slice(&[point[0], point[1], point[2], 0., 0., 1., 0., 0.]);
        }
        indices.extend([0, 1, 2, 0, 2, 3].map(|i| i + offset));
    }
    CollisionWorld::build(&Scene::from_geometry(
        "wall-medium".into(),
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

fn liquid(low_y: f32, high_y: f32) -> LiquidRegions {
    LiquidRegions::from_boxes([LiquidBox {
        kind: LiquidKind::Water,
        center: [0.09, (low_y + high_y) * 0.5, 3.],
        half_extents: [0.02, (high_y - low_y) * 0.5, 3.],
        rotation: [0., 0., 0., 1.],
    }])
    .unwrap()
}

fn input() -> MotionInput {
    MotionInput {
        walk_velocity: [40., 40.],
        volume_velocity: [40., 40., 0.],
        jump: false,
        gravity: PlayerGravity::Grounded,
    }
}

fn prefix(world: &MotionWorld<'_>, fraction: f32) -> ResolvedMove {
    let dt = STEP as f32;
    world.move_velocity(
        [0.; 3],
        [40., 40., -GRAVITY * dt],
        dt * fraction,
        true,
        MotionMode::Ground,
    )
}

fn advance(world: &MotionWorld<'_>, liquids: &LiquidRegions) -> ([f32; 3], u32, MotionMode) {
    let mut motion = GroundMotion::default();
    let feet = motion.step_in_world(
        MotionWorld {
            collision: world.collision,
            dynamic: world.dynamic,
            liquids: Some(liquids),
        },
        [0.; 3],
        input(),
        STEP as f32,
    );
    (feet, motion.velocity_z.to_bits(), motion.mode)
}

fn snapshots(mut check: impl FnMut(MotionWorld<'_>)) {
    let all = geometry(&[FLOOR, WALL]);
    let floor = geometry(&[FLOOR]);
    let wall = geometry(&[WALL]);
    let empty = CollisionWorld::default();
    for (collision, dynamic) in [
        (&all, None),
        (&empty, Some(&all)),
        (&floor, Some(&wall)),
        (&wall, Some(&floor)),
    ] {
        check(MotionWorld {
            collision,
            dynamic,
            liquids: None,
        });
    }
}

#[test]
fn wall_slide_can_cross_water_outside_its_endpoint_chord() {
    let water = liquid(0.13, 0.2);
    snapshots(|world| {
        let end = prefix(&world, 1.);
        let inside = prefix(&world, 0.39);
        assert!(!end.straight);
        assert!(!inside.straight);
        assert!(water.at(center([0.; 3])).is_none());
        assert!(water.at(center(end.feet)).is_none());
        assert!(water.at(center(inside.feet)).is_some());
        assert!(water.segment(center([0.; 3]), center(end.feet)).is_empty());
        // Until a wall certificate establishes the actual prefix family, do
        // not turn its endpoint depenetration into invented liquid travel.
        assert_eq!(
            advance(&world, &water),
            advance(&world, &LiquidRegions::default())
        );
    });
}

#[test]
fn wall_tangent_rounding_makes_one_box_nonmonotone_in_requested_time() {
    let water = liquid(0.12996963, 0.25);
    snapshots(|world| {
        // Consecutive f32 fractions, using velocity * (tick * fraction) as
        // movement does. This reversal is along Y, tangent to the X wall;
        // excluding the normal-direction skin jump would not remove it.
        let before = prefix(&world, f32::from_bits(1_053_270_558)).feet;
        let after = prefix(&world, f32::from_bits(1_053_270_559)).feet;
        let later = prefix(&world, f32::from_bits(1_053_270_565)).feet;
        assert!(after[1] < before[1], "{before:?} -> {after:?}");
        assert!(water.at(center(before)).is_some());
        assert!(water.at(center(after)).is_none());
        assert!(water.at(center(later)).is_some());
        let min = [0., 0., 3.];
        let max = [0.34, 0.34, 3.];
        assert!(water.height_invariant_in_bounds(min, max));
        assert!(water.has_single_liquid_interval_in_bounds(min, max));
        // The existing ramp gate needs monotone *position* in addition to
        // these volume properties. They cannot admit an ordinary wall slide.
        assert_eq!(
            advance(&world, &water),
            advance(&world, &LiquidRegions::default())
        );
    });
}

#[test]
fn dry_and_submerged_wall_slides_keep_the_original_complete_solve() {
    let dry = liquid(10., 11.);
    let wet = LiquidRegions::from_boxes([LiquidBox {
        kind: LiquidKind::Water,
        center: [0., 0., 3.],
        half_extents: [10.; 3],
        rotation: [0., 0., 0., 1.],
    }])
    .unwrap();
    snapshots(|world| {
        assert_eq!(
            advance(&world, &dry),
            advance(&world, &LiquidRegions::default())
        );
        let original = world.move_velocity(
            [0.; 3],
            [24., 24., 0.],
            STEP as f32,
            true,
            MotionMode::Swimming,
        );
        assert_eq!(
            advance(&world, &wet),
            (
                original.feet,
                original.velocity_z.to_bits(),
                MotionMode::Swimming
            )
        );
    });
}
