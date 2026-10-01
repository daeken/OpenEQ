# Executed ordinary-terrain point-light membership and ordering

2026-10-01, native research and source-metadata preservation. This extends
[the SPL light binder](EQG_TER_LIGHT_BINDING.md) upstream through the actual
three-light selector. Asset loading now retains its authored candidate gate;
renderer light selection and rendering remain unchanged.

The native ordinary-terrain path selects eligible lights from the active
DPVS influence list, ranks them by radius, RGB brightness, distance to the
region center, and two priority flags, then stores three pointers. It does
not simply select the nearest three archive lights. Most importantly,
ordinary terrain excludes unflagged authored lights from this list.
Runtime-created lights are eligible; EQG-authored lights whose **third name
byte is `B` or `b`** are also eligible.

## Binary and execution boundary

All addresses refer to the original `EQGraphicsDX9.dll`, base `0x10000000`,
SHA-256 `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The witness `/tmp/openeq-ter-light-list.py` executes original x86 instructions
with Unicorn 2.1.4, x87 CW `0x037f`, and MXCSR `0x1f80`.

Executed code includes RGB and point-light constructors, the light-influence
record initializer and its weight calculation, native position-to-DPVS-matrix
assembly, the terrain receiver constructor, DPVS commander handlers,
candidate ordering/filtering, the ordinary batch factory, and the ordinary
batch caller through shader point-vector binding. The native EQG name-flag
block and complete runtime light-creation API are also executed.

Allocation, memcpy, DPVS interface objects/events, render-state/effect calls,
and the runtime creation API's registration boundary are controlled. The
probe supplies the influence enter/leave and visible-object event sequence.
It does **not** execute `dpvs.dll` geometry traversal, compute actual visible
region/light overlap, establish real-world event ordering, run a native
graphics device, or compare rendered pixels. This closes selection **given
the original event stream**, not all spatial/culling behavior.

## Receiver-to-batch pointer chain

The ordinary region's constructor/registration call at `0x1001828d` enters
`0x1000e830`, which allocates its visibility receiver and invokes
`0x1000da40`. The latter sets:

| Receiver field | Meaning established here |
| --- | --- |
| `+0x0c = 2` | Ordinary region receiver kind |
| `+0x18` | Region pointer |
| `+0x1c` | Selected light count, initially zero |
| `+0x20..+0x28` | Three selected light pointers |
| `+0x2c = 0` | Do not admit unflagged lights |

The constructor writes the receiver into **region + 8**. The original
terrain batch creation site at `0x1001987f` passes `&region[+8]` to
`0x1008f960`; the factory writes that pointer-to-pointer into **batch + 0x10**
at `0x1008fd25`. The witness executes the factory with layout 1/runtime
material 9, obtaining descriptor index 6. This is the established
`Opaque_MaxCB1.fx` / SPL `RegionCB1.fxo` route.

The real batch draw `0x1008a9f0` calls binder `0x1008a380`, which follows
batch +0x10 → region +8 → receiver, then reads count +0x1c and slots +0x20.
The witness feeds the factory-created batch directly to this caller after
native list selection; all six shader vectors match the selected lights.
There is one selected list per region, shared by its relevant material batches.

## Where candidate membership comes from

`0x1000e410` creates a DPVS `SphereModel` with the point light's radius at
the local origin, then a `RegionOfInfluence` in the world cell. Its user
pointer is the light-influence record. `0x1000dc90` builds the translation
matrix from the original point-light position getter. These calls and their
submitted values are recorded by the witness's controlled DPVS interfaces.

The command dispatcher at `0x10010640` handles these native command values:

| Command | Handler | Effect |
| --- | --- | --- |
| `0x40` | `0x10010ca8` | Append the influence record to manager +0xb0's active linked list |
| `0x41` | `0x10010d46` | Remove that influence record from the active list |
| `0x30` | `0x10010aed` | For visible receiver kind 2, call the selector `0x1000fcb0` |

`0x10011370` appends at the tail. Candidate traversal therefore starts in
active influence-entry order; an exit removes that source without sorting
the others. The selector does not independently calculate sphere/region
intersections or reject by light radius. Those are upstream DPVS concerns.
Replacing that input with every archive light is not equivalent.

## Authored versus runtime eligibility

The point-light constructor `0x100126e0` defaults **light +0x64 to zero**.
The selector at `0x1000fe5c..0x1000fe6d` skips a zero-flagged light when
receiver +0x2c is zero. The ordinary region constructor explicitly chooses
that receiver policy. An independently tested receiver override includes
the otherwise excluded light; this does not establish which other real
receiver classes choose that override.

The full runtime creation API `0x1006b660` sets light +0x64 to one before
registering it. The EQG zone-loader paths at `0x1006587e..0x10065898` and
`0x10065fc4..0x10065fda` read **name[2]**, compare against `0x42`/`0x62`,
and set the flag for `B`/`b`. For other names the constructor default remains.
Calling this merely a dynamic/static split would lose eligible authored
`LIB_...` lights. The probe uses the neutral term *terrain-list eligibility*;
the original field's source-level name is not known.

Original installed ZON records provide a concrete cross-check:

| Zone | Authored lights | Eligible by native name test | Examples |
| --- | ---: | ---: | --- |
| Anguish | 452 | 0 | `LIT_walllight294`, `LIT_extlight68` |
| Causeway | 126 | 55 | Eligible `LIB_brazier45`; excluded `LIT_torch95` |
| Bloodfields | 134 | 12 | Eligible `LIB_room11`; excluded `LIT_Torch60` |
| Wall of Slaughter | 188 | 0 | `LIT_torch121`, `LIT_torch120` |

These counts concern the point slots of this ordinary-terrain route, not
whether those source lights have any other purpose. In particular, no
Anguish `.zon` light enters these slots through the ordinary name path;
runtime lights remain possible. Unflagged lights should not be added again
to this terrain path merely because they exist in the zone archive.

`ZonLight` now retains the name and byte-derived eligibility. `loader.rs`
carries both into `Light::eqg_source` together with the original record ordinal
and selected declaration provenance (archive path/member or loose path).
Each original name byte maps to the corresponding U+00xx character, so high
bytes are preserved without changing the source-byte eligibility test or
subsequent name offsets. The metadata follows each light when cloned or
removed. WLD, heightmap and programmatically constructed lights use `None`;
they make no binary-ZON eligibility claim.

Every light remains loaded in the original order with unchanged numeric
values. No renderer consumes the new metadata or filters lights by it.
Native terrain integration still needs region membership and event order
rather than one zone-wide selected set.

## Weight, priority, cap, and exact ordering

`0x1000e410` uses the original point-light and RGB-definition getter chain
to calculate a stored influence weight at record +0x18:

```text
stored_weight = radius² × (min(R,G,B) + max(R,G,B)) / 2
```

This uses the minimum and maximum channel, not the mean of all three.
An asymmetric RGB witness `(1, 0.125, 0.5)` with radius 4 produces stored
weight 9. Native operations and the store to f32 execute in the probe.

For receiver kind 2, the selector computes the center of the region's
axis-aligned bounds, reading region +4 → bounds +0x1c/+0x28. Other receiver
kinds use receiver +0x30/+0x34/+0x38. For ordinary finite inputs:

```text
score = (2 × stored_weight / distance_to_region_center²) × priority
priority = 100 if light byte +0x1d is nonzero
           10 if light byte +0x1c is nonzero
            1 otherwise
```

The priority getters are vtable +0x38 and +0x34 respectively. Their original
semantic names and all callers setting them remain unestablished. Do not
invent player/equipment meanings for these flags.

At most **199 eligible candidates** reach the scratch array at `0x1017ba18`
(score followed by light pointer). In traversal order, the selector performs
strict descending swaps for the first `min(candidate_count, 5) - 1`
positions, then copies the first three pointers to the receiver. Equal
scores do not directly swap, but earlier exchanges can reorder equal lights;
this is not a stable sort. Replacing it with a stable sort changes observable
slot order in tied cases.

Executed cases include:

- A farther, large bright light beating a nearer dim light.
- Both priority levels changing the top three.
- An extremely strong unflagged light excluded from ordinary terrain.
- Uppercase/lowercase third-character `B` lights admitted.
- Zero, one, three, four, six, and 200 candidate inputs; the 200th, strongest
  light is ignored after 199 eligible inputs.
- Equal-score input order and equal-score reordering caused by a stronger
  later candidate; unrolled sorting with more than five candidates.
- Influence leave removing a previously entered source.
- The ancillary light-pass preparation branch yielding the same selected
  terrain list for the tested case.
- Factory-created descriptor-6 batch binding precisely the selected lights,
  with color W = radius and position W = 1.

The witness checks every resulting scratch score and order against a
separate finite-input calculation. Zero-distance, nonfinite data, other x87
precision modes, and exact DPVS event ordering are not expanded claims.

## Reproduction and remaining boundary

The source-metadata implementation is covered by four focused CPU tests in
`crates/openeq-assets/src/loader/eqg_light_source_tests.rs`. Two synthetic
tests cover both binary ZON versions, exact names including high bytes,
duplicate names/ordinals, and exact-archive > loose > unambiguous-alias
declaration precedence. Two opt-in original-asset tests check every numeric
light record and the four zone counts above, plus absence of binary-ZON
metadata on Gfaydark WLD and Nektulos heightmap lights. All four pass with
`CARGO_INCREMENTAL=0 cargo test -p openeq-assets --lib eqg_light_source_tests -- --include-ignored`.

Run `PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-light-list.py`.
The script needs `pefile`, Unicorn, the installed DLL and four original
`.eqg` archives. It checks the DLL and extracted ZON hashes and writes
`/tmp/openeq-ter-light-list.json`. All assertions pass.

| Artifact | SHA-256 |
| --- | --- |
| Probe `.py` | `2abda543b71f540ac790d108f0e0f9d2f2eb840ee55c0e1840f8a4e9215a4006` |
| Result `.json` | `3acbc3e38e02b004493fb7afd3326bb0fdbf11aef34556ae73da9338116e442a` |

This establishes the concrete receiver, eligibility, ranking, and binder
contract. Live parity still requires the region partition/bounds and the
actual DPVS influence intersection/event policy, plus the independent
environment-vector and shader-input work described in the earlier notes.
Approximating DPVS candidates by all eligible lights or by a guessed nearest
subset must be labeled as an approximation rather than native list parity.
