//! The installed client's type-6 dbstr strings are explanatory templates, not
//! formulas. Unknown substitutions stay explicitly unavailable.
use std::collections::BTreeMap;

const MAX_DESCRIPTION_BYTES: usize = 16_384;

pub(super) fn parse(text: &str) -> BTreeMap<u32, String> {
    let mut descriptions = BTreeMap::new();
    for line in text.lines() {
        if line.len() > MAX_DESCRIPTION_BYTES + 64 {
            continue;
        }
        let mut fields = line.trim_end_matches('\r').split('^');
        let (Some(id), Some("6"), Some(text), Some("0"), Some(""), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            continue;
        };
        let Ok(id) = id.parse::<u32>() else { continue };
        if id != 0 && text.len() <= MAX_DESCRIPTION_BYTES && !text.trim().is_empty() {
            descriptions.insert(id, readable(text));
        }
    }
    descriptions
}

fn readable(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    let mut unknown = false;
    while let Some((offset, ch)) = chars.next() {
        if ch == '<' {
            let remaining = &text[offset..];
            if let Some(tag) = ["<br>", "<br/>", "<br />"].into_iter().find(|tag| {
                remaining
                    .get(..tag.len())
                    .is_some_and(|s| s.eq_ignore_ascii_case(tag))
            }) {
                while chars
                    .peek()
                    .is_some_and(|(index, _)| *index < offset + tag.len())
                {
                    chars.next();
                }
                result.push('\n');
                continue;
            }
        }
        if matches!(ch, '#' | '@' | '$' | '%')
            && chars
                .peek()
                .is_some_and(|(_, next)| next.is_ascii_alphanumeric())
        {
            while chars
                .peek()
                .is_some_and(|(_, next)| next.is_ascii_alphanumeric())
            {
                chars.next();
            }
            result.push('?');
            unknown = true;
        } else if ch == '\n' || ch == '\t' || !ch.is_control() {
            result.push(ch);
        }
    }
    if unknown {
        result.push_str("\n\nSome effect values are unavailable.");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_requires_type_and_valid_complete_record() {
        let parsed = parse(
            "200^6^Healing.^0^\n200^7^Wrong type.^0^\r\n201^6^Missing suffix\n202^6^Extra^field^0^\n0^6^No description ID.^0^\n203^6^^0^",
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[&200], "Healing.");
    }

    #[test]
    fn substitutions_are_atomic_and_never_guessed() {
        let text = readable(
            "Between #1 and @12. #12#2 $7 %z %H %L %l %1 #x #unknown. #6% chance, 50% remaining.",
        );
        assert_eq!(
            text,
            "Between ? and ?. ?? ? ? ? ? ? ? ? ?. ?% chance, 50% remaining.\n\nSome effect values are unavailable."
        );
    }

    #[test]
    fn breaks_unicode_and_untrusted_markup_stay_plain_text() {
        assert_eq!(
            readable("Café—雪<BR><br/><br />Next <a href='run'>text</a>\0"),
            "Café—雪\n\n\nNext <a href='run'>text</a>"
        );
        assert_eq!(readable(""), "");
        let bytes = b"200^6^Mends \x93minor\x94 wounds.\x85^0^";
        let decoded = encoding_rs::WINDOWS_1252.decode(bytes).0;
        assert_eq!(parse(&decoded)[&200], "Mends “minor” wounds.…");
    }

    #[test]
    fn oversized_records_are_skipped() {
        assert!(
            parse(&format!(
                "200^6^{}^0^",
                "x".repeat(MAX_DESCRIPTION_BYTES + 1)
            ))
            .is_empty()
        );
    }
}
