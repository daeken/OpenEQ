//! Stock RoF2 trainer records, verified against EQEmu 4aceae18b94ff.
//! Opening is not a price quote and can trigger server specialization repair.
//! Completion reports assessed cost, not a money/practice balance or commit ID.
use crate::{AppPacket, wire::Reader, zone::ZoneError};

pub const OP_TRAINING: u16 = 0x1966;
pub const OP_END_TRAINING: u16 = 0x4d6b;
pub const OP_TRAIN_SKILL: u16 = 0x2a85;
pub const OP_TRAIN_SKILL_CONFIRM: u16 = 0x4b64;
pub const SKILL_COUNT: usize = 78;
pub const TRAINABLE_LANGUAGE_COUNT: usize = 26;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bank {
    Skill = 0,
    Language = 1,
}

/// A validated outgoing selection. Receive-only languages 26/27 cannot train.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    bank: Bank,
    id: u16,
}
impl Selection {
    pub fn new(bank: u16, id: u32) -> Result<Self, ZoneError> {
        let bank = match bank {
            0 if id < SKILL_COUNT as u32 => Bank::Skill,
            1 if id < TRAINABLE_LANGUAGE_COUNT as u32 => Bank::Language,
            _ => return Err(ZoneError::Malformed("trainer selection")),
        };
        Ok(Self {
            bank,
            id: id as u16,
        })
    }

    pub fn from_wire_id(id: u32) -> Option<Self> {
        match id {
            0..=77 => Self::new(0, id).ok(),
            100..=125 => Self::new(1, id - 100).ok(),
            _ => None,
        }
    }

    pub fn bank(self) -> Bank {
        self.bank
    }
    pub fn id(self) -> u16 {
        self.id
    }
    pub fn wire_id(self) -> u32 {
        u32::from(self.id) + if self.bank == Bank::Language { 100 } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainingCommand {
    Open {
        trainer_id: u32,
        player_id: u32,
    },
    End {
        trainer_id: u32,
        player_id: u32,
    },
    Train {
        trainer_id: u32,
        selection: Selection,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    /// Unknown incoming IDs remain raw and must never index a skill vector.
    pub wire_skill_id: u32,
    pub assessed_cost_copper: u32,
    /// EQEmu sets 0/1 by comparing the resulting value with 1. Not a skill value.
    pub new_skill: u8,
    pub trainer_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrainingEvent {
    Opened {
        /// Both IDs were echoed from the request, not independently assigned.
        trainer_id: u32,
        player_id: u32,
        /// Only 0..78 are server maxima. Entries 78..100 echo the request.
        skills: Box<[u32; 100]>,
    },
    Completed(Completion),
}

/// Pure construction only. No request token is invented in the opaque bytes.
pub fn encode_command(command: TrainingCommand) -> Result<AppPacket, ZoneError> {
    let (opcode, data) = match command {
        TrainingCommand::Open {
            trainer_id,
            player_id,
        }
        | TrainingCommand::End {
            trainer_id,
            player_id,
        } => {
            if trainer_id == 0 || player_id == 0 || trainer_id == player_id {
                return Err(ZoneError::Malformed("trainer participants"));
            }
            let open = matches!(command, TrainingCommand::Open { .. });
            let mut data = vec![0; if open { 448 } else { 8 }];
            data[..4].copy_from_slice(&trainer_id.to_le_bytes());
            data[4..8].copy_from_slice(&player_id.to_le_bytes());
            (if open { OP_TRAINING } else { OP_END_TRAINING }, data)
        }
        TrainingCommand::Train {
            trainer_id,
            selection,
        } => {
            let npc = u16::try_from(trainer_id)
                .ok()
                .filter(|id| *id != 0)
                .ok_or(ZoneError::Malformed("trainer purchase NPC"))?;
            let mut data = vec![0; 12];
            data[..2].copy_from_slice(&npc.to_le_bytes());
            data[4..6].copy_from_slice(&(selection.bank as u16).to_le_bytes());
            data[8..10].copy_from_slice(&selection.id.to_le_bytes());
            (OP_TRAIN_SKILL, data)
        }
    };
    Ok(AppPacket::new(opcode, data))
}

pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<TrainingEvent, ZoneError>> {
    let size = match opcode {
        OP_TRAINING => 448,
        OP_TRAIN_SKILL_CONFIRM => 76,
        _ => return None,
    };
    if data.len() != size {
        return Some(Err(ZoneError::Malformed("trainer packet")));
    }
    let event = (|| {
        let mut r = Reader(data);
        if opcode == OP_TRAINING {
            let trainer_id = r.u32()?;
            let player_id = r.u32()?;
            let mut skills = [0; 100];
            for skill in &mut skills {
                *skill = r.u32()?;
            }
            r.skip(40)?;
            Some(TrainingEvent::Opened {
                trainer_id,
                player_id,
                skills: Box::new(skills),
            })
        } else {
            let wire_skill_id = r.u32()?;
            let assessed_cost_copper = r.u32()?;
            let new_skill = r.u8()?;
            let name = r.take(64)?;
            let end = name.iter().position(|byte| *byte == 0)?;
            let trainer_name = String::from_utf8_lossy(&name[..end]).into_owned();
            r.skip(3)?;
            Some(TrainingEvent::Completed(Completion {
                wire_skill_id,
                assessed_cost_copper,
                new_skill,
                trainer_name,
            }))
        }
    })();
    Some(event.ok_or(ZoneError::Malformed("trainer packet")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_use_exact_fields_and_zero_reserved_bytes() {
        for (command, opcode, size) in [
            (
                TrainingCommand::Open {
                    trainer_id: 70_001,
                    player_id: 42,
                },
                OP_TRAINING,
                448,
            ),
            (
                TrainingCommand::End {
                    trainer_id: 70_001,
                    player_id: 42,
                },
                OP_END_TRAINING,
                8,
            ),
        ] {
            let packet = encode_command(command).unwrap();
            assert_eq!(packet.opcode, opcode);
            assert_eq!(packet.data.len(), size);
            assert_eq!(&packet.data[..8], &[113, 17, 1, 0, 42, 0, 0, 0]);
            assert!(packet.data[8..].iter().all(|v| *v == 0));
        }
        let selection = Selection::new(1, 25).unwrap();
        let packet = encode_command(TrainingCommand::Train {
            trainer_id: 65535,
            selection,
        })
        .unwrap();
        assert_eq!(packet.opcode, OP_TRAIN_SKILL);
        assert_eq!(packet.data, [255, 255, 0, 0, 1, 0, 0, 0, 25, 0, 0, 0]);
        for trainer_id in [0, 65536, u32::MAX] {
            assert!(
                encode_command(TrainingCommand::Train {
                    trainer_id,
                    selection
                })
                .is_err()
            );
        }
        for (bank, id) in [(0, 78), (0, u32::MAX), (1, 26), (1, 27), (2, 0), (65535, 0)] {
            assert!(Selection::new(bank, id).is_err());
        }
        for (bank, id, wire) in [(0, 0, 0), (0, 77, 77), (1, 0, 100), (1, 25, 125)] {
            let selection = Selection::new(bank, id).unwrap();
            assert_eq!(selection.wire_id(), wire);
            assert_eq!(Selection::from_wire_id(wire), Some(selection));
        }
        for id in [78, 99, 126, 127, u32::MAX] {
            assert!(Selection::from_wire_id(id).is_none());
        }
    }

    #[test]
    fn open_preserves_bounded_maxima_and_echoed_padding() {
        let mut packet = encode_command(TrainingCommand::Open {
            trainer_id: 7,
            player_id: 9,
        })
        .unwrap();
        for index in 0..100 {
            packet.data[8 + index * 4..12 + index * 4]
                .copy_from_slice(&(index as u32 + 1000).to_le_bytes());
        }
        packet.data[408..].fill(0xff);
        let TrainingEvent::Opened {
            trainer_id,
            player_id,
            skills,
        } = parse_packet(packet.opcode, &packet.data).unwrap().unwrap()
        else {
            panic!()
        };
        assert_eq!((trainer_id, player_id), (7, 9));
        assert_eq!((skills[77], skills[78], skills[99]), (1077, 1078, 1099));
    }

    #[test]
    fn completion_preserves_assessed_cost_and_bounds_name() {
        let mut data = vec![0xee; 76];
        data[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        data[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        data[8] = 2;
        data[9..13].copy_from_slice(b"Bob\0");
        assert_eq!(
            parse_packet(OP_TRAIN_SKILL_CONFIRM, &data)
                .unwrap()
                .unwrap(),
            TrainingEvent::Completed(Completion {
                wire_skill_id: u32::MAX,
                assessed_cost_copper: u32::MAX,
                new_skill: 2,
                trainer_name: "Bob".into(),
            })
        );
        data[9..73].fill(b'X');
        assert!(
            parse_packet(OP_TRAIN_SKILL_CONFIRM, &data)
                .unwrap()
                .is_err()
        );
        data[72] = 0;
        assert!(parse_packet(OP_TRAIN_SKILL_CONFIRM, &data).unwrap().is_ok());
    }

    #[test]
    fn exact_lengths_and_unmapped_end_response() {
        for (opcode, size) in [(OP_TRAINING, 448), (OP_TRAIN_SKILL_CONFIRM, 76)] {
            let data = vec![0; size];
            assert!(parse_packet(opcode, &data).unwrap().is_ok());
            for length in 0..size {
                assert!(parse_packet(opcode, &data[..length]).unwrap().is_err());
            }
            assert!(parse_packet(opcode, &vec![0; size + 1]).unwrap().is_err());
        }
        for opcode in [0, OP_END_TRAINING, OP_TRAIN_SKILL, 0x004c, 0x640c, 0xffff] {
            assert!(parse_packet(opcode, &[]).is_none());
        }
    }

    #[test]
    fn production_gameplay_router_encodes_and_decodes_checked_trainer_records() {
        let command = TrainingCommand::Open {
            trainer_id: 7,
            player_id: 9,
        };
        let packet =
            crate::gameplay::encode_command(crate::gameplay::Command::Training(command)).unwrap();
        let crate::gameplay::GameplayEvent::Training(TrainingEvent::Opened {
            trainer_id,
            player_id,
            ..
        }) = crate::gameplay::parse_packet(packet.opcode, &packet.data)
            .unwrap()
            .unwrap()
        else {
            panic!("trainer routing")
        };
        assert_eq!((trainer_id, player_id), (7, 9));
        assert!(
            crate::gameplay::parse_packet(OP_TRAIN_SKILL_CONFIRM, &[0; 75])
                .unwrap()
                .is_err()
        );
        let end = crate::gameplay::encode_command(crate::gameplay::Command::Training(
            TrainingCommand::End {
                trainer_id: 7,
                player_id: 9,
            },
        ))
        .unwrap();
        assert_eq!((end.opcode, end.data.len()), (OP_END_TRAINING, 8));
        assert!(crate::gameplay::parse_packet(OP_END_TRAINING, &[]).is_none());
    }
}
