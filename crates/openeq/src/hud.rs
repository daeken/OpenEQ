//! Live HUD binding for the original EverQuest XML skin.
//!
//! Resource bars use the client's PlayerWindow and TargetWindow. The legacy
//! frame method displays connection diagnostics; gameplay_frame adds chat,
//! inventory, bags, loot and hit actions through the gameplay_ui module.

pub use crate::gameplay_ui::{
    ChatLine, GameHudState, UiAction, UiBag, UiBuff, UiCasting, UiItem, UiLoot, UiLootItem, UiSlot,
    UiSpell, UiSpellGem,
};

use anyhow::Context;
use openeq_ui::{Rect, UiBindings, UiDocument, UiFrame};
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct HudTarget {
    pub name: String,
    /// Current server-reported HP as a fraction in [0, 1].
    pub hp: f32,
    pub level: u8,
}

#[derive(Clone, Debug, Default)]
pub struct HudState {
    pub character: String,
    pub player_level: u8,
    /// Current server-reported HP as a fraction in [0, 1].
    pub hp: f32,
    /// Unknown values are hidden rather than displayed as full or empty bars.
    pub mana: Option<f32>,
    pub endurance: Option<f32>,
    pub target: Option<HudTarget>,
    pub status: String,
    pub entities: usize,
    pub movement_updates: u64,
}

pub struct Hud {
    pub(crate) ui: UiDocument,
    pub(crate) initial_bindings: UiBindings,
}

impl Hud {
    /// Accepts an EverQuest install directory, `uifiles`, or a skin directory.
    /// Returns an error instead of replacing missing original art with mock UI.
    pub fn load(base: impl AsRef<Path>) -> anyhow::Result<Self> {
        let base = base.as_ref();
        let directory = [
            base.to_owned(),
            base.join("default"),
            base.join("uifiles/default"),
        ]
        .into_iter()
        .find(|path| path.join("EQUI.xml").is_file())
        .with_context(|| {
            format!(
                "cannot find uifiles/default/EQUI.xml under {}",
                base.display()
            )
        })?;
        let ui =
            UiDocument::load(&directory, "EQUI.xml").context("loading the EverQuest HUD skin")?;
        for item in ["PlayerWindow", "TargetWindow", "ChatWindow"] {
            ui.window(item)
                .with_context(|| format!("validating HUD window {item}"))?;
        }
        let mut initial_bindings = UiBindings::default();
        for item in [
            "Pet_HP",
            "PW_CombatStateAnim",
            "PW_NewMailIcon",
            "PW_ParcelsIcon",
            "PW_ParcelsOverLimitIcon",
            "PW_VoiceVolume",
            "PW_GroupRoleTank",
            "PW_GroupRoleAssist",
            "PW_GroupRolePuller",
            "PW_GroupRoleMarkNPC",
            "Player_CombatTimer",
            "Player_CombatTimerLabel",
            "PW_AggroPctPlayerLabel",
            "PW_AggroNameSecondaryLabel",
            "PW_AggroPctSecondaryLabel",
            "A_AttackIndicatorAnimTop",
            "A_AttackIndicatorAnimBottom",
            "A_AttackIndicatorAnimLeft",
            "A_AttackIndicatorAnimRight",
            "A_AttackIndicatorAnimFill",
            "A_TargetBoxStaticAnimTop",
            "A_TargetBoxStaticAnimBottom",
            "A_TargetBoxStaticAnimLeft",
            "A_TargetBoxStaticAnimRight",
            "A_TargetBoxStaticAnimFill",
            "Target_AggroPctPlayerLabel",
            "Target_AggroNameSecondaryLabel",
            "Target_AggroPctSecondaryLabel",
            "Target_BuffWindow",
            "CW_ChatInput",
        ] {
            initial_bindings.widget_mut(item).visible = Some(false);
        }
        Ok(Self {
            ui,
            initial_bindings,
        })
    }

    /// Builds the three windows in physical pixels. Hit targets preserve XML
    /// `item`/`ScreenID` names, so callers can use `frame.hit_test(pointer)` to
    /// suppress world clicks over UI without coupling this module to input.
    pub fn frame(&self, viewport: [u32; 2], state: &HudState) -> UiFrame {
        let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
        let mut bindings = self.initial_bindings.clone();
        let margin = 12_f32.min(screen.width / 4.).min(screen.height / 4.);
        let available = (screen.width - margin * 2.).max(0.);
        let player = Rect::new(margin, margin, 240_f32.min(available), 95.);
        let target_width = 260_f32.min(available);
        let target = if available >= player.width + target_width + margin {
            Rect::new(player.right() + margin, margin, target_width, 52.)
        } else {
            Rect::new(margin, player.bottom() + margin, target_width, 52.)
        };
        let chat_height = 140_f32.min((screen.height * 0.4).max(0.));
        let chat = Rect::new(
            margin,
            (screen.height - margin - chat_height).max(margin),
            560_f32.min(available),
            chat_height,
        );
        bindings.widget_mut("PlayerWindow").rect = Some(player);
        bindings.widget_mut("TargetWindow").rect = Some(target);
        bindings.widget_mut("ChatWindow").rect = Some(chat);

        self.bind_resources(&mut bindings, state);

        bindings.widget_mut("ChatWindow").text = Some("World connection".to_owned());
        bindings.widget_mut("CW_ChatOutput").rect = Some(Rect::new(
            chat.x + 10.,
            chat.y + 23.,
            (chat.width - 20.).max(0.),
            (chat.height - 31.).max(0.),
        ));
        let status = if state.status.is_empty() {
            "Waiting for connection status"
        } else {
            &state.status
        };
        bindings.widget_mut("CW_ChatOutput").text = Some(format!(
            "{status}\n{} zone entities  |  {} movement updates",
            state.entities, state.movement_updates
        ));

        let mut result = UiFrame {
            bounds: screen,
            ..Default::default()
        };
        for item in ["PlayerWindow", "TargetWindow", "ChatWindow"] {
            // These immutable references were already checked during load.
            match self.ui.window(item) {
                Ok(window) => {
                    let mut frame = window.layout(screen, &bindings);
                    result.commands.append(&mut frame.commands);
                    result.hit_targets.append(&mut frame.hit_targets);
                    result.warnings.append(&mut frame.warnings);
                }
                Err(error) => result.warnings.push(error.to_string()),
            }
        }
        result.warnings.sort();
        result.warnings.dedup();
        result
    }
    pub(crate) fn bind_resources(&self, bindings: &mut UiBindings, state: &HudState) {
        let character = if state.character.is_empty() {
            "Player"
        } else {
            &state.character
        };
        bindings.widget_mut("Player_HP").text = Some(with_level(character, state.player_level));
        set_gauge(
            bindings,
            "Player_HP",
            "Player_HPLabel",
            "Player_HPPercLabel",
            Some(state.hp),
        );
        set_gauge(
            bindings,
            "Player_Mana",
            "Player_ManaLabel",
            "Player_ManaPercLabel",
            state.mana,
        );
        set_gauge(
            bindings,
            "Player_Fatigue",
            "Player_FatigueLabel",
            "Player_FatiguePercLabel",
            state.endurance,
        );

        bindings.widget_mut("Target_HP").text = Some(state.target.as_ref().map_or_else(
            || "No target".to_owned(),
            |target| with_level(&target.name, target.level),
        ));
        if let Some(target) = &state.target {
            set_gauge(
                bindings,
                "Target_HP",
                "Target_HPLabel",
                "Target_HPPercLabel",
                Some(target.hp),
            );
        } else {
            // Keep the target name area visible, but no invented health label.
            bindings.widget_mut("Target_HP").gauge = Some(0.);
            bindings.widget_mut("Target_HPLabel").visible = Some(false);
            bindings.widget_mut("Target_HPPercLabel").visible = Some(false);
        }
    }
}

fn with_level(name: &str, level: u8) -> String {
    if level == 0 {
        name.to_owned()
    } else {
        format!("{name} (Lv. {level})")
    }
}
fn fraction(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    }
}
fn set_gauge(
    bindings: &mut UiBindings,
    gauge: &str,
    label: &str,
    percent: &str,
    value: Option<f32>,
) {
    for item in [gauge, label, percent] {
        bindings.widget_mut(item).visible = Some(value.is_some());
    }
    if let Some(value) = value {
        let value = fraction(value);
        bindings.widget_mut(gauge).gauge = Some(value);
        bindings.widget_mut(label).text = Some(format!("{:.0}", value * 100.));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_ui::DrawCommand;

    #[test]
    fn unknown_stats_are_hidden_and_values_are_clamped() {
        let mut bindings = UiBindings::default();
        set_gauge(&mut bindings, "mana", "label", "percent", None);
        assert_eq!(bindings.widgets["mana"].visible, Some(false));
        assert_eq!(bindings.widgets["label"].visible, Some(false));
        set_gauge(&mut bindings, "hp", "hp_label", "hp_percent", Some(1.5));
        assert_eq!(bindings.widgets["hp"].gauge, Some(1.));
        assert_eq!(bindings.widgets["hp_label"].text.as_deref(), Some("100"));
        set_gauge(
            &mut bindings,
            "hp",
            "hp_label",
            "hp_percent",
            Some(f32::NAN),
        );
        assert_eq!(bindings.widgets["hp"].gauge, Some(0.));
    }

    #[test]
    #[ignore = "requires original EverQuest UI files; set EQ_UI_DIR or install at ~/EverQuest"]
    fn actual_hud_binds_network_state_without_inactive_indicators() {
        let path = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(path).unwrap();
        let state = HudState {
            character: "Adventurer".into(),
            player_level: 50,
            hp: 0.73,
            target: Some(HudTarget {
                name: "A wandering guard".into(),
                hp: 0.48,
                level: 45,
            }),
            status: "Connected to Plane of Knowledge".into(),
            entities: 87,
            movement_updates: 1234,
            ..Default::default()
        };
        let frame = hud.frame([1280, 720], &state);
        assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
        let texts: Vec<&str> = frame
            .commands
            .iter()
            .filter_map(|command| {
                if let DrawCommand::Text { text, .. } = command {
                    Some(text.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(texts.contains(&"Adventurer (Lv. 50)"));
        assert!(texts.contains(&"A wandering guard (Lv. 45)"));
        assert!(texts.contains(&"73") && texts.contains(&"48"));
        assert!(!texts.contains(&"00:30"));
        assert!(texts.iter().any(
            |text| text.contains("87 zone entities") && text.contains("1234 movement updates")
        ));
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "PW_NewMailIcon" || hit.item == "CW_ChatInput")
        );
        assert_eq!(frame.hit_test([15., 15.]).unwrap().item, "PlayerWindow");
        assert!(frame.hit_test([900., 350.]).is_none());
        let frame = hud.frame([1280, 720], &HudState::default());
        assert!(
            frame
                .commands
                .iter()
                .any(|command| matches!(command,DrawCommand::Text{text,..} if text=="No target"))
        );
    }
}
