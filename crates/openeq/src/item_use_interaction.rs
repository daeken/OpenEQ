//! Owned-item intentions, with cursor scribing ordered behind the sent move.
use crate::{
    game,
    gameplay_ui::GameHudState,
    interaction::Interaction,
    item_use_state::{
        OwnedItem, PendingKind, PendingUse, ScribePhase, click_plan, owned_item, scribe_plan,
    },
    live::LiveWorld,
    trade_ui::{ItemUseAction, UiItemUse},
};
use openeq_net::{
    gameplay::{Command, GameplayEvent},
    inventory::InventorySlot,
    item_use::ItemUseCommand,
};
use std::time::{Duration, Instant};

fn available(live: &LiveWorld) -> Result<(), String> {
    if !live.ready || live.error.is_some() || live.own_id.is_none() {
        return Err("You are not connected to the zone.".into());
    }
    if live.game.hp.current == Some(0)
        || live
            .own_id
            .and_then(|id| live.entities.get(&id))
            .is_some_and(|entity| entity.spawn.is_corpse)
    {
        return Err("You cannot use items while dead.".into());
    }
    if live.game.item_use.busy() {
        return Err(live.game.item_use.status.clone());
    }
    if live.game.trade.engaged() {
        return Err("Finish trading before using an item.".into());
    }
    if live.game.inventory_command_pending
        || live.game.commerce.pending.is_some()
        || live.game.commerce.coin_pending
        || live.game.commerce.merchant.is_some()
        || live.game.commerce.merchant_closing
        || live.game.commerce.bank.is_some()
        || live.game.loot.is_some()
    {
        return Err("Finish the current inventory action first.".into());
    }
    if live.game.casting.is_some()
        || live
            .game
            .cast_pending_until
            .is_some_and(|until| until > Instant::now())
    {
        return Err("A spell is already being cast.".into());
    }
    Ok(())
}

impl Interaction {
    pub(crate) fn item_use_view(&self, live: &LiveWorld, view: &mut GameHudState) {
        let Some(identity) = self.inspected_owned.map(OwnedItem::from) else {
            return;
        };
        let mut state = UiItemUse::default();
        let item = match owned_item(&live.game, identity) {
            Ok(item) => item,
            Err(error) => {
                state.status = if live.game.item_use.last_item == Some(identity) {
                    live.game.item_use.status.clone()
                } else {
                    error
                };
                view.item_use = Some(state);
                return;
            }
        };
        // Keep inspection details tied to the current instance and real catalog.
        let mut inspected = game::item_view(item);
        if let Some(spell_id) = item.scroll_spell_id {
            let class = live
                .game
                .profile
                .as_ref()
                .map_or(0, |profile| profile.class);
            let spell = live.game.spell_catalog.view(spell_id, class);
            inspected.details.push(format!("Scribes: {}", spell.name));
            if spell.level != 255 && spell.level > 0 {
                inspected
                    .details
                    .push(format!("Scribing level: {}", spell.level));
            }
        }
        if matches!(item.click.effect_type, 1 | 3 | 4 | 5) && item.click.spell_id > 0 {
            let spell_id = item.click.spell_id as u32;
            let spell = live.game.spell_catalog.spells.get(&spell_id);
            let name =
                spell.map_or_else(|| format!("Spell {spell_id}"), |spell| spell.name.clone());
            let cast_ms = if item.click.cast_time_ms < 0 {
                spell.map_or(0, |spell| spell.cast_time_ms)
            } else {
                item.click.cast_time_ms as u32
            };
            inspected.details.push(format!("Effect: {name}"));
            inspected.details.push(if cast_ms == 0 {
                "Cast: instant".into()
            } else {
                format!("Cast: {:.1} seconds", cast_ms as f32 / 1000.)
            });
            if item.click.required_level > 0 {
                inspected
                    .details
                    .push(format!("Required level: {}", item.click.required_level));
            }
            if item.charges < 0 {
                inspected.details.push("Unlimited charges".into());
            }
            if item.click.recast_seconds > 0 {
                inspected
                    .details
                    .push(format!("Reuse: {} seconds", item.click.recast_seconds));
            }
        }
        view.inspected_item = Some(inspected);
        let ready = available(live);
        if item.scroll_spell_id.is_some() && item.item_type == 20 {
            state.scribe_label = Some("Scribe spell".into());
            let check = ready.clone().and_then(|()| {
                if live.game.attack {
                    return Err("Stop attacking before scribing.".into());
                }
                scribe_plan(&live.game, identity).map(|_| ())
            });
            state.can_scribe = check.is_ok();
            if let Err(error) = check {
                state.status = error;
            }
        }
        if matches!(item.click.effect_type, 1 | 3 | 4 | 5) && item.click.spell_id > 0 {
            state.use_label = Some("Use item".into());
            let check =
                ready.and_then(|()| click_plan(&live.game, identity, Instant::now()).map(|_| ()));
            state.can_use = check.is_ok();
            if let Err(error) = check {
                state.status = error;
            }
        }
        if state.status.is_empty() && live.game.item_use.last_item == Some(identity) {
            state.status = live.game.item_use.status.clone();
        }
        if state.scribe_label.is_some() || state.use_label.is_some() {
            view.item_use = Some(state);
        }
    }

    pub(crate) fn item_use_action(&mut self, live: &mut LiveWorld, action: ItemUseAction) {
        let result = (|| {
            available(live)?;
            let identity = self
                .inspected_owned
                .map(OwnedItem::from)
                .ok_or("Inspect an item in your own inventory first.")?;
            let own = live.own_id.ok_or("Your character is not ready.")?;
            let now = Instant::now();
            match action {
                ItemUseAction::Scribe => {
                    if live.game.attack {
                        return Err("Stop attacking before scribing.".into());
                    }
                    let (spell_id, book_slot) = scribe_plan(&live.game, identity)?;
                    if !live.game.sitting
                        && !live.command(Command::Posture {
                            player_id: own,
                            posture: 1,
                        })
                    {
                        return Err("Could not sit to scribe.".into());
                    }
                    let moving = identity.slot != InventorySlot::CURSOR;
                    let move_count = u32::from(owned_item(&live.game, identity)?.stack_size > 1);
                    if moving
                        && !live.command(Command::MoveItem {
                            from: identity.slot,
                            to: InventorySlot::CURSOR,
                            count: move_count,
                        })
                    {
                        return Err("Could not move the scroll to your cursor.".into());
                    }
                    live.game.item_use.pending = Some(PendingUse {
                        item: identity,
                        deadline: now + Duration::from_secs(10),
                        kind: PendingKind::Scribe {
                            spell_id,
                            book_slot,
                            phase: if moving {
                                ScribePhase::Moving
                            } else {
                                ScribePhase::Queued
                            },
                            book_confirmed: false,
                            consumed: false,
                        },
                    });
                    live.game.item_use.last_item = Some(identity);
                    live.game.item_use.status = if moving {
                        "Moving one scroll to your cursor…"
                    } else {
                        "Scribing; waiting for the server…"
                    }
                    .into();
                    if !moving
                        && !live.command(Command::ItemUse(ItemUseCommand::Scribe {
                            spell_id,
                            book_slot,
                        }))
                    {
                        live.game.item_use.fail("The scribe request was not sent.");
                        return Err(live.game.item_use.status.clone());
                    }
                }
                ItemUseAction::Use => {
                    let spell_id = click_plan(&live.game, identity, now)?;
                    let unlimited = owned_item(&live.game, identity)?.click.max_charges < 0;
                    let target_id = if live
                        .game
                        .spell_catalog
                        .spells
                        .get(&spell_id)
                        .is_some_and(|spell| spell.target_type == 6)
                    {
                        own
                    } else {
                        live.target.unwrap_or(own)
                    };
                    if live.game.sitting
                        && !live.command(Command::Posture {
                            player_id: own,
                            posture: 0,
                        })
                    {
                        return Err("Could not stand to use this item.".into());
                    }
                    live.game.item_use.pending = Some(PendingUse {
                        item: identity,
                        deadline: now + Duration::from_secs(8),
                        kind: PendingKind::Click {
                            spell_id,
                            target_id,
                            queued: false,
                            sent: false,
                            began: false,
                            applied: false,
                            finished: false,
                            consumed: unlimited,
                        },
                    });
                    live.game.item_use.last_item = Some(identity);
                    live.game.item_use.status = "Waiting for the item cast…".into();
                    if !live.command(Command::ItemUse(ItemUseCommand::Click {
                        slot: identity.slot,
                        target_id,
                    })) {
                        live.game.item_use.fail("The item request was not sent.");
                        return Err(live.game.item_use.status.clone());
                    }
                    live.game.cast_pending_until = Some(now + Duration::from_secs(3));
                }
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            live.game.error(error);
        }
    }

    pub(crate) fn item_use_tick(&mut self, live: &mut LiveWorld) {
        if !live.ready || live.error.is_some() {
            live.item_use_reset();
            return;
        }
        if live.game.item_use.expire(Instant::now()) {
            live.game.cast_pending_until = None;
            live.game.error(live.game.item_use.status.clone());
        }
        if let Some(item) = self
            .inspected_owned
            .map(OwnedItem::from)
            .and_then(|identity| owned_item(&live.game, identity).ok())
        {
            self.inspected_item = Some(game::item_view(item));
        }
    }
}

impl LiveWorld {
    pub(crate) fn item_use_command_allowed(&mut self, command: &Command) -> bool {
        let allowed = self.game.item_use.allows(command);
        if !allowed {
            self.game
                .error("Finish the current item action before starting another.");
        }
        allowed
    }
    pub(crate) fn item_use_queued(&mut self, command: &Command) {
        let Some(pending) = &mut self.game.item_use.pending else {
            return;
        };
        match (&mut pending.kind, command) {
            (
                PendingKind::Scribe { phase, .. },
                Command::ItemUse(ItemUseCommand::Scribe { .. }),
            ) if *phase == ScribePhase::Queued => *phase = ScribePhase::Sending,
            (PendingKind::Click { queued, .. }, Command::ItemUse(ItemUseCommand::Click { .. })) => {
                *queued = true
            }
            _ => {}
        }
    }
    pub(crate) fn item_use_command_sent(&mut self, command: &Command) {
        let Some(pending) = self.game.item_use.pending.clone() else {
            return;
        };
        match (&pending.kind, command) {
            (
                PendingKind::Scribe {
                    phase: ScribePhase::Moving,
                    spell_id,
                    book_slot,
                    ..
                },
                Command::MoveItem { from, to, count },
            ) if *from == pending.item.slot && *to == InventorySlot::CURSOR && *count <= 1 => {
                let plan = scribe_plan(&self.game, pending.item.on_cursor());
                if plan
                    .as_ref()
                    .is_ok_and(|(spell, slot)| spell == spell_id && slot == book_slot)
                {
                    if let Some(PendingUse {
                        kind: PendingKind::Scribe { phase, .. },
                        ..
                    }) = &mut self.game.item_use.pending
                    {
                        *phase = ScribePhase::Queued;
                    }
                    self.game.item_use.status = "Scribing; waiting for the server…".into();
                    if !self.command(Command::ItemUse(ItemUseCommand::Scribe {
                        spell_id: *spell_id,
                        book_slot: *book_slot,
                    })) {
                        self.game.item_use.fail(
                            "The scribe request was not sent. The scroll remains on your cursor.",
                        );
                    }
                } else {
                    let message = plan.err().unwrap_or_else(|| {
                        "Your spellbook changed. Inspect the scroll on your cursor again.".into()
                    });
                    self.game.item_use.fail(message.clone());
                    self.game.error(message);
                }
            }
            (PendingKind::Scribe { .. }, Command::ItemUse(ItemUseCommand::Scribe { .. })) => {
                if let Some(PendingUse {
                    kind: PendingKind::Scribe { phase, .. },
                    ..
                }) = &mut self.game.item_use.pending
                {
                    *phase = ScribePhase::Waiting;
                }
            }
            (PendingKind::Click { .. }, Command::ItemUse(ItemUseCommand::Click { .. })) => {
                if let Some(PendingUse {
                    kind: PendingKind::Click { sent, .. },
                    ..
                }) = &mut self.game.item_use.pending
                {
                    *sent = true;
                }
            }
            _ => {}
        }
    }
    pub(crate) fn item_use_command_rejected(&mut self, command: &Command) {
        let relevant = self.game.item_use.pending.as_ref().is_some_and(|pending| {
            match (&pending.kind, command) {
                (
                    PendingKind::Scribe {
                        phase: ScribePhase::Moving,
                        ..
                    },
                    Command::MoveItem { from, to, .. },
                ) => *from == pending.item.slot && *to == InventorySlot::CURSOR,
                (
                    PendingKind::Scribe {
                        spell_id,
                        book_slot,
                        ..
                    },
                    Command::ItemUse(ItemUseCommand::Scribe {
                        spell_id: spell,
                        book_slot: slot,
                    }),
                ) => spell_id == spell && book_slot == slot,
                (
                    PendingKind::Click { target_id, .. },
                    Command::ItemUse(ItemUseCommand::Click {
                        slot,
                        target_id: target,
                    }),
                ) => pending.item.slot == *slot && target_id == target,
                _ => false,
            }
        });
        if relevant {
            self.game.item_use.fail("The item action was not sent.");
            self.game.cast_pending_until = None;
        }
    }
    pub(crate) fn item_use_event(&mut self, event: &GameplayEvent) {
        if let Some(notice) = self
            .game
            .item_use
            .observe(event, self.own_id, Instant::now())
        {
            self.game.cast_pending_until = None;
            self.game.notice(notice);
        }
    }
    pub(crate) fn item_use_reset(&mut self) {
        self.game.item_use.reset();
        self.game.cast_pending_until = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        item_use_state::tests::{fixture, identity, item},
        live::{NetworkCommand, tests::command_world},
    };

    #[test]
    fn nonstackable_scroll_moves_whole_then_waits_for_server_scribe_results() {
        let (mut live, mut commands) = command_world(1, 10.);
        live.game = fixture();
        live.game.sitting = true;
        let mut scroll = item(InventorySlot::possessions(23));
        scroll.stack_size = 1;
        scroll.count = 1;
        let selected = identity(&scroll);
        live.game.inventory.insert(scroll);
        let mut ui = Interaction {
            inspected_owned: Some((selected.slot, selected.id, selected.instance_id)),
            ..Default::default()
        };
        assert!(ui.view(&live).item_use.unwrap().can_scribe);
        ui.item_use_action(&mut live, ItemUseAction::Scribe);
        let NetworkCommand::Gameplay(movement, _) = commands.try_recv().unwrap() else {
            panic!("move command")
        };
        assert!(matches!(movement, Command::MoveItem { count: 0, .. }));
        assert!(commands.try_recv().is_err());
        live.command_sent(movement);
        assert!(!live.game.inventory.items.contains_key(&selected.slot));
        assert_eq!(
            live.game.inventory.items[&InventorySlot::CURSOR].instance_id,
            selected.instance_id
        );
        let NetworkCommand::Gameplay(scribe, _) = commands.try_recv().unwrap() else {
            panic!("scribe command")
        };
        assert!(matches!(
            scribe,
            Command::ItemUse(ItemUseCommand::Scribe {
                spell_id: 288,
                book_slot: 0
            })
        ));
        live.command_sent(scribe);
        assert!(live.game.item_use.busy());
        assert!(
            live.game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
        );
        assert!(
            !live
                .game
                .profile
                .as_ref()
                .unwrap()
                .spell_book
                .contains(&288)
        );
    }

    #[test]
    fn scroll_stack_moves_one_and_only_scribes_after_command_sent() {
        let (mut live, mut commands) = command_world(1, 10.);
        live.game = fixture();
        let scroll = item(InventorySlot::possessions(23));
        let selected = identity(&scroll);
        live.game.inventory.insert(scroll);
        let mut ui = Interaction {
            inspected_owned: Some((selected.slot, selected.id, selected.instance_id)),
            ..Default::default()
        };
        assert!(ui.view(&live).item_use.unwrap().can_scribe);
        ui.item_use_action(&mut live, ItemUseAction::Scribe);
        let NetworkCommand::Gameplay(posture, _) = commands.try_recv().unwrap() else {
            panic!("posture command")
        };
        live.command_sent(posture);
        let NetworkCommand::Gameplay(movement, _) = commands.try_recv().unwrap() else {
            panic!("move command")
        };
        assert!(matches!(
            movement,
            Command::MoveItem {
                to: InventorySlot::CURSOR,
                count: 1,
                ..
            }
        ));
        assert!(
            commands.try_recv().is_err(),
            "scribe must wait for actual move transmission"
        );
        assert_eq!(live.game.inventory.items[&selected.slot].count, 3);
        assert!(!live.command(Command::MoveItem {
            from: selected.slot,
            to: InventorySlot::possessions(26),
            count: 1
        }));
        live.command_sent(movement);
        assert_eq!(live.game.inventory.items[&selected.slot].count, 2);
        assert_eq!(live.game.inventory.items[&InventorySlot::CURSOR].count, 1);
        let NetworkCommand::Gameplay(scribe, _) = commands.try_recv().unwrap() else {
            panic!("scribe command")
        };
        assert!(matches!(
            scribe,
            Command::ItemUse(ItemUseCommand::Scribe {
                book_slot: 0,
                spell_id: 288
            })
        ));
        assert!(
            !live.command(scribe.clone()),
            "cannot enqueue twice before transmission"
        );
        live.command_sent(scribe);
        live.gameplay_event(GameplayEvent::SpellMemorized {
            slot: 0,
            spell_id: 288,
            action: 0,
            reduction: 0,
        });
        assert!(
            live.game.item_use.busy(),
            "book acknowledgment does not consume cursor locally"
        );
        assert!(
            live.game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
        );
        live.gameplay_event(GameplayEvent::ItemMoved {
            from: InventorySlot::CURSOR,
            to: InventorySlot::DELETE,
            count: u32::MAX,
        });
        assert!(!live.game.item_use.busy());
        assert_eq!(live.game.profile.as_ref().unwrap().spell_book[0], 288);
        assert_eq!(live.game.inventory.items[&selected.slot].count, 2);
        assert!(
            !ui.view(&live).item_use.unwrap().can_scribe,
            "known spell cannot consume another scroll"
        );
    }

    #[test]
    fn rejected_move_never_scribes_and_stale_inspection_never_sends() {
        let (mut live, mut commands) = command_world(1, 10.);
        live.game = fixture();
        live.game.sitting = true;
        let scroll = item(InventorySlot::possessions(23));
        let selected = identity(&scroll);
        live.game.inventory.insert(scroll);
        let mut ui = Interaction {
            inspected_owned: Some((selected.slot, selected.id, selected.instance_id + 1)),
            ..Default::default()
        };
        ui.item_use_action(&mut live, ItemUseAction::Scribe);
        assert!(commands.try_recv().is_err());
        ui.inspected_owned = Some((selected.slot, selected.id, selected.instance_id));
        ui.item_use_action(&mut live, ItemUseAction::Scribe);
        let NetworkCommand::Gameplay(movement, _) = commands.try_recv().unwrap() else {
            panic!("move")
        };
        live.command_rejected(movement, "zone changed".into());
        assert!(!live.game.item_use.busy());
        assert!(!live.game.inventory_command_pending);
        assert!(commands.try_recv().is_err());
        assert_eq!(live.game.inventory.items[&selected.slot].count, 3);
        assert!(
            !live
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
        );
    }

    #[test]
    fn only_owned_inspections_get_actions_and_services_disable_them() {
        let (mut live, _) = command_world(1, 10.);
        live.game = fixture();
        let scroll = item(InventorySlot::possessions(23));
        let selected = identity(&scroll);
        live.game.inventory.insert(scroll.clone());
        let mut ui = Interaction {
            inspected_item: Some(game::item_view(&scroll)),
            ..Default::default()
        };
        assert!(
            ui.view(&live).item_use.is_none(),
            "linked or merchant inspections are read only"
        );
        ui.inspected_owned = Some((selected.slot, selected.id, selected.instance_id));
        let view = ui.view(&live);
        assert!(
            view.inspected_item
                .unwrap()
                .details
                .iter()
                .any(|text| text.contains("Minor Shielding"))
        );
        assert!(view.item_use.unwrap().can_scribe);
        live.game.commerce.bank = Some(crate::commerce::BankSession {
            id: 2,
            name: "Bank".into(),
        });
        assert!(!ui.view(&live).item_use.unwrap().can_scribe);
        live.game.commerce.bank = None;
        live.game.commerce.merchant_closing = true;
        assert!(!ui.view(&live).item_use.unwrap().can_scribe);
    }
}
