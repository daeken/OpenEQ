//! Read-only live NPC bearing audit; login restricted to the dedicated Barterer fixture.
use anyhow::{Context, ensure};
use openeq::coordinates::server_to_scene;
use openeq_net::{
    session::ConnectionConfig,
    zone::{Spawn, ZoneEvent},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::{Duration, Instant},
};

struct Observed {
    spawn: Spawn,
    at: Instant,
}
struct Sample {
    id: u32,
    name: String,
    race: u32,
    heading: f32,
    scene_heading: f32,
    delta: [f32; 2],
    dot: f32,
    scene_dot: f32,
}
fn landmark(name: &str) -> bool {
    [
        "Dogle_Pitt",
        "Amile_Pitt",
        "Savage_Lord_Etherat",
        "Oracle_Maeth",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix))
}
fn dot(delta: [f32; 2], heading: f32) -> f32 {
    let angle = heading * std::f32::consts::TAU / 512.;
    (delta[0] * angle.sin() + delta[1] * angle.cos()) / delta[0].hypot(delta[1])
}
fn turn(a: f32, b: f32) -> f32 {
    ((a - b + 256.).rem_euclid(512.) - 256.).abs()
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("npc_facing_probe TRADE1_CONFIG [SECONDS]")?;
    let seconds = std::env::args()
        .nth(2)
        .map(|s| s.parse::<u64>())
        .transpose()?
        .unwrap_or(60)
        .clamp(10, 180);
    let config = ConnectionConfig::load(Path::new(&path))?;
    ensure!(
        config.host == "storage2.daeken.dev"
            && config.username == "openeq_trade1"
            && config.character == "Barterer",
        "requires dedicated Barterer fixture"
    );
    let mut zone = config.connect().await?;
    let mut seen = BTreeMap::<u32, Observed>::new();
    let mut samples = Vec::<Sample>::new();
    let mut moving = BTreeSet::new();
    let mut updates = 0;
    let mut rejected = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(seconds);
    // Observe without sending player movement. Own-spawn Z can contain an
    // appearance offset, so echoing it would move this fixture on each login.
    loop {
        tokio::select! {
            _=tokio::time::sleep_until(deadline)=>break,
            event=zone.next_event()=>match event? {
                ZoneEvent::Spawn(spawn)=> {
                    if landmark(&spawn.name) {
                        let p=spawn.position;
                        let s=server_to_scene(p);
                        println!("SPAWN_POSE name={} race={} class={} server=({:.3},{:.3},{:.3}) heading={:.3} scene=({:.3},{:.3},{:.3}) heading={:.3}",spawn.name,spawn.race,spawn.class,p.x,p.y,p.z,p.heading,s.x,s.y,s.z,s.heading);
                    }
                    seen.insert(spawn.id,Observed {spawn,at:Instant::now()});
                }
                ZoneEvent::Movement {id,position}=> {
                    let now=Instant::now();
                    if let Some(old)=seen.get_mut(&id) {
                        let prev=old.spawn.position;
                        if old.spawn.npc && !old.spawn.is_corpse {
                            updates+=1;
                            let delta=[position.x-prev.x,position.y-prev.y];
                            let distance=delta[0].hypot(delta[1]);
                            let age=now.duration_since(old.at).as_secs_f32();
                            if distance>0.5 { moving.insert(id); }
                            if (0.5..=120.).contains(&distance) && (0.05..=12.).contains(&age)
                                && turn(prev.heading,position.heading)<8. && prev.delta_heading.abs()<0.05 && position.delta_heading.abs()<0.05 {
                                let scene=server_to_scene(prev);
                                let next=server_to_scene(position);
                                samples.push(Sample {id,name:old.spawn.name.clone(),race:old.spawn.race,heading:prev.heading,
                                    scene_heading:scene.heading,delta,dot:dot(delta,prev.heading),scene_dot:dot([next.x-scene.x,next.y-scene.y],scene.heading)});
                            } else if distance>0.5 { rejected+=1; }
                        }
                        old.spawn.position=position; old.at=now;
                    }
                }
                ZoneEvent::Despawn(id)=>{seen.remove(&id);}
                _=>{}
            }
        }
    }
    zone.logout().await?;
    for npc in seen.values().filter(|npc| landmark(&npc.spawn.name)) {
        let p = npc.spawn.position;
        let s = server_to_scene(p);
        println!(
            "FINAL_POSE name={} race={} gender={} model={:?} server=({:.3},{:.3},{:.3}) heading={:.3} scene=({:.3},{:.3},{:.3}) heading={:.3} animation={} turn={:.3}",
            npc.spawn.name,
            npc.spawn.race,
            npc.spawn.gender,
            openeq_assets::character::race_model_code(npc.spawn.race, npc.spawn.gender),
            p.x,
            p.y,
            p.z,
            p.heading,
            s.x,
            s.y,
            s.z,
            s.heading,
            p.animation,
            p.delta_heading
        );
    }
    if std::env::args().any(|arg| arg == "--walls") {
        let base = openeq_assets::loader::default_client_dir().context("original assets")?;
        let scene = openeq_assets::loader::load_zone(&base, "poknowledge")?;
        let collision = openeq_assets::collision::CollisionWorld::build(&scene);
        for npc in seen.values().filter(|npc| landmark(&npc.spawn.name)) {
            let p = server_to_scene(npc.spawn.position);
            let angle = p.heading * std::f32::consts::TAU / 512.;
            for height in [0., 2., 4.] {
                let origin = [p.x, p.y, p.z + height];
                for (label, delta) in [
                    ("forward", 0.),
                    ("back", std::f32::consts::PI),
                    ("left", -std::f32::consts::FRAC_PI_2),
                    ("right", std::f32::consts::FRAC_PI_2),
                ] {
                    let a = angle + delta;
                    let end = [
                        origin[0] + a.sin() * 60.,
                        origin[1] + a.cos() * 60.,
                        origin[2],
                    ];
                    let hit = collision.clip_camera(origin, end, 0.);
                    println!(
                        "ROOM_RAY name={} height_above_center={} dir={label} distance={:.3} end=({:.3},{:.3},{:.3})",
                        npc.spawn.name,
                        height,
                        (hit[0] - origin[0]).hypot(hit[1] - origin[1]),
                        hit[0],
                        hit[1],
                        hit[2]
                    );
                }
            }
        }
    }
    for sample in &samples {
        println!(
            "SAMPLE id={} name={} race={} server_h={:.2} scene_h={:.2} delta_server=({:.3},{:.3}) dot={:.6} scene_dot={:.6} error_deg={:.3}",
            sample.id,
            sample.name,
            sample.race,
            sample.heading,
            sample.scene_heading,
            sample.delta[0],
            sample.delta[1],
            sample.dot,
            sample.scene_dot,
            sample.dot.clamp(-1., 1.).acos().to_degrees()
        );
    }
    ensure!(
        !samples.is_empty(),
        "no stable NPC movement pairs in {seconds}s ({updates} updates, {} moving IDs, {rejected} rejected pairs)",
        moving.len()
    );
    let mut errors: Vec<_> = samples
        .iter()
        .map(|s| s.dot.clamp(-1., 1.).acos().to_degrees())
        .collect();
    errors.sort_by(f32::total_cmp);
    let aligned = samples.iter().filter(|s| s.dot > 0.9).count();
    let reverse = samples.iter().filter(|s| s.dot < -0.9).count();
    let ids: BTreeSet<_> = samples.iter().map(|s| s.id).collect();
    println!(
        "RESULT seconds={seconds} updates={updates} moving={} stable_pairs={} stable_npcs={} aligned_dot_gt_09={aligned} reversed_dot_lt_neg09={reverse} median_error_deg={:.3} p95_error_deg={:.3} mean_dot={:.6} rejected_pairs={rejected}",
        moving.len(),
        samples.len(),
        ids.len(),
        errors[errors.len() / 2],
        errors[(errors.len() * 95 / 100).min(errors.len() - 1)],
        samples.iter().map(|s| s.dot as f64).sum::<f64>() / samples.len() as f64
    );
    Ok(())
}
