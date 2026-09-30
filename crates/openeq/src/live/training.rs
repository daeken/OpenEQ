//! Production trainer boundary: one foreground claim, a fresh worker check, and
//! correlated authoritative receipts. No opening or purchasing occurs on lookup.
use super::*;
use crate::training::{
    BlockReason, DispatchState, Observation, Stamp, TrainerIdentity, TrainingError,
    TrainingPreview, TrainingState,
};
use openeq_net::training::{Selection, TrainingCommand};
use std::time::Duration;

pub(super) const REQUEST_TIMEOUT: Duration = Duration::from_secs(12);

/// EQEmu common/strings_legacy.cpp CleanMobName: underscores become spaces;
/// ASCII letters and backticks survive; digits and other punctuation do not.
pub fn clean_trainer_name(name: &str) -> String {
    name.chars()
        .filter_map(|c| match c {
            '_' => Some(' '),
            '`' => Some(c),
            c if c.is_ascii_alphabetic() => Some(c),
            _ => None,
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(crate) struct Request {
    pub stamp: Stamp,
    pub command: TrainingCommand,
    pub trainer: TrainerIdentity,
    pub authority_revision: u64,
    pub known_copper: u64,
    pub created: Instant,
}

#[derive(Clone)]
struct Trainer {
    identity: TrainerIdentity,
    class: u8,
    position: Position,
    living: bool,
}

pub(crate) struct State {
    reducer: TrainingState,
    revision: u64,
    authority_revision: u64,
    next_incarnation: u64,
    trainers: BTreeMap<u32, Trainer>,
    player_class: Option<u8>,
    player_position: Option<Position>,
    player_id: Option<u32>,
    player_living: bool,
    pending_since: Option<Instant>,
    /// Worker-local operation IDs can differ after proven-unsent foreground choices.
    worker_pending_external: Option<Stamp>,
    last_worker_operation: u64,
    pub(super) currency_uncertain: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            reducer: TrainingState::new(0, 0),
            revision: 0,
            authority_revision: 0,
            next_incarnation: 0,
            trainers: BTreeMap::new(),
            player_class: None,
            player_position: None,
            player_id: None,
            player_living: false,
            pending_since: None,
            worker_pending_external: None,
            last_worker_operation: 0,
            currency_uncertain: false,
        }
    }
}

#[derive(Default)]
pub(super) struct Effect {
    pub debit: Option<u32>,
    pub became_uncertain: bool,
}

impl State {
    pub(super) fn busy(&self) -> bool {
        self.reducer.pending().is_some()
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    fn pending_purchase(&self) -> bool {
        self.reducer
            .pending()
            .is_some_and(|(_, command, _)| matches!(command, TrainingCommand::Train { .. }))
    }
    fn dispatched_purchase(&self) -> bool {
        self.reducer.pending().is_some_and(|(_, command, phase)| {
            matches!(command, TrainingCommand::Train { .. }) && phase == DispatchState::Sent
        })
    }
    fn uncertain(&mut self) -> bool {
        let changed = !self.currency_uncertain;
        self.currency_uncertain = true;
        changed
    }
    fn observation(&mut self, observation: Observation, purchase: bool) -> Effect {
        let mut effect = Effect::default();
        match observation {
            Observation::Completed => {
                self.pending_since = None;
                self.worker_pending_external = None;
                effect.debit = self
                    .reducer
                    .last_report()
                    .map(|r| r.completion.assessed_cost_copper);
            }
            Observation::Opened => {
                self.pending_since = None;
                self.worker_pending_external = None;
            }
            Observation::Blocked(_) => {
                self.pending_since = None;
                self.worker_pending_external = None;
                if purchase {
                    effect.became_uncertain = self.uncertain();
                }
            }
            _ => {}
        }
        if observation != Observation::Ignored {
            self.changed();
        }
        effect
    }
    /// Worker publishes Sent before the next delivered authoritative event.
    /// Thus a Claimed-only request retired by such an event is proven unsent.
    fn retire_undispatched(&mut self) {
        if let Some((stamp, _, DispatchState::Claimed | DispatchState::Prepared)) =
            self.reducer.pending()
        {
            self.reducer.rejected_unsent(stamp);
            self.pending_since = None;
            self.worker_pending_external = None;
        }
    }
    fn sync_epoch(&mut self, epoch: u64, own: Option<u32>) -> Effect {
        if self.reducer.epoch() == epoch {
            return Effect::default();
        }
        let purchase = self.dispatched_purchase();
        self.retire_undispatched();
        self.reducer.begin_epoch(epoch, own.unwrap_or(0));
        self.player_class = None;
        self.player_id = own;
        self.player_position = None;
        self.player_living = false;
        self.pending_since = None;
        self.worker_pending_external = None;
        self.authority_revision = self.authority_revision.wrapping_add(1);
        self.changed();
        Effect {
            debit: None,
            became_uncertain: purchase && self.uncertain(),
        }
    }
    fn interrupt(&mut self, reason: BlockReason) -> Effect {
        let purchase = self.dispatched_purchase();
        self.retire_undispatched();
        let observation = self.reducer.interrupt(self.reducer.epoch(), reason);
        self.observation(observation, purchase)
    }
    fn invalidate_trainer(&mut self, id: u32) -> Effect {
        if !self.reducer.active_trainer().is_some_and(|t| t.id == id) {
            return Effect::default();
        }
        if self.reducer.pending().is_some() {
            self.interrupt(BlockReason::TrainerChanged)
        } else {
            self.reducer.close(self.reducer.epoch());
            self.changed();
            Effect::default()
        }
    }
    /// Positions retained here use server coordinates in foreground and worker.
    pub(super) fn zone_event(
        &mut self,
        event: &ZoneEvent,
        epoch: u64,
        own: Option<u32>,
        character: &str,
    ) -> Effect {
        if let ZoneEvent::Gameplay(event) = event {
            return self.gameplay_event(event, epoch, own);
        }
        let mut effect = self.sync_epoch(epoch, own);
        match event {
            ZoneEvent::Spawn(spawn) => {
                if is_own_spawn(spawn, character) && own == Some(spawn.id) {
                    self.player_id = own;
                    self.player_position = Some(spawn.position);
                    self.player_living = true;
                    self.authority_revision = self.authority_revision.wrapping_add(1);
                    if !self.reducer.bind_player(epoch, spawn.id) {
                        let purchase = self.dispatched_purchase();
                        self.reducer.begin_epoch(epoch, spawn.id);
                        self.pending_since = None;
                        self.worker_pending_external = None;
                        effect.became_uncertain |= purchase && self.uncertain();
                    }
                    self.changed();
                }
                if self.trainers.contains_key(&spawn.id) {
                    let changed = self.invalidate_trainer(spawn.id);
                    effect.became_uncertain |= changed.became_uncertain;
                }
                self.trainers.remove(&spawn.id);
                if spawn.npc && (20..=35).contains(&spawn.class) {
                    self.next_incarnation = self.next_incarnation.wrapping_add(1);
                    self.trainers.insert(
                        spawn.id,
                        Trainer {
                            identity: TrainerIdentity {
                                id: spawn.id,
                                revision: self.next_incarnation,
                                clean_name: clean_trainer_name(&spawn.name),
                            },
                            class: spawn.class,
                            position: spawn.position,
                            living: !spawn.is_corpse && spawn.hp_percent > 0,
                        },
                    );
                    self.changed();
                }
            }
            ZoneEvent::Movement { id, position } => {
                if own == Some(*id) {
                    self.player_position = Some(*position);
                }
                if let Some(trainer) = self.trainers.get_mut(id) {
                    trainer.position = *position;
                }
            }
            ZoneEvent::Despawn(id) => {
                let changed = self.invalidate_trainer(*id);
                effect.became_uncertain |= changed.became_uncertain;
                self.trainers.remove(id);
                if own == Some(*id) {
                    self.player_living = false;
                }
                self.changed();
            }
            ZoneEvent::Hp { id, percent } if *percent == 0 => {
                if own == Some(*id) {
                    self.player_living = false;
                }
                if let Some(trainer) = self.trainers.get_mut(id) {
                    trainer.living = false;
                }
                let changed = self.invalidate_trainer(*id);
                effect.became_uncertain |= changed.became_uncertain;
            }
            ZoneEvent::Gameplay(event) => return self.gameplay_event(event, epoch, own),
            _ => {}
        }
        effect
    }
    pub(super) fn gameplay_event(
        &mut self,
        event: &GameplayEvent,
        epoch: u64,
        own: Option<u32>,
    ) -> Effect {
        let mut effect = self.sync_epoch(epoch, own);
        let mut purchase = self.dispatched_purchase();
        let observation = match event {
            GameplayEvent::Profile(profile) => {
                self.player_class = Some(profile.class);
                self.reducer.apply_profile(epoch, profile.into());
                self.currency_uncertain = false;
                self.pending_since = None;
                self.worker_pending_external = None;
                self.authority_revision = self.authority_revision.wrapping_add(1);
                Observation::Updated
            }
            GameplayEvent::Training(event) => self.reducer.observe(epoch, event.clone()),
            GameplayEvent::Progression(openeq_net::progression::ProgressionEvent::SkillValue {
                wire_skill_id,
                value,
            }) => {
                self.authority_revision = self.authority_revision.wrapping_add(1);
                self.retire_undispatched();
                self.reducer.observe_skill(epoch, *wire_skill_id, *value)
            }
            GameplayEvent::Progression(openeq_net::progression::ProgressionEvent::Level {
                ..
            }) => {
                self.authority_revision = self.authority_revision.wrapping_add(1);
                self.retire_undispatched();
                self.reducer.observe_level(epoch)
            }
            GameplayEvent::Currency(currency)
            | GameplayEvent::BankerBalances {
                carried: currency, ..
            } => {
                self.authority_revision = self.authority_revision.wrapping_add(1);
                self.retire_undispatched();
                self.currency_uncertain = false;
                self.reducer.observe_currency(epoch, *currency)
            }
            GameplayEvent::Trade(_)
            | GameplayEvent::MerchantBought { .. }
            | GameplayEvent::MerchantSold { .. } => {
                self.authority_revision = self.authority_revision.wrapping_add(1);
                Observation::Updated
            }
            GameplayEvent::ZoneTransition { .. } => {
                self.trainers.clear();
                self.player_living = false;
                Observation::Updated
            }
            GameplayEvent::Death(death) => {
                if own == Some(death.id) || self.player_id == Some(death.id) {
                    self.player_living = false;
                }
                if let Some(trainer) = self.trainers.get_mut(&death.id) {
                    trainer.living = false;
                }
                let changed = self.invalidate_trainer(death.id);
                effect.became_uncertain |= changed.became_uncertain;
                Observation::Updated
            }
            GameplayEvent::SpawnAppearance {
                id,
                kind: 14,
                parameter: 3 | 115,
            } => {
                self.authority_revision = self.authority_revision.wrapping_add(1);
                if own == Some(*id) {
                    self.player_living = false;
                    self.retire_undispatched();
                    self.reducer.interrupt(epoch, BlockReason::Recovery)
                } else {
                    if let Some(trainer) = self.trainers.get_mut(id) {
                        trainer.living = false;
                    }
                    let changed = self.invalidate_trainer(*id);
                    effect.became_uncertain |= changed.became_uncertain;
                    Observation::Updated
                }
            }
            GameplayEvent::Health { id, current, .. } if own == Some(*id) => {
                self.player_living = *current > 0;
                Observation::Updated
            }
            GameplayEvent::ZoneChangeRequested(_) => {
                self.authority_revision = self.authority_revision.wrapping_add(1);
                self.retire_undispatched();
                self.reducer.interrupt(epoch, BlockReason::Recovery)
            }
            _ => {
                purchase = false;
                Observation::Ignored
            }
        };
        let result = self.observation(observation, purchase);
        effect.debit = result.debit;
        effect.became_uncertain |= result.became_uncertain;
        effect
    }
    fn identity(&self, id: u32, position: Option<Position>) -> Option<TrainerIdentity> {
        let trainer = self.trainers.get(&id)?;
        let class = self.player_class?;
        let position = position.or(self.player_position)?;
        (self.player_living
            && self.player_id.is_some()
            && trainer.living
            && (1..=16).contains(&class)
            && trainer.class == class + 19
            && in_range(position, trainer.position))
        .then(|| trainer.identity.clone())
    }
    pub(super) fn command_sent(&mut self, command: &Command) {
        if incompatible(command) {
            self.authority_revision = self.authority_revision.wrapping_add(1);
        }
    }
    pub(super) fn permits_command(&self, command: &Command) -> bool {
        !(self.reducer.pending().is_some() && incompatible(command)
            || self.currency_uncertain && spends_money(command))
    }
    /// A fresh worker replica checks queued authority, profile, trainer and local
    /// balance provenance again. Its local operation counter is independent.
    pub(super) fn worker_claim(
        &mut self,
        request: &Request,
        epoch: u64,
        position: Option<Position>,
    ) -> Result<Stamp, &'static str> {
        if request.created.elapsed() >= REQUEST_TIMEOUT
            || request.stamp.epoch != epoch
            || request.stamp.profile_revision != self.reducer.profile_revision()
            || request.authority_revision != self.authority_revision
            || request.stamp.operation_id <= self.last_worker_operation
            || self.currency_uncertain
            || self.reducer.pending().is_some()
            || self.identity(request.trainer.id, position).as_ref() != Some(&request.trainer)
        {
            return Err("That trainer choice is no longer current.");
        }
        self.last_worker_operation = request.stamp.operation_id;
        self.reducer
            .rebase_estimated_copper(epoch, request.known_copper);
        let stamp = match request.command {
            TrainingCommand::Open {
                trainer_id,
                player_id,
            } if trainer_id == request.trainer.id && Some(player_id) == self.player_id => {
                self.reducer.request_open(epoch, request.trainer.clone())
            }
            TrainingCommand::Train {
                trainer_id,
                selection,
            } if trainer_id == request.trainer.id => self.reducer.request_train(epoch, selection),
            _ => return Err("Invalid trainer request."),
        }
        .map_err(|_| "Training is unavailable with the current character data.")?;
        self.reducer
            .claim(stamp, &request.trainer)
            .map_err(|_| "That trainer choice is no longer current.")?;
        self.pending_since = Some(Instant::now());
        self.worker_pending_external = Some(request.stamp);
        Ok(stamp)
    }
    pub(super) fn sent(&mut self, stamp: Stamp) {
        if self.reducer.sent(stamp) {
            self.changed();
        }
    }
    pub(super) fn rejected(&mut self, stamp: Stamp) -> bool {
        if self.reducer.rejected_unsent(stamp) {
            self.pending_since = None;
            self.worker_pending_external = None;
            self.changed();
            true
        } else {
            false
        }
    }
    pub(super) fn failed(&mut self) -> Effect {
        let Some((stamp, _, _)) = self.reducer.pending() else {
            return Effect::default();
        };
        let purchase = self.pending_purchase();
        let observation = self.reducer.send_failed(stamp);
        self.observation(observation, purchase)
    }
    pub(super) fn close(&mut self, epoch: u64) -> (Option<TrainingCommand>, Effect) {
        if epoch != self.reducer.epoch() {
            return (None, Effect::default());
        }
        let purchase = self.pending_purchase();
        let command = self.reducer.close(epoch);
        self.pending_since = None;
        self.worker_pending_external = None;
        self.changed();
        let effect = Effect {
            debit: None,
            became_uncertain: purchase && self.uncertain(),
        };
        (command, effect)
    }
    pub(super) fn timed_out(&mut self, stamp: Stamp) -> Effect {
        let purchase = self.pending_purchase();
        let observation = self.reducer.timeout(stamp);
        self.observation(observation, purchase)
    }
    /// Publish this foreground stamp before any later receipt is delivered.
    /// A paused foreground must learn the worker already retired the operation.
    pub(super) fn worker_timeout(&mut self, now: Instant) -> Option<Stamp> {
        let external = self.worker_pending_external?;
        if !self
            .pending_since
            .is_some_and(|sent| now.saturating_duration_since(sent) >= REQUEST_TIMEOUT)
        {
            return None;
        }
        self.tick(now);
        Some(external)
    }
    pub(super) fn tick(&mut self, now: Instant) -> Effect {
        if self
            .pending_since
            .is_some_and(|sent| now.saturating_duration_since(sent) >= REQUEST_TIMEOUT)
            && let Some((stamp, _, _)) = self.reducer.pending()
        {
            return self.timed_out(stamp);
        }
        Effect::default()
    }
}

fn in_range(player: Position, trainer: Position) -> bool {
    let deltas = [
        player.x - trainer.x,
        player.y - trainer.y,
        player.z - trainer.z,
    ];
    deltas.iter().all(|v| v.is_finite()) && deltas.iter().map(|v| v * v).sum::<f32>() <= 40_000.
}

pub(super) fn incompatible(command: &Command) -> bool {
    !matches!(
        command,
        Command::Chat { .. }
            | Command::Emote(_)
            | Command::Consider { .. }
            | Command::Assist(_)
            | Command::AutoAttack(false)
            | Command::InterruptSpell
    )
}
fn spends_money(command: &Command) -> bool {
    matches!(
        command,
        Command::MerchantBuy { .. }
            | Command::MerchantSell { .. }
            | Command::MoveCoin { .. }
            | Command::BankerChange
            | Command::Trade(
                openeq_net::trade::TradeCommand::OfferCoin { .. }
                    | openeq_net::trade::TradeCommand::Accept { .. }
            )
    )
}

impl LiveWorld {
    pub fn training_state(&self) -> &TrainingState {
        &self.training.reducer
    }
    pub fn training_revision(&self) -> u64 {
        self.training.revision
    }
    fn training_position(&self) -> Option<Position> {
        self.player_position().map(|p| {
            coordinates::scene_to_server(Position {
                x: p[0],
                y: p[1],
                z: p[2],
                ..Default::default()
            })
        })
    }
    pub fn training_available(&self, trainer_id: u32) -> bool {
        self.movement_allowed()
            && !self.game.attack
            && self.error.is_none()
            && self
                .training
                .identity(trainer_id, self.training_position())
                .is_some()
    }
    fn training_services_clear(&self) -> bool {
        !self.game.trade.engaged()
            && !self.game.inventory_command_pending
            && !self.game.item_use.busy()
            && !self.game.commerce.coin_pending
            && self.game.commerce.pending.is_none()
            && self.game.commerce.merchant.is_none()
            && self.game.commerce.bank.is_none()
            && self.game.loot.is_none()
            && self.game.casting.is_none()
            && self.game.cast_pending_until.is_none()
            && self.game.commerce.currency_ready
    }
    pub fn training_preview(&self, selection: Selection) -> Result<TrainingPreview, TrainingError> {
        let trainer = self
            .training
            .reducer
            .trainer()
            .ok_or(TrainingError::NotOpen)?;
        if !self.training_available(trainer.id) {
            return Err(TrainingError::InvalidTrainer);
        }
        if !self.training_services_clear() {
            return Err(TrainingError::Busy);
        }
        self.training
            .reducer
            .preview(self.movement_authority.action_epoch, selection)
    }
    fn queue_training(&mut self, stamp: Stamp, trainer: TrainerIdentity) -> bool {
        let command = match self.training.reducer.claim(stamp, &trainer) {
            Ok(command) => command,
            Err(_) => return false,
        };
        let request = Request {
            stamp,
            command,
            trainer,
            authority_revision: self.training.authority_revision,
            known_copper: crate::commerce::total_copper(self.game.currency),
            created: Instant::now(),
        };
        if self
            .commands
            .send(NetworkCommand::Training(request))
            .is_err()
        {
            self.training.rejected(stamp);
            self.game
                .error("The connection closed before training was queued.");
            return false;
        }
        self.training.pending_since = Some(Instant::now());
        self.training.changed();
        true
    }
    pub fn open_training(&mut self, trainer_id: u32) -> bool {
        if self.training.busy()
            || !self.training_available(trainer_id)
            || !self.training_services_clear()
        {
            self.game.notice("Finish the current action and select a living trainer for your class within reach.");
            return false;
        }
        let epoch = self.movement_authority.action_epoch;
        let trainer = self
            .training
            .identity(trainer_id, self.training_position())
            .unwrap();
        self.training
            .reducer
            .rebase_estimated_copper(epoch, crate::commerce::total_copper(self.game.currency));
        match self.training.reducer.request_open(epoch, trainer.clone()) {
            Ok(stamp) => self.queue_training(stamp, trainer),
            Err(_) => {
                self.game.notice("Training is not ready. Wait for the current request or reconnect to refresh character data.");
                false
            }
        }
    }
    pub fn train_selection(&mut self, selection: Selection) -> bool {
        if self.training_preview(selection).is_err() {
            self.game
                .notice("That training choice is unavailable with the current character data.");
            return false;
        }
        let epoch = self.movement_authority.action_epoch;
        let trainer = self.training.reducer.trainer().unwrap().clone();
        self.training
            .reducer
            .rebase_estimated_copper(epoch, crate::commerce::total_copper(self.game.currency));
        match self.training.reducer.request_train(epoch, selection) {
            Ok(stamp) => self.queue_training(stamp, trainer),
            Err(_) => false,
        }
    }
    pub fn close_training(&mut self) {
        let epoch = self.movement_authority.action_epoch;
        let (command, effect) = self.training.close(epoch);
        self.training_effect(effect);
        if let Some(command) = command {
            let _ = self
                .commands
                .send(NetworkCommand::TrainingEnd { command, epoch });
        }
    }
    pub(super) fn training_effect(&mut self, mut effect: Effect) {
        if let Some(cost) = effect.debit
            && (!self.game.commerce.currency_ready
                || !crate::commerce::debit(&mut self.game.currency, cost))
        {
            effect.became_uncertain |= self.training.uncertain();
            self.training.reducer.interrupt(
                self.movement_authority.action_epoch,
                BlockReason::AssessedCostExceedsFunds,
            );
            self.training.changed();
        }
        if self.training.currency_uncertain {
            self.game.commerce.currency_ready = false;
        }
        if effect.became_uncertain {
            self.game.error("Training could not be confirmed. Money actions are unavailable until a fresh balance arrives; reconnect to confirm practices.");
        }
    }
    pub(super) fn training_command_sent(&mut self, command: &Command) {
        self.training.command_sent(command);
        self.training_refresh_money();
    }
    pub(super) fn training_refresh_money(&mut self) {
        if self.training.reducer.pending().is_none() && self.game.commerce.currency_ready {
            self.training.reducer.rebase_estimated_copper(
                self.movement_authority.action_epoch,
                crate::commerce::total_copper(self.game.currency),
            );
            self.training.changed();
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use openeq_net::{
        gameplay::{Currency, PlayerProfile},
        training::{Completion, TrainingEvent},
    };

    fn spawn(id: u32, npc: bool, class: u8, name: &str) -> Spawn {
        Spawn {
            id,
            name: name.into(),
            last_name: String::new(),
            level: 10,
            class,
            race: 1,
            gender: 0,
            npc,
            size: 6.,
            hp_percent: 100,
            walk_speed: 0.,
            run_speed: 0.,
            body_type: 1,
            position: Position {
                x: if npc { 10. } else { 0. },
                ..Default::default()
            },
            appearance: Default::default(),
            is_corpse: false,
            stand_state: 100,
            fly_mode: 0,
        }
    }
    fn profile() -> PlayerProfile {
        PlayerProfile {
            name: "Player".into(),
            last_name: String::new(),
            guild_id: None,
            guild_rank: 0,
            race: 1,
            class: 1,
            level: 10,
            hp: 100,
            mana: 0,
            endurance: 100,
            stats: [75; 7],
            currency: Currency {
                platinum: 100,
                ..Default::default()
            },
            bank_currency: Currency::default(),
            cursor_currency: Currency::default(),
            shared_platinum: 0,
            training_points: 2,
            skills: vec![55; 78],
            languages: vec![99; 28],
            experience_total: 0,
            spell_book: Vec::new(),
            memorized_spells: Vec::new(),
            spell_refresh: Vec::new(),
            buffs: Vec::new(),
        }
    }
    pub(crate) struct Fixture {
        pub(crate) live: LiveWorld,
        worker: State,
        io: NetworkIo,
        motion: MovementAuthority,
    }
    impl Fixture {
        pub(crate) fn queue_empty(&self) -> bool {
            self.io.requests.is_empty()
        }
        pub(crate) fn new() -> Self {
            let (live, io) = LiveWorld::channels("Player".into(), false);
            let mut fixture = Self {
                live,
                worker: State::default(),
                io,
                motion: MovementAuthority::default(),
            };
            // Reproduce the actual profile-before-own-spawn handshake order.
            fixture.event(ZoneEvent::Gameplay(GameplayEvent::Profile(profile())));
            fixture.event(ZoneEvent::Spawn(spawn(1, false, 1, "Player")));
            fixture.event(ZoneEvent::Spawn(spawn(2, true, 20, "Warlord_Welorf000")));
            fixture.event(ZoneEvent::Ready);
            fixture
        }
        pub(crate) fn event(&mut self, event: ZoneEvent) {
            if let ZoneEvent::Gameplay(event) = &event {
                self.motion.gameplay(event, None);
            }
            if let ZoneEvent::Spawn(spawn) = &event {
                self.motion.spawn(spawn, "Player");
            }
            self.worker.zone_event(
                &event,
                self.motion.action_epoch,
                self.motion.own_id,
                "Player",
            );
            self.io.tx.send(Message::Event(Box::new(event))).unwrap();
            self.live.poll();
        }
        pub(crate) fn queued(&mut self) -> Request {
            let NetworkCommand::Training(request) = self.io.requests.try_recv().unwrap() else {
                panic!("trainer queue expected")
            };
            request
        }
        pub(crate) fn send(&mut self, request: &Request) -> Stamp {
            let internal = self
                .worker
                .worker_claim(request, self.motion.action_epoch, None)
                .unwrap();
            self.worker.sent(internal);
            self.io
                .tx
                .send(Message::TrainingSent(request.stamp))
                .unwrap();
            self.live.poll();
            internal
        }
        pub(crate) fn open(&mut self) {
            assert!(self.live.open_training(2));
            let request = self.queued();
            self.send(&request);
            self.event(ZoneEvent::Gameplay(GameplayEvent::Training(
                TrainingEvent::Opened {
                    trainer_id: 2,
                    player_id: 1,
                    skills: Box::new([200; 100]),
                },
            )));
            assert_eq!(
                self.live.training_state().trainer().unwrap().clean_name,
                "Warlord Welorf"
            );
        }
        pub(crate) fn receipt(&mut self, value: u32, cost: u32) {
            self.event(ZoneEvent::Gameplay(GameplayEvent::Progression(
                openeq_net::progression::ProgressionEvent::SkillValue {
                    wire_skill_id: 0,
                    value,
                },
            )));
            self.event(ZoneEvent::Gameplay(GameplayEvent::Training(
                TrainingEvent::Completed(Completion {
                    wire_skill_id: 0,
                    assessed_cost_copper: cost,
                    new_skill: 0,
                    trainer_name: "Warlord Welorf".into(),
                }),
            )));
        }
    }

    #[test]
    fn production_sequential_purchases_debit_shared_coins_once_and_keep_profile_snapshot() {
        let mut fixture = Fixture::new();
        fixture.open();
        let selection = Selection::new(0, 0).unwrap();
        for (value, cost, copper, points) in [(56, 911, 99_089, 1), (57, 973, 98_116, 0)] {
            assert!(fixture.live.train_selection(selection));
            assert!(!fixture.live.train_selection(selection));
            assert!(!fixture.live.open_training(2));
            assert!(fixture.live.training_state().pending().is_some());
            assert!(fixture.live.training_state().blocked().is_none());
            let request = fixture.queued();
            assert!(fixture.queue_empty());
            fixture.send(&request);
            assert!(fixture.worker.worker_claim(&request, 0, None).is_err());
            fixture.receipt(value, cost);
            assert_eq!(
                crate::commerce::total_copper(fixture.live.game.currency),
                copper
            );
            assert_eq!(fixture.live.game.progression.skills[0], Some(value));
            assert_eq!(
                fixture
                    .live
                    .training_state()
                    .estimate()
                    .unwrap()
                    .training_points,
                points
            );
            assert_eq!(
                fixture.live.game.profile.as_ref().unwrap().training_points,
                2
            );
            assert_eq!(
                fixture.live.game.progression.profile_training_points,
                Some(2)
            );
            assert!(fixture.live.game.commerce.currency_ready);
            fixture.receipt(value, cost);
            assert_eq!(
                crate::commerce::total_copper(fixture.live.game.currency),
                copper
            );
        }
        assert_eq!(
            fixture.live.training_preview(selection),
            Err(TrainingError::NoPractices)
        );
    }

    #[test]
    fn worker_checks_late_profile_level_range_and_trainer_incarnation_before_dispatch() {
        for boundary in 0..5 {
            let mut fixture = Fixture::new();
            assert!(fixture.live.open_training(2));
            let request = fixture.queued();
            match boundary {
                0 => fixture.event(ZoneEvent::Gameplay(GameplayEvent::Profile(profile()))),
                1 => fixture.event(ZoneEvent::Gameplay(GameplayEvent::Progression(
                    openeq_net::progression::ProgressionEvent::Level {
                        level: 11,
                        reported_old_level: 10,
                        bar_units: 0,
                    },
                ))),
                2 => fixture.event(ZoneEvent::Despawn(2)),
                3 => fixture.event(ZoneEvent::Spawn(spawn(2, true, 20, "Warlord_Welorf001"))),
                _ => fixture.event(ZoneEvent::Movement {
                    id: 2,
                    position: Position {
                        x: 201.,
                        ..Default::default()
                    },
                }),
            }
            assert!(
                fixture
                    .worker
                    .worker_claim(&request, fixture.motion.action_epoch, None)
                    .is_err(),
                "boundary {boundary}"
            );
            assert!(fixture.live.game.commerce.currency_ready);
            assert_eq!(
                crate::commerce::total_copper(fixture.live.game.currency),
                100_000
            );
        }
        let mut fixture = Fixture::new();
        assert!(fixture.live.open_training(2));
        let request = fixture.queued();
        assert!(
            fixture
                .worker
                .worker_claim(
                    &request,
                    0,
                    Some(Position {
                        x: -201.,
                        ..Default::default()
                    })
                )
                .is_err()
        );
        assert!(!fixture.live.training_available(3));
        fixture.event(ZoneEvent::Spawn(spawn(3, true, 21, "Cleric_Trainer")));
        assert!(!fixture.live.training_available(3));
    }

    #[test]
    fn foreground_and_worker_gate_conflicting_actions_and_generic_training_bypass() {
        let mut fixture = Fixture::new();
        fixture.open();
        assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
        let request = fixture.queued();
        fixture.send(&request);
        fixture.live.camp.enabled = true;
        assert!(!fixture.live.request_camp());
        for command in [
            Command::MerchantOpen {
                merchant_id: 9,
                player_id: 1,
            },
            Command::MoveItem {
                from: openeq_net::inventory::InventorySlot::possessions(23),
                to: openeq_net::inventory::InventorySlot::CURSOR,
                count: 1,
            },
            Command::Trade(openeq_net::trade::TradeCommand::Request {
                to_id: 8,
                from_id: 1,
            }),
            Command::Training(request.command),
        ] {
            assert!(!fixture.worker.permits_command(&command));
            assert!(!fixture.live.command(command));
        }
        assert!(fixture.io.requests.try_recv().is_err());
    }

    #[test]
    fn proven_unsent_rejection_releases_claim_without_debit_or_ambiguity() {
        let mut fixture = Fixture::new();
        fixture.open();
        assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
        let request = fixture.queued();
        fixture.event(ZoneEvent::Gameplay(GameplayEvent::Currency(Currency {
            platinum: 100,
            ..Default::default()
        })));
        assert!(fixture.worker.worker_claim(&request, 0, None).is_err());
        fixture
            .io
            .tx
            .send(Message::TrainingRejected {
                stamp: request.stamp,
                notice: "not sent".into(),
            })
            .unwrap();
        fixture.live.poll();
        assert!(fixture.live.training_state().pending().is_none());
        assert!(fixture.live.training_state().blocked().is_none());
        assert_eq!(
            fixture
                .live
                .training_state()
                .estimate()
                .unwrap()
                .training_points,
            2
        );
        assert!(fixture.live.game.commerce.currency_ready);
        assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
    }

    #[test]
    fn sent_currency_conflict_and_missing_receipts_freeze_spending_until_actual_balance() {
        for timeout in [false, true] {
            let mut fixture = Fixture::new();
            fixture.open();
            assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
            let request = fixture.queued();
            fixture.send(&request);
            if timeout {
                fixture.event(ZoneEvent::Gameplay(GameplayEvent::Progression(
                    openeq_net::progression::ProgressionEvent::SkillValue {
                        wire_skill_id: 0,
                        value: 55,
                    },
                )));
                let effect = fixture.live.training.tick(Instant::now() + REQUEST_TIMEOUT);
                fixture.live.training_effect(effect);
            } else {
                fixture.event(ZoneEvent::Gameplay(GameplayEvent::Currency(Currency {
                    platinum: 100,
                    ..Default::default()
                })));
            }
            assert!(fixture.live.training_state().blocked().is_some());
            assert!(!fixture.live.game.commerce.currency_ready);
            assert_eq!(
                crate::commerce::total_copper(fixture.live.game.currency),
                100_000
            );
            assert!(!fixture.live.command(Command::BankerChange));
            fixture.event(ZoneEvent::Gameplay(GameplayEvent::Currency(Currency {
                platinum: 99,
                gold: 8,
                silver: 9,
                copper: 0,
            })));
            assert!(fixture.live.game.commerce.currency_ready);
            assert!(fixture.live.training_state().estimate().is_none());
            assert!(!fixture.live.train_selection(Selection::new(0, 0).unwrap()));
        }
    }

    #[test]
    fn death_before_dispatch_is_unsent_but_after_dispatch_money_is_uncertain() {
        for sent in [false, true] {
            let mut fixture = Fixture::new();
            fixture.open();
            assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
            let request = fixture.queued();
            if sent {
                fixture.send(&request);
            }
            fixture.event(ZoneEvent::Gameplay(GameplayEvent::Death(
                openeq_net::gameplay::Death {
                    id: 1,
                    killer_id: 3,
                    corpse_id: 1,
                    skill: 0,
                    spell_id: u32::MAX,
                    damage: 100,
                },
            )));
            assert!(
                fixture
                    .worker
                    .worker_claim(&request, fixture.motion.action_epoch, None)
                    .is_err()
            );
            assert_eq!(fixture.live.game.commerce.currency_ready, !sent);
            assert!(!fixture.live.movement_allowed());
            assert!(fixture.live.training_state().pending().is_none());
            assert!(fixture.live.training_state().estimate().is_none());
            fixture
                .io
                .tx
                .send(Message::TrainingSent(request.stamp))
                .unwrap();
            fixture.live.poll();
            assert!(fixture.live.training_state().pending().is_none());
        }
    }

    #[test]
    fn assessed_cost_above_funds_never_changes_shared_coins_or_refunds_skill() {
        let mut fixture = Fixture::new();
        fixture.open();
        assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
        let request = fixture.queued();
        fixture.send(&request);
        fixture.receipt(56, 100_001);
        assert_eq!(fixture.live.game.progression.skills[0], Some(56));
        assert_eq!(
            crate::commerce::total_copper(fixture.live.game.currency),
            100_000
        );
        assert!(!fixture.live.game.commerce.currency_ready);
        assert_eq!(
            fixture
                .live
                .training_state()
                .last_report()
                .unwrap()
                .completion
                .assessed_cost_copper,
            100_001
        );
    }

    #[test]
    fn clean_name_matches_eqemu_instead_of_display_name_rules() {
        assert_eq!(clean_trainer_name("#Warlord_Welorf000"), "Warlord Welorf");
        assert_eq!(clean_trainer_name("a`b's-2._x"), "a`bs x");
    }

    #[test]
    fn skill_receipt_before_dispatch_retires_selection_and_cannot_be_its_confirmation() {
        let mut fixture = Fixture::new();
        fixture.open();
        assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
        let stale = fixture.queued();
        fixture.event(ZoneEvent::Gameplay(GameplayEvent::Progression(
            openeq_net::progression::ProgressionEvent::SkillValue {
                wire_skill_id: 0,
                value: 56,
            },
        )));
        assert!(fixture.worker.worker_claim(&stale, 0, None).is_err());
        assert!(fixture.live.training_state().pending().is_none());
        assert!(fixture.live.training_state().blocked().is_none());
        assert_eq!(fixture.live.training_state().value(0), Some(56));
        assert_eq!(
            fixture
                .live
                .training_state()
                .estimate()
                .unwrap()
                .training_points,
            2
        );
        assert_eq!(
            crate::commerce::total_copper(fixture.live.game.currency),
            100_000
        );
        assert!(fixture.live.game.commerce.currency_ready);
        assert!(fixture.live.train_selection(Selection::new(0, 0).unwrap()));
        let current = fixture.queued();
        fixture.send(&current);
        fixture.receipt(57, 973);
        assert_eq!(
            fixture
                .live
                .training_state()
                .estimate()
                .unwrap()
                .training_points,
            1
        );
        assert_eq!(
            crate::commerce::total_copper(fixture.live.game.currency),
            99_027
        );
    }

    #[test]
    fn worker_timeout_precedes_late_receipts_when_foreground_polling_stalls() {
        let mut fixture = Fixture::new();
        fixture.open();
        let selection = Selection::new(0, 0).unwrap();
        // A proven-unsent foreground request creates different local counters.
        assert!(fixture.live.train_selection(selection));
        let unsent = fixture.queued();
        fixture
            .io
            .tx
            .send(Message::TrainingRejected {
                stamp: unsent.stamp,
                notice: "not sent".into(),
            })
            .unwrap();
        fixture.live.poll();
        assert!(fixture.live.train_selection(selection));
        let request = fixture.queued();
        let internal = fixture.send(&request);
        assert_ne!(request.stamp.operation_id, internal.operation_id);
        let expired = fixture
            .worker
            .worker_timeout(Instant::now() + REQUEST_TIMEOUT)
            .unwrap();
        assert_eq!(
            expired, request.stamp,
            "timeout must identify the foreground operation"
        );
        assert!(
            fixture
                .worker
                .worker_timeout(Instant::now() + REQUEST_TIMEOUT)
                .is_none()
        );
        fixture
            .io
            .tx
            .send(Message::TrainingTimedOut(expired))
            .unwrap();
        for event in [
            GameplayEvent::Progression(openeq_net::progression::ProgressionEvent::SkillValue {
                wire_skill_id: 0,
                value: 56,
            }),
            GameplayEvent::Training(TrainingEvent::Completed(Completion {
                wire_skill_id: 0,
                assessed_cost_copper: 911,
                new_skill: 0,
                trainer_name: "Warlord Welorf".into(),
            })),
        ] {
            let event = ZoneEvent::Gameplay(event);
            fixture.worker.zone_event(
                &event,
                fixture.motion.action_epoch,
                fixture.motion.own_id,
                "Player",
            );
            fixture.io.tx.send(Message::Event(Box::new(event))).unwrap();
        }
        assert!(
            fixture.live.training_state().pending().is_some(),
            "foreground has not consumed the timeout or late receipts yet"
        );
        fixture.live.poll();
        assert_eq!(
            fixture.live.training_state().blocked(),
            Some(BlockReason::TimedOut)
        );
        assert_eq!(
            fixture.worker.reducer.blocked(),
            Some(BlockReason::TimedOut)
        );
        assert_eq!(
            fixture.live.game.progression.skills[0],
            Some(56),
            "late skill remains authoritative"
        );
        assert_eq!(fixture.live.training_state().estimate(), None);
        assert_eq!(
            crate::commerce::total_copper(fixture.live.game.currency),
            100_000,
            "late receipts cannot debit an expired request"
        );
        assert!(!fixture.live.game.commerce.currency_ready);
        assert!(!fixture.live.train_selection(selection));
    }

    #[test]
    fn stale_worker_timeout_does_not_retire_a_new_or_completed_operation() {
        let mut fixture = Fixture::new();
        fixture.open();
        let selection = Selection::new(0, 0).unwrap();
        assert!(fixture.live.train_selection(selection));
        let first = fixture.queued();
        fixture.send(&first);
        fixture.receipt(56, 911);
        assert!(
            fixture
                .worker
                .worker_timeout(Instant::now() + REQUEST_TIMEOUT)
                .is_none()
        );
        assert!(fixture.live.train_selection(selection));
        let second = fixture.queued();
        fixture.send(&second);
        fixture
            .io
            .tx
            .send(Message::TrainingTimedOut(first.stamp))
            .unwrap();
        fixture.live.poll();
        assert_eq!(
            fixture.live.training_state().pending().unwrap().0,
            second.stamp
        );
        assert_eq!(fixture.live.training_state().blocked(), None);
        assert!(fixture.live.game.commerce.currency_ready);
        fixture.receipt(57, 973);
        fixture
            .io
            .tx
            .send(Message::TrainingTimedOut(second.stamp))
            .unwrap();
        fixture.live.poll();
        assert_eq!(fixture.live.training_state().blocked(), None);
        assert_eq!(
            fixture
                .live
                .training_state()
                .estimate()
                .unwrap()
                .training_points,
            0
        );
        assert_eq!(
            crate::commerce::total_copper(fixture.live.game.currency),
            98_116
        );
    }
}
