//! Window-ordered text input and recovery after native focus changes.
use crate::chat::ChatEditor;
use bevy::{
    input::ButtonState,
    prelude::{ButtonInput, Entity, KeyCode, MouseButton},
    window::{Ime, WindowEvent},
};
use std::collections::HashSet;

/// Bevy can omit its keyboard reset when focus loss and gain arrive in one
/// frame. macOS does not necessarily send releases for Command/Control then.
/// Reset on every loss and replay subsequent events so fresh input survives.
pub fn recover_focus_loss<'a>(
    window: Entity,
    events: impl IntoIterator<Item = &'a WindowEvent>,
    keys: &mut ButtonInput<KeyCode>,
    mouse: &mut ButtonInput<MouseButton>,
) {
    let mut lost = false;
    for event in events {
        match event {
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                keys.reset_all();
                mouse.reset_all();
                lost = true;
            }
            WindowEvent::KeyboardInput(event) if lost && event.window == window => {
                match event.state {
                    ButtonState::Pressed => keys.press(event.key_code),
                    ButtonState::Released => keys.release(event.key_code),
                }
            }
            WindowEvent::MouseButtonInput(event) if lost && event.window == window => {
                match event.state {
                    ButtonState::Pressed => mouse.press(event.button),
                    ButtonState::Released => mouse.release(event.button),
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct ChatInput {
    // Track modifiers in event order: a press, text and release can all arrive
    // in one frame, where ButtonInput only exposes the final held state.
    modifiers: ButtonInput<KeyCode>,
    // Physical keys belong to chat until released, even after submission.
    // Keeping this separate from Bevy's held state also catches OS repeats.
    captured_keys: HashSet<KeyCode>,
}

#[derive(Default, Debug)]
pub struct ChatInputResult {
    pub captured: bool,
    pub escape_handled: bool,
    pub submitted: Option<String>,
    pub scroll: i32,
}

impl ChatInput {
    pub fn reset(&mut self, editor: &mut ChatEditor) {
        self.modifiers.reset_all();
        self.captured_keys.clear();
        editor.cancel();
    }

    /// Run after routing this frame's window events and before gameplay reads
    /// physical keys. Only keys still owned by chat lose their held/edge state;
    /// unrelated fresh movement keys remain available immediately.
    pub fn suppress_captured_keys(&self, keys: &mut ButtonInput<KeyCode>) {
        for key in &self.captured_keys {
            keys.reset(*key);
        }
    }

    /// Consume the combined WindowEvent stream, preserving ordering between
    /// key presses, IME commits and focus changes within the same frame.
    pub fn event(
        &mut self,
        editor: &mut ChatEditor,
        window: Entity,
        event: &WindowEvent,
    ) -> ChatInputResult {
        let mut result = ChatInputResult::default();
        match event {
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.modifiers.reset_all();
                self.captured_keys.clear();
                editor.preedit.clear();
            }
            WindowEvent::Ime(Ime::Preedit {
                window: id, value, ..
            }) if *id == window && editor.active => {
                editor.preedit.clone_from(value);
                result.captured = true;
            }
            WindowEvent::Ime(Ime::Commit { window: id, value })
                if *id == window && editor.active =>
            {
                editor.insert(value);
                editor.preedit.clear();
                result.captured = true;
            }
            WindowEvent::Ime(Ime::Disabled { window: id }) if *id == window => {
                editor.preedit.clear();
            }
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if matches!(
                    event.key_code,
                    KeyCode::ControlLeft
                        | KeyCode::ControlRight
                        | KeyCode::SuperLeft
                        | KeyCode::SuperRight
                ) {
                    match event.state {
                        ButtonState::Pressed => self.modifiers.press(event.key_code),
                        ButtonState::Released => self.modifiers.release(event.key_code),
                    }
                }
                if event.state != ButtonState::Pressed {
                    self.captured_keys.remove(&event.key_code);
                    return result;
                }
                if !editor.active && self.captured_keys.contains(&event.key_code) {
                    // Suppress this held key without globally capturing the
                    // frame: unrelated gameplay input can arrive alongside it.
                    return result;
                }
                if editor.active {
                    self.captured_keys.insert(event.key_code);
                }
                let command = self.modifiers.get_pressed().next().is_some();
                let enter = matches!(event.key_code, KeyCode::Enter | KeyCode::NumpadEnter);
                // Holding Enter through submission/loading must not reopen an
                // invisible editor and steal movement in the destination zone.
                if enter && event.repeat {
                    result.captured = editor.active;
                    return result;
                }
                if !editor.active {
                    if enter {
                        editor.open("");
                    } else if event.text.as_deref() == Some("/") && !command && !event.repeat {
                        editor.open("/");
                    } else {
                        return result;
                    }
                    self.captured_keys.insert(event.key_code);
                    result.captured = true;
                    return result;
                }
                result.captured = true;
                if event.key_code == KeyCode::Escape {
                    editor.cancel();
                    result.escape_handled = true;
                    return result;
                }
                if !editor.preedit.is_empty() {
                    return result;
                }
                match event.key_code {
                    KeyCode::Enter | KeyCode::NumpadEnter => result.submitted = editor.submit(),
                    KeyCode::Backspace => editor.backspace(command),
                    KeyCode::Delete => editor.delete(),
                    KeyCode::ArrowLeft => editor.left(),
                    KeyCode::ArrowRight => editor.right(),
                    KeyCode::Home => editor.cursor = 0,
                    KeyCode::End => editor.cursor = editor.text.len(),
                    KeyCode::ArrowUp => editor.history(true),
                    KeyCode::ArrowDown => editor.history(false),
                    KeyCode::PageUp => result.scroll = 8,
                    KeyCode::PageDown => result.scroll = -8,
                    KeyCode::KeyU if command => {
                        editor.text.clear();
                        editor.cursor = 0;
                    }
                    _ if !command => {
                        if let Some(text) = &event.text {
                            editor.insert(text);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        input::{
            keyboard::{Key, KeyboardInput, NativeKey},
            mouse::MouseButtonInput,
        },
        window::WindowFocused,
    };

    fn window(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap()
    }

    fn keyboard(
        window: Entity,
        key_code: KeyCode,
        state: ButtonState,
        text: Option<&str>,
        repeat: bool,
    ) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyboardInput {
            key_code,
            logical_key: text.map_or(Key::Unidentified(NativeKey::Unidentified), |text| {
                Key::Character(text.into())
            }),
            state,
            text: text.map(Into::into),
            repeat,
            window,
        })
    }

    fn press(window: Entity, key: KeyCode, text: Option<&str>) -> WindowEvent {
        keyboard(window, key, ButtonState::Pressed, text, false)
    }

    fn release(window: Entity, key: KeyCode) -> WindowEvent {
        keyboard(window, key, ButtonState::Released, None, false)
    }

    fn focus(window: Entity, focused: bool) -> WindowEvent {
        WindowEvent::WindowFocused(WindowFocused { window, focused })
    }

    fn mouse_button(window: Entity, button: MouseButton, state: ButtonState) -> WindowEvent {
        WindowEvent::MouseButtonInput(MouseButtonInput {
            window,
            button,
            state,
        })
    }

    fn preedit(window: Entity, value: &str) -> WindowEvent {
        WindowEvent::Ime(Ime::Preedit {
            window,
            value: value.into(),
            cursor: Some((value.len(), value.len())),
        })
    }

    fn commit(window: Entity, value: &str) -> WindowEvent {
        WindowEvent::Ime(Ime::Commit {
            window,
            value: value.into(),
        })
    }

    fn route_keyboard_frame(
        input: &mut ChatInput,
        editor: &mut ChatEditor,
        window: Entity,
        keys: &mut ButtonInput<KeyCode>,
        events: &[WindowEvent],
    ) -> Vec<ChatInputResult> {
        // Bevy updates the physical resource before the application routes
        // ordered events; suppression must work with that final held state.
        keys.clear();
        for event in events {
            if let WindowEvent::KeyboardInput(event) = event {
                match event.state {
                    ButtonState::Pressed => keys.press(event.key_code),
                    ButtonState::Released => keys.release(event.key_code),
                }
            }
        }
        let results = events
            .iter()
            .map(|event| input.event(editor, window, event))
            .collect();
        input.suppress_captured_keys(keys);
        results
    }

    #[test]
    fn held_chat_key_stays_suppressed_after_submission_until_its_release() {
        let primary = window(1);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        let mut keys = ButtonInput::default();
        editor.open("");
        let submitted = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                press(primary, KeyCode::KeyW, Some("w")),
                press(primary, KeyCode::Enter, None),
            ],
        );
        assert_eq!(submitted[1].submitted.as_deref(), Some("w"));
        assert!(!editor.active);
        assert!(!keys.pressed(KeyCode::KeyW));
        assert!(!keys.just_pressed(KeyCode::KeyW));
        let stale_repeat = keyboard(
            primary,
            KeyCode::KeyW,
            ButtonState::Pressed,
            Some("w"),
            true,
        );
        let results = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                stale_repeat.clone(),
                press(primary, KeyCode::KeyD, Some("d")),
            ],
        );
        assert!(
            results
                .iter()
                .all(|result| !result.captured && result.submitted.is_none()),
            "a suppressed chat repeat must not block unrelated gameplay input"
        );
        assert!(!keys.pressed(KeyCode::KeyW));
        assert!(!keys.just_pressed(KeyCode::KeyW));
        assert!(keys.pressed(KeyCode::KeyD));
        assert!(keys.just_pressed(KeyCode::KeyD));
        // Another window cannot release a physical key still owned by chat.
        route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[release(window(2), KeyCode::KeyW), stale_repeat],
        );
        assert!(!keys.pressed(KeyCode::KeyW));
        assert!(keys.pressed(KeyCode::KeyD));
        // A genuine release followed by a new press in one frame is fresh
        // gameplay input, even though its final physical state is still held.
        let fresh = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                release(primary, KeyCode::KeyW),
                press(primary, KeyCode::KeyW, Some("w")),
            ],
        );
        assert!(fresh.iter().all(|result| !result.captured));
        assert!(keys.pressed(KeyCode::KeyW));
        assert!(keys.just_pressed(KeyCode::KeyW));
        assert!(keys.pressed(KeyCode::KeyD));
        assert!(editor.text.is_empty());
    }

    #[test]
    fn text_autorepeat_edits_chat_but_does_not_become_movement_after_escape() {
        let primary = window(1);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        let mut keys = ButtonInput::default();
        editor.open("");
        let repeat = keyboard(
            primary,
            KeyCode::KeyW,
            ButtonState::Pressed,
            Some("w"),
            true,
        );
        let results = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                press(primary, KeyCode::KeyW, Some("w")),
                repeat.clone(),
                repeat.clone(),
            ],
        );
        assert!(results.iter().all(|result| result.captured));
        assert_eq!(editor.text, "www");
        assert!(!keys.pressed(KeyCode::KeyW));
        let closed = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[press(primary, KeyCode::Escape, None), repeat],
        );
        assert!(closed[0].escape_handled);
        assert!(!closed[1].captured);
        assert!(!editor.active);
        assert!(!keys.pressed(KeyCode::KeyW));
        assert!(!keys.pressed(KeyCode::Escape));
        assert!(!keys.just_pressed(KeyCode::Escape));
    }

    #[test]
    fn focus_loss_and_zone_reset_clear_captured_key_ownership() {
        let primary = window(1);
        for zone_reset in [false, true] {
            let mut input = ChatInput::default();
            let mut editor = ChatEditor::default();
            let mut keys = ButtonInput::default();
            editor.open("");
            route_keyboard_frame(
                &mut input,
                &mut editor,
                primary,
                &mut keys,
                &[
                    press(primary, KeyCode::KeyW, Some("w")),
                    press(primary, KeyCode::Enter, None),
                ],
            );
            assert!(!keys.pressed(KeyCode::KeyW));
            if zone_reset {
                input.reset(&mut editor);
            } else {
                input.event(&mut editor, primary, &focus(primary, false));
                input.event(&mut editor, primary, &focus(primary, true));
            }
            // No release was delivered for the old W. New native input must
            // be usable after the reset rather than retaining that ownership.
            let results = route_keyboard_frame(
                &mut input,
                &mut editor,
                primary,
                &mut keys,
                &[
                    press(primary, KeyCode::KeyW, Some("w")),
                    press(primary, KeyCode::KeyA, Some("a")),
                ],
            );
            assert!(results.iter().all(|result| !result.captured));
            assert!(keys.pressed(KeyCode::KeyW));
            assert!(keys.just_pressed(KeyCode::KeyW));
            assert!(keys.pressed(KeyCode::KeyA));
            assert!(!editor.active);
        }
    }

    #[test]
    fn losing_and_regaining_focus_in_one_batch_clears_stale_command() {
        let primary = window(1);
        for modifier in [
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
        ] {
            let mut input = ChatInput::default();
            let mut editor = ChatEditor::default();
            editor.open("draft ");
            input.event(&mut editor, primary, &press(primary, modifier, None));
            input.event(&mut editor, primary, &preedit(primary, "uncommitted"));
            // Native focus changes and resumed typing can all share a frame;
            // the OS need not provide a matching modifier release.
            for event in [
                focus(primary, false),
                focus(primary, true),
                press(primary, KeyCode::KeyH, Some("hello")),
            ] {
                assert!(
                    input
                        .event(&mut editor, primary, &event)
                        .submitted
                        .is_none()
                );
            }
            assert!(editor.active);
            assert!(editor.preedit.is_empty());
            assert_eq!(
                editor.text, "draft hello",
                "stale {modifier:?} blocked resumed typing"
            );
        }
    }

    #[test]
    fn focus_recovery_clears_stale_buttons_and_replays_fresh_presses_and_releases() {
        let primary = window(1);
        let mut keys = ButtonInput::default();
        let mut mouse = ButtonInput::default();
        keys.press(KeyCode::SuperLeft);
        keys.press(KeyCode::KeyS);
        mouse.press(MouseButton::Left);
        let events = [
            focus(primary, false),
            focus(primary, true),
            press(primary, KeyCode::KeyW, Some("w")),
            press(primary, KeyCode::KeyA, Some("a")),
            release(primary, KeyCode::KeyA),
            mouse_button(primary, MouseButton::Right, ButtonState::Pressed),
            mouse_button(primary, MouseButton::Middle, ButtonState::Pressed),
            mouse_button(primary, MouseButton::Middle, ButtonState::Released),
        ];
        recover_focus_loss(primary, &events, &mut keys, &mut mouse);
        assert!(!keys.pressed(KeyCode::SuperLeft));
        assert!(!keys.just_pressed(KeyCode::SuperLeft));
        assert!(!keys.pressed(KeyCode::KeyS));
        assert!(keys.pressed(KeyCode::KeyW));
        assert!(keys.just_pressed(KeyCode::KeyW));
        assert!(!keys.pressed(KeyCode::KeyA));
        assert!(keys.just_released(KeyCode::KeyA));
        assert!(!mouse.pressed(MouseButton::Left));
        assert!(!mouse.just_pressed(MouseButton::Left));
        assert!(mouse.pressed(MouseButton::Right));
        assert!(mouse.just_pressed(MouseButton::Right));
        assert!(!mouse.pressed(MouseButton::Middle));
        assert!(mouse.just_released(MouseButton::Middle));
    }

    #[test]
    fn focus_recovery_ignores_other_windows_and_preserves_state_without_primary_loss() {
        let primary = window(1);
        let other = window(2);
        let mut keys = ButtonInput::default();
        let mut mouse = ButtonInput::default();
        keys.press(KeyCode::KeyW);
        mouse.press(MouseButton::Right);
        recover_focus_loss(
            primary,
            &[focus(other, false), focus(primary, true)],
            &mut keys,
            &mut mouse,
        );
        assert!(keys.pressed(KeyCode::KeyW));
        assert!(mouse.pressed(MouseButton::Right));
        recover_focus_loss(
            primary,
            &[
                focus(primary, false),
                press(other, KeyCode::KeyA, Some("a")),
                mouse_button(other, MouseButton::Left, ButtonState::Pressed),
                press(primary, KeyCode::KeyW, Some("w")),
                mouse_button(primary, MouseButton::Right, ButtonState::Pressed),
                focus(other, false),
                release(other, KeyCode::KeyW),
                mouse_button(other, MouseButton::Right, ButtonState::Released),
            ],
            &mut keys,
            &mut mouse,
        );
        assert!(keys.pressed(KeyCode::KeyW));
        assert!(!keys.pressed(KeyCode::KeyA));
        assert!(mouse.pressed(MouseButton::Right));
        assert!(!mouse.pressed(MouseButton::Left));
    }

    #[test]
    fn ime_commit_before_enter_in_one_batch_submits_the_complete_message() {
        let primary = window(1);
        for enter in [KeyCode::Enter, KeyCode::NumpadEnter] {
            let mut input = ChatInput::default();
            let mut editor = ChatEditor::default();
            editor.open("hello ");
            let events = [
                preedit(primary, "せかい"),
                commit(primary, "世界🌍"),
                press(primary, enter, Some("\r")),
            ];
            let results: Vec<_> = events
                .iter()
                .map(|event| input.event(&mut editor, primary, event))
                .collect();
            assert!(results.iter().all(|result| result.captured));
            assert_eq!(
                results
                    .iter()
                    .filter_map(|result| result.submitted.as_deref())
                    .collect::<Vec<_>>(),
                ["hello 世界🌍"]
            );
            assert!(!editor.active);
            assert!(editor.text.is_empty());
            assert!(editor.preedit.is_empty());
        }
    }

    #[test]
    fn enter_while_composing_does_not_submit_or_discard_the_composition() {
        let primary = window(1);
        for enter in [KeyCode::Enter, KeyCode::NumpadEnter] {
            let mut input = ChatInput::default();
            let mut editor = ChatEditor::default();
            editor.open("say ");
            input.event(&mut editor, primary, &preedit(primary, "にほん"));
            let result = input.event(&mut editor, primary, &press(primary, enter, Some("\r")));
            assert!(result.captured);
            assert!(result.submitted.is_none());
            assert!(editor.active);
            assert_eq!(editor.text, "say ");
            assert_eq!(editor.preedit, "にほん");
            input.event(&mut editor, primary, &commit(primary, "日本"));
            assert_eq!(
                input
                    .event(&mut editor, primary, &press(primary, enter, Some("\r")))
                    .submitted
                    .as_deref(),
                Some("say 日本")
            );
        }
    }

    #[test]
    fn repeated_enter_never_reopens_after_submit_or_reset() {
        let primary = window(1);
        for enter in [KeyCode::Enter, KeyCode::NumpadEnter] {
            let mut input = ChatInput::default();
            let mut editor = ChatEditor::default();
            input.event(&mut editor, primary, &press(primary, enter, None));
            input.event(
                &mut editor,
                primary,
                &press(primary, KeyCode::KeyH, Some("hello")),
            );
            let repeated = keyboard(primary, enter, ButtonState::Pressed, Some("\r"), true);
            let held_while_editing = input.event(&mut editor, primary, &repeated);
            assert!(held_while_editing.captured);
            assert!(held_while_editing.submitted.is_none());
            assert_eq!(editor.text, "hello");
            assert_eq!(
                input
                    .event(&mut editor, primary, &press(primary, enter, None))
                    .submitted
                    .as_deref(),
                Some("hello")
            );
            for _ in 0..8 {
                let result = input.event(&mut editor, primary, &repeated);
                assert!(!result.captured);
                assert!(result.submitted.is_none());
                assert!(!editor.active);
            }
            input.event(&mut editor, primary, &release(primary, enter));
            input.event(&mut editor, primary, &press(primary, enter, None));
            assert!(editor.active);
            input.event(
                &mut editor,
                primary,
                &press(primary, KeyCode::SuperLeft, None),
            );
            input.event(&mut editor, primary, &preedit(primary, "draft"));
            input.reset(&mut editor);
            assert!(editor.preedit.is_empty());
            for _ in 0..8 {
                let result = input.event(&mut editor, primary, &repeated);
                assert!(!result.captured);
                assert!(result.submitted.is_none());
                assert!(!editor.active);
            }
            input.event(
                &mut editor,
                primary,
                &press(primary, KeyCode::Slash, Some("/")),
            );
            input.event(
                &mut editor,
                primary,
                &press(primary, KeyCode::KeyS, Some("say hi")),
            );
            assert_eq!(editor.text, "/say hi", "reset retained stale Command");
            assert!(editor.active);
        }
    }

    #[test]
    fn command_text_and_release_are_processed_in_event_order() {
        let primary = window(1);
        for modifier in [
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
        ] {
            let mut input = ChatInput::default();
            let mut editor = ChatEditor::default();
            editor.open("");
            for event in [
                press(primary, KeyCode::KeyA, Some("before ")),
                press(primary, modifier, None),
                press(primary, KeyCode::KeyC, Some("must not appear")),
                release(primary, modifier),
                press(primary, KeyCode::KeyB, Some("after")),
            ] {
                assert!(
                    input
                        .event(&mut editor, primary, &event)
                        .submitted
                        .is_none()
                );
            }
            assert_eq!(
                editor.text, "before after",
                "incorrect {modifier:?} ordering"
            );
        }
    }

    #[test]
    fn chat_ignores_foreign_keyboard_ime_and_focus_events() {
        let primary = window(1);
        let other = window(2);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        assert!(
            !input
                .event(&mut editor, primary, &press(other, KeyCode::Enter, None))
                .captured
        );
        assert!(!editor.active);
        editor.open("draft");
        input.event(&mut editor, primary, &preedit(primary, "pending"));
        input.event(
            &mut editor,
            primary,
            &press(primary, KeyCode::SuperLeft, None),
        );
        for event in [
            focus(other, false),
            release(other, KeyCode::SuperLeft),
            press(other, KeyCode::Escape, None),
            preedit(other, "foreign"),
            commit(other, "foreign"),
            WindowEvent::Ime(Ime::Disabled { window: other }),
            press(other, KeyCode::Enter, None),
        ] {
            let result = input.event(&mut editor, primary, &event);
            assert!(!result.captured);
            assert!(!result.escape_handled);
            assert!(result.submitted.is_none());
            assert_eq!(result.scroll, 0);
            assert!(editor.active);
            assert_eq!(editor.text, "draft");
            assert_eq!(editor.preedit, "pending");
        }
        input.event(
            &mut editor,
            primary,
            &WindowEvent::Ime(Ime::Disabled { window: primary }),
        );
        input.event(
            &mut editor,
            primary,
            &press(primary, KeyCode::KeyC, Some("suppressed")),
        );
        assert_eq!(
            editor.text, "draft",
            "foreign release cleared the primary modifier"
        );
        input.event(&mut editor, primary, &release(primary, KeyCode::SuperLeft));
        input.event(
            &mut editor,
            primary,
            &press(other, KeyCode::SuperRight, None),
        );
        input.event(
            &mut editor,
            primary,
            &press(primary, KeyCode::KeyA, Some(" accepted")),
        );
        assert_eq!(
            editor.text, "draft accepted",
            "foreign modifier blocked primary typing"
        );
    }

    #[test]
    fn ordinary_unicode_keyboard_text_preserves_characters_and_cursor_boundaries() {
        let primary = window(1);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        input.event(&mut editor, primary, &press(primary, KeyCode::Enter, None));
        for text in ["h", "é", "猫", "👋", "e\u{301}"] {
            let result = input.event(
                &mut editor,
                primary,
                &press(primary, KeyCode::KeyA, Some(text)),
            );
            assert!(result.captured);
            assert!(result.submitted.is_none());
            assert_eq!(editor.cursor, editor.text.len());
            assert!(editor.text.is_char_boundary(editor.cursor));
        }
        assert_eq!(editor.text, "hé猫👋e\u{301}");
        assert_eq!(
            input
                .event(&mut editor, primary, &press(primary, KeyCode::Enter, None))
                .submitted
                .as_deref(),
            Some("hé猫👋e\u{301}")
        );
    }
}
