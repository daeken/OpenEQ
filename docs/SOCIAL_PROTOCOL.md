# Groups and quest links

`openeq-net::social` implements the RoF2 group wire messages. Commands are wrapped
in `gameplay::Command::Social`; received events are
`gameplay::GameplayEvent::Social`. Sending an invite or accept does not create a
local group. The client applies the server's roster, join, leave and leader
events. It can resolve member spawn IDs, health and actual level by name when
the member is present in the zone.

| Message | RoF2 opcode | Layout |
| --- | --- | --- |
| Invite / alternate invite | `6110` / `32c2` | 148 bytes; invitee then inviter, 64-byte names |
| Accept / accepted notice | `1649` / `2060` | 152 bytes; inviter then invitee |
| Decline / cancellation | `2a50` | 152 bytes; inviter, invitee, unused fields and toggle |
| Full roster | `6194` | Group ID, count, leader string, variable member records |
| Member joins | `3abb` | 148 bytes; owner, member name, mercenary flag and metadata |
| Your group disbands / another member leaves | `1ae5` / `74da` | 148 bytes; recipient and departed member |
| Leader changes | `21b4` | 148 bytes; leader name at byte 64 |
| Join acknowledged | `7323` | 4 bytes |
| Leave request | `4c10` | 148 bytes; local character in both name fields |
| Make leader request | `4229` | 456 bytes; current and new leader at bytes 4 and 68 |
| Item/quest link activation | `4cef` | 52 bytes; item, six augments, hash and icon |

Names are bounded and terminated. Rosters contain at most six unique names and
indices; malformed or trailing packet data produces an error, never an empty
roster. Incremental join records have no authoritative slot index. Roster
levels are deliberately omitted: EQEmu's RoF2 encoder inserts the constants 70
and 65 in those fields. Member levels must come from live spawn data.

Initial group creation can send a one-member roster with an empty leader,
followed by a separate leader-change event. During zone entry, EQEmu restores
database group membership and sends a complete roster; clients should preserve
membership through the handoff until authoritative events arrive. A full
disband can have an empty departed-member name. A leave request must first
target the local player, because EQEmu gives the current target precedence over
the packet's names and a leader could otherwise remove the targeted member.

The shipped EQEmu RoF2 opcode file contains a stale `OP_CancelInvite=0x2a50`
entry and `OP_GroupCancelInvite=0x0000`, although only `OP_GroupCancelInvite` is
registered. The development server maps `OP_GroupCancelInvite=0x2a50`; its prior
file is saved at `/srv/eqemu/private/patch_RoF2.pre-social.conf`. Reload opcodes
after updating that mapping. Decline is relayed to the inviter only, so the
invitee dismisses the invitation locally after the click without changing a
roster.

Chat links are parsed by `chat_links::parse_chat` before any plain-text cleanup.
The returned text includes readable `[labels]`; each link records a UTF-8 byte
range for its label and the numeric `LinkPayload`. Both `0x12` delimiters, all
56 hexadecimal descriptor bytes and a printable label of 1–256 bytes are
required for a clickable link. Broken links remain noninteractive text.

Quest links use item ID `0xFFFFF`. Augment 1 stores an ordinary phrase ID;
augment 2 stores a silent phrase ID and takes precedence when nonzero. Clicking
sends the received ID, augments, hash and icon through `OP_ItemLinkClick`. The
server looks up the phrase and dispatches the quest event. Display labels may
be completely different from the phrase and are never sent as `/say`, parsed
as commands, or run as scripts. Ordinary item links use the same request to
ask the server for an item inspection.

Verification:

```sh
cargo test -p openeq-net social::tests
cargo test -p openeq --lib chat_links
cargo run -p openeq --bin social_smoke -- \
  "$HOME/.config/openeq/storage2-social1-credentials.json" \
  "$HOME/.config/openeq/storage2-social2-credentials.json"
```

The live probe is restricted to the dedicated Fellowship and Companion
characters. It exercises invitation, decline, a two-client roster, group chat,
leader transfer and leave, then hails Aid Eino and activates his received
`help` quest link. The latter advances only the fixture's Eino quest global.
The probe uses the client's actual `GroupState` reducer, waits for the server's
asynchronous opcode-reload confirmation, and uses a fixture-only GM `#goto` to
reach the NPC without relying on a large client movement jump.

The normal workspace probe passed against Storage2 on 2026-09-28. Both clients
received the same roster and group chat, both confirmed the leader change and
disband, and Aid Eino's received phrase ID 23 produced “Then meet me this night
in the Plane of Nightmares” after activation. The runtime log is
`/tmp/openeq-social-proof-workspace.log`; phrase IDs belong to that database and
are never hardcoded by the client. Five protocol tests and three chat-link tests
cover fixed layouts, truncation, invalid counts/names, Unicode label ranges,
malformed descriptors and label/payload separation.

The native client also verified the rendered link: clicking `help` in Aid Eino's
wrapped chat response produced his Plane of Nightmares instructions. The click
went through Retina-scaled glyph hit regions, UI dispatch and the stored numeric
payload. The native fixture was logged out afterward.

Credentials stay outside the repository. Guilds, raids, mercenary management,
group-role changes and leadership abilities are outside this implementation.
