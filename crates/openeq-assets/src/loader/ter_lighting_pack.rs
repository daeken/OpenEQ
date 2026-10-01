//! Preserve independent TER lighting through ordinary attribute deduplication.
use std::collections::HashMap;

use crate::zone::TerMod;

/// The extra color word is part of vertex identity. Returned source indices
/// refer to original TER vertices, never packed or material-local indices.
/// Equal geometry AND lighting may still merge; a later tangent channel must
/// independently extend this identity before using the representative index.
pub(super) fn pack(
    object: &TerMod,
    indices: &[u32],
    lighting: &[u32],
) -> (Vec<f32>, Vec<u32>, Vec<u32>) {
    assert_eq!(lighting.len(), object.positions.len());
    let mut vertices = Vec::new();
    let mut remap = HashMap::new();
    let mut out_indices = Vec::with_capacity(indices.len());
    let mut sources = Vec::new();
    for &source in indices {
        let i = source as usize;
        let attributes = [
            object.positions[i][0],
            object.positions[i][1],
            object.positions[i][2],
            object.normals[i][0],
            object.normals[i][1],
            object.normals[i][2],
            object.tex_coords[i][0],
            object.tex_coords[i][1],
        ];
        let key = (attributes.map(f32::to_bits), lighting[i]);
        let next = sources.len() as u32;
        let mapped = *remap.entry(key).or_insert(next);
        if mapped == next {
            vertices.extend(attributes);
            sources.push(source);
        }
        out_indices.push(mapped);
    }
    (vertices, out_indices, sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coincident_vertices_with_different_lighting_do_not_merge() {
        let object = TerMod {
            is_terrain: true,
            version: 3,
            materials: vec![],
            positions: vec![[1., 2., 3.]; 4],
            normals: vec![[0., 0., 1.]; 4],
            tex_coords: vec![[0.5, 0.25]; 4],
            vertex_colors: Some(vec![0xff808080; 4]),
            secondary_tex_coords: None,
            polygons: vec![],
        };
        let lighting = [0x001b1202, 0x4c0d0901, 0x001b1202, 0xe5000000];
        let corners = [2, 1, 0, 3, 1, 2];
        let (vertices, indices, sources) = pack(&object, &corners, &lighting);
        assert_eq!(sources, [2, 1, 3]);
        assert_eq!(indices, [0, 1, 0, 2, 1, 0]);
        assert_eq!(vertices.len(), 24);
        for (original, packed) in corners.into_iter().zip(indices) {
            assert_eq!(
                lighting[original as usize],
                lighting[sources[packed as usize] as usize]
            );
        }
        // Identical lighting preserves the old geometry deduplication exactly.
        let (v, i, _) = pack(&object, &corners, &[0x001f1f1f; 4]);
        let (old_v, old_i) = crate::mesh::pack(
            &object.positions,
            &object.normals,
            &object.tex_coords,
            &corners,
        );
        assert_eq!(v, old_v);
        assert_eq!(i, old_i);
    }

    #[test]
    fn source_words_and_first_seen_order_survive_nonfinite_attributes() {
        let mut object = TerMod {
            is_terrain: true,
            version: 2,
            materials: vec![],
            positions: vec![[1., 2., 3.]; 3],
            normals: vec![[0., 0., 1.]; 3],
            tex_coords: vec![[f32::from_bits(0x7fc01234), -0.0]; 3],
            vertex_colors: None,
            secondary_tex_coords: None,
            polygons: vec![],
        };
        object.tex_coords[1][1] = 0.0;
        let lighting = [0x11223344; 3];
        let (v, i, sources) = pack(&object, &[2, 1, 0], &lighting);
        assert_eq!(sources, [2, 1]);
        assert_eq!(i, [0, 1, 0]);
        assert_eq!(v[6].to_bits(), 0x7fc01234);
        assert_eq!(v[7].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(v[15].to_bits(), 0.0_f32.to_bits());
    }
}
