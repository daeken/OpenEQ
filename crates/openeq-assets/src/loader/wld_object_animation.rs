//! Stable render bindings for supported animation with stationary collision.
use super::*;

/// Initial-bake vertex identity; buffers never deduplicate separate motion owners.
#[derive(Debug, Clone)]
pub struct RenderAnimation {
    /// Flattened source-part vertex indices for each baked geometry, in order.
    vertices: Vec<Vec<usize>>,
    /// Conservative origin-centered radius enclosing all interpolated poses.
    radius: f32,
}

impl RenderAnimation {
    pub(super) fn new(vertices: Vec<Vec<usize>>, radius: f32) -> Self {
        Self { vertices, radius }
    }
    pub fn vertices(&self) -> &[Vec<usize>] {
        &self.vertices
    }
    pub fn radius(&self) -> f32 {
        self.radius
    }
}

impl ObjectSource {
    /// Only actor-owned bakes with supported clips and provably stationary
    /// physical vertices have render bindings. Extracted standalone models do
    /// not by themselves authorize automatic controller ownership.
    pub fn render_animation(&self) -> Option<&RenderAnimation> {
        self.render_animation.as_ref()
    }

    /// Sufficient, deliberately conservative gate. Every collidable source
    /// vertex must belong to an entirely one-frame ancestry, including hidden
    /// faces. A finite sweep of sampled poses cannot establish this property.
    pub fn stationary_collision_animation_radius(&self) -> Result<f32> {
        self.animation_period()?;
        posed_meshes(self)?;
        let skeleton = self.skeleton.as_ref().unwrap();
        let (parents, order) = hierarchy(&skeleton.definition)?;
        let mut stationary = vec![false; skeleton.tracks.len()];
        let mut scales = vec![0f64; skeleton.tracks.len()];
        let mut distances = scales.clone();
        for index in order {
            let frame = &skeleton.tracks[index].definition.frames[0];
            stationary[index] = skeleton.tracks[index].definition.frames.len() == 1
                && parents[index].is_none_or(|parent| stationary[parent]);
            let (parent_scale, parent_distance) =
                parents[index].map_or((1., 0.), |parent| (scales[parent], distances[parent]));
            // Positive uniform scales and unit rotations permit an origin
            // sphere bound, independent of rotation extrema or sample times.
            // Roundoff slack is accumulated at each ancestry stage.
            scales[index] = parent_scale * f64::from(frame.scale) * 1.0001 + 0.00001;
            distances[index] =
                (parent_distance + parent_scale * length(frame.translation)) * 1.0001 + 0.00001;
            if !scales[index].is_finite() || !distances[index].is_finite() {
                return Err(invalid("object animation ancestry bounds overflow"));
            }
        }
        let mut radius = 0f64;
        for part in &self.parts {
            let bindings = part_bindings(part, skeleton.tracks.len())?;
            for polygon in part
                .mesh
                .polygons
                .iter()
                .filter(|polygon| polygon.collidable)
            {
                for index in [polygon.a, polygon.b, polygon.c] {
                    let track = bindings
                        .get(index as usize)
                        .ok_or_else(|| invalid("invalid collidable animation vertex"))?;
                    if !stationary[*track] {
                        return Err(invalid("object animation moves collision ancestry"));
                    }
                }
            }
            for (point, track) in part.mesh.vertices.iter().zip(bindings) {
                let extent = distances[track] + scales[track] * length(*point);
                if !extent.is_finite() {
                    return Err(invalid("object animation bounds overflow"));
                }
                radius = radius.max(extent);
            }
        }
        let radius = ((radius * 1.0001 + 0.00001) as f32).next_up();
        if !radius.is_finite() {
            return Err(invalid("object animation bounds are not representable"));
        }
        Ok(radius)
    }
}

fn length(point: [f32; 3]) -> f64 {
    point
        .into_iter()
        .map(|v| f64::from(v).powi(2))
        .sum::<f64>()
        .sqrt()
}
