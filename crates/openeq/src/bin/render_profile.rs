//! Production render-path CPU/GPU profile using only the dedicated Broker fixture.
//! No gameplay commands are sent; stationary heartbeats preserve the login pose.
//! Timed frames have one submission in flight, no sleeps and no image readbacks.
//! Camera-only overrides: OPENEQ_PROFILE_POS=x,y,z, OPENEQ_PROFILE_YAW=degrees,
//! and OPENEQ_PROFILE_PITCH=degrees. OPENEQ_PROFILE_NO_LIGHTS=1 removes zone lights.
//! OPENEQ_PROFILE_BRUTE_LIGHTS=1 disables the light grid for a reference comparison.
use anyhow::{Context, ensure};
use openeq::{hud, interaction::Interaction, live::LiveWorld, spell_effects::EffectAssets};
use openeq_assets::{collision::CollisionWorld, loader};
use openeq_net::session::ConnectionConfig;
use openeq_render::{
    Camera, GpuScene, Renderer,
    actors::{ActorRenderer, CharacterModelSet},
    doors::{DoorRenderer, DoorState},
    environment::EnvironmentSettings,
    profiling::{GpuFrameTimings, GpuProfileStats},
};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::Write as _,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const WARMUP_FRAMES: usize = 60;
const CPU_NAMES: [&str; 11] = [
    "network",
    "states",
    "actors",
    "effects",
    "doors",
    "ui_build",
    "ui_upload",
    "render_submit",
    "gpu_wait",
    "gpu_collect",
    "total",
];
const GPU_NAMES: [&str; 21] = [
    "shadow",
    "gbuffer",
    "lighting",
    "transparency",
    "waterfall",
    "additive",
    "particles",
    "ui",
    "total",
    "frame_span",
    "raw_shadow",
    "raw_gbuffer",
    "raw_lighting",
    "raw_transparency_accum",
    "raw_transparency_resolve",
    "raw_waterfall",
    "raw_additive",
    "raw_particles",
    "raw_ui",
    "raw_pass_sum",
    "overlap",
];

struct Options {
    config: ConnectionConfig,
    output: PathBuf,
    width: u32,
    height: u32,
    frames: usize,
    models: CharacterModelSet,
    yaw: Option<f32>,
    pitch: Option<f32>,
    position: Option<[f32; 3]>,
    no_lights: bool,
    brute_lights: bool,
}
impl Options {
    fn parse() -> anyhow::Result<Self> {
        let mut args = std::env::args().skip(1);
        let config = args.next().context(
            "usage: render_profile CONFIG OUTPUT_PREFIX [WIDTH HEIGHT FRAMES classic|luclin]",
        )?;
        ensure!(
            Path::new(&config)
                .file_name()
                .is_some_and(|name| name == "storage2-commerce-credentials.json"),
            "requires storage2-commerce-credentials.json for the dedicated Broker fixture"
        );
        let config = ConnectionConfig::load(Path::new(&config))?;
        ensure!(
            config.host == "storage2.daeken.dev"
                && config.username == "openeq_commerce"
                && config.character == "Broker",
            "requires the dedicated Storage2 Broker fixture"
        );
        let output = PathBuf::from(args.next().context("missing OUTPUT_PREFIX")?);
        let width = args.next().map_or(Ok(2880), |value| value.parse::<u32>())?;
        let height = args.next().map_or(Ok(1800), |value| value.parse::<u32>())?;
        let frames = args
            .next()
            .map_or(Ok(300), |value| value.parse::<usize>())?;
        ensure!(
            (64..=8192).contains(&width) && (64..=8192).contains(&height),
            "dimensions must be 64–8192"
        );
        ensure!((1..=6000).contains(&frames), "frames must be 1–6000");
        let models = match args.next().as_deref() {
            None | Some("classic") => CharacterModelSet::Classic,
            Some("luclin") => CharacterModelSet::Luclin,
            Some(other) => anyhow::bail!("unknown model family {other:?}"),
        };
        ensure!(args.next().is_none(), "unexpected extra argument");
        // This changes only the diagnostic camera, never outgoing player heading.
        let yaw = std::env::var("OPENEQ_PROFILE_YAW")
            .ok()
            .map(|value| value.parse::<f32>())
            .transpose()
            .context("OPENEQ_PROFILE_YAW must be a finite number in degrees")?;
        ensure!(yaw.is_none_or(f32::is_finite), "yaw must be finite");
        let pitch = std::env::var("OPENEQ_PROFILE_PITCH")
            .ok()
            .map(|value| value.parse::<f32>())
            .transpose()
            .context("OPENEQ_PROFILE_PITCH must be a finite number in degrees")?;
        ensure!(pitch.is_none_or(f32::is_finite), "pitch must be finite");
        let no_lights = std::env::var("OPENEQ_PROFILE_NO_LIGHTS").is_ok_and(|value| value == "1");
        let brute_lights =
            std::env::var("OPENEQ_PROFILE_BRUTE_LIGHTS").is_ok_and(|value| value == "1");
        let position = std::env::var("OPENEQ_PROFILE_POS")
            .ok()
            .map(|value| -> anyhow::Result<[f32; 3]> {
                let values = value
                    .split(',')
                    .map(|part| part.trim().parse::<f32>())
                    .collect::<Result<Vec<_>, _>>()?;
                ensure!(
                    values.len() == 3 && values.iter().all(|value| value.is_finite()),
                    "OPENEQ_PROFILE_POS must be three finite scene coordinates: x,y,z"
                );
                Ok([values[0], values[1], values[2]])
            })
            .transpose()?;
        Ok(Self {
            config,
            output,
            width,
            height,
            frames,
            models,
            yaw: yaw.map(f32::to_radians),
            pitch: pitch.map(f32::to_radians),
            position,
            no_lights,
            brute_lights,
        })
    }
}

#[derive(Default)]
struct Sample {
    cpu: [f64; CPU_NAMES.len()],
    gpu: Option<GpuFrameTimings>,
    actors: usize,
    doors: usize,
    actor_draws: usize,
    actor_triangles: u64,
    ui_commands: usize,
}
struct Profile {
    live: LiveWorld,
    interaction: Interaction,
    renderer: Renderer,
    scene: GpuScene,
    collision: CollisionWorld,
    actors: ActorRenderer,
    doors: DoorRenderer,
    hud: hud::Hud,
    player: Camera,
    camera: Camera,
    ui_size: [u32; 2],
    started: Instant,
    last_gpu_frame: Option<u64>,
}
impl Profile {
    fn load(options: &Options) -> anyhow::Result<Self> {
        let base = loader::default_client_dir().context("original client assets")?;
        let strings = openeq::game::StringTable::load(&base);
        let spells = openeq::spells::SpellCatalog::load(&base)?;
        let effects = EffectAssets::load(&base)?;
        let hud = hud::Hud::load(&base)?;
        let mut live = LiveWorld::start(options.config.clone());
        live.game.strings = strings;
        live.game.spell_catalog = spells;
        live.spell_effects.set_assets(effects);
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            live.poll();
            ensure!(live.error.is_none(), "connection failed: {:?}", live.error);
            if live.ready
                && live.environment.is_some()
                && live.initial_position.is_some()
                && live.own_id.is_some()
                && live.game.profile.is_some()
                && live.game.inventory.received
            {
                break;
            }
            ensure!(Instant::now() < deadline, "Broker login timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
        let env = live.environment.as_ref().unwrap();
        ensure!(
            env.short_name == "poknowledge",
            "Broker must already be in poknowledge; no travel is performed"
        );
        ensure!(
            live.game.profile.as_ref().unwrap().name == "Broker",
            "unexpected fixture profile"
        );
        let mut source = loader::load_zone(&base, &env.short_name)?;
        if options.no_lights {
            source.lights.clear();
        }
        let collision = CollisionWorld::build(&source);
        let mut renderer = Renderer::new_headless(options.width, options.height)?;
        let sky = openeq_assets::environment::load_sky(
            &base,
            &env.short_name,
            (live.hour as f32 + live.minute as f32 / 60.) / 24.,
        )?;
        renderer.set_environment(
            EnvironmentSettings {
                zone_type: Some(env.zone_type),
                fog_color: env.fog_color[0],
                fog_start: env.fog_start[0],
                fog_end: env.fog_end[0],
                fog_density: env.fog_density,
                fog_enabled: env.fog_end[0] > env.fog_start[0],
                sky_enabled: !matches!(env.zone_type, 0 | 3 | 4) && env.sky != 0,
                ..Default::default()
            },
            Some(&sky),
        );
        let scene = GpuScene::build(renderer.device(), renderer.queue(), &source)?;
        scene.set_light_grid_enabled(renderer.queue(), !options.brute_lights);
        renderer.set_scene(&scene);
        let actors = ActorRenderer::load_with_model_set(&base, &env.short_name, options.models)?;
        let doors = DoorRenderer::load(&base, &env.short_name)?;
        let position = live.initial_position.take().unwrap();
        let player = Camera {
            position: [position.x, position.y, position.z + 3.],
            yaw: position.heading * std::f32::consts::TAU / 512.,
            ..Default::default()
        };
        let camera = Camera {
            position: options.position.unwrap_or(player.position),
            yaw: options.yaw.unwrap_or(player.yaw),
            pitch: options.pitch.unwrap_or(player.pitch),
            ..player
        };
        // Drain spawns received during asset loading. Default inventory/map closed.
        live.poll();
        renderer.enable_profiling(true);
        Ok(Self {
            live,
            interaction: Interaction::default(),
            renderer,
            scene,
            collision,
            actors,
            doors,
            hud,
            player,
            camera,
            ui_size: [options.width / 2, options.height / 2],
            started: Instant::now(),
            last_gpu_frame: None,
        })
    }

    fn frame(&mut self) -> anyhow::Result<Sample> {
        let start = Instant::now();
        let mut mark = start;
        let mut sample = Sample::default();
        self.live.poll();
        ensure!(
            self.live.ready && self.live.error.is_none(),
            "live session failed: {:?}",
            self.live.error
        );
        self.live.camera_position(&self.player, false);
        sample.cpu[0] = take_ms(&mut mark);
        let states = self.live.actor_states_with_terrain(
            self.camera.position,
            None,
            Some((&self.collision, Some(self.doors.collision_world()))),
        );
        sample.cpu[1] = take_ms(&mut mark);
        let elapsed = self.started.elapsed().as_secs_f32();
        self.actors.update(&self.renderer, &states, elapsed);
        sample.actors = self.actors.rendered_instances;
        sample.cpu[2] = take_ms(&mut mark);
        let anchors = self.live.effect_anchors(&self.player, Some(&self.actors));
        let particles = self.live.spell_effects.frame(Instant::now(), &anchors);
        self.renderer.set_particles(&particles);
        let projectiles = self
            .live
            .spell_effects
            .projectiles(Instant::now(), &anchors);
        self.actors.update_projectiles(&self.renderer, &projectiles);
        sample.cpu[3] = take_ms(&mut mark);
        self.doors
            .update(&self.renderer, &door_states(&self.live), elapsed);
        sample.doors = self.doors.rendered_instances;
        sample.cpu[4] = take_ms(&mut mark);
        let ui = self.ui_frame();
        sample.ui_commands = ui.commands.len();
        sample.cpu[5] = take_ms(&mut mark);
        self.renderer.set_ui_scaled(&ui, 2.);
        // The windowed app retains a copy for input/link hit testing too.
        let _link_hits = self.renderer.ui_link_hits().to_vec();
        sample.cpu[6] = take_ms(&mut mark);
        let mut draws = self.actors.draws();
        draws.extend(self.doors.draws());
        self.renderer
            .render_with_actors(&self.scene, &self.camera, &draws);
        sample.cpu[7] = take_ms(&mut mark);
        self.renderer
            .device()
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .context("waiting for submitted frame")?;
        sample.cpu[8] = take_ms(&mut mark);
        // Counts are outside the render-submit phase and included in collection overhead.
        sample.actor_draws = draws.iter().map(|actor| actor.scene.draws.len()).sum();
        sample.actor_triangles = draws.iter().map(|actor| triangles(&actor.scene)).sum();
        if let Some(gpu) = self.renderer.latest_gpu_timings()
            && self
                .last_gpu_frame
                .is_none_or(|previous| gpu.frame_id > previous)
        {
            self.last_gpu_frame = Some(gpu.frame_id);
            sample.gpu = Some(gpu);
        }
        sample.cpu[9] = take_ms(&mut mark);
        sample.cpu[10] = start.elapsed().as_secs_f64() * 1000.;
        Ok(sample)
    }

    fn ui_frame(&self) -> openeq_ui::UiFrame {
        let live = &self.live;
        let player = live.own_id.and_then(|id| live.entities.get(&id));
        let target = live
            .target
            .and_then(|id| live.entities.get(&id))
            .map(|entity| hud::HudTarget {
                name: display_name(&entity.spawn.name),
                hp: entity.spawn.hp_percent as f32 / 100.,
                level: entity.spawn.level,
            });
        let state = hud::HudState {
            character: live.character.clone(),
            player_level: player.map_or(0, |entity| entity.spawn.level),
            hp: player.map_or(0., |entity| entity.spawn.hp_percent as f32 / 100.),
            mana: live.game.mana.fraction(),
            endurance: live.game.endurance.fraction(),
            target,
            status: "Connected • Enter chat • I inventory • Q attack • /help".into(),
            entities: live.entities.len(),
            movement_updates: live.moves,
        };
        let mut frame = self
            .hud
            .gameplay_frame(self.ui_size, &state, &self.interaction.view(live));
        add_nameplates(&mut frame, live, &self.actors, &self.camera, self.ui_size);
        let mut feedback = openeq_ui::UiFrame {
            bounds: frame.bounds,
            ..Default::default()
        };
        live.combat_feedback.append(
            &mut feedback,
            &self.camera,
            self.ui_size,
            self.actors.bounds(),
            live.own_id,
            Instant::now(),
        );
        feedback.commands.append(&mut frame.commands);
        frame.commands = feedback.commands;
        frame
    }

    /// Metal resolves queries in a second submission after render completion.
    /// Drain outside the measured loop; neither warmup results nor stale IDs
    /// enter the measured GPU set. Eight bounded waits cover the four-slot ring.
    fn drain_gpu(&mut self) -> anyhow::Result<(GpuProfileStats, Vec<GpuFrameTimings>)> {
        let mut timings = Vec::new();
        let mut stats = self.renderer.profiling_stats();
        for _ in 0..8 {
            if let Some(gpu) = stats.latest
                && self
                    .last_gpu_frame
                    .is_none_or(|previous| gpu.frame_id > previous)
            {
                self.last_gpu_frame = Some(gpu.frame_id);
                timings.push(gpu);
            }
            if stats.in_flight == 0 {
                return Ok((stats, timings));
            }
            self.renderer
                .device()
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(2)),
                })
                .context("draining deferred GPU timing readbacks")?;
            stats = self.renderer.profiling_stats();
        }
        if let Some(gpu) = stats.latest
            && self
                .last_gpu_frame
                .is_none_or(|previous| gpu.frame_id > previous)
        {
            self.last_gpu_frame = Some(gpu.frame_id);
            timings.push(gpu);
        }
        Ok((stats, timings))
    }
}

fn take_ms(mark: &mut Instant) -> f64 {
    let now = Instant::now();
    let elapsed = now.duration_since(*mark).as_secs_f64() * 1000.;
    *mark = now;
    elapsed
}
fn triangles(scene: &GpuScene) -> u64 {
    scene
        .draws
        .iter()
        .map(|draw| u64::from(draw.index_count / 3) * u64::from(draw.instance_count))
        .sum()
}
fn door_states(live: &LiveWorld) -> Vec<DoorState> {
    live.doors
        .values()
        .map(|door| DoorState {
            id: door.id,
            name: door.name.clone(),
            position: door.position,
            heading: door.heading,
            incline: door.incline,
            size: door.size,
            open_type: door.open_type,
            state: door.state,
            inverted: door.inverted,
            parameter: door.parameter,
        })
        .collect()
}
fn display_name(name: &str) -> String {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
        .replace('_', " ")
        .trim_start_matches('#')
        .to_owned()
}

// Keep in step with main.rs: private windowed helper, including its 180-unit range.
fn add_nameplates(
    frame: &mut openeq_ui::UiFrame,
    live: &LiveWorld,
    actors: &ActorRenderer,
    camera: &Camera,
    size: [u32; 2],
) {
    let viewport = openeq_ui::Rect::new(0., 0., size[0] as f32, size[1] as f32);
    let mut labels = Vec::new();
    for entity in live.entities.values() {
        let spawn = &entity.spawn;
        if Some(spawn.id) == live.own_id || spawn.race == 127 || spawn.body_type >= 66 {
            continue;
        }
        let Some(bounds) = actors.bounds().get(&spawn.id) else {
            continue;
        };
        let distance: f32 = bounds
            .center()
            .iter()
            .zip(camera.position)
            .map(|(a, b)| (a - b).powi(2))
            .sum();
        if distance > 180. * 180. {
            continue;
        }
        let Some([left, top, right, bottom]) =
            openeq::targeting::screen_bounds(camera, size.map(|v| v as f32), bounds)
        else {
            continue;
        };
        if right < 0. || left > size[0] as f32 || bottom < 0. || top > size[1] as f32 {
            continue;
        }
        labels.push(openeq_ui::DrawCommand::Text {
            rect: openeq_ui::Rect::new((left + right) * 0.5 - 110., top - 22., 220., 20.),
            clip: viewport,
            text: display_name(&spawn.name),
            font: 2,
            color: if live.target == Some(spawn.id) {
                [255, 230, 100, 255]
            } else {
                [190, 235, 245, 255]
            },
            align: openeq_ui::TextAlign::Center,
            vertical_center: true,
            wrap: false,
        });
    }
    labels.append(&mut frame.commands);
    frame.commands = labels;
}
fn gpu_values(gpu: GpuFrameTimings) -> [f64; GPU_NAMES.len()] {
    [
        gpu.shadow_ms,
        gpu.gbuffer_ms,
        gpu.lighting_ms,
        gpu.transparency_ms,
        gpu.waterfall_ms,
        gpu.additive_ms,
        gpu.particles_ms,
        gpu.ui_ms,
        gpu.total_ms,
        gpu.frame_span_ms,
        gpu.raw_pass_ms[0],
        gpu.raw_pass_ms[1],
        gpu.raw_pass_ms[2],
        gpu.raw_pass_ms[3],
        gpu.raw_pass_ms[4],
        gpu.raw_pass_ms[5],
        gpu.raw_pass_ms[6],
        gpu.raw_pass_ms[7],
        gpu.raw_pass_ms[8],
        gpu.raw_pass_sum_ms,
        gpu.overlap_ms,
    ]
}
fn percentiles(mut values: Vec<f64>) -> (f64, f64) {
    values.sort_by(f64::total_cmp);
    let median = if values.len().is_multiple_of(2) {
        (values[values.len() / 2 - 1] + values[values.len() / 2]) / 2.
    } else {
        values[values.len() / 2]
    };
    (
        median,
        values[(values.len() * 95).div_ceil(100).saturating_sub(1)],
    )
}
fn output_path(prefix: &Path, extension: &str) -> PathBuf {
    let mut value = prefix.as_os_str().to_os_string();
    value.push(extension);
    PathBuf::from(value)
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn".into()),
        )
        .init();
    let options = Options::parse()?;
    if let Some(parent) = options
        .output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut profile = Profile::load(&options)?;
    println!(
        "Preloading PoK and warming {WARMUP_FRAMES} frames at {}×{} ({:?})…",
        options.width, options.height, options.models
    );
    for _ in 0..WARMUP_FRAMES {
        profile.frame()?;
    }
    let (gpu_before, _) = profile.drain_gpu()?;
    ensure!(
        gpu_before.in_flight == 0,
        "warmup GPU timing readbacks did not drain"
    );
    let moves_before = profile.live.moves;
    let entities_before = profile.live.entities.len();
    let first_gpu_frame = gpu_before.submitted + gpu_before.dropped + 1;
    let mut samples = Vec::with_capacity(options.frames);
    for _ in 0..options.frames {
        samples.push(profile.frame()?);
    }
    let (gpu_after, drained) = profile.drain_gpu()?;
    let mut gpu_by_frame: BTreeMap<_, _> = samples
        .iter_mut()
        .filter_map(|sample| sample.gpu.take().map(|gpu| (gpu.frame_id, gpu)))
        .collect();
    gpu_by_frame.extend(drained.into_iter().map(|gpu| (gpu.frame_id, gpu)));
    // Readback arrival may lag rendering by a frame. Pair CSV values with the
    // frame that produced them, rather than whichever CPU frame observed them.
    for (index, sample) in samples.iter_mut().enumerate() {
        sample.gpu = gpu_by_frame.remove(&(first_gpu_frame + index as u64));
    }
    let mut report = String::new();
    writeln!(
        report,
        "Diagnostic OPENEQ_PROFILE_NO_LIGHTS={}: {}",
        u8::from(options.no_lights),
        if options.no_lights {
            "all authored zone lights removed before GPU upload"
        } else {
            "normal authored zone lights enabled"
        }
    )?;
    writeln!(
        report,
        "Diagnostic OPENEQ_PROFILE_BRUTE_LIGHTS={}: {}",
        u8::from(options.brute_lights),
        if options.brute_lights {
            "brute-force lighting reference; light grid disabled"
        } else {
            "light grid enabled"
        }
    )?;
    writeln!(
        report,
        "PoK render profile: {}×{} physical, {}×{} logical, {:?} models; warmup={}, measured={}",
        options.width,
        options.height,
        profile.ui_size[0],
        profile.ui_size[1],
        options.models,
        WARMUP_FRAMES,
        options.frames
    )?;
    writeln!(
        report,
        "Camera position={:?}, yaw_degrees={:.3}, pitch_degrees={:.3}; inventory/map closed",
        profile.camera.position,
        profile.camera.yaw.to_degrees(),
        profile.camera.pitch.to_degrees()
    )?;
    writeln!(
        report,
        "World lights={}, draws={}, triangles={}; entities={}→{}, NPC movement updates={}",
        profile.scene.light_count,
        profile.scene.draws.len(),
        triangles(&profile.scene),
        entities_before,
        profile.live.entities.len(),
        profile.live.moves - moves_before
    )?;
    let first = samples.first().unwrap();
    let last = samples.last().unwrap();
    writeln!(
        report,
        "Actors rendered={}–{}; doors={}→{}; actor+door draws={}→{}, triangles={}→{}; UI commands={}→{}",
        samples.iter().map(|s| s.actors).min().unwrap(),
        samples.iter().map(|s| s.actors).max().unwrap(),
        first.doors,
        last.doors,
        first.actor_draws,
        last.actor_draws,
        first.actor_triangles,
        last.actor_triangles,
        first.ui_commands,
        last.ui_commands
    )?;
    writeln!(
        report,
        "Fixed camera; live network updates and NPC interpolation continue. Not a frozen-world replay. Sky/time/fog selected once at preload. Timing excludes loading, warmup, final timestamp drain, report writing and PNG readback."
    )?;
    writeln!(
        report,
        "A GPU Wait after every submit bounds queue depth to one. CPU render-submit may overlap GPU work; gpu_wait is the remaining wait, not total GPU execution. These frame times are serialized diagnostic measurements, not ordinary pipelined FPS."
    )?;
    writeln!(report, "CPU phase                 median_ms      p95_ms")?;
    for (index, name) in CPU_NAMES.iter().enumerate() {
        let (median, p95) = percentiles(samples.iter().map(|s| s.cpu[index]).collect());
        writeln!(report, "{name:<24} {median:>10.3} {p95:>11.3}")?;
    }
    let gpu: Vec<_> = samples
        .iter()
        .filter_map(|sample| sample.gpu.map(gpu_values))
        .collect();
    writeln!(
        report,
        "GPU supported={}, enabled={}, unique samples={}, submitted={}, completed={}, dropped={}, failed={}, pending_after_drain={}",
        gpu_after.supported,
        gpu_after.enabled,
        gpu.len(),
        gpu_after.submitted - gpu_before.submitted,
        gpu_after.completed - gpu_before.completed,
        gpu_after.dropped - gpu_before.dropped,
        gpu_after.failed - gpu_before.failed,
        gpu_after.in_flight
    )?;
    writeln!(
        report,
        "GPU pass values use completion-boundary attribution: each pass is charged only time after preceding passes finished. These are not isolated pass cycle counts. Raw vertex-start to fragment-end intervals may overlap on tile GPUs; raw_pass_sum includes that overlap, and overlap reports the excess over attributed total. frame_span includes gaps. Unique GPU IDs are matched to their originating CPU frame; missing or failed samples remain blank."
    )?;
    if !gpu.is_empty() {
        writeln!(report, "GPU pass                  median_ms      p95_ms")?;
        for (index, name) in GPU_NAMES.iter().enumerate() {
            let (median, p95) = percentiles(gpu.iter().map(|sample| sample[index]).collect());
            writeln!(report, "{name:<24} {median:>10.3} {p95:>11.3}")?;
        }
    }
    let mut csv = std::fs::File::create(output_path(&options.output, ".csv"))?;
    writeln!(
        csv,
        "frame,{},gpu_frame,{},actors,doors,actor_draws,actor_triangles,ui_commands",
        CPU_NAMES.map(|name| format!("cpu_{name}_ms")).join(","),
        GPU_NAMES.map(|name| format!("gpu_{name}_ms")).join(",")
    )?;
    for (index, sample) in samples.iter().enumerate() {
        let gpu = sample
            .gpu
            .map(|gpu| {
                format!(
                    "{},{}",
                    gpu.frame_id,
                    gpu_values(gpu).map(|value| format!("{value:.6}")).join(",")
                )
            })
            .unwrap_or_else(|| ",".repeat(GPU_NAMES.len()));
        writeln!(
            csv,
            "{index},{},{gpu},{},{},{},{},{}",
            sample.cpu.map(|value| format!("{value:.6}")).join(","),
            sample.actors,
            sample.doors,
            sample.actor_draws,
            sample.actor_triangles,
            sample.ui_commands
        )?;
    }
    let image_path = output_path(&options.output, ".png");
    let (width, height, pixels) = profile
        .renderer
        .read_rgba()
        .context("final frame readback")?;
    image::save_buffer(&image_path, &pixels, width, height, image::ColorType::Rgba8)?;
    writeln!(report, "Screenshot: {}", image_path.display())?;
    writeln!(
        report,
        "Only stationary heartbeat sent; no gameplay actions. Server login may adjust saved Z by +0.75."
    )?;
    std::fs::write(output_path(&options.output, ".txt"), &report)?;
    print!("{report}");
    drop(profile);
    Ok(())
}
