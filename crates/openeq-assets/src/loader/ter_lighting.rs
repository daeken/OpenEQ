//! Source selection for the narrowly established TER lighting channel.
//! Original vertex order is retained; material grouping and packing happen later.

use std::path::{Path, PathBuf};

use crate::{
    Error, Result,
    pfs::Archive,
    read::Reader,
    zone::{LIT_MAGIC, VertexLighting, ZON_MAGIC},
};

pub const DEFAULT_LIGHTING: u32 = 0x001f_1f1f;

/// The native lookup context, including unsuccessful/defaulted lookups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightingSource {
    LooseZon { path: PathBuf },
    ArchiveLit { member: String },
    LooseLit { path: PathBuf },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightingStatus {
    Matched { count: usize },
    Missing,
    UnrecognizedMagic,
    CountMismatch { supplied: usize, expected: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightingProvenance {
    pub source: LightingSource,
    pub status: LightingStatus,
}

/// Packed 0xAARRGGBB lighting words in original TER vertex order.
/// Alpha is a lighting weight, never a surface-opacity declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Always has exactly the requested TER vertex count on success.
    pub colors: Vec<u32>,
    pub provenance: LightingProvenance,
}

/// Select only after the caller has established the supported TER material scope.
/// The loose outer-zone stream is independent of the chosen scene declaration.
pub(super) fn select(
    base: &Path,
    zone_name: &str,
    archive: &Archive,
    ter_name: &str,
    ter_from_archive: bool,
    vertex_count: usize,
) -> Result<Selection> {
    let zon_path = super::case_insensitive_file(base, &format!("{zone_name}.zon"));
    if let Some(data) = read_optional_file(&zon_path)?
        && let Some(colors) = first_loose_placement_lighting(&data)?
    {
        return Ok(with_count(
            colors,
            LightingSource::LooseZon { path: zon_path },
            vertex_count,
        ));
    }

    // Native member parsing removes the four-byte extension before appending
    // .LIT. Callers supply the actual TER filename, not an object_N scene name.
    let stem = ter_name
        .get(..ter_name.len().saturating_sub(4))
        .filter(|_| ter_name.to_ascii_lowercase().ends_with(".ter"))
        .ok_or_else(|| Error::Format(format!("invalid TER lighting source name {ter_name}")))?;
    let lit_name = format!("{stem}.lit");
    let (source, data) = if ter_from_archive {
        (
            LightingSource::ArchiveLit {
                member: lit_name.clone(),
            },
            archive.read_opt(&lit_name)?,
        )
    } else {
        let path = super::case_insensitive_file(base, &lit_name);
        let data = read_optional_file(&path)?;
        (LightingSource::LooseLit { path }, data)
    };
    let Some(data) = data else {
        return Ok(defaulted(source, LightingStatus::Missing, vertex_count));
    };
    // Truncation is an explicit error; a complete but unrecognized magic has
    // the native missing-light result. Do not retry in the other file context.
    if Reader::new(&data).u32()? != LIT_MAGIC {
        return Ok(defaulted(
            source,
            LightingStatus::UnrecognizedMagic,
            vertex_count,
        ));
    }
    Ok(with_count(
        VertexLighting::parse(&data)?.colors,
        source,
        vertex_count,
    ))
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

fn first_loose_placement_lighting(data: &[u8]) -> Result<Option<Vec<u32>>> {
    let mut reader = Reader::new(data);
    if reader.u32()? != ZON_MAGIC || reader.u32()? != 2 {
        return Ok(None);
    }
    let string_size = reader.bounded_count()?;
    let object_count = reader.bounded_count()?;
    let placeable_count = reader.bounded_count()?;
    reader.skip(8)?; // Unknown-record and light counts do not affect this offset.
    reader.skip(string_size)?;
    let model_table_bytes = object_count
        .checked_mul(4)
        .ok_or_else(|| Error::Format("ZON model-table length overflow".into()))?;
    reader.skip(model_table_bytes)?;
    if placeable_count == 0 {
        // The original helper unconditionally reads a first placement. Reject
        // malformed input rather than reading unrelated bytes as a stream.
        return Err(Error::Format(
            "loose ZON v2 has no first placement for TER lighting".into(),
        ));
    }
    reader.skip(36)?; // First placement only; do not search by object ID/name.
    let count = reader.bounded_count()?;
    let bytes = count
        .checked_mul(4)
        .ok_or_else(|| Error::Format("ZON lighting length overflow".into()))?;
    let colors = reader
        .take(bytes)?
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
        .collect();
    // Some(empty) remains an installed native pointer and suppresses LIT retry.
    Ok(Some(colors))
}

fn with_count(colors: Vec<u32>, source: LightingSource, expected: usize) -> Selection {
    let supplied = colors.len();
    if supplied == expected {
        Selection {
            colors,
            provenance: LightingProvenance {
                source,
                status: LightingStatus::Matched { count: supplied },
            },
        }
    } else {
        defaulted(
            source,
            LightingStatus::CountMismatch { supplied, expected },
            expected,
        )
    }
}

fn defaulted(source: LightingSource, status: LightingStatus, count: usize) -> Selection {
    Selection {
        colors: vec![DEFAULT_LIGHTING; count],
        provenance: LightingProvenance { source, status },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "openeq-ter-lighting-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, data: &[u8]) {
            std::fs::write(self.0.join(name), data).unwrap();
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

    fn lit(colors: &[u32]) -> Vec<u8> {
        let mut bytes = words(&[LIT_MAGIC, colors.len() as u32]);
        bytes.extend(words(colors));
        bytes
    }

    fn zon(first: &[u32], second: &[u32]) -> Vec<u8> {
        let mut bytes = words(&[ZON_MAGIC, 2, 1, 2, 2, 0, 0]);
        bytes.push(0);
        bytes.extend(words(&[0, 0]));
        // First object ID deliberately differs from the second. Native lighting
        // selection consumes the first stream irrespective of which ID is TER.
        for (id, colors) in [(1, first), (0, second)] {
            bytes.extend(words(&[id, 0, 0, 0, 0, 0, 0, 0, 0x3f80_0000]));
            bytes.extend(words(&[colors.len() as u32]));
            bytes.extend(words(colors));
        }
        bytes
    }

    fn archive(files: &[(&str, Vec<u8>)]) -> Archive {
        let mut bytes = words(&[0, crate::pfs::PFS_MAGIC, 0]);
        let mut entries = Vec::new();
        let mut names = words(&[files.len() as u32]);
        let mut block = |crc: u32, data: &[u8]| {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(data).unwrap();
            let compressed = encoder.finish().unwrap();
            let offset = bytes.len() as u32;
            bytes.extend(words(&[compressed.len() as u32, data.len() as u32]));
            bytes.extend(compressed);
            entries.push((crc, offset, data.len() as u32));
        };
        for (i, (name, data)) in files.iter().enumerate() {
            block(i as u32 + 1, data);
            names.extend(words(&[name.len() as u32 + 1]));
            names.extend(name.as_bytes());
            names.push(0);
        }
        block(crate::pfs::DIR_CRC, &names);
        let offset = bytes.len() as u32;
        bytes[..4].copy_from_slice(&offset.to_le_bytes());
        bytes.extend(words(&[entries.len() as u32]));
        for (crc, offset, len) in entries {
            bytes.extend(words(&[crc, offset, len]));
        }
        Archive::from_bytes(PathBuf::from("source.eqg"), bytes).unwrap()
    }

    #[test]
    fn loose_first_stream_overrides_archive_and_empty_mismatch_never_retries() {
        let fixture = Fixture::new();
        let archive = archive(&[
            ("zone.zon", zon(&[9, 9], &[8, 8])),
            ("ter_test.lit", lit(&[7, 7])),
        ]);
        fixture.write("TER_TEST.LIT", &lit(&[6, 6]));
        for first in [&[0x1122_3344, 0xaabb_ccdd][..], &[1][..], &[][..]] {
            fixture.write("ZONE.ZON", &zon(first, &[5, 5]));
            let selected = select(&fixture.0, "zone", &archive, "ter_test.ter", true, 2).unwrap();
            let LightingSource::LooseZon { path } = &selected.provenance.source else {
                panic!("expected loose ZON source");
            };
            assert_eq!(
                path.canonicalize().unwrap(),
                fixture.0.join("ZONE.ZON").canonicalize().unwrap()
            );
            if first.len() == 2 {
                assert_eq!(selected.colors, first);
                assert_eq!(
                    selected.provenance.status,
                    LightingStatus::Matched { count: 2 }
                );
            } else {
                assert_eq!(selected.colors, [DEFAULT_LIGHTING; 2]);
                assert_eq!(
                    selected.provenance.status,
                    LightingStatus::CountMismatch {
                        supplied: first.len(),
                        expected: 2
                    }
                );
            }
        }
    }

    #[test]
    fn lit_lookup_is_exclusive_to_actual_ter_source_and_archived_zon_is_not_embedded() {
        let fixture = Fixture::new();
        fixture.write("TER_TEST.LIT", &lit(&[0x4455_6677, 0x8899_aabb]));
        let archive = archive(&[
            ("zone.zon", zon(&[9, 9], &[8, 8])),
            ("ter_test.lit", lit(&[0x1122_3344, 0x5566_7788])),
        ]);
        let from_archive = select(&fixture.0, "zone", &archive, "TER_TEST.TER", true, 2).unwrap();
        assert_eq!(from_archive.colors, [0x1122_3344, 0x5566_7788]);
        assert!(matches!(
            from_archive.provenance.source,
            LightingSource::ArchiveLit { .. }
        ));
        let from_loose = select(&fixture.0, "zone", &archive, "ter_test.ter", false, 2).unwrap();
        assert_eq!(from_loose.colors, [0x4455_6677, 0x8899_aabb]);
        assert!(matches!(
            from_loose.provenance.source,
            LightingSource::LooseLit { .. }
        ));
        std::fs::remove_file(fixture.0.join("TER_TEST.LIT")).unwrap();
        let missing = select(&fixture.0, "zone", &archive, "ter_test.ter", false, 2).unwrap();
        assert_eq!(missing.colors, [DEFAULT_LIGHTING; 2]);
        assert_eq!(missing.provenance.status, LightingStatus::Missing);
    }

    #[test]
    fn missing_bad_magic_and_wrong_counts_default_without_cross_context_fallback() {
        let fixture = Fixture::new();
        fixture.write("ter_test.lit", &lit(&[0xffff_ffff; 2]));
        let candidates = [
            (None, LightingStatus::Missing),
            (Some(b"NOPE".to_vec()), LightingStatus::UnrecognizedMagic),
            (
                Some(lit(&[1])),
                LightingStatus::CountMismatch {
                    supplied: 1,
                    expected: 2,
                },
            ),
        ];
        for (candidate, expected) in candidates {
            let files = candidate
                .into_iter()
                .map(|b| ("ter_test.lit", b))
                .collect::<Vec<_>>();
            let archive = archive(&files);
            let selected = select(&fixture.0, "zone", &archive, "ter_test.ter", true, 2).unwrap();
            assert_eq!(selected.colors, [DEFAULT_LIGHTING; 2]);
            assert_eq!(selected.provenance.status, expected);
        }
    }

    #[test]
    fn ineligible_loose_zon_falls_through_but_truncated_selected_streams_error() {
        let fixture = Fixture::new();
        let archive = archive(&[("ter_test.lit", lit(&[0x1234_5678]))]);
        for header in [
            words(&[ZON_MAGIC, 1]),
            b"EQTZP".to_vec(),
            words(&[ZON_MAGIC, 3]),
        ] {
            fixture.write("zone.zon", &header);
            assert_eq!(
                select(&fixture.0, "zone", &archive, "ter_test.ter", true, 1)
                    .unwrap()
                    .colors,
                [0x1234_5678]
            );
        }
        let complete = zon(&[0x1234_5678], &[9]);
        let first_end = 28 + 1 + 2 * 4 + 40 + 4;
        for end in 0..first_end {
            fixture.write("zone.zon", &complete[..end]);
            assert!(
                select(&fixture.0, "zone", &archive, "ter_test.ter", true, 1).is_err(),
                "ZON prefix {end}"
            );
        }
        fixture.write("zone.zon", &complete[..first_end]);
        assert_eq!(
            select(&fixture.0, "zone", &archive, "ter_test.ter", true, 1)
                .unwrap()
                .colors,
            [0x1234_5678]
        );
        std::fs::remove_file(fixture.0.join("zone.zon")).unwrap();
        let full_lit = lit(&[0x1234_5678]);
        for end in 0..full_lit.len() {
            fixture.write("ter_test.lit", &full_lit[..end]);
            assert!(
                select(&fixture.0, "zone", &archive, "ter_test.ter", false, 1).is_err(),
                "LIT prefix {end}"
            );
        }
        let mut oversized = zon(&[1], &[]);
        oversized[28 + 1 + 2 * 4 + 36..28 + 1 + 2 * 4 + 40]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(first_loose_placement_lighting(&oversized).is_err());
        oversized[16..20].copy_from_slice(&0u32.to_le_bytes());
        assert!(first_loose_placement_lighting(&oversized).is_err());
    }

    #[test]
    #[ignore = "requires original EverQuest assets"]
    fn original_sources_and_native_count_mismatches_have_explicit_provenance() {
        let base = crate::loader::default_client_dir().unwrap();
        for (zone, ter, count, supplied) in [
            ("causeway", "ter_gorge.ter", 111_269, 111_269),
            ("dranikhollowsa", "ter_cavea.ter", 131_590, 89_819),
            ("dranikhollowsb", "ter_caveb.ter", 98_382, 85_582),
            ("dranikhollowsc", "ter_cavec.ter", 101_791, 77_873),
        ] {
            let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
            let selection = select(&base, zone, &archive, ter, true, count).unwrap();
            assert_eq!(selection.colors.len(), count);
            assert!(matches!(
                selection.provenance.source,
                LightingSource::ArchiveLit { .. }
            ));
            if count == supplied {
                assert_eq!(
                    selection.provenance.status,
                    LightingStatus::Matched { count }
                );
                for (i, color) in [
                    (87_375, 0x001b_1202),
                    (87_468, 0x4c0d_0901),
                    (96_771, 0),
                    (103_109, 0xe500_0000),
                ] {
                    assert_eq!(selection.colors[i], color);
                }
            } else {
                assert_eq!(
                    selection.provenance.status,
                    LightingStatus::CountMismatch {
                        supplied,
                        expected: count
                    }
                );
                assert!(
                    selection
                        .colors
                        .iter()
                        .all(|&color| color == DEFAULT_LIGHTING)
                );
            }
        }
        for (zone, ter, count, index, color) in [
            (
                "guildhall",
                "ter_guildhall.ter",
                28_584,
                16_302,
                0x32a6_a6a6,
            ),
            ("guildlobby", "ter_guildlobby.ter", 57_912, 14, 0x0068_6456),
            ("roost", "ter_roost.ter", 73_847, 72_880, 0x0002_0202),
        ] {
            let archive = Archive::open(base.join(format!("{zone}.eqg"))).unwrap();
            let selected = select(&base, zone, &archive, ter, true, count).unwrap();
            assert!(matches!(
                selected.provenance.source,
                LightingSource::LooseZon { .. }
            ));
            assert_eq!(
                selected.provenance.status,
                LightingStatus::Matched { count }
            );
            assert_eq!(selected.colors[index], color);
        }
    }
}
