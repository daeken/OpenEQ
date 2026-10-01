//! Authored top-level heightmap DAT regions and isolated native CPU contracts.
//!
//! `LiquidRegions` uses the verified whole-set subset below. The original
//! client registers ATP first, then visits the grid in longitude /
//! latitude order. It chooses a region before classifying its environment, so
//! discarding dry or unrecognized records changes overlap behavior.
//!
//! See `docs/EQG_LIQUID_TRANSFORMS.md` for native addresses and original fixtures.
//! The bounded box implementation accepts only unit stored scale, yaw-only
//! rotations, positive extents, matching grids and strict interior anchors.
//! AFG's special box constructor is unsupported and rejects the whole set.
//! Whole-set queries require proof that placed object groups contain no areas;
//! embedded transforms and registration interleaving are not yet decoded.
//! EQGZ is a separate format.

use std::collections::BTreeSet;

use glam::{DMat3, DVec3};

use super::{Heightmap, TerrainOptions, TerrainTile, cstring};
use crate::{Error, Result, read::Reader};

/// Unmodified authored metadata, including its position in the DAT stream.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainRegion {
    pub source_offset: usize,
    pub tile_index: usize,
    pub index_in_tile: usize,
    /// Enclosing tile coordinates with the 100000 bias removed.
    pub enclosing_grid: [i32; 2],
    pub name: String,
    pub type_id: u32,
    pub alternate_name: String,
    /// Repeated on-disk grid words, with the original bias still present.
    pub grid: [u32; 2],
    /// XY is tile-local; Z is a separate offset from the sampled terrain height.
    pub position: [f32; 3],
    pub rotation_degrees: [f32; 3],
    pub scale: [f32; 3],
    pub full_size: [f32; 3],
}

impl TerrainRegion {
    pub(super) fn read(
        reader: &mut Reader<'_>,
        tile_index: usize,
        index_in_tile: usize,
        enclosing_grid: [i32; 2],
    ) -> Result<Self> {
        let value = Self {
            source_offset: reader.pos(),
            tile_index,
            index_in_tile,
            enclosing_grid,
            name: cstring(reader)?,
            type_id: reader.u32()?,
            alternate_name: cstring(reader)?,
            grid: [reader.u32()?, reader.u32()?],
            position: reader.vec3()?,
            rotation_degrees: reader.vec3()?,
            scale: reader.vec3()?,
            full_size: reader.vec3()?,
        };
        value.validate_finite()?;
        Ok(value)
    }

    fn validate_finite(&self) -> Result<()> {
        if self
            .position
            .iter()
            .chain(&self.rotation_degrees)
            .chain(&self.scale)
            .chain(&self.full_size)
            .any(|v| !v.is_finite())
        {
            return Err(invalid("non-finite region transform"));
        }
        Ok(())
    }
}

/// The recovered part of eqgame's region callback. Comparisons are case
/// sensitive, names shorter than four bytes return the raw word, and unknown
/// uppercase A names preserve it. `None` means the classic-name branch has not
/// been interpreted here, not that the region is dry. No gameplay side effects
/// (such as ATP destination parsing) are performed.
pub fn native_type_word(name: &str, raw: u32) -> Option<u32> {
    if name.len() < 4 {
        return Some(raw);
    }
    if !name.starts_with('A') {
        return None;
    }
    Some(match name.as_bytes().get(..3) {
        Some(b"AWT") => (raw & 0xffff_ff00) | 5,
        Some(b"ALV") => (raw & 0xffff_ff00) | 7,
        Some(b"AVW") => (raw & 0xffff_ff00) | 8,
        Some(b"APK") => raw | 0x4000_0000,
        Some(b"ATP") => raw | 0x8000_0000,
        Some(b"ASL") => raw | 0x1000_0000,
        _ => raw,
    })
}

/// Native top-level registration order, expressed as indices into `map.regions`.
/// This is not a complete order for embedded object-group regions. Metadata is
/// never rearranged, so its original stream order remains independently usable.
pub fn top_level_registration_order(map: &Heightmap) -> Vec<usize> {
    let mut indices: Vec<_> = (0..map.regions.len()).collect();
    indices.sort_by_key(|&index| {
        let region = &map.regions[index];
        (
            !region.name.starts_with("ATP"),
            region.enclosing_grid[0],
            region.enclosing_grid[1],
            region.index_in_tile,
        )
    });
    indices
}

/// Samples the authored quad-diagonal cache for a region anchor. Native x87
/// plane arithmetic is evaluated in f64 and the stored height rounds to f32.
/// This deliberately does not change the renderer's existing height sampler.
/// Exact tile edges/out-of-tile anchors are outside the supported subset.
pub fn native_anchor_height(
    options: &TerrainOptions,
    tile: &TerrainTile,
    local: [f32; 2],
) -> Result<f32> {
    let q = options.quads_per_tile;
    let width = options.tile_size();
    if !(1..=512).contains(&q)
        || !options.units_per_vertex.is_finite()
        || options.units_per_vertex <= 0.
        || !width.is_finite()
        || !local.iter().all(|v| v.is_finite() && *v > 0. && *v < width)
    {
        return Err(invalid("unsupported region anchor or terrain dimensions"));
    }
    if tile.heights.len() != (q + 1) * (q + 1) || tile.quad_flags.len() != q * q {
        return Err(invalid("incomplete terrain grid for region anchor"));
    }
    let x = f64::from(local[0]) / f64::from(options.units_per_vertex);
    let y = f64::from(local[1]) / f64::from(options.units_per_vertex);
    let col = x.floor() as usize;
    let row = y.floor() as usize;
    if col >= q || row >= q {
        return Err(invalid("region anchor rounds beyond the terrain grid"));
    }
    let u = x - col as f64;
    let v = y - row as f64;
    let a = f64::from(tile.heights[row * (q + 1) + col]);
    let b = f64::from(tile.heights[row * (q + 1) + col + 1]);
    let c = f64::from(tile.heights[(row + 1) * (q + 1) + col + 1]);
    let d = f64::from(tile.heights[(row + 1) * (q + 1) + col]);
    let height = if tile.quad_flags[row * q + col] & 0x80 == 0 {
        if v <= u {
            a + u * (b - a) + v * (c - b)
        } else {
            a + u * (c - d) + v * (d - a)
        }
    } else if u + v <= 1. {
        a + u * (b - a) + v * (d - a)
    } else {
        c + (1. - u) * (d - c) + (1. - v) * (b - c)
    } as f32;
    if ![a, b, c, d].into_iter().all(f64::is_finite) || !height.is_finite() {
        return Err(invalid("non-finite region anchor height"));
    }
    Ok(height)
}

/// A bounded native DAT box, with its raw record still identified. Floating
/// point matrices approximate the native x87 implementation; exact faces use
/// inclusive comparisons, without an added epsilon or an unbounded water floor.
#[derive(Debug, Clone)]
pub struct NativeRegionBox {
    pub record_index: usize,
    pub name: String,
    pub raw_type: u32,
    pub effective_type: Option<u32>,
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    /// Truncated 512-unit angle, before wrapping to the trig table.
    pub yaw_units: i32,
    inverse: DMat3,
}

impl NativeRegionBox {
    /// Recovers one top-level authored box without claiming that all regions
    /// in its zone are decoded. Unsupported transforms are errors, never dry
    /// substitutes. This is useful for original-asset research fixtures even
    /// when the zone also contains unresolved object-group placements.
    pub fn from_record(map: &Heightmap, record_index: usize) -> Result<Self> {
        if !matches!(map.header[0], 20 | 21) {
            return Err(invalid("unsupported DAT region fixture version"));
        }
        let region = map
            .regions
            .get(record_index)
            .ok_or_else(|| invalid("missing region record"))?;
        region.validate_finite()?;
        // Native AFG construction equalizes horizontal extents before the
        // shared box builder. An ordinary box could expose water behind it.
        if region.name.starts_with("AFG") {
            return Err(invalid("unsupported AFG special box constructor"));
        }
        let tile = map
            .tiles
            .get(region.tile_index)
            .ok_or_else(|| invalid("missing region tile"))?;
        if region.enclosing_grid != [tile.longitude, tile.latitude]
            || region.grid.map(|v| i64::from(v) - 100000) != region.enclosing_grid.map(i64::from)
        {
            return Err(invalid("unsupported mismatched region grid"));
        }
        if region.scale != [1.; 3] {
            return Err(invalid("unsupported nonunit region scale"));
        }
        if region.rotation_degrees[..2] != [0., 0.] {
            return Err(invalid("unsupported tilted region"));
        }
        if region.full_size.iter().any(|v| *v <= 0.) {
            return Err(invalid("unsupported nonpositive region dimensions"));
        }
        let height =
            native_anchor_height(&map.options, tile, [region.position[0], region.position[1]])?;
        let width = f64::from(map.options.tile_size());
        let center = [
            (f64::from(tile.longitude) * width + f64::from(region.position[0])) as f32,
            (f64::from(tile.latitude) * width + f64::from(region.position[1])) as f32,
            height + region.position[2],
        ];
        let half_extents = region.full_size.map(|v| v * 0.5);
        let units = region.rotation_degrees[2] * (512_f32 / 360.);
        if !center.iter().all(|v| v.is_finite())
            || half_extents.contains(&0.)
            || !units.is_finite()
            || f64::from(units) < f64::from(i32::MIN)
            || f64::from(units) > f64::from(i32::MAX)
        {
            return Err(invalid("region transform overflow"));
        }
        let yaw_units = units.trunc() as i32;
        let (sin, cos) = table_sin_cos(yaw_units);
        let half = half_extents.map(f64::from);
        // Float32 lookup entries are not exactly orthonormal: invert the
        // scaled matrix, rather than substituting its transpose.
        let inverse = DMat3::from_cols(
            DVec3::new(cos * half[0], sin * half[0], 0.),
            DVec3::new(-sin * half[1], cos * half[1], 0.),
            DVec3::new(0., 0., half[2]),
        )
        .inverse();
        if !inverse.is_finite() {
            return Err(invalid("noninvertible region transform"));
        }
        Ok(Self {
            record_index,
            name: region.name.clone(),
            raw_type: region.type_id,
            effective_type: native_type_word(&region.name, region.type_id),
            center,
            half_extents,
            yaw_units,
            inverse,
        })
    }

    pub fn contains(&self, point: [f64; 3]) -> bool {
        if !point.into_iter().all(f64::is_finite) {
            return false;
        }
        let local = self.inverse * (DVec3::from(point) - DVec3::from(self.center.map(f64::from)));
        local.abs().cmple(DVec3::ONE).all()
    }

    /// Positive-length intersection with this finite box, including crossings
    /// whose endpoints are both outside. Fractions are along from + (to-from)t.
    /// This is a CPU geometry helper, not a recovered native movement routine.
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

/// Native query for zones whose complete top-level subset can be
/// represented. Every region participates, including unknown and dry kinds.
/// Any unsupported record fails construction; no possibly winning region is
/// silently omitted. Object groups require checked region-free definitions.
#[derive(Debug)]
pub struct NativeTopLevelRegions {
    pub boxes: Vec<NativeRegionBox>,
}

impl NativeTopLevelRegions {
    pub fn from_heightmap(map: &Heightmap) -> Result<Self> {
        if !map.groups.is_empty() {
            return Err(invalid(
                "unresolved embedded regions in object-group placements",
            ));
        }
        Self::from_top_level_records(map)
    }

    /// Allows placed groups only after checking every referenced TOG against
    /// the region-free grammar. Missing, malformed, unknown or area-bearing
    /// groups reject the entire set, preserving potentially winning dry areas.
    pub fn from_heightmap_with_groups(
        map: &Heightmap,
        mut resolve: impl FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let names: BTreeSet<_> = map.groups.iter().map(|group| &group.model).collect();
        for name in names {
            let data = resolve(name)?;
            validate_region_free_group(&data)
                .map_err(|error| invalid(&format!("object group {name}: {error}")))?;
        }
        Self::from_top_level_records(map)
    }

    fn from_top_level_records(map: &Heightmap) -> Result<Self> {
        if !matches!(map.header[0], 20 | 21) {
            return Err(invalid("unsupported DAT region fixture version"));
        }
        if map.region_count != map.regions.len() {
            return Err(invalid("inconsistent region metadata count"));
        }
        let mut grids = BTreeSet::new();
        for tile in &map.tiles {
            if !grids.insert([tile.longitude, tile.latitude]) {
                return Err(invalid("unsupported duplicate region tiles"));
            }
        }
        let mut record_positions = BTreeSet::new();
        for region in &map.regions {
            if !record_positions.insert((region.tile_index, region.index_in_tile)) {
                return Err(invalid("duplicate region source position"));
            }
        }
        let boxes = top_level_registration_order(map)
            .into_iter()
            .map(|index| NativeRegionBox::from_record(map, index))
            .collect::<Result<_>>()?;
        Ok(Self { boxes })
    }

    /// Native first-match selection, before gameplay classification. An
    /// explicit prefix gets a first pass; fallback excludes APV. Passing None
    /// reproduces the generic environment query. A selected effective_type of
    /// None means unsupported classification, not permission to try later boxes.
    pub fn at(
        &self,
        point: [f64; 3],
        preferred_prefix: Option<[u8; 3]>,
    ) -> Option<&NativeRegionBox> {
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

/// Proves absence of embedded areas in a deliberately narrow TOG subset.
/// Native 0x100fa159..0x100fa1fd appends only BEGIN_AREA records to the area
/// list; BEGIN_OBJECT populates a separate list. See EQG_LIQUID_TRANSFORMS.md.
/// Requiring complete blocks and known fields prevents ignored/truncated text
/// from becoming false evidence that a possibly overriding area is absent.
fn validate_region_free_group(data: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(data).map_err(|_| invalid("TOG is not UTF-8 text"))?;
    let mut started = false;
    let mut ended = false;
    let mut object_fields = None;
    for line in text.lines() {
        let words: Vec<_> = line.split_ascii_whitespace().collect();
        let Some(&key) = words.first() else {
            continue;
        };
        if key == "*BEGIN_AREA" || key == "*END_AREA" {
            return Err(invalid(
                "embedded group areas require verified parent transforms",
            ));
        }
        match key {
            "*BEGIN_OBJECTGROUP" if !started && words.len() == 1 => started = true,
            "*END_OBJECTGROUP"
                if started && !ended && object_fields.is_none() && words.len() == 1 =>
            {
                ended = true;
            }
            "*BEGIN_OBJECT" if started && !ended && object_fields.is_none() && words.len() == 1 => {
                object_fields = Some(0u8);
            }
            "*END_OBJECT" if object_fields == Some(0b11111) && words.len() == 1 => {
                object_fields = None;
            }
            _ => {
                let Some(fields) = object_fields.as_mut() else {
                    return Err(invalid(
                        "unsupported or malformed region-free TOG structure",
                    ));
                };
                let (bit, count, numeric) = match key {
                    "*NAME" => (1, 2, false),
                    "*POSITION" => (2, 4, true),
                    "*ROTATION" => (4, 4, true),
                    "*SCALE" => (8, 2, true),
                    "*FILE" if words.get(1) == Some(&"LIT") => (16, 3, false),
                    _ => return Err(invalid("unsupported region-free TOG field")),
                };
                if words.len() != count
                    || *fields & bit != 0
                    || words[1..].iter().any(|word| word.contains(['*', '\0']))
                    || (numeric
                        && words[1..].iter().any(|word| {
                            word.parse::<f32>().map_or(true, |value| !value.is_finite())
                        }))
                {
                    return Err(invalid("malformed region-free TOG field"));
                }
                *fields |= bit;
            }
        }
    }
    if !started || !ended || object_fields.is_some() {
        return Err(invalid("incomplete region-free TOG"));
    }
    Ok(())
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

fn invalid(message: &str) -> Error {
    Error::Format(format!("terrain regions: {message}"))
}
