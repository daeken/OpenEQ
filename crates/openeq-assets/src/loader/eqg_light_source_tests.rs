//! Authored eligibility is retained independently of renderer light selection.
use super::*;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openeq-eqg-light-source-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn archive(&self, files: &[(&str, &[u8])]) -> Archive {
        let mut bytes = words(&[0, crate::pfs::PFS_MAGIC, 0]);
        let mut entries = Vec::new();
        let mut names = words(&[files.len() as u32]);
        let mut block = |crc, data: &[u8]| {
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(data).unwrap();
            let compressed = encoder.finish().unwrap();
            let offset = bytes.len() as u32;
            bytes.extend(words(&[compressed.len() as u32, data.len() as u32]));
            bytes.extend(compressed);
            entries.push((crc, offset, data.len() as u32));
        };
        for (index, (name, data)) in files.iter().enumerate() {
            block(index as u32 + 1, data);
            names.extend(words(&[name.len() as u32 + 1]));
            names.extend(name.as_bytes());
            names.push(0);
        }
        block(crate::pfs::DIR_CRC, &names);
        let directory = bytes.len() as u32;
        bytes[..4].copy_from_slice(&directory.to_le_bytes());
        bytes.extend(words(&[entries.len() as u32]));
        for (crc, offset, size) in entries.into_iter().rev() {
            bytes.extend(words(&[crc, offset, size]));
        }
        Archive::from_bytes(self.0.join("fixture.eqg"), bytes).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn declaration(version: u32, names: &[&[u8]], terrain: bool) -> Vec<u8> {
    let mut strings = b"fixture.ter\0".to_vec();
    let mut records = Vec::new();
    for (index, name) in names.iter().enumerate() {
        records.extend(words(&[strings.len() as u32]));
        strings.extend(*name);
        strings.push(0);
        records.extend(
            [-0.0_f32, 20. + index as f32, -3., 0.125, 1.5, 0., 8.]
                .into_iter()
                .flat_map(|v| v.to_bits().to_le_bytes()),
        );
    }
    let mut data = words(&[
        crate::zone::ZON_MAGIC,
        version,
        strings.len() as u32,
        u32::from(terrain),
        0,
        0,
        names.len() as u32,
    ]);
    data.extend(strings);
    if terrain {
        data.extend(words(&[0]));
    }
    data.extend(records);
    data
}

#[test]
fn exact_names_and_original_third_byte_gate_survive_scene_loading() {
    let fixture = Fixture::new();
    // High bytes expand in the readable String, but source offsets and name[2]
    // must still refer to original bytes. Repeated names retain distinct ordinals.
    let cases: &[(&[u8], bool)] = &[
        (b"LIT_torch", false),
        (b"LIB_MixedCase", true),
        (b"Lib_lower", true),
        (b"Bxx", false),
        (b"xBx", false),
        (b"", false),
        (b"LB", false),
        (b"\xe9xB_after_high_byte", true),
        (b"x\xe9b_second_high_byte", true),
        (b"LIT_after_expanded_strings", false),
        (b"LIB_MixedCase", true),
    ];
    let names: Vec<_> = cases.iter().map(|(name, _)| *name).collect();
    for version in [1, 2] {
        let data = declaration(version, &names, false);
        let scene = load_eqg_archive(
            &fixture.0,
            "fixture",
            fixture.archive(&[("fixture.zon", &data)]),
        )
        .unwrap();
        assert_eq!(scene.lights.len(), cases.len());
        for (ordinal, (light, (name, eligible))) in scene.lights.iter().zip(cases).enumerate() {
            let source = light.eqg_source.as_ref().unwrap();
            assert_eq!(source.ordinal, ordinal);
            assert_eq!(
                source.name,
                name.iter().map(|b| char::from(*b)).collect::<String>()
            );
            assert_eq!(source.terrain_eligible, *eligible, "{name:?}");
            assert_eq!(
                source.declaration,
                EqgDeclarationSource::Archive {
                    path: fixture.0.join("fixture.eqg"),
                    member: "fixture.zon".into(),
                }
            );
            assert_eq!(
                light.position.map(f32::to_bits),
                [20. + ordinal as f32, 0., -3.].map(f32::to_bits)
            );
            assert_eq!(light.color, [0.125, 1.5, 0.]);
            assert_eq!(light.radius, 8.);
            assert_eq!(light.attenuation, 200.);
        }
        let mut copied = scene.lights.clone();
        copied.remove(0);
        assert_eq!(copied[0].eqg_source.as_ref().unwrap().ordinal, 1);
    }
}

#[test]
fn provenance_follows_archive_loose_and_unambiguous_alias_precedence() {
    let fixture = Fixture::new();
    let archived = declaration(1, &[b"LIT_archived"], false);
    let loose = declaration(2, &[b"LIB_Loose"], false);
    let alias = declaration(1, &[b"Lib_Alias"], true);
    let ter = words(&[crate::zone::TER_MAGIC, 1, 0, 0, 0, 0]);
    let loose_path = fixture.0.join("FiXtUrE.ZON");
    std::fs::write(&loose_path, &loose).unwrap();
    let exact = load_eqg_archive(
        &fixture.0,
        "fixture",
        fixture.archive(&[("fixture.zon", &loose), ("FiXtUrE.ZON", &archived)]),
    )
    .unwrap();
    assert_eq!(
        exact.lights[0].eqg_source.as_ref().unwrap().name,
        "LIT_archived"
    );
    assert_eq!(
        exact.lights[0].eqg_source.as_ref().unwrap().declaration,
        EqgDeclarationSource::Archive {
            path: fixture.0.join("fixture.eqg"),
            member: "FiXtUrE.ZON".into(),
        }
    );
    let load_alias = || {
        load_eqg_archive(
            &fixture.0,
            "fixture",
            fixture.archive(&[
                ("internal.zon", &alias),
                ("InTeRnAl.ZON", &alias),
                ("duplicate.zon", &alias),
                ("fixture.ter", &ter),
            ]),
        )
        .unwrap()
    };
    let selected = load_alias();
    let source = selected.lights[0].eqg_source.as_ref().unwrap();
    assert_eq!(source.name, "LIB_Loose");
    let EqgDeclarationSource::Loose { path } = &source.declaration else {
        panic!("loose declaration should precede the internal archive alias")
    };
    // Case-insensitive volumes resolve the direct lowercase lookup without
    // enumerating the directory; both spellings must identify the selected file.
    assert_eq!(
        path.canonicalize().unwrap(),
        loose_path.canonicalize().unwrap()
    );
    std::fs::remove_file(loose_path).unwrap();
    let selected = load_alias();
    let source = selected.lights[0].eqg_source.as_ref().unwrap();
    assert_eq!(source.name, "Lib_Alias");
    assert_eq!(
        source.declaration,
        EqgDeclarationSource::Archive {
            path: fixture.0.join("fixture.eqg"),
            member: "InTeRnAl.ZON".into(),
        }
    );
}

#[test]
#[ignore = "requires original EQG zones; CPU only"]
fn original_zone_light_metadata_preserves_all_numeric_records_and_counts() {
    let base = default_client_dir().expect("original client assets");
    for (zone, count, eligible) in [
        ("anguish", 452, 0),
        ("causeway", 126, 55),
        ("bloodfields", 134, 12),
        ("wallofslaughter", 188, 0),
    ] {
        let path = base.join(format!("{zone}.eqg"));
        let archive = Archive::open(&path).unwrap();
        let member = format!("{zone}.zon");
        let data = archive.read(&member).unwrap();
        // Independent raw record offsets, without reparsing objects/materials.
        let word = |at| u32::from_le_bytes(data[at..at + 4].try_into().unwrap()) as usize;
        assert_eq!(&data[..4], b"EQGZ");
        assert_eq!(word(4), 1);
        assert_eq!(word(24), count);
        let strings = &data[28..28 + word(8)];
        let start = 28 + word(8) + 4 * word(12) + 36 * word(16) + 40 * word(20);
        assert_eq!(start + 32 * count, data.len());
        let scene = load_zone(&base, zone).unwrap();
        assert_eq!(scene.lights.len(), count);
        assert_eq!(
            scene
                .lights
                .iter()
                .filter(|l| l.eqg_source.as_ref().unwrap().terrain_eligible)
                .count(),
            eligible
        );
        for (ordinal, light) in scene.lights.iter().enumerate() {
            let at = start + 32 * ordinal;
            let raw_name = strings[word(at)..].split(|b| *b == 0).next().unwrap();
            let source = light.eqg_source.as_ref().unwrap();
            assert_eq!(source.ordinal, ordinal);
            assert_eq!(
                source.name,
                raw_name.iter().map(|b| char::from(*b)).collect::<String>()
            );
            assert_eq!(
                source.terrain_eligible,
                matches!(raw_name.get(2), Some(b'B' | b'b'))
            );
            assert_eq!(
                source.declaration,
                EqgDeclarationSource::Archive {
                    path: path.clone(),
                    member: member.clone()
                }
            );
            assert_eq!(
                light.position.map(f32::to_bits),
                [
                    word(at + 8) as u32,
                    word(at + 4) as u32 ^ 0x8000_0000,
                    word(at + 12) as u32
                ]
            );
            assert_eq!(
                light.color.map(f32::to_bits),
                [
                    word(at + 16) as u32,
                    word(at + 20) as u32,
                    word(at + 24) as u32
                ]
            );
            assert_eq!(light.radius.to_bits(), word(at + 28) as u32);
            assert_eq!(light.attenuation, 200.);
        }
    }
}

#[test]
#[ignore = "requires original WLD and heightmap zones; CPU only"]
fn other_original_zone_formats_do_not_claim_binary_eqg_eligibility() {
    let base = default_client_dir().expect("original client assets");
    for zone in ["gfaydark", "nektulos"] {
        let scene = load_zone(&base, zone).unwrap();
        assert!(!scene.lights.is_empty(), "{zone}");
        assert!(
            scene.lights.iter().all(|light| light.eqg_source.is_none()),
            "{zone}"
        );
    }
}
