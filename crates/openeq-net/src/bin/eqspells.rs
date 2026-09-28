//! Live spell probe, restricted to the separately provisioned Arcanist fixture.
use anyhow::{Context, ensure};
use openeq_net::{
    gameplay::{Buff, ChatChannel, Command, GameplayEvent, PlayerProfile},
    session::ConnectionConfig,
    zone::{Spawn, ZoneClient, ZoneEvent},
};
use std::{collections::BTreeMap, path::PathBuf, time::Duration};

#[derive(Default)]
struct Probe {
    spawns: BTreeMap<u32, Spawn>,
    own: Option<u32>,
    ready: bool,
    profile: Option<PlayerProfile>,
    confirmations: Vec<(u32, u32, u32)>,
    begun: Vec<u32>,
    interrupted: usize,
    enabled: usize,
    buffs: BTreeMap<u32, Buff>,
    shield_seen: bool,
    removed: usize,
    damage: usize,
    deaths: Vec<(u32, u32)>,
}
impl Probe {
    fn event(&mut self, event: ZoneEvent) {
        match event {
            ZoneEvent::Ready => self.ready = true,
            ZoneEvent::Spawn(s) => {
                if s.name == "Arcanist" {
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
                GameplayEvent::Profile(p) => {
                    println!(
                        "profile: {} mana{} spells{:?}",
                        p.name,
                        p.mana,
                        &p.memorized_spells[..12.min(p.memorized_spells.len())]
                    );
                    self.buffs = p.buffs.iter().map(|b| (b.slot, b.clone())).collect();
                    self.profile = Some(p);
                }
                GameplayEvent::SpellMemorized {
                    slot,
                    spell_id,
                    action,
                    reduction,
                } => {
                    println!("gem: slot{slot} spell{spell_id} action{action} reduction{reduction}");
                    self.confirmations.push((slot, spell_id, action));
                }
                GameplayEvent::BeginCast {
                    caster_id,
                    spell_id,
                    cast_time_ms,
                } => {
                    println!("begin: caster{caster_id} spell{spell_id} duration{cast_time_ms}");
                    if Some(caster_id) == self.own {
                        self.begun.push(spell_id);
                    }
                }
                GameplayEvent::CastInterrupted {
                    id,
                    string_id,
                    message,
                } => {
                    println!("interrupt: id{id} string{string_id} {message}");
                    if Some(id) == self.own {
                        self.interrupted += 1;
                    }
                }
                GameplayEvent::SpellBarEnabled {
                    spell_id,
                    slot,
                    mana,
                    ..
                } => {
                    println!("enabled: spell{spell_id} slot{slot} mana{mana}");
                    self.enabled += 1;
                }
                GameplayEvent::SpellAction {
                    source_id,
                    target_id,
                    spell_id,
                    ..
                } => println!("action: {source_id}->{target_id} spell{spell_id}"),
                GameplayEvent::Buffs { id, all, buffs, .. } => {
                    println!("buffs: id{id} all{all} {buffs:?}");
                    if Some(id) == self.own {
                        if all {
                            self.buffs.clear();
                        }
                        for b in buffs {
                            if b.spell_id == 288 {
                                self.shield_seen = true;
                            }
                            if b.spell_id == u32::MAX {
                                self.buffs.remove(&b.slot);
                            } else {
                                self.buffs.insert(b.slot, b);
                            }
                        }
                    }
                }
                GameplayEvent::BuffChanged { id, buff, removed } => {
                    println!("buff: id{id} removed{removed} {buff:?}");
                    if Some(id) == self.own {
                        if removed {
                            self.removed += 1;
                            self.buffs.remove(&buff.slot);
                        } else {
                            if buff.spell_id == 288 {
                                self.shield_seen = true;
                            }
                            self.buffs.insert(buff.slot, buff);
                        }
                    }
                }
                GameplayEvent::Damage(d) => {
                    println!(
                        "damage: spell{} {}->{} amount{}",
                        d.spell_id, d.source_id, d.target_id, d.amount
                    );
                    if d.spell_id == 54 && d.amount > 0 && Some(d.source_id) == self.own {
                        self.damage += 1;
                    }
                }
                GameplayEvent::Death(d) => {
                    println!(
                        "death: id{} spell{} killer{}",
                        d.id, d.spell_id, d.killer_id
                    );
                    self.deaths.push((d.id, d.spell_id));
                }
                GameplayEvent::Message(m) => {
                    println!("message: {:?} {:?} {:?}", m.string_id, m.text, m.arguments)
                }
                GameplayEvent::Chat(c) => println!("chat: {}", c.text),
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
                event=zone.next_event()=>self.event(event?),
            }
        }
        Ok(())
    }
}
async fn say(zone: &ZoneClient, text: &str) -> anyhow::Result<()> {
    zone.command(Command::Chat {
        channel: ChatChannel::Say,
        target: String::new(),
        text: text.into(),
        language: 0,
    })
    .await?;
    Ok(())
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq_net=info")
        .init();
    let mut args = std::env::args().skip(1);
    ensure!(
        args.next().as_deref() == Some("--config"),
        "eqspells --config FILE"
    );
    let config = ConnectionConfig::load(&PathBuf::from(args.next().context("--config required")?))?;
    ensure!(
        config.character == "Arcanist"
            && config.username == "openeq_spells"
            && config.host == "storage2.daeken.dev",
        "spell probe requires dedicated Storage2 Arcanist fixture"
    );
    let mut zone = config.connect().await?;
    let mut p = Probe::default();
    p.pump(&mut zone, 5.).await?;
    ensure!(
        p.ready && p.profile.is_some(),
        "initial spell profile missing"
    );
    let own = p.own.context("own spawn")?;
    zone.command(Command::Posture {
        player_id: own,
        posture: 1,
    })
    .await?;
    for (slot, spell_id) in [(0, 54), (1, 288), (2, 36)] {
        zone.command(Command::MemorizeSpell { slot, spell_id })
            .await?;
    }
    p.pump(&mut zone, 2.).await?;
    for (slot, spell_id) in [(0, 54), (1, 288), (2, 36)] {
        ensure!(
            p.confirmations.contains(&(slot, spell_id, 1)),
            "missing memorize confirmation for {spell_id}"
        );
    }
    zone.command(Command::Posture {
        player_id: own,
        posture: 0,
    })
    .await?;
    // A repeated probe clears only the fixture's old shield before casting it again.
    for slot in p
        .buffs
        .values()
        .filter(|b| b.spell_id == 288)
        .map(|b| b.slot)
        .collect::<Vec<_>>()
    {
        zone.command(Command::RemoveBuff {
            slot,
            player_id: own,
        })
        .await?;
    }
    p.pump(&mut zone, 1.).await?;
    p.shield_seen = false;
    for _ in 0..4 {
        zone.command(Command::CastSpell {
            slot: 1,
            spell_id: 288,
            target_id: own,
        })
        .await?;
        p.pump(&mut zone, 5.).await?;
        if p.shield_seen {
            break;
        }
    }
    ensure!(p.shield_seen, "successful shield buff not received");
    ensure!(
        p.confirmations.contains(&(1, 288, 3)),
        "successful cast cooldown not received"
    );
    let shield_slot = p
        .buffs
        .values()
        .find(|b| b.spell_id == 288)
        .context("shield slot")?
        .slot;
    let previous_removed = p.removed;
    zone.command(Command::RemoveBuff {
        slot: shield_slot,
        player_id: own,
    })
    .await?;
    p.pump(&mut zone, 2.).await?;
    ensure!(
        p.removed > previous_removed,
        "buff removal acknowledgment missing"
    );
    // Interrupt Gate after the server has started it, before it can zone us.
    let interrupted = p.interrupted;
    let begun = p.begun.len();
    zone.command(Command::CastSpell {
        slot: 2,
        spell_id: 36,
        target_id: own,
    })
    .await?;
    p.pump(&mut zone, 0.4).await?;
    ensure!(
        p.begun.len() > begun && p.begun.last() == Some(&36),
        "Gate did not begin"
    );
    zone.command(Command::InterruptSpell).await?;
    p.pump(&mut zone, 2.).await?;
    ensure!(
        p.interrupted > interrupted,
        "server cast interruption missing"
    );
    let before: Vec<_> = p.spawns.keys().copied().collect();
    say(&zone, "#npctypespawn 351043").await?;
    p.pump(&mut zone, 2.).await?;
    let target = p
        .spawns
        .values()
        .find(|s| !before.contains(&s.id) && s.npc && !s.is_corpse)
        .context("new test rat")?
        .id;
    zone.target(target).await?;
    zone.command(Command::CastSpell {
        slot: 0,
        spell_id: 54,
        target_id: target,
    })
    .await?;
    p.pump(&mut zone, 5.).await?;
    ensure!(
        p.damage > 0 || p.deaths.contains(&(target, 54)),
        "offensive spell damage/death missing"
    );
    zone.command(Command::UnmemorizeSpell {
        slot: 2,
        spell_id: 36,
    })
    .await?;
    p.pump(&mut zone, 1.).await?;
    ensure!(
        p.confirmations
            .iter()
            .any(|(slot, _, action)| *slot == 2 && *action == 2),
        "forget confirmation missing"
    );
    zone.logout().await?;
    drop(zone);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut zone = config.connect().await?;
    let mut reconnected = Probe::default();
    reconnected.pump(&mut zone, 5.).await?;
    let profile = reconnected.profile.context("reconnected profile")?;
    ensure!(
        profile.memorized_spells.get(0..2) == Some(&[54, 288]),
        "memorized spells did not persist"
    );
    ensure!(
        profile
            .memorized_spells
            .get(2)
            .is_some_and(|id| matches!(id, 0 | 0xffff | 0xffffffff)),
        "forgotten spell persisted"
    );
    zone.logout().await?;
    println!(
        "RESULT spells_memorize_persistence=verified shield_buff=verified buff_remove=verified interrupted={} offensive_damage={} spell_deaths={} cooldown=verified",
        p.interrupted,
        p.damage,
        p.deaths.len()
    );
    Ok(())
}
