//! Optional proof of a narrow family of support-projected movement requests.
//! This does not change collision solving or the existing path classification.

use super::{CollisionQuery, CollisionWorld, PlayerMove, SKIN, Triangle, cross};
use glam::{Vec2, Vec3};

/// A single-substep request whose component-bounded prefixes follow one
/// ascending support triangle. The worlds are borrowed to retain the exact
/// static/dynamic snapshot used by the proof.
///
/// This establishes a family of collision responses, not native-client physics
/// or a continuous contact trace. The caller supplies its own integration-time
/// mapping and must preserve the multiplication order used to obtain a prefix
/// displacement. No corrected-endpoint interpolation is involved.
pub struct AscendingSupport<'a> {
    query: CollisionQuery<'a>,
    triangle: &'a Triangle,
    start: Vec3,
    delta: Vec3,
    radius: f32,
    height: f32,
    max_step: f32,
}

impl AscendingSupport<'_> {
    /// Evaluate the original source plane at the exact requested prefix XY.
    /// Pass, for example, `velocity * (tick_time * fraction)`, preserving the
    /// caller's arithmetic, rather than scaling an already rounded full delta.
    ///
    /// Each component must be between zero and the corresponding original
    /// delta. The proof covers the resulting whole XY rectangle, including
    /// rounding differences between independently scaled vector components.
    /// Very short requests retain the existing solver's no-motion cutoff.
    pub fn position_for_delta(&self, delta: [f32; 3]) -> Option<[f32; 3]> {
        let delta = Vec3::from(delta);
        if !delta.is_finite()
            || delta
                .to_array()
                .into_iter()
                .zip(self.delta.to_array())
                .any(|(prefix, full)| prefix < full.min(0.) || prefix > full.max(0.))
        {
            return None;
        }
        if delta.length() <= 1e-7 {
            return Some(self.start.to_array());
        }
        let xy = (self.start + delta).truncate();
        let z = self.triangle.plane_z(xy)?;
        z.is_finite().then_some([xy.x, xy.y, z])
    }

    /// Re-solve a prefix against the borrowed geometry and require bit-exact
    /// agreement with the certified source-plane evaluation. Existing callers
    /// can instead compare their own collision result with `position_for_delta`.
    pub fn resolve_prefix(&self, delta: [f32; 3]) -> Option<PlayerMove> {
        let expected = self.position_for_delta(delta)?;
        let solved = self.query.world.move_player_with_path(
            self.query.dynamic,
            self.start.to_array(),
            delta,
            self.radius,
            self.height,
            self.max_step,
        );
        (solved.position.map(f32::to_bits) == expected.map(f32::to_bits)).then_some(solved)
    }
}

impl CollisionWorld {
    /// Lazily certify ascending travel on one isolated support triangle.
    /// Ordinary movement does not perform this additional geometry query.
    ///
    /// Only one uncapped substep is admitted, with nonpositive requested Z,
    /// exact initial plane support, and nondecreasing height on each requested
    /// XY axis. The complete swept footprint must remain strictly inside the
    /// triangle. Both worlds are checked over the full step/drop support-query
    /// envelope as well as body clearance. Extra geometry in the swept spatial
    /// buckets may conservatively reject a request even when it is harmless.
    /// Flat paths, descents, edges, slides, steps, and uncertain arithmetic are
    /// left to the existing movement result and classification.
    pub fn certify_ascending_support<'a>(
        &'a self,
        dynamic: Option<&'a Self>,
        position: [f32; 3],
        delta: [f32; 3],
        radius: f32,
        height: f32,
        max_step: f32,
    ) -> Option<AscendingSupport<'a>> {
        let start = Vec3::from(position);
        let delta = Vec3::from(delta);
        if !start.is_finite()
            || !delta.is_finite()
            || ![radius, height, max_step].iter().all(|n| n.is_finite())
            || radius <= 0.
            || height <= SKIN * 2.
            || delta.z > 0.
        {
            return None;
        }
        // Match the existing solver's clamps and its 3D substep calculation.
        let radius = radius.max(SKIN * 2.);
        let max_step = max_step.max(0.).min(height * 0.5);
        let distance = delta.length();
        let step_length = (radius * 0.5).min(1.);
        if !distance.is_finite() || distance <= 1e-7 || distance > step_length {
            return None;
        }
        let end_xy = (start + delta).truncate();
        let centers = [
            Bounds::new(start.x.min(end_xy.x), start.x.max(end_xy.x))?,
            Bounds::new(start.y.min(end_xy.y), start.y.max(end_xy.y))?,
        ];
        let padded_radius = radius + SKIN;
        let footprint = centers.map(|axis| {
            Bounds::new(
                (axis.low - padded_radius).next_down(),
                (axis.high + padded_radius).next_up(),
            )
        });
        let [Some(x), Some(y)] = footprint else {
            return None;
        };
        let max_drop = (max_step * 1.5).max(delta.z.abs() + SKIN);
        let support_low = (start.z - max_drop - SKIN).next_down();
        let support_high = (start.z + max_step + SKIN).next_up();
        let body_high = (support_high + height + SKIN).next_up();
        if ![support_low, support_high, body_high]
            .iter()
            .all(|value| value.is_finite())
        {
            return None;
        }
        let query = CollisionQuery {
            world: self,
            dynamic,
        };
        let mut source = None;
        for triangle in query.candidates(Vec2::new(x.low, y.low), Vec2::new(x.high, y.high)) {
            // The support query reaches below the body. Also, a poorly
            // conditioned plane may evaluate beyond its vertex Z bounds. Only
            // discard vertically distant geometry if its computed plane cannot
            // compete for support anywhere in the complete center rectangle.
            let body_clear = triangle.max.z < support_low || triangle.min.z > body_high;
            let support_clear = !triangle.walkable()
                || plane_bounds(triangle, centers)
                    .is_some_and(|z| z.high < support_low || z.low > support_high);
            if body_clear && support_clear {
                continue;
            }
            if source.replace(triangle).is_some() {
                return None;
            }
        }
        let triangle = source?;
        if !triangle.walkable() || !contains_footprint(triangle, [x, y]) {
            return None;
        }
        // Each operation in plane_z is monotone when both axis contributions
        // have this sign. This proves all independently rounded prefix XYs
        // remain between the exact source-plane endpoint heights. Mixed uphill
        // and downhill contributions are deliberately excluded.
        for (normal, displacement) in triangle
            .normal
            .truncate()
            .to_array()
            .into_iter()
            .zip(delta.truncate().to_array())
        {
            let rise = -(normal as f64) * displacement as f64 / triangle.normal.z as f64;
            if !rise.is_finite() || rise < 0. {
                return None;
            }
        }
        let start_z = triangle.plane_z(start.truncate())?;
        let end_z = triangle.plane_z(end_xy)?;
        let support_limit = start.z + max_step + SKIN;
        if !start_z.is_finite()
            || !end_z.is_finite()
            || start_z.to_bits() != start.z.to_bits()
            || end_z <= start.z
            // Retain a full skin of clearance from the support-rise cutoff.
            || (support_limit as f64 - end_z as f64) < SKIN as f64
        {
            return None;
        }
        let proof = AscendingSupport {
            query,
            triangle,
            start,
            delta,
            radius,
            height,
            max_step,
        };
        proof.resolve_prefix(delta.to_array())?;
        Some(proof)
    }
}

/// Outward bounds for the actual f32 expressions used by ground_at/plane_z.
/// Each operation expands one representable value beyond its rounded extrema;
/// overflow, division by zero, and uncertain footprint coverage reject proof.
#[derive(Clone, Copy)]
struct Bounds {
    low: f32,
    high: f32,
}

impl Bounds {
    fn new(low: f32, high: f32) -> Option<Self> {
        (low.is_finite() && high.is_finite() && low <= high).then_some(Self { low, high })
    }
    fn exact(value: f32) -> Option<Self> {
        Self::new(value, value)
    }
    fn add(self, other: Self) -> Option<Self> {
        Self::new(
            (self.low + other.low).next_down(),
            (self.high + other.high).next_up(),
        )
    }
    fn sub(self, other: Self) -> Option<Self> {
        Self::new(
            (self.low - other.high).next_down(),
            (self.high - other.low).next_up(),
        )
    }
    fn mul(self, value: f32) -> Option<Self> {
        let a = self.low * value;
        let b = self.high * value;
        Self::new(a.min(b).next_down(), a.max(b).next_up())
    }
    fn div(self, value: f32) -> Option<Self> {
        if !value.is_finite() || value == 0. {
            return None;
        }
        let a = self.low / value;
        let b = self.high / value;
        Self::new(a.min(b).next_down(), a.max(b).next_up())
    }
}

fn plane_bounds(triangle: &Triangle, xy: [Bounds; 2]) -> Option<Bounds> {
    let a = triangle.points[0];
    let x = xy[0].sub(Bounds::exact(a.x)?)?.mul(triangle.normal.x)?;
    let y = xy[1].sub(Bounds::exact(a.y)?)?.mul(triangle.normal.y)?;
    Bounds::exact(a.z)?.sub(x.add(y)?.div(triangle.normal.z)?)
}

fn contains_footprint(triangle: &Triangle, xy: [Bounds; 2]) -> bool {
    fn barycentric(triangle: &Triangle, xy: [Bounds; 2]) -> Option<(Bounds, Bounds)> {
        let [a, b, c] = triangle.points.map(Vec3::truncate);
        let ab = b - a;
        let ac = c - a;
        let denominator = cross(ab, ac);
        let x = xy[0].sub(Bounds::exact(a.x)?)?;
        let y = xy[1].sub(Bounds::exact(a.y)?)?;
        let u = x.mul(ac.y)?.sub(y.mul(ac.x)?)?.div(denominator)?;
        let v = y.mul(ab.x)?.sub(x.mul(ab.y)?)?.div(denominator)?;
        Some((u, v))
    }
    barycentric(triangle, xy)
        .is_some_and(|(u, v)| u.low > 0. && v.low > 0. && u.add(v).is_some_and(|sum| sum.high < 1.))
}

#[cfg(test)]
#[path = "collision_ascending_tests.rs"]
mod tests;
