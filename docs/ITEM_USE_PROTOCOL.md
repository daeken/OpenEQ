# Scroll scribing and item casting

`openeq-net::item_use` provides `ItemUseCommand::{Scribe, Click}` and
`ItemUseEvent::{Verified, Recast}`, wrapped by the ordinary gameplay command and
event enums. Spellbook, cast bar, inventory, buffs and interruption notifications
continue to use the existing gameplay events. A click acknowledgement does not
mean the item was usable or its effect succeeded.

| Message | RoF2 opcode | Layout |
| --- | --- | --- |
| Scribe scroll | `217c` | Four little-endian u32: book slot, spell ID, action `0`, reduction `0` |
| Activate inventory item | `189c` | 12-byte typed inventory slot, u32 target spawn ID |
| Item verification reply | `097b` | 12-byte typed slot, u32 alternate spell, u32 target |
| Shared item reuse timer | `15a9` | Packed 9 bytes: u32 remaining seconds, i32 timer group, bool override |

Scribing requires the actual matching scroll on the cursor. The client checks
the spell's class/level requirement, rejects an already known spell and chooses
an empty slot within the 720-entry RoF2 spellbook. The server's
`Client::OPMemorizeSpell` validates the spell's class/level and compares its ID
with the cursor item's scroll effect. With `Character:RestrictSpellScribing`
enabled it also checks the scroll's race/class restrictions. It does not protect
an occupied book slot or reject an already known spell on the client's behalf.

For a carried scroll, first move one unit of the identified item to an empty cursor, then
send the scribe request on the same ordered stream. Ordinary successful
inventory moves have no acknowledgement, so waiting for a move echo would
stall. Keep the operation pending while the server verifies it and handle any
corrective inventory updates. `ScribeSpell` saves the book entry and sends
`SpellMemorized { action: 0 }`; the handler then consumes one cursor scroll and
saves the character. Do not declare consumption from the scribe request alone.
The server handler has no posture gate, although the client can sit for normal
scribe presentation. Do not put an entire scroll stack on the cursor: EQEmu's
cursor consumption path emits a whole-slot deletion even when a stack remainder
exists. A stack already held on the cursor should be separated before scribing.

RoF2 item activation uses `OP_ItemVerifyRequest`, including for items in bags.
EQEmu's `Handle_OP_ItemVerifyRequest` sends its reply before running validation
and then starts the cast itself. **Do not send `OP_CastSpell` after the reply.**
The supported address range is equipment and general inventory (`0..32`) and
subslots of general bags (`23..32`, index less than 200); bank, shared bank,
cursor, merchant and explicit augment addresses are rejected. The server may
choose an augment effect or a quest hook internally; an augment is not addressed
as a standalone inventory item.

Click requirements differ from spellbook requirements. A wizard can activate a
Spirit of Wolf potion without being able to scribe that spell. Check the item's
`required_level` (`Level2`), available charges and cooldown. The separate `level`
field is not the requirement: the live Cloudy Potion has `level = 60` and
`required_level = 0`, and works for the level-20 fixture. Effect types `1` and `3` are normal
and expendable clicks; type `4` requires the item to be equipped, while type `5`
is restricted by the item's class mask. EQEmu checks the class mask for types
`4` and `5`. The server also checks casting state, stun/fear/mez, silence, zone
restrictions, reuse timers and quest handlers. Preserve its rejection messages.

The inventory decoder retains these original fields:

| Field | Serialized source |
| --- | --- |
| `click.spell_id` | Click effect header i32 at byte 0 |
| `click.required_level` | Click effect header u8 at byte 4 (`Level2`) |
| `click.effect_type`, `click.level` | Header u32 at 5, u8 at 9 |
| `click.max_charges`, `click.cast_time_ms` | Header i32 at 10 and 14 |
| `click.recast_seconds`, `click.recast_type` | Header u32 at 18 and i32 at 22 |
| `scroll_spell_id` | Effect ID in the fifth of six 30-byte effect blocks |
| `recast_timestamp` | Item serialization header u32 at 52: reuse expiry in Unix seconds |

Effect blocks also contain a variable name and trailing unknown word. Cast
duration is milliseconds; item reuse duration is seconds. Instance `charges`
and stack `count` are separate. Negative charges denote unlimited usage.

Successful finite casts consume a charge only after `SpellFinished` succeeds.
An interrupted cast does not consume a charge. Server `ItemChargeUsed` events
decrement a nonstackable item's charges, `ItemDeleted` events decrement a stack,
and an `ItemMoved` to `InventorySlot::DELETE` removes the final item. The wire
consumption count `0xffffffff` represents one unit for the charge/stack messages;
whole-item deletion retains its separate move-to-delete meaning. Cast completion
and charge changes must come from these server events.

Shared reuse timers arrive by group in `ItemUseEvent::Recast`. `recast_type == -1`
uses an independent item-ID timer and has no `OP_ItemRecastDelay` packet. Serialized
item updates and reconnect inventory expose the authoritative expiry timestamp.
On a rejected repeated click, EQEmu refreshes matching serialized items and sends
the remaining shared timer. Finishing an item cast can also send a spell-gem
notification with a special item casting slot; it is not a new memorized gem.

The source references are EQEmu's `common/patches/rof2_structs.h`,
`common/patches/rof2.cpp`, `common/inventory_profile.cpp`,
`zone/client_process.cpp::OPMemorizeSpell`,
`zone/client_packet.cpp::Handle_OP_ItemVerifyRequest`, and
`zone/spells.cpp::{CastedSpellFinished, GetItemSlotToConsumeCharge,
CheckItemRaceClassDietyRestrictionsOnCast, SendItemRecastTimer, SetItemRecastTimer}`.

## Client interaction and confirmation

`item_use_state.rs` owns the pending-operation state, exact inventory-instance
validation, scribe/click eligibility and server reuse timers.
`item_use_interaction.rs` connects those rules to `Interaction`, `LiveWorld` and
the original-art item inspection window. Only inspection of an owned item offers
Scribe or Use; item links, merchant stock and a trade partner's items remain
read-only. Every action rechecks the current slot, item ID and instance ID.

Carried scribing first queues a one-unit cursor move. The scribe request is
queued only after the move's `CommandSent` notification has updated local
inventory, preserving order without inventing a server acknowledgement.
Completion requires both the authoritative book entry and cursor consumption.
A rejected move sends no scribe request. Competing item moves, spell actions,
services, zoning, attacks and trades are gated while the operation is pending.

An item click remains pending across verification, cast start and application.
Finite-use items also wait for their authoritative charge/stack deletion; an
unlimited item finishes without waiting for a charge packet. Interruption clears
the pending cast without changing inventory. Rejections and timeouts remain
visible, and shared or serialized reuse expiry disables Use with the remaining
time. Item inspection displays the actual spell name, scribing or click level,
cast time, unlimited charges and reuse duration when applicable.

## Dedicated live fixture

Artificer is a separate level-20 human wizard on the `openeq_itemuse` account in
Plane of Knowledge. Its private connection file is
`~/.config/openeq/storage2-itemuse-credentials.json`, mode `0600`. Provisioning
followed backup `20260928T171357Z`; the server-side seeder is
`/home/daeken/eqemu-bootstrap/itemuse-seed.py`. No existing character, spellbook,
item definition or server rule was changed. The fixture contains the original
PEQ items below, rather than specially authored test effects.

| Item | ID | Slot | Effect |
| --- | --- | --- | --- |
| Spell: Minor Shielding | 15288 | 23 | Scribes spell 288 |
| Dragoncrypt Token | 47742 | 24 | Geomantra 5105, 2-second cast, unlimited charges, 180-second shared reuse group 24 |
| Backpack | 17005 | 25 | Contains the two potion fixtures |
| Cloudy Potion | 14514 | Bag 25, child 0 | Invisibility 42, instant, stack of two |
| 10 Dose Blood of the Wolf | 14534 | Bag 25, child 1 | Spirit of Wolf 278, 4-second cast, ten charges |
| Spell: Invisibility | 15042 | 26 | Scribes spell 42; reserved for the client UI probe |

```sh
cargo test -p openeq-net item_use::tests
cargo run -p openeq --bin item_use_smoke -- \
  "$HOME/.config/openeq/storage2-itemuse-credentials.json"
cargo test -p openeq --lib item_use
cargo run -p openeq --bin item_use_client_smoke -- \
  "$HOME/.config/openeq/storage2-itemuse-credentials.json"
```

The first live probe uses the ordinary client's `GameplayState` reducers. Its full
sequence consumes the scroll and both Cloudy Potions and leaves the charged
potion with nine charges, so it requires the fixture's original state. Back up
before deliberately restoring only this dedicated character for another run;
the initial provisioning script is not a general world reset. The subsequent
client UI probe consumes the Invisibility scroll in slot 26 and one more charge.
Neither probe should be rerun expecting the consumed fixture state to be intact.

The complete live proof passed on Storage2 on 2026-09-28. Its first run compiled
the actual repository `GameplayState` and presentation modules through a
temporary harness while unrelated interaction dispatch was being integrated;
the probe source above is the ordinary workspace binary. The log is
`/tmp/openeq-item-use-proof.log`. It verified:

- Scribe acknowledgement followed by scroll consumption and a persisted book entry.
- A reusable item cast, unchanged unlimited charges, a packed 9-byte shared timer,
  and rejection of a second click while that timer remained active.
- An interrupted four-second bag cast with all ten charges retained, then a
  completed cast with an authoritative decrement to nine charges.
- Instant bag casts consuming a stack from two to one, then removing its final unit.
- Reconnect inventory, spellbook and reuse expiry matching the server database.

The second live proof ran successfully as the ordinary workspace
`item_use_client_smoke` binary. It used actual rendered HUD hit targets through
`UiAction`, `Interaction` and `LiveWorld`, then confirmed the resulting server
events and reconnect persistence. The log is
`/tmp/openeq-item-use-client-proof.log`. It verified:

- Invisibility scribing through the one-scroll move and `CommandSent` FIFO.
- Completion of an unlimited Dragoncrypt Token click and a server reuse timer
  disabling the actual Use button while displaying the remaining time.
- A charged bag cast interrupted with nine charges retained, followed by a
  successful cast reducing the charge count to eight.
- Reconnect persistence of the learned spell and consumed charge.

The actual-state GPU captures `/tmp/openeq-live-scroll-scribe.png`,
`/tmp/openeq-live-item-recast.png` and `/tmp/openeq-live-item-charges.png` were
visually reviewed. They show the original UI artwork, readable effect metadata,
the enabled Scribe control, the disabled reuse control and the updated eight
charges. These captures render the HUD over a plain background; they do not
claim to validate the zone renderer.

The dedicated character remains logged out with Minor Shielding and Invisibility
learned, both scrolls and both Cloudy Potions consumed, the reusable token in
slot 24 and eight charges in the bag's second slot. Three item-use protocol tests
cover scribe bounds, actual bag-slot bytes, address rejection and exact timer
framing; the inventory decoder has a separate click/scroll/recast field
regression. Six client regressions cover eligibility, inventory identity,
confirmation and interruption, the live command FIFO, rejected moves and
read-only/service UI gating. Client Clippy passes with warnings denied for the
library and both probe binaries.
