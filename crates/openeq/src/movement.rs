//! Fixed-step live player motion. Positions are feet in EQ coordinates (Z up).
//! Presentation applies the eye offset; protocol serialization applies the
//! separate server center offset. Neither offset belongs in the physics state.

use openeq_assets::collision::CollisionWorld;

const STEP: f64 = 1.0 / 120.0;
// A roughly four-unit hop reaches its apex in a quarter second and lands in
// half a second. Increase impulse together with gravity: height alone should
// not make the player hang in the air longer.
const GRAVITY: f32 = 128.0;
const JUMP_SPEED: f32 = 32.0;
const RADIUS: f32 = 1.0;
const HEIGHT: f32 = 6.0;

#[derive(Clone, Debug, Default)]
pub struct GroundMotion {
    pub velocity_z: f32,
    /// Unsimulated time in seconds, normally less than one 1/120s step.
    pub accumulator: f64,
    jump_pending: bool,
}

impl GroundMotion {
    /// Advances real elapsed time using fixed 1/120s intervals. Horizontal input
    /// is velocity in units/second, already normalized/scaled by the caller.
    /// `jump` is a press event, not a held button. A press survives until the
    /// next physics step even at render rates greater than 120 FPS.
    ///
    /// A frame consumes at most 0.25s so resuming after a long suspension cannot
    /// launch the player across a zone. Ordinary 10/20/60/120 FPS frames all
    /// preserve elapsed time. Reset the state to Default after a teleport or
    /// when switching between fly mode and walking.
    pub fn step(
        &mut self,
        world: &CollisionWorld,
        feet: [f32; 3],
        velocity_xy: [f32; 2],
        jump: bool,
        elapsed: f32,
    ) -> [f32; 3] {
        self.step_with_dynamic(world, None, feet, velocity_xy, jump, elapsed)
    }

    pub fn step_with_dynamic(
        &mut self,
        world: &CollisionWorld,
        dynamic: Option<&CollisionWorld>,
        mut feet: [f32; 3],
        velocity_xy: [f32; 2],
        jump: bool,
        elapsed: f32,
    ) -> [f32; 3] {
        if !feet.iter().all(|value| value.is_finite()) {
            return feet;
        }
        if !self.accumulator.is_finite() || self.accumulator < 0.0 {
            self.accumulator = 0.0;
        }
        if !self.velocity_z.is_finite() {
            self.velocity_z = 0.0;
        }
        self.jump_pending |= jump;
        if elapsed.is_finite() && elapsed > 0.0 {
            self.accumulator += f64::from(elapsed.min(0.25));
        }
        let velocity_xy = velocity_xy.map(|value| if value.is_finite() { value } else { 0.0 });
        let dt = STEP as f32;
        // The tiny tolerance absorbs f32 render-clock rounding, not missing
        // simulation time. Keeping the accumulator in f64 avoids further drift.
        while self.accumulator + 1e-7 >= STEP {
            self.accumulator = (self.accumulator - STEP).max(0.0);
            let grounded =
                on_ground(world, feet) || dynamic.is_some_and(|world| on_ground(world, feet));
            if grounded && self.velocity_z < 0.0 {
                self.velocity_z = 0.0;
            }
            if std::mem::take(&mut self.jump_pending) && grounded {
                self.velocity_z = JUMP_SPEED;
            }
            self.velocity_z = (self.velocity_z - GRAVITY * dt).max(-80.0);
            let displacement = [
                velocity_xy[0] * dt,
                velocity_xy[1] * dt,
                self.velocity_z * dt,
            ];
            // Stair following is a grounded behavior. Giving an airborne body
            // a two-unit step range snaps the last two units of a fall/jump.
            let step_height = if grounded && self.velocity_z <= 0.0 {
                2.0
            } else {
                0.0
            };
            let moved = world.move_player_with_dynamic(
                dynamic,
                feet,
                displacement,
                RADIUS,
                HEIGHT,
                step_height,
            );
            if (moved[2] - (feet[2] + displacement[2])).abs() > 0.001 {
                self.velocity_z = 0.0;
            }
            feet = moved;
        }
        feet
    }
}

fn on_ground(world: &CollisionWorld, feet: [f32; 3]) -> bool {
    world.supports_player(feet, RADIUS, 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_assets::{Scene, mesh::Geometry};

    fn world(quads: &[[[f32; 3]; 4]]) -> CollisionWorld {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for points in quads {
            let offset = (vertices.len() / 8) as u32;
            for point in points {
                vertices.extend_from_slice(&[point[0], point[1], point[2], 0., 0., 1., 0., 0.]);
            }
            indices.extend([0, 1, 2, 0, 2, 3].map(|index| index + offset));
        }
        CollisionWorld::build(&Scene::from_geometry(
            "motion-test".into(),
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
    fn flat() -> CollisionWorld {
        world(&[[
            [-200., -200., 0.],
            [200., -200., 0.],
            [200., 200., 0.],
            [-200., 200., 0.],
        ]])
    }
    fn near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.025,
            "expected {expected}, got {actual}"
        );
    }
    fn simulate(fps: usize, frames: usize, jump: bool) -> ([f32; 3], f32) {
        let world = flat();
        let mut motion = GroundMotion::default();
        let mut feet = [0.; 3];
        for frame in 0..frames {
            feet = motion.step(&world, feet, [40., 0.], jump && frame == 0, 1. / fps as f32);
        }
        (feet, motion.velocity_z)
    }

    #[test]
    fn walking_preserves_real_time_at_10_60_and_120_fps() {
        for fps in [10, 60, 120] {
            let (feet, velocity) = simulate(fps, fps * 2, false);
            near(feet[0], 80.);
            near(feet[1], 0.);
            near(feet[2], 0.);
            near(velocity, 0.);
        }
    }

    #[test]
    fn jump_arc_matches_at_equal_time_across_frame_rates() {
        // Check both airborne descent and the completed landing, not just the
        // final zero height (which would hide premature floor snapping).
        for tenths in [1, 3, 6] {
            let (reference, reference_velocity) = simulate(120, 12 * tenths, true);
            for fps in [10, 60] {
                let (feet, velocity) = simulate(fps, fps * tenths / 10, true);
                for i in 0..3 {
                    near(feet[i], reference[i]);
                }
                near(velocity, reference_velocity);
            }
            if tenths == 3 {
                assert!(
                    reference[2] > 3.,
                    "fall should not snap to ground: {reference:?}"
                );
            }
            if tenths == 6 {
                near(reference[2], 0.);
            }
        }
    }

    #[test]
    fn jump_is_a_taller_short_hop_and_falling_accelerates_promptly() {
        let world = flat();
        let mut motion = GroundMotion::default();
        let mut feet = [0.; 3];
        let mut apex = (0., 0.);
        let mut landing = None;
        for tick in 1..=120 {
            feet = motion.step(&world, feet, [0.; 2], tick == 1, 1. / 120.);
            if feet[2] > apex.0 {
                apex = (feet[2], tick as f32 / 120.);
            }
            if feet[2] <= 0. && landing.is_none() {
                landing = Some(tick as f32 / 120.);
            }
        }
        assert!((3.75..4.1).contains(&apex.0), "jump height {apex:?}");
        assert!((0.23..0.27).contains(&apex.1), "jump apex {apex:?}");
        assert!(
            (0.46..0.53).contains(&landing.unwrap()),
            "airtime {landing:?}"
        );
        near(motion.velocity_z, 0.);

        let mut falling = GroundMotion::default();
        let mut feet = [0., 0., 100.];
        for _ in 0..30 {
            feet = falling.step(&world, feet, [0.; 2], false, 1. / 120.);
        }
        assert!(
            (95.7..96.1).contains(&feet[2]),
            "fall should accelerate: {feet:?}"
        );
        near(falling.velocity_z, -32.);
    }

    #[test]
    fn brief_jump_press_survives_a_frame_without_a_physics_tick() {
        let world = flat();
        let mut motion = GroundMotion::default();
        let feet = motion.step(&world, [0.; 3], [0.; 2], true, 1. / 240.);
        near(feet[2], 0.);
        let feet = motion.step(&world, feet, [0.; 2], false, 1. / 240.);
        assert!(feet[2] > 0.09 && motion.velocity_z > 11.);
    }

    #[test]
    fn suspension_is_clamped_without_halving_ten_fps_motion() {
        let world = flat();
        let mut motion = GroundMotion::default();
        let feet = motion.step(&world, [0.; 3], [40., 0.], false, 10.);
        near(feet[0], 10.);
        assert!(motion.accumulator < STEP);
        let feet = motion.step(&world, feet, [40., 0.], false, 0.1);
        near(feet[0], 14.);
    }

    #[test]
    fn closing_dynamic_door_blocks_motion_and_opening_restores_passage() {
        let floor = flat();
        let closed = world(&[[
            [5., -20., 0.],
            [5., 20., 0.],
            [5., 20., 20.],
            [5., -20., 20.],
        ]]);
        let opened = world(&[]);
        let mut motion = GroundMotion::default();
        let mut feet = [0.; 3];
        for _ in 0..120 {
            feet =
                motion.step_with_dynamic(&floor, Some(&closed), feet, [20., 0.], false, 1. / 120.);
        }
        near(feet[0], 4.);
        near(feet[2], 0.);
        for _ in 0..120 {
            feet =
                motion.step_with_dynamic(&floor, Some(&opened), feet, [20., 0.], false, 1. / 120.);
        }
        near(feet[0], 24.);
        near(feet[2], 0.);
    }

    #[test]
    fn fixed_step_walking_climbs_stairs_and_slides_at_a_wall() {
        let world = world(&[
            [
                [-200., -200., 0.],
                [200., -200., 0.],
                [200., 200., 0.],
                [-200., 200., 0.],
            ],
            [[5., -20., 0.], [5., 20., 0.], [5., 20., 2.], [5., -20., 2.]],
            [
                [5., -20., 2.],
                [15., -20., 2.],
                [15., 20., 2.],
                [5., 20., 2.],
            ],
            [
                [15., -20., 2.],
                [15., 20., 2.],
                [15., 20., 20.],
                [15., -20., 20.],
            ],
        ]);
        for speed in [4., 40.] {
            let mut motion = GroundMotion::default();
            let mut feet = [0.; 3];
            for _ in 0..(1200. / speed) as usize {
                feet = motion.step(&world, feet, [speed, 0.], false, 1. / 120.);
            }
            near(feet[0], 10.);
            near(feet[2], 2.);
            for _ in 0..120 {
                feet = motion.step(&world, feet, [40., 2.], false, 1. / 120.);
            }
            near(feet[0], 14.);
            near(feet[1], 2.);
            near(feet[2], 2.);
        }
    }

    #[test]
    fn short_ledges_can_be_walked_over_at_oblique_angles() {
        for rise in [0.125, 0.5, 2.] {
            let ledge = [
                [
                    [5., -200., 0.],
                    [5., 200., 0.],
                    [5., 200., rise],
                    [5., -200., rise],
                ],
                [
                    [5., -200., rise],
                    [100., -200., rise],
                    [100., 200., rise],
                    [5., 200., rise],
                ],
            ];
            let floor = flat();
            let obstacles = world(&ledge);
            let combined = world(&[
                [
                    [-200., -200., 0.],
                    [200., -200., 0.],
                    [200., 200., 0.],
                    [-200., 200., 0.],
                ],
                ledge[0],
                ledge[1],
            ]);
            for angle in [0f32, 30., 60., 80.] {
                let velocity = [
                    20. * angle.to_radians().cos(),
                    20. * angle.to_radians().sin(),
                ];
                for fps in [20, 120] {
                    let frames = (10. / velocity[0] * fps as f32).ceil() as usize;
                    for dynamic in [false, true] {
                        let mut motion = GroundMotion::default();
                        let mut feet = [0.; 3];
                        for _ in 0..frames {
                            feet = if dynamic {
                                motion.step_with_dynamic(
                                    &floor,
                                    Some(&obstacles),
                                    feet,
                                    velocity,
                                    false,
                                    1. / fps as f32,
                                )
                            } else {
                                motion.step(&combined, feet, velocity, false, 1. / fps as f32)
                            };
                        }
                        assert!(
                            (feet[0] - velocity[0] * frames as f32 / fps as f32).abs() < 0.025,
                            "stuck on {rise}-unit ledge at {angle} degrees, {fps} FPS, dynamic={dynamic}: {feet:?}"
                        );
                        near(feet[2], rise);
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "requires original Greater Faydark assets and GPU"]
    fn actual_kelethin_lift_landings_are_walkable() {
        use glam::{Quat, Vec3};
        use openeq_assets::loader;
        use openeq_render::{
            Renderer,
            doors::{DoorRenderer, DoorState},
        };
        let base = loader::default_client_dir().expect("original client assets");
        let terrain = CollisionWorld::build(&loader::load_zone(&base, "gfaydark").unwrap());
        let renderer = Renderer::new_headless(64, 64).unwrap();
        for (id, position, heading, parameter) in [
            (69, [137.463, 350.014, 2.1582], 256., 68),
            (77, [872.302, 221.448, -27.6523], 128., 98),
            (80, [-88.6519, -136.173, 2.4082], 128., 69),
        ] {
            for open in [0, 1] {
                let mut doors = DoorRenderer::load(&base, "gfaydark").unwrap();
                doors.update(
                    &renderer,
                    &[DoorState {
                        id,
                        name: "FAYLEVATOR".into(),
                        position,
                        heading,
                        open_type: 59,
                        inverted: true,
                        parameter,
                        size: 100,
                        state: open,
                        ..Default::default()
                    }],
                    0.,
                );
                let z = position[2] + 2.99893 + f32::from(open) * parameter as f32;
                // Lower: ramp -> lip -> platform. Upper: platform -> city deck,
                // whose authored height differs slightly from the lift.
                let xs = if open == 0 { [136., 97.] } else { [100., 60.] };
                let rotation = Quat::from_rotation_z(
                    std::f32::consts::FRAC_PI_2 - heading * std::f32::consts::TAU / 512.,
                );
                let surface = |x, y| {
                    // Parts of the lower ramps are buried in the forest floor.
                    // Start on the top surface, not the buried ramp underside.
                    [&terrain, doors.collision_world()]
                        .into_iter()
                        .filter_map(|world| world.ground_height(x, y, z + 20., 0., 40.))
                        .max_by(f32::total_cmp)
                        .unwrap()
                };
                let endpoints = [[xs[0], -6., 0.], [xs[1], 6., 0.]].map(|local| {
                    let p = Vec3::from(position) + rotation * Vec3::from(local);
                    [p.x, p.y, surface(p.x, p.y)]
                });
                for reverse in [false, true] {
                    let [start, end] = if reverse {
                        [endpoints[1], endpoints[0]]
                    } else {
                        endpoints
                    };
                    for fps in [20, 120] {
                        let mut motion = GroundMotion::default();
                        let mut feet = start;
                        for _ in 0..fps * 2 {
                            feet = motion.step_with_dynamic(
                                &terrain,
                                Some(doors.collision_world()),
                                feet,
                                [(end[0] - start[0]) / 2., (end[1] - start[1]) / 2.],
                                false,
                                1. / fps as f32,
                            );
                        }
                        eprintln!(
                            "lift{id} open={open}, reverse={reverse}, fps={fps}: {start:?} -> {feet:?}; expected {end:?}"
                        );
                        for i in 0..3 {
                            near(feet[i], end[i]);
                        }
                    }
                }
            }
        }
    }
}
