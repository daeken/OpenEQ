use super::*;
use crate::commerce::{BANKER_CLASS, BankSession, MERCHANT_CLASS, MerchantSession};
use openeq_net::{
    death::{BindTransfer, DeathEvent, RespawnOption, RespawnWindow},
    gameplay::{CoinLocation, CoinType, Currency, Death},
    inventory::InventorySlot,
};

#[derive(Clone, Copy, Debug)]
enum Action {
    Coin,
    Buy,
    Sell,
}

const ACTIONS: [Action; 3] = [Action::Coin, Action::Buy, Action::Sell];
const CARRIED_SLOT: InventorySlot = InventorySlot::possessions(23);

impl Action {
    fn class(self) -> u8 {
        match self {
            Self::Coin => BANKER_CLASS,
            Self::Buy | Self::Sell => MERCHANT_CLASS,
        }
    }

    fn open(self, live: &mut LiveWorld) {
        match self {
            Self::Coin => {
                live.game.commerce.bank = Some(BankSession {
                    id: 2,
                    name: "Banker".into(),
                });
            }
            Self::Buy | Self::Sell => {
                let mut merchant = MerchantSession::new(2, "Merchant".into());
                merchant.opened = true;
                merchant.items.insert(5, tests::carried_item(CARRIED_SLOT));
                live.game.commerce.merchant = Some(merchant);
            }
        }
    }

    fn command(self, live: &LiveWorld) -> Command {
        match self {
            Self::Coin => Command::MoveCoin {
                from: CoinLocation::Carried,
                to: CoinLocation::Bank,
                coin: CoinType::Platinum,
                amount: 2,
            },
            Self::Buy => Command::MerchantBuy {
                merchant_id: 2,
                player_id: live.own_id.unwrap(),
                slot: 5,
                quantity: 2,
                price: 40,
            },
            Self::Sell => Command::MerchantSell {
                merchant_id: 2,
                slot: CARRIED_SLOT,
                quantity: 2,
            },
        }
    }

    fn assert_queued(self, live: &LiveWorld) {
        match self {
            Self::Coin => assert!(live.game.commerce.coin_pending),
            Self::Buy | Self::Sell => {
                let pending = live.game.commerce.pending.as_ref().unwrap();
                assert_eq!(pending.merchant_id, 2);
                assert!(!pending.sent, "queued transaction is not dispatched");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    Death,
    Respawn,
    Bind,
}

const BOUNDARIES: [Boundary; 3] = [Boundary::Death, Boundary::Respawn, Boundary::Bind];

impl Boundary {
    fn apply(self, live: &mut LiveWorld) {
        let event = match self {
            Self::Death => GameplayEvent::Death(Death {
                id: 1,
                killer_id: 99,
                corpse_id: 1,
                skill: 0,
                spell_id: u32::MAX,
                damage: 100,
            }),
            Self::Respawn => {
                // Exercise this boundary's cleanup independently of Death's.
                live.game.recovery.own_death(live.zone_generation, 1);
                GameplayEvent::Recovery(DeathEvent::RespawnWindow(RespawnWindow {
                    initial_selection: 17,
                    remaining_ms: 60_000,
                    options: vec![RespawnOption {
                        id: 17,
                        zone_id: 54,
                        position: [0.; 3],
                        heading: 0.,
                        label: "Bind Location".into(),
                        requires_resurrection: false,
                    }],
                }))
            }
            Self::Bind => GameplayEvent::Recovery(DeathEvent::BindTransfer(BindTransfer {
                zone_id: 54,
                instance_id: 0,
                position: [0.; 3],
                heading: 0.,
                label: "Bind Location".into(),
                save_items: 1,
                resources: [0; 3],
            })),
        };
        live.gameplay_event(event);
        assert!(!live.movement_allowed());
        assert!(live.game.commerce.bank.is_none());
        assert!(live.game.commerce.merchant.is_none());
    }
}

fn fixture(
    action: Action,
) -> (
    LiveWorld,
    tokio::sync::mpsc::UnboundedReceiver<NetworkCommand>,
    mpsc::Sender<Message>,
) {
    let (mut live, commands) = tests::command_world(action.class(), 10.);
    let (events, received) = mpsc::channel();
    live.rx = Mutex::new(received);
    live.game
        .inventory
        .insert(tests::carried_item(CARRIED_SLOT));
    action.open(&mut live);
    (live, commands, events)
}

fn queued(
    live: &mut LiveWorld,
    commands: &mut tokio::sync::mpsc::UnboundedReceiver<NetworkCommand>,
    action: Action,
) -> (Command, u64) {
    assert!(live.command(action.command(live)), "queue {action:?}");
    action.assert_queued(live);
    let NetworkCommand::Gameplay(command, epoch) = commands.try_recv().unwrap() else {
        panic!("expected gameplay command");
    };
    (command, epoch)
}

fn callback(command: Command, epoch: u64, sent: bool) -> Message {
    if sent {
        Message::CommandSent { command, epoch }
    } else {
        Message::CommandRejected {
            command,
            epoch,
            notice: "retired command was not sent".into(),
        }
    }
}

fn revive(live: &mut LiveWorld, events: &mpsc::Sender<Message>) {
    let mut player = live.entities[&2].spawn.clone();
    player.id = 3;
    player.name = live.character.clone();
    player.npc = false;
    player.is_corpse = false;
    player.class = 1;
    player.position = Position::default();
    events
        .send(Message::Event(Box::new(ZoneEvent::Spawn(player))))
        .unwrap();
    live.poll();
    assert!(live.movement_allowed());
    assert_eq!(live.own_id, Some(3));
}

fn assert_unchanged_assets(live: &LiveWorld) {
    assert_eq!(live.game.currency.platinum, 10);
    assert_eq!(live.game.commerce.bank_money.platinum, 10);
    assert_eq!(live.game.inventory.items[&CARRIED_SLOT].count, 5);
}

#[test]
fn recovery_retires_unsent_commerce_without_waiting_for_stale_rejection() {
    for action in ACTIONS {
        for boundary in BOUNDARIES {
            let (mut live, mut commands, events) = fixture(action);
            let (command, epoch) = queued(&mut live, &mut commands, action);
            boundary.apply(&mut live);
            assert_ne!(live.movement_authority.action_epoch, epoch);
            assert!(!live.game.commerce.coin_pending, "{action:?}/{boundary:?}");
            assert!(
                live.game.commerce.pending.is_none(),
                "{action:?}/{boundary:?}"
            );
            events.send(callback(command, epoch, false)).unwrap();
            live.poll();
            assert_unchanged_assets(&live);
            revive(&mut live, &events);
            action.open(&mut live);
            queued(&mut live, &mut commands, action);
        }
    }
}

#[test]
fn retired_commerce_callbacks_cannot_release_or_dispatch_new_recovery_actions() {
    for action in ACTIONS {
        for boundary in BOUNDARIES {
            for sent in [false, true] {
                let (mut live, mut commands, events) = fixture(action);
                let (old_command, old_epoch) = queued(&mut live, &mut commands, action);
                boundary.apply(&mut live);
                revive(&mut live, &events);
                action.open(&mut live);
                let (new_command, new_epoch) = queued(&mut live, &mut commands, action);
                assert_ne!(old_epoch, new_epoch);
                events.send(callback(old_command, old_epoch, sent)).unwrap();
                live.poll();
                action.assert_queued(&live);
                assert_unchanged_assets(&live);
                events
                    .send(callback(new_command, new_epoch, false))
                    .unwrap();
                live.poll();
                assert!(!live.game.commerce.coin_pending);
                assert!(live.game.commerce.pending.is_none());
                assert_unchanged_assets(&live);
            }
        }
    }
}

#[test]
fn recovery_retains_dispatched_merchants_and_reconciles_delayed_ack_once() {
    for action in [Action::Buy, Action::Sell] {
        for boundary in BOUNDARIES {
            let (mut live, mut commands, events) = fixture(action);
            let (command, epoch) = queued(&mut live, &mut commands, action);
            events.send(callback(command, epoch, true)).unwrap();
            live.poll();
            assert!(live.game.commerce.pending.as_ref().unwrap().sent);
            boundary.apply(&mut live);
            assert!(live.game.commerce.pending.as_ref().unwrap().sent);
            assert_unchanged_assets(&live);
            let ack = match action {
                Action::Buy => GameplayEvent::MerchantBought {
                    merchant_id: 2,
                    player_id: 1,
                    slot: 5,
                    quantity: 2,
                    price: 40,
                },
                Action::Sell => GameplayEvent::MerchantSold {
                    merchant_id: 2,
                    slot: CARRIED_SLOT,
                    quantity: 2,
                    price: 40,
                    rejected: false,
                },
                Action::Coin => unreachable!(),
            };
            live.gameplay_event(ack.clone());
            live.gameplay_event(ack);
            assert!(live.game.commerce.pending.is_none());
            match action {
                Action::Buy => {
                    assert_eq!(crate::commerce::total_copper(live.game.currency), 9960);
                    assert_eq!(live.game.inventory.items[&CARRIED_SLOT].count, 5);
                    assert!(live.game.commerce.currency_ready);
                }
                Action::Sell => {
                    assert_eq!(live.game.inventory.items[&CARRIED_SLOT].count, 3);
                    assert!(!live.game.commerce.currency_ready);
                    live.gameplay_event(GameplayEvent::Currency(Currency {
                        copper: 10040,
                        ..Default::default()
                    }));
                    assert_eq!(crate::commerce::total_copper(live.game.currency), 10040);
                    assert!(live.game.commerce.currency_ready);
                }
                Action::Coin => unreachable!(),
            }
        }
    }
}
