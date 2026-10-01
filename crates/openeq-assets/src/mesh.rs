//! Baking parsed fragments into GPU-ready triangle geometry.
//!
//! EverQuest meshes are indexed triangle lists with per-vertex positions,
//! normals and texture coordinates, grouped into runs that share a material
//! and a "collidable" flag. Baking here means:
//!
//! * splitting the index list into one buffer per material;
//! * de-duplicating vertices so hardware can share them (the original data
//!   repeats vertices freely);
//! * interleaving everything into a single `pos(3) normal(3) uv(2)` stream.
//!
//! The vertex dedup key is the exact bit pattern of each component, matching
//! the original C# behaviour rather than an epsilon comparison.

use std::collections::HashMap;

use crate::wld::{Fragment, Mesh, Ref, Wld};

/// Number of `f32` components per vertex in a baked buffer.
pub const VERTEX_STRIDE: usize = 8;

/// Proven conversion between raw asset UVs and shader inputs.
/// Kept on the material so extraction and material remapping preserve it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum UvEncoding {
    #[default]
    Float32,
    /// Proven TER ordinary bump/waterfall families: signed SHORT2 / 256.
    /// Reproduces the SSE2 conversion with masked exceptions, independent of
    /// this process's CPU. Legacy x87 overflow behavior is a separate target.
    NativeTerShort2Sse2,
}

/// Parameters of an EQG `Opaque_MaxWater.fx` surface.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterMaterial {
    /// Indexed heightmap surfaces use native tile UVs and this authored scale.
    /// None retains the existing world-coordinate EQG/finite-sheet shading.
    pub indexed_uv_scale: Option<f32>,
    pub color1: [f32; 4],
    pub color2: [f32; 4],
    pub reflection_color: [f32; 4],
    pub fresnel_bias: f32,
    pub fresnel_power: f32,
    pub reflection_amount: f32,
    pub environment_map: Option<String>,
}

/// A drawable surface's material description.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// Diffuse texture names; more than one means an animated flipbook.
    pub textures: Vec<String>,
    /// Optional normal map (only seen in `.eqg` zones).
    pub normal_map: Option<String>,
    pub water: Option<WaterMaterial>,
    /// Raw texture flags from the source data.
    pub flags: u32,
    /// Milliseconds per animation frame, when animated.
    pub anim_speed: u32,
    /// Cut out fully transparent texels (1-bit alpha).
    pub alpha_mask: bool,
    /// Blend using the texture's alpha channel.
    pub transparent: bool,
    /// Proven EQG region additive color with alpha cutoff, no fog or depth writes.
    pub additive: bool,
    /// Unlit/emissive surface, e.g. fire.
    pub emissive: bool,
    /// Clamp diffuse sampling to its edges (for nonperiodic baked terrain tiles).
    pub clamp_uv: bool,
    /// Proven TER waterfall slide rates: color XY, independent opacity XY.
    pub waterfall: Option<[f32; 4]>,
    /// Upload conversion only; baked/source vertex words remain untouched.
    pub uv_encoding: UvEncoding,
}

type MaterialKey = (
    u32,
    u32,
    String,
    bool,
    bool,
    bool,
    bool,
    bool,
    UvEncoding,
    Option<[u32; 4]>,
);

impl Material {
    fn key(&self) -> MaterialKey {
        (
            self.flags,
            self.anim_speed,
            self.textures.join(","),
            self.alpha_mask,
            self.transparent,
            self.additive,
            self.emissive,
            self.clamp_uv,
            self.uv_encoding,
            self.waterfall.map(|rates| rates.map(f32::to_bits)),
        )
    }
}

/// A baked indexed triangle list.
#[derive(Debug, Clone)]
pub struct Geometry {
    /// Interleaved `pos(3) normal(3) uv(2)`.
    pub vertices: Vec<f32>,
    pub indices: Vec<u32>,
    pub material: usize,
    pub collidable: bool,
}

impl Geometry {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / VERTEX_STRIDE
    }
}

/// Physical geometry independent of drawable materials and textures.
/// Positions are in the same local/scene space as the corresponding meshes.
#[derive(Debug, Clone, Default)]
pub struct CollisionGeometry {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// Collects authored invisible, collidable WLD polygons without changing the
/// drawable bake. An unresolved material is not evidence of invisibility.
pub fn bake_wld_collision_meshes<'a, I>(wld: &Wld, meshes: I) -> Vec<CollisionGeometry>
where
    I: IntoIterator<Item = &'a Mesh>,
{
    let mut geometries = Vec::new();
    for mesh in meshes {
        let Some(Fragment::MaterialList(list)) = wld.resolve(mesh.materials).map(|c| &c.fragment)
        else {
            continue;
        };
        let mut geometry = CollisionGeometry::default();
        let mut vertices = HashMap::new();
        let mut cursor = 0usize;
        let mut invalid = false;
        for &(count, slot) in &mesh.polygon_textures {
            let end = cursor.saturating_add(count as usize);
            let polygons = mesh.polygons.get(cursor..end);
            cursor = end;
            let Some(polygons) = polygons else {
                invalid = true;
                continue;
            };
            let material = list
                .materials
                .get(slot as usize)
                .and_then(|reference| wld.resolve(*reference));
            let Some(Fragment::Material(material)) = material.map(|c| &c.fragment) else {
                invalid = true;
                continue;
            };
            if material.flags != 0 {
                continue;
            }
            for polygon in polygons.iter().filter(|polygon| polygon.collidable) {
                // Match the drawable bake's winding. Parsed positions already
                // contain the WLD fragment center and quantization scale.
                let [Some(a), Some(b), Some(c)] = [polygon.a, polygon.c, polygon.b]
                    .map(|index| mesh.vertices.get(index as usize).copied())
                else {
                    invalid = true;
                    continue;
                };
                let points = [a, b, c];
                if !points.iter().flatten().all(|value| value.is_finite()) {
                    invalid = true;
                    continue;
                }
                for point in points {
                    let key = point.map(f32::to_bits);
                    let next = geometry.positions.len() as u32;
                    let index = *vertices.entry(key).or_insert_with(|| {
                        geometry.positions.push(point);
                        next
                    });
                    geometry.indices.push(index);
                }
            }
        }
        if invalid {
            tracing::warn!(wld = %wld.filename, "skipped invalid invisible collision geometry");
        }
        if !geometry.indices.is_empty() {
            geometries.push(geometry);
        }
    }
    geometries
}

/// De-duplicates and interleaves vertices, preserving index order.
pub fn pack(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> (Vec<f32>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut remap = HashMap::with_capacity(positions.len());
    let mut out_indices = Vec::with_capacity(indices.len());

    for &index in indices {
        let i = index as usize;
        let key = (
            positions[i][0].to_bits(),
            positions[i][1].to_bits(),
            positions[i][2].to_bits(),
            normals[i][0].to_bits(),
            normals[i][1].to_bits(),
            normals[i][2].to_bits(),
            uvs[i][0].to_bits(),
            uvs[i][1].to_bits(),
        );
        let next = remap.len() as u32;
        let mapped = *remap.entry(key).or_insert(next);
        if mapped == next {
            vertices.extend_from_slice(&positions[i]);
            vertices.extend_from_slice(&normals[i]);
            vertices.extend_from_slice(&uvs[i]);
        }
        out_indices.push(mapped);
    }

    (vertices, out_indices)
}

/// One `0x36` mesh together with everything needed to resolve its textures.
struct Piece {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    polygons: Vec<(bool, u32, u32, u32)>,
    textures: Vec<TextureEntry>,
    polygon_textures: Vec<(u16, u16)>,
}

#[derive(Clone)]
struct TextureEntry {
    flags: u32,
    anim_speed: u32,
    filenames: Vec<String>,
}

impl Piece {
    fn from_mesh(wld: &Wld, mesh: &Mesh) -> Self {
        let textures = texture_entries(wld, mesh);
        Self {
            positions: mesh.vertices.clone(),
            normals: mesh.normals.clone(),
            uvs: mesh.tex_coords.clone(),
            polygons: mesh
                .polygons
                .iter()
                .map(|p| (p.collidable, p.a, p.b, p.c))
                .collect(),
            textures,
            polygon_textures: mesh.polygon_textures.clone(),
        }
    }
}

/// Walks the `mesh -> material list -> material -> animation -> texture list`
/// chain to recover the texture names and flags for one mesh.
///
/// Entries are never dropped, even when a link is missing: polygon runs index
/// this list positionally, so removing one entry shifts every material after it
/// and surfaces end up wearing each other's textures.
fn texture_entries(wld: &Wld, mesh: &Mesh) -> Vec<TextureEntry> {
    let Some(Fragment::MaterialList(list)) = wld.resolve(mesh.materials).map(|c| &c.fragment)
    else {
        return Vec::new();
    };

    let mut entries = Vec::with_capacity(list.materials.len());
    for material_ref in &list.materials {
        entries.push(texture_entry(wld, *material_ref));
    }
    entries
}

/// Resolves one material, degrading to an empty entry instead of vanishing.
fn texture_entry(wld: &Wld, material_ref: Ref) -> TextureEntry {
    let mut entry = TextureEntry {
        flags: 0,
        anim_speed: 0,
        filenames: Vec::new(),
    };

    let Some(Fragment::Material(material)) = wld.resolve(material_ref).map(|c| &c.fragment) else {
        return entry;
    };
    entry.flags = material.flags;

    // A zero reference means the material deliberately has no texture.
    let Some(Fragment::AnimationRef(animation_ref)) =
        wld.resolve(material.animation).map(|c| &c.fragment)
    else {
        return entry;
    };
    let Some(Fragment::Animation(animation)) =
        wld.resolve(animation_ref.animation).map(|c| &c.fragment)
    else {
        return entry;
    };
    entry.anim_speed = animation.frame_time;
    for texture_ref in &animation.textures {
        if let Some(Fragment::TextureList(list)) = wld.resolve(*texture_ref).map(|c| &c.fragment) {
            // Each 0x04 reference is one animation frame. Extra names within
            // its 0x03 bitmap are texture layers (e.g. *_DETAIL_4.000000), not
            // additional frames. We currently render only the diffuse layer.
            entry.filenames.extend(list.filenames.first().cloned());
        }
    }
    entry
}

/// Bakes a set of `0x36` fragments into drawable geometry and materials.
pub fn bake_wld_meshes<'a, I>(wld: &Wld, meshes: I) -> (Vec<Material>, Vec<Geometry>)
where
    I: IntoIterator<Item = &'a Mesh>,
{
    let (materials, geometries, _) = bake_wld_meshes_inner(wld, meshes, false);
    (materials, geometries)
}

/// Animation packing preserves original vertex identity even when two source
/// vertices have identical first-pose attributes but different motion owners.
/// Bindings use flattened input-mesh vertex indices, in input order.
pub(crate) fn bake_wld_meshes_with_sources<'a>(
    wld: &Wld,
    meshes: impl IntoIterator<Item = &'a Mesh>,
) -> (Vec<Material>, Vec<Geometry>, Vec<Vec<usize>>) {
    bake_wld_meshes_inner(wld, meshes, true)
}

fn bake_wld_meshes_inner<'a>(
    wld: &Wld,
    meshes: impl IntoIterator<Item = &'a Mesh>,
    preserve_sources: bool,
) -> (Vec<Material>, Vec<Geometry>, Vec<Vec<usize>>) {
    let pieces: Vec<Piece> = meshes
        .into_iter()
        .map(|mesh| Piece::from_mesh(wld, mesh))
        .collect();

    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut textures = Vec::new();
    // (texture index, collidable) -> triangle indices
    let mut groups: HashMap<(usize, bool), Vec<u32>> = HashMap::new();

    for piece in &pieces {
        let vertex_offset = positions.len() as u32;
        let texture_offset = textures.len();
        positions.extend_from_slice(&piece.positions);
        normals.extend_from_slice(&piece.normals);
        uvs.extend_from_slice(&piece.uvs);
        textures.extend(piece.textures.iter().cloned());

        let mut polygon_cursor = 0usize;
        for &(count, texture_index) in &piece.polygon_textures {
            let count = count as usize;
            if texture_index as usize >= piece.textures.len() {
                // Only that run is unusable; the rest of the mesh still is not.
                tracing::warn!(
                    mesh = %wld.filename,
                    index = texture_index,
                    materials = piece.textures.len(),
                    "polygon run references a material outside this mesh"
                );
                polygon_cursor += count;
                continue;
            }
            for polygon in piece.polygons.iter().skip(polygon_cursor).take(count) {
                let key = (texture_index as usize + texture_offset, polygon.0);
                let group = groups.entry(key).or_default();
                // Winding is flipped here to match the original renderer.
                group.push(polygon.1 + vertex_offset);
                group.push(polygon.3 + vertex_offset);
                group.push(polygon.2 + vertex_offset);
            }
            polygon_cursor += count;
        }
    }

    // Collapse identical texture entries.
    let mut unique: Vec<TextureEntry> = Vec::new();
    let mut unique_index: HashMap<(u32, u32, String), usize> = HashMap::new();
    let mut remap = Vec::with_capacity(textures.len());
    for entry in &textures {
        let key = (entry.flags, entry.anim_speed, entry.filenames.join(","));
        let index = *unique_index.entry(key).or_insert_with(|| {
            unique.push(entry.clone());
            unique.len() - 1
        });
        remap.push(index);
    }

    let mut merged: HashMap<(usize, bool), Vec<u32>> = HashMap::new();
    // Keep source piece/material order when identical textures merge. Sorting
    // only the final material keys leaves their triangles (and packed vertices)
    // dependent on the intermediate HashMap's randomized iteration order.
    let mut source_groups: Vec<_> = groups.into_iter().collect();
    source_groups.sort_unstable_by_key(|(key, _)| *key);
    for ((texture, collidable), indices) in source_groups {
        let Some(texture) = remap.get(texture) else {
            // A polygon run that points past the material list. Nothing sane to
            // draw, so drop the run rather than guess.
            tracing::warn!(
                mesh = %wld.filename,
                index = texture,
                materials = textures.len(),
                "polygon run references a material that does not exist"
            );
            continue;
        };
        merged
            .entry((*texture, collidable))
            .or_default()
            .extend(indices);
    }

    let mut materials = Vec::new();
    let mut material_index: HashMap<MaterialKey, usize> = HashMap::new();
    let mut geometries = Vec::new();
    let mut sources = Vec::new();

    // Stable ordering keeps output deterministic between runs.
    let mut keys: Vec<_> = merged.keys().copied().collect();
    keys.sort_by_key(|(texture, collidable)| (*texture, *collidable));

    for key in keys {
        let (texture, collidable) = key;
        let entry = &unique[texture];
        // A zero WLD render method is invisible, even when it has a texture
        // such as COLLIDE.DDS. Keep its slot while resolving polygon runs, but
        // exclude it from drawable geometry. This is independent of whether
        // a polygon is collidable; visible floors and walls are collidable too.
        if entry.flags == 0 {
            continue;
        }

        let masked = entry.flags & (2 | 8 | 16) != 0;
        let mut transparent = entry.flags & (4 | 8) != 0;
        let mut alpha_mask = masked;
        if entry.flags & 0xFFFF == 0x14 {
            // Known quirk: this flag combination is not actually masked.
            alpha_mask = false;
            transparent = false;
        }
        let emissive = entry
            .filenames
            .first()
            .is_some_and(|name| name.eq_ignore_ascii_case("fire1.bmp"));

        let material = Material {
            textures: entry.filenames.clone(),
            normal_map: None,
            water: None,
            flags: entry.flags,
            anim_speed: entry.anim_speed,
            alpha_mask,
            transparent,
            additive: false,
            emissive,
            clamp_uv: false,
            waterfall: None,
            uv_encoding: Default::default(),
        };
        let index = *material_index.entry(material.key()).or_insert_with(|| {
            materials.push(material.clone());
            materials.len() - 1
        });

        let indices = &merged[&(texture, collidable)];
        let (vertices, indices) = if preserve_sources {
            let mut source_vertices = Vec::new();
            let mut remap = HashMap::new();
            let indices = indices
                .iter()
                .map(|&index| {
                    *remap.entry(index).or_insert_with(|| {
                        let next = source_vertices.len() as u32;
                        source_vertices.push(index as usize);
                        next
                    })
                })
                .collect();
            let vertices = source_vertices
                .iter()
                .flat_map(|&index| {
                    positions[index]
                        .into_iter()
                        .chain(normals[index])
                        .chain(uvs[index])
                })
                .collect();
            sources.push(source_vertices);
            (vertices, indices)
        } else {
            pack(&positions, &normals, &uvs, indices)
        };
        geometries.push(Geometry {
            vertices,
            indices,
            material: index,
            collidable,
        });
    }

    (materials, geometries, sources)
}

/// Resolves a reference and returns the mesh fragment it points at.
pub fn mesh_of(wld: &Wld, reference: Ref) -> Option<&Mesh> {
    match wld.resolve(reference).map(|chunk| &chunk.fragment) {
        Some(Fragment::Mesh(mesh)) => Some(mesh),
        _ => None,
    }
}
