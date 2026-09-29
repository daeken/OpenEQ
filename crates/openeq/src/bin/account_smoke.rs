//! Real interactive controller flow, restricted to the disposable Reviver.
//! No direct automatic connect, player movement, or gameplay requests.
use anyhow::{Context, ensure};
use openeq::account::{AccountController, Action, Endpoint, Ready, Stage};
use openeq_net::session::ConnectionConfig;
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const FIXTURE: &str =
    "name='Reviver' AND account_id=(SELECT id FROM account WHERE name='openeq_recovery')";
const FIELDS: &str =
    "zone_id,zone_instance,x,y,z,heading,cur_hp,mana,endurance,hunger_level,thirst_level";
const IDLE: Duration = Duration::from_secs(35);

fn sql(query: &str) -> anyhow::Result<String> {
    let mut child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "storage2.daeken.dev",
            "sudo mariadb peq -N -B",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("SQL input")?
        .write_all(query.as_bytes())?;
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "fixture query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}
fn offline() -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(25);
    loop {
        if sql(&format!(
            "SELECT ingame FROM character_data WHERE {FIXTURE};"
        ))?
        .trim()
            == "0"
        {
            return Ok(());
        }
        ensure!(Instant::now() < until, "Reviver remains online");
        std::thread::sleep(Duration::from_millis(300));
    }
}
fn invariant() -> anyhow::Result<String> {
    let id = format!("(SELECT id FROM character_data WHERE {FIXTURE})");
    sql(&format!("SELECT level,exp,gm,race,class,`str`,sta,cha,dex,`int`,agi,wis FROM character_data WHERE {FIXTURE};
        SELECT COUNT(*) FROM inventory WHERE character_id={id};
        SELECT * FROM character_currency WHERE id={id};
        SELECT * FROM character_bind WHERE id={id} ORDER BY slot;
        SELECT * FROM character_spells WHERE id={id} ORDER BY slot_id;
        SELECT * FROM character_memmed_spells WHERE id={id} ORDER BY slot_id;
        SELECT * FROM character_buffs WHERE character_id={id} ORDER BY slot_id;
        SELECT COUNT(*) FROM character_corpses WHERE charid={id};"))
}
fn poll_selection(controller: &mut AccountController) -> anyhow::Result<Option<Ready>> {
    let ready = controller.poll();
    ensure!(
        controller.view.notice.is_none(),
        "selection error: {:?}",
        controller.view.notice
    );
    Ok(ready)
}
fn wait_stage(controller: &mut AccountController, stage: Stage) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        ensure!(
            poll_selection(controller)?.is_none(),
            "unexpected early live handoff"
        );
        if controller.view.stage == stage {
            return Ok(());
        }
        ensure!(
            Instant::now() < until,
            "stage timeout: expected {stage:?}, observed {:?}",
            controller.view.stage
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn pause(controller: &mut AccountController, stage: Stage, enabled: bool) -> anyhow::Result<u128> {
    if !enabled {
        return Ok(0);
    }
    let started = Instant::now();
    let token = controller.view.token;
    while started.elapsed() < IDLE {
        ensure!(
            poll_selection(controller)?.is_none(),
            "entered a character during selection idle"
        );
        ensure!(
            controller.view.stage == stage && controller.view.token == token,
            "selection state changed during idle"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(started.elapsed().as_millis())
}
fn flow(config: &ConnectionConfig, long_pauses: bool) -> anyhow::Result<()> {
    let endpoint = Endpoint {
        host: config.host.clone(),
        login_port: config.login_port,
        world_port: config.world_port,
    };
    let mut controller = AccountController::default();
    let initial_token = controller.view.token;
    controller.sign_in(
        endpoint.clone(),
        config.username.clone(),
        config.password.clone(),
    )?;
    ensure!(
        controller.view.stage == Stage::Authenticating,
        "sign-in did not enter pending state"
    );
    let signing_in = controller.view.token;
    ensure!(
        controller
            .sign_in(
                endpoint.clone(),
                config.username.clone(),
                config.password.clone()
            )
            .is_err(),
        "duplicate sign-in accepted"
    );
    ensure!(
        controller.view.token == signing_in,
        "duplicate sign-in replaced active attempt"
    );
    wait_stage(&mut controller, Stage::Worlds)?;
    println!(
        "WORLDS rows={} token={:?} pause_seconds={}",
        controller.view.servers.len(),
        controller.view.token,
        if long_pauses { 35 } else { 0 }
    );
    let worlds_token = controller.view.token;
    ensure!(
        !controller.action(initial_token, Action::RefreshWorlds),
        "stale attempt accepted"
    );
    ensure!(
        !controller.action(worlds_token, Action::ChooseWorld(0)),
        "unadvertised server accepted"
    );
    let worlds_idle = pause(&mut controller, Stage::Worlds, long_pauses)?;
    ensure!(
        controller.action(worlds_token, Action::RefreshWorlds),
        "server refresh refused"
    );
    ensure!(
        !controller.action(worlds_token, Action::RefreshWorlds),
        "duplicate refresh accepted"
    );
    wait_stage(&mut controller, Stage::Worlds)?;
    let refreshed = controller.view.token;
    ensure!(
        refreshed.attempt == worlds_token.attempt && refreshed.revision > worlds_token.revision,
        "refresh did not advance list revision"
    );
    let server = controller
        .view
        .servers
        .iter()
        .find(|s| config.server_id.map_or(s.is_up(), |id| s.server_id == id))
        .context("advertised world unavailable")?;
    let server_id = server.server_id;
    let world_name = server.name.clone();
    ensure!(
        !controller.action(worlds_token, Action::ChooseWorld(server_id)),
        "old list revision accepted"
    );
    ensure!(
        !controller.action(refreshed, Action::ChooseCharacter("Reviver".into())),
        "character selected from world-list stage"
    );
    ensure!(
        controller.action(refreshed, Action::ChooseWorld(server_id)),
        "advertised world refused"
    );
    ensure!(
        !controller.action(refreshed, Action::ChooseWorld(server_id)),
        "duplicate world entry accepted"
    );
    wait_stage(&mut controller, Stage::Characters)?;
    ensure!(
        controller.view.world_name == world_name,
        "selected world identity changed"
    );
    ensure!(
        controller
            .view
            .characters
            .iter()
            .any(|c| c.name == "Reviver" && c.enabled && c.zone == 77 && c.instance_id == 0),
        "missing/disabled/wrong-zone Reviver row"
    );
    ensure!(
        controller.view.selected_character.as_deref() == Some("Reviver"),
        "selected character identity mismatch"
    );
    let characters_token = controller.view.token;
    println!(
        "CHARACTERS rows={} selected=Reviver world_id={server_id} token={characters_token:?} pause_seconds={}",
        controller.view.characters.len(),
        if long_pauses { 35 } else { 0 }
    );
    ensure!(
        !controller.action(refreshed, Action::ChooseCharacter("Reviver".into())),
        "stale world revision selected a character"
    );
    ensure!(
        !controller.action(
            characters_token,
            Action::ChooseCharacter("NotThisFixture".into())
        ),
        "unlisted character accepted"
    );
    let characters_idle = pause(&mut controller, Stage::Characters, long_pauses)?;
    ensure!(
        controller.action(characters_token, Action::ChooseCharacter("Reviver".into())),
        "Reviver selection refused"
    );
    ensure!(
        !controller.action(characters_token, Action::ChooseCharacter("Reviver".into())),
        "duplicate character entry accepted"
    );
    ensure!(
        !controller.action(characters_token, Action::Back),
        "Back changed an active zone entry"
    );
    let until = Instant::now() + Duration::from_secs(45);
    let ready = loop {
        if let Some(ready) = poll_selection(&mut controller)? {
            break ready;
        }
        ensure!(Instant::now() < until, "controller Ready timeout");
        std::thread::sleep(Duration::from_millis(10));
    };
    ensure!(
        ready.identity.endpoint == endpoint
            && ready.identity.server_id == server_id
            && ready.identity.character == "Reviver",
        "incorrect authenticated handoff identity"
    );
    // Ready transfers lifetime to LiveWorld. Destroying the controller must not
    // destroy the runtime or cancel the zone socket retained by that live state.
    drop(controller);
    let mut live = ready.live;
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        live.poll();
        ensure!(live.error.is_none(), "live handoff error: {:?}", live.error);
        if live.ready && live.game.profile.is_some() && live.own_id.is_some() {
            break;
        }
        ensure!(Instant::now() < until, "LiveWorld profile/Ready timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    ensure!(
        live.game.profile.as_ref().unwrap().name == "Reviver"
            && live.game.inventory.items.is_empty(),
        "incorrect live profile/inventory"
    );
    ensure!(
        live.environment
            .as_ref()
            .is_some_and(|e| e.zone_id == 77 && e.instance_id == 0),
        "wrong live zone"
    );
    ensure!(
        live.own_id
            .and_then(|id| live.entities.get(&id))
            .is_some_and(|e| !e.spawn.npc && !e.spawn.is_corpse && e.spawn.name == "Reviver"),
        "wrong live player ownership"
    );
    let settled = Instant::now() + Duration::from_secs(2);
    while Instant::now() < settled {
        live.poll();
        ensure!(
            live.ready && live.error.is_none(),
            "dropping account controller killed live session"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "PASS worlds_idle_ms={worlds_idle} characters_idle_ms={characters_idle} refresh=true advertised_world={server_id} character=Reviver stale_double_rejected=true controller_ready=true live_profile_ready=true controller_drop_kept_runtime=true own_id={:?} movement_sent=false gameplay_sent=false",
        live.own_id
    );
    drop(live); // Existing NetworkIo channel closure sends normal zone logout.
    Ok(())
}
fn log_since(log: &Path, offset: usize) -> anyhow::Result<String> {
    let bytes = std::fs::read(log)?;
    Ok(String::from_utf8_lossy(bytes.get(offset..).context("log was truncated")?).into_owned())
}
fn cancel_after_entry(config: &ConnectionConfig, log: &Path) -> anyhow::Result<()> {
    let mut controller = AccountController::default();
    controller.sign_in(
        Endpoint {
            host: config.host.clone(),
            login_port: config.login_port,
            world_port: config.world_port,
        },
        config.username.clone(),
        config.password.clone(),
    )?;
    wait_stage(&mut controller, Stage::Worlds)?;
    let server_id = controller
        .view
        .servers
        .iter()
        .find(|s| config.server_id.map_or(s.is_up(), |id| s.server_id == id))
        .context("advertised cancellation world unavailable")?
        .server_id;
    ensure!(
        controller.action(controller.view.token, Action::ChooseWorld(server_id)),
        "cancellation world refused"
    );
    wait_stage(&mut controller, Stage::Characters)?;
    ensure!(
        controller
            .view
            .characters
            .iter()
            .any(|c| c.name == "Reviver" && c.enabled && c.zone == 77 && c.instance_id == 0),
        "missing/disabled/wrong-zone cancellation fixture"
    );
    let token = controller.view.token;
    let offset = std::fs::metadata(log)?.len() as usize;
    ensure!(
        controller.action(token, Action::ChooseCharacter("Reviver".into())),
        "cancellation character selection refused"
    );
    // The session diagnostic follows the world's EnterWorld reply. Waiting for
    // it proves this cancellation happens after entry committed on the server,
    // rather than merely dropping a queued foreground selection request. Do not
    // poll the controller here: a raced Ready must remain queued and go stale.
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        if log_since(log, offset)?.contains("entering zone") {
            break;
        }
        ensure!(Instant::now() < until, "post-EnterWorld barrier timeout");
        std::thread::sleep(Duration::from_millis(1));
    }
    controller.cancel();
    let cancelled = controller.view.token;
    ensure!(
        cancelled.attempt != token.attempt
            && controller.view.stage == Stage::Credentials
            && !controller.action(token, Action::ChooseCharacter("Reviver".into())),
        "cancel did not invalidate the entry"
    );
    // An early offline value alone is insufficient: the previous worker may
    // still be handshaking. Await its explicit cleanup completion first.
    let until = Instant::now() + Duration::from_secs(80);
    loop {
        ensure!(
            poll_selection(&mut controller)?.is_none()
                && controller.view.stage == Stage::Credentials
                && controller.view.token == cancelled,
            "cancelled entry published a stale live world"
        );
        if log_since(log, offset)?.contains("connection cleanup sent logout") {
            break;
        }
        ensure!(Instant::now() < until, "cancelled entry cleanup timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    offline()?;
    println!(
        "PASS cancel_after_enter_world=true stale_ready_rejected=true cleanup_completed=true offline=true movement_sent=false gameplay_sent=false"
    );
    Ok(())
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3,
        "usage: account_smoke RECOVERY_CONFIG NEW_LOG"
    );
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_recovery"
            && config.character == "Reviver",
        "requires disposable Reviver only"
    );
    offline()?;
    let id = format!("(SELECT id FROM character_data WHERE {FIXTURE})");
    let empty = sql(&format!(
        "SELECT COUNT(*) FROM inventory WHERE character_id={id}; SELECT COUNT(*) FROM character_buffs WHERE character_id={id}; SELECT COUNT(*) FROM character_corpses WHERE charid={id};"
    ))?;
    ensure!(
        empty.split_whitespace().collect::<Vec<_>>() == ["0", "0", "0"],
        "fixture must have no items/buffs/corpses"
    );
    let original = sql(&format!(
        "SELECT {FIELDS} FROM character_data WHERE {FIXTURE};"
    ))?;
    let values: Vec<f64> = original
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        values.len() == 11 && values.iter().all(|v| v.is_finite()),
        "invalid snapshot"
    );
    let before = invariant()?;
    let fields = FIELDS
        .split(',')
        .zip(values)
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    let restore = format!("UPDATE character_data SET {fields} WHERE {FIXTURE} AND ingame=0;");
    let log = PathBuf::from(&args[2]);
    let mut backup = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(log.with_extension("restore.sql"))?;
    backup.write_all(restore.as_bytes())?;
    backup.sync_all()?;
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&log)?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(
            "openeq_net::movement=debug,openeq_net::session=info,openeq_net::account=info,openeq::live=info",
        )
        .with_writer(file)
        .init();
    let result = flow(&config, true).and_then(|()| {
        offline()?;
        cancel_after_entry(&config, &log)
    });
    if let Err(error) = &result {
        eprintln!("PROBE_FAILED {error:#}");
    }
    if offline().is_err() {
        // Cleanup still uses the public interactive controller, never the
        // automatic ConnectionConfig::connect path this test must avoid.
        flow(&config, false)?;
        offline()?;
    }
    sql(&restore)?;
    ensure!(
        sql(&format!(
            "SELECT {FIELDS} FROM character_data WHERE {FIXTURE};"
        ))? == original,
        "pose/resources restoration failed"
    );
    ensure!(invariant()? == before, "fixture invariant changed");
    ensure!(
        !std::fs::read_to_string(&log)?.contains("sent player position"),
        "selection probe sent movement"
    );
    println!(
        "FIXTURE offline=true pose_resources=restored level_xp_stats_inventory_cash_binds_spells_buffs_corpses=unchanged log={}",
        log.display()
    );
    result
}
