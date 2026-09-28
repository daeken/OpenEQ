//! UI interaction state and user-intent dispatch. Inventory addresses come from
//! decoded protocol slots; skin widget IDs never become packet addresses.
use crate::{
    chat::{self, Action, ChatEditor},
    game::{self, LootSession},
    gameplay_ui::*,
    live::LiveWorld,
};
use openeq_net::{
    gameplay::{ChatChannel, Command},
    inventory::InventorySlot,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Interaction {
    pub editor: ChatEditor,
    pub inventory_open: bool,
    pub open_bags: BTreeSet<i32>,
    pub chat_scroll: usize,
    pub pointer: Option<[f32; 2]>,
    pub window_positions: BTreeMap<String, [f32; 2]>,
    pub inspected_item: Option<UiItem>,
    pub drag: Option<(String, [f32; 2])>,
    /// Captures the whole frame even when Enter/Escape closes the editor.
    pub controls_blocked: bool,
    pub escape_handled: bool,
    pub ime_composing: bool,
    pub spellbook_open: bool,
    pub spellbook_page: usize,
    pub selected_gem: Option<u8>,
    pub merchant_stock: Option<u32>,
    pub merchant_sell: Option<InventorySlot>,
    pub merchant_quantity: u32,
    pub merchant_scroll: usize,
    pub bank_coin: u8,
    pub bank_quantity: u32,
}

impl Interaction {
    pub fn view(&self, live: &LiveWorld) -> GameHudState {
        let mut chat_input = self.editor.text.clone();
        chat_input.insert_str(self.editor.cursor, &self.editor.preedit);
        let inventory = &live.game.inventory.items;
        let class = live
            .game
            .profile
            .as_ref()
            .map_or(1, |profile| profile.class);
        let now = std::time::Instant::now();
        let known_spells = live
            .game
            .profile
            .as_ref()
            .map(|profile| {
                profile
                    .spell_book
                    .iter()
                    .copied()
                    .filter(|id| *id != u32::MAX && *id > 0)
                    .map(|id| live.game.spell_catalog.view(id, class))
                    .collect()
            })
            .unwrap_or_default();
        let memorized = live
            .game
            .profile
            .as_ref()
            .map(|profile| {
                profile
                    .memorized_spells
                    .iter()
                    .take(12)
                    .enumerate()
                    .map(|(gem, &id)| UiSpellGem {
                        gem: gem as u8,
                        spell: (id > 0 && id != u32::MAX)
                            .then(|| live.game.spell_catalog.view(id, class)),
                        ready: live.game.casting.is_none()
                            && live
                                .game
                                .cast_pending_until
                                .is_none_or(|until| until <= now)
                            && live
                                .game
                                .spell_cooldowns
                                .get(&(gem as u8))
                                .is_none_or(|until| *until <= now),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let casting = live.game.casting.as_ref().map(|cast| UiCasting {
            label: live.game.spell_catalog.view(cast.spell_id, class).name,
            progress: (cast.started.elapsed().as_secs_f32()
                / cast.duration.as_secs_f32().max(0.001))
            .min(1.),
        });
        let slot_view = |id: u16, label: &str| UiSlot {
            slot: id as i32,
            label: label.into(),
            item: inventory
                .get(&InventorySlot::possessions(id))
                .map(game::item_view),
        };
        let equipment = [
            "Charm",
            "Left ear",
            "Head",
            "Face",
            "Right ear",
            "Neck",
            "Shoulders",
            "Arms",
            "Back",
            "Left wrist",
            "Right wrist",
            "Range",
            "Hands",
            "Primary",
            "Secondary",
            "Left finger",
            "Right finger",
            "Chest",
            "Legs",
            "Feet",
            "Waist",
            "Power source",
            "Ammo",
        ]
        .iter()
        .enumerate()
        .map(|(i, label)| slot_view(i as u16, label))
        .collect();
        let general = (23..=32)
            .map(|i| slot_view(i, &format!("Bag {}", i - 22)))
            .collect();
        let bags = self
            .open_bags
            .iter()
            .filter_map(|&id| {
                let parent = InventorySlot::from_server_slot(id.try_into().ok()?)?;
                let item = inventory.get(&parent).filter(|item| item.bag_slots > 0)?;
                let slots = (0..item.bag_slots as u16)
                    .filter_map(|i| {
                        let slot = parent.in_bag(i);
                        Some(UiSlot {
                            slot: slot.server_slot()? as i32,
                            label: format!("Slot {}", i + 1),
                            item: inventory.get(&slot).map(game::item_view),
                        })
                    })
                    .collect();
                Some(UiBag {
                    parent_slot: id,
                    name: item.name.clone(),
                    slots,
                })
            })
            .collect();
        let loot = live
            .game
            .loot
            .as_ref()
            .filter(|loot| loot.opened)
            .map(|loot| UiLoot {
                name: loot.name.clone(),
                items: loot
                    .items
                    .iter()
                    .map(|(&slot, item)| UiLootItem {
                        slot,
                        item: game::item_view(item),
                    })
                    .collect(),
            });
        let mut view = GameHudState {
            chat: live.game.chat.iter().cloned().collect(),
            chat_input,
            chat_active: self.editor.active,
            chat_cursor: Some(self.editor.cursor + self.editor.preedit.len()),
            chat_scroll: self.chat_scroll,
            inventory_open: self.inventory_open,
            equipment,
            inventory: general,
            bags,
            loot,
            cursor_item: inventory.get(&InventorySlot::CURSOR).map(game::item_view),
            pointer: self.pointer,
            selected_slot: None,
            attack: live.game.attack,
            sitting: live.game.sitting,
            window_positions: self.window_positions.clone(),
            inspected_item: self
                .inspected_item
                .clone()
                .or_else(|| live.game.linked_item.as_ref().map(game::item_view)),
            known_spells,
            memorized,
            casting,
            spellbook_open: self.spellbook_open,
            spellbook_page: self.spellbook_page,
            selected_gem: self.selected_gem,
            buffs: live
                .game
                .buffs
                .values()
                .map(|buff| UiBuff {
                    slot: buff.slot,
                    spell: live.game.spell_catalog.view(buff.spell_id, class),
                    remaining_seconds: live.game.buff_seconds(buff.slot),
                })
                .collect(),
            ..Default::default()
        };
        self.commerce_view(live, &mut view);
        self.social_view(live, &mut view);
        view
    }

    /// Returns true only for an explicit quit command.
    pub fn submit(&mut self, text: &str, live: &mut LiveWorld, position: [f32; 3]) -> bool {
        match chat::parse(text) {
            Ok(action) => self.action(action, live, position),
            Err(error) => {
                live.game.error(error);
                false
            }
        }
    }

    pub fn action(&mut self, action: Action, live: &mut LiveWorld, position: [f32; 3]) -> bool {
        let target = live.target;
        match action {
            Action::Chat {
                channel,
                recipient,
                text,
            } => {
                let channel = match channel {
                    0 => ChatChannel::Guild,
                    2 => ChatChannel::Group,
                    3 => ChatChannel::Shout,
                    4 => ChatChannel::Auction,
                    5 => ChatChannel::Ooc,
                    7 => ChatChannel::Tell,
                    8 => ChatChannel::Say,
                    _ => return false,
                };
                live.command(Command::Chat {
                    channel,
                    target: recipient,
                    text,
                    language: 0,
                });
                self.chat_scroll = 0;
            }
            Action::Reply(text) => {
                if let Some(recipient) = live.game.last_tell.clone() {
                    live.command(Command::Chat {
                        channel: ChatChannel::Tell,
                        target: recipient,
                        text,
                        language: 0,
                    });
                } else {
                    live.game.error("No one has sent you a tell yet.");
                }
            }
            Action::Emote(text) => {
                live.command(Command::Emote(text));
            }
            Action::Attack(active) => {
                let active = active.unwrap_or(!live.game.attack);
                if active
                    && !target
                        .and_then(|id| live.entities.get(&id))
                        .is_some_and(|e| !e.spawn.is_corpse && Some(e.spawn.id) != live.own_id)
                {
                    live.game.error("Select a living target before attacking.");
                    return false;
                }
                if active
                    && live.game.sitting
                    && let Some(id) = live.own_id
                {
                    live.command(Command::Posture {
                        player_id: id,
                        posture: 0,
                    });
                }
                live.command(Command::AutoAttack(active));
            }
            Action::Sit(sit) => {
                if let Some(id) = live.own_id {
                    if sit && live.game.attack {
                        live.command(Command::AutoAttack(false));
                    }
                    live.command(Command::Posture {
                        player_id: id,
                        posture: u32::from(sit),
                    });
                }
            }
            Action::Hail => {
                let text = target.and_then(|id| live.entities.get(&id)).map_or_else(
                    || "Hail".to_owned(),
                    |e| format!("Hail, {}", game::display_name(&e.spawn.name)),
                );
                live.command(Command::Chat {
                    channel: ChatChannel::Say,
                    target: String::new(),
                    text,
                    language: 0,
                });
            }
            Action::Loot => {
                let Some(corpse) = target
                    .and_then(|id| live.entities.get(&id))
                    .filter(|e| e.spawn.is_corpse)
                else {
                    live.game.error("Select a corpse to loot.");
                    return false;
                };
                let corpse_id = corpse.spawn.id;
                let name = game::display_name(&corpse.spawn.name);
                if let Some(previous) = live.game.loot.as_ref().map(|loot| loot.corpse_id) {
                    if previous != corpse_id {
                        live.command(Command::EndLoot(previous));
                    }
                    return false;
                }
                if live.command(Command::LootRequest(corpse_id)) {
                    live.game.loot = Some(LootSession {
                        corpse_id,
                        name,
                        opened: false,
                        items_complete: false,
                        items: BTreeMap::new(),
                        pending: None,
                        take_all: false,
                    });
                }
            }
            Action::Consider => {
                if let (Some(player_id), Some(target_id)) = (live.own_id, target) {
                    live.command(Command::Consider {
                        player_id,
                        target_id,
                    });
                } else {
                    live.game.error("Select a target to consider.");
                }
            }
            Action::Assist(name) => {
                let id = name
                    .as_ref()
                    .and_then(|name| find_target(live, name, position))
                    .or(target);
                if let Some(id) = id {
                    live.command(Command::Assist(id));
                } else {
                    live.game
                        .error("Select someone to assist, or use /assist NAME.");
                }
            }
            Action::Target(name) => {
                if name.eq_ignore_ascii_case("clear") {
                    live.set_target(None);
                } else if let Some(id) = find_target(live, &name, position) {
                    live.set_target(Some(id));
                } else {
                    live.game.error(format!("No nearby target matches {name}."));
                }
            }
            Action::UseTarget => self.open_service(live, None),
            Action::Merchant => self.open_service(live, Some(crate::commerce::MERCHANT_CLASS)),
            Action::Bank => self.open_service(live, Some(crate::commerce::BANKER_CLASS)),
            Action::Invite(name) => self.invite(live, name),
            Action::AcceptInvite => self.answer_invite(live, true),
            Action::DeclineInvite => self.answer_invite(live, false),
            Action::LeaveGroup => {
                live.set_target(live.own_id);
                live.command(Command::Social(openeq_net::social::SocialCommand::Leave {
                    character: live.character.clone(),
                }));
            }
            Action::MakeLeader(leader) => {
                if live.game.group.leader.eq_ignore_ascii_case(&live.character)
                    && live
                        .game
                        .group
                        .members
                        .iter()
                        .any(|member| member.name.eq_ignore_ascii_case(&leader))
                {
                    live.command(Command::Social(
                        openeq_net::social::SocialCommand::MakeLeader {
                            character: live.character.clone(),
                            leader,
                        },
                    ));
                } else {
                    live.game
                        .error("Only the group leader can transfer leadership to a group member.");
                }
            }
            Action::Inventory => self.inventory_open = !self.inventory_open,
            Action::Spellbook => self.spellbook_open = !self.spellbook_open,
            Action::Cast(gem) => self.cast(gem, live),
            Action::StopCast => {
                live.command(Command::InterruptSpell);
            }
            Action::Location => live.game.notice(format!(
                "Location: {:.1}, {:.1}, {:.1} (Y, X, Z)",
                position[0],
                position[1],
                position[2] - 3.
            )),
            Action::Help => live.game.notice(chat::HELP),
            Action::Quit => return true,
        }
        false
    }

    pub fn ui_action(
        &mut self,
        action: UiAction,
        right: bool,
        split: bool,
        live: &mut LiveWorld,
        position: [f32; 3],
    ) {
        if right
            && !matches!(
                action,
                UiAction::InventorySlot(_)
                    | UiAction::LootItem(_)
                    | UiAction::CastGem(_)
                    | UiAction::RemoveBuff(_)
            )
        {
            return;
        }
        match action {
            UiAction::Commerce(action) => self.commerce_action(action, live),
            UiAction::Social(action) => self.social_action(action, live),
            UiAction::ChatLink(id) => {
                if let Some(link) = live.game.chat_links.get(&id).cloned() {
                    self.inspected_item = None;
                    live.game.linked_item = None;
                    live.command(Command::Social(
                        openeq_net::social::SocialCommand::ActivateLink(link),
                    ));
                }
            }
            UiAction::RemoveBuff(slot) => {
                if right && let Some(player_id) = live.own_id {
                    live.command(Command::RemoveBuff { slot, player_id });
                }
            }
            UiAction::CastGem(gem) => {
                if right {
                    self.spellbook_open = true;
                    self.selected_gem = Some(gem);
                    return;
                }
                if self.spellbook_open {
                    self.selected_gem = Some(gem);
                } else {
                    self.cast(gem, live);
                }
            }
            UiAction::MemorizeSpell { id, gem } => {
                if live
                    .game
                    .profile
                    .as_ref()
                    .is_some_and(|profile| profile.spell_book.contains(&id))
                {
                    if let Some(player_id) = live.own_id {
                        live.command(Command::Posture {
                            player_id,
                            posture: 1,
                        });
                    }
                    live.command(Command::MemorizeSpell {
                        slot: gem as u32,
                        spell_id: id,
                    });
                }
            }
            UiAction::ForgetGem(gem) => {
                if let Some(id) = live
                    .game
                    .profile
                    .as_ref()
                    .and_then(|profile| profile.memorized_spells.get(gem as usize))
                    .copied()
                    .filter(|id| *id > 0 && *id != u32::MAX)
                {
                    live.command(Command::UnmemorizeSpell {
                        slot: gem as u32,
                        spell_id: id,
                    });
                }
            }
            UiAction::ToggleSpellbook => self.spellbook_open = !self.spellbook_open,
            UiAction::SpellbookPage(delta) => {
                let count = live.game.profile.as_ref().map_or(0, |profile| {
                    profile
                        .spell_book
                        .iter()
                        .filter(|id| **id > 0 && **id != u32::MAX)
                        .count()
                });
                self.spellbook_page = self
                    .spellbook_page
                    .saturating_add_signed(delta as isize)
                    .min(count.saturating_sub(1) / SPELLBOOK_PAGE_SIZE);
            }
            UiAction::InventorySlot(id) => {
                let Some(slot) = u32::try_from(id)
                    .ok()
                    .and_then(InventorySlot::from_server_slot)
                else {
                    return;
                };
                if right {
                    if let Some(item) = live.game.inventory.items.get(&slot) {
                        if item.bag_slots > 0 {
                            if !self.open_bags.remove(&id) {
                                self.open_bags.insert(id);
                            }
                        } else {
                            self.inspected_item = Some(game::item_view(item));
                        }
                    }
                } else if live
                    .game
                    .commerce
                    .merchant
                    .as_ref()
                    .is_some_and(|merchant| merchant.opened)
                    && !live
                        .game
                        .inventory
                        .items
                        .contains_key(&InventorySlot::CURSOR)
                {
                    if slot.kind == 0 && live.game.inventory.items.contains_key(&slot) {
                        self.merchant_sell = Some(slot);
                        self.merchant_stock = None;
                        self.merchant_quantity = 1;
                    }
                } else if live
                    .game
                    .inventory
                    .items
                    .contains_key(&InventorySlot::CURSOR)
                {
                    live.command(Command::MoveItem {
                        from: InventorySlot::CURSOR,
                        to: slot,
                        count: if split { 1 } else { 0 },
                    });
                } else if live.game.inventory.items.contains_key(&slot) {
                    live.command(Command::MoveItem {
                        from: slot,
                        to: InventorySlot::CURSOR,
                        count: if split { 1 } else { 0 },
                    });
                }
            }
            UiAction::LootItem(slot) => {
                if right {
                    self.inspected_item = live
                        .game
                        .loot
                        .as_ref()
                        .and_then(|loot| loot.items.get(&slot))
                        .map(game::item_view);
                } else {
                    request_loot(live, slot);
                }
            }
            UiAction::LootAll => {
                if let Some(loot) = &mut live.game.loot {
                    loot.take_all = true;
                }
            }
            UiAction::ToggleInventory => {
                self.action(Action::Inventory, live, position);
            }
            UiAction::ToggleAttack => {
                self.action(Action::Attack(None), live, position);
            }
            UiAction::ToggleSit => {
                self.action(Action::Sit(!live.game.sitting), live, position);
            }
            UiAction::Hail => {
                self.action(Action::Hail, live, position);
            }
            UiAction::LootTarget => {
                self.action(Action::Loot, live, position);
            }
            UiAction::FocusChat => self.editor.open(""),
            UiAction::CloseWindow(window) => self.close_window(&window, live),
            UiAction::BeginWindowDrag(_) => {}
        }
    }

    pub fn close_window(&mut self, window: &str, live: &mut LiveWorld) {
        if window == "merchant" {
            live.command(Command::MerchantClose);
            live.game.commerce.merchant = None;
            self.merchant_stock = None;
            self.merchant_sell = None;
        } else if window == "bank" {
            live.game.commerce.bank = None;
            self.open_bags.retain(|id| {
                InventorySlot::from_server_slot(*id as u32).is_some_and(|slot| slot.kind == 0)
            });
        } else if window == "spellbook" {
            self.spellbook_open = false;
        } else if window == "inventory" {
            self.inventory_open = false;
        } else if window == "loot" {
            if let Some(corpse) = live.game.loot.as_ref().map(|loot| loot.corpse_id) {
                live.command(Command::EndLoot(corpse));
            }
        } else if window == "inspect" {
            self.inspected_item = None;
            live.game.linked_item = None;
        } else if let Some(id) = window
            .strip_prefix("bag:")
            .and_then(|id| id.parse::<i32>().ok())
        {
            self.open_bags.remove(&id);
        }
    }

    pub fn tick(&mut self, live: &mut LiveWorld) {
        if live
            .game
            .commerce
            .bank
            .as_ref()
            .is_some_and(|bank| !live.service_available(bank.id, crate::commerce::BANKER_CLASS))
        {
            self.close_window("bank", live);
            live.game.notice("Bank closed: the banker is out of reach.");
        }
        if live
            .game
            .commerce
            .merchant
            .as_ref()
            .is_some_and(|merchant| {
                !live.service_available(merchant.id, crate::commerce::MERCHANT_CLASS)
            })
        {
            self.close_window("merchant", live);
            live.game.notice("Merchant closed: out of reach.");
        }
        let slot = live
            .game
            .loot
            .as_ref()
            .filter(|loot| loot.items_complete && loot.take_all && loot.pending.is_none())
            .and_then(|loot| loot.items.keys().next().copied());
        if let Some(slot) = slot {
            request_loot(live, slot);
        }
    }

    pub fn cast(&mut self, gem: u8, live: &mut LiveWorld) {
        if gem >= 12 {
            return;
        }
        let now = std::time::Instant::now();
        if live.game.casting.is_some()
            || live
                .game
                .cast_pending_until
                .is_some_and(|until| until > now)
        {
            live.game.notice("A spell is already being cast.");
            return;
        }
        if live
            .game
            .spell_cooldowns
            .get(&gem)
            .is_some_and(|until| *until > now)
        {
            live.game.notice("That spell is not ready yet.");
            return;
        }
        let Some(id) = live
            .game
            .profile
            .as_ref()
            .and_then(|profile| profile.memorized_spells.get(gem as usize))
            .copied()
            .filter(|id| *id > 0 && *id != u32::MAX)
        else {
            live.game
                .notice("That spell gem is empty. Open the spellbook with B.");
            return;
        };
        let Some(player_id) = live.own_id else {
            return;
        };
        let spell = live.game.spell_catalog.spells.get(&id);
        let target_id = if spell.is_some_and(|spell| spell.target_type == 6) {
            player_id
        } else {
            live.target.unwrap_or(player_id)
        };
        if live.game.sitting {
            live.command(Command::Posture {
                player_id,
                posture: 0,
            });
        }
        if live.command(Command::CastSpell {
            slot: gem as u32,
            spell_id: id,
            target_id,
        }) {
            live.game.cast_pending_until = Some(now + std::time::Duration::from_secs(3));
        }
    }
}

fn request_loot(live: &mut LiveWorld, slot: u16) {
    let Some(loot) = live
        .game
        .loot
        .as_ref()
        .filter(|loot| loot.opened && loot.pending.is_none() && loot.items.contains_key(&slot))
    else {
        return;
    };
    let Some(player_id) = live.own_id else {
        return;
    };
    let corpse_id = loot.corpse_id;
    if live.command(Command::LootItem {
        corpse_id,
        player_id,
        slot,
        auto_loot: true,
    }) && let Some(loot) = &mut live.game.loot
    {
        loot.pending = Some(slot);
    }
}

fn find_target(live: &LiveWorld, query: &str, position: [f32; 3]) -> Option<u32> {
    let query = query.to_lowercase();
    live.entities
        .values()
        .filter(|e| Some(e.spawn.id) != live.own_id)
        .filter_map(|e| {
            let name = game::display_name(&e.spawn.name).to_lowercase();
            if !name.contains(&query) {
                return None;
            }
            let distance = e
                .position(std::time::Instant::now())
                .iter()
                .zip(position)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>();
            (distance < 500. * 500.).then_some((e.spawn.id, name != query, distance))
        })
        .min_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)))
        .map(|entry| entry.0)
}
