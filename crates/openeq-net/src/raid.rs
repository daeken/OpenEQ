//! Checked RoF2 raid records. EQEmu rebuilds rosters with create/add events,
//! not the unused variable roster struct in its headers. Outgoing commands
//! request changes; only received events establish membership and leadership.
use crate::{AppPacket, wire::u32_at, zone::ZoneError};

pub const MAX_MEMBERS: usize = 72;
pub const MAX_GROUPS: u8 = 12;
pub const MEMBERS_PER_GROUP: usize = 6;
pub const UPDATE_OPCODE: u16 = 0x3973;
pub const COMMAND_OPCODE: u16 = 0x55ac;
const GENERAL_SIZE: usize = 140;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RaidMember {
    pub name: String,
    pub class: u8,
    pub level: u8,
    /// Zero-based subgroup, or None for EQEmu's 0xffffffff unassigned value.
    pub group: Option<u8>,
    pub group_leader: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaidEvent {
    Invitation {
        inviter: String,
        invitee: String,
    },
    /// Starts or rebuilds a roster; it has no total count or end marker.
    Created {
        leader: String,
    },
    MemberAdded(RaidMember),
    /// Also used during subgroup moves; never means global disband by itself.
    MemberRemoved {
        name: String,
    },
    Disbanded,
    /// Zone-entry action10, using the unchanged136-byte ZoneInSendName record.
    NoRaid,
    LeaderChanged {
        leader: String,
    },
    LockChanged {
        locked: bool,
    },
    Motd {
        text: String,
    },
    Note {
        member: String,
        text: String,
    },
    /// Known leadership-AA layout, deliberately not interpreted as a leader change.
    LeadershipData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaidCommand {
    Invite {
        inviter: String,
        invitee: String,
    },
    Accept {
        inviter: String,
        invitee: String,
    },
    /// Self-only: this API cannot accidentally remove the selected other member.
    Leave {
        character: String,
    },
    MakeLeader {
        character: String,
        leader: String,
    },
}

fn read_name(data: &[u8], offset: usize) -> Option<String> {
    let field = data.get(offset..offset + 64)?;
    let length = field.iter().position(|b| *b == 0)?;
    let name = std::str::from_utf8(&field[..length]).ok()?;
    if name.is_empty() || name.chars().any(char::is_control) {
        return None;
    }
    Some(name.to_owned())
}

fn read_text(data: &[u8], offset: usize, size: usize) -> Option<String> {
    let field = data.get(offset..offset + size)?;
    let length = field.iter().position(|b| *b == 0)?;
    Some(String::from_utf8_lossy(&field[..length]).into_owned())
}

/// Unknown opcodes/actions remain opaque to this subsystem. A malformed known
/// action returns an error, never an empty roster or a fabricated invitation.
pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<RaidEvent, ZoneError>> {
    if opcode != UPDATE_OPCODE {
        return None;
    }
    let Some(action) = u32_at(data, 0) else {
        return Some(Err(ZoneError::Malformed("raid action")));
    };
    let expected = match action {
        0 => 148,
        1 | 5 | 8 | 17 | 18 | 20 => GENERAL_SIZE,
        // Client::SendZoneInPackets sends ZoneInSendName_Struct, not the
        // general raid record. The RoF2 encoder passes it through unchanged.
        10 => 136,
        14 | 30 => 392,
        35 => 1164,
        36 => 204,
        _ => return None,
    };
    let event = (data.len() == expected)
        .then(|| parse_known(action, data))
        .flatten();
    Some(event.ok_or(ZoneError::Malformed("raid packet")))
}

fn parse_known(action: u32, data: &[u8]) -> Option<RaidEvent> {
    Some(match action {
        0 => {
            let group = match u32_at(data, 136)? {
                u32::MAX => None,
                group if group < u32::from(MAX_GROUPS) => Some(group as u8),
                _ => return None,
            };
            let group_leader = match data[142] {
                0 => false,
                1 => true,
                _ => return None,
            };
            RaidEvent::MemberAdded(RaidMember {
                name: read_name(data, 4)?,
                class: data[140],
                level: data[141],
                group,
                group_leader,
            })
        }
        1 => RaidEvent::MemberRemoved {
            name: read_name(data, 4)?,
        },
        5 => RaidEvent::Disbanded,
        8 => RaidEvent::Created {
            leader: read_name(data, 72)?,
        },
        10 => RaidEvent::NoRaid,
        14 => RaidEvent::LeadershipData,
        17 | 18 => RaidEvent::LockChanged {
            locked: action == 17,
        },
        20 => RaidEvent::Invitation {
            inviter: read_name(data, 72)?,
            invitee: read_name(data, 4)?,
        },
        30 => RaidEvent::LeaderChanged {
            leader: read_name(data, 72)?,
        },
        35 => RaidEvent::Motd {
            text: read_text(data, 140, 1024)?,
        },
        36 => RaidEvent::Note {
            member: read_name(data, 72)?,
            text: read_text(data, 140, 64)?,
        },
        _ => return None,
    })
}

fn write_name(field: &mut [u8], name: &str) -> Result<(), ZoneError> {
    if name.is_empty() || name.len() > 63 || name.chars().any(char::is_control) {
        return Err(ZoneError::Malformed("raid character name"));
    }
    field[..name.len()].copy_from_slice(name.as_bytes());
    Ok(())
}

pub fn encode_command(command: RaidCommand) -> Result<AppPacket, ZoneError> {
    let (action, player, leader) = match &command {
        RaidCommand::Invite { inviter, invitee } => (3u32, invitee, inviter),
        RaidCommand::Accept { inviter, invitee } => (1, inviter, invitee),
        RaidCommand::Leave { character } => (5, character, character),
        RaidCommand::MakeLeader { character, leader } => (30, character, leader),
    };
    if matches!(
        command,
        RaidCommand::Invite { .. } | RaidCommand::Accept { .. }
    ) && player.eq_ignore_ascii_case(leader)
    {
        return Err(ZoneError::Malformed("raid invitation identity"));
    }
    let mut data = vec![0; GENERAL_SIZE];
    data[..4].copy_from_slice(&action.to_le_bytes());
    write_name(&mut data[4..68], player)?;
    write_name(&mut data[72..136], leader)?;
    Ok(AppPacket::new(COMMAND_OPCODE, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(action: u32, size: usize, player: &str, leader: &str) -> Vec<u8> {
        let mut data = vec![0; size];
        data[..4].copy_from_slice(&action.to_le_bytes());
        data[4..4 + player.len()].copy_from_slice(player.as_bytes());
        data[72..72 + leader.len()].copy_from_slice(leader.as_bytes());
        // Unknown bytes are not flags or part of either adjacent name.
        data[68..72].copy_from_slice(&0xaabbccddu32.to_le_bytes());
        data
    }
    fn parse(data: &[u8]) -> Result<RaidEvent, ZoneError> {
        parse_packet(UPDATE_OPCODE, data).unwrap()
    }

    #[test]
    fn records_follow_rof2_encoder_offsets_not_stale_header_comments() {
        let mut member = record(0, 148, "Fellowship", "Fellowship");
        member[136..140].copy_from_slice(&11u32.to_le_bytes());
        member[140] = 2;
        member[141] = 65;
        member[142] = 1;
        member[143..148].copy_from_slice(&[0xff, 8, 7, 6, 5]);
        assert_eq!(
            parse(&member).unwrap(),
            RaidEvent::MemberAdded(RaidMember {
                name: "Fellowship".into(),
                class: 2,
                level: 65,
                group: Some(11),
                group_leader: true,
            })
        );
        member[136..140].copy_from_slice(&u32::MAX.to_le_bytes());
        member[142] = 0;
        assert!(matches!(
            parse(&member),
            Ok(RaidEvent::MemberAdded(RaidMember {
                group: None,
                group_leader: false,
                ..
            }))
        ));

        assert_eq!(
            parse(&record(20, 140, "Companion", "Fellowship")).unwrap(),
            RaidEvent::Invitation {
                inviter: "Fellowship".into(),
                invitee: "Companion".into()
            }
        );
        assert_eq!(
            parse(&record(8, 140, "Fellowship", "Fellowship")).unwrap(),
            RaidEvent::Created {
                leader: "Fellowship".into()
            }
        );
        assert_eq!(
            parse(&record(30, 392, "Companion", "Companion")).unwrap(),
            RaidEvent::LeaderChanged {
                leader: "Companion".into()
            }
        );
        assert_eq!(
            parse(&record(14, 392, "", "")).unwrap(),
            RaidEvent::LeadershipData
        );
    }

    #[test]
    fn removal_absence_and_lock_are_distinct_events() {
        assert_eq!(
            parse(&record(1, 140, "Companion", "Companion")).unwrap(),
            RaidEvent::MemberRemoved {
                name: "Companion".into()
            }
        );
        for (action, event) in [
            (5, RaidEvent::Disbanded),
            (17, RaidEvent::LockChanged { locked: true }),
            (18, RaidEvent::LockChanged { locked: false }),
        ] {
            let mut data = record(action, 140, "", "");
            // These actions establish no member identity; placeholder names
            // and reserved bytes are intentionally ignored, never presented.
            data[4..136].fill(0xff);
            assert_eq!(parse(&data).unwrap(), event);
        }
    }

    #[test]
    fn zone_entry_no_raid_uses_unchanged_zone_in_send_name_record() {
        // Captured live action10 is136 bytes. Client::SendZoneInPackets sends
        // two adjacent64-byte names; ENCODE(OP_RaidUpdate) passes it through.
        let mut data = vec![0; 136];
        data[..4].copy_from_slice(&10u32.to_le_bytes());
        data[4..14].copy_from_slice(b"Companion\0");
        data[68..78].copy_from_slice(b"Companion\0");
        assert_eq!(parse(&data).unwrap(), RaidEvent::NoRaid);
        data[4..].fill(0xff); // No membership identity is inferred from names.
        assert_eq!(parse(&data).unwrap(), RaidEvent::NoRaid);
        assert!(parse(&record(10, 140, "Companion", "Companion")).is_err());
    }

    #[test]
    fn motd_and_notes_have_action_specific_text_and_identity() {
        let mut motd = record(35, 1164, "Fellowship", "recipient");
        motd[140..153].copy_from_slice(b"First\nSecond\0");
        assert_eq!(
            parse(&motd).unwrap(),
            RaidEvent::Motd {
                text: "First\nSecond".into()
            }
        );
        motd[140] = 0;
        assert_eq!(
            parse(&motd).unwrap(),
            RaidEvent::Motd {
                text: String::new()
            }
        );
        motd[140..].fill(b'x');
        assert!(parse(&motd).is_err());

        let mut note = record(36, 204, "not-the-member", "Companion");
        note[140..147].copy_from_slice(b"Healer\0");
        assert_eq!(
            parse(&note).unwrap(),
            RaidEvent::Note {
                member: "Companion".into(),
                text: "Healer".into()
            }
        );
        note[140..].fill(b'x');
        assert!(parse(&note).is_err());
    }

    #[test]
    fn known_packets_reject_every_truncation_and_trailing_byte() {
        for (action, size) in [
            (0, 148),
            (1, 140),
            (5, 140),
            (8, 140),
            (10, 136),
            (14, 392),
            (17, 140),
            (18, 140),
            (20, 140),
            (30, 392),
            (35, 1164),
            (36, 204),
        ] {
            let mut data = record(action, size, "Companion", "Fellowship");
            assert!(parse(&data).is_ok(), "valid action {action}");
            for length in 0..size {
                assert!(
                    parse(&data[..length]).is_err(),
                    "action {action}, length {length}"
                );
            }
            data.push(0);
            assert!(parse(&data).is_err(), "trailing byte, action {action}");
        }
    }

    #[test]
    fn malformed_names_groups_and_booleans_are_not_members() {
        let member = record(0, 148, "Companion", "Companion");
        for invalid in [12, 0xffff, u32::MAX - 1] {
            let mut data = member.clone();
            data[136..140].copy_from_slice(&invalid.to_le_bytes());
            assert!(parse(&data).is_err());
        }
        let mut data = member.clone();
        data[142] = 2;
        assert!(parse(&data).is_err());
        for bad_name in [
            vec![b'x'; 64],
            vec![0; 64],
            vec![0xff, 0],
            vec![b'X', b'\n', 0],
        ] {
            let mut data = member.clone();
            data[4..4 + bad_name.len()].copy_from_slice(&bad_name);
            assert!(parse(&data).is_err());
        }
        let mut invitation = record(20, 140, "Companion", "Fellowship");
        invitation[72..136].fill(b'x');
        assert!(parse(&invitation).is_err());
    }

    #[test]
    fn unknown_actions_and_other_opcodes_do_not_disconnect_or_mutate() {
        for action in [2u32, 3, 4, 6, 11, 13, 16, 999, u32::MAX] {
            assert!(parse_packet(UPDATE_OPCODE, &action.to_le_bytes()).is_none());
            assert!(parse_packet(UPDATE_OPCODE, &record(action, 140, "", "")).is_none());
        }
        assert!(parse_packet(COMMAND_OPCODE, &[]).is_none());
        assert!(parse_packet(0, &[]).is_none()); // RaidJoin has no RoF2 wire opcode.
    }

    #[test]
    fn commands_use_direction_specific_names_and_self_only_leave() {
        let cases = [
            (
                RaidCommand::Invite {
                    inviter: "Fellowship".into(),
                    invitee: "Companion".into(),
                },
                3,
                "Companion",
                "Fellowship",
            ),
            (
                RaidCommand::Accept {
                    inviter: "Fellowship".into(),
                    invitee: "Companion".into(),
                },
                1,
                "Fellowship",
                "Companion",
            ),
            (
                RaidCommand::Leave {
                    character: "Companion".into(),
                },
                5,
                "Companion",
                "Companion",
            ),
            (
                RaidCommand::MakeLeader {
                    character: "Fellowship".into(),
                    leader: "Companion".into(),
                },
                30,
                "Fellowship",
                "Companion",
            ),
        ];
        for (command, action, player, leader) in cases {
            let packet = encode_command(command).unwrap();
            assert_eq!(packet.opcode, 0x55ac);
            assert_eq!(packet.data.len(), 140);
            assert_eq!(&packet.data[..4], &(action as u32).to_le_bytes());
            assert_eq!(read_name(&packet.data, 4).as_deref(), Some(player));
            assert_eq!(read_name(&packet.data, 72).as_deref(), Some(leader));
            assert_eq!(&packet.data[68..72], &[0; 4]);
            assert_eq!(&packet.data[136..], &[0; 4]);
        }
        for character in [
            String::new(),
            "X".repeat(64),
            "Comp\0anion".into(),
            "Comp\nanion".into(),
        ] {
            assert!(encode_command(RaidCommand::Leave { character }).is_err());
        }
        for command in [
            RaidCommand::Invite {
                inviter: "Same".into(),
                invitee: "same".into(),
            },
            RaidCommand::Accept {
                inviter: "Same".into(),
                invitee: "same".into(),
            },
        ] {
            assert!(encode_command(command).is_err());
        }
    }
}
