//! One stock trainer request at a time, with explicit snapshot provenance.
//!
//! The protocol has no transaction ID or final balance. A matched skill receipt
//! and completion support a session estimate, never an authoritative payment
//! claim. Transport must preserve its ordinary ordering/deduplication: an
//! arbitrary delayed replay during a later identical purchase is not detectable.
use openeq_net::{
    gameplay::{Currency, PlayerProfile},
    training::{Bank, Completion, SKILL_COUNT, Selection, TrainingCommand, TrainingEvent},
};

const LANGUAGE_COUNT: usize = 28;

#[derive(Clone, Copy, Debug)]
pub struct ProfileSnapshot<'a> {
    pub training_points: u32,
    pub currency: Currency,
    pub skills: &'a [u32],
    pub languages: &'a [u8],
}
impl<'a> From<&'a PlayerProfile> for ProfileSnapshot<'a> {
    fn from(profile: &'a PlayerProfile) -> Self {
        Self {
            training_points: profile.training_points,
            currency: profile.currency,
            skills: &profile.skills,
            languages: &profile.languages,
        }
    }
}

pub fn carried_copper(currency: Currency) -> u64 {
    u64::from(currency.platinum) * 1000
        + u64::from(currency.gold) * 100
        + u64::from(currency.silver) * 10
        + u64::from(currency.copper)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainerIdentity {
    pub id: u32,
    /// Caller-owned spawn incarnation, changed on despawn/reuse.
    pub revision: u64,
    /// Exact expected EQEmu clean name, not a decorated target display label.
    pub clean_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp {
    pub epoch: u64,
    pub profile_revision: u64,
    pub operation_id: u64,
    pub trainer_id: u32,
    pub trainer_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileBalances {
    pub training_points: u32,
    pub carried_copper: u64,
}

/// Derived from the profile and matched completions, not a fresh server balance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionEstimate {
    pub training_points: u32,
    pub carried_copper: u64,
    pub matched_purchases: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrainingPreview {
    pub base_value: u32,
    /// Source-derived stock estimate; the server has supplied no price quote.
    pub estimated_cost_copper: u64,
}

pub fn estimated_cost_copper(selection: Selection, value: u32) -> Option<u64> {
    if selection.bank() == Bank::Skill && value == 0 {
        return Some(0);
    }
    // EQEmu multiplies in signed int before division. Do not authorize custom
    // values that would overflow that intermediate, even if a u64 could hold it.
    u64::from(value.saturating_sub(10))
        .checked_pow(3)
        .filter(|cube| *cube <= i32::MAX as u64)
        .map(|cost| cost / 100)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchState {
    Prepared,
    Claimed,
    Sent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReason {
    UnexpectedReply,
    UnexpectedSkillValue,
    ConcurrentCurrencyUpdate,
    AssessedCostExceedsFunds,
    NoKnownPracticeBalance,
    TransportUncertain,
    TimedOut,
    ClosedDuringRequest,
    TrainerChanged,
    LevelChanged,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingError {
    StaleEpoch,
    MissingProfile,
    Blocked(BlockReason),
    Busy,
    NotOpen,
    InvalidTrainer,
    NoPractices,
    MissingValue,
    AtReportedMaximum,
    LanguageAtCap,
    InsufficientEstimatedFunds,
    CostEstimateOverflow,
    StaleStamp,
    AlreadyClaimed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Ignored,
    Updated,
    Opened,
    Completed,
    Blocked(BlockReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurchaseReport {
    pub stamp: Stamp,
    pub completion: Completion,
    /// Present only if an authoritative progress receipt was correlated.
    pub received_value: Option<u32>,
    /// Present only after both receipts match and the cost fits known funds.
    pub estimate_after: Option<SessionEstimate>,
}

#[derive(Clone, Debug)]
struct Pending {
    stamp: Stamp,
    command: TrainingCommand,
    trainer: TrainerIdentity,
    dispatch: DispatchState,
    old_value: Option<u32>,
    last_value_receipt: Option<u32>,
    received_value: Option<u32>,
    completion: Option<Completion>,
}

#[derive(Clone, Debug)]
pub struct TrainingState {
    epoch: u64,
    player_id: u32,
    profile_revision: u64,
    next_operation: u64,
    profile: Option<ProfileBalances>,
    /// Last actual profile/MoneyUpdate balance, not reduced by trainer receipts.
    observed_copper: Option<u64>,
    estimate: Option<SessionEstimate>,
    skills: [Option<u32>; SKILL_COUNT],
    languages: [Option<u32>; LANGUAGE_COUNT],
    trainer: Option<TrainerIdentity>,
    reported_maxima: Option<[u32; SKILL_COUNT]>,
    pending: Option<Pending>,
    last_report: Option<PurchaseReport>,
    blocked: Option<BlockReason>,
}

impl TrainingState {
    pub fn new(epoch: u64, player_id: u32) -> Self {
        Self {
            epoch,
            player_id,
            profile_revision: 0,
            next_operation: 0,
            profile: None,
            observed_copper: None,
            estimate: None,
            skills: [None; SKILL_COUNT],
            languages: [None; LANGUAGE_COUNT],
            trainer: None,
            reported_maxima: None,
            pending: None,
            last_report: None,
            blocked: None,
        }
    }

    /// Call on connection, zone, or character replacement. Old callbacks cannot
    /// claim or complete a new request, even if entity IDs are reused.
    pub fn begin_epoch(&mut self, epoch: u64, player_id: u32) {
        let revision = self.profile_revision.wrapping_add(1);
        let next_operation = self.next_operation;
        *self = Self::new(epoch, player_id);
        self.profile_revision = revision;
        self.next_operation = next_operation;
    }

    /// The profile can precede the own-spawn packet. Bind its identity without
    /// discarding that profile; replacing an established player needs a new epoch.
    pub fn bind_player(&mut self, epoch: u64, player_id: u32) -> bool {
        if epoch != self.epoch
            || player_id == 0
            || (self.player_id != 0 && self.player_id != player_id)
        {
            return false;
        }
        self.player_id = player_id;
        true
    }

    /// A fresh profile is the recovery baseline. Reopening a trainer alone never
    /// restores balances lost to ambiguity. The caller must reject stale profiles.
    pub fn apply_profile(&mut self, epoch: u64, profile: ProfileSnapshot<'_>) -> bool {
        if epoch != self.epoch {
            return false;
        }
        self.profile_revision = self.profile_revision.wrapping_add(1);
        let carried_copper = carried_copper(profile.currency);
        self.profile = Some(ProfileBalances {
            training_points: profile.training_points,
            carried_copper,
        });
        self.observed_copper = Some(carried_copper);
        self.estimate = Some(SessionEstimate {
            training_points: profile.training_points,
            carried_copper,
            matched_purchases: 0,
        });
        self.skills = std::array::from_fn(|id| profile.skills.get(id).copied());
        self.languages =
            std::array::from_fn(|id| profile.languages.get(id).copied().map(u32::from));
        self.trainer = None;
        self.reported_maxima = None;
        self.pending = None;
        self.last_report = None;
        self.blocked = None;
        true
    }

    pub fn profile_balances(&self) -> Option<ProfileBalances> {
        self.profile
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn profile_revision(&self) -> u64 {
        self.profile_revision
    }
    pub fn observed_copper(&self) -> Option<u64> {
        self.observed_copper
    }
    pub fn estimate(&self) -> Option<SessionEstimate> {
        self.estimate
    }
    pub fn blocked(&self) -> Option<BlockReason> {
        self.blocked
    }
    pub fn trainer(&self) -> Option<&TrainerIdentity> {
        self.trainer.as_ref()
    }
    pub fn active_trainer(&self) -> Option<&TrainerIdentity> {
        self.pending
            .as_ref()
            .map(|p| &p.trainer)
            .or(self.trainer.as_ref())
    }
    pub fn reported_maxima(&self) -> Option<&[u32; SKILL_COUNT]> {
        self.reported_maxima.as_ref()
    }
    pub fn last_report(&self) -> Option<&PurchaseReport> {
        self.last_report.as_ref()
    }
    pub fn pending(&self) -> Option<(Stamp, TrainingCommand, DispatchState)> {
        self.pending
            .as_ref()
            .map(|p| (p.stamp, p.command, p.dispatch))
    }
    pub fn pending_value_receipt(&self) -> Option<u32> {
        self.pending.as_ref().and_then(|p| p.last_value_receipt)
    }
    pub fn value(&self, wire_id: u32) -> Option<u32> {
        match wire_id {
            0..=77 => self.skills[wire_id as usize],
            100..=127 => self.languages[(wire_id - 100) as usize],
            _ => None,
        }
    }

    fn ready(&self, epoch: u64) -> Result<(), TrainingError> {
        if epoch != self.epoch {
            return Err(TrainingError::StaleEpoch);
        }
        if self.profile.is_none() {
            return Err(TrainingError::MissingProfile);
        }
        if let Some(reason) = self.blocked {
            return Err(TrainingError::Blocked(reason));
        }
        if self.pending.is_some() {
            return Err(TrainingError::Busy);
        }
        Ok(())
    }

    fn prepare(
        &mut self,
        trainer: TrainerIdentity,
        command: TrainingCommand,
        old_value: Option<u32>,
    ) -> Stamp {
        self.next_operation = self.next_operation.wrapping_add(1);
        let stamp = Stamp {
            epoch: self.epoch,
            profile_revision: self.profile_revision,
            operation_id: self.next_operation,
            trainer_id: trainer.id,
            trainer_revision: trainer.revision,
        };
        self.pending = Some(Pending {
            stamp,
            command,
            trainer,
            dispatch: DispatchState::Prepared,
            old_value,
            last_value_receipt: None,
            received_value: None,
            completion: None,
        });
        stamp
    }

    pub fn request_open(
        &mut self,
        epoch: u64,
        trainer: TrainerIdentity,
    ) -> Result<Stamp, TrainingError> {
        self.ready(epoch)?;
        if trainer.id == 0
            || trainer.id == self.player_id
            || self.player_id == 0
            || trainer.clean_name.is_empty()
            || trainer.clean_name.len() >= 64
            || trainer.clean_name.contains('\0')
        {
            return Err(TrainingError::InvalidTrainer);
        }
        let command = TrainingCommand::Open {
            trainer_id: trainer.id,
            player_id: self.player_id,
        };
        self.trainer = None;
        self.reported_maxima = None;
        Ok(self.prepare(trainer, command, None))
    }

    /// Pure local eligibility/estimate check, also used by request_train. Server
    /// class, range, rule and current-level restrictions still apply at execution.
    pub fn preview(
        &self,
        epoch: u64,
        selection: Selection,
    ) -> Result<TrainingPreview, TrainingError> {
        self.ready(epoch)?;
        let trainer = self.trainer.as_ref().ok_or(TrainingError::NotOpen)?;
        if trainer.id > u32::from(u16::MAX) {
            return Err(TrainingError::InvalidTrainer);
        }
        let estimate = self.estimate.ok_or(TrainingError::MissingProfile)?;
        if estimate.training_points == 0 {
            return Err(TrainingError::NoPractices);
        }
        let value = self
            .value(selection.wire_id())
            .ok_or(TrainingError::MissingValue)?;
        match selection.bank() {
            Bank::Language if value >= 100 => return Err(TrainingError::LanguageAtCap),
            Bank::Skill => {
                let max = self
                    .reported_maxima
                    .as_ref()
                    .ok_or(TrainingError::NotOpen)?[usize::from(selection.id())];
                if max == 0 || value >= max {
                    return Err(TrainingError::AtReportedMaximum);
                }
            }
            Bank::Language => {}
        }
        let cost =
            estimated_cost_copper(selection, value).ok_or(TrainingError::CostEstimateOverflow)?;
        if cost > estimate.carried_copper {
            return Err(TrainingError::InsufficientEstimatedFunds);
        }
        Ok(TrainingPreview {
            base_value: value,
            estimated_cost_copper: cost,
        })
    }

    pub fn request_train(
        &mut self,
        epoch: u64,
        selection: Selection,
    ) -> Result<Stamp, TrainingError> {
        let preview = self.preview(epoch, selection)?;
        let trainer = self.trainer.clone().unwrap();
        let command = TrainingCommand::Train {
            trainer_id: trainer.id,
            selection,
        };
        Ok(self.prepare(trainer, command, Some(preview.base_value)))
    }

    /// Consume the only dispatch claim immediately before sending. Revalidate
    /// the live trainer incarnation here; class, range and player state remain
    /// caller gates. A subsequent transport error is uncertain, never retryable.
    pub fn claim(
        &mut self,
        stamp: Stamp,
        current_trainer: &TrainerIdentity,
    ) -> Result<TrainingCommand, TrainingError> {
        let pending = self
            .pending
            .as_ref()
            .filter(|p| p.stamp == stamp)
            .ok_or(TrainingError::StaleStamp)?;
        if pending.dispatch != DispatchState::Prepared {
            return Err(TrainingError::AlreadyClaimed);
        }
        if &pending.trainer != current_trainer {
            self.freeze(BlockReason::TrainerChanged);
            return Err(TrainingError::Blocked(BlockReason::TrainerChanged));
        }
        let pending = self.pending.as_mut().unwrap();
        pending.dispatch = DispatchState::Claimed;
        Ok(pending.command)
    }

    /// A receipt may legitimately arrive between claim() and this callback.
    pub fn sent(&mut self, stamp: Stamp) -> bool {
        let Some(pending) = self.pending.as_mut().filter(|p| p.stamp == stamp) else {
            return false;
        };
        if pending.dispatch != DispatchState::Claimed {
            return false;
        }
        pending.dispatch = DispatchState::Sent;
        true
    }

    /// Use only when no dispatch claim has escaped. It is never a refund.
    pub fn cancel_unsent(&mut self, stamp: Stamp) -> bool {
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.stamp == stamp && p.dispatch == DispatchState::Prepared)
        {
            self.pending = None;
            true
        } else {
            false
        }
    }

    /// A worker that proves the command never reached transport may release a
    /// claimed request. Never use this for a timeout, send error, or missing ACK.
    pub fn rejected_unsent(&mut self, stamp: Stamp) -> bool {
        if self.pending.as_ref().is_some_and(|p| {
            p.stamp == stamp && p.dispatch != DispatchState::Sent
                && p.completion.is_none() && p.last_value_receipt.is_none()
        }) {
            self.pending = None;
            true
        } else {
            false
        }
    }

    pub fn send_failed(&mut self, stamp: Stamp) -> Observation {
        self.interrupt_stamp(stamp, BlockReason::TransportUncertain)
    }
    pub fn timeout(&mut self, stamp: Stamp) -> Observation {
        self.interrupt_stamp(stamp, BlockReason::TimedOut)
    }
    fn interrupt_stamp(&mut self, stamp: Stamp, reason: BlockReason) -> Observation {
        let Some(pending) = self.pending.as_ref().filter(|p| p.stamp == stamp) else {
            return Observation::Ignored;
        };
        if pending.dispatch == DispatchState::Prepared {
            self.pending = None;
            Observation::Updated
        } else {
            self.freeze(reason)
        }
    }

    /// End has no mapped acknowledgement and cannot undo an escaped purchase.
    /// Returns at most one End command; dispatch it once through the owner.
    pub fn close(&mut self, epoch: u64) -> Option<TrainingCommand> {
        if epoch != self.epoch {
            return None;
        }
        let trainer_id = self
            .trainer
            .as_ref()
            .map(|t| t.id)
            .or_else(|| self.pending.as_ref().map(|p| p.trainer.id));
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.dispatch != DispatchState::Prepared)
        {
            self.freeze(BlockReason::ClosedDuringRequest);
        } else {
            self.pending = None;
        }
        self.trainer = None;
        self.reported_maxima = None;
        trainer_id.map(|trainer_id| TrainingCommand::End {
            trainer_id,
            player_id: self.player_id,
        })
    }

    pub fn interrupt(&mut self, epoch: u64, reason: BlockReason) -> Observation {
        if epoch != self.epoch || self.profile.is_none() {
            return Observation::Ignored;
        }
        self.freeze(reason)
    }

    fn freeze(&mut self, reason: BlockReason) -> Observation {
        if let Some(pending) = self.pending.take()
            && let Some(completion) = pending.completion
        {
            self.last_report = Some(PurchaseReport {
                stamp: pending.stamp,
                completion,
                received_value: pending.received_value,
                estimate_after: None,
            });
        }
        self.estimate = None;
        self.blocked = Some(reason);
        Observation::Blocked(reason)
    }

    pub fn observe(&mut self, epoch: u64, event: TrainingEvent) -> Observation {
        if epoch != self.epoch || self.profile.is_none() || self.blocked.is_some() {
            return Observation::Ignored;
        }
        match event {
            TrainingEvent::Opened {
                trainer_id,
                player_id,
                skills,
            } => {
                // An exact duplicate open reply cannot authorize a second send.
                if self.trainer.as_ref().is_some_and(|t| t.id == trainer_id)
                    && player_id == self.player_id
                {
                    return Observation::Ignored;
                }
                let Some(pending) = self.pending.as_ref() else {
                    return Observation::Ignored;
                };
                if pending.dispatch == DispatchState::Prepared
                    || pending.command
                        != (TrainingCommand::Open {
                            trainer_id,
                            player_id,
                        })
                {
                    return self.freeze(BlockReason::UnexpectedReply);
                }
                let pending = self.pending.take().unwrap();
                self.trainer = Some(pending.trainer);
                self.reported_maxima = Some(std::array::from_fn(|i| skills[i]));
                Observation::Opened
            }
            TrainingEvent::Completed(completion) => {
                let Some(pending) = self.pending.as_ref() else {
                    if self
                        .last_report
                        .as_ref()
                        .is_some_and(|r| r.completion == completion)
                    {
                        return Observation::Ignored;
                    }
                    return self.freeze(BlockReason::UnexpectedReply);
                };
                let TrainingCommand::Train { selection, .. } = pending.command else {
                    return self.freeze(BlockReason::UnexpectedReply);
                };
                if pending.dispatch == DispatchState::Prepared
                    || completion.wire_skill_id != selection.wire_id()
                    || completion.trainer_name != pending.trainer.clean_name
                {
                    return self.freeze(BlockReason::UnexpectedReply);
                }
                if let Some(previous) = &pending.completion {
                    if previous == &completion {
                        return Observation::Ignored;
                    }
                    return self.freeze(BlockReason::UnexpectedReply);
                }
                self.pending.as_mut().unwrap().completion = Some(completion);
                self.finish_if_ready()
            }
        }
    }

    /// Call before a general progression reducer discards identical values.
    /// Values remain authoritative even when their transaction is ambiguous.
    pub fn observe_skill(&mut self, epoch: u64, wire_id: u32, value: u32) -> Observation {
        if epoch != self.epoch || self.profile.is_none() {
            return Observation::Ignored;
        }
        let slot = match wire_id {
            0..=77 => &mut self.skills[wire_id as usize],
            100..=127 => &mut self.languages[(wire_id - 100) as usize],
            _ => return Observation::Ignored,
        };
        let previous = slot.replace(value);
        if self.blocked.is_some() {
            return Observation::Updated;
        }
        let Some(pending) = self.pending.as_ref() else {
            return Observation::Updated;
        };
        let TrainingCommand::Train { selection, .. } = pending.command else {
            return Observation::Updated;
        };
        if wire_id != selection.wire_id() {
            return if previous != Some(value) {
                self.freeze(BlockReason::UnexpectedSkillValue)
            } else {
                Observation::Updated
            };
        }
        if pending.dispatch == DispatchState::Prepared {
            return if previous != Some(value) {
                self.freeze(BlockReason::UnexpectedSkillValue)
            } else {
                Observation::Updated
            };
        }
        let old = pending.old_value.unwrap();
        let already_received = pending.received_value;
        self.pending.as_mut().unwrap().last_value_receipt = Some(value);
        // A same-value rejection is a receipt, but supplies no progress and no
        // practice/currency deduction. Timeout remains uncertain without a reply.
        if value == old && already_received.is_none() {
            return Observation::Updated;
        }
        let valid = if selection.bank() == Bank::Skill && old == 0 {
            value > 0
        } else {
            old.checked_add(1) == Some(value)
        };
        if !valid || already_received.is_some_and(|seen| seen != value) {
            return self.freeze(BlockReason::UnexpectedSkillValue);
        }
        self.pending.as_mut().unwrap().received_value = Some(value);
        self.finish_if_ready()
    }

    fn finish_if_ready(&mut self) -> Observation {
        let pending = self.pending.as_ref().unwrap();
        let Some(completion) = pending.completion.as_ref() else {
            return Observation::Updated;
        };
        let Some(estimate) = self.estimate else {
            return self.freeze(BlockReason::NoKnownPracticeBalance);
        };
        if u64::from(completion.assessed_cost_copper) > estimate.carried_copper {
            return self.freeze(BlockReason::AssessedCostExceedsFunds);
        }
        if estimate.training_points == 0 {
            return self.freeze(BlockReason::NoKnownPracticeBalance);
        }
        if pending.received_value.is_none() {
            return Observation::Updated;
        }
        let after = SessionEstimate {
            training_points: estimate.training_points - 1,
            carried_copper: estimate.carried_copper - u64::from(completion.assessed_cost_copper),
            matched_purchases: estimate.matched_purchases.saturating_add(1),
        };
        let pending = self.pending.take().unwrap();
        self.estimate = Some(after);
        self.last_report = Some(PurchaseReport {
            stamp: pending.stamp,
            completion: pending.completion.unwrap(),
            received_value: pending.received_value,
            estimate_after: Some(after),
        });
        Observation::Completed
    }

    pub fn observe_currency(&mut self, epoch: u64, currency: Currency) -> Observation {
        if epoch != self.epoch || self.profile.is_none() {
            return Observation::Ignored;
        }
        let value = carried_copper(currency);
        self.observed_copper = Some(value);
        if self.blocked.is_some() {
            return Observation::Updated;
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| matches!(p.command, TrainingCommand::Train { .. }))
        {
            return self.freeze(BlockReason::ConcurrentCurrencyUpdate);
        }
        if let Some(estimate) = &mut self.estimate {
            estimate.carried_copper = value;
        }
        Observation::Updated
    }

    /// Other production money actions can derive a carried balance too. Rebase
    /// the session estimate without labelling that derivation as a new packet.
    pub fn rebase_estimated_copper(&mut self, epoch: u64, copper: u64) -> Observation {
        if epoch != self.epoch || self.profile.is_none() || self.blocked.is_some() {
            return Observation::Ignored;
        }
        if self.pending.is_some() {
            return self.freeze(BlockReason::ConcurrentCurrencyUpdate);
        }
        if let Some(estimate) = &mut self.estimate {
            estimate.carried_copper = copper;
        }
        Observation::Updated
    }

    /// Even an identical level receipt cannot establish new practice points.
    pub fn observe_level(&mut self, epoch: u64) -> Observation {
        self.interrupt(epoch, BlockReason::LevelChanged)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPOCH: u64 = 7;
    fn trainer() -> TrainerIdentity {
        TrainerIdentity {
            id: 42,
            revision: 9,
            clean_name: "Teacher".into(),
        }
    }
    fn state(points: u32, copper: u32) -> TrainingState {
        let mut state = TrainingState::new(EPOCH, 8);
        let mut skills = [20; SKILL_COUNT];
        skills[1] = 0;
        assert!(state.apply_profile(
            EPOCH,
            ProfileSnapshot {
                training_points: points,
                currency: Currency {
                    copper,
                    ..Currency::default()
                },
                skills: &skills,
                languages: &[99; LANGUAGE_COUNT],
            }
        ));
        state
    }
    fn open(state: &mut TrainingState) {
        let stamp = state.request_open(EPOCH, trainer()).unwrap();
        assert_eq!(
            state.claim(stamp, &trainer()).unwrap(),
            TrainingCommand::Open {
                trainer_id: 42,
                player_id: 8
            }
        );
        assert!(state.sent(stamp));
        let mut skills = Box::new([200; 100]);
        skills[78..].fill(u32::MAX);
        assert_eq!(
            state.observe(
                EPOCH,
                TrainingEvent::Opened {
                    trainer_id: 42,
                    player_id: 8,
                    skills
                }
            ),
            Observation::Opened
        );
        assert_eq!(state.reported_maxima().unwrap().len(), SKILL_COUNT);
        assert_eq!(state.reported_maxima().unwrap()[77], 200);
    }
    fn train(state: &mut TrainingState, bank: u16, id: u32) -> Stamp {
        let selection = Selection::new(bank, id).unwrap();
        let stamp = state.request_train(EPOCH, selection).unwrap();
        assert_eq!(
            state.claim(stamp, &trainer()).unwrap(),
            TrainingCommand::Train {
                trainer_id: 42,
                selection
            }
        );
        stamp
    }
    fn receipt(id: u32, cost: u32) -> TrainingEvent {
        TrainingEvent::Completed(Completion {
            wire_skill_id: id,
            assessed_cost_copper: cost,
            new_skill: 0,
            trainer_name: "Teacher".into(),
        })
    }

    #[test]
    fn ordinary_receipts_update_only_estimates_and_allow_repeated_purchases() {
        let mut state = state(2, 30);
        open(&mut state);
        let stamp = train(&mut state, 0, 0);
        assert_eq!(state.value(0), Some(20));
        assert_eq!(state.estimate().unwrap().training_points, 2);
        assert_eq!(
            state.claim(stamp, &trainer()),
            Err(TrainingError::AlreadyClaimed)
        );
        assert_eq!(
            state.request_train(EPOCH, Selection::new(0, 1).unwrap()),
            Err(TrainingError::Busy)
        );
        assert_eq!(state.observe_skill(EPOCH, 0, 21), Observation::Updated);
        assert_eq!(state.observe(EPOCH, receipt(0, 10)), Observation::Completed);
        // An early receipt can finish before the asynchronous sent callback.
        assert!(!state.sent(stamp));
        assert_eq!(state.send_failed(stamp), Observation::Ignored);
        assert_eq!(
            state.estimate(),
            Some(SessionEstimate {
                training_points: 1,
                carried_copper: 20,
                matched_purchases: 1
            })
        );
        assert_eq!(
            state.profile_balances(),
            Some(ProfileBalances {
                training_points: 2,
                carried_copper: 30
            })
        );
        assert_eq!(state.observed_copper(), Some(30));
        assert_eq!(state.observe(EPOCH, receipt(0, 10)), Observation::Ignored);
        let second = train(&mut state, 0, 0);
        assert!(state.sent(second));
        assert!(!state.sent(second));
        assert_eq!(state.timeout(stamp), Observation::Ignored);
        // A completion cannot locally invent a skill value, even if it arrives first.
        assert_eq!(state.observe(EPOCH, receipt(0, 13)), Observation::Updated);
        assert_eq!(state.observe(EPOCH, receipt(0, 13)), Observation::Ignored);
        assert_eq!(state.value(0), Some(21));
        assert_eq!(state.observe_skill(EPOCH, 0, 22), Observation::Completed);
        assert_eq!(
            state.estimate(),
            Some(SessionEstimate {
                training_points: 0,
                carried_copper: 7,
                matched_purchases: 2
            })
        );
        assert_eq!(state.last_report().unwrap().received_value, Some(22));
        assert_eq!(
            state.request_train(EPOCH, Selection::new(0, 0).unwrap()),
            Err(TrainingError::NoPractices)
        );
    }

    #[test]
    fn new_skill_uses_received_value_and_language_stops_at_100() {
        let mut state = state(3, 50_000);
        open(&mut state);
        train(&mut state, 0, 1);
        assert_eq!(state.observe(EPOCH, receipt(1, 0)), Observation::Updated);
        assert_eq!(state.value(1), Some(0));
        assert_eq!(state.observe_skill(EPOCH, 1, 7), Observation::Completed);
        assert_eq!(state.value(1), Some(7));
        train(&mut state, 1, 25);
        assert_eq!(state.observe_skill(EPOCH, 125, 100), Observation::Updated);
        assert_eq!(
            state.observe(EPOCH, receipt(125, 7049)),
            Observation::Completed
        );
        assert_eq!(
            state.request_train(EPOCH, Selection::new(1, 25).unwrap()),
            Err(TrainingError::LanguageAtCap)
        );
        assert_eq!(
            state.observe_skill(EPOCH, 124, u32::MAX),
            Observation::Updated
        );
        assert_eq!(
            state.request_train(EPOCH, Selection::new(1, 24).unwrap()),
            Err(TrainingError::LanguageAtCap)
        );
        for (bank, id) in [(1, 26), (1, 27), (2, 0), (0, 78), (u16::MAX, u32::MAX)] {
            assert!(Selection::new(bank, id).is_err());
        }
        // Receive-only languages stay observable without acquiring train authority.
        assert_eq!(state.observe_skill(EPOCH, 127, 200), Observation::Updated);
        assert_eq!(state.value(127), Some(200));
        assert_eq!(
            state.observe_skill(EPOCH, u32::MAX, 123),
            Observation::Ignored
        );
    }

    #[test]
    fn same_value_receipt_is_not_success_or_a_refund() {
        let mut state = state(1, 20);
        open(&mut state);
        let stamp = train(&mut state, 0, 0);
        assert_eq!(state.observe_skill(EPOCH, 0, 20), Observation::Updated);
        assert_eq!(state.pending_value_receipt(), Some(20));
        assert_eq!(state.estimate().unwrap().training_points, 1);
        assert_eq!(state.observe(EPOCH, receipt(0, 10)), Observation::Updated);
        assert!(state.pending().is_some());
        assert_eq!(
            state.timeout(stamp),
            Observation::Blocked(BlockReason::TimedOut)
        );
        assert_eq!(state.estimate(), None);
        assert_eq!(
            state.last_report().unwrap().completion.assessed_cost_copper,
            10
        );
        assert_eq!(state.last_report().unwrap().received_value, None);
        assert_eq!(state.observe(EPOCH, receipt(0, 10)), Observation::Ignored);
        assert_eq!(state.profile_balances().unwrap().carried_copper, 20);
        assert_eq!(
            state.request_open(EPOCH, trainer()),
            Err(TrainingError::Blocked(BlockReason::TimedOut))
        );
    }

    #[test]
    fn cost_above_known_funds_is_assessed_without_claiming_payment() {
        let mut state = state(1, 10);
        open(&mut state);
        train(&mut state, 0, 0);
        state.observe_skill(EPOCH, 0, 21);
        assert_eq!(
            state.observe(EPOCH, receipt(0, u32::MAX)),
            Observation::Blocked(BlockReason::AssessedCostExceedsFunds)
        );
        assert_eq!(state.value(0), Some(21));
        assert_eq!(state.estimate(), None);
        assert_eq!(state.observed_copper(), Some(10));
        let report = state.last_report().unwrap();
        assert_eq!(report.completion.assessed_cost_copper, u32::MAX);
        assert_eq!(report.received_value, Some(21));
        assert_eq!(report.estimate_after, None);
        // Later independent currency evidence cannot invent the missing practices.
        state.observe_currency(
            EPOCH,
            Currency {
                copper: 5,
                ..Currency::default()
            },
        );
        assert_eq!(state.estimate(), None);
        state.close(EPOCH);
        assert_eq!(state.blocked(), Some(BlockReason::AssessedCostExceedsFunds));
    }

    #[test]
    fn completion_requires_bank_id_trainer_and_current_epoch() {
        for bad in [
            receipt(100, 10),
            receipt(u32::MAX, 10),
            TrainingEvent::Completed(Completion {
                wire_skill_id: 0,
                assessed_cost_copper: 10,
                new_skill: 1,
                trainer_name: "Other Teacher".into(),
            }),
        ] {
            let mut state = state(1, 20);
            open(&mut state);
            train(&mut state, 0, 0);
            assert_eq!(state.observe(EPOCH - 1, bad.clone()), Observation::Ignored);
            assert_eq!(state.estimate().unwrap().training_points, 1);
            assert_eq!(
                state.observe(EPOCH, bad),
                Observation::Blocked(BlockReason::UnexpectedReply)
            );
            assert_eq!(state.estimate(), None);
        }
        let mut state = state(1, 20);
        let stamp = state.request_open(EPOCH, trainer()).unwrap();
        state.claim(stamp, &trainer()).unwrap();
        assert_eq!(
            state.observe(
                EPOCH,
                TrainingEvent::Opened {
                    trainer_id: 43,
                    player_id: 8,
                    skills: Box::new([200; 100])
                }
            ),
            Observation::Blocked(BlockReason::UnexpectedReply)
        );
    }

    #[test]
    fn unsolicited_preclaim_or_contradictory_receipts_cannot_complete() {
        let mut state = state(2, 30);
        open(&mut state);
        state
            .request_train(EPOCH, Selection::new(0, 0).unwrap())
            .unwrap();
        assert_eq!(
            state.observe(EPOCH, receipt(0, 10)),
            Observation::Blocked(BlockReason::UnexpectedReply)
        );
        let mut state = self::state(2, 30);
        open(&mut state);
        train(&mut state, 0, 0);
        state.observe(EPOCH, receipt(0, 10));
        assert_eq!(
            state.observe(EPOCH, receipt(0, 11)),
            Observation::Blocked(BlockReason::UnexpectedReply)
        );
        for (id, value) in [(0, 19), (0, 22), (1, 1)] {
            let mut state = self::state(2, 30);
            open(&mut state);
            train(&mut state, 0, 0);
            assert_eq!(
                state.observe_skill(EPOCH, id, value),
                Observation::Blocked(BlockReason::UnexpectedSkillValue)
            );
            assert_eq!(state.value(id), Some(value));
            assert_eq!(state.estimate(), None);
        }
    }

    #[test]
    fn dispatch_cancellation_identity_and_late_callbacks_are_bounded() {
        let mut state = state(2, 30);
        open(&mut state);
        let first = state
            .request_train(EPOCH, Selection::new(0, 0).unwrap())
            .unwrap();
        assert!(state.cancel_unsent(first));
        assert!(!state.cancel_unsent(first));
        let second = state
            .request_train(EPOCH, Selection::new(0, 0).unwrap())
            .unwrap();
        assert_ne!(first, second);
        assert_eq!(
            state.claim(first, &trainer()),
            Err(TrainingError::StaleStamp)
        );
        assert_eq!(state.timeout(first), Observation::Ignored);
        assert_eq!(state.send_failed(first), Observation::Ignored);
        assert_eq!(state.estimate().unwrap().training_points, 2);
        let mut reused = trainer();
        reused.revision += 1;
        assert_eq!(
            state.claim(second, &reused),
            Err(TrainingError::Blocked(BlockReason::TrainerChanged))
        );
        assert_eq!(
            state.claim(second, &trainer()),
            Err(TrainingError::StaleStamp)
        );
        let mut state = self::state(2, 30);
        open(&mut state);
        let stamp = train(&mut state, 0, 0);
        assert!(!state.cancel_unsent(stamp));
        assert_eq!(
            state.send_failed(stamp),
            Observation::Blocked(BlockReason::TransportUncertain)
        );
        assert_eq!(state.timeout(stamp), Observation::Ignored);
        assert_eq!(state.estimate(), None);
    }

    #[test]
    fn close_does_not_roll_back_and_reopen_does_not_reset_spending() {
        let mut state = state(2, 30);
        open(&mut state);
        train(&mut state, 0, 0);
        state.observe_skill(EPOCH, 0, 21);
        state.observe(EPOCH, receipt(0, 10));
        assert_eq!(
            state.close(EPOCH),
            Some(TrainingCommand::End {
                trainer_id: 42,
                player_id: 8
            })
        );
        assert_eq!(state.close(EPOCH), None);
        open(&mut state);
        assert_eq!(state.estimate().unwrap().training_points, 1);
        let stamp = train(&mut state, 0, 0);
        assert!(state.close(EPOCH).is_some());
        assert_eq!(state.blocked(), Some(BlockReason::ClosedDuringRequest));
        assert_eq!(state.estimate(), None);
        assert!(!state.sent(stamp));
        assert_eq!(state.observe(EPOCH, receipt(0, 13)), Observation::Ignored);
    }

    #[test]
    fn level_recovery_and_epoch_replacement_invalidate_pending_claims() {
        for reason in [BlockReason::LevelChanged, BlockReason::Recovery] {
            let mut state = state(1, 20);
            open(&mut state);
            let stamp = train(&mut state, 0, 0);
            let result = if reason == BlockReason::LevelChanged {
                state.observe_level(EPOCH)
            } else {
                state.interrupt(EPOCH, reason)
            };
            assert_eq!(result, Observation::Blocked(reason));
            assert_eq!(state.estimate(), None);
            assert!(!state.sent(stamp));
            state.begin_epoch(EPOCH + 1, 8);
            assert_eq!(state.observe_skill(EPOCH, 0, 21), Observation::Ignored);
            assert_eq!(state.observe(EPOCH, receipt(0, 10)), Observation::Ignored);
            assert_eq!(state.value(0), None);
            assert_eq!(
                state.claim(stamp, &trainer()),
                Err(TrainingError::StaleStamp)
            );
            assert_eq!(
                state.request_open(EPOCH + 1, trainer()),
                Err(TrainingError::MissingProfile)
            );
            assert!(!state.apply_profile(
                EPOCH,
                ProfileSnapshot {
                    training_points: 1,
                    currency: Currency::default(),
                    skills: &[20],
                    languages: &[]
                }
            ));
            assert!(state.apply_profile(
                EPOCH + 1,
                ProfileSnapshot {
                    training_points: 1,
                    currency: Currency::default(),
                    skills: &[20],
                    languages: &[]
                }
            ));
            assert!(state.request_open(EPOCH + 1, trainer()).is_ok());
        }
    }

    #[test]
    fn currency_receipts_rebase_money_but_do_not_double_account_during_purchase() {
        let mut state = state(1, 20);
        open(&mut state);
        let currency = Currency {
            platinum: u32::MAX,
            gold: u32::MAX,
            silver: u32::MAX,
            copper: u32::MAX,
        };
        state.observe_currency(EPOCH, currency);
        assert_eq!(
            state.estimate().unwrap().carried_copper,
            u64::from(u32::MAX) * 1111
        );
        assert_eq!(state.profile_balances().unwrap().carried_copper, 20);
        train(&mut state, 0, 0);
        assert_eq!(
            state.observe_currency(
                EPOCH,
                Currency {
                    copper: 10,
                    ..Currency::default()
                }
            ),
            Observation::Blocked(BlockReason::ConcurrentCurrencyUpdate)
        );
        assert_eq!(state.observed_copper(), Some(10));
        assert_eq!(state.estimate(), None);
    }

    #[test]
    fn opening_specialization_updates_are_authoritative_and_missing_values_stay_missing() {
        let mut state = state(0, 20);
        let stamp = state.request_open(EPOCH, trainer()).unwrap();
        state.claim(stamp, &trainer()).unwrap();
        for id in 43..=47 {
            assert_eq!(state.observe_skill(EPOCH, id, 1), Observation::Updated);
            assert_eq!(state.value(id), Some(1));
        }
        state.observe(
            EPOCH,
            TrainingEvent::Opened {
                trainer_id: 42,
                player_id: 8,
                skills: Box::new([200; 100]),
            },
        );
        assert_eq!(
            state.request_train(EPOCH, Selection::new(0, 43).unwrap()),
            Err(TrainingError::NoPractices)
        );
        assert_eq!(state.estimate().unwrap().training_points, 0);
        state.apply_profile(
            EPOCH,
            ProfileSnapshot {
                training_points: 2,
                currency: Currency::default(),
                skills: &[20],
                languages: &[],
            },
        );
        open(&mut state);
        assert_eq!(
            state.request_train(EPOCH, Selection::new(0, 1).unwrap()),
            Err(TrainingError::MissingValue)
        );
        state.observe_skill(EPOCH, 0, 200);
        assert_eq!(
            state.request_train(EPOCH, Selection::new(0, 0).unwrap()),
            Err(TrainingError::AtReportedMaximum)
        );
        let mut wide = trainer();
        wide.id = 70_001;
        state.close(EPOCH);
        let stamp = state.request_open(EPOCH, wide.clone()).unwrap();
        state.claim(stamp, &wide).unwrap();
        state.observe(
            EPOCH,
            TrainingEvent::Opened {
                trainer_id: 70_001,
                player_id: 8,
                skills: Box::new([200; 100]),
            },
        );
        assert_eq!(
            state.request_train(EPOCH, Selection::new(0, 0).unwrap()),
            Err(TrainingError::InvalidTrainer)
        );
    }

    #[test]
    fn preview_checks_stock_price_and_funds_without_creating_pending() {
        let skill = Selection::new(0, 0).unwrap();
        for (value, expected) in [
            (0, 0),
            (10, 0),
            (11, 0),
            (14, 0),
            (15, 1),
            (20, 10),
            (55, 911),
            (56, 973),
            (100, 7290),
        ] {
            assert_eq!(estimated_cost_copper(skill, value), Some(expected));
        }
        assert_eq!(estimated_cost_copper(skill, u32::MAX), None);
        assert_eq!(estimated_cost_copper(skill, 1300), Some(21_466_890));
        assert_eq!(estimated_cost_copper(skill, 1301), None);
        let mut state = state(2, 9);
        open(&mut state);
        assert_eq!(
            state.preview(EPOCH, skill),
            Err(TrainingError::InsufficientEstimatedFunds)
        );
        assert_eq!(
            state.request_train(EPOCH, skill),
            Err(TrainingError::InsufficientEstimatedFunds)
        );
        assert!(state.pending().is_none());
        state.observe_currency(
            EPOCH,
            Currency {
                copper: 10,
                ..Currency::default()
            },
        );
        assert_eq!(
            state.preview(EPOCH, skill),
            Ok(TrainingPreview {
                base_value: 20,
                estimated_cost_copper: 10
            })
        );
        assert!(state.pending().is_none());
        let stamp = state.request_train(EPOCH, skill).unwrap();
        assert_eq!(state.preview(EPOCH, skill), Err(TrainingError::Busy));
        assert!(state.cancel_unsent(stamp));
        state.close(EPOCH);
        let stamp = state.request_open(EPOCH, trainer()).unwrap();
        state.claim(stamp, &trainer()).unwrap();
        state.observe(
            EPOCH,
            TrainingEvent::Opened {
                trainer_id: 42,
                player_id: 8,
                skills: Box::new([u32::MAX; 100]),
            },
        );
        state.observe_skill(EPOCH, 0, u32::MAX - 1);
        assert_eq!(
            state.preview(EPOCH, skill),
            Err(TrainingError::CostEstimateOverflow)
        );
    }

    #[test]
    fn late_player_binding_preserves_profile_but_rejects_replacement() {
        let mut state = TrainingState::new(EPOCH, 0);
        state.apply_profile(
            EPOCH,
            ProfileSnapshot {
                training_points: 2,
                currency: Currency::default(),
                skills: &[20],
                languages: &[],
            },
        );
        assert_eq!(
            state.request_open(EPOCH, trainer()),
            Err(TrainingError::InvalidTrainer)
        );
        assert!(!state.bind_player(EPOCH - 1, 8));
        assert!(!state.bind_player(EPOCH, 0));
        assert!(state.bind_player(EPOCH, 8));
        assert!(state.bind_player(EPOCH, 8));
        assert!(!state.bind_player(EPOCH, 9));
        let stamp = state.request_open(EPOCH, trainer()).unwrap();
        assert_eq!(state.active_trainer(), Some(&trainer()));
        assert_eq!(state.trainer(), None);
        assert!(state.claim(stamp, &trainer()).is_ok());
        assert_eq!(state.profile_balances().unwrap().training_points, 2);
    }
}
