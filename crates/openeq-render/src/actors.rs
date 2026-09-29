//! Cached character appearance batches with independent action timelines.
//! Actors that currently share a pose are instanced; reusable pose slots grow
//! only when a batch needs more simultaneous poses. Zone geometry is untouched.
use crate::{GpuActor, GpuScene, Renderer, scene::Instance, upload::UploadContext};
use glam::{Mat4, Quat, Vec3};
pub use openeq_assets::character::{CharacterAppearance, CharacterModelSet, EquipmentAppearance};
use openeq_assets::{
    Scene,
    character::{CharacterLibrary, CharacterModel, CharacterPose},
    mesh::Geometry,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ActorAction {
    #[default]
    Auto,
    Stand,
    Walk,
    Run,
    Swim,
    Sit,
    Duck,
    Dead,
    Attack,
    Hit,
    Cast,
    /// Original EQ OP_Animation action number (combat, spell, social, etc.).
    Animation(u8),
}

#[derive(Debug, Clone, Default)]
pub struct ActorState {
    pub id: u32,
    pub race: u32,
    pub gender: u8,
    pub size: f32,
    pub position: [f32; 3],
    pub heading: f32,
    pub moving: bool,
    pub appearance: CharacterAppearance,
    pub action: ActorAction,
    /// Increment on every action event, including repeats of the same action.
    pub action_sequence: u64,
}

/// Bounds of the pose actually submitted to the renderer, in scene coordinates.
/// UI and picking must use these rather than guessing a body from spawn size:
/// some creatures are wide or animate well above their authored origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActorBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl ActorBounds {
    fn from_geometry(meshes: &[Geometry]) -> Option<Self> {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for vertex in meshes.iter().flat_map(|mesh| mesh.vertices.chunks_exact(8)) {
            let p = Vec3::from_slice(&vertex[..3]);
            if p.is_finite() {
                min = min.min(p);
                max = max.max(p);
            }
        }
        (min.is_finite() && max.is_finite()).then_some(Self {
            min: min.to_array(),
            max: max.to_array(),
        })
    }

    pub fn corners(&self) -> [[f32; 3]; 8] {
        std::array::from_fn(|i| {
            std::array::from_fn(|axis| {
                if i & (1 << axis) == 0 {
                    self.min[axis]
                } else {
                    self.max[axis]
                }
            })
        })
    }

    fn transformed(self, matrix: Mat4) -> Self {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for corner in self.corners() {
            let p = matrix.transform_point3(Vec3::from(corner));
            min = min.min(p);
            max = max.max(p);
        }
        Self {
            min: min.to_array(),
            max: max.to_array(),
        }
    }

    pub fn center(&self) -> [f32; 3] {
        std::array::from_fn(|axis| (self.min[axis] + self.max[axis]) * 0.5)
    }

    pub fn nameplate_anchor(&self) -> [f32; 3] {
        let [x, y, _] = self.center();
        [x, y, self.max[2] + 0.5]
    }
}

type AppearanceKey = (u32, u8, CharacterAppearance);

/// Animated spell attachment locations, in scene coordinates. Missing sockets
/// remain absent so non-humanoid models can use an explicit body fallback.
#[derive(Debug, Clone, Copy, Default)]
pub struct ActorSockets {
    pub head: Option<[f32; 3]>,
    pub chest: Option<[f32; 3]>,
    pub left_hand: Option<[f32; 3]>,
    pub right_hand: Option<[f32; 3]>,
    pub left_foot: Option<[f32; 3]>,
    pub right_foot: Option<[f32; 3]>,
    pub left_hand_rotation: Option<[f32; 4]>,
    pub right_hand_rotation: Option<[f32; 4]>,
}

fn socket_indices(model: &CharacterModel) -> [Option<usize>; 6] {
    let find = |names: &[&str]| {
        names.iter().find_map(|name| {
            model
                .bone_names
                .iter()
                .position(|bone| bone == name || bone.get(3..) == Some(*name))
        })
    };
    [
        find(&["HEAD_POINT_TRACK", "HEAD_HEAD", "HE_TRACK"]),
        find(&[
            "CHEST_POINT_TRACK",
            "CHCHEST2_TRACK",
            "CH_TRACK",
            "CHEST_CHEST03",
        ]),
        find(&["L_POINT_TRACK", "ARML_WEAP", "ARML_HAND"]),
        find(&["R_POINT_TRACK", "ARMR_WEAP", "ARMR_HAND"]),
        find(&["BO_L_TRACK", "BOFOOTL_TRACK", "LEGL_FOOT"]),
        find(&["BO_R_TRACK", "BOFOOTR_TRACK", "LEGR_FOOT"]),
    ]
}

struct Batch {
    model: CharacterModel,
    poses: Vec<Geometry>,
    actor: GpuActor,
    capacity: usize,
    last_seen: f32,
    socket_indices: [Option<usize>; 6],
}
struct Timeline {
    action: ActorAction,
    sequence: u64,
    started: f32,
    changed: bool,
    appearance: Option<AppearanceKey>,
    previous: Option<Pose>,
    transition: Option<Transition>,
    last_time: f32,
}
struct Transition {
    from: Pose,
    started: f32,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Pose {
    clip: String,
    tick: u32,
    looping: bool,
}
impl Pose {
    fn sample(&self) -> CharacterPose<'_> {
        CharacterPose {
            animation: &self.clip,
            time_seconds: self.tick as f32 / 30.,
            looping: self.looping,
        }
    }
}

// Five 30 Hz steps make transitions short enough to preserve combat response.
// The key contains poses, never actor IDs: equal blends still share geometry.
const TRANSITION_STEPS: u8 = 5;
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DrawPose {
    to: Pose,
    from: Option<Pose>,
    step: u8,
}

impl Timeline {
    fn pose(&mut self, key: AppearanceKey, to: Pose, time: f32) -> DrawPose {
        if self.appearance != Some(key) || time < self.last_time {
            self.previous = None;
            self.transition = None;
        }
        let switched = self
            .previous
            .as_ref()
            .is_some_and(|old| self.changed || old.clip != to.clip || old.looping != to.looping);
        if switched {
            self.transition = self.previous.clone().map(|from| Transition {
                from,
                started: time,
            });
        }
        self.changed = false;
        self.last_time = time;
        self.appearance = Some(key);
        self.previous = Some(to.clone());
        let mut result = DrawPose {
            to,
            from: None,
            step: TRANSITION_STEPS,
        };
        if let Some(transition) = &self.transition {
            let step = ((time - transition.started).max(0.) * 30.).round() as u8;
            if step < TRANSITION_STEPS {
                result.from = Some(transition.from.clone());
                result.step = step;
            } else {
                self.transition = None;
            }
        }
        result
    }
}

pub struct ActorRenderer {
    library: CharacterLibrary,
    batches: BTreeMap<AppearanceKey, Batch>,
    unavailable: BTreeSet<(u32, u8)>,
    timelines: BTreeMap<u32, Timeline>,
    bounds: BTreeMap<u32, ActorBounds>,
    sockets: BTreeMap<u32, ActorSockets>,
    projectiles: crate::projectiles::ProjectileRenderer,
    pub rendered_instances: usize,
}

impl ActorRenderer {
    pub fn load(base: &Path, zone: &str) -> anyhow::Result<Self> {
        Self::load_with_model_set(base, zone, CharacterModelSet::Classic)
    }

    pub fn load_with_model_set(
        base: &Path,
        zone: &str,
        model_set: CharacterModelSet,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            library: CharacterLibrary::load_with_model_set(base, zone, model_set)?,
            batches: BTreeMap::new(),
            unavailable: BTreeSet::new(),
            timelines: BTreeMap::new(),
            bounds: BTreeMap::new(),
            sockets: BTreeMap::new(),
            projectiles: Default::default(),
            rendered_instances: 0,
        })
    }

    pub fn update(&mut self, renderer: &Renderer, states: &[ActorState], time: f32) {
        self.preload(&renderer.upload_context(), states, time);
    }

    /// Submit the complete current projectile snapshot; an empty slice clears
    /// visible projectiles while retaining a bounded, expiring model cache.
    pub fn update_projectiles(
        &mut self,
        renderer: &Renderer,
        states: &[crate::projectiles::ProjectileState],
    ) -> crate::projectiles::ProjectileStats {
        self.projectiles.update(renderer, &self.library, states)
    }

    pub fn projectile_stats(&self) -> crate::projectiles::ProjectileStats {
        self.projectiles.stats()
    }

    /// Decodes models and uploads the initial appearances, poses and instances.
    /// Call on an asset worker with a cloned upload context; later frame updates
    /// reuse these batches. `time` uses the same clock as [`Self::update`].
    pub fn preload(&mut self, upload: &UploadContext, states: &[ActorState], time: f32) {
        self.preload_with_progress(upload, states, time, |_, _| true);
    }

    /// Reports completed/total appearance batches and checks cancellation
    /// between batches. A false result leaves a partial snapshot: the loading
    /// worker must discard it rather than publish it to the active renderer.
    pub fn preload_with_progress(
        &mut self,
        upload: &UploadContext,
        states: &[ActorState],
        time: f32,
        mut progress: impl FnMut(usize, usize) -> bool,
    ) -> bool {
        let time = if time.is_finite() { time.max(0.) } else { 0. };
        let mut groups: BTreeMap<AppearanceKey, Vec<_>> = BTreeMap::new();
        let mut present = BTreeSet::new();
        for state in states {
            present.insert(state.id);
            let timeline = self.timelines.entry(state.id).or_insert(Timeline {
                action: state.action,
                sequence: state.action_sequence,
                started: time,
                changed: false,
                appearance: None,
                previous: None,
                transition: None,
                last_time: time,
            });
            if timeline.action != state.action
                || timeline.sequence != state.action_sequence
                || timeline.started > time
            {
                if timeline.started > time {
                    timeline.previous = None;
                    timeline.transition = None;
                }
                timeline.action = state.action;
                timeline.sequence = state.action_sequence;
                timeline.started = time;
                timeline.changed = true;
            }
            groups
                .entry((
                    state.race,
                    state.gender,
                    self.library.normalize_race_appearance(
                        state.race,
                        state.gender,
                        state.appearance,
                    ),
                ))
                .or_default()
                .push(state);
        }
        self.timelines.retain(|id, _| present.contains(id));
        // Repeated equipment changes should not retain every old texture set.
        self.batches
            .retain(|key, batch| groups.contains_key(key) || time - batch.last_seen < 30.);
        let total = groups.len();
        for (completed, key) in groups.keys().enumerate() {
            if !progress(completed, total) {
                return false;
            }
            if self.batches.contains_key(key) || self.unavailable.contains(&(key.0, key.1)) {
                continue;
            }
            match self.build_batch(upload, *key, time) {
                Ok(batch) => {
                    tracing::info!(race=key.0, gender=key.1, model=%batch.model.code, "character appearance uploaded");
                    self.batches.insert(*key, batch);
                }
                Err(error) => {
                    tracing::warn!(race=key.0, gender=key.1, %error, "character model unavailable");
                    self.unavailable.insert((key.0, key.1));
                }
            }
        }
        self.rendered_instances = 0;
        self.bounds.clear();
        self.sockets.clear();
        for (key, batch) in &mut self.batches {
            if !progress(total, total) {
                return false;
            }
            for draw in &mut batch.actor.scene.draws {
                draw.instance_count = 0;
            }
            let Some(group) = groups.get(key) else {
                continue;
            };
            batch.last_seen = time;
            let mut poses: BTreeMap<DrawPose, Vec<&ActorState>> = BTreeMap::new();
            for state in group {
                let timeline = self
                    .timelines
                    .get_mut(&state.id)
                    .expect("present actor timeline");
                let elapsed = time - timeline.started;
                let to = select_pose(&batch.model, state, time, elapsed);
                poses
                    .entry(timeline.pose(*key, to, time))
                    .or_default()
                    .push(state);
            }
            if poses.len() > batch.capacity {
                let capacity = poses.len().next_power_of_two();
                match upload_actor(upload, &self.library, &batch.model, capacity) {
                    Ok((actor, geometry)) => {
                        batch.actor = actor;
                        batch.poses = geometry;
                        batch.capacity = capacity;
                    }
                    Err(error) => {
                        tracing::warn!(%error, "character pose allocation failed");
                        continue;
                    }
                }
            }
            let meshes = batch.model.meshes.len();
            let mut instances = Vec::with_capacity(group.len());
            for (slot, (pose, actors)) in poses.iter().enumerate() {
                let start = instances.len() as u32;
                let geometry = &mut batch.poses[slot * meshes..(slot + 1) * meshes];
                if let Some(from) = &pose.from {
                    batch.model.sample_blended_into(
                        from.sample(),
                        pose.to.sample(),
                        f32::from(pose.step) / f32::from(TRANSITION_STEPS),
                        geometry,
                    );
                } else {
                    batch.model.sample_into_mode(
                        &pose.to.clip,
                        pose.to.tick as f32 / 30.,
                        pose.to.looping,
                        geometry,
                    );
                }
                let bounds = ActorBounds::from_geometry(geometry);
                let transforms = batch.model.attachment_transforms(
                    pose.from.as_ref().unwrap_or(&pose.to).sample(),
                    pose.to.sample(),
                    f32::from(pose.step) / f32::from(TRANSITION_STEPS),
                );
                let height = (batch.model.bounds_max[2] - batch.model.bounds_min[2]).max(0.1);
                for state in actors {
                    let instance = actor_instance(state, height);
                    let matrix = Mat4::from_cols_array_2d(&instance.columns);
                    if let Some(transforms) = &transforms {
                        let hand_rotation = |index: usize| {
                            batch.socket_indices[index]
                                .and_then(|index| transforms.get(index))
                                .map(|bone| {
                                    (matrix * *bone)
                                        .to_scale_rotation_translation()
                                        .1
                                        .to_array()
                                })
                        };
                        let [head, chest, left_hand, right_hand, left_foot, right_foot] =
                            batch.socket_indices.map(|index| {
                                index.and_then(|index| {
                                    transforms.get(index).map(|bone| {
                                        (matrix * *bone).transform_point3(Vec3::ZERO).to_array()
                                    })
                                })
                            });
                        self.sockets.insert(
                            state.id,
                            ActorSockets {
                                head,
                                chest,
                                left_hand,
                                right_hand,
                                left_foot,
                                right_foot,
                                left_hand_rotation: hand_rotation(2),
                                right_hand_rotation: hand_rotation(3),
                            },
                        );
                    }
                    if let Some(bounds) = bounds {
                        self.bounds.insert(state.id, bounds.transformed(matrix));
                    }
                    instances.push(instance);
                }
                for draw in &mut batch.actor.scene.draws[slot * meshes..(slot + 1) * meshes] {
                    draw.instance_start = start;
                    draw.instance_count = actors.len() as u32;
                }
            }
            // Newly allocated unused slots also need zero instance counts.
            for draw in &mut batch.actor.scene.draws[poses.len() * meshes..] {
                draw.instance_count = 0;
            }
            self.rendered_instances += instances.len();
            batch
                .actor
                .scene
                .update_geometry(upload.queue(), &batch.poses);
            batch
                .actor
                .scene
                .update_instances(upload.device(), upload.queue(), &instances);
        }
        progress(total, total)
    }

    fn build_batch(
        &self,
        upload: &UploadContext,
        key: AppearanceKey,
        time: f32,
    ) -> anyhow::Result<Batch> {
        let model = self
            .library
            .load_race_with_appearance(key.0, key.1, &key.2)?;
        let (actor, poses) = upload_actor(upload, &self.library, &model, 2)?;
        let socket_indices = socket_indices(&model);
        Ok(Batch {
            model,
            actor,
            poses,
            capacity: 2,
            last_seen: time,
            socket_indices,
        })
    }

    pub fn draws(&self) -> Vec<&GpuActor> {
        self.batches
            .values()
            .filter(|batch| {
                batch
                    .actor
                    .scene
                    .draws
                    .iter()
                    .any(|draw| draw.instance_count > 0)
            })
            .map(|batch| &batch.actor)
            .chain(self.projectiles.draws())
            .collect()
    }

    pub fn bounds(&self) -> &BTreeMap<u32, ActorBounds> {
        &self.bounds
    }

    pub fn sockets(&self) -> &BTreeMap<u32, ActorSockets> {
        &self.sockets
    }
}

fn actor_instance(state: &ActorState, height: f32) -> Instance {
    let scale = if state.size > 0. {
        state.size / height
    } else {
        1.
    };
    // Authored characters face +X. Scene heading zero faces +Y; headings
    // increase clockwise. Server-to-scene conversion happens in LiveWorld.
    let rotation = Quat::from_rotation_z(
        std::f32::consts::FRAC_PI_2 - state.heading * std::f32::consts::TAU / 512.,
    );
    Instance::from_parts(state.position, rotation.to_array(), [scale; 3])
}

fn upload_actor(
    upload: &UploadContext,
    library: &CharacterLibrary,
    model: &CharacterModel,
    capacity: usize,
) -> anyhow::Result<(GpuActor, Vec<Geometry>)> {
    let names: BTreeSet<_> = model
        .materials
        .iter()
        .flat_map(|m| m.textures.iter().chain(m.normal_map.iter()))
        .collect();
    let textures = names
        .into_iter()
        .filter_map(|name| library.texture(name))
        .collect();
    let poses: Vec<_> = (0..capacity).flat_map(|_| model.meshes.clone()).collect();
    let scene = Scene::from_geometry(
        model.code.clone(),
        model.materials.clone(),
        poses.clone(),
        textures,
    );
    let gpu = GpuScene::build(upload.device(), upload.queue(), &scene)?;
    Ok((upload.prepare_actor(gpu), poses))
}

fn select_pose(model: &CharacterModel, state: &ActorState, time: f32, elapsed: f32) -> Pose {
    let auto = || {
        if state.moving {
            model.walk_animation()
        } else {
            model.idle_animation()
        }
    };
    let action = match state.action {
        ActorAction::Auto => 0,
        ActorAction::Stand => 32,
        ActorAction::Walk => 17,
        ActorAction::Run => 18,
        ActorAction::Swim => 37,
        ActorAction::Sit => 38,
        ActorAction::Duck => 24,
        ActorAction::Dead => 16,
        ActorAction::Attack => 5,
        ActorAction::Hit => 12,
        ActorAction::Cast => 43,
        ActorAction::Animation(action) => action,
    };
    let name = animation_code(action);
    let mut clip = name
        .filter(|name| model.animations.contains_key(*name))
        .unwrap_or_else(auto);
    let mut looping =
        matches!(action, 0 | 17 | 18 | 22 | 23 | 25 | 26 | 32 | 34 | 37 | 38 | 39..=41 | 71..=73);
    let mut sample_time = if looping { time } else { elapsed };
    if action == 38 && !model.animations.contains_key("P07") && model.animations.contains_key("P02")
    {
        // P02 is sit -> stand. Reverse it to sit, then hold its first frame.
        clip = "P02";
        looping = false;
        let animation = &model.animations[clip];
        sample_time = ((animation.frame_count - 1) as f32 * animation.frame_time_ms as f32 / 1000.
            - elapsed)
            .max(0.);
    } else if !looping
        && !matches!(action, 16 | 24)
        && elapsed >= model.animations[clip].duration_seconds()
    {
        clip = auto();
        looping = true;
        sample_time = time;
    }
    // Equal phases share one draw, while event start times remain actor-local.
    Pose {
        clip: clip.to_owned(),
        tick: (sample_time.max(0.) * 30.).round() as u32,
        looping,
    }
}

/// Original numeric animation ordering, also documented in EQEmu's combat
/// constants and Lantern's AnimationType table. Unknown ids fall back to idle.
fn animation_code(action: u8) -> Option<&'static str> {
    const CODES: [&str; 74] = [
        "", "C01", "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09", "C10", "C11", "D01",
        "D02", "D03", "D04", "D05", "L01", "L02", "L03", "L04", "L05", "L06", "L07", "L08", "L09",
        "O01", "S01", "S02", "S03", "S04", "S05", "P01", "P02", "P03", "P04", "P05", "P06", "P07",
        "T01", "T02", "T03", "T04", "T05", "T06", "T07", "T08", "T09", "S06", "S07", "S08", "S09",
        "S10", "S11", "S12", "S13", "S14", "S15", "S16", "S17", "S18", "S19", "S20", "S21", "S22",
        "S23", "S24", "S25", "S26", "S27", "S28", "P08", "O02", "O03",
    ];
    if action == 0 {
        None
    } else {
        CODES.get(action as usize).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transitions_cover_locomotion_repeated_actions_and_model_changes() {
        let key = (1, 0, CharacterAppearance::default());
        let mut timeline = Timeline {
            action: ActorAction::Auto,
            sequence: 0,
            started: 0.,
            changed: false,
            appearance: None,
            previous: None,
            transition: None,
            last_time: 0.,
        };
        let pose = |clip: &str, tick| Pose {
            clip: clip.into(),
            tick,
            looping: true,
        };
        let idle = pose("P01", 0);
        assert!(timeline.pose(key, idle.clone(), 0.).from.is_none());
        let walking = pose("L01", 30);
        let switch = timeline.pose(key, walking.clone(), 1.);
        assert_eq!(switch.from.as_ref().unwrap().clip, "P01");
        assert_eq!(switch.step, 0);
        assert_eq!(timeline.pose(key, pose("L01", 32), 1.0667).step, 2);
        assert!(timeline.pose(key, pose("L01", 36), 1.2).from.is_none());
        // Consecutive actions with the same clip still restart and crossfade.
        timeline.changed = true;
        assert_eq!(timeline.pose(key, walking, 1.3).from.unwrap().tick, 36);
        // Illusions/equipment have different topology: never reuse old poses.
        assert!(
            timeline
                .pose((522, 1, key.2), idle.clone(), 1.4)
                .from
                .is_none()
        );
        assert!(timeline.pose((522, 1, key.2), idle, 0.1).from.is_none());
    }

    #[test]
    fn original_animation_ids_include_combat_death_and_postures() {
        assert_eq!(animation_code(5), Some("C05"));
        assert_eq!(animation_code(12), Some("D01"));
        assert_eq!(animation_code(16), Some("D05"));
        assert_eq!(animation_code(38), Some("P07"));
        assert_eq!(animation_code(37), Some("P06"));
        assert_eq!(animation_code(43), Some("T05"));
        assert_eq!(animation_code(255), None);
    }

    #[test]
    #[ignore = "requires original humanoid character assets"]
    fn authored_toes_face_the_rendered_heading() {
        use glam::{Mat4, Vec3};
        let base = openeq_assets::loader::default_client_dir().expect("original assets");
        let library = CharacterLibrary::load(base, "poknowledge").unwrap();
        for code in ["HUM", "HUF", "GNM", "GNF", "BAM"] {
            let model = library.load_model(code).unwrap();
            let bones = model.bone_transforms("", 0., false).unwrap();
            // Toe minus boot origins gives a semantic forward vector from the
            // authored skeleton, independently of our heading formula.
            let point = |suffix: &str| {
                let name = format!("{code}{suffix}_TRACK");
                let index = model.bone_names.iter().position(|n| n == &name).unwrap();
                bones[index].transform_point3(Vec3::ZERO)
            };
            let mut forward = (point("TO_L") - point("BO_L") + point("TO_R") - point("BO_R")) / 2.;
            forward.z = 0.;
            forward = forward.normalize();
            eprintln!("{code} authored boot-to-toe forward: {forward:?}");
            assert!(
                forward.x > 0.99,
                "{code} authored forward changed: {forward:?}"
            );
            for (heading, expected) in [
                (0., Vec3::Y),
                (128., Vec3::X),
                (256., -Vec3::Y),
                (384., -Vec3::X),
            ] {
                let instance = actor_instance(
                    &ActorState {
                        heading,
                        size: 6.,
                        position: [13., -25., 7.],
                        ..Default::default()
                    },
                    model.bounds_max[2] - model.bounds_min[2],
                );
                let actual = Mat4::from_cols_array_2d(&instance.columns)
                    .transform_vector3(forward)
                    .normalize();
                assert!(
                    actual.dot(expected) > 0.99,
                    "{code} heading {heading}: facing {actual:?}, expected {expected:?}"
                );
            }
        }
    }

    /// Runs the actual Metal/Vulkan renderer and checks appearance caching,
    /// instance grouping, restarted actions, terminal death and actor removal.
    #[test]
    #[ignore = "requires original client assets and a GPU; writes /tmp/openeq-characters.png"]
    fn render_equipment_and_independent_actions() {
        use crate::{environment::EnvironmentSettings, scene::Camera};
        use openeq_assets::{mesh::Material, texture::Texture};
        let base = openeq_assets::loader::default_client_dir().expect("original client assets");
        let mut renderer = Renderer::new_headless(1600, 900).unwrap();
        renderer.set_environment(
            EnvironmentSettings {
                sky_enabled: false,
                ..Default::default()
            },
            None,
        );
        let ground = Scene::from_geometry(
            "character test stage".into(),
            vec![Material {
                textures: vec!["ground".into()],
                normal_map: None,
                water: None,
                flags: 0,
                anim_speed: 0,
                alpha_mask: false,
                transparent: false,
                emissive: false,
                clamp_uv: false,
            }],
            vec![Geometry {
                vertices: vec![
                    -60., -60., 0., 0., 0., 1., 0., 0., 60., -60., 0., 0., 0., 1., 1., 0., 60.,
                    60., 0., 0., 0., 1., 1., 1., -60., 60., 0., 0., 0., 1., 0., 1.,
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                material: 0,
                collidable: false,
            }],
            vec![Texture {
                name: "ground".into(),
                width: 1,
                height: 1,
                rgba: vec![130, 132, 140, 255],
            }],
        );
        let ground = GpuScene::build(renderer.device(), renderer.queue(), &ground).unwrap();
        let mut actors = ActorRenderer::load(&base, "poknowledge").unwrap();
        let mut blue = CharacterAppearance {
            face: 3,
            ..Default::default()
        };
        for slot in &mut blue.equipment[..7] {
            slot.material = 3;
            slot.color = 0xff6090ff;
        }
        blue.equipment[7].material = 1;
        blue.equipment[8].material = 201;
        let mut red = blue;
        for slot in &mut red.equipment[..7] {
            slot.color = 0xffff8060;
        }
        red.equipment[0].material = 0;
        red.equipment[8].material = 0;
        let mut states: Vec<_> = (0..6)
            .map(|i| ActorState {
                id: i + 1,
                race: 1,
                gender: 0,
                size: 6.,
                position: [(i as f32 - 2.5) * 5.5, 0., 3.],
                heading: 256.,
                appearance: blue,
                ..Default::default()
            })
            .collect();
        states[0].appearance = CharacterAppearance::default();
        states[1].action = ActorAction::Stand;
        states[2].action = ActorAction::Attack;
        states[3].action = ActorAction::Sit;
        states[4].action = ActorAction::Dead;
        states[5].appearance = red;
        states[5].action = ActorAction::Cast;
        actors.update(&renderer, &states, 0.);
        actors.update(&renderer, &states, 0.35);
        // Repeated attack starts independently while the others keep playing.
        states[2].action_sequence += 1;
        actors.update(&renderer, &states, 0.75);
        assert_eq!(actors.timelines[&3].started, 0.75);
        assert_eq!(actors.timelines[&5].started, 0.);
        actors.update(&renderer, &states, 1.05);
        assert_eq!(actors.rendered_instances, 6);
        let key = (1, 0, blue);
        assert_eq!(
            actors.batches[&key].capacity, 4,
            "four independent blue poses should reuse four slots"
        );
        assert_eq!(
            actors.batches.len(),
            3,
            "appearances share a texture/mesh cache"
        );
        let camera = Camera {
            position: [0., -27., 12.],
            yaw: 0.,
            pitch: -18f32.to_radians(),
            fov_y: 55f32.to_radians(),
        };
        renderer.render_with_actors(&ground, &camera, &actors.draws());
        let (width, height, pixels) = renderer.read_rgba().unwrap();
        let pink = pixels
            .chunks_exact(4)
            .filter(|p| p[0] > 230 && p[1] < 25 && p[2] > 230)
            .count();
        assert_eq!(pink, 0, "missing texture or unapplied palette transparency");
        let output = std::env::var("OPENEQ_CHARACTER_RENDER_OUT")
            .unwrap_or_else(|_| "/tmp/openeq-characters.png".into());
        image::RgbaImage::from_raw(width, height, pixels)
            .unwrap()
            .save(&output)
            .unwrap();
        eprintln!("rendered equipped actors to {output}");
        actors.update(&renderer, &[], 33.);
        assert!(actors.draws().is_empty());
        assert!(actors.timelines.is_empty());
        assert!(actors.batches.is_empty());
        assert_eq!(actors.rendered_instances, 0);
    }
}

#[cfg(test)]
mod modern_gpu_tests {
    use super::*;
    #[test]
    #[ignore = "requires original EQG character assets and GPU"]
    fn weighted_eqg_characters_render() {
        use crate::{environment::EnvironmentSettings, scene::Camera};
        let base = openeq_assets::loader::default_client_dir().expect("original assets");
        let mut renderer = Renderer::new_headless(1400, 800).unwrap();
        renderer.set_environment(
            EnvironmentSettings {
                sky_enabled: false,
                ..Default::default()
            },
            None,
        );
        // Reuse an original elevator platform as the stage.
        let objects = openeq_assets::loader::load_object_library(&base, "poknowledge").unwrap();
        let stage = objects.object_model("POKELEVATOR500").unwrap();
        let stage = GpuScene::build(renderer.device(), renderer.queue(), &stage).unwrap();
        let mut actors = ActorRenderer::load(&base, "poknowledge").unwrap();
        let states: Vec<_> = [(464, 2), (522, 0), (522, 1)]
            .into_iter()
            .enumerate()
            .map(|(i, (race, gender))| ActorState {
                id: i as u32,
                race,
                gender,
                size: 6.,
                position: [(i as f32 - 1.) * 7., 0., 4.],
                heading: 256.,
                ..Default::default()
            })
            .collect();
        actors.update(&renderer, &states, 0.);
        actors.update(&renderer, &states, 0.4);
        assert_eq!(actors.rendered_instances, 3);
        let camera = Camera {
            position: [0., -24., 11.],
            yaw: 0.,
            pitch: -16f32.to_radians(),
            fov_y: 55f32.to_radians(),
        };
        renderer.render_with_actors(&stage, &camera, &actors.draws());
        let (w, h, pixels) = renderer.read_rgba().unwrap();
        assert!(
            !pixels
                .chunks_exact(4)
                .any(|p| p[0] > 230 && p[1] < 25 && p[2] > 230),
            "modern characters contain missing-texture magenta"
        );
        image::RgbaImage::from_raw(w, h, pixels)
            .unwrap()
            .save("/tmp/openeq-modern-characters.png")
            .unwrap();
    }
}
