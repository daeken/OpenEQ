//! Motion permissions from server spawn appearance and the player's buffs.
//!
//! Wire gravity 3 means ordinary land/water movement, not "in water now".
//! Liquid occupancy comes from zone regions. Acceleration belongs in movement.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlayerGravity {
    #[default]
    Grounded,
    Flying,
    Levitating,
    Floating,
    LevitateWhileRunning,
}

impl PlayerGravity {
    pub fn from_wire_mode(mode: u8) -> Self {
        match mode {
            1 => Self::Flying,
            2 => Self::Levitating,
            4 => Self::Floating,
            5 => Self::LevitateWhileRunning,
            // Preserve ordinary gravity for Ground, Water, and unknown modes.
            _ => Self::Grounded,
        }
    }

    /// EQEmu sends levitate appearance to observers while the affected player
    /// receives the buff. A distinct explicit flight/float mode still wins.
    pub fn with_levitation_buff(self, buff_mode: Option<u8>) -> Self {
        if matches!(self, Self::Flying | Self::Floating) {
            return self;
        }
        match buff_mode {
            Some(2) => Self::Levitating,
            Some(5) => Self::LevitateWhileRunning,
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_wire_mode_keeps_land_gravity_and_unknown_modes_are_conservative() {
        for mode in [0, 3, 6, 255] {
            assert_eq!(PlayerGravity::from_wire_mode(mode), PlayerGravity::Grounded);
        }
        assert_eq!(PlayerGravity::from_wire_mode(1), PlayerGravity::Flying);
        assert_eq!(PlayerGravity::from_wire_mode(2), PlayerGravity::Levitating);
        assert_eq!(PlayerGravity::from_wire_mode(4), PlayerGravity::Floating);
        assert_eq!(
            PlayerGravity::from_wire_mode(5),
            PlayerGravity::LevitateWhileRunning
        );
    }

    #[test]
    fn own_buff_grants_levitation_without_overwriting_explicit_flight() {
        assert_eq!(
            PlayerGravity::Grounded.with_levitation_buff(Some(2)),
            PlayerGravity::Levitating
        );
        assert_eq!(
            PlayerGravity::Grounded.with_levitation_buff(Some(5)),
            PlayerGravity::LevitateWhileRunning
        );
        assert_eq!(
            PlayerGravity::Flying.with_levitation_buff(Some(2)),
            PlayerGravity::Flying
        );
        assert_eq!(
            PlayerGravity::Floating.with_levitation_buff(Some(2)),
            PlayerGravity::Floating
        );
    }
}
