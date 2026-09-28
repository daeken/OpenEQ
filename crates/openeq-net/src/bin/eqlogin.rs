//! Drives the EQEmu login and world handshake from the command line.
//!
//! ```text
//! eqlogin --host 127.0.0.1 --user myuser --pass mypass
//! ```
//!
//! It authenticates, prints the server list, asks to enter a world, then
//! connects to that world and prints the characters on the account. Reaching
//! character select is the milestone that proves the whole protocol stack.

use std::net::SocketAddr;
use std::time::Duration;

use openeq_net::login::LoginClient;
use openeq_net::world::WorldClient;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut host = "127.0.0.1".to_string();
    let mut port = 5998u16;
    let mut world_port = 9000u16;
    let mut user = None;
    let mut pass = None;
    let mut server_id = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => host = args.next().unwrap_or(host),
            "--port" => port = args.next().and_then(|v| v.parse().ok()).unwrap_or(port),
            "--world-port" => {
                world_port = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(world_port)
            }
            "--user" => user = args.next(),
            "--pass" => pass = args.next(),
            "--server" => server_id = args.next().and_then(|v| v.parse::<u32>().ok()),
            "-v" | "--verbose" => {}
            other => anyhow::bail!("unrecognised argument {other}"),
        }
    }

    let user = user.unwrap_or_else(|| {
        eprintln!("usage: eqlogin --host HOST [--port 5998] --user USER --pass PASS");
        std::process::exit(2);
    });
    let pass = pass.unwrap_or_default();

    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid login address {host}:{port}"))?;

    println!("connecting to login server at {address}");
    let mut login = LoginClient::connect(address).await?;

    let session = login.login(&user, &pass).await?;
    println!(
        "authenticated: account id {}, session key {}",
        session.account_id, session.key
    );

    let servers = login.server_list().await?;
    println!("{} server(s):", servers.len());
    for entry in &servers {
        println!(
            "  [{:>3}] {:<40} {:<16} {:>4} players  {}",
            entry.server_id,
            entry.name,
            entry.address,
            entry.players,
            if entry.is_up() { "up" } else { "down" }
        );
    }

    let target = match server_id {
        Some(id) => servers
            .iter()
            .find(|entry| entry.server_id == id)
            .ok_or_else(|| anyhow::anyhow!("no server with id {id}"))?
            .clone(),
        None => servers
            .iter()
            .find(|entry| entry.is_up())
            .or_else(|| servers.first())
            .ok_or_else(|| anyhow::anyhow!("the login server returned no servers"))?
            .clone(),
    };
    println!("entering {} ({})", target.name, target.address);
    login.play(target.server_id).await?;

    let world_address = SocketAddr::new(target.address, world_port);
    println!("connecting to world server at {world_address}");
    let mut world = WorldClient::connect(world_address, session.account_id, &session.key).await?;

    // Give the world a moment, then report whatever it sends.
    let characters = tokio::time::timeout(Duration::from_secs(20), world.characters())
        .await
        .map_err(|_| anyhow::anyhow!("world server did not send a character list"))??;

    println!("{} character(s):", characters.len());
    for character in &characters {
        println!(
            "  {:<16} level {:>3} class {:>3} race {:>3} zone {}",
            character.name, character.level, character.class, character.race, character.zone
        );
    }

    Ok(())
}
