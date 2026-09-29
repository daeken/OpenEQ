# NPC item and coin hand-ins: source-backed implementation plan

Research date: 2026-09-29. This is a plan, not a claim that OpenEQ supports NPC
hand-ins. This task read the backlog, current client, EQEmu source and selected
deployed rules/quests. It performed **no logins, fixture mutations, NPC spawns,
quest edits, server configuration changes, or runtime code changes**.

EQEmu source: `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`, sibling checkout
`/Users/daeken/projects/EQEmu`. Current client starting point is the account/raid
work following `08afb0d`. Read alongside `OVERNIGHT_2026-09-29.md`,
`NEXT_TEN_MILESTONES.md`, `TRADE_PROTOCOL.md`, and `TRADE_ITEM_USE_TASKS.md`.
The backlog requires real PEQ quest proof, retained returned items, cancellation,
and honest uncertain outcomes. Existing player-trade proof is useful but does
not prove NPC hand-ins or a multi-item cursor queue.

**Recommended next implementation: ordered cursor support first.** NPC hand-ins
reuse existing checked trade/item/coin codecs, but require a distinct session
kind, four-slot limit, NPC cancellation behavior, no-drop policy, and outcomes
that remain server-driven. Do not enable NPC targets merely by removing the
current `!spawn.npc` checks.

## Current client coverage and gaps

| Area | Already implemented | Required difference / gap |
| --- | --- | --- |
| Wire transport | `openeq-net/src/trade.rs`: request/ack/accept/cancel/close/coin; `gameplay.rs`: MoveItem, ItemPacket, currency | No new guessed hand-in opcode. Existing layouts serve both kinds, but received cancellation identity differs. |
| Start | `Interaction::trade_request`, `LiveWorld::trade_command_allowed`, `trade_player_available` bind a nearby player ID | NPC request auto-starts on server and returns ACK. Preserve player invitations; bind NPC identity/type at request time and await its ACK. |
| Escrow | Cursor-only whole-item moves into empty trade addresses; sent callback predicts ownership transfer once | NPCs consume **only slots0–3**, not player slots0–7. Enforce at UI and command boundary. |
| No-drop | `game.rs::Inventory::validate_move` recursively rejects no-drop/attuned items in player escrow | Real quest items can be no-drop. Permission must depend on a verified active NPC session; never weaken player/shared-bank validation globally. |
| Currency | `TradeCommand::OfferCoin`, bounded positive amounts, sent-callback debit, full received balance replaces prediction | NPC has no partner coin display or offer echo. Preserve exact denomination buckets and refund headroom. |
| Acceptance | Separate player accept flags; freeze modifications after own acceptance | Give is one-sided. Freeze on queued Give, await server finish/results; do not wait for NPC acceptance or claim quest success. |
| Cancellation | Player cancel waits for local close and one reciprocal cancel, with invitation-withdrawal barrier | NPC cancel echoes the **NPC ID**, then refunds and both close packets; there is no reciprocal player. Current own-ID match/barrier would remain stuck. |
| Item receipt | Item type0x65 is isolated as remote view; 0x67/0x6a enter owned inventory | `Inventory::insert` replaces cursor33. Several 0x6a pushes currently overwrite the visible head. Preserve an ordered queue and bag trees. |
| Close | `Finished` clears only kind3 escrow, preserving actual refund/delivery slots | For NPCs FinishTrade precedes script/result packets. Closing is no success acknowledgment and must not discard later returns. |
| UI | Original `TradeWnd` with eight own/partner slots | Original `GiveWnd` already supplies four own slots, coins, Give/Cancel. Do not show fictitious partner acceptance or rewards. |
| Safety/lifecycle | Pending-operation guards; commerce/cast/loot/trade exclusion; departure/death cleanup | Correlate queued/sent/rejected work to the current NPC/session/zone. Existing trade commands carry no server transaction ID. |

Useful current files: `crates/openeq/src/{trade.rs,trade_interaction.rs,game.rs,
live.rs}`, `crates/openeq-net/src/{trade.rs,gameplay.rs,inventory.rs}`. Inventory
prediction occurs in `LiveWorld::command_sent`; it must remain one operation per
acknowledged transmission and must not become a second receipt path.

## Exact reusable RoF2 packets

Read-only comparison with `/srv/eqemu/patch_RoF2.conf` confirmed the following
deployed trade/item opcodes. MoneyUpdate0x640c is source-confirmed in the same
RoF2 table and already decoded by the client. All listed integers are LE.
Recheck the deployed mapping immediately before any future live fixture.

| Direction | Opcode | Exact record and NPC meaning |
| --- | --- | --- |
| C→S | TradeRequest0x77b5 | 8 bytes: target entity ID u32@0, own entity ID u32@4. NPC target is the current spawn ID, not its database NPC type ID. |
| S→C | TradeRequestAck0x14bf | 8 bytes: own ID@0, NPC entity ID@4. Server already called `trade->Start` before sending it. No client ACK is required. |
| C→S | MoveItem0x32ee | 28 bytes: from InventorySlot@0, to InventorySlot@12, count u32@24. Each slot is six u16s: type, reserved0, main slot, subslot or0xffff, augment or0xffff, reserved0. Cursor is type0/main33; hand-in is type3/main0–3, no subslot/augment. Offer whole cursor with count0. |
| C→S | MoveCoin0x0bcf | 20 bytes: from location1, to location3, source denomination, same destination denomination, amount, each u32. Copper0/silver1/gold2/platinum3. Require0<amount≤i32::MAX and sufficient current carried funds. |
| C→S | TradeAcceptClick0x69e2 | 8 bytes, own ID@0, unused zero@4, as existing checked encoder. The handler acts on the caller's current trade object; a stale Give can act on a different session. |
| S→C | FinishTrade0x3993 | Empty. NPC handler queues this **before** calling FinishTrade(NPC), tasks, quest script, rewards or returns. It only ends the give transaction. |
| C→S | CancelTrade0x354c | 8 bytes, own ID@0, action0@4. |
| S→C | CancelTrade0x354c | NPC branch copies request but rewrites ID@0 to the **NPC ID** and echoes it to the caller. This is distinct from player cancel's recipient/own ID. Match the recorded session; do not respond recursively. |
| S→C | FinishWindow0x7349; FinishWindow2=0x40ef | Empty, queued after cancellation refund and reset. Handle idempotently; no guessed NPC peer response. |
| S→C | ItemPacket0x368e | u32 packet type followed by existing bounded recursive RoF2 item serialization. Actual returned/reward item, count, instance, slot and bag contents are authoritative. |
| S→C | MoneyUpdate0x640c | 16 bytes: platinum, gold, silver, copper u32. Replace carried balances; do not add the same refund twice. |

TradeCoins0x4206 is a12-byte delta to the other **player**: local recipient ID,
denomination u8/reserved3, amount u32. `OPMoveCoin` only sends it when the partner
is a client. NPC coins receive no immediate self echo. TradeBusy0x5505 remains
the existing player invitation path, not an invented NPC decline. The inspected
handlers do not emit TradeMoneyUpdate0x68c2; its existence in a table is not a
reason to add a guessed packet.

Wire sources: `common/patches/rof2_structs.h:1822,2698,2704,2717`;
`common/patches/rof2.cpp::DECODE(OP_MoveItem)`;
`common/patches/rof2_limits.h:105,174` (TRADE_NPC_SIZE4, canonical3000–3003);
`zone/client_packet.cpp:4317,15428,15675`;
`zone/client_process.cpp::OPMoveCoin` (1280–1620).

## Server transaction and return semantics

### Starting and offering

`Handle_OP_TradeRequest` ignores body type11 and only auto-starts an NPC/bot if
it is not engaged. The inspected handler does not implement the client's
proximity or idle-action policy, and it need not reply when it refuses. Keep a
bounded waiting state and a cancel/reconnect path; do not treat silence as ACK.
`Trade::Start` resets counters and can start the other mob's trade object.
Resending requests against active escrow is unsafe. No request nonce or unique
transaction identifier is carried on wire.

`Inventory::SwapItem` rejects a noncursor source into trade by kicking the
client. `Trade::AddEntity` rejects occupied destinations; its nonzero count path
is stack-on-existing behavior with additional kick cases. Initial hand-ins
should offer only whole cursor items into empty NPC slots. Split a stack onto
the cursor first through normal inventory rules. Do not offer into slots4–7:
the broad MoveItem handler accepts player trade addresses through3007, while
`FinishTrade(NPC)` only pops3000–3003.

Player-only no-drop enforcement in `zone/inventory.cpp:1868` deliberately
excludes ordinary NPC targets. An NPC can legitimately consume a no-drop quest
item; that does not authorize bound items to players/banks. Start with ordinary
noncontainer quest items and stacks. Bags, augments, attuned/evolving items,
pet equipment and bot trades need their own proofs; the generic NPC bool is
not evidence of quest semantics. Current Spawn exposes body_type/npc/corpse,
but not an explicit quest capability or pet/bot ownership identity.

Coins debit and save the player's carried bucket on receipt of MoveCoin.
Amounts can be clamped by the server; do not intentionally test excessive
amounts. A missing destination can still lose the source money: enforce active
ACKed session and same current target before queue/transmission. Trade coin
withdrawal is unsupported; cancel refunds. Preserve the existing signed32-bit
bucket bounds because AddMoneyToPP silently omits an overflowing credit.

### Give and quest outcomes

`Handle_OP_TradeAcceptClick` queues FinishTrade first, then calls
`Client::FinishTrade(NPC)` and resets trade. It does not provide an atomic quest
success/failure result. The NPC branch in `zone/trading.cpp:508–687`:

1. Pops the four item instances out of inventory and saves each trade slot
   empty before calling scripts. This is not a database transaction that the
   client can roll back by resending a packet.
2. Runs `UpdateTasksOnDeliver` first if enabled. Task delivery/cash progress may
   change independently of the NPC quest's visible reward. No-quest pet hand-ins
   can equip/consume items, including items taken from bags.
3. Initializes the NPC hand-in bucket, invokes EVENT_TRADE if the NPC has a
   handler and does not have aggro on this client. Perl exposes this as
   EVENT_ITEM; Lua uses event_trade.
4. If the script has not processed returns, initializes any missing bucket and
   calls ReturnHandinItems when Items:AlwaysReturnHandins is true. It finally
   resets the hand-in bucket and deletes the owned original instances.

Deployed `plugins/check_handin.pl` delegates to NPC::CheckHandin;
its `plugin::return_items` body is empty, relying on the server fallback.
Deployed `lua_modules/items.lua::return_items` calls NPC::ReturnHandinItems
explicitly. Do not copy an old plugin assumption into client logic.

`NPC::CheckHandin` aggregates required counts across slots; stack count is
charges, nonstackable items count as one. It can consume only the required
portion of a stack and retain extras for return. Money requirements compare
**each denomination for equality**, not total copper value or “at least”.
Multiquest can retain a bucket across hand-ins/players; do not use such an NPC
as an isolated fixture. Quest code can change XP, factions, tasks, globals,
spells, inventory or character state; the client must not infer these from
offered IDs or locally execute the quest.

`NPC::ReturnHandinItems` pushes remaining item clones to the cursor, adjusting
stack charges to remaining count. It returns unconsumed money with
AddMoneyToPP(...,true). It guards against script-external item/money returns to
avoid duplicates. External item guard is coarse: a matching returned item can
suppress the bucket's automatic returns; therefore “AlwaysReturnHandins=true”
is not a universal refund guarantee. Multiquest items may intentionally remain
with the NPC. Preserve the actual results and chat; do not fabricate refunds.

Source concern requiring separate server review: in
`zone/npc.cpp:4353–4420`, matching item counts are decremented before combining
`money_met && items_met`. The rollback restores counts only when `items_met`
fails. When items match but money does not, a previously initialized bucket can
retain decremented counts despite a failed overall check. A matching stack plus
wrong denomination/amount could consequently receive a reduced/zero-count
return. This is a source finding, **not a live reproduction**. Keep that mixed
case out of the first fixture; do not hide it with client-side compensation.

### Cancel, disappearance and disconnect

For an existing NPC partner, CancelTrade echoes the NPC ID, calls
`FinishTrade(this)` to refund self, resets the caller's trade, then sends both
empty window-close messages. It does not call the NPC quest. The refund uses
the same general placement/stacking/cursor path as player-trade cancellation.
No reciprocal NPC cancel is expected; the existing player barrier cannot be
reused unchanged. Cancel-before-ACK may have no partner at handler time and
still sends closes. A late ACK must not unlock a new session; scope it and its
cleanup to the old attempt.

`Client::OnDisconnect` (`zone/client_process.cpp:714`) refunds self if
`trade->With()` still resolves, refunds the peer only if a client, then resets
both trade objects. If the NPC has vanished and With() returns null, the
inspected cancel/disconnect paths do not invoke that refund. Do not promise
refund on despawn or guess a return slot; require authoritative reconnect and
preserve unresolved escrow evidence. This edge needs a separately guarded test
after ordinary cancellation passes, not a first live destructive scenario.

## Ordered cursor prerequisite

EQEmu's cursor is a queue, with its **head** exposed at canonical slot33. It is
not an inventory map with several independently movable cursor addresses.
The current OpenEQ map can represent only one head and therefore loses earlier
visible data when later returns arrive. Fixing this benefits player trades too.

| Server action | Actual packet / ordering | Client requirement |
| --- | --- | --- |
| `InventoryProfile::PushCursor` | Appends a clone to queue tail; `ItemInstQueue::push` uses push_back | Preserve tail order; do not replace current head. |
| `Client::PushItemOnCursor(inst,true)` | Pushes, then ItemPacketLimbo0x6a at cursor33 for that appended instance; saves complete cursor | Several returns can all have the same slot field. Preserve item trees and counts independently. |
| Zone-entry inventory | `BulkSendInventoryItems` includes head; client_packet.cpp:1793 iterates later queue entries and sends each as0x6a/cursor33 | Replace inventory baseline/head once, then rebuild tails in wire order. No queue inferred from database bag addresses. |
| Cursor→empty slot or fully depleted stacking move | `SwapItem` calls `SendCursorBuffer` when `dstitemid==0 || all_to_stack`, emitting0x6a for the **current head**, if any | This is a head refresh, not an additional tail push. Full removal exposes the next item; a partial split to an empty destination instead refreshes the remaining original head. |
| Cursor→empty trade slot | `Trade::AddEntity` consumes cursor; inventory.cpp:2004–2007 calls SendCursorBuffer | Same next-head refresh, without a normal self MoveItem echo. Retain unoffered tails. |
| Partial cursor stack move | Source retains remaining charges. Empty destination still triggers the same0x6a head refresh; a partial merge into an occupied stack does not | Retain current head with new count; do not advance or append it again. |
| Cursor swap with an occupied ordinary slot | `_PutItem(cursor,inst)` replaces current head via pop/push_front while retaining tail | This is not append. Queue update must mirror the sent move once. |
| Delete/deplete cursor | Inventory deletion pops/reduces head; server paths can expose next head | Correlate mutation and received item state; preserve tail, avoid double deletion/advance. |
| Refund to a normal possession/bag slot | ItemPacketTrade0x67 with destination | Replace that actual destination; never enqueue it because a trade was open. |
| Cursor resync | For example augmentation rejection can send ItemPacketCharInventory0x69 for cursor33 | Not all cursor-addressed packets are tail appends; classify packet and operation context. Augmentation itself remains out of scope. |

Evidence: `common/inventory_profile.cpp:44,50,56,250,446,1394`;
`zone/inventory.cpp:916,1035,1054,1991,2211,2970`;
`zone/client_packet.cpp:1783–1800`;
`common/shareddb.cpp::SaveCursor`; `zone/npc.cpp:4687`.
`SendItemPacket` passes the supplied type/serialized instance through; it does
not convert all nonhead Limbo pushes into a different on-wire operation.

Item instance IDs offer evidence, not a universal dedup rule. RoF2 SerializeItem
uses GetSerialNumber for ordinary items (`rof2.cpp:6485`), and Clone's copy
constructor preserves it (`item_instance.cpp:144,886`). However a stack split
creates a **new** server item (`inventory.cpp:2073`), while current client split
prediction clones the old instance ID. Login regenerates ordinary instance
serials. Evolving items use a different ID encoding. Two equal item IDs are
never proof of a duplicate, and a zero/unknown/predicted serial is not an
authoritative identity.

Implement a bounded cursor model with explicit ownership of head/tails, packet
provenance and pending move context. Before choosing a refresh/dedup algorithm,
capture the limited dedicated sequence below and verify instance IDs, source
counts, packet ordering and sent callbacks. Do not adopt “every0x6a appends”,
“every0x6a replaces”, or global repeated-ID suppression. Ambiguous state should
freeze the affected cursor operation and request authoritative reconnect,
rather than guessing or repeating a destructive command. Preserve original
bag child indices and independent instance identities during promotion.

This also affects **existing player-trade cancellation and disconnect**:
`FinishTrade(this)` calls FindFreeSlotForTradeItem. With no ordinary space it
returns cursor; PutItemInInventory(cursor,true) delegates to PushItemOnCursor.
Multiple returned items then arrive as successive0x6a pushes. Its fallback and
lore/no-drop rejection paths also push to cursor. `TRADE_PROTOCOL.md`'s prior
live tests exercised normal refund/persistence, not this full-inventory queue
case; do not retroactively claim it was covered.

### Reconciliation investigation: unresolved

The source confirms push/refresh behavior, but the inspected wire does **not**
label those two uses of Limbo differently. In particular, after splitting a
stack onto the cursor the client lacks its freshly allocated server serial. A
later partial move out to an empty slot sends a refresh of the remaining head.
A newly pushed legitimate identical item can have the same item ID, contents
and count as that expected refresh. Neither a content comparison nor “first
Limbo after a sent move” proves identity: another ordered server push can have
been queued before the server processes that move. Do not discard it.

Checked possible reconciliation mechanisms:

| Mechanism | Source evidence | Conclusion |
| --- | --- | --- |
| ItemVerifyRequest0x189c / Reply0x097b | Request16 is slot12+target4; reply20 is slot12+spell4+target4 (`rof2_structs.h:4710–4722`, `rof2.cpp:2128`). Handler `client_packet.cpp:9414` replies before validation, then enters item-click hooks/casting | No item serialization, item ID or instance serial. It is an item-use action, not a read-only inventory probe. |
| OpenInventory | `client_packet.cpp:11118` handler is empty; RoF2 opcode table comments it out as nonexistent | No supported request for full inventory. |
| CharInventory0x5ca6 | Server zone-entry bulk sender, followed by cursor-tail packets | Reconnect yields a new authoritative snapshot; no corresponding request handler was found. |
| SwapItemResync | Called after a failed valid-slot MoveItem; `inventory.cpp:2238` sends temporary Copper Coin22292 type0x67, then actual item0x67 or delete for each slot | This exposes real serialization on an error path, but intentionally failing a mutation is not a supported read-only query. Do not provoke resync as normal operation. |
| Item links/previews | Item-template lookup/inspection handlers, distinct from owned-instance synchronization | Not evidence of the current cursor instance. |
| Old OpenEQ C# client | Search found profile cursor currency/opcode declarations, no implemented cursor queue handling to reuse | No native behavioral evidence. |

No source-backed, harmless serial-reconciliation query was found. Native RoF2
cursor packet handling was **not reverse-engineered or captured** in this pass.
Therefore full cursor support, including split-created unknown serials and
asynchronous identical pushes, remains unresolved; this plan does not prescribe
a guessed dedup algorithm. A future bounded investigation can capture exact
native behavior on dedicated fixtures, or evaluate an explicit server-side
synchronization improvement as its own reviewed change. Reconnect or disabling
an ambiguous operation are conservative fallbacks, not claims of full parity.

Current item-use intent checks also compare instance identity. A correct fix
must distinguish authoritative versus locally predicted serials and cover
item-use/inspection stale selections, not merely the rendered cursor count.

## Implementation order and invariants

1. Land cursor queue behavior independently of NPC UI. Preserve player trade,
   loot, summons, item-use and scribing; test queue rebuilding after reconnect
   and ordinary head moves before enabling multi-item returns.
2. Add an explicit partner kind and immutable session identity: player versus
   NPC, own/NPC spawn IDs, name, current zone generation and local request token.
   Preserve existing player rules; don't infer kind later from a reused entity
   ID. Use checked queued/sent/rejected callbacks and invalidate stale choices.
3. Add NPC availability checks and auto-ACK handling; no player invitation
   screen for NPCs. Preserve the existing finite20-unit proximity policy as a
   client choice, not a measured server limit. Filter corpses/body11 and do not
   label arbitrary bots/pets as safe quest NPCs without confirmed spawn fields.
4. Add four-slot context-aware escrow validation. Keep cursor-only, empty
   destination, no direct withdrawal/destruction, sent-once ownership and coin
   arithmetic guards. Allow ordinary no-drop quest items only in confirmed NPC
   context. Bags/attuned/evolving items remain explicit later coverage.
5. Bind Give/Cancel to the recorded session and current offer revision. One
   Give locks mutation until the finish transition; Finish clears escrow only.
   Display “Hand-in ended; check items and chat” or equivalent. There is no
   generic post-script completion marker or safe timer that means “quest
   succeeded”; continue applying ordered result packets after the window ends.
   Never auto-retry Give after timeout, disconnect or an uncertain outcome.
6. Handle NPC cancellation independently of player reciprocal barriers; retain
   unresolved state on connection loss and trust reconnect inventory/currency.
   Keep delayed old ACK/cancel/close packets from changing a replacement session.
7. Render original `EQUI_GiveWnd.xml`, Screen `GiveWnd`: GVW_NPCName,
   GVW_MyItemSlot0–3 (EQType3000–3003), GVW_MyMoney0–3 (platinum→copper),
   GVW_Give_Button and GVW_Cancel_Button. Actual quantities replace XML sample
   text. Closing active Give should run the same guarded cancel, not silently
   discard escrow. Preserve window focus/stacking/scaling and player TradeWnd.

Inventory invariants: an item tree is owned in exactly one carried/cursor/escrow
location; pending sends don't double-move; only the current head is actionable;
slot4–7 cannot be NPC escrow; remote views never enter ownership; ending escrow
doesn't delete refunds; count/charge distinction is retained; no inferred
reward/refund; delayed events cannot operate on a new session. Coin invariants:
exact denomination debit once, nonnegative bounded buckets, no trade withdrawal,
full MoneyUpdate replaces rather than adds, and state after reconnect wins.

## Dedicated fixture plan (not executed)

Read-only preflight found existing **Barterer/openeq_trade1** and
**Swapper/openeq_trade2** offline, level10 warriors, PoK202 instance0. Prefer
Barterer for hand-ins and Swapper only for player-refund regression, after
coordination and a fresh offline check. Fellowship/Companion are existing social
fixtures but are unnecessary here. Never use Explorer or another task's active
character. No new player accounts are needed.

Snapshot private create-new mode0600 files before any future fixture mutation:
exact character/account IDs, pose/resources, currency including bank/cursor,
all inventory rows and cursor queue/bag descendants, item flags/augments/custom
data, binds, spells/gems/buffs, XP/AA/level/stats, skills/disciplines, factions,
tasks/activity/completion rows, quest globals/data buckets, corpses and social
membership. Inspect deployed schemas first. Preserve all item gameplay fields;
record instance GUIDs separately because normal login regenerates them. Verify
no preexisting escrow/queued cursor items or relevant active/shared tasks, and
that the character/NPC fixture contains no unrelated participant/state.

The four simple item records already exist: Water Flask13006 (stack20),
Short Sword5001, Cloth Cap1001 and Backpack17005. Use cheap noncontainer items
first; reserve a bag test for cursor-tree coverage. Read-only rules found
Items:AlwaysReturnHandins=true and TaskSystem:EnableTaskSystem=true in ruleset1.
PoK has ruleset2 (`pop+`), Arena ruleset1; no hand-in override was returned for
ruleset2. Confirm the active loaded rule before testing; do not change shared
rules or infer task immunity from a quiet location.

### A. Cursor and existing player refund first

Use only the existing two trade fixtures. Plan the full-inventory case with
recorded cheap fixture items and no valuable possessions. After a private
snapshot, deliberately arrange two ordinary refunded items plus an existing
cursor head, cancel through the foreground, capture0x6a/cursor33 IDs/order, and
move each head into a known empty legal slot one at a time. Verify SQL and a
normal reconnect preserve every instance/count exactly. Capture the matching
SendCursorBuffer refresh when a known tail becomes head. Repeat with two equal
item IDs but distinct instances, a partially moved stack, a full consumed head,
and a bag tree. Negative tests reject repeated pending moves and ambiguous
identity; don't send invalid moves to provoke server kicks.

Keep any necessary inventory rearrangement reversible, scoped to fixture rows,
and recorded. Reuse the established player cancel/disconnect API; this phase
needs no NPC or quest changes. No result should be described as safe queue
support until both remote state and foreground order agree after reconnect.

### B. Deterministic isolated NPC

No existing OpenEQ hand-in test NPC was found in the read-only name search.
For a later authorized implementation run, reserve a new recorded NPC type and
unique quest file; never replace a PEQ NPC/script. Spawn only that NPC in a
quiet controlled fixture location through the supported GM path, with no
permanent public respawn, aggro, pet ownership, multiquest, tasks, quest globals,
faction/XP changes or world messages. Restrict its script to the exact fixture
character IDs and return everything for other callers. Record NPC/spawn/quest
identity and original absence before setup so cleanup is exact and reviewable.

Use source-backed `eq.handin`/CheckHandin and ReturnHandinItems, not a simulated
client acknowledgment. Distinct branches can prove:

- Unknown ordinary item and a small unsupported coin offer: all returned.
- Water Flask stack5, requiring2, no money: consume2, return3, and deliver one
  non-lore reward. This intentionally exercises reward plus returned-tail order.
- Coins-only exact1gold, no items: consume the offered bucket and give one
  known reward; separate rejected2gold case returns money. Do not combine a
  matching required item with wrong money because of the source concern above.
- Cancel before Give with item+coin offers: no script consumption/reward.
- Normal disconnect before Give and reconnect: verify refund when NPC survives.

The script must report fixture-local text and deterministic outputs only. Do
not introduce rewards that alter external return guards accidentally. Test one
branch per fresh transaction. Captures are restricted to relevant trade/item/
currency events, not auth/UCS secrets. Use real `Interaction`/`LiveWorld` paths
and rendered actions for integration proof, not only direct ZoneClient packets.

### C. Real deployed PEQ quest

After cursor/NPC tests pass, **Caden_Zharik** in PoK is a small audited candidate:
deployed `quests/poknowledge/Caden_Zharik.pl` accepts one Note to Caden28084,
summons Boiron's Standard28085, and calls `quest::exp(250)`; it has no faction,
global or task mutation in that handler. Both item records are ordinary
nonstackable no-drop quest items. This proves NPC-specific no-drop behavior.

Use the database, not the script's stale footer: current NPC type is **202129**,
class41/body1/faction0; NPC202128 is Councilman_Srethix. Current spawn2 row40497,
spawngroup34379, PoK version0, server pose `(752,-343,-94)`, heading254.
Resolve actual runtime spawn identity after entry; database IDs are not packet
targets. Don't alter Caden's script, merchant inventory or respawn. Script hash:
`82ed85621df4e143d02aae6af276992ee73fa29d099ea599eeb15b371544dd69`.

Provision only the disposable fixture's one quest input after snapshot, or earn
it through the audited previous quest if separately scoped. Require real
reward ItemPacket/cursor, consumed input, server XP response and reconnect
persistence; do not predict final XP=baseline+250 because server scaling may
apply. Preserve all observed deltas before offline restoration. The handler's
source simplicity is not proof that global player hooks never run, so snapshot
and compare those independent invariants too.

### Acceptance, cleanup and remaining limits

Pure/reducer tests must cover NPC ACK and four-slot boundaries, no-drop context,
NPC-ID cancel echo, Finish-before-items, repeated cancel/close/Give, stale old
session events, sent/rejected callbacks, currency replacement, ordered cursor
push versus refresh, identical item IDs, partial splits, bags, reconnect and
unchanged player trade behavior. GPU/native checks use the original GiveWnd at
normal/Retina/narrow sizes, disabled pending controls and actual quantity text.

Live acceptance requires exact sent/received sequence, inventory/cursor order,
counts/charges, money, quest outputs and SQL persistence, then normal logout.
Restore each fixture independently only while offline. Delete only recorded
temporary NPC/spawn/quest rows/files created by the test; do not globally clear
inventory, task or quest data and do not restart shared zones as cleanup.
Retain a private journal of generated rewards and intended restoration. Verify
all fixtures offline and exact baseline gameplay data restored, or explicitly
report a preserved intentional change.

Deferred: task journal UX, arbitrary quest completion prediction, mixed
item/wrong-money server defect, NPC disappearance during escrow, multiquest,
pet/bot equipment, attuned/evolving/bag hand-ins, augmentation and shared-task
delivery. There is no live NPC hand-in proof from this research task.
