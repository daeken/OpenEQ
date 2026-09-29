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
3. [ ] UI compatibility and persistence: focus/stacking, saved layouts, screen
   bounds/scaling, missing usable original widgets and rich text.
4. [ ] Authored audio: asset formats, zone emitters, bounded voice lifetimes,
   combat/spell events and user volume controls.
5. [ ] Interactive account flow: connection/server/character selection with
   clear errors and private credential handling.
6. [ ] Social parity: raid/guild state and usable controls, channel coverage.
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

## Active follow-up ownership after 7f3eab3

- **xml_ui**: implement click-to-front/shared map window stacking. Owns
  gameplay_ui/hud/openeq-ui layer metadata, main.rs input/composition hooks,
  ui_layout.rs persistent order (version1 with serde defaults), tests/docs.
  Preserve existing chat event ordering and death gates. No fixture login.
- **eqemu_server**: live resurrection coverage with Reviver and a new disposable
  caster: decline, fresh offer, accept, stale/double clicks, actual relocation,
  corpse state and restoration. Owns live/net recovery code, smoke probe, docs.
  Do not toggle global rules or reboot occupied zones; no existing-fixture use.
- **npc_assets**: authored audio asset parser/index slice only, after research.
  Owns openeq-assets audio module/lib/tests and AUDIO_PLAN.md. Classic84-byte EFF,
  flexible EMT versions, correct sound/MP3 resolution; no playback or client
  runtime/dependency changes yet. Installed EMTs have19–22fields, not only20.
- Agents must not commit/push. Root reviews completed work, resolves integration,
  runs appropriate checks, and publishes the next coherent batch. Read their
  messages/status before starting overlapping work. Main/UI/prefs are now owned
  by xml_ui until it hands them back.
- Root should revisit native sky celestial orientation, thin liquid crossings,
  NPC swimming projection, and complete live hover/rez scenarios later; do not
  confuse first-pass completion with full original-client parity.
