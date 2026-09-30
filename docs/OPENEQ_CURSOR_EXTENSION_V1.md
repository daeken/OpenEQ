# Proposed OpenEQ owned-state extension, version 1

Design and source review, 2026-09-30. **Proposal only:** no protocol handler,
client runtime change, deployment, database access or live action was performed.
EQEmu references below use `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`.
Read alongside [CURSOR_RECONCILIATION.md](CURSOR_RECONCILIATION.md) and
[QUEST_HANDIN_PLAN.md](QUEST_HANDIN_PLAN.md).

## Decision and bounded goal

Add one explicitly enabled, negotiated zone application opcode carrying full
owned-state snapshots and a small set of guarded commands. Use the existing
reliable EQ application stream and existing server inventory/quest operations.
Do not interpret native cursor packets as append, promotion or transaction
acknowledgments. Do not add a second network service or an arbitrary native
packet tunnel.

The minimum useful snapshot contains **carried possessions, the complete
ordered cursor, the caller's escrow, and carried/cursor/escrow coin buckets**.
A cursor-only response cannot settle a cancellation that stacks into an
existing carried item or returns to an ordinary slot. A cursor-only revision
also cannot protect a coin offer or a changed escrow offer.

The first implementation proves ordinary NPC hand-ins: whole ordinary items,
stacks, ordinary no-drop quest items, exact coin denominations, cancellation,
multiple returns behind existing cursor items, and one audited PEQ quest.
Bag/augment/evolving items must be represented in snapshots, but may be
read-only. Their presence must never cause omitted ownership or fabricated
empty state. Initial mutation support need not include those item types.

This is authoritative **observed inventory state**, with at-most-once dispatch
of a bounded command during one connection. It is not an atomic database
transaction, durable exactly-once quest execution, proof of quest success, or
a promise that future timers cannot award more items.

## Why an explicit extension is necessary

| Pinned source | Consequence for the design |
| --- | --- |
| `common/inventory_profile.cpp:44–92`; `inventory_profile.h:110–129` | The cursor is a FIFO list. Enumerate `cursor_cbegin()` through `cursor_cend()`; only its head is normally addressable. |
| `zone/inventory.cpp:1035–1108` | Pushes can serialize an appended instance; occupied-cursor RoF+ loot can publish nothing. Packet observation cannot reconstruct the list. |
| `zone/inventory.cpp:1126–1138` | Looted bag contents can become separate cursor roots. Snapshot the actual list and actual children, not an inferred loot bag. |
| `zone/client_packet.cpp:1791–1800` | Login sends a bulk head and then tails, without a complete-queue transaction marker. The extension needs its own activation baseline. |
| `zone/client_packet.cpp:15428–15502`; `zone/trading.cpp:508–687` | Native FinishTrade precedes task/quest execution and returns. A command receipt must be captured after the handler and synchronous quest stack return. |
| `zone/trading.cpp:319–507`; `zone/npc.cpp:4611–4726` | Refunds can change carried stacks/slots, the cursor and money. Capture all these together. |
| `zone/client_packet.cpp:4317–4358`; `zone/client_process.cpp:714–731` | Stock cancel/disconnect refund depends on `trade->With()` resolving. Vanished NPCs require an explicit extension cleanup policy. |
| `zone/trading.cpp:66–92` | `Trade::Start` and `Reset` provide central places for a trade generation; an entity ID alone is insufficient. |
| `common/item_instance.cpp:107–144,886` | Clone preserves native serials. Native instance IDs cannot be extension lifetime identities. |
| `common/inventory_profile.h:110`; `common/item_instance.h:129,160–161,230` | Writable instances, child maps and timers escape inventory helpers. Helper-only dirty flags miss in-place changes. |
| `zone/client_process.cpp:614–618`; `zone/main.cpp:592–618` | Packet dispatch, mobs and quest timers have separate boundaries. A flush only after one client's packet loop misses later callbacks in the same iteration. |
| `zone/client_packet.cpp:445–514` | Unhandled opcodes can invoke EVENT_UNHANDLED_OPCODE scripts. An arbitrary-server probe is not established as harmless. |

These are source findings, not live traces of this proposed protocol.

## Transport, enablement and negotiation

Proposed internal name: `OP_OpenEQOwnedState`. Proposed wire value: `0x7ffe`,
which is absent from the pinned `utils/patches/patch_RoF2.conf`. This is a new
allocation for an explicitly configured OpenEQ/EQEmu deployment, **not a stock
opcode or a universally reserved number**. Both sides must use the configured
value. Server startup must reject a collision with any other mapped opcode;
the feature is disabled by default. Opcode-table reload must either preserve
this binding for active streams or close their extension sessions safely.

Add the internal opcode to `common/emu_oplist.h`, the enabled RoF2 mapping,
and the connected-client dispatch table. The current RoF2 strategy defaults
unlisted encoders to pass-through (`common/patches/rof2.cpp:132–138`), which
is suitable for already encoded, pointer-free extension bytes. Do not expose
an `InternalSerializedItem` pointer through that pass-through path.

OpenEQ's server profile must explicitly enable this extension and record its
opcode. Normal stock profiles send no extension traffic. Keep the ordinary
RoF2 world/zone signatures and authentication unchanged: RoF2 identifies its
zone stream by the existing ZoneEntry opcode and exact record length
(`rof2.cpp:100–109,6417–6425`). Negotiate only after the normal connected/ready
barrier, with inventory actions temporarily frozen.

Handshake:

1. **Hello** supplies a fresh random 128-bit client nonce, version 1.0,
   required/requested capabilities and receive limits. Header epoch is zero.
2. **Welcome** echoes that nonce and returns a fresh random 128-bit server
   epoch, selected capabilities, limits, item codec version and diagnostic
   server build/policy identifiers. Server binds the epoch to the authenticated
   Client and its exact zone stream. No character ID in a request can select
   a different player's state.
3. **Activate** echoes the epoch and client nonce. Server verifies no
   unsupported commerce session is active, establishes the publication
   boundary on the zone owner thread, and sends **Activated**, immediately
   followed by a complete baseline snapshot.
4. Client changes ownership authority at Activated, stages that baseline,
   and unlocks supported actions only after its valid complete Snapshot. Any native
   ownership updates after Activated are excluded from the covered domains.
   Pre-activation state is replaced by the baseline, not merged into it.

An identical repeated Hello before activation may return the same Welcome;
a different Hello or Activate on an active epoch is a protocol error, not a
reset of its command history. The server queues Activated and the baseline on
the same ordered application stream as native packets. Implementation tests
must establish this barrier across the existing QueuePacket/FastQueuePacket
paths and the OpenEQ worker/foreground delivery boundary.

Capability bits are a contract, not just a version string:

| Bit | Version 1 meaning |
| --- | --- |
| `0x01 FULL_OWNED_SNAPSHOT` | Complete bounded snapshots, empty state, mutation publication, read-only resynchronization and epoch rules below. |
| `0x02 GUARDED_CURSOR_MOVE` | Server-checked revisions/identities and command nonces for the supported cursor/carried moves. Requires `0x01`. |
| `0x04 GUARDED_NPC_HANDIN` | Bound NPC escrow sessions, guarded offer/coin/Give/Cancel, vanished-target cleanup and synchronous-handler receipts. Requires both other bits and reviewed ownership prerequisites. |

Hand-ins require all three bits. Unknown required bits or incompatible limits
fail negotiation. Snapshot-only support may be useful diagnostically but must
not unlock Give. The selected mutation policy initially says
`ordinary_npc_handins_v1`; it is not a claim to support bags, bots or arbitrary
commerce operations.

No Welcome within three seconds means unavailable, not permission to invent
a reply or retry mutations. Before activation and before any extension
mutation, a profile may return to the existing bounded stock mode. A malformed
handshake is a profile/protocol error. **After activation, there is no silent
mid-session downgrade:** reconnect before changing ownership authority.
Stock-server NPC hand-ins remain disabled until a separately proven stock
contract exists. Existing stock-profile player trade remains unchanged.

## Envelope and bounded snapshot format

All integers are little-endian, encoded field by field without C/C++ padding.
The application opcode is outside this fixed 44-byte extension header:

| Offset | Field |
| ---: | --- |
| 0 | Eight bytes `4f 45 51 49 4e 56 00 00` (`OEQINV` plus two zeros) |
| 8 | `u16 major = 1` |
| 10 | `u16 minor = 0` |
| 12 | `u16 message_type` |
| 14 | `u16 flags = 0` in v1; unknown flags rejected |
| 16 | `u32 payload_bytes`, exact remaining application payload length |
| 20 | 16-byte epoch; zero only during Hello or pre-epoch rejection |
| 36 | `u64 request_id`; zero for unsolicited publication |

Message IDs: 1 Hello, 2 Welcome, 3 Activate, 4 Activated, 5 SnapshotRequest,
6 Snapshot, 7 Command, 8 CommandStatusRequest, 9 CommandStatus, 10 Fault.
Query IDs and mutation nonces use separate namespaces identified by message
type; Snapshot also identifies `baseline`, `query`, `mutation_result` or
`unsolicited` origin.
Every response echoes its origin's request ID. No stringly typed command or
embedded native opcode is accepted.

Version 1 maximums, negotiated downward:

- 200 cursor roots, 4096 total item nodes, four child edges of nesting,
  200 child indices per bag and six augment indices per common item.
- 3 MiB for the entire encoded Snapshot application payload, including its
  extension header and metadata. Use the existing reliable EQ transport
  fragmentation/reassembly, whose OpenEQ application ceiling is 4 MiB
  (`stream.rs:506–552`). There is no second fragmentation protocol or separate
  begin/end message whose arrival could be confused with a mutation result.
- One snapshot under construction and one command in flight per client.
  At most two complete snapshot buffers retained for transmission/receipt
  recovery. Bound actual byte queues too; a stalled peer must not allocate
  without limit.
- Snapshot requests: at most two per second with a burst of two. Mutation
  results and the activation baseline are not dropped by this query throttle.
  Rate-limited queries receive an explicit response; they never dispatch work.

**Snapshot** declares `snapshot_id u64`, `state_revision u64`, origin `u8`,
status `u8`, reserved-zero `u16`, domain mask `u32`, root count `u32`, node
count `u32`, cursor count `u32`, body byte count `u32`, and the correlated
command metadata, followed by its complete body. The fixed command metadata
is nonce `u64`, verb `u16`, disposition `u16`, reason `u32`, trade session
`u64`, pre-revision `u64`, post-revision `u64`; all are zero for non-command
snapshots. Origin values are 0 baseline, 1 query, 2 mutation result and 3
unsolicited. All lengths must match the application message exactly.

Server freezes all bytes before queueing this single application message.
The existing stream delivers it only after ordered reassembly. Client requires
matching epoch/IDs/counts and successful bounded parsing of the complete body
before one atomic reducer replacement. No partial tree or head is exposed.
Explicit cursor count zero and absent root records within a complete domain
mean empty; silence never does. Unknown schemas, duplicate roots/edges,
impossible nodes, excess depth, unconsumed bytes, truncation or failed
serialization reject the whole snapshot and freeze affected actions. A broken
transport fragment sequence cannot become a partially accepted snapshot.

The snapshot body contains, in order:

1. A fixed domain declaration: possessions roots 0–32, the ordered cursor
   separately, own escrow roots 0–7, and carried/cursor/escrow wallets. Root 33
   appears **only** as cursor element zero, never as a duplicate possession.
   Empty ordinary and escrow roots are represented by absence within the
   declared complete domain. Bank/shared-bank/tribute/world containers and
   other players' escrow are outside this v1 domain and are not erased.
2. Trade descriptor: kind (`none`, `npc`, `legacy_external`), session ID,
   phase (`open`, `dispatching`, `ended`, `target_gone`, `unresolved`), partner
   spawn ID/generation, NPC type/name, trade generation, and permitted actions.
   NPC sessions require slots 4–7 empty; unexpected contents are a fault.
3. Three wallets, each four `u32` buckets in copper/silver/gold/platinum order.
   Values are actual server state; offers remain bounded by the existing
   signed-32-bit server arithmetic. No denomination conversion is inferred.
4. Roots ordered by domain, then ordinary/escrow slot or cursor queue index.
   Each root records domain `u8`, index `u16`, node count `u32`, manifest byte
   length `u32`, item byte length `u32`, then the manifest and item bytes.
5. Each manifest node, in preorder: node token `u64`, parent preorder index
   `u32` (`0xffffffff` for root), edge kind `u8` (`root`, `bag`, `augment`),
   edge index `u16`, item ID `u32`, raw signed charges `i32`, and item flags
   `u32` (stackable, container, attuned, evolving, unsupported-for-mutation).
   Parent kinds and child indices must agree with the actual server tree.

The versioned item codec is **RoF2-owned-tree-v1**: the existing RoF2 item
serialization with a fixed `ItemPacketCharInventory` encoding context, from
an immutable render copy, plus the explicit manifest. Every cursor root is
serialized with canonical cursor slot 33; its envelope index supplies queue
order. The native slot field is never a tail address. For children, the
explicit sparse manifest edge supplies the address, including augment versus
bag distinction. Native serialized child indices must match it exactly.

`rof2.cpp:6441–6545,6890–6929` already serializes rich item data and sparse
child indices, but this needs a bounded exported helper, not calls to a private
symbol from zone code or a copied serializer. Validate the actual GetContents
graph first, including unsupported/hidden indices, null entries, duplicate
pointers and cycles. Compare serialized root/node counts with the graph;
never inherit stock's “item skipped” behavior (`rof2.cpp:1078–1082`). Audit and
zero-initialize wire structures so padding/uninitialized bytes cannot enter
the snapshot. Evolving serialization calls CalculateEvolveProgression; use
the render copy so a read-only query does not modify live items or normalize
the queue. Do not call SendCursorBuffer, lore cleanup, SaveCursor or Save()
from snapshot requests.

OpenEQ's existing parser preserves sparse child indices, but treats all child
edges as bag edges and skips evolving metadata (`inventory.rs:196–250`). The
extension decoder must retain raw unsupported item data and the complete typed
ownership tree; it cannot quietly use that lossy interpretation as full
augmentation/evolving support. Unsupported roots remain visible and locked.
If the decoder cannot retain a complete tree, reject the snapshot instead.

## Epochs, identity, revision and freshness

Epochs end on disconnect, zone transfer, a replacement zone stream, extension
failure, or server restart. They are not character IDs, spawn IDs, the EQ
transport sequence number, or persistent item GUIDs. Clear pending ownership
commands and require a new complete baseline on every new epoch. Preserve an
uncertain transaction journal for display, but never replay it into that epoch.

Add a transient, nonzero 64-bit **object lifetime token** to server item
instances. Allocate a new token on every construction/copy/Clone, preserve it
when the same object moves, and preserve the destination's token during
assignment. Child clones get new tokens too. Tokens cannot be raw pointers or
GetSerialNumber; pointer reuse and serial-preserving clones would permit ABA
identity mistakes. They need not be stored in SQL or sent to native clients.
The pair `(epoch, token)` is the extension identity; a server clone may thus
change identity during a normal move. The resulting snapshot wins, without
trying to match old and new objects by item ID or contents. Exhaustion faults
the extension instead of wrapping. Render copies do not replace the tokens
captured from live owned nodes in their manifests.

`state_revision u64` starts at 1. It advances when the canonical owned-state
projection changes: root membership/order, object token, typed child graph,
item serialization/manifest values, included wallets, or trade descriptor.
There is one revision for this combined domain, not unrelated cursor and
escrow clocks. A node remaining at the same slot with changed charges still
changes the revision. Native serials are diagnostic item data only.

`snapshot_id u64` advances for every newly frozen snapshot, even if its state
revision is unchanged. This permits separately correlated read-only responses.
Duplicates of a cached response retain both IDs. Client may resolve a matching
command receipt from an older snapshot, but must never replace newer installed
state with it. Snapshot IDs/revisions cannot wrap; reconnect on exhaustion.

Revision describes this declared ownership projection. It does not version
every quest global, NPC condition, hidden item custom value, or future timer.
Server always revalidates current gameplay eligibility at dispatch; client
does not derive quest outcomes from the revision. Private custom data is not
copied into network snapshots.

## Publication that includes silent and in-place mutations

For the initial bounded implementation, prefer complete state comparison to
a claimed-complete set of dirty hooks. Freeze/compare a deterministic deep
projection on the zone owner thread:

1. Immediately before validating every extension mutation or snapshot query.
2. After each incoming application handler returns, including its synchronous
   task/quest calls and helper calls; a Give result is captured here.
3. At the end of each loaded-zone main-loop iteration, after world/entity/mob,
   scheduler, zone and quest-timer work (`zone/main.cpp:592–618`). This scans
   every activated client, including clients mutated by another participant.

Only publish a new unsolicited full snapshot when the projection differs.
Queries and command receipts can produce a new snapshot ID without a state
change. Comparing a stable capture must be deterministic; do not compare raw
C++ object memory, timer remaining-time noise or uninitialized serializer
bytes. A scan/compare cap failure is a protocol Fault, never “unchanged”.
Dirty hints may optimize later; they cannot replace these checks without a
separate complete mutation audit and regression.

The pre-dispatch comparison is mandatory even if a snapshot was just sent:
another client's handler or timer may have changed the cursor since then.
Advance revision first, then reject the stale command without executing it
and return the current snapshot. This closes the publication/network-delay
window without pretending the displayed UI is continuously current.

This covers silent loot, direct false-update pushes, in-place SetCharges,
queue rebuilds, lore filtering, native resync token packets, delayed quest
awards, another player's refund, and post-client-loop quest timers. It does
not repair the underlying mutation or promise a snapshot for every transient
state inside one synchronous operation. The operation boundary owns the final
state. Correctness requires all accesses to this graph on the owner thread;
assert that invariant and audit any background writer before enabling the
capability. Do not iterate concurrently with mutation.

If output is slow, coalesce only unsent unsolicited snapshots to the newest
state. Preserve every outstanding command receipt. If the bounded send queue
cannot retain the result, fault/close the epoch and report an uncertain
outcome. Never keep processing mutations while dropping their authority data.

## Commands and at-most-once dispatch

Every Command body starts with `mutation_nonce u64`, `expected_revision u64`,
`trade_session_id u64` (zero where inapplicable), `verb u16`, reserved zero
`u16`, and a length-checked typed verb body. Header request ID equals the
mutation nonce. Nonces start at 1, increase by one, never wrap, and are scoped
to the epoch. Only one mutation may be outstanding in worker **and** foreground.

Before invoking any existing gameplay helper, the server validates epoch,
nonce, fresh current projection/revision, trade generation, source token/path,
destination occupancy, current eligibility and all existing game restrictions.
A destination expecting empty is explicit; it is not “whatever is there now”.
No part of a rejected precondition may debit money or pop an item.

Minimum typed verbs:

| Verb | Initial support |
| --- | --- |
| `MoveWhole` | Actual cursor head to an empty legal carried slot, or an ordinary carried item to an empty cursor. Source/destination use typed possession/bag paths, expected source token and expected-empty destination. Tails cannot be selected. |
| `SplitToCursor` | A bounded positive proper subset of an ordinary carried stack into an empty cursor. Server creates the new instance; the snapshot supplies its token. No client-cloned serial is authoritative. |
| `OpenNpc` | Current target spawn ID, no existing escrow or conflicting interaction. Server validates a targetable, nonengaged real NPC with an eligible hand-in handler, excluding bots/pets; returns its identity and a fresh trade session. Opening does not transfer an item. |
| `OfferItem` | Current head token, active NPC session, empty destination 0–3, count 0 (whole item). Ordinary no-drop is allowed only through this NPC path; player/bank restrictions stay intact. Initial policy rejects containers, augments, attuned/evolving and otherwise unsupported offered trees. |
| `OfferCoin` | Current NPC session, denomination 0–3, positive amount within carried funds and both existing signed-32-bit bucket/headroom limits. No cross-denomination conversion or escrow withdrawal. |
| `Give` | Current NPC session and combined state revision; dispatch existing NPC FinishTrade flow exactly once, then capture after synchronous returns and trade reset. |
| `Cancel` | Exact extension NPC session; refund caller's escrow once through the reviewed self-refund operation and reset the session. No quest EVENT_TRADE. |

Assign verb IDs 1–7 in the table's order. A location is `domain u8`, root index
`u16`, path length `u8`, then `(edge_kind u8, index u16)` steps; only documented
possession/head paths are actionable. MoveWhole carries source and destination
locations plus source token `u64`; SplitToCursor adds count `u32`. OpenNpc
carries target spawn ID `u32`. OfferItem carries head token `u64` and destination
escrow index `u16`; OfferCoin carries denomination `u8` and amount `u32`.
Give and Cancel have no additional verb body. Unknown verbs, nonzero reserved
fields or trailing bytes are rejected.

Use common gameplay helpers with explicit checked results where needed; do
not recursively hand an arbitrary client-supplied packet to HandlePacket.
Existing void functions such as Trade::AddEntity do not provide a trustworthy
success boolean. A wrapper must distinguish refusal before dispatch from an
operation that ran and yielded an unexpected state. All existing server
inventory restrictions remain necessary after protocol validation.

Each result contains nonce, verb, trade session, pre/post revisions, disposition
and complete post-boundary snapshot. Dispositions:

- `REJECTED_BEFORE_DISPATCH`: no helper ran, no requested mutation occurred;
  reason identifies stale state, invalid session, unavailable target, policy
  rejection, invalid path/amount or unsupported command.
- `HANDLER_RETURNED`: the validated helper and synchronous call stack returned.
  This is not a claim that a quest accepted the offer, rewarded anything, or
  committed SQL atomically. The state and trade phase describe the result.
- `FAULT_AFTER_DISPATCH`: work may have occurred but a trustworthy complete
  result is unavailable. Freeze; do not retry or fabricate a rollback.

After envelope/epoch/order validation, reserve the next nonce before gameplay
preconditions or game code. Even a precondition rejection consumes that nonce
and caches its rejection; it cannot become executable later when conditions
change. Check duplicate nonces before rechecking current gameplay conditions.
Store exact request bytes/hash and the immutable result. A duplicate of the
last nonce with identical content returns that result without dispatch. Different
content for the same nonce is a protocol error. Older nonces always return
`EXPIRED_NO_REPLAY`; a newer nonce with a gap returns `OUT_OF_ORDER`. Never
interpret an evicted nonce as a new operation. One retained last result plus
a permanent per-epoch high-water mark suffices with one command outstanding.
Read-only status/snapshot queries do not evict the last mutation result.

CommandStatusRequest takes the mutation nonce and returns `in_flight`, its
cached result, `not_dispatched_as_of_this_barrier`, or `expired`. Queries have
independent query IDs. A status of not-dispatched is not permission to race
an old queued send with a new Give. The client uses explicit local send
retirement plus server barriers; the initial implementation should never
automatically resend a mutating command at all. A timeout offers a read-only
status query and reconnect, preserving uncertainty across connection loss.

## Trade lifetime, cancellation and client authority

Allocate a nonzero session ID at successful OpenNpc. Attach it to a generation
advanced centrally by every Trade::Start/Reset, including nonextension callers.
Bind the NPC to an entity registration generation, not merely a recycled spawn
ID or pointer address. A replacement NPC at the same ID cannot receive a Give
for the prior session. Echo NPC name/type/runtime generation in the snapshot;
client binds the returned session to its current target/zone and displays that
identity before any offer. A late open response cannot unlock a replacement
foreground interaction.

When a bound NPC disappears, preserve the caller's bound escrow and mark
`target_gone`. Give rejects before dispatch. **Cancel still refunds that exact
caller-owned extension session**, using FinishTrade(this) without requiring
the NPC pointer to resolve. Implement the same guarded self-refund during
disconnect before clearing the extension session; do not require a vanished
partner for cleanup. This is a new reviewed cleanup path, not a claim that
stock already does it. If another script reset the underlying trade generation
or the ownership binding is inconsistent, report `unresolved`, not a successful
refund. Never borrow a new trade's escrow to settle an old session.

Give/Cancel close messages are ordinary UI evidence only. The extension result
can say the synchronous handler ended and escrow is now empty; delayed quest
timers may later produce independent snapshots. UI wording remains “Hand-in
ended; check items and chat.” No correlation tag promises that every item in a
post-state was caused by that command or that later rewards are complete.

After activation, native owned ItemPacket/CharInventory/MoveItem/DeleteItem,
covered currency updates, and native trade closes cannot mutate or clear the
covered authority model. Retain native nonowned merchant/corpse/partner/preview
data separately, and continue normal chat, XP, task and world events. Native
resync's temporary Copper Coin must never enter the extension's owned state.
The full snapshot alone projects cursor element zero to visible slot 33 and
owns the ordered tail list. Native serial equality never deduplicates roots.

Queued/sent callbacks update only operation status in this mode. They do not
predict a second item move or coin debit. Worker and foreground must carry
epoch, installed revision, nonce, trade session and authority generation;
stale queued work is retired at both boundaries. A newer snapshot invalidates
old rendered choices even when the current head has an identical item ID.

The first mutation-enabled profile is deliberately bounded. Block raw native
cursor/trade mutation commands from that activated connection unless invoked
by its validated extension handler; client-only button disabling is insufficient.
Unaudited item use, destroy/drop, augmentation, bank/world-container exchange,
bandolier and player-trade controls cannot bypass the guard while this bounded
mode is active. Audit incoming paths that can select cursor items, including
cast/item-use slots, not only MoveItem. Server-generated/script changes remain
allowed and are published by the scanner.

This restriction is a scope limit of the first opt-in profile, not a silent
regression of stock profiles. Before offering the extension as the general
default client mode, add guarded equivalents for existing player-trade and
other cursor controls, preserving their separate bilateral/session rules.
In particular, do not advertise player cancellation as correlated merely
because its refunds are present in a snapshot. Its ordinary refund projection
can be tested first in read-only snapshot mode.

## Ownership and persistence prerequisites are separate

Snapshot correctness cannot prevent the server from destroying an item.
Require the reviewed equivalents of patches
[0001](../tools/eqemu-patches/0001-preserve-cursor-tail-on-partial-consumption.patch),
[0002](../tools/eqemu-patches/0002-preserve-cursor-tail-on-head-replacement.patch)
and [0003](../tools/eqemu-patches/0003-preserve-cursor-tail-on-popped-head-restoration.patch)
before advertising either mutation capability. These repair partial
consumption, visible-head replacement, and the two audited already-popped
restoration callers. They do not implement this extension, fix all cursor
persistence, or make a native packet an acknowledgment. A diagnostic read-only
snapshot build may omit them, but must advertise no mutation capability.

Additional pinned-source limits must remain visible:

- `SharedDatabase::SaveCursor` (`common/shareddb.cpp:151–183`) stops after
  CURSOR_BAG_COUNT roots and still returns true. That count is 200. A complete
  in-memory snapshot of a larger queue cannot claim persistence.
- SaveCursor uses cursor-bag-range slots for tails. UpdateInventorySlot only
  saves bag children when SupportsContainers(slot) is true
  (`shareddb.cpp:280–293`; `inventory_profile.cpp:1240–1255`), which excludes
  those tail slots. LoadInventory treats the cursor-bag range as queued roots
  (`shareddb.cpp:803–807`). This is a source-backed persistence concern for
  cursor bags/children. Subsequent isolated actual-source tests reproduced
  omitted queued-bag children, head-child/tail row collisions, and ignored
  clear/child-write failures. See [CURSOR_PERSISTENCE_AUDIT.md](CURSOR_PERSISTENCE_AUDIT.md)
  for the exact database seam and loader-dispatch limits; no live database or
  reconnect was used.
- Existing bandolier persistence/head publication, generic charged deletion
  packets, mixed matching-item/wrong-money hand-in behavior, and external
  return suppression remain separate concerns from queue snapshots.

Initial mutation proof therefore uses ordinary noncontainer cursor roots and
small queues, with an audited bounded reward script. Do not permit an initial
hand-in when persistence cannot represent its current cursor state. Unexpected
post-script growth/unsupported state produces a fault without deleting or
truncating any in-memory roots. It is not safe to report durable success after
such a fault. General arbitrary-script/bag support needs separate persistence
regressions/fixes and output-capacity policy; it must not be inferred from the
ordinary hand-in proof. The protocol itself never drops a tail to meet a limit.

Even within these bounds, v1 receipts say **observed**, not **durably committed**.
Several existing helpers ignore individual SQL save failures. A process crash
can lose its nonce ledger or leave a partially persisted quest operation.
The reconnect baseline wins; old nonces are invalid, and an unresolved Give
is never automatically replayed. Durable transaction journaling would be a
separate server feature.

## Bounded implementation and proof sequence

1. **Freeze the contract and ownership base.** Review this proposal, make
   isolated server patches on the pinned source plus 0001–0003, and record
   exact protocol fixtures. No deployment is implied. Add constructor/clone/
   assignment token tests, copied-serial and pointer-reuse tests, and explicit
   200/201-root persistence-limit tests. Reproduce cursor-bag persistence
   separately before claiming that support.
2. **Build read-only capture and codec.** Actual InventoryProfile/ItemInstance
   tests must snapshot `[A,B,C]`, duplicate item IDs and serials, empty queues,
   sparse bags/augments, flattened loot roots, evolving metadata and unsupported
   trees. Verify full counts and no inventory, charge, lore, timer or database
   mutation during capture. Test native serializer failure as whole-snapshot
   failure. Rust decoding tests cover length/depth/count caps, typed edges,
   malformed/truncated transport reassembly, old epochs and atomic replacement.
3. **Prove publication before commands.** An actual-source harness executes
   false-update pushes, direct SetCharges/GetContents changes, queue replacement,
   lore promotion, a second client's refund, and a quest timer after the client
   loop. Require a new complete state or explicit fault. Add a mutation between
   the last publication and dispatch and prove its expected revision rejects.
   Measure scan/encode cost and bounded backpressure in this stage.
4. **Add guarded moves and nonce handling.** Prove whole head removal, promotion,
   carried-to-empty-cursor and stack split with real server-generated identity.
   Test stale source/destination, identical-looking clones, duplicate/different
   same-nonce payloads, old/noncontiguous nonces, reply loss/status query, and
   pre-dispatch vs post-dispatch faults. No assertion should merely mirror a
   copied queue algorithm.
5. **Add NPC session wrappers and foreground GiveWnd.** Prove four-slot and
   no-drop context boundaries, coin arithmetic, old-session Give/Cancel after
   reset/reopen, spawn-ID reuse, despawn cancellation, disconnect cleanup,
   native FinishTrade before synchronous rewards/returns, native receipt
   exclusion, and paused-foreground/worker races. Inject a delayed quest award
   after HANDLER_RETURNED and require a later unsolicited snapshot without a
   second transaction completion. Keep stock profiles on existing paths.
6. **Reviewed isolated live proof, separately authorized.** Follow the private
   snapshot/offline restoration and deterministic NPC plan in QUEST_HANDIN_PLAN.
   First test existing player refunds with a complete read-only snapshot;
   then enable the bounded mutation profile for one fixture. Prove multiple
   ordinary returns behind an occupied cursor, repeated equal item IDs,
   stack 5 / consume 2 / return 3 plus reward, accepted/rejected coin-only offers,
   cancel before Give, and disconnect/reconnect. Compare foreground order,
   snapshot manifest, server in-memory order and SQL/reconnect persistence.
   Do not treat matching final counts alone as proof of one dispatch.
7. **Finish the first NPC milestone.** Use the already audited Caden_Zharik
   quest with its ordinary no-drop note/reward and real server XP response.
   Require rendered request/offer/Give actions, one guarded Give dispatch,
   empty final escrow, preserved unrelated cursor roots, actual reward/chat/XP
   evidence and reconnect persistence, followed by exact fixture restoration.
   This establishes the documented ordinary NPC scope, not arbitrary quest
   success, multiquest, bags, bots, or crash-safe transactions.

Minimum evidence to call the extension complete in that scope: protocol
fixtures, actual-source precondition/ownership/publication tests, worker and
foreground races, malformed/stale traffic tests, source revisions and patches,
the bounded live trace, persistence comparison and cleanup record. Until then
this document is a reviewable route to implementation, not a new capability.
