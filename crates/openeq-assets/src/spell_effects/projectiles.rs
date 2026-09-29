//! Particle bindings carried by the original missile EQG archive.

use std::collections::BTreeMap;

use super::*;
use crate::pfs::Archive;

#[derive(Clone, Debug)]
pub struct ProjectileEmitterBinding {
    /// Index in actoremittersnew.edd, not spellsnew.edd.
    pub emitter_id: u32,
    pub point_name: String,
    pub attachment_name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
    /// Undecoded words following the point name in a PTCL version 5 record.
    pub raw_words: [u32; 10],
}

impl ProjectileEmitterBinding {
    pub fn is_identity_origin(&self) -> bool {
        self.attachment_name
            .eq_ignore_ascii_case("ATTACH_TO_ORIGIN")
            && self.position == [0.0; 3]
            && self.rotation == [0.0; 3]
            && self.scale == [1.0; 3]
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProjectileEffectCatalog {
    /// Complete actor emitter table, preserving native indices.
    pub emitters: Vec<EmitterDefinition>,
    /// Canonical uppercase model code to supported original particle bindings.
    pub bindings: BTreeMap<String, Vec<ProjectileEmitterBinding>>,
    /// Unsupported bindings or malformed individual model definitions.
    pub skipped: Vec<String>,
}

/// Load the original missile archive's authored actor effects. Archive name
/// spelling `missle.eqg` is intentional and matches OnDemandResources.txt.
/// Only identity ATTACH_TO_ORIGIN points are currently enabled. Other points
/// are reported, rather than silently placing their particles incorrectly.
pub fn load_projectile_effects(base: &Path) -> Result<ProjectileEffectCatalog> {
    let path = find_file(base, "actoremittersnew.edd")?;
    let bytes = read_bounded(&path, 8 + MAX_RECORDS * EMITTER_RECORD_BYTES)?;
    let actor_catalog = SpellEffectCatalog::parse(&[], &bytes)?;
    let archive_path = find_file(base, "missle.eqg")?;
    let archive = bounded_archive(&archive_path)?;
    let mut catalog = ProjectileEffectCatalog {
        emitters: actor_catalog.emitters,
        ..Default::default()
    };
    let names = archive
        .names()
        .iter()
        .filter(|name| name.to_ascii_lowercase().ends_with(".prt"))
        .cloned()
        .collect::<BTreeSet<_>>();
    if names.len() > 4096 {
        return Err(Error::Format(
            "too many projectile particle definitions".into(),
        ));
    }
    for name in names {
        let stem = &name[..name.len() - 4];
        if !stem.is_ascii()
            || stem.len() <= 2
            || !stem[..2].eq_ignore_ascii_case("IT")
            || !stem[2..].bytes().all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let result = archive.read(&name).and_then(|prt| {
            archive
                .read(&format!("{stem}.pts"))
                .and_then(|pts| parse_bindings(&prt, &pts, catalog.emitters.len()))
        });
        match result {
            Ok(bindings) => {
                let mut supported = Vec::new();
                for binding in bindings {
                    if binding.is_identity_origin() {
                        supported.push(binding);
                    } else {
                        catalog.skipped.push(format!(
                            "{stem}: unsupported particle attachment {} ({})",
                            binding.point_name, binding.attachment_name
                        ));
                    }
                }
                if !supported.is_empty() {
                    catalog
                        .bindings
                        .insert(stem.to_ascii_uppercase(), supported);
                }
            }
            Err(error) => catalog.skipped.push(format!("{stem}: {error}")),
        }
    }
    Ok(catalog)
}

// The general PFS reader trusts declared inflate sizes. Preflight this small
// catalog archive before it allocates/decompresses any payload or directory.
fn bounded_archive(path: &Path) -> Result<Archive> {
    let bytes = read_bounded(path, MAX_TEXTURE_BYTES)?;
    validate_archive_sizes(&bytes)?;
    Archive::from_bytes(path.into(), bytes)
}

fn validate_archive_sizes(data: &[u8]) -> Result<()> {
    let invalid = || Error::Format("invalid or oversized projectile archive".into());
    if data.len() < 12 || data.get(4..8) != Some(b"PFS ") {
        return Err(invalid());
    }
    let directory = word(data, 0) as usize;
    if directory.checked_add(4).is_none_or(|end| end > data.len()) {
        return Err(invalid());
    }
    let count = word(data, directory) as usize;
    if count > 4096
        || directory
            .checked_add(4 + count * 12)
            .is_none_or(|end| end > data.len())
    {
        return Err(invalid());
    }
    let mut total = 0usize;
    for index in 0..count {
        let entry = directory + 4 + index * 12;
        let mut cursor = word(data, entry + 4) as usize;
        let size = word(data, entry + 8) as usize;
        total += size;
        if size > 4 * 1024 * 1024 || total > 64 * 1024 * 1024 {
            return Err(invalid());
        }
        let mut inflated = 0;
        while inflated < size {
            if cursor.checked_add(8).is_none_or(|end| end > data.len()) {
                return Err(invalid());
            }
            let compressed = word(data, cursor) as usize;
            let chunk = word(data, cursor + 4) as usize;
            if chunk == 0 || chunk > size - inflated {
                return Err(invalid());
            }
            cursor = cursor
                .checked_add(8 + compressed)
                .filter(|end| *end <= data.len())
                .ok_or_else(invalid)?;
            inflated += chunk;
        }
    }
    Ok(())
}

fn table<'a>(data: &'a [u8], magic: &[u8; 4], version: u32, stride: usize) -> Result<&'a [u8]> {
    if data.len() < 12 || &data[..4] != magic || word(data, 8) != version {
        return Err(Error::Format(format!(
            "unsupported projectile {} header/version",
            String::from_utf8_lossy(magic)
        )));
    }
    let count = word(data, 4) as usize;
    if count > 4096 || data.len() != 12 + count * stride {
        return Err(Error::Format(
            "invalid projectile particle record count/length".into(),
        ));
    }
    Ok(&data[12..])
}

fn parse_bindings(
    prt: &[u8],
    pts: &[u8],
    emitter_count: usize,
) -> Result<Vec<ProjectileEmitterBinding>> {
    let records = table(prt, b"PTCL", 5, 108)?;
    let points = table(pts, b"EQPT", 1, 164)?;
    let mut point_map = BTreeMap::new();
    for data in points.chunks_exact(164) {
        let name = fixed_string(&data[..64]).to_ascii_lowercase();
        if point_map.insert(name, data).is_some() {
            return Err(Error::Format("duplicate projectile particle point".into()));
        }
        for offset in (128..164).step_by(4) {
            if !float(data, offset).is_finite() {
                return Err(Error::Format(
                    "nonfinite projectile attachment transform".into(),
                ));
            }
        }
    }
    let mut result = Vec::new();
    for data in records.chunks_exact(108) {
        let emitter_id = word(data, 0);
        if emitter_id as usize >= emitter_count {
            return Err(Error::Format(format!(
                "projectile references missing actor emitter {emitter_id}"
            )));
        }
        if emitter_id == 0 {
            continue;
        }
        let point_name = fixed_string(&data[4..68]);
        let point = point_map
            .get(&point_name.to_ascii_lowercase())
            .ok_or_else(|| Error::NotFound(format!("projectile particle point {point_name}")))?;
        result.push(ProjectileEmitterBinding {
            emitter_id,
            point_name,
            attachment_name: fixed_string(&point[64..128]),
            position: std::array::from_fn(|i| float(point, 128 + i * 4)),
            rotation: std::array::from_fn(|i| float(point, 140 + i * 4)),
            scale: std::array::from_fn(|i| float(point, 152 + i * 4)),
            raw_words: std::array::from_fn(|i| word(data, 68 + i * 4)),
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<u8>, Vec<u8>) {
        let mut prt = vec![0; 120];
        prt[..4].copy_from_slice(b"PTCL");
        prt[4..8].copy_from_slice(&1u32.to_le_bytes());
        prt[8..12].copy_from_slice(&5u32.to_le_bytes());
        prt[12..16].copy_from_slice(&2u32.to_le_bytes());
        prt[16..22].copy_from_slice(b"POINT1");
        let mut pts = vec![0; 176];
        pts[..4].copy_from_slice(b"EQPT");
        pts[4..8].copy_from_slice(&1u32.to_le_bytes());
        pts[8..12].copy_from_slice(&1u32.to_le_bytes());
        pts[12..18].copy_from_slice(b"POINT1");
        pts[76..92].copy_from_slice(b"ATTACH_TO_ORIGIN");
        for offset in [164, 168, 172] {
            pts[offset..offset + 4].copy_from_slice(&1f32.to_le_bytes());
        }
        (prt, pts)
    }

    #[test]
    fn parses_actor_emitter_index_and_named_origin_point() {
        let (mut prt, mut pts) = fixture();
        prt[116..120].copy_from_slice(&u32::MAX.to_le_bytes());
        pts[40..44].copy_from_slice(&u32::MAX.to_le_bytes()); // stale name padding
        let bindings = parse_bindings(&prt, &pts, 3).unwrap();
        assert_eq!(bindings[0].emitter_id, 2);
        assert_eq!(bindings[0].raw_words[9], u32::MAX);
        assert!(bindings[0].is_identity_origin());
        pts[140..144].copy_from_slice(&2f32.to_le_bytes());
        assert!(!parse_bindings(&prt, &pts, 3).unwrap()[0].is_identity_origin());
    }

    #[test]
    fn rejects_bad_particle_counts_versions_references_and_transforms() {
        let (mut prt, mut pts) = fixture();
        assert!(parse_bindings(&prt[..119], &pts, 3).is_err());
        assert!(parse_bindings(&prt, &pts, 2).is_err());
        prt[8] = 4;
        assert!(parse_bindings(&prt, &pts, 3).is_err());
        prt[8] = 5;
        pts[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_bindings(&prt, &pts, 3).is_err());
        pts[4..8].copy_from_slice(&1u32.to_le_bytes());
        pts[140..144].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(parse_bindings(&prt, &pts, 3).is_err());
    }

    #[test]
    fn rejects_archive_inflate_claims_before_decompression() {
        let mut bytes = vec![0; 36];
        bytes[..4].copy_from_slice(&20u32.to_le_bytes());
        bytes[4..8].copy_from_slice(b"PFS ");
        bytes[20..24].copy_from_slice(&1u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&12u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_archive_sizes(&bytes).is_err());
        bytes[32..36].copy_from_slice(&1u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_archive_sizes(&bytes).is_err());
    }

    #[test]
    #[ignore = "requires an installed EverQuest client (EQ_DIR)"]
    fn installed_missiles_select_original_actor_emitters() {
        let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR");
        let catalog = load_projectile_effects(Path::new(&base)).unwrap();
        assert!(catalog.skipped.is_empty(), "{:?}", catalog.skipped);
        assert_eq!(catalog.bindings.len(), 17);
        for id in 11503..=11519 {
            let bindings = &catalog.bindings[&format!("IT{id}")];
            assert_eq!(bindings.len(), 1);
            assert_eq!(bindings[0].emitter_id, id - 11503 + 471);
            assert!(bindings[0].is_identity_origin());
        }
        assert_eq!(catalog.emitters[472].name, "missle_fire_red");
        assert_eq!(catalog.emitters[472].texture, "fire_missle.dds");
    }
}
