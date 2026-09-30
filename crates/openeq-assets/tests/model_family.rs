//! Resolved family must describe the loaded assets, including preference fallback.
use openeq_assets::character::{
    CharacterAppearance, CharacterLibrary, CharacterModelFamily, CharacterModelSet,
};

#[test]
#[ignore = "requires original classic/Luclin face assets; CPU only"]
fn authored_face_defaults_and_shared_head_pieces_resolve() {
    let base = openeq_assets::loader::default_client_dir().expect("original assets");
    for preference in [CharacterModelSet::Classic, CharacterModelSet::Luclin] {
        let library = CharacterLibrary::load_with_model_set(&base, "", preference).unwrap();
        for race in (1..=12).chain([128]) {
            for gender in [0, 1] {
                for face in 0..=7 {
                    let model = library
                        .load_race_with_appearance(
                            race,
                            gender,
                            &CharacterAppearance {
                                face,
                                ..Default::default()
                            },
                        )
                        .unwrap();
                    assert!(
                        model.appearance_resolved(),
                        "{} {preference:?} face{face}",
                        model.code
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "requires original classic/Luclin/Drakkin/EQG assets; CPU only"]
fn original_models_retain_actual_family_including_missing_replacement() {
    let base = openeq_assets::loader::default_client_dir().expect("original assets");
    for (preference, race, gender, expected) in [
        (
            CharacterModelSet::Classic,
            1,
            0,
            CharacterModelFamily::Classic,
        ),
        (
            CharacterModelSet::Luclin,
            1,
            0,
            CharacterModelFamily::Luclin,
        ),
        (
            CharacterModelSet::Classic,
            522,
            0,
            CharacterModelFamily::Drakkin,
        ),
        (
            CharacterModelSet::Luclin,
            522,
            1,
            CharacterModelFamily::Drakkin,
        ),
        (
            CharacterModelSet::Classic,
            464,
            2,
            CharacterModelFamily::Modern,
        ),
    ] {
        let library = CharacterLibrary::load_with_model_set(&base, "", preference).unwrap();
        for appearance in [
            CharacterAppearance::default(),
            CharacterAppearance {
                face: 1,
                ..Default::default()
            },
        ] {
            let model = library
                .load_race_with_appearance(race, gender, &appearance)
                .unwrap();
            assert_eq!(model.family(), expected, "{} {preference:?}", model.code);
            assert!(
                model.appearance_resolved(),
                "{} {preference:?} {appearance:?}",
                model.code
            );
            assert_eq!(model.clone().family(), expected);
        }
    }

    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = Temporary(std::env::temp_dir().join(format!(
        "openeq-family-fallback-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir(&directory.0).unwrap();
    // Only the classic archive exists: the requested replacement cannot be
    // confused with an actual loaded Luclin actor of the same model code.
    std::fs::copy(
        base.join("global_chr.s3d"),
        directory.0.join("global_chr.s3d"),
    )
    .unwrap();
    let fallback =
        CharacterLibrary::load_with_model_set(&directory.0, "", CharacterModelSet::Luclin).unwrap();
    let model = fallback.load_race(1, 0).unwrap();
    assert_eq!(model.code, "HUM");
    assert_eq!(model.family(), CharacterModelFamily::Classic);
}

#[cfg(unix)]
#[test]
#[ignore = "requires original Luclin assets; CPU only, creates disposable symlinks"]
fn missing_luclin_module_keeps_graceful_geometry_but_reports_unresolved_appearance() {
    use std::os::unix::fs::symlink;
    let source = openeq_assets::loader::default_client_dir().expect("original assets");
    struct Temporary(std::path::PathBuf);
    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let missing = Temporary(std::env::temp_dir().join(format!(
        "openeq-missing-appearance-{}-{unique}",
        std::process::id()
    )));
    std::fs::create_dir(&missing.0).unwrap();
    for entry in std::fs::read_dir(&source).unwrap() {
        let entry = entry.unwrap();
        if !entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with("lgequip")
        {
            symlink(entry.path(), missing.0.join(entry.file_name())).unwrap();
        }
    }
    for (path, expected) in [(&source, true), (&missing.0, false)] {
        let library =
            CharacterLibrary::load_with_model_set(path, "", CharacterModelSet::Luclin).unwrap();
        for appearance in [
            CharacterAppearance {
                hair_style: 1,
                ..Default::default()
            },
            CharacterAppearance {
                beard: 1,
                ..Default::default()
            },
        ] {
            let model = library
                .load_race_with_appearance(1, 0, &appearance)
                .unwrap();
            assert_eq!(model.appearance_resolved(), expected, "{appearance:?}");
            assert_eq!(model.family(), CharacterModelFamily::Luclin);
            assert!(!model.meshes.is_empty());
            assert!(
                model
                    .materials
                    .iter()
                    .flat_map(|m| &m.textures)
                    .all(|name| library.texture(name).is_some())
            );
            let bald = library.load_race(1, 0).unwrap();
            assert!(
                bald.appearance_resolved(),
                "failed clone poisoned cached default"
            );
            if !expected {
                assert_eq!(model.materials, bald.materials);
                assert_eq!(model.meshes.len(), bald.meshes.len());
                assert!(
                    model
                        .meshes
                        .iter()
                        .zip(&bald.meshes)
                        .all(|(a, b)| a.vertices == b.vertices && a.indices == b.indices)
                );
            }
        }
    }
}
