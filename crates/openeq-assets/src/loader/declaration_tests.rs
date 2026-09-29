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
