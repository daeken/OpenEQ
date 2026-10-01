# Binary EQGZ: native registered regions

Static research and bounded CPU implementation, 2026-10-01. Binary EQGZ
versions 1 and 2 register region records in **file order**. Their centers,
orientation vectors and signed half-extents reach the native box builder
unchanged. In particular, binary orientation is consumed as quantized **512-unit
turns**, even when a stored value resembles radians. Original Anguish and
Crescent use `-1.5707963705062866`, which becomes **-1 unit / -0.703125 degrees**
in registered containment. It does not become -90 degrees.

`crates/openeq-assets/src/binary_regions.rs` implements this bounded contract
without resolving any mesh, texture or light resources. `LiquidRegions` queries
the complete ordered set before classifying the selected name. Dry and unknown
regions therefore keep their ability to override later liquid regions.

## Reproducible evidence

The installed `EQGraphicsDX9.dll` is PE32 at preferred base `0x10000000`,
1,615,360 bytes, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below are virtual addresses in that binary. `.text` starts at
`0x10001000` / file offset `0x400`; `.rdata` starts at `0x10133000` / offset
`0x131600`. The shared constructor and classification research is also recorded
in `EQG_LIQUID_TRANSFORMS.md`.

This is static disassembly plus independent arithmetic replay. No native client
code was executed. It establishes registered box geometry and selection, not a
live swimming experiment or compatibility with every historical client build.

## Binary record layout and reader boundary

Both versions use a 28-byte header: `EQGZ`, version, string-table byte length,
model count, placed-object count, region count, and light count. The string table
is followed by `model_count` 32-bit name offsets. A version-1 object is 36 bytes.
A version-2 object has the same 36 bytes followed by a 32-bit lighting count and
that many 32-bit lighting entries. After objects come 40-byte regions, followed
by 32-byte lights.

| Region offset | Value passed to construction |
| --- | --- |
| `+0x00` | String-table name offset, fixed to a pointer by the reader |
| `+0x04..+0x0c` | Center XYZ, float32 |
| `+0x10..+0x18` | Raw Z/Y/X angle vector, float32 |
| `+0x1c..+0x24` | Signed XYZ half-extents, float32 |

Version-1 reader `0x10065240` checks magic at `0x10065268` and version at
`0x10065285`. Its object loop advances `0x24` bytes at `0x10065710`. Region
handling fixes only the name word at `0x10065735..0x10065744`, then passes the
region count and direct array pointer to `0x100bc6e0` at `0x10065783`.

Version-2 reader `0x100658f0` checks magic at `0x10065918` and version at
`0x1006593a`. The first object starts immediately after the model-offset array
(`0x10065968..0x10065972`). Each next object is computed as current + `0x28`
+ `4 * lighting_count` at `0x10065b4a..0x10065b59`, and restored into the iterator
at `0x10065e01`. Region handling fixes only the name word at
`0x10065e30..0x10065e44`; registration calls `0x100bc6e0` at `0x10065eac`.
Its pre-reader `0x100649f0` copies strings and fixes model names; it does not
transform region coordinates or rotations.

## Registration and active factory

In `0x100bc6e0`, `esi` is record + 8. The loads/stores at
`0x100bc725..0x100bc776` copy three independent vectors onto the stack:

- Center: record `+4`, `+8`, `+12`, unchanged.
- Half-extents: record `+28`, `+32`, `+36`, unchanged, preserving signs.
- Angles: record `+16`, `+20`, `+24`, unchanged.

There is no unit conversion, axis exchange, negation, division by two, terrain
height sampling, or application of a placed terrain object's transform here.
The routine supplies type zero at `0x100bc72d`. The virtual call at
`0x100bc78a` receives `(center*, half_extents*, angles*, name, 0)`.

For a precise stack check, let B be ESP after the routine's local allocation
and saved-register pushes. Center lives at B+`0x24`, extents at B+`0x18`, and
angles at B+`0x0c`. After name construction, `lea` instructions at `0x100bc779`,
`0x100bc77e`, and `0x100bc783` push angles, extents, then center in that order.

The actual client manager is constructed by `0x1001f180`. It first installs the
base vtable, then overwrites it with **`0x101358ac`** at `0x1001f18f`.
The world construction path creates this manager at `0x1006a876` and stores it
through `0x100bb430` at `0x1006a887`. Reader lookup `0x100bb420` returns the
stored manager; its vtable `+0x24`, `0x1001f890`, returns itself.

The active manager's slot zero is **`0x10022230`**, allocating the derived
0xf8-byte box. It calls `0x10022050` at `0x100222ab`; that constructor forwards
all five original arguments to **`0x10021de0`** at `0x100220b9`. The ordinary
non-AFG branch then calls **`0x10021c50`** at `0x10021e9f`. All transforms reach
the shared builder unchanged. The base vtable `0x10141998` instead selects
`0x100bdbe0`, allocating a 0x6c-byte box and reaching the same constructor.
Using only the base vtable would miss the actual client factory, although its
geometry contract is the same.

Registration stores the returned pointer at `manager+0x74[index]` at
`0x100bc78f`, increments the index at `0x100bc792`, and advances the source
record by `0x28` at `0x100bc793`. No ATP-first sorting occurs in this binary
path. Original Crescent has ATP at index 56, followed by water at index 57.

## Transform and selection

`0x10021c50` calls rotation builder `0x100c27f0`, scales its basis through
`0x100218c0`, fills translation from the supplied center, and inverts the
matrix through `0x10021930`. Containment `0x10021d10` tests the transformed
point against inclusive `[-1,1]` on all three axes.

The rotation builder consumes angle 0, **negates angle 1**, and consumes angle
2. Trig vtable `0x10134fb0` slots `+8` / `+12` dispatch to `0x100b9d70` /
`0x100b9d90`: truncate toward zero, mask `0x1ff`, then read float32 sine/cosine
entries. Table initialization `0x100ba060` uses cosine with a quarter-turn sine
shift and explicit exact cardinal entries.

In column-vector notation:

```text
q(a) = trunc_toward_zero(a) & 511
angle(a) = q(a) * 2*pi/512
M = T(center) * Rz(angle(raw[0])) * Ry(angle(-raw[1]))
              * Rx(angle(raw[2])) * S(signed_half_extents)
```

The implementation rounds table entries and matrix stores to float32. One
additional native spill matters for the general XYZ expression: `cx * sy` is
stored as float32 at `0x100c2899` before the final basis column uses it. The
`sx * sy` product remains in x87. A static interpreter of
`0x100c2889..0x100c2906` independently checked all nine basis entries for
`raw=[31.9,-73.8,19.2]`, center `[10,-20,30]`, signed extents `[2,-7,4]`.

Native matrix inversion uses x87 and stored float32 values; the implementation
uses f64 inversion. Thus the contract approximates finite boundary arithmetic,
not byte-exact CPU rounding. It does not replace inverse with transpose or add
an epsilon to inflate boxes. Segment clipping is a CPU geometry helper for the
same finite boxes, not a recovered native movement routine.

The active client query is vtable `0x101358ac` slot `+0x1c`, **`0x10021410`**.
An explicit preferred prefix gets a first pass at `0x100214b3..0x100214df`.
Generic fallback at `0x100215d9..0x10021612` scans in source order and returns
the first containing non-APV box. Base query `0x100bcf60` has the same selection
semantics. AWT/ALV/AVW classification happens after selection; unsupported and
dry names must not be filtered away before deciding which box wins.

## Original fixtures and supported limits

Original Anguish is version 1 with two regions. `AWT_water` is source index 1,
offset 48057: center `[700.9058837890625, 2.6773910522460938,
-256.8711242675781]`, raw angles `[-1.5707963705062866,-0,0]`, signed extents
`[125.73063659667969,-123.33009338378906,11.755911827087402]`.

Original Crescent is version 2 with 58 regions. `AWT_river30` is source index
31, offset 2726476: center `[-1091.171875,-2348.66162109375,
-204.40826416015625]`, raw angles `[-1.5707963705062866,0,0]`, signed extents
`[6.1612548828125,-117.21435546875,35.02192687988281]`. Its very different
horizontal extents make it a useful orientation fixture: center + `[0,100,0]`
is inside, while center + `[100,0,0]` is outside.

Portable tests include original numeric records and independently replayed
points just inside/outside both horizontal faces, along with general XYZ,
signed cardinal faces, malformed/truncated records and dry-overlap swept
queries. An explicit original-assets test also checks the real files and
`LiquidRegions::load` for wet centers, dry sides, finite tops/bottoms and swept
vertical intervals. Original assets are not committed.

AFG records now use the recovered signed horizontal extent normalization before
rotation; see [EQG_AFG_REGIONS.md](EQG_AFG_REGIONS.md). Raw extents remain available
separately from registered extents. The whole set is rejected on unknown versions,
nonfinite or overflowing angles, zero/subnormal registered extents, unusable basis or
inverse arithmetic, invalid names, incomplete arrays, or unrecognized trailing
bytes. Names must be NUL-terminated UTF-8 with at most 4096 bytes. This is a
conservative resource/encoding boundary, not a recovered native name limit.
All object lighting arrays and final light records are bounds-checked. A dry
or otherwise unsupported region never disappears to expose water behind it.

## Terrain coordinates and traversability check

The region center is the center of its authored volume, not a promise of a
walkable or reachable point. A follow-up comparison against original terrain
found centers below nearby physical surfaces. That finding does not justify
moving regions or applying the ZON terrain placement transform.

The original terrain placements are nonidentity:

| Zone / terrain | Placement XYZ | Raw yaw / scale |
| --- | --- | --- |
| Anguish / `TER_island.TER` | `[600.184326,-0.001665,-300]` | `-1.57079637` / `1` |
| Crescent / `TER_crescent.TER` | `[-193.844757,-1579.323364,384.824829]` | `-1.57079637` / `1` |

However, native TER reader `0x100643b0` registers terrain separately. At
`0x100647e0..0x100647ed` it calls manager vtable `+0x28`, **`0x10020970`**.
That routine copies source vertex XYZ directly into manager `+0xa0` at
`0x100209b8..0x100209d1`, without the ZON placement's rotation or translation.
The version-2 object reader explicitly skips object index zero at
`0x10065b5d..0x10065b61`; that is Crescent's terrain record. The ZON object's
lighting array is still consumed. Ordinary object rotations use a separate
radians-to-native-unit conversion at `0x10065c2f..0x10065c54` (version 1:
`0x100653cf..0x100653f4`), with constants `+/-81.4873275756836` at
`0x1013b830` / `0x1013b858`. This object conversion is absent from region
registration.

There is also no coordinate remap at the recovered region-query boundary.
EXE wrapper `0x004ae550` forwards its first three coordinates unchanged to
the scene's `+0x1c` method, DLL **`0x10069b00`**. That method copies them into
XYZ order at `0x10069b12..0x10069b34`; `0x100bd020` passes the vector directly
to the registered-region query at `0x100bd041..0x100bd045`. No terrain-specific
offset, horizontal swap, rotation or vertical adjustment occurs in this chain.

Independent vertical intersections with original TER triangles found:

- At Anguish `AWT_water` center XY, a physical `grnd` face is at Z
  **-361.92725**, about **105.056** below the volume center. A downward search
  limited to 100 units misses it. Other physical prison faces are above the
  volume, including Z **-198.04010** and **-16.71902**. The large visible
  MaxWater surface is at **-288.19308**, below this particular authored box;
  passable ground faces around **-245.3** lie near the box's top **-245.11521**.
- At Crescent `AWT_river30` center XY, a physical terrain face is at
  **-163.83597**, above the volume's top **-169.38634**. At the same X and
  **Y + 60**, the physical river floor is approximately **-180.670**, while a
  passable waterfall face is **-166.508**. Here the upper portion of the box
  is above the floor, even though its center is below the floor.

These are original-terrain intersections, not a full gameplay or reachability
test, and do not prove that any chosen center is a playable swimming location.
The traces support retaining raw terrain/region coordinates and show that a
center-below-floor observation alone is insufficient evidence of a coordinate
bug. The intended gameplay use of Anguish's isolated authored box remains
unresolved. Runtime geometry support and isolated movement tests should not be
described as verified live traversal of that location.
