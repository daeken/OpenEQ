//! Static day-key sampling; weather changes and live clock state are separate.
use super::{Ini, original_color_map, path_case_insensitive, read_texture_path, value};
use crate::{Error, Result, texture::Texture};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkyColorMapSource {
    /// Original ColorMapN ordinal, retained even when keys sort differently.
    pub key_index: usize,
    pub color_map: String,
    pub path: PathBuf,
    pub start_tick: u32,
    pub transition_ticks: u32,
}

/// Actual inputs to the sampled table, not a weather-manager transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkyColorMapInputs {
    Single(SkyColorMapSource),
    Blend {
        previous: SkyColorMapSource,
        current: SkyColorMapSource,
        /// Native byte weight: `(current*w + previous*(255-w)) >> 8`.
        current_weight: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkyColorMapProvenance {
    pub color_set: String,
    /// Truncated normalized day fraction multiplied by 65536.
    pub day_tick: u32,
    pub inputs: SkyColorMapInputs,
}

#[derive(Debug)]
struct Key {
    index: usize,
    map: String,
    filename: String,
    time: u32,
    transition: u32,
}

fn keys(ini: &Ini, color_set: &str) -> Result<Vec<Key>> {
    let section = format!("ColorSet-{color_set}");
    let entries = ini
        .get(&section.to_ascii_lowercase())
        .ok_or_else(|| Error::Format(format!("missing sky color set: {color_set}")))?;
    let mut indices = entries
        .keys()
        .filter_map(|key| key.strip_prefix("colormap")?.parse::<usize>().ok())
        .collect::<Vec<_>>();
    indices.sort_unstable();
    if indices.is_empty() {
        return Err(Error::Format(format!(
            "sky color set has no maps: {color_set}"
        )));
    }
    let mut keys = Vec::with_capacity(indices.len());
    for (expected, index) in indices.into_iter().enumerate() {
        if index != expected {
            return Err(Error::Format(format!(
                "sky color set has a key gap: {color_set}"
            )));
        }
        let map = value(ini, &section, &format!("ColorMap{index}"))
            .ok_or_else(|| Error::Format(format!("empty sky color map: {color_set}[{index}]")))?;
        let field = |prefix: &str| -> Result<f32> {
            // Native INI calls default both numeric fields to zero.
            let raw = value(ini, &section, &format!("{prefix}{index}")).unwrap_or("0");
            raw.parse::<f32>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0. && *v < 1.)
                .ok_or_else(|| {
                    Error::Format(format!(
                        "unsupported sky {prefix}{index}={raw}: {color_set}"
                    ))
                })
        };
        let time = (field("Time")? * 65536.) as u32;
        let transition = (field("Transition")? * 65536.) as u32;
        let file = value(ini, &format!("ColorMap-{map}"), "File")
            .ok_or_else(|| Error::Format(format!("sky color map has no file: {map}")))?;
        keys.push(Key {
            index,
            map: map.to_owned(),
            filename: format!("colormap-{file}.dds"),
            time,
            transition,
        });
    }
    keys.sort_by_key(|key| key.time);
    for (index, key) in keys.iter().enumerate() {
        let next = keys.get(index + 1).map_or(65536, |key| key.time);
        if key.time == next || key.time + key.transition > next {
            return Err(Error::Format(format!(
                "unsupported duplicate or overlapping sky keys: {color_set}"
            )));
        }
    }
    Ok(keys)
}

fn normalized_tick(day_fraction: f32) -> u32 {
    // Keep load_sky's pre-existing public input normalization. Native callers
    // supply a day fraction; this does not emulate a live client clock.
    let fraction = if day_fraction.is_finite() {
        day_fraction.rem_euclid(1.)
    } else {
        0.5
    };
    // Preserve f32 rem_euclid rounding, including tiny negatives rounding to 1.
    (fraction * 65536.) as u32
}

fn read_key(directory: &Path, key: &Key) -> Result<(Texture, SkyColorMapSource)> {
    let path = path_case_insensitive(directory, &key.filename)?;
    let texture = original_color_map(read_texture_path(&path, &key.filename)?)?;
    Ok((
        texture,
        SkyColorMapSource {
            key_index: key.index,
            color_map: key.map.clone(),
            path,
            start_tick: key.time,
            transition_ticks: key.transition,
        },
    ))
}

pub(super) fn sample(
    directory: &Path,
    ini: &Ini,
    color_set: &str,
    day_fraction: f32,
) -> Result<(Texture, SkyColorMapProvenance)> {
    let keys = keys(ini, color_set)?;
    let day_tick = normalized_tick(day_fraction);
    let index = keys
        .iter()
        // Native equality still selects the preceding key, even when the new
        // key has a zero-length transition. The new key begins one tick later.
        .rposition(|key| key.time < day_tick)
        .unwrap_or(keys.len() - 1);
    let current = &keys[index];
    // The native unsigned subtraction selects the final map before the first
    // key; it does not invent an extra midnight transition.
    let elapsed = day_tick.wrapping_sub(current.time);
    let (texture, inputs) = if keys.len() == 1 || elapsed >= current.transition {
        let (texture, source) = read_key(directory, current)?;
        (texture, SkyColorMapInputs::Single(source))
    } else {
        let previous = &keys[(index + keys.len() - 1) % keys.len()];
        let weight = (elapsed * 255 / current.transition) as u8;
        let (texture, previous_source) = read_key(directory, previous)?;
        if weight == 0 {
            (texture, SkyColorMapInputs::Single(previous_source))
        } else {
            let (next, current_source) = read_key(directory, current)?;
            let rgba = texture
                .rgba
                .iter()
                .zip(&next.rgba)
                .map(|(old, new)| {
                    ((u32::from(*new) * u32::from(weight)
                        + u32::from(*old) * (255 - u32::from(weight)))
                        >> 8) as u8
                })
                .collect();
            // This is a newly derived table. Actual file identities remain in
            // the provenance, and neither decoded source is mutated or cached.
            let texture = Texture {
                name: format!("ColorSet-{color_set} at tick {day_tick}"),
                width: 32,
                height: 32,
                rgba,
            };
            (
                texture,
                SkyColorMapInputs::Blend {
                    previous: previous_source,
                    current: current_source,
                    current_weight: weight,
                },
            )
        }
    };
    Ok((
        texture,
        SkyColorMapProvenance {
            color_set: color_set.to_owned(),
            day_tick,
            inputs,
        },
    ))
}

/// Raw original table words in AARRGGBB order, including alpha. These are
/// inputs to the native host lighting logic, not final rendered light values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkyLightColors {
    pub ambient: u32,
    pub sun_directional: u32,
    pub moon_directional: u32,
    pub sun_bounce: u32,
    pub moon_bounce: u32,
}

impl super::SkyAssets {
    /// Reads reserved color-map swatches without applying them to lighting.
    pub fn raw_light_colors(&self) -> Option<SkyLightColors> {
        let texture = &self.color_map;
        if self.color_map_layout != super::SkyColorMapLayout::OriginalDome
            || texture.width != 32
            || texture.height != 32
            || texture.rgba.len() != 4096
        {
            return None;
        }
        let word = |row: usize| {
            let pixel = &texture.rgba[(row * 32 + 31) * 4..][..4];
            u32::from_le_bytes([pixel[2], pixel[1], pixel[0], pixel[3]])
        };
        Some(SkyLightColors {
            ambient: word(3),
            sun_directional: word(0),
            moon_directional: word(1),
            sun_bounce: word(28),
            moon_bounce: word(29),
        })
    }
}
