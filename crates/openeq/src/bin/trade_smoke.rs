//! Two-player proof through the windowed client's UI actions and live reducers.
//! Mutates only the isolated Barterer/Swapper trading fixtures, then restores them.
use anyhow::{Context, ensure};
use openeq::{
    game::StringTable,
    gameplay_ui::{TradeAction, UiAction},
    interaction::Interaction,
    live::LiveWorld,
    trade::TradePhase,
};
use openeq_net::{
    gameplay::{CoinType, Command, Currency},
    inventory::InventorySlot,
    session::ConnectionConfig,
    trade::TradeCommand,
};
use openeq_render::Camera;
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

struct Client {
    live: LiveWorld,
    ui: Interaction,
}
type Snapshot = (BTreeMap<u32, u32>, [u32; 4]);
fn money(c: Currency) -> [u32; 4] {
    [c.platinum, c.gold, c.silver, c.copper]
}
fn inventory(live: &LiveWorld) -> BTreeMap<u32, u32> {
    let mut out = BTreeMap::new();
    for item in live
        .game
        .inventory
        .items
        .values()
        .filter(|item| item.slot.kind == 0)
    {
        *out.entry(item.id).or_default() += if item.stack_size > 1 {
            item.count.max(1)
        } else {
            1
        };
    }
    out
}
fn snapshot(live: &LiveWorld) -> Snapshot {
    (inventory(live), money(live.game.currency))
}
fn phase(live: &LiveWorld) -> Option<TradePhase> {
    live.game.trade.session.as_ref().map(|s| s.phase)
}
fn idle(live: &LiveWorld) -> bool {
    !live.game.inventory_command_pending
        && live.game.trade.session.as_ref().is_none_or(|s| !s.pending)
}
fn poll(client: &mut Client) -> anyhow::Result<()> {
    client.live.poll();
    ensure!(
        client.live.error.is_none(),
        "{} connection failed: {:?}",
        client.live.character,
        client.live.error
    );
    if let Some(p) = client.live.initial_position.take() {
        client.live.camera_position(
            &Camera {
                position: [p.x, p.y, p.z + 3.],
                yaw: p.heading * std::f32::consts::TAU / 512.,
                ..Default::default()
            },
            false,
        );
    }
    client.ui.tick(&mut client.live);
    Ok(())
}
fn wait(
    clients: &mut [Client],
    label: &str,
    condition: impl Fn(&[Client]) -> bool,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        for client in clients.iter_mut() {
            poll(client)?;
        }
        if condition(clients) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timeout {label}: {:?}",
            clients
                .iter()
                .map(|c| (
                    c.live.character.as_str(),
                    phase(&c.live),
                    c.live.game.trade.session.as_ref().map(|s| (
                        &s.status,
                        s.pending,
                        s.cancel_sent
                    )),
                    c.live
                        .game
                        .chat
                        .iter()
                        .rev()
                        .take(4)
                        .map(|line| line.text.as_str())
                        .collect::<Vec<_>>()
                ))
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn settle(clients: &mut [Client]) -> anyhow::Result<()> {
    let start = Instant::now();
    wait(clients, "settling", |_| {
        start.elapsed() >= Duration::from_millis(700)
    })
}
fn connect(configs: &[ConnectionConfig; 2], base: &Path) -> anyhow::Result<Vec<Client>> {
    let mut clients: Vec<_> = configs
        .iter()
        .map(|config| {
            let mut live = LiveWorld::start(config.clone());
            live.game.strings = StringTable::load(base);
            Client {
                live,
                ui: Interaction::default(),
            }
        })
        .collect();
    wait(&mut clients, "both client inventories", |clients| {
        clients.iter().all(|c| {
            c.live.ready
                && c.live.own_id.is_some()
                && c.live.game.inventory.received
                && c.live.game.profile.is_some()
        })
    })?;
    settle(&mut clients)?;
    let ids = [
        clients[0].live.own_id.unwrap(),
        clients[1].live.own_id.unwrap(),
    ];
    ensure!(
        clients[0].live.trade_player_available(ids[1])
            && clients[1].live.trade_player_available(ids[0]),
        "fixture players not within trade range"
    );
    clients[0].live.set_target(Some(ids[1]));
    clients[1].live.set_target(Some(ids[0]));
    Ok(clients)
}
fn reconnect(
    clients: Vec<Client>,
    configs: &[ConnectionConfig; 2],
    base: &Path,
) -> anyhow::Result<Vec<Client>> {
    drop(clients);
    std::thread::sleep(Duration::from_secs(2));
    connect(configs, base)
}
fn action(client: &mut Client, action: UiAction) {
    let position = client.live.player_position().unwrap_or([0.; 3]);
    client
        .ui
        .ui_action(action, false, false, &mut client.live, position);
}
fn trade(client: &mut Client, action_value: TradeAction) {
    action(client, UiAction::Trade(action_value));
}
fn invite(clients: &mut [Client]) -> anyhow::Result<()> {
    let a = &mut clients[0];
    let position = a.live.player_position().context("own position")?;
    a.ui.submit("/trade", &mut a.live, position);
    wait(clients, "trade invitation", |c| {
        phase(&c[0].live) == Some(TradePhase::Waiting)
            && phase(&c[1].live) == Some(TradePhase::Invitation)
    })
}
fn start(clients: &mut [Client]) -> anyhow::Result<()> {
    invite(clients)?;
    trade(&mut clients[1], TradeAction::AcceptInvite);
    wait(clients, "acknowledged trade", |c| {
        c.iter()
            .all(|p| phase(&p.live) == Some(TradePhase::Active) && idle(&p.live))
    })
}
fn offer(clients: &mut [Client], who: usize, item_id: u32) -> anyhow::Result<()> {
    let slot = clients[who]
        .live
        .game
        .inventory
        .items
        .values()
        .find(|item| item.slot.kind == 0 && item.id == item_id)
        .context("owned offer item")?
        .slot
        .server_slot()
        .context("canonical item slot")?;
    action(&mut clients[who], UiAction::InventorySlot(slot as i32));
    wait(clients, "cursor prediction", |c| {
        idle(&c[who].live)
            && c[who]
                .live
                .game
                .inventory
                .items
                .get(&InventorySlot::CURSOR)
                .is_some_and(|i| i.id == item_id)
    })?;
    trade(&mut clients[who], TradeAction::OwnSlot(0));
    wait(clients, "escrow and partner item display", |c| {
        idle(&c[who].live)
            && c[who]
                .live
                .game
                .inventory
                .items
                .get(&InventorySlot::trade(0))
                .is_some_and(|i| i.id == item_id)
            && c[1 - who]
                .live
                .game
                .trade
                .session
                .as_ref()
                .is_some_and(|s| s.partner_items.get(&0).is_some_and(|i| i.id == item_id))
    })?;
    ensure!(
        !clients[who]
            .live
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::CURSOR),
        "cursor duplicated offered item"
    );
    ensure!(
        !clients[1 - who]
            .live
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::possessions(0)),
        "remote TradeView corrupted own worn slot0"
    );
    ensure!(
        !clients[1 - who]
            .live
            .game
            .inventory
            .items
            .values()
            .any(|i| i.slot.kind == 0 && i.id == item_id),
        "remote offer became owned inventory"
    );
    Ok(())
}
fn coins(clients: &mut [Client], who: usize, amount: u32) -> anyhow::Result<()> {
    let before = clients[who].live.game.currency.platinum;
    trade(&mut clients[who], TradeAction::Coin(3));
    trade(&mut clients[who], TradeAction::Quantity(amount as i32 - 1));
    trade(&mut clients[who], TradeAction::AddCoin);
    ensure!(
        clients[who].live.game.currency.platinum == before,
        "coin debit happened before sent callback"
    );
    // The same visible button cannot queue a second debit while its first send is pending.
    trade(&mut clients[who], TradeAction::AddCoin);
    wait(clients, "coin debit and partner delta", |c| {
        idle(&c[who].live)
            && c[who]
                .live
                .game
                .trade
                .session
                .as_ref()
                .is_some_and(|s| s.own_money.platinum == amount)
            && c[1 - who]
                .live
                .game
                .trade
                .session
                .as_ref()
                .is_some_and(|s| s.partner_money.platinum == amount)
    })?;
    ensure!(
        clients[who].live.game.currency.platinum == before - amount,
        "coin debit duplicated"
    );
    settle(clients)?;
    ensure!(
        clients[who].live.game.currency.platinum == before - amount,
        "coin event debited local money again"
    );
    Ok(())
}
fn ended(clients: &[Client]) -> bool {
    clients.iter().all(|c| {
        phase(&c.live) == Some(TradePhase::Ended)
            && idle(&c.live)
            && !c
                .live
                .game
                .inventory
                .items
                .keys()
                .any(|slot| slot.kind == 3)
    })
}
fn assert_snapshots(clients: &[Client], expected: &[Snapshot; 2]) -> anyhow::Result<()> {
    for (index, client) in clients.iter().enumerate() {
        ensure!(
            snapshot(&client.live) == expected[index],
            "{} possessions differ: actual {:?}, expected {:?}",
            client.live.character,
            snapshot(&client.live),
            expected[index]
        );
    }
    Ok(())
}
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq_net=info")
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(args.len() == 2, "trade_smoke TRADE1_CONFIG TRADE2_CONFIG");
    let configs = [
        ConnectionConfig::load(Path::new(&args[0]))?,
        ConnectionConfig::load(Path::new(&args[1]))?,
    ];
    for (config, name, account) in [
        (&configs[0], "Barterer", "openeq_trade1"),
        (&configs[1], "Swapper", "openeq_trade2"),
    ] {
        ensure!(
            config.host == "storage2.daeken.dev"
                && config.character == name
                && config.username == account,
            "use isolated trade fixture configs"
        );
    }
    let base = std::path::PathBuf::from(std::env::var("HOME")?).join("EverQuest");
    let mut clients = connect(&configs, &base)?;
    let baseline = [snapshot(&clients[0].live), snapshot(&clients[1].live)];
    ensure!(
        baseline[0].0.get(&5001) == Some(&1) && baseline[1].0.get(&1001) == Some(&1),
        "dedicated items not restored before probe"
    );
    // Withdraw before B acknowledges. Both invitation windows must drain so a
    // late ACK cannot establish a different server session behind a new trade.
    invite(&mut clients)?;
    trade(&mut clients[0], TradeAction::Cancel);
    ensure!(
        phase(&clients[0].live) == Some(TradePhase::Completing),
        "withdrawal did not wait for peer response"
    );
    wait(&mut clients, "unacknowledged invitation withdrawn", ended)?;
    assert_snapshots(&clients, &baseline)?;
    start(&mut clients)?;
    println!(
        "PROOF: withdrawing an unacknowledged invitation closes both windows; immediate next trade succeeds"
    );
    offer(&mut clients, 0, 5001)?;
    offer(&mut clients, 1, 1001)?;
    coins(&mut clients, 0, 2)?;
    coins(&mut clients, 1, 3)?;
    trade(&mut clients[0], TradeAction::Cancel);
    ensure!(
        phase(&clients[0].live) == Some(TradePhase::Completing),
        "cancellation skipped settlement barrier"
    );
    ensure!(
        clients[0]
            .live
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::trade(0)),
        "cancel fabricated an immediate refund"
    );
    trade(&mut clients[0], TradeAction::Cancel);
    let ids = [
        clients[0].live.own_id.unwrap(),
        clients[1].live.own_id.unwrap(),
    ];
    ensure!(
        !clients[0]
            .live
            .command(Command::Trade(TradeCommand::Request {
                to_id: ids[1],
                from_id: ids[0]
            })),
        "new trade bypassed cancellation barrier"
    );
    wait(&mut clients, "two-sided cancellation", ended)?;
    assert_snapshots(&clients, &baseline)?;
    settle(&mut clients)?;
    assert_snapshots(&clients, &baseline)?;
    clients = reconnect(clients, &configs, &base)?;
    assert_snapshots(&clients, &baseline)?;
    println!(
        "PROOF: native UI cancel/refund and callback barriers preserve both inventories; no coin double debit; reconnect matches"
    );

    start(&mut clients)?;
    offer(&mut clients, 0, 5001)?;
    offer(&mut clients, 1, 1001)?;
    coins(&mut clients, 0, 2)?;
    trade(&mut clients[0], TradeAction::Accept);
    wait(&mut clients, "first acceptance", |c| {
        c[0].live
            .game
            .trade
            .session
            .as_ref()
            .is_some_and(|s| s.you_accepted)
            && c[1]
                .live
                .game
                .trade
                .session
                .as_ref()
                .is_some_and(|s| s.partner_accepted)
    })?;
    let accepted_snapshot = snapshot(&clients[0].live);
    let accepted = &mut clients[0];
    ensure!(
        phase(&accepted.live) == Some(TradePhase::Active),
        "first acceptance should still await the peer"
    );
    ensure!(
        !accepted
            .live
            .command(Command::Trade(TradeCommand::OfferCoin {
                coin: CoinType::Platinum,
                amount: 1,
            })),
        "accepted offer still permits coin mutation"
    );
    ensure!(
        !accepted.live.command(Command::MoveItem {
            from: InventorySlot::possessions(24),
            to: InventorySlot::CURSOR,
            count: 0,
        }),
        "accepted offer still permits inventory moves"
    );
    ensure!(
        !accepted.live.command(Command::DeleteItem {
            slot: InventorySlot::possessions(24),
            count: 1,
        }),
        "accepted offer still permits item destruction"
    );
    ensure!(
        accepted
            .ui
            .view(&accepted.live)
            .trade
            .is_some_and(|view| !view.can_modify && !view.can_add_coin),
        "accepted trade UI still enables mutation"
    );
    settle(&mut clients)?;
    ensure!(
        snapshot(&clients[0].live) == accepted_snapshot,
        "blocked post-accept mutations changed possessions"
    );
    coins(&mut clients, 1, 3)?;
    println!(
        "PROOF: accepted player cannot alter coins, move or destroy items; remote offer changes restore review"
    );
    ensure!(
        clients.iter().all(|c| c
            .live
            .game
            .trade
            .session
            .as_ref()
            .is_some_and(|s| !s.you_accepted && !s.partner_accepted)),
        "offer change left acceptance set"
    );
    trade(&mut clients[1], TradeAction::Accept);
    wait(&mut clients, "only second player accepted", |c| {
        c[1].live
            .game
            .trade
            .session
            .as_ref()
            .is_some_and(|s| s.you_accepted)
            && c[0]
                .live
                .game
                .trade
                .session
                .as_ref()
                .is_some_and(|s| s.partner_accepted)
    })?;
    ensure!(
        clients
            .iter()
            .all(|c| phase(&c.live) == Some(TradePhase::Active)),
        "stale acceptance completed trade"
    );
    trade(&mut clients[0], TradeAction::Accept);
    wait(&mut clients, "exchange settlement", ended)?;
    let exchanged = [snapshot(&clients[0].live), snapshot(&clients[1].live)];
    ensure!(
        exchanged[0].0.get(&1001) == Some(&1)
            && !exchanged[0].0.contains_key(&5001)
            && exchanged[1].0.get(&5001) == Some(&1)
            && !exchanged[1].0.contains_key(&1001),
        "exchange delivery incorrect"
    );
    ensure!(
        exchanged[0].1[0] == baseline[0].1[0] + 1 && exchanged[1].1[0] + 1 == baseline[1].1[0],
        "exchange coin delta incorrect"
    );
    clients = reconnect(clients, &configs, &base)?;
    assert_snapshots(&clients, &exchanged)?;
    println!(
        "PROOF: native acceptance resets on changed offer; actual item+coin exchange survives reconnect"
    );

    start(&mut clients)?;
    offer(&mut clients, 0, 1001)?;
    offer(&mut clients, 1, 5001)?;
    coins(&mut clients, 0, 3)?;
    coins(&mut clients, 1, 2)?;
    trade(&mut clients[0], TradeAction::Accept);
    trade(&mut clients[1], TradeAction::Accept);
    wait(&mut clients, "reverse exchange", ended)?;
    clients = reconnect(clients, &configs, &base)?;
    assert_snapshots(&clients, &baseline)?;
    println!("PROOF: reverse exchange restored both initial inventories and currency");

    start(&mut clients)?;
    offer(&mut clients, 0, 5001)?;
    offer(&mut clients, 1, 1001)?;
    coins(&mut clients, 0, 2)?;
    coins(&mut clients, 1, 3)?;
    drop(clients.remove(0));
    wait(&mut clients, "partner disconnect and refund", ended)?;
    ensure!(
        snapshot(&clients[0].live) == baseline[1],
        "surviving player's disconnect refund incorrect"
    );
    clients = reconnect(clients, &configs, &base)?;
    assert_snapshots(&clients, &baseline)?;
    println!(
        "PROOF: partner disconnect closes native trade and refunds persisted; fixtures restored"
    );
    Ok(())
}
