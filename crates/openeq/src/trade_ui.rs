//! Original trade-window presentation. Offers, acceptance and completion are
//! supplied by the application; drawing never transfers items or money.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, UiItem, UiMoney, WHITE, position};
use openeq_ui::{HitTarget, Rect};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiTradeStage {
    Invitation,
    Waiting,
    #[default]
    Active,
    Completing,
    /// The server ended the exchange; `status` supplies its outcome. A finish
    /// message alone does not prove success (for example, lore conflicts).
    Completed,
}
#[derive(Clone, Debug, Default)]
pub struct UiTradeSlot {
    /// Explicit offer index 0–7, independent for each participant.
    pub slot: u8,
    pub item: Option<UiItem>,
}
#[derive(Clone, Debug, Default)]
pub struct UiTrade {
    pub partner: String,
    pub stage: UiTradeStage,
    pub own_slots: Vec<UiTradeSlot>,
    pub partner_slots: Vec<UiTradeSlot>,
    pub own_money: UiMoney,
    pub partner_money: UiMoney,
    pub carried_money: Option<UiMoney>,
    pub you_accepted: bool,
    pub partner_accepted: bool,
    /// Wire denomination: copper, silver, gold, platinum.
    pub coin: u8,
    pub quantity: u32,
    pub can_modify: bool,
    pub can_accept: bool,
    pub can_add_coin: bool,
    pub can_remove_coin: bool,
    pub status: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TradeAction {
    AcceptInvite,
    DeclineInvite,
    Accept,
    Cancel,
    OwnSlot(u8),
    InspectPartner(u8),
    Coin(u8),
    Quantity(i32),
    AddCoin,
    RemoveCoin,
}
impl TradeAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        let id = hit.item.as_str();
        for (prefix, partner) in [("trade:own:", false), ("trade:partner:", true)] {
            if let Some(index) = id.strip_prefix(prefix) {
                let index = index.parse::<u8>().ok().filter(|index| *index < 8)?;
                return Some(if partner {
                    Self::InspectPartner(index)
                } else {
                    Self::OwnSlot(index)
                });
            }
        }
        if let Some(coin) = id.strip_prefix("trade:coin:") {
            return coin
                .parse::<u8>()
                .ok()
                .filter(|coin| *coin < 4)
                .map(Self::Coin);
        }
        if let Some(delta) = id.strip_prefix("trade:quantity:") {
            return delta.parse().ok().map(Self::Quantity);
        }
        match id {
            "trade:accept_invite" => Some(Self::AcceptInvite),
            "trade:decline_invite" => Some(Self::DeclineInvite),
            "trade:accept" => Some(Self::Accept),
            "trade:cancel" => Some(Self::Cancel),
            "trade:add_coin" => Some(Self::AddCoin),
            "trade:remove_coin" => Some(Self::RemoveCoin),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UiItemUse {
    pub scribe_label: Option<String>,
    pub use_label: Option<String>,
    pub can_scribe: bool,
    pub can_use: bool,
    pub status: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemUseAction {
    Scribe,
    Use,
}
impl ItemUseAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        match hit.item.as_str() {
            "item:scribe" => Some(Self::Scribe),
            "item:use" => Some(Self::Use),
            _ => None,
        }
    }
}

impl Painter<'_> {
    pub(crate) fn trade(&mut self, state: &GameHudState, trade: &UiTrade) {
        let height = if matches!(
            trade.stage,
            UiTradeStage::Invitation | UiTradeStage::Waiting
        ) {
            255.
        } else {
            488.
        };
        let rect = position(
            state,
            "trade",
            Rect::new(self.screen.width - 980., 110., 618., height),
            self.screen,
        );
        self.shell(
            "TradeWnd",
            "trade",
            rect,
            &format!("Trade with {}", trade.partner),
            true,
        );
        // Override the shell close hit with a typed cancellation action.
        self.hit(
            "trade:cancel",
            "Button",
            Rect::new(rect.right() - 21., rect.y + 3., 16., 16.),
            Some("Close trade".into()),
        );
        if matches!(
            trade.stage,
            UiTradeStage::Invitation | UiTradeStage::Waiting
        ) {
            let invitation = trade.stage == UiTradeStage::Invitation;
            self.text(
                Rect::new(rect.x + 24., rect.y + 48., rect.width - 48., 55.),
                if invitation {
                    format!("{} would like to trade with you.", trade.partner)
                } else {
                    format!(
                        "Waiting for {} to answer your trade request…",
                        trade.partner
                    )
                },
                GOLD,
                true,
            );
            if !trade.status.is_empty() {
                self.text(
                    Rect::new(rect.x + 24., rect.y + 115., rect.width - 48., 64.),
                    &trade.status,
                    WHITE,
                    true,
                );
            }
            if invitation {
                self.button(
                    "TRDW_Trade_Button",
                    "trade:accept_invite",
                    Rect::new(rect.x + 22., rect.y + 193., 276., 28.),
                    "Accept invitation",
                    false,
                );
            }
            self.button(
                "TRDW_Cancel_Button",
                if invitation {
                    "trade:decline_invite"
                } else {
                    "trade:cancel"
                },
                Rect::new(rect.x + 320., rect.y + 193., 276., 28.),
                if invitation {
                    "Decline"
                } else {
                    "Cancel request"
                },
                false,
            );
            return;
        }
        let editable = trade.stage == UiTradeStage::Active && trade.can_modify;
        for (own, x, name, slots, money, accepted) in [
            (
                false,
                rect.x + 18.,
                trade.partner.as_str(),
                &trade.partner_slots,
                trade.partner_money,
                trade.partner_accepted,
            ),
            (
                true,
                rect.x + 320.,
                "Your offer",
                &trade.own_slots,
                trade.own_money,
                trade.you_accepted,
            ),
        ] {
            self.text(Rect::new(x, rect.y + 31., 278., 22.), name, GOLD, false);
            for index in 0..8u8 {
                let item = slots
                    .iter()
                    .find(|slot| slot.slot == index)
                    .and_then(|slot| slot.item.as_ref());
                let bounds = Rect::new(
                    x + f32::from(index % 4) * 52.,
                    rect.y + 63. + f32::from(index / 4) * 48.,
                    44.,
                    44.,
                );
                self.item_slot(
                    &format!("TRDW_TradeSlot{}", index + if own { 0 } else { 8 }),
                    bounds,
                    item,
                    false,
                );
                self.hit_enabled(
                    format!("trade:{}:{index}", if own { "own" } else { "partner" }),
                    if own {
                        "TradeOfferSlot"
                    } else {
                        "TradePartnerSlot"
                    },
                    bounds,
                    item.map(|item| item.name.clone()),
                    if own {
                        editable || (state.cursor_item.is_none() && item.is_some())
                    } else {
                        item.is_some()
                    },
                );
            }
            self.text(
                Rect::new(x, rect.y + 165., 270., 18.),
                "Offered money",
                MUTED,
                false,
            );
            for coin in 0..4u8 {
                let bounds = Rect::new(
                    x + f32::from(coin % 2) * 140.,
                    rect.y + 188. + f32::from(coin / 2) * 30.,
                    132.,
                    24.,
                );
                let label = money.denomination(coin).unwrap().to_string();
                self.button_enabled(
                    &format!("TRDW_{}Money{}", if own { "My" } else { "His" }, 3 - coin),
                    &format!("trade:{}:{coin}", if own { "coin" } else { "partner_coin" }),
                    bounds,
                    &label,
                    own && trade.coin == coin,
                    own && editable,
                );
            }
            let (acceptance, color) = if trade.stage == UiTradeStage::Completed {
                ("Offer closed", MUTED)
            } else if accepted {
                ("Accepted", [125, 225, 145, 255])
            } else {
                ("Not accepted", MUTED)
            };
            self.fill(Rect::new(x, rect.y + 254., 272., 25.), [18, 24, 31, 235]);
            self.text(
                Rect::new(x + 8., rect.y + 258., 256., 18.),
                acceptance,
                color,
                false,
            );
        }
        self.fill(
            Rect::new(rect.x + 305., rect.y + 31., 1., 248.),
            [98, 94, 78, 255],
        );
        if trade.stage == UiTradeStage::Active {
            let denomination = ["copper", "silver", "gold", "platinum"]
                .get(trade.coin as usize)
                .copied()
                .unwrap_or("coin");
            self.text(
                Rect::new(rect.x + 18., rect.y + 293., rect.width - 36., 18.),
                format!("Adjust offered {denomination}"),
                WHITE,
                false,
            );
            let y = rect.y + 320.;
            for (x, delta, label) in [
                (18., -10, "−10"),
                (62., -1, "−"),
                (186., 1, "+"),
                (230., 10, "+10"),
            ] {
                self.button_enabled(
                    "AMP_SitButton",
                    &format!("trade:quantity:{delta}"),
                    Rect::new(rect.x + x, y, 38., 24.),
                    label,
                    false,
                    editable,
                );
            }
            self.text(
                Rect::new(rect.x + 110., y + 4., 72., 18.),
                trade.quantity.max(1).to_string(),
                WHITE,
                false,
            );
            self.button_enabled(
                "TRDW_Trade_Button",
                "trade:add_coin",
                Rect::new(rect.x + 320., y, 132., 24.),
                "Add money",
                false,
                editable && trade.can_add_coin,
            );
            self.text(
                Rect::new(rect.x + 460., y, 138., 32.),
                "Cancel returns offered coins.",
                MUTED,
                true,
            );
            if let Some(money) = trade.carried_money {
                self.text(
                    Rect::new(rect.x + 18., rect.y + 355., rect.width - 36., 18.),
                    format!(
                        "Available: {}p {}g {}s {}c",
                        money.platinum, money.gold, money.silver, money.copper
                    ),
                    GOLD,
                    false,
                );
            }
        }
        let (status, status_color) = match trade.stage {
            UiTradeStage::Completed => (
                if trade.status.is_empty() {
                    "Trade ended. Check your inventory and chat for the result."
                } else {
                    &trade.status
                },
                GOLD,
            ),
            UiTradeStage::Completing => (
                if trade.status.is_empty() {
                    "Waiting for server confirmation…"
                } else {
                    &trade.status
                },
                GOLD,
            ),
            _ if trade.status.is_empty() => (
                "Review both offers before accepting. Cancel returns all offered items and coins.",
                MUTED,
            ),
            _ => (trade.status.as_str(), MUTED),
        };
        self.text(
            Rect::new(rect.x + 18., rect.bottom() - 99., rect.width - 36., 43.),
            status,
            status_color,
            true,
        );
        self.button_enabled(
            "TRDW_Trade_Button",
            "trade:accept",
            Rect::new(rect.x + 18., rect.bottom() - 43., 280., 28.),
            if trade.you_accepted {
                "Accepted"
            } else {
                "Accept trade"
            },
            trade.you_accepted,
            trade.stage == UiTradeStage::Active && trade.can_accept && !trade.you_accepted,
        );
        self.button(
            "TRDW_Cancel_Button",
            "trade:cancel",
            Rect::new(rect.x + 320., rect.bottom() - 43., 278., 28.),
            if trade.stage == UiTradeStage::Completed {
                "Close"
            } else {
                "Cancel"
            },
            false,
        );
    }

    pub(crate) fn item_use_controls(&mut self, item_use: &UiItemUse, rect: Rect) {
        let buttons: Vec<_> = [
            (
                item_use.scribe_label.as_ref(),
                "item:scribe",
                item_use.can_scribe,
            ),
            (item_use.use_label.as_ref(), "item:use", item_use.can_use),
        ]
        .into_iter()
        .filter_map(|(label, id, enabled)| label.map(|label| (label, id, enabled)))
        .collect();
        let width = (rect.width - 24. - 8. * buttons.len().saturating_sub(1) as f32)
            / buttons.len().max(1) as f32;
        for (index, (label, id, enabled)) in buttons.iter().enumerate() {
            self.button_enabled(
                "TRDW_Trade_Button",
                id,
                Rect::new(
                    rect.x + 12. + index as f32 * (width + 8.),
                    rect.bottom() - 69.,
                    width,
                    25.,
                ),
                label,
                false,
                *enabled,
            );
        }
        if !item_use.status.is_empty() {
            self.text(
                Rect::new(rect.x + 12., rect.bottom() - 36., rect.width - 24., 30.),
                &item_use.status,
                MUTED,
                true,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gameplay_ui::{UiAction, UiSlot},
        hud::{Hud, HudState},
    };
    use openeq_ui::{DrawCommand, UiFrame};

    #[test]
    fn offer_addresses_and_item_actions_stay_explicit() {
        let hit = |id: &str| HitTarget {
            item: id.into(),
            screen_id: String::new(),
            kind: String::new(),
            rect: Rect::default(),
            enabled: true,
            tooltip: None,
        };
        assert_eq!(
            UiAction::from_hit(&hit("trade:own:7")),
            Some(UiAction::Trade(TradeAction::OwnSlot(7)))
        );
        assert_eq!(
            UiAction::from_hit(&hit("trade:partner:7")),
            Some(UiAction::Trade(TradeAction::InspectPartner(7)))
        );
        for invalid in [
            "trade:own:8",
            "trade:partner:255",
            "trade:coin:4",
            "trade:own:-1",
        ] {
            assert!(UiAction::from_hit(&hit(invalid)).is_none());
        }
        assert_eq!(
            UiAction::from_hit(&hit("item:scribe")),
            Some(UiAction::ItemUse(ItemUseAction::Scribe))
        );
        assert_eq!(
            UiAction::from_hit(&hit("item:use")),
            Some(UiAction::ItemUse(ItemUseAction::Use))
        );
    }

    fn fixture() -> GameHudState {
        let item = |id, icon, name: &str, count| UiItem {
            id,
            icon,
            name: name.into(),
            count,
            details: vec!["Trade offer".into()],
            bag_slots: 0,
        };
        GameHudState {
            trade: Some(UiTrade {
                partner: "Companion".into(),
                own_slots: vec![
                    UiTradeSlot {
                        slot: 0,
                        item: Some(item(5001, 580, "Short Sword", 1)),
                    },
                    UiTradeSlot {
                        slot: 3,
                        item: Some(item(13005, 570, "Iron Ration", 5)),
                    },
                ],
                partner_slots: vec![
                    UiTradeSlot {
                        slot: 7,
                        item: Some(item(17005, 565, "Backpack", 1)),
                    },
                    UiTradeSlot {
                        slot: 0,
                        item: Some(item(13006, 584, "Water Flask", 5)),
                    },
                ],
                own_money: UiMoney {
                    platinum: 2,
                    copper: 5,
                    ..Default::default()
                },
                partner_money: UiMoney {
                    platinum: 7,
                    gold: 23,
                    silver: 4,
                    copper: 1,
                },
                carried_money: Some(UiMoney {
                    platinum: 12,
                    gold: 8,
                    silver: 6,
                    copper: 4,
                }),
                partner_accepted: true,
                coin: 3,
                quantity: 2,
                can_modify: true,
                can_accept: true,
                can_add_coin: true,
                // Even a stale caller flag must never expose unsupported withdrawals.
                can_remove_coin: true,
                ..Default::default()
            }),
            inventory_open: true,
            inventory: (23..=32)
                .map(|slot| UiSlot {
                    slot,
                    label: format!("Inventory {slot}"),
                    item: (slot == 23).then(|| item(13005, 570, "Iron Ration", 12)),
                })
                .collect(),
            ..Default::default()
        }
    }

    fn capture(frame: &UiFrame, filename: &str) {
        let Some(destination) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") else {
            return;
        };
        let mut renderer = openeq_render::Renderer::new_headless(2560, 1800).unwrap();
        let scene = openeq_assets::Scene::from_geometry("Trade UI".into(), vec![], vec![], vec![]);
        let gpu =
            openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene).unwrap();
        renderer.set_scene(&gpu);
        renderer.set_ui_scaled(frame, 2.);
        renderer.render(&gpu, &openeq_render::Camera::default());
        let (width, height, pixels) = renderer.read_rgba().unwrap();
        let path = std::path::PathBuf::from(destination).join(filename);
        image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
        eprintln!("wrote {}", path.display());
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn original_trade_and_item_use_windows_respect_server_state() {
        let directory = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(directory).unwrap();
        let resources = HudState {
            character: "Broker".into(),
            player_level: 20,
            hp: 1.,
            ..Default::default()
        };
        let mut state = fixture();
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
        for id in [
            "trade:own:0",
            "trade:own:7",
            "trade:partner:0",
            "trade:partner:7",
            "trade:coin:3",
            "trade:add_coin",
            "trade:accept",
            "trade:cancel",
            "game:slot:23",
        ] {
            let hit = frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == id)
                .unwrap_or_else(|| panic!("missing {id}"));
            assert!(hit.enabled, "{id}");
            assert_eq!(
                frame
                    .hit_test([hit.rect.x + 4., hit.rect.y + 4.])
                    .unwrap()
                    .item,
                id
            );
        }
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "trade:remove_coin")
        );
        let partner7 = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "trade:partner:7")
            .unwrap();
        assert_eq!(
            partner7.tooltip.as_deref(),
            Some("Backpack"),
            "slot placement uses explicit offer index, not vector order"
        );
        assert_eq!(state.trade.as_ref().unwrap().own_money.platinum, 2);
        capture(&frame, "openeq-trade-ui.png");

        state.trade.as_mut().unwrap().stage = UiTradeStage::Invitation;
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        for id in ["trade:accept_invite", "trade:decline_invite"] {
            assert!(
                frame
                    .hit_targets
                    .iter()
                    .any(|hit| hit.item == id && hit.enabled)
            );
        }
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item.starts_with("trade:own:"))
        );
        state.trade.as_mut().unwrap().stage = UiTradeStage::Waiting;
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "trade:accept_invite")
        );

        state.trade.as_mut().unwrap().stage = UiTradeStage::Completing;
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        for id in ["trade:accept", "trade:own:7"] {
            let hit = frame.hit_targets.iter().find(|hit| hit.item == id).unwrap();
            assert!(!hit.enabled, "{id}");
            assert_ne!(
                frame
                    .hit_test([hit.rect.x + 2., hit.rect.y + 2.])
                    .unwrap()
                    .item,
                id
            );
        }
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "trade:own:0" && hit.enabled),
            "an offered item remains inspectable while awaiting confirmation"
        );
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text.contains("Waiting for server confirmation"))));
        state.trade.as_mut().unwrap().status =
            "Cancelling trade; waiting for returned items and coins…".into();
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text.contains("Cancelling trade"))));
        assert!(!frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text.contains("Both offers accepted"))));
        let trade = state.trade.as_mut().unwrap();
        trade.stage = UiTradeStage::Completed;
        trade.status = "Trade ended: a lore item prevented the exchange.".into();
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text.contains("lore item prevented"))));
        assert!(!frame.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text.contains("Trade complete"))));

        state.trade = None;
        state.item_use = Some(UiItemUse {
            scribe_label: Some("Scribe spell".into()),
            use_label: None,
            can_scribe: true,
            can_use: false,
            status: "Adds Minor Shielding to your spellbook.".into(),
        });
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item.starts_with("item:")),
            "hover and absent inspection must never expose item actions"
        );
        state.inspected_item = Some(UiItem {
            id: 15288,
            icon: 504,
            name: "Spell: Minor Shielding".into(),
            count: 1,
            details: vec!["Wizard level 1".into(), "Spell: Minor Shielding".into()],
            bag_slots: 0,
        });
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        let scribe = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "item:scribe")
            .unwrap();
        assert!(scribe.enabled);
        assert_eq!(
            frame
                .hit_test([scribe.rect.x + 3., scribe.rect.y + 3.])
                .unwrap()
                .item,
            "item:scribe"
        );
        capture(&frame, "openeq-scribe-ui.png");
        state.cursor_item = state.inspected_item.clone();
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "item:scribe" && hit.enabled),
            "holding an owned scroll must not hide its explicit persistent action"
        );
        state.cursor_item = None;
        state.inspected_item = Some(UiItem {
            id: 47742,
            icon: 1478,
            name: "Dragoncrypt Token".into(),
            count: 1,
            details: vec![
                "Effect: Geomantra".into(),
                "Cast time: 2 seconds".into(),
                "Unlimited charges".into(),
            ],
            bag_slots: 0,
        });
        state.item_use = Some(UiItemUse {
            use_label: Some("Use item".into()),
            can_use: false,
            status: "This item is not ready yet.".into(),
            ..Default::default()
        });
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        let use_item = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "item:use")
            .unwrap();
        assert!(!use_item.enabled);
        assert_ne!(
            frame
                .hit_test([use_item.rect.x + 3., use_item.rect.y + 3.])
                .unwrap()
                .item,
            "item:use"
        );
        state.item_use.as_mut().unwrap().can_use = true;
        state.item_use.as_mut().unwrap().status = "Ready to use.".into();
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        capture(&frame, "openeq-item-use-ui.png");
    }
}
