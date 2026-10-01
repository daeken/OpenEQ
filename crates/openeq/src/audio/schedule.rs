//! Device-free emitter selection. Distance curves remain first-pass client policy.
//! Native day selection uses raw server hours 5..=18 (the clock is 1..=24);
//! see docs/XMI_NATIVE_SELECTION.md. Opaque flags never become new behavior.
use openeq_assets::audio::{
    ActivePeriod, AudioEmitter, AudioReference, ClassicEmitterKind, EmtLoopMode, ZoneAudio,
    xmi::XmiSequenceOrdinal,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

pub const MAX_EFFECT_VOICES: usize = 32;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Ambience,
    Music,
}

#[derive(Clone, Debug)]
struct Emitter {
    id: usize,
    files: [Option<Track>; 2],
    channel: Channel,
    position: [f32; 3],
    full_radius: f32,
    max_radius: f32,
    activation: f32,
    global: bool,
    gains: [f32; 2],
    continuous: [bool; 2],
    delays: [[u64; 2]; 2],
    fades: [u64; 2],
    environment: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Track {
    file: String,
    sequence: Option<XmiSequenceOrdinal>,
}
fn supported_file(reference: &AudioReference) -> Option<Track> {
    match reference {
        AudioReference::File(file) if file.ends_with(".wav") || file.ends_with(".mp3") => {
            Some(Track {
                file: file.to_ascii_lowercase(),
                sequence: None,
            })
        }
        AudioReference::XmiSequence { file, sequence } => Some(Track {
            file: file.to_ascii_lowercase(),
            sequence: Some(XmiSequenceOrdinal(u16::try_from(*sequence).ok()?)),
        }),
        _ => None,
    }
}
fn millis(value: i32) -> u64 {
    u64::try_from(value).unwrap_or(0).min(86_400_000)
}
/// Classic kind-0 construction, converted to the scheduler's inclusive bounds.
/// Nonpositive base loops continuously regardless of random; positive random
/// adds 500 ms and has an exclusive native upper bound. Clamp final durations
/// to one day as OpenEQ policy instead of reproducing signed native overflow.
/// Timer anchoring and RNG remain scheduler policy; see CLASSIC_AMBIENT_COOLDOWNS.
fn classic_ambient_delays(base: i32, random: i32) -> [u64; 2] {
    if base <= 0 {
        return [0; 2];
    }
    let base = base as u64;
    let bounds = if random > 0 {
        [base + 500, base + random as u64 + 499]
    } else {
        [base; 2]
    };
    bounds.map(|delay| delay.min(86_400_000))
}

/// Native CreateOldEmitter kind-0 base level, before master gain. Positive
/// values attenuate in hundredths of a decibel; nonpositive values select the
/// 20% ambient default. The original wrapping NEG sends i32::MIN to silence.
/// See docs/CLASSIC_AMBIENT_LEVELS.md; this does not select EAL overrides.
fn classic_ambient_gain(level: i32) -> f32 {
    if level == i32::MIN || level > 10_000 {
        0.
    } else if level <= 0 {
        0.2
    } else {
        10f64.powf(-f64::from(level) / 2000.) as f32
    }
}

impl Emitter {
    fn from_asset(id: usize, asset: &AudioEmitter, side: usize) -> Option<Self> {
        Some(match asset {
            AudioEmitter::Classic(source) => {
                let all_day = source.is_all_day();
                if all_day && side == 1 {
                    return None;
                }
                let channel = match source.kinds[side] {
                    ClassicEmitterKind::Ambient => Channel::Ambience,
                    ClassicEmitterKind::Music => Channel::Music,
                    _ => return None,
                };
                if source.radius <= 0. {
                    return None;
                }
                let delays = source.cooldown_ms.map(|base| {
                    if channel == Channel::Ambience {
                        classic_ambient_delays(base, source.random_delay_ms)
                    } else {
                        let min = millis(base);
                        [min, min.saturating_add(millis(source.random_delay_ms))]
                    }
                });
                Self {
                    id,
                    files: std::array::from_fn(|period| {
                        (all_day || period == side)
                            .then(|| supported_file(&source.sounds[side]))
                            .flatten()
                    }),
                    channel,
                    position: source.position,
                    // Kind 0 is two-dimensional ambience: its radius activates
                    // the sound rather than locating a point source.
                    full_radius: source.radius,
                    max_radius: source.radius,
                    activation: source.radius,
                    global: false,
                    gains: [if channel == Channel::Music {
                        1.
                    } else {
                        classic_ambient_gain(source.raw_words[15 + side] as i32)
                    }; 2],
                    continuous: delays.map(|delay| channel == Channel::Music || delay == [0, 0]),
                    delays,
                    fades: [
                        0,
                        if channel == Channel::Music {
                            millis(source.raw_words[17] as i32)
                        } else {
                            150
                        },
                    ],
                    environment: channel == Channel::Ambience,
                }
            }
            AudioEmitter::Emt(source) => {
                if side == 1 {
                    return None;
                }
                let continuous = match source.loop_mode {
                    EmtLoopMode::Continuous => true,
                    EmtLoopMode::DelayedRepeat => false,
                    _ => return None,
                };
                let file = supported_file(&source.sound)?;
                let channel = if file.sequence.is_some() || file.file.ends_with(".mp3") {
                    Channel::Music
                } else {
                    Channel::Ambience
                };
                let files = match source.active_period {
                    ActivePeriod::Always => [Some(file.clone()), Some(file)],
                    ActivePeriod::Day => [Some(file), None],
                    ActivePeriod::Night => [None, Some(file)],
                    _ => return None,
                };
                let global = channel == Channel::Music
                    && source.max_audible_distance == 0.
                    && source.activation_range == 0.
                    && source.full_volume_radius == 0.;
                if !global && source.max_audible_distance <= 0. {
                    return None;
                }
                let min = millis(source.repeat_delay_ms[0]);
                let max = millis(source.repeat_delay_ms[1]).max(min);
                Self {
                    id,
                    files,
                    channel,
                    position: source.position,
                    full_radius: source.full_volume_radius.min(source.max_audible_distance),
                    max_radius: source.max_audible_distance,
                    activation: source.activation_range,
                    global,
                    gains: [source.gain.clamp(0., 1.); 2],
                    continuous: [continuous; 2],
                    delays: [[min, max]; 2],
                    fades: source.fade_ms.map(millis),
                    environment: source.environment_flag == Some(1),
                }
            }
        })
    }
    fn gain(&self, position: [f32; 3], retained: bool) -> f32 {
        if self.global {
            return 1.;
        }
        let distance = self
            .position
            .iter()
            .zip(position)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();
        let activation = if self.activation > 0. {
            self.activation.min(self.max_radius)
        } else {
            self.max_radius
        };
        if !distance.is_finite()
            || (!retained && distance > activation)
            || distance > self.max_radius * if retained { 1.03 } else { 1. }
        {
            return 0.;
        }
        if self.channel == Channel::Music || distance <= self.full_radius {
            return 1.;
        }
        ((self.max_radius - distance) / (self.max_radius - self.full_radius).max(0.001))
            .clamp(0., 1.)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Effect(usize),
    Music(Track),
}
#[derive(Clone, Debug)]
struct State {
    token: Option<u64>,
    next: Duration,
    file: Track,
    delay: [u64; 2],
}
#[derive(Clone, Debug)]
pub struct Voice {
    pub token: u64,
    pub file: String,
    pub sequence: Option<XmiSequenceOrdinal>,
    pub channel: Channel,
    pub gain: f32,
    pub continuous: bool,
    pub fade_in: Duration,
    pub fade_out: Duration,
}

#[derive(Default)]
pub struct Scheduler {
    emitters: Vec<Emitter>,
    states: BTreeMap<Key, State>,
    sequence: u64,
    random: u64,
    current_music: Option<Track>,
    failed: BTreeSet<Track>,
    active_regions: BTreeSet<usize>,
}
impl Scheduler {
    pub fn set_zone(&mut self, zone: &ZoneAudio) {
        self.emitters = zone
            .emitters
            .iter()
            .enumerate()
            .flat_map(|(id, source)| {
                (0..2).filter_map(move |side| Emitter::from_asset(id * 2 + side, source, side))
            })
            .collect();
        self.states.clear();
        self.failed.clear();
        self.active_regions.clear();
        self.current_music = None;
        // The sequence intentionally survives same-zone re-entry.
        self.random = 0x9e3779b97f4a7c15 ^ self.sequence;
    }
    pub fn finished(&mut self, token: u64, now: Duration, failed: bool) {
        let Some(state) = self
            .states
            .values_mut()
            .find(|state| state.token == Some(token))
        else {
            return;
        };
        state.token = None;
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        let [min, max] = state.delay;
        state.next = now.saturating_add(Duration::from_millis(min + self.random % (max - min + 1)));
        if failed {
            self.failed.insert(state.file.clone());
        }
    }
    pub fn update(
        &mut self,
        now: Duration,
        position: [f32; 3],
        hour: u8,
        environment: bool,
    ) -> Vec<Voice> {
        let period = usize::from(!(5..19).contains(&hour));
        let mut candidates = Vec::new();
        let mut active_regions = BTreeSet::new();
        for emitter in &self.emitters {
            if emitter.environment && !environment {
                continue;
            }
            let Some(file) = &emitter.files[period] else {
                continue;
            };
            if self.failed.contains(file) {
                continue;
            }
            let key = if emitter.channel == Channel::Music {
                Key::Music(file.clone())
            } else {
                Key::Effect(emitter.id)
            };
            let retained = self.active_regions.contains(&emitter.id);
            let gain = emitter.gain(position, retained) * emitter.gains[period];
            if gain > 0. {
                active_regions.insert(emitter.id);
                candidates.push((key, emitter, file, gain));
            }
        }
        self.active_regions = active_regions;
        // Stable order under equal gain; retain an audible music track through
        // overlap, avoiding restarts between adjacent regions using that track.
        candidates.sort_by(|a, b| b.3.total_cmp(&a.3).then(a.1.id.cmp(&b.1.id)));
        let music = candidates
            .iter()
            .find(|(_, e, file, _)| {
                e.channel == Channel::Music && self.current_music.as_ref() == Some(*file)
            })
            .or_else(|| {
                candidates
                    .iter()
                    .find(|(_, e, _, _)| e.channel == Channel::Music)
            })
            .map(|(_, _, file, _)| (*file).clone());
        self.current_music = music.clone();
        let mut kept = BTreeSet::new();
        let mut voices = Vec::new();
        let mut effects = 0;
        for (key, emitter, file, gain) in candidates {
            if emitter.channel == Channel::Music {
                if music.as_ref() != Some(file) || kept.contains(&key) {
                    continue;
                }
            } else {
                if effects == MAX_EFFECT_VOICES {
                    continue;
                }
            }
            kept.insert(key.clone());
            let state = self.states.entry(key).or_insert_with(|| State {
                token: None,
                next: Duration::ZERO,
                file: file.clone(),
                delay: emitter.delays[period],
            });
            if state.file != *file {
                state.file = file.clone();
                state.token = None;
                state.next = now;
            }
            state.delay = emitter.delays[period];
            if now < state.next {
                continue;
            }
            if emitter.channel != Channel::Music {
                effects += 1;
            }
            let token = *state.token.get_or_insert_with(|| {
                self.sequence = self.sequence.wrapping_add(1);
                self.sequence
            });
            voices.push(Voice {
                token,
                file: file.file.clone(),
                sequence: file.sequence,
                channel: emitter.channel,
                gain,
                continuous: emitter.continuous[period],
                fade_in: Duration::from_millis(emitter.fades[0]),
                fade_out: Duration::from_millis(emitter.fades[1]),
            });
        }
        for (key, state) in &mut self.states {
            if !kept.contains(key) {
                state.token = None;
            }
        }
        voices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn zone(rows: &str) -> ZoneAudio {
        ZoneAudio::parse_emt("test", rows).unwrap()
    }
    fn music() -> ZoneAudio {
        zone("2,anguish.mp3,0,0,1,0,0,1,7,-14,6,0,0,0,0,5000,5000,0,1,0")
    }
    fn tick(s: &mut Scheduler, ms: u64) -> Vec<Voice> {
        s.update(Duration::from_millis(ms), [0.; 3], 12, true)
    }
    #[test]
    fn repeat_is_anchored_to_completion_and_never_catches_up_after_stall() {
        let mut s = Scheduler::default();
        s.set_zone(&music());
        let first = tick(&mut s, 0).remove(0);
        assert_eq!(tick(&mut s, 500000)[0].token, first.token);
        s.finished(first.token, Duration::from_millis(500000), false);
        assert!(tick(&mut s, 504999).is_empty());
        let second = tick(&mut s, 800000);
        assert_eq!(second.len(), 1);
        assert_ne!(second[0].token, first.token);
    }
    #[test]
    fn zone_change_invalidates_old_completion_and_voice_tokens() {
        let mut s = Scheduler::default();
        s.set_zone(&music());
        let old = tick(&mut s, 0)[0].token;
        s.set_zone(&music());
        let new = tick(&mut s, 1)[0].token;
        assert_ne!(old, new);
        s.finished(old, Duration::from_secs(20), true);
        assert_eq!(tick(&mut s, 2)[0].token, new);
    }
    #[test]
    fn same_track_regions_reuse_music_and_environment_option_is_selective() {
        let mut s = Scheduler::default();
        s.set_zone(&zone("2,a.mp3,0,0,1,0,0,0,0,0,0,100,100,0,0,0,0,0,0,0\n2,a.mp3,0,0,1,0,0,0,150,0,0,100,100,0,0,0,0,0,0,0\n2,b.wav,0,0,1,0,0,0,0,0,0,20,100,0,0,0,0,0,0,1"));
        let first = tick(&mut s, 0);
        assert_eq!(first.len(), 2);
        let token = first
            .iter()
            .find(|v| v.channel == Channel::Music)
            .unwrap()
            .token;
        let next = s.update(Duration::from_secs(1), [150., 0., 0.], 12, false);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].token, token);
    }

    #[test]
    fn xmi_identity_includes_ordinal_and_failure_does_not_silence_other_sequences() {
        let mut metadata = zone("2,gfaydark.xmi,0,0,1,0,0,0,0,0,0,100,100,0,0,0,0,0,0,0");
        let AudioEmitter::Emt(first) = &mut metadata.emitters[0] else {
            panic!("EMT")
        };
        first.sound = AudioReference::XmiSequence {
            file: "gfaydark.xmi".into(),
            sequence: 0,
        };
        let mut second = first.clone();
        second.position = [150., 0., 0.];
        second.sound = AudioReference::XmiSequence {
            file: "gfaydark.xmi".into(),
            sequence: 1,
        };
        metadata.emitters.push(AudioEmitter::Emt(second));
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&metadata);
        let first = scheduler
            .update(Duration::ZERO, [0.; 3], 12, true)
            .remove(0);
        assert_eq!(first.sequence, Some(XmiSequenceOrdinal(0)));
        let second = scheduler
            .update(Duration::from_secs(1), [150., 0., 0.], 12, true)
            .remove(0);
        assert_eq!(second.sequence, Some(XmiSequenceOrdinal(1)));
        assert_ne!(first.token, second.token);
        scheduler.finished(second.token, Duration::from_secs(2), true);
        assert!(
            scheduler
                .update(Duration::from_secs(3), [150., 0., 0.], 12, true)
                .is_empty()
        );
        let restored = scheduler
            .update(Duration::from_secs(3), [0.; 3], 12, true)
            .remove(0);
        assert_eq!(restored.sequence, Some(XmiSequenceOrdinal(0)));
        assert_eq!(restored.channel, Channel::Music);
    }
    #[test]
    fn caps_voices_skips_unknown_modes_and_selects_day_night() {
        let rows = (0..40)
            .map(|i| format!("2,a{i}.wav,0,1,75,0,0,0,0,0,0,20,100,0,0,0,0,0,0,1"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut s = Scheduler::default();
        s.set_zone(&zone(&rows));
        let voices = tick(&mut s, 0);
        assert_eq!(voices.len(), 32);
        assert!(voices.iter().all(|v| v.gain == 1.));
        assert!(s.update(Duration::ZERO, [0.; 3], 23, true).is_empty());
        assert!(s.update(Duration::ZERO, [101.; 3], 12, true).is_empty());
        s.set_zone(&zone("2,a.wav,0,0,1,0,0,100,0,0,0,20,100,0,0,0,0,0,0,1"));
        assert!(tick(&mut s, 0).is_empty());
    }

    #[test]
    fn native_day_boundaries_use_unnormalized_server_hours() {
        for (hour, active) in [
            (1, false),
            (4, false),
            (5, true),
            (18, true),
            (19, false),
            (24, false),
        ] {
            let mut scheduler = Scheduler::default();
            scheduler.set_zone(&zone("2,day.wav,0,1,1,0,0,0,0,0,0,20,100,0,0,0,0,0,0,1"));
            assert_eq!(
                !scheduler
                    .update(Duration::ZERO, [0.; 3], hour, true)
                    .is_empty(),
                active,
                "raw hour {hour}"
            );
        }
    }

    #[test]
    fn delayed_emitters_on_cooldown_do_not_starve_a_ready_quieter_voice() {
        let mut rows = (0..MAX_EFFECT_VOICES)
            .map(|i| format!("2,delayed{i}.wav,0,0,1,0,0,1,0,0,0,20,100,0,0,10000,10000,0,0,0"))
            .collect::<Vec<_>>();
        rows.push("2,ready.wav,0,0,0.5,0,0,0,0,0,0,20,100,0,0,0,0,0,0,0".into());
        let mut s = Scheduler::default();
        s.set_zone(&zone(&rows.join("\n")));
        let initial = tick(&mut s, 0);
        assert_eq!(initial.len(), MAX_EFFECT_VOICES);
        assert!(
            initial
                .iter()
                .all(|voice| voice.file.starts_with("delayed"))
        );
        for voice in initial {
            s.finished(voice.token, Duration::from_millis(100), false);
        }
        let during_cooldown = tick(&mut s, 101);
        assert_eq!(during_cooldown.len(), 1);
        assert_eq!(during_cooldown[0].file, "ready.wav");
        assert_eq!(tick(&mut s, 10_099)[0].token, during_cooldown[0].token);
        let resumed = tick(&mut s, 10_100);
        assert_eq!(resumed.len(), MAX_EFFECT_VOICES);
        assert!(
            resumed
                .iter()
                .all(|voice| voice.file.starts_with("delayed"))
        );
    }

    #[test]
    fn shared_music_file_does_not_activate_an_unentered_region() {
        let mut s = Scheduler::default();
        s.set_zone(&zone(concat!(
            "2,a.mp3,0,0,1,0,0,0,0,0,0,100,100,0,30,0,0,0,0,0\n",
            "2,a.mp3,0,0,1,0,0,0,200,0,0,200,200,0,20,0,0,0,0,0"
        )));
        let first = tick(&mut s, 0)[0].token;
        let inside_a = s.update(Duration::from_secs(1), [90., 0., 0.], 12, true);
        assert_eq!(inside_a[0].token, first);
        // Outside A, within B's audible radius but never inside B's trigger.
        assert!(
            s.update(Duration::from_secs(2), [120., 0., 0.], 12, true)
                .is_empty()
        );
        let entered_b = s.update(Duration::from_secs(3), [190., 0., 0.], 12, true);
        assert_eq!(entered_b.len(), 1);
        assert_ne!(entered_b[0].token, first);
        let retained_b = s.update(Duration::from_secs(4), [120., 0., 0.], 12, true);
        assert_eq!(retained_b[0].token, entered_b[0].token);
    }

    #[test]
    fn classic_day_loop_and_night_repeat_use_their_own_period_delays() {
        let mut s = Scheduler::default();
        s.set_zone(&ZoneAudio {
            emitters: vec![AudioEmitter::Classic(
                openeq_assets::audio::ClassicEmitter {
                    record_index: 0,
                    position: [0.; 3],
                    radius: 100.,
                    kinds: [ClassicEmitterKind::Ambient; 2],
                    cooldown_ms: [0, 5000],
                    random_delay_ms: 0,
                    sound_ids: [1, 2],
                    sounds: [
                        AudioReference::File("day.wav".into()),
                        AudioReference::File("night.wav".into()),
                    ],
                    raw_words: [0; openeq_assets::audio::EFF_RECORD_BYTES / 4],
                },
            )],
            ..Default::default()
        });
        let day = tick(&mut s, 0);
        assert_eq!(day[0].file, "day.wav");
        assert!(day[0].continuous);
        let night = s.update(Duration::from_secs(1), [0.; 3], 23, true);
        assert_eq!(night[0].file, "night.wav");
        assert!(!night[0].continuous);
        s.finished(night[0].token, Duration::from_secs(2), false);
        assert!(
            s.update(Duration::from_secs(6), [0.; 3], 23, true)
                .is_empty()
        );
        assert_eq!(s.update(Duration::from_secs(7), [0.; 3], 23, true).len(), 1);
        assert!(s.update(Duration::from_secs(8), [0.; 3], 12, true)[0].continuous);
    }

    #[test]
    fn classic_independent_period_types_keep_channels_gain_and_environment_gates() {
        use openeq_assets::audio::{Mp3Index, SoundBank, SoundIdTable};
        let mut data = [0u8; 84];
        data[28..32].copy_from_slice(&100f32.to_le_bytes());
        data[48..52].copy_from_slice(&(-1i32).to_le_bytes());
        data[52..56].copy_from_slice(&1i32.to_le_bytes());
        data[56] = 1; // day music
        data[57] = 0; // night ambience
        data[64..68].copy_from_slice(&2000u32.to_le_bytes());
        let zone = ZoneAudio::parse_eff(
            "test",
            &data,
            &SoundBank::parse("EMIT\nnight\n").unwrap(),
            &SoundIdTable::default(),
            &Mp3Index::parse("day.mp3").unwrap(),
        )
        .unwrap();
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&zone);
        let day = scheduler.update(Duration::ZERO, [0.; 3], 12, false);
        assert_eq!(day.len(), 1);
        assert_eq!(day[0].file, "day.mp3");
        assert_eq!(day[0].channel, Channel::Music);
        assert_eq!(day[0].gain, 1.);
        assert!(
            scheduler
                .update(Duration::ZERO, [0.; 3], 23, false)
                .is_empty()
        );
        let night = scheduler.update(Duration::ZERO, [0.; 3], 23, true);
        assert_eq!(night.len(), 1);
        assert_eq!(night[0].file, "night.wav");
        assert_eq!(night[0].channel, Channel::Ambience);
        assert!((night[0].gain - 0.1).abs() < 1e-6);
        assert_ne!(day[0].token, night[0].token);
    }

    #[test]
    fn classic_native_all_day_coalescing_preserves_voice_and_type_two_suppresses_second() {
        use openeq_assets::audio::{Mp3Index, SoundBank, SoundIdTable};
        let mut data = [0u8; 84];
        data[28..32].copy_from_slice(&100f32.to_le_bytes());
        data[48..52].copy_from_slice(&1i32.to_le_bytes());
        data[52..56].copy_from_slice(&1i32.to_le_bytes());
        let bank = SoundBank::parse("EMIT\nwind\n").unwrap();
        let parse = |data: &[u8]| {
            ZoneAudio::parse_eff(
                "test",
                data,
                &bank,
                &SoundIdTable::default(),
                &Mp3Index::default(),
            )
            .unwrap()
        };
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&parse(&data));
        assert_eq!(scheduler.emitters.len(), 1);
        let day = scheduler.update(Duration::ZERO, [0.; 3], 12, true);
        let night = scheduler.update(Duration::ZERO, [0.; 3], 23, true);
        assert_eq!(day[0].token, night[0].token);
        // Unequal native level fields prevent all-day coalescing.
        data[64..68].copy_from_slice(&1000u32.to_le_bytes());
        scheduler.set_zone(&parse(&data));
        assert_eq!(scheduler.emitters.len(), 2);
        data[56] = 2;
        scheduler.set_zone(&parse(&data));
        assert!(
            scheduler
                .update(Duration::ZERO, [0.; 3], 12, true)
                .is_empty()
        );
        assert!(
            scheduler
                .update(Duration::ZERO, [0.; 3], 23, true)
                .is_empty()
        );
    }
}

#[cfg(test)]
#[path = "schedule_gain_tests.rs"]
mod gain_tests;

#[cfg(test)]
#[path = "schedule_cooldown_tests.rs"]
mod cooldown_tests;
