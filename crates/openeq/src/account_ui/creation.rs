//! Local creation wizard, isolated name editor, and final explicit commit.
use super::*;
use crate::account_creation::{
    self, Feature, Phase, Rejection,
    editor::{ChoiceField, Editor, Stat},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Identity,
    Origin,
    Stats,
    Appearance,
    Review,
}
impl Step {
    const ALL: [Self; 5] = [
        Self::Identity,
        Self::Origin,
        Self::Stats,
        Self::Appearance,
        Self::Review,
    ];
    fn index(self) -> usize {
        Self::ALL.iter().position(|step| *step == self).unwrap()
    }
    fn title(self) -> &'static str {
        match self {
            Self::Identity => "Name and identity",
            Self::Origin => "Starting choices",
            Self::Stats => "Starting attributes",
            Self::Appearance => "Appearance",
            Self::Review => "Review your character",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Choice(ChoiceField),
    Stat(Stat),
    Feature(Feature),
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Row(usize),
    Previous,
    Next,
    Close,
    Reset,
    RotateLeft,
    RotateRight,
}

pub(super) struct CreationScreen {
    editor: Editor,
    pub(super) name: Edit,
    pub(super) composing: bool,
    step: Step,
    focus: Control,
    first: usize,
    pub(super) heading: f32,
    prior_operation: Option<account_creation::Token>,
    notice: Option<String>,
}
impl CreationScreen {
    fn fields(&self) -> Vec<Field> {
        match self.step {
            Step::Identity => vec![
                Field::Name,
                Field::Choice(ChoiceField::Race),
                Field::Choice(ChoiceField::Class),
                Field::Choice(ChoiceField::Gender),
            ],
            Step::Origin => vec![
                Field::Choice(ChoiceField::Deity),
                Field::Choice(ChoiceField::StartZone),
            ],
            Step::Stats => Stat::ALL.into_iter().map(Field::Stat).collect(),
            Step::Appearance => [
                Feature::Heritage,
                Feature::Face,
                Feature::Hair,
                Feature::Beard,
                Feature::Eye1,
                Feature::Eye2,
                Feature::HairColor,
                Feature::BeardColor,
                Feature::Tattoo,
                Feature::Details,
            ]
            .into_iter()
            .filter(|feature| self.editor.feature_values(*feature).len() > 1)
            .map(Field::Feature)
            .collect(),
            Step::Review => vec![],
        }
    }
    fn outcome<'a>(&self, view: &'a View) -> Option<&'a Phase> {
        let creation = view.creation.as_ref()?;
        (creation.operation.is_some() && creation.operation != self.prior_operation)
            .then_some(creation.phase.as_ref())
            .flatten()
    }
    fn pending(&self, view: &View) -> bool {
        view.creation
            .as_ref()
            .is_some_and(|creation| creation.pending())
    }
    fn status(&self, view: &View) -> bool {
        self.pending(view) || self.outcome(view).is_some()
    }
    fn order(&self, view: &View) -> Vec<Control> {
        if self.status(view) {
            return if matches!(self.outcome(view), Some(Phase::Rejected(_))) {
                vec![Control::Previous, Control::Close]
            } else {
                vec![Control::Close]
            };
        }
        let mut out: Vec<_> = (0..self.fields().len()).map(Control::Row).collect();
        if self.step == Step::Stats {
            out.push(Control::Reset);
        }
        if self.step == Step::Review {
            out.extend([Control::RotateLeft, Control::RotateRight]);
        }
        if self.step != Step::Identity {
            out.push(Control::Previous);
        }
        out.extend([Control::Next, Control::Close]);
        out
    }
    fn change(&mut self, field: Field, direction: i32) -> bool {
        self.notice = None;
        match field {
            Field::Name => false,
            Field::Choice(field) => self.editor.cycle_choice(field, direction),
            Field::Stat(stat) => self.editor.adjust_stat(stat, direction),
            Field::Feature(feature) => self.editor.cycle_feature(feature, direction),
        }
    }
}

impl AccountInput {
    pub(super) fn creation_valid(&self, view: &View) -> bool {
        self.creation.as_ref().is_some_and(|screen| {
            view.stage == Stage::Characters
                && screen.editor.context().session == view.token.attempt
                && view.creation.as_ref().is_some_and(|creation| {
                    creation.selection.connection == screen.editor.context().connection
                })
        })
    }
    pub fn creation_request(
        &self,
        view: &View,
    ) -> Option<(account_creation::Context, openeq_net::world::Character)> {
        if !self.creation_valid(view) {
            return None;
        }
        let editor = &self.creation.as_ref()?.editor;
        Some((editor.context(), editor.draft().preview_character().ok()?))
    }
    pub fn creation_showing_preview(&self, view: &View) -> bool {
        self.creation_valid(view)
            && self
                .creation
                .as_ref()
                .is_some_and(|screen| screen.step == Step::Review && !screen.status(view))
    }
    pub fn apply_creation_preview(
        &mut self,
        view: &View,
        context: account_creation::Context,
        result: Result<crate::account_preview::CreationPreview, String>,
    ) {
        self.creation_sync(view);
        let Some(screen) = &mut self.creation else {
            return;
        };
        if screen.status(view) {
            return;
        }
        let changed = match result {
            Ok(preview) => screen.editor.apply_preview(
                context,
                preview.policy,
                preview.receipt,
                preview.heritages,
            ),
            Err(message) => {
                screen.editor.preview_failed(context, message);
                false
            }
        };
        if changed {
            self.creation_transition();
        }
    }
    pub(super) fn creation_name_focused(&self, view: &View) -> bool {
        self.creation_valid(view)
            && self.creation.as_ref().is_some_and(|screen| {
                !screen.status(view)
                    && screen.step == Step::Identity
                    && screen.focus == Control::Row(0)
            })
    }
    pub(super) fn creation_sync(&mut self, view: &View) {
        if self.creation.is_none() {
            return;
        }
        if !self.creation_valid(view) {
            self.cancel_composition();
            self.creation = None;
            self.creation_transition();
            return;
        }
        let screen = self.creation.as_mut().unwrap();
        if screen.status(view) {
            return;
        }
        match screen.editor.refresh(
            view.token.attempt,
            &view.creation.as_ref().unwrap().selection,
        ) {
            Ok(true) => {
                screen.notice = Some("The server choices changed. Review this draft again.".into());
                self.creation_transition();
            }
            Err(error) => {
                screen.notice = Some(error.to_string());
                self.creation_transition();
            }
            _ => {}
        }
    }
    fn creation_transition(&mut self) {
        self.cancel_composition();
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.preview_held.clone_from(&self.captured);
        self.pressed = None;
    }
    pub(super) fn open_creation(&mut self, view: &View) {
        if view.stage != Stage::Characters {
            return;
        }
        let Some(creation) = &view.creation else {
            self.notice = Some("Waiting for the server's creation choices.");
            return;
        };
        if creation.pending() {
            return;
        }
        if matches!(creation.phase, Some(Phase::Uncertain(_))) {
            self.notice = Some(
                "Sign in again to check your character list before creating another character.",
            );
            return;
        }
        match Editor::new(view.token.attempt, &creation.selection) {
            Ok(editor) => {
                self.cancel_composition();
                self.preview = None;
                self.creation = Some(CreationScreen {
                    editor,
                    name: Edit::default(),
                    composing: false,
                    step: Step::Identity,
                    focus: Control::Row(0),
                    first: 0,
                    heading: 256.,
                    prior_operation: creation.operation,
                    notice: None,
                });
                self.creation_transition();
            }
            Err(_) => {
                self.notice = Some(
                    "Character creation is unavailable for the current server choices or account capacity.",
                )
            }
        }
    }
    fn close_creation(&mut self, view: &View) -> Option<Intent> {
        let operation = self.creation.as_ref().and_then(|screen| {
            view.creation
                .as_ref()
                .filter(|creation| {
                    creation.pending() && creation.operation != screen.prior_operation
                })
                .and_then(|creation| creation.operation)
        });
        self.cancel_composition();
        self.creation = None;
        self.creation_transition();
        self.focus = Focus::Create;
        operation.map(|operation| Intent::CancelCreation {
            token: view.token,
            operation,
        })
    }
    fn creation_action(&mut self, view: &View, action: &str) -> Option<Intent> {
        if action == "create_close" {
            return self.close_creation(view);
        }
        let screen = self.creation.as_mut()?;
        if screen.status(view) {
            if action == "create_previous"
                && matches!(screen.outcome(view), Some(Phase::Rejected(_)))
            {
                screen.prior_operation = view
                    .creation
                    .as_ref()
                    .and_then(|creation| creation.operation);
                screen.step = Step::Identity;
                screen.focus = Control::Row(0);
                screen.first = 0;
                self.creation_transition();
            }
            return None;
        }
        if screen.composing {
            return None;
        }
        match action {
            "create_previous" => {
                if screen.step == Step::Identity {
                    return self.close_creation(view);
                }
                screen.step = Step::ALL[screen.step.index() - 1];
                screen.focus = Control::Next;
                screen.first = 0;
                self.creation_transition();
            }
            "create_next" => {
                if screen.step == Step::Review {
                    match screen.editor.submission() {
                        Ok(submission) => {
                            return Some(Intent::Create {
                                token: view.token,
                                submission: Box::new(submission),
                            });
                        }
                        Err(error) => screen.notice = Some(error.to_string()),
                    }
                } else {
                    screen.step = Step::ALL[screen.step.index() + 1];
                    screen.focus = Control::Row(0);
                    screen.first = 0;
                    if screen.fields().is_empty() {
                        screen.focus = Control::Next;
                    }
                    self.creation_transition();
                }
            }
            "create_reset" => {
                screen.editor.reset_stats();
            }
            "create_rotate_left" => screen.heading = (screen.heading - 32.).rem_euclid(512.),
            "create_rotate_right" => screen.heading = (screen.heading + 32.).rem_euclid(512.),
            _ => {
                let (prefix, index) = action.rsplit_once(':')?;
                let index = index.parse::<usize>().ok()?;
                let field = *screen.fields().get(index)?;
                screen.focus = Control::Row(index);
                if prefix == "create_less" || prefix == "create_more" {
                    let changes_layout =
                        matches!(field, Field::Choice(_) | Field::Feature(Feature::Heritage));
                    if screen.change(field, if prefix == "create_more" { 1 } else { -1 })
                        && changes_layout
                    {
                        self.creation_transition();
                    }
                }
            }
        }
        None
    }
    pub(super) fn creation_event(
        &mut self,
        view: &View,
        frame: &UiFrame,
        window: Entity,
        event: &WindowEvent,
    ) -> Option<Intent> {
        match event {
            WindowEvent::WindowFocused(event) if event.window == window => {
                self.focused = event.focused;
                if !event.focused {
                    self.modifiers.reset_all();
                    self.captured.clear();
                    self.returning_held.clear();
                    self.preview_held.clear();
                    self.pressed = None;
                    self.pointer = None;
                    self.cancel_composition();
                }
            }
            WindowEvent::CursorMoved(event) if event.window == window && self.focused => {
                self.pointer = Some([event.position.x, event.position.y])
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
                    return None;
                }
                self.captured.insert(event.key_code);
                if !self.focused
                    || self.returning_held.contains(&event.key_code)
                    || self.preview_held.contains(&event.key_code)
                    || !current_frame(view, self, frame)
                {
                    return None;
                }
                self.modifiers.press(event.key_code);
                if event.key_code == KeyCode::Escape && !event.repeat {
                    if self.creation.as_ref()?.composing {
                        self.cancel_composition();
                        return None;
                    }
                    return self.creation_action(
                        view,
                        if self.creation.as_ref()?.status(view) {
                            "create_close"
                        } else {
                            "create_previous"
                        },
                    );
                }
                let shift = self.modifiers.pressed(KeyCode::ShiftLeft)
                    || self.modifiers.pressed(KeyCode::ShiftRight);
                let command = [
                    KeyCode::ControlLeft,
                    KeyCode::ControlRight,
                    KeyCode::SuperLeft,
                    KeyCode::SuperRight,
                ]
                .iter()
                .any(|key| self.modifiers.pressed(*key));
                let screen = self.creation.as_mut()?;
                if screen.composing {
                    return None;
                }
                if event.key_code == KeyCode::Tab && !event.repeat {
                    let order = screen.order(view);
                    let index = order
                        .iter()
                        .position(|control| *control == screen.focus)
                        .unwrap_or(0);
                    screen.focus =
                        order[(index + if shift { order.len() - 1 } else { 1 }) % order.len()];
                    if let Control::Row(index) = screen.focus {
                        screen.first = index;
                    }
                    return None;
                }
                let field = if let Control::Row(index) = screen.focus {
                    screen.fields().get(index).copied()
                } else {
                    None
                };
                if field == Some(Field::Name) && !screen.status(view) {
                    match event.key_code {
                        KeyCode::Enter | KeyCode::NumpadEnter if !event.repeat => {
                            return self.creation_action(view, "create_next");
                        }
                        KeyCode::Backspace => screen.name.backspace(),
                        KeyCode::Delete => screen.name.delete(),
                        KeyCode::ArrowLeft => screen.name.left(),
                        KeyCode::ArrowRight => screen.name.right(),
                        KeyCode::Home => {
                            screen.name.cursor = 0;
                            screen.name.all_selected = false;
                        }
                        KeyCode::End => {
                            screen.name.cursor = screen.name.text.len();
                            screen.name.all_selected = false;
                        }
                        KeyCode::KeyA if command => screen.name.all_selected = true,
                        KeyCode::KeyU if command => screen.name.clear(),
                        _ if !command => {
                            if let Some(text) = &event.text {
                                insert_name(&mut screen.name, text);
                            }
                        }
                        _ => {}
                    }
                    normalize_name(&mut screen.name);
                    screen.editor.set_name(screen.name.text.clone());
                    screen.notice = None;
                    return None;
                }
                if matches!(event.key_code, KeyCode::ArrowUp | KeyCode::ArrowDown)
                    && !screen.status(view)
                {
                    let count = screen.fields().len();
                    if count > 0 {
                        let index = if let Control::Row(index) = screen.focus {
                            index
                        } else {
                            0
                        };
                        let next = (index
                            + if event.key_code == KeyCode::ArrowDown {
                                1
                            } else {
                                count - 1
                            })
                            % count;
                        screen.focus = Control::Row(next);
                        screen.first = next;
                    }
                    return None;
                }
                if matches!(event.key_code, KeyCode::ArrowLeft | KeyCode::ArrowRight)
                    && !screen.status(view)
                {
                    if let Some(field) = field {
                        let layout =
                            matches!(field, Field::Choice(_) | Field::Feature(Feature::Heritage));
                        if screen.change(
                            field,
                            if event.key_code == KeyCode::ArrowRight {
                                1
                            } else {
                                -1
                            },
                        ) && layout
                        {
                            self.creation_transition();
                        }
                    }
                    return None;
                }
                if matches!(
                    event.key_code,
                    KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space
                ) && !event.repeat
                {
                    let action = match screen.focus {
                        Control::Previous => "create_previous",
                        Control::Next => "create_next",
                        Control::Close => "create_close",
                        Control::Reset => "create_reset",
                        Control::RotateLeft => "create_rotate_left",
                        Control::RotateRight => "create_rotate_right",
                        Control::Row(_) => return None,
                    };
                    return self.creation_action(view, action);
                }
            }
            WindowEvent::Ime(Ime::Preedit {
                window: id, value, ..
            }) if *id == window
                && self.creation_name_focused(view)
                && self.focused
                && current_frame(view, self, frame) =>
            {
                let screen = self.creation.as_mut()?;
                if !value.is_empty() {
                    screen.composing = true;
                    self.cancelled_composition = false;
                }
                screen.name.preedit = bounded_text(value, 15);
            }
            WindowEvent::Ime(Ime::Commit { window: id, value })
                if *id == window && self.focused && current_frame(view, self, frame) =>
            {
                if self.cancelled_composition {
                    self.cancelled_composition = false;
                    return None;
                }
                if self.creation_name_focused(view) {
                    let screen = self.creation.as_mut()?;
                    screen.composing = false;
                    screen.name.preedit.clear();
                    insert_name(&mut screen.name, value);
                    normalize_name(&mut screen.name);
                    screen.editor.set_name(screen.name.text.clone());
                }
            }
            WindowEvent::Ime(Ime::Disabled { window: id }) if *id == window => {
                self.cancel_composition()
            }
            WindowEvent::MouseWheel(event)
                if event.window == window && self.focused && current_frame(view, self, frame) =>
            {
                let screen = self.creation.as_mut()?;
                if event.y.is_finite() && !screen.status(view) {
                    screen.first = screen
                        .first
                        .saturating_add_signed(if event.y < 0. { 1 } else { -1 })
                        .min(screen.fields().len().saturating_sub(1));
                    self.pressed = None;
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
                    self.pressed = hit.map(|hit| hit.item.clone());
                    if let Some(action) = hit.and_then(|hit| action_suffix(view, self, &hit.item))
                        && action.starts_with("create_row:")
                    {
                        let index = action[11..].parse::<usize>().ok()?;
                        self.cancel_composition();
                        self.creation.as_mut()?.focus = Control::Row(index);
                    }
                } else if let Some(pressed) = self.pressed.take()
                    && let Some(hit) = hit.filter(|hit| hit.item == pressed)
                {
                    let action = action_suffix(view, self, &hit.item)?;
                    return self.creation_action(view, action);
                }
            }
            _ => {}
        }
        None
    }
}
fn insert_name(edit: &mut Edit, text: &str) {
    edit.insert(
        &text
            .chars()
            .filter(char::is_ascii_alphabetic)
            .collect::<String>(),
        15,
        false,
    );
}
fn normalize_name(edit: &mut Edit) {
    edit.text.make_ascii_lowercase();
    if let Some(first) = edit.text.get(..1) {
        let first = first.to_ascii_uppercase();
        edit.text.replace_range(..1, &first);
    }
}

/// Keep the retained operation's result visible after its editor closes.
pub(super) fn roster_status(view: &View) -> Option<&'static str> {
    let creation = view.creation.as_ref()?;
    if creation.pending() {
        return Some("Creation continues. You can close this screen.");
    }
    Some(match creation.phase.as_ref()? {
        Phase::Completed(_) => "Character created. Select it to enter the world.",
        Phase::Rejected(Rejection::Name) => "The server rejected this name. Choose another.",
        Phase::Rejected(Rejection::Creation) => "Character rejected. Review your choices.",
        Phase::Uncertain(_) => {
            "Creation was not confirmed. Sign in again to check your character list."
        }
        Phase::Cancelled => "Creation cancelled before sending.",
        _ => return None,
    })
}

fn race_name(race: u32) -> String {
    match race {
        1 => "Human",
        2 => "Barbarian",
        3 => "Erudite",
        4 => "Wood Elf",
        5 => "High Elf",
        6 => "Dark Elf",
        7 => "Half Elf",
        8 => "Dwarf",
        9 => "Troll",
        10 => "Ogre",
        11 => "Halfling",
        12 => "Gnome",
        128 => "Iksar",
        130 => "Vah Shir",
        330 => "Froglok",
        522 => "Drakkin",
        _ => return format!("Race {race}"),
    }
    .into()
}
fn deity_name(deity: u32) -> String {
    match deity {
        140 | 396 => "Agnostic",
        201 => "Bertoxxulous",
        202 => "Brell Serilis",
        203 => "Cazic-Thule",
        204 => "Erollisi Marr",
        205 => "Bristlebane",
        206 => "Innoruuk",
        207 => "Karana",
        208 => "Mithaniel Marr",
        209 => "Prexus",
        210 => "Quellious",
        211 => "Rallos Zek",
        212 => "Rodcet Nife",
        213 => "Solusek Ro",
        214 => "The Tribunal",
        215 => "Tunare",
        216 => "Veeshan",
        _ => return format!("Deity {deity}"),
    }
    .into()
}
fn feature_label(feature: Feature) -> &'static str {
    match feature {
        Feature::Face => "Face",
        Feature::Hair => "Hair style",
        Feature::Beard => "Beard",
        Feature::HairColor => "Hair color",
        Feature::BeardColor => "Beard color",
        Feature::Eye1 => "Right eye",
        Feature::Eye2 => "Left eye",
        Feature::Heritage => "Heritage",
        Feature::Tattoo => "Tattoo",
        Feature::Details => "Facial details",
    }
}
/// Display names only, from EQEmu common/eq_constants.h::Zones (4aceae18b).
/// Eligibility and IDs always come from the live catalog, including custom
/// starts absent from this deliberately bounded standard-city label table.
fn starting_zone_name(zone: u32) -> String {
    match zone {
        1 => "South Qeynos",
        2 => "North Qeynos",
        3 => "The Surefall Glade",
        8 => "North Freeport",
        9 | 383 => "West Freeport",
        10 | 382 => "East Freeport",
        19 => "Rivervale",
        23 => "The Erudin Palace",
        24 => "Erudin",
        29 => "Halas",
        40 => "Neriak - Foreign Quarter",
        41 => "Neriak - Commons",
        42 => "Neriak - 3rd Gate",
        49 => "Oggok",
        52 => "Grobb",
        54 => "The Greater Faydark",
        55 => "Ak'Anon",
        60 => "South Kaladim",
        61 => "Northern Felwithe",
        62 => "Southern Felwithe",
        67 => "North Kaladim",
        75 => "Paineel",
        77 => "The Arena",
        82 => "Cabilis West",
        106 => "Cabilis East",
        155 => "The City of Shar Vahl",
        394 => "Crescent Reach",
        _ => return format!("Server starting zone {zone}"),
    }
    .into()
}
fn field_text(screen: &CreationScreen, field: Field) -> (String, String, bool) {
    match field {
        Field::Name => ("Name".into(), screen.name.text.clone(), false),
        Field::Choice(field) => {
            let value = screen.editor.value(field);
            let (label, value) = match field {
                ChoiceField::Race => ("Race", race_name(value)),
                ChoiceField::Class => ("Class", class_name(value as u8).into()),
                ChoiceField::Gender => {
                    ("Gender", if value == 0 { "Male" } else { "Female" }.into())
                }
                ChoiceField::Deity => ("Deity", deity_name(value)),
                ChoiceField::StartZone => ("Starting zone", starting_zone_name(value)),
            };
            (label.into(), value, screen.editor.choices(field).len() > 1)
        }
        Field::Stat(stat) => (
            stat.label().into(),
            stat.value(screen.editor.draft().stats).to_string(),
            true,
        ),
        Field::Feature(feature) => (
            feature_label(feature).into(),
            screen.editor.feature(feature).to_string(),
            screen.editor.feature_values(feature).len() > 1,
        ),
    }
}

impl Paint<'_> {
    pub(super) fn creation(
        &mut self,
        input: &AccountInput,
        preview_status: Option<&str>,
        time: f32,
    ) {
        let Some(screen) = &input.creation else {
            return;
        };
        if screen.status(self.view) {
            self.creation_status(screen, input);
            return;
        }
        if screen.step == Step::Review {
            self.creation_review(screen, input, preview_status);
            return;
        }
        self.fill(self.screen, [9, 13, 20, 255]);
        let panel = Rect::new(
            ((self.screen.width - 640.) * 0.5).max(0.),
            ((self.screen.height - 480.) * 0.5).max(0.),
            self.screen.width.min(640.),
            self.screen.height.min(480.),
        );
        self.skin("CharacterListWnd", panel, "", true, false, false, true);
        let inner = Rect::new(
            panel.x + 12.,
            panel.y + 12.,
            (panel.width - 24.).max(0.),
            (panel.height - 24.).max(0.),
        );
        self.fill(inner, [9, 13, 20, 225]);
        let narrow = inner.width < 240.;
        self.text(
            Rect::new(
                inner.x + 8.,
                inner.y + 6.,
                inner.width
                    - if screen.step == Step::Stats && !narrow {
                        92.
                    } else {
                        16.
                    },
                if narrow { 36. } else { 24. },
            ),
            &format!("{} · {}/5", screen.step.title(), screen.step.index() + 1),
            3,
            GOLD,
            narrow,
        );
        if screen.step == Step::Stats {
            self.button(
                "CLW_Quit_Button",
                "create_reset",
                Rect::new(
                    inner.right() - 78.,
                    inner.y + if narrow { 45. } else { 2. },
                    70.,
                    28.,
                ),
                "Reset",
                true,
                screen.focus == Control::Reset,
                input,
            );
        }
        let hint = match screen.step {
            Step::Identity => "4–15 letters. The server decides name availability.",
            Step::Origin => "Choices supplied by this server; final zone may differ.",
            Step::Stats if narrow => "",
            Step::Stats => "Remove a point before adding it to another attribute.",
            Step::Appearance => "Only supported preview features are offered.",
            Step::Review => "",
        };
        self.text(
            Rect::new(
                inner.x + 8.,
                inner.y + if narrow { 44. } else { 34. },
                inner.width - 16.,
                if narrow { 48. } else { 30. },
            ),
            hint,
            1,
            MUTED,
            true,
        );
        let rows = screen.fields();
        let row_top = if narrow { 96. } else { 68. };
        let available = (inner.height - row_top - 78.).max(28.);
        let row_height = if narrow {
            68.
        } else if inner.height < 300. {
            36.
        } else {
            43.
        };
        let visible = (available / row_height).floor().max(1.) as usize;
        let first = screen.first.min(rows.len().saturating_sub(visible));
        for (index, field) in rows.iter().enumerate().skip(first).take(visible) {
            let rect = Rect::new(
                inner.x + 8.,
                inner.y + row_top + (index - first) as f32 * row_height,
                inner.width - 16.,
                row_height - 3.,
            );
            let (label, value, editable) = field_text(screen, *field);
            if *field == Field::Name {
                self.text(
                    Rect::new(rect.x, rect.y, rect.width, 14.),
                    &label,
                    1,
                    MUTED,
                    false,
                );
                let edit = Rect::new(
                    rect.x,
                    rect.y + 14.,
                    rect.width,
                    (rect.height - 14.).clamp(0., 28.),
                );
                self.fill(edit, [8, 13, 20, 255]);
                self.skin("LOGIN_UsernameEdit", edit, "", true, false, false, false);
                if screen.focus == Control::Row(index) {
                    self.outline(edit, GOLD);
                }
                let display = screen.name.display(
                    false,
                    screen.focus == Control::Row(index) && input.focused,
                    time.rem_euclid(1.2) < 0.7,
                    ((edit.width - 20.) / 10.).max(1.) as usize,
                );
                self.text(
                    Rect::new(edit.x + 6., edit.y + 2., edit.width - 12., edit.height - 3.),
                    &display,
                    2,
                    WHITE,
                    false,
                );
                self.hit(&format!("create_row:{index}"), "CreationName", edit, true);
            } else {
                self.fill(rect, [19, 27, 39, 245]);
                if screen.focus == Control::Row(index) {
                    self.outline(rect, GOLD);
                }
                let button = rect.height.min(if narrow { 27. } else { 32. });
                let button_y = if narrow {
                    rect.bottom() - button
                } else {
                    rect.y
                };
                let text_width =
                    (rect.width - if narrow { 10. } else { button * 2. + 16. }).max(0.);
                self.text(
                    Rect::new(rect.x + 5., rect.y + 2., text_width, 13.),
                    &label,
                    1,
                    MUTED,
                    false,
                );
                self.text(
                    Rect::new(
                        rect.x + 5.,
                        rect.y + 15.,
                        text_width,
                        if narrow { 20. } else { rect.height - 16. },
                    ),
                    &value,
                    2,
                    WHITE,
                    false,
                );
                self.hit(
                    &format!("create_row:{index}"),
                    "CreationRow",
                    Rect::new(
                        rect.x,
                        rect.y,
                        text_width,
                        if narrow { 35. } else { rect.height },
                    ),
                    true,
                );
                for (action, label, x) in [
                    ("create_less", "−", rect.right() - button * 2. - 3.),
                    ("create_more", "+", rect.right() - button),
                ] {
                    self.button(
                        "CLW_Quit_Button",
                        &format!("{action}:{index}"),
                        Rect::new(x, button_y, button, button),
                        label,
                        editable,
                        false,
                        input,
                    );
                }
            }
        }
        if rows.is_empty() {
            self.text(
                Rect::new(inner.x + 8., inner.y + 76., inner.width - 16., 62.),
                screen
                    .editor
                    .preview_problem()
                    .unwrap_or("Loading supported appearance choices…"),
                2,
                MUTED,
                true,
            );
        }
        let status = screen.notice.as_deref().or_else(|| {
            self.view
                .creation
                .as_ref()
                .and_then(|view| view.problem.as_deref())
        });
        let footer = inner.bottom() - 69.;
        let detail = if let Some(status) = status {
            status.to_owned()
        } else if screen.step == Step::Stats {
            format!(
                "{} points left · {}–{}/{}{}",
                screen.editor.remaining_points().unwrap_or(0),
                first + 1,
                (first + visible).min(rows.len()),
                rows.len(),
                if rows.len() > visible {
                    " · Scroll or ↑/↓"
                } else {
                    ""
                }
            )
        } else if rows.len() > visible {
            format!(
                "{}–{} of {} · Scroll or ↑/↓",
                first + 1,
                (first + visible).min(rows.len()),
                rows.len()
            )
        } else if screen
            .editor
            .policy()
            .is_some_and(|policy| policy.colors_not_previewed)
            && screen.step == Step::Appearance
        {
            "Hair colors are fixed until their preview is supported.".into()
        } else {
            String::new()
        };
        self.text(
            Rect::new(inner.x + 8., footer, inner.width - 16., 28.),
            &detail,
            1,
            MUTED,
            true,
        );
        let width = (inner.width - 28.) / 3.;
        let y = inner.bottom() - 35.;
        self.button(
            "CLW_Quit_Button",
            "create_close",
            Rect::new(inner.x + 8., y, width, 28.),
            "Close",
            true,
            screen.focus == Control::Close,
            input,
        );
        self.button(
            "CLW_Quit_Button",
            "create_previous",
            Rect::new(inner.x + 14. + width, y, width, 28.),
            "Back",
            screen.step != Step::Identity,
            screen.focus == Control::Previous,
            input,
        );
        self.button(
            "CLW_Play_Button",
            "create_next",
            Rect::new(inner.x + 20. + width * 2., y, width, 28.),
            "Next",
            !screen.composing,
            screen.focus == Control::Next,
            input,
        );
    }
    fn creation_review(
        &mut self,
        screen: &CreationScreen,
        input: &AccountInput,
        status: Option<&str>,
    ) {
        let compact = self.screen.height < 360.;
        let header = if compact { 56. } else { 82. };
        let footer = if compact { 78. } else { 100. };
        if status.is_some() {
            self.fill(self.screen, [9, 13, 20, 255]);
        }
        self.fill(
            Rect::new(0., 0., self.screen.width, header),
            [9, 13, 20, 238],
        );
        self.fill(
            Rect::new(0., self.screen.height - footer, self.screen.width, footer),
            [9, 13, 20, 238],
        );
        let draft = screen.editor.draft();
        self.text(
            Rect::new(12., 7., (self.screen.width - 96.).max(0.), 25.),
            if draft.name.is_empty() {
                "Choose a name before creating"
            } else {
                &draft.name
            },
            if compact { 3 } else { 4 },
            GOLD,
            false,
        );
        self.button(
            "CLW_Quit_Button",
            "create_close",
            Rect::new((self.screen.width - 74.).max(0.), 5., 66., 28.),
            "Close",
            true,
            screen.focus == Control::Close,
            input,
        );
        self.text(
            Rect::new(
                12.,
                if compact { 33. } else { 43. },
                self.screen.width - 24.,
                20.,
            ),
            &format!(
                "{} · {} · {}",
                race_name(draft.choice.race),
                class_name(draft.choice.class as u8),
                deity_name(draft.choice.deity)
            ),
            1,
            MUTED,
            false,
        );
        if let Some(status) = status {
            self.text(
                Rect::new(
                    16.,
                    header + 12.,
                    self.screen.width - 32.,
                    (self.screen.height - header - footer - 20.).max(0.),
                ),
                status,
                2,
                WHITE,
                true,
            );
        }
        let validation = screen.editor.submission();
        let detail = screen
            .notice
            .clone()
            .or_else(|| {
                self.view
                    .creation
                    .as_ref()
                    .and_then(|view| view.problem.clone())
            })
            .or_else(|| screen.editor.preview_problem().map(str::to_owned))
            .or_else(|| validation.as_ref().err().map(ToString::to_string))
            .unwrap_or_else(|| "Create commits this character to the server.".into());
        self.text(
            Rect::new(
                12.,
                self.screen.height - footer + 3.,
                self.screen.width - 24.,
                if compact { 23. } else { 35. },
            ),
            &detail,
            1,
            MUTED,
            true,
        );
        let available = (self.screen.width - 28.).max(0.);
        let narrow = self.screen.width < 260.;
        let widths = if narrow {
            [0.27, 0.15, 0.15, 0.43]
        } else {
            [0.25; 4]
        };
        let mut x = 8.;
        let y = self.screen.height - if compact { 47. } else { 55. };
        for (index, (action, label, control)) in [
            ("create_previous", "Back", Control::Previous),
            ("create_rotate_left", "←", Control::RotateLeft),
            ("create_rotate_right", "→", Control::RotateRight),
            ("create_next", "Create", Control::Next),
        ]
        .into_iter()
        .enumerate()
        {
            let width = available * widths[index];
            self.button(
                "CLW_Quit_Button",
                action,
                Rect::new(x, y, width, 28.),
                label,
                index != 3 || validation.is_ok(),
                screen.focus == control,
                input,
            );
            x += width + 4.;
        }
        self.text(
            Rect::new(12., self.screen.height - 16., self.screen.width - 24., 14.),
            if narrow {
                "Escape to edit"
            } else {
                "Tab to move · Escape to edit · Create is final"
            },
            1,
            MUTED,
            false,
        );
    }
    fn creation_status(&mut self, screen: &CreationScreen, input: &AccountInput) {
        self.fill(self.screen, [9, 13, 20, 255]);
        let (title, detail) = match screen.outcome(self.view) {
            Some(Phase::Completed(_)) => (
                "Character created",
                "Return to the roster to enter the world.",
            ),
            Some(Phase::Rejected(Rejection::Name)) => (
                "Name rejected",
                "The server rejected this name. Choose another.",
            ),
            Some(Phase::Rejected(Rejection::Creation)) => (
                "Creation rejected",
                "The server rejected the character. Review the draft before trying again.",
            ),
            Some(Phase::Uncertain(_)) => (
                "Creation was not confirmed",
                "Sign in again to check your character list.",
            ),
            Some(Phase::Cancelled) => (
                "Creation cancelled",
                "No character creation request was sent.",
            ),
            _ => (
                "Creating character…",
                "You can close this screen while creation continues. Wait for the server’s result.",
            ),
        };
        let x = 16.;
        let width = (self.screen.width - 32.).max(0.);
        let narrow = self.screen.width < 260.;
        let detail_top = if narrow { 112. } else { 74. };
        self.text(
            Rect::new(x, 24., width, if narrow { 78. } else { 34. }),
            title,
            4,
            GOLD,
            narrow,
        );
        self.text(
            Rect::new(
                x,
                detail_top,
                width,
                (self.screen.height - detail_top - 70.).max(0.),
            ),
            detail,
            2,
            WHITE,
            true,
        );
        let y = (self.screen.height - 50.).max(0.);
        self.button(
            "CLW_Quit_Button",
            "create_close",
            Rect::new(x, y, width * 0.45, 30.),
            "Close",
            true,
            screen.focus == Control::Close,
            input,
        );
        if matches!(screen.outcome(self.view), Some(Phase::Rejected(_))) {
            self.button(
                "CLW_Quit_Button",
                "create_previous",
                Rect::new(x + width * 0.55, y, width * 0.45, 30.),
                if narrow { "Edit" } else { "Edit draft" },
                true,
                screen.focus == Control::Previous,
                input,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{click, frame, press, release, send, window};
    use super::*;
    use crate::account::CreationView;
    use crate::account_creation::{
        PreviewFamily, Uncertainty,
        editor::tests::{attach_preview, selection},
    };
    use bevy::input::mouse::MouseButtonInput;

    fn view() -> View {
        View {
            stage: Stage::Characters,
            token: Token {
                attempt: 3,
                revision: 8,
            },
            creation: Some(CreationView {
                selection: selection(),
                operation: None,
                phase: None,
                detached: false,
                submitting: false,
                problem: None,
            }),
            ..Default::default()
        }
    }
    fn opened(view: &View) -> AccountInput {
        let mut input = AccountInput::new(Endpoint::default());
        input.sync(view);
        let roster = frame(view, &input);
        assert!(click(&mut input, view, &roster, "new_character").is_none());
        assert!(input.creation.is_some());
        input
    }
    fn tap(input: &mut AccountInput, view: &View, action: &str) -> Option<Intent> {
        let current = frame(view, input);
        click(input, view, &current, action)
    }
    fn named(input: &mut AccountInput, view: &View) {
        send(input, view, press(KeyCode::KeyA, Some("aSTERIA123")));
        send(input, view, release(KeyCode::KeyA));
        assert_eq!(
            input.creation.as_ref().unwrap().editor.draft().name,
            "Asteria"
        );
    }
    fn review(input: &mut AccountInput, view: &View) {
        for _ in 0..4 {
            assert!(tap(input, view, "create_next").is_none());
        }
        assert!(input.creation_showing_preview(view));
    }
    fn preedit() -> WindowEvent {
        WindowEvent::Ime(Ime::Preedit {
            window: window(),
            value: "Pending".into(),
            cursor: Some((0, 7)),
        })
    }
    fn commit() -> WindowEvent {
        WindowEvent::Ime(Ime::Commit {
            window: window(),
            value: "Unexpected".into(),
        })
    }
    #[test]
    fn browsing_is_local_and_only_exact_review_receipt_allows_final_create() {
        let view = view();
        let mut input = opened(&view);
        named(&mut input, &view);
        let context = input.creation_request(&view).unwrap().0;
        assert_eq!(input.creation_request(&view).unwrap().1.name, "Asteria");
        review(&mut input, &view);
        assert!(tap(&mut input, &view, "create_next").is_none());
        attach_preview(&mut input.creation.as_mut().unwrap().editor);
        let intent = tap(&mut input, &view, "create_next").unwrap();
        let Intent::Create { token, submission } = intent else {
            panic!("expected creation")
        };
        assert_eq!(token, view.token);
        assert_eq!(submission.context, context);
        assert_eq!(submission.draft.name, "Asteria");
        assert_eq!(submission.appearance.family(), PreviewFamily::Classic);
        assert!(tap(&mut input, &view, "create_previous").is_none());
        assert!(!input.creation_showing_preview(&view));
        assert!(tap(&mut input, &view, "create_more:0").is_none());
        assert!(!input.creation.as_ref().unwrap().editor.receipt_ready());
    }
    #[test]
    fn name_editor_is_isolated_and_cancelled_ime_cannot_cross_close_or_reopen() {
        for close in [false, true] {
            let view = view();
            let mut input = opened(&view);
            input.edits[3].text = "Accountlogin".into();
            named(&mut input, &view);
            assert!(send(&mut input, &view, preedit()).is_none());
            assert!(input.creation.as_ref().unwrap().composing);
            if close {
                assert!(tap(&mut input, &view, "create_close").is_none());
                assert!(input.creation.is_none());
                assert!(tap(&mut input, &view, "new_character").is_none());
            } else {
                send(&mut input, &view, press(KeyCode::Escape, None));
                send(&mut input, &view, release(KeyCode::Escape));
            }
            send(&mut input, &view, commit());
            assert_eq!(
                input.creation.as_ref().unwrap().name.text,
                if close { "" } else { "Asteria" }
            );
            assert_eq!(input.edits[3].text, "Accountlogin");
            assert!(!input.creation.as_ref().unwrap().composing);
        }
    }
    #[test]
    fn held_enter_cannot_advance_across_creation_pages() {
        let view = view();
        let mut input = opened(&view);
        named(&mut input, &view);
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.creation.as_ref().unwrap().step == Step::Origin);
        input.creation.as_mut().unwrap().focus = Control::Next;
        for _ in 0..8 {
            assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        }
        assert!(input.creation.as_ref().unwrap().step == Step::Origin);
        send(&mut input, &view, release(KeyCode::Enter));
        assert!(send(&mut input, &view, press(KeyCode::Enter, None)).is_none());
        assert!(input.creation.as_ref().unwrap().step == Step::Stats);
    }
    #[test]
    fn refreshing_catalog_or_roster_between_press_and_release_retires_click_and_receipt() {
        for catalog in [false, true] {
            let mut view = view();
            let mut input = opened(&view);
            named(&mut input, &view);
            review(&mut input, &view);
            attach_preview(&mut input.creation.as_mut().unwrap().editor);
            let old = frame(&view, &input);
            let hit = old
                .hit_targets
                .iter()
                .find(|h| h.screen_id == "create_next")
                .unwrap();
            input.pointer = Some([hit.rect.x + 1., hit.rect.y + 1.]);
            input.event(
                &view,
                &old,
                window(),
                &WindowEvent::MouseButtonInput(MouseButtonInput {
                    window: window(),
                    button: MouseButton::Left,
                    state: ButtonState::Pressed,
                }),
            );
            assert!(input.pressed.is_some());
            let selection = &mut view.creation.as_mut().unwrap().selection;
            if catalog {
                selection.catalog_revision += 1;
            } else {
                selection.roster_revision += 1;
            }
            assert!(
                input
                    .event(
                        &view,
                        &old,
                        window(),
                        &WindowEvent::MouseButtonInput(MouseButtonInput {
                            window: window(),
                            button: MouseButton::Left,
                            state: ButtonState::Released,
                        })
                    )
                    .is_none()
            );
            assert!(input.pressed.is_none());
            assert!(!input.creation.as_ref().unwrap().editor.receipt_ready());
            assert!(!current_frame(&view, &input, &old));
        }
    }
    #[test]
    fn pending_and_unknown_outcomes_never_offer_retry_and_close_targets_exact_operation() {
        let mut view = view();
        let mut input = opened(&view);
        named(&mut input, &view);
        review(&mut input, &view);
        attach_preview(&mut input.creation.as_mut().unwrap().editor);
        let operation = account_creation::Token {
            context: input.creation.as_ref().unwrap().editor.context(),
            operation: 88,
        };
        let creation = view.creation.as_mut().unwrap();
        creation.operation = Some(operation);
        creation.phase = Some(Phase::AwaitingApproval);
        assert!(input.creation_action(&view, "create_next").is_none());
        assert!(
            !frame(&view, &input)
                .hit_targets
                .iter()
                .any(|h| h.screen_id == "create_next")
        );
        assert_eq!(
            tap(&mut input, &view, "create_close"),
            Some(Intent::CancelCreation {
                token: view.token,
                operation
            })
        );
        assert!(input.creation.is_none());
        assert!(tap(&mut input, &view, "new_character").is_none());
        assert!(input.creation.is_none());
        view.creation.as_mut().unwrap().phase = Some(Phase::Rejected(Rejection::Name));
        input.open_creation(&view);
        view.creation.as_mut().unwrap().phase = Some(Phase::Uncertain(Uncertainty::Transport));
        // A new draft ignores an old outcome, but a new uncertain operation is terminal.
        view.creation
            .as_mut()
            .unwrap()
            .operation
            .as_mut()
            .unwrap()
            .operation += 1;
        assert!(
            !frame(&view, &input)
                .hit_targets
                .iter()
                .any(|h| h.screen_id == "create_previous")
        );
        assert!(input.creation_action(&view, "create_next").is_none());
        view.creation.as_mut().unwrap().phase = Some(Phase::Rejected(Rejection::Name));
        assert!(tap(&mut input, &view, "create_previous").is_none());
        assert!(input.creation.as_ref().unwrap().step == Step::Identity);
    }
    #[test]
    fn stale_creation_never_covers_new_stage_even_without_input_events() {
        let mut view = view();
        let mut input = opened(&view);
        review(&mut input, &view);
        send(&mut input, &view, release(KeyCode::Enter));
        view.stage = Stage::Credentials;
        view.token.attempt += 1;
        view.creation = None;
        assert!(
            !frame(&view, &input)
                .hit_targets
                .iter()
                .any(|h| h.screen_id.starts_with("create_"))
        );
        input.sync(&view);
        assert!(input.creation.is_none());
    }
    #[test]
    fn all_creation_actions_fit_compact_viewports_and_review_avoids_avatar() {
        let view = view();
        let mut input = opened(&view);
        for viewport in [[800, 600], [320, 240], [180, 640]] {
            for step in Step::ALL {
                input.creation.as_mut().unwrap().step = step;
                let frame = AccountUi::default().frame(viewport, &view, &input, 0.);
                for hit in frame.hit_targets.iter().filter(|h| h.screen_id != "frame") {
                    assert!(!hit.rect.is_empty(), "{}", hit.screen_id);
                    assert!(hit.rect.x >= 0. && hit.rect.right() <= viewport[0] as f32);
                    assert!(hit.rect.y >= 0. && hit.rect.bottom() <= viewport[1] as f32);
                    if step == Step::Review {
                        assert!(
                            hit.rect
                                .intersect(crate::account_preview::model_rect(viewport))
                                .is_empty(),
                            "{}",
                            hit.screen_id
                        );
                    }
                }
                for action in ["create_close", "create_previous", "create_next"] {
                    assert!(frame.hit_targets.iter().any(|h| h.screen_id == action));
                }
            }
        }
    }
    #[test]
    #[ignore = "requires original UI/character assets and GPU; no network or audio"]
    fn original_creation_pages_at_both_scales_and_compact() {
        use crate::account_preview::{Preview, Request};
        use openeq_render::{Renderer, actors::CharacterModelSet};
        use std::time::{Duration, Instant};
        let dir = std::env::var_os("EQ_CLIENT_DIR")
            .map(std::path::PathBuf::from)
            .or_else(openeq_assets::loader::default_client_dir)
            .unwrap();
        let ui = AccountUi::load(&dir);
        assert!(ui.characters.is_some());
        let output = std::env::var_os("OPENEQ_CREATION_CAPTURE_DIR").map(std::path::PathBuf::from);
        if let Some(output) = &output {
            std::fs::create_dir_all(output).unwrap();
        }
        let mut renderer = Renderer::new_headless(800, 600).unwrap();
        let mut view = view();
        let mut input = opened(&view);
        named(&mut input, &view);
        let mut preview = Preview::default();
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            let (context, character) = input.creation_request(&view).unwrap();
            let request = Request {
                token: view.token,
                creation: Some(context),
                character,
                dir: dir.clone(),
                model_set: CharacterModelSet::Classic,
            };
            preview.update(Some(request.clone()), &renderer);
            if preview.status().is_none() {
                let proof = preview.creation_preview(&request).unwrap();
                input.apply_creation_preview(&view, context, Ok(proof));
            }
            if input.creation.as_ref().unwrap().editor.receipt_ready() {
                break;
            }
            assert!(
                preview
                    .status()
                    .is_none_or(|text| !text.contains("unavailable")),
                "{:?}",
                preview.status()
            );
            assert!(Instant::now() < deadline, "creation preview timed out");
            std::thread::sleep(Duration::from_millis(8));
        }
        for (viewport, scale) in [
            ([800, 600], 1_u32),
            ([800, 600], 2),
            ([320, 240], 1),
            ([180, 640], 1),
        ] {
            renderer.resize(viewport[0] * scale, viewport[1] * scale);
            for (label, step) in [
                ("identity", Step::Identity),
                ("origin", Step::Origin),
                ("stats", Step::Stats),
                ("appearance", Step::Appearance),
                ("review", Step::Review),
            ] {
                let screen = input.creation.as_mut().unwrap();
                screen.step = step;
                screen.first = 0;
                screen.focus = if step == Step::Review {
                    Control::Next
                } else {
                    Control::Row(0)
                };
                let frame = ui.frame(viewport, &view, &input, 0.4);
                assert!(frame.commands.iter().any(|command| matches!(command, DrawCommand::Image { texture, .. } if texture.exists())));
                renderer.set_ui_scaled(&frame, scale as f32);
                if step == Step::Review {
                    assert!(
                        frame
                            .hit_targets
                            .iter()
                            .any(|hit| hit.screen_id == "create_next" && hit.enabled)
                    );
                    assert!(preview.render(&mut renderer, viewport, 256.));
                } else {
                    renderer.render_ui();
                }
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                assert!(pixels.chunks_exact(4).any(|p| p[0] > 80 && p[1] > 80));
                if step == Step::Review {
                    let area = crate::account_preview::model_rect(viewport);
                    let body: Vec<_> = pixels
                        .chunks_exact(4)
                        .enumerate()
                        .filter(|(i, _)| {
                            area.contains([
                                (*i as u32 % width) as f32 / scale as f32,
                                (*i as u32 / width) as f32 / scale as f32,
                            ])
                        })
                        .map(|(_, p)| p)
                        .collect();
                    assert!(
                        body.iter()
                            .filter(|p| p[0].max(p[1]).max(p[2]) > 80)
                            .count()
                            > 100,
                        "avatar absent"
                    );
                    assert!(
                        body.iter()
                            .filter(|p| p[0] > 230 && p[1] < 25 && p[2] > 230)
                            .count()
                            < 10,
                        "placeholder texture"
                    );
                }
                if let Some(output) = &output {
                    image::save_buffer(
                        output.join(format!(
                            "{label}-{}x{}-{scale}x.png",
                            viewport[0], viewport[1]
                        )),
                        &pixels,
                        width,
                        height,
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        }
        view.creation.as_mut().unwrap().submitting = true;
        for scale in [1_u32, 2] {
            renderer.resize(800 * scale, 600 * scale);
            let frame = ui.frame([800, 600], &view, &input, 0.);
            renderer.set_ui_scaled(&frame, scale as f32);
            renderer.render_ui();
            let (width, height, pixels) = renderer.read_rgba().unwrap();
            if let Some(output) = &output {
                image::save_buffer(
                    output.join(format!("pending-{scale}x.png")),
                    &pixels,
                    width,
                    height,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn detached_results_remain_visible_without_covering_compact_roster_controls() {
        for phase in [
            Phase::AwaitingApproval,
            Phase::Rejected(Rejection::Name),
            Phase::Rejected(Rejection::Creation),
            Phase::Uncertain(Uncertainty::Transport),
        ] {
            let mut view = view();
            let mut input = opened(&view);
            let operation = account_creation::Token {
                context: input.creation.as_ref().unwrap().editor.context(),
                operation: 94,
            };
            let creation = view.creation.as_mut().unwrap();
            creation.operation = Some(operation);
            creation.phase = Some(phase.clone());
            creation.detached = true;
            input.close_creation(&view);
            for viewport in [[800, 600], [320, 240], [180, 640]] {
                let frame = AccountUi::default().frame(viewport, &view, &input, 0.);
                let (text, rect) = frame
                    .commands
                    .iter()
                    .find_map(|command| match command {
                        DrawCommand::Text { text, rect, .. }
                            if text.contains("Creation continues")
                                || text.contains("rejected")
                                || text.contains("not confirmed") =>
                        {
                            Some((text, rect))
                        }
                        _ => None,
                    })
                    .expect("detached outcome disappeared");
                assert!(!text.contains("No characters"));
                for hit in frame.hit_targets.iter().filter(|hit| {
                    ["back", "new_character", "preview", "primary"]
                        .contains(&hit.screen_id.as_str())
                }) {
                    assert!(
                        rect.intersect(hit.rect).is_empty(),
                        "status covers {}",
                        hit.screen_id
                    );
                }
                if matches!(phase, Phase::Uncertain(_)) {
                    assert!(
                        !frame
                            .hit_targets
                            .iter()
                            .find(|hit| hit.screen_id == "new_character")
                            .unwrap()
                            .enabled
                    );
                }
            }
            if matches!(phase, Phase::Uncertain(_)) {
                input.open_creation(&view);
                assert!(input.creation.is_none());
            }
        }
    }
    #[test]
    #[ignore = "requires original UI assets and GPU; no network or audio"]
    fn original_creation_outcomes_at_both_scales_and_compact() {
        let dir = std::env::var_os("EQ_CLIENT_DIR")
            .map(std::path::PathBuf::from)
            .or_else(openeq_assets::loader::default_client_dir)
            .unwrap();
        let ui = AccountUi::load(&dir);
        assert!(ui.characters.is_some());
        let output = std::env::var_os("OPENEQ_CREATION_CAPTURE_DIR").map(std::path::PathBuf::from);
        if let Some(output) = &output {
            std::fs::create_dir_all(output).unwrap();
        }
        let mut renderer = openeq_render::Renderer::new_headless(800, 600).unwrap();
        for label in [
            "pending",
            "complete",
            "name-rejected",
            "create-rejected",
            "unknown",
            "cancelled",
            "detached-pending",
            "detached-existing",
            "detached-rejected",
            "detached-unknown",
        ] {
            let mut view = view();
            let mut input = opened(&view);
            named(&mut input, &view);
            let editor = &input.creation.as_ref().unwrap().editor;
            let operation = account_creation::Token {
                context: editor.context(),
                operation: 95,
            };
            let character = editor.draft().preview_character().unwrap();
            let creation = view.creation.as_mut().unwrap();
            creation.operation = Some(operation);
            creation.phase = Some(match label {
                "complete" => Phase::Completed(Box::new(character.clone())),
                "name-rejected" | "detached-rejected" => Phase::Rejected(Rejection::Name),
                "create-rejected" => Phase::Rejected(Rejection::Creation),
                "unknown" | "detached-unknown" => Phase::Uncertain(Uncertainty::Transport),
                "cancelled" => Phase::Cancelled,
                _ => Phase::AwaitingApproval,
            });
            if label.starts_with("detached") {
                creation.detached = true;
                if label == "detached-existing" {
                    let mut existing = character.clone();
                    existing.name = "Existing".into();
                    existing.enabled = true;
                    view.selected_character = Some(existing.name.clone());
                    view.characters.push(existing);
                }
                input.close_creation(&view);
            }
            for (viewport, scale) in [
                ([800, 600], 1_u32),
                ([800, 600], 2),
                ([320, 240], 1),
                ([180, 640], 1),
            ] {
                renderer.resize(viewport[0] * scale, viewport[1] * scale);
                let frame = ui.frame(viewport, &view, &input, 0.);
                assert!(
                    !frame
                        .hit_targets
                        .iter()
                        .any(|hit| hit.screen_id == "create_next")
                );
                renderer.set_ui_scaled(&frame, scale as f32);
                renderer.render_ui();
                let (width, height, pixels) = renderer.read_rgba().unwrap();
                if let Some(output) = &output {
                    image::save_buffer(
                        output.join(format!(
                            "outcome-{label}-{}x{}-{scale}x.png",
                            viewport[0], viewport[1]
                        )),
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
}
