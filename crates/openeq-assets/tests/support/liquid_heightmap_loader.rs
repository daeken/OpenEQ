//! Metadata-only archives; no textures, meshes or proprietary bytes required.
use super::*;
use std::{io::Write, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "openeq-liquid-heightmap-{}-{}-{}",
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
    fn archive(&self, entries: &[(&str, &[u8])]) {
        std::fs::write(self.0.join("renamed.eqg"), archive_bytes(entries)).unwrap();
    }
    fn load(&self) -> openeq_assets::Result<LiquidRegions> {
        LiquidRegions::load(&self.0, "RENAMED")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn archive_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![0; 12];
    bytes[4..8].copy_from_slice(&openeq_assets::pfs::PFS_MAGIC.to_le_bytes());
    let mut entries = Vec::new();
    let mut names = (files.len() as u32).to_le_bytes().to_vec();
    let mut block = |crc: u32, data: &[u8]| {
        let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
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
    block(openeq_assets::pfs::DIR_CRC, &names);
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

fn zon(name: &str) -> Vec<u8> {
    format!(" \nEQTZP 4\n*NAME {name}\n*MINLNG 0\n*MAXLNG 0\n*MINLAT 0\n*MAXLAT 0\n*UNITSPERVERT 10\n*QUADSPERTILE 1\n").into_bytes()
}

#[test]
fn runtime_load_follows_internal_and_renamed_declarations_without_s3d_fallback() {
    let fixture = Fixture::new();
    // Attempting the obsolete S3D would fail, including for unsupported EQG.
    std::fs::write(fixture.0.join("renamed.s3d"), b"obsolete s3d").unwrap();
    let good = zon("internal");
    let missing = zon("missing");
    let data = dat(vec![([0, 0], vec![record("AWT_pool", 0)])]);
    for exact in [false, true] {
        let mut entries = vec![
            ("INTERNAL.ZON", good.as_slice()),
            ("internal.DAT", data.as_slice()),
        ];
        if exact {
            entries.push(("renamed.zon", &missing));
        }
        fixture.archive(&entries);
        assert_eq!(
            fixture.load().unwrap().at([5., 5., 0.]),
            Some(LiquidKind::Water)
        );
    }
    fixture.archive(&[("renamed.zon", b"EQGZ")]);
    assert!(fixture.load().unwrap().is_empty());
    let mut unsupported = record("AWT_pool", 0);
    unsupported.scale[0] = 2.;
    let data = dat(vec![([0, 0], vec![unsupported])]);
    fixture.archive(&[("internal.zon", &good), ("internal.dat", &data)]);
    assert!(fixture.load().unwrap().is_empty());
}

#[test]
fn runtime_load_rejects_ambiguous_or_missing_dat_instead_of_guessing() {
    let fixture = Fixture::new();
    let first = zon("first");
    let second = zon("second");
    let data = dat(vec![([0, 0], vec![record("AWT_pool", 0)])]);
    let mut entries = vec![
        ("first.zon", first.as_slice()),
        ("first.dat", data.as_slice()),
        ("second.zon", second.as_slice()),
        ("second.dat", data.as_slice()),
    ];
    for _ in 0..2 {
        fixture.archive(&entries);
        assert!(fixture.load().is_err());
        entries.reverse();
    }
    fixture.archive(&[("renamed.zon", &first), ("renamed.dat", &data)]);
    assert!(
        fixture.load().is_err(),
        "DAT filename cannot be guessed from zone name"
    );
}

#[test]
fn runtime_load_preserves_exact_and_loose_zon_priority() {
    let fixture = Fixture::new();
    let internal = zon("internal");
    let loose = zon("loose");
    let wet = dat(vec![([0, 0], vec![record("AWT_pool", 0)])]);
    let dry = dat(vec![([0, 0], vec![record("APK_dry", 0)])]);
    std::fs::write(fixture.0.join("ReNamed.ZON"), &loose).unwrap();
    fixture.archive(&[
        ("renamed.zon", &internal),
        ("internal.dat", &wet),
        ("loose.dat", &dry),
    ]);
    assert_eq!(
        fixture.load().unwrap().at([5., 5., 0.]),
        Some(LiquidKind::Water)
    );
    fixture.archive(&[
        ("internal.zon", &internal),
        ("internal.dat", &wet),
        ("loose.dat", &dry),
    ]);
    assert!(fixture.load().unwrap().is_empty());
    std::fs::write(fixture.0.join("ReNamed.ZON"), b"EQTZP corrupt").unwrap();
    assert!(fixture.load().is_err());
}
