# Trading and item use milestone

Started 2026-09-28. Continue toward original-client parity with player trading,
learning spells from scrolls and casting item effects. Preserve server authority
and verify actual item, charge, money and spellbook persistence.

## Player trading

- [x] Document actual RoF2 request, invitation, accept, cancel and finish flows.
- [x] Decode offered items, trade addresses and both players' coin offers.
- [x] Add bounded packet tests and reject malformed or unsupported actions.
- [x] Track partner identity and each side's acceptance independently.
- [x] Invalidate acceptance when an offer changes.
- [x] Keep offered items separate from merchant, bank and item-link views.
- [x] Open player trades from a selected nearby player or an incoming request.
- [x] Render the original trade window, item slots, coin offers and status.
- [x] Offer items and coins through validated UI actions.
- [x] Accept and cancel through server commands without inventing completion.
- [x] Handle partner cancellation, departure and connection loss.
- [x] Prevent overlapping merchant, bank and inventory transactions.
- [x] Verify two-player acceptance, cancellation and reconnect persistence.
- [x] Restore dedicated live fixtures after the checks.

## Scrolls and item effects

- [x] Decode scroll and click-effect metadata from actual serialized items.
- [x] Document scribing/casting packets and authoritative results.
- [x] Add explicit Scribe/Use controls for owned items in inspection.
- [x] Validate current item identity, slot, class, level and spell availability.
- [x] Scribe into an available spellbook slot and show the learned spell.
- [x] Consume a scroll only through confirmed server behavior.
- [x] Cast item effects with server cast time, interruption and charge updates.
- [x] Show item cooldown/rejection feedback without treating submission as success.
- [x] Verify learned spells and remaining items/charges across reconnects.

## Integration and publication

- [x] Add useful controls and commands without breaking ordinary item inspection.
- [x] Check original XML artwork, focus, disabled controls and Retina hit regions.
- [x] Add reducer and command-boundary regressions for stale/invalid actions.
- [x] Run dedicated live and integrated client probes.
- [x] Inspect the native UI and/or GPU captures of the new controls.
- [x] Run appropriate workspace, original-asset/GPU, format and lint checks.
- [x] Document controls, protocol findings and remaining limitations.
- [x] Commit and push the verified milestone to master (`a9feb3e`).

## Separate scope

NPC quest hand-ins, augmentation, raid/guild management, AA activation, spell
particles/audio and interactive account/character creation remain later work.
Only verified implementation is checked off; packet submission alone is not
proof of a completed trade, learned spell or successful item cast.


## Verification record

On 2026-09-28, both the protocol and ordinary-client trade probes completed
item/coin exchanges, two-sided cancellation, acceptance resets, reconnect
persistence, reverse exchange and disconnect refunds. The final client run also
withdrew an invitation before acknowledgement, immediately opened another trade,
and verified that accepting locks item/coin mutations until the offer changes or
ends. Both dedicated trading characters were restored to their original items,
positions and 100 platinum.

Actual scroll and item records verified scribing, reusable effects, shared reuse
timers, interruption without charge loss, charged bag items and stacked potion
consumption. The final client probe clicked rendered original-HUD hit targets
through Interaction and LiveWorld, then reconnected to verify spellbook and
charge persistence. Artificer intentionally retains the learned spells and
consumed fixture state documented in ITEM_USE_PROTOCOL.md.

All 202 workspace tests and all 15 opt-in original-asset/GPU tests passed.
Original-art trade, Scribe and Use captures were visually reviewed, alongside
captures of the live fixture's learned-scroll control, cooldown and remaining
charges. Strict workspace Clippy, formatting and the native client build also passed. No original assets or private connection
credentials are included in this change.
