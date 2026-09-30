# ItemInstance clone review

Reviewed against EQEmu `4aceae18b94ffaafc08e2b17bc41cd72c77f795d` on
September 30, 2026. The isolated worktree is
`/private/tmp/openeq-eqemu-item-clone-fix`; the main EQEmu checkout and existing
cursor-ownership repair worktree were not modified.

[Patch 0006](../tools/eqemu-patches/0006-preserve-item-task-delivery-count-on-clone.patch)
adds one production assignment to the copy constructor and five utility tests.
EQEmu commit: `b4201d7aa0d1dd0c66dd1676327a25a25cae5b39`. Independent source
review found no remaining issue. All six numbered server patches also applied
cleanly, in order, from the pinned base in the fresh disposable worktree
`/private/tmp/openeq-eqemu-clone-integration`.
It is independent of the cursor ownership and persistence patches, and has not
been deployed or pushed upstream.

## Defect and invariant

`ItemInstance` declares 24 member fields. The copy constructor at
`common/item_instance.cpp:107` omits only `m_task_delivered_count`, so its default
initializer silently replaces a nonzero count with zero. `Clone` invokes that
constructor; bag/augment copies and `InventoryProfile::PutItem` / `PushCursor`
recursively use Clone.

Task delivery sets this counter in `ClientTaskState::UpdateTasksOnDeliver`
(`zone/task_client_state.cpp:766`). `RemoveTaskDeliveredItems`
(`common/item_instance.cpp:1756`) explicitly subtracts credited deliveries and
then resets the counter. A copy must retain the same pending delivery accounting;
copy construction is not the operation that consumes or resets it.

Executed example: a five-item stack credited for three deliveries should report
two remaining when removal is applied to its copy. Before the fix it reports five.
A delivered nonstackable item should report zero remaining; its copy reports one.
The patch copies the count exactly, leaving consumption/reset behavior unchanged.

## Complete field audit

All declarations in `common/item_instance.h:347–372` were checked against the
constructor. Existing semantics are retained:

| Fields | Copy behavior |
| --- | --- |
| `m_use_type` | Preserve normal/world-container usage; a world container may have no item definition. |
| `m_item`, `m_scaledItem` | Deep-copy each owned ItemData object, preserving null. Scaling a copy must not change the original. |
| `m_charges`, `m_price`, `m_color`, `m_attuned` | Preserve exact instance values, including signed charge sentinels and zero color. |
| `m_merchantslot`, `m_currentslot`, `m_merchantcount` | Preserve bookkeeping, including unlimited merchant-count sentinel. Destination APIs may subsequently change slot state. |
| `m_SerialNumber` | Preserve native identity intentionally. Clone is not a new item grant or serial allocation. |
| `m_exp`, `m_evolveLvl`, `m_scaling` | Preserve scaling state. The legacy `m_evolveLvl` scalar is copied even though the public level accessor reads ItemData. |
| `m_ornamenticon`, `m_ornamentidfile`, `m_new_id_file`, `m_ornament_hero_model` | Preserve appearance overrides. |
| `m_recast_timestamp` | Preserve recast timestamp. |
| `m_task_delivered_count` | **Previously omitted; now copied.** Explicit removal owns the reset. |
| `m_evolving_details` | Value-copy the entire record, including identity/owner, activation/equipment flags, amount, progression, final item and deletion timestamp. No new database identity is allocated. |
| `m_contents` | Recursively clone each non-null owned child at its exact sparse key. Bags and augments share this map. Null entries intentionally own no item and remain omitted. |
| `m_custom_data` | Copy the string map directly, preserving arbitrary bytes without caret serialization. The existing redundant loop plus assignment is unchanged. |
| `m_timers` | Value-copy timer state and deadlines; copying does not restart timers. No persistence/replay policy is implied. |

## Executed evidence

The same 84 utility tests ran against the actual pinned implementation and then
the patched ItemInstance object:

| Run | Existing 79 tests | New 5 tests | Total |
| --- | --- | --- | --- |
| Before | 79 passed | 2 passed, 3 failed | 81/84 |
| After | 79 passed | 5 passed | 84/84 |

The three baseline failures cover direct copies, Clone's stack/nonstackable
return result, and actual InventoryProfile insertion of roots, sparse bag
children, and augments. Additional checks preserve serial identity, signed
charges, appearance/merchant/scaling state, independent scaled/unscaled ItemData,
binary custom strings, timers, evolving state, and world-container/null-entry
semantics. Source instances retain their original counters and charges.

The remote disposable fixture is `/tmp/openeq-item-clone.8eZYgd` on storage2.
Logs and the exact build harness are preserved locally under
`/tmp/openeq-item-clone-audit/` (`before.log`, `after.log`, `build.sh`,
`utility_main.cpp`, `deny_connect.cpp`). The fixture uses only a generated
credential-free config and its own shared-memory directory. A linker wrapper
aborts any attempted `mysql_real_connect`; each execution also runs inside a new
network namespace. No live database, character, deployment, or service restart
was used.

The harness links production `InventoryProfile` and other common objects from
`/home/daeken/eqemu-bootstrap/build/bin/libcommon.a`, whose source checkout is the
pinned revision. For the patched run it compiles the actual modified
`item_instance.cpp` and places that object before the archive. No ItemInstance,
InventoryProfile, or task-removal method is mocked. The normal runner ignores
its suite's boolean result; the fixture runner only converts that result into
an exit failure for the baseline proof.

Actual build/run steps, from the remote fixture:

```sh
bash build.sh before
bash build.sh after
(cd before && sudo unshare --net runuser -u daeken -- ../tests-before)
(cd after && sudo unshare --net runuser -u daeken -- ../tests-after)
```

`before` must exit 1 with the three expected failures; `after` must exit 0 with
84/84 passing. Each directory contains its own `eqemu_config.json` with only
`{"server":{"directories":{"shared_memory":"shared"}}}` and a writable
`shared/` directory. Both builds use the same final test header. The build script
uses the cached dependency include/libraries, C++20, and `--wrap=mysql_real_connect`;
only `after` adds the separately compiled ItemInstance object. In a normal EQEmu
build, enable `EQEMU_BUILD_TESTS=ON` and build the `tests` target; registration is
included in the patch's `tests/main.cpp` and `tests/CMakeLists.txt`.

## Combined patch-series proof

A fresh worktree at the pinned base accepted patches 0001 through 0006 in order
with no manual conflict resolution. The initial check exposed overlapping CMake
context between 0004 and 0005; moving 0005's target declaration before the existing
install block resolved it without changing production code. The refreshed series
and its complete diff pass whitespace checks.

The resulting combined utility suite passes **98/98**: the original 79 tests,
14 cursor ownership tests, and five clone tests. The fixture compiles the actual
integrated InventoryProfile implementation/header, ItemInstance, and SkillCaps;
it links the separately compiled patched SharedDatabase object from
`/tmp/openeq-inventory-save-errors.mMyr1d/shareddb.o` ahead of the pinned common
archive. The integrated `tests/main.cpp` is unmodified. The connection-denial
wrapper and network namespace remain active.

Remote fixture: `/tmp/openeq-item-clone-combined.7drXHI`. Executed there:

```sh
bash build-combined.sh
sudo unshare --net runuser -u daeken -- ./tests-combined
```

`combined.log`, `combined-build.log`, `build-combined.sh`, and the exact integrated
source inputs (`combined-inputs.tar.gz`) are preserved under the local evidence
directory above. This utility run does not replace the separate skill-cap cache
or save-error failure-injection targets, and does not claim a full zone/server
build or live deployment.

## Limits

This repairs copy construction and Clone for valid existing ownership trees. It
does not establish persistence of task-delivery state, atomic quest delivery,
exactly-once hand-ins, rollback safety for external side effects, assignment
operator semantics, invalid cyclic graphs, or exception-safe cloning. No live
NPC quest flow was executed. The lossless cursor persistence plan and its other
caller/schema requirements remain necessary.
