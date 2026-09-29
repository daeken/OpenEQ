//! Original rigid item projectiles, grouped by model and instanced each frame.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::{Duration, Instant},
};

use glam::{Mat4, Quat, Vec3};
use openeq_assets::character::CharacterLibrary;

use crate::{GpuActor, GpuScene, Renderer, scene::Instance};

pub const MAX_PROJECTILES: usize = 64;
pub const MAX_PROJECTILE_MODELS: usize = 32;
const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct ProjectileState {
    pub id: u64,
    /// Authored item code from the projectile packet, for example `IT11504`.
    pub model_name: String,
    /// Position and forward direction in scene coordinates (Z up).
    pub position: [f32; 3],
    pub direction: [f32; 3],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProjectileStats {
    pub submitted: usize,
    pub rendered: usize,
    pub invalid: usize,
    pub missing_model: usize,
    pub over_capacity: usize,
    pub cached_models: usize,
}

struct Model {
    actor: GpuActor,
    model_to_forward: Mat4,
    last_seen: Instant,
}

#[derive(Default)]
pub(crate) struct ProjectileRenderer {
    models: BTreeMap<String, Model>,
    missing: BTreeMap<String, Instant>,
    stats: ProjectileStats,
}

impl ProjectileRenderer {
    pub(crate) fn update(
        &mut self,
        renderer: &Renderer,
        library: &CharacterLibrary,
        states: &[ProjectileState],
    ) -> ProjectileStats {
        self.update_at(renderer, library, states, Instant::now())
    }

    fn update_at(
        &mut self,
        renderer: &Renderer,
        library: &CharacterLibrary,
        states: &[ProjectileState],
        now: Instant,
    ) -> ProjectileStats {
        self.models
            .retain(|_, model| now.saturating_duration_since(model.last_seen) < CACHE_TTL);
        self.missing
            .retain(|_, when| now.saturating_duration_since(*when) < CACHE_TTL);
        for model in self.models.values_mut() {
            for draw in &mut model.actor.scene.draws {
                draw.instance_count = 0;
            }
        }
        self.stats = ProjectileStats {
            submitted: states.len(),
            over_capacity: states.len().saturating_sub(MAX_PROJECTILES),
            ..Default::default()
        };
        let mut groups: BTreeMap<String, Vec<Instance>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for state in states.iter().take(MAX_PROJECTILES) {
            let Some((name, instance)) = projectile_instance(state) else {
                self.stats.invalid += 1;
                continue;
            };
            if !seen.insert(state.id) {
                self.stats.invalid += 1;
                continue;
            }
            if groups.len() >= MAX_PROJECTILE_MODELS && !groups.contains_key(&name) {
                self.stats.over_capacity += 1;
                continue;
            }
            groups.entry(name).or_default().push(instance);
        }
        for (name, instances) in &groups {
            if !self.models.contains_key(name) {
                if self.missing.contains_key(name) {
                    self.stats.missing_model += instances.len();
                    continue;
                }
                if self.models.len() >= MAX_PROJECTILE_MODELS {
                    let oldest = self
                        .models
                        .iter()
                        .filter(|(name, _)| !groups.contains_key(*name))
                        .min_by_key(|(_, model)| model.last_seen)
                        .map(|(name, _)| name.clone());
                    if let Some(oldest) = oldest {
                        self.models.remove(&oldest);
                    } else {
                        self.stats.over_capacity += instances.len();
                        continue;
                    }
                }
                match library
                    .load_equipment_scene(name)
                    .map_err(anyhow::Error::from)
                    .and_then(|scene| {
                        let forward = model_forward(&scene);
                        GpuScene::build(renderer.device(), renderer.queue(), &scene).map(|gpu| {
                            (
                                gpu,
                                Mat4::from_quat(Quat::from_rotation_arc(forward, Vec3::X)),
                            )
                        })
                    }) {
                    Ok((scene, model_to_forward)) => {
                        self.models.insert(
                            name.clone(),
                            Model {
                                actor: renderer.prepare_actor(scene),
                                model_to_forward,
                                last_seen: now,
                            },
                        );
                    }
                    Err(error) => {
                        if self.missing.len() >= MAX_PROJECTILE_MODELS {
                            let oldest = self
                                .missing
                                .iter()
                                .min_by_key(|(_, when)| **when)
                                .map(|(name, _)| name.clone());
                            if let Some(oldest) = oldest {
                                self.missing.remove(&oldest);
                            }
                        }
                        self.missing.insert(name.clone(), now);
                        self.stats.missing_model += instances.len();
                        tracing::warn!(model = %name, %error, "original projectile model unavailable");
                        continue;
                    }
                }
            }
            let model = self
                .models
                .get_mut(name)
                .expect("uploaded projectile model");
            model.last_seen = now;
            let instances: Vec<_> = instances
                .iter()
                .map(|instance| Instance {
                    columns: (Mat4::from_cols_array_2d(&instance.columns) * model.model_to_forward)
                        .to_cols_array_2d(),
                })
                .collect();
            model
                .actor
                .scene
                .update_instances(renderer.device(), renderer.queue(), &instances);
            for draw in &mut model.actor.scene.draws {
                draw.instance_start = 0;
                draw.instance_count = instances.len() as u32;
            }
            self.stats.rendered += instances.len();
        }
        self.stats.cached_models = self.models.len();
        self.stats
    }

    pub(crate) fn stats(&self) -> ProjectileStats {
        self.stats
    }

    pub(crate) fn draws(&self) -> impl Iterator<Item = &GpuActor> {
        self.models
            .values()
            .filter(|model| {
                model
                    .actor
                    .scene
                    .draws
                    .iter()
                    .any(|draw| draw.instance_count > 0)
            })
            .map(|model| &model.actor)
    }
}

fn model_forward(scene: &openeq_assets::Scene) -> Vec3 {
    // Classic arrows (including IT10) point along -Z, unlike +X swords. Use
    // their actual head/fletching geometry to establish the direction without
    // guessing an arrow from its numeric item model ID.
    let center = |name: &str| {
        let mut sum = Vec3::ZERO;
        let mut count = 0;
        for mesh in &scene.meshes {
            if scene.materials[mesh.material]
                .textures
                .iter()
                .any(|texture| texture.to_ascii_uppercase().contains(name))
            {
                for vertex in mesh.vertices.chunks_exact(8) {
                    sum += Vec3::from_slice(&vertex[..3]);
                    count += 1;
                }
            }
        }
        (count > 0).then(|| sum / count as f32)
    };
    center("AROHEAD")
        .zip(center("FLETCH"))
        .and_then(|(head, tail)| (head - tail).try_normalize())
        .unwrap_or(Vec3::X)
}

fn projectile_instance(state: &ProjectileState) -> Option<(String, Instance)> {
    let name = state.model_name.trim().to_ascii_uppercase();
    if !(3..=27).contains(&name.len())
        || !name.starts_with("IT")
        || !name[2..].bytes().all(|byte| byte.is_ascii_digit())
        || !state
            .position
            .iter()
            .chain(&state.direction)
            .all(|v| v.is_finite())
    {
        return None;
    }
    let forward = Vec3::from(state.direction).try_normalize()?;
    let item = name[2..].parse::<u32>().ok()?;
    let name = format!("IT{item}");
    // Original rigid weapons retain their longitudinal +X axis. Do not center
    // or rescale their geometry: the authored origin travels with the packet.
    let rotation = Quat::from_rotation_arc(Vec3::X, forward);
    Some((
        name,
        Instance::from_parts(state.position, rotation.to_array(), [1.; 3]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Mat4;

    #[test]
    fn item_transform_preserves_origin_scale_and_points_along_flight_direction() {
        for direction in [[1., 0., 0.], [-1., 0., 0.], [0., 0., 1.], [1., -3., 2.]] {
            let (name, instance) = projectile_instance(&ProjectileState {
                id: 1,
                model_name: "it0001".into(),
                position: [3., -2., 7.],
                direction,
            })
            .unwrap();
            assert_eq!(name, "IT1");
            let matrix = Mat4::from_cols_array_2d(&instance.columns);
            assert!((matrix.transform_point3(Vec3::ZERO) - Vec3::new(3., -2., 7.)).length() < 1e-5);
            assert!(
                (matrix.transform_vector3(Vec3::X) - Vec3::from(direction).normalize()).length()
                    < 1e-5
            );
            assert!((matrix.transform_vector3(Vec3::Y).length() - 1.).abs() < 1e-5);
        }
        let mut state = ProjectileState {
            id: 1,
            model_name: "IT1".into(),
            position: [0.; 3],
            direction: [1., 0., 0.],
        };
        for name in [
            "",
            "../IT1",
            "IT1/foo",
            "other",
            "IT99999999999999999999999",
            "ITé",
        ] {
            state.model_name = name.into();
            assert!(projectile_instance(&state).is_none());
        }
        state.model_name = "IT1".into();
        state.direction = [0.; 3];
        assert!(projectile_instance(&state).is_none());
        state.direction = [1., 0., 0.];
        state.position[0] = f32::NAN;
        assert!(projectile_instance(&state).is_none());
    }

    #[test]
    #[ignore = "requires original item assets and GPU"]
    fn original_projectile_gpu_and_missing_caches_expire_and_remain_bounded() {
        let base = openeq_assets::loader::default_client_dir().expect("original client directory");
        let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
        let arrow = library.load_equipment_scene("IT10").unwrap();
        assert!(
            model_forward(&arrow).z < -0.99,
            "arrow must travel head first"
        );
        let renderer = Renderer::new_headless(16, 16).unwrap();
        let now = Instant::now();
        let mut projectiles = ProjectileRenderer::default();
        let states: Vec<_> = (0..MAX_PROJECTILES + 3)
            .map(|id| ProjectileState {
                id: id as u64,
                model_name: "IT1".into(),
                position: [0.; 3],
                direction: [1., 0., 0.],
            })
            .collect();
        let stats = projectiles.update_at(&renderer, &library, &states, now);
        assert_eq!(stats.rendered, MAX_PROJECTILES);
        assert_eq!(stats.over_capacity, 3);
        assert_eq!(stats.cached_models, 1);
        projectiles.update_at(&renderer, &library, &[], now + Duration::from_secs(1));
        assert_eq!(projectiles.draws().count(), 0);
        assert_eq!(projectiles.models.len(), 1);
        let missing: Vec<_> = (0..MAX_PROJECTILES)
            .map(|id| ProjectileState {
                id: id as u64,
                model_name: format!("IT{}", 9_000_000 + id),
                position: [0.; 3],
                direction: [1., 0., 0.],
            })
            .collect();
        let stats =
            projectiles.update_at(&renderer, &library, &missing, now + Duration::from_secs(2));
        assert_eq!(stats.rendered, 0);
        assert_eq!(stats.missing_model, MAX_PROJECTILE_MODELS);
        assert_eq!(stats.over_capacity, MAX_PROJECTILES - MAX_PROJECTILE_MODELS);
        assert_eq!(projectiles.missing.len(), MAX_PROJECTILE_MODELS);
        projectiles.update_at(&renderer, &library, &[], now + Duration::from_secs(33));
        assert!(projectiles.models.is_empty());
        assert!(projectiles.missing.is_empty());
    }
}
