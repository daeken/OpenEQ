//! Server-confirmed base skills and level progress. Profile totals are snapshots;
//! a 330-unit experience bar never establishes a new absolute experience total.
use crate::game::StringTable;
use openeq_net::{gameplay::PlayerProfile, progression::ProgressionEvent};

pub const SKILL_COUNT: usize = 78;
pub const LANGUAGE_COUNT: usize = 28;
const SKILL_STRINGS: [u32; SKILL_COUNT] = [
    13855, 13856, 13857, 13858, 13859, 13861, 13862, 13863, 13864, 13866, 13867, 13871, 13872,
    13874, 13875, 13876, 13877, 13878, 13879, 13880, 13881, 13882, 13883, 13884, 13885, 13886,
    13888, 13889, 13890, 13891, 13893, 13894, 13895, 13896, 13897, 13899, 13900, 13903, 13904,
    13905, 13906, 13908, 13909, 13910, 13911, 13912, 13913, 13914, 13915, 13916, 13917, 13919,
    13920, 13921, 13923, 13854, 13853, 13852, 13851, 13850, 13865, 13918, 13907, 13870, 13887,
    13873, 13860, 13868, 13892, 13901, 13898, 13922, 13869, 13902, 5837, 3670, 13049, 789,
];
const LANGUAGE_STRINGS: [Option<u32>; LANGUAGE_COUNT] = [
    Some(3114),
    Some(3200),
    Some(3201),
    Some(3202),
    Some(3203),
    Some(3204),
    Some(3205),
    Some(3206),
    Some(3207),
    Some(3208),
    Some(3217),
    Some(3219),
    Some(3220),
    Some(3209),
    Some(3210),
    Some(3211),
    Some(3212),
    Some(3213),
    Some(3214),
    Some(3215),
    Some(3216),
    Some(3218),
    Some(3221),
    Some(3222),
    Some(3223),
    Some(7658),
    Some(7659),
    None,
];

pub fn skill_name(strings: &StringTable, id: u32) -> String {
    SKILL_STRINGS
        .get(id as usize)
        .and_then(|id| strings.get(*id))
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Skill {id}"))
}
pub fn language_name(strings: &StringTable, id: u32) -> String {
    LANGUAGE_STRINGS
        .get(id as usize)
        .copied()
        .flatten()
        .and_then(|id| strings.get(id))
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Language {id}"))
}

pub struct ProgressionState {
    pub confirmed: bool,
    pub revision: u64,
    pub level: Option<u32>,
    /// Raw received ratio; values above 330 remain observable but not a percent.
    pub experience_bar_units: Option<u32>,
    pub aa_experience_bar_units: Option<u32>,
    /// Latest received balance: full-width profile or zero-extended u16 stats.
    pub aa_unspent_points: Option<u32>,
    /// Preserve invalid percentages raw; presentation must not invent a value.
    pub aa_allocation_percent: Option<u8>,
    pub skills: [Option<u32>; SKILL_COUNT],
    pub languages: [Option<u32>; LANGUAGE_COUNT],
    /// Last profile snapshots, never inferred from gains, levels or trainer replies.
    pub profile_training_points: Option<u32>,
    pub profile_experience_total: Option<u64>,
}
impl Default for ProgressionState {
    fn default() -> Self {
        Self {
            confirmed: false,
            revision: 0,
            level: None,
            experience_bar_units: None,
            aa_experience_bar_units: None,
            aa_unspent_points: None,
            aa_allocation_percent: None,
            skills: [None; SKILL_COUNT],
            languages: [None; LANGUAGE_COUNT],
            profile_training_points: None,
            profile_experience_total: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SkillChange {
    pub id: u32,
    pub old: u32,
    pub value: u32,
}

impl ProgressionState {
    pub fn begin_zone(&mut self) {
        self.confirmed = false;
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn profile(&mut self, profile: &PlayerProfile) {
        self.replace(
            u32::from(profile.level),
            &profile.skills,
            &profile.languages,
            profile.training_points,
            profile.experience_total,
        );
        self.aa_unspent_points = Some(profile.aa_unspent_points);
    }

    fn replace(&mut self, level: u32, skills: &[u32], languages: &[u8], points: u32, xp: u64) {
        self.confirmed = true;
        self.level = Some(level);
        self.experience_bar_units = None;
        self.aa_experience_bar_units = None;
        self.aa_unspent_points = None;
        self.aa_allocation_percent = None;
        self.skills = std::array::from_fn(|id| skills.get(id).copied());
        self.languages = std::array::from_fn(|id| languages.get(id).copied().map(u32::from));
        self.profile_training_points = Some(points);
        self.profile_experience_total = Some(xp);
        self.revision = self.revision.wrapping_add(1);
    }

    /// Updates before a new zone's profile cannot revalidate an old snapshot.
    /// Language notices already arrive as server messages, so only normal skill
    /// changes can supply local feedback, and duplicates never produce it.
    pub fn apply(&mut self, event: ProgressionEvent) -> Option<SkillChange> {
        if !self.confirmed {
            return None;
        }
        let mut skill_change = None;
        let changed = match event {
            ProgressionEvent::AlternateAdvancement {
                bar_units,
                unspent_points,
                allocation_percent,
            } => {
                let unspent_points = u32::from(unspent_points);
                let changed = self.aa_experience_bar_units != Some(bar_units)
                    || self.aa_unspent_points != Some(unspent_points)
                    || self.aa_allocation_percent != Some(allocation_percent);
                self.aa_experience_bar_units = Some(bar_units);
                self.aa_unspent_points = Some(unspent_points);
                self.aa_allocation_percent = Some(allocation_percent);
                changed
            }
            ProgressionEvent::Experience { bar_units } => {
                let changed = self.experience_bar_units != Some(bar_units);
                self.experience_bar_units = Some(bar_units);
                changed
            }
            ProgressionEvent::Level {
                level, bar_units, ..
            } => {
                let changed =
                    self.level != Some(level) || self.experience_bar_units != Some(bar_units);
                self.level = Some(level);
                self.experience_bar_units = Some(bar_units);
                changed
            }
            ProgressionEvent::SkillValue {
                wire_skill_id,
                value,
            } => {
                let slot = match wire_skill_id {
                    0..=77 => &mut self.skills[wire_skill_id as usize],
                    100..=127 => &mut self.languages[(wire_skill_id - 100) as usize],
                    _ => return None,
                };
                if *slot == Some(value) {
                    return None;
                }
                if wire_skill_id < 100 {
                    skill_change = slot.map(|old| SkillChange {
                        id: wire_skill_id,
                        old,
                        value,
                    });
                }
                *slot = Some(value);
                true
            }
        };
        if changed {
            self.revision = self.revision.wrapping_add(1);
        }
        skill_change
    }

    pub fn experience_fraction(&self) -> Option<f32> {
        self.confirmed
            .then_some(self.experience_bar_units)
            .flatten()
            .filter(|units| *units <= openeq_net::progression::EXPERIENCE_BAR_UNITS)
            .map(|units| units as f32 / openeq_net::progression::EXPERIENCE_BAR_UNITS as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn skill(id: u32, value: u32) -> ProgressionEvent {
        ProgressionEvent::SkillValue {
            wire_skill_id: id,
            value,
        }
    }
    #[test]
    fn skill_updates_preserve_missing_zero_wide_values_and_language_identity() {
        let mut state = ProgressionState::default();
        state.replace(10, &[55, 0], &[100], 3, 999);
        assert_eq!(state.skills[2], None);
        assert_eq!(
            state.apply(skill(0, 56)),
            Some(SkillChange {
                id: 0,
                old: 55,
                value: 56
            })
        );
        let revision = state.revision;
        assert_eq!(state.apply(skill(0, 56)), None);
        assert_eq!(state.revision, revision);
        assert_eq!(
            state.apply(skill(0, 54)),
            Some(SkillChange {
                id: 0,
                old: 56,
                value: 54
            })
        );
        for value in [0, 254, 255, u32::MAX] {
            state.apply(skill(77, value));
            assert_eq!(state.skills[77], Some(value));
        }
        assert_eq!(state.apply(skill(100, 99)), None);
        assert_eq!(state.languages[0], Some(99));
        assert_eq!(state.skills[0], Some(54));
        state.apply(skill(127, 7));
        assert_eq!(state.languages[27], Some(7));
        let revision = state.revision;
        for id in [78, 99, 128, u32::MAX] {
            state.apply(skill(id, 3));
        }
        assert_eq!(state.revision, revision);
    }
    #[test]
    fn level_and_ratio_updates_never_invent_totals_points_or_wrap_levels() {
        let mut state = ProgressionState::default();
        state.replace(20, &[], &[], 5, 123_456);
        assert_eq!(state.experience_fraction(), None);
        for (bar_units, expected) in [
            (0, Some(0.)),
            (165, Some(0.5)),
            (330, Some(1.)),
            (331, None),
            (u32::MAX, None),
        ] {
            state.apply(ProgressionEvent::Experience { bar_units });
            assert_eq!(state.experience_fraction(), expected);
        }
        state.apply(ProgressionEvent::Level {
            level: 19,
            reported_old_level: 25,
            bar_units: u32::MAX,
        });
        assert_eq!(state.level, Some(19));
        assert_eq!(state.experience_fraction(), None);
        state.apply(ProgressionEvent::Experience { bar_units: 33 });
        assert_eq!(state.experience_fraction(), Some(0.1));
        state.apply(ProgressionEvent::Level {
            level: 70_000,
            reported_old_level: 1,
            bar_units: 0,
        });
        assert_eq!(state.level, Some(70_000));
        assert_eq!(state.profile_training_points, Some(5));
        assert_eq!(state.profile_experience_total, Some(123_456));
    }
    #[test]
    fn aa_is_independent_preserves_raw_values_and_requires_a_fresh_profile() {
        let mut state = ProgressionState::default();
        let mut profile = crate::item_use_state::tests::fixture()
            .profile
            .take()
            .unwrap();
        profile.aa_unspent_points = u32::MAX;
        profile.training_points = 7;
        profile.experience_total = 123_456;
        let event = ProgressionEvent::AlternateAdvancement {
            bar_units: 165,
            unspent_points: 12,
            allocation_percent: 50,
        };
        state.apply(event);
        assert_eq!(state.aa_unspent_points, None);
        state.profile(&profile);
        assert_eq!(state.aa_unspent_points, Some(u32::MAX));
        assert_eq!(state.aa_experience_bar_units, None);
        assert_eq!(state.aa_allocation_percent, None);
        state.apply(ProgressionEvent::Experience { bar_units: 33 });
        assert_eq!(state.apply(event), None);
        let revision = state.revision;
        state.apply(event);
        assert_eq!(state.revision, revision);
        assert_eq!(state.aa_unspent_points, Some(12));
        assert_eq!(state.aa_experience_bar_units, Some(165));
        assert_eq!(state.aa_allocation_percent, Some(50));
        for (bar_units, unspent_points, allocation_percent) in
            [(0, 0, 0), (330, u16::MAX, 100), (u32::MAX, 1, u8::MAX)]
        {
            assert_eq!(
                state.apply(ProgressionEvent::AlternateAdvancement {
                    bar_units,
                    unspent_points,
                    allocation_percent,
                }),
                None
            );
            assert_eq!(state.aa_experience_bar_units, Some(bar_units));
            assert_eq!(state.aa_unspent_points, Some(u32::from(unspent_points)));
            assert_eq!(state.aa_allocation_percent, Some(allocation_percent));
        }
        assert_eq!(state.experience_fraction(), Some(0.1));
        assert_eq!(state.profile_training_points, Some(7));
        assert_eq!(state.profile_experience_total, Some(123_456));
        assert_eq!(state.skills, [None; SKILL_COUNT]);
        assert_eq!(state.languages, [None; LANGUAGE_COUNT]);
        state.begin_zone();
        let revision = state.revision;
        state.apply(event);
        assert_eq!(state.revision, revision);
        assert_eq!(state.aa_unspent_points, Some(1));
        profile.aa_unspent_points = 70_001;
        state.profile(&profile);
        assert_eq!(state.aa_unspent_points, Some(70_001));
        assert_eq!(state.aa_experience_bar_units, None);
        assert_eq!(state.aa_allocation_percent, None);
        let new_character = ProgressionState::default();
        assert_eq!(new_character.aa_unspent_points, None);
    }
    #[test]
    fn travel_requires_a_fresh_profile_and_does_not_celebrate_snapshot_changes() {
        let mut state = ProgressionState::default();
        state.apply(skill(0, 90));
        assert_eq!(state.skills[0], None);
        state.replace(10, &[55], &[100], 2, 3);
        state.apply(ProgressionEvent::Experience { bar_units: 165 });
        state.begin_zone();
        let revision = state.revision;
        state.apply(skill(0, 90));
        state.apply(ProgressionEvent::Experience { bar_units: 330 });
        assert_eq!(state.revision, revision);
        assert_eq!(state.skills[0], Some(55));
        assert_eq!(state.experience_fraction(), None);
        state.replace(11, &[95], &[60], 7, 8);
        assert_eq!(state.skills[0], Some(95));
        assert_eq!(state.languages[0], Some(60));
        assert_eq!(state.experience_bar_units, None);
        assert_eq!(state.apply(skill(0, 95)), None);
    }
    #[test]
    fn names_use_explicit_original_ids_and_missing_names_stay_identifiable() {
        let strings = StringTable::parse(
            "13855 1H Blunt\n13854 Fishing\n5837 Frenzy\n789 2H Piercing\n3114 Common Tongue\n7658 Alaran\n7659 Hadal\n",
        );
        for (id, name) in [
            (0, "1H Blunt"),
            (55, "Fishing"),
            (74, "Frenzy"),
            (77, "2H Piercing"),
        ] {
            assert_eq!(skill_name(&strings, id), name);
        }
        assert_eq!(skill_name(&strings, 1), "Skill 1");
        assert_eq!(language_name(&strings, 0), "Common Tongue");
        assert_eq!(language_name(&strings, 25), "Alaran");
        assert_eq!(language_name(&strings, 26), "Hadal");
        assert_eq!(language_name(&strings, 27), "Language 27");
    }
    #[test]
    fn live_profile_updates_feedback_and_zone_gates_use_the_same_reducer() {
        use openeq_net::gameplay::GameplayEvent;
        let (mut live, mut wire) = crate::live::tests::command_world(1, 1.);
        let mut profile = crate::item_use_state::tests::fixture()
            .profile
            .take()
            .unwrap();
        profile.skills = vec![55; 100];
        profile.languages = vec![100; 32];
        profile.training_points = 7;
        profile.experience_total = 123456;
        profile.aa_unspent_points = 70_001;
        live.game.strings = StringTable::parse("13855 1H Blunt");
        live.gameplay_event(GameplayEvent::Profile(profile.clone()));
        assert_eq!(live.game.progression.skills[0], Some(55));
        assert_eq!(live.game.progression.profile_training_points, Some(7));
        assert_eq!(live.game.progression.profile_experience_total, Some(123456));
        assert_eq!(live.game.progression.aa_unspent_points, Some(70_001));
        assert!(live.game.chat.is_empty());
        let aa = GameplayEvent::Progression(ProgressionEvent::AlternateAdvancement {
            bar_units: 165,
            unspent_points: 12,
            allocation_percent: 50,
        });
        live.gameplay_event(aa.clone());
        assert_eq!(live.game.progression.aa_unspent_points, Some(12));
        assert!(live.game.chat.is_empty());
        live.gameplay_event(GameplayEvent::Progression(skill(0, 56)));
        assert!(
            live.game
                .chat
                .back()
                .unwrap()
                .text
                .contains("1H Blunt skill changed from 55 to 56")
        );
        let count = live.game.chat.len();
        live.gameplay_event(GameplayEvent::Progression(skill(0, 56)));
        live.gameplay_event(GameplayEvent::Progression(skill(100, 99)));
        assert_eq!(live.game.chat.len(), count);
        live.gameplay_event(GameplayEvent::Progression(ProgressionEvent::Level {
            level: 19,
            reported_old_level: 25,
            bar_units: u32::MAX,
        }));
        assert_eq!(live.game.profile.as_ref().unwrap().level, 19);
        assert_eq!(live.game.progression.level, Some(19));
        assert_eq!(live.game.progression.experience_fraction(), None);
        live.gameplay_event(GameplayEvent::Progression(ProgressionEvent::Experience {
            bar_units: 165,
        }));
        assert_eq!(live.game.progression.experience_fraction(), Some(0.5));
        live.gameplay_event(GameplayEvent::Progression(ProgressionEvent::Level {
            level: 70_000,
            reported_old_level: 1,
            bar_units: 0,
        }));
        assert_eq!(live.game.profile.as_ref().unwrap().level, 19);
        assert_eq!(live.game.progression.level, Some(70_000));
        live.gameplay_event(GameplayEvent::ZoneTransition {
            zone_id: 54,
            instance_id: 0,
        });
        let count = live.game.chat.len();
        live.gameplay_event(GameplayEvent::Progression(skill(0, 90)));
        assert_eq!(live.game.chat.len(), count);
        assert_eq!(live.game.progression.skills[0], Some(56));
        assert!(!live.game.progression.confirmed);
        live.gameplay_event(aa);
        assert_eq!(live.game.progression.aa_unspent_points, Some(12));
        profile.skills[0] = 99;
        live.gameplay_event(GameplayEvent::Profile(profile));
        assert_eq!(live.game.progression.skills[0], Some(99));
        assert_eq!(live.game.chat.len(), count);
        assert_eq!(live.game.progression.experience_bar_units, None);
        assert_eq!(live.game.progression.aa_unspent_points, Some(70_001));
        assert_eq!(live.game.progression.aa_experience_bar_units, None);
        assert_eq!(live.game.progression.aa_allocation_percent, None);
        assert!(wire.try_recv().is_err());
    }
}
