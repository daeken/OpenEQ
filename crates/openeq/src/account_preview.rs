//! Read-only character-select previews. The model comes from the received
//! roster and local assets; this module never creates a connection or command.
use crate::{account::Token, loading::Job};
use openeq_assets::{
    Scene,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_net::world::Character;
use openeq_render::{
    Renderer,
    actors::{
        ActorAction, ActorBounds, ActorRenderer, ActorState, CharacterAppearance,
        CharacterModelSet, EquipmentAppearance,
    },
    environment::EnvironmentSettings,
    scene::{Camera, GpuScene},
    upload::UploadContext,
};
use openeq_ui::Rect;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub token: Token,
    pub character: Character,
    pub dir: PathBuf,
    pub model_set: CharacterModelSet,
}

struct InFlight<T> {
    request: Request,
    cancelled: Arc<AtomicBool>,
    job: Job<T>,
}

/// Keep the cancelled job until it exits. Dropping a Job is nonblocking, so
/// immediately replacing it would otherwise permit unbounded parallel loads.
struct Cache<T> {
    desired: Option<Request>,
    job: Option<InFlight<T>>,
    ready: Option<T>,
    failed: bool,
}
impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            desired: None,
            job: None,
            ready: None,
            failed: false,
        }
    }
}
impl<T: Send + 'static> Cache<T> {
    fn update(
        &mut self,
        desired: Option<Request>,
        start: impl FnOnce(Request, Arc<AtomicBool>) -> Job<T>,
    ) {
        if self.desired != desired {
            self.desired = desired;
            self.ready = None;
            self.failed = false;
        }
        if let Some(inflight) = &self.job
            && Some(&inflight.request) != self.desired.as_ref()
        {
            inflight.cancelled.store(true, Ordering::Relaxed);
        }
        if let Some(result) = self.job.as_mut().and_then(|inflight| inflight.job.poll()) {
            let inflight = self.job.take().unwrap();
            if Some(&inflight.request) == self.desired.as_ref()
                && !inflight.cancelled.load(Ordering::Relaxed)
            {
                match result {
                    Ok(prepared) => self.ready = Some(prepared),
                    Err(error) => {
                        tracing::warn!(%error, "character preview unavailable");
                        self.failed = true;
                    }
                }
            }
        }
        if self.job.is_none()
            && self.ready.is_none()
            && !self.failed
            && let Some(request) = self.desired.clone()
        {
            let cancelled = Arc::new(AtomicBool::new(false));
            let job = start(request.clone(), cancelled.clone());
            self.job = Some(InFlight {
                request,
                cancelled,
                job,
            });
        }
    }
}
impl<T> Drop for Cache<T> {
    fn drop(&mut self) {
        if let Some(inflight) = &self.job {
            inflight.cancelled.store(true, Ordering::Relaxed);
        }
    }
}

#[derive(Default)]
pub struct Preview {
    cache: Cache<Prepared>,
}
impl Preview {
    pub fn update(&mut self, request: Option<Request>, renderer: &Renderer) {
        self.cache.update(request, |request, cancelled| {
            let upload = renderer.upload_context();
            Job::start(move |report| {
                let check = |detail| -> anyhow::Result<()> {
                    anyhow::ensure!(!cancelled.load(Ordering::Relaxed), "Preview cancelled");
                    report.stage(detail, None)
                };
                check("Loading character models")?;
                let mut actors =
                    ActorRenderer::load_with_model_set(&request.dir, "", request.model_set)?;
                check("Preparing appearance")?;
                let state = actor_state(&request.character);
                anyhow::ensure!(
                    actors.preload_with_progress(
                        &upload,
                        std::slice::from_ref(&state),
                        0.,
                        |_, _| { check("Preparing appearance").is_ok() }
                    ),
                    "Preview cancelled"
                );
                anyhow::ensure!(
                    actors.rendered_instances == 1,
                    "Character model is unavailable"
                );
                let bounds = *actors
                    .bounds()
                    .get(&state.id)
                    .ok_or_else(|| anyhow::anyhow!("Character has no visible geometry"))?;
                let framing = Framing::new(bounds)?;
                check("Preparing preview stage")?;
                let scene = build_stage(&upload, framing)?;
                check("Appearance ready")?;
                Ok(Prepared {
                    actors,
                    state,
                    scene,
                    framing,
                    installed: false,
                })
            })
        });
    }

    /// None means the matching appearance is ready. Errors never disable Play.
    pub fn status(&self) -> Option<&'static str> {
        if self.cache.failed {
            Some(
                "Appearance preview is unavailable. Return to the character list to enter the world.",
            )
        } else if self.cache.ready.is_none() {
            Some("Loading appearance…")
        } else {
            None
        }
    }

    /// The UI must already be installed. Returns false while loading/failed.
    pub fn render(&mut self, renderer: &mut Renderer, viewport: [u32; 2], heading: f32) -> bool {
        let Some(prepared) = &mut self.cache.ready else {
            return false;
        };
        if !prepared.installed {
            renderer.set_scene(&prepared.scene);
            renderer.clear_particles();
            renderer.set_view_liquid(None);
            prepared.installed = true;
        }
        renderer.set_environment(
            EnvironmentSettings {
                sky_enabled: false,
                ..Default::default()
            },
            None,
        );
        prepared.state.heading = if heading.is_finite() {
            heading.rem_euclid(512.)
        } else {
            256.
        };
        // A fixed standing pose keeps equipment inspection and framing stable;
        // rotation changes instances without rebuilding model/texture assets.
        prepared
            .actors
            .update(renderer, std::slice::from_ref(&prepared.state), 0.);
        renderer.render_with_actors(
            &prepared.scene,
            &prepared.framing.camera(viewport),
            &prepared.actors.draws(),
        );
        true
    }
}

struct Prepared {
    actors: ActorRenderer,
    state: ActorState,
    scene: GpuScene,
    framing: Framing,
    installed: bool,
}

fn actor_state(character: &Character) -> ActorState {
    let source = character.appearance;
    let mut equipment = source.equipment.map(|piece| EquipmentAppearance {
        material: piece.material,
        elite_material: piece.elite_material,
        hero_forge_model: piece.hero_forge_model,
        color: piece.color,
    });
    // EQEmu worlddb.cpp writes these explicit IDFile values and slots 7/8
    // from the same item/ornament model. Keep the dedicated roster fields.
    equipment[7].material = source.primary_model;
    equipment[8].material = source.secondary_model;
    ActorState {
        id: 1,
        race: character.race,
        gender: character.gender,
        size: 6., // A normalized inspection scale, not the character's live size.
        heading: 256.,
        action: ActorAction::Stand,
        appearance: CharacterAppearance {
            face: source.face,
            equipment,
            hair_color: source.hair_color,
            beard_color: source.beard_color,
            eye_color_1: source.eye_color_1,
            eye_color_2: source.eye_color_2,
            hair_style: source.hair_style,
            beard: source.beard,
            drakkin_heritage: source.drakkin_heritage,
            drakkin_tattoo: source.drakkin_tattoo,
            drakkin_details: source.drakkin_details,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Same reserved header/footer as the account preview overlay, in logical pixels.
pub fn model_rect(viewport: [u32; 2]) -> Rect {
    let compact = viewport[1] < 360;
    let top = if compact { 56. } else { 82. };
    let bottom = if compact { 78. } else { 100. };
    Rect::new(
        12.,
        top,
        (viewport[0] as f32 - 24.).max(1.),
        (viewport[1] as f32 - top - bottom).max(1.),
    )
}

#[derive(Clone, Copy)]
struct Framing {
    radius: f32,
    bottom: f32,
    top: f32,
}
impl Framing {
    fn new(bounds: ActorBounds) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bounds.min.iter().chain(&bounds.max).all(|v| v.is_finite())
                && (0..3).all(|axis| bounds.max[axis] >= bounds.min[axis])
                && bounds.max[2] > bounds.min[2],
            "Invalid character bounds"
        );
        let radius = bounds
            .corners()
            .iter()
            .map(|p| p[0].hypot(p[1]))
            .fold(0_f32, f32::max)
            .max(0.1);
        anyhow::ensure!(
            radius.is_finite() && (bounds.max[2] - bounds.min[2]).is_finite(),
            "Invalid character bounds"
        );
        Ok(Self {
            radius,
            bottom: bounds.min[2],
            top: bounds.max[2],
        })
    }

    fn camera(self, viewport: [u32; 2]) -> Camera {
        let width = viewport[0].max(1) as f32;
        let height = viewport[1].max(1) as f32;
        let area = model_rect(viewport);
        let fov_y = 40_f32.to_radians();
        let tangent = (fov_y * 0.5).tan();
        let aspect = width / height;
        let center_y = 1. - 2. * (area.y + area.height * 0.5) / height;
        let half_height = (self.top - self.bottom) * 0.5;
        // Radial horizontal bounds contain all rotations, including held gear.
        // Include depth in the fit and leave an inspection margin at all edges.
        let distance = (self.radius / (tangent * aspect * area.width / width).max(0.001))
            .max(half_height / (tangent * area.height / height).max(0.001))
            * 1.20
            + self.radius * (1. + center_y.abs());
        Camera {
            position: [
                0.,
                -distance,
                (self.top + self.bottom) * 0.5 - center_y * distance * tangent,
            ],
            yaw: 0.,
            pitch: 0.,
            fov_y,
        }
    }
}

fn build_stage(upload: &UploadContext, framing: Framing) -> anyhow::Result<GpuScene> {
    let radius = framing.radius * 1.25;
    let z = framing.bottom - 0.04;
    let mut vertices = vec![0., 0., z, 0., 0., 1., 0.5, 0.5];
    for step in 0..48 {
        let angle = step as f32 * std::f32::consts::TAU / 48.;
        vertices.extend_from_slice(&[
            radius * angle.cos(),
            radius * angle.sin(),
            z,
            0.,
            0.,
            1.,
            0.5,
            0.5,
        ]);
    }
    let indices = (0..48)
        .flat_map(|step| [0, step + 1, (step + 1) % 48 + 1])
        .collect();
    let scene = Scene::from_geometry(
        "character appearance stage".into(),
        vec![Material {
            textures: vec!["preview_stage".into()],
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
            vertices,
            indices,
            material: 0,
            collidable: false,
        }],
        vec![Texture {
            name: "preview_stage".into(),
            width: 1,
            height: 1,
            rgba: vec![61, 66, 77, 255],
        }],
    );
    GpuScene::build(upload.device(), upload.queue(), &scene)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    fn request() -> Request {
        Request {
            token: Token {
                attempt: 4,
                revision: 2,
            },
            dir: PathBuf::from("unused"),
            model_set: CharacterModelSet::Classic,
            character: Character {
                name: "Preview".into(),
                race: 1,
                gender: 0,
                class: 1,
                level: 10,
                zone: 202,
                instance_id: 0,
                enabled: true,
                appearance: Default::default(),
            },
        }
    }

    #[test]
    fn roster_appearance_and_explicit_held_models_reach_actor_state() {
        let mut character = request().character;
        character.race = 522;
        character.gender = 1;
        let appearance = &mut character.appearance;
        appearance.face = 4;
        appearance.hair_style = 5;
        appearance.hair_color = 6;
        appearance.beard = 7;
        appearance.beard_color = 8;
        appearance.eye_color_1 = 9;
        appearance.eye_color_2 = 10;
        appearance.drakkin_heritage = 2;
        appearance.drakkin_tattoo = 3;
        appearance.drakkin_details = 4;
        for (slot, piece) in appearance.equipment.iter_mut().enumerate() {
            piece.material = slot as u32 + 1;
            piece.elite_material = 100;
            piece.hero_forge_model = 200;
            piece.color = 0xff12_3456;
            piece.unknown1 = u32::MAX;
            piece.material2 = u32::MAX;
        }
        appearance.primary_model = 201;
        appearance.secondary_model = 0; // Do not invent a shield from another field.
        let actor = actor_state(&character);
        assert_eq!((actor.race, actor.gender), (522, 1));
        assert_eq!(actor.appearance.face, 4);
        assert_eq!(
            [
                actor.appearance.hair_style,
                actor.appearance.hair_color,
                actor.appearance.beard,
                actor.appearance.beard_color,
                actor.appearance.eye_color_1,
                actor.appearance.eye_color_2
            ],
            [5, 6, 7, 8, 9, 10]
        );
        assert_eq!(
            [
                actor.appearance.drakkin_heritage,
                actor.appearance.drakkin_tattoo,
                actor.appearance.drakkin_details
            ],
            [2, 3, 4]
        );
        assert_eq!(
            actor.appearance.equipment[0],
            EquipmentAppearance {
                material: 1,
                elite_material: 100,
                hero_forge_model: 200,
                color: 0xff12_3456
            }
        );
        assert_eq!(actor.appearance.equipment[7].material, 201);
        assert_eq!(actor.appearance.equipment[8].material, 0);
        assert_eq!(actor.action, ActorAction::Stand);
    }

    #[test]
    fn every_rotation_fits_between_preview_controls_at_small_and_wide_sizes() {
        let framing = Framing::new(ActorBounds {
            min: [-2., -1., -0.3],
            max: [3., 2., 7.],
        })
        .unwrap();
        for viewport in [
            [800, 600],
            [1600, 1200],
            [320, 240],
            [180, 640],
            [1800, 360],
        ] {
            let camera = framing.camera(viewport);
            let matrix =
                camera.view_projection(viewport[0] as f32 / viewport[1] as f32, 0.1, 10000.);
            let area = model_rect(viewport);
            for step in 0..64 {
                let angle = step as f32 * std::f32::consts::TAU / 64.;
                for z in [framing.bottom, framing.top] {
                    let world = Camera::to_world([
                        framing.radius * angle.cos(),
                        framing.radius * angle.sin(),
                        z,
                    ]);
                    let projected = matrix.project_point3(world);
                    let point = [
                        (projected.x + 1.) * 0.5 * viewport[0] as f32,
                        (1. - projected.y) * 0.5 * viewport[1] as f32,
                    ];
                    assert!(area.contains(point), "{viewport:?} {point:?} {area:?}");
                    assert!((0.0..=1.0).contains(&projected.z));
                }
            }
        }
        assert!(
            Framing::new(ActorBounds {
                min: [f32::NAN; 3],
                max: [1.; 3]
            })
            .is_err()
        );
        assert!(
            Framing::new(ActorBounds {
                min: [0.; 3],
                max: [0.; 3]
            })
            .is_err()
        );
    }

    #[test]
    fn cancelled_work_is_bounded_and_cannot_restore_even_the_same_request() {
        let original = request();
        let (release, wait) = mpsc::channel();
        let (started, running) = mpsc::channel();
        let mut cache = Cache::<u32>::default();
        cache.update(Some(original.clone()), |_, cancel| {
            Job::start(move |_| {
                started.send(cancel).unwrap();
                wait.recv().unwrap();
                Ok(1) // Deliberately ignore cancellation: publication must still reject it.
            })
        });
        let cancelled = running.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut changed = original.clone();
        changed.token.revision += 1;
        for _ in 0..100 {
            cache.update(Some(changed.clone()), |_, _| {
                panic!("started parallel asset load")
            });
        }
        assert!(cancelled.load(Ordering::Relaxed));
        cache.update(None, |_, _| panic!("loaded without a selection"));
        cache.update(Some(original.clone()), |_, _| {
            panic!("reused a cancelled in-flight load")
        });
        release.send(()).unwrap();
        let mut starts = 0;
        let deadline = Instant::now() + Duration::from_secs(5);
        while cache.ready.is_none() {
            cache.update(Some(original.clone()), |_, _| {
                starts += 1;
                Job::start(|_| Ok(2))
            });
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(starts, 1);
        assert_eq!(cache.ready, Some(2));
        changed = original.clone();
        changed.character.appearance.face += 1;
        cache.update(Some(changed), |_, _| Job::start(|_| Ok(3)));
        assert!(
            cache.ready.is_none(),
            "old appearance survived changed roster data"
        );
    }

    #[test]
    fn failed_preview_is_terminal_until_selection_changes() {
        let request = request();
        let mut cache = Cache::<()>::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut starts = 0;
        while !cache.failed {
            cache.update(Some(request.clone()), |_, _| {
                starts += 1;
                Job::start(|_| anyhow::bail!("fixture asset unavailable"))
            });
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert_eq!(starts, 1);
        cache.update(Some(request.clone()), |_, _| {
            panic!("automatically retried failed preview")
        });
        cache.update(None, |_, _| panic!("loaded without a selection"));
        assert!(!cache.failed);
        cache.update(Some(request), |_, _| Job::start(|_| Ok(())));
        assert!(cache.job.is_some());
    }
}
