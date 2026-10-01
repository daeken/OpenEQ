//! Startup bounds of submitted finite triangles and supported animation envelopes.
use glam::{Mat4, Vec3};
use openeq_assets::mesh::{Geometry, VERTEX_STRIDE};

#[derive(Default)]
pub(super) struct DrawBounds(Option<(Vec3, Vec3)>);

impl DrawBounds {
    pub fn from_radius(radius: f32) -> Self {
        Self(Some((Vec3::splat(-radius), Vec3::splat(radius))))
    }

    pub fn from_geometry(geometry: &Geometry) -> Self {
        let mut bounds = Self::default();
        for triangle in geometry.indices.chunks_exact(3) {
            let points = [triangle[0], triangle[1], triangle[2]].map(|index| {
                let start = (index as usize).checked_mul(VERTEX_STRIDE)?;
                let position = geometry
                    .vertices
                    .get(start..start.checked_add(VERTEX_STRIDE)?)?;
                let point = Vec3::new(position[0], position[1], position[2]);
                point.is_finite().then_some(point)
            });
            // Invalid triangles stay in their original buffers for diagnosis.
            // They cannot establish a finite extent for the startup camera.
            if let [Some(a), Some(b), Some(c)] = points {
                for point in [a, b, c] {
                    bounds.include(point);
                }
            }
        }
        bounds
    }

    pub fn include_transformed(&mut self, local: &Self, transform: Mat4) {
        let Some((min, max)) = local.0 else {
            return;
        };
        // Eight corners once per instance, rather than rewalking every vertex
        // of repeated foliage. This also supports reflections and uneven scale.
        let corners: [Vec3; 8] = std::array::from_fn(|bits| {
            transform.transform_point3(Vec3::new(
                if bits & 1 == 0 { min.x } else { max.x },
                if bits & 2 == 0 { min.y } else { max.y },
                if bits & 4 == 0 { min.z } else { max.z },
            ))
        });
        if corners.iter().all(|point| point.is_finite()) {
            for point in corners {
                self.include(point);
            }
        }
    }

    fn include(&mut self, point: Vec3) {
        self.0 = Some(match self.0 {
            Some((min, max)) => (min.min(point), max.max(point)),
            None => (point, point),
        });
    }

    pub fn extents(&self) -> (Vec3, Vec3) {
        self.0.unwrap_or((Vec3::ZERO, Vec3::ZERO))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Quat;

    fn fixture() -> Geometry {
        Geometry {
            vertices: [[-1., -2., -3.], [1., 2., -3.], [1., 2., 3.], [1e20; 3]]
                .into_iter()
                .flat_map(|[x, y, z]| [x, y, z, 0., 0., 1., 0., 0.])
                .collect(),
            indices: vec![0, 1, 2],
            material: 0,
            collidable: true,
        }
    }

    #[test]
    fn reflected_scaled_and_rotated_instances_enclose_the_world_geometry() {
        let source = fixture();
        let local = DrawBounds::from_geometry(&source);
        assert_eq!(
            local.extents(),
            (Vec3::new(-1., -2., -3.), Vec3::new(1., 2., 3.))
        );
        let mut world = DrawBounds::default();
        world.include_transformed(
            &local,
            Mat4::from_scale_rotation_translation(
                Vec3::new(-2., 3., 4.),
                Quat::from_rotation_z(std::f32::consts::FRAC_PI_2),
                Vec3::new(100., 200., 300.),
            ),
        );
        let (min, max) = world.extents();
        assert!(min.abs_diff_eq(Vec3::new(94., 198., 288.), 0.0001));
        assert!(max.abs_diff_eq(Vec3::new(106., 202., 312.), 0.0001));
        // A second placement must contribute independently of its definition.
        world.include_transformed(&local, Mat4::from_translation(Vec3::splat(-500.)));
        assert_eq!(world.extents().0, Vec3::new(-501., -502., -503.));
        assert_eq!(world.extents().1, max);
    }

    #[test]
    fn only_complete_finite_indexed_triangles_establish_bounds() {
        let mut source = fixture();
        source.indices = vec![0, 1];
        assert_eq!(
            DrawBounds::from_geometry(&source).extents(),
            (Vec3::ZERO, Vec3::ZERO)
        );
        source.indices = vec![0, 1, u32::MAX];
        assert_eq!(
            DrawBounds::from_geometry(&source).extents(),
            (Vec3::ZERO, Vec3::ZERO)
        );
        source.vertices[3 * VERTEX_STRIDE] = f32::NAN;
        source.indices = vec![0, 1, 2, 1, 2, 3];
        let local = DrawBounds::from_geometry(&source);
        assert_eq!(
            local.extents(),
            (Vec3::new(-1., -2., -3.), Vec3::new(1., 2., 3.))
        );
        let mut world = DrawBounds::default();
        world.include_transformed(&local, Mat4::from_scale(Vec3::splat(f32::MAX)));
        world.include_transformed(&local, Mat4::from_translation(Vec3::splat(f32::NAN)));
        assert_eq!(world.extents(), (Vec3::ZERO, Vec3::ZERO));
        world.include_transformed(&local, Mat4::IDENTITY);
        assert_eq!(world.extents(), local.extents());
        assert!(source.vertices[3 * VERTEX_STRIDE].is_nan());
        assert_eq!(source.indices, [0, 1, 2, 1, 2, 3]);
    }
}
