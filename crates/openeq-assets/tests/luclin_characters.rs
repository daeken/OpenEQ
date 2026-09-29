//! Compatibility checks against an installed client's replacement WLD actors.
use openeq_assets::character::{CharacterAppearance, CharacterLibrary, CharacterModelSet};

fn library() -> Option<CharacterLibrary> {
    let base = openeq_assets::loader::default_client_dir()?;
    if !base.join("globalhum_chr.s3d").is_file() || !base.join("lgequip.s3d").is_file() {
        return None;
    }
    Some(
        CharacterLibrary::load_with_model_set(base, "poknowledge", CharacterModelSet::Luclin)
            .unwrap(),
    )
}

#[test]
fn replacement_races_resolve_textures_and_animate_with_native_tracks() {
    let Some(library) = library() else {
        return;
    };
    for race in (1..=12).chain([128, 130]) {
        for gender in 0..=1 {
            let model = library.load_race(race, gender).unwrap();
            // Not every installation has every replacement. Classic fallback
            // is intentional, but the installed human is our Luclin sentinel.
            if race == 1 {
                assert!(
                    model.animations.contains_key("P01A"),
                    "{} lacks native take",
                    model.code
                );
                assert!(model.bone_names.len() > 80);
            }
            for animation in ["P01", "L01", "C01", "D01"] {
                if !model.animations.contains_key(animation) {
                    continue;
                }
                for time in [0., 0.17, 0.43] {
                    for mesh in model.sample(animation, time) {
                        assert!(
                            mesh.vertices.iter().all(|v| v.is_finite()),
                            "{} {animation}",
                            model.code
                        );
                        assert!(
                            mesh.vertices
                                .chunks_exact(8)
                                .all(|v| v[..3].iter().all(|x| x.abs() < 30.)),
                            "{} {animation} stretched",
                            model.code
                        );
                    }
                }
            }
            let a = model.sample("L01", 0.);
            let b = model.sample("L01", 0.2);
            assert!(
                a.iter().zip(&b).any(|(a, b)| a.vertices != b.vertices),
                "{} walk is static",
                model.code
            );
            for name in model.materials.iter().flat_map(|m| &m.textures) {
                assert!(
                    library.texture(name).is_some(),
                    "{} missing {name}",
                    model.code
                );
            }
            let plate = library
                .load_race_with_appearance(
                    race,
                    gender,
                    &CharacterAppearance {
                        texture: 3,
                        helm_texture: 3,
                        ..Default::default()
                    },
                )
                .unwrap();
            for mesh in plate.sample("L01", 0.3) {
                assert!(mesh.vertices.iter().all(|v| v.is_finite()));
                assert!(
                    mesh.vertices
                        .chunks_exact(8)
                        .all(|v| v[..3].iter().all(|x| x.abs() < 30.)),
                    "{} plate stretched",
                    model.code
                );
            }
            for name in plate.materials.iter().flat_map(|m| &m.textures) {
                assert!(
                    library.texture(name).is_some(),
                    "{} plate missing {name}",
                    model.code
                );
            }
        }
    }
}

#[test]
fn erudite_glyphs_and_barbarian_tattoos_use_authored_head_layers() {
    let Some(library) = library() else {
        return;
    };
    for (code, appearance, expected) in [
        (
            "ERM",
            CharacterAppearance {
                face: 3,
                hair_style: 5,
                ..Default::default()
            },
            "ermhe0501.dds",
        ),
        (
            "ERF",
            CharacterAppearance {
                face: 3,
                hair_style: 8,
                ..Default::default()
            },
            "erfhe0801.dds",
        ),
        (
            "BAM",
            CharacterAppearance {
                face: 73,
                ..Default::default()
            },
            "bamhe0701.dds",
        ),
        (
            "BAF",
            CharacterAppearance {
                face: 73,
                ..Default::default()
            },
            "bafhe0701.dds",
        ),
    ] {
        let model = library
            .load_model_with_appearance(code, &appearance)
            .unwrap();
        assert!(
            model
                .materials
                .iter()
                .flat_map(|m| &m.textures)
                .any(|name| name.contains(expected)),
            "{code} lacks {expected}"
        );
        for name in model.materials.iter().flat_map(|m| &m.textures) {
            assert!(library.texture(name).is_some(), "{code} missing {name}");
        }
    }
}

#[test]
fn layered_armor_faces_eyes_and_modular_parts_preserve_body_scale() {
    let Some(library) = library() else {
        return;
    };
    for code in ["HUM", "HUF"] {
        let bare = library.load_model(code).unwrap();
        for armor in [0, 1, 2, 3, 10, 16] {
            let mut appearance = CharacterAppearance {
                texture: armor,
                helm_texture: armor.min(3),
                face: 3,
                hair_style: 1,
                beard: if code == "HUM" { 1 } else { 0 },
                eye_color_1: 2,
                eye_color_2: 7,
                ..Default::default()
            };
            appearance.equipment[1].color = 0xff40_80ff;
            let model = library
                .load_model_with_appearance(code, &appearance)
                .unwrap();
            assert_eq!(model.bounds_min, bare.bounds_min);
            assert_eq!(model.bounds_max, bare.bounds_max);
            assert!(
                model.meshes.len() > bare.meshes.len(),
                "{code} armor {armor} has no appearance parts"
            );
            let textures: Vec<_> = model.materials.iter().flat_map(|m| &m.textures).collect();
            assert!(
                textures.iter().any(|name| name.contains("hesk31.dds")),
                "{code} armor {armor} missing face: {textures:?}"
            );
            for eye in ["chr_eye003.dds", "chr_eye008.dds"] {
                assert!(
                    model
                        .meshes
                        .iter()
                        .any(|mesh| model.materials[mesh.material]
                            .textures
                            .iter()
                            .any(|name| name == eye)),
                    "{code} missing independent eye {eye}"
                );
            }
            for name in textures {
                assert!(
                    library.texture(name).is_some(),
                    "{code} armor {armor} missing {name}"
                );
            }
            let a = model.sample("L01", 0.);
            let b = model.sample("L01", 0.3);
            assert!(
                a[bare.meshes.len()..]
                    .iter()
                    .zip(&b[bare.meshes.len()..])
                    .any(|(a, b)| a.vertices != b.vertices),
                "{code} modular parts do not follow animation"
            );
            for mesh in &b {
                assert!(mesh.vertices.iter().all(|v| v.is_finite()));
                assert!(
                    mesh.vertices
                        .chunks_exact(8)
                        .all(|v| v[..3].iter().all(|x| x.abs() < 15.)),
                    "{code} armor {armor} part stretched"
                );
            }
        }
        let mut appearance = CharacterAppearance::default();
        appearance.equipment[1].material = 1;
        let armor = library
            .load_model_with_appearance(code, &appearance)
            .unwrap();
        assert!(
            bare.materials
                .iter()
                .zip(&armor.materials)
                .any(|(skin, clothed)| {
                    skin.textures
                        .iter()
                        .zip(&clothed.textures)
                        .any(|(skin, clothed)| {
                            skin.contains("chsk")
                                && library.texture(skin).unwrap().rgba
                                    != library.texture(clothed).unwrap().rgba
                        })
                }),
            "{code} armor overlay has no effect"
        );
    }
}

#[test]
fn classic_remains_default_and_unreplaced_actors_remain_classic() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("globalhum_chr.s3d").is_file() {
        return;
    }
    let classic = CharacterLibrary::load(&base, "poknowledge").unwrap();
    let Some(luclin) = library() else {
        return;
    };
    let normalized = luclin.normalize_race_appearance(
        1,
        0,
        CharacterAppearance {
            face: 7,
            hair_style: 3,
            beard: 5,
            hair_color: 12,
            beard_color: 14,
            eye_color_1: 2,
            eye_color_2: 7,
            ..Default::default()
        },
    );
    assert_eq!(
        (normalized.face, normalized.hair_style, normalized.beard),
        (7, 3, 5)
    );
    assert_eq!((normalized.eye_color_1, normalized.eye_color_2), (2, 7));
    // Unsupported tint fields must not create distinct visual cache entries.
    assert_eq!((normalized.hair_color, normalized.beard_color), (0, 0));
    let human = classic.load_race(1, 0).unwrap();
    assert!(!human.animations.contains_key("P01A"));
    assert!(human.bone_names.len() < 80);
    // Skeleton NPCs are unrelated to the playable-race replacement switch.
    let a = classic.load_race(60, 2).unwrap();
    let b = luclin.load_race(60, 2).unwrap();
    assert_eq!(a.bone_names, b.bone_names);
    assert_eq!(a.materials, b.materials);
    assert_eq!(a.bounds_min, b.bounds_min);
    assert_eq!(a.bounds_max, b.bounds_max);
}
