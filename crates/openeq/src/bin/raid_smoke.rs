//! Foreground raid proof, restricted to the disposable Fellowship/Companion pair.
//! Uses LiveWorld::raid_request and the real /rsay interaction path, no movement.
use anyhow::{Context, ensure};
use openeq::{interaction::Interaction, live::LiveWorld, raid::Request};
use openeq_net::session::ConnectionConfig;
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const FIXTURES: [(&str, &str); 2] = [
    ("Fellowship", "openeq_social1"),
    ("Companion", "openeq_social2"),
];
const POSE: &str =
    "zone_id,zone_instance,x,y,z,heading,cur_hp,mana,endurance,hunger_level,thirst_level";

fn remote(command: &str, input: &str) -> anyhow::Result<String> {
    let mut child = Command::new("ssh")
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
        SELECT * FROM guild_members WHERE char_id={id};", fixture(i)))
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
                if self.seen[i] != live.game.raid.revision {
                    let raid = &live.game.raid;
                    tracing::info!(target:"raid_smoke", character=FIXTURES[i].0,revision=raid.revision,active=raid.active,confirmed=raid.confirmed,leader=%raid.leader,members=?raid.members,inviter=?raid.invitation.as_ref().map(|v|&v.inviter),"foreground raid state");
                    self.seen[i] = raid.revision;
                }
            }
        }
        Ok(())
    }
    fn wait(&mut self, label: &str, predicate: impl Fn(&Self) -> bool) -> anyhow::Result<()> {
        let until = Instant::now() + Duration::from_secs(45);
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
    fn roster(&self, i: usize, leader: &str) -> bool {
        let r = &self.get(i).game.raid;
        r.active
            && r.confirmed
            && r.leader == leader
            && r.members.len() == 2
            && FIXTURES.iter().all(|(name, _)| {
                r.member(name)
                    .is_some_and(|m| m.level == 10 && m.class == 1)
            })
    }
    fn request(&mut self, i: usize, request: Request) -> bool {
        self.get_mut(i).raid_request(request)
    }
}

fn raid_rows() -> anyhow::Result<String> {
    sql(
        "SELECT raidid,charid,name,groupid,israidleader,isgroupleader FROM raid_members WHERE name IN ('Fellowship','Companion') ORDER BY name;",
    )
}
fn verify_membership(raid_id: u32, leader: &str, expected: &[&str]) -> anyhow::Result<()> {
    let rows = sql(&format!(
        "SELECT name,israidleader FROM raid_members WHERE raidid={raid_id} ORDER BY name;"
    ))?;
    let actual: BTreeSet<_> = rows
        .lines()
        .map(|line| line.split('\t').next().unwrap())
        .collect();
    ensure!(
        actual == expected.iter().copied().collect(),
        "raid membership differs from dedicated fixture set"
    );
    for row in rows.lines() {
        let (name, flag) = row.split_once('\t').context("raid leader row")?;
        ensure!(
            flag == if name == leader { "1" } else { "0" },
            "database raid leader mismatch"
        );
    }
    Ok(())
}

fn run(
    pair: &mut Pair,
    configs: &[ConnectionConfig; 2],
    original_raid_ids: &BTreeSet<u32>,
    raid_id: &mut Option<u32>,
    journal: &mut File,
) -> anyhow::Result<()> {
    for (i, config) in configs.iter().enumerate() {
        pair.live[i] = Some(LiveWorld::start(config.clone()));
    }
    pair.ready(0)?;
    pair.ready(1)?;
    pair.wait("both visible player identities", |p| {
        (0..2).all(|i| {
            p.get(i)
                .entities
                .values()
                .any(|e| !e.spawn.npc && e.spawn.name == FIXTURES[1 - i].0)
        })
    })?;
    ensure!(
        (0..2).all(|i| !pair.get(i).game.raid.active && pair.get(i).game.group.members.is_empty()),
        "fixture started grouped or raided"
    );

    ensure!(
        pair.request(
            0,
            Request::Invite {
                invitee: "Companion".into()
            }
        ),
        "invite refused"
    );
    ensure!(
        !pair.request(
            0,
            Request::Invite {
                invitee: "Companion".into()
            }
        ),
        "duplicate invite queued"
    );
    pair.wait("first invitation", |p| {
        p.get(1).game.raid.invitation.is_some()
    })?;
    let first = pair.get(1).game.raid.invitation.as_ref().unwrap();
    ensure!(first.inviter == "Fellowship", "wrong inviter");
    let stale_token = first.token;
    ensure!(
        pair.request(1, Request::Dismiss { token: stale_token }),
        "dismiss refused"
    );
    ensure!(
        !pair.request(1, Request::Accept { token: stale_token }),
        "dismissed invite accepted"
    );
    ensure!(raid_rows()?.is_empty(), "local dismissal created a raid");
    pair.wait("invite debounce expiry", |p| !p.get(0).game.raid.busy())?;
    ensure!(
        pair.request(
            0,
            Request::Invite {
                invitee: "Companion".into()
            }
        ),
        "second invite refused"
    );
    pair.wait("new invitation", |p| {
        p.get(1)
            .game
            .raid
            .invitation
            .as_ref()
            .is_some_and(|v| v.token != stale_token)
    })?;
    let token = pair.get(1).game.raid.invitation.as_ref().unwrap().token;
    ensure!(
        !pair.request(1, Request::Accept { token: stale_token }),
        "old invitation selected"
    );
    ensure!(pair.request(1, Request::Accept { token }), "accept refused");
    ensure!(
        !pair.request(1, Request::Accept { token }),
        "double accept queued"
    );
    pair.wait("two authoritative raid rosters", |p| {
        p.roster(0, "Fellowship") && p.roster(1, "Fellowship")
    })?;
    let rows = raid_rows()?;
    let ids: BTreeSet<u32> = rows
        .lines()
        .map(|r| {
            r.split('\t')
                .next()
                .context("raid id")?
                .parse()
                .map_err(anyhow::Error::from)
        })
        .collect::<Result<_, _>>()?;
    ensure!(ids.len() == 1, "fixture raid identities differ");
    let created = *ids.first().unwrap();
    ensure!(
        created > 0 && !original_raid_ids.contains(&created),
        "raid id was not newly created"
    );
    *raid_id = Some(created);
    writeln!(journal, "Created fixture-only raid ID: {created}\n{rows}")?;
    journal.sync_all()?;
    verify_membership(created, "Fellowship", &["Fellowship", "Companion"])?;
    println!(
        "RAID joined id={created} dismiss_local=true stale_double_rejected=true rosters=2 leader=Fellowship"
    );

    let mut interaction = Interaction::default();
    // EQEmu intentionally excludes the sender from ServerOP_RaidSay. Prove
    // actual delivery in both directions, independently of any local echo.
    for (sender, (name, _)) in FIXTURES.iter().enumerate() {
        let text = format!("OpenEQ raid smoke {} from {name}", std::process::id());
        ensure!(
            !interaction.submit(&format!("/rsay {text}"), pair.get_mut(sender), [0.; 3]),
            "rsay unexpectedly quit"
        );
        pair.wait("raid chat received by other member", |p| {
            p.get(1 - sender)
                .game
                .chat
                .iter()
                .any(|line| line.text == format!("[Raid] {name}: {text}"))
        })?;
    }
    let stale_revision = pair.get(0).game.raid.revision;
    let revision = pair.get(1).game.raid.revision;
    ensure!(
        !pair.request(
            1,
            Request::MakeLeader {
                revision,
                leader: "Fellowship".into()
            }
        ),
        "nonleader transfer accepted"
    );
    let transfer = Request::MakeLeader {
        revision: stale_revision,
        leader: "Companion".into(),
    };
    ensure!(pair.request(0, transfer.clone()), "leader transfer refused");
    ensure!(!pair.request(0, transfer), "double leader transfer queued");
    pair.wait("leader confirmed on both clients", |p| {
        p.roster(0, "Companion") && p.roster(1, "Companion")
    })?;
    ensure!(
        !pair.request(
            0,
            Request::Leave {
                revision: stale_revision
            }
        ),
        "stale roster leave accepted"
    );
    verify_membership(created, "Companion", &["Fellowship", "Companion"])?;
    println!(
        "RAID chat_channel=15 remote_deliveries=2 leader_transfer=Companion stale_nonleader_double_rejected=true"
    );

    pair.live[1] = None;
    offline(1)?;
    verify_membership(created, "Companion", &["Fellowship", "Companion"])?;
    pair.live[1] = Some(LiveWorld::start(configs[1].clone()));
    pair.seen[1] = 0;
    pair.ready(1)?;
    pair.wait("reconnected authoritative roster", |p| {
        p.roster(0, "Companion") && p.roster(1, "Companion")
    })?;
    verify_membership(created, "Companion", &["Fellowship", "Companion"])?;
    println!("RAID reconnect=Companion roster_rebuilt=true membership_preserved=true");

    for i in [0, 1] {
        let revision = pair.get(i).game.raid.revision;
        ensure!(
            pair.request(i, Request::Leave { revision }),
            "self-leave refused"
        );
        ensure!(
            !pair.request(i, Request::Leave { revision }),
            "double self-leave queued"
        );
        pair.wait("self disband confirmed", |p| {
            !p.get(i).game.raid.active && p.get(i).game.raid.members.is_empty()
        })?;
        if i == 0 {
            pair.wait("remaining member roster", |p| {
                p.get(1).game.raid.members.len() == 1
            })?;
            verify_membership(created, "Companion", &["Companion"])?;
        }
    }
    verify_membership(created, "", &[])?;
    println!("PASS raid_membership_chat_leader_reconnect_leave=true movement_sent=false");
    Ok(())
}

fn cleanup(
    pair: &mut Pair,
    configs: &[ConnectionConfig; 2],
    raid_id: &mut Option<u32>,
    original_raid_ids: &BTreeSet<u32>,
    journal: &mut File,
) -> anyhow::Result<()> {
    // This is also the failure path. Drop any failed connection before retrying
    // through the same foreground reducer; never directly delete membership.
    for i in 0..2 {
        pair.live[i] = None;
    }
    for i in 0..2 {
        offline(i)?;
    }
    let rows = raid_rows()?;
    for row in rows.lines() {
        let found: u32 = row.split('\t').next().context("cleanup raid id")?.parse()?;
        ensure!(
            !original_raid_ids.contains(&found) && raid_id.is_none_or(|id| id == found),
            "unexpected existing raid during cleanup"
        );
        *raid_id = Some(found);
        writeln!(journal, "Cleanup observed fixture raid ID: {found}")?;
        journal.sync_all()?;
        let others = sql(&format!(
            "SELECT COUNT(*) FROM raid_members WHERE raidid={found} AND NOT ((charid={} AND name='Fellowship') OR (charid={} AND name='Companion'));",
            id(0),
            id(1)
        ))?;
        ensure!(
            others.trim() == "0",
            "raid contains a nonfixture member; refusing cleanup"
        );
    }
    if !rows.is_empty() {
        for (i, config) in configs.iter().enumerate() {
            let member = sql(&format!(
                "SELECT COUNT(*) FROM raid_members WHERE charid={} AND name='{}';",
                id(i),
                FIXTURES[i].0
            ))?;
            if member.trim() == "0" {
                continue;
            }
            pair.live[i] = Some(LiveWorld::start(config.clone()));
            pair.ready(i)?;
            pair.wait("cleanup raid roster", |p| {
                p.get(i).game.raid.active && p.get(i).game.raid.member(FIXTURES[i].0).is_some()
            })?;
            let revision = pair.get(i).game.raid.revision;
            ensure!(
                pair.request(i, Request::Leave { revision }),
                "cleanup self-leave refused"
            );
            pair.wait("cleanup leave confirmed", |p| !p.get(i).game.raid.active)?;
            pair.live[i] = None;
            offline(i)?;
        }
    }
    ensure!(raid_rows()?.is_empty(), "fixture raid membership remains");
    if let Some(rid) = *raid_id {
        ensure!(
            sql(&format!(
                "SELECT COUNT(*) FROM raid_members WHERE raidid={rid};"
            ))?
            .trim()
                == "0",
            "nonfixture raid members appeared"
        );
        // EQEmu's normal empty-raid path leaves metadata rows. Only delete the
        // newly recorded ID, after normal self-leave and both fixtures offline.
        let offline = format!(
            "(SELECT COUNT(*) FROM character_data WHERE ({}) OR ({}))=2 AND (SELECT COUNT(*) FROM character_data WHERE (({}) OR ({})) AND ingame<>0)=0",
            fixture(0),
            fixture(1),
            fixture(0),
            fixture(1)
        );
        sql(&format!("DELETE FROM raid_details WHERE raidid={rid} AND NOT EXISTS(SELECT 1 FROM raid_members WHERE raidid={rid}) AND {offline};
            DELETE FROM raid_leaders WHERE rid={rid} AND NOT EXISTS(SELECT 1 FROM raid_members WHERE raidid={rid}) AND {offline};"))?;
        let counts = sql(&format!(
            "SELECT COUNT(*) FROM raid_details WHERE raidid={rid}; SELECT COUNT(*) FROM raid_leaders WHERE rid={rid};"
        ))?;
        ensure!(
            counts.split_whitespace().collect::<Vec<_>>() == ["0", "0"],
            "fixture raid metadata remains"
        );
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: raid_smoke SOCIAL1_CONFIG SOCIAL2_CONFIG NEW_LOG"
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
            "requires the dedicated Fellowship/Companion pair only"
        );
    }
    let opcodes = remote(
        "sudo grep -E '^(OP_RaidInvite|OP_RaidUpdate|OP_RaidJoin|OP_ChannelMessage)=' /srv/eqemu/patch_RoF2.conf",
        "",
    )?;
    for expected in [
        "OP_RaidInvite=0x55ac",
        "OP_RaidUpdate=0x3973",
        "OP_RaidJoin=0x0000",
        "OP_ChannelMessage=0x2b2d",
    ] {
        ensure!(
            opcodes.lines().any(|l| l == expected),
            "deployed mapping mismatch: {expected}"
        );
    }
    let snapshots = [snapshot(0)?, snapshot(1)?];
    let ids = sql(
        "SELECT raidid FROM raid_details UNION SELECT raidid FROM raid_members UNION SELECT rid FROM raid_leaders;",
    )?;
    let original_raid_ids = ids
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<BTreeSet<u32>, _>>()?;
    let log = PathBuf::from(&args[3]);
    let mut restore = private_file(&log.with_extension("restore.sql"))?;
    let mut journal = private_file(&log.with_extension("baseline.txt"))?;
    for (i, snapshot) in snapshots.iter().enumerate() {
        restore.write_all(snapshot.restore.as_bytes())?;
        writeln!(
            journal,
            "{}\nPOSE\n{}INVARIANTS\n{}",
            FIXTURES[i].0, snapshot.pose, snapshot.invariant
        )?;
        // EQEmu reassigns inventory.guid on login. Preserve the full rows as
        // evidence, but compare every persistent gameplay field separately.
        writeln!(
            journal,
            "INVENTORY INCLUDING SESSION GUID\n{}",
            sql(&format!(
                "SELECT * FROM inventory WHERE character_id={} ORDER BY slot_id;",
                id(i)
            ))?
        )?;
    }
    writeln!(journal, "DEPLOYED OPCODES\n{opcodes}PRIOR RAID IDS\n{ids}")?;
    restore.sync_all()?;
    journal.sync_all()?;
    tracing_subscriber::fmt().with_ansi(false).with_env_filter("raid_smoke=info,openeq_net::raid=info,openeq_net::movement=debug,openeq_net::session=info,openeq_net::account=info,openeq::live=info").with_writer(private_file(&log)?).init();
    let mut pair = Pair::default();
    let mut raid_id = None;
    let result = run(
        &mut pair,
        &configs,
        &original_raid_ids,
        &mut raid_id,
        &mut journal,
    );
    if let Err(error) = &result {
        eprintln!("PROBE_FAILED {error:#}");
        for (i, live) in pair.live.iter().enumerate() {
            if let Some(live) = live {
                tracing::error!(target:"raid_smoke", character=FIXTURES[i].0,
                    status=?live.game.raid.status,
                    recent_chat=?live.game.chat.iter().rev().take(12).map(|line|&line.text).collect::<Vec<_>>(),
                    "probe failure state");
            }
        }
    }
    let cleaned = cleanup(
        &mut pair,
        &configs,
        &mut raid_id,
        &original_raid_ids,
        &mut journal,
    );
    if let Err(error) = &cleaned {
        eprintln!("CLEANUP_FAILED {error:#}");
    }
    // Even a metadata cleanup failure must not prevent offline restoration of
    // each independent character. Never overwrite a character still online.
    pair.live = [None, None];
    let mut restoration_errors = Vec::new();
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
        "raid probe sent movement"
    );
    println!(
        "FIXTURES offline=true pose_resources=restored inventory_cash_binds_spells_buffs_corpses_level_xp_stats_group_guild=unchanged raid_cleanup={} log={}",
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
