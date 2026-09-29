//! Pure death/recovery lifecycle. Server observations establish revival;
//! issuing a UI command only establishes a pending request.
use openeq_net::death::{DeathCommand, DeathEvent, RespawnWindow, ResurrectionOffer};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecoveryPhase {
    #[default]
    Alive,
    Dead,
    ChoosingRespawn,
    AwaitingRevival,
    Zoning,
    Disconnected,
}

/// Identifies the server state a UI frame was built from. Old dialog actions
/// cannot select options or answer offers introduced after that frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecoveryToken {
    pub generation: u64,
    pub revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryIntent {
    SelectOption(u32),
    PageOptions(i32),
    Respawn(u32),
    AcceptResurrection,
    DeclineResurrection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryAction {
    pub token: RecoveryToken,
    pub intent: RecoveryIntent,
}

#[derive(Clone, Debug)]
pub struct RecoveryRequest {
    pub token: RecoveryToken,
    pub command: DeathCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryError {
    StaleAction,
    RequestPending,
    InvalidOption,
    Unavailable,
    TimerElapsed,
}
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::StaleAction => "That recovery choice is no longer current.",
            Self::RequestPending => "Waiting for the server to finish recovery.",
            Self::InvalidOption => "That respawn option is unavailable.",
            Self::Unavailable => "No matching resurrection is available.",
            Self::TimerElapsed => "The respawn timer has elapsed; waiting for the server.",
        })
    }
}
impl std::error::Error for RecoveryError {}

#[derive(Clone, Debug)]
pub struct RecoveryChoiceView {
    pub id: u32,
    pub zone_id: u32,
    pub label: String,
    pub enabled: bool,
    pub selected: bool,
    pub requires_resurrection: bool,
}
#[derive(Clone, Debug)]
pub struct RespawnView {
    pub options: Vec<RecoveryChoiceView>,
    pub first_visible: usize,
    pub selected: Option<u32>,
    pub remaining_seconds: u64,
    pub timer_fraction: f32,
    pub can_respawn: bool,
}
#[derive(Clone, Debug)]
pub struct ResurrectionView {
    pub caster: String,
    pub corpse: String,
    pub spell_id: u32,
    pub zone_id: u16,
    pub hovering: bool,
    pub can_accept: bool,
    pub can_decline: bool,
}
#[derive(Clone, Debug)]
pub struct RecoveryView {
    pub token: RecoveryToken,
    pub phase: RecoveryPhase,
    pub respawn: Option<RespawnView>,
    pub resurrection: Option<ResurrectionView>,
    pub pending: bool,
    pub status: &'static str,
}

#[derive(Clone, Debug)]
struct Choices {
    window: RespawnWindow,
    received: Instant,
    selected: Option<u32>,
    first_visible: usize,
}
impl Choices {
    fn remaining(&self, now: Instant) -> Duration {
        Duration::from_millis(u64::from(self.window.remaining_ms))
            .saturating_sub(now.saturating_duration_since(self.received))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestKind {
    Respawn,
    LivingResurrection,
    Decline,
}
#[derive(Clone, Debug)]
struct Pending {
    token: RecoveryToken,
    previous: RecoveryPhase,
    kind: RequestKind,
}

#[derive(Clone, Debug, Default)]
pub struct RecoveryState {
    token: RecoveryToken,
    phase: RecoveryPhase,
    corpse_id: Option<u32>,
    zone: Option<(u16, u16)>,
    choices: Option<Choices>,
    offer: Option<Box<ResurrectionOffer>>,
    pending: Option<Pending>,
}

impl RecoveryState {
    pub fn phase(&self) -> RecoveryPhase {
        self.phase
    }
    pub fn token(&self) -> RecoveryToken {
        self.token
    }
    pub fn corpse_id(&self) -> Option<u32> {
        self.corpse_id
    }
    pub fn blocks_movement(&self) -> bool {
        self.phase != RecoveryPhase::Alive
    }

    fn revised(&mut self) {
        self.token.revision = self.token.revision.wrapping_add(1);
    }

    /// Call only for authoritative own-player Death, not ordinary zero-HP or
    /// NPC events. Duplicate death packets preserve an already-open window.
    pub fn own_death(&mut self, generation: u64, corpse_id: u32) {
        if generation != self.token.generation || self.phase == RecoveryPhase::Disconnected {
            return;
        }
        if self.corpse_id == Some(corpse_id) && self.phase != RecoveryPhase::Alive {
            return;
        }
        self.phase = RecoveryPhase::Dead;
        self.corpse_id = Some(corpse_id);
        self.choices = None;
        self.offer = None;
        self.pending = None;
        self.revised();
    }

    /// Returns whether a generation/recipient-valid observation was applied.
    /// Call with the current zone *and instance*. Resurrection offers stay in
    /// server coordinates; BindTransfer movement belongs to LiveWorld/transport.
    pub fn event(
        &mut self,
        generation: u64,
        character: &str,
        zone: Option<(u16, u16)>,
        event: &DeathEvent,
        now: Instant,
    ) -> bool {
        if generation != self.token.generation || self.phase == RecoveryPhase::Disconnected {
            return false;
        }
        self.zone = zone;
        match event {
            DeathEvent::RespawnWindow(window) => {
                if self.pending.is_some()
                    || !matches!(
                        self.phase,
                        RecoveryPhase::Dead | RecoveryPhase::ChoosingRespawn
                    )
                    || window.options.is_empty()
                {
                    return false;
                }
                if self
                    .choices
                    .as_ref()
                    .is_some_and(|old| old.window == *window)
                {
                    // Transport retransmissions must not extend the countdown.
                    return true;
                }
                let selected = window
                    .options
                    .iter()
                    .find(|option| {
                        option.id == window.initial_selection && !option.requires_resurrection
                    })
                    .or_else(|| {
                        window
                            .options
                            .iter()
                            .find(|option| !option.requires_resurrection)
                    })
                    .map(|option| option.id);
                self.choices = Some(Choices {
                    window: window.clone(),
                    received: now,
                    selected,
                    first_visible: 0,
                });
                self.phase = RecoveryPhase::ChoosingRespawn;
                self.revised();
            }
            DeathEvent::BindTransfer(_) => {
                self.phase = RecoveryPhase::AwaitingRevival;
                self.choices = None;
                self.offer = None;
                self.pending = None;
                self.revised();
            }
            DeathEvent::ResurrectionOffer(offer) => {
                if character.is_empty()
                    || !offer.recipient().eq_ignore_ascii_case(character)
                    || offer.caster().is_empty()
                    || offer.corpse().is_empty()
                    || offer.zone_id() == 0
                    || matches!(offer.spell_id(), 0 | u32::MAX)
                    || offer.action() != 0
                    || !matches!(
                        self.phase,
                        RecoveryPhase::Alive | RecoveryPhase::Dead | RecoveryPhase::ChoosingRespawn
                    )
                {
                    return false;
                }
                if self.offer.as_ref() == Some(offer) {
                    return true;
                }
                if self.pending.is_some() {
                    return false;
                }
                self.offer = Some(offer.clone());
                self.revised();
            }
        }
        true
    }

    /// Zone generations are supplied by LiveWorld, not UI. A transfer invalidates
    /// every offer/choice even when the destination uses the same zone assets.
    pub fn begin_zone(&mut self, generation: u64) {
        if generation < self.token.generation {
            return;
        }
        self.token.generation = generation;
        self.phase = RecoveryPhase::Zoning;
        self.corpse_id = None;
        self.zone = None;
        self.choices = None;
        self.offer = None;
        self.pending = None;
        self.revised();
    }

    /// The caller must already establish that this is a living own spawn. A
    /// same-generation spawn reusing the recorded corpse ID cannot revive us.
    pub fn own_spawn(&mut self, generation: u64, id: u32) -> bool {
        if generation != self.token.generation
            || self.phase == RecoveryPhase::Disconnected
            || (self.phase != RecoveryPhase::Alive && self.corpse_id == Some(id))
        {
            return false;
        }
        if self.phase == RecoveryPhase::Alive {
            return true;
        }
        self.phase = RecoveryPhase::Alive;
        self.corpse_id = None;
        self.choices = None;
        self.offer = None;
        self.pending = None;
        self.revised();
        true
    }

    /// Completes a living player's accepted resurrection on an authoritative
    /// same-zone teleport. A dead/hovering player still requires a fresh spawn.
    pub fn relocated(&mut self, generation: u64) {
        if generation == self.token.generation
            && self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.kind == RequestKind::LivingResurrection)
        {
            self.phase = RecoveryPhase::Alive;
            self.offer = None;
            self.pending = None;
            self.revised();
        }
    }

    pub fn disconnect(&mut self) {
        self.phase = RecoveryPhase::Disconnected;
        self.choices = None;
        self.offer = None;
        self.pending = None;
        self.revised();
    }

    fn hover_rez_option(&self) -> Option<u32> {
        let offer = self.offer.as_ref()?;
        if self.zone != Some((offer.zone_id(), offer.instance_id())) {
            return None;
        }
        self.choices
            .as_ref()?
            .window
            .options
            .last()
            .filter(|option| {
                option.requires_resurrection && option.zone_id == u32::from(offer.zone_id())
            })
            .map(|option| option.id)
    }

    fn option_available(&self, id: u32, now: Instant) -> bool {
        self.phase == RecoveryPhase::ChoosingRespawn
            && self.pending.is_none()
            && self.choices.as_ref().is_some_and(|choices| {
                !choices.remaining(now).is_zero()
                    && choices.window.options.iter().any(|option| {
                        option.id == id
                            && (!option.requires_resurrection
                                || self.hover_rez_option() == Some(id))
                    })
            })
    }

    pub fn act(
        &mut self,
        action: RecoveryAction,
        now: Instant,
    ) -> Result<Option<RecoveryRequest>, RecoveryError> {
        if action.token != self.token {
            return Err(RecoveryError::StaleAction);
        }
        if self.pending.is_some() {
            return Err(RecoveryError::RequestPending);
        }
        let (command, kind) = match action.intent {
            RecoveryIntent::PageOptions(delta) => {
                let choices = self.choices.as_mut().ok_or(RecoveryError::InvalidOption)?;
                choices.first_visible = (choices.first_visible as i64 + i64::from(delta))
                    .clamp(0, choices.window.options.len().saturating_sub(1) as i64)
                    as usize;
                return Ok(None);
            }
            RecoveryIntent::SelectOption(id) | RecoveryIntent::Respawn(id) => {
                if self
                    .choices
                    .as_ref()
                    .is_some_and(|choices| choices.remaining(now).is_zero())
                {
                    return Err(RecoveryError::TimerElapsed);
                }
                if !self.option_available(id, now) {
                    return Err(RecoveryError::InvalidOption);
                }
                if matches!(action.intent, RecoveryIntent::SelectOption(_)) {
                    self.choices.as_mut().unwrap().selected = Some(id);
                    return Ok(None);
                }
                if self.choices.as_ref().and_then(|choices| choices.selected) != Some(id) {
                    return Err(RecoveryError::StaleAction);
                }
                (
                    DeathCommand::SelectRespawn { option_id: id },
                    RequestKind::Respawn,
                )
            }
            RecoveryIntent::AcceptResurrection => {
                let offer = self.offer.as_ref().ok_or(RecoveryError::Unavailable)?;
                if self.phase == RecoveryPhase::Alive {
                    (
                        DeathCommand::AnswerResurrection {
                            offer: offer.clone(),
                            accept: true,
                        },
                        RequestKind::LivingResurrection,
                    )
                } else if self.phase == RecoveryPhase::ChoosingRespawn {
                    let id = self.hover_rez_option().ok_or(RecoveryError::Unavailable)?;
                    if !self.option_available(id, now) {
                        return Err(RecoveryError::TimerElapsed);
                    }
                    (
                        DeathCommand::SelectRespawn { option_id: id },
                        RequestKind::Respawn,
                    )
                } else {
                    return Err(RecoveryError::Unavailable);
                }
            }
            RecoveryIntent::DeclineResurrection => {
                if !matches!(
                    self.phase,
                    RecoveryPhase::Alive | RecoveryPhase::Dead | RecoveryPhase::ChoosingRespawn
                ) {
                    return Err(RecoveryError::Unavailable);
                }
                let offer = self.offer.as_ref().ok_or(RecoveryError::Unavailable)?;
                (
                    DeathCommand::AnswerResurrection {
                        offer: offer.clone(),
                        accept: false,
                    },
                    RequestKind::Decline,
                )
            }
        };
        let request = RecoveryRequest {
            token: self.token,
            command,
        };
        self.pending = Some(Pending {
            token: request.token,
            previous: self.phase,
            kind,
        });
        if kind != RequestKind::Decline {
            self.phase = RecoveryPhase::AwaitingRevival;
        }
        Ok(Some(request))
    }

    pub fn request_pending(&self, token: RecoveryToken) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| pending.token == token)
    }

    /// Declines have no server acknowledgment. Successful dispatch can close
    /// that prompt, but accept/select dispatch never establishes revival.
    pub fn request_sent(&mut self, token: RecoveryToken) {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.token == token && pending.kind == RequestKind::Decline)
        {
            self.offer = None;
            self.pending = None;
            self.revised();
            if let Some(choices) = &mut self.choices {
                choices.selected = choices
                    .window
                    .options
                    .iter()
                    .find(|option| !option.requires_resurrection)
                    .map(|option| option.id);
            }
        }
    }

    /// Only call for a known-unsent or rejected request. An uncertain network
    /// failure should disconnect instead of silently allowing a duplicate send.
    pub fn request_failed(&mut self, token: RecoveryToken) {
        if self.request_pending(token) {
            let pending = self.pending.take().unwrap();
            self.phase = pending.previous;
            self.revised();
        }
    }

    pub fn view(&self, now: Instant) -> RecoveryView {
        let respawn = self.choices.as_ref().map(|choices| {
            let remaining = choices.remaining(now);
            let seconds = remaining.as_secs() + u64::from(remaining.subsec_nanos() != 0);
            RespawnView {
                first_visible: choices.first_visible,
                options: choices
                    .window
                    .options
                    .iter()
                    .map(|option| RecoveryChoiceView {
                        id: option.id,
                        zone_id: option.zone_id,
                        label: option.label.clone(),
                        enabled: self.option_available(option.id, now),
                        selected: choices.selected == Some(option.id),
                        requires_resurrection: option.requires_resurrection,
                    })
                    .collect(),
                selected: choices.selected,
                remaining_seconds: seconds,
                timer_fraction: if choices.window.remaining_ms == 0 {
                    0.
                } else {
                    (remaining.as_secs_f32() * 1000. / choices.window.remaining_ms as f32)
                        .clamp(0., 1.)
                },
                can_respawn: choices
                    .selected
                    .is_some_and(|id| self.option_available(id, now)),
            }
        });
        let resurrection = self.offer.as_ref().map(|offer| ResurrectionView {
            caster: offer.caster().into(),
            corpse: offer.corpse().into(),
            spell_id: offer.spell_id(),
            zone_id: offer.zone_id(),
            hovering: self.phase != RecoveryPhase::Alive,
            can_accept: self.pending.is_none()
                && (self.phase == RecoveryPhase::Alive
                    || self
                        .hover_rez_option()
                        .is_some_and(|id| self.option_available(id, now))),
            can_decline: self.pending.is_none()
                && matches!(
                    self.phase,
                    RecoveryPhase::Alive | RecoveryPhase::Dead | RecoveryPhase::ChoosingRespawn
                ),
        });
        RecoveryView {
            token: self.token,
            phase: self.phase,
            respawn,
            resurrection,
            pending: self.pending.is_some(),
            status: match self.phase {
                RecoveryPhase::Alive => "",
                RecoveryPhase::Dead => "You have died. Waiting for the server…",
                RecoveryPhase::ChoosingRespawn
                    if self
                        .choices
                        .as_ref()
                        .is_some_and(|choices| choices.remaining(now).is_zero()) =>
                {
                    "Waiting for the server’s respawn timer…"
                }
                RecoveryPhase::ChoosingRespawn => "Choose a respawn location.",
                RecoveryPhase::AwaitingRevival => "Waiting for the server to return you to play…",
                RecoveryPhase::Zoning => "Traveling. Waiting for your character to arrive…",
                RecoveryPhase::Disconnected => "Recovery interrupted. Reconnect to continue.",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_net::death::{BindTransfer, OP_REZZ_REQUEST, RespawnOption, parse_packet};

    fn window() -> DeathEvent {
        DeathEvent::RespawnWindow(RespawnWindow {
            initial_selection: 1,
            remaining_ms: 300_000,
            options: vec![
                RespawnOption {
                    id: 0,
                    zone_id: 202,
                    position: [1., 2., 3.],
                    heading: 0.,
                    label: "Bind Location".into(),
                    requires_resurrection: false,
                },
                RespawnOption {
                    id: 1,
                    zone_id: 202,
                    position: [4., 5., 6.],
                    heading: 0.,
                    label: "Resurrect".into(),
                    requires_resurrection: true,
                },
            ],
        })
    }
    fn offer(recipient: &str, zone: u16, instance: u16, spell: u32) -> DeathEvent {
        let mut bytes = vec![0; 236];
        bytes[4..6].copy_from_slice(&zone.to_le_bytes());
        bytes[6..8].copy_from_slice(&instance.to_le_bytes());
        bytes[24..24 + recipient.len()].copy_from_slice(recipient.as_bytes());
        bytes[92..98].copy_from_slice(b"Cleric");
        bytes[156..160].copy_from_slice(&spell.to_le_bytes());
        bytes[160..177].copy_from_slice(b"Player's corpse42");
        parse_packet(OP_REZZ_REQUEST, &bytes).unwrap().unwrap()
    }
    fn bind() -> DeathEvent {
        DeathEvent::BindTransfer(BindTransfer {
            zone_id: 202,
            instance_id: 0,
            position: [4., 5., 6.],
            heading: 0.,
            label: "Resurrect".into(),
            save_items: 1,
            resources: [0; 3],
        })
    }
    fn observe(state: &mut RecoveryState, event: &DeathEvent, now: Instant) -> bool {
        state.event(0, "Player", Some((202, 0)), event, now)
    }
    fn action(state: &RecoveryState, intent: RecoveryIntent) -> RecoveryAction {
        RecoveryAction {
            token: state.token(),
            intent,
        }
    }
    fn hovering(now: Instant) -> RecoveryState {
        let mut state = RecoveryState::default();
        state.own_death(0, 41);
        assert!(observe(&mut state, &window(), now));
        state
    }

    #[test]
    fn death_and_repeated_window_are_idempotent_and_do_not_extend_timer() {
        let now = Instant::now();
        let mut state = hovering(now);
        let token = state.token();
        state.own_death(0, 41);
        assert_eq!(state.token(), token);
        assert!(state.blocks_movement());
        assert!(observe(
            &mut state,
            &window(),
            now + Duration::from_secs(15)
        ));
        let view = state.view(now + Duration::from_secs(20));
        assert_eq!(view.token, token);
        let options = view.respawn.unwrap();
        assert_eq!(options.remaining_seconds, 280);
        assert_eq!(
            options.selected,
            Some(0),
            "disabled initial resurrection falls back to bind"
        );
        assert!(!options.options[1].enabled);
        assert_eq!(state.corpse_id(), Some(41));
    }

    #[test]
    fn hover_accept_uses_only_selection_and_waits_for_new_living_identity() {
        let now = Instant::now();
        let mut state = hovering(now);
        assert!(observe(&mut state, &offer("pLaYeR", 202, 0, 388), now));
        let intent = action(&state, RecoveryIntent::AcceptResurrection);
        let request = state.act(intent, now).unwrap().unwrap();
        assert!(matches!(
            request.command,
            DeathCommand::SelectRespawn { option_id: 1 }
        ));
        assert!(state.request_pending(request.token));
        assert_eq!(
            state.act(intent, now).unwrap_err(),
            RecoveryError::RequestPending
        );
        state.request_sent(request.token);
        assert_eq!(state.phase(), RecoveryPhase::AwaitingRevival);
        assert!(!state.own_spawn(0, 41));
        state.relocated(0);
        assert!(
            state.blocks_movement(),
            "hover teleport alone is not a fresh player"
        );
        assert!(observe(&mut state, &bind(), now));
        assert!(!state.own_spawn(0, 41));
        assert!(state.own_spawn(0, 42));
        assert!(!state.blocks_movement());
        assert!(state.view(now).respawn.is_none());
        assert!(state.view(now).resurrection.is_none());
        state.request_failed(request.token);
        assert_eq!(
            state.phase(),
            RecoveryPhase::Alive,
            "late send failure cannot undo revival"
        );
    }

    #[test]
    fn living_accept_uses_answer_and_only_server_relocation_completes_it() {
        let now = Instant::now();
        let mut state = RecoveryState::default();
        assert!(observe(&mut state, &offer("Player", 202, 0, 388), now));
        let request = state
            .act(action(&state, RecoveryIntent::AcceptResurrection), now)
            .unwrap()
            .unwrap();
        assert!(matches!(
            request.command,
            DeathCommand::AnswerResurrection { accept: true, .. }
        ));
        state.request_sent(request.token);
        assert!(state.blocks_movement());
        state.relocated(1);
        assert!(
            state.blocks_movement(),
            "stale generation cannot teleport us"
        );
        state.relocated(0);
        assert!(!state.blocks_movement());
        assert!(!state.request_pending(request.token));
        assert!(state.view(now).resurrection.is_none());
    }

    #[test]
    fn recipient_generation_and_offer_replacement_invalidate_old_ui_actions() {
        let now = Instant::now();
        let mut state = RecoveryState::default();
        assert!(!observe(
            &mut state,
            &offer("SomeoneElse", 202, 0, 388),
            now
        ));
        assert!(!observe(&mut state, &offer("Player", 202, 0, 0), now));
        assert!(state.view(now).resurrection.is_none());
        assert!(observe(&mut state, &offer("Player", 202, 0, 388), now));
        let old = action(&state, RecoveryIntent::AcceptResurrection);
        assert!(observe(&mut state, &offer("Player", 202, 0, 391), now));
        assert_eq!(state.act(old, now).unwrap_err(), RecoveryError::StaleAction);
        let current = action(&state, RecoveryIntent::AcceptResurrection);
        state.begin_zone(1);
        assert!(!state.event(
            0,
            "Player",
            Some((202, 0)),
            &offer("Player", 202, 0, 388),
            now
        ));
        assert_eq!(
            state.act(current, now).unwrap_err(),
            RecoveryError::StaleAction
        );
        assert!(state.view(now).resurrection.is_none());
        assert!(!state.own_spawn(0, 55));
        assert!(state.own_spawn(1, 55));
        state.begin_zone(0);
        assert_eq!(state.phase(), RecoveryPhase::Alive);
    }

    #[test]
    fn cross_zone_or_instance_hover_offer_is_decline_only() {
        let now = Instant::now();
        for destination in [(203, 0), (202, 7)] {
            let mut state = hovering(now);
            assert!(observe(
                &mut state,
                &offer("Player", destination.0, destination.1, 388),
                now
            ));
            let view = state.view(now);
            let rez = view.resurrection.unwrap();
            assert!(!rez.can_accept);
            assert!(rez.can_decline);
            assert!(!view.respawn.unwrap().options[1].enabled);
            assert_eq!(
                state
                    .act(action(&state, RecoveryIntent::AcceptResurrection), now)
                    .unwrap_err(),
                RecoveryError::Unavailable
            );
            let request = state
                .act(action(&state, RecoveryIntent::DeclineResurrection), now)
                .unwrap()
                .unwrap();
            assert!(matches!(
                request.command,
                DeathCommand::AnswerResurrection { accept: false, .. }
            ));
            state.request_sent(request.token);
            assert_eq!(state.phase(), RecoveryPhase::ChoosingRespawn);
            assert!(state.view(now).resurrection.is_none());
            assert!(state.view(now).respawn.unwrap().can_respawn);
        }
    }

    #[test]
    fn decline_needs_dispatch_but_never_requires_a_server_ack() {
        let now = Instant::now();
        let mut state = RecoveryState::default();
        observe(&mut state, &offer("Player", 202, 0, 388), now);
        let intent = action(&state, RecoveryIntent::DeclineResurrection);
        let request = state.act(intent, now).unwrap().unwrap();
        assert!(!state.blocks_movement());
        assert!(state.view(now).resurrection.is_some());
        assert_eq!(
            state.act(intent, now).unwrap_err(),
            RecoveryError::RequestPending
        );
        state.request_sent(request.token);
        assert!(state.view(now).resurrection.is_none());
        assert_eq!(
            state.act(intent, now).unwrap_err(),
            RecoveryError::StaleAction
        );
    }

    #[test]
    fn failed_unsent_request_allows_only_a_new_current_action() {
        let now = Instant::now();
        let mut state = hovering(now);
        let intent = action(&state, RecoveryIntent::Respawn(0));
        let request = state.act(intent, now).unwrap().unwrap();
        state.request_failed(RecoveryToken {
            revision: 999,
            ..request.token
        });
        assert!(state.request_pending(request.token));
        state.request_failed(request.token);
        assert_eq!(state.phase(), RecoveryPhase::ChoosingRespawn);
        assert_eq!(
            state.act(intent, now).unwrap_err(),
            RecoveryError::StaleAction
        );
        let retry = state
            .act(action(&state, RecoveryIntent::Respawn(0)), now)
            .unwrap()
            .unwrap();
        state.request_failed(request.token);
        assert!(state.request_pending(retry.token));
    }

    #[test]
    fn countdown_clamps_without_ever_sending_timeout_selection() {
        let now = Instant::now();
        let mut state = hovering(now);
        let before = state.token();
        for seconds in [0, 1, 299, 300, 900] {
            let view = state.view(now + Duration::from_secs(seconds));
            let choices = view.respawn.unwrap();
            assert_eq!(choices.remaining_seconds, 300_u64.saturating_sub(seconds));
            assert!((0. ..=1.).contains(&choices.timer_fraction));
            assert_eq!(choices.can_respawn, seconds < 300);
            assert!(!view.pending);
            assert_eq!(state.token(), before);
        }
        assert_eq!(
            state
                .act(
                    action(&state, RecoveryIntent::Respawn(0)),
                    now + Duration::from_secs(300)
                )
                .unwrap_err(),
            RecoveryError::TimerElapsed
        );
        assert_eq!(state.phase(), RecoveryPhase::ChoosingRespawn);
    }

    #[test]
    fn selection_and_confirmation_are_bound_to_the_displayed_option() {
        let now = Instant::now();
        let mut state = hovering(now);
        assert_eq!(
            state
                .act(action(&state, RecoveryIntent::SelectOption(1)), now)
                .unwrap_err(),
            RecoveryError::InvalidOption
        );
        observe(&mut state, &offer("Player", 202, 0, 388), now);
        let old_confirmation = action(&state, RecoveryIntent::Respawn(0));
        assert!(
            state
                .act(action(&state, RecoveryIntent::SelectOption(1)), now)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            state.act(old_confirmation, now).unwrap_err(),
            RecoveryError::StaleAction
        );
        let request = state
            .act(action(&state, RecoveryIntent::Respawn(1)), now)
            .unwrap()
            .unwrap();
        assert!(matches!(
            request.command,
            DeathCommand::SelectRespawn { option_id: 1 }
        ));
    }

    #[test]
    fn disconnect_clears_offers_and_requires_a_fresh_generation() {
        let now = Instant::now();
        let mut state = hovering(now);
        observe(&mut state, &offer("Player", 202, 0, 388), now);
        let old = action(&state, RecoveryIntent::Respawn(0));
        state.disconnect();
        assert!(state.blocks_movement());
        assert!(state.view(now).respawn.is_none());
        assert!(!observe(&mut state, &offer("Player", 202, 0, 388), now));
        assert!(!state.own_spawn(0, 50));
        assert_eq!(state.act(old, now).unwrap_err(), RecoveryError::StaleAction);
        state.begin_zone(1);
        assert!(
            state.own_spawn(1, 41),
            "entity IDs may be reused in a new generation"
        );
        assert!(!state.blocks_movement());
        assert!(!state.event(0, "Player", Some((202, 0)), &window(), now));
        assert!(
            !state.event(1, "Player", Some((202, 0)), &window(), now),
            "stale respawn window cannot kill a living player"
        );
    }
}
