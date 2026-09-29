# Playable client milestone

Started 2026-09-28. The long-term direction is original-client parity; this
milestone delivers and verifies the core interaction loop on Storage2. A box is
checked only when the feature is integrated and its stated check passes.

## Publication

- [x] Preserve the old C# mainline remotely as `codex/legacy-csharp-mainline-2026-09-28`.
- [x] Publish the verified Rust foundation to `master` (`c1f2c55`).
- [x] Publish the completed gameplay milestone with reproducible checks.

## Protocol and authoritative state

- [x] Decode initial character inventory from actual RoF2 item serialization.
- [x] Decode individual item updates, moves, deletion and stack counts.
- [x] Represent worn, general, cursor and bag slots consistently.
- [x] Send validated item moves; reconcile with server responses.
- [x] Receive chat channels and server system/combat messages.
- [x] Send say, tell, group, guild, shout and out-of-character chat.
- [x] Decode combat damage, deaths and animation updates.
- [x] Toggle server autoattack and support target/assist/con actions.
- [x] Decode player health, mana/endurance and relevant profile state.
- [x] Open corpses, receive loot items, loot an item and close the session.
- [x] Handle connection failure without silently accepting actions.
- [x] Add malformed/truncated packet and state-transition regressions.

## UI and input

- [x] Type chat using keyboard text events without moving the character.
- [x] Add Unicode-safe editing, history, send/cancel and channel commands.
- [x] Show bounded, scrollable chat history with channel/combat colors.
- [x] Show original XML inventory and equipment windows with item icons.
- [x] Show bag contents and item details.
- [x] Support click-to-pick-up/place inventory interactions.
- [x] Show cursor item and server-confirmed inventory changes.
- [x] Show and interact with corpse loot.
- [x] Add attack, sit, hail, loot and inventory shortcuts/actions.
- [x] Bind actual player resources and visible combat state.
- [x] Keep mouse/keyboard focus consistent across UI and world controls.
- [x] Verify UI at multiple viewport sizes using original assets.

## Characters and world interaction

- [x] Preserve spawn appearance/equipment and apply wear changes.
- [x] Render held weapons/shields at animated attachment points.
- [x] Support classic armor textures/tints where source assets permit.
- [x] Render attack, hit, death and sit animations from server actions.
- [x] Verify equipped and unequipped characters with real-asset renders.
- [x] Add practical world interaction extensions after the core loop works.

## Live verification and delivery

- [x] Provision a separate development test character and isolated combat fixture.
- [x] Confirm actual initial items and persistence after reconnect.
- [x] Move an item out and back; confirm server inventory and UI state.
- [x] Confirm chat roundtrip through EQEmu.
- [x] Confirm damage/death and loot against a development combat fixture.
- [x] Run the native client and a rendered live gameplay smoke test.
- [x] Run workspace tests, real-asset/GPU checks, formatting and lint checks.
- [x] Update launch instructions, controls, architecture and exact limitations.
- [x] Commit and push the verified milestone to `master`.

## Spells and world interaction

- [x] Parse original spell metadata in both fixed and compact client formats.
- [x] Decode learned spells, memorized gems, refresh timers and buff state.
- [x] Memorize and forget spells with persistence across reconnect.
- [x] Cast from gems or `/cast`; use server duration and actual mana consumption.
- [x] Show casting progress, authoritative cooldown feedback and interruption.
- [x] Display buff icons/durations and dismiss on right-click.
- [x] Hide empty spell bars for characters with no learned or memorized spells.
- [x] Verify beneficial spells, interrupted Gate, offensive damage and spell death.
- [x] Decode original map lines, labels and multiple layers.
- [x] Show player heading, target, zoom and clickable waypoints on the map.
- [x] Add first/third-person toggle with segment-clipped static/door collision.
- [x] Receive and render server doors, books, portals and world props.
- [x] Send door interaction and animate server open/close state.
- [x] Add dynamic door collision and verify blocked/open passage.
- [x] Retain authentication through world/zone handoff despite canceled polls.
- [x] Rebuild scene, collision, actors, doors, atmosphere and map on travel.
- [x] Suppress old-zone movement until a fresh destination position arrives.
- [x] Verify Arena → PoK → Arena through the network probe and native client.
- [x] Decode weighted EQGS/EQGM characters with inverse bind skinning.
- [x] Sample EQGA timed animations and fix quaternion convention.
- [x] Render original gargoyle and male/female Drakkin base models.

## Final review fixes

- [x] Use column-major original item atlas cells and correct RoF2 slot addresses.
- [x] Preserve Unicode chat input and hide raw saylink descriptors.
- [x] Keep logical UI coordinates separate from physical Retina rendering.
- [x] Block world click-through on inspection and other active windows.
- [x] Treat loot response 6 as completed item list, then serialize Loot All.
- [x] Preserve rejected corpse items and stop an unsuccessful Loot All sequence.
- [x] Distinguish single-unit item consumption from click-charge usage.
- [x] Send autoattack-off to the server after own/target death.
- [x] Start spell cooldown on success rather than any spellbar unlock.
- [x] Remove expired buffs from partial server updates.
- [x] Suppress non-damage spell effects from melee-miss messages.
- [x] Clip following cameras against terrain and walls without walking/slide rules.

## Verification record

Storage2 has dedicated **Mechanic** and **Arcanist** fixtures in Arena;
**Explorer** remains the regular PoK development character. Private credentials
are outside the repository. Fixture commands and exact protocol results are in
[GAMEPLAY_PROTOCOL.md](GAMEPLAY_PROTOCOL.md).

The network gameplay probe proved equipment/bag persistence, chat, damage, death
and loot. The spell probe proved buff/cost/dismissal, interruption, damage and
memorization persistence. The integrated client probe additionally checked
`LiveWorld` → reducers → `Interaction` → HUD data and cursor restoration over two
reconnects. Native checks covered inventory pickup/replacement, bags, chat, map,
third person, spellbook, spell gem casting, buffs and round-trip scene travel.

Final verification: 140 workspace tests passed, plus all 11 opt-in original-asset
and GPU tests; strict workspace Clippy, formatting and debug client build passed.
The final moving-NPC render recorded 322 entities, 228 drawn NPCs, 33 server
position changes and 559 frames of motion between packets over 20 seconds.
Original assets remain local. See the README for reproducible commands.

## Following milestones

These are intentionally separate parity milestones:

- [x] Merchant buying/selling and personal/shared banking ([milestone](COMMERCE_SOCIAL_TASKS.md)).
- [x] Clickable quest saylinks and linked item inspection.
- [x] Group membership, invitations, leadership and related window.
- [x] Player trading ([milestone](TRADE_ITEM_USE_TASKS.md)).
- [ ] Augmentation workflows.
- [ ] Complete rich-text/STML rendering and quest journals.
- [ ] Raid membership and management.
- [ ] UCS custom chat channels and guild management.
- [ ] Interactive login, character creation and selection screens.
- [x] Authored classic reference border detection ([milestone](EXPLORATION_COMBAT_TASKS.md)).
- [ ] EQG/absolute-destination border triggers and complete special door/lift behavior.
- [x] Kelethin button/lift cycles, animated collision and passenger carrying.
- [x] Item casting and spell scroll scribing ([milestone](TRADE_ITEM_USE_TASKS.md)).
- [ ] Spell particles/audio and AA casting.
- [ ] Luclin replacements and modern modular armor/hair/equipment assemblies.
- [ ] Full swimming, water rules and invisible collision-volume behavior.
- [ ] Remaining XML widgets, saved window positions and movable stacking order.
- [ ] Complete weather, ecosystem vegetation, terrain water and sky transitions.
- [ ] Character/audio effects, combat feedback polish and broader compatibility.

Original-client parity remains the long-term direction. Completed boxes describe
implemented, verified behavior, not a claim that the whole game is finished.
