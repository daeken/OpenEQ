//! Headless end-to-end login / NPC movement probe. Credentials stay in a file.
use openeq_net::{session::ConnectionConfig, zone::ZoneEvent};
use std::{collections::HashMap, path::PathBuf, time::Duration};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openeq_net=info".into()),
        )
        .init();
    let mut args = std::env::args().skip(1);
    let mut config = None;
    let mut seconds = 30u64;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--config" => config = args.next().map(PathBuf::from),
            "--seconds" => {
                seconds = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing duration"))?
                    .parse()?
            }
            _ => anyhow::bail!("usage: eqlogin --config FILE [--seconds 30]"),
        }
    }
    let config =
        ConnectionConfig::load(&config.ok_or_else(|| anyhow::anyhow!("--config is required"))?)?;
    let mut zone = config.connect().await?;
    let mut spawns = HashMap::new();
    let mut moves = 0u64;
    let mut ready = false;
    let mut own = None;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(2));
    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(deadline) => break,
            _ = heartbeat.tick() => if let Some((id, position)) = own { zone.send_position(id, position).await?; },
            event = zone.next_event() => match event? {
                ZoneEvent::Spawn(spawn) => {
                    if spawn.name.eq_ignore_ascii_case(&config.character) { own = Some((spawn.id, spawn.position)); }
                    if spawns.len() < 8 { println!("spawn {}: {} race={} at {:.1},{:.1},{:.1}", spawn.id, spawn.name, spawn.race, spawn.position.x, spawn.position.y, spawn.position.z); }
                    spawns.insert(spawn.id, spawn);
                }
                ZoneEvent::Movement { id, position } => {
                    if let Some(spawn) = spawns.get_mut(&id) {
                        if spawn.npc && (spawn.position.x != position.x || spawn.position.y != position.y || spawn.position.z != position.z) { moves += 1; }
                        spawn.position = position;
                    }
                }
                ZoneEvent::Despawn(id) => { spawns.remove(&id); }
                ZoneEvent::Environment(env) => println!("zone: {} ({}) fog={:?} range={:?}..{:?} sky={}", env.short_name, env.zone_id, env.fog_color[0], env.fog_start, env.fog_end, env.sky),
                ZoneEvent::Ready => { ready = true; println!("zone ready"); }
                ZoneEvent::Other { opcode, size } => tracing::debug!(opcode = format!("{opcode:#06x}"), size, "zone packet"),
                _ => {}
            }
        }
    }
    zone.logout().await?;
    let npcs = spawns.values().filter(|s| s.npc).count();
    println!(
        "RESULT ready={ready} entities={} npcs={npcs} npc_position_changes={moves}",
        spawns.len()
    );
    anyhow::ensure!(ready && npcs > 0, "zone entry / NPC streaming incomplete");
    Ok(())
}
