//! Live UI/reducer commerce proof; mutates only the dedicated Broker fixture.
use anyhow::{Context, ensure};
use openeq::{
    chat::Action,
    commerce::{BANKER_CLASS, MERCHANT_CLASS, total_copper},
    commerce_ui::CommerceAction,
    game::StringTable,
    gameplay_ui::UiAction,
    interaction::Interaction,
    live::LiveWorld,
};
use openeq_assets::collision::CollisionWorld;
use openeq_net::{
    gameplay::{Command, Currency},
    inventory::InventorySlot,
    session::ConnectionConfig,
};
use openeq_render::Camera;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn coins(c: Currency) -> [u32; 4] {
    [c.platinum, c.gold, c.silver, c.copper]
}
fn poll(live: &mut LiveWorld, ui: &mut Interaction) -> anyhow::Result<()> {
    live.poll();
    ensure!(live.error.is_none(), "connection failed: {:?}", live.error);
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
    condition: impl Fn(&LiveWorld) -> bool,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        poll(live, ui)?;
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
                .take(5)
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}
fn settle(live: &mut LiveWorld, ui: &mut Interaction) -> anyhow::Result<()> {
    let start = Instant::now();
    wait(live, ui, "network settling", 3, |_| {
        start.elapsed() >= Duration::from_secs(1)
    })
}
fn connect(
    config: &ConnectionConfig,
    base: &Path,
    ui: &mut Interaction,
) -> anyhow::Result<LiveWorld> {
    let mut live = LiveWorld::start(config.clone());
    live.game.strings = StringTable::load(base);
    wait(&mut live, ui, "initial commerce state", 25, |live| {
        live.ready
            && live.own_id.is_some()
            && live.game.profile.is_some()
            && live.game.inventory.received
    })?;
    // Bank/shared items can arrive individually after the main inventory packet.
    settle(&mut live, ui)?;
    Ok(live)
}
fn reconnect(
    live: LiveWorld,
    config: &ConnectionConfig,
    base: &Path,
    ui: &mut Interaction,
) -> anyhow::Result<LiveWorld> {
    drop(live);
    std::thread::sleep(Duration::from_secs(2));
    connect(config, base, ui)
}
fn action(ui: &mut Interaction, live: &mut LiveWorld, action: UiAction) {
    let position = live.player_position().unwrap_or([0.; 3]);
    ui.ui_action(action, false, false, live, position);
}
fn commerce(ui: &mut Interaction, live: &mut LiveWorld, command: CommerceAction) {
    action(ui, live, UiAction::Commerce(command));
}
fn approach(
    live: &mut LiveWorld,
    ui: &mut Interaction,
    world: &CollisionWorld,
    name: &str,
    class: u8,
) -> anyhow::Result<u32> {
    let entity = live
        .entities
        .values()
        .find(|e| e.spawn.class == class && e.spawn.name.starts_with(name))
        .with_context(|| format!("missing service NPC {name}"))?;
    let id = entity.spawn.id;
    let target = entity.position(Instant::now());
    // NPC center height depends on race and size. The human fixture must stand
    // above the actual asset floor rather than copying a small NPC's center.
    let destination = [[0., -10.], [10., 0.], [0., 10.], [-10., 0.]]
        .into_iter()
        .find_map(|offset| {
            let [x, y] = [target[0] + offset[0], target[1] + offset[1]];
            world
                .ground_height(x, y, target[2], 10., 30.)
                .map(|floor| [x, y, floor + 3.125])
        })
        .context("no nearby asset floor for commerce fixture")?;
    let server = openeq::coordinates::scene_point_to_server(destination);
    let position = live.player_position().unwrap_or([0.; 3]);
    // #goto takes server X/Y/Z, whereas LiveWorld and collision use scene axes.
    // Await the actual server correction instead of assuming a large movement
    // update bypasses the server's anti-warp checks.
    ui.submit(
        &format!("/say #goto {} {} {}", server[0], server[1], server[2]),
        live,
        position,
    );
    wait(live, ui, "server-approved service approach", 8, |live| {
        live.player_position().is_some_and(|position| {
            position
                .into_iter()
                .zip(destination)
                .all(|(actual, expected)| (actual - expected).abs() < 0.2)
        })
    })?;
    settle(live, ui)?;
    ensure!(live.service_available(id, class), "NPC is out of reach");
    live.set_target(Some(id));
    Ok(id)
}
fn item_at(live: &LiveWorld, slot: InventorySlot, id: u32, count: u32) -> bool {
    live.game
        .inventory
        .items
        .get(&slot)
        .is_some_and(|item| item.id == id && item.count == count)
}
fn move_by_click(
    live: &mut LiveWorld,
    ui: &mut Interaction,
    from: InventorySlot,
    to: InventorySlot,
) -> anyhow::Result<()> {
    ensure!(
        !live
            .game
            .inventory
            .items
            .contains_key(&InventorySlot::CURSOR)
            && !live.game.inventory.items.contains_key(&to),
        "fixture cursor or destination occupied"
    );
    let item = live
        .game
        .inventory
        .items
        .get(&from)
        .context("source item")?;
    let (id, count) = (item.id, item.count);
    action(
        ui,
        live,
        UiAction::InventorySlot(from.server_slot().unwrap() as i32),
    );
    wait(live, ui, "item cursor pickup", 5, |live| {
        !live.game.inventory_command_pending
            && !live.game.inventory.items.contains_key(&from)
            && item_at(live, InventorySlot::CURSOR, id, count)
    })?;
    ensure!(
        ui.view(live).cursor_item.is_some_and(|item| item.id == id),
        "cursor presentation missing"
    );
    action(
        ui,
        live,
        UiAction::InventorySlot(to.server_slot().unwrap() as i32),
    );
    wait(live, ui, "item cursor placement", 5, |live| {
        !live.game.inventory_command_pending
            && !live
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
            && item_at(live, to, id, count)
    })
}
fn open_bank(
    live: &mut LiveWorld,
    ui: &mut Interaction,
    world: &CollisionWorld,
) -> anyhow::Result<()> {
    approach(live, ui, world, "Dogle_Pitt", BANKER_CLASS)?;
    let position = live.player_position().unwrap();
    ui.action(Action::Bank, live, position);
    ensure!(ui.view(live).bank.is_some(), "bank window did not open");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq=info,openeq_net=info")
        .init();
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .context("commerce_smoke CONFIG [EVERQUEST_DIRECTORY]")?,
    );
    let base = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest"));
    let config = ConnectionConfig::load(&path)?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_commerce"
            && config.character == "Broker",
        "requires the dedicated Broker fixture"
    );
    let scene = openeq_assets::loader::load_zone(&base, "poknowledge")?;
    let collision = CollisionWorld::build(&scene);
    let mut ui = Interaction::default();
    let mut live = connect(&config, &base, &mut ui)?;
    let cap = InventorySlot::possessions(24);
    let water = InventorySlot::possessions(23);
    let bank = InventorySlot::bank(0);
    let shared = InventorySlot::shared_bank(0);
    ensure!(
        item_at(&live, cap, 1001, 1)
            && item_at(&live, water, 13006, 20)
            && !live
                .game
                .inventory
                .items
                .keys()
                .any(|slot| slot.kind == 1 || slot.kind == 2)
            && !live
                .game
                .inventory
                .items
                .values()
                .any(|item| item.id == 13005)
            && total_copper(live.game.commerce.bank_money) == 0
            && live.game.commerce.shared_platinum == 0,
        "fixture differs from restored baseline; resolve previous run before mutating"
    );
    let starting_money = total_copper(live.game.currency);
    approach(&mut live, &mut ui, &collision, "Amile_Pitt", MERCHANT_CLASS)?;
    let position = live.player_position().unwrap();
    ui.action(Action::Merchant, &mut live, position);
    wait(&mut live, &mut ui, "merchant catalog", 8, |live| {
        live.game
            .commerce
            .merchant
            .as_ref()
            .is_some_and(|merchant| {
                merchant.opened && merchant.items.values().any(|item| item.id == 13005)
            })
    })?;
    let merchant = live.game.commerce.merchant.as_ref().unwrap();
    let offer = merchant
        .items
        .values()
        .find(|item| item.id == 13005)
        .unwrap();
    let slot = u32::from(offer.slot.slot);
    let cost = offer.price * 2;
    ensure!(
        offer.price > 0 && offer.merchant_count == -1,
        "invalid server quote/stock"
    );
    commerce(&mut ui, &mut live, CommerceAction::SelectStock(slot));
    commerce(&mut ui, &mut live, CommerceAction::SetQuantity(2));
    let view = ui.view(&live).merchant.context("merchant presentation")?;
    ensure!(
        view.can_buy
            && view.quantity == 2
            && view.stock.iter().any(|entry| {
                entry.slot == slot && entry.unit_price_copper == Some(u64::from(cost / 2))
            }),
        "merchant quote/quantity presentation mismatch"
    );
    commerce(&mut ui, &mut live, CommerceAction::Buy);
    ensure!(
        live.game.commerce.pending.is_some(),
        "purchase did not enter pending state"
    );
    wait(
        &mut live,
        &mut ui,
        "purchase delivery and acknowledged debit",
        8,
        |live| {
            live.game.commerce.pending.is_none()
                && total_copper(live.game.currency) == starting_money - u64::from(cost)
                && live
                    .game
                    .inventory
                    .items
                    .values()
                    .any(|item| item.id == 13005 && item.count == 2)
        },
    )?;
    let bought_slot = live
        .game
        .inventory
        .items
        .values()
        .find(|item| item.id == 13005)
        .unwrap()
        .slot;
    action(
        &mut ui,
        &mut live,
        UiAction::InventorySlot(bought_slot.server_slot().unwrap() as i32),
    );
    commerce(&mut ui, &mut live, CommerceAction::SetQuantity(2));
    ensure!(
        ui.view(&live).merchant.is_some_and(|view| view.can_sell),
        "sale not available in UI"
    );
    commerce(&mut ui, &mut live, CommerceAction::Sell);
    ensure!(
        live.game.commerce.pending.is_some(),
        "sale did not enter pending state"
    );
    wait(
        &mut live,
        &mut ui,
        "sale removal and authoritative money",
        8,
        |live| {
            live.game.commerce.pending.is_none()
                && !live
                    .game
                    .inventory
                    .items
                    .values()
                    .any(|item| item.id == 13005)
                && total_copper(live.game.currency) > starting_money - u64::from(cost)
        },
    )?;
    let after_sale = live.game.currency;
    let sale_total = total_copper(after_sale) - (starting_money - u64::from(cost));
    println!(
        "merchant UI/reducer proof: bought 2 for {cost}, sold 2 for {sale_total}; wallet {:?}",
        coins(after_sale)
    );
    action(&mut ui, &mut live, UiAction::CloseWindow("merchant".into()));
    ensure!(
        ui.view(&live).merchant.is_none(),
        "merchant failed to close"
    );
    settle(&mut live, &mut ui)?;

    ensure!(
        !live.command(Command::MoveItem {
            from: cap,
            to: bank,
            count: 0
        }),
        "bank move accepted without an open banker session"
    );
    open_bank(&mut live, &mut ui, &collision)?;
    move_by_click(&mut live, &mut ui, cap, bank)?;
    move_by_click(&mut live, &mut ui, water, shared)?;
    let view = ui.view(&live).bank.context("bank presentation")?;
    ensure!(
        view.slots[0]
            .item
            .as_ref()
            .is_some_and(|item| item.id == 1001)
            && view.shared_slots[0]
                .item
                .as_ref()
                .is_some_and(|item| item.id == 13006),
        "bank item presentation mismatch"
    );
    commerce(&mut ui, &mut live, CommerceAction::BankCoin(3));
    commerce(&mut ui, &mut live, CommerceAction::Deposit);
    wait(&mut live, &mut ui, "personal coin deposit", 5, |live| {
        !live.game.commerce.coin_pending && live.game.commerce.bank_money.platinum == 1
    })?;
    commerce(&mut ui, &mut live, CommerceAction::DepositShared);
    wait(&mut live, &mut ui, "shared coin deposit", 5, |live| {
        !live.game.commerce.coin_pending && live.game.commerce.shared_platinum == 1
    })?;
    let mut deposited = coins(after_sale);
    deposited[0] -= 2;
    ensure!(
        coins(live.game.currency) == deposited,
        "coin prediction changed other denominations"
    );
    settle(&mut live, &mut ui)?;
    live = reconnect(live, &config, &base, &mut ui)?;
    ensure!(
        item_at(&live, bank, 1001, 1)
            && item_at(&live, shared, 13006, 20)
            && !live.game.inventory.items.contains_key(&cap)
            && !live.game.inventory.items.contains_key(&water)
            && !live
                .game
                .inventory
                .items
                .values()
                .any(|item| item.id == 13005)
            && coins(live.game.currency) == deposited
            && live.game.commerce.bank_money.platinum == 1
            && live.game.commerce.shared_platinum == 1,
        "bank deposits or merchant results did not persist"
    );
    println!("bank items and both platinum deposits persisted with exact carried denominations");
    open_bank(&mut live, &mut ui, &collision)?;
    move_by_click(&mut live, &mut ui, bank, cap)?;
    move_by_click(&mut live, &mut ui, shared, water)?;
    commerce(&mut ui, &mut live, CommerceAction::BankCoin(3));
    commerce(&mut ui, &mut live, CommerceAction::Withdraw);
    wait(&mut live, &mut ui, "personal coin withdrawal", 5, |live| {
        !live.game.commerce.coin_pending && live.game.commerce.bank_money.platinum == 0
    })?;
    commerce(&mut ui, &mut live, CommerceAction::WithdrawShared);
    wait(&mut live, &mut ui, "shared coin withdrawal", 5, |live| {
        !live.game.commerce.coin_pending && live.game.commerce.shared_platinum == 0
    })?;
    ensure!(
        coins(live.game.currency) == coins(after_sale),
        "coin restoration mismatch"
    );
    let position = live.player_position().unwrap();
    live.camera_position(
        &Camera {
            position: [position[0] + 500., position[1], position[2] + 3.],
            ..Default::default()
        },
        false,
    );
    ui.tick(&mut live);
    ensure!(
        live.game.commerce.bank.is_none(),
        "bank access remained open out of reach"
    );
    ensure!(
        !live.command(Command::MoveItem {
            from: cap,
            to: bank,
            count: 0
        }),
        "out-of-reach bank move was accepted"
    );
    open_bank(&mut live, &mut ui, &collision)?;
    action(&mut ui, &mut live, UiAction::CloseWindow("bank".into()));
    settle(&mut live, &mut ui)?;
    live = reconnect(live, &config, &base, &mut ui)?;
    ensure!(
        item_at(&live, cap, 1001, 1)
            && item_at(&live, water, 13006, 20)
            && !live
                .game
                .inventory
                .items
                .keys()
                .any(|slot| slot.kind == 1 || slot.kind == 2)
            && !live
                .game
                .inventory
                .items
                .contains_key(&InventorySlot::CURSOR)
            && coins(live.game.currency) == coins(after_sale)
            && total_copper(live.game.commerce.bank_money) == 0
            && live.game.commerce.shared_platinum == 0,
        "bank restoration did not persist"
    );
    let profile = live.game.profile.as_ref().unwrap();
    ensure!(
        coins(profile.currency) == coins(after_sale)
            && total_copper(profile.bank_currency) == 0
            && profile.shared_platinum == 0,
        "fresh profile balances disagree"
    );
    let position = live.player_position().context("restored player position")?;
    let floor = collision
        .ground_height(position[0], position[1], position[2], 10., 30.)
        .context("restored fixture has no asset floor")?;
    ensure!(
        position[2] >= floor + 3. && position[2] < floor + 4.,
        "restored player center is not above its floor: {position:?}, floor {floor}"
    );
    println!("restored scene center {position:?}, asset floor {floor}");
    drop(live);
    std::thread::sleep(Duration::from_secs(1));
    println!(
        "RESULT integrated_merchant_ui=verified purchase_delivery_debit=verified sale_removal_currency=verified bank_ui=verified service_validation=verified bank_shared_items=roundtrip_persisted bank_shared_coins=roundtrip_persisted purchase_total={cost} sale_total={sale_total}"
    );
    Ok(())
}
