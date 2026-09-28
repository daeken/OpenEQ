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

use std::net::{IpAddr, SocketAddr};

use tokio::time::{Duration, timeout};

use crate::opcodes::LoginOp;
use crate::packet::AppPacket;
use crate::stream::{EqStream, StreamError};
use crate::{crypto, opcodes};

/// How long to wait for a reply before giving up.
const REPLY_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum LoginError {
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
    #[error("login rejected by the server")]
    Rejected,
    #[error("malformed {context} packet")]
    Malformed { context: &'static str },
    #[error("stream closed by the server")]
    Closed,
}

/// An authenticated login session.
#[derive(Debug, Clone)]
pub struct Session {
    pub account_id: u32,
    /// The short-lived key handed to the world server to prove the login.
    pub key: String,
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
        // An unencrypted header, then an encrypted 80-byte player login reply.
        if reply.data.len() < 90 {
            return Err(LoginError::Rejected);
        }
        let decrypted = crypto::decrypt(&reply.data[10..90]).ok_or(LoginError::Malformed {
            context: "login accepted",
        })?;
        if decrypted.first().copied().unwrap_or(0) == 0 {
            return Err(LoginError::Rejected);
        }
        // PlayerLoginReply: success(1), error(4), string(1), unknown(1),
        // unknown(1), login server id, 11-byte key, failed attempts.
        let account_id = read_u32(&decrypted, 8).ok_or(LoginError::Malformed {
            context: "login accepted",
        })?;
        let key = read_c_string(&decrypted, 12, 11);

        Ok(Session { account_id, key })
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
        payload.extend_from_slice(&5u32.to_le_bytes()); // sequence
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
        if reply.data.len() < 15 {
            return Err(LoginError::Malformed {
                context: "play response",
            });
        }
        if reply.data[10] != 0 {
            Ok(())
        } else {
            Err(LoginError::Rejected)
        }
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

fn parse_server_list(data: &[u8]) -> Result<Vec<ServerEntry>, LoginError> {
    // LoginBaseMessage (10) + LoginBaseReplyMessage (6) + count (4).
    let count = read_u32(data, 16).ok_or(LoginError::Malformed {
        context: "server list",
    })? as usize;
    let mut cursor = 20usize;
    let mut servers = Vec::with_capacity(count);
    for _ in 0..count {
        let ip = read_c_string_advance(data, &mut cursor);
        let server_type = read_u32(data, cursor).ok_or(LoginError::Malformed {
            context: "server list entry",
        })?;
        cursor += 4;
        let server_id = read_u32(data, cursor).ok_or(LoginError::Malformed {
            context: "server list entry",
        })?;
        cursor += 4;
        let name = read_c_string_advance(data, &mut cursor);
        let country = read_c_string_advance(data, &mut cursor);
        let language = read_c_string_advance(data, &mut cursor);
        let status = read_u32(data, cursor).ok_or(LoginError::Malformed {
            context: "server list entry",
        })?;
        cursor += 4;
        let players = read_u32(data, cursor).ok_or(LoginError::Malformed {
            context: "server list entry",
        })?;
        cursor += 4;

        servers.push(ServerEntry {
            address: ip.parse().unwrap_or(IpAddr::from([127, 0, 0, 1])),
            server_type,
            server_id,
            name,
            country,
            language,
            status,
            players,
        });
    }
    Ok(servers)
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let slice = data.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_c_string(data: &[u8], offset: usize, limit: usize) -> String {
    let end = (offset + limit).min(data.len());
    let slice = data.get(offset..end).unwrap_or(&[]);
    let slice = match slice.iter().position(|byte| *byte == 0) {
        Some(index) => &slice[..index],
        None => slice,
    };
    String::from_utf8_lossy(slice).into_owned()
}

fn read_c_string_advance(data: &[u8], cursor: &mut usize) -> String {
    let start = *cursor;
    while *cursor < data.len() && data[*cursor] != 0 {
        *cursor += 1;
    }
    let value = String::from_utf8_lossy(&data[start..*cursor]).into_owned();
    *cursor += 1; // step over the terminator
    value
}

/// Formats a world opcode for logging.
pub fn describe(opcode: u16) -> &'static str {
    opcodes::world_op_name(opcode)
}
