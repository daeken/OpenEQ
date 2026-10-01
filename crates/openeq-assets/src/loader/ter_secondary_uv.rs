//! Source-only dual-coordinate preservation; shader admission stays unchanged.
use std::collections::HashMap;

use super::PackedTerSecondaryUv;
use crate::zone::{TerMaterial, TerMod};

pub(super) fn channel<'a>(object: &'a TerMod, material: &TerMaterial) -> Option<&'a [[f32; 2]]> {
    if !object.is_terrain
        || !matches!(object.version, 1..=3)
        || !matches!(
            material.shader.as_str(),
            "Opaque_MaxCB1_2UV.fx" | "Opaque_MaxCBSG1_2UV.fx"
        )
    {
        return None;
    }
    object
        .secondary_tex_coords
        .as_deref()
        .filter(|coords| coords.len() == object.positions.len())
}

pub(super) fn pack(
    object: &TerMod,
    indices: &[u32],
    secondary: &[[f32; 2]],
) -> (Vec<f32>, Vec<u32>, PackedTerSecondaryUv) {
    assert_eq!(secondary.len(), object.positions.len());
    let mut vertices = Vec::new();
    let mut out_indices = Vec::with_capacity(indices.len());
    let mut metadata = PackedTerSecondaryUv {
        tex_coords: Vec::new(),
        source_indices: Vec::new(),
    };
    let mut remap = HashMap::new();
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
        let key = (attributes.map(f32::to_bits), secondary[i].map(f32::to_bits));
        let next = metadata.source_indices.len() as u32;
        let mapped = *remap.entry(key).or_insert(next);
        if mapped == next {
            vertices.extend(attributes);
            metadata.tex_coords.push(secondary[i]);
            metadata.source_indices.push(source);
        }
        out_indices.push(mapped);
    }
    (vertices, out_indices, metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loader::Scene, mesh::UvEncoding};

    fn fixture() -> TerMod {
        TerMod {
            is_terrain: true,
            version: 2,
            materials: vec![TerMaterial {
                stored_id: 9,
                name: "dual".into(),
                shader: "Opaque_MaxCB1_2UV.fx".into(),
                properties: HashMap::new(),
            }],
            positions: vec![[1., 2., 3.]; 4],
            normals: vec![[0., 0., 1.]; 4],
            tex_coords: vec![[0.125, -0.25]; 4],
            vertex_colors: None,
            secondary_tex_coords: Some(vec![
                [f32::from_bits(0x7fff0005), -0.0],
                [f32::from_bits(0x7fff0005), 0.0],
                [f32::from_bits(0x7fff0005), -0.0],
                [f32::from_bits(0x7f800001), -0.0],
            ]),
            polygons: vec![(2, 1, 0, 0, 0), (3, 1, 2, 0, 1)],
        }
    }

    #[test]
    fn uv1_bits_prevent_lossy_merges_and_preserve_corner_order() {
        let object = fixture();
        let corners = [2, 1, 0, 3, 1, 2];
        let secondary = channel(&object, &object.materials[0]).unwrap();
        let (v, i, m) = pack(&object, &corners, secondary);
        assert_eq!(m.source_indices, [2, 1, 3]);
        assert_eq!(i, [0, 1, 0, 2, 1, 0]);
        assert_eq!(v.len(), 24);
        for (&source, &packed) in corners.iter().zip(&i) {
            assert_eq!(
                secondary[source as usize].map(f32::to_bits),
                m.tex_coords[packed as usize].map(f32::to_bits)
            );
        }
        assert_eq!(
            m.tex_coords
                .iter()
                .map(|p| p.map(f32::to_bits))
                .collect::<Vec<_>>(),
            [
                [0x7fff0005, 0x80000000],
                [0x7fff0005, 0],
                [0x7f800001, 0x80000000]
            ]
        );
        // If UV1 is identical, preserve existing geometry packing exactly.
        let (v, i, _) = pack(&object, &corners, &[[1., 2.]; 4]);
        let old = crate::mesh::pack(
            &object.positions,
            &object.normals,
            &object.tex_coords,
            &corners,
        );
        assert_eq!((v, i), old);
    }

    #[test]
    fn admission_requires_exact_ter_family_version_and_complete_channel() {
        let mut object = fixture();
        for shader in ["Opaque_MaxCB1_2UV.fx", "Opaque_MaxCBSG1_2UV.fx"] {
            object.materials[0].shader = shader.into();
            for version in 1..=3 {
                object.version = version;
                assert!(channel(&object, &object.materials[0]).is_some());
            }
        }
        for version in [0, 4] {
            object.version = version;
            assert!(channel(&object, &object.materials[0]).is_none());
        }
        object.version = 2;
        object.is_terrain = false;
        assert!(channel(&object, &object.materials[0]).is_none());
        object.is_terrain = true;
        for shader in [
            "opaque_maxcb1_2uv.fx",
            "Opaque_MaxCB1.fx",
            "Opaque_MaxWaterFall.fx",
            "Alpha_MaxCB1_2UV.fx",
        ] {
            object.materials[0].shader = shader.into();
            assert!(channel(&object, &object.materials[0]).is_none());
        }
        object.materials[0].shader = "Opaque_MaxCB1_2UV.fx".into();
        object.secondary_tex_coords.as_mut().unwrap().pop();
        assert!(channel(&object, &object.materials[0]).is_none());
        object.secondary_tex_coords = None;
        assert!(channel(&object, &object.materials[0]).is_none());
    }

    #[test]
    fn bake_retains_metadata_without_admitting_shader_or_changing_collision() {
        let mut object = fixture();
        object.positions[1] = [2., 2., 3.];
        object.positions[3] = [1., 3., 3.];
        object.polygons = vec![(0, 1, 3, 0, 0), (2, 3, 1, 0, 1)];
        let mut scene = Scene::from_geometry(
            "dual".into(),
            vec![],
            vec![],
            vec![crate::texture::Texture {
                name: "missing.dds".into(),
                width: 1,
                height: 1,
                rgba: vec![255; 4],
            }],
        );
        super::super::append_eqg_object(&mut scene, &object, "terrain", 0, None);
        let metadata = &scene.secondary_ter_uv[&0];
        assert_eq!(metadata.tex_coords.len(), scene.meshes[0].vertex_count());
        assert_eq!(scene.meshes[0].indices.len(), 6);
        assert_eq!(scene.collision_meshes[0].indices.len(), 3);
        assert_eq!(scene.materials[0].uv_encoding, UvEncoding::Float32);
        assert!(scene.materials[0].waterfall.is_none());
        assert!(scene.native_ter_lighting.is_empty());
        assert!(!scene.meshes[0].collidable);
    }
}
