//! Background networking and bounded prediction of server-authoritative spawns.
use crate::coordinates;
use crate::game::{GameplayState, display_name};
use openeq_assets::collision::CollisionWorld;
use openeq_net::{
    gameplay::{Command, Door, GameplayEvent, ZoneDestination, ZonePoint},
    session::ConnectionConfig,
    zone::{Environment, Position, Spawn, ZoneEvent},
};
use openeq_render::actors::{ActorAction, ActorState, CharacterAppearance, EquipmentAppearance};
use std::{
    collections::BTreeMap,
    sync::{Mutex, mpsc},
    time::Instant,
};

pub enum Message {
    Event(Box<ZoneEvent>),
    Error(String),
    CommandSent(Command),
    CommandRejected {
        command: Command,
        notice: String,
    },
    RecoverySent(crate::death::RecoveryToken),
    RaidSent(u64),
    RaidRejected {
        token: u64,
        notice: String,
    },
    RecoveryRejected {
        token: crate::death::RecoveryToken,
        notice: String,
    },
}
pub(crate) enum NetworkCommand {
    Target(u32),
    Gameplay(Command),
    Raid {
        request: crate::raid::QueuedRequest,
        motion_revision: u64,
        raid_generation: u64,
    },
    Recovery {
        request: crate::death::RecoveryRequest,
        motion_revision: u64,
    },
}
// EQEmu sends NPC walking corrections every five seconds. Predict only a little
// beyond that interval, and converge small network corrections without a jump.
const PREDICTION_SECONDS: f32 = 6.;
const CORRECTION_SECONDS: f32 = 0.15;
const TELEPORT_DISTANCE: f32 = 100.;

pub struct Entity {
    pub spawn: Spawn,
    correction: [f32; 3],
    heading_correction: f32,
    arrived: Instant,
    moving_until: Instant,
    action: ActorAction,
    action_sequence: u64,
}
impl Entity {
    fn new(spawn: Spawn, now: Instant) -> Self {
        let action = if spawn.is_corpse {
            ActorAction::Dead
        } else {
            posture_action(spawn.stand_state as u32)
        };
        Self {
            spawn,
            correction: [0.; 3],
            heading_correction: 0.,
            arrived: now,
            moving_until: now,
            action,
            action_sequence: 0,
        }
    }

    fn age(&self, now: Instant) -> f32 {
        now.saturating_duration_since(self.arrived).as_secs_f32()
    }

    pub fn position(&self, now: Instant) -> [f32; 3] {
        let age = self.age(now);
        let p = self.spawn.position;
        let mut target = [p.x, p.y, p.z];
        if self.spawn.npc && !self.spawn.is_corpse {
            // MoveToCommand sends zero velocity deltas: animation is its actual
            // speed. CalculateHeadingToTarget is clockwise from +Y (north).
            let angle = p.heading * std::f32::consts::TAU / 512.;
            let distance = p.animation as f32 * 0.4 * 1.45 * age.min(PREDICTION_SECONDS);
            target[0] += angle.sin() * distance;
            target[1] += angle.cos() * distance;
        }
        let remaining = 1. - (age / CORRECTION_SECONDS).min(1.);
        std::array::from_fn(|i| target[i] + self.correction[i] * remaining)
    }

    /// NPC movement packets describe horizontal speed but omit the ramp's
    /// vertical velocity. Follow nearby connected floors between corrections,
    /// preserving the server's anchor height above the floor. This changes only
    /// presentation; authoritative positions and outgoing packets stay intact.
    pub fn position_on_terrain(
        &self,
        now: Instant,
        world: &CollisionWorld,
        dynamic: Option<&CollisionWorld>,
    ) -> [f32; 3] {
        let mut target = self.position(now);
        if !self.spawn.npc || self.spawn.is_corpse || !matches!(self.spawn.fly_mode, 0 | 3) {
            return target;
        }
        let p = self.spawn.position;
        if target == [p.x, p.y, p.z] {
            return target;
        }
        let ground = |x, y, z, rise, drop| {
            std::iter::once(world)
                .chain(dynamic)
                .filter_map(|world| world.ground_height(x, y, z, rise, drop))
                .min_by(|a, b| (a - z).abs().total_cmp(&(b - z).abs()))
        };
        let max_offset = self.spawn.size.max(6.) * 1.5;
        let Some(mut floor) = ground(p.x, p.y, p.z, 0.5, max_offset) else {
            return target;
        };
        let offset = p.z - floor;
        let distance = (target[0] - p.x).hypot(target[1] - p.y);
        if !distance.is_finite() || distance > 256. {
            return target;
        }
        let steps = (distance / 2.).ceil().max(1.) as usize;
        let reach = distance / steps as f32 + 0.75;
        // Short contiguous probes preserve stacked floors and do not drag a
        // guard from a bridge down to distant ground across a prediction gap.
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let Some(next) = ground(
                p.x + (target[0] - p.x) * t,
                p.y + (target[1] - p.y) * t,
                floor,
                reach,
                reach,
            ) else {
                return target;
            };
            floor = next;
        }
        target[2] = floor + offset;
        target
    }

    pub fn heading(&self, now: Instant) -> f32 {
        let age = self.age(now);
        let p = self.spawn.position;
        let turn = if self.spawn.npc {
            p.delta_heading * 19. * age.min(PREDICTION_SECONDS)
        } else {
            0.
        };
        (p.heading + turn + self.heading_correction * (1. - (age / CORRECTION_SECONDS).min(1.)))
            .rem_euclid(512.)
    }

    fn moving(&self, now: Instant) -> bool {
        if self.spawn.npc {
            self.spawn.position.animation != 0 && self.age(now) < PREDICTION_SECONDS
        } else {
            self.moving_until > now
                || (self.spawn.position.animation != 0 && self.age(now) < PREDICTION_SECONDS)
        }
    }

    /// Returns whether the server's authoritative position changed.
    fn update(&mut self, position: Position, now: Instant) -> bool {
        let previous = self.spawn.position;
        let moved = (previous.x - position.x).abs()
            + (previous.y - position.y).abs()
            + (previous.z - position.z).abs()
            > 0.01;
        let displayed = self.position(now);
        let correction: [f32; 3] =
            std::array::from_fn(|i| displayed[i] - [position.x, position.y, position.z][i]);
        let teleport =
            correction.iter().map(|v| v * v).sum::<f32>() > TELEPORT_DISTANCE * TELEPORT_DISTANCE;
        self.correction = if teleport { [0.; 3] } else { correction };
        self.heading_correction = if teleport {
            0.
        } else {
            (self.heading(now) - position.heading + 256.).rem_euclid(512.) - 256.
        };
        if moved {
            self.moving_until = now + std::time::Duration::from_millis(500);
        }
        self.spawn.position = position;
        self.arrived = now;
        moved
    }
}

struct PendingZoneRequest {
    started: Instant,
    heading: f32,
}

#[derive(Clone, Copy)]
struct MovementUpdate {
    id: u32,
    revision: u64,
    position: Position,
}

/// Shared by foreground and worker: a camera pose belongs to one authoritative
/// player/arrival revision. A queued old pose cannot move a new spawn or corpse.
#[derive(Default)]
struct MovementAuthority {
    own_id: Option<u32>,
    corpse_id: Option<u32>,
    revision: u64,
    suspended: bool,
    awaiting_spawn: bool,
}

impl MovementAuthority {
    fn suspend(&mut self) {
        if self.corpse_id.is_none() {
            self.corpse_id = self.own_id;
        }
        self.own_id = None;
        self.revision = self.revision.wrapping_add(1);
        self.suspended = true;
        self.awaiting_spawn = true;
    }

    fn gameplay(&mut self, event: &GameplayEvent, current_zone: Option<(u16, u16)>) {
        match event {
            GameplayEvent::Death(death) if self.own_id == Some(death.id) => {
                self.corpse_id = self.own_id.take();
                self.suspend();
            }
            GameplayEvent::Recovery(
                openeq_net::death::DeathEvent::RespawnWindow(_)
                | openeq_net::death::DeathEvent::BindTransfer(_),
            ) => {
                self.suspend();
            }
            GameplayEvent::ZoneTransition { .. } => {
                self.own_id = None;
                self.corpse_id = None; // Entity IDs may be reused in another zone.
                self.suspend();
            }
            GameplayEvent::ZoneChangeRequested(destination) => {
                self.revision = self.revision.wrapping_add(1);
                self.suspended = self.awaiting_spawn
                    || current_zone != Some((destination.zone_id, destination.instance_id));
            }
            GameplayEvent::ZoneChangeResult { success: 1, .. } => {
                // A cancelled boundary can rewind the player in the same zone.
                self.revision = self.revision.wrapping_add(1);
            }
            GameplayEvent::Recovery(openeq_net::death::DeathEvent::ResurrectionOffer(_)) => {
                // A queued reply to an older offer must fail even if the UI
                // has not yet consumed the replacement offer from the worker.
                self.revision = self.revision.wrapping_add(1);
            }
            _ => {}
        }
    }

    fn spawn(&mut self, spawn: &Spawn, character: &str) -> bool {
        if !is_own_spawn(spawn, character)
            || self.corpse_id == Some(spawn.id)
            || (self.suspended && !self.awaiting_spawn && self.own_id == Some(spawn.id))
        {
            return false;
        }
        self.own_id = Some(spawn.id);
        self.revision = self.revision.wrapping_add(1);
        self.suspended = false;
        self.awaiting_spawn = false;
        true
    }

    fn position(&self, update: Option<MovementUpdate>) -> Option<(u32, Position)> {
        let update = update?;
        (!self.suspended && self.own_id == Some(update.id) && self.revision == update.revision)
            .then_some((update.id, update.position))
    }
}

pub struct LiveWorld {
    pub entities: BTreeMap<u32, Entity>,
    pub environment: Option<Environment>,
    pub own_id: Option<u32>,
    pub initial_position: Option<Position>,
    pub character: String,
    pub ready: bool,
    /// Advances even when the server transfers us back to the same zone.
    pub zone_generation: u64,
    pub error: Option<String>,
    pub moves: u64,
    pub hour: u8,
    pub minute: u8,
    pub target: Option<u32>,
    pub game: GameplayState,
    pub doors: BTreeMap<u8, Door>,
    pub zone_points: BTreeMap<u32, ZonePoint>,
    pub combat_feedback: crate::combat_feedback::CombatFeedback,
    pub spell_effects: crate::spell_effects::SpellEffects,
    zone_request: Option<PendingZoneRequest>,
    door_return_deadlines: BTreeMap<u8, Instant>,
    pending_destination: Option<ZoneDestination>,
    rx: Mutex<mpsc::Receiver<Message>>,
    movement: tokio::sync::watch::Sender<Option<MovementUpdate>>,
    movement_authority: MovementAuthority,
    recovery_request: Option<crate::death::RecoveryRequest>,
    commands: tokio::sync::mpsc::UnboundedSender<NetworkCommand>,
}

fn is_own_spawn(spawn: &Spawn, character: &str) -> bool {
    !spawn.npc && !spawn.is_corpse && spawn.name.eq_ignore_ascii_case(character)
}

fn command_while_dead(command: &Command) -> bool {
    matches!(
        command,
        Command::Chat { .. }
            | Command::Death(_)
            | Command::AutoAttack(false)
            | Command::InterruptSpell
            | Command::MerchantClose
            | Command::EndLoot(_)
            | Command::Trade(openeq_net::trade::TradeCommand::Cancel { .. })
    )
}

pub(crate) struct NetworkIo {
    tx: mpsc::Sender<Message>,
    updates: tokio::sync::watch::Receiver<Option<MovementUpdate>>,
    requests: tokio::sync::mpsc::UnboundedReceiver<NetworkCommand>,
}
impl NetworkIo {
    pub(crate) fn fail(self, error: anyhow::Error) {
        let _ = self.tx.send(Message::Error(format!("{error:#}")));
    }
    pub(crate) async fn run(self, mut zone: openeq_net::zone::ZoneClient, character: &str) {
        let Self {
            tx,
            mut updates,
            mut requests,
        } = self;
        let result = async {
                let mut motion = MovementAuthority::default();
                let mut raid_generation = 0u64;
                let mut heartbeat = tokio::time::interval(std::time::Duration::from_millis(100));
                loop {
                    tokio::select! {
                        Some(request) = requests.recv() => {
                            match request {
                                NetworkCommand::Raid { request, motion_revision, raid_generation: requested_generation } => {
                                    if motion_revision != motion.revision || motion.suspended || requested_generation != raid_generation {
                                        let _ = tx.send(Message::RaidRejected { token: request.token, notice: "That raid choice is no longer current.".into() });
                                        continue;
                                    }
                                    match zone.command(Command::Raid(request.command)).await {
                                        Ok(()) => { let _ = tx.send(Message::RaidSent(request.token)); }
                                        Err(openeq_net::zone::ZoneError::Malformed(what)) => {
                                            let _ = tx.send(Message::RaidRejected { token: request.token, notice: format!("Invalid {what}; raid request was not sent.") });
                                        }
                                        Err(openeq_net::zone::ZoneError::Zoning) => {
                                            let _ = tx.send(Message::RaidRejected { token: request.token, notice: "Zone travel has already started.".into() });
                                        }
                                        Err(error) => return Err(error.into()),
                                    }
                                }
                                NetworkCommand::Recovery { request, motion_revision } => {
                                    if motion_revision != motion.revision {
                                        let _ = tx.send(Message::RecoveryRejected { token: request.token, notice: "That recovery choice is no longer current.".into() });
                                        continue;
                                    }
                                    let accepting = matches!(&request.command, openeq_net::death::DeathCommand::AnswerResurrection { accept: true, .. });
                                    match zone.command(Command::Death(request.command)).await {
                                        Ok(()) => {
                                            if accepting { motion.suspended = true; }
                                            let _ = tx.send(Message::RecoverySent(request.token));
                                        }
                                        Err(openeq_net::zone::ZoneError::Malformed(what)) => {
                                            let _ = tx.send(Message::RecoveryRejected { token: request.token, notice: format!("Invalid {what}; recovery was not sent.") });
                                        }
                                        Err(openeq_net::zone::ZoneError::Zoning) => {
                                            let _ = tx.send(Message::RecoveryRejected { token: request.token, notice: "Zone travel has already started.".into() });
                                        }
                                        Err(error) => return Err(error.into()),
                                    }
                                }
                                NetworkCommand::Target(id) => { if !zone.is_zoning() && !motion.suspended { zone.target(id).await?; } },
                                NetworkCommand::Gameplay(command) => {
                                    if motion.suspended && !command_while_dead(&command) {
                                        let _ = tx.send(Message::CommandRejected { command, notice: "Wait for your character to recover before doing that.".into() });
                                        continue;
                                    }
                                    let mut wire_command = command.clone();
                                    if let Command::ZoneChange { position, .. } = &mut wire_command {
                                        *position = coordinates::scene_point_to_server(*position);
                                        // The server validates borders against its last player
                                        // position, not the destination in the zone request.
                                        let current = motion.position(*updates.borrow());
                                        if !zone.is_zoning() && let Some((id, current)) = current {
                                            zone.send_position(id, coordinates::scene_to_server(current)).await?;
                                        }
                                    }
                                    match zone.command(wire_command).await {
                                        Ok(()) => {
                                            if matches!(&command, Command::Death(openeq_net::death::DeathCommand::AnswerResurrection { accept: true, .. })) {
                                                motion.suspended = true;
                                            }
                                            let _ = tx.send(Message::CommandSent(command));
                                        }
                                        Err(openeq_net::zone::ZoneError::Malformed(what)) => {
                                            let _ = tx.send(Message::CommandRejected { command, notice: format!("Invalid {what}; action was not sent.") });
                                        }
                                        Err(openeq_net::zone::ZoneError::Zoning) => {
                                            let _ = tx.send(Message::CommandRejected { command, notice: "Please wait for zone travel to finish.".into() });
                                        }
                                        Err(error) => return Err(error.into()),
                                    }
                                }
                            }
                        },
                        changed = updates.changed() => {
                            if changed.is_err() { logout_zone(&mut zone).await?; break; }
                        },
                        _ = heartbeat.tick() => {
                            let position = motion.position(*updates.borrow());
                            if let Some((id, position)) = position { zone.send_position(id, coordinates::scene_to_server(position)).await?; }
                        }
                        event = zone.next_event() => {
                            let event = event?;
                            if let ZoneEvent::Gameplay(event) = &event {
                                motion.gameplay(event, zone.current_zone());
                                if let GameplayEvent::Raid(event) = event && crate::raid::changes_membership(event) {
                                    raid_generation = raid_generation.wrapping_add(1);
                                }
                            }
                            if let ZoneEvent::Spawn(spawn) = &event { motion.spawn(spawn, character); }
                            if tx.send(Message::Event(Box::new(event))).is_err() { logout_zone(&mut zone).await?; break; }
                        }
                    }
                }
                Ok::<_, anyhow::Error>(())
        }.await;
        if let Err(error) = result {
            let _ = tx.send(Message::Error(format!("{error:#}")));
        }
    }
}

/// Entry already sent to EQEmu must reach ClientReady before Logout is valid.
/// This also covers dropping queued foreground state before its first poll.
pub(crate) async fn logout_zone(zone: &mut openeq_net::zone::ZoneClient) -> anyhow::Result<()> {
    tracing::info!(target:"openeq_net::account",ready=zone.is_ready(),"starting connection cleanup");
    if !zone.is_ready() {
        tokio::time::timeout(std::time::Duration::from_secs(20), async {
            while !zone.is_ready() {
                zone.next_event().await?;
            }
            Ok::<_, openeq_net::zone::ZoneError>(())
        })
        .await??;
    }
    zone.logout().await?;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    tracing::info!(target:"openeq_net::account","connection cleanup sent logout");
    Ok(())
}

impl LiveWorld {
    pub fn movement_allowed(&self) -> bool {
        self.ready
            && self.own_id.is_some()
            && self.error.is_none()
            && !self.zone_request_pending()
            && !self.game.recovery.blocks_movement()
            && !self.movement_authority.suspended
    }

    fn current_zone(&self) -> Option<(u16, u16)> {
        self.environment
            .as_ref()
            .map(|zone| (zone.zone_id, zone.instance_id))
    }
    pub fn player_gravity(&self) -> crate::movement_rules::PlayerGravity {
        let mode = self
            .own_id
            .and_then(|id| self.entities.get(&id))
            .map_or(0, |entity| entity.spawn.fly_mode);
        crate::movement_rules::PlayerGravity::from_wire_mode(mode)
            .with_levitation_buff(self.game.levitation_mode())
    }

    pub fn start(config: ConnectionConfig) -> Self {
        let (world, io) = Self::channels(
            config.character.clone(),
            config.host.eq_ignore_ascii_case("storage2.daeken.dev"),
        );
        std::thread::Builder::new()
            .name("eq-network".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("network runtime");
                rt.block_on(async move {
                    match config.connect().await {
                        Ok(zone) => io.run(zone, &config.character).await,
                        Err(error) => io.fail(error),
                    }
                });
            })
            .expect("network worker");
        world
    }

    /// Build foreground state without moving a socket between Tokio runtimes.
    /// Interactive selection delivers this state before serving the connected
    /// zone on the same worker/runtime that authenticated it.
    pub(crate) fn channels(character: String, shared_coin: bool) -> (Self, NetworkIo) {
        let (tx, rx) = mpsc::channel();
        let (movement, updates) = tokio::sync::watch::channel(None);
        let (commands, requests) = tokio::sync::mpsc::unbounded_channel();
        let mut game = GameplayState::default();
        game.commerce.shared_coin_enabled = shared_coin;
        let world = Self {
            entities: BTreeMap::new(),
            environment: None,
            own_id: None,
            initial_position: None,
            character,
            ready: false,
            zone_generation: 0,
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: None,
            game,
            doors: BTreeMap::new(),
            zone_points: BTreeMap::new(),
            combat_feedback: Default::default(),
            spell_effects: Default::default(),
            zone_request: None,
            door_return_deadlines: BTreeMap::new(),
            pending_destination: None,
            rx: Mutex::new(rx),
            movement,
            movement_authority: MovementAuthority::default(),
            recovery_request: None,
            commands,
        };
        (
            world,
            NetworkIo {
                tx,
                updates,
                requests,
            },
        )
    }

    pub fn poll(&mut self) {
        let now = Instant::now();
        let messages: Vec<_> = self.rx.lock().unwrap().try_iter().collect();
        for message in messages {
            match message {
                Message::Error(error) => {
                    tracing::error!(%error, "live connection failed");
                    self.error = Some(error);
                    self.ready = false;
                    self.game.recovery.disconnect();
                    self.recovery_request = None;
                    self.movement.send_replace(None);
                    self.zone_request = None;
                    self.combat_feedback.clear();
                    self.spell_effects.clear();
                    self.game.attack = false;
                    self.game.commerce.close_services();
                    self.game.trade = Default::default();
                    self.game.raid.begin_zone();
                    self.game.guild.begin_zone();
                    self.game.progression.begin_zone();
                    self.game.inventory.clear_trade();
                    self.item_use_reset();
                    self.game.inventory_command_pending = false;
                    self.game.error(format!(
                        "Connection lost: {}",
                        self.error.as_deref().unwrap_or("unknown error")
                    ));
                }
                Message::CommandRejected { command, notice } => {
                    self.command_rejected(command, notice)
                }
                Message::CommandSent(command) => self.command_sent(command),
                Message::RaidSent(token) => self.game.raid.sent(token, now),
                Message::RaidRejected { token, notice } => {
                    if self.game.raid.rejected(token) {
                        self.game.error(notice);
                    }
                }
                Message::RecoverySent(token) => {
                    self.game.recovery.request_sent(token);
                    if self
                        .recovery_request
                        .as_ref()
                        .is_some_and(|request| request.token == token)
                    {
                        self.recovery_request = None;
                    }
                }
                Message::RecoveryRejected { token, notice } => {
                    if self.game.recovery.request_pending(token) {
                        self.game.recovery.request_failed(token);
                        if !self.game.recovery.blocks_movement()
                            && !self.movement_authority.awaiting_spawn
                        {
                            self.movement_authority.suspended = false;
                        }
                        self.game.error(notice);
                    }
                    if self
                        .recovery_request
                        .as_ref()
                        .is_some_and(|request| request.token == token)
                    {
                        self.recovery_request = None;
                    }
                }
                Message::Event(event) => {
                    match *event {
                        ZoneEvent::Spawn(mut spawn) => {
                            spawn.position = coordinates::server_to_scene(spawn.position);
                            if self.movement_authority.corpse_id == Some(spawn.id)
                                && is_own_spawn(&spawn, &self.character)
                            {
                                continue;
                            }
                            if self.movement_authority.spawn(&spawn, &self.character) {
                                let recovering = self.game.recovery.blocks_movement();
                                self.game.recovery.own_spawn(self.zone_generation, spawn.id);
                                if recovering {
                                    self.zone_request = None;
                                }
                                self.own_id = Some(spawn.id);
                                self.movement.send_replace(None);
                                self.initial_position = Some(spawn.position);
                                if !self.game.buffs.is_empty() {
                                    self.spell_effects.event(
                                        &GameplayEvent::Buffs {
                                            id: spawn.id,
                                            all: true,
                                            tick_timer: 0,
                                            kind: 0,
                                            buffs: self.game.buffs.values().cloned().collect(),
                                        },
                                        &self.game.spell_catalog,
                                        self.own_id,
                                        now,
                                    );
                                }
                            }
                            self.entities.insert(spawn.id, Entity::new(spawn, now));
                        }
                        ZoneEvent::Movement { id, position } => {
                            let position = coordinates::server_to_scene(position);
                            if let Some(entity) = self.entities.get_mut(&id)
                                && entity.update(position, now)
                                && entity.spawn.npc
                            {
                                self.moves += 1;
                            }
                        }
                        ZoneEvent::Despawn(id) => {
                            self.trade_partner_gone(id);
                            self.entities.remove(&id);
                            self.spell_effects.remove_entity(id);
                            if self.target == Some(id) {
                                self.target = None;
                            }
                        }
                        ZoneEvent::Environment(mut environment) => {
                            environment.safe_position =
                                coordinates::server_point_to_scene(environment.safe_position);
                            tracing::info!(zone = %environment.short_name, "zone environment received");
                            self.environment = Some(environment);
                        }
                        ZoneEvent::Ready => {
                            self.ready = true;
                            tracing::info!(entities = self.entities.len(), "live zone ready");
                            self.game.notice("Connected. Enter opens chat; /help lists commands. I opens inventory.");
                        }
                        ZoneEvent::Time { hour, minute } => {
                            self.hour = hour;
                            self.minute = minute;
                        }
                        ZoneEvent::Hp { id, percent } => {
                            if let Some(e) = self.entities.get_mut(&id) {
                                e.spawn.hp_percent = percent;
                            }
                        }
                        ZoneEvent::Gameplay(event) => self.gameplay_event(event),
                        ZoneEvent::Other { .. } => {}
                    }
                }
            }
        }
        self.check_zone_request_timeout(now);
        self.game.raid.tick(now);
        self.combat_feedback.prune(now);
        self.advance_door_cycles(now);
    }

    fn advance_door_cycles(&mut self, now: Instant) {
        self.door_return_deadlines.retain(|id, deadline| {
            if now < *deadline {
                return true;
            }
            if let Some(door) = self.doors.get_mut(id) {
                door.state = 0;
            }
            false
        });
    }

    fn schedule_door_return(&mut self, id: u8) {
        self.door_return_deadlines.remove(&id);
        if self
            .doors
            .get(&id)
            .is_some_and(|door| matches!(door.open_type, 59 | 60) && door.state != 0)
        {
            // EQEmu silently resets ordinary lifts/buttons; RoF2 omits the
            // configured timer. Five seconds matches the default and PEQ's
            // Kelethin records. A fresh open restarts the local cycle.
            self.door_return_deadlines
                .insert(id, Instant::now() + std::time::Duration::from_secs(5));
        }
    }

    pub fn set_target(&mut self, id: Option<u32>) {
        self.target = id;
        let _ = self.commands.send(NetworkCommand::Target(id.unwrap_or(0)));
    }

    pub fn zone_request_pending(&self) -> bool {
        self.zone_request.is_some()
    }

    /// Use only a server-advertised destination for an authored boundary number.
    /// The server retains control over access checks and the final arrival pose.
    pub fn cross_zone_line(&mut self, number: u32, camera: &openeq_render::Camera) -> bool {
        if !self.movement_allowed() || self.own_id.is_none() {
            return false;
        }
        let Some(point) = self.zone_points.get(&number).cloned() else {
            self.game
                .notice("This zone exit is not available on the server.");
            return false;
        };
        // Same-zone teleports need separately verified authored semantics.
        if point.zone_id == 0
            || self
                .environment
                .as_ref()
                .is_none_or(|env| env.zone_id == point.zone_id)
        {
            return false;
        }
        self.camera_position(camera, false);
        if !self.command(Command::ZoneChange {
            character: self.character.clone(),
            zone_id: point.zone_id,
            instance_id: point.instance_id,
            position: [
                camera.position[0],
                camera.position[1],
                camera.position[2] - 3.,
            ],
            reason: 0,
        }) {
            return false;
        }
        self.zone_request = Some(PendingZoneRequest {
            started: Instant::now(),
            heading: camera.yaw.rem_euclid(std::f32::consts::TAU) * 512. / std::f32::consts::TAU,
        });
        true
    }

    fn check_zone_request_timeout(&mut self, now: Instant) {
        if self
            .zone_request
            .as_ref()
            .is_some_and(|request| now.saturating_duration_since(request.started).as_secs() >= 30)
        {
            self.zone_request = None;
            self.ready = false;
            self.movement.send_replace(None);
            self.error = Some("Zone travel timed out. Please reconnect.".into());
        }
    }

    pub fn player_position(&self) -> Option<[f32; 3]> {
        self.initial_position
            .or(self.movement.borrow().map(|update| update.position))
            .or_else(|| {
                self.own_id
                    .and_then(|id| self.entities.get(&id))
                    .map(|e| e.spawn.position)
            })
            .map(|p| [p.x, p.y, p.z])
    }

    pub fn service_available(&self, id: u32, class: u8) -> bool {
        self.movement_allowed()
            && self.error.is_none()
            && self.entities.get(&id).is_some_and(|entity| {
                entity.spawn.npc
                    && !entity.spawn.is_corpse
                    && entity.spawn.class == class
                    && self.player_position().is_some_and(|position| {
                        crate::commerce::in_range(position, entity.position(Instant::now()))
                    })
            })
    }

    pub fn recovery_action(&mut self, action: crate::death::RecoveryAction) -> bool {
        if !self.ready || self.error.is_some() {
            self.game.error("You are not connected to the zone.");
            return false;
        }
        let request = match self.game.recovery.act(action, Instant::now()) {
            Ok(Some(request)) => request,
            Ok(None) => return true,
            Err(error) => {
                self.game.error(error.to_string());
                return false;
            }
        };
        if !self.game.recovery.request_pending(request.token) {
            return false;
        }
        if self
            .commands
            .send(NetworkCommand::Recovery {
                request: request.clone(),
                motion_revision: self.movement_authority.revision,
            })
            .is_err()
        {
            self.game.recovery.request_failed(request.token);
            self.game
                .error("The connection closed before recovery was sent.");
            return false;
        }
        self.recovery_request = Some(request);
        if self.game.recovery.blocks_movement() {
            self.movement_authority.suspended = true;
            self.movement.send_replace(None);
        }
        true
    }

    pub(crate) fn queue_raid(&mut self, request: crate::raid::QueuedRequest) -> bool {
        if self
            .commands
            .send(NetworkCommand::Raid {
                request,
                motion_revision: self.movement_authority.revision,
                raid_generation: self.game.raid.generation,
            })
            .is_err()
        {
            self.game.error("The network worker has stopped.");
            return false;
        }
        true
    }

    pub fn command(&mut self, command: Command) -> bool {
        if !self.ready || self.error.is_some() {
            self.game.error("You are not connected to the zone.");
            return false;
        }
        if matches!(command, Command::Death(_)) {
            self.game
                .error("Use the current recovery choices to respond.");
            return false;
        }
        if matches!(command, Command::Raid(_)) {
            self.game
                .error("Use the current raid choices to send that request.");
            return false;
        }
        if !self.movement_allowed() && !command_while_dead(&command) {
            self.game
                .error("Wait for your character to recover before doing that.");
            return false;
        }
        if !self.trade_command_allowed(&command) || !self.item_use_command_allowed(&command) {
            return false;
        }
        if let Command::MoveItem { from, to, count } = &command {
            if (from.kind == 1 || from.kind == 2 || to.kind == 1 || to.kind == 2)
                && !self.game.commerce.bank.as_ref().is_some_and(|bank| {
                    self.service_available(bank.id, crate::commerce::BANKER_CLASS)
                })
            {
                self.game
                    .error("Open a nearby banker before moving bank items.");
                return false;
            }
            if self.game.inventory_command_pending || self.game.commerce.pending.is_some() {
                return false;
            }
            if let Err(error) = self.game.inventory.validate_move(*from, *to, *count) {
                self.game.error(error);
                return false;
            }
        }
        let mut pending = None;
        match &command {
            Command::MerchantOpen { merchant_id, .. } => {
                if self.game.commerce.merchant_closing {
                    self.game
                        .notice("Waiting for the previous merchant to close.");
                    return false;
                }
                if !self.service_available(*merchant_id, crate::commerce::MERCHANT_CLASS) {
                    self.game.error("The merchant is out of reach.");
                    return false;
                }
            }
            Command::MerchantClose if self.game.commerce.merchant_closing => return false,
            Command::MerchantBuy {
                merchant_id,
                slot,
                quantity,
                price,
                ..
            } => {
                let valid = self
                    .game
                    .commerce
                    .merchant
                    .as_ref()
                    .filter(|merchant| merchant.id == *merchant_id && merchant.opened)
                    .and_then(|merchant| merchant.items.get(slot))
                    .is_some_and(|item| {
                        *quantity > 0
                            && *quantity <= item.stack_size.max(1)
                            && (item.merchant_count < 0 || *quantity <= item.merchant_count as u32)
                            && item.price.checked_mul(*quantity) == Some(*price)
                    });
                if !valid
                    || !self.service_available(*merchant_id, crate::commerce::MERCHANT_CLASS)
                    || self.game.commerce.pending.is_some()
                    || self.game.commerce.coin_pending
                    || !self.game.commerce.currency_ready
                    || self.game.inventory_command_pending
                    || self
                        .game
                        .inventory
                        .items
                        .contains_key(&openeq_net::inventory::InventorySlot::CURSOR)
                    || crate::commerce::total_copper(self.game.currency) < u64::from(*price)
                {
                    self.game.error(
                        "Purchase unavailable: check quantity, money, cursor and merchant status.",
                    );
                    return false;
                }
                pending = Some(crate::commerce::PendingTransaction {
                    merchant_id: *merchant_id,
                    kind: crate::commerce::TransactionKind::Buy {
                        slot: *slot,
                        quantity: *quantity,
                    },
                    started: Instant::now(),
                });
            }
            Command::MerchantSell {
                merchant_id,
                slot,
                quantity,
            } => {
                let valid = slot.kind == 0
                    && self.game.inventory.items.get(slot).is_some_and(|item| {
                        !item.no_drop
                            && !item.attuned
                            && *quantity > 0
                            && *quantity <= item.count
                            && (item.bag_slots == 0
                                || !self.game.inventory.items.keys().any(|child| {
                                    child.kind == slot.kind
                                        && child.slot == slot.slot
                                        && child.bag.is_some()
                                }))
                    });
                if !valid
                    || !self.service_available(*merchant_id, crate::commerce::MERCHANT_CLASS)
                    || !self
                        .game
                        .commerce
                        .merchant
                        .as_ref()
                        .is_some_and(|merchant| merchant.id == *merchant_id && merchant.opened)
                    || self.game.commerce.pending.is_some()
                    || self.game.commerce.coin_pending
                    || !self.game.commerce.currency_ready
                    || self.game.inventory_command_pending
                {
                    self.game
                        .error("That item cannot be sold now. Empty bags before selling them.");
                    return false;
                }
                pending = Some(crate::commerce::PendingTransaction {
                    merchant_id: *merchant_id,
                    kind: crate::commerce::TransactionKind::Sell {
                        slot: *slot,
                        quantity: *quantity,
                    },
                    started: Instant::now(),
                });
            }
            Command::MoveCoin {
                from,
                to,
                coin,
                amount,
            } => {
                if !self.game.commerce.bank.as_ref().is_some_and(|bank| {
                    self.service_available(bank.id, crate::commerce::BANKER_CLASS)
                }) || self.game.commerce.coin_pending
                    || !self.game.commerce.currency_ready
                    || self.game.commerce.pending.is_some()
                {
                    self.game
                        .error("Open a nearby banker and finish the current transaction first.");
                    return false;
                }
                if let Err(error) =
                    self.game
                        .commerce
                        .validate_coin(self.game.currency, *from, *to, *coin, *amount)
                {
                    self.game.error(error);
                    return false;
                }
            }
            Command::BankerChange
                if !self.game.commerce.bank.as_ref().is_some_and(|bank| {
                    self.service_available(bank.id, crate::commerce::BANKER_CLASS)
                }) =>
            {
                self.game.error("Open a nearby banker first.");
                return false;
            }
            _ => {}
        }
        let coin_command = matches!(command, Command::MoveCoin { .. });
        let merchant_close = matches!(command, Command::MerchantClose);
        let inventory_command = matches!(command, Command::MoveItem { .. });
        if self
            .commands
            .send(NetworkCommand::Gameplay(command.clone()))
            .is_err()
        {
            self.game.error("The network worker has stopped.");
            return false;
        }
        self.trade_queued(&command);
        self.item_use_queued(&command);
        if inventory_command {
            self.game.inventory_command_pending = true;
        }
        if coin_command {
            self.game.commerce.coin_pending = true;
        }
        if merchant_close {
            self.game.commerce.merchant_closing = true;
        }
        if let Some(pending) = pending {
            self.game.commerce.pending = Some(pending);
        }
        true
    }

    pub(crate) fn command_sent(&mut self, command: Command) {
        let sent = command.clone();
        match command {
            Command::MoveCoin {
                from,
                to,
                coin,
                amount,
            } => {
                self.game.commerce.coin_pending = false;
                if let Err(error) =
                    self.game
                        .commerce
                        .move_coins(&mut self.game.currency, from, to, coin, amount)
                {
                    self.game.error(error);
                }
            }
            Command::MoveItem { from, to, count } => {
                self.game.inventory_command_pending = false;
                if let Err(error) = self.game.inventory.move_item(from, to, count) {
                    self.game.error(error);
                }
            }
            Command::Chat {
                channel: openeq_net::gameplay::ChatChannel::Raid,
                text,
                ..
            } => self.game.sent_raid_chat(&self.character, &text),
            Command::AutoAttack(active) => self.game.attack = active,
            Command::Posture { player_id, posture } => {
                self.game.sitting = posture == 1;
                if let Some(entity) = self.entities.get_mut(&player_id) {
                    entity.action = posture_action(posture);
                    entity.action_sequence = entity.action_sequence.wrapping_add(1);
                }
            }
            Command::EndLoot(_) => self.game.loot = None,
            _ => {}
        }
        self.trade_command_sent(&sent);
        self.item_use_command_sent(&sent);
    }

    pub(crate) fn command_rejected(&mut self, command: Command, notice: String) {
        self.trade_command_rejected(&command);
        self.item_use_command_rejected(&command);
        // The worker rejected this command before transmission. Release only
        // its pending state; another outstanding action may still be valid.
        match command {
            Command::MoveItem { .. } => self.game.inventory_command_pending = false,
            Command::MoveCoin { .. } => self.game.commerce.coin_pending = false,
            Command::MerchantClose => self.game.commerce.merchant_closing = false,
            Command::MerchantOpen { merchant_id, .. } => {
                if self
                    .game
                    .commerce
                    .merchant
                    .as_ref()
                    .is_some_and(|m| m.id == merchant_id && !m.opened)
                {
                    self.game.commerce.merchant = None;
                }
            }
            Command::MerchantBuy { merchant_id, .. }
            | Command::MerchantSell { merchant_id, .. } => {
                if self
                    .game
                    .commerce
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.merchant_id == merchant_id)
                {
                    self.game.commerce.pending = None;
                }
            }
            Command::CastSpell { .. } => self.game.cast_pending_until = None,
            _ => {}
        }
        if matches!(command, Command::ZoneChange { .. }) {
            self.zone_request = None;
        }
        self.game.error(notice);
    }

    pub(crate) fn gameplay_event(&mut self, mut event: GameplayEvent) {
        self.movement_authority
            .gameplay(&event, self.current_zone());
        self.trade_event(&event);
        self.item_use_event(&event);
        if let GameplayEvent::Damage(damage) = &event {
            self.combat_feedback
                .record_damage(damage, self.own_id, self.target, Instant::now());
        }
        // Network structs stay in EQEmu's coordinates. Everything retained by
        // LiveWorld uses the original assets' coordinate basis.
        match &mut event {
            GameplayEvent::ZoneChangeRequested(destination) => {
                destination.position = coordinates::server_point_to_scene(destination.position);
                destination.heading = coordinates::server_heading_to_scene(destination.heading);
            }
            GameplayEvent::ZoneChangeResult { position, .. } => {
                *position = coordinates::server_point_to_scene(*position);
            }
            GameplayEvent::ZonePoints(points) => {
                for point in points {
                    point.position = coordinates::server_point_to_scene(point.position);
                    if point.heading != 999. {
                        // Server sentinel: preserve current heading.
                        point.heading = coordinates::server_heading_to_scene(point.heading);
                    }
                }
            }
            GameplayEvent::Projectile(projectile) => {
                projectile.position = coordinates::server_point_to_scene(projectile.position);
                projectile.launch_angle =
                    coordinates::server_heading_to_scene(projectile.launch_angle);
            }
            GameplayEvent::Doors(doors) => {
                for door in doors {
                    door.position = coordinates::server_point_to_scene(door.position);
                    door.heading = coordinates::server_heading_to_scene(door.heading);
                    // Spawn state is inverted on the wire for inverted doors;
                    // movement actions below already produce logical open state.
                    door.state = u8::from((door.state != 0) ^ door.inverted);
                }
            }
            _ => {}
        }
        self.spell_effects.event(
            &event,
            &self.game.spell_catalog,
            self.own_id,
            Instant::now(),
        );
        match &event {
            GameplayEvent::ZoneTransition { zone_id, .. } => {
                self.zone_generation = self.zone_generation.wrapping_add(1);
                self.game.recovery.begin_zone(self.zone_generation);
                self.game.raid.begin_zone();
                self.game.guild.begin_zone();
                self.game.progression.begin_zone();
                self.recovery_request = None;
                self.ready = false;
                self.environment = None;
                self.zone_points.clear();
                self.zone_request = None;
                self.pending_destination = None;
                self.combat_feedback.clear();
                self.spell_effects.clear();
                self.entities.clear();
                self.doors.clear();
                self.door_return_deadlines.clear();
                self.own_id = None;
                self.target = None;
                self.initial_position = None;
                self.movement.send_replace(None);
                self.game.commerce = crate::commerce::CommerceState {
                    shared_coin_enabled: self.game.commerce.shared_coin_enabled,
                    ..Default::default()
                };
                self.game.inventory = Default::default();
                self.game.trade = Default::default();
                self.item_use_reset();
                self.game.loot = None;
                self.game.attack = false;
                self.game.sitting = false;
                self.game.inventory_command_pending = false;
                self.game.hp = Default::default();
                self.game.mana = Default::default();
                self.game.endurance = Default::default();
                self.game.casting = None;
                self.game.cast_pending_until = None;
                self.game.spell_cooldowns.clear();
                self.game.notice(format!("Traveling to zone {zone_id}…"));
            }
            GameplayEvent::ZoneChangeRequested(destination) => {
                if self.environment.as_ref().is_some_and(|environment| {
                    (environment.zone_id, environment.instance_id)
                        == (destination.zone_id, destination.instance_id)
                }) {
                    self.initial_position = Some(Position {
                        x: destination.position[0],
                        y: destination.position[1],
                        z: destination.position[2],
                        heading: destination.heading,
                        ..Default::default()
                    });
                    self.game.recovery.relocated(self.zone_generation);
                    self.movement.send_replace(None);
                }
                self.pending_destination = Some(destination.clone());
            }
            GameplayEvent::ZoneChangeResult {
                zone_id,
                instance_id,
                position,
                success,
                ..
            } => {
                let request = self.zone_request.take();
                if *success == 1
                    && !self.game.recovery.blocks_movement()
                    && self.environment.as_ref().is_some_and(|env| {
                        (env.zone_id, env.instance_id) == (*zone_id, *instance_id)
                    })
                {
                    self.initial_position = Some(Position {
                        x: position[0],
                        y: position[1],
                        z: position[2],
                        heading: request
                            .as_ref()
                            .map(|request| request.heading)
                            .or_else(|| {
                                self.pending_destination
                                    .as_ref()
                                    .map(|destination| destination.heading)
                            })
                            .unwrap_or(0.),
                        ..Default::default()
                    });
                    if request.is_some() {
                        self.game.notice("Zone travel was cancelled. Back away from the exit before trying again.");
                    }
                } else if *success != 1 {
                    if self.game.recovery.blocks_movement() {
                        let error = format!(
                            "Recovery transfer was rejected ({success}). Please reconnect."
                        );
                        self.ready = false;
                        self.error = Some(error.clone());
                        self.movement.send_replace(None);
                        self.game.error(error);
                    } else {
                        self.game
                            .error(format!("Zone travel was rejected ({success})."));
                    }
                }
            }
            GameplayEvent::ZonePoints(points) => {
                self.zone_points = points
                    .iter()
                    .map(|point| (point.number, point.clone()))
                    .collect();
            }
            GameplayEvent::Doors(doors) => {
                self.doors = doors.iter().map(|door| (door.id, door.clone())).collect();
                self.door_return_deadlines.clear();
                for id in doors.iter().map(|door| door.id) {
                    self.schedule_door_return(id);
                }
            }
            GameplayEvent::DoorMoved { id, action } => {
                if let Some(door) = self.doors.get_mut(id) {
                    match action {
                        2 => door.state = u8::from(!door.inverted),
                        3 => door.state = u8::from(door.inverted),
                        _ => {}
                    }
                }
                if matches!(action, 2 | 3) {
                    self.schedule_door_return(*id);
                }
            }
            GameplayEvent::Assist(id) => self.set_target(Some(*id)),
            GameplayEvent::BeginCast {
                caster_id,
                spell_id,
                ..
            } => {
                let action = self
                    .game
                    .spell_catalog
                    .spells
                    .get(spell_id)
                    .map_or(ActorAction::Cast, |spell| {
                        ActorAction::Animation(spell.casting_animation)
                    });
                if let Some(entity) = self.entities.get_mut(caster_id) {
                    entity.action = action;
                    entity.action_sequence = entity.action_sequence.wrapping_add(1);
                }
            }
            GameplayEvent::Animation { id, action, .. } => {
                if let Some(entity) = self.entities.get_mut(id) {
                    entity.action = ActorAction::Animation(*action);
                    entity.action_sequence = entity.action_sequence.wrapping_add(1);
                }
            }
            GameplayEvent::WearChange(change) => {
                if let Some(entity) = self.entities.get_mut(&change.id)
                    && let Some(slot) = entity
                        .spawn
                        .appearance
                        .equipment
                        .get_mut(change.slot as usize)
                {
                    *slot = change.appearance;
                }
            }
            GameplayEvent::Illusion(change) => {
                if let Some(entity) = self.entities.get_mut(&change.id) {
                    entity.spawn.race = u32::from(change.race);
                    entity.spawn.gender = change.gender;
                    if change.size > 0. {
                        entity.spawn.size = change.size;
                    }
                    let appearance = &mut entity.spawn.appearance;
                    // Player illusions use 255 to retain existing armor;
                    // subsequent wear packets carry equipment replacements.
                    if change.texture != u8::MAX {
                        appearance.texture = change.texture;
                    }
                    if change.helm_texture != u8::MAX {
                        appearance.helm_texture = change.helm_texture;
                    }
                    if let Ok(face) = u8::try_from(change.face) {
                        appearance.face = face;
                    }
                    appearance.hair_color = change.hair_color;
                    appearance.beard_color = change.beard_color;
                    appearance.hair_style = change.hair_style;
                    appearance.beard = change.beard;
                    appearance.drakkin_heritage = change.drakkin_heritage;
                    appearance.drakkin_tattoo = change.drakkin_tattoo;
                    appearance.drakkin_details = change.drakkin_details;
                    // RoF2's illusion encoder omits eye colors entirely.
                }
            }
            GameplayEvent::FaceChange(change) => {
                if let Some(entity) = self.entities.get_mut(&change.id) {
                    let appearance = &mut entity.spawn.appearance;
                    appearance.face = change.face;
                    appearance.hair_color = change.hair_color;
                    appearance.beard_color = change.beard_color;
                    appearance.eye_color_1 = change.eye_color_1;
                    appearance.eye_color_2 = change.eye_color_2;
                    appearance.hair_style = change.hair_style;
                    appearance.beard = change.beard;
                    appearance.drakkin_heritage = change.drakkin_heritage;
                    appearance.drakkin_tattoo = change.drakkin_tattoo;
                    appearance.drakkin_details = change.drakkin_details;
                }
            }
            GameplayEvent::SpawnAppearance {
                id,
                kind,
                parameter,
            } => {
                if let Some(entity) = self.entities.get_mut(id) {
                    match kind {
                        14 => {
                            entity.action = posture_action(*parameter);
                            entity.action_sequence = entity.action_sequence.wrapping_add(1);
                        }
                        29 => entity.spawn.size = *parameter as f32,
                        19 => entity.spawn.fly_mode = u8::try_from(*parameter).unwrap_or(u8::MAX),
                        1 => entity.spawn.level = (*parameter).min(255) as u8,
                        _ => {}
                    }
                }
            }
            GameplayEvent::Health {
                id,
                current,
                maximum,
            } => {
                if let Some(entity) = self.entities.get_mut(id)
                    && *maximum > 0
                {
                    entity.spawn.hp_percent =
                        (*current as f64 / *maximum as f64 * 100.).clamp(0., 100.) as u8;
                }
            }
            GameplayEvent::Death(death) => {
                if Some(death.id) == self.own_id {
                    self.game.recovery.own_death(self.zone_generation, death.id);
                    self.movement.send_replace(None);
                    self.initial_position = None;
                    self.zone_request = None;
                    self.pending_destination = None;
                    self.item_use_reset();
                    if self.game.trade.engaged() {
                        self.command(Command::Trade(openeq_net::trade::TradeCommand::Cancel {
                            player_id: death.id,
                        }));
                    }
                    self.game.attack = false;
                    self.game.casting = None;
                    self.game.cast_pending_until = None;
                    self.game.loot = None;
                    self.game.commerce.close_services();
                    self.game.trade = Default::default();
                    self.game.inventory.clear_trade();
                    self.game.inventory_command_pending = false;
                } else {
                    self.trade_partner_gone(death.id);
                }
                if self.game.attack
                    && (Some(death.id) == self.target || Some(death.id) == self.own_id)
                {
                    self.command(Command::AutoAttack(false));
                }
                if let Some(entity) = self.entities.get_mut(&death.id) {
                    entity.action = ActorAction::Dead;
                    entity.action_sequence = entity.action_sequence.wrapping_add(1);
                    entity.spawn.is_corpse = true;
                    entity.spawn.hp_percent = 0;
                    entity.spawn.position.animation = 0;
                }
            }
            GameplayEvent::Recovery(event) => {
                self.game.recovery.event(
                    self.zone_generation,
                    &self.character,
                    self.current_zone(),
                    event,
                    Instant::now(),
                );
                if matches!(
                    event,
                    openeq_net::death::DeathEvent::BindTransfer(_)
                        | openeq_net::death::DeathEvent::RespawnWindow(_)
                ) {
                    self.movement.send_replace(None);
                    self.initial_position = None;
                    self.game.attack = false;
                    self.game.casting = None;
                    self.game.cast_pending_until = None;
                    self.item_use_reset();
                    self.game.loot = None;
                    self.game.commerce.close_services();
                    self.game.trade = Default::default();
                    self.game.inventory.clear_trade();
                    self.game.inventory_command_pending = false;
                    if let openeq_net::death::DeathEvent::BindTransfer(bind) = event {
                        self.zone_request = Some(PendingZoneRequest {
                            started: Instant::now(),
                            heading: coordinates::server_heading_to_scene(bind.heading),
                        });
                    }
                }
            }
            _ => {}
        }
        let entities = &self.entities;
        self.game.apply(event, self.own_id, self.target, |id| {
            entities
                .get(&id)
                .map_or_else(|| format!("Entity {id}"), |e| display_name(&e.spawn.name))
        });
    }

    pub fn camera_position(&self, camera: &openeq_render::Camera, moving: bool) {
        if !self.movement_allowed() {
            return;
        }
        let Some(id) = self.own_id else {
            return;
        };
        self.movement.send_replace(Some(MovementUpdate {
            id,
            revision: self.movement_authority.revision,
            position: Position {
                x: camera.position[0],
                y: camera.position[1],
                z: camera.position[2] - 3.,
                heading: camera.yaw.rem_euclid(std::f32::consts::TAU) * 512.
                    / std::f32::consts::TAU,
                animation: if moving { 12 } else { 0 },
                ..Position::default()
            },
        }));
    }

    pub fn actors(&self, camera: [f32; 3]) -> Vec<ActorState> {
        self.actor_states(camera, None)
    }

    /// Prefer the rendered, terrain-adjusted skeleton. Unrendered entities use
    /// their network anchor; first-person casting hands use a camera-relative
    /// fallback because no player body is submitted in that view.
    pub fn effect_anchors(
        &self,
        camera: &openeq_render::Camera,
        actors: Option<&openeq_render::actors::ActorRenderer>,
    ) -> BTreeMap<u32, crate::spell_effects::EffectAnchor> {
        let now = Instant::now();
        self.entities
            .iter()
            .filter(|(_, entity)| !entity.spawn.is_corpse)
            .map(|(&id, entity)| {
                let sockets = actors.and_then(|actors| actors.sockets().get(&id)).copied();
                let mut position = entity.position(now);
                let mut heading = entity.heading(now);
                let sockets = if Some(id) == self.own_id && sockets.is_none() {
                    position = [
                        camera.position[0],
                        camera.position[1],
                        camera.position[2] - 3.,
                    ];
                    heading = camera.yaw * 512. / std::f32::consts::TAU;
                    let forward = [camera.yaw.sin(), camera.yaw.cos()];
                    let hand = |side: f32| {
                        [
                            position[0] + forward[0] * 2. + forward[1] * side,
                            position[1] + forward[1] * 2. - forward[0] * side,
                            position[2] + 1.5,
                        ]
                    };
                    openeq_render::actors::ActorSockets {
                        left_hand: Some(hand(-0.8)),
                        right_hand: Some(hand(0.8)),
                        ..Default::default()
                    }
                } else {
                    sockets.unwrap_or_default()
                };
                if let Some(chest) = sockets.chest {
                    position = chest;
                }
                (
                    id,
                    crate::spell_effects::EffectAnchor {
                        position,
                        heading,
                        size: entity.spawn.size,
                        sockets,
                    },
                )
            })
            .collect()
    }

    pub fn actor_states(
        &self,
        camera: [f32; 3],
        player: Option<(&openeq_render::Camera, bool)>,
    ) -> Vec<ActorState> {
        self.actor_states_with_terrain(camera, player, None)
    }

    pub fn actor_states_with_terrain(
        &self,
        camera: [f32; 3],
        player: Option<(&openeq_render::Camera, bool)>,
        terrain: Option<(&CollisionWorld, Option<&CollisionWorld>)>,
    ) -> Vec<ActorState> {
        let now = Instant::now();
        self.entities
            .values()
            .filter(|e| {
                (e.spawn.is_corpse || Some(e.spawn.id) != self.own_id || player.is_some())
                    && e.spawn.race != 127
                    && e.spawn.body_type < 66
            })
            .filter_map(|e| {
                let own = (!e.spawn.is_corpse && Some(e.spawn.id) == self.own_id)
                    .then_some(player)
                    .flatten();
                let p = own.map_or_else(
                    || {
                        terrain.map_or_else(
                            || e.position(now),
                            |(world, dynamic)| e.position_on_terrain(now, world, dynamic),
                        )
                    },
                    |(camera, _)| {
                        [
                            camera.position[0],
                            camera.position[1],
                            camera.position[2] - 3.,
                        ]
                    },
                );
                let distance: f32 = p.iter().zip(camera).map(|(a, b)| (a - b) * (a - b)).sum();
                (distance < 1500. * 1500.).then_some(ActorState {
                    id: e.spawn.id,
                    race: e.spawn.race,
                    gender: e.spawn.gender,
                    size: e.spawn.size,
                    position: p,
                    heading: own.map_or_else(
                        || e.heading(now),
                        |(camera, _)| camera.yaw * 512. / std::f32::consts::TAU,
                    ),
                    moving: own.map_or_else(|| e.moving(now), |(_, moving)| moving),
                    appearance: CharacterAppearance {
                        texture: e.spawn.appearance.texture,
                        helm_texture: e.spawn.appearance.helm_texture,
                        face: e.spawn.appearance.face,
                        hair_color: e.spawn.appearance.hair_color,
                        beard_color: e.spawn.appearance.beard_color,
                        eye_color_1: e.spawn.appearance.eye_color_1,
                        eye_color_2: e.spawn.appearance.eye_color_2,
                        hair_style: e.spawn.appearance.hair_style,
                        beard: e.spawn.appearance.beard,
                        drakkin_heritage: e.spawn.appearance.drakkin_heritage,
                        drakkin_tattoo: e.spawn.appearance.drakkin_tattoo,
                        drakkin_details: e.spawn.appearance.drakkin_details,
                        equipment: std::array::from_fn(|i| {
                            let value = e.spawn.appearance.equipment[i];
                            EquipmentAppearance {
                                material: value.material,
                                elite_material: value.elite_material,
                                hero_forge_model: value.hero_forge_model,
                                color: value.color,
                            }
                        }),
                    },
                    action: e.action,
                    action_sequence: e.action_sequence,
                })
            })
            .collect()
    }
}

fn posture_action(posture: u32) -> ActorAction {
    match posture {
        1 | 110 => ActorAction::Sit,
        2 | 111 => ActorAction::Duck,
        3 | 115 => ActorAction::Dead,
        _ => ActorAction::Auto,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::commerce::{BankSession, MerchantSession, PendingTransaction, TransactionKind};
    use openeq_net::{
        gameplay::{CoinLocation, CoinType, Currency},
        inventory::{InventoryItem, InventorySlot},
    };
    use std::time::Duration;

    pub(crate) fn command_world(
        class: u8,
        distance: f32,
    ) -> (
        LiveWorld,
        tokio::sync::mpsc::UnboundedReceiver<NetworkCommand>,
    ) {
        let (_, events) = mpsc::channel();
        let (movement, _) = tokio::sync::watch::channel(None);
        let (commands, received) = tokio::sync::mpsc::unbounded_channel();
        let mut service = npc(
            Position {
                x: distance,
                ..Default::default()
            },
            Instant::now(),
        );
        service.spawn.id = 2;
        service.spawn.class = class;
        let mut live = LiveWorld {
            entities: BTreeMap::from([(2, service)]),
            environment: None,
            own_id: Some(1),
            initial_position: Some(Position::default()),
            character: "Player".into(),
            ready: true,
            zone_generation: 0,
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: None,
            game: GameplayState::default(),
            doors: BTreeMap::new(),
            zone_points: BTreeMap::new(),
            combat_feedback: Default::default(),
            spell_effects: Default::default(),
            zone_request: None,
            door_return_deadlines: BTreeMap::new(),
            pending_destination: None,
            rx: Mutex::new(events),
            movement,
            movement_authority: MovementAuthority {
                own_id: Some(1),
                ..Default::default()
            },
            recovery_request: None,
            commands,
        };
        live.game.currency = Currency {
            platinum: 10,
            ..Default::default()
        };
        live.game.commerce.currency_ready = true;
        live.game.commerce.bank_money = live.game.currency;
        live.game.commerce.shared_platinum = 10;
        live.game.commerce.shared_coin_enabled = true;
        (live, received)
    }

    pub(crate) fn carried_item(slot: InventorySlot) -> InventoryItem {
        InventoryItem {
            slot,
            id: 13005,
            instance_id: 99,
            name: "Iron Ration".into(),
            lore: String::new(),
            id_file: String::new(),
            icon: 570,
            price: 20,
            merchant_count: -1,
            base_price: 20,
            no_drop: false,
            attuned: false,
            count: 5,
            charges: 0,
            stack_size: 20,
            item_class: 0,
            item_type: 0,
            equip_slots: 0,
            classes: u32::MAX,
            races: u32::MAX,
            material: 0,
            color: 0,
            damage: 0,
            delay: 0,
            ac: 0,
            hp: 0,
            mana: 0,
            endurance: 0,
            required_level: 0,
            click: Default::default(),
            scroll_spell_id: None,
            recast_timestamp: 0,
            bag_slots: 0,
            bag_size: 0,
            weight: 10,
            size: 1,
            children: Vec::new(),
        }
    }

    #[test]
    fn raid_chat_local_transcript_waits_for_successful_send() {
        use openeq_net::gameplay::ChatChannel;

        let (mut live, mut wire) = command_world(1, 10.);
        let chat = |channel, text: &str| Command::Chat {
            channel,
            target: String::new(),
            text: text.into(),
            language: 0,
        };
        assert!(live.command(chat(ChatChannel::Raid, "Ready here")));
        assert!(live.game.chat.is_empty());
        let NetworkCommand::Gameplay(sent) = wire.try_recv().unwrap() else {
            panic!("expected chat command");
        };
        live.command_sent(sent);
        assert_eq!(live.game.chat.len(), 1);
        assert_eq!(live.game.chat[0].text, "[Raid] Player: Ready here");

        assert!(live.command(chat(ChatChannel::Raid, "Not sent")));
        let NetworkCommand::Gameplay(rejected) = wire.try_recv().unwrap() else {
            panic!("expected chat command");
        };
        live.command_rejected(rejected, "Zone travel has already started.".into());
        assert_eq!(live.game.chat.len(), 2);
        assert_eq!(live.game.chat[1].text, "Zone travel has already started.");
        assert!(
            !live
                .game
                .chat
                .iter()
                .any(|line| line.text.contains("Not sent"))
        );

        // Other channels already receive server echoes and must not duplicate.
        live.command_sent(chat(ChatChannel::Say, "Server echoes this"));
        assert_eq!(live.game.chat.len(), 2);
    }

    fn zone_test_environment() -> Environment {
        Environment {
            short_name: "gfaydark".into(),
            long_name: "Greater Faydark".into(),
            zone_id: 54,
            instance_id: 0,
            fog_color: [[0.; 3]; 4],
            fog_start: [0.; 4],
            fog_end: [1000.; 4],
            fog_density: 0.,
            min_clip: 1.,
            max_clip: 1000.,
            sky: 1,
            zone_type: 1,
            safe_position: [0.; 3],
            gravity: 0.4,
            underworld: -3000.,
            underworld_teleport_index: 0,
            lava_damage: 50,
            min_lava_damage: 10,
            fall_damage_disabled: false,
            levitation_disabled: false,
        }
    }

    fn add_zone_test_point(live: &mut LiveWorld) {
        live.environment = Some(zone_test_environment());
        live.gameplay_event(GameplayEvent::ZonePoints(vec![ZonePoint {
            number: 4,
            zone_id: 58,
            instance_id: 0,
            position: [162., -660., 4.],
            heading: 999.,
        }]));
    }

    #[test]
    fn border_request_is_single_flight_and_server_rewind_preserves_heading() {
        let (mut live, mut wire) = command_world(1, 10.);
        add_zone_test_point(&mut live);
        let camera = openeq_render::Camera {
            position: [2616., -55., 22.],
            yaw: 1.1,
            ..Default::default()
        };
        assert_eq!(live.zone_points[&4].position, [-660., 162., 4.]);
        assert!(!live.cross_zone_line(99, &camera));
        assert!(wire.try_recv().is_err());
        assert!(live.cross_zone_line(4, &camera));
        assert!(!live.cross_zone_line(4, &camera));
        let NetworkCommand::Gameplay(Command::ZoneChange {
            zone_id,
            position,
            reason,
            ..
        }) = wire.try_recv().unwrap()
        else {
            panic!("expected zone request");
        };
        assert_eq!(zone_id, 58);
        assert_eq!(position, [2616., -55., 19.]);
        assert_eq!(reason, 0);
        assert_eq!(live.movement.borrow().unwrap().position.x, 2616.);
        assert_eq!(live.movement.borrow().unwrap().position.animation, 0);
        assert!(wire.try_recv().is_err());
        live.gameplay_event(GameplayEvent::ZoneChangeResult {
            zone_id: 54,
            instance_id: 0,
            position: [-55., 2608., 19.],
            success: 1,
        });
        assert!(!live.zone_request_pending());
        assert!(live.ready);
        let rewind = live.initial_position.unwrap();
        assert_eq!([rewind.x, rewind.y, rewind.z], [2608., -55., 19.]);
        assert!((rewind.heading - camera.yaw * 512. / std::f32::consts::TAU).abs() < 0.001);
    }

    #[test]
    fn rejected_and_timed_out_travel_cannot_stay_pending_or_retry() {
        let (mut live, mut wire) = command_world(1, 10.);
        add_zone_test_point(&mut live);
        assert!(live.cross_zone_line(4, &openeq_render::Camera::default()));
        let NetworkCommand::Gameplay(command) = wire.try_recv().unwrap() else {
            panic!("expected request");
        };
        live.command_rejected(command, "invalid test request".into());
        assert!(!live.zone_request_pending());
        assert!(live.cross_zone_line(4, &openeq_render::Camera::default()));
        wire.try_recv().unwrap();
        live.check_zone_request_timeout(Instant::now() + std::time::Duration::from_secs(31));
        assert!(!live.zone_request_pending());
        assert!(!live.ready);
        assert!(live.error.as_ref().unwrap().contains("timed out"));
        assert!(!live.cross_zone_line(4, &openeq_render::Camera::default()));
        assert!(live.movement.borrow().is_none());
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn live_damage_populates_feedback_and_zone_transition_clears_it() {
        let (mut live, _) = command_world(1, 10.);
        let camera = openeq_render::Camera {
            position: [0., -30., 8.],
            ..Default::default()
        };
        let actors = BTreeMap::from([(
            2,
            openeq_render::actors::ActorBounds {
                min: [-2., -2., 0.],
                max: [2., 2., 8.],
            },
        )]);
        live.gameplay_event(GameplayEvent::Damage(openeq_net::gameplay::Damage {
            source_id: 1,
            target_id: 2,
            amount: 17,
            skill: 1,
            spell_id: 0xffff,
            secondary: false,
            special: 0,
        }));
        let frame = |live: &LiveWorld| {
            let mut frame = openeq_ui::UiFrame::default();
            live.combat_feedback.append(
                &mut frame,
                &camera,
                [1280, 720],
                &actors,
                live.own_id,
                Instant::now(),
            );
            frame
        };
        assert!(
            frame(&live)
                .commands
                .iter()
                .any(|cmd| matches!(cmd,openeq_ui::DrawCommand::Text{text,..} if text=="17"))
        );
        live.gameplay_event(GameplayEvent::ZoneTransition {
            zone_id: 58,
            instance_id: 0,
        });
        assert!(frame(&live).commands.is_empty());
    }

    #[test]
    fn every_zone_transition_invalidates_the_loaded_destination() {
        let (mut live, _) = command_world(1, 10.);
        let environment = Environment {
            short_name: "gfaydark".into(),
            long_name: "Greater Faydark".into(),
            zone_id: 54,
            instance_id: 0,
            fog_color: [[0.; 3]; 4],
            fog_start: [0.; 4],
            fog_end: [1000.; 4],
            fog_density: 0.,
            min_clip: 1.,
            max_clip: 1000.,
            sky: 1,
            zone_type: 1,
            safe_position: [0.; 3],
            gravity: 0.4,
            underworld: -3000.,
            underworld_teleport_index: 0,
            lava_damage: 50,
            min_lava_damage: 10,
            fall_damage_disabled: false,
            levitation_disabled: false,
        };
        live.environment = Some(environment.clone());
        live.camera_position(&openeq_render::Camera::default(), true);
        for generation in 1..=2 {
            live.gameplay_event(GameplayEvent::ZoneTransition {
                zone_id: 54,
                instance_id: 0,
            });
            assert_eq!(live.zone_generation, generation);
            assert!(!live.ready);
            assert!(live.environment.is_none());
            assert!(live.movement.borrow().is_none());
            assert!(live.initial_position.is_none());
            // Re-entering the same zone must also require a new presentation.
            live.environment = Some(environment.clone());
            live.ready = true;
        }
    }

    #[test]
    fn trade_offers_predict_once_and_remote_views_do_not_replace_worn_items() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::{TradeCommand, TradeEvent};
        let (mut live, mut wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        live.game
            .inventory
            .insert(carried_item(InventorySlot::possessions(0)));
        let offer = Command::Trade(TradeCommand::OfferCoin {
            coin: CoinType::Platinum,
            amount: 3,
        });
        assert!(live.command(offer.clone()));
        assert_eq!(
            live.game.currency.platinum, 10,
            "enqueue is not transmission"
        );
        assert!(
            !live.command(offer.clone()),
            "pending offer cannot be double submitted"
        );
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(_)))
        ));
        live.command_sent(offer);
        assert_eq!(live.game.currency.platinum, 7);
        assert_eq!(
            live.game.trade.session.as_ref().unwrap().own_money.platinum,
            3
        );
        let accept = Command::Trade(TradeCommand::Accept { player_id: 1 });
        assert!(live.command(accept.clone()));
        live.command_sent(accept);
        assert!(
            !live.command(Command::Trade(TradeCommand::OfferCoin {
                coin: CoinType::Platinum,
                amount: 1
            })),
            "peer may already have completed the exchange"
        );
        assert!(!live.command(Command::MoveItem {
            from: InventorySlot::possessions(0),
            to: InventorySlot::CURSOR,
            count: 0
        }));
        assert!(!live.command(Command::DeleteItem {
            slot: InventorySlot::possessions(0),
            count: 1
        }));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Accepted { player_id: 2 }));
        assert_eq!(
            live.game.trade.session.as_ref().unwrap().phase,
            TradePhase::Completing
        );
        let mut remote = carried_item(InventorySlot::possessions(0));
        remote.id = 999;
        live.gameplay_event(GameplayEvent::Item {
            packet_type: 0x65,
            item: remote,
        });
        let session = live.game.trade.session.as_ref().unwrap();
        assert!(!session.you_accepted && !session.partner_accepted);
        assert_eq!(session.phase, TradePhase::Active);
        assert_eq!(session.partner_items[&0].id, 999);
        assert_eq!(
            live.game.inventory.items[&InventorySlot::possessions(0)].id,
            13005
        );
        assert!(!live.command(Command::DeleteItem {
            slot: InventorySlot::trade(0),
            count: 1
        }));
        assert!(!live.command(Command::MerchantOpen {
            merchant_id: 2,
            player_id: 1
        }));
    }

    #[test]
    fn trade_cancel_waits_for_refunds_and_late_peer_reply_without_ping_pong() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::{TradeCommand, TradeEvent};
        let (mut live, mut wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        live.game
            .inventory
            .insert(carried_item(InventorySlot::trade(0)));
        assert!(live.command(Command::Trade(TradeCommand::Cancel { player_id: 1 })));
        assert!(wire.try_recv().is_ok());
        live.gameplay_event(GameplayEvent::Currency(Currency {
            platinum: 10,
            ..Default::default()
        }));
        live.gameplay_event(GameplayEvent::Item {
            packet_type: 0x67,
            item: carried_item(InventorySlot::possessions(23)),
        });
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed));
        assert!(
            live.game
                .inventory
                .items
                .contains_key(&InventorySlot::trade(0))
        );
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed2));
        assert!(
            !live
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::trade(0))
        );
        assert!(
            live.game
                .inventory
                .items
                .contains_key(&InventorySlot::possessions(23))
        );
        assert!(
            live.game.trade.engaged(),
            "old untagged reciprocal cancel must drain first"
        );
        assert!(!live.command(Command::Trade(TradeCommand::Request {
            to_id: 2,
            from_id: 1
        })));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Cancelled {
            player_id: 1,
            action: 0,
        }));
        assert_eq!(
            live.game.trade.session.as_ref().unwrap().phase,
            TradePhase::Ended
        );
        assert!(
            wire.try_recv().is_err(),
            "reciprocal cancel never sends another reply"
        );
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Cancelled {
            player_id: 1,
            action: 0,
        }));
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn trade_remote_cancel_replies_once_and_disconnect_cannot_leave_escrow_locked() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::{TradeCommand, TradeEvent};
        let (mut live, mut wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        for _ in 0..2 {
            live.gameplay_event(GameplayEvent::Trade(TradeEvent::Cancelled {
                player_id: 1,
                action: 0,
            }));
        }
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Cancel { player_id: 1 }
            )))
        ));
        assert!(wire.try_recv().is_err());
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed2));
        assert!(!live.game.trade.engaged());
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        live.trade_partner_gone(2);
        assert!(wire.try_recv().is_ok());
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed2));
        assert!(
            !live.game.trade.engaged(),
            "gone peer cannot send a reciprocal cancel"
        );
    }

    #[test]
    fn queued_trade_ack_must_cancel_server_escrow_even_before_sent_callback() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::TradeCommand;
        let (mut live, mut wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        live.game.trade.session = Some(TradeSession::new(
            2,
            "Partner".into(),
            TradePhase::Invitation,
        ));
        let ack = Command::Trade(TradeCommand::Acknowledge {
            to_id: 2,
            from_id: 1,
        });
        assert!(live.command(ack.clone()));
        let mut ui = crate::interaction::Interaction::default();
        ui.trade_cancel(&mut live);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Acknowledge { .. }
            )))
        ));
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Cancel { .. }
            )))
        ));
        live.command_sent(ack);
        let session = live.game.trade.session.as_ref().unwrap();
        assert_eq!(session.phase, TradePhase::Completing);
        assert!(session.cancel_sent);
    }

    #[test]
    fn withdrawn_invitation_waits_for_peer_and_late_ack_requires_another_close() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::{TradeCommand, TradeEvent};
        let (mut live, mut wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Waiting));
        assert!(live.command(Command::Trade(TradeCommand::Cancel { player_id: 1 })));
        assert!(wire.try_recv().is_ok());
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Busy { .. }
            )))
        ));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed2));
        assert!(
            live.game.trade.engaged(),
            "local close does not drain an in-flight peer ACK"
        );
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Acknowledged {
            to_id: 1,
            from_id: 2,
        }));
        assert!(!live.game.trade.desynchronized);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Cancel { .. }
            )))
        ));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Cancelled {
            player_id: 1,
            action: 0,
        }));
        assert!(
            live.game.trade.engaged(),
            "old close cannot finish new server escrow"
        );
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed2));
        assert!(!live.game.trade.engaged());
    }

    #[test]
    fn withdrawing_invitation_replies_busy_once_and_unexpected_ack_freezes_state() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::{TradeCommand, TradeEvent};
        let (mut live, mut wire) = command_world(1, 10.);
        live.game.trade.session = Some(TradeSession::new(
            2,
            "Partner".into(),
            TradePhase::Invitation,
        ));
        let busy = GameplayEvent::Trade(TradeEvent::Busy {
            to_id: 1,
            from_id: 2,
        });
        live.gameplay_event(busy.clone());
        live.gameplay_event(busy);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Busy { .. }
            )))
        ));
        assert!(wire.try_recv().is_err());
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Acknowledged {
            to_id: 1,
            from_id: 2,
        }));
        assert!(
            live.game.trade.desynchronized,
            "even a same-partner duplicate ACK resets server escrow"
        );
        assert!(!live.game.commerce.currency_ready);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Trade(
                TradeCommand::Cancel { .. }
            )))
        ));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Cancelled {
            player_id: 1,
            action: 0,
        }));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::WindowClosed2));
        assert!(
            live.game.trade.engaged(),
            "desynchronized ownership needs a fresh connection"
        );
        assert!(!live.command(Command::Trade(TradeCommand::Request {
            to_id: 2,
            from_id: 1
        })));
        assert!(!live.command(Command::MoveItem {
            from: InventorySlot::possessions(23),
            to: InventorySlot::CURSOR,
            count: 0
        }));
    }

    #[test]
    fn trade_money_cannot_exceed_server_signed_credit_limit() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::TradeCommand;
        let (mut live, _wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        let mut session = TradeSession::new(2, "Partner".into(), TradePhase::Active);
        session.own_money.platinum = i32::MAX as u32;
        live.game.trade.session = Some(session);
        assert!(!live.command(Command::Trade(TradeCommand::OfferCoin {
            coin: CoinType::Platinum,
            amount: 1
        })));
        let session = live.game.trade.session.as_mut().unwrap();
        session.own_money.platinum = 0;
        session.partner_money.platinum = i32::MAX as u32;
        assert!(!live.command(Command::Trade(TradeCommand::Accept { player_id: 1 })));
    }

    #[test]
    fn unsolicited_trade_view_is_never_an_owned_item_and_finished_is_neutral() {
        use crate::trade::{TradePhase, TradeSession};
        use openeq_net::trade::TradeEvent;
        let (mut live, _) = command_world(1, 10.);
        live.gameplay_event(GameplayEvent::Item {
            packet_type: 0x65,
            item: carried_item(InventorySlot::possessions(0)),
        });
        assert!(live.game.inventory.items.is_empty());
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        live.game
            .inventory
            .insert(carried_item(InventorySlot::trade(0)));
        live.gameplay_event(GameplayEvent::Trade(TradeEvent::Finished));
        let session = live.game.trade.session.as_ref().unwrap();
        assert_eq!(session.phase, TradePhase::Ended);
        assert_eq!(
            session.status,
            "Trade ended. Check your inventory and chat for the result."
        );
        assert!(live.game.inventory.items.is_empty());
    }

    #[test]
    fn live_events_use_asset_coordinates_for_entities_doors_and_zone_corrections() {
        let (mut live, _) = command_world(crate::commerce::BANKER_CLASS, 10.);
        let (events, rx) = mpsc::channel();
        live.rx = Mutex::new(rx);
        let server = Position {
            x: 944.,
            y: -305.,
            z: -93.625,
            heading: 32.,
            delta_heading: 2.,
            velocity: [1., 3., 0.],
            ..Default::default()
        };
        let mut spawn = npc(server, Instant::now()).spawn;
        spawn.name = "Player".into();
        spawn.npc = false;
        events
            .send(Message::Event(Box::new(ZoneEvent::Spawn(spawn))))
            .unwrap();
        events
            .send(Message::Event(Box::new(ZoneEvent::Environment(
                Environment {
                    short_name: "poknowledge".into(),
                    long_name: "Plane of Knowledge".into(),
                    zone_id: 202,
                    instance_id: 0,
                    fog_color: [[0.; 3]; 4],
                    fog_start: [0.; 4],
                    fog_end: [1000.; 4],
                    fog_density: 0.,
                    min_clip: 1.,
                    max_clip: 1000.,
                    sky: 1,
                    zone_type: 0,
                    safe_position: [944., -305., -90.],
                    gravity: 0.4,
                    underworld: -3000.,
                    underworld_teleport_index: 0,
                    lava_damage: 50,
                    min_lava_damage: 10,
                    fall_damage_disabled: false,
                    levitation_disabled: false,
                },
            ))))
            .unwrap();
        live.poll();
        assert_eq!(live.player_position().unwrap(), [-305., 944., -93.625]);
        assert_eq!(
            live.environment.as_ref().unwrap().safe_position,
            [-305., 944., -90.]
        );
        let pose = live.entities[&1].spawn.position;
        assert_eq!(pose.heading, 96.);
        assert_eq!(pose.delta_heading, -2.);
        assert_eq!(pose.velocity, [3., 1., 0.]);
        events
            .send(Message::Event(Box::new(ZoneEvent::Movement {
                id: 1,
                position: Position {
                    x: 940.,
                    y: -310.,
                    ..server
                },
            })))
            .unwrap();
        live.poll();
        assert_eq!(live.entities[&1].spawn.position.x, -310.);
        assert_eq!(live.entities[&1].spawn.position.y, 940.);
        live.gameplay_event(GameplayEvent::Doors(vec![Door {
            id: 9,
            name: "POKDOOR500".into(),
            position: [950., -320., -96.],
            heading: 64.,
            incline: 0,
            size: 100,
            open_type: 0,
            state: 0,
            inverted: false,
            parameter: 0,
        }]));
        assert_eq!(live.doors[&9].position, [-320., 950., -96.]);
        assert_eq!(live.doors[&9].heading, 64.);
        live.gameplay_event(GameplayEvent::ZoneChangeRequested(ZoneDestination {
            zone_id: 202,
            instance_id: 0,
            position: [930., -300., -90.],
            heading: 256.,
        }));
        assert_eq!(live.player_position().unwrap(), [-300., 930., -90.]);
        assert_eq!(live.initial_position.unwrap().heading, 384.);
        live.gameplay_event(GameplayEvent::ZoneChangeResult {
            zone_id: 202,
            instance_id: 0,
            position: [935., -301., -91.],
            success: 1,
        });
        assert_eq!(live.player_position().unwrap(), [-301., 935., -91.]);
        assert_eq!(live.initial_position.unwrap().heading, 384.);
    }

    #[test]
    fn inverted_kelethin_lift_starts_down_then_opens_and_closes() {
        let (mut live, mut sent) = command_world(crate::commerce::BANKER_CLASS, 10.);
        live.gameplay_event(GameplayEvent::Doors(vec![Door {
            id: 69,
            name: "FAYLEVATOR".into(),
            position: [350.014, 137.463, 2.1582],
            heading: 384.,
            incline: 0,
            size: 100,
            open_type: 59,
            state: 1,
            inverted: true,
            parameter: 68,
        }]));
        assert_eq!(live.doors[&69].state, 0);
        live.gameplay_event(GameplayEvent::DoorMoved { id: 69, action: 3 });
        assert_eq!(live.doors[&69].state, 1);
        let deadline = live.door_return_deadlines[&69];
        live.advance_door_cycles(deadline - Duration::from_millis(1));
        assert_eq!(live.doors[&69].state, 1);
        live.advance_door_cycles(deadline);
        assert_eq!(
            live.doors[&69].state, 0,
            "silent server reset must return the lift"
        );
        assert!(
            sent.try_recv().is_err(),
            "local return must not send a door click"
        );
        live.gameplay_event(GameplayEvent::DoorMoved { id: 69, action: 3 });
        assert_eq!(live.doors[&69].state, 1);
        // Another OPEN is a fresh cycle even if its state value is unchanged.
        live.door_return_deadlines
            .insert(69, Instant::now() - Duration::from_secs(1));
        live.gameplay_event(GameplayEvent::DoorMoved { id: 69, action: 3 });
        assert!(live.door_return_deadlines[&69] > Instant::now());
        live.gameplay_event(GameplayEvent::DoorMoved { id: 69, action: 2 });
        assert_eq!(live.doors[&69].state, 0);
        assert!(live.door_return_deadlines.is_empty());
        live.gameplay_event(GameplayEvent::DoorMoved { id: 69, action: 3 });
        live.gameplay_event(GameplayEvent::Doors(vec![]));
        assert!(live.door_return_deadlines.is_empty());
    }

    #[test]
    fn bank_commands_require_an_open_banker_still_within_reach() {
        for (opened, distance, allowed) in
            [(false, 10., false), (true, 201., false), (true, 10., true)]
        {
            for destination in [CoinLocation::Bank, CoinLocation::SharedBank] {
                for (from, to) in [
                    (CoinLocation::Carried, destination),
                    (destination, CoinLocation::Carried),
                ] {
                    let (mut live, mut received) =
                        command_world(crate::commerce::BANKER_CLASS, distance);
                    live.game.commerce.bank = opened.then(|| BankSession {
                        id: 2,
                        name: "Banker".into(),
                    });
                    assert_eq!(
                        live.command(Command::MoveCoin {
                            from,
                            to,
                            coin: CoinType::Platinum,
                            amount: 2
                        }),
                        allowed
                    );
                    assert_eq!(live.game.commerce.coin_pending, allowed);
                    assert_eq!(
                        live.game.currency.platinum, 10,
                        "queueing must not change the carried balance"
                    );
                    assert_eq!(live.game.commerce.bank_money.platinum, 10);
                    if allowed {
                        assert!(
                            matches!(received.try_recv().unwrap(), NetworkCommand::Gameplay(Command::MoveCoin { from: queued_from, to: queued_to, coin: CoinType::Platinum, amount: 2 }) if queued_from == from && queued_to == to)
                        );
                    }
                    assert!(
                        received.try_recv().is_err(),
                        "rejected or duplicate command reached the worker"
                    );
                }
            }
            for bank_slot in [InventorySlot::bank(0), InventorySlot::shared_bank(0)] {
                let carried = InventorySlot::possessions(23);
                for (from, to) in [(carried, bank_slot), (bank_slot, carried)] {
                    let (mut live, mut received) =
                        command_world(crate::commerce::BANKER_CLASS, distance);
                    live.game.commerce.bank = opened.then(|| BankSession {
                        id: 2,
                        name: "Banker".into(),
                    });
                    live.game.inventory.items.insert(from, carried_item(from));
                    assert_eq!(
                        live.command(Command::MoveItem { from, to, count: 2 }),
                        allowed
                    );
                    assert_eq!(live.game.inventory_command_pending, allowed);
                    assert_eq!(
                        live.game.inventory.items[&from].count, 5,
                        "queueing must not move items"
                    );
                    assert!(!live.game.inventory.items.contains_key(&to));
                    if allowed {
                        assert!(
                            matches!(received.try_recv().unwrap(), NetworkCommand::Gameplay(Command::MoveItem { from: queued_from, to: queued_to, count: 2 }) if queued_from == from && queued_to == to)
                        );
                    }
                    assert!(
                        received.try_recv().is_err(),
                        "rejected or duplicate item move reached the worker"
                    );
                }
            }
        }
    }

    #[test]
    fn pending_merchant_transaction_blocks_even_valid_inventory_moves() {
        let from = InventorySlot::possessions(23);
        for kind in [
            TransactionKind::Buy {
                slot: 7,
                quantity: 1,
            },
            TransactionKind::Sell {
                slot: from,
                quantity: 1,
            },
        ] {
            let (mut live, mut received) = command_world(crate::commerce::MERCHANT_CLASS, 10.);
            live.game.inventory.items.insert(from, carried_item(from));
            live.game.commerce.pending = Some(PendingTransaction {
                merchant_id: 2,
                kind,
                started: Instant::now(),
            });
            let command = Command::MoveItem {
                from,
                to: InventorySlot::CURSOR,
                count: 2,
            };
            assert!(!live.command(command.clone()));
            assert!(received.try_recv().is_err());
            assert!(!live.game.inventory_command_pending);
            assert_eq!(live.game.inventory.items[&from].count, 5);
            live.game.commerce.pending = None;
            assert!(live.command(command));
            assert!(
                matches!(received.try_recv().unwrap(), NetworkCommand::Gameplay(Command::MoveItem { from: queued_from, to: InventorySlot::CURSOR, count: 2 }) if queued_from == from)
            );
            assert!(received.try_recv().is_err());
        }
    }

    #[test]
    fn merchant_close_blocks_reopen_and_duplicate_close_until_server_acknowledges() {
        let (mut live, mut received) = command_world(crate::commerce::MERCHANT_CLASS, 10.);
        let mut merchant = MerchantSession::new(2, "Merchant".into());
        merchant.opened = true;
        live.game.commerce.merchant = Some(merchant);
        let reopen = Command::MerchantOpen {
            merchant_id: 2,
            player_id: 1,
        };
        assert!(live.command(Command::MerchantClose));
        assert!(live.game.commerce.merchant_closing);
        live.game.commerce.merchant = None; // The window closes immediately.
        assert!(!live.command(Command::MerchantClose));
        assert!(!live.command(reopen.clone()));
        assert!(matches!(
            received.try_recv().unwrap(),
            NetworkCommand::Gameplay(Command::MerchantClose)
        ));
        assert!(received.try_recv().is_err());
        live.command_sent(Command::MerchantClose);
        assert!(
            live.game.commerce.merchant_closing,
            "sending the close is not its server acknowledgment"
        );
        assert!(!live.command(reopen.clone()));
        assert!(received.try_recv().is_err());
        live.gameplay_event(GameplayEvent::MerchantClosed);
        assert!(!live.game.commerce.merchant_closing);
        assert!(live.command(reopen));
        assert!(matches!(
            received.try_recv().unwrap(),
            NetworkCommand::Gameplay(Command::MerchantOpen {
                merchant_id: 2,
                player_id: 1
            })
        ));
        live.game.commerce.merchant = Some(MerchantSession::new(2, "Merchant".into()));
        live.gameplay_event(GameplayEvent::MerchantClosed);
        assert!(
            live.game
                .commerce
                .merchant
                .as_ref()
                .is_some_and(|merchant| merchant.id == 2 && !merchant.opened),
            "a trailing close reply must preserve the provisional new session"
        );
        assert!(received.try_recv().is_err());
    }

    #[test]
    fn failed_merchant_close_enqueue_does_not_start_close_barrier() {
        let (mut live, received) = command_world(crate::commerce::MERCHANT_CLASS, 10.);
        drop(received);
        assert!(!live.command(Command::MerchantClose));
        assert!(!live.game.commerce.merchant_closing);
    }

    #[test]
    fn same_name_corpse_does_not_replace_new_player_identity() {
        let (mut live, _) = command_world(1, 10.);
        let (events, rx) = mpsc::channel();
        live.rx = Mutex::new(rx);
        live.own_id = None;
        live.initial_position = None;
        let mut corpse = npc(Position::default(), Instant::now()).spawn;
        corpse.id = 41;
        corpse.name = "Player".into();
        corpse.is_corpse = true;
        events
            .send(Message::Event(Box::new(ZoneEvent::Spawn(corpse.clone()))))
            .unwrap();
        live.poll();
        assert_eq!(live.own_id, None);
        assert!(live.initial_position.is_none());

        let mut player = corpse.clone();
        player.id = 42;
        player.npc = false;
        player.is_corpse = false;
        player.name = "pLaYeR".into();
        player.position.x = 123.;
        player.position.y = -456.;
        assert!(is_own_spawn(&player, "Player"));
        events
            .send(Message::Event(Box::new(ZoneEvent::Spawn(player))))
            .unwrap();
        live.poll();
        assert_eq!(live.own_id, Some(42));
        let position = live.initial_position.take().unwrap();
        assert_eq!([position.x, position.y], [-456., 123.]);

        // A later corpse refresh must not reclaim the network movement ID or
        // teleport the camera back to the death location.
        events
            .send(Message::Event(Box::new(ZoneEvent::Spawn(corpse.clone()))))
            .unwrap();
        live.poll();
        assert_eq!(live.own_id, Some(42));
        assert!(live.initial_position.is_none());
        assert!(live.entities[&41].spawn.is_corpse);
        assert!(!live.entities[&42].spawn.is_corpse);
        corpse.is_corpse = false;
        assert!(
            !is_own_spawn(&corpse, "Player"),
            "same-name NPC is not our character"
        );
    }

    fn npc(position: Position, now: Instant) -> Entity {
        Entity::new(
            Spawn {
                id: 1,
                name: "Patroller".into(),
                last_name: String::new(),
                level: 1,
                class: 1,
                race: 1,
                gender: 0,
                npc: true,
                size: 6.,
                hp_percent: 100,
                walk_speed: 0.7,
                run_speed: 1.25,
                body_type: 1,
                position,
                appearance: Default::default(),
                is_corpse: false,
                stand_state: 100,
                fly_mode: 3,
            },
            now,
        )
    }

    #[test]
    fn facial_updates_reach_rendering_and_illusions_preserve_omitted_fields() {
        use openeq_net::gameplay::{FaceChange, Illusion};
        let (mut live, _) = command_world(1, 10.);
        let old = &mut live.entities.get_mut(&2).unwrap().spawn.appearance;
        old.texture = 3;
        old.helm_texture = 2;
        old.equipment[7].material = 1;
        live.gameplay_event(GameplayEvent::FaceChange(FaceChange {
            id: 2,
            face: 6,
            hair_color: 1,
            beard_color: 2,
            eye_color_1: 3,
            eye_color_2: 4,
            hair_style: 5,
            beard: 6,
            drakkin_heritage: 4,
            drakkin_tattoo: 7,
            drakkin_details: 3,
        }));
        live.gameplay_event(GameplayEvent::Illusion(Illusion {
            id: 2,
            race: 522,
            gender: 1,
            size: 7.,
            texture: 255,
            helm_texture: 255,
            face: u32::MAX,
            hair_style: 2,
            hair_color: 8,
            beard: 1,
            beard_color: 9,
            drakkin_heritage: 2,
            drakkin_tattoo: 5,
            drakkin_details: 6,
        }));
        let states = live.actor_states([0.; 3], None);
        let state = states.iter().find(|s| s.id == 2).unwrap();
        assert_eq!((state.race, state.gender, state.size), (522, 1, 7.));
        let a = state.appearance;
        assert_eq!((a.face, a.texture, a.helm_texture), (6, 3, 2));
        assert_eq!((a.eye_color_1, a.eye_color_2), (3, 4));
        assert_eq!(
            (a.hair_style, a.hair_color, a.beard, a.beard_color),
            (2, 8, 1, 9)
        );
        assert_eq!(
            (a.drakkin_heritage, a.drakkin_tattoo, a.drakkin_details),
            (2, 5, 6)
        );
        assert_eq!(a.equipment[7].material, 1);
    }
    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
    }

    fn terrain(quads: &[[[f32; 3]; 4]]) -> CollisionWorld {
        use openeq_assets::{Scene, mesh::Geometry};
        let mut geometry = Geometry {
            vertices: vec![],
            indices: vec![],
            material: 0,
            collidable: true,
        };
        for quad in quads {
            let offset = geometry.vertices.len() as u32 / 8;
            for p in quad {
                geometry
                    .vertices
                    .extend([p[0], p[1], p[2], 0., 0., 1., 0., 0.]);
            }
            geometry
                .indices
                .extend([0, 1, 2, 0, 2, 3].map(|i| i + offset));
        }
        CollisionWorld::build(&Scene::from_geometry(
            "terrain".into(),
            vec![],
            vec![geometry],
            vec![],
        ))
    }

    #[test]
    fn npc_height_follows_ramps_between_zero_velocity_packets() {
        let world = terrain(&[[
            [-100., -20., -50.],
            [100., -20., 50.],
            [100., 20., 50.],
            [-100., 20., -50.],
        ]]);
        let now = Instant::now();
        let mut guard = npc(
            Position {
                z: 3.75,
                heading: 128.,
                animation: 20,
                ..Default::default()
            },
            now,
        );
        for tenth in 0..=50 {
            let time = now + Duration::from_millis(tenth * 100);
            let p = guard.position_on_terrain(time, &world, None);
            close(p[2], p[0] * 0.5 + 3.75);
        }
        // The protocol position remains authoritative. The next five-second
        // correction must not cause a vertical jump in the displayed guard.
        let time = now + Duration::from_secs(5);
        let before = guard.position_on_terrain(time, &world, None);
        close(guard.position(time)[2], 3.75);
        guard.update(
            Position {
                x: 58.,
                z: 32.75,
                heading: 128.,
                animation: 20,
                ..Default::default()
            },
            time,
        );
        let after = guard.position_on_terrain(time, &world, None);
        for axis in 0..3 {
            close(before[axis], after[axis]);
        }
        let later = guard.position_on_terrain(time + Duration::from_millis(500), &world, None);
        close(later[2], later[0] * 0.5 + 3.75);
        let stop_time = time + Duration::from_millis(500);
        guard.update(
            Position {
                x: later[0],
                z: later[2],
                heading: 128.,
                ..Default::default()
            },
            stop_time,
        );
        close(
            guard.position_on_terrain(stop_time, &world, None)[2],
            later[2],
        );
    }

    #[test]
    fn npc_terrain_projection_preserves_flight_and_stacked_floor_gaps() {
        let ramp = terrain(&[[
            [-100., -20., -50.],
            [100., -20., 50.],
            [100., 20., 50.],
            [-100., 20., -50.],
        ]]);
        let now = Instant::now();
        let mut actor = npc(
            Position {
                z: 3.75,
                heading: 128.,
                animation: 20,
                ..Default::default()
            },
            now,
        );
        let time = now + Duration::from_secs(1);
        for mode in [1, 2, 4, 5, 255] {
            actor.spawn.fly_mode = mode;
            assert_eq!(
                actor.position_on_terrain(time, &ramp, None),
                actor.position(time)
            );
        }
        actor.spawn.fly_mode = 3;
        actor.spawn.is_corpse = true;
        assert_eq!(
            actor.position_on_terrain(time, &ramp, None),
            actor.position(time)
        );
        actor.spawn.is_corpse = false;
        let bridge = terrain(&[
            [
                [-20., -20., 0.],
                [5., -20., 0.],
                [5., 20., 0.],
                [-20., 20., 0.],
            ],
            [
                [-100., -20., -70.],
                [100., -20., -70.],
                [100., 20., -70.],
                [-100., 20., -70.],
            ],
        ]);
        assert_eq!(
            actor.position_on_terrain(time, &bridge, None),
            actor.position(time)
        );
    }

    #[test]
    fn appearance_events_change_npc_gravity_mode() {
        let (mut live, _) = command_world(1, 10.);
        for mode in [1, 3, 255, 256] {
            live.gameplay_event(GameplayEvent::SpawnAppearance {
                id: 2,
                kind: 19,
                parameter: mode,
            });
            assert_eq!(live.entities[&2].spawn.fly_mode, mode.min(255) as u8);
        }
    }

    fn player_death(id: u32) -> GameplayEvent {
        GameplayEvent::Death(openeq_net::gameplay::Death {
            id,
            killer_id: 99,
            corpse_id: id,
            skill: 0,
            spell_id: u32::MAX,
            damage: 100,
        })
    }

    fn bind_transfer(zone_id: u16, instance_id: u16) -> GameplayEvent {
        GameplayEvent::Recovery(openeq_net::death::DeathEvent::BindTransfer(
            openeq_net::death::BindTransfer {
                zone_id,
                instance_id,
                position: [1455., 15., -131.],
                heading: 64.,
                label: "Bind Location".into(),
                save_items: 1,
                resources: [0; 3],
            },
        ))
    }

    #[test]
    fn worker_death_invalidates_populated_and_late_camera_poses_until_a_new_owner() {
        let mut authority = MovementAuthority::default();
        let mut player = npc(Position::default(), Instant::now()).spawn;
        player.npc = false;
        player.name = "Player".into();
        player.id = 10;
        assert!(authority.spawn(&player, "Player"));
        let old = MovementUpdate {
            id: 10,
            revision: authority.revision,
            position: Position::default(),
        };
        assert!(authority.position(Some(old)).is_some());
        authority.gameplay(&player_death(20), Some((202, 0)));
        assert!(authority.position(Some(old)).is_some()); // Unrelated NPC.
        authority.gameplay(&player_death(10), Some((202, 0)));
        let death_revision = authority.revision;
        assert!(authority.position(Some(old)).is_none());
        authority.gameplay(&player_death(10), Some((202, 0)));
        assert_eq!(authority.revision, death_revision);
        player.is_corpse = true;
        assert!(!authority.spawn(&player, "Player"));
        player.is_corpse = false;
        assert!(!authority.spawn(&player, "Player")); // Stale old living spawn.
        authority.gameplay(&bind_transfer(202, 0), Some((202, 0)));
        player.id = 11;
        assert!(authority.spawn(&player, "Player"));
        assert!(authority.position(Some(old)).is_none());
        let wrong_revision = MovementUpdate { id: 11, ..old };
        assert!(authority.position(Some(wrong_revision)).is_none());
        let fresh = MovementUpdate {
            id: 11,
            revision: authority.revision,
            ..old
        };
        assert!(authority.position(Some(fresh)).is_some());
        authority.gameplay(
            &GameplayEvent::ZoneTransition {
                zone_id: 202,
                instance_id: 1,
            },
            Some((202, 0)),
        );
        assert!(authority.position(Some(fresh)).is_none());
        player.id = 10; // IDs are scoped to a zone, and can be reused on re-entry.
        assert!(authority.spawn(&player, "Player"));
    }

    #[test]
    fn local_hover_revival_waits_for_new_living_spawn_and_preserves_corpse_and_items() {
        let (mut live, mut commands) = command_world(1, 10.);
        let (tx, rx) = mpsc::channel();
        live.rx = Mutex::new(rx);
        live.environment = Some(zone_test_environment());
        let mut player = npc(Position::default(), Instant::now()).spawn;
        player.npc = false;
        player.name = "Player".into();
        player.id = 1;
        live.entities
            .insert(1, Entity::new(player.clone(), Instant::now()));
        live.game
            .inventory
            .insert(carried_item(InventorySlot::possessions(23)));
        live.game.hp.current = Some(123);
        live.camera_position(&openeq_render::Camera::default(), true);
        assert!(live.movement.borrow().is_some());
        live.gameplay_event(player_death(1));
        assert!(!live.movement_allowed());
        assert!(live.movement.borrow().is_none());
        assert!(live.entities[&1].spawn.is_corpse);
        live.camera_position(&openeq_render::Camera::default(), true);
        assert!(live.movement.borrow().is_none());
        assert!(!live.command(Command::AutoAttack(true)));
        assert!(commands.try_recv().is_err());
        live.gameplay_event(bind_transfer(54, 0));
        assert!(live.initial_position.is_none());
        assert_eq!(live.game.hp.current, Some(123)); // Zero footer is not HP.
        let mut corpse = player.clone();
        corpse.is_corpse = true;
        tx.send(Message::Event(Box::new(ZoneEvent::Spawn(corpse))))
            .unwrap();
        live.poll();
        assert!(!live.movement_allowed());
        player.id = 3;
        player.position = Position {
            x: 1455.,
            y: 15.,
            z: -130.25,
            heading: 32.,
            ..Default::default()
        };
        tx.send(Message::Event(Box::new(ZoneEvent::Spawn(player))))
            .unwrap();
        live.poll();
        assert!(live.movement_allowed()); // No full profile/Ready resend needed.
        assert_eq!(live.own_id, Some(3));
        let position = live.initial_position.unwrap();
        assert_eq!(
            [position.x, position.y, position.z, position.heading],
            [15., 1455., -130.25, 96.]
        );
        assert!(live.entities[&1].spawn.is_corpse);
        assert!(
            live.game
                .inventory
                .items
                .contains_key(&InventorySlot::possessions(23))
        );
        live.camera_position(&openeq_render::Camera::default(), false);
        assert_eq!(live.movement.borrow().unwrap().id, 3);
    }

    #[test]
    fn forced_bind_success_never_installs_the_zeroed_response_as_an_arrival() {
        let (mut live, _) = command_world(1, 10.);
        live.environment = Some(zone_test_environment());
        live.gameplay_event(player_death(1));
        live.gameplay_event(bind_transfer(0, 0));
        assert!(live.initial_position.is_none());
        // ZoneClient emits transition before the successful bind reply.
        live.gameplay_event(GameplayEvent::ZoneTransition {
            zone_id: 54,
            instance_id: 0,
        });
        live.gameplay_event(GameplayEvent::ZoneChangeResult {
            zone_id: 54,
            instance_id: 0,
            position: [0.; 3],
            success: 1,
        });
        assert_eq!(live.zone_generation, 1);
        assert!(!live.movement_allowed());
        assert!(live.initial_position.is_none());
        assert!(live.environment.is_none());
        assert!(live.movement.borrow().is_none());
    }

    #[test]
    fn own_corpse_renders_at_server_pose_in_first_and_third_person() {
        let (mut live, _) = command_world(1, 10.);
        let mut player = npc(
            Position {
                x: 25.,
                y: -40.,
                z: 10.,
                heading: 173.,
                ..Default::default()
            },
            Instant::now(),
        )
        .spawn;
        player.npc = false;
        player.name = "Player".into();
        player.id = 1;
        live.entities.insert(1, Entity::new(player, Instant::now()));
        let camera = openeq_render::Camera {
            position: [100., 200., 300.],
            yaw: 1.5,
            ..Default::default()
        };
        assert!(
            live.actor_states(camera.position, None)
                .iter()
                .all(|actor| actor.id != 1)
        );
        live.gameplay_event(player_death(1));
        assert_eq!(live.own_id, Some(1)); // Hover still awaits the new player ID.
        for player_view in [None, Some((&camera, true))] {
            let actors = live.actor_states(camera.position, player_view);
            let corpse = actors
                .iter()
                .find(|actor| actor.id == 1)
                .expect("visible own corpse");
            assert_eq!(corpse.position, [25., -40., 10.]);
            assert_eq!(corpse.heading, 173.);
            assert_eq!(corpse.action, ActorAction::Dead);
            assert!(!corpse.moving);
        }
    }

    #[test]
    fn recovery_actions_send_one_validated_command_and_only_matching_rejection_retries() {
        use crate::death::{RecoveryAction, RecoveryIntent};
        let (mut live, mut wire) = command_world(1, 10.);
        let (tx, rx) = mpsc::channel();
        live.rx = Mutex::new(rx);
        live.environment = Some(zone_test_environment());
        live.gameplay_event(player_death(1));
        live.gameplay_event(GameplayEvent::Recovery(
            openeq_net::death::DeathEvent::RespawnWindow(openeq_net::death::RespawnWindow {
                initial_selection: 17,
                remaining_ms: 60_000,
                options: vec![openeq_net::death::RespawnOption {
                    id: 17,
                    zone_id: 54,
                    position: [5., 6., 7.],
                    heading: 32.,
                    label: "Quest bind".into(),
                    requires_resurrection: false,
                }],
            }),
        ));
        let action = RecoveryAction {
            token: live.game.recovery.token(),
            intent: RecoveryIntent::Respawn(17),
        };
        assert!(live.recovery_action(action));
        let NetworkCommand::Recovery {
            request,
            motion_revision,
        } = wire.try_recv().unwrap()
        else {
            panic!("missing recovery command");
        };
        assert!(matches!(
            request.command,
            openeq_net::death::DeathCommand::SelectRespawn { option_id: 17 }
        ));
        assert_eq!(motion_revision, live.movement_authority.revision);
        assert!(!live.recovery_action(action));
        assert!(wire.try_recv().is_err());
        tx.send(Message::RecoveryRejected {
            token: request.token,
            notice: "Not transmitted".into(),
        })
        .unwrap();
        live.poll();
        assert!(!live.game.recovery.request_pending(request.token));
        assert!(!live.movement_allowed());
        assert!(live.recovery_action(RecoveryAction {
            token: live.game.recovery.token(),
            intent: RecoveryIntent::Respawn(17)
        }));
        let NetworkCommand::Recovery { request: next, .. } = wire.try_recv().unwrap() else {
            panic!("missing retry");
        };
        tx.send(Message::RecoveryRejected {
            token: request.token,
            notice: "Stale failure".into(),
        })
        .unwrap();
        live.poll();
        assert!(live.game.recovery.request_pending(next.token));
    }

    #[test]
    fn rejected_recovery_and_instance_changes_cannot_resume_dead_movement() {
        let (mut live, _) = command_world(1, 10.);
        live.environment = Some(zone_test_environment());
        live.gameplay_event(player_death(1));
        live.gameplay_event(bind_transfer(54, 9));
        live.gameplay_event(GameplayEvent::ZoneChangeRequested(ZoneDestination {
            zone_id: 54,
            instance_id: 9,
            position: [4., 5., 6.],
            heading: 32.,
        }));
        assert!(live.initial_position.is_none());
        live.gameplay_event(GameplayEvent::ZoneChangeResult {
            zone_id: 54,
            instance_id: 9,
            position: [0.; 3],
            success: -1,
        });
        assert!(!live.ready);
        assert!(!live.movement_allowed());
        assert!(live.error.as_deref().unwrap().contains("reconnect"));
        assert!(live.movement.borrow().is_none());
    }

    #[test]
    fn own_levitation_follows_server_buffs_and_explicit_appearance_changes() {
        use crate::movement_rules::PlayerGravity;
        let (mut live, _) = command_world(1, 10.);
        let mut player = npc(Position::default(), Instant::now());
        player.spawn.id = 1;
        player.spawn.npc = false;
        player.spawn.fly_mode = 3;
        live.entities.insert(1, player);
        let mut fields = vec!["0"; 174];
        fields[0] = "261";
        fields[1] = "Levitate";
        fields[173] = "3|57|1|0|100|0$4|14|1|0|100|0";
        live.game.spell_catalog = crate::spells::SpellCatalog::parse(&fields.join("^"));
        let buff = openeq_net::gameplay::Buff {
            slot: 0,
            spell_id: 261,
            ticks_remaining: 0,
            num_hits: 0,
            caster: "Caster".into(),
        };
        assert_eq!(live.player_gravity(), PlayerGravity::Grounded);
        live.gameplay_event(GameplayEvent::Buffs {
            id: 2,
            all: true,
            tick_timer: 0,
            kind: 0,
            buffs: vec![buff.clone()],
        });
        assert_eq!(live.player_gravity(), PlayerGravity::Grounded);
        live.gameplay_event(GameplayEvent::Buffs {
            id: 1,
            all: true,
            tick_timer: 0,
            kind: 0,
            buffs: vec![buff.clone()],
        });
        // An expired presentation countdown is not an authoritative buff fade.
        assert_eq!(live.game.buff_seconds(0), Some(0));
        assert_eq!(live.player_gravity(), PlayerGravity::Levitating);
        assert!(live.game.has_water_breathing_buff());
        live.gameplay_event(GameplayEvent::SpawnAppearance {
            id: 1,
            kind: 19,
            parameter: 1,
        });
        assert_eq!(live.player_gravity(), PlayerGravity::Flying);
        live.gameplay_event(GameplayEvent::SpawnAppearance {
            id: 1,
            kind: 19,
            parameter: 0,
        });
        live.gameplay_event(GameplayEvent::BuffChanged {
            id: 1,
            buff,
            removed: true,
        });
        assert_eq!(live.player_gravity(), PlayerGravity::Grounded);
        assert!(!live.game.has_water_breathing_buff());
    }

    #[test]
    #[ignore = "requires original Greater Faydark assets"]
    fn actual_kelethin_guard_climbs_the_ramp_without_height_corrections() {
        let world = CollisionWorld::build(
            &openeq_assets::loader::load_zone(
                openeq_assets::loader::default_client_dir().unwrap(),
                "gfaydark",
            )
            .unwrap(),
        );
        let start_floor = world.ground_height(137.463, 216., 0., 1., 5.).unwrap();
        let now = Instant::now();
        let mut guard = npc(
            Position {
                x: 137.463,
                y: 216.,
                z: start_floor + 3.125,
                heading: 0.,
                animation: 20,
                ..Default::default()
            },
            now,
        );
        guard.spawn.race = 112;
        guard.spawn.size = 5.;
        let mut last = guard.position(now);
        for tick in 1..=180 {
            let time = now + Duration::from_secs_f32(tick as f32 / 120.);
            let p = guard.position_on_terrain(time, &world, None);
            let floor = world
                .ground_height(p[0], p[1], p[2] - 3.125, 0.01, 0.01)
                .unwrap();
            close(p[2] - floor, 3.125);
            assert!(
                p[2] >= last[2] - 0.001 && p[2] - last[2] < 0.08,
                "ramp height jumped: {last:?} -> {p:?}"
            );
            last = p;
        }
        assert!(last[2] - guard.spawn.position.z > 6.);
    }

    #[test]
    fn target_death_stops_server_autoattack_before_another_target_is_selected() {
        let (_, events) = mpsc::channel();
        let (movement, _) = tokio::sync::watch::channel(None);
        let (commands, mut received) = tokio::sync::mpsc::unbounded_channel();
        let mut live = LiveWorld {
            entities: BTreeMap::new(),
            environment: None,
            own_id: Some(1),
            initial_position: None,
            character: "Player".into(),
            ready: true,
            zone_generation: 0,
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: Some(2),
            game: GameplayState::default(),
            doors: BTreeMap::new(),
            zone_points: BTreeMap::new(),
            combat_feedback: Default::default(),
            spell_effects: Default::default(),
            zone_request: None,
            door_return_deadlines: BTreeMap::new(),
            pending_destination: None,
            rx: Mutex::new(events),
            movement,
            movement_authority: MovementAuthority {
                own_id: Some(1),
                ..Default::default()
            },
            recovery_request: None,
            commands,
        };
        live.game.attack = true;
        let death = |id| {
            GameplayEvent::Death(openeq_net::gameplay::Death {
                id,
                killer_id: 1,
                corpse_id: id,
                skill: 0,
                spell_id: u32::MAX,
                damage: 12,
            })
        };
        // An unrelated nearby fight must not cancel our current combat.
        live.gameplay_event(death(3));
        assert!(received.try_recv().is_err());
        assert!(live.game.attack);
        live.gameplay_event(death(2));
        live.set_target(Some(4));
        assert!(matches!(
            received.try_recv().unwrap(),
            NetworkCommand::Gameplay(Command::AutoAttack(false))
        ));
        assert!(matches!(
            received.try_recv().unwrap(),
            NetworkCommand::Target(4)
        ));
        assert!(!live.game.attack);
    }

    #[test]
    fn sparse_patrol_updates_continue_for_five_seconds_then_stop() {
        let now = Instant::now();
        let mut e = npc(Position::default(), now);
        e.update(
            Position {
                animation: 20,
                ..Position::default()
            },
            now,
        );
        for seconds in [0.5, 1., 2.5, 4.9, 5.] {
            close(
                e.position(now + Duration::from_secs_f32(seconds))[1],
                11.6 * seconds,
            );
        }
        let later = now + Duration::from_secs(5);
        let continuing = Position {
            y: 58.,
            animation: 20,
            ..Position::default()
        };
        assert!(e.update(continuing, later));
        close(e.position(later)[1], 58.);
        close(e.position(later + Duration::from_secs(2))[1], 81.2);
        let stopped = later + Duration::from_secs(2);
        e.update(
            Position {
                y: 81.2,
                ..Position::default()
            },
            stopped,
        );
        assert!(!e.moving(stopped));
        close(e.position(stopped + Duration::from_secs(5))[1], 81.2);
    }

    #[test]
    fn heading_direction_matches_server_calculate_heading_quadrants() {
        let now = Instant::now();
        for (heading, direction) in [
            (0., [0., 1.]),
            (128., [1., 0.]),
            (256., [0., -1.]),
            (384., [-1., 0.]),
            (64., [0.70710677, 0.70710677]),
        ] {
            let e = npc(
                Position {
                    heading,
                    animation: 20,
                    ..Position::default()
                },
                now,
            );
            let p = e.position(now + Duration::from_secs(1));
            close(p[0], direction[0] * 11.6);
            close(p[1], direction[1] * 11.6);
        }
    }

    #[test]
    fn quantized_server_headings_follow_the_original_path() {
        let now = Instant::now();
        // Same quadrant calculation and 511.5-unit scaling as EQEmu's
        // CalculateHeadingAngleBetweenPositions, followed by FloatToEQ12.
        for (x, y) in [
            (0f32, 100f32),
            (100., 0.),
            (0., -100.),
            (-100., 0.),
            (30., 70.),
            (30., -70.),
            (-30., -70.),
            (-30., 70.),
        ] {
            let angle = x.abs().atan2(y.abs().max(0.000001)).to_degrees();
            let degrees = if y <= 0. {
                if x >= 0. { 180. - angle } else { 180. + angle }
            } else if x > 0. {
                angle
            } else {
                360. - angle
            };
            let heading = (((degrees * 511.5 / 360. + 2048.) * 4.) as u32 % 2048) as f32 / 4.;
            let e = npc(
                Position {
                    heading,
                    animation: 20,
                    ..Position::default()
                },
                now,
            );
            let p = e.position(now + Duration::from_secs(5));
            let length = x.hypot(y);
            // Quarter-heading quantization and EQ's 511.5 scale introduce less
            // than 0.6 units of error across a five-second, 58-unit walk.
            assert!((p[0] - x / length * 58.).abs() < 0.6);
            assert!((p[1] - y / length * 58.).abs() < 0.6);
        }
    }

    #[test]
    fn corrections_converge_and_teleports_snap() {
        let now = Instant::now();
        let mut e = npc(Position::default(), now);
        e.update(
            Position {
                x: 3.,
                z: 2.,
                ..Position::default()
            },
            now,
        );
        assert_eq!(e.position(now), [0.; 3]);
        let corrected = now + Duration::from_millis(150);
        assert_eq!(e.position(corrected), [3., 0., 2.]);
        e.update(
            Position {
                x: 200.,
                ..Position::default()
            },
            corrected,
        );
        assert_eq!(e.position(corrected), [200., 0., 0.]);
    }

    #[test]
    fn signed_turn_prediction_wraps_and_stops_on_authoritative_heading() {
        let now = Instant::now();
        for rate in [-16., 16.] {
            let mut e = npc(
                Position {
                    heading: 500.,
                    delta_heading: rate,
                    ..Position::default()
                },
                now,
            );
            let halfway = now + Duration::from_millis(500);
            close(
                e.heading(halfway),
                (500. + rate * 19. * 0.5).rem_euclid(512.),
            );
            let heading = e.heading(halfway);
            e.update(
                Position {
                    heading,
                    ..Position::default()
                },
                halfway,
            );
            close(e.heading(halfway + Duration::from_secs(2)), heading);
        }
        let mut e = npc(
            Position {
                heading: 510.,
                ..Position::default()
            },
            now,
        );
        e.update(
            Position {
                heading: 2.,
                ..Position::default()
            },
            now,
        );
        close(e.heading(now + Duration::from_millis(75)), 0.);
    }

    #[test]
    fn stale_predictions_freeze_and_stop_animation() {
        let now = Instant::now();
        let e = npc(
            Position {
                animation: 20,
                delta_heading: 16.,
                ..Position::default()
            },
            now,
        );
        let stale = now + Duration::from_secs(6);
        let later = now + Duration::from_secs(100);
        assert_eq!(e.position(stale), e.position(later));
        assert_eq!(e.heading(stale), e.heading(later));
        assert!(!e.moving(stale));
    }
}
