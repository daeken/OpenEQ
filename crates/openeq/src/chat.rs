//! Text editing and command parsing independent of the window and network.
use std::collections::VecDeque;

pub const MAX_CHAT_BYTES: usize = 512;
const HISTORY_LIMIT: usize = 100;

#[derive(Default)]
pub struct ChatEditor {
    pub active: bool,
    pub text: String,
    /// Byte offset, always on a UTF-8 boundary.
    pub cursor: usize,
    pub preedit: String,
    history: VecDeque<String>,
    history_index: Option<usize>,
    draft: String,
}

impl ChatEditor {
    pub fn open(&mut self, prefix: &str) {
        self.active = true;
        self.text = prefix.to_owned();
        self.cursor = self.text.len();
        self.preedit.clear();
        self.history_index = None;
        self.draft.clear();
    }

    pub fn cancel(&mut self) {
        self.active = false;
        self.preedit.clear();
    }

    pub fn insert(&mut self, text: &str) {
        for ch in text.chars().filter(|ch| !ch.is_control()) {
            if self.text.len() + ch.len_utf8() > MAX_CHAT_BYTES {
                break;
            }
            self.text.insert(self.cursor, ch);
            self.cursor += ch.len_utf8();
        }
        self.history_index = None;
    }

    pub fn left(&mut self) {
        self.cursor = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
    }

    pub fn right(&mut self) {
        if let Some(ch) = self.text[self.cursor..].chars().next() {
            self.cursor += ch.len_utf8();
        }
    }

    pub fn backspace(&mut self, word: bool) {
        let end = self.cursor;
        if word {
            while self.cursor > 0
                && self.text[..self.cursor]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
            {
                self.left();
            }
            while self.cursor > 0
                && self.text[..self.cursor]
                    .chars()
                    .next_back()
                    .is_some_and(|c| !c.is_whitespace())
            {
                self.left();
            }
        } else {
            self.left();
        }
        self.text.drain(self.cursor..end);
        self.history_index = None;
    }

    pub fn delete(&mut self) {
        let start = self.cursor;
        self.right();
        self.text.drain(start..self.cursor);
        self.cursor = start;
        self.history_index = None;
    }

    pub fn history(&mut self, older: bool) {
        if self.history.is_empty() || (!older && self.history_index.is_none()) {
            return;
        }
        if older {
            self.history_index = Some(match self.history_index {
                Some(i) => i.saturating_sub(1),
                None => {
                    self.draft.clone_from(&self.text);
                    self.history.len() - 1
                }
            });
        } else {
            self.history_index = self
                .history_index
                .and_then(|i| (i + 1 < self.history.len()).then_some(i + 1));
        }
        self.text = self
            .history_index
            .map_or_else(|| self.draft.clone(), |i| self.history[i].clone());
        self.cursor = self.text.len();
    }

    pub fn submit(&mut self) -> Option<String> {
        self.active = false;
        self.preedit.clear();
        let line = self.text.trim().to_owned();
        self.text.clear();
        self.cursor = 0;
        self.history_index = None;
        if line.is_empty() {
            return None;
        }
        if self.history.back() != Some(&line) {
            self.history.push_back(line.clone());
            if self.history.len() > HISTORY_LIMIT {
                self.history.pop_front();
            }
        }
        Some(line)
    }

    pub fn display(&self) -> String {
        if !self.active {
            return String::new();
        }
        format!(
            "{}{}│{}",
            &self.text[..self.cursor],
            self.preedit,
            &self.text[self.cursor..]
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Chat {
        channel: u32,
        recipient: String,
        text: String,
    },
    Reply(String),
    Emote(String),
    Attack(Option<bool>),
    Sit(bool),
    Hail,
    Loot,
    Consider,
    Assist(Option<String>),
    Target(String),
    Inventory,
    UseTarget,
    Trade,
    CancelTrade,
    Scribe,
    UseItem,
    Merchant,
    Bank,
    Invite(Option<String>),
    AcceptInvite,
    DeclineInvite,
    LeaveGroup,
    MakeLeader(String),
    Spellbook,
    Cast(u8),
    StopCast,
    Location,
    Help,
    Quit,
}

pub const HELP: &str = "Enter: chat • /say /tell NAME /reply /group /guild /ooc /shout /auction /emote\n/attack [on|off] /sit /stand /hail /con /assist [NAME] /target NAME /loot /inventory /cast 1–12 /book /stopcast /loc /quit\n/trade /canceltrade /scribe /useitem (inspected item)\n/use /merchant /bank /invite [NAME] /accept /decline /leavegroup /makeleader NAME\nI inventory • Q attack • H hail • X sit/stand • L loot • C consider • V assist • Tab target • B spellbook • Alt+1–0 spell gems • M map • F9 camera • E door • R NPC service";

pub fn parse(line: &str) -> Result<Action, String> {
    let line = line.trim();
    if line.is_empty() {
        return Err("Enter a message or command.".into());
    }
    if !line.starts_with('/') {
        return Ok(chat(8, "", line));
    }
    let (command, rest) = line[1..]
        .split_once(char::is_whitespace)
        .unwrap_or((&line[1..], ""));
    let rest = rest.trim();
    let message = |channel| {
        if rest.is_empty() {
            Err(format!("/{command} needs a message."))
        } else {
            Ok(chat(channel, "", rest))
        }
    };
    Ok(match command.to_ascii_lowercase().as_str() {
        "say" | "s" => return message(8),
        "group" | "g" | "gsay" => return message(2),
        "guild" | "gu" => return message(0),
        "ooc" => return message(5),
        "shout" | "sh" => return message(3),
        "auction" | "auc" => return message(4),
        "emote" | "em" | "me" => {
            if rest.is_empty() {
                return Err("Usage: /emote MESSAGE".into());
            }
            Action::Emote(rest.into())
        }
        "tell" | "t" => {
            let (recipient, text) = rest
                .split_once(char::is_whitespace)
                .ok_or("Usage: /tell NAME MESSAGE")?;
            if recipient.is_empty() || text.trim().is_empty() {
                return Err("Usage: /tell NAME MESSAGE".into());
            }
            chat(7, recipient, text.trim())
        }
        "reply" | "r" => {
            if rest.is_empty() {
                return Err("Usage: /reply MESSAGE".into());
            }
            Action::Reply(rest.into())
        }
        "attack" => Action::Attack(match rest.to_ascii_lowercase().as_str() {
            "" => None,
            "on" => Some(true),
            "off" => Some(false),
            _ => return Err("Usage: /attack [on|off]".into()),
        }),
        "sit" => Action::Sit(true),
        "stand" => Action::Sit(false),
        "hail" => Action::Hail,
        "loot" => Action::Loot,
        "con" | "consider" => Action::Consider,
        "assist" => Action::Assist((!rest.is_empty()).then(|| rest.to_owned())),
        "target" | "tar" => {
            if rest.is_empty() {
                return Err("Usage: /target NAME (or /target clear)".into());
            }
            Action::Target(rest.into())
        }
        "inventory" | "inv" => Action::Inventory,
        "trade" => Action::Trade,
        "canceltrade" => Action::CancelTrade,
        "scribe" => Action::Scribe,
        "useitem" => Action::UseItem,
        "use" => Action::UseTarget,
        "merchant" => Action::Merchant,
        "bank" => Action::Bank,
        "invite" => Action::Invite((!rest.is_empty()).then(|| rest.to_owned())),
        "accept" | "acceptinvite" => Action::AcceptInvite,
        "decline" | "declineinvite" => Action::DeclineInvite,
        "leavegroup" | "disband" => Action::LeaveGroup,
        "makeleader" => {
            if rest.is_empty() {
                return Err("Usage: /makeleader NAME".into());
            }
            Action::MakeLeader(rest.into())
        }
        "book" | "spellbook" => Action::Spellbook,
        "stopcast" => Action::StopCast,
        "cast" => Action::Cast(
            rest.parse::<u8>()
                .ok()
                .filter(|gem| (1..=12).contains(gem))
                .ok_or("Usage: /cast GEM (1–12)")?
                - 1,
        ),
        "loc" => Action::Location,
        "help" => Action::Help,
        "quit" | "camp" => Action::Quit,
        _ => {
            return Err(format!(
                "Unknown command /{command}. Type /help for supported commands."
            ));
        }
    })
}

fn chat(channel: u32, recipient: &str, text: &str) -> Action {
    Action::Chat {
        channel,
        recipient: recipient.into(),
        text: text.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_in_the_middle_of_multibyte_text() {
        let mut e = ChatEditor::default();
        e.open("");
        e.insert("hé猫!");
        e.left();
        e.backspace(false);
        assert_eq!(e.text, "hé!");
        e.insert("界");
        e.left();
        e.delete();
        assert_eq!(e.text, "hé!");
        e.left();
        e.backspace(false);
        assert_eq!(e.text, "é!");
        assert!(e.text.is_char_boundary(e.cursor));
    }

    #[test]
    fn history_preserves_drafts_and_deduplicates() {
        let mut e = ChatEditor::default();
        for text in ["first", "second", "second"] {
            e.open(text);
            e.submit();
        }
        e.open("draft");
        e.history(true);
        assert_eq!(e.text, "second");
        e.history(true);
        assert_eq!(e.text, "first");
        e.history(true);
        assert_eq!(e.text, "first");
        e.history(false);
        assert_eq!(e.text, "second");
        e.history(false);
        assert_eq!(e.text, "draft");
    }

    #[test]
    fn input_is_bounded_and_control_characters_are_removed() {
        let mut e = ChatEditor::default();
        e.open("");
        e.insert("a\n\r\0b");
        assert_eq!(e.text, "ab");
        e.insert(&"界".repeat(1000));
        assert!(e.text.len() <= MAX_CHAT_BYTES);
        assert!(e.text.is_char_boundary(e.cursor));
    }

    #[test]
    fn word_deletion_and_submission() {
        let mut e = ChatEditor::default();
        e.open("hello world  ");
        e.backspace(true);
        assert_eq!(e.text, "hello ");
        assert_eq!(e.submit().as_deref(), Some("hello"));
        assert!(!e.active);
    }

    #[test]
    fn chat_commands_preserve_recipient_and_text() {
        assert_eq!(parse("  hello  ").unwrap(), chat(8, "", "hello"));
        assert_eq!(
            parse("/TeLl Explorer hello there").unwrap(),
            chat(7, "Explorer", "hello there")
        );
        assert_eq!(
            parse("/gu guild message").unwrap(),
            chat(0, "", "guild message")
        );
        assert_eq!(parse("/em waves").unwrap(), Action::Emote("waves".into()));
        assert!(parse("/tell Explorer").is_err());
        assert!(parse("/say").is_err());
        assert!(parse("/attack maybe").is_err());
        assert_eq!(parse("/cast 12").unwrap(), Action::Cast(11));
        assert!(parse("/cast 0").is_err());
        assert!(parse("/cast 13").is_err());
        assert_eq!(parse("/stopcast").unwrap(), Action::StopCast);
        assert!(parse("/unsupported").is_err());
        assert_eq!(parse("/attack off").unwrap(), Action::Attack(Some(false)));
        assert_eq!(
            parse("/target A wandering guard").unwrap(),
            Action::Target("A wandering guard".into())
        );
    }
}
