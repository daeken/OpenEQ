//! Parsers for the newer `.eqg` zone formats: `ZON`, `TER` and `MOD`.
//!
//! A modern zone is a `.zon` file that references a number of `.ter` (terrain)
//! and `.mod` (object) files, all stored inside a single `.eqg` archive. `TER`
//! and `MOD` share one layout; the only difference is a leading unused field
//! present in `MOD`.
//!
//! Unlike `WLD`, these files are flat indexed meshes with named materials, so
//! they need no fragment graph to be walked.

use std::collections::HashMap;

use crate::read::Reader;
use crate::{Error, Result};

/// Magic for a `.zon` zone description: the ASCII bytes "EQGZ".
pub const ZON_MAGIC: u32 = 0x5A47_5145;
/// Magic for a `.ter` terrain file: the ASCII bytes "EQGT".
pub const TER_MAGIC: u32 = 0x5447_5145;
/// Magic for a `.mod` object file: the ASCII bytes "EQGM".
pub const MOD_MAGIC: u32 = 0x4D47_5145;

/// A material property value.
#[derive(Debug, Clone)]
pub enum Property {
    Float(f32),
    /// On-disk type 1, copied as a 32-bit word by the native material reader.
    /// Kept distinct from type 3 colors/words; channel selection is not applied.
    IntegerBits(u32),
    Text(String),
    Uint(u32),
}

impl Property {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Property::Text(value) => Some(value),
            _ => None,
        }
    }
}

/// A named material with engine-specific properties.
#[derive(Debug, Clone)]
pub struct TerMaterial {
    /// Authored first word, retained as metadata. Polygon references do not use it.
    pub stored_id: u32,
    pub name: String,
    pub shader: String,
    pub properties: HashMap<String, Property>,
}

/// A parsed `.ter` or `.mod` file.
#[derive(Debug, Clone)]
pub struct TerMod {
    pub is_terrain: bool,
    pub version: u32,
    /// Every source record, in file order, including repeated IDs and names.
    pub materials: Vec<TerMaterial>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub tex_coords: Vec<[f32; 2]>,
    /// `(a, b, c, material_ordinal, flags)`; the ordinal indexes source records.
    pub polygons: Vec<(u32, u32, u32, u32, u32)>,
}

impl TerMod {
    /// Resolve a polygon's source ordinal as the native TER/MOD reader does:
    /// select that record, then the first record with the same exact name.
    /// Raw source records remain intact; invalid ordinals have no material.
    pub fn material_for_polygon(&self, ordinal: u32) -> Option<&TerMaterial> {
        let source = self.materials.get(ordinal as usize)?;
        self.materials
            .iter()
            .find(|material| material.name == source.name)
    }

    /// Index lists grouped by material index.
    pub fn mesh_groups(&self) -> HashMap<u32, Vec<u32>> {
        let mut groups: HashMap<u32, Vec<u32>> = HashMap::new();
        for &(a, b, c, material, _) in &self.polygons {
            groups.entry(material).or_default().extend([a, b, c]);
        }
        groups
    }
}

/// A placed object inside a zone.
#[derive(Debug, Clone)]
pub struct Placeable {
    pub object_id: i32,
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
}

/// A static light inside a zone.
#[derive(Debug, Clone)]
pub struct ZonLight {
    pub name: String,
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub radius: f32,
}

/// A parsed `.zon` file.
#[derive(Debug, Clone)]
pub struct ZoneFile {
    pub objects: Vec<TerMod>,
    pub placeables: Vec<Placeable>,
    pub lights: Vec<ZonLight>,
}

impl ZoneFile {
    /// Parses a `.zon` file, resolving its `.ter`/`.mod` embed references
    /// through `resolve`, which maps a file name to its bytes.
    pub fn parse<F>(data: &[u8], mut resolve: F) -> Result<Self>
    where
        F: FnMut(&str) -> Result<Vec<u8>>,
    {
        let mut reader = Reader::new(data);
        let magic = reader.u32()?;
        if magic != ZON_MAGIC {
            return Err(Error::BadMagic {
                found: magic,
                expected: ZON_MAGIC,
            });
        }
        let version = reader.u32()?;
        if !(1..=2).contains(&version) {
            return Err(Error::Format(format!("unsupported .zon version {version}")));
        }
        let string_size = reader.bounded_count()?;
        let object_count = reader.bounded_count()?;
        let placeable_count = reader.bounded_count()?;
        let unknown_count = reader.bounded_count()?;
        let light_count = reader.bounded_count()?;

        let strings: String = reader
            .take(string_size)?
            .iter()
            .map(|b| *b as char)
            .collect();

        let mut objects = Vec::with_capacity(object_count);
        for _ in 0..object_count {
            let name = string_at(&strings, reader.i32()? as usize);
            let lower = name.to_ascii_lowercase();
            let data = resolve(&lower)?;
            let is_terrain = lower.ends_with(".ter");
            objects.push(TerMod::parse(&data, is_terrain)?);
        }

        let mut placeables = Vec::with_capacity(placeable_count);
        for _ in 0..placeable_count {
            let object_id = reader.i32()?;
            let name = string_at(&strings, reader.i32()? as usize);
            let position = reader.vec3()?;
            // Stored as (around Z, around Y, around X); normalise to (X, Y, Z)
            // so both zone formats share one convention.
            let raw_rotation = reader.vec3()?;
            let rotation = [raw_rotation[2], raw_rotation[1], raw_rotation[0]];
            let scale = reader.f32()?;
            if version >= 2 {
                // Per-instance lighting data. Preserve stream alignment even
                // though the renderer currently uses dynamic zone lighting.
                let lighting_count = reader.bounded_count()?;
                reader.skip(
                    lighting_count
                        .checked_mul(4)
                        .ok_or_else(|| Error::Format(".zon lighting length overflow".into()))?,
                )?;
            }
            placeables.push(Placeable {
                object_id,
                name,
                position,
                rotation,
                scale,
            });
        }

        for _ in 0..unknown_count {
            reader.skip(4 + 12 + 12 + 12)?;
        }

        let mut lights = Vec::with_capacity(light_count);
        for _ in 0..light_count {
            let name = string_at(&strings, reader.i32()? as usize);
            let raw = reader.vec3()?;
            // Positions are stored as (y, x, z) and then y is negated.
            let position = [raw[1], -raw[0], raw[2]];
            let color = reader.vec3()?;
            let radius = reader.f32()?;
            lights.push(ZonLight {
                name,
                position,
                color,
                radius,
            });
        }

        Ok(Self {
            objects,
            placeables,
            lights,
        })
    }
}

impl TerMod {
    /// Parses a `.ter` (when `is_terrain`) or `.mod` payload.
    pub fn parse(data: &[u8], is_terrain: bool) -> Result<Self> {
        let mut reader = Reader::new(data);
        let magic = reader.u32()?;
        let expected = if is_terrain { TER_MAGIC } else { MOD_MAGIC };
        if magic != expected {
            return Err(Error::BadMagic {
                found: magic,
                expected,
            });
        }
        let version = reader.u32()?;
        let string_size = reader.bounded_count()?;
        let material_count = reader.bounded_count()?;
        let vertex_count = reader.bounded_count()?;
        let polygon_count = reader.bounded_count()?;
        if !is_terrain {
            reader.u32()?;
        }

        let strings: String = reader
            .take(string_size)?
            .iter()
            .map(|b| *b as char)
            .collect();

        let mut materials = Vec::with_capacity(material_count);
        for _ in 0..material_count {
            let stored_id = reader.u32()?;
            let name = string_at(&strings, reader.i32()? as usize);
            let shader = string_at(&strings, reader.i32()? as usize);
            let property_count = reader.bounded_count()?;
            let mut properties = HashMap::with_capacity(property_count);
            for _ in 0..property_count {
                let key = string_at(&strings, reader.i32()? as usize);
                let kind = reader.u32()?;
                let value = match kind {
                    0 => Property::Float(reader.f32()?),
                    // Native 0x1001538b dispatches type 1 to 0x10015444,
                    // retaining its word separately from type 3. See
                    // docs/EQG_MATERIAL_PROPERTIES.md for the original fixture.
                    1 => Property::IntegerBits(reader.u32()?),
                    2 => Property::Text(string_at(&strings, reader.i32()? as usize)),
                    3 => Property::Uint(reader.u32()?),
                    other => {
                        return Err(Error::Format(format!(
                            "unknown .ter material property type {other} for {key}"
                        )));
                    }
                };
                properties.insert(key, value);
            }
            materials.push(TerMaterial {
                stored_id,
                name,
                shader,
                properties,
            });
        }

        let has_extra = version == 3;
        let mut positions = Vec::with_capacity(vertex_count);
        let mut normals = Vec::with_capacity(vertex_count);
        let mut tex_coords = Vec::with_capacity(vertex_count);
        for _ in 0..vertex_count {
            positions.push(reader.vec3()?);
            normals.push(reader.vec3()?);
            if has_extra {
                reader.u32()?; // Packed vertex color.
            }
            tex_coords.push(reader.vec2()?);
            if has_extra {
                reader.skip(8)?; // Secondary coverage/detail UV set.
            }
        }

        let mut polygons = Vec::with_capacity(polygon_count);
        for _ in 0..polygon_count {
            let a = reader.u32()?;
            let b = reader.u32()?;
            let c = reader.u32()?;
            let material = reader.u32()?;
            let flags = reader.u32()?;
            polygons.push((a, b, c, material, flags));
        }

        Ok(Self {
            is_terrain,
            version,
            materials,
            positions,
            normals,
            tex_coords,
            polygons,
        })
    }
}

fn string_at(strings: &str, offset: usize) -> String {
    strings
        .get(offset..)
        .unwrap_or("")
        .split('\0')
        .next()
        .unwrap_or("")
        .to_owned()
}
