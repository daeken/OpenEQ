//! Background networking and bounded prediction of server-authoritative spawns.
use crate::coordinates;
use crate::game::{GameplayState, display_name};
use openeq_assets::collision::CollisionWorld;
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
    CommandRejected { command: Command, notice: String },
}
pub(crate) enum NetworkCommand {
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
    door_return_deadlines: BTreeMap<u8, Instant>,
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
        let mut game = GameplayState::default();
        game.commerce.shared_coin_enabled = config.host.eq_ignore_ascii_case("storage2.daeken.dev");
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
                                    let mut wire_command = command.clone();
                                    if let Command::ZoneChange { position, .. } = &mut wire_command {
                                        *position = coordinates::scene_point_to_server(*position);
                                    }
                                    match zone.command(wire_command).await {
                                        Ok(()) => { let _ = tx.send(Message::CommandSent(command)); }
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
                            if changed.is_err() { zone.logout().await?; break; }
                            movement_ready = own.is_some() && !zone.is_zoning();
                        },
                        _ = heartbeat.tick() => {
                            let position = *updates.borrow();
                            if movement_ready && let (Some(id), Some(position)) = (own, position) { zone.send_position(id, coordinates::scene_to_server(position)).await?; }
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
            game,
            doors: BTreeMap::new(),
            door_return_deadlines: BTreeMap::new(),
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
                    self.game.commerce.close_services();
                    self.game.trade = Default::default();
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
                Message::Event(event) => {
                    match *event {
                        ZoneEvent::Spawn(mut spawn) => {
                            spawn.position = coordinates::server_to_scene(spawn.position);
                            if spawn.name.eq_ignore_ascii_case(&self.character) {
                                self.own_id = Some(spawn.id);
                                self.initial_position = Some(spawn.position);
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

    pub fn player_position(&self) -> Option<[f32; 3]> {
        self.initial_position
            .or(*self.movement.borrow())
            .or_else(|| {
                self.own_id
                    .and_then(|id| self.entities.get(&id))
                    .map(|e| e.spawn.position)
            })
            .map(|p| [p.x, p.y, p.z])
    }

    pub fn service_available(&self, id: u32, class: u8) -> bool {
        self.ready
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

    pub fn command(&mut self, command: Command) -> bool {
        if !self.ready || self.error.is_some() {
            self.game.error("You are not connected to the zone.");
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
        self.game.error(notice);
    }

    pub(crate) fn gameplay_event(&mut self, mut event: GameplayEvent) {
        self.trade_event(&event);
        self.item_use_event(&event);
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
        match &event {
            GameplayEvent::ZoneTransition { zone_id, .. } => {
                self.ready = false;
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
                    self.item_use_reset();
                    if self.game.trade.engaged() {
                        self.command(Command::Trade(openeq_net::trade::TradeCommand::Cancel {
                            player_id: death.id,
                        }));
                    }
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
                (Some(e.spawn.id) != self.own_id || player.is_some())
                    && e.spawn.race != 127
                    && e.spawn.body_type < 66
            })
            .filter_map(|e| {
                let own = (Some(e.spawn.id) == self.own_id)
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
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: None,
            game: GameplayState::default(),
            doors: BTreeMap::new(),
            door_return_deadlines: BTreeMap::new(),
            pending_destination: None,
            rx: Mutex::new(events),
            movement,
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
        events
            .send(Message::Event(Box::new(ZoneEvent::Spawn(spawn))))
            .unwrap();
        events
            .send(Message::Event(Box::new(ZoneEvent::Environment(
                Environment {
                    short_name: "poknowledge".into(),
                    long_name: "Plane of Knowledge".into(),
                    zone_id: 202,
                    fog_color: [[0.; 3]; 4],
                    fog_start: [0.; 4],
                    fog_end: [1000.; 4],
                    fog_density: 0.,
                    min_clip: 1.,
                    max_clip: 1000.,
                    sky: 1,
                    zone_type: 0,
                    safe_position: [944., -305., -90.],
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
            error: None,
            moves: 0,
            hour: 12,
            minute: 0,
            target: Some(2),
            game: GameplayState::default(),
            doors: BTreeMap::new(),
            door_return_deadlines: BTreeMap::new(),
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
