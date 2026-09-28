//! Background networking and bounded prediction of server-authoritative spawns.
use crate::game::{GameplayState, display_name};
use openeq_net::{
    gameplay::{Command, Door, GameplayEvent, ZoneDestination},
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
    Notice(String),
}
enum NetworkCommand {
    Target(u32),
    Gameplay(Command),
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

pub struct LiveWorld {
    pub entities: BTreeMap<u32, Entity>,
    pub environment: Option<Environment>,
    pub own_id: Option<u32>,
    pub initial_position: Option<Position>,
    pub character: String,
    pub ready: bool,
    pub error: Option<String>,
    pub moves: u64,
    pub hour: u8,
    pub minute: u8,
    pub target: Option<u32>,
    pub game: GameplayState,
    pub doors: BTreeMap<u8, Door>,
    pending_destination: Option<ZoneDestination>,
    rx: Mutex<mpsc::Receiver<Message>>,
    movement: tokio::sync::watch::Sender<Option<Position>>,
    commands: tokio::sync::mpsc::UnboundedSender<NetworkCommand>,
}

impl LiveWorld {
    pub fn start(config: ConnectionConfig) -> Self {
        let (tx, rx) = mpsc::channel();
        let (movement, mut updates) = tokio::sync::watch::channel(None);
        let (commands, mut requests) = tokio::sync::mpsc::unbounded_channel();
        let character = config.character.clone();
        std::thread::Builder::new().name("eq-network".into()).spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("network runtime");
            let result = rt.block_on(async {
                let mut zone = config.connect().await?;
                let mut own = None;
                let mut movement_ready = false;
                let mut heartbeat = tokio::time::interval(std::time::Duration::from_millis(100));
                loop {
                    tokio::select! {
                        Some(request) = requests.recv() => {
                            match request {
                                NetworkCommand::Target(id) => { if !zone.is_zoning() { zone.target(id).await?; } },
                                NetworkCommand::Gameplay(command) => {
                                    match zone.command(command.clone()).await {
                                        Ok(()) => { let _ = tx.send(Message::CommandSent(command)); }
                                        Err(openeq_net::zone::ZoneError::Malformed(what)) => {
                                            let _ = tx.send(Message::Notice(format!("Invalid {what}; action was not sent.")));
                                        }
                                        Err(openeq_net::zone::ZoneError::Zoning) => {
                                            let _ = tx.send(Message::Notice("Please wait for zone travel to finish.".into()));
                                        }
                                        Err(error) => return Err(error.into()),
                                    }
                                }
                            }
                        },
                        changed = updates.changed() => {
                            if changed.is_err() { zone.logout().await?; break; }
                            movement_ready = own.is_some() && !zone.is_zoning();
                        },
                        _ = heartbeat.tick() => {
                            let position = *updates.borrow();
                            if movement_ready && let (Some(id), Some(position)) = (own, position) { zone.send_position(id, position).await?; }
                        }
                        event = zone.next_event() => {
                            let event = event?;
                            if matches!(&event, ZoneEvent::Gameplay(GameplayEvent::ZoneTransition { .. })) {
                                own = None; movement_ready = false;
                            }
                            if let ZoneEvent::Spawn(spawn) = &event
                                && spawn.name.eq_ignore_ascii_case(&config.character) { own = Some(spawn.id); }
                            if tx.send(Message::Event(Box::new(event))).is_err() { zone.logout().await?; break; }
                        }
                    }
                }
                Ok::<_, anyhow::Error>(())
            });
            if let Err(error) = result { let _ = tx.send(Message::Error(format!("{error:#}"))); }
        }).expect("network worker");
        Self {
            entities: BTreeMap::new(),
            environment: None,
            own_id: None,
            initial_position: None,
            character,
            ready: false,
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: None,
            game: GameplayState::default(),
            doors: BTreeMap::new(),
            pending_destination: None,
            rx: Mutex::new(rx),
            movement,
            commands,
        }
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
                    self.game.attack = false;
                    self.game.inventory_command_pending = false;
                    self.game.error(format!(
                        "Connection lost: {}",
                        self.error.as_deref().unwrap_or("unknown error")
                    ));
                }
                Message::Notice(notice) => {
                    self.game.inventory_command_pending = false;
                    self.game.error(notice);
                }
                Message::CommandSent(command) => self.command_sent(command),
                Message::Event(event) => {
                    match *event {
                        ZoneEvent::Spawn(spawn) => {
                            if spawn.name.eq_ignore_ascii_case(&self.character) {
                                self.own_id = Some(spawn.id);
                                self.initial_position = Some(spawn.position);
                            }
                            self.entities.insert(spawn.id, Entity::new(spawn, now));
                        }
                        ZoneEvent::Movement { id, position } => {
                            if let Some(entity) = self.entities.get_mut(&id)
                                && entity.update(position, now)
                                && entity.spawn.npc
                            {
                                self.moves += 1;
                            }
                        }
                        ZoneEvent::Despawn(id) => {
                            self.entities.remove(&id);
                            if self.target == Some(id) {
                                self.target = None;
                            }
                        }
                        ZoneEvent::Environment(environment) => {
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
    }

    pub fn set_target(&mut self, id: Option<u32>) {
        self.target = id;
        let _ = self.commands.send(NetworkCommand::Target(id.unwrap_or(0)));
    }

    pub fn command(&mut self, command: Command) -> bool {
        if !self.ready || self.error.is_some() {
            self.game.error("You are not connected to the zone.");
            return false;
        }
        if let Command::MoveItem { from, to, count } = &command {
            if self.game.inventory_command_pending {
                return false;
            }
            if let Err(error) = self.game.inventory.validate_move(*from, *to, *count) {
                self.game.error(error);
                return false;
            }
        }
        let inventory_command = matches!(command, Command::MoveItem { .. });
        if self
            .commands
            .send(NetworkCommand::Gameplay(command))
            .is_err()
        {
            self.game.error("The network worker has stopped.");
            return false;
        }
        if inventory_command {
            self.game.inventory_command_pending = true;
        }
        true
    }

    fn command_sent(&mut self, command: Command) {
        match command {
            Command::MoveItem { from, to, count } => {
                self.game.inventory_command_pending = false;
                if let Err(error) = self.game.inventory.move_item(from, to, count) {
                    self.game.error(error);
                }
            }
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
    }

    fn gameplay_event(&mut self, event: GameplayEvent) {
        match &event {
            GameplayEvent::ZoneTransition { zone_id, .. } => {
                self.ready = false;
                self.entities.clear();
                self.doors.clear();
                self.own_id = None;
                self.target = None;
                self.initial_position = None;
                self.game.inventory = Default::default();
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
                if self
                    .environment
                    .as_ref()
                    .is_some_and(|environment| environment.zone_id == destination.zone_id)
                {
                    self.initial_position = Some(Position {
                        x: destination.position[0],
                        y: destination.position[1],
                        z: destination.position[2],
                        heading: destination.heading,
                        ..Default::default()
                    });
                }
                self.pending_destination = Some(destination.clone());
            }
            GameplayEvent::ZoneChangeResult {
                zone_id,
                position,
                success,
                ..
            } => {
                if *success == 1
                    && self
                        .environment
                        .as_ref()
                        .is_some_and(|env| env.zone_id == *zone_id)
                {
                    self.initial_position = Some(Position {
                        x: position[0],
                        y: position[1],
                        z: position[2],
                        heading: self
                            .pending_destination
                            .as_ref()
                            .map_or(0., |destination| destination.heading),
                        ..Default::default()
                    });
                } else if *success != 1 {
                    self.game
                        .error(format!("Zone travel was rejected ({success})."));
                }
            }
            GameplayEvent::Doors(doors) => {
                self.doors = doors.iter().map(|door| (door.id, door.clone())).collect()
            }
            GameplayEvent::DoorMoved { id, action } => {
                if let Some(door) = self.doors.get_mut(id) {
                    match action {
                        2 => door.state = u8::from(!door.inverted),
                        3 => door.state = u8::from(door.inverted),
                        _ => {}
                    }
                }
            }
            GameplayEvent::Assist(id) => self.set_target(Some(*id)),
            GameplayEvent::BeginCast { caster_id, .. } => {
                if let Some(entity) = self.entities.get_mut(caster_id) {
                    entity.action = ActorAction::Cast;
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
        if !self.ready {
            return;
        }
        self.movement.send_replace(Some(Position {
            x: camera.position[0],
            y: camera.position[1],
            z: camera.position[2] - 3.,
            heading: camera.yaw.rem_euclid(std::f32::consts::TAU) * 512. / std::f32::consts::TAU,
            animation: if moving { 12 } else { 0 },
            ..Position::default()
        }));
    }

    pub fn actors(&self, camera: [f32; 3]) -> Vec<ActorState> {
        self.actor_states(camera, None)
    }

    pub fn actor_states(
        &self,
        camera: [f32; 3],
        player: Option<(&openeq_render::Camera, bool)>,
    ) -> Vec<ActorState> {
        let now = Instant::now();
        self.entities
            .values()
            .filter(|e| {
                (Some(e.spawn.id) != self.own_id || player.is_some())
                    && e.spawn.race != 127
                    && e.spawn.body_type < 66
            })
            .filter_map(|e| {
                let own = (Some(e.spawn.id) == self.own_id)
                    .then_some(player)
                    .flatten();
                let p = own.map_or_else(
                    || e.position(now),
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
mod tests {
    use super::*;
    use std::time::Duration;

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
            },
            now,
        )
    }
    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.001, "{actual} != {expected}");
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
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: Some(2),
            game: GameplayState::default(),
            doors: BTreeMap::new(),
            pending_destination: None,
            rx: Mutex::new(events),
            movement,
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
