use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use flate2::{Compression, write::ZlibEncoder};

use super::*;

fn eff(kind: u8, ids: [i32; 2]) -> Vec<u8> {
    let mut words = [0u32; 21];
    words[1] = 0xdead_beef;
    for (word, value) in words[4..8].iter_mut().zip([12.5f32, -20., 30., 75.]) {
        *word = value.to_bits();
    }
    words[8] = 5000;
    words[9] = 7000;
    words[10] = 1000;
    words[12] = ids[0] as u32;
    words[13] = ids[1] as u32;
    words[14] = u32::from(kind) | 0xaabb_cc00;
    words[17] = 2000;
    words[20] = 12345;
    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

fn emt() -> Vec<String> {
    "2,torch.wav,0,0,1.00,200,400,1,12.5,-20,30,5,25,0,20,5000,7000,0,1,1"
        .split(',')
        .map(str::to_owned)
        .collect()
}

fn classic(emitter: &AudioEmitter) -> &ClassicEmitter {
    match emitter {
        AudioEmitter::Classic(emitter) => emitter,
        _ => panic!("not EFF"),
    }
}

fn modern(emitter: &AudioEmitter) -> &EmtEmitter {
    match emitter {
        AudioEmitter::Emt(emitter) => emitter,
        _ => panic!("not EMT"),
    }
}

#[test]
fn classic_bank_indices_and_global_ids_are_distinct() {
    let bank =
        SoundBank::parse("EMIT\r\nbird\r\n\r\n../bad\r\nFourth.WAV\r\nLOOP\r\nwind\r\nnight\r\n")
            .unwrap();
    let global =
        SoundIdTable::parse("39^Death_M.WAV^\n162^actual_global.wav^\n39^wrong.wav^\n").unwrap();
    assert_eq!(
        bank.resolve_effect(1, &global),
        AudioReference::File("bird.wav".into())
    );
    assert!(matches!(
        bank.resolve_effect(2, &global),
        AudioReference::Unresolved {
            namespace: SoundNamespace::EmitBank,
            ..
        }
    ));
    assert!(matches!(
        bank.resolve_effect(3, &global),
        AudioReference::Unresolved { .. }
    ));
    assert_eq!(
        bank.resolve_effect(4, &global),
        AudioReference::File("fourth.wav".into())
    );
    assert_eq!(
        bank.resolve_effect(39, &global),
        AudioReference::File("death_m.wav".into())
    );
    assert_eq!(
        bank.resolve_effect(162, &global),
        AudioReference::File("wind.wav".into())
    );
    assert_eq!(
        global.resolve(162),
        AudioReference::File("actual_global.wav".into())
    );
    assert_eq!(bank.resolve_effect(0, &global), AudioReference::Silent);
    assert_eq!(
        global.diagnostics.entries[0].kind,
        DiagnosticKind::Duplicate
    );
    assert_eq!(bank.diagnostics.entries.len(), 1);
}

#[test]
fn mp3_table_is_one_based_and_preserves_empty_invalid_slots() {
    let table = Mp3Index::parse("First.MP3\r\n\r\n../bad.mp3\r\nFourth.mp3\r\n").unwrap();
    assert_eq!(
        table.resolve_music("zone", -1),
        AudioReference::File("first.mp3".into())
    );
    assert_eq!(
        table.resolve_music("zone", -4),
        AudioReference::File("fourth.mp3".into())
    );
    for id in [-2, -3, -5, i32::MIN] {
        assert_eq!(
            table.resolve_music("zone", id),
            AudioReference::Unresolved {
                id,
                namespace: SoundNamespace::Mp3Index
            }
        );
    }
    assert_eq!(
        table.resolve_music("Gfaydark", 4),
        AudioReference::XmiSequence {
            file: "gfaydark.xmi".into(),
            sequence: 4
        }
    );
    assert_eq!(table.resolve_music("zone", 0), AudioReference::Silent);
}

#[test]
fn classic_raw_fields_and_asymmetric_source_coordinates_survive() {
    let bank = SoundBank::parse("EMIT\nbird\nLOOP\nwind\n").unwrap();
    let global = SoundIdTable::default();
    let mp3 = Mp3Index::parse("first.mp3\nsecond.mp3").unwrap();
    let mut bytes = eff(0, [1, 162]);
    bytes.extend(eff(1, [-2, -2]));
    bytes.extend(eff(3, [1, 0]));
    bytes.extend(eff(99, [1, 0]));
    let zone = ZoneAudio::parse_eff("test", &bytes, &bank, &global, &mp3).unwrap();
    let first = classic(&zone.emitters[0]);
    assert_eq!(first.position, [12.5, -20., 30.]);
    assert_eq!(first.cooldown_ms, [5000, 7000]);
    assert_eq!(first.random_delay_ms, 1000);
    assert_eq!(first.raw_words[1], 0xdead_beef);
    assert_eq!(first.raw_words[14], 0xaabb_cc00);
    assert_eq!(first.raw_words[20], 12345);
    assert_eq!(
        first.sounds,
        [
            AudioReference::File("bird.wav".into()),
            AudioReference::File("wind.wav".into())
        ]
    );
    let music = classic(&zone.emitters[1]);
    assert_eq!(music.kind, ClassicEmitterKind::Music);
    assert_eq!(
        music.sounds,
        [
            AudioReference::File("second.mp3".into()),
            AudioReference::File("second.mp3".into())
        ]
    );
    assert_eq!(classic(&zone.emitters[2]).kind, ClassicEmitterKind::Effect3);
    assert_eq!(
        classic(&zone.emitters[3]).kind,
        ClassicEmitterKind::Unknown(99)
    );
    assert!(matches!(
        classic(&zone.emitters[3]).sounds[0],
        AudioReference::Unresolved {
            namespace: SoundNamespace::UnknownEmitter,
            ..
        }
    ));
}

#[test]
fn classic_rejects_truncation_nonfinite_geometry_and_unbounded_records() {
    let parse = |data: &[u8]| {
        ZoneAudio::parse_eff(
            "zone",
            data,
            &SoundBank::default(),
            &SoundIdTable::default(),
            &Mp3Index::default(),
        )
    };
    for length in [1, 83, 85] {
        assert!(parse(&vec![0; length]).is_err());
    }
    assert!(parse(&vec![0; (MAX_ZONE_EMITTERS + 1) * EFF_RECORD_BYTES]).is_err());
    let mut bytes = eff(0, [0, 0]);
    bytes[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
    bytes.extend(eff(0, [0, 0]));
    let mut sentinel = eff(0, [0, 0]);
    sentinel[28..32].copy_from_slice(&(-2f32).to_le_bytes());
    bytes.extend(sentinel);
    let result = parse(&bytes).unwrap();
    assert_eq!(result.emitters.len(), 2);
    assert_eq!(classic(&result.emitters[0]).record_index, 1);
    assert_eq!(classic(&result.emitters[1]).radius, -2.);
    assert_eq!(result.diagnostics.entries[0].entry, Some(1));
    assert_eq!(
        result.diagnostics.entries[1].kind,
        DiagnosticKind::Unsupported
    );
}

#[test]
fn emt_flexible_rows_keep_extensions_and_do_not_become_classic_kind_two() {
    let mut old = emt();
    old[0] = "1".into();
    old.pop();
    let plain = emt();
    let mut longer = emt();
    longer.push("0".into());
    let mut extended = longer.clone();
    extended.push("1".into());
    let text = [old, plain, longer, extended]
        .iter()
        .map(|r| r.join(","))
        .collect::<Vec<_>>()
        .join("\r\n");
    let result = ZoneAudio::parse_emt("zone", &text).unwrap();
    assert_eq!(result.emitters.len(), 4);
    assert_eq!(modern(&result.emitters[0]).environment_flag, None);
    assert_eq!(modern(&result.emitters[1]).environment_flag, Some(1));
    assert_eq!(modern(&result.emitters[2]).extensions, ["0"]);
    assert_eq!(modern(&result.emitters[3]).extensions, ["0", "1"]);
    assert_eq!(modern(&result.emitters[1]).position, [12.5, -20., 30.]);
    assert_eq!(modern(&result.emitters[1]).fade_ms, [200, 400]);
    assert_eq!(result.diagnostics.entries.len(), 1);
    assert_eq!(
        result.diagnostics.entries[0].kind,
        DiagnosticKind::Unsupported
    );
}

#[test]
fn emt_preserves_unknown_controls_and_boosted_gain_but_skips_corrupt_rows() {
    let mut unknown = emt();
    unknown[0] = "9".into();
    unknown[3] = "8".into();
    unknown[4] = "75.0".into();
    unknown[7] = "1147504798".into();
    let mut broken = emt();
    broken[10] = "88.50.70.00".into();
    let mut nonfinite = emt();
    nonfinite[8] = "NaN".into();
    let mut unsafe_name = emt();
    unsafe_name[1] = "../elsewhere.wav".into();
    let text = format!(
        "; comment\n{}\n{}\n{}\n{}\n{}",
        unknown.join(","),
        broken.join(","),
        nonfinite.join(","),
        unsafe_name.join(","),
        emt().join(",")
    );
    let result = ZoneAudio::parse_emt("zone", &text).unwrap();
    assert_eq!(result.emitters.len(), 2);
    let first = modern(&result.emitters[0]);
    assert_eq!(first.active_period, ActivePeriod::Unknown(8));
    assert_eq!(first.loop_mode, EmtLoopMode::Unknown(1147504798));
    assert_eq!(first.gain, 75.);
    assert_eq!(modern(&result.emitters[1]).line_number, 6);
    assert_eq!(
        result
            .diagnostics
            .entries
            .iter()
            .filter(|d| d.kind == DiagnosticKind::Malformed)
            .count(),
        3
    );
    assert_eq!(first.raw_fields[7], "1147504798");
}

#[test]
fn emt_music_and_none_references_are_typed_without_playback_assumptions() {
    let mut mp3 = emt();
    mp3[1] = "Anguish.MP3".into();
    let mut xmi = emt();
    xmi[1] = "GFAYDARK.XMI".into();
    xmi[17] = "4".into();
    let mut none = emt();
    none[1] = "None.WAV".into();
    let result = ZoneAudio::parse_emt(
        "zone",
        &[mp3, xmi, none]
            .iter()
            .map(|r| r.join(","))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    assert_eq!(
        modern(&result.emitters[0]).sound,
        AudioReference::File("anguish.mp3".into())
    );
    assert_eq!(
        modern(&result.emitters[1]).sound,
        AudioReference::XmiSequence {
            file: "gfaydark.xmi".into(),
            sequence: 4
        }
    );
    assert_eq!(modern(&result.emitters[2]).sound, AudioReference::Silent);
}

#[test]
fn metadata_limits_and_diagnostics_are_bounded() {
    assert!(ZoneAudio::parse_emt("../bad", "").is_err());
    assert!(ZoneAudio::parse_emt("zone", &"x".repeat(MAX_AUDIO_TEXT_BYTES + 1)).is_err());
    assert!(ZoneAudio::parse_emt("zone", &"bad\n".repeat(MAX_ZONE_EMITTERS + 1)).is_err());
    let result = ZoneAudio::parse_emt("zone", &"bad\n".repeat(MAX_DIAGNOSTICS + 10)).unwrap();
    assert_eq!(result.diagnostics.entries.len(), MAX_DIAGNOSTICS);
    assert_eq!(result.diagnostics.suppressed, 10);
    assert_eq!(asset_name("SoUnDs\\Wind.wav").unwrap(), "wind.wav");
    assert_eq!(asset_name("sounds/Wind.wav").unwrap(), "wind.wav");
    for name in [
        "",
        "..",
        "/root.wav",
        "sub/file.wav",
        "sub\\file.wav",
        "C:drive.wav",
        "https://host/x.wav",
        "nul\0.wav",
        "sounds/../file.wav",
        "sounds\\nested\\file.wav",
    ] {
        assert!(asset_name(name).is_err(), "{name:?}");
    }
}

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "openeq-audio-{}-{stamp}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, bytes: impl AsRef<[u8]>) {
        fs::write(self.0.join(name), bytes).unwrap();
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![0u8; 12];
    bytes[4..8].copy_from_slice(&crate::pfs::PFS_MAGIC.to_le_bytes());
    let mut entries = Vec::new();
    let mut names = (files.len() as u32).to_le_bytes().to_vec();
    let mut block = |crc: u32, body: &[u8]| {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();
        let offset = bytes.len() as u32;
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);
        entries.push((crc, offset, body.len() as u32));
    };
    for (index, (name, body)) in files.iter().enumerate() {
        block(index as u32 + 1, body);
        names.extend_from_slice(&(name.len() as u32 + 1).to_le_bytes());
        names.extend_from_slice(name.as_bytes());
        names.push(0);
    }
    block(crate::pfs::DIR_CRC, &names);
    let offset = bytes.len() as u32;
    bytes[..4].copy_from_slice(&offset.to_le_bytes());
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (crc, offset, size) in entries.into_iter().rev() {
        for value in [crc, offset, size] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

fn source_filename(asset: &AudioAsset) -> &Path {
    match &asset.location {
        AudioAssetLocation::Loose(path) | AudioAssetLocation::Archive { path, .. } => path,
    }
}

#[test]
fn index_reads_archive_only_assets_and_has_stable_numeric_overlay_priority() {
    let dir = TestDirectory::new();
    fs::create_dir(dir.0.join("SOUNDS")).unwrap();
    dir.write(
        "SND2.PFS",
        archive(&[
            ("common.wav", b"older"),
            ("archive_only.wav", b"original"),
            ("root.wav", b"old root"),
        ]),
    );
    dir.write("snd10.pfs", archive(&[("common.wav", b"newer")]));
    dir.write("Root.WAV", b"loose root");
    dir.write("SOUNDS/ROOT.wav", b"loose sounds wins");
    dir.write("Song.MP3", b"ID3 music");
    let catalog = AudioCatalog::load(&dir.0).unwrap();
    assert_eq!(catalog.len(), 4);
    assert_eq!(catalog.read("COMMON.WAV").unwrap().unwrap(), b"newer");
    assert_eq!(
        catalog.read("archive_only.wav").unwrap().unwrap(),
        b"original"
    );
    assert_eq!(
        catalog.read("root.wav").unwrap().unwrap(),
        b"loose sounds wins"
    );
    assert_eq!(catalog.read("song.mp3").unwrap().unwrap(), b"ID3 music");
    assert_eq!(
        source_filename(catalog.asset("common.wav").unwrap())
            .file_name()
            .unwrap(),
        "snd10.pfs"
    );
    assert_eq!(catalog.asset("song.mp3").unwrap().format, AudioFormat::Mp3);
    assert!(matches!(
        catalog.asset("song.mp3").unwrap().location,
        AudioAssetLocation::Loose(_)
    ));
    assert_eq!(catalog.shadowed_assets, 3);
    assert!(catalog.read("missing.wav").unwrap().is_none());
    assert!(catalog.read("../root.wav").is_err());
}

#[test]
fn bad_archive_lengths_are_rejected_before_inflation_and_other_sources_survive() {
    let dir = TestDirectory::new();
    let mut bomb = archive(&[("bomb.wav", b"small")]);
    let table = u32::from_le_bytes(bomb[..4].try_into().unwrap()) as usize;
    bomb[table + 12..table + 16].copy_from_slice(&u32::MAX.to_le_bytes());
    dir.write("snd1.pfs", bomb);
    dir.write("good.mp3", b"ID3 good");
    let catalog = AudioCatalog::load(&dir.0).unwrap();
    assert!(catalog.asset("bomb.wav").is_none());
    assert!(catalog.asset("good.mp3").is_some());
    assert!(
        catalog
            .diagnostics
            .entries
            .iter()
            .any(|d| d.source == "snd1.pfs" && d.kind == DiagnosticKind::Malformed)
    );
}

#[test]
fn catalog_selects_present_emt_without_falling_back_or_requiring_a_bank() {
    let dir = TestDirectory::new();
    dir.write("mp3index.txt", "theme.mp3\n");
    dir.write("theme.mp3", b"ID3 theme");
    dir.write("zone_sounds.eff", eff(1, [-1, -1]));
    let catalog = AudioCatalog::load(&dir.0).unwrap();
    let zone = catalog.load_zone("ZONE").unwrap();
    assert_eq!(zone.format, Some(ZoneAudioFormat::ClassicEff));
    assert_eq!(
        classic(&zone.emitters[0]).sounds[0],
        AudioReference::File("theme.mp3".into())
    );
    assert_eq!(catalog.load_zone("absent").unwrap().format, None);
    dir.write("Zone.EMT", "malformed");
    let catalog = AudioCatalog::load(&dir.0).unwrap();
    let zone = catalog.load_zone("zone").unwrap();
    assert_eq!(zone.format, Some(ZoneAudioFormat::Emt));
    assert!(zone.emitters.is_empty());
    assert!(
        zone.diagnostics
            .entries
            .iter()
            .any(|d| d.kind == DiagnosticKind::Precedence)
    );
    dir.write("Zone.EMT", [0xff]);
    let catalog = AudioCatalog::load(&dir.0).unwrap();
    assert!(catalog.load_zone("zone").is_err());
}
