//! Bounded native-style rectangles from indexed heightmap water records.
//!
//! This is drawable geometry only. It does not infer liquid volumes, terrain
//! clipping, collision, or any meaning for the record's trailing float.

use std::collections::HashMap;

use crate::mesh::{Geometry, VERTEX_STRIDE};
use crate::{Error, Result};

use super::{Heightmap, IndexedWaterDefinition, IndexedWaterResolution, TerrainTile, WaterData};

const MAX_VERTICES: usize = 2_000_000;
const MAX_INDICES: usize = 12_000_000;
const MAX_DIAGNOSTICS: usize = 128;

#[derive(Debug)]
pub struct IndexedWaterSurface {
    pub tile_index: usize,
    /// Authored selector, not an ordinal in the indexed definition array.
    pub material_index: i32,
    /// Material zero is a placeholder for the loader to remap.
    pub geometry: Geometry,
}

#[derive(Debug)]
pub struct IndexedWaterDiagnostic {
    pub tile_index: usize,
    pub material_index: i32,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct BakedIndexedWater {
    pub surfaces: Vec<IndexedWaterSurface>,
    pub diagnostics: Vec<IndexedWaterDiagnostic>,
    /// Additional invalid active records beyond the bounded diagnostic list.
    pub omitted_diagnostics: usize,
}

/// Bake eligible indexed rectangles without changing their source metadata.
/// Invalid active records are diagnosed individually; inactive records are
/// ignored. Invalid global dimensions, allocation failure, or a zone exceeding
/// the resource budget returns an error without any partial surface set.
pub fn bake(map: &Heightmap, data: &WaterData) -> Result<BakedIndexedWater> {
    bake_with_limits(map, data, MAX_VERTICES, MAX_INDICES)
}

fn bake_with_limits(
    map: &Heightmap,
    data: &WaterData,
    vertex_limit: usize,
    index_limit: usize,
) -> Result<BakedIndexedWater> {
    let q = map.options.quads_per_tile;
    let spacing = map.options.units_per_vertex;
    let width = map.options.tile_size();
    if !(1..=512).contains(&q)
        || !spacing.is_finite()
        || spacing <= 0.0
        || !width.is_finite()
        || width <= 0.0
        || map.options.min_lng > map.options.max_lng
        || map.options.min_lat > map.options.max_lat
    {
        return Err(Error::Format(
            "invalid indexed water grid dimensions".into(),
        ));
    }

    let mut result = BakedIndexedWater::default();
    // resolve_index compares all authored fields. Cache its compact outcome,
    // including failures, instead of rescanning definitions for every tile.
    let mut materials = HashMap::new();
    let mut plans = Vec::new();
    let mut vertices = 0usize;
    let mut indices = 0usize;
    for (tile_index, tile) in map.tiles.iter().enumerate() {
        let Some(extension) = tile.water_metadata.extension.as_ref() else {
            continue;
        };
        if extension.tag == 0 {
            continue;
        }
        let material_index = tile.water_metadata.word_bits as i32;
        let plan = if !(1..10_000).contains(&material_index) {
            Err("selector is outside the decoded indexed rectangle branch")
        } else {
            let material = materials.entry(material_index).or_insert_with(|| {
                match data.resolve_index(material_index) {
                    IndexedWaterResolution::Missing => Err("missing indexed water definition"),
                    IndexedWaterResolution::Ambiguous { .. } => {
                        Err("conflicting indexed water definitions")
                    }
                    IndexedWaterResolution::Unique { definition, .. } => {
                        validate_material(definition)
                    }
                }
            });
            material.and_then(|scale| {
                plan_surface(tile_index, tile, material_index, q, spacing, width, scale)
            })
        };
        let plan = match plan {
            Ok(plan) => plan,
            Err(reason) => {
                if result.diagnostics.len() < MAX_DIAGNOSTICS {
                    result.diagnostics.push(IndexedWaterDiagnostic {
                        tile_index,
                        material_index,
                        reason: reason.into(),
                    });
                } else {
                    result.omitted_diagnostics += 1;
                }
                continue;
            }
        };
        vertices = vertices
            .checked_add(plan.vertex_count)
            .ok_or_else(|| Error::Format("indexed water vertex count overflow".into()))?;
        indices = indices
            .checked_add(plan.index_count)
            .ok_or_else(|| Error::Format("indexed water index count overflow".into()))?;
        if vertices > vertex_limit || indices > index_limit {
            return Err(Error::Format(format!(
                "indexed water exceeds zone geometry budget ({vertices}/{vertex_limit} vertices, {indices}/{index_limit} indices)"
            )));
        }
        plans.push(plan);
    }

    // Every eligible rectangle and the complete aggregate budget have passed
    // preflight before allocating any vertex or index buffers.
    result
        .surfaces
        .try_reserve_exact(plans.len())
        .map_err(|_| Error::Format("cannot allocate indexed water surface list".into()))?;
    for plan in plans {
        result.surfaces.push(IndexedWaterSurface {
            tile_index: plan.tile_index,
            material_index: plan.material_index,
            geometry: generate(&plan, q, spacing)?,
        });
    }
    Ok(result)
}

fn validate_material(
    definition: &IndexedWaterDefinition,
) -> std::result::Result<f32, &'static str> {
    let material = &definition.material;
    if !definition.uv_scale.is_finite()
        || !material
            .color1
            .iter()
            .chain(&material.color2)
            .chain(&material.reflection_color)
            .chain([
                &material.fresnel_bias,
                &material.fresnel_power,
                &material.reflection_amount,
            ])
            .all(|value| value.is_finite())
        || material
            .indexed_uv_scale
            .is_some_and(|scale| !scale.is_finite())
    {
        return Err("nonfinite indexed water material value");
    }
    Ok(definition.uv_scale)
}

#[derive(Debug)]
struct Axis {
    origin: f32,
    min: f32,
    max: f32,
    cells: usize,
}

impl Axis {
    fn local(&self, vertex: usize) -> f32 {
        // Keep authored endpoints exactly, including non-grid-aligned bounds.
        if vertex == self.cells {
            self.max
        } else {
            self.min + (vertex as f32 / self.cells as f32) * (self.max - self.min)
        }
    }

    fn position(&self, vertex: usize) -> f32 {
        self.origin + self.local(vertex)
    }

    fn uv(&self, vertex: usize, q: usize, spacing: f32) -> f32 {
        let grid_index = (self.local(vertex) / spacing + 0.5).trunc();
        ((grid_index / q as f32) * 256.0).trunc() / 256.0
    }

    fn validate(
        &self,
        q: usize,
        spacing: f32,
        scale: f32,
    ) -> std::result::Result<(), &'static str> {
        let mut previous = None;
        for vertex in 0..=self.cells {
            let position = self.position(vertex);
            if !position.is_finite() {
                return Err("nonfinite translated indexed water position");
            }
            if previous.is_some_and(|value| position <= value) {
                return Err("indexed water grid collapses at world-coordinate precision");
            }
            previous = Some(position);
            let grid_index = (self.local(vertex) / spacing + 0.5).trunc();
            if !grid_index.is_finite() || grid_index < 0.0 || grid_index > q as f32 {
                return Err("indexed water UV lookup is outside the tile grid");
            }
            // The native second layer is (asset_uv * scale) * 2. Preserve any
            // finite authored scale, rejecting only nonfinite derived UVs.
            if !(self.uv(vertex, q, spacing) * scale * 2.0).is_finite() {
                return Err("nonfinite scaled indexed water UV");
            }
        }
        Ok(())
    }
}

struct SurfacePlan {
    tile_index: usize,
    material_index: i32,
    x: Axis,
    y: Axis,
    z: f32,
    vertex_count: usize,
    index_count: usize,
}

fn grid_counts(nx: usize, ny: usize) -> Option<(usize, usize)> {
    let vertices = nx
        .checked_add(1)?
        .checked_mul(ny.checked_add(1)?)?
        .checked_mul(2)?;
    let indices = nx.checked_mul(ny)?.checked_mul(12)?;
    // Geometry uses u32 indices and interleaved float storage.
    u32::try_from(vertices).ok()?;
    vertices.checked_mul(VERTEX_STRIDE)?;
    Some((vertices, indices))
}

fn plan_surface(
    tile_index: usize,
    tile: &TerrainTile,
    material_index: i32,
    q: usize,
    spacing: f32,
    width: f32,
    scale: f32,
) -> std::result::Result<SurfacePlan, &'static str> {
    let bounds = tile
        .water_metadata
        .extension
        .as_ref()
        .and_then(|extension| extension.bounds)
        .ok_or("active indexed water record has no bounds")?;
    if !tile.water_level.is_finite() || !bounds.iter().all(|value| value.is_finite()) {
        return Err("nonfinite indexed water bounds or elevation");
    }
    let [xmin, xmax, ymin, ymax] = bounds;
    if xmin < 0.0 || xmax > width || xmin >= xmax || ymin < 0.0 || ymax > width || ymin >= ymax {
        return Err("indexed water rectangle is empty, reversed, or outside its tile");
    }
    let nx = ((xmax - xmin) / spacing).trunc().max(1.0);
    let ny = ((ymax - ymin) / spacing).trunc().max(1.0);
    if !nx.is_finite() || !ny.is_finite() || nx > q as f32 || ny > q as f32 {
        return Err("indexed water subdivision exceeds its tile grid");
    }
    let x = Axis {
        origin: tile.longitude as f32 * width,
        min: xmin,
        max: xmax,
        cells: nx as usize,
    };
    let y = Axis {
        origin: tile.latitude as f32 * width,
        min: ymin,
        max: ymax,
        cells: ny as usize,
    };
    x.validate(q, spacing, scale)?;
    y.validate(q, spacing, scale)?;
    let (vertex_count, index_count) =
        grid_counts(x.cells, y.cells).ok_or("indexed water geometry count overflow")?;
    Ok(SurfacePlan {
        tile_index,
        material_index,
        x,
        y,
        z: tile.water_level,
        vertex_count,
        index_count,
    })
}

fn generate(plan: &SurfacePlan, q: usize, spacing: f32) -> Result<Geometry> {
    let mut geometry = Geometry {
        vertices: Vec::new(),
        indices: Vec::new(),
        material: 0,
        collidable: false,
    };
    geometry
        .vertices
        .try_reserve_exact(plan.vertex_count * VERTEX_STRIDE)
        .map_err(|_| Error::Format("cannot allocate indexed water vertices".into()))?;
    geometry
        .indices
        .try_reserve_exact(plan.index_count)
        .map_err(|_| Error::Format("cannot allocate indexed water indices".into()))?;
    for normal in [1.0, -1.0] {
        for row in 0..=plan.y.cells {
            for col in 0..=plan.x.cells {
                geometry.vertices.extend_from_slice(&[
                    plan.x.position(col),
                    plan.y.position(row),
                    plan.z,
                    0.0,
                    0.0,
                    normal,
                    plan.x.uv(col, q, spacing),
                    plan.y.uv(row, q, spacing),
                ]);
            }
        }
    }
    let stride = (plan.x.cells + 1) as u32;
    let side_vertices = (plan.vertex_count / 2) as u32;
    for side in 0..2 {
        for row in 0..plan.y.cells {
            for col in 0..plan.x.cells {
                let a = side * side_vertices + row as u32 * stride + col as u32;
                let b = a + stride;
                if side == 0 {
                    geometry
                        .indices
                        .extend_from_slice(&[a, a + 1, b + 1, a, b + 1, b]);
                } else {
                    geometry
                        .indices
                        .extend_from_slice(&[a, b + 1, a + 1, a, b, b + 1]);
                }
            }
        }
    }
    Ok(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::{
        TerrainOptions, TerrainWaterExtension, TerrainWaterMetadata, WaterField, parse_water_data,
    };

    fn data() -> WaterData {
        parse_water_data(
            b"*WATERSHEETDATA
*INDEX 1
*FRESNELBIAS 0.25
*FRESNELPOWER 8
*REFLECTIONAMOUNT 0.7
*UVSCALE 1
*REFLECTIONCOLOR 0.7 1 1 1
*WATERCOLOR1 0 0.04 0.11 1
*WATERCOLOR2 0 0.23 0.17 1
*NORMALMAP water_n.dds
*ENVIRONMENTMAP water_e.dds
*ENDWATERSHEETDATA
",
        )
        .unwrap()
    }

    fn tile(longitude: i32, latitude: i32, bounds: [f32; 4]) -> TerrainTile {
        TerrainTile {
            longitude,
            latitude,
            heights: Vec::new(),
            colors: Vec::new(),
            secondary_colors: Vec::new(),
            quad_flags: Vec::new(),
            water_level: -50.0,
            water_metadata: TerrainWaterMetadata {
                word_bits: 1,
                extension: Some(TerrainWaterExtension {
                    tag: 1,
                    bounds: Some(bounds),
                    trailing_value: 0.0,
                }),
            },
            layers: Vec::new(),
        }
    }

    fn terrain_map(q: usize, spacing: f32, tiles: Vec<TerrainTile>) -> Heightmap {
        Heightmap {
            options: TerrainOptions {
                name: "synthetic_water".into(),
                min_lng: -20,
                max_lng: 20,
                min_lat: -20,
                max_lat: 20,
                quads_per_tile: q,
                units_per_vertex: spacing,
            },
            header: [21, 0, 1],
            base_texture: String::new(),
            tiles,
            placements: Vec::new(),
            lights: Vec::new(),
            groups: Vec::new(),
            region_count: 0,
        }
    }

    fn full_map(q: usize) -> Heightmap {
        terrain_map(
            q,
            16.0,
            vec![tile(0, 0, [0.0, q as f32 * 16.0, 0.0, q as f32 * 16.0])],
        )
    }

    fn assert_omitted(map: &Heightmap, data: &WaterData, reason: &str) {
        let result = bake(map, data).unwrap();
        assert!(result.surfaces.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        assert!(
            result.diagnostics[0].reason.contains(reason),
            "{:?}",
            result.diagnostics
        );
    }

    #[test]
    fn pond_has_authored_spatial_extents_elevation_counts_and_opposing_winding() {
        let map = terrain_map(16, 16.0, vec![tile(-3, -12, [0.0, 96.0, 112.0, 224.0])]);
        let result = bake(&map, &data()).unwrap();
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.surfaces.len(), 1);
        let surface = &result.surfaces[0];
        assert_eq!(surface.tile_index, 0);
        assert_eq!(surface.material_index, 1);
        let mesh = &surface.geometry;
        assert_eq!(mesh.vertex_count(), 112);
        assert_eq!(mesh.indices.len(), 504);
        assert!(!mesh.collidable);
        assert_eq!(mesh.material, 0);
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for (index, vertex) in mesh.vertices.chunks_exact(VERTEX_STRIDE).enumerate() {
            for axis in 0..2 {
                min[axis] = min[axis].min(vertex[axis]);
                max[axis] = max[axis].max(vertex[axis]);
            }
            assert_eq!(vertex[2], -50.0);
            assert_eq!(
                &vertex[3..6],
                &[0.0, 0.0, if index < 56 { 1.0 } else { -1.0 }]
            );
            assert!((0.0..=0.375).contains(&vertex[6]));
            assert!((0.4375..=0.875).contains(&vertex[7]));
        }
        assert_eq!(min, [-768.0, -2960.0]);
        assert_eq!(max, [-672.0, -2848.0]);
        for triangle in mesh.indices.chunks_exact(3) {
            let vertex =
                |index: u32| &mesh.vertices[index as usize * VERTEX_STRIDE..][..VERTEX_STRIDE];
            let a = vertex(triangle[0]);
            let b = vertex(triangle[1]);
            let c = vertex(triangle[2]);
            let cross_z = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
            assert!(cross_z * a[5] > 0.0);
        }
    }

    #[test]
    fn subdivides_before_quantizing_and_preserves_one_at_maximum_edges() {
        for q in [14, 16, 24] {
            let result = bake(&full_map(q), &data()).unwrap();
            let mesh = &result.surfaces[0].geometry;
            assert_eq!(mesh.vertex_count(), 2 * (q + 1) * (q + 1));
            for row in 0..=q {
                for col in 0..=q {
                    let vertex = &mesh.vertices[(row * (q + 1) + col) * VERTEX_STRIDE..];
                    assert_eq!(vertex[0], col as f32 * 16.0);
                    assert_eq!(vertex[1], row as f32 * 16.0);
                    assert_eq!(vertex[6], ((col as f32 / q as f32) * 256.0).trunc() / 256.0);
                    assert_eq!(vertex[7], ((row as f32 / q as f32) * 256.0).trunc() / 256.0);
                }
            }
            assert_eq!(mesh.vertices[q * VERTEX_STRIDE + 6], 1.0);
            if q == 14 {
                // A single interpolated quad would have 1/14 at the first
                // interior vertex, instead of native pre-interpolation 18/256.
                assert_eq!(mesh.vertices[VERTEX_STRIDE + 6], 18.0 / 256.0);
                assert_ne!(mesh.vertices[VERTEX_STRIDE + 6], 1.0 / 14.0);
            }
        }
    }

    #[test]
    fn nonaligned_subspacing_rectangles_keep_exact_endpoints_and_nearest_grid_uvs() {
        let map = terrain_map(16, 16.0, vec![tile(2, -1, [8.0, 15.5, 24.0, 25.25])]);
        let result = bake(&map, &data()).unwrap();
        let mesh = &result.surfaces[0].geometry;
        assert_eq!(mesh.vertex_count(), 8);
        assert_eq!(mesh.indices.len(), 12);
        let points: Vec<_> = mesh
            .vertices
            .chunks_exact(8)
            .map(|v| [v[0], v[1], v[6], v[7]])
            .collect();
        assert_eq!(
            &points[..4],
            &[
                [520.0, -232.0, 1.0 / 16.0, 2.0 / 16.0],
                [527.5, -232.0, 1.0 / 16.0, 2.0 / 16.0],
                [520.0, -230.75, 1.0 / 16.0, 2.0 / 16.0],
                [527.5, -230.75, 1.0 / 16.0, 2.0 / 16.0],
            ]
        );
    }

    #[test]
    fn negative_and_positive_tile_seams_repeat_without_wrapping_mesh_uvs() {
        for longitude in [-4, -1, 0, 3] {
            let map = terrain_map(
                16,
                16.0,
                vec![
                    tile(longitude, -2, [0.0, 256.0, 0.0, 256.0]),
                    tile(longitude + 1, -2, [0.0, 256.0, 0.0, 256.0]),
                ],
            );
            let result = bake(&map, &data()).unwrap();
            for row in 0..=16 {
                let left = &result.surfaces[0].geometry.vertices[(row * 17 + 16) * 8..];
                let right = &result.surfaces[1].geometry.vertices[row * 17 * 8..];
                assert_eq!(&left[..3], &right[..3]);
                assert_eq!(left[6], 1.0);
                assert_eq!(right[6], 0.0);
                assert_eq!(left[7], right[7]);
                assert_eq!(left[6].fract(), right[6].fract());
            }
        }
    }

    #[test]
    fn terrain_heights_quad_flags_and_unknown_tail_do_not_clip_or_change_surface() {
        let mut map = full_map(16);
        let before = bake(&map, &data()).unwrap();
        map.tiles[0].heights = vec![1_000.0; 289];
        map.tiles[0].quad_flags = vec![0xff; 256];
        map.tiles[0]
            .water_metadata
            .extension
            .as_mut()
            .unwrap()
            .trailing_value = f32::NAN;
        let after = bake(&map, &data()).unwrap();
        assert_eq!(
            before.surfaces[0].geometry.vertices,
            after.surfaces[0].geometry.vertices
        );
        assert_eq!(
            before.surfaces[0].geometry.indices,
            after.surfaces[0].geometry.indices
        );
    }

    #[test]
    fn signed_nonzero_tags_are_active_but_inactive_and_legacy_records_are_silent() {
        for tag in [1, 127, -128, -1] {
            let mut map = full_map(16);
            map.tiles[0].water_metadata.extension.as_mut().unwrap().tag = tag;
            assert_eq!(bake(&map, &data()).unwrap().surfaces.len(), 1);
        }
        for extension in [
            None,
            Some(TerrainWaterExtension {
                tag: 0,
                bounds: Some([f32::NAN; 4]),
                trailing_value: 0.0,
            }),
        ] {
            let mut map = full_map(16);
            map.tiles[0].water_metadata.extension = extension;
            map.tiles[0].water_metadata.word_bits = u32::MAX;
            let result = bake(&map, &data()).unwrap();
            assert!(result.surfaces.is_empty());
            assert!(result.diagnostics.is_empty());
        }
    }

    #[test]
    fn only_exact_unique_supported_selectors_produce_geometry() {
        let mut map = full_map(16);
        for index in [i32::MIN, -1, 0, 10_000, i32::MAX] {
            map.tiles[0].water_metadata.word_bits = index as u32;
            assert_omitted(&map, &data(), "outside the decoded");
        }
        map.tiles[0].water_metadata.word_bits = 2;
        assert_omitted(&map, &data(), "missing");
        map.tiles[0].water_metadata.word_bits = 1;
        let mut definitions = data();
        definitions.indexed.push(definitions.indexed[0].clone());
        assert_eq!(bake(&map, &definitions).unwrap().surfaces.len(), 1);
        definitions.indexed[1].fields.push(WaterField {
            key: "*UNKNOWN".into(),
            values: vec!["different".into()],
        });
        assert_omitted(&map, &definitions, "conflicting");
        definitions.indexed.clear();
        let mut fallback = data().indexed.remove(0);
        fallback.index = 0;
        definitions.indexed.push(fallback);
        assert_omitted(&map, &definitions, "missing");
        definitions.indexed[0].index = 9999;
        map.tiles[0].water_metadata.word_bits = 9999;
        assert_eq!(
            bake(&map, &definitions).unwrap().surfaces[0].material_index,
            9999
        );
    }

    #[test]
    fn missing_reversed_degenerate_nonfinite_and_outside_bounds_are_omitted() {
        let mut map = full_map(16);
        let invalid = [
            None,
            Some([16.0, 0.0, 0.0, 16.0]),
            Some([0.0, 16.0, 16.0, 0.0]),
            Some([0.0, 0.0, 0.0, 16.0]),
            Some([0.0, 16.0, 1.0, 1.0]),
            Some([-1.0, 16.0, 0.0, 16.0]),
            Some([0.0, 257.0, 0.0, 16.0]),
            Some([0.0, 16.0, -1.0, 16.0]),
            Some([0.0, 16.0, 0.0, 257.0]),
            Some([f32::NAN, 16.0, 0.0, 16.0]),
            Some([0.0, f32::INFINITY, 0.0, 16.0]),
            Some([0.0, 16.0, f32::NEG_INFINITY, 16.0]),
        ];
        for bounds in invalid {
            map.tiles[0]
                .water_metadata
                .extension
                .as_mut()
                .unwrap()
                .bounds = bounds;
            let result = bake(&map, &data()).unwrap();
            assert!(result.surfaces.is_empty(), "{bounds:?}");
            assert_eq!(result.diagnostics.len(), 1, "{bounds:?}");
            assert_eq!(result.diagnostics[0].tile_index, 0);
            assert_eq!(result.diagnostics[0].material_index, 1);
        }
        map.tiles[0]
            .water_metadata
            .extension
            .as_mut()
            .unwrap()
            .bounds = Some([0.0, 16.0, 0.0, 16.0]);
        for level in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            map.tiles[0].water_level = level;
            assert_omitted(&map, &data(), "nonfinite");
        }
        for level in [0.0, -1000.0, 50.0] {
            map.tiles[0].water_level = level;
            assert_eq!(
                bake(&map, &data()).unwrap().surfaces[0].geometry.vertices[2],
                level
            );
        }
    }

    #[test]
    fn invalid_global_grid_and_nonfinite_or_collapsed_world_positions_are_rejected() {
        let definitions = data();
        for q in [0, 513, usize::MAX] {
            let mut map = full_map(16);
            map.options.quads_per_tile = q;
            assert!(bake(&map, &definitions).is_err());
        }
        for spacing in [0.0, -1.0, f32::NAN, f32::INFINITY, f32::MAX] {
            let mut map = full_map(16);
            map.options.units_per_vertex = spacing;
            assert!(bake(&map, &definitions).is_err());
        }
        let mut map = full_map(16);
        map.options.min_lat = map.options.max_lat + 1;
        assert!(bake(&map, &definitions).is_err());
        let map = terrain_map(
            1,
            f32::MAX / 2.0,
            vec![tile(4, 0, [0.0, f32::MAX / 2.0, 0.0, f32::MAX / 2.0])],
        );
        assert_omitted(&map, &definitions, "nonfinite translated");
        let mut map = full_map(16);
        map.tiles[0].longitude = i32::MAX;
        assert_omitted(&map, &definitions, "precision");
    }

    #[test]
    fn nonfinite_material_values_are_rejected_without_restricting_finite_scales() {
        let map = full_map(16);
        for field in 0..17 {
            let mut definitions = data();
            let definition = &mut definitions.indexed[0];
            match field {
                0..=3 => definition.material.color1[field] = f32::INFINITY,
                4..=7 => definition.material.color2[field - 4] = f32::INFINITY,
                8..=11 => definition.material.reflection_color[field - 8] = f32::NEG_INFINITY,
                12 => definition.material.fresnel_bias = f32::INFINITY,
                13 => definition.material.fresnel_power = f32::INFINITY,
                14 => definition.material.reflection_amount = f32::NEG_INFINITY,
                15 => definition.uv_scale = f32::INFINITY,
                16 => definition.material.indexed_uv_scale = Some(f32::INFINITY),
                _ => unreachable!(),
            }
            assert_omitted(&map, &definitions, "nonfinite indexed water material");
        }
        let reference = bake(&map, &data()).unwrap();
        for scale in [0.0, -1.0, 0.125, 3.1, 1e20, -1e20] {
            let mut definitions = data();
            definitions.indexed[0].uv_scale = scale;
            let result = bake(&map, &definitions).unwrap();
            assert_eq!(
                result.surfaces[0].geometry.vertices,
                reference.surfaces[0].geometry.vertices
            );
            assert_eq!(definitions.indexed[0].uv_scale, scale);
        }
        let mut definitions = data();
        definitions.indexed[0].uv_scale = f32::NAN;
        // The resolver's full authored equality treats NaN as conflicting,
        // even in a single record; it must still never generate geometry.
        assert_omitted(&map, &definitions, "conflicting");
        definitions.indexed[0].uv_scale = f32::MAX;
        assert_omitted(&map, &definitions, "nonfinite scaled");
        // Huge scale is still valid where the actual local UVs stay small.
        let small = terrain_map(16, 16.0, vec![tile(0, 0, [0.0, 16.0, 0.0, 16.0])]);
        assert_eq!(bake(&small, &definitions).unwrap().surfaces.len(), 1);
    }

    #[test]
    fn diagnostics_are_bounded_and_valid_later_tiles_still_render() {
        let mut invalid = tile(0, 0, [0.0, 16.0, 0.0, 16.0]);
        invalid.water_metadata.word_bits = 0;
        let mut tiles = vec![invalid; MAX_DIAGNOSTICS + 9];
        tiles.push(tile(1, 1, [0.0, 16.0, 0.0, 16.0]));
        let result = bake(&terrain_map(16, 16.0, tiles), &data()).unwrap();
        assert_eq!(result.diagnostics.len(), MAX_DIAGNOSTICS);
        assert_eq!(result.omitted_diagnostics, 9);
        assert_eq!(result.surfaces.len(), 1);
        assert_eq!(result.surfaces[0].tile_index, MAX_DIAGNOSTICS + 9);
    }

    #[test]
    fn checked_counts_and_aggregate_budgets_are_transactional() {
        assert_eq!(grid_counts(1, 1), Some((8, 12)));
        assert_eq!(grid_counts(512, 512), Some((526_338, 3_145_728)));
        assert!(grid_counts(usize::MAX, 1).is_none());
        assert!(grid_counts(1, usize::MAX).is_none());
        assert!(grid_counts(usize::MAX / 2, usize::MAX / 2).is_none());
        let map = terrain_map(1, 16.0, vec![tile(0, 0, [0.0, 16.0, 0.0, 16.0]); 2]);
        assert_eq!(
            bake_with_limits(&map, &data(), 16, 24)
                .unwrap()
                .surfaces
                .len(),
            2
        );
        assert!(
            bake_with_limits(&map, &data(), 15, 24)
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
        assert!(
            bake_with_limits(&map, &data(), 16, 23)
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
        // Exercise the production cap with tiny source records. Preflight must
        // fail before the four full 512x512 meshes allocate their buffers.
        let large = terrain_map(512, 1.0, vec![tile(0, 0, [0.0, 512.0, 0.0, 512.0]); 4]);
        assert!(
            bake(&large, &data())
                .unwrap_err()
                .to_string()
                .contains("budget")
        );
    }
}
