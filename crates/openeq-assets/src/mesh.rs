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

/// A drawable surface's material description.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// Diffuse texture names; more than one means an animated flipbook.
    pub textures: Vec<String>,
    /// Optional normal map (only seen in `.eqg` zones).
    pub normal_map: Option<String>,
    /// Raw texture flags from the source data.
    pub flags: u32,
    /// Milliseconds per animation frame, when animated.
    pub anim_speed: u32,
    /// Cut out fully transparent texels (1-bit alpha).
    pub alpha_mask: bool,
    /// Blend using the texture's alpha channel.
    pub transparent: bool,
    /// Unlit/emissive surface, e.g. fire.
    pub emissive: bool,
}

impl Material {
    fn key(&self) -> (u32, u32, String, bool, bool, bool) {
        (
            self.flags,
            self.anim_speed,
            self.textures.join(","),
            self.alpha_mask,
            self.transparent,
            self.emissive,
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
fn texture_entries(wld: &Wld, mesh: &Mesh) -> Vec<TextureEntry> {
    let Some(Fragment::MaterialList(list)) = wld.resolve(mesh.materials).map(|c| &c.fragment)
    else {
        return Vec::new();
    };

    let mut entries = Vec::with_capacity(list.materials.len());
    for material_ref in &list.materials {
        let Some(Fragment::Material(material)) = wld.resolve(*material_ref).map(|c| &c.fragment)
        else {
            continue;
        };
        let Some(Fragment::AnimationRef(animation_ref)) =
            wld.resolve(material.animation).map(|c| &c.fragment)
        else {
            continue;
        };
        let Some(Fragment::Animation(animation)) =
            wld.resolve(animation_ref.animation).map(|c| &c.fragment)
        else {
            continue;
        };

        let mut filenames = Vec::new();
        for texture_ref in &animation.textures {
            if let Some(Fragment::TextureList(list)) =
                wld.resolve(*texture_ref).map(|c| &c.fragment)
            {
                filenames.extend(list.filenames.iter().cloned());
            }
        }

        entries.push(TextureEntry {
            flags: material.flags,
            anim_speed: animation.frame_time,
            filenames,
        });
    }
    entries
}

/// Bakes a set of `0x36` fragments into drawable geometry and materials.
pub fn bake_wld_meshes<'a, I>(wld: &Wld, meshes: I) -> (Vec<Material>, Vec<Geometry>)
where
    I: IntoIterator<Item = &'a Mesh>,
{
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
    for ((texture, collidable), indices) in groups {
        merged
            .entry((remap[texture], collidable))
            .or_default()
            .extend(indices);
    }

    let mut materials = Vec::new();
    let mut material_index: HashMap<(u32, u32, String, bool, bool, bool), usize> = HashMap::new();
    let mut geometries = Vec::new();

    // Stable ordering keeps output deterministic between runs.
    let mut keys: Vec<_> = merged.keys().copied().collect();
    keys.sort_by_key(|(texture, collidable)| (*texture, *collidable));

    for key in keys {
        let (texture, collidable) = key;
        let entry = &unique[texture];
        if entry.flags == 0 && entry.filenames.is_empty() {
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
            flags: entry.flags,
            anim_speed: entry.anim_speed,
            alpha_mask,
            transparent,
            emissive,
        };
        let index = *material_index.entry(material.key()).or_insert_with(|| {
            materials.push(material.clone());
            materials.len() - 1
        });

        let indices = &merged[&(texture, collidable)];
        let (vertices, indices) = pack(&positions, &normals, &uvs, indices);
        geometries.push(Geometry {
            vertices,
            indices,
            material: index,
            collidable,
        });
    }

    (materials, geometries)
}

/// Resolves a reference and returns the mesh fragment it points at.
pub fn mesh_of(wld: &Wld, reference: Ref) -> Option<&Mesh> {
    match wld.resolve(reference).map(|chunk| &chunk.fragment) {
        Some(Fragment::Mesh(mesh)) => Some(mesh),
        _ => None,
    }
}
