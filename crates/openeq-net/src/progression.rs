//! Receive-only RoF2 progression messages, following EQEmu's senders and
//! `common/patches/rof2.cpp` at 4aceae18b94ffaafc08e2b17bc41cd72c77f795d.
//! Experience updates carry a bar ratio, not an absolute experience total.
use crate::{wire::u32_at, zone::ZoneError};

pub const OP_EXP_UPDATE: u16 = 0x20ed;
pub const OP_LEVEL_UPDATE: u16 = 0x1eec;
pub const OP_SKILL_UPDATE: u16 = 0x004c;
/// Scale used by EQEmu's normal experience-bar senders. Transient level-update
/// ratios can exceed this; preserve their wire value instead of rejecting them.
pub const EXPERIENCE_BAR_UNITS: u32 = 330;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressionEvent {
    Experience {
        bar_units: u32,
    },
    Level {
        level: u32,
        /// EQEmu can report the historical highest level here, rather than the
        /// immediately preceding level. Do not infer training-point gains.
        reported_old_level: u32,
        bar_units: u32,
    },
    SkillValue {
        /// Normal skill IDs and language IDs (100 + language) share this field.
        /// Keep unknown IDs raw; consumers must bound any collection indexing.
        wire_skill_id: u32,
        /// Base value from the server, without item modifiers or inferred caps.
        value: u32,
    },
}

/// Malformed recognized packets are errors. Unknown opcodes, including AA and
/// trainer traffic, remain available to other parsers. Reserved bytes are
/// consumed without exposing or requiring their observed contents.
pub fn parse_packet(opcode: u16, data: &[u8]) -> Option<Result<ProgressionEvent, ZoneError>> {
    let expected_length = match opcode {
        OP_EXP_UPDATE => 8,
        OP_LEVEL_UPDATE | OP_SKILL_UPDATE => 12,
        _ => return None,
    };
    if data.len() != expected_length {
        return Some(Err(ZoneError::Malformed("progression packet")));
    }
    let event = match opcode {
        OP_EXP_UPDATE => ProgressionEvent::Experience {
            bar_units: u32_at(data, 0)?,
            // The second u32 is not assigned by the inspected XP senders.
        },
        OP_LEVEL_UPDATE => ProgressionEvent::Level {
            level: u32_at(data, 0)?,
            reported_old_level: u32_at(data, 4)?,
            bar_units: u32_at(data, 8)?,
        },
        OP_SKILL_UPDATE => ProgressionEvent::SkillValue {
            wire_skill_id: u32_at(data, 0)?,
            value: u32_at(data, 4)?,
            // RoF2 writes an opaque four-byte suffix, not another skill value.
        },
        _ => unreachable!(),
    };
    Some(Ok(event))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    #[test]
    fn experience_preserves_ratio_and_ignores_uninitialized_second_word() {
        for bar_units in [0, 1, EXPERIENCE_BAR_UNITS, 331, u32::MAX] {
            for opaque in [0, 0x7654_3210, u32::MAX] {
                assert_eq!(
                    parse_packet(OP_EXP_UPDATE, &words(&[bar_units, opaque]))
                        .unwrap()
                        .unwrap(),
                    ProgressionEvent::Experience { bar_units }
                );
            }
        }
    }

    #[test]
    fn level_keeps_wide_fields_historical_old_level_and_transient_ratio() {
        for (level, reported_old_level, bar_units) in
            [(85, 90, 330), (256, 70_001, 331), (u32::MAX, 0, u32::MAX)]
        {
            assert_eq!(
                parse_packet(
                    OP_LEVEL_UPDATE,
                    &words(&[level, reported_old_level, bar_units])
                )
                .unwrap()
                .unwrap(),
                ProgressionEvent::Level {
                    level,
                    reported_old_level,
                    bar_units,
                }
            );
        }
    }

    #[test]
    fn skill_and_language_values_keep_full_width_and_ignore_reserved_suffix() {
        for wire_skill_id in [0, 77, 99, 100, 127, 128, u32::MAX] {
            for value in [0, 254, 255, 70_001, u32::MAX] {
                for opaque in [0, 0x3688_5001, u32::MAX] {
                    assert_eq!(
                        parse_packet(OP_SKILL_UPDATE, &words(&[wire_skill_id, value, opaque]))
                            .unwrap()
                            .unwrap(),
                        ProgressionEvent::SkillValue {
                            wire_skill_id,
                            value,
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn recognized_packets_require_exact_lengths() {
        for (opcode, length) in [
            (OP_EXP_UPDATE, 8),
            (OP_LEVEL_UPDATE, 12),
            (OP_SKILL_UPDATE, 12),
        ] {
            let mut data = vec![0; length];
            assert!(parse_packet(opcode, &data).unwrap().is_ok());
            for end in 0..length {
                assert!(parse_packet(opcode, &data[..end]).unwrap().is_err());
            }
            data.push(0);
            assert!(parse_packet(opcode, &data).unwrap().is_err());
        }
    }

    #[test]
    fn unknown_aa_and_training_opcodes_are_not_claimed() {
        for opcode in [0, 0xffff, 0x7d14, 0x1966, 0x4b64, 0x3bc9, 0x5f8e] {
            assert!(parse_packet(opcode, &[]).is_none());
            assert!(parse_packet(opcode, &[0; 12]).is_none());
        }
    }
}
