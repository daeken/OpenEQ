//! Constructor bounds from executed original CreateOldEmitter; clocks remain
//! OpenEQ's completion-relative policy. These tests never open an audio device.
use super::*;
use openeq_assets::audio::{AudioCatalog, Mp3Index, SoundBank, SoundIdTable};

fn parsed(bases: [i32; 2], random: i32, kinds: [u8; 2]) -> ZoneAudio {
    let mut record = [0u8; 84];
    record[28..32].copy_from_slice(&100f32.to_le_bytes());
    for (side, base) in bases.into_iter().enumerate() {
        record[32 + side * 4..36 + side * 4].copy_from_slice(&base.to_le_bytes());
        record[48 + side * 4..52 + side * 4].copy_from_slice(&1i32.to_le_bytes());
        record[56 + side] = kinds[side];
    }
    record[40..44].copy_from_slice(&random.to_le_bytes());
    ZoneAudio::parse_eff(
        "test",
        &record,
        &SoundBank::parse("EMIT\nwind\n").unwrap(),
        &SoundIdTable::default(),
        &Mp3Index::default(),
    )
    .unwrap()
}

#[test]
fn native_constructor_bounds_and_explicit_large_value_policy() {
    for (base, random, expected) in [
        (i32::MIN, i32::MAX, [0, 0]),
        (-1, 100, [0, 0]),
        (0, i32::MAX, [0, 0]),
        (1, i32::MIN, [1, 1]),
        (1000, -1, [1000, 1000]),
        (1000, 0, [1000, 1000]),
        (1000, 1, [1500, 1500]),
        (1000, 100, [1500, 1599]),
        (86_399_499, 2, [86_399_999, 86_400_000]),
        (86_399_500, 2, [86_400_000, 86_400_000]),
        (86_399_501, 1, [86_400_000, 86_400_000]),
        (i32::MAX, 0, [86_400_000, 86_400_000]),
        (i32::MAX - 500, 1, [86_400_000, 86_400_000]),
        (i32::MAX, i32::MAX, [86_400_000, 86_400_000]),
    ] {
        assert_eq!(
            classic_ambient_delays(base, random),
            expected,
            "{base},{random}"
        );
    }
}

#[test]
fn parsed_day_and_night_apply_distinct_construction_and_keep_all_day_voice() {
    for bases in [[0, 1000], [-1, 1000], [1000, 0], [1000, 2000]] {
        let zone = parsed(bases, 100, [0; 2]);
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&zone);
        assert_eq!(scheduler.emitters.len(), 2);
        for (side, hour) in [12, 23].into_iter().enumerate() {
            let voices = scheduler.update(Duration::ZERO, [0.; 3], hour, true);
            assert_eq!(voices.len(), 1);
            assert_eq!(voices[0].continuous, bases[side] <= 0);
            let emitter = &scheduler.emitters[side];
            assert_eq!(
                emitter.delays[side],
                if bases[side] <= 0 {
                    [0, 0]
                } else {
                    [bases[side] as u64 + 500, bases[side] as u64 + 599]
                }
            );
        }
    }
    for base in [0, 1000] {
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&parsed([base; 2], 100, [0; 2]));
        assert_eq!(scheduler.emitters.len(), 1);
        let day = scheduler.update(Duration::ZERO, [0.; 3], 12, true);
        let night = scheduler.update(Duration::ZERO, [0.; 3], 23, true);
        assert_eq!(day[0].token, night[0].token);
        assert_eq!(day[0].continuous, base == 0);
    }
}

#[test]
fn repeat_delay_includes_offset_but_never_native_exclusive_upper_endpoint() {
    for (random, expected) in [(1, [1500, 1500]), (100, [1500, 1599])] {
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&parsed([1000; 2], random, [0; 2]));
        let mut now = Duration::ZERO;
        let mut observed = BTreeSet::new();
        for _ in 0..2048 {
            let voices = scheduler.update(now, [0.; 3], 12, true);
            assert_eq!(voices.len(), 1);
            assert!(!voices[0].continuous);
            scheduler.finished(voices[0].token, now, false);
            let next = scheduler.states.values().next().unwrap().next;
            let delay = (next - now).as_millis() as u64;
            assert!((expected[0]..=expected[1]).contains(&delay));
            observed.insert(delay);
            assert!(
                scheduler
                    .update(next - Duration::from_millis(1), [0.; 3], 12, true)
                    .is_empty()
            );
            now = next;
        }
        assert!(observed.contains(&expected[0]));
        assert!(observed.contains(&expected[1]));
    }
}

#[test]
fn classic_music_and_emt_keep_existing_delay_semantics() {
    let zone = parsed([0, 1000], 100, [1; 2]);
    for side in 0..2 {
        let emitter = Emitter::from_asset(side, &zone.emitters[0], side).unwrap();
        assert_eq!(emitter.channel, Channel::Music);
        assert_eq!(emitter.continuous, [true; 2]);
        assert_eq!(emitter.delays, [[0, 100], [1000, 1100]]);
    }
    let zone = ZoneAudio::parse_emt(
        "test",
        "2,wind.wav,0,0,1,0,0,1,0,0,0,20,100,0,0,1000,1100,0,0,0",
    )
    .unwrap();
    let emitter = Emitter::from_asset(0, &zone.emitters[0], 0).unwrap();
    assert_eq!(emitter.continuous, [false; 2]);
    assert_eq!(emitter.delays, [[1000, 1100]; 2]);
}

#[test]
#[ignore = "requires original EverQuest assets; metadata only, no device"]
fn original_randomized_ambience_has_native_offset_and_exclusive_endpoint() {
    let catalog = AudioCatalog::load(
        &openeq_assets::loader::default_client_dir().expect("original EverQuest assets"),
    )
    .unwrap();
    let zone = catalog.load_zone("ponightmare").unwrap();
    let source = zone
        .emitters
        .iter()
        .find(|source| {
            matches!(source,
                AudioEmitter::Classic(raw) if raw.record_index == 1
            )
        })
        .unwrap();
    let AudioEmitter::Classic(raw) = source else {
        unreachable!()
    };
    assert_eq!(raw.cooldown_ms[0], 30_000);
    assert_eq!(raw.random_delay_ms, 30_000);
    assert_eq!(raw.kinds[0], ClassicEmitterKind::Ambient);
    let emitter = Emitter::from_asset(2, source, 0).unwrap();
    assert!(emitter.files[0].is_some());
    assert_eq!(emitter.delays[0], [30_500, 60_499]);
    assert!(!emitter.continuous[0]);
}

#[test]
#[ignore = "requires original EverQuest assets; metadata only, no device"]
fn original_qeynos_zero_cooldown_ignores_random_and_loops() {
    let catalog = AudioCatalog::load(
        &openeq_assets::loader::default_client_dir().expect("original EverQuest assets"),
    )
    .unwrap();
    let zone = catalog.load_zone("qeynos").unwrap();
    let source = zone
        .emitters
        .iter()
        .find(|source| {
            matches!(source,
                AudioEmitter::Classic(raw) if raw.record_index == 15
            )
        })
        .unwrap();
    let AudioEmitter::Classic(raw) = source else {
        unreachable!()
    };
    assert_eq!(raw.cooldown_ms, [0, 0]);
    assert_eq!(raw.random_delay_ms, 50);
    assert_eq!(raw.sound_ids, [143, 144]);
    assert_eq!(raw.kinds, [ClassicEmitterKind::Ambient; 2]);
    let mut scheduler = Scheduler::default();
    scheduler.set_zone(&ZoneAudio {
        emitters: vec![source.clone()],
        ..Default::default()
    });
    for hour in [12, 23] {
        let voices = scheduler.update(Duration::ZERO, raw.position, hour, true);
        assert_eq!(voices.len(), 1);
        assert!(voices[0].continuous);
        assert_eq!(
            scheduler
                .states
                .values()
                .find(|state| state.token == Some(voices[0].token))
                .unwrap()
                .delay,
            [0, 0]
        );
    }
}
