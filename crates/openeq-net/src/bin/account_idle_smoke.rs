//! Selection-screen idle audit, restricted to the disposable Reviver account.
//! No invented keepalive; records/restores character state if entry is needed.
use anyhow::{Context, ensure};
use openeq_net::{
    login::{LoginClient, LoginError, ServerEntry, Session},
    session::ConnectionConfig,
    stream::StreamError,
    world::{WorldClient, WorldError},
    zone::{ZoneClient, ZoneEvent},
};
use std::{
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    os::unix::fs::OpenOptionsExt,
    path::Path,
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
async fn offline() -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        if sql(&format!(
            "SELECT ingame FROM character_data WHERE {FIXTURE};"
        ))?
        .trim()
            == "0"
        {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Reviver remains online");
        tokio::time::sleep(Duration::from_millis(300)).await;
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
async fn authenticate(
    config: &ConnectionConfig,
) -> anyhow::Result<(LoginClient, Session, Vec<ServerEntry>)> {
    let address = tokio::net::lookup_host((config.host.as_str(), config.login_port))
        .await?
        .find(SocketAddr::is_ipv4)
        .context("IPv4 login endpoint")?;
    let mut login = LoginClient::connect(address).await?;
    let session = login.login(&config.username, &config.password).await?;
    let servers = login.server_list().await?;
    Ok((login, session, servers))
}
async fn observe_and_leave(mut zone: ZoneClient) -> anyhow::Result<()> {
    let result = tokio::time::timeout(Duration::from_secs(40), async {
        let (mut own, mut profile, mut ready) = (false, false, false);
        while !(own && profile && ready) {
            match zone.next_event().await? {
                ZoneEvent::Spawn(spawn)
                    if spawn.name == "Reviver" && !spawn.npc && !spawn.is_corpse =>
                {
                    own = true
                }
                ZoneEvent::Gameplay(openeq_net::gameplay::GameplayEvent::Profile(p)) => {
                    ensure!(p.name == "Reviver", "wrong profile");
                    profile = true;
                }
                ZoneEvent::Ready => ready = true,
                _ => {}
            }
        }
        println!(
            "ZONE ready=true own_profile=true movement_sent=false gameplay_actions_sent=false"
        );
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("zone readiness deadline")
    .and_then(|result| result);
    let logout = zone.logout().await;
    // Leave the reliable transport alive long enough to deliver logout.
    tokio::time::sleep(Duration::from_millis(500)).await;
    drop(zone);
    result?;
    logout?;
    Ok(())
}
async fn probe(config: &ConnectionConfig) -> anyhow::Result<()> {
    let (mut login, mut session, mut servers) = authenticate(config).await?;
    println!(
        "SERVER_LIST rows={} idle_seconds={}",
        servers.len(),
        IDLE.as_secs()
    );
    let started = Instant::now();
    tokio::time::sleep(IDLE).await;
    match login.server_list().await {
        Ok(refreshed) => {
            servers = refreshed;
            println!(
                "SERVER_LIST_IDLE elapsed_ms={} refresh=accepted rows={}",
                started.elapsed().as_millis(),
                servers.len()
            );
        }
        Err(error @ (LoginError::Closed | LoginError::Stream(StreamError::Closed))) => {
            println!(
                "SERVER_LIST_IDLE elapsed_ms={} refresh=closed error={error}",
                started.elapsed().as_millis()
            );
            drop(login);
            (login, session, servers) = authenticate(config).await?;
            println!(
                "SERVER_LIST_RETRY fresh_authentication=true rows={}",
                servers.len()
            );
        }
        Err(error) => return Err(error.into()),
    }
    let server = servers
        .iter()
        .find(|s| config.server_id.map_or(s.is_up(), |id| s.server_id == id))
        .context("configured world")?;
    login.play(server.server_id).await?;
    let mut world = WorldClient::connect(
        SocketAddr::new(server.address, config.world_port),
        session.account_id,
        &session.key,
    )
    .await?;
    let characters = world.characters().await?;
    let character = characters
        .iter()
        .find(|c| c.name == "Reviver")
        .context("Reviver roster entry")?;
    ensure!(character.enabled, "Reviver disabled");
    println!(
        "CHARACTER_LIST rows={} selected=Reviver enabled={} zone={} instance={} idle_seconds={}",
        characters.len(),
        character.enabled,
        character.zone,
        character.instance_id,
        IDLE.as_secs()
    );
    let started = Instant::now();
    tokio::time::sleep(IDLE).await;
    let entered = match world.enter_world("Reviver").await {
        Ok(address) => {
            println!(
                "CHARACTER_LIST_IDLE elapsed_ms={} enter=accepted",
                started.elapsed().as_millis()
            );
            observe_and_leave(ZoneClient::connect(address, "Reviver").await?).await?;
            true
        }
        Err(error @ (WorldError::Closed | WorldError::Stream(StreamError::Closed))) => {
            println!(
                "CHARACTER_LIST_IDLE elapsed_ms={} enter=closed error={error}",
                started.elapsed().as_millis()
            );
            false
        }
        Err(error) => return Err(error.into()),
    };
    drop(world);
    drop(login);
    if !entered {
        // Verify retry through the unchanged automatic config path, using only
        // this existing disposable character, then immediately leave cleanly.
        observe_and_leave(config.connect().await?).await?;
        println!("AUTOMATIC_RETRY ready=true");
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3,
        "usage: account_idle_smoke RECOVERY_CONFIG NEW_SNAPSHOT_PATH"
    );
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_recovery"
            && config.character == "Reviver",
        "requires disposable recovery account only"
    );
    offline().await?;
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
    let assignments = FIELDS
        .split(',')
        .zip(values)
        .map(|(field, value)| format!("{field}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    let restore = format!("UPDATE character_data SET {assignments} WHERE {FIXTURE} AND ingame=0;");
    let mut snapshot = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&args[2])?;
    snapshot.write_all(restore.as_bytes())?;
    snapshot.sync_all()?;
    let result = probe(&config).await;
    if let Err(error) = &result {
        eprintln!("PROBE_FAILED {error:#}");
    }
    if offline().await.is_err() {
        // Never repair an online row. A failed connection may need one clean
        // dedicated reconnect/logout before restoration can be applied.
        observe_and_leave(config.connect().await?).await?;
        offline().await?;
    }
    sql(&restore)?;
    ensure!(
        sql(&format!(
            "SELECT {FIELDS} FROM character_data WHERE {FIXTURE};"
        ))? == original,
        "pose/resources restoration failed"
    );
    ensure!(invariant()? == before, "fixture invariant changed");
    println!(
        "FIXTURE offline=true pose_resources=restored inventory_cash_binds_spells_buffs_corpses=unchanged"
    );
    result
}
