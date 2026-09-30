//! Pure saved command bindings. Validation never executes a command or retains
//! live targets, item identities, drafts, input ownership or session state.
use crate::chat::{self, Action, MAX_CHAT_BYTES};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const HOTBUTTON_COUNT: usize = 12;
pub const MAX_HOTBUTTON_LABEL_BYTES: usize = 64;
pub const MAX_HOTBUTTON_LABEL_CHARS: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedHotbutton {
    pub label: String,
    pub command: String,
}

pub type HotbuttonBindings = [Option<SavedHotbutton>; HOTBUTTON_COUNT];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotbuttonToken {
    pub revision: u64,
    pub zone_generation: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HotbuttonField {
    #[default]
    Label,
    Command,
}

/// Fixed diagnostics never echo potentially sensitive saved command contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotbuttonError {
    LabelControlCharacter,
    CommandControlCharacter,
    LabelTooLong,
    CommandTooLong,
    MissingLabel,
    MissingCommand,
    SlashRequired,
    InvalidCommand,
    ManagementCommand,
}

impl fmt::Display for HotbuttonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LabelControlCharacter => "Use a label without line breaks or control characters.",
            Self::CommandControlCharacter => {
                "Enter one command without line breaks or control characters."
            }
            Self::LabelTooLong => "Use a label of at most 32 characters and 64 bytes.",
            Self::CommandTooLong => "Use a command of at most 512 bytes.",
            Self::MissingLabel => "Enter a label, or clear both fields to remove the button.",
            Self::MissingCommand => "Enter a command, or clear both fields to remove the button.",
            Self::SlashRequired => "Start the command with a slash.",
            Self::InvalidCommand => "Use a supported command with its required arguments.",
            Self::ManagementCommand => "Hotbutton editor commands cannot be assigned to a button.",
        })
    }
}
impl std::error::Error for HotbuttonError {}

fn invalid_character(character: char) -> bool {
    character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
}

/// Both empty fields clear a slot. The returned definition is trimmed and
/// bounded; a half-filled slot is an error. Controls are rejected before trim.
pub fn validate_hotbutton(
    label: &str,
    command: &str,
) -> Result<Option<SavedHotbutton>, HotbuttonError> {
    if label.chars().any(invalid_character) {
        return Err(HotbuttonError::LabelControlCharacter);
    }
    if command.chars().any(invalid_character) {
        return Err(HotbuttonError::CommandControlCharacter);
    }
    let label = label.trim();
    let command = command.trim();
    if label.is_empty() && command.is_empty() {
        return Ok(None);
    }
    if label.is_empty() {
        return Err(HotbuttonError::MissingLabel);
    }
    if command.is_empty() {
        return Err(HotbuttonError::MissingCommand);
    }
    if label.len() > MAX_HOTBUTTON_LABEL_BYTES || label.chars().count() > MAX_HOTBUTTON_LABEL_CHARS
    {
        return Err(HotbuttonError::LabelTooLong);
    }
    parse_hotbutton_command(command)?;
    Ok(Some(SavedHotbutton {
        label: label.into(),
        command: command.into(),
    }))
}

/// Parse again on activation and dispatch the returned action through the
/// ordinary interaction path. Quit includes the existing immediate Camp alias.
pub fn parse_hotbutton_command(command: &str) -> Result<Action, HotbuttonError> {
    if command.chars().any(invalid_character) {
        return Err(HotbuttonError::CommandControlCharacter);
    }
    let command = command.trim();
    if command.is_empty() {
        return Err(HotbuttonError::MissingCommand);
    }
    if command.len() > MAX_CHAT_BYTES {
        return Err(HotbuttonError::CommandTooLong);
    }
    if !command.starts_with('/') {
        return Err(HotbuttonError::SlashRequired);
    }
    let action = chat::parse(command).map_err(|_| HotbuttonError::InvalidCommand)?;
    // Keep this exhaustive: new dispatcher actions need an explicit decision.
    match action {
        Action::Chat { .. }
        | Action::Reply(_)
        | Action::Emote(_)
        | Action::Attack(_)
        | Action::Sit(_)
        | Action::Hail
        | Action::Loot
        | Action::Consider
        | Action::Assist(_)
        | Action::Target(_)
        | Action::Inventory
        | Action::UseTarget
        | Action::Trade
        | Action::CancelTrade
        | Action::Scribe
        | Action::UseItem
        | Action::Merchant
        | Action::Bank
        | Action::Train
        | Action::Invite(_)
        | Action::AcceptInvite
        | Action::DeclineInvite
        | Action::LeaveGroup
        | Action::MakeLeader(_)
        | Action::Raid
        | Action::GuildWindow
        | Action::Skills
        | Action::RaidInvite(_)
        | Action::RaidAccept
        | Action::RaidDismiss
        | Action::RaidLeave
        | Action::RaidLeader(_)
        | Action::Spellbook
        | Action::Cast(_)
        | Action::StopCast
        | Action::Location
        | Action::Help
        | Action::Quit
        | Action::Camp => Ok(action),
        Action::Hotbuttons | Action::Hotbutton(_) => Err(HotbuttonError::ManagementCommand),
    }
}

/// Only bounded indices/counts leave the saved-field normalizer, never labels,
/// command strings or deserializer errors containing saved values.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HotbuttonLoadDiagnostics {
    pub discarded_slots: Vec<usize>,
    pub unexpected_count: Option<usize>,
    pub invalid_shape: bool,
}
impl HotbuttonLoadDiagnostics {
    pub fn is_empty(&self) -> bool {
        self.discarded_slots.is_empty() && self.unexpected_count.is_none() && !self.invalid_shape
    }
}

/// The caller enforces the whole document byte ceiling before JSON decoding.
/// In memory and on save there are always twelve slots. Recovery never compacts
/// neighboring entries or executes commands. Missing fields are legacy defaults.
pub(crate) fn decode_saved_hotbuttons(
    value: Option<serde_json::Value>,
) -> (HotbuttonBindings, HotbuttonLoadDiagnostics) {
    let mut bindings = HotbuttonBindings::default();
    let mut diagnostics = HotbuttonLoadDiagnostics::default();
    let Some(value) = value else {
        return (bindings, diagnostics);
    };
    let serde_json::Value::Array(entries) = value else {
        diagnostics.invalid_shape = true;
        return (bindings, diagnostics);
    };
    if entries.len() != HOTBUTTON_COUNT {
        diagnostics.unexpected_count = Some(entries.len());
    }
    for (index, entry) in entries.into_iter().take(HOTBUTTON_COUNT).enumerate() {
        if entry.is_null() {
            continue;
        }
        let validated = serde_json::from_value::<SavedHotbutton>(entry)
            .ok()
            .and_then(|button| validate_hotbutton(&button.label, &button.command).ok());
        match validated {
            Some(button) => bindings[index] = button,
            None => diagnostics.discarded_slots.push(index + 1),
        }
    }
    (bindings, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_command_aliases_retain_dispatcher_semantics() {
        for command in [
            "/say hello",
            "/s hello",
            "/group hello",
            "/g hello",
            "/gsay hello",
            "/guild hello",
            "/gu hello",
            "/rsay hello",
            "/rs hello",
            "/ooc hello",
            "/shout hello",
            "/sh hello",
            "/auction hello",
            "/auc hello",
            "/tell Friend hello",
            "/t Friend hello",
            "/reply hello",
            "/r hello",
            "/emote waves",
            "/em waves",
            "/me waves",
            "/attack",
            "/attack on",
            "/attack off",
            "/sit",
            "/stand",
            "/hail",
            "/loot",
            "/con",
            "/consider",
            "/assist",
            "/assist Friend",
            "/target A wandering guard",
            "/tar clear",
            "/inventory",
            "/inv",
            "/trade",
            "/canceltrade",
            "/scribe",
            "/useitem",
            "/use",
            "/merchant",
            "/bank",
            "/invite",
            "/invite Friend",
            "/accept",
            "/acceptinvite",
            "/decline",
            "/declineinvite",
            "/leavegroup",
            "/disband",
            "/makeleader Friend",
            "/raid",
            "/raidinvite",
            "/raidinvite Friend",
            "/raidaccept",
            "/raiddecline",
            "/raidleave",
            "/raidleader Friend",
            "/guildwindow",
            "/skills",
            "/book",
            "/spellbook",
            "/cast 1",
            "/cast 12",
            "/stopcast",
            "/loc",
            "/help",
            "/quit",
            "/camp",
        ] {
            assert_eq!(
                parse_hotbutton_command(command).unwrap(),
                chat::parse(command).unwrap()
            );
        }
        assert_eq!(parse_hotbutton_command(" /QuIt "), Ok(Action::Quit));
        assert_eq!(parse_hotbutton_command("/camp"), Ok(Action::Camp));
        assert_eq!(parse_hotbutton_command("/sit"), Ok(Action::Sit(true)));
        assert_eq!(parse_hotbutton_command("/stand"), Ok(Action::Sit(false)));
        assert!(matches!(
            parse_hotbutton_command("/say literal; /quit"),
            Ok(Action::Chat { text, .. }) if text == "literal; /quit"
        ));
    }

    #[test]
    fn unsupported_management_and_malformed_commands_never_become_bindings() {
        for command in [
            "",
            "hello",
            "/",
            "/unknown-private-text",
            "/cast 0",
            "/cast 13",
            "/tell Friend",
            "/say",
            "/attack maybe",
            "/target",
            "/makeleader",
            "/raidleader",
            "/audio mute",
            "/pause 10",
            "/map",
            "/sit;/quit",
            "/hotbuttons",
            "/hotbutton 1",
            "/hotbutton 12",
        ] {
            assert!(parse_hotbutton_command(command).is_err());
            assert!(validate_hotbutton("Action", command).is_err());
        }
        assert_eq!(
            parse_hotbutton_command("/hotbuttons"),
            Err(HotbuttonError::ManagementCommand)
        );
        let error = parse_hotbutton_command("/unknown-private-text").unwrap_err();
        assert!(!error.to_string().contains("unknown-private-text"));
        assert!(!format!("{error:?}").contains("unknown-private-text"));
    }

    #[test]
    fn validation_trims_bounds_utf8_and_distinguishes_clear_from_partial_drafts() {
        assert_eq!(validate_hotbutton(" ", " "), Ok(None));
        assert_eq!(
            validate_hotbutton("", "/sit"),
            Err(HotbuttonError::MissingLabel)
        );
        assert_eq!(
            validate_hotbutton("Sit", ""),
            Err(HotbuttonError::MissingCommand)
        );
        assert_eq!(
            validate_hotbutton(" Sit ", " /SiT ").unwrap(),
            Some(SavedHotbutton {
                label: "Sit".into(),
                command: "/SiT".into(),
            })
        );
        for label in ["x".repeat(32), "é".repeat(32), "界".repeat(21)] {
            assert!(validate_hotbutton(&label, "/sit").is_ok());
        }
        for label in ["x".repeat(33), "界".repeat(22)] {
            assert_eq!(
                validate_hotbutton(&label, "/sit"),
                Err(HotbuttonError::LabelTooLong)
            );
        }
        let boundary = format!("/say {}", "界".repeat(169));
        assert_eq!(boundary.len(), MAX_CHAT_BYTES);
        assert!(validate_hotbutton("Say", &boundary).is_ok());
        assert_eq!(
            validate_hotbutton("Say", &(boundary + "x")),
            Err(HotbuttonError::CommandTooLong)
        );
        for control in ['\n', '\r', '\t', '\0', '\u{0085}', '\u{2028}', '\u{2029}'] {
            assert_eq!(
                validate_hotbutton(&format!("{control}Sit"), "/sit"),
                Err(HotbuttonError::LabelControlCharacter)
            );
            assert_eq!(
                validate_hotbutton("Sit", &format!("/sit{control}")),
                Err(HotbuttonError::CommandControlCharacter)
            );
            assert_eq!(
                parse_hotbutton_command(&format!("{control}/sit")),
                Err(HotbuttonError::CommandControlCharacter)
            );
        }
    }

    #[test]
    fn field_recovery_preserves_indices_and_reports_only_bounded_metadata() {
        use serde_json::json;
        let (bindings, warnings) = decode_saved_hotbuttons(Some(json!([
            {"label":" Sit ","command":" /sit "},
            {"label":"private label","command":"/unsupported-private-content"},
            null,
            123,
            {"label":"Camp","command":"/camp"},
            {"label":" ","command":" "},
            {"label":"Missing command"}
        ])));
        assert_eq!(bindings[0], validate_hotbutton("Sit", "/sit").unwrap());
        assert_eq!(bindings[4], validate_hotbutton("Camp", "/camp").unwrap());
        assert!(
            bindings
                .iter()
                .enumerate()
                .all(|(i, value)| [0, 4].contains(&i) || value.is_none())
        );
        assert_eq!(warnings.discarded_slots, vec![2, 4, 7]);
        assert_eq!(warnings.unexpected_count, Some(7));
        assert!(!warnings.invalid_shape);
        assert!(!format!("{warnings:?}").contains("private"));
        assert_eq!(
            decode_saved_hotbuttons(None),
            (
                HotbuttonBindings::default(),
                HotbuttonLoadDiagnostics::default()
            )
        );
        for value in [json!(null), json!("bad"), json!({"slot": 0})] {
            let (bindings, warnings) = decode_saved_hotbuttons(Some(value));
            assert!(bindings.iter().all(Option::is_none));
            assert!(warnings.invalid_shape);
        }
        let mut entries = vec![json!(null); HOTBUTTON_COUNT + 3];
        entries[11] = json!({"label":"Last","command":"/loc"});
        entries[12] = json!({"label":"Overflow","command":"/quit"});
        let (bindings, warnings) = decode_saved_hotbuttons(Some(json!(entries)));
        assert_eq!(bindings[11], validate_hotbutton("Last", "/loc").unwrap());
        assert_eq!(bindings.iter().filter(|entry| entry.is_some()).count(), 1);
        assert_eq!(warnings.unexpected_count, Some(15));
    }
}
