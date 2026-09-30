//! CPU compatibility path for EQG terrain splatting. Direct terrain rendering
//! consumes native sources and preserved recipes; ordinary diffuse materials
//! can request the same compatibility tile paint lazily or through eager bake.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, OnceLock};

use glam::Vec3;

use crate::Result;
use crate::mesh::{Geometry, Material};
use crate::texture::Texture;

use super::{
    EcoLayer, Ecosystems, Heightmap, MaterialLayer, TerrainMaterial, TerrainOptions, TerrainTile,
};

/// Keep per-zone memory bounded: oldcommons has over 1,500 tiles. This is a
/// compatibility bake, not a reproduction of the client's detail shader.
const TILE_TEXTURE_SIZE: usize = 128;

pub struct BakedTerrain {
    pub materials: Vec<Material>,
    pub meshes: Vec<Geometry>,
    pub textures: Vec<Texture>,
    pub terrain_materials: BTreeMap<usize, TerrainMaterial>,
}

/// Geometry and native sources are ready immediately. Compatibility tile
/// images are retained as recipes until a renderer actually requests them.
pub(crate) struct PreparedTerrain {
    pub materials: Vec<Material>,
    pub meshes: Vec<Geometry>,
    pub textures: Vec<Arc<Texture>>,
    pub terrain_materials: BTreeMap<usize, TerrainMaterial>,
    pub deferred_textures: Vec<DeferredTexture>,
}

struct PaintContext {
    options: TerrainOptions,
    base_texture: String,
    ecosystems: Ecosystems,
    textures: HashMap<String, Arc<Texture>>,
}

/// An immutable snapshot of the existing painter's inputs. Source images and
/// ecosystem definitions are shared across tiles; only the requested tile's
/// pixels are initialized, once, even when callers race.
pub(crate) struct DeferredTexture {
    pub name: String,
    tile: TerrainTile,
    normals: Vec<Vec3>,
    context: Arc<PaintContext>,
    cached: OnceLock<Texture>,
}

impl DeferredTexture {
    #[cfg(test)]
    pub(crate) fn is_cached(&self) -> bool {
        self.cached.get().is_some()
    }

    pub(crate) fn texture(&self) -> &Texture {
        self.cached.get_or_init(|| Texture {
            name: self.name.clone(),
            width: TILE_TEXTURE_SIZE as u32,
            height: TILE_TEXTURE_SIZE as u32,
            rgba: paint_tile(
                &self.context.options,
                &self.context.base_texture,
                &self.tile,
                &self.normals,
                &self.context.ecosystems,
                &self.context.textures,
            ),
        })
    }

    fn into_texture(self) -> Texture {
        let Self {
            name,
            tile,
            normals,
            context,
            cached,
        } = self;
        cached.into_inner().unwrap_or_else(|| Texture {
            name,
            width: TILE_TEXTURE_SIZE as u32,
            height: TILE_TEXTURE_SIZE as u32,
            rgba: paint_tile(
                &context.options,
                &context.base_texture,
                &tile,
                &normals,
                &context.ecosystems,
                &context.textures,
            ),
        })
    }
}

/// The public compatibility API remains eager and preserves texture ordering.
pub fn bake<F>(map: &Heightmap, ecosystems: &Ecosystems, texture: F) -> Result<BakedTerrain>
where
    F: FnMut(&str) -> Option<Texture>,
{
    let prepared = prepare(map, ecosystems, texture)?;
    let textures = prepared
        .deferred_textures
        .into_iter()
        .map(DeferredTexture::into_texture)
        // All per-tile contexts have been dropped before the shared sources
        // are consumed, so the eager path can recover their owned buffers.
        .chain(prepared.textures.into_iter().map(Arc::unwrap_or_clone))
        .collect();
    Ok(BakedTerrain {
        materials: prepared.materials,
        meshes: prepared.meshes,
        textures,
        terrain_materials: prepared.terrain_materials,
    })
}

pub(crate) fn prepare<F>(
    map: &Heightmap,
    ecosystems: &Ecosystems,
    mut texture: F,
) -> Result<PreparedTerrain>
where
    F: FnMut(&str) -> Option<Texture>,
{
    let mut names: Vec<_> = ecosystems
        .values()
        .flatten()
        .map(|layer| layer.detail_map.to_ascii_lowercase())
        .chain(std::iter::once(map.base_texture.to_ascii_lowercase()))
        .collect();
    names.sort();
    names.dedup();
    let mut source_textures = HashMap::new();
    for name in names {
        if let Some(mut value) = texture(&name) {
            value.name.clone_from(&name);
            source_textures.insert(name, Arc::new(value));
        }
    }
    let context = Arc::new(PaintContext {
        options: map.options.clone(),
        base_texture: map.base_texture.clone(),
        ecosystems: ecosystems.clone(),
        textures: source_textures,
    });
    let source_textures = &context.textures;
    let q = map.options.quads_per_tile;
    let step = map.options.units_per_vertex;
    // Neighbor-aware derivatives avoid normal seams at tile boundaries.
    let mut heights = HashMap::new();
    for tile in &map.tiles {
        for row in 0..=q {
            for col in 0..=q {
                heights.insert(
                    (
                        tile.longitude * q as i32 + col as i32,
                        tile.latitude * q as i32 + row as i32,
                    ),
                    tile.heights[row * (q + 1) + col],
                );
            }
        }
    }
    let mut result = PreparedTerrain {
        materials: Vec::new(),
        meshes: Vec::new(),
        textures: Vec::new(),
        terrain_materials: BTreeMap::new(),
        deferred_textures: Vec::new(),
    };
    for (tile_id, tile) in map.tiles.iter().enumerate() {
        // Client files sometimes retain unpainted editor tiles without any
        // base texture (e.g. the edge of Dead Hills). There is no drawable
        // material for those tiles, so do not turn them into magenta planes.
        if tile.layers.is_empty()
            && !source_textures.contains_key(&map.base_texture.to_ascii_lowercase())
        {
            continue;
        }
        let mut vertices = Vec::with_capacity((q + 1) * (q + 1) * 8);
        let mut normals = Vec::with_capacity((q + 1) * (q + 1));
        for row in 0..=q {
            for col in 0..=q {
                let x = tile.longitude * q as i32 + col as i32;
                let y = tile.latitude * q as i32 + row as i32;
                let h = tile.heights[row * (q + 1) + col];
                let left = heights.get(&(x - 1, y)).copied();
                let right = heights.get(&(x + 1, y)).copied();
                let down = heights.get(&(x, y - 1)).copied();
                let up = heights.get(&(x, y + 1)).copied();
                let dx = (right.unwrap_or(h) - left.unwrap_or(h))
                    / (step
                        * if left.is_some() && right.is_some() {
                            2.0
                        } else {
                            1.0
                        });
                let dy = (up.unwrap_or(h) - down.unwrap_or(h))
                    / (step
                        * if up.is_some() && down.is_some() {
                            2.0
                        } else {
                            1.0
                        });
                let normal = Vec3::new(-dx, -dy, 1.0).normalize();
                normals.push(normal);
                vertices.extend([x as f32 * step, y as f32 * step, h]);
                vertices.extend(normal.to_array());
                vertices.extend([col as f32 / q as f32, row as f32 / q as f32]);
            }
        }
        let mut indices = Vec::with_capacity(q * q * 6);
        for row in 0..q {
            for col in 0..q {
                let flags = tile.quad_flags[row * q + col];
                if flags & 1 != 0 {
                    continue;
                }
                let a = (row * (q + 1) + col) as u32;
                let b = a + (q + 1) as u32;
                // Match height_at and native collision's saved diagonal. The
                // native renderer may later regenerate this cache for LOD;
                // our full-grid mesh preserves the authored cached topology.
                if flags & 0x80 != 0 {
                    indices.extend([a, a + 1, b, a + 1, b + 1, b]);
                } else {
                    indices.extend([a, a + 1, b + 1, a, b + 1, b]);
                }
            }
        }
        if indices.is_empty() {
            continue;
        }
        let name = format!(
            "__terrain_{}_{}.rgba",
            map.options.name.to_ascii_lowercase(),
            tile_id
        );
        result.deferred_textures.push(DeferredTexture {
            name: name.clone(),
            tile: tile.clone(),
            normals,
            context: Arc::clone(&context),
            cached: OnceLock::new(),
        });
        let material = result.materials.len();
        result.terrain_materials.insert(
            material,
            TerrainMaterial {
                fallback_texture: name.clone(),
                base_texture: source_textures
                    .contains_key(&map.base_texture.to_ascii_lowercase())
                    .then(|| map.base_texture.to_ascii_lowercase()),
                layers: tile
                    .layers
                    .iter()
                    .map(|layer| MaterialLayer {
                        mask_size: layer.mask_size,
                        mask: layer.mask.clone(),
                        layers: ecosystems
                            .get(&layer.ecosystem.to_ascii_lowercase())
                            .cloned()
                            .unwrap_or_default(),
                    })
                    .collect(),
            },
        );
        result.materials.push(Material {
            textures: vec![name],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            emissive: false,
            clamp_uv: true,
        });
        result.meshes.push(Geometry {
            vertices,
            indices,
            material,
            collidable: true,
        });
    }
    // Share each successfully decoded source with Scene once, without copying
    // its pixels per tile or decoding it again when a fallback is requested.
    let mut sources: Vec<_> = source_textures.values().cloned().collect();
    sources.sort_by(|a, b| a.name.cmp(&b.name));
    result.textures.extend(sources);
    Ok(result)
}

fn paint_tile(
    options: &TerrainOptions,
    base_texture: &str,
    tile: &TerrainTile,
    normals: &[Vec3],
    ecosystems: &Ecosystems,
    textures: &HashMap<String, Arc<Texture>>,
) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(TILE_TEXTURE_SIZE * TILE_TEXTURE_SIZE * 4);
    let q = options.quads_per_tile;
    for y in 0..TILE_TEXTURE_SIZE {
        for x in 0..TILE_TEXTURE_SIZE {
            let u = (x as f32 + 0.5) / TILE_TEXTURE_SIZE as f32;
            let v = (y as f32 + 0.5) / TILE_TEXTURE_SIZE as f32;
            let height = tile.height_at(options, u * options.tile_size(), v * options.tile_size());
            let col = (u * q as f32).round() as usize;
            let row = (v * q as f32).round() as usize;
            let slope = normals[row * (q + 1) + col]
                .z
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            let mut color = textures
                .get(&base_texture.to_ascii_lowercase())
                .map(|t| sample_texture(t, u * 8.0, v * 8.0))
                .unwrap_or([1.0, 0.0, 1.0, 1.0]);
            for (index, layer) in tile.layers.iter().enumerate() {
                let Some(eco) = ecosystems.get(&layer.ecosystem.to_ascii_lowercase()) else {
                    continue;
                };
                let mut layer_color = None;
                for (eco_index, sublayer) in eco.iter().enumerate() {
                    let Some(texture) = textures.get(&sublayer.detail_map) else {
                        continue;
                    };
                    let sample = sample_texture(texture, u * sublayer.repeat, v * sublayer.repeat);
                    let alpha = if eco_index == 0 {
                        1.0
                    } else {
                        layer_weight(sublayer, height, slope)
                    };
                    layer_color =
                        Some(layer_color.map_or(sample, |prior| mix(prior, sample, alpha)));
                }
                if let Some(layer_color) = layer_color {
                    let opacity = if index == 0 {
                        1.0
                    } else {
                        sample_mask(&layer.mask, layer.mask_size, u, v)
                    };
                    color = mix(color, layer_color, opacity);
                }
            }
            rgba.extend(color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
        }
    }
    rgba
}

fn layer_weight(layer: &EcoLayer, height: f32, slope: f32) -> f32 {
    let range = |v: f32, min: f32, max: f32, tolerance: f32| {
        if tolerance <= 0.0 {
            if v >= min && v <= max { 1.0 } else { 0.0 }
        } else {
            ((v - min + tolerance) / tolerance).clamp(0.0, 1.0)
                * ((max + tolerance - v) / tolerance).clamp(0.0, 1.0)
        }
    };
    range(
        height,
        layer.min_height,
        layer.max_height,
        layer.height_tolerance,
    ) * range(
        slope,
        layer.min_slope,
        layer.max_slope,
        layer.slope_tolerance,
    )
}

fn mix(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

fn sample_texture(texture: &Texture, u: f32, v: f32) -> [f32; 4] {
    let x = u.rem_euclid(1.0) * texture.width as f32 - 0.5;
    let y = v.rem_euclid(1.0) * texture.height as f32 - 0.5;
    let tx = x.floor() as i32;
    let ty = y.floor() as i32;
    let texel = |x: i32, y: i32| {
        let x = x.rem_euclid(texture.width as i32) as usize;
        let y = y.rem_euclid(texture.height as i32) as usize;
        let offset = (y * texture.width as usize + x) * 4;
        std::array::from_fn(|i| texture.rgba[offset + i] as f32 / 255.0)
    };
    mix(
        mix(texel(tx, ty), texel(tx + 1, ty), x.fract().rem_euclid(1.0)),
        mix(
            texel(tx, ty + 1),
            texel(tx + 1, ty + 1),
            x.fract().rem_euclid(1.0),
        ),
        y.fract().rem_euclid(1.0),
    )
}

fn sample_mask(mask: &[u8], size: usize, u: f32, v: f32) -> f32 {
    if size == 0 {
        return 1.0;
    }
    let x = (u * size as f32 - 0.5).clamp(0.0, (size - 1) as f32);
    let y = (v * size as f32 - 0.5).clamp(0.0, (size - 1) as f32);
    let ix = x.floor() as usize;
    let iy = y.floor() as usize;
    let sample = |x: usize, y: usize| mask[y.min(size - 1) * size + x.min(size - 1)] as f32 / 255.0;
    let a = sample(ix, iy) + (sample(ix + 1, iy) - sample(ix, iy)) * x.fract();
    let b = sample(ix, iy + 1) + (sample(ix + 1, iy + 1) - sample(ix, iy + 1)) * x.fract();
    a + (b - a) * y.fract()
}
