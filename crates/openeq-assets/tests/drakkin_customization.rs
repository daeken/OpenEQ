//! Palette fidelity and isolation checks against an installed original client.
use openeq_assets::character::{CharacterAppearance, CharacterLibrary};

#[test]
fn authored_drakkin_heritages_tint_only_their_selected_modules() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("dkm.eqg").is_file()
        || !base.join("dkf.eqg").is_file()
        || !base.join("Resources/playercustomization.txt").is_file()
    {
        return;
    }
    let library = CharacterLibrary::load(base, "poknowledge").unwrap();
    let colors = [0xc80000, 0x0a000a, 0x003cdc, 0x006400, 0x647896, 0x967800];
    for (heritage, expected) in colors.into_iter().enumerate() {
        for (code, gender, style) in [("DKM", 0, 6), ("DKF", 1, 4)] {
            let entry = library
                .customization()
                .get(522, heritage as u32, gender)
                .unwrap();
            assert_eq!(entry.base_color, expected);
            assert_eq!(entry.colors.len(), 4);
            assert_eq!(entry.features.hair_styles, if gender == 0 { 9 } else { 8 });
            assert_eq!(entry.features.beards, if gender == 0 { 12 } else { 4 });
            let appearance = CharacterAppearance {
                drakkin_heritage: heritage as u32,
                hair_color: 2,
                beard_color: 1,
                hair_style: style,
                beard: 1,
                drakkin_details: 3,
                drakkin_tattoo: 3,
                ..Default::default()
            };
            assert_eq!(
                library.normalize_race_appearance(522, gender, appearance),
                appearance
            );
            let model = library
                .load_model_with_appearance(code, &appearance)
                .unwrap();
            let textures: Vec<_> = model.materials.iter().flat_map(|m| &m.textures).collect();
            let hair = format!("_hr_s{style:02}_c.dds#tint=ff{:06x}", entry.colors[2]);
            let details = format!("_hr_s{style:02}_c.dds#tint=ff{:06x}", entry.base_color);
            let beard = format!("_fh_c.dds#tint=ff{:06x}", entry.colors[1]);
            let tattoo = format!("_tattoo_s03_m01_c.dds#tint=ff{:06x}", entry.base_color);
            for suffix in [&hair, &details, &beard, &tattoo] {
                assert!(
                    textures.iter().any(|name| name.ends_with(suffix)),
                    "{code} heritage{heritage} lacks {suffix}"
                );
            }
            assert!(
                textures
                    .iter()
                    .filter(|name| name.contains("_head_")
                        || name.contains("_body_")
                        || name.contains("righteye"))
                    .all(|name| !name.contains("#tint=")),
                "heritage unexpectedly recolored skin or eyes"
            );
            assert!(
                model
                    .materials
                    .iter()
                    .filter_map(|m| m.normal_map.as_ref())
                    .all(|name| !name.contains("#tint="))
            );
            for name in textures {
                let tinted = library
                    .texture(name)
                    .unwrap_or_else(|| panic!("missing {name}"));
                if let Some((source, _)) = name.split_once("#tint=") {
                    let original = library.texture(source).unwrap();
                    assert!(
                        tinted
                            .rgba
                            .chunks_exact(4)
                            .zip(original.rgba.chunks_exact(4))
                            .all(|(a, b)| a[3] == b[3]),
                        "tint changed alpha for {name}"
                    );
                    assert!(
                        tinted.rgba != original.rgba,
                        "authored tint did not change {name}"
                    );
                }
            }
            // Two pieces share a hair texture but one belongs to heritage.
            // Changing a hair shade must leave that facial detail untouched.
            let hair_change = library
                .load_model_with_appearance(
                    code,
                    &CharacterAppearance {
                        hair_color: 3,
                        ..appearance
                    },
                )
                .unwrap();
            let beard_change = library
                .load_model_with_appearance(
                    code,
                    &CharacterAppearance {
                        beard_color: 2,
                        ..appearance
                    },
                )
                .unwrap();
            for (a, b) in model.materials.iter().zip(&hair_change.materials) {
                if a.textures != b.textures {
                    assert!(a.textures.iter().all(|name| name.ends_with(&hair)));
                }
            }
            for (a, b) in model.materials.iter().zip(&beard_change.materials) {
                if a.textures != b.textures {
                    assert!(a.textures.iter().all(|name| name.ends_with(&beard)));
                }
            }
            assert_ne!(model.materials, hair_change.materials);
            assert_ne!(model.materials, beard_change.materials);
            assert_eq!(model.bounds_min, hair_change.bounds_min);
            assert_eq!(model.bounds_max, hair_change.bounds_max);
        }
    }
    for code in ["DKM", "DKF"] {
        let model = library.load_model(code).unwrap();
        assert!(
            model
                .materials
                .iter()
                .flat_map(|m| &m.textures)
                .any(|name| name.ends_with("#tint=ffc80000")),
            "default heritage not applied to {code}"
        );
    }
}
