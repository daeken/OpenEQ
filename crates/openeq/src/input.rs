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
    // Only the current editor session may accept repeats from its held keys.
    session_keys: HashSet<KeyCode>,
    // Keys whose whole-frame physical state needs ordered reconstruction.
    handoff_replay: HashSet<KeyCode>,
    // Empty preedit is often delivered immediately before Commit, so it does
    // not end ownership of the native composition.
    composing: bool,
    cancelled_composition: bool,
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
        self.session_keys.clear();
        self.handoff_replay.clear();
        self.composing = false;
        self.cancelled_composition = false;
        editor.cancel();
    }

    fn retire_session(&mut self) {
        for key in self.session_keys.drain() {
            self.modifiers.reset(key);
        }
    }

    /// Observe releases before another input owner can consume them. Presses
    /// still belong to the normal chat route, preserving retired-key ownership.
    pub fn observe_modifiers(&mut self, event: &WindowEvent, window: Entity) {
        match event {
            WindowEvent::KeyboardInput(event)
                if event.window == window
                    && event.state == ButtonState::Released
                    && matches!(
                        event.key_code,
                        KeyCode::ControlLeft
                            | KeyCode::ControlRight
                            | KeyCode::SuperLeft
                            | KeyCode::SuperRight
                    ) =>
            {
                self.modifiers.reset(event.key_code);
            }
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.modifiers.reset_all();
            }
            _ => {}
        }
    }

    fn cancel_composition(&mut self, editor: &mut ChatEditor) {
        self.cancelled_composition |= self.composing || !editor.preedit.is_empty();
        self.composing = false;
        editor.preedit.clear();
    }

    /// Retire chat before handing native input to another local editor. Old
    /// physical keys stay captured until released, and late IME commits cannot
    /// become text in the other field.
    pub fn cancel_for_handoff(&mut self, editor: &mut ChatEditor) {
        self.cancel_composition(editor);
        editor.cancel();
        self.retire_session();
    }

    /// Captured-only routing while another editor owns input. Unlike route_event
    /// this never opens chat, edits a draft or submits a command.
    pub fn filter_handoff_event(
        &mut self,
        event: &WindowEvent,
        window: Entity,
        keys: &mut ButtonInput<KeyCode>,
    ) -> bool {
        self.observe_modifiers(event, window);
        self.retire_session();
        match event {
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if self.captured_keys.contains(&event.key_code) {
                    keys.reset(event.key_code);
                    self.handoff_replay.insert(event.key_code);
                    if event.state == ButtonState::Released {
                        self.captured_keys.remove(&event.key_code);
                        self.modifiers.reset(event.key_code);
                    }
                    return true;
                }
                if self.handoff_replay.contains(&event.key_code) {
                    match event.state {
                        ButtonState::Pressed => keys.press(event.key_code),
                        ButtonState::Released => keys.release(event.key_code),
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit { window: id, .. })
                if *id == window && self.cancelled_composition =>
            {
                self.cancelled_composition = false;
                return true;
            }
            WindowEvent::Ime(Ime::Preedit {
                window: id, value, ..
            }) if *id == window && !value.is_empty() => {
                self.cancelled_composition = false;
            }
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.captured_keys.clear();
                self.modifiers.reset_all();
                for key in &self.handoff_replay {
                    keys.reset(*key);
                }
            }
            _ => {}
        }
        false
    }

    /// Begin once before routing a frame's native events. Bevy has already
    /// applied the entire batch, including releases after old chat repeats.
    pub fn begin_handoff_frame(&mut self, keys: &mut ButtonInput<KeyCode>) {
        self.handoff_replay.clone_from(&self.captured_keys);
        self.suppress_captured_keys(keys);
    }

    /// Route ordered input and rebuild physical state only for chat-owned
    /// keys. A release removes ownership but cannot leave a gameplay edge;
    /// a subsequent fresh press in the same batch can enter gameplay normally.
    /// Call after account-UI filtering, once for each remaining native event.
    pub fn route_event(
        &mut self,
        editor: &mut ChatEditor,
        window: Entity,
        event: &WindowEvent,
        keys: &mut ButtonInput<KeyCode>,
    ) -> ChatInputResult {
        let owned_before = matches!(event, WindowEvent::KeyboardInput(event)
            if event.window == window && self.captured_keys.contains(&event.key_code));
        let result = self.event(editor, window, event);
        match event {
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if owned_before || self.captured_keys.contains(&event.key_code) {
                    self.handoff_replay.insert(event.key_code);
                    keys.reset(event.key_code);
                } else if self.handoff_replay.contains(&event.key_code) {
                    match event.state {
                        ButtonState::Pressed => keys.press(event.key_code),
                        ButtonState::Released => keys.release(event.key_code),
                    }
                }
            }
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                for key in &self.handoff_replay {
                    keys.reset(*key);
                }
            }
            _ => {}
        }
        result
    }

    /// Run after routing this frame's window events and before gameplay reads
    /// physical keys. Only keys still owned by chat lose their held/edge state;
    /// unrelated fresh movement keys remain available immediately. Pair this
    /// with begin_handoff_frame/route_event to handle keys released this frame.
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
        self.observe_modifiers(event, window);
        // Pointer handlers can cancel the editor before forwarding this event.
        if !editor.active {
            self.cancel_composition(editor);
            self.retire_session();
        }
        match event {
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.modifiers.reset_all();
                self.captured_keys.clear();
                self.session_keys.clear();
                self.cancel_composition(editor);
            }
            WindowEvent::Ime(Ime::Preedit {
                window: id, value, ..
            }) if *id == window => {
                if !value.is_empty() {
                    self.cancelled_composition = false;
                }
                if editor.active {
                    self.composing |= !value.is_empty();
                    editor.preedit.clone_from(value);
                    result.captured = true;
                }
            }
            WindowEvent::Ime(Ime::Commit { window: id, .. })
                if *id == window && self.cancelled_composition =>
            {
                self.cancelled_composition = false;
                result.captured = true;
            }
            WindowEvent::Ime(Ime::Commit { window: id, value })
                if *id == window && editor.active =>
            {
                editor.insert(value);
                editor.preedit.clear();
                self.composing = false;
                result.captured = true;
            }
            WindowEvent::Ime(Ime::Disabled { window: id }) if *id == window => {
                self.cancel_composition(editor);
            }
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if event.state == ButtonState::Pressed
                    && self.captured_keys.contains(&event.key_code)
                    && !self.session_keys.contains(&event.key_code)
                {
                    // A previous editor's key remains owned until release,
                    // including when a different editor session is now open.
                    return result;
                }
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
                    self.session_keys.remove(&event.key_code);
                    return result;
                }
                if editor.active {
                    self.captured_keys.insert(event.key_code);
                    self.session_keys.insert(event.key_code);
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
                    self.session_keys.insert(event.key_code);
                    result.captured = true;
                    return result;
                }
                result.captured = true;
                if event.key_code == KeyCode::Escape {
                    self.cancel_composition(editor);
                    editor.cancel();
                    self.retire_session();
                    result.escape_handled = true;
                    return result;
                }
                if self.composing || !editor.preedit.is_empty() {
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
                if !editor.active {
                    self.retire_session();
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
    use crate::{
        hotbutton_input::{HotbuttonInput, HotbuttonInputContext},
        hotbutton_interaction::HotbuttonState,
        hotbutton_ui::HotbuttonActionKind,
    };
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

    #[derive(Default)]
    struct EditorOwners {
        chat: ChatInput,
        editor: ChatEditor,
        hotbuttons: HotbuttonState,
        hotbutton_input: HotbuttonInput,
        keys: ButtonInput<KeyCode>,
    }

    impl EditorOwners {
        fn open_hotbutton_editor(&mut self) {
            self.hotbuttons.sync_context(1, true);
            self.chat.cancel_for_handoff(&mut self.editor);
            assert!(self.hotbuttons.open_editor(self.hotbuttons.token(), 0));
        }

        fn route_frame(&mut self, events: &[WindowEvent]) {
            let primary = window(1);
            self.hotbuttons.sync_context(1, true);
            self.keys.clear();
            for event in events {
                if let WindowEvent::KeyboardInput(event) = event
                    && event.window == primary
                {
                    match event.state {
                        ButtonState::Pressed => self.keys.press(event.key_code),
                        ButtonState::Released => self.keys.release(event.key_code),
                    }
                }
            }
            recover_focus_loss(primary, events, &mut self.keys, &mut ButtonInput::default());
            self.chat.begin_handoff_frame(&mut self.keys);
            self.hotbutton_input.begin_handoff_frame(&mut self.keys);
            let frame = openeq_ui::UiFrame::default();
            for event in events {
                // Match main's routing order: passive observation, retired
                // chat filter, modal hotbutton owner, then ordinary chat.
                self.chat.observe_modifiers(event, primary);
                self.hotbutton_input.observe_modifiers(event, primary);
                if self.hotbuttons.editor.is_some()
                    && self
                        .chat
                        .filter_handoff_event(event, primary, &mut self.keys)
                {
                    continue;
                }
                let result = self.hotbutton_input.route_event(
                    &mut self.hotbuttons,
                    &HotbuttonInputContext {
                        frame: &frame,
                        window: primary,
                        pointer: None,
                        pointer_allowed: true,
                    },
                    event,
                    &mut self.keys,
                );
                if let Some((action, _)) = result.action {
                    assert_eq!(action.kind, HotbuttonActionKind::Cancel);
                    self.hotbuttons.cancel_editor();
                }
                if !result.captured {
                    self.chat
                        .route_event(&mut self.editor, primary, event, &mut self.keys);
                }
            }
            self.chat.suppress_captured_keys(&mut self.keys);
            self.hotbutton_input.suppress_captured_keys(&mut self.keys);
        }
    }

    #[test]
    fn late_chat_commit_stays_cancelled_after_hotbutton_editor_closes() {
        let primary = window(1);
        for empty_preedit_before_handoff in [false, true] {
            let mut owners = EditorOwners::default();
            owners.editor.open("old draft");
            owners.route_frame(&[preedit(primary, "old composition")]);
            if empty_preedit_before_handoff {
                owners.route_frame(&[preedit(primary, "")]);
            }
            owners.open_hotbutton_editor();
            owners.route_frame(&[
                press(primary, KeyCode::Escape, None),
                press(primary, KeyCode::Enter, None),
                commit(window(2), "foreign composition"),
                commit(primary, "late old composition"),
            ]);
            assert!(owners.hotbuttons.editor.is_none());
            assert!(owners.editor.active);
            assert!(owners.editor.text.is_empty());
            owners.route_frame(&[
                preedit(primary, "new composition"),
                preedit(primary, ""),
                commit(primary, "new composition"),
            ]);
            assert_eq!(owners.editor.text, "new composition");
        }
    }

    #[test]
    fn fresh_chat_composition_replaces_cancelled_owner_after_hotbutton_close() {
        let primary = window(1);
        let mut owners = EditorOwners::default();
        owners.editor.open("old draft");
        owners.route_frame(&[preedit(primary, "cancelled")]);
        owners.open_hotbutton_editor();
        owners.route_frame(&[
            press(primary, KeyCode::Escape, None),
            press(primary, KeyCode::Enter, None),
            preedit(primary, "fresh"),
            commit(primary, "fresh"),
        ]);
        assert!(owners.editor.active);
        assert_eq!(owners.editor.text, "fresh");
        assert!(owners.editor.preedit.is_empty());
    }

    #[test]
    fn hotbutton_owned_modifier_release_does_not_stick_in_reopened_chat() {
        let primary = window(1);
        for modifier in [
            KeyCode::SuperLeft,
            KeyCode::SuperRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
        ] {
            let mut owners = EditorOwners::default();
            // This press was observed in gameplay, before chat could own it.
            owners.route_frame(&[press(primary, modifier, None)]);
            owners.open_hotbutton_editor();
            owners.route_frame(&[
                release(window(2), modifier),
                release(primary, modifier),
                press(primary, KeyCode::Escape, None),
                press(primary, KeyCode::Enter, None),
                press(primary, KeyCode::KeyA, Some("fresh")),
            ]);
            assert!(owners.editor.active);
            assert_eq!(owners.editor.text, "fresh", "stale {modifier:?}");
            assert!(!owners.keys.pressed(modifier));
        }
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
        recover_focus_loss(window, events, keys, &mut ButtonInput::default());
        input.begin_handoff_frame(keys);
        let results = events
            .iter()
            .map(|event| input.route_event(editor, window, event, keys))
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
    fn submitted_chat_repeat_then_release_has_no_gameplay_edge() {
        let primary = window(1);
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
                press(primary, KeyCode::KeyQ, Some("q")),
                press(primary, KeyCode::Enter, None),
            ],
        );
        let results = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                keyboard(
                    primary,
                    KeyCode::KeyQ,
                    ButtonState::Pressed,
                    Some("q"),
                    true,
                ),
                release(primary, KeyCode::KeyQ),
                press(primary, KeyCode::KeyD, Some("d")),
            ],
        );
        assert!(results.iter().all(|result| !result.captured));
        assert!(!keys.pressed(KeyCode::KeyQ));
        assert!(!keys.just_pressed(KeyCode::KeyQ));
        assert!(!keys.just_released(KeyCode::KeyQ));
        assert!(keys.pressed(KeyCode::KeyD));
        assert!(keys.just_pressed(KeyCode::KeyD));
    }

    #[test]
    fn reopened_chat_rejects_previous_session_repeats_until_release() {
        let primary = window(1);
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
        route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                release(primary, KeyCode::Enter),
                press(primary, KeyCode::Enter, None),
                keyboard(
                    primary,
                    KeyCode::KeyW,
                    ButtonState::Pressed,
                    Some("w"),
                    true,
                ),
                press(primary, KeyCode::KeyA, Some("a")),
            ],
        );
        assert!(editor.active);
        assert_eq!(editor.text, "a");
        route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                release(primary, KeyCode::KeyW),
                press(primary, KeyCode::KeyW, Some("w")),
                keyboard(
                    primary,
                    KeyCode::KeyW,
                    ButtonState::Pressed,
                    Some("w"),
                    true,
                ),
            ],
        );
        assert_eq!(editor.text, "aww");
        assert!(!keys.pressed(KeyCode::KeyW));
    }

    #[test]
    fn chat_handoff_replays_fresh_keys_around_focus_loss_in_native_order() {
        let primary = window(1);
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
                press(primary, KeyCode::KeyQ, Some("q")),
                press(primary, KeyCode::Enter, None),
            ],
        );
        route_keyboard_frame(&mut input, &mut editor, primary, &mut keys, &[]);
        assert!(!keys.pressed(KeyCode::KeyQ));
        let results = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                keyboard(
                    primary,
                    KeyCode::KeyQ,
                    ButtonState::Pressed,
                    Some("q"),
                    true,
                ),
                release(primary, KeyCode::KeyQ),
                press(primary, KeyCode::KeyQ, Some("q")),
                press(primary, KeyCode::KeyA, Some("a")),
                focus(primary, false),
                focus(primary, true),
                press(primary, KeyCode::KeyQ, Some("q")),
                release(primary, KeyCode::KeyQ),
                press(primary, KeyCode::KeyD, Some("d")),
            ],
        );
        assert!(results.iter().all(|result| !result.captured));
        assert!(!keys.pressed(KeyCode::KeyQ));
        assert!(keys.just_pressed(KeyCode::KeyQ));
        assert!(keys.just_released(KeyCode::KeyQ));
        assert!(!keys.pressed(KeyCode::KeyA));
        assert!(!keys.just_pressed(KeyCode::KeyA));
        assert!(keys.pressed(KeyCode::KeyD));
        assert!(keys.just_pressed(KeyCode::KeyD));
        assert!(!editor.active);
        assert!(editor.text.is_empty());
    }

    #[test]
    fn pointer_closed_chat_does_not_transfer_held_text_or_modifiers_to_next_editor() {
        let primary = window(1);
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
                press(primary, KeyCode::SuperLeft, None),
            ],
        );
        // Main's pointer handler closes or opens before forwarding the click.
        editor.cancel();
        route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[mouse_button(
                primary,
                MouseButton::Left,
                ButtonState::Pressed,
            )],
        );
        editor.open("");
        route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                mouse_button(primary, MouseButton::Left, ButtonState::Pressed),
                keyboard(
                    primary,
                    KeyCode::SuperLeft,
                    ButtonState::Pressed,
                    None,
                    true,
                ),
                keyboard(
                    primary,
                    KeyCode::KeyW,
                    ButtonState::Pressed,
                    Some("w"),
                    true,
                ),
                press(primary, KeyCode::KeyA, Some("a")),
                keyboard(
                    primary,
                    KeyCode::KeyA,
                    ButtonState::Pressed,
                    Some("a"),
                    true,
                ),
            ],
        );
        assert_eq!(editor.text, "aa");
        assert!(!keys.pressed(KeyCode::KeyW));
        assert!(!keys.pressed(KeyCode::SuperLeft));
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
    fn empty_preedit_does_not_release_chat_composition_before_commit() {
        let primary = window(1);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        let mut keys = ButtonInput::default();
        editor.open("say ");
        let results = route_keyboard_frame(
            &mut input,
            &mut editor,
            primary,
            &mut keys,
            &[
                preedit(primary, "にほん"),
                preedit(primary, ""),
                press(primary, KeyCode::Enter, None),
                commit(primary, "日本"),
            ],
        );
        assert!(results.iter().all(|result| result.submitted.is_none()));
        assert!(editor.active);
        assert_eq!(editor.text, "say 日本");
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
    #[test]
    fn retired_chat_owner_filters_old_text_and_restores_a_fresh_same_frame_press() {
        let primary = window(1);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        let mut keys = ButtonInput::default();
        editor.open("/hotbutton 1");
        input.route_event(
            &mut editor,
            primary,
            &press(primary, KeyCode::KeyW, Some("w")),
            &mut keys,
        );
        input.cancel_for_handoff(&mut editor);
        keys.press(KeyCode::KeyW);
        keys.release(KeyCode::KeyW); // Bevy already applied the whole batch.
        input.begin_handoff_frame(&mut keys);
        assert!(input.filter_handoff_event(
            &keyboard(
                primary,
                KeyCode::KeyW,
                ButtonState::Pressed,
                Some("old"),
                true
            ),
            primary,
            &mut keys
        ));
        assert!(input.filter_handoff_event(&release(primary, KeyCode::KeyW), primary, &mut keys));
        assert!(!keys.pressed(KeyCode::KeyW) && !keys.just_released(KeyCode::KeyW));
        assert!(!input.filter_handoff_event(
            &press(primary, KeyCode::KeyW, Some("fresh")),
            primary,
            &mut keys
        ));
        assert!(keys.pressed(KeyCode::KeyW) && keys.just_pressed(KeyCode::KeyW));
        assert!(!input.filter_handoff_event(
            &press(primary, KeyCode::Enter, None),
            primary,
            &mut keys
        ));
        assert!(!editor.active, "captured-only filter must never open chat");
        assert_eq!(editor.text, "/hotbutton 1w");
    }

    #[test]
    fn chat_composition_cannot_commit_into_another_editor_after_handoff() {
        let primary = window(1);
        let mut input = ChatInput::default();
        let mut editor = ChatEditor::default();
        let mut keys = ButtonInput::default();
        editor.open("draft");
        input.event(&mut editor, primary, &preedit(primary, "old composition"));
        input.cancel_for_handoff(&mut editor);
        assert!(!input.filter_handoff_event(&commit(window(2), "foreign"), primary, &mut keys));
        assert!(input.filter_handoff_event(
            &commit(primary, "late old composition"),
            primary,
            &mut keys
        ));
        assert!(!input.filter_handoff_event(
            &preedit(primary, "new composition"),
            primary,
            &mut keys
        ));
        assert!(!input.filter_handoff_event(
            &commit(primary, "new composition"),
            primary,
            &mut keys
        ));
        assert_eq!(editor.text, "draft");
        assert!(!editor.active && editor.preedit.is_empty());
    }
}
