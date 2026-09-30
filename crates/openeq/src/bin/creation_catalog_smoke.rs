//! Read-only creation advertisements through the production account controller.
//! Pinned to the dedicated trade account; never selects/creates a character.
use anyhow::{Context, ensure};
use openeq::account::{AccountController, Action, Endpoint, Stage};
use openeq_net::session::ConnectionConfig;
use std::{
    path::Path,
    time::{Duration, Instant},
};

fn wait(
    account: &mut AccountController,
    predicate: impl Fn(&AccountController) -> bool,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        ensure!(account.poll().is_none(), "unexpected zone handoff");
        ensure!(
            account.view.notice.is_none(),
            "account flow reported a failure"
        );
        if predicate(account) {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "creation catalog timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 2,
        "usage: creation_catalog_smoke PRIVATE_CONFIG"
    );
    let config = ConnectionConfig::load(Path::new(&args[1]))?;
    ensure!(
        config.character == "Barterer"
            && config.username == "openeq_trade1"
            && config.host == "storage2.daeken.dev",
        "creation catalog proof is restricted to the dedicated trade account"
    );
    let mut account = AccountController::default();
    account.sign_in(
        Endpoint {
            host: config.host,
            login_port: config.login_port,
            world_port: config.world_port,
        },
        config.username,
        config.password,
    )?;
    wait(&mut account, |account| account.view.stage == Stage::Worlds)?;
    let server = account
        .view
        .servers
        .iter()
        .find(|server| server.is_up() && config.server_id.is_none_or(|id| server.server_id == id))
        .context("configured world unavailable")?
        .server_id;
    ensure!(
        account.action(account.view.token, Action::ChooseWorld(server)),
        "world selection refused"
    );
    wait(&mut account, |account| {
        account.view.stage == Stage::Characters
            && account.view.creation.as_ref().is_some_and(|view| {
                let selection = &view.selection;
                selection.catalog.is_some()
                    && selection.capabilities.expansion_mask.is_some()
                    && selection.capabilities.maximum_characters.is_some()
                    && selection.capabilities.membership.is_some()
            })
    })?;
    let view = account.view.creation.as_ref().unwrap();
    let selection = &view.selection;
    ensure!(
        selection.catalog_error.is_none() && view.operation.is_none(),
        "unexpected creation operation or invalid catalog"
    );
    ensure!(
        selection.characters.len() == 1 && selection.characters[0].name == "Barterer",
        "dedicated roster changed"
    );
    let catalog = selection.catalog.as_ref().unwrap();
    let permitted = catalog
        .combinations()
        .iter()
        .filter(|combination| {
            selection
                .capabilities
                .permits(combination, selection.characters.len())
                .is_ok()
        })
        .count();
    ensure!(permitted > 0, "no supported creation choices advertised");
    for combination in catalog.combinations() {
        catalog.resolve(combination.choice)?.1.default_stats()?;
    }
    println!(
        "CATALOG roster=1 combinations={} permitted={} capacity={} expansion_mask={} membership_tier={} default_allocations_valid=true",
        catalog.combinations().len(),
        permitted,
        selection.capabilities.maximum_characters.unwrap(),
        selection.capabilities.expansion_mask.unwrap(),
        selection.capabilities.membership.as_ref().unwrap().tier
    );
    account.cancel();
    ensure!(
        account.view.stage == Stage::Credentials,
        "cancellation did not retire selection"
    );
    println!("CANCELLED character_entry=false name_approval=false character_creation=false");
    Ok(())
}
