# Embedded TOG areas: native transform research

Static research, 2026-10-01. The installed client contains a complete-looking
parent-to-area transform routine, but its invocation has **not** been proved.
The recovered loading path creates runtime areas and registers them, while the
group's construction and placement setters call only its object transform
routine. Embedded areas must remain unsupported until this gap is resolved.
The numerical fixture below is a replay of the separate area routine, not an
observation of an initialized or queried native area.

No production behavior changes accompany this document. No native code, live
character, server operation, or audio was used.

## Evidence identity

The installed `EQGraphicsDX9.dll` is PE32, preferred base `0x10000000`,
1,615,360 bytes, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below are virtual addresses in that binary. The common DAT region
registration and terrain sampler are documented in `EQG_LIQUID_TRANSFORMS.md`;
the shared box constructor is documented in `EQGZ_NATIVE_REGIONS.md`.

## Definition, placement, and runtime area records

Group definition reader `0x100fa010` recognizes `*BEGIN_AREA` at
`0x100fa159..0x100fa189`, calls area text reader `0x100f9530` at `0x100fa1b2`,
and appends the result to the definition's area list at `+0x18`. Definition
area count/getter are `0x100f9a60` and `0x100f9a80`; their linked records advance
through `+0x08`. This preserves source AREA order independently of OBJECT
records. The unplaced area constructor is `0x100f98f0`, with vtable
`0x10144474` and size `0x38`.

| Definition field | Text token | Reader stores |
| --- | --- | --- |
| `+0x10` | `*NAME` | `0x100f95de..0x100f95e3` |
| `+0x14..+0x1c` | `*POSITION`, XYZ | `0x100f964c`, `0x100f967a`, `0x100f96a8` |
| `+0x20..+0x28` | `*ROTATION`, XYZ degrees | `0x100f9713`, `0x100f9741`, `0x100f976f` |
| `+0x2c..+0x34` | `*EXTENTS`, full XYZ size | `0x100f97d6`, `0x100f9804`, `0x100f9832` |

Numeric tokens pass through `0x10110cf1` and are stored as float32. This grammar
has no area type, alternate shape, or per-area scale token.

The version-21 DAT group record is its NUL-terminated name followed by 48 bytes:
two grid words, local XYZ, XYZ degree rotation, XYZ scale, and a final Z
adjustment. Reader `0x10101912..0x10101975` copies all these fields, loads the
definition through `0x100f99a0` / `0x100fa280`, and calls group constructor
`0x10104290` at `0x101019bb`.

| Runtime group field | Meaning |
| --- | --- |
| `+0x10`, `+0x14` | Runtime object list head/tail |
| `+0x18`, `+0x1c` | Runtime area list head/tail |
| `+0x20` | Group definition |
| `+0x24`, `+0x28` | Longitude, latitude |
| `+0x2c..+0x34` | Parent local XYZ |
| `+0x38..+0x40` | Parent XYZ degree rotation |
| `+0x44..+0x4c` | Parent XYZ scale |
| `+0x50` | Parent Z adjustment |

`0x10104b10` creates runtime objects followed by runtime areas. Its area loop
gets each definition area at `0x10104d7b`; terrain vtable `0x10140644` slot
`+0x58` dispatches to factory `0x100a3d40`, constructing `CEQTerrainArea` through
`0x100a0c00`. At `0x10104d99..0x10104de9`, the loop sets the name, type zero,
shape string `"Box"`, authored area Z offset zero, and the original full-size
vector. It appends runtime areas in source order at `0x10104e43..0x10104e71`.
Runtime list nodes use next `+0x04` and payload `+0x0c`.

## Missing active caller

The separate area update body occupies `0x101046c0..0x10104a2b`. The following
checks prevent treating its arithmetic as an active compatibility contract:

- `0x10104b10` finishes by calling **object update `0x10104340`** at
  `0x10104e85`. That routine walks only runtime `group+0x10` and definition
  objects, then returns at `0x101046b5`; padding separates it from area update.
- Position, rotation, uniform scale, Z adjustment, and composite setters call
  the same object update at `0x10104f45`, `0x10104f65`, `0x10104f7d`,
  `0x10104f97`, and `0x10104fe5`.
- Runtime group vtable `0x10144cfc` does not contain the area update address.
- A scan of decoded direct calls/jumps in the DLL's entire `.text` found no
  branch from outside the area body into any address in that body. An unaligned
  32-bit scan of all PE sections found no absolute pointer into it either.
- The runtime area creation loop does not copy local position or rotation.
  Base area constructor `0x100f12a0` zeroes those floats but does not initialize
  its grid words at `+0x0c/+0x10`. Therefore a presumed default placement is
  not a safe fallback for the absent update call.

These observations strongly suggest a dormant routine in this build, but they
are not a proof excluding every possible computed call or external writer.
Do not synthesize a call, silently drop these records, register them at zero,
or claim their original runtime positions are known.

## Arithmetic in the separate area routine

The following describes the body **if called with an initialized group**.
It first requires runtime and definition area counts to agree. A mismatch
returns without updating any area (`0x101046dd..0x101046e7`).

Parent rotation goes through terrain vtable `+0x248`, active function
`0x100a80f0`. This reorders XYZ degrees into ZYX, multiplies by float32
`512/360`, and calls quantized rotation builder `0x100c27f0`. Copy helper
`0x100ce250` extracts its 3-by-3 basis. In conventional column-vector notation,
with `q(a)=trunc(float32(a*float32(512/360))) & 511`:

```text
R(parent) = Rz(q(parent.z)) * Ry(q(-parent.y)) * Rx(q(parent.x))
rotated_local = float32_per_component(R(parent) * area.position)
P = native_grid_to_world(parent.grid, parent.local)
C.x = float32(P.x + parent.scale.x * rotated_local.x)
C.y = float32(P.y + parent.scale.x * rotated_local.y)
C.z = float32(P.z + parent.scale.x * rotated_local.z
                 + parent.scale.x * parent.z_adjust)
```

`q` denotes 512 units per turn; lookup entries and stored basis values are
float32. Native intermediate arithmetic uses x87. Parent rotation calls are
at `0x101046ed..0x10104716`; the three local matrix products are at
`0x1010476b..0x10104809`. Parent grid-to-world conversion is called at
`0x1010484e`. Center arithmetic is `0x10104850..0x101048a6`.

All three center components use **parent scale X** (`group+0x44`), including
the Z adjustment. The routine copies all three scale components into the
child composite later, but registration does not consume that scale vector.
Full extents were copied unchanged at creation and remain unchanged here.

World-to-grid conversion `0x100eba80` is called at `0x101048b2`; it preserves
Z and converts XY into a grid plus local position. For negative world values
it uses remainder/truncation followed by one grid decrement and tile-width
addition. The positive exact-tile-edge and negative exact-multiple cases
should retain their own rounding/normalization tests if this path becomes
supported.

Angles are **componentwise parent-plus-child sums**, not a product of their
orientation matrices (`0x10104900..0x10104920`), then wrapped by repeated
360-degree addition/subtraction (`0x10104939..0x10104a29`). Parent scales are
copied at `0x10104924..0x10104935`. The composite setter, runtime area vtable
`+0x30` / `0x100f1b70`, is called at `0x101049f7`.

No terrain sampling occurs in this area body. The parent composite Z comes
directly from its current placement. The traced DAT load stores the authored
parent Z, and base tile initialization `0x100f4360` calls anchoring routines
for top-level objects, top-level areas, and lights at `0x100f447e`,
`0x100f4485`, and `0x100f448c`. Those iterate tile-data lists `+0x68`, `+0x70`,
and `+0x78`, respectively; groups occupy `+0x80`. This is not evidence for
sampling either the group anchor or each transformed child area's XY.

## Registration order and geometry handoff

The terrain loader's first pass handles top-level ATP areas across longitude
ascending, then latitude ascending. Its second pass repeats that grid order;
for each tile it handles non-ATP top-level areas, then that tile's groups.
The embedded loop does not perform a further ATP test or ATP-first promotion.

DAT group placement append `0x101019d5..0x10101a2d` preserves placement order.
Tile group count/getter `0x100f6120` / `0x100f6140` traverse that list; the
registration calls are at `0x100a6e41` and `0x100a6e73`. Within each group,
registration walks runtime areas in their source AREA order at
`0x100a6ea0..0x100a6fd9`, regardless of the tile containing the eventual
transformed center.

For each runtime area, registration gets its composite and calls
grid-to-world at `0x100a6ed8..0x100a6eea`, adds the authored area Z offset
at `0x100a6f07..0x100a6f0f` (zero for these embedded areas), halves the full
size at `0x100a6f18..0x100a6f4c`, converts degree rotation to 512-unit ZYX
at `0x100a6f55..0x100a6f7f`, and calls `0x100bbe80` at `0x100a6fc4`.
No additional parent scale is applied to the half-extents. This proves the
registration ordering and handoff, but cannot fill the missing initialization
step identified above.

## Independent original fixture: Ocean Green Hills

The installed archive contains seven group placements and six top-level areas.
Only `surefallglade.tog` contains an area: 305 objects followed by this record:

```text
*BEGIN_AREA
    *NAME AWT_watervolume
    *POSITION -285.2553 -193.0022 -1.0880
    *ROTATION -180.0000 0.0000 -90.0000
    *EXTENTS 306.0381 161.5721 29.8471
*END_AREA
```

Uncompressed member SHA-256 values:

- `oceangreen.dat`:
  `450a2472a04af2d61d012d886b8bb86936abbc5c6ee7f0efc56983e531b32fdc`
- `surefallglade.tog`:
  `268aaab1872e390e83f615d1c13f920cdcf52b88cd293951f48e45d6e8148b14`

The parent record starts at uncompressed DAT offset **112338**, in source tile
22 (zero-based), grid `[100031,99999]`. Its repeated placement grid matches.
The terrain has 16 quads per tile and 12 units per vertex, so width is 192.

| Quantity | Original or independently replayed value |
| --- | --- |
| Parent local XYZ | `[65.70545959472656,94.31065368652344,6.2783203125]` |
| Parent XYZ degrees | `[0,0,90]` |
| Parent scale / Z adjustment | `[1,1,1]` / `-7` |
| Float32 parent world XYZ | `[6017.70556640625,-97.68934631347656,6.2783203125]` |
| Float32 child local XYZ | `[-285.25531005859375,-193.002197265625,-1.0880000591278076]` |
| Rotated child offset | `[193.002197265625,-285.25531005859375,-1.0880000591278076]` |
| Candidate center | `[6210.7080078125,-382.94464111328125,-1.8096797466278076]` |
| Candidate runtime grid | `[100032,99998]` |
| Candidate runtime local XYZ | `[66.7080078125,1.05535888671875,-1.8096797466278076]` |
| Summed/wrapped child XYZ degrees | `[180,0,0]` |
| Registration ZYX units | `[0,0,256]` |
| Unscaled half-extents | `[153.01904296875,80.78604888916016,14.92354965209961]` |

The parent uses an exact cardinal rotation, so the independently replayed
offset is exactly `[-local.y,local.x,local.z]` after float32 parsing. Parent
world XY and final center are rounded at the stores in the disassembled body.
The subsequent world/grid/world round trip preserves this fixture's values.

As a separate check against assuming terrain anchoring, the parent XY lies in
quad `(5,7)`, flag `0x01`, with corner heights in h00/h10/h11/h01 order:
`[-1.4557502269744873,42.05022430419922,4.490553855895996,-5.549067497253418]`.
Its fractional coordinates are approximately
`[0.47545496622721384,0.8592211405436201]`. The recovered top-level sampler
would produce **-0.19942712783813477**, whereas DAT stores **6.2783203125**.
Substituting that sampled height would change the candidate area center Z to
**-8.287426948547363**. That substitution is not supported by the group trace.

The registration slot implied by the original lists is zero-based index 5:
the three ATP records, `AWT_fishing`, `AWT_surfall`, then embedded
`AWT_watervolume`, then `AWT_cavewaterfall`. Region-free groups add no entries.
The area's candidate center lies in a different tile from the parent, which
does not change this order.

This fixture exercises a nonzero parent yaw and a child X rotation of -180
degrees. Its unit scale and cardinal angles do not validate general nonuniform
scale or arbitrary-angle rounding. Further controls contain one APV area in
`olddranik.tog` and ten APK areas in `shardslandingdungeon.tog`; no implementation
claim is made for those either.

## Remaining requirement before support

Find an active call or an equivalent writer that initializes the runtime
area composite before registration in this exact client build, or establish a
different, explicitly identified client build with a complete reachable path.
Then compare that initialized geometry against original fixtures. The existing
whole-set rejection for area-bearing groups should remain in place meanwhile;
a plausible dormant transform and a consistent arithmetic replay do not close
the active-path evidence gap.
