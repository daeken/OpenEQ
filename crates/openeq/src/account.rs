//! Interactive login selection. Private credentials remain on one worker, whose
//! Tokio runtime also owns the eventual live zone and its transport tasks.
pub mod preferences;
use crate::live::LiveWorld;
use anyhow::{Context, Result, bail, ensure};
use openeq_net::{
    login::{LoginClient, ServerEntry},
    world::{Character, WorldClient},
    zone::ZoneClient,
};
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    pub host: String,
    pub login_port: u16,
    pub world_port: u16,
}
impl Default for Endpoint {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            login_port: 5999,
            world_port: 9000,
        }
    }
}
impl Endpoint {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.host.trim().is_empty()
                && self.host.len() <= 253
                && !self
                    .host
                    .chars()
                    .any(|c| c.is_control() || c.is_whitespace()),
            "Enter a server hostname or IPv4 address."
        );
        ensure!(
            self.login_port > 0 && self.world_port > 0,
            "Server ports must be between 1 and 65535."
        );
        Ok(())
    }
}
/// A nonsecret identity for presentation preferences, never a temporary config
/// with invented credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionIdentity {
    pub endpoint: Endpoint,
    pub server_id: u32,
    pub character: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Credentials,
    Authenticating,
    Worlds,
    JoiningWorld,
    Characters,
    EnteringZone,
    Playing,
    Returning,
}
impl Stage {
    pub fn busy(self) -> bool {
        matches!(
            self,
            Self::Authenticating | Self::JoiningWorld | Self::EnteringZone | Self::Returning
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Credentials => "Sign in",
            Self::Authenticating => "Signing in",
            Self::Worlds => "Choose a server",
            Self::JoiningWorld => "Connecting to the world",
            Self::Characters => "Choose a character",
            Self::EnteringZone => "Entering Norrath",
            Self::Playing => "Playing",
            Self::Returning => "Returning to character selection",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Token {
    pub attempt: u64,
    pub revision: u64,
}
#[derive(Default)]
pub struct View {
    pub stage: Stage,
    pub token: Token,
    pub servers: Vec<ServerEntry>,
    pub characters: Vec<Character>,
    pub world_name: String,
    pub selected_world: Option<u32>,
    pub selected_character: Option<String>,
    pub notice: Option<String>,
    /// Back on a resumed roster ends the authenticated session; the old login
    /// socket is not assumed to survive an arbitrarily long play session.
    pub resumed: bool,
}
pub struct Ready {
    pub identity: SessionIdentity,
    pub live: LiveWorld,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    ChooseWorld(u32),
    ChooseCharacter(String),
    RefreshWorlds,
    Back,
}
struct Request {
    token: Token,
    action: Action,
}
enum Event {
    Worlds(Vec<ServerEntry>),
    Characters {
        world_name: String,
        characters: Vec<Character>,
    },
    Ready(Box<Ready>),
    Returning {
        disconnected: bool,
    },
    SignedOut,
    Failed(String),
}
struct Reply {
    token: Token,
    event: Event,
}
pub struct AccountController {
    pub view: View,
    replies: Mutex<mpsc::Receiver<Reply>>,
    sender: mpsc::Sender<Reply>,
    requests: Option<tokio::sync::mpsc::Sender<Request>>,
    cancel: Option<tokio::sync::watch::Sender<bool>>,
}
impl Default for AccountController {
    fn default() -> Self {
        let (sender, replies) = mpsc::channel();
        Self {
            view: View::default(),
            replies: Mutex::new(replies),
            sender,
            requests: None,
            cancel: None,
        }
    }
}
impl AccountController {
    pub fn sign_in(
        &mut self,
        endpoint: Endpoint,
        username: String,
        password: String,
    ) -> Result<()> {
        ensure!(
            self.view.stage == Stage::Credentials,
            "A sign-in is already in progress."
        );
        endpoint.validate()?;
        ensure!(
            !username.is_empty()
                && username.len() <= 128
                && !username.chars().any(char::is_control),
            "Enter your account username."
        );
        ensure!(
            !password.is_empty()
                && password.len() <= 512
                && !password.chars().any(char::is_control),
            "Enter your account password."
        );
        self.cancel();
        let token = self.view.token;
        self.view.stage = Stage::Authenticating;
        let (requests, rx) = tokio::sync::mpsc::channel(8);
        let (cancel, mut cancelled) = tokio::sync::watch::channel(false);
        self.requests = Some(requests);
        self.cancel = Some(cancel);
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new().name("eq-account".into()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(_) => { let _ = sender.send(Reply { token,event:Event::Failed("Could not start the connection worker.".into()) }); return; }
            };
            runtime.block_on(async move {
                let entering = AtomicBool::new(false);
                let connection = connect_stages(&endpoint,username,password,token,rx,&sender,&entering);
                tokio::pin!(connection);
                let result = tokio::select! {
                    _ = cancelled.changed() => {
                        // EnterWorld may already have committed on the server.
                        // Finish that handshake only to log out; never publish
                        // its foreground world after cancellation.
                        if entering.load(Ordering::Relaxed)
                            && let Ok(Ok(connected)) = tokio::time::timeout(std::time::Duration::from_secs(45),connection).await {
                            cleanup_entry(connected.zone).await;
                        }
                        return;
                    },
                    result = &mut connection => result,
                };
                match result {
                    Ok(connected) => {
                        if *cancelled.borrow() { cleanup_entry(connected.zone).await; return; }
                        if let Err(error) = play_session(connected, &sender, &mut cancelled).await {
                            let _ = sender.send(Reply { token, event: Event::Failed(format!("{error:#}")) });
                        }
                    }
                    Err(error) => { let _ = sender.send(Reply { token,event:Event::Failed(format!("{error:#}")) }); }
                }
            });
        });
        if let Err(error) = spawned {
            self.cancel();
            return Err(error).context("starting connection worker");
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
        self.requests = None;
        self.view = View {
            selected_world: self.view.selected_world,
            selected_character: self.view.selected_character.take(),
            token: Token {
                attempt: self.view.token.attempt.wrapping_add(1),
                revision: 0,
            },
            ..Default::default()
        };
    }
    pub fn action(&mut self, token: Token, action: Action) -> bool {
        if token != self.view.token {
            return false;
        }
        let next = match (&action, self.view.stage) {
            (Action::ChooseWorld(id), Stage::Worlds)
                if self
                    .view
                    .servers
                    .iter()
                    .any(|s| s.server_id == *id && s.is_up()) =>
            {
                Stage::JoiningWorld
            }
            (Action::RefreshWorlds, Stage::Worlds) => Stage::JoiningWorld,
            (Action::ChooseCharacter(name), Stage::Characters)
                if self
                    .view
                    .characters
                    .iter()
                    .any(|c| c.enabled && c.name == *name) =>
            {
                Stage::EnteringZone
            }
            (Action::Back, Stage::Characters) => Stage::JoiningWorld,
            _ => return false,
        };
        if self
            .requests
            .as_ref()
            .is_none_or(|requests| requests.try_send(Request { token, action }).is_err())
        {
            return false;
        }
        self.view.stage = next;
        self.view.notice = None;
        true
    }
    pub fn poll(&mut self) -> Option<Ready> {
        let mut ready = None;
        let replies: Vec<_> = self.replies.lock().unwrap().try_iter().collect();
        for reply in replies {
            // Failure can arrive from any pending stage in this attempt.
            if reply.token.attempt != self.view.token.attempt {
                continue;
            }
            if !matches!(reply.event, Event::Failed(_))
                && reply.token.revision < self.view.token.revision
            {
                continue;
            }
            match reply.event {
                Event::Worlds(servers) => {
                    self.view.stage = Stage::Worlds;
                    self.view.token = reply.token;
                    self.view.selected_world = self
                        .view
                        .selected_world
                        .filter(|id| servers.iter().any(|s| s.server_id == *id && s.is_up()))
                        .or_else(|| servers.iter().find(|s| s.is_up()).map(|s| s.server_id));
                    self.view.servers = servers;
                    self.view.characters.clear();
                    self.view.notice = None;
                }
                Event::Characters {
                    world_name,
                    characters,
                } => {
                    self.view.stage = Stage::Characters;
                    self.view.token = reply.token;
                    self.view.selected_character = self
                        .view
                        .selected_character
                        .take()
                        .filter(|name| characters.iter().any(|c| c.enabled && c.name == *name))
                        .or_else(|| {
                            characters
                                .iter()
                                .find(|c| c.enabled)
                                .map(|c| c.name.clone())
                        });
                    self.view.world_name = world_name;
                    self.view.characters = characters;
                    if !self.view.resumed {
                        self.view.notice = None;
                    }
                }
                Event::Ready(returned) => {
                    self.view.stage = Stage::Playing;
                    // Keep private worker channels/session alive for camp.
                    // A later failure in this batch must supersede this Ready.
                    self.view.token = reply.token;
                    self.view.notice = None;
                    ready = Some(*returned);
                }
                Event::Returning { disconnected } => {
                    self.view.stage = Stage::Returning;
                    self.view.token = reply.token;
                    self.view.resumed = true;
                    self.view.notice = disconnected.then(|| {
                        "Connection closed while camping; returning to character selection.".into()
                    });
                    self.view.characters.clear();
                }
                Event::SignedOut => self.cancel(),
                Event::Failed(notice) => {
                    self.cancel();
                    self.view.notice = Some(notice);
                }
            }
        }
        if self.view.stage == Stage::Playing {
            ready
        } else {
            None
        }
    }
}
impl Drop for AccountController {
    fn drop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(true);
        }
    }
}

struct Connected {
    zone: ZoneClient,
    identity: SessionIdentity,
    token: Token,
    session: openeq_net::login::Session,
    world_address: SocketAddr,
    world_name: String,
    requests: tokio::sync::mpsc::Receiver<Request>,
}

async fn connect_stages(
    endpoint: &Endpoint,
    username: String,
    password: String,
    mut token: Token,
    mut requests: tokio::sync::mpsc::Receiver<Request>,
    sender: &mpsc::Sender<Reply>,
    entering: &AtomicBool,
) -> Result<Connected> {
    let address = tokio::net::lookup_host((endpoint.host.as_str(), endpoint.login_port))
        .await?
        .find(|address| address.is_ipv4())
        .context("The login hostname has no IPv4 address.")?;
    let mut login = LoginClient::connect(address).await?;
    let session = login.login(&username, &password).await?;
    drop(password);
    drop(username);
    'worlds: loop {
        let servers = login.server_list().await?;
        token.revision = token.revision.wrapping_add(1);
        sender
            .send(Reply {
                token,
                event: Event::Worlds(servers.clone()),
            })
            .map_err(|_| anyhow::anyhow!("Sign-in cancelled."))?;
        let server = loop {
            let request = requests.recv().await.context("Sign-in cancelled.")?;
            if request.token != token {
                continue;
            }
            match request.action {
                Action::RefreshWorlds => continue 'worlds,
                Action::ChooseWorld(id) => {
                    if let Some(server) = servers
                        .iter()
                        .find(|server| server.server_id == id && server.is_up())
                    {
                        break server;
                    }
                }
                _ => {}
            }
        };
        login.play(server.server_id).await?;
        let world_address = SocketAddr::new(server.address, endpoint.world_port);
        let mut world =
            WorldClient::connect(world_address, session.account_id, &session.key).await?;
        let characters = world.characters().await?;
        token.revision = token.revision.wrapping_add(1);
        sender
            .send(Reply {
                token,
                event: Event::Characters {
                    world_name: server.name.clone(),
                    characters: characters.clone(),
                },
            })
            .map_err(|_| anyhow::anyhow!("Sign-in cancelled."))?;
        let character = loop {
            let request = requests.recv().await.context("Sign-in cancelled.")?;
            if request.token != token {
                continue;
            }
            match request.action {
                Action::Back => continue 'worlds,
                Action::ChooseCharacter(name) => {
                    if let Some(character) =
                        characters.iter().find(|character| character.name == name)
                    {
                        if !character.enabled {
                            bail!("That character is disabled by the server.");
                        }
                        break character.name.clone();
                    }
                }
                _ => {}
            }
        };
        entering.store(true, Ordering::Relaxed);
        let zone =
            openeq_net::session::enter_character(&mut world, world_address, &session, &character)
                .await?;
        let identity = SessionIdentity {
            endpoint: endpoint.clone(),
            server_id: server.server_id,
            character,
        };
        return Ok(Connected {
            zone,
            identity,
            token,
            session,
            world_address,
            world_name: server.name.clone(),
            requests,
        });
    }
}

/// The same runtime owns every zone and world socket for the whole account
/// session. Its password was already discarded before the first world list.
async fn play_session(
    mut connected: Connected,
    sender: &mpsc::Sender<Reply>,
    cancelled: &mut tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    loop {
        let character = connected.identity.character.clone();
        let (mut live, io) = LiveWorld::channels(
            character.clone(),
            connected
                .identity
                .endpoint
                .host
                .eq_ignore_ascii_case("storage2.daeken.dev"),
        );
        live.enable_roster_return();
        if sender
            .send(Reply {
                token: connected.token,
                event: Event::Ready(Box::new(Ready {
                    identity: connected.identity.clone(),
                    live,
                })),
            })
            .is_err()
        {
            cleanup_entry(connected.zone).await;
            return Ok(());
        }
        let disconnected = match io.run_for_account(connected.zone, &character).await {
            crate::live::LiveExit::Shutdown => return Ok(()),
            crate::live::LiveExit::Failed(error) => {
                bail!("Connection ended: {error}. Sign in again to reconnect.")
            }
            crate::live::LiveExit::Camped => false,
            crate::live::LiveExit::DisconnectedWhileCamping => true,
        };
        if *cancelled.borrow() || cancelled.has_changed().is_err() {
            return Ok(());
        }
        connected.token.revision = connected.token.revision.wrapping_add(1);
        sender
            .send(Reply {
                token: connected.token,
                event: Event::Returning { disconnected },
            })
            .map_err(|_| anyhow::anyhow!("Account screen closed."))?;
        let reconnect = async {
            let mut world = WorldClient::connect(
                connected.world_address,
                connected.session.account_id,
                &connected.session.key,
            )
            .await?;
            let characters = world.characters().await?;
            Ok::<_, anyhow::Error>((world, characters))
        };
        let (mut world, characters) = tokio::select! {
            _ = cancelled.changed() => return Ok(()),
            result = reconnect => result.context("Could not return to character selection; sign in again")?,
        };
        connected.token.revision = connected.token.revision.wrapping_add(1);
        sender
            .send(Reply {
                token: connected.token,
                event: Event::Characters {
                    world_name: connected.world_name.clone(),
                    characters: characters.clone(),
                },
            })
            .map_err(|_| anyhow::anyhow!("Account screen closed."))?;
        let character = loop {
            let request = tokio::select! {
                _ = cancelled.changed() => return Ok(()),
                request = connected.requests.recv() => request.context("Account screen closed.")?,
            };
            if request.token != connected.token {
                continue;
            }
            match request.action {
                Action::Back => {
                    let _ = sender.send(Reply {
                        token: connected.token,
                        event: Event::SignedOut,
                    });
                    return Ok(());
                }
                Action::ChooseCharacter(name)
                    if characters
                        .iter()
                        .any(|entry| entry.enabled && entry.name == name) =>
                {
                    break name;
                }
                _ => {}
            }
        };
        let entry = openeq_net::session::enter_character(
            &mut world,
            connected.world_address,
            &connected.session,
            &character,
        );
        tokio::pin!(entry);
        let zone = tokio::select! {
            _ = cancelled.changed() => {
                // Entry can already be committed. Complete only to log out,
                // retaining the same bounded cancellation contract as login.
                if let Ok(Ok(zone)) = tokio::time::timeout(std::time::Duration::from_secs(45), &mut entry).await {
                    cleanup_entry(zone).await;
                }
                return Ok(());
            },
            result = &mut entry => result?,
        };
        if *cancelled.borrow() || cancelled.has_changed().is_err() {
            cleanup_entry(zone).await;
            return Ok(());
        }
        connected.identity.character = character.clone();
        connected.zone = zone;
    }
}

async fn cleanup_entry(mut zone: ZoneClient) {
    if let Err(error) = crate::live::logout_zone(&mut zone).await {
        tracing::warn!(target:"openeq_net::account",%error,"cancelled entry cleanup failed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server(id: u32) -> ServerEntry {
        ServerEntry {
            server_id: id,
            name: format!("World {id}"),
            address: "127.0.0.1".parse().unwrap(),
            server_type: 0,
            country: "US".into(),
            language: "EN".into(),
            status: 0,
            players: 1,
        }
    }
    fn roster(enabled: bool) -> Character {
        Character {
            name: "Fixture".into(),
            level: 1,
            class: 1,
            race: 1,
            gender: 0,
            zone: 77,
            instance_id: 0,
            enabled,
            appearance: Default::default(),
        }
    }
    #[test]
    fn cancelled_attempt_cannot_restore_old_lists_or_errors() {
        let mut c = AccountController::default();
        let old = c.view.token;
        c.cancel();
        c.sender
            .send(Reply {
                token: old,
                event: Event::Worlds(vec![server(1)]),
            })
            .unwrap();
        c.sender
            .send(Reply {
                token: old,
                event: Event::Failed("old error".into()),
            })
            .unwrap();
        assert!(c.poll().is_none());
        assert_eq!(c.view.stage, Stage::Credentials);
        assert!(c.view.notice.is_none());
    }
    #[test]
    fn stable_selection_survives_refresh_and_stale_or_double_clicks_are_rejected() {
        let mut c = AccountController::default();
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        c.requests = Some(tx);
        let first = Token {
            attempt: 0,
            revision: 1,
        };
        c.sender
            .send(Reply {
                token: first,
                event: Event::Worlds(vec![server(1), server(2)]),
            })
            .unwrap();
        c.poll();
        c.view.selected_world = Some(2);
        assert!(c.action(first, Action::RefreshWorlds));
        assert!(!c.action(first, Action::RefreshWorlds));
        assert!(matches!(
            rx.try_recv().unwrap().action,
            Action::RefreshWorlds
        ));
        let refreshed = Token {
            revision: 2,
            ..first
        };
        c.sender
            .send(Reply {
                token: refreshed,
                event: Event::Worlds(vec![server(2), server(1)]),
            })
            .unwrap();
        c.poll();
        assert_eq!(c.view.selected_world, Some(2));
        assert!(!c.action(first, Action::ChooseWorld(1)));
        assert!(!c.action(refreshed, Action::ChooseWorld(99)));
        assert!(c.action(refreshed, Action::ChooseWorld(2)));
        assert!(!c.action(refreshed, Action::ChooseWorld(2)));
    }
    #[test]
    fn empty_and_disabled_rosters_do_not_send_entry() {
        let mut c = AccountController::default();
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        c.requests = Some(tx);
        let token = Token {
            attempt: 0,
            revision: 1,
        };
        for characters in [vec![], vec![roster(false)]] {
            c.sender
                .send(Reply {
                    token,
                    event: Event::Characters {
                        world_name: "World".into(),
                        characters,
                    },
                })
                .unwrap();
            c.poll();
            assert!(c.view.selected_character.is_none());
            assert!(!c.action(token, Action::ChooseCharacter("Fixture".into())));
        }
        c.sender
            .send(Reply {
                token,
                event: Event::Characters {
                    world_name: "World".into(),
                    characters: vec![roster(true)],
                },
            })
            .unwrap();
        c.poll();
        assert!(c.action(token, Action::ChooseCharacter("Fixture".into())));
        assert!(!c.action(token, Action::ChooseCharacter("Fixture".into())));
    }

    #[test]
    fn unavailable_world_cannot_retain_selection_or_send_play() {
        let mut c = AccountController::default();
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        c.requests = Some(tx);
        c.view.selected_world = Some(1);
        let mut down = server(1);
        down.status = 1;
        c.sender
            .send(Reply {
                token: c.view.token,
                event: Event::Worlds(vec![down, server(2)]),
            })
            .unwrap();
        c.poll();
        assert_eq!(c.view.selected_world, Some(2));
        assert!(!c.action(c.view.token, Action::ChooseWorld(1)));
        assert!(rx.try_recv().is_err());
        assert!(c.action(c.view.token, Action::ChooseWorld(2)));
    }
}

#[cfg(test)]
mod return_tests {
    use super::*;
    fn ready(controller: &AccountController, token: Token) {
        let (live, _io) = LiveWorld::channels("CampFixture".into(), false);
        controller
            .sender
            .send(Reply {
                token,
                event: Event::Ready(Box::new(Ready {
                    identity: SessionIdentity {
                        endpoint: Endpoint::default(),
                        server_id: 1,
                        character: "CampFixture".into(),
                    },
                    live,
                })),
            })
            .unwrap();
    }
    #[test]
    fn playing_retains_cancel_and_request_handles_and_failure_wins_same_batch() {
        let mut controller = AccountController::default();
        let (requests, _rx) = tokio::sync::mpsc::channel(8);
        let (cancel, _cancelled) = tokio::sync::watch::channel(false);
        controller.requests = Some(requests);
        controller.cancel = Some(cancel);
        let token = controller.view.token;
        ready(&controller, token);
        assert!(controller.poll().is_some());
        assert_eq!(controller.view.stage, Stage::Playing);
        assert!(controller.requests.is_some() && controller.cancel.is_some());
        ready(&controller, token);
        controller
            .sender
            .send(Reply {
                token,
                event: Event::Failed("Connection ended".into()),
            })
            .unwrap();
        assert!(controller.poll().is_none());
        assert_eq!(controller.view.stage, Stage::Credentials);
    }
    #[test]
    fn return_retires_old_roster_and_stale_actions_and_exposes_uncertain_close() {
        let mut controller = AccountController::default();
        let old = controller.view.token;
        let returned = Token { revision: 2, ..old };
        controller.view.stage = Stage::Playing;
        controller
            .sender
            .send(Reply {
                token: returned,
                event: Event::Returning { disconnected: true },
            })
            .unwrap();
        assert!(controller.poll().is_none());
        assert_eq!(controller.view.stage, Stage::Returning);
        assert!(controller.view.characters.is_empty());
        assert!(controller.view.resumed);
        assert!(
            controller
                .view
                .notice
                .as_deref()
                .unwrap()
                .contains("Connection closed while camping")
        );
        assert!(!controller.action(old, Action::ChooseCharacter("CampFixture".into())));
        controller.cancel();
        ready(&controller, returned);
        controller
            .sender
            .send(Reply {
                token: returned,
                event: Event::Failed("late return".into()),
            })
            .unwrap();
        assert!(controller.poll().is_none());
        assert_eq!(controller.view.stage, Stage::Credentials);
        assert!(controller.view.notice.is_none());
    }
}
