//! Server-placed doors, lifts and props. Model buffers are shared by name;
//! opening updates only an instance transform around the authored hinge.
use crate::{GpuActor, GpuScene, Renderer, scene::Instance};
use glam::{Quat, Vec3};
use openeq_assets::{Scene, collision::CollisionWorld, loader, mesh::Geometry};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DoorState {
    pub id: u8,
    pub name: String,
    pub position: [f32; 3],
    pub heading: f32,
    pub incline: u32,
    pub size: u32,
    pub open_type: u8,
    /// Effective open state, after applying inverted OP_MoveDoor semantics.
    pub state: u8,
    pub inverted: bool,
    pub parameter: u32,
}
struct Batch {
    actor: GpuActor,
    extent: Vec3,
    collision: Vec<Geometry>,
}
struct Motion {
    open: bool,
    from: f32,
    started: f32,
}
impl Motion {
    fn progress(&self, time: f32, duration: f32) -> f32 {
        let t = ((time - self.started) / duration.max(0.001)).clamp(0., 1.);
        let t = t * t * (3. - 2. * t);
        self.from + (f32::from(self.open) - self.from) * t
    }
}
pub struct DoorRenderer {
    library: Scene,
    batches: BTreeMap<String, Batch>,
    missing: BTreeSet<String>,
    motion: BTreeMap<u8, Motion>,
    pub rendered_instances: usize,
    collision: CollisionWorld,
    collision_states: Vec<DoorState>,
}
impl DoorRenderer {
    pub fn load(base: &Path, zone: &str) -> anyhow::Result<Self> {
        Ok(Self {
            library: loader::load_object_library(base, zone)?,
            batches: BTreeMap::new(),
            missing: BTreeSet::new(),
            motion: BTreeMap::new(),
            rendered_instances: 0,
            collision: CollisionWorld::default(),
            collision_states: Vec::new(),
        })
    }
    pub fn update(&mut self, renderer: &Renderer, states: &[DoorState], time: f32) {
        let time = if time.is_finite() { time.max(0.) } else { 0. };
        let mut groups: BTreeMap<String, Vec<&DoorState>> = BTreeMap::new();
        let ids: BTreeSet<_> = states.iter().map(|state| state.id).collect();
        self.motion.retain(|id, _| ids.contains(id));
        for state in states {
            if matches!(state.open_type, 50 | 53 | 54) {
                continue;
            }
            groups
                .entry(state.name.to_ascii_lowercase())
                .or_default()
                .push(state);
        }
        for name in groups.keys() {
            if self.batches.contains_key(name) || self.missing.contains(name) {
                continue;
            }
            let model = self.library.object_model(name).and_then(|model| {
                let collision = model
                    .meshes
                    .iter()
                    .filter(|mesh| {
                        mesh.collidable && model.materials[mesh.material].water.is_none()
                    })
                    .cloned()
                    .collect();
                GpuScene::build(renderer.device(), renderer.queue(), &model)
                    .map(|scene| (scene, collision))
                    .map_err(|error| openeq_assets::Error::Format(error.to_string()))
            });
            match model {
                Ok((scene, collision)) => {
                    let extent = scene.bounds_max - scene.bounds_min;
                    tracing::info!(model=%name,"dynamic door model uploaded");
                    self.batches.insert(
                        name.clone(),
                        Batch {
                            actor: renderer.prepare_actor(scene),
                            extent,
                            collision,
                        },
                    );
                }
                Err(error) => {
                    tracing::warn!(model=%name,%error,"dynamic door model unavailable");
                    self.missing.insert(name.clone());
                }
            }
        }
        self.rendered_instances = 0;
        for (name, batch) in &mut self.batches {
            let mut instances = Vec::new();
            for state in groups.get(name).into_iter().flatten() {
                let duration = duration(state);
                let open = state.state != 0;
                let motion = self.motion.entry(state.id).or_insert(Motion {
                    open,
                    from: f32::from(open),
                    started: time,
                });
                if motion.open != open || motion.started > time {
                    *motion = Motion {
                        open,
                        from: motion.progress(time, duration),
                        started: time,
                    };
                }
                let progress = motion.progress(time, duration);
                instances.push(transform(state, progress, batch.extent, time));
            }
            for draw in &mut batch.actor.scene.draws {
                draw.instance_start = 0;
                draw.instance_count = instances.len() as u32;
            }
            if !instances.is_empty() {
                batch
                    .actor
                    .scene
                    .update_instances(renderer.device(), renderer.queue(), &instances);
            }
            self.rendered_instances += instances.len();
        }
        if states != self.collision_states {
            let mut collision = CollisionWorld::default();
            for state in states {
                if matches!(state.open_type, 50 | 53 | 54) {
                    continue;
                }
                let Some(batch) = self.batches.get(&state.name.to_ascii_lowercase()) else {
                    continue;
                };
                let instance = transform(state, f32::from(state.state != 0), batch.extent, 0.);
                let matrix = glam::Mat4::from_cols_array_2d(&instance.columns);
                for mesh in &batch.collision {
                    collision.add_geometry(mesh, matrix);
                }
            }
            self.collision = collision;
            self.collision_states = states.to_vec();
        }
    }
    /// Closed/open final-pose collision, rebuilt only when server door state
    /// changes. Interpolated visuals do not rebuild the static zone or grid.
    pub fn collision_world(&self) -> &CollisionWorld {
        &self.collision
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
            .collect()
    }
}
fn duration(state: &DoorState) -> f32 {
    match state.open_type {
        59 | 60 => (state.parameter as i32 as f32).abs().max(20.) / 25.,
        45 => 2.,
        _ => 0.75,
    }
}
fn transform(state: &DoorState, progress: f32, extent: Vec3, time: f32) -> Instance {
    let size = if state.size == 0 {
        1.
    } else {
        state.size as f32 / 100.
    };
    let mut turn = 0.;
    let mut tilt = 0.;
    let mut offset = Vec3::ZERO;
    // Motion classes follow EQEmu's documented door-open-type table. Exact
    // old-client timing/travel constants are unavailable; slides use the
    // object's footprint, and lifts use the authoritative door parameter.
    match state.open_type {
        0..=4 | 8 => turn = -std::f32::consts::FRAC_PI_2 * progress,
        5..=7 | 30 | 35 | 36 | 40 => turn = std::f32::consts::FRAC_PI_2 * progress,
        10..=12 | 15..=17 | 20..=22 | 25..=27 => {
            let factor = (state.open_type / 5 - 1) as f32;
            offset.x = extent.x.max(extent.y).max(1.) * factor * progress * size;
        }
        45 => offset.y = extent.x.max(extent.y).max(1.) * progress * size,
        59 | 60 => offset.z = state.parameter as i32 as f32 * progress,
        100..=102 if state.state != 0 || state.inverted => {
            turn = time * std::f32::consts::TAU / ((103 - state.open_type) as f32 * 4.)
        }
        105..=107 if state.state != 0 || state.inverted => {
            tilt = time * std::f32::consts::TAU / ((108 - state.open_type) as f32 * 4.)
        }
        _ => {}
    }
    let base = Quat::from_rotation_z(-state.heading * std::f32::consts::TAU / 512.);
    // Incline is signed despite its unsigned wire representation.
    let rotation = base
        * Quat::from_rotation_z(turn)
        * Quat::from_rotation_y(state.incline as i32 as f32 * std::f32::consts::TAU / 512. + tilt);
    let position = Vec3::from_array(state.position) + base * offset;
    Instance::from_parts(position.to_array(), rotation.to_array(), [size; 3])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn door_hinges_lifts_and_reversal_preserve_the_authored_origin() {
        let state = DoorState {
            position: [10., 20., 30.],
            heading: 128.,
            size: 100,
            open_type: 5,
            ..Default::default()
        };
        let closed =
            glam::Mat4::from_cols_array_2d(&transform(&state, 0., Vec3::splat(10.), 0.).columns);
        let open =
            glam::Mat4::from_cols_array_2d(&transform(&state, 1., Vec3::splat(10.), 0.).columns);
        assert!((closed.transform_point3(Vec3::ZERO) - Vec3::new(10., 20., 30.)).length() < 1e-5);
        assert!(
            (open.transform_point3(Vec3::ZERO) - closed.transform_point3(Vec3::ZERO)).length()
                < 1e-5
        );
        assert!(
            (open.transform_vector3(Vec3::Y) - closed.transform_vector3(Vec3::Y)).length() > 1.
        );
        let lift = DoorState {
            open_type: 59,
            parameter: 70,
            ..state
        };
        let raised =
            glam::Mat4::from_cols_array_2d(&transform(&lift, 1., Vec3::splat(10.), 0.).columns);
        assert_eq!(raised.transform_point3(Vec3::ZERO).z, 100.);
        let motion = Motion {
            open: true,
            from: 0.,
            started: 0.,
        };
        let halfway = motion.progress(0.375, 0.75);
        let reversed = Motion {
            open: false,
            from: halfway,
            started: 0.375,
        };
        assert_eq!(reversed.progress(0.375, 0.75), halfway);
        assert_eq!(reversed.progress(2., 0.75), 0.);
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;
    #[test]
    #[ignore = "requires original assets and GPU; writes /tmp/openeq-doors.png"]
    fn original_doors_render_open_and_block_only_at_current_server_pose() {
        use crate::{environment::EnvironmentSettings, scene::Camera};
        use openeq_assets::{mesh::Material, texture::Texture};
        let base = loader::default_client_dir().expect("original client assets");
        let mut renderer = Renderer::new_headless(1280, 720).unwrap();
        renderer.set_environment(
            EnvironmentSettings {
                sky_enabled: false,
                ..Default::default()
            },
            None,
        );
        let stage = Scene::from_geometry(
            "door test stage".into(),
            vec![Material {
                textures: vec!["ground".into()],
                normal_map: None,
                water: None,
                flags: 0,
                anim_speed: 0,
                alpha_mask: false,
                transparent: false,
                emissive: false,
            }],
            vec![Geometry {
                vertices: vec![
                    -100., -100., -0.1, 0., 0., 1., 0., 0., 100., -100., -0.1, 0., 0., 1., 1., 0.,
                    100., 100., -0.1, 0., 0., 1., 1., 1., -100., 100., -0.1, 0., 0., 1., 0., 1.,
                ],
                indices: vec![0, 1, 2, 0, 2, 3],
                material: 0,
                collidable: true,
            }],
            vec![Texture {
                name: "ground".into(),
                width: 1,
                height: 1,
                rgba: vec![130, 132, 140, 255],
            }],
        );
        let stage = GpuScene::build(renderer.device(), renderer.queue(), &stage).unwrap();
        let mut doors = DoorRenderer::load(&base, "poknowledge").unwrap();
        let mut states = vec![DoorState {
            id: 1,
            name: "POKDOOR500".into(),
            size: 100,
            open_type: 5,
            ..Default::default()
        }];
        doors.update(&renderer, &states, 0.);
        assert!(doors.collision_world().triangle_count() > 0);
        let closed = doors
            .collision_world()
            .move_player([-5., -6., 0.], [10., 0., 0.], 1., 6., 0.);
        assert!(closed[0] < 0., "closed door allowed passage: {closed:?}");
        states[0].state = 1;
        doors.update(&renderer, &states, 1.);
        let open = doors
            .collision_world()
            .move_player([-5., -6., 0.], [10., 0., 0.], 1., 6., 0.);
        assert!(open[0] > 4., "open doorway stayed blocked: {open:?}");
        states = vec![
            DoorState {
                id: 1,
                name: "POKDOOR500".into(),
                position: [-10., 0., 0.],
                heading: 128.,
                size: 100,
                open_type: 5,
                ..Default::default()
            },
            DoorState {
                id: 2,
                name: "POKDOOR500".into(),
                position: [20., 0., 0.],
                heading: 128.,
                size: 100,
                open_type: 5,
                ..Default::default()
            },
        ];
        doors.update(&renderer, &states, 2.);
        states[1].state = 1;
        doors.update(&renderer, &states, 3.);
        doors.update(&renderer, &states, 4.);
        assert_eq!(doors.rendered_instances, 2);
        assert_eq!(
            doors.batches.len(),
            1,
            "two door instances must share their uploaded model"
        );
        let camera = Camera {
            position: [10., -62., 28.],
            yaw: 0.,
            pitch: -17f32.to_radians(),
            fov_y: 55f32.to_radians(),
        };
        renderer.render_with_actors(&stage, &camera, &doors.draws());
        let (w, h, pixels) = renderer.read_rgba().unwrap();
        assert_eq!(
            pixels
                .chunks_exact(4)
                .filter(|p| p[0] > 230 && p[1] < 25 && p[2] > 230)
                .count(),
            0
        );
        image::RgbaImage::from_raw(w, h, pixels)
            .unwrap()
            .save("/tmp/openeq-doors.png")
            .unwrap();
        doors.update(&renderer, &[], 5.);
        assert!(doors.draws().is_empty());
        assert_eq!(doors.collision_world().triangle_count(), 0);
    }
}
