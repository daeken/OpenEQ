//! Production account-controller camp proof. The operator owns fixture snapshot,
//! offline preflight, and guarded restoration. No database writes or audio.
use anyhow::{Context, ensure};
use openeq::{
    account::{AccountController, Action, Endpoint, Ready, Stage},
    live::LiveWorld,
};
use openeq_net::session::ConnectionConfig;
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn poll(controller: &mut AccountController) -> anyhow::Result<Option<Ready>> {
    let ready = controller.poll();
    ensure!(
        controller.view.stage != Stage::Credentials,
        "account session ended: {}",
        controller.view.notice.as_deref().unwrap_or("signed out")
    );
    Ok(ready)
}
fn wait_stage(controller: &mut AccountController, stage: Stage) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        ensure!(poll(controller)?.is_none(), "unexpected zone entry");
        if controller.view.stage == stage {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "account stage timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn enter(controller: &mut AccountController, character: &str) -> anyhow::Result<LiveWorld> {
    ensure!(
        controller
            .view
            .characters
            .iter()
            .any(|row| row.enabled && row.name == character),
        "expected enabled fixture missing from fresh roster"
    );
    ensure!(
        controller.action(
            controller.view.token,
            Action::ChooseCharacter(character.into())
        ),
        "fixture entry rejected"
    );
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut live = loop {
        if let Some(ready) = poll(controller)? {
            ensure!(
                ready.identity.character == character,
                "unexpected character handoff"
            );
            break ready.live;
        }
        ensure!(Instant::now() < deadline, "zone handoff timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    loop {
        live.poll();
        ensure!(live.error.is_none(), "zone connection failed");
        ensure!(poll(controller)?.is_none(), "duplicate live handoff");
        if live.movement_allowed() {
            break;
        }
        ensure!(Instant::now() < deadline, "living ready fixture timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    println!(
        "ENTRY ready=true own_spawn=true entities={} generation={}",
        live.entities.len(),
        live.zone_generation()
    );
    Ok(live)
}
fn cancellation(live: &mut LiveWorld, controller: &mut AccountController) -> anyhow::Result<()> {
    ensure!(live.request_camp(), "camp request rejected");
    ensure!(!live.request_camp(), "duplicate camp accepted");
    let deadline = Instant::now() + Duration::from_secs(8);
    let token = loop {
        live.poll();
        ensure!(
            poll(controller)?.is_none() && controller.view.stage == Stage::Playing,
            "ordinary fixture returned before cancellation"
        );
        let view = live
            .camp_view()
            .context("camp interrupted before cancellation")?;
        if view.seconds.is_some() {
            break view.token;
        }
        ensure!(Instant::now() < deadline, "camp dispatch timed out");
        std::thread::sleep(Duration::from_millis(10));
    };
    ensure!(
        !live.cancel_camp(token.wrapping_add(1)),
        "stale camp cancel accepted"
    );
    ensure!(live.cancel_camp(token), "current camp cancel rejected");
    loop {
        live.poll();
        if live.camp_view().is_none() {
            break;
        }
        ensure!(Instant::now() < deadline, "camp cancellation timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    ensure!(
        live.movement_allowed(),
        "cancel did not restore living controls"
    );
    println!("CANCEL current=true stale=false duplicate=false movement_restored=true");
    Ok(())
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        (3..=4).contains(&args.len()),
        "usage: camp_smoke PRIVATE_CONFIG EXPECTED_CHARACTER [--early-close]"
    );
    let early = args.get(3).is_some_and(|flag| flag == "--early-close");
    ensure!(args.len() == 3 || early, "unknown option");
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    let character = &args[2];
    ensure!(
        config.character == *character,
        "configuration does not match expected fixture"
    );
    let mut controller = AccountController::default();
    controller.sign_in(
        Endpoint {
            host: config.host,
            login_port: config.login_port,
            world_port: config.world_port,
        },
        config.username,
        config.password,
    )?;
    wait_stage(&mut controller, Stage::Worlds)?;
    let world = controller
        .view
        .servers
        .iter()
        .find(|world| world.is_up() && config.server_id.is_none_or(|id| id == world.server_id))
        .context("configured world unavailable")?
        .server_id;
    ensure!(
        controller.action(controller.view.token, Action::ChooseWorld(world)),
        "world selection rejected"
    );
    wait_stage(&mut controller, Stage::Characters)?;
    let first_roster = controller.view.token;
    let mut live = enter(&mut controller, character)?;
    if !early {
        cancellation(&mut live, &mut controller)?;
    }
    let began = Instant::now();
    ensure!(live.request_camp(), "camp request rejected");
    let mut countdown_seen = false;
    loop {
        live.poll();
        countdown_seen |= live.camp_view().is_some_and(|view| view.seconds.is_some());
        ensure!(
            poll(&mut controller)?.is_none(),
            "unexpected entry during return"
        );
        if controller.view.stage == Stage::Characters {
            break;
        }
        ensure!(
            began.elapsed() < Duration::from_secs(75),
            "camp return timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let uncertain = controller
        .view
        .notice
        .as_deref()
        .is_some_and(|notice| notice.contains("Connection closed while camping"));
    ensure!(
        uncertain == early,
        "camp close classification differs from expected fixture behavior"
    );
    if !early {
        ensure!(
            countdown_seen && began.elapsed() >= Duration::from_secs(30),
            "ordinary camp skipped countdown"
        );
    }
    ensure!(
        controller.view.resumed && controller.view.token.revision > first_roster.revision,
        "fresh roster revision missing"
    );
    ensure!(
        !controller.action(first_roster, Action::ChooseCharacter(character.clone())),
        "stale roster entry accepted"
    );
    println!(
        "RETURN elapsed_ms={} fresh_roster=true uncertain_close={} countdown_seen={}",
        began.elapsed().as_millis(),
        uncertain,
        countdown_seen
    );
    drop(live);
    let live = enter(&mut controller, character)?;
    ensure!(
        live.zone_generation() == 0 && live.camp_view().is_none(),
        "reentry inherited old camp state"
    );
    println!("REENTRY ready=true fresh_live=true");
    drop(live); // Existing graceful ready-zone shutdown contract.
    std::thread::sleep(Duration::from_secs(4));
    drop(controller);
    println!("SHUTDOWN requested=true operator_must_verify_offline_and_restore=true");
    Ok(())
}
