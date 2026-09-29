//! Zone sky definitions from the original client's Resources/sky INI files.
//!
//! These are sky domes, not cubemaps: weather patterns select a color table and
//! cloud sprites. The native 32x32 table includes non-dome entries; its layout
//! must stay distinct from a generic texture. The loader resolves the authored
//! chain and samples its day cycle at a supplied fraction of an EverQuest day.
use crate::{Error, Result, texture::Texture};
use std::{collections::HashMap, path::Path};

/// Identifies usable sky colors without discarding the original source pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SkyColorMapLayout {
    /// Generic or synthetic lookup texture; every source texel is usable.
    #[default]
    FullTexture,
    /// Native EQ 32x32 vertex-color table. The dome uses columns 0..=30 and
    /// rows 0..=29, with the two poles reading column zero of rows 0 and 29.
    /// Column 31 and rows 30/31 contain other colors and must not be sampled
    /// as sky. This describes the data domain, not its celestial orientation.
    OriginalDome,
}

impl SkyColorMapLayout {
    pub fn usable_size(self, texture: &Texture) -> [u32; 2] {
        match self {
            Self::FullTexture => [texture.width, texture.height],
            Self::OriginalDome => [31, 30],
        }
    }

    /// Source texels for the positive and negative native dome-axis poles.
    /// They are not necessarily the world zenith and nadir.
    pub fn pole_texels(self) -> Option<[[u32; 2]; 2]> {
        match self {
            Self::FullTexture => None,
            Self::OriginalDome => Some([[0, 0], [0, 29]]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SkyAssets {
    pub weather: String,
    pub color_map: Texture,
    pub color_map_layout: SkyColorMapLayout,
    pub cloud_texture: Option<Texture>,
    pub cloud_color_map: Option<Texture>,
    pub cloud_color_map_layout: SkyColorMapLayout,
    /// Texture-space movement per second (the INI expresses it per millisecond).
    pub cloud_velocity: f32,
}

type Ini = HashMap<String, HashMap<String, String>>;
fn parse_ini(text: &str) -> Ini {
    let mut sections = Ini::new();
    let mut section = String::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
            section = name.to_ascii_lowercase();
        } else if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(section.clone())
                .or_default()
                .insert(key.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    sections
}
fn value<'a>(ini: &'a Ini, section: &str, key: &str) -> Option<&'a str> {
    ini.get(&section.to_ascii_lowercase())?
        .get(&key.to_ascii_lowercase())
        .map(String::as_str)
        .filter(|s| !s.is_empty())
}
fn path_case_insensitive(directory: &Path, file: &str) -> Result<std::path::PathBuf> {
    if Path::new(file).components().count() != 1 {
        return Err(Error::Format(format!("invalid sky filename: {file}")));
    }
    let direct = directory.join(file);
    if direct.is_file() {
        return Ok(direct);
    }
    std::fs::read_dir(directory)?
        .filter_map(std::result::Result::ok)
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(file)
        })
        .map(|entry| entry.path())
        .ok_or_else(|| Error::Format(format!("missing sky asset: {file}")))
}
fn read_texture(directory: &Path, file: &str) -> Result<Texture> {
    let path = path_case_insensitive(directory, file)?;
    let mut bytes = std::fs::read(path)?;
    // Cloud sprites use the D3D L8A8 format. Treat its luminance mask as all
    // three RGB masks so the general packed-DDS decoder preserves both planes.
    if bytes.len() >= 128 && bytes.starts_with(b"DDS ") {
        let flags = u32::from_le_bytes(bytes[80..84].try_into().unwrap());
        if flags & 0x20000 != 0 {
            bytes[80..84].copy_from_slice(&((flags & !0x20000) | 0x40).to_le_bytes());
            let luminance = bytes[92..96].to_vec();
            bytes[96..100].copy_from_slice(&luminance);
            bytes[100..104].copy_from_slice(&luminance);
        }
    }
    Texture::decode(file, &bytes)
}

fn original_color_map(texture: Texture) -> Result<Texture> {
    if texture.width != 32 || texture.height != 32 {
        return Err(Error::Format(format!(
            "original sky color map must be 32x32: {} ({}x{})",
            texture.name, texture.width, texture.height
        )));
    }
    Ok(texture)
}
fn color_map_file(ini: &Ini, color_set: &str, day_fraction: f32) -> Result<String> {
    let section = format!("ColorSet-{color_set}");
    let mut frames: Vec<(f32, &str)> = (0..32)
        .filter_map(|index| {
            let map = value(ini, &section, &format!("ColorMap{index}"))?;
            let time = value(ini, &section, &format!("Time{index}"))?
                .parse::<f32>()
                .ok()?;
            time.is_finite().then_some((time, map))
        })
        .collect();
    frames.sort_by(|a, b| a.0.total_cmp(&b.0));
    let time = if day_fraction.is_finite() {
        day_fraction.rem_euclid(1.0)
    } else {
        0.5
    };
    let map = frames
        .iter()
        .rev()
        .find(|(start, _)| *start <= time)
        .or_else(|| frames.last())
        .map(|(_, map)| *map)
        .or_else(|| value(ini, &section, "ColorMap0"))
        .ok_or_else(|| Error::Format(format!("sky color set has no maps: {color_set}")))?;
    let file = value(ini, &format!("ColorMap-{map}"), "File")
        .ok_or_else(|| Error::Format(format!("sky color map has no file: {map}")))?;
    Ok(format!("colormap-{file}.dds"))
}

/// Resolves a zone's clear-weather sky. Missing zone overrides inherit the
/// client's default definition. `day_fraction=0.5` selects the midday assets.
/// Color-map transitions and multi-layer cloud population are not simulated yet.
pub fn load_sky(client_directory: &Path, zone: &str, day_fraction: f32) -> Result<SkyAssets> {
    let directory = client_directory.join("Resources/sky");
    let settings = parse_ini(&std::fs::read_to_string(directory.join("sky.ini"))?);
    let weather = parse_ini(&std::fs::read_to_string(directory.join("weather.ini"))?);
    let pattern = value(&settings, &format!("SkySetting-{zone}"), "DefaultWeather")
        .or_else(|| value(&settings, "SkySetting-default", "DefaultWeather"))
        .unwrap_or("DefaultClear");
    let section = format!("WeatherPattern-{pattern}");
    let color_set = value(&weather, &section, "ColorSet")
        .ok_or_else(|| Error::Format(format!("missing sky weather pattern: {pattern}")))?;
    let color_map = original_color_map(read_texture(
        &directory,
        &color_map_file(&weather, color_set, day_fraction)?,
    )?)?;
    let mut cloud_texture = None;
    let mut cloud_color_map = None;
    let mut cloud_color_map_layout = SkyColorMapLayout::FullTexture;
    let mut cloud_velocity = 0.001;
    if let Some(cloud) = value(&weather, &section, "Cloud0") {
        let cloud = format!("Cloud-{cloud}");
        if let Some(texture) = value(&weather, &cloud, "Texture") {
            cloud_texture = Some(read_texture(&directory, &format!("cloud-{texture}.dds"))?);
        }
        if let Some(set) = value(&weather, &cloud, "ColorSet") {
            cloud_color_map = Some(original_color_map(read_texture(
                &directory,
                &color_map_file(&weather, set, day_fraction)?,
            )?)?);
            cloud_color_map_layout = SkyColorMapLayout::OriginalDome;
        }
        cloud_velocity = value(&weather, &cloud, "VelocityMin")
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .map_or(0.001, |v| v * 1000.0);
    }
    Ok(SkyAssets {
        weather: pattern.to_owned(),
        color_map,
        color_map_layout: SkyColorMapLayout::OriginalDome,
        cloud_texture,
        cloud_color_map,
        cloud_color_map_layout,
        cloud_velocity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_color_table_domain_excludes_helper_entries_and_preserves_source() {
        let mut texture = Texture {
            name: "synthetic native color table".into(),
            width: 32,
            height: 32,
            rgba: vec![0; 32 * 32 * 4],
        };
        for (index, pixel) in texture.rgba.chunks_exact_mut(4).enumerate() {
            pixel.copy_from_slice(&[(index % 32) as u8, (index / 32) as u8, 123, 255]);
        }
        let original = texture.rgba.clone();
        let texture = original_color_map(texture).unwrap();
        assert_eq!(texture.rgba, original);
        let layout = SkyColorMapLayout::OriginalDome;
        assert_eq!(layout.usable_size(&texture), [31, 30]);
        for [x, y] in layout.pole_texels().unwrap() {
            let offset = ((y * texture.width + x) * 4) as usize;
            assert_eq!(&texture.rgba[offset..offset + 4], &[0, y as u8, 123, 255]);
        }
        let generic = Texture {
            width: 2,
            height: 3,
            ..texture
        };
        assert_eq!(SkyColorMapLayout::FullTexture.usable_size(&generic), [2, 3]);
        assert_eq!(SkyColorMapLayout::FullTexture.pole_texels(), None);
        assert!(original_color_map(generic).is_err());
    }

    #[test]
    fn day_cycle_wraps_to_previous_night_and_ini_is_case_insensitive() {
        let ini = parse_ini(
            "[ColorSet-Clear]\nColorMap0=Dawn\nTime0=0.2\nColorMap1=Day\nTime1=0.3\nColorMap2=Night\nTime2=0.8\n[ColorMap-Dawn]\nFile=red\n[ColorMap-Day]\nFile=blue\n[ColorMap-Night]\nFile=black\n",
        );
        assert_eq!(
            color_map_file(&ini, "CLEAR", 0.1).unwrap(),
            "colormap-black.dds"
        );
        assert_eq!(
            color_map_file(&ini, "clear", 0.5).unwrap(),
            "colormap-blue.dds"
        );
    }
    #[test]
    fn installed_client_skies_resolve_authored_textures() {
        let directory = std::env::var_os("EQ_CLIENT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(&std::env::var("HOME").unwrap_or_default()).join("EverQuest")
            });
        if !directory.join("Resources/sky/sky.ini").exists() {
            return;
        }
        let anguish = load_sky(&directory, "anguish", 0.5).unwrap();
        assert_eq!(anguish.weather, "OmensClear");
        assert!(
            anguish
                .color_map
                .name
                .to_ascii_lowercase()
                .contains("yelloworangeday")
        );
        let cloud = anguish.cloud_texture.unwrap();
        assert!(cloud.rgba.chunks_exact(4).any(|p| p[3] > 0 && p[3] < 255));
        assert!(
            cloud
                .rgba
                .chunks_exact(4)
                .all(|p| p[0] == p[1] && p[1] == p[2])
        );
        let pok = load_sky(&directory, "poknowledge", 0.5).unwrap();
        assert_eq!(pok.weather, "DefaultClear");
        assert_eq!(pok.color_map_layout, SkyColorMapLayout::OriginalDome);
        assert_eq!(pok.cloud_color_map_layout, SkyColorMapLayout::OriginalDome);
        assert!(
            pok.color_map
                .name
                .to_ascii_lowercase()
                .contains("defaultday")
        );
        let pixel = |x: usize, y: usize| &pok.color_map.rgba[(y * 32 + x) * 4..][..4];
        // This neon green exists in the original source. A full-width sky
        // lookup paints it across the sky; the dome domain excludes it.
        assert_eq!(pixel(31, 23), &[0, 255, 30, 255]);
        assert_eq!(pixel(0, 0), &[206, 209, 233, 0]);
        assert_eq!(pixel(0, 29), &[204, 208, 233, 0]);
        assert_eq!(pok.color_map_layout.usable_size(&pok.color_map), [31, 30]);
    }
}

/// Server-independent atmosphere data for the offline zone viewer. RGB values
/// are normalized sRGB, and clip distances are in the original EQ world units.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneEnvironment {
    pub zone_id: u16,
    pub version: u8,
    pub fog_color: [[f32; 3]; 4],
    pub fog_start: [f32; 4],
    pub fog_end: [f32; 4],
    pub fog_density: f32,
    pub min_clip: f32,
    pub max_clip: f32,
    pub sky: u8,
    pub zone_type: u8,
    pub time_type: u8,
    pub cast_outdoor: bool,
    pub timezone: i32,
}
const ZONE_ENVIRONMENTS: &str = include_str!("../data/peq-zone-environment.txt");

/// PEQ 2026-09-26 snapshot, schema 9328. The primary zone version is returned;
/// online callers must replace this fallback with their server's NewZone data.
/// The full provenance and column mapping are in data/README.md.
pub fn load_zone_environment(name: &str) -> Option<ZoneEnvironment> {
    load_zone_environment_version(name, 0)
}

pub fn load_zone_environment_version(name: &str, version: u8) -> Option<ZoneEnvironment> {
    for line in ZONE_ENVIRONMENTS
        .lines()
        .filter(|line| !line.starts_with('#'))
    {
        let mut fields = line.split_whitespace();
        if !fields.next()?.eq_ignore_ascii_case(name) {
            continue;
        }
        let values = fields.collect::<Vec<_>>();
        if values.len() != 30 || values[0].parse::<u8>().ok()? != version {
            continue;
        }
        let mut fog_color = [[0.0; 3]; 4];
        let mut fog_start = [0.0; 4];
        let mut fog_end = [0.0; 4];
        for index in 0..4 {
            let offset = 2 + index * 5;
            for channel in 0..3 {
                fog_color[index][channel] =
                    values[offset + channel].parse::<u8>().ok()? as f32 / 255.0;
            }
            fog_start[index] = values[offset + 3].parse().ok()?;
            fog_end[index] = values[offset + 4].parse().ok()?;
        }
        return Some(ZoneEnvironment {
            zone_id: values[1].parse().ok()?,
            version,
            fog_color,
            fog_start,
            fog_end,
            fog_density: values[22].parse().ok()?,
            min_clip: values[23].parse().ok()?,
            max_clip: values[24].parse().ok()?,
            sky: values[25].parse().ok()?,
            zone_type: values[26].parse().ok()?,
            time_type: values[27].parse().ok()?,
            cast_outdoor: values[28] != "0",
            timezone: values[29].parse().ok()?,
        });
    }
    None
}

#[cfg(test)]
mod atmosphere_tests {
    use super::*;
    #[test]
    fn snapshot_matches_live_pok_and_anguish_new_zone_fields() {
        let anguish = load_zone_environment("ANGUISH").unwrap();
        assert_eq!(anguish.zone_id, 317);
        assert_eq!(
            anguish.fog_color[0],
            [100.0 / 255.0, 10.0 / 255.0, 10.0 / 255.0]
        );
        assert_eq!((anguish.fog_start[0], anguish.fog_end[0]), (300.0, 1800.0));
        let pok = load_zone_environment("poknowledge").unwrap();
        assert_eq!(pok.zone_id, 202);
        assert_eq!(
            pok.fog_color[0],
            [50.0 / 255.0, 50.0 / 255.0, 155.0 / 255.0]
        );
        assert_eq!((pok.fog_start[0], pok.fog_end[0]), (400.0, 2000.0));
        assert!(load_zone_environment("not_a_zone").is_none());
    }
    #[test]
    fn every_snapshot_row_can_be_loaded() {
        let mut count = 0;
        for line in ZONE_ENVIRONMENTS
            .lines()
            .filter(|line| !line.starts_with('#'))
        {
            let mut fields = line.split_whitespace();
            let name = fields.next().unwrap();
            let version = fields.next().unwrap().parse().unwrap();
            assert!(
                load_zone_environment_version(name, version).is_some(),
                "invalid snapshot row: {name}"
            );
            count += 1;
        }
        assert_eq!(count, 620);
    }
}
