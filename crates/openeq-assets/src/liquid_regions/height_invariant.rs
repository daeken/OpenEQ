//! Conservative height-invariance proofs for collision support trajectories.

use super::{LiquidRegions, Volumes};
use glam::DVec3;

impl LiquidRegions {
    /// Prove that `at([x, y, z])` is independent of Z throughout a finite AABB.
    /// This preserves the complete liquid kind, including dry results and
    /// source precedence. Bounds are inclusive. False means no proof, not that
    /// a height-dependent liquid boundary necessarily exists in the bounds.
    ///
    /// A caller must enclose every possible queried position, including its
    /// actual rounded body-center heights. This does not certify a trajectory
    /// or its XY parameterization. Initially only WLD BSP and identity-rotation
    /// authored boxes are supported; native and rotated volumes reject proof.
    pub fn height_invariant_in_bounds(&self, min: [f32; 3], max: [f32; 3]) -> bool {
        if min
            .into_iter()
            .zip(max)
            .any(|(a, b)| !a.is_finite() || !b.is_finite() || a > b)
        {
            return false;
        }
        let min = DVec3::from_array(min.map(f64::from));
        let max = DVec3::from_array(max.map(f64::from));
        match &*self.volumes {
            Volumes::Empty => true,
            Volumes::NativeTerrain(_) | Volumes::NativeBinary(_) => false,
            Volumes::Boxes(boxes) => boxes.iter().all(|volume| {
                let [x, y, z, w] = volume.inverse_rotation.to_array();
                if x != 0. || y != 0. || z != 0. || w.abs() != 1. {
                    return false;
                }
                // With identity rotation, at() reduces to these same local
                // coordinates. Rounded f64 subtraction is monotone, so its
                // actual endpoint results bound every coordinate in between.
                // Keep exact inclusive faces, rather than dilating the volume.
                let low = min - volume.center;
                let high = max - volume.center;
                let half = volume.half_extents;
                if (0..3).any(|axis| high[axis] < -half[axis] || low[axis] > half[axis]) {
                    return true;
                }
                low.z >= -half.z && high.z <= half.z
            }),
            Volumes::Bsp { nodes, wet, .. } => {
                let axes = [
                    Bounds {
                        low: min.x,
                        high: max.x,
                    },
                    Bounds {
                        low: min.y,
                        high: max.y,
                    },
                    Bounds {
                        low: min.z,
                        high: max.z,
                    },
                ];
                let mut pending = vec![0];
                while let Some(index) = pending.pop() {
                    if !wet[index] {
                        continue;
                    }
                    let node = &nodes[index];
                    if node.children == [0, 0] {
                        continue;
                    }
                    let side = distance_bounds(node.plane, axes).and_then(|distance| {
                        if distance.low > 0. {
                            Some(0)
                        } else if distance.high < 0. {
                            Some(1)
                        } else {
                            None
                        }
                    });
                    if let Some(side) = side {
                        let child = node.children[side];
                        if child != 0 {
                            pending.push(child as usize - 1);
                        }
                    } else if node.plane[2] == 0. {
                        // This decision (including its dry zero plane) cannot
                        // depend on Z. Both reachable sides must be invariant.
                        for child in node.children {
                            if child != 0 {
                                pending.push(child as usize - 1);
                            }
                        }
                    } else {
                        // Crossing or touching a height-dependent plane is
                        // unsupported, even if both leaves have the same kind:
                        // the WLD split plane itself is always dry.
                        return false;
                    }
                }
                true
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Bounds {
    low: f64,
    high: f64,
}

impl Bounds {
    fn outward(low: f64, high: f64) -> Option<Self> {
        let low = low.next_down();
        let high = high.next_up();
        (low.is_finite() && high.is_finite() && low <= high).then_some(Self { low, high })
    }
    fn mul(self, coefficient: f64) -> Option<Self> {
        if !coefficient.is_finite() {
            return None;
        }
        let a = self.low * coefficient;
        let b = self.high * coefficient;
        Self::outward(a.min(b), a.max(b))
    }
    fn add(self, other: Self) -> Option<Self> {
        Self::outward(self.low + other.low, self.high + other.high)
    }
}

/// Bound the actual f64 dot-product order used by LiquidRegions::at, including
/// rounding at each multiply and add. A geometric corner-sign test alone can
/// miss cancellation near a WLD plane, whose zero boundary is explicitly dry.
fn distance_bounds(plane: [f64; 4], axes: [Bounds; 3]) -> Option<Bounds> {
    axes[0]
        .mul(plane[0])?
        .add(axes[1].mul(plane[1])?)?
        .add(axes[2].mul(plane[2])?)?
        .add(Bounds {
            low: plane[3],
            high: plane[3],
        })
}

#[cfg(test)]
#[path = "height_invariant_tests.rs"]
mod tests;
