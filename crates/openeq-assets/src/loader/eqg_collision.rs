//! Physical EQG polygons, independent of material-based drawable batches.
//!
//! EQEmu's current map generator retains material -1 and excludes collision
//! exactly when polygon bit 0 is set. Other flag bits do not override this.
//! See docs/EQG_COLLISION_PLAN.md for pinned generator/consumer evidence.

use std::collections::HashMap;

use glam::DVec3;

use crate::{mesh::CollisionGeometry, zone::TerMod};

/// Collects physical terrain/object geometry in its original coordinate space.
/// The loader owns its placement; no transform or winding change belongs here.
pub(super) fn collect(object: &TerMod) -> Option<CollisionGeometry> {
    let materials: HashMap<_, _> = object
        .materials
        .iter()
        .map(|(&index, material)| (index, super::water_material(material).is_none()))
        .collect();
    let mut geometry = CollisionGeometry::default();
    let mut vertices = HashMap::new();
    let mut invalid_polygons = 0usize;
    let mut missing_material_polygons = 0usize;

    for &(a, b, c, material, flags) in &object.polygons {
        if flags & 1 != 0 {
            continue;
        }
        // Only the signed -1 sentinel is evidence of material-free geometry.
        // An arbitrary unresolved material must not become a hidden barrier.
        if material != u32::MAX {
            match materials.get(&material) {
                Some(true) => {}
                Some(false) => continue,
                None => {
                    missing_material_polygons += 1;
                    continue;
                }
            }
        }
        let source_indices = [a, b, c];
        let [Some(a), Some(b), Some(c)] =
            source_indices.map(|index| object.positions.get(index as usize).copied())
        else {
            invalid_polygons += 1;
            continue;
        };
        let points = [a, b, c].map(|point| DVec3::from_array(point.map(f64::from)));
        let area_squared = (points[1] - points[0])
            .cross(points[2] - points[0])
            .length_squared();
        // Reject malformed source faces without imposing a world-space area
        // threshold here. Small/large local faces can become valid after an
        // instance scale; CollisionWorld checks f32 area after that transform.
        if !points.iter().all(|point| point.is_finite())
            || !area_squared.is_finite()
            || area_squared == 0.
        {
            invalid_polygons += 1;
            continue;
        }
        for (source_index, point) in source_indices.into_iter().zip([a, b, c]) {
            let next = geometry.positions.len() as u32;
            let index = *vertices.entry(source_index).or_insert_with(|| {
                geometry.positions.push(point);
                next
            });
            geometry.indices.push(index);
        }
    }

    if invalid_polygons != 0 || missing_material_polygons != 0 {
        tracing::warn!(
            invalid_polygons,
            missing_material_polygons,
            terrain = object.is_terrain,
            version = object.version,
            "skipped invalid or unresolved EQG collision polygons"
        );
    }
    (!geometry.indices.is_empty()).then_some(geometry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zone::{Property, TerMaterial};

    fn object() -> TerMod {
        TerMod {
            is_terrain: false,
            version: 2,
            materials: HashMap::from([(
                0,
                TerMaterial {
                    name: "ordinary".into(),
                    shader: "Opaque_MaxCB1.fx".into(),
                    properties: HashMap::new(),
                },
            )]),
            positions: vec![[0., 0., 0.], [2., 0., 0.], [0., 2., 0.]],
            normals: vec![[0., 0., 1.]; 3],
            tex_coords: vec![[0.; 2]; 3],
            polygons: vec![(0, 1, 2, 0, 0)],
        }
    }

    #[test]
    fn only_passable_bit_excludes_resolved_physical_faces() {
        let mut object = object();
        // The authored high-bit cases and every low-bit combination guard
        // against nonzero, signed, or CollisionRequired override policies.
        for flags in 0..32 {
            object.polygons[0].4 = flags;
            let expected = if flags % 2 == 0 { 3 } else { 0 };
            assert_eq!(
                collect(&object).map_or(0, |geometry| geometry.indices.len()),
                expected,
                "flags {flags:#010x}"
            );
        }
        for flags in (1..32)
            .map(|bit| 1u32 << bit)
            .chain([0xa0000, 0x80000000, 0xfffffffe])
        {
            object.polygons[0].4 = flags;
            assert_eq!(collect(&object).unwrap().indices, [0, 1, 2]);
            object.polygons[0].4 = flags | 1;
            assert!(collect(&object).is_none(), "flags {:#010x}", flags | 1);
        }
    }

    #[test]
    fn mixed_flags_share_material_without_changing_source_order() {
        let mut object = object();
        object.polygons = vec![
            (0, 2, 1, 0, 0),
            (0, 1, 2, 0, 1),
            (2, 1, 0, 0, 0x80000000),
            (1, 2, 0, 0, 0x80000001),
        ];
        let geometry = collect(&object).unwrap();
        assert_eq!(
            geometry.positions,
            [[0., 0., 0.], [0., 2., 0.], [2., 0., 0.]]
        );
        assert_eq!(geometry.indices, [0, 1, 2, 1, 2, 0]);
        assert_eq!(
            object.mesh_groups()[&0],
            [0, 2, 1, 0, 1, 2, 2, 1, 0, 1, 2, 0]
        );
    }

    #[test]
    fn only_exact_material_sentinel_admits_unresolved_faces() {
        let mut object = object();
        object.materials.clear();
        for flags in [0, 2, 0x80000000, 0xfffffffe] {
            object.polygons[0] = (0, 1, 2, u32::MAX, flags);
            assert_eq!(collect(&object).unwrap().indices, [0, 1, 2]);
            object.polygons[0].4 = flags | 1;
            assert!(collect(&object).is_none());
        }
        for material in [0, 1, 0x80000000, u32::MAX - 1] {
            object.polygons[0] = (0, 1, 2, material, 2);
            assert!(collect(&object).is_none(), "material {material:#010x}");
        }
    }

    #[test]
    fn resolved_water_is_omitted_but_missing_diffuse_is_still_physical() {
        let mut object = object();
        let material = object.materials.get_mut(&0).unwrap();
        material.properties.insert(
            "e_TextureDiffuse0".into(),
            Property::Text("unavailable.dds".into()),
        );
        assert_eq!(collect(&object).unwrap().indices, [0, 1, 2]);
        object.materials.get_mut(&0).unwrap().shader = "oPaQuE_mAxWaTeR.Fx".into();
        assert!(collect(&object).is_none());
        // Material-free faces do not inherit another material's water shader.
        object.polygons[0].3 = u32::MAX;
        assert_eq!(collect(&object).unwrap().indices, [0, 1, 2]);
    }

    #[test]
    fn invalid_source_faces_cannot_publish_vertices() {
        for material in [0, u32::MAX] {
            let mut object = object();
            object.positions.extend([
                [f32::NAN, 0., 0.],
                [0., f32::INFINITY, 0.],
                [10000., 0., 0.],
                [20000., 0., 0.],
            ]);
            object.polygons = vec![
                (0, 1, u32::MAX, material, 0),
                (0, 1, 3, material, 0),
                (0, 1, 4, material, 0),
                (0, 0, 1, material, 0),
                (0, 5, 6, material, 0),
            ];
            assert!(collect(&object).is_none());
            object.polygons.push((0, 1, 2, material, 0));
            let geometry = collect(&object).unwrap();
            assert_eq!(geometry.positions, object.positions[..3]);
            assert_eq!(geometry.indices, [0, 1, 2]);
        }
    }

    #[test]
    fn physical_area_validation_uses_the_placed_object_scale() {
        use crate::{
            collision::CollisionWorld,
            mesh::{self, Geometry},
        };
        use glam::{Mat4, Vec3};

        for material in [0, u32::MAX] {
            for (edge, scale) in [(1e-4, 100.), (1e20, 1e-18)] {
                let mut object = object();
                object.positions = vec![[0., 0., 0.], [edge, 0., 0.], [0., edge, 0.]];
                object.polygons[0].3 = material;
                let physical = collect(&object).unwrap();
                let (vertices, indices) = mesh::pack(
                    &object.positions,
                    &object.normals,
                    &object.tex_coords,
                    &[0, 1, 2],
                );
                let drawable = Geometry {
                    vertices,
                    indices,
                    material: 0,
                    collidable: true,
                };
                let mut local = CollisionWorld::default();
                local.add_collision_geometry(&physical, Mat4::IDENTITY);
                assert_eq!(local.triangle_count(), 0);

                let transform = Mat4::from_scale(Vec3::splat(scale));
                let mut legacy = CollisionWorld::default();
                legacy.add_geometry(&drawable, transform);
                let mut collected = CollisionWorld::default();
                collected.add_collision_geometry(&physical, transform);
                assert_eq!(legacy.triangle_count(), 1);
                assert_eq!(collected.triangle_count(), legacy.triangle_count());
                let interior = edge * scale * 0.25;
                let expected = legacy.ground_height(interior, interior, 0.1, 0., 1.);
                assert_eq!(expected, Some(0.));
                assert_eq!(
                    collected.ground_height(interior, interior, 0.1, 0., 1.),
                    expected
                );
            }
        }
    }

    #[test]
    fn compaction_uses_source_indices_without_merging_equal_positions() {
        let mut object = object();
        object.positions.push(object.positions[0]);
        object.positions.push([10000.; 3]);
        object.polygons = vec![(3, 2, 1, 0, 0), (0, 1, 2, 0, 0)];
        let geometry = collect(&object).unwrap();
        assert_eq!(geometry.positions.len(), 4);
        assert_eq!(geometry.indices, [0, 1, 2, 3, 2, 1]);
        assert_eq!(geometry.positions[0], geometry.positions[3]);
        assert!(!geometry.positions.contains(&[10000.; 3]));
        object.is_terrain = true;
        let terrain = collect(&object).unwrap();
        assert_eq!(terrain.positions, geometry.positions);
        assert_eq!(terrain.indices, geometry.indices);
    }
}
