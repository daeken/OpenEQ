//! RoF2 death recovery messages. Packet coordinates remain in server space.
//!
//! Layouts follow EQEmu `zone/client.cpp::SendRespawnBinds`,
//! `common/patches/rof2.cpp::ENCODE(OP_ZonePlayerToBind)` and
//! `rof2_structs.h::Resurrect_Struct`, not the 228-byte emulator rez structure.
use crate::{AppPacket, wire::Reader, zone::ZoneError};

pub const OP_RESPAWN_WINDOW: u16 = 0x0ecb;
pub const OP_ZONE_PLAYER_TO_BIND: u16 = 0x08d8;
pub const OP_REZZ_REQUEST: u16 = 0x3c21;
pub const OP_REZZ_ANSWER: u16 = 0x701c;
const MAX_OPTIONS: usize = 256;
const MAX_LABEL_BYTES: usize = 1024;
const RESURRECTION_BYTES: usize = 236;

#[derive(Debug, Clone, PartialEq)]
pub struct RespawnOption {
    pub id: u32,
    pub zone_id: u32,
    pub position: [f32; 3],
    pub heading: f32,
    pub label: String,
    /// EQEmu marks its last option this way. The UI must keep it disabled until
    /// a matching resurrection offer arrives; the label is not an identifier.
    pub requires_resurrection: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RespawnWindow {
    pub initial_selection: u32,
    pub remaining_ms: u32,
    pub options: Vec<RespawnOption>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BindTransfer {
    /// Zero is meaningful: EQEmu requests a full same-zone bind re-entry when
    /// hover respawn is disabled. It must not be treated as an unknown zone.
    pub zone_id: u16,
    pub instance_id: u16,
    pub position: [f32; 3],
    pub heading: f32,
    pub label: String,
    pub save_items: u8,
    /// EQEmu's RoF2 encoder currently writes zero for these footer values.
    /// Authoritative resource updates arrive separately.
    pub resources: [u32; 3],
}

/// An immutable server offer. Reply bytes retain the original names, coordinate
/// basis and reserved words, changing only the action. In particular, converting
/// a displayed location to scene space must never alter the reply destination.
#[derive(Debug, Clone, PartialEq)]
pub struct ResurrectionOffer {
    wire: [u8; RESURRECTION_BYTES],
    zone_id: u16,
    instance_id: u16,
    position: [f32; 3],
    recipient: String,
    caster: String,
    spell_id: u32,
    corpse: String,
    action: u32,
}

impl ResurrectionOffer {
    pub fn zone_id(&self) -> u16 {
        self.zone_id
    }
    pub fn instance_id(&self) -> u16 {
        self.instance_id
    }
    pub fn position(&self) -> [f32; 3] {
        self.position
    }
    pub fn recipient(&self) -> &str {
        &self.recipient
    }
    pub fn caster(&self) -> &str {
        &self.caster
    }
    pub fn spell_id(&self) -> u32 {
        self.spell_id
    }
    pub fn corpse(&self) -> &str {
        &self.corpse
    }
    pub fn action(&self) -> u32 {
        self.action
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeathEvent {
    RespawnWindow(RespawnWindow),
    BindTransfer(BindTransfer),
    ResurrectionOffer(Box<ResurrectionOffer>),
}

#[derive(Debug, Clone)]
pub enum DeathCommand {
    /// Only send an ID from the current window, once. This codec deliberately
    /// does not invent session state or assume sending means the player revived.
    SelectRespawn { option_id: u32 },
    /// A hovering player accepting the final respawn option uses SelectRespawn
    /// instead. Do not send both that choice and a RezzAnswer for the same offer.
    AnswerResurrection {
        offer: Box<ResurrectionOffer>,
        accept: bool,
    },
}

pub fn encode_command(command: DeathCommand) -> Result<AppPacket, ZoneError> {
    Ok(match command {
        DeathCommand::SelectRespawn { option_id } => {
            AppPacket::new(OP_RESPAWN_WINDOW, option_id.to_le_bytes().to_vec())
        }
        DeathCommand::AnswerResurrection { offer, accept } => {
            let mut data = offer.wire.to_vec();
            data[224..228].copy_from_slice(&u32::from(accept).to_le_bytes());
            AppPacket::new(OP_REZZ_ANSWER, data)
        }
    })
}

pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<DeathEvent, ZoneError>> {
    let event = match opcode {
        OP_RESPAWN_WINDOW => parse_respawn(data).map(DeathEvent::RespawnWindow),
        OP_ZONE_PLAYER_TO_BIND => parse_bind(data).map(DeathEvent::BindTransfer),
        OP_REZZ_REQUEST => {
            parse_resurrection(data).map(|offer| DeathEvent::ResurrectionOffer(Box::new(offer)))
        }
        _ => return None,
    };
    Some(event.ok_or(ZoneError::Malformed("death recovery packet")))
}

fn parse_respawn(data: &[u8]) -> Option<RespawnWindow> {
    let mut reader = Reader(data);
    let initial_selection = reader.u32()?;
    let remaining_ms = reader.u32()?;
    reader.skip(4)?;
    let count = reader.u32()? as usize;
    // Each option needs 24 fixed bytes, a NUL label terminator and one flag.
    // Check this lower bound before trusting an allocation from the wire.
    if count == 0 || count > MAX_OPTIONS || count.checked_mul(26)? > reader.0.len() {
        return None;
    }
    let mut options = Vec::with_capacity(count);
    for _ in 0..count {
        let id = reader.u32()?;
        let zone_id = reader.u32()?;
        let position = [reader.float()?, reader.float()?, reader.float()?];
        let heading = reader.float()?;
        let label = reader.string(MAX_LABEL_BYTES)?;
        let requires_resurrection = match reader.u8()? {
            0 => false,
            1 => true,
            _ => return None,
        };
        if options.iter().any(|option: &RespawnOption| option.id == id) {
            return None;
        }
        options.push(RespawnOption {
            id,
            zone_id,
            position,
            heading,
            label,
            requires_resurrection,
        });
    }
    // SendRespawnBinds allocates 17 + 26*n + sum(label.len()+1), but writes
    // 16 + 25*n + sum(label.len()+1). Accept its n+1 zero bytes, as well as a
    // correctly sized payload; do not silently swallow arbitrary trailers.
    if !reader.done() && (reader.0.len() != count + 1 || reader.0.iter().any(|&b| b != 0)) {
        return None;
    }
    Some(RespawnWindow {
        initial_selection,
        remaining_ms,
        options,
    })
}

fn parse_bind(data: &[u8]) -> Option<BindTransfer> {
    let mut reader = Reader(data);
    let zone_id = reader.u16()?;
    let instance_id = reader.u16()?;
    let position = [reader.float()?, reader.float()?, reader.float()?];
    let heading = reader.float()?;
    let label = reader.string(MAX_LABEL_BYTES)?;
    let save_items = reader.u8()?;
    let resources = [reader.u32()?, reader.u32()?, reader.u32()?];
    reader.done().then_some(BindTransfer {
        zone_id,
        instance_id,
        position,
        heading,
        label,
        save_items,
        resources,
    })
}

fn parse_resurrection(data: &[u8]) -> Option<ResurrectionOffer> {
    let wire = data.try_into().ok()?;
    let mut reader = Reader(data);
    reader.skip(4)?;
    let zone_id = reader.u16()?;
    let instance_id = reader.u16()?;
    let y = reader.float()?;
    let x = reader.float()?;
    let z = reader.float()?;
    reader.skip(4)?;
    let recipient = Reader(reader.take(64)?).string(63)?;
    reader.skip(4)?;
    let caster = Reader(reader.take(64)?).string(63)?;
    let spell_id = reader.u32()?;
    let corpse = Reader(reader.take(64)?).string(63)?;
    let action = reader.u32()?;
    reader.skip(8)?;
    reader.done().then_some(ResurrectionOffer {
        wire,
        zone_id,
        instance_id,
        position: [x, y, z],
        recipient,
        caster,
        spell_id,
        corpse,
        action,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_field(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn float_field(bytes: &mut [u8], offset: usize, value: f32) {
        u32_field(bytes, offset, value.to_bits());
    }

    // Built from Client::SendRespawnBinds' writes and allocation separately;
    // these fixtures deliberately do not call the production codec's encoder.
    fn respawn_fixture(padded: bool) -> Vec<u8> {
        let labels = ["Bind Location", "Resurrect"];
        let exact_size = 16 + labels.iter().map(|name| 26 + name.len()).sum::<usize>();
        let allocation = exact_size + if padded { labels.len() + 1 } else { 0 };
        let mut data = vec![0; allocation];
        u32_field(&mut data, 0, 1);
        u32_field(&mut data, 4, 300_000);
        u32_field(&mut data, 12, 2);
        let mut offset = 16;
        for (i, name) in labels.iter().enumerate() {
            u32_field(&mut data, offset, i as u32);
            u32_field(&mut data, offset + 4, 202 + i as u32);
            for (field, value) in [(8, 123.25), (12, -456.5), (16, 78.75), (20, 384.)] {
                float_field(&mut data, offset + field, value);
            }
            data[offset + 24..offset + 24 + name.len()].copy_from_slice(name.as_bytes());
            data[offset + 25 + name.len()] = i as u8;
            offset += 26 + name.len();
        }
        data
    }

    fn bind_fixture() -> Vec<u8> {
        // rof2.cpp ENCODE(OP_ZonePlayerToBind): fixed 20, cstring, 13 footer.
        let mut data = vec![0; 20 + "Bind Location".len() + 1 + 13];
        data[2..4].copy_from_slice(&7_u16.to_le_bytes());
        for (offset, value) in [(4, 12.5), (8, -34.25), (12, 56.75), (16, 128.)] {
            float_field(&mut data, offset, value);
        }
        data[20..33].copy_from_slice(b"Bind Location");
        data[34] = 1;
        data
    }

    fn resurrection_fixture() -> Vec<u8> {
        // rof2_structs.h Resurrect_Struct offsets, including RoF2's last 8 bytes.
        let mut data = vec![0; 236];
        data[4..6].copy_from_slice(&202_u16.to_le_bytes());
        data[6..8].copy_from_slice(&17_u16.to_le_bytes());
        for (offset, value) in [(8, -456.5), (12, 123.25), (16, 78.75)] {
            float_field(&mut data, offset, value);
        }
        for (offset, value) in [
            (24, "RecoveryTest"),
            (92, "ClericTest"),
            (160, "RecoveryTest's corpse42"),
        ] {
            data[offset..offset + value.len()].copy_from_slice(value.as_bytes());
        }
        u32_field(&mut data, 156, 388);
        // Unknown values are irrelevant to display but must survive our reply.
        for offset in [0, 20, 88, 228, 232] {
            u32_field(&mut data, offset, 0x8877_6655 + offset as u32);
        }
        data
    }

    #[test]
    fn respawn_writers_exact_and_overallocated_payloads_decode_identically() {
        let parse = |bytes: &[u8]| parse_packet(OP_RESPAWN_WINDOW, bytes).unwrap().unwrap();
        let exact = parse(&respawn_fixture(false));
        assert_eq!(exact, parse(&respawn_fixture(true)));
        let DeathEvent::RespawnWindow(window) = exact else {
            panic!("respawn window")
        };
        assert_eq!(
            (window.initial_selection, window.remaining_ms),
            (1, 300_000)
        );
        assert_eq!(window.options.len(), 2);
        assert_eq!(window.options[0].id, 0);
        assert_eq!(window.options[0].zone_id, 202);
        assert_eq!(window.options[0].position, [123.25, -456.5, 78.75]);
        assert_eq!(window.options[0].heading, 384.);
        assert_eq!(window.options[0].label, "Bind Location");
        assert!(!window.options[0].requires_resurrection);
        assert!(window.options[1].requires_resurrection);
    }

    #[test]
    fn respawn_rejects_truncation_counts_duplicates_and_unexpected_trailers() {
        let exact = respawn_fixture(false);
        for size in 0..exact.len() {
            assert!(
                parse_packet(OP_RESPAWN_WINDOW, &exact[..size])
                    .unwrap()
                    .is_err(),
                "size {size}"
            );
        }
        for count in [0, 257, u32::MAX] {
            let mut bad = exact.clone();
            u32_field(&mut bad, 12, count);
            assert!(parse_packet(OP_RESPAWN_WINDOW, &bad).unwrap().is_err());
        }
        for trailer in [vec![0], vec![0; 2], vec![0; 4], vec![0, 0, 1]] {
            let mut bad = exact.clone();
            bad.extend(trailer);
            assert!(parse_packet(OP_RESPAWN_WINDOW, &bad).unwrap().is_err());
        }
        let mut duplicate = exact.clone();
        u32_field(&mut duplicate, 16 + 26 + "Bind Location".len(), 0);
        assert!(
            parse_packet(OP_RESPAWN_WINDOW, &duplicate)
                .unwrap()
                .is_err()
        );
        let mut invalid_flag = exact;
        invalid_flag[16 + 25 + "Bind Location".len()] = 2;
        assert!(
            parse_packet(OP_RESPAWN_WINDOW, &invalid_flag)
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn bind_zero_zone_and_footer_are_preserved_without_guessing_resources() {
        let data = bind_fixture();
        let DeathEvent::BindTransfer(bind) = parse_packet(OP_ZONE_PLAYER_TO_BIND, &data)
            .unwrap()
            .unwrap()
        else {
            panic!("bind")
        };
        assert_eq!((bind.zone_id, bind.instance_id), (0, 7));
        assert_eq!(bind.position, [12.5, -34.25, 56.75]);
        assert_eq!(bind.heading, 128.);
        assert_eq!(bind.label, "Bind Location");
        assert_eq!(bind.save_items, 1);
        assert_eq!(bind.resources, [0; 3]);
        for size in 0..data.len() {
            assert!(
                parse_packet(OP_ZONE_PLAYER_TO_BIND, &data[..size])
                    .unwrap()
                    .is_err()
            );
        }
        let mut extra = data;
        extra.push(0);
        assert!(
            parse_packet(OP_ZONE_PLAYER_TO_BIND, &extra)
                .unwrap()
                .is_err()
        );
    }

    #[test]
    fn resurrection_uses_rof2_size_yx_order_and_verbatim_reply() {
        let data = resurrection_fixture();
        let DeathEvent::ResurrectionOffer(offer) =
            parse_packet(OP_REZZ_REQUEST, &data).unwrap().unwrap()
        else {
            panic!("resurrection")
        };
        assert_eq!((offer.zone_id(), offer.instance_id()), (202, 17));
        assert_eq!(offer.position(), [123.25, -456.5, 78.75]);
        assert_eq!(offer.recipient(), "RecoveryTest");
        assert_eq!(offer.caster(), "ClericTest");
        assert_eq!(offer.corpse(), "RecoveryTest's corpse42");
        assert_eq!(offer.spell_id(), 388);
        assert_eq!(offer.action(), 0);
        for accept in [false, true] {
            let packet = encode_command(DeathCommand::AnswerResurrection {
                offer: offer.clone(),
                accept,
            })
            .unwrap();
            let mut expected = data.clone();
            u32_field(&mut expected, 224, u32::from(accept));
            assert_eq!(packet.opcode, OP_REZZ_ANSWER);
            assert_eq!(packet.data, expected);
        }
        for size in 0..data.len() {
            assert!(
                parse_packet(OP_REZZ_REQUEST, &data[..size])
                    .unwrap()
                    .is_err()
            );
        }
        let mut extra = data;
        extra.push(0);
        assert!(parse_packet(OP_REZZ_REQUEST, &extra).unwrap().is_err());
    }

    #[test]
    fn recovery_rejects_nonfinite_positions_and_unterminated_names() {
        for (opcode, data, offsets) in [
            (
                OP_RESPAWN_WINDOW,
                respawn_fixture(false),
                vec![24, 28, 32, 36],
            ),
            (OP_ZONE_PLAYER_TO_BIND, bind_fixture(), vec![4, 8, 12, 16]),
            (OP_REZZ_REQUEST, resurrection_fixture(), vec![8, 12, 16]),
        ] {
            for offset in offsets {
                for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                    let mut bad = data.clone();
                    float_field(&mut bad, offset, invalid);
                    assert!(parse_packet(opcode, &bad).unwrap().is_err());
                }
            }
        }
        for offset in [24, 92, 160] {
            let mut bad = resurrection_fixture();
            bad[offset..offset + 64].fill(b'A');
            assert!(parse_packet(OP_REZZ_REQUEST, &bad).unwrap().is_err());
        }
        let mut bad = bind_fixture();
        bad[20..].fill(b'A');
        assert!(parse_packet(OP_ZONE_PLAYER_TO_BIND, &bad).unwrap().is_err());
    }

    #[test]
    fn selection_is_exactly_one_u32_and_unknown_opcodes_are_unclaimed() {
        let packet = encode_command(DeathCommand::SelectRespawn { option_id: 2 }).unwrap();
        assert_eq!(packet.opcode, OP_RESPAWN_WINDOW);
        assert_eq!(packet.data, [2, 0, 0, 0]);
        assert!(parse_packet(OP_REZZ_ANSWER, &resurrection_fixture()).is_none());
        assert!(parse_packet(0x760d, &resurrection_fixture()).is_none());
        assert!(parse_packet(0, &[]).is_none());
    }
}
