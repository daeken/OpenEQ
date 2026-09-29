# Death and recovery: RoF2 implementation plan

Research date: 2026-09-29. Source baseline: local EQEmu `4aceae18b` in
`../EQEmu`, OpenEQ `4f32558` plus the in-progress overnight changes. This plan
distinguishes source-confirmed behavior from live behavior still to test.

**First target: ordinary forced return to bind.** The server agent checked
Storage2 read-only and reported `Character:RespawnFromHover=false`. EQEmu's
source default is also false (`common/ruletypes.h:126`). Hover recovery is a
separate supported server mode, not a prerequisite for basic player death.
Initial protocol research was read-only. Live probes use dedicated disposable
characters; Explorer is excluded.

## Status and scope

Implemented:

- `crates/openeq-net/src/death.rs`: bounded decoders for respawn choices, bind
  transfer, and resurrection offers; encoders for choice and resurrection reply.
- `GameplayEvent::Recovery(DeathEvent)` and `Command::Death(DeathCommand)`
  delegate to that module. The client never sends a local countdown timeout.
- `ResurrectionOffer` preserves the original 236-byte server packet privately.
  Answering changes only action at offset 224; presentation cannot accidentally
  transpose reply coordinates or rewrite original name bytes.
- `LiveWorld` and its network worker identify the player only from a living
  player spawn with matching name. A corpse or same-name NPC cannot reclaim the
  player's movement ID or camera destination.

- `crates/openeq/src/death.rs`: a pure recovery reducer with generation/revision
  tokens, countdown presentation, distinct hovering/living resurrection paths,
  pending-command rollback, and authoritative revival gates.
- `crates/openeq/src/death_ui.rs`: original-skin respawn choices, timer, paging,
  resurrection offers and pending/error presentation. The runtime appends this
  overlay after HUD/map, dispatches typed recovery actions, releases the cursor,
  and gates movement using the live lifecycle.

Transport/lifecycle integration is described below. Live death/recovery
verification is tracked separately. Decoding a packet or sending an answer does
**not** establish revival.

## Source-confirmed wire messages

All integers and floats below are little-endian. Coordinates are EQEmu server
coordinates. Convert position/heading once at the presentation boundary with
`openeq::coordinates`; never convert the retained resurrection reply.
Opcodes come from `../EQEmu/utils/patches/patch_RoF2.conf`.

| Message | Opcode | Direction | Wire layout |
| --- | --- | --- | --- |
| Death | `0x6517` | Server → client | 32 bytes: eight `u32` values at 0 spawn ID, 4 killer ID, 8 corpse ID, 12 attack skill, 16 spell ID, 20 bind zone ID, 24 damage, 28 unknown. Existing `gameplay.rs` decoder skips bind zone and the unknown field. |
| Respawn window | `0x0ecb` | Server → client | 16-byte header, variable choices, optional EQEmu zero trailer; detailed below. |
| Respawn selection | `0x0ecb` | Client → server | Exactly one `u32` option ID. No coordinates or label. |
| Zone player to bind | `0x08d8` | Server → client | `u16 zone, u16 instance, f32 x,y,z,heading`, NUL-terminated label, `u8 save_items, u32 hp,mana,endurance`. Minimum 34 bytes; length is 34 + label bytes. |
| Resurrection offer | `0x3c21` | Server → client | Exactly 236 bytes, detailed below. |
| Resurrection answer | `0x701c` | Client → server | The same 236-byte layout, action 0 decline / 1 accept. |
| Resurrection complete | `0x760d` | Server/world routing | No direct client command needed. The zone server forwards completion after the accepted answer. |

References: `common/patches/rof2_structs.h:1546,1565,1609,3068`;
`common/patches/rof2.cpp:3430,4544,6061`;
`zone/client_packet.cpp:13664,13692`.

### Respawn choices

Header: offset 0 initial selection (`u32`, despite the struct's "unknown"
comment), offset 4 countdown milliseconds, offset 8 unknown, offset 12 count.
Each choice contains 24 fixed bytes: `u32 option_id, u32 zone_id`, then `f32
x,y,z,heading`; a NUL-terminated label; and one flag byte. The last choice is
the server's resurrection choice and has flag 1, initially unavailable. Other
choices have flag 0. There is **no instance ID** in a wire choice; the server
retains that in its `RespawnOption`. Do not reconstruct a zone command from the
choice's location: submit the numeric selection.

`Client::SendRespawnBinds` (`zone/client.cpp:5565`) defaults to bind if the option
list is empty, then appends resurrection. It writes
`16 + sum(26 + label_length)` bytes but allocates
`17 + sum(27 + label_length)`. Thus the actual current EQEmu packet contains
`count + 1` zero bytes after the encoded choices. The new parser accepts either
the exact encoded size or exactly that zero trailer, rejecting partial or
nonzero trailers. A naive exact-consumption parser rejects this server's packet.

The codec bounds choices to 256, labels to 1024 bytes, checks minimum remaining
length before allocation, rejects duplicate IDs, and rejects nonfinite poses.
An out-of-range initial selection is preserved; the UI should fall back to its
first valid choice instead of trusting that index. Source scripts can change
the default list before death (`Client::AddRespawnOption`, `client.cpp:8704`).

### Resurrection layout

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | Unknown |
| 4 / 6 | 2 / 2 | Zone / instance |
| 8 / 12 / 16 | 4 each | **Y / X / Z** |
| 20 | 4 | Unknown |
| 24 | 64 | Recipient name, NUL-terminated within field |
| 88 | 4 | Unknown |
| 92 | 64 | Resurrecting caster name |
| 156 | 4 | Spell ID |
| 160 | 64 | Corpse name |
| 224 | 4 | Action: 0 decline, 1 accept |
| 228 / 232 | 4 each | RoF2 trailing unknown words |

The emulator's canonical `Resurrect_Struct` is **228 bytes**; the RoF2 wire
version is **236 bytes**. EQEmu's patch decodes exactly 236 bytes and translates
to the canonical layout. Sending the canonical structure will be rejected.
Offer details are not a grant of health, XP, inventory, or a completed teleport.

`Corpse::CastRezz` builds the offer (`zone/corpse.cpp:2356`), world routes it to
the online recipient (`zone/worldserver.cpp:908`), and the receiving zone stores
pending XP/corpse/spell data before forwarding it. The client must validate the
recipient against the connected character and keep only the current pending
offer. The packet has no explicit expiry duration; do not invent one from the
respawn countdown. A displayed spell percentage is advisory, derived from the
spell catalog only when known.

## Server lifecycle and client requirements

### Death and corpse identity

`Client::Death` (`zone/attack.cpp:1814`) sends logout-related notices, death,
interrupts casting, marks the player dead, applies server rules, and optionally
creates the player corpse. When a corpse is made it retains the old entity ID;
the live player's server ID becomes zero (`attack.cpp:2099`). The death packet
uses that old ID for `corpseid`, even before the branch decides whether a corpse
will be left. Do not treat its nonzero corpse ID as proof of persistence.

`OP_BecomeCorpse` is mapped to `0x0000` / "Unused?" in this RoF2 patch file.
Do not register opcode zero as a death message. Existing `Death` plus spawn
kind 2/3 carries the usable corpse presentation path. Keep the old corpse in
the scene across a same-zone revival; the replacement player is a new entity.

The previous `live.rs::gameplay_event` implementation only set a dead animation,
corpse flag, and zero entity HP and stopped attack. Its camera and background
heartbeat could continue sending the old player/corpse ID. The transport slice
now closes that gap with independent foreground and worker movement authority.

Required state: record a death generation plus old player/corpse identity;
clear outgoing movement immediately in **both** network and foreground layers;
cancel casting/item-use and close loot/trade/merchant actions. Preserve chat.
Do not wait for a UI frame to stop the background heartbeat. HP reaching zero
alone is not the sole death authority: wait for own `Death` or a recovery packet.
Do not fabricate inventory loss, XP loss, buff loss, or money changes.

### Forced bind: Storage2's current mode

When hover is disabled or no corpse is left, `Client::Death` calls `GoToDeath`
(`attack.cpp:2130`, `zoning.cpp:1182`). That invokes `MovePC` with mode
`ZoneToBindPoint`. `ZonePC` sends `OP_ZonePlayerToBind` after resolving bind
coordinates (`zoning.cpp:894,948`).

For a same-zone bind with hover disabled, the packet's zone ID is **zero**
(`zoning.cpp:956`): this forces a real zone re-entry instead of a local respawn.
The client must send `ZoneChange` with that zero ID, let the server resolve bind,
and retain a **forced re-entry reason** across the response. Server
`Handle_OP_ZoneChange` explicitly resolves zero using bind (`zoning.cpp:78`), and
`DoZoneSuccess` can return success for the *same* zone/instance (`zoning.cpp:536`).

OpenEQ's previous `ZoneClient::next_event` only started a handoff if the accepted
zone/instance differed. A narrowly scoped pending recovery transfer flag now
allows successful forced bind to re-enter the same zone through the existing
authenticated world handoff. Ordinary cancelled border travel keeps its local
semantics. The same-zone success's zeroed location fields are not a teleport to
the origin. Fresh zone-entry/profile state is authoritative; queuing a request
does not predict revival.

### Hover bind and timer

With hover enabled **and a corpse left**, the server starts its respawn timer
and sends choices. `client_process.cpp:99` automatically selects option zero
when the timer expires. Render the countdown locally but let the server perform
timeout selection; do not race it by automatically sending another selection.

For a selected option in the same zone/instance,
`HandleRespawnFromHover` (`client_process.cpp:2126`) sends bind transfer,
restores server resources, then `ClearHover` (`:2294`) allocates a **new entity
ID**, broadcasts `OP_ZoneEntry`, sends buffs, and clears its dead flag. This path
does **not** resend a complete profile or normal zone Ready handshake. The spawn
adds six Z units; avoid stacking an additional recovery lift onto that.

The client must distinguish this local revival from forced bind re-entry,
apply destination once, wait for the new living own spawn, update both movement
IDs, preserve the corpse, and resume only after fresh spawn/position state is
installed. Across zones, hover selection uses ordinary solicited zone change
and the existing handoff machinery. Compare zone **and instance**, not just zone.

The packet label ("Bind Location", "Resurrect", or a quest-provided name) is
display text, not a machine-readable recovery type.

### Resurrection before and after returning to bind

While hovering, accept the final resurrection choice with `OP_RespawnWindow`.
The server requires pending rez XP/spell state, positions at the corpse, sends
bind transfer, performs `ClearHover`, then executes resurrection effects. Do not
also send `RezzAnswer`: that would consume the same offer twice.

After returning alive to bind, accept or decline through `OP_RezzAnswer`.
`Client::OPRezzAnswer` (`client_process.cpp:1034`) verifies there is pending XP,
applies resurrection rules and XP, and requests a `ZoneSolicited` move to the
corpse. This may be within the same zone or across zones. The client follows the
server's movement/zone messages rather than teleporting immediately on click.
The server sends completion internally to the corpse zone; the client does not
emit `RezzComplete`.

An invalid hover resurrection selection is particularly harmful: the handler
disables its timer before checking pending resurrection state, then returns if
there is no valid offer. Disable that choice unless there is an appropriate
offer, and validate again at dispatch. Reject duplicate clicks and stale option
IDs locally. Require a matching latest offer and character for any answer.
Decline while hovering uses RezzAnswer action zero to clear pending rez, while
leaving ordinary respawn choices available. Cross-zone offers during hover need
a dedicated test before enabling them; the hover selection path uses its stored
current-zone option, not the offer's arbitrary destination.

### Inventory, resources, corpse recovery and disconnects

Keep existing server-authoritative item/loot machinery. Corpse construction
depends on `LeaveCorpses`, `LeaveNakedCorpses`, level, and GM status. Items may
move to the corpse via `MoveItemToCorpse`, which sends item-deletion updates
(`zone/corpse.cpp:419,467,471`). RoF2 hover mode keeps carried cash in this source
branch (`:319`). Blanket local clearing would be incorrect for several rules.
Bind packet footer resource values are zero in the encoder; do not apply them
as an authoritative new resource pool. Use new profile/resource packets.

Player corpses use the existing loot-request/item/acknowledgment flow. Test
equipment and bag contents, a rejected loot, and corpse removal using existing
ownership slots. Resurrection does not imply items have been looted or the corpse
has been deleted. XP is server-owned; OpenEQ currently does not expose complete
experience state in `PlayerProfile`, so report that limitation instead of claiming
XP recovery verified from the UI alone.

Disconnects invalidate pending selections/offers. On reconnect do not replay a
selection or accepted offer, restore a corpse as the live character, or reuse an
old zone generation. Reconstruct from fresh server state. There is no need to
persist client-side death state across independent login sessions.

## Prioritized implementation slices and ownership

1. **Packet foundation — complete in this slice.** `openeq-net/death.rs` owns
   wire types, codecs, limits, and fixtures. `gameplay.rs` only has wrapper/delegate
   glue. Existing `Death` remains compatible; bind-zone metadata can be exposed
   later if needed, but forced-zero transfer does not require guessing it.
2. **Pure recovery reducer — implemented.** `openeq/src/death.rs` owns `Alive`,
   `Dead`, `ChoosingRespawn`, `AwaitingRevival`, `Zoning`, and `Disconnected`.
   Immutable offers are separate from life state because a living player may
   receive one. The reducer returns intended commands without touching sockets,
   camera, inventory or rendering. Ten tests cover timing, stale actions, retries,
   instance matching and authoritative arrivals.
3. **Bind transport + lifecycle integration — implemented.** Net/zone and live
   classify forced same-zone handoff, suspend foreground/background movement,
   connect reducer events and command outcomes, and require fresh arrival
   authority. The runtime consumes the movement gate and typed UI action API.
4. **UI — implemented in `death_ui.rs`.** Existing XML `RespawnWnd`,
   `RW_TimeGauge` (EQType 28), `RW_SelectButton`, and `ConfirmationDialogBox`
   Yes/No buttons provide the skin. Recovery rows are explicit because generic
   Listbox layout supplies only framing/text/hit area. Disabled choices capture
   pointer input, action IDs preserve reducer tokens, long lists page, and
   pending controls disable. Four portable hit/layout tests and four inspected
   original-skin GPU captures cover 320×240, ordinary and Retina layouts.
5. **Dedicated live probes.** First forced bind in same zone, then cross-zone;
   subsequently hover bind and rez, decline, corpse looting and interruption.
   No Explorer use, no rule toggling on an occupied test world. A separate test
   zone/ruleset is needed for hover if the main server remains configured false.

## Implemented transport and foreground integration

`ZoneClient` retains a bounded pending bind transfer. A bind destination zero
queues the exact zero-zone `ZoneChange` request and marks forced re-entry. A
successful matching response invokes the existing authenticated world/zone
handoff even if the current zone/instance are unchanged. Ordinary same-zone
border cancellation does not. Duplicate pending bind packets do not enqueue
another request, rejection consumes the intent, and a nonzero destination is
compared by both zone and instance. `Environment` now exposes `instance_id`.
The transition event is delivered before its success reply; foreground zone
state is invalidated before any zeroed reply position could be considered.

Both the background worker and foreground use `MovementAuthority`. Watched
camera poses carry their living owner ID and arrival revision. Own death,
recovery transfer, zone transition, and server relocation invalidate old poses
immediately. The worker's heartbeat checks those tags independently of the
foreground update rate. A stale UI pose cannot be sent for the corpse or under
a new player's ID. A fresh living spawn may reuse an old ID after a full zone
generation change, but cannot reuse the corpse ID for a same-zone hover revive.
Incoming same-name corpse refreshes remain visible without claiming ownership.

`LiveWorld::movement_allowed()` supplies the runtime gate. `camera_position`,
zone crossing, merchants and outgoing gameplay commands enforce it as well;
chat and service cancellation remain available. Own death clears pending camera
arrival, casting, item-use, attack, loot and service interaction state. Carried
inventory, buffs, currency and resources remain server-owned. Bind footer zeros
are ignored. A same-zone hover bind waits for the replacement living spawn,
converts that pose once, retains the old corpse, and resumes without requiring
a second full profile or Ready handshake. A failed recovery transfer requires
reconnection; a pending bind which never yields an arrival uses the existing
30-second zone-request timeout.

Recovery UI dispatch goes through `LiveWorld::recovery_action(RecoveryAction)`.
The pure reducer validates its current generation/revision and returns a single
immutable request. A separate worker command carries that token plus the motion
revision; a newer server recovery event or arrival invalidates an older queued
request before transmission. Send/reject outcomes carry the token back to the
foreground, so a stale failure cannot roll back a newer choice. Raw generic
`Command::Death` dispatch is rejected by `LiveWorld`; callers use the validated
action entry point. Neither a successful send nor a local timeout revives the
player. Living resurrection waits for authoritative relocation; hover recovery
waits for a new living own spawn.

Portable tests exercise forced-zero re-entry versus ordinary cancellation,
instance changes, duplicate/rejected transfer intent, populated and late watched
poses across death, old and new player identities, corpse retention, unchanged
carried inventory/resource values, one-time coordinate conversion, zeroed success
positions, recovery-action duplicate/retry handling, and rejected recovery.
These checks do not substitute for a live death/hover/resurrection probe.

## Live forced-bind verification, 2026-09-29

The guarded `death_smoke` binary exercised a disposable account
`openeq_recovery`, character **Reviver**, in Arena. The character is level 1,
non-GM, with no inventory, money or XP; its account has administrative command
permission. This separates command permission from `GetGM()`, whose death path
would suppress corpse creation. `#kill OWN_ID` calls `Mob::Kill()` and then the
ordinary `Client::Death` path. Current `LeaveNakedCorpses=true` produces a real
empty player corpse, while level 1 avoids experience loss. No server rules were
changed. Explorer, Mechanic and other existing fixtures were not logged in.

The first death reached bind zone zero and exposed an additional protocol bug:
after handing its entity ID to the corpse, EQEmu's `SendLogoutPackets()` emits
`CancelTrade` with player ID 0 and action 7 (`groupActUpdate`). The prior trade
decoder rejected this valid logout notification and disconnected before bind
handoff. The decoder now permits that exact cancellation combination, while
still rejecting zero IDs for trade acceptance/ordinary cancellation. A focused
codec regression checks the valid notification, truncation, and invalid cases.

The complete retry passed through `LiveWorld` and its background worker:

- Own living entity **14** died; movement was frozen and an attack request was
  rejected locally while recovery was pending.
- At `2026-09-29T06:10:38.172999Z`, the wire bind packet reported zone **0**,
  instance 0. At `06:10:38.255283Z`, the accepted response started authenticated
  world/zone handoff to Arena **77/0**, with `forced_reentry=true`.
- A fresh zone environment and Ready arrived, advancing generation 0 → 1.
  Living entity **15** appeared at server `[160,-1009,51.75]`; the old entity
  **14** remained a corpse at the death site. No zero-origin teleport occurred.
- Ordinary outgoing movement after revival was saved by the server at
  `[160,-1001,51.75]`, confirming the new entity could move again.
- Cleanup targeted only Reviver's observed corpse with `#corpse delete`.
  After logout, the fixture's original pose/resources were restored from a
  private snapshot; level, XP, stats, empty inventory, cash and binds matched.
  The fixture was confirmed offline with zero remaining corpses.

Protocol log: `/tmp/openeq-death-live-3.log`. The private configuration is
`~/.config/openeq/storage2-recovery-credentials.json`; credential contents are
never printed. The probe saves a mode-0600 `.restore.sql` alongside its log
before login and refuses nonempty inventory or existing corpses.

```sh
cargo run -p openeq --bin death_smoke -- \
  "$HOME/.config/openeq/storage2-recovery-credentials.json" \
  /tmp/openeq-death-next-run.log
```

Use a new log name for each run so a prior restoration snapshot is never
overwritten. Initial login can populate previously absent bind slots and an
empty currency row; the dedicated fixture was normalized by a normal login
before the successful invariant comparison. A character with no inventory
receives no inventory list, so this probe waits for profile/Ready and verifies
empty inventory independently against the database.

This establishes ordinary same-zone forced bind with a real corpse. Hover
selection, corpse loot, and XP/item-loss restoration remain separate live
scenarios. Living resurrection and cross-zone death are verified below. First-person and
third-person corpse presentation have a portable regression: a dead own entity
is visible at its server pose and never follows the living-player camera pose.

## Separate live resurrection fixture

`resurrection_smoke` uses Reviver plus **Rezzer** on the new
`openeq_resurrector` account. Rezzer is an empty human level-50 cleric, GM flag
off, in Arena 77/0 at server `[150,-1009,51]`. Account command status permits
the fixture to invoke `#castspell 392` (Resurrection) against Reviver's observed
corpse. This enters `Client::SpellFinished` and the ordinary resurrection effect,
`Corpse::CastRezz`, world routing, and `Client::OPRezzAnswer`; it does not construct
an offer on the client or directly mark the corpse resurrected in SQL. Both
characters have no items, money, or XP to recover. No global rules or occupied
zone processes are changed.

The source-backed expectations are:

- `Corpse::CastRezz` reads the corpse's current database state and sends its
  position, spell, caster, owner, and corpse identity through world.
- World allows only one pending offer. A normal decline clears pending state in
  `Client::OPRezzAnswer`, so the caster can send a fresh identical offer.
- Accepting spell 392 with this server's `UseResurrectionSickness=true` sets HP
  to one fifth of maximum, mana to zero, casts sickness 756, marks the corpse
  resurrected, and calls `MovePC` to the corpse. The client grants none of these
  effects locally. The server sends `RezzComplete` through world itself.
- A living player's same-zone resurrection relocates its existing entity without
  another zone generation; the corpse remains independently present.

The probe snapshots both fixtures before login and refuses preexisting items,
corpses, or buffs. It checks decline, stale and duplicate actions, authoritative
relocation/resources, corpse state, and saved post-recovery movement. Cleanup
fades only the test's resurrection sickness through a targeted server command,
deletes only the observed Reviver corpses, logs out, restores original
pose/resources, and verifies both fixtures' level, XP, stats, items, cash, binds,
spell books, spell gems, and buffs. Credentials live in private mode-0600 files;
each run also creates a new mode-0600 restoration snapshot before logging in.

```sh
cargo run -p openeq --bin resurrection_smoke -- \
  "$HOME/.config/openeq/storage2-recovery-credentials.json" \
  "$HOME/.config/openeq/storage2-resurrector-credentials.json" \
  /tmp/openeq-resurrection-next-run.log
```

Source: EQEmu `zone/gm_commands/castspell.cpp`, `zone/corpse.cpp::CastRezz`
and `CompleteResurrection`, `zone/worldserver.cpp::ServerOP_RezzPlayer`,
`zone/client_packet.cpp::Handle_OP_RezzAnswer`,
`zone/client_process.cpp::OPRezzAnswer`, and
`zone/gm_commands/nukebuffs.cpp`.

### Live result, 2026-09-29

`/tmp/openeq-resurrection-live-2.log` records the complete successful cycle:

- Reviver entity 19 died and reentered at bind as living entity 21, generation 1.
- At `06:30:11.050161Z`, a real spell-392 offer arrived from Rezzer for
  `Reviver's corpse19`. The client sent one decline at `06:30:11.057352Z`.
  Decline left the player at bind and the corpse unresurrected.
- A fresh identical offer arrived at `06:30:12.507057Z`, with a new local action
  revision. The old acceptance token and repeated decline/accept clicks were
  rejected. Exactly one acceptance was sent, at `06:30:12.520246Z`.
- Acceptance initially froze outgoing movement without changing position or
  resources. The server then relocated living entity 21, still generation 1,
  to server `[146,-1009,50.541245]`. This is the corpse's fixed ground Z from
  `Corpse::CastRezz`, not the original center Z.
- Authoritative resources changed from HP 36 / mana 0 / endurance 21 to
  HP 7 / mana 0 / endurance 19. Server persistence independently read
  `[7,0,19,146,-1009,50.5412]`; sickness 756 was present. The original corpse
  remained, with `is_rezzed=1` and zero recoverable XP.
- Ordinary post-resurrection movement persisted at `[146,-1001]`.
- Targeted cleanup and logout succeeded. Both fixtures were confirmed offline,
  their original pose/resources restored, invariants unchanged, and zero
  corpses remained.

The first exploratory run reached the same resurrection path but attempted
`#save` before the first post-teleport movement heartbeat. EQEmu's same-zone
`ZoneSolicited` records the destination in `m_ZoneSummonLocation` and sends it to
the client without immediately changing `m_Position`. The probe now allows a
normal movement heartbeat before checking saved coordinates. The exploratory
run also cleaned and restored both fixtures successfully; it did not expose a
client recovery failure.

This verifies ordinary living same-zone acceptance and decline. Hover recovery,
cross-zone resurrection, disconnect/reconnect around acceptance, XP recovery
percentages, and item-bearing corpse loot remain unverified live scenarios.
The empty level-1 fixture intentionally cannot establish XP or item recovery.

The final guarded probe also passed at `/tmp/openeq-resurrection-live-3.log`
(Reviver entity 23 → living entity 24), with the same measured resources and
destination. That run asserted the diagnostic log contained exactly two real
offers and exactly one decline followed by one acceptance; stale and repeated
actions emitted no additional replies. Both fixtures were restored again.

## Live cross-zone death and bind recovery

`death_smoke --cross-zone` temporarily changes **only Reviver's** five bind
slots to North Qeynos 2/0 at server `[-74,428,3]`, after saving their exact values
alongside the existing private pose/resource restoration snapshot. It uses the
same real death in Arena, verifies the different-zone authenticated handoff,
then returns with normal `#zone arena` to inspect and remove the original corpse.
It changes no global rules, restarts no occupied zone, and uses no other account.

```sh
cargo run -p openeq --bin death_smoke -- \
  "$HOME/.config/openeq/storage2-recovery-credentials.json" \
  /tmp/openeq-death-cross-zone-next-run.log --cross-zone
```

`/tmp/openeq-death-cross-zone-1.log` passed on 2026-09-29:

- Arena living entity 25 died at `06:36:31.297923Z`; the bind packet identified
  zone **2/0**, and authenticated handoff began at `06:36:31.381958Z` with
  `forced_reentry=false`. The zero-zone same-zone special case was not involved.
- The deliberately stale camera update supplied while dead emitted no player
  movement. Diagnostic position logs showed no heartbeat after death until
  fresh destination readiness. North Qeynos loaded 104 entities.
- Generation advanced **0 → 1**, with fresh living entity **259** at server
  `[-74,428,3.75]`. Reviver's Arena corpse was absent from the destination entity
  set, and the database retained that corpse at the original Arena location.
- Ordinary movement in North Qeynos persisted at `[-74,436,3.75]`.
- Returning normally to Arena advanced generation **1 → 2**, with living entity
  **26**. The original corpse remained visible at its unchanged source position
  and was removed by targeting that corpse alone.
- After logout, all five original Arena binds, original pose/resources, and
  level/XP/stats/inventory/cash invariants matched the pre-login snapshot. The
  fixture was offline with no remaining corpses. A separate read-only check
  also confirmed Rezzer remained offline and unchanged.

An entity number may legitimately be reused across different zones. The probe
requires fresh generation and a living, correctly named own spawn; it does not
assume globally unique entity numbers. Cross-zone resurrection and hover remain
separate scenarios, as do item loss, corpse loot, and XP restoration.

After adding cross-zone coverage, the default same-zone mode passed again at
`/tmp/openeq-death-same-zone-4.log` (Arena entity 27 → 28), including the stronger
assertion that no position heartbeat was sent between the wire death event and
fresh destination readiness. Its fixture was also restored. Focused validation
passed all 68 network library tests, strict Clippy for both smoke binaries,
formatting, and diff checks. No additional recovery lifecycle changes were
needed by these scenarios.

## Meaningful test plan

The first slice already has portable codec fixtures testing EQEmu's padded
choice packet, exact RoF2 resurrection length, asymmetric XY coordinates,
verbatim accept/decline replies, truncation, malformed counts/duplicate options,
unexpected trailers, nonfinite coordinates, and nonterminated names. A living
spawn → corpse refresh identity regression test covers shared ownership logic.

Implemented portable coverage:

- Own death stops movement immediately, including already-populated watch state;
  unrelated NPC death leaves the player active. Duplicate death is idempotent.
- New own spawn with a new ID revives locally without deleting the old corpse;
  an intervening corpse refresh never restores movement or hijacks ownership.
- Bind zone zero → same-zone success performs authenticated re-entry exactly
  once; ordinary same-zone cancellation does not. Failed/rejected handoff remains
  noninteractive and offers a clear reconnect/error path, not false revival.
- Same zone/different instance is a transfer; same zone/same instance hover
  revival stays on the socket. Position converts exactly once in each case.
- Choices use server IDs, maintain generation, disallow unavailable rez, and
  serialize at most one pending choice. Countdown never sends duplicate timeout
  commands. Unsupported/empty lists fail visibly rather than making up bind data.
- Living and hovering resurrection acceptance choose the correct distinct
  command. Decline, double-click, wrong recipient, stale zone generation,
  replacement offer and reconnect cannot revive locally or send an old reply.
- Death and same-zone revival retain accurate server-sent inventory/buffs;
  no profile resend is required to make a hover recovery succeed.
- UI hit/layout tests cover disabled resurrection, long quest labels, small
  viewports, paging, pending controls and modal click capture. Reducer tests
  cover countdown clamping without client timeout commands.

The ignored `death_ui::tests::capture_original_recovery_dialogs` test can render
the original skin when `OPENEQ_UI_CAPTURE_DIR` and original client assets are
available. The four inspected captures have no layout warnings. Keyboard/chat
focus and Escape handling remain runtime review/smoke checks rather than claims
made by these isolated UI tests.

Live test acceptance must observe server outcomes, using a dedicated recovery
character with recorded bind, level, inventory, cash and XP plus a separate rez
caster. Use intentionally disposable items for item-loss rules. Capture the
packet sequence and verify a new living spawn/accepted zone entry, resource
updates, usable movement/combat after recovery, the old corpse identity, and
loot/inventory persistence after reconnect. Server-side read-only snapshots can
verify XP and corpse resurrection flags that the client does not yet display.
Include disconnects after Death, after selection submission, and after rez
acceptance to establish that recovery is not replayed. Beyond the verified
same-zone forced-bind, cross-zone bind, and living-resurrection cases above, these broader live
scenarios remain open.
