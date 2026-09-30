# Cursor persistence: isolated source reproductions

September30,2026. Five defects were reproduced against pinned EQEmu
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`. **No fix, migration, deployment or
live-character persistence claim is included here.** This is a prerequisite for
the proposed [owned-state extension](OPENEQ_CURSOR_EXTENSION_V1.md), separate
from the three existing [in-memory ownership fixes](CURSOR_RECONCILIATION.md).

## Results

| Case | Actual source result |
| --- | --- |
|201 ordinary cursor roots | `SaveCursor` returns true after saving200; the queue still contains201, and its iterator has one unsaved root remaining. |
| Ordinary head followed by a bag containing two items | Both roots are saved; neither child is saved. The loader's slot-dispatch boundary reconstructs an empty queued bag. |
| Head bag with children0/1 followed by an ordinary tail | Head child1 and tail root1 use the same inventory row key. Saving the tail overwrites that child. The loader dispatches surviving child0 as a separate cursor root; the head bag is empty. |
| Initial cursor-range DELETE fails | `SaveCursor` ignores the failure and returns true; a stale tail row survives alongside the new head. |
| A child REPLACE fails | `UpdateInventorySlot` ignores the recursive child's save result and reports the successful parent write; `SaveCursor` returns true with the child missing. |

The first three results use only successful simulated SQL operations. They do
not depend on a connection failure. The last two inject a failed
`MySQLRequestResult` at exactly the stated boundary. A failed parent/root write
already returns false; this audit does not claim that all failures are ignored.

Source anchors:

- `common/shareddb.cpp:151–183`: preclear,200-root limit, tail slot allocation
  and successful return after truncation.
- `shareddb.cpp:211–246,249–295,348–378`: real SaveInventory, parent/child
  persistence, unchecked recursive result and deletion paths.
- `shareddb.cpp:803–807`: cursor-range rows become `PushCursor` roots instead
  of being attached as children.
- `common/inventory_profile.cpp`: `SupportsContainers` excludes queued-root
  slots; `CalcSlotId(cursor,child)` occupies the same range used for queue tails.

## What the fixture actually executes

The reviewable diagnostic is
[`cursor_persistence_probe.cpp`](../tools/eqemu-tests/cursor_persistence_probe.cpp).
Its successful exit means it reproduced all five known defects on this pinned
source; it is **not** a passing test of a corrected implementation.

The executable links the real cached `SharedDatabase` save methods and real
`ItemInstance` implementation from `libcommon.a`, plus the actual
`InventoryProfile` object containing ownership patches0001–0003. Those patches
do not change SaveCursor or the persistence slot mapping. No inventory/item
classes or save routines are reimplemented in the diagnostic.

GNU ld wraps the exact `DBcore::QueryDatabase(const std::string&,bool)` symbol.
Only inventory DELETE/REPLACE statements for synthetic character424242 are
accepted. A small in-memory row map models the unique character/slot key and
records item IDs/charges. Unknown statements abort. This seam does not emulate
all SQL, transactions, item metadata or a real database's failure behavior.

For the two reconstruction observations, the diagnostic replays only the
five-line slot dispatch from `GetInventory` using the saved rows, synthetic
item definitions and a real RoF2 InventoryProfile. It does **not** execute the
full GetInventory query/augmentation/evolving-item pipeline or a live reconnect.
Save-time omission/overwrite is independently visible before this replay.

The fixture additionally wraps `mysql_real_connect` to refuse connections. It
ran under an isolated Linux network namespace, dropped to the ordinary user,
with an empty environment in a disposable directory. It reads no production
configuration or credentials. All220 statements were intercepted; no database
connection, live SQL, server restart or fixture mutation occurred.

## Build and evidence

Local evidence: `/tmp/openeq-cursor-persistence-audit`, including the compiled
source copy, exact build command, preserved setup failures and final
`result-reviewed.log`. Remote disposable build:
`/tmp/openeq-cursor-persistence.l4pfJa`.

The build used GCC14/C++20, the pinned cached source and dependencies under
`/home/daeken/eqemu-bootstrap/{source,build}`, and
`/tmp/openeq-cursor-regression.alUt1w/inventory_profile.o`. The latter is the
previously tested actual source with patches0001–0003. Link it before
`libcommon.a`, with these additional GNU linker flags:

```
-Wl,--wrap=_ZN6DBcore13QueryDatabaseERKNSt7__cxx1112basic_stringIcSt11char_traitsIcESaIcEEEb
-Wl,--wrap=mysql_real_connect
```

The initial fixture needed two setup corrections: allow the repository's two
spaces before VALUES in the strict parser, and initialize RoF2 inventory
limits for the loader dispatch. Both corrections are in the committed fixture.
Independent source review checked the SQL seam and bag slot mapping. The final
run additionally asserts the cloned input bag children exist and records the
exact child-to-tail overwrite; it reproduced all five defects. This is a focused actual-object
diagnostic, not a full EQEmu rebuild or a claimed persistence repair.

## Required follow-up

Do not enable general bag/large-queue hand-ins based on a correct in-memory
snapshot alone. A durable solution needs distinct, lossless root/child storage,
an explicit compatible migration/load policy, checked recursive writes and a
transaction boundary that does not erase the previous persisted queue before
discovering a failure. Existing ambiguous rows may already have lost their
original ownership; do not guess a reconstruction or rewrite live inventories.

Rejecting a201-root save before deletion would prevent one truncation but
would not solve growth beyond capacity, bag aliasing, existing ambiguous rows
or ignored callers. A complete repair needs caller/result handling and
actual isolated-database save/reload tests before any rollout. The proposed
extension's initial ordinary-item proof remains bounded to small queues and
must describe receipts as observed state, not a durable quest transaction.
