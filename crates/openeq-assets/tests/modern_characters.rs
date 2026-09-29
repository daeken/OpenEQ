//! Compatibility checks against the locally installed, original EQG assets.
use openeq_assets::character::{CharacterAppearance, CharacterLibrary, CharacterPose};

fn library() -> Option<CharacterLibrary> {
    let base = openeq_assets::loader::default_client_dir()?;
    if !base.join("dkm.eqg").is_file() || !base.join("dkf.eqg").is_file() {
        return None;
    }
    Some(CharacterLibrary::load(&base, "poknowledge").unwrap())
}

#[test]
fn drakkin_face_uvs_match_original_dds_landmarks() {
    let Some(library) = library() else {
        return;
    };
    for code in ["DKM", "DKF"] {
        let model = library.load_model(code).unwrap();
        let nose = model
            .meshes
            .iter()
            .filter(|mesh| {
                model.materials[mesh.material]
                    .textures
                    .iter()
                    .any(|name| name.contains("_head_s00_"))
            })
            .flat_map(|mesh| mesh.vertices.chunks_exact(8))
            .max_by(|a, b| a[0].total_cmp(&b[0]))
            .unwrap();
        // In both original head DDSes the nose is centered at (U=.5,V=.394).
        // Keeping the MOD's bottom-origin V=.606 puts chin pixels on the nose,
        // eye sockets on the neck, and mouth pixels across the forehead.
        assert!((nose[6] - 0.5).abs() < 0.01);
        assert!((nose[7] - 0.394).abs() < 0.01, "{code}: nose V={}", nose[7]);
    }
}

#[test]
fn drakkin_modules_have_textures_and_keep_body_scale_across_appearances() {
    let Some(library) = library() else {
        return;
    };
    for code in ["DKM", "DKF"] {
        let base = library.load_model(code).unwrap();
        let names: Vec<_> = base.materials.iter().flat_map(|m| &m.textures).collect();
        assert!(
            names.iter().any(|name| name.contains("_00_00_")),
            "{code} lacks default clothing"
        );
        assert!(
            names.iter().any(|name| name.contains("_hr_s00_")),
            "{code} lacks default hair"
        );
        assert!(
            names.iter().any(|name| name.contains("_tattoo_s00_")),
            "{code} lacks tattoos"
        );
        for armor in [0, 1, 2, 3, 4, 10, 11, 12, 13, 14, 15, 16] {
            let mut appearance = CharacterAppearance {
                texture: armor,
                helm_texture: if armor <= 4 { armor } else { 0 },
                face: 6,
                eye_color_1: 3,
                eye_color_2: 11,
                hair_style: 7,
                beard: 3,
                drakkin_details: 7,
                drakkin_tattoo: 7,
                ..Default::default()
            };
            appearance.equipment[1].color = 0xff80_a0c0;
            let model = library
                .load_model_with_appearance(code, &appearance)
                .unwrap();
            assert_eq!(
                model.bounds_min, base.bounds_min,
                "{code} armor{armor} changed scale"
            );
            assert_eq!(
                model.bounds_max, base.bounds_max,
                "{code} armor{armor} changed scale"
            );
            assert!(model.meshes.len() > 4, "{code} lost modular geometry");
            for texture in model
                .materials
                .iter()
                .flat_map(|m| m.textures.iter().chain(m.normal_map.iter()))
            {
                assert!(
                    library.texture(texture).is_some(),
                    "{code} armor{armor}: missing {texture}"
                );
            }
            let textures: Vec<_> = model.materials.iter().flat_map(|m| &m.textures).collect();
            assert!(
                textures.iter().any(|t| t.contains("_head_s06_")),
                "{code} armor{armor} textures {textures:?}"
            );
            assert!(textures.iter().any(|t| t.contains("_righteye_s03_")));
            assert!(textures.iter().any(|t| t.contains("_righteye_s11_")));
            assert!(textures.iter().any(|t| t.contains("_tattoo_s07_")));
            assert!(textures.iter().any(|t| t.ends_with("#tint=ff80a0c0")));
            if armor >= 10 {
                assert!(
                    textures
                        .iter()
                        .any(|t| t.contains(&format!("_10_00_s{:02}_", armor - 10)))
                );
            }
            for time in [0., 0.22, 0.48, 0.8] {
                let posed = model.sample("L01", time);
                for (mesh, original) in posed.iter().zip(&model.meshes) {
                    assert_eq!(mesh.indices, original.indices);
                    assert!(mesh.vertices.iter().all(|value| value.is_finite()));
                    assert!(
                        mesh.vertices
                            .chunks_exact(8)
                            .all(|v| v[..3].iter().all(|x| x.abs() < 15.)),
                        "{code} modular bone mapping stretched geometry"
                    );
                }
            }
        }
    }
}

#[test]
fn drakkin_facial_modules_and_bald_option_select_authored_geometry() {
    let Some(library) = library() else {
        return;
    };
    for code in ["DKM", "DKF"] {
        let base = library.load_model(code).unwrap();
        for style in 0..=if code == "DKM" { 8 } else { 7 } {
            let model = library
                .load_model_with_appearance(
                    code,
                    &CharacterAppearance {
                        hair_style: style,
                        ..Default::default()
                    },
                )
                .unwrap();
            if style == 8 {
                assert!(
                    model.meshes.len() < base.meshes.len(),
                    "male bald variant kept hair"
                );
            } else {
                assert!(
                    model
                        .materials
                        .iter()
                        .flat_map(|m| &m.textures)
                        .any(|t| t.contains(&format!("_hr_s{style:02}_")))
                );
            }
        }
        for (limit, facial_hair) in [(if code == "DKM" { 11 } else { 3 }, true), (7, false)] {
            for variant in 1..=limit {
                let appearance = if facial_hair {
                    CharacterAppearance {
                        beard: variant,
                        ..Default::default()
                    }
                } else {
                    CharacterAppearance {
                        drakkin_details: u32::from(variant),
                        ..Default::default()
                    }
                };
                let model = library
                    .load_model_with_appearance(code, &appearance)
                    .unwrap();
                assert_ne!(
                    model.meshes.iter().map(|m| &m.vertices).collect::<Vec<_>>(),
                    base.meshes.iter().map(|m| &m.vertices).collect::<Vec<_>>(),
                    "{code} unchanged facial_hair={facial_hair} variant{variant}"
                );
            }
        }
    }
}

#[test]
fn modern_equipment_follows_hand_bones_and_blending_preserves_endpoints() {
    let Some(library) = library() else {
        return;
    };
    for code in ["DKM", "DKF"] {
        let base = library.load_model(code).unwrap();
        let mut appearance = CharacterAppearance::default();
        appearance.equipment[7].material = 1;
        appearance.equipment[8].material = 201;
        let model = library
            .load_model_with_appearance(code, &appearance)
            .unwrap();
        assert!(model.meshes.len() >= base.meshes.len() + 2);
        assert_eq!(model.bounds_min, base.bounds_min);
        assert_eq!(model.bounds_max, base.bounds_max);
        let from = CharacterPose {
            animation: "P01",
            time_seconds: 0.,
            looping: true,
        };
        let to = CharacterPose {
            animation: "C05",
            time_seconds: 0.2,
            looping: false,
        };
        let mut a = model.meshes.clone();
        let mut b = model.meshes.clone();
        let mut middle = model.meshes.clone();
        assert!(model.sample_blended_into(from, to, 0., &mut a));
        assert!(model.sample_blended_into(from, to, 1., &mut b));
        assert!(model.sample_blended_into(from, to, 0.5, &mut middle));
        let from_meshes = model.sample("P01", 0.);
        let mut to_meshes = model.meshes.clone();
        assert!(model.sample_into_mode("C05", 0.2, false, &mut to_meshes));
        for ((a, from), (b, to)) in a.iter().zip(&from_meshes).zip(b.iter().zip(&to_meshes)) {
            assert_eq!(a.vertices, from.vertices);
            assert_eq!(b.vertices, to.vertices);
        }
        for mesh in &middle {
            assert!(mesh.vertices.iter().all(|v| v.is_finite()));
        }
        assert!(
            a[base.meshes.len()..]
                .iter()
                .zip(&b[base.meshes.len()..])
                .any(|(a, b)| a
                    .vertices
                    .iter()
                    .zip(&b.vertices)
                    .any(|(x, y)| (x - y).abs() > 0.05)),
            "{code} equipment does not follow animated hand"
        );
    }
}
