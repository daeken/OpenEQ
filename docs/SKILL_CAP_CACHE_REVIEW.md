# Skill-cap maximum cache: isolated server regression

Reviewed 2026-09-30 against EQEmu
`4aceae18b94ffaafc08e2b17bc41cd72c77f795d`.

**The maximum-level cache truncates class/skill identities to eight bits.**
An actual-source regression reproduces incorrect maximum levels and empty
cap results, and a one-line key-width change fixes those cases. This patch
has not been deployed or pushed to EQEmu. No live database or character was
examined for this review, so these results do not establish a deployed symptom.

## Root cause and observable behavior

In `common/skill_caps.cpp:23`, `skill_max_level` is a
`std::map<uint8_t, int32_t>`. Both its producer in `LoadSkillCaps` and its
consumer in `GetSkillCapMaxLevel` use `class_id * 1000000 + skill_id` as the
key. Conversion to eight bits reduces that identity to
`(64 * class_id + skill_id) % 256`, because `1000000 % 256 == 64`.

- Classes 1, 5, 9 and 13 collide for every same skill; corresponding groups
  beginning with classes 2, 3 and 4 do too.
- Different skills can collide across adjacent classes: class 1/skill 64 and
  class 2/skill 0 both reduce to key 128.
- The 16 supported classes and 78 supported skill IDs have 1,248 distinct
  identities, but their narrowed keys occupy only 256 entries.

The cache can therefore return another pair's higher maximum. `GetSkillCap`
then clamps the requested level to that foreign maximum and looks for an
exact row belonging to the original pair. If that row is absent, it returns
the repository's default empty cap. For example, fixture rows ending at
class 1/skill 0/level 10/cap 50 and class 5/skill 0/level 50/cap 250 cause a
class 1/skill 0/level 65 request to return cap 0 before the fix and cap 50
afterward. Both source row orders are tested after the fix.

A completely absent pair can also inherit a colliding pair's maximum instead
of using the configured maximum-level fallback. Existing exact-level rows
below a pair's true maximum can continue to work; equal maxima among
colliding pairs can hide the error. This is not evidence that every live
skill cap is wrong.

`Client::CanHaveSkill` and `Client::MaxSkill` call `GetSkillCap`
(`zone/client.cpp:3176–3199`); trainer open and purchase paths use those
methods (`zone/client_process.cpp:1646,1752,1757,1813`). Those callers make
cap and eligibility effects possible, without proving any particular live
trainer failure.

## Patch and regression

The production change is only:

```cpp
std::map<uint32_t, int32_t> skill_max_level = {};
```

The largest supported key is 16,000,077, which fits in 32 bits. Existing key
formulas, APIs, cap row storage, wire formats and database schema stay the
same. A source search found no other production declaration of the global
cache requiring a matching type change.

- EQEmu branch: `codex/skill-cap-cache-key`.
- Local EQEmu commit: `18987a397be48c79143b16497f6dea948c46c609`.
- Isolated worktree: `/tmp/openeq-eqemu-skill-cap-fix`.
- Patch: [0004-preserve-full-skill-cap-cache-keys.patch](../tools/eqemu-patches/0004-preserve-full-skill-cap-cache-keys.patch).
- The format-patch contains EQEmu source and tests under GPLv3 or later;
  storing it here does not relicense that code as client code.

`tests/skill_caps_cache_test.cpp` includes the real `skill_caps.h`, then
includes the actual `common/skill_caps.cpp` with a repository adapter that
substitutes only the `All(Database&)` query result. The actual loader,
cache, getters, repository row type and default row implementation are used;
the test does not copy the cache algorithm. A real default `Database` handle
is constructed, which initializes the client library without connecting.
There is no server configuration load, endpoint, credential or SQL query.

The fixture clears the global maximum cache between cases and sets the
maximum-level ceiling to 65. This deliberately isolates the narrowing defect
from separate reload and first-row ceiling behavior. Eight cases cover:

1. Classes four apart, including both input row orders.
2. Upper-level cap lookup returning the pair's own final row.
3. Different skills in adjacent classes.
4. Absent-pair fallback.
5. All 1,248 supported class/skill pairs, each with a distinguishable cap.
6. Noncolliding maxima, exact-level lookup, normal upper clamping and absent
   exact-level rows.
7. The existing ceiling behavior with levels 1, 65 and 80 in that order.
8. Filtering of zero-level, invalid-class and invalid-skill rows.

The test executable returns failure when any case fails. Its standalone
`skill_caps_cache_tests` CMake target is built when tests are enabled and
reuses the existing test target's link libraries and platform dependencies.

## Before/after proof and build limits

| Actual-source runner | Before | After |
| --- | ---: | ---: |
| Passing cases | 3/8 | 8/8 |
| Cached pairs from 1,248 supported identities | 256 | 1,248 |
| Incorrect exhaustive maxima | 992 | 0 |
| Incorrect exhaustive cap results | 992 | 0 |
| Runner exit status | 1 | 0 |

The same fixture and compiler/link inputs were used before and after the
one-line production change. Tests ran with GCC 14/C++20 in disposable remote
directory `/tmp/openeq-skill-cap-regression.bnrooY` on `storage2.daeken.dev`.
The cached source at `/home/daeken/eqemu-bootstrap/source` was verified at the
pinned base, and the matching static common archive and dependencies under
`/home/daeken/eqemu-bootstrap/build` were used read-only. The test translation
unit provides the actual tested SkillCaps implementation before the common
archive is linked; the archive provides the other real server dependencies.

The new `tests/CMakeLists.txt` target was also configured, built and run
successfully in a disposable wrapper that imports that common archive and
its dependencies. All eight cases passed again. The wrapper supplies the
cached library search directory needed by the existing test target's `z`
link dependency; its initial omission was corrected before this passing run.
No cached source/build directory was reconfigured or changed.

Local evidence is in `/tmp/openeq-skill-cap-evidence/`: `before.log`,
`after.log`, `build-after.log`, `cmake-build.log`, `cmake-test.log`, the direct
`build.sh`, and the disposable wrapper `CMakeLists.txt`. Equivalent remote
logs remain in the disposable directory. These temporary paths are session
evidence; the patch's regression source is the durable reproduction.

This is an actual-source focused build and standalone regression, not a full
server rebuild or live fixture test. The original sibling EQEmu checkout and
the existing cursor worktree remain clean. No server restart, deployment,
live database mutation or upstream push was performed.

## Applying and running separately

The patch passes `git apply --check` against the clean pinned checkout and
the clean cursor worktree after patches 0001–0003. It is independent of those
cursor changes and does not require them. `git diff --check` passes.

For a separate review checkout based on the pinned revision:

```sh
git -C /path/to/EQEmu apply --check /path/to/OpenEQ/tools/eqemu-patches/0004-preserve-full-skill-cap-cache-keys.patch
git -C /path/to/EQEmu am /path/to/OpenEQ/tools/eqemu-patches/0004-preserve-full-skill-cap-cache-keys.patch
```

With an EQEmu build configured normally with `EQEMU_BUILD_TESTS=ON`:

```sh
cmake --build /path/to/build --target skill_caps_cache_tests
/path/to/build/bin/skill_caps_cache_tests
```

These commands build and run only the isolated regression target. Applying
the patch does not constitute a reviewed server rollout.

## Separate source hazards left out of this patch

The pinned code also leaves global maxima behind when loading cap rows again;
only `m_skill_caps` is cleared. Its first insertion of a pair's maximum does
not clamp to the configured ceiling until an existing-key update occurs.
`GetSkillTrainLevel` builds a lookup key using the requested `level` outside
its loop instead of the changing `current_level`. The getter and loader also
use inconsistent upper skill bounds. These are distinct review items; the
cache-width regression neither fixes them nor establishes their live impact.
