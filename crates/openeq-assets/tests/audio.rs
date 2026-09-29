//! Optional silent checks against locally installed original audio assets.
use std::sync::OnceLock;

use openeq_assets::{
    audio::{
        AudioAssetLocation, AudioCatalog, AudioEmitter, AudioReference, ClassicEmitterKind,
        DiagnosticKind, EmtLoopMode, ZoneAudioFormat,
    },
    loader,
};

fn catalog() -> &'static AudioCatalog {
    static CATALOG: OnceLock<AudioCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        AudioCatalog::load(&loader::default_client_dir().expect("original client assets")).unwrap()
    })
}

#[test]
#[ignore = "requires original EverQuest audio assets; does not play sound"]
fn original_pok_emitters_resolve_archive_effects_and_correct_mp3_index() {
    let catalog = catalog();
    let zone = catalog.load_zone("poknowledge").unwrap();
    assert_eq!(zone.format, Some(ZoneAudioFormat::ClassicEff));
    assert_eq!(zone.emitters.len(), 40);
    let AudioEmitter::Classic(night) = &zone.emitters[0] else {
        panic!("EFF");
    };
    assert_eq!(night.kind, ClassicEmitterKind::Ambient);
    assert_eq!(night.radius, 850.);
    assert_eq!(
        night.sounds,
        [
            AudioReference::Silent,
            AudioReference::File("nightime_background02_lp.wav".into())
        ]
    );
    let AudioEmitter::Classic(music) = &zone.emitters[38] else {
        panic!("EFF");
    };
    assert_eq!(music.sound_ids, [-14, -14]);
    assert_eq!(
        music.sounds,
        [
            AudioReference::File("poknowledge.mp3".into()),
            AudioReference::File("poknowledge.mp3".into())
        ]
    );
    assert_eq!(music.raw_words[17], 2000);
    assert!(matches!(
        catalog.asset_for(&music.sounds[0]).unwrap().location,
        AudioAssetLocation::Loose(_)
    ));
    let page = catalog.asset("PAGE_TURN01.WAV").unwrap();
    assert!(
        matches!(&page.location, AudioAssetLocation::Archive { path, .. } if path.file_name().unwrap() == "snd11.pfs")
    );
    let bytes = page.read().unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(bytes.len(), 26184);
    assert!(
        !zone
            .diagnostics
            .entries
            .iter()
            .any(|d| matches!(d.kind, DiagnosticKind::Missing | DiagnosticKind::Malformed))
    );
}

#[test]
#[ignore = "requires original EverQuest audio assets; does not play sound"]
fn original_anguish_and_extended_crescent_emt_rows_keep_authored_fields() {
    let catalog = catalog();
    let zone = catalog.load_zone("anguish").unwrap();
    assert_eq!(zone.format, Some(ZoneAudioFormat::Emt));
    assert_eq!(zone.emitters.len(), 1);
    let AudioEmitter::Emt(music) = &zone.emitters[0] else {
        panic!("EMT");
    };
    assert_eq!(music.revision, 2);
    assert_eq!(music.sound, AudioReference::File("anguish.mp3".into()));
    assert_eq!(music.position, [7.42, -14.08, 6.84]);
    assert_eq!(music.activation_range, 0.);
    assert_eq!(music.repeat_delay_ms, [5000, 5000]);
    assert_eq!(music.loop_mode, EmtLoopMode::DelayedRepeat);
    assert_eq!(music.environment_flag, Some(0));
    assert!(catalog.asset_for(&music.sound).is_some());
    let crescent = catalog.load_zone("crescent").unwrap();
    let AudioEmitter::Emt(waterfall) = &crescent.emitters[0] else {
        panic!("EMT");
    };
    assert_eq!(
        waterfall.sound,
        AudioReference::File("waterfall_big_lp.wav".into())
    );
    assert_eq!(waterfall.position, [555.32, -976.77, -156.86]);
    assert_eq!(waterfall.extensions, ["0"]);
    assert_eq!(waterfall.full_volume_radius, 200.);
    assert_eq!(waterfall.max_audible_distance, 300.);
}

#[test]
#[ignore = "requires original EverQuest audio assets; does not play sound"]
fn original_gfay_xmi_references_remain_sequences_and_sound_banks_are_readable() {
    let catalog = catalog();
    let zone = catalog.load_zone("gfaydark").unwrap();
    assert_eq!(zone.emitters.len(), 19);
    let AudioEmitter::Classic(first) = &zone.emitters[0] else {
        panic!("EFF");
    };
    assert_eq!(
        first.sounds[0],
        AudioReference::XmiSequence {
            file: "gfaydark.xmi".into(),
            sequence: 2
        }
    );
    let bytes = catalog.read("gfaydark.xmi").unwrap().unwrap();
    assert_eq!(&bytes[..4], b"FORM");
    assert_eq!(&bytes[8..12], b"XDIR");
    assert_eq!(
        bytes.windows(4).filter(|chunk| *chunk == b"EVNT").count(),
        6
    );
    let AudioEmitter::Classic(wind) = &zone.emitters[12] else {
        panic!("EFF");
    };
    assert_eq!(
        wind.sounds,
        [
            AudioReference::File("wind_lp2.wav".into()),
            AudioReference::File("darkwds1.wav".into())
        ]
    );
    assert!(
        matches!(&catalog.asset_for(&wind.sounds[0]).unwrap().location, AudioAssetLocation::Archive { path, .. } if path.file_name().unwrap() == "snd6.pfs")
    );
    assert!(
        !zone
            .diagnostics
            .entries
            .iter()
            .any(|d| d.kind == DiagnosticKind::Missing)
    );
    assert!(
        !catalog
            .diagnostics
            .entries
            .iter()
            .any(|d| d.kind == DiagnosticKind::Malformed)
    );
}

#[test]
#[ignore = "requires original EverQuest audio assets; does not play sound"]
fn installed_bad_emt_coordinate_is_reported_without_losing_valid_emitters() {
    let zone = catalog().load_zone("westkorlach").unwrap();
    assert!(!zone.emitters.is_empty());
    assert!(
        zone.diagnostics
            .entries
            .iter()
            .any(|d| d.entry == Some(150) && d.kind == DiagnosticKind::Malformed)
    );
    assert!(
        zone.emitters
            .iter()
            .any(|emitter| matches!(emitter, AudioEmitter::Emt(e) if e.line_number == 151))
    );
}

#[test]
#[ignore = "requires original EverQuest audio assets; does not play sound"]
fn installed_emitter_lists_are_bounded_and_report_malformed_rows() {
    let base = loader::default_client_dir().expect("original client assets");
    let mut zones = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(base).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if let Some(zone) = name
            .strip_suffix(".emt")
            .or_else(|| name.strip_suffix("_sounds.eff"))
        {
            zones.insert(zone.to_owned());
        }
    }
    assert!(zones.len() > 400);
    let mut total = 0;
    let mut malformed = Vec::new();
    for zone in &zones {
        let audio = catalog()
            .load_zone(zone)
            .unwrap_or_else(|error| panic!("{zone}: {error}"));
        assert!(audio.emitters.len() <= openeq_assets::audio::MAX_ZONE_EMITTERS);
        total += audio.emitters.len();
        malformed.extend(
            audio
                .diagnostics
                .entries
                .into_iter()
                .filter(|d| d.kind == DiagnosticKind::Malformed)
                .map(|d| (d.source, d.entry, d.message)),
        );
    }
    let mut counts = std::collections::BTreeMap::new();
    for (_, _, reason) in &malformed {
        *counts.entry(reason).or_insert(0usize) += 1;
    }
    eprintln!(
        "{} zones, {total} emitters, {} malformed rows: {counts:?}",
        zones.len(),
        malformed.len()
    );
    assert!(total > 39000);
    assert!(
        malformed
            .iter()
            .any(|(source, entry, _)| source == "westkorlach.emt" && *entry == Some(150))
    );
}
