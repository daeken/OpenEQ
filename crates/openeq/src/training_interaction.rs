//! Explicit trainer choices; opening the ordinary Skills window never trains.
use crate::{
    gameplay_ui::GameHudState,
    interaction::Interaction,
    live::LiveWorld,
    progression::{language_name, skill_name},
    training::{BlockReason, TrainingError},
    training_ui::{TrainingAction, TrainingActionKind, UiTraining, UiTrainingPage, UiTrainingRow},
};
use openeq_net::training::Selection;
use std::sync::Arc;

pub struct TrainingWindowState {
    pub open: bool,
    pub page: UiTrainingPage,
    pub scroll: usize,
    pub visible_rows: usize,
    pub selected_wire_id: Option<u32>,
    revision: u64,
    seen_live_revision: Option<u64>,
    skills: Arc<[UiTrainingRow]>,
    languages: Arc<[UiTrainingRow]>,
}

impl Default for TrainingWindowState {
    fn default() -> Self {
        Self {
            open: false,
            page: Default::default(),
            scroll: 0,
            visible_rows: 1,
            selected_wire_id: None,
            revision: 0,
            seen_live_revision: None,
            skills: Arc::from([]),
            languages: Arc::from([]),
        }
    }
}
impl TrainingWindowState {
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    fn accepts(&self, wire_id: u32) -> bool {
        match self.page {
            UiTrainingPage::Skills => wire_id < 78,
            UiTrainingPage::Languages => (100..126).contains(&wire_id),
        }
    }
    fn clamp_scroll(&mut self) {
        let count = match self.page {
            UiTrainingPage::Skills => self.skills.len(),
            UiTrainingPage::Languages => self.languages.len(),
        };
        self.scroll = self
            .scroll
            .min(count.saturating_sub(self.visible_rows.max(1)));
    }
}

fn eligibility_notice(error: TrainingError) -> &'static str {
    match error {
        TrainingError::NoPractices => "No practices remain in this session.",
        TrainingError::InsufficientEstimatedFunds => {
            "You do not have enough carried money for the estimated cost."
        }
        TrainingError::AtReportedMaximum | TrainingError::LanguageAtCap => {
            "This selection is already at its training limit."
        }
        TrainingError::CostEstimateOverflow => "The cost for this value is unsupported.",
        TrainingError::MissingValue | TrainingError::MissingProfile => {
            "Waiting for current character data."
        }
        TrainingError::Busy => "Waiting for the trainer's reply.",
        TrainingError::NotOpen | TrainingError::InvalidTrainer => {
            "Select a living trainer for your class within reach."
        }
        _ => "Training is unavailable until the character's current state is confirmed.",
    }
}

impl Interaction {
    pub fn open_training_window(&mut self, live: &mut LiveWorld) {
        let Some(id) = live.target else {
            live.game.notice("Select a trainer first.");
            return;
        };
        if self.training_window.open {
            self.window_stack.raise("training");
            return;
        }
        if !live.open_training(id) {
            return;
        }
        let next_revision = self.training_window.revision.wrapping_add(1);
        self.training_window = TrainingWindowState {
            open: true,
            revision: next_revision,
            visible_rows: self.training_window.visible_rows,
            ..Default::default()
        };
        self.training_tick(live);
        self.window_stack.raise("training");
    }

    pub(crate) fn training_tick(&mut self, live: &LiveWorld) {
        let window = &mut self.training_window;
        if window.seen_live_revision != Some(live.training_revision()) {
            let state = live.training_state();
            if window.open
                && state.active_trainer().is_none()
                && state.pending().is_none()
                && state.blocked().is_none()
            {
                window.open = false;
                window.selected_wire_id = None;
            }
            window.skills = (0..78)
                .map(|id| UiTrainingRow {
                    wire_id: id,
                    name: skill_name(&live.game.strings, id),
                    value: state.value(id),
                    reported_maximum: state.reported_maxima().map(|max| max[id as usize]),
                })
                .collect();
            window.languages = (0..26)
                .map(|id| UiTrainingRow {
                    wire_id: id + 100,
                    name: language_name(&live.game.strings, id),
                    value: state.value(id + 100),
                    reported_maximum: None,
                })
                .collect();
            window.seen_live_revision = Some(live.training_revision());
            window.changed();
        }
        window.clamp_scroll();
    }

    pub(crate) fn training_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let window = &self.training_window;
        if !window.open {
            return;
        }
        let state = live.training_state();
        let selected = window.selected_wire_id.and_then(Selection::from_wire_id);
        let available = state
            .active_trainer()
            .is_some_and(|trainer| live.training_available(trainer.id));
        let current = window.seen_live_revision == Some(live.training_revision());
        let status = if state.blocked().is_some() {
            match state.blocked().unwrap() {
                BlockReason::LevelChanged => "Your level changed. Reconnect to confirm your remaining practices.".into(),
                _ => "Training could not be confirmed. Reconnect before another purchase; nothing is automatically retried.".into(),
            }
        } else if state.pending().is_some() {
            "Waiting for the trainer's reply.".into()
        } else if !available {
            "The trainer is no longer within reach.".into()
        } else if let Some(selection) = selected
            && let Err(error) = live.training_preview(selection)
        {
            if error == TrainingError::NoPractices
                && let Some(report) = state.last_report()
            {
                format!(
                    "No practices remain. Last training cost: {} copper.",
                    report.completion.assessed_cost_copper
                )
            } else {
                eligibility_notice(error).into()
            }
        } else if let Some(report) = state.last_report() {
            format!(
                "Last training: value {}, trainer assessed {} copper.",
                report
                    .received_value
                    .map_or_else(|| "unconfirmed".into(), |v| v.to_string()),
                report.completion.assessed_cost_copper
            )
        } else {
            "Select a skill or language to practice.".into()
        };
        let estimates = state.estimate();
        view.training = Some(UiTraining {
            revision: window.revision,
            open: true,
            trainer_name: state
                .active_trainer()
                .map_or_else(|| "Trainer".into(), |t| t.clean_name.clone()),
            status,
            practices: estimates.map(|e| e.training_points),
            carried_copper: estimates.map(|e| e.carried_copper),
            skills: window.skills.clone(),
            languages: window.languages.clone(),
            page: window.page,
            scroll: window.scroll,
            selected_wire_id: window.selected_wire_id,
            pending: state.pending().is_some(),
            train_enabled: current
                && available
                && selected.is_some_and(|s| live.training_preview(s).is_ok()),
        });
    }

    pub fn training_action(&mut self, live: &mut LiveWorld, action: TrainingAction) {
        self.training_tick(live);
        if !self.training_window.open || action.revision != self.training_window.revision {
            return;
        }
        match action.kind {
            TrainingActionKind::Page(page) => {
                self.training_window.page = page;
                self.training_window.scroll = 0;
                self.training_window.selected_wire_id = None;
            }
            TrainingActionKind::Select { wire_id } => {
                if !self.training_window.accepts(wire_id) {
                    return;
                }
                self.training_window.selected_wire_id = Some(wire_id);
            }
            TrainingActionKind::Train { wire_id } => {
                if self.training_window.selected_wire_id != Some(wire_id)
                    || !self.training_window.accepts(wire_id)
                {
                    return;
                }
                if let Some(selection) = Selection::from_wire_id(wire_id) {
                    live.train_selection(selection);
                }
            }
            TrainingActionKind::Scroll { rows } => {
                self.training_window.scroll = self
                    .training_window
                    .scroll
                    .saturating_add_signed(rows as isize);
            }
            TrainingActionKind::Close => {
                live.close_training();
                self.training_window.open = false;
                self.training_window.selected_wire_id = None;
            }
        }
        self.training_window.changed();
        self.training_window.clamp_scroll();
    }

    pub(crate) fn close_training_window(&mut self, live: &mut LiveWorld) {
        self.training_tick(live);
        self.training_action(
            live,
            TrainingAction {
                revision: self.training_window.revision,
                kind: TrainingActionKind::Close,
            },
        );
    }

    pub fn training_wheel(&mut self, live: &mut LiveWorld, hit: &openeq_ui::HitTarget, delta: f32) {
        if !delta.is_finite() || delta == 0. {
            return;
        }
        if let Some(revision) = crate::training_ui::training_scroll_hit_identity(hit) {
            self.training_action(
                live,
                TrainingAction {
                    revision,
                    kind: TrainingActionKind::Scroll {
                        rows: (-delta * 3.).clamp(-72., 72.) as i32,
                    },
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::training::tests::Fixture;
    use openeq_net::{gameplay::GameplayEvent, training::TrainingEvent, zone::ZoneEvent};

    fn view(ui: &Interaction, fixture: &Fixture) -> UiTraining {
        ui.view(&fixture.live).training.unwrap()
    }
    fn act(ui: &mut Interaction, fixture: &mut Fixture, kind: TrainingActionKind) {
        let revision = view(ui, fixture).revision;
        ui.training_action(&mut fixture.live, TrainingAction { revision, kind });
    }
    fn open(command: &str) -> (Fixture, Interaction) {
        let mut fixture = Fixture::new();
        let mut ui = Interaction::default();
        fixture.live.target = Some(2);
        assert!(!ui.submit(command, &mut fixture.live, [0.; 3]));
        assert!(view(&ui, &fixture).pending);
        let request = fixture.queued();
        fixture.send(&request);
        fixture.event(ZoneEvent::Gameplay(GameplayEvent::Training(
            TrainingEvent::Opened {
                trainer_id: 2,
                player_id: 1,
                skills: Box::new([200; 100]),
            },
        )));
        ui.tick(&mut fixture.live);
        assert!(!view(&ui, &fixture).pending);
        assert!(fixture.queue_empty());
        (fixture, ui)
    }

    #[test]
    fn explicit_train_and_service_openers_work_while_skills_remains_local() {
        for command in ["/train", "/use"] {
            let (mut fixture, mut ui) = open(command);
            assert_eq!(view(&ui, &fixture).trainer_name, "Warlord Welorf");
            assert!(!ui.submit(command, &mut fixture.live, [0.; 3]));
            assert!(
                fixture.queue_empty(),
                "reopening the local panel must not repeat trainer open"
            );
        }
        let mut fixture = Fixture::new();
        let mut ui = Interaction::default();
        fixture.live.target = Some(2);
        ui.submit("/skills", &mut fixture.live, [0.; 3]);
        assert!(ui.skills_window.open && !ui.training_window.open);
        assert!(fixture.queue_empty());
        assert_eq!(
            crate::hotbuttons::parse_hotbutton_command("/train").unwrap(),
            crate::chat::Action::Train
        );
    }

    #[test]
    fn stale_page_selection_and_double_click_cannot_train_the_wrong_choice_or_twice() {
        let (mut fixture, mut ui) = open("/train");
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Select { wire_id: 0 },
        );
        let old = TrainingAction {
            revision: view(&ui, &fixture).revision,
            kind: TrainingActionKind::Train { wire_id: 0 },
        };
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Page(UiTrainingPage::Languages),
        );
        ui.training_action(&mut fixture.live, old.clone());
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Train { wire_id: 0 },
        );
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Select { wire_id: 126 },
        );
        assert!(view(&ui, &fixture).selected_wire_id.is_none());
        assert!(fixture.queue_empty());
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Page(UiTrainingPage::Skills),
        );
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Select { wire_id: 0 },
        );
        let before = view(&ui, &fixture);
        assert!(before.train_enabled);
        let train = TrainingAction {
            revision: before.revision,
            kind: TrainingActionKind::Train { wire_id: 0 },
        };
        ui.training_action(&mut fixture.live, train.clone());
        ui.training_action(&mut fixture.live, train.clone());
        let request = fixture.queued();
        assert!(fixture.queue_empty());
        assert!(!view(&ui, &fixture).train_enabled);
        fixture.send(&request);
        fixture.receipt(56, 911);
        ui.tick(&mut fixture.live);
        let after = view(&ui, &fixture);
        assert_eq!(after.skills[0].value, Some(56));
        assert_eq!(after.practices, Some(1));
        assert_eq!(after.carried_copper, Some(99_089));
        assert!(after.status.contains("911 copper"));
        assert_eq!(
            fixture.live.game.profile.as_ref().unwrap().training_points,
            2
        );
        ui.training_action(&mut fixture.live, train);
        ui.training_action(&mut fixture.live, old);
        assert!(fixture.queue_empty());
        assert_eq!(
            fixture
                .live
                .training_state()
                .estimate()
                .unwrap()
                .matched_purchases,
            1
        );
    }

    #[test]
    fn scroll_resize_and_close_are_local_and_old_hits_cannot_reopen_the_session() {
        let (mut fixture, mut ui) = open("/train");
        ui.training_window.visible_rows = 4;
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Scroll { rows: i32::MAX },
        );
        assert_eq!(view(&ui, &fixture).scroll, 74);
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Page(UiTrainingPage::Languages),
        );
        act(
            &mut ui,
            &mut fixture,
            TrainingActionKind::Scroll { rows: i32::MAX },
        );
        assert_eq!(view(&ui, &fixture).scroll, 22);
        ui.training_window.visible_rows = 12;
        ui.tick(&mut fixture.live);
        assert_eq!(view(&ui, &fixture).scroll, 14);
        assert!(fixture.queue_empty());
        let old = TrainingAction {
            revision: view(&ui, &fixture).revision,
            kind: TrainingActionKind::Select { wire_id: 100 },
        };
        ui.close_window("training", &mut fixture.live);
        assert!(!ui.training_window.open);
        assert!(fixture.live.training_state().active_trainer().is_none());
        ui.training_action(&mut fixture.live, old);
        assert!(ui.view(&fixture.live).training.is_none());
    }

    #[test]
    fn final_practice_keeps_its_assessed_cost_visible() {
        let (mut fixture, mut ui) = open("/train");
        for (value, cost) in [(56, 911), (57, 973)] {
            act(
                &mut ui,
                &mut fixture,
                TrainingActionKind::Select { wire_id: 0 },
            );
            act(
                &mut ui,
                &mut fixture,
                TrainingActionKind::Train { wire_id: 0 },
            );
            let request = fixture.queued();
            fixture.send(&request);
            fixture.receipt(value, cost);
            ui.tick(&mut fixture.live);
        }
        let after = view(&ui, &fixture);
        assert_eq!(after.practices, Some(0));
        assert!(!after.train_enabled);
        assert!(after.status.contains("No practices remain"));
        assert!(after.status.contains("973 copper"));
    }
}
