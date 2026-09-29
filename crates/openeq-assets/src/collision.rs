//! Conservative static-zone locomotion in EverQuest coordinates (Z is up).
//!
//! This uses the *rendered* scene's collidable triangles. Invisible collision
//! surfaces discarded by mesh baking are unavailable. Dynamic doors can be
//! supplied through a separate world built with `add_geometry`. Dynamic actors
//! are not represented. It is therefore a useful walking aid, not authoritative EQ
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

    /// Highest actual point of a walkable triangle inside the body footprint.
    /// A plane evaluated at the body center can miss a raised ramp edge, or
    /// invent a higher step beyond the ramp's end. Clip to the circle instead.
    fn max_z_in_footprint(&self, xy: Vec2, radius: f32) -> Option<f32> {
        if !self.walkable() {
            return None;
        }
        let mut highest = self.ground_at(xy);
        let mut include = |z: f32| {
            highest = Some(highest.map_or(z, |previous| previous.max(z)));
        };
        // A linear height function on a disk reaches its maximum uphill. If
        // that point lies outside the triangle, the maximum is on an edge.
        let gradient = -self.normal.truncate() / self.normal.z;
        if let Some(uphill) = gradient.try_normalize()
            && let Some(z) = self.ground_at(xy + uphill * radius)
        {
            include(z);
        }
        for i in 0..3 {
            let a = self.points[i];
            let b = self.points[(i + 1) % 3];
            let edge = (b - a).truncate();
            let length_squared = edge.length_squared();
            if length_squared <= 1e-10 {
                if a.truncate().distance_squared(xy) <= radius * radius {
                    include(a.z.max(b.z));
                }
                continue;
            }
            let center = (xy - a.truncate()).dot(edge) / length_squared;
            let distance_squared = (a.truncate() + edge * center).distance_squared(xy);
            if distance_squared > radius * radius {
                continue;
            }
            let extent = ((radius * radius - distance_squared) / length_squared).sqrt();
            let start = (center - extent).max(0.);
            let end = (center + extent).min(1.);
            if start <= end {
                include(a.z + (b.z - a.z) * start);
                include(a.z + (b.z - a.z) * end);
            }
        }
        highest
    }

    /// Double-sided segment/triangle intersection, expressed as 0..=1 of delta.
    fn segment_hit(&self, origin: Vec3, delta: Vec3) -> Option<f32> {
        let [a, b, c] = self.points;
        let edge1 = b - a;
        let edge2 = c - a;
        let cross = delta.cross(edge2);
        let determinant = edge1.dot(cross);
        if determinant.abs() < 1e-8 {
            return None;
        }
        let inverse = determinant.recip();
        let relative = origin - a;
        let u = relative.dot(cross) * inverse;
        let cross = relative.cross(edge1);
        let v = delta.dot(cross) * inverse;
        let t = edge2.dot(cross) * inverse;
        (u >= -1e-5 && v >= -1e-5 && u + v <= 1.00001 && (0. ..=1.).contains(&t)).then_some(t)
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

/// Borrow static and moving geometry together so support and clearance are
/// solved in the same pass without rebuilding either spatial index.
struct CollisionQuery<'a> {
    world: &'a CollisionWorld,
    dynamic: Option<&'a CollisionWorld>,
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

    /// Keeps an orbit camera on the unobstructed segment from `focus` toward
    /// `desired`. Both sides of triangles block, including ceilings and slopes.
    /// `padding` is clearance along that segment in world units; this is a point
    /// ray, not a body sweep, and it does not slide or depenetrate the focus.
    pub fn clip_camera(&self, focus: [f32; 3], desired: [f32; 3], padding: f32) -> [f32; 3] {
        let origin = Vec3::from(focus);
        let desired = Vec3::from(desired);
        if !origin.is_finite() || !desired.is_finite() || !padding.is_finite() {
            return focus;
        }
        let delta = desired - origin;
        let distance = delta.length();
        if distance <= 1e-7 {
            return focus;
        }
        let hit = self
            .candidates(
                origin.min(desired).truncate(),
                origin.max(desired).truncate(),
            )
            .into_iter()
            .filter_map(|index| self.triangles[index].segment_hit(origin, delta))
            .min_by(f32::total_cmp);
        let fraction = hit.map_or(1., |t| (t - padding.max(0.) / distance).max(0.));
        (origin + delta * fraction).to_array()
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

    /// Ground contact across the body's circular footprint, including narrow
    /// overlaps at rotated ledges that a few point samples can miss.
    pub fn supports_player(&self, feet: [f32; 3], radius: f32, tolerance: f32) -> bool {
        let feet = Vec3::from(feet);
        feet.is_finite()
            && radius.is_finite()
            && radius > 0.
            && tolerance.is_finite()
            && tolerance >= 0.
            && CollisionQuery {
                world: self,
                dynamic: None,
            }
            .supported(feet.truncate(), feet.z, radius, tolerance)
    }

    /// Repairs a slightly embedded arrival by raising feet onto a nearby floor.
    /// This is for an authoritative spawn/teleport only, never ordinary motion.
    /// It does not lower an airborne body, cross a gap, move sideways, or accept
    /// a position whose full upright body intersects a wall or ceiling.
    pub fn recover_player_from_floor(
        &self,
        feet: [f32; 3],
        radius: f32,
        height: f32,
        max_rise: f32,
    ) -> Option<[f32; 3]> {
        let feet = Vec3::from(feet);
        if !feet.is_finite()
            || ![radius, height, max_rise]
                .iter()
                .all(|value| value.is_finite())
            || radius <= 0.
            || height <= SKIN * 2.
            || max_rise <= SKIN
        {
            return None;
        }
        // A recovery remains a small step, even for an accidental large input.
        let max_rise = max_rise.min(height * 0.5);
        let floor = self.ground_height(feet.x, feet.y, feet.z, max_rise, 0.)?;
        if floor <= feet.z + SKIN || floor > feet.z + max_rise {
            return None;
        }
        let raised = Vec3::new(feet.x, feet.y, floor);
        let query = CollisionQuery {
            world: self,
            dynamic: None,
        };
        let clear = query.slide(raised, raised, radius, height)?;
        // Clearance must hold at the supplied XY; accepting a slide here could
        // silently move a character around a wall or through a narrow opening.
        (clear.distance_squared(raised) <= 1e-8).then_some(raised.to_array())
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
        self.move_player_with_dynamic(None, position, delta, radius, height, max_step)
    }

    /// Uses one support/clearance solve for static terrain and moving objects.
    /// Sequential solves can snap a player off a platform to the floor below.
    pub fn move_player_with_dynamic(
        &self,
        dynamic: Option<&Self>,
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
        let query = CollisionQuery {
            world: self,
            dynamic,
        };
        for _ in 0..steps {
            position = query.move_step(position, motion, radius, height, max_step);
        }
        position.to_array()
    }
}

impl<'a> CollisionQuery<'a> {
    fn candidates(&self, min: Vec2, max: Vec2) -> impl Iterator<Item = &'a Triangle> {
        std::iter::once(self.world)
            .chain(self.dynamic)
            .flat_map(move |world| {
                world
                    .candidates(min, max)
                    .into_iter()
                    .map(|i| &world.triangles[i])
            })
    }

    fn ground_height(
        &self,
        x: f32,
        y: f32,
        feet: f32,
        max_step: f32,
        max_drop: f32,
    ) -> Option<f32> {
        std::iter::once(self.world)
            .chain(self.dynamic)
            .filter_map(|world| world.ground_height(x, y, feet, max_step, max_drop))
            .min_by(|a, b| {
                (a - feet)
                    .abs()
                    .total_cmp(&(b - feet).abs())
                    .then_with(|| a.total_cmp(b))
            })
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
            // Check the same circular footprint used by collision. A single
            // forward probe never reaches a ledge at a glancing approach: the
            // side of the body touches the riser before that probe reaches its
            // top. Try the lowest reachable support first, retaining head and
            // full-body clearance checks for ceilings and tall obstacles.
            let xy = desired.truncate();
            let mut supports: Vec<_> = self
                .candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius))
                .filter_map(|triangle| {
                    triangle
                        .max_z_in_footprint(xy, radius)
                        .filter(|z| *z > position.z + SKIN && *z <= position.z + max_step + SKIN)
                })
                .collect();
            supports.sort_by(f32::total_cmp);
            supports.dedup_by(|a, b| (*a - *b).abs() < SKIN);
            for support in supports {
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
        let ground = self.ground_height(xy.x, xy.y, feet, max_step, max_drop);
        if ground.is_none_or(|z| z < feet - SKIN) {
            // Keep resting on the edge until the body's footprint clears it.
            // Dropping when only the center leaves a step embeds the rear of
            // the body in its riser and incorrectly pushes the player forward.
            if self.supported(xy, feet, radius, SKIN) {
                return Some(feet);
            }
        }
        ground
    }

    fn supported(&self, xy: Vec2, feet: f32, radius: f32, tolerance: f32) -> bool {
        self.candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius))
            .any(|triangle| {
                // Ordinary ramps use center support; raised edges use the
                // highest contact inside the circle until the body clears it.
                triangle
                    .ground_at(xy)
                    .is_some_and(|z| (z - feet).abs() <= tolerance + SKIN)
                    || triangle
                        .max_z_in_footprint(xy, radius)
                        .is_some_and(|z| (z - feet).abs() <= tolerance + SKIN)
            })
    }

    fn ceiling(&self, xy: Vec2, radius: f32, old_head: f32, new_head: f32) -> Option<f32> {
        if new_head <= old_head {
            return None;
        }
        self.candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius))
            .filter_map(|triangle| {
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
            for triangle in self.candidates(xy - Vec2::splat(radius), xy + Vec2::splat(radius)) {
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
}

impl CollisionWorld {
    /// Adds a prefiltered local mesh at a world transform. Useful for small
    /// dynamic-object worlds; callers decide which materials are collidable.
    pub fn add_geometry(&mut self, geometry: &Geometry, transform: Mat4) {
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
    fn camera_segment_stops_at_floor_from_either_side() {
        let mut world = CollisionWorld::default();
        floor(&mut world, 0.);
        let above = world.clip_camera([0., 0., 5.], [10., 0., -5.], 0.5);
        let below = world.clip_camera([10., 0., -5.], [0., 0., 5.], 0.5);
        near(above[0], 5. - 0.5 / 2f32.sqrt());
        near(above[2], 0.5 / 2f32.sqrt());
        near(below[0], 5. + 0.5 / 2f32.sqrt());
        near(below[2], -0.5 / 2f32.sqrt());
        assert_eq!(
            world.clip_camera([0., 0., 5.], [10., 0., 5.], 1.),
            [10., 0., 5.]
        );
    }

    #[test]
    fn camera_segment_uses_nearest_diagonal_wall_without_sliding() {
        let mut world = CollisionWorld::default();
        // Diagonal plane x+y=10; the ray reaches it at (7.5, 2.5, 5).
        quad(
            &mut world,
            [0., 10., 0.],
            [10., 0., 0.],
            [10., 0., 10.],
            [0., 10., 10.],
        );
        wall(&mut world, 12., 0., 10.);
        let camera = world.clip_camera([0., 0., 5.], [15., 5., 5.], 0.5);
        near(camera[0] / camera[1], 3.);
        near(Vec3::from(camera).distance(Vec3::new(7.5, 2.5, 5.)), 0.5);
        assert!(camera[0] + camera[1] < 10.);
        assert_eq!(
            world.clip_camera([0., 0., 5.], [15., 5., 5.], 100.),
            [0., 0., 5.]
        );
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
    fn rotated_ledge_support_covers_the_whole_footprint() {
        let rotation = Quat::from_rotation_z(22.5f32.to_radians());
        let points = [
            [5., -20., 2.],
            [20., -20., 2.],
            [20., 20., 2.],
            [5., 20., 2.],
        ]
        .map(|p| (rotation * Vec3::from(p)).to_array());
        let mut world = CollisionWorld::default();
        quad(&mut world, points[0], points[1], points[2], points[3]);
        // The circle overlaps by 0.01, between the old eight edge probes.
        let edge = (rotation * Vec3::new(4.01, 0., 2.)).to_array();
        assert!(world.supports_player(edge, 1., 0.05));
        near(world.move_player(edge, [0., 0., -0.1], 1., 6., 2.)[2], 2.);
        let clear = (rotation * Vec3::new(3.9, 0., 2.)).to_array();
        assert!(!world.supports_player(clear, 1., 0.05));
        near(world.move_player(clear, [0., 0., -0.1], 1., 6., 2.)[2], 1.9);
    }

    #[test]
    fn combined_worlds_preserve_wall_and_ceiling_clearance() {
        for (rise, roof) in [(0.5, 6.25), (2.5, 20.)] {
            let mut terrain = CollisionWorld::default();
            floor(&mut terrain, 0.);
            floor(&mut terrain, roof);
            let mut platform = CollisionWorld::default();
            wall(&mut platform, 5., 0., rise);
            quad(
                &mut platform,
                [5., -20., rise],
                [15., -20., rise],
                [15., 20., rise],
                [5., 20., rise],
            );
            for (world, dynamic) in [(&terrain, &platform), (&platform, &terrain)] {
                let moved = world.move_player_with_dynamic(
                    Some(dynamic),
                    [0.; 3],
                    [10., 2., -0.1],
                    1.,
                    6.,
                    2.,
                );
                near(moved[0], 4.);
                near(moved[1], 2.);
                near(moved[2], 0.);
            }
        }
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
    fn footprint_support_uses_local_slope_height_and_stops_at_triangle_edges() {
        let ramp = Triangle::new([
            Vec3::new(0., -20., 0.),
            Vec3::new(20., -20., 10.),
            Vec3::new(0., 20., 0.),
        ])
        .unwrap();
        // The uphill contact is inside the face, far below its distant peak.
        near(ramp.max_z_in_footprint(Vec2::new(4., 0.), 1.).unwrap(), 2.5);
        // Only a small sliver of the left edge overlaps the circular body.
        near(
            ramp.max_z_in_footprint(Vec2::new(-0.75, 0.), 1.).unwrap(),
            0.125,
        );
        // A square approximation would incorrectly reach this corner.
        assert_eq!(ramp.max_z_in_footprint(Vec2::new(-0.8, 20.8), 1.), None);
        // Contact is clipped to the actual high vertex, never its infinite plane.
        near(
            ramp.max_z_in_footprint(Vec2::new(20.5, -20.), 1.).unwrap(),
            10.,
        );
        assert_eq!(ramp.max_z_in_footprint(Vec2::new(21.1, -20.), 1.), None);
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
