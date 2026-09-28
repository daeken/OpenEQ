//! Live gameplay probe. Uses a separate, explicitly provisioned development character.
//! --exercise mutates only that character's inventory and a transient Arena NPC.
use anyhow::{Context, ensure};
use openeq_net::{
    gameplay::{ChatChannel, Command, GameplayEvent},
    inventory::{InventoryItem, InventorySlot},
    session::ConnectionConfig,
    zone::{Spawn, ZoneClient, ZoneEvent},
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

#[derive(Default)]
struct Probe {
    spawns: BTreeMap<u32, Spawn>,
    inventory: Vec<InventoryItem>,
    own: Option<u32>,
    ready: bool,
    profile: bool,
    chats: usize,
    wear: usize,
    damage: usize,
    deaths: Vec<u32>,
    loot: Vec<InventoryItem>,
    loot_open: bool,
    loot_ack: bool,
    loot_complete: bool,
    animations: usize,
    character: String,
    follow: Option<u32>,
    zone_id: u16,
    transitions: usize,
    doors: Vec<openeq_net::gameplay::Door>,
    door_moves: usize,
}
impl Probe {
    fn event(&mut self, event: ZoneEvent) {
        match event {
            ZoneEvent::Environment(env) => {
                self.zone_id = env.zone_id;
                println!("environment: {}", env.short_name);
            }
            ZoneEvent::Ready => {
                self.ready = true;
                println!("zone ready");
            }
            ZoneEvent::Spawn(spawn) => {
                if spawn.name == self.character {
                    self.own = Some(spawn.id);
                }
                self.spawns.insert(spawn.id, spawn);
            }
            ZoneEvent::Movement { id, position } => {
                if let Some(s) = self.spawns.get_mut(&id) {
                    s.position = position;
                }
            }
            ZoneEvent::Despawn(id) => {
                self.spawns.remove(&id);
            }
            ZoneEvent::Gameplay(event) => match event {
                GameplayEvent::ZoneTransition {
                    zone_id,
                    instance_id,
                } => {
                    println!("zone transition {zone_id}/{instance_id}");
                    self.transitions += 1;
                    self.spawns.clear();
                    self.own = None;
                    self.ready = false;
                    self.doors.clear();
                }
                GameplayEvent::ZoneChangeRequested(destination) => {
                    println!(
                        "zone requested {} at {:?}",
                        destination.zone_id, destination.position
                    );
                    if destination.zone_id == self.zone_id
                        && let Some(own) = self.own.and_then(|id| self.spawns.get_mut(&id))
                    {
                        let [x, y, z] = destination.position;
                        own.position = openeq_net::zone::Position {
                            x,
                            y,
                            z,
                            heading: destination.heading,
                            ..Default::default()
                        };
                    }
                }
                GameplayEvent::ZoneChangeResult {
                    zone_id, success, ..
                } => println!("zone result {zone_id} success{success}"),
                GameplayEvent::Doors(doors) => {
                    println!("doors: {}", doors.len());
                    self.doors = doors;
                }
                GameplayEvent::DoorMoved { id, action } => {
                    println!("door{id} action{action}");
                    self.door_moves += 1;
                }

                GameplayEvent::Inventory(items) => {
                    println!("initial inventory: {} main entries", items.len());
                    for i in &items {
                        print_item(i);
                    }
                    self.inventory = items;
                }
                GameplayEvent::Profile(profile) => {
                    println!(
                        "profile: {} level{} hp{} mana{} endurance{} skills{}",
                        profile.name,
                        profile.level,
                        profile.hp,
                        profile.mana,
                        profile.endurance,
                        profile.skills.len()
                    );
                    self.profile = true;
                }
                GameplayEvent::Chat(chat) => {
                    println!("chat {} {}: {}", chat.channel, chat.sender, chat.text);
                    if chat.text.contains("OpenEQ gameplay probe") {
                        self.chats += 1;
                    }
                }
                GameplayEvent::Message(message) => {
                    println!("message: {:?} {:?}", message.string_id, message.text);
                }
                GameplayEvent::WearChange(change) => {
                    println!(
                        "wear: id{} slot{} material{}",
                        change.id, change.slot, change.appearance.material
                    );
                    self.wear += 1;
                }
                GameplayEvent::Damage(damage) => {
                    println!(
                        "damage: {} -> {} amount{}",
                        damage.source_id, damage.target_id, damage.amount
                    );
                    self.damage += 1;
                }
                GameplayEvent::Death(death) => {
                    println!("death: id{} corpse{}", death.id, death.corpse_id);
                    self.deaths.push(if death.corpse_id != 0 {
                        death.corpse_id
                    } else {
                        death.id
                    });
                }
                GameplayEvent::Animation { .. } => self.animations += 1,
                GameplayEvent::LootOpened { response, currency } => {
                    println!("loot response{response} money{currency:?}");
                    self.loot_open = matches!(response, 1 | 6);
                }
                GameplayEvent::Item { packet_type, item } => {
                    println!("item type{packet_type:#x}");
                    print_item(&item);
                    if packet_type == 0x66 {
                        self.loot.push(item);
                    }
                }
                GameplayEvent::LootItemAcknowledged { slot, rejected, .. } => {
                    println!("loot item acknowledged slot{slot} rejected{rejected}");
                    self.loot_ack |= !rejected;
                }
                GameplayEvent::LootComplete => {
                    println!("loot complete");
                    self.loot_complete = true;
                }
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
                _=heartbeat.tick()=>{
                    if let Some(target)=self.follow.and_then(|id|self.spawns.get(&id)).map(|s|s.position)
                        && let Some(own)=self.own.and_then(|id|self.spawns.get_mut(&id)) {
                            own.position=openeq_net::zone::Position{x:target.x,y:target.y-2.,z:target.z,heading:0.,..Default::default()};
                    }
                    if let Some(spawn)=self.own.and_then(|id|self.spawns.get(&id)){zone.send_position(spawn.id,spawn.position).await?;}
                },
                event=zone.next_event()=>self.event(event?),
            }
        }
        Ok(())
    }
}
fn print_item(i: &InventoryItem) {
    println!(
        "  item{} {} slot{:?} count{} bag{}",
        i.id, i.name, i.slot, i.count, i.bag_slots
    );
    for child in &i.children {
        print_item(child);
    }
}
async fn say(zone: &ZoneClient, message: &str) -> anyhow::Result<()> {
    zone.command(Command::Chat {
        channel: ChatChannel::Say,
        target: String::new(),
        text: message.into(),
        language: 0,
    })
    .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openeq_net=info".into()),
        )
        .init();
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut exercise = false;
    let mut travel = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => path = args.next().map(PathBuf::from),
            "--exercise" => exercise = true,
            "--travel" => travel = true,
            _ => anyhow::bail!("eqgameplay --config FILE [--exercise] [--travel]"),
        }
    }
    let config = ConnectionConfig::load(&path.context("--config required")?)?;
    if exercise || travel {
        ensure!(
            config.character == "Mechanic"
                && config.username == "openeq_gameplay"
                && config.host == "storage2.daeken.dev",
            "mutating probes require the dedicated Storage2 Mechanic fixture"
        );
    }
    let mut zone = config.connect().await?;
    let mut probe = Probe {
        character: config.character.clone(),
        ..Probe::default()
    };
    probe.pump(&mut zone, 8.).await?;
    ensure!(
        probe.ready && probe.profile && !probe.inventory.is_empty(),
        "initial gameplay state incomplete"
    );
    if travel {
        say(&zone, "#zone poknowledge").await?;
        probe.pump(&mut zone, 14.).await?;
        ensure!(
            probe.ready && probe.zone_id == 202 && probe.transitions > 0,
            "travel to PoK failed"
        );
        ensure!(!probe.doors.is_empty(), "PoK doors missing");
        let door = probe
            .doors
            .iter()
            .find(|d| d.open_type == 5)
            .context("no ordinary hinged door")?
            .clone();
        let own = probe.own.context("own spawn after zoning")?;
        let p = openeq_net::zone::Position {
            x: door.position[0],
            y: door.position[1] - 5.,
            z: door.position[2] + 3.,
            ..Default::default()
        };
        probe.spawns.get_mut(&own).unwrap().position = p;
        zone.send_position(own, p).await?;
        zone.command(Command::ClickDoor {
            door_id: door.id,
            player_id: own,
        })
        .await?;
        probe.pump(&mut zone, 1.).await?;
        zone.command(Command::ClickDoor {
            door_id: door.id,
            player_id: own,
        })
        .await?;
        probe.pump(&mut zone, 1.).await?;
        ensure!(probe.door_moves >= 2, "door toggle roundtrip missing");
        say(&zone, "#zone arena").await?;
        probe.pump(&mut zone, 14.).await?;
        ensure!(
            probe.ready && probe.zone_id == 77 && probe.transitions >= 2,
            "travel back to Arena failed"
        );
        println!(
            "RESULT travel_roundtrip=verified door_toggles={}",
            probe.door_moves
        );
    }
    if !exercise {
        zone.logout().await?;
        println!("RESULT initial_state=verified");
        return Ok(());
    }
    zone.command(Command::Chat {
        channel: ChatChannel::Tell,
        target: config.character.clone(),
        text: "OpenEQ gameplay probe self tell".into(),
        language: 0,
    })
    .await?;
    say(&zone, "OpenEQ gameplay probe say").await?;
    probe.pump(&mut zone, 2.).await?;
    ensure!(probe.chats > 0, "chat not echoed by server");
    // A previous successful run may have autoequipped its generated loot cap.
    // Destroy only that exact duplicate fixture item before exercising equip.
    if probe
        .inventory
        .iter()
        .any(|i| i.slot == InventorySlot::possessions(2) && i.id == 1001)
        && probe
            .inventory
            .iter()
            .any(|i| i.slot == InventorySlot::possessions(24) && i.id == 1001)
    {
        zone.command(Command::DeleteItem {
            slot: InventorySlot::possessions(2),
            count: 0,
        })
        .await?;
        probe.pump(&mut zone, 0.5).await?;
    }
    let bag = InventorySlot::possessions(23);
    zone.command(Command::MoveItem {
        from: InventorySlot::possessions(24),
        to: InventorySlot::possessions(2),
        count: 0,
    })
    .await?;
    zone.command(Command::MoveItem {
        from: bag.in_bag(5),
        to: bag.in_bag(6),
        count: 0,
    })
    .await?;
    probe.pump(&mut zone, 1.).await?;
    zone.logout().await?;
    probe.pump(&mut zone, 0.5).await.ok();
    drop(zone);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut zone = config.connect().await?;
    probe.inventory.clear();
    probe.ready = false;
    probe.spawns.clear();
    probe.own = None;
    probe.pump(&mut zone, 8.).await?;
    ensure!(
        probe
            .inventory
            .iter()
            .any(|i| i.slot == InventorySlot::possessions(2) && i.id == 1001),
        "equipped cap did not survive reconnect"
    );
    ensure!(
        probe
            .inventory
            .iter()
            .flat_map(|i| &i.children)
            .any(|i| i.slot == bag.in_bag(6) && i.id == 13005),
        "bag move did not survive reconnect"
    );
    println!("inventory equip and bag move persisted across reconnect");
    zone.command(Command::MoveItem {
        from: InventorySlot::possessions(2),
        to: InventorySlot::possessions(24),
        count: 0,
    })
    .await?;
    zone.command(Command::MoveItem {
        from: bag.in_bag(6),
        to: bag.in_bag(5),
        count: 0,
    })
    .await?;
    probe.pump(&mut zone, 1.).await?;
    // Explicitly spawned ephemeral rat, never a natural spawn or another player.
    say(&zone, "#npctypespawn 351043").await?;
    probe.pump(&mut zone, 1.).await?;
    let target = probe
        .spawns
        .values()
        .filter(|s| s.name.starts_with("a_rat") && !s.is_corpse)
        .max_by_key(|s| s.id)
        .context("fixture rat did not spawn")?
        .id;
    zone.target(target).await?;
    say(&zone, "#npcloot add 1001 1 0").await?;
    let own = probe.own.context("missing own spawn")?;
    zone.command(Command::Consider {
        player_id: own,
        target_id: target,
    })
    .await?;
    // Step a few units south of the rat and face north so the server can attack.
    let rat = probe.spawns[&target].position;
    let position = openeq_net::zone::Position {
        y: rat.y - 5.,
        heading: 0.,
        ..rat
    };
    probe.spawns.get_mut(&own).unwrap().position = position;
    zone.send_position(own, position).await?;
    probe.pump(&mut zone, 1.).await?;
    probe.follow = Some(target);
    zone.command(Command::AutoAttack(true)).await?;
    for _ in 0..30 {
        probe.pump(&mut zone, 1.).await?;
        if !probe.deaths.is_empty() {
            break;
        }
    }
    zone.command(Command::AutoAttack(false)).await?;
    probe.follow = None;
    ensure!(
        probe.damage > 0 && !probe.deaths.is_empty(),
        "server combat/death not observed"
    );
    let corpse = *probe.deaths.last().unwrap();
    zone.command(Command::LootRequest(corpse)).await?;
    probe.pump(&mut zone, 2.).await?;
    ensure!(
        probe.loot_open && !probe.loot.is_empty(),
        "corpse loot not received"
    );
    let slot = probe
        .loot
        .iter()
        .find(|i| i.id == 1001)
        .context("seeded loot not present")?
        .slot
        .slot;
    zone.command(Command::LootItem {
        corpse_id: corpse,
        player_id: own,
        slot,
        auto_loot: true,
    })
    .await?;
    probe.pump(&mut zone, 2.).await?;
    zone.command(Command::EndLoot(corpse)).await?;
    probe.pump(&mut zone, 1.).await?;
    zone.logout().await?;
    ensure!(probe.loot_ack, "loot acknowledgement missing");
    println!(
        "RESULT initial_state=verified chat_echoes={} damage_packets={} deaths={} animations={} loot_ack={} loot_complete={} inventory_equip_bag_persistence=verified",
        probe.chats,
        probe.damage,
        probe.deaths.len(),
        probe.animations,
        probe.loot_ack,
        probe.loot_complete
    );
    Ok(())
}
