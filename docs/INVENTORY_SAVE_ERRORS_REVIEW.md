# Inventory save error propagation: isolated server patch

Reviewed September 30, 2026 against EQEmu
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`.

Actual-source tests reproduce eight failures in ordinary inventory persistence.
The patch reports SQL errors, stops subsequent writes, and treats successful
zero-row deletes as success. **It remains undeployed.** Accurate leaf results
do not make a multi-statement save atomic or repair callers that already changed
ownership and sent packets before checking the result.

## Change and boundaries

- `SharedDatabase::SaveCursor` checks the initial range clear before saving roots.
- Ordinary `SaveInventory` checks its container clear before replacement.
- `UpdateInventorySlot` stops after a failed parent replacement and propagates
  every recursive child save or delete result.
- `DeleteInventorySlot` checks query success rather than affected-row count.
  Even when the parent row is absent, supported child rows are still cleared.

The generated repository's `DeleteWhere` returns zero for both SQL failure and
successful no-op deletion. These three delete sites therefore use the actual
query result's `Success()` directly, retaining their original SQL predicates.
Parent `REPLACE` still uses the repository: a successful replacement affects
one or two rows and can retain its existing nonzero-result check.

Shared-bank handling, schema, transaction ownership, bag mapping, queue limits,
load behavior and wire messages are outside this patch. The reference cursor
iterator still advances once per successful root and points at the failed root
on failure. **Do not retry from that iterator:** the next SaveCursor invocation
would clear the whole cursor range, including previously saved roots.

## Actual-source regression

Patch [0005-propagate-inventory-save-errors.patch](../tools/eqemu-patches/0005-propagate-inventory-save-errors.patch)
contains the production change and `tests/inventory_save_errors_test.cpp`.
The isolated EQEmu commit is `9adae85c6` on `codex/cursor-partial-consumption`,
following the three earlier cursor ownership patches in
`/tmp/openeq-eqemu-cursor-fix`. The patch itself changes no InventoryProfile code.
The EQEmu source/tests remain GPLv3 or later.

The test links real SharedDatabase, ItemInstance and InventoryProfile code.
A GNU linker seam replaces only DBcore's SQL execution with a strict in-memory
model of the inventory table's composite character/slot key. Its parser accepts
only the exact pinned inventory column order and the synthetic fixture's
DELETE/REPLACE forms for character 424242. Unknown SQL throws instead of being
forwarded. A second wrapper denies `mysql_real_connect`.

Failures return an actual failed `MySQLRequestResult`; successful statements
return its success form with the affected-row count, including zero. Per-case
statement journals prove no later statement executes after the reported error.
The seam stores item IDs and charges to verify ownership footprints; it is not
a general SQL engine, a persistence codec test, or a live database proof.

| Case | Before | After |
| --- | --- | --- |
| Empty cursor clear | pass | pass |
| Failed cursor preclear stops writes and preserves iterator | fail | pass |
| Successful zero-row ordinary delete | fail | pass |
| Absent container parent still clears orphan children | fail | pass |
| Failed parent delete stops replacement | fail | pass |
| Failed child-range clear reports partial deletion | fail | pass |
| Empty bag, including no-op child deletes | pass | pass |
| Failed parent replacement stops child writes | fail | pass |
| Failed child replacement retains earlier writes | fail | pass |
| Failed null-child delete stops later children | fail | pass |
| Successful carried bag and cursor queue coexist | pass | pass |
| Later cursor-root failure preserves failed-root iterator | pass | pass |

The same final strict fixture passed 4/12 against the original SharedDatabase
object (exit 1), then 12/12 against the modified translation unit (exit 0).
The added `inventory_save_errors_tests` CMake target also built and passed 12/12.
It is gated to Linux/GCC because the seam uses GNU wrapping and the libstdc++
C++11 string ABI. It is separate from the existing utility runner.

Builds used GCC 14/C++20 in disposable remote directory
`/tmp/openeq-inventory-save-errors.mMyr1d` on storage2. The pinned source and
matching common archive/dependencies under `/home/daeken/eqemu-bootstrap`
were read-only. A separately compiled, complete modified `shareddb.cpp` and
the actual previously tested cursor-fix InventoryProfile object were linked
before that archive. The original SharedDatabase archive object was used for
the before run. A disposable CMake wrapper imported the same dependencies and
built the patch's actual test target; no cached build was reconfigured.

All three final executions ran as the ordinary user in an isolated network
namespace with an empty environment. No configuration, credentials, live SQL,
character login, database change, server restart or deployment was involved.
This is a focused actual-source build, not a full server rebuild.

Local evidence directory `/tmp/openeq-inventory-save-errors` contains
`result-before-reviewed.log`, `result-after-reviewed.log`, `result-cmake.log`,
the build scripts and disposable wrapper `CMakeLists.txt`. Initial direct-build
include-path errors were resolved by using quote-only lookup for `common`:
adding it as a general include directory shadows the system `features.h`.
The final direct and CMake builds passed without modifying production headers.
The durable reproduction is the test source in the patch. The final target
declaration is placed before the existing install block so it composes with
patch0004's separate target; reconfiguration and all12 cases passed again in
`result-cmake-final.log`. All six patches apply in order from the pinned base,
and the combined utility suite passes98/98 as recorded in
[ITEM_CLONE_REVIEW.md](ITEM_CLONE_REVIEW.md).

With a normal EQEmu build configured with `EQEMU_BUILD_TESTS=ON` on Linux/GCC:

```sh
cmake --build /path/to/build --target inventory_save_errors_tests
/path/to/build/bin/inventory_save_errors_tests
```

## Caller trap and remaining work

`PushItemOnCursor` and `PutItemInInventory` change in-memory inventory and may
send the item before saving. Some trade and barter callers interpret a false
return by returning another copy to the giver, without undoing the recipient's
insertion. Exposing more accurate errors can therefore expose duplicate
ownership unless the surrounding operation is redesigned. Other callers simply
discard the result. This patch is a reviewable prerequisite, not a rollout.

Do not add deterministic rejection of oversized queues until those caller
semantics are repaired: that would introduce a new false path after a successful
in-memory insertion. Do not add an inner transaction to SaveCursor; MoveItem and
corpse paths already own outer transactions, and DBcore reconnect/retry behavior
also needs a checked transaction contract.

The tests deliberately retain earlier writes after a later failure. A false
return does not promise rollback, restored memory, or unchanged packets.
Cursor bag/tail row aliasing, omitted queued bag children and truncation at
200 roots remain reproduced defects. Continue with the operation boundaries and
lossless storage proof in [CURSOR_PERSISTENCE_REPAIR_PLAN.md](CURSOR_PERSISTENCE_REPAIR_PLAN.md)
before enabling arbitrary queued hand-ins or deploying these patches.
