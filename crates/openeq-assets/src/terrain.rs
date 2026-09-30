//! Heightmap zones (`EQTZP`, commonly called EQG v4).
//!
//! The text ZON describes the grid. The binary DAT contains tiled elevations,
//! ecosystem masks and placements; ECO and TOG files describe materials and
//! object groups. The layouts are independently checked against complete client
//! files and the EQEmu zone-utilities reader. See `docs/HEIGHTMAP_FORMAT.md` for
//! confirmed fields and rendering limitations.

use std::collections::{BTreeMap, HashMap};

use glam::{Mat4, Quat, Vec3};

use crate::mesh::WaterMaterial;
use crate::read::Reader;
use crate::{Error, Result};

mod bake;
pub use bake::{BakedTerrain, bake};
pub(crate) use bake::{DeferredTexture, PreparedTerrain, prepare};
pub mod indexed_water;
pub mod regions;
pub use regions::TerrainRegion;

#[derive(Debug, Clone)]
pub struct TerrainOptions {
    pub name: String,
    pub min_lng: i32,
    pub max_lng: i32,
    pub min_lat: i32,
    pub max_lat: i32,
    pub units_per_vertex: f32,
    pub quads_per_tile: usize,
}

impl TerrainOptions {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(data)
            .map_err(|_| Error::Format("terrain ZON is not text".into()))?;
        let tokens: Vec<_> = text.split_whitespace().collect();
        if tokens.first() != Some(&"EQTZP") {
            return Err(Error::Format("expected EQTZP heightmap zone".into()));
        }
        let field = |name| {
            tokens
                .iter()
                .position(|v| *v == name)
                .and_then(|i| tokens.get(i + 1))
                .copied()
                .ok_or_else(|| Error::Format(format!("missing terrain option {name}")))
        };
        let integer = |name| {
            field(name)?
                .parse::<i32>()
                .map_err(|_| Error::Format(format!("invalid terrain option {name}")))
        };
        let options = Self {
            name: field("*NAME")?.to_owned(),
            min_lng: integer("*MINLNG")?,
            max_lng: integer("*MAXLNG")?,
            min_lat: integer("*MINLAT")?,
            max_lat: integer("*MAXLAT")?,
            units_per_vertex: field("*UNITSPERVERT")?
                .parse()
                .map_err(|_| Error::Format("invalid vertex spacing".into()))?,
            quads_per_tile: integer("*QUADSPERTILE")? as usize,
        };
        if !(1..=512).contains(&options.quads_per_tile)
            || !options.units_per_vertex.is_finite()
            || options.units_per_vertex <= 0.0
            || options.min_lng > options.max_lng
            || options.min_lat > options.max_lat
        {
            return Err(Error::Format("invalid terrain grid dimensions".into()));
        }
        Ok(options)
    }

    pub fn tile_size(&self) -> f32 {
        self.units_per_vertex * self.quads_per_tile as f32
    }
}

#[derive(Debug, Clone)]
pub struct TerrainLayer {
    pub ecosystem: String,
    /// Base layer has no mask. Overlays are square row-major opacity images.
    pub mask_size: usize,
    pub mask: Vec<u8>,
}

/// The part of a tile's water record following its base elevation.
///
/// The native DAT reader uses a version gate: version 21 and newer records
/// contain an index and extension; version 20 contains a second float instead.
/// The raw bits are retained in both cases. Earlier versions are not fixtures.
#[derive(Debug, Clone, Default)]
pub struct TerrainWaterMetadata {
    pub word_bits: u32,
    pub extension: Option<TerrainWaterExtension>,
}

impl TerrainWaterMetadata {
    pub fn material_index(&self) -> Option<i32> {
        self.extension.as_ref().map(|_| self.word_bits as i32)
    }
}

#[derive(Debug, Clone)]
pub struct TerrainWaterExtension {
    /// Raw byte represented as i8, without normalizing it to a boolean. Any
    /// nonzero value, including a negative i8 value, enables the quartet.
    pub tag: i8,
    /// Authored local [xmin, xmax, ymin, ymax] rectangle. The native indexed
    /// mesh uses these extents; complete shoreline rendering is unverified.
    pub bounds: Option<[f32; 4]>,
    /// Meaning unknown. In particular, this is not a liquid bottom.
    pub trailing_value: f32,
}

#[derive(Debug, Clone)]
pub struct TerrainTile {
    /// Grid coordinates with the on-disk 100000 bias already removed.
    pub longitude: i32,
    pub latitude: i32,
    pub heights: Vec<f32>,
    pub colors: Vec<u32>,
    pub secondary_colors: Vec<u32>,
    /// Bit 0 hides the quad; bit 7 selects its cached negative-slope diagonal.
    pub quad_flags: Vec<u8>,
    pub water_level: f32,
    pub water_metadata: TerrainWaterMetadata,
    pub layers: Vec<TerrainLayer>,
}

impl TerrainTile {
    /// Piecewise planar height matching the authored quad-diagonal cache, for
    /// objects whose DAT Z is relative to the ground. Local X/Y are in world
    /// units and clamp to this tile's edges. Native adaptive LOD is not applied.
    pub fn height_at(&self, options: &TerrainOptions, x: f32, y: f32) -> f32 {
        let q = options.quads_per_tile;
        let gx = (x / options.units_per_vertex).clamp(0.0, q as f32);
        let gy = (y / options.units_per_vertex).clamp(0.0, q as f32);
        let col = (gx.floor() as usize).min(q - 1);
        let row = (gy.floor() as usize).min(q - 1);
        let x = gx - col as f32;
        let y = gy - row as f32;
        let a = self.heights[row * (q + 1) + col];
        let b = self.heights[(row + 1) * (q + 1) + col];
        let c = self.heights[(row + 1) * (q + 1) + col + 1];
        let d = self.heights[row * (q + 1) + col + 1];
        if self.quad_flags[row * q + col] & 0x80 != 0 {
            if x + y <= 1.0 {
                a + (d - a) * x + (b - a) * y
            } else {
                c + (1.0 - x) * (b - c) + (1.0 - y) * (d - c)
            }
        } else if y >= x {
            a + (b - a) * y + (c - b) * x
        } else {
            a + (d - a) * x + (c - d) * y
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerrainPlacement {
    pub model: String,
    pub transform: Mat4,
}

#[derive(Debug, Clone)]
pub struct TerrainLight {
    pub name: String,
    pub definition: String,
    pub position: [f32; 3],
    pub radius: f32,
}

/// Parsed heightmap data. Unrendered color/flag fields remain available for
/// future renderer work instead of being silently discarded.
#[derive(Debug, Clone)]
pub struct Heightmap {
    pub options: TerrainOptions,
    pub header: [u32; 3],
    pub base_texture: String,
    pub tiles: Vec<TerrainTile>,
    pub placements: Vec<TerrainPlacement>,
    pub lights: Vec<TerrainLight>,
    pub groups: Vec<TerrainPlacement>,
    /// Top-level DAT records in file order. Embedded group regions are not
    /// decoded here; liquid queries reject unresolved groups and transforms.
    pub regions: Vec<TerrainRegion>,
    pub region_count: usize,
}

impl Heightmap {
    pub fn parse(options: TerrainOptions, data: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(data);
        let header = [reader.u32()?, reader.u32()?, reader.u32()?];
        let base_texture = cstring(&mut reader)?;
        let count = reader.bounded_count()?;
        let q = options.quads_per_tile;
        let vertices = (q + 1) * (q + 1);
        let mut map = Self {
            options,
            header,
            base_texture,
            tiles: Vec::new(),
            placements: Vec::new(),
            lights: Vec::new(),
            groups: Vec::new(),
            regions: Vec::new(),
            region_count: 0,
        };
        for _ in 0..count {
            let longitude = reader.i32()? - 100000;
            let latitude = reader.i32()? - 100000;
            reader.i32()?; // Editor tile identifier.
            let mut tile = TerrainTile {
                longitude,
                latitude,
                heights: Vec::with_capacity(vertices),
                colors: Vec::with_capacity(vertices),
                secondary_colors: Vec::with_capacity(vertices),
                quad_flags: Vec::new(),
                water_level: 0.0,
                water_metadata: TerrainWaterMetadata::default(),
                layers: Vec::new(),
            };
            for _ in 0..vertices {
                let h = reader.f32()?;
                if !h.is_finite() {
                    return Err(Error::Format("non-finite terrain height".into()));
                }
                tile.heights.push(h);
            }
            for _ in 0..vertices {
                tile.colors.push(reader.u32()?);
            }
            for _ in 0..vertices {
                tile.secondary_colors.push(reader.u32()?);
            }
            tile.quad_flags = reader.take(q * q)?.to_vec();
            tile.water_level = reader.f32()?;
            tile.water_metadata.word_bits = reader.u32()?;
            // Native full DAT reader 0x10100f09..0x10101029 gates this by
            // version, not index sign; its byte test is != 0. See the static
            // evidence in docs/HEIGHTMAP_WATER_REVERSE_ENGINEERING.md.
            if header[0] >= 21 {
                let tag = reader.i8()?;
                let bounds = if tag != 0 {
                    Some([reader.f32()?, reader.f32()?, reader.f32()?, reader.f32()?])
                } else {
                    None
                };
                tile.water_metadata.extension = Some(TerrainWaterExtension {
                    tag,
                    bounds,
                    trailing_value: reader.f32()?,
                });
            }
            let layers = reader.bounded_count()?;
            for layer in 0..layers {
                let ecosystem = cstring(&mut reader)?;
                let mask_size = if layer == 0 {
                    0
                } else {
                    reader.bounded_count()?
                };
                if mask_size > 4096 {
                    return Err(Error::Format("terrain mask is too large".into()));
                }
                let mask = reader.take(mask_size * mask_size)?.to_vec();
                tile.layers.push(TerrainLayer {
                    ecosystem,
                    mask_size,
                    mask,
                });
            }
            let tile_origin = Vec3::new(
                longitude as f32 * map.options.tile_size(),
                latitude as f32 * map.options.tile_size(),
                0.0,
            );
            let singles = reader.bounded_count()?;
            for _ in 0..singles {
                let model = cstring(&mut reader)?.to_ascii_lowercase();
                cstring(&mut reader)?; // Ecosystem membership.
                reader.skip(8)?; // Repeated longitude/latitude.
                let mut position = Vec3::from_array(reader.vec3()?);
                let rotation = reader.vec3()?;
                let scale = reader.vec3()?;
                reader.u8()?;
                if header[0] & 2 != 0 {
                    reader.u32()?;
                }
                position.z += tile.height_at(
                    &map.options,
                    position.x.rem_euclid(map.options.tile_size()),
                    position.y.rem_euclid(map.options.tile_size()),
                );
                position += tile_origin;
                map.placements.push(TerrainPlacement {
                    model,
                    transform: transform(position, rotation, scale),
                });
            }
            let regions = reader.bounded_count()?;
            map.region_count += regions;
            for index in 0..regions {
                map.regions.push(TerrainRegion::read(
                    &mut reader,
                    map.tiles.len(),
                    index,
                    [longitude, latitude],
                )?);
            }
            let lights = reader.bounded_count()?;
            for _ in 0..lights {
                let name = cstring(&mut reader)?;
                let definition = cstring(&mut reader)?;
                reader.u8()?;
                reader.skip(8)?;
                let mut position = Vec3::from_array(reader.vec3()?);
                reader.skip(24)?; // Euler rotation and scale (unused for points).
                let radius = reader.f32()?;
                position.z += tile.height_at(&map.options, position.x, position.y);
                position += tile_origin;
                map.lights.push(TerrainLight {
                    name,
                    definition,
                    position: position.to_array(),
                    radius,
                });
            }
            let groups = reader.bounded_count()?;
            for _ in 0..groups {
                let model = cstring(&mut reader)?.to_ascii_lowercase();
                reader.skip(8)?;
                let mut position = Vec3::from_array(reader.vec3()?);
                let rotation = reader.vec3()?;
                let scale = reader.vec3()?;
                let z_adjust = reader.f32()?;
                position += tile_origin;
                position.z += scale[2] * z_adjust;
                map.groups.push(TerrainPlacement {
                    model,
                    transform: transform(position, rotation, scale),
                });
            }
            map.tiles.push(tile);
        }
        if reader.remaining() != 0 {
            return Err(Error::Format(format!(
                "{} unparsed bytes after terrain DAT",
                reader.remaining()
            )));
        }
        Ok(map)
    }
}

/// An ECO texture layer. Object/flora recipes are retained in the source files
/// but are not synthesized: explicit DAT objects are the authored placements.
#[derive(Debug, Clone)]
pub struct EcoLayer {
    pub detail_map: String,
    pub repeat: f32,
    pub min_height: f32,
    pub max_height: f32,
    pub height_tolerance: f32,
    pub min_slope: f32,
    pub max_slope: f32,
    pub slope_tolerance: f32,
}

/// Direct terrain texture recipe, keyed by an emitted scene material. This
/// preserves the compatibility baker's ordered detail/range inputs; it does
/// not claim to reproduce the native coverage, blend or normal-map shader.
#[derive(Debug, Clone)]
pub struct TerrainMaterial {
    /// Identity of the original compatibility material. Consumers must check
    /// this against the current material before applying an indexed recipe.
    pub fallback_texture: String,
    /// Present only when the source image actually decoded. A valid first
    /// ecosystem can replace an absent base (original Dead Hills uses `None`).
    pub base_texture: Option<String>,
    pub layers: Vec<MaterialLayer>,
}

/// One DAT ecosystem application, kept in source order. The first tile layer
/// has no mask; later layers contain square, row-major opacity bytes.
#[derive(Debug, Clone)]
pub struct MaterialLayer {
    pub mask_size: usize,
    pub mask: Vec<u8>,
    /// Original ECO order, including references whose image did not decode.
    /// Consumers must validate the whole recipe before replacing its bake.
    pub layers: Vec<EcoLayer>,
}

pub fn parse_ecosystem(data: &[u8]) -> Result<Vec<EcoLayer>> {
    let text = std::str::from_utf8(data).map_err(|_| Error::Format("ECO is not text".into()))?;
    let mut layers = Vec::new();
    let mut active = false;
    let mut layer = None;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next().unwrap_or("") {
            "*TEXTUREPART" => active = true,
            "*END_TEXTUREPART" => break,
            "*LAYER" if active => {
                layer = Some(EcoLayer {
                    detail_map: String::new(),
                    repeat: 1.0,
                    min_height: -10000.0,
                    max_height: 10000.0,
                    height_tolerance: 0.0,
                    min_slope: 0.0,
                    max_slope: 90.0,
                    slope_tolerance: 0.0,
                })
            }
            "*END_LAYER" if active => {
                if let Some(value) = layer.take()
                    && !value.detail_map.is_empty()
                {
                    layers.push(value);
                }
            }
            key if active => {
                if let Some(layer) = &mut layer {
                    let value = words.next().unwrap_or("");
                    if key == "*DETAILMAP" {
                        layer.detail_map = value.replace('\\', "/").to_ascii_lowercase();
                    } else if let Ok(value) = value.parse::<f32>() {
                        match key {
                            "*DETAILREPEAT" => layer.repeat = value.max(0.01),
                            "*MINHEIGHT" => layer.min_height = value,
                            "*MAXHEIGHT" => layer.max_height = value,
                            "*HEIGHTTOL" => layer.height_tolerance = value,
                            "*MINSLOPE" => layer.min_slope = value,
                            "*MAXSLOPE" => layer.max_slope = value,
                            "*SLOPETOL" => layer.slope_tolerance = value,
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(layers)
}

pub fn parse_object_group(data: &[u8]) -> Result<Vec<TerrainPlacement>> {
    let text = std::str::from_utf8(data).map_err(|_| Error::Format("TOG is not text".into()))?;
    let mut instances = Vec::new();
    let mut model = String::new();
    let mut position = Vec3::ZERO;
    let mut rotation = [0.0; 3];
    let mut scale = [1.0; 3];
    for line in text.lines() {
        let words: Vec<_> = line.split_whitespace().collect();
        let Some(&key) = words.first() else {
            continue;
        };
        match key {
            "*BEGIN_OBJECT" => {
                model.clear();
                position = Vec3::ZERO;
                rotation = [0.0; 3];
                scale = [1.0; 3];
            }
            "*NAME" if words.len() >= 2 => model = words[1].to_ascii_lowercase(),
            "*POSITION" if words.len() >= 4 => {
                position = Vec3::from_array(parse_three(&words[1..4])?)
            }
            "*ROTATION" if words.len() >= 4 => rotation = parse_three(&words[1..4])?,
            "*SCALE" if words.len() >= 2 => scale = [number(words[1])?; 3],
            "*END_OBJECT" if !model.is_empty() => instances.push(TerrainPlacement {
                model: model.clone(),
                transform: transform(position, rotation, scale),
            }),
            _ => {}
        }
    }
    Ok(instances)
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaterSheet {
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub height: f32,
    pub uv_scale: f32,
    pub normal_map: String,
    pub material: WaterMaterial,
}

/// Finite water sheets, retaining the existing rendering defaults. Use
/// [`parse_water_data`] to also preserve indexed material definitions.
pub fn parse_water(data: &[u8]) -> Result<Vec<WaterSheet>> {
    let text =
        std::str::from_utf8(data).map_err(|_| Error::Format("water DAT is not text".into()))?;
    let mut sheets = Vec::new();
    let mut fields = BTreeMap::<String, Vec<String>>::new();
    let mut active = false;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let key = words.next().unwrap_or("");
        if key == "*WATERSHEET" {
            fields.clear();
            active = true;
        } else if key == "*END_SHEET" && active {
            let scalar = |key: &str, default| {
                fields
                    .get(key)
                    .and_then(|v| v.first())
                    .and_then(|v| v.parse::<f32>().ok())
                    .unwrap_or(default)
            };
            let color = |key: &str, default| {
                fields
                    .get(key)
                    .and_then(|v| {
                        v.iter()
                            .map(|v| v.parse::<f32>().ok())
                            .collect::<Option<Vec<_>>>()
                    })
                    .and_then(|v| <[f32; 4]>::try_from(v).ok())
                    .unwrap_or(default)
            };
            let filename = |key: &str| {
                fields.get(key).and_then(|v| v.first()).map(|v| {
                    v.rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(v)
                        .to_ascii_lowercase()
                })
            };
            sheets.push(WaterSheet {
                min: [scalar("*MINX", 0.0), scalar("*MINY", 0.0)],
                max: [scalar("*MAXX", 0.0), scalar("*MAXY", 0.0)],
                height: scalar("*ZHEIGHT", 0.0),
                uv_scale: scalar("*UVSCALE", 1.0),
                normal_map: filename("*NORMALMAP").unwrap_or("water_n.dds".into()),
                material: WaterMaterial {
                    indexed_uv_scale: None,
                    color1: color("*WATERCOLOR1", [0.1, 0.2, 0.2, 1.0]),
                    color2: color("*WATERCOLOR2", [0.2, 0.3, 0.3, 1.0]),
                    reflection_color: color("*REFLECTIONCOLOR", [0.5, 0.6, 0.6, 1.0]),
                    fresnel_bias: scalar("*FRESNELBIAS", 0.25),
                    fresnel_power: scalar("*FRESNELPOWER", 8.0),
                    reflection_amount: scalar("*REFLECTIONAMOUNT", 0.5),
                    environment_map: filename("*ENVIRONMENTMAP"),
                },
            });
            active = false;
        } else if active {
            fields.insert(key.to_owned(), words.map(str::to_owned).collect());
        }
    }
    Ok(sheets)
}

/// An authored field, including unknown fields and repeated occurrences.
/// Whitespace is tokenized; keys, values and their order are retained exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaterField {
    pub key: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IndexedWaterDefinition {
    pub index: i32,
    pub uv_scale: f32,
    /// Authored path, without basename extraction or case normalization.
    pub normal_map: String,
    /// The environment map likewise retains its authored path. No rendering
    /// or UV behavior is inferred by decoding these material parameters.
    pub material: WaterMaterial,
    pub fields: Vec<WaterField>,
}

#[derive(Debug, Clone)]
pub struct WaterData {
    pub finite_sheets: Vec<WaterSheet>,
    /// In file order, including repeated indices and identical duplicates.
    pub indexed: Vec<IndexedWaterDefinition>,
}

#[derive(Debug)]
pub enum IndexedWaterResolution<'a> {
    Missing,
    Unique {
        definition: &'a IndexedWaterDefinition,
        /// Identical authored definitions resolve together without removing
        /// any records from [`WaterData::indexed`].
        occurrences: usize,
    },
    Ambiguous {
        definitions: Vec<&'a IndexedWaterDefinition>,
    },
}

impl WaterData {
    /// Resolves this exact index. Never substitutes index zero or the first
    /// definition. Equality includes all authored fields, so unknown differing
    /// values also prevent duplicate coalescing.
    pub fn resolve_index(&self, index: i32) -> IndexedWaterResolution<'_> {
        let definitions: Vec<_> = self
            .indexed
            .iter()
            .filter(|definition| definition.index == index)
            .collect();
        let Some(&first) = definitions.first() else {
            return IndexedWaterResolution::Missing;
        };
        if definitions.iter().all(|definition| *definition == first) {
            IndexedWaterResolution::Unique {
                definition: first,
                occurrences: definitions.len(),
            }
        } else {
            IndexedWaterResolution::Ambiguous { definitions }
        }
    }
}

/// Parses finite sheets and indexed definitions as separate record types.
/// Indexed records have no implicit material defaults: missing, duplicate or
/// malformed required fields are errors. Unknown fields remain available.
pub fn parse_water_data(data: &[u8]) -> Result<WaterData> {
    let text =
        std::str::from_utf8(data).map_err(|_| Error::Format("water DAT is not text".into()))?;
    let mut indexed = Vec::new();
    let mut active = None::<Vec<WaterField>>;
    let mut finite_active = false;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let Some(key) = words.next() else {
            continue;
        };
        match key {
            "*WATERSHEETDATA" => {
                if active.is_some() || finite_active {
                    return Err(Error::Format("nested indexed water definition".into()));
                }
                active = Some(Vec::new());
            }
            "*ENDWATERSHEETDATA" => {
                let fields = active.take().ok_or_else(|| {
                    Error::Format("indexed water end without a definition".into())
                })?;
                indexed.push(parse_indexed_water_definition(fields)?);
            }
            "*BEGIN_WATERSHEETDATA" | "*END_WATERSHEETDATA" | "*WATERSHEET" | "*END_SHEET"
                if active.is_some() =>
            {
                return Err(Error::Format(
                    "unterminated indexed water definition".into(),
                ));
            }
            "*WATERSHEET" => finite_active = true,
            "*END_SHEET" => finite_active = false,
            _ => {
                if let Some(fields) = &mut active {
                    fields.push(WaterField {
                        key: key.to_owned(),
                        values: words.map(str::to_owned).collect(),
                    });
                }
            }
        }
    }
    if active.is_some() {
        return Err(Error::Format(
            "unterminated indexed water definition".into(),
        ));
    }
    Ok(WaterData {
        finite_sheets: parse_water(data)?,
        indexed,
    })
}

fn parse_indexed_water_definition(fields: Vec<WaterField>) -> Result<IndexedWaterDefinition> {
    let field = |key: &str, count: usize| {
        let mut matches = fields.iter().filter(|field| field.key == key);
        let field = matches
            .next()
            .ok_or_else(|| Error::Format(format!("missing indexed water field {key}")))?;
        if matches.next().is_some() || field.values.len() != count {
            return Err(Error::Format(format!("invalid indexed water field {key}")));
        }
        Ok(field.values.as_slice())
    };
    let scalar = |key| number(&field(key, 1)?[0]);
    let color = |key| -> Result<[f32; 4]> {
        let values = field(key, 4)?;
        Ok([
            number(&values[0])?,
            number(&values[1])?,
            number(&values[2])?,
            number(&values[3])?,
        ])
    };
    let definition = IndexedWaterDefinition {
        index: field("*INDEX", 1)?[0]
            .parse()
            .map_err(|_| Error::Format("invalid indexed water index".into()))?,
        uv_scale: scalar("*UVSCALE")?,
        normal_map: field("*NORMALMAP", 1)?[0].clone(),
        material: WaterMaterial {
            indexed_uv_scale: None,
            color1: color("*WATERCOLOR1")?,
            color2: color("*WATERCOLOR2")?,
            reflection_color: color("*REFLECTIONCOLOR")?,
            fresnel_bias: scalar("*FRESNELBIAS")?,
            fresnel_power: scalar("*FRESNELPOWER")?,
            reflection_amount: scalar("*REFLECTIONAMOUNT")?,
            environment_map: Some(field("*ENVIRONMENTMAP", 1)?[0].clone()),
        },
        fields,
    };
    Ok(definition)
}

pub fn parse_light_color(data: &[u8]) -> Option<[f32; 3]> {
    let text = std::str::from_utf8(data).ok()?;
    let words: Vec<_> = text.split_whitespace().collect();
    let get = |key| {
        words
            .iter()
            .position(|word| *word == key)
            .and_then(|i| words.get(i + 1))
            .copied()
    };
    let color = get("*COLOR")?.parse::<i64>().ok()? as u32;
    let intensity = get("*INTENSITY")
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(1.0);
    Some([
        ((color >> 16) & 255) as f32 / 255.0 * intensity,
        ((color >> 8) & 255) as f32 / 255.0 * intensity,
        (color & 255) as f32 / 255.0 * intensity,
    ])
}

fn cstring(reader: &mut Reader<'_>) -> Result<String> {
    let mut bytes = Vec::new();
    loop {
        let b = reader.u8()?;
        if b == 0 {
            break;
        }
        bytes.push(b);
    }
    String::from_utf8(bytes).map_err(|_| Error::Format("invalid terrain string".into()))
}

fn number(value: &str) -> Result<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| Error::Format(format!("invalid terrain number {value}")))
}
fn parse_three(values: &[&str]) -> Result<[f32; 3]> {
    Ok([number(values[0])?, number(values[1])?, number(values[2])?])
}
fn transform(position: Vec3, degrees: [f32; 3], scale: [f32; 3]) -> Mat4 {
    let rotation = Quat::from_rotation_z(degrees[2].to_radians())
        * Quat::from_rotation_y(degrees[1].to_radians())
        * Quat::from_rotation_x(degrees[0].to_radians());
    Mat4::from_scale_rotation_translation(Vec3::from_array(scale), rotation, position)
}

/// Named ecosystems collected once per zone for terrain texturing.
pub type Ecosystems = HashMap<String, Vec<EcoLayer>>;
