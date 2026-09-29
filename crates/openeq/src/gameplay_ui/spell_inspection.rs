//! One read-only original spell window. Selection/scroll state is transient;
//! its logical window position and stack order use the ordinary UI layout.
use super::*;

const WINDOW: &str = "spell_inspection";
const DESCRIPTION: &str = "SDW_SpellDescription";
const MAX_SCROLL: usize = 16_384;

#[derive(Clone, Debug, Default)]
pub struct SpellInspection {
    id: Option<u32>,
    scroll_rows: usize,
    max_scroll: Option<usize>,
}

impl SpellInspection {
    pub fn id(&self) -> Option<u32> {
        self.id
    }
    pub fn scroll_rows(&self) -> usize {
        self.scroll_rows
    }

    pub fn open(&mut self, id: u32) {
        if id == 0 || id == u32::MAX {
            return;
        }
        self.id = Some(id);
        self.scroll_rows = 0;
        self.max_scroll = None;
    }

    /// Returns whether a window was closed, so Escape can retain one owner.
    pub fn close(&mut self) -> bool {
        let open = self.id.take().is_some();
        self.scroll_rows = 0;
        self.max_scroll = None;
        open
    }

    pub fn scroll(&mut self, rows: isize) {
        if self.id.is_some() {
            self.scroll_rows = self
                .scroll_rows
                .saturating_add_signed(rows)
                .min(self.max_scroll.unwrap_or(MAX_SCROLL))
                .min(MAX_SCROLL);
        }
    }

    /// Pass only the displayed frame's topmost hit, after recovery ownership.
    /// Positive wheel Y scrolls toward the start. Body/chrome consume the wheel
    /// too, preventing a map or world view behind this window from moving.
    pub fn wheel(&mut self, hit: &HitTarget, y: f32) -> bool {
        if self.id.is_none() || hit.window_id.as_deref() != Some(WINDOW) {
            return false;
        }
        if y.is_finite() {
            let rows = (y.abs() * 3.).ceil().min(MAX_SCROLL as f32) as isize;
            self.scroll(if y > 0. { -rows } else { rows });
        }
        true
    }

    /// A content-specific ID prevents a previous spell's frame from clamping
    /// a newly opened description. The renderer owns wrap/Retina measurement.
    pub fn update_metrics(&mut self, metrics: &[openeq_ui::TextScrollMetrics]) {
        let Some(id) = self.id else { return };
        if let Some(metrics) = metrics
            .iter()
            .find(|metrics| metrics.id == format!("spell:{id}"))
        {
            self.max_scroll = Some(metrics.max_scroll().min(MAX_SCROLL));
            self.scroll_rows = self.scroll_rows.min(self.max_scroll.unwrap());
        }
    }
}

impl Painter<'_> {
    pub(super) fn spell_inspection(&mut self, state: &GameHudState, spell: &UiSpell) {
        let rect = position(
            state,
            WINDOW,
            Rect::new(self.screen.width * 0.5 - 200., 110., 400., 190.),
            self.screen,
        );
        self.shell(
            "SpellDisplayWindow",
            WINDOW,
            rect,
            "Spell information",
            true,
        );
        let icon = Rect::new(rect.right() - 54., rect.y + 25., 42., 42.);
        let mut bindings = UiBindings::default();
        bindings.widget_mut("SDW_IconButton").rect = Some(icon);
        self.widget("SDW_IconButton", &bindings);
        self.animation("A_SpellIcons", Some(spell.icon as usize), inset(icon, 2.));
        self.text(
            Rect::new(rect.x + 12., rect.y + 28., (rect.width - 78.).max(0.), 42.),
            &spell.name,
            GOLD,
            true,
        );
        let bounds = Rect::new(
            rect.x + 8.,
            rect.y + 74.,
            (rect.width - 16.).max(0.),
            (rect.height - 82.).max(0.),
        );
        let description = bindings.widget_mut(DESCRIPTION);
        description.rect = Some(bounds);
        description.text = Some(spell.description.clone());
        description.scroll_rows = Some(state.spell_scroll_rows);
        description.scroll_id = Some(format!("spell:{}", spell.id));
        self.widget(DESCRIPTION, &bindings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(owner: Option<&str>) -> HitTarget {
        HitTarget {
            item: DESCRIPTION.into(),
            screen_id: DESCRIPTION.into(),
            window_id: owner.map(str::to_owned),
            kind: "STMLbox".into(),
            rect: Rect::new(0., 0., 100., 100.),
            enabled: true,
            tooltip: None,
        }
    }

    #[test]
    fn scroll_uses_current_content_metrics_and_topmost_owner() {
        let mut state = SpellInspection::default();
        assert!(!state.wheel(&hit(Some(WINDOW)), -1.));
        state.open(200);
        assert!(!state.wheel(&hit(Some("map")), -1.));
        assert!(!state.wheel(&hit(None), -1.));
        assert!(state.wheel(&hit(Some(WINDOW)), -2.));
        assert_eq!(state.scroll_rows(), 6);
        state.update_metrics(&[openeq_ui::TextScrollMetrics {
            id: "spell:201".into(),
            total_rows: 1,
            visible_rows: 1,
            first_row: 0,
        }]);
        assert_eq!(state.scroll_rows(), 6);
        state.update_metrics(&[openeq_ui::TextScrollMetrics {
            id: "spell:200".into(),
            total_rows: 7,
            visible_rows: 3,
            first_row: 4,
        }]);
        assert_eq!(state.scroll_rows(), 4);
        state.wheel(&hit(Some(WINDOW)), f32::NEG_INFINITY);
        assert_eq!(state.scroll_rows(), 4);
        state.scroll(isize::MIN);
        assert_eq!(state.scroll_rows(), 0);
        state.scroll(isize::MAX);
        assert_eq!(state.scroll_rows(), 4);
        state.open(201);
        assert_eq!(state.scroll_rows(), 0);
        assert!(state.close());
        assert!(!state.close());
    }

    #[test]
    fn spellbook_inspection_sends_no_commands_and_preserves_existing_gestures() {
        use crate::{
            interaction::Interaction,
            live::{NetworkCommand, tests::command_world},
        };
        use openeq_net::gameplay::Command;
        let (mut live, mut wire) = command_world(1, 10.);
        live.game = crate::item_use_state::tests::fixture();
        live.game.profile.as_mut().unwrap().spell_book[0] = 288;
        let mut interaction = Interaction::default();
        let action = UiAction::MemorizeSpell { id: 288, gem: 2 };
        interaction.ui_action(action.clone(), true, false, &mut live, [0.; 3]);
        assert_eq!(interaction.spell_inspection.id(), Some(288));
        assert!(
            wire.try_recv().is_err(),
            "inspection must not send posture or memorize"
        );
        interaction.ui_action(
            UiAction::MemorizeSpell { id: 9999, gem: 2 },
            true,
            false,
            &mut live,
            [0.; 3],
        );
        assert_eq!(interaction.spell_inspection.id(), Some(288));
        assert!(wire.try_recv().is_err());
        interaction.ui_action(action, false, false, &mut live, [0.; 3]);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::Posture {
                posture: 1,
                ..
            }))
        ));
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::MemorizeSpell {
                slot: 2,
                spell_id: 288
            }))
        ));
        interaction.ui_action(UiAction::CastGem(4), true, false, &mut live, [0.; 3]);
        assert!(interaction.spellbook_open);
        assert_eq!(interaction.selected_gem, Some(4));
        assert!(wire.try_recv().is_err());
        interaction.ui_action(UiAction::RemoveBuff(2), false, false, &mut live, [0.; 3]);
        assert!(wire.try_recv().is_err());
        interaction.ui_action(UiAction::RemoveBuff(2), true, false, &mut live, [0.; 3]);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(Command::RemoveBuff {
                slot: 2,
                player_id: 1
            }))
        ));
        interaction.close_window(WINDOW, &mut live);
        assert_eq!(interaction.spell_inspection.id(), None);
        assert!(wire.try_recv().is_err());
    }

    #[test]
    #[ignore = "requires original EverQuest assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn original_spell_inspection_scrolls_and_stacks_at_both_scales() {
        let base = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let catalog = crate::spells::SpellCatalog::load(&base).unwrap();
        assert_eq!(catalog.spells[&41711].description_id, 13679);
        assert!(catalog.spells[&41711].description.len() >= 900);
        let hud = Hud::load(&base).unwrap();
        let viewport = [800, 600];
        let mut state = GameHudState {
            inventory_open: true,
            ..Default::default()
        };
        state
            .window_positions
            .insert("inventory".into(), [195., 95.]);
        state.window_positions.insert(WINDOW.into(), [220., 110.]);
        let resources = HudState {
            character: "Adventurer".into(),
            hp: 1.,
            ..Default::default()
        };
        let mut stack = WindowStack::default();
        for scale in [1., 2.] {
            let mut renderer = openeq_render::Renderer::new_headless(
                (viewport[0] as f32 * scale) as u32,
                (viewport[1] as f32 * scale) as u32,
            )
            .unwrap();
            for (name, id, scroll) in [
                ("short", 13, 0),
                ("long-top", 41711, 0),
                ("long-bottom", 41711, usize::MAX),
            ] {
                state.inspected_spell = Some(catalog.inspection(id, if id == 13 { 2 } else { 12 }));
                state.spell_scroll_rows = scroll;
                let frame = hud.gameplay_frame_with_windows(
                    viewport,
                    &resources,
                    &state,
                    vec![],
                    &mut stack,
                );
                assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
                let close = frame
                    .hit_targets
                    .iter()
                    .find(|hit| hit.item == "game:close:spell_inspection")
                    .unwrap();
                assert_eq!(
                    frame
                        .hit_test([close.rect.x + 3., close.rect.y + 3.])
                        .unwrap()
                        .item,
                    close.item
                );
                assert_eq!(
                    UiAction::from_hit(close),
                    Some(UiAction::CloseWindow(WINDOW.into()))
                );
                let text = frame
                    .hit_targets
                    .iter()
                    .find(|hit| hit.item == DESCRIPTION)
                    .unwrap();
                assert_eq!(
                    frame
                        .hit_test([text.rect.x + 8., text.rect.y + 8.])
                        .unwrap()
                        .window_id
                        .as_deref(),
                    Some(WINDOW)
                );
                assert_eq!(
                    frame
                        .hit_targets
                        .iter()
                        .find_map(|hit| (hit.item == "SDW_SpellDescription:scroll:1")
                            .then(|| UiAction::from_hit(hit)))
                        .flatten(),
                    Some(UiAction::ScrollSpell(1))
                );
                let thumb = frame
                    .commands
                    .iter()
                    .find_map(|command| match command {
                        DrawCommand::TextArea { thumb, .. } => thumb.as_ref(),
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    thumb.images.iter().all(Option::is_some),
                    "original thumb art must resolve"
                );
                renderer.set_ui_scaled(&frame, scale);
                let metrics = &renderer.ui_text_scroll_metrics()[0];
                assert_eq!(metrics.id, format!("spell:{id}"));
                if scroll == 0 {
                    assert_eq!(metrics.first_row, 0);
                } else {
                    assert_eq!(metrics.first_row, metrics.max_scroll());
                    assert!(metrics.first_row > 0);
                }
                renderer.render_ui();
                if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    let (width, height, pixels) = renderer.read_rgba().unwrap();
                    let path = std::path::PathBuf::from(directory)
                        .join(format!("spell-inspection-{name}-{}x.png", scale as u32));
                    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
                        .unwrap();
                }
            }
        }
        stack.raise("inventory");
        let covered =
            hud.gameplay_frame_with_windows(viewport, &resources, &state, vec![], &mut stack);
        assert_eq!(
            covered.hit_test([240., 210.]).unwrap().window_id.as_deref(),
            Some("inventory")
        );
        let mut inspection = SpellInspection::default();
        inspection.open(41711);
        assert!(!inspection.wheel(covered.hit_test([240., 210.]).unwrap(), -1.));
        state.inspected_spell = None;
        let closed =
            hud.gameplay_frame_with_windows(viewport, &resources, &state, vec![], &mut stack);
        assert!(
            !closed
                .hit_targets
                .iter()
                .any(|hit| hit.window_id.as_deref() == Some(WINDOW))
        );
        assert!(!closed.commands.iter().any(
            |command| matches!(command,DrawCommand::TextArea { id, .. } if id.starts_with("spell:"))
        ));
    }
}
