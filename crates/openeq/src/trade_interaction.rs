//! Player trade intentions. The command boundary owns session validation and
//! lifecycle; these bindings never debit money or predict an item transfer.
use crate::{
    game,
    gameplay_ui::{GameHudState, TradeAction, UiMoney, UiTrade, UiTradeSlot, UiTradeStage},
    interaction::Interaction,
    live::LiveWorld,
    trade::TradePhase,
};
use openeq_net::{
    gameplay::{CoinType, Command, Currency},
    inventory::InventorySlot,
    trade::TradeCommand,
};

fn money(value: Currency) -> UiMoney {
    UiMoney {
        platinum: value.platinum,
        gold: value.gold,
        silver: value.silver,
        copper: value.copper,
    }
}
fn coin_type(value: u8) -> CoinType {
    match value {
        1 => CoinType::Silver,
        2 => CoinType::Gold,
        3 => CoinType::Platinum,
        _ => CoinType::Copper,
    }
}

impl Interaction {
    pub(crate) fn trade_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let Some(session) = &live.game.trade.session else {
            view.trade = None;
            return;
        };
        let can_modify = matches!(session.phase, TradePhase::Active)
            && !session.pending
            && !session.cancel_sent
            && !session.you_accepted
            && !live.game.trade.desynchronized
            && !live.game.inventory_command_pending
            && live.game.commerce.pending.is_none()
            && !live.game.commerce.coin_pending
            && live.ready
            && live.error.is_none()
            && live.trade_player_available(session.partner_id);
        let coin = self.trade_coin.min(3);
        let quantity = self.trade_quantity.max(1).min(i32::MAX as u32);
        let own_money = money(session.own_money);
        let carried = money(live.game.currency);
        view.trade = Some(UiTrade {
            partner: session.partner.clone(),
            stage: match session.phase {
                TradePhase::Invitation => UiTradeStage::Invitation,
                TradePhase::Waiting => UiTradeStage::Waiting,
                TradePhase::Active => UiTradeStage::Active,
                TradePhase::Completing => UiTradeStage::Completing,
                TradePhase::Ended => UiTradeStage::Completed,
            },
            own_slots: (0..8)
                .map(|slot| UiTradeSlot {
                    slot,
                    item: live
                        .game
                        .inventory
                        .items
                        .get(&InventorySlot::trade(u16::from(slot)))
                        .map(game::item_view),
                })
                .collect(),
            partner_slots: session
                .partner_items
                .iter()
                .filter(|(slot, _)| **slot < 8)
                .map(|(&slot, item)| UiTradeSlot {
                    slot,
                    item: Some(game::item_view(item)),
                })
                .collect(),
            own_money,
            partner_money: money(session.partner_money),
            carried_money: live.game.commerce.currency_ready.then_some(carried),
            you_accepted: session.you_accepted,
            partner_accepted: session.partner_accepted,
            coin,
            quantity,
            can_modify,
            can_accept: can_modify
                && !session.you_accepted
                && crate::trade::money_fits(live.game.currency, session.partner_money)
                && crate::trade::money_fits(live.game.currency, session.own_money)
                && !live
                    .game
                    .inventory
                    .items
                    .contains_key(&InventorySlot::CURSOR),
            can_add_coin: can_modify
                && live.game.commerce.currency_ready
                && crate::trade::money_fits(live.game.currency, session.own_money)
                && carried
                    .denomination(coin)
                    .is_some_and(|value| value >= quantity)
                && own_money
                    .denomination(coin)
                    .and_then(|value| value.checked_add(quantity))
                    .is_some_and(|total| total <= i32::MAX as u32),
            can_remove_coin: false,
            status: session.status.clone(),
        });
    }

    pub fn trade_request(&mut self, live: &mut LiveWorld) {
        if live.game.trade.engaged() {
            live.game
                .notice("Finish or cancel the current trade first.");
            return;
        }
        let Some(from_id) = live.own_id else {
            return;
        };
        let Some(to_id) = live.target.filter(|id| *id != from_id).and_then(|id| {
            live.entities
                .get(&id)
                .filter(|entity| !entity.spawn.npc && !entity.spawn.is_corpse)
                .map(|_| id)
        }) else {
            live.game.notice("Select another player to trade with.");
            return;
        };
        if live.command(Command::Trade(TradeCommand::Request { to_id, from_id })) {
            self.inventory_open = true;
            self.trade_coin = 3;
            self.trade_quantity = 1;
        }
    }

    pub fn trade_cancel(&mut self, live: &mut LiveWorld) {
        let Some(session) = &live.game.trade.session else {
            return;
        };
        if matches!(session.phase, TradePhase::Ended) {
            // Dismissing a finished window is local; its escrow was resolved by
            // the server. Do not send another cancellation/refund request.
            live.game.trade.session = None;
            return;
        }
        if session.cancel_sent {
            return;
        }
        let Some(player_id) = live.own_id else {
            return;
        };
        let command = if matches!(session.phase, TradePhase::Invitation) {
            TradeCommand::Busy {
                to_id: session.partner_id,
                from_id: player_id,
            }
        } else {
            TradeCommand::Cancel { player_id }
        };
        live.command(Command::Trade(command));
    }

    pub fn trade_action(&mut self, live: &mut LiveWorld, action: TradeAction) {
        match action {
            TradeAction::AcceptInvite => {
                let Some(session) = live
                    .game
                    .trade
                    .session
                    .as_ref()
                    .filter(|session| matches!(session.phase, TradePhase::Invitation))
                else {
                    return;
                };
                let Some(from_id) = live.own_id else {
                    return;
                };
                let to_id = session.partner_id;
                if live.command(Command::Trade(TradeCommand::Acknowledge { to_id, from_id })) {
                    self.inventory_open = true;
                    self.trade_coin = 3;
                    self.trade_quantity = 1;
                }
            }
            TradeAction::DeclineInvite | TradeAction::Cancel => self.trade_cancel(live),
            TradeAction::Accept => {
                if let Some(player_id) = live.own_id {
                    live.command(Command::Trade(TradeCommand::Accept { player_id }));
                }
            }
            TradeAction::OwnSlot(slot) if slot < 8 => {
                let to = InventorySlot::trade(u16::from(slot));
                if live
                    .game
                    .inventory
                    .items
                    .contains_key(&InventorySlot::CURSOR)
                {
                    live.command(Command::MoveItem {
                        from: InventorySlot::CURSOR,
                        to,
                        count: 0,
                    });
                } else if let Some(item) = live.game.inventory.items.get(&to) {
                    // Offered items cannot be safely retracted individually.
                    self.inspected_item = Some(game::item_view(item));
                    self.inspected_owned = None;
                    live.game.linked_item = None;
                }
            }
            TradeAction::InspectPartner(slot) if slot < 8 => {
                if let Some(item) = live
                    .game
                    .trade
                    .session
                    .as_ref()
                    .and_then(|session| session.partner_items.get(&slot))
                {
                    self.inspected_item = Some(game::item_view(item));
                    self.inspected_owned = None;
                    live.game.linked_item = None;
                }
            }
            TradeAction::Coin(coin) if coin < 4 => self.trade_coin = coin,
            TradeAction::Quantity(delta) => {
                self.trade_quantity = self
                    .trade_quantity
                    .max(1)
                    .saturating_add_signed(delta)
                    .clamp(1, i32::MAX as u32);
            }
            TradeAction::AddCoin => {
                live.command(Command::Trade(TradeCommand::OfferCoin {
                    coin: coin_type(self.trade_coin),
                    amount: self.trade_quantity.max(1).min(i32::MAX as u32),
                }));
            }
            TradeAction::RemoveCoin => {
                live.game
                    .notice("Cancel the trade to return offered items and coins.");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        live::{NetworkCommand, tests::carried_item, tests::command_world},
        trade::TradeSession,
    };
    use tokio::sync::mpsc::UnboundedReceiver;

    fn player_world() -> (LiveWorld, UnboundedReceiver<NetworkCommand>) {
        let (mut live, wire) = command_world(1, 10.);
        live.entities.get_mut(&2).unwrap().spawn.npc = false;
        live.target = Some(2);
        (live, wire)
    }

    fn active_world() -> (LiveWorld, UnboundedReceiver<NetworkCommand>) {
        let (mut live, wire) = player_world();
        live.game.trade.session = Some(TradeSession::new(2, "Partner".into(), TradePhase::Active));
        (live, wire)
    }

    #[test]
    fn request_binds_real_player_ids_and_opens_inventory_only_when_queued() {
        let (mut live, mut wire) = player_world();
        let mut interaction = Interaction::default();
        interaction.trade_request(&mut live);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(
                Command::Trade(TradeCommand::Request {
                    to_id: 2,
                    from_id: 1
                }),
                _
            ))
        ));
        assert!(interaction.inventory_open);
        assert_eq!((interaction.trade_coin, interaction.trade_quantity), (3, 1));
        assert_eq!(
            live.game.trade.session.as_ref().unwrap().phase,
            TradePhase::Waiting
        );
        interaction.trade_request(&mut live);
        assert!(
            wire.try_recv().is_err(),
            "a waiting request cannot be repeated"
        );

        for target in [None, Some(1), Some(99)] {
            let (mut live, mut wire) = player_world();
            live.target = target;
            let mut interaction = Interaction::default();
            interaction.trade_request(&mut live);
            assert!(wire.try_recv().is_err());
            assert!(!interaction.inventory_open);
            assert!(live.game.trade.session.is_none());
        }
        for corpse in [false, true] {
            let (mut live, mut wire) = player_world();
            let spawn = &mut live.entities.get_mut(&2).unwrap().spawn;
            spawn.npc = !corpse;
            spawn.is_corpse = corpse;
            Interaction::default().trade_request(&mut live);
            assert!(wire.try_recv().is_err());
        }
    }

    #[test]
    fn invitation_reply_uses_session_partner_instead_of_changed_target() {
        let (mut live, mut wire) = player_world();
        live.target = Some(99);
        live.game.trade.session = Some(TradeSession::new(
            2,
            "Partner".into(),
            TradePhase::Invitation,
        ));
        let mut interaction = Interaction::default();
        interaction.trade_action(&mut live, TradeAction::AcceptInvite);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(
                Command::Trade(TradeCommand::Acknowledge {
                    to_id: 2,
                    from_id: 1
                }),
                _
            ))
        ));
        assert!(interaction.inventory_open);

        let (mut live, mut wire) = player_world();
        live.game.trade.session = Some(TradeSession::new(
            2,
            "Partner".into(),
            TradePhase::Invitation,
        ));
        interaction.trade_action(&mut live, TradeAction::DeclineInvite);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(
                Command::Trade(TradeCommand::Busy {
                    to_id: 2,
                    from_id: 1
                }),
                _
            ))
        ));
        assert!(live.game.trade.session.is_none());
    }

    #[test]
    fn offered_items_inspect_without_retraction_or_owned_item_actions() {
        let (mut live, mut wire) = active_world();
        let own = InventorySlot::trade(7);
        let mut item = carried_item(own);
        item.name = "Own ration".into();
        live.game.inventory.insert(item);
        let mut partner_item = carried_item(InventorySlot::possessions(2));
        partner_item.name = "Partner ration".into();
        live.game
            .trade
            .session
            .as_mut()
            .unwrap()
            .partner_items
            .insert(2, partner_item);
        let mut interaction = Interaction {
            inspected_owned: Some((InventorySlot::possessions(23), 13005, 99)),
            ..Default::default()
        };
        interaction.trade_action(&mut live, TradeAction::OwnSlot(7));
        assert_eq!(
            interaction.inspected_item.as_ref().unwrap().name,
            "Own ration"
        );
        assert!(interaction.inspected_owned.is_none());
        assert!(live.game.inventory.items.contains_key(&own));
        assert!(
            !live
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
        );

        interaction.inspected_owned = Some((InventorySlot::possessions(23), 13005, 99));
        interaction.trade_action(&mut live, TradeAction::InspectPartner(2));
        assert_eq!(
            interaction.inspected_item.as_ref().unwrap().name,
            "Partner ration"
        );
        assert!(interaction.inspected_owned.is_none());
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn placing_cursor_item_queues_explicit_trade_slot_and_waits_for_transmission() {
        let (mut live, mut wire) = active_world();
        live.game
            .inventory
            .insert(carried_item(InventorySlot::CURSOR));
        let mut interaction = Interaction::default();
        interaction.trade_action(&mut live, TradeAction::OwnSlot(6));
        let Ok(NetworkCommand::Gameplay(Command::MoveItem { from, to, count }, _)) =
            wire.try_recv()
        else {
            panic!("trade slot must queue an item move")
        };
        assert_eq!(from, InventorySlot::CURSOR);
        assert_eq!(to, InventorySlot::trade(6));
        assert_eq!(count, 0);
        assert!(
            live.game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
        );
        assert!(!live.game.inventory.items.contains_key(&to));
        assert!(live.game.inventory_command_pending);
        interaction.trade_action(&mut live, TradeAction::OwnSlot(6));
        assert!(
            wire.try_recv().is_err(),
            "pending placement must not be duplicated"
        );
    }

    #[test]
    fn view_keeps_owned_and_partner_offers_separate_and_disables_unavailable_actions() {
        let (mut live, _wire) = active_world();
        let mut worn = carried_item(InventorySlot::possessions(0));
        worn.id = 100;
        live.game.inventory.insert(worn);
        let mut own = carried_item(InventorySlot::trade(7));
        own.id = 200;
        live.game.inventory.insert(own);
        let mut partner = carried_item(InventorySlot::possessions(0));
        partner.id = 300;
        let session = live.game.trade.session.as_mut().unwrap();
        session.partner_items.insert(0, partner);
        session.own_money.gold = 12;
        session.partner_money.copper = 34;
        session.partner_accepted = true;
        let interaction = Interaction {
            trade_coin: 3,
            trade_quantity: 2,
            ..Default::default()
        };
        let mut view = GameHudState::default();
        interaction.trade_view(&live, &mut view);
        let trade = view.trade.as_ref().unwrap();
        assert_eq!(trade.own_slots.len(), 8);
        assert!(trade.own_slots[0].item.is_none());
        assert_eq!(trade.own_slots[7].item.as_ref().unwrap().id, 200);
        assert_eq!(trade.partner_slots[0].slot, 0);
        assert_eq!(trade.partner_slots[0].item.as_ref().unwrap().id, 300);
        assert_eq!(trade.own_money.gold, 12);
        assert_eq!(trade.partner_money.copper, 34);
        assert!(trade.partner_accepted);
        assert!(trade.can_accept && trade.can_add_coin && trade.can_modify);
        assert!(!trade.can_remove_coin);

        live.game.trade.session.as_mut().unwrap().you_accepted = true;
        interaction.trade_view(&live, &mut view);
        let trade = view.trade.as_ref().unwrap();
        assert!(!trade.can_modify && !trade.can_accept && !trade.can_add_coin);
        live.game.trade.session.as_mut().unwrap().you_accepted = false;

        live.game
            .inventory
            .insert(carried_item(InventorySlot::CURSOR));
        interaction.trade_view(&live, &mut view);
        assert!(!view.trade.as_ref().unwrap().can_accept);
        live.game.inventory.items.remove(&InventorySlot::CURSOR);
        live.game.trade.session.as_mut().unwrap().pending = true;
        interaction.trade_view(&live, &mut view);
        let trade = view.trade.as_ref().unwrap();
        assert!(!trade.can_modify && !trade.can_accept && !trade.can_add_coin);
        live.game.trade.session.as_mut().unwrap().pending = false;
        live.entities.remove(&2);
        interaction.trade_view(&live, &mut view);
        assert!(!view.trade.as_ref().unwrap().can_modify);
    }

    #[test]
    fn adding_coin_does_not_debit_balances_and_cancel_is_not_repeated() {
        let (mut live, mut wire) = active_world();
        let mut interaction = Interaction {
            trade_coin: 3,
            trade_quantity: 4,
            ..Default::default()
        };
        interaction.trade_action(&mut live, TradeAction::AddCoin);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(
                Command::Trade(TradeCommand::OfferCoin {
                    coin: CoinType::Platinum,
                    amount: 4
                }),
                _
            ))
        ));
        assert_eq!(live.game.currency.platinum, 10);
        assert_eq!(
            live.game.trade.session.as_ref().unwrap().own_money.platinum,
            0
        );
        interaction.trade_action(&mut live, TradeAction::AddCoin);
        assert!(wire.try_recv().is_err());
        interaction.trade_action(&mut live, TradeAction::RemoveCoin);
        assert!(
            wire.try_recv().is_err(),
            "individual coin withdrawal is unsupported"
        );

        interaction.trade_cancel(&mut live);
        assert!(matches!(
            wire.try_recv(),
            Ok(NetworkCommand::Gameplay(
                Command::Trade(TradeCommand::Cancel { player_id: 1 }),
                _
            ))
        ));
        interaction.trade_cancel(&mut live);
        assert!(wire.try_recv().is_err());
        live.game.trade.session.as_mut().unwrap().phase = TradePhase::Ended;
        interaction.trade_cancel(&mut live);
        assert!(live.game.trade.session.is_none());
        assert!(
            wire.try_recv().is_err(),
            "dismissing an ended trade is local"
        );
    }
}
