//! Original zone sound metadata and lazy audio asset lookup. No device output.
//!
//! Classic EFF kinds and later EMT revisions are deliberately different types.
//! Undecoded fields remain available; parsing an emitter does not imply that
//! every original playback behavior is implemented. See `docs/AUDIO_PLAN.md`.

use crate::{Error, Result};

mod index;
mod tables;

pub use index::{AudioAsset, AudioAssetLocation, AudioCatalog, AudioFormat};
pub use tables::{Mp3Index, SoundBank, SoundIdTable};

pub const EFF_RECORD_BYTES: usize = 84;
pub const MAX_ZONE_EMITTERS: usize = 4096;
pub const MAX_AUDIO_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_AUDIO_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 4096;
const MAX_DIAGNOSTICS: usize = 256;
const MAX_TABLE_ROWS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Malformed,
    Unsupported,
    Missing,
    Duplicate,
    Precedence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDiagnostic {
    pub source: String,
    /// One-based text line or binary record number, when applicable.
    pub entry: Option<usize>,
    pub kind: DiagnosticKind,
    pub message: String,
}

/// Bounded diagnostics; callers can report suppression without per-frame logs.
#[derive(Clone, Debug, Default)]
pub struct AudioDiagnostics {
    pub entries: Vec<AudioDiagnostic>,
    pub suppressed: usize,
}

impl AudioDiagnostics {
    fn push(
        &mut self,
        source: &str,
        entry: Option<usize>,
        kind: DiagnosticKind,
        message: impl Into<String>,
    ) {
        if self.entries.len() < MAX_DIAGNOSTICS {
            self.entries.push(AudioDiagnostic {
                source: source.into(),
                entry,
                kind,
                message: message.into(),
            });
        } else {
            self.suppressed += 1;
        }
    }

    fn append(&mut self, other: &Self, source: &str) {
        for diagnostic in &other.entries {
            self.push(
                source,
                diagnostic.entry,
                diagnostic.kind,
                &diagnostic.message,
            );
        }
        self.suppressed += other.suppressed;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundNamespace {
    Global,
    EmitBank,
    LoopBank,
    Mp3Index,
    UnknownEmitter,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioReference {
    Silent,
    File(String),
    /// Sequence numbering is preserved, not converted to MIDI track indices.
    XmiSequence {
        file: String,
        sequence: i32,
    },
    Unresolved {
        id: i32,
        namespace: SoundNamespace,
    },
}

impl AudioReference {
    pub fn file_name(&self) -> Option<&str> {
        match self {
            Self::File(file) | Self::XmiSequence { file, .. } => Some(file),
            Self::Silent | Self::Unresolved { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClassicEmitterKind {
    Ambient,
    Music,
    /// Legacy effect kinds whose secondary selections/tail are not verified.
    Effect2,
    Effect3,
    Unknown(u8),
}

#[derive(Clone, Debug)]
pub struct ClassicEmitter {
    /// Zero-based record index in the original file, including rejected rows.
    pub record_index: usize,
    /// Authored asset/scene coordinates, Z up. Do not swap like network poses.
    pub position: [f32; 3],
    pub radius: f32,
    pub kind: ClassicEmitterKind,
    pub cooldown_ms: [i32; 2],
    pub random_delay_ms: i32,
    pub sound_ids: [i32; 2],
    pub sounds: [AudioReference; 2],
    /// Exact words, including opaque headers, padding and kind-dependent tail.
    pub raw_words: [u32; EFF_RECORD_BYTES / 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivePeriod {
    Always,
    Day,
    Night,
    Unknown(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmtLoopMode {
    Continuous,
    DelayedRepeat,
    Unknown(i32),
}

#[derive(Clone, Debug)]
pub struct EmtEmitter {
    pub line_number: usize,
    /// Revision-like first field; never interpret as ClassicEmitterKind.
    pub revision: i32,
    pub sound: AudioReference,
    pub flags: i32,
    pub active_period: ActivePeriod,
    /// Authored level; not clamped to the separate user volume range.
    pub gain: f32,
    pub fade_ms: [i32; 2],
    pub loop_mode: EmtLoopMode,
    pub position: [f32; 3],
    pub full_volume_radius: f32,
    pub max_audible_distance: f32,
    pub random_location_distance: f32,
    pub activation_range: f32,
    pub repeat_delay_ms: [i32; 2],
    pub xmi_index: i32,
    pub echo_level: f32,
    /// Absent in 19-field older rows; preserve non-boolean future values.
    pub environment_flag: Option<i32>,
    /// Fields 20 onward, preserved without assigning speculative meanings.
    pub extensions: Vec<String>,
    /// Trimmed original fields, including unknown values and exact numerals.
    pub raw_fields: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum AudioEmitter {
    Classic(ClassicEmitter),
    Emt(EmtEmitter),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoneAudioFormat {
    ClassicEff,
    Emt,
}

#[derive(Clone, Debug, Default)]
pub struct ZoneAudio {
    pub zone: String,
    /// None means neither authored emitter format was present.
    pub format: Option<ZoneAudioFormat>,
    pub emitters: Vec<AudioEmitter>,
    pub diagnostics: AudioDiagnostics,
}

impl ZoneAudio {
    pub fn parse_eff(
        zone: &str,
        bytes: &[u8],
        bank: &SoundBank,
        global: &SoundIdTable,
        mp3: &Mp3Index,
    ) -> Result<Self> {
        let zone = zone_name(zone)?;
        if !bytes.len().is_multiple_of(EFF_RECORD_BYTES)
            || bytes.len() / EFF_RECORD_BYTES > MAX_ZONE_EMITTERS
        {
            return Err(Error::Format("invalid EFF record count/size".into()));
        }
        let source = format!("{zone}_sounds.eff");
        let mut result = Self {
            zone: zone.clone(),
            format: Some(ZoneAudioFormat::ClassicEff),
            ..Self::default()
        };
        for (record_index, record) in bytes.chunks_exact(EFF_RECORD_BYTES).enumerate() {
            let words = std::array::from_fn(|i| {
                u32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap())
            });
            let position = std::array::from_fn(|i| f32::from_bits(words[4 + i]));
            let radius = f32::from_bits(words[7]);
            if !position.iter().all(|v| v.is_finite()) || !radius.is_finite() {
                result.diagnostics.push(
                    &source,
                    Some(record_index + 1),
                    DiagnosticKind::Malformed,
                    "non-finite coordinates or radius",
                );
                continue;
            }
            if radius < 0. {
                result.diagnostics.push(
                    &source,
                    Some(record_index + 1),
                    DiagnosticKind::Unsupported,
                    "negative legacy radius sentinel retained; activation semantics unverified",
                );
            }
            let kind = match record[56] {
                0 => ClassicEmitterKind::Ambient,
                1 => ClassicEmitterKind::Music,
                2 => ClassicEmitterKind::Effect2,
                3 => ClassicEmitterKind::Effect3,
                value => ClassicEmitterKind::Unknown(value),
            };
            if matches!(
                kind,
                ClassicEmitterKind::Effect2
                    | ClassicEmitterKind::Effect3
                    | ClassicEmitterKind::Unknown(_)
            ) {
                result.diagnostics.push(
                    &source,
                    Some(record_index + 1),
                    DiagnosticKind::Unsupported,
                    "legacy effect kind has unverified playback semantics; raw fields retained",
                );
            }
            let sound_ids = [words[12] as i32, words[13] as i32];
            let sounds = sound_ids.map(|id| match kind {
                ClassicEmitterKind::Music => mp3.resolve_music(&zone, id),
                ClassicEmitterKind::Unknown(_) if id != 0 => AudioReference::Unresolved {
                    id,
                    namespace: SoundNamespace::UnknownEmitter,
                },
                _ => bank.resolve_effect(id, global),
            });
            for sound in &sounds {
                if let AudioReference::Unresolved { id, namespace } = sound {
                    result.diagnostics.push(
                        &source,
                        Some(record_index + 1),
                        DiagnosticKind::Missing,
                        format!("unresolved sound ID {id} in {namespace:?}"),
                    );
                }
            }
            result.emitters.push(AudioEmitter::Classic(ClassicEmitter {
                record_index,
                position,
                radius,
                kind,
                cooldown_ms: [words[8] as i32, words[9] as i32],
                random_delay_ms: words[10] as i32,
                sound_ids,
                sounds,
                raw_words: words,
            }));
        }
        Ok(result)
    }

    pub fn parse_emt(zone: &str, text: &str) -> Result<Self> {
        let zone = zone_name(zone)?;
        check_text(text)?;
        let source = format!("{zone}.emt");
        let mut result = Self {
            zone,
            format: Some(ZoneAudioFormat::Emt),
            ..Self::default()
        };
        let mut count = 0;
        for (line, text) in text.lines().enumerate() {
            let text = text.trim().trim_start_matches('\u{feff}');
            if text.is_empty() || text.starts_with(['#', ';']) {
                continue;
            }
            count += 1;
            if count > MAX_ZONE_EMITTERS {
                return Err(Error::Format("too many EMT rows".into()));
            }
            match parse_emt_row(text, line + 1) {
                Ok(emitter) => {
                    if !matches!(emitter.revision, 1 | 2)
                        || emitter.flags != 0
                        || matches!(emitter.active_period, ActivePeriod::Unknown(_))
                        || matches!(emitter.loop_mode, EmtLoopMode::Unknown(_))
                        || emitter
                            .environment_flag
                            .is_some_and(|v| !matches!(v, 0 | 1))
                        || emitter.extensions.iter().any(|v| v != "0")
                    {
                        result.diagnostics.push(
                            &source,
                            Some(line + 1),
                            DiagnosticKind::Unsupported,
                            "unverified EMT revision, flags or controls retained",
                        );
                    }
                    result.emitters.push(AudioEmitter::Emt(emitter));
                }
                Err(error) => result.diagnostics.push(
                    &source,
                    Some(line + 1),
                    DiagnosticKind::Malformed,
                    error.to_string(),
                ),
            }
        }
        Ok(result)
    }
}

fn parse_emt_row(text: &str, line_number: usize) -> Result<EmtEmitter> {
    if text.len() > MAX_LINE_BYTES {
        return Err(Error::Format("EMT row exceeds byte limit".into()));
    }
    // Original EMT fields are unquoted comma-separated values. Quoted names
    // are deliberately rejected instead of allowing a comma to shift fields.
    let fields: Vec<_> = text.split(',').map(str::trim).collect();
    if !(19..=32).contains(&fields.len()) {
        return Err(Error::Format("EMT row needs 19..=32 fields".into()));
    }
    let integer = |i: usize| {
        fields[i]
            .parse::<i32>()
            .map_err(|_| Error::Format(format!("invalid integer at EMT field {i}")))
    };
    let float = |i: usize| {
        fields[i]
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| Error::Format(format!("invalid finite float at EMT field {i}")))
    };
    let revision = integer(0)?;
    if revision == 2 && fields.len() < 20 {
        return Err(Error::Format(
            "revision-2 EMT row lacks environment field".into(),
        ));
    }
    let filename = asset_name(fields[1])?;
    let xmi_index = integer(17)?;
    let sound = if filename == "none.wav" || filename == "unknown" {
        AudioReference::Silent
    } else if filename.ends_with(".xmi") {
        AudioReference::XmiSequence {
            file: filename,
            sequence: xmi_index,
        }
    } else {
        AudioReference::File(filename)
    };
    let result = EmtEmitter {
        line_number,
        revision,
        sound,
        flags: integer(2)?,
        active_period: match integer(3)? {
            0 => ActivePeriod::Always,
            1 => ActivePeriod::Day,
            2 => ActivePeriod::Night,
            value => ActivePeriod::Unknown(value),
        },
        gain: float(4)?,
        fade_ms: [integer(5)?, integer(6)?],
        loop_mode: match integer(7)? {
            0 => EmtLoopMode::Continuous,
            1 => EmtLoopMode::DelayedRepeat,
            value => EmtLoopMode::Unknown(value),
        },
        position: [float(8)?, float(9)?, float(10)?],
        full_volume_radius: float(11)?,
        max_audible_distance: float(12)?,
        random_location_distance: float(13)?,
        activation_range: float(14)?,
        repeat_delay_ms: [integer(15)?, integer(16)?],
        xmi_index,
        echo_level: float(18)?,
        environment_flag: (fields.len() >= 20).then(|| integer(19)).transpose()?,
        extensions: fields[fields.len().min(20)..]
            .iter()
            .map(|v| (*v).into())
            .collect(),
        raw_fields: fields.iter().map(|v| (*v).into()).collect(),
    };
    if [
        result.gain,
        result.full_volume_radius,
        result.max_audible_distance,
        result.random_location_distance,
        result.activation_range,
    ]
    .iter()
    .any(|v| *v < 0.)
    {
        return Err(Error::Format("negative EMT gain or range".into()));
    }
    Ok(result)
}

fn check_text(text: &str) -> Result<()> {
    if text.len() > MAX_AUDIO_TEXT_BYTES {
        Err(Error::Format("audio metadata exceeds byte limit".into()))
    } else {
        Ok(())
    }
}

fn zone_name(zone: &str) -> Result<String> {
    if zone.is_empty()
        || zone.len() > 128
        || !zone.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(Error::Format("invalid audio zone short name".into()));
    }
    Ok(zone.to_ascii_lowercase())
}

fn asset_name(name: &str) -> Result<String> {
    let name = name.trim();
    // Later authored emitters sometimes include this known local directory.
    // Strip that one prefix only; the remaining name still must be a leaf.
    let name = if name.get(..7).is_some_and(|prefix| {
        prefix.eq_ignore_ascii_case("sounds\\") || prefix.eq_ignore_ascii_case("sounds/")
    }) {
        &name[7..]
    } else {
        name
    };
    if name.is_empty()
        || name.len() > 255
        || matches!(name, "." | "..")
        || name.chars().any(|c| c.is_control() || "/\\:\"".contains(c))
    {
        return Err(Error::Format("invalid local audio filename".into()));
    }
    Ok(name.to_ascii_lowercase())
}

#[cfg(test)]
mod tests;
