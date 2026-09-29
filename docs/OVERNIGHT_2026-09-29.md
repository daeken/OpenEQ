# Overnight development — September 29, 2026

User request: continue improving OpenEQ overnight, find and fix problems, and
test thoroughly. Work in this task through 08:00 America/Chicago (13:00 UTC),
then checkpoint and report. A thread heartbeat resumes this backlog every
30 minutes until that cutoff. Preserve current work when resuming; do not start
duplicate investigations. User-authorized publication target is `master`.

Starting point: `4f32558`, clean tree, 363 original-asset-inclusive workspace
tests passed, strict Clippy/format/build passed. Live checks proved chat focus
recovery and movement after a Plane of Knowledge arrival.

## Backlog, hardest first

1. [x] Movement and liquids, first pass: original WLD regions, swimming, underwater
   presentation, gravity/levitation, entry/exit and floor/ceiling behavior.
   Root owns movement/main integration; npc_assets owns asset region queries;
   eqemu_server owns protocol/gravity investigation and state decoding.
2. [x] Death and recovery, first pass: own death, respawn choices/bind, corpse state,
   resurrection requests and disconnect cleanup. Live same-zone forced bind is
   verified; live hover/resurrection and item/XP recovery remain follow-up work.
3. [x] UI compatibility and persistence, first pass: click-to-front, matching
   hit/pixel order, saved stacking/positions, screen bounds/scaling. Missing
   original widgets, rich text and movable inspection remain follow-ups.
4. [x] Authored audio, first pass: original metadata and archive index, WAV
   ambience/streamed MP3 music, bounded voices, zone cancellation and saved
   volume commands. XMI synthesis, combat/spell events and graphical controls
   remain follow-ups; listening/native acoustic comparison is still manual.
5. [x] Interactive account flow, first pass: connection/server/character selection with
   clear errors and private credential handling.
6. [x] Social parity, first pass: raid lifecycle/chat and receive-only guild
   roster/MOTD. Guild mutations/refresh, raid administration and UCS remain open.
7. [ ] Quest interaction: safe NPC item/coin hand-ins and server outcomes.
8. [ ] Progression: skills/training/experience feedback and configurable
   hotbuttons, using server-confirmed values.
9. [ ] Broad compatibility sweep: representative WLD, EQG and heightmap zones,
   missing-model/material reports, travel endpoints and collision seams.
10. [ ] Performance/stability follow-up: long-run resource bounds, cancellation,
    reconnect/zone churn, and measurable remaining CPU/GPU bottlenecks.

The list sets priorities, not a promise to finish original-client parity in one
night. Keep completed slices and remaining scope explicit. Add discoveries and
smaller verified fixes under the appropriate item.

## Working rules and verification

- Do not log into or alter Explorer. Use the existing dedicated test fixtures;
  restore intentional fixture changes and record normal server-side changes.
- Keep private credentials outside the repository and tool output. Original
  EverQuest assets remain local and are not committed.
- Verify packet layouts and asset semantics against source or actual fixtures.
  Do not fill unknown fields with guessed behavior.
- Add behavioral regressions for meaningful bugs. Use original-asset queries,
  GPU captures, native interaction and dedicated live checks when relevant.
- After a cohesive slice, run targeted checks, then workspace tests including
  original assets/GPU, strict Clippy, format and client build before publishing.
  Avoid repeating broad tests on unchanged code without a new concern.
  Reproducible full run: create the capture directory, then set `EQ_DIR` and
  `EQ_CLIENT_DIR` to `~/EverQuest`, `OPENEQ_UI_CAPTURE_DIR` to that directory,
  and run `cargo test --workspace -- --include-ignored --test-threads=1`. Leave
  `EQ_UI_DIR` unset: individual test helpers have different directory conventions.
- Commit coherent verified slices and push `master`. Keep this record and the
  existing milestone documentation current; report limitations candidly.

## Completed slices

First verified overnight batch:

Published to `master` as **7f3eab3**. This is the next continuation's baseline.

- Classic WLD water/lava/freezing/opaque-water queries with shared BSP parser;
  synthetic and original-zone fixtures pass. EQG rotation evidence conflicts,
  so those liquid volumes remain explicitly unsupported.
- Fixed-step swimming/levitation/flight with ordinary static/dynamic collision,
  shore exit, underwater camera fog, and original swimming animation selection.
  All 18 movement tests passed including original PoK pool and Kelethin lifts;
  five GPU atmosphere tests passed, including the sky regression. Dedicated live
  swim probe passed; Mechanic's original pose/persistent state restored, offline.
- Death/respawn codecs and same-name corpse ownership guard verified. Pure
  recovery reducer, forced bind transport, movement gating and XML dialogs are
  integrated. A new disposable Reviver proved real forced same-zone bind recovery,
  fresh identity, preserved corpse and usable movement. Reviver is offline with
  original pose/resources restored, invariant data unchanged and no corpse left.
- Saved character/world UI positions and map preferences added by root; atomic
  file replacement, delayed saves and resize handling. Four persistence tests and
  map control/waypoint resize regression pass.
- Final integrated verification: **427 workspace tests passed**, including all
  original-asset/GPU tests, zero ignored/failing; strict Clippy, format and client
  build passed. This includes the live-discovered zero-ID cleanup packet fix and
  independent own-corpse rendering regression. Logs are temporary local artifacts:
  `/tmp/openeq-overnight-final-tests.log`, `/tmp/openeq-swimming-smoke.log`,
  `/tmp/openeq-death-live-3.log`; sky captures `/tmp/pok-sky-before.png` and after.

## Discoveries and next handoff

- Baseline remaining-work lists contain stale entries for features already
  delivered (player trading, Luclin/modular models, item casting and particles).
  Reconcile these while recording new work so the backlog reflects reality.
- PoK pool at scene `[15,1455]`: floor -134, surface -126, server center -131,
  eye -128. WLD agrees with server water map. Do not use center -132 (embedded).
- Storage2 has hover respawn disabled. A bind transfer with zone zero must
  reconnect even when the accepted destination is the current zone. Death must
  suppress the network heartbeat immediately, not wait for UI consumption.
- Core movement rates and underwater palettes are client tuning, not measured
  native-client parity. No guessed breath/damage timers or raw gravity scaling.
- User-reported PoK sky rainbow reproduced and fixed. Native 32×32 DDS color
  tables reserve column31 and rows30/31; native poles use column0. GPU upload
  now excludes auxiliary swatches and collapses pole rings, preserving source
  bytes. Full-zone before/after captures confirm wedge/pinwheel gone. Evidence:
  `SKY_COLORMAPS.md`; native celestial orientation remains a separate follow-up.
- Actual own death sends a zero-player-ID CancelTrade(action7) after the player
  ID becomes zero. Treating that logout cleanup as malformed disconnected the
  client before bind transfer. Narrow compatibility fix is covered by tests.
- Follow-up research is in `UI_LAYOUT_PLAN.md` (click-to-front/persisted stacking)
  and `AUDIO_PLAN.md` (authored sound assets).

## Second overnight batch (verified)

Published to `master` as **71291ec**. This is the continuation's current baseline.

- UI windows and map now share stacking. Left/right click raises the original
  hit owner before chat event consumption; whole-window pixels and hits move
  together. Saved version-1 layouts accept optional order without losing older
  positions. Original-skin normal/Retina GPU captures were inspected:
  `/tmp/openeq-window-stack/window-stack-{before,after}-{1,2}x.png`.
- Live resurrection proved real decline, fresh offer, accept, stale/double
  suppression, server relocation, sickness756, saved resources and resumed
  movement. Reviver and new disposable Rezzer restored/offline, no corpses.
  `/tmp/openeq-resurrection-live-3.log`; no gameplay lifecycle fix was needed.
- Cross-zone death used only Reviver's temporary bind. Arena death → North
  Qeynos authenticated recovery → normal movement/save → Arena return with old
  corpse retained. All five bind rows/pose/resources/invariants restored and
  corpse cleaned; `/tmp/openeq-death-cross-zone-1.log`. Same-zone recovery also
  reran successfully in `/tmp/openeq-death-live-4.log`.
- Audio asset audit loads460 zones/48,017 emitters with bounded diagnostics.
  Parser accepts installed EMT variants and the authored `sounds\\` prefix,
  indexes archive-only PoK/GFay WAVs, and resolves MP3 IDs with correct one-based
  numbering. Negative classic radii and legacy kinds2/3 remain unsupported.
- Dedicated audio service plays confirmed classic ambience/MP3 and EMT WAV/MP3,
  maintains32 effect/two music voices and64MiB effect cache, decodes off the
  render/output threads, and rejects stale zone work. Same-track music regions
  reuse playback; repeats never accumulate catch-up bursts. Volume settings
  persist via `/audio`; `--no-audio` never opens a device. Headless build checked.
- Fourteen runtime audio tests pass, including original decode/service tests,
  scheduling boundaries, starvation regression, stereo underrun, offline mixer,
  and device initialization with **only digital silence**. No original audio
  was played over speakers. Asset tests add11 synthetic and5 original checks.
  Usage, limits and unsupported semantics are in `AUDIO_RUNTIME.md`.
- Combined verification: **466 workspace tests passed**, zero failures/ignored,
  including original assets and GPU tests. Strict workspace Clippy, format and
  client build pass; playback-disabled all-target build also passes. Log:
  `/tmp/openeq-overnight-audio-ui-tests.log`.

## Third overnight batch (verified)

Published to `master` as **08afb0d**. This is the current verified baseline.

- Interactive original-skinned credentials → world list → character list →
  playable world is integrated. No-argument startup opens sign-in; `--login`
  selects an endpoint; private-file `--connect` and explicit offline zones remain.
  Passwords are masked before rendering and never persisted. Atomic preferences
  store only endpoint and last world/character choices. Character creation,
  deletion, 3D preview and camping back to the roster remain follow-ups.
- Account worker retains the same runtime through authentication and gameplay.
  Attempt/revision tokens reject stale and double actions; disabled characters
  and unavailable worlds cannot send entry. Source-backed checked login and
  roster decoders reject malformed lists/replies and show useful rejection text.
- Dedicated Reviver probes passed both >35-second selection pauses, refresh,
  real profile/Ready handoff, continued runtime after controller drop, and cancel
  after EnterWorld. EQEmu ignores Logout before ClientReady: cleanup now finishes
  that bounded handshake before normal logout, including stale queued Ready.
  Reviver was confirmed offline with original pose/resources/invariants restored;
  no movement or gameplay commands. Evidence:
  `/tmp/openeq-account-idle-1.log`, `/tmp/openeq-account-controller-1.log`.
- Account input review reproduced held-key repeats leaking into new chat and
  same-frame repeat/release triggering a shortcut. Native-order handoff filtering
  now suppresses both raw text and physical edges until release, preserves fresh
  presses, and drains releases during loading. Positive regressions cover both
  failures plus focus loss and fresh presses in the same batch.
- Original account-screen GPU captures cover credentials/worlds/characters/busy
  at 1×/2×. Native unauthenticated QA verified Unicode typing, password masking,
  ordinary Tab, Backspace and Exit. Only dummy strings were entered and the QA
  client was closed without authenticating. Synthesized modifier chords were
  inconclusive; pure ordered modifier tests remain the evidence for those.
- Spellbook right-click opens original scrollable SpellDisplayWindow without
  gameplay packets. Actual descriptions use dbstr type6 and the verified compact/
  fixed description-ID fields; landing text stays separate. Unknown dynamic
  values show `?`. Measured text rows drive scroll clamping/thumb placement;
  positions/stacking persist. Original short/long top/bottom captures at both
  scales were inspected. See `UI_WIDGET_FOLLOWUP.md`.
- Combined verification: **499 workspace tests passed**, zero failures/ignored,
  including original assets and GPU tests. Strict workspace Clippy, format,
  client build and playback-disabled all-target check pass. Logs:
  `/tmp/openeq-overnight-account-{tests,clippy,build,headless}.log`.
- XMI research is recorded in `XMI_PLAN.md`: original timing/containers and the
  existing macOS system DLS bank are understood, but EQ selector-to-sequence
  mapping remains unresolved. No synthesis/playback/download was attempted.

## Fourth overnight batch (verified)

Published to `master` as **b1979df**. This is the current verified baseline.

- Source-backed raid events/commands, bounded incremental RaidState and original
  raid UI are integrated. `/raid`, `/raidinvite`, `/raidaccept`, `/raiddecline`,
  `/raidleave`, `/raidleader` and `/rsay` are wired. Decline is local; no guessed
  raid-decline packet. Closing the window does not leave. Subgroup/loot controls,
  leadership abilities and guild mutation remain unsupported.
- Raid rows display received class/level/subgroup/leader data; roster/MOTD
  scrolling, selection, window persistence/stacking, narrow and Retina layouts
  have original-art GPU coverage. Four UI behavior tests and14 captures passed
  under `/tmp/openeq-raid-ui`. No complete-roster or offline-state guesses.
- Independent review caught and fixed delayed invitation status, stale worker
  rejection affecting a newer request, a replacement invite dropping an in-flight
  accept, unconfirmed rosters after travel, and queued commands crossing a
  same-zone raid rebuild. Internal request tokens and a shared server-event
  membership generation now guard transmission and replies. Only acknowledged
  sends start request timeouts. Old roster snapshots remain explicitly unconfirmed
  after travel until the destination sends a rebuild; no invented absence event.
- Chat had the same held-key lifecycle bug as account UI: a submitted key's
  repeat/release could trigger a shortcut, or its repeat could enter newly opened
  chat. Native-order routing now suppresses old-session keys while preserving
  same-session repeats and fresh input. Both core bugs were reproduced before
  fixes; all16 input tests, including four new regressions, pass.
- The dedicated live `raid_smoke` passed invite → local dismiss → reinvite →
  accept, duplicate/stale action rejection, matching two-member rosters, actual
  raid chat delivery in both directions, leadership transfer with SQL agreement,
  reconnect roster rebuild, and both normal self-leaves. Fellowship/Companion
  are offline with exact pose/resources restored and gameplay invariants intact;
  only the recorded fixture raid IDs3003/3004 had leftover metadata cleaned.
  No movement packets, Explorer, or other characters were involved. Evidence:
  `/tmp/openeq-raid-foreground-4.log`; details in `SOCIAL_PARITY_PLAN.md`.
- Live testing corrected two source assumptions: zone-entry action10 uses the
  unchanged136-byte ZoneInSendName structure, while ordinary raid records use
  140 bytes; the decoder now handles that exact case with a regression. EQEmu
  excludes the sender from raid chat broadcasts; the client displays its own
  line only after the worker acknowledges transmission. A regression verifies
  no local line on queue/rejection and no duplicate local echo for other channels.
  Local display is not proof of delivery; the live probe verifies the recipient.
- Normal login/logout regenerates inventory GUID serials. Actual inventory
  fields are compared unchanged; private baseline journals preserve complete
  rows while invariant equality excludes only those serials. Fixture restoration
  is attempted independently for both characters, including on test failure.
- Combined verification: **527 workspace tests passed**, zero failures/ignored,
  including original assets and GPU tests. Strict Clippy, format, client build
  and playback-disabled all-target check passed. Logs:
  `/tmp/openeq-overnight-raid-publish-{tests,clippy,build,headless}.log`.
- `SOCIAL_PARITY_PLAN.md` and `SOCIAL_UI_PLAN.md` document guild receive formats
  and unresolved invitation packet size. Guild controls are still plans, not
  implemented features. `COMPATIBILITY_SWEEP_PLAN.md` records high-value original
  asset gaps: invisible collidable Timorous WLD faces, Feerrott2 tiled water and
  Bloodfields flags/LIT coverage. Original collision is the next priority if
  evidence supports a narrow verified fix.

## Fifth overnight batch (verified)

Published to `master` as **13a354b**. This is the current verified baseline.

- Original hidden WLD collision is being preserved in a separate CPU geometry
  channel. Drawable meshes/materials remain unchanged; missing materials do not
  become fabricated barriers. Static object ownership and transforms, extracted
  object models, dynamic doors/lifts and hidden-only model bounds are covered.
- The Timorous dry barrier fixture and exact source evidence are documented in
  `INVISIBLE_COLLISION_PLAN.md`. Asset tests verify invisible blocking versus
  noncollidable polygons and placement behavior. GPU A/B checks cover invisible
  geometry, unchanged draw/bounds/pixels, original Timorous and moving hidden lifts.
  These targeted CPU/GPU tests pass, including unchanged-pixel original A/B
  captures; independent review's one invalid-area bounds finding is fixed and
  covered. Combined verification: **537 workspace tests passed**, zero failures
  or ignored, including original assets and GPU. Strict Clippy, format, client
  build and playback-disabled all-target check passed. Logs:
  `/tmp/openeq-overnight-collision-{tests,clippy,build,headless}.log`.
- Timorous retains 12,438 extra physical triangles with no oversized global-grid
  fallbacks. Local median build 32.85→44.86 ms, small movement 0.497→1.035 µs,
  10-unit crossing 11.73→30.19 µs. These are bounded fixture measurements, not a
  guarantee for every zone. Details: `INVISIBLE_COLLISION_PLAN.md`.
- Read-only NPC quest hand-in investigation identified cursor-queue and refund
  prerequisites plus server transaction caveats. See `QUEST_HANDIN_PLAN.md`;
  no quest hand-in mutation has been implemented or live-tested. The same
  cursor overwrite affects multi-item player-trade refunds when inventory is full.
  Serial reconciliation after stack splitting remains ambiguous; do not ship
  guessed item-ID/serial deduplication or enable hand-ins before resolving it.

## Sixth overnight batch (verified)

- Receive-only guild directory, profile identity, roster, MOTD and scoped member
  updates are integrated. `/guildwindow` opens the original GuildManagementWnd;
  `/guild` and `/gu` remain chat commands. No guild mutation/refresh/target/UCS
  request API is exposed. Roster reserved bytes never supply identity or logs.
- Bounded reducers reject wrong-guild/stale updates; unknown presence stays
  unknown. Travel preserves visibly stale data until fresh identity, duplicate
  same-ID appearance preserves the current roster, partial additions retain
  absent metadata, and unrelated rename traffic cannot expand the directory.
- UI filters are local and retain unknown presence/alt state. Revision/name hits
  guard selection and note scrolling; rows are cached with shared storage rather
  than cloning a large roster every frame. Window position/stacking persists.
  Six UI CPU tests and22 original-skin normal/Retina/narrow GPU captures passed
  under `/tmp/openeq-guild-ui`; all13 guild CPU tests and strict app Clippy pass.
- Network library95 tests and strict net Clippy pass. A review's possible join
  ordering concern was retracted after tracing all callers: local own appearance
  precedes the world callback's roster and duplicate appearance. Do not add
  speculative roster staging. Dedicated live capture confirmed this order.
- Dedicated `guild_smoke` passed supported isolated guild creation/assignment,
  receive-only profile/directory/roster/MOTD proof against SQL, Companion reconnect
  and supported deletion. Both fixtures are offline with original pose/resources
  restored and inventory/currency/binds/spells/buffs/XP/stats/corpses/group/raid
  invariants intact. All seven guild-related tables returned to their empty
  baseline; no Explorer or movement packets. Evidence:
  `/tmp/openeq-guild-foreground-2.log` plus private baseline/restore journals.
- Run1 stopped because a supported rename rewrote its cached empty MOTD over
  the fixture's SQL MOTD. Cleanup passed; run2 used the server's valid empty
  message without SQL setup. Nonempty MOTD is portable/GPU-tested, not live-proven.
  Creation also exposed an EQEmu bug: `_StoreGuildDB` inserts default tribute
  metadata without assigning the new guild ID, leaving an empty guild0 sentinel.
  Only that exact new row was removed under baseline/identity/offline guards.
  No server patch or broad cleanup was applied. Temporary typed runtime traces
  were removed after proof.
- Combined verification: **562 workspace tests passed**, zero failures/ignored,
  including original assets and GPU captures. Strict workspace Clippy, format,
  client build and playback-disabled all-target check passed. Logs:
  `/tmp/openeq-overnight-guild-{tests,clippy,build,headless}.log`; complete UI
  captures in `/tmp/openeq-overnight-guild-ui`.
- `HEIGHTMAP_WATER_PLAN.md` records lossless DAT water fields and indexed-material
  evidence across six zones. Sixty-five Feerrott2 rectangles are understood as
  stored data, but shoreline clipping has nine counterexamples; surface rendering
  and swimming semantics remain unimplemented pending original-client evidence.

## Active follow-up ownership

- **eqemu_server**: guild live proof and cleanup complete; finalizes probe and
  protocol evidence. Dedicated fixtures are free after root checkpoint.
- **xml_ui**: guild UI handed back; progression UI research complete in
  `PROGRESSION_UI_PLAN.md`.
- **npc_assets**: heightmap water and guild review handed back; progression
  protocol plan complete in `PROGRESSION_PROTOCOL_PLAN.md`, cursor reconciliation
  follow-up read-only.
- Root completed guild combined verification and owns publication.
  Next supported implementation is receive-only
  skills/languages/experience, with original skill names and inventory XP gauge.
- Root should revisit native sky celestial orientation, thin liquid crossings,
  NPC swimming projection, hover/cross-zone resurrection and XP/item recovery;
  do not confuse first-pass completion with full original-client parity.
