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
3. **GPU terrain materials.** Replace the compatibility tile bake with direct
   detail/mask sampling, preserving a bounded fallback. Add evidenced normal and
   lighting data, retain water/holes, and measure memory plus CPU/GPU time across
   representative large zones. Do not claim native pixel fidelity for unknown
   ecosystem blending rules.
4. **Character/session lifecycle.** Complete creation, appearance preview,
   camp-to-roster and reconnect; finish hover/cross-zone resurrection and item/XP
   recovery. Bind pending work to session identity and test interruption at each
   boundary. Use dedicated characters only.
5. **Progression controls.** Source-backed trainer quotes/purchases, AA and
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
- Native liquid research remains assigned to npc_assets; cursor/native protocol
  investigation is complete and receiving independent design review. xml_ui now
  owns selected-character appearance preview. Root owns inventory runtime,
  fixture proof and final integration.

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
  `_PutItem`. An isolated EQEmu source regression/fix is in progress; it has not
  been deployed. Track this separately from client queue prediction.
