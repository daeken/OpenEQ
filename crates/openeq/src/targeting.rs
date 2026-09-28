//! Screen picking from the same animated bounds used to draw each actor.
use glam::{Vec3, Vec4};
use openeq_render::{Camera, actors::ActorBounds};
use std::collections::BTreeMap;

/// Logical-pixel rectangle, clipped at the camera's near plane.
pub fn screen_bounds(camera: &Camera, size: [f32; 2], bounds: &ActorBounds) -> Option<[f32; 4]> {
    if size.iter().any(|v| !v.is_finite() || *v <= 0.) {
        return None;
    }
    let matrix = camera.view_projection(size[0] / size[1], 0.2, 20000.);
    let corners = bounds
        .corners()
        .map(|p| matrix * Camera::to_world(p).extend(1.));
    let mut min = [f32::INFINITY; 2];
    let mut max = [f32::NEG_INFINITY; 2];
    let mut include = |clip: Vec4| {
        if !clip.is_finite() || clip.w <= 0. || clip.z < -1e-5 {
            return;
        }
        let ndc = clip.truncate() / clip.w;
        let p = [(ndc.x * 0.5 + 0.5) * size[0], (0.5 - ndc.y * 0.5) * size[1]];
        for axis in 0..2 {
            min[axis] = min[axis].min(p[axis]);
            max[axis] = max[axis].max(p[axis]);
        }
    };
    for (i, &corner) in corners.iter().enumerate() {
        include(corner);
        for axis in 0..3 {
            let next = i ^ (1 << axis);
            if next > i && (corner.z < 0.) != (corners[next].z < 0.) {
                include(corner.lerp(corners[next], corner.z / (corner.z - corners[next].z)));
            }
        }
    }
    (min[0].is_finite() && min[1].is_finite()).then_some([min[0], min[1], max[0], max[1]])
}

fn ray_hit(origin: Vec3, direction: Vec3, bounds: &ActorBounds) -> Option<f32> {
    let mut near = 0f32;
    let mut far = f32::INFINITY;
    for axis in 0..3 {
        if direction[axis].abs() < 1e-7 {
            if origin[axis] < bounds.min[axis] || origin[axis] > bounds.max[axis] {
                return None;
            }
        } else {
            let a = (bounds.min[axis] - origin[axis]) / direction[axis];
            let b = (bounds.max[axis] - origin[axis]) / direction[axis];
            near = near.max(a.min(b));
            far = far.min(a.max(b));
        }
    }
    (far >= near).then_some(near)
}

/// Pick the closest body hit; a small screen-space margin helps with tiny or
/// distant models. All positions and margins use logical, not physical, pixels.
pub fn pick_actor(
    actors: &BTreeMap<u32, ActorBounds>,
    own_id: Option<u32>,
    camera: &Camera,
    size: [f32; 2],
    point: [f32; 2],
) -> Option<u32> {
    if size.iter().any(|v| !v.is_finite() || *v <= 0.) || point.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let inverse = camera
        .view_projection(size[0] / size[1], 0.2, 20000.)
        .inverse();
    let near = inverse.project_point3(Vec3::new(
        point[0] / size[0] * 2. - 1.,
        1. - point[1] / size[1] * 2.,
        0.,
    ));
    let direction = (near - Camera::to_world(camera.position)).normalize();
    let direction = Vec3::new(direction.x, -direction.z, direction.y);
    let origin = Vec3::from(camera.position);
    actors
        .iter()
        .filter_map(|(&id, bounds)| {
            if Some(id) == own_id {
                return None;
            }
            let [left, top, right, bottom] = screen_bounds(camera, size, bounds)?;
            let dx = (left - point[0]).max(point[0] - right).max(0.);
            let dy = (top - point[1]).max(point[1] - bottom).max(0.);
            if dx > 6. || dy > 6. {
                return None;
            }
            let hit = ray_hit(origin, direction, bounds);
            let distance = hit.unwrap_or_else(|| origin.distance(Vec3::from(bounds.center())));
            if distance > 250. {
                return None;
            }
            Some((id, hit.is_none(), dx * dx + dy * dy, distance))
        })
        .min_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| a.2.total_cmp(&b.2))
                .then_with(|| a.3.total_cmp(&b.3))
        })
        .map(|hit| hit.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn camera() -> Camera {
        Camera {
            position: [0., -20., 4.],
            yaw: 0.,
            pitch: 0.,
            ..Default::default()
        }
    }
    fn project(camera: &Camera, size: [f32; 2], p: [f32; 3]) -> [f32; 2] {
        let clip =
            camera.view_projection(size[0] / size[1], 0.2, 20000.) * Camera::to_world(p).extend(1.);
        [
            (clip.x / clip.w * 0.5 + 0.5) * size[0],
            (0.5 - clip.y / clip.w * 0.5) * size[1],
        ]
    }

    #[test]
    fn whole_animated_body_is_clickable_at_any_viewport_size() {
        // A broad, airborne bat whose animation is offset above its root.
        let bat = ActorBounds {
            min: [-8., -1., 6.],
            max: [8., 1., 15.],
        };
        let actors = BTreeMap::from([(12, bat)]);
        for size in [[800., 600.], [1600., 1200.]] {
            for p in [[-7.8, 0., 7.], [7.8, 0., 7.], [0., 0., 14.8], [0., 0., 6.2]] {
                assert_eq!(
                    pick_actor(&actors, None, &camera(), size, project(&camera(), size, p)),
                    Some(12)
                );
            }
            assert_eq!(
                pick_actor(
                    &actors,
                    Some(12),
                    &camera(),
                    size,
                    project(&camera(), size, bat.center())
                ),
                None
            );
        }
        assert!(bat.nameplate_anchor()[2] > bat.max[2]);
    }

    #[test]
    fn overlapping_bodies_pick_the_nearer_actor_and_ignore_behind_camera() {
        let actors = BTreeMap::from([
            (
                1,
                ActorBounds {
                    min: [-3., 10., 0.],
                    max: [3., 12., 8.],
                },
            ),
            (
                2,
                ActorBounds {
                    min: [-2., 0., 0.],
                    max: [2., 2., 8.],
                },
            ),
            (
                3,
                ActorBounds {
                    min: [-2., -30., 0.],
                    max: [2., -28., 8.],
                },
            ),
        ]);
        assert_eq!(
            pick_actor(&actors, None, &camera(), [800., 600.], [400., 300.]),
            Some(2)
        );
        assert!(screen_bounds(&camera(), [800., 600.], &actors[&3]).is_none());
        assert_eq!(
            pick_actor(&actors, None, &camera(), [800., 600.], [5., 5.]),
            None
        );
    }

    #[test]
    fn body_crossing_near_plane_keeps_its_visible_selection_area() {
        let bounds = ActorBounds {
            min: [-1., -20.5, 0.],
            max: [1., -19., 8.],
        };
        let rect = screen_bounds(&camera(), [800., 600.], &bounds).unwrap();
        assert!(rect[0] < 400. && rect[2] > 400. && rect[1] < 300. && rect[3] > 300.);
        assert_eq!(
            pick_actor(
                &BTreeMap::from([(1, bounds)]),
                None,
                &camera(),
                [800., 600.],
                [400., 300.]
            ),
            Some(1)
        );
    }
}
