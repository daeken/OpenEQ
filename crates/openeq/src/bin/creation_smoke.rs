//! One operator-gated creation on the explicitly provisioned ordinary fixture.
//! No zone entry, SQL, movement, combat or audio. Never retries or deletes.
use anyhow::{Context, ensure};
use openeq::{
    account::{AccountController, Action, Endpoint, Stage},
    account_creation::{
        Feature, Phase,
        editor::{ChoiceField, Editor},
    },
    account_preview::{Preview, Request},
};
use openeq_net::session::ConnectionConfig;
use openeq_render::{Renderer, actors::CharacterModelSet};
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const NAME: &str = "Trailborn";
fn poll(account: &mut AccountController) -> anyhow::Result<()> {
    ensure!(account.poll().is_none(), "unexpected zone entry");
    if let Some(notice) = &account.view.notice {
        anyhow::bail!("account flow failed: {notice}; inspect dedicated fixture without retry");
    }
    if let Some(view) = &account.view.creation {
        ensure!(
            !matches!(view.phase, Some(Phase::Uncertain(_) | Phase::Rejected(_))),
            "creation was rejected or uncertain; inspect dedicated fixture without retry"
        );
    }
    Ok(())
}
fn wait(
    account: &mut AccountController,
    predicate: impl Fn(&AccountController) -> bool,
) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(45);
    loop {
        poll(account)?;
        if predicate(account) {
            return Ok(());
        }
        ensure!(
            Instant::now() < until,
            "account wait expired; do not retry creation"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn roster(config: &ConnectionConfig) -> anyhow::Result<AccountController> {
    let mut account = AccountController::default();
    account.sign_in(
        Endpoint {
            host: config.host.clone(),
            login_port: config.login_port,
            world_port: config.world_port,
        },
        config.username.clone(),
        config.password.clone(),
    )?;
    wait(&mut account, |account| account.view.stage == Stage::Worlds)?;
    let id = account
        .view
        .servers
        .iter()
        .find(|server| server.is_up() && config.server_id.is_none_or(|id| server.server_id == id))
        .context("configured world unavailable")?
        .server_id;
    ensure!(
        account.action(account.view.token, Action::ChooseWorld(id)),
        "world selection refused"
    );
    wait(&mut account, |account| {
        account.view.stage == Stage::Characters
            && account.view.creation.as_ref().is_some_and(|view| {
                let c = &view.selection.capabilities;
                view.selection.catalog.is_some()
                    && c.expansion_mask.is_some()
                    && c.maximum_characters.is_some()
                    && c.membership.is_some()
            })
    })?;
    Ok(account)
}
fn write_new(path: &Path, value: &serde_json::Value) -> anyhow::Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    writeln!(file)?;
    file.sync_all()?;
    Ok(())
}
fn gate(account: &mut AccountController, path: &Path) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(600);
    while !path.is_file() {
        poll(account)?;
        ensure!(
            Instant::now() < until,
            "operator gate expired; do not retry creation"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    Ok(())
}
fn prepare_preview(
    account: &mut AccountController,
    editor: &mut Editor,
    preview: &mut Preview,
    renderer: &Renderer,
    assets: &Path,
) -> anyhow::Result<()> {
    let until = Instant::now() + Duration::from_secs(90);
    loop {
        poll(account)?;
        let selection = &account
            .view
            .creation
            .as_ref()
            .context("missing creation authority")?
            .selection;
        editor.refresh(account.view.token.attempt, selection)?;
        let context = editor.context();
        let request = Request {
            token: account.view.token,
            creation: Some(context),
            character: editor.draft().preview_character()?,
            dir: assets.into(),
            model_set: CharacterModelSet::Classic,
        };
        preview.update(Some(request.clone()), renderer);
        if let Ok(support) = preview.creation_preview(&request) {
            editor.apply_preview(context, support.policy, support.receipt, support.heritages);
            if editor.receipt_ready() {
                return Ok(());
            }
        }
        ensure!(
            Instant::now() < until,
            "matching original-asset preview unavailable"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: creation_smoke PRIVATE_CONFIG PRIVATE_PROOF_DIRECTORY EQ_ASSET_DIRECTORY"
    );
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    ensure!(
        config.username == "openeq_create1"
            && config.character == NAME
            && config.host == "storage2.daeken.dev",
        "creation proof is pinned to its dedicated account/name"
    );
    let proof = PathBuf::from(&args[2]);
    let assets = PathBuf::from(&args[3]);
    ensure!(
        proof.is_dir() && assets.is_dir(),
        "proof and asset directories must exist"
    );
    for marker in [
        "ready-to-create.json",
        "idle-verified.json",
        "authorize-create",
        "created.json",
        "continue-to-reconnect",
        "reconnected.json",
    ] {
        ensure!(
            !proof.join(marker).exists(),
            "old creation proof marker exists; inspect instead of retrying"
        );
    }
    let mut account = roster(&config)?;
    ensure!(
        account.view.characters.is_empty(),
        "dedicated roster must be empty before creation"
    );
    let first_connection = account.view.creation.as_ref().unwrap().selection.connection;
    let mut editor = Editor::new(
        account.view.token.attempt,
        &account.view.creation.as_ref().unwrap().selection,
    )?;
    for (field, value) in [
        (ChoiceField::Race, 1),
        (ChoiceField::Class, 1),
        (ChoiceField::Gender, 0),
    ] {
        let limit = editor.choices(field).len();
        for _ in 0..limit {
            if editor.value(field) == value {
                break;
            }
            ensure!(editor.cycle_choice(field, 1), "fixture choice unavailable");
        }
        ensure!(
            editor.value(field) == value,
            "fixture choice was not advertised"
        );
    }
    ensure!(editor.set_name(NAME.into()), "fixture name was not set");
    let mut renderer = Renderer::new_headless(800, 600)?;
    let mut preview = Preview::default();
    prepare_preview(&mut account, &mut editor, &mut preview, &renderer, &assets)?;
    ensure!(
        editor.cycle_feature(Feature::Face, 1),
        "fixture face choice unavailable"
    );
    prepare_preview(&mut account, &mut editor, &mut preview, &renderer, &assets)?;
    let submission = editor.submission()?;
    let draft = &submission.draft;
    ensure!(
        draft.choice.race == 1
            && draft.choice.class == 1
            && draft.gender == 0
            && draft.appearance.face == 1
            && editor.remaining_points()? == 0,
        "fixture draft differs from human warrior/default stats/face1"
    );
    renderer.set_ui_scaled(&openeq_ui::UiFrame::default(), 1.);
    ensure!(
        preview.render(&mut renderer, [800, 600], 256.),
        "fixture preview did not render"
    );
    let (width, height, pixels) = renderer
        .read_rgba()
        .context("preview readback unavailable")?;
    image::save_buffer(
        proof.join("preview.png"),
        &pixels,
        width,
        height,
        image::ColorType::Rgba8,
    )?;
    let record = serde_json::json!({"name":NAME,"race":draft.choice.race,"class":draft.choice.class,
        "gender":draft.gender,"deity":draft.choice.deity,"requested_start_zone":draft.choice.start_zone,
        "face":draft.appearance.face,"hair_color":draft.appearance.hair_color,
        "hair_style":draft.appearance.hair_style,"beard":draft.appearance.beard,"beard_color":draft.appearance.beard_color,
        "eye_color_1":draft.appearance.eye_color_1,"eye_color_2":draft.appearance.eye_color_2,
        "drakkin_heritage":draft.appearance.heritage,"drakkin_tattoo":draft.appearance.tattoo,"drakkin_details":draft.appearance.details,
        "str":draft.stats.strength,"sta":draft.stats.stamina,"dex":draft.stats.dexterity,"agi":draft.stats.agility,
        "int":draft.stats.intelligence,"wis":draft.stats.wisdom,"cha":draft.stats.charisma});
    write_new(&proof.join("ready-to-create.json"), &record)?;
    println!(
        "READY name=Trailborn actual_preview=true face=1 default_stats=true approval_sent=false"
    );
    // Character selection must survive deliberation without application polls.
    // This exceeds both the old observed failure and transport silence timeout.
    let idle_started = Instant::now();
    while idle_started.elapsed() < Duration::from_secs(120) {
        poll(&mut account)?;
        ensure!(
            account.view.stage == Stage::Characters
                && account.view.characters.is_empty()
                && account.view.creation.as_ref().is_some_and(|view| {
                    view.selection.connection == first_connection && view.operation.is_none()
                }),
            "idle character-selection authority changed before approval"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    write_new(
        &proof.join("idle-verified.json"),
        &serde_json::json!({"elapsed_ms":idle_started.elapsed().as_millis(),
            "same_connection":true,"roster_empty":true,"approval_sent":false}),
    )?;
    println!("IDLE_VERIFIED seconds=120 same_connection=true approval_sent=false");
    gate(&mut account, &proof.join("authorize-create"))?;
    ensure!(
        account.view.characters.is_empty(),
        "roster changed before explicit creation"
    );
    let action_token = account.view.token;
    ensure!(
        account.action(action_token, Action::Create(Box::new(submission.clone()))),
        "explicit creation refused"
    );
    ensure!(
        !account.action(action_token, Action::Create(Box::new(submission))),
        "duplicate creation accepted"
    );
    wait(&mut account, |account| {
        account
            .view
            .creation
            .as_ref()
            .is_some_and(|view| matches!(view.phase, Some(Phase::Completed(_))))
    })?;
    ensure!(
        account.view.characters.len() == 1,
        "unexpected roster size after creation"
    );
    let character = account.view.characters[0].clone();
    ensure!(
        character.name == NAME
            && character.race == 1
            && character.class == 1
            && character.gender == 0
            && character.level == 1
            && character.appearance.face == 1
            && character.enabled
            && character.zone == 202,
        "created roster identity/appearance/zone differs from the fixture"
    );
    write_new(&proof.join("created.json"), &record)?;
    println!("CREATED name=Trailborn level=1 zone=202 face=1 enabled=true duplicate_rejected=true");
    gate(&mut account, &proof.join("continue-to-reconnect"))?;
    account.cancel();
    drop(account);
    std::thread::sleep(Duration::from_secs(2));
    let mut account = roster(&config)?;
    ensure!(
        account.view.creation.as_ref().unwrap().selection.connection != first_connection,
        "reconnect reused creation authority"
    );
    ensure!(
        account.view.characters == vec![character],
        "fresh roster did not preserve the new character"
    );
    ensure!(
        account.view.creation.as_ref().unwrap().operation.is_none(),
        "reconnect inherited creation operation"
    );
    write_new(&proof.join("reconnected.json"), &record)?;
    account.cancel();
    println!(
        "RECONNECTED name=Trailborn fresh_roster=true creation_retried=false zone_entry=false"
    );
    Ok(())
}
