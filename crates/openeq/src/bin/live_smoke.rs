//! Reproducible end-to-end NPC render probe against a configured EQEmu world.
use openeq::{hud, live};
use openeq_net::session::ConnectionConfig;
use openeq_render::{Camera, GpuScene, Renderer, actors::ActorRenderer};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq=info,openeq_net=info,openeq_render=info")
        .init();
    let mut args = std::env::args().skip(1);
    let config = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: live_smoke CONFIG OUTPUT.png [SECONDS]"))?;
    let output = PathBuf::from(args.next().unwrap_or_else(|| "/tmp/openeq-live.png".into()));
    let seconds: u64 = args.next().map_or(Ok(40), |value| value.parse())?;
    anyhow::ensure!(
        (1..=180).contains(&seconds),
        "duration must be 1–180 seconds"
    );
    let mut live = live::LiveWorld::start(ConnectionConfig::load(std::path::Path::new(&config))?);
    let mut interaction = openeq::interaction::Interaction {
        inventory_open: true,
        ..Default::default()
    };
    let start = Instant::now();
    while !live.ready || live.environment.is_none() {
        live.poll();
        anyhow::ensure!(live.error.is_none(), "connection error: {:?}", live.error);
        anyhow::ensure!(start.elapsed().as_secs() < 40, "zone connection timed out");
        std::thread::sleep(Duration::from_millis(20));
    }
    let base = openeq_assets::loader::default_client_dir().unwrap();
    live.game.strings = openeq::game::StringTable::load(&base);
    live.game.spell_catalog = openeq::spells::SpellCatalog::load(&base)?;
    interaction.open_bags.extend(
        live.game
            .inventory
            .items
            .values()
            .filter(|item| item.bag_slots > 0)
            .filter_map(|item| item.slot.server_slot().map(|slot| slot as i32)),
    );
    let zone = &live.environment.as_ref().unwrap().short_name;
    let scene = openeq_assets::loader::load_zone(&base, zone)?;
    let mut renderer = Renderer::new_headless(1280, 720)?;
    let env = live.environment.as_ref().unwrap();
    let settings = openeq_render::environment::EnvironmentSettings {
        fog_color: env.fog_color[0],
        fog_start: env.fog_start[0],
        fog_end: env.fog_end[0],
        fog_density: env.fog_density,
        fog_enabled: env.fog_end[0] > env.fog_start[0],
        sky_enabled: !matches!(env.zone_type, 0 | 3 | 4) && env.sky != 0,
        ..Default::default()
    };
    let sky = openeq_assets::environment::load_sky(
        &base,
        zone,
        (live.hour as f32 + live.minute as f32 / 60.) / 24.,
    )
    .ok();
    renderer.set_environment(settings, sky.as_ref());
    let hud = hud::Hud::load(&base)?;
    let collision = openeq_assets::collision::CollisionWorld::build(&scene);
    let scene = GpuScene::build(renderer.device(), renderer.queue(), &scene)?;
    renderer.set_scene(&scene);
    let mut actors = ActorRenderer::load(&base, zone)?;
    let position = live.initial_position.take().unwrap();
    let mut camera = Camera {
        position: [position.x, position.y, position.z + 3.],
        ..Default::default()
    };
    // Consume spawns received while assets loaded. Prefer a walking NPC so the
    // capture exercises tracking as well as texture, pose and ground alignment.
    live.poll();
    let nearest = live
        .entities
        .values()
        .filter(|e| e.spawn.npc && e.spawn.race <= 12)
        .min_by_key(|e| {
            (
                e.spawn.position.animation == 0,
                ((e.spawn.position.x - position.x).powi(2)
                    + (e.spawn.position.y - position.y).powi(2)) as u64,
            )
        })
        .unwrap();
    let target = [
        nearest.spawn.position.x,
        nearest.spawn.position.y,
        nearest.spawn.position.z,
    ];
    camera.position = [target[0] - 14., target[1] - 22., target[2] + 4.];
    camera.yaw = (target[0] - camera.position[0]).atan2(target[1] - camera.position[1]);
    camera.pitch = -0.12;
    println!("Framing {} at {:?}", nearest.spawn.name, target);
    live.set_target(Some(nearest.spawn.id));
    let frame_start = Instant::now();
    let mut previous_motion = std::collections::BTreeMap::new();
    let mut predicted_motion_frames = 0u64;
    let mut last_diagnostic_second = u64::MAX;
    while frame_start.elapsed().as_secs() < seconds {
        live.poll();
        anyhow::ensure!(
            live.error.is_none() && live.ready,
            "live session failed: {:?}",
            live.error
        );
        // Heartbeat uses the actual character's location, not the diagnostic camera.
        let player = Camera {
            position: [position.x, position.y, position.z + 3.],
            ..Default::default()
        };
        live.camera_position(&player, false);
        if let Some(entity) = live.target.and_then(|id| live.entities.get(&id)) {
            let target = entity.position(Instant::now());
            let focus = [target[0], target[1], target[2] + 2.];
            camera.position =
                collision.clip_camera(focus, [focus[0] - 14., focus[1] - 22., focus[2] + 4.], 0.5);
            let delta: [f32; 3] = std::array::from_fn(|i| focus[i] - camera.position[i]);
            camera.yaw = delta[0].atan2(delta[1]);
            camera.pitch = delta[2].atan2(delta[0].hypot(delta[1]));
            let second = frame_start.elapsed().as_secs();
            if second != last_diagnostic_second {
                last_diagnostic_second = second;
                let p = entity.spawn.position;
                println!(
                    "FRAME {second}: authoritative={:?}, predicted={target:?}, camera={:?}, floor_at_authoritative={:?}, floor_at_predicted={:?}, floor_at_camera={:?}, heading={}, animation={}",
                    [p.x, p.y, p.z],
                    camera.position,
                    collision.ground_height(p.x, p.y, p.z, 300., 300.),
                    collision.ground_height(target[0], target[1], target[2], 300., 300.),
                    collision.ground_height(
                        camera.position[0],
                        camera.position[1],
                        camera.position[2],
                        300.,
                        300.
                    ),
                    p.heading,
                    p.animation,
                );
            }
        }
        let states = live.actors(camera.position);
        for actor in &states {
            let position = live.entities[&actor.id].spawn.position;
            let authoritative = [position.x, position.y, position.z];
            if let Some((last_drawn, last_authoritative)) =
                previous_motion.insert(actor.id, (actor.position, authoritative))
                && last_authoritative == authoritative
                && actor
                    .position
                    .iter()
                    .zip(last_drawn)
                    .any(|(a, b)| (a - b).abs() > 0.001)
            {
                predicted_motion_frames += 1;
            }
        }
        actors.update(&renderer, &states, frame_start.elapsed().as_secs_f32());
        let player = live.own_id.and_then(|id| live.entities.get(&id));
        let target = live
            .target
            .and_then(|id| live.entities.get(&id))
            .map(|e| hud::HudTarget {
                name: e
                    .spawn
                    .name
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .replace('_', " "),
                hp: e.spawn.hp_percent as f32 / 100.,
                level: e.spawn.level,
            });
        renderer.set_ui(&hud.gameplay_frame(
            [1280, 720],
            &hud::HudState {
                character: live.character.clone(),
                player_level: player.map_or(0, |e| e.spawn.level),
                hp: player.map_or(0., |e| e.spawn.hp_percent as f32 / 100.),
                mana: live.game.mana.fraction(),
                endurance: live.game.endurance.fraction(),
                target,
                status: format!(
                    "Connected to Storage2 • {}",
                    live.environment.as_ref().unwrap().short_name
                ),
                entities: live.entities.len(),
                movement_updates: live.moves,
            },
            &interaction.view(&live),
        ));
        renderer.render_with_actors(&scene, &camera, &actors.draws());
        std::thread::sleep(Duration::from_millis(50));
    }
    let (w, h, pixels) = renderer.read_rgba().unwrap();
    image::save_buffer(&output, &pixels, w, h, image::ColorType::Rgba8)?;
    println!(
        "LIVE RENDER: {} entities, {} drawn NPCs, {} position changes; {}",
        live.entities.len(),
        actors.rendered_instances,
        live.moves,
        output.display()
    );
    println!("NPC motion continued between packets on {predicted_motion_frames} actor frames");
    anyhow::ensure!(
        actors.rendered_instances > 0 && live.moves > 0 && predicted_motion_frames > 0,
        "NPC render/movement smoke check failed"
    );
    Ok(())
}
