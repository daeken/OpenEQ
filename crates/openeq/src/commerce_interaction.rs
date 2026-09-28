//! NPC service intentions and presentation, using server-owned item addresses.
use crate::{
    commerce::*,
    commerce_ui::*,
    game,
    gameplay_ui::{GameHudState, UiSlot},
    interaction::Interaction,
    live::LiveWorld,
};
use openeq_net::{
    gameplay::{CoinLocation, CoinType, Command, Currency},
    inventory::InventorySlot,
};

fn money(value: Currency) -> UiMoney {
    UiMoney {
        platinum: value.platinum,
        gold: value.gold,
        silver: value.silver,
        copper: value.copper,
    }
}
impl Interaction {
    pub fn open_service(&mut self, live: &mut LiveWorld, expected: Option<u8>) {
        let Some(entity) = live.target.and_then(|id| live.entities.get(&id)) else {
            live.game.notice("Select a merchant or banker first.");
            return;
        };
        let id = entity.spawn.id;
        let class = expected.unwrap_or(entity.spawn.class);
        if !matches!(class, BANKER_CLASS | MERCHANT_CLASS) || !live.service_available(id, class) {
            live.game
                .error("Select a living merchant or banker within reach.");
            return;
        }
        let name = game::display_name(&entity.spawn.name);
        self.inventory_open = true;
        if class == BANKER_CLASS {
            if live.game.commerce.merchant.is_some() {
                self.close_window("merchant", live);
            }
            live.game.commerce.bank = Some(BankSession { id, name });
            self.bank_quantity = 1;
        } else {
            if live
                .game
                .commerce
                .merchant
                .as_ref()
                .is_some_and(|merchant| merchant.id == id)
            {
                return;
            }
            if live.game.commerce.merchant.is_some() {
                live.game
                    .notice("Close the current merchant before opening another.");
                return;
            }
            if live.command(Command::MerchantOpen {
                merchant_id: id,
                player_id: live.own_id.unwrap_or(0),
            }) {
                live.game.commerce.bank = None;
                live.game.commerce.merchant = Some(MerchantSession::new(id, name));
                self.merchant_stock = None;
                self.merchant_sell = None;
                self.merchant_quantity = 1;
                self.merchant_scroll = 0;
            }
        }
    }
    pub(crate) fn commerce_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let state = &live.game.commerce;
        view.money = live
            .game
            .profile
            .as_ref()
            .map(|_| money(live.game.currency));
        let quantity = self.merchant_quantity.max(1);
        view.merchant = state.merchant.as_ref().map(|merchant| {
            let item = self
                .merchant_stock
                .and_then(|slot| merchant.items.get(&slot));
            let sell_item = self
                .merchant_sell
                .and_then(|slot| live.game.inventory.items.get(&slot));
            let available = merchant.opened
                && live.service_available(merchant.id, MERCHANT_CLASS)
                && state.pending.is_none()
                && !state.coin_pending
                && state.currency_ready
                && !live.game.inventory_command_pending;
            UiMerchant {
                name: merchant.name.clone(),
                stock: merchant
                    .items
                    .iter()
                    .map(|(&slot, item)| UiMerchantStock {
                        slot,
                        item: game::item_view(item),
                        unit_price_copper: Some(u64::from(item.price)),
                        available: (item.merchant_count >= 0).then_some(item.merchant_count as u32),
                    })
                    .collect(),
                selected_stock: self.merchant_stock,
                sell_slot: self
                    .merchant_sell
                    .zip(sell_item)
                    .map(|(slot, item)| UiSlot {
                        slot: slot.server_slot().unwrap_or(u32::MAX) as i32,
                        label: "Sell".into(),
                        item: Some(game::item_view(item)),
                    }),
                sell_price_copper: sell_item
                    .filter(|_| merchant.rate.is_finite() && merchant.rate > 0.)
                    .map(|item| {
                        (f64::from(item.base_price) * f64::from(quantity)
                            / f64::from(merchant.rate))
                        .floor()
                        .max(0.) as u64
                    }),
                quantity,
                scroll: self.merchant_scroll,
                can_buy: available
                    && item.is_some_and(|item| {
                        quantity <= item.stack_size.max(1)
                            && (item.merchant_count < 0 || quantity <= item.merchant_count as u32)
                            && u64::from(item.price) * u64::from(quantity)
                                <= total_copper(live.game.currency)
                    })
                    && !live
                        .game
                        .inventory
                        .items
                        .contains_key(&InventorySlot::CURSOR),
                can_sell: available
                    && sell_item.is_some_and(|item| {
                        !item.no_drop
                            && !item.attuned
                            && quantity <= item.count
                            && (item.bag_slots == 0
                                || !live.game.inventory.items.keys().any(|slot| {
                                    slot.kind == item.slot.kind
                                        && slot.slot == item.slot.slot
                                        && slot.bag.is_some()
                                }))
                    }),
                busy: state.pending.is_some(),
                status: state.pending_message().unwrap_or(&merchant.status).into(),
            }
        });
        view.bank = state.bank.as_ref().map(|bank| {
            let slot = |slot: InventorySlot| UiSlot {
                slot: slot.server_slot().unwrap() as i32,
                label: format!("Slot {}", slot.slot + 1),
                item: live.game.inventory.items.get(&slot).map(game::item_view),
            };
            let coin = match self.bank_coin {
                1 => CoinType::Silver,
                2 => CoinType::Gold,
                3 => CoinType::Platinum,
                _ => CoinType::Copper,
            };
            let quantity = self.bank_quantity.max(1);
            let available = live.service_available(bank.id, BANKER_CLASS)
                && !state.coin_pending
                && state.currency_ready
                && state.pending.is_none();
            UiBank {
                name: bank.name.clone(),
                slots: (0..24).map(|id| slot(InventorySlot::bank(id))).collect(),
                shared_slots: (0..2)
                    .map(|id| slot(InventorySlot::shared_bank(id)))
                    .collect(),
                money: Some(money(state.bank_money)),
                shared_platinum: Some(state.shared_platinum),
                coin: self.bank_coin,
                quantity,
                can_deposit: available
                    && state
                        .validate_coin(
                            live.game.currency,
                            CoinLocation::Carried,
                            CoinLocation::Bank,
                            coin,
                            quantity,
                        )
                        .is_ok(),
                can_withdraw: available
                    && state
                        .validate_coin(
                            live.game.currency,
                            CoinLocation::Bank,
                            CoinLocation::Carried,
                            coin,
                            quantity,
                        )
                        .is_ok(),
                can_deposit_shared: available
                    && state
                        .validate_coin(
                            live.game.currency,
                            CoinLocation::Carried,
                            CoinLocation::SharedBank,
                            CoinType::Platinum,
                            quantity,
                        )
                        .is_ok(),
                can_withdraw_shared: available
                    && state
                        .validate_coin(
                            live.game.currency,
                            CoinLocation::SharedBank,
                            CoinLocation::Carried,
                            CoinType::Platinum,
                            quantity,
                        )
                        .is_ok(),
                status: if state.coin_pending {
                    "Moving coins…"
                } else {
                    "Click to move items. Right-click a bank bag to open it."
                }
                .into(),
            }
        });
    }
    pub(crate) fn commerce_action(&mut self, action: CommerceAction, live: &mut LiveWorld) {
        match action {
            CommerceAction::SelectStock(slot) => {
                self.merchant_stock = Some(slot);
                self.merchant_sell = None;
                self.merchant_quantity = 1;
            }
            CommerceAction::Quantity(delta) => {
                self.merchant_quantity = self
                    .merchant_quantity
                    .max(1)
                    .saturating_add_signed(delta)
                    .clamp(1, 1000)
            }
            CommerceAction::SetQuantity(quantity) => {
                self.merchant_quantity = quantity.clamp(1, 1000)
            }
            CommerceAction::Scroll(delta) => {
                self.merchant_scroll = self
                    .merchant_scroll
                    .saturating_add_signed(delta as isize)
                    .min(live.game.commerce.merchant.as_ref().map_or(0, |merchant| {
                        merchant.items.len().saturating_sub(MERCHANT_VISIBLE_ROWS)
                    }))
            }
            CommerceAction::Buy => {
                if let Some(merchant) = &live.game.commerce.merchant
                    && let Some(slot) = self.merchant_stock
                    && let Some(item) = merchant.items.get(&slot)
                    && let Some(price) = item.price.checked_mul(self.merchant_quantity.max(1))
                {
                    live.command(Command::MerchantBuy {
                        merchant_id: merchant.id,
                        player_id: live.own_id.unwrap_or(0),
                        slot,
                        quantity: self.merchant_quantity.max(1),
                        price,
                    });
                }
            }
            CommerceAction::Sell => {
                if let (Some(merchant), Some(slot)) =
                    (&live.game.commerce.merchant, self.merchant_sell)
                {
                    live.command(Command::MerchantSell {
                        merchant_id: merchant.id,
                        slot,
                        quantity: self.merchant_quantity.max(1),
                    });
                }
            }
            CommerceAction::BankCoin(coin) => self.bank_coin = coin.min(3),
            CommerceAction::BankQuantity(delta) => {
                self.bank_quantity = self
                    .bank_quantity
                    .max(1)
                    .saturating_add_signed(delta)
                    .clamp(1, 1_000_000)
            }
            transfer @ (CommerceAction::Deposit
            | CommerceAction::Withdraw
            | CommerceAction::DepositShared
            | CommerceAction::WithdrawShared) => {
                let (from, to) = match transfer {
                    CommerceAction::Deposit => (CoinLocation::Carried, CoinLocation::Bank),
                    CommerceAction::Withdraw => (CoinLocation::Bank, CoinLocation::Carried),
                    CommerceAction::DepositShared => {
                        (CoinLocation::Carried, CoinLocation::SharedBank)
                    }
                    _ => (CoinLocation::SharedBank, CoinLocation::Carried),
                };
                let coin = if matches!(from, CoinLocation::SharedBank)
                    || matches!(to, CoinLocation::SharedBank)
                {
                    CoinType::Platinum
                } else {
                    match self.bank_coin {
                        1 => CoinType::Silver,
                        2 => CoinType::Gold,
                        3 => CoinType::Platinum,
                        _ => CoinType::Copper,
                    }
                };
                live.command(Command::MoveCoin {
                    from,
                    to,
                    coin,
                    amount: self.bank_quantity.max(1),
                });
            }
        }
    }
}
