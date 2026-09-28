//! RoF2 scroll scribing and item activation. `ItemVerifyRequest` asks the server
//! to execute the item click; its reply is not an instruction to cast again.
use crate::{AppPacket, inventory::InventorySlot, wire::Reader, zone::ZoneError};

/// The packed click-effect block in a serialized inventory item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClickEffect {
    pub spell_id: i32,
    pub required_level: u8,
    pub effect_type: u32,
    pub level: u8,
    /// -1 denotes unlimited charges; finite charges remain instance state.
    pub max_charges: i32,
    pub cast_time_ms: i32,
    pub recast_seconds: u32,
    /// -1 is an independent timer identified by item ID, not a shared group.
    pub recast_type: i32,
}

#[derive(Debug, Clone)]
pub enum ItemUseCommand {
    /// The matching scroll must already be on the cursor. The client chooses an
    /// empty book slot and rejects known spells before sending this request.
    Scribe { book_slot: u32, spell_id: u32 },
    /// Activates an owned item in equipment, general inventory, or a bag.
    Click { slot: InventorySlot, target_id: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemUseEvent {
    /// Acknowledges the request before the server performs casting checks.
    /// It does not establish cast success or consume a charge locally.
    Verified {
        slot: InventorySlot,
        spell_id: u32,
        target_id: u32,
    },
    Recast {
        recast_type: i32,
        seconds: u32,
        ignore_casting_requirement: bool,
    },
}

/// RoF2 permits bag clicks but not bank, cursor, merchant, or augment addresses.
pub fn supports_click(slot: InventorySlot) -> bool {
    slot.kind == 0
        && slot.augment.is_none()
        && match slot.bag {
            None => slot.slot <= 32,
            Some(index) => (23..=32).contains(&slot.slot) && index < 200,
        }
}

pub fn encode_command(command: ItemUseCommand) -> Result<AppPacket, ZoneError> {
    let mut data = Vec::new();
    let opcode = match command {
        ItemUseCommand::Scribe {
            book_slot,
            spell_id,
        } => {
            if book_slot >= 720 || spell_id == 0 || spell_id > 45000 {
                return Err(ZoneError::Malformed("scribe spell"));
            }
            for word in [book_slot, spell_id, 0, 0] {
                data.extend(word.to_le_bytes());
            }
            0x217c
        }
        ItemUseCommand::Click { slot, target_id } => {
            if !supports_click(slot) {
                return Err(ZoneError::Malformed("item click slot"));
            }
            slot.write(&mut data);
            data.extend(target_id.to_le_bytes());
            0x189c
        }
    };
    Ok(AppPacket::new(opcode, data))
}

pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<ItemUseEvent, ZoneError>> {
    if !matches!(opcode, 0x097b | 0x15a9) {
        return None;
    }
    let result = (|| {
        let mut reader = Reader(data);
        let event = match opcode {
            0x097b => ItemUseEvent::Verified {
                slot: InventorySlot::read(&mut reader)?,
                spell_id: reader.u32()?,
                target_id: reader.u32()?,
            },
            0x15a9 => ItemUseEvent::Recast {
                seconds: reader.u32()?,
                recast_type: reader.u32()? as i32,
                ignore_casting_requirement: match reader.u8()? {
                    0 => false,
                    1 => true,
                    _ => return None,
                },
            },
            _ => unreachable!(),
        };
        reader.done().then_some(event)
    })();
    Some(result.ok_or(ZoneError::Malformed("item use packet")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scribe_uses_book_slot_and_mode_zero() {
        let packet = encode_command(ItemUseCommand::Scribe {
            book_slot: 719,
            spell_id: 288,
        })
        .unwrap();
        assert_eq!(packet.opcode, 0x217c);
        assert_eq!(
            packet.data,
            [719u32, 288, 0, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert!(
            encode_command(ItemUseCommand::Scribe {
                book_slot: 720,
                spell_id: 288
            })
            .is_err()
        );
        assert!(
            encode_command(ItemUseCommand::Scribe {
                book_slot: 1,
                spell_id: 0
            })
            .is_err()
        );
    }

    #[test]
    fn clicks_preserve_bag_subslots_and_reject_non_owned_locations() {
        let slot = InventorySlot::possessions(25).in_bag(3);
        let packet = encode_command(ItemUseCommand::Click {
            slot,
            target_id: 1234,
        })
        .unwrap();
        assert_eq!(packet.opcode, 0x189c);
        assert_eq!(
            packet.data,
            vec![0, 0, 0, 0, 25, 0, 3, 0, 255, 255, 0, 0, 210, 4, 0, 0]
        );
        assert_eq!(slot.server_slot(), Some(4413));
        for slot in [
            InventorySlot::CURSOR,
            InventorySlot::bank(0),
            InventorySlot::shared_bank(0),
            InventorySlot::possessions(13).in_bag(0),
            InventorySlot::possessions(25).in_bag(200),
            InventorySlot {
                augment: Some(0),
                ..slot
            },
        ] {
            assert!(encode_command(ItemUseCommand::Click { slot, target_id: 1 }).is_err());
        }
        assert!(supports_click(InventorySlot::possessions(13)));
        assert!(supports_click(InventorySlot::possessions(32)));
    }

    #[test]
    fn verification_and_packed_recast_timer_are_bounded() {
        let slot = InventorySlot::possessions(25).in_bag(3);
        let mut reply = Vec::new();
        slot.write(&mut reply);
        reply.extend(288u32.to_le_bytes());
        reply.extend(1234u32.to_le_bytes());
        assert_eq!(
            parse_packet(0x097b, &reply).unwrap().unwrap(),
            ItemUseEvent::Verified {
                slot,
                spell_id: 288,
                target_id: 1234
            }
        );
        let timer = [20, 0, 0, 0, 9, 0, 0, 0, 1];
        assert_eq!(
            parse_packet(0x15a9, &timer).unwrap().unwrap(),
            ItemUseEvent::Recast {
                recast_type: 9,
                seconds: 20,
                ignore_casting_requirement: true
            }
        );
        for length in 0..timer.len() {
            assert!(parse_packet(0x15a9, &timer[..length]).unwrap().is_err());
        }
        let mut padded = timer.to_vec();
        padded.push(0);
        assert!(parse_packet(0x15a9, &padded).unwrap().is_err());
        assert!(parse_packet(0xffff, &timer).is_none());
    }
}
