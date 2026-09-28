//! Optional integration tests against a legally installed client. Set
//! EQ_CLIENT_DIR when it is not in the default ~/EverQuest location.

use openeq_assets::character::CharacterLibrary;

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
