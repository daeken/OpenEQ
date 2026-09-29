//! The login server: authentication, the server list, and entering a world.
//!
//! Flow, as seen by the client:
//!
//! 1. session handshake (handled by [`EqStream::connect`])
//! 2. `SessionReady` -> the server replies with `ChatMessage`
//! 3. `Login` with DES-encrypted credentials -> `LoginAccepted`, carrying the
//!    account id and a short-lived session key
//! 4. `ServerListRequest` -> `ServerListResponse`
//! 5. `PlayEverquestRequest` -> `PlayEverquestResponse`

use std::{
    collections::HashSet,
    fmt,
    net::{IpAddr, SocketAddr},
};

use tokio::time::{Duration, timeout};

use crate::opcodes::LoginOp;
use crate::packet::AppPacket;
use crate::stream::{EqStream, StreamError};
use crate::wire::{Reader, u32_at};
use crate::{crypto, opcodes};

/// How long to wait for a reply before giving up.
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_SERVERS: usize = 1024;
const MAX_SERVER_NAME: usize = 200;
const PLAY_SEQUENCE: u32 = 5;

/// EQEmu loginserver/login_types.h::LS::ErrStr and Client::SendFailedLogin.
/// Unknown codes remain available to the caller without inventing a reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RejectionReason {
    #[error("the username or password was not accepted")]
    InvalidCredentials,
    #[error("a character is already online on this world; wait briefly and try again")]
    CharacterAlreadyOnline,
    #[error("the world is currently unavailable")]
    ServerUnavailable,
    #[error("the account is suspended")]
    AccountSuspended,
    #[error("the account is banned")]
    AccountBanned,
    #[error("the world is at capacity; try again later")]
    WorldFull,
    #[error("the server rejected the request (code {0})")]
    Unknown(u32),
}
impl RejectionReason {
    fn from_code(code: u32) -> Self {
        match code {
            105 => Self::InvalidCredentials,
            111 => Self::CharacterAlreadyOnline,
            326 => Self::ServerUnavailable,
            337 => Self::AccountSuspended,
            338 => Self::AccountBanned,
            339 => Self::WorldFull,
            other => Self::Unknown(other),
        }
    }
    pub fn code(self) -> u32 {
        match self {
            Self::InvalidCredentials => 105,
            Self::CharacterAlreadyOnline => 111,
            Self::ServerUnavailable => 326,
            Self::AccountSuspended => 337,
            Self::AccountBanned => 338,
            Self::WorldFull => 339,
            Self::Unknown(code) => code,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
    #[error("{context} rejected: {reason}")]
    Rejected {
        context: &'static str,
        reason: RejectionReason,
    },
    #[error("malformed {context} packet")]
    Malformed { context: &'static str },
    #[error("stream closed by the server")]
    Closed,
}

/// An authenticated login session.
#[derive(Clone)]
pub struct Session {
    pub account_id: u32,
    /// The short-lived key handed to the world server to prove the login.
    pub key: String,
}
impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("account_id", &self.account_id)
            .field("key", &"[redacted]")
            .finish()
    }
}

/// One entry in the login server's world list.
#[derive(Debug, Clone)]
pub struct ServerEntry {
    pub address: IpAddr,
    pub server_type: u32,
    pub server_id: u32,
    pub name: String,
    pub country: String,
    pub language: String,
    pub status: u32,
    pub players: u32,
}

impl ServerEntry {
    pub fn is_up(&self) -> bool {
        self.status == 0 || self.status == 2
    }
}

/// A connected login-server session.
pub struct LoginClient {
    stream: EqStream,
}

impl LoginClient {
    /// Connects and performs the session handshake.
    pub async fn connect(address: SocketAddr) -> Result<Self, LoginError> {
        Ok(Self {
            stream: EqStream::connect(address).await?,
        })
    }

    /// Announces the session and authenticates.
    pub async fn login(&mut self, username: &str, password: &str) -> Result<Session, LoginError> {
        self.stream
            .send(&AppPacket::new(LoginOp::SessionReady as u16, vec![0; 4]))
            .await?;
        self.wait_for(LoginOp::ChatMessage as u16, "chat message")
            .await?;

        // Credentials are packed as "user\0pass\0" and then encrypted.
        let mut credentials = Vec::with_capacity(username.len() + password.len() + 2);
        credentials.extend_from_slice(username.as_bytes());
        credentials.push(0);
        credentials.extend_from_slice(password.as_bytes());
        credentials.push(0);

        // LoginBaseMessage: sequence, compressed, encrypt type, unused.
        let mut payload = Vec::with_capacity(10 + credentials.len() + 8);
        payload.extend_from_slice(&3u32.to_le_bytes());
        payload.push(0);
        payload.push(2); // DES
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&crypto::encrypt(&credentials));

        self.stream
            .send(&AppPacket::new(LoginOp::Login as u16, payload))
            .await?;

        let reply = self
            .wait_for(LoginOp::LoginAccepted as u16, "login accepted")
            .await?;
        parse_login_reply(&reply.data)
    }

    /// Fetches the list of worlds available to this account.
    pub async fn server_list(&mut self) -> Result<Vec<ServerEntry>, LoginError> {
        self.stream
            .send(&AppPacket::new(
                LoginOp::ServerListRequest as u16,
                vec![0, 0, 0, 0],
            ))
            .await?;
        let reply = self
            .wait_for(LoginOp::ServerListResponse as u16, "server list")
            .await?;
        parse_server_list(&reply.data)
    }

    /// Asks to enter `server_id`; the login server forwards it to the world.
    pub async fn play(&mut self, server_id: u32) -> Result<(), LoginError> {
        let mut payload = Vec::with_capacity(14);
        payload.extend_from_slice(&PLAY_SEQUENCE.to_le_bytes());
        payload.push(0);
        payload.push(0);
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.extend_from_slice(&server_id.to_le_bytes());
        self.stream
            .send(&AppPacket::new(
                LoginOp::PlayEverquestRequest as u16,
                payload,
            ))
            .await?;

        let reply = self
            .wait_for(LoginOp::PlayEverquestResponse as u16, "play response")
            .await?;
        parse_play_reply(&reply.data, server_id)
    }

    /// Waits for a specific application opcode, logging anything else.
    async fn wait_for(&mut self, opcode: u16, what: &'static str) -> Result<AppPacket, LoginError> {
        loop {
            let packet = match timeout(REPLY_TIMEOUT, self.stream.recv()).await {
                Ok(Some(packet)) => packet,
                Ok(None) => return Err(LoginError::Closed),
                Err(_) => return Err(LoginError::Timeout(what)),
            };
            if packet.opcode == opcode {
                return Ok(packet);
            }
            tracing::debug!(
                opcode = format!("{:#06x}", packet.opcode),
                data = packet.data.len(),
                "ignoring login packet while waiting for {what}"
            );
        }
    }
}

fn malformed(context: &'static str) -> LoginError {
    LoginError::Malformed { context }
}

/// Fixed LoginBaseMessage header. EQEmu sends no compression in these replies.
fn header(
    data: &[u8],
    sequence: u32,
    encryption: u8,
    context: &'static str,
) -> Result<(), LoginError> {
    if data.len() < 10 || u32_at(data, 0) != Some(sequence) || data[4] != 0 || data[5] != encryption
    {
        return Err(malformed(context));
    }
    Ok(())
}

fn base_reply(data: &[u8], context: &'static str) -> Result<(), LoginError> {
    if data.len() < 6 || data[0] > 1 || data[5] != 0 {
        return Err(malformed(context));
    }
    if data[0] == 0 {
        return Err(LoginError::Rejected {
            context,
            reason: RejectionReason::from_code(u32_at(data, 1).unwrap()),
        });
    }
    Ok(())
}

fn parse_login_reply(data: &[u8]) -> Result<Session, LoginError> {
    const CONTEXT: &str = "authentication";
    // EQEmu sends a 10-byte unencrypted header plus an 80-byte encrypted reply.
    if data.len() != 90 {
        return Err(malformed(CONTEXT));
    }
    header(data, 3, 2, CONTEXT)?;
    let decrypted = crypto::decrypt(&data[10..]).ok_or_else(|| malformed(CONTEXT))?;
    base_reply(&decrypted, CONTEXT)?;
    let account_id = u32_at(&decrypted, 8).ok_or_else(|| malformed(CONTEXT))?;
    let key = Reader(&decrypted[12..23])
        .string(10)
        .ok_or_else(|| malformed(CONTEXT))?;
    if account_id == 0 || key.is_empty() || !key.is_ascii() {
        return Err(malformed(CONTEXT));
    }
    Ok(Session { account_id, key })
}

fn parse_play_reply(data: &[u8], server_id: u32) -> Result<(), LoginError> {
    const CONTEXT: &str = "world access";
    if data.len() != 20 {
        return Err(malformed(CONTEXT));
    }
    header(data, PLAY_SEQUENCE, 0, CONTEXT)?;
    if u32_at(data, 16) != Some(server_id) {
        return Err(malformed("play response server ID"));
    }
    base_reply(&data[10..16], CONTEXT)
}

fn parse_server_list(data: &[u8]) -> Result<Vec<ServerEntry>, LoginError> {
    const CONTEXT: &str = "server list";
    // Our ServerListRequest sends sequence zero; EQEmu echoes it in the header.
    if data.len() < 20 {
        return Err(malformed(CONTEXT));
    }
    header(data, 0, 0, CONTEXT)?;
    base_reply(&data[10..16], CONTEXT)?;
    let mut r = Reader(&data[16..]);
    let count = r.u32().ok_or_else(|| malformed(CONTEXT))? as usize;
    // Even empty C-strings require 20 bytes per entry. Bound allocation before
    // trusting the count, and reject partial/trailing lists as a whole.
    if count > MAX_SERVERS || count > r.0.len() / 20 {
        return Err(malformed(CONTEXT));
    }
    let mut servers = Vec::with_capacity(count);
    let mut ids = HashSet::with_capacity(count);
    for _ in 0..count {
        let entry = (|| -> Option<ServerEntry> {
            let address = r.string(45)?.parse().ok()?;
            let server_type = r.u32()?;
            let server_id = r.u32()?;
            let name = r.string(MAX_SERVER_NAME)?;
            let country = r.string(16)?;
            let language = r.string(16)?;
            let status = r.u32()?;
            let players = r.u32()?;
            if server_id == 0 || name.is_empty() || !ids.insert(server_id) {
                return None;
            }
            Some(ServerEntry {
                address,
                server_type,
                server_id,
                name,
                country,
                language,
                status,
                players,
            })
        })()
        .ok_or_else(|| malformed("server list entry"))?;
        servers.push(entry);
    }
    if !r.done() {
        return Err(malformed(CONTEXT));
    }
    Ok(servers)
}

/// Formats a world opcode for logging.
pub fn describe(opcode: u16) -> &'static str {
    opcodes::world_op_name(opcode)
}

#[cfg(test)]
mod tests {
    use super::*;

    // EQEmu LoginBaseMessage + packed LoginBaseReplyMessage.
    fn reply(sequence: u32, success: bool, code: u32) -> Vec<u8> {
        let mut out = sequence.to_le_bytes().to_vec();
        out.extend_from_slice(&[0; 6]);
        out.push(u8::from(success));
        out.extend_from_slice(&code.to_le_bytes());
        out.push(0);
        out
    }
    fn server(out: &mut Vec<u8>, address: &str, id: u32, name: &str, status: u32) {
        out.extend_from_slice(address.as_bytes());
        out.push(0);
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b"\0us\0en\0");
        out.extend_from_slice(&status.to_le_bytes());
        out.extend_from_slice(&37u32.to_le_bytes());
    }
    fn roster() -> Vec<u8> {
        let mut out = reply(0, true, 101);
        out.extend_from_slice(&2u32.to_le_bytes());
        server(&mut out, "192.0.2.7", 9, "Open World", 0);
        server(&mut out, "2001:db8::8", 13, "Locked World", 4);
        out
    }
    #[test]
    fn server_list_preserves_identifiers_addresses_and_status() {
        let servers = parse_server_list(&roster()).unwrap();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0].server_id, 9);
        assert_eq!(servers[0].address, "192.0.2.7".parse::<IpAddr>().unwrap());
        assert_eq!(servers[0].name, "Open World");
        assert_eq!(servers[0].country, "us");
        assert_eq!(servers[0].language, "en");
        assert_eq!(servers[0].players, 37);
        assert!(servers[0].is_up());
        assert_eq!(servers[1].address, "2001:db8::8".parse::<IpAddr>().unwrap());
        assert!(!servers[1].is_up());
        let mut empty = reply(0, true, 101);
        empty.extend_from_slice(&0u32.to_le_bytes());
        assert!(parse_server_list(&empty).unwrap().is_empty());
    }
    #[test]
    fn server_list_rejects_all_truncations_unbounded_counts_and_trailers() {
        let data = roster();
        for n in 0..data.len() {
            assert!(
                parse_server_list(&data[..n]).is_err(),
                "accepted prefix {n}"
            );
        }
        let mut extra = data.clone();
        extra.push(0);
        assert!(parse_server_list(&extra).is_err());
        for count in [MAX_SERVERS as u32 + 1, u32::MAX] {
            let mut bad = data.clone();
            bad[16..20].copy_from_slice(&count.to_le_bytes());
            assert!(parse_server_list(&bad).is_err());
        }
        let mut malformed = reply(0, true, 101);
        malformed.extend_from_slice(&1u32.to_le_bytes());
        malformed.extend_from_slice(&[b'A'; 100]);
        assert!(parse_server_list(&malformed).is_err());
    }
    #[test]
    fn server_list_rejects_invalid_addresses_names_ids_and_headers() {
        for (ip, id, name) in [
            ("invalid", 1, "World"),
            ("", 1, "World"),
            ("192.0.2.1", 0, "World"),
            ("192.0.2.1", 1, ""),
        ] {
            let mut data = reply(0, true, 101);
            data.extend_from_slice(&1u32.to_le_bytes());
            server(&mut data, ip, id, name, 0);
            assert!(parse_server_list(&data).is_err());
        }
        let mut data = reply(0, true, 101);
        data.extend_from_slice(&1u32.to_le_bytes());
        server(
            &mut data,
            "192.0.2.1",
            1,
            &"x".repeat(MAX_SERVER_NAME + 1),
            0,
        );
        assert!(parse_server_list(&data).is_err());
        let mut duplicate = reply(0, true, 101);
        duplicate.extend_from_slice(&2u32.to_le_bytes());
        server(&mut duplicate, "192.0.2.1", 1, "First", 0);
        server(&mut duplicate, "192.0.2.2", 1, "Second", 0);
        assert!(parse_server_list(&duplicate).is_err());
        for offset in [0, 4, 5, 10, 15] {
            let mut bad = roster();
            bad[offset] = 2;
            assert!(
                parse_server_list(&bad).is_err(),
                "accepted bad header at {offset}"
            );
        }
        let mut failed = reply(0, false, 326);
        failed.extend_from_slice(&0u32.to_le_bytes());
        assert!(matches!(
            parse_server_list(&failed),
            Err(LoginError::Rejected {
                reason: RejectionReason::ServerUnavailable,
                ..
            })
        ));
    }
    #[test]
    fn play_requires_complete_matching_reply_and_retains_server_reason() {
        let mut accepted = reply(PLAY_SEQUENCE, true, 101);
        accepted.extend_from_slice(&91u32.to_le_bytes());
        assert!(parse_play_reply(&accepted, 91).is_ok());
        for n in 0..accepted.len() {
            assert!(parse_play_reply(&accepted[..n], 91).is_err());
        }
        assert!(parse_play_reply(&accepted, 92).is_err());
        let mut extra = accepted.clone();
        extra.push(0);
        assert!(parse_play_reply(&extra, 91).is_err());
        for offset in [0, 4, 5, 10, 15] {
            let mut bad = accepted.clone();
            bad[offset] = 2;
            assert!(parse_play_reply(&bad, 91).is_err());
        }
        for (code, reason) in [
            (105, RejectionReason::InvalidCredentials),
            (111, RejectionReason::CharacterAlreadyOnline),
            (326, RejectionReason::ServerUnavailable),
            (337, RejectionReason::AccountSuspended),
            (338, RejectionReason::AccountBanned),
            (339, RejectionReason::WorldFull),
            (98765, RejectionReason::Unknown(98765)),
        ] {
            let mut denied = reply(PLAY_SEQUENCE, false, code);
            denied.extend_from_slice(&91u32.to_le_bytes());
            let Err(LoginError::Rejected { reason: actual, .. }) = parse_play_reply(&denied, 91)
            else {
                panic!("missing reason {code}");
            };
            assert_eq!(actual, reason);
            assert_eq!(actual.code(), code);
            assert!(!actual.to_string().is_empty());
            assert!(matches!(
                parse_play_reply(&denied, 92),
                Err(LoginError::Malformed { .. })
            ));
        }
    }
    fn encrypted_login(body: &[u8]) -> Vec<u8> {
        let mut out = 3u32.to_le_bytes().to_vec();
        out.extend_from_slice(&[0, 2, 0, 0, 0, 0]);
        out.extend_from_slice(&crypto::encrypt(body));
        out
    }
    #[test]
    fn login_reply_checks_identity_and_redacts_session_key() {
        let mut body = vec![0; 80];
        body[0] = 1;
        body[1..5].copy_from_slice(&101u32.to_le_bytes());
        body[8..12].copy_from_slice(&42u32.to_le_bytes());
        body[12..22].copy_from_slice(b"TESTKEY123");
        let data = encrypted_login(&body);
        let session = parse_login_reply(&data).unwrap();
        assert_eq!(session.account_id, 42);
        assert_eq!(session.key, "TESTKEY123");
        let debug = format!("{session:?}");
        assert!(debug.contains("42") && debug.contains("redacted"));
        assert!(!debug.contains(&session.key));
        for n in 0..data.len() {
            assert!(matches!(
                parse_login_reply(&data[..n]),
                Err(LoginError::Malformed { .. })
            ));
        }
        let mut extra = data.clone();
        extra.push(0);
        assert!(parse_login_reply(&extra).is_err());
        for range in [8..12, 12..23] {
            let mut bad = body.clone();
            bad[range].fill(0);
            assert!(parse_login_reply(&encrypted_login(&bad)).is_err());
        }
        let mut no_terminator = body.clone();
        no_terminator[22] = b'X';
        assert!(parse_login_reply(&encrypted_login(&no_terminator)).is_err());
        body[0] = 0;
        body[1..5].copy_from_slice(&105u32.to_le_bytes());
        assert!(matches!(
            parse_login_reply(&encrypted_login(&body)),
            Err(LoginError::Rejected {
                reason: RejectionReason::InvalidCredentials,
                ..
            })
        ));
    }
}
