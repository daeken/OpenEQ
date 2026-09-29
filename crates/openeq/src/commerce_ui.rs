//! Merchant and banking presentation. Quotes, balances, stock, slots, and action
//! enablement come from the application/server; drawing never makes transactions.
use crate::gameplay_ui::{GOLD, GameHudState, MUTED, Painter, UiItem, UiSlot, WHITE, position};
use openeq_ui::{HitTarget, Rect, UiBindings};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiMoney {
    pub platinum: u32,
    pub gold: u32,
    pub silver: u32,
    pub copper: u32,
}
impl UiMoney {
    pub fn copper_total(self) -> u64 {
        u64::from(self.platinum) * 1000
            + u64::from(self.gold) * 100
            + u64::from(self.silver) * 10
            + u64::from(self.copper)
    }
    pub fn denomination(self, coin: u8) -> Option<u32> {
        match coin {
            0 => Some(self.copper),
            1 => Some(self.silver),
            2 => Some(self.gold),
            3 => Some(self.platinum),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct UiMerchantStock {
    pub slot: u32,
    pub item: UiItem,
    pub unit_price_copper: Option<u64>,
    pub available: Option<u32>,
}
#[derive(Clone, Debug, Default)]
pub struct UiMerchant {
    pub name: String,
    pub stock: Vec<UiMerchantStock>,
    pub selected_stock: Option<u32>,
    pub sell_slot: Option<UiSlot>,
    /// Estimated total for the selected quantity; the server confirms the sale.
    pub sell_price_copper: Option<u64>,
    pub quantity: u32,
    pub scroll: usize,
    pub can_buy: bool,
    pub can_sell: bool,
    pub busy: bool,
    pub status: String,
}
#[derive(Clone, Debug, Default)]
pub struct UiBank {
    pub name: String,
    pub slots: Vec<UiSlot>,
    pub shared_slots: Vec<UiSlot>,
    pub money: Option<UiMoney>,
    pub shared_platinum: Option<u32>,
    /// 0 copper, 1 silver, 2 gold, 3 platinum (wire denomination, not XML order).
    pub coin: u8,
    pub quantity: u32,
    pub can_deposit: bool,
    pub can_withdraw: bool,
    pub can_deposit_shared: bool,
    pub can_withdraw_shared: bool,
    pub status: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommerceAction {
    SelectStock(u32),
    Quantity(i32),
    SetQuantity(u32),
    Buy,
    Sell,
    Scroll(i32),
    BankCoin(u8),
    BankQuantity(i32),
    Deposit,
    Withdraw,
    DepositShared,
    WithdrawShared,
}
impl CommerceAction {
    pub fn from_hit(hit: &HitTarget) -> Option<Self> {
        let id = hit.item.as_str();
        for (prefix, kind) in [
            ("commerce:quantity:", 0),
            ("commerce:scroll:", 1),
            ("commerce:bank_quantity:", 2),
        ] {
            if let Some(value) = id.strip_prefix(prefix) {
                let value = value.parse().ok()?;
                return Some(match kind {
                    0 => Self::Quantity(value),
                    1 => Self::Scroll(value),
                    _ => Self::BankQuantity(value),
                });
            }
        }
        if let Some(value) = id.strip_prefix("commerce:stock:") {
            return value.parse().ok().map(Self::SelectStock);
        }
        if let Some(value) = id.strip_prefix("commerce:set_quantity:") {
            return value.parse().ok().map(Self::SetQuantity);
        }
        if let Some(value) = id.strip_prefix("commerce:coin:") {
            return value
                .parse()
                .ok()
                .filter(|coin| *coin < 4)
                .map(Self::BankCoin);
        }
        match id {
            "commerce:buy" => Some(Self::Buy),
            "commerce:sell" => Some(Self::Sell),
            "commerce:deposit" => Some(Self::Deposit),
            "commerce:withdraw" => Some(Self::Withdraw),
            "commerce:deposit_shared" => Some(Self::DepositShared),
            "commerce:withdraw_shared" => Some(Self::WithdrawShared),
            _ => None,
        }
    }
}
/// Exact prices use integer copper; no floating-point rounding or guessed quote.
pub fn money_text(value: u64) -> String {
    let mut parts = Vec::new();
    for (amount, label) in [
        (value / 1000, "p"),
        (value / 100 % 10, "g"),
        (value / 10 % 10, "s"),
        (value % 10, "c"),
    ] {
        if amount > 0 {
            parts.push(format!("{amount}{label}"));
        }
    }
    if parts.is_empty() {
        "0c".into()
    } else {
        parts.join(" ")
    }
}
const ROW_HEIGHT: f32 = 35.;
pub const MERCHANT_VISIBLE_ROWS: usize = 8;

impl Painter<'_> {
    pub(crate) fn commerce(&mut self, state: &GameHudState) {
        if let Some(merchant) = &state.merchant {
            self.merchant(state, merchant);
        }
        if let Some(bank) = &state.bank {
            self.bank(state, bank);
        }
    }
    fn merchant(&mut self, state: &GameHudState, merchant: &UiMerchant) {
        let rect = position(
            state,
            "merchant",
            Rect::new(180., 110., 508., 488.),
            self.screen,
        );
        self.shell("MerchantWnd", "merchant", rect, &merchant.name, true);
        self.text(
            Rect::new(rect.x + 12., rect.y + 28., rect.width - 24., 18.),
            format!(
                "Your money: {}",
                state.money.map_or_else(
                    || "Awaiting balance".into(),
                    |money| money_text(money.copper_total())
                )
            ),
            GOLD,
            false,
        );
        self.text(
            Rect::new(rect.x + 51., rect.y + 52., 255., 18.),
            "Item",
            MUTED,
            false,
        );
        self.text(
            Rect::new(rect.x + 327., rect.y + 52., 48., 18.),
            "Stock",
            MUTED,
            false,
        );
        self.text(
            Rect::new(rect.x + 386., rect.y + 52., 104., 18.),
            "Each",
            MUTED,
            false,
        );
        let list = Rect::new(
            rect.x + 10.,
            rect.y + 72.,
            rect.width - 20.,
            ROW_HEIGHT * MERCHANT_VISIBLE_ROWS as f32,
        );
        let mut bindings = UiBindings::default();
        bindings.widget_mut("MW_ItemList").rect = Some(list);
        self.widget("MW_ItemList", &bindings);
        self.hit("commerce:stock_list", "MerchantStockList", list, None);
        let start = merchant
            .scroll
            .min(merchant.stock.len().saturating_sub(MERCHANT_VISIBLE_ROWS));
        for (row, stock) in merchant
            .stock
            .iter()
            .skip(start)
            .take(MERCHANT_VISIBLE_ROWS)
            .enumerate()
        {
            let bounds = Rect::new(
                list.x + 2.,
                list.y + row as f32 * ROW_HEIGHT,
                list.width - 19.,
                ROW_HEIGHT,
            );
            let selected = merchant.selected_stock == Some(stock.slot);
            self.fill(
                bounds,
                if selected {
                    [78, 66, 37, 230]
                } else if row % 2 == 0 {
                    [21, 27, 37, 205]
                } else {
                    [13, 18, 28, 205]
                },
            );
            self.icon(
                &stock.item,
                Rect::new(bounds.x + 3., bounds.y + 2., 30., 30.),
            );
            self.text(
                Rect::new(bounds.x + 40., bounds.y + 9., 267., 18.),
                &stock.item.name,
                if selected { GOLD } else { WHITE },
                false,
            );
            self.text(
                Rect::new(bounds.x + 315., bounds.y + 9., 50., 18.),
                stock
                    .available
                    .map_or_else(|| "∞".into(), |value| value.to_string()),
                MUTED,
                false,
            );
            self.text(
                Rect::new(bounds.x + 374., bounds.y + 9., bounds.width - 375., 18.),
                stock
                    .unit_price_copper
                    .map_or_else(|| "—".into(), money_text),
                WHITE,
                false,
            );
            self.hit_enabled(
                format!("commerce:stock:{}", stock.slot),
                "MerchantStock",
                bounds,
                Some(stock.item.name.clone()),
                !merchant.busy,
            );
        }
        if merchant.stock.is_empty() {
            self.text(
                Rect::new(list.x + 12., list.y + 22., list.width - 24., 35.),
                if merchant.busy {
                    "Waiting for the merchant's stock…"
                } else {
                    "No stock available."
                },
                MUTED,
                true,
            );
        }
        let max_scroll = merchant.stock.len().saturating_sub(MERCHANT_VISIBLE_ROWS);
        self.button_enabled(
            "AMP_SitButton",
            "commerce:scroll:-1",
            Rect::new(list.right() - 17., list.y + 1., 16., 20.),
            "↑",
            false,
            start > 0,
        );
        self.button_enabled(
            "AMP_SitButton",
            "commerce:scroll:1",
            Rect::new(list.right() - 17., list.bottom() - 21., 16., 20.),
            "↓",
            false,
            start < max_scroll,
        );
        if max_scroll > 0 {
            let track = list.height - 46.;
            let thumb =
                (track * MERCHANT_VISIBLE_ROWS as f32 / merchant.stock.len() as f32).max(14.);
            self.fill(
                Rect::new(
                    list.right() - 13.,
                    list.y + 23. + (track - thumb) * start as f32 / max_scroll as f32,
                    8.,
                    thumb,
                ),
                [140, 130, 99, 255],
            );
        }
        let selected = merchant
            .selected_stock
            .and_then(|slot| merchant.stock.iter().find(|stock| stock.slot == slot));
        let selling = merchant
            .sell_slot
            .as_ref()
            .and_then(|slot| slot.item.as_ref());
        let selection = selling.or(selected.map(|stock| &stock.item));
        let quantity = merchant.quantity.max(1);
        let selection_y = list.bottom() + 8.;
        if let Some(item) = selection {
            self.icon(item, Rect::new(rect.x + 12., selection_y, 34., 34.));
        }
        self.text(
            Rect::new(rect.x + 55., selection_y, rect.width - 67., 17.),
            selection.map_or(
                "Select stock to buy, or an inventory item to sell.",
                |item| &item.name,
            ),
            WHITE,
            false,
        );
        let total = if selling.is_some() {
            merchant.sell_price_copper
        } else {
            selected
                .and_then(|stock| stock.unit_price_copper)
                .and_then(|price| price.checked_mul(u64::from(quantity)))
        };
        self.text(
            Rect::new(rect.x + 55., selection_y + 18., rect.width - 67., 17.),
            total.map_or_else(
                || {
                    if selling.is_some() {
                        "Price confirmed by merchant on sale".into()
                    } else {
                        "".into()
                    }
                },
                |value| {
                    format!(
                        "{}: {}",
                        if selling.is_some() {
                            "Estimated total"
                        } else {
                            "Total"
                        },
                        money_text(value)
                    )
                },
            ),
            GOLD,
            false,
        );
        let y = selection_y + 41.;
        self.text(
            Rect::new(rect.x + 12., y + 4., 57., 18.),
            "Quantity",
            MUTED,
            false,
        );
        for (x, delta, label) in [
            (74., -10, "−10"),
            (113., -1, "−"),
            (215., 1, "+"),
            (254., 10, "+10"),
        ] {
            self.button_enabled(
                "AMP_SitButton",
                &format!("commerce:quantity:{delta}"),
                Rect::new(rect.x + x, y, 34., 23.),
                label,
                false,
                !merchant.busy,
            );
        }
        self.text(
            Rect::new(rect.x + 154., y + 4., 59., 18.),
            quantity.to_string(),
            WHITE,
            false,
        );
        self.button_enabled(
            "MW_Buy_Button",
            "commerce:buy",
            Rect::new(rect.x + 304., y, 86., 24.),
            "Buy",
            false,
            merchant.can_buy
                && !merchant.busy
                && selected.is_some()
                && selling.is_none()
                && total.is_some(),
        );
        self.button_enabled(
            "MW_Sell_Button",
            "commerce:sell",
            Rect::new(rect.x + 398., y, 96., 24.),
            "Sell",
            false,
            merchant.can_sell && !merchant.busy && selling.is_some(),
        );
        let status = if merchant.busy {
            "Waiting for the merchant…"
        } else if merchant.status.is_empty() {
            "Mouse wheel scrolls stock · Prices and stock are set by the merchant."
        } else {
            &merchant.status
        };
        self.text(
            Rect::new(rect.x + 12., rect.bottom() - 28., rect.width - 24., 20.),
            status,
            MUTED,
            false,
        );
    }
    fn bank(&mut self, state: &GameHudState, bank: &UiBank) {
        let rect = position(
            state,
            "bank",
            Rect::new(self.screen.width - 780., 110., 418., 460.),
            self.screen,
        );
        self.shell("BankWnd", "bank", rect, &bank.name, true);
        self.text(
            Rect::new(rect.x + 12., rect.y + 28., 180., 18.),
            "Personal bank",
            GOLD,
            false,
        );
        for (index, slot) in bank.slots.iter().take(24).enumerate() {
            let bounds = Rect::new(
                rect.x + 12. + (index % 6) as f32 * 43.,
                rect.y + 51. + (index / 6) as f32 * 43.,
                40.,
                40.,
            );
            self.slot(
                "BW_BankSlot0",
                bounds,
                slot,
                state.selected_slot == Some(slot.slot),
            );
        }
        self.text(
            Rect::new(rect.x + 281., rect.y + 28., 125., 18.),
            "Shared bank",
            GOLD,
            false,
        );
        for (index, slot) in bank.shared_slots.iter().take(2).enumerate() {
            self.slot(
                "BW_SharedBankSlot0",
                Rect::new(rect.x + 283. + index as f32 * 43., rect.y + 51., 40., 40.),
                slot,
                state.selected_slot == Some(slot.slot),
            );
        }
        self.text(
            Rect::new(rect.x + 283., rect.y + 101., 120., 52.),
            "Items in these slots are shared with your other characters.",
            MUTED,
            true,
        );
        self.text(
            Rect::new(rect.x + 12., rect.y + 232., rect.width - 24., 18.),
            format!(
                "Carried: {}",
                state.money.map_or_else(
                    || "Awaiting balance".into(),
                    |money| money_text(money.copper_total())
                )
            ),
            WHITE,
            false,
        );
        self.text(
            Rect::new(rect.x + 12., rect.y + 253., rect.width - 24., 18.),
            "Bank balance · Choose a coin type",
            GOLD,
            false,
        );
        for coin in 0..4u8 {
            let template = format!("BW_Money{}", 3 - coin);
            let label = bank
                .money
                .and_then(|money| money.denomination(coin))
                .map_or_else(|| "—".into(), |value| value.to_string());
            self.button_enabled(
                &template,
                &format!("commerce:coin:{coin}"),
                Rect::new(
                    rect.x + 12. + f32::from(coin) * 99.,
                    rect.y + 277.,
                    94.,
                    24.,
                ),
                &label,
                bank.coin == coin,
                true,
            );
        }
        let y = rect.y + 310.;
        self.text(
            Rect::new(rect.x + 12., y + 4., 56., 18.),
            "Amount",
            MUTED,
            false,
        );
        for (x, delta, label) in [
            (68., -10, "−10"),
            (107., -1, "−"),
            (202., 1, "+"),
            (241., 10, "+10"),
        ] {
            self.button(
                "AMP_SitButton",
                &format!("commerce:bank_quantity:{delta}"),
                Rect::new(rect.x + x, y, 34., 23.),
                label,
                false,
            );
        }
        self.text(
            Rect::new(rect.x + 149., y + 4., 50., 18.),
            bank.quantity.max(1).to_string(),
            WHITE,
            false,
        );
        self.button_enabled(
            "BW_AutoButton",
            "commerce:deposit",
            Rect::new(rect.x + 12., y + 32., 126., 24.),
            "Deposit",
            false,
            bank.can_deposit,
        );
        self.button_enabled(
            "BW_AutoButton",
            "commerce:withdraw",
            Rect::new(rect.x + 145., y + 32., 126., 24.),
            "Withdraw",
            false,
            bank.can_withdraw,
        );
        self.text(
            Rect::new(rect.x + 12., y + 66., rect.width - 24., 18.),
            bank.shared_platinum.map_or_else(
                || "Shared platinum is not enabled on this server.".into(),
                |value| format!("Shared platinum: {value}p"),
            ),
            MUTED,
            false,
        );
        self.button_enabled(
            "BW_AutoButton",
            "commerce:deposit_shared",
            Rect::new(rect.x + 12., y + 90., 126., 24.),
            "Deposit shared",
            false,
            bank.can_deposit_shared && bank.shared_platinum.is_some(),
        );
        self.button_enabled(
            "BW_AutoButton",
            "commerce:withdraw_shared",
            Rect::new(rect.x + 145., y + 90., 126., 24.),
            "Withdraw shared",
            false,
            bank.can_withdraw_shared && bank.shared_platinum.is_some(),
        );
        self.button(
            "BW_DoneButton",
            "game:close:bank",
            Rect::new(rect.right() - 116., y + 90., 104., 24.),
            "Done",
            false,
        );
        if !bank.status.is_empty() {
            self.text(
                Rect::new(rect.x + 12., rect.bottom() - 21., rect.width - 24., 16.),
                &bank.status,
                MUTED,
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gameplay_ui::{ChatLine, UiAction, UiChatLink, UiGroup, UiGroupMember},
        hud::{Hud, HudState},
    };

    #[test]
    fn exact_money_and_explicit_action_addresses() {
        assert_eq!(money_text(0), "0c");
        assert_eq!(money_text(12345), "12p 3g 4s 5c");
        assert_eq!(
            UiMoney {
                platinum: u32::MAX,
                gold: 0,
                silver: 0,
                copper: 0
            }
            .copper_total(),
            u64::from(u32::MAX) * 1000
        );
        let hit = |id: &str| HitTarget {
            window_id: None,
            item: id.into(),
            screen_id: String::new(),
            kind: String::new(),
            rect: Rect::default(),
            enabled: true,
            tooltip: None,
        };
        assert_eq!(
            CommerceAction::from_hit(&hit("commerce:stock:207")),
            Some(CommerceAction::SelectStock(207))
        );
        assert_eq!(
            CommerceAction::from_hit(&hit("commerce:coin:3")),
            Some(CommerceAction::BankCoin(3))
        );
        assert_eq!(CommerceAction::from_hit(&hit("commerce:coin:4")), None);
        assert_eq!(
            CommerceAction::from_hit(&hit("commerce:quantity:-10")),
            Some(CommerceAction::Quantity(-10))
        );
        assert_eq!(
            UiAction::from_hit(&hit("game:slot:2501")),
            Some(UiAction::InventorySlot(2501))
        );
    }

    fn fixture() -> GameHudState {
        let items = [
            (5001, 580, "Short Sword"),
            (13005, 570, "Iron Ration"),
            (13006, 584, "Water Flask"),
            (17005, 565, "Backpack"),
        ];
        let stock: Vec<_> = (0..23)
            .map(|index| {
                let (id, icon, name) = items[index % 4];
                UiMerchantStock {
                    slot: 100 + index as u32,
                    item: UiItem {
                        id,
                        icon,
                        name: name.into(),
                        count: 1,
                        details: vec!["Merchant stock item".into()],
                        bag_slots: if id == 17005 { 8 } else { 0 },
                    },
                    unit_price_copper: Some([1250, 20, 10, 50][index % 4]),
                    available: if index % 4 == 0 { None } else { Some(17) },
                }
            })
            .collect();
        let bank_item = stock[3].item.clone();
        let link_label = "a narrow passage through the ancient ruins beyond the hidden valley and the old bridge that leads toward the mountain";
        let message = format!("Companion says, 'I know of {link_label}. Ask me about it.'");
        let link_start = message.find(link_label).unwrap();
        let mut state = GameHudState {
            chat: vec![ChatLine {
                text: message,
                color: [120, 220, 180, 255],
                links: vec![UiChatLink {
                    range: link_start..link_start + link_label.len(),
                    id: 42,
                }],
            }],
            money: Some(UiMoney {
                platinum: 12,
                gold: 3,
                silver: 2,
                copper: 1,
            }),
            merchant: Some(UiMerchant {
                name: "Falia Frikniller".into(),
                stock,
                selected_stock: Some(102),
                quantity: 5,
                can_buy: true,
                ..Default::default()
            }),
            bank: Some(UiBank {
                name: "Dogle — Bank".into(),
                slots: (0..24)
                    .map(|index| UiSlot {
                        slot: 2000 + index,
                        label: format!("Bank slot {}", index + 1),
                        item: (index == 0).then(|| bank_item.clone()),
                    })
                    .collect(),
                shared_slots: (0..2)
                    .map(|index| UiSlot {
                        slot: 2500 + index,
                        label: format!("Shared bank slot {}", index + 1),
                        item: (index == 0).then(|| bank_item.clone()),
                    })
                    .collect(),
                money: Some(UiMoney {
                    platinum: 100,
                    gold: 23,
                    silver: 4,
                    copper: 5,
                }),
                shared_platinum: Some(1000),
                coin: 3,
                quantity: 25,
                can_deposit: true,
                can_withdraw: true,
                ..Default::default()
            }),
            group: Some(UiGroup {
                leader: "Scout".into(),
                members: vec![
                    UiGroupMember {
                        name: "Scout".into(),
                        level: Some(20),
                        hp: Some(0.75),
                        mana: Some(0.4),
                        in_zone: true,
                        leader: true,
                    },
                    UiGroupMember {
                        name: "Distant Friend".into(),
                        ..Default::default()
                    },
                ],
                invitation: Some("Companion".into()),
            }),
            ..Default::default()
        };
        state.window_positions.extend([
            ("merchant".into(), [30., 130.]),
            ("group".into(), [560., 130.]),
            ("bank".into(), [830., 130.]),
        ]);
        state
    }

    #[test]
    #[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
    fn actual_commerce_and_group_windows_preserve_server_state_and_hits() {
        let directory = std::env::var_os("EQ_UI_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let hud = Hud::load(directory).unwrap();
        let mut state = fixture();
        let resources = HudState {
            character: "Adventurer".into(),
            player_level: 20,
            hp: 0.8,
            mana: Some(0.6),
            ..Default::default()
        };
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(frame.warnings.is_empty(), "{:?}", frame.warnings);
        for id in [
            "commerce:stock:102",
            "commerce:buy",
            "game:slot:2000",
            "game:slot:2023",
            "game:slot:2501",
            "commerce:coin:3",
            "commerce:deposit",
            "social:member:Scout",
            "social:accept",
            "social:decline",
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
        for id in [
            "commerce:sell",
            "commerce:deposit_shared",
            "social:member:Distant Friend",
        ] {
            let hit = frame.hit_targets.iter().find(|hit| hit.item == id).unwrap();
            assert!(!hit.enabled);
            assert_ne!(
                frame
                    .hit_test([hit.rect.x + 4., hit.rect.y + 4.])
                    .unwrap()
                    .item,
                id
            );
        }
        assert_eq!(state.money.unwrap().copper_total(), 12321);
        assert_eq!(
            state.bank.as_ref().unwrap().slots[0]
                .item
                .as_ref()
                .unwrap()
                .id,
            17005
        );
        if let Some(destination) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
            for scale in [1, 2] {
                let mut renderer =
                    openeq_render::Renderer::new_headless(1280 * scale, 900 * scale).unwrap();
                let scene = openeq_assets::Scene::from_geometry(
                    "Commerce UI".into(),
                    vec![],
                    vec![],
                    vec![],
                );
                let gpu =
                    openeq_render::GpuScene::build(renderer.device(), renderer.queue(), &scene)
                        .unwrap();
                renderer.set_scene(&gpu);
                renderer.set_ui_scaled(&frame, scale as f32);
                assert!(renderer.ui_link_hits().len() >= 2);
                for hit in renderer.ui_link_hits() {
                    assert_eq!(UiAction::from_hit(hit), Some(UiAction::ChatLink(42)));
                    assert_eq!(
                        frame
                            .hit_test([hit.rect.x + 1., hit.rect.y + 1.])
                            .unwrap()
                            .item,
                        "game:chat_log"
                    );
                }
                renderer.render(&gpu, &openeq_render::Camera::default());
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                let name = if scale == 1 {
                    "openeq-commerce-ui.png"
                } else {
                    "openeq-commerce-ui-retina.png"
                };
                let path = std::path::PathBuf::from(&destination).join(name);
                image::save_buffer(&path, &pixels, width, height, image::ColorType::Rgba8).unwrap();
                eprintln!("wrote {}", path.display());
            }
        }
        let merchant = state.merchant.as_mut().unwrap();
        merchant.sell_slot = Some(UiSlot {
            slot: 23,
            label: "Sell".into(),
            item: Some(merchant.stock[0].item.clone()),
        });
        merchant.sell_price_copper = Some(250);
        merchant.can_sell = true;
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(frame.commands.iter().any(|command| matches!(command, openeq_ui::DrawCommand::Text { text, .. } if text == "Estimated total: 2g 5s")));
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "commerce:buy" && !hit.enabled)
        );
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "commerce:sell" && hit.enabled)
        );
        let merchant = state.merchant.as_mut().unwrap();
        merchant.sell_slot = None;
        merchant
            .stock
            .iter_mut()
            .for_each(|stock| stock.unit_price_copper = None);
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "commerce:buy" && !hit.enabled)
        );
        let merchant = state.merchant.as_mut().unwrap();
        merchant.busy = true;
        merchant.scroll = 999;
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        assert!(
            frame
                .hit_targets
                .iter()
                .find(|hit| hit.item == "commerce:buy")
                .is_some_and(|hit| !hit.enabled)
        );
        assert!(
            frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "commerce:stock:122")
        );
        assert!(
            !frame
                .hit_targets
                .iter()
                .any(|hit| hit.item == "commerce:stock:100")
        );
        state.window_positions.clear();
        state.merchant = None;
        state.inventory_open = true;
        state.inventory = vec![UiSlot {
            slot: 23,
            label: "Carried slot".into(),
            item: None,
        }];
        let frame = hud.gameplay_frame([1280, 900], &resources, &state);
        let carried = frame
            .hit_targets
            .iter()
            .find(|hit| hit.item == "game:slot:23")
            .unwrap();
        assert_eq!(
            frame
                .hit_test([carried.rect.x + 1., carried.rect.y + 1.])
                .unwrap()
                .item,
            "game:slot:23",
            "the default bank position must not cover carried inventory"
        );
    }
}
