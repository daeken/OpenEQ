//! CPU compatibility path for EQG terrain splatting. It keeps the renderer's
//! existing single-diffuse material interface; a future GPU terrain material can
//! consume the preserved heightfield, ECO layers and masks at full resolution.

use std::collections::HashMap;

use glam::Vec3;

use crate::Result;
use crate::mesh::{Geometry, Material};
use crate::texture::Texture;

use super::{EcoLayer, Ecosystems, Heightmap, TerrainTile};

/// Keep per-zone memory bounded: oldcommons has over 1,500 tiles. This is a
/// compatibility bake, not a reproduction of the client's detail shader.
const TILE_TEXTURE_SIZE: usize = 128;

pub struct BakedTerrain {
    pub materials: Vec<Material>,
    pub meshes: Vec<Geometry>,
    pub textures: Vec<Texture>,
}

pub fn bake<F>(map: &Heightmap, ecosystems: &Ecosystems, mut texture: F) -> Result<BakedTerrain>
where
    F: FnMut(&str) -> Option<Texture>,
{
    let mut source_textures = HashMap::new();
    for layers in ecosystems.values() {
        for layer in layers {
            if let Some(value) = texture(&layer.detail_map) {
                source_textures
                    .entry(layer.detail_map.to_ascii_lowercase())
                    .or_insert(value);
            }
        }
    }
    if let Some(value) = texture(&map.base_texture) {
        source_textures.insert(map.base_texture.to_ascii_lowercase(), value);
    }
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
    let mut result = BakedTerrain {
        materials: Vec::new(),
        meshes: Vec::new(),
        textures: Vec::new(),
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
                if tile.quad_flags[row * q + col] & 1 != 0 {
                    continue;
                }
                let a = (row * (q + 1) + col) as u32;
                let b = a + (q + 1) as u32;
                indices.extend([a, a + 1, b + 1, a, b + 1, b]);
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
        let rgba = paint_tile(map, tile, &normals, ecosystems, &source_textures);
        result.textures.push(Texture {
            name: name.clone(),
            width: TILE_TEXTURE_SIZE as u32,
            height: TILE_TEXTURE_SIZE as u32,
            rgba,
        });
        let material = result.materials.len();
        result.materials.push(Material {
            textures: vec![name],
            normal_map: None,
            water: None,
            flags: 0,
            anim_speed: 0,
            alpha_mask: false,
            transparent: false,
            emissive: false,
        });
        result.meshes.push(Geometry {
            vertices,
            indices,
            material,
            collidable: true,
        });
    }
    Ok(result)
}

fn paint_tile(
    map: &Heightmap,
    tile: &TerrainTile,
    normals: &[Vec3],
    ecosystems: &Ecosystems,
    textures: &HashMap<String, Texture>,
) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(TILE_TEXTURE_SIZE * TILE_TEXTURE_SIZE * 4);
    let q = map.options.quads_per_tile;
    for y in 0..TILE_TEXTURE_SIZE {
        for x in 0..TILE_TEXTURE_SIZE {
            let u = (x as f32 + 0.5) / TILE_TEXTURE_SIZE as f32;
            let v = (y as f32 + 0.5) / TILE_TEXTURE_SIZE as f32;
            let height = tile.height_at(
                &map.options,
                u * map.options.tile_size(),
                v * map.options.tile_size(),
            );
            let col = (u * q as f32).round() as usize;
            let row = (v * q as f32).round() as usize;
            let slope = normals[row * (q + 1) + col]
                .z
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees();
            let mut color = textures
                .get(&map.base_texture.to_ascii_lowercase())
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
