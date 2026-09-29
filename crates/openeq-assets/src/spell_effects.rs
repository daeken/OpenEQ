//! Authored spell stages (`spellsnew.eff`) and particle emitters (`spellsnew.edd`).
//!
//! The EDD 110 layout was checked against the original graphics DLL. Unresolved
//! fields remain available as raw words; attachment identifiers are deliberately
//! not assigned speculative bone names. See `docs/SPELL_EFFECT_FORMAT.md`.

use std::{
    collections::{BTreeSet, HashMap},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{Error, Result, texture::Texture};

mod projectiles;
pub use projectiles::{ProjectileEffectCatalog, ProjectileEmitterBinding, load_projectile_effects};

pub const EFFECT_RECORD_BYTES: usize = 268;
pub const EMITTER_RECORD_BYTES: usize = 416;
const MAX_RECORDS: usize = 65_536;
const MAX_TEXTURE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TEXTURE_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum EffectStage {
    Cast = 0,
    Projectile = 1,
    Impact = 2,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EmitterReference {
    /// Zero is an empty slot. Other values index the EDD table directly.
    pub emitter_id: u32,
    pub unknown: u32,
    /// Original attachment/emitter mode. Its full enum is not decoded yet.
    pub mode: u32,
    /// Original actor attachment identifier (4/5 are paired casting hands).
    pub attachment: u32,
}

#[derive(Clone, Debug, Default)]
pub struct EffectStageDefinition {
    pub sound_id: u32,
    pub emitters: [EmitterReference; 4],
}

#[derive(Clone, Debug)]
pub struct EffectDefinition {
    pub name: String,
    /// Casting, projectile travel, target impact, in file order.
    pub stages: [EffectStageDefinition; 3],
}

pub type SpellEffectDefinition = EffectDefinition;

impl EffectDefinition {
    pub fn stage(&self, stage: EffectStage) -> &EffectStageDefinition {
        &self.stages[stage as usize]
    }
}

#[derive(Clone, Debug)]
pub struct EmitterDefinition {
    pub name: String,
    pub texture: String,
    /// Negative values force their absolute duration; nonnegative values can
    /// be replaced by the caller's requested duration. This is NOT gravity.
    pub emitter_lifetime_raw: f32,
    /// Particle lifetime in seconds. Nonpositive means use emitter lifetime.
    pub lifetime: f32,
    pub initial_particles: u32,
    pub particles_per_emission: u32,
    /// Emission ticks per second, each spawning `particles_per_emission`.
    pub emission_rate: f32,
    pub emission_delay: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub grow_time: f32,
    pub shrink_time: f32,
    pub opacity: f32,
    /// Native 0..9 shape enum. Preserve unknown future values.
    pub shape: u32,
    pub shape_dimensions: [f32; 3],
    /// Offset along the three emitter-local basis vectors, not world XYZ.
    pub offset: [f32; 3],
    /// Native angular units; a full revolution is 512 units.
    pub orientation: [f32; 2],
    /// Random width and height endpoints. Authored endpoints may be reversed.
    pub size_ranges: [[f32; 2]; 2],
    /// Width and height reuse a random factor, but keep separate ranges.
    /// This preserves aspect correlation; it does not force square sprites.
    pub correlated_size: bool,
    /// Authored 0..255 RGB endpoints, interpolated over particle lifetime.
    pub color_start: [u32; 3],
    pub color_end: [u32; 3],
    /// Axial and two transverse local velocities, before radial/orbit motion.
    pub velocity_ranges: [[f32; 2]; 3],
    pub acceleration: [f32; 3],
    pub radial_velocity: [f32; 2],
    pub radial_acceleration: f32,
    pub orbit_velocity: [f32; 2],
    pub orbit_acceleration: f32,
    /// Positive values accelerate down the EQ world Z axis.
    pub gravity: f32,
    /// Acceleration along native world X (independent of emitter basis).
    pub wind: f32,
    pub frame_count: u32,
    pub frames_per_second: f32,
    /// Native angular units per second, random endpoints.
    pub spin: [f32; 2],
    pub random_rotation: bool,
    pub additive: bool,
    pub depth_write: bool,
    pub follow_attachment: bool,
    /// Use an attachment's orientation when available; otherwise actor basis.
    pub use_attachment_basis: bool,
    /// Apply actor scale to billboard width/height (native flag 108).
    pub scale_particle_size: bool,
    /// Apply actor scale to emitter basis and local offsets (native flag 412).
    pub scale_emitter_basis: bool,
    /// Every original little-endian word at record bytes 96..416, unmodified.
    pub raw_words: [u32; 80],
}

impl EmitterDefinition {
    /// Transform an axial/transverse vector by the two authored orientation
    /// angles, expressed in the original unrotated local basis. Apply the
    /// attachment-to-world transform afterward. Position `offset` is applied
    /// separately in the unrotated basis, as in the original client.
    pub fn orient_local_vector(&self, vector: [f32; 3]) -> [f32; 3] {
        let radians = std::f32::consts::TAU / 512.0;
        let (sin_theta, cos_theta) = (self.orientation[0].rem_euclid(512.0) * radians).sin_cos();
        let (sin_phi, cos_phi) = (self.orientation[1].rem_euclid(512.0) * radians).sin_cos();
        let axial = cos_phi * vector[0] - sin_phi * vector[2];
        [
            cos_theta * axial - sin_theta * vector[1],
            sin_theta * axial + cos_theta * vector[1],
            sin_phi * vector[0] + cos_phi * vector[2],
        ]
    }

    /// UV rectangle in top-left texture coordinates. The native grids are
    /// 2x2 for 2..4 frames, 4x2 for 5..8, and 4x4 for 9..16.
    pub fn uv_rect(&self, age: f32) -> [f32; 4] {
        let (columns, rows) = match self.frame_count {
            2..=4 => (2, 2),
            5..=8 => (4, 2),
            9..=16 => (4, 4),
            _ => return [0.0, 0.0, 1.0, 1.0],
        };
        let time = if age.is_finite() { age.max(0.0) } else { 0.0 };
        let frame = ((time as f64 * self.frames_per_second as f64)
            .floor()
            .rem_euclid(self.frame_count as f64)) as u32;
        let x = (frame % columns) as f32 / columns as f32;
        let y = (frame / columns) as f32 / rows as f32;
        [x, y, x + 1.0 / columns as f32, y + 1.0 / rows as f32]
    }

    /// Normalized authored sRGB and alpha. Convert RGB to linear before
    /// supplying a renderer that expects linear tint values.
    pub fn color_rgba(&self, age: f32) -> [f32; 4] {
        self.color_rgba_with_lifetime(age, self.lifetime)
    }

    /// Use the actual particle lifetime when the authored value is nonpositive.
    pub fn color_rgba_with_lifetime(&self, age: f32, lifetime: f32) -> [f32; 4] {
        let age = finite_nonnegative(age);
        let lifetime = finite_nonnegative(lifetime);
        let t = if lifetime > 0.0 {
            (age / lifetime).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut rgba = [0.0; 4];
        for (channel, value) in rgba[..3].iter_mut().enumerate() {
            let a = self.color_start[channel] as f32;
            let b = self.color_end[channel] as f32;
            *value = ((a + (b - a) * t) / 255.0).clamp(0.0, 1.0);
        }
        // Native alpha applies both ramps then caps, rather than multiplying
        // the peak opacity by the ramps.
        rgba[3] = envelope(age, lifetime, self.fade_in, self.fade_out)
            .min(self.opacity.max(0.0))
            .clamp(0.0, 1.0);
        rgba
    }

    pub fn size_envelope(&self, age: f32) -> f32 {
        self.size_envelope_with_lifetime(age, self.lifetime)
    }

    pub fn size_envelope_with_lifetime(&self, age: f32, lifetime: f32) -> f32 {
        envelope(
            finite_nonnegative(age),
            finite_nonnegative(lifetime),
            self.grow_time,
            self.shrink_time,
        )
    }
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

fn envelope(age: f32, lifetime: f32, start: f32, end: f32) -> f32 {
    let remaining = (lifetime - age).max(0.0);
    let a = if start > 0.0 {
        (age / start).min(1.0)
    } else {
        1.0
    };
    let b = if end > 0.0 {
        (remaining / end).min(1.0)
    } else {
        1.0
    };
    a * b
}

#[derive(Clone, Debug, Default)]
pub struct SpellEffectCatalog {
    pub effects: Vec<EffectDefinition>,
    pub emitters: Vec<EmitterDefinition>,
}

impl SpellEffectCatalog {
    pub fn load(base: &Path) -> Result<Self> {
        let eff = find_file(base, "spellsnew.eff")?;
        let edd = find_file(base, "spellsnew.edd")?;
        Self::parse(
            &read_bounded(&eff, MAX_RECORDS * EFFECT_RECORD_BYTES)?,
            &read_bounded(&edd, 8 + MAX_RECORDS * EMITTER_RECORD_BYTES)?,
        )
    }

    pub fn parse(effect_bytes: &[u8], emitter_bytes: &[u8]) -> Result<Self> {
        if !emitter_bytes.starts_with(b"EDD\0") {
            return Err(Error::Format("spell emitter file has no EDD header".into()));
        }
        if emitter_bytes.get(4..8) != Some(b"110\0") {
            return Err(Error::Format(
                "unsupported spell emitter version (expected EDD 110)".into(),
            ));
        }
        validate_records(effect_bytes, EFFECT_RECORD_BYTES, "spell effects")?;
        validate_records(&emitter_bytes[8..], EMITTER_RECORD_BYTES, "spell emitters")?;
        let emitters = emitter_bytes[8..]
            .chunks_exact(EMITTER_RECORD_BYTES)
            .enumerate()
            .map(|(id, data)| parse_emitter(id, data))
            .collect::<Result<Vec<_>>>()?;
        let mut effects = Vec::with_capacity(effect_bytes.len() / EFFECT_RECORD_BYTES);
        for (id, data) in effect_bytes.chunks_exact(EFFECT_RECORD_BYTES).enumerate() {
            let name = fixed_string(&data[..64]);
            let stages = std::array::from_fn(|stage| {
                let start = 64 + stage * 68;
                EffectStageDefinition {
                    sound_id: word(data, start),
                    emitters: std::array::from_fn(|slot| {
                        let start = start + 4 + slot * 16;
                        EmitterReference {
                            emitter_id: word(data, start),
                            unknown: word(data, start + 4),
                            mode: word(data, start + 8),
                            attachment: word(data, start + 12),
                        }
                    }),
                }
            });
            for reference in stages.iter().flat_map(|stage| stage.emitters) {
                if reference.emitter_id != 0 && reference.emitter_id as usize >= emitters.len() {
                    return Err(Error::Format(format!(
                        "spell effect {id} references missing emitter {}",
                        reference.emitter_id
                    )));
                }
            }
            effects.push(EffectDefinition { name, stages });
        }
        Ok(Self { effects, emitters })
    }

    pub fn effect(&self, id: u32) -> Option<&EffectDefinition> {
        self.effects.get(id as usize)
    }

    pub fn emitter(&self, id: u32) -> Option<&EmitterDefinition> {
        self.emitters.get(id as usize)
    }

    /// Decode referenced original textures once, in stable filename order.
    /// Missing or damaged assets are reported and omitted, never replaced with
    /// magenta. The returned Arc can be shared with every particle frame.
    pub fn load_textures(&self, base: &Path) -> Result<SpellEffectTextures> {
        let base_files = directory_index(base)?;
        let mut files = HashMap::new();
        // The original graphics DLL searches these three loose texture
        // directories in order; spell emitters also use environment textures.
        for name in ["spelleffects", "envemittereffects", "actoreffects"] {
            if let Some(directory) = base_files.get(name) {
                for (name, path) in directory_index(directory)? {
                    files.entry(name).or_insert(path);
                }
            }
        }
        let names = self
            .emitters
            .iter()
            .map(|e| e.texture.to_ascii_lowercase())
            .filter(|name| !name.is_empty() && name != "none")
            .collect::<BTreeSet<_>>();
        let mut result = SpellEffectTextures::default();
        let mut decoded = Vec::new();
        let mut decoded_bytes = 0usize;
        for name in names {
            let Some(path) = files.get(&name) else {
                result.missing.push(name);
                continue;
            };
            let texture =
                read_bounded(path, MAX_TEXTURE_BYTES).and_then(|data| decode_texture(&name, &data));
            match texture {
                Ok(texture) => {
                    if decoded_bytes + texture.rgba.len() > MAX_DECODED_BYTES {
                        result
                            .errors
                            .push(format!("{name}: decoded spell texture budget exceeded"));
                        continue;
                    }
                    decoded_bytes += texture.rgba.len();
                    result.indices.insert(name, decoded.len() as u32);
                    decoded.push(texture);
                }
                Err(error) => result.errors.push(format!("{name}: {error}")),
            }
        }
        result.textures = Arc::new(decoded);
        Ok(result)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SpellEffectTextures {
    pub textures: Arc<Vec<Texture>>,
    /// Lowercase authored filename to stable texture index.
    pub indices: HashMap<String, u32>,
    pub missing: Vec<String>,
    pub errors: Vec<String>,
}

impl SpellEffectTextures {
    pub fn index(&self, name: &str) -> Option<u32> {
        self.indices.get(&name.to_ascii_lowercase()).copied()
    }
}

fn parse_emitter(id: usize, data: &[u8]) -> Result<EmitterDefinition> {
    let raw_words = std::array::from_fn(|i| word(data, 96 + i * 4));
    // Only known floating-point slots are validated as floats. Unknown words
    // can legitimately contain integer flags or sentinel bit patterns.
    const FLOATS: &[usize] = &[
        120, 124, 136, 140, 144, 148, 152, 156, 160, 164, 172, 176, 180, 184, 188, 192, 196, 200,
        204, 208, 236, 240, 244, 248, 252, 256, 260, 264, 268, 272, 276, 280, 284, 288, 292, 296,
        300, 308, 312, 364, 368, 380, 384, 388, 392, 400, 404,
    ];
    for &offset in FLOATS {
        if !float(data, offset).is_finite() {
            return Err(Error::Format(format!(
                "spell emitter {id} has nonfinite float at byte {offset}"
            )));
        }
    }
    let texture = fixed_string(&data[64..96]);
    if texture.contains(['/', '\\', ':'])
        || texture == "."
        || texture == ".."
        || texture.chars().any(char::is_control)
    {
        return Err(Error::Format(format!(
            "spell emitter {id} has unsafe texture filename"
        )));
    }
    let triple = |offset| std::array::from_fn(|i| float(data, offset + i * 4));
    Ok(EmitterDefinition {
        name: fixed_string(&data[..64]),
        texture,
        emitter_lifetime_raw: float(data, 120),
        lifetime: float(data, 124),
        initial_particles: word(data, 128),
        particles_per_emission: word(data, 132),
        emission_rate: float(data, 136),
        emission_delay: float(data, 140),
        fade_in: float(data, 144),
        fade_out: float(data, 148),
        grow_time: float(data, 152),
        shrink_time: float(data, 156),
        opacity: float(data, 164),
        shape: word(data, 168),
        shape_dimensions: triple(172),
        offset: triple(184),
        orientation: [float(data, 196), float(data, 200)],
        size_ranges: [
            [float(data, 204), float(data, 388)],
            [float(data, 380), float(data, 384)],
        ],
        correlated_size: word(data, 396) == 1,
        color_start: std::array::from_fn(|i| word(data, 212 + i * 4)),
        color_end: std::array::from_fn(|i| word(data, 224 + i * 4)),
        velocity_ranges: std::array::from_fn(|i| {
            [float(data, 236 + i * 12), float(data, 240 + i * 12)]
        }),
        acceleration: std::array::from_fn(|i| float(data, 244 + i * 12)),
        radial_velocity: [float(data, 272), float(data, 276)],
        radial_acceleration: float(data, 280),
        orbit_velocity: [float(data, 284), float(data, 288)],
        orbit_acceleration: float(data, 292),
        gravity: float(data, 296),
        wind: float(data, 300),
        frame_count: word(data, 304),
        frames_per_second: float(data, 308),
        spin: [float(data, 312), float(data, 392)],
        random_rotation: word(data, 372) == 1,
        additive: word(data, 104) == 1,
        depth_write: word(data, 100) != 1,
        follow_attachment: word(data, 116) == 1,
        use_attachment_basis: word(data, 96) == 1,
        scale_particle_size: word(data, 108) == 1,
        scale_emitter_basis: word(data, 412) == 1,
        raw_words,
    })
}

fn validate_records(data: &[u8], stride: usize, label: &str) -> Result<()> {
    if !data.len().is_multiple_of(stride) || data.len() / stride > MAX_RECORDS {
        return Err(Error::Format(format!(
            "invalid {label} table length {} (stride {stride}, limit {MAX_RECORDS})",
            data.len()
        )));
    }
    Ok(())
}

fn fixed_string(data: &[u8]) -> String {
    let end = data
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).into_owned()
}

fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn float(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(word(data, offset))
}

fn directory_index(directory: &Path) -> Result<HashMap<String, PathBuf>> {
    let entries = fs::read_dir(directory).map_err(|source| Error::Io {
        path: directory.into(),
        source,
    })?;
    let mut files = HashMap::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: directory.into(),
            source,
        })?;
        if let Some(name) = entry.file_name().to_str() {
            files.insert(name.to_ascii_lowercase(), entry.path());
        }
    }
    Ok(files)
}

fn find_file(directory: &Path, name: &str) -> Result<PathBuf> {
    directory_index(directory)?
        .remove(&name.to_ascii_lowercase())
        .ok_or_else(|| Error::NotFound(directory.join(name).display().to_string()))
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| Error::Io {
            path: path.into(),
            source,
        })?;
    if bytes.len() > limit {
        return Err(Error::Format(format!(
            "asset exceeds byte limit: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

fn decode_texture(name: &str, data: &[u8]) -> Result<Texture> {
    let tga = !data.starts_with(b"DDS ") && name.to_ascii_lowercase().ends_with(".tga");
    let dimensions = if data.starts_with(b"DDS ") && data.len() >= 128 {
        (word(data, 16), word(data, 12))
    } else if tga && data.len() >= 18 {
        (
            u16::from_le_bytes([data[12], data[13]]) as u32,
            u16::from_le_bytes([data[14], data[15]]) as u32,
        )
    } else {
        return Err(Error::Format(format!(
            "unsupported spell texture header: {name}"
        )));
    };
    if dimensions.0 == 0
        || dimensions.1 == 0
        || dimensions.0 > 8192
        || dimensions.1 > 8192
        || dimensions.0 as u64 * dimensions.1 as u64 > MAX_TEXTURE_PIXELS
    {
        return Err(Error::Format(format!(
            "invalid spell texture dimensions: {name}"
        )));
    }
    if !tga {
        return Texture::decode(name, data);
    }
    // TGA has no unique magic, so image's automatic sniffing cannot detect it.
    let decoded = image::load_from_memory_with_format(data, image::ImageFormat::Tga)
        .map_err(|source| Error::Image {
            name: name.into(),
            source,
        })?
        .into_rgba8();
    Ok(Texture {
        name: name.into(),
        width: decoded.width(),
        height: decoded.height(),
        rgba: decoded.into_raw(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> (Vec<u8>, Vec<u8>) {
        let eff = vec![0; EFFECT_RECORD_BYTES];
        let mut edd = b"EDD\0".iter().chain(b"110\0").copied().collect::<Vec<_>>();
        edd.resize(8 + 2 * EMITTER_RECORD_BYTES, 0);
        (eff, edd)
    }

    fn set_float(data: &mut [u8], offset: usize, value: f32) {
        data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn reads_stages_and_ignores_stale_string_padding() {
        let (mut eff, mut edd) = tables();
        eff[..9].copy_from_slice(b"Frost\0old");
        eff[64..68].copy_from_slice(&107u32.to_le_bytes());
        eff[68..72].copy_from_slice(&1u32.to_le_bytes());
        eff[80..84].copy_from_slice(&4u32.to_le_bytes());
        let start = 8 + EMITTER_RECORD_BYTES;
        edd[start..start + 10].copy_from_slice(b"Mist\0stale");
        edd[start + 64..start + 77].copy_from_slice(b"mist.dds\0junk");
        set_float(&mut edd, start + 124, 2.5);
        set_float(&mut edd, start + 120, -5.0);
        set_float(&mut edd, start + 296, 1.5);
        let catalog = SpellEffectCatalog::parse(&eff, &edd).unwrap();
        assert_eq!(catalog.effects[0].name, "Frost");
        assert_eq!(catalog.effects[0].stage(EffectStage::Cast).sound_id, 107);
        assert_eq!(catalog.effects[0].stages[0].emitters[0].attachment, 4);
        assert_eq!(catalog.emitters[1].texture, "mist.dds");
        assert_eq!(catalog.emitters[1].emitter_lifetime_raw, -5.0);
        assert_eq!(catalog.emitters[1].lifetime, 2.5);
        assert_eq!(catalog.emitters[1].gravity, 1.5);
    }

    #[test]
    fn rejects_bad_versions_lengths_references_and_nonfinite_values() {
        let (mut eff, mut edd) = tables();
        assert!(SpellEffectCatalog::parse(&eff, b"EDD\0").is_err());
        edd[4] = b'2';
        assert!(SpellEffectCatalog::parse(&eff, &edd).is_err());
        edd[4] = b'1';
        assert!(SpellEffectCatalog::parse(&eff[..267], &edd).is_err());
        assert!(SpellEffectCatalog::parse(&eff, &edd[..edd.len() - 1]).is_err());
        eff[68..72].copy_from_slice(&2u32.to_le_bytes());
        assert!(SpellEffectCatalog::parse(&eff, &edd).is_err());
        eff[68..72].fill(0);
        set_float(&mut edd, 8 + 124, f32::NAN);
        assert!(SpellEffectCatalog::parse(&eff, &edd).is_err());
        assert!(
            validate_records(
                &vec![0; (MAX_RECORDS + 1) * EFFECT_RECORD_BYTES],
                EFFECT_RECORD_BYTES,
                "test"
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_texture_traversal_but_retains_unknown_word_bits() {
        let (eff, mut edd) = tables();
        edd[72..80].copy_from_slice(b"../a.dds");
        assert!(SpellEffectCatalog::parse(&eff, &edd).is_err());
        edd[72..80].fill(0);
        edd[8 + 340..8 + 344].copy_from_slice(&u32::MAX.to_le_bytes());
        let catalog = SpellEffectCatalog::parse(&eff, &edd).unwrap();
        assert_eq!(catalog.emitters[0].raw_words[(340 - 96) / 4], u32::MAX);
    }

    #[test]
    fn native_flipbook_grids_include_rectangular_eight_frame_sheet() {
        let (eff, edd) = tables();
        let mut emitter = SpellEffectCatalog::parse(&eff, &edd)
            .unwrap()
            .emitters
            .remove(0);
        emitter.frame_count = 8;
        emitter.frames_per_second = 8.0;
        assert_eq!(emitter.uv_rect(0.625), [0.25, 0.5, 0.5, 1.0]);
        assert_eq!(emitter.uv_rect(1.0), [0.0, 0.0, 0.25, 0.5]);
        emitter.frame_count = 16;
        assert_eq!(emitter.uv_rect(0.625), [0.25, 0.25, 0.5, 0.5]);
        emitter.frame_count = 17;
        assert_eq!(emitter.uv_rect(0.625), [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn interpolates_color_and_caps_alpha_after_fades() {
        let (eff, edd) = tables();
        let mut emitter = SpellEffectCatalog::parse(&eff, &edd)
            .unwrap()
            .emitters
            .remove(0);
        emitter.lifetime = 2.0;
        emitter.fade_in = 1.0;
        emitter.fade_out = 1.0;
        emitter.opacity = 0.5;
        emitter.color_start = [255, 0, 0];
        emitter.color_end = [0, 0, 255];
        assert_eq!(emitter.color_rgba(0.5), [0.75, 0.0, 0.25, 0.5]);
        assert_eq!(emitter.color_rgba(1.75), [0.125, 0.0, 0.875, 0.25]);
    }

    #[test]
    fn orientation_rotates_axial_toward_each_transverse_axis() {
        let (eff, edd) = tables();
        let mut emitter = SpellEffectCatalog::parse(&eff, &edd)
            .unwrap()
            .emitters
            .remove(0);
        let close = |actual: [f32; 3], expected: [f32; 3]| {
            for i in 0..3 {
                assert!((actual[i] - expected[i]).abs() < 1e-5);
            }
        };
        emitter.orientation = [128.0, 0.0];
        close(
            emitter.orient_local_vector([1.0, 0.0, 0.0]),
            [0.0, 1.0, 0.0],
        );
        emitter.orientation = [0.0, 128.0];
        close(
            emitter.orient_local_vector([1.0, 0.0, 0.0]),
            [0.0, 0.0, 1.0],
        );
        emitter.orientation = [128.0, 128.0];
        close(
            emitter.orient_local_vector([0.0, 0.0, 1.0]),
            [0.0, -1.0, 0.0],
        );
    }

    #[test]
    fn decodes_tga_alpha_with_top_left_origin_and_bounds_dimensions() {
        let mut bytes = vec![0; 18];
        bytes[2] = 2;
        bytes[12] = 1;
        bytes[14] = 1;
        bytes[16] = 32;
        bytes[17] = 0x28;
        bytes.extend_from_slice(&[30, 20, 10, 40]);
        assert_eq!(
            decode_texture("particle.tga", &bytes).unwrap().rgba,
            [10, 20, 30, 40]
        );
        bytes[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(decode_texture("particle.tga", &bytes).is_err());
    }

    #[test]
    fn texture_cache_searches_native_directories_in_order() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "openeq-spell-textures-{}-{unique}",
            std::process::id()
        ));
        let spell = base.join("SpellEffects");
        let environment = base.join("EnvEmitterEffects");
        fs::create_dir_all(&spell).unwrap();
        fs::create_dir_all(&environment).unwrap();
        let mut bytes = vec![0; 18];
        bytes[2] = 2;
        bytes[12] = 1;
        bytes[14] = 1;
        bytes[16] = 32;
        bytes[17] = 0x28;
        bytes.extend_from_slice(&[30, 20, 10, 40]);
        fs::write(spell.join("MiSt.TGA"), &bytes).unwrap();
        bytes[18] = 200;
        fs::write(environment.join("mist.tga"), &bytes).unwrap();
        fs::write(environment.join("smoke.tga"), &bytes).unwrap();
        let (eff, edd) = tables();
        let mut catalog = SpellEffectCatalog::parse(&eff, &edd).unwrap();
        catalog.emitters[0].texture = "mist.tga".into();
        catalog.emitters[1].texture = "smoke.tga".into();
        let mut missing = catalog.emitters[1].clone();
        missing.texture = "missing.tga".into();
        catalog.emitters.push(missing);
        let textures = catalog.load_textures(&base).unwrap();
        fs::remove_dir_all(&base).unwrap();
        assert_eq!(textures.missing, ["missing.tga"]);
        assert!(textures.errors.is_empty());
        assert_eq!(textures.index("MIST.TGA"), Some(0));
        assert_eq!(textures.index("smoke.tga"), Some(1));
        assert_eq!(textures.textures[0].rgba, [10, 20, 30, 40]);
        assert_eq!(textures.textures[1].rgba, [10, 20, 200, 40]);
        assert!(Arc::ptr_eq(&textures.textures, &textures.clone().textures));
    }

    #[test]
    #[ignore = "requires an installed EverQuest client (EQ_DIR)"]
    fn installed_client_catalog_and_textures() {
        let base = std::env::var_os("EQ_DIR").expect("set EQ_DIR to the EverQuest directory");
        let base = Path::new(&base);
        let catalog = SpellEffectCatalog::load(base).unwrap();
        let textures = catalog.load_textures(base).unwrap();
        println!(
            "{} effects, {} emitters, {} decoded textures; missing: {:?}; errors: {:?}",
            catalog.effects.len(),
            catalog.emitters.len(),
            textures.textures.len(),
            textures.missing,
            textures.errors
        );
        assert!(!catalog.effects.is_empty());
        assert!(!catalog.emitters.is_empty());
        assert!(!textures.textures.is_empty());
        assert!(textures.errors.is_empty());
        for effect_id in [179, 220, 218, 278] {
            let effect = catalog.effect(effect_id).unwrap();
            for slot in effect.stages.iter().flat_map(|stage| stage.emitters) {
                if slot.emitter_id == 0 {
                    continue;
                }
                let emitter = catalog.emitter(slot.emitter_id).unwrap();
                assert!(
                    textures.index(&emitter.texture).is_some(),
                    "missing core spell texture: {}",
                    emitter.texture
                );
            }
            println!("effect {effect_id}: {} {:?}", effect.name, effect.stages);
        }
    }
}
