# Cursor reconciliation: bounded client design and server regression

Reviewed 2026-09-29 against EQEmu
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. Read alongside
`QUEST_HANDIN_PLAN.md` and `DEEP_PARITY_ROADMAP.md`.

**The stock wire does not establish a complete authoritative cursor queue.**
A bounded client can retain known queue entries and refuse ambiguous actions,
but this review does not establish full cursor parity or enable NPC hand-ins.
The three server patches below fix independently reproduced ownership bugs.
They do not resolve packet ambiguity and have not been deployed.

## Source-backed packet limits

The server cursor is a queue whose only ordinarily actionable address is
slot33. `InventoryProfile::PushCursor` appends a clone. The following distinct
events can all expose cursor-addressed serialized items:

| Event | Source behavior | Consequence |
| --- | --- | --- |
| `PushItemOnCursor(inst,true)` | Appends, then serializes the appended item as Limbo0x6a at33 | A received item need not be the head. |
| `SendCursorBuffer` | Serializes the current head as Limbo0x6a at33 | A receipt need not append anything. |
| Zone-entry inventory | Bulk inventory includes head; subsequent Limbo packets serialize tails in order | Rebuilding requires the whole ordered entry sequence, not just the bulk packet. |
| RoF+ loot into occupied cursor | Appends without serializing the tail | The received item stream can omit owned items. |
| Looted bag behind occupied cursor | Appends bag and its contents as separate cursor entries | Do not reconstruct that queue from a corpse bag preview. |
| Partial stack split | Creates a new server ItemInstance without returning its serial | Cloning the original serial locally creates a false identity. |
| Cursor slot correction | Can use0x67 or0x69; SwapItemResync sends a temporary Copper Coin followed by actual state or deletion | Packet type and slot alone do not authorize treating each receipt as ownership append. |
| Cursor promotion | `SendCursorBuffer` may silently destroy duplicate lore heads before serializing a later one | A locally predicted promotion is not an acknowledgment. |

Principal sources: `common/inventory_profile.cpp:250,401,1394`;
`common/item_instance.cpp:144`; `zone/inventory.cpp:916,960,1035,1083,1133,2073,2211,2238`;
`zone/client_packet.cpp:1791`; `zone/client_process.cpp:749`.
References use the pinned unpatched revision above.

Ordinary serials are useful correlation evidence within audited paths, but
are not universal event identifiers. Clone preserves the serial, Lua can
push an existing instance again, and normal login regenerates ordinary
serials. Evolving identities have separate serialization rules. A nonzero
serial does not justify globally deduplicating receipts; matching item IDs or
contents are weaker still.

An indistinguishable case remains even without duplicate serials: after a
split creates an unknown head serial, two equal-looking receipts with serials
X thenY could mean an asynchronous pushX followed by head refreshY, or head
refreshX followed by asynchronous pushY. The visible order is identical but
the true head differs. A local send callback, a quiet interval, or the first
Limbo after sending cannot distinguish those histories. No harmless stock
inventory-refresh request has been established; intentionally failing a move
to trigger SwapItemResync is not a supported reconciliation query.

## Recommended bounded client model

Keep three separate properties: known queue order, identity provenance, and
pending operation state. A useful representation has:

- An ordered, actionable known prefix of complete item trees.
- A flag for a possible unknown suffix, plus retained observations behind that
  unknown gap. A later serialized refund must not jump ahead of an unseen
  looted tail. Once the prefix is exhausted, the cursor is unknown, not empty.
- Per-entry local identity tokens, with either an authoritative
  `(connection_epoch, wire_serial)` or provisional split identity. All split
  destinations need this distinction, including ordinary inventory slots.
- A pending operation record containing a local token, connection/zone epoch,
  source and destination revisions, predicted counts/trees, and the exact
  source-backed refresh expectation. Transport-queued, transport-sent,
  reconciling, and ambiguous are different states.
- A finite bound on retained entries/observations. Exceeding the bound freezes
  affected operations; it must not evict owned data and silently continue.

Keep cursor tails separate from the ordinary slot map, with the head projected
at33 for existing UI consumers. Only the head and its explicitly indexed bag
children are actionable. Moving a tree must preserve sparse child indices.
Inspection, scribing and click stale-selection checks must use local identity
and revision, not a copied split serial.

Suggested reducer boundaries are `begin_inventory_epoch`,
`receive_owned_item(packet_type,item)`, `queue_move(token,...)`,
`move_sent(token)`, `move_rejected(token)`, and
`mark_possible_hidden_tail(reason)`. Keep queue mutation in one reducer;
rendering and trade closure must not independently append or pop ownership.
Reject duplicate or old-epoch callbacks without reapplying their prediction.

Within explicitly audited ordinary move/refund paths, allow known whole-head
moves, correctly counted stack merges, head swaps that retain tails, and
normal serialized refunds. Track whether the server sends a head refresh:
partial movement to an empty slot retains and refreshes the original head;
partial merge into an occupied stack does not refresh; full removal may expose
the next head. A matching authoritative expected head is correlation evidence
for those paths. It is not a proof covering arbitrary quest-side cloning.

Do not unlock another affected operation merely because `CommandSent` fired.
Keep uncertain packets and entries as evidence; do not discard them by content
or consume an arbitrary first receipt as the expected refresh. When the next
head is provisional, hidden, contradicted, or ambiguously duplicated, freeze
affected item use/trade/destruction and require authoritative reconnect.
Timeout does not prove an empty queue, successful mutation, refund, or quest
completion. Unrelated ordinary inventory may remain usable where independent
validation proves its source/destination are unaffected.

Client tests should cover:

1. Known `[A,B,C]`: whole removalA and refreshB preserve `[B,C]` once.
2. Equal item templates with distinct serials remain distinct entries.
3. Partial cursor-to-empty-slot refresh retainsA; partial merge emits no
   expected refresh; zero-count ordinary swap retains all tails.
4. Split-created provisional identity followed by identical asynchronous
   receipt freezes instead of guessing which instance is the head.
5. Silent occupied-cursor loot followed by serialized refunds creates a gap;
   refunds stay behind it, including the bag/content flattening case.
6. Lore filtering skips a predicted tail; cursor correction emits the
   temporary0x67 token and actual state; neither silently authorizes a wrong
   head or deletes retained ownership evidence.
7. Delayed/duplicate sent callbacks and receipts after reconnect cannot mutate
   the new epoch. Reconnect rebuilds bags and queue order independently.
8. Player cancellation returns several items to an already occupied cursor;
   normal known refunds remain usable, while ambiguous outcomes stay explicit.

This is bounded support, not proof that an arbitrary server script cannot
silently change inventory. The existing cursor-consumption issues below also
mean item-use on a cursor with possible tails needs its own guard and proof.

## Reproduced server ownership bug and isolated patch

`InventoryProfile::DeleteItem(cursor, quantity)` pops the current head before
reducing its charges. If that head remains, it calls `_PutItem(cursor,head)`.
`_PutItem` itself pops the current cursor head before pushing its argument.
Thus `[A(5),B,C]` consumed by two units becomes `[A(3),C]`: B is lost from
the owned queue. Charged, nonexpendable heads have the same problem even when
their final charge is consumed and the empty item should remain.

The fix restores the already-popped remaining cursor instance with
`push_front`, retaining `_PutItem`'s `SetEvolveEquipped(false)` behavior.
Noncursor restoration, whole-head deletion, and expendable depletion retain
their existing paths. It changes no wire packet or database schema.

- EQEmu branch: `codex/cursor-partial-consumption`.
- Patch commit: `d414cef5fba6b4f3810c2f35e0e60e7fa9e4ba6b`.
- Isolated local worktree: `/tmp/openeq-eqemu-cursor-fix`.
- Durable patch: [`../tools/eqemu-patches/0001-preserve-cursor-tail-on-partial-consumption.patch`](../tools/eqemu-patches/0001-preserve-cursor-tail-on-partial-consumption.patch).
- The format-patch retains EQEmu source and test code licensed under GPLv3 or
  later. It is a separately reviewable EQEmu patch, not a relicensing of that
  code as part of the client.

The patch includes five real `EQ::InventoryProfile`/`EQ::ItemInstance` test
cases: repeated partial cursor-stack consumption with sparse bag children in
tails; a single cursor stack; a charged nonexpendable head through zero;
complete/expendable head removal with exact next-head promotion; and ordinary
carried-slot partial/full consumption leaving the cursor untouched. Assertions
check pointer/order preservation and charges, not a reimplemented queue mock.

**Before the source fix: 82/84 utility tests passed; two new inventory tests
failed. After the fix: all84 passed.** The runner returned1 before and0 after.
The old test main ignored `Test::Suite::run`'s boolean and returned success even
after assertion failures; the patch registers the suite and propagates failure
as exit1. `git diff --check` passes, and the format-patch passes `git apply
--check` against the clean pinned sibling checkout.

### Actual build and evidence

Tests ran on `storage2.daeken.dev` in disposable directory
`/tmp/openeq-cursor-regression.alUt1w`. The baseline runner compiled all
existing utility suites, the new suite and bundled cppunit with GCC14/C++20,
then linked actual cached `libcommon.a` and dependency libraries from
`/home/daeken/eqemu-bootstrap/build`. The corresponding cached source revision
was verified as the pinned commit above. No fake inventory/item classes were
substituted and no database connection was needed.

For the passing run, the actual patched `inventory_profile.cpp` was compiled
into a separate object and linked **before** that same common archive. This
selects the changed InventoryProfile implementation with the existing real
ItemInstance implementation. The runner used the cached build's feature
definitions and dependency headers/libraries; optimization wasO0. The common
header directory must be a quote include path (`-iquote`), since using it as
a general include path shadows the system `features.h`.

A synthetic configuration containing only disposable shared/log directory
paths supported the existing IPC tests. An initial no-config run had four
unrelated IPC failures; both reported before/after results use the same
synthetic configuration and all existing79 tests pass. No production
configuration or credentials were read or copied for these tests.

Local evidence: `/tmp/openeq-cursor-before.log`,
`/tmp/openeq-cursor-after.log`, `/tmp/openeq-cursor-build-tests.sh`, and
`/tmp/openeq-cursor-validation.txt`. Remote before/after logs and the build
script remain in the disposable directory. The committed test header adds
the standard license comment after the passing run; its tested code is
unchanged. The existing sibling EQEmu checkout remains clean.

This was a focused actual-object build and full utility-suite run, not a full
server rebuild or live fixture test. No deployment, restart or upstream push
was performed, and the deployed server should still be treated as vulnerable
until a separately reviewed rollout is verified.

## Second isolated patch: replacing the visible cursor head

`InventoryProfile::PutItem(cursor,replacement)` also removes two heads in the
pinned source: its initial DeleteItem removesA, then `_PutItem` removesB before
inserting the replacement. Ordinary `[A,B,C]` replacement therefore becomes
`[replacement,C]`. This is independent of partial consumption and was
reproduced after the first patch had already passed its84 tests.

The second fix keeps the existing validation, initial deletion and null-item
handling. For a valid cursor replacement, it clones and pushes the new item to
the front without a second removal. It preserves `_PutItem`'s evolve-equipped
reset, while ordinary noncursor PutItem retains its prior implementation.

- Second commit: `d4869f88f8799c6774cb71c802ae3540c03d6250`, on the same isolated
  branch/worktree and directly after `d414cef5f`.
- Second durable patch: [`../tools/eqemu-patches/0002-preserve-cursor-tail-on-head-replacement.patch`](../tools/eqemu-patches/0002-preserve-cursor-tail-on-head-replacement.patch).
- Apply patch0001 before patch0002. Both retain EQEmu GPLv3-or-later code and
  remain undeployed and unpushed.

Five added tests cover valid head replacement; sparse replacement-bag children
and unchanged tail-bag child pointers; null-item deletion removing exactly one
head; valid/null insertion on an empty queue; and unchanged noncursor
replacement/deletion. The same actual-object build and synthetic disposable
test environment were used. **Before patch0002: 87/89 tests passed, with the
ordinary and bag cursor-replacement regressions failing; exit1. After
patch0002: all89 tests passed; exit0.** This includes all first-patch tests and
all existing79 utility tests.

Local evidence is `/tmp/openeq-cursor-put-before.log` and
`/tmp/openeq-cursor-put-after.log`; remote evidence is `put-before.log`,
`put-after.log` and build logs in the same disposable directory. Both exported
patches were applied in order to a disposable copy of the relevant files from
the pinned baseline; the resulting implementation and test header match the
tested worktree byte-for-byte. `git diff --check` passes.

**Replacement is not restoration of an already-popped head.** A caller that
first popsA and then calls PutItem(cursor,A) will still replace the next headB
by design. Such callers need a distinct restore/prepend operation. The
bandolier/world-container restoration paths are addressed by the third patch
below; neither of the first two patches repairs those callers by itself.

## Third isolated patch: restoring an already-popped cursor head

Two typed C++ callers pop a cursor head and then restore a replacement through
`PutItem`: the world-container exchange in `Client::SwapItem`
(`zone/inventory.cpp:1963–1965`) and the stacked ammunition remainder in
`Client::SetBandolier` (`zone/inventory.cpp:3142–3155`). After patch 0002,
`PopItem` changes `[A,B,C]` to `[B,C]`, and the subsequent `PutItem` replaces B.
Restoration therefore produces `[replacement,C]`, losing a tail. These line
references are to the pinned, unpatched source.

Patch 0003 adds `InventoryProfile::PushCursorFront(const ItemInstance&)`, an
explicit operation that clones and prepends without removing any queued item.
The caller retains ownership of its input. The clone has evolve-equipped reset,
matching the prior restoration path. Only the cursor branches of those two
callers change; ordinary slots still use `PutItem`. Cloning matters for bandolier:
its temporary instance is subsequently changed to one charge and equipped,
so transferring that pointer would corrupt the restored remainder.

- Third commit: `02ad89a6badc90fab1b7a3c14d2cb717b7355c46`, directly after
  `d4869f88f` in the same isolated EQEmu worktree.
- Third durable patch: [`../tools/eqemu-patches/0003-preserve-cursor-tail-on-popped-head-restoration.patch`](../tools/eqemu-patches/0003-preserve-cursor-tail-on-popped-head-restoration.patch).
- Apply patches 0001, 0002 and 0003 in order. All retain EQEmu's GPLv3-or-later
  license and remain undeployed and unpushed.

Four new tests exercise real `InventoryProfile` and `ItemInstance` objects:
bandolier remainder restoration followed by equipping one charge; a world
container's underlying item exchange, including sparse bag children and
unchanged tail pointers; restoring the only cursor head; and ordinary-slot
restoration leaving the cursor unchanged. **Before patch 0003: 91/93 tests
passed, with the bandolier and world-exchange scenarios failing; exit 1. After
patch 0003: all 93 passed; exit 0.** The baseline used the literal existing
`PopItem`/`PutItem` sequence against patch 0002. The passing tests differ at the
three cursor restoration calls, which use the new API. No nonexistent-API
baseline, mock queue, database or live character was used.

The changed `zone/inventory.cpp` also passes GCC 14/C++20 `-fsyntax-only` with
the cached zone feature definitions and dependency headers. The disposable
build overlaid the changed common header and implementation; it used no cached
precompiled header. Local and tested remote copies of all four changed files
have identical SHA-256 hashes. All three exported patches were applied in
sequence to pinned-baseline files, and all six resulting touched files match
the tested worktree byte-for-byte. `git diff --check` passes.

Local evidence is `/tmp/openeq-cursor-restore-before.log`,
`/tmp/openeq-cursor-restore-after.log`,
`/tmp/openeq-cursor-restore-zone-syntax.log` and
`/tmp/openeq-cursor-build-restore.sh`; corresponding logs and build scripts are
in the same remote disposable directory as the first two patches.

This establishes **in-memory ownership and clone independence**, not a full
execution of `Client::SetBandolier` or `Client::SwapItem`. Their actual source
compiles, but the utility scenarios do not execute their database or packet
side effects. World exchange already saves the full cursor after the branch.
Bandolier retains its existing single-slot `SaveInventory` calls, so its full
queue persistence and head publication remain separate follow-ups. No packet
semantics, database schema, deployment or restart changed.

### Bounded direct-caller audit and API contracts

| Operation or caller | Contract and audit result |
| --- | --- |
| `PutItem(cursor, item)` | Replaces the current head after patch 0002. Do not call it to restore an item after popping that head. |
| `PushCursor(item)` | Clones and appends at the tail. Correct for new queued ownership or rebuilding a fully drained queue; not a head-restoration operation. |
| `PushCursorFront(item)` | Clones and prepends without replacement. Patch 0003 uses it for the two already-popped cursor callers above. |
| `PushItem(slot, pointer)` | Transfers an allocated pointer through `_PutItem`; cursor use replaces the head. Its existing C++ caller in `RemoveDuplicateLore` explicitly skips cursor in the ordinary-slot loop. Do not silently change this API's semantics. |
| `InventoryProfile::SwapItem` | Reads instances with `GetItem`, without pre-popping, then uses `_PutItem` for actual replacement. `_PutItem` is intentionally unchanged. |
| Disenchant, no-rent and duplicate-lore cleanup | Drain the complete cursor, then append retained items in order. These are queue rebuilds, not a single already-popped head restored through replacement. |
| Guild-bank rejected deposit (`client_packet.cpp:7781,7802`) | Pops the head and re-appends it through `PushItemOnCursor(...,true)`, reordering a nonempty queue while publishing the appended item. Popped-pointer ownership, persistence and publication need a separate fix; patch 0003 does not change this path. |
| `MultiMoveItem` (`client_packet.cpp:11051`) | Handles bag children; the source explicitly excludes raw cursor items. |
| Disarm, invalid-slot relocation, trade completion and zone entry | The audited pop-and-append paths remove equipment, ordinary or trade slots, not a cursor head being restored. |
| NPC and bot inventory callers | Audited direct callers operate equipment/bot slots; no further same-cursor restoration path was identified. |
| Lua and Perl inventory bindings | Expose raw `PopItem`, `PutItem` and `PushCursor`. Scripts can compose an unsafe restore sequence. The new C++ method is not exposed to scripts by this patch. |
| World-container pickup | Uses the client `PutItemInInventory` wrapper, which appends cursor items; it is a separate contract from the patched exchange branch. |

This audit is bounded to the pinned C++ source and its exposed bindings. It
does not establish safety for arbitrary quest scripts or resolve the separate
wire limitations described below.

## Separate cursor-consumption wire follow-up

`Client::DeleteItemInInventory` initializes a local `inst` to null. Its cursor
branch saves the cursor but does not set `inst` to the surviving head. With
`client_update=true`, the packet selection consequently enters the whole
`OP_MoveItem` deletion branch even when `InventoryProfile::DeleteItem` returned
false and the head still exists. Simply fixing the underlying queue does not
make the packet describe partial cursor consumption correctly. Choosing
charge-versus-stack updates and exposing a newly promoted head require a
separate source/packet regression. Do not compensate by fabricating client
refunds or assume the ownership patch resolves this behavior.

## Optional negotiated synchronization direction

A future extension must be explicitly negotiated and reviewed as its own
client/server feature. A read-only snapshot response should identify its
request, connection epoch and queue revision; serialize complete ordered item
trees; and provide an explicit length/end marker. It must be ordered relative
to server mutations, including empty queues. Reject stale/incomplete snapshots.

A snapshot alone does not announce later silent changes. Supporting arbitrary
server cursor mutations also needs revision/invalidation coverage for those
paths, or explicit operation records; strong mutation preconditions require
server-checked expected revisions. No new opcode, stock request shape, or
read-only behavior is asserted here. Another possible source-backed direction
is to make the server's existing RoF+ head-only workaround internally
consistent; that requires a complete sender/promotion audit before changing
runtime behavior.

## Read-only audit: making the existing RoF+ head-only workaround consistent

This is a credible compatibility-fix direction worth testing before designing
a new protocol. It is not yet a proven native-client fix. The separately
reported2016 native replace-at-index result corroborates the hypothesis, but
is not a live trace from the exact supported RoF2 client.

Three source facts support it:

1. `SendCursorBuffer` explicitly describes its RoF+ workaround as sending the
   next cursor-buffer item to the visible cursor instead of having the client
   move buffered items. It returns immediately for older clients.
2. `PutLootInInventory` suppresses subordinate cursor receipts for RoF+, while
   sending Limbo for older clients. The comment warns that wrong subordinate
   serialization overwrites the visible cursor and desynchronizes the client.
3. RoF2 `ENCODE(OP_ItemPacket)` preserves packet type; `SerializeItem` and
   `ServerToRoF2Slot` encode ordinary cursor instances as possessions/main33.
   They do not convert appended tail instances into distinct buffer indices.
   Conversely, `RoF2ToServerSlot` maps every accepted native Limbo index to the
   server's one cursor head. `ConcatenateInvTypeLimbo=false` is additional
   metadata evidence, not by itself proof of native handling.

The initial send/push/head-mutation audit found these concrete paths:

| Path family | Present behavior | Required review for a head-only contract |
| --- | --- | --- |
| Login baseline (`client_process.cpp:749`, `client_packet.cpp:1791`) | Bulk includes head; unconditionally serializes each later tail as0x6a/main33 | RoF+ must not publish tails as visible-head replacements. Keep older-client buffering behavior separate. |
| `PushItemOnCursor(...,true)` and cursor `PutItemInInventory` | Publishes appended instance, even behind existing head | Central owned-cursor publication must fetch the actual head or suppress an unchanged head. Do not compare supplied pointers: push clones instances. |
| Summon, forage/fishing, spells, summoned bags, guild-bank withdrawal | Explicit `PushItemOnCursor(...,false)` followed by direct0x6a send bypasses the true-update helper | Audit `inventory.cpp:652`, `forage.cpp:368,507`, `spell_effects.cpp:1226,3440`, `client.cpp:11541`, `client_packet.cpp:7905`; changing only PushItemOnCursor is insufficient. |
| NPC/player returns, augments, evolution, Lua pushes | Common true-update helper or cursor PutItemInInventory | A corrected helper covers their tail sends, but their surrounding head deletions and scripted changes still need ordered publication. |
| Loot | Already suppresses RoF+ occupied-cursor tail sends; empty-cursor item uses0x67 | Preserve loot semantics and bag-child treatment. Empty-cursor receipt is an owned head, not a queue-size acknowledgment. |
| Ground pickup, bot/script ResetTrade, trader delivery | Append via false-update PutItemInInventory then direct0x67 at returned slot, potentially33 (`object.cpp:682`, `trading.cpp:196,974`) | Tail suppression limited to0x6a misses these owned-item senders. Ground duplicate-lore handling also deletes afterward. |
| Ordinary cursor moves, trade offers, world-container moves, explicit destroy/drop | Specific call sites invoke SendCursorBuffer (`inventory.cpp:813,1716,1976,2007,2215`) | Preserve counts, partial moves, swaps and lore filtering; avoid issuing a second head advance. |
| Generic deletion and consumption | DeleteItemInInventory does not invoke SendCursorBuffer; cursor wire mismatch remains | Charged/stack partial updates and whole-head promotion need explicit packet-level tests. Callers may use false-update because the client already predicts an action. |
| Guild-bank deposit | Success deletes cursor with false-update and no head refresh; rejection pops head then re-appends it (`client_packet.cpp:7781,7802,7831`) | Suppression alone would strand an unannounced next head; rejection currently reorders a multi-item queue. |
| Login cleanup and script mutations | No-rent/lore/disenchanted-bag paths rebuild queues before bulk send; Perl exposes RemoveNoRent; Lua/Perl expose deletion with optional false-update | Establish which paths are baseline-only and which need publication during a live session. False-update script mutations cannot be inferred from absent packets. |
| Corrections and nonowned views | SwapItemResync has deliberate0x67 token/actual sequence; augmentation rejection uses0x69; merchant/preview packets can also name cursor | Do not blanket-rewrite every cursor-addressed SendItemPacket into an owned-head update. Preserve packet provenance. |

The already-popped bandolier/world-container ownership bugs identified during
this audit are reproduced and fixed by patch 0003 above. Their remaining
packet and persistence behavior still needs separate validation. A wire-only
change cannot substitute for correct queue ownership.

The intended server contract would be: for RoF+, every **owned cursor item
update** represents the current visible head at the server operation boundary;
unexposed tails stay server-owned; every actual head change is published once
with source-backed existing packet semantics. Prefer a dedicated publication
helper over serial/pointer-based filtering in the generic packet sender. Keep
older clients on their existing buffer behavior. Do not implement only the
push helper and login loop and call that complete.

If established, this removes append-versus-refresh interpretation from ordinary
head-only item updates and allows provisional split identity to be replaced by
the next actual head serialization. It still does not reveal queue length,
provide a transaction acknowledgment, prove an empty queue, or correlate a
delayed pre-operation update with a later client prediction. SendCursorBuffer
currently sends nothing for empty queues. Those lifecycle/ordering limits
remain explicit, and clients would need an identified server capability or
verified deployment profile before relying on the changed invariant.

Before a runtime patch or native-compatibility claim, verify exact RoF2 and an
older supported client with login `[A,B,C]`, occupied-cursor summons, multiple
refunds, loot and bag tails, partial splits/merges, swaps, whole and charged
consumption, lore-skipped promotion, rejection/resync, and reconnect persistence.
Record actual head packets, server order and counts after each operation;
do not infer success from a UI that happens to show the expected item. No
head-only runtime code or deployment was changed by this audit.
