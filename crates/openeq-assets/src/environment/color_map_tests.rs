//! Native full-table oracle and static loader boundary tests.
use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "openeq-sky-sampling-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("Resources/sky")).unwrap();
        Self(path)
    }
    fn directory(&self) -> PathBuf {
        self.0.join("Resources/sky")
    }
    fn texture(&self, file: &str, rgba: [u8; 4]) {
        let mut dds = vec![0u8; 128];
        dds[..4].copy_from_slice(b"DDS ");
        for (offset, value) in [
            (4, 124u32),
            (8, 0x100f),
            (12, 32),
            (16, 32),
            (20, 128),
            (76, 32),
            (80, 0x41),
            (88, 32),
            (92, 0xff0000),
            (96, 0xff00),
            (100, 0xff),
            (104, 0xff000000),
            (108, 0x1000),
        ] {
            dds[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for _ in 0..1024 {
            dds.extend([rgba[2], rgba[1], rgba[0], rgba[3]]);
        }
        std::fs::write(self.directory().join(file), dds).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const TWO_KEYS: &str = "[ColorSet-Clear]\nColorMap0=Old\nTime0=0\nTransition0=0\nColorMap1=New\nTime1=0.25\nTransition1=0.25\n[ColorMap-Old]\nFile=Old\n[ColorMap-New]\nFile=New\n";
const OLD: [u8; 4] = [201, 111, 9, 17];
const NEW: [u8; 4] = [101, 41, 249, 241];

#[test]
fn exact_zero_transition_keys_keep_the_preceding_map_for_one_tick() {
    let fixture = Fixture::new();
    fixture.texture("colormap-Old.dds", OLD);
    fixture.texture("colormap-New.dds", NEW);
    let ini = parse_ini(&TWO_KEYS.replace("Transition1=0.25", "Transition1=0"));
    // Executed native 1002dfe0/1002eae0 results, including midnight equality.
    for (tick, expected) in [(0, NEW), (1, OLD), (16383, OLD), (16384, OLD), (16385, NEW)] {
        let (texture, _) =
            color_map::sample(&fixture.directory(), &ini, "clear", tick as f32 / 65536.).unwrap();
        assert_eq!(&texture.rgba[..4], &expected, "tick {tick}");
    }
}

#[test]
fn native_key_endpoints_rounding_and_all_channels_match_executed_witness() {
    let fixture = Fixture::new();
    fixture.texture("colormap-Old.dds", OLD);
    fixture.texture("colormap-New.dds", NEW);
    let ini = parse_ini(TWO_KEYS);
    // Expected byte weights are native x86 results at exact 16.16 day ticks.
    for (tick, weight) in [
        (16383, None),
        (16384, Some(0)),
        (16448, Some(0)),
        (16449, Some(1)),
        (24576, Some(127)),
        (32767, Some(254)),
        (32768, None),
    ] {
        let (texture, source) =
            color_map::sample(&fixture.directory(), &ini, "CLEAR", tick as f32 / 65536.).unwrap();
        let expected = if tick >= 32768 {
            NEW
        } else if let Some(w) = weight.filter(|w| *w != 0) {
            std::array::from_fn(|i| {
                ((u32::from(NEW[i]) * w + u32::from(OLD[i]) * (255 - w)) >> 8) as u8
            })
        } else {
            OLD
        };
        assert!(
            texture.rgba.chunks_exact(4).all(|pixel| pixel == expected),
            "tick {tick}"
        );
        assert_eq!(source.day_tick, tick);
        assert_eq!(source.color_set, "CLEAR");
        match source.inputs {
            SkyColorMapInputs::Single(source) => {
                assert_eq!(source.key_index, usize::from(tick >= 32768));
                assert!(source.path.is_file());
                assert_eq!(
                    texture.name,
                    if tick >= 32768 {
                        "colormap-New.dds"
                    } else {
                        "colormap-Old.dds"
                    }
                );
            }
            SkyColorMapInputs::Blend {
                previous,
                current,
                current_weight,
            } => {
                assert_eq!(previous.key_index, 0);
                assert_eq!(current.key_index, 1);
                assert_eq!(
                    (previous.color_map.as_str(), current.color_map.as_str()),
                    ("Old", "New")
                );
                assert_eq!(
                    (current.start_tick, current.transition_ticks),
                    (16384, 16384)
                );
                assert_eq!(u32::from(current_weight), weight.unwrap());
                assert_eq!(
                    std::fs::read(&previous.path).unwrap()[128..132],
                    [OLD[2], OLD[1], OLD[0], OLD[3]]
                );
                assert_eq!(
                    std::fs::read(&current.path).unwrap()[128..132],
                    [NEW[2], NEW[1], NEW[0], NEW[3]]
                );
                assert_eq!(texture.name, format!("ColorSet-CLEAR at tick {tick}"));
            }
        }
        if tick == 24576 {
            assert_eq!(&texture.rgba[..4], &[150, 75, 128, 128]);
        }
    }
}

#[test]
fn key_sorting_midnight_wrap_single_maps_and_input_normalization_are_explicit() {
    let fixture = Fixture::new();
    fixture.texture("colormap-Old.dds", OLD);
    fixture.texture("colormap-New.dds", NEW);
    // Authored order is retained in provenance even when native time order differs.
    let ini = parse_ini(&TWO_KEYS.replace("Time0=0\nTransition0=0", "Time0=0.75\nTransition0=0"));
    for (fraction, index, pixel) in [
        (0.1, 0, OLD),
        (0.5, 1, NEW),
        (-0.5, 1, NEW),
        (1.5, 1, NEW),
        (f32::NAN, 1, NEW),
        (f32::INFINITY, 1, NEW),
        (f32::NEG_INFINITY, 1, NEW),
    ] {
        let (texture, source) =
            color_map::sample(&fixture.directory(), &ini, "clear", fraction).unwrap();
        assert_eq!(&texture.rgba[..4], &pixel);
        let SkyColorMapInputs::Single(source) = source.inputs else {
            panic!("direct map expected")
        };
        assert_eq!(source.key_index, index);
    }
    let single = parse_ini(
        "[ColorSet-one]\nColorMap0=Old\nTime0=0.25\nTransition0=0.25\n[ColorMap-Old]\nFile=Old",
    );
    for fraction in [0., 0.25, 0.375, 0.5, 0.99] {
        let (texture, source) =
            color_map::sample(&fixture.directory(), &single, "ONE", fraction).unwrap();
        assert_eq!(&texture.rgba[..4], &OLD);
        assert!(matches!(source.inputs, SkyColorMapInputs::Single(_)));
    }
}

#[test]
fn unsupported_keys_and_missing_declarations_do_not_fall_back() {
    let fixture = Fixture::new();
    fixture.texture("colormap-Old.dds", OLD);
    fixture.texture("colormap-New.dds", NEW);
    for text in [
        TWO_KEYS.replace("Time1=0.25", "Time1=NaN"),
        TWO_KEYS.replace("Time1=0.25", "Time1=-0.1"),
        TWO_KEYS.replace("Time1=0.25", "Time1=1"),
        TWO_KEYS.replace("Transition1=0.25", "Transition1=-0.1"),
        TWO_KEYS.replace("Time1=0.25", "Time1=0.000001"),
        TWO_KEYS.replace("Transition0=0", "Transition0=0.5"),
        TWO_KEYS.replace("Transition1=0.25", "Transition1=0.9"),
        TWO_KEYS.replace("ColorMap1=New", "ColorMap2=New"),
        TWO_KEYS.replace("ColorMap1=New", "ColorMap1=Missing"),
        TWO_KEYS.replace("File=New", "File="),
    ] {
        assert!(
            color_map::sample(&fixture.directory(), &parse_ini(&text), "clear", 0.5).is_err(),
            "{text}"
        );
    }
    assert!(color_map::sample(&fixture.directory(), &parse_ini(TWO_KEYS), "missing", 0.5).is_err());
    let missing_file = parse_ini(&TWO_KEYS.replace("File=New", "File=missing"));
    assert!(color_map::sample(&fixture.directory(), &missing_file, "clear", 0.5).is_err());
}

#[test]
fn loader_samples_sky_and_cloud_tables_and_retains_raw_swatches() {
    let fixture = Fixture::new();
    fixture.texture("colormap-Old.dds", OLD);
    fixture.texture("colormap-New.dds", NEW);
    std::fs::write(
        fixture.directory().join("sky.ini"),
        "[SkySetting-default]\nDefaultWeather=fixture\n",
    )
    .unwrap();
    std::fs::write(fixture.directory().join("weather.ini"),format!("{TWO_KEYS}[WeatherPattern-fixture]\nColorSet=Clear\nCloud0=cloud\n[Cloud-cloud]\nColorSet=Clear\n")).unwrap();
    let mut assets = load_sky(&fixture.0, "unlisted", 0.375).unwrap();
    assert_eq!(&assets.color_map.rgba[..4], &[150, 75, 128, 128]);
    assert_eq!(
        assets.color_map.rgba,
        assets.cloud_color_map.as_ref().unwrap().rgba
    );
    assert_eq!(
        assets.color_map_provenance,
        assets.cloud_color_map_provenance
    );
    assert_eq!(assets.color_map_layout, SkyColorMapLayout::OriginalDome);
    assert_eq!(
        assets.cloud_color_map_layout,
        SkyColorMapLayout::OriginalDome
    );
    for (row, rgba) in [
        (0, [1, 2, 3, 0]),
        (1, [4, 5, 6, 7]),
        (3, [8, 9, 10, 11]),
        (28, [12, 13, 14, 15]),
        (29, [16, 17, 18, 19]),
    ] {
        assets.color_map.rgba[(row * 32 + 31) * 4..][..4].copy_from_slice(&rgba);
    }
    assert_eq!(
        assets.raw_light_colors(),
        Some(SkyLightColors {
            sun_directional: 0x00010203,
            moon_directional: 0x07040506,
            ambient: 0x0b08090a,
            sun_bounce: 0x0f0c0d0e,
            moon_bounce: 0x13101112
        })
    );
    assets.color_map_layout = SkyColorMapLayout::FullTexture;
    assert_eq!(assets.raw_light_colors(), None);
    assets.color_map_layout = SkyColorMapLayout::OriginalDome;
    assets.color_map.rgba.pop();
    assert_eq!(assets.raw_light_colors(), None);
}

#[test]
#[ignore = "requires original sky INI and DDS assets; CPU only"]
fn all_installed_color_sets_match_1131_native_full_table_samples() {
    let base = crate::loader::default_client_dir().expect("original client assets");
    let directory = base.join("Resources/sky");
    let ini = parse_ini(&std::fs::read_to_string(directory.join("weather.ini")).unwrap());
    let fixture = include_str!("native_color_map_samples.txt");
    let mut sets = 0;
    let mut samples = 0;
    for line in fixture
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
    {
        let fields = line.split('|').collect::<Vec<_>>();
        let set = fields[0].strip_prefix("colorset-").unwrap();
        let mut checksum = flate2::Crc::new();
        for tick in fields[1]
            .split(',')
            .map(|tick| tick.parse::<u32>().unwrap())
        {
            let (texture, source) =
                color_map::sample(&directory, &ini, set, tick as f32 / 65536.).unwrap();
            assert_eq!(source.day_tick, tick);
            assert_eq!(
                (texture.width, texture.height, texture.rgba.len()),
                (32, 32, 4096)
            );
            checksum.update(&tick.to_le_bytes());
            checksum.update(&texture.rgba);
            samples += 1;
        }
        assert_eq!(
            format!("{:08x}", checksum.sum()),
            fields[2],
            "native full-table mismatch: {set}"
        );
        sets += 1;
    }
    assert_eq!((sets, samples), (78, 1131));
    assert!(color_map::sample(&directory, &ini, "PoDisease-3", 0.5).is_err());
    assert!(color_map::sample(&directory, &ini, "BrightOverCastCloud0", 0.5).is_err());
    let noon = load_sky(&base, "poknowledge", 0.5).unwrap();
    assert_eq!(
        noon.raw_light_colors(),
        Some(SkyLightColors {
            sun_directional: 0xffffffff,
            moon_directional: 0xff000000,
            ambient: 0xffdbd9d9,
            sun_bounce: 0xffbdbcbc,
            moon_bounce: 0xffbdbcbc
        })
    );
}

#[test]
fn provenance_preserves_exact_sample_fraction_before_tick_truncation() {
    let fixture = Fixture::new();
    fixture.texture("colormap-Old.dds", OLD);
    fixture.texture("colormap-New.dds", NEW);
    let ini = parse_ini(TWO_KEYS);
    for fraction in [
        0.,
        0.5,
        -0.5,
        1.5,
        -f32::from_bits(1),
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x3e7c_71c6),
        f32::from_bits(0x3e7c_71c7),
        f32::from_bits(0x3f43_8e39),
        f32::from_bits(0x3f43_8e3a),
    ] {
        let (_, source) = color_map::sample(&fixture.directory(), &ini, "clear", fraction).unwrap();
        let expected = if fraction.is_finite() {
            fraction.rem_euclid(1.)
        } else {
            0.5
        };
        assert_eq!(source.day_fraction_bits, expected.to_bits());
        assert_eq!(source.day_tick, (expected * 65536.) as u32);
    }
}
