//! Receive-only skills, languages and normal/AA experience using the original skin.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, WHITE, position};
use openeq_ui::{HitTarget, Rect, UiBindings};
use std::sync::Arc;

pub const PROGRESSION_VISIBLE_ROWS: usize = 12;
const WINDOW_HEIGHT: f32 = 430.;
const LIST_TOP: f32 = 104.;
const HEADER_HEIGHT: f32 = 24.;
const ROW_HEIGHT: f32 = 20.;
const FOOTER_HEIGHT: f32 = 54.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiProgressionPage {
    #[default]
    Skills,
    Languages,
    AlternateAdvancement,
}

#[derive(Clone, Debug, Default)]
pub struct UiProgressionRow {
    /// Numeric skill/language ID within this page, never a displayed row index.
    pub id: u32,
    pub name: String,
    pub value: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct UiProgression {
    /// Changes across sessions and snapshot replacement; adapters recheck hits.
    pub revision: u64,
    pub status: String,
    pub current: bool,
    pub level: Option<u32>,
    pub experience_bar_units: Option<u32>,
    pub aa_experience_bar_units: Option<u32>,
    pub aa_unspent_points: Option<u32>,
    pub aa_allocation_percent: Option<u8>,
    /// Adapter-supplied names and base values, sorted by numeric ID.
    pub skills: Arc<[UiProgressionRow]>,
    pub languages: Arc<[UiProgressionRow]>,
    pub open: bool,
    pub page: UiProgressionPage,
    pub scroll: usize,
}

impl UiProgression {
    pub fn experience_fraction(&self) -> Option<f32> {
        self.experience_bar_units
            .filter(|units| self.current && *units <= 330)
            .map(|units| units as f32 / 330.)
    }

    pub fn aa_experience_fraction(&self) -> Option<f32> {
        self.aa_experience_bar_units
            .filter(|units| {
                self.current && *units <= openeq_net::progression::AA_EXPERIENCE_BAR_UNITS
            })
            .map(|units| units as f32 / openeq_net::progression::AA_EXPERIENCE_BAR_UNITS as f32)
    }

    pub fn aa_allocation(&self) -> Option<u8> {
        self.aa_allocation_percent
            .filter(|percent| self.current && *percent <= 100)
    }

    fn rows(&self) -> impl Iterator<Item = &UiProgressionRow> {
        let (rows, bound): (&[UiProgressionRow], usize) = match self.page {
            UiProgressionPage::Skills => (&self.skills, 78),
            UiProgressionPage::Languages => (&self.languages, 28),
            UiProgressionPage::AlternateAdvancement => (&[], 0),
        };
        rows.iter()
            .take(bound)
            .filter(move |row| row.id < bound as u32)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressionAction {
    pub revision: u64,
    pub kind: ProgressionActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressionActionKind {
    Page(UiProgressionPage),
    Scroll { rows: i32 },
}

impl ProgressionAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        let (revision, action) = progression_hit(hit)?;
        let kind = match action {
            "skills" => ProgressionActionKind::Page(UiProgressionPage::Skills),
            "languages" => ProgressionActionKind::Page(UiProgressionPage::Languages),
            "aa" => ProgressionActionKind::Page(UiProgressionPage::AlternateAdvancement),
            "scroll:-1" => ProgressionActionKind::Scroll { rows: -1 },
            "scroll:1" => ProgressionActionKind::Scroll { rows: 1 },
            _ => return None,
        };
        Some(Self { revision, kind })
    }
}

fn progression_hit(hit: &HitTarget) -> Option<(u64, &str)> {
    if !hit.enabled || hit.window_id.as_deref() != Some("skills") {
        return None;
    }
    let (revision, action) = hit.item.strip_prefix("progression:")?.split_once(':')?;
    Some((revision.parse().ok()?, action))
}

/// Wheel routing keeps the generation of the list that was actually displayed.
pub fn progression_scroll_hit_identity(hit: &HitTarget) -> Option<u64> {
    let (revision, action) = progression_hit(hit)?;
    if hit.kind != "ProgressionList" {
        return None;
    }
    (matches!(action, "list" | "scroll:-1" | "scroll:1")
        || action
            .strip_prefix("row:")
            .is_some_and(|id| id.parse::<u32>().is_ok_and(|id| id < 78)))
    .then_some(revision)
}

/// Same row budget used by painting and the interaction adapter.
pub fn progression_visible_rows(viewport_height: u32) -> usize {
    let available =
        (viewport_height as f32).min(WINDOW_HEIGHT) - LIST_TOP - HEADER_HEIGHT - FOOTER_HEIGHT;
    ((available.max(0.) / ROW_HEIGHT) as usize).min(PROGRESSION_VISIBLE_ROWS)
}

impl Painter<'_> {
    pub(crate) fn progression(&mut self, state: &GameHudState, progression: &UiProgression) {
        if !progression.open {
            return;
        }
        let rect = position(
            state,
            "skills",
            Rect::new(self.screen.width * 0.5 - 210., 32., 420., WINDOW_HEIGHT),
            self.screen,
        );
        self.shell(
            "SkillsWindow",
            "skills",
            rect,
            "Skills, languages & AA",
            true,
        );
        self.text(
            Rect::new(rect.x + 12., rect.y + 28., (rect.width - 24.).max(0.), 34.),
            if progression.status.is_empty() {
                "Waiting for character progression"
            } else {
                &progression.status
            },
            MUTED,
            true,
        );
        let tab_width = ((rect.width - 32.) / 3.).max(0.);
        for (index, (page, suffix, label)) in [
            (UiProgressionPage::Skills, "skills", "Skills"),
            (UiProgressionPage::Languages, "languages", "Languages"),
            (UiProgressionPage::AlternateAdvancement, "aa", "AA"),
        ]
        .into_iter()
        .enumerate()
        {
            self.button(
                "SKLW_DoneButton",
                &format!("progression:{}:{suffix}", progression.revision),
                Rect::new(
                    rect.x + 12. + index as f32 * (tab_width + 4.),
                    rect.y + 68.,
                    tab_width,
                    24.,
                ),
                label,
                progression.page == page,
            );
        }
        if progression.page == UiProgressionPage::AlternateAdvancement {
            self.aa_progression(rect, progression);
            return;
        }
        let rows = progression_visible_rows(self.screen.height as u32);
        let list = Rect::new(
            rect.x + 10.,
            rect.y + LIST_TOP,
            (rect.width - 20.).max(0.),
            HEADER_HEIGHT + rows as f32 * ROW_HEIGHT,
        );
        let mut bindings = UiBindings::default();
        bindings.widget_mut("SKLW_SkillList").rect = Some(list);
        self.widget("SKLW_SkillList", &bindings);
        self.hit(
            format!("progression:{}:list", progression.revision),
            "ProgressionList",
            list,
            None,
        );
        let cells = (list.width - 25.).max(0.);
        let value_width = 88_f32.min(cells * 0.4);
        let name_width = (cells - value_width).max(0.);
        self.text(
            Rect::new(list.x + 5., list.y + 3., (name_width - 4.).max(0.), 18.),
            if progression.page == UiProgressionPage::Skills {
                "Skill name"
            } else {
                "Language"
            },
            GOLD,
            false,
        );
        self.text(
            Rect::new(
                list.x + name_width + 5.,
                list.y + 3.,
                (value_width - 4.).max(0.),
                18.,
            ),
            if progression.page == UiProgressionPage::Skills {
                "Base value"
            } else {
                "Value"
            },
            GOLD,
            false,
        );
        let count = progression.rows().count();
        let max_scroll = count.saturating_sub(rows.max(1));
        let start = progression.scroll.min(max_scroll);
        for (index, row) in progression.rows().skip(start).take(rows).enumerate() {
            let bounds = Rect::new(
                list.x + 3.,
                list.y + HEADER_HEIGHT + index as f32 * ROW_HEIGHT,
                cells,
                ROW_HEIGHT,
            );
            if index % 2 == 1 {
                self.fill(bounds, [25, 28, 31, 170]);
            }
            let color = if progression.current { WHITE } else { MUTED };
            self.text(
                Rect::new(bounds.x + 2., bounds.y + 1., (name_width - 4.).max(0.), 18.),
                &row.name,
                color,
                false,
            );
            let value = row
                .value
                .map_or_else(|| "—".into(), |value| value.to_string());
            self.text(
                Rect::new(
                    bounds.x + name_width + 2.,
                    bounds.y + 1.,
                    (value_width - 4.).max(0.),
                    18.,
                ),
                &value,
                color,
                false,
            );
            self.hit(
                format!("progression:{}:row:{}", progression.revision, row.id),
                "ProgressionList",
                bounds,
                Some(format!(
                    "{}: {}{}",
                    row.name,
                    row.value
                        .map_or_else(|| "unavailable".into(), |value| value.to_string()),
                    if progression.current {
                        ""
                    } else {
                        " (last received; stale)"
                    }
                )),
            );
        }
        if count == 0 && rows > 0 {
            self.text(
                Rect::new(
                    list.x + 8.,
                    list.y + HEADER_HEIGHT + 8.,
                    (list.width - 35.).max(0.),
                    40.,
                ),
                "No values received yet.",
                MUTED,
                true,
            );
        }
        for (direction, label, y, enabled) in [
            (-1, "↑", list.y + 2., start > 0 && rows > 0),
            (1, "↓", list.bottom() - 22., start < max_scroll && rows > 0),
        ] {
            self.button_enabled(
                "SKLW_DoneButton",
                &format!("progression:{}:scroll:{direction}", progression.revision),
                Rect::new(list.right() - 20., y, 18., 20.),
                label,
                false,
                enabled,
            );
            if let Some(hit) = self.frame.hit_targets.last_mut() {
                hit.kind = "ProgressionList".into();
            }
        }
        if max_scroll > 0 && rows > 0 {
            let track = (list.height - 48.).max(0.);
            let thumb = (track * rows as f32 / count as f32).max(10.).min(track);
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
        self.text(
            Rect::new(
                rect.x + 12.,
                list.bottom() + 4.,
                (rect.width - 24.).max(0.),
                18.,
            ),
            if progression.current {
                "Server values · Caps and ranks unavailable"
            } else if count == 0 {
                "Values will appear when received"
            } else {
                "Last received values · Not current"
            },
            MUTED,
            false,
        );
        self.button(
            "SKLW_DoneButton",
            "game:close:skills",
            Rect::new(rect.right() - 106., rect.bottom() - 29., 94., 22.),
            "Done",
            false,
        );
    }

    fn aa_progression(&mut self, rect: Rect, progression: &UiProgression) {
        let left = rect.x + 12.;
        let width = (rect.width - 24.).max(0.);
        let top = rect.y + LIST_TOP;
        let fraction = progression.aa_experience_fraction();
        let allocation = progression.aa_allocation();
        // Keep each value on its own row even in the narrow viewport. On very
        // short windows, omit rows that would overlap the Done control.
        for (offset, label, current) in [
            (
                0.,
                fraction.map_or_else(
                    || "AA experience unavailable".into(),
                    |value| format!("AA experience {:.1}%", value * 100.),
                ),
                fraction.is_some(),
            ),
            (
                36.,
                progression.aa_unspent_points.map_or_else(
                    || "Unspent AA points unavailable".into(),
                    |points| {
                        format!(
                            "Unspent AA points: {points}{}",
                            if progression.current { "" } else { " (stale)" }
                        )
                    },
                ),
                progression.current && progression.aa_unspent_points.is_some(),
            ),
            (
                58.,
                allocation.map_or_else(
                    || "Experience to AA unavailable".into(),
                    |percent| format!("Experience to AA: {percent}%"),
                ),
                allocation.is_some(),
            ),
        ] {
            if top + offset + 18. <= rect.bottom() - 38. {
                self.text(
                    Rect::new(left, top + offset, width, 18.),
                    label,
                    if current { WHITE } else { MUTED },
                    false,
                );
            }
        }
        if top + 33. <= rect.bottom() - 38. {
            let mut bindings = UiBindings::default();
            let gauge = bindings.widget_mut("IW_AltAdvGauge");
            gauge.rect = Some(Rect::new(left, top + 22., width, 11.));
            gauge.gauge = Some(fraction.unwrap_or(0.));
            gauge.enabled = Some(fraction.is_some());
            self.widget("IW_AltAdvGauge", &bindings);
        }
        if top + 142. <= rect.bottom() - 38. {
            self.text(
                Rect::new(left, top + 108., width, 34.),
                if progression.current {
                    "Values update when received."
                } else {
                    "Last received values · Not current"
                },
                MUTED,
                true,
            );
        }
        self.button(
            "SKLW_DoneButton",
            "game:close:skills",
            Rect::new(rect.right() - 106., rect.bottom() - 29., 94., 22.),
            "Done",
            false,
        );
    }

    pub(crate) fn inventory_progression(
        &mut self,
        rect: Rect,
        progression: Option<&UiProgression>,
    ) {
        let top = rect.bottom() - 86.;
        let level = progression.and_then(|state| state.level).map_or_else(
            || "Level unavailable".into(),
            |level| {
                format!(
                    "Level {level}{}",
                    if progression.is_some_and(|state| state.current) {
                        ""
                    } else {
                        " (stale)"
                    }
                )
            },
        );
        self.text(
            Rect::new(rect.x + 12., top, (rect.width - 122.).max(0.), 18.),
            level,
            if progression.is_some_and(|state| state.current) {
                WHITE
            } else {
                MUTED
            },
            false,
        );
        self.button(
            "IW_Skills",
            "game:skills",
            Rect::new(rect.right() - 100., top, 88., 22.),
            "Skills",
            false,
        );
        let fraction = progression.and_then(UiProgression::experience_fraction);
        self.text(
            Rect::new(rect.x + 12., top + 21., (rect.width - 24.).max(0.), 18.),
            fraction.map_or_else(
                || "Experience unavailable".into(),
                |value| format!("Experience {:.1}%", value * 100.),
            ),
            if fraction.is_some() { GOLD } else { MUTED },
            false,
        );
        let mut bindings = UiBindings::default();
        let gauge = bindings.widget_mut("IW_ExpGauge");
        gauge.rect = Some(Rect::new(
            rect.x + 12.,
            top + 42.,
            (rect.width - 24.).max(0.),
            11.,
        ));
        gauge.gauge = Some(fraction.unwrap_or(0.));
        gauge.enabled = Some(fraction.is_some());
        self.widget("IW_ExpGauge", &bindings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gameplay_ui::{UiAction, UiSlot, WindowStack, valid_window_id},
        hud::{Hud, HudState},
    };
    use openeq_ui::{DrawCommand, UiDocument};

    fn fixture() -> UiProgression {
        UiProgression {
            revision: 73,
            status: "Received character progression".into(),
            current: true,
            level: Some(65),
            experience_bar_units: Some(165),
            aa_experience_bar_units: Some(66),
            aa_unspent_points: Some(12),
            aa_allocation_percent: Some(50),
            open: true,
            skills: (0..78)
                .map(|id| UiProgressionRow {
                    id,
                    name: format!("Skill {id}"),
                    value: Some(id * 3),
                })
                .collect::<Vec<_>>()
                .into(),
            languages: (0..28)
                .map(|id| UiProgressionRow {
                    id,
                    name: format!("Language {id}"),
                    value: Some(id * 2),
                })
                .collect::<Vec<_>>()
                .into(),
            ..Default::default()
        }
    }
    fn minimal_hud() -> Hud {
        Hud {
            ui:UiDocument::from_xml(r#"<XML>
                <Screen item="SkillsWindow"><Pieces>SKLW_MakeHotKeyButton</Pieces></Screen>
                <Button item="SKLW_MakeHotKeyButton"><Text>Make Hotkey</Text></Button>
                <Listbox item="SKLW_SkillList" />
                <Button item="SKLW_DoneButton" /><Button item="IW_Skills" />
                <Gauge item="IW_ExpGauge"><GaugeOffsetY>0</GaugeOffsetY><FillTint><R>220</R><G>150</G><B>0</B></FillTint></Gauge>
                <Gauge item="IW_AltAdvGauge"><GaugeOffsetY>0</GaugeOffsetY><FillTint><R>20</R><G>150</G><B>220</B></FillTint></Gauge>
                </XML>"#).unwrap(),
            initial_bindings:UiBindings::default(),
        }
    }
    fn hit(item: &str) -> HitTarget {
        HitTarget {
            item: item.into(),
            screen_id: item.into(),
            window_id: Some("skills".into()),
            kind: "ProgressionList".into(),
            rect: Rect::new(0., 0., 20., 20.),
            enabled: true,
            tooltip: None,
        }
    }

    #[test]
    fn local_actions_require_the_displayed_window_and_revision() {
        for (suffix, kind) in [
            (
                "skills",
                ProgressionActionKind::Page(UiProgressionPage::Skills),
            ),
            (
                "languages",
                ProgressionActionKind::Page(UiProgressionPage::Languages),
            ),
            (
                "aa",
                ProgressionActionKind::Page(UiProgressionPage::AlternateAdvancement),
            ),
            ("scroll:-1", ProgressionActionKind::Scroll { rows: -1 }),
            ("scroll:1", ProgressionActionKind::Scroll { rows: 1 }),
        ] {
            assert_eq!(
                UiAction::from_hit(&hit(&format!("progression:73:{suffix}"))),
                Some(UiAction::Progression(ProgressionAction {
                    revision: 73,
                    kind
                }))
            );
        }
        for suffix in [
            "train",
            "hotkey",
            "scroll:0",
            "scroll:99",
            "row:1",
            "list",
            "skills:1",
        ] {
            assert!(
                ProgressionAction::from_hit(&hit(&format!("progression:73:{suffix}"))).is_none()
            );
        }
        for suffix in ["list", "row:77", "scroll:1"] {
            assert_eq!(
                progression_scroll_hit_identity(&hit(&format!("progression:73:{suffix}"))),
                Some(73)
            );
        }
        for item in [
            "progression:x:list",
            "progression:73:row:78",
            "progression:73:row:9999999999",
            "progression:73:train",
        ] {
            assert!(progression_scroll_hit_identity(&hit(item)).is_none());
        }
        let mut covered = hit("progression:73:scroll:1");
        covered.window_id = Some("inventory".into());
        assert!(ProgressionAction::from_hit(&covered).is_none());
        assert!(progression_scroll_hit_identity(&covered).is_none());
        covered.window_id = Some("skills".into());
        covered.enabled = false;
        assert!(ProgressionAction::from_hit(&covered).is_none());
        assert!(progression_scroll_hit_identity(&covered).is_none());
    }

    #[test]
    fn experience_distinguishes_zero_unknown_invalid_and_stale() {
        let mut state = fixture();
        for (units, expected) in [
            (None, None),
            (Some(0), Some(0.)),
            (Some(165), Some(0.5)),
            (Some(330), Some(1.)),
            (Some(331), None),
            (Some(u32::MAX), None),
        ] {
            state.experience_bar_units = units;
            assert_eq!(state.experience_fraction(), expected);
        }
        state.experience_bar_units = Some(165);
        state.current = false;
        assert_eq!(state.experience_fraction(), None);
    }

    #[test]
    fn aa_display_distinguishes_zero_unknown_invalid_and_stale_without_actions() {
        let hud = minimal_hud();
        for (units, points, allocation, current, labels, fill_fraction) in [
            (
                Some(0),
                Some(0),
                Some(0),
                true,
                [
                    "AA experience 0.0%",
                    "Unspent AA points: 0",
                    "Experience to AA: 0%",
                ],
                0.,
            ),
            (
                Some(330),
                Some(u32::MAX),
                Some(100),
                true,
                [
                    "AA experience 100.0%",
                    "Unspent AA points: 4294967295",
                    "Experience to AA: 100%",
                ],
                1.,
            ),
            (
                Some(165),
                Some(12),
                Some(50),
                true,
                [
                    "AA experience 50.0%",
                    "Unspent AA points: 12",
                    "Experience to AA: 50%",
                ],
                0.5,
            ),
            (
                None,
                None,
                None,
                true,
                [
                    "AA experience unavailable",
                    "Unspent AA points unavailable",
                    "Experience to AA unavailable",
                ],
                0.,
            ),
            (
                Some(331),
                Some(12),
                Some(101),
                true,
                [
                    "AA experience unavailable",
                    "Unspent AA points: 12",
                    "Experience to AA unavailable",
                ],
                0.,
            ),
            (
                Some(165),
                Some(12),
                Some(50),
                false,
                [
                    "AA experience unavailable",
                    "Unspent AA points: 12 (stale)",
                    "Experience to AA unavailable",
                ],
                0.,
            ),
        ] {
            let progression = UiProgression {
                page: UiProgressionPage::AlternateAdvancement,
                aa_experience_bar_units: units,
                aa_unspent_points: points,
                aa_allocation_percent: allocation,
                current,
                ..fixture()
            };
            let state = GameHudState {
                progression: Some(progression),
                ..Default::default()
            };
            for viewport in [[900, 650], [350, 400], [240, 220]] {
                let frame = hud.gameplay_frame(viewport, &HudState::default(), &state);
                let done = frame
                    .hit_targets
                    .iter()
                    .rev()
                    .find(|hit| hit.item == "game:close:skills")
                    .unwrap();
                for label in labels {
                    assert!(frame.commands.iter().any(|cmd| matches!(cmd,
                        DrawCommand::Text { text, rect, .. } if text == label && rect.bottom() < done.rect.y)),
                        "{viewport:?}: {label}");
                }
                let fill = frame
                    .commands
                    .iter()
                    .find_map(|cmd| match cmd {
                        DrawCommand::Fill {
                            rect, clip, color, ..
                        } if *color == [20, 150, 220, 255] => Some((rect.width, clip.width)),
                        _ => None,
                    })
                    .unwrap();
                assert!((fill.1 - fill.0 * fill_fraction).abs() < 0.001);
                assert!(
                    frame
                        .hit_targets
                        .iter()
                        .filter(|hit| hit.window_id.as_deref() == Some("skills"))
                        .all(|hit| !hit.item.starts_with("progression:")
                            || matches!(
                                ProgressionAction::from_hit(hit),
                                Some(ProgressionAction {
                                    kind: ProgressionActionKind::Page(_),
                                    ..
                                })
                            ))
                );
            }
        }
        let mut progression = fixture();
        for units in [331, u32::MAX] {
            progression.aa_experience_bar_units = Some(units);
            assert_eq!(progression.aa_experience_fraction(), None);
        }
        for allocation in [101, u8::MAX] {
            progression.aa_allocation_percent = Some(allocation);
            assert_eq!(progression.aa_allocation(), None);
        }
    }

    #[test]
    fn bounded_rows_keep_ids_at_bottom_on_each_page_and_viewport() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            progression: Some(fixture()),
            ..Default::default()
        };
        let p = state.progression.as_mut().unwrap();
        p.skills = (0..10_000)
            .map(|id| UiProgressionRow {
                id,
                name: format!("Skill {id}"),
                value: Some(id),
            })
            .collect::<Vec<_>>()
            .into();
        p.scroll = usize::MAX;
        for (page, last_id) in [
            (UiProgressionPage::Skills, 77),
            (UiProgressionPage::Languages, 27),
        ] {
            state.progression.as_mut().unwrap().page = page;
            for viewport in [[900, 650], [350, 400], [240, 220]] {
                let frame = hud.gameplay_frame(viewport, &HudState::default(), &state);
                let rows: Vec<_> = frame
                    .hit_targets
                    .iter()
                    .filter(|hit| hit.item.starts_with("progression:73:row:"))
                    .collect();
                assert_eq!(rows.len(), progression_visible_rows(viewport[1]));
                assert_eq!(
                    rows.last().unwrap().item,
                    format!("progression:73:row:{last_id}")
                );
                assert!(
                    rows.iter()
                        .all(|hit| hit.window_id.as_deref() == Some("skills")
                            && hit.rect.bottom() <= viewport[1] as f32)
                );
                assert!(
                    !frame
                        .hit_targets
                        .iter()
                        .find(|hit| hit.item == "progression:73:scroll:1")
                        .unwrap()
                        .enabled
                );
                assert!(!frame.commands.iter().any(|cmd|matches!(cmd,DrawCommand::Text{text,..} if text=="Make Hotkey"||text=="Rank")));
            }
        }
        assert_eq!(progression_visible_rows(80), 0);
    }

    #[test]
    fn inventory_keeps_current_zero_separate_from_unknown_progress() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            inventory_open: true,
            progression: Some(fixture()),
            ..Default::default()
        };
        state.progression.as_mut().unwrap().open = false;
        for (units, current, label, width) in [
            (Some(0), true, "Experience 0.0%", 0.),
            (Some(165), true, "Experience 50.0%", 157.),
            (Some(330), true, "Experience 100.0%", 314.),
            (None, true, "Experience unavailable", 0.),
            (Some(331), true, "Experience unavailable", 0.),
            (Some(165), false, "Experience unavailable", 0.),
        ] {
            let p = state.progression.as_mut().unwrap();
            p.experience_bar_units = units;
            p.current = current;
            let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
            assert!(
                frame
                    .commands
                    .iter()
                    .any(|cmd| matches!(cmd,DrawCommand::Text{text,..} if text==label))
            );
            let fill = frame
                .commands
                .iter()
                .find_map(|cmd| match cmd {
                    DrawCommand::Fill { clip, color, .. } if *color == [220, 150, 0, 255] => {
                        Some(clip.width)
                    }
                    _ => None,
                })
                .unwrap();
            assert_eq!(fill, width);
            let opener = frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == "game:skills")
                .unwrap();
            assert_eq!(UiAction::from_hit(opener), Some(UiAction::OpenSkills));
            assert!(
                !frame
                    .hit_targets
                    .iter()
                    .any(|hit| hit.window_id.as_deref() == Some("skills"))
            );
        }
    }

    #[test]
    fn window_order_and_close_preserve_pointer_ownership() {
        let hud = minimal_hud();
        let mut state = GameHudState {
            inventory_open: true,
            progression: Some(fixture()),
            ..Default::default()
        };
        state.window_positions.insert("skills".into(), [40., 50.]);
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
        assert!(valid_window_id("skills"));
        let close = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:close:skills")
            .unwrap();
        assert_eq!(
            UiAction::from_hit(close),
            Some(UiAction::CloseWindow("skills".into()))
        );
        let point = [80., 230.];
        assert_eq!(
            frame.hit_test(point).unwrap().window_id.as_deref(),
            Some("skills")
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
        state.progression.as_mut().unwrap().open = false;
        let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.window_id.as_deref() == Some("skills"))
        );
    }

    #[test]
    fn short_inventory_explains_hidden_slots_and_preserves_progression_controls() {
        let hud = minimal_hud();
        let state = GameHudState {
            inventory_open: true,
            progression: Some(UiProgression {
                open: false,
                ..fixture()
            }),
            equipment: (0..23)
                .map(|slot| UiSlot {
                    slot,
                    ..Default::default()
                })
                .collect(),
            inventory: (23..33)
                .map(|slot| UiSlot {
                    slot,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        for (viewport, clipped) in [([900, 650], false), ([350, 400], true)] {
            let frame = hud.gameplay_frame(viewport, &HudState::default(), &state);
            let slots: Vec<_> = frame
                .hit_targets
                .iter()
                .filter(|hit| hit.item.starts_with("game:slot:"))
                .collect();
            assert_eq!(slots.len() < 33, clipped);
            assert_eq!(frame.commands.iter().any(|cmd| matches!(cmd, DrawCommand::Text {text,..} if text == "Enlarge the game window to see all slots.")), clipped);
            let skills = frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == "game:skills")
                .unwrap();
            assert!(slots.iter().all(|hit| hit.rect.bottom() < skills.rect.y));
            assert_eq!(
                frame
                    .hit_test([skills.rect.x + 3., skills.rect.y + 3.])
                    .and_then(UiAction::from_hit),
                Some(UiAction::OpenSkills)
            );
        }
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn original_progression_skin_at_normal_retina_and_narrow_sizes() {
        let base = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(&base).unwrap();
        let names = crate::game::StringTable::load(&base);
        let resources = HudState {
            character: "Adventurer".into(),
            hp: 1.,
            ..Default::default()
        };
        let strings = std::fs::read(base.join("eqstr_us.txt")).unwrap();
        let decoded = encoding_rs::WINDOWS_1252.decode(&strings).0;
        let table: std::collections::BTreeMap<u32, &str> = decoded
            .lines()
            .filter_map(|line| {
                let (id, name) = line.split_once(' ')?;
                Some((id.parse().ok()?, name.trim_end()))
            })
            .collect();
        let skill_strings = [
            13855, 13856, 13857, 13858, 13859, 13861, 13862, 13863, 13864, 13866, 13867, 13871,
            13872, 13874, 13875, 13876, 13877, 13878, 13879, 13880, 13881, 13882, 13883, 13884,
            13885, 13886, 13888, 13889, 13890, 13891, 13893, 13894, 13895, 13896, 13897, 13899,
            13900, 13903, 13904, 13905, 13906, 13908, 13909, 13910, 13911, 13912, 13913, 13914,
            13915, 13916, 13917, 13919, 13920, 13921, 13923, 13854, 13853, 13852, 13851, 13850,
            13865, 13918, 13907, 13870, 13887, 13873, 13860, 13868, 13892, 13901, 13898, 13922,
            13869, 13902, 5837, 3670, 13049, 789,
        ];
        let language_strings = [
            3114, 3200, 3201, 3202, 3203, 3204, 3205, 3206, 3207, 3208, 3217, 3219, 3220, 3209,
            3210, 3211, 3212, 3213, 3214, 3215, 3216, 3218, 3221, 3222, 3223, 7658, 7659,
        ];
        for scale in [1, 2] {
            let mut renderer =
                openeq_render::Renderer::new_headless(1000 * scale, 650 * scale).unwrap();
            for mode in [
                "skills",
                "skills-bottom",
                "languages",
                "languages-bottom",
                "unknown",
                "stale",
                "zero",
                "full",
                "narrow-skills",
                "narrow-inventory",
                "aa",
                "aa-zero",
                "aa-full",
                "aa-unknown",
                "aa-invalid",
                "aa-stale",
                "narrow-aa",
                "narrow-aa-stale",
            ] {
                let viewport = if mode.starts_with("narrow") {
                    [350, 400]
                } else {
                    [1000, 650]
                };
                renderer.resize(viewport[0] * scale, viewport[1] * scale);
                let mut progression = fixture();
                for (row, string) in Arc::make_mut(&mut progression.skills)
                    .iter_mut()
                    .zip(skill_strings)
                {
                    row.name = crate::progression::skill_name(&names, row.id);
                    assert_eq!(row.name, table[&string]);
                }
                for (row, string) in Arc::make_mut(&mut progression.languages)
                    .iter_mut()
                    .zip(language_strings)
                {
                    row.name = crate::progression::language_name(&names, row.id);
                    assert_eq!(row.name, table[&string]);
                }
                if mode.starts_with("languages") {
                    progression.page = UiProgressionPage::Languages;
                }
                if mode.ends_with("bottom") {
                    progression.scroll = usize::MAX;
                }
                if mode == "unknown" {
                    progression = UiProgression {
                        open: true,
                        revision: 74,
                        ..Default::default()
                    };
                }
                if mode == "stale" {
                    progression.current = false;
                    progression.status =
                        "Connection lost · Values are from the previous session".into();
                }
                if mode == "zero" {
                    progression.experience_bar_units = Some(0);
                }
                if mode == "full" {
                    progression.experience_bar_units = Some(330);
                }
                if mode == "narrow-inventory" {
                    progression.open = false;
                }
                if mode.contains("aa") {
                    progression.page = UiProgressionPage::AlternateAdvancement;
                    progression.status =
                        "Alternate advancement values reported by the server.".into();
                    if mode.ends_with("zero") {
                        progression.aa_experience_bar_units = Some(0);
                        progression.aa_unspent_points = Some(0);
                        progression.aa_allocation_percent = Some(0);
                    } else if mode.ends_with("full") {
                        progression.aa_experience_bar_units = Some(330);
                        progression.aa_unspent_points = Some(u32::MAX);
                        progression.aa_allocation_percent = Some(100);
                    } else if mode.ends_with("unknown") {
                        progression.aa_experience_bar_units = None;
                        progression.aa_unspent_points = None;
                        progression.aa_allocation_percent = None;
                    } else if mode.ends_with("invalid") {
                        progression.aa_experience_bar_units = Some(u32::MAX);
                        progression.aa_allocation_percent = Some(u8::MAX);
                    } else if mode.ends_with("stale") {
                        progression.current = false;
                        progression.status =
                            "Connection lost · Values are from the previous session".into();
                    }
                }
                let mut state = GameHudState {
                    progression: Some(progression),
                    inventory_open: !mode.starts_with("narrow") || mode == "narrow-inventory",
                    equipment: (0..23)
                        .map(|slot| UiSlot {
                            slot,
                            ..Default::default()
                        })
                        .collect(),
                    inventory: (23..33)
                        .map(|slot| UiSlot {
                            slot,
                            ..Default::default()
                        })
                        .collect(),
                    ..Default::default()
                };
                state.window_positions.insert("skills".into(), [12., 110.]);
                let frame = hud.gameplay_frame(viewport, &resources, &state);
                assert!(frame.warnings.is_empty(), "{mode}: {:?}", frame.warnings);
                assert!(
                    !frame
                        .hit_targets
                        .iter()
                        .any(|hit| hit.item == "SKLW_MakeHotKeyButton")
                );
                renderer.set_ui_scaled(&frame, scale as f32);
                renderer.render_ui();
                if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    let (width, height, pixels) = renderer.read_rgba().unwrap();
                    image::save_buffer(
                        std::path::PathBuf::from(directory)
                            .join(format!("progression-{mode}-{scale}x.png")),
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
