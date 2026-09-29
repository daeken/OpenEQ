use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use super::*;
use crate::pfs::{Archive, DIR_CRC, PFS_MAGIC};

const MAX_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;
const MAX_DIRECTORY_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARCHIVES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    Wav,
    Mp3,
    /// Indexed for future synthesis, not supported by waveform decoders.
    Xmi,
}

impl AudioFormat {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.rsplit('.').next()?.to_ascii_lowercase().as_str() {
            "wav" => Some(Self::Wav),
            "mp3" => Some(Self::Mp3),
            "xmi" => Some(Self::Xmi),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioAssetLocation {
    Loose(PathBuf),
    Archive { path: PathBuf, member: String },
}

#[derive(Clone, Debug)]
pub struct AudioAsset {
    pub name: String,
    pub format: AudioFormat,
    pub location: AudioAssetLocation,
}

impl AudioAsset {
    /// Read source bytes only. No codec, synthesis, playback or extracted file.
    /// Archives are reopened lazily, without retaining all banks in memory.
    pub fn read(&self) -> Result<Vec<u8>> {
        match &self.location {
            AudioAssetLocation::Loose(path) => read_bounded(path, MAX_AUDIO_FILE_BYTES),
            AudioAssetLocation::Archive { path, member } => open_archive(path)?.read(member),
        }
    }
}

/// Installed audio index. Construction performs filesystem/archive metadata
/// I/O, so callers should build this off the render thread and reuse it.
#[derive(Debug, Default)]
pub struct AudioCatalog {
    root_files: BTreeMap<String, PathBuf>,
    assets: BTreeMap<String, AudioAsset>,
    pub sound_ids: SoundIdTable,
    pub mp3_index: Mp3Index,
    pub diagnostics: AudioDiagnostics,
    /// Deterministic overlays: later numbered bank, then root, then sounds/.
    /// Native archive precedence is not yet verified; see AUDIO_PLAN.md.
    pub shadowed_assets: usize,
}

impl AudioCatalog {
    pub fn load(directory: &Path) -> Result<Self> {
        let entries = directory_entries(directory)?;
        let mut result = Self::default();
        let mut sound_directory = None;
        for (name, path, is_directory) in entries {
            if is_directory {
                if name.eq_ignore_ascii_case("sounds") && sound_directory.is_none() {
                    sound_directory = Some(path);
                }
                continue;
            }
            let key = name.to_ascii_lowercase();
            if let std::collections::btree_map::Entry::Vacant(entry) = result.root_files.entry(key)
            {
                entry.insert(path);
            } else {
                result.diagnostics.push(
                    &name,
                    None,
                    DiagnosticKind::Duplicate,
                    "case-colliding filename; lexicographically first path retained",
                );
            }
        }
        result.load_tables();
        let mut archives: Vec<_> = result
            .root_files
            .iter()
            .filter_map(|(name, path)| {
                archive_number(name).map(|number| (number, name.clone(), path.clone()))
            })
            .collect();
        archives.sort();
        if archives.len() > MAX_ARCHIVES {
            return Err(Error::Format("too many sound archives".into()));
        }
        // Ascending insertion means snd10 wins over snd2, not lexical order.
        for (_, name, path) in archives {
            match open_archive(&path) {
                Ok(archive) => {
                    for member in archive.names() {
                        let Some(format) = AudioFormat::from_name(member) else {
                            continue;
                        };
                        let Ok(name) = asset_name(member) else {
                            result.diagnostics.push(
                                &path.display().to_string(),
                                None,
                                DiagnosticKind::Malformed,
                                "invalid archived audio filename",
                            );
                            continue;
                        };
                        result.insert(AudioAsset {
                            name,
                            format,
                            location: AudioAssetLocation::Archive {
                                path: path.clone(),
                                member: member.clone(),
                            },
                        })?;
                    }
                }
                Err(error) => result.diagnostics.push(
                    &name,
                    None,
                    DiagnosticKind::Malformed,
                    error.to_string(),
                ),
            }
        }
        for (name, path) in result.root_files.clone() {
            if let Some(format) = AudioFormat::from_name(&name) {
                result.insert(AudioAsset {
                    name,
                    format,
                    location: AudioAssetLocation::Loose(path),
                })?;
            }
        }
        if let Some(directory) = sound_directory {
            let mut seen = std::collections::BTreeSet::new();
            for (name, path, is_directory) in directory_entries(&directory)? {
                if is_directory {
                    continue;
                }
                let Some(format) = AudioFormat::from_name(&name) else {
                    continue;
                };
                let Ok(name) = asset_name(&name) else {
                    continue;
                };
                if seen.insert(name.clone()) {
                    result.insert(AudioAsset {
                        name,
                        format,
                        location: AudioAssetLocation::Loose(path),
                    })?;
                } else {
                    result.diagnostics.push(
                        &name,
                        None,
                        DiagnosticKind::Duplicate,
                        "case-colliding sounds filename; lexicographically first path retained",
                    );
                }
            }
        }
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.assets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.assets.is_empty()
    }

    pub fn asset(&self, name: &str) -> Option<&AudioAsset> {
        self.assets.get(&asset_name(name).ok()?)
    }

    pub fn asset_for(&self, reference: &AudioReference) -> Option<&AudioAsset> {
        self.asset(reference.file_name()?)
    }

    pub fn read(&self, name: &str) -> Result<Option<Vec<u8>>> {
        asset_name(name)?;
        self.asset(name).map(AudioAsset::read).transpose()
    }

    /// Prefer present EMT metadata. Malformed EMT does not fall back to stale
    /// EFF, and merely absent global/bank files do not prevent MP3 resolution.
    pub fn load_zone(&self, zone: &str) -> Result<ZoneAudio> {
        let zone = zone_name(zone)?;
        let emt_name = format!("{zone}.emt");
        let eff_name = format!("{zone}_sounds.eff");
        let mut result = if let Some(text) = self.text(&emt_name)? {
            let mut result = ZoneAudio::parse_emt(&zone, &text)?;
            if self.root_files.contains_key(&eff_name) {
                result.diagnostics.push(
                    &emt_name,
                    None,
                    DiagnosticKind::Precedence,
                    "EMT present; legacy EFF intentionally not combined",
                );
            }
            result
        } else if let Some(path) = self.root_files.get(&eff_name) {
            let bank_name = format!("{zone}_sndbnk.eff");
            let mut diagnostics = AudioDiagnostics::default();
            let bank = match self
                .text(&bank_name)
                .and_then(|text| text.map(|text| SoundBank::parse(&text)).transpose())
            {
                Ok(Some(bank)) => {
                    diagnostics.append(&bank.diagnostics, &bank_name);
                    bank
                }
                Ok(None) => {
                    diagnostics.push(
                        &bank_name,
                        None,
                        DiagnosticKind::Missing,
                        "sound bank absent; bank references remain unresolved",
                    );
                    SoundBank::default()
                }
                Err(error) => {
                    diagnostics.push(
                        &bank_name,
                        None,
                        DiagnosticKind::Malformed,
                        error.to_string(),
                    );
                    SoundBank::default()
                }
            };
            let bytes = read_bounded(path, MAX_ZONE_EMITTERS * EFF_RECORD_BYTES)?;
            let mut result =
                ZoneAudio::parse_eff(&zone, &bytes, &bank, &self.sound_ids, &self.mp3_index)?;
            result.diagnostics.append(&diagnostics, &bank_name);
            result
        } else {
            ZoneAudio {
                zone,
                ..ZoneAudio::default()
            }
        };
        for emitter in &result.emitters {
            let (source, entry, sounds): (&str, usize, &[AudioReference]) = match emitter {
                AudioEmitter::Classic(emitter) => {
                    (&eff_name, emitter.record_index + 1, &emitter.sounds)
                }
                AudioEmitter::Emt(emitter) => (
                    &emt_name,
                    emitter.line_number,
                    std::slice::from_ref(&emitter.sound),
                ),
            };
            for sound in sounds {
                if let Some(name) = sound.file_name()
                    && self.asset(name).is_none()
                {
                    result.diagnostics.push(
                        source,
                        Some(entry),
                        DiagnosticKind::Missing,
                        format!("audio asset absent: {name}"),
                    );
                }
            }
        }
        Ok(result)
    }

    fn insert(&mut self, asset: AudioAsset) -> Result<()> {
        if self.assets.insert(asset.name.clone(), asset).is_some() {
            self.shadowed_assets += 1;
        }
        if self.assets.len() > MAX_TABLE_ROWS {
            return Err(Error::Format("too many indexed audio assets".into()));
        }
        Ok(())
    }

    fn text(&self, name: &str) -> Result<Option<String>> {
        self.root_files
            .get(name)
            .map(|path| {
                let bytes = read_bounded(path, MAX_AUDIO_TEXT_BYTES)?;
                String::from_utf8(bytes)
                    .map_err(|_| Error::Format(format!("invalid UTF-8 audio metadata: {name}")))
            })
            .transpose()
    }

    fn load_tables(&mut self) {
        match self
            .text("soundassets.txt")
            .and_then(|text| text.map(|text| SoundIdTable::parse(&text)).transpose())
        {
            Ok(Some(table)) => {
                self.diagnostics
                    .append(&table.diagnostics, "soundassets.txt");
                self.sound_ids = table;
            }
            Ok(None) => self.diagnostics.push(
                "soundassets.txt",
                None,
                DiagnosticKind::Missing,
                "global sound table absent",
            ),
            Err(error) => self.diagnostics.push(
                "soundassets.txt",
                None,
                DiagnosticKind::Malformed,
                error.to_string(),
            ),
        }
        match self
            .text("mp3index.txt")
            .and_then(|text| text.map(|text| Mp3Index::parse(&text)).transpose())
        {
            Ok(Some(table)) => {
                self.diagnostics.append(&table.diagnostics, "mp3index.txt");
                self.mp3_index = table;
            }
            Ok(None) => self.diagnostics.push(
                "mp3index.txt",
                None,
                DiagnosticKind::Missing,
                "MP3 index absent",
            ),
            Err(error) => self.diagnostics.push(
                "mp3index.txt",
                None,
                DiagnosticKind::Malformed,
                error.to_string(),
            ),
        }
    }
}

fn directory_entries(path: &Path) -> Result<Vec<(String, PathBuf, bool)>> {
    let entries = fs::read_dir(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    let mut result = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_TABLE_ROWS {
            return Err(Error::Format("audio directory exceeds entry limit".into()));
        }
        let entry = entry?;
        let kind = entry.file_type()?;
        if !(kind.is_file() || kind.is_dir()) {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        result.push((name.into(), entry.path(), kind.is_dir()));
    }
    result.sort();
    Ok(result)
}

fn archive_number(name: &str) -> Option<u32> {
    let digits = name.strip_prefix("snd")?.strip_suffix(".pfs")?;
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|n| *n > 0)
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
            "audio file exceeds byte limit: {}",
            path.display()
        )));
    }
    Ok(bytes)
}

fn open_archive(path: &Path) -> Result<Archive> {
    let bytes = read_bounded(path, MAX_ARCHIVE_BYTES)?;
    validate_archive_bounds(&bytes)?;
    Archive::from_bytes(path.into(), bytes)
}

// The generic PFS decoder trusts advertised inflated lengths. Bound them
// before invoking it, including the name directory and every block scratch
// buffer. This remains specific to sound banks and does not change other asset
// readers' accepted sizes or formats.
fn validate_archive_bounds(bytes: &[u8]) -> Result<()> {
    let word = |offset: usize| -> Result<usize> {
        let slice = bytes
            .get(
                offset
                    ..offset
                        .checked_add(4)
                        .ok_or_else(|| Error::Format("audio archive offset overflow".into()))?,
            )
            .ok_or_else(|| Error::Format("truncated audio archive word".into()))?;
        Ok(u32::from_le_bytes(slice.try_into().unwrap()) as usize)
    };
    if word(4)? != PFS_MAGIC as usize {
        return Err(Error::Format("invalid audio archive magic".into()));
    }
    let offset = word(0)?;
    let count = word(offset)?;
    if count == 0 || count > MAX_TABLE_ROWS {
        return Err(Error::Format("invalid audio archive member count".into()));
    }
    let end = offset
        .checked_add(4)
        .and_then(|v| v.checked_add(count * 12))
        .filter(|end| *end <= bytes.len())
        .ok_or_else(|| Error::Format("truncated audio archive table".into()))?;
    let mut directories = 0;
    for entry in (offset + 4..end).step_by(12) {
        let directory = word(entry)? == DIR_CRC as usize;
        directories += usize::from(directory);
        let mut cursor = word(entry + 4)?;
        let total = word(entry + 8)?;
        let limit = if directory {
            MAX_DIRECTORY_BYTES
        } else {
            MAX_AUDIO_FILE_BYTES
        };
        if total > limit || (directory && total < 4) {
            return Err(Error::Format(
                "audio archive member exceeds size bounds".into(),
            ));
        }
        let mut inflated = 0;
        while inflated < total {
            let compressed = word(cursor)?;
            let block_size = word(cursor + 4)?;
            if compressed == 0 || block_size == 0 || block_size > total - inflated {
                return Err(Error::Format("invalid audio archive block size".into()));
            }
            cursor = cursor
                .checked_add(8)
                .and_then(|v| v.checked_add(compressed))
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| Error::Format("truncated audio archive block".into()))?;
            inflated += block_size;
        }
    }
    if directories != 1 {
        return Err(Error::Format(
            "audio archive must have exactly one directory".into(),
        ));
    }
    Ok(())
}
