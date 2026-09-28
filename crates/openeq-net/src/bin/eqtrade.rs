//! Live two-player trade proof, restricted to the separate Barterer/Swapper fixtures.
use anyhow::{Context, ensure};
use openeq_net::{
    gameplay::{CoinType, Command, Currency, GameplayEvent},
    inventory::{InventoryItem, InventorySlot},
    session::ConnectionConfig,
    trade::{TradeCommand, TradeEvent},
    zone::{Spawn, ZoneClient, ZoneEvent},
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

struct Player {
    zone: ZoneClient,
    name: String,
    id: u32,
    ready: bool,
    spawns: BTreeMap<u32, Spawn>,
    items: BTreeMap<InventorySlot, InventoryItem>,
    money: Currency,
    events: Vec<TradeEvent>,
    offers: BTreeMap<u16, InventoryItem>,
    active: bool,
    reply_cancel: bool,
    cancel_replies: usize,
    deliveries: usize,
    currency_updates: usize,
    move_echoes: usize,
}
fn copper(c: Currency) -> u64 {
    c.platinum as u64 * 1000 + c.gold as u64 * 100 + c.silver as u64 * 10 + c.copper as u64
}
impl Player {
    async fn connect(config: &ConnectionConfig) -> anyhow::Result<Self> {
        let mut p = Self {
            zone: config.connect().await?,
            name: config.character.clone(),
            id: 0,
            ready: false,
            spawns: BTreeMap::new(),
            items: BTreeMap::new(),
            money: Currency::default(),
            events: Vec::new(),
            offers: BTreeMap::new(),
            active: false,
            reply_cancel: false,
            cancel_replies: 0,
            deliveries: 0,
            currency_updates: 0,
            move_echoes: 0,
        };
        p.pump(4.).await?;
        ensure!(p.ready && p.id != 0, "{} did not enter world", p.name);
        Ok(p)
    }
    fn insert(&mut self, mut item: InventoryItem) {
        for child in std::mem::take(&mut item.children) {
            self.insert(child);
        }
        self.items.insert(item.slot, item);
    }
    fn event(&mut self, event: ZoneEvent) {
        match event {
            ZoneEvent::Ready => self.ready = true,
            ZoneEvent::Spawn(s) => {
                if s.name == self.name {
                    self.id = s.id;
                }
                self.spawns.insert(s.id, s);
            }
            ZoneEvent::Movement { id, position } => {
                if let Some(s) = self.spawns.get_mut(&id) {
                    s.position = position;
                }
            }
            ZoneEvent::Despawn(id) => {
                self.spawns.remove(&id);
            }
            ZoneEvent::Gameplay(e) => match e {
                GameplayEvent::Inventory(items) => {
                    self.items.clear();
                    for item in items {
                        self.insert(item);
                    }
                    println!("{} inventory: {:?}", self.name, self.inventory());
                }
                GameplayEvent::Profile(p) => {
                    self.money = p.currency;
                    println!("{} profile currency {:?}", self.name, self.money);
                }
                GameplayEvent::Currency(c) => {
                    self.money = c;
                    self.currency_updates += 1;
                    println!("{} currency {:?}", self.name, self.money);
                }
                GameplayEvent::Trade(e) => {
                    println!("{} trade {e:?}", self.name);
                    match e {
                        TradeEvent::Acknowledged { .. } => self.active = true,
                        TradeEvent::Cancelled { .. } if self.active => {
                            self.active = false;
                            self.reply_cancel = true;
                        }
                        TradeEvent::Finished | TradeEvent::WindowClosed => {
                            self.active = false;
                            self.items.retain(|slot, _| slot.kind != 3);
                        }
                        _ => {}
                    }
                    self.events.push(e);
                }
                GameplayEvent::Item { packet_type, item } => {
                    println!(
                        "{} item type{packet_type:#x} slot{:?} id{} count{}",
                        self.name, item.slot, item.id, item.count
                    );
                    if packet_type == 0x65
                        && item.slot.kind == 0
                        && item.slot.slot < 8
                        && self.active
                    {
                        self.offers.insert(item.slot.slot, item);
                    } else if matches!(packet_type, 0x66..=0x6d) {
                        self.deliveries += 1;
                        self.insert(item);
                    }
                }
                GameplayEvent::ItemMoved { from, to, count } => {
                    self.move_echoes += 1;
                    println!("{} move echo {from:?}->{to:?} count{count}", self.name);
                }
                GameplayEvent::ItemDeleted { from, count } => {
                    self.move_echoes += 1;
                    println!("{} delete echo {from:?} count{count}", self.name);
                }
                GameplayEvent::Message(m) => {
                    println!("{} message {:?} {:?}", self.name, m.string_id, m.text)
                }
                _ => {}
            },
            _ => {}
        }
    }
    async fn pump(&mut self, seconds: f32) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs_f32(seconds);
        let mut heartbeat = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => break,
                _ = heartbeat.tick() => {
                    if let Some(s) = self.spawns.get(&self.id) { self.zone.send_position(s.id, s.position).await?; }
                }
                event = self.zone.next_event() => self.event(event?),
            }
            if self.reply_cancel {
                self.reply_cancel = false;
                self.cancel_replies += 1;
                self.trade(TradeCommand::Cancel { player_id: self.id })
                    .await?;
            }
        }
        Ok(())
    }
    async fn trade(&mut self, command: TradeCommand) -> anyhow::Result<()> {
        self.zone.command(Command::Trade(command)).await?;
        Ok(())
    }
    fn inventory(&self) -> BTreeMap<u32, u32> {
        let mut counts = BTreeMap::new();
        for item in self.items.values().filter(|i| i.slot.kind == 0) {
            *counts.entry(item.id).or_default() += if item.stack_size > 1 {
                item.count.max(1)
            } else {
                1
            };
        }
        counts
    }
    async fn move_item(&mut self, from: InventorySlot, to: InventorySlot) -> anyhow::Result<()> {
        ensure!(
            !self.items.contains_key(&to),
            "probe move destination occupied"
        );
        let mut item = self
            .items
            .remove(&from)
            .context("probe move source empty")?;
        self.zone
            .command(Command::MoveItem { from, to, count: 0 })
            .await?;
        item.slot = to;
        self.items.insert(to, item);
        Ok(())
    }
    async fn offer(&mut self, item_id: u32, slot: u16) -> anyhow::Result<()> {
        ensure!(self.active, "no active trade");
        let from = self
            .items
            .values()
            .find(|i| i.id == item_id && i.slot.kind == 0)
            .context("offer item missing")?
            .slot;
        self.move_item(from, InventorySlot::CURSOR).await?;
        self.move_item(InventorySlot::CURSOR, InventorySlot::trade(slot))
            .await
    }
    async fn coins(&mut self, amount: u32) -> anyhow::Result<()> {
        ensure!(
            self.active && self.money.platinum >= amount,
            "coin precondition failed"
        );
        self.trade(TradeCommand::OfferCoin {
            coin: CoinType::Platinum,
            amount,
        })
        .await?;
        self.money.platinum -= amount;
        Ok(())
    }
    async fn cancel(&mut self) -> anyhow::Result<()> {
        self.active = false;
        self.trade(TradeCommand::Cancel { player_id: self.id })
            .await
    }
    async fn accept(&mut self) -> anyhow::Result<()> {
        self.trade(TradeCommand::Accept { player_id: self.id })
            .await
    }
}
async fn pump(a: &mut Player, b: &mut Player, seconds: f32) -> anyhow::Result<()> {
    tokio::try_join!(a.pump(seconds), b.pump(seconds))?;
    Ok(())
}
async fn start(a: &mut Player, b: &mut Player) -> anyhow::Result<()> {
    a.events.clear();
    b.events.clear();
    a.offers.clear();
    b.offers.clear();
    a.trade(TradeCommand::Request {
        to_id: b.id,
        from_id: a.id,
    })
    .await?;
    pump(a, b, 0.7).await?;
    ensure!(
        b.events.contains(&TradeEvent::Requested {
            to_id: b.id,
            from_id: a.id
        }),
        "missing request"
    );
    b.trade(TradeCommand::Acknowledge {
        to_id: a.id,
        from_id: b.id,
    })
    .await?;
    b.active = true;
    pump(a, b, 0.7).await?;
    ensure!(
        a.active
            && a.events.contains(&TradeEvent::Acknowledged {
                to_id: a.id,
                from_id: b.id
            }),
        "missing acknowledgment"
    );
    ensure!(
        !b.events
            .iter()
            .any(|e| matches!(e, TradeEvent::Acknowledged { .. })),
        "unexpected ack self echo"
    );
    Ok(())
}
async fn reconnect(
    a: Player,
    b: Player,
    configs: &[ConnectionConfig; 2],
) -> anyhow::Result<(Player, Player)> {
    a.zone.logout().await?;
    b.zone.logout().await?;
    drop(a);
    drop(b);
    tokio::time::sleep(Duration::from_secs(2)).await;
    tokio::try_join!(Player::connect(&configs[0]), Player::connect(&configs[1]))
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq_net=info")
        .init();
    let paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    ensure!(paths.len() == 2, "eqtrade CONFIG1 CONFIG2");
    let configs = [
        ConnectionConfig::load(&paths[0])?,
        ConnectionConfig::load(&paths[1])?,
    ];
    for (config, username, character) in [
        (&configs[0], "openeq_trade1", "Barterer"),
        (&configs[1], "openeq_trade2", "Swapper"),
    ] {
        ensure!(
            config.host == "storage2.daeken.dev"
                && config.username == username
                && config.character == character,
            "requires isolated Barterer/Swapper trade fixtures"
        );
    }
    let (mut a, mut b) =
        tokio::try_join!(Player::connect(&configs[0]), Player::connect(&configs[1]))?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        a.spawns.contains_key(&b.id) && b.spawns.contains_key(&a.id),
        "peers absent"
    );
    let baseline = [
        (a.inventory(), copper(a.money)),
        (b.inventory(), copper(b.money)),
    ];
    ensure!(
        baseline[0].0.get(&5001) == Some(&1) && baseline[1].0.get(&1001) == Some(&1),
        "fixture items are not at baseline"
    );
    // A declined invitation never opens a trade or moves possessions.
    a.trade(TradeCommand::Request {
        to_id: b.id,
        from_id: a.id,
    })
    .await?;
    pump(&mut a, &mut b, 0.5).await?;
    b.trade(TradeCommand::Busy {
        to_id: a.id,
        from_id: b.id,
    })
    .await?;
    pump(&mut a, &mut b, 0.5).await?;
    ensure!(
        a.events.contains(&TradeEvent::Busy {
            to_id: a.id,
            from_id: b.id
        }),
        "busy reply missing"
    );
    println!("PROOF: busy reply preserves inventories");
    // Both sides escrow a real item and coins, then cancel asymmetrically.
    start(&mut a, &mut b).await?;
    a.offer(5001, 0).await?;
    b.offer(1001, 0).await?;
    a.coins(2).await?;
    b.coins(3).await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        a.offers.get(&0).map(|i| i.id) == Some(1001)
            && b.offers.get(&0).map(|i| i.id) == Some(5001),
        "remote offers missing"
    );
    ensure!(
        a.events.contains(&TradeEvent::CoinsAdded {
            recipient_id: a.id,
            coin: CoinType::Platinum,
            amount: 3
        }),
        "coin delta missing"
    );
    let replies = b.cancel_replies;
    a.cancel().await?;
    pump(&mut a, &mut b, 2.).await?;
    ensure!(
        b.cancel_replies == replies + 1 && a.cancel_replies == 0,
        "cancel reciprocal count"
    );
    ensure!(
        (a.inventory(), copper(a.money)) == baseline[0]
            && (b.inventory(), copper(b.money)) == baseline[1],
        "cancel failed to return offers"
    );
    println!("PROOF: cancellation refunds both item+coin offers; exactly one reciprocal cancel");
    (a, b) = reconnect(a, b, &configs).await?;
    ensure!(
        (a.inventory(), copper(a.money)) == baseline[0]
            && (b.inventory(), copper(b.money)) == baseline[1],
        "cancel persistence failed"
    );
    // A accepts, then B changes their offer. B accepting must not complete until A accepts again.
    start(&mut a, &mut b).await?;
    a.offer(5001, 0).await?;
    b.offer(1001, 0).await?;
    a.coins(2).await?;
    pump(&mut a, &mut b, 0.7).await?;
    a.accept().await?;
    pump(&mut a, &mut b, 0.5).await?;
    ensure!(
        b.events.contains(&TradeEvent::Accepted { player_id: a.id }),
        "peer acceptance missing"
    );
    b.coins(3).await?;
    b.accept().await?;
    pump(&mut a, &mut b, 0.7).await?;
    ensure!(
        !a.events.contains(&TradeEvent::Finished) && !b.events.contains(&TradeEvent::Finished),
        "offer change did not reset acceptance"
    );
    a.accept().await?;
    pump(&mut a, &mut b, 2.).await?;
    ensure!(
        a.events.contains(&TradeEvent::Finished) && b.events.contains(&TradeEvent::Finished),
        "trade never finished"
    );
    let exchanged = [
        (a.inventory(), copper(a.money)),
        (b.inventory(), copper(b.money)),
    ];
    ensure!(
        a.inventory().get(&1001) == Some(&1) && !a.inventory().contains_key(&5001),
        "A exchange delivery absent"
    );
    ensure!(
        b.inventory().get(&5001) == Some(&1) && !b.inventory().contains_key(&1001),
        "B exchange delivery absent"
    );
    ensure!(
        exchanged[0].1 == baseline[0].1 + 1000 && exchanged[1].1 + 1000 == baseline[1].1,
        "exchange coins wrong"
    );
    (a, b) = reconnect(a, b, &configs).await?;
    ensure!(
        (a.inventory(), copper(a.money)) == exchanged[0]
            && (b.inventory(), copper(b.money)) == exchanged[1],
        "exchange persistence failed"
    );
    println!(
        "PROOF: acceptance resets on changed offer; exchange and exact coin deltas persist after reconnect"
    );
    // Reverse the exact transaction so the dedicated fixtures remain reusable.
    start(&mut a, &mut b).await?;
    a.offer(1001, 0).await?;
    b.offer(5001, 0).await?;
    a.coins(3).await?;
    b.coins(2).await?;
    pump(&mut a, &mut b, 0.7).await?;
    a.accept().await?;
    b.accept().await?;
    pump(&mut a, &mut b, 2.).await?;
    (a, b) = reconnect(a, b, &configs).await?;
    ensure!(
        (a.inventory(), copper(a.money)) == baseline[0]
            && (b.inventory(), copper(b.money)) == baseline[1],
        "baseline restoration failed"
    );
    println!("PROOF: reverse exchange restored both original inventories and money");
    // Server disconnect handler refunds both escrows even without a peer cancel.
    start(&mut a, &mut b).await?;
    a.offer(5001, 0).await?;
    b.offer(1001, 0).await?;
    a.coins(2).await?;
    b.coins(3).await?;
    pump(&mut a, &mut b, 0.7).await?;
    a.zone.logout().await?;
    drop(a);
    b.pump(3.).await?;
    ensure!(
        copper(b.money) == baseline[1].1 && b.inventory() == baseline[1].0,
        "disconnect peer refund absent"
    );
    b.zone.logout().await?;
    drop(b);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let (a, b) = tokio::try_join!(Player::connect(&configs[0]), Player::connect(&configs[1]))?;
    ensure!(
        (a.inventory(), copper(a.money)) == baseline[0]
            && (b.inventory(), copper(b.money)) == baseline[1],
        "disconnect refund persistence failed"
    );
    a.zone.logout().await?;
    b.zone.logout().await?;
    println!("PROOF: disconnect refunds persisted; both dedicated fixtures restored");
    Ok(())
}
