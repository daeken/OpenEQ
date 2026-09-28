//! Live merchant/bank proof restricted to the separate Broker fixture.
use anyhow::{Context, ensure};
use openeq_net::{
    gameplay::{CoinLocation, CoinType, Command, Currency, GameplayEvent, PlayerProfile},
    inventory::{InventoryItem, InventorySlot},
    session::ConnectionConfig,
    zone::{Position, Spawn, ZoneClient, ZoneEvent},
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};
#[derive(Default)]
struct Probe {
    spawns: BTreeMap<u32, Spawn>,
    own: Option<u32>,
    ready: bool,
    items: BTreeMap<InventorySlot, InventoryItem>,
    offers: BTreeMap<u32, InventoryItem>,
    profile: Option<PlayerProfile>,
    money: Currency,
    bank: Currency,
    merchant: Option<u32>,
    closed: bool,
    bought: Option<(u32, u32, u32)>,
    sold: Option<(InventorySlot, u32, u32)>,
    item_deliveries: usize,
    deletions: usize,
    currency_updates: usize,
}
fn copper(c: Currency) -> u64 {
    c.platinum as u64 * 1000 + c.gold as u64 * 100 + c.silver as u64 * 10 + c.copper as u64
}
impl Probe {
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
                if s.name == "Broker" {
                    self.own = Some(s.id);
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
                    for i in items {
                        self.insert(i);
                    }
                    println!("inventory: {} flattened items", self.items.len());
                }
                GameplayEvent::Profile(p) => {
                    println!(
                        "profile: {} carried{:?} bank{:?} shared{}",
                        p.name, p.currency, p.bank_currency, p.shared_platinum
                    );
                    self.money = p.currency;
                    self.bank = p.bank_currency;
                    self.profile = Some(p);
                }
                GameplayEvent::Item { packet_type, item } => {
                    if packet_type == 0x64 {
                        println!(
                            "offer: slot{} item{} {} quote{} stock{}",
                            item.slot.slot, item.id, item.name, item.price, item.merchant_count
                        );
                        self.offers.insert(item.slot.slot as u32, item);
                    } else {
                        println!(
                            "delivery: type{packet_type:#x} slot{:?} item{} count{}",
                            item.slot, item.id, item.count
                        );
                        self.item_deliveries += 1;
                        self.insert(item);
                    }
                }
                GameplayEvent::MerchantOpened {
                    merchant_id,
                    command,
                    rate,
                    tabs,
                } => {
                    println!("merchant: id{merchant_id} command{command} rate{rate} tabs{tabs}");
                    if command == 1 {
                        self.merchant = Some(merchant_id);
                    } else {
                        self.merchant = None;
                    }
                }
                GameplayEvent::MerchantBought {
                    slot,
                    quantity,
                    price,
                    ..
                } => {
                    println!("purchase confirmed: slot{slot} count{quantity} total{price}");
                    self.bought = Some((slot, quantity, price));
                }
                GameplayEvent::MerchantSold {
                    slot,
                    quantity,
                    price,
                    rejected,
                    ..
                } => {
                    println!(
                        "sale confirmed: slot{slot:?} count{quantity} total{price} rejected{rejected}"
                    );
                    if !rejected {
                        self.sold = Some((slot, quantity, price));
                    }
                }
                GameplayEvent::MerchantItemRemoved { slot, .. } => {
                    println!("merchant removed slot{slot}");
                    self.offers.remove(&slot);
                }
                GameplayEvent::MerchantClosed => {
                    println!("merchant closed");
                    self.closed = true;
                    self.merchant = None;
                }
                GameplayEvent::Currency(c) => {
                    println!("money update: {c:?}");
                    self.money = c;
                    self.currency_updates += 1;
                }
                GameplayEvent::BankerBalances { carried, bank } => {
                    println!("banker balances: carried{carried:?} bank{bank:?}");
                    self.money = carried;
                    self.bank = bank;
                }
                GameplayEvent::ItemDeleted { from, count } => {
                    println!("delete units: {from:?} count{count}");
                    self.deletions += 1;
                }
                GameplayEvent::ItemMoved { from, to, count } => {
                    println!("server move: {from:?}->{to:?} count{count}");
                    self.deletions += 1;
                }
                GameplayEvent::Message(m) => println!("message: {:?} {:?}", m.string_id, m.text),
                _ => {}
            },
            _ => {}
        }
    }
    async fn pump(&mut self, zone: &mut ZoneClient, seconds: f32) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs_f32(seconds);
        let mut heartbeat = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _=tokio::time::sleep_until(deadline)=>break,
                _=heartbeat.tick()=>{if let Some(s)=self.own.and_then(|id|self.spawns.get(&id)){zone.send_position(s.id,s.position).await?;}},
                e=zone.next_event()=>self.event(e?),
            }
        }
        Ok(())
    }
    async fn approach(&mut self, zone: &mut ZoneClient, id: u32) -> anyhow::Result<()> {
        let target = self.spawns.get(&id).context("service NPC")?.position;
        let p = Position {
            x: target.x,
            y: target.y - 10.,
            // The guarded fixture is human, while Dogle is smaller. Leave
            // clearance above the NPC center so a later walking client does
            // not start its feet below the floor. The integrated client probe
            // additionally verifies its saved height against the actual assets.
            z: target.z + 3.125,
            heading: 0.,
            ..Default::default()
        };
        let own = self.own.context("own spawn")?;
        self.spawns.get_mut(&own).unwrap().position = p;
        zone.send_position(own, p).await?;
        self.pump(zone, 1.).await?;
        let actual = self.spawns[&own].position;
        ensure!(
            (actual.x - target.x).powi(2)
                + (actual.y - target.y).powi(2)
                + (actual.z - target.z).powi(2)
                < 40000.,
            "service NPC out of range"
        );
        Ok(())
    }
}
async fn connect(config: &ConnectionConfig) -> anyhow::Result<(ZoneClient, Probe)> {
    let mut zone = config.connect().await?;
    let mut p = Probe::default();
    p.pump(&mut zone, 4.).await?;
    ensure!(
        p.ready && p.profile.is_some() && p.own.is_some(),
        "initial commerce state incomplete"
    );
    Ok((zone, p))
}
async fn reconnect(
    zone: ZoneClient,
    config: &ConnectionConfig,
) -> anyhow::Result<(ZoneClient, Probe)> {
    zone.logout().await?;
    drop(zone);
    tokio::time::sleep(Duration::from_secs(2)).await;
    connect(config).await
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq_net=info")
        .init();
    let mut args = std::env::args().skip(1);
    ensure!(
        args.next().as_deref() == Some("--config"),
        "eqcommerce --config FILE"
    );
    let config = ConnectionConfig::load(&PathBuf::from(args.next().context("config path")?))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_commerce"
            && config.character == "Broker",
        "requires dedicated Broker fixture"
    );
    let (mut zone, mut p) = connect(&config).await?;
    let merchant = p
        .spawns
        .values()
        .find(|s| s.class == 41 && s.name.starts_with("Amile_Pitt"))
        .context("Amile Pitt merchant")?
        .id;
    p.approach(&mut zone, merchant).await?;
    let start_money = copper(p.money);
    let own = p.own.unwrap();
    zone.command(Command::MerchantOpen {
        merchant_id: merchant,
        player_id: own,
    })
    .await?;
    p.pump(&mut zone, 3.).await?;
    ensure!(p.merchant == Some(merchant), "merchant opening rejected");
    let offer = p
        .offers
        .values()
        .find(|i| i.id == 13005)
        .context("ration listing")?
        .clone();
    ensure!(
        offer.price > 0 && offer.merchant_count == -1,
        "unlimited stock and real price absent"
    );
    ensure!(
        !p.items.values().any(|i| i.id == 13005),
        "fixture already contains rations; resolve previous run before mutating"
    );
    let currencies_before = p.currency_updates;
    let deliveries_before = p.item_deliveries;
    zone.command(Command::MerchantBuy {
        merchant_id: merchant,
        player_id: own,
        slot: offer.slot.slot as u32,
        quantity: 2,
        price: offer.price * 2,
    })
    .await?;
    p.pump(&mut zone, 3.).await?;
    let (bought_slot, bought_count, bought_price) = p.bought.context("purchase acknowledgment")?;
    ensure!(
        bought_slot == offer.slot.slot as u32
            && bought_count == 2
            && bought_price == offer.price * 2,
        "purchase quote/quantity mismatch"
    );
    ensure!(
        p.item_deliveries > deliveries_before,
        "purchase delivered no item"
    );
    let purchased = p
        .items
        .values()
        .find(|i| i.id == 13005 && i.count == 2)
        .context("actual delivered ration stack")?
        .clone();
    println!(
        "BUY wire proof: deliveries{} currency_updates{}",
        p.item_deliveries - deliveries_before,
        p.currency_updates - currencies_before
    );
    let delete_before = p.deletions;
    zone.command(Command::MerchantSell {
        merchant_id: merchant,
        slot: purchased.slot,
        quantity: 2,
    })
    .await?;
    p.pump(&mut zone, 3.).await?;
    let (sold_slot, sold_count, sold_price) = p.sold.context("sale acknowledgment")?;
    ensure!(
        sold_slot == purchased.slot && sold_count == 2 && sold_price > 0,
        "sale acknowledgment incorrect"
    );
    ensure!(
        copper(p.money) == start_money - bought_price as u64 + sold_price as u64,
        "authoritative sale money mismatch"
    );
    println!(
        "SELL wire proof: deletion_packets{} currency_updates{}",
        p.deletions - delete_before,
        p.currency_updates - currencies_before
    );
    let after_commerce = copper(p.money);
    zone.command(Command::MerchantClose).await?;
    p.pump(&mut zone, 1.).await?;
    ensure!(p.closed, "merchant close acknowledgment missing");

    let banker = p
        .spawns
        .values()
        .find(|s| s.class == 40 && s.name.starts_with("Dogle_Pitt"))
        .context("banker")?
        .id;
    p.approach(&mut zone, banker).await?;
    let cap = InventorySlot::possessions(24);
    let water = InventorySlot::possessions(23);
    ensure!(
        p.items.get(&cap).is_some_and(|i| i.id == 1001)
            && p.items
                .get(&water)
                .is_some_and(|i| i.id == 13006 && !i.no_drop && !i.attuned),
        "fixture banking items absent/unsafe"
    );
    ensure!(
        !p.items.contains_key(&InventorySlot::bank(0))
            && !p.items.contains_key(&InventorySlot::shared_bank(0)),
        "test bank slots occupied"
    );
    let water_count = p.items[&water].count;
    let bank_initial = copper(p.bank);
    let shared_initial = p.profile.as_ref().unwrap().shared_platinum;
    ensure!(
        p.money.platinum >= 2,
        "need two platinum for bank roundtrip"
    );
    zone.command(Command::MoveItem {
        from: cap,
        to: InventorySlot::bank(0),
        count: 0,
    })
    .await?;
    zone.command(Command::MoveItem {
        from: water,
        to: InventorySlot::shared_bank(0),
        count: 0,
    })
    .await?;
    for to in [CoinLocation::Bank, CoinLocation::SharedBank] {
        zone.command(Command::MoveCoin {
            from: CoinLocation::Carried,
            to,
            coin: CoinType::Platinum,
            amount: 1,
        })
        .await?;
    }
    zone.command(Command::BankerChange).await?;
    p.pump(&mut zone, 2.).await?;
    ensure!(
        copper(p.bank) == bank_initial + 1000 && copper(p.money) == after_commerce - 2000,
        "banker balances did not confirm coin deposit"
    );
    (zone, p) = reconnect(zone, &config).await?;
    ensure!(
        !p.items.values().any(|i| i.id == 13005),
        "sold rations persisted unexpectedly"
    );
    ensure!(
        p.items
            .get(&InventorySlot::bank(0))
            .is_some_and(|i| i.id == 1001)
            && p.items
                .get(&InventorySlot::shared_bank(0))
                .is_some_and(|i| i.id == 13006 && i.count == water_count),
        "bank/shared deposit did not persist"
    );
    ensure!(
        copper(p.bank) == bank_initial + 1000
            && p.profile.as_ref().unwrap().shared_platinum == shared_initial + 1
            && copper(p.money) == after_commerce - 2000,
        "coin deposit did not persist"
    );
    let banker = p
        .spawns
        .values()
        .find(|s| s.class == 40 && s.name.starts_with("Dogle_Pitt"))
        .context("banker after reconnect")?
        .id;
    p.approach(&mut zone, banker).await?;
    zone.command(Command::MoveItem {
        from: InventorySlot::bank(0),
        to: cap,
        count: 0,
    })
    .await?;
    zone.command(Command::MoveItem {
        from: InventorySlot::shared_bank(0),
        to: water,
        count: 0,
    })
    .await?;
    for from in [CoinLocation::Bank, CoinLocation::SharedBank] {
        zone.command(Command::MoveCoin {
            from,
            to: CoinLocation::Carried,
            coin: CoinType::Platinum,
            amount: 1,
        })
        .await?;
    }
    zone.command(Command::BankerChange).await?;
    p.pump(&mut zone, 2.).await?;
    (zone, p) = reconnect(zone, &config).await?;
    ensure!(
        p.items.get(&cap).is_some_and(|i| i.id == 1001)
            && p.items
                .get(&water)
                .is_some_and(|i| i.id == 13006 && i.count == water_count),
        "bank item withdrawal did not persist"
    );
    ensure!(
        !p.items.contains_key(&InventorySlot::bank(0))
            && !p.items.contains_key(&InventorySlot::shared_bank(0)),
        "withdrawn items still in bank"
    );
    ensure!(
        copper(p.bank) == bank_initial
            && p.profile.as_ref().unwrap().shared_platinum == shared_initial
            && copper(p.money) == after_commerce,
        "currency withdrawal did not persist"
    );
    zone.logout().await?;
    println!(
        "RESULT merchant_open_close=verified purchase_delivery_quote=verified sale_currency=verified bank_shared_items_roundtrip=verified bank_shared_coin_roundtrip=verified purchase_total={bought_price} sale_total={sold_price}"
    );
    Ok(())
}
