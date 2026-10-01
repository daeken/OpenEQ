//! Opt-in CPU diagnostic of the original default sky dome after color update.
//!
//! Topology and source-color indices follow executed original instructions;
//! positions use the native stored f32 angles with f64 trigonometry, then f32
//! coordinates. This is a tolerance-bounded approximation of x87 arithmetic,
//! not a promise of identical position bits. Sparse native witnesses at radius
//! 800 are checked within 0.000062 world units.
//!
//! Coordinates retain original EQ Z-up, with positive/negative Z poles and the
//! caller's camera far distance as radius. No scene-axis conversion, celestial
//! orientation, camera translation, graphics state or rendering is applied.
//! See `docs/SKY_DOME_GEOMETRY.md` for the original-instruction witness scope.

use super::{SkyAssets, SkyColorMapLayout};
use crate::{Error, Result};

const SECTORS: u16 = 31;
const STEPS: u16 = 29;
const RING_VERTICES: u16 = SECTORS + 1;
const VERTEX_COUNT: usize = 962;
const INDEX_COUNT: usize = 5583;
const NEGATIVE_POLE: u16 = 961;
// Original float32 stores, promoted before the angle products and trig calls.
const DPHI: f64 = f32::from_bits(0x3e4f_8c3d) as f64;
const DTHETA: f64 = f32::from_bits(0x3dcf_bcc0) as f64;
const RING_MARGIN: f64 = 0.1_f32 as f64;
const FIRST_RING_ANGLE: f64 = 0.01_f32 as f64;

/// Original XYZ | DIFFUSE layout: 12 coordinate bytes and one packed word.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NativeSkyVertex {
    pub position: [f32; 3],
    /// Packed AARRGGBB, preserving all source RGBA bits without color-space or
    /// alpha conversion. On little-endian machines the memory bytes are BGRA.
    pub diffuse: u32,
}

#[derive(Debug, Clone)]
pub struct NativeSkyDome {
    pub radius: f32,
    pub vertices: Vec<NativeSkyVertex>,
    /// Original triangle-list order, including the extra bottom-cap triangle.
    pub indices: Vec<u16>,
    /// One word index into the original 32x32 table for each vertex.
    pub source_color_indices: Vec<u32>,
}

impl NativeSkyDome {
    /// Build only when explicitly requested by diagnostic code. The provided
    /// sampled table must retain the original 32x32 layout. Temporary native
    /// allocation colors are omitted: every vertex has its final table color.
    pub fn build(radius: f32, sky: &SkyAssets) -> Result<Self> {
        if !radius.is_finite() || radius <= 0. {
            return Err(Error::Format(
                "sky dome radius must be finite and positive".into(),
            ));
        }
        let table = &sky.color_map;
        if sky.color_map_layout != SkyColorMapLayout::OriginalDome
            || (table.width, table.height) != (32, 32)
            || table.rgba.len() != 32 * 32 * 4
        {
            return Err(Error::Format(
                "sky dome requires an original 32x32 RGBA table".into(),
            ));
        }
        let mut vertices = Vec::with_capacity(VERTEX_COUNT);
        let mut source_color_indices = Vec::with_capacity(VERTEX_COUNT);
        let mut vertex = |position, source: u32| {
            let offset = source as usize * 4;
            let [r, g, b, a] = table.rgba[offset..offset + 4].try_into().unwrap();
            vertices.push(NativeSkyVertex {
                position,
                diffuse: u32::from_be_bytes([a, r, g, b]),
            });
            source_color_indices.push(source);
        };
        vertex([0., 0., radius], 0);
        let radius_f64 = f64::from(radius);
        for ring in 0..=STEPS {
            let theta = if ring == 0 {
                FIRST_RING_ANGLE
            } else {
                RING_MARGIN + f64::from(ring - 1) * DTHETA
            };
            for k in 0..=SECTORS {
                // Keep the native seam's small rounding gap; do not replace
                // the first angle with an exact TAU or duplicate its vertex.
                let phi = f64::from(SECTORS - k) * DPHI;
                let position = [
                    (radius_f64 * theta.sin() * phi.sin()) as f32,
                    (radius_f64 * theta.sin() * phi.cos()) as f32,
                    (radius_f64 * theta.cos()) as f32,
                ];
                let source = if ring == 0 {
                    30_u16.saturating_sub(k)
                } else {
                    // The original divides by steps (29), not sectors (31).
                    ring * 32 + k % STEPS
                };
                vertex(position, u32::from(source));
            }
        }
        vertex([0., 0., -radius], 928);

        let mut indices = Vec::with_capacity(INDEX_COUNT);
        for k in 0..SECTORS {
            indices.extend_from_slice(&[0, 1 + k, 2 + k]);
        }
        for ring in 1..=STEPS {
            for k in 0..SECTORS {
                let v = 1 + RING_VERTICES * ring + k;
                indices.extend_from_slice(&[v, v + 1, v - 32, v + 1, v - 31, v - 32]);
            }
        }
        // The native cap has 32 triangles, ending in [961, 929, 928], where
        // 928 belongs to the previous ring. Retain this observed irregularity.
        for k in 0..=SECTORS {
            indices.extend_from_slice(&[NEGATIVE_POLE, 960 - k, 959 - k]);
        }
        Ok(Self {
            radius,
            vertices,
            indices,
            source_color_indices,
        })
    }
}

#[cfg(test)]
mod tests;
