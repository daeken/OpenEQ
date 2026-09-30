use super::*;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "openeq-zone-declaration-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn archive(files: &[(&str, &[u8])]) -> Archive {
    Archive::from_bytes("fixture.eqg".into(), archive_bytes(files)).unwrap()
}

fn archive_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![0; 12];
    bytes[4..8].copy_from_slice(&crate::pfs::PFS_MAGIC.to_le_bytes());
    let mut entries = Vec::new();
    let mut names = (files.len() as u32).to_le_bytes().to_vec();
    let mut block = |crc: u32, data: &[u8]| {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(data).unwrap();
        let compressed = encoder.finish().unwrap();
        let offset = bytes.len() as u32;
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);
        entries.push((crc, offset, data.len() as u32));
    };
    for (index, (name, data)) in files.iter().enumerate() {
        block(index as u32 + 1, data);
        names.extend_from_slice(&(name.len() as u32 + 1).to_le_bytes());
        names.extend_from_slice(name.as_bytes());
        names.push(0);
    }
    block(crate::pfs::DIR_CRC, &names);
    let directory_offset = bytes.len() as u32;
    bytes[..4].copy_from_slice(&directory_offset.to_le_bytes());
    bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for (crc, offset, size) in entries.into_iter().rev() {
        for word in [crc, offset, size] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    bytes
}

fn declaration(name: &str) -> Vec<u8> {
    format!("EQTZP 4\n*NAME {name}\n*MINLNG 0\n*MAXLNG 0\n*MINLAT 0\n*MAXLAT 0\n*UNITSPERVERT 16\n*QUADSPERTILE 16\n").into_bytes()
}

#[test]
fn exact_archive_and_loose_declarations_keep_precedence_even_if_corrupt() {
    let fixture = Fixture::new();
    let alternate = declaration("internal");
    std::fs::write(fixture.0.join("Renamed.ZON"), b"loose declaration").unwrap();
    let packed = archive(&[
        ("renamed.zon", b"invalid exact declaration"),
        ("internal.zon", &alternate),
        ("internal.dat", b""),
    ]);
    assert_eq!(
        read_eqg_declaration(&fixture.0, "renamed", &packed).unwrap(),
        b"invalid exact declaration"
    );
    let packed = archive(&[("internal.zon", &alternate), ("internal.dat", b"")]);
    assert_eq!(
        read_eqg_declaration(&fixture.0, "renamed", &packed).unwrap(),
        b"loose declaration"
    );
    std::fs::remove_file(fixture.0.join("Renamed.ZON")).unwrap();
    std::fs::create_dir(fixture.0.join("renamed.zon")).unwrap();
    assert!(
        matches!(
            read_eqg_declaration(&fixture.0, "renamed", &packed),
            Err(Error::Io { .. })
        ),
        "an unreadable exact path must not silently select another zone"
    );
}

#[test]
fn renamed_heightmap_requires_a_declaration_with_its_own_dat() {
    let fixture = Fixture::new();
    let good = declaration("internal");
    let missing = declaration("missing");
    let packed = archive(&[
        ("misleading.zon", &missing),
        ("INTERNAL.ZON", &good),
        ("internal.DAT", b""),
        ("binary.zon", b"EQGZ"),
    ]);
    assert_eq!(
        read_eqg_declaration(&fixture.0, "renamed", &packed).unwrap(),
        good
    );
    let packed = archive(&[("misleading.zon", &missing), ("internal.dat", b"")]);
    assert!(unique_heightmap_declaration(&packed).unwrap().is_none());
    assert!(matches!(
        read_eqg_declaration(&fixture.0, "renamed", &packed),
        Err(Error::Io { .. })
    ));
}

#[test]
fn ambiguous_internal_declarations_are_rejected_independent_of_archive_order() {
    let first = declaration("first");
    let second = declaration("second");
    let fixture = Fixture::new();
    let mut entries = vec![
        ("first.zon", first.as_slice()),
        ("second.zon", second.as_slice()),
        ("first.dat", b"".as_slice()),
        ("second.dat", b"".as_slice()),
    ];
    for _ in 0..2 {
        let packed = archive(&entries);
        assert!(
            matches!(read_eqg_declaration(&fixture.0, "renamed", &packed), Err(Error::Format(message)) if message.contains("multiple distinct"))
        );
        entries.reverse();
    }
    let packed = archive(&[
        ("first.zon", &first),
        ("alias.zon", &first),
        ("first.dat", b""),
    ]);
    assert_eq!(
        read_eqg_declaration(&fixture.0, "renamed", &packed).unwrap(),
        first
    );
}

#[test]
fn whitespace_prefixed_internal_declaration_reaches_the_heightmap_parser() {
    let fixture = Fixture::new();
    let mut zon = b" \r\n\t".to_vec();
    zon.extend(declaration("internal"));
    let packed = archive(&[("internal.zon", &zon), ("internal.dat", b"")]);
    assert!(
        matches!(
            load_eqg_archive(&fixture.0, "renamed", packed),
            Err(Error::Truncated { offset: 0, .. })
        ),
        "an empty DAT must reach terrain parsing, not fail EQGZ magic detection"
    );
}

#[test]
fn unreadable_alternate_is_not_silently_excluded_from_ambiguity_check() {
    let zon = declaration("internal");
    let mut bytes = archive_bytes(&[
        ("broken.zon", b"unreadable declaration"),
        ("internal.zon", &zon),
        ("internal.dat", b""),
    ]);
    // First payload's compressed zlib stream begins after the header and its
    // two length words. The archive directory remains independently valid.
    bytes[20] = 0;
    let packed = Archive::from_bytes("fixture.eqg".into(), bytes).unwrap();
    assert!(packed.read("internal.zon").is_ok());
    assert!(unique_heightmap_declaration(&packed).is_err());
}

fn binary_declaration(terrain_name: &str) -> Vec<u8> {
    let mut zon = Vec::new();
    for value in [
        crate::zone::ZON_MAGIC,
        1,
        terrain_name.len() as u32 + 1,
        1,
        0,
        0,
        0,
    ] {
        zon.extend(value.to_le_bytes());
    }
    zon.extend(terrain_name.as_bytes());
    zon.push(0);
    zon.extend(0u32.to_le_bytes());
    zon
}

#[test]
fn binary_alias_requires_resolved_geometry_and_rejects_ambiguity() {
    let fixture = Fixture::new();
    let first = binary_declaration("first.ter");
    let second = binary_declaration("second.ter");
    let ter: Vec<u8> = [crate::zone::TER_MAGIC, 1, 0, 0, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let packed = archive(&[("INTERNAL.ZON", &first), ("first.TER", &ter)]);
    assert_eq!(
        read_eqg_declaration(&fixture.0, "renamed", &packed).unwrap(),
        first
    );
    let missing = archive(&[("internal.zon", &first)]);
    assert!(read_eqg_declaration(&fixture.0, "renamed", &missing).is_err());
    let mut entries = vec![
        ("first.zon", first.as_slice()),
        ("second.zon", second.as_slice()),
        ("first.ter", ter.as_slice()),
        ("second.ter", ter.as_slice()),
    ];
    for _ in 0..2 {
        assert!(
            matches!(read_eqg_declaration(&fixture.0, "renamed", &archive(&entries)), Err(Error::Format(message)) if message.contains("multiple distinct"))
        );
        entries.reverse();
    }
    let text = declaration("heightmap");
    let mixed = archive(&[
        ("binary.zon", &first),
        ("first.ter", &ter),
        ("heightmap.zon", &text),
        ("heightmap.dat", b""),
    ]);
    assert!(
        matches!(read_eqg_declaration(&fixture.0, "renamed", &mixed), Err(Error::Format(message)) if message.contains("multiple distinct"))
    );
    let duplicated = archive(&[
        ("first.zon", &first),
        ("alias.zon", &first),
        ("first.ter", &ter),
    ]);
    assert_eq!(
        read_eqg_declaration(&fixture.0, "renamed", &duplicated).unwrap(),
        first
    );
}

#[test]
#[ignore = "requires original renamed EQG dungeons; CPU only"]
fn original_renamed_binary_dungeons_load_authored_geometry() {
    let base = default_client_dir().expect("original client assets");
    for (zone, internal) in [
        ("chambersb", "chambersa"),
        ("chambersc", "chambersa"),
        ("chambersd", "chambersa"),
        ("chamberse", "chambersa"),
        ("chambersf", "chambersa"),
        ("dranikcatacombsb", "catacombb"),
        ("dranikcatacombsc", "catacombc"),
        ("dranikhollowsa", "cavea"),
        ("dranikhollowsb", "caveb"),
        ("dranikhollowsc", "cavec"),
        ("draniksewersa", "sewera"),
        ("draniksewersb", "sewerb"),
        ("draniksewersc", "sewerc"),
    ] {
        let packed = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
        assert!(!packed.contains(&format!("{zone}.zon")), "{zone}");
        let declaration = read_eqg_declaration(&base, zone, &packed).unwrap();
        assert_eq!(
            declaration,
            packed.read(&format!("{internal}.zon")).unwrap(),
            "{zone}"
        );
        let source = ZoneFile::parse(&declaration, |name| packed.read(name)).unwrap();
        let scene = load_zone(&base, zone).unwrap();
        assert_eq!(scene.name, zone);
        assert_eq!(scene.instances.len(), source.placeables.len(), "{zone}");
        assert_eq!(scene.lights.len(), source.lights.len(), "{zone}");
        assert!(scene.triangle_count() > 100, "{zone}");
        assert!(
            crate::collision::CollisionWorld::build(&scene).triangle_count() > 100,
            "{zone}"
        );
    }
}

#[test]
fn sole_broken_binary_alias_reports_its_internal_dependency_and_duplicates_collapse() {
    let fixture = Fixture::new();
    let zon = binary_declaration("missing.ter");
    for entries in [
        vec![("internal.zon", zon.as_slice())],
        vec![
            ("internal.zon", zon.as_slice()),
            ("duplicate.zon", zon.as_slice()),
        ],
    ] {
        let error = read_eqg_declaration(&fixture.0, "renamed", &archive(&entries)).unwrap_err();
        assert!(
            matches!(error, Error::Format(message)
            if message.contains("internal.zon") && message.contains("missing.ter")),
            "{entries:?}"
        );
    }
    let broken = archive(&[("internal.zon", &zon), ("missing.ter", b"bad")]);
    assert!(
        matches!(read_eqg_declaration(&fixture.0, "renamed", &broken), Err(Error::Format(message))
        if message.contains("internal.zon") && message.contains("truncated"))
    );
    let other = binary_declaration("other.ter");
    let mut entries = vec![
        ("first.zon", zon.as_slice()),
        ("second.zon", other.as_slice()),
    ];
    for _ in 0..2 {
        assert!(
            matches!(read_eqg_declaration(&fixture.0, "renamed", &archive(&entries)), Err(Error::Format(message))
            if message.contains("multiple distinct") && message.contains("first.zon") && message.contains("second.zon"))
        );
        entries.reverse();
    }
}

#[test]
#[ignore = "requires original Dranik Catacombs A archive; CPU only"]
fn original_catacombs_mismatched_banner_names_remain_an_explicit_missing_dependency() {
    let base = default_client_dir().expect("original client assets");
    let packed = Archive::open(base.join("dranikcatacombsa.eqg")).unwrap();
    let zon = packed.read("catacomba.zon").unwrap();
    // The archive directory and filename CRCs agree on underscores:
    // 0x1409b969 / 0x0f21b411. Authored ')' names instead hash to
    // 0x78533c40 / 0x637b3138; no native normalization rule is established.
    for suffix in ["00", "01"] {
        assert!(packed.contains(&format!("obp_dz_lbanner0__{suffix}.mod")));
        assert!(!packed.contains(&format!("obp_dz_lbanner0)_{suffix}.mod")));
    }
    let error = ZoneFile::parse(&zon, |name| packed.read(name)).unwrap_err();
    assert!(matches!(error, Error::NotFound(name) if name == "obp_dz_lbanner0)_00.mod"));
    let error = load_zone(&base, "dranikcatacombsa").err().unwrap();
    assert!(matches!(error, Error::Format(message)
        if message.contains("catacomba.zon") && message.contains("obp_dz_lbanner0)_00.mod")));
}
