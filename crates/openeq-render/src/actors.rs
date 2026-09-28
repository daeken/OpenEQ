//! Cached, instanced classic character rendering. One idle/walk pose per model
//! is skinned per frame; NPC transforms remain independent and interpolate in
//! the game layer. The static zone is never rebuilt when an NPC moves.
use crate::{GpuActor, GpuScene, Renderer, scene::Instance};
use glam::Quat;
use openeq_assets::{
    Scene,
    character::{CharacterLibrary, CharacterModel},
    mesh::Geometry,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub struct ActorState {
    pub id: u32,
    pub race: u32,
    pub gender: u8,
    pub size: f32,
    pub position: [f32; 3],
    pub heading: f32,
    pub moving: bool,
}

struct Batch {
    model: CharacterModel,
    poses: Vec<Geometry>,
    actor: GpuActor,
}

pub struct ActorRenderer {
    library: CharacterLibrary,
    batches: BTreeMap<(u32, u8), Batch>,
    unavailable: BTreeSet<(u32, u8)>,
    pub rendered_instances: usize,
}

impl ActorRenderer {
    pub fn load(base: &Path, zone: &str) -> anyhow::Result<Self> {
        Ok(Self {
            library: CharacterLibrary::load(base, zone)?,
            batches: BTreeMap::new(),
            unavailable: BTreeSet::new(),
            rendered_instances: 0,
        })
    }

    pub fn update(&mut self, renderer: &Renderer, states: &[ActorState], time: f32) {
        let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for state in states {
            groups
                .entry((state.race, state.gender))
                .or_default()
                .push(state);
        }
        for key in groups.keys() {
            if self.batches.contains_key(key) || self.unavailable.contains(key) {
                continue;
            }
            match self.build_batch(renderer, *key) {
                Ok(batch) => {
                    tracing::info!(race = key.0, gender = key.1, model = %batch.model.code, "character model uploaded");
                    self.batches.insert(*key, batch);
                }
                Err(error) => {
                    tracing::warn!(race = key.0, gender = key.1, %error, "character model unavailable");
                    self.unavailable.insert(*key);
                }
            }
        }
        self.rendered_instances = 0;
        for (key, batch) in &mut self.batches {
            let group = groups.get(key).map(Vec::as_slice).unwrap_or(&[]);
            let meshes = batch.model.meshes.len();
            let mut instances = Vec::with_capacity(group.len());
            for moving in [false, true] {
                instances.extend(group.iter().filter(|s| s.moving == moving).map(|s| {
                    let height = (batch.model.bounds_max[2] - batch.model.bounds_min[2]).max(0.1);
                    let scale = if s.size > 0. { s.size / height } else { 1. };
                    let rotation = Quat::from_rotation_z(
                        std::f32::consts::FRAC_PI_2 - s.heading * std::f32::consts::TAU / 512.,
                    );
                    Instance::from_parts(s.position, rotation.to_array(), [scale; 3])
                }));
            }
            let idle_count = group.iter().filter(|s| !s.moving).count() as u32;
            for (i, draw) in batch.actor.scene.draws.iter_mut().enumerate() {
                draw.instance_start = if i < meshes { 0 } else { idle_count };
                draw.instance_count = if i < meshes {
                    idle_count
                } else {
                    instances.len() as u32 - idle_count
                };
            }
            if instances.is_empty() {
                continue;
            }
            self.rendered_instances += instances.len();
            batch.model.sample_into(
                batch.model.idle_animation(),
                time,
                &mut batch.poses[..meshes],
            );
            batch.model.sample_into(
                batch.model.walk_animation(),
                time,
                &mut batch.poses[meshes..],
            );
            batch
                .actor
                .scene
                .update_geometry(renderer.queue(), &batch.poses);
            batch
                .actor
                .scene
                .update_instances(renderer.device(), renderer.queue(), &instances);
        }
    }

    fn build_batch(&self, renderer: &Renderer, key: (u32, u8)) -> anyhow::Result<Batch> {
        let model = self.library.load_race(key.0, key.1)?;
        let names: BTreeSet<_> = model
            .materials
            .iter()
            .flat_map(|m| m.textures.iter())
            .collect();
        let textures = names
            .into_iter()
            .filter_map(|name| self.library.texture(name))
            .collect();
        let mut poses = model.meshes.clone();
        poses.extend(model.meshes.clone());
        let scene = Scene::from_geometry(
            model.code.clone(),
            model.materials.clone(),
            poses.clone(),
            textures,
        );
        let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene)?;
        Ok(Batch {
            model,
            poses,
            actor: renderer.prepare_actor(gpu),
        })
    }

    pub fn draws(&self) -> Vec<&GpuActor> {
        self.batches.values().map(|batch| &batch.actor).collect()
    }
}
