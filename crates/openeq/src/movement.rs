//! Fixed-step live player motion. Positions are feet in EQ coordinates (Z up).
//! Presentation applies the eye offset; protocol serialization applies the
//! separate server center offset. Neither offset belongs in the physics state.

use crate::movement_rules::PlayerGravity;
use openeq_assets::collision::{CollisionWorld, PlayerMovePath};
use openeq_assets::liquid_regions::{LiquidRegions, LiquidSpan};

const STEP: f64 = 1.0 / 120.0;
// A roughly four-unit hop reaches its apex in a quarter second and lands in
// half a second. Increase impulse together with gravity: height alone should
// not make the player hang in the air longer.
const GRAVITY: f32 = 128.0;
const JUMP_SPEED: f32 = 32.0;
const RADIUS: f32 = 1.0;
const HEIGHT: f32 = 6.0;
const SWIM_SPEED_SCALE: f32 = 0.6;
const LEVITATION_FALL_SPEED: f32 = 2.;
// Opposing swim/walk input at a surface can keep changing direction without
// consuming representable time. Stop at the last safe point after this many
// transitions; never skip the remaining boundaries to spend the tick's time.
const MAX_MEDIUM_TRANSITIONS: usize = 16;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MotionMode {
    #[default]
    Ground,
    Swimming,
    Flying,
    Levitating,
    Floating,
}

pub struct MotionWorld<'a> {
    pub collision: &'a CollisionWorld,
    pub dynamic: Option<&'a CollisionWorld>,
    pub liquids: Option<&'a LiquidRegions>,
}

#[derive(Clone, Copy, Debug)]
pub struct MotionInput {
    /// Full-speed horizontal walking velocity, unaffected by look pitch.
    pub walk_velocity: [f32; 2],
    /// Full-speed 3D swim/fly direction, including pitch and up/down input.
    pub volume_velocity: [f32; 3],
    pub jump: bool,
    pub gravity: PlayerGravity,
}

impl MotionWorld<'_> {
    pub fn mode(&self, feet: [f32; 3], input: &MotionInput) -> MotionMode {
        match input.gravity {
            PlayerGravity::Flying => return MotionMode::Flying,
            PlayerGravity::Floating => return MotionMode::Floating,
            _ => {}
        }
        let center = [feet[0], feet[1], feet[2] + HEIGHT * 0.5];
        if self
            .liquids
            .is_some_and(|regions| regions.at(center).is_some())
        {
            return MotionMode::Swimming;
        }
        if input.gravity == PlayerGravity::Levitating
            || (input.gravity == PlayerGravity::LevitateWhileRunning
                && input.walk_velocity.iter().any(|v| v.abs() > 0.001))
        {
            MotionMode::Levitating
        } else {
            MotionMode::Ground
        }
    }

    fn grounded(&self, feet: [f32; 3]) -> bool {
        on_ground(self.collision, feet) || self.dynamic.is_some_and(|world| on_ground(world, feet))
    }

    fn move_velocity(
        &self,
        feet: [f32; 3],
        velocity: [f32; 3],
        dt: f32,
        grounded: bool,
        mode: MotionMode,
    ) -> ResolvedMove {
        let displacement = velocity.map(|value| value * dt);
        let step_height = step_height(grounded, velocity[2], mode);
        let resolved = self.collision.move_player_with_path(
            self.dynamic,
            feet,
            displacement,
            RADIUS,
            HEIGHT,
            step_height,
        );
        let moved = resolved.position;
        let desired_z = feet[2] + displacement[2];
        let straight = resolved.path == PlayerMovePath::Unchanged
            || (grounded && velocity[2] <= 0. && resolved.path == PlayerMovePath::FlatSupport);
        ResolvedMove {
            feet: moved,
            velocity_z: if (moved[2] - desired_z).abs() > 0.001 {
                0.
            } else {
                velocity[2]
            },
            straight,
        }
    }
}

// Stair following is grounded behavior. Giving an airborne body a two-unit
// step range snaps the last two units of a fall/jump.
fn step_height(grounded: bool, velocity_z: f32, mode: MotionMode) -> f32 {
    if grounded && velocity_z <= 0. && !matches!(mode, MotionMode::Flying | MotionMode::Floating) {
        2.
    } else {
        0.
    }
}

#[derive(Clone, Copy)]
struct ResolvedMove {
    feet: [f32; 3],
    velocity_z: f32,
    // Verified against every accepted collision substep, not just the endpoint.
    // Ascending support requires a separate lazy certificate during splitting.
    straight: bool,
}

struct MediumBoundary {
    fraction: f32,
    next_end: f32,
    wet: bool,
}

fn center(feet: [f32; 3]) -> [f32; 3] {
    [feet[0], feet[1], feet[2] + HEIGHT * 0.5]
}

/// Match collision's unobstructed substeps exactly, including f32 rounding.
/// A tolerance against a one-shot endpoint could misidentify a small slide as
/// straight motion. Oversized capped moves have no constant-speed time mapping.
fn unobstructed_position(feet: [f32; 3], delta: [f32; 3]) -> Option<[f32; 3]> {
    let delta = glam::Vec3::from(delta);
    let distance = delta.length();
    let step_length = (RADIUS * 0.5).min(1.);
    if !distance.is_finite() || distance > step_length * 256. {
        return None;
    }
    if distance <= 1e-7 {
        return Some(feet);
    }
    let steps = (distance / step_length).ceil().max(1.) as usize;
    let motion = delta / steps as f32;
    let mut position = glam::Vec3::from(feet);
    for _ in 0..steps {
        position += motion;
    }
    Some(position.to_array())
}

/// All supported liquids share the same movement response. A change of liquid
/// kind at a touching boundary is not a dry interval.
fn medium_boundary(spans: &[LiquidSpan], wet: bool) -> Option<MediumBoundary> {
    let starts_wet = spans.first().is_some_and(|span| span.enter == 0.);
    if starts_wet {
        let mut exit = spans[0].exit;
        let mut next = 1;
        while next < spans.len() && spans[next].enter <= exit {
            exit = exit.max(spans[next].exit);
            next += 1;
        }
        if !wet {
            return Some(MediumBoundary {
                fraction: 0.,
                next_end: exit,
                wet: true,
            });
        }
        (exit < 1.).then(|| MediumBoundary {
            fraction: exit,
            next_end: spans.get(next).map_or(1., |span| span.enter),
            wet: false,
        })
    } else {
        let enter = spans.first().map_or(1., |span| span.enter);
        if wet {
            Some(MediumBoundary {
                fraction: 0.,
                next_end: enter,
                wet: false,
            })
        } else {
            spans.first().map(|span| MediumBoundary {
                fraction: span.enter,
                next_end: span.exit,
                wet: true,
            })
        }
    }
}

/// Find a representable point on the outgoing side, not an arbitrary world
/// epsilon. WLD planes themselves are dry, while box boundaries are inclusive.
fn boundary_fraction(
    regions: &LiquidRegions,
    position: impl Fn(f32) -> Option<[f32; 3]>,
    boundary: &MediumBoundary,
) -> Option<f32> {
    let in_next_medium = |fraction| {
        position(fraction).map(|feet| regions.at(center(feet)).is_some() == boundary.wet)
    };
    let mut low = boundary.fraction;
    let mut high = low + (boundary.next_end - low) * 0.5;
    if high <= low || !in_next_medium(high)? {
        return None;
    }
    // f32 positions need at most 24 significand bits. Bisection only queries
    // metadata; the selected prefix still goes through normal body collision.
    for _ in 0..24 {
        let middle = low + (high - low) * 0.5;
        if middle == low || middle == high {
            break;
        }
        if in_next_medium(middle)? {
            high = middle;
        } else {
            low = middle;
        }
    }
    Some(high)
}

/// First representable nonnegative f32 fraction satisfying a monotone predicate.
/// Searching float bit order also covers transitions much closer to zero than
/// 24 arithmetic halvings can resolve. The caller establishes the bracket.
fn first_fraction(high: f32, predicate: impl Fn(f32) -> Option<bool>) -> Option<f32> {
    if !(0. ..=1.).contains(&high) || !predicate(high)? {
        return None;
    }
    if predicate(0.)? {
        return Some(0.);
    }
    let mut low = 0_u32;
    let mut high = high.to_bits();
    while high - low > 1 {
        let middle = low + (high - low) / 2;
        if predicate(f32::from_bits(middle))? {
            high = middle;
        } else {
            low = middle;
        }
    }
    Some(f32::from_bits(high))
}

/// The endpoint's rounded distance is not the requested distance. Invert the
/// actual single-axis prefix positions to reach the interior of the next span,
/// then find its first outgoing representable time from a verified old medium.
/// Height invariance is established separately before this function is called.
fn support_boundary_fraction(
    regions: &LiquidRegions,
    from: [f32; 3],
    to: [f32; 3],
    position: impl Fn(f32) -> Option<[f32; 3]>,
    boundary: &MediumBoundary,
) -> Option<f32> {
    let axis = usize::from(from[0] == to[0]);
    let start = f64::from(from[axis]);
    let extent = f64::from(to[axis]) - start;
    if extent == 0. || !extent.is_finite() {
        return None;
    }
    let target = (f64::from(boundary.fraction) + f64::from(boundary.next_end)) * 0.5;
    let progress =
        |fraction| position(fraction).map(|point| (f64::from(point[axis]) - start) / extent);
    let high = first_fraction(1., |fraction| Some(progress(fraction)? >= target))?;
    let actual_progress = progress(high)?;
    // A rounded position may skip the entire span or reach a later one of the
    // same kind. Do not authorize that jump merely by checking wet/dry state.
    if actual_progress < f64::from(boundary.fraction)
        || actual_progress >= f64::from(boundary.next_end)
    {
        return None;
    }
    first_fraction(high, |fraction| {
        Some(regions.at(center(position(fraction)?)).is_some() == boundary.wet)
    })
}

/// Corrects a small floor penetration once when installing an authoritative
/// spawn. Some EQEmu safe points place the server center too low for our body
/// height. Keep normal airborne arrivals unchanged, and require full clearance
/// before raising feet by at most the ordinary two-unit step height.
pub fn recover_spawn(world: &CollisionWorld, feet: [f32; 3]) -> [f32; 3] {
    world
        .recover_player_from_floor(feet, RADIUS, HEIGHT, 2.)
        .unwrap_or(feet)
}

#[derive(Clone, Debug, Default)]
pub struct GroundMotion {
    pub velocity_z: f32,
    pub mode: MotionMode,
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
        feet: [f32; 3],
        velocity_xy: [f32; 2],
        jump: bool,
        elapsed: f32,
    ) -> [f32; 3] {
        self.step_in_world(
            MotionWorld {
                collision: world,
                dynamic,
                liquids: None,
            },
            feet,
            MotionInput {
                walk_velocity: velocity_xy,
                volume_velocity: [velocity_xy[0], velocity_xy[1], 0.],
                jump,
                gravity: PlayerGravity::Grounded,
            },
            elapsed,
        )
    }

    /// Samples the authored medium every physics tick, so entry/exit and
    /// gravity changes behave equally at low and high rendering frame rates.
    /// Swimming and server flight still use the normal body collision solver.
    /// Swim speed and levitation fall speed are explicitly client tuning;
    /// health, buffs, breath damage and other outcomes remain server-owned.
    pub fn step_in_world(
        &mut self,
        world: MotionWorld<'_>,
        mut feet: [f32; 3],
        mut input: MotionInput,
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
        self.jump_pending |= input.jump;
        if elapsed.is_finite() && elapsed > 0.0 {
            self.accumulator += f64::from(elapsed.min(0.25));
        }
        let finite = |value: f32| if value.is_finite() { value } else { 0.0 };
        input.walk_velocity = input.walk_velocity.map(finite);
        input.volume_velocity = input.volume_velocity.map(finite);
        let dt = STEP as f32;
        // The tiny tolerance absorbs f32 render-clock rounding, not missing
        // simulation time. Keeping the accumulator in f64 avoids further drift.
        while self.accumulator + 1e-7 >= STEP {
            self.accumulator = (self.accumulator - STEP).max(0.0);
            self.mode = world.mode(feet, &input);
            let grounded = world.grounded(feet);
            if grounded && self.velocity_z < 0.0 {
                self.velocity_z = 0.0;
            }
            let jump = std::mem::take(&mut self.jump_pending);
            if jump && grounded && matches!(self.mode, MotionMode::Ground | MotionMode::Levitating)
            {
                self.velocity_z = JUMP_SPEED;
            }
            let velocity_xy = match self.mode {
                MotionMode::Swimming | MotionMode::Flying => {
                    let scale = if self.mode == MotionMode::Swimming {
                        SWIM_SPEED_SCALE
                    } else {
                        1.
                    };
                    self.velocity_z = input.volume_velocity[2] * scale;
                    [
                        input.volume_velocity[0] * scale,
                        input.volume_velocity[1] * scale,
                    ]
                }
                MotionMode::Floating => {
                    self.velocity_z = 0.;
                    input.walk_velocity
                }
                MotionMode::Ground | MotionMode::Levitating => {
                    let terminal = if self.mode == MotionMode::Levitating {
                        LEVITATION_FALL_SPEED
                    } else {
                        80.
                    };
                    self.velocity_z = (self.velocity_z - GRAVITY * dt).max(-terminal);
                    input.walk_velocity
                }
            };
            let velocity = [velocity_xy[0], velocity_xy[1], self.velocity_z];
            let moved = world.move_velocity(feet, velocity, dt, grounded, self.mode);
            if let Some((split, mode)) = self.cross_liquids(&world, feet, &input, velocity, moved) {
                feet = split.feet;
                self.velocity_z = split.velocity_z;
                self.mode = mode;
            } else {
                feet = moved.feet;
                self.velocity_z = moved.velocity_z;
            }
        }
        feet
    }

    /// Split straight motion or a certified single ascending support plane.
    /// General stair/slide/descending responses retain the original whole tick;
    /// their corrected endpoints do not establish timed travel.
    fn cross_liquids(
        &self,
        world: &MotionWorld<'_>,
        mut feet: [f32; 3],
        input: &MotionInput,
        mut velocity: [f32; 3],
        mut moved: ResolvedMove,
    ) -> Option<(ResolvedMove, MotionMode)> {
        let regions = world.liquids.filter(|regions| !regions.is_empty())?;
        if matches!(self.mode, MotionMode::Flying | MotionMode::Floating) {
            return None;
        }
        let mut mode = self.mode;
        let mut remaining = STEP as f32;
        for transition in 0..MAX_MEDIUM_TRANSITIONS {
            let spans = regions.segment(center(feet), center(moved.feet));
            // With no first boundary candidate, the saved whole-tick solve is
            // already the fallback. Avoid extra swept geometry work on ordinary
            // dry ramps and fully submerged travel.
            if transition == 0
                && !moved.straight
                && medium_boundary(&spans, mode == MotionMode::Swimming).is_none()
            {
                return None;
            }
            let support = if !moved.straight {
                let grounded = world.grounded(feet);
                if !grounded || velocity[2] > 0. || (velocity[0] != 0. && velocity[1] != 0.) {
                    return None;
                }
                let displacement = velocity.map(|value| value * remaining);
                let support = world.collision.certify_ascending_support(
                    world.dynamic,
                    feet,
                    displacement,
                    RADIUS,
                    HEIGHT,
                    step_height(grounded, velocity[2], mode),
                )?;
                if support.position_for_delta(displacement)? != moved.feet {
                    return None;
                }
                // Rounded source-plane height can form a staircase that leaves
                // the endpoint chord. Only discover intervals from XY when all
                // liquid membership in the swept center box is independent of Z.
                // One horizontal axis also excludes rounded diagonal corner cuts.
                let start = center(feet);
                let end = center(moved.feet);
                if !regions.height_invariant_in_bounds(
                    std::array::from_fn(|axis| start[axis].min(end[axis])),
                    std::array::from_fn(|axis| start[axis].max(end[axis])),
                ) {
                    return None;
                }
                Some(support)
            } else {
                None
            };
            if moved.feet == feet {
                return Some((moved, mode));
            }
            let Some(boundary) = medium_boundary(&spans, mode == MotionMode::Swimming) else {
                return Some((moved, mode));
            };
            let flat = moved.feet[2] == feet[2] && velocity[2] <= 0.;
            let fraction = if let Some(support) = &support {
                support_boundary_fraction(
                    regions,
                    feet,
                    moved.feet,
                    |fraction| {
                        support.position_for_delta(
                            velocity.map(|value| value * (remaining * fraction)),
                        )
                    },
                    &boundary,
                )?
            } else {
                boundary_fraction(
                    regions,
                    |fraction| {
                        let dt = remaining * fraction;
                        let mut desired =
                            unobstructed_position(feet, velocity.map(|value| value * dt))?;
                        if flat {
                            desired[2] = feet[2];
                        }
                        Some(desired)
                    },
                    &boundary,
                )?
            };
            let dt = remaining * fraction;
            let prefix = world.move_velocity(feet, velocity, dt, world.grounded(feet), mode);
            // Re-query the real collision result. A boundary candidate alone
            // cannot authorize a mode switch on a blocked or snapped prefix.
            let verified_prefix = if let Some(support) = &support {
                support.position_for_delta(velocity.map(|value| value * dt))? == prefix.feet
            } else {
                prefix.straight
            };
            if !verified_prefix || regions.at(center(prefix.feet)).is_some() != boundary.wet {
                return None;
            }
            feet = prefix.feet;
            remaining = (remaining - dt).max(0.);
            mode = world.mode(feet, input);
            velocity = if mode == MotionMode::Swimming {
                input.volume_velocity.map(|value| value * SWIM_SPEED_SCALE)
            } else {
                // Gravity/jump impulses occur once per fixed tick, as before.
                // Leaving water carries its vertical momentum until that next
                // impulse; entering water immediately takes swim direction.
                [
                    input.walk_velocity[0],
                    input.walk_velocity[1],
                    prefix.velocity_z,
                ]
            };
            if remaining == 0. {
                return Some((prefix, mode));
            }
            moved = world.move_velocity(feet, velocity, remaining, world.grounded(feet), mode);
        }
        Some((
            ResolvedMove {
                feet,
                velocity_z: 0.,
                straight: true,
            },
            mode,
        ))
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

    fn water(center: [f32; 3], half_extents: [f32; 3]) -> LiquidRegions {
        use openeq_assets::liquid_regions::{LiquidBox, LiquidKind};
        LiquidRegions::from_boxes([LiquidBox {
            kind: LiquidKind::Water,
            center,
            half_extents,
            rotation: [0., 0., 0., 1.],
        }])
        .unwrap()
    }

    fn input(velocity: [f32; 3], gravity: PlayerGravity) -> MotionInput {
        MotionInput {
            walk_velocity: [velocity[0], velocity[1]],
            volume_velocity: velocity,
            jump: false,
            gravity,
        }
    }

    fn advance(
        motion: &mut GroundMotion,
        collision: &CollisionWorld,
        liquids: &LiquidRegions,
        feet: [f32; 3],
        input: MotionInput,
        dt: f32,
    ) -> [f32; 3] {
        motion.step_in_world(
            MotionWorld {
                collision,
                liquids: Some(liquids),
                dynamic: None,
            },
            feet,
            input,
            dt,
        )
    }

    #[test]
    fn land_water_land_crossing_preserves_time_at_low_and_high_fps() {
        let collision = flat();
        let liquid = water([20., 0., 10.], [10., 100., 10.]);
        let mut results = Vec::new();
        for fps in [10, 30, 60, 120, 240] {
            let mut motion = GroundMotion::default();
            let mut feet = [0.; 3];
            let mut swam = false;
            for _ in 0..fps * 2 {
                feet = advance(
                    &mut motion,
                    &collision,
                    &liquid,
                    feet,
                    input([40., 0., 0.], PlayerGravity::Grounded),
                    1. / fps as f32,
                );
                swam |= motion.mode == MotionMode::Swimming;
            }
            assert!(swam);
            assert_eq!(motion.mode, MotionMode::Ground);
            assert!((feet[0] - 66.67).abs() < 0.4, "{fps}fps: {feet:?}");
            near(feet[2], 0.);
            results.push(feet);
        }
        for feet in &results[1..] {
            near(feet[0], results[0][0]);
        }
    }

    #[test]
    fn thin_water_between_dry_endpoints_consumes_swimming_time() {
        let collision = flat();
        let liquid = water([0.15, 0., 3.], [0.05, 10., 3.]);
        let mut motion = GroundMotion::default();
        let feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0.; 3],
            input([40., 0., 0.], PlayerGravity::Grounded),
            1. / 120.,
        );
        assert!(liquid.at([0., 0., 3.]).is_none());
        assert!(liquid.at(center(feet)).is_none());
        // .1 units at 40/s, .1 at 24/s, then the remaining time at 40/s.
        let expected = 0.2 + 40. * (1. / 120. - 0.1 / 40. - 0.1 / 24.);
        assert!((feet[0] - expected).abs() < 0.00001, "{feet:?}");
        assert_eq!(feet[2], 0.);
        assert_eq!(motion.mode, MotionMode::Ground);
        let mut endpoints = Vec::new();
        for fps in [10, 30, 60, 120, 240] {
            let mut motion = GroundMotion::default();
            let mut feet = [0.; 3];
            for _ in 0..fps {
                feet = advance(
                    &mut motion,
                    &collision,
                    &liquid,
                    feet,
                    input([40., 0., 0.], PlayerGravity::Grounded),
                    1. / fps as f32,
                );
            }
            endpoints.push(feet);
        }
        for feet in endpoints {
            near(feet[0], 40. - 40. * (0.1 / 24. - 0.1 / 40.));
            assert_eq!(feet[2], 0.);
        }
    }

    #[test]
    fn entering_thin_water_uses_swim_direction_for_remaining_tick() {
        let collision = flat();
        let liquid = water([0.15, 0., 10.], [0.05, 10., 10.]);
        let mut motion = GroundMotion::default();
        let feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0., 0., 5.],
            MotionInput {
                walk_velocity: [40., 0.],
                volume_velocity: [0., 20., 20.],
                jump: false,
                gravity: PlayerGravity::Grounded,
            },
            1. / 120.,
        );
        assert!((feet[0] - 0.1).abs() < 0.00001, "{feet:?}");
        let dry_time = 0.1 / 40.;
        let swim_time = 1. / 120. - dry_time;
        assert!((feet[1] - 12. * swim_time).abs() < 0.00001);
        let expected_z = 5. - GRAVITY / 120. * dry_time + 12. * swim_time;
        assert!((feet[2] - expected_z).abs() < 0.00001, "{feet:?}");
        assert_eq!(motion.mode, MotionMode::Swimming);
        assert_eq!(motion.velocity_z, 12.);
    }

    #[test]
    fn falling_through_a_thin_water_layer_stops_at_immersion() {
        let collision = flat();
        let liquid = water([0., 0., 12.45], [10., 10., 0.05]);
        let mut motion = GroundMotion {
            velocity_z: -80.,
            ..Default::default()
        };
        let feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0., 0., 10.],
            input([0.; 3], PlayerGravity::Grounded),
            1. / 120.,
        );
        assert!((feet[2] - 9.5).abs() < 0.00001, "{feet:?}");
        assert!(liquid.at(center(feet)).is_some());
        assert_eq!(motion.mode, MotionMode::Swimming);
        assert_eq!(motion.velocity_z, 0.);
    }

    #[test]
    fn liquid_queries_preserve_dry_jumping_and_fully_submerged_motion() {
        let collision = flat();
        let far_water = water([100., 100., 10.], [1.; 3]);
        let mut plain = GroundMotion::default();
        let mut queried = GroundMotion::default();
        let (mut a, mut b) = ([0.; 3], [0.; 3]);
        for tick in 0..120 {
            a = plain.step(&collision, a, [40., 0.], tick == 0, 1. / 120.);
            let mut keys = input([40., 0., 0.], PlayerGravity::Grounded);
            keys.jump = tick == 0;
            b = advance(&mut queried, &collision, &far_water, b, keys, 1. / 120.);
            assert_eq!(a, b);
            assert_eq!(plain.velocity_z, queried.velocity_z);
        }
        let liquid = water([0., 0., 50.], [100.; 3]);
        let start = [0., 0., 20.];
        for velocity in [[40., 0., 0.], [20., 0., 20.], [0., 0., -40.]] {
            let expected = collision.move_player(
                start,
                velocity.map(|value| value * SWIM_SPEED_SCALE * STEP as f32),
                RADIUS,
                HEIGHT,
                0.,
            );
            let actual = advance(
                &mut GroundMotion::default(),
                &collision,
                &liquid,
                start,
                input(velocity, PlayerGravity::Grounded),
                STEP as f32,
            );
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn matching_endpoints_do_not_authorize_a_liquid_chord_over_a_step() {
        let collision = world(&[
            [
                [-50., -50., 0.],
                [50., -50., 0.],
                [50., 50., 0.],
                [-50., 50., 0.],
            ],
            [[3., -20., 1.], [4., -20., 1.], [4., 20., 1.], [3., 20., 1.]],
            [[3., -20., 0.], [3., 20., 0.], [3., 20., 1.], [3., -20., 1.]],
            [[4., -20., 0.], [4., 20., 0.], [4., 20., 1.], [4., -20., 1.]],
        ]);
        let liquid = water([3.5, 0., 3.], [0.1, 10., 0.1]);
        let world = MotionWorld {
            collision: &collision,
            dynamic: None,
            liquids: Some(&liquid),
        };
        // A deliberately fast request crosses an entire step in one fixed
        // tick. Its endpoint looks like flat walking; its accepted substeps
        // rise above the thin water and descend again.
        let velocity = [960., 0., -GRAVITY * STEP as f32];
        let moved = world.move_velocity([0.; 3], velocity, STEP as f32, true, MotionMode::Ground);
        let old_chord = unobstructed_position([0.; 3], velocity.map(|v| v * STEP as f32)).unwrap();
        assert_eq!(moved.feet[..2], old_chord[..2]);
        assert_eq!(moved.feet[2], 0.);
        assert!(
            !liquid
                .segment(center([0.; 3]), center(moved.feet))
                .is_empty()
        );
        assert!(
            !moved.straight,
            "the collision route rose above this endpoint chord"
        );
        let prefix = world.move_velocity(
            [0.; 3],
            velocity,
            3.5 / velocity[0],
            true,
            MotionMode::Ground,
        );
        assert_eq!(prefix.feet[2], 1.);
        assert!(liquid.at(center(prefix.feet)).is_none());
    }

    #[test]
    fn deflected_liquid_crossings_keep_original_collision_result() {
        let ramp = world(&[[
            [-100., -100., -50.],
            [100., -100., 50.],
            [100., 100., 50.],
            [-100., 100., -50.],
        ]]);
        let liquid = water([0.15, 0., 3.], [0.05, 10., 3.]);
        let expected = GroundMotion::default().step(&ramp, [0.; 3], [40., 0.], false, STEP as f32);
        let actual = advance(
            &mut GroundMotion::default(),
            &ramp,
            &liquid,
            [0.; 3],
            input([40., 0., 0.], PlayerGravity::Grounded),
            STEP as f32,
        );
        assert_eq!(actual, expected);
        assert!(actual[2] > 0.1);
        let floor = flat();
        let wall = world(&[[
            [1.1, -20., 0.],
            [1.1, 20., 0.],
            [1.1, 20., 20.],
            [1.1, -20., 20.],
        ]]);
        // The body touches the wall at x=.1, before the liquid starts at .15.
        let water_behind_wall = water([0.2, 0., 3.], [0.05, 10., 3.]);
        for velocity in [[40., 0., 0.], [40., 40., 0.]] {
            let expected = GroundMotion::default().step_with_dynamic(
                &floor,
                Some(&wall),
                [0.; 3],
                [velocity[0], velocity[1]],
                false,
                STEP as f32,
            );
            let mut motion = GroundMotion::default();
            let actual = motion.step_in_world(
                MotionWorld {
                    collision: &floor,
                    dynamic: Some(&wall),
                    liquids: Some(&water_behind_wall),
                },
                [0.; 3],
                input(velocity, PlayerGravity::Grounded),
                STEP as f32,
            );
            assert_eq!(actual, expected);
            assert!(actual[0] < 0.15);
            assert_eq!(motion.mode, MotionMode::Ground);
        }
    }

    #[test]
    fn opposing_medium_directions_stop_at_boundary_with_bounded_work() {
        let collision = flat();
        let liquid = water([0.15, 0., 3.], [0.05, 10., 3.]);
        let mut motion = GroundMotion::default();
        let feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0.; 3],
            MotionInput {
                walk_velocity: [40., 0.],
                volume_velocity: [-40., 0., 0.],
                jump: false,
                gravity: PlayerGravity::Grounded,
            },
            STEP as f32,
        );
        assert!((feet[0] - 0.1).abs() < 0.00001, "{feet:?}");
        assert_eq!(motion.velocity_z, 0.);
        assert!(motion.accumulator < 1e-7);
    }

    #[test]
    fn too_many_thin_regions_stop_at_last_processed_boundary() {
        use openeq_assets::liquid_regions::{LiquidBox, LiquidKind};
        let collision = flat();
        let liquids = LiquidRegions::from_boxes((0..20).map(|index| LiquidBox {
            kind: LiquidKind::Water,
            center: [0.0105 + index as f32 * 0.003, 0., 3.],
            half_extents: [0.0005, 10., 3.],
            rotation: [0., 0., 0., 1.],
        }))
        .unwrap();
        let mut motion = GroundMotion::default();
        let feet = advance(
            &mut motion,
            &collision,
            &liquids,
            [0.; 3],
            input([40., 0., 0.], PlayerGravity::Grounded),
            STEP as f32,
        );
        // Sixteen transitions traverse eight slabs, ending at x=.032. The
        // unprocessed intervals must not be skipped to reach the dry endpoint.
        assert!((feet[0] - 0.032).abs() < 0.00001, "{feet:?}");
        assert_eq!(motion.mode, MotionMode::Ground);
        assert!(motion.accumulator < 1e-7);
    }

    #[test]
    fn swimming_holds_depth_and_can_ascend_descend_and_exit_surface() {
        let collision = flat();
        let liquid = water([0., 0., 10.], [100., 100., 10.]);
        let mut motion = GroundMotion {
            velocity_z: -80.,
            ..Default::default()
        };
        let mut feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0., 0., 10.],
            input([0.; 3], PlayerGravity::Grounded),
            0.25,
        );
        near(feet[2], 10.);
        feet = advance(
            &mut motion,
            &collision,
            &liquid,
            feet,
            input([0., 0., -40.], PlayerGravity::Grounded),
            0.25,
        );
        near(feet[2], 4.);
        feet = advance(
            &mut motion,
            &collision,
            &liquid,
            feet,
            input([0., 0., 40.], PlayerGravity::Grounded),
            0.25,
        );
        near(feet[2], 10.);
        let mut highest = feet[2];
        let mut surfaced = false;
        for _ in 0..120 {
            feet = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([0., 0., 40.], PlayerGravity::Grounded),
                1. / 120.,
            );
            highest = highest.max(feet[2]);
            surfaced |= motion.mode == MotionMode::Ground;
        }
        assert!(
            surfaced && highest > 18. && highest < 21.,
            "surface exit apex {highest}"
        );
    }

    #[test]
    fn swimming_cannot_pass_through_pool_floor_ceiling_or_wall() {
        let collision = world(&[
            [
                [-100., -100., 0.],
                [100., -100., 0.],
                [100., 100., 0.],
                [-100., 100., 0.],
            ],
            [
                [-100., -100., 50.],
                [-100., 100., 50.],
                [100., 100., 50.],
                [100., -100., 50.],
            ],
            [
                [-100., 10., 0.],
                [100., 10., 0.],
                [100., 10., 50.],
                [-100., 10., 50.],
            ],
        ]);
        let liquid = water([0., 0., 50.], [100., 100., 50.]);
        for (velocity, axis, expected) in [
            ([0., 0., -40.], 2, 0.),
            ([0., 0., 40.], 2, 44.),
            ([0., 40., 0.], 1, 9.),
        ] {
            let mut motion = GroundMotion::default();
            let mut feet = [0., 0., 20.];
            for _ in 0..20 {
                feet = advance(
                    &mut motion,
                    &collision,
                    &liquid,
                    feet,
                    input(velocity, PlayerGravity::Grounded),
                    0.1,
                );
            }
            near(feet[axis], expected);
        }
    }

    #[test]
    fn swimming_can_walk_out_up_a_shore_ramp() {
        let collision = world(&[
            [
                [-100., -100., 0.],
                [0., -100., 0.],
                [0., 100., 0.],
                [-100., 100., 0.],
            ],
            [
                [0., -100., 0.],
                [20., -100., 10.],
                [20., 100., 10.],
                [0., 100., 0.],
            ],
            [
                [20., -100., 10.],
                [100., -100., 10.],
                [100., 100., 10.],
                [20., 100., 10.],
            ],
        ]);
        let liquid = water([0., 0., 4.], [100., 100., 4.]);
        let mut motion = GroundMotion::default();
        let mut feet = [-5., 0., 0.];
        for _ in 0..120 {
            feet = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([40., 0., 0.], PlayerGravity::Grounded),
                1. / 120.,
            );
        }
        assert!(feet[0] > 20., "failed shore exit: {feet:?}");
        near(feet[2], 10.);
        assert_eq!(motion.mode, MotionMode::Ground);
    }

    #[test]
    fn underwater_dynamic_door_blocks_swimming_until_opened() {
        let collision = flat();
        let closed = world(&[[
            [-20., 10., 0.],
            [20., 10., 0.],
            [20., 10., 50.],
            [-20., 10., 50.],
        ]]);
        let liquid = water([0., 0., 25.], [100., 100., 25.]);
        let mut motion = GroundMotion::default();
        let mut feet = [0., 0., 10.];
        for _ in 0..120 {
            feet = motion.step_in_world(
                MotionWorld {
                    collision: &collision,
                    dynamic: Some(&closed),
                    liquids: Some(&liquid),
                },
                feet,
                input([0., 40., 0.], PlayerGravity::Grounded),
                1. / 120.,
            );
        }
        near(feet[1], 9.);
        near(feet[2], 10.);
        for _ in 0..120 {
            feet = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([0., 40., 0.], PlayerGravity::Grounded),
                1. / 120.,
            );
        }
        near(feet[1], 33.);
        near(feet[2], 10.);
    }

    #[test]
    #[ignore = "requires original Plane of Knowledge assets"]
    fn authored_pok_pool_supports_swimming_without_sinking_or_leaving_its_bounds() {
        let base = openeq_assets::loader::default_client_dir().expect("original assets");
        let scene = openeq_assets::loader::load_zone(&base, "poknowledge").unwrap();
        let collision = CollisionWorld::build(&scene);
        let liquid = LiquidRegions::load(&base, "poknowledge").unwrap();
        let start = [15., 1455., -134.];
        let mut motion = GroundMotion::default();
        let mut feet = start;
        for _ in 0..120 {
            feet = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([0.; 3], PlayerGravity::Grounded),
                1. / 120.,
            );
        }
        near(feet[2], start[2]);
        assert_eq!(motion.mode, MotionMode::Swimming);
        for _ in 0..60 {
            feet = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([-40., 0., 0.], PlayerGravity::Grounded),
                1. / 120.,
            );
        }
        near(feet[0], 3.);
        near(feet[2], start[2]);
        assert!(liquid.at([feet[0], feet[1], feet[2] + 6.]).is_some());
        let mut surfaced = false;
        for _ in 0..120 {
            feet = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([0., 0., 40.], PlayerGravity::Grounded),
                1. / 120.,
            );
            surfaced |= liquid.at([feet[0], feet[1], feet[2] + 6.]).is_none();
        }
        assert!(
            surfaced,
            "could not raise eyes above the authored surface: {feet:?}"
        );
        assert!(
            feet[2] >= start[2] && feet[2] < -124.,
            "invalid pool position: {feet:?}"
        );
    }

    #[test]
    fn server_flight_and_float_override_water_and_end_when_removed() {
        let collision = flat();
        let liquid = water([0., 0., 50.], [100., 100., 50.]);
        for gravity in [PlayerGravity::Flying, PlayerGravity::Floating] {
            let mut motion = GroundMotion::default();
            let feet = advance(
                &mut motion,
                &collision,
                &liquid,
                [0., 0., 20.],
                input([0., 0., 40.], gravity),
                0.25,
            );
            near(
                feet[2],
                if gravity == PlayerGravity::Flying {
                    30.
                } else {
                    20.
                },
            );
            let stayed = advance(
                &mut motion,
                &collision,
                &liquid,
                feet,
                input([0.; 3], PlayerGravity::Grounded),
                0.25,
            );
            near(stayed[2], feet[2]);
            assert_eq!(motion.mode, MotionMode::Swimming);
            let fell = advance(
                &mut motion,
                &collision,
                &LiquidRegions::default(),
                stayed,
                input([0.; 3], PlayerGravity::Grounded),
                0.25,
            );
            assert!(fell[2] < stayed[2] - 3.);
        }
    }

    #[test]
    fn levitation_slows_falling_but_does_not_inflate_jumps() {
        let collision = flat();
        let liquid = LiquidRegions::default();
        let mut motion = GroundMotion::default();
        let feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0., 0., 20.],
            input([0.; 3], PlayerGravity::Levitating),
            0.25,
        );
        assert!(
            (19.49..19.52).contains(&feet[2]),
            "levitation drift: {feet:?}"
        );
        let fell = advance(
            &mut motion,
            &collision,
            &liquid,
            feet,
            input([0.; 3], PlayerGravity::Grounded),
            0.25,
        );
        assert!(fell[2] < feet[2] - 4.);
        let mut motion = GroundMotion::default();
        let mut feet = [0.; 3];
        let mut apex = 0_f32;
        for tick in 0..120 {
            let mut keys = input([0.; 3], PlayerGravity::Levitating);
            keys.jump = tick == 0;
            feet = advance(&mut motion, &collision, &liquid, feet, keys, 1. / 120.);
            apex = apex.max(feet[2]);
        }
        assert!((3.7..4.1).contains(&apex), "levitation jump apex {apex}");
        assert!(feet[2] > 2., "levitation fall too fast: {feet:?}");
    }

    #[test]
    fn running_only_levitation_stops_when_player_stops() {
        let collision = flat();
        let liquid = LiquidRegions::default();
        let mut motion = GroundMotion::default();
        let feet = advance(
            &mut motion,
            &collision,
            &liquid,
            [0., 0., 20.],
            input([40., 0., 0.], PlayerGravity::LevitateWhileRunning),
            0.25,
        );
        assert_eq!(motion.mode, MotionMode::Levitating);
        let fell = advance(
            &mut motion,
            &collision,
            &liquid,
            feet,
            input([0.; 3], PlayerGravity::LevitateWhileRunning),
            0.25,
        );
        assert_eq!(motion.mode, MotionMode::Ground);
        assert!(fell[2] < feet[2] - 4.);
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

#[cfg(test)]
#[path = "movement_planar_tests.rs"]
mod planar_tests;
