//! Bounded production-UI stack proof for the disposable Barterer fixture.
//! Requires a fresh private fixture.py baseline; it never provisions inventory.
use anyhow::{Context, ensure};
use openeq::{gameplay_ui::UiAction, interaction::Interaction, live::LiveWorld};
use openeq_net::{gameplay::Command, inventory::InventorySlot, session::ConnectionConfig};
use std::{
    io::Write,
    path::Path,
    process::{Command as Process, Stdio},
    time::{Duration, Instant},
};

fn sql(query: &str) -> anyhow::Result<String> {
    let mut process = Process::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=10",
            "storage2.daeken.dev",
            "sudo mariadb peq -N -B --raw",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    process
        .stdin
        .take()
        .context("SQL input")?
        .write_all(query.as_bytes())?;
    let output = process.wait_with_output()?;
    ensure!(output.status.success(), "fixture SQL failed");
    Ok(String::from_utf8(output.stdout)?)
}

fn offline() -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if sql("SELECT ingame FROM character_data WHERE id=8 AND name='Barterer' AND account_id=10 AND account_id=(SELECT id FROM account WHERE name='openeq_trade1');")?.trim() == "0" {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "dedicated fixture did not log out"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn baseline(path: &Path) -> anyhow::Result<()> {
    let metadata = std::fs::metadata(path)?;
    ensure!(
        metadata.modified()?.elapsed()? < Duration::from_secs(1800),
        "take a fresh private snapshot before this proof"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "fixture baseline must be private"
        );
    }
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let columns = value["descriptor"]["character_data"]
        .as_array()
        .context("snapshot descriptor")?;
    let rows = value["snapshot"]["character_data"]
        .as_array()
        .context("snapshot character")?;
    ensure!(rows.len() == 1, "one dedicated fixture required");
    for (field, expected) in [
        ("id", serde_json::json!(8)),
        ("name", serde_json::json!("Barterer")),
        ("account_id", serde_json::json!(10)),
        ("ingame", serde_json::json!(0)),
    ] {
        let index = columns
            .iter()
            .position(|column| column[0] == field)
            .context("snapshot identity column")?;
        ensure!(rows[0][index] == expected, "snapshot identity mismatch");
    }
    ensure!(
        value["snapshot"]
            .as_object()
            .context("snapshot tables")?
            .len()
            >= 29,
        "full gameplay snapshot required"
    );
    offline()
}

fn poll(live: &mut LiveWorld, ui: &mut Interaction) -> anyhow::Result<()> {
    live.poll();
    ensure!(live.error.is_none(), "fixture connection failed");
    ui.tick(live);
    Ok(())
}

fn wait(
    live: &mut LiveWorld,
    ui: &mut Interaction,
    condition: impl Fn(&LiveWorld) -> bool,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        poll(live, ui)?;
        if condition(live) {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "fixture operation timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn local(live: &LiveWorld) -> Vec<(i32, u32, u32)> {
    let mut items: Vec<_> = live
        .game
        .inventory
        .items
        .values()
        .map(|item| {
            (
                item.slot.server_slot().unwrap_or(u32::MAX) as i32,
                item.id,
                item.count,
            )
        })
        .collect();
    items.sort_unstable();
    items
}

fn check(
    live: &mut LiveWorld,
    ui: &mut Interaction,
    expected: &[(i32, u32, u32)],
    label: &str,
) -> anyhow::Result<()> {
    wait(live, ui, |live| {
        !live.game.inventory_command_pending && local(live) == expected
    })?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let rows = expected
        .iter()
        .map(|(slot, id, count)| format!("{slot}\t{id}\t{count}"))
        .collect::<Vec<_>>()
        .join("\n");
    loop {
        let actual = sql(
            "SELECT slot_id,item_id,charges FROM inventory WHERE character_id=8 ORDER BY slot_id;",
        )?;
        poll(live, ui)?;
        if actual.trim() == rows {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "server inventory did not match {label}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        local(live) == expected,
        "foreground changed during SQL verification"
    );
    println!("PROOF {label}: foreground_and_server_match=true");
    Ok(())
}

fn click(live: &mut LiveWorld, ui: &mut Interaction, slot: i32, split: bool) {
    ui.ui_action(UiAction::InventorySlot(slot), false, split, live, [0.; 3]);
}

fn connect(config: &ConnectionConfig, ui: &mut Interaction) -> anyhow::Result<LiveWorld> {
    let mut live = LiveWorld::start(config.clone());
    // No camera updates, movement, target selection, sound or gameplay requests.
    wait(&mut live, ui, |live| {
        live.ready
            && live.own_id.is_some()
            && live.game.inventory.received
            && live.game.profile.is_some()
    })?;
    Ok(live)
}

fn proof(config: &ConnectionConfig) -> anyhow::Result<()> {
    let mut ui = Interaction::default();
    let mut live = connect(config, &mut ui)?;
    let original = [(23, 5001, 1), (24, 13006, 20)];
    check(&mut live, &mut ui, &original, "baseline")?;
    click(&mut live, &mut ui, 24, true);
    check(
        &mut live,
        &mut ui,
        &[(23, 5001, 1), (24, 13006, 19), (33, 13006, 1)],
        "shift_split",
    )?;
    click(&mut live, &mut ui, 24, false);
    check(&mut live, &mut ui, &original, "ordinary_merge")?;
    ensure!(
        live.command(Command::MoveItem {
            from: InventorySlot::possessions(24),
            to: InventorySlot::CURSOR,
            count: 3
        }),
        "split was not queued"
    );
    check(
        &mut live,
        &mut ui,
        &[(23, 5001, 1), (24, 13006, 17), (33, 13006, 3)],
        "three_item_split",
    )?;
    ensure!(
        live.command(Command::MoveItem {
            from: InventorySlot::CURSOR,
            to: InventorySlot::possessions(24),
            count: 0
        }),
        "zero-count swap was not queued"
    );
    check(
        &mut live,
        &mut ui,
        &[(23, 5001, 1), (24, 13006, 3), (33, 13006, 17)],
        "zero_count_same_id_swap",
    )?;
    click(&mut live, &mut ui, 24, false);
    check(&mut live, &mut ui, &original, "merge_after_swap")?;
    click(&mut live, &mut ui, 23, true);
    check(
        &mut live,
        &mut ui,
        &[(24, 13006, 20), (33, 5001, 1)],
        "shift_nonstackable_pickup",
    )?;
    click(&mut live, &mut ui, 23, true);
    check(&mut live, &mut ui, &original, "shift_nonstackable_drop")?;
    drop(live);
    offline()?;
    let mut live = connect(config, &mut ui)?;
    check(&mut live, &mut ui, &original, "normal_reconnect")?;
    drop(live);
    offline()
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: inventory_stack_smoke PRIVATE_BASELINE_JSON")?;
    baseline(Path::new(&path))?;
    let config = ConnectionConfig::load(
        &std::path::PathBuf::from(std::env::var("HOME")?)
            .join(".config/openeq/storage2-trade1-credentials.json"),
    )?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.character == "Barterer"
            && config.username == "openeq_trade1",
        "dedicated fixture configuration required"
    );
    let result = proof(&config);
    // Keep the process alive for LiveWorld's normal logout even on a failed assertion.
    let logout = offline();
    result?;
    logout?;
    println!("FINISHED dedicated_fixture_offline=true restore_private_baseline_next=true");
    Ok(())
}
