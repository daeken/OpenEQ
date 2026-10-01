//! Shared terrain detail textures and bounded paint recipes. Baked tiles remain
//! available when a recipe or device budget is unsupported. ECO weights retain
//! the compatibility interpretation; native shader equivalence is not assumed.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, ensure};
use bytemuck::{Pod, Zeroable};
use openeq_assets::Scene;

const BUDGET: u64 = 128 * 1024 * 1024;
const MAX_MASK_BYTES: usize = 32 * 1024 * 1024;
const MAX_DETAIL_SIZE: u32 = 2048;
const MAX_LAYERS: usize = 32;
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TerrainMode {
    #[default]
    Direct,
    Baked,
}

#[derive(Clone, Debug, Default)]
pub struct TerrainStats {
    pub materials: usize,
    pub detail_layers: usize,
    pub detail_size: u32,
    pub texture_bytes: u64,
    pub buffer_bytes: u64,
    pub mask_bytes: u64,
    pub fallback_reason: Option<String>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Header {
    // First flattened layer, layer count, base detail layer, reserved.
    words: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Layer {
    // Detail index, mask word offset, mask size, flags (first/last ECO entry).
    words: [u32; 4],
    // Repeat, minimum height, maximum height, height tolerance.
    height: [f32; 4],
    // Minimum slope, maximum slope, slope tolerance, reserved.
    slope: [f32; 4],
}

struct Plan {
    admitted: BTreeSet<usize>,
    headers: Vec<Header>,
    layers: Vec<Layer>,
    masks: Vec<u32>,
    details: Vec<image::RgbaImage>,
    size: u32,
    stats: TerrainStats,
}

impl Plan {
    /// Validate aggregate recipe sizes before allocating any flattened arrays.
    /// A per-tile cap alone still permits an unbounded number of valid tiles.
    fn preflight(scene: &Scene, limits: &wgpu::Limits) -> anyhow::Result<[u64; 3]> {
        let header_bytes = (scene.materials.len() as u64)
            .checked_mul(16)
            .context("terrain header overflow")?
            .max(16);
        ensure!(
            header_bytes <= limits.max_storage_buffer_binding_size
                && header_bytes <= limits.max_buffer_size
                && header_bytes <= BUDGET,
            "terrain material buffer exceeds limit"
        );
        let mut layer_count = 0u64;
        let mut mask_bytes = 0u64;
        for (&index, recipe) in &scene.terrain_materials {
            ensure!(
                index < scene.materials.len(),
                "terrain material index is invalid"
            );
            ensure!(
                recipe.layers.len() <= MAX_LAYERS,
                "too many terrain paint layers"
            );
            let mut count = 0usize;
            for (i, paint) in recipe.layers.iter().enumerate() {
                count = count
                    .checked_add(paint.layers.len())
                    .context("terrain detail count overflow")?;
                ensure!(count <= MAX_LAYERS, "too many terrain detail layers");
                if i != 0 && paint.mask_size != 0 {
                    ensure!(
                        paint.mask_size <= 512 && paint.mask_size.is_power_of_two(),
                        "unsupported terrain mask dimensions"
                    );
                    ensure!(
                        paint.mask.len() == paint.mask_size * paint.mask_size,
                        "incomplete terrain mask"
                    );
                    // Power-of-two square levels are word-sized except 1x1.
                    mask_bytes = mask_bytes
                        .checked_add(mip_bytes(paint.mask_size as u32, 1) + 3)
                        .context("terrain mask size overflow")?;
                    ensure!(
                        mask_bytes <= MAX_MASK_BYTES as u64,
                        "terrain masks exceed CPU budget"
                    );
                }
            }
            layer_count = layer_count
                .checked_add(count as u64)
                .context("terrain layer count overflow")?;
        }
        let sizes = [
            header_bytes,
            layer_count
                .max(1)
                .checked_mul(48)
                .context("terrain layer size overflow")?,
            mask_bytes.max(4),
        ];
        ensure!(
            sizes
                .iter()
                .all(|&size| size <= limits.max_storage_buffer_binding_size
                    && size <= limits.max_buffer_size),
            "terrain storage exceeds device limit"
        );
        ensure!(
            sizes.iter().sum::<u64>() <= BUDGET,
            "terrain recipes exceed memory budget"
        );
        Ok(sizes)
    }

    fn empty(reason: Option<String>) -> Self {
        Self {
            admitted: BTreeSet::new(),
            headers: vec![Header::zeroed()],
            layers: vec![Layer::zeroed()],
            masks: vec![0],
            details: vec![image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4]))],
            size: 1,
            stats: TerrainStats {
                fallback_reason: reason,
                ..Default::default()
            },
        }
    }

    fn prepare(scene: &Scene, limits: &wgpu::Limits) -> anyhow::Result<Self> {
        if scene.terrain_materials.is_empty() {
            return Ok(Self::empty(None));
        }
        let sizes = Self::preflight(scene, limits)?;
        let mut plan = Self::empty(None);
        let header_bytes = sizes[0];
        plan.stats.buffer_bytes = sizes.iter().sum();
        plan.headers = vec![Header::zeroed(); scene.materials.len()];
        plan.layers.clear();
        plan.masks.clear();
        plan.details.clear();
        let mut textures = BTreeMap::<String, u32>::new();
        for (&index, recipe) in &scene.terrain_materials {
            let material = scene
                .materials
                .get(index)
                .context("terrain material index is invalid")?;
            ensure!(
                material.textures == [recipe.fallback_texture.clone()],
                "terrain fallback identity changed"
            );
            ensure!(
                material.water.is_none()
                    && !material.transparent
                    && !material.additive
                    && material.waterfall.is_none()
                    && !material.alpha_mask
                    && !material.emissive,
                "terrain recipe requires an opaque surface"
            );
            ensure!(
                recipe.layers.len() <= MAX_LAYERS,
                "too many terrain paint layers"
            );
            let count: usize = recipe.layers.iter().map(|layer| layer.layers.len()).sum();
            ensure!(count <= MAX_LAYERS, "too many terrain detail layers");
            ensure!(
                count > 0 || recipe.base_texture.is_some(),
                "terrain has no resolved detail"
            );
            let first = plan.layers.len() as u32;
            let base = match &recipe.base_texture {
                Some(name) => plan.detail(scene, name, &mut textures, limits)?,
                None => NONE,
            };
            for (paint_index, paint) in recipe.layers.iter().enumerate() {
                ensure!(!paint.layers.is_empty(), "empty terrain ecosystem");
                let (mask_offset, mask_size) = if paint_index == 0 || paint.mask_size == 0 {
                    (0, 0)
                } else {
                    ensure!(
                        paint.mask_size <= 512 && paint.mask_size.is_power_of_two(),
                        "unsupported terrain mask dimensions"
                    );
                    ensure!(
                        paint.mask.len() == paint.mask_size * paint.mask_size,
                        "incomplete terrain mask"
                    );
                    let bytes = mip_bytes(paint.mask_size as u32, 1);
                    ensure!(
                        (plan.masks.len() * 4) as u64 + bytes + 16 <= MAX_MASK_BYTES as u64,
                        "terrain masks exceed CPU budget"
                    );
                    let offset = plan.masks.len() as u32;
                    append_mask(&mut plan.masks, &paint.mask, paint.mask_size);
                    (offset, paint.mask_size as u32)
                };
                for (i, eco) in paint.layers.iter().enumerate() {
                    let values = [
                        eco.repeat,
                        eco.min_height,
                        eco.max_height,
                        eco.height_tolerance,
                        eco.min_slope,
                        eco.max_slope,
                        eco.slope_tolerance,
                    ];
                    ensure!(
                        values.iter().all(|v| v.is_finite())
                            && eco.repeat > 0.
                            && eco.min_height <= eco.max_height
                            && eco.min_slope <= eco.max_slope,
                        "unsupported terrain ecosystem range"
                    );
                    let detail = plan.detail(scene, &eco.detail_map, &mut textures, limits)?;
                    let flags = u32::from(i == 0) | (u32::from(i + 1 == paint.layers.len()) << 1);
                    plan.layers.push(Layer {
                        words: [detail, mask_offset, mask_size, flags],
                        height: [
                            eco.repeat,
                            eco.min_height,
                            eco.max_height,
                            eco.height_tolerance,
                        ],
                        slope: [eco.min_slope, eco.max_slope, eco.slope_tolerance, 0.],
                    });
                }
            }
            plan.headers[index].words = [first, plan.layers.len() as u32 - first, base, 0];
            plan.admitted.insert(index);
        }
        let texture_bytes = mip_bytes(plan.size, 4) * plan.details.len() as u64;
        let buffer_sizes = [
            header_bytes,
            (plan.layers.len().max(1) * 48) as u64,
            (plan.masks.len().max(1) * 4) as u64,
        ];
        ensure!(
            buffer_sizes
                .iter()
                .all(|&size| size <= limits.max_storage_buffer_binding_size
                    && size <= limits.max_buffer_size),
            "terrain storage exceeds device limit"
        );
        let buffer_bytes = buffer_sizes.iter().sum::<u64>();
        ensure!(
            texture_bytes + buffer_bytes <= BUDGET,
            "terrain exceeds GPU memory budget"
        );
        plan.stats = TerrainStats {
            materials: plan.admitted.len(),
            detail_layers: plan.details.len(),
            detail_size: plan.size,
            texture_bytes,
            buffer_bytes,
            mask_bytes: buffer_sizes[2],
            fallback_reason: None,
        };
        // WGSL bindings remain valid even when all recipes use only a base map.
        if plan.layers.is_empty() {
            plan.layers.push(Layer::zeroed());
        }
        if plan.masks.is_empty() {
            plan.masks.push(0);
        }
        Ok(plan)
    }

    fn detail(
        &mut self,
        scene: &Scene,
        name: &str,
        known: &mut BTreeMap<String, u32>,
        limits: &wgpu::Limits,
    ) -> anyhow::Result<u32> {
        let key = name.to_ascii_lowercase();
        if let Some(&index) = known.get(&key) {
            return Ok(index);
        }
        ensure!(
            self.details.len() < limits.max_texture_array_layers as usize,
            "terrain detail array exceeds device limit"
        );
        let texture = scene
            .texture(name)
            .context("terrain detail texture unavailable")?;
        ensure!(
            texture.width > 0
                && texture.height > 0
                && texture.width.max(texture.height)
                    <= MAX_DETAIL_SIZE.min(limits.max_texture_dimension_2d),
            "unsupported terrain detail dimensions"
        );
        let size = texture
            .width
            .max(texture.height)
            .next_power_of_two()
            .max(self.size);
        ensure!(
            size <= limits.max_texture_dimension_2d,
            "terrain detail pool exceeds device dimensions"
        );
        ensure!(
            mip_bytes(size, 4) * (self.details.len() + 1) as u64 + self.stats.buffer_bytes
                <= BUDGET,
            "terrain detail array exceeds memory budget"
        );
        let image = image::RgbaImage::from_raw(texture.width, texture.height, texture.rgba)
            .context("incomplete terrain detail texture")?;
        let index = self.details.len() as u32;
        self.size = size;
        self.details.push(image);
        known.insert(key, index);
        Ok(index)
    }
}

fn mip_bytes(mut size: u32, channels: u64) -> u64 {
    let mut bytes = 0;
    loop {
        bytes += u64::from(size) * u64::from(size) * channels;
        if size == 1 {
            return bytes;
        }
        size /= 2;
    }
}

/// Each level starts on a word boundary. Bytes within a word follow increasing
/// X/Y, independent of the host endian convention used to upload the u32 words.
fn append_mask(out: &mut Vec<u32>, mask: &[u8], mut size: usize) {
    let mut values = mask.to_vec();
    loop {
        out.extend(values.chunks(4).map(|bytes| {
            bytes
                .iter()
                .enumerate()
                .fold(0, |word, (i, &value)| word | (u32::from(value) << (8 * i)))
        }));
        if size == 1 {
            return;
        }
        let next = size / 2;
        values = (0..next * next)
            .map(|index| {
                let x = (index % next) * 2;
                let y = (index / next) * 2;
                let sum = u32::from(values[y * size + x])
                    + u32::from(values[y * size + x + 1])
                    + u32::from(values[(y + 1) * size + x])
                    + u32::from(values[(y + 1) * size + x + 1]);
                ((sum + 2) / 4) as u8
            })
            .collect();
        size = next;
    }
}

pub(crate) struct GpuTerrain {
    pub view: wgpu::TextureView,
    pub headers: wgpu::Buffer,
    pub layers: wgpu::Buffer,
    pub masks: wgpu::Buffer,
    pub stats: TerrainStats,
    admitted: BTreeSet<usize>,
}

impl GpuTerrain {
    pub fn contains(&self, material: usize) -> bool {
        self.admitted.contains(&material)
    }

    pub fn build(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene,
        mode: TerrainMode,
    ) -> Self {
        let plan = if mode == TerrainMode::Baked {
            Plan::empty(None)
        } else {
            match Plan::prepare(scene, &device.limits()) {
                Ok(plan) => plan,
                Err(error) => {
                    tracing::warn!(zone = %scene.name, %error, "using baked terrain compatibility materials");
                    Plan::empty(Some(error.to_string()))
                }
            }
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shared terrain details"),
            size: wgpu::Extent3d {
                width: plan.size,
                height: plan.size,
                depth_or_array_layers: plan.details.len() as u32,
            },
            mip_level_count: plan.size.ilog2() + 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (index, image) in plan.details.iter().enumerate() {
            let mut pixels = image::imageops::resize(
                image,
                plan.size,
                plan.size,
                image::imageops::FilterType::Triangle,
            );
            for mip in 0..=plan.size.ilog2() {
                let size = plan.size >> mip;
                if mip > 0 {
                    pixels = image::imageops::resize(
                        &pixels,
                        size,
                        size,
                        image::imageops::FilterType::Triangle,
                    );
                }
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: mip,
                        origin: wgpu::Origin3d {
                            x: 0,
                            y: 0,
                            z: index as u32,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(size * 4),
                        rows_per_image: Some(size),
                    },
                    wgpu::Extent3d {
                        width: size,
                        height: size,
                        depth_or_array_layers: 1,
                    },
                );
            }
        }
        let buffer = |label, bytes: &[u8]| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.len() as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&buffer, 0, bytes);
            buffer
        };
        Self {
            view: texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            }),
            headers: buffer(
                "terrain material headers",
                bytemuck::cast_slice(&plan.headers),
            ),
            layers: buffer("terrain paint layers", bytemuck::cast_slice(&plan.layers)),
            masks: buffer(
                "terrain packed mask mipmaps",
                bytemuck::cast_slice(&plan.masks),
            ),
            stats: plan.stats,
            admitted: plan.admitted,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_assets::{
        mesh::Material,
        terrain::{EcoLayer, MaterialLayer, TerrainMaterial},
        texture::Texture,
    };

    fn budget_fixture() -> Scene {
        let mut scene = Scene::from_geometry(
            "budget".into(),
            vec![Material {
                textures: vec!["fallback".into()],
                normal_map: None,
                water: None,
                flags: 0,
                anim_speed: 0,
                alpha_mask: false,
                transparent: false,
                additive: false,
                emissive: false,
                clamp_uv: true,
                waterfall: None,
                uv_encoding: Default::default(),
            }],
            vec![],
            vec![Texture {
                name: "detail".into(),
                width: 3,
                height: 3,
                rgba: vec![255; 36],
            }],
        );
        scene.terrain_materials.insert(
            0,
            TerrainMaterial {
                fallback_texture: "fallback".into(),
                base_texture: Some("detail".into()),
                layers: vec![],
            },
        );
        scene
    }

    #[test]
    fn device_limits_reject_whole_plans_before_gpu_allocation() {
        let scene = budget_fixture();
        let valid = Plan::prepare(&scene, &wgpu::Limits::default()).unwrap();
        assert_eq!(valid.size, 4);
        assert_eq!(valid.stats.materials, 1);
        for limits in [
            wgpu::Limits {
                max_texture_dimension_2d: 3,
                ..Default::default()
            },
            wgpu::Limits {
                max_texture_array_layers: 0,
                ..Default::default()
            },
            wgpu::Limits {
                max_storage_buffer_binding_size: 15,
                ..Default::default()
            },
            wgpu::Limits {
                max_buffer_size: 15,
                ..Default::default()
            },
        ] {
            assert!(Plan::prepare(&scene, &limits).is_err());
        }
    }

    #[test]
    fn aggregate_layers_and_masks_are_checked_in_preflight() {
        let mut scene = budget_fixture();
        let eco = EcoLayer {
            detail_map: "detail".into(),
            repeat: 1.,
            min_height: -100.,
            max_height: 100.,
            height_tolerance: 0.,
            min_slope: 0.,
            max_slope: 90.,
            slope_tolerance: 0.,
        };
        scene.terrain_materials.get_mut(&0).unwrap().layers = vec![MaterialLayer {
            layers: vec![eco.clone(); 32],
            mask_size: 0,
            mask: vec![],
        }];
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: 32 * 48,
            ..Default::default()
        };
        assert!(Plan::prepare(&scene, &limits).is_ok());
        scene.materials.push(scene.materials[0].clone());
        scene
            .terrain_materials
            .insert(1, scene.terrain_materials[&0].clone());
        assert!(
            Plan::preflight(&scene, &limits).is_err(),
            "two individually valid layer sets exceed their shared buffer"
        );
        scene.terrain_materials.remove(&1);
        scene.terrain_materials.get_mut(&0).unwrap().layers[0]
            .layers
            .push(eco.clone());
        assert!(
            Plan::preflight(&scene, &wgpu::Limits::default()).is_err(),
            "33 layers exceed the per-material cap"
        );
        scene.terrain_materials.get_mut(&0).unwrap().layers = vec![
            MaterialLayer {
                layers: vec![eco.clone()],
                mask_size: 0,
                mask: vec![],
            },
            MaterialLayer {
                layers: vec![eco],
                mask_size: 64,
                mask: vec![128; 64 * 64],
            },
        ];
        let limits = wgpu::Limits {
            max_storage_buffer_binding_size: 8000,
            ..Default::default()
        };
        assert!(Plan::prepare(&scene, &limits).is_ok());
        scene
            .terrain_materials
            .insert(1, scene.terrain_materials[&0].clone());
        assert!(
            Plan::preflight(&scene, &limits).is_err(),
            "two valid masks exceed the shared mip buffer"
        );
    }

    #[test]
    fn mask_mips_keep_word_boundaries_and_area_averages() {
        let mut words = Vec::new();
        append_mask(
            &mut words,
            &[0, 4, 16, 20, 8, 12, 24, 28, 32, 36, 48, 52, 40, 44, 56, 60],
            4,
        );
        assert_eq!(words.len(), 6);
        assert_eq!(words[0], u32::from_le_bytes([0, 4, 16, 20]));
        assert_eq!(words[4], u32::from_le_bytes([6, 22, 38, 54]));
        assert_eq!(words[5], 30);
        assert_eq!(mip_bytes(4, 1), 21);
    }
}
