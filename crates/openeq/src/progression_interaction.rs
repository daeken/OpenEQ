//! Local skills/languages/AA window; opening and scrolling never send a packet.
use crate::{
    gameplay_ui::GameHudState,
    interaction::Interaction,
    live::LiveWorld,
    progression::{language_name, skill_name},
    progression_ui::{
        ProgressionAction, ProgressionActionKind, UiProgression, UiProgressionPage,
        UiProgressionRow,
    },
};
use std::sync::Arc;

pub struct SkillsWindowState {
    pub open: bool,
    pub page: UiProgressionPage,
    pub scroll: usize,
    pub visible_rows: usize,
    cached_revision: Option<u64>,
    skills: Arc<[UiProgressionRow]>,
    languages: Arc<[UiProgressionRow]>,
}
impl Default for SkillsWindowState {
    fn default() -> Self {
        Self {
            open: false,
            page: UiProgressionPage::Skills,
            scroll: 0,
            visible_rows: 1,
            cached_revision: None,
            skills: Arc::from([]),
            languages: Arc::from([]),
        }
    }
}
fn rows(live: &LiveWorld, page: UiProgressionPage) -> Arc<[UiProgressionRow]> {
    let progression = &live.game.progression;
    let values: &[Option<u32>] = match page {
        UiProgressionPage::Skills => &progression.skills,
        UiProgressionPage::Languages => &progression.languages,
        UiProgressionPage::AlternateAdvancement => &[],
    };
    values
        .iter()
        .enumerate()
        .map(|(id, value)| UiProgressionRow {
            id: id as u32,
            name: match page {
                UiProgressionPage::Skills => skill_name(&live.game.strings, id as u32),
                UiProgressionPage::Languages => language_name(&live.game.strings, id as u32),
                UiProgressionPage::AlternateAdvancement => unreachable!(),
            },
            value: *value,
        })
        .collect()
}
impl Interaction {
    pub(crate) fn progression_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let progression = &live.game.progression;
        let window = &self.skills_window;
        let current = live.ready && live.error.is_none() && progression.confirmed;
        let (skills, languages) = if window.cached_revision == Some(progression.revision) {
            (window.skills.clone(), window.languages.clone())
        } else {
            (
                rows(live, UiProgressionPage::Skills),
                rows(live, UiProgressionPage::Languages),
            )
        };
        view.progression = Some(UiProgression {
            revision: progression.revision,
            status: if current && window.page == UiProgressionPage::AlternateAdvancement {
                "Alternate advancement values reported by the server.".into()
            } else if current {
                "Base values reported by the server; bonuses and training caps are not included."
                    .into()
            } else {
                "Waiting for current character data; displayed values are the last received snapshot.".into()
            },
            current,
            level: progression.level,
            experience_bar_units: progression
                .experience_bar_units
                .filter(|units| *units <= openeq_net::progression::EXPERIENCE_BAR_UNITS),
            aa_experience_bar_units: progression.aa_experience_bar_units,
            aa_unspent_points: progression.aa_unspent_points,
            aa_allocation_percent: progression.aa_allocation_percent,
            skills,
            languages,
            open: window.open,
            page: window.page,
            scroll: window.scroll,
        });
    }

    pub(crate) fn progression_tick(&mut self, live: &LiveWorld) {
        if self.skills_window.cached_revision != Some(live.game.progression.revision) {
            self.skills_window.skills = rows(live, UiProgressionPage::Skills);
            self.skills_window.languages = rows(live, UiProgressionPage::Languages);
            self.skills_window.cached_revision = Some(live.game.progression.revision);
        }
        self.clamp_skills_scroll();
    }

    fn clamp_skills_scroll(&mut self) {
        let window = &mut self.skills_window;
        let count = match window.page {
            UiProgressionPage::Skills => window.skills.len(),
            UiProgressionPage::Languages => window.languages.len(),
            UiProgressionPage::AlternateAdvancement => 0,
        };
        window.scroll = window
            .scroll
            .min(count.saturating_sub(window.visible_rows.max(1)));
    }

    pub(crate) fn progression_action(&mut self, live: &LiveWorld, action: ProgressionAction) {
        if !self.skills_window.open || action.revision != live.game.progression.revision {
            return;
        }
        self.progression_tick(live);
        match action.kind {
            ProgressionActionKind::Page(page) => {
                self.skills_window.page = page;
                self.skills_window.scroll = 0;
            }
            ProgressionActionKind::Scroll { rows } => {
                self.skills_window.scroll = self
                    .skills_window
                    .scroll
                    .saturating_add_signed(rows as isize);
            }
        }
        self.clamp_skills_scroll();
    }

    pub fn progression_wheel(&mut self, live: &LiveWorld, hit: &openeq_ui::HitTarget, delta: f32) {
        if !delta.is_finite() || delta == 0. {
            return;
        }
        let Some(revision) = crate::progression_ui::progression_scroll_hit_identity(hit) else {
            return;
        };
        self.progression_action(
            live,
            ProgressionAction {
                revision,
                kind: ProgressionActionKind::Scroll {
                    rows: (-delta * 3.).clamp(-72., 72.) as i32,
                },
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_net::progression::ProgressionEvent;
    #[test]
    fn cached_values_and_local_controls_follow_revision_and_keep_commands_empty() {
        let (mut live, mut wire) = crate::live::tests::command_world(1, 1.);
        live.game.progression.confirmed = true;
        live.game.progression.level = Some(10);
        live.game.progression.apply(ProgressionEvent::SkillValue {
            wire_skill_id: 55,
            value: 80,
        });
        live.game.progression.apply(ProgressionEvent::SkillValue {
            wire_skill_id: 100,
            value: 99,
        });
        let mut interaction = Interaction::default();
        assert!(!interaction.submit("/skills", &mut live, [0.; 3]));
        assert!(interaction.skills_window.open);
        interaction.skills_window.visible_rows = 12;
        interaction.progression_tick(&live);
        let mut view = GameHudState::default();
        interaction.progression_view(&live, &mut view);
        let view = view.progression.unwrap();
        assert_eq!(view.skills.len(), 78);
        assert_eq!(view.skills[55].value, Some(80));
        assert_eq!(view.languages[0].value, Some(99));
        assert!(Arc::ptr_eq(&view.skills, &interaction.skills_window.skills));
        interaction.progression_action(
            &live,
            ProgressionAction {
                revision: view.revision,
                kind: ProgressionActionKind::Scroll { rows: 9000 },
            },
        );
        assert_eq!(interaction.skills_window.scroll, 66);
        interaction.progression_action(
            &live,
            ProgressionAction {
                revision: view.revision,
                kind: ProgressionActionKind::Page(UiProgressionPage::Languages),
            },
        );
        assert_eq!(interaction.skills_window.scroll, 0);
        live.game
            .progression
            .apply(ProgressionEvent::AlternateAdvancement {
                bar_units: 331,
                unspent_points: 12,
                allocation_percent: 101,
            });
        interaction.progression_action(
            &live,
            ProgressionAction {
                revision: live.game.progression.revision,
                kind: ProgressionActionKind::Page(UiProgressionPage::AlternateAdvancement),
            },
        );
        let mut aa = GameHudState::default();
        interaction.progression_view(&live, &mut aa);
        let aa = aa.progression.unwrap();
        assert_eq!(aa.page, UiProgressionPage::AlternateAdvancement);
        assert_eq!(aa.aa_unspent_points, Some(12));
        assert_eq!(aa.aa_experience_bar_units, Some(331));
        assert_eq!(aa.aa_allocation_percent, Some(101));
        assert_eq!(aa.aa_experience_fraction(), None);
        assert_eq!(aa.aa_allocation(), None);
        assert_eq!(interaction.skills_window.scroll, 0);
        live.game.progression.begin_zone();
        interaction.progression_action(
            &live,
            ProgressionAction {
                revision: view.revision,
                kind: ProgressionActionKind::Page(UiProgressionPage::Skills),
            },
        );
        assert_eq!(
            interaction.skills_window.page,
            UiProgressionPage::AlternateAdvancement
        );
        let mut stale = GameHudState::default();
        interaction.progression_view(&live, &mut stale);
        assert!(!stale.progression.unwrap().current);
        interaction.close_window("skills", &mut live);
        assert!(!interaction.skills_window.open);
        assert!(wire.try_recv().is_err());
    }
}
