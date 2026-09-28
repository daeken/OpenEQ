//! Visible chat labels and their numeric RoF2 item/quest link descriptors.
//! Parsing only creates data; activation happens through an explicit UI click.
use openeq_net::{
    gameplay::Command,
    social::{LINK_BODY_BYTES, LinkPayload, SocialCommand},
};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatLink {
    /// UTF-8 byte range of the visible label in `ParsedChat::text`.
    pub range: Range<usize>,
    pub payload: LinkPayload,
}
impl ChatLink {
    pub fn activation(&self) -> Command {
        Command::Social(SocialCommand::ActivateLink(self.payload.clone()))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedChat {
    pub text: String,
    pub links: Vec<ChatLink>,
}

fn visible(output: &mut String, raw: &str) {
    output.extend(raw.chars().filter(|ch| *ch == '\n' || !ch.is_control()));
}

/// Requires paired 0x12 delimiters, exactly 56 hexadecimal descriptor bytes,
/// and a nonempty printable label of at most 256 bytes. Malformed links remain
/// noninteractive text. The label is never interpreted as a command or phrase.
pub fn parse_chat(raw: &str) -> ParsedChat {
    let mut result = ParsedChat::default();
    let mut rest = raw;
    while let Some(open) = rest.find('\u{12}') {
        visible(&mut result.text, &rest[..open]);
        rest = &rest[open + 1..];
        let Some(close) = rest.find('\u{12}') else {
            visible(&mut result.text, rest);
            return result;
        };
        let token = &rest[..close];
        rest = &rest[close + 1..];
        let descriptor = token
            .get(..LINK_BODY_BYTES)
            .filter(|body| body.bytes().all(|b| b.is_ascii_hexdigit()));
        if let Some(body) = descriptor {
            let label = &token[LINK_BODY_BYTES..];
            result.text.push('[');
            let start = result.text.len();
            visible(&mut result.text, label);
            let end = result.text.len();
            result.text.push(']');
            if !label.is_empty()
                && label.len() <= 256
                && !label.chars().any(char::is_control)
                && result.links.len() < 64
                && let Some(payload) = LinkPayload::parse_body(body)
            {
                result.links.push(ChatLink {
                    range: start..end,
                    payload,
                });
            }
        } else {
            visible(&mut result.text, token);
        }
    }
    visible(&mut result.text, rest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use openeq_net::social::LinkKind;

    fn quest(label: &str, silent: bool) -> String {
        let body = format!(
            "0FFFFF{:05X}{:05X}{}12345678",
            if silent { 0 } else { 123 },
            if silent { 456 } else { 0 },
            "0".repeat(32)
        );
        format!("\u{12}{body}{label}\u{12}")
    }
    #[test]
    fn unicode_visible_labels_keep_exact_numeric_activation() {
        let parsed = parse_chat(&format!(
            "NPC says: {} or {}!",
            quest("héllo", false),
            quest("different display", true)
        ));
        assert_eq!(parsed.text, "NPC says: [héllo] or [different display]!");
        assert_eq!(parsed.links.len(), 2);
        assert_eq!(&parsed.text[parsed.links[0].range.clone()], "héllo");
        assert_eq!(
            &parsed.text[parsed.links[1].range.clone()],
            "different display"
        );
        assert_eq!(
            parsed.links[0].payload.kind(),
            LinkKind::Quest {
                phrase_id: 123,
                silent: false
            }
        );
        let Command::Social(SocialCommand::ActivateLink(link)) = parsed.links[1].activation()
        else {
            panic!()
        };
        assert_eq!(
            link.kind(),
            LinkKind::Quest {
                phrase_id: 456,
                silent: true
            }
        );
    }
    #[test]
    fn malformed_and_unterminated_tokens_have_no_click_action() {
        for raw in [
            quest("", false),
            quest("bad\nlabel", false),
            quest(&"x".repeat(257), false),
            format!("\u{12}{}label\u{12}", "é".repeat(28)),
            format!("\u{12}{}label\u{12}", "0".repeat(56)),
            quest("no closing delimiter", false)
                .trim_end_matches('\u{12}')
                .to_owned(),
        ] {
            let parsed = parse_chat(&raw);
            assert!(parsed.links.is_empty(), "accepted {raw:?}");
            assert!(!parsed.text.contains('\u{12}'));
        }
        let mut raw = quest("valid", false);
        raw.replace_range(2..3, "z");
        assert!(parse_chat(&raw).links.is_empty());
    }
    #[test]
    fn plain_chat_is_preserved_without_interpreting_label_contents() {
        assert_eq!(
            parse_chat("Hello\0 world\t!\nNext").text,
            "Hello world!\nNext"
        );
        let parsed = parse_chat(&quest("/say this is only a label", false));
        let packet = openeq_net::social::encode_command(SocialCommand::ActivateLink(
            parsed.links[0].payload.clone(),
        ))
        .unwrap();
        assert_eq!(packet.data.len(), 52);
        assert!(!packet.data.windows(4).any(|w| w == b"/say"));
    }
}
