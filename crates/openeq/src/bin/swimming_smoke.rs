//! Authenticated swimming proof, restricted to the dedicated Mechanic fixture.
//! Records recovery data before login and restores the original server pose,
//! including when an assertion fails. Never uses Explorer or changes inventory.
use anyhow::{Context, ensure};
use openeq::{
    coordinates::{scene_point_to_server, server_heading_to_scene},
    live::LiveWorld,
    movement::{GroundMotion, MotionInput, MotionMode, MotionWorld},
    movement_rules::PlayerGravity,
};
use openeq_assets::{collision::CollisionWorld, liquid_regions::LiquidRegions, loader};
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
    collision: CollisionWorld,
    liquids: LiquidRegions,
}
impl Zone {
    fn load(base: &Path) -> anyhow::Result<Self> {
        let scene = loader::load_zone(base, "poknowledge")?;
        Ok(Self {
            collision: CollisionWorld::build(&scene),
            liquids: LiquidRegions::load(base, "poknowledge")?,
        })
    }
}
struct Probe {
    live: LiveWorld,
    camera: Camera,
    generation: u64,
    corrections: u64,
}
impl Probe {
    fn new(config: ConnectionConfig) -> Self {
        Self {
            live: LiveWorld::start(config),
            camera: Camera::default(),
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
    fn feet(&self) -> [f32; 3] {
        [
            self.camera.position[0],
            self.camera.position[1],
            self.camera.position[2] - 6.,
        ]
    }
    fn settle(&mut self, milliseconds: u64) -> anyhow::Result<()> {
        let until = Instant::now() + Duration::from_millis(milliseconds);
        while Instant::now() < until {
            self.poll()?;
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
    fn saved_position(&mut self, label: &str) -> anyhow::Result<()> {
        self.settle(350)?;
        let marker = "successfully saved";
        let before = self
            .live
            .game
            .chat
            .iter()
            .filter(|line| line.text.contains(marker))
            .count();
        self.live.set_target(self.live.own_id);
        self.say("#save".into())?;
        self.wait("server save acknowledgment", 8, |p| {
            p.live
                .game
                .chat
                .iter()
                .filter(|line| line.text.contains(marker))
                .count()
                > before
        })?;
        let row = query("SELECT zone_id,x,y,z FROM character_data WHERE name='Mechanic';")?;
        let fields: Vec<_> = row.trim().split('\t').collect();
        ensure!(
            fields.len() == 4 && fields[0] == "202",
            "swim fixture left PoK"
        );
        let server: [f32; 3] = [fields[1].parse()?, fields[2].parse()?, fields[3].parse()?];
        let expected = scene_point_to_server(self.anchor());
        ensure!(
            server
                .into_iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 0.15),
            "server rejected {label} movement: saved={server:?}, sent={expected:?}"
        );
        self.live.set_target(None);
        println!("SERVER_POSITION phase={label} saved_center={server:?} accepted=verified");
        Ok(())
    }
    fn swim(
        &mut self,
        zone: &Zone,
        motion: &mut GroundMotion,
        frames: usize,
        velocity: [f32; 3],
    ) -> anyhow::Result<(f32, bool)> {
        let generation = self.live.zone_generation;
        let corrections = self.corrections;
        let mut highest = self.feet()[2];
        let mut dry_eyes = false;
        for _ in 0..frames {
            self.poll()?;
            ensure!(
                self.ready_in(202)
                    && self.live.zone_generation == generation
                    && self.live.movement_allowed(),
                "swimming zone/session became unusable"
            );
            ensure!(
                self.corrections == corrections,
                "unexpected server movement correction during swimming"
            );
            ensure!(
                self.live.player_gravity() == PlayerGravity::Grounded,
                "fixture has nonstandard server gravity/buffs"
            );
            let feet = motion.step_in_world(
                MotionWorld {
                    collision: &zone.collision,
                    dynamic: None,
                    liquids: Some(&zone.liquids),
                },
                self.feet(),
                MotionInput {
                    walk_velocity: [velocity[0], velocity[1]],
                    volume_velocity: velocity,
                    jump: false,
                    gravity: self.live.player_gravity(),
                },
                1. / 60.,
            );
            self.camera.position = [feet[0], feet[1], feet[2] + 6.];
            self.live.camera_position(&self.camera, velocity != [0.; 3]);
            highest = highest.max(feet[2]);
            dry_eyes |= zone.liquids.at(self.camera.position).is_none();
            std::thread::sleep(Duration::from_secs_f32(1. / 60.));
        }
        Ok((highest, dry_eyes))
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
fn proof(probe: &mut Probe, zone: &Zone) -> anyhow::Result<()> {
    probe.wait("initial fixture login", 40, |p| {
        p.live.ready && p.live.own_id.is_some()
    })?;
    if !probe.ready_in(202) {
        probe.say("#zone poknowledge".into())?;
        probe.wait("Plane of Knowledge setup", 40, |p| p.ready_in(202))?;
    }
    // GM relocation is setup only. Every proof displacement below goes through
    // production fixed-step movement, real collision, and ordinary position packets.
    probe.goto(&Pose {
        zone: 202,
        instance: 0,
        xyz: scene_point_to_server([15., 1455., -131.]),
        heading: 0.,
    })?;
    ensure!(
        zone.liquids.at(probe.anchor()).is_some()
            && zone.liquids.at(probe.camera.position).is_some(),
        "pool center/eye fixture is dry"
    );
    let mut motion = GroundMotion::default();
    let start = probe.feet();
    probe.swim(zone, &mut motion, 60, [0.; 3])?;
    ensure!(
        (probe.feet()[2] - start[2]).abs() < 0.025 && motion.mode == MotionMode::Swimming,
        "idle swimmer sank: {:?}",
        probe.feet()
    );
    println!("SWIM idle_depth_hold=verified feet={:?}", probe.feet());
    probe.swim(zone, &mut motion, 30, [-40., 0., 0.])?;
    ensure!(
        (probe.feet()[0] - 3.).abs() < 0.05 && (probe.feet()[2] + 134.).abs() < 0.025,
        "horizontal swim did not preserve depth: {:?}",
        probe.feet()
    );
    probe.saved_position("horizontal")?;
    let (highest, dry_eyes) = probe.swim(zone, &mut motion, 60, [0., 0., 40.])?;
    ensure!(
        dry_eyes && highest > -132. && highest < -124.,
        "ascent failed to reach authored surface: highest={highest} dry_eyes={dry_eyes}"
    );
    println!(
        "SWIM ascent_surface=verified highest_feet_z={highest:.3} final_feet={:?}",
        probe.feet()
    );
    probe.saved_position("ascent")?;
    probe.swim(zone, &mut motion, 60, [0., 0., -40.])?;
    ensure!(
        (probe.feet()[2] + 134.).abs() < 0.025
            && motion.mode == MotionMode::Swimming
            && zone.liquids.at(probe.camera.position).is_some(),
        "descent did not stop at pool floor: {:?}",
        probe.feet()
    );
    probe.saved_position("descent")?;
    let npc_count = probe.live.entities.values().filter(|e| e.spawn.npc).count();
    ensure!(
        npc_count > 0 && probe.live.movement_allowed(),
        "zone no longer usable after swim"
    );
    println!(
        "SWIM descent_floor=verified zone_usable=verified npc_count={npc_count} damage_or_breath_simulation=none"
    );
    Ok(())
}
fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .context("swimming_smoke GAMEPLAY_CONFIG [EVERQUEST_DIRECTORY]")?,
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
        "/tmp/openeq-swimming-recovery-{}.txt",
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
    let zone = Zone::load(&base)?;
    let mut probe = Probe::new(config.clone());
    let result = proof(&mut probe, &zone);
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
    println!("RESULT live_swimming=PASS restoration=PASS");
    Ok(())
}
