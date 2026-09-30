//! Original-skin account screens and an isolated, ordered credential editor.
//! `None` from `event` still consumes input: callers must route account events
//! exclusively, then suppress held keys before returning to gameplay.
use crate::account::{Endpoint, Stage, Token, View};
use bevy::{
    input::{ButtonState, mouse::MouseScrollUnit},
    prelude::{ButtonInput, Entity, KeyCode, MouseButton},
    window::{Ime, WindowEvent},
};
use openeq_ui::{Color, DrawCommand, HitTarget, Rect, TextAlign, UiBindings, UiDocument, UiFrame};
use std::{collections::HashSet, path::Path};

mod menu;
mod preview;
use menu::{Navigation, Page};
use preview::PreviewChoice;

const MAX_SERVERS: usize = 2048;
const MAX_CHARACTERS: usize = 256;
const VISIBLE_ROWS: usize = 8;
const WHITE: Color = [235, 231, 220, 255];
const GOLD: Color = [221, 190, 124, 255];
const MUTED: Color = [160, 168, 181, 255];

/// A modal must retain physical ownership even though Bevy button state is
/// cleared each frame. Release-aware raw events survive an empty handoff frame.
#[derive(Default)]
pub struct CampInput {
    held: HashSet<KeyCode>,
}
impl CampInput {
    pub fn observe(&mut self, window: Entity, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if event.state == ButtonState::Pressed {
                    self.held.insert(event.key_code);
                } else {
                    self.held.remove(&event.key_code);
                }
            }
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.held.clear()
            }
            _ => {}
        }
    }
    pub fn capture_pressed(&mut self, keys: &ButtonInput<KeyCode>) {
        self.held.extend(keys.get_pressed().copied());
    }
    pub fn handoff(&mut self, account: &mut AccountInput) {
        account.returning_held.extend(self.held.drain());
        account
            .captured
            .extend(account.returning_held.iter().copied());
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    SignIn,
    Cancel,
    Exit,
    SelectWorld { token: Token, id: u32 },
    SelectCharacter { token: Token, name: String },
    Play { token: Token },
    Refresh { token: Token },
    Back { token: Token },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Host,
    LoginPort,
    WorldPort,
    Username,
    Password,
    List,
    Primary,
    Connection,
    Refresh,
    Preview,
    RotateLeft,
    RotateRight,
    Back,
    Exit,
}
impl Focus {
    fn field(self) -> Option<usize> {
        match self {
            Self::Host => Some(0),
            Self::LoginPort => Some(1),
            Self::WorldPort => Some(2),
            Self::Username => Some(3),
            Self::Password => Some(4),
            _ => None,
        }
    }
}
const CREDENTIAL_FOCUS: &[Focus] = &[
    Focus::Host,
    Focus::LoginPort,
    Focus::WorldPort,
    Focus::Username,
    Focus::Password,
    Focus::Primary,
    Focus::Exit,
];
const WORLD_FOCUS: &[Focus] = &[Focus::List, Focus::Primary, Focus::Refresh, Focus::Back];
const CHARACTER_FOCUS: &[Focus] = &[Focus::List, Focus::Preview, Focus::Primary, Focus::Back];

// Intentionally no Debug/Display/Serialize on any editor state. Passwords never
// enter UiBindings, draw commands, hit IDs, chat history, notices or diagnostics.
#[derive(Default)]
struct Edit {
    text: String,
    cursor: usize,
    all_selected: bool,
    preedit: String,
}
impl Edit {
    fn new(text: String) -> Self {
        Self {
            cursor: text.len(),
            text,
            ..Default::default()
        }
    }
    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.preedit.clear();
        self.all_selected = false;
    }
    fn insert(&mut self, text: &str, limit: usize, digits: bool) {
        if self.all_selected {
            self.clear();
        }
        for ch in text
            .chars()
            .filter(|ch| !ch.is_control() && (!digits || ch.is_ascii_digit()))
        {
            if self.text.len() + ch.len_utf8() > limit {
                break;
            }
            self.text.insert(self.cursor, ch);
            self.cursor += ch.len_utf8();
        }
    }
    fn left(&mut self) {
        self.cursor = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
        self.all_selected = false;
    }
    fn right(&mut self) {
        self.cursor += self.text[self.cursor..]
            .chars()
            .next()
            .map_or(0, char::len_utf8);
        self.all_selected = false;
    }
    fn backspace(&mut self) {
        if self.all_selected {
            self.clear();
            return;
        }
        let end = self.cursor;
        self.left();
        self.text.replace_range(self.cursor..end, "");
    }
    fn delete(&mut self) {
        if self.all_selected {
            self.clear();
            return;
        }
        let end = self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        self.text.replace_range(self.cursor..end, "");
    }
    fn display(&self, secret: bool, focused: bool, blink: bool, capacity: usize) -> String {
        // A small cursor-centered window keeps editing bounded and visible.
        // Mask before constructing any presentation text, including IME text.
        let before = self.text[..self.cursor].chars().count();
        let chars: Vec<char> = self
            .text
            .chars()
            .map(|c| if secret { '•' } else { c })
            .collect();
        let start = before.saturating_sub(capacity.saturating_sub(2));
        let mut out = String::new();
        if start > 0 {
            out.push('…');
        }
        for ch in &chars[start..before] {
            out.push(*ch);
        }
        if focused {
            for ch in self
                .preedit
                .chars()
                .take(capacity.saturating_sub(before - start))
            {
                out.push(if secret { '•' } else { ch });
            }
            if blink {
                out.push('|');
            }
        }
        for ch in chars[before..]
            .iter()
            .take(capacity.saturating_sub(before - start))
        {
            out.push(*ch);
        }
        out
    }
}

pub struct AccountInput {
    edits: [Edit; 5],
    focus: Focus,
    focused: bool,
    modifiers: ButtonInput<KeyCode>,
    captured: HashSet<KeyCode>,
    handoff_replay: HashSet<KeyCode>,
    returning_held: HashSet<KeyCode>,
    pointer: Option<[f32; 2]>,
    pressed: Option<String>,
    seen: Option<(Token, Stage)>,
    first: usize,
    notice: Option<&'static str>,
    cancelled_composition: bool,
    composition_field: Option<usize>,
    navigation: Option<Navigation>,
    preview: Option<PreviewChoice>,
    preview_revision: u64,
    preview_held: HashSet<KeyCode>,
}
impl AccountInput {
    pub fn new(endpoint: Endpoint) -> Self {
        Self {
            edits: [
                Edit::new(bounded_text(&endpoint.host, 253)),
                Edit::new(endpoint.login_port.to_string()),
                Edit::new(endpoint.world_port.to_string()),
                Edit::default(),
                Edit::default(),
            ],
            focus: Focus::Username,
            focused: true,
            modifiers: ButtonInput::default(),
            captured: HashSet::new(),
            handoff_replay: HashSet::new(),
            returning_held: HashSet::new(),
            pointer: None,
            pressed: None,
            seen: None,
            first: 0,
            notice: None,
            cancelled_composition: false,
            composition_field: None,
            navigation: None,
            preview: None,
            preview_revision: 0,
            preview_held: HashSet::new(),
        }
    }
    /// Starts at the local main menu. The controller remains idle until the
    /// existing credentials page emits SignIn; no connection starts here.
    pub fn with_main_menu(endpoint: Endpoint) -> Self {
        Self {
            navigation: Some(Navigation::default()),
            focus: Focus::Primary,
            ..Self::new(endpoint)
        }
    }
    /// Moves the secret to the connection controller and immediately clears the
    /// editor, including composition. Call only in response to `SignIn`.
    pub fn take_credentials(&mut self) -> (Endpoint, String, String) {
        let endpoint = self.endpoint();
        let username = self.edits[3].text.clone();
        let password = std::mem::take(&mut self.edits[4].text);
        self.edits[4].clear();
        (endpoint, username, password)
    }
    fn endpoint(&self) -> Endpoint {
        Endpoint {
            host: self.edits[0].text.trim().into(),
            login_port: self.edits[1].text.parse().unwrap_or(0),
            world_port: self.edits[2].text.parse().unwrap_or(0),
        }
    }
    /// Clear transient input and secrets on cancellation, shutdown or handoff.
    pub fn reset(&mut self) {
        self.cancel_composition();
        self.edits[4].clear();
        self.modifiers.reset_all();
        self.captured.clear();
        self.handoff_replay.clear();
        self.returning_held.clear();
        self.preview = None;
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.preview_held.clear();
        if let Some(navigation) = &mut self.navigation {
            navigation.held_keys.clear();
        }
        self.pressed = None;
        self.pointer = None;
        self.notice = None;
    }
    /// Inputs held in the departing world remain inert until their release.
    pub fn suppress_held_keys(&mut self, keys: &ButtonInput<KeyCode>) {
        self.returning_held.extend(keys.get_pressed().copied());
        self.captured.extend(self.returning_held.iter().copied());
    }
    fn cancel_composition(&mut self) {
        self.cancelled_composition |= self.composition_field.take().is_some();
        for edit in &mut self.edits {
            edit.preedit.clear();
        }
    }
    pub fn ime_enabled(&self, view: &View) -> bool {
        self.focused && self.editable_field(view).is_some()
    }
    /// Begin exactly once before routing a gameplay frame's raw events. Bevy's
    /// ButtonInput already describes the entire batch, so clear both held state
    /// and edges for account-owned keys, even if a release follows a repeat.
    pub fn begin_handoff_frame(&mut self, keys: &mut ButtonInput<KeyCode>) {
        self.handoff_replay.clone_from(&self.captured);
        self.suppress_captured_keys(keys);
    }
    /// Route exactly once, in native order, before ChatInput or any raw-key
    /// consumer. `true` means the event belongs to account UI and must be skipped.
    /// A release retires ownership; a fresh press later in the same batch is
    /// allowed and restores its held/edge state, which `begin` initially cleared.
    pub fn filter_handoff_event(
        &mut self,
        event: &WindowEvent,
        window: Entity,
        keys: &mut ButtonInput<KeyCode>,
    ) -> bool {
        match event {
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if self.captured.contains(&event.key_code) {
                    if event.state == ButtonState::Released {
                        self.captured.remove(&event.key_code);
                        self.returning_held.remove(&event.key_code);
                        self.modifiers.release(event.key_code);
                    }
                    return true;
                }
                // Only these keys were cleared by begin. Other keys retain
                // Bevy's ordinary whole-frame state and focus recovery.
                if self.handoff_replay.contains(&event.key_code) {
                    match event.state {
                        ButtonState::Pressed => keys.press(event.key_code),
                        ButtonState::Released => keys.release(event.key_code),
                    }
                }
            }
            WindowEvent::WindowFocused(event) if event.window == window && !event.focused => {
                self.captured.clear();
                self.returning_held.clear();
                self.modifiers.reset_all();
                for key in &self.handoff_replay {
                    keys.reset(*key);
                }
            }
            _ => {}
        }
        false
    }
    /// Clear still-owned physical keys. Handoff additionally requires the
    /// begin/filter pair to suppress raw events and released keys' stale edges.
    pub fn suppress_captured_keys(&self, keys: &mut ButtonInput<KeyCode>) {
        for key in &self.captured {
            keys.reset(*key);
        }
    }
    fn sync(&mut self, view: &View) {
        if self.preview.is_some() && self.preview_character(view).is_none() {
            self.close_preview();
        }
        if self.seen != Some((view.token, view.stage)) {
            self.cancel_composition();
            if view.stage != Stage::Credentials {
                self.edits[4].clear();
            }
            self.first = 0;
            self.pressed = None;
            self.notice = None;
            self.focus = match view.stage {
                Stage::Credentials => match self.page(view) {
                    Page::Welcome => Focus::Primary,
                    Page::Connection => Focus::Host,
                    Page::Credentials => Focus::Username,
                },
                Stage::Worlds | Stage::Characters => Focus::List,
                _ => Focus::Back,
            };
            self.seen = Some((view.token, view.stage));
        }
    }
    fn first(&self, view: &View) -> usize {
        if self
            .seen
            .is_some_and(|seen| seen != (view.token, view.stage))
        {
            return 0;
        }
        self.first.min(row_count(view).saturating_sub(VISIBLE_ROWS))
    }
    /// Feed the combined WindowEvent stream in native order. Every event belongs
    /// to the account screen while it is active, even if no Intent is produced.
    pub fn event(
        &mut self,
        view: &View,
        frame: &UiFrame,
        window: Entity,
        event: &WindowEvent,
    ) -> Option<Intent> {
        self.sync(view);
        match event {
            WindowEvent::WindowFocused(event) if event.window == window => {
                self.focused = event.focused;
                if !event.focused {
                    self.modifiers.reset_all();
                    self.captured.clear();
                    self.returning_held.clear();
                    self.preview_held.clear();
                    if let Some(navigation) = &mut self.navigation {
                        navigation.held_keys.clear();
                    }
                    self.pressed = None;
                    self.pointer = None;
                    self.cancel_composition();
                }
            }
            WindowEvent::CursorMoved(event) if event.window == window && self.focused => {
                self.pointer = Some([event.position.x, event.position.y]);
            }
            WindowEvent::CursorLeft(event) if event.window == window => {
                self.pointer = None;
                self.pressed = None;
            }
            WindowEvent::KeyboardInput(event) if event.window == window => {
                if event.state == ButtonState::Released {
                    self.modifiers.release(event.key_code);
                    self.captured.remove(&event.key_code);
                    self.returning_held.remove(&event.key_code);
                    self.preview_held.remove(&event.key_code);
                    if let Some(navigation) = &mut self.navigation {
                        navigation.held_keys.remove(&event.key_code);
                    }
                    return None;
                }
                if self.returning_held.contains(&event.key_code) {
                    return None;
                }
                self.captured.insert(event.key_code);
                if !self.focused {
                    return None;
                }
                self.modifiers.press(event.key_code);
                if self.preview_held.contains(&event.key_code)
                    || self
                        .navigation
                        .as_ref()
                        .is_some_and(|navigation| navigation.held_keys.contains(&event.key_code))
                {
                    return None;
                }
                if !current_frame(view, self, frame) {
                    return None;
                }
                let command = [
                    KeyCode::ControlLeft,
                    KeyCode::ControlRight,
                    KeyCode::SuperLeft,
                    KeyCode::SuperRight,
                ]
                .iter()
                .any(|key| self.modifiers.pressed(*key));
                let shift = self.modifiers.pressed(KeyCode::ShiftLeft)
                    || self.modifiers.pressed(KeyCode::ShiftRight);
                if event.key_code == KeyCode::Escape && !event.repeat {
                    if self.composition_field.is_some() {
                        self.cancel_composition();
                        return None;
                    }
                    return self.back(view);
                }
                if event.key_code == KeyCode::Tab && !event.repeat {
                    if self.composition_field.is_some() {
                        // Let the input method finish before traversing. A
                        // late password commit must never reach another field.
                        return None;
                    }
                    let order = if self.preview_character(view).is_some() {
                        &[Focus::Back, Focus::RotateLeft, Focus::RotateRight][..]
                    } else {
                        self.focus_order(view)
                    };
                    let index = order.iter().position(|f| *f == self.focus).unwrap_or(0);
                    self.focus =
                        order[(index + if shift { order.len() - 1 } else { 1 }) % order.len()];
                    return None;
                }
                if self.preview_character(view).is_some() {
                    self.preview_key(event.key_code, event.repeat);
                    return None;
                }
                if let Some(index) = self.editable_field(view) {
                    if !self.edits[index].preedit.is_empty() {
                        return None;
                    }
                    self.notice = None;
                    let edit = &mut self.edits[index];
                    match event.key_code {
                        KeyCode::Enter | KeyCode::NumpadEnter if !event.repeat => {
                            return self.primary(view);
                        }
                        KeyCode::Enter | KeyCode::NumpadEnter => {}
                        KeyCode::Backspace => edit.backspace(),
                        KeyCode::Delete => edit.delete(),
                        KeyCode::ArrowLeft => edit.left(),
                        KeyCode::ArrowRight => edit.right(),
                        KeyCode::Home => {
                            edit.cursor = 0;
                            edit.all_selected = false;
                        }
                        KeyCode::End => {
                            edit.cursor = edit.text.len();
                            edit.all_selected = false;
                        }
                        KeyCode::KeyA if command => edit.all_selected = true,
                        KeyCode::KeyU if command => edit.clear(),
                        _ if !command => {
                            if let Some(text) = &event.text {
                                edit.insert(text, field_limit(index), index == 1 || index == 2);
                            }
                        }
                        _ => {}
                    }
                } else {
                    match event.key_code {
                        KeyCode::ArrowUp | KeyCode::ArrowDown
                            if matches!(view.stage, Stage::Worlds | Stage::Characters) =>
                        {
                            self.focus = Focus::List;
                            return self.step(view, event.key_code == KeyCode::ArrowDown);
                        }
                        KeyCode::PageUp => self.scroll(view, -(VISIBLE_ROWS as isize)),
                        KeyCode::PageDown => self.scroll(view, VISIBLE_ROWS as isize),
                        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space if !event.repeat => {
                            return match self.focus {
                                Focus::Refresh if view.stage == Stage::Worlds => {
                                    Some(Intent::Refresh { token: view.token })
                                }
                                Focus::Back => self.back(view),
                                Focus::Preview => {
                                    self.open_preview(view);
                                    None
                                }
                                Focus::Connection if self.page(view) == Page::Welcome => {
                                    self.navigate(Page::Connection);
                                    None
                                }
                                Focus::Exit => {
                                    self.reset();
                                    Some(Intent::Exit)
                                }
                                _ => self.primary(view),
                            };
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::Ime(Ime::Preedit {
                window: id, value, ..
            }) if *id == window && self.focused && current_frame(view, self, frame) => {
                if let Some(index) = self.editable_field(view) {
                    if !value.is_empty() {
                        self.cancelled_composition = false;
                        self.composition_field = Some(index);
                    }
                    self.edits[index].preedit = bounded_text(value, field_limit(index));
                }
            }
            WindowEvent::Ime(Ime::Commit { window: id, value })
                if *id == window && self.focused && current_frame(view, self, frame) =>
            {
                if self.cancelled_composition {
                    self.cancelled_composition = false;
                    return None;
                }
                if let Some(owner) = self.composition_field.take()
                    && self.focus.field() != Some(owner)
                {
                    return None;
                }
                if let Some(index) = self.editable_field(view) {
                    self.edits[index].preedit.clear();
                    self.edits[index].insert(value, field_limit(index), index == 1 || index == 2);
                    self.notice = None;
                }
            }
            WindowEvent::Ime(Ime::Disabled { window: id }) if *id == window => {
                self.cancel_composition();
            }
            WindowEvent::MouseWheel(event)
                if event.window == window && self.focused && current_frame(view, self, frame) =>
            {
                if self.pointer.is_some_and(|point| {
                    frame
                        .hit_targets
                        .iter()
                        .any(|h| h.kind == "AccountList" && h.rect.contains(point))
                }) {
                    let amount = match event.unit {
                        MouseScrollUnit::Line => event.y,
                        MouseScrollUnit::Pixel => event.y / 24.,
                    };
                    if amount.is_finite() && amount != 0. {
                        self.scroll(view, -(amount.signum() as isize) * 3);
                    }
                }
            }
            WindowEvent::MouseButtonInput(event)
                if event.window == window
                    && event.button == MouseButton::Left
                    && self.focused
                    && current_frame(view, self, frame) =>
            {
                let hit = self.pointer.and_then(|point| frame.hit_test(point));
                if event.state == ButtonState::Pressed {
                    self.pressed = hit.map(|h| h.item.clone());
                    if let Some(hit) = hit
                        && let Some(action) = action_suffix(view, self, &hit.item)
                        && let Some(field) = action
                            .strip_prefix("field:")
                            .and_then(|s| s.parse::<usize>().ok())
                            .filter(|i| *i < 5)
                    {
                        self.cancel_composition();
                        self.focus = CREDENTIAL_FOCUS[field];
                        self.edits[field].cursor = self.edits[field].text.len();
                        self.edits[field].all_selected = false;
                    }
                } else if let Some(pressed) = self.pressed.take()
                    && let Some(hit) = hit.filter(|h| h.item == pressed)
                {
                    let action = action_suffix(view, self, &hit.item)?;
                    if self.preview_character(view).is_some() {
                        self.preview_action(action);
                        return None;
                    }
                    if let Some(index) = action
                        .strip_prefix("row:")
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        self.focus = Focus::List;
                        return row_intent(view, index);
                    }
                    return match action {
                        "primary" => {
                            self.focus = Focus::Primary;
                            self.primary(view)
                        }
                        "refresh" if view.stage == Stage::Worlds => {
                            self.focus = Focus::Refresh;
                            Some(Intent::Refresh { token: view.token })
                        }
                        "back" => {
                            self.focus = Focus::Back;
                            self.back(view)
                        }
                        "preview" => {
                            self.open_preview(view);
                            None
                        }
                        "connection" if self.page(view) == Page::Welcome => {
                            self.navigate(Page::Connection);
                            None
                        }
                        "exit" => {
                            self.reset();
                            Some(Intent::Exit)
                        }
                        _ => None,
                    };
                }
            }
            _ => {}
        }
        None
    }
    fn sign_in(&mut self) -> Option<Intent> {
        if self.composition_field.is_some() {
            return None;
        }
        self.notice = if self.endpoint().validate().is_err() {
            Some("Enter a hostname and ports between 1 and 65535.")
        } else if self.edits[3].text.is_empty() {
            Some("Enter your account username.")
        } else if self.edits[4].text.is_empty() {
            Some("Enter your account password.")
        } else {
            None
        };
        self.notice.is_none().then_some(Intent::SignIn)
    }
    fn primary(&mut self, view: &View) -> Option<Intent> {
        if self.preview_character(view).is_some() {
            return None;
        }
        match view.stage {
            Stage::Credentials => match self.page(view) {
                Page::Welcome => {
                    self.navigate(Page::Credentials);
                    None
                }
                Page::Connection => {
                    if self.composition_field.is_some() {
                        return None;
                    }
                    if self.endpoint().validate().is_err() {
                        self.notice = Some("Enter a hostname and ports between 1 and 65535.");
                    } else {
                        if let Some(navigation) = &mut self.navigation {
                            navigation.connection_snapshot = None;
                        }
                        self.navigate(Page::Welcome);
                    }
                    None
                }
                Page::Credentials => self.sign_in(),
            },
            Stage::Worlds | Stage::Characters if playable(view) => {
                Some(Intent::Play { token: view.token })
            }
            _ => None,
        }
    }
    fn back(&mut self, view: &View) -> Option<Intent> {
        if self.preview_character(view).is_some() {
            self.close_preview();
            return None;
        }
        if view.stage == Stage::Credentials
            && self.navigation.is_some()
            && self.page(view) != Page::Welcome
        {
            self.restore_connection();
            self.navigate(Page::Welcome);
            return None;
        }
        self.reset();
        Some(match view.stage {
            Stage::Credentials => Intent::Exit,
            Stage::Characters => Intent::Back { token: view.token },
            _ => Intent::Cancel,
        })
    }
    fn scroll(&mut self, view: &View, amount: isize) {
        if self.preview_character(view).is_some() {
            return;
        }
        self.first = self
            .first(view)
            .saturating_add_signed(amount)
            .min(row_count(view).saturating_sub(VISIBLE_ROWS));
        self.pressed = None;
    }
    fn step(&mut self, view: &View, down: bool) -> Option<Intent> {
        let count = row_count(view);
        let selected = selected_index(view);
        let start = selected.map_or(if down { 0 } else { count.saturating_sub(1) }, |i| {
            if down { i + 1 } else { i.saturating_sub(1) }
        });
        for offset in 0..count {
            let index = if down {
                start.checked_add(offset)?
            } else {
                start.checked_sub(offset)?
            };
            if index >= count {
                return None;
            }
            if let Some(intent) = row_intent(view, index) {
                if index < self.first {
                    self.first = index;
                }
                if index >= self.first + VISIBLE_ROWS {
                    self.first = index + 1 - VISIBLE_ROWS;
                }
                return Some(intent);
            }
        }
        None
    }
}

fn field_limit(index: usize) -> usize {
    match index {
        0 => 253,
        1 | 2 => 5,
        3 => 128,
        _ => 512,
    }
}
fn bounded_text(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|ch| !ch.is_control())
        .scan(0, |bytes, ch| {
            *bytes += ch.len_utf8();
            (*bytes <= limit).then_some(ch)
        })
        .collect()
}
fn focus_order(stage: Stage) -> &'static [Focus] {
    match stage {
        Stage::Credentials => CREDENTIAL_FOCUS,
        Stage::Worlds => WORLD_FOCUS,
        Stage::Characters => CHARACTER_FOCUS,
        _ => &[Focus::Back],
    }
}
fn row_count(view: &View) -> usize {
    match view.stage {
        Stage::Worlds => view.servers.len().min(MAX_SERVERS),
        Stage::Characters => view.characters.len().min(MAX_CHARACTERS),
        _ => 0,
    }
}
fn selected_index(view: &View) -> Option<usize> {
    match view.stage {
        Stage::Worlds => view
            .servers
            .iter()
            .take(MAX_SERVERS)
            .position(|s| Some(s.server_id) == view.selected_world),
        Stage::Characters => view
            .characters
            .iter()
            .take(MAX_CHARACTERS)
            .position(|c| Some(&c.name) == view.selected_character.as_ref()),
        _ => None,
    }
}
fn playable(view: &View) -> bool {
    if view
        .creation
        .as_ref()
        .is_some_and(|creation| creation.pending())
    {
        return false;
    }
    selected_index(view)
        .and_then(|index| row_intent(view, index))
        .is_some()
}
fn row_intent(view: &View, index: usize) -> Option<Intent> {
    if index >= row_count(view) {
        return None;
    }
    match view.stage {
        Stage::Worlds => {
            view.servers
                .get(index)
                .filter(|s| s.is_up())
                .map(|s| Intent::SelectWorld {
                    token: view.token,
                    id: s.server_id,
                })
        }
        Stage::Characters => {
            view.characters
                .get(index)
                .filter(|c| c.enabled)
                .map(|c| Intent::SelectCharacter {
                    token: view.token,
                    name: c.name.clone(),
                })
        }
        _ => None,
    }
}
fn prefix(view: &View, input: &AccountInput) -> String {
    format!(
        "account:{}:{}:{}:{}:{}:",
        view.token.attempt,
        view.token.revision,
        view.stage as u8,
        input
            .navigation
            .as_ref()
            .map_or(0, |navigation| navigation.revision),
        input.preview_revision,
    )
}
fn action_suffix<'a>(view: &View, input: &AccountInput, id: &'a str) -> Option<&'a str> {
    id.strip_prefix(&prefix(view, input))
}
fn current_frame(view: &View, input: &AccountInput, frame: &UiFrame) -> bool {
    frame
        .hit_targets
        .first()
        .is_some_and(|h| h.item == format!("{}frame", prefix(view, input)))
}

#[derive(Default)]
pub struct AccountUi {
    login: Option<UiDocument>,
    characters: Option<UiDocument>,
}
impl AccountUi {
    /// Missing optional skin files fall back to usable local controls.
    pub fn load(client_dir: &Path) -> Self {
        let directory = client_dir.join("uifiles/default");
        let mut characters = UiDocument::load(&directory, "EQUI.xml").ok();
        // Character-select button decals contain baked text. Preserve the
        // original button frame, replacing only those labels with ours so
        // "Back" and "Enter world" do not draw over "Quit"/"Enter World".
        if let Some(document) = &mut characters {
            for name in ["CLW_Quit_Button", "CLW_Play_Button"] {
                if let Some(button) = document.definitions.get_mut(name)
                    && let Some(template) = button
                        .children
                        .iter_mut()
                        .find(|c| c.kind == "ButtonDrawTemplate")
                {
                    template.children.retain(|c| !c.kind.ends_with("Decal"));
                }
            }
        }
        Self {
            login: UiDocument::load(&directory, "EQLSUI.xml").ok(),
            characters,
        }
    }
    pub fn frame(
        &self,
        viewport: [u32; 2],
        view: &View,
        input: &AccountInput,
        elapsed: f32,
    ) -> UiFrame {
        self.frame_with_preview_status(viewport, view, input, elapsed, None)
    }

    pub fn frame_with_preview_status(
        &self,
        viewport: [u32; 2],
        view: &View,
        input: &AccountInput,
        elapsed: f32,
        preview_status: Option<&str>,
    ) -> UiFrame {
        let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
        let mut paint = Paint {
            frame: UiFrame {
                bounds: screen,
                ..Default::default()
            },
            screen,
            ui: self,
            view,
            prefix: prefix(view, input),
        };
        paint.hit("frame", "AccountCapture", screen, true);
        if screen.is_empty() {
            return paint.frame;
        }
        if let Some(character) = input.preview_character(view) {
            paint.preview(character, input, preview_status);
            return paint.frame;
        }
        paint.fill(screen, [9, 13, 20, 255]);
        // Authored skins are 640x480 logical pixels. Reflow the content inside
        // a bounded centered panel, preserving readable fonts on smaller views.
        let panel = Rect::new(
            ((screen.width - 640.) * 0.5).max(0.),
            ((screen.height - 480.) * 0.5).max(0.),
            screen.width.min(640.),
            screen.height.min(480.),
        );
        let chars = matches!(view.stage, Stage::Characters | Stage::EnteringZone);
        let root = if chars {
            "CharacterListWnd"
        } else if matches!(view.stage, Stage::Worlds | Stage::JoiningWorld) {
            "serverselect"
        } else if input.page(view) == Page::Welcome {
            "main"
        } else {
            "connect"
        };
        paint.skin(root, panel, "", true, false, false, true);
        if chars {
            paint.skin("CLW_ButtonsScreen", panel, "", true, false, false, false);
        }
        let inner = Rect::new(
            panel.x + 20.,
            panel.y + 20.,
            (panel.width - 40.).max(0.),
            (panel.height - 40.).max(0.),
        );
        paint.fill(inner, [9, 13, 20, 205]);
        paint.text(
            Rect::new(inner.x + 14., inner.y + 8., inner.width - 28., 32.),
            input.title(view),
            5,
            GOLD,
            false,
        );
        let time = if elapsed.is_finite() {
            elapsed.max(0.)
        } else {
            0.
        };
        if view.stage.busy() {
            let dots = ".".repeat((time * 2.) as usize % 4);
            paint.text(
                Rect::new(inner.x + 14., inner.y + 110., inner.width - 28., 40.),
                &format!("{}{dots}", view.stage.label()),
                4,
                WHITE,
                false,
            );
            paint.button(
                "LOGIN_CancelButton",
                "back",
                Rect::new(inner.x + 14., inner.bottom() - 54., 140., 32.),
                "Cancel",
                true,
                input.focus == Focus::Back,
                input,
            );
        } else if view.stage == Stage::Credentials {
            match input.page(view) {
                Page::Welcome => paint.welcome(inner, input),
                Page::Connection => paint.connection(inner, input, time),
                Page::Credentials => paint.credentials(inner, input, time),
            }
        } else {
            paint.list(inner, input);
        }
        if let Some(notice) = input.notice.or_else(|| {
            (input.page(view) == Page::Credentials)
                .then_some(view.notice.as_deref())
                .flatten()
        }) {
            let compact_connection = input.page(view) == Page::Connection && inner.height < 300.;
            paint.text(
                Rect::new(
                    inner.x + 14.,
                    inner.bottom() - if compact_connection { 64. } else { 88. },
                    inner.width - 28.,
                    if compact_connection { 20. } else { 32. },
                ),
                &bounded_text(notice, 512),
                2,
                [244, 185, 155, 255],
                true,
            );
        }
        paint.text(
            Rect::new(inner.x + 14., inner.bottom() - 15., inner.width - 28., 15.),
            if input.page(view) == Page::Welcome {
                "Tab to move · Enter to choose · Escape to exit"
            } else {
                "Tab to move · Enter to continue · Escape to go back"
            },
            1,
            MUTED,
            false,
        );
        paint.frame.warnings.sort();
        paint.frame.warnings.dedup();
        paint.frame
    }
}

struct Paint<'a> {
    frame: UiFrame,
    screen: Rect,
    ui: &'a AccountUi,
    view: &'a View,
    prefix: String,
}
impl Paint<'_> {
    fn fill(&mut self, rect: Rect, color: Color) {
        let clip = rect.intersect(self.screen);
        if !clip.is_empty() {
            self.frame
                .commands
                .push(DrawCommand::Fill { rect, clip, color });
        }
    }
    fn text(&mut self, rect: Rect, text: &str, font: u32, color: Color, wrap: bool) {
        let clip = rect.intersect(self.screen);
        if !clip.is_empty() {
            self.frame.commands.push(DrawCommand::Text {
                rect,
                clip,
                text: text.into(),
                font,
                color,
                align: TextAlign::Left,
                vertical_center: false,
                wrap,
            });
        }
    }
    fn hit(&mut self, action: &str, kind: &str, rect: Rect, enabled: bool) {
        self.frame.hit_targets.push(HitTarget {
            item: format!("{}{action}", self.prefix),
            screen_id: action.into(),
            window_id: Some("account".into()),
            kind: kind.into(),
            rect: rect.intersect(self.screen),
            enabled,
            tooltip: None,
        });
    }
    #[allow(clippy::too_many_arguments)]
    fn skin(
        &mut self,
        name: &str,
        rect: Rect,
        text: &str,
        enabled: bool,
        hovered: bool,
        pressed: bool,
        background: bool,
    ) -> bool {
        let doc = if name.starts_with("CLW_") || name == "CharacterListWnd" {
            self.ui.characters.as_ref()
        } else {
            self.ui.login.as_ref()
        };
        let Some(doc) = doc else {
            return false;
        };
        let Some(element) = doc.definition(name) else {
            return false;
        };
        let Ok(window) = doc.window(name) else {
            return false;
        };
        let mut bindings = UiBindings::default();
        for child in element.values("Pieces").chain(element.values("Pages")) {
            bindings.widget_mut(child).visible =
                Some(background && child.starts_with("EQLS_") && child.contains("BG"));
        }
        let state = bindings.widget_mut(name);
        state.rect = Some(rect);
        state.text = Some(text.into());
        state.enabled = Some(enabled);
        state.hovered = hovered;
        state.pressed = pressed;
        let mut frame = window.layout(self.screen, &bindings);
        self.frame.commands.append(&mut frame.commands);
        self.frame.warnings.append(&mut frame.warnings);
        // Application IDs replace XML hits; optional/unimplemented controls
        // never become active just because the original skin contains them.
        true
    }
    fn outline(&mut self, rect: Rect, color: Color) {
        for edge in [
            Rect::new(rect.x, rect.y, rect.width, 1.),
            Rect::new(rect.x, rect.bottom() - 1., rect.width, 1.),
            Rect::new(rect.x, rect.y, 1., rect.height),
            Rect::new(rect.right() - 1., rect.y, 1., rect.height),
        ] {
            self.fill(edge, color);
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn button(
        &mut self,
        template: &str,
        action: &str,
        rect: Rect,
        label: &str,
        enabled: bool,
        focused: bool,
        input: &AccountInput,
    ) {
        let hovered = input.pointer.is_some_and(|point| rect.contains(point));
        let pressed = input.pressed.as_deref() == Some(&format!("{}{action}", self.prefix));
        if !self.skin(template, rect, label, enabled, hovered, pressed, false) {
            self.fill(
                rect,
                if enabled && (hovered || focused) {
                    [63, 57, 40, 255]
                } else {
                    [29, 36, 48, 255]
                },
            );
            self.text(
                Rect::new(rect.x + 8., rect.y + 7., rect.width - 16., rect.height - 7.),
                label,
                3,
                if enabled { WHITE } else { MUTED },
                false,
            );
        }
        if focused && enabled {
            self.outline(rect, GOLD);
        }
        self.hit(action, "AccountButton", rect, enabled);
    }
    fn edit_field(
        &mut self,
        index: usize,
        label: &str,
        rect: Rect,
        input: &AccountInput,
        time: f32,
    ) {
        self.text(
            Rect::new(rect.x, rect.y - 22., rect.width, 20.),
            label,
            2,
            WHITE,
            false,
        );
        self.fill(rect, [8, 13, 20, 245]);
        self.skin(
            if index == 4 {
                "LOGIN_PasswordEdit"
            } else {
                "LOGIN_UsernameEdit"
            },
            rect,
            "",
            true,
            false,
            false,
            false,
        );
        let focused = input.focus.field() == Some(index) && input.focused;
        if focused {
            self.outline(rect, GOLD);
        }
        if focused && input.edits[index].all_selected {
            self.fill(
                Rect::new(
                    rect.x + 5.,
                    rect.y + 5.,
                    rect.width - 10.,
                    rect.height - 10.,
                ),
                [61, 66, 92, 255],
            );
        }
        let capacity = ((rect.width - 24.) / 10.).max(1.) as usize;
        let display =
            input.edits[index].display(index == 4, focused, time.rem_euclid(1.2) < 0.7, capacity);
        let compact = rect.height < 32.;
        self.text(
            Rect::new(
                rect.x + 8.,
                rect.y + if compact { 3. } else { 8. },
                rect.width - 16.,
                rect.height - if compact { 6. } else { 12. },
            ),
            &display,
            3,
            WHITE,
            false,
        );
        self.hit(&format!("field:{index}"), "AccountEdit", rect, true);
    }
    fn credentials(&mut self, inner: Rect, input: &AccountInput, time: f32) {
        let gap = 22.;
        let col = ((inner.width - 28. - gap) * 0.5).max(0.);
        let left = inner.x + 14.;
        let right = left + col + gap;
        let y = inner.y + 112.;
        let fields = [
            (0, "Server hostname", Rect::new(left, y, col, 36.)),
            (
                1,
                "Login port",
                Rect::new(left, y + 77., (col - 12.) * 0.5, 36.),
            ),
            (
                2,
                "World port",
                Rect::new(left + (col + 12.) * 0.5, y + 77., (col - 12.) * 0.5, 36.),
            ),
            (3, "Account username", Rect::new(right, y, col, 36.)),
            (4, "Password", Rect::new(right, y + 77., col, 36.)),
        ];
        for (index, label, rect) in fields {
            self.edit_field(index, label, rect, input, time);
        }
        let button_y = inner.bottom() - 54.;
        self.button(
            "LOGIN_ConnectButton",
            "primary",
            Rect::new(right, button_y, col, 32.),
            "Sign in",
            true,
            input.focus == Focus::Primary,
            input,
        );
        self.button(
            "LOGIN_CancelButton",
            if input.navigation.is_some() {
                "back"
            } else {
                "exit"
            },
            Rect::new(left, button_y, col.min(120.), 32.),
            if input.navigation.is_some() {
                "Back"
            } else {
                "Exit"
            },
            true,
            input.focus
                == if input.navigation.is_some() {
                    Focus::Back
                } else {
                    Focus::Exit
                },
            input,
        );
    }
    fn list(&mut self, inner: Rect, input: &AccountInput) {
        let characters = self.view.stage == Stage::Characters;
        let title = if characters {
            bounded_text(&self.view.world_name, 160)
        } else {
            "Available worlds".into()
        };
        self.text(
            Rect::new(inner.x + 14., inner.y + 46., inner.width - 28., 24.),
            &title,
            3,
            MUTED,
            false,
        );
        let list = Rect::new(
            inner.x + 14.,
            inner.y + 78.,
            inner.width - 28.,
            (inner.height - 190.).max(0.),
        );
        self.fill(list, [6, 11, 18, 240]);
        self.hit("list", "AccountList", list, true);
        let count = row_count(self.view);
        let first = input.first(self.view);
        let row_height = (list.height / VISIBLE_ROWS as f32).min(32.);
        if count == 0 {
            self.text(
                Rect::new(list.x + 10., list.y + 18., list.width - 20., 60.),
                if characters {
                    "No characters are available on this account."
                } else {
                    "No servers are available. Try Refresh."
                },
                3,
                MUTED,
                true,
            );
        }
        for (row, index) in (first..count.min(first + VISIBLE_ROWS)).enumerate() {
            let rect = Rect::new(
                list.x,
                list.y + row as f32 * row_height,
                list.width,
                (row_height - 1.).max(0.),
            );
            let selected = selected_index(self.view) == Some(index);
            let enabled = row_intent(self.view, index).is_some();
            if selected {
                self.fill(rect, [64, 59, 39, 245]);
            } else if input.pointer.is_some_and(|point| rect.contains(point)) {
                self.fill(rect, [30, 41, 56, 245]);
            }
            let (name, detail) = if characters {
                let c = &self.view.characters[index];
                (
                    bounded_text(&c.name, 128),
                    format!(
                        "Level {} · {}{}",
                        c.level,
                        class_name(c.class),
                        if c.enabled { "" } else { " · Unavailable" }
                    ),
                )
            } else {
                let s = &self.view.servers[index];
                (
                    bounded_text(&s.name, 160),
                    if s.is_up() {
                        format!("{} players", s.players)
                    } else {
                        "Unavailable".into()
                    },
                )
            };
            let detail_width = if characters { 210_f32 } else { 120. }.min(rect.width * 0.5);
            self.text(
                Rect::new(
                    rect.x + 8.,
                    rect.y + 5.,
                    (rect.width - detail_width - 16.).max(0.),
                    rect.height - 5.,
                ),
                &name,
                3,
                if enabled { WHITE } else { MUTED },
                false,
            );
            self.text(
                Rect::new(
                    rect.right() - detail_width,
                    rect.y + 6.,
                    detail_width - 6.,
                    rect.height - 6.,
                ),
                &detail,
                2,
                MUTED,
                false,
            );
            self.hit(&format!("row:{index}"), "AccountRow", rect, enabled);
        }
        if input.focus == Focus::List {
            self.outline(list, [139, 118, 79, 255]);
        }
        self.text(
            Rect::new(list.x, list.bottom() + 3., list.width, 18.),
            &format!(
                "{}–{} of {}  ·  Scroll or use arrow keys",
                if count == 0 { 0 } else { first + 1 },
                count.min(first + VISIBLE_ROWS),
                count
            ),
            1,
            MUTED,
            false,
        );
        let y = inner.bottom() - 54.;
        let width = list.width;
        self.button(
            if characters {
                "CLW_Quit_Button"
            } else {
                "SERVERSELECT_ExitButton"
            },
            "back",
            Rect::new(list.x, y, width * 0.22, 32.),
            if characters && !self.view.resumed {
                "Back"
            } else {
                "Sign out"
            },
            true,
            input.focus == Focus::Back,
            input,
        );
        if !characters {
            self.button(
                "SERVERSELECT_NewsButton",
                "refresh",
                Rect::new(list.x + width * 0.25, y, width * 0.22, 32.),
                "Refresh",
                true,
                input.focus == Focus::Refresh,
                input,
            );
        } else {
            self.button(
                "CLW_Quit_Button",
                "preview",
                Rect::new(list.x + width * 0.25, y, width * 0.30, 32.),
                "Preview",
                playable(self.view),
                input.focus == Focus::Preview,
                input,
            );
        }
        self.button(
            if characters {
                "CLW_Play_Button"
            } else {
                "SERVERSELECT_PlayButton"
            },
            "primary",
            Rect::new(list.x + width * 0.59, y, width * 0.41, 32.),
            if characters { "Enter world" } else { "Play" },
            playable(self.view),
            input.focus == Focus::Primary,
            input,
        );
    }
}

fn class_name(class: u8) -> &'static str {
    [
        "Unknown",
        "Warrior",
        "Cleric",
        "Paladin",
        "Ranger",
        "Shadowknight",
        "Druid",
        "Monk",
        "Bard",
        "Rogue",
        "Shaman",
        "Necromancer",
        "Wizard",
        "Magician",
        "Enchanter",
        "Beastlord",
        "Berserker",
    ]
    .get(class as usize)
    .copied()
    .unwrap_or("Unknown")
}

/// Modal countdown uses the installed EQ button artwork while keeping its hit
/// identity tied to one camp request. No character data or credentials appear.
pub fn camp_frame(
    ui: Option<&AccountUi>,
    viewport: [u32; 2],
    camp: crate::live::camp::View,
) -> UiFrame {
    let fallback = AccountUi {
        login: None,
        characters: None,
    };
    let ui = ui.unwrap_or(&fallback);
    let view = View::default();
    let screen = Rect::new(0., 0., viewport[0] as f32, viewport[1] as f32);
    let mut paint = Paint {
        frame: UiFrame {
            bounds: screen,
            ..Default::default()
        },
        screen,
        ui,
        view: &view,
        prefix: format!("camp:{}:", camp.token),
    };
    paint.fill(screen, [0, 0, 0, 115]);
    paint.hit("frame", "CampCapture", screen, true);
    let width = 440_f32.min(screen.width);
    let height = 190_f32.min(screen.height);
    let panel = Rect::new(
        (screen.width - width) * 0.5,
        (screen.height - height) * 0.5,
        width,
        height,
    );
    paint.fill(panel, [12, 18, 27, 245]);
    paint.outline(panel, GOLD);
    paint.text(
        Rect::new(panel.x + 22., panel.y + 20., width - 44., 32.),
        "Return to character selection",
        5,
        GOLD,
        false,
    );
    let status = match camp.seconds {
        Some(seconds) => format!("Camping in {seconds} seconds"),
        None if camp.cancellable => "Preparing to camp…".into(),
        None => "Leaving the world…".into(),
    };
    paint.text(
        Rect::new(panel.x + 22., panel.y + 67., width - 44., 30.),
        &status,
        3,
        WHITE,
        false,
    );
    let button = Rect::new(panel.x + 22., panel.bottom() - 60., 140., 32.);
    if !paint.skin(
        "LOGIN_CancelButton",
        button,
        "Cancel",
        camp.cancellable,
        false,
        false,
        false,
    ) {
        paint.fill(button, [38, 45, 60, 255]);
        paint.text(
            Rect::new(button.x + 15., button.y + 7., 110., 24.),
            "Cancel",
            3,
            if camp.cancellable { WHITE } else { MUTED },
            false,
        );
    }
    paint.hit("cancel", "Button", button, camp.cancellable);
    paint.frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{
        input::{
            keyboard::{Key, KeyboardInput, NativeKey},
            mouse::{MouseButtonInput, MouseWheel},
        },
        window::{CursorMoved, WindowFocused},
    };
    use openeq_net::{login::ServerEntry, world::Character};

    pub(super) fn window() -> Entity {
        Entity::from_raw_u32(1).unwrap()
    }
    fn key(code: KeyCode, state: ButtonState, text: Option<&str>, repeat: bool) -> WindowEvent {
        WindowEvent::KeyboardInput(KeyboardInput {
            window: window(),
            key_code: code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: text.map(Into::into),
            repeat,
        })
    }
    pub(super) fn press(code: KeyCode, text: Option<&str>) -> WindowEvent {
        key(code, ButtonState::Pressed, text, false)
    }
    pub(super) fn release(code: KeyCode) -> WindowEvent {
        key(code, ButtonState::Released, None, false)
    }
    fn focus(focused: bool) -> WindowEvent {
        WindowEvent::WindowFocused(WindowFocused {
            window: window(),
            focused,
        })
    }
    pub(super) fn frame(view: &View, input: &AccountInput) -> UiFrame {
        AccountUi::default().frame([640, 480], view, input, 0.)
    }
    pub(super) fn send(
        input: &mut AccountInput,
        view: &View,
        event: WindowEvent,
    ) -> Option<Intent> {
        input.event(view, &frame(view, input), window(), &event)
    }
    pub(super) fn click(
        input: &mut AccountInput,
        view: &View,
        frame: &UiFrame,
        suffix: &str,
    ) -> Option<Intent> {
        let hit = frame
            .hit_targets
            .iter()
            .find(|h| action_suffix(view, input, &h.item) == Some(suffix))
            .unwrap();
        let position = bevy::math::Vec2::new(
            hit.rect.x + hit.rect.width * 0.5,
            hit.rect.y + hit.rect.height * 0.5,
        );
        input.event(
            view,
            frame,
            window(),
            &WindowEvent::CursorMoved(CursorMoved {
                window: window(),
                position,
                delta: None,
            }),
        );
        input.event(
            view,
            frame,
            window(),
            &WindowEvent::MouseButtonInput(MouseButtonInput {
                window: window(),
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            }),
        );
        input.event(
            view,
            frame,
            window(),
            &WindowEvent::MouseButtonInput(MouseButtonInput {
                window: window(),
                button: MouseButton::Left,
                state: ButtonState::Released,
            }),
        )
    }
    fn worlds() -> View {
        View {
            stage: Stage::Worlds,
            token: Token {
                attempt: 4,
                revision: 8,
            },
            servers: (0..24)
                .map(|id| ServerEntry {
                    address: "127.0.0.1".parse().unwrap(),
                    server_type: 0,
                    server_id: id,
                    name: format!("World {id}"),
                    country: "US".into(),
                    language: "EN".into(),
                    status: if id == 0 { 1 } else { 0 },
                    players: id * 3,
                })
                .collect(),
            ..Default::default()
        }
    }
    pub(super) fn characters() -> View {
        View {
            stage: Stage::Characters,
            token: Token {
                attempt: 4,
                revision: 10,
            },
            world_name: "OpenEQ Test World".into(),
            characters: (0..12)
                .map(|id| Character {
                    name: format!("Adventurer{id}"),
                    level: 12 + id,
                    class: 6,
                    race: 4,
                    gender: 0,
                    zone: 54,
                    instance_id: 0,
                    enabled: id != 0,
                    appearance: Default::default(),
                })
                .collect(),
            selected_character: Some("Adventurer1".into()),
            ..Default::default()
        }
    }
    #[test]
    fn private_unicode_editor_masking_and_credentials_move() {
        let view = View::default();
        let mut input = AccountInput::new(Endpoint::default());
        send(&mut input, &view, press(KeyCode::KeyA, Some("é猫a")));
        send(&mut input, &view, press(KeyCode::ArrowLeft, None));
        send(&mut input, &view, press(KeyCode::Backspace, None));
        assert_eq!(input.edits[3].text, "éa");
        send(&mut input, &view, press(KeyCode::Delete, None));
        assert_eq!(input.edits[3].text, "é");
        send(&mut input, &view, press(KeyCode::Tab, None));
        send(
            &mut input,
            &view,
            press(KeyCode::KeyP, Some("PRIVATE_SENTINEL")),
        );
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Preedit {
                window: window(),
                value: "SECRET_IME".into(),
                cursor: None,
            }),
        );
        let drawn = format!("{:?}", frame(&view, &input));
        assert!(!drawn.contains("PRIVATE_SENTINEL"));
        assert!(!drawn.contains("SECRET_IME"));
        assert!(drawn.contains('•'));
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Commit {
                window: window(),
                value: "猫".into(),
            }),
        );
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Enter, None)),
            Some(Intent::SignIn)
        );
        let (endpoint, username, password) = input.take_credentials();
        assert_eq!(endpoint, Endpoint::default());
        assert_eq!(username, "é");
        assert!(password.ends_with('猫'));
        assert!(input.edits[4].text.is_empty());
        assert!(input.edits[4].preedit.is_empty());
    }
    #[test]
    fn modifiers_tab_focus_loss_and_input_bounds_follow_raw_order() {
        let view = View::default();
        let mut input = AccountInput::new(Endpoint::default());
        send(&mut input, &view, press(KeyCode::ShiftLeft, None));
        send(&mut input, &view, press(KeyCode::Tab, None));
        assert!(input.focus == Focus::WorldPort);
        send(&mut input, &view, release(KeyCode::ShiftLeft));
        send(&mut input, &view, press(KeyCode::Tab, None));
        send(&mut input, &view, press(KeyCode::SuperLeft, None));
        send(&mut input, &view, press(KeyCode::KeyC, Some("c")));
        assert!(input.edits[3].text.is_empty());
        send(&mut input, &view, focus(false));
        send(&mut input, &view, press(KeyCode::KeyA, Some("ignored")));
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Commit {
                window: window(),
                value: "ignored".into(),
            }),
        );
        assert!(input.edits[3].text.is_empty());
        send(&mut input, &view, focus(true));
        send(
            &mut input,
            &view,
            press(KeyCode::KeyA, Some(&"猫".repeat(1000))),
        );
        assert!(input.edits[3].text.len() <= 128);
        assert!(input.edits[3].text.is_char_boundary(input.edits[3].cursor));
        send(&mut input, &view, press(KeyCode::SuperLeft, None));
        send(&mut input, &view, press(KeyCode::KeyA, None));
        send(&mut input, &view, release(KeyCode::SuperLeft));
        send(&mut input, &view, press(KeyCode::KeyZ, Some("Z")));
        assert_eq!(input.edits[3].text, "Z");
    }
    #[test]
    fn cancelled_password_composition_cannot_commit_into_username() {
        let view = View::default();
        let mut input = AccountInput::new(Endpoint::default());
        let f = frame(&view, &input);
        click(&mut input, &view, &f, "field:4");
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Preedit {
                window: window(),
                value: "PRIVATE_COMPOSITION".into(),
                cursor: None,
            }),
        );
        send(&mut input, &view, press(KeyCode::ShiftLeft, None));
        send(&mut input, &view, press(KeyCode::Tab, None));
        assert!(input.focus == Focus::Password);
        // Some platforms end preedit with an empty value before the commit.
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Preedit {
                window: window(),
                value: "".into(),
                cursor: None,
            }),
        );
        let f = frame(&view, &input);
        click(&mut input, &view, &f, "field:3");
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Commit {
                window: window(),
                value: "PRIVATE_COMPOSITION".into(),
            }),
        );
        assert!(input.edits[3].text.is_empty());
        assert!(!format!("{:?}", frame(&view, &input)).contains("PRIVATE_COMPOSITION"));
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Preedit {
                window: window(),
                value: "猫".into(),
                cursor: None,
            }),
        );
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Commit {
                window: window(),
                value: "猫".into(),
            }),
        );
        assert_eq!(input.edits[3].text, "猫");
    }
    #[test]
    fn credential_ports_validate_before_sign_in_and_never_accept_text() {
        let view = View::default();
        let mut input = AccountInput::new(Endpoint::default());
        let f = frame(&view, &input);
        click(&mut input, &view, &f, "field:1");
        send(&mut input, &view, press(KeyCode::SuperLeft, None));
        send(&mut input, &view, press(KeyCode::KeyA, None));
        send(&mut input, &view, release(KeyCode::SuperLeft));
        send(&mut input, &view, press(KeyCode::Digit9, Some("999999xyz")));
        assert_eq!(input.edits[1].text, "99999");
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.notice.is_some());
        let f = frame(&view, &input);
        click(&mut input, &view, &f, "field:4");
        send(
            &mut input,
            &view,
            WindowEvent::Ime(Ime::Commit {
                window: window(),
                value: "password".into(),
            }),
        );
        input.reset();
        assert!(input.edits[4].text.is_empty());
        assert!(input.captured.is_empty());
    }
    #[test]
    fn stable_row_ids_disabled_controls_scroll_and_stale_frames() {
        let mut view = worlds();
        let mut input = AccountInput::new(Endpoint::default());
        let f = frame(&view, &input);
        assert_eq!(
            f.hit_targets
                .iter()
                .filter(|h| h.kind == "AccountRow")
                .count(),
            VISIBLE_ROWS
        );
        assert!(click(&mut input, &view, &f, "row:0").is_none());
        assert!(click(&mut input, &view, &f, "primary").is_none());
        assert_eq!(
            click(&mut input, &view, &f, "row:2"),
            Some(Intent::SelectWorld {
                token: view.token,
                id: 2
            })
        );
        view.selected_world = Some(2);
        assert_eq!(
            send(&mut input, &view, press(KeyCode::ArrowDown, None)),
            Some(Intent::SelectWorld {
                token: view.token,
                id: 3
            })
        );
        assert_eq!(
            send(&mut input, &view, press(KeyCode::Enter, None)),
            Some(Intent::Play { token: view.token })
        );
        send(
            &mut input,
            &view,
            WindowEvent::MouseWheel(MouseWheel {
                window: window(),
                unit: MouseScrollUnit::Line,
                x: 0.,
                y: -2.,
                phase: bevy::input::touch::TouchPhase::Moved,
            }),
        );
        assert_eq!(input.first, 3);
        send(&mut input, &view, press(KeyCode::PageDown, None));
        assert_eq!(input.first, 11);
        view.token.revision += 1;
        assert!(
            input
                .event(&view, &f, window(), &press(KeyCode::Enter, None))
                .is_none()
        );
        let old = f
            .hit_targets
            .iter()
            .find(|h| h.screen_id == "row:2")
            .unwrap();
        assert!(action_suffix(&view, &input, &old.item).is_none());
        assert_eq!(input.first, 0);
    }
    #[test]
    fn handoff_releases_only_owned_keys_and_other_windows_are_ignored() {
        let view = View::default();
        let mut input = AccountInput::new(Endpoint::default());
        send(&mut input, &view, press(KeyCode::KeyW, Some("w")));
        let mut keys = ButtonInput::default();
        keys.press(KeyCode::KeyW);
        keys.press(KeyCode::KeyD);
        input.begin_handoff_frame(&mut keys);
        assert!(!input.filter_handoff_event(&press(KeyCode::KeyD, Some("d")), window(), &mut keys));
        assert!(!keys.pressed(KeyCode::KeyW));
        assert!(keys.pressed(KeyCode::KeyD));
        assert!(input.filter_handoff_event(&release(KeyCode::KeyW), window(), &mut keys));
        assert!(!input.filter_handoff_event(&press(KeyCode::KeyW, Some("w")), window(), &mut keys));
        assert!(keys.pressed(KeyCode::KeyW));
        let mut other = press(KeyCode::KeyX, Some("x"));
        if let WindowEvent::KeyboardInput(event) = &mut other {
            event.window = Entity::from_raw_u32(2).unwrap();
        }
        send(&mut input, &view, other);
        assert_eq!(input.edits[3].text, "w");
    }
    #[test]
    fn fallback_screens_have_bounded_clipped_rows_and_no_unavailable_actions() {
        for view in [
            View::default(),
            worlds(),
            characters(),
            View {
                stage: Stage::Authenticating,
                ..Default::default()
            },
        ] {
            let input = AccountInput::new(Endpoint::default());
            for size in [[640, 480], [320, 240], [1, 1], [0, 0]] {
                let f = AccountUi::default().frame(size, &view, &input, f32::NAN);
                assert!(f.hit_targets.len() < 32);
                assert!(f.commands.len() < 200);
                assert!(
                    f.hit_targets
                        .iter()
                        .all(|h| h.rect.intersect(f.bounds) == h.rect)
                );
                assert!(f.hit_targets.iter().all(|h| !h.item.contains("Create")
                    && !h.item.contains("Delete")
                    && !h.item.contains("Tutorial")
                    && !h.item.contains("Marketplace")));
            }
        }
    }
    fn prepare_physical_frame(keys: &mut ButtonInput<KeyCode>, events: &[WindowEvent]) {
        keys.clear();
        for event in events {
            if let WindowEvent::KeyboardInput(event) = event {
                match event.state {
                    ButtonState::Pressed => keys.press(event.key_code),
                    ButtonState::Released => keys.release(event.key_code),
                }
            }
        }
        crate::input::recover_focus_loss(window(), events, keys, &mut ButtonInput::default());
    }
    fn own(code: KeyCode) -> AccountInput {
        let mut input = AccountInput::new(Endpoint::default());
        send(&mut input, &View::default(), press(code, Some("x")));
        input
    }
    #[test]
    fn handoff_owned_repeat_then_release_has_no_gameplay_edges() {
        let mut input = own(KeyCode::KeyQ);
        let mut keys = ButtonInput::default();
        let events = [
            key(KeyCode::KeyQ, ButtonState::Pressed, Some("q"), true),
            release(KeyCode::KeyQ),
        ];
        prepare_physical_frame(&mut keys, &events);
        assert!(keys.just_pressed(KeyCode::KeyQ)); // The original failure.
        input.begin_handoff_frame(&mut keys);
        for event in &events {
            assert!(input.filter_handoff_event(event, window(), &mut keys));
        }
        assert!(!keys.pressed(KeyCode::KeyQ));
        assert!(!keys.just_pressed(KeyCode::KeyQ));
        assert!(!keys.just_released(KeyCode::KeyQ));
        // No stale per-frame state suppresses an ordinary press next frame.
        let fresh = [press(KeyCode::KeyQ, Some("q"))];
        prepare_physical_frame(&mut keys, &fresh);
        input.begin_handoff_frame(&mut keys);
        assert!(!input.filter_handoff_event(&fresh[0], window(), &mut keys));
        assert!(keys.pressed(KeyCode::KeyQ));
        assert!(keys.just_pressed(KeyCode::KeyQ));
    }
    #[test]
    fn handoff_filters_owned_raw_text_before_new_chat_and_allows_fresh_repress() {
        let mut input = own(KeyCode::KeyW);
        let mut keys = ButtonInput::default();
        let mut chat = crate::input::ChatInput::default();
        let mut editor = crate::chat::ChatEditor::default();
        let events = [
            press(KeyCode::Enter, None),
            key(KeyCode::KeyW, ButtonState::Pressed, Some("w"), true),
            release(KeyCode::KeyW),
        ];
        prepare_physical_frame(&mut keys, &events);
        input.begin_handoff_frame(&mut keys);
        for event in &events {
            if !input.filter_handoff_event(event, window(), &mut keys) {
                chat.event(&mut editor, window(), event);
            }
        }
        chat.suppress_captured_keys(&mut keys);
        assert!(editor.active);
        assert!(editor.text.is_empty());
        assert!(!keys.just_pressed(KeyCode::KeyW));

        let mut input = own(KeyCode::KeyW);
        let events = [
            key(KeyCode::KeyW, ButtonState::Pressed, Some("w"), true),
            release(KeyCode::KeyW),
            press(KeyCode::KeyW, Some("w")),
        ];
        prepare_physical_frame(&mut keys, &events);
        input.begin_handoff_frame(&mut keys);
        let mut filtered = Vec::new();
        for event in &events {
            let owned = input.filter_handoff_event(event, window(), &mut keys);
            filtered.push(owned);
            if !owned {
                chat.event(&mut editor, window(), event);
            }
        }
        assert_eq!(filtered, [true, true, false]);
        assert_eq!(editor.text, "w");
        assert!(keys.pressed(KeyCode::KeyW));
        assert!(keys.just_pressed(KeyCode::KeyW));
        chat.suppress_captured_keys(&mut keys);
        assert!(!keys.pressed(KeyCode::KeyW));
    }
    #[test]
    fn handoff_focus_loss_and_same_frame_fresh_presses_preserve_raw_order() {
        let mut input = own(KeyCode::KeyQ);
        let mut keys = ButtonInput::default();
        let events = [
            key(KeyCode::KeyQ, ButtonState::Pressed, Some("q"), true),
            focus(false),
            focus(true),
            press(KeyCode::KeyQ, Some("q")),
            press(KeyCode::KeyD, Some("d")),
            release(KeyCode::KeyQ),
        ];
        prepare_physical_frame(&mut keys, &events);
        input.begin_handoff_frame(&mut keys);
        for (index, event) in events.iter().enumerate() {
            assert_eq!(
                input.filter_handoff_event(event, window(), &mut keys),
                index == 0
            );
        }
        assert!(keys.just_pressed(KeyCode::KeyQ));
        assert!(keys.just_released(KeyCode::KeyQ));
        assert!(!keys.pressed(KeyCode::KeyQ));
        assert!(keys.pressed(KeyCode::KeyD));
        // A held account key is still suppressed in frames without raw events.
        let mut input = own(KeyCode::KeyW);
        keys.press(KeyCode::KeyW);
        input.begin_handoff_frame(&mut keys);
        assert!(!keys.pressed(KeyCode::KeyW));
        input.begin_handoff_frame(&mut keys);
        assert!(!keys.just_pressed(KeyCode::KeyW));
    }
    #[test]
    fn camp_raw_ownership_survives_cleared_buttons_and_empty_handoff_frame() {
        let view = characters();
        let mut input = AccountInput::new(Endpoint::default());
        let ui = AccountUi::default();
        let mut camp = CampInput::default();
        let mut keys = ButtonInput::default();
        for key in [KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Escape] {
            keys.press(key);
            camp.observe(window(), &press(key, None));
        }
        keys.reset_all(); // Happens on every countdown frame.
        camp.capture_pressed(&keys); // An empty later frame must not lose ownership.
        input.reset(); // Character retirement.
        input.suppress_held_keys(&keys); // Ordinary handoff alone cannot recover it.
        camp.handoff(&mut input);
        let frame = ui.frame([800, 600], &view, &input, 0.);
        for key in [KeyCode::Enter, KeyCode::ArrowDown, KeyCode::Escape] {
            let mut repeat = press(key, None);
            if let WindowEvent::KeyboardInput(event) = &mut repeat {
                event.repeat = true;
            }
            assert_eq!(input.event(&view, &frame, window(), &repeat), None);
            assert_eq!(
                input.event(&view, &frame, window(), &press(key, None)),
                None
            );
            assert_eq!(input.event(&view, &frame, window(), &release(key)), None);
        }
        assert_eq!(
            input.event(&view, &frame, window(), &press(KeyCode::Enter, None)),
            Some(Intent::Play { token: view.token })
        );
    }
    #[test]
    fn returning_held_keys_cannot_activate_roster_until_released() {
        let view = characters();
        let mut input = AccountInput::new(Endpoint::default());
        let frame = AccountUi::default().frame([800, 600], &view, &input, 0.);
        let mut held = ButtonInput::default();
        held.press(KeyCode::Enter);
        held.press(KeyCode::Escape);
        input.suppress_held_keys(&held);
        for key in [KeyCode::Enter, KeyCode::Escape] {
            assert_eq!(
                input.event(&view, &frame, window(), &press(key, None)),
                None
            );
            assert_eq!(input.event(&view, &frame, window(), &release(key)), None);
        }
        assert_eq!(
            input.event(&view, &frame, window(), &press(KeyCode::Enter, None)),
            Some(Intent::Play { token: view.token })
        );
    }
    #[test]
    fn camp_overlay_hits_are_attempt_scoped_and_disabled_during_logout() {
        for (token, cancellable) in [(7, true), (8, false)] {
            let frame = camp_frame(
                None,
                [800, 600],
                crate::live::camp::View {
                    token,
                    seconds: cancellable.then_some(29),
                    cancellable,
                },
            );
            let cancel = frame
                .hit_targets
                .iter()
                .find(|h| h.screen_id == "cancel")
                .unwrap();
            assert_eq!(cancel.item, format!("camp:{token}:cancel"));
            assert_eq!(cancel.enabled, cancellable);
            assert!(
                frame
                    .hit_targets
                    .iter()
                    .all(|h| h.item.starts_with(&format!("camp:{token}:")))
            );
        }
    }
    #[test]
    #[ignore = "requires original UI assets and GPU; no network or audio"]
    fn original_camp_frames_at_one_and_two_scales() {
        let directory = openeq_assets::loader::default_client_dir().unwrap();
        let ui = AccountUi::load(&directory);
        assert!(ui.login.is_some());
        for (name, camp) in [
            (
                "countdown",
                crate::live::camp::View {
                    token: 1,
                    seconds: Some(27),
                    cancellable: true,
                },
            ),
            (
                "leaving",
                crate::live::camp::View {
                    token: 1,
                    seconds: None,
                    cancellable: false,
                },
            ),
        ] {
            for scale in [1., 2.] {
                let frame = camp_frame(Some(&ui), [800, 600], camp);
                assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Image { texture, .. } if texture.exists())));
                let mut renderer = openeq_render::Renderer::new_headless(
                    (800. * scale) as u32,
                    (600. * scale) as u32,
                )
                .unwrap();
                renderer.set_ui_scaled(&frame, scale);
                renderer.render_ui();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                assert!(
                    pixels
                        .chunks_exact(4)
                        .any(|pixel| pixel[0] > 80 && pixel[1] > 80)
                );
                if let Some(output) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(output)
                        .join(format!("camp-{name}-{}x.png", scale as u32));
                    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
                        .unwrap();
                }
            }
        }
    }
    #[test]
    #[ignore = "requires original UI assets and GPU; no network or audio"]
    fn original_account_frames_at_one_and_two_scales() {
        let directory = std::env::var_os("EQ_CLIENT_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(std::env::var_os("HOME").unwrap()).join("EverQuest")
            });
        let ui = AccountUi::load(&directory);
        assert!(ui.login.is_some());
        assert!(ui.characters.is_some());
        let mut input = AccountInput::new(Endpoint {
            host: "storage2.daeken.dev".into(),
            ..Default::default()
        });
        let views = [
            ("credentials", View::default()),
            ("worlds", worlds()),
            ("characters", characters()),
            (
                "busy",
                View {
                    stage: Stage::Authenticating,
                    ..Default::default()
                },
            ),
        ];
        for (name, view) in views {
            input.sync(&view);
            if view.stage == Stage::Credentials {
                input.edits[3].insert("Adventurer", 128, false);
                input.edits[4].insert("CAPTURE_SECRET", 512, false);
            }
            let frame = ui.frame([800, 600], &view, &input, 0.4);
            let debug = format!("{frame:?}");
            assert!(!debug.contains("CAPTURE_SECRET"));
            assert!(
                frame
                    .commands
                    .iter()
                    .any(|c| matches!(c, DrawCommand::Image { texture, .. } if texture.exists()))
            );
            for scale in [1., 2.] {
                let mut renderer = openeq_render::Renderer::new_headless(
                    (800. * scale) as u32,
                    (600. * scale) as u32,
                )
                .unwrap();
                renderer.set_ui_scaled(&frame, scale);
                renderer.render_ui();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                assert!(pixels.chunks_exact(4).any(|p| p[0] > 80 && p[1] > 80));
                if let Some(output) = std::env::var_os("OPENEQ_UI_CAPTURE_DIR") {
                    let path = std::path::PathBuf::from(output)
                        .join(format!("account-{name}-{}x.png", scale as u32));
                    image::save_buffer(path, &pixels, width, height, image::ColorType::Rgba8)
                        .unwrap();
                }
            }
        }
    }
}
