use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Levels {
    pub master: f32,
    pub music: f32,
    pub effects: f32,
    pub ambience: f32,
    pub muted: bool,
    pub environment_enabled: bool,
}
impl Default for Levels {
    fn default() -> Self {
        Self {
            master: 0.5,
            music: 0.6,
            effects: 0.7,
            ambience: 0.6,
            muted: false,
            environment_enabled: true,
        }
    }
}
impl Levels {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            [self.master, self.music, self.effects, self.ambience]
                .iter()
                .all(|v| v.is_finite() && (0. ..=1.).contains(v)),
            "audio volume must be between 0 and 1"
        );
        Ok(())
    }
    pub fn gain(&self, music: bool) -> f32 {
        if self.muted {
            0.
        } else {
            self.master * if music { self.music } else { self.ambience }
        }
    }
    pub fn summary(&self) -> String {
        format!(
            "Audio: master {}%, music {}%, ambience {}%, effects {}%; {}; environment {}.",
            (self.master * 100.).round(),
            (self.music * 100.).round(),
            (self.ambience * 100.).round(),
            (self.effects * 100.).round(),
            if self.muted { "muted" } else { "on" },
            if self.environment_enabled {
                "on"
            } else {
                "off"
            }
        )
    }
    /// Returns None for unrelated chat, including `/audiofoo`.
    pub fn command(&mut self, text: &str) -> Option<String> {
        let mut words = text.split_whitespace();
        if !words.next()?.eq_ignore_ascii_case("/audio") {
            return None;
        }
        let args: Vec<_> = words.map(str::to_ascii_lowercase).collect();
        match args.as_slice() {
            [] => return Some(self.summary()),
            [toggle] if toggle == "mute" => self.muted = true,
            [toggle] if toggle == "unmute" => self.muted = false,
            [option, value]
                if option == "environment" && matches!(value.as_str(), "on" | "off") =>
            {
                self.environment_enabled = value == "on"
            }
            [channel, value] => {
                let Some(value) = value
                    .parse::<f32>()
                    .ok()
                    .filter(|v| v.is_finite() && (0. ..=100.).contains(v))
                else {
                    return Some("Volume must be a number from 0 to 100.".into());
                };
                let target = match channel.as_str() {
                    "master" => &mut self.master,
                    "music" => &mut self.music,
                    "ambience" => &mut self.ambience,
                    "effects" => &mut self.effects,
                    _ => return Some(help()),
                };
                *target = value / 100.;
            }
            _ => return Some(help()),
        }
        Some(self.summary())
    }
}
fn help() -> String {
    "Audio: /audio [mute|unmute], /audio master|music|ambience|effects 0–100, /audio environment on|off.".into()
}
#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    levels: Levels,
}
pub fn settings_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|v| PathBuf::from(v).join(".config")))
        .map(|v| v.join("openeq/audio.json"))
}
pub fn load(path: &Path) -> Result<Levels> {
    if let Ok(metadata) = std::fs::metadata(path) {
        ensure!(metadata.len() <= 4096, "audio settings exceed size limit");
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Levels::default()),
        Err(e) => return Err(e.into()),
    };
    ensure!(bytes.len() <= 4096, "audio settings exceed size limit");
    let doc: Document = serde_json::from_slice(&bytes)?;
    ensure!(doc.version == 1, "unsupported audio settings version");
    doc.levels.validate()?;
    Ok(doc.levels)
}
pub fn save(path: &Path, levels: Levels) -> Result<()> {
    levels.validate()?;
    let parent = path.parent().context("audio settings parent")?;
    std::fs::create_dir_all(parent)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let temp = parent.join(format!(".audio-{}-{stamp}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut options = std::fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(&serde_json::to_vec_pretty(&Document {
            version: 1,
            levels,
        })?)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn volume_commands_validate_without_accidental_unmute() {
        let mut l = Levels::default();
        l.command("/audio mute");
        l.command("/audio music 25");
        assert_eq!(l.music, 0.25);
        assert_eq!(l.gain(true), 0.);
        assert!(l.command("/audiofoo 5").is_none());
        for value in ["NaN", "inf", "-1", "101"] {
            l.command(&format!("/audio master {value}"));
            assert_eq!(l.master, 0.5);
        }
        l.command("/audio unmute");
        assert_eq!(l.gain(true), 0.125);
    }
}
