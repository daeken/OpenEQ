//! Optional integration tests against a legally installed client. Set
//! EQ_CLIENT_DIR when it is not in the default ~/EverQuest location.

use openeq_assets::character::CharacterLibrary;

#[test]
fn classic_matrick_outfits_load_original_extended_armor_and_keep_equipment_overrides() {
    use openeq_assets::character::CharacterAppearance;
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("global20_amr.s3d").is_file() || !base.join("global21_amr.s3d").is_file() {
        return;
    }
    let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
    // These are the actual PEQ PoK NPC appearances: the default body texture
    // is not their outfit. Extended armor lives outside global_chr.s3d.
    for (npc, gender, texture, face) in [
        ("Tratlan_Matrick", 0, 20, 2),
        ("Higwyn_Matrick", 0, 21, 3),
        ("Sherin_Matrick", 1, 21, 1),
    ] {
        let appearance = library.normalize_race_appearance(
            4,
            gender,
            CharacterAppearance {
                texture,
                helm_texture: 255,
                face,
                ..Default::default()
            },
        );
        let model = library
            .load_race_with_appearance(4, gender, &appearance)
            .unwrap();
        let code = model.code.to_ascii_lowercase();
        let names = |model: &openeq_assets::character::CharacterModel| {
            model
                .materials
                .iter()
                .flat_map(|m| m.textures.clone())
                .collect::<Vec<_>>()
        };
        let outfit = names(&model);
        for part in ["ch", "ua", "fa", "hn", "lg", "ft"] {
            let expected = format!("{code}{part}{texture:02}01.bmp");
            assert!(
                outfit.contains(&expected),
                "{npc} did not select {expected}: {outfit:?}"
            );
        }
        assert!(outfit.contains(&format!("{code}he00{face}1.bmp")));
        for name in &outfit {
            let source = library
                .texture(name)
                .unwrap_or_else(|| panic!("missing {npc} texture {name}"));
            assert!(source.width > 1 && source.height > 1);
        }
        let mut equipped = appearance;
        equipped.equipment[1].material = 3;
        equipped.equipment[1].color = 0xff4080ff;
        equipped.equipment[7].material = 1;
        let equipped = library
            .load_race_with_appearance(4, gender, &equipped)
            .unwrap();
        let overridden = names(&equipped);
        assert!(overridden.contains(&format!("{code}ch0301.bmp#tint=ff4080ff")));
        assert!(overridden.contains(&format!("{code}lg{texture:02}01.bmp")));
        assert_eq!(equipped.bounds_min, model.bounds_min);
        assert_eq!(equipped.bounds_max, model.bounds_max);
        assert!(equipped.meshes.len() > model.meshes.len());
        assert_eq!(
            names(
                &library
                    .load_race_with_appearance(4, gender, &appearance)
                    .unwrap()
            ),
            outfit
        );
        let default = library.load_race(4, gender).unwrap();
        assert!(names(&default).contains(&format!("{code}ch0001.bmp")));
        assert_ne!(
            library
                .texture(&format!("{code}ch{texture:02}01.bmp"))
                .unwrap()
                .rgba,
            library.texture(&format!("{code}ch0001.bmp")).unwrap().rgba
        );
    }
}

#[test]
fn original_eqg_items_attach_without_changing_body_scale() {
    use openeq_assets::character::CharacterAppearance;
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("it13911.eqg").is_file() || !base.join("it100044.eqg").is_file() {
        return;
    }
    let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
    let mut appearance = CharacterAppearance::default();
    appearance.equipment[7].material = 100044; // Authored Ram Sword.
    appearance.equipment[8].material = 13911; // Authored Shield of Fear surface.
    assert_eq!(
        library.normalize_race_appearance(1, 0, appearance),
        appearance
    );
    let bare = library.load_race(1, 0).unwrap();
    let equipped = library
        .load_race_with_appearance(1, 0, &appearance)
        .unwrap();
    assert!(equipped.meshes.len() >= bare.meshes.len() + 2);
    assert_eq!(bare.bounds_min, equipped.bounds_min);
    assert_eq!(bare.bounds_max, equipped.bounds_max);
    for material in &equipped.materials {
        for name in &material.textures {
            assert!(library.texture(name).is_some(), "missing {name}");
        }
    }
    let a = equipped.sample("C05", 0.1);
    let b = equipped.sample("C05", 0.3);
    assert!(
        a[bare.meshes.len()..]
            .iter()
            .zip(&b[bare.meshes.len()..])
            .all(|(a, b)| a.vertices != b.vertices)
    );
    assert!(b.iter().flat_map(|m| &m.vertices).all(|v| v.is_finite()));
}

#[test]
fn greater_faydark_decaying_skeleton_uses_its_global_ldon_model() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("skt_chr.s3d").is_file() || !base.join("gfaydark_chr.s3d").is_file() {
        return;
    }
    // PEQ's decaying skeleton is race 367; classic race 60 is a different
    // model. SKT must be available even though gfaydark_chr.txt omits it.
    let library = CharacterLibrary::load(&base, "gfaydark").unwrap();
    let model = library.load_race(367, 2).expect("global LDON skeleton");
    assert_eq!(model.code, "SKT");
    assert_eq!(library.load_race(60, 2).unwrap().code, "SKE");
    assert!(model.meshes.iter().map(|m| m.indices.len()).sum::<usize>() > 300);
    assert!(model.bounds_min[2] < -2.0 && model.bounds_max[2] > 2.0);
    for material in &model.materials {
        for name in &material.textures {
            let texture = library
                .texture(name)
                .unwrap_or_else(|| panic!("missing {name}"));
            assert!(
                texture.width > 1 && texture.height > 1,
                "placeholder {name}"
            );
            assert!(
                texture.rgba.chunks_exact(4).any(|p| p[3] >= 128),
                "invisible {name}"
            );
        }
    }
    for clip in [model.idle_animation(), model.walk_animation()] {
        assert!(!clip.is_empty());
        assert!(model.animations[clip].frame_count > 1);
        let a = model.sample(clip, 0.0);
        let b = model.sample(clip, 0.22);
        assert!(a.iter().zip(&b).any(|(a, b)| a.vertices != b.vertices));
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(a.indices, b.indices);
            assert!(b.vertices.iter().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn classic_character_faces_use_bitmap_orientation_before_caching_and_tinting() {
    use openeq_assets::{character::CharacterAppearance, pfs::Archive, texture::Texture};
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("global_chr.s3d").is_file() {
        return;
    }
    let archive = Archive::open(base.join("global_chr.s3d")).unwrap();
    let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
    for (race, code) in [(1, "hum"), (12, "gnm")] {
        for face in [0, 3] {
            let model = library
                .load_race_with_appearance(
                    race,
                    0,
                    &CharacterAppearance {
                        face,
                        ..Default::default()
                    },
                )
                .unwrap();
            let name = format!("{code}he00{face}1.bmp");
            assert!(model.materials.iter().any(|m| m.textures.contains(&name)));
            let bytes = archive.read(&name).unwrap();
            assert!(bytes.starts_with(b"BM"), "original face must exercise BMP");
            let raw = Texture::decode(&name, &bytes).unwrap();
            let texture = library.texture(&name).unwrap();
            let stride = raw.width as usize * 4;
            assert_ne!(
                texture.rgba, raw.rgba,
                "asymmetric face must change orientation"
            );
            for (expected, actual) in raw
                .rgba
                .chunks_exact(stride)
                .rev()
                .zip(texture.rgba.chunks_exact(stride))
            {
                assert_eq!(actual, expected, "{name} does not match authored WLD UVs");
            }
            assert_eq!(library.texture(&name).unwrap().rgba, texture.rgba);
            let tinted = library.texture(&format!("{name}#tint=ff804020")).unwrap();
            for (source, tinted) in texture
                .rgba
                .chunks_exact(4)
                .zip(tinted.rgba.chunks_exact(4))
            {
                for (channel, factor) in [128u16, 64, 32].into_iter().enumerate() {
                    assert_eq!(
                        tinted[channel],
                        (u16::from(source[channel]) * factor / 255) as u8
                    );
                }
                assert_eq!(tinted[3], source[3]);
            }
        }
    }
}

#[test]
fn real_classic_characters_have_textures_and_moving_bones() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("global_chr.s3d").is_file() {
        return;
    }
    let library = CharacterLibrary::load(&base, "poknowledge").expect("character library");
    eprintln!("loaded {} actor definitions", library.model_codes().count());
    for code in ["HUM", "HUF", "DWM", "ELF", "SKE", "WER", "AVI"] {
        let model = library
            .load_model(code)
            .unwrap_or_else(|e| panic!("{code}: {e}"));
        let animation = model.walk_animation();
        assert_ne!(
            animation,
            model.idle_animation(),
            "{code} needs a walking animation"
        );
        let clip = &model.animations[animation];
        assert!(clip.frame_count > 1, "{code} walking clip has no animation");
        let a = model.sample(animation, 0.0);
        let b = model.sample(animation, clip.duration_seconds() * 0.27);
        let moved = a.iter().zip(&b).any(|(a, b)| {
            a.vertices
                .iter()
                .zip(&b.vertices)
                .any(|(a, b)| (a - b).abs() > 0.02)
        });
        assert!(moved, "{code} walking animation does not move any vertices");
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(
                a.indices, b.indices,
                "{code} topology changed while animating"
            );
            assert_eq!(a.vertices.len(), b.vertices.len());
            assert!(b.vertices.iter().all(|v| v.is_finite()));
            for vertex in b.vertices.chunks_exact(8) {
                let length =
                    (vertex[3] * vertex[3] + vertex[4] * vertex[4] + vertex[5] * vertex[5]).sqrt();
                assert!(
                    length < 0.001 || (length - 1.0).abs() < 0.001,
                    "{code}: non-unit normal {length}"
                );
            }
        }
        let height = model.bounds_max[2] - model.bounds_min[2];
        assert!(
            height > 1.0 && height < 30.0,
            "{code} implausible height: {height}"
        );
        for material in &model.materials {
            for name in &material.textures {
                assert!(
                    library.texture(name).is_some(),
                    "{code} texture {name} not decoded"
                );
            }
        }
        eprintln!(
            "{code}: {} meshes, {} clips, walk {animation} ({} frames), bounds {:?} to {:?}",
            model.meshes.len(),
            model.animations.len(),
            clip.frame_count,
            model.bounds_min,
            model.bounds_max
        );
    }
}

#[test]
fn available_zone_models_load_without_panics() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("global_chr.s3d").is_file() {
        return;
    }
    let library = CharacterLibrary::load(&base, "poknowledge").expect("character library");
    let mut loaded = 0;
    let mut rejected = Vec::new();
    for code in library.model_codes() {
        match library.load_model(code) {
            Ok(model) => {
                assert!(!model.meshes.is_empty());
                assert!(
                    model
                        .sample(model.walk_animation(), 1.25)
                        .iter()
                        .all(|m| m.vertices.iter().all(|v| v.is_finite())),
                    "{code}: non-finite animated vertices"
                );
                loaded += 1;
            }
            Err(error) => rejected.push(format!("{code}: {error}")),
        }
    }
    eprintln!("loaded {loaded} models; rejected: {}", rejected.join("; "));
    assert!(loaded >= 30, "too few supported classic models");
}

/// The packed track's last word is uniform scale; translations always use
/// 1/256 units. Old readers divided translation by scale and lost scaling.
#[test]
fn wld_animation_decodes_packed_and_float_transforms() {
    use openeq_assets::wld::{Fragment, WLD_MAGIC, Wld};
    fn decode(flags: u32, frame: &[u8]) -> openeq_assets::wld::Frame {
        let mut data = Vec::new();
        for value in [WLD_MAGIC, 0x0001_5500, 1, 0, 0, 4, 0] {
            data.extend(value.to_le_bytes());
        }
        data.extend([0x95, 0x3A, 0xC5, 0x2A]); // Four XOR-encoded NULs.
        for value in [12 + frame.len() as u32, 0x12, 0, flags, 1] {
            data.extend(value.to_le_bytes());
        }
        data.extend(frame);
        let wld = Wld::parse("synthetic.wld".into(), &data).unwrap();
        let Fragment::PieceTrack(track) = &wld.chunks()[0].fragment else {
            panic!("track expected")
        };
        assert_eq!(track.frames.len(), 1);
        track.frames[0]
    }
    let packed: Vec<_> = [16384i16, 0, 0, 0, 512, -256, 128, 512]
        .into_iter()
        .flat_map(i16::to_le_bytes)
        .collect();
    let floating: Vec<_> = [2.0f32, 2.0, -1.0, 0.5, 1.0, 0.0, 0.0, 0.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    for frame in [decode(8, &packed), decode(0, &floating)] {
        assert_eq!(frame.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(frame.translation, [2.0, -1.0, 0.5]);
        assert_eq!(frame.scale, 2.0);
    }
}

#[test]
fn real_equipment_follows_bones_and_appearance_variants_resolve() {
    use openeq_assets::character::CharacterAppearance;
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("gequip.s3d").is_file() {
        return;
    }
    let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
    let naked = library.load_model("HUM").unwrap();
    let mut appearance = CharacterAppearance {
        face: 3,
        ..Default::default()
    };
    for slot in &mut appearance.equipment[..7] {
        slot.material = 3;
        slot.color = 0xff4080ff;
    }
    appearance.equipment[7].material = 1;
    appearance.equipment[8].material = 201;
    let equipped = library
        .load_model_with_appearance("HUM", &appearance)
        .unwrap();
    assert_eq!(equipped.bounds_min[2], naked.bounds_min[2]);
    assert_eq!(
        equipped.bounds_max, naked.bounds_max,
        "gear changes must not resize the body"
    );
    assert!(equipped.meshes.len() > naked.meshes.len());
    let names: Vec<_> = equipped
        .materials
        .iter()
        .flat_map(|m| &m.textures)
        .collect();
    assert!(
        names.iter().any(|name| name.starts_with("humch03")),
        "{names:?}"
    );
    assert!(
        names.iter().any(|name| name.contains("#tint=ff4080ff")),
        "{names:?}"
    );
    for name in names {
        let texture = library
            .texture(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert!(texture.rgba.len() >= 4);
        if let Some((source, _)) = name.split_once("#tint=") {
            let untinted = library.texture(source).unwrap();
            assert_ne!(texture.rgba, untinted.rgba, "tint not applied to {name}");
            assert_eq!(texture.rgba[3], untinted.rgba[3]);
        }
    }
    let a = equipped.sample("C05", 0.);
    let b = equipped.sample("C05", 0.4);
    let last = a.len() - 1;
    assert_ne!(
        a[last].vertices, b[last].vertices,
        "held equipment did not move with attack"
    );
    assert_eq!(a[last].indices, b[last].indices);
    assert_eq!(a[last].vertices[6..8], b[last].vertices[6..8]);
    // A death must stay down, rather than wrap into the standing first frame.
    let mut dead = equipped.meshes.clone();
    let mut later = dead.clone();
    equipped.sample_into_mode("D05", 10., false, &mut dead);
    equipped.sample_into_mode("D05", 20., false, &mut later);
    assert!(
        dead.iter()
            .zip(&later)
            .all(|(a, b)| a.vertices == b.vertices)
    );
    let vertices = |m: &openeq_assets::character::CharacterModel| {
        m.meshes.iter().map(|g| g.vertex_count()).sum::<usize>()
    };
    let mut robe = CharacterAppearance {
        texture: 255,
        helm_texture: 255,
        ..Default::default()
    };
    robe.equipment[1].material = 10;
    let robe = library.load_model_with_appearance("HUM", &robe).unwrap();
    assert_ne!(
        vertices(&robe),
        vertices(&naked),
        "robe did not replace the body mesh"
    );
    for name in robe.materials.iter().flat_map(|m| &m.textures) {
        assert!(library.texture(name).is_some(), "robe texture {name}");
    }
    let mut robe_variant = CharacterAppearance {
        texture: 255,
        ..Default::default()
    };
    robe_variant.equipment[1].material = 16;
    let robe_variant = library
        .load_model_with_appearance("HUM", &robe_variant)
        .unwrap();
    assert!(
        robe_variant
            .materials
            .iter()
            .flat_map(|m| &m.textures)
            .any(|name| name.starts_with("clk10"))
    );
    // Palette keyed masked textures must not display their key color.
    let mask = library.texture("helm15.bmp#masked").unwrap();
    assert!(mask.rgba.chunks_exact(4).any(|p| p[3] == 0));
    assert!(
        mask.rgba
            .chunks_exact(4)
            .filter(|p| p[..3] == [255, 0, 255])
            .all(|p| p[3] == 0)
    );
}

#[test]
fn modern_weighted_characters_preserve_bind_pose_and_animate() {
    let Some(base) = openeq_assets::loader::default_client_dir() else {
        return;
    };
    if !base.join("ggy.eqg").is_file() {
        return;
    }
    let library = CharacterLibrary::load(&base, "poknowledge").unwrap();
    for code in ["GGY", "DKM", "DKF", "BDR", "ONM"] {
        let model = library
            .load_model(code)
            .unwrap_or_else(|e| panic!("{code}: {e}"));
        assert!(
            model.animations.contains_key("L01"),
            "{code} has no walking clip"
        );
        let bind = model.sample("", 0.);
        for (a, b) in model.meshes.iter().zip(&bind) {
            assert!(
                a.vertices
                    .iter()
                    .zip(&b.vertices)
                    .all(|(a, b)| (a - b).abs() < 0.0001),
                "{code} bind pose drift"
            );
        }
        let a = model.sample("L01", 0.);
        let b = model.sample("L01", 0.22);
        assert!(
            a.iter().zip(&b).any(|(a, b)| a
                .vertices
                .iter()
                .zip(&b.vertices)
                .any(|(a, b)| (a - b).abs() > 0.02)),
            "{code} animation is static"
        );
        for (a, b) in a.iter().zip(&b) {
            assert_eq!(a.indices, b.indices);
            assert!(b.vertices.iter().all(|v| v.is_finite()));
        }
        for name in model.materials.iter().flat_map(|m| &m.textures) {
            assert!(library.texture(name).is_some(), "{code} missing {name}");
        }
    }
}
