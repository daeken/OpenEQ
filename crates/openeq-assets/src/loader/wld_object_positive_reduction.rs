//! Bounded forty-frame scalar-PC64 key reduction with certified f32 scores.
//! Every native arithmetic result is enclosed by outward binary64 bounds;
//! admission requires a unique final f32 score, without a tuned tolerance.

use super::{Frame, Quat, Result, invalid};

#[derive(Debug)]
pub(super) struct FortyFrameKeys {
    rotations: [[f32; 4]; 41],
    pub(super) omitted: [usize; 5],
}

impl FortyFrameKeys {
    pub(super) fn new(frames: &[Frame], interval: u32) -> Result<Self> {
        if frames.len() != 40
            || interval == 0
            || interval > (1 << 24) / 40
            || frames.iter().any(|frame| {
                frame.translation != frames[0].translation || frame.scale != frames[0].scale
            })
        {
            return Err(invalid(
                "forty-frame rotation requires fixed translation, scale and exact timing",
            ));
        }
        let mut rotations = [[0.; 4]; 41];
        for index in 0..41 {
            let mut rotation = frames[index % 40].rotation;
            if rotation.iter().any(|value| {
                let packed = *value * 16384.;
                !packed.is_finite()
                    || !(-32768. ..=32767.).contains(&packed)
                    || packed.fract() != 0.
            }) || (dot(rotation, rotation) - 1.).abs() > 0.01
            {
                return Err(invalid("unproven forty-frame rotation encoding"));
            }
            if index > 0 && dot(rotations[index - 1], rotation) < 0. {
                rotation = rotation.map(|value| -value);
            }
            rotations[index] = rotation;
        }
        let mut scores = [0.; 41];
        let mut heap = Vec::with_capacity(39);
        for index in 1..40 {
            scores[index] = score(&rotations, index, index - 1, index + 1, interval, false)?;
            heap.push(index);
            let last = heap.len() - 1;
            sift_up(&mut heap, &scores, last);
        }
        let mut previous: [usize; 41] = std::array::from_fn(|index| index.saturating_sub(1));
        let mut following: [usize; 41] = std::array::from_fn(|index| index + 1);
        let mut omitted = [0; 5];
        // 41 * f32(1 - 0.1) is strictly between 36 and 37 in the reference
        // precision, so the native compressor retains 36 keys.
        for removed in &mut omitted {
            *removed = heap.swap_remove(0);
            sift_down(&mut heap, &scores, 0);
            let left = previous[*removed];
            let right = following[*removed];
            following[left] = right;
            previous[right] = left;
            let saved_right = heap.iter().position(|&key| key == right);
            if let Some(position) = heap.iter().position(|&key| key == left) {
                scores[left] = score(
                    &rotations,
                    left,
                    previous[left],
                    following[left],
                    interval,
                    true,
                )?;
                let position = sift_up(&mut heap, &scores, position);
                sift_down(&mut heap, &scores, position);
            }
            // Native saves the right neighbor's heap address before the left
            // sift. It can become stale; admit only when it still names right.
            if let Some(position) = saved_right {
                if heap[position] != right {
                    return Err(invalid("object reduction moves saved right neighbor"));
                }
                scores[right] = score(
                    &rotations,
                    right,
                    previous[right],
                    following[right],
                    interval,
                    true,
                )?;
                let position = sift_up(&mut heap, &scores, position);
                sift_down(&mut heap, &scores, position);
            }
        }
        let mut previous = 0;
        for index in (1..41).filter(|index| !omitted.contains(index)) {
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

fn sift_up(heap: &mut [usize], scores: &[f32; 41], mut index: usize) -> usize {
    while index > 0 {
        let parent = (index - 1) / 2;
        if scores[heap[index]] >= scores[heap[parent]] {
            break;
        }
        heap.swap(index, parent);
        index = parent;
    }
    index
}

fn sift_down(heap: &mut [usize], scores: &[f32; 41], mut index: usize) {
    loop {
        let mut smallest = index;
        for child in [index * 2 + 1, index * 2 + 2] {
            if child < heap.len() && scores[heap[child]] < scores[heap[smallest]] {
                smallest = child;
            }
        }
        if smallest == index {
            return;
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

#[derive(Clone, Copy, Debug)]
struct Enclosure {
    low: f64,
    high: f64,
}

impl Enclosure {
    fn point(value: f64) -> Self {
        Self {
            low: value,
            high: value,
        }
    }
    fn add(self, other: Self) -> Self {
        Self {
            low: (self.low + other.low).next_down(),
            high: (self.high + other.high).next_up(),
        }
    }
    fn sub(self, other: Self) -> Self {
        Self {
            low: (self.low - other.high).next_down(),
            high: (self.high - other.low).next_up(),
        }
    }
    fn mul(self, other: Self) -> Self {
        self.corners(other, |a, b| a * b)
    }
    fn div(self, other: Self) -> Self {
        // All divisors here are strictly positive time spans or square roots.
        self.corners(other, |a, b| a / b)
    }
    fn corners(self, other: Self, operation: impl Fn(f64, f64) -> f64) -> Self {
        let values = [
            operation(self.low, other.low),
            operation(self.low, other.high),
            operation(self.high, other.low),
            operation(self.high, other.high),
        ];
        Self {
            low: values.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            high: values
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .next_up(),
        }
    }
    fn sqrt(self) -> Self {
        Self {
            low: self.low.sqrt().next_down(),
            high: self.high.sqrt().next_up(),
        }
    }
    fn stored(self) -> Self {
        Self {
            low: f64::from(self.low as f32),
            high: f64::from(self.high as f32),
        }
    }
    fn unit_clamp(self) -> Self {
        Self {
            low: self.low.clamp(-1., 1.),
            high: self.high.clamp(-1., 1.),
        }
    }
}

fn score(
    keys: &[[f32; 4]; 41],
    index: usize,
    left: usize,
    right: usize,
    interval: u32,
    weighted: bool,
) -> Result<f32> {
    let time = |index: usize| Enclosure::point(f64::from(index as u32 * interval));
    let span = time(right).sub(time(left));
    let fraction = time(index).sub(time(left)).div(span);
    let mut midpoint: [Enclosure; 4] = std::array::from_fn(|axis| {
        let a = Enclosure::point(f64::from(keys[left][axis]));
        let b = Enclosure::point(f64::from(keys[right][axis]));
        a.add(b.sub(a).mul(fraction)).stored()
    });
    let mut squared = midpoint[0].mul(midpoint[0]);
    for component in &midpoint[1..] {
        squared = squared.add(component.mul(*component));
    }
    squared = squared.stored();
    if !(squared.low >= 0.5 && squared.high <= 1.5) {
        return Err(invalid("unproven object reduction midpoint magnitude"));
    }
    let low = 1. - f64::from(f32::EPSILON);
    let high = 1. + f64::from(f32::EPSILON);
    if squared.low >= low && squared.high <= high {
        // Native near-unit branch returns the stored midpoint unchanged.
    } else if squared.high < low || squared.low > high {
        let inverse = Enclosure::point(1.).div(squared.sqrt());
        midpoint = midpoint.map(|component| component.mul(inverse).stored());
    } else {
        return Err(invalid("object reduction normalizer branch is uncertain"));
    }
    let products: [Enclosure; 4] = std::array::from_fn(|axis| {
        midpoint[axis].mul(Enclosure::point(f64::from(keys[index][axis])))
    });
    let mut similarity = products[3];
    for component in products[..3].iter().rev() {
        similarity = similarity.add(*component);
    }
    similarity = similarity.unit_clamp();
    let mut error = Enclosure::point(1.).sub(similarity.mul(similarity));
    if weighted {
        error = error.mul(span);
    }
    let stored = error.stored();
    if !stored.low.is_finite()
        || !stored.high.is_finite()
        || stored.low < 0.
        || (stored.low as f32).to_bits() != (stored.high as f32).to_bits()
    {
        return Err(invalid("object reduction score store is uncertain"));
    }
    Ok(stored.low as f32)
}
