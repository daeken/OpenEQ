//! RoF2 group messages and item/quest-link activation. Membership is established
//! by received events; an invitation or an outgoing accept is not a roster.
use crate::{AppPacket, wire::Reader, zone::ZoneError};

pub const SAYLINK_ITEM_ID: u32 = 0xfffff;
pub const LINK_BODY_BYTES: usize = 56;

/// Numeric link metadata from the original 56-byte hexadecimal descriptor.
/// The visible label is deliberately absent: it is never sent as quest text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPayload {
    pub item_id: u32,
    pub augments: [u32; 6],
    pub hash: u32,
    pub ornament_icon: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Item,
    Quest { phrase_id: u32, silent: bool },
}
impl LinkPayload {
    pub fn parse_body(body: &str) -> Option<Self> {
        if body.len() != LINK_BODY_BYTES || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let hex = |offset, len| u32::from_str_radix(&body[offset..offset + len], 16).ok();
        let result = Self {
            item_id: hex(1, 5)?,
            augments: [
                hex(6, 5)?,
                hex(11, 5)?,
                hex(16, 5)?,
                hex(21, 5)?,
                hex(26, 5)?,
                hex(31, 5)?,
            ],
            hash: hex(48, 8)?,
            ornament_icon: hex(43, 5)?,
        };
        result.validate().ok()?;
        Some(result)
    }
    pub fn kind(&self) -> LinkKind {
        if self.item_id == SAYLINK_ITEM_ID {
            let silent = self.augments[1] > 0;
            LinkKind::Quest {
                phrase_id: self.augments[usize::from(silent)],
                silent,
            }
        } else {
            LinkKind::Item
        }
    }
    fn validate(&self) -> Result<(), ZoneError> {
        if self.item_id == 0
            || self.item_id > 0xfffff
            || self.augments.iter().any(|id| *id > 0xfffff)
            || self.ornament_icon > u16::MAX as u32
            || matches!(self.kind(), LinkKind::Quest { phrase_id: 0, .. })
        {
            return Err(ZoneError::Malformed("link descriptor"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum SocialCommand {
    Invite {
        inviter: String,
        invitee: String,
    },
    Accept {
        inviter: String,
        invitee: String,
    },
    Decline {
        inviter: String,
        invitee: String,
    },
    /// Target the local player first: the server gives current target priority.
    Leave {
        character: String,
    },
    MakeLeader {
        character: String,
        leader: String,
    },
    ActivateLink(LinkPayload),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMember {
    /// Present in a full roster, absent in incremental joins.
    pub index: Option<u32>,
    pub name: String,
    pub owner: String,
    pub mercenary: bool,
    /// Tank, assist, puller.
    pub roles: [bool; 3],
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialEvent {
    Invitation {
        inviter: String,
        invitee: String,
    },
    InvitationCancelled {
        inviter: String,
        invitee: String,
    },
    FollowAccepted {
        inviter: String,
        invitee: String,
    },
    Snapshot {
        leader: String,
        members: Vec<GroupMember>,
    },
    MemberJoined(GroupMember),
    MemberLeft {
        character: String,
        member: String,
    },
    Disbanded {
        character: String,
        member: String,
    },
    LeaderChanged(String),
    JoinAcknowledged,
}

fn write_name(data: &mut [u8], name: &str) -> Result<(), ZoneError> {
    if name.is_empty() || name.len() > 63 || name.chars().any(char::is_control) {
        return Err(ZoneError::Malformed("group character name"));
    }
    data[..name.len()].copy_from_slice(name.as_bytes());
    Ok(())
}
fn pair(size: usize, first: &str, second: &str) -> Result<Vec<u8>, ZoneError> {
    let mut data = vec![0; size];
    write_name(&mut data[..64], first)?;
    write_name(&mut data[64..128], second)?;
    Ok(data)
}
pub fn encode_command(command: SocialCommand) -> Result<AppPacket, ZoneError> {
    let (opcode, data) = match command {
        SocialCommand::Invite { inviter, invitee } => (0x6110, pair(148, &invitee, &inviter)?),
        SocialCommand::Accept { inviter, invitee } => (0x1649, pair(152, &inviter, &invitee)?),
        // EQEmu's shipped RoF2 config retains the obsolete OP_CancelInvite name.
        // Servers must map OP_GroupCancelInvite to its existing wire value 2a50.
        SocialCommand::Decline { inviter, invitee } => (0x2a50, pair(152, &inviter, &invitee)?),
        SocialCommand::Leave { character } => (0x4c10, pair(148, &character, &character)?),
        SocialCommand::MakeLeader { character, leader } => {
            let mut data = vec![0; 456];
            write_name(&mut data[4..68], &character)?;
            write_name(&mut data[68..132], &leader)?;
            (0x4229, data)
        }
        SocialCommand::ActivateLink(link) => {
            link.validate()?;
            let mut data = vec![0; 52];
            data[..4].copy_from_slice(&link.item_id.to_le_bytes());
            for (slot, augment) in link.augments.iter().enumerate() {
                data[4 + slot * 4..8 + slot * 4].copy_from_slice(&augment.to_le_bytes());
            }
            data[28..32].copy_from_slice(&link.hash.to_le_bytes());
            data[32..36].copy_from_slice(&4u32.to_le_bytes());
            data[48..50].copy_from_slice(&(link.ornament_icon as u16).to_le_bytes());
            (0x4cef, data)
        }
    };
    Ok(AppPacket::new(opcode, data))
}

fn name(reader: &mut Reader<'_>, empty: bool) -> Option<String> {
    let n = reader.0.iter().take(64).position(|b| *b == 0)?;
    let value = std::str::from_utf8(reader.take(n)?).ok()?.to_owned();
    reader.skip(1)?;
    (empty || !value.is_empty()).then_some(())?;
    (!value.chars().any(char::is_control)).then_some(value)
}
fn fixed(reader: &mut Reader<'_>, empty: bool) -> Option<String> {
    name(&mut Reader(reader.take(64)?), empty)
}

/// Unknown opcodes are left to other subsystems. Recognized malformed packets
/// return an error without clearing a valid roster or creating an invitation.
pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<SocialEvent, ZoneError>> {
    if !matches!(
        opcode,
        0x6110
            | 0x32c2
            | 0x1649
            | 0x2060
            | 0x6194
            | 0x3abb
            | 0x1ae5
            | 0x74da
            | 0x21b4
            | 0x7323
            | 0x2a50
    ) {
        return None;
    }
    Some(parse_known(opcode, data).ok_or(ZoneError::Malformed("group packet")))
}
fn parse_known(opcode: u16, data: &[u8]) -> Option<SocialEvent> {
    let mut r = Reader(data);
    let event = match opcode {
        0x6110 | 0x32c2 => {
            let invitee = fixed(&mut r, false)?;
            let inviter = fixed(&mut r, false)?;
            r.skip(20)?;
            SocialEvent::Invitation { inviter, invitee }
        }
        0x1649 | 0x2060 | 0x2a50 => {
            let inviter = fixed(&mut r, false)?;
            let invitee = fixed(&mut r, false)?;
            r.skip(24)?;
            if opcode == 0x2a50 {
                SocialEvent::InvitationCancelled { inviter, invitee }
            } else {
                SocialEvent::FollowAccepted { inviter, invitee }
            }
        }
        0x6194 => {
            r.u32()?; // EQEmu emits zero for the group ID.
            let count = r.u32()?;
            if !(1..=6).contains(&count) {
                return None;
            }
            let leader = name(&mut r, true)?;
            let mut members: Vec<GroupMember> = Vec::with_capacity(count as usize);
            for _ in 0..count {
                let index = r.u32()?;
                let member_name = name(&mut r, false)?;
                if index >= 6
                    || members.iter().any(|m| {
                        m.index == Some(index) || m.name.eq_ignore_ascii_case(&member_name)
                    })
                {
                    return None;
                }
                let mercenary = r.u16()? != 0;
                let owner = name(&mut r, true)?;
                // RoF2's EQEmu encoder supplies placeholder levels 70/65.
                // Derive a displayed level from actual spawn data instead.
                r.u32()?;
                let roles = [r.u8()? != 0, r.u8()? != 0, r.u8()? != 0];
                let offline = r.u32()? != 0;
                r.u32()?; // timestamp
                members.push(GroupMember {
                    index: Some(index),
                    name: member_name,
                    owner,
                    mercenary,
                    roles,
                    offline,
                });
            }
            SocialEvent::Snapshot { leader, members }
        }
        0x3abb => {
            let owner = fixed(&mut r, true)?;
            let member_name = fixed(&mut r, false)?;
            let mercenary = r.u8()? != 0;
            r.skip(19)?; // padding, level, group metadata
            SocialEvent::MemberJoined(GroupMember {
                index: None,
                name: member_name,
                owner,
                mercenary,
                roles: [false; 3],
                offline: false,
            })
        }
        0x1ae5 | 0x74da => {
            let character = fixed(&mut r, false)?;
            let member = fixed(&mut r, opcode == 0x1ae5)?;
            r.skip(20)?;
            if opcode == 0x1ae5 {
                SocialEvent::Disbanded { character, member }
            } else {
                SocialEvent::MemberLeft { character, member }
            }
        }
        0x21b4 => {
            r.skip(64)?;
            let leader = fixed(&mut r, false)?;
            r.skip(20)?;
            SocialEvent::LeaderChanged(leader)
        }
        0x7323 => {
            r.u32()?;
            SocialEvent::JoinAcknowledged
        }
        _ => return None,
    };
    r.done().then_some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roster() -> Vec<u8> {
        let mut data = vec![0; 4];
        data.extend(2u32.to_le_bytes());
        data.extend(b"Leader\0");
        for (i, value) in [b"Leader\0".as_slice(), b"Member\0".as_slice()]
            .into_iter()
            .enumerate()
        {
            data.extend((i as u32).to_le_bytes());
            data.extend(value);
            data.extend([0; 3]);
            data.extend(65u32.to_le_bytes());
            data.extend([0; 11]);
        }
        data
    }
    #[test]
    fn names_reverse_between_invite_and_accept() {
        let invite = encode_command(SocialCommand::Invite {
            inviter: "Leader".into(),
            invitee: "Member".into(),
        })
        .unwrap();
        assert_eq!(invite.opcode, 0x6110);
        assert_eq!(invite.data.len(), 148);
        assert_eq!(&invite.data[..7], b"Member\0");
        assert_eq!(
            parse_packet(invite.opcode, &invite.data).unwrap().unwrap(),
            SocialEvent::Invitation {
                inviter: "Leader".into(),
                invitee: "Member".into()
            }
        );
        let accept = encode_command(SocialCommand::Accept {
            inviter: "Leader".into(),
            invitee: "Member".into(),
        })
        .unwrap();
        assert_eq!(accept.data.len(), 152);
        assert_eq!(&accept.data[..7], b"Leader\0");
        let decline = encode_command(SocialCommand::Decline {
            inviter: "Leader".into(),
            invitee: "Member".into(),
        })
        .unwrap();
        assert!(matches!(
            parse_packet(decline.opcode, &decline.data),
            Some(Ok(SocialEvent::InvitationCancelled { .. }))
        ));
    }
    #[test]
    fn roster_and_incremental_events_are_authoritative_and_bounded() {
        let data = roster();
        let SocialEvent::Snapshot { leader, members } =
            parse_packet(0x6194, &data).unwrap().unwrap()
        else {
            panic!()
        };
        assert_eq!(leader, "Leader");
        assert_eq!(
            members.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            ["Leader", "Member"]
        );
        let mut join = vec![0; 148];
        write_name(&mut join[64..128], "Member").unwrap();
        assert!(matches!(
            parse_packet(0x3abb, &join),
            Some(Ok(SocialEvent::MemberJoined(_)))
        ));
        let mut disband = vec![0; 148];
        write_name(&mut disband[..64], "Member").unwrap();
        assert!(matches!(
            parse_packet(0x1ae5, &disband),
            Some(Ok(SocialEvent::Disbanded { .. }))
        ));
        assert!(matches!(parse_packet(0x74da, &disband), Some(Err(_))));
        for valid in [data, join, disband] {
            let opcode = if valid.len() != 148 {
                0x6194
            } else if valid[0] == 0 {
                0x3abb
            } else {
                0x1ae5
            };
            for n in 0..valid.len() {
                assert!(parse_packet(opcode, &valid[..n]).unwrap().is_err());
            }
            let mut trailing = valid;
            trailing.push(0);
            assert!(parse_packet(opcode, &trailing).unwrap().is_err());
        }
        let mut invalid = roster();
        invalid[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_packet(0x6194, &invalid).unwrap().is_err());
        assert!(parse_packet(0x0000, &[]).is_none());
    }
    #[test]
    fn invalid_names_and_descriptors_never_become_commands() {
        for name in ["", "A\0B", "A\nB", &"A".repeat(64)] {
            assert!(
                encode_command(SocialCommand::Leave {
                    character: name.into()
                })
                .is_err()
            );
        }
        assert!(LinkPayload::parse_body(&"0".repeat(56)).is_none());
        assert!(LinkPayload::parse_body(&"f".repeat(55)).is_none());
        assert!(LinkPayload::parse_body(&"é".repeat(28)).is_none());
    }
    #[test]
    fn fixed_packets_require_full_bounded_names_and_exact_lengths() {
        for (opcode, size) in [
            (0x6110, 148),
            (0x32c2, 148),
            (0x1649, 152),
            (0x2060, 152),
            (0x2a50, 152),
            (0x74da, 148),
            (0x21b4, 148),
        ] {
            let good = pair(size, "First", "Second").unwrap();
            assert!(parse_packet(opcode, &good).unwrap().is_ok());
            for n in 0..size {
                assert!(parse_packet(opcode, &good[..n]).unwrap().is_err());
            }
            let mut no_terminator = good.clone();
            no_terminator[64..128].fill(b'A');
            assert!(parse_packet(opcode, &no_terminator).unwrap().is_err());
            let mut extra = good;
            extra.push(0);
            assert!(parse_packet(opcode, &extra).unwrap().is_err());
        }
    }
    #[test]
    fn quest_link_activation_preserves_id_silence_and_hash() {
        let body = format!("0FFFFF{:05X}{:05X}{}12345678", 123, 456, "0".repeat(32));
        assert_eq!(body.len(), 56);
        let link = LinkPayload::parse_body(&body).unwrap();
        assert_eq!(
            link.kind(),
            LinkKind::Quest {
                phrase_id: 456,
                silent: true
            }
        );
        let packet = encode_command(SocialCommand::ActivateLink(link)).unwrap();
        assert_eq!(packet.opcode, 0x4cef);
        assert_eq!(packet.data.len(), 52);
        assert_eq!(&packet.data[..4], &0xfffffu32.to_le_bytes());
        assert_eq!(&packet.data[4..8], &123u32.to_le_bytes());
        assert_eq!(&packet.data[8..12], &456u32.to_le_bytes());
        assert_eq!(&packet.data[28..32], &0x12345678u32.to_le_bytes());
    }
}
