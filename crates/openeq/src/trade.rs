//! Client trade lifecycle. Escrow is predicted only after transmission; the
//! server's delivery packets and balances remain authoritative on termination.
use crate::{game::display_name, live::LiveWorld};
use openeq_net::{
    gameplay::{CoinType, Command, Currency, GameplayEvent},
    inventory::InventoryItem,
    trade::{TradeCommand, TradeEvent},
};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradePhase {
    Invitation,
    Waiting,
    Active,
    Completing,
    Ended,
}

pub struct TradeSession {
    pub partner_id: u32,
    pub partner: String,
    pub phase: TradePhase,
    pub own_money: Currency,
    pub partner_money: Currency,
    pub partner_items: BTreeMap<u8, InventoryItem>,
    pub you_accepted: bool,
    pub partner_accepted: bool,
    pub pending: bool,
    pub cancel_sent: bool,
    pub status: String,
    close_received: bool,
    cancel_received: bool,
    awaiting_peer_cancel: bool,
    withdrew_request: bool,
}
impl TradeSession {
    pub fn new(partner_id: u32, partner: String, phase: TradePhase) -> Self {
        Self {
            partner_id,
            partner,
            phase,
            own_money: Currency::default(),
            partner_money: Currency::default(),
            partner_items: BTreeMap::new(),
            you_accepted: false,
            partner_accepted: false,
            pending: false,
            cancel_sent: false,
            status: String::new(),
            close_received: false,
            cancel_received: false,
            awaiting_peer_cancel: false,
            withdrew_request: false,
        }
    }
    pub fn offer_changed(&mut self) {
        self.you_accepted = false;
        self.partner_accepted = false;
        if !self.cancel_sent {
            self.status = "Offer changed. Both players must accept again.".into();
        }
    }
    fn finish_cancel(&mut self) {
        if self.close_received && (!self.awaiting_peer_cancel || self.cancel_received) {
            self.phase = TradePhase::Ended;
            self.pending = false;
            self.status = "Trade cancelled. Offered items and coins have been returned.".into();
        }
    }
}

#[derive(Default)]
pub struct TradeState {
    pub session: Option<TradeSession>,
    /// Unexpected acknowledgments can reset EQEmu's server escrow. Only a
    /// reconnect can re-establish ownership and money in that case.
    pub desynchronized: bool,
}
impl TradeState {
    pub fn active(&self) -> bool {
        !self.desynchronized
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.phase == TradePhase::Active && !s.cancel_sent)
    }
    pub fn engaged(&self) -> bool {
        self.desynchronized
            || self
                .session
                .as_ref()
                .is_some_and(|s| s.phase != TradePhase::Ended)
    }
    pub fn remote_item(&mut self, item: InventoryItem) {
        // TradeView serializes remote slots as possessions 0..7, which overlap
        // our worn slots. Never pass this packet to the owned inventory reducer.
        if let Some(s) = &mut self.session
            && matches!(s.phase, TradePhase::Active | TradePhase::Completing)
            && !s.cancel_sent
            && item.slot.kind == 0
            && item.slot.slot < 8
            && item.slot.bag.is_none()
            && item.slot.augment.is_none()
        {
            s.partner_items.insert(item.slot.slot as u8, item);
            s.offer_changed();
            s.phase = TradePhase::Active;
        }
    }
}

pub fn denomination(m: Currency, coin: CoinType) -> u32 {
    match coin {
        CoinType::Copper => m.copper,
        CoinType::Silver => m.silver,
        CoinType::Gold => m.gold,
        CoinType::Platinum => m.platinum,
    }
}
pub fn money_fits(carried: Currency, incoming: Currency) -> bool {
    [
        CoinType::Copper,
        CoinType::Silver,
        CoinType::Gold,
        CoinType::Platinum,
    ]
    .into_iter()
    .all(|coin| {
        u64::from(denomination(carried, coin)) + u64::from(denomination(incoming, coin))
            <= i32::MAX as u64
    })
}
fn balance(m: &mut Currency, coin: CoinType) -> &mut u32 {
    match coin {
        CoinType::Copper => &mut m.copper,
        CoinType::Silver => &mut m.silver,
        CoinType::Gold => &mut m.gold,
        CoinType::Platinum => &mut m.platinum,
    }
}

impl LiveWorld {
    fn trade_alive(&self) -> bool {
        self.game.hp.current != Some(0)
            && self
                .own_id
                .and_then(|id| self.entities.get(&id))
                .is_none_or(|e| !e.spawn.is_corpse)
    }
    pub fn trade_player_available(&self, id: u32) -> bool {
        Some(id) != self.own_id
            && self.entities.get(&id).is_some_and(|e| {
                !e.spawn.npc
                    && !e.spawn.is_corpse
                    && self.player_position().is_some_and(|p| {
                        let q = e.position(Instant::now());
                        p.iter().chain(q.iter()).all(|v| v.is_finite())
                            && p.iter().zip(q).map(|(a, b)| (a - b).powi(2)).sum::<f32>() <= 400.
                    })
            })
    }
    fn trade_free(&self) -> bool {
        !self.game.trade.engaged()
            && self.trade_alive()
            && self.game.commerce.merchant.is_none()
            && !self.game.commerce.merchant_closing
            && self.game.commerce.bank.is_none()
            && self.game.commerce.pending.is_none()
            && !self.game.commerce.coin_pending
            && self.game.loot.is_none()
            && !self.game.inventory_command_pending
            && self.game.casting.is_none()
            && self
                .game
                .cast_pending_until
                .is_none_or(|t| t <= Instant::now())
            && !self.game.attack
            && !self.game.item_use.busy()
    }
    pub(crate) fn trade_command_allowed(&mut self, command: &Command) -> bool {
        let own = self.own_id;
        let session = self.game.trade.session.as_ref();
        let ready = session.is_some_and(|s| {
            s.phase == TradePhase::Active && !s.pending && !s.cancel_sent && !s.you_accepted
        }) && !self.game.inventory_command_pending
            && self.trade_alive()
            && !self.game.trade.desynchronized;
        let valid = match command {
            Command::Trade(TradeCommand::Request { to_id, from_id }) => {
                Some(*from_id) == own && self.trade_free() && self.trade_player_available(*to_id)
            }
            Command::Trade(TradeCommand::Acknowledge { to_id, from_id }) => {
                Some(*from_id) == own
                    && self.trade_alive()
                    && session.is_some_and(|s| {
                        s.partner_id == *to_id && s.phase == TradePhase::Invitation && !s.pending
                    })
                    && self.trade_player_available(*to_id)
            }
            Command::Trade(TradeCommand::Busy { to_id, from_id }) => {
                Some(*from_id) == own && Some(*to_id) != own && *to_id != 0
            }
            Command::Trade(TradeCommand::Cancel { player_id }) => {
                Some(*player_id) == own
                    && session.is_some_and(|s| s.phase != TradePhase::Ended && !s.cancel_sent)
            }
            Command::Trade(TradeCommand::Accept { player_id }) => {
                Some(*player_id) == own
                    && ready
                    && !self
                        .game
                        .inventory
                        .items
                        .contains_key(&openeq_net::inventory::InventorySlot::CURSOR)
                    && session.is_some_and(|s| {
                        !s.you_accepted
                            && self.trade_player_available(s.partner_id)
                            && money_fits(self.game.currency, s.partner_money)
                            && money_fits(self.game.currency, s.own_money)
                    })
            }
            Command::Trade(TradeCommand::OfferCoin { coin, amount }) => {
                ready
                    && self.game.commerce.currency_ready
                    && *amount > 0
                    && *amount <= i32::MAX as u32
                    && denomination(self.game.currency, *coin) >= *amount
                    && session.is_some_and(|s| {
                        money_fits(self.game.currency, s.own_money)
                            && denomination(s.own_money, *coin)
                                .checked_add(*amount)
                                .is_some_and(|total| total <= i32::MAX as u32)
                            && self.trade_player_available(s.partner_id)
                    })
            }
            Command::MoveItem { from, to, .. } if from.kind == 3 || to.kind == 3 => {
                ready && session.is_some_and(|s| self.trade_player_available(s.partner_id))
            }
            Command::DeleteItem { slot, .. } if slot.kind == 3 => false,
            // Their acceptance may have completed the exchange on the server
            // before we see its packets. Freeze our inventory after accepting.
            Command::MoveItem { .. } | Command::DeleteItem { .. } if self.game.trade.engaged() => {
                ready
            }
            Command::MerchantOpen { .. }
            | Command::MerchantBuy { .. }
            | Command::MerchantSell { .. }
            | Command::MoveCoin { .. }
            | Command::BankerChange
            | Command::LootRequest(_)
            | Command::LootItem { .. }
            | Command::ItemUse(_)
            | Command::CastSpell { .. }
            | Command::MemorizeSpell { .. }
            | Command::UnmemorizeSpell { .. }
            | Command::ZoneChange { .. }
            | Command::AutoAttack(true) => !self.game.trade.engaged(),
            _ => true,
        };
        if !valid {
            self.game.error(
                "Trade unavailable: finish the current action and stay near your trading partner.",
            );
        }
        valid
    }
    pub(crate) fn trade_queued(&mut self, command: &Command) {
        let withdrawn = if matches!(command, Command::Trade(TradeCommand::Cancel { .. })) {
            self.game
                .trade
                .session
                .as_ref()
                .filter(|s| s.phase == TradePhase::Waiting)
                .map(|s| s.partner_id)
        } else {
            None
        };
        match command {
            Command::Trade(TradeCommand::Request { to_id, .. }) => {
                let name = self.entities.get(to_id).map_or_else(
                    || format!("Player {to_id}"),
                    |e| display_name(&e.spawn.name),
                );
                let mut s = TradeSession::new(*to_id, name, TradePhase::Waiting);
                s.pending = true;
                self.game.trade.session = Some(s);
            }
            Command::Trade(TradeCommand::Busy { to_id, .. }) => {
                if self
                    .game
                    .trade
                    .session
                    .as_ref()
                    .is_some_and(|s| s.partner_id == *to_id && s.phase == TradePhase::Invitation)
                {
                    self.game.trade.session = None;
                }
            }
            Command::Trade(command) => {
                if let Some(s) = &mut self.game.trade.session {
                    s.pending = true;
                    if matches!(command, TradeCommand::Acknowledge { .. }) {
                        // Once queued, ACK will create escrow before a later
                        // cancellation. Do not treat this as a plain decline.
                        s.phase = TradePhase::Active;
                    }
                    if matches!(command, TradeCommand::Cancel { .. }) {
                        s.withdrew_request |= s.phase == TradePhase::Waiting;
                        s.awaiting_peer_cancel = matches!(
                            s.phase,
                            TradePhase::Active | TradePhase::Completing | TradePhase::Waiting
                        );
                        s.cancel_sent = true;
                        s.phase = TradePhase::Completing;
                        s.status = "Cancelling trade; waiting for returned items and coins…".into();
                    }
                }
            }
            Command::MoveItem { to, .. } if to.kind == 3 => {
                if let Some(s) = &mut self.game.trade.session {
                    s.pending = true;
                }
            }
            _ => {}
        }
        if let (Some(to_id), Some(from_id)) = (withdrawn, self.own_id) {
            // A request does not create server escrow until acknowledged, so
            // Cancel alone would leave the recipient's invitation on screen.
            self.command(Command::Trade(TradeCommand::Busy { to_id, from_id }));
        }
    }
    pub(crate) fn trade_command_sent(&mut self, command: &Command) {
        let Some(s) = &mut self.game.trade.session else {
            return;
        };
        match command {
            Command::Trade(TradeCommand::Request { .. }) => s.pending = false,
            Command::Trade(TradeCommand::Acknowledge { .. }) if !s.cancel_sent => {
                s.phase = TradePhase::Active;
                s.pending = false;
            }
            Command::Trade(TradeCommand::Accept { .. }) if !s.cancel_sent => {
                s.pending = false;
                s.you_accepted = true;
                if s.partner_accepted {
                    s.phase = TradePhase::Completing;
                }
            }
            Command::Trade(TradeCommand::OfferCoin { coin, amount }) => {
                // Only CommandSent changes the local balance. Trade coins have
                // no own echo; cancellation later supplies a full Currency.
                let carried = balance(&mut self.game.currency, *coin);
                *carried = carried.saturating_sub(*amount);
                let offered = balance(&mut s.own_money, *coin);
                *offered = offered.saturating_add(*amount);
                s.pending = false;
                s.offer_changed();
            }
            Command::MoveItem { to, .. } if to.kind == 3 => {
                s.pending = false;
                s.offer_changed();
            }
            _ => {}
        }
    }
    pub(crate) fn trade_command_rejected(&mut self, command: &Command) {
        if matches!(command, Command::Trade(_) | Command::MoveItem { .. })
            && let Some(s) = &mut self.game.trade.session
        {
            s.pending = false;
            if matches!(command, Command::Trade(TradeCommand::Request { .. })) {
                s.phase = TradePhase::Ended;
            }
            if matches!(command, Command::Trade(TradeCommand::Acknowledge { .. })) && !s.cancel_sent
            {
                s.phase = TradePhase::Invitation;
            }
            if matches!(command, Command::Trade(TradeCommand::Cancel { .. })) {
                s.cancel_sent = false;
                s.phase = TradePhase::Active;
            }
        }
    }
    pub(crate) fn trade_event(&mut self, event: &GameplayEvent) {
        let GameplayEvent::Trade(event) = event else {
            return;
        };
        let Some(own) = self.own_id else { return };
        match event {
            TradeEvent::Requested { to_id, from_id } if *to_id == own => {
                if self.trade_free() && self.trade_player_available(*from_id) {
                    let name = self.entities.get(from_id).map_or_else(
                        || format!("Player {from_id}"),
                        |e| display_name(&e.spawn.name),
                    );
                    self.game.notice(format!("{name} would like to trade."));
                    self.game.trade.session =
                        Some(TradeSession::new(*from_id, name, TradePhase::Invitation));
                } else {
                    self.command(Command::Trade(TradeCommand::Busy {
                        to_id: *from_id,
                        from_id: own,
                    }));
                }
            }
            TradeEvent::Acknowledged { to_id, from_id } if *to_id == own => {
                let expected = self.game.trade.session.as_ref().is_some_and(|s| {
                    s.partner_id == *from_id && s.phase == TradePhase::Waiting && !s.cancel_sent
                });
                if expected && self.trade_player_available(*from_id) {
                    if let Some(s) = &mut self.game.trade.session {
                        s.phase = TradePhase::Active;
                        s.pending = false;
                    }
                } else {
                    let unexpected = self.game.trade.session.as_ref().is_some_and(|s| {
                        matches!(s.phase, TradePhase::Active | TradePhase::Completing)
                            && !(s.withdrew_request && s.partner_id == *from_id)
                    });
                    if unexpected {
                        // EQEmu starts both server trade objects before sending
                        // ACK. An unsolicited ACK can reset existing escrow;
                        // continuing this offer would risk further item loss.
                        self.game.trade.desynchronized = true;
                        self.game.commerce.currency_ready = false;
                        self.game.error("Trade state changed unexpectedly. Reconnect before trading or moving items.");
                    }
                    // A delayed acknowledgment can start server escrow after
                    // we cancelled the invitation. Close that session, requiring
                    // a fresh close response and one reciprocal cancellation.
                    self.game.trade.session = Some(TradeSession::new(
                        *from_id,
                        format!("Player {from_id}"),
                        TradePhase::Active,
                    ));
                    self.command(Command::Trade(TradeCommand::Cancel { player_id: own }));
                }
            }
            TradeEvent::Busy { to_id, from_id } if *to_id == own => {
                let reply = self.game.trade.session.as_ref().is_some_and(|s| {
                    s.partner_id == *from_id && s.phase == TradePhase::Invitation && !s.pending
                });
                if let Some(s) = &mut self.game.trade.session
                    && s.partner_id == *from_id
                {
                    if s.withdrew_request && s.cancel_sent {
                        s.cancel_received = true;
                        s.finish_cancel();
                    } else if matches!(s.phase, TradePhase::Waiting | TradePhase::Invitation) {
                        s.phase = TradePhase::Ended;
                        s.pending = false;
                        s.status = "Trade invitation declined or player is busy.".into();
                    }
                }
                if reply {
                    self.command(Command::Trade(TradeCommand::Busy {
                        to_id: *from_id,
                        from_id: own,
                    }));
                }
            }
            TradeEvent::Accepted { player_id } => {
                if let Some(s) = &mut self.game.trade.session
                    && s.partner_id == *player_id
                    && s.phase == TradePhase::Active
                    && !s.cancel_sent
                {
                    s.partner_accepted = true;
                    if s.you_accepted {
                        s.phase = TradePhase::Completing;
                    }
                }
            }
            TradeEvent::CoinsAdded {
                recipient_id,
                coin,
                amount,
            } if *recipient_id == own => {
                if let Some(s) = &mut self.game.trade.session
                    && matches!(s.phase, TradePhase::Active | TradePhase::Completing)
                    && !s.cancel_sent
                {
                    let value = balance(&mut s.partner_money, *coin);
                    *value = value.saturating_add(*amount);
                    s.offer_changed();
                    s.phase = TradePhase::Active;
                }
            }
            TradeEvent::Cancelled { player_id, .. } if *player_id == own => {
                let reply = if let Some(s) = &mut self.game.trade.session {
                    s.cancel_received = true;
                    s.finish_cancel();
                    !s.cancel_sent && s.phase != TradePhase::Ended
                } else {
                    false
                };
                if reply {
                    self.command(Command::Trade(TradeCommand::Cancel { player_id: own }));
                }
            }
            TradeEvent::Finished => {
                if let Some(s) = &mut self.game.trade.session
                    && matches!(s.phase, TradePhase::Active | TradePhase::Completing)
                {
                    s.phase = TradePhase::Ended;
                    s.pending = false;
                    s.status = "Trade ended. Check your inventory and chat for the result.".into();
                    self.game.inventory.clear_trade();
                }
            }
            TradeEvent::WindowClosed2 => {
                if let Some(s) = &mut self.game.trade.session
                    && s.cancel_sent
                {
                    self.game.inventory.clear_trade();
                    s.close_received = true;
                    s.finish_cancel();
                }
            }
            _ => {}
        }
        if self.game.trade.desynchronized
            && let Some(s) = &mut self.game.trade.session
        {
            s.status =
                "Trade state changed unexpectedly. Reconnect before trading or moving items."
                    .into();
        }
    }
    pub(crate) fn trade_partner_gone(&mut self, id: u32) {
        if !self
            .game
            .trade
            .session
            .as_ref()
            .is_some_and(|s| s.partner_id == id && s.phase != TradePhase::Ended)
        {
            return;
        }
        if let Some(own) = self.own_id {
            self.command(Command::Trade(TradeCommand::Cancel { player_id: own }));
        }
        if let Some(s) = &mut self.game.trade.session {
            s.awaiting_peer_cancel = false;
            s.finish_cancel();
        }
    }
    pub(crate) fn trade_tick(&mut self) {
        if let Some(id) = self
            .game
            .trade
            .session
            .as_ref()
            .filter(|s| {
                matches!(
                    s.phase,
                    TradePhase::Active | TradePhase::Invitation | TradePhase::Waiting
                ) && !s.cancel_sent
            })
            .map(|s| s.partner_id)
            && !self.trade_player_available(id)
        {
            if self.entities.contains_key(&id) {
                if let Some(own) = self.own_id {
                    self.command(Command::Trade(TradeCommand::Cancel { player_id: own }));
                }
            } else {
                self.trade_partner_gone(id);
            }
        }
    }
}
