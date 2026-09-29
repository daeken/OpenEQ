# Progression receive data and trainer protocol plan

Source investigation and receive-only implementation, 2026-09-29. EQEmu source revision is
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`; OpenEQ HEAD when inspected was
`13a354b690040468f16ce9fd2c63f28745d79e97` with guild integration in progress.
The original research was read-only. Subsequent guarded fixture validation is
recorded below, separately from source evidence.
[PROGRESSION_UI_PLAN.md](PROGRESSION_UI_PLAN.md) covers presentation.

Current implementation exposes profile training points, languages and absolute
experience, checked incoming skill/language, experience and level events, and
a foreground progression reducer with a received 330-unit XP ratio. It has no
outgoing progression or trainer API. The dedicated Fellowship live proof passed
for real profile data, skill/language changes, invalid-ratio handling and a
restored fresh login. Live level changes and same-total XP commands were deferred
on this fixture because its inherited level10/absolute-XP0 state is inconsistent.
The original audit/proposals below are historical; the completed validation
section records the implemented scope and live limits.

The first useful slice is **profile skills/languages, server skill gains, local
level changes and the server's experience progress bar**. It needs no outgoing
progression command. Trainer purchases remain a later slice: the available
response does not quote current eligibility/costs or remaining training points,
and the completion packet has no trainer entity ID or request ID.

## Original audit: existing OpenEQ behavior

`openeq-net/src/gameplay.rs::PlayerProfile` retains level, a counted `Vec<u32>`
of skills, spells and resources. The profile decoder currently skips:

- Unspent training points: the u32 immediately before mana and current HP.
- Counted language bytes, immediately before zone/instance and position.
- Absolute experience: u64 after guild identity/rank and nine reserved bytes,
  before the eye-height byte and bank currency.

The profile parser already verifies its encoded total size and uses bounded
array reads. Extend those exact cursor positions; do not introduce guessed
absolute offsets in this variable-length packet. The foreground retains the
profile but has no skill/XP update reducer. These progression opcodes currently
fall through the generic gameplay parser.

## Checked incoming packets

Integers below are little-endian. Exact packet lengths come from actual senders
plus registered RoF2 translators; old comments saying XP is 14 bytes are stale.

| Event | RoF2 opcode | Exact bytes | Fields and interpretation |
| --- | --- | ---: | --- |
| Normal XP update | `0x20ed` | 8 | u32 bar units @0; ignore u32 @4 |
| Local level update | `0x1eec` | 12 | u32 new level @0; u32 reported old level @4; u32 XP bar units @8 |
| Skill/language value | `0x004c` | 12 | u32 wire skill ID @0; u32 value @4; opaque four bytes @8 |
| AA progress, separate optional follow-up | `0x7d14` | 12 | u32 AA bar units @0; u16 unspent AA points @4; ignore u16 @6; u8 allocation percentage @8; ignore three bytes @9 |

`rof2_ops.h` registers a SkillUpdate encoder but no ExpUpdate, LevelUpdate or
AAExpUpdate translator; those latter packets use the common structs unchanged.
The SkillUpdate encoder copies the two u32 values and writes the observed bytes
`01 50 88 36`. Consume those bytes without treating the constants as protocol
validation rules. A valid different reserved tail must not disconnect a player.

### Experience is a bar ratio, not an absolute count

`Client::SendZoneInPackets` and `Client::SetEXP` calculate:

```text
bar_units = uint32(330 * (absolute_xp - xp_for_current_level)
                         / (xp_for_next_level - xp_for_current_level))
```

A full ordinary bar is 330, not 100, 1000 or 10000. Display a progress fraction
using that source-backed scale. The second u32 of `ExpUpdate_Struct`, named
`aaxp` in the headers, is **not assigned by either inspected sender**. Ignore
it; AA has its own packet. Do not manufacture an AA bar from that field.

The profile's `WriteUInt64(emu->exp)` is a different quantity: absolute XP at
profile time. There is no ordinary absolute-XP delta in ExpUpdate. Do not
replace the total with the bar value, infer XP-per-level thresholds, calculate
numeric gained XP from a percentage change, or extrapolate a current absolute
total after subsequent bar updates. The server can customize its XP curve.

LevelUpdate has two additional traps:

- `level_old` is set from `m_pp.level2` when nonzero. That field tracks the
  highest level used for training-point grants, so it is **not necessarily the
  immediately previous level** after level loss/recovery. Detect a foreground
  level transition against the previously confirmed current level, not this
  field, and do not derive a training-point gain from it.
- `SetEXP` calls `SetLevel` before assigning the new `m_pp.exp`, and then sends
  a corrective ExpUpdate. The LevelUpdate ratio can therefore be transient or
  out of the ordinary 0–330 range. Preserve the u32 and keep the presentation
  bounded/unknown as appropriate; do not reject an otherwise well-formed
  packet just because the ratio exceeds 330. A subsequent ExpUpdate replaces
  the displayed bar. GM-command SetLevel explicitly sends ratio 0.

Both level and skill values can decrease through server changes. No monotonic
increase assumption belongs in the reducer. The `OP_LevelAppearance=0x3bc9`
packet is a graphic, not a level/XP authority. `OP_SendExpZonein=0x5f8e` already
belongs to zone-entry control and must not be reused as an XP refresh request.

### Skills, languages, and unknown slots

The RoF2 profile writes 100 u32 skill slots and 32 u8 language slots.
The common profile has 28 languages; RoF2 pads four extra bytes with zero.
`common/skills.h` names normal skills 0–77 (`Skill2HPiercing` is 77), but this
is a known ID range, **not a maximum skill value or class-specific cap**.
The additional profile skill slots should remain bounded/raw, not receive
invented names, capabilities or commands.

`Client::SetSkill` updates `m_pp.skills[id]` and sends that same raw value.
The profile encoder also sends `m_pp.skills`. These are base values.
`Client::GetSkill` may add item skill modifiers, so the receive slice must not
label its values effective combat skills. Values 0, 254 and 255 must be retained
without guessing learned/unavailable semantics; the sender explicitly allows
254/255 for server skill resets. A zero alone does not distinguish an unlearned
skill from one unavailable to that character.

Language setters and gains use the **same SkillUpdate opcode** with
`wire_skill_id = 100 + language_id`. Inspected setters accept language IDs
0–27, so those updates are 100–127. Do not index them into the normal skill
vector or classify them as unknown normal skills. Keep the profile's remaining
language padding separate. Unknown future IDs must not resize a vector to an
arbitrary wire index or allocate an unbounded map.

The server skill-gain path (`CheckIncreaseSkill`) calls SetSkill only after its
own cap/rule/chance checks. A received value can update/highlight the row and,
when both old and new values are known, produce a bounded gain notice. Sending
an attack, cast or future skill-use command does not imply a gain. Language
setters already send a formatted skill-improvement message, so avoid duplicate
chat notices if local presentation also marks the row.

Profile training points are a snapshot. No independent current-point-count
update was established. LevelUpdate does not carry the new count; the trainer
confirmation merely tells the original client to decrement its known count.
Keep points labelled last known, or invalidate currentness after relevant
changes until a fresh authoritative snapshot. Do not promise a live spendable
balance from local level arithmetic.

## Proposed receive contract

Suggested module `openeq-net::progression`, following existing checked event
modules, with no `Command` variant in the first slice:

```rust
pub enum ProgressionEvent {
    Experience { bar_units: u32 },
    Level {
        level: u32,
        reported_old_level: u32,
        bar_units: u32,
    },
    SkillValue { wire_skill_id: u32, value: u32 },
}

pub fn parse_packet(opcode: u16, data: &[u8])
    -> Option<Result<ProgressionEvent, ZoneError>>;
```

Wrap as `GameplayEvent::Progression`. Recognized packets require exact length;
malformed packets return a checked error and unknown opcodes stay opaque.
Keep full-width wire values; presentation or profile synchronization must use
checked conversion rather than silently wrapping a u32 level into u8.

Add to the existing `PlayerProfile`:

```rust
pub training_points: u32,
pub languages: Vec<u8>,
pub experience_total: u64,
```

Retain existing bounded profile counts. A reducer can seed from the profile,
apply known skill IDs only to checked in-range slots, and keep normal/language
updates distinct. Unknown IDs may be counted in bounded diagnostics, without
creating arbitrary persistent rows. Display the 0–77 named normal skills;
preserve additional profile slots without pretending they are action buttons.

Scope progression to the connected character and zone generation. On a new
connection/character, clear it. On zoning/disconnection, visibly mark retained
values stale and require the new profile before treating updates as current.
If an early-update buffer is necessary after checking actual packet order,
make it bounded and generation-scoped; never replay a previous character's
skill or XP update into a new profile. Do not emit a gain celebration merely
because a new profile has a higher value than a stale snapshot.

AA can be a later receive-only addition using its separate 12-byte layout.
It is not required to deliver ordinary XP and skills, and no AA purchase or
allocation command is part of this plan.

## Trainer protocol: known records, planned interaction

The active handlers are normal player training handlers despite the `GM`
opcode names. No RoF2 translator is registered for Training/EndTraining/
TrainSkill; the common request structs are the relevant exact wire layouts.

| Direction / purpose | Opcode | Bytes | Layout |
| --- | --- | ---: | --- |
| C→S open, S→C availability reply | `0x1966` | 448 | u32 NPC ID @0, u32 player ID @4, 100 u32 skill entries @8, opaque 40-byte tail @408 |
| C→S end training | `0x4d6b` | 8 | u32 NPC ID @0, u32 player ID @4 |
| C→S train one skill, **deferred** | `0x2a85` | 12 | u16 NPC ID @0; opaque2 @2; u16 bank @4 (0 skill, 1 language); opaque2 @6; u16 skill ID @8; opaque2 @10 |
| S→C training completion | `0x4b64` | 76 | u32 wire skill ID @0; u32 copper cost @4; u8 NewSkill @8; terminated trainer name[64] @9; opaque3 @73 |

The 12-byte training request's header comments repeat the wrong `/*002*/`
offset; packed declarations place bank at4 and skill ID at8. Its NPC ID is
**u16**, unlike the open packet's u32: reject an unrepresentable trainer ID
instead of truncating it when this command is eventually implemented.

`OPGMTraining` copies the request, checks that the NPC is a trainer (classes
20–35), checks class unless `AllowCrossClassTrainers`, and checks squared
interaction distance against `USE_NPC_RANGE2` (40000 in this source).
It overwrites skills **0–77 only** using a cap calculated at configured
`Character:MaxLevel`, adjusted for specialization; non-gnome tinkering is zero.
It overwrites the 40-byte tail with observed constants and sends the copy.
Consequences:

- NPC/player IDs and slots78–99 are echoed request content, not newly
  established server identity or availability. Opening requires a pending
  request tied to the actual local player, trainer and current zone.
- The caps are trainer maxima at the server's configured max level, not the
  character's current-level skill caps. They are not a cost quote, current
  point count, race/class permission table, or proof that a purchase succeeds.
- Client `SkillCaps.txt` cannot silently override the server database/rules or
  specialization/AA restrictions. A shown maximum must say what was reported.
- The reply contains no language-cap bank or separate language availability
  table. Do not fabricate one from the normal skills array.

`OPGMEndTraining` sends an empty `OP_GMEndTrainingResponse` before checking the
NPC. **No mapping for that response appears in the inspected RoF2 config.**
Do not invent an acknowledgement opcode or await a guessed packet to close a
window. Resolve deployed behavior in the later trainer fixture.

### Costs, availability and confirmation limits

`OPGMTrainSkill` checks points, trainer, class/range, bank and skill ID. Normal
skills additionally consult `CanHaveSkill`, current-level `MaxSkill`, initial
training level, trade/research/specialization training limits, and specialization
rules. These depend on server data and rules absent from the opening response.

For an existing skill, the inspected cost formula is integer copper:
`max(raw_skill - 10, 0)^3 / 100`; languages use their existing value similarly.
A newly trained normal skill uses `GetSkillTrainLevel` and the initial cost is
zero. These formulas document this revision, not an authoritative client price
quote or permission to send a transaction. Future client estimation needs
checked arithmetic and must still await server outcomes.

The server sends SetSkill/IncreaseLanguageSkill before the completion packet,
then calls `TakeMoneyFromPP(Cost)` and decrements `m_pp.points`.
`TakeMoneyFromPP` defaults to `update_client=false`, and the training handler
does not check its returned success boolean. A confirmation therefore is not
independent evidence of a newly received authoritative currency/point balance.
Do not enable purchases by depending on permissive server behavior or mutate
foreground money optimistically from the opening response.

Completion contains a trainer **name**, no entity ID, no request token, no
updated skill value and no remaining points. `NewSkill` is set by testing the
resulting value for equality to1, so it is not a universal substitute for
comparing old/new skills. A later mutation UI must allow only one pending
request tied to session/zone/trainer/bank/skill, correlate the matching received
skill value and completion, handle rejection/timeouts, and invalidate on travel,
disconnect, trainer loss or character change. No trainer command should be sent
just because a skill row, hotbutton, or stats window is opened.

## Validation sequence

1. Synthetic profile fixtures exercise variable array counts and distinct
   training points, languages and total XP around adjacent fields, preserving
   guild identity and currency alignment. Include truncation and oversized
   arrays; original client assets are unnecessary for these protocol tests.
2. Exact packet fixtures cover every shorter prefix and extra trailer for8/12
   bytes; reserved tails can vary. Assert ID77 remains normal and100/127 are
   languages, unknown IDs stay bounded, values0/254/255 are preserved, and no
   u32 value silently narrows. XP fixtures cover0,165,330 and out-of-range
   reports, without invented level thresholds.
3. Reducer behavior covers profile→gain→duplicate→decrease; language isolation;
   level loss/recovery with reported-old-level different from previous current
   level; transient LevelUpdate XP followed by correcting ExpUpdate; and stale
   zone/character updates. Opening/filtering the skill UI emits zero commands.
4. Run the narrow dedicated GM fixture below through the server-fixture owner.
   This proves receive/reducer behavior, not ordinary gameplay gain chance.
   A later unmodified gameplay gain can establish the latter separately.
5. Trainer preview/mutation is a separate milestone. First capture a valid
   open/close without purchase, then establish current eligibility, price,
   points and money using a dedicated guarded character. Only after review
   should one explicitly budgeted training transaction validate success,
   rejection, delayed/stale confirmation and exact restoration. No trainer
   purchases or server changes were performed for this document.

## Original proposal: narrow dedicated live proof

Use one disposable/guarded progression character, with no other player target,
no autoattack or cast pending, and the existing foreground/network capture
path. The assigned server agent owns all authentication, source/config checks,
snapshots and restoration; this research performs none of them.

The supported command forms are source-backed in `zone/gm_commands/set.cpp`
and `set/set_{skill,language,exp,level}.cpp`:

- `#set skill <id> <value>` calls SetSkill, clamping the request to the server's
  current MaxSkill. First use one known available normal skill with a recorded
  baseline and room for one point, then restore the exact baseline. Expect
  12-byte `0x004c`, exact observed ID/value, one-row change, and no language
  change. Confirm the server result, not the requested unclamped number.
- `#set language <id> <value>` calls SetLanguageSkill. Use one dedicated
  language with recorded baseline, make one small allowed change, and restore.
  Expect 12-byte `0x004c` with ID100+language and only the language row changing.
- `#set exp exp <absolute_total>` calls SetEXP preserving AA XP. Reapplying the
  recorded absolute total is the narrowest ExpUpdate proof and avoids choosing
  any level threshold. A changed-bar proof is optional only when the fixture
  owner already established safe totals within that character's current level;
  do not derive those from client XP tables. Restore the exact original total.
- A LevelUpdate proof can use `#set level <current_level>` on the dedicated
  character to request a same-level event, then restore absolute XP. This is
  **not a no-op**: command SetLevel resets XP to the level baseline, can grant
  AA, heals/recalculates resources, saves, and its wrapper clamps all skills
  above current caps. It is a separate guarded step, never an automatic fallback
  on a user's existing character. If a complete snapshot/restoration of that
  footprint is unavailable, report level decoding as synthetic/source-proven
  and defer the live level step. An actual level gain/loss test has an even wider
  footprint and is unnecessary for this receive milestone.

Before changes, the fixture owner must save/verify the exact character row,
normal skills, languages, training points/highest-level bookkeeping, total and
AA XP/points, AA allocations and resources touched by the chosen steps. Use
the existing guarded fixture mechanism so unrelated rows and later user edits
cannot be overwritten. After the client is quiescent/disconnected, restore
and compare those values. SetSkill's cap clamp can prevent restoring an
above-cap baseline by command; do not select such a skill, or rely on the
owner's pre-established exact guarded restoration rather than blind commands.

Acceptance records packet sizes, before/after foreground/profile values,
unknown-to-confirmed XP state, language isolation, and unchanged/restored
persistent data. Repeat deliveries must not double-count gains. A subsequent
fresh login should reproduce the restored baseline and contain no stale
fixture progress. No trainer request, training purchase, fabricated XP amount,
or guessed server cap is required for this proof.

## Completed Fellowship live validation, 2026-09-29

`crates/openeq/src/bin/progression_smoke.rs` passed against the deployed RoF2
server on storage2 using only the dedicated Fellowship character (character5,
account `openeq_social1`). The preflight verified identity, account status250,
GM1, offline status and no group, raid, guild, buffs or corpses. No other
character was logged in or changed by this proof.

The received profile had100 normal-skill entries and32 language entries,
level10, training points0 and absolute XP0. SQL and profile agreed on normal
skills0/1/28=55 and language0=100; all other received skill/language entries
were zero. Skill0's server cap was75. The four commands were exactly:

```text
#set skill 0 54
#set skill 0 55
#set language 0 99
#set language 0 100
```

Every command and poll required no current target. The actual incoming
SkillUpdate events changed only normal skill0 or language0 respectively; wire
ID100 updated language0 without touching normal skills. SQL row checks after
each command matched the expected complete skill and language tables. The
foreground retained level10, training-point snapshot0 and absolute-XP
snapshot0 throughout. No movement, trainer, combat, XP or level command was
sent.

The initial ExpUpdate supplied raw bar units **4294966409**. This fixture's
inherited level10/absolute-XP0 combination is inconsistent with the server XP
curve: the zone-in calculation produces a negative ratio cast to u32. The
reducer preserved that received value and returned `None` from
`experience_fraction()`, rather than displaying a fabricated percentage. The
initial profile-to-bar transition and a second fresh login both reproduced
this result. This is live evidence for invalid-ratio handling, not a valid
within-level experience gain.

The same inherited state made the proposed same-total XP command unsafe:
`SetEXP(0, ...)` recalculates level and would delevel Fellowship to1. A
same-level command also resets XP and clamps every above-cap skill; existing
skill28=55 exceeds its current server cap40. Those commands were deliberately
deferred. Level packet decoding, transition semantics and valid XP fractions
remain covered by source review and portable fixtures at this checkpoint;
there is no claimed live LevelUpdate or ordinary gameplay XP/skill gain.

The commands restored the original skill/language values. After disconnect,
the probe independently compared all skills, languages,14 innate AA rows,
training/highest-level/XP/AA bookkeeping, inventory gameplay fields, currency,
binds, spells, buffs, corpses and social membership. The offline fallback
allowed only skill0 at54/55 and language0 at99/100 on the exact dedicated
identity with level10/level2=10, XP0, points0 and AA0. It did not insert/delete
rows or restore whole tables. Pose/resources were restored under the same
identity/offline/progression guards. The complete character and inventory
rows were retained in a private journal; regenerated inventory GUIDs were
excluded only from the gameplay-field comparison.

A fresh connection reproduced the restored profile and foreground, followed
by another disconnect and independent SQL verification: Fellowship offline
in PoK202 instance0 at(1005,-15,389), heading0, HP338/mana0/endurance225,
hunger/thirst6000, skills0/1/28=55, language0=100, level10/level2=10, XP0,
points0, AA0,14 innate AA rows and no group/raid/guild/corpses. The probe exited
successfully. Its evidence files are private (mode0600):

- `/tmp/openeq-progression-foreground-1.log`
- `/tmp/openeq-progression-foreground-1.baseline.txt`
- `/tmp/openeq-progression-foreground-1.restore.sql`

The probe build, targeted strict Clippy and `git diff --check` passed. Two
preflight-only attempts stopped before login: an initially miscounted innate
AA row total and textual opcode padding (`0x04c` versus `0x004c`). The final
probe checks14 AA rows and compares opcode values numerically; no decoder
change or fixture mutation was needed for those preflight corrections.

## Exact source anchors

All EQEmu references below are from revision
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d` in `/Users/daeken/projects/EQEmu`:

- `utils/patches/patch_RoF2.conf:69,103,104,191–197` — mapped opcodes.
- `common/patches/rof2_ops.h:74,120` and `rof2.cpp:1476,3690` — translated
  completion and skill-update packets; other listed progression packets pass
  through common structs.
- `common/eq_packet_structs.h:606–638,1519–1536,2463,5177` and
  `common/patches/rof2_structs.h:811–845,1732–1749,2549–2557` — exact layouts.
- `common/patches/rof2.cpp:2630,2668–2673,2940–2950,2971` — profile points,
  raw skills, padded languages and absolute XP.
- `zone/client.cpp:843–854,2043–2080,3040–3100,4047–4071,12730–12737` —
  initial XP, raw skill/language setters, server gain decisions and effective
  item-modified skill contrast.
- `zone/exp.cpp:803–869,877–970` — level/XP ordering, highest-level field,
  training-point grant bookkeeping and corrective XP.
- `zone/client_process.cpp:1612–1878` — trainer open/close, availability,
  purchase checks, completion, cost and point changes.
- `zone/client.cpp:2773–2785,3176–3215` and `zone/client.h:899` — monetary
  deduction semantics, skill cap sources and default no-currency-update flag.
- `common/skills.h:29–159`, `common/eq_constants.h:684–714`,
  `common/patches/rof2_structs.h:134–137` — known skill/language IDs and packet
  array sizes. These constants do not establish server-specific skill caps.
- `zone/aa.cpp:985–993` — separate AA bar/points/allocation sender.
