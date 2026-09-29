//! Live offer → decline → fresh offer → accept, using only disposable fixtures.
use anyhow::{Context, ensure};
use openeq::{
    coordinates::scene_point_to_server,
    death::{RecoveryAction, RecoveryIntent},
    live::LiveWorld,
};
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

const NAMES: [&str; 2] = ["Reviver", "Rezzer"];
const ACCOUNTS: [&str; 2] = ["openeq_recovery", "openeq_resurrector"];
const POSE: &str =
    "zone_id,zone_instance,x,y,z,heading,cur_hp,mana,endurance,hunger_level,thirst_level";
const REZ_SPELL: u32 = 392;
const SICKNESS: u32 = 756;

fn fixture(i: usize) -> String {
    format!(
        "name='{}' AND account_id=(SELECT id FROM account WHERE name='{}')",
        NAMES[i], ACCOUNTS[i]
    )
}
fn char_id(i: usize) -> String {
    format!("(SELECT id FROM character_data WHERE {})", fixture(i))
}
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
        let state = sql(&format!(
            "SELECT ingame FROM character_data WHERE ({}) OR ({});",
            fixture(0),
            fixture(1)
        ))?;
        if state.split_whitespace().collect::<Vec<_>>() == ["0", "0"] {
            return Ok(());
        }
        ensure!(
            Instant::now() < until,
            "recovery fixtures remain online: {state}"
        );
        std::thread::sleep(Duration::from_millis(300));
    }
}
fn invariant(i: usize) -> anyhow::Result<String> {
    let id = char_id(i);
    sql(&format!("SELECT level,level2,exp,gm,race,class,`str`,sta,cha,dex,`int`,agi,wis FROM character_data WHERE {};
        SELECT COUNT(*) FROM inventory WHERE character_id={id};
        SELECT * FROM character_currency WHERE id={id};
        SELECT * FROM character_bind WHERE id={id} ORDER BY slot;
        SELECT * FROM character_spells WHERE id={id} ORDER BY slot_id;
        SELECT * FROM character_memmed_spells WHERE id={id} ORDER BY slot_id;
        SELECT * FROM character_buffs WHERE character_id={id} ORDER BY slot_id;", fixture(i)))
}
fn corpse() -> anyhow::Result<Vec<f32>> {
    sql(&format!(
        "SELECT id,is_rezzed,rezzable,x,y,z,exp FROM character_corpses WHERE charid={};",
        char_id(0)
    ))?
    .split_whitespace()
    .map(|v| v.parse().context("corpse number"))
    .collect()
}
fn snapshot() -> anyhow::Result<([String; 2], [String; 2], String)> {
    let mut poses = [String::new(), String::new()];
    let mut invariants = [String::new(), String::new()];
    let mut restore = String::new();
    for i in 0..2 {
        let id = char_id(i);
        let identity = sql(&format!(
            "SELECT level,exp,gm FROM character_data WHERE {};",
            fixture(i)
        ))?;
        let expected = if i == 0 { "1\t0\t0" } else { "50\t0\t0" };
        ensure!(
            identity.trim() == expected,
            "{} level/XP/GM fixture changed",
            NAMES[i]
        );
        let counts = sql(&format!(
            "SELECT COUNT(*) FROM inventory WHERE character_id={id};
            SELECT COUNT(*) FROM character_corpses WHERE charid={id};
            SELECT COUNT(*) FROM character_buffs WHERE character_id={id};
            SELECT COALESCE(SUM(platinum+gold+silver+copper+platinum_bank+gold_bank+silver_bank+copper_bank),0) FROM character_currency WHERE id={id};"
        ))?;
        ensure!(
            counts.split_whitespace().collect::<Vec<_>>() == ["0", "0", "0", "0"],
            "{} must have no items, corpses, buffs or cash",
            NAMES[i]
        );
        poses[i] = sql(&format!(
            "SELECT {POSE} FROM character_data WHERE {};",
            fixture(i)
        ))?;
        let values: Vec<f64> = poses[i]
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<_, _>>()?;
        ensure!(
            values.len() == 11 && values.iter().all(|v| v.is_finite()),
            "invalid fixture snapshot"
        );
        let fields = POSE
            .split(',')
            .zip(values)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(",");
        restore.push_str(&format!(
            "UPDATE character_data SET {fields} WHERE {} AND ingame=0;\n",
            fixture(i)
        ));
        invariants[i] = invariant(i)?;
    }
    Ok((poses, invariants, restore))
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
                "ARRIVAL generation={} own={:?} server={:?}",
                self.live.zone_generation,
                self.live.own_id,
                self.position()
            );
        }
        self.live.camera_position(&self.camera, false);
        Ok(())
    }
    fn position(&self) -> [f32; 3] {
        scene_point_to_server([
            self.camera.position[0],
            self.camera.position[1],
            self.camera.position[2] - 3.,
        ])
    }
    fn ready(&self) -> bool {
        self.live.movement_allowed()
            && self
                .live
                .environment
                .as_ref()
                .is_some_and(|e| e.zone_id == 77 && e.instance_id == 0)
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
    fn resources(&self) -> [Option<u32>; 3] {
        [
            self.live.game.hp.current,
            self.live.game.mana.current,
            self.live.game.endurance.current,
        ]
    }
}
struct Pair {
    clients: [Probe; 2],
}
impl Pair {
    fn new(configs: &[ConnectionConfig; 2]) -> Self {
        Self {
            clients: [
                Probe::new(configs[0].clone()),
                Probe::new(configs[1].clone()),
            ],
        }
    }
    fn poll(&mut self) -> anyhow::Result<()> {
        for p in &mut self.clients {
            p.poll()?;
        }
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
                "timeout {what}; phases={:?}; recent={:?}",
                self.clients
                    .each_ref()
                    .map(|p| p.live.game.recovery.phase()),
                self.clients.each_ref().map(|p| p
                    .live
                    .game
                    .chat
                    .iter()
                    .rev()
                    .take(6)
                    .map(|c| &c.text)
                    .collect::<Vec<_>>())
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
    fn save(&mut self, i: usize) -> anyhow::Result<()> {
        // Same-zone ZoneSolicited sends a destination but leaves the server's
        // current pose to the next normal client movement packet. Let its
        // heartbeat arrive before the GM save command snapshots that pose.
        self.pump(0.3)?;
        self.clients[i].live.set_target(self.clients[i].live.own_id);
        self.clients[i].say("#save".into())?;
        self.pump(0.5)
    }
    fn offer(&mut self, corpse_id: u32) -> anyhow::Result<()> {
        ensure!(
            self.clients[1]
                .live
                .entities
                .get(&corpse_id)
                .is_some_and(|e| e.spawn.is_corpse && e.spawn.name.starts_with("Reviver")),
            "caster target must be Reviver corpse"
        );
        self.clients[1].live.set_target(Some(corpse_id));
        self.clients[1].say(format!("#castspell {REZ_SPELL}"))?;
        self.wait("real resurrection offer", 15, |p| {
            p.clients[0]
                .live
                .game
                .recovery
                .view(Instant::now())
                .resurrection
                .is_some()
        })
    }
    fn cleanup(&mut self) -> anyhow::Result<()> {
        self.wait("cleanup connections", 60, |p| {
            p.clients.iter().all(Probe::ready)
        })?;
        // Baseline was empty, and this smoke creates only this detrimental buff.
        ensure!(
            self.clients[0]
                .live
                .game
                .buffs
                .values()
                .all(|b| b.spell_id == SICKNESS),
            "unexpected buff; refusing broad fade"
        );
        if !self.clients[0].live.game.buffs.is_empty() {
            self.clients[0].live.set_target(self.clients[0].live.own_id);
            self.clients[0].say("#nukebuffs detrimental".into())?;
            self.wait("resurrection sickness fade", 10, |p| {
                p.clients[0].live.game.buffs.is_empty()
            })?;
        }
        let ids: Vec<_> = self.clients[0]
            .live
            .entities
            .values()
            .filter(|e| e.spawn.is_corpse && e.spawn.name.starts_with("Reviver"))
            .map(|e| e.spawn.id)
            .collect();
        for id in ids {
            self.clients[0].live.set_target(Some(id));
            self.clients[0].say("#corpse delete".into())?;
            self.wait("disposable corpse deletion", 10, |p| {
                !p.clients[0].live.entities.contains_key(&id)
            })?;
        }
        self.save(0)?;
        self.save(1)
    }
}

fn proof(pair: &mut Pair, log: &Path) -> anyhow::Result<()> {
    pair.wait("both profiles", 60, |p| {
        p.clients
            .iter()
            .all(|p| p.ready() && p.live.game.profile.is_some())
    })?;
    for (i, p) in pair.clients.iter().enumerate() {
        ensure!(
            p.live.game.profile.as_ref().unwrap().name == NAMES[i]
                && p.live.game.inventory.items.is_empty()
                && p.live.game.buffs.is_empty(),
            "wrong or nonempty fixture"
        );
    }
    let old_id = pair.clients[0].live.own_id.context("living Reviver")?;
    let generation = pair.clients[0].live.zone_generation;
    pair.clients[0].say(format!("#kill {old_id}"))?;
    pair.wait("death", 10, |p| !p.clients[0].live.movement_allowed())?;
    pair.wait("living bind return", 60, |p| {
        p.clients[0].live.zone_generation == generation + 1 && p.clients[0].ready()
    })?;
    pair.wait("corpse visible to caster", 10, |p| {
        p.clients[1]
            .live
            .entities
            .get(&old_id)
            .is_some_and(|e| e.spawn.is_corpse)
    })?;
    let new_id = pair.clients[0].live.own_id.context("returned Reviver")?;
    ensure!(new_id != old_id, "old corpse identity reused");
    let initial_corpse = corpse()?;
    ensure!(
        initial_corpse.len() == 7 && initial_corpse[1] == 0. && initial_corpse[2] == 1.,
        "corpse not rezzable: {initial_corpse:?}"
    );
    let bind = pair.clients[0].position();
    ensure!((bind[0] - 160.).abs() < 0.2, "wrong bind arrival: {bind:?}");
    pair.offer(old_id)?;
    let first = pair.clients[0].live.game.recovery.view(Instant::now());
    let offer = first.resurrection.context("first offer")?;
    ensure!(
        offer.caster == "Rezzer"
            && offer.spell_id == REZ_SPELL
            && offer.zone_id == 77
            && !offer.hovering
            && offer.can_accept
            && offer.can_decline,
        "wrong offer: {offer:?}"
    );
    let arrivals = pair.clients[0].arrivals;
    let old_accept = RecoveryAction {
        token: first.token,
        intent: RecoveryIntent::AcceptResurrection,
    };
    let decline = RecoveryAction {
        token: first.token,
        intent: RecoveryIntent::DeclineResurrection,
    };
    ensure!(
        pair.clients[0].live.recovery_action(decline),
        "decline rejected"
    );
    ensure!(
        !pair.clients[0].live.recovery_action(decline),
        "duplicate decline allowed"
    );
    pair.wait("decline sent", 10, |p| {
        p.clients[0]
            .live
            .game
            .recovery
            .view(Instant::now())
            .resurrection
            .is_none()
    })?;
    pair.pump(0.5)?;
    ensure!(
        pair.clients[0].arrivals == arrivals
            && pair.clients[0].position() == bind
            && pair.clients[0].ready(),
        "decline moved or froze player"
    );
    ensure!(
        corpse()?.get(1) == Some(&0.),
        "decline removed or marked corpse resurrected"
    );
    pair.offer(old_id)?;
    let second = pair.clients[0].live.game.recovery.view(Instant::now());
    ensure!(
        second.token != first.token,
        "fresh offer reused stale token"
    );
    ensure!(
        !pair.clients[0].live.recovery_action(old_accept),
        "stale acceptance allowed"
    );
    let accept = RecoveryAction {
        token: second.token,
        intent: RecoveryIntent::AcceptResurrection,
    };
    let before = pair.clients[0].resources();
    ensure!(
        pair.clients[0].live.recovery_action(accept),
        "accept rejected"
    );
    ensure!(
        !pair.clients[0].live.movement_allowed(),
        "accept did not await authoritative relocation"
    );
    ensure!(
        pair.clients[0].resources() == before && pair.clients[0].position() == bind,
        "accept fabricated resources or relocation"
    );
    ensure!(
        !pair.clients[0].live.recovery_action(accept),
        "duplicate acceptance allowed"
    );
    pair.wait("authoritative resurrection relocation", 20, |p| {
        p.clients[0].ready() && p.clients[0].arrivals > arrivals
    })?;
    pair.wait("resurrection sickness", 10, |p| {
        p.clients[0]
            .live
            .game
            .buffs
            .values()
            .any(|b| b.spell_id == SICKNESS)
    })?;
    let actual = pair.clients[0].position();
    ensure!(
        pair.clients[0].live.zone_generation == generation + 1
            && pair.clients[0].live.own_id == Some(new_id),
        "living same-zone resurrection replaced zone/entity generation"
    );
    ensure!(
        (actual[0] - initial_corpse[3]).abs() < 0.2 && (actual[1] - initial_corpse[4]).abs() < 0.2,
        "wrong resurrection destination {actual:?}, corpse {initial_corpse:?}"
    );
    ensure!(
        !pair.clients[0].live.recovery_action(accept),
        "completed acceptance replay allowed"
    );
    pair.save(0)?;
    let saved = sql(&format!(
        "SELECT cur_hp,mana,endurance,x,y,z FROM character_data WHERE {};",
        fixture(0)
    ))?;
    let saved: Vec<f32> = saved
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        saved.len() == 6
            && saved[0] > 0.
            && saved[1] == 0.
            && (saved[3] - actual[0]).abs() < 0.2
            && (saved[4] - actual[1]).abs() < 0.2,
        "server did not save resurrection state: {saved:?}"
    );
    // HP updates may await the next server tick; never derive them from acceptance.
    pair.wait("server reduced HP update", 10, |p| {
        p.clients[0]
            .live
            .game
            .hp
            .current
            .is_some_and(|hp| hp > 0 && hp < before[0].unwrap_or(u32::MAX))
    })?;
    let after = pair.clients[0].resources();
    let rezzed = corpse()?;
    ensure!(
        rezzed.len() == 7 && rezzed[0] == initial_corpse[0] && rezzed[1] == 1. && rezzed[6] == 0.,
        "server corpse state incorrect: {rezzed:?}"
    );
    ensure!(
        pair.clients[0]
            .live
            .entities
            .get(&old_id)
            .is_some_and(|e| e.spawn.is_corpse),
        "resurrection deleted corpse locally"
    );
    pair.clients[0].camera.position[0] += 8.;
    pair.pump(0.5)?;
    pair.save(0)?;
    let moved = sql(&format!(
        "SELECT x,y FROM character_data WHERE {};",
        fixture(0)
    ))?;
    let moved: Vec<f32> = moved
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        moved.len() == 2
            && (moved[0] - actual[0]).abs() < 0.2
            && (moved[1] - actual[1] - 8.).abs() < 0.2,
        "movement not restored: {moved:?}"
    );
    let trace = std::fs::read_to_string(log)?;
    ensure!(
        trace
            .lines()
            .filter(|line| line.contains("received resurrection offer"))
            .count()
            == 2,
        "expected exactly two real incoming offers"
    );
    let answers: Vec<_> = trace
        .lines()
        .filter(|line| line.contains("sent resurrection answer"))
        .collect();
    ensure!(
        answers.len() == 2
            && answers[0].contains("accept=false")
            && answers[1].contains("accept=true"),
        "expected one decline then one acceptance, with no stale/duplicate sends"
    );
    println!(
        "PASS decline=true fresh_offer=true stale_duplicate_rejected=true server_relocation={actual:?} same_generation=true resources_before={before:?} resources_after={after:?} saved_hp_mana_end_xyz={saved:?} corpse_rezzed=true sickness={SICKNESS} movement_saved={moved:?}"
    );
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: resurrection_smoke REVIVER_CONFIG REZZER_CONFIG NEW_LOG"
    );
    let configs = [
        ConnectionConfig::load(Path::new(&args[1]))?,
        ConnectionConfig::load(Path::new(&args[2]))?,
    ];
    for (i, c) in configs.iter().enumerate() {
        ensure!(
            c.host == "storage2.daeken.dev" && c.username == ACCOUNTS[i] && c.character == NAMES[i],
            "requires separate disposable recovery fixtures"
        );
    }
    offline()?;
    ensure!(
        sql("SELECT rule_value FROM rule_values WHERE rule_name='Character:RespawnFromHover';")?
            .trim()
            == "false",
        "requires forced-bind server mode"
    );
    ensure!(sql("SELECT rule_value FROM rule_values WHERE rule_name='Character:UseResurrectionSickness'; SELECT rule_value FROM rule_values WHERE rule_name='Character:ResurrectionSicknessSpellID';")?.split_whitespace().collect::<Vec<_>>() == ["true", "756"],
        "requires source-backed standard resurrection sickness rules");
    let (poses, invariants, restore) = snapshot()?;
    let log = PathBuf::from(&args[3]);
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
        .with_env_filter("openeq_net::recovery=info,openeq_net::zone=warn,openeq::live=info")
        .with_writer(file)
        .init();
    let mut pair = Pair::new(&configs);
    let result = proof(&mut pair, &log);
    if let Err(error) = &result {
        eprintln!("PROOF_FAILED {error:#}");
    }
    let cleanup = pair.cleanup();
    drop(pair);
    let logged_out = offline();
    if cleanup.is_err() || logged_out.is_err() {
        eprintln!(
            "Reconnecting disposable fixtures for cleanup: cleanup={cleanup:?} logout={logged_out:?}"
        );
        let mut recovery = Pair::new(&configs);
        recovery.cleanup()?;
        drop(recovery);
        offline()?;
    }
    sql(&restore)?;
    for i in 0..2 {
        ensure!(
            invariant(i)? == invariants[i],
            "{} invariant changed",
            NAMES[i]
        );
        ensure!(
            sql(&format!(
                "SELECT {POSE} FROM character_data WHERE {};",
                fixture(i)
            ))? == poses[i],
            "{} pose/resources restoration failed",
            NAMES[i]
        );
        ensure!(
            sql(&format!(
                "SELECT COUNT(*) FROM character_corpses WHERE charid={};",
                char_id(i)
            ))?
            .trim()
                == "0",
            "{} corpse remains",
            NAMES[i]
        );
    }
    println!(
        "FIXTURES offline=true pose_resources=restored level_xp_stats_items_cash_binds_spells_buffs=unchanged corpses=0 log={}",
        log.display()
    );
    result
}
