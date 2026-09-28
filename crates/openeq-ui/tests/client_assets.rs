//! Opt-in tests read a user's existing installation; no proprietary assets are
//! copied into this repository. Run with EQ_UI_DIR=... cargo test -p openeq-ui
//! --test client_assets -- --ignored --nocapture.
use openeq_ui::{DrawCommand, Rect, UiBindings, UiDocument};
use std::path::PathBuf;

fn directory() -> PathBuf {
    std::env::var_os("EQ_UI_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("set EQ_UI_DIR"))
                .join("EverQuest/uifiles/default")
        })
}

#[test]
#[ignore = "requires original EverQuest UI assets"]
fn actual_player_and_character_windows() {
    let document = UiDocument::load(directory(), "EQUI.xml").unwrap();
    assert!(document.source_files.len() > 50);
    let mut bindings = UiBindings::default();
    bindings.eq_text.insert("19".into(), "73".into());
    bindings.eq_gauges.insert("1".into(), 0.73);
    bindings.widget_mut("Player_HP").text = Some("Test adventurer".into());
    let frame = document
        .window("PlayerWindow")
        .unwrap()
        .layout(Rect::new(0., 0., 1280., 720.), &bindings);
    assert_eq!(frame.bounds, Rect::new(3., 0., 160., 95.));
    assert!(
        frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "73"))
    );
    assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Image {texture, ..} if texture.file_name().unwrap() == "window_pieces01.tga")));
    assert_images_exist(&frame.commands);
    let characters = document
        .window("CharacterListWnd")
        .unwrap()
        .layout(Rect::new(0., 0., 1280., 720.), &bindings);
    assert!(!characters.hit_targets.is_empty());
    eprintln!(
        "{} files, {} definitions, {} animations, {} player draw commands; warnings: {:?}",
        document.source_files.len(),
        document.definitions.len(),
        document.animations.len(),
        frame.commands.len(),
        frame.warnings
    );
}

#[test]
#[ignore = "requires original EverQuest UI assets"]
fn actual_login_window_has_controls_and_atlas_images() {
    let document = UiDocument::load(directory(), "EQLSUI.xml").unwrap();
    let mut bindings = UiBindings::default();
    bindings.widget_mut("UsernameEdit").text = Some("OpenEQ".into());
    let password = bindings.widget_mut("PasswordEdit");
    password.text = Some("secret".into());
    password.password = true;
    let frame = document
        .window("connect")
        .unwrap()
        .layout(Rect::new(0., 0., 1280., 720.), &bindings);
    assert_eq!(frame.bounds, Rect::new(0., 0., 640., 480.));
    assert_eq!(
        frame.hit_test([340., 295.]).unwrap().screen_id,
        "ConnectButton"
    );
    assert!(
        frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "OpenEQ"))
    );
    assert!(
        frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "••••••"))
    );
    assert!(
        !frame
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "secret"))
    );
    assert_images_exist(&frame.commands);
    eprintln!(
        "{} files, {} login draw commands; warnings: {:?}",
        document.source_files.len(),
        frame.commands.len(),
        frame.warnings
    );
}

fn assert_images_exist(commands: &[DrawCommand]) {
    for command in commands {
        if let DrawCommand::Image {
            texture, source, ..
        } = command
        {
            assert!(texture.exists(), "missing texture {}", texture.display());
            assert!(source.width >= 0. && source.height >= 0.);
        }
    }
}
