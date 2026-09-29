//! Receive-only RoF2 guild records, following EQEmu's actual senders and
//! `common/patches/rof2.cpp` at 4aceae18b94ffaafc08e2b17bc41cd72c77f795d.
//! A directory is not membership; a roster has no reliable guild ID. Consumers
//! must scope those records to independently confirmed identity and the current
//! session. This module intentionally has no outgoing guild command API.
use crate::{
    wire::{Reader, u32_at},
    zone::ZoneError,
};
use std::collections::HashSet;

pub const GUILD_NONE: u32 = u32::MAX;
/// Client resource ceilings, not EQEmu guild-size or directory-count promises.
pub const MAX_DIRECTORY_ENTRIES: usize = 50_000;
pub const MAX_DIRECTORY_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MEMBERS: usize = 16_384;
pub const MAX_ROSTER_BYTES: usize = 8 * 1024 * 1024;
const MAX_NAME_BYTES: usize = 63;
const MAX_NOTE_BYTES: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildDirectoryEntry {
    pub id: u32,
    pub name: String,
}

/// Server-reported status when this record was sent, not continuous presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuildPresence {
    Unknown,
    Offline,
    /// Instance is intentionally absent: the roster encoder always writes zero.
    Online {
        zone_id: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMember {
    pub name: String,
    pub level: u32,
    pub class: u32,
    pub rank: u32,
    /// Member-add packets omit these flags and the public note.
    pub banker: Option<bool>,
    pub alt: Option<bool>,
    pub presence: GuildPresence,
    /// Original Unix timestamp; wire zero means no known last-seen time.
    pub last_seen: Option<u32>,
    pub public_note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuildEvent {
    Directory(Vec<GuildDirectoryEntry>),
    /// The prefix may be a guild or player name and is not authoritative. The
    /// following four bytes are uninitialized by EQEmu; neither is exposed.
    Roster {
        members: Vec<GuildMember>,
    },
    /// Has no guild ID. Validate recipient and current identity before applying.
    Motd {
        recipient: String,
        author: String,
        text: String,
    },
    MemberAdded {
        guild_id: u32,
        member: GuildMember,
    },
    MemberRemoved {
        guild_id: u32,
        name: String,
    },
    MemberRenamed {
        guild_id: u32,
        old_name: String,
        new_name: String,
    },
    MemberLevel {
        guild_id: u32,
        name: String,
        level: u32,
    },
    MemberRank {
        guild_id: u32,
        name: String,
        rank: u32,
        banker: bool,
        alt: bool,
    },
    MemberNote {
        guild_id: u32,
        name: String,
        note: String,
    },
    MemberDetails {
        guild_id: u32,
        name: String,
        last_seen: Option<u32>,
        presence: GuildPresence,
    },
    Renamed {
        guild_id: u32,
        name: String,
    },
    Deleted {
        guild_id: u32,
    },
}

/// Unknown opcodes and shared-layout variants remain opaque. A malformed known
/// record is an error, never an empty roster or a membership transition. Errors
/// deliberately contain no packet bytes (the roster contains uninitialized data).
pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<GuildEvent, ZoneError>> {
    let event = match opcode {
        0x507a => parse_directory(data),
        0x12a6 => parse_roster(data),
        0x3e13 | 0x4f1f => parse_motd(data),
        0x2925 => exact(data, 104).and_then(parse_add),
        0x3141 => exact(data, 68).and_then(|data| {
            Some(GuildEvent::MemberRemoved {
                guild_id: u32_at(data, 0)?,
                name: fixed_name(data, 4, false)?,
            })
        }),
        0x3b26 => exact(data, 132).and_then(|data| {
            Some(GuildEvent::MemberRenamed {
                guild_id: u32_at(data, 0)?,
                old_name: fixed_name(data, 4, false)?,
                new_name: fixed_name(data, 68, false)?,
            })
        }),
        0x1bd3 => exact(data, 72).and_then(|data| {
            Some(GuildEvent::MemberLevel {
                guild_id: u32_at(data, 0)?,
                name: fixed_name(data, 4, false)?,
                level: u32_at(data, 68)?,
            })
        }),
        0x0b9c => exact(data, 80).and_then(|data| {
            let flags = u32_at(data, 72)?;
            // Also OP_SetGuildRank. Its final u32 is 1, not presence; ignore it.
            Some(GuildEvent::MemberRank {
                guild_id: u32_at(data, 0)?,
                name: fixed_name(data, 8, false)?,
                rank: u32_at(data, 4)?,
                banker: flags & 1 != 0,
                alt: flags & 2 != 0,
            })
        }),
        0x01f9 => exact(data, 324).and_then(|data| {
            Some(GuildEvent::MemberNote {
                guild_id: u32_at(data, 0)?,
                name: fixed_name(data, 4, false)?,
                note: fixed_text(data, 68, 256)?,
            })
        }),
        0x69b9 => {
            // Shared by modern GuildMemberDetails (zone u32) and translated
            // legacy GuildMemberUpdate (zone u16 + instance u16). Their final
            // u32 distinguishes modern offline=1, but zero is ambiguous. Never
            // combine legacy instance bits into a fictional huge zone ID.
            if data.len() == 80 && u32_at(data, 76)? > 1 {
                return None;
            }
            exact(data, 80).and_then(|data| {
                Some(GuildEvent::MemberDetails {
                    guild_id: u32_at(data, 0)?,
                    name: fixed_name(data, 4, false)?,
                    last_seen: timestamp(u32_at(data, 72)?),
                    presence: if u32_at(data, 76)? == 1 {
                        GuildPresence::Offline
                    } else {
                        GuildPresence::Unknown
                    },
                })
            })
        }
        0x61db => exact(data, 68).and_then(|data| {
            Some(GuildEvent::Renamed {
                guild_id: u32_at(data, 0)?,
                name: fixed_name(data, 4, false)?,
            })
        }),
        0x6dab => exact(data, 4).and_then(|data| {
            Some(GuildEvent::Deleted {
                guild_id: u32_at(data, 0)?,
            })
        }),
        _ => return None,
    };
    Some(event.ok_or(ZoneError::Malformed("guild packet")))
}

fn exact(data: &[u8], size: usize) -> Option<&[u8]> {
    (data.len() == size).then_some(data)
}

fn timestamp(value: u32) -> Option<u32> {
    (value != 0).then_some(value)
}

fn name(bytes: &[u8], allow_empty: bool) -> Option<String> {
    let value = std::str::from_utf8(bytes).ok()?;
    if (!allow_empty && value.is_empty()) || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_owned())
}

fn fixed_name(data: &[u8], offset: usize, allow_empty: bool) -> Option<String> {
    let field = data.get(offset..offset + 64)?;
    let length = field.iter().position(|b| *b == 0)?;
    name(&field[..length], allow_empty)
}

fn fixed_text(data: &[u8], offset: usize, size: usize) -> Option<String> {
    let field = data.get(offset..offset + size)?;
    let length = field.iter().position(|b| *b == 0)?;
    Some(String::from_utf8_lossy(&field[..length]).into_owned())
}

fn variable_name(r: &mut Reader<'_>) -> Option<String> {
    let length = r.0.iter().take(MAX_NAME_BYTES + 1).position(|b| *b == 0)?;
    let value = name(r.take(length)?, false)?;
    r.skip(1)?;
    Some(value)
}

fn be_u32(r: &mut Reader<'_>) -> Option<u32> {
    Some(u32::from_be_bytes(r.take(4)?.try_into().ok()?))
}

fn be_u16(r: &mut Reader<'_>) -> Option<u16> {
    Some(u16::from_be_bytes(r.take(2)?.try_into().ok()?))
}

fn parse_directory(data: &[u8]) -> Option<GuildEvent> {
    if data.len() > MAX_DIRECTORY_BYTES {
        return None;
    }
    let mut r = Reader(data);
    r.skip(64)?;
    let count = r.u32()? as usize;
    // Each entry needs at least an ID, a nonempty name, and its terminator.
    if count > MAX_DIRECTORY_ENTRIES || count.checked_mul(6)? > r.0.len() {
        return None;
    }
    let mut entries = Vec::with_capacity(count);
    let mut ids = HashSet::with_capacity(count);
    for _ in 0..count {
        let id = r.u32()?;
        // RoF2's encoder only writes guild IDs below MAX_GUILD_ID=50000. Its
        // source count may include excluded IDs; reject any inconsistent packet
        // instead of interpreting its unwritten allocation tail as entries.
        if id >= 50_000 || !ids.insert(id) {
            return None;
        }
        entries.push(GuildDirectoryEntry {
            id,
            name: variable_name(&mut r)?,
        });
    }
    r.done().then_some(GuildEvent::Directory(entries))
}

fn parse_roster(data: &[u8]) -> Option<GuildEvent> {
    if data.len() > MAX_ROSTER_BYTES {
        return None;
    }
    let mut r = Reader(data);
    variable_name(&mut r)?; // Legacy player name or modern guild name; not identity.
    r.skip(4)?; // UNINITIALIZED sender bytes. Do not decode, validate, or expose.
    let count = be_u32(&mut r)? as usize;
    // Each member has 52 numeric bytes, nonempty name+NUL, and note NUL.
    if count > MAX_MEMBERS || count.checked_mul(55)? > r.0.len() {
        return None;
    }
    let mut members = Vec::with_capacity(count);
    let mut names = HashSet::with_capacity(count);
    for _ in 0..count {
        let name = variable_name(&mut r)?;
        if !names.insert(name.to_ascii_lowercase()) {
            return None;
        }
        let level = be_u32(&mut r)?;
        let flags = be_u32(&mut r)?;
        let class = be_u32(&mut r)?;
        let rank = be_u32(&mut r)?;
        let last_seen = timestamp(be_u32(&mut r)?);
        r.skip(5 * 4)?; // tribute fields and reserved values
        let public_note = Some(r.string(MAX_NOTE_BYTES)?);
        r.skip(2)?; // Encoder writes zero, not a reliable instance ID.
        let zone_id = u32::from(be_u16(&mut r)?);
        r.skip(8)?;
        members.push(GuildMember {
            name,
            level,
            class,
            rank,
            banker: Some(flags & 1 != 0),
            alt: Some(flags & 2 != 0),
            presence: if zone_id == 0 {
                GuildPresence::Offline
            } else {
                GuildPresence::Online { zone_id }
            },
            last_seen,
            public_note,
        });
    }
    r.done().then_some(GuildEvent::Roster { members })
}

fn parse_motd(data: &[u8]) -> Option<GuildEvent> {
    let data = exact(data, 648)?;
    Some(GuildEvent::Motd {
        recipient: fixed_name(data, 4, false)?,
        author: fixed_name(data, 68, true)?,
        text: fixed_text(data, 136, 512)?,
    })
}

fn parse_add(data: &[u8]) -> Option<GuildEvent> {
    let zone_id = u32_at(data, 32)?;
    Some(GuildEvent::MemberAdded {
        guild_id: u32_at(data, 0)?,
        member: GuildMember {
            name: fixed_name(data, 40, false)?,
            level: u32_at(data, 16)?,
            class: u32_at(data, 20)?,
            rank: u32_at(data, 24)?,
            banker: None,
            alt: None,
            // Unlike the full roster's documented offline zero, an add carries
            // only a location. Do not infer offline from an absent location.
            presence: if zone_id == 0 {
                GuildPresence::Unknown
            } else {
                GuildPresence::Online { zone_id }
            },
            last_seen: timestamp(u32_at(data, 36)?),
            public_note: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(opcode: u16, data: &[u8]) -> GuildEvent {
        parse_packet(opcode, data).unwrap().unwrap()
    }

    fn fixed_name_at(data: &mut [u8], offset: usize, value: &str) {
        data[offset..offset + 64].fill(0);
        data[offset..offset + value.len()].copy_from_slice(value.as_bytes());
    }

    fn put(data: &mut [u8], offset: usize, value: u32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    // Source-shaped fixtures from ENCODE(OP_GuildMemberList), not struct-memory
    // serialization: roster numeric fields are network order, names variable.
    fn roster(names: &[&str]) -> Vec<u8> {
        let mut data = b"Test Guild\0".to_vec();
        data.extend_from_slice(&[0xf3, 0x72, 0x99, 0x18]); // never a guild identity
        data.extend_from_slice(&(names.len() as u32).to_be_bytes());
        for (index, name) in names.iter().enumerate() {
            data.extend_from_slice(name.as_bytes());
            data.push(0);
            for value in [
                70_001u32,
                3,
                70_002,
                70_003,
                1_760_001_234,
                1,
                0,
                400,
                500,
                1,
            ] {
                data.extend_from_slice(&value.to_be_bytes());
            }
            data.extend_from_slice(b"Public note\0");
            data.extend_from_slice(&0u16.to_be_bytes());
            data.extend_from_slice(&(if index == 0 { 202u16 } else { 0 }).to_be_bytes());
            data.extend_from_slice(&1u32.to_be_bytes());
            data.extend_from_slice(&0u32.to_be_bytes());
        }
        data
    }

    fn directory(entries: &[(u32, &str)]) -> Vec<u8> {
        let mut data = vec![0; 64];
        data.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for (id, name) in entries {
            data.extend_from_slice(&id.to_le_bytes());
            data.extend_from_slice(name.as_bytes());
            data.push(0);
        }
        data
    }

    #[test]
    fn roster_uses_network_order_and_preserves_wide_member_values() {
        let GuildEvent::Roster { members } = parsed(0x12a6, &roster(&["Fellowship", "Companion"]))
        else {
            panic!()
        };
        assert_eq!(members.len(), 2);
        assert_eq!(
            members[0],
            GuildMember {
                name: "Fellowship".into(),
                level: 70_001,
                class: 70_002,
                rank: 70_003,
                banker: Some(true),
                alt: Some(true),
                presence: GuildPresence::Online { zone_id: 202 },
                last_seen: Some(1_760_001_234),
                public_note: Some("Public note".into()),
            }
        );
        assert_eq!(members[1].presence, GuildPresence::Offline);
    }

    #[test]
    fn roster_reserved_identity_bytes_and_legacy_prefix_never_escape() {
        let original = roster(&["Fellowship"]);
        let expected = parsed(0x12a6, &original);
        for bytes in [[0; 4], [255; 4], [1, 2, 3, 4]] {
            let mut data = original.clone();
            data[11..15].copy_from_slice(&bytes);
            assert_eq!(parsed(0x12a6, &data), expected);
        }
        let mut legacy = b"LegacyPlayer\0".to_vec();
        legacy.extend_from_slice(&original[11..]);
        assert_eq!(parsed(0x12a6, &legacy), expected);
        assert_eq!(
            parsed(0x12a6, &roster(&[])),
            GuildEvent::Roster { members: vec![] }
        );
    }

    #[test]
    fn variable_packets_reject_all_truncations_trailers_and_duplicate_keys() {
        for (opcode, data) in [
            (0x12a6, roster(&["Fellowship", "Companion"])),
            (
                0x507a,
                directory(&[(17, "Test Guild"), (49_999, "Another Guild")]),
            ),
        ] {
            for n in 0..data.len() {
                assert!(
                    parse_packet(opcode, &data[..n]).unwrap().is_err(),
                    "{opcode:x} prefix {n}"
                );
            }
            let mut extra = data.clone();
            extra.push(0);
            assert!(parse_packet(opcode, &extra).unwrap().is_err());
        }
        assert!(
            parse_packet(0x12a6, &roster(&["Fellowship", "fELLOWSHIP"]))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_packet(0x507a, &directory(&[(17, "One"), (17, "Two")]))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_packet(0x507a, &directory(&[(50_000, "Excluded")]))
                .unwrap()
                .is_err()
        );
        assert_eq!(
            parsed(0x507a, &directory(&[])),
            GuildEvent::Directory(vec![])
        );
        assert_eq!(
            parsed(0x507a, &directory(&[(17, "Test Guild")])),
            GuildEvent::Directory(vec![GuildDirectoryEntry {
                id: 17,
                name: "Test Guild".into()
            }])
        );
    }

    #[test]
    fn limits_bound_names_counts_and_payloads_before_allocation() {
        let max_name = "A".repeat(63);
        assert!(parse_packet(0x12a6, &roster(&[&max_name])).unwrap().is_ok());
        let long_name = "A".repeat(64);
        assert!(
            parse_packet(0x12a6, &roster(&[&long_name]))
                .unwrap()
                .is_err()
        );
        assert!(
            parse_packet(0x507a, &directory(&[(17, &long_name)]))
                .unwrap()
                .is_err()
        );
        for invalid_name in ["", "Bad\nName"] {
            assert!(
                parse_packet(0x12a6, &roster(&[invalid_name]))
                    .unwrap()
                    .is_err()
            );
        }
        for count in [(MAX_MEMBERS + 1) as u32, u32::MAX] {
            let mut data = roster(&[]);
            data[15..19].copy_from_slice(&count.to_be_bytes());
            assert!(parse_packet(0x12a6, &data).unwrap().is_err());
        }
        for count in [(MAX_DIRECTORY_ENTRIES + 1) as u32, u32::MAX] {
            let mut data = directory(&[]);
            put(&mut data, 64, count);
            assert!(parse_packet(0x507a, &data).unwrap().is_err());
        }
        assert!(
            parse_packet(0x12a6, &vec![0; MAX_ROSTER_BYTES + 1])
                .unwrap()
                .is_err()
        );
        assert!(
            parse_packet(0x507a, &vec![0; MAX_DIRECTORY_BYTES + 1])
                .unwrap()
                .is_err()
        );
        // A little-endian count of one is not a valid roster network-order count.
        let mut data = roster(&["Fellowship"]);
        data[15..19].copy_from_slice(&1u32.to_le_bytes());
        assert!(parse_packet(0x12a6, &data).unwrap().is_err());
    }

    #[test]
    fn roster_note_boundary_and_absent_last_seen_are_distinct_from_missing_fields() {
        let mut data = roster(&["Fellowship"]);
        // Header19, name11, then level/flags/class/rank before timestamp.
        data[46..50].fill(0);
        let note_start = 19 + 11 + 40;
        data.splice(note_start..note_start + 11, vec![b'n'; 255]);
        let GuildEvent::Roster { members } = parsed(0x12a6, &data) else {
            panic!()
        };
        assert_eq!(members[0].last_seen, None);
        assert_eq!(members[0].public_note.as_ref().unwrap().len(), 255);
        data.insert(note_start, b'n');
        assert!(parse_packet(0x12a6, &data).unwrap().is_err());
    }

    #[test]
    fn motd_uses_common_fixed_record_including_valid_empty_text() {
        // zone/guild.cpp::SendGuildMOTD sends common GuildMOTD_Struct unchanged.
        let mut data = vec![0xaa; 648];
        fixed_name_at(&mut data, 4, "Fellowship");
        fixed_name_at(&mut data, 68, "Companion");
        data[136..].fill(0);
        data[136..143].copy_from_slice(b"Welcome");
        let expected = GuildEvent::Motd {
            recipient: "Fellowship".into(),
            author: "Companion".into(),
            text: "Welcome".into(),
        };
        for opcode in [0x3e13, 0x4f1f] {
            assert_eq!(parsed(opcode, &data), expected);
            assert!(matches!(
                crate::gameplay::parse_packet(opcode, &data)
                    .unwrap()
                    .unwrap(),
                crate::gameplay::GameplayEvent::Guild(GuildEvent::Motd { .. })
            ));
        }
        fixed_name_at(&mut data, 68, "");
        data[136] = 0;
        assert_eq!(
            parsed(0x3e13, &data),
            GuildEvent::Motd {
                recipient: "Fellowship".into(),
                author: String::new(),
                text: String::new(),
            }
        );
        data[136..].fill(b'a');
        assert!(parse_packet(0x3e13, &data).unwrap().is_err());
    }

    #[test]
    fn incrementals_follow_common_struct_offsets_and_keep_missing_fields_unknown() {
        let mut add = vec![0xcc; 104];
        put(&mut add, 0, 17);
        for (offset, value) in [
            (16, 70_001),
            (20, 70_002),
            (24, 70_003),
            (28, 1),
            (32, 70_004),
            (36, 1234),
        ] {
            put(&mut add, offset, value);
        }
        fixed_name_at(&mut add, 40, "Fellowship");
        let GuildEvent::MemberAdded { guild_id, member } = parsed(0x2925, &add) else {
            panic!()
        };
        assert_eq!(guild_id, 17);
        assert_eq!(
            (member.level, member.class, member.rank),
            (70_001, 70_002, 70_003)
        );
        assert_eq!(member.presence, GuildPresence::Online { zone_id: 70_004 });
        assert_eq!(
            (member.banker, member.alt, member.public_note),
            (None, None, None)
        );
        put(&mut add, 32, 0);
        put(&mut add, 36, 0);
        let GuildEvent::MemberAdded { member, .. } = parsed(0x2925, &add) else {
            panic!()
        };
        assert_eq!(member.presence, GuildPresence::Unknown);
        assert_eq!(member.last_seen, None);

        let mut data = vec![0; 132];
        put(&mut data, 0, 17);
        fixed_name_at(&mut data, 4, "Fellowship");
        fixed_name_at(&mut data, 68, "Companion");
        assert_eq!(
            parsed(0x3b26, &data),
            GuildEvent::MemberRenamed {
                guild_id: 17,
                old_name: "Fellowship".into(),
                new_name: "Companion".into()
            }
        );
        assert_eq!(
            parsed(0x3141, &data[..68]),
            GuildEvent::MemberRemoved {
                guild_id: 17,
                name: "Fellowship".into()
            }
        );
        assert_eq!(
            parsed(0x61db, &data[..68]),
            GuildEvent::Renamed {
                guild_id: 17,
                name: "Fellowship".into()
            }
        );
        assert_eq!(
            parsed(0x6dab, &data[..4]),
            GuildEvent::Deleted { guild_id: 17 }
        );
        put(&mut data, 68, 70_001);
        assert_eq!(
            parsed(0x1bd3, &data[..72]),
            GuildEvent::MemberLevel {
                guild_id: 17,
                name: "Fellowship".into(),
                level: 70_001
            }
        );

        let mut note = vec![0; 324];
        put(&mut note, 0, 17);
        fixed_name_at(&mut note, 4, "Fellowship");
        note[68..74].copy_from_slice(b"Hello!");
        assert_eq!(
            parsed(0x01f9, &note),
            GuildEvent::MemberNote {
                guild_id: 17,
                name: "Fellowship".into(),
                note: "Hello!".into()
            }
        );
    }

    #[test]
    fn shared_opcodes_never_guess_presence_or_pack_instance_into_zone() {
        let mut data = vec![0; 80];
        put(&mut data, 0, 17);
        put(&mut data, 4, 70_003);
        fixed_name_at(&mut data, 8, "Fellowship");
        put(&mut data, 72, 3);
        for reserved in [0, 1, u32::MAX] {
            put(&mut data, 76, reserved);
            assert_eq!(
                parsed(0x0b9c, &data),
                GuildEvent::MemberRank {
                    guild_id: 17,
                    name: "Fellowship".into(),
                    rank: 70_003,
                    banker: true,
                    alt: true,
                }
            );
        }
        fixed_name_at(&mut data, 4, "Fellowship");
        put(&mut data, 68, (4321 << 16) | 202); // legacy nonzero instance
        put(&mut data, 72, 1234);
        put(&mut data, 76, 0);
        assert_eq!(
            parsed(0x69b9, &data),
            GuildEvent::MemberDetails {
                guild_id: 17,
                name: "Fellowship".into(),
                last_seen: Some(1234),
                presence: GuildPresence::Unknown,
            }
        );
        put(&mut data, 76, 1); // unambiguously modern offline, no location claim
        assert_eq!(
            parsed(0x69b9, &data),
            GuildEvent::MemberDetails {
                guild_id: 17,
                name: "Fellowship".into(),
                last_seen: Some(1234),
                presence: GuildPresence::Offline,
            }
        );
        put(&mut data, 76, 2);
        assert!(parse_packet(0x69b9, &data).is_none());
    }

    #[test]
    fn fixed_records_require_exact_size_and_terminated_names() {
        for (opcode, size, name_offset) in [
            (0x3e13, 648, 4),
            (0x4f1f, 648, 4),
            (0x2925, 104, 40),
            (0x3141, 68, 4),
            (0x3b26, 132, 4),
            (0x1bd3, 72, 4),
            (0x0b9c, 80, 8),
            (0x01f9, 324, 4),
            (0x69b9, 80, 4),
            (0x61db, 68, 4),
        ] {
            let mut data = vec![0; size];
            fixed_name_at(&mut data, name_offset, "Fellowship");
            if opcode == 0x3b26 {
                fixed_name_at(&mut data, 68, "Companion");
            }
            assert!(parse_packet(opcode, &data).unwrap().is_ok());
            for n in 0..data.len() {
                assert!(
                    parse_packet(opcode, &data[..n]).unwrap().is_err(),
                    "{opcode:x} prefix {n}"
                );
            }
            let mut extra = data.clone();
            extra.push(0);
            assert!(parse_packet(opcode, &extra).unwrap().is_err());
            data[name_offset..name_offset + 64].fill(b'a');
            assert!(parse_packet(opcode, &data).unwrap().is_err());
        }
        for n in 0..4 {
            assert!(parse_packet(0x6dab, &vec![0; n]).unwrap().is_err());
        }
        assert!(parse_packet(0x6dab, &[0; 5]).unwrap().is_err());
    }

    #[test]
    fn unsupported_guild_actions_and_outbound_opcodes_remain_opaque() {
        for opcode in [
            0x2958, 0x36e0, 0x0276, 0x7099, 0x7053, 0x1444, 0x0b0b, 0xffff,
        ] {
            assert!(parse_packet(opcode, &[]).is_none());
            assert!(parse_packet(opcode, &[0; 648]).is_none());
        }
    }
}
