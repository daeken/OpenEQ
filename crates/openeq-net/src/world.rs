//! The world server: character select and the handoff to a zone.

use std::net::SocketAddr;

use tokio::time::{Duration, timeout};

use crate::opcodes::WorldOp;
use crate::packet::AppPacket;
use crate::stream::{EqStream, StreamError};

const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

/// RoF2's `LoginInfo_Struct` length. The world server identifies the client
/// version by this length, so it must be exact.
const LOGIN_INFO_SIZE: usize = 488;

/// Bytes of fixed data following a character's name in `CharacterSelectEntry`.
const CHARACTER_TAIL: usize = 188;

#[derive(Debug, thiserror::Error)]
pub enum WorldError {
    #[error(transparent)]
    Stream(#[from] StreamError),
    #[error("timed out waiting for {0}")]
    Timeout(&'static str),
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
        let stream = EqStream::connect(address).await?;
        let client = Self { stream };

        // The payload is a fixed-size buffer holding "account\0key".
        let mut payload = vec![0u8; LOGIN_INFO_SIZE];
        let text = format!("{account_id}\0{session_key}");
        let bytes = text.as_bytes();
        let length = bytes.len().min(payload.len());
        payload[..length].copy_from_slice(&bytes[..length]);

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

    /// Sends an arbitrary application packet (used once zone entry exists).
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
/// Each entry is a three-byte prefix, a null-terminated name, then a fixed
/// tail matching RoF2's `CharacterSelectEntry_Struct`.
fn parse_characters(data: &[u8]) -> Vec<Character> {
    if data.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut cursor = 4usize;
    let mut characters = Vec::with_capacity(count);

    for _ in 0..count {
        // Prefix: level, hair style, gender.
        let (Some(&level), Some(&gender)) = (data.get(cursor), data.get(cursor + 2)) else {
            break;
        };
        cursor += 3;

        let name_start = cursor;
        while cursor < data.len() && data[cursor] != 0 {
            cursor += 1;
        }
        let name = String::from_utf8_lossy(&data[name_start..cursor]).into_owned();
        cursor += 1;

        if cursor + CHARACTER_TAIL > data.len() {
            break;
        }
        let tail = &data[cursor..cursor + CHARACTER_TAIL];
        // Offsets within the tail: zone 160, race 166, class 171.
        let zone = u16::from_le_bytes([tail[160], tail[161]]);
        let race = u32::from_le_bytes([tail[166], tail[167], tail[168], tail[169]]);
        let class = tail[171];
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
        payload.extend_from_slice(&[30, 3, 1]); // level, hair, gender
        payload.extend_from_slice(b"Daeken\0");
        let mut tail = vec![0u8; CHARACTER_TAIL];
        tail[160..162].copy_from_slice(&394u16.to_le_bytes()); // zone
        tail[166..170].copy_from_slice(&1u32.to_le_bytes()); // race
        tail[171] = 4; // class
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
