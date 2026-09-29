# RoF2 gameplay protocol

The client uses the actual RoF2 encoders and handlers in EQEmu commit
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. Reference files are
`common/patches/rof2.cpp`, `rof2_structs.h`, `rof2_limits.h`,
`common/emu_constants.h`, and `zone/client_packet.cpp`, `corpse.cpp`, `zoning.cpp`.

## API

`ConnectionConfig::connect()` still returns a `ZoneClient`. Its `next_event()`
returns existing spawn/environment events plus `ZoneEvent::Gameplay(GameplayEvent)`.
Send structured `gameplay::Command` values through `ZoneClient::command()`.
A successful send is transport submission, not server acceptance.

Network positions retain EQEmu server X/Y/Z and heading conventions. Raw zone
assets use the opposite X/Y order: EQEmu's `zone/map.cpp` explicitly swaps model
vertices when constructing its collision map. The client `coordinates` module
converts points, headings, velocity and turn direction at the `LiveWorld`
boundary. Direct `ZoneClient` users and GM command text such as `#goto X Y Z`
continue to use server coordinates. A server actor's Z is its center, not the
floor; its offset depends on race and size. Do not position a human by copying
a smaller NPC's center height.

Implemented messages cover:

- Initial recursive inventory, item updates, moves/deletes/charge consumption;
  item identity, icon, equipment mask, weapon/armor stats, bags and children.
- Say, tell, guild, group, shout, auction, OOC, raid, and emote;
  plain, formatted, and simple server messages. Formatted messages retain their
  original `string_id`, color and arguments for the client's `eqstr_us.txt` catalog.
- Initial profile: current HP/mana/endurance, stats, skills, spellbook,
  memorized spells, currency and identity; subsequent resource updates.
- Target/assist/consider, autoattack, damage, death, animation, posture,
  initial equipment appearance and wear changes.
- Corpse opening, serialized loot items, item pickup acknowledgment and completion.
- Server door lists, door clicks and movement; server-requested zoning and
  authenticated world/zone handoffs, including same-zone teleport information.
- Spell memorization/forgetting, gem casting, interruption, cast start/finish and
  cooldown feedback, initial/replacement/partial buff lists, and buff dismissal.
- Merchant catalog, purchases/sales and personal/shared banking; see
  [COMMERCE_PROTOCOL.md](COMMERCE_PROTOCOL.md) for transaction reconciliation.
- Quest/item links and group membership; see [SOCIAL_PROTOCOL.md](SOCIAL_PROTOCOL.md).

## Inventory details

RoF2 sends a binary stream of variable-length item records. It is not the old
pipe-separated Titanium serialization. An inventory packet begins with a dword
item count. Each item contains a 77-byte instance header, optional 25-byte
Evolution header, ornament strings, a 26-byte header finish, name/lore/model
strings, and fixed bodies of 255, 74, 76 and 171 bytes separated by strings and
six 30-byte effect headers. It ends with a child count followed by explicit child
indices and recursively serialized items.

The explicit child index is essential: the current EQEmu serializer reuses a
bag's first `SubSlotNumber` in the child headers. `InventoryItem.children` therefore
uses the preceding child index to assign each actual bag address. Parsers bound
item count, recursion, string lengths, and every byte access; incomplete or
trailing records fail instead of producing empty/fabricated items.

`InventorySlot {kind,slot,bag,augment}` represents the wire address. Use
`server_slot()` and `from_server_slot()` to obtain this server's canonical IDs.
Do not use Titanium's historical bag slots 251/351.

| Address | Current EQEmu server slot |
| --- | --- |
| Equipment / power / ammo | 0–22; power is 21, ammo is 22 |
| General inventory | 23–32 |
| Cursor | 33 |
| Bag in general slot `s`, child `i` | `4010 + (s - 23) * 200 + i` |
| Cursor bag child `i` | `6010 + i` |
| Bank main slot `s` | `2000 + s` |
| Bank bag child `i` | `6210 + s * 200 + i` |
| Shared bank main slot `s` | `2500 + s` |
| Shared bank bag child `i` | `11010 + s * 200 + i` |

Move count zero means the entire item. Normal successful client item moves do
**not** get an echo from EQEmu; the client predicts the requested change and
applies any subsequent server corrections. The live probe proves persistence
through a fresh inventory stream after reconnecting rather than inventing an
acknowledgment. Server item packet type `0x66` is a corpse item, `0x67` is an
inventory delivery (including auto-equipped loot), `0x69` inventory, and `0x6a`
limbo/cursor delivery. `OP_DeleteItem` consumes stack units and `OP_DeleteCharge`
consumes click charges; their `0xffffffff` quantity means one unit/charge per
packet. `ItemChargeUsed` preserves that distinction, including a non-expendable
item reaching zero charges. Whole-item removal is `OP_MoveItem` to the invalid
destination and retains its whole-item sentinel. Corpse slot kind is 11 and its slots are distinct from
possessions despite sharing slot numbers.

## Combat, loot, and resources

Damage is signed: misses, invulnerability and other special outcomes must not be
rendered as positive damage. Zero-damage spell-effect packets (including successful
self buffs) are not melee misses; server resist messages remain separate. Spawn animation packets are four bytes and carry
repeated independent action events. Wear changes are 27 bytes; unknown slots
such as 255 occur when removing corpse equipment and must be ignored by the
renderer, not treated as a disconnected stream.

Loot response 1 opens the corpse; response 6 means the complete item list has
arrived and the Loot All feature is available. Response 6 is **not** failure.
Item pickup uses corpse ID, looter ID, and the corpse item's slot. The server may
autoequip a looted item. The resulting item-delivery packet contains its actual
possessions address. A quest or dynamic-zone loot denial returns `auto_loot=-1`
in the item acknowledgment without closing the corpse. `rejected` preserves this
result: the item stays listed, pending clears, and automatic Loot All stops.

The EQEmu profile encoder writes placeholder resource maxima 123/234/345. They
are deliberately not exposed as real totals. Live gauges use actual HP, mana and endurance update packets.

## Spellcasting

`MemorizeSpell` and `UnmemorizeSpell` use the RoF2 16-byte spell/gem message.
Both require a valid spell ID, including forgetting a spell. Gem slots are 0–11;
the extra four profile slots are unsupported client padding. The server enforces
class/level/spellbook and any AA restrictions on additional gems.

`CastSpell` uses RoF2's 44-byte structure with an invalid inventory slot for gem
casting. `InterruptSpell` is an empty client-to-server `OP_ManaChange`, not a
fabricated server interruption. Server `SpellBarEnabled` is the opposite-direction
20-byte `OP_ManaChange` and carries actual resources plus the gem to unlock.
Unlocking alone does not prove success: interrupted and failed casts also send it.

`SpellMemorized.action` is 0 for scribing, 1 for memorizing, 2 for forgetting,
and 3 for a successful cast's cooldown update. `reduction` adjusts the original
spell catalog recast time. `BeginCast` supplies authoritative duration in ms.
`CastInterrupted` preserves the string table ID and optional message.
`SpellAction` uses the actual 56-byte RoF2 `ActionAlt_Struct`; the historical
39-byte structure is not what this encoder sends.

Profile spell refresh timers and buffs are exposed. `Buffs.all` replaces the
entity's list; a partial list can add/update one slot or remove it using spell ID
`0xffffffff`. `BuffChanged.removed` also reports removal. Player and target buff
lists share their wire encoder. `RemoveBuff` uses the RoF2 slot from the server
buff event unchanged; EQEmu converts it to its internal slot representation.

## Zoning and doors

RoF2 reconnects to world for each accepted zone transition using the existing
login session key and `LoginInfo.zoning=1` at byte 188 of its 464-byte login
payload. `ZoneClient` retains that private authentication context; it does not
re-run login or return to character select. The handoff future lives on the
client object so repeatedly canceled `next_event()` calls from a movement
heartbeat do not restart a partially completed handshake.

`ZoneTransition` marks when old entities/targets should be discarded. Fresh
profile, inventory, spawns, environment and `Ready` events then arrive normally.
Movement heartbeats are suppressed while the handoff is active. Gameplay commands
and targeting return `ZoneError::Zoning` during that interval. A direct
`ZoneClient::connect()` without world authentication cannot cross zones;
`ConnectionConfig::connect()` enables the complete path.

For a same-zone `ZoneChangeRequested`, apply its position and heading locally;
no world handoff is needed. A zone rejection retains its server result code.
Authored classic WLD reference volumes now drive natural border requests.
They resolve a point number against `OP_SendZonepoints`; the destination
coordinates in that packet are never used as source trigger locations. The
latest player position is sent before the request, and the server chooses the
arrival point and enforces access checks. See [zone travel](ZONE_TRAVEL_PROTOCOL.md).
GM `/say #zone ZONE` remains useful for development and unsupported trigger formats.

Door records are 100 bytes. Positions remain EQ XYZ, heading uses 512 units per
turn, and size is percent. Door action 2 opens and 3 closes, reversed for inverted
doors. `open_type` selects motion/interaction behavior; static objects also occur
in the door list and should not all be animated as hinged doors.

## Live fixture and verification

A separate account `openeq_gameplay`, character **Mechanic**, lives in Arena. It
is a level 10 warrior with GM permissions and invulnerability for explicit
client tests. Explorer is never logged in by this probe. Provisioning followed
the backup run `20260928T063219Z` in `/srv/eqemu/backups` and did not alter Explorer or world
content. The reproducible server seeder is
`/home/daeken/eqemu-bootstrap/gameplay-seed.py`.

Private configuration (mode 0600, never committed):
`~/.config/openeq/storage2-gameplay-credentials.json`.

```sh
cargo run -p openeq-net --bin eqgameplay -- \
  --config "$HOME/.config/openeq/storage2-gameplay-credentials.json"
cargo run -p openeq-net --bin eqgameplay -- \
  --config "$HOME/.config/openeq/storage2-gameplay-credentials.json" --exercise
cargo run -p openeq-net --bin eqgameplay -- \
  --config "$HOME/.config/openeq/storage2-gameplay-credentials.json" --travel
```

`--exercise` mutates only the dedicated fixture: moves a cap between inventory
and head equipment, moves rations between bag children, reconnects to verify
both, restores them, spawns a transient test rat, attacks it, and loots a seeded
cap. A repeated run destroys the duplicate cap from the previous test before
starting. It does not claim transport submission proves an inventory move.
`--travel` crosses Arena → Plane of Knowledge → Arena and toggles a normal PoK
door open and closed.

Verified 2026-09-28 against Storage2:

- Initial profile, three main inventory items and two differently indexed bag
  children parsed from live packets.
- Say and self-tell yielded three server chat messages (tell recipient channel 7
  and sender confirmation channel 14).
- Cap equipment and bag child movement persisted across reconnect; database
  queries independently confirmed restoration and the newly acquired loot cap.
- Real autoattack produced damage, NPC death and animations; corpse contents,
  pickup acknowledgment, actual auto-equipped delivery and loot completion all
  arrived. Final run: two damage packets, one death, three animation events.
- Both zone handoffs completed with the 200 ms movement heartbeat running;
  PoK supplied 142 door records, and a click roundtrip produced actions 2 and 3.
- Unit tests exercise every truncated prefix of item/movement/text/fixed gameplay
  packets, explicit bag child addresses, malformed counts and recursion,
  signed damage, typed slot conversion, zone and door offsets.

### Separate caster fixture

**Arcanist**, account `openeq_spells`, is a level 10 wizard in Arena with GM
permissions. This account deliberately has invulnerability disabled because
EQEmu rejects beneficial spells cast on invulnerable targets. Provisioning
followed backup run `20260928T070201Z`. The server seeder is
`/home/daeken/eqemu-bootstrap/spell-seed.py`; its private local configuration is
`~/.config/openeq/storage2-spell-credentials.json` (0600).

```sh
cargo run -p openeq-net --bin eqspells -- \
  --config "$HOME/.config/openeq/storage2-spell-credentials.json"
```

This guarded probe only accepts the Arcanist fixture. It memorizes Frost Bolt,
Minor Shielding and Gate; casts and removes Minor Shielding; interrupts Gate;
casts Frost Bolt at a newly spawned transient test rat; forgets Gate; and
reconnects to verify the remaining gems and forgotten slot persisted. GM status
prevents random fizzles; mana consumption and server effect handling remain real.

Verified live on 2026-09-28: three memorize confirmations; Minor Shielding's
2500 ms cast, 270-tick buff and 10 mana cost; buff removal; Gate's 5000 ms start
and explicit interruption; Frost Bolt's 14 damage and rat death with spell ID 54;
action-3 cooldown notifications; and reconnect profile gems `[54, 288, 0xffffffff]`
after forgetting Gate. EQEmu sends caster-side spell damage through formatted
message 9073; a lethal hit need not also produce an `OP_Damage` event. The probe
therefore requires real damage or a death attributed to spell 54, not merely a
successful command submission or spellbar unlock. All 35 net unit tests and
`cargo clippy -p openeq-net --all-targets -- -D warnings` pass.

Limitations: player trading/augmentation workflows, guild/raid membership
management, UCS custom channels, item/AA casting, spell scroll scribing and authored
EQG and absolute-destination zone-line triggers remain separate work. Rendering spell particles and richer spell
UI behavior are client concerns separate from these server-authoritative messages.

### Integrated client verification

`cargo run -p openeq --bin gameplay_smoke -- CONFIG` accepts only the dedicated
Arcanist configuration. It exercises `LiveWorld`, `Interaction`, `GameplayState`
and the HUD presentation data together: original spell names, self-tell echoes,
casting pending/start/finish, real mana cost and cooldown, buff duration/removal,
and an inventory item moved to the cursor then restored across two reconnects.
This passed live on 2026-09-28. Gate interruption is additionally exercised when
that spell is memorized; the separate network probe always tests it.
