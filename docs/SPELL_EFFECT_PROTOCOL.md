# RoF2 spell and particle events

This describes the EQEmu source used by Storage2 and the RoF2 decoder in
`openeq-net`. Spell IDs, particle-definition IDs, and item IDs are separate
namespaces. Presentation must not cause damage, apply buffs, or delay server
results while a visual finishes.

## Spell catalog mapping

The original `spells_us.txt` here has 174 fields, with effect arrays moved into
the final pipe-separated field. EQEmu's older fixed-array export has more than
200 fields. These zero-based fields supply visual metadata:

| Meaning | Fixed export | Compact client |
|---|---:|---:|
| Actor tag / projectile IT model | 2 | 2 |
| Cast duration, milliseconds | 13 | 13 |
| Buff duration formula / cap | 16 / 17 | 16 / 17 |
| Target type | 98 | 38 |
| Casting body animation | 120 | 60 |
| Target animation | 121 | 61 |
| Travel type | 122 | 62 |
| Spell affect category | 123 | 63 |
| Particle effect index | 145 | 85 |
| Uses persistent particles | 153 | 93 |

The particle effect index selects `spellsnew.eff`; the spell affect category is
not that index. Actual client examples: Minor Healing (spell 200) and Complete
Heal (13) use effect 278; Frost Bolt (54) uses 179; Minor Shielding (288) uses
220; Gate (36) uses 218. Empty optional fields must remain valid. The cast-time
field is presentation metadata; `BeginCast` supplies the active server timing.

Sources: `common/spdat.h`, `SPDat_Spell_Struct` numbered field comments;
`common/shareddb.cpp`, spell row loading; original client records above.

## Packet layouts and API

All integer fields are little endian. IDs are zone entity IDs unless specified
otherwise. Projectile positions remain in server XYZ, like spawn positions;
the app converts them once into scene coordinates. Recognized packets require
their complete payload and reject trailing bytes. Projectile floats and the
Action instrument modifier must be finite.

| Opcode | Bytes | Decoded event |
|---|---:|---|
| BeginCast `0x318f` | 10 | `BeginCast { caster_id, spell_id, cast_time_ms }` |
| InterruptCast `0x048c` | 8 plus optional NUL string | `CastInterrupted { id, string_id, message }` |
| Action `0x744c` | 56 | `SpellAction { source_id, target_id, spell_id, level, action_type, spell_level, instrument_modifier, effect_flag }` |
| SpellEffect `0x5936` | 28 | `SpellEffect(SpellEffect)` |
| SomeItemPacketMaybe `0x747c` | 116 | `Projectile(Projectile)` |
| AddNimbusEffect `0xc693` | 8 | `NimbusEffect { id, effect_id, removed: false }` |
| RemoveNimbusEffect `0x7b1e` | 8 | `NimbusEffect { id, effect_id, removed: true }` |

`BeginCast` stores spell u32 at 0, caster u16 at 4, and milliseconds u32 at 6.
An interrupt has no spell ID; it cancels the current cast for its entity.

RoF2 encodes Action as **ActionAlt_Struct**, not the 39-byte Action_Struct:
target u16 at 0, source u16 at 2, caster level u16 at 4, instrument float at 10,
action type u8 at 26, spell u32 at 33, spell level u8 at 37, success flag u8 at
38. Type **231** denotes a spell; other values are combat skill/action types.
The wire instrument modifier is normalized by the encoder: internal 10 becomes
1.0, 15 becomes 1.5. Push force/heading/pitch and inventory-casting metadata are
not visual effect identifiers.

`SpellEffect` carries effect ID u32 at 0, source ID at 4, target ID at 8,
duration in milliseconds at 12, finish delay in milliseconds at 16, and an
unknown u32 at 20 (normally 3000). EQEmu writes the remaining bytes as 1, 1, 0,
0. **The effect ID is not a spell ID.** The native timing semantics of finish
delay remain unverified; OpenEQ currently starts the impact immediately and
uses the supplied duration, without interpreting nonzero finish delay. No wire
flag distinguishes a permanent effect.

`Projectile` reads source Y/X/Z floats at 0/4/8, velocity at 24, launch angle
at 28, tilt at 32, arc at 44, source/target/item u32 at 48/52/56, skill u8 at 73,
item type u8 at 74, and a 27-byte model name at 89. The fixed name may occupy
all 27 bytes without a NUL. The RoF2 encoder inserts byte 70 = 175 to select
ranged animation; this is neither an ID nor a spell marker. Projectile item
IDs and IT model names select actual item geometry. No spell ID or authoritative
arrival time exists in this packet.

Nimbus addition/removal shares `{ spawnid: u32, nimbus_effect: int32 }`. The
decoder preserves the effect bit pattern as u32; invalid/unavailable visual IDs
must not prevent processing other events. Zero removal packets may be emitted
while clearing empty nimbus slots.

Sources: `utils/patches/patch_RoF2.conf`; `common/patches/rof2_structs.h`;
`common/patches/rof2.cpp` encoders for Action, BeginCast, SomeItemPacketMaybe;
`common/eq_packet_structs.h` SpellEffect_Struct/RemoveNimbusEffect_Struct;
`zone/mob.cpp` SendSpellEffect, AddNimbusEffect, RemoveNimbusEffect.

## Authoritative lifecycle

`Mob::SendBeginCast` broadcasts the active spell and cast duration. Normal
completion reaches `SpellOnTarget`, which sends Action with effect flag 0 to
the caster, target, and nearby observers **before** checking every application
condition. Successful application sends a second Action with flag 4 to the
caster and target. Nearby observers may receive only flag 0. Thus:

- Render the visual attempt from flag 0; do not require flag 4 for all viewers.
- Deduplicate a matching flag 4 so caster/target do not see a second burst.
- Flag 0 is not proof that a buff landed or damage occurred. Buff and damage
  packets remain authoritative for those results.
- Area effects can send one Action per target. Deduplicate per source, target,
  and spell; do not collapse all targets into one effect.

`Client::SendSpellAnim` can also send visual-only Action with flag 0, without a
preceding BeginCast. Proc effects likewise need not have a normal cast bar.
Do not require a local cast request to render an authoritative effect.

**Spell names and travel_type do not establish a server projectile.** Frost
Bolt (54) has travel_type 3 but target_type 5 (`ST_Target`); its Action is sent
at ordinary spell application. Do not invent travel before it or delay impact.

Actual mage bolts such as Bolt of Flame (68) have target_type 1
(`ST_TargetOptional`). `TrySpellProjectile` emits an item projectile at launch,
and `ProjectileAttack` later invokes `SpellOnTarget` at server-predicted impact.
The internal projectile record stores the spell ID, but the packet does not.
`UseLiveSpellProjectileGFX` selects the spell's field-2 IT model (e.g. IT11504).
Without that rule, configured fire items or an ordinary arrow provide the
graphic. Wire skill is not a reliable classifier: `ProjectileAnimation` can
leave it zero although internal tracking uses Conjuration.

Projectile speed, arc and homing parameters describe presentation, not damage.
The server recalculates impact time when the target moves. A projectile can be
sent even when the initial LOS test prevents applying damage, so bound visual
lifetime and retire it on despawn or timeout. A later cast interruption should
not erase a projectile already launched by a completed cast.

Sources: `zone/spells.cpp` SendBeginCast, SpellFinished target dispatch,
SpellOnTarget, SendSpellAnim; `zone/spell_effects.cpp` TrySpellProjectile;
`zone/special_attacks.cpp` ProjectileAnimation and ProjectileAttack.

## Persistent effects and cleanup

Existing `Buffs` events decode BuffCreate (`0x3377`) and TargetBuffs (`0x4f4b`).
They carry an entity ID, tick timer, complete-list flag, entries, and list kind.
Each entry has slot, spell ID, remaining ticks, hit count, and caster name.
BuffChanged (`0x659c`) adds/removes a specific slot. Durations are six-second
buff ticks, not particle milliseconds; permanent/sentinel values must not
overflow duration calculations. Spell ID `0xffffffff` removes a list entry.
The RoF2 buff-removal encoder can send both BuffChanged and a BuffCreate removal;
cleanup must be idempotent. A complete list replaces its previous slots.

Use persistent-particle metadata together with actual buff state. A spell
Action alone cannot establish durable buff state. Local profile buffs and
explicit target-buff lists are available; the protocol does not give every
nearby observer a full buff list for every NPC.

Explicit AddNimbus persists until RemoveNimbus, despawn, or leaving the zone.
However, EQEmu's `perm_effect` argument to SendSpellEffect only updates the
server's internal nimbus list; it is **not serialized**. Late-join replay in
`EntityList::SendNimbusEffects` sends the three retained effects as ordinary
SpellEffect packets with durations 1000/2000/3000, without a permanence flag.
Those durations cannot reliably distinguish replay from a transient effect.
Respect authored effect metadata and retain this protocol limitation.

RoF2 ZoneSpawns does not serialize a nimbus ID list. Its early DefaultEmitterID
is hardcoded zero and its reserved tail/PhysicsEffects area is zero-filled.
Do not reinterpret skipped spawn bytes as a nimbus list.

Cancel active cast emitters on interruption; clear entity-attached effects on
despawn and all effect state on zone handoff/disconnect. Bound transient effect
counts/lifetimes independently of network delivery. Timers may retire visuals;
they must never fabricate gameplay outcomes.

## Validation

Net regressions reproduce the original fixed offsets with distinct IDs,
asymmetric coordinates and timing values, preserve Action flags/types, verify
every truncated prefix and trailing data, reject nonfinite projectile values,
and accept a full-width item model name. Existing buff tests cover variable
lists, complete strings and bounded counts. No server data changes are needed.

For opt-in packet metadata tracing, set
`RUST_LOG=openeq_net::spell_effects=debug` in a tool that honors the tracing
environment. The target logs these events, not profiles, inventory or secrets.
The guarded `gameplay_smoke` accepts that filter.

On 2026-09-29 the dedicated offline Arcanist fixture completed the gameplay
packet probe. Minor Shielding produced BeginCast (2500 ms), then 2.531 seconds later
Action flag 0, a full Buffs entry (270 ticks), and Action flag 4 within 0.15 ms.
Buff removal produced both BuffChanged and Buffs removal events. Counts were
1 BeginCast, 2 Action, 7 Buffs, 2 BuffChanged, 1 zero-damage event and 4 spell-bar
updates across the probe's reconnects. This spell sent no explicit SpellEffect,
Projectile or Nimbus packet. Interruption was skipped because Gate was not
memorized; it is not claimed as part of this run. Stats, currency and inventory
content were unchanged and Arcanist logged out. Its server/login Z adjustment
across reconnects was observed separately; no position-restoration commands or
database writes were made.

EQEmu deliberately adds six packed Z units to the player's ZoneEntry spawn
(`zone/client_packet.cpp`, the "arbitrary lift" after `FillSpawnStruct`). RoF2
passes that value through, and position decoding divides it by eight. A probe
that preserves this login position in its stationary heartbeat can therefore
save a Z value 0.75 above the pre-login database value. Pose equality is not
claimed by these probes; inventory, currency and character-stat comparisons
are independent of this adjustment.

The separate `spell_effect_smoke` uses the same production effect anchors and
original assets as the client. It renders both the player body with animated
hand sockets (default third-person view) and camera-relative hand anchors with
the player body omitted (`--first-person`). The dedicated fixture must already
have Minor Shielding memorized and must not have its buff active. The probe
casts that spell, waits for the authoritative buff, removes only that added
buff, then casts it again and sends an interruption after casting particles
appear. It never moves inventory, coins, memorized spells or the spellbook.

```sh
cargo run -p openeq --bin spell_effect_smoke -- \
  "$HOME/.config/openeq/storage2-spell-credentials.json" \
  /tmp/openeq-spell-effects-third-person
cargo run -p openeq --bin spell_effect_smoke -- \
  "$HOME/.config/openeq/storage2-spell-credentials.json" \
  /tmp/openeq-spell-effects-first-person --first-person
```

Both final live runs passed on 2026-09-29 after the native emitter-motion
implementation. Each image was compared with the identical camera, actor pose
and scene rendered without particles; a pixel counted as changed when any RGB
channel differed by more than four. The completion frame contains the reported
impact-stage particles and can also contain surviving casting particles; its
pixel count is for that whole frame, not an isolated impact-only pass.

| View | Casting particles / changed pixels | Impact particles / completion-frame changed pixels | Interrupted particles |
|---|---:|---:|---:|
| Third-person | 90 / 173 | 38 / 589 | 2 → 0 |
| First-person | 90 / 6,246 | 38 / 1,368 | 2 → 0 |

The screenshots were inspected in
`/tmp/openeq-spell-effects-third-person/{casting,impact,interrupted}.png` and
`/tmp/openeq-spell-effects-first-person/{casting,impact,interrupted}.png`.
There were no invalid instances, missing textures or capacity rejections.
Interruption cleared both casting and pending-cast state, left no Shielding
buff, and removed all particles. Both sessions logged out and left no test buff.
Read-only database snapshots confirmed unchanged character stats, item content,
coin balances (including shared platinum), spellbook and memorized spells.
Current HP/mana/endurance were excluded because casting and regeneration alter
those resources. The observed Z values were 52.75 → 53.5 → 54.25 across the two
logins, with unchanged X/Y/heading. Inventory instance GUIDs also changed:
`SharedDatabase::GetInventory` creates fresh item serials and writes them back
to `inventory.guid`; these generated identifiers are recorded separately from
item-content equality. No database restoration or direct database writes were used.
