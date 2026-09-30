# Trainer transactions: authority, interruption and next fixture

Source audit and trainer implementation, September29–30, against EQEmu
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. The bounded dedicated-character
purchase and restoration proof is recorded in `TRAINER_LIVE_PROOF_PLAN.md`.
No server deployment or server runtime edit was performed.

Read with [PROGRESSION_UI_PLAN.md](PROGRESSION_UI_PLAN.md),
[PROGRESSION_PROTOCOL_PLAN.md](PROGRESSION_PROTOCOL_PLAN.md), and
[REVIVER_PROGRESSION_PROOF_PLAN.md](REVIVER_PROGRESSION_PROOF_PLAN.md).
This adds transaction findings to those plans; it does not reopen their
completed skills/languages/XP work or extend their live-fixture authorization.

**Stock RoF2 trainer traffic cannot provide a fully authoritative, nonmutating
purchase preview.** The open reply contains reported skill maxima, not prices,
remaining practices, current eligibility or a purchase token. More seriously,
opening training can reset specialization skills and save the character.
A usable bounded stock purchase flow is still possible. Missing quotes and
transaction IDs limit its guarantees; they do not block ordinary purchases
with one tracked request, matching receipts and explicit uncertainty handling.
A universally read-only quote request is not established.

## Implemented trainer flow

The current network progression module receives skill/language values, level
and XP updates. The profile supplies a training-point snapshot and currency.
`ProgressionState.profile_training_points` deliberately remains a profile
snapshot; level events do not grant local points. The existing money decoder
handles `OP_MoneyUpdate=0x640c`, but training does not call its sender.
The checked `openeq-net::training` wire API and `openeq::training` session
reducer are connected through the gameplay packet router and
`live::training`. The original-art trainer window opens through explicit
`/train` or trainer service interaction.
The Skills window and command hotbuttons must continue to send no trainer
traffic merely because a row or window opens.

The implemented flow uses a **checked trainer wire API and session reducer**
with production dispatch and UI checks:

- Decode the 448-byte open reply and 76-byte completion separately from the
  receive-only progression reducer. Preserve bounded raw fields; expose only
  reported normal-skill maxima 0–77 as named skills.
- Model one pending trainer operation, stale/ambiguous results and snapshot
  provenance. Observe skill events before identical-value reduction discards
  their receipt information; receipt is not the same as a changed row.
- Permit one explicitly selected normal skill or language to produce one
  stamped purchase command after a matching open reply and current profile
  baseline. A dispatch claim is consumed once. Do not preapply skill, cost or
  points merely because the command was queued or sent.
- Match completion by the selected wire ID (which also identifies its bank),
  expected trainer clean name and current epoch. Apply base values only from
  skill events. Match those observations before completing an ordinary request.
- Maintain a clearly labelled session estimate of practices and carried money
  from the profile and matched completions. Report completion cost as the
  server-assessed cost; it is not independent proof of successful coin debit.
  A cost above known funds makes the balance uncertain instead of wrapping,
  fabricating free success or attempting a refund.
- The original-art trainer flow shows reported maxima beside current base
  values, current session estimates, and price unavailable before Train because
  no quote exists. An eligible explicitly selected row enables Train. Page,
  selection, close and live-state revisions invalidate stale UI hits.
- Runtime sends one claimed request through a separate trainer queue. The
  worker rechecks current action/profile authority, trainer incarnation, class,
  living state and three-dimensional range before transport. Explicit trainer
  open remains a real interaction with the footprint below, not a harmless query.

This is new trainer coverage; the ordinary Skills window remains receive-only.
A dedicated operator-gated production probe is available as described below.

### API, production dispatch and verification

`openeq-net::training` validates outgoing `Selection` bank/ID pairs, builds
`TrainingCommand::{Open,End,Train}`, and parses exact-sized `TrainingEvent`
records. The open array is boxed and preserves all 100 wire values; only the
first 78 become session maxima. Opaque bytes do not become local tokens.

`TrainingState::new(epoch, player_id)` accepts a current profile through
`apply_profile`; `bind_player` handles a profile arriving before the own-spawn
identity. `request_open` and `request_train` return a local `Stamp` containing
epoch, profile revision, operation identity and trainer incarnation. `claim`
revalidates the current trainer and returns the single dispatchable command.
`sent`, `send_failed`, `timeout`, and `cancel_unsent` accept the same stamp;
callbacks for an already completed or replaced operation have no effect.

`preview` provides local eligibility and a clearly labelled source estimate
without creating a pending request. It rejects missing values, normal values
at/above the reported maximum, languages at/above 100, zero practices,
insufficient known carried funds, and price arithmetic outside the server's
signed-int intermediate range. A normal zero-value skill estimates zero cost.
No preview claims current server eligibility or a quoted price.

`observe`, `observe_skill`, `observe_currency`, and `observe_level` consume
actual receipts. Observe skill packets before deduplicating identical values
for display; `pending_value_receipt` preserves such receipts without treating
them as progress. `profile_balances`, `observed_copper`, and `estimate` expose
three distinct forms of evidence. `last_report` retains assessed cost and a
matched received value where available. Successful matching permits the next
explicit purchase immediately, including the same skill. `active_trainer`
includes pending opens for range/despawn guards. `close` returns at most one
End command and never refunds or cancels a dispatched request. `begin_epoch`
invalidates old connection/zone/character data; recovery or level interruption
requires a fresh profile before further purchases.

Offline validation: five wire tests and fifteen reducer tests cover exact
lengths/offsets, opaque bytes, bounded names, full-width incoming IDs/costs,
invalid outgoing banks/languages/NPC IDs, successful sequential purchases,
new skills, language cap 100, same-value receipts, duplicate/mismatched/late
callbacks, current trainer identity, known-funds guards, unaffordable reported
cost, signed-int overflow, concurrent currency changes, close, level/recovery,
epoch changes and profile-before-spawn ordering. These tests do not claim a
server-backed transaction token or atomic persistence.

`live::training::State` maintains a foreground session and a worker replica.
A request includes the current profile and authority revision, trainer
incarnation, known carried amount and a twelve-second deadline. The worker
consumes each operation once after checking current class/range/living state.
The event-priority worker selector handles ready server authority before queued
requests. Profile, currency, level and skill receipts retire a claimed but
undispatched choice; trainer despawn/replacement and own-spawn replacement
likewise invalidate stale choices. `TrainingSent` is published before the next
server event, so an operation retired before that callback is proven unsent.
Explicit worker rejection releases only that matching claim. A worker timeout
publishes the foreground operation stamp before later receipts, even when the
foreground has stopped polling. Local worker counters can differ after an
unsent rejection; an explicit mapping preserves the correct callback identity.
A transport error, timeout or interrupted sent operation remains uncertain and
is never retried.

A matching skill update and completion debit shared gameplay currency exactly
once through `commerce::debit`, using the existing denomination handling. The
practice profile snapshot stays unchanged. Contradictory receipts, a reported
cost above known money, or an interrupted sent purchase make carried money
unavailable to merchant, bank and trade spending until an independent balance
or profile arrives. Practice uncertainty still requires a new profile. Item,
commerce, trade and camp actions are gated while the trainer request is pending.
Current production UI choices are checked again by the runtime, even when a
stale hit reaches the adapter.

Eleven production runtime tests cover foreground/worker authority, sequential
55→56→57 purchases and shared 100000→99089→98116 copper, duplicate sends,
profile snapshots, zero-practice prevention, class/range/trainer replacement,
pre-dispatch skill changes, proven-unsent rejection, sent/unsent death,
conflicting currency, missing receipts, over-funds costs, source clean-name
rules, stalled-foreground late receipts and stale timeout callbacks. Adapter and original-art presentation tests cover the panel separately.

### Operator-gated production probe

`trainer_smoke PRIVATE_CONFIG PRIVATE_PROOF_DIRECTORY` is pinned to Barterer
and a unique living nearby Warlord_Welorf. It requires level 10/class 1, received
skill 0 value 55, two profile practices, 100000 carried copper and all five
specializations at most 50 before opening. It sends no movement, audio, SQL or
combat. The operator owns offline snapshots, fixture seeding and restoration.

The probe opens via production `/train`, selects skill 0 with a current UI
revision, and replays the same Train hit to verify it cannot queue twice. After
the first matched value 56/cost 911 it writes `purchase-1.ready` and waits for
`continue-after-1`; after value 57/cost 973 it writes `purchase-2.ready` and
waits for `continue-after-2`. All four markers must be absent before starting.
Each checkpoint allows the operator to inspect persisted server state before
the next step. The probe then closes through the production UI, logs out
normally, reconnects, and requires a fresh profile containing value 57,
zero practices and 98116 copper, then logs out normally again. The presence of
the probe and synthetic test results do not themselves establish live proof.
The optional `--verify-persistence` mode performs only the final fresh-profile
check and normal logout, allowing an operator to inspect an interrupted proof
without ever repeating its purchases. Verify the dedicated fixture is offline
and at the expected persisted result before invoking that mode.

## Exact stock records and what they establish

All integers are little-endian. Training, end and purchase use the common
packed layouts unchanged; only completion has a registered RoF2 encoder.

| Record | Opcode / size | Fields and authority |
| --- | --- | --- |
| Open request/reply | `0x1966`, 448 bytes | u32 NPC ID @0; u32 player ID @4; 100 u32 entries @8; opaque 40 bytes @408. The server copies the request, overwrites entries 0–77 and the tail, and echoes both IDs and entries 78–99. The echoed IDs are correlation data, not independent identity validation. |
| End request | `0x4d6b`, 8 bytes | u32 NPC ID @0; u32 player ID @4. The server emits empty `OP_GMEndTrainingResponse` before NPC validation; no mapping exists in the pinned RoF2 configuration. No invented acknowledgement belongs in the client. |
| Purchase request | `0x2a85`, 12 bytes | u16 NPC ID @0; opaque 2 bytes @2; u16 bank @4; opaque 2 bytes @6; u16 skill ID @8; opaque 2 bytes @10. Bank 0 is normal skills; bank 1 is languages. There is no price, expected balance, expected skill, sequence or transaction ID. |
| Completion | `0x4b64`, 76 bytes | u32 skill ID @0 (100+language for bank 1); u32 reported copper cost @4; u8 NewSkill @8; trainer name[64] @9; opaque 3 bytes @73. No entity ID, result code, new skill value, remaining points or money balance. |
| Skill value | `0x004c`, 12 bytes | Existing progression event: u32 ID/value plus opaque suffix. A duplicate value can accompany either rejection or a charged language-at-cap operation. |
| Money update | `0x640c`, 16 bytes | Carried platinum/gold/silver/copper, four 32-bit fields. This is an existing independent balance observation, not a training-specific acknowledgement. |

Accept varied opaque bytes; do not use them as a fabricated request token.
Require exact lengths and bounded trainer-name decoding. Reject outgoing NPC
IDs above `u16::MAX`; the open packet's wider ID does not authorize truncation.
Keep future/unknown incoming IDs bounded and unattributed, rather than indexing
arbitrary skill arrays or treating them as pending success.

## Opening can mutate specializations

`Client::OPGMTraining` validates an existing NPC with class 20–35, the player's
class unless `Character:AllowCrossClassTrainers`, and three-dimensional squared
distance no greater than `USE_NPC_RANGE2=40000`. It does not require positive
practice points. It does not establish a server-side training session that
later purchases must reference.

For each skill 0–77, the reply is computed from `MaxSkill(skill, class,
Character:MaxLevel)` through `GetMaxSkillAfterSpecializationRules`; non-gnome
tinkering is explicitly zero. `MaxSkill` reads the server's skill-cap data and
its per-class/skill maximum level. The specialization helper also adds spell,
item and AA `RaiseSkillCap` bonuses and AA `GrantForage`. Thus these are the
reported maxima at this operation, not simply a client file's cap or the
character's current trainable ceiling. Research/tradeskill training limits and
the separate language bank are not supplied in the reply.

**The helper is not a pure cap lookup.** For specialization skills 43–47,
it counts values above 50. If that exceeds one, or two when the effective
`SecondaryForte` AA bonus is present, it calls `SetSkill(...,1)` for all five,
then `Save()`. Each setter persists its skill and queues SkillUpdate. `Save`
also touches the broader character/resource/currency/bind/buff/pet/timer data
already discussed in the Reviver plan. Those updates can precede the open
reply. This can happen even with zero practice points and without a purchase.
The open also causes a trainer greeting.

The prior protocol plan's suggestion to capture open/close without purchase
must therefore include this additional footprint. It is not sufficient to
assert that no purchase packet was sent. A fixture with all five values at
most 50 avoids this branch at its inspected baseline; that is a bounded fixture
condition, not a guarantee for arbitrary characters or concurrent script changes.

## Purchase validation and price

`OPGMTrainSkill` first returns silently for zero points, missing/nontrainer NPC,
wrong class or excessive range. It rechecks the NPC directly; a prior open is
not required. The handlers have no pending quote, replay ID or close/cancel
barrier. Their class/range checks do not constitute a quoted permission grant.

For normal skills, bank 0 accepts IDs 0–77, then checks server `CanHaveSkill`
and current-level `MaxSkill != 0`. Raw value zero takes the initial-training
branch. An existing value is subject to `MaxTrainTradeskills`,
`MaxTrainResearch` or `MaxTrainSpecializations` when applicable, current-level
MaxSkill, and additional specialization restrictions. Defaults are 21, 21 and
50 respectively, but a client must not assume deployed rules match defaults.
Several cap failures send `MORE_SKILLED_THAN_I` and `SetSkill(old_value)` before
returning; the same-value setter still writes the skill row and sends an update.
Other failures have no dedicated response at all.

For an existing normal skill or language value `s`, the source computes integer
copper `max(s - 10, 0)^3 / 100`, truncated down: 10→0, 11→0, 14→0, 15→1,
20→10, 55→911 and 100→7290. A newly trained normal skill has cost zero and
uses `GetSkillTrainLevel` as its resulting value. These are source-derived
estimates until execution, not server quotes; calculate any diagnostic estimate
with checked wide arithmetic and do not copy the server's signed-int overflow
behavior for extreme/custom values.

The pinned `SkillCaps::GetSkillTrainLevel` contains a further source hazard:
its lookup key uses the requested `level` outside a loop over `current_level`.
The key never changes inside that loop. With the usual bounded level and a row
at that exact key, it returns 1 immediately; otherwise it finds no new key.
Do not describe this implementation as a trustworthy first-eligible-level
search or infer an initial trained value from a local class table. This audit
does not fix that server helper.

Languages have different limits from the receive-only catalog: purchases allow
only IDs **0–25**, while the language setter/receive state supports 0–27.
There is no language-cap rejection in this purchase branch.
`IncreaseLanguageSkill` clamps to 100, saves and emits the value even when
already 100. Training a language at 100 can therefore report 100 again while
charging 7290 copper and spending one practice. Never use a cap purchase as a
probe, and do not equate an unchanged value with rejection.

The handler also lacks an `else` rejection for banks other than 0/1: those
values skip the skill branches and fall through to completion and point
decrement. A future client encoder must reject them. Test this as source-backed
unexpected-result handling, not by sending malformed transactions to a server.

## Mutation and persistence ordering

For an accepted normal operation, source execution is:

1. Change `m_pp.skills`/languages, call the corresponding database ReplaceOne,
   and queue SkillUpdate. Persistence failures are not turned into a trainer
   transaction failure.
2. Queue completion with reported cost and trainer clean name. `NewSkill` is
   merely whether the resulting raw value equals 1, not a universal new-skill
   identity or a replacement for the value event.
3. If cost is nonzero, call `TakeMoneyFromPP(Cost)` with its default
   `update_client=false`. It uses carried coins only, makes change across
   denominations, and saves currency on success. Bank, shared-bank and cursor
   coins do not pay. Insufficient carried funds returns false before deduction;
   the trainer handler ignores that return value.
4. Decrement `m_pp.points`. The handler makes no final `Save()` or dedicated
   point-balance update. Points are persisted by later character saves; the
   skill and successful coin deduction have already used separate saves.

Consequently the completion packet is a report from the handler, **not an
atomic durable commit or authoritative debit/balance receipt**. Insufficient
funds does not undo the already-applied skill, suppress completion or avoid the
point decrement. A server failure between separate saves can leave different
durable components. Neither local subtraction of completion cost nor decrement
of profile points fixes this missing authority. Do not rely on the permissive
money path to offer training the player cannot afford. For ordinary completed
requests with sufficient known funds, subtracting the assessed cost and one
practice in a session ledger is a useful source-backed reconciliation. Keep
that derivation distinct from fresh balance packets and persisted server state.

## Interruption, duplicate and retry contract

A future purchase controller must separate local intent, transport submission,
observed skill, completion report and fresh balances. Use a local token plus
connection/zone/profile generation, trainer spawn identity/revision, bank,
skill ID and captured before-value. Only one request may be outstanding.
Trainer names alone cannot distinguish identical trainers or reused spawn IDs.
Register pending intent before dispatch, so a receipt arriving before a local
sent callback is not lost. A `CommandSent` event is not server acceptance.

- Cancel a request before submission only when the owner can establish that
  it was not dispatched. Once submission may have occurred, timeout, close,
  range exit, despawn, zoning, disconnect or character change makes the result
  uncertain; it does not refund anything or prove rejection.
- EndTraining causes no rollback and has no server session to cancel. Closing
  and reopening does not make an earlier late completion safe to attribute.
  An open retry can repeat its specialization side effects as well.
- Never automatically retransmit a purchase as a new application request.
  Each accepted request can increment/spend again. Reliable transport delivery
  mechanics are not an application idempotency token.
- Matching skill value plus completion is useful evidence for the one request,
  but neither contains a transaction ID. Same-value updates alone are not
  success, failure or permission to retry. Duplicate or unexpected completions
  must not decrement local money/practices twice or attach to a newer request.
- After uncertainty, freeze trainer purchases and retain bounded observations
  until a fresh authoritative character snapshot or separately verified server
  reconciliation. A new profile supplies current values, not historical proof
  of which request caused them or which cost was actually deducted.

The wire cannot fully disambiguate delayed results for repeated purchases of
the same skill. No quiet interval, trainer greeting, changed UI revision,
duplicate-value suppression or unrelated MoneyUpdate creates that missing
correlation. These limits belong in the state model, not hidden behind retry.

## Concrete offline fixtures

Use synthetic bytes and production decoders/reducers; no original account,
client login, server credentials or GPU is needed.

| Fixture | Required result |
| --- | --- |
| Open 448 bytes, real pending IDs, distinctive slots 77/78/99, varied opaque tail | Accept exactly 448 bytes; expose at most 78 named maxima; keep echoed padding from establishing eligibility. Reject every shorter prefix and extra bytes. |
| Open response with wrong IDs, old zone/profile generation, closed request or reused spawn identity | Do not mark trainer state current or unlock controls. Specialization value events preceding a valid open still reach progression state. |
| Pure purchase encoding | Exact 12-byte offsets; zero chosen reserved bytes; reject NPC 65536, bank 2, normal ID 78, language IDs 26/27, and any narrowing. Encoding alone sends nothing. |
| Existing normal skill 0: 20→21, completion cost 10, starting points 1/carried copper 20 | After the matched pair, session estimates become points 0/copper 10; original profile snapshots stay 1/20. A later explicit snapshot may independently establish the new balances. |
| Same-value skill update without completion; cap-rejection message | No successful purchase inference, no automatic retry or predicted debit. |
| Language 0 already 100, update 100 plus completion cost 7290 | Accept a completion observation despite unchanged progression revision; do not classify unchanged value as a free rejection. |
| Completion before local sent notification; value/completion delayed, duplicated or unexpectedly ordered | Keep one bounded request record; no duplicate mutation, reentrant send or reuse of a later request. |
| Timeout after submission, then close/reopen and late same-skill completion | Remain uncertain; send no retry or refund. Profile replacement clears old request correlation. |
| Profile points 1 followed by level update; unrelated currency update | Neither event supplies a new practice count or matches a trainer transaction. |
| Cost estimate boundaries and large/custom skill values | Check integer rounding with independent known values; overflow is unavailable, never wrapped into a cheap price. |

For the usable slice, the normal 20→21 fixture should reconcile **session
estimates** to points 0/copper 10 after the matching value and completion;
the original profile snapshots remain 1/20. Neither estimate is relabelled as
a new authoritative balance packet. Cost above known funds, an unexplained
currency change while pending, or level/recovery interruption invalidates
estimates and disables further purchases until a fresh profile. A language
already at 100 is rejected locally before constructing a purchase.

A separate isolated server harness can later exercise the **actual** Client
handlers and persistence ordering. A concrete paid fixture is a human warrior
at a verified level whose skill-0 cap exceeds 20, raw skill 0 exactly 20,
all five specializations at most 50, one practice, twenty carried copper,
no other currency, and one class-20 trainer with a u16 ID in the same zone
within range. Freeze other skill/money/level changes. One purchase should emit
skill 0=21 then cost 10 completion, leave carried copper 10 and in-memory
points 0, and emit no training MoneyUpdate. Compare the actual separate
persistence calls and a subsequent ordinary save/reload, not a mocked copy of
the transaction algorithm.

Use additional disposable harness cases for no points, current cap rejection,
insufficient money, language at 100, duplicated requests and the specialization
reset on open. Injected interruption between actual persistence calls should
make the missing atomicity visible. Do not run these on a deployed character.

Fellowship and Reviver both have documented zero-practice baselines. Their
earlier authorization covers neither point/currency seeding nor trainer travel,
opening, spending or restoration of new skill rows. Any eventual live fixture
needs its own concrete identity, current source/rules/content validation,
bounded mutation budget and guarded offline restoration recipe. No such live
execution or authorization is claimed by this document.

## What would enable an authoritative trainer purchase flow

For stock compatibility, label maxima and price estimates honestly and retain
the limitations above; this does not meet a fully authoritative quote contract.
For that stronger contract, a separately reviewed server feature must provide
a pure eligibility/price/balance query, authoritative skill/points/currency
result, server-checked preconditions and idempotent request/result correlation.
It must separate specialization repair from preview, validate bank/language
caps, check money before applying the skill, and define persistence failure
semantics. No opcode, extension negotiation or deployment is specified here.

## Source anchors

All EQEmu anchors are at the pinned revision:

- `zone/client_packet.cpp:6630,7043,7055`: exact request-length gates.
- `zone/client_process.cpp:1612–1699`: open/end validation, echoed fields,
  maximum loop and greetings; `1701–1874`: purchase branches and ordering.
- `zone/client.cpp:2043–2080`: immediate skill/language saves and updates;
  `2773–2856`: carried-money deduction and unchecked failure;
  `2975–2986`: independent MoneyUpdate sender.
- `zone/client.cpp:3176–3306`: server skill caps and specialization repair;
  `common/skill_caps.cpp:23–89`: cap lookup and constant-key training-level loop.
- `zone/client.cpp:981–1107`, `zone/zonedb.cpp:919,951,1112`: broader Save,
  individual skill/language persistence and persisted point field.
- `zone/exp.cpp:903–910`: training-point grants are highest-level bookkeeping,
  not a point count carried by LevelUpdate.
- `common/eq_packet_structs.h:606–638,3804`,
  `common/patches/rof2_structs.h:811–845,3648`,
  `common/patches/rof2.cpp:1476–1488`, `rof2_ops.h:74`,
  `utils/patches/patch_RoF2.conf:191–197,453`: layouts, translation and mappings.
- `common/features.h:215`, `common/classes.h:44–59`,
  `common/ruletypes.h:174,275–280`: range, trainer classes and training rules.

OpenEQ integration anchors are `crates/openeq-net/src/{progression,gameplay}.rs`,
`crates/openeq/src/{progression,game,live}.rs`, and the existing progression UI
adapter. The original control inventory is already in PROGRESSION_UI_PLAN.
