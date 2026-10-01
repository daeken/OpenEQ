//! Two zero-error removals for a bounded 16-frame scalar-reference family.
//! See docs/WLD_LAMP_ANIMATION.md. This is not a general native compressor.

use super::{Frame, Quat, Result, invalid};

const CLAMP_MARGIN: f64 = 1e-6;
const ROUNDING_MARGIN: f64 = 1e-12;

#[derive(Debug)]
pub(super) struct SixteenFrameKeys {
    rotations: [[f32; 4]; 17],
    pub(super) omitted: [usize; 2],
}

impl SixteenFrameKeys {
    pub(super) fn new(frames: &[Frame]) -> Result<Self> {
        if frames.len() != 16
            || frames.iter().any(|frame| {
                frame.translation != frames[0].translation || frame.scale != frames[0].scale
            })
        {
            return Err(invalid(
                "sixteen-frame rotation requires constant translation and scale",
            ));
        }
        let mut axes = [false; 3];
        let mut rotations = [[0.; 4]; 17];
        for index in 0..17 {
            let mut rotation = frames[index % 16].rotation;
            if rotation.iter().any(|value| {
                let packed = *value * 16384.;
                !packed.is_finite()
                    || !(-32768. ..=32767.).contains(&packed)
                    || packed.fract() != 0.
            }) || (dot(rotation, rotation) - 1.).abs() > 0.01
            {
                return Err(invalid("unproven sixteen-frame rotation encoding"));
            }
            for axis in 0..3 {
                axes[axis] |= rotation[axis] != 0.;
            }
            if index > 0 && dot(rotations[index - 1], rotation) < 0. {
                rotation = rotation.map(|value| -value);
            }
            rotations[index] = rotation;
        }
        if axes.into_iter().filter(|active| *active).count() != 1 {
            return Err(invalid("sixteen-frame rotation requires one axis"));
        }

        // Scalar reference: every interior error is either robustly zero or
        // positive. Native strict-less heap ordering among zero keys depends
        // only on these categories, not on the order of positive errors.
        let mut categories = [1; 17];
        let mut heap = Vec::with_capacity(15);
        for index in 1..16 {
            categories[index] = classify(&rotations, index, index - 1, index + 1)?;
            heap.push(index);
            let mut child = heap.len() - 1;
            while child > 0 {
                let parent = (child - 1) / 2;
                if categories[heap[child]] >= categories[heap[parent]] {
                    break;
                }
                heap.swap(parent, child);
                child = parent;
            }
        }
        let first = remove_min(&mut heap, &categories);
        if categories[first] != 0 {
            return Err(invalid("sixteen-frame rotation has no first zero error"));
        }
        // Native recomputes neighboring errors and multiplies by their time
        // span after a removal. A positive span preserves zero/positive status.
        // Admit only unchanged categories, so positive-key heap rearrangements
        // cannot affect which remaining zero key is selected next.
        for neighbor in [first - 1, first + 1] {
            if neighbor > 0 && neighbor < 16 {
                let left = neighbor - 1 - usize::from(neighbor - 1 == first);
                let right = neighbor + 1 + usize::from(neighbor + 1 == first);
                if classify(&rotations, neighbor, left, right)? != categories[neighbor] {
                    return Err(invalid("object reduction changes error category"));
                }
            }
        }
        let second = remove_min(&mut heap, &categories);
        if categories[second] != 0 {
            return Err(invalid("sixteen-frame rotation has no second zero error"));
        }
        let omitted = [first, second];
        let mut previous = 0;
        for index in (1..17).filter(|index| !omitted.contains(index)) {
            let a = Quat::from_array(rotations[previous]).normalize();
            let b = Quat::from_array(rotations[index]).normalize();
            if a.dot(b).abs() < 1e-6 {
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
        let mut left = (phase / u128::from(interval)) as usize;
        while self.omitted.contains(&left) {
            left -= 1;
        }
        let mut right = left + 1;
        while self.omitted.contains(&right) {
            right += 1;
        }
        let fraction = f64::from(phase as u32 - left as u32 * interval)
            / f64::from((right - left) as u32 * interval);
        std::array::from_fn(|axis| {
            let a = f64::from(self.rotations[left][axis]);
            let b = f64::from(self.rotations[right][axis]);
            (a + (b - a) * fraction) as f32
        })
    }
}

fn remove_min(heap: &mut Vec<usize>, categories: &[u8; 17]) -> usize {
    let result = heap.swap_remove(0);
    let mut index = 0;
    loop {
        let mut smallest = index;
        for child in [index * 2 + 1, index * 2 + 2] {
            if child < heap.len() && categories[heap[child]] < categories[heap[smallest]] {
                smallest = child;
            }
        }
        if smallest == index {
            return result;
        }
        heap.swap(index, smallest);
        index = smallest;
    }
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f64 {
    (0..4)
        .rev()
        .map(|axis| f64::from(a[axis]) * f64::from(b[axis]))
        .sum()
}

fn guarded_round(value: f64) -> Result<f32> {
    let rounded = value as f32;
    if !rounded.is_finite() {
        return Err(invalid("nonfinite object reduction intermediate"));
    }
    if value != 0. {
        let magnitude = rounded.abs();
        let lower = (f64::from(magnitude.next_down()) + f64::from(magnitude)) * 0.5;
        let upper = (f64::from(magnitude.next_up()) + f64::from(magnitude)) * 0.5;
        if (value.abs() - lower).abs().min((value.abs() - upper).abs()) <= ROUNDING_MARGIN {
            return Err(invalid("object reduction near rounding boundary"));
        }
    }
    Ok(rounded)
}

fn classify(keys: &[[f32; 4]; 17], index: usize, left: usize, right: usize) -> Result<u8> {
    let fraction = (index - left) as f64 / (right - left) as f64;
    let mut midpoint = [0.; 4];
    for (axis, component) in midpoint.iter_mut().enumerate() {
        let a = f64::from(keys[left][axis]);
        let b = f64::from(keys[right][axis]);
        *component = guarded_round(a + (b - a) * fraction)?;
    }
    let squared = guarded_round(midpoint.iter().map(|value| f64::from(*value).powi(2)).sum())?;
    if !(0.5..=1.5).contains(&squared) {
        return Err(invalid("unproven object reduction midpoint magnitude"));
    }
    // Original scalar D3DX stores squared length, applies its near-unit fast
    // path, otherwise normalizes with wider arithmetic and f32 component stores.
    if (f64::from(squared) - 1.).abs() > f64::from(f32::EPSILON) {
        let inverse = 1. / f64::from(squared).sqrt();
        for component in &mut midpoint {
            *component = guarded_round(f64::from(*component) * inverse)?;
        }
    }
    let authored = keys[index];
    for axis in 0..4 {
        if authored[axis].abs() == 1.
            && (0..4).all(|other| other == axis || authored[other] == 0.)
            && midpoint[axis] == authored[axis]
        {
            return Ok(0);
        }
    }
    let similarity = dot(midpoint, authored).abs();
    if similarity >= 1. + CLAMP_MARGIN {
        Ok(0)
    } else if similarity <= 1. - CLAMP_MARGIN {
        Ok(1)
    } else {
        Err(invalid("object reduction near clamp boundary"))
    }
}
