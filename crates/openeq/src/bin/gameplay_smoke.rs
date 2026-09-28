//! Integrated live client reducer/interaction probe. Mutates only Arcanist.
use anyhow::{Context, ensure};
use openeq::{
    game::StringTable, gameplay_ui::UiAction, interaction::Interaction, live::LiveWorld,
    spells::SpellCatalog,
};
use openeq_net::{inventory::InventorySlot, session::ConnectionConfig};
use openeq_render::Camera;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn poll(live: &mut LiveWorld) -> anyhow::Result<()> {
    live.poll();
    ensure!(
        live.error.is_none(),
        "live connection failed: {:?}",
        live.error
    );
    if let Some(p) = live.initial_position {
        live.camera_position(
            &Camera {
                position: [p.x, p.y, p.z + 3.],
                yaw: p.heading * std::f32::consts::TAU / 512.,
                ..Default::default()
            },
            false,
        );
    }
    Ok(())
}
fn wait(
    live: &mut LiveWorld,
    label: &str,
    seconds: u64,
    condition: impl Fn(&LiveWorld) -> bool,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        poll(live)?;
        if condition(live) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for {label}; recent chat: {:?}",
            live.game
                .chat
                .iter()
                .rev()
                .take(4)
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn connect(config: &ConnectionConfig, base: &Path) -> anyhow::Result<LiveWorld> {
    let mut live = LiveWorld::start(config.clone());
    live.game.strings = StringTable::load(base);
    live.game.spell_catalog = SpellCatalog::load(base)?;
    wait(
        &mut live,
        "initial profile, inventory, resources and spawn",
        25,
        |live| {
            live.ready
                && live.own_id.is_some()
                && live.game.inventory.received
                && live.game.profile.is_some()
                && live.game.mana.maximum.is_some()
        },
    )?;
    Ok(live)
}
fn reconnect(live: LiveWorld, config: &ConnectionConfig, base: &Path) -> anyhow::Result<LiveWorld> {
    drop(live); // Closing LiveWorld sends the network worker through normal logout.
    std::thread::sleep(Duration::from_secs(2));
    connect(config, base)
}
fn position(live: &LiveWorld) -> [f32; 3] {
    live.initial_position.map_or([0.; 3], |p| [p.x, p.y, p.z])
}
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq=info,openeq_net=info")
        .init();
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .context("usage: gameplay_smoke CONFIG [EVERQUEST_DIRECTORY]")?,
    );
    let base = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest"));
    let config = ConnectionConfig::load(&path)?;
    ensure!(
        config.character == "Arcanist"
            && config.username == "openeq_spells"
            && config.host == "storage2.daeken.dev",
        "integrated probe requires the dedicated Arcanist fixture"
    );
    let mut live = connect(&config, &base)?;
    let mut ui = Interaction::default();
    let profile = live.game.profile.as_ref().unwrap();
    ensure!(
        profile.name == "Arcanist" && profile.class == 12 && profile.spell_book.contains(&288),
        "caster profile mismatch"
    );
    let shield_gem = profile
        .memorized_spells
        .iter()
        .take(12)
        .position(|id| *id == 288)
        .context("Minor Shielding must already be memorized")? as u8;
    let gate_gem = profile
        .memorized_spells
        .iter()
        .take(12)
        .position(|id| *id == 36)
        .map(|gem| gem as u8);
    let view = ui.view(&live);
    ensure!(
        view.memorized.iter().any(|gem| gem.gem == shield_gem
            && gem
                .spell
                .as_ref()
                .is_some_and(|spell| spell.id == 288 && spell.name == "Minor Shielding")),
        "spell presentation did not use the original catalog"
    );
    println!("initial profile, inventory, gems and original spell names verified");

    let marker = format!("OpenEQ integrated smoke {}", std::process::id());
    let pos = position(&live);
    ui.submit(&format!("/tell Arcanist {marker}"), &mut live, pos);
    wait(
        &mut live,
        "self-tell recipient and sender echoes",
        8,
        |live| {
            live.game
                .chat
                .iter()
                .filter(|line| line.text.contains(&marker))
                .count()
                >= 2
        },
    )?;
    println!("self-tell recipient and sender text verified");

    let existing: Vec<_> = live
        .game
        .buffs
        .values()
        .filter(|buff| buff.spell_id == 288)
        .map(|buff| buff.slot)
        .collect();
    for slot in existing {
        ui.ui_action(UiAction::RemoveBuff(slot), true, false, &mut live, pos);
        wait(&mut live, "old shield removal", 5, |live| {
            !live.game.buffs.contains_key(&slot)
        })?;
    }
    wait(&mut live, "shield gem ready", 15, |live| {
        ui.view(live)
            .memorized
            .iter()
            .any(|gem| gem.gem == shield_gem && gem.ready)
    })?;
    let mana = live.game.mana.current.context("current mana")?;
    ui.cast(shield_gem, &mut live);
    ensure!(
        live.game.cast_pending_until.is_some(),
        "cast did not enter pending state"
    );
    wait(&mut live, "server casting state", 4, |live| {
        live.game
            .casting
            .as_ref()
            .is_some_and(|cast| cast.spell_id == 288)
    })?;
    ensure!(
        ui.view(&live)
            .casting
            .as_ref()
            .is_some_and(|cast| cast.label == "Minor Shielding"),
        "casting presentation missing"
    );
    wait(
        &mut live,
        "completed shield with mana cost and cooldown",
        7,
        |live| {
            live.game.casting.is_none()
                && live.game.cast_pending_until.is_none()
                && live.game.buffs.values().any(|buff| buff.spell_id == 288)
                && live.game.mana.current.is_some_and(|current| current < mana)
                && live
                    .game
                    .spell_cooldowns
                    .get(&shield_gem)
                    .is_some_and(|until| *until > Instant::now())
        },
    )?;
    let buff_slot = live
        .game
        .buffs
        .values()
        .find(|buff| buff.spell_id == 288)
        .unwrap()
        .slot;
    let remaining = live.game.buff_seconds(buff_slot).context("buff duration")?;
    ensure!(
        remaining > 1500 && remaining <= 1620,
        "incorrect buff duration: {remaining}"
    );
    ensure!(
        ui.view(&live)
            .buffs
            .iter()
            .any(|buff| buff.slot == buff_slot && buff.spell.name == "Minor Shielding"),
        "buff presentation missing"
    );
    println!(
        "cast pending/start/finish, mana {}->{}, cooldown and buff duration {remaining}s verified",
        mana,
        live.game.mana.current.unwrap()
    );
    ui.ui_action(UiAction::RemoveBuff(buff_slot), true, false, &mut live, pos);
    wait(&mut live, "shield disappearance", 5, |live| {
        !live.game.buffs.contains_key(&buff_slot)
    })?;
    ensure!(
        !ui.view(&live)
            .buffs
            .iter()
            .any(|buff| buff.slot == buff_slot),
        "removed buff remains in presentation"
    );

    let interruption = if let Some(gem) = gate_gem {
        wait(&mut live, "Gate ready", 15, |live| {
            ui.view(live)
                .memorized
                .iter()
                .any(|entry| entry.gem == gem && entry.ready)
        })?;
        ui.cast(gem, &mut live);
        wait(&mut live, "Gate start", 4, |live| {
            live.game
                .casting
                .as_ref()
                .is_some_and(|cast| cast.spell_id == 36)
        })?;
        ui.submit("/stopcast", &mut live, pos);
        wait(&mut live, "interrupted Gate", 4, |live| {
            live.game.casting.is_none() && live.game.cast_pending_until.is_none()
        })?;
        ensure!(
            live.game
                .spell_cooldowns
                .get(&gem)
                .is_none_or(|until| *until <= Instant::now()),
            "interrupted Gate gained a cooldown"
        );
        "verified"
    } else {
        "skipped (Gate not memorized)"
    };

    // Choose only the fixture's known water stack, preserve it whole, and prove
    // both directions with fresh server inventory rather than trusting prediction.
    let slot = InventorySlot::possessions(24);
    let item = live
        .game
        .inventory
        .items
        .get(&slot)
        .filter(|item| item.id == 13006)
        .cloned();
    let inventory = if let Some(item) = item.filter(|_| {
        !live
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::CURSOR)
    }) {
        ui.ui_action(UiAction::InventorySlot(24), false, false, &mut live, pos);
        wait(&mut live, "cursor prediction", 3, |live| {
            !live.game.inventory_command_pending
                && !live.game.inventory.items.contains_key(&slot)
                && live
                    .game
                    .inventory
                    .items
                    .get(&InventorySlot::CURSOR)
                    .is_some_and(|cursor| cursor.id == item.id && cursor.count == item.count)
        })?;
        ensure!(
            ui.view(&live)
                .cursor_item
                .as_ref()
                .is_some_and(|cursor| cursor.id == item.id),
            "cursor presentation missing"
        );
        live = reconnect(live, &config, &base)?;
        ensure!(
            live.game
                .inventory
                .items
                .get(&InventorySlot::CURSOR)
                .is_some_and(|cursor| cursor.id == item.id && cursor.count == item.count)
                && !live.game.inventory.items.contains_key(&slot),
            "cursor move did not persist"
        );
        let pos = position(&live);
        ui.ui_action(UiAction::InventorySlot(24), false, false, &mut live, pos);
        wait(&mut live, "inventory restoration", 3, |live| {
            !live.game.inventory_command_pending
                && !live
                    .game
                    .inventory
                    .items
                    .contains_key(&InventorySlot::CURSOR)
                && live
                    .game
                    .inventory
                    .items
                    .get(&slot)
                    .is_some_and(|restored| restored.id == item.id && restored.count == item.count)
        })?;
        live = reconnect(live, &config, &base)?;
        ensure!(
            live.game
                .inventory
                .items
                .get(&slot)
                .is_some_and(|restored| restored.id == item.id && restored.count == item.count)
                && !live
                    .game
                    .inventory
                    .items
                    .contains_key(&InventorySlot::CURSOR),
            "restoration did not persist"
        );
        "cursor move + restoration persisted"
    } else {
        "skipped (water stack unavailable or cursor occupied)"
    };
    drop(live);
    std::thread::sleep(Duration::from_secs(1));
    println!(
        "RESULT integrated_profile=verified chat=verified casting=verified mana=verified cooldown=verified buffs=verified interruption={interruption} inventory={inventory}"
    );
    Ok(())
}
