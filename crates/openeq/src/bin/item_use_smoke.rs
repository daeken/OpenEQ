//! Scroll/item proof using only the dedicated Artificer fixture. The ordinary
//! client inventory/spell reducers consume all replies; credentials stay private.
use anyhow::{Context, ensure};
use openeq::game::{GameplayState, StringTable};
use openeq_net::{
    gameplay::{Command, GameplayEvent},
    inventory::InventorySlot,
    item_use::{ItemUseCommand, ItemUseEvent},
    session::ConnectionConfig,
    zone::{Position, ZoneClient, ZoneEvent},
};
use std::{path::Path, time::Duration};

struct Probe {
    game: GameplayState,
    own: Option<u32>,
    position: Option<Position>,
    ready: bool,
    events: Vec<GameplayEvent>,
}
impl Probe {
    fn new() -> Self {
        let mut game = GameplayState::default();
        if let Some(base) = openeq_assets::loader::default_client_dir() {
            game.strings = StringTable::load(&base);
        }
        Self {
            game,
            own: None,
            position: None,
            ready: false,
            events: Vec::new(),
        }
    }
    fn event(&mut self, event: ZoneEvent) {
        match event {
            ZoneEvent::Ready => self.ready = true,
            ZoneEvent::Spawn(spawn) if spawn.name == "Artificer" => {
                self.own = Some(spawn.id);
                self.position = Some(spawn.position);
            }
            ZoneEvent::Movement { id, position } if Some(id) == self.own => {
                self.position = Some(position);
            }
            ZoneEvent::Gameplay(event) => {
                match &event {
                    GameplayEvent::Inventory(items) => {
                        println!("inventory: {} root items", items.len())
                    }
                    GameplayEvent::Item { packet_type, item } => println!(
                        "item: kind{packet_type} slot{:?} id{} count{} charges{} recast{}",
                        item.slot, item.id, item.count, item.charges, item.recast_timestamp
                    ),
                    GameplayEvent::ItemUse(event) => println!("item-use: {event:?}"),
                    GameplayEvent::SpellMemorized {
                        slot,
                        spell_id,
                        action,
                        ..
                    } => println!("spell: slot{slot} spell{spell_id} action{action}"),
                    GameplayEvent::ItemMoved { from, to, count } => {
                        println!("move: {from:?}->{to:?} count{count}")
                    }
                    GameplayEvent::ItemDeleted { from, count } => {
                        println!("consume: {from:?} count{count}")
                    }
                    GameplayEvent::ItemChargeUsed { from, count } => {
                        println!("charge: {from:?} count{count}")
                    }
                    GameplayEvent::BeginCast {
                        caster_id,
                        spell_id,
                        cast_time_ms,
                    } if Some(*caster_id) == self.own => {
                        println!("begin: spell{spell_id} duration{cast_time_ms}")
                    }
                    GameplayEvent::CastInterrupted {
                        id,
                        string_id,
                        message,
                    } if Some(*id) == self.own || *id == 0 => {
                        println!("interrupt: string{string_id} {message}")
                    }
                    GameplayEvent::SpellAction {
                        source_id,
                        target_id,
                        spell_id,
                        ..
                    } if Some(*source_id) == self.own => {
                        println!("action: spell{spell_id} target{target_id}")
                    }
                    GameplayEvent::SpellBarEnabled { spell_id, slot, .. } => {
                        println!("enabled: spell{spell_id} slot{slot}")
                    }
                    GameplayEvent::Message(message) => {
                        println!("message: {}", self.game.strings.format(message))
                    }
                    _ => {}
                }
                self.events.push(event.clone());
                self.game
                    .apply(event, self.own, self.own, |_| "Artificer".into());
            }
            _ => {}
        }
    }
    async fn pump(&mut self, zone: &mut ZoneClient, seconds: f32) -> anyhow::Result<()> {
        let end = tokio::time::Instant::now() + Duration::from_secs_f32(seconds);
        let mut heartbeat = tokio::time::interval(Duration::from_millis(200));
        loop {
            tokio::select! {
                _=tokio::time::sleep_until(end)=>break,
                _=heartbeat.tick()=>if let (Some(id),Some(position))=(self.own,self.position) {zone.send_position(id,position).await?;},
                event=zone.next_event()=>self.event(event?),
            }
        }
        Ok(())
    }
    fn began(&self, after: usize, spell: u32) -> bool {
        self.events[after..].iter().any(|e| matches!(e, GameplayEvent::BeginCast { caster_id, spell_id, .. } if Some(*caster_id)==self.own && *spell_id==spell))
    }
    fn applied(&self, after: usize, spell: u32) -> bool {
        self.events[after..].iter().any(|e| matches!(e, GameplayEvent::SpellAction { source_id, spell_id, .. } if Some(*source_id)==self.own && *spell_id==spell))
    }
    async fn wait_begin(
        &mut self,
        zone: &mut ZoneClient,
        after: usize,
        spell: u32,
    ) -> anyhow::Result<()> {
        for _ in 0..20 {
            self.pump(zone, 0.1).await?;
            if self.began(after, spell) {
                return Ok(());
            }
        }
        anyhow::bail!("item spell{spell} did not begin");
    }
    async fn click(&self, zone: &ZoneClient, slot: InventorySlot) -> anyhow::Result<()> {
        zone.command(Command::ItemUse(ItemUseCommand::Click {
            slot,
            target_id: self.own.context("own spawn")?,
        }))
        .await?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq_net=info")
        .init();
    let argument = std::env::args().nth(1).context("item_use_smoke CONFIG")?;
    let config = ConnectionConfig::load(Path::new(&argument))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_itemuse"
            && config.character == "Artificer",
        "requires isolated Artificer fixture"
    );
    let mut zone = config.connect().await?;
    let mut probe = Probe::new();
    probe.pump(&mut zone, 5.).await?;
    ensure!(
        probe.ready && probe.game.inventory.received,
        "initial inventory missing"
    );
    let own = probe.own.context("own spawn")?;
    let scroll_slot = InventorySlot::possessions(23);
    let reusable_slot = InventorySlot::possessions(24);
    let cloudy_slot = InventorySlot::possessions(25).in_bag(0);
    let charged_slot = InventorySlot::possessions(25).in_bag(1);
    for (slot, id) in [
        (scroll_slot, 15288),
        (reusable_slot, 47742),
        (cloudy_slot, 14514),
        (charged_slot, 14534),
    ] {
        let item = probe
            .game
            .inventory
            .items
            .get(&slot)
            .context("fixture item missing; restore only the dedicated item-use fixture")?;
        ensure!(item.id == id, "unexpected fixture item in {slot:?}");
        println!(
            "fixture: {} id{} icon{} count{} charges{} click{:?} scroll{:?}",
            item.name,
            item.id,
            item.icon,
            item.count,
            item.charges,
            item.click,
            item.scroll_spell_id
        );
    }
    ensure!(
        probe.game.inventory.items[&scroll_slot].scroll_spell_id == Some(288),
        "scroll metadata"
    );
    ensure!(
        probe.game.inventory.items[&reusable_slot].click.spell_id == 5105
            && probe.game.inventory.items[&reusable_slot].charges == -1,
        "reusable click metadata"
    );
    ensure!(
        probe.game.inventory.items[&charged_slot].charges == 10,
        "charged fixture must start with ten charges"
    );
    ensure!(
        probe.game.inventory.items[&cloudy_slot].count == 2,
        "consumable fixture must start with two units"
    );
    ensure!(
        !probe
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::CURSOR),
        "cursor must be empty"
    );
    let book = &probe
        .game
        .profile
        .as_ref()
        .context("profile missing")?
        .spell_book;
    ensure!(
        !book.contains(&288),
        "fixture already knows Minor Shielding; restore fixture before repeating full proof"
    );
    let book_slot = book
        .iter()
        .position(|id| matches!(*id, 0 | 0xffff | 0xffffffff))
        .context("empty spellbook slot")? as u32;

    zone.command(Command::Posture {
        player_id: own,
        posture: 1,
    })
    .await?;
    zone.command(Command::MoveItem {
        from: scroll_slot,
        to: InventorySlot::CURSOR,
        count: 1,
    })
    .await?;
    probe
        .game
        .inventory
        .move_item(scroll_slot, InventorySlot::CURSOR, 1)
        .map_err(anyhow::Error::msg)?;
    let scribe_start = probe.events.len();
    zone.command(Command::ItemUse(ItemUseCommand::Scribe {
        book_slot,
        spell_id: 288,
    }))
    .await?;
    probe.pump(&mut zone, 2.).await?;
    let acknowledgements = &probe.events[scribe_start..];
    let scribed=acknowledgements.iter().position(|e|matches!(e,GameplayEvent::SpellMemorized{slot,spell_id:288,action:0,..} if *slot==book_slot)).context("scribe acknowledgment")?;
    let consumed=acknowledgements.iter().position(|e|matches!(e,GameplayEvent::ItemMoved{from,to,..} if *from==InventorySlot::CURSOR && *to==InventorySlot::DELETE)).context("authoritative cursor scroll consumption")?;
    ensure!(scribed < consumed, "unexpected scribe/consumption ordering");
    ensure!(
        !probe
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::CURSOR),
        "scroll remained on cursor"
    );
    ensure!(
        probe.game.profile.as_ref().unwrap().spell_book[book_slot as usize] == 288,
        "spellbook reducer did not apply acknowledgment"
    );
    println!("PROOF scribed slot{book_slot}; acknowledgment preceded scroll consumption");
    zone.command(Command::Posture {
        player_id: own,
        posture: 0,
    })
    .await?;
    probe.pump(&mut zone, 0.5).await?;

    let start = probe.events.len();
    probe.click(&zone, reusable_slot).await?;
    probe.pump(&mut zone, 4.).await?;
    ensure!(
        probe.began(start, 5105) && probe.applied(start, 5105),
        "reusable cast did not finish"
    );
    ensure!(
        probe.events[start..].iter().any(|e| matches!(
            e,
            GameplayEvent::ItemUse(ItemUseEvent::Recast {
                recast_type: 24,
                seconds: 180,
                ..
            })
        )),
        "shared item cooldown missing"
    );
    ensure!(
        probe.game.inventory.items[&reusable_slot].charges == -1,
        "reusable item lost a charge"
    );
    let cooldown_start = probe.events.len();
    probe.click(&zone, reusable_slot).await?;
    probe.pump(&mut zone, 2.).await?;
    ensure!(
        !probe.began(cooldown_start, 5105) && !probe.applied(cooldown_start, 5105),
        "cooldown did not block repeated use"
    );
    println!("PROOF reusable cast, unchanged unlimited charges and shared cooldown rejection");

    let interrupted_start = probe.events.len();
    probe.click(&zone, charged_slot).await?;
    probe.wait_begin(&mut zone, interrupted_start, 278).await?;
    zone.command(Command::InterruptSpell).await?;
    probe.pump(&mut zone, 2.).await?;
    ensure!(
        probe.events[interrupted_start..].iter().any(
            |e| matches!(e,GameplayEvent::CastInterrupted{id,..} if Some(*id)==probe.own || *id==0)
        ),
        "item cast interruption missing"
    );
    ensure!(
        !probe.applied(interrupted_start, 278)
            && probe.game.inventory.items[&charged_slot].charges == 10,
        "interrupted item cast consumed a charge"
    );
    let charged_start = probe.events.len();
    probe.click(&zone, charged_slot).await?;
    probe.pump(&mut zone, 6.).await?;
    ensure!(
        probe.began(charged_start, 278) && probe.applied(charged_start, 278),
        "charged bag cast did not finish"
    );
    ensure!(
        probe.game.inventory.items[&charged_slot].charges == 9,
        "charge reducer did not apply authoritative decrement"
    );
    println!(
        "PROOF interrupted bag click kept ten charges; successful bag click reduced ten to nine"
    );

    for remaining in [1, 0] {
        let start = probe.events.len();
        probe.click(&zone, cloudy_slot).await?;
        probe.pump(&mut zone, 3.).await?;
        ensure!(
            probe.applied(start, 42),
            "instant consumable cast did not apply"
        );
        let count = probe
            .game
            .inventory
            .items
            .get(&cloudy_slot)
            .map_or(0, |item| item.count);
        ensure!(
            count == remaining,
            "consumable stack count{count}, expected{remaining}"
        );
    }
    println!("PROOF instant bag consumable decremented then removed its final unit");
    zone.logout().await?;
    drop(zone);
    tokio::time::sleep(Duration::from_secs(2)).await;
    let mut zone = config.connect().await?;
    let mut persisted = Probe::new();
    persisted.pump(&mut zone, 5.).await?;
    ensure!(
        persisted
            .game
            .profile
            .as_ref()
            .context("reconnected profile")?
            .spell_book[book_slot as usize]
            == 288,
        "scribed book entry did not persist"
    );
    ensure!(
        !persisted.game.inventory.items.contains_key(&scroll_slot)
            && !persisted
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR),
        "consumed scroll persisted"
    );
    ensure!(
        !persisted.game.inventory.items.contains_key(&cloudy_slot),
        "consumed potion persisted"
    );
    ensure!(
        persisted.game.inventory.items[&charged_slot].charges == 9,
        "charge count did not persist"
    );
    let reusable = &persisted.game.inventory.items[&reusable_slot];
    ensure!(
        reusable.charges == -1 && reusable.recast_timestamp > 0,
        "reusable item/cooldown did not persist"
    );
    zone.logout().await?;
    println!(
        "RESULT scroll_consumption=verified spellbook_persistence=verified reusable_cast=verified shared_recast=verified charged_bag_cast=verified interrupted_charge_preservation=verified consumable_stack_and_final_deletion=verified inventory_persistence=verified"
    );
    Ok(())
}
