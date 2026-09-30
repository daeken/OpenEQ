//! Local creation drafts. Browsing never claims or sends a network packet.
use super::*;
use openeq_net::{creation::Combination, world::CharacterSelection};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoiceField {
    Race,
    Class,
    Gender,
    Deity,
    StartZone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Strength,
    Dexterity,
    Agility,
    Stamina,
    Intelligence,
    Wisdom,
    Charisma,
}
impl Stat {
    pub const ALL: [Self; 7] = [
        Self::Strength,
        Self::Stamina,
        Self::Agility,
        Self::Dexterity,
        Self::Wisdom,
        Self::Intelligence,
        Self::Charisma,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Strength => "Strength",
            Self::Dexterity => "Dexterity",
            Self::Agility => "Agility",
            Self::Stamina => "Stamina",
            Self::Intelligence => "Intelligence",
            Self::Wisdom => "Wisdom",
            Self::Charisma => "Charisma",
        }
    }
    fn field(self, stats: &mut Stats) -> &mut u32 {
        match self {
            Self::Strength => &mut stats.strength,
            Self::Dexterity => &mut stats.dexterity,
            Self::Agility => &mut stats.agility,
            Self::Stamina => &mut stats.stamina,
            Self::Intelligence => &mut stats.intelligence,
            Self::Wisdom => &mut stats.wisdom,
            Self::Charisma => &mut stats.charisma,
        }
    }
    pub fn value(self, mut stats: Stats) -> u32 {
        *self.field(&mut stats)
    }
}

pub struct Editor {
    context: Context,
    draft: Draft,
    selection: CharacterSelection,
    eligible: Vec<Combination>,
    policy: Option<AppearancePolicy>,
    receipt: Option<PreviewReceipt>,
    heritages: Vec<u32>,
    preview_problem: Option<String>,
}
impl Editor {
    pub fn new(session: u64, selection: &CharacterSelection) -> Result<Self, CreationError> {
        let eligible = eligible(selection)?;
        let first = eligible
            .first()
            .ok_or(CreationError::Unavailable("no permitted character choices"))?;
        let stats = selection
            .catalog
            .as_ref()
            .unwrap()
            .allocation(first.allocation_index)
            .unwrap()
            .default_stats()?;
        Ok(Self {
            context: Context {
                session,
                connection: selection.connection,
                catalog_revision: selection.catalog_revision,
                roster_revision: selection.roster_revision,
                draft_revision: 1,
            },
            draft: Draft {
                name: String::new(),
                choice: first.choice,
                gender: 0,
                appearance: Appearance::default(),
                stats,
            },
            selection: selection.clone(),
            eligible,
            policy: None,
            receipt: None,
            heritages: Vec::new(),
            preview_problem: None,
        })
    }
    pub fn context(&self) -> Context {
        self.context
    }
    pub fn draft(&self) -> &Draft {
        &self.draft
    }
    pub fn policy(&self) -> Option<&AppearancePolicy> {
        self.policy.as_ref()
    }
    pub fn preview_problem(&self) -> Option<&str> {
        self.preview_problem.as_deref()
    }
    pub fn receipt_ready(&self) -> bool {
        self.receipt.is_some()
    }
    fn changed(&mut self, model_changed: bool) {
        self.context.draft_revision = self.context.draft_revision.wrapping_add(1);
        self.receipt = None;
        self.preview_problem = None;
        if model_changed {
            self.policy = None;
            self.heritages.clear();
        }
    }
    /// A new socket/session must close this editor. New advertisements on the
    /// same socket keep a local draft only after revalidation and re-preview.
    pub fn refresh(
        &mut self,
        session: u64,
        selection: &CharacterSelection,
    ) -> Result<bool, CreationError> {
        if session != self.context.session || selection.connection != self.context.connection {
            return Err(CreationError::Unavailable(
                "character selection session changed",
            ));
        }
        if (selection.catalog_revision, selection.roster_revision)
            == (self.context.catalog_revision, self.context.roster_revision)
        {
            return Ok(false);
        }
        self.context.catalog_revision = selection.catalog_revision;
        self.context.roster_revision = selection.roster_revision;
        self.selection = selection.clone();
        self.changed(true);
        self.eligible = eligible(selection).unwrap_or_default();
        let chosen = self
            .eligible
            .iter()
            .find(|combo| combo.choice == self.draft.choice)
            .or_else(|| self.eligible.first())
            .ok_or(CreationError::Unavailable("no permitted character choices"))?;
        self.draft.choice = chosen.choice;
        self.draft.stats = selection
            .catalog
            .as_ref()
            .unwrap()
            .allocation(chosen.allocation_index)
            .unwrap()
            .default_stats()?;
        Ok(true)
    }
    pub fn set_name(&mut self, name: String) -> bool {
        if self.draft.name == name
            || name.len() > 15
            || !name.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return false;
        }
        self.draft.name = name;
        self.changed(false);
        true
    }
    pub fn value(&self, field: ChoiceField) -> u32 {
        let choice = self.draft.choice;
        match field {
            ChoiceField::Race => choice.race,
            ChoiceField::Class => choice.class,
            ChoiceField::Gender => u32::from(self.draft.gender),
            ChoiceField::Deity => choice.deity,
            ChoiceField::StartZone => choice.start_zone,
        }
    }
    pub fn choices(&self, field: ChoiceField) -> Vec<u32> {
        if field == ChoiceField::Gender {
            return vec![0, 1];
        }
        let current = self.draft.choice;
        self.eligible
            .iter()
            .filter(|combo| match field {
                ChoiceField::Race => true,
                ChoiceField::Class => combo.choice.race == current.race,
                ChoiceField::Deity => {
                    combo.choice.race == current.race && combo.choice.class == current.class
                }
                ChoiceField::StartZone => {
                    combo.choice.race == current.race
                        && combo.choice.class == current.class
                        && combo.choice.deity == current.deity
                }
                ChoiceField::Gender => unreachable!(),
            })
            .map(|combo| match field {
                ChoiceField::Race => combo.choice.race,
                ChoiceField::Class => combo.choice.class,
                ChoiceField::Deity => combo.choice.deity,
                ChoiceField::StartZone => combo.choice.start_zone,
                ChoiceField::Gender => unreachable!(),
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn cycle_choice(&mut self, field: ChoiceField, direction: i32) -> bool {
        let values = self.choices(field);
        let Some(value) = cycle(&values, self.value(field), direction) else {
            return false;
        };
        if field == ChoiceField::Gender {
            self.draft.gender = value as u8;
        } else {
            let current = self.draft.choice;
            let best = self
                .eligible
                .iter()
                .filter(|combo| {
                    let candidate = combo.choice;
                    match field {
                        ChoiceField::Race => candidate.race == value,
                        ChoiceField::Class => {
                            candidate.race == current.race && candidate.class == value
                        }
                        ChoiceField::Deity => {
                            candidate.race == current.race
                                && candidate.class == current.class
                                && candidate.deity == value
                        }
                        ChoiceField::StartZone => {
                            candidate.race == current.race
                                && candidate.class == current.class
                                && candidate.deity == current.deity
                                && candidate.start_zone == value
                        }
                        ChoiceField::Gender => unreachable!(),
                    }
                })
                .max_by_key(|combo| {
                    u8::from(combo.choice.class == current.class) * 4
                        + u8::from(combo.choice.deity == current.deity) * 2
                        + u8::from(combo.choice.start_zone == current.start_zone)
                })
                .unwrap();
            self.draft.choice = best.choice;
            self.draft.stats = self
                .selection
                .catalog
                .as_ref()
                .unwrap()
                .allocation(best.allocation_index)
                .unwrap()
                .default_stats()
                .unwrap();
        }
        self.draft.appearance = Appearance::default();
        self.changed(true);
        true
    }
    pub fn remaining_points(&self) -> Result<u32, CreationError> {
        self.selection
            .catalog
            .as_ref()
            .ok_or(CreationError::Missing("creation choices"))?
            .resolve(self.draft.choice)?
            .1
            .validate_stats(self.draft.stats)
    }
    pub fn adjust_stat(&mut self, stat: Stat, direction: i32) -> bool {
        let mut candidate = self.draft.stats;
        let value = stat.field(&mut candidate);
        let Some(next) = (if direction > 0 {
            value.checked_add(1)
        } else {
            value.checked_sub(1)
        }) else {
            return false;
        };
        *value = next;
        let Some(catalog) = &self.selection.catalog else {
            return false;
        };
        let Ok((_, allocation)) = catalog.resolve(self.draft.choice) else {
            return false;
        };
        if allocation.validate_stats(candidate).is_err() {
            return false;
        }
        self.draft.stats = candidate;
        self.changed(false);
        true
    }
    pub fn reset_stats(&mut self) -> bool {
        let Some(catalog) = &self.selection.catalog else {
            return false;
        };
        let Ok((_, allocation)) = catalog.resolve(self.draft.choice) else {
            return false;
        };
        let Ok(stats) = allocation.default_stats() else {
            return false;
        };
        if self.draft.stats == stats {
            return false;
        }
        self.draft.stats = stats;
        self.changed(false);
        true
    }
    pub fn feature_values(&self, feature: Feature) -> &[u32] {
        if feature == Feature::Heritage {
            &self.heritages
        } else {
            self.policy
                .as_ref()
                .map_or(&[], |policy| policy.values(feature))
        }
    }
    pub fn feature(&self, feature: Feature) -> u32 {
        let a = self.draft.appearance;
        match feature {
            Feature::Face => a.face.into(),
            Feature::Hair => a.hair_style.into(),
            Feature::Beard => a.beard.into(),
            Feature::HairColor => a.hair_color.into(),
            Feature::BeardColor => a.beard_color.into(),
            Feature::Eye1 => a.eye_color_1.into(),
            Feature::Eye2 => a.eye_color_2.into(),
            Feature::Heritage => a.heritage,
            Feature::Tattoo => a.tattoo,
            Feature::Details => a.details,
        }
    }
    pub fn cycle_feature(&mut self, feature: Feature, direction: i32) -> bool {
        let Some(value) = cycle(
            self.feature_values(feature),
            self.feature(feature),
            direction,
        ) else {
            return false;
        };
        let a = &mut self.draft.appearance;
        match feature {
            Feature::Face => a.face = value as u8,
            Feature::Hair => a.hair_style = value as u8,
            Feature::Beard => a.beard = value as u8,
            Feature::HairColor => a.hair_color = value as u8,
            Feature::BeardColor => a.beard_color = value as u8,
            Feature::Eye1 => a.eye_color_1 = value as u8,
            Feature::Eye2 => a.eye_color_2 = value as u8,
            Feature::Heritage => a.heritage = value,
            Feature::Tattoo => a.tattoo = value,
            Feature::Details => a.details = value,
        }
        self.changed(feature == Feature::Heritage);
        true
    }
    /// Policy comes from the actual loaded preview. If its family changed the
    /// supported defaults, update only the local draft and require a new receipt.
    pub fn apply_preview(
        &mut self,
        context: Context,
        policy: AppearancePolicy,
        receipt: Option<PreviewReceipt>,
        heritages: Vec<u32>,
    ) -> bool {
        if context != self.context {
            return false;
        }
        self.preview_problem = None;
        if policy.validate(&self.draft).is_err() {
            self.draft.appearance = policy.default_appearance();
            self.changed(false);
            self.policy = Some(policy);
            self.heritages = heritages;
            return true;
        }
        self.policy = Some(policy);
        self.heritages = heritages;
        self.receipt = receipt.filter(|receipt| {
            receipt.matches(
                self.context,
                &self.draft,
                self.policy.as_ref().unwrap().family(),
            )
        });
        false
    }
    pub fn preview_failed(&mut self, context: Context, message: String) {
        if context == self.context {
            self.receipt = None;
            self.preview_problem = Some(message);
        }
    }
    pub fn submission(&self) -> Result<Submission, CreationError> {
        let catalog = self
            .selection
            .catalog
            .as_ref()
            .ok_or(CreationError::Missing("creation choices"))?;
        let appearance = self
            .policy
            .as_ref()
            .ok_or(CreationError::Missing("appearance preview"))?;
        let preview = self
            .receipt
            .ok_or(CreationError::Missing("current appearance preview"))?;
        // Prepare validates the exact same frozen contract as the worker, but
        // claiming/sending remains exclusively in the account worker.
        Transaction::prepare(
            self.context,
            self.draft.clone(),
            catalog,
            &self.selection.capabilities,
            &self.selection.characters,
            appearance,
            preview,
        )?;
        Ok(Submission {
            context: self.context,
            draft: self.draft.clone(),
            appearance: appearance.clone(),
            preview,
        })
    }
}
fn eligible(selection: &CharacterSelection) -> Result<Vec<Combination>, CreationError> {
    let catalog = selection
        .catalog
        .as_ref()
        .ok_or(CreationError::Missing("creation choices"))?;
    Ok(catalog
        .combinations()
        .iter()
        .filter(|combo| {
            selection
                .capabilities
                .permits(combo, selection.characters.len())
                .is_ok()
        })
        .copied()
        .collect())
}
fn cycle(values: &[u32], current: u32, direction: i32) -> Option<u32> {
    if values.len() < 2 {
        return None;
    }
    let index = values
        .iter()
        .position(|value| *value == current)
        .unwrap_or(0);
    Some(values[(index + if direction > 0 { 1 } else { values.len() - 1 }) % values.len()])
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn selection() -> CharacterSelection {
        let mut bytes = vec![0];
        let mut words = vec![1u32, 17, 71, 72, 73, 74, 75, 76, 77, 1, 2, 3, 4, 5, 6, 7, 5];
        for (race, class, deity, zone) in [
            (1, 2, 201, 77),
            (1, 2, 201, 394),
            (1, 1, 396, 394),
            (6, 2, 206, 394),
            (522, 2, 216, 394),
        ] {
            words.extend([0, race, class, deity, 17, zone]);
        }
        for word in words {
            bytes.extend(word.to_le_bytes());
        }
        CharacterSelection {
            connection: 4,
            catalog_revision: 5,
            roster_revision: 6,
            capabilities: Capabilities {
                expansion_mask: Some(u32::MAX),
                maximum_characters: Some(12),
                membership: Some(creation::Membership {
                    tier: 2,
                    race_mask: 0xffff,
                    class_mask: 0xffff,
                    settings: [-1; 25],
                }),
            },
            catalog: Some(std::sync::Arc::new(Catalog::parse(&bytes).unwrap())),
            ..Default::default()
        }
    }
    pub(crate) fn attach_preview(editor: &mut Editor) {
        let d = editor.draft();
        let policy = AppearancePolicy::for_preview(
            d.choice.race,
            d.choice.class,
            d.gender,
            PreviewFamily::Classic,
            &CustomizationCatalog::default(),
            0,
        )
        .unwrap();
        let receipt = PreviewReceipt {
            context: editor.context(),
            family: PreviewFamily::Classic,
            model_loaded: true,
            race: d.choice.race,
            class: d.choice.class,
            gender: d.gender,
            appearance: d.appearance,
        };
        editor.apply_preview(editor.context(), policy, Some(receipt), vec![]);
    }
    #[test]
    fn dependent_choices_always_resolve_to_advertised_combinations() {
        let selection = selection();
        let mut editor = Editor::new(3, &selection).unwrap();
        assert_eq!(editor.choices(ChoiceField::Race), [1, 6, 522]);
        assert_eq!(editor.choices(ChoiceField::StartZone), [77, 394]);
        assert!(editor.cycle_choice(ChoiceField::Class, 1));
        assert_eq!(editor.draft.choice.class, 1);
        assert_eq!(editor.choices(ChoiceField::Deity), [396]);
        assert_eq!(editor.choices(ChoiceField::StartZone), [394]);
        assert!(!editor.cycle_choice(ChoiceField::Deity, 1));
        for field in [
            ChoiceField::Race,
            ChoiceField::Class,
            ChoiceField::Deity,
            ChoiceField::StartZone,
        ] {
            for _ in 0..8 {
                editor.cycle_choice(field, 1);
                assert!(
                    selection
                        .catalog
                        .as_ref()
                        .unwrap()
                        .resolve(editor.draft.choice)
                        .is_ok()
                );
            }
        }
    }
    #[test]
    fn stats_allow_redistribution_but_never_below_base_or_over_budget() {
        let mut editor = Editor::new(3, &selection()).unwrap();
        let defaults = editor.draft.stats;
        assert_eq!(
            defaults,
            Stats::from_catalog_order([72, 74, 76, 78, 80, 82, 84])
        );
        assert_eq!(editor.remaining_points().unwrap(), 0);
        assert!(!editor.adjust_stat(Stat::Strength, 1));
        assert!(editor.adjust_stat(Stat::Strength, -1));
        assert!(!editor.adjust_stat(Stat::Strength, -1));
        assert_eq!(editor.remaining_points().unwrap(), 1);
        assert!(editor.adjust_stat(Stat::Wisdom, 1));
        assert!(!editor.adjust_stat(Stat::Wisdom, 1));
        assert!(editor.reset_stats());
        assert_eq!(editor.draft.stats, defaults);
        assert!(!editor.reset_stats());
    }
    #[test]
    fn missing_membership_and_full_capacity_never_offer_creation() {
        let mut selection = selection();
        selection.capabilities.membership = None;
        assert!(Editor::new(3, &selection).is_err());
        let mut selection = self::selection();
        selection.capabilities.maximum_characters = Some(0);
        assert!(Editor::new(3, &selection).is_err());
    }
    #[test]
    fn edits_and_refresh_retire_receipts_and_socket_change_rejects_editor() {
        let mut selection = selection();
        let mut editor = Editor::new(3, &selection).unwrap();
        assert!(editor.set_name("Asteria".into()));
        attach_preview(&mut editor);
        assert!(editor.submission().is_ok());
        let old_context = editor.context();
        editor.adjust_stat(Stat::Strength, -1);
        assert!(!editor.receipt_ready());
        assert!(editor.context().draft_revision > old_context.draft_revision);
        attach_preview(&mut editor);
        selection.roster_revision += 1;
        assert!(editor.refresh(3, &selection).unwrap());
        assert!(!editor.receipt_ready());
        assert_eq!(editor.remaining_points().unwrap(), 0);
        attach_preview(&mut editor);
        selection.catalog_revision += 1;
        selection.capabilities.maximum_characters = Some(0);
        assert!(editor.refresh(3, &selection).is_err());
        assert!(!editor.receipt_ready());
        assert!(editor.submission().is_err());
        selection.connection += 1;
        assert!(editor.refresh(3, &selection).is_err());
    }
    #[test]
    fn actual_family_defaults_require_a_new_preview_and_receipt_matches_exact_appearance() {
        let mut editor = Editor::new(3, &selection()).unwrap();
        editor.set_name("Asteria".into());
        editor.cycle_choice(ChoiceField::Race, 1);
        let provisional = editor.context();
        attach_preview(&mut editor);
        assert_eq!(editor.draft.appearance.hair_color, 13);
        assert_eq!(editor.draft.appearance.beard_color, 13);
        assert_ne!(editor.context(), provisional);
        assert!(!editor.receipt_ready());
        attach_preview(&mut editor);
        assert!(editor.submission().is_ok());
        let mut wrong = editor.receipt.unwrap();
        wrong.appearance.face = 3;
        editor.apply_preview(
            editor.context(),
            editor.policy.clone().unwrap(),
            Some(wrong),
            vec![],
        );
        assert!(!editor.receipt_ready());
        attach_preview(&mut editor);
        editor.cycle_feature(Feature::Face, 1);
        assert!(!editor.receipt_ready());
        assert!(!editor.cycle_feature(Feature::HairColor, 1));
        assert_eq!(editor.feature_values(Feature::HairColor), [13]);
    }
}
