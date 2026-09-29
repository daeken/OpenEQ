//! Real server-driven spell particles, restricted to the Arcanist fixture.
//! Never memorizes spells or moves items/coins. Heartbeats preserve the login
//! position and heading; the server may still adjust the saved Z on login.
use anyhow::{Context, ensure};
use openeq::{
    interaction::Interaction, live::LiveWorld, spell_effects::EffectAssets, spells::SpellCatalog,
};
use openeq_assets::{collision::CollisionWorld, loader};
use openeq_net::{gameplay::Command, session::ConnectionConfig};
use openeq_render::{
    Camera, GpuScene, Renderer,
    actors::{ActorRenderer, CharacterModelSet},
    particles::{ParticleFrame, ParticleStats},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const SPELL: u32 = 288;

struct Probe {
    live: LiveWorld,
    ui: Interaction,
    renderer: Renderer,
    actors: ActorRenderer,
    scene: GpuScene,
    player: Camera,
    camera: Camera,
    started: Instant,
    particles: ParticleFrame,
    stats: ParticleStats,
    gem: u8,
    first_person: bool,
}

impl Probe {
    fn connect(config: ConnectionConfig, base: &Path, first_person: bool) -> anyhow::Result<Self> {
        let mut live = LiveWorld::start(config);
        live.game.spell_catalog = SpellCatalog::load(base)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            live.poll();
            ensure!(live.error.is_none(), "connection failed: {:?}", live.error);
            if live.ready
                && live.own_id.is_some()
                && live.environment.is_some()
                && live.initial_position.is_some()
                && live.game.profile.is_some()
                && live.game.inventory.received
            {
                break;
            }
            ensure!(Instant::now() < deadline, "fixture login timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
        let profile = live.game.profile.as_ref().unwrap();
        ensure!(
            profile.name == "Arcanist" && profile.class == 12,
            "wrong fixture profile"
        );
        let gem = profile
            .memorized_spells
            .iter()
            .take(12)
            .position(|id| *id == SPELL)
            .context("Minor Shielding must already be memorized; fixture unchanged")?
            as u8;
        ensure!(
            !live.game.buffs.values().any(|buff| buff.spell_id == SPELL),
            "fixture already has Minor Shielding; refusing to overwrite its existing buff"
        );

        live.spell_effects.set_assets(EffectAssets::load(base)?);
        let zone = live.environment.as_ref().unwrap().short_name.clone();
        let source = loader::load_zone(base, &zone)?;
        let collision = CollisionWorld::build(&source);
        let mut renderer = Renderer::new_headless(1280, 720)?;
        let scene = GpuScene::build(renderer.device(), renderer.queue(), &source)?;
        renderer.set_scene(&scene);
        let actors = ActorRenderer::load_with_model_set(base, &zone, CharacterModelSet::Luclin)?;
        let position = live.initial_position.context("initial player position")?;
        let player = Camera {
            position: [position.x, position.y, position.z + 3.],
            yaw: position.heading * std::f32::consts::TAU / 512.,
            ..Default::default()
        };
        let focus = [position.x, position.y, position.z + 0.5];
        let (sin, cos) = player.yaw.sin_cos();
        let desired = [
            focus[0] + sin * 16. + cos * 6.,
            focus[1] + cos * 16. - sin * 6.,
            focus[2] + 3.,
        ];
        let camera_position = collision.clip_camera(focus, desired, 0.4);
        let delta: [f32; 3] = std::array::from_fn(|i| focus[i] - camera_position[i]);
        let camera = if first_person {
            player
        } else {
            Camera {
                position: camera_position,
                yaw: delta[0].atan2(delta[1]),
                pitch: delta[2].atan2(delta[0].hypot(delta[1])),
                ..Default::default()
            }
        };
        let own = live.own_id;
        live.set_target(own);
        Ok(Self {
            live,
            ui: Interaction::default(),
            renderer,
            actors,
            scene,
            player,
            camera,
            started: Instant::now(),
            particles: ParticleFrame::default(),
            stats: ParticleStats::default(),
            gem,
            first_person,
        })
    }

    fn step(&mut self) -> anyhow::Result<()> {
        self.live.poll();
        ensure!(
            self.live.ready && self.live.error.is_none(),
            "session failed: {:?}",
            self.live.error
        );
        self.live.camera_position(&self.player, false);
        let now = Instant::now();
        let player = (!self.first_person).then_some((&self.player, false));
        let states = self.live.actor_states(self.camera.position, player);
        self.actors.update(
            &self.renderer,
            &states,
            self.started.elapsed().as_secs_f32(),
        );
        let anchors = self.live.effect_anchors(&self.player, Some(&self.actors));
        self.particles = self.live.spell_effects.frame(now, &anchors);
        self.stats = self.renderer.set_particles(&self.particles);
        ensure!(
            self.stats.invalid == 0
                && self.stats.missing_texture == 0
                && self.stats.over_capacity == 0,
            "particle upload rejected instances: {:?}",
            self.stats
        );
        self.renderer
            .render_with_actors(&self.scene, &self.camera, &self.actors.draws());
        Ok(())
    }

    fn until(
        &mut self,
        label: &str,
        seconds: u64,
        mut ready: impl FnMut(&mut Self) -> anyhow::Result<bool>,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.step()?;
            if ready(self)? {
                return Ok(());
            }
            ensure!(Instant::now() < deadline, "timed out waiting for {label}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Compare the same scene, pose and camera with and without billboards.
    /// Submitted GPU instances alone would not prove particles are visible.
    fn capture(&mut self, path: &Path) -> anyhow::Result<usize> {
        let (w, h, with) = self
            .renderer
            .read_rgba()
            .context("particle frame readback")?;
        self.renderer.clear_particles();
        self.renderer
            .render_with_actors(&self.scene, &self.camera, &self.actors.draws());
        let (_, _, without) = self.renderer.read_rgba().context("baseline readback")?;
        let changed = with
            .chunks_exact(4)
            .zip(without.chunks_exact(4))
            .filter(|(a, b)| a[..3].iter().zip(&b[..3]).any(|(a, b)| a.abs_diff(*b) > 4))
            .count();
        image::save_buffer(path, &with, w, h, image::ColorType::Rgba8)?;
        self.renderer.set_particles(&self.particles);
        Ok(changed)
    }

    fn shield_present(&self) -> bool {
        self.live
            .game
            .buffs
            .values()
            .any(|buff| buff.spell_id == SPELL)
    }

    fn remove_shield(&mut self) -> anyhow::Result<()> {
        let id = self.live.own_id.context("fixture entity")?;
        let slots: Vec<_> = self
            .live
            .game
            .buffs
            .values()
            .filter(|buff| buff.spell_id == SPELL)
            .map(|buff| buff.slot)
            .collect();
        for slot in slots {
            ensure!(
                self.live.command(Command::RemoveBuff {
                    slot,
                    player_id: id
                }),
                "buff removal not queued"
            );
        }
        self.until("added shield removal", 5, |probe| {
            Ok(!probe.shield_present())
        })
    }

    fn exercise(&mut self, output: &Path) -> anyhow::Result<()> {
        self.until("initial particle idle", 10, |probe| {
            Ok(probe.stats.rendered == 0)
        })?;
        self.ui.cast(self.gem, &mut self.live);
        self.until("first authoritative cast", 5, |probe| {
            Ok(probe.live.game.casting.is_some())
        })?;
        let mut cast_peak = 0;
        let mut cast_pixels = 0;
        self.until("first cast completion", 8, |probe| {
            let cast_particles = probe.live.spell_effects.stats().cast_particles;
            if probe.live.game.casting.is_some() && cast_particles > cast_peak {
                cast_peak = cast_particles;
                cast_pixels = probe.capture(&output.join("casting.png"))?;
            }
            Ok(probe.live.game.casting.is_none() && probe.shield_present())
        })?;
        ensure!(
            cast_peak > 0 && cast_pixels > 10,
            "cast not visible: {cast_peak} particles, {cast_pixels} changed pixels"
        );
        let mut impact_peak = 0;
        let mut impact_pixels = 0;
        let completed = Instant::now();
        self.until("completion effect sampling", 8, |probe| {
            let impact_particles = probe.live.spell_effects.stats().impact_particles;
            if impact_particles > impact_peak {
                impact_peak = impact_particles;
                impact_pixels = probe.capture(&output.join("impact.png"))?;
            }
            Ok(completed.elapsed() > Duration::from_secs(2))
        })?;
        ensure!(
            impact_peak > 0 && impact_pixels > 10,
            "completion not visible: {impact_peak} particles, {impact_pixels} changed pixels"
        );
        println!(
            "VISIBLE cast_particles={cast_peak} cast_pixels={cast_pixels} completion_particles={impact_peak} completion_pixels={impact_pixels}"
        );
        self.remove_shield()?;
        let gem = self.gem;
        self.until("first effect retirement and gem readiness", 25, |probe| {
            Ok(probe.stats.rendered == 0
                && probe
                    .live
                    .game
                    .spell_cooldowns
                    .get(&gem)
                    .is_none_or(|until| *until <= Instant::now()))
        })?;

        self.ui.cast(self.gem, &mut self.live);
        self.until("second cast particles", 5, |probe| {
            Ok(probe.live.game.casting.is_some()
                && probe.live.spell_effects.stats().cast_particles > 0)
        })?;
        let interrupt_particles = self.stats.rendered;
        ensure!(
            self.live.command(Command::InterruptSpell),
            "interrupt not queued"
        );
        self.until(
            "authoritative interruption and particle cleanup",
            5,
            |probe| {
                Ok(probe.live.game.casting.is_none()
                    && probe.live.game.cast_pending_until.is_none()
                    && probe.stats.rendered == 0)
            },
        )?;
        ensure!(
            !self.shield_present(),
            "interrupted cast incorrectly added a buff"
        );
        self.capture(&output.join("interrupted.png"))?;
        println!(
            "INTERRUPTED particles_before={interrupt_particles} particles_after=0 buff_absent=true"
        );
        Ok(())
    }

    fn cleanup(&mut self) -> anyhow::Result<()> {
        if self.live.game.casting.is_some() || self.live.game.cast_pending_until.is_some() {
            self.live.command(Command::InterruptSpell);
            self.until("cleanup interruption", 5, |probe| {
                Ok(probe.live.game.casting.is_none()
                    && probe.live.game.cast_pending_until.is_none())
            })?;
        }
        self.remove_shield()
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openeq=info,openeq_net=info".into()),
        )
        .init();
    let mut first_person = false;
    let mut positional = Vec::new();
    for argument in std::env::args().skip(1) {
        if argument == "--first-person" {
            ensure!(!first_person, "duplicate --first-person");
            first_person = true;
        } else {
            ensure!(!argument.starts_with("--"), "unknown option: {argument}");
            positional.push(argument);
        }
    }
    let mut args = positional.into_iter();
    let config_path = args
        .next()
        .context("usage: spell_effect_smoke CONFIG [OUTPUT_DIRECTORY] [--first-person]")?;
    let output = PathBuf::from(
        args.next()
            .unwrap_or_else(|| "/tmp/openeq-spell-effects".into()),
    );
    ensure!(args.next().is_none(), "unexpected extra argument");
    let config = ConnectionConfig::load(Path::new(&config_path))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_spells"
            && config.character == "Arcanist",
        "requires the dedicated Storage2 Arcanist fixture"
    );
    let base = loader::default_client_dir().context("original client assets")?;
    std::fs::create_dir_all(&output)?;
    let mut probe = Probe::connect(config, &base, first_person)?;
    let original_inventory = format!("{:?}", probe.live.game.inventory.items);
    let original_currency = format!("{:?}", probe.live.game.currency);
    let profile = probe.live.game.profile.as_ref().unwrap();
    let original_book = profile.spell_book.clone();
    let original_gems = profile.memorized_spells.clone();
    let outcome = probe.exercise(&output);
    let cleanup = probe.cleanup();
    ensure!(
        format!("{:?}", probe.live.game.inventory.items) == original_inventory,
        "fixture inventory changed"
    );
    ensure!(
        format!("{:?}", probe.live.game.currency) == original_currency,
        "fixture currency changed"
    );
    let profile = probe.live.game.profile.as_ref().unwrap();
    ensure!(
        profile.spell_book == original_book && profile.memorized_spells == original_gems,
        "fixture spellbook/gems changed"
    );
    if let Err(error) = cleanup {
        anyhow::bail!("probe result {outcome:?}; cleanup failed: {error:#}");
    }
    outcome?;
    drop(probe);
    std::thread::sleep(Duration::from_secs(1));
    println!(
        "RESULT view={} cast_visible=true completion_visible=true interruption_clean=true fixture_items_currency_book_gems=unchanged output={}",
        if first_person {
            "first-person"
        } else {
            "third-person"
        },
        output.display()
    );
    Ok(())
}
