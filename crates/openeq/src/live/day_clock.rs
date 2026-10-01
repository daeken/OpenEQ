//! Local progression between authoritative TimeOfDay packets.
//!
//! EQEmu and the original client advance one EQ minute per three real seconds.
//! Wire hours are 1..=24; keep that numbering for the public live clock and its
//! sky/audio consumers. Only display code subtracts one from the hour.

use std::time::Instant;

const SECONDS_PER_MINUTE: u64 = 3;
const MINUTES_PER_DAY: u64 = 24 * 60;

#[derive(Default)]
pub(super) struct DayClock {
    anchor: Option<Anchor>,
}

struct Anchor {
    received: Instant,
    minute_of_day: u16,
}

impl DayClock {
    /// A packet has no subminute phase, so every valid receipt starts a fresh
    /// minute, including corrections backwards or to the same displayed time.
    pub(super) fn synchronize(&mut self, hour: u8, minute: u8, received: Instant) -> bool {
        if !(1..=24).contains(&hour) || minute >= 60 {
            return false;
        }
        self.anchor = Some(Anchor {
            received,
            minute_of_day: u16::from(hour - 1) * 60 + u16::from(minute),
        });
        true
    }

    /// Derive from the receipt each time to retain fractional elapsed seconds
    /// through uneven foreground polls. Monotonic time deliberately avoids
    /// clock jumps caused by changes to the machine's wall clock.
    pub(super) fn at(&self, now: Instant) -> Option<(u8, u8)> {
        let anchor = self.anchor.as_ref()?;
        let elapsed = now.saturating_duration_since(anchor.received).as_secs();
        let advance = (elapsed / SECONDS_PER_MINUTE) % MINUTES_PER_DAY;
        let minute = (u64::from(anchor.minute_of_day) + advance) % MINUTES_PER_DAY;
        Some(((minute / 60) as u8 + 1, (minute % 60) as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn no_time_is_invented_before_a_valid_packet() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        for (hour, minute) in [(0, 0), (25, 0), (255, 59), (12, 60), (12, 255)] {
            assert!(!clock.synchronize(hour, minute, now));
        }
        assert_eq!(clock.at(now + Duration::from_secs(86_400)), None);
    }

    #[test]
    fn progresses_at_three_seconds_per_minute_without_polling_drift() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        assert!(clock.synchronize(12, 58, now));
        for (milliseconds, expected) in [
            (0, (12, 58)),
            (2_999, (12, 58)),
            (3_000, (12, 59)),
            (5_999, (12, 59)),
            (6_000, (13, 0)),
            (8_999, (13, 0)),
            (9_000, (13, 1)),
        ] {
            assert_eq!(
                clock.at(now + Duration::from_millis(milliseconds)),
                Some(expected)
            );
        }
    }

    #[test]
    fn every_wire_minute_advances_and_wraps_to_hour_one() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        for hour in 1..=24 {
            for minute in 0..60 {
                assert!(clock.synchronize(hour, minute, now));
                assert_eq!(clock.at(now), Some((hour, minute)));
                let expected = if minute < 59 {
                    (hour, minute + 1)
                } else {
                    (hour % 24 + 1, 0)
                };
                assert_eq!(clock.at(now + Duration::from_secs(3)), Some(expected));
            }
        }
    }

    #[test]
    fn long_gaps_wrap_whole_days_and_keep_partial_minutes() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        clock.synchronize(24, 59, now);
        let later = now + Duration::from_secs(3 * (1440 * 1000 + 61));
        assert_eq!(clock.at(later + Duration::from_millis(2_999)), Some((2, 0)));
        assert_eq!(clock.at(later + Duration::from_secs(3)), Some((2, 1)));
    }

    #[test]
    fn each_valid_packet_resets_the_anchor_and_fractional_phase() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        clock.synchronize(20, 10, now);
        let correction = now + Duration::from_millis(5_999);
        assert_eq!(clock.at(correction), Some((20, 11)));
        assert!(clock.synchronize(5, 20, correction));
        assert_eq!(
            clock.at(correction + Duration::from_millis(2_999)),
            Some((5, 20))
        );
        let repeated = correction + Duration::from_millis(2_999);
        assert!(clock.synchronize(5, 20, repeated));
        assert_eq!(
            clock.at(repeated + Duration::from_millis(2_999)),
            Some((5, 20))
        );
        assert_eq!(clock.at(repeated + Duration::from_secs(3)), Some((5, 21)));
    }

    #[test]
    fn invalid_packets_preserve_the_last_valid_anchor() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        clock.synchronize(24, 59, now);
        for (hour, minute) in [(0, 0), (25, 0), (255, 59), (12, 60), (12, 255)] {
            assert!(!clock.synchronize(hour, minute, now + Duration::from_secs(2)));
        }
        assert_eq!(clock.at(now + Duration::from_secs(3)), Some((1, 0)));
    }

    #[test]
    fn a_sample_before_receipt_cannot_reverse_or_wrap_the_clock() {
        let mut clock = DayClock::default();
        let now = Instant::now();
        clock.synchronize(7, 12, now + Duration::from_secs(3));
        assert_eq!(clock.at(now), Some((7, 12)));
    }
}
