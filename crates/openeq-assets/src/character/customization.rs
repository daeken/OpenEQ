//! Authored character creation choices from Resources/playercustomization.txt.
//! Colors are decimal 0xRRGGBB values; PARENT_ID identifies Drakkin heritage.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeatureCounts {
    pub faces: u32,
    pub hair_styles: u32,
    pub eyes: u32,
    pub beards: u32,
    pub tattoos: u32,
    pub facial_attachments: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomizationEntry {
    pub race: u32,
    pub parent: u32,
    pub gender: u8,
    pub name: String,
    pub base_color: u32,
    pub classes: Vec<u32>,
    pub colors: Vec<u32>,
    pub features: FeatureCounts,
}

#[derive(Debug, Clone, Default)]
pub struct CustomizationCatalog {
    entries: BTreeMap<(u32, u32, u8), CustomizationEntry>,
}

impl CustomizationCatalog {
    pub fn parse(text: &str) -> Result<Self> {
        let mut catalog = Self::default();
        for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let error = |message: &str| {
                Error::Format(format!(
                    "playercustomization.txt line {}: {message}",
                    index + 1
                ))
            };
            let mut fields: Vec<_> = line.split('^').map(str::trim).collect();
            if fields.last() == Some(&"") {
                fields.pop();
            }
            if fields.len() != 13 {
                return Err(error("expected 13 fields"));
            }
            let number = |field: usize| {
                fields[field]
                    .parse::<u32>()
                    .map_err(|_| error("invalid number"))
            };
            let list = |field: usize| -> Result<Vec<u32>> {
                if fields[field].is_empty() {
                    return Ok(Vec::new());
                }
                fields[field]
                    .split(',')
                    .map(|value| {
                        value
                            .trim()
                            .parse::<u32>()
                            .map_err(|_| error("invalid number list"))
                    })
                    .collect()
            };
            let count = |field| -> Result<u32> {
                let value = number(field)?;
                if value > 256 {
                    return Err(error("feature count exceeds byte selectors"));
                }
                Ok(value)
            };
            let gender = number(12)?;
            if gender > 2 {
                return Err(error("invalid gender"));
            }
            let entry = CustomizationEntry {
                race: number(0)?,
                parent: number(1)?,
                gender: gender as u8,
                name: fields[2].into(),
                base_color: number(3)?,
                classes: list(4)?,
                colors: list(5)?,
                features: FeatureCounts {
                    faces: count(6)?,
                    hair_styles: count(7)?,
                    eyes: count(8)?,
                    beards: count(9)?,
                    tattoos: count(10)?,
                    facial_attachments: count(11)?,
                },
            };
            if entry.base_color > 0x00ff_ffff
                || entry.colors.iter().any(|color| *color > 0x00ff_ffff)
            {
                return Err(error("color exceeds RGB range"));
            }
            if entry.colors.len() > 256 {
                return Err(error("palette exceeds byte selectors"));
            }
            let key = (entry.race, entry.parent, entry.gender);
            if catalog.entries.insert(key, entry).is_some() {
                return Err(error("duplicate race, parent, and gender"));
            }
        }
        Ok(catalog)
    }

    /// Missing client metadata is supported: geometry still renders with its
    /// source textures, without inventing a replacement color palette.
    pub fn load(base: impl AsRef<Path>) -> Result<Option<Self>> {
        fn child(directory: &Path, name: &str) -> Result<Option<PathBuf>> {
            let direct = directory.join(name);
            if direct.exists() {
                return Ok(Some(direct));
            }
            for entry in std::fs::read_dir(directory).map_err(|source| Error::Io {
                path: directory.to_owned(),
                source,
            })? {
                let entry = entry?;
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(name)
                {
                    return Ok(Some(entry.path()));
                }
            }
            Ok(None)
        }
        let Some(resources) = child(base.as_ref(), "Resources")? else {
            return Ok(None);
        };
        let Some(path) = child(&resources, "playercustomization.txt")? else {
            return Ok(None);
        };
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Io { path, source })?;
        Self::parse(&text).map(Some)
    }

    pub fn get(&self, race: u32, parent: u32, gender: u8) -> Option<&CustomizationEntry> {
        self.entries.get(&(race, parent, gender))
    }

    pub fn entries(&self) -> impl Iterator<Item = &CustomizationEntry> {
        self.entries.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customization_preserves_authored_colors_and_gender_specific_counts() {
        let catalog = CustomizationCatalog::parse("\u{feff}# header\r\n522^2^Blue^15580^1,2^20,1315890,3947620,7237270^7^9^12^12^8^8^0^\r\n522^2^Blue^15580^1,2^20,1315890,3947620,7237270^7^8^12^4^8^8^1^\n1^0^^0^^^0^0^0^0^0^0^0^").unwrap();
        let male = catalog.get(522, 2, 0).unwrap();
        let female = catalog.get(522, 2, 1).unwrap();
        assert_eq!(male.base_color, 0x003cdc);
        assert_eq!(male.colors, [0x000014, 0x141432, 0x3c3c64, 0x6e6e96]);
        assert_eq!(
            (male.features.hair_styles, female.features.hair_styles),
            (9, 8)
        );
        assert_eq!((male.features.beards, female.features.beards), (12, 4));
        assert!(catalog.get(1, 0, 0).unwrap().colors.is_empty());
        assert!(catalog.get(522, 6, 0).is_none());
    }

    #[test]
    fn customization_rejects_bad_rows_instead_of_accepting_shifted_palettes() {
        for text in [
            "522^0^broken^0",
            "522^0^bad color^16777216^^^7^9^12^12^8^8^0^",
            "522^0^bad list^0^^20,nope^7^9^12^12^8^8^0^",
            "522^0^bad gender^0^^^7^9^12^12^8^8^255^",
            "522^0^bad count^0^^^7^257^12^12^8^8^0^",
            "522^0^one^0^^^7^9^12^12^8^8^0^\n522^0^two^0^^^7^9^12^12^8^8^0^",
        ] {
            assert!(
                CustomizationCatalog::parse(text).is_err(),
                "accepted {text}"
            );
        }
    }
}
