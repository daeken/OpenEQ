//! Base-level regressions from executed original CreateOldEmitter instructions.
use super::*;
use openeq_assets::audio::{AudioCatalog, Mp3Index, SoundBank, SoundIdTable};

#[test]
fn native_classic_ambient_level_boundaries() {
    // Frozen original x87/CRT outputs; pow implementations may differ by one ULP.
    for (level, bits) in [
        (i32::MIN, 0),
        (i32::MIN + 1, 0x3e4c_cccd),
        (-10_000, 0x3e4c_cccd),
        (-600, 0x3e4c_cccd),
        (-1, 0x3e4c_cccd),
        (0, 0x3e4c_cccd),
        (1, 0x3f7f_b498),
        (600, 0x3f00_4dce),
        (2000, 0x3dcc_cccd),
        (10_000, 0x3727_c5ac),
        (10_001, 0),
        (i32::MAX, 0),
    ] {
        let actual = classic_ambient_gain(level);
        assert!(actual.to_bits().abs_diff(bits) <= 1, "{level}: {actual}");
        if bits == 0 {
            assert_eq!(actual, 0.);
        }
    }
}

#[test]
fn parsed_period_levels_reach_scheduled_voices_without_absolute_value() {
    for (day, night) in [
        (0i32, 2000i32),
        (600, -600),
        (-1, 10_001),
        (i32::MIN, i32::MAX),
    ] {
        let mut record = [0u8; 84];
        record[28..32].copy_from_slice(&100f32.to_le_bytes());
        record[48..52].copy_from_slice(&1i32.to_le_bytes());
        record[52..56].copy_from_slice(&1i32.to_le_bytes());
        record[60..64].copy_from_slice(&day.to_le_bytes());
        record[64..68].copy_from_slice(&night.to_le_bytes());
        let zone = ZoneAudio::parse_eff(
            "test",
            &record,
            &SoundBank::parse("EMIT\nwind\n").unwrap(),
            &SoundIdTable::default(),
            &Mp3Index::default(),
        )
        .unwrap();
        let mut scheduler = Scheduler::default();
        scheduler.set_zone(&zone);
        assert_eq!(scheduler.emitters.len(), 2);
        for (hour, level) in [(12, day), (23, night)] {
            let voices = scheduler.update(Duration::ZERO, [0.; 3], hour, true);
            let expected = classic_ambient_gain(level);
            if expected == 0. {
                assert!(voices.is_empty());
            } else {
                assert_eq!(voices.len(), 1);
                assert_eq!(voices[0].channel, Channel::Ambience);
                assert_eq!(voices[0].gain, expected);
            }
        }
    }
}

#[test]
#[ignore = "requires original EverQuest assets; metadata only, no device"]
fn original_gfay_default_ambience_is_twenty_percent() {
    let catalog = AudioCatalog::load(
        &openeq_assets::loader::default_client_dir().expect("original EverQuest assets"),
    )
    .unwrap();
    let zone = catalog.load_zone("gfaydark").unwrap();
    let source = zone
        .emitters
        .iter()
        .find(|source| {
            matches!(source,
                AudioEmitter::Classic(value) if value.record_index == 12
            )
        })
        .unwrap();
    let AudioEmitter::Classic(raw) = source else {
        unreachable!()
    };
    assert_eq!(raw.kinds, [ClassicEmitterKind::Ambient; 2]);
    assert_eq!([raw.raw_words[15], raw.raw_words[16]], [0, 0]);
    assert_eq!(raw.sound_ids, [162, 164]);
    for side in 0..2 {
        let emitter = Emitter::from_asset(24 + side, source, side).unwrap();
        assert_eq!(emitter.gains, [0.2; 2]);
        assert!(emitter.files[side].is_some());
        let mut scheduler = Scheduler::default();
        scheduler.emitters.push(emitter);
        let voices = scheduler.update(
            Duration::ZERO,
            raw.position,
            if side == 0 { 12 } else { 23 },
            true,
        );
        assert_eq!(voices.len(), 1);
        assert_eq!(voices[0].gain, 0.2);
        assert_eq!(voices[0].file, raw.sounds[side].file_name().unwrap());
    }
}
