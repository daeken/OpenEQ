//! Authenticated border round trip, restricted to the dedicated Mechanic fixture.
//! Records recovery data before login and restores the original server pose,
//! including when an assertion fails. Never uses Explorer or changes inventory.
use anyhow::{Context, ensure};
use openeq::{
    coordinates::{scene_point_to_server, server_heading_to_scene},
    live::LiveWorld,
    movement::GroundMotion,
    zone_travel::ZoneTravel,
};
use openeq_assets::{collision::CollisionWorld, loader, zone_lines::ZoneLines};
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
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
struct Pose {
    zone: u16,
    instance: u16,
    xyz: [f32; 3],
    heading: f32,
}
struct Snapshot {
    pose: Pose,
    stable: String,
    resources: String,
}
fn query(sql: &str) -> anyhow::Result<String> {
    let mut child = Process::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "storage2.daeken.dev",
            "sudo mariadb --batch --raw --skip-column-names peq",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("database stdin")?
        .write_all(sql.as_bytes())?;
    let output = child.wait_with_output()?;
    ensure!(
        output.status.success(),
        "read-only fixture query failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}
fn snapshot() -> anyhow::Result<Snapshot> {
    let row = query(
        "SELECT c.zone_id,c.zone_instance,c.x,c.y,c.z,c.heading FROM character_data c JOIN account a ON a.id=c.account_id WHERE c.name='Mechanic' AND a.name='openeq_gameplay';",
    )?;
    let fields: Vec<_> = row.trim().split('\t').collect();
    ensure!(
        fields.len() == 6,
        "dedicated Mechanic fixture missing or ambiguous"
    );
    let pose = Pose {
        zone: fields[0].parse()?,
        instance: fields[1].parse()?,
        xyz: [fields[2].parse()?, fields[3].parse()?, fields[4].parse()?],
        heading: fields[5].parse()?,
    };
    // EQEmu assigns new runtime item GUIDs when loading another zone; compare
    // all persistent item content without treating that bookkeeping as a trade.
    let stable = query(
        "SELECT 'STATS'; SELECT race,class,level,exp,aa_exp,aa_points,aa_points_spent,`str`,sta,cha,dex,`int`,agi,wis,gm FROM character_data WHERE name='Mechanic'; SELECT 'CURRENCY'; SELECT * FROM character_currency WHERE id=(SELECT id FROM character_data WHERE name='Mechanic'); SELECT 'INVENTORY'; SELECT character_id,slot_id,item_id,charges,color,augment_one,augment_two,augment_three,augment_four,augment_five,augment_six,instnodrop,HEX(custom_data),ornament_icon,ornament_idfile,ornament_hero_model FROM inventory WHERE character_id=(SELECT id FROM character_data WHERE name='Mechanic') ORDER BY slot_id;",
    )?;
    let resources =
        query("SELECT cur_hp,mana,endurance FROM character_data WHERE name='Mechanic';")?;
    Ok(Snapshot {
        pose,
        stable,
        resources,
    })
}
fn wait_offline() -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if query("SELECT ingame FROM character_data WHERE name='Mechanic';")?.trim() == "0" {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Mechanic has not logged out");
        std::thread::sleep(Duration::from_millis(500));
    }
}
struct Zone {
    name: &'static str,
    collision: CollisionWorld,
    lines: ZoneLines,
}
impl Zone {
    fn load(base: &Path, name: &'static str) -> anyhow::Result<Self> {
        let scene = loader::load_zone(base, name)?;
        Ok(Self {
            name,
            collision: CollisionWorld::build(&scene),
            lines: ZoneLines::load(base, name)?,
        })
    }
}
struct Probe {
    live: LiveWorld,
    camera: Camera,
    travel: ZoneTravel,
    generation: u64,
    corrections: u64,
}
impl Probe {
    fn new(config: ConnectionConfig) -> Self {
        Self {
            live: LiveWorld::start(config),
            camera: Camera::default(),
            travel: ZoneTravel::default(),
            generation: 0,
            corrections: 0,
        }
    }
    fn anchor(&self) -> [f32; 3] {
        [
            self.camera.position[0],
            self.camera.position[1],
            self.camera.position[2] - 3.,
        ]
    }
    fn poll(&mut self) -> anyhow::Result<()> {
        self.live.poll();
        ensure!(
            self.live.error.is_none(),
            "connection failed: {:?}",
            self.live.error
        );
        if let Some(p) = self.live.initial_position.take() {
            self.camera.position = [p.x, p.y, p.z + 3.];
            self.camera.yaw = p.heading * std::f32::consts::TAU / 512.;
            self.corrections += 1;
            if self.live.zone_generation != self.generation {
                self.generation = self.live.zone_generation;
                self.travel.reset([p.x, p.y, p.z]);
            } else {
                self.travel.rebase([p.x, p.y, p.z]);
            }
            println!(
                "AUTHORITATIVE generation={} zone={:?} scene_anchor=({:.3},{:.3},{:.3}) heading={:.3}",
                self.generation,
                self.live
                    .environment
                    .as_ref()
                    .map(|e| e.short_name.as_str()),
                p.x,
                p.y,
                p.z,
                p.heading
            );
        }
        self.live.camera_position(&self.camera, false);
        Ok(())
    }
    fn wait(
        &mut self,
        label: &str,
        seconds: u64,
        done: impl Fn(&Self) -> bool,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.poll()?;
            if done(self) {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "timeout {label}; zone={:?} ready={} pending={} recent={:?}",
                self.live.environment.as_ref().map(|e| &e.short_name),
                self.live.ready,
                self.live.zone_request_pending(),
                self.live
                    .game
                    .chat
                    .iter()
                    .rev()
                    .take(5)
                    .map(|l| &l.text)
                    .collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn ready_in(&self, zone: u16) -> bool {
        self.live.ready
            && self.live.own_id.is_some()
            && self
                .live
                .environment
                .as_ref()
                .is_some_and(|e| e.zone_id == zone)
    }
    fn say(&mut self, message: String) -> anyhow::Result<()> {
        ensure!(
            self.live.command(Command::Chat {
                channel: ChatChannel::Say,
                target: String::new(),
                text: message,
                language: 0
            }),
            "GM fixture command refused locally"
        );
        Ok(())
    }
    fn goto(&mut self, pose: &Pose) -> anyhow::Result<()> {
        let before = self.corrections;
        self.say(format!(
            "#goto {} {} {} {}",
            pose.xyz[0], pose.xyz[1], pose.xyz[2], pose.heading
        ))?;
        self.wait("GM positioning correction", 10, |p| {
            let actual = scene_point_to_server(p.anchor());
            p.corrections > before
                && actual
                    .into_iter()
                    .zip(pose.xyz)
                    .all(|(a, b)| (a - b).abs() < 0.05)
                && (p.camera.yaw * 512. / std::f32::consts::TAU
                    - server_heading_to_scene(pose.heading))
                .abs()
                    < 0.1
        })
    }
    fn cross(
        &mut self,
        zone: &Zone,
        number: u32,
        direction: f32,
        destination: u16,
        expected: [f32; 3],
    ) -> anyhow::Result<()> {
        let generation = self.live.zone_generation;
        ensure!(
            self.live
                .zone_points
                .get(&number)
                .is_some_and(|p| p.zone_id == destination),
            "{} destination table missing exit{number} -> {destination}",
            zone.name
        );
        self.travel.reset(self.anchor());
        ensure!(
            zone.lines.region_at(self.anchor()).is_none(),
            "{} starts inside exit volume",
            zone.name
        );
        let start = self.anchor();
        let mut motion = GroundMotion::default();
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut crossing = None;
        while Instant::now() < deadline {
            self.poll()?;
            ensure!(
                self.live.zone_generation == generation,
                "unexpected transfer before authored crossing"
            );
            let feet = [
                self.camera.position[0],
                self.camera.position[1],
                self.camera.position[2] - 6.,
            ];
            let moved = motion.step(
                &zone.collision,
                feet,
                [direction * 20., 0.],
                false,
                1. / 60.,
            );
            self.camera.position = [moved[0], moved[1], moved[2] + 6.];
            self.camera.yaw = if direction > 0. {
                std::f32::consts::FRAC_PI_2
            } else {
                std::f32::consts::PI * 1.5
            };
            self.live.camera_position(&self.camera, true);
            if let Some(line) = self.travel.observe(&zone.lines, self.anchor()) {
                ensure!(
                    line.number == number,
                    "unexpected exit {} instead of {number}",
                    line.number
                );
                let anchor = self.anchor();
                println!(
                    "CROSS source={} number={number} scene_anchor={anchor:?} server_anchor={:?} moved={:.3}",
                    zone.name,
                    scene_point_to_server(anchor),
                    (anchor[0] - start[0]).abs()
                );
                ensure!(
                    self.live.cross_zone_line(number, &self.camera),
                    "natural crossing was refused locally"
                );
                ensure!(
                    self.live.zone_request_pending(),
                    "request missing pending state"
                );
                ensure!(
                    !self.live.cross_zone_line(number, &self.camera),
                    "duplicate crossing accepted while pending"
                );
                crossing = Some(anchor);
                break;
            }
            std::thread::sleep(Duration::from_secs_f32(1. / 60.));
        }
        ensure!(
            crossing.is_some(),
            "{} never crossed exit{number}: start={start:?} end={:?}",
            zone.name,
            self.anchor()
        );
        let before = self.corrections;
        self.wait("authenticated border handoff", 45, |p| {
            (p.live.zone_generation > generation
                && p.ready_in(destination)
                && p.corrections > before
                && !p.live.zone_points.is_empty())
                || (p.live.zone_generation == generation && !p.live.zone_request_pending())
        })?;
        ensure!(
            self.live.zone_generation > generation && self.ready_in(destination),
            "server rejected/cancelled border travel; recent={:?}",
            self.live
                .game
                .chat
                .iter()
                .rev()
                .take(5)
                .map(|line| &line.text)
                .collect::<Vec<_>>()
        );
        let arrival = self.anchor();
        ensure!(
            arrival.iter().any(|v| v.abs() > 10.),
            "used zero-coordinate zone acknowledgment as arrival"
        );
        ensure!(
            arrival
                .into_iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 2.),
            "unexpected arrival {arrival:?}; expected {expected:?}"
        );
        ensure!(
            !self.live.zone_request_pending(),
            "pending state survived handoff"
        );
        println!(
            "ARRIVAL zone={} generation={} own={} scene_anchor={arrival:?} npc_count={} zone_points={}",
            destination,
            self.live.zone_generation,
            self.live.own_id.unwrap(),
            self.live.entities.values().filter(|e| e.spawn.npc).count(),
            self.live.zone_points.len()
        );
        Ok(())
    }
    fn restore(&mut self, pose: &Pose) -> anyhow::Result<()> {
        ensure!(
            pose.instance == 0,
            "nonzero fixture instance needs explicit restoration support"
        );
        self.wait("restore connection ready", 40, |p| {
            p.live.ready && p.live.own_id.is_some() && !p.live.zone_request_pending()
        })?;
        if !self.ready_in(pose.zone) {
            self.say(format!(
                "#zone {} {} {} {}",
                pose.zone, pose.xyz[0], pose.xyz[1], pose.xyz[2]
            ))?;
            self.wait("restore original zone", 40, |p| p.ready_in(pose.zone))?;
        }
        self.goto(pose)?;
        let until = Instant::now() + Duration::from_millis(500);
        while Instant::now() < until {
            self.poll()?;
            std::thread::sleep(Duration::from_millis(20));
        }
        println!("RESTORED_LIVE server_pose={pose:?}");
        Ok(())
    }
}
fn proof(probe: &mut Probe, gfay: &Zone, crushbone: &Zone) -> anyhow::Result<()> {
    probe.wait("initial fixture login", 40, |p| {
        p.live.ready && p.live.own_id.is_some()
    })?;
    if !probe.ready_in(54) {
        probe.say("#zone gfaydark".into())?;
        probe.wait("Greater Faydark setup", 40, |p| {
            p.ready_in(54) && !p.live.zone_points.is_empty()
        })?;
    }
    probe.goto(&Pose {
        zone: 54,
        instance: 0,
        xyz: scene_point_to_server([2606., -55., 19.]),
        heading: 0.,
    })?;
    probe.cross(gfay, 4, 1., 58, [-660., 162., 4.])?;
    probe.cross(crushbone, 1, -1., 54, [2608., -55., 18.5])?;
    println!(
        "ROUND_TRIP gfaydark_exit4_to_crushbone_exit1=verified authored_BSP=verified collision_walk=verified duplicate_suppression=verified"
    );
    Ok(())
}
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .context("zone_travel_smoke GAMEPLAY_CONFIG [EVERQUEST_DIRECTORY]")?,
    );
    let base = args
        .next()
        .map(PathBuf::from)
        .or_else(loader::default_client_dir)
        .context("EverQuest assets")?;
    let config = ConnectionConfig::load(&path)?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_gameplay"
            && config.character == "Mechanic",
        "requires dedicated storage2 Mechanic fixture"
    );
    ensure!(
        query("SELECT ingame FROM character_data WHERE name='Mechanic';")?.trim() == "0",
        "Mechanic is already in use"
    );
    let original = snapshot()?;
    ensure!(
        original.pose.instance == 0,
        "fixture must be in a non-instanced zone"
    );
    let recovery = PathBuf::from(format!(
        "/tmp/openeq-zone-travel-recovery-{}.txt",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&recovery)?;
    writeln!(
        file,
        "Mechanic on storage2.daeken.dev\nOriginal server pose: {:?}\nRecovery GM commands:\n#zone {} {} {} {}\n#goto {} {} {} {}\n\n{}\nRESOURCES\n{}",
        original.pose,
        original.pose.zone,
        original.pose.xyz[0],
        original.pose.xyz[1],
        original.pose.xyz[2],
        original.pose.xyz[0],
        original.pose.xyz[1],
        original.pose.xyz[2],
        original.pose.heading,
        original.stable,
        original.resources
    )?;
    file.sync_all()?;
    println!(
        "ORIGINAL server_pose={:?} recovery={}",
        original.pose,
        recovery.display()
    );
    // Decode before logging in so asset work cannot leave a fixture unattended.
    let gfay = Zone::load(&base, "gfaydark")?;
    let crushbone = Zone::load(&base, "crushbone")?;
    let mut probe = Probe::new(config.clone());
    let result = proof(&mut probe, &gfay, &crushbone);
    if let Err(error) = &result {
        eprintln!("PROOF_FAILED {error:#}");
    }
    let mut restored = probe.restore(&original.pose);
    drop(probe);
    wait_offline()?;
    if restored.is_err() {
        eprintln!("Restoring through a fresh fixture session after: {restored:?}");
        let mut recovery_probe = Probe::new(config);
        restored = recovery_probe.restore(&original.pose);
        drop(recovery_probe);
        wait_offline()?;
    }
    if let Err(error) = &restored {
        eprintln!(
            "RESTORATION_FAILED {error:#}; recovery={}",
            recovery.display()
        );
    }
    restored?;
    let after = snapshot()?;
    ensure!(
        after.pose.zone == original.pose.zone
            && after.pose.instance == original.pose.instance
            && after
                .pose
                .xyz
                .into_iter()
                .zip(original.pose.xyz)
                .all(|(a, b)| (a - b).abs() < 0.02)
            && (after.pose.heading - original.pose.heading).abs() < 0.02,
        "fixture saved pose not restored: {:?}",
        after.pose
    );
    ensure!(
        after.stable == original.stable,
        "fixture stats/currency/inventory changed; recovery={}",
        recovery.display()
    );
    ensure!(
        after.resources == original.resources,
        "fixture HP/mana/endurance changed: before={} after={}",
        original.resources.trim(),
        after.resources.trim()
    );
    println!(
        "FIXTURE restored_pose=verified stats_currency_inventory=unchanged hp_mana_endurance=unchanged offline=verified"
    );
    result?;
    println!("RESULT natural_zone_roundtrip=PASS restoration=PASS");
    Ok(())
}
