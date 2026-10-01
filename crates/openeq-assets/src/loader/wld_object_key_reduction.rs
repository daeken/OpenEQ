//! The native compressor's first removal, for five authored rotation frames.
//! See docs/WLD_LONG_OBJECT_ANIMATION.md. This is not a general key compressor.

use super::{Frame, Quat, Result, invalid};

// Deliberately larger than the score differences observed across native scalar,
// SSE, SSE2 and x87 precision modes. This is a conservative admission policy,
// not a claim of bit-identical ranking for arbitrary floating-point inputs.
const MIN_ERROR_SEPARATION: f32 = 1e-5;

pub(super) struct FiveFrameKeys {
    rotations: [[f32; 4]; 6],
    pub(super) omitted: usize,
}

impl FiveFrameKeys {
    pub(super) fn new(frames: &[Frame]) -> Result<Self> {
        if frames.len() != 5 {
            return Err(invalid("rotation reduction requires five authored frames"));
        }
        let mut rotations = [[0.; 4]; 6];
        for index in 0..6 {
            let mut rotation = frames[index % 5].rotation;
            if rotation.iter().any(|value| {
                let packed = *value * 16384.;
                !packed.is_finite()
                    || !(-32768. ..=32767.).contains(&packed)
                    || packed.fract() != 0.
            }) || (dot(rotation, rotation) - 1.).abs() > 0.01
            {
                return Err(invalid(
                    "unproven five-frame object rotation magnitude or encoding",
                ));
            }
            if index > 0 && dot(rotations[index - 1], rotation) < 0. {
                rotation = rotation.map(|value| -value);
            }
            rotations[index] = rotation;
        }

        // At lossiness 0.1 the native compressor retains five of six keys.
        // Only its first removal matters: compare initial neighbor errors and
        // retain the earliest source index on an exact f32 tie.
        let mut omitted = 1;
        let mut least_error = f32::INFINITY;
        let mut scores = [0.; 4];
        let mut midpoints = [[0.; 4]; 4];
        for index in 1..5 {
            let midpoint = midpoint(rotations[index - 1], rotations[index + 1]);
            // Keep normalization away from a cancellation or denormal regime.
            if dot(midpoint, midpoint) < 0.5 {
                return Err(invalid("unproven five-frame object rotation midpoint"));
            }
            let error = removal_error(midpoint, rotations[index]);
            if !error.is_finite() {
                return Err(invalid("nonfinite object rotation reduction error"));
            }
            scores[index - 1] = error;
            midpoints[index - 1] = midpoint;
            if error < least_error {
                least_error = error;
                omitted = index;
            }
        }
        for index in (1..5).filter(|index| *index != omitted) {
            if scores[index - 1] - least_error <= MIN_ERROR_SEPARATION
                && !same_error_inputs(
                    rotations[omitted],
                    midpoints[omitted - 1],
                    rotations[index],
                    midpoints[index - 1],
                )
            {
                return Err(invalid("numerically ambiguous object rotation reduction"));
            }
        }

        // The compressor reapplies hemisphere continuity after omitting a key.
        // Also reject a newly adjacent ambiguous pair, just as the existing
        // sampler rejects ambiguous authored neighbors.
        let mut previous = 0;
        for index in (1..6).filter(|index| *index != omitted) {
            let a = Quat::from_array(rotations[previous]).normalize();
            let b = Quat::from_array(rotations[index]).normalize();
            let normalized_dot = a.dot(b);
            if !normalized_dot.is_finite() || normalized_dot.abs() < 1e-6 {
                return Err(invalid("ambiguous reduced object quaternion hemisphere"));
            }
            if dot(rotations[previous], rotations[index]) < 0. {
                rotations[index] = rotations[index].map(|value| -value);
            }
            previous = index;
        }
        Ok(Self { rotations, omitted })
    }

    pub(super) fn sample(&self, phase: u128, interval: u32) -> [f32; 4] {
        let index = (phase / u128::from(interval)) as usize;
        let left = if index == self.omitted {
            index - 1
        } else {
            index
        };
        let right = left + if left + 1 == self.omitted { 2 } else { 1 };
        let fraction = (phase as u32 - left as u32 * interval) as f32
            / ((right - left) as u32 * interval) as f32;
        let a = Quat::from_array(self.rotations[left]);
        let b = Quat::from_array(self.rotations[right]);
        (a + (b - a) * fraction).to_array()
    }
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f64 {
    // The original error routine accumulates W, Z, Y, X in wider registers.
    (0..4)
        .rev()
        .map(|index| f64::from(a[index]) * f64::from(b[index]))
        .sum()
}

fn midpoint(previous: [f32; 4], next: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|index| {
        (f64::from(previous[index]) + (f64::from(next[index]) - f64::from(previous[index])) * 0.5)
            as f32
    })
}

fn same_error_inputs(a: [f32; 4], am: [f32; 4], b: [f32; 4], bm: [f32; 4]) -> bool {
    (a == b && am == bm) || (a == b.map(|value| -value) && am == bm.map(|value| -value))
}

fn removal_error(mut midpoint: [f32; 4], authored: [f32; 4]) -> f32 {
    // Native D3DX stores squared length before its near-unit test. Do not
    // normalize the authored key: its packed magnitude affects ranking.
    let squared = midpoint
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>() as f32;
    if (f64::from(squared) - 1.).abs() > f64::from(f32::EPSILON) {
        midpoint = if squared > f32::MIN_POSITIVE {
            let inverse = 1. / f64::from(squared).sqrt();
            midpoint.map(|value| (f64::from(value) * inverse) as f32)
        } else {
            [0.; 4]
        };
    }
    let similarity = dot(midpoint, authored).clamp(-1., 1.);
    (1. - similarity * similarity) as f32
}
