//! Destructive only to the empty, disposable Reviver recovery fixture.
//! Exercises a real own death and forced same-zone authenticated bind re-entry.
use anyhow::{Context, ensure};
use openeq::{coordinates::scene_point_to_server, live::LiveWorld};
use openeq_net::{
    gameplay::{ChatChannel, Command},
    session::ConnectionConfig,
};
use openeq_render::Camera;
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command as Process, Stdio},
    time::{Duration, Instant},
};

const FIXTURE: &str =
    "name='Reviver' AND account_id=(SELECT id FROM account WHERE name='openeq_recovery')";

fn sql(query: &str) -> anyhow::Result<String> {
    let mut child = Process::new("ssh")
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
        "fixture SQL failed: {}",
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
    sql(&format!("SELECT level,exp,gm,race,class,`str`,sta,cha,dex,`int`,agi,wis FROM character_data WHERE {FIXTURE};
      SELECT COUNT(*) FROM inventory WHERE character_id=(SELECT id FROM character_data WHERE {FIXTURE});
      SELECT COALESCE(SUM(platinum+gold+silver+copper+platinum_bank+gold_bank+silver_bank+copper_bank),0) FROM character_currency WHERE id=(SELECT id FROM character_data WHERE {FIXTURE});
      SELECT slot,zone_id,instance_id,x,y,z,heading FROM character_bind WHERE id=(SELECT id FROM character_data WHERE {FIXTURE}) ORDER BY slot;"))
}

struct Probe {
    live: LiveWorld,
    camera: Camera,
    arrivals: u32,
}
impl Probe {
    fn new(config: ConnectionConfig) -> Self {
        Self {
            live: LiveWorld::start(config),
            camera: Camera::default(),
            arrivals: 0,
        }
    }
    fn poll(&mut self) -> anyhow::Result<()> {
        self.live.poll();
        ensure!(
            self.live.error.is_none(),
            "connection error: {:?}",
            self.live.error
        );
        if let Some(p) = self.live.initial_position.take() {
            self.camera.position = [p.x, p.y, p.z + 3.];
            self.camera.yaw = p.heading * std::f32::consts::TAU / 512.;
            self.arrivals += 1;
            println!(
                "ARRIVAL generation={} own={:?} scene=[{},{},{}]",
                self.live.zone_generation, self.live.own_id, p.x, p.y, p.z
            );
        }
        self.live.camera_position(&self.camera, false);
        Ok(())
    }
    fn wait(
        &mut self,
        what: &str,
        seconds: u64,
        done: impl Fn(&Self) -> bool,
    ) -> anyhow::Result<()> {
        let until = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.poll()?;
            if done(self) {
                return Ok(());
            }
            ensure!(
                Instant::now() < until,
                "timeout {what}; phase={:?} ready={} recent={:?}",
                self.live.game.recovery.phase(),
                self.live.ready,
                self.live
                    .game
                    .chat
                    .iter()
                    .rev()
                    .take(5)
                    .map(|line| &line.text)
                    .collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn pump(&mut self, seconds: f32) -> anyhow::Result<()> {
        let until = Instant::now() + Duration::from_secs_f32(seconds);
        while Instant::now() < until {
            self.poll()?;
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
    fn say(&mut self, text: String) -> anyhow::Result<()> {
        ensure!(
            self.live.command(Command::Chat {
                channel: ChatChannel::Say,
                target: String::new(),
                text,
                language: 0
            }),
            "fixture command refused"
        );
        Ok(())
    }
    fn ready(&self) -> bool {
        self.live.movement_allowed()
            && self
                .live
                .environment
                .as_ref()
                .is_some_and(|e| e.zone_id == 77 && e.instance_id == 0)
    }
    fn cleanup_corpses(&mut self) -> anyhow::Result<()> {
        self.wait("cleanup connection", 45, |p| p.ready())?;
        let ids: Vec<_> = self
            .live
            .entities
            .values()
            .filter(|e| e.spawn.is_corpse && e.spawn.name.starts_with("Reviver"))
            .map(|e| e.spawn.id)
            .collect();
        for id in ids {
            ensure!(
                self.live.entities[&id].spawn.is_corpse,
                "cleanup target ceased being a corpse"
            );
            self.live.set_target(Some(id));
            self.say("#corpse delete".into())?;
            self.wait("own disposable corpse deletion", 10, |p| {
                !p.live.entities.contains_key(&id)
            })?;
        }
        Ok(())
    }
}

fn proof(probe: &mut Probe, log: &Path) -> anyhow::Result<()> {
    probe.wait("initial zone", 45, |p| {
        p.ready() && p.live.game.profile.is_some()
    })?;
    ensure!(
        probe.live.game.profile.as_ref().unwrap().name == "Reviver",
        "wrong profile"
    );
    ensure!(
        probe.live.game.inventory.items.is_empty(),
        "fixture must have no items"
    );
    let old_id = probe.live.own_id.context("own player")?;
    let generation = probe.live.zone_generation;
    let arrivals = probe.arrivals;
    let death_xy = scene_point_to_server(probe.camera.position);
    probe.pump(0.3)?;
    probe.say(format!("#kill {old_id}"))?;
    probe.wait("own death freeze", 10, |p| !p.live.movement_allowed())?;
    ensure!(
        !probe.live.command(Command::AutoAttack(true)),
        "dead player accepted attack"
    );
    probe.wait("same-zone bind re-entry", 60, |p| {
        p.live.zone_generation == generation + 1 && p.ready() && p.arrivals > arrivals
    })?;
    let new_id = probe.live.own_id.context("revived player")?;
    ensure!(new_id != old_id, "corpse ID reused as new living player");
    let actual = scene_point_to_server([
        probe.camera.position[0],
        probe.camera.position[1],
        probe.camera.position[2] - 3.,
    ]);
    ensure!(
        (actual[0] - 160.).abs() < 0.2
            && (actual[1] + 1009.).abs() < 0.2
            && (actual[2] - 51.).abs() < 2.,
        "bind arrival wrong: {actual:?}"
    );
    probe.wait("old corpse in refreshed zone", 10, |p| {
        p.live
            .entities
            .get(&old_id)
            .is_some_and(|e| e.spawn.is_corpse && e.spawn.name.starts_with("Reviver"))
    })?;
    let corpse = sql(&format!(
        "SELECT x,y,z FROM character_corpses WHERE charid=(SELECT id FROM character_data WHERE {FIXTURE});"
    ))?;
    let fields: Vec<f32> = corpse
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        fields.len() == 3
            && (fields[0] - death_xy[0]).abs() < 0.2
            && (fields[1] - death_xy[1]).abs() < 0.2,
        "corpse moved or ambiguous: {fields:?}"
    );
    probe.camera.position[0] += 8.;
    probe.pump(0.5)?;
    probe.live.set_target(Some(new_id));
    probe.say("#save".into())?;
    probe.pump(0.5)?;
    let saved = sql(&format!(
        "SELECT x,y,z FROM character_data WHERE {FIXTURE};"
    ))?;
    let saved: Vec<f32> = saved
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        saved.len() == 3 && (saved[0] - 160.).abs() < 0.2 && (saved[1] + 1001.).abs() < 0.2,
        "post-recovery motion not saved: {saved:?}"
    );
    let trace = std::fs::read_to_string(log)?;
    ensure!(
        trace
            .lines()
            .any(|line| line.contains("received bind transfer") && line.contains("zone_id=0")),
        "no actual forced-zero bind packet recorded"
    );
    ensure!(
        trace
            .lines()
            .any(|line| line.contains("starting authenticated zone handoff")
                && line.contains("forced_reentry=true")
                && line.contains("zone_id=77")),
        "no authenticated same-zone re-entry recorded"
    );
    println!(
        "PASS death_freeze=true bind_zone_zero=true authenticated_same_zone_reentry=true old_id={old_id} new_id={new_id} corpse_retained=true movement_saved={saved:?}"
    );
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .context("usage: death_smoke CONFIG [LOG]")?;
    let log = PathBuf::from(
        std::env::args()
            .nth(2)
            .unwrap_or_else(|| "/tmp/openeq-death-live-protocol.log".into()),
    );
    let config = ConnectionConfig::load(Path::new(&config_path))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_recovery"
            && config.character == "Reviver",
        "requires only disposable Reviver fixture"
    );
    if std::env::args().nth(3).as_deref() == Some("--cleanup-only") {
        let mut cleanup = Probe::new(config);
        cleanup.cleanup_corpses()?;
        drop(cleanup);
        offline()?;
        println!("Disposable Reviver corpses removed; fixture offline.");
        return Ok(());
    }
    offline()?;
    ensure!(
        sql("SELECT rule_value FROM rule_values WHERE rule_name='Character:RespawnFromHover';")?
            .trim()
            == "false",
        "requires forced-bind server mode"
    );
    ensure!(sql(&format!("SELECT COUNT(*) FROM inventory WHERE character_id=(SELECT id FROM character_data WHERE {FIXTURE}); SELECT COUNT(*) FROM character_corpses WHERE charid=(SELECT id FROM character_data WHERE {FIXTURE});"))?.split_whitespace().all(|s|s=="0"),"fixture must have no items or corpses");
    let original = sql(&format!(
        "SELECT zone_id,zone_instance,x,y,z,heading,cur_hp,mana,endurance,hunger_level,thirst_level FROM character_data WHERE {FIXTURE};"
    ))?;
    let values: Vec<f64> = original
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        values.len() == 11 && values.iter().all(|v| v.is_finite()),
        "invalid fixture snapshot"
    );
    let before = invariant()?;
    let restore = format!(
        "UPDATE character_data SET zone_id={},zone_instance={},x={},y={},z={},heading={},cur_hp={},mana={},endurance={},hunger_level={},thirst_level={} WHERE {FIXTURE} AND ingame=0;",
        values[0],
        values[1],
        values[2],
        values[3],
        values[4],
        values[5],
        values[6],
        values[7],
        values[8],
        values[9],
        values[10]
    );
    let snapshot = log.with_extension("restore.sql");
    let mut backup = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&snapshot)?;
    backup.write_all(restore.as_bytes())?;
    backup.sync_all()?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(&log)?;
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter("openeq_net::recovery=info,openeq_net::zone=warn,openeq::live=info")
        .with_writer(file)
        .init();
    let mut probe = Probe::new(config.clone());
    let result = proof(&mut probe, &log);
    if let Err(error) = &result {
        eprintln!("PROOF_FAILED {error:#}");
    }
    let cleanup = probe.cleanup_corpses();
    drop(probe);
    let logged_out = offline();
    if cleanup.is_err() || logged_out.is_err() {
        let mut recovery = Probe::new(config);
        recovery.cleanup_corpses()?;
        drop(recovery);
        offline()?;
    }
    sql(&restore)?;
    ensure!(
        invariant()? == before,
        "fixture level/XP/stats/items/cash/binds changed"
    );
    ensure!(
        sql(&format!(
            "SELECT zone_id,zone_instance,x,y,z,heading,cur_hp,mana,endurance,hunger_level,thirst_level FROM character_data WHERE {FIXTURE};"
        ))? == original,
        "fixture pose/resources restoration failed"
    );
    ensure!(sql(&format!("SELECT COUNT(*) FROM character_corpses WHERE charid=(SELECT id FROM character_data WHERE {FIXTURE});"))?.trim()=="0","disposable corpse remains");
    println!(
        "FIXTURE offline=true original_pose_resources=restored level_xp_stats_inventory_cash_binds=unchanged corpses=0 log={}",
        log.display()
    );
    result
}
