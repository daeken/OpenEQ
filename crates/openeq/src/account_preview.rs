//! Read-only character previews from the roster or a local creation draft.
//! This module never creates a connection or command.
use crate::{
    account::Token,
    account_creation::{AppearancePolicy, Context, PreviewFamily, PreviewReceipt},
    loading::Job,
};
use openeq_assets::{
    Scene,
    character::customization::CustomizationCatalog,
    mesh::{Geometry, Material},
    texture::Texture,
};
use openeq_net::{creation::Appearance, world::Character};
use openeq_render::{
    Renderer,
    actors::{
        ActorAction, ActorBounds, ActorRenderer, ActorState, CharacterAppearance,
        CharacterModelFamily, CharacterModelSet, EquipmentAppearance,
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
    /// Creation authority is part of the cache key; roster previews use None.
    pub creation: Option<Context>,
    pub character: Character,
    pub dir: PathBuf,
    pub model_set: CharacterModelSet,
}

/// Available only for a matching loaded creation preview. Provisional invalid
/// choices receive the resolved family's policy, but no submission receipt.
#[derive(Debug, Clone)]
pub struct CreationPreview {
    pub policy: AppearancePolicy,
    pub receipt: Option<PreviewReceipt>,
    pub heritages: Vec<u32>,
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

impl<T> Cache<T> {
    fn matching(&self, expected: &Request) -> Option<&T> {
        (self.desired.as_ref() == Some(expected))
            .then_some(self.ready.as_ref())
            .flatten()
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

    /// No receipt can cross a changed draft, account, world socket, roster,
    /// catalog, asset directory, or model preference. A caller must request a
    /// fresh preview after adopting defaults from a newly resolved family.
    pub fn creation_preview(&self, expected: &Request) -> Result<CreationPreview, String> {
        let prepared = self.cache.matching(expected).ok_or_else(|| {
            if self.cache.desired.as_ref() == Some(expected) && self.cache.failed {
                "Appearance preview is unavailable.".to_owned()
            } else {
                "Loading the current appearance…".to_owned()
            }
        })?;
        let family = prepared
            .actors
            .model_family(prepared.state.id)
            .ok_or_else(|| "Character model is unavailable.".to_owned())?;
        creation_preview(
            expected,
            family,
            prepared.actors.diffuse_textures_loaded(prepared.state.id),
            prepared.actors.appearance_resolved(prepared.state.id),
            prepared.actors.customization(),
        )
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

fn creation_preview(
    request: &Request,
    loaded: CharacterModelFamily,
    diffuse_textures_loaded: bool,
    appearance_resolved: bool,
    metadata: &CustomizationCatalog,
) -> Result<CreationPreview, String> {
    let context = request
        .creation
        .ok_or_else(|| "Select a creation draft to preview.".to_owned())?;
    let family = match loaded {
        CharacterModelFamily::Classic => PreviewFamily::Classic,
        CharacterModelFamily::Luclin => PreviewFamily::Luclin,
        CharacterModelFamily::Drakkin => PreviewFamily::Drakkin,
        CharacterModelFamily::Modern => {
            return Err("This model has no supported creation appearance.".to_owned());
        }
    };
    let character = &request.character;
    let class = u32::from(character.class);
    let policy_for = |heritage| {
        AppearancePolicy::for_preview(
            character.race,
            class,
            character.gender,
            family,
            metadata,
            heritage,
        )
    };
    let heritages = if family == PreviewFamily::Drakkin {
        (0..=7)
            .filter(|heritage| policy_for(*heritage).is_ok())
            .collect::<Vec<_>>()
    } else {
        vec![0]
    };
    let requested_heritage = character.appearance.drakkin_heritage;
    let heritage = heritages
        .iter()
        .find(|heritage| **heritage == requested_heritage)
        .or_else(|| heritages.first())
        .copied()
        .ok_or_else(|| {
            "No authored appearance is available for this class and gender.".to_owned()
        })?;
    let policy = policy_for(heritage).map_err(|error| error.to_string())?;
    let a = character.appearance;
    let appearance = Appearance {
        face: a.face,
        hair_style: a.hair_style,
        beard: a.beard,
        hair_color: a.hair_color,
        beard_color: a.beard_color,
        eye_color_1: a.eye_color_1,
        eye_color_2: a.eye_color_2,
        heritage: a.drakkin_heritage,
        tattoo: a.drakkin_tattoo,
        details: a.drakkin_details,
    };
    let valid = policy
        .validate_appearance(character.race, class, character.gender, appearance)
        .is_ok();
    // A provisional appearance can be invalid for the actual loaded family
    // or heritage. Return its supported defaults even when the provisional
    // model fell back, but certify only a fresh valid, fully resolved request.
    if valid && !appearance_resolved {
        return Err(
            "The selected appearance is missing required model parts or textures.".to_owned(),
        );
    }
    if valid && !diffuse_textures_loaded {
        return Err("One or more appearance textures are unavailable.".to_owned());
    }
    let receipt = valid.then_some(PreviewReceipt {
        context,
        family,
        model_loaded: true,
        race: character.race,
        class,
        gender: character.gender,
        appearance,
    });
    Ok(CreationPreview {
        policy,
        receipt,
        heritages,
    })
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
            additive: false,
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
            creation: None,
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

    fn creation_request() -> Request {
        Request {
            creation: Some(Context {
                session: 1,
                connection: 2,
                catalog_revision: 3,
                roster_revision: 4,
                draft_revision: 5,
            }),
            ..request()
        }
    }

    #[test]
    fn creation_capability_uses_loaded_family_and_requires_current_appearance() {
        let mut request = creation_request();
        request.model_set = CharacterModelSet::Luclin;
        request.character.appearance.hair_style = 1;
        let metadata = CustomizationCatalog::default();
        let fallback = creation_preview(
            &request,
            CharacterModelFamily::Classic,
            false,
            false,
            &metadata,
        )
        .unwrap();
        assert_eq!(fallback.policy.family(), PreviewFamily::Classic);
        assert_eq!(fallback.policy.default_appearance().hair_style, 0);
        assert!(
            fallback.receipt.is_none(),
            "discarded hair cannot certify preview"
        );
        request.character.appearance.hair_style = 0;
        let fresh = creation_preview(
            &request,
            CharacterModelFamily::Classic,
            true,
            true,
            &metadata,
        )
        .unwrap();
        let receipt = fresh.receipt.unwrap();
        assert_eq!(receipt.family, PreviewFamily::Classic);
        assert_eq!(receipt.context, request.creation.unwrap());
        assert_eq!(receipt.appearance, fresh.policy.default_appearance());
        request.model_set = CharacterModelSet::Classic;
        let luclin = creation_preview(
            &request,
            CharacterModelFamily::Luclin,
            true,
            true,
            &metadata,
        )
        .unwrap();
        assert_eq!(luclin.policy.family(), PreviewFamily::Luclin);
        assert_eq!(luclin.receipt.unwrap().family, PreviewFamily::Luclin);
        assert!(
            creation_preview(
                &request,
                CharacterModelFamily::Modern,
                true,
                true,
                &metadata
            )
            .is_err()
        );
        request.creation = None;
        assert!(
            creation_preview(
                &request,
                CharacterModelFamily::Classic,
                true,
                true,
                &metadata
            )
            .is_err()
        );
    }

    #[test]
    fn missing_diffuse_texture_cannot_issue_a_creation_receipt() {
        let request = creation_request();
        let metadata = CustomizationCatalog::default();
        let complete = creation_preview(
            &request,
            CharacterModelFamily::Classic,
            true,
            true,
            &metadata,
        )
        .unwrap();
        assert!(complete.receipt.is_some());
        let missing = creation_preview(
            &request,
            CharacterModelFamily::Classic,
            false,
            true,
            &metadata,
        );
        assert_eq!(
            missing.unwrap_err(),
            "One or more appearance textures are unavailable."
        );
    }

    #[test]
    fn drakkin_policy_bootstraps_authored_heritage_without_certifying_fallback() {
        let metadata = CustomizationCatalog::parse(
            "522^0^Red^13107200^1^1,2,3,4^7^9^12^12^8^8^0^\n\
             522^2^Blue^15580^2^1,2,3,4^7^9^12^12^8^8^0^\n\
             522^3^Green^25600^2^1,2,3,4^7^9^12^12^8^8^0^\n\
             522^4^Broken^0^2^^7^9^12^12^8^8^0^",
        )
        .unwrap();
        let mut request = creation_request();
        request.character.race = 522;
        request.character.class = 2;
        let provisional = creation_preview(
            &request,
            CharacterModelFamily::Drakkin,
            false,
            false,
            &metadata,
        )
        .unwrap();
        assert_eq!(provisional.heritages, [2, 3]);
        assert_eq!(provisional.policy.default_appearance().heritage, 2);
        assert!(provisional.receipt.is_none());
        request.character.appearance.drakkin_heritage = 2;
        let fresh = creation_preview(
            &request,
            CharacterModelFamily::Drakkin,
            true,
            true,
            &metadata,
        )
        .unwrap();
        assert_eq!(fresh.receipt.unwrap().appearance.heritage, 2);
        assert!(
            creation_preview(
                &request,
                CharacterModelFamily::Classic,
                true,
                true,
                &metadata
            )
            .is_err()
        );
        assert!(
            creation_preview(
                &request,
                CharacterModelFamily::Drakkin,
                true,
                true,
                &CustomizationCatalog::default()
            )
            .is_err()
        );
        request.character.gender = 1;
        assert!(
            creation_preview(
                &request,
                CharacterModelFamily::Drakkin,
                true,
                true,
                &metadata
            )
            .is_err()
        );
    }

    #[test]
    fn creation_readiness_requires_the_complete_current_request() {
        let original = creation_request();
        let mut cache = Cache {
            desired: Some(original.clone()),
            ready: Some(()),
            job: None,
            failed: false,
        };
        assert!(cache.matching(&original).is_some());
        for field in 0..12 {
            let mut changed = original.clone();
            match field {
                0 => changed.creation.as_mut().unwrap().session += 1,
                1 => changed.creation.as_mut().unwrap().connection += 1,
                2 => changed.creation.as_mut().unwrap().catalog_revision += 1,
                3 => changed.creation.as_mut().unwrap().roster_revision += 1,
                4 => changed.creation.as_mut().unwrap().draft_revision += 1,
                5 => changed.character.appearance.face += 1,
                6 => changed.character.race += 1,
                7 => changed.character.class += 1,
                8 => changed.character.gender += 1,
                9 => changed.token.revision += 1,
                10 => changed.model_set = CharacterModelSet::Luclin,
                11 => changed.dir.push("other-assets"),
                _ => unreachable!(),
            }
            assert!(
                cache.matching(&changed).is_none(),
                "changed request field {field}"
            );
        }
        cache.ready = None;
        cache.failed = true;
        assert!(cache.matching(&original).is_none());
    }

    #[test]
    #[cfg(unix)]
    #[ignore = "requires original Luclin assets and GPU; disposable symlinks, no network or audio"]
    fn missing_requested_hair_can_render_but_cannot_certify_creation() {
        let source = openeq_assets::loader::default_client_dir().expect("original assets");
        struct Temporary(PathBuf);
        impl Drop for Temporary {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let missing = Temporary(std::env::temp_dir().join(format!(
            "openeq-preview-missing-hair-{}-{unique}",
            std::process::id()
        )));
        std::fs::create_dir(&missing.0).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if !entry
                .file_name()
                .to_string_lossy()
                .to_ascii_lowercase()
                .starts_with("lgequip")
            {
                std::os::unix::fs::symlink(entry.path(), missing.0.join(entry.file_name()))
                    .unwrap();
            }
        }
        let mut renderer = Renderer::new_headless(400, 300).unwrap();
        renderer.set_ui_scaled(&openeq_ui::UiFrame::default(), 1.);
        let mut request = creation_request();
        request.dir = missing.0.clone();
        request.model_set = CharacterModelSet::Luclin;
        let mut preview = Preview::default();
        for hair in [1, 0] {
            request.character.appearance.hair_style = hair;
            request.creation.as_mut().unwrap().draft_revision += 1;
            preview.update(Some(request.clone()), &renderer);
            let deadline = Instant::now() + Duration::from_secs(90);
            while preview.status().is_some() {
                preview.update(Some(request.clone()), &renderer);
                assert!(!preview.cache.failed);
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(2));
            }
            let prepared = preview.cache.ready.as_ref().unwrap();
            assert!(prepared.actors.diffuse_textures_loaded(prepared.state.id));
            assert_eq!(
                prepared.actors.model_family(prepared.state.id),
                Some(CharacterModelFamily::Luclin)
            );
            assert!(preview.render(&mut renderer, [400, 300], 256.));
            let support = preview.creation_preview(&request);
            if hair == 1 {
                assert!(
                    support
                        .unwrap_err()
                        .contains("missing required model parts")
                );
            } else {
                assert!(support.unwrap().receipt.is_some());
            }
        }
    }

    #[test]
    #[ignore = "requires original classic/Luclin/Drakkin assets and GPU; no network or audio"]
    fn loaded_creation_previews_issue_only_matching_receipts() {
        let base = openeq_assets::loader::default_client_dir().expect("original client assets");
        let renderer = Renderer::new_headless(400, 300).unwrap();
        for (preference, race, face, family) in [
            (CharacterModelSet::Classic, 1, 0, PreviewFamily::Classic),
            (CharacterModelSet::Classic, 1, 1, PreviewFamily::Classic),
            (CharacterModelSet::Luclin, 1, 0, PreviewFamily::Luclin),
            (CharacterModelSet::Luclin, 1, 1, PreviewFamily::Luclin),
            (CharacterModelSet::Luclin, 522, 0, PreviewFamily::Drakkin),
            (CharacterModelSet::Luclin, 522, 1, PreviewFamily::Drakkin),
        ] {
            let mut request = creation_request();
            request.dir = base.clone();
            request.model_set = preference;
            request.character.race = race;
            request.character.appearance.face = face;
            let mut preview = Preview::default();
            assert!(preview.creation_preview(&request).is_err());
            let deadline = Instant::now() + Duration::from_secs(90);
            while preview.status().is_some() {
                preview.update(Some(request.clone()), &renderer);
                assert!(!preview.cache.failed, "original {family:?} preview failed");
                assert!(Instant::now() < deadline, "original preview timed out");
                std::thread::sleep(Duration::from_millis(2));
            }
            let capability = preview.creation_preview(&request).unwrap();
            let receipt = capability.receipt.unwrap();
            assert_eq!(receipt.family, family);
            assert_eq!(receipt.race, race);
            assert_eq!(receipt.context, request.creation.unwrap());
            let mut stale = request.clone();
            stale.creation.as_mut().unwrap().draft_revision += 1;
            assert!(preview.creation_preview(&stale).is_err());
            stale = request.clone();
            stale.character.appearance.face += 1;
            assert!(preview.creation_preview(&stale).is_err());
            preview.update(None, &renderer);
            assert!(preview.creation_preview(&request).is_err());
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
