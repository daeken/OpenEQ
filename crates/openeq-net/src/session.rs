//! Shared connection configuration for the game and the headless zone probe.
use crate::{
    login::{LoginClient, Session},
    world::WorldClient,
    zone::ZoneClient,
};
use anyhow::{Context, bail};
use serde::Deserialize;
use std::{net::SocketAddr, path::Path};

// Deliberately no Debug implementation: this contains a password.
#[derive(Clone, Deserialize)]
pub struct ConnectionConfig {
    pub host: String,
    #[serde(default = "login_port")]
    pub login_port: u16,
    #[serde(default = "world_port")]
    pub world_port: u16,
    pub username: String,
    pub password: String,
    pub character: String,
    pub server_id: Option<u32>,
}
fn login_port() -> u16 {
    5999
}
fn world_port() -> u16 {
    9000
}

/// Enter a selected character while preserving authenticated later zoning.
/// Keep this future and its returned ZoneClient on the same live Tokio runtime
/// as the world connection: its UDP reader/ticker tasks belong to that runtime.
pub async fn enter_character(
    world: &mut WorldClient,
    world_address: SocketAddr,
    session: &Session,
    character: &str,
) -> anyhow::Result<ZoneClient> {
    let address = world.enter_world(character).await?;
    tracing::info!(%address, character, "entering zone");
    let mut zone = ZoneClient::connect(address, character).await?;
    zone.enable_zoning(world_address, session.account_id, session.key.clone());
    Ok(zone)
}

impl ConnectionConfig {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)
            .with_context(|| format!("reading connection config {}", path.display()))?;
        serde_json::from_slice(&bytes).context("invalid connection config")
    }
    pub async fn connect(&self) -> anyhow::Result<ZoneClient> {
        let addr = tokio::net::lookup_host((self.host.as_str(), self.login_port))
            .await?
            .find(|addr| addr.is_ipv4())
            .context("login hostname has no IPv4 address")?;
        tracing::info!(%addr, "connecting to login server");
        let mut login = LoginClient::connect(addr).await?;
        let session = login.login(&self.username, &self.password).await?;
        tracing::info!(account_id = session.account_id, "authenticated");
        let servers = login.server_list().await?;
        let server = servers
            .iter()
            .find(|s| self.server_id.map_or(s.is_up(), |id| s.server_id == id))
            .context("requested world is unavailable")?;
        login.play(server.server_id).await?;
        let world_address = SocketAddr::new(server.address, self.world_port);
        let mut world =
            WorldClient::connect(world_address, session.account_id, &session.key).await?;
        let characters = world.characters().await?;
        tracing::info!(names = ?characters.iter().map(|c| &c.name).collect::<Vec<_>>(), "character list");
        let selected = characters
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&self.character))
            .with_context(|| format!("character {} is not on this account", self.character))?;
        if !selected.enabled {
            bail!(
                "character {} is currently disabled by the server",
                selected.name
            );
        }
        enter_character(&mut world, world_address, &session, &self.character).await
    }
}
