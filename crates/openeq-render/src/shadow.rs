//! Directional shadow projection in renderer (Y-up) space.

use glam::{Mat4, Vec3};

pub const RADIUS: f32 = 300.0;

/// `to_sun` is the same surface-to-light direction used for diffuse lighting.
pub fn view_projection(focus: Vec3, to_sun: Vec3, size: u32) -> Mat4 {
    let distance = 1600.0;
    let rotation = Mat4::look_at_rh(Vec3::ZERO, -to_sun, Vec3::Y);
    let mut center = rotation.transform_point3(focus);
    let texel = 2.0 * RADIUS / size as f32;
    // Keep the sampling grid fixed in world space as the camera moves. Merely
    // rounding the camera in world axes does not stabilize the light's axes.
    center.x = (center.x / texel).round() * texel;
    center.y = (center.y / texel).round() * texel;
    let view = Mat4::from_translation(-center - Vec3::Z * distance) * rotation;
    let projection = Mat4::orthographic_rh(-RADIUS, RADIUS, -RADIUS, RADIUS, 1.0, distance * 2.0);
    projection * view
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sunward_occluders_are_closer_than_receivers() {
        let sun = Vec3::new(-0.45, 0.82, 0.35).normalize();
        let focus = Vec3::new(500.0, 40.0, -900.0);
        let projection = view_projection(focus, sun, 2048);
        let receiver = projection.project_point3(focus);
        let occluder = projection.project_point3(focus + sun * 100.0);
        assert!((receiver.truncate() - occluder.truncate()).length() < 1e-5);
        assert!(occluder.z < receiver.z);
        assert!(occluder.z > 0.0 && receiver.z < 1.0);
        // Geometry below a horizontal plane must not shadow it from above.
        assert!(projection.project_point3(focus - Vec3::Y * 10.0).z > receiver.z);
    }

    #[test]
    fn camera_motion_does_not_slide_the_shadow_texel_grid() {
        let sun = Vec3::new(-0.45, 0.82, 0.35).normalize();
        let rotation = Mat4::look_at_rh(Vec3::ZERO, -sun, Vec3::Y);
        let right = rotation.inverse().transform_vector3(Vec3::X);
        let texel = 2.0 * RADIUS / 2048.0;
        let point = Vec3::new(25.0, 30.0, -50.0);
        let original = view_projection(Vec3::ZERO, sun, 2048).project_point3(point);
        let small = view_projection(right * texel * 0.25, sun, 2048).project_point3(point);
        assert!((original - small).length() < 1e-6);
        let moved = view_projection(right * texel * 1.25, sun, 2048).project_point3(point);
        assert!(((original.x - moved.x) * 1024.0 - 1.0).abs() < 1e-4);
    }
}
