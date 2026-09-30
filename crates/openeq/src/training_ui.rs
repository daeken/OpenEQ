//! Trainer presentation using original TrainWindow art and session-stamped hits.
//! Prices are deliberately absent: the stock server assesses cost after purchase.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, WHITE, position};
use openeq_ui::{HitTarget, Rect, UiBindings};
use std::sync::Arc;

pub const TRAINING_VISIBLE_ROWS: usize = 12;
const WINDOW_HEIGHT: f32 = 480.;
const LIST_TOP: f32 = 100.;
const HEADER_HEIGHT: f32 = 24.;
const ROW_HEIGHT: f32 = 20.;
const FOOTER_HEIGHT: f32 = 112.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiTrainingPage {
    #[default]
    Skills,
    Languages,
}

#[derive(Clone, Debug, Default)]
pub struct UiTrainingRow {
    /// Skill 0–77 or trainable language 100–125, never a visible row index.
    pub wire_id: u32,
    pub name: String,
    /// Current base skill value or language value from received player data.
    pub value: Option<u32>,
    /// Display only an actual trainer-reported maximum; do not infer a quote,
    /// rank, level cap or language maximum from a skill-bank entry.
    pub reported_maximum: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct UiTraining {
    /// Runtime changes this whenever a displayed choice can become stale.
    pub revision: u64,
    pub open: bool,
    pub trainer_name: String,
    /// Includes pending/blocked reasons and the last server-assessed cost.
    pub status: String,
    /// Session estimates, not fresh authoritative post-purchase balances.
    pub practices: Option<u32>,
    pub carried_copper: Option<u64>,
    pub skills: Arc<[UiTrainingRow]>,
    pub languages: Arc<[UiTrainingRow]>,
    pub page: UiTrainingPage,
    pub scroll: usize,
    pub selected_wire_id: Option<u32>,
    pub pending: bool,
    /// The runtime must recheck eligibility and revision when dispatching.
    pub train_enabled: bool,
}

impl UiTraining {
    fn rows(&self) -> impl Iterator<Item = &UiTrainingRow> {
        let (rows, count, min, max) = match self.page {
            UiTrainingPage::Skills => (&self.skills, 78, 0, 77),
            UiTrainingPage::Languages => (&self.languages, 26, 100, 125),
        };
        rows.iter()
            .take(count)
            .filter(move |row| (min..=max).contains(&row.wire_id))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrainingAction {
    pub revision: u64,
    pub kind: TrainingActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingActionKind {
    Page(UiTrainingPage),
    Select { wire_id: u32 },
    Train { wire_id: u32 },
    Scroll { rows: i32 },
    Close,
}

fn wire_id(text: &str) -> Option<u32> {
    text.parse()
        .ok()
        .filter(|id| matches!(id, 0..=77 | 100..=125))
}

fn training_hit(hit: &HitTarget) -> Option<(u64, &str)> {
    if !hit.enabled || hit.window_id.as_deref() != Some("training") {
        return None;
    }
    let (revision, suffix) = hit.item.strip_prefix("training:")?.split_once(':')?;
    Some((revision.parse().ok()?, suffix))
}

impl TrainingAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        let (revision, suffix) = training_hit(hit)?;
        let kind = match suffix {
            "skills" => TrainingActionKind::Page(UiTrainingPage::Skills),
            "languages" => TrainingActionKind::Page(UiTrainingPage::Languages),
            "close" => TrainingActionKind::Close,
            "scroll:-1" => TrainingActionKind::Scroll { rows: -1 },
            "scroll:1" => TrainingActionKind::Scroll { rows: 1 },
            _ => {
                let (action, id) = suffix.split_once(':')?;
                match action {
                    "select" => TrainingActionKind::Select {
                        wire_id: wire_id(id)?,
                    },
                    "train" => TrainingActionKind::Train {
                        wire_id: wire_id(id)?,
                    },
                    _ => return None,
                }
            }
        };
        Some(Self { revision, kind })
    }
}

/// Wheel events retain the identity of the list actually under the pointer.
pub fn training_scroll_hit_identity(hit: &HitTarget) -> Option<u64> {
    let (revision, suffix) = training_hit(hit)?;
    if hit.kind != "TrainingList" {
        return None;
    }
    (matches!(suffix, "list" | "scroll:-1" | "scroll:1")
        || suffix.strip_prefix("select:").and_then(wire_id).is_some())
    .then_some(revision)
}

/// Shared with the runtime so wheel and page bounds match the visible list.
pub fn training_visible_rows(viewport_height: u32) -> usize {
    let available =
        (viewport_height as f32).min(WINDOW_HEIGHT) - LIST_TOP - HEADER_HEIGHT - FOOTER_HEIGHT;
    ((available.max(0.) / ROW_HEIGHT) as usize).min(TRAINING_VISIBLE_ROWS)
}

fn money_text(copper: Option<u64>) -> String {
    copper.map_or_else(
        || "unavailable".into(),
        |copper| {
            format!(
                "{}p {}g {}s {}c",
                copper / 1000,
                copper / 100 % 10,
                copper / 10 % 10,
                copper % 10
            )
        },
    )
}

impl Painter<'_> {
    pub(crate) fn training(&mut self, state: &GameHudState, training: &UiTraining) {
        if !training.open {
            return;
        }
        let rect = position(
            state,
            "training",
            Rect::new(self.screen.width * 0.5 - 290., 24., 580., WINDOW_HEIGHT),
            self.screen,
        );
        let title = if training.trainer_name.is_empty() {
            "Training".to_owned()
        } else {
            format!("Training — {}", training.trainer_name)
        };
        self.shell("TrainWindow", "training", rect, &title, true);
        // The original close-box art remains, but its action belongs to the
        // displayed trainer session just like Done and Train below.
        if let Some(hit) = self.frame.hit_targets.last_mut()
            && hit.item == "game:close:training"
        {
            hit.item = format!("training:{}:close", training.revision);
        }
        self.text(
            Rect::new(rect.x + 12., rect.y + 28., (rect.width - 24.).max(0.), 32.),
            if training.status.is_empty() {
                "Waiting for trainer information"
            } else {
                &training.status
            },
            MUTED,
            true,
        );
        let tab_width = ((rect.width - 28.) / 2.).max(0.);
        for (index, (page, suffix, label)) in [
            (UiTrainingPage::Skills, "skills", "Skills"),
            (UiTrainingPage::Languages, "languages", "Languages"),
        ]
        .into_iter()
        .enumerate()
        {
            self.button(
                "TW_DoneButton",
                &format!("training:{}:{suffix}", training.revision),
                Rect::new(
                    rect.x + 12. + index as f32 * (tab_width + 4.),
                    rect.y + 66.,
                    tab_width,
                    24.,
                ),
                label,
                training.page == page,
            );
        }
        let visible = training_visible_rows(self.screen.height as u32);
        let list = Rect::new(
            rect.x + 10.,
            rect.y + LIST_TOP,
            (rect.width - 20.).max(0.),
            HEADER_HEIGHT + visible as f32 * ROW_HEIGHT,
        );
        let mut bindings = UiBindings::default();
        bindings.widget_mut("TRNW_SkillList").rect = Some(list);
        self.widget("TRNW_SkillList", &bindings);
        self.hit(
            format!("training:{}:list", training.revision),
            "TrainingList",
            list,
            None,
        );
        let cells = (list.width - 25.).max(0.);
        let name_width = cells * 0.48;
        let value_width = cells * 0.22;
        let max_width = cells - name_width - value_width;
        for (x, width, label) in [
            (
                list.x + 5.,
                name_width,
                if training.page == UiTrainingPage::Skills {
                    "Skill"
                } else {
                    "Language"
                },
            ),
            (
                list.x + name_width + 5.,
                value_width,
                if training.page == UiTrainingPage::Skills {
                    "Base value"
                } else {
                    "Value"
                },
            ),
            (
                list.x + name_width + value_width + 5.,
                max_width,
                "Reported max",
            ),
        ] {
            self.text(
                Rect::new(x, list.y + 3., (width - 4.).max(0.), 18.),
                label,
                GOLD,
                false,
            );
        }
        let count = training.rows().count();
        let max_scroll = count.saturating_sub(visible.max(1));
        let start = training.scroll.min(max_scroll);
        let selected = training
            .rows()
            .find(|row| Some(row.wire_id) == training.selected_wire_id);
        for (index, row) in training.rows().skip(start).take(visible).enumerate() {
            let bounds = Rect::new(
                list.x + 3.,
                list.y + HEADER_HEIGHT + index as f32 * ROW_HEIGHT,
                cells,
                ROW_HEIGHT,
            );
            if Some(row.wire_id) == training.selected_wire_id {
                self.fill(bounds, [78, 66, 38, 225]);
            } else if index % 2 == 1 {
                self.fill(bounds, [25, 28, 31, 170]);
            }
            let value = row
                .value
                .map_or_else(|| "—".into(), |value| value.to_string());
            let maximum = row
                .reported_maximum
                .map_or_else(|| "—".into(), |value| value.to_string());
            for (x, width, text) in [
                (bounds.x + 2., name_width, row.name.as_str()),
                (bounds.x + name_width + 2., value_width, value.as_str()),
                (
                    bounds.x + name_width + value_width + 2.,
                    max_width,
                    maximum.as_str(),
                ),
            ] {
                self.text(
                    Rect::new(x, bounds.y + 1., (width - 4.).max(0.), 18.),
                    text,
                    WHITE,
                    false,
                );
            }
            self.hit(
                format!("training:{}:select:{}", training.revision, row.wire_id),
                "TrainingList",
                bounds,
                Some(format!(
                    "{} · value {} · reported maximum {}",
                    row.name, value, maximum
                )),
            );
        }
        if count == 0 && visible > 0 {
            self.text(
                Rect::new(
                    list.x + 8.,
                    list.y + HEADER_HEIGHT + 8.,
                    (list.width - 35.).max(0.),
                    40.,
                ),
                "No trainer values available.",
                MUTED,
                true,
            );
        }
        for (direction, label, y, enabled) in [
            (-1, "↑", list.y + 2., start > 0 && visible > 0),
            (
                1,
                "↓",
                list.bottom() - 22.,
                start < max_scroll && visible > 0,
            ),
        ] {
            self.button_enabled(
                "TW_DoneButton",
                &format!("training:{}:scroll:{direction}", training.revision),
                Rect::new(list.right() - 20., y, 18., 20.),
                label,
                false,
                enabled,
            );
            if let Some(hit) = self.frame.hit_targets.last_mut() {
                hit.kind = "TrainingList".into();
            }
        }
        if max_scroll > 0 && visible > 0 {
            let track = (list.height - 48.).max(0.);
            let thumb = (track * visible as f32 / count as f32).max(10.).min(track);
            self.fill(
                Rect::new(
                    list.right() - 14.,
                    list.y + 24. + (track - thumb) * start as f32 / max_scroll as f32,
                    6.,
                    thumb,
                ),
                [140, 130, 99, 255],
            );
        }
        for (line, text, color) in [
            (
                0.,
                selected.map_or_else(
                    || "Select a skill or language".into(),
                    |row| format!("Selected: {}", row.name),
                ),
                WHITE,
            ),
            (
                20.,
                format!(
                    "Practices (session estimate): {}",
                    training
                        .practices
                        .map_or_else(|| "unavailable".into(), |value| value.to_string())
                ),
                MUTED,
            ),
            (
                40.,
                format!(
                    "Carried (session estimate): {}",
                    money_text(training.carried_copper)
                ),
                MUTED,
            ),
            (60., "Price unavailable before training.".into(), MUTED),
        ] {
            self.text(
                Rect::new(
                    rect.x + 12.,
                    list.bottom() + 4. + line,
                    (rect.width - 24.).max(0.),
                    18.,
                ),
                text,
                color,
                false,
            );
        }
        let train_enabled = training.train_enabled
            && !training.pending
            && selected.is_some_and(|row| row.value.is_some());
        self.button_enabled(
            "TRNW_TrainButton",
            &selected.map_or_else(
                || format!("training:{}:unselected", training.revision),
                |row| format!("training:{}:train:{}", training.revision, row.wire_id),
            ),
            Rect::new(
                rect.x + 12.,
                rect.bottom() - 29.,
                ((rect.width - 28.) * 0.65).max(0.),
                22.,
            ),
            if training.pending {
                "Training…"
            } else {
                "Train"
            },
            false,
            train_enabled,
        );
        let done_width = ((rect.width - 28.) * 0.35).max(0.);
        self.button(
            "TW_DoneButton",
            &format!("training:{}:close", training.revision),
            Rect::new(
                rect.right() - 12. - done_width,
                rect.bottom() - 29.,
                done_width,
                22.,
            ),
            "Done",
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gameplay_ui::{UiAction, WindowStack, valid_window_id},
        hud::{Hud, HudState},
    };
    use openeq_ui::{DrawCommand, UiDocument, UiFrame};

    fn fixture() -> UiTraining {
        UiTraining {
            revision: 91,
            open: true,
            trainer_name: "Warrior Guildmaster".into(),
            status: "Trainer information received".into(),
            practices: Some(2),
            carried_copper: Some(100_000),
            skills: (0..78)
                .map(|wire_id| UiTrainingRow {
                    wire_id,
                    name: format!("Skill {wire_id}"),
                    value: Some(20 + wire_id),
                    reported_maximum: Some(55),
                })
                .collect::<Vec<_>>()
                .into(),
            languages: (100..126)
                .map(|wire_id| UiTrainingRow {
                    wire_id,
                    name: format!("Language {}", wire_id - 100),
                    value: Some(wire_id - 100),
                    reported_maximum: None,
                })
                .collect::<Vec<_>>()
                .into(),
            selected_wire_id: Some(0),
            train_enabled: true,
            ..Default::default()
        }
    }

    fn minimal_hud() -> Hud {
        Hud {
            ui: UiDocument::from_xml(r#"<XML>
                <Screen item="TrainWindow"><Pieces>TRNW_PracticeCount</Pieces><Pieces>TRNW_CoinCount0</Pieces><Pieces>TRNW_CoinCount1</Pieces><Pieces>TRNW_CoinCount2</Pieces><Pieces>TRNW_CoinCount3</Pieces><Pieces>TRNW_ExpGauge</Pieces></Screen>
                <Label item="TRNW_PracticeCount"><Text>255</Text></Label>
                <Label item="TRNW_CoinCount0"><Text>10000</Text></Label>
                <Label item="TRNW_CoinCount1"><Text>6645</Text></Label>
                <Label item="TRNW_CoinCount2"><Text>4444</Text></Label>
                <Label item="TRNW_CoinCount3"><Text>5555</Text></Label>
                <Gauge item="TRNW_ExpGauge"><Text>Experience</Text></Gauge>
                <Listbox item="TRNW_SkillList"/><Button item="TRNW_TrainButton"/><Button item="TW_DoneButton"/>
                </XML>"#).unwrap(),
            initial_bindings: UiBindings::default(),
        }
    }

    fn hit(suffix: &str) -> HitTarget {
        HitTarget {
            item: format!("training:91:{suffix}"),
            screen_id: "fixture".into(),
            window_id: Some("training".into()),
            kind: "TrainingList".into(),
            rect: Rect::new(0., 0., 10., 10.),
            enabled: true,
            tooltip: None,
        }
    }

    fn frame(training: UiTraining, viewport: [u32; 2]) -> UiFrame {
        minimal_hud().gameplay_frame(
            viewport,
            &HudState::default(),
            &GameHudState {
                training: Some(training),
                ..Default::default()
            },
        )
    }

    #[test]
    fn every_training_action_is_window_revision_and_wire_id_scoped() {
        for (suffix, kind) in [
            ("skills", TrainingActionKind::Page(UiTrainingPage::Skills)),
            (
                "languages",
                TrainingActionKind::Page(UiTrainingPage::Languages),
            ),
            ("select:0", TrainingActionKind::Select { wire_id: 0 }),
            ("select:77", TrainingActionKind::Select { wire_id: 77 }),
            ("select:100", TrainingActionKind::Select { wire_id: 100 }),
            ("select:125", TrainingActionKind::Select { wire_id: 125 }),
            ("train:125", TrainingActionKind::Train { wire_id: 125 }),
            ("scroll:-1", TrainingActionKind::Scroll { rows: -1 }),
            ("scroll:1", TrainingActionKind::Scroll { rows: 1 }),
            ("close", TrainingActionKind::Close),
        ] {
            assert_eq!(
                UiAction::from_hit(&hit(suffix)),
                Some(UiAction::Training(TrainingAction { revision: 91, kind }))
            );
        }
        for suffix in [
            "train",
            "train:78",
            "select:99",
            "select:126",
            "select:127",
            "select:-1",
            "select:4294967296",
            "train:1:2",
            "list",
            "scroll:0",
            "scroll:99",
        ] {
            assert!(TrainingAction::from_hit(&hit(suffix)).is_none(), "{suffix}");
        }
        let mut covered = hit("train:0");
        covered.window_id = Some("merchant".into());
        assert!(TrainingAction::from_hit(&covered).is_none());
        covered.window_id = Some("training".into());
        covered.enabled = false;
        assert!(TrainingAction::from_hit(&covered).is_none());
        covered.enabled = true;
        covered.item = "training:stale:train:0".into();
        assert!(TrainingAction::from_hit(&covered).is_none());
        for suffix in ["list", "select:0", "select:125", "scroll:1"] {
            assert_eq!(training_scroll_hit_identity(&hit(suffix)), Some(91));
        }
        for suffix in ["select:126", "train:0", "close"] {
            assert!(training_scroll_hit_identity(&hit(suffix)).is_none());
        }
    }

    #[test]
    fn purchase_requires_a_current_selected_row_and_explicit_adapter_permission() {
        for (selected, pending, allowed, page, enabled) in [
            (Some(0), false, true, UiTrainingPage::Skills, true),
            (Some(0), true, true, UiTrainingPage::Skills, false),
            (Some(0), false, false, UiTrainingPage::Skills, false),
            (None, false, true, UiTrainingPage::Skills, false),
            (Some(78), false, true, UiTrainingPage::Skills, false),
            (Some(0), false, true, UiTrainingPage::Languages, false),
            (Some(125), false, true, UiTrainingPage::Languages, true),
            (Some(126), false, true, UiTrainingPage::Languages, false),
        ] {
            let mut view = fixture();
            view.selected_wire_id = selected;
            view.pending = pending;
            view.train_enabled = allowed;
            view.page = page;
            let frame = frame(view, [800, 600]);
            let button = frame
                .hit_targets
                .iter()
                .find(|h| {
                    h.item.starts_with("training:91:train:") || h.item == "training:91:unselected"
                })
                .unwrap();
            assert_eq!(button.enabled, enabled);
            assert_eq!(
                matches!(
                    UiAction::from_hit(button),
                    Some(UiAction::Training(TrainingAction {
                        kind: TrainingActionKind::Train { .. },
                        ..
                    }))
                ),
                enabled
            );
            assert!(
                frame
                    .hit_targets
                    .iter()
                    .filter(|h| h.item == "training:91:close")
                    .all(|h| h.enabled)
            );
        }
        let mut view = fixture();
        Arc::make_mut(&mut view.skills)[0].value = None;
        let frame = frame(view, [800, 600]);
        assert!(
            !frame
                .hit_targets
                .iter()
                .find(|h| h.item == "training:91:train:0")
                .unwrap()
                .enabled
        );
    }

    #[test]
    fn rows_scroll_to_wire_bounds_and_never_expose_xml_sample_values() {
        for (page, last) in [
            (UiTrainingPage::Skills, 77),
            (UiTrainingPage::Languages, 125),
        ] {
            for viewport in [[900, 650], [640, 360], [350, 400], [320, 240]] {
                let mut view = fixture();
                view.page = page;
                view.scroll = usize::MAX;
                let frame = frame(view, viewport);
                let rows: Vec<_> = frame
                    .hit_targets
                    .iter()
                    .filter(|h| h.item.starts_with("training:91:select:"))
                    .collect();
                assert_eq!(rows.len(), training_visible_rows(viewport[1]));
                if !rows.is_empty() {
                    assert_eq!(
                        rows.last().unwrap().item,
                        format!("training:91:select:{last}")
                    );
                }
                assert!(rows.iter().all(|hit| hit.rect.right() <= viewport[0] as f32
                    && hit.rect.bottom() <= viewport[1] as f32
                    && hit.window_id.as_deref() == Some("training")));
                assert!(
                    !frame
                        .hit_targets
                        .iter()
                        .find(|h| h.item == "training:91:scroll:1")
                        .unwrap()
                        .enabled
                );
                assert!(!frame.commands.iter().any(|cmd| matches!(cmd, DrawCommand::Text { text, .. } if ["255", "10000", "6645", "4444", "5555", "Rank", "Experience"].contains(&text.as_str()))));
            }
        }
        let unknown = frame(
            UiTraining {
                open: true,
                ..Default::default()
            },
            [800, 600],
        );
        for label in [
            "Practices (session estimate): unavailable",
            "Carried (session estimate): unavailable",
            "Price unavailable before training.",
        ] {
            assert!(
                unknown
                    .commands
                    .iter()
                    .any(|cmd| matches!(cmd, DrawCommand::Text { text, .. } if text == label))
            );
        }
        assert_eq!(money_text(Some(100_023)), "100p 0g 2s 3c");
        assert_eq!(money_text(Some(0)), "0p 0g 0s 0c");
    }

    #[test]
    fn trainer_close_drag_and_stack_keep_the_displayed_session_identity() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            training: Some(fixture()),
            inventory_open: true,
            ..Default::default()
        };
        state.window_positions.insert("training".into(), [40., 50.]);
        state
            .window_positions
            .insert("inventory".into(), [40., 50.]);
        let mut stack = WindowStack::default();
        let frame = hud.gameplay_frame_with_windows(
            [900, 650],
            &HudState::default(),
            &state,
            vec![],
            &mut stack,
        );
        assert!(valid_window_id("training"));
        assert_eq!(
            frame
                .hit_targets
                .iter()
                .filter(|h| h.item == "training:91:close")
                .count(),
            2
        );
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|h| h.item == "game:close:training")
        );
        let drag = frame
            .hit_targets
            .iter()
            .find(|h| h.item == "game:drag:training")
            .unwrap();
        assert_eq!(
            UiAction::from_hit(drag),
            Some(UiAction::BeginWindowDrag("training".into()))
        );
        let point = [80., 230.];
        assert_eq!(
            frame.hit_test(point).unwrap().window_id.as_deref(),
            Some("training")
        );
        stack.raise("inventory");
        let frame = hud.gameplay_frame_with_windows(
            [900, 650],
            &HudState::default(),
            &state,
            vec![],
            &mut stack,
        );
        assert_eq!(
            frame.hit_test(point).unwrap().window_id.as_deref(),
            Some("inventory")
        );
        state.training.as_mut().unwrap().open = false;
        let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|h| h.window_id.as_deref() == Some("training"))
        );
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR; no network or audio"]
    fn original_trainer_skin_at_normal_retina_and_compact_sizes() {
        let base = openeq_assets::loader::default_client_dir().unwrap();
        let hud = Hud::load(&base).unwrap();
        let names = crate::game::StringTable::load(&base);
        let resources = HudState {
            character: "Adventurer".into(),
            hp: 1.,
            ..Default::default()
        };
        for scale in [1, 2] {
            let mut renderer =
                openeq_render::Renderer::new_headless(800 * scale, 600 * scale).unwrap();
            for mode in [
                "skills",
                "languages-bottom",
                "pending",
                "blocked",
                "unknown",
                "compact",
                "narrow",
            ] {
                let viewport = match mode {
                    "compact" => [640, 360],
                    "narrow" => [350, 400],
                    _ => [800, 600],
                };
                renderer.resize(viewport[0] * scale, viewport[1] * scale);
                let mut training = fixture();
                for row in Arc::make_mut(&mut training.skills) {
                    row.name = crate::progression::skill_name(&names, row.wire_id);
                }
                for row in Arc::make_mut(&mut training.languages) {
                    row.name = crate::progression::language_name(&names, row.wire_id - 100);
                }
                if mode == "languages-bottom" {
                    training.page = UiTrainingPage::Languages;
                    training.scroll = usize::MAX;
                    training.selected_wire_id = Some(125);
                }
                if mode == "pending" {
                    training.pending = true;
                    training.status = "Training requested. Waiting for the server's result.".into();
                }
                if mode == "blocked" {
                    training.train_enabled = false;
                    training.practices = None;
                    training.carried_copper = None;
                    training.status =
                        "Balance is uncertain. Reconnect before training again.".into();
                }
                if mode == "narrow" {
                    training.train_enabled = false;
                    training.status = "Training could not be confirmed. Reconnect before another purchase; nothing is automatically retried.".into();
                }
                if mode == "unknown" {
                    training = UiTraining {
                        open: true,
                        revision: 92,
                        ..Default::default()
                    };
                }
                let mut state = GameHudState {
                    training: Some(training),
                    ..Default::default()
                };
                state.window_positions.insert("training".into(), [30., 90.]);
                let frame = hud.gameplay_frame(viewport, &resources, &state);
                assert!(frame.warnings.is_empty(), "{mode}: {:?}", frame.warnings);
                assert!(frame.commands.iter().any(
                    |cmd| matches!(cmd, DrawCommand::Image { texture, .. } if texture.exists())
                ));
                renderer.set_ui_scaled(&frame, scale as f32);
                renderer.render_ui();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                assert!(
                    pixels
                        .chunks_exact(4)
                        .any(|pixel| pixel[0] > 80 && pixel[1] > 80)
                );
                if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        std::path::PathBuf::from(directory)
                            .join(format!("training-{mode}-{scale}x.png")),
                        &pixels,
                        width,
                        height,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        }
    }
}
