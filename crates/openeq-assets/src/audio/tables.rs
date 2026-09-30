use std::collections::BTreeMap;

use super::*;

#[derive(Clone, Debug, Default)]
pub struct SoundBank {
    /// Empty/invalid slots remain present so following IDs never shift.
    pub emit: Vec<Option<String>>,
    pub looped: Vec<Option<String>>,
    pub diagnostics: AudioDiagnostics,
}

impl SoundBank {
    pub fn parse(text: &str) -> Result<Self> {
        check_text(text)?;
        let mut result = Self::default();
        let mut section = None;
        for (line, raw) in text.lines().enumerate() {
            if line >= MAX_TABLE_ROWS {
                return Err(Error::Format("too many sound bank rows".into()));
            }
            let name = raw.trim().trim_start_matches('\u{feff}');
            if name.eq_ignore_ascii_case("EMIT") {
                section = Some(false);
                continue;
            }
            if name.eq_ignore_ascii_case("LOOP") {
                section = Some(true);
                continue;
            }
            if name.starts_with(['#', ';']) {
                continue;
            }
            let Some(looped) = section else {
                if !name.is_empty() {
                    result.diagnostics.push(
                        "sound bank",
                        Some(line + 1),
                        DiagnosticKind::Malformed,
                        "name outside EMIT/LOOP section",
                    );
                }
                continue;
            };
            let sound = if name.is_empty() || name.eq_ignore_ascii_case("unknown") {
                None
            } else {
                let filename = if name.contains('.') {
                    name.into()
                } else {
                    format!("{name}.wav")
                };
                match asset_name(&filename) {
                    Ok(name) => Some(name),
                    Err(error) => {
                        result.diagnostics.push(
                            "sound bank",
                            Some(line + 1),
                            DiagnosticKind::Malformed,
                            error.to_string(),
                        );
                        None
                    }
                }
            };
            if looped {
                result.looped.push(sound);
            } else {
                result.emit.push(sound);
            }
        }
        Ok(result)
    }

    pub fn resolve_effect(&self, id: i32, global: &SoundIdTable) -> AudioReference {
        let (namespace, filename) = match id {
            0 => return AudioReference::Silent,
            1..=31 => (
                SoundNamespace::EmitBank,
                self.emit.get((id - 1) as usize).and_then(Option::as_deref),
            ),
            32..=161 => return global.resolve(id),
            162.. => (
                SoundNamespace::LoopBank,
                self.looped
                    .get((id - 162) as usize)
                    .and_then(Option::as_deref),
            ),
            _ => (SoundNamespace::UnknownEmitter, None),
        };
        filename.map_or(AudioReference::Unresolved { id, namespace }, |name| {
            AudioReference::File(name.into())
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct SoundIdTable {
    names: BTreeMap<i32, Option<String>>,
    pub diagnostics: AudioDiagnostics,
}

impl SoundIdTable {
    pub fn parse(text: &str) -> Result<Self> {
        check_text(text)?;
        let mut result = Self::default();
        for (line, raw) in text.lines().enumerate() {
            if line >= MAX_TABLE_ROWS {
                return Err(Error::Format("too many global sound rows".into()));
            }
            let raw = raw.trim().trim_start_matches('\u{feff}');
            if raw.is_empty() || raw.starts_with(['#', ';']) {
                continue;
            }
            let parsed = (|| -> Result<(i32, Option<String>)> {
                if raw.len() > MAX_LINE_BYTES {
                    return Err(Error::Format("sound table row exceeds byte limit".into()));
                }
                let mut fields = raw.split('^');
                let id = fields
                    .next()
                    .unwrap_or("")
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .filter(|v| *v > 0)
                    .ok_or_else(|| Error::Format("invalid global sound ID".into()))?;
                let filename = fields
                    .next()
                    .ok_or_else(|| Error::Format("missing global sound filename".into()))?
                    .trim();
                let name = if filename.eq_ignore_ascii_case("unknown") || filename.is_empty() {
                    None
                } else {
                    Some(asset_name(filename)?)
                };
                Ok((id, name))
            })();
            match parsed {
                Ok((id, name)) => {
                    if let std::collections::btree_map::Entry::Vacant(slot) = result.names.entry(id)
                    {
                        slot.insert(name);
                    } else {
                        result.diagnostics.push(
                            "soundassets.txt",
                            Some(line + 1),
                            DiagnosticKind::Duplicate,
                            format!("duplicate sound ID {id}; first entry retained"),
                        );
                    }
                }
                Err(error) => result.diagnostics.push(
                    "soundassets.txt",
                    Some(line + 1),
                    DiagnosticKind::Malformed,
                    error.to_string(),
                ),
            }
        }
        Ok(result)
    }

    /// Global IDs used by spells/animations are independent of zone bank IDs.
    pub fn resolve(&self, id: i32) -> AudioReference {
        if id == 0 {
            return AudioReference::Silent;
        }
        self.names.get(&id).and_then(Option::as_ref).map_or(
            AudioReference::Unresolved {
                id,
                namespace: SoundNamespace::Global,
            },
            |name| AudioReference::File(name.clone()),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct Mp3Index {
    /// One-based file positions. Blank or malformed lines retain an empty slot.
    pub names: Vec<Option<String>>,
    pub diagnostics: AudioDiagnostics,
}

impl Mp3Index {
    pub fn parse(text: &str) -> Result<Self> {
        check_text(text)?;
        let mut result = Self::default();
        for (line, name) in text.lines().enumerate() {
            if line >= MAX_TABLE_ROWS {
                return Err(Error::Format("too many MP3 index rows".into()));
            }
            let name = name.trim().trim_start_matches('\u{feff}');
            let filename = if name.is_empty() {
                None
            } else {
                match asset_name(name) {
                    Ok(name) if name.ends_with(".mp3") => Some(name),
                    _ => {
                        result.diagnostics.push(
                            "mp3index.txt",
                            Some(line + 1),
                            DiagnosticKind::Malformed,
                            "invalid MP3 filename; index slot retained",
                        );
                        None
                    }
                }
            };
            result.names.push(filename);
        }
        Ok(result)
    }

    /// Negative IDs use the one-based MP3 table; nonnegative IDs are native
    /// zero-based XMI sequence selections. A parsed sequence is not a decoded audio stream.
    pub fn resolve_music(&self, zone: &str, id: i32) -> AudioReference {
        if id >= 0 {
            return match zone_name(zone) {
                Ok(zone) => AudioReference::XmiSequence {
                    file: format!("{zone}.xmi"),
                    sequence: id,
                },
                Err(_) => AudioReference::Unresolved {
                    id,
                    namespace: SoundNamespace::UnknownEmitter,
                },
            };
        }
        id.checked_neg()
            .and_then(|index| self.names.get((index - 1) as usize))
            .and_then(Option::as_ref)
            .map_or(
                AudioReference::Unresolved {
                    id,
                    namespace: SoundNamespace::Mp3Index,
                },
                |name| AudioReference::File(name.clone()),
            )
    }
}
