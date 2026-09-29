//! Detect deliberate movement into authored zone volumes, not spawn/teleport placement.
use openeq_assets::zone_lines::{ZoneLine, ZoneLines};

#[derive(Default)]
pub struct ZoneTravel {
    previous: Option<[f32; 3]>,
    /// A refused crossing stays latched until the player backs away. In
    /// particular, a server rewind must not make held-forward input spam travel.
    latched: Option<(u32, [f32; 3])>,
}

impl ZoneTravel {
    /// New world: forget all crossings and establish its authoritative spawn.
    pub fn reset(&mut self, position: [f32; 3]) {
        *self = Self {
            previous: Some(position),
            latched: None,
        };
    }

    /// Server teleport/rewind: change the baseline without interpreting the
    /// teleport as walking, or unlatching a refused request.
    pub fn rebase(&mut self, position: [f32; 3]) {
        self.previous = Some(position);
    }

    pub fn observe(&mut self, lines: &ZoneLines, position: [f32; 3]) -> Option<ZoneLine> {
        let previous = self.previous.replace(position)?;
        if !previous.into_iter().chain(position).all(f32::is_finite) {
            return None;
        }
        let delta: [f32; 3] = std::array::from_fn(|i| position[i] - previous[i]);
        let distance2: f32 = delta.iter().map(|v| v * v).sum();
        // Normal movement is at most 15 units per clamped physics frame.
        // Larger displacements are development flight/teleports, not walking.
        if !(0.000001..=64. * 64.).contains(&distance2) {
            return None;
        }
        if self.latched.is_some_and(|(_, direction)| {
            delta.iter().zip(direction).map(|(a, b)| a * b).sum::<f32>() < -0.001
        }) {
            self.latched = None;
        }
        // Do not immediately send a player back after arriving inside a border.
        if lines.region_at(previous).is_some() {
            return None;
        }
        let crossed = lines.crossed(previous, position)?;
        if self
            .latched
            .is_some_and(|(number, _)| number == crossed.number)
        {
            return None;
        }
        self.latched = Some((crossed.number, delta));
        Some(crossed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires original Greater Faydark zone data"]
    fn walks_cross_thin_borders_but_spawn_teleport_and_refused_repeat_do_not() {
        let base = openeq_assets::loader::default_client_dir().unwrap();
        let lines = ZoneLines::load(&base, "gfaydark").unwrap();
        let outside = [2606., -55., 19.];
        let inside = [2619., -55., 19.];
        let beyond = [2630., -55., 19.];
        let mut travel = ZoneTravel::default();
        travel.reset(outside);
        assert_eq!(
            travel.observe(&lines, beyond).unwrap().number,
            4,
            "swept motion must see a volume even when both endpoints are outside"
        );
        // A cancel response rewinds outside. Held-forward input must not spam.
        travel.rebase(outside);
        assert!(travel.observe(&lines, [2609., -55., 19.]).is_none());
        assert!(travel.observe(&lines, inside).is_none());
        // Back away from the failed crossing, then deliberately try again.
        assert!(travel.observe(&lines, outside).is_none());
        assert_eq!(travel.observe(&lines, inside).unwrap().number, 4);
        // Arrival inside an exit must not immediately bounce to the old zone.
        travel.reset(inside);
        assert!(travel.observe(&lines, [2620., -55., 19.]).is_none());
        assert!(travel.observe(&lines, outside).is_none());
        assert_eq!(travel.observe(&lines, inside).unwrap().number, 4);
        travel.reset([0., 0., 19.]);
        assert!(travel.observe(&lines, inside).is_none());
        assert!(travel.observe(&lines, [2620., -55., 19.]).is_none());
        travel.reset([2606., -55., 120.]);
        assert!(
            travel.observe(&lines, [2630., -55., 120.]).is_none(),
            "flying above a border is not walking into its authored volume"
        );
        assert!(travel.observe(&lines, [f32::NAN, 0., 0.]).is_none());
        assert!(travel.observe(&lines, outside).is_none());
    }
}
