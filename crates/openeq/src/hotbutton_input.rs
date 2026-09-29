//! Ordered native input for the modal command-button editor. This controller
//! edits drafts and returns intents; it never dispatches gameplay commands.
use crate::{
    chat::MAX_CHAT_BYTES,
    hotbutton_interaction::{HotbuttonDraft, HotbuttonState},
    hotbutton_ui::{
        HotbuttonAction, HotbuttonActionKind, hotbutton_editor_hit_identity, hotbutton_window_hit,
    },
    hotbuttons::{
        HotbuttonField, HotbuttonToken, MAX_HOTBUTTON_LABEL_BYTES, MAX_HOTBUTTON_LABEL_CHARS,
    },
};
use bevy::{
    input::ButtonState,
    prelude::{ButtonInput, Entity, KeyCode, MouseButton},
    window::{Ime, WindowEvent},
};
use openeq_ui::UiFrame;
use std::collections::HashSet;

pub struct HotbuttonInputContext<'a> {
    pub frame: &'a UiFrame,
    pub window: Entity,
    pub pointer: Option<[f32; 2]>,
    pub pointer_allowed: bool,
}

#[derive(Default, Debug)]
pub struct HotbuttonInputResult {
    pub captured: bool,
    pub escape_handled: bool,
    /// The flag denotes a right-button release, never a keyboard action.
    pub action: Option<(HotbuttonAction, bool)>,
}

struct Pressed {
    action: HotbuttonAction,
    item: String,
    button: MouseButton,
}

pub struct HotbuttonInput {
    modifiers: ButtonInput<KeyCode>,
    captured_keys: HashSet<KeyCode>,
    session_keys: HashSet<KeyCode>,
    handoff_replay: HashSet<KeyCode>,
    captured_buttons: HashSet<MouseButton>,
    pressed: Option<Pressed>,
    seen: Option<(Entity, HotbuttonToken, bool)>,
    focus: Option<HotbuttonField>,
    focused: bool,
    composition: Option<(HotbuttonToken, HotbuttonField)>,
    cancelled_composition: bool,
}

impl Default for HotbuttonInput {
    fn default() -> Self {
        Self {
            modifiers: Default::default(),
            captured_keys: Default::default(),
            session_keys: Default::default(),
            handoff_replay: Default::default(),
            captured_buttons: Default::default(),
            pressed: None,
            seen: None,
            focus: None,
            focused: true,
            composition: None,
            cancelled_composition: false,
        }
    }
}

impl HotbuttonInput {
    pub fn pressed(&self) -> Option<&HotbuttonAction> {
        self.pressed.as_ref().map(|press| &press.action)
    }

    fn retire_session(&mut self) {
        self.session_keys.clear();
    }

    /// Observe before any input owner's filter, since that owner can consume a
    /// modifier release. Keep physical modifiers in native event order even
    /// when chat owns the key; never infer them from Bevy's final batch state.
    pub fn observe_modifiers(&mut self, event: &WindowEvent, window: Entity) {
        match event {
            WindowEvent::KeyboardInput(event)
                if event.window == window
                    && matches!(
                        event.key_code,
                        KeyCode::ControlLeft
                            | KeyCode::ControlRight
                            | KeyCode::SuperLeft
                            | KeyCode::SuperRight
                            | KeyCode::ShiftLeft
                            | KeyCode::ShiftRight
                    ) =>
            {
                match event.state {
                    ButtonState::Pressed => self.modifiers.press(event.key_code),
                    ButtonState::Released => self.modifiers.reset(event.key_code),
                }
            }
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.modifiers.reset_all();
            }
            _ => {}
        }
    }

    fn cancel_composition(&mut self, state: &mut HotbuttonState) {
        self.cancelled_composition |= self.composition.take().is_some();
        if let Some(draft) = &mut state.editor {
            draft.label.preedit.clear();
            draft.command.preedit.clear();
        }
    }

    pub fn begin_handoff_frame(&mut self, keys: &mut ButtonInput<KeyCode>) {
        self.handoff_replay.clone_from(&self.captured_keys);
        self.suppress_captured_keys(keys);
    }

    pub fn suppress_captured_keys(&self, keys: &mut ButtonInput<KeyCode>) {
        for key in &self.captured_keys {
            keys.reset(*key);
        }
    }

    /// For loading or another input owner. Retire the editor without opening
    /// chat or interpreting a fresh key. Old physical keys remain ours until
    /// release, including repeats delivered after Save/Cancel.
    pub fn filter_handoff_event(
        &mut self,
        event: &WindowEvent,
        window: Entity,
        keys: &mut ButtonInput<KeyCode>,
    ) -> bool {
        self.observe_modifiers(event, window);
        self.retire_session();
        self.pressed = None;
        self.seen = None;
        self.focus = None;
        self.cancelled_composition |= self.composition.take().is_some();
        match event {
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if self.captured_keys.contains(&event.key_code) {
                    self.handoff_replay.insert(event.key_code);
                    keys.reset(event.key_code);
                    if event.state == ButtonState::Released {
                        self.captured_keys.remove(&event.key_code);
                        self.modifiers.reset(event.key_code);
                    }
                    return true;
                }
                self.replay_key(event.key_code, event.state, keys);
            }
            WindowEvent::MouseButtonInput(event) if event.window == window => {
                if self.captured_buttons.contains(&event.button) {
                    if event.state == ButtonState::Released {
                        self.captured_buttons.remove(&event.button);
                    }
                    return true;
                }
            }
            WindowEvent::WindowFocused(event) if event.window == window => {
                self.focused = event.focused;
                if !event.focused {
                    self.lose_focus(keys);
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
                // A fresh composition belongs to the new input owner.
                self.cancelled_composition = false;
            }
            _ => {}
        }
        false
    }

    fn replay_key(&self, key: KeyCode, state: ButtonState, keys: &mut ButtonInput<KeyCode>) {
        if self.handoff_replay.contains(&key) {
            match state {
                ButtonState::Pressed => keys.press(key),
                ButtonState::Released => keys.release(key),
            }
        }
    }

    fn lose_focus(&mut self, keys: &mut ButtonInput<KeyCode>) {
        for key in self.captured_keys.iter().chain(&self.handoff_replay) {
            keys.reset(*key);
        }
        self.modifiers.reset_all();
        self.captured_keys.clear();
        self.session_keys.clear();
        self.captured_buttons.clear();
        self.pressed = None;
    }

    pub fn route_event(
        &mut self,
        state: &mut HotbuttonState,
        context: &HotbuttonInputContext<'_>,
        event: &WindowEvent,
        keys: &mut ButtonInput<KeyCode>,
    ) -> HotbuttonInputResult {
        self.observe_modifiers(event, context.window);
        let seen = (context.window, state.token(), state.editor.is_some());
        let focus = state.editor.as_ref().map(|draft| draft.focus);
        if self.seen != Some(seen) {
            self.cancel_composition(state);
            self.retire_session();
            self.pressed = None;
            self.seen = Some(seen);
        } else if self.focus != focus {
            self.cancel_composition(state);
            self.pressed = None;
        }
        self.focus = focus;
        if !context.pointer_allowed {
            self.pressed = None;
        }
        if !state.available() {
            self.cancel_composition(state);
            return HotbuttonInputResult {
                captured: self.filter_handoff_event(event, context.window, keys),
                ..Default::default()
            };
        }
        let owned_before = matches!(event, WindowEvent::KeyboardInput(event)
            if event.window == context.window && self.captured_keys.contains(&event.key_code));
        let result = self.event(state, context, event, keys);
        if let WindowEvent::KeyboardInput(event) = event
            && event.window == context.window
        {
            if owned_before || self.captured_keys.contains(&event.key_code) {
                self.handoff_replay.insert(event.key_code);
                keys.reset(event.key_code);
            } else {
                self.replay_key(event.key_code, event.state, keys);
            }
        }
        result
    }

    fn event(
        &mut self,
        state: &mut HotbuttonState,
        context: &HotbuttonInputContext<'_>,
        event: &WindowEvent,
        keys: &mut ButtonInput<KeyCode>,
    ) -> HotbuttonInputResult {
        let mut result = HotbuttonInputResult::default();
        let modal = state.editor.is_some();
        let token = state.token();
        match event {
            WindowEvent::WindowFocused(event) if event.window == context.window => {
                self.focused = event.focused;
                if !event.focused {
                    self.cancel_composition(state);
                    self.lose_focus(keys);
                }
            }
            WindowEvent::CursorLeft(event) if event.window == context.window => {
                self.pressed = None;
            }
            WindowEvent::CursorMoved(event) if event.window == context.window => {
                let hit = context.frame.hit_test([event.position.x, event.position.y]);
                if self.pressed.as_ref().is_some_and(|press| {
                    hit.is_none_or(|hit| hit.item != press.item)
                        || hit.and_then(HotbuttonAction::from_hit).as_ref() != Some(&press.action)
                }) {
                    self.pressed = None;
                }
            }
            WindowEvent::MouseWheel(event) if event.window == context.window => {
                result.captured = modal;
            }
            WindowEvent::MouseButtonInput(event) if event.window == context.window => {
                let hit = context
                    .pointer
                    .filter(|_| context.pointer_allowed && self.focused)
                    .and_then(|point| context.frame.hit_test(point));
                let action = hit.and_then(HotbuttonAction::from_hit).filter(|action| {
                    action.token == token
                        && match action.kind {
                            HotbuttonActionKind::Slot(_) | HotbuttonActionKind::CloseBar => {
                                state.open && !modal
                            }
                            _ => state.editor.as_ref().is_some_and(|draft| {
                                hit.and_then(hotbutton_editor_hit_identity)
                                    == Some((token, draft.slot))
                            }),
                        }
                });
                let owned = self.captured_buttons.contains(&event.button);
                result.captured = modal || owned || hit.is_some_and(hotbutton_window_hit);
                match event.state {
                    ButtonState::Pressed if result.captured => {
                        self.captured_buttons.insert(event.button);
                        self.pressed = action
                            .filter(|action| {
                                event.button == MouseButton::Left
                                    || (event.button == MouseButton::Right
                                        && matches!(action.kind, HotbuttonActionKind::Slot(_)))
                            })
                            .map(|action| Pressed {
                                action,
                                item: hit.unwrap().item.clone(),
                                button: event.button,
                            });
                    }
                    ButtonState::Released => {
                        self.captured_buttons.remove(&event.button);
                        if self
                            .pressed
                            .as_ref()
                            .is_some_and(|press| press.button == event.button)
                        {
                            let press = self.pressed.take().unwrap();
                            if action.as_ref() == Some(&press.action)
                                && hit.is_some_and(|hit| hit.item == press.item)
                            {
                                if modal {
                                    self.cancel_composition(state);
                                    if let HotbuttonActionKind::Focus(field) = press.action.kind {
                                        let draft = state.editor.as_mut().unwrap();
                                        draft.focus = field;
                                        draft.set_selected(field, false);
                                        let edit = draft.edit_mut(field);
                                        edit.cursor = edit.text.len();
                                        self.focus = Some(field);
                                    }
                                }
                                result.action =
                                    Some((press.action, event.button == MouseButton::Right));
                            }
                        }
                    }
                    _ => self.pressed = None,
                }
            }
            WindowEvent::KeyboardInput(event) if event.window == context.window => {
                let owned = self.captured_keys.contains(&event.key_code);
                result.captured = modal || owned;
                result.escape_handled = result.captured
                    && event.key_code == KeyCode::Escape
                    && event.state == ButtonState::Pressed;
                if event.state == ButtonState::Released {
                    self.captured_keys.remove(&event.key_code);
                    self.session_keys.remove(&event.key_code);
                    self.modifiers.reset(event.key_code);
                    return result;
                }
                if owned && !self.session_keys.contains(&event.key_code) {
                    return result;
                }
                if !self.focused {
                    return result;
                }
                if !modal {
                    return result;
                }
                self.captured_keys.insert(event.key_code);
                self.session_keys.insert(event.key_code);
                if event.key_code == KeyCode::Escape {
                    result.escape_handled = true;
                    if !event.repeat {
                        self.pressed = None;
                        if self.composition.is_some() {
                            self.cancel_composition(state);
                        } else {
                            result.action = Some((
                                HotbuttonAction {
                                    token,
                                    kind: HotbuttonActionKind::Cancel,
                                },
                                false,
                            ));
                        }
                    }
                    return result;
                }
                if self.composition.is_some() {
                    return result;
                }
                let command = [
                    KeyCode::ControlLeft,
                    KeyCode::ControlRight,
                    KeyCode::SuperLeft,
                    KeyCode::SuperRight,
                ]
                .iter()
                .any(|key| self.modifiers.pressed(*key));
                let draft = state.editor.as_mut().unwrap();
                let field = draft.focus;
                match event.key_code {
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        if !event.repeat {
                            self.pressed = None;
                            result.action = Some((
                                HotbuttonAction {
                                    token,
                                    kind: HotbuttonActionKind::Save,
                                },
                                false,
                            ));
                        }
                    }
                    KeyCode::Tab if !event.repeat => {
                        draft.focus = match field {
                            HotbuttonField::Label => HotbuttonField::Command,
                            HotbuttonField::Command => HotbuttonField::Label,
                        };
                        self.focus = Some(draft.focus);
                        self.pressed = None;
                    }
                    KeyCode::Tab => {}
                    KeyCode::KeyA if command => {
                        draft.set_selected(field, true);
                        self.pressed = None;
                    }
                    KeyCode::KeyU if command => {
                        clear_field(draft, field);
                        self.pressed = None;
                    }
                    KeyCode::Backspace | KeyCode::Delete => {
                        if draft.selected(field) {
                            clear_field(draft, field);
                        } else if event.key_code == KeyCode::Backspace {
                            draft.edit_mut(field).backspace(command);
                        } else {
                            draft.edit_mut(field).delete();
                        }
                        draft.error = None;
                        self.pressed = None;
                    }
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home | KeyCode::End => {
                        let selected = draft.selected(field);
                        draft.set_selected(field, false);
                        let edit = draft.edit_mut(field);
                        match event.key_code {
                            KeyCode::Home => edit.cursor = 0,
                            KeyCode::End => edit.cursor = edit.text.len(),
                            KeyCode::ArrowLeft if selected => edit.cursor = 0,
                            KeyCode::ArrowRight if selected => edit.cursor = edit.text.len(),
                            KeyCode::ArrowLeft => edit.left(),
                            _ => edit.right(),
                        }
                        self.pressed = None;
                    }
                    _ if !command => {
                        if let Some(text) = &event.text {
                            insert(draft, field, text);
                            self.pressed = None;
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::Ime(Ime::Preedit { window, value, .. }) if *window == context.window => {
                result.captured = modal;
                if !value.is_empty() {
                    // Main still routes through us while chat is active. A
                    // fresh composition releases the old cancellation here
                    // as well as in the loading-only handoff filter.
                    self.cancelled_composition = false;
                }
                if modal && self.focused {
                    let draft = state.editor.as_mut().unwrap();
                    if !value.is_empty() {
                        self.composition = Some((token, draft.focus));
                    }
                    let field = draft.focus;
                    let (bytes, chars) = if draft.selected(field) {
                        (0, 0)
                    } else {
                        let edit = draft.edit(field);
                        (edit.text.len(), edit.text.chars().count())
                    };
                    draft.edit_mut(field).preedit = bounded(value, field, bytes, chars);
                    self.pressed = None;
                }
            }
            WindowEvent::Ime(Ime::Commit { window, value }) if *window == context.window => {
                result.captured = modal || self.cancelled_composition;
                if self.cancelled_composition {
                    self.cancelled_composition = false;
                    return result;
                }
                let owner = self.composition.take();
                if let Some(draft) = &mut state.editor
                    && self.focused
                    && owner.is_none_or(|owner| owner == (token, draft.focus))
                {
                    draft.edit_mut(draft.focus).preedit.clear();
                    insert(draft, draft.focus, value);
                    self.pressed = None;
                }
            }
            WindowEvent::Ime(Ime::Disabled { window }) if *window == context.window => {
                self.cancel_composition(state);
            }
            _ => {}
        }
        result
    }
}

fn clear_field(draft: &mut HotbuttonDraft, field: HotbuttonField) {
    draft.edit_mut(field).open("");
    draft.set_selected(field, false);
    draft.error = None;
}

fn bounded(
    text: &str,
    field: HotbuttonField,
    existing_bytes: usize,
    existing_chars: usize,
) -> String {
    let (bytes, chars) = match field {
        HotbuttonField::Label => (MAX_HOTBUTTON_LABEL_BYTES, MAX_HOTBUTTON_LABEL_CHARS),
        HotbuttonField::Command => (MAX_CHAT_BYTES, MAX_CHAT_BYTES),
    };
    let mut out = String::new();
    for (count, ch) in text.chars().filter(|ch| !ch.is_control()).enumerate() {
        if existing_bytes + out.len() + ch.len_utf8() > bytes || existing_chars + count >= chars {
            break;
        }
        out.push(ch);
    }
    out
}

fn insert(draft: &mut HotbuttonDraft, field: HotbuttonField, text: &str) {
    // Reject the original payload rather than turning a pasted multi-line
    // command into a different, apparently valid single command.
    if text.chars().any(char::is_control) {
        draft.error = Some("Enter text without control characters or line breaks.".into());
        return;
    }
    if draft.selected(field) {
        clear_field(draft, field);
    }
    let edit = draft.edit_mut(field);
    let text = bounded(text, field, edit.text.len(), edit.text.chars().count());
    edit.insert(&text);
    draft.error = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        input::{
            keyboard::{Key, KeyboardInput, NativeKey},
            mouse::MouseButtonInput,
        },
        math::Vec2,
        window::{CursorMoved, WindowFocused},
    };
    use openeq_ui::{HitTarget, Rect};

    fn window() -> Entity {
        Entity::from_raw_u32(1).unwrap()
    }
    fn keyboard(
        key_code: KeyCode,
        state: ButtonState,
        text: Option<&str>,
        repeat: bool,
    ) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyboardInput {
            window: window(),
            key_code,
            state,
            repeat,
            text: text.map(Into::into),
            logical_key: Key::Unidentified(NativeKey::Unidentified),
        })
    }
    fn down(key: KeyCode, text: Option<&str>) -> WindowEvent {
        keyboard(key, ButtonState::Pressed, text, false)
    }
    fn up(key: KeyCode) -> WindowEvent {
        keyboard(key, ButtonState::Released, None, false)
    }
    fn repeat(key: KeyCode, text: Option<&str>) -> WindowEvent {
        keyboard(key, ButtonState::Pressed, text, true)
    }
    fn mouse(button: MouseButton, state: ButtonState) -> WindowEvent {
        WindowEvent::MouseButtonInput(MouseButtonInput {
            window: window(),
            button,
            state,
        })
    }
    fn preedit(text: &str) -> WindowEvent {
        WindowEvent::Ime(Ime::Preedit {
            window: window(),
            value: text.into(),
            cursor: None,
        })
    }
    fn commit(text: &str) -> WindowEvent {
        WindowEvent::Ime(Ime::Commit {
            window: window(),
            value: text.into(),
        })
    }
    fn hit(state: &HotbuttonState, suffix: &str, editor: bool, x: f32) -> HitTarget {
        let token = state.token();
        HitTarget {
            item: format!(
                "hotbutton:{}:{}:{suffix}",
                token.revision, token.zone_generation
            ),
            screen_id: String::new(),
            window_id: Some(
                if editor {
                    "hotbutton_editor"
                } else {
                    "hotbuttons"
                }
                .into(),
            ),
            kind: "HotbuttonControl".into(),
            rect: Rect::new(x, 0., 20., 20.),
            enabled: true,
            tooltip: None,
        }
    }
    fn frame(state: &HotbuttonState) -> UiFrame {
        let mut frame = UiFrame::default();
        if let Some(draft) = &state.editor {
            for (index, name) in ["label", "command", "clear", "save", "cancel"]
                .into_iter()
                .enumerate()
            {
                frame.hit_targets.push(hit(
                    state,
                    &format!("editor:{}:{name}", draft.slot),
                    true,
                    index as f32 * 25.,
                ));
            }
        } else {
            frame.hit_targets.push(hit(state, "slot:0", false, 0.));
            frame.hit_targets.push(hit(state, "slot:1", false, 25.));
        }
        frame
    }
    struct Harness {
        state: HotbuttonState,
        input: HotbuttonInput,
        frame: UiFrame,
        keys: ButtonInput<KeyCode>,
        pointer: Option<[f32; 2]>,
        pointer_allowed: bool,
    }
    impl Harness {
        fn new(editor: bool) -> Self {
            let mut state = HotbuttonState::default();
            state.sync_context(7, true);
            if editor {
                assert!(state.open_editor(state.token(), 0));
            }
            let frame = frame(&state);
            Self {
                state,
                frame,
                input: Default::default(),
                keys: Default::default(),
                pointer: Some([10., 10.]),
                pointer_allowed: true,
            }
        }
        fn send(&mut self, event: WindowEvent) -> HotbuttonInputResult {
            if let WindowEvent::CursorMoved(event) = &event {
                self.pointer = Some([event.position.x, event.position.y]);
            }
            self.input.route_event(
                &mut self.state,
                &HotbuttonInputContext {
                    frame: &self.frame,
                    window: window(),
                    pointer: self.pointer,
                    pointer_allowed: self.pointer_allowed,
                },
                &event,
                &mut self.keys,
            )
        }
        fn apply(&mut self, result: HotbuttonInputResult) {
            if let Some((action, right)) = result.action {
                assert_eq!(action.token, self.state.token());
                match action.kind {
                    HotbuttonActionKind::Save => {
                        self.state.save_editor(action.token);
                    }
                    HotbuttonActionKind::Cancel => self.state.cancel_editor(),
                    HotbuttonActionKind::Clear => self.state.clear_draft(),
                    HotbuttonActionKind::Focus(field) => {
                        self.state.editor.as_mut().unwrap().focus = field
                    }
                    HotbuttonActionKind::Slot(slot)
                        if right || self.state.bindings()[usize::from(slot)].is_none() =>
                    {
                        self.state.open_editor(action.token, slot);
                    }
                    HotbuttonActionKind::CloseBar => self.state.set_open(false),
                    HotbuttonActionKind::Slot(_) => {}
                }
            }
        }
        fn move_to(&mut self, x: f32) {
            self.send(WindowEvent::CursorMoved(CursorMoved {
                window: window(),
                position: Vec2::new(x, 10.),
                delta: None,
            }));
        }
        fn click(&mut self, x: f32, button: MouseButton) -> HotbuttonInputResult {
            self.move_to(x);
            assert!(self.send(mouse(button, ButtonState::Pressed)).captured);
            self.send(mouse(button, ButtonState::Released))
        }
        fn draft(&self) -> &HotbuttonDraft {
            self.state.editor.as_ref().unwrap()
        }
        fn valid_draft(&mut self) {
            let draft = self.state.editor.as_mut().unwrap();
            draft.label.open("Sit");
            draft.command.open("/sit");
        }
    }

    #[test]
    fn pointer_requires_same_button_hit_token_and_one_release() {
        let mut h = Harness::new(false);
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Pressed))
                .captured
        );
        assert!(h.input.pressed().is_some());
        assert!(
            h.send(mouse(MouseButton::Right, ButtonState::Released))
                .action
                .is_none()
        );
        let result = h.send(mouse(MouseButton::Left, ButtonState::Released));
        assert_eq!(
            result.action,
            Some((
                HotbuttonAction {
                    token: h.state.token(),
                    kind: HotbuttonActionKind::Slot(0)
                },
                false
            ))
        );
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );

        h.send(mouse(MouseButton::Left, ButtonState::Pressed));
        h.move_to(35.);
        h.move_to(10.);
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
        h.send(mouse(MouseButton::Left, ButtonState::Pressed));
        let mut occluder = hit(&h.state, "slot:0", false, 0.);
        occluder.item = "unrelated".into();
        occluder.window_id = Some("inventory".into());
        h.frame.hit_targets.push(occluder);
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
        h.frame = frame(&h.state);
        h.send(mouse(MouseButton::Left, ButtonState::Pressed));
        h.state.sync_context(8, true);
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
        assert!(h.input.pressed().is_none());
        assert!(h.click(10., MouseButton::Left).action.is_none()); // stale frame
    }

    #[test]
    fn right_click_and_empty_slot_only_return_intents_and_modal_consumes_outside() {
        let mut h = Harness::new(false);
        let action = h.click(10., MouseButton::Right);
        assert!(action.action.as_ref().unwrap().1);
        assert!(h.state.editor.is_none());
        h.apply(action);
        assert!(h.state.editor.is_some());
        assert!(h.state.bindings().iter().all(Option::is_none));
        h.frame = frame(&h.state);
        let result = h.click(300., MouseButton::Left);
        assert!(result.captured && result.action.is_none());
        assert!(h.state.editor.is_some());
        assert!(h.click(85., MouseButton::Right).action.is_none()); // no right Save
        let cancel = h.click(110., MouseButton::Left);
        assert!(cancel.captured);
        h.apply(cancel);
        assert!(h.state.editor.is_none());
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
    }

    #[test]
    fn editor_slot_identity_and_changes_during_pointer_press_cancel() {
        let mut h = Harness::new(true);
        h.valid_draft();
        h.frame.hit_targets[3] = hit(&h.state, "editor:1:save", true, 75.);
        assert!(h.click(85., MouseButton::Left).action.is_none());
        h.frame = frame(&h.state);
        h.move_to(85.);
        h.send(mouse(MouseButton::Left, ButtonState::Pressed));
        h.send(down(KeyCode::KeyX, Some("x")));
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
        h.send(mouse(MouseButton::Left, ButtonState::Pressed));
        h.pointer_allowed = false;
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
    }

    #[test]
    fn save_cancel_and_loading_hold_keys_until_release_then_allow_fresh_press() {
        for save in [true, false] {
            let mut h = Harness::new(true);
            h.valid_draft();
            h.keys.press(KeyCode::KeyW);
            h.send(down(KeyCode::KeyW, Some("w")));
            let closing = if save {
                KeyCode::Enter
            } else {
                KeyCode::Escape
            };
            let result = h.send(down(closing, None));
            assert!(result.captured);
            assert_eq!(result.escape_handled, !save);
            h.apply(result);
            assert!(h.state.editor.is_none());
            // Bevy contains the final state of the whole native event batch.
            h.keys.reset_all();
            h.keys.press(KeyCode::KeyW);
            h.input.begin_handoff_frame(&mut h.keys);
            h.state.sync_context(8, false);
            for event in [
                repeat(KeyCode::KeyW, Some("w")),
                repeat(closing, None),
                up(KeyCode::KeyW),
                up(closing),
            ] {
                assert!(h.input.filter_handoff_event(&event, window(), &mut h.keys));
                assert!(!h.keys.pressed(KeyCode::KeyW));
                assert!(!h.keys.just_released(KeyCode::KeyW));
            }
            assert!(!h.input.filter_handoff_event(
                &down(KeyCode::KeyW, Some("w")),
                window(),
                &mut h.keys
            ));
            assert!(h.keys.pressed(KeyCode::KeyW) && h.keys.just_pressed(KeyCode::KeyW));
            h.input.suppress_captured_keys(&mut h.keys);
            assert!(h.keys.pressed(KeyCode::KeyW));
            h.state.sync_context(8, true);
            assert!(!h.send(repeat(KeyCode::Enter, None)).captured);
            assert!(h.state.editor.is_none()); // never opens chat or an editor
        }
    }

    #[test]
    fn old_repeats_cannot_enter_new_editor_but_same_batch_fresh_presses_can() {
        let mut h = Harness::new(true);
        h.send(down(KeyCode::KeyW, Some("w")));
        let result = h.send(down(KeyCode::Escape, None));
        h.apply(result);
        h.state.open_editor(h.state.token(), 1);
        assert!(h.send(repeat(KeyCode::KeyW, Some("w"))).captured);
        assert!(h.draft().label.text.is_empty());
        h.send(up(KeyCode::KeyW));
        h.send(down(KeyCode::KeyW, Some("w")));
        assert_eq!(h.draft().label.text, "w");
        h.send(down(KeyCode::Tab, None));
        h.send(down(KeyCode::Slash, Some("/sit")));
        assert_eq!(h.draft().command.text, "/sit");
    }

    #[test]
    fn held_modifiers_survive_editor_handoffs_and_other_owners_consumed_release() {
        for modifier in [KeyCode::ControlLeft, KeyCode::SuperLeft] {
            let mut h = Harness::new(false);
            // Gameplay or chat owns this press before the editor opens.
            h.send(down(modifier, None));
            h.state.open_editor(h.state.token(), 0);
            h.valid_draft();
            h.send(down(KeyCode::KeyA, Some("a")));
            assert!(h.draft().label_selected);
            assert_eq!(h.draft().label.text, "Sit");
            h.send(up(KeyCode::KeyA));
            // Simulate chat consuming the release before hotbutton routing.
            h.input.observe_modifiers(&up(modifier), window());
            h.send(down(KeyCode::KeyX, Some("x")));
            assert_eq!(h.draft().label.text, "x");

            // A modifier first captured inside the old hotbutton editor also
            // remains physically held across Cancel/new-draft session change.
            h.send(down(modifier, None));
            let cancel = h.send(down(KeyCode::Escape, None));
            h.apply(cancel);
            h.state.open_editor(h.state.token(), 1);
            h.valid_draft();
            h.send(down(KeyCode::KeyA, Some("a")));
            assert!(h.draft().label_selected);
            assert_eq!(h.draft().label.text, "Sit");
            h.send(up(modifier));
            h.send(up(KeyCode::KeyX));
            h.send(down(KeyCode::KeyX, Some("new")));
            assert_eq!(h.draft().label.text, "new");
        }
    }

    #[test]
    fn enter_and_tab_repeats_do_not_resave_or_change_focus() {
        let mut h = Harness::new(true);
        h.send(down(KeyCode::Tab, None));
        assert_eq!(h.draft().focus, HotbuttonField::Command);
        h.send(repeat(KeyCode::Tab, None));
        assert_eq!(h.draft().focus, HotbuttonField::Command);
        h.send(up(KeyCode::Tab));
        h.send(down(KeyCode::ShiftLeft, None));
        h.send(down(KeyCode::Tab, None));
        assert_eq!(h.draft().focus, HotbuttonField::Label);
        h.valid_draft();
        let save = h.send(down(KeyCode::Enter, None));
        assert!(matches!(
            save.action.as_ref().map(|(a, _)| &a.kind),
            Some(HotbuttonActionKind::Save)
        ));
        h.apply(save);
        assert!(h.send(repeat(KeyCode::Enter, None)).action.is_none());
        assert!(h.state.editor.is_none());
    }

    #[test]
    fn select_replace_delete_and_utf8_cursor_edits_are_field_local() {
        let mut h = Harness::new(true);
        h.send(down(KeyCode::KeyX, Some("é界")));
        h.send(down(KeyCode::ArrowLeft, None));
        h.send(down(KeyCode::Backspace, None));
        assert_eq!(h.draft().label.text, "界");
        assert_eq!(h.draft().label.cursor, 0);
        h.send(down(KeyCode::SuperLeft, None));
        h.send(down(KeyCode::KeyA, Some("a")));
        assert!(h.draft().label_selected);
        h.send(up(KeyCode::SuperLeft));
        h.send(down(KeyCode::KeyS, Some("Sit")));
        assert_eq!(h.draft().label.text, "Sit");
        assert!(!h.draft().label_selected);
        h.send(up(KeyCode::KeyA));
        h.send(down(KeyCode::ControlLeft, None));
        h.send(down(KeyCode::KeyA, None));
        h.send(down(KeyCode::Delete, None));
        assert!(h.draft().label.text.is_empty());
        h.send(up(KeyCode::ControlLeft));
        h.send(down(KeyCode::Tab, None));
        h.send(down(KeyCode::Slash, Some("/sit")));
        h.send(down(KeyCode::ControlLeft, None));
        h.send(down(KeyCode::KeyU, None));
        assert!(h.draft().command.text.is_empty());
        assert!(h.draft().label.text.is_empty());
    }

    #[test]
    fn pasted_controls_are_rejected_before_selection_or_command_changes() {
        let mut h = Harness::new(true);
        h.valid_draft();
        h.send(down(KeyCode::Tab, None));
        h.send(down(KeyCode::SuperLeft, None));
        h.send(down(KeyCode::KeyA, None));
        h.send(up(KeyCode::SuperLeft));
        for text in ["/quit\n/sit", "\t/quit", "/quit\r", "/quit\0"] {
            h.send(commit(text));
            assert_eq!(h.draft().command.text, "/sit");
            assert!(h.draft().command_selected);
            assert!(h.draft().error.is_some());
            assert!(h.state.bindings().iter().all(Option::is_none));
        }
    }

    #[test]
    fn insertion_and_preedit_obey_bytes_scalars_and_remaining_capacity() {
        let mut h = Harness::new(true);
        h.send(commit(&"é".repeat(40)));
        assert_eq!(h.draft().label.text, "é".repeat(32));
        h.send(preedit("overflow"));
        assert!(h.draft().label.preedit.is_empty());
        h.send(commit("overflow"));
        assert_eq!(h.draft().label.text.len(), 64);
        h.state.clear_draft();
        h.send(commit(&"界".repeat(40)));
        assert_eq!(h.draft().label.text, "界".repeat(21));
        h.send(down(KeyCode::Tab, None));
        h.send(commit(&format!("/say {}", "界".repeat(200))));
        assert_eq!(h.draft().command.text.len(), MAX_CHAT_BYTES);
        assert!(
            h.draft()
                .command
                .text
                .is_char_boundary(h.draft().command.cursor)
        );
        h.state.clear_draft();
        h.send(preedit(&"界".repeat(500)));
        assert_eq!(h.draft().label.preedit, "界".repeat(21));
    }

    #[test]
    fn composition_blocks_save_and_tab_even_after_empty_preedit_until_commit() {
        let mut h = Harness::new(true);
        h.send(preedit("漢"));
        assert!(h.send(down(KeyCode::Enter, None)).action.is_none());
        h.send(down(KeyCode::Tab, None));
        assert_eq!(h.draft().focus, HotbuttonField::Label);
        h.send(preedit(""));
        h.send(up(KeyCode::Enter));
        assert!(h.send(down(KeyCode::Enter, None)).action.is_none());
        h.send(commit("漢"));
        assert_eq!(h.draft().label.text, "漢");
        assert!(h.draft().label.preedit.is_empty());
        h.send(up(KeyCode::Enter));
        assert!(h.send(down(KeyCode::Enter, None)).action.is_some());
    }

    #[test]
    fn field_click_and_context_change_cancel_late_commits_but_fresh_preedit_works() {
        let mut h = Harness::new(true);
        h.send(preedit("old label"));
        let focus = h.click(35., MouseButton::Left);
        h.apply(focus);
        assert_eq!(h.draft().focus, HotbuttonField::Command);
        h.send(commit("old label"));
        assert!(h.draft().command.text.is_empty());
        assert!(h.draft().label.preedit.is_empty());
        h.send(preedit("new command"));
        h.send(commit("/sit"));
        assert_eq!(h.draft().command.text, "/sit");
        h.send(preedit("stale"));
        h.state.sync_context(8, false);
        assert!(h.send(commit("stale")).captured);
        assert!(h.state.editor.is_none());
        h.state.sync_context(8, true);
        h.state.open_editor(h.state.token(), 0);
        h.send(preedit("fresh"));
        h.send(commit("Fresh"));
        assert_eq!(h.draft().label.text, "Fresh");
    }

    #[test]
    fn escape_cancels_composition_first_and_filters_late_commit_after_handoff() {
        let mut h = Harness::new(true);
        h.send(preedit("pending"));
        let clear = h.send(down(KeyCode::Escape, None));
        assert!(clear.escape_handled && clear.action.is_none());
        assert!(h.draft().label.preedit.is_empty());
        assert!(h.send(repeat(KeyCode::Escape, None)).action.is_none());
        h.send(up(KeyCode::Escape));
        let close = h.send(down(KeyCode::Escape, None));
        assert!(close.escape_handled);
        h.apply(close);
        assert!(h.state.editor.is_none());
        assert!(
            h.input
                .filter_handoff_event(&commit("pending"), window(), &mut h.keys)
        );
        assert!(
            !h.input
                .filter_handoff_event(&preedit("new owner"), window(), &mut h.keys)
        );
        assert!(
            !h.input
                .filter_handoff_event(&commit("new owner"), window(), &mut h.keys)
        );
    }

    #[test]
    fn inactive_route_relinquishes_cancelled_ime_to_fresh_chat_composition() {
        use crate::{chat::ChatEditor, input::ChatInput};
        let mut h = Harness::new(true);
        h.send(preedit("retired hotbutton"));
        h.state.cancel_editor();
        let mut chat = ChatEditor::default();
        let mut chat_input = ChatInput::default();
        chat.open("");
        for event in [preedit("新"), commit("新しいチャット")] {
            let result = h.send(event.clone());
            assert!(!result.captured && result.action.is_none());
            assert!(
                chat_input
                    .route_event(&mut chat, window(), &event, &mut h.keys)
                    .captured
            );
        }
        assert_eq!(chat.text, "新しいチャット");
        assert!(h.state.editor.is_none());
    }

    #[test]
    fn focus_loss_and_foreign_window_events_cannot_complete_pointer_or_composition() {
        let mut h = Harness::new(true);
        h.valid_draft();
        h.send(preedit("pending"));
        let other = Entity::from_raw_u32(2).unwrap();
        assert!(
            !h.send(WindowEvent::Ime(Ime::Commit {
                window: other,
                value: "foreign".into()
            }))
            .captured
        );
        assert_eq!(h.draft().label.text, "Sit");
        h.move_to(85.);
        h.send(mouse(MouseButton::Left, ButtonState::Pressed));
        h.send(WindowEvent::WindowFocused(WindowFocused {
            window: window(),
            focused: false,
        }));
        assert!(h.input.pressed().is_none());
        assert!(h.draft().label.preedit.is_empty());
        h.send(WindowEvent::WindowFocused(WindowFocused {
            window: window(),
            focused: true,
        }));
        assert!(
            h.send(mouse(MouseButton::Left, ButtonState::Released))
                .action
                .is_none()
        );
        h.send(commit("pending"));
        assert_eq!(h.draft().label.text, "Sit");
    }
}
