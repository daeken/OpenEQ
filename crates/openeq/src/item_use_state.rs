//! Pending item intentions and server-confirmed item reuse timers.
use crate::game::GameplayState;
use openeq_net::{
    gameplay::{Command, GameplayEvent},
    inventory::{InventoryItem, InventorySlot},
    item_use::{ItemUseCommand, ItemUseEvent, supports_click},
};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedItem {
    pub slot: InventorySlot,
    pub id: u32,
    pub instance_id: u32,
}
impl From<(InventorySlot, u32, u32)> for OwnedItem {
    fn from((slot, id, instance_id): (InventorySlot, u32, u32)) -> Self {
        Self {
            slot,
            id,
            instance_id,
        }
    }
}
impl OwnedItem {
    pub fn matches(&self, item: &InventoryItem) -> bool {
        item.slot == self.slot && item.id == self.id && item.instance_id == self.instance_id
    }
    pub(crate) fn on_cursor(self) -> Self {
        Self {
            slot: InventorySlot::CURSOR,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScribePhase {
    Moving,
    Queued,
    Sending,
    Waiting,
}

#[derive(Clone, Debug)]
pub(crate) enum PendingKind {
    Scribe {
        spell_id: u32,
        book_slot: u32,
        phase: ScribePhase,
        book_confirmed: bool,
        consumed: bool,
    },
    Click {
        spell_id: u32,
        target_id: u32,
        queued: bool,
        sent: bool,
        began: bool,
        applied: bool,
        finished: bool,
        consumed: bool,
    },
}
#[derive(Clone, Debug)]
pub(crate) struct PendingUse {
    pub item: OwnedItem,
    pub deadline: Instant,
    pub kind: PendingKind,
}

#[derive(Default)]
pub struct ItemUseState {
    pub(crate) pending: Option<PendingUse>,
    /// Shared item recast groups, updated only by the server's timer event.
    pub cooldowns: BTreeMap<i32, Instant>,
    pub status: String,
    pub last_item: Option<OwnedItem>,
}

impl ItemUseState {
    pub fn busy(&self) -> bool {
        self.pending.is_some()
    }
    pub fn reset(&mut self) {
        self.pending = None;
        self.last_item = None;
        self.status.clear();
    }
    pub(crate) fn fail(&mut self, message: impl Into<String>) {
        self.pending = None;
        self.status = message.into();
    }
    pub fn remaining(&self, item: &InventoryItem, now: Instant) -> Duration {
        let grouped = self
            .cooldowns
            .get(&item.click.recast_type)
            .map_or(Duration::ZERO, |until| until.saturating_duration_since(now));
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let saved = Duration::from_secs(u64::from(item.recast_timestamp).saturating_sub(epoch));
        grouped.max(saved)
    }
    pub(crate) fn allows(&self, command: &Command) -> bool {
        if let Command::ItemUse(command) = command {
            return self
                .pending
                .as_ref()
                .is_some_and(|pending| match (&pending.kind, command) {
                    (
                        PendingKind::Scribe {
                            spell_id,
                            book_slot,
                            phase: ScribePhase::Queued,
                            ..
                        },
                        ItemUseCommand::Scribe {
                            spell_id: requested,
                            book_slot: slot,
                        },
                    ) => spell_id == requested && book_slot == slot,
                    (
                        PendingKind::Click {
                            spell_id: _,
                            target_id,
                            queued: false,
                            ..
                        },
                        ItemUseCommand::Click {
                            slot,
                            target_id: target,
                        },
                    ) => pending.item.slot == *slot && target_id == target,
                    _ => false,
                });
        }
        if self.busy()
            && matches!(command,Command::Trade(trade) if !matches!(trade,openeq_net::trade::TradeCommand::Busy {..}|openeq_net::trade::TradeCommand::Cancel {..}))
        {
            return false;
        }
        !self.busy()
            || !matches!(
                command,
                Command::MoveItem { .. }
                    | Command::DeleteItem { .. }
                    | Command::LootItem { .. }
                    | Command::LootRequest(_)
                    | Command::MerchantOpen { .. }
                    | Command::MerchantBuy { .. }
                    | Command::MerchantSell { .. }
                    | Command::MoveCoin { .. }
                    | Command::BankerChange
                    | Command::AutoAttack(true)
                    | Command::ZoneChange { .. }
                    | Command::CastSpell { .. }
                    | Command::MemorizeSpell { .. }
                    | Command::UnmemorizeSpell { .. }
            )
    }
    /// Returns a completion/interruption notice. Verification alone changes no
    /// inventory, book entry, cast success or pending lock.
    pub(crate) fn observe(
        &mut self,
        event: &GameplayEvent,
        own: Option<u32>,
        now: Instant,
    ) -> Option<String> {
        if let GameplayEvent::ItemUse(ItemUseEvent::Recast {
            recast_type,
            seconds,
            ..
        }) = event
        {
            if *seconds == 0 {
                self.cooldowns.remove(recast_type);
            } else {
                self.cooldowns
                    .insert(*recast_type, now + Duration::from_secs(u64::from(*seconds)));
            }
        }
        let pending = self.pending.as_mut()?;
        let mut interrupted = false;
        match (&mut pending.kind, event) {
            (
                PendingKind::Scribe {
                    spell_id,
                    book_slot,
                    book_confirmed,
                    phase: ScribePhase::Waiting,
                    ..
                },
                GameplayEvent::SpellMemorized {
                    slot,
                    spell_id: actual,
                    action: 0,
                    ..
                },
            ) if *book_slot == *slot && *spell_id == *actual => *book_confirmed = true,
            (
                PendingKind::Scribe {
                    consumed,
                    phase: ScribePhase::Waiting,
                    ..
                },
                GameplayEvent::ItemMoved { from, to, .. },
            ) if *from == InventorySlot::CURSOR && *to == InventorySlot::DELETE => *consumed = true,
            (
                PendingKind::Scribe {
                    consumed,
                    phase: ScribePhase::Waiting,
                    ..
                },
                GameplayEvent::ItemDeleted { from, count },
            ) if *from == InventorySlot::CURSOR && *count > 0 => *consumed = true,
            (
                PendingKind::Click {
                    spell_id, began, ..
                },
                GameplayEvent::BeginCast {
                    caster_id,
                    spell_id: actual,
                    cast_time_ms,
                },
            ) if Some(*caster_id) == own && *spell_id == *actual => {
                *began = true;
                pending.deadline =
                    now + Duration::from_millis(u64::from(*cast_time_ms)) + Duration::from_secs(8);
                self.status = "Casting item effect…".into();
            }
            (
                PendingKind::Click {
                    spell_id, applied, ..
                },
                GameplayEvent::SpellAction {
                    source_id,
                    spell_id: actual,
                    ..
                },
            ) if Some(*source_id) == own && *spell_id == *actual => *applied = true,
            (
                PendingKind::Click {
                    spell_id, finished, ..
                },
                GameplayEvent::SpellBarEnabled {
                    spell_id: actual,
                    keep_casting: false,
                    ..
                },
            ) if *spell_id == *actual => *finished = true,
            (
                PendingKind::Click { consumed, .. },
                GameplayEvent::ItemChargeUsed { from, count }
                | GameplayEvent::ItemDeleted { from, count },
            ) if *from == pending.item.slot && *count > 0 => *consumed = true,
            (PendingKind::Click { consumed, .. }, GameplayEvent::ItemMoved { from, to, .. })
                if *from == pending.item.slot && *to == InventorySlot::DELETE =>
            {
                *consumed = true
            }
            (PendingKind::Click { .. }, GameplayEvent::CastInterrupted { id, .. })
                if Some(*id) == own || *id == 0 =>
            {
                interrupted = true
            }
            _ => {}
        }
        let complete = match pending.kind {
            PendingKind::Scribe {
                book_confirmed,
                consumed,
                ..
            } => book_confirmed && consumed,
            PendingKind::Click {
                applied,
                finished,
                consumed,
                ..
            } => applied && finished && consumed,
        };
        if interrupted {
            self.fail("Item cast interrupted.");
            return Some(self.status.clone());
        }
        if complete {
            let text = if matches!(pending.kind, PendingKind::Scribe { .. }) {
                "Spell scribed."
            } else {
                "Item effect completed."
            };
            self.fail(text);
            return Some(text.into());
        }
        None
    }
    pub(crate) fn expire(&mut self, now: Instant) -> bool {
        self.cooldowns.retain(|_, until| *until > now);
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| now >= pending.deadline)
        {
            self.fail("The server did not confirm this item action. Check your spellbook and inventory before trying again.");
            true
        } else {
            false
        }
    }
}

pub(crate) fn owned_item(
    game: &GameplayState,
    identity: OwnedItem,
) -> Result<&InventoryItem, String> {
    game.inventory
        .items
        .get(&identity.slot)
        .filter(|item| identity.matches(item))
        .ok_or_else(|| "That item moved or changed. Inspect it again.".into())
}

pub(crate) fn scribe_plan(game: &GameplayState, identity: OwnedItem) -> Result<(u32, u32), String> {
    let item = owned_item(game, identity)?;
    if !supports_click(identity.slot) && identity.slot != InventorySlot::CURSOR {
        return Err("Move the scroll into your carried inventory first.".into());
    }
    let spell_id = item
        .scroll_spell_id
        .filter(|_| item.item_type == 20)
        .ok_or("That item is not a spell scroll.")?;
    let profile = game
        .profile
        .as_ref()
        .ok_or("Your spellbook has not arrived yet.")?;
    let spell = game
        .spell_catalog
        .spells
        .get(&spell_id)
        .ok_or("Spell information is unavailable for this scroll.")?;
    let level = profile
        .class
        .checked_sub(1)
        .and_then(|class| spell.levels.get(class as usize))
        .copied()
        .filter(|level| *level != 255 && *level > 0)
        .ok_or("Your class cannot scribe that spell.")?;
    if profile.level < level {
        return Err(format!("Requires level {level} to scribe."));
    }
    if profile.spell_book.contains(&spell_id) {
        return Err("You already know this spell.".into());
    }
    let book_slot = profile
        .spell_book
        .iter()
        .take(720)
        .position(|id| matches!(*id, 0 | 0xffff | 0xffffffff))
        .ok_or("Your spellbook is full.")? as u32;
    if identity.slot == InventorySlot::CURSOR {
        if item.count != 1 {
            return Err("Separate one scroll from the cursor stack before scribing.".into());
        }
    } else {
        if game.inventory.items.contains_key(&InventorySlot::CURSOR) {
            return Err("Empty your cursor before scribing.".into());
        }
        game.inventory
            .validate_move(identity.slot, InventorySlot::CURSOR, 1)?;
    }
    Ok((spell_id, book_slot))
}

pub(crate) fn click_plan(
    game: &GameplayState,
    identity: OwnedItem,
    now: Instant,
) -> Result<u32, String> {
    let item = owned_item(game, identity)?;
    if !supports_click(identity.slot) {
        return Err("Move this item into your carried inventory first.".into());
    }
    if !matches!(item.click.effect_type, 1 | 3 | 4 | 5)
        || !(1..=45000).contains(&item.click.spell_id)
    {
        return Err("This item has no usable click effect.".into());
    }
    let profile = game
        .profile
        .as_ref()
        .ok_or("Your character information has not arrived yet.")?;
    if profile.level < item.click.required_level {
        return Err(format!(
            "Requires level {} to use.",
            item.click.required_level
        ));
    }
    if item.charges == 0 || (item.stack_size > 1 && item.count == 0) {
        return Err("This item is out of charges.".into());
    }
    if matches!(item.click.effect_type, 4 | 5) {
        let class = profile
            .class
            .checked_sub(1)
            .filter(|class| *class < 16)
            .ok_or("Your class cannot use this item.")?;
        if item.classes & (1 << class) == 0 {
            return Err("Your class cannot use this item.".into());
        }
    }
    if item.click.effect_type == 4 && (identity.slot.slot > 22 || identity.slot.bag.is_some()) {
        return Err("Equip this item before using its effect.".into());
    }
    let remaining = game.item_use.remaining(item, now);
    if !remaining.is_zero() {
        return Err(format!(
            "Ready in {} seconds.",
            remaining.as_secs() + u64::from(remaining.subsec_nanos() > 0)
        ));
    }
    Ok(item.click.spell_id as u32)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use openeq_net::{
        gameplay::{Currency, PlayerProfile},
        item_use::ClickEffect,
    };

    pub(crate) fn fixture() -> GameplayState {
        let mut game = GameplayState::default();
        game.inventory.received = true;
        game.profile = Some(PlayerProfile {
            guild_id: None,
            guild_rank: 0,
            name: "Player".into(),
            last_name: String::new(),
            race: 1,
            class: 12,
            level: 20,
            hp: 100,
            mana: 100,
            endurance: 100,
            stats: [100; 7],
            currency: Currency::default(),
            bank_currency: Currency::default(),
            cursor_currency: Currency::default(),
            shared_platinum: 0,
            skills: vec![],
            spell_book: vec![u32::MAX; 720],
            memorized_spells: vec![u32::MAX; 12],
            spell_refresh: vec![],
            buffs: vec![],
        });
        let mut levels = [255; 16];
        levels[11] = 1;
        game.spell_catalog.spells.insert(
            288,
            crate::spells::Spell {
                id: 288,
                name: "Minor Shielding".into(),
                projectile_model: String::new(),
                icon: 0,
                mana: 10,
                cast_time_ms: 2000,
                recovery_ms: 2000,
                recast_ms: 2500,
                range: 100.,
                target_type: 6,
                beneficial: true,
                effect_id: 220,
                casting_animation: 43,
                travel_type: 0,
                persistent_particles: false,
                levitation_mode: None,
                water_breathing: false,
                levels,
                description_id: 0,
                landing_message: String::new(),
                description: String::new(),
            },
        );
        game
    }
    pub(crate) fn item(slot: InventorySlot) -> InventoryItem {
        InventoryItem {
            slot,
            id: 15288,
            instance_id: 99,
            name: "Scroll".into(),
            lore: String::new(),
            id_file: String::new(),
            icon: 504,
            price: 0,
            merchant_count: 0,
            base_price: 1,
            no_drop: false,
            attuned: false,
            count: 3,
            charges: 0,
            stack_size: 20,
            item_class: 0,
            item_type: 20,
            equip_slots: 0,
            classes: 65535,
            races: 65535,
            material: 0,
            color: 0,
            damage: 0,
            delay: 0,
            ac: 0,
            hp: 0,
            mana: 0,
            endurance: 0,
            required_level: 0,
            click: ClickEffect::default(),
            scroll_spell_id: Some(288),
            recast_timestamp: 0,
            bag_slots: 0,
            bag_size: 0,
            weight: 0,
            size: 1,
            children: vec![],
        }
    }
    pub(crate) fn identity(item: &InventoryItem) -> OwnedItem {
        OwnedItem {
            slot: item.slot,
            id: item.id,
            instance_id: item.instance_id,
        }
    }
    pub(crate) fn pending_click(slot: InventorySlot, finite: bool) -> PendingUse {
        PendingUse {
            item: OwnedItem {
                slot,
                id: 14534,
                instance_id: 20,
            },
            deadline: Instant::now() + Duration::from_secs(10),
            kind: PendingKind::Click {
                spell_id: 278,
                target_id: 1,
                queued: true,
                sent: true,
                began: false,
                applied: false,
                finished: false,
                consumed: !finite,
            },
        }
    }
    fn action() -> GameplayEvent {
        GameplayEvent::SpellAction {
            source_id: 1,
            target_id: 1,
            spell_id: 278,
            level: 20,
            effect_flag: 0,
            action_type: 231,
            spell_level: 20,
            instrument_modifier: 1.,
        }
    }

    #[test]
    fn scribing_checks_identity_class_level_known_book_and_single_cursor_scroll() {
        let mut game = fixture();
        let scroll = item(InventorySlot::possessions(23));
        let owned = identity(&scroll);
        game.inventory.insert(scroll);
        assert_eq!(scribe_plan(&game, owned).unwrap(), (288, 0));
        assert!(
            scribe_plan(
                &game,
                OwnedItem {
                    instance_id: 100,
                    ..owned
                }
            )
            .is_err()
        );
        game.profile.as_mut().unwrap().spell_book[0] = 288;
        assert!(
            scribe_plan(&game, owned)
                .unwrap_err()
                .contains("already know")
        );
        game.profile.as_mut().unwrap().spell_book.fill(u32::MAX);
        game.profile.as_mut().unwrap().class = 1;
        assert!(scribe_plan(&game, owned).unwrap_err().contains("class"));
        game.profile.as_mut().unwrap().class = 12;
        game.spell_catalog.spells.get_mut(&288).unwrap().levels[11] = 30;
        assert!(scribe_plan(&game, owned).unwrap_err().contains("level 30"));
        game.spell_catalog.spells.get_mut(&288).unwrap().levels[11] = 1;
        game.inventory
            .move_item(owned.slot, InventorySlot::CURSOR, 1)
            .unwrap();
        assert_eq!(game.inventory.items[&owned.slot].count, 2);
        assert_eq!(scribe_plan(&game, owned.on_cursor()).unwrap(), (288, 0));
        assert!(scribe_plan(&game, owned).unwrap_err().contains("cursor"));
        game.inventory
            .items
            .get_mut(&InventorySlot::CURSOR)
            .unwrap()
            .count = 2;
        assert!(
            scribe_plan(&game, owned.on_cursor())
                .unwrap_err()
                .contains("Separate one")
        );
    }

    #[test]
    fn clicks_use_level2_effect_rules_and_real_cooldowns_instead_of_spell_class() {
        let mut game = fixture();
        let slot = InventorySlot::possessions(25).in_bag(1);
        let mut potion = item(slot);
        potion.id = 14534;
        potion.charges = 10;
        potion.count = 1;
        potion.stack_size = 1;
        potion.item_type = 21;
        potion.scroll_spell_id = None;
        potion.classes = 1;
        potion.click = ClickEffect {
            spell_id: 278,
            required_level: 0,
            level: 60,
            effect_type: 1,
            max_charges: 10,
            cast_time_ms: 4000,
            recast_seconds: 180,
            recast_type: 24,
        };
        let owned = identity(&potion);
        game.inventory.insert(potion);
        let now = Instant::now();
        assert_eq!(
            click_plan(&game, owned, now).unwrap(),
            278,
            "wizard can click non-wizard spell; level60 is not requiredlevel"
        );
        game.inventory
            .items
            .get_mut(&slot)
            .unwrap()
            .click
            .effect_type = 5;
        assert!(click_plan(&game, owned, now).unwrap_err().contains("class"));
        game.inventory.items.get_mut(&slot).unwrap().classes = 65535;
        game.inventory
            .items
            .get_mut(&slot)
            .unwrap()
            .click
            .effect_type = 4;
        assert!(click_plan(&game, owned, now).unwrap_err().contains("Equip"));
        game.inventory
            .items
            .get_mut(&slot)
            .unwrap()
            .click
            .effect_type = 1;
        game.inventory.items.get_mut(&slot).unwrap().charges = 0;
        assert!(
            click_plan(&game, owned, now)
                .unwrap_err()
                .contains("charges")
        );
        game.inventory.items.get_mut(&slot).unwrap().charges = 10;
        game.item_use
            .cooldowns
            .insert(24, now + Duration::from_secs(10));
        assert!(
            click_plan(&game, owned, now)
                .unwrap_err()
                .contains("Ready in 10")
        );
        assert!(click_plan(&game, owned, now + Duration::from_secs(11)).is_ok());
        game.inventory
            .items
            .get_mut(&slot)
            .unwrap()
            .recast_timestamp = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 60) as u32;
        assert!(click_plan(&game, owned, now + Duration::from_secs(11)).is_err());
    }

    #[test]
    fn verification_does_not_complete_click_and_interrupt_does_not_consume() {
        let slot = InventorySlot::possessions(25).in_bag(1);
        let now = Instant::now();
        let mut state = ItemUseState {
            pending: Some(pending_click(slot, true)),
            ..Default::default()
        };
        assert!(
            state
                .observe(
                    &GameplayEvent::ItemUse(ItemUseEvent::Verified {
                        slot,
                        spell_id: 0,
                        target_id: 1
                    }),
                    Some(1),
                    now
                )
                .is_none()
        );
        assert!(state.busy());
        state.observe(&action(), Some(1), now);
        let enabled = GameplayEvent::SpellBarEnabled {
            spell_id: 278,
            slot: -1,
            mana: 100,
            endurance: 100,
            keep_casting: false,
        };
        assert!(
            state.observe(&enabled, Some(1), now).is_none(),
            "finite item still needs charge event"
        );
        assert!(
            state
                .observe(
                    &GameplayEvent::ItemChargeUsed {
                        from: slot,
                        count: 1
                    },
                    Some(1),
                    now
                )
                .is_some()
        );
        assert!(!state.busy());
        state.pending = Some(pending_click(slot, true));
        assert!(
            state
                .observe(
                    &GameplayEvent::CastInterrupted {
                        id: 1,
                        string_id: 439,
                        message: String::new()
                    },
                    Some(1),
                    now
                )
                .unwrap()
                .contains("interrupted")
        );
        assert!(!state.busy());
        state.pending = Some(pending_click(slot, false));
        state.observe(&action(), Some(1), now);
        assert!(
            state.observe(&enabled, Some(1), now).is_some(),
            "unlimited item completes without a charge packet"
        );
    }
}
