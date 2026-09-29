//! Complete destination preparation, including GPU uploads, off the event loop.
use crate::{
    hud::Hud,
    loading::{Job, Reporter},
    map::ZoneMap,
};
use openeq_assets::{collision::CollisionWorld, environment::SkyAssets, loader};
use openeq_render::{
    actors::{ActorRenderer, ActorState},
    doors::{DoorRenderer, DoorState},
    scene::GpuScene,
    upload::UploadContext,
};
use std::path::PathBuf;

pub struct Request {
    pub dir: PathBuf,
    pub zone: String,
    pub online: bool,
    pub model_set: openeq_assets::character::CharacterModelSet,
    pub time_of_day: f32,
    pub actors: Vec<ActorState>,
    pub doors: Vec<DoorState>,
}

pub struct ClientData {
    pub hud: Option<Hud>,
    pub strings: crate::game::StringTable,
    pub spells: crate::spells::SpellCatalog,
    pub spell_effects: Option<crate::spell_effects::EffectAssets>,
}

pub struct PreparedZone {
    pub scene: GpuScene,
    pub collision: Option<CollisionWorld>,
    pub actors: Option<ActorRenderer>,
    pub doors: Option<DoorRenderer>,
    pub map: Option<ZoneMap>,
    pub sky: Option<SkyAssets>,
    pub zone_lines: openeq_assets::zone_lines::ZoneLines,
    pub liquids: openeq_assets::liquid_regions::LiquidRegions,
}

pub fn start(request: Request, upload: UploadContext) -> Job<PreparedZone> {
    Job::start(move |report| prepare(request, upload, report))
}

fn prepare(
    request: Request,
    upload: UploadContext,
    report: Reporter<PreparedZone>,
) -> anyhow::Result<PreparedZone> {
    let Request {
        dir,
        zone,
        online,
        model_set,
        time_of_day,
        actors: actor_states,
        doors: door_states,
    } = request;
    let started = std::time::Instant::now();
    report.stage("Reading the zone and its textures", Some(0.05))?;
    let scene = loader::load_zone(&dir, &zone)?;
    report.stage("Preparing terrain and collision", Some(0.25))?;
    let collision = online.then(|| CollisionWorld::build(&scene));
    report.stage("Uploading the world", Some(0.40))?;
    let scene = GpuScene::build(upload.device(), upload.queue(), &scene)?;
    report.stage("Preparing the sky and map", Some(0.55))?;
    let sky = openeq_assets::environment::load_sky(&dir, &zone, time_of_day).ok();
    let map = ZoneMap::load(&dir, &zone).ok();
    let zone_lines =
        openeq_assets::zone_lines::ZoneLines::load(&dir, &zone).unwrap_or_else(|error| {
            tracing::warn!(%zone, %error, "authored zone borders unavailable");
            Default::default()
        });
    let liquids =
        openeq_assets::liquid_regions::LiquidRegions::load(&dir, &zone).unwrap_or_else(|error| {
            tracing::warn!(%zone, %error, "authored liquid regions unavailable");
            Default::default()
        });
    report.stage("Loading characters", Some(0.60))?;
    let actors = if online {
        let mut actors = ActorRenderer::load_with_model_set(&dir, &zone, model_set)?;
        anyhow::ensure!(
            actors.preload_with_progress(&upload, &actor_states, 0., |done, total| {
                report
                    .stage(
                        format!("Preparing characters ({done}/{total})"),
                        Some(0.60 + 0.20 * done as f32 / total.max(1) as f32),
                    )
                    .is_ok()
            }),
            "Loading cancelled"
        );
        Some(actors)
    } else {
        None
    };
    report.stage("Preparing doors and lifts", Some(0.80))?;
    let doors = if online {
        let mut doors = DoorRenderer::load(&dir, &zone)?;
        anyhow::ensure!(
            doors.preload_with_progress(&upload, &door_states, 0., |done, total| {
                report
                    .stage(
                        format!("Preparing doors and lifts ({done}/{total})"),
                        Some(0.80 + 0.10 * done as f32 / total.max(1) as f32),
                    )
                    .is_ok()
            }),
            "Loading cancelled"
        );
        Some(doors)
    } else {
        None
    };
    report.stage("Entering the world", Some(1.))?;
    tracing::info!(%zone, elapsed_ms = started.elapsed().as_millis() as u64, "destination prepared");
    Ok(PreparedZone {
        scene,
        collision,
        actors,
        doors,
        map,
        sky,
        zone_lines,
        liquids,
    })
}

/// Load metadata before networking starts, so incoming ID-based messages and
/// spell cooldowns can be interpreted immediately. The window can draw meanwhile.
pub fn start_client(dir: PathBuf) -> Job<ClientData> {
    Job::start(move |report| {
        report.stage("Reading game messages", Some(0.1))?;
        let strings = crate::game::StringTable::load(&dir);
        report.stage("Reading spells and abilities", Some(0.35))?;
        let spells = crate::spells::SpellCatalog::load(&dir).unwrap_or_default();
        report.stage("Reading spell effects", Some(0.50))?;
        let spell_effects = match crate::spell_effects::EffectAssets::load(&dir) {
            Ok(assets) => Some(assets),
            Err(error) => {
                tracing::warn!(%error, "original spell effects unavailable");
                None
            }
        };
        report.stage("Preparing the interface", Some(0.65))?;
        let hud = match Hud::load(&dir) {
            Ok(hud) => Some(hud),
            Err(error) => {
                tracing::warn!(%error, "XML HUD unavailable");
                None
            }
        };
        report.stage("Connecting to the world", Some(1.))?;
        Ok(ClientData {
            hud,
            strings,
            spells,
            spell_effects,
        })
    })
}
