//! Shared placed-actor poses. Only validated source-index bakes participate.
use super::Vertex;
use openeq_assets::{Scene, SceneObject, loader::wld_objects::ObjectSource};
use std::{
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

struct MeshUpload {
    range: Range<usize>,
    sources: Vec<usize>,
    template: Vec<Vertex>,
}

pub(super) struct PlacedAnimation {
    source: Arc<ObjectSource>,
    meshes: Vec<MeshUpload>,
    radius: f32,
    failed: AtomicBool,
}

impl PlacedAnimation {
    pub fn prepare(
        scene: &Scene,
        object: &SceneObject,
        ranges: &[Range<usize>],
        vertices: &[Vertex],
    ) -> anyhow::Result<Option<Self>> {
        let Some(source) = scene.wld_object_sources.get(&object.name) else {
            return Ok(None);
        };
        let Some(binding) = source.render_animation() else {
            return Ok(None);
        };
        // No submitted triangles means no visible controller or animation
        // envelope; invisible-only definitions must not expand scene bounds.
        if !object.meshes.iter().any(|&index| {
            scene
                .meshes
                .get(index)
                .is_some_and(|mesh| !mesh.indices.is_empty())
        }) {
            return Ok(None);
        }
        let radius = source.stationary_collision_animation_radius()?;
        anyhow::ensure!(
            binding.vertices().len() == object.meshes.len(),
            "animation mesh count changed"
        );
        let posed = source.sample_animation(Duration::ZERO)?;
        let flat: Vec<_> = posed
            .iter()
            .flat_map(|mesh| mesh.vertices.iter().zip(&mesh.normals))
            .collect();
        let mut meshes = Vec::new();
        for (&mesh_index, sources) in object.meshes.iter().zip(binding.vertices()) {
            let range = ranges
                .get(mesh_index)
                .ok_or_else(|| anyhow::anyhow!("animation mesh missing"))?
                .clone();
            let template = vertices
                .get(range.clone())
                .ok_or_else(|| anyhow::anyhow!("animation vertex range missing"))?
                .to_vec();
            anyhow::ensure!(
                template.len() == sources.len(),
                "animation vertex count changed"
            );
            for (vertex, &index) in template.iter().zip(sources) {
                let (position, normal) = flat
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("animation source vertex missing"))?;
                anyhow::ensure!(
                    vertex.position == **position && vertex.normal == **normal,
                    "animation source no longer matches initial baked pose"
                );
            }
            meshes.push(MeshUpload {
                range,
                sources: sources.clone(),
                template,
            });
        }
        Ok(Some(Self {
            source: Arc::clone(source),
            meshes,
            radius,
            failed: AtomicBool::new(false),
        }))
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn update(&self, queue: &wgpu::Queue, buffer: &wgpu::Buffer, elapsed: Duration) {
        if self.failed.load(Ordering::Relaxed) {
            return;
        }
        match self.pose(elapsed) {
            Ok(uploads) => {
                for (mesh, vertices) in self.meshes.iter().zip(uploads) {
                    if !vertices.is_empty() {
                        queue.write_buffer(
                            buffer,
                            (mesh.range.start * std::mem::size_of::<Vertex>()) as u64,
                            bytemuck::cast_slice(&vertices),
                        );
                    }
                }
            }
            Err(error) => {
                self.failed.store(true, Ordering::Relaxed);
                tracing::warn!(actor=%self.source.actor_name, %error, "stopped invalid placed animation");
            }
        }
    }

    fn pose(&self, elapsed: Duration) -> anyhow::Result<Vec<Vec<Vertex>>> {
        let posed = self.source.sample_animation(elapsed)?;
        let flat: Vec<_> = posed
            .iter()
            .flat_map(|mesh| mesh.vertices.iter().zip(&mesh.normals))
            .collect();
        // Validate/build every range before submitting any part of this pose.
        self.meshes
            .iter()
            .map(|mesh| {
                let mut vertices = mesh.template.clone();
                for (vertex, &index) in vertices.iter_mut().zip(&mesh.sources) {
                    let (position, normal) = flat
                        .get(index)
                        .ok_or_else(|| anyhow::anyhow!("animation source vertex missing"))?;
                    vertex.position = **position;
                    vertex.normal = **normal;
                }
                Ok(vertices)
            })
            .collect()
    }
}
