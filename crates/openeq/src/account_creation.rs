//! Immutable, single-flight creation transactions. No socket or database I/O.
//! Name approval already reserves durable state; cancellation after dispatch
//! detaches the UI while the worker finishes this exact bounded transaction.
pub mod editor;
use openeq_assets::character::customization::CustomizationCatalog;
use openeq_net::{
    AppPacket,
    creation::{
        self, Appearance, Approval, Capabilities, Catalog, Choice, Creation, CreationError, Stats,
    },
    world::{Character, CharacterAppearance},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const REPLY_TIMEOUT: Duration = Duration::from_secs(20);
static NEXT_TRANSACTION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewFamily {
    Classic,
    Luclin,
    Drakkin,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Feature {
    Face,
    Hair,
    Beard,
    HairColor,
    BeardColor,
    Eye1,
    Eye2,
    Heritage,
    Tattoo,
    Details,
}

/// Creation controls are an intersection of source-backed race limits and
/// implemented preview features. This is not renderer fallback normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearancePolicy {
    race: u32,
    class: u32,
    gender: u8,
    family: PreviewFamily,
    choices: BTreeMap<Feature, Vec<u32>>,
    /// Luclin/classic palettes are not reproduced. Never show a color picker
    /// for these fixed source defaults; UI must explain incomplete preview.
    pub colors_not_previewed: bool,
}
impl AppearancePolicy {
    /// Caller must identify the actual loaded model family, not merely the
    /// requested preference (the renderer may fall back when assets are absent).
    pub fn for_preview(
        race: u32,
        class: u32,
        gender: u8,
        family: PreviewFamily,
        metadata: &CustomizationCatalog,
        heritage: u32,
    ) -> Result<Self, CreationError> {
        if creation::player_race_bit(race).is_none()
            || creation::class_bit(class).is_none()
            || gender > 1
        {
            return Err(CreationError::Unavailable("player appearance"));
        }
        let mut policy = Self {
            race,
            class,
            gender,
            family,
            choices: BTreeMap::new(),
            colors_not_previewed: family != PreviewFamily::Drakkin,
        };
        for feature in [
            Feature::Face,
            Feature::Hair,
            Feature::Beard,
            Feature::HairColor,
            Feature::BeardColor,
            Feature::Eye1,
            Feature::Eye2,
            Feature::Heritage,
            Feature::Tattoo,
            Feature::Details,
        ] {
            policy.choices.insert(feature, vec![0]);
        }
        if family == PreviewFamily::Drakkin {
            if race != 522 {
                return Err(CreationError::Unavailable(
                    "Drakkin appearance requires Drakkin model",
                ));
            }
            let entry = metadata
                .get(522, heritage, gender)
                .filter(|entry| heritage <= 7 && entry.classes.contains(&class))
                .ok_or(CreationError::Unavailable(
                    "authored Drakkin heritage/class",
                ))?;
            policy.choices.insert(Feature::Heritage, vec![heritage]);
            for (feature, count, maximum) in [
                (Feature::Face, entry.features.faces, 7),
                (
                    Feature::Hair,
                    entry.features.hair_styles,
                    if gender == 0 { 9 } else { 8 },
                ),
                (
                    Feature::Beard,
                    entry.features.beards,
                    if gender == 0 { 12 } else { 4 },
                ),
                (Feature::HairColor, entry.colors.len() as u32, 4),
                (Feature::BeardColor, entry.colors.len() as u32, 4),
                (Feature::Eye1, entry.features.eyes, 12),
                (Feature::Eye2, entry.features.eyes, 12),
                (Feature::Tattoo, entry.features.tattoos, 8),
                (Feature::Details, entry.features.facial_attachments, 8),
            ] {
                if count == 0 {
                    return Err(CreationError::Unavailable(
                        "missing authored Drakkin choices",
                    ));
                }
                policy
                    .choices
                    .insert(feature, (0..count.min(maximum)).collect());
            }
            return Ok(policy);
        }
        if race == 522 {
            return Err(CreationError::Unavailable(
                "Drakkin requires authored modern appearance",
            ));
        }
        // Classic WLD has face variants. Other cosmetic fields are locked,
        // never exposed as sliders whose preview would silently discard them.
        let faces = if race == 330 { 10 } else { 8 };
        policy.choices.insert(Feature::Face, (0..faces).collect());
        if race == 6 {
            // EQEmu IsValidHairColor/BeardColor: Dark Elf13..18.
            policy.choices.insert(Feature::HairColor, vec![13]);
            if gender == 0 {
                policy.choices.insert(Feature::BeardColor, vec![13]);
            }
        }
        if family == PreviewFamily::Luclin {
            // The current Luclin renderer handles at most faces0..7/eyes0..9.
            // Barbarian woad is intentionally not offered as a guessed slider.
            policy.choices.insert(Feature::Face, (0..8).collect());
            policy.choices.insert(Feature::Eye1, (0..10).collect());
            policy.choices.insert(Feature::Eye2, (0..10).collect());
            let hair = match (race, gender) {
                (3, 0) => 6,
                (3, 1) => 9,
                (1 | 2 | 4 | 5 | 6 | 7 | 8 | 11 | 12, _) | (9 | 10, 1) => 4,
                _ => 1,
            };
            let beard = match (race, gender) {
                (8, 1) => 2,
                (5..=7, 0) => 4,
                (1 | 2 | 3 | 8 | 11 | 12, 0) => 6,
                _ => 1,
            };
            policy.choices.insert(Feature::Hair, (0..hair).collect());
            policy.choices.insert(Feature::Beard, (0..beard).collect());
        }
        Ok(policy)
    }
    pub fn values(&self, feature: Feature) -> &[u32] {
        &self.choices[&feature]
    }
    pub fn editable(&self, feature: Feature) -> bool {
        self.values(feature).len() > 1
    }
    pub fn default_appearance(&self) -> Appearance {
        let first = |feature| self.values(feature)[0];
        Appearance {
            face: first(Feature::Face) as u8,
            hair_style: first(Feature::Hair) as u8,
            beard: first(Feature::Beard) as u8,
            hair_color: first(Feature::HairColor) as u8,
            beard_color: first(Feature::BeardColor) as u8,
            eye_color_1: first(Feature::Eye1) as u8,
            eye_color_2: first(Feature::Eye2) as u8,
            heritage: first(Feature::Heritage),
            tattoo: first(Feature::Tattoo),
            details: first(Feature::Details),
        }
    }
    pub fn validate(&self, draft: &Draft) -> Result<(), CreationError> {
        self.validate_appearance(
            draft.choice.race,
            draft.choice.class,
            draft.gender,
            draft.appearance,
        )
    }
    pub(crate) fn validate_appearance(
        &self,
        race: u32,
        class: u32,
        gender: u8,
        a: Appearance,
    ) -> Result<(), CreationError> {
        if (self.race, self.class, self.gender) != (race, class, gender) {
            return Err(CreationError::Unavailable("stale appearance capability"));
        }
        for (feature, value) in [
            (Feature::Face, u32::from(a.face)),
            (Feature::Hair, u32::from(a.hair_style)),
            (Feature::Beard, u32::from(a.beard)),
            (Feature::HairColor, u32::from(a.hair_color)),
            (Feature::BeardColor, u32::from(a.beard_color)),
            (Feature::Eye1, u32::from(a.eye_color_1)),
            (Feature::Eye2, u32::from(a.eye_color_2)),
            (Feature::Heritage, a.heritage),
            (Feature::Tattoo, a.tattoo),
            (Feature::Details, a.details),
        ] {
            if !self.values(feature).contains(&value) {
                return Err(CreationError::Unavailable("appearance choice"));
            }
        }
        Ok(())
    }
    pub fn family(&self) -> PreviewFamily {
        self.family
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub name: String,
    pub choice: Choice,
    pub gender: u8,
    pub appearance: Appearance,
    pub stats: Stats,
}
impl Draft {
    pub fn preview_character(&self) -> Result<Character, CreationError> {
        let zone = u16::try_from(self.choice.start_zone)
            .map_err(|_| CreationError::Unavailable("preview start zone"))?;
        if self.gender > 1
            || creation::class_bit(self.choice.class).is_none()
            || creation::player_race_bit(self.choice.race).is_none()
        {
            return Err(CreationError::Unavailable("preview race/class/gender"));
        }
        let a = self.appearance;
        Ok(Character {
            name: self.name.clone(),
            level: 1,
            class: self.choice.class as u8,
            race: self.choice.race,
            gender: self.gender,
            zone,
            instance_id: 0,
            enabled: false,
            appearance: CharacterAppearance {
                face: a.face,
                hair_color: a.hair_color,
                beard_color: a.beard_color,
                hair_style: a.hair_style,
                beard: a.beard,
                eye_color_1: a.eye_color_1,
                eye_color_2: a.eye_color_2,
                drakkin_heritage: a.heritage,
                drakkin_tattoo: a.tattoo,
                drakkin_details: a.details,
                ..Default::default()
            },
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    /// Authenticated account session; no credentials or keys belong here.
    pub session: u64,
    /// Exact world socket incarnation. Approval/create must share this value.
    pub connection: u64,
    pub catalog_revision: u64,
    pub roster_revision: u64,
    pub draft_revision: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewReceipt {
    pub(crate) context: Context,
    pub(crate) family: PreviewFamily,
    pub(crate) model_loaded: bool,
    pub(crate) race: u32,
    pub(crate) class: u32,
    pub(crate) gender: u8,
    pub(crate) appearance: Appearance,
}
impl PreviewReceipt {
    pub(crate) fn matches(&self, context: Context, draft: &Draft, family: PreviewFamily) -> bool {
        self.model_loaded
            && self.context == context
            && self.family == family
            && (self.race, self.class, self.gender)
                == (draft.choice.race, draft.choice.class, draft.gender)
            && self.appearance == draft.appearance
    }
}
/// A complete local proposal. The retained account worker validates this
/// against its own socket snapshot before it claims either outgoing packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Submission {
    pub context: Context,
    pub draft: Draft,
    pub appearance: AppearancePolicy,
    pub preview: PreviewReceipt,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub context: Context,
    pub operation: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenDraft {
    draft: Draft,
    approval: AppPacket,
    create: AppPacket,
}
impl FrozenDraft {
    pub fn draft(&self) -> &Draft {
        &self.draft
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    Name,
    Creation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uncertainty {
    Transport,
    TimedOut,
    MalformedReply,
    UnknownApproval,
    UnexpectedApproval,
    MissingFromRoster,
    RosterIdentityMismatch,
    SessionChanged,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase {
    Prepared,
    AwaitingApproval,
    AwaitingRoster,
    Completed(Box<Character>),
    Rejected(Rejection),
    Uncertain(Uncertainty),
    Cancelled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionError {
    StaleContext,
    AlreadyDispatched,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Ignored,
    SendCreate(AppPacket),
    Updated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cancellation {
    CancelledBeforeDispatch,
    FinishDetached,
    AlreadyFinished,
}

/// Non-cloneable owner. Packet claims advance state before returning a packet,
/// so a failed/unknown send cannot accidentally be claimed again.
pub struct Transaction {
    token: Token,
    frozen: FrozenDraft,
    phase: Phase,
    deadline: Option<Instant>,
    detached: bool,
}
impl Transaction {
    pub fn prepare(
        context: Context,
        draft: Draft,
        catalog: &Catalog,
        capabilities: &Capabilities,
        roster: &[Character],
        appearance: &AppearancePolicy,
        preview: PreviewReceipt,
    ) -> Result<Self, CreationError> {
        creation::validate_name(&draft.name)?;
        if roster
            .iter()
            .any(|character| character.name.eq_ignore_ascii_case(&draft.name))
        {
            return Err(CreationError::Unavailable("name already on this roster"));
        }
        let (combination, allocation) = catalog.resolve(draft.choice)?;
        capabilities.permits(combination, roster.len())?;
        allocation.validate_stats(draft.stats)?;
        appearance.validate(&draft)?;
        if !preview.matches(context, &draft, appearance.family()) {
            return Err(CreationError::Unavailable("current character preview"));
        }
        let approval = creation::approve_name(&draft.name, draft.choice)?;
        let create = Creation {
            choice: draft.choice,
            gender: draft.gender,
            appearance: draft.appearance,
            stats: draft.stats,
        }
        .packet()?;
        Ok(Self {
            token: Token {
                context,
                operation: NEXT_TRANSACTION.fetch_add(1, Ordering::Relaxed),
            },
            frozen: FrozenDraft {
                draft,
                approval,
                create,
            },
            phase: Phase::Prepared,
            deadline: None,
            detached: false,
        })
    }
    pub fn token(&self) -> Token {
        self.token
    }
    pub fn frozen(&self) -> &FrozenDraft {
        &self.frozen
    }
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    pub fn detached(&self) -> bool {
        self.detached
    }
    pub fn pending(&self) -> bool {
        matches!(
            self.phase,
            Phase::Prepared | Phase::AwaitingApproval | Phase::AwaitingRoster
        )
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    pub fn take_approval(
        &mut self,
        token: Token,
        current: Context,
        now: Instant,
    ) -> Result<AppPacket, TransactionError> {
        if token != self.token || current != token.context {
            return Err(TransactionError::StaleContext);
        }
        if self.phase != Phase::Prepared {
            return Err(TransactionError::AlreadyDispatched);
        }
        self.phase = Phase::AwaitingApproval;
        self.deadline = Some(now + REPLY_TIMEOUT);
        Ok(self.frozen.approval.clone())
    }
    /// The worker must pass its retained socket's context, never retag a reply
    /// with a newer UI attempt. This method returns at most one create packet.
    pub fn approval(
        &mut self,
        token: Token,
        reply: Result<Approval, CreationError>,
        now: Instant,
    ) -> Effect {
        if token != self.token {
            return Effect::Ignored;
        }
        if self.expire(now) {
            return Effect::Updated;
        }
        match (&self.phase, reply) {
            (Phase::AwaitingApproval, Ok(Approval::Approved)) => {
                self.phase = Phase::AwaitingRoster;
                self.deadline = Some(now + REPLY_TIMEOUT);
                Effect::SendCreate(self.frozen.create.clone())
            }
            (Phase::AwaitingApproval, Ok(Approval::Rejected)) => {
                self.finish(Phase::Rejected(Rejection::Name));
                Effect::Updated
            }
            (Phase::AwaitingRoster, Ok(Approval::Rejected)) => {
                self.finish(Phase::Rejected(Rejection::Creation));
                Effect::Updated
            }
            (Phase::AwaitingApproval | Phase::AwaitingRoster, Err(_)) => {
                self.finish(Phase::Uncertain(Uncertainty::MalformedReply));
                Effect::Updated
            }
            (Phase::AwaitingApproval | Phase::AwaitingRoster, Ok(Approval::Unknown(_))) => {
                self.finish(Phase::Uncertain(Uncertainty::UnknownApproval));
                Effect::Updated
            }
            (Phase::AwaitingRoster, Ok(Approval::Approved)) => {
                self.finish(Phase::Uncertain(Uncertainty::UnexpectedApproval));
                Effect::Updated
            }
            _ => Effect::Ignored,
        }
    }
    pub fn roster(
        &mut self,
        token: Token,
        revision: u64,
        characters: &[Character],
        now: Instant,
    ) -> Effect {
        if token != self.token
            || self.phase != Phase::AwaitingRoster
            || revision <= token.context.roster_revision
        {
            return Effect::Ignored;
        }
        if self.expire(now) {
            return Effect::Updated;
        }
        let draft = &self.frozen.draft;
        let matches: Vec<_> = characters
            .iter()
            .filter(|character| character.name.eq_ignore_ascii_case(&draft.name))
            .collect();
        let phase = match matches.as_slice() {
            [character]
                if character.level > 0
                    && character.race == draft.choice.race
                    && u32::from(character.class) == draft.choice.class
                    && character.gender == draft.gender =>
            {
                Phase::Completed(Box::new((*character).clone()))
            }
            [] => Phase::Uncertain(Uncertainty::MissingFromRoster),
            _ => Phase::Uncertain(Uncertainty::RosterIdentityMismatch),
        };
        self.finish(phase);
        Effect::Updated
    }
    pub fn transport_failed(&mut self, token: Token) {
        if token != self.token {
            return;
        }
        match self.phase {
            Phase::Prepared => self.finish(Phase::Cancelled),
            Phase::AwaitingApproval | Phase::AwaitingRoster => {
                self.finish(Phase::Uncertain(Uncertainty::Transport))
            }
            _ => {}
        }
    }
    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.finish(Phase::Uncertain(Uncertainty::TimedOut));
            true
        } else {
            false
        }
    }
    pub fn cancel(&mut self) -> Cancellation {
        self.detached = true;
        match self.phase {
            Phase::Prepared => {
                self.finish(Phase::Cancelled);
                Cancellation::CancelledBeforeDispatch
            }
            Phase::AwaitingApproval | Phase::AwaitingRoster => Cancellation::FinishDetached,
            _ => Cancellation::AlreadyFinished,
        }
    }
    pub fn session_changed(&mut self) {
        self.detached = true;
        match self.phase {
            Phase::Prepared => self.finish(Phase::Cancelled),
            Phase::AwaitingApproval | Phase::AwaitingRoster => {
                self.finish(Phase::Uncertain(Uncertainty::SessionChanged))
            }
            _ => {}
        }
    }
    fn finish(&mut self, phase: Phase) {
        self.phase = phase;
        self.deadline = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeginError {
    Busy,
    InspectUncertainResult,
    Invalid(CreationError),
}
/// Exactly one owner belongs to each account worker. An uncertain reservation
/// cannot be followed by another attempt until a different world connection
/// supplies a fresh selection snapshot. Its unresolved name stays blocked for
/// the session even then; only another name may be explicitly submitted. This
/// component offers no retry, reservation deletion or cleanup mutation.
#[derive(Default)]
pub struct CreationState {
    active: Option<Transaction>,
    unresolved_names: BTreeSet<(u64, String)>,
}
impl CreationState {
    pub fn active(&self) -> Option<&Transaction> {
        self.active.as_ref()
    }
    pub fn active_mut(&mut self) -> Option<&mut Transaction> {
        self.active.as_mut()
    }
    #[allow(clippy::too_many_arguments)]
    pub fn begin(
        &mut self,
        context: Context,
        draft: Draft,
        catalog: &Catalog,
        capabilities: &Capabilities,
        roster: &[Character],
        appearance: &AppearancePolicy,
        preview: PreviewReceipt,
    ) -> Result<Token, BeginError> {
        if let Some(active) = &self.active {
            if matches!(active.phase(), Phase::Uncertain(_)) {
                self.unresolved_names.insert((
                    active.token.context.session,
                    active.frozen.draft.name.to_ascii_lowercase(),
                ));
            }
            if active.pending() {
                return Err(BeginError::Busy);
            }
            if matches!(active.phase(), Phase::Uncertain(_))
                && (context.session == active.token.context.session
                    && (context.connection == active.token.context.connection
                        || context.roster_revision <= active.token.context.roster_revision))
            {
                return Err(BeginError::InspectUncertainResult);
            }
        }
        if self
            .unresolved_names
            .contains(&(context.session, draft.name.to_ascii_lowercase()))
        {
            return Err(BeginError::InspectUncertainResult);
        }
        let transaction = Transaction::prepare(
            context,
            draft,
            catalog,
            capabilities,
            roster,
            appearance,
            preview,
        )
        .map_err(BeginError::Invalid)?;
        let token = transaction.token();
        self.active = Some(transaction);
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        Context {
            session: 1,
            connection: 2,
            catalog_revision: 3,
            roster_revision: 4,
            draft_revision: 5,
        }
    }
    fn catalog() -> Catalog {
        let mut data = vec![0];
        for word in [
            1u32, 17, 71, 72, 73, 74, 75, 76, 77, 1, 2, 3, 4, 5, 6, 7, 1, 0, 1, 2, 201, 17, 77,
        ] {
            data.extend_from_slice(&word.to_le_bytes());
        }
        Catalog::parse(&data).unwrap()
    }
    fn capabilities() -> Capabilities {
        Capabilities {
            expansion_mask: Some(0xffff),
            maximum_characters: Some(12),
            membership: Some(creation::Membership {
                tier: 2,
                race_mask: 0xffff,
                class_mask: 0xffff,
                settings: [-1; 25],
            }),
        }
    }
    fn policy() -> AppearancePolicy {
        AppearancePolicy::for_preview(
            1,
            2,
            0,
            PreviewFamily::Luclin,
            &CustomizationCatalog::default(),
            0,
        )
        .unwrap()
    }
    fn draft() -> Draft {
        Draft {
            name: "Asteria".into(),
            choice: catalog().combinations()[0].choice,
            gender: 0,
            appearance: policy().default_appearance(),
            stats: catalog().allocation(17).unwrap().default_stats().unwrap(),
        }
    }
    fn receipt(context: Context) -> PreviewReceipt {
        let draft = draft();
        PreviewReceipt {
            context,
            family: PreviewFamily::Luclin,
            model_loaded: true,
            race: draft.choice.race,
            class: draft.choice.class,
            gender: draft.gender,
            appearance: draft.appearance,
        }
    }
    fn prepared() -> Transaction {
        Transaction::prepare(
            context(),
            draft(),
            &catalog(),
            &capabilities(),
            &[],
            &policy(),
            receipt(context()),
        )
        .unwrap()
    }
    fn character() -> Character {
        let mut character = draft().preview_character().unwrap();
        character.enabled = true;
        character
    }
    #[test]
    fn validation_precedes_reservation_and_stale_preview_does_not_authorize_commit() {
        let mut changed = draft();
        changed.choice.start_zone = 78;
        assert!(
            Transaction::prepare(
                context(),
                changed,
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context())
            )
            .is_err()
        );
        let mut changed = draft();
        changed.stats.strength = u32::MAX;
        assert!(
            Transaction::prepare(
                context(),
                changed,
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context())
            )
            .is_err()
        );
        let mut changed = draft();
        changed.appearance.hair_color = 1;
        assert!(
            Transaction::prepare(
                context(),
                changed,
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context())
            )
            .is_err()
        );
        let mut stale = receipt(context());
        stale.context.connection += 1;
        assert!(
            Transaction::prepare(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                stale
            )
            .is_err()
        );
        let mut absent = receipt(context());
        absent.model_loaded = false;
        assert!(
            Transaction::prepare(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                absent
            )
            .is_err()
        );
        assert!(
            Transaction::prepare(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[character()],
                &policy(),
                receipt(context())
            )
            .is_err()
        );
    }
    #[test]
    fn receipt_cannot_be_reused_for_another_valid_appearance_in_the_same_context() {
        let mut changed = draft();
        changed.appearance.face = 1;
        assert!(policy().validate(&changed).is_ok());
        assert!(
            Transaction::prepare(
                context(),
                changed,
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context())
            )
            .is_err()
        );
        for field in 0..4 {
            let mut reused = receipt(context());
            match field {
                0 => reused.race += 1,
                1 => reused.class += 1,
                2 => reused.gender += 1,
                3 => reused.appearance.face = 1,
                _ => unreachable!(),
            }
            assert!(
                Transaction::prepare(
                    context(),
                    draft(),
                    &catalog(),
                    &capabilities(),
                    &[],
                    &policy(),
                    reused
                )
                .is_err(),
                "reused receipt field {field}"
            );
        }
    }

    #[test]
    fn frozen_pair_is_one_shot_and_draft_mutation_cannot_change_reserved_identity() {
        let mut original = draft();
        let mut transaction = Transaction::prepare(
            context(),
            original.clone(),
            &catalog(),
            &capabilities(),
            &[],
            &policy(),
            receipt(context()),
        )
        .unwrap();
        original.name = "Changed".into();
        original.choice.race = 2;
        original.stats.strength = 999;
        let token = transaction.token();
        let now = Instant::now();
        let approval = transaction.take_approval(token, context(), now).unwrap();
        assert_eq!(
            approval,
            creation::approve_name("Asteria", draft().choice).unwrap()
        );
        assert!(transaction.take_approval(token, context(), now).is_err());
        let Effect::SendCreate(packet) = transaction.approval(token, Ok(Approval::Approved), now)
        else {
            panic!()
        };
        let expected = Creation {
            choice: draft().choice,
            gender: 0,
            appearance: draft().appearance,
            stats: draft().stats,
        }
        .packet()
        .unwrap();
        assert_eq!(packet, expected);
        assert_eq!(transaction.frozen().draft().name, "Asteria");
        assert!(!matches!(
            transaction.approval(token, Ok(Approval::Approved), now),
            Effect::SendCreate(_)
        ));
        assert_eq!(
            transaction.phase(),
            &Phase::Uncertain(Uncertainty::UnexpectedApproval)
        );
    }
    #[test]
    fn cancellation_before_claim_sends_nothing_after_claim_finishes_detached() {
        let now = Instant::now();
        let mut transaction = prepared();
        let token = transaction.token();
        assert_eq!(transaction.cancel(), Cancellation::CancelledBeforeDispatch);
        assert!(transaction.take_approval(token, context(), now).is_err());
        assert_eq!(
            transaction.approval(token, Ok(Approval::Approved), now),
            Effect::Ignored
        );
        let mut transaction = prepared();
        let token = transaction.token();
        transaction.take_approval(token, context(), now).unwrap();
        assert_eq!(transaction.cancel(), Cancellation::FinishDetached);
        assert!(
            matches!(transaction.approval(token,Ok(Approval::Approved),now),Effect::SendCreate(packet) if packet.opcode==creation::OP_CREATE)
        );
        assert_eq!(
            transaction.roster(token, 5, &[character()], now),
            Effect::Updated
        );
        assert!(matches!(transaction.phase(), Phase::Completed(_)));
        assert!(transaction.detached());
    }
    #[test]
    fn uncertain_transport_timeout_unknown_reply_and_new_socket_never_retry_or_delete() {
        let now = Instant::now();
        for failure in 0..5 {
            let mut transaction = prepared();
            let token = transaction.token();
            transaction.take_approval(token, context(), now).unwrap();
            match failure {
                0 => transaction.transport_failed(token),
                1 => {
                    assert!(transaction.expire(now + REPLY_TIMEOUT));
                }
                2 => {
                    transaction.approval(token, Ok(Approval::Unknown(3)), now);
                }
                3 => transaction.session_changed(),
                _ => {
                    transaction.approval(token, Err(CreationError::Invalid("reply")), now);
                }
            }
            assert!(matches!(transaction.phase(), Phase::Uncertain(_)));
            assert!(transaction.take_approval(token, context(), now).is_err());
            assert_eq!(
                transaction.approval(token, Ok(Approval::Approved), now),
                Effect::Ignored
            );
        }
        let mut transaction = prepared();
        let token = transaction.token();
        transaction.take_approval(token, context(), now).unwrap();
        assert_eq!(
            transaction.approval(token, Ok(Approval::Approved), now + REPLY_TIMEOUT),
            Effect::Updated
        );
        assert_eq!(
            transaction.phase(),
            &Phase::Uncertain(Uncertainty::TimedOut)
        );
    }
    #[test]
    fn fresh_roster_confirms_identity_without_predicting_server_start_zone() {
        let now = Instant::now();
        let mut transaction = prepared();
        let token = transaction.token();
        assert_eq!(
            transaction.roster(token, 5, &[character()], now),
            Effect::Ignored
        );
        transaction.take_approval(token, context(), now).unwrap();
        let mut changed = token;
        changed.context.connection += 1;
        assert_eq!(
            transaction.approval(changed, Ok(Approval::Approved), now),
            Effect::Ignored
        );
        assert!(matches!(
            transaction.approval(token, Ok(Approval::Approved), now),
            Effect::SendCreate(_)
        ));
        assert_eq!(
            transaction.roster(token, 4, &[character()], now),
            Effect::Ignored
        );
        let mut actual = character();
        actual.zone = 394;
        assert_eq!(
            transaction.roster(token, 5, &[actual.clone()], now),
            Effect::Updated
        );
        assert_eq!(transaction.phase(), &Phase::Completed(Box::new(actual)));
        for roster in [
            vec![],
            {
                let mut mismatch = character();
                mismatch.race = 2;
                vec![mismatch]
            },
            {
                let mut reserved = character();
                reserved.level = 0;
                vec![reserved]
            },
            vec![character(), character()],
        ] {
            let mut transaction = prepared();
            let token = transaction.token();
            transaction.take_approval(token, context(), now).unwrap();
            transaction.approval(token, Ok(Approval::Approved), now);
            transaction.roster(token, 5, &roster, now);
            assert!(matches!(transaction.phase(), Phase::Uncertain(_)));
        }
    }
    #[test]
    fn name_and_create_rejections_are_distinct_and_single_flight_owner_blocks_overlap() {
        let now = Instant::now();
        let mut state = CreationState::default();
        let token = state
            .begin(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context()),
            )
            .unwrap();
        assert_eq!(
            state.begin(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context())
            ),
            Err(BeginError::Busy)
        );
        let transaction = state.active_mut().unwrap();
        transaction.take_approval(token, context(), now).unwrap();
        transaction.approval(token, Ok(Approval::Rejected), now);
        assert_eq!(transaction.phase(), &Phase::Rejected(Rejection::Name));
        let next = state
            .begin(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context()),
            )
            .unwrap();
        assert_ne!(next.operation, token.operation);
        let transaction = state.active_mut().unwrap();
        assert_eq!(
            transaction.approval(token, Ok(Approval::Approved), now),
            Effect::Ignored
        );
        transaction.take_approval(next, context(), now).unwrap();
        transaction.approval(next, Ok(Approval::Approved), now);
        transaction.approval(next, Ok(Approval::Rejected), now);
        assert_eq!(transaction.phase(), &Phase::Rejected(Rejection::Creation));
        let token = state
            .begin(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context()),
            )
            .unwrap();
        let transaction = state.active_mut().unwrap();
        transaction.take_approval(token, context(), now).unwrap();
        transaction.transport_failed(token);
        assert_eq!(
            state.begin(
                context(),
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(context())
            ),
            Err(BeginError::InspectUncertainResult)
        );
        let mut fresh = context();
        fresh.connection += 1;
        fresh.roster_revision += 1;
        assert_eq!(
            state.begin(
                fresh,
                draft(),
                &catalog(),
                &capabilities(),
                &[],
                &policy(),
                receipt(fresh)
            ),
            Err(BeginError::InspectUncertainResult)
        );
        let mut another = draft();
        another.name = "Another".into();
        assert!(
            state
                .begin(
                    fresh,
                    another,
                    &catalog(),
                    &capabilities(),
                    &[],
                    &policy(),
                    receipt(fresh)
                )
                .is_ok()
        );
    }
    #[test]
    fn appearance_limits_intersect_helpers_and_preview_without_offering_ignored_palettes() {
        let catalog = CustomizationCatalog::default();
        let human = policy();
        assert_eq!(human.values(Feature::Hair), &[0, 1, 2, 3]);
        assert_eq!(human.values(Feature::Beard), &[0, 1, 2, 3, 4, 5]);
        assert!(!human.editable(Feature::HairColor));
        let elf =
            AppearancePolicy::for_preview(6, 2, 0, PreviewFamily::Luclin, &catalog, 0).unwrap();
        assert_eq!(elf.values(Feature::HairColor), &[13]);
        assert!(elf.colors_not_previewed);
        let dwarf =
            AppearancePolicy::for_preview(8, 2, 1, PreviewFamily::Luclin, &catalog, 0).unwrap();
        assert_eq!(dwarf.values(Feature::Beard), &[0, 1]);
        let classic =
            AppearancePolicy::for_preview(1, 2, 0, PreviewFamily::Classic, &catalog, 0).unwrap();
        assert!(!classic.editable(Feature::Hair));
        assert!(!classic.editable(Feature::Eye1));
        let metadata =
            CustomizationCatalog::parse("522^2^Parent^123^1,2^1,2,3,4^9^10^14^14^9^9^0^\n")
                .unwrap();
        let drakkin =
            AppearancePolicy::for_preview(522, 2, 0, PreviewFamily::Drakkin, &metadata, 2).unwrap();
        assert_eq!(drakkin.values(Feature::Face).len(), 7);
        assert_eq!(drakkin.values(Feature::Hair).len(), 9);
        assert_eq!(drakkin.values(Feature::Beard).len(), 12);
        assert_eq!(drakkin.values(Feature::Heritage), &[2]);
        assert!(!drakkin.colors_not_previewed);
        assert!(
            AppearancePolicy::for_preview(522, 3, 0, PreviewFamily::Drakkin, &metadata, 2).is_err()
        );
        assert!(
            AppearancePolicy::for_preview(522, 2, 0, PreviewFamily::Drakkin, &metadata, 7).is_err()
        );
        assert!(
            AppearancePolicy::for_preview(522, 2, 0, PreviewFamily::Luclin, &metadata, 2).is_err()
        );
    }
}
