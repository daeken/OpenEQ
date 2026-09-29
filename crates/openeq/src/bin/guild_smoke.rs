//! Guarded receive-only guild proof using the dedicated Fellowship/Companion pair.
//! Temporary fixture administration uses supported GM chat commands. There is
//! no outgoing guild protocol API, player movement, item, currency, or combat use.
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

const SETUP_NAME: &str = "OpenEQ Receive Setup";
const GUILD_NAME: &str = "OpenEQ Receive Proof";
const MOTD: &str = "";
const FIXTURES: [(&str, &str); 2] = [
    ("Fellowship", "openeq_social1"),
    ("Companion", "openeq_social2"),
];
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
    sql(&format!("SELECT id,account_id,name,level,exp,gm,race,class,`str`,sta,cha,dex,`int`,agi,wis FROM character_data WHERE {};
        SELECT character_id,slot_id,item_id,charges,color,augment_one,augment_two,augment_three,augment_four,augment_five,augment_six,instnodrop,custom_data,ornament_icon,ornament_idfile,ornament_hero_model FROM inventory WHERE character_id={id} ORDER BY slot_id;
        SELECT * FROM character_currency WHERE id={id};
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
            "UPDATE character_data SET {set} WHERE {} AND ingame=0;\n",
            fixture(i)
        ),
    })
}

#[derive(Default)]
struct Pair {
    live: [Option<LiveWorld>; 2],
    seen: [u64; 2],
}
impl Pair {
    fn get(&self, i: usize) -> &LiveWorld {
        self.live[i].as_ref().expect("connected fixture")
    }
    fn get_mut(&mut self, i: usize) -> &mut LiveWorld {
        self.live[i].as_mut().expect("connected fixture")
    }
    fn poll(&mut self) -> anyhow::Result<()> {
        for (i, slot) in self.live.iter_mut().enumerate() {
            if let Some(live) = slot {
                live.poll();
                ensure!(
                    live.error.is_none(),
                    "{} network error: {:?}",
                    FIXTURES[i].0,
                    live.error
                );
                if self.seen[i] != live.game.guild.revision {
                    let g = &live.game.guild;
                    tracing::info!(target: "guild_smoke", character=FIXTURES[i].0, revision=g.revision,
                        confirmed=g.confirmed, id=?g.guild_id, rank=?g.rank, name=?g.name,
                        roster_received=g.roster_received, members=?g.members, motd=?g.motd,
                        "foreground guild state");
                    self.seen[i] = g.revision;
                }
            }
        }
        Ok(())
    }
    fn wait(&mut self, label: &str, predicate: impl Fn(&Self) -> bool) -> anyhow::Result<()> {
        let until = Instant::now() + Duration::from_secs(30);
        loop {
            self.poll()?;
            if predicate(self) {
                return Ok(());
            }
            ensure!(Instant::now() < until, "timeout: {label}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn ready(&mut self, i: usize) -> anyhow::Result<()> {
        self.wait("profile and zone Ready", |p| {
            let l = p.get(i);
            l.ready && l.own_id.is_some() && l.game.profile.is_some()
        })?;
        let l = self.get(i);
        ensure!(
            l.game.profile.as_ref().unwrap().name == FIXTURES[i].0
                && l.environment
                    .as_ref()
                    .is_some_and(|e| e.zone_id == 202 && e.instance_id == 0),
            "wrong player/zone"
        );
        Ok(())
    }
    fn connect(&mut self, i: usize, config: &ConnectionConfig) -> anyhow::Result<()> {
        self.live[i] = Some(LiveWorld::start(config.clone()));
        self.ready(i)
    }
    fn admin(&mut self, text: String) -> anyhow::Result<()> {
        ensure!(
            text.starts_with("#guild "),
            "only guild fixture administration is allowed"
        );
        tracing::info!(target: "guild_smoke", %text, "fixture administration");
        ensure!(
            self.get_mut(0).command(Command::Chat {
                channel: ChatChannel::Say,
                target: String::new(),
                text,
                language: 0,
            }),
            "fixture administration command refused"
        );
        Ok(())
    }
    fn complete(&self, i: usize, guild_id: u32) -> bool {
        let g = &self.get(i).game.guild;
        g.confirmed
            && g.guild_id == Some(guild_id)
            && g.name.as_deref() == Some(GUILD_NAME)
            && g.roster_received
            && g.rank == Some(if i == 0 { 1 } else { 5 })
            && g.members.len() == 2
            && FIXTURES.iter().enumerate().all(|(j, (name, _))| {
                g.member(name).is_some_and(|m| {
                    m.level == 10
                        && m.class == 1
                        && m.rank == if j == 0 { 1 } else { 5 }
                        && m.alt.is_some()
                        && m.banker.is_some()
                        && m.public_note.is_some()
                })
            })
            && g.motd
                .as_ref()
                .is_some_and(|m| m.author.is_empty() && m.text == MOTD)
    }
}

fn guard_guild(guild_id: u32, require_both: bool) -> anyhow::Result<()> {
    ensure!(
        guild_id > 0 && guild_id < 50_000,
        "invalid fixture guild ID"
    );
    ensure!(sql(&format!("SELECT COUNT(*) FROM guilds WHERE id={guild_id} AND leader={} AND name IN ('{SETUP_NAME}','{GUILD_NAME}');", id(0)))?.trim() == "1",
            "fixture guild identity changed; refusing mutation");
    let members = sql(&format!(
        "SELECT char_id,rank FROM guild_members WHERE guild_id={guild_id} ORDER BY char_id;"
    ))?;
    let allowed = [
        format!(
            "{}\t1",
            sql(&format!(
                "SELECT id FROM character_data WHERE {};",
                fixture(0)
            ))?
            .trim()
        ),
        format!(
            "{}\t5",
            sql(&format!(
                "SELECT id FROM character_data WHERE {};",
                fixture(1)
            ))?
            .trim()
        ),
    ];
    ensure!(
        members
            .lines()
            .all(|row| allowed.iter().any(|entry| entry == row)),
        "nonfixture guild member or unexpected rank; refusing mutation"
    );
    ensure!(
        !require_both || members.lines().count() == 2,
        "fixture guild needs both members"
    );
    let assets = sql(&format!("SELECT COUNT(*) FROM guild_bank WHERE guild_id={guild_id};
        SELECT COUNT(*) FROM guild_tributes WHERE guild_id={guild_id} AND (tribute_id_1<>0 OR tribute_id_2<>0 OR enabled<>0);
        SELECT COUNT(*) FROM guild_relations WHERE guild1={guild_id} OR guild2={guild_id};
        SELECT COUNT(*) FROM guilds WHERE id={guild_id} AND (tribute<>0 OR favor<>0);
        SELECT COUNT(*) FROM guild_members WHERE guild_id={guild_id} AND (tribute_enable<>0 OR total_tribute<>0 OR last_tribute<>0);"))?;
    ensure!(
        assets.split_whitespace().all(|v| v == "0"),
        "fixture guild has assets/relations/tribute; refusing mutation"
    );
    Ok(())
}

fn verify_roster(pair: &Pair, i: usize, guild_id: u32) -> anyhow::Result<()> {
    let rows = sql(&format!(
        "SELECT cd.name,cd.level,cd.class,gm.rank,gm.banker,gm.alt,HEX(gm.public_note) FROM guild_members gm JOIN character_data cd ON cd.id=gm.char_id WHERE gm.guild_id={guild_id} ORDER BY cd.name;"
    ))?;
    let actual = &pair.get(i).game.guild;
    for line in rows.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        ensure!(fields.len() == 7, "unexpected guild member SQL shape");
        let member = actual
            .member(fields[0])
            .context("database member absent from foreground reducer")?;
        ensure!(
            member.level == fields[1].parse::<u32>()?
                && member.class == fields[2].parse::<u32>()?
                && member.rank == fields[3].parse::<u32>()?
                && member.banker == Some(fields[4] == "1")
                && member.alt == Some(fields[5] == "1")
                && fields[6].is_empty()
                && member.public_note.as_deref() == Some(""),
            "foreground roster differs from SQL"
        );
    }
    ensure!(
        rows.lines().count() == actual.members.len(),
        "foreground/database member counts differ"
    );
    Ok(())
}

fn run(
    pair: &mut Pair,
    configs: &[ConnectionConfig; 2],
    guild_id: &mut Option<u32>,
    journal: &mut File,
) -> anyhow::Result<()> {
    for (i, config) in configs.iter().enumerate() {
        pair.connect(i, config)?;
    }
    pair.wait("both confirmed guildless", |p| {
        (0..2).all(|i| {
            let g = &p.get(i).game.guild;
            g.confirmed && g.guild_id.is_none()
        })
    })?;
    pair.admin(format!("#guild create Fellowship {SETUP_NAME}"))?;
    pair.wait("creator guild identity", |p| {
        p.get(0).game.guild.confirmed && p.get(0).game.guild.guild_id.is_some()
    })?;
    let gid = pair
        .get(0)
        .game
        .guild
        .guild_id
        .context("created guild identity")?;
    *guild_id = Some(gid);
    writeln!(journal, "CREATED FIXTURE GUILD ID {gid}")?;
    journal.sync_all()?;
    guard_guild(gid, false)?;
    pair.admin(format!("#guild set Companion {gid}"))?;
    pair.wait("both fixture guild identities", |p| {
        (0..2).all(|i| p.get(i).game.guild.guild_id == Some(gid))
    })?;
    guard_guild(gid, true)?;
    pair.admin(format!("#guild rename {gid} {GUILD_NAME}"))?;
    pair.wait("cache refresh directory name", |p| {
        (0..2).all(|i| p.get(i).game.guild.name.as_deref() == Some(GUILD_NAME))
    })?;
    writeln!(
        journal,
        "PREPARED GUILD ROWS\n{}",
        sql(&format!(
            "SELECT * FROM guilds WHERE id={gid}; SELECT * FROM guild_members WHERE guild_id={gid} ORDER BY char_id; SELECT * FROM guild_ranks WHERE guild_id={gid} ORDER BY rank; SELECT * FROM guild_permissions WHERE guild_id={gid} ORDER BY id; SELECT * FROM guild_tributes WHERE guild_id={gid};"
        ))?
    )?;
    journal.sync_all()?;
    pair.live = [None, None];
    for i in 0..2 {
        offline(i)?;
    }
    for (i, config) in configs.iter().enumerate() {
        pair.connect(i, config)?;
    }
    pair.wait("both full initial guild snapshots", |p| {
        (0..2).all(|i| p.complete(i, gid))
    })?;
    for i in 0..2 {
        verify_roster(pair, i, gid)?;
    }
    tracing::info!(target: "guild_smoke", guild_id=gid, "both initial profiles directories rosters and MOTDs verified");
    pair.live[1] = None;
    offline(1)?;
    // Poll through the server's logout update. Do not require a guessed status
    // from the shared legacy/update opcode; the typed trace preserves certainty.
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        pair.poll()?;
        std::thread::sleep(Duration::from_millis(10));
    }
    pair.connect(1, &configs[1])?;
    pair.wait("reconnected full guild snapshot", |p| p.complete(1, gid))?;
    verify_roster(pair, 1, gid)?;
    ensure!(
        pair.get(0).game.guild.confirmed && pair.get(0).game.guild.guild_id == Some(gid),
        "peer membership changed on reconnect"
    );
    tracing::info!(target: "guild_smoke", "normal reconnect retained server membership and rebuilt guild state");
    Ok(())
}

fn cleanup(
    pair: &mut Pair,
    configs: &[ConnectionConfig; 2],
    guild_id: &mut Option<u32>,
    journal: &mut File,
) -> anyhow::Result<()> {
    pair.live = [None, None];
    for i in 0..2 {
        offline(i)?;
    }
    let rows = sql(&format!(
        "SELECT id FROM guilds WHERE leader={} AND name IN ('{SETUP_NAME}','{GUILD_NAME}');",
        id(0)
    ))?;
    ensure!(rows.lines().count() <= 1, "ambiguous fixture guild cleanup");
    if let Some(row) = rows.lines().next() {
        let gid: u32 = row.parse()?;
        ensure!(
            guild_id.is_none_or(|known| known == gid),
            "unexpected fixture guild ID"
        );
        *guild_id = Some(gid);
        writeln!(journal, "CLEANUP RECORDED FIXTURE GUILD {gid}")?;
        journal.sync_all()?;
        guard_guild(gid, false)?;
        for (i, config) in configs.iter().enumerate() {
            pair.connect(i, config)?;
        }
        guard_guild(gid, false)?;
        pair.admin(format!("#guild delete {gid}"))?;
        pair.wait("both server-confirmed guildless after deletion", |p| {
            (0..2).all(|i| {
                let g = &p.get(i).game.guild;
                g.confirmed && g.guild_id.is_none()
            })
        })?;
    }
    pair.live = [None, None];
    for i in 0..2 {
        offline(i)?;
    }
    if let Some(gid) = *guild_id {
        let remaining = sql(&format!(
            "SELECT COUNT(*) FROM guilds WHERE id={gid}; SELECT COUNT(*) FROM guild_members WHERE guild_id={gid}; SELECT COUNT(*) FROM guild_ranks WHERE guild_id={gid}; SELECT COUNT(*) FROM guild_permissions WHERE guild_id={gid}; SELECT COUNT(*) FROM guild_tributes WHERE guild_id={gid}; SELECT COUNT(*) FROM guild_bank WHERE guild_id={gid}; SELECT COUNT(*) FROM guild_relations WHERE guild1={gid} OR guild2={gid};"
        ))?;
        ensure!(
            remaining.split_whitespace().collect::<Vec<_>>() == ["0"; 7],
            "fixture guild dependent rows remain"
        );
        // EQEmu _StoreGuildDB (common/guild_base.cpp342–349) neglects to set
        // gt.guild_id before ReplaceOne, writing this empty sentinel under ID0.
        // The preflight requires no tribute rows. Only restore this exact new
        // artifact once the recorded fixture guild is deleted and both offline.
        let orphan = sql("SELECT * FROM guild_tributes WHERE guild_id=0;")?;
        if !orphan.is_empty() {
            ensure!(
                orphan.trim_end() == "0\t4294967295\t0\t4294967295\t0\t600000\t0",
                "unexpected guild0 tribute state; refusing cleanup"
            );
            writeln!(
                journal,
                "SOURCE-CONFIRMED NEW EMPTY GUILD0 TRIBUTE ORPHAN\n{orphan}"
            )?;
            journal.sync_all()?;
            let removed = sql(&format!(
                "DELETE FROM guild_tributes WHERE guild_id=0 AND tribute_id_1=4294967295 AND tribute_id_1_tier=0 AND tribute_id_2=4294967295 AND tribute_id_2_tier=0 AND time_remaining=600000 AND enabled=0 AND NOT EXISTS(SELECT 1 FROM guilds WHERE id=0) AND NOT EXISTS(SELECT 1 FROM guild_members) AND (SELECT COUNT(*) FROM character_data WHERE (({}) OR ({})) AND ingame=0)=2; SELECT ROW_COUNT();",
                fixture(0),
                fixture(1)
            ))?;
            ensure!(
                removed.trim() == "1",
                "guarded empty tribute orphan cleanup failed"
            );
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: guild_smoke SOCIAL1_CONFIG SOCIAL2_CONFIG NEW_LOG"
    );
    let configs = [
        ConnectionConfig::load(Path::new(&args[1]))?,
        ConnectionConfig::load(Path::new(&args[2]))?,
    ];
    for (i, c) in configs.iter().enumerate() {
        ensure!(
            c.host == "storage2.daeken.dev"
                && c.username == FIXTURES[i].1
                && c.character == FIXTURES[i].0,
            "only dedicated social fixtures are supported"
        );
    }
    ensure!(
        sql("SELECT COUNT(*) FROM guilds; SELECT COUNT(*) FROM guild_members; SELECT COUNT(*) FROM guild_tributes;")?
            .split_whitespace()
            .collect::<Vec<_>>()
            == ["0", "0", "0"],
        "this first guild proof requires the observed empty guild baseline"
    );
    let opcodes = remote(
        "sudo grep -E '^(OP_GuildsList|OP_GuildMemberList|OP_GuildMOTD|OP_GetGuildMOTDReply|OP_GuildMemberDetails|OP_GuildMemberUpdate|OP_GuildMemberRankAltBanker|OP_SetGuildRank|OP_SpawnAppearance)=' /srv/eqemu/patch_RoF2.conf",
        "",
    )?;
    for expected in [
        "OP_GuildsList=0x507a",
        "OP_GuildMemberList=0x12a6",
        "OP_GuildMOTD=0x3e13",
        "OP_GetGuildMOTDReply=0x4f1f",
        "OP_GuildMemberDetails=0x69b9",
        "OP_GuildMemberUpdate=0x69b9",
        "OP_GuildMemberRankAltBanker=0x0b9c",
        "OP_SetGuildRank=0x0b9c",
    ] {
        ensure!(
            opcodes.lines().any(|line| line == expected),
            "deployed mapping mismatch: {expected}"
        );
    }
    let snapshots = [snapshot(0)?, snapshot(1)?];
    let log = PathBuf::from(&args[3]);
    let mut restore = private_file(&log.with_extension("restore.sql"))?;
    let mut journal = private_file(&log.with_extension("baseline.txt"))?;
    for (i, snapshot) in snapshots.iter().enumerate() {
        restore.write_all(snapshot.restore.as_bytes())?;
        writeln!(
            journal,
            "{}\nPOSE\n{}INVARIANTS\n{}FULL CHARACTER AND INVENTORY (SESSION GUID MAY REGENERATE)\n{}",
            FIXTURES[i].0,
            snapshot.pose,
            snapshot.invariant,
            sql(&format!(
                "SELECT * FROM character_data WHERE {}; SELECT * FROM inventory WHERE character_id={} ORDER BY slot_id;",
                fixture(i),
                id(i)
            ))?
        )?;
    }
    writeln!(
        journal,
        "DEPLOYED OPCODES\n{opcodes}BUILD HASHES\n{}GUILD BASELINE\n{}",
        remote(
            "sudo sha256sum /srv/eqemu/bin/world /srv/eqemu/bin/zone",
            ""
        )?,
        sql(
            "SELECT * FROM guilds; SELECT * FROM guild_members; SELECT * FROM guild_ranks; SELECT * FROM guild_permissions; SELECT * FROM guild_tributes; SELECT * FROM guild_bank; SELECT * FROM guild_relations;"
        )?
    )?;
    restore.sync_all()?;
    journal.sync_all()?;
    tracing_subscriber::fmt().with_ansi(false).with_env_filter("guild_smoke=info,openeq_net::movement=debug,openeq_net::session=info,openeq::live=info").with_writer(private_file(&log)?).init();
    let mut pair = Pair::default();
    let mut guild_id = None;
    let result = run(&mut pair, &configs, &mut guild_id, &mut journal);
    if let Err(error) = &result {
        eprintln!("PROBE_FAILED {error:#}");
        for (i, live) in pair.live.iter().enumerate() {
            if let Some(live) = live {
                tracing::error!(target: "guild_smoke", character=FIXTURES[i].0,
                    recent_chat=?live.game.chat.iter().rev().take(10).map(|l| &l.text).collect::<Vec<_>>(), "probe failure state");
            }
        }
    }
    let cleaned = cleanup(&mut pair, &configs, &mut guild_id, &mut journal);
    if let Err(error) = &cleaned {
        eprintln!("CLEANUP_FAILED {error:#}");
    }
    pair.live = [None, None];
    let mut restoration_errors = vec![];
    for (i, snapshot) in snapshots.iter().enumerate() {
        let restored = (|| -> anyhow::Result<()> {
            offline(i)?;
            sql(&snapshot.restore)?;
            ensure!(
                sql(&format!(
                    "SELECT {POSE} FROM character_data WHERE {};",
                    fixture(i)
                ))? == snapshot.pose,
                "{} pose/resources restore failed",
                FIXTURES[i].0
            );
            ensure!(
                invariants(i)? == snapshot.invariant,
                "{} gameplay/social invariants changed",
                FIXTURES[i].0
            );
            Ok(())
        })();
        if let Err(error) = restored {
            restoration_errors.push(format!("{error:#}"));
        }
    }
    ensure!(
        restoration_errors.is_empty(),
        "fixture restoration: {}",
        restoration_errors.join("; ")
    );
    ensure!(
        !std::fs::read_to_string(&log)?.contains("sent player position"),
        "guild probe sent movement"
    );
    println!(
        "FIXTURES offline=true pose_resources=restored inventory_cash_binds_spells_buffs_corpses_level_xp_stats_group_raid_guild=unchanged guild_cleanup={} log={}",
        if cleaned.is_ok() {
            "complete"
        } else {
            "failed"
        },
        log.display()
    );
    cleaned?;
    result
}
