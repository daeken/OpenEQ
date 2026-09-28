//! Native-equivalent item use: original HUD hit targets dispatch through the
//! real Interaction/LiveWorld path. Mutates only the isolated Artificer fixture.
use anyhow::{Context, ensure};
use openeq::{
    game::StringTable,
    gameplay_ui::UiAction,
    hud::{Hud, HudState},
    interaction::Interaction,
    live::LiveWorld,
    spells::SpellCatalog,
};
use openeq_net::{gameplay::Command, inventory::InventorySlot, session::ConnectionConfig};
use openeq_render::{Camera, GpuScene, Renderer};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn poll(live: &mut LiveWorld, ui: &mut Interaction) -> anyhow::Result<()> {
    live.poll();
    ensure!(live.error.is_none(), "connection error: {:?}", live.error);
    if let Some(p) = live.initial_position.take() {
        live.camera_position(
            &Camera {
                position: [p.x, p.y, p.z + 3.],
                yaw: p.heading * std::f32::consts::TAU / 512.,
                ..Default::default()
            },
            false,
        );
    }
    ui.tick(live);
    Ok(())
}
fn wait(
    live: &mut LiveWorld,
    ui: &mut Interaction,
    label: &str,
    seconds: u64,
    test: impl Fn(&LiveWorld) -> bool,
) -> anyhow::Result<()> {
    let end = Instant::now() + Duration::from_secs(seconds);
    loop {
        poll(live, ui)?;
        if test(live) {
            return Ok(());
        }
        ensure!(
            Instant::now() < end,
            "{label} timed out; item status={}; recent chat={:?}",
            live.game.item_use.status,
            live.game
                .chat
                .iter()
                .rev()
                .take(4)
                .map(|line| &line.text)
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn connect(
    config: &ConnectionConfig,
    base: &Path,
    ui: &mut Interaction,
) -> anyhow::Result<LiveWorld> {
    let mut live = LiveWorld::start(config.clone());
    live.game.strings = StringTable::load(base);
    live.game.spell_catalog = SpellCatalog::load(base)?;
    wait(&mut live, ui, "initial player state", 25, |live| {
        live.ready
            && live.game.inventory.received
            && live.game.profile.is_some()
            && live.own_id.is_some()
    })?;
    Ok(live)
}
fn inspect(live: &mut LiveWorld, ui: &mut Interaction, slot: InventorySlot) -> anyhow::Result<()> {
    let position = live.player_position().context("player position")?;
    ui.ui_action(
        UiAction::InventorySlot(slot.server_slot().context("inventory address")? as i32),
        true,
        false,
        live,
        position,
    );
    ensure!(
        ui.inspected_owned
            .is_some_and(|(actual, _, _)| actual == slot),
        "owned inspection not selected"
    );
    Ok(())
}
fn frame(hud: &Hud, live: &LiveWorld, ui: &Interaction) -> openeq_ui::UiFrame {
    hud.gameplay_frame(
        [1280, 720],
        &HudState {
            character: live.character.clone(),
            player_level: live.game.profile.as_ref().map_or(0, |p| p.level),
            hp: live.game.hp.fraction().unwrap_or(1.),
            mana: live.game.mana.fraction(),
            endurance: live.game.endurance.fraction(),
            status: "Plane of Knowledge".into(),
            entities: live.entities.len(),
            movement_updates: live.moves,
            ..Default::default()
        },
        &ui.view(live),
    )
}
fn click(hud: &Hud, live: &mut LiveWorld, ui: &mut Interaction, id: &str) -> anyhow::Result<()> {
    let drawn = frame(hud, live, ui);
    let button = drawn
        .hit_targets
        .iter()
        .find(|hit| hit.item == id)
        .with_context(|| format!("rendered {id} button missing"))?;
    ensure!(button.enabled, "{id} button is disabled");
    let point = [
        button.rect.x + button.rect.width / 2.,
        button.rect.y + button.rect.height / 2.,
    ];
    let hit = drawn.hit_test(point).context("button is not topmost")?;
    ensure!(hit.item == id, "button covered by {}", hit.item);
    let action = UiAction::from_hit(hit).context("button action")?;
    let position = live.player_position().context("player position")?;
    ui.ui_action(action, false, false, live, position);
    ensure!(
        live.game.item_use.busy(),
        "item action was not queued: {}",
        live.game.item_use.status
    );
    Ok(())
}
fn capture(hud: &Hud, live: &LiveWorld, ui: &Interaction, path: &Path) -> anyhow::Result<()> {
    let mut renderer = Renderer::new_headless(1280, 720)?;
    let scene =
        openeq_assets::Scene::from_geometry("live item controls".into(), vec![], vec![], vec![]);
    let gpu = GpuScene::build(renderer.device(), renderer.queue(), &scene)?;
    renderer.set_scene(&gpu);
    renderer.set_ui(&frame(hud, live, ui));
    renderer.render(&gpu, &Camera::default());
    let (width, height, pixels) = renderer.read_rgba().context("GPU capture")?;
    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)?;
    println!("CAPTURE {}", path.display());
    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq=info,openeq_net=info")
        .init();
    let mut args = std::env::args().skip(1);
    let config = ConnectionConfig::load(Path::new(
        &args
            .next()
            .context("item_use_client_smoke CONFIG [CAPTURE_DIRECTORY]")?,
    ))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_itemuse"
            && config.character == "Artificer",
        "requires isolated Artificer fixture"
    );
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    std::fs::create_dir_all(&output)?;
    let base = openeq_assets::loader::default_client_dir().context("original client assets")?;
    let hud = Hud::load(&base)?;
    let mut ui = Interaction {
        inventory_open: true,
        ..Default::default()
    };
    ui.open_bags.insert(25);
    let mut live = connect(&config, &base, &mut ui)?;
    let scroll = InventorySlot::possessions(26);
    let reusable = InventorySlot::possessions(24);
    let potion = InventorySlot::possessions(25).in_bag(1);
    ensure!(
        live.game
            .inventory
            .items
            .get(&scroll)
            .is_some_and(|item| item.id == 15042 && item.count == 1),
        "unused Invisibility scroll26 required"
    );
    ensure!(
        !live.game.profile.as_ref().unwrap().spell_book.contains(&42),
        "Invisibility already known"
    );
    let initial_charges = live
        .game
        .inventory
        .items
        .get(&potion)
        .context("charged bag potion")?
        .charges;
    ensure!(initial_charges > 1, "at least two charges required");
    inspect(&mut live, &mut ui, scroll)?;
    ensure!(
        ui.view(&live)
            .item_use
            .as_ref()
            .is_some_and(|item| item.can_scribe),
        "scroll unexpectedly disabled"
    );
    capture(
        &hud,
        &live,
        &ui,
        &output.join("openeq-live-scroll-scribe.png"),
    )?;
    click(&hud, &mut live, &mut ui, "item:scribe")?;
    wait(
        &mut live,
        &mut ui,
        "scribe confirmation and cursor consumption",
        10,
        |live| {
            !live.game.item_use.busy()
                && live.game.profile.as_ref().unwrap().spell_book.contains(&42)
                && !live
                    .game
                    .inventory
                    .items
                    .contains_key(&InventorySlot::CURSOR)
        },
    )?;
    ensure!(
        !live.game.inventory.items.contains_key(&scroll),
        "scroll remained in carried inventory"
    );
    println!("PROOF rendered Scribe button -> one-scroll cursor FIFO -> book42 and consumption");

    inspect(&mut live, &mut ui, reusable)?;
    wait(
        &mut live,
        &mut ui,
        "reusable cooldown expiry",
        185,
        |live| {
            live.game
                .item_use
                .remaining(&live.game.inventory.items[&reusable], Instant::now())
                .is_zero()
        },
    )?;
    click(&hud, &mut live, &mut ui, "item:use")?;
    wait(&mut live, &mut ui, "reusable item completed", 10, |live| {
        !live.game.item_use.busy() && live.game.item_use.status == "Item effect completed."
    })?;
    ensure!(
        live.game.inventory.items[&reusable].charges == -1,
        "unlimited charges changed"
    );
    let view = ui.view(&live);
    ensure!(
        view.item_use
            .as_ref()
            .is_some_and(|item| !item.can_use && item.status.starts_with("Ready in")),
        "cooldown did not disable actual item button"
    );
    ensure!(
        view.inspected_item
            .as_ref()
            .unwrap()
            .details
            .iter()
            .any(|text| text == "Effect: Geomantra"),
        "actual effect metadata missing"
    );
    capture(
        &hud,
        &live,
        &ui,
        &output.join("openeq-live-item-recast.png"),
    )?;
    println!(
        "PROOF rendered Use -> reusable spell complete; disabled button shows server cooldown"
    );

    inspect(&mut live, &mut ui, potion)?;
    click(&hud, &mut live, &mut ui, "item:use")?;
    wait(&mut live, &mut ui, "charged cast begins", 5, |live| {
        live.game
            .casting
            .as_ref()
            .is_some_and(|cast| cast.spell_id == 278)
    })?;
    ensure!(
        live.command(Command::InterruptSpell),
        "interrupt queue failed"
    );
    wait(&mut live, &mut ui, "item interruption", 5, |live| {
        !live.game.item_use.busy() && live.game.item_use.status == "Item cast interrupted."
    })?;
    ensure!(
        live.game.inventory.items[&potion].charges == initial_charges,
        "interrupted UI click consumed a charge"
    );
    let recovery = Instant::now();
    wait(&mut live, &mut ui, "cast recovery", 5, |_| {
        recovery.elapsed() > Duration::from_secs(2)
    })?;
    click(&hud, &mut live, &mut ui, "item:use")?;
    wait(&mut live, &mut ui, "charged cast completion", 10, |live| {
        !live.game.item_use.busy()
            && live.game.inventory.items[&potion].charges == initial_charges - 1
    })?;
    capture(
        &hud,
        &live,
        &ui,
        &output.join("openeq-live-item-charges.png"),
    )?;
    println!(
        "PROOF interrupted bag UI click preserved {initial_charges} charges; completed click left {}",
        initial_charges - 1
    );
    drop(live);
    std::thread::sleep(Duration::from_secs(2));
    let mut ui = Interaction::default();
    let live = connect(&config, &base, &mut ui)?;
    ensure!(
        live.game.profile.as_ref().unwrap().spell_book.contains(&42),
        "UI-scribed spell did not persist"
    );
    ensure!(
        !live.game.inventory.items.contains_key(&scroll)
            && !live
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR),
        "UI-consumed scroll reappeared"
    );
    ensure!(
        live.game.inventory.items[&potion].charges == initial_charges - 1,
        "UI charge update did not persist"
    );
    drop(live);
    std::thread::sleep(Duration::from_secs(1));
    println!(
        "RESULT rendered_hit_dispatch=verified command_sent_scribe_fifo=verified unlimited_click=verified shared_cooldown_controls=verified charged_click_interruption=verified charge_reduction=verified reconnect_persistence=verified"
    );
    Ok(())
}
