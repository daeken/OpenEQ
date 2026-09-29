//! Player trade handshakes and escrow. Offer coin/item changes implicitly reset
//! both accept indicators. A finished window is not proof of a successful trade.
use crate::{AppPacket, gameplay::CoinType, wire::Reader, zone::ZoneError};

#[derive(Debug, Clone)]
pub enum TradeCommand {
    Request {
        to_id: u32,
        from_id: u32,
    },
    Acknowledge {
        to_id: u32,
        from_id: u32,
    },
    Busy {
        to_id: u32,
        from_id: u32,
    },
    Accept {
        player_id: u32,
    },
    Cancel {
        player_id: u32,
    },
    /// Requires an active acknowledged session and sufficient carried coins.
    /// EQEmu allows adding but not withdrawing escrowed coins; cancel to refund.
    OfferCoin {
        coin: CoinType,
        amount: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeEvent {
    Requested {
        to_id: u32,
        from_id: u32,
    },
    Acknowledged {
        to_id: u32,
        from_id: u32,
    },
    Busy {
        to_id: u32,
        from_id: u32,
    },
    Accepted {
        player_id: u32,
    },
    /// EQEmu rewrites player_id to the recipient. Reply once when an active
    /// session was cancelled remotely, so the recipient's escrow is refunded.
    Cancelled {
        player_id: u32,
        action: u32,
    },
    /// A delta to the partner's offer. EQEmu supplies the recipient's own ID.
    CoinsAdded {
        recipient_id: u32,
        coin: CoinType,
        amount: u32,
    },
    /// Emitted on both successful exchange and lore/no-drop rejection.
    Finished,
    WindowClosed,
    WindowClosed2,
}

pub fn encode_command(command: TradeCommand) -> Result<AppPacket, ZoneError> {
    let mut data = Vec::new();
    let opcode = match command {
        TradeCommand::Request { to_id, from_id }
        | TradeCommand::Acknowledge { to_id, from_id }
        | TradeCommand::Busy { to_id, from_id } => {
            if to_id == 0 || from_id == 0 || to_id == from_id {
                return Err(ZoneError::Malformed("trade participants"));
            }
            data.extend(to_id.to_le_bytes());
            data.extend(from_id.to_le_bytes());
            match command {
                TradeCommand::Request { .. } => 0x77b5,
                TradeCommand::Acknowledge { .. } => 0x14bf,
                _ => {
                    data.extend([1, 0xef, 0xff, 0xff]);
                    0x5505
                }
            }
        }
        TradeCommand::Accept { player_id } | TradeCommand::Cancel { player_id } => {
            if player_id == 0 {
                return Err(ZoneError::Malformed("trade player"));
            }
            data.extend(player_id.to_le_bytes());
            data.extend(0u32.to_le_bytes());
            if matches!(command, TradeCommand::Accept { .. }) {
                0x69e2
            } else {
                0x354c
            }
        }
        TradeCommand::OfferCoin { coin, amount } => {
            if amount == 0 || amount > i32::MAX as u32 {
                return Err(ZoneError::Malformed("trade coin amount"));
            }
            for value in [1, 3, coin as u32, coin as u32, amount] {
                data.extend(value.to_le_bytes());
            }
            0x0bcf
        }
    };
    Ok(AppPacket::new(opcode, data))
}

pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<TradeEvent, ZoneError>> {
    if !matches!(
        opcode,
        0x77b5 | 0x14bf | 0x5505 | 0x69e2 | 0x354c | 0x4206 | 0x3993 | 0x7349 | 0x40ef
    ) {
        return None;
    }
    Some(parse_known(opcode, data).ok_or(ZoneError::Malformed("trade packet")))
}
fn parse_known(opcode: u16, data: &[u8]) -> Option<TradeEvent> {
    let mut r = Reader(data);
    let event = match opcode {
        0x77b5 | 0x14bf | 0x5505 => {
            let to_id = r.u32()?;
            let from_id = r.u32()?;
            (to_id != 0 && from_id != 0 && to_id != from_id).then_some(())?;
            match opcode {
                0x77b5 => TradeEvent::Requested { to_id, from_id },
                0x14bf => TradeEvent::Acknowledged { to_id, from_id },
                _ => {
                    r.skip(4)?;
                    TradeEvent::Busy { to_id, from_id }
                }
            }
        }
        0x69e2 | 0x354c => {
            let player_id = r.u32()?;
            let action = r.u32()?;
            // Client::SendLogoutPackets also runs after death has transferred
            // the old entity ID to its corpse. That cancellation has ID zero
            // and groupActUpdate (7); it is not a malformed trade participant.
            (player_id != 0 || (opcode == 0x354c && action == 7)).then_some(())?;
            if opcode == 0x69e2 {
                TradeEvent::Accepted { player_id }
            } else {
                TradeEvent::Cancelled { player_id, action }
            }
        }
        0x4206 => {
            let recipient_id = r.u32()?;
            let coin = match r.u8()? {
                0 => CoinType::Copper,
                1 => CoinType::Silver,
                2 => CoinType::Gold,
                3 => CoinType::Platinum,
                _ => return None,
            };
            r.skip(3)?;
            let amount = r.u32()?;
            (recipient_id != 0 && amount <= i32::MAX as u32).then_some(())?;
            TradeEvent::CoinsAdded {
                recipient_id,
                coin,
                amount,
            }
        }
        0x3993 => TradeEvent::Finished,
        0x7349 => TradeEvent::WindowClosed,
        0x40ef => TradeEvent::WindowClosed2,
        _ => return None,
    };
    r.done().then_some(event)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn death_logout_cancellation_allows_zero_id_only_for_group_update() {
        let data = [0u32, 7]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>();
        assert_eq!(
            parse_packet(0x354c, &data).unwrap().unwrap(),
            TradeEvent::Cancelled {
                player_id: 0,
                action: 7
            }
        );
        assert!(parse_packet(0x69e2, &data).unwrap().is_err());
        assert!(parse_packet(0x354c, &[0; 8]).unwrap().is_err());
        for n in 0..8 {
            assert!(parse_packet(0x354c, &data[..n]).unwrap().is_err());
        }
    }
    #[test]
    fn handshakes_have_exact_sizes_and_ids() {
        for (command, event) in [
            (
                TradeCommand::Request {
                    to_id: 2,
                    from_id: 1,
                },
                TradeEvent::Requested {
                    to_id: 2,
                    from_id: 1,
                },
            ),
            (
                TradeCommand::Acknowledge {
                    to_id: 1,
                    from_id: 2,
                },
                TradeEvent::Acknowledged {
                    to_id: 1,
                    from_id: 2,
                },
            ),
            (
                TradeCommand::Busy {
                    to_id: 1,
                    from_id: 2,
                },
                TradeEvent::Busy {
                    to_id: 1,
                    from_id: 2,
                },
            ),
            (
                TradeCommand::Accept { player_id: 1 },
                TradeEvent::Accepted { player_id: 1 },
            ),
            (
                TradeCommand::Cancel { player_id: 1 },
                TradeEvent::Cancelled {
                    player_id: 1,
                    action: 0,
                },
            ),
        ] {
            let p = encode_command(command).unwrap();
            assert_eq!(parse_packet(p.opcode, &p.data).unwrap().unwrap(), event);
            for n in 0..p.data.len() {
                assert!(parse_packet(p.opcode, &p.data[..n]).unwrap().is_err());
            }
            let mut extra = p.data;
            extra.push(0);
            assert!(parse_packet(p.opcode, &extra).unwrap().is_err());
        }
        for opcode in [0x3993, 0x7349, 0x40ef] {
            assert!(parse_packet(opcode, &[]).unwrap().is_ok());
            assert!(parse_packet(opcode, &[0]).unwrap().is_err());
        }
    }
    #[test]
    fn coins_are_carried_to_trade_and_received_as_delta() {
        let packet = encode_command(TradeCommand::OfferCoin {
            coin: CoinType::Platinum,
            amount: 7,
        })
        .unwrap();
        assert_eq!(packet.opcode, 0x0bcf);
        assert_eq!(
            packet.data,
            [1u32, 3, 3, 3, 7]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let mut data = 17u32.to_le_bytes().to_vec();
        data.extend([3, 0xd2, 0x4f, 0]);
        data.extend(7u32.to_le_bytes());
        assert_eq!(
            parse_packet(0x4206, &data).unwrap().unwrap(),
            TradeEvent::CoinsAdded {
                recipient_id: 17,
                coin: CoinType::Platinum,
                amount: 7
            }
        );
        for n in 0..data.len() {
            assert!(parse_packet(0x4206, &data[..n]).unwrap().is_err());
        }
        data[4] = 4;
        assert!(parse_packet(0x4206, &data).unwrap().is_err());
        for amount in [0, u32::MAX] {
            assert!(
                encode_command(TradeCommand::OfferCoin {
                    coin: CoinType::Copper,
                    amount
                })
                .is_err()
            );
        }
        assert!(
            encode_command(TradeCommand::Request {
                to_id: 1,
                from_id: 1
            })
            .is_err()
        );
        assert!(encode_command(TradeCommand::Accept { player_id: 0 }).is_err());
    }
}
