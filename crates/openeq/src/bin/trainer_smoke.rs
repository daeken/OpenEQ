//! Production trainer proof for the explicitly prepared Barterer fixture.
//! The operator owns offline snapshot, seeding, SQL checks and restoration.
//! This probe sends no movement, database commands, combat, or audio.
use anyhow::{Context, ensure};
use openeq::{
    commerce::total_copper,
    interaction::Interaction,
    live::{LiveWorld, training::clean_trainer_name},
    training_ui::{TrainingAction, TrainingActionKind},
};
use openeq_net::{session::ConnectionConfig, training::Selection};
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::{Duration, Instant},
};

const CHARACTER: &str = "Barterer";
const TRAINER: &str = "Warlord_Welorf";

fn poll(live: &mut LiveWorld, interaction: &mut Interaction) -> anyhow::Result<()> {
    live.poll();
    interaction.tick(live);
    ensure!(live.error.is_none(), "zone connection failed");
    ensure!(live.character == CHARACTER, "unexpected active character");
    ensure!(
        !live.game.attack && live.game.casting.is_none(),
        "unexpected combat or cast state"
    );
    Ok(())
}
fn wait(
    live: &mut LiveWorld,
    interaction: &mut Interaction,
    label: &str,
    predicate: impl Fn(&LiveWorld) -> bool,
) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        poll(live, interaction)?;
        if predicate(live) {
            return Ok(());
        }
        ensure!(Instant::now() < until, "timed out waiting for {label}");
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn verify(live: &LiveWorld, value: u32, points: u32, copper: u64) -> anyhow::Result<()> {
    ensure!(
        live.movement_allowed() && live.game.commerce.currency_ready,
        "fixture is not living with a current balance"
    );
    ensure!(
        live.game.progression.skills[0] == Some(value),
        "unexpected received skill value"
    );
    ensure!(
        total_copper(live.game.currency) == copper,
        "unexpected carried balance"
    );
    let estimate = live
        .training_state()
        .estimate()
        .context("missing trainer balance estimate")?;
    ensure!(
        estimate.training_points == points && estimate.carried_copper == copper,
        "unexpected trainer estimate"
    );
    Ok(())
}
fn operator_gate(
    proof: &Path,
    step: u32,
    live: &mut LiveWorld,
    interaction: &mut Interaction,
) -> anyhow::Result<()> {
    let ready = proof.join(format!("purchase-{step}.ready"));
    let resume = proof.join(format!("continue-after-{step}"));
    let report = live
        .training_state()
        .last_report()
        .context("missing matched trainer report")?;
    let estimate = report
        .estimate_after
        .context("purchase estimate unavailable")?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(ready)?;
    writeln!(
        file,
        "step={step} skill={} assessed_cost={} points_estimate={} carried_estimate={}",
        report.received_value.unwrap_or(0),
        report.completion.assessed_cost_copper,
        estimate.training_points,
        estimate.carried_copper
    )?;
    file.sync_all()?;
    println!(
        "PURCHASE step={step} skill={} assessed_cost={} points_estimate={} carried_estimate={} waiting_for_operator=true",
        report.received_value.unwrap_or(0),
        report.completion.assessed_cost_copper,
        estimate.training_points,
        estimate.carried_copper
    );
    let until = Instant::now() + Duration::from_secs(600);
    loop {
        poll(live, interaction)?;
        ensure!(
            live.training_state().blocked().is_none(),
            "trainer state became uncertain during operator check"
        );
        if resume.is_file() {
            return Ok(());
        }
        ensure!(
            Instant::now() < until,
            "operator checkpoint timed out after purchase {step}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 3 || (args.len() == 4 && args[3] == "--verify-persistence"),
        "usage: trainer_smoke PRIVATE_CONFIG PRIVATE_PROOF_DIRECTORY [--verify-persistence]"
    );
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    ensure!(
        config.character == CHARACTER,
        "trainer proof is pinned to Barterer"
    );
    let proof = Path::new(&args[2]);
    ensure!(proof.is_dir(), "private proof directory must already exist");
    if args.len() == 4 {
        println!(
            "PERSISTENCE_ONLY training_requests=false operator_must_verify_offline_before_login=true"
        );
        return verify_persistence(config);
    }
    for name in [
        "purchase-1.ready",
        "purchase-2.ready",
        "continue-after-1",
        "continue-after-2",
    ] {
        ensure!(
            !proof.join(name).exists(),
            "stale trainer checkpoint marker exists"
        );
    }
    let mut live = LiveWorld::start(config.clone());
    let mut interaction = Interaction::default();
    wait(
        &mut live,
        &mut interaction,
        "fresh fixture profile and own spawn",
        |live| {
            live.movement_allowed()
                && live.game.profile.is_some()
                && live.training_state().estimate().is_some()
        },
    )?;
    verify(&live, 55, 2, 100_000)?;
    let profile = live.game.profile.as_ref().unwrap();
    ensure!(
        profile.name == CHARACTER && profile.level == 10 && profile.class == 1,
        "unexpected fixture identity, level or class"
    );
    ensure!(
        profile.training_points == 2 && profile.skills[0] == 55,
        "unexpected original profile baseline"
    );
    ensure!(
        (43..=47).all(|id| profile.skills.get(id).is_some_and(|value| *value <= 50)),
        "fixture specialization baseline could trigger repair"
    );
    wait(
        &mut live,
        &mut interaction,
        "pinned nearby trainer",
        |live| {
            live.entities.values().any(|entity| {
                entity.spawn.name.starts_with(TRAINER)
                    && clean_trainer_name(&entity.spawn.name) == "Warlord Welorf"
                    && live.training_available(entity.spawn.id)
            })
        },
    )?;
    let trainers: Vec<_> = live
        .entities
        .values()
        .filter(|entity| {
            entity.spawn.name.starts_with(TRAINER)
                && clean_trainer_name(&entity.spawn.name) == "Warlord Welorf"
                && live.training_available(entity.spawn.id)
        })
        .map(|entity| entity.spawn.id)
        .collect();
    ensure!(
        trainers.len() == 1,
        "pinned trainer is ambiguous or out of range"
    );
    live.target = Some(trainers[0]);
    let position = live.player_position().context("missing player position")?;
    interaction.submit("/train", &mut live, position);
    wait(&mut live, &mut interaction, "trainer open reply", |live| {
        live.training_state().trainer().is_some()
    })?;
    ensure!(
        live.training_state().trainer().unwrap().clean_name == "Warlord Welorf",
        "wrong trainer replied"
    );
    ensure!(
        interaction
            .view(&live)
            .training
            .is_some_and(|view| view.open),
        "production trainer window did not open"
    );
    println!(
        "OPEN character=Barterer trainer=Warlord_Welorf skill=55 practices=2 carried=100000 production_interaction=true"
    );

    for (step, value, cost, copper, points) in [(1, 56, 911, 99_089, 1), (2, 57, 973, 98_116, 0)] {
        let revision = interaction
            .view(&live)
            .training
            .context("trainer panel missing")?
            .revision;
        interaction.training_action(
            &mut live,
            TrainingAction {
                revision,
                kind: TrainingActionKind::Select { wire_id: 0 },
            },
        );
        let view = interaction
            .view(&live)
            .training
            .context("trainer selection missing")?;
        ensure!(
            view.train_enabled && view.selected_wire_id == Some(0),
            "production Train action is disabled"
        );
        let action = TrainingAction {
            revision: view.revision,
            kind: TrainingActionKind::Train { wire_id: 0 },
        };
        interaction.training_action(&mut live, action.clone());
        interaction.training_action(&mut live, action); // Same stamped click must not send twice.
        ensure!(
            live.training_state().pending().is_some(),
            "training was not queued through production interaction"
        );
        wait(
            &mut live,
            &mut interaction,
            "matching skill and completion",
            |live| {
                live.training_state().last_report().is_some_and(|report| {
                    report.received_value == Some(value) && report.estimate_after.is_some()
                })
            },
        )?;
        verify(&live, value, points, copper)?;
        let report = live.training_state().last_report().unwrap();
        ensure!(
            report.completion.assessed_cost_copper == cost,
            "unexpected server-assessed cost"
        );
        ensure!(
            live.training_state().estimate().unwrap().matched_purchases == u64::from(step),
            "duplicate or missing purchase"
        );
        ensure!(
            live.game.profile.as_ref().unwrap().training_points == 2,
            "profile snapshot was optimistically rewritten"
        );
        operator_gate(proof, step, &mut live, &mut interaction)?;
        verify(&live, value, points, copper)?;
    }
    ensure!(
        live.training_preview(Selection::new(0, 0).unwrap())
            .is_err(),
        "zero-practice training remained enabled"
    );
    let revision = interaction
        .view(&live)
        .training
        .context("trainer panel missing before close")?
        .revision;
    interaction.training_action(
        &mut live,
        TrainingAction {
            revision,
            kind: TrainingActionKind::Close,
        },
    );
    ensure!(
        live.training_state().active_trainer().is_none(),
        "trainer session did not close"
    );
    drop(live);
    std::thread::sleep(Duration::from_secs(4));
    println!("LOGOUT first_session=true reconnecting_for_persistence=true");

    verify_persistence(config)
}

fn verify_persistence(config: ConnectionConfig) -> anyhow::Result<()> {
    let mut live = LiveWorld::start(config);
    let mut interaction = Interaction::default();
    wait(
        &mut live,
        &mut interaction,
        "reconnected authoritative profile",
        |live| {
            live.movement_allowed()
                && live.game.profile.is_some()
                && live.training_state().estimate().is_some()
        },
    )?;
    verify(&live, 57, 0, 98_116)?;
    let profile = live.game.profile.as_ref().unwrap();
    ensure!(
        profile.name == CHARACTER
            && profile.training_points == 0
            && profile.skills[0] == 57
            && total_copper(profile.currency) == 98_116,
        "reconnected profile did not persist both purchases"
    );
    ensure!(
        live.training_state().last_report().is_none()
            && live.training_state().active_trainer().is_none(),
        "reconnect inherited a trainer transaction"
    );
    println!(
        "RECONNECT profile_skill=57 profile_practices=0 profile_carried=98116 fresh_session=true"
    );
    drop(live);
    std::thread::sleep(Duration::from_secs(4));
    println!("SHUTDOWN normal_logout=true operator_must_verify_offline_and_restore=true");
    Ok(())
}
