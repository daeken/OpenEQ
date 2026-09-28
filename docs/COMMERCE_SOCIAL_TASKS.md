# Commerce and social milestone

Started 2026-09-28. Continue toward original-client parity by making NPC services
and cooperative play usable. Features are complete only when integrated into the
client and verified against actual EQEmu behavior.

## Merchants

- [x] Decode merchant session results and serialized item catalog entries.
- [x] Preserve server prices, stock, item identifiers and merchant slot addresses.
- [x] Open and close merchants from the selected NPC.
- [x] Render the original merchant window with icons, prices and selection.
- [x] Select purchase quantities, buy goods and reconcile item/currency results.
- [x] Sell a selected inventory item with quantity and authoritative feedback.
- [x] Handle rejection, insufficient money, stale stock and pending transactions.
- [x] Verify purchase and sale on a dedicated live fixture, including reconnect.

## Banking

- [x] Extend inventory state to personal/shared bank slots and bag interiors.
- [x] Permit bank moves only while within reach of a banker.
- [x] Render personal/shared bank contents and currency using original UI art.
- [x] Deposit, withdraw and move containers without losing their children.
- [x] Support coin transfers if the server supplies enough authoritative state.
- [x] Close service access when moving out of reach, zoning or disconnecting.
- [x] Verify deposited items persist after reconnect, then restore the fixture.

## Quest links and groups

- [x] Decode displayed saylink labels and bounded activation payloads.
- [x] Preserve link identity through chat formatting and wrapping.
- [x] Add clickable link hit regions without blocking ordinary chat editing.
- [x] Activate quest links through the actual server protocol.
- [x] Decode group invitations, membership, leadership and leave/disband updates.
- [x] Invite, accept, decline and leave through commands and UI.
- [x] Render group members, leader and available health information.
- [x] Verify actual two-character group membership and group chat.

## Integration and publication

- [x] Keep original XML artwork, Retina scaling and UI/world focus working.
- [x] Add useful service shortcuts and clearly document the controls.
- [x] Add packet-boundary, invalid-state and reconciliation regressions.
- [x] Run dedicated live probes and integrated client tests.
- [x] Inspect native or GPU captures of each new window.
- [x] Run workspace, original-asset/GPU, lint and format checks.
- [x] Update architecture, launch instructions and exact parity limitations.
- [ ] Commit and push verified changes to master.

## World alignment and Kelethin lifts

- [x] Convert EQEmu coordinates to the original asset axes at the client boundary.
- [x] Keep movement, heading, doors, zone corrections and map landmarks consistent.
- [x] Verify native banker geometry and movement after the coordinate correction.
- [x] Decode inverted lift state consistently on entry and button activation.
- [x] Animate Kelethin lifts from actual server button messages.
- [x] Move lift collision with its visible platform and carry standing passengers.
- [x] Verify Greater Faydark button activation, lift motion and player support, including automatic return.

## Deferred scope

Player-to-player trading, augmentation, raid management, UCS channels, advanced
STML and complete quest-journal behavior remain separate features. No list item
above should be checked based only on packet submission or an unconnected mockup.

## Verification record

On 2026-09-28, the integrated Broker commerce probe bought and sold two rations,
verified personal/shared items and coins across reconnects, then restored them.
Native controls independently verified buying, selling, scrolling and bank
roundtrips. Two dedicated characters verified invitations, decline, membership,
group chat, leadership and leave. Aid Eino’s actual quest link worked both through
the protocol probe and a click on rendered native chat.

The coordinate correction was verified against original PoK bank geometry and
map landmarks, then by standing inside the bank in the native client. The first
native Kelethin ride rose 68 units while carrying the player; longer checks
identified EQEmu’s silent lift reset and added the local return cycle. Exact
proprietary-client lift timing and custom server timers remain limitations.
The final native check rode that complete automatic cycle and started another
ride with the same button. All three lifts passed 2,160 live-event-driven physics
samples, including return and reuse; Broker was restored to PoK afterward.

Workspace checks passed 172 tests; all 14 opt-in original-asset/GPU tests passed.
Strict workspace Clippy, formatting and native build also passed. Focused timer
regressions cover automatic return, repeated open, explicit close and replacement
door data. See COMMERCE_PROTOCOL.md, SOCIAL_PROTOCOL.md and LIFT_PROTOCOL.md for
reproducible probes and the distinction between protocol, physics and native
verification. Private fixture credentials remain outside the repository.
