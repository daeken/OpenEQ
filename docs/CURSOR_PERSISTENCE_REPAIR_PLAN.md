# Lossless cursor persistence repair plan

Source review: September 30, 2026, EQEmu commit
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`.

This is a design and proof plan. No server source, database, schema, deployment,
or existing character was changed for this review. The five executed source
reproductions are in [CURSOR_PERSISTENCE_AUDIT.md](CURSOR_PERSISTENCE_AUDIT.md).
The [owned-state extension](OPENEQ_CURSOR_EXTENSION_V1.md) remains an observed
state protocol, not a durability or quest-transaction guarantee.

Follow-up implementation: [INVENTORY_SAVE_ERRORS_REVIEW.md](INVENTORY_SAVE_ERRORS_REVIEW.md)
records an isolated, tested leaf error-propagation patch; [ITEM_CLONE_REVIEW.md](ITEM_CLONE_REVIEW.md)
records the task-delivery copy fix. Both remain undeployed. They establish
prerequisites, not the operation-level ownership/rollback contract below.

## Recommendation

Use separate versioned cursor storage with explicit ordered roots and typed
child edges, written as an immutable generation inside the **outer inventory
operation's transaction**. Load it into a temporary item graph before publishing
any inventory. Keep the previous committed generation and exact legacy evidence
until the replacement is confirmed. Migrate callers and direct cursor-child
writes together; a new `SaveCursor` implementation alone is insufficient.

Separate two milestones:

1. Schema-free containment: expose failures, reject unrepresentable legacy
   saves before any destructive statement, and stop affected operations instead
   of reporting success. This does not make bags or large queues durable.
2. A schema, codec, loader, caller and migration change proven against an
   isolated real database. General cursor mutation is enabled only for paths
   whose ownership transfer and failure handling have passed that proof.

Containment must be introduced with the caller changes: some current trade and
barter paths return a copy to the giver on insertion failure after the recipient
already owns it in memory. Do not deploy leaf error checks alone or add a new
deterministic oversized-queue failure before repairing that ownership boundary.

Increasing `CURSOR_BAG_END`, widening a local counter, assigning another magic
slot range, saving only the first 200 roots, or encoding only augment IDs cannot
satisfy the required invariants. A raw `ItemInstance::Serialize` blob is also
invalid: that API serializes an in-process pointer, not an item record.

## Source facts that constrain the repair

Paths below refer to the pinned EQEmu checkout, not proposed implementation.

| Source | Consequence |
| --- | --- |
| `common/shareddb.cpp:151–183` | SaveCursor deletes the legacy cursor range, ignores that delete result, saves at most 200 roots and returns success after truncation. Its iterator parameters are references and the start iterator advances. |
| `common/emu_constants.h:217–234`; `common/patches/rof2_limits.h:225–226`; `common/inventory_profile.cpp:1086–1105,1240–1255` | The server container index range is 200 slots. The visible cursor's children and persisted tail roots occupy the same range. Queued root slots do not support containers. Do not hard-code a ten-slot assumption. |
| `common/shareddb.cpp:211–295,348–378` | SaveInventory clears supported containers before replacement without checking the clear result. UpdateInventorySlot ignores child-save results and enumerates only the template BagSlots range. DeleteInventorySlot confuses zero rows affected with failure. |
| `common/repositories/base/base_inventory_repository.h:407–463` | GetWhere returns an empty vector on either no rows or query failure; DeleteWhere returns zero for both successful no-op and failure. They cannot express the required persistence outcomes. The generated repository is not the appropriate place for bespoke transaction policy. |
| `common/shareddb.cpp:612–845` | GetInventory mutates the destination while reading, skips invalid definitions, treats all cursor-range rows as roots, and writes regenerated GUIDs back through unchecked ReplaceMany. An inventory with no rows is treated as a load failure. |
| `common/shareddb.cpp:700–803` | Load uses template Hero's Forge data instead of the stored ornament override, recreates augments from IDs, normalizes some charge values, and associates evolving records by item ID rather than instance identity. These are additional source observations, not new executed defect reproductions. |
| `common/item_instance.h:224–230,336–372`; `common/item_instance.cpp:107–165,809–842` | Serialize contains a pointer; Clone preserves serial numbers; contents are shared storage for bag children or augments. Custom data's caret encoding is not a general lossless string-map encoding. The clone constructor also does not copy every scalar, such as task-delivery count. |
| `common/dbcore.cpp:72–123,175–187`; `common/dbcore.h:39–68` | Each statement takes a recursive mutex separately; lost queries retry once by default, and a disconnected handle may reopen even when per-query retry is disabled. Begin and rollback discard their results. There is no checked transaction owner/depth contract. MYSQL handles can be shared through SetMySQL. |
| `zone/client_packet.cpp:10931–10950`; `zone/corpse.cpp:369–402` | MoveItem and corpse creation already wrap operations that reach SaveCursor in transactions. Adding START TRANSACTION inside SaveCursor risks committing an existing MySQL transaction. |
| `zone/client_packet.cpp:1360,1786–1805` | GetInventory failure primarily suppresses inventory packets; it does not itself establish a fail-closed character session. The repair must stop entry and later saves when authoritative inventory could not be loaded. |

The schema manifest's inventory conversion at version 9298 explicitly defines
`PRIMARY KEY(character_id, slot_id)` and a medium unsigned slot column. A new
representation must not rely on the generated repository's single-field
`PrimaryKey()` string as if it described that composite key. The installed
schema, storage engines and durability settings still require verification in
an isolated database; source migrations alone do not prove a live installation.

## Complete caller inventory and transaction boundaries

The pinned tree has 13 textual SaveCursor call sites: 12 in
`zone/inventory.cpp`, including one unreachable duplicate branch, and one in
`zone/corpse.cpp`. All reachable sites must be converted or explicitly blocked.

| Location / operation | Current handling | Required operation boundary |
| --- | --- | --- |
| `inventory.cpp:815`, DropItem | Ignores result; memory/quest callbacks have already changed; ground object publication follows. | Source removal and durable ground-object ownership together; publish the object only after commit. Irreversible script effects need a separate audited boundary. |
| `:992`, DeleteItemInInventory | Void API; mutates memory and may soft-delete evolving state before ignoring the save result. | Item/stack deletion, evolving state and outgoing deletion packet together. |
| `:1047`, PushItemOnCursor | Returns bool, but mutates memory, performs evolving checks and may send the item first. | Stage the append and dependent evolving writes before publishing. Callers must consume the result. |
| `:1074`, PutItemInInventory | Redundant cursor branch is unreachable after the earlier delegation to PushItemOnCursor. | Remove the ambiguity when replacing the API; the real cursor path is the delegated one. |
| `:1094`, PutLootInInventory | Void API; ignores result; bag children are handled later, and occupied-cursor bags may intentionally become separate roots. | Construct the intended complete post-operation graph first; corpse/source removal and all destinations commit together. Preserve actual source semantics rather than guessing a bag regrouping. |
| `:1298`, MoveItemCharges | Void API; changes both charge counts and sends loot data before saving. | Both source and destination stacks, including the cursor, in one operation. |
| `:1979`, SwapItem world-container path | Ignores result; world-container writes already occurred. | World container and carried/cursor changes together, or disable that path until its storage can participate. |
| `:2218,:2226`, ordinary SwapItem | Both endpoint saves are ignored; caller returns true. | Source, destination, currency/evolving side effects and cursor generation in the caller's transaction. Save the final cursor once. |
| `:2682`, DisenchantSummonedBags | Rebuilds the queue, deletes old instances, ignores result. | Stage the entire transformation, retain a recoverable preimage, publish only after commit. |
| `:2790`, RemoveNoRent | Destructive login cleanup ignores result. | Run only after a successful complete load, as a separate checked inventory operation. |
| `:2894`, RemoveDuplicateLore | Destructive login cleanup ignores result. | Same; do not treat corruption/migration ambiguity as permission to delete a supposed duplicate. |
| `corpse.cpp:394`, corpse construction | Existing outer transaction ignores cursor, character-save and commit results. | Corpse rows, inventory removals, cursor, cash/evolving ownership and character state need one checked owner. |

Returning false from SaveCursor is not sufficient: 11 textual sites ignore the
result, and the reachable bool-returning push already publishes before it can
fail. Transitive callers also matter: trade refunds, loot, merchant operations,
augment insertion/removal, quest APIs and summon paths often ignore
PushItemOnCursor/PutItemInInventory results. For example, trade return paths in
`zone/trading.cpp:319–507` can send items to either player, the cursor or carried
slots; augment operations in `zone/client_packet.cpp:3340–3483` have several
related writes even where an individual bool is checked.

Direct `SaveInventory(character_id, inst, slot_id)` must not bypass the new
cursor representation. Cursor child slots occur through runtime slot IDs, not
just literals. Concrete examples are `PutLootInInventory`'s child loop
(`inventory.cpp:1111–1148`), four `InventoryProfile::SetCustomItemData` overloads
(`:3625–3655`), and MoveMultipleItems (`client_packet.cpp:11040–11065`). That
handler also calls SwapItem without the ordinary MoveItem handler's enclosing
transaction. Audit direct Update/DeleteInventorySlot and repository writes as
well as SaveCursor. A grep for a literal slotCursor argument is not coverage.

Other explicit transaction sites are `Database::CopyCharacter`
(`common/database.cpp:1855–2050`) and instance purging
(`common/database_instances.cpp:571–584`). They are not demonstrated direct
SaveCursor callers, but a replacement transaction layer must remain correct for
all users and shared MYSQL aliases. `Client::Save` (`zone/client.cpp:981–1110`)
performs many unchecked writes and returns true; calling it within the corpse
transaction is not itself evidence of durable success.

## Required preservation contract

For an accepted operation, reloading its committed representation must preserve:

- Every root, its FIFO ordinal, and the distinction between an empty queue and
  a failed/incomplete read. Root count is independent of protocol slot numbers.
- Every non-null child and its exact typed edge: bag index or augment socket.
  Duplicate item IDs and equal native serials are distinct nodes. No deduplication
  by item ID, sorting by GUID, first-200 limit, or flattening bag contents.
- All supported instance attributes at **every** node, including augments:
  signed charges, color including zero, attunement, custom key/value bytes,
  ornament icon/IDFile/Hero's Forge overrides, native serial and evolving identity
  plus its associated state. Root-only attribute serialization is insufficient.
- One committed ownership outcome for all participants of the operation. A
  cursor-to-slot move must not resurrect the old cursor after also persisting
  its destination; a refund must not leave both sender and recipient owning it.
- The exact previous durable state after a confirmed abort. An unconfirmed
  commit produces an explicit unknown result, with further mutation fenced,
  not an automatic replay or a claimed rollback.

An item definition ID is a reference, not an instance identifier. Keep a new
storage node/instance identity separate from the native serial; clones preserve
native serials, and the legacy GUID column is uint64 while the runtime serial is
int32. Reject or quarantine an unrepresentable legacy value without narrowing
it. Restore supported serials explicitly rather than allocating a different
value and rewriting the database as a side effect of login.

Inventory attributes beyond today's inventory columns need an explicit codec
contract. `ItemInstance` also contains scaling/experience and scaled item data,
new IDFile, recast state, price/merchant state, task-delivery bookkeeping and
quest timers. Before claiming full attribute parity, inventory every field and
assign it to one of: stored/restored, derived from an explicitly versioned source,
or deliberately ephemeral with documented lifetime. Storage must preserve
unknown/versioned bytes when it cannot interpret them; unsupported live values
block activation/mutation rather than being silently reset. Persisting a timer
must specify elapsed-time policy and must never replay an already-fired quest
callback. No raw pointer, C++ memory image or wall-clock guess is a valid policy.

Expose a const, complete contents/map visitor for the codec. Do not enumerate
only template BagSlots or only valid-looking augments and omit the rest. Validate
all actual map entries, cycles, repeated pointer ownership, edge indices and
item classes. Invalid graphs remain in preserved custody and block publication;
calling them invalid does not authorize dropping their children. A memory
rollback must use an audited full preimage; the current Clone is not sufficient
proof that every field was copied.

## Proposed schema and codec

Use new InnoDB tables rather than repurposing `inventory.slot_id`. Final DDL and
manifest version are implementation deliverables, not assumed existing tables.

| Proposed table | Essential fields and constraints |
| --- | --- |
| `character_cursor_state` | Character PK; format/codec version; active generation ID; monotonically checked revision; migration/blocked status; writer fencing epoch. Explicit state exists even for an empty cursor. |
| `character_cursor_generations` | `(character_id, generation_id)` PK; unique `(character_id, operation_id)`; predecessor/revision; root/node/edge counts; canonical payload digest; codec version; provenance. Immutable after commit. |
| `character_cursor_nodes` | `(character_id, generation_id, node_id)` PK; item definition ID; versioned pointer-free attribute payload and checksum. Node IDs are positive and unique within this generation, not derived from item ID or native serial. |
| `character_cursor_edges` | Character/generation, parent node, edge kind, index, child node. Parent zero denotes the virtual queue root; only root edges use it. Root index is a 64-bit ordinal. Unique `(character_id,generation_id,parent,kind,index)` and unique child ownership. All non-root parents must resolve to nodes in that generation. |
| `character_cursor_legacy_archive` | Immutable source schema/version, exact source key/column bytes including NULLs, extraction identity/digest and migration disposition. Archive all cursor-related rows and necessary context; preserve raw attributes before parsing. |

Edges encode root order and children separately. Root ordinals must be contiguous
for an activated generation; child indices retain their original values and
holes. Foreign keys should protect node/generation references where practical;
application validation must enforce the virtual-root exception, edge typing,
acyclic reachability, counts and digest. Do not depend on NULL uniqueness or
DB-version-dependent CHECK behavior for ownership constraints. Store integer
widths explicitly and validate counts before conversion to runtime indices.

Use a versioned, length-delimited binary codec (or an equivalently exact typed
encoding) for attributes. Custom data must be an explicit map of byte strings,
not the existing caret-delimited string. Keep scalar types and signedness;
encode evolving doubles without lossy decimal conversion. Legacy custom-data
raw bytes are preserved separately because their original map can already be
ambiguous. Do not serialize item pointers or regenerate augmented instances
from six item IDs.

Evolving instances must reference their exact `character_evolving_items.id` and
owner, with changes made inside the same transaction. Never select the first
matching item_id when two instances have the same definition. Legacy rows do
not supply a general instance-to-evolving-row mapping; conflicting or missing
links require preserved evidence and explicit resolution, not guessed pairing.
The snapshot's evolving payload and referenced row must agree before activation.

Retain previous generations and the operation record through proof and unknown
commit resolution. Garbage collection is a separate audited operation after
retention requirements are met; never delete the prior generation in the write
that first makes a replacement visible. Capacity limits are admission controls
checked before mutation. Crossing a limit must refuse the entire new operation
while retaining ownership, not truncate an existing queue or fabricate success.

## Transaction ownership and failure behavior

Introduce a checked transaction scope shared by every alias of the same MYSQL
handle. It holds exclusive connection ownership across all statements, tracks
the connection generation, records rollback-only failure, and returns checked
begin/commit/rollback results. Disallow reconnect and statement retry while the
scope is active, including the unconditional pre-query Open path. A disconnected
scope cannot resume on a new connection. Prepared-statement failure must enter
the same failure state. Forbid DDL, TRUNCATE, implicit-commit statements and
unowned transaction control inside inventory scopes; stored procedures require
an explicit transaction-safety audit before admission.

Existing operations join the outer scope with an explicit token. A helper
never starts or commits a nested transaction. A checked savepoint can be added
later for a genuinely recoverable sub-operation; it does not turn an inner
success into durable success. Prefer a rollback-only joined scope for this
repair. Replace ambiguous bools with outcomes such as Staged, Committed,
Aborted, Conflict and Unknown. A compatibility bool wrapper may return true
only for a completed standalone commit; transaction-internal callers use the
staged API and let the owner decide publication.

A concrete write sequence is:

1. Hold the inventory operation/session fence; construct and validate the full
   candidate ownership graph and exact preimage before destructive effects.
   Allocate one operation ID. Buffer client packets and externally visible
   ownership changes until the operation's outcome is known.
2. Begin/join the checked SQL scope. Lock every affected character/state row in
   deterministic order; compare expected revision and writer epoch. Include
   shared-account, corpse or world-container ownership locks where applicable.
3. Insert the complete immutable generation, nodes and edges using success-aware
   queries. An empty generation is valid. Validate expected counts/digests; a
   duplicate operation ID with different content is an error, not an overwrite.
4. Stage all related inventory/currency/corpse/evolving rows, then compare-and-
   swap the active generation/revision. All involved tables must be transactional.
   Do not use the old repository's affected-row-only result for success checks.
5. Commit exactly once at the operation owner. Only Committed publishes the
   candidate state, releases packets and permits the next operation. Confirmed
   rollback restores the full preimage and preserves the prior database state.
6. On connection loss or commit acknowledgement loss, fence mutations and retain
   the operation ID, snapshot and prior generation. On a separate verified
   connection, resolve the immutable operation record and locked state after
   the old transaction has finished. A matching committed record proves the
   whole atomic operation only if every participant was in that same transaction.
   An absent record is not immediately proof of rollback while the original
   server session could still hold an uncommitted transaction. Never replay an
   unknown operation automatically or restore an old generation over a possibly
   committed transfer.

Native handler callbacks and quest scripts may already publish non-database
side effects or call arbitrary APIs before a save. Merely adding a SQL rollback
cannot undo those effects. Stage only paths whose effects can be controlled;
otherwise explicitly disable that mutation path for the repaired mode until its
boundary is implemented. Full quest durability would require its own bounded,
idempotent/outbox or operation-journal design. This plan does not invent generic
rollback for arbitrary Lua/Perl quests or claim exactly-once quest execution.

## API routing and load strategy

Add an inventory operation context that owns the authoritative InventoryProfile
and resolves a live cursor head/child address into a typed graph path. Replace
legacy cursor-range writes with `StageCursorSnapshot(context)` or an explicit
path update followed by a complete snapshot. `SaveInventory(char, inst, slot)`
alone cannot discover the rest of a queue; a fallback write to a magic slot is
forbidden. Direct cursor-range writes without the required context must fail
before SQL. Internal queued-root ordinals never become native client slot IDs.

On zone entry, read the state pointer, generation, graph and dependent evolving
state from one consistent transaction/snapshot. Return typed results: valid
empty, complete, blocked legacy, unsupported codec, corruption or database
failure. Parse and validate into a temporary owner. Restore attributes on every
node, attach children with their exact edge kind/index, then publish the full
FIFO once. Avoid `GetTotalItemCount` as a wide count source: its return type is
uint8. The loader must not mutate legacy rows, create missing evolving rows,
normalize unsupported charge values, skip unknown items, or regenerate GUIDs.

Unknown definitions, duplicate edges, dangling nodes, invalid indices, missing
records, changed hashes or unsupported codecs fail the complete load while
preserving stored bytes. Do not fall back to legacy rows when a v2 marker exists
but its generation is unavailable. Only after successful load may intentional
NoRent/lore/other cleanup run as separately checked operations with before/after
records. Stop character entry and all automatic saves after a blocked load;
merely suppressing BulkSendInventoryItems is insufficient.

Cursor ownership can subsequently leave the cursor. If the destination's legacy
inventory/corpse/shared-bank representation cannot preserve a node's attributes,
that transfer must remain disabled or its destination storage must receive the
same lossless codec/identity support first. A perfect cursor-only table followed
by an ID-only augment save in another slot is not end-to-end preservation.

## Legacy migration and compatibility

Use a staged, offline/read-only classification tool and a separate explicitly
controlled activation step. Do not auto-migrate on login through today's
GetInventory: it already changes and writes the representation while loading.

- Capture exact rows, schema fingerprint, definitions/version references and
  relevant evolving records from a consistent source snapshot. Seal the export
  before interpreting it. Include the surrounding inventory/ownership context
  needed to distinguish known participants; archive cannot recover missing rows.
- Mark ambiguous cases blocked. A head bag and cursor-range rows cannot generally
  distinguish child1 from tail1; overwritten or omitted children are absent.
  Contiguity, charges, item class, GUID order, bag capacity, or the current
  loader's output do not prove the original topology. Stale tails after a failed
  delete are another ambiguity. Do not attach, split, duplicate or discard rows
  using a heuristic.
- Augment IDs alone do not recover augment attributes. Existing caret strings,
  the charge sentinel (all negative runtime charges were saved as 32767), serial
  width mismatches and evolving associations can also be ambiguous. Preserve
  raw evidence and report the missing information. Never label default-created
  augments an exact recovery of a historical instance.
- A fresh dedicated character with explicitly known empty state, or a complete
  trusted source snapshot with provable tree ownership/attributes, is a viable
  first migration fixture. An empty legacy row set proves only that no cursor
  rows survived; it cannot prove there was never historical loss. A single known
  head row may have clear slot ownership yet still lack attribute provenance.
  Require a documented classification rather than declaring all ordinary-looking
  queues safe. Fully ambiguous production queues remain quarantined until an
  explicit evidence-backed repair decision; unrelated playable ownership must
  not be duplicated as an attempted workaround.
- Activation verifies the source digest has not changed, writes the exact new
  generation and explicit state marker, and archives/retires legacy cursor rows
  atomically. Keep backups and provenance. Never dual-read legacy and new roots
  or let old code rewrite a projection after conversion.
- Roll out schema readers and operation routing to every relevant writer before
  enabling converted characters. Reject old writer binaries and incompatible
  tools at startup; a version marker is ineffective if an old binary ignores it.
  Include world/zone administrative tools, character copy/delete/export/import,
  inventory snapshots and restoration. Update DatabaseSchema character-table
  registries and those lifecycle operations for generations and archives.
- Downgrading to the overlapping legacy format is permitted only after a proved
  exact representability check and explicit maintenance operation. Bags,
  oversized queues or richer attributes cannot be exported by truncation. If
  lossless downgrade is impossible, keep the character blocked on the older
  binary and retain the new data; restoring only cursor history over later
  ownership changes would create duplication.

## What can ship without a schema migration

Failure-aware SQL results, checked child/delete propagation, a preflight count
and graph representability check, failure logging, load error distinction, and
transaction ownership plumbing can be implemented and proven separately.
Successful zero-row DELETE is success; SQL error is not. Do not simply add
`if (!DeleteInventorySlot(...))` to the current code, since its bool is already
wrong for a legitimate empty slot.

For legacy mode, conservatively reject a save whose entire graph cannot round-
trip through the legacy representation, before the cursor preclear. The audit's
head/tail bags and 201-root queue are required rejection cases. This is a
containment restriction, not acceptance of fewer items: the originating action
must preserve the existing items and newly offered item's owner. Rejecting only
at the post-mutation SaveCursor site still permits loss in memory or another
owner and is not a safe release. Caller handling must accompany containment.

Schema-free work cannot recover ambiguous legacy rows, represent arbitrary
ordered roots/children, preserve all augment-instance attributes, or establish
a durable operation identity for resolving an uncertain commit. Those require
the new representation and caller/load migration. Do not enable general bag or
large-queue hand-ins based on the containment milestone.

## Implementation and proof stages

1. **Freeze the contract and enumerate routes.** Produce field-by-field codec
   and lifetime tables, all SaveCursor/SaveInventory/direct-repository routes,
   transaction owners and every unsupported mutation gate. Add result-aware
   query/transaction interfaces without deploying them. Retain the original
   five-defect reproducer as a baseline, clearly distinct from repair tests.
2. **Schema-free containment proofs.** Use actual source objects with the strict
   SQL seam. Assert no DELETE or other mutation for unrepresentable graphs;
   initial-delete, parent-write, child-write and recursive-delete failures cannot
   return success. Assert successful no-op deletion does not spuriously fail.
   Exercise callers rather than only leaf return values.
3. **Codec/schema/load proofs.** In a disposable database with production-equivalent
   schema definitions but no production rows or credentials, use actual item
   classes and the proposed loader. Round-trip empty, 1, 199, 200, 201 and larger
   queues; a head bag plus tails; tail bags; sparse indices up to supported limits;
   augmented bag children; equal item IDs/serials; every supported attribute,
   exact custom bytes and evolving instances with equal item IDs but distinct
   IDs. Compare a canonical deep graph, not merely row count or item IDs.
4. **Failure and concurrency matrix.** Inject failures at begin, lock, every row
   and edge write, active-pointer update, dependent save, commit and rollback;
   disconnect before/after each boundary. Kill the disposable server process
   before commit, after commit and before its response is observed. Prove no
   automatic SQL replay/reconnect within a transaction; zero partial loads;
   previous generation retained; empty generation distinct from read failure;
   unknown outcomes fenced and resolvable by operation ID. Test two writers,
   stale revisions, shared MYSQL aliases and attempted nested scopes. Check real
   storage engines and database durability/recovery configuration explicitly.
5. **Operation proofs.** Run the actual MoveItem and MoveMultipleItems paths,
   cursor append/delete/stack, bag child updates, custom data setters, loot and
   refunds, corpse creation, augment changes and login cleanup. Verify all
   participating owners/attributes before and after reconnect. Confirm packets
   and quest-visible success are not published on abort. Unsupported side-effect
   paths must be demonstrably blocked, not silently outside the assertion set.
6. **Migration/lifecycle proofs.** Import sealed unambiguous fixtures; ambiguous
   fixtures remain byte-for-byte archived and blocked. Simulate interruption
   before/after activation, old writer startup, missing generation, backup and
   restore, character copy/delete and refused lossy downgrade. Never infer an
   expected item tree from the same faulty loader under test.
7. **Reviewable rollout proposal.** Only after those stages, prepare exact schema
   and source patches, compatibility gates, sealed backup/restore procedure,
   characterization report and dedicated-account proof steps. No live migration,
   restart, compensation or deletion is authorized by this design document.

Completion means a fully admitted graph and its related ownership operation
survive checked save/reload with exact supported attributes, or the operation
is refused without changing ownership. A green test for a small ordinary cursor
is not a substitute for the bag, large-queue, ambiguity and commit-loss proofs.
