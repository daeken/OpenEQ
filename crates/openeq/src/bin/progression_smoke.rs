//! Guarded Fellowship progression receive proof: one normal skill and one
//! language change, restored by commands and checked offline. Its inherited
//! level10/XP0 baseline makes SetEXP/SetLevel unsafe for this narrow fixture.
//! No trainer, XP, level, movement, combat or other-character action is sent.
use anyhow::{Context, ensure};
use openeq::live::LiveWorld;
use openeq_net::{
    gameplay::{ChatChannel, Command},
    session::ConnectionConfig,
};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, Stdio},
    time::{Duration, Instant},
};

const FIXTURES: [(&str, &str); 1] = [("Fellowship", "openeq_social1")];
const POSE: &str =
    "zone_id,zone_instance,x,y,z,heading,cur_hp,mana,endurance,hunger_level,thirst_level";

fn remote(command: &str, input: &str) -> anyhow::Result<String> {
    let mut child = ProcessCommand::new("ssh")
        .args(["-o", "BatchMode=yes", "storage2.daeken.dev", command])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("remote input")?
        .write_all(input.as_bytes())?;
    let out = child.wait_with_output()?;
    ensure!(
        out.status.success(),
        "remote fixture operation failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?)
}
fn sql(query: &str) -> anyhow::Result<String> {
    remote("sudo mariadb peq -N -B", query)
}
fn fixture(i: usize) -> String {
    let (name, account) = FIXTURES[i];
    format!("name='{name}' AND account_id=(SELECT id FROM account WHERE name='{account}')")
}
fn id(i: usize) -> String {
    format!("(SELECT id FROM character_data WHERE {})", fixture(i))
}
fn private_file(path: &Path) -> anyhow::Result<File> {
    Ok(OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?)
}
fn offline(i: usize) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        if sql(&format!(
            "SELECT ingame FROM character_data WHERE {};",
            fixture(i)
        ))?
        .trim()
            == "0"
        {
            return Ok(());
        }
        ensure!(Instant::now() < until, "{} remains online", FIXTURES[i].0);
        std::thread::sleep(Duration::from_millis(250));
    }
}
fn invariants(i: usize) -> anyhow::Result<String> {
    let id = id(i);
    let name = FIXTURES[i].0;
    sql(&format!("SELECT id,account_id,name,level,level2,exp,points,aa_points,aa_exp,aa_points_spent,aa_points_spent_old,aa_points_old,gm,race,class,`str`,sta,cha,dex,`int`,agi,wis FROM character_data WHERE {};
        SELECT character_id,slot_id,item_id,charges,color,augment_one,augment_two,augment_three,augment_four,augment_five,augment_six,instnodrop,custom_data,ornament_icon,ornament_idfile,ornament_hero_model FROM inventory WHERE character_id={id} ORDER BY slot_id;
        SELECT * FROM character_currency WHERE id={id};
        SELECT * FROM character_skills WHERE id={id} ORDER BY skill_id;
        SELECT * FROM character_languages WHERE id={id} ORDER BY lang_id;
        SELECT * FROM character_alternate_abilities WHERE id={id} ORDER BY aa_id;
        SELECT * FROM character_bind WHERE id={id} ORDER BY slot;
        SELECT * FROM character_spells WHERE id={id} ORDER BY slot_id;
        SELECT * FROM character_memmed_spells WHERE id={id} ORDER BY slot_id;
        SELECT * FROM character_buffs WHERE character_id={id} ORDER BY slot_id;
        SELECT COUNT(*) FROM character_corpses WHERE charid={id};
        SELECT * FROM group_id WHERE character_id={id} OR name='{name}';
        SELECT gid,leadername,marknpc,HEX(leadershipaa),maintank,assist,puller,mentoree,mentor_percent FROM group_leaders WHERE leadername='{name}';
        SELECT * FROM guild_members WHERE char_id={id};
        SELECT * FROM raid_members WHERE charid={id} OR name='{name}';", fixture(i)))
}
struct Snapshot {
    pose: String,
    invariant: String,
    restore: String,
}
fn snapshot(i: usize) -> anyhow::Result<Snapshot> {
    offline(i)?;
    let name = FIXTURES[i].0;
    let id = id(i);
    let counts = sql(&format!(
        "SELECT COUNT(*) FROM character_data WHERE {};
        SELECT COUNT(*) FROM group_id WHERE character_id={id} OR name='{name}';
        SELECT COUNT(*) FROM raid_members WHERE charid={id} OR name='{name}';
        SELECT COUNT(*) FROM guild_members WHERE char_id={id};
        SELECT COUNT(*) FROM character_buffs WHERE character_id={id};
        SELECT COUNT(*) FROM character_corpses WHERE charid={id};",
        fixture(i)
    ))?;
    ensure!(
        counts.split_whitespace().collect::<Vec<_>>() == ["1", "0", "0", "0", "0", "0"],
        "{name} must be ungrouped, unraided, unguilded, unbuffed and corpse-free"
    );
    let pose = sql(&format!(
        "SELECT {POSE} FROM character_data WHERE {};",
        fixture(i)
    ))?;
    let numbers: Vec<f64> = pose
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    ensure!(
        numbers.len() == 11 && numbers.iter().all(|v| v.is_finite()) && numbers[..2] == [202., 0.],
        "invalid/non-PoK fixture snapshot"
    );
    let set = POSE
        .split(',')
        .zip(numbers)
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(",");
    Ok(Snapshot {
        pose,
        invariant: invariants(i)?,
        restore: format!(
            "UPDATE character_data SET {set} WHERE {} AND ingame=0 AND level=10 AND level2=10 AND exp=0 AND points=0 AND aa_exp=0 AND aa_points=0 AND aa_points_spent=0;\n",
            fixture(i)
        ),
    })
}

#[derive(Default)]
struct Probe {
    live: Option<LiveWorld>,
    seen: u64,
}
impl Probe {
    fn live(&self) -> &LiveWorld {
        self.live.as_ref().expect("connected fixture")
    }
    fn poll(&mut self) -> anyhow::Result<()> {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        live.poll();
        ensure!(
            live.error.is_none(),
            "fixture network error: {:?}",
            live.error
        );
        ensure!(live.target.is_none(), "fixture unexpectedly has a target");
        let state = &live.game.progression;
        if self.seen != state.revision {
            tracing::info!(target: "progression_smoke", revision=state.revision,
                confirmed=state.confirmed, level=?state.level,
                experience_bar_units=?state.experience_bar_units,
                experience_fraction=?state.experience_fraction(),
                training_points_snapshot=?state.profile_training_points,
                experience_total_snapshot=?state.profile_experience_total,
                skills=?state.skills, languages=?state.languages,
                "foreground progression state");
            self.seen = state.revision;
        }
        Ok(())
    }
    fn wait(&mut self, label: &str, predicate: impl Fn(&LiveWorld) -> bool) -> anyhow::Result<()> {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            self.poll()?;
            if predicate(self.live()) {
                return Ok(());
            }
            ensure!(Instant::now() < until, "timeout: {label}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn connect(&mut self, config: &ConnectionConfig) -> anyhow::Result<()> {
        self.live = Some(LiveWorld::start(config.clone()));
        self.seen = 0;
        self.wait("profile and zone Ready", |l| {
            l.ready
                && l.own_id.is_some()
                && l.game.profile.is_some()
                && l.game.progression.confirmed
        })?;
        let live = self.live();
        let profile = live.game.profile.as_ref().context("profile")?;
        ensure!(
            profile.name == "Fellowship"
                && profile.level == 10
                && profile.race == 1
                && profile.class == 1
                && profile.training_points == 0
                && profile.experience_total == 0,
            "fixture profile identity/progression mismatch"
        );
        ensure!(
            live.environment
                .as_ref()
                .is_some_and(|e| e.zone_id == 202 && e.instance_id == 0),
            "wrong fixture zone"
        );
        ensure!(
            profile.skills.len() == 100 && profile.languages.len() == 32,
            "unexpected RoF2 profile counts"
        );
        for (id, value) in profile.skills.iter().enumerate() {
            ensure!(
                *value == if matches!(id, 0 | 1 | 28) { 55 } else { 0 },
                "unexpected profile skill {id}"
            );
        }
        for (id, value) in profile.languages.iter().enumerate() {
            ensure!(
                *value == if id == 0 { 100 } else { 0 },
                "unexpected profile language {id}"
            );
        }
        tracing::info!(target: "progression_smoke", level=profile.level,
            training_points=profile.training_points, experience_total=profile.experience_total,
            skills=?profile.skills, languages=?profile.languages, "actual profile progression snapshot");
        self.wait("server initial ExpUpdate", |l| {
            l.game.progression.experience_bar_units.is_some()
        })?;
        let progression = &self.live().game.progression;
        let units = progression.experience_bar_units.context("initial XP bar")?;
        if units > 330 {
            ensure!(
                progression.experience_fraction().is_none(),
                "invalid XP ratio became a percentage"
            );
        } else {
            ensure!(
                progression.experience_fraction() == Some(units as f32 / 330.),
                "valid received XP ratio lost"
            );
        }
        self.values(55, 100)?;
        tracing::info!(target: "progression_smoke", units,
            valid=units<=330, "received initial XP without changing inconsistent level10 XP0 baseline");
        Ok(())
    }
    fn values(&self, skill: u32, language: u32) -> anyhow::Result<()> {
        let state = &self.live().game.progression;
        ensure!(
            state.confirmed
                && state.level == Some(10)
                && state.profile_training_points == Some(0)
                && state.profile_experience_total == Some(0),
            "progression identity or profile snapshots changed"
        );
        for (id, value) in state.skills.iter().enumerate() {
            let expected = match id {
                0 => skill,
                1 | 28 => 55,
                _ => 0,
            };
            ensure!(
                *value == Some(expected),
                "unexpected foreground skill {id}: {value:?}"
            );
        }
        for (id, value) in state.languages.iter().enumerate() {
            ensure!(
                *value == Some(if id == 0 { language } else { 0 }),
                "unexpected foreground language {id}: {value:?}"
            );
        }
        Ok(())
    }
    fn admin(&mut self, text: &'static str) -> anyhow::Result<()> {
        ensure!(
            matches!(
                text,
                "#set skill 0 54"
                    | "#set skill 0 55"
                    | "#set language 0 99"
                    | "#set language 0 100"
            ),
            "unsupported fixture mutation"
        );
        self.poll()?;
        ensure!(self.live().target.is_none(), "fixture must have no target");
        tracing::info!(target: "progression_smoke", text, "bounded self fixture command");
        ensure!(
            self.live.as_mut().unwrap().command(Command::Chat {
                channel: ChatChannel::Say,
                target: String::new(),
                text: text.into(),
                language: 0,
            }),
            "fixture command refused"
        );
        Ok(())
    }
}

fn skills(value: u32) -> String {
    format!("5\t0\t{value}\n5\t1\t55\n5\t28\t55\n")
}
fn languages(value: u32) -> String {
    format!("5\t0\t{value}\n")
}
fn verify_rows(skill: u32, language: u32) -> anyhow::Result<()> {
    ensure!(
        sql("SELECT * FROM character_skills WHERE id=5 ORDER BY skill_id;")? == skills(skill),
        "persistent normal skills changed unexpectedly"
    );
    ensure!(
        sql("SELECT * FROM character_languages WHERE id=5 ORDER BY lang_id;")?
            == languages(language),
        "persistent language skills changed unexpectedly"
    );
    Ok(())
}
fn guard_baseline() -> anyhow::Result<()> {
    offline(0)?;
    let row = sql(
        "SELECT c.id,c.account_id,c.level,c.level2,c.exp,c.points,c.aa_points,c.aa_exp,c.aa_points_spent,c.aa_points_spent_old,c.aa_points_old,c.gm,a.status FROM character_data c JOIN account a ON a.id=c.account_id WHERE c.name='Fellowship' AND a.name='openeq_social1' AND c.ingame=0;",
    )?;
    ensure!(
        row.trim() == "5\t7\t10\t10\t0\t0\t0\t0\t0\t0\t0\t1\t250",
        "dedicated progression baseline changed"
    );
    verify_rows(55, 100)?;
    ensure!(
        sql("SELECT cap FROM skill_caps WHERE class_id=1 AND level=10 AND skill_id=0;")?.trim()
            == "75",
        "deployed fixture skill cap changed"
    );
    ensure!(
        sql("SELECT COUNT(*) FROM character_alternate_abilities WHERE id=5;")?.trim() == "14",
        "fixture AA baseline changed"
    );
    Ok(())
}

fn skill_restore_sql() -> String {
    let guard = format!(
        "EXISTS(SELECT 1 FROM character_data WHERE {} AND id=5 AND account_id=7 AND ingame=0 AND level=10 AND level2=10 AND exp=0 AND points=0 AND aa_points=0 AND aa_exp=0 AND aa_points_spent=0)",
        fixture(0)
    );
    // Existing row identities only; never insert, delete, or replace tables.
    // Unexpected current values are not overwritten. Exact invariant comparison
    // below then reports any external/unexpected change for manual review.
    format!(
        "UPDATE character_skills SET value=55 WHERE id=5 AND skill_id=0 AND value IN (54,55) AND {guard};\nUPDATE character_languages SET value=100 WHERE id=5 AND lang_id=0 AND value IN (99,100) AND {guard};\n"
    )
}

fn restore_fixture(snapshot: &Snapshot) -> anyhow::Result<()> {
    offline(0)?;
    sql(&skill_restore_sql())?;
    sql(&snapshot.restore)?;
    ensure!(
        sql(&format!(
            "SELECT {POSE} FROM character_data WHERE {};",
            fixture(0)
        ))? == snapshot.pose,
        "pose/resources restoration failed"
    );
    ensure!(
        invariants(0)? == snapshot.invariant,
        "progression/gameplay invariants differ after restoration"
    );
    Ok(())
}

fn run(probe: &mut Probe, config: &ConnectionConfig, snapshot: &Snapshot) -> anyhow::Result<()> {
    probe.connect(config)?;
    probe.admin("#set skill 0 54")?;
    probe.wait("skill0 decrease", |l| {
        l.game.progression.skills[0] == Some(54)
    })?;
    probe.values(54, 100)?;
    verify_rows(54, 100)?;
    probe.admin("#set skill 0 55")?;
    probe.wait("skill0 original value", |l| {
        l.game.progression.skills[0] == Some(55)
    })?;
    probe.values(55, 100)?;
    verify_rows(55, 100)?;
    probe.admin("#set language 0 99")?;
    probe.wait("language0 decrease", |l| {
        l.game.progression.languages[0] == Some(99)
    })?;
    probe.values(55, 99)?;
    verify_rows(55, 99)?;
    probe.admin("#set language 0 100")?;
    probe.wait("language0 original value", |l| {
        l.game.progression.languages[0] == Some(100)
    })?;
    probe.values(55, 100)?;
    verify_rows(55, 100)?;
    tracing::info!(target: "progression_smoke", "normal and language updates isolated; level XP and training snapshots unchanged");
    probe.live = None;
    restore_fixture(snapshot)?;
    probe.connect(config)?;
    probe.values(55, 100)?;
    verify_rows(55, 100)?;
    tracing::info!(target: "progression_smoke", "fresh reconnect reproduced restored profile and progression state");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3,
        "usage: progression_smoke SOCIAL1_CONFIG NEW_LOG"
    );
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_social1"
            && config.character == "Fellowship",
        "only the dedicated Fellowship fixture is supported"
    );
    guard_baseline()?;
    let snapshot = snapshot(0)?;
    let opcodes = remote(
        "sudo grep -E '^(OP_SkillUpdate|OP_ExpUpdate|OP_LevelUpdate|OP_PlayerProfile)=' /srv/eqemu/patch_RoF2.conf",
        "",
    )?;
    for (name, expected) in [
        ("OP_SkillUpdate", 0x004c),
        ("OP_ExpUpdate", 0x20ed),
        ("OP_LevelUpdate", 0x1eec),
        ("OP_PlayerProfile", 0x6506),
    ] {
        ensure!(
            opcodes
                .lines()
                .filter_map(|line| line.split_once('='))
                .any(|(key, value)| key == name
                    && u16::from_str_radix(value.strip_prefix("0x").unwrap_or(value), 16)
                        == Ok(expected)),
            "deployed mapping mismatch: {name}"
        );
    }
    let log = PathBuf::from(&args[2]);
    let mut journal = private_file(&log.with_extension("baseline.txt"))?;
    let mut restore = private_file(&log.with_extension("restore.sql"))?;
    restore.write_all(snapshot.restore.as_bytes())?;
    // Record the exact guarded fallback, not a destructive full-row restore.
    restore.write_all(skill_restore_sql().as_bytes())?;
    writeln!(
        journal,
        "FELLOWSHIP PROFILE BASELINE\nPOSE\n{}INVARIANTS (INCLUDING ALL SKILLS/LANGUAGES/AA)\n{}FULL CHARACTER AND INVENTORY\n{}DEPLOYED OPCODES\n{opcodes}BUILD HASHES\n{}",
        snapshot.pose,
        snapshot.invariant,
        sql(
            "SELECT * FROM character_data WHERE id=5 AND name='Fellowship' AND account_id=7; SELECT * FROM inventory WHERE character_id=5 ORDER BY slot_id;"
        )?,
        remote(
            "sudo sha256sum /srv/eqemu/bin/world /srv/eqemu/bin/zone",
            ""
        )?
    )?;
    journal.sync_all()?;
    restore.sync_all()?;
    tracing_subscriber::fmt().with_ansi(false).with_env_filter("progression_smoke=info,openeq_net::movement=debug,openeq_net::session=info,openeq::live=info").with_writer(private_file(&log)?).init();
    let mut probe = Probe::default();
    let result = run(&mut probe, &config, &snapshot);
    if let Err(error) = &result {
        eprintln!("PROBE_FAILED {error:#}");
    }
    probe.live = None;
    let restored = restore_fixture(&snapshot);
    if let Err(error) = &restored {
        eprintln!("RESTORE_FAILED {error:#}");
    }
    ensure!(
        !std::fs::read_to_string(&log)?.contains("sent player position"),
        "progression proof sent movement"
    );
    restored?;
    println!(
        "FELLOWSHIP offline=true restored=pose_resources_skills_languages_training_XP_level_AA_inventory_cash_binds_spells_buffs_corpses_social no_XP_or_level_mutation=true log={}",
        log.display()
    );
    result
}
