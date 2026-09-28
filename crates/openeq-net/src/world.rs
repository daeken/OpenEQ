//! The world server: character select and the handoff to a zone.

use std::net::SocketAddr;

use tokio::time::{Duration, timeout};

use crate::opcodes::WorldOp;
use crate::packet::AppPacket;
use crate::stream::{EqStream, StreamError};

const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

/// RoF2's `LoginInfo_Struct` length. The world server identifies the client
/// version by this length, so it must be exact.
const LOGIN_INFO_SIZE: usize = 464;

/// Bytes of fixed data following a character's name in `CharacterSelectEntry`.
const CHARACTER_TAIL: usize = 274;

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
    #[error("invalid world packet: {0}")]
    Invalid(&'static str),
    #[error("zone is unavailable")]
    ZoneUnavailable,
    #[error("stream closed by the server")]
    Closed,
}

/// A character on the account, as shown at character select.
#[derive(Debug, Clone)]
pub struct Character {
    pub name: String,
    pub level: u8,
    pub class: u8,
    pub race: u32,
    pub gender: u8,
    pub zone: u16,
}

/// A connected world-server session.
pub struct WorldClient {
    stream: EqStream,
}

impl WorldClient {
    /// Connects and sends the login handoff issued by the login server.
    pub async fn connect(
        address: SocketAddr,
        account_id: u32,
        session_key: &str,
    ) -> Result<Self, WorldError> {
        Self::connect_mode(address, account_id, session_key, false).await
    }

    /// Reconnect to world while crossing a zone boundary using the existing
    /// authenticated login session. This does not return to character select.
    pub async fn connect_zoning(
        address: SocketAddr,
        account_id: u32,
        session_key: &str,
    ) -> Result<Self, WorldError> {
        Self::connect_mode(address, account_id, session_key, true).await
    }

    async fn connect_mode(
        address: SocketAddr,
        account_id: u32,
        session_key: &str,
        zoning: bool,
    ) -> Result<Self, WorldError> {
        let stream = EqStream::connect(address).await?;
        let client = Self { stream };

        // The payload is a fixed-size buffer holding "account\0key".
        let mut payload = vec![0u8; LOGIN_INFO_SIZE];
        let text = format!("{account_id}\0{session_key}");
        let bytes = text.as_bytes();
        let length = bytes.len().min(payload.len());
        payload[..length].copy_from_slice(&bytes[..length]);
        // RoF2 LoginInfo_Struct places zoning at byte188 (464 total bytes).
        payload[188] = u8::from(zoning);

        client
            .stream
            .send(&AppPacket::new(WorldOp::SendLoginInfo as u16, payload))
            .await?;
        Ok(client)
    }

    /// Reads the next packet, whatever it is.
    pub async fn next_packet(&mut self, what: &'static str) -> Result<AppPacket, WorldError> {
        match timeout(REPLY_TIMEOUT, self.stream.recv()).await {
            Ok(Some(packet)) => Ok(packet),
            Ok(None) => Err(WorldError::Closed),
            Err(_) => Err(WorldError::Timeout(what)),
        }
    }

    /// Waits for the character list, logging the message of the day on the way.
    pub async fn characters(&mut self) -> Result<Vec<Character>, WorldError> {
        loop {
            let packet = self.next_packet("character list").await?;
            match packet.opcode {
                op if op == WorldOp::SendCharInfo as u16 => {
                    return Ok(parse_characters(&packet.data));
                }
                op if op == WorldOp::MessageOfTheDay as u16 => {
                    let text: Vec<u8> = packet
                        .data
                        .iter()
                        .copied()
                        .take_while(|byte| *byte != 0)
                        .collect();
                    tracing::info!(motd = %String::from_utf8_lossy(&text), "message of the day");
                }
                op => {
                    tracing::debug!(
                        opcode = format!("{op:#06x}"),
                        name = crate::opcodes::world_op_name(op),
                        data = packet.data.len(),
                        "world packet"
                    );
                }
            }
        }
    }

    /// Selects a character and waits for the world server's zone handoff.
    pub async fn enter_world(&mut self, name: &str) -> Result<SocketAddr, WorldError> {
        if name.is_empty() || name.len() >= 64 || name.as_bytes().contains(&0) {
            return Err(WorldError::Invalid("character name"));
        }
        self.send(&AppPacket::empty(WorldOp::WorldClientReady as u16))
            .await?;
        let mut payload = vec![0; 72];
        payload[..name.len()].copy_from_slice(name.as_bytes());
        self.send(&AppPacket::new(WorldOp::EnterWorld as u16, payload))
            .await?;
        loop {
            let packet = self.next_packet("zone server handoff").await?;
            match packet.opcode {
                op if op == WorldOp::ZoneServerInfo as u16 => {
                    if packet.data.len() < 130 {
                        return Err(WorldError::Invalid("zone address"));
                    }
                    let ip_end = packet.data[..128]
                        .iter()
                        .position(|b| *b == 0)
                        .unwrap_or(128);
                    let ip = std::str::from_utf8(&packet.data[..ip_end])
                        .ok()
                        .and_then(|s| s.parse().ok())
                        .ok_or(WorldError::Invalid("zone IP"))?;
                    return Ok(SocketAddr::new(
                        ip,
                        u16::from_le_bytes([packet.data[128], packet.data[129]]),
                    ));
                }
                op if op == WorldOp::ZoneUnavailable as u16 => {
                    return Err(WorldError::ZoneUnavailable);
                }
                _ => {}
            }
        }
    }

    /// Sends an arbitrary application packet.
    pub async fn send(&self, packet: &AppPacket) -> Result<(), WorldError> {
        self.stream.send(packet).await?;
        Ok(())
    }

    pub async fn recv(&mut self) -> Option<AppPacket> {
        self.stream.recv().await
    }
}

/// Parses the character-select payload.
///
/// Each entry is a null-terminated name, then a fixed
/// tail matching RoF2's `CharacterSelectEntry_Struct`.
fn parse_characters(data: &[u8]) -> Vec<Character> {
    if data.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut cursor = 4usize;
    let mut characters = Vec::with_capacity(count.min(32));

    for _ in 0..count.min(32) {
        let name_start = cursor;
        while cursor < data.len() && data[cursor] != 0 {
            cursor += 1;
        }
        let name = String::from_utf8_lossy(&data[name_start..cursor]).into_owned();
        if cursor == data.len() {
            break;
        }
        cursor += 1;

        if cursor + CHARACTER_TAIL > data.len() {
            break;
        }
        let tail = &data[cursor..cursor + CHARACTER_TAIL];
        let class = tail[0];
        let race = u32::from_le_bytes(tail[1..5].try_into().unwrap());
        let level = tail[5];
        let zone = u16::from_le_bytes([tail[11], tail[12]]);
        let gender = tail[15];
        cursor += CHARACTER_TAIL;

        characters.push(Character {
            name,
            level,
            class,
            race,
            gender,
            zone,
        });
    }
    characters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_one_character() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1u32.to_le_bytes());
        payload.extend_from_slice(b"Daeken\0");
        let mut tail = vec![0u8; CHARACTER_TAIL];
        tail[11..13].copy_from_slice(&394u16.to_le_bytes()); // zone
        tail[1..5].copy_from_slice(&1u32.to_le_bytes()); // race
        tail[0] = 4;
        tail[5] = 30;
        tail[15] = 1; // class
        payload.extend_from_slice(&tail);

        let characters = parse_characters(&payload);
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].name, "Daeken");
        assert_eq!(characters[0].level, 30);
        assert_eq!(characters[0].race, 1);
        assert_eq!(characters[0].class, 4);
        assert_eq!(characters[0].zone, 394);
    }
}
