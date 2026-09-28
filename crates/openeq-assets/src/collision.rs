//! Conservative static-zone locomotion in EverQuest coordinates (Z is up).
//!
//! This uses the *rendered* scene's collidable triangles. Invisible collision
//! surfaces discarded by mesh baking are unavailable, as are closed doors and
//! dynamic actors. It is therefore a useful walking aid, not authoritative EQ
//! physics. Water is excluded. Callers supply gravity/jump displacement and send
//! the resulting feet position to the server. The body is an upright cylinder
//! rather than an exact rounded capsule; motion is substepped to avoid tunneling.

use crate::{
    Scene,
    mesh::{Geometry, VERTEX_STRIDE},
};
use glam::{Mat4, Quat, Vec2, Vec3};
use std::collections::HashMap;

const CELL_SIZE: f32 = 64.;
const MAX_CELLS_PER_TRIANGLE: i64 = 256;
const WALKABLE_NORMAL_Z: f32 = std::f32::consts::FRAC_1_SQRT_2;
const SKIN: f32 = 0.01;

#[derive(Clone, Debug)]
struct Triangle {
    points: [Vec3; 3],
    normal: Vec3,
    min: Vec3,
    max: Vec3,
}

impl Triangle {
    fn new(points: [Vec3; 3]) -> Option<Self> {
        if !points.iter().all(|point| point.is_finite()) {
            return None;
        }
        let cross = (points[1] - points[0]).cross(points[2] - points[0]);
        if cross.length_squared() < 1e-10 {
            return None;
        }
        Some(Self {
            points,
            normal: cross.normalize(),
            min: points[0].min(points[1]).min(points[2]),
            max: points[0].max(points[1]).max(points[2]),
        })
    }
    fn walkable(&self) -> bool {
        self.normal.z.abs() >= WALKABLE_NORMAL_Z
    }
    fn plane_z(&self, xy: Vec2) -> Option<f32> {
        if self.normal.z.abs() < 1e-6 {
            return None;
        }
        Some(
            self.points[0].z
                - self.normal.truncate().dot(xy - self.points[0].truncate()) / self.normal.z,
        )
    }
    fn ground_at(&self, xy: Vec2) -> Option<f32> {
        if !self.walkable() {
            return None;
        }
        let [a, b, c] = self.points.map(Vec3::truncate);
        let denominator = cross(b - a, c - a);
        let u = cross(xy - a, c - a) / denominator;
        let v = cross(b - a, xy - a) / denominator;
        if u >= -1e-5 && v >= -1e-5 && u + v <= 1. + 1e-5 {
            self.plane_z(xy)
        } else {
            None
        }
    }
}

/// A sparse XY grid. Very large triangles are held in a short global list so
/// broad water/zone bounds cannot allocate millions of grid cells.
#[derive(Default, Debug)]
pub struct CollisionWorld {
    triangles: Vec<Triangle>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    large: Vec<usize>,
}

impl CollisionWorld {
    /// Matches the renderer's object ownership and instance transforms. Source
    /// object meshes without an instance do not accidentally collide at origin.
    pub fn build(scene: &Scene) -> Self {
        let mut world = Self::default();
        let mut owners = vec![None; scene.meshes.len()];
        for (index, object) in scene.objects.iter().enumerate() {
            for &mesh in &object.meshes {
                if let Some(owner) = owners.get_mut(mesh) {
                    *owner = Some(index);
                }
            }
        }
        let mut transforms = vec![Vec::new(); scene.objects.len()];
        for instance in &scene.instances {
            if let Some(index) = scene
                .objects
                .iter()
                .position(|object| object.name == instance.object)
            {
                transforms[index].push(Mat4::from_scale_rotation_translation(
                    Vec3::from(instance.scale),
                    Quat::from_array(instance.rotation),
                    Vec3::from(instance.position),
                ));
            }
        }
        for (index, mesh) in scene.meshes.iter().enumerate() {
            if !mesh.collidable
                || scene
                    .materials
                    .get(mesh.material)
                    .is_some_and(|material| material.water.is_some())
            {
                continue;
            }
            if let Some(owner) = owners[index] {
                for transform in &transforms[owner] {
                    world.add_geometry(mesh, *transform);
                }
            } else {
                world.add_geometry(mesh, Mat4::IDENTITY);
            }
        }
        world
    }

    pub fn triangle_count(&self) -> usize {
        self.triangles.len()
    }

    /// Returns the closest walkable surface to the supplied feet height within
    /// the permitted step/drop interval. Looking up a floor does not snap to the
    /// highest roof above it. Ties prefer the lower surface. Slope limit: 45°.
    pub fn ground_height(
        &self,
        x: f32,
        y: f32,
        feet_z: f32,
        max_step: f32,
        max_drop: f32,
    ) -> Option<f32> {
        if ![x, y, feet_z, max_step, max_drop]
            .iter()
            .all(|n| n.is_finite())
        {
            return None;
        }
        let xy = Vec2::new(x, y);
        self.candidates(xy, xy)
            .into_iter()
            .filter_map(|index| self.triangles[index].ground_at(xy))
            .filter(|z| {
                *z <= feet_z + max_step.max(0.) + SKIN && *z >= feet_z - max_drop.max(0.) - SKIN
            })
            .min_by(|a, b| {
                (a - feet_z)
                    .abs()
                    .total_cmp(&(b - feet_z).abs())
                    .then_with(|| a.total_cmp(b))
            })
    }

    /// Slides an upright body through static geometry and follows reachable
    /// ground. `position` is the feet, `delta` is displacement (not velocity),
    /// all in EQ world units. Supply negative delta Z for gravity, positive for
    /// jumping. A zero vertical delta follows steps; it does not simulate falls.
    ///
    /// Horizontal displacement is divided into intervals <= radius/2 (at most
    /// one unit). More than 256 intervals is conservatively truncated; teleports
    /// should set the position directly. Invalid or initially obstructed moves
    /// leave the player in place rather than pushing through large obstacles.
    pub fn move_player(
        &self,
        position: [f32; 3],
        delta: [f32; 3],
        radius: f32,
        height: f32,
        max_step: f32,
    ) -> [f32; 3] {
        let mut position = Vec3::from(position);
        let mut delta = Vec3::from(delta);
        if !position.is_finite()
            || !delta.is_finite()
            || ![radius, height, max_step].iter().all(|n| n.is_finite())
            || radius <= 0.
            || height <= SKIN * 2.
        {
            return position.to_array();
        }
        let radius = radius.max(SKIN * 2.);
        let max_step = max_step.max(0.).min(height * 0.5);
        let step_length = (radius * 0.5).min(1.);
        let distance = delta.length();
        if distance <= 1e-7 {
            return position.to_array();
        }
        if distance > step_length * 256. {
            delta *= step_length * 256. / distance;
        }
        let steps = (delta.length() / step_length).ceil().max(1.) as usize;
        let motion = delta / steps as f32;
        for _ in 0..steps {
            position = self.move_step(position, motion, radius, height, max_step);
        }
        position.to_array()
    }

    fn move_step(
        &self,
        position: Vec3,
        motion: Vec3,
        radius: f32,
        height: f32,
        max_step: f32,
    ) -> Vec3 {
        let mut desired = position + motion;
        let max_drop = (max_step * 1.5).max(motion.z.abs() + SKIN);
        if motion.z <= 0. {
            if let Some(ground) =
                self.movement_ground(desired.truncate(), position.z, radius, max_step, max_drop)
            {
                // Catch a downward crossing or follow a nearby walkable grade.
                if desired.z <= ground + max_step.max(SKIN) {
                    desired.z = ground;
                }
            }
        } else if let Some(ceiling) = self.ceiling(
            position.truncate(),
            radius,
            position.z + height,
            desired.z + height,
        ) {
            desired.z = (ceiling - height - SKIN).max(position.z);
        }
        let solved = self.slide(desired, position, radius, height);
        let blocked = solved.is_none_or(|point| {
            point.truncate().distance_squared(desired.truncate()) > SKIN * SKIN
        });
        if blocked && motion.truncate().length_squared() > 1e-8 && max_step > SKIN && motion.z <= 0.
        {
            // A radius-ahead support probe lets the body clear a short stair
            // riser before its center crosses the edge. The head sweep and full
            // body check prevent stepping through a low ceiling or tall wall.
            let ahead = desired.truncate() + motion.truncate().normalize() * radius;
            let support =
                self.ground_height(ahead.x, ahead.y, position.z + max_step, 0., max_step - SKIN);
            if let Some(support) = support.filter(|z| *z > position.z + SKIN) {
                let rise = support - position.z;
                let head_clear = self
                    .ceiling(
                        position.truncate(),
                        radius,
                        position.z + height,
                        position.z + height + rise,
                    )
                    .is_none();
                if head_clear {
                    let raised = Vec3::new(desired.x, desired.y, support);
                    if let Some(stepped) = self.slide(raised, position, radius, height)
                        && stepped.truncate().distance_squared(desired.truncate()) <= SKIN * SKIN
                    {
                        return stepped;
                    }
                }
            }
        }
        let Some(mut moved) = solved else {
            return position;
        };
        if motion.z <= 0.
            && let Some(ground) =
                self.movement_ground(moved.truncate(), position.z, radius, max_step, max_drop)
            && moved.z <= ground + max_step.max(SKIN)
        {
            let snapped = Vec3::new(moved.x, moved.y, ground);
            if let Some(clear) = self.slide(snapped, position, radius, height) {
                moved = clear;
            }
        }
        moved
    }

    fn movement_ground(
        &self,
        xy: Vec2,
        feet: f32,
        radius: f32,
        max_step: f32,
        max_drop: f32,
    ) -> Option<f32> {
        let ground = self.ground_height(xy.x, xy.y, feet, max_step, max_drop)?;
        if ground < feet - SKIN {
            // Keep resting on the edge until the body's footprint clears it.
            // Dropping when only the center leaves a step embeds the rear of
            // the body in its riser and incorrectly pushes the player forward.
            let supported = self
                .candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius))
                .into_iter()
                .any(|index| {
                    let triangle = &self.triangles[index];
                    triangle.walkable()
                        && triangle
                            .plane_z(xy)
                            .is_some_and(|z| (z - feet).abs() <= SKIN)
                        && footprint_push(xy, &triangle.points.map(Vec3::truncate), radius, Vec2::X)
                            .is_some()
                });
            if supported {
                return Some(feet);
            }
        }
        Some(ground)
    }

    fn ceiling(&self, xy: Vec2, radius: f32, old_head: f32, new_head: f32) -> Option<f32> {
        if new_head <= old_head {
            return None;
        }
        self.candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius))
            .into_iter()
            .filter_map(|index| {
                let triangle = &self.triangles[index];
                if !triangle.walkable()
                    || triangle.max.z < old_head - SKIN
                    || triangle.min.z > new_head + SKIN
                {
                    return None;
                }
                // Sample the entire circular footprint against the projected
                // triangle, not just the center, so overhang edges stop the head.
                let polygon = triangle.points.map(Vec3::truncate);
                footprint_push(xy, &polygon, radius, Vec2::X)
                    .map(|_| triangle.plane_z(xy).unwrap_or(triangle.min.z))
            })
            .filter(|z| *z >= old_head - SKIN && *z <= new_head + SKIN)
            .min_by(f32::total_cmp)
    }

    fn slide(&self, mut desired: Vec3, previous: Vec3, radius: f32, height: f32) -> Option<Vec3> {
        let original = desired;
        for _ in 0..8 {
            let xy = desired.truncate();
            let mut changed = false;
            for index in self.candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius)) {
                let triangle = &self.triangles[index];
                if triangle.max.z <= desired.z + SKIN || triangle.min.z >= desired.z + height - SKIN
                {
                    continue;
                }
                // Standing on a sloped floor legitimately overlaps its uphill
                // side with the cylindrical footprint. Do not treat that floor
                // as a wall once the feet have been grounded on its plane.
                if triangle.walkable()
                    && triangle
                        .plane_z(desired.truncate())
                        .is_some_and(|z| z <= desired.z + SKIN)
                {
                    continue;
                }
                let polygon = clip_height(
                    &triangle.points,
                    desired.z + SKIN,
                    desired.z + height - SKIN,
                );
                let fallback = (previous.truncate() - desired.truncate())
                    .try_normalize()
                    .or_else(|| triangle.normal.truncate().try_normalize())
                    .unwrap_or(Vec2::X);
                if let Some(push) = footprint_push(desired.truncate(), &polygon, radius, fallback) {
                    if push.length_squared() < SKIN * SKIN * 0.01 {
                        continue;
                    }
                    // Never eject a body from deep inside a ceiling/solid to
                    // some distant edge. Spawn recovery is caller-controlled.
                    if push.length() > radius * 2. {
                        return None;
                    }
                    desired.x += push.x;
                    desired.y += push.y;
                    if desired.truncate().distance(original.truncate()) > radius * 4. {
                        return None;
                    }
                    changed = true;
                }
            }
            if !changed {
                return Some(desired);
            }
        }
        None
    }

    fn add_geometry(&mut self, geometry: &Geometry, transform: Mat4) {
        if !transform.is_finite() {
            return;
        }
        for indices in geometry.indices.chunks_exact(3) {
            let points: Option<Vec<_>> = indices
                .iter()
                .map(|index| {
                    let offset = (*index as usize).checked_mul(VERTEX_STRIDE)?;
                    let coords = geometry.vertices.get(offset..offset + 3)?;
                    Some(transform.transform_point3(Vec3::new(coords[0], coords[1], coords[2])))
                })
                .collect();
            if let Some(points) =
                points.and_then(|points| Triangle::new([points[0], points[1], points[2]]))
            {
                self.add_triangle(points);
            }
        }
    }
    fn add_triangle(&mut self, triangle: Triangle) {
        let index = self.triangles.len();
        let min = cell(triangle.min.truncate());
        let max = cell(triangle.max.truncate());
        let count =
            (max.0 as i64 - min.0 as i64 + 1).saturating_mul(max.1 as i64 - min.1 as i64 + 1);
        if count > MAX_CELLS_PER_TRIANGLE {
            self.large.push(index);
        } else {
            for x in min.0..=max.0 {
                for y in min.1..=max.1 {
                    self.cells.entry((x, y)).or_default().push(index);
                }
            }
        }
        self.triangles.push(triangle);
    }
    fn candidates(&self, min: Vec2, max: Vec2) -> Vec<usize> {
        let min = cell(min);
        let max = cell(max);
        let count =
            (max.0 as i64 - min.0 as i64 + 1).saturating_mul(max.1 as i64 - min.1 as i64 + 1);
        if count > 4096 {
            return (0..self.triangles.len()).collect();
        }
        let mut result = self.large.clone();
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                if let Some(indices) = self.cells.get(&(x, y)) {
                    result.extend(indices);
                }
            }
        }
        result.sort_unstable();
        result.dedup();
        result
    }
}

fn cell(xy: Vec2) -> (i32, i32) {
    (
        (xy.x / CELL_SIZE).floor() as i32,
        (xy.y / CELL_SIZE).floor() as i32,
    )
}
fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

/// Intersects a 3D triangle with the body's vertical range, leaving a convex
/// footprint (or line segment for a vertical wall) in the horizontal plane.
fn clip_height(points: &[Vec3; 3], bottom: f32, top: f32) -> Vec<Vec2> {
    let mut polygon = points.to_vec();
    for (height, above) in [(bottom, true), (top, false)] {
        let input = std::mem::take(&mut polygon);
        if input.is_empty() {
            break;
        }
        let mut previous = *input.last().unwrap();
        let inside = |point: Vec3| {
            if above {
                point.z >= height
            } else {
                point.z <= height
            }
        };
        for current in input {
            if inside(current) != inside(previous) {
                let t = (height - previous.z) / (current.z - previous.z);
                polygon.push(previous.lerp(current, t));
            }
            if inside(current) {
                polygon.push(current);
            }
            previous = current;
        }
    }
    polygon.into_iter().map(Vec3::truncate).collect()
}

/// Minimal translation of a circle out of a convex polygon, including degenerate
/// line polygons. This handles edges/corners rather than relying on face normals.
fn footprint_push(center: Vec2, polygon: &[Vec2], radius: f32, fallback: Vec2) -> Option<Vec2> {
    if polygon.is_empty() {
        return None;
    }
    let mut area = 0.;
    let mut has_positive = false;
    let mut has_negative = false;
    let mut nearest = polygon[0];
    let mut distance_sq = f32::INFINITY;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        area += cross(a, b);
        let signed = cross(b - a, center - a);
        has_positive |= signed > 1e-5;
        has_negative |= signed < -1e-5;
        let edge = b - a;
        let t = if edge.length_squared() > 1e-10 {
            (center - a).dot(edge) / edge.length_squared()
        } else {
            0.
        };
        let point = a + edge * t.clamp(0., 1.);
        let candidate = center.distance_squared(point);
        if candidate < distance_sq {
            distance_sq = candidate;
            nearest = point;
        }
    }
    let inside = area.abs() > 1e-6 && !(has_positive && has_negative);
    let distance = distance_sq.sqrt();
    if !inside && distance >= radius {
        return None;
    }
    let direction = if distance > 1e-6 {
        if inside {
            (nearest - center) / distance
        } else {
            (center - nearest) / distance
        }
    } else {
        fallback
    };
    let penetration = if inside {
        radius + distance
    } else {
        radius - distance
    };
    Some(direction * (penetration + SKIN * 0.1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Instance, SceneObject};

    fn quad(world: &mut CollisionWorld, a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) {
        let [a, b, c, d] = [a, b, c, d].map(Vec3::from);
        world.add_triangle(Triangle::new([a, b, c]).unwrap());
        world.add_triangle(Triangle::new([a, c, d]).unwrap());
    }
    fn floor(world: &mut CollisionWorld, z: f32) {
        quad(
            world,
            [-50., -50., z],
            [50., -50., z],
            [50., 50., z],
            [-50., 50., z],
        );
    }
    fn wall(world: &mut CollisionWorld, x: f32, bottom: f32, top: f32) {
        quad(
            world,
            [x, -20., bottom],
            [x, 20., bottom],
            [x, 20., top],
            [x, -20., top],
        );
    }
    fn near(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 0.025,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn ground_query_preserves_stacked_rooms_and_respects_step_drop() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        floor(&mut world, 4.);
        floor(&mut world, 20.);
        near(world.ground_height(0., 0., 0., 6., 100.).unwrap(), 0.);
        near(world.ground_height(0., 0., 4., 30., 100.).unwrap(), 4.);
        near(world.ground_height(0., 0., 19.9, 1., 100.).unwrap(), 20.);
        assert_eq!(world.ground_height(0., 0., 10., 1., 1.), None);
        assert_eq!(world.ground_height(100., 0., 0., 1., 1.), None);
    }

    #[test]
    fn wall_substeps_prevent_tunneling_and_slide_tangentially() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        wall(&mut world, 5., 0., 20.);
        let stopped = world.move_player([0., 0., 0.], [20., 0., 0.], 0.5, 6., 2.);
        near(stopped[0], 4.5);
        near(stopped[2], 0.);
        let slid = world.move_player([4., 0., 0.], [10., 7., 0.], 0.5, 6., 2.);
        near(slid[0], 4.5);
        near(slid[1], 7.);
        let reverse = world.move_player([10., 0., 0.], [-20., 0., 0.], 0.5, 6., 2.);
        near(reverse[0], 5.5);
    }

    #[test]
    fn stairs_step_up_and_down_but_tall_risers_block() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        wall(&mut world, 5., 0., 2.);
        quad(
            &mut world,
            [5., -20., 2.],
            [15., -20., 2.],
            [15., 20., 2.],
            [5., 20., 2.],
        );
        wall(&mut world, 15., 0., 2.);
        let climbed = world.move_player([0., 0., 0.], [10., 0., 0.], 0.5, 6., 2.5);
        near(climbed[0], 10.);
        near(climbed[2], 2.);
        let descended = world.move_player(climbed, [10., 0., 0.], 0.5, 6., 2.5);
        near(descended[0], 20.);
        near(descended[2], 0.);
        let blocked = world.move_player([0., 0., 0.], [10., 0., 0.], 0.5, 6., 1.);
        near(blocked[0], 4.5);
        near(blocked[2], 0.);
    }

    #[test]
    fn walkable_ramp_is_followed_and_steep_ramp_rejected() {
        let mut world = CollisionWorld::default();
        quad(
            &mut world,
            [0., -10., 0.],
            [20., -10., 10.],
            [20., 10., 10.],
            [0., 10., 0.],
        );
        let moved = world.move_player([1., 0., 0.5], [10., 0., 0.], 0.5, 6., 1.);
        near(moved[0], 11.);
        near(moved[2], 5.5);
        let mut steep = CollisionWorld::default();
        quad(
            &mut steep,
            [0., -10., 0.],
            [10., -10., 20.],
            [10., 10., 20.],
            [0., 10., 0.],
        );
        assert_eq!(steep.ground_height(2., 0., 4., 1., 1.), None);
    }

    #[test]
    fn ceilings_limit_upward_motion_and_prevent_stepping_into_low_room() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        floor(&mut world, 7.);
        let jumped = world.move_player([0., 0., 0.], [0., 0., 3.], 0.5, 6., 2.);
        assert!(jumped[2] <= 1. && jumped[2] > 0.9, "{jumped:?}");
        wall(&mut world, 5., 0., 2.);
        quad(
            &mut world,
            [5., -20., 2.],
            [15., -20., 2.],
            [15., 20., 2.],
            [5., 20., 2.],
        );
        let blocked = world.move_player([0., 0., 0.], [10., 0., 0.], 0.5, 6., 2.5);
        near(blocked[0], 4.5);
        near(blocked[2], 0.);
    }

    #[test]
    fn falling_crosses_floor_without_tunneling_and_disconnected_ledge_does_not_teleport() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        let fallen = world.move_player([0., 0., 10.], [0., 0., -20.], 0.5, 6., 1.);
        near(fallen[2], 0.);
        let mut ledge = CollisionWorld::default();
        quad(
            &mut ledge,
            [0., 0., 0.],
            [2., 0., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
        );
        let walked = ledge.move_player([1., 1., 0.], [4., 0., 0.], 0.5, 6., 1.);
        near(walked[0], 5.);
        near(walked[2], 0.);
        let falling = ledge.move_player(walked, [0., 0., -2.], 0.5, 6., 1.);
        near(falling[2], -2.);
    }

    #[test]
    fn build_uses_instance_translation_rotation_scale_and_collision_flags() {
        let geometry = Geometry {
            vertices: vec![
                0., 0., 0., 0., 0., 1., 0., 0., 2., 0., 0., 0., 0., 1., 0., 0., 0., 2., 0., 0., 0.,
                1., 0., 0.,
            ],
            indices: vec![0, 1, 2],
            material: 0,
            collidable: true,
        };
        let mut scene = Scene::from_geometry(
            "test".into(),
            Vec::new(),
            vec![geometry.clone()],
            Vec::new(),
        );
        scene.objects.push(SceneObject {
            name: "deck".into(),
            meshes: vec![0],
        });
        assert_eq!(CollisionWorld::build(&scene).triangle_count(), 0);
        scene.instances.push(Instance {
            object: "deck".into(),
            position: [10., 20., 5.],
            scale: [2.; 3],
            rotation: Quat::from_rotation_z(std::f32::consts::FRAC_PI_2).to_array(),
        });
        let world = CollisionWorld::build(&scene);
        assert_eq!(world.triangle_count(), 1);
        near(world.ground_height(9., 21., 5., 0., 0.).unwrap(), 5.);
        assert_eq!(world.ground_height(0., 0., 0., 10., 10.), None);
        scene.meshes[0].collidable = false;
        assert_eq!(CollisionWorld::build(&scene).triangle_count(), 0);
    }

    #[test]
    fn inside_corner_stops_both_components_and_does_not_jitter() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        wall(&mut world, 5., 0., 20.);
        quad(
            &mut world,
            [-20., 5., 0.],
            [20., 5., 0.],
            [20., 5., 20.],
            [-20., 5., 20.],
        );
        let mut position = [0., 0., 0.];
        for _ in 0..60 {
            position = world.move_player(position, [0.64, 0.64, -0.1], 0.5, 6., 2.);
        }
        near(position[0], 4.5);
        near(position[1], 4.5);
        near(position[2], 0.);
        let previous = position;
        for _ in 0..60 {
            position = world.move_player(position, [0.64, 0.64, -0.1], 0.5, 6., 2.);
        }
        for i in 0..3 {
            near(position[i], previous[i]);
        }
    }

    #[test]
    #[ignore = "requires original poknowledge assets"]
    fn actual_poknowledge_builds_and_queries_visible_floors() {
        let base = crate::loader::default_client_dir().expect("client assets");
        let scene = crate::loader::load_zone(base, "poknowledge").unwrap();
        let start = std::time::Instant::now();
        let world = CollisionWorld::build(&scene);
        assert!(world.triangle_count() > 1000);
        let build = start.elapsed();
        let mut checked = 0;
        for triangle in world
            .triangles
            .iter()
            .filter(|triangle| triangle.walkable())
            .take(1000)
        {
            let center = (triangle.points[0] + triangle.points[1] + triangle.points[2]) / 3.;
            let z = world
                .ground_height(center.x, center.y, center.z, 0.1, 0.1)
                .unwrap();
            near(z, center.z);
            checked += 1;
        }
        assert_eq!(checked, 1000);
        eprintln!(
            "collision: {} triangles, {} cells, {} oversized triangles, built in {:?}",
            world.triangles.len(),
            world.cells.len(),
            world.large.len(),
            build
        );
    }

    #[test]
    fn huge_floor_stays_bounded_and_queries_negative_cells() {
        let mut world = CollisionWorld::default();
        quad(
            &mut world,
            [-100000., -100000., 0.],
            [100000., -100000., 0.],
            [100000., 100000., 0.],
            [-100000., 100000., 0.],
        );
        assert!(world.cells.is_empty());
        assert_eq!(world.large.len(), 2);
        near(world.ground_height(-500., -700., 0., 1., 1.).unwrap(), 0.);
    }
}
