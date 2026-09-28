//! Live Kelethin button/lift proof, restricted to the dedicated Broker fixture.
use anyhow::{Context, ensure};
use openeq::{
    coordinates::{scene_point_to_server, server_point_to_scene},
    game::StringTable,
    interaction::Interaction,
    live::LiveWorld,
    movement::GroundMotion,
};
use openeq_assets::{collision::CollisionWorld, loader, mesh::VERTEX_STRIDE};
use openeq_net::{gameplay::Command, session::ConnectionConfig};
use openeq_render::{
    Camera, Renderer,
    doors::{DoorRenderer, DoorState},
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

struct Probe {
    live: LiveWorld,
    ui: Interaction,
}
impl Probe {
    fn poll(&mut self) -> anyhow::Result<()> {
        self.live.poll();
        ensure!(
            self.live.error.is_none(),
            "connection failed: {:?}",
            self.live.error
        );
        if let Some(p) = self.live.initial_position.take() {
            self.live.camera_position(
                &Camera {
                    position: [p.x, p.y, p.z + 3.],
                    yaw: p.heading * std::f32::consts::TAU / 512.,
                    ..Default::default()
                },
                false,
            );
        }
        self.ui.tick(&mut self.live);
        Ok(())
    }
    fn wait(
        &mut self,
        label: &str,
        seconds: u64,
        condition: impl Fn(&LiveWorld) -> bool,
    ) -> anyhow::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.poll()?;
            if condition(&self.live) {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "timeout {label}; recent chat: {:?}",
                self.live
                    .game
                    .chat
                    .iter()
                    .rev()
                    .take(5)
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    fn say(&mut self, text: &str) {
        let position = self.live.player_position().unwrap_or([0.; 3]);
        self.ui
            .submit(&format!("/say {text}"), &mut self.live, position);
    }
    fn zone(&mut self, name: &str) -> anyhow::Result<()> {
        if self.live.ready
            && self
                .live
                .environment
                .as_ref()
                .is_some_and(|e| e.short_name == name)
        {
            return Ok(());
        }
        self.say(&format!("#zone {name}"));
        self.wait("zone handoff and doors", 35, |live| {
            live.ready
                && live
                    .environment
                    .as_ref()
                    .is_some_and(|e| e.short_name == name)
                && !live.doors.is_empty()
        })
    }
    fn goto(&mut self, scene: [f32; 3]) -> anyhow::Result<()> {
        let server = scene_point_to_server(scene);
        self.say(&format!("#goto {} {} {}", server[0], server[1], server[2]));
        self.wait("authoritative position correction", 8, |live| {
            live.player_position()
                .is_some_and(|p| p.into_iter().zip(scene).all(|(a, b)| (a - b).abs() < 0.2))
        })
    }
    fn state(&self, id: u8) -> anyhow::Result<DoorState> {
        let door = self
            .live
            .doors
            .get(&id)
            .with_context(|| format!("missing door {id}"))?;
        Ok(DoorState {
            id: door.id,
            name: door.name.clone(),
            position: door.position,
            heading: door.heading,
            incline: door.incline,
            size: door.size,
            open_type: door.open_type,
            state: door.state,
            inverted: door.inverted,
            parameter: door.parameter,
        })
    }
    fn click(&mut self, button: u8, lift: u8) -> anyhow::Result<(DoorState, DoorState)> {
        let switch = self.state(button)?;
        self.goto([
            switch.position[0],
            switch.position[1],
            switch.position[2] + 3.125,
        ])?;
        let before = self.state(lift)?;
        ensure!(
            self.live.command(Command::ClickDoor {
                door_id: button,
                player_id: self.live.own_id.context("own spawn")?
            }),
            "door click submission failed"
        );
        self.wait("button and linked lift transition", 4, |live| {
            live.doors
                .get(&button)
                .is_some_and(|door| door.state != switch.state)
                && live
                    .doors
                    .get(&lift)
                    .is_some_and(|door| door.state != before.state)
        })?;
        let after = self.state(lift)?;
        println!(
            "actual button {button} -> lift {lift}: state {} -> {}, travel {}",
            before.state, after.state, after.parameter
        );
        Ok((before, after))
    }
    fn restore(&mut self) -> anyhow::Result<()> {
        self.zone("poknowledge")?;
        self.goto(server_point_to_scene([934., -305., -92.875]))?;
        let start = Instant::now();
        self.wait("restoration heartbeat", 3, |_| {
            start.elapsed() > Duration::from_secs(1)
        })?;
        println!("Broker restored to PoK server center [934,-305,-92.875]");
        Ok(())
    }
}

/// A centroid strictly inside the largest horizontal lift triangle gives a
/// reproducible rider location on the original mesh, away from its edges.
fn support_point(base: &Path) -> anyhow::Result<[f32; 3]> {
    let library = loader::load_object_library(base, "gfaydark")?;
    let scene = library.object_model("FAYLEVATOR")?;
    let mut best = None;
    for mesh in scene.meshes.iter().filter(|mesh| mesh.collidable) {
        for triangle in mesh.indices.chunks_exact(3) {
            let p: [[f32; 3]; 3] = std::array::from_fn(|i| {
                let start = triangle[i] as usize * VERTEX_STRIDE;
                mesh.vertices[start..start + 3].try_into().unwrap()
            });
            if (p[0][2] - p[1][2]).abs() > 0.001 || (p[0][2] - p[2][2]).abs() > 0.001 {
                continue;
            }
            // Positive Z winding selects the upper platform, not its underside.
            let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1])
                - (p[1][1] - p[0][1]) * (p[2][0] - p[0][0]);
            if area <= 0. {
                continue;
            }
            let center = std::array::from_fn(|i| (p[0][i] + p[1][i] + p[2][i]) / 3.);
            if best.as_ref().is_none_or(|(old, _)| area > *old) {
                best = Some((area, center));
            }
        }
    }
    best.filter(|(area, _)| *area > 1.)
        .map(|(_, p)| p)
        .context("original lift has no support triangle")
}
fn rider_proof(
    renderer: &Renderer,
    doors: &mut DoorRenderer,
    local: [f32; 3],
    before: &DoorState,
    after: &DoorState,
    clock: &mut f32,
) -> anyhow::Result<()> {
    doors.update(renderer, std::slice::from_ref(before), *clock);
    let angle = std::f32::consts::FRAC_PI_2 - before.heading * std::f32::consts::TAU / 512.;
    let (s, c) = angle.sin_cos();
    let size = if before.size == 0 {
        1.
    } else {
        before.size as f32 / 100.
    };
    let mut feet = [
        before.position[0] + (c * local[0] - s * local[1]) * size,
        before.position[1] + (s * local[0] + c * local[1]) * size,
        before.position[2]
            + local[2] * size
            + f32::from(before.state != 0) * before.parameter as f32,
    ];
    feet[2] = doors
        .collision_world()
        .ground_height(feet[0], feet[1], feet[2], 0.1, 0.1)
        .context("original lift does not support rider")?;
    let start = feet;
    *clock += 1.;
    doors.update(renderer, std::slice::from_ref(after), *clock);
    let duration = (after.parameter as i32 as f32).abs().max(20.) / 25.;
    let expected = after.parameter as i32 as f32
        * (f32::from(after.state != 0) - f32::from(before.state != 0));
    let mut carried = 0.;
    let mut motion = GroundMotion::default();
    let empty = CollisionWorld::default();
    for step in 1..=120 {
        doors.update(
            renderer,
            std::slice::from_ref(after),
            *clock + duration * step as f32 / 120.,
        );
        let delta = doors.take_platform_displacement(feet, true);
        ensure!(
            delta[0].abs() < 0.001 && delta[1].abs() < 0.001,
            "lift moved horizontally"
        );
        ensure!(
            doors.take_platform_displacement(feet, true) == [0.; 3],
            "rider carried twice for one frame"
        );
        carried += delta[2];
        feet[2] += delta[2];
        feet = motion.step_with_dynamic(
            &empty,
            Some(doors.collision_world()),
            feet,
            [0.; 2],
            false,
            duration / 120.,
        );
        ensure!(
            (feet[2] - start[2] - carried).abs() < 0.03,
            "rider fell through animated lift"
        );
        ensure!(
            doors
                .collision_world()
                .ground_height(feet[0], feet[1], feet[2], 0.06, 0.06)
                .is_some(),
            "animated collision lost support"
        );
    }
    ensure!(
        (carried - expected).abs() < 0.03,
        "wrong rider travel: {carried}, expected {expected}"
    );
    *clock += duration + 1.;
    println!(
        "lift {} original-mesh collision/rider: Z {} -> {}, displacement {carried}, 120 supported physics samples",
        before.id, start[2], feet[2]
    );
    Ok(())
}
fn proof(probe: &mut Probe, base: &Path) -> anyhow::Result<()> {
    let renderer = Renderer::new_headless(640, 480)?;
    let mut doors = DoorRenderer::load(base, "gfaydark")?;
    let support = support_point(base)?;
    probe.zone("gfaydark")?;
    let mut clock = 0.;
    // Each pair exercises both independently authored buttons. The first
    // starts travel; the other toggles it back before the server's 5s timer.
    for (lift, lower, upper, distance) in [(69, 73, 74, 68), (77, 79, 78, 98), (80, 81, 82, 69)] {
        let state = probe.state(lift)?;
        ensure!(
            state.name.eq_ignore_ascii_case("FAYLEVATOR")
                && state.open_type == 59
                && state.inverted
                && state.parameter == distance,
            "unexpected lift metadata"
        );
        for button in [lower, upper] {
            let (before, after) = probe.click(button, lift)?;
            rider_proof(&renderer, &mut doors, support, &before, &after, &mut clock)?;
        }
        // Both buttons also use type 59. EQEmu resets them after five seconds
        // without a closing packet, so wait for their local return before reuse.
        probe.wait("button timer reset", 8, |live| {
            [lower, upper, lift]
                .into_iter()
                .all(|id| live.doors.get(&id).is_some_and(|door| door.state == 0))
        })?;
        let (before, opened) = probe.click(lower, lift)?;
        let opened_at = Instant::now();
        rider_proof(&renderer, &mut doors, support, &before, &opened, &mut clock)?;
        probe.wait("automatic lift return without another click", 8, |live| {
            live.doors.get(&lift).is_some_and(|door| door.state == 0)
        })?;
        let elapsed = opened_at.elapsed().as_secs_f32();
        ensure!(
            (4.7..7.).contains(&elapsed),
            "lift returned at an unexpected time: {elapsed}s"
        );
        let returned = probe.state(lift)?;
        rider_proof(
            &renderer, &mut doors, support, &opened, &returned, &mut clock,
        )?;
        println!(
            "lift {lift} and button {lower} automatically returned after {elapsed:.2}s without another click"
        );
        // The exact same button must work again after the server silently
        // resets its open state. It sends OPEN again, not an opposite action.
        for button in [lower, upper] {
            let (before, after) = probe.click(button, lift)?;
            rider_proof(&renderer, &mut doors, support, &before, &after, &mut clock)?;
        }
        println!("lift {lift} repeated button {lower} after automatic return: verified");
    }
    Ok(())
}
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq=info,openeq_net=info,openeq_render=info")
        .init();
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .context("lift_smoke CONFIG [EVERQUEST_DIRECTORY]")?,
    );
    let base = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest"));
    let config = ConnectionConfig::load(&path)?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_commerce"
            && config.character == "Broker",
        "requires dedicated Broker fixture"
    );
    let mut probe = Probe {
        live: LiveWorld::start(config),
        ui: Interaction::default(),
    };
    probe.live.game.strings = StringTable::load(&base);
    probe.wait("initial zone", 25, |live| {
        live.ready && live.own_id.is_some() && !live.doors.is_empty()
    })?;
    let result = proof(&mut probe, &base);
    // Restore even when a proof fails; report the original failure separately.
    let restored = probe.restore();
    drop(probe);
    std::thread::sleep(Duration::from_secs(2));
    if let Err(error) = &restored {
        eprintln!("RESTORATION FAILED: {error:#}");
    }
    result?;
    restored?;
    println!(
        "RESULT gfaydark_lifts=verified buttons_73_74_to_69=verified buttons_78_79_to_77=verified buttons_81_82_to_80=verified automatic_return=verified repeated_buttons=verified animated_collision=verified rider_roundtrip=verified fixture_restored=verified"
    );
    Ok(())
}
