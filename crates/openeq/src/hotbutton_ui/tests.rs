use super::*;
use crate::{
    gameplay_ui::{UiAction, WindowStack, valid_window_id},
    hud::{Hud, HudState},
};
use openeq_ui::{DrawCommand, UiDocument};

fn fixture() -> UiHotbuttons {
    let token = HotbuttonToken {
        revision: 17,
        zone_generation: 3,
    };
    UiHotbuttons {
        token,
        open: true,
        enabled: true,
        slots: std::array::from_fn(|slot| match slot {
            0 => Some(UiHotbutton {
                label: "Attack".into(),
                command: "/attack".into(),
            }),
            1 => Some(UiHotbutton {
                label: "Hail".into(),
                command: "/hail".into(),
            }),
            2 => Some(UiHotbutton {
                label: "Flame".into(),
                command: "/cast 1".into(),
            }),
            11 => Some(UiHotbutton {
                label: "Inspect & use".into(),
                command: "/useitem".into(),
            }),
            _ => None,
        }),
        ..Default::default()
    }
}
fn editor(token: HotbuttonToken) -> UiHotbuttonEditor {
    UiHotbuttonEditor {
        token,
        slot: 11,
        label: "Inspect & use".into(),
        command: "/useitem".into(),
        focus: HotbuttonField::Command,
        ..Default::default()
    }
}
fn hud() -> Hud {
    let mut xml = String::from(
        r#"<XML><Screen item="HotButtonWnd"><Pieces>TileLayoutBox:unused</Pieces></Screen><TileLayoutBox item="unused"/><Screen item="SocialEditWnd"><Pieces>SEW_Line1Input</Pieces></Screen><Editbox item="SEW_Line1Input"><Text>hidden social command</Text></Editbox><Editbox item="SEW_NameInput"/><Editbox item="SEW_Line0Input"/><Button item="SEW_Clear_Button"/><Button item="SEW_Accept_Button"/>"#,
    );
    for number in 1..=12 {
        xml.push_str(&format!(
            "<HotButton item=\"HB_Button{number}\"><Text>{number}</Text></HotButton>"
        ));
    }
    xml.push_str("</XML>");
    Hud {
        ui: UiDocument::from_xml(&xml).unwrap(),
        initial_bindings: UiBindings::default(),
    }
}
fn hit(item: &str, window: &str) -> HitTarget {
    HitTarget {
        item: item.into(),
        screen_id: item.into(),
        window_id: Some(window.into()),
        kind: "HotbuttonControl".into(),
        enabled: true,
        rect: Rect::new(0., 0., 20., 20.),
        tooltip: None,
    }
}

#[test]
fn actions_validate_token_slot_window_and_disabled_state() {
    let token = fixture().token;
    for (slot, kind) in [
        ("slot:0", HotbuttonActionKind::Slot(0)),
        ("slot:11", HotbuttonActionKind::Slot(11)),
        ("close", HotbuttonActionKind::CloseBar),
    ] {
        assert_eq!(
            HotbuttonAction::from_hit(&hit(&id(token, slot), "hotbuttons")),
            Some(HotbuttonAction { token, kind })
        );
    }
    for (suffix, kind) in [
        ("label", HotbuttonActionKind::Focus(HotbuttonField::Label)),
        (
            "command",
            HotbuttonActionKind::Focus(HotbuttonField::Command),
        ),
        ("clear", HotbuttonActionKind::Clear),
        ("save", HotbuttonActionKind::Save),
        ("cancel", HotbuttonActionKind::Cancel),
    ] {
        let h = hit(
            &id(token, &format!("editor:11:{suffix}")),
            "hotbutton_editor",
        );
        assert_eq!(
            HotbuttonAction::from_hit(&h),
            Some(HotbuttonAction { token, kind })
        );
        assert_eq!(hotbutton_editor_hit_identity(&h), Some((token, 11)));
        assert!(UiAction::from_hit(&h).is_none());
    }
    for item in [
        "hotbutton:17:3:slot:12",
        "hotbutton:17:3:slot:-1",
        "hotbutton:x:3:slot:0",
        "hotbutton:17:x:close",
        "hotbutton:17:3:editor:12:save",
        "hotbutton:17:3:editor:0:train",
        "hotbutton:17:3:editor:0:save:extra",
    ] {
        for window in ["hotbuttons", "hotbutton_editor", "inventory"] {
            assert!(HotbuttonAction::from_hit(&hit(item, window)).is_none());
        }
    }
    let mut disabled = hit(&id(token, "editor:0:save"), "hotbutton_editor");
    disabled.enabled = false;
    assert!(HotbuttonAction::from_hit(&disabled).is_none());
    assert_eq!(hotbutton_editor_hit_identity(&disabled), Some((token, 0)));
    let body = hit(&id(token, "editor:0:body"), "hotbutton_editor");
    assert!(HotbuttonAction::from_hit(&body).is_none());
    assert_eq!(hotbutton_editor_hit_identity(&body), Some((token, 0)));
    assert!(hotbutton_window_hit(&body));
}

#[test]
fn twelve_slots_wrap_keep_identity_and_avoid_generic_dispatch() {
    let hud = hud();
    let mut bar = fixture();
    for viewport in [[1000, 650], [350, 400], [240, 400]] {
        let frame = hud.gameplay_frame(
            viewport,
            &HudState::default(),
            &GameHudState {
                hotbuttons: Some(bar.clone()),
                ..Default::default()
            },
        );
        let slots: Vec<_> = frame
            .hit_targets
            .iter()
            .filter_map(|hit| {
                HotbuttonAction::from_hit(hit)
                    .filter(|a| matches!(a.kind, HotbuttonActionKind::Slot(_)))
                    .map(|action| (hit, action))
            })
            .collect();
        assert_eq!(slots.len(), 12);
        for (index, (hit, action)) in slots.iter().enumerate() {
            assert_eq!(action.kind, HotbuttonActionKind::Slot(index as u8));
            assert_eq!(
                frame
                    .hit_test([hit.rect.x + 3., hit.rect.y + 3.])
                    .and_then(HotbuttonAction::from_hit),
                Some(action.clone())
            );
            assert!(UiAction::from_hit(hit).is_none());
            assert!(
                hit.rect.right() <= viewport[0] as f32 && hit.rect.bottom() <= viewport[1] as f32
            );
        }
        for (index, (left, _)) in slots.iter().enumerate() {
            for (right, _) in slots.iter().skip(index + 1) {
                assert!(left.rect.intersect(right.rect).is_empty());
            }
        }
        assert!(
            frame
                .hit_targets
                .iter()
                .all(|hit| !hit.item.starts_with("HB_"))
        );
        assert!(valid_window_id("hotbuttons"));
    }
    bar.enabled = false;
    let frame = hud.gameplay_frame(
        [900, 650],
        &HudState::default(),
        &GameHudState {
            hotbuttons: Some(bar),
            ..Default::default()
        },
    );
    assert!(
        !frame
            .hit_targets
            .iter()
            .filter_map(HotbuttonAction::from_hit)
            .any(|a| matches!(a.kind, HotbuttonActionKind::Slot(_)))
    );
}

#[test]
fn editor_has_only_two_fields_and_modal_body_without_implicit_save() {
    let hud = hud();
    let mut bar = fixture();
    bar.editor = Some(editor(bar.token));
    let mut state = GameHudState {
        hotbuttons: Some(bar),
        inventory_open: true,
        ..Default::default()
    };
    state
        .window_positions
        .insert("hotbutton_editor".into(), [30., 60.]);
    state
        .window_positions
        .insert("inventory".into(), [30., 60.]);
    let mut stack = WindowStack::default();
    let frame = hud.gameplay_frame_with_windows(
        [900, 650],
        &HudState::default(),
        &state,
        vec![],
        &mut stack,
    );
    let fields: Vec<_> = frame
        .hit_targets
        .iter()
        .filter(|hit| hit.kind == "HotbuttonField")
        .collect();
    assert_eq!(fields.len(), 2);
    for field in fields {
        assert!(matches!(
            HotbuttonAction::from_hit(field).unwrap().kind,
            HotbuttonActionKind::Focus(_)
        ));
        assert!(UiAction::from_hit(field).is_none());
    }
    assert!(valid_window_id("hotbutton_editor"));
    assert_eq!(
        frame.hit_test([50., 245.]).unwrap().window_id.as_deref(),
        Some("hotbutton_editor")
    );
    assert!(
        frame
            .hit_targets
            .iter()
            .filter_map(HotbuttonAction::from_hit)
            .any(|action| action.kind == HotbuttonActionKind::Cancel)
    );
    assert!(
        !frame
            .hit_targets
            .iter()
            .any(|hit| hit.item.starts_with("SEW_") || hit.item == "game:close:hotbutton_editor")
    );
    assert!(
        !frame
            .commands
            .iter()
            .any(|cmd| matches!(cmd,DrawCommand::Text{text,..}if text=="hidden social command"))
    );
    assert!(
        !frame
            .hit_targets
            .iter()
            .filter_map(HotbuttonAction::from_hit)
            .any(|action| matches!(action.kind, HotbuttonActionKind::Slot(_)))
    );
    state.hotbuttons.as_mut().unwrap().editor = None;
    let frame = hud.gameplay_frame([900, 650], &HudState::default(), &state);
    assert!(
        !frame
            .hit_targets
            .iter()
            .any(|hit| hit.window_id.as_deref() == Some("hotbutton_editor"))
    );
}

#[test]
fn editor_caret_selection_and_error_remain_visible_with_utf8() {
    let hud = hud();
    let mut bar = fixture();
    let mut draft = editor(bar.token);
    draft.label = "Éowyn".into();
    draft.label_cursor = Some(1);
    draft.focus = HotbuttonField::Label;
    draft.label_selected = true;
    draft.error = Some("Use one supported slash command.".into());
    bar.editor = Some(draft);
    let state = GameHudState {
        hotbuttons: Some(bar),
        ..Default::default()
    };
    let frame = hud.gameplay_frame([350, 400], &HudState::default(), &state);
    assert!(
        frame
            .commands
            .iter()
            .any(|cmd| matches!(cmd,DrawCommand::Text{text,..}if text=="|Éowyn"))
    );
    assert!(frame.commands.iter().any(
        |cmd| matches!(cmd,DrawCommand::Text{text,..}if text=="Use one supported slash command.")
    ));
    assert!(
        frame
            .commands
            .iter()
            .any(|cmd| matches!(cmd,DrawCommand::Fill{color,..}if *color==[43,69,107,240]))
    );
    let controls: Vec<_> = frame
        .hit_targets
        .iter()
        .filter_map(|hit| HotbuttonAction::from_hit(hit).map(|action| (hit, action)))
        .filter(|(_, a)| {
            matches!(
                a.kind,
                HotbuttonActionKind::Save
                    | HotbuttonActionKind::Clear
                    | HotbuttonActionKind::Cancel
            )
        })
        .collect();
    for (hit, _) in controls {
        assert!(hit.rect.bottom() <= 400.);
    }
}

#[test]
#[ignore = "requires original UI assets and GPU; optional OPENEQ_UI_CAPTURE_DIR"]
fn original_hotbutton_bar_and_editor_normal_retina_and_narrow() {
    let base = std::env::var_os("EQ_UI_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
        });
    let hud = Hud::load(base).unwrap();
    let resources = HudState {
        character: "Adventurer".into(),
        hp: 1.,
        ..Default::default()
    };
    for scale in [1, 2] {
        let mut renderer =
            openeq_render::Renderer::new_headless(1000 * scale, 650 * scale).unwrap();
        for mode in [
            "bar",
            "empty",
            "hover",
            "pressed",
            "disabled",
            "editor",
            "selected",
            "error",
            "narrow-bar",
            "narrow-editor",
            "narrow-error",
        ] {
            let viewport = if mode.starts_with("narrow") {
                [350, 400]
            } else {
                [1000, 650]
            };
            renderer.resize(viewport[0] * scale, viewport[1] * scale);
            let mut bar = fixture();
            if mode == "empty" {
                bar.slots = std::array::from_fn(|_| None);
            }
            if mode == "disabled" {
                bar.enabled = false;
            }
            if mode == "pressed" {
                bar.pressed = Some(HotbuttonAction {
                    token: bar.token,
                    kind: HotbuttonActionKind::Slot(0),
                });
            }
            if mode.contains("editor") || mode.contains("error") || mode == "selected" {
                let mut draft = editor(bar.token);
                if mode == "selected" {
                    draft.command_selected = true;
                    draft.command = "/tell Éowyn Welcome to the Plane of Knowledge!".into();
                }
                if mode.contains("error") {
                    draft.command = "/pause 5".into();
                    draft.error = Some(
                        "This command is not supported. Enter one slash command, such as /cast 1."
                            .into(),
                    );
                }
                bar.editor = Some(draft);
            }
            let mut state = GameHudState {
                hotbuttons: Some(bar),
                ..Default::default()
            };
            if matches!(mode, "hover" | "pressed") {
                let initial = hud.gameplay_frame(viewport, &resources, &state);
                let slot = initial
                    .hit_targets
                    .iter()
                    .find(|hit| hit.item == "hotbutton:17:3:slot:0")
                    .unwrap();
                state.pointer = Some([slot.rect.x + 10., slot.rect.y + 10.]);
            }
            let frame = hud.gameplay_frame(viewport, &resources, &state);
            assert!(frame.warnings.is_empty(), "{mode}: {:?}", frame.warnings);
            assert!(
                frame
                    .hit_targets
                    .iter()
                    .all(|hit| !hit.item.starts_with("SEW_") && !hit.item.starts_with("HB_"))
            );
            renderer.set_ui_scaled(&frame, scale as f32);
            renderer.render_ui();
            if let Some(directory) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                image::save_buffer(
                    std::path::PathBuf::from(directory)
                        .join(format!("hotbuttons-{mode}-{scale}x.png")),
                    &pixels,
                    width,
                    height,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }
}
