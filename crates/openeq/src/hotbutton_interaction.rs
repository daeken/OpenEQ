//! User-configured single-command buttons. Drafts and interaction identities are
//! transient; only validated labels and command strings are saved.
use crate::{
    chat::ChatEditor,
    gameplay_ui::GameHudState,
    hotbutton_ui::{
        HotbuttonAction, HotbuttonActionKind, UiHotbutton, UiHotbuttonEditor, UiHotbuttons,
    },
    hotbuttons::{
        HotbuttonBindings, HotbuttonField, HotbuttonToken, parse_hotbutton_command,
        validate_hotbutton,
    },
    interaction::Interaction,
    live::LiveWorld,
};

pub struct HotbuttonDraft {
    pub slot: u8,
    pub label: ChatEditor,
    pub command: ChatEditor,
    pub focus: HotbuttonField,
    pub label_selected: bool,
    pub command_selected: bool,
    pub error: Option<String>,
}
impl HotbuttonDraft {
    pub fn edit(&self, field: HotbuttonField) -> &ChatEditor {
        match field {
            HotbuttonField::Label => &self.label,
            HotbuttonField::Command => &self.command,
        }
    }
    pub fn edit_mut(&mut self, field: HotbuttonField) -> &mut ChatEditor {
        match field {
            HotbuttonField::Label => &mut self.label,
            HotbuttonField::Command => &mut self.command,
        }
    }
    pub fn selected(&self, field: HotbuttonField) -> bool {
        match field {
            HotbuttonField::Label => self.label_selected,
            HotbuttonField::Command => self.command_selected,
        }
    }
    pub fn set_selected(&mut self, field: HotbuttonField, selected: bool) {
        match field {
            HotbuttonField::Label => self.label_selected = selected,
            HotbuttonField::Command => self.command_selected = selected,
        }
    }
}

pub struct HotbuttonState {
    bindings: HotbuttonBindings,
    pub open: bool,
    pub editor: Option<HotbuttonDraft>,
    revision: u64,
    zone_generation: u64,
    available: bool,
}
impl Default for HotbuttonState {
    fn default() -> Self {
        Self {
            bindings: Default::default(),
            open: true,
            editor: None,
            revision: 0,
            zone_generation: 0,
            available: false,
        }
    }
}
impl HotbuttonState {
    pub fn bindings(&self) -> &HotbuttonBindings {
        &self.bindings
    }
    pub fn token(&self) -> HotbuttonToken {
        HotbuttonToken {
            revision: self.revision,
            zone_generation: self.zone_generation,
        }
    }
    pub fn available(&self) -> bool {
        self.available
    }
    fn bump(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn begin_session(&mut self, bindings: HotbuttonBindings) {
        self.bindings = bindings;
        self.editor = None;
        self.available = false;
        self.bump();
    }
    pub fn sync_context(&mut self, zone_generation: u64, available: bool) {
        if self.zone_generation != zone_generation || self.available != available {
            self.zone_generation = zone_generation;
            self.available = available;
            self.editor = None;
            self.bump();
        }
    }
    pub fn set_open(&mut self, open: bool) {
        if self.open != open {
            self.open = open;
            self.editor = None;
            self.bump();
        }
    }
    pub fn open_editor(&mut self, token: HotbuttonToken, slot: u8) -> bool {
        if token != self.token() || !self.available || usize::from(slot) >= self.bindings.len() {
            return false;
        }
        let binding = self.bindings[usize::from(slot)].as_ref();
        let mut label = ChatEditor::default();
        label.open(binding.map_or("", |binding| binding.label.as_str()));
        let mut command = ChatEditor::default();
        command.open(binding.map_or("", |binding| binding.command.as_str()));
        self.editor = Some(HotbuttonDraft {
            slot,
            label,
            command,
            focus: HotbuttonField::Label,
            label_selected: false,
            command_selected: false,
            error: None,
        });
        self.open = true;
        self.bump();
        true
    }
    pub fn cancel_editor(&mut self) {
        if self.editor.take().is_some() {
            self.bump();
        }
    }
    pub fn clear_draft(&mut self) {
        if let Some(editor) = &mut self.editor {
            editor.label.open("");
            editor.command.open("");
            editor.focus = HotbuttonField::Label;
            editor.label_selected = false;
            editor.command_selected = false;
            editor.error = None;
            self.bump();
        }
    }
    pub fn save_editor(&mut self, token: HotbuttonToken) -> bool {
        if token != self.token() || !self.available {
            return false;
        }
        let Some(editor) = &mut self.editor else {
            return false;
        };
        match validate_hotbutton(&editor.label.text, &editor.command.text) {
            Ok(binding) => {
                self.bindings[usize::from(editor.slot)] = binding;
                self.editor = None;
                self.bump();
                true
            }
            Err(error) => {
                editor.error = Some(error.to_string());
                false
            }
        }
    }
    pub fn command_for_activation(&self, token: HotbuttonToken, slot: u8) -> Option<&str> {
        if token != self.token() || !self.open || !self.available || self.editor.is_some() {
            return None;
        }
        self.bindings
            .get(usize::from(slot))?
            .as_ref()
            .map(|binding| binding.command.as_str())
    }
}

fn edit_view(editor: &ChatEditor) -> (String, Option<usize>) {
    (
        format!(
            "{}{}{}",
            &editor.text[..editor.cursor],
            editor.preedit,
            &editor.text[editor.cursor..]
        ),
        Some(editor.cursor + editor.preedit.len()),
    )
}

impl Interaction {
    pub(crate) fn hotbutton_view(&self, view: &mut GameHudState) {
        let state = &self.hotbuttons;
        let token = state.token();
        let editor = state.editor.as_ref().map(|editor| {
            let (label, label_cursor) = edit_view(&editor.label);
            let (command, command_cursor) = edit_view(&editor.command);
            UiHotbuttonEditor {
                token,
                slot: editor.slot,
                label,
                command,
                label_cursor,
                command_cursor,
                focus: editor.focus,
                error: editor.error.clone(),
                label_selected: editor.label_selected,
                command_selected: editor.command_selected,
            }
        });
        if editor.is_some() {
            view.hover_blocked = true;
            view.inspected_item = None;
        }
        view.hotbuttons = Some(UiHotbuttons {
            token,
            open: state.open,
            enabled: state.available,
            slots: std::array::from_fn(|slot| {
                state.bindings[slot].as_ref().map(|binding| UiHotbutton {
                    label: binding.label.clone(),
                    command: binding.command.clone(),
                })
            }),
            editor,
            pressed: self
                .hotbutton_input
                .pressed()
                .filter(|action| action.token == token)
                .cloned(),
        });
    }
    pub fn sync_hotbuttons(&mut self, zone_generation: u64, available: bool) {
        self.hotbuttons.sync_context(zone_generation, available);
    }
    pub fn activate_hotbutton(
        &mut self,
        token: HotbuttonToken,
        slot: u8,
        live: &mut LiveWorld,
        position: [f32; 3],
    ) -> bool {
        self.sync_hotbuttons(
            live.zone_generation(),
            live.ready && live.error.is_none() && !live.game.recovery.blocks_movement(),
        );
        let Some(command) = self
            .hotbuttons
            .command_for_activation(token, slot)
            .map(str::to_owned)
        else {
            return false;
        };
        match parse_hotbutton_command(&command) {
            Ok(action) => self.action(action, live, position),
            Err(error) => {
                live.game.error(error.to_string());
                false
            }
        }
    }
    /// Called once for a validated native release (or an explicit editor key).
    /// The returned Quit result goes through the ordinary application exit path.
    pub fn hotbutton_action(
        &mut self,
        action: HotbuttonAction,
        right: bool,
        live: &mut LiveWorld,
        position: [f32; 3],
    ) -> bool {
        self.sync_hotbuttons(
            live.zone_generation(),
            live.ready && live.error.is_none() && !live.game.recovery.blocks_movement(),
        );
        if action.token != self.hotbuttons.token() {
            return false;
        }
        match action.kind {
            HotbuttonActionKind::Slot(slot) => {
                if self.hotbuttons.editor.is_some() {
                    return false;
                }
                if right
                    || self
                        .hotbuttons
                        .bindings
                        .get(usize::from(slot))
                        .is_some_and(Option::is_none)
                {
                    if self.hotbuttons.open_editor(action.token, slot) {
                        self.chat_input.cancel_for_handoff(&mut self.editor);
                    }
                } else {
                    return self.activate_hotbutton(action.token, slot, live, position);
                }
            }
            HotbuttonActionKind::Focus(field) => {
                if let Some(editor) = &mut self.hotbuttons.editor {
                    editor.focus = field;
                }
            }
            HotbuttonActionKind::Clear => self.hotbuttons.clear_draft(),
            HotbuttonActionKind::Save => {
                self.hotbuttons.save_editor(action.token);
            }
            HotbuttonActionKind::Cancel => self.hotbuttons.cancel_editor(),
            HotbuttonActionKind::CloseBar => self.hotbuttons.set_open(false),
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live::{NetworkCommand, tests::command_world};
    use openeq_net::gameplay::{ChatChannel, Command};

    fn configured(command: &str) -> HotbuttonState {
        let mut bindings = HotbuttonBindings::default();
        bindings[0] = validate_hotbutton("Action", command).unwrap();
        let mut state = HotbuttonState::default();
        state.begin_session(bindings);
        state.sync_context(0, true);
        state
    }

    #[test]
    fn editing_clear_and_cancel_are_local_and_invalid_save_preserves_binding() {
        let (mut live, mut wire) = command_world(1, 1.);
        let mut interaction = Interaction {
            hotbuttons: configured("/quit"),
            ..Default::default()
        };
        let original = interaction.hotbuttons.bindings().clone();
        let action = |state: &HotbuttonState, kind| HotbuttonAction {
            token: state.token(),
            kind,
        };
        assert!(!interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Slot(0)),
            true,
            &mut live,
            [0.; 3]
        ));
        let draft = interaction.hotbuttons.editor.as_mut().unwrap();
        draft.command.open("/unsupported");
        assert!(!interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Save),
            false,
            &mut live,
            [0.; 3]
        ));
        assert_eq!(interaction.hotbuttons.bindings(), &original);
        let draft = interaction.hotbuttons.editor.as_ref().unwrap();
        assert_eq!(draft.command.text, "/unsupported");
        assert!(draft.error.is_some());
        interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Clear),
            false,
            &mut live,
            [0.; 3],
        );
        assert_eq!(interaction.hotbuttons.bindings(), &original);
        assert!(
            interaction
                .hotbuttons
                .editor
                .as_ref()
                .unwrap()
                .command
                .text
                .is_empty()
        );
        interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Cancel),
            false,
            &mut live,
            [0.; 3],
        );
        assert_eq!(interaction.hotbuttons.bindings(), &original);
        interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Slot(0)),
            true,
            &mut live,
            [0.; 3],
        );
        interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Clear),
            false,
            &mut live,
            [0.; 3],
        );
        interaction.hotbutton_action(
            action(&interaction.hotbuttons, HotbuttonActionKind::Save),
            false,
            &mut live,
            [0.; 3],
        );
        assert!(interaction.hotbuttons.bindings()[0].is_none());
        assert!(interaction.hotbuttons.editor.is_none());
        assert!(
            wire.try_recv().is_err(),
            "editing must never dispatch the saved command"
        );
    }

    #[test]
    fn stale_actions_cannot_cross_editor_bar_zone_or_character_lifecycles() {
        let mut state = configured("/loc");
        let before_edit = state.token();
        assert!(state.open_editor(before_edit, 0));
        let editor_token = state.token();
        state.clear_draft();
        assert!(!state.save_editor(editor_token));
        state.cancel_editor();
        assert!(state.command_for_activation(before_edit, 0).is_none());
        let before_close = state.token();
        state.set_open(false);
        assert!(state.command_for_activation(state.token(), 0).is_none());
        state.set_open(true);
        assert!(state.command_for_activation(before_close, 0).is_none());
        let before_travel = state.token();
        state.open_editor(before_travel, 0);
        state.sync_context(1, false);
        assert!(state.editor.is_none());
        assert!(!state.open_editor(state.token(), 0));
        assert!(state.command_for_activation(state.token(), 0).is_none());
        state.sync_context(1, true);
        assert!(state.command_for_activation(before_travel, 0).is_none());
        assert_eq!(state.command_for_activation(state.token(), 0), Some("/loc"));
        let before_character = state.token();
        state.begin_session(Default::default());
        state.sync_context(1, true);
        assert!(state.command_for_activation(before_character, 0).is_none());
        assert!(state.bindings().iter().all(Option::is_none));
        assert!(!state.open_editor(state.token(), 12));
    }

    #[test]
    fn activation_uses_live_context_and_existing_gameplay_guards() {
        let (mut live, mut wire) = command_world(1, 1.);
        let mut interaction = Interaction {
            hotbuttons: configured("/attack on"),
            ..Default::default()
        };
        assert!(!interaction.activate_hotbutton(
            interaction.hotbuttons.token(),
            0,
            &mut live,
            [0.; 3]
        ));
        assert!(wire.try_recv().is_err());
        assert!(
            live.game
                .chat
                .back()
                .unwrap()
                .text
                .contains("living target")
        );
        live.target = Some(2);
        assert!(!interaction.activate_hotbutton(
            interaction.hotbuttons.token(),
            0,
            &mut live,
            [0.; 3]
        ));
        assert!(matches!(
            wire.try_recv().unwrap(),
            NetworkCommand::Gameplay(Command::AutoAttack(true), _)
        ));
        assert!(wire.try_recv().is_err());
        for command in ["/cast 1", "/reply hello", "/useitem"] {
            interaction.hotbuttons = configured(command);
            let notices = live.game.chat.len();
            assert!(!interaction.activate_hotbutton(
                interaction.hotbuttons.token(),
                0,
                &mut live,
                [0.; 3]
            ));
            assert!(
                wire.try_recv().is_err(),
                "{command} bypassed its normal prerequisites"
            );
            assert!(live.game.chat.len() > notices);
        }
        interaction.hotbuttons = configured("/reply hello");
        for recipient in ["First", "Second"] {
            live.game.last_tell = Some(recipient.into());
            interaction.activate_hotbutton(interaction.hotbuttons.token(), 0, &mut live, [0.; 3]);
            assert!(
                matches!(wire.try_recv().unwrap(), NetworkCommand::Gameplay(Command::Chat { channel: ChatChannel::Tell, target, text, .. }, _) if target == recipient && text == "hello")
            );
        }
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn quit_aliases_keep_exit_result_and_local_buttons_never_send_packets() {
        let (mut live, mut wire) = command_world(1, 1.);
        let mut interaction = Interaction::default();
        for command in ["/quit", "/camp"] {
            interaction.hotbuttons = configured(command);
            assert!(interaction.activate_hotbutton(
                interaction.hotbuttons.token(),
                0,
                &mut live,
                [0.; 3]
            ));
        }
        interaction.hotbuttons = configured("/skills");
        assert!(!interaction.skills_window.open);
        interaction.activate_hotbutton(interaction.hotbuttons.token(), 0, &mut live, [0.; 3]);
        assert!(interaction.skills_window.open);
        let old = interaction.hotbuttons.token();
        live.ready = false;
        interaction.activate_hotbutton(old, 0, &mut live, [0.; 3]);
        assert!(interaction.skills_window.open);
        live.ready = true;
        interaction.activate_hotbutton(old, 0, &mut live, [0.; 3]);
        assert!(
            interaction.skills_window.open,
            "a click cannot cross disconnect/reconnect"
        );
        assert!(wire.try_recv().is_err());
    }

    #[test]
    fn modal_editor_suppresses_inspected_item_overlay_without_losing_item_context() {
        let (mut live, mut wire) = command_world(1, 1.);
        let slot = openeq_net::inventory::InventorySlot::possessions(23);
        let item = crate::live::tests::carried_item(slot);
        let identity = (slot, item.id, item.instance_id);
        live.game.inventory.items.insert(slot, item);
        let mut interaction = Interaction {
            hotbuttons: configured("/useitem"),
            inspected_owned: Some(identity),
            ..Default::default()
        };
        assert!(interaction.view(&live).inspected_item.is_some());
        interaction
            .hotbuttons
            .open_editor(interaction.hotbuttons.token(), 0);
        let view = interaction.view(&live);
        assert!(view.hover_blocked && view.inspected_item.is_none());
        assert_eq!(interaction.inspected_owned, Some(identity));
        interaction.hotbuttons.cancel_editor();
        assert!(interaction.view(&live).inspected_item.is_some());
        assert!(wire.try_recv().is_err());
    }
}
