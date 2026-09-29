//! Gameplay state shared by the windowed client and end-to-end probes.
//!
//! EQEmu does not echo successful item moves. The client applies sent moves and
//! replaces them with the server's item/delete packets if validation rejects one.
use crate::gameplay_ui::{ChatLine, UiItem};
use openeq_net::{
    gameplay::{Currency, GameplayEvent, PlayerProfile, ServerMessage},
    inventory::{InventoryItem, InventorySlot},
};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    time::{Duration, Instant},
};

const CHAT_LIMIT: usize = 500;
pub const SYSTEM_COLOR: [u8; 4] = [240, 220, 150, 255];
pub const ERROR_COLOR: [u8; 4] = [255, 125, 110, 255];

#[derive(Default, Debug, Clone, Copy)]
pub struct ResourceValue {
    pub current: Option<u32>,
    pub maximum: Option<u32>,
}
impl ResourceValue {
    pub fn fraction(self) -> Option<f32> {
        let maximum = self.maximum.filter(|m| *m > 0)?;
        Some((self.current? as f32 / maximum as f32).clamp(0., 1.))
    }
}

#[derive(Default)]
pub struct StringTable(BTreeMap<u32, String>);
impl StringTable {
    pub fn load(base: &Path) -> Self {
        let path = ["eqstr_us.txt", "eqstr_en.txt"]
            .into_iter()
            .map(|name| base.join(name))
            .find(|path| path.is_file());
        path.and_then(|path| std::fs::read(path).ok())
            .map(|bytes| Self::parse(&encoding_rs::WINDOWS_1252.decode(&bytes).0))
            .unwrap_or_default()
    }
    pub fn parse(text: &str) -> Self {
        Self(
            text.lines()
                .filter_map(|line| {
                    let (id, text) = line.split_once(' ')?;
                    Some((id.parse().ok()?, text.trim_end().to_owned()))
                })
                .collect(),
        )
    }
    pub fn format(&self, message: &ServerMessage) -> String {
        if let Some(text) = &message.text {
            return text.clone();
        }
        let Some(id) = message.string_id else {
            return String::new();
        };
        let Some(template) = self.0.get(&id) else {
            return format!("Server message {id}: {}", message.arguments.join(" "));
        };
        // Substitute from the template in one pass: percent sequences inside
        // player names or arguments must never become additional substitutions.
        let mut out = String::new();
        let mut chars = template.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '%' {
                match chars.peek().copied() {
                    Some('%') => {
                        chars.next();
                        out.push('%');
                        continue;
                    }
                    Some(digit @ '1'..='9') => {
                        chars.next();
                        if let Some(argument) = message.arguments.get(digit as usize - '1' as usize)
                        {
                            out.push_str(argument);
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            out.push(ch);
        }
        out
    }
}

#[derive(Default)]
pub struct Inventory {
    pub items: BTreeMap<InventorySlot, InventoryItem>,
    pub received: bool,
}
impl Inventory {
    pub fn replace(&mut self, items: Vec<InventoryItem>) {
        self.items.clear();
        for item in items {
            self.insert(item);
        }
        self.received = true;
    }

    pub fn insert(&mut self, mut item: InventoryItem) {
        let children = std::mem::take(&mut item.children);
        let slot = item.slot;
        self.remove_tree(slot);
        self.items.insert(slot, item);
        for child in children {
            self.insert(child);
        }
    }

    fn remove_tree(&mut self, slot: InventorySlot) -> Option<InventoryItem> {
        if slot.bag.is_none() {
            self.items.retain(|key, _| {
                !(key.kind == slot.kind && key.slot == slot.slot && key.bag.is_some())
            });
        }
        self.items.remove(&slot)
    }

    pub fn delete(&mut self, slot: InventorySlot, count: u32) {
        if let Some(item) = self.items.get_mut(&slot)
            && count > 0
            && count < item.count
            && count != u32::MAX
        {
            item.count -= count;
            return;
        }
        self.remove_tree(slot);
    }

    /// Discard ended-session escrow only. Refunded/delivered possession slots
    /// remain whatever the server actually sent; never invent a return slot.
    pub fn clear_trade(&mut self) {
        self.items.retain(|slot, _| slot.kind != 3);
    }

    fn tree_is_tradable(&self, slot: InventorySlot) -> bool {
        fn tradable(item: &InventoryItem) -> bool {
            !item.no_drop && !item.attuned && item.children.iter().all(tradable)
        }
        self.items
            .values()
            .filter(|entry| {
                entry.slot == slot
                    || (slot.bag.is_none()
                        && entry.slot.kind == slot.kind
                        && entry.slot.slot == slot.slot)
            })
            .all(tradable)
    }

    pub fn validate_move(
        &self,
        from: InventorySlot,
        to: InventorySlot,
        count: u32,
    ) -> Result<(), String> {
        if from == to {
            return Err("Choose a different inventory slot.".into());
        }
        if from.kind > 2
            || to.kind > 3
            || from.augment.is_some()
            || to.augment.is_some()
            || from.server_slot().is_none()
            || to.server_slot().is_none()
        {
            return Err("That inventory location is not supported yet.".into());
        }
        let item = self.items.get(&from).ok_or("That slot is empty.")?;
        if count > item.count {
            return Err("That stack does not contain enough items.".into());
        }
        if to.kind == 3 {
            if from != InventorySlot::CURSOR || to.bag.is_some() || count != 0 {
                return Err(
                    "Place the item on your cursor, then offer it in an empty trade slot.".into(),
                );
            }
            if self
                .items
                .keys()
                .any(|key| key.kind == 3 && key.slot == to.slot)
            {
                return Err(
                    "That trade slot is occupied. Cancel the trade to change its items.".into(),
                );
            }
            if !self.tree_is_tradable(from) {
                return Err(
                    "No-trade or attuned items and their containers cannot be offered.".into(),
                );
            }
            return Ok(());
        }
        self.validate_destination(item, to)?;
        if let Some(other) = self.items.get(&to) {
            let stacking = item.stack_size > 1 && item.id == other.id;
            if stacking {
                if other.count >= other.stack_size {
                    return Err("The destination stack is full.".into());
                }
            } else {
                if count > 0 && count < item.count {
                    return Err("Split stacks need an empty slot or matching stack.".into());
                }
                self.validate_destination(other, from)?;
            }
        }
        Ok(())
    }

    fn validate_destination(&self, item: &InventoryItem, to: InventorySlot) -> Result<(), String> {
        if to.kind == 2 && !self.tree_is_tradable(item.slot) {
            return Err("No-trade or attuned items cannot enter the shared bank.".into());
        }
        if let Some(index) = to.bag {
            let parent = InventorySlot { bag: None, ..to };
            if item.slot == parent {
                return Err("A bag cannot be placed inside itself.".into());
            }
            let bag = self
                .items
                .get(&parent)
                .ok_or("That bag is no longer there.")?;
            if index >= bag.bag_slots as u16 {
                return Err("That bag slot does not exist.".into());
            }
            if item.bag_slots > 0 {
                return Err("Containers cannot be put inside other containers.".into());
            }
            if item.size > bag.bag_size {
                return Err("That item is too large for this bag.".into());
            }
        } else if to.kind == 0 && to.slot < 23 && item.equip_slots & (1u32 << to.slot) == 0 {
            return Err("That item cannot be equipped in this slot.".into());
        }
        Ok(())
    }

    /// Apply a sent move or an unsolicited server move. Normal successful moves
    /// have no server echo; corrective item packets subsequently replace state.
    pub fn move_item(
        &mut self,
        from: InventorySlot,
        to: InventorySlot,
        count: u32,
    ) -> Result<(), String> {
        if from.kind == 3 {
            return Err("Cancel the trade to recover offered items.".into());
        }
        if from == to {
            return Ok(());
        }
        if to == InventorySlot::DELETE {
            self.delete(from, count);
            return Ok(());
        }
        self.validate_move(from, to, count)?;
        let mut source = self.items.get(&from).unwrap().clone();
        let amount = if count == 0 {
            source.count
        } else {
            count.min(source.count)
        };
        if let Some(destination) = self.items.get_mut(&to)
            && source.id == destination.id
            && source.stack_size > 1
        {
            let moved = amount.min(destination.stack_size.saturating_sub(destination.count));
            destination.count += moved;
            self.delete(from, moved);
            return Ok(());
        }
        if amount < source.count {
            self.items.get_mut(&from).unwrap().count -= amount;
            source.count = amount;
            source.slot = to;
            self.items.insert(to, source);
            return Ok(());
        }
        let mut source_tree = self.take_tree(from);
        let destination_tree = self.take_tree(to);
        self.put_tree(from, to, &mut source_tree);
        let mut destination_tree = destination_tree;
        self.put_tree(to, from, &mut destination_tree);
        Ok(())
    }

    fn take_tree(&mut self, slot: InventorySlot) -> Vec<InventoryItem> {
        let keys: Vec<_> = self
            .items
            .keys()
            .copied()
            .filter(|key| {
                *key == slot
                    || (slot.bag.is_none()
                        && key.kind == slot.kind
                        && key.slot == slot.slot
                        && key.bag.is_some())
            })
            .collect();
        keys.into_iter()
            .filter_map(|key| self.items.remove(&key))
            .collect()
    }
    fn put_tree(
        &mut self,
        old_slot: InventorySlot,
        slot: InventorySlot,
        items: &mut Vec<InventoryItem>,
    ) {
        for mut item in items.drain(..) {
            let bag = if item.slot == old_slot {
                slot.bag
            } else {
                item.slot.bag
            };
            item.slot = InventorySlot { bag, ..slot };
            self.items.insert(item.slot, item);
        }
    }
}

pub struct LootSession {
    pub corpse_id: u32,
    pub name: String,
    pub opened: bool,
    pub items_complete: bool,
    pub items: BTreeMap<u16, InventoryItem>,
    pub pending: Option<u16>,
    pub take_all: bool,
}

pub struct ActiveCast {
    pub spell_id: u32,
    pub started: Instant,
    pub duration: Duration,
}

#[derive(Default)]
pub struct GameplayState {
    pub recovery: crate::death::RecoveryState,
    pub inventory: Inventory,
    pub commerce: crate::commerce::CommerceState,
    pub trade: crate::trade::TradeState,
    pub item_use: crate::item_use_state::ItemUseState,
    pub group: crate::group::GroupState,
    pub chat: VecDeque<ChatLine>,
    pub chat_links: BTreeMap<u64, openeq_net::social::LinkPayload>,
    next_chat_link: u64,
    pub linked_item: Option<InventoryItem>,
    pub strings: StringTable,
    pub profile: Option<PlayerProfile>,
    pub hp: ResourceValue,
    pub mana: ResourceValue,
    pub endurance: ResourceValue,
    pub currency: Currency,
    pub attack: bool,
    pub sitting: bool,
    pub last_tell: Option<String>,
    pub loot: Option<LootSession>,
    pub inventory_command_pending: bool,
    pub spell_catalog: crate::spells::SpellCatalog,
    pub casting: Option<ActiveCast>,
    pub cast_pending_until: Option<Instant>,
    pub spell_cooldowns: BTreeMap<u8, Instant>,
    pub buffs: BTreeMap<u32, openeq_net::gameplay::Buff>,
    buff_updated: BTreeMap<u32, Instant>,
}
impl GameplayState {
    /// The authoritative buff list, not its display countdown, grants flight.
    /// Wait for the server to fade a buff rather than expiring it locally.
    pub fn levitation_mode(&self) -> Option<u8> {
        self.buffs
            .values()
            .filter_map(|buff| self.spell_catalog.spells.get(&buff.spell_id))
            .filter_map(|spell| spell.levitation_mode)
            // Ordinary levitation is unrestricted by movement, so prefer it
            // when the server has sent both kinds of active effect.
            .min()
    }

    /// Buff contribution only; racial/item water breathing is not decoded yet.
    pub fn has_water_breathing_buff(&self) -> bool {
        self.buffs
            .values()
            .filter_map(|buff| self.spell_catalog.spells.get(&buff.spell_id))
            .any(|spell| spell.water_breathing)
    }

    pub fn buff_seconds(&self, slot: u32) -> Option<u32> {
        let buff = self.buffs.get(&slot)?;
        // Permanent buffs use an unsigned -1 duration. Ordinary durations are
        // server ticks (six seconds); refreshes reset this presentation timer.
        if buff.ticks_remaining == u32::MAX {
            return None;
        }
        let elapsed = self.buff_updated.get(&slot).map_or(0, |time| {
            time.elapsed().as_secs().min(u32::MAX as u64) as u32
        });
        Some(
            buff.ticks_remaining
                .saturating_mul(6)
                .saturating_sub(elapsed),
        )
    }

    pub fn line(&mut self, text: impl Into<String>, color: [u8; 4]) {
        let parsed = crate::chat_links::parse_chat(&text.into());
        if parsed.text.is_empty() {
            return;
        }
        let links = parsed
            .links
            .into_iter()
            .map(|link| {
                self.next_chat_link = self.next_chat_link.wrapping_add(1).max(1);
                let id = self.next_chat_link;
                self.chat_links.insert(id, link.payload);
                crate::gameplay_ui::UiChatLink {
                    id,
                    range: link.range,
                }
            })
            .collect();
        self.chat.push_back(ChatLine {
            text: parsed.text,
            color,
            links,
        });
        while self.chat.len() > CHAT_LIMIT {
            if let Some(line) = self.chat.pop_front() {
                for link in line.links {
                    self.chat_links.remove(&link.id);
                }
            }
        }
    }
    pub fn notice(&mut self, text: impl Into<String>) {
        self.line(text, SYSTEM_COLOR);
    }
    pub fn error(&mut self, text: impl Into<String>) {
        self.line(text, ERROR_COLOR);
    }

    pub fn apply(
        &mut self,
        event: GameplayEvent,
        own: Option<u32>,
        target: Option<u32>,
        name: impl Fn(u32) -> String,
    ) {
        match event {
            GameplayEvent::Social(event) => {
                self.group.apply(event, &own.map(&name).unwrap_or_default());
            }
            GameplayEvent::MerchantOpened {
                merchant_id,
                command,
                rate,
                ..
            } => {
                if let Some(merchant) = &mut self.commerce.merchant
                    && merchant.id == merchant_id
                {
                    merchant.opened = command == 1;
                    merchant.rate = rate;
                    merchant.status = if command == 1 {
                        "Select an item to buy, or select a carried item to sell."
                    } else {
                        "This merchant is unavailable."
                    }
                    .into();
                }
            }
            GameplayEvent::MerchantBought {
                merchant_id,
                player_id,
                slot,
                quantity,
                price,
            } => {
                let matches = self.commerce.pending.as_ref().is_some_and(|pending| {
                    pending.merchant_id == merchant_id && Some(player_id) == own && matches!(pending.kind, crate::commerce::TransactionKind::Buy {slot: expected, quantity: requested} if expected == slot && quantity <= requested)
                });
                if matches {
                    self.commerce.pending = None;
                    if !crate::commerce::debit(&mut self.currency, price) {
                        self.commerce.currency_ready = false;
                        self.error("Merchant purchase confirmed, but money needs synchronization. Reconnect to refresh balances.");
                    }
                    if let Some(merchant) = &mut self.commerce.merchant {
                        merchant.status = format!(
                            "Bought {quantity} for {}.",
                            crate::commerce_ui::money_text(u64::from(price))
                        );
                    }
                }
            }
            GameplayEvent::MerchantSold {
                merchant_id,
                slot,
                quantity,
                price,
                rejected,
            } => {
                let matches = self.commerce.pending.as_ref().is_some_and(|pending| pending.merchant_id == merchant_id && matches!(pending.kind, crate::commerce::TransactionKind::Sell {slot: expected, quantity: requested} if rejected || (expected == slot && quantity <= requested)));
                if matches {
                    self.commerce.pending = None;
                    if !rejected && quantity > 0 {
                        self.inventory.delete(slot, quantity);
                        self.commerce.currency_ready = false;
                    }
                    if let Some(merchant) = &mut self.commerce.merchant {
                        merchant.status = if rejected {
                            "The merchant declined that sale.".into()
                        } else {
                            format!(
                                "Sold {quantity} for {}.",
                                crate::commerce_ui::money_text(u64::from(price))
                            )
                        };
                    }
                }
            }
            GameplayEvent::MerchantItemRemoved {
                merchant_id, slot, ..
            } => {
                if let Some(merchant) = &mut self.commerce.merchant
                    && merchant.id == merchant_id
                {
                    merchant.items.remove(&slot);
                }
            }
            GameplayEvent::MerchantClosed => {
                self.commerce.merchant_closing = false;
                // An unsolicited close can precede our explicit close reply.
                // Ordered delivery guarantees both precede a new open reply.
                if self.commerce.merchant.as_ref().is_some_and(|m| m.opened) {
                    self.commerce.merchant = None;
                }
            }
            GameplayEvent::BankerBalances { carried, bank } => {
                self.currency = carried;
                self.commerce.currency_ready = true;
                self.commerce.bank_money = bank;
            }
            GameplayEvent::SpellMemorized {
                slot,
                spell_id,
                action,
                reduction,
            } => {
                if action == 3
                    && slot < 12
                    && let Some(spell) = self.spell_catalog.spells.get(&spell_id)
                {
                    self.spell_cooldowns.insert(
                        slot as u8,
                        Instant::now()
                            + Duration::from_millis(
                                spell
                                    .recast_ms
                                    .saturating_sub(reduction)
                                    .max(spell.recovery_ms) as u64,
                            ),
                    );
                }
                if let Some(profile) = &mut self.profile {
                    let list = if action == 0 {
                        &mut profile.spell_book
                    } else {
                        &mut profile.memorized_spells
                    };
                    if let Some(entry) = list.get_mut(slot as usize) {
                        match action {
                            0 | 1 => *entry = spell_id,
                            2 => *entry = u32::MAX,
                            _ => {}
                        }
                    }
                }
            }
            GameplayEvent::BeginCast {
                caster_id,
                spell_id,
                cast_time_ms,
            } if Some(caster_id) == own => {
                self.casting = Some(ActiveCast {
                    spell_id,
                    started: Instant::now(),
                    duration: Duration::from_millis(cast_time_ms as u64),
                });
                self.cast_pending_until = None;
            }
            GameplayEvent::CastInterrupted {
                id,
                message,
                string_id,
            } if Some(id) == own || id == 0 => {
                self.casting = None;
                self.cast_pending_until = None;
                if !message.is_empty() {
                    self.notice(message);
                } else {
                    let text = self.strings.format(&ServerMessage {
                        color: 0,
                        string_id: Some(string_id),
                        arguments: Vec::new(),
                        text: None,
                    });
                    self.notice(text);
                }
            }
            GameplayEvent::SpellBarEnabled {
                keep_casting,
                mana,
                endurance,
                ..
            } => {
                self.mana.current = Some(mana);
                self.endurance.current = Some(endurance);
                self.cast_pending_until = None;
                if !keep_casting {
                    self.casting = None;
                }
            }
            GameplayEvent::Buffs { id, all, buffs, .. } if Some(id) == own => {
                if all {
                    self.buffs.clear();
                    self.buff_updated.clear();
                }
                for buff in buffs {
                    if buff.spell_id != u32::MAX && buff.spell_id != 0 {
                        self.buff_updated.insert(buff.slot, Instant::now());
                        self.buffs.insert(buff.slot, buff);
                    } else {
                        self.buff_updated.remove(&buff.slot);
                        self.buffs.remove(&buff.slot);
                    }
                }
            }
            GameplayEvent::BuffChanged { id, buff, removed } if Some(id) == own => {
                if removed || buff.spell_id == 0 || buff.spell_id == u32::MAX {
                    self.buff_updated.remove(&buff.slot);
                    self.buffs.remove(&buff.slot);
                } else {
                    self.buff_updated.insert(buff.slot, Instant::now());
                    self.buffs.insert(buff.slot, buff);
                }
            }
            GameplayEvent::Inventory(items) => self.inventory.replace(items),
            GameplayEvent::Item { packet_type, item } => {
                if packet_type == 0 {
                    self.linked_item = Some(item);
                } else if packet_type == 0x65 {
                    self.trade.remote_item(item);
                } else if packet_type == 0x64 {
                    if let Some(merchant) = &mut self.commerce.merchant
                        && merchant.opened
                    {
                        merchant.items.insert(u32::from(item.slot.slot), item);
                    }
                } else if packet_type == 0x66 {
                    if let Some(loot) = &mut self.loot {
                        loot.items.insert(item.slot.slot, item);
                    }
                } else {
                    self.inventory.insert(item);
                }
            }
            GameplayEvent::ItemMoved { from, to, count } => {
                if let Err(error) = self.inventory.move_item(from, to, count) {
                    self.error(error);
                }
            }
            GameplayEvent::ItemDeleted { from, count } => self.inventory.delete(from, count),
            GameplayEvent::ItemChargeUsed { from, count } => {
                if let Some(item) = self.inventory.items.get_mut(&from)
                    && item.charges >= 0
                {
                    // Non-expendable click items remain when their final charge
                    // is used. Negative charges mean unlimited uses.
                    item.charges = (item.charges as u32).saturating_sub(count) as i32;
                }
            }
            GameplayEvent::Chat(message) => {
                if message.channel == 7
                    && !message.sender.is_empty()
                    && own.is_none_or(|id| !message.sender.eq_ignore_ascii_case(&name(id)))
                {
                    self.last_tell = Some(message.sender.clone());
                }
                let channel = match message.channel {
                    0 => "Guild",
                    2 => "Group",
                    3 => "Shout",
                    4 => "Auction",
                    5 => "OOC",
                    7 | 14 => "Tell",
                    8 => "Say",
                    15 => "Raid",
                    _ => "World",
                };
                let speaker = if message.sender.is_empty() {
                    "You"
                } else {
                    &message.sender
                };
                let recipient = if message.channel == 14 && !message.target.is_empty() {
                    format!(" → {}", message.target)
                } else {
                    String::new()
                };
                self.line(
                    format!("[{channel}] {speaker}{recipient}: {}", message.text),
                    chat_color(message.channel),
                );
            }
            GameplayEvent::Message(message) => {
                self.line(self.strings.format(&message), message_color(message.color));
            }
            GameplayEvent::Profile(profile) => {
                // RoF2 profile resource fields can be placeholders in EQEmu.
                // Only the live resource packets are used for gauge values.
                self.currency = profile.currency;
                self.commerce.currency_ready = true;
                self.commerce.bank_money = profile.bank_currency;
                self.commerce.shared_platinum = profile.shared_platinum;
                self.commerce.cursor_money = profile.cursor_currency;
                self.buffs = profile
                    .buffs
                    .iter()
                    .filter(|buff| buff.spell_id > 0 && buff.spell_id != u32::MAX)
                    .map(|buff| (buff.slot, buff.clone()))
                    .collect();
                let now = Instant::now();
                self.buff_updated = self
                    .buffs
                    .keys()
                    .map(|&slot| (slot, Instant::now()))
                    .collect();
                self.spell_cooldowns = profile
                    .spell_refresh
                    .iter()
                    .take(12)
                    .enumerate()
                    .filter(|(_, ms)| **ms > 0 && **ms < 86_400_000)
                    .map(|(i, ms)| (i as u8, now + Duration::from_millis(*ms as u64)))
                    .collect();
                self.profile = Some(profile);
            }
            GameplayEvent::Health {
                id,
                current,
                maximum,
            } if Some(id) == own => {
                self.hp = ResourceValue {
                    current: Some(current.max(0) as u32),
                    maximum: Some(maximum.max(0) as u32),
                };
            }
            GameplayEvent::Mana { current, maximum } => {
                self.mana.current = Some(current);
                if maximum.is_some() {
                    self.mana.maximum = maximum;
                }
            }
            GameplayEvent::Endurance { current, maximum } => {
                self.endurance.current = Some(current);
                if maximum.is_some() {
                    self.endurance.maximum = maximum;
                }
            }
            GameplayEvent::ManaEndurance { mana, endurance } => {
                self.mana.current = Some(mana);
                self.endurance.current = Some(endurance);
            }
            GameplayEvent::Currency(currency) => {
                self.currency = currency;
                self.commerce.currency_ready = true;
            }
            GameplayEvent::Consider(consider) => {
                let difficulty = match consider.level {
                    6 => "gray",
                    2 => "green",
                    4 => "blue",
                    10 | 20 => "white",
                    13 => "red",
                    15 => "yellow",
                    18 => "light blue",
                    _ => "unknown difficulty",
                };
                // The server swaps apprehensive/scowls and dubious/threatening
                // into the client's ordering in Handle_OP_Consider.
                let attitude = match consider.faction {
                    1 => "ally",
                    2 => "warmly",
                    3 => "kindly",
                    4 => "amiably",
                    5 => "indifferently",
                    6 => "scowls, ready to attack",
                    7 => "threateningly",
                    8 => "apprehensively",
                    9 => "dubiously",
                    _ => "unknown faction",
                };
                self.notice(format!(
                    "{}: {difficulty}; regards you {attitude}.",
                    name(consider.target_id)
                ));
            }
            // Spell effects such as buffs send zero-damage packets. Resists
            // have their own server messages; neither is a melee miss.
            GameplayEvent::Damage(damage)
                if damage.amount == 0 && damage.spell_id > 0 && damage.spell_id < 0xffff => {}
            GameplayEvent::Damage(damage)
                if [Some(damage.source_id), Some(damage.target_id)]
                    .iter()
                    .any(|id| *id == own || *id == target) =>
            {
                let source = if Some(damage.source_id) == own {
                    "You".to_owned()
                } else {
                    name(damage.source_id)
                };
                let target_name = if Some(damage.target_id) == own {
                    "you".to_owned()
                } else {
                    name(damage.target_id)
                };
                let text = match damage.amount {
                    amount if amount > 0 => {
                        format!("{source} hit {target_name} for {amount} damage.")
                    }
                    -1 => format!("{source}'s attack on {target_name} was blocked."),
                    -2 => format!("{source}'s attack on {target_name} was parried."),
                    -3 => format!("{source}'s attack on {target_name} was riposted."),
                    -4 => format!("{source}'s attack on {target_name} was dodged."),
                    -5 => format!("{source}'s attack did not affect {target_name} (invulnerable)."),
                    -6 => format!("{source}'s attack on {target_name} was absorbed by a rune."),
                    _ => format!("{source} miss {target_name}."),
                };
                let color = if Some(damage.target_id) == own {
                    ERROR_COLOR
                } else {
                    [255, 185, 95, 255]
                };
                self.line(text, color);
            }
            GameplayEvent::Death(death) => {
                if Some(death.id) == own || Some(death.id) == target {
                    self.notice(format!(
                        "{} has been slain by {}.",
                        name(death.id),
                        name(death.killer_id)
                    ));
                    self.attack = false;
                }
            }
            GameplayEvent::SpawnAppearance {
                id,
                kind: 14,
                parameter,
            } if Some(id) == own => {
                self.sitting = parameter == 1 || parameter == 110;
            }
            GameplayEvent::LootOpened { response, currency } => {
                if response == 1 || response == 6 {
                    if let Some(loot) = &mut self.loot {
                        loot.opened = true;
                        loot.items_complete |= response == 6;
                    }
                    if currency.platinum + currency.gold + currency.silver + currency.copper > 0 {
                        self.notice(format!(
                            "Looted {} platinum, {} gold, {} silver, {} copper.",
                            currency.platinum, currency.gold, currency.silver, currency.copper
                        ));
                    }
                } else {
                    self.loot = None;
                    self.error(match response {
                        0 => "Someone else is looting that corpse.",
                        2 => "You cannot loot that corpse.",
                        _ => "The corpse could not be opened.",
                    });
                }
            }
            GameplayEvent::LootItemAcknowledged {
                corpse_id,
                player_id,
                slot,
                rejected,
            } => {
                if let Some(loot) = &mut self.loot
                    && loot.corpse_id == corpse_id
                    && own == Some(player_id)
                {
                    if rejected {
                        loot.take_all = false;
                    } else {
                        loot.items.remove(&slot);
                    }
                    loot.pending = None;
                }
            }
            GameplayEvent::LootComplete => self.loot = None,
            _ => {}
        }
    }
}

pub fn display_name(name: &str) -> String {
    name.trim_end_matches(|c: char| c.is_ascii_digit())
        .replace('_', " ")
        .trim_start_matches('#')
        .to_owned()
}
pub fn item_view(item: &InventoryItem) -> UiItem {
    let mut details = Vec::new();
    if item.no_drop {
        details.push("No trade".into());
    }
    if item.attuned {
        details.push("Attuned".into());
    }
    if item.damage > 0 {
        details.push(format!("Damage {}  Delay {}", item.damage, item.delay));
    }
    if item.ac != 0 {
        details.push(format!("AC {:+}", item.ac));
    }
    if item.hp != 0 || item.mana != 0 || item.endurance != 0 {
        details.push(format!(
            "HP {:+}  Mana {:+}  Endurance +{}",
            item.hp, item.mana, item.endurance
        ));
    }
    if item.required_level > 0 {
        details.push(format!("Required level {}", item.required_level));
    }
    if item.bag_slots > 0 {
        details.push(format!("{} container slots", item.bag_slots));
    }
    if item.charges > 0 && item.stack_size <= 1 {
        details.push(format!("{} charges", item.charges));
    }
    details.push(format!("Weight {:.1}", item.weight as f32 / 10.));
    if !item.lore.is_empty() && item.lore != item.name {
        details.push(item.lore.clone());
    }
    UiItem {
        id: item.id,
        icon: item.icon,
        name: item.name.clone(),
        count: item.count,
        details,
        bag_slots: item.bag_slots,
    }
}
pub fn clean_text(text: &str) -> String {
    crate::chat_links::parse_chat(text).text
}

fn chat_color(channel: u32) -> [u8; 4] {
    match channel {
        0 => [145, 245, 145, 255],
        2 => [135, 220, 255, 255],
        7 | 14 => [235, 150, 255, 255],
        3..=5 => [130, 230, 150, 255],
        _ => [235, 235, 235, 255],
    }
}
fn message_color(color: u32) -> [u8; 4] {
    match color {
        13 | 256 | 265 | 267 => ERROR_COLOR,
        10 | 15 => [245, 225, 120, 255],
        2 | 14 => [130, 230, 150, 255],
        _ => SYSTEM_COLOR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(slot: u16, id: u32, count: u32) -> InventoryItem {
        InventoryItem {
            slot: InventorySlot::possessions(slot),
            id,
            instance_id: id,
            name: format!("Item {id}"),
            lore: String::new(),
            id_file: String::new(),
            icon: 500,
            price: 0,
            merchant_count: -1,
            base_price: 0,
            no_drop: false,
            attuned: false,
            count,
            charges: 0,
            stack_size: 20,
            item_class: 0,
            item_type: 0,
            equip_slots: 0x7fffff,
            classes: u32::MAX,
            races: u32::MAX,
            material: 0,
            color: 0,
            damage: 0,
            delay: 0,
            ac: 0,
            hp: 0,
            mana: 0,
            endurance: 0,
            required_level: 0,
            click: Default::default(),
            scroll_spell_id: None,
            recast_timestamp: 0,
            bag_slots: 0,
            bag_size: 4,
            weight: 10,
            size: 1,
            children: Vec::new(),
        }
    }
    #[test]
    fn spell_failure_does_not_start_cooldown_and_confirmed_cast_does() {
        let mut state = GameplayState::default();
        state.spell_catalog.spells.insert(
            288,
            crate::spells::Spell {
                id: 288,
                name: "Minor Shielding".into(),
                projectile_model: String::new(),
                icon: 0,
                mana: 10,
                cast_time_ms: 2500,
                recovery_ms: 1500,
                recast_ms: 9000,
                range: 0.,
                target_type: 6,
                beneficial: true,
                effect_id: 220,
                casting_animation: 43,
                travel_type: 0,
                persistent_particles: false,
                levitation_mode: None,
                water_breathing: false,
                levels: [1; 16],
                description: String::new(),
            },
        );
        let apply = |state: &mut GameplayState, event| {
            state.apply(event, Some(7), None, |id| id.to_string())
        };
        apply(
            &mut state,
            GameplayEvent::BeginCast {
                caster_id: 7,
                spell_id: 288,
                cast_time_ms: 2500,
            },
        );
        assert!(state.casting.is_some());
        apply(
            &mut state,
            GameplayEvent::CastInterrupted {
                id: 7,
                string_id: 439,
                message: "Interrupted".into(),
            },
        );
        apply(
            &mut state,
            GameplayEvent::SpellBarEnabled {
                spell_id: 288,
                slot: 1,
                keep_casting: false,
                mana: 100,
                endurance: 50,
            },
        );
        assert!(state.casting.is_none());
        assert!(state.spell_cooldowns.is_empty());
        apply(
            &mut state,
            GameplayEvent::SpellMemorized {
                spell_id: 288,
                slot: 1,
                action: 3,
                reduction: 1000,
            },
        );
        let remaining = state.spell_cooldowns[&1]
            .duration_since(Instant::now())
            .as_secs_f32();
        assert!((7.9..=8.).contains(&remaining));
        assert_eq!(state.mana.current, Some(100));
    }

    #[test]
    fn partial_buff_removal_and_full_refresh_clear_stale_slots() {
        let mut state = GameplayState::default();
        let buff = |slot, spell_id| openeq_net::gameplay::Buff {
            slot,
            spell_id,
            ticks_remaining: 10,
            num_hits: 0,
            caster: "Caster".into(),
        };
        let apply = |state: &mut GameplayState, all, buffs| {
            state.apply(
                GameplayEvent::Buffs {
                    id: 7,
                    all,
                    tick_timer: 0,
                    kind: 0,
                    buffs,
                },
                Some(7),
                None,
                |id| id.to_string(),
            )
        };
        apply(&mut state, true, vec![buff(0, 288), buff(1, 36)]);
        assert_eq!(state.buff_seconds(0), Some(60));
        state
            .buff_updated
            .insert(0, Instant::now() - Duration::from_secs(8));
        assert_eq!(state.buff_seconds(0), Some(52));
        apply(&mut state, false, vec![buff(1, u32::MAX)]);
        assert!(state.buffs.contains_key(&0));
        assert!(!state.buffs.contains_key(&1));
        apply(&mut state, true, vec![buff(3, 54)]);
        assert_eq!(state.buffs.keys().copied().collect::<Vec<_>>(), vec![3]);
    }

    #[test]
    fn trade_escrow_only_accepts_whole_cursor_items_into_empty_top_slots() {
        let carried = InventorySlot::possessions(23);
        let cursor = InventorySlot::CURSOR;
        let offer = InventorySlot::trade(0);
        let mut inventory = Inventory::default();
        inventory.replace(vec![item(23, 10, 10)]);
        assert!(inventory.move_item(carried, offer, 0).is_err());
        inventory.move_item(carried, cursor, 3).unwrap();
        for (to, count) in [
            (offer, 1),
            (offer, 3),
            (InventorySlot::trade(8), 0),
            (offer.in_bag(0), 0),
            (
                InventorySlot {
                    augment: Some(0),
                    ..offer
                },
                0,
            ),
        ] {
            assert!(inventory.move_item(cursor, to, count).is_err());
            assert_eq!(inventory.items[&cursor].count, 3);
            assert_eq!(inventory.items[&carried].count, 7);
        }
        inventory.move_item(cursor, offer, 0).unwrap();
        assert_eq!(inventory.items[&offer].count, 3);
        assert!(!inventory.items.contains_key(&cursor));
        inventory.move_item(carried, cursor, 2).unwrap();
        // Even an otherwise-compatible stack cannot replace/extend an offer.
        assert!(inventory.move_item(cursor, offer, 0).is_err());
        for to in [cursor, carried, offer.in_bag(0), InventorySlot::DELETE] {
            assert!(inventory.move_item(offer, to, 0).is_err());
        }
        assert_eq!(inventory.items[&cursor].count, 2);
        assert_eq!(inventory.items[&carried].count, 5);
        assert_eq!(inventory.items[&offer].count, 3);
    }

    #[test]
    fn trade_escrow_rejects_bound_items_and_nested_bag_contents() {
        let cursor = InventorySlot::CURSOR;
        for (child_bound, attuned) in [(false, false), (false, true), (true, false), (true, true)] {
            let mut bag = item(33, 10, 1);
            bag.stack_size = 1;
            bag.bag_slots = 8;
            let mut child = item(33, 20, 1);
            child.slot = cursor.in_bag(2);
            let bound = if child_bound { &mut child } else { &mut bag };
            bound.attuned = attuned;
            bound.no_drop = !attuned;
            bag.children.push(child);
            let mut inventory = Inventory::default();
            inventory.replace(vec![bag]);
            assert!(
                inventory
                    .move_item(cursor, InventorySlot::trade(7), 0)
                    .is_err()
            );
            assert_eq!(inventory.items.len(), 2);
            assert_eq!(inventory.items[&cursor.in_bag(2)].id, 20);
        }
        // Public item storage can also contain unflattened metadata; a nested
        // bound child must not bypass the same rule before insertion flattens it.
        let mut outer = item(33, 10, 1);
        let mut inner = item(33, 20, 1);
        let mut bound = item(33, 30, 1);
        bound.attuned = true;
        inner.children.push(bound);
        outer.children.push(inner);
        let mut inventory = Inventory::default();
        inventory.items.insert(cursor, outer);
        assert!(
            inventory
                .validate_move(cursor, InventorySlot::trade(0), 0)
                .is_err()
        );
    }

    #[test]
    fn trade_bag_prediction_and_cleanup_preserve_authoritative_refunds() {
        let cursor = InventorySlot::CURSOR;
        let offer = InventorySlot::trade(7);
        let returned = InventorySlot::possessions(26);
        let mut bag = item(33, 10, 1);
        bag.stack_size = 1;
        bag.bag_slots = 8;
        let mut child = item(33, 20, 5);
        child.slot = cursor.in_bag(2);
        bag.children.push(child);
        let mut inventory = Inventory::default();
        inventory.replace(vec![bag, item(23, 30, 1)]);
        inventory.move_item(cursor, offer, 0).unwrap();
        assert_eq!(inventory.items[&offer].id, 10);
        assert_eq!(inventory.items[&offer.in_bag(2)].count, 5);
        assert!(!inventory.items.contains_key(&cursor));
        assert!(!inventory.items.contains_key(&cursor.in_bag(2)));
        assert!(inventory.move_item(offer.in_bag(2), cursor, 0).is_err());
        // A server-selected refund slot can arrive before window-close. Keep it
        // and all its children while dropping the old separate escrow tree.
        let mut refund = inventory.items[&offer].clone();
        refund.slot = returned;
        let mut refund_child = inventory.items[&offer.in_bag(2)].clone();
        refund_child.slot = returned.in_bag(2);
        refund.children.push(refund_child);
        inventory.insert(refund);
        inventory.clear_trade();
        inventory.clear_trade();
        assert_eq!(inventory.items.len(), 3);
        assert_eq!(inventory.items[&returned].id, 10);
        assert_eq!(inventory.items[&returned.in_bag(2)].count, 5);
        assert_eq!(inventory.items[&InventorySlot::possessions(23)].id, 30);
        assert!(!inventory.items.keys().any(|slot| slot.kind == 3));
    }
    #[test]
    fn split_merge_and_swap_preserve_items() {
        let mut inventory = Inventory::default();
        inventory.replace(vec![item(23, 1, 10), item(24, 2, 1)]);
        inventory
            .move_item(InventorySlot::possessions(23), InventorySlot::CURSOR, 3)
            .unwrap();
        assert_eq!(inventory.items[&InventorySlot::possessions(23)].count, 7);
        assert_eq!(inventory.items[&InventorySlot::CURSOR].count, 3);
        inventory
            .move_item(InventorySlot::CURSOR, InventorySlot::possessions(23), 0)
            .unwrap();
        assert_eq!(inventory.items[&InventorySlot::possessions(23)].count, 10);
        assert!(!inventory.items.contains_key(&InventorySlot::CURSOR));
        inventory
            .move_item(
                InventorySlot::possessions(23),
                InventorySlot::possessions(24),
                0,
            )
            .unwrap();
        assert_eq!(inventory.items[&InventorySlot::possessions(24)].id, 1);
        assert_eq!(inventory.items[&InventorySlot::possessions(23)].id, 2);
    }
    #[test]
    fn bag_moves_relocate_contents_and_server_replacement_clears_old_children() {
        let mut bag = item(23, 10, 1);
        bag.bag_slots = 8;
        bag.stack_size = 1;
        let mut child = item(23, 20, 1);
        child.slot = bag.slot.in_bag(2);
        bag.children.push(child);
        let mut inventory = Inventory::default();
        inventory.replace(vec![bag.clone()]);
        inventory
            .move_item(bag.slot, InventorySlot::CURSOR, 0)
            .unwrap();
        assert_eq!(inventory.items[&InventorySlot::CURSOR.in_bag(2)].id, 20);
        assert!(!inventory.items.contains_key(&bag.slot.in_bag(2)));
        inventory
            .move_item(InventorySlot::CURSOR, bag.slot, 0)
            .unwrap();
        bag.children.clear();
        inventory.insert(bag);
        assert!(
            !inventory
                .items
                .contains_key(&InventorySlot::possessions(23).in_bag(2))
        );
    }
    #[test]
    fn merchant_catalog_and_link_views_never_enter_inventory_and_acks_apply_once() {
        use crate::commerce::{MerchantSession, PendingTransaction, TransactionKind};
        let mut state = GameplayState::default();
        state.currency.platinum = 1;
        state.commerce.merchant = Some(MerchantSession::new(10, "Merchant".into()));
        let apply = |state: &mut GameplayState, event| {
            state.apply(event, Some(1), Some(10), |id| id.to_string())
        };
        apply(
            &mut state,
            GameplayEvent::MerchantOpened {
                merchant_id: 10,
                command: 1,
                rate: 1.03,
                tabs: 1,
            },
        );
        let mut stock = item(5, 13005, 1);
        stock.price = 151;
        stock.merchant_count = -1;
        apply(
            &mut state,
            GameplayEvent::Item {
                packet_type: 0x64,
                item: stock.clone(),
            },
        );
        apply(
            &mut state,
            GameplayEvent::Item {
                packet_type: 0,
                item: stock,
            },
        );
        assert!(state.inventory.items.is_empty());
        assert_eq!(state.commerce.merchant.as_ref().unwrap().items.len(), 1);
        assert_eq!(state.linked_item.as_ref().unwrap().id, 13005);
        state.commerce.pending = Some(PendingTransaction {
            merchant_id: 10,
            kind: TransactionKind::Buy {
                slot: 5,
                quantity: 2,
            },
            started: Instant::now(),
        });
        let ack = |merchant_id| GameplayEvent::MerchantBought {
            merchant_id,
            player_id: 1,
            slot: 5,
            quantity: 2,
            price: 302,
        };
        apply(&mut state, ack(11));
        assert_eq!(crate::commerce::total_copper(state.currency), 1000);
        apply(&mut state, ack(10));
        apply(&mut state, ack(10));
        assert_eq!(crate::commerce::total_copper(state.currency), 698);
        assert!(!state.commerce.currency_ready);
        assert!(state.inventory.items.is_empty());
        apply(
            &mut state,
            GameplayEvent::Item {
                packet_type: 0x67,
                item: item(26, 13005, 2),
            },
        );
        assert_eq!(
            state.inventory.items[&InventorySlot::possessions(26)].count,
            2
        );
        state.commerce.pending = Some(PendingTransaction {
            merchant_id: 10,
            kind: TransactionKind::Sell {
                slot: InventorySlot::possessions(26),
                quantity: 1,
            },
            started: Instant::now(),
        });
        let sold = GameplayEvent::MerchantSold {
            merchant_id: 10,
            slot: InventorySlot::possessions(26),
            quantity: 1,
            price: 147,
            rejected: false,
        };
        apply(&mut state, sold.clone());
        apply(&mut state, sold);
        assert_eq!(
            state.inventory.items[&InventorySlot::possessions(26)].count,
            1
        );
        assert_eq!(crate::commerce::total_copper(state.currency), 698);
        apply(
            &mut state,
            GameplayEvent::Currency(Currency {
                copper: 845,
                ..Default::default()
            }),
        );
        assert_eq!(crate::commerce::total_copper(state.currency), 845);
        assert!(state.commerce.currency_ready);
    }

    #[test]
    fn delayed_merchant_close_and_catalog_do_not_replace_a_new_session() {
        use crate::commerce::MerchantSession;
        let mut state = GameplayState::default();
        state.commerce.merchant_closing = true;
        let apply = |state: &mut GameplayState, event| {
            state.apply(event, Some(1), Some(11), |id| id.to_string())
        };
        // An unsolicited server close may release the barrier before the
        // explicit close's response arrives. The next session is provisional.
        apply(&mut state, GameplayEvent::MerchantClosed);
        assert!(!state.commerce.merchant_closing);
        state.commerce.merchant = Some(MerchantSession::new(11, "Next merchant".into()));
        apply(
            &mut state,
            GameplayEvent::Item {
                packet_type: 0x64,
                item: item(5, 13005, 1),
            },
        );
        apply(&mut state, GameplayEvent::MerchantClosed);
        let merchant = state.commerce.merchant.as_ref().unwrap();
        assert_eq!(merchant.id, 11);
        assert!(merchant.items.is_empty());
        apply(
            &mut state,
            GameplayEvent::MerchantOpened {
                merchant_id: 11,
                command: 1,
                rate: 1.03,
                tabs: 1,
            },
        );
        apply(
            &mut state,
            GameplayEvent::Item {
                packet_type: 0x64,
                item: item(6, 13006, 1),
            },
        );
        assert_eq!(
            state.commerce.merchant.as_ref().unwrap().items[&6].id,
            13006
        );
        // A close after the new open acknowledgment belongs to this session.
        apply(&mut state, GameplayEvent::MerchantClosed);
        assert!(state.commerce.merchant.is_none());
    }

    #[test]
    fn shared_bank_rejects_bound_items_inside_containers() {
        let mut bag = item(23, 10, 1);
        bag.stack_size = 1;
        bag.bag_slots = 8;
        let mut child = item(23, 20, 1);
        child.slot = bag.slot.in_bag(0);
        child.no_drop = true;
        bag.children.push(child);
        let mut inventory = Inventory::default();
        inventory.replace(vec![bag]);
        let from = InventorySlot::possessions(23);
        assert!(
            inventory
                .validate_move(from, InventorySlot::shared_bank(0), 0)
                .is_err()
        );
        assert!(
            inventory
                .validate_move(from, InventorySlot::bank(0), 0)
                .is_ok()
        );
        assert_eq!(inventory.items.len(), 2);
    }

    #[test]
    fn chat_link_actions_expire_with_their_visible_lines() {
        let body = format!("0{:05X}{}", 500, "0".repeat(50));
        let mut state = GameplayState::default();
        for _ in 0..550 {
            state.notice(format!("Hello \u{12}{body}世界\u{12}"));
        }
        assert_eq!(state.chat.len(), 500);
        assert_eq!(state.chat_links.len(), 500);
        let line = state.chat.front().unwrap();
        let link = &line.links[0];
        assert_eq!(&line.text[link.range.clone()], "世界");
        assert!(state.chat_links.contains_key(&link.id));
        assert!(!state.chat_links.contains_key(&1));
    }

    #[test]
    fn banking_moves_preserve_container_children_and_do_not_use_equipment_masks() {
        let mut bag = item(23, 10, 1);
        bag.equip_slots = 0;
        bag.bag_slots = 8;
        bag.stack_size = 1;
        let mut child = item(23, 20, 5);
        child.slot = bag.slot.in_bag(2);
        bag.children.push(child);
        let carried = bag.slot;
        let bank = InventorySlot::from_server_slot(2000).unwrap();
        let shared = InventorySlot::from_server_slot(2500).unwrap();
        let mut inventory = Inventory::default();
        inventory.replace(vec![bag]);
        for (from, to) in [(carried, bank), (bank, shared), (shared, carried)] {
            inventory.move_item(from, to, 0).unwrap();
            assert_eq!(inventory.items[&to].id, 10);
            assert_eq!(inventory.items[&to.in_bag(2)].count, 5);
            assert!(!inventory.items.contains_key(&from.in_bag(2)));
        }
        assert!(
            inventory
                .validate_move(carried, InventorySlot::from_server_slot(3000).unwrap(), 0)
                .is_err()
        );
    }

    #[test]
    fn invalid_moves_do_not_drop_or_duplicate_items() {
        let mut inventory = Inventory::default();
        inventory.replace(vec![item(23, 1, 10)]);
        assert!(
            inventory
                .move_item(
                    InventorySlot::possessions(23),
                    InventorySlot::possessions(24).in_bag(0),
                    0
                )
                .is_err()
        );
        assert!(
            inventory
                .move_item(InventorySlot::possessions(23), InventorySlot::CURSOR, 11)
                .is_err()
        );
        assert_eq!(inventory.items.len(), 1);
        assert_eq!(inventory.items[&InventorySlot::possessions(23)].count, 10);
    }
    #[test]
    fn bag_contents_can_be_picked_up_and_placed_back() {
        let mut bag = item(23, 10, 1);
        bag.bag_slots = 8;
        bag.stack_size = 1;
        let inside = bag.slot.in_bag(2);
        let mut child = item(23, 20, 1);
        child.slot = inside;
        bag.children.push(child);
        let mut inventory = Inventory::default();
        inventory.replace(vec![bag]);
        inventory
            .move_item(inside, InventorySlot::CURSOR, 0)
            .unwrap();
        assert_eq!(inventory.items[&InventorySlot::CURSOR].id, 20);
        assert!(!inventory.items.contains_key(&inside));
        inventory
            .move_item(InventorySlot::CURSOR, inside, 0)
            .unwrap();
        assert_eq!(inventory.items[&inside].id, 20);
        assert!(!inventory.items.contains_key(&InventorySlot::CURSOR));
    }
    #[test]
    fn formatting_does_not_interpret_substitutions_inside_arguments() {
        let table = StringTable::parse("EQST0002\n100 %1 hit %2 for %3 damage (100%%).\n");
        let message = ServerMessage {
            color: 0,
            string_id: Some(100),
            arguments: vec!["a %2 goblin".into(), "you".into(), "12".into()],
            text: None,
        };
        assert_eq!(
            table.format(&message),
            "a %2 goblin hit you for 12 damage (100%)."
        );
    }
    #[test]
    fn logs_are_bounded_and_unknown_resources_hidden() {
        let mut state = GameplayState::default();
        for i in 0..600 {
            state.notice(format!("line {i}"));
        }
        assert_eq!(state.chat.len(), CHAT_LIMIT);
        assert_eq!(state.chat.front().unwrap().text, "line 100");
        assert_eq!(state.mana.fraction(), None);
        state.mana = ResourceValue {
            current: Some(90),
            maximum: Some(100),
        };
        assert_eq!(state.mana.fraction(), Some(0.9));
    }
    #[test]
    fn server_links_show_labels_without_hex_descriptors() {
        let body = "0".repeat(56);
        let text = format!("Choose \u{12}{body}Reload Menu\u{12} or \u{12}{body}help\u{12}.");
        assert_eq!(clean_text(&text), "Choose [Reload Menu] or [help].");
        assert_eq!(clean_text("hello\0\u{12}short\u{12}!"), "helloshort!");
    }
    #[test]
    fn zero_damage_spell_effects_are_not_misses_but_melee_outcomes_remain() {
        let mut state = GameplayState::default();
        let mut damage = openeq_net::gameplay::Damage {
            source_id: 1,
            target_id: 1,
            skill: 0,
            spell_id: 288,
            amount: 0,
            secondary: false,
            special: 0,
        };
        let apply = |state: &mut GameplayState, damage| {
            state.apply(GameplayEvent::Damage(damage), Some(1), None, |id| {
                id.to_string()
            })
        };
        apply(&mut state, damage.clone());
        assert!(state.chat.is_empty());
        damage.spell_id = 0xffff;
        damage.target_id = 2;
        apply(&mut state, damage.clone());
        assert!(state.chat.back().unwrap().text.contains("miss"));
        damage.amount = -3;
        apply(&mut state, damage.clone());
        assert!(state.chat.back().unwrap().text.contains("riposted"));
        damage.spell_id = 54;
        damage.amount = 14;
        apply(&mut state, damage);
        assert!(state.chat.back().unwrap().text.contains("14 damage"));
        // Real resist feedback still travels through ordinary server messages.
        state.apply(
            GameplayEvent::Message(ServerMessage {
                color: 0,
                string_id: None,
                arguments: vec![],
                text: Some("Your target resisted Frost Bolt.".into()),
            }),
            Some(1),
            None,
            |id| id.to_string(),
        );
        assert_eq!(
            state.chat.back().unwrap().text,
            "Your target resisted Frost Bolt."
        );
    }
    #[test]
    fn consumed_stack_units_and_click_charges_keep_remaining_inventory() {
        use openeq_net::gameplay::{Command, encode_command, parse_packet};
        let slot = InventorySlot::possessions(23);
        let packet = encode_command(Command::DeleteItem {
            slot,
            count: u32::MAX,
        })
        .unwrap();
        let mut state = GameplayState::default();
        state.inventory.replace(vec![item(23, 100, 3)]);
        for remaining in [2, 1] {
            state.apply(
                parse_packet(0x18ad, &packet.data).unwrap().unwrap(),
                Some(1),
                None,
                |id| id.to_string(),
            );
            assert_eq!(state.inventory.items[&slot].count, remaining);
        }
        // The final unit is removed with OP_MoveItem, not a charge decrement.
        state.apply(
            parse_packet(0x32ee, &packet.data).unwrap().unwrap(),
            Some(1),
            None,
            |id| id.to_string(),
        );
        assert!(!state.inventory.items.contains_key(&slot));
        let mut click = item(23, 101, 1);
        click.stack_size = 1;
        click.charges = 2;
        state.inventory.replace(vec![click]);
        for remaining in [1, 0] {
            state.apply(
                parse_packet(0x01b8, &packet.data).unwrap().unwrap(),
                Some(1),
                None,
                |id| id.to_string(),
            );
            assert_eq!(state.inventory.items[&slot].charges, remaining);
            assert_eq!(state.inventory.items[&slot].count, 1);
        }
        state.inventory.items.get_mut(&slot).unwrap().charges = -1;
        state.apply(
            parse_packet(0x01b8, &packet.data).unwrap().unwrap(),
            Some(1),
            None,
            |id| id.to_string(),
        );
        assert_eq!(state.inventory.items[&slot].charges, -1);
    }
    #[test]
    fn denied_loot_remains_on_corpse_and_stops_loot_all() {
        use openeq_net::gameplay::{Command, encode_command, parse_packet};
        let mut state = GameplayState {
            loot: Some(LootSession {
                corpse_id: 5,
                name: "Corpse".into(),
                opened: true,
                items_complete: true,
                items: BTreeMap::from([(23, item(23, 100, 1))]),
                pending: Some(23),
                take_all: true,
            }),
            ..Default::default()
        };
        let mut packet = encode_command(Command::LootItem {
            corpse_id: 5,
            player_id: 1,
            slot: 23,
            auto_loot: true,
        })
        .unwrap();
        packet.data[12..16].copy_from_slice(&(-1i32).to_le_bytes());
        state.apply(
            parse_packet(packet.opcode, &packet.data).unwrap().unwrap(),
            Some(1),
            Some(5),
            |id| id.to_string(),
        );
        let loot = state.loot.as_ref().unwrap();
        assert!(loot.items.contains_key(&23));
        assert!(loot.pending.is_none());
        assert!(!loot.take_all);
        // An unrelated corpse acknowledgment must not mutate the open session.
        packet.data[12..16].copy_from_slice(&1u32.to_le_bytes());
        packet.data[..4].copy_from_slice(&6u32.to_le_bytes());
        state.apply(
            parse_packet(packet.opcode, &packet.data).unwrap().unwrap(),
            Some(1),
            Some(5),
            |id| id.to_string(),
        );
        assert!(state.loot.as_ref().unwrap().items.contains_key(&23));
        packet.data[..4].copy_from_slice(&5u32.to_le_bytes());
        state.apply(
            parse_packet(packet.opcode, &packet.data).unwrap().unwrap(),
            Some(1),
            Some(5),
            |id| id.to_string(),
        );
        assert!(state.loot.as_ref().unwrap().items.is_empty());
    }
    #[test]
    fn loot_list_completion_keeps_items_available() {
        let mut state = GameplayState {
            loot: Some(LootSession {
                corpse_id: 5,
                name: "Corpse".into(),
                opened: false,
                items_complete: false,
                items: BTreeMap::new(),
                pending: None,
                take_all: false,
            }),
            ..Default::default()
        };
        for response in [1, 6] {
            state.apply(
                GameplayEvent::LootOpened {
                    response,
                    currency: Currency::default(),
                },
                Some(1),
                Some(5),
                |id| id.to_string(),
            );
        }
        assert!(state.loot.as_ref().unwrap().opened);
        assert!(state.loot.as_ref().unwrap().items_complete);
    }
}
