# Deep client parity roadmap

User-authorized continuation, 2026-09-29. All seven projects below are in scope;
each ships in bounded, verified slices. Discoveries become explicit work items.
The preceding checkpoint is `f705d17`: 687 workspace tests passed with original
assets/GPU, strict Clippy, format, normal build and playback-disabled checks.

## Execution order

1. **Inventory transactions and NPC hand-ins — active.** Resolve ordered cursor
   ownership and push/refresh ambiguity first; preserve bags, stack counts,
   instance identities, player refunds, loot and reconnect state. Then implement
   four-slot NPC Give with source-backed session/cancellation semantics and real
   quest proof. Acceptance includes server persistence and exact restoration of
   dedicated fixtures. See `QUEST_HANDIN_PLAN.md`.
2. **Newer geometry, collision and liquids — research in parallel.** Complete
   the native terrain-diagonal/region-precedence traces, retain authored metadata,
   then add explicitly supported liquid volumes and border triggers. Verify
   original finite/rotated/overlapping volumes, shores and NPC movement. Keep
   native-client versus generated-server-map differences visible. See
   `EQG_LIQUID_TRANSFORMS.md` and `HEIGHTMAP_FORMAT.md`.
3. **GPU terrain materials — direct sampling and lazy fallback verified.** Replace the compatibility tile bake with direct
   detail/mask sampling, preserving a bounded fallback. Add evidenced normal and
   lighting data, retain water/holes, and measure memory plus CPU/GPU time across
   representative large zones. Do not claim native pixel fidelity for unknown
   ecosystem blending rules.
4. **Character/session lifecycle — creation to fresh roster verified.** Complete
   first-entry coverage and remaining appearance support; finish hover/cross-zone resurrection and item/XP
   recovery. Bind pending work to session identity and test interruption at each
   boundary. Use dedicated characters only.
5. **Progression controls — trainer purchases verified.** Source-backed trainer purchases, AA and
   augmentation. Reconcile server-confirmed points, currency and item outcomes;
   preserve eligibility and stale-action guards. Never infer success from a
   closed window or invent missing authoritative updates.
6. **Original XML UI compatibility.** Broader widgets, rich text, scrolling,
   resizing and skin compatibility. Add controls against concrete authored
   layouts and working gameplay operations. Verify input ownership, window
   stacking, small/Retina layouts and persistence.
7. **Long-session/world reliability.** Repeated travel/reconnect, crowded combat,
   unusual assets, moving-platform edges and resource bounds. Use reproducible
   original-zone routes and performance measurements; fold findings back into
   the relevant milestone.

The order follows the agreed recommendation: cursor/hand-ins first with native
world research alongside it. Work on an independent later slice can continue
while a specific earlier unknown is being researched; unfinished scope remains
listed rather than being treated as complete.

## Working constraints

- Preserve existing work and publish cohesive verified commits to `master`.
- Never log into or alter Explorer. Dedicated fixture mutations need private
  snapshots, guarded offline restoration and a verification record.
- Keep credentials and proprietary assets out of the repository/tool output.
- Follow source and observed protocol; no guessed packets, blind retries of
  transactions, global rule changes or occupied-zone restarts.
- Targeted regressions first; original-asset/GPU checks where relevant, then
  workspace verification before publication. No original audio over speakers.

## Discoveries and implementation record

- Cursor packets alone do not describe a complete authoritative queue: Limbo
  can append or refresh, and RoF+ loot can add tails without serialization.
  Full support must address operation context and unknown identity explicitly.
- Inventory source audit found a second prerequisite: EQEmu treats a zero-count
  item move as a direct swap. OpenEQ incorrectly predicted a stack merge and
  sent zero on ordinary UI drops. Fixed the wire quantity and reducer semantics;
  the dedicated live proof below verifies real server persistence.
- Latest batch ownership: npc_assets handles original-model certification and
  idle transport; xml_ui handles creation controls/outcomes and the cursor
  repair design; cursor_review independently reviews authority and source
  reproductions. Root handles integration, live proofs and publication.

### Inventory stack and scribing prerequisite

- Normal drops onto matching stacks send a positive quantity; zero-count wire
  moves swap full instances even for matching/full stacks. Shift-click splits
  stackable items only. Nonstackable scroll scribing uses a whole-item move and
  still waits for the sent callback before requesting the scribe.
- Regression reproduced before the fix. Added reducer/UI/scribing coverage for
  stack overflow, same-ID instance swaps, nonstackable charges, mismatched
  positive-count moves, pending duplicate clicks and delayed scribe results.
- `inventory_stack_smoke` passed through production `Interaction`/`LiveWorld`
  with the disposable **Barterer** fixture: split one, merge, split three,
  explicitly swap same-ID stacks with count zero, merge, Shift-pick/drop sword,
  logout and normal reconnect. Foreground and read-only SQL agreed at every
  step. No movement, targeting, quest calls or original audio; no Explorer use.
- Private 29-table baseline and observations:
  `/tmp/openeq-inventory-stack-proof`. Fixture is offline. All gameplay tables,
  inventory/currency and ordinary resources/pose match the baseline (normal
  regenerated instance GUIDs and login bookkeeping recorded separately).
  The restoration guard caught Water Flask color `0 -> 0xff000000`: EQEmu
  initializes zero saved tint from the item template and persists it on move
  (`ItemInstance` constructor, `SharedDatabase::GetInventory/SaveInventory`).
  Confirmed the template value, restored only that exact fixture field under
  an offline/full-row guard, then completed the full invariant check.
- **692 workspace tests passed**, zero failures or ignored, with original assets
  and GPU checks. Strict workspace Clippy, formatting, client build and
  playback-disabled all-target check pass. Logs use
  `/tmp/openeq-inventory-stack-`; UI captures are in the matching `ui` directory.

### Cursor reconciliation research

- Source and installed September 19, 2016 native-client inspection found no
  append/refresh discriminator in item type `0x6a`, and no harmless stock
  request for a complete inventory snapshot. The native path replaces cursor
  slot 33 and does not provide a RoF2 queue reconciliation algorithm.
- Silent loot tails, provisional serials after splits, and lore filtering
  remain explicit unknowns. Do not deduplicate by contents or assume the first
  receipt after a move is a refresh. NPC hand-ins remain pending a verified
  queue design; no hand-in commands have been enabled by this batch.
- Independent source review found `InventoryProfile::DeleteItem` removes the
  next queued item when reinserting a partially consumed cursor head through
  `_PutItem`. An isolated EQEmu source regression reproduced two failures;
  the fix passes all 84 utility tests. It has not been deployed. The reviewed
  patch and remaining wire-level issues are in `CURSOR_RECONCILIATION.md`.

### Action lifetime and selected-character preview (verified)

- A reproduced delayed-response bug let a rejected move from the previous zone
  release the new zone's pending inventory action. Gameplay/target commands and
  their callbacks now carry an internal epoch that retires on zone transfer,
  own death and bind/recovery reset. Worker checks stop old queued actions from
  being sent; foreground checks stop old callbacks changing replacement state.
  Ordinary camera corrections retain their valid actions. No wire changes.
- Three regressions cover retired success/rejection callbacks, current callbacks,
  worker death/bind/zone boundaries, permitted death-state chat, same-zone camera
  corrections and target stamps. Targeted live-state tests pass.
- Selected-character Preview decodes appearance from the existing RoF2 roster
  and uses the configured Classic/Luclin models, including Drakkin support.
  Rotation, Back, asynchronous cancellation, stale input, missing assets and
  normal/Retina/compact framing are tested. Eighteen original-asset front/back
  captures passed and were inspected. No login, character mutation or audio.
- Creation, camp-to-roster, remaining appearance material/tint support and full
  cursor reconciliation remain open; see `ACCOUNT_UI.md` for precise limits.

### Native heightmap liquid integration (verified)

- Retained all authored top-level region records and recovered startup height
  anchoring, quantized yaw, registration order and first-containing precedence.
  Unknown/dry winners suppress later water; APV is excluded from generic lookup.
- Enabled the verified group-free DAT20/21 subset. Maiden's Grave passes actual
  original-scene swimming, stationary depth, surfacing and finite side crossing
  at 10/30/120 FPS. Unsupported groups/transforms and binary EQGZ stay explicit.
- Rendering and liquid queries share the same archived/loose ZON/DAT selection.
  This does not claim generated server-map parity or decode unnamed liquid types.

### Combined checkpoint, 2026-09-29

- Published slices: `b67737f` action epochs, `1d3b5bc` roster appearance preview,
  and `4fc44ea` native region metadata/verified liquids.
- **732 workspace tests passed**, zero failures/ignored, including original
  assets and GPU checks. Strict Clippy, formatting, client build and
  playback-disabled all-target check passed. Evidence prefix:
  `/tmp/openeq-deep-parity-checkpoint-`; UI captures in matching `ui` directory.
  An initial run found four capture-output-directory errors; creating that
  directory resolved them, with no source change or suppressed tests.
- The action-epoch live inventory proof again matched foreground and SQL through
  split/merge/swap/nonstackable moves and reconnect. Barterer is offline and the
  private 29-table baseline is restored: `/tmp/openeq-action-epoch-proof`.
- Three additional commerce regressions cover 9 unsent-action retirement cases,
  18 stale-callback cases and 6 dispatched merchant acknowledgment cases.
  Dispatched merchant work remains available for authoritative reconciliation;
  retired unsent work cannot leave coin/buy/sell controls permanently pending.
- Direct GPU terrain material work is next. Existing source textures and masks
  are small enough to share across tiles; the CPU bake remains the fallback
  until bounded GPU validation and original-scene comparisons pass.

### Direct terrain sampling checkpoint

- `8039448` replaces uploaded baked tiles with shared original-resolution detail
  textures and ordered filtered masks for supported heightmap scenes. Whole-set
  admission checks material identity, source completeness, aggregate CPU/GPU
  budgets and device limits; existing baked materials remain the fallback.
- **265 assets/render tests passed**, all original/GPU cases enabled, plus the
  final seven-test terrain suite after optimization; strict Clippy passed.
  Source geometry/collision and existing water remain unchanged. Independent
  negative controls prove the mip/height/tolerance tests detect broken shading.
- Initial full-scene material GPU allocations fell from 548→33MiB in Old
  Commonlands and 437→78MiB in Dead Hills. Preparation/upload fell from
  about 3.5s→0.17s and 3.3s→0.43s respectively. CPU fallback painting still
  dominates asset preparation; its lazy replacement is the next active slice.
- Frame-time samples are noisy and detail sampling adds shader work. Subsequent
  ABBA runs (180 measured frames each) overlap: Feerrott2 G-buffer medians
  baked1.43–3.22ms/direct1.41–2.82ms, OldCommons baked2.73–3.25ms/
  direct2.91–3.48ms. No universal frame-rate improvement is claimed.
- Static native shader research proved coverage/blend/layering images feed CPU
  preprocessing and identified the audited effect's detail-mask/normal-map
  bindings. Full packing, basis and technique selection remain unresolved;
  `GPU_TERRAIN_NATIVE_RESEARCH.md` prevents guessing those semantics.

### Further transaction/lifecycle discoveries

- `90e2b59` records three independently reproduced EQEmu cursor ownership bugs
  with reviewable patches. The final actual utility suite passes93/93. None is
  deployed: cursor persistence/publication and head-only synchronization still
  require separate proof before enabling arbitrary queued hand-ins.
- Trainer open is not universally nonmutating: stock EQEmu may reset invalid
  specialization state. No authoritative harmless cost/eligibility quote exists.
  A bounded normal purchase path is being designed around one pending request,
  exact server confirmation, explicit assessed-cost/balance provenance, and no
  blind retries. See `TRAINER_TRANSACTION_PLAN.md` when that audit lands.
- Character name approval reserves a database row. Creation must handle it as
  a transaction, not an innocuous availability query.
- Camp uses a cancellable normal countdown, but GM Camp can cause an immediate
  peer close. The implementation distinguishes that uncertain disconnect from
  Logout confirmation before trying a fresh authenticated roster connection.

### Lazy terrain loading verification

- Fallback tile painting is deferred until requested; immutable recipes share
  source images and each tile caches its pixels once. The public eager bake
  API, exact fallback pixels and ordinary material fallback remain available.
- Six new CPU tests include independent pre-refactor hashes and concurrent
  cache requests. Complete original asset tests, strict assets Clippy and all
  seven terrain GPU tests pass. Original Old Commonlands loads with 1,552
  unpainted fallback caches.
- Original full-scene asset preparation fell from about 5.6/5.9 seconds to
  0.13/0.12 seconds in Dead Hills/Old Commonlands; their direct and forced-baked
  screenshots match previous PNGs byte-for-byte. Direct GPU preparation remains
  about 0.43/0.17 seconds. These are diagnostic samples, not windowed FPS or
  complete interactive load times. See `GPU_TERRAIN_PLAN.md`.

### Camp live proof and review discoveries

- The production account controller passed normal-player cancel → full 30s
  camp → fresh roster → same-character reentry with Reviver. Barterer passed
  the stock GM early-peer-close path with an explicit uncertain-close notice,
  fresh roster and reentry. Both dedicated fixtures are restored and offline;
  private 29-table journals are under `/tmp/openeq-camp-{reviver,barterer}-proof`.
- Independent review found and fixed queued interruption/start and deadline
  races, held input leaking to the resumed roster, and catalogs consumed on
  first entry. Production-selector regressions cover simultaneous authority,
  cancellation and deadline, plus death/corpse/respawn authority agreement.
- Final rebuilt Reviver proof passed again after revalidation: cancel,30,257ms
  camp, fresh roster, reentry and full offline restoration. Race correctness is
  established by targeted regressions, separately from the uncontested live
  server proof.

### Camp/lazy-terrain combined checkpoint

- `957beca` defers fallback painting; `65b596e` completes the verified camp
  lifecycle; `2ea21d5` adds standalone checked trainer wire/state prerequisites.
- **789 workspace tests passed**, zero failures/ignored, with original assets
  and GPU checks. Strict workspace/all-target Clippy, formatting, client build
  and playback-disabled all-target check passed. Formatting initially found
  only a module-order change, which was corrected before the build checks.
- Verification prefix: `/tmp/openeq-camp-lazy-checkpoint-`; original UI captures
  use the matching `ui` directory. No original audio was played.
- Trainer runtime/UI integration and character creation are next. A read-only
  private Barterer preflight passed for exactly two paid training operations;
  no fixture seeding or training has occurred. See `TRAINER_LIVE_PROOF_PLAN.md`.

### Additional source finding during trainer verification

Pinned EQEmu `common/skill_caps.cpp` declares the skill maximum-level cache as
`std::map<uint8_t, int32_t>` while computing keys as `class_id * 1,000,000 +
skill_id`. Those keys narrow and alias between classes/skills. The actual-source
regression now reproduces incorrect cap results and the separate one-line
32-bit-key patch passes all eight cases. It remains undeployed; no deployed
failure or client-side compensation is claimed. See `SKILL_CAP_CACHE_REVIEW.md`,
which also records distinct reload, ceiling and train-level lookup hazards for
follow-up. The paid Barterer fixture separately checks its actual
class-1/skill-0/level-10 database cap of75.

### Trainer runtime, original-art UI and paid proof

- `/train` and trainer service interaction now open the original training
  window. Skill/language selection, reported maxima, session practice/money
  estimates, assessed costs and explicit uncertainty are connected to checked
  single-use requests. The ordinary Skills window remains receive-only.
- Independent review found and fixed two authority races: skill updates
  overtaking queued purchases, and worker timeouts reaching a stalled UI after
  late receipts. Production regressions cover both, duplicate/stale actions,
  shared currency debits, interrupted purchases and trainer identity/range.
- Barterer completed two paid purchases through production interaction, with
  skill55→56→57, costs911/973 and copper100000→99089→98116 matching SQL. A
  connection loss during the operator pause was followed by a separate
  purchase-free reconnect proving fresh profile57/0/98116. The29-table offline
  restoration passed. See `TRAINER_LIVE_PROOF_PLAN.md` for precise scope.
- Trainer pricing has no stock nonmutating quote; opening may repair invalid
  specialization state. No invented quote or broad specialization claim is
  made. AA and augmentation remain open progression work.

### Trainer/creation foundation checkpoint, September 30

- `5192af6` publishes the verified trainer runtime/UI and paid-proof tooling.
  `455cce1` retains creation catalogs/capabilities and owns the immutable
  approval/create transaction on one world connection. The creation editor is
  still pending; no live name reservation or character creation has occurred.
- Independent creation review reproduced and fixed a stale Enter/Back/Create
  action leaving the foreground stuck after a capability refresh. The worker
  now republishes current selection without executing the old action. Local
  UDP tests cover this alongside cancellation, unknown outcomes and deadlines.
- The full workspace passed831 tests, zero failed/ignored, including original
  assets and GPU checks. After the review fix, the complete changed app suite
  passed414 tests (one more regression), bringing current coverage to832.
  Strict workspace/all-target Clippy, formatting, normal build and the
  playback-disabled all-target check passed on the final source.
- Evidence prefix: `/tmp/openeq-trainer-creation-`; final app/lint/build logs
  include `final-`, and original-art captures are in the matching `ui` folder.
  The dedicated trainer fixture is restored/offline. A separate read-only
  creation-catalog proof left all29 tracked character tables exactly unchanged.
- Next: bind creation choices to the actually loaded model family, add the
  editor, then prove a single explicit creation on a new dedicated test account.
  Cursor authority still blocks arbitrary NPC hand-ins; reviewed server patches
  are stored separately and remain undeployed.

### Idle connection reliability

- A real character-selection pause failed before any approval/create request
  was sent. Client transport now emits the source-backed empty keepalive after
  nine seconds without outgoing traffic. Raw empty KeepAlive/OutboundPing and
  exact negotiated empty encodings are accepted; malformed/trailing payloads
  cannot masquerade as valid control traffic.
- Receive silence and original unacknowledged-send deadlines remain independent
  of these keepalives. Independent review reproduced a zlib trailing-data edge
  case, fixed by exact empty-control framing rather than generic decompression.
- All122 net-library tests passed, including16 stream cases and a180s synthetic
  idle/real local UDP regression. Strict net all-target lint passed. A dedicated
  live empty-roster connection then survived120,020ms without name approval,
  application polling or zone entry before proceeding to its bounded creation
  proof. Evidence: `/tmp/openeq-creation-proof-v2` and
  `/tmp/openeq-idle-transport-tests.log`.

### Further cursor persistence prerequisites

- Five actual-source database-seam cases reproduce successful saves that omit
  the201st root, omit queued-bag children, overwrite a head-bag child with a
  tail root, or ignore initial-clear/child-write failures. The loader's narrow
  slot-dispatch replay also turns surviving head-bag children into loose roots.
- Real save methods/items and the previously patched InventoryProfile object
  run against a fail-closed in-memory SQL seam, with database connection denial
  and an isolated network namespace. No live character or database was touched.
  This is a reproduction, not a deployed fix or full save/reconnect proof.
- `CURSOR_PERSISTENCE_AUDIT.md` records evidence and the reviewable diagnostic.
  The proposed opt-in extension in `OPENEQ_CURSOR_EXTENSION_V1.md` separates
  complete observed ownership from durable storage and keeps arbitrary hand-ins
  pending the required persistence contract and proof.

### Character creation, original-model preview and live roster proof

- The character list now offers a local five-page New flow: identity, origin,
  source-valid stat allocation, supported appearance and full-avatar Review.
  Browsing sends no approval. Only final Create claims the immutable draft on
  its original selection connection. Keyboard/IME, held/stale clicks,
  capability refreshes and compact/Retina layouts are covered.
- Preview authority binds the exact account/socket/catalog/roster/draft and
  chosen class/appearance to the actually drawn model family. Missing diffuse
  textures or silently omitted requested parts cannot approve creation. Existing
  world/roster rendering keeps graceful fallback; provisional drafts can adopt
  supported defaults without claiming a receipt.
- Independent review reproduced and fixed missing Luclin hair being shown bald
  yet approved, stale creation screens hiding connection errors, detached
  results disappearing, and compact pending text overlapping buttons. Unknown
  outcomes require a fresh sign-in before another creation attempt.
- Independent original-asset CPU sweep:2,299 cases,2,210 resolved,89 withheld.
  Each withheld case was compared to default and produced identical geometry
  and materials; the UI cannot promise an unrendered choice. This is a bounded
  appearance sweep, not every cosmetic combination or a live creation matrix.
- The dedicated ordinary **Trailborn** creation passed after120s idle. Exact
  submitted stats/face and starting items/binds/skills/languages matched SQL;
  a fresh authenticated roster returned the same enabled level1 character.
  All30 tracked character tables stayed identical across reconnect. The fixture
  remains offline; no zone entry, other-character actions or audio occurred.
  See `CHARACTER_CREATION_PROOF_PLAN.md` for exact scope and private evidence.
- Follow-up performance question: name/stat changes retire the full preview
  request along with its receipt. Measure editor reload costs before adding an
  immutable model-resource cache keyed separately from draft authority; any
  reuse must still require an exact fresh submission receipt. This is a source
  observation, not a reproduced user-facing slowdown.

### Creation verification checkpoint, September30

- `ae09bde` publishes the creation editor, exact model certification, outcome
  controls and bounded live proof tooling.
- **864 workspace tests passed**, zero failed/ignored, with original assets and
  GPU tests enabled and serialized. Strict workspace/all-target Clippy,
  formatting, normal client build and no-default-feature all-target check pass.
  Evidence prefix: `/tmp/openeq-creation-checkpoint-final-`. An initial run
  omitted `EQ_DIR` for three asset tests; the complete corrected run passed.
- Original-art screenshots cover identity/origin/stats/appearance/review and
  pending/completed/rejected/unknown/detached outcomes at normal, Retina and
  compact dimensions. Representative final captures were inspected; files use
  `/tmp/openeq-creation-checkpoint-{ui,preview,wizard}` and
  `/tmp/openeq-creation-ui`.
- Independently reviewed cursor diagnostic/proposal is published in3bad1b7;
  the idle transport fix is9179f49. Five cursor-save defects reproduce in an
  isolated actual-source seam; none is described as fixed or deployed.
- Next implementation priorities: bounded owned-state capture/negotiation and
  verified NPC hand-ins; lossless cursor storage/caller handling described in
  `CURSOR_PERSISTENCE_REPAIR_PLAN.md`; first-entry verification for Trailborn;
  remaining recovery, terrain, AA/augmentation and broader XML UI work. Account
  registration and unsupported cosmetics remain explicit limits.
