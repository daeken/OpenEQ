//! Bounded native region queries for binary EQGZ versions 1 and 2.
//!
//! This reads region metadata without resolving meshes, textures or lights.
//! Binary angles reach the native box builder unchanged in Z/Y/X order and
//! 512-unit turns; sizes are signed half-extents. Regions retain file order.
//! See `docs/EQGZ_NATIVE_REGIONS.md` for the reader/factory trace and limits.

use glam::{DMat3, DVec3};

use crate::{Error, Result, read::Reader, terrain::regions::native_type_word, zone::ZON_MAGIC};

/// One registered box and its original binary record. Native EQGZ registration
/// supplies type zero; classification happens after choosing a winning box.
#[derive(Debug, Clone)]
pub struct BinaryRegionBox {
    pub record_index: usize,
    pub source_offset: usize,
    pub name: String,
    pub raw_type: u32,
    pub effective_type: Option<u32>,
    pub center: [f32; 3],
    /// Raw Z/Y/X native angular units, not radians or degrees.
    pub orientation: [f32; 3],
    /// Raw signed XYZ half-extents, including reflection axes.
    pub half_extents: [f32; 3],
    /// Truncated raw Z/Y/X values, before table wrapping and middle negation.
    pub angle_units: [i32; 3],
    inverse: DMat3,
}

impl BinaryRegionBox {
    fn read(reader: &mut Reader<'_>, strings: &[u8], record_index: usize) -> Result<Self> {
        let source_offset = reader.pos();
        let name_offset = reader.u32()? as usize;
        let tail = strings
            .get(name_offset..)
            .ok_or_else(|| invalid("region name outside string table"))?;
        let end = tail
            .iter()
            .take(4097)
            .position(|byte| *byte == 0)
            .ok_or_else(|| invalid("unterminated or overlong region name"))?;
        let name = std::str::from_utf8(&tail[..end])
            .map_err(|_| invalid("unsupported region name encoding"))?
            .to_owned();
        let center = reader.vec3()?;
        let orientation = reader.vec3()?;
        let half_extents = reader.vec3()?;
        if center
            .iter()
            .chain(&orientation)
            .chain(&half_extents)
            .any(|value| !value.is_finite())
        {
            return Err(invalid("non-finite region transform"));
        }
        if name.starts_with("AFG") {
            return Err(invalid("unsupported AFG special box constructor"));
        }
        if half_extents.iter().any(|value| !value.is_normal()) {
            return Err(invalid("zero or subnormal region extent"));
        }
        // Both the raw angle and its negation must fit the native signed
        // conversion. Reject conversion overflow rather than emulate a CPU's
        // invalid-conversion sentinel as an ordinary zero table index.
        if orientation
            .iter()
            .any(|value| f64::from(*value).abs() >= 2_147_483_648.)
        {
            return Err(invalid("region angle conversion overflow"));
        }
        let angle_units = orientation.map(|value| value.trunc() as i32);
        let (sz, cz) = table_sin_cos(angle_units[0]);
        let (sy, cy) = table_sin_cos(-angle_units[1]);
        let (sx, cx) = table_sin_cos(angle_units[2]);
        // The native builder spills this product to float32 at 0x100c2899
        // before reusing it in the last column. The sx*sy product stays in x87.
        let cx_sy = f64::from((cx * sy) as f32);
        // Native row-vector storage transposed to column-vector notation:
        // Rz(raw[0]) * Ry(-raw[1]) * Rx(raw[2]) * S(signed extents).
        // Rotation entries and scaled entries are each stored as float32.
        let columns = [
            [cy * cz, cy * sz, -sy],
            [sx * sy * cz - cx * sz, sx * sy * sz + cx * cz, sx * cy],
            [cx_sy * cz + sx * sz, cx_sy * sz - sx * cz, cx * cy],
        ];
        let scaled = std::array::from_fn::<_, 3, _>(|axis| {
            DVec3::from(columns[axis].map(|value| f64::from((value as f32) * half_extents[axis])))
        });
        let basis = DMat3::from_cols(scaled[0], scaled[1], scaled[2]);
        let determinant = basis.determinant() as f32;
        if !basis.is_finite() || !determinant.is_normal() {
            return Err(invalid("unsupported singular or overflowing region basis"));
        }
        // Float32 trig entries are not exactly orthonormal. Inverting the
        // basis retains this distinction; transpose is not its inverse.
        let inverse = basis.inverse();
        let translation = inverse * DVec3::from(center.map(f64::from));
        if !inverse
            .to_cols_array()
            .into_iter()
            .chain(translation.to_array())
            .all(|value| (value as f32).is_finite())
        {
            return Err(invalid("region inverse transform overflow"));
        }
        Ok(Self {
            record_index,
            source_offset,
            effective_type: native_type_word(&name, 0),
            name,
            raw_type: 0,
            center,
            orientation,
            half_extents,
            angle_units,
            inverse,
        })
    }

    /// Inclusive unit-box containment. f64 inversion approximates native x87
    /// inverse arithmetic; no tolerance is added to enlarge the occupied box.
    pub fn contains(&self, point: [f64; 3]) -> bool {
        if !point.into_iter().all(f64::is_finite) {
            return false;
        }
        let local = self.inverse * (DVec3::from(point) - DVec3::from(self.center.map(f64::from)));
        local.abs().cmple(DVec3::ONE).all()
    }

    /// Positive-length interval along from + (to-from)t inside this box.
    /// This CPU geometry helper is not a recovered native movement routine.
    pub fn segment(&self, from: [f64; 3], to: [f64; 3]) -> Option<[f64; 2]> {
        if !from.into_iter().chain(to).all(f64::is_finite) {
            return None;
        }
        let center = DVec3::from(self.center.map(f64::from));
        let start = self.inverse * (DVec3::from(from) - center);
        let end = self.inverse * (DVec3::from(to) - center);
        let delta = end - start;
        if !start.is_finite() || !end.is_finite() || !delta.is_finite() || delta == DVec3::ZERO {
            return None;
        }
        let mut enter: f64 = 0.;
        let mut exit: f64 = 1.;
        for axis in 0..3 {
            if delta[axis] == 0. {
                if start[axis].abs() > 1. {
                    return None;
                }
            } else {
                let a = (-1. - start[axis]) / delta[axis];
                let b = (1. - start[axis]) / delta[axis];
                enter = enter.max(a.min(b));
                exit = exit.min(a.max(b));
            }
        }
        (exit > enter).then_some([enter, exit])
    }
}

/// All binary regions, including dry and unknown kinds, in native registration
/// order. An unsupported record rejects the entire set, never a single winner.
#[derive(Debug)]
pub struct BinaryRegions {
    pub version: u32,
    pub boxes: Vec<BinaryRegionBox>,
}

impl BinaryRegions {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data);
        let magic = reader.u32()?;
        if magic != ZON_MAGIC {
            return Err(Error::BadMagic {
                found: magic,
                expected: ZON_MAGIC,
            });
        }
        let version = reader.u32()?;
        if !matches!(version, 1 | 2) {
            return Err(invalid("unsupported EQGZ version"));
        }
        let string_size = reader.bounded_count()?;
        let model_count = reader.bounded_count()?;
        let object_count = reader.bounded_count()?;
        let region_count = reader.bounded_count()?;
        let light_count = reader.bounded_count()?;
        let strings = reader.take(string_size)?;
        skip_records(&mut reader, model_count, 4)?;
        let object_size = if version == 1 { 36 } else { 40 };
        if object_count > reader.remaining() / object_size {
            return Err(invalid("object count exceeds EQGZ data"));
        }
        for _ in 0..object_count {
            reader.skip(36)?;
            if version == 2 {
                let count = reader.bounded_count()?;
                skip_records(&mut reader, count, 4)?;
            }
        }
        if region_count > reader.remaining() / 40 {
            return Err(invalid("region count exceeds EQGZ data"));
        }
        let mut boxes = Vec::with_capacity(region_count);
        for index in 0..region_count {
            boxes.push(BinaryRegionBox::read(&mut reader, strings, index)?);
        }
        skip_records(&mut reader, light_count, 32)?;
        if reader.remaining() != 0 {
            return Err(invalid("unrecognized trailing EQGZ data"));
        }
        Ok(Self { version, boxes })
    }

    /// Native first-match selection before classification. Preferred prefixes
    /// get an initial pass; generic fallback excludes APV and preserves dry
    /// or unrecognized winners instead of searching onward for a liquid.
    pub fn at(
        &self,
        point: [f64; 3],
        preferred_prefix: Option<[u8; 3]>,
    ) -> Option<&BinaryRegionBox> {
        if let Some(prefix) = preferred_prefix
            && let Some(region) = self.boxes.iter().find(|region| {
                region.name.as_bytes().get(..3) == Some(prefix.as_slice()) && region.contains(point)
            })
        {
            return Some(region);
        }
        self.boxes
            .iter()
            .find(|region| !region.name.starts_with("APV") && region.contains(point))
    }
}

fn skip_records(reader: &mut Reader<'_>, count: usize, size: usize) -> Result<()> {
    if count > reader.remaining() / size {
        return Err(invalid("record array exceeds EQGZ data"));
    }
    reader.skip(count * size)
}

fn table_sin_cos(units: i32) -> (f64, f64) {
    match units & 511 {
        0 => (0., 1.),
        128 => (1., 0.),
        256 => (0., -1.),
        384 => (-1., 0.),
        index => {
            let angle = f64::from(index) * std::f64::consts::TAU / 512.;
            let (sin, cos) = angle.sin_cos();
            (f64::from(sin as f32), f64::from(cos as f32))
        }
    }
}

fn invalid(detail: &str) -> Error {
    Error::Format(format!("binary regions: {detail}"))
}
