//! The world server: character select and the handoff to a zone.

use std::{collections::HashSet, net::SocketAddr};

use tokio::time::{Duration, timeout};

use crate::opcodes::WorldOp;
use crate::packet::AppPacket;
use crate::stream::{EqStream, StreamError};
use crate::wire::{Reader, u16_at, u32_at};

const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

/// RoF2's `LoginInfo_Struct` length. The world server identifies the client
/// version by this length, so it must be exact.
const LOGIN_INFO_SIZE: usize = 464;

/// Bytes of fixed data following a character's name in `CharacterSelectEntry`.
const CHARACTER_TAIL: usize = 274;
/// EQEmu common/patches/rof2_limits.h::CHARACTER_CREATION_LIMIT.
const MAX_CHARACTERS: usize = 12;

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Character {
    pub name: String,
    pub level: u8,
    pub class: u8,
    pub race: u32,
    pub gender: u8,
    pub zone: u16,
    pub instance_id: u16,
    /// RoF2's Enabled byte, separate from the return-home/tutorial flags.
    pub enabled: bool,
    pub appearance: CharacterAppearance,
}

/// RoF2 character-select appearance, independent of zone spawns and inventory.
/// Unknown material fields are retained without assigning visual semantics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharacterEquipment {
    pub material: u32,
    pub unknown1: u32,
    pub elite_material: u32,
    pub hero_forge_model: u32,
    pub material2: u32,
    pub color: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharacterAppearance {
    pub face: u8,
    /// Head, chest, arms, wrists, hands, legs, feet, primary, secondary.
    pub equipment: [CharacterEquipment; 9],
    pub drakkin_tattoo: u32,
    pub drakkin_details: u32,
    pub primary_model: u32,
    pub secondary_model: u32,
    pub hair_color: u8,
    pub beard_color: u8,
    pub eye_color_1: u8,
    pub eye_color_2: u8,
    pub hair_style: u8,
    pub beard: u8,
    pub drakkin_heritage: u32,
}

impl CharacterAppearance {
    fn from_tail(tail: &[u8]) -> Self {
        // common/patches/rof2_structs.h::CharacterSelectEntry_Struct:
        // Face at 16, nine 24-byte CharSelectEquip records at 17, two unknown
        // bytes at 233, then Drakkin/deity/held-model fields. The six cosmetic
        // bytes start at 255; GoHome/Tutorial at 261/262 precede heritage.
        Self {
            face: tail[16],
            equipment: std::array::from_fn(|index| {
                let offset = 17 + index * 24;
                CharacterEquipment {
                    material: u32_at(tail, offset).unwrap(),
                    unknown1: u32_at(tail, offset + 4).unwrap(),
                    elite_material: u32_at(tail, offset + 8).unwrap(),
                    hero_forge_model: u32_at(tail, offset + 12).unwrap(),
                    material2: u32_at(tail, offset + 16).unwrap(),
                    color: u32_at(tail, offset + 20).unwrap(),
                }
            }),
            drakkin_tattoo: u32_at(tail, 235).unwrap(),
            drakkin_details: u32_at(tail, 239).unwrap(),
            primary_model: u32_at(tail, 247).unwrap(),
            secondary_model: u32_at(tail, 251).unwrap(),
            hair_color: tail[255],
            beard_color: tail[256],
            eye_color_1: tail[257],
            eye_color_2: tail[258],
            hair_style: tail[259],
            beard: tail[260],
            drakkin_heritage: u32_at(tail, 263).unwrap(),
        }
    }
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
                    return parse_characters(&packet.data);
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
fn parse_characters(data: &[u8]) -> Result<Vec<Character>, WorldError> {
    let invalid = || WorldError::Invalid("character roster");
    let mut r = Reader(data);
    let count = r.u32().ok_or_else(invalid)? as usize;
    if count > MAX_CHARACTERS || count > r.0.len() / (CHARACTER_TAIL + 2) {
        return Err(invalid());
    }
    let mut characters = Vec::with_capacity(count);
    let mut names = HashSet::with_capacity(count);
    for _ in 0..count {
        // Identity strings must be complete and valid, not lossy display text.
        let length =
            r.0.iter()
                .take(64)
                .position(|b| *b == 0)
                .filter(|n| *n > 0)
                .ok_or_else(invalid)?;
        let name = std::str::from_utf8(r.take(length).ok_or_else(invalid)?)
            .map_err(|_| invalid())?
            .to_owned();
        r.skip(1).ok_or_else(invalid)?;
        let tail = r.take(CHARACTER_TAIL).ok_or_else(invalid)?;
        // Enabled follows Unknown1 at tail268; GoHome at261 is independent.
        if tail[268] > 1 || !names.insert(name.to_ascii_lowercase()) {
            return Err(invalid());
        }
        characters.push(Character {
            name,
            class: tail[0],
            race: u32_at(tail, 1).unwrap(),
            level: tail[5],
            zone: u16_at(tail, 11).unwrap(),
            instance_id: u16_at(tail, 13).unwrap(),
            gender: tail[15],
            enabled: tail[268] != 0,
            appearance: CharacterAppearance::from_tail(tail),
        });
    }
    if !r.done() {
        return Err(invalid());
    }
    Ok(characters)
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
        tail[13..15].copy_from_slice(&237u16.to_le_bytes());
        tail[261] = 0; // GoHome must not masquerade as Enabled.
        tail[268] = 1;
        payload.extend_from_slice(&tail);

        let characters = parse_characters(&payload).unwrap();
        assert_eq!(characters.len(), 1);
        assert_eq!(characters[0].name, "Daeken");
        assert_eq!(characters[0].level, 30);
        assert_eq!(characters[0].race, 1);
        assert_eq!(characters[0].class, 4);
        assert_eq!(characters[0].zone, 394);
        assert_eq!(characters[0].instance_id, 237);
        assert!(characters[0].enabled);
    }

    fn roster(names: &[&str]) -> Vec<u8> {
        let mut out = (names.len() as u32).to_le_bytes().to_vec();
        for (i, name) in names.iter().enumerate() {
            out.extend_from_slice(name.as_bytes());
            out.push(0);
            let mut tail = vec![0; CHARACTER_TAIL];
            tail[0] = 2;
            tail[1..5].copy_from_slice(&1u32.to_le_bytes());
            tail[5] = 50;
            tail[11..13].copy_from_slice(&77u16.to_le_bytes());
            tail[13..15].copy_from_slice(&(i as u16).to_le_bytes());
            tail[261] = 1; // GoHome true even for the disabled second row.
            tail[268] = u8::from(i != 1);
            out.extend_from_slice(&tail);
        }
        out
    }
    #[test]
    fn roster_distinguishes_empty_and_disabled_entries() {
        assert!(parse_characters(&0u32.to_le_bytes()).unwrap().is_empty());
        let characters = parse_characters(&roster(&["First", "Second"])).unwrap();
        assert_eq!(characters.len(), 2);
        assert!(characters[0].enabled);
        assert!(!characters[1].enabled);
        assert_eq!(characters[1].instance_id, 1);
        let names: Vec<_> = (0..MAX_CHARACTERS)
            .map(|i| format!("Character{i}"))
            .collect();
        let refs: Vec<_> = names.iter().map(String::as_str).collect();
        assert_eq!(
            parse_characters(&roster(&refs)).unwrap().len(),
            MAX_CHARACTERS
        );
    }
    #[test]
    fn roster_rejects_every_truncation_excess_count_and_trailer() {
        let data = roster(&["First", "Second"]);
        for n in 0..data.len() {
            assert!(
                parse_characters(&data[..n]).is_err(),
                "accepted partial roster {n}"
            );
        }
        let mut extra = data.clone();
        extra.push(0);
        assert!(parse_characters(&extra).is_err());
        for count in [MAX_CHARACTERS as u32 + 1, u32::MAX] {
            let mut bad = data.clone();
            bad[..4].copy_from_slice(&count.to_le_bytes());
            assert!(parse_characters(&bad).is_err());
        }
        assert!(parse_characters(&[0, 0, 0, 0, 0]).is_err());
    }
    #[test]
    fn roster_rejects_invalid_identity_and_enabled_byte() {
        for names in [vec![""], vec!["Same", "sAME"], vec!["a"; 13]] {
            assert!(parse_characters(&roster(&names)).is_err());
        }
        assert!(parse_characters(&roster(&[&"a".repeat(64)])).is_err());
        let mut bad = roster(&["First"]);
        bad[4] = 0xff;
        assert!(parse_characters(&bad).is_err());
        let mut bad = roster(&["First"]);
        bad[4 + 6 + 268] = 2;
        assert!(parse_characters(&bad).is_err());
        let mut no_nul = 1u32.to_le_bytes().to_vec();
        no_nul.extend_from_slice(&[b'A'; 350]);
        assert!(parse_characters(&no_nul).is_err());
    }

    #[test]
    fn roster_appearance_follows_packed_rof2_field_order() {
        // Construct in C++ declaration order, with different sentinels in
        // adjacent fields and every equipment record (including unknowns).
        let mut data = 1u32.to_le_bytes().to_vec();
        data.extend_from_slice(b"Appearance\0");
        let mut tail = vec![1]; // Class
        tail.extend_from_slice(&522u32.to_le_bytes());
        tail.extend_from_slice(&[75, 2]); // Level, ShroudClass
        tail.extend_from_slice(&1u32.to_le_bytes()); // ShroudRace
        tail.extend_from_slice(&202u16.to_le_bytes());
        tail.extend_from_slice(&4u16.to_le_bytes());
        tail.extend_from_slice(&[1, 7]); // Gender, Face
        for slot in 0..9u32 {
            for field in 0..6u32 {
                tail.extend_from_slice(&(0xa000_0000 + slot * 0x100 + field).to_le_bytes());
            }
        }
        tail.extend_from_slice(&[0xff, 0xfe]);
        for value in [3u32, 5, 396, 10001, 10002] {
            tail.extend_from_slice(&value.to_le_bytes());
        }
        tail.extend_from_slice(&[11, 12, 13, 14, 15, 16, 1, 0]);
        tail.extend_from_slice(&2u32.to_le_bytes());
        tail.extend_from_slice(&[0xf9, 1]);
        tail.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        tail.push(0xfa);
        assert_eq!(tail.len(), CHARACTER_TAIL);
        data.extend_from_slice(&tail);
        let character = parse_characters(&data).unwrap().remove(0);
        assert_eq!((character.race, character.gender), (522, 1));
        let appearance = character.appearance;
        assert_eq!(appearance.face, 7);
        for (slot, piece) in appearance.equipment.iter().enumerate() {
            let base = 0xa000_0000 + slot as u32 * 0x100;
            assert_eq!(
                *piece,
                CharacterEquipment {
                    material: base,
                    unknown1: base + 1,
                    elite_material: base + 2,
                    hero_forge_model: base + 3,
                    material2: base + 4,
                    color: base + 5
                }
            );
        }
        assert_eq!(
            (
                appearance.drakkin_tattoo,
                appearance.drakkin_details,
                appearance.drakkin_heritage
            ),
            (3, 5, 2)
        );
        assert_eq!(
            (appearance.primary_model, appearance.secondary_model),
            (10001, 10002)
        );
        assert_eq!(
            [
                appearance.hair_color,
                appearance.beard_color,
                appearance.eye_color_1,
                appearance.eye_color_2,
                appearance.hair_style,
                appearance.beard
            ],
            [11, 12, 13, 14, 15, 16]
        );
        assert!(character.enabled);
    }
}
