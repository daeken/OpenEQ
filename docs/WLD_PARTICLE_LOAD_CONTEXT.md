# Native WLD particle loading, resource scopes and sharing

Evidence checkpoint: 2026-10-01. The installed original client's PoK zone
loading path calls the **full** object-WLD loader at resource scope 2. Native
WLD setup and full assembly, executed separately against all 2,805 original
`poknowledge_obj.wld` fragments and an initially empty resource cache, register
four particle definitions. All eight cloud fragments then reference those
four definitions through shared wrapper pointers. This follows from executed
native registration and lookup, not from comparing duplicate source records.

The cache's scope rule is **signed stored scope <= requested scope**. It is
not an exact `(name, scope)` lookup. The earlier controlled dictionary in
[WLD_PARTICLE_RUNTIME.md](WLD_PARTICLE_RUNTIME.md) deliberately supplied a
simpler policy; its changed-context miss was a harness condition, not a native
cache result.

These are bounded native-code witnesses, not a recording of a running original
client. In particular, they do not reconstruct the entire cache populated by
all earlier startup/import paths or establish complete visual parity.

## Provenance and controlled boundaries

All addresses are preferred PE32 virtual addresses, not recovered symbols.

| Original input | SHA-256 |
| --- | --- |
| `EQGraphicsDX9.dll`, base `0x10000000` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| `eqgame.exe`, base `0x00400000` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| `poknowledge_obj.s3d / poknowledge_obj.wld` | `e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3` |

Three temporary Python/Unicorn 2.1.4 witnesses divide the evidence:

- The cache witness executes the original constructor, registration, hash,
  bucket traversal, lookup and particle helper. Only heap allocation/free and
  diagnostic boundaries are supplied. There is no Python resource dictionary.
- The upstream witness executes two original `eqgame.exe` zone-loader blocks
  and the full WLD request wrapper, using actual installed file existence.
  Archive handles, path/progress/CRT calls and final graphics-manager load
  endpoints are controlled. It records their arguments without assembling WLDs.
- The downstream witness reads all original object-WLD bytes through a supplied
  archive read/size/close interface. Native header/fragment reading, string XOR
  decoding, full assembly, cloud parsing/conversion, definition registration,
  resource cache, wrapper getters, emitter factories and cleanup execute.
  Allocation interfaces use mapped memory; frees are recorded without unmapping.
  Native material, sprite, track, animation, actor, instance, light and zone
  processing boundaries are intercepted. The cloud texture resolver supplies
  four controlled single-frame texture descriptors and fake AddRef endpoints;
  it rejects any other texture reference. The supplied material word
  `0x011b0507` is the separately established PoK alias result. Image dimensions
  are controlled at 64 by 64; no image decode/upload or GPU work occurs here.
  Native trigonometric initialization executes; no particle update, randomness
  or actual object-placement assembly is needed for the sharing witness.

The downstream source check compares every native fragment's kind, name and
raw body against the original file. The filename marker check supplies the
correct negative result for `_lit.wld`; it does not alter PoK's load path.
Unrelated diagnostic formatting and ASCII classification/comparison have
controlled substitutes. Context allocator internals and full client startup
are outside these witnesses.

## Upstream zone and object load calls

DLL factory `0x1001247a..0x10012480` places the resource manager from global
`0x1017c0a4` into returned interface structure `+4`. The executable calls the
factory at `0x00933699` and publishes that manager at `0x01822670` at
`0x009336b8..0x009336bb`.

The manager's vtable is `0x1013b2ec`. Relevant entries are:

| Slot | Target | Role |
| --- | --- | --- |
| `+0` | `0x10066700` | EQG archive loading |
| `+0xc` | `0x100675f0` | Full WLD loading |
| `+0x10` | `0x10067550` | Filtered WLD/actor loading |

Executable wrapper `0x004ab520` opens the archive through global `0x0182266c`
and invokes manager `+0xc` at `0x004ab852`. Its arguments are member name,
archive handle, resource scope and two byte flags. Filtered wrapper
`0x004ab9e0` invokes `+0x10` at `0x004abb01`, supplying a filter name instead of
the two flags.

The normal world loader at `0x004bdca0` is called at `0x00569738` after the
`Initializing world.` log; another caller at `0x00579eee` follows `Starting
load.`. The bounded upstream witness executes object block
`0x004be442..0x004be837` and later world-members block
`0x004beb26..0x004bed67`, with PoK's name and zone ID 202:

| Within the two captured blocks | Endpoint | Scope | Flags |
| --- | --- | ---: | --- |
| `poknowledge_obj3.eqg` | EQG load | 2 | 0, 0 |
| `poknowledge_obj2.s3d / poknowledge_obj2.wld` | Full WLD load | 2 | 1, 0 |
| `poknowledge_obj.s3d / poknowledge_obj.wld` | Full WLD load | 2 | 1, 0 |
| `poknowledge.s3d / poknowledge.wld` | Full WLD load | 2 | 0, 0 |
| `poknowledge.s3d / objects.wld` | Full WLD load | 2 | 0, 0 |
| `poknowledge.s3d / lights.wld` | Full WLD load | 2 | 0, 0 |

The installed `_us` variants and optional `poknowledge_2_obj.s3d` are absent.
The object and world-member blocks are **not adjacent**: intervening code at
`0x004be837..0x004beb26` loads character imports and other assets. The installed
`poknowledge_chr.txt` lists 31 entries and its assets list names five EQGs;
those imports were not executed by this witness. An EQG endpoint returning
controlled success is not evidence of its actual native parsing result.

An original-source audit finds no `0x26` or `0x34` fragments, and no names
matching these four effects, in object2 (52 fragments), main world (3,890),
`objects.wld` (1,249) or `lights.wld` (1,860). This does not exclude entries
introduced by other startup, EQG or import paths.

## Full assembly and within-file registration order

The full DLL wrapper creates a temporary WLD loader with constructor
`0x100c04e0`, installs vtable `0x1013b8fc`, and calls setup `0x100c1030`.
Setup stores its third argument at loader `+0x2c`, chooses the allocator at
`0x101d30ec + 4*scope`, and stores that allocator at loader `+0x3c`.
The downstream witness uses the upstream-established arguments: scope 2,
flags 1 and 0.

Fragment entries have 20-byte stride: size `+0`, kind `+4`, name `+8`, original
body `+0xc`, runtime resource `+0x10`. Full assembly `0x100c17f0` scans all
entries before invoking these stages:

1. Material/bitmap bulk loading at `0x100c1977`.
2. `0x26` sprite bulk loading at `0x100c1995`.
3. `0x34` cloud bulk loading at `0x100c19b3`, with no name filter.
4. Track references, then optional animations.
5. Actor definitions in increasing fragment order at `0x100c1ab7`.
6. Object instances, when present.
7. Cloud bulk loading again at `0x100c1b2c`, with no name filter.
8. Remaining zone/light processing and scratch-allocator finalization.

Both cloud passes execute against the complete original fragment table. The
first pass registers four definitions; the second registers none. The native
texture resolver boundary receives only 4, 9, 14 and 19 during assembly.

| Cloud fragments sharing a native wrapper and definition record | Name | Definition index |
| --- | --- | ---: |
| 5, 406 | `CSMOKE_PCD` | 0 |
| 10, 411, 471 | `L301_PCD` | 1 |
| 15, 438 | `L308_PCD` | 2 |
| 20 | `L500_PCD` | 3 |

For each new definition, cloud reader `0x1001e640` calls converter
`0x1001c410`, real particle registry `0x10070ce0`, context allocation, wrapper
constructor `0x1006f2f0(index, scope)`, and global resource registration
`0x100622d0(name, wrapper, 0x1002, scope)`. The wrapper holds definition index
at `+4` and resource scope at `+8`. Getter `0x1006f310` resolves the index into
the particle manager's WLD definition collection at `+0x38`.

The filtered path differs: `0x1001a730` applies the supplied name prefix to
material and cloud bulk readers, then finds the requested actor. It does not
invoke the `0x26` bulk stage. These static filtered-path observations must not
be substituted for the proven full PoK object-WLD path.

## Exact native cache policy

Loader virtual lookup `0x1001aa00` calls global resource manager wrapper
`0x100622f0`, which forwards to embedded cache `manager+4`, lookup
`0x100c7bd0`. The native cache record is 24 bytes:

| Offset | Value |
| --- | --- |
| `+0` | Name hash |
| `+4` | Next entry in hash bucket |
| `+8` | Resource type |
| `+0xc` | Signed resource scope |
| `+0x10` | Name pointer |
| `+0x14` | Resource pointer |

Names and hashing are case sensitive. Lookup at `0x100c7c30` skips an entry
when its stored scope is **greater** than the requested scope, using signed
`jg`. It returns the first same-name qualifying entry in bucket order. It
does not choose the numerically greatest qualifying scope.

Registration `0x100622d0 -> 0x100c8110` checks exact name plus exact scope via
`0x100c7ae0`. An existing match returns success without overwriting either
its pointer or type. A new entry is inserted at the bucket head. Thus later
registrations at another scope can precede older entries at higher scopes.

The standalone native cache witness passes 17 direct lookup assertions and
two particle-helper assertions:

| Registration/control | Requests | Observed result |
| --- | --- | --- |
| Only scope 0 | -1 / 0 / 1 | miss / hit / hit |
| Only scope -1 | -2 / -1 / 0 / 1 | miss / hit / hit / hit |
| Same name: scope 1, then scope 0 | 0 / 1 / 2 | Later scope 0 returned every time |
| Same name: scope 0, then scope 1 | -1 / 0 / 1 / 2 | miss / scope 0 / scope 1 / scope 1 |
| Register same exact name/scope again with changed pointer and type | 1 | Original entry unchanged |
| Query lowercase `order` after registering `ORDER` | 1 | miss |
| Correct type at scope 0, then wrong type at scope 1 | 1 | Cache returns newer wrong type; particle helper rejects it |
| Same typed pair | 0 | Scope 1 is skipped; particle helper accepts scope 0 |

Type validation happens after lookup in helper `0x100c0520`; a wrong-type
first match does not cause it to search older matching names. The negative
scope tests exercise the cache only. They do not pass negative scope indices
to WLD setup's allocator array.

The original on-demand reader `0x1001d9b0` additionally executes for PoK
fragment 411 after clearing its runtime slot:

| Requested scope, with definition registered at 2 | Result |
| ---: | --- |
| 3 | Reuses fragment 10's wrapper; no texture request |
| 1 | Miss; requests texture 154 (`410 & 255`); controlled resolver rejects it |
| 2 | Reuses fragment 10's wrapper; no texture request |

The actual higher-scope hit directly corrects the earlier dictionary-based
changed-context miss. Metadata must still retain full original references;
cache visibility does not make truncated references safe on a miss.

## Emitter grouping and lifetime

The downstream witness calls native factory `0x10071080` once for each of the
eight cloud records, using each resolved definition index and distinct
controlled actor/node owners. It produces eight distinct emitters and eight
distinct particle rings. The four shared definition records head linked
emitter lists at record `+0x10`; emitter `+0x84` links the next instance.
Each list contains exactly the emitters created from its shared definition.
This establishes definition sharing with independent emitter state. It does
not assert that PoK has eight placed effects: the original object metadata
contains ten particle attachments across eight actors, and world placement
can instantiate those actors multiple times.

Native WLD destructor `0x100c0fa0` closes the member, releases temporary entry
and source storage, frees its filename and clears the loader fields. It does
not clear the named-resource cache or release the context-owned string table.
All eight lookups still return their original four wrappers after this
routine executes. Allocator internals remain a controlled boundary.

Particle clear `0x10071800` invokes `0x10071790` for its four collections.
Executing it calls the release boundaries for all eight emitters and rings,
sets all four definition emitter-list heads to zero, and retains the four
definition records.

Resource cleanup `0x10062370 -> 0x10068490` is separate. For particle resource
type `0x1002`, its nonzero-scope branch compares wrapper `+8` with the requested
threshold at `0x100686bd`; it removes wrappers at or above that threshold.
Cleanup scope 3 preserves scope-2 wrappers; cleanup scope 2 removes all four
cache entries and calls native wrapper destructor `0x1006f3a0` four times.
The particle manager still contains four definition records afterward.
Cleanup 0 has a distinct all-resources branch, not the same threshold loop.

Static inspection of the outer resource-manager reset `0x1005c020` establishes
an ordering not reproduced by calling cache cleanup alone: it first invokes
particle manager `+0xc8 -> 0x10071800` at `0x1005c04d`, then eventually removes
scoped resources at `0x1005c14e`, and resets the selected context allocators at
`0x1005c1d7`. Full outer reset and the complete lifetime of retained particle
definition records are not executed here. Resource-scope visibility and the
manager/emitter drawing-context gate in
[WLD_PARTICLE_PLACEMENT.md](WLD_PARTICLE_PLACEMENT.md) are different mechanisms.

## Frozen artifacts and remaining limits

| Temporary artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-native-particle-context.py` | `82de099d6e471c669b4e3632f26e525953a49cb070fb7196b0fb0fea9827bf35` |
| `/tmp/openeq-native-particle-context.json` | `27b581c493a2e05589e6e51670659357e6918fb4e5bc04a755c28a8b067d3425` |
| `/tmp/openeq-native-particle-load.py` | `4cd412de5d2f55a7239d6e2b76e0e495e9e429923361a8465253f7bcc27ba315` |
| `/tmp/openeq-native-particle-load.json` | `b59b6fd962bff17f5e923650a95a520638bca7bb6a67e88b2e4f59805c6d6c61` |
| `/tmp/openeq-native-wld-load-upstream.py` | `ceb2b795e56b59b23eac0b52da2016ff6a2442f14e35718f604aa83443737070` |
| `/tmp/openeq-native-wld-load-upstream.json` | `0bf3a1ceb2ca14f63b52b97804a6ef78a771c4a04c7ae6df417913a3714df109` |

Run each script with `PYTHONPATH=/tmp/openeq-re-tools python3 SCRIPT`. All
assertions pass. The downstream script reuses only the setup prefix of the
cache script and reader prefix of `/tmp/openeq-wld-particle-probe.py`; it does
not execute either script's output-writing experiment body.

These results establish the active loader selection, arguments, within-file
ordering and native cache semantics. They do not prove that a complete live
startup has no earlier same-name resources, cover every import/EQG path,
execute all scene assembly, or demonstrate final pixels. The separate
[material](WLD_PARTICLE_MATERIALS.md),
[runtime](WLD_PARTICLE_RUNTIME.md) and
[placement](WLD_PARTICLE_PLACEMENT.md) witnesses retain their own boundaries.
No production particle playback or effect-generated lighting is added here.
