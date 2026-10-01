# Newer liquid regions: native transform evidence

Research begun 2026-09-29; runtime support extended 2026-10-01.
`LiquidRegions::load` supports the verified top-level heightmap DAT subset and
binary EQGZ v1/v2. Heightmap groups are accepted only after proving their complete
supported grammar contains no areas; unresolved groups/transforms and special
AFG constructors reject the whole set. Water surfaces never invent a volume or
lower bound. See [EQGZ_NATIVE_REGIONS.md](EQGZ_NATIVE_REGIONS.md) for the now
recovered binary reader-to-constructor boundary and the October 1 section below
for group validation.

The heightmap DAT path is traced from the original client's reader through
terrain-height anchoring and registered box containment. It differs from the
pinned EQEmu map generator in height interpolation, angle quantization and scale
handling. Historical checkpoint sections below retain the earlier evidence
limits; the October 1 extensions supersede their blanket binary/group exclusion.

## Reproducible sources and evidence limits

- Installed `EQGraphicsDX9.dll`, PE i386 preferred image base `0x10000000`,
  1,615,360 bytes, SHA-256
  `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
  Addresses below are virtual addresses at that image base. `.text` begins at
  VA `0x10001000` / file offset `0x400`; `.rdata` begins at VA `0x10133000` /
  file offset `0x131600`.
- Installed `eqgame.exe`, PE i386 preferred image base `0x00400000`,
  11,678,208 bytes, SHA-256
  `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
  Its `.text` begins at VA `0x00401000` / file offset `0x400`.
- [EQEmu zone-utilities at
  b361e63dd067e8959f5bf2341579f481d2374fd5](https://github.com/EQEmu/zone-utilities/tree/b361e63dd067e8959f5bf2341579f481d2374fd5),
  particularly `src/common/eqg_v4_loader.cpp`, `eqg_loader.cpp`,
  `eqg_structs.h`, and `src/awater/water_map.cpp`.
- Local EQEmu `zone/oriented_bounding_box.cpp` and `zone/water_map_v2.cpp`.
- Original `feerrott2.eqg`, `deadhills.eqg`, `lopingplains.eqg`, `buriedsea.eqg`,
  `oldcommons.eqg`, and `nektulos.eqg`. No proprietary binary data is committed.

This is static disassembly plus independent arithmetic replay, not a live-client
swimming experiment. The height expressions were checked against a small x87
instruction interpreter for all 66 original regions in these six zones. The
rotation expression was checked against the builder's x87 operations at five
nontrivial XYZ angle combinations. Reported coordinates are rounded and should
be checked with a tolerance; they are not byte-exact CPU emulation fixtures.

**The quad-diagonal finding now reaches visible native terrain indices.**
The renderer rebuilds bit `0x80` from generated triangle edges, then uploads
those indices to D3D. This establishes a link to visible topology, but not that
the original on-disk cache always equals the live tessellator's result. OpenEQ's
`TerrainTile::height_at` and full-resolution terrain construction now follow
that authored cache, as do the resulting collision triangles and relative
prop/light anchors. This is the supported full-grid compatibility contract;
the evidence does not justify claiming native adaptive tessellation or LOD parity.

## Heightmap DAT record grammar

The tile reader `0x10100dd0` reads region records for DAT version >= 4.
The branch begins at `0x10101466`; count is read at `0x10101474`. Each record is:

| Field | On-disk encoding |
| --- | --- |
| Name | NUL-terminated string |
| Type | 32-bit word |
| Alternate name | NUL-terminated string |
| Longitude, latitude | Two 32-bit grid words, biased by 100000 |
| Local position | XYZ float32 |
| Rotation | XYZ float32, **degrees** |
| Stored scale | XYZ float32 |
| Full size | XYZ float32 |

There are 56 fixed bytes after the alternate name: two grid words and four
vec3s. These are not 8 disposable padding bytes. The native client retains the
grid words in the region's composite location.

The reader creates a `CEQTerrainArea` through terrain-system vtable slot `+0x58`
(`0x100a3d40`, constructor `0x100a0c00`). Its vtable is `0x1013ff0c`.
Reader calls at `0x101014f1`, `0x101014fd`, and `0x10101540` set name, type,
and alternate name. The transform is copied at `0x10101542..0x10101591`.
The raw position Z is preserved separately, and the composite position Z is
zeroed at `0x10101595`. The composite setter is called at `0x101015a0`, the
authored-Z-offset setter at `0x101015b1`, and full-size setter at `0x101015d7`.
The region is linked into the tile's region list at `0x10101640`.

Useful native object fields and methods:

| Object offset | Meaning | Getter / setter |
| --- | --- | --- |
| `+0x0c`, `+0x10` | Disk grid longitude, latitude | Composite getter `0x100f1b20`, setter `0x100f1b70` |
| `+0x14..+0x1c` | Local XYZ; Z later becomes sampled height | Same composite methods |
| `+0x20..+0x28` | Authored XYZ rotation in degrees | Same composite methods |
| `+0x2c..+0x34` | Stored XYZ scale | Same composite methods |
| `+0x38` | Authored position-Z offset | `0x100f11a0` / `0x100f1240` |
| `+0x3c..+0x44` | Full size | `0x100f1160` / `0x100f13f0` |
| `+0x48` | Additional field, initialized to zero | `0x100f1190` / `0x100f1220` |
| `+0x4c` | Type word | `0x100f1140` / `0x100f11d0` |

The extra field `+0x48` is **not** the sampled terrain height. That height is
written into composite position Z (`+0x1c`).

## Native DAT center and terrain anchoring

Tile initialization `0x100f4360` calls `0x100f33c0` at `0x100f4485` after copying
the height grid. A later height refresh also calls it at `0x100f3651`.
`0x100f33c0` iterates the tile's region list, gets each composite location,
samples the tile at the region's local XY through tile vtable slot `+0x08`,
stores the result in composite Z (`0x100f3470`), and calls the composite setter
(`0x100f3480`). This is separate from debug rendering.

Both base tile vtable `0x101440a4` and client tile vtable `0x101408c4` use
`0x100f38c0` for that sampler. The latter is installed by `0x100a92d0`.
The height getter at slot `+0x04`, `0x100aa020`, reads row-major
`heights[y * (quads_per_tile + 1) + x]`.

For strict interior local XY, let `s = units_per_vertex`, `i=floor(x/s)`,
`j=floor(y/s)`, `u=x/s-i`, `v=y/s-j`. Let the quad's four heights be `h00`,
`h10`, `h11`, `h01` at `(i,j)`, `(i+1,j)`, `(i+1,j+1)`, `(i,j+1)`.
The sampler reads the quad flag at `0x100f3a19`:

```text
flag & 0x80 == 0:
    v <= u: H = h00 + u*(h10-h00) + v*(h11-h10)
    v >  u: H = h00 + u*(h11-h01) + v*(h01-h00)

flag & 0x80 != 0:
    u+v <= 1: H = h00 + u*(h10-h00) + v*(h01-h00)
    u+v >  1: H = h11 + (1-u)*(h01-h11) + (1-v)*(h10-h11)
```

These are two different piecewise planar diagonals, not bilinear interpolation.
The routines are at `0x100f3a37`, `0x100f3a93`, `0x100f3b04`, and `0x100f3b6d`.
The sampler rejects negative coordinates and coordinates beyond the tile width
with height zero. Exact positive tile-edge behavior needs its own fixture:
native code fetches four vertices before decrementing an edge cell index.
There is no evidence here for wrapping out-of-tile coordinates as the pinned
EQEmu loader does. All 66 audited region anchors are strictly inside their tile.

Grid-to-world conversion `0x100ebb80`, terrain vtable slot `+0xa8`, computes:

```text
W = quads_per_tile * units_per_vertex
center.x = (longitude - 100000) * W + local.x
center.y = (latitude  - 100000) * W + local.y
center.z = H(local.x, local.y) + authored_position.z
```

The final authored offset addition is in registered-region construction
`0x100a6b99..0x100a6ba6` (and repeated registration paths at `0x100a6d00` and
`0x100a6ea0`). No horizontal-axis exchange occurs here. OpenEQ's asset/scene
convention is already X/Y horizontal, Z up; server X/Y exchange belongs at the
protocol boundary.

## Visible triangles and the diagonal cache

Client tile method `0x100a9560` calls base index generation `0x100f6550` at
`0x100a962c`. The latter calls terrain tessellator `0x101077d0` or
`0x10107760`, depending on terrain vtable slot `+0x244`. Both pass through
`0x101064f0`, `0x10106970` and `0x10106a80`. The `0x101077d0` path remaps
generated vertex indices to grid indices `y * (quads_per_tile + 1) + x`
at `0x10107810..0x10107840`.

The generated index array is tile field `+0x90`, with triangle count at
`+0x84`. `0x100f6550` copies it into the DAT tile's cached array/count
(`+0x0c`, `+0x10`) at `0x100f6676..0x100f6699`, then:

1. Calls `0x100fe660`, which clears bit `0x80` in every quad flag
   (`and byte ptr [flags + i], 0x7f` at `0x100fe693`).
2. Visits every generated triangle and calls `0x100f4060` on its three edges
   at `0x100f66d0..0x100f675a`.
3. The edge helper skips horizontal/vertical edges and accepts negative-slope
   edges. Its slope threshold at `0x10133498` is float zero. For cells crossed
   by those edges, `0x100fe6a0` computes `row * q + col` and sets `0x80`
   (`or byte ptr [flags + i], 0x80` at `0x100fe6bf`).

Back in the client method, the D3D index buffer at tile `+0x104` is locked at
`0x100a9815`. The **same tile `+0x90` index array** is copied into it at
`0x100a9817..0x100a982f`, then unlocked at `0x100a9843`. D3D buffer creation
uses format `0x65` (`D3DFMT_INDEX16`); the draw triangle count is copied from
`+0x84` to `+0x100` at `0x100a97ef..0x100a97f5`.

Thus `0x80` records the negative diagonal of generated visible geometry, rather
than indicating collision-only or hidden geometry. The native terrain collision
routine also selects its quad triangles using `flags & 0x80` at `0x100cbcf4`;
it separately excludes hidden quads using bit `0x01` at `0x100cbc1e`.

Region anchoring happens in base tile initialization `0x100f4360` before this
later index-generation path. OpenEQ uses the authored cache for the recorded
anchor contract and visible full-grid triangles, without reproducing live LOD.
The ordinary height sampler remains f32 with its existing edge clamping, while
the stricter region-anchor sampler evaluates the recovered expressions with
f64 intermediates and rejects unsupported coordinates. A native Feerrott2 seam
fixture checks their agreement within f32 tolerance and the matching collision
surface; see `HEIGHTMAP_FORMAT.md`. The normal client's tessellator-mode
selection and later re-anchoring schedule are not recovered here.

## Native DAT rotation, extent and containment

The registered-region path `0x100a6baa..0x100a6c5b` reads full size and multiplies
it by `0.5`. It converts degree rotations to native angular units using the
float32 constant at `0x101402c8`, `1.4222222566604614` (`512/360`). It supplies
the angle vector in **Z, Y, X** order to registration helper `0x100bbe80`.
The stored scale fields are not applied in this registration path.

The helper dispatches to `0x100bdbe0`, then constructor `0x10021de0`, then
matrix construction `0x10021c50`:

1. `0x100c27f0` constructs rotation using the Z/Y/X angle vector. Its middle
   angle is negated.
2. Trig vtable `0x10134fb0`, slots `+0x08` and `+0x0c`, call `0x100b9d70` and
   `0x100b9d90`. Each **truncates toward zero**, masks with `0x1ff`, and reads a
   512-entry sine/cosine table. This is not interpolated trig. The conversion
   helper's SSE path uses `cvttsd2si` at `0x1010f295`; its x87 fallback implements
   the same truncation.
3. `0x100218c0` multiplies the three rotation-matrix rows by the corresponding
   signed half-size. Translation is filled from the supplied center.
4. `0x10021930` inverts that matrix. `0x10021d10` transforms query points by
   the inverse and requires each coordinate to be within **inclusive [-1,1]**.

Table initialization `0x100ba060` uses `fcos`; the sine table is its quarter-turn
shift, with exact cardinal values explicitly assigned. This confirms the trig
method identities, independently of names or exporter assumptions.

In conventional **column-vector** notation the DAT box transform is:

```text
q(a) = trunc_toward_zero(float32(a * float32(512/360)))
angle(a) = q(a) * 2*pi/512
R = Rz(angle(rot.z)) * Ry(-angle(rot.y)) * Rx(angle(rot.x))
M = T(center) * R * S(full_size / 2)
```

This describes the table-angle geometry; native lookup entries themselves are
float32. For original yaw-only fixtures, a stored yaw of -35 degrees becomes
`q=-49`, or **-34.453125 degrees**. A stored yaw of -20 becomes -19.6875;
-15 becomes -14.765625; 45 stays 45.

The separate debug matrix builder `0x100c9ed0` converts the same degree angles
but calls trig slots `+0x00` and `+0x04`, which interpolate table samples.
Its rendering cannot be used to establish exact registered containment edges.
The constructor also has a special `AFG` name branch. At
`0x10021e3a..0x10021e65`, it replaces both horizontal half-extents with their
maximum before calling the shared box builder. This does not apply to `AWT`,
`ALV`, or `AVW`. The bounded DAT and binary implementations both reject the
whole region set when any `AFG` record is present, including square records.
Treating a nonsquare AFG as an ordinary box can incorrectly expose a later
liquid region through what native construction makes a dry winning area.

This trace establishes the top-level DAT region path. It does not establish how
every possible region embedded in an instanced object/group inherits transforms.
The audited original records all have stored scale `[1,1,1]`; do not claim those
fixtures exercise nonunit scale, tilted liquids, or parent transforms.

## Classification and server differences

The pinned map writer classifies the first three name characters as follows:

| Prefix | Meaning |
| --- | --- |
| `AWT` | Water |
| `ALV` | Lava |
| `AVW` | Freezing water (`VWater`) |
| `APK` | PvP, not liquid |
| `ATP` | Zone line, not liquid |
| `ASL` | Ice/slippery floor, not liquid |
| `APV` | Generic area, not liquid |

These same prefix strings occur together at native VA `0x10135940..0x10135958`.
The graphics constructor uses them for debug colors; the following independent
trace reaches the gameplay callback.

### Native gameplay type callback

`eqgame.exe` registers callback `0x004a98a0` at `0x004b9bf5`, through graphics
vtable slot `+0x2c`. The matching DLL vtable is `0x1013becc`; that slot is
`0x1006b2d0`, which adjusts `this` by four and jumps to callback setter
`0x100bb4e0`. Getter `0x100bb4d0` reads the same pointer.

Generic graphics environment query `0x10069b00` passes a **null preferred
prefix** to region-name/type query `0x100bd020` at `0x10069b47`, then calls the
registered callback with that name and raw type at `0x10069b5d`. It returns its
caller-supplied default when the region or callback is missing. The zone-line
query `0x10068e20` instead explicitly prefers `ATP`.

For a name of at least four bytes beginning with uppercase `A`, the gameplay
callback produces these words (`t` is the original type):

| Prefix | Returned word | Native address |
| --- | --- | --- |
| `AWT` | `(t & 0xffffff00) \| 5` | `0x004a98e2` |
| `ALV` | `(t & 0xffffff00) \| 7` | `0x004a9908` |
| `AVW` | `(t & 0xffffff00) \| 8` | `0x004a992e` |
| `APK` | `t \| 0x40000000` | `0x004a9954` |
| `ATP` | `t \| 0x80000000` | `0x004a9977` |
| `ASL` | `t \| 0x10000000` | `0x004a99b3` |
| Other uppercase `A...` | `t` | `0x004a9b42` |

`ATP` also parses zone-line metadata from the name. Null/short names return the
original type at `0x004a9b47`; non-`A` names enter a separate classic-name
decoder. The prefix helpers, DLL `0x1011068d` and EXE `0x00935cf7`, perform
**case-sensitive byte comparisons** (`strncmp`), with no case conversion.
Do not normalize the authored name before applying these rules.

This is why `AWT` type 0 is water and `AWT` type 10 does not mean a distinct
liquid kind. Conversely, `APK`, `ATP` and `ASL` do **not** force a dry low byte:
they preserve it and add independent flags. Every audited `ATP` has type 0 and
is dry, but a hypothetical `ATP` type 5 cannot be treated as dry on its name
alone. Unrecognized names/types must not become automatic water or silently
vanish from overlap precedence.

### Native region order and overlap precedence

`0x100bcf60` searches the registered array at terrain `+0x74` with count
`+0x6c`. Its complete selection rule is:

```text
if preferred_prefix is present:
    return the first containing region matching its first three bytes, if any
return the first containing region whose first three bytes are not "APV"
return no region if neither pass found one
```

The preferred pass is `0x100bcf6a..0x100bcfaf`; generic fallback is
`0x100bcfb1..0x100bcffe`. Explicit `APV` preference can select an `APV` box;
only the generic fallback excludes it. `0x100bd020` forwards the chosen name
and raw type without first filtering for liquids. Consequently, a dry selected
region blocks a later overlapping wet one. A union of liquid boxes is wrong.

Registration order is **not global DAT file order**. Client region name setter
`0x100a0bd0` sets region byte `+0x9c` for prefix `ATP`, and loader `0x100a6880`
registers regions in two passes:

1. All top-level `ATP` regions, longitude ascending, latitude ascending, then
   source order within each tile (`0x100a6ae5..0x100a6ca3`).
2. The other top-level regions in that same grid order, followed by embedded
   group regions for each tile (`0x100a6ca9..0x100a7023`).

The grid bounds at `+0x04/+0x08/+0x0c/+0x10` are longitude/latitude minima and
maxima. Inner traversal increments latitude at `0x100a6c86`; outer traversal
increments longitude at `0x100a6c9b`. The DAT reader appends region list nodes
to the tile tail at `0x10101640..0x10101667`; getters `0x100f5ae0` and
`0x100f5b70` count and index that list from its head. `0x100bbe80` stores each
constructed region at the supplied sequential array index (`0x100bbec5`).

For top-level records with no embedded-region interference, the ordering key is
therefore `(not ATP, tile longitude, tile latitude, index within tile)`.
This gives `ATP` priority even in the generic, null-prefix query. Do not extend
that simple sort key to embedded groups without retaining the two-pass/group
insertion structure. Original file order demonstrably differs: Dead Hills'
first DAT region is `AWT_dh_pools_3F` at `[-9,0]`, whereas native top-level
registration begins with `AWT_dh_ocean_sidebay` at `[-15,-1]`.

**Do not copy the map writer's unknown-name fallback.** For heightmap records it
maps numeric types 1, 10, 9 and 5 to water, 7 to lava, 0 to zone line, and any
remaining unknown type to water. Original Feerrott2 and Dead Hills water records
have type 0, while Nektulos water has type 10. Type 0 is also used by every
audited `ATP` record. Names are currently the useful positive evidence; numeric
type alone is insufficient for a conservative liquid decoder.

The pinned server pipeline is a distinct contract:

- `eqg_v4_loader.cpp` adds an interpolated terrain height, but
  `HeightWithinQuad` always uses the `00--11` diagonal and ignores bit `0x80`.
  It uses the enclosing tile's origin and wraps local coordinates for sampling.
- It forwards unquantized degree rotations, stored scale, and half size.
- `oriented_bounding_box.cpp` uses `Rz * Ry * Rx` and **`T * S * R`**, with
  degree conversion `3.14159/180`. Nonuniform scale occurs after rotation.
  It swaps negative local min/max extents and has inclusive faces.
- `water_map_v2.cpp` exchanges server X/Y before querying; the first containing
  source region wins, including non-liquid kinds.

Thus native and generated-server boundaries must not be described as identical.
For example, the sampled anchor-height difference from ignoring the diagonal is
about 1.879780 at Dead Hills `AWT_dh_pools_3B`, 0.685452 at Loping Plains
`AWT_river7`, and 0.699655 at Old Commonlands `AWT_lake`. No water-surface height
was involved in these comparisons.

## Original DAT fixtures

The six archives contain 66 records. Repeated biased grid words agree with the
enclosing tile in every case; every scale is `[1,1,1]`. Every liquid-labelled
record is water; this sample does not test original lava/freezing records.

| Zone | Tile width | Name/type counts |
| --- | ---: | --- |
| Feerrott2 | 256 | 6 `AWT` type 0; 5 `ATP` type 0 |
| Dead Hills | 128 | 24 `AWT` type 0 |
| Loping Plains | 224 | 10 `AWT` type 1; 2 `ATP` type 0 |
| Buried Sea | 384 | 7 `AWT` type 1; 1 `ATP` type 0 |
| Old Commonlands | 128 | 1 `AWT` type 1; 5 `ATP` type 0 |
| Nektulos | 160 | 1 `AWT` type 10; 4 `ATP` type 0 |

### Feerrott2 pond: finite axis-aligned volume

DAT v21 record at byte 738595, `AWT_small_pond`, alternate name identical,
type 0, enclosing tile `[-3,-12]`, biased grid `[99997,99988]`:

```text
position = [27.157033920288086, 175.78500366210938, -2.802546739578247]
rotation = [0,0,0]
scale    = [1,1,1]
fullsize = [150,100,10]
sampled H = -53
center ~= [-740.842966, -2896.214996, -55.802547]
halfsize = [75,50,5]
```

| Scene point | Expected authored-region result |
| --- | --- |
| `[-740.842966,-2896.214996,-55.802547]` | Water |
| `[-665.942966,-2896.214996,-55.802547]` | Water, 0.1 inside X face |
| `[-665.742966,-2896.214996,-55.802547]` | Dry, 0.1 beyond X face |
| `[-740.842966,-2896.214996,-50.702547]` | Dry above |
| `[-740.842966,-2896.214996,-60.902547]` | Dry below |

### Dead Hills pool: rotated nonsquare volume

DAT v21 record at byte 611587, `AWT_dh_pools_3C`, alternate name identical,
type 0, enclosing tile `[-10,-2]`, biased grid `[99990,99998]`:

```text
position = [46.9638557434082, 118.45137023925781, -9.25]
rotation = [0,0,-35]
scale    = [1,1,1]
fullsize = [80,180,80]
sampled H ~= -84.332024429
center ~= [-1233.036144,-137.548630,-93.582024]
halfsize = [40,90,40]
registered yaw = -34.453125 degrees
```

| Scene point | Expected authored-region result |
| --- | --- |
| `[-1233.036144,-137.548630,-93.582024]` | Water |
| `[-1182.176854,-63.418051,-93.582024]` | Water, 0.1 inside local +Y face |
| `[-1182.063708,-63.253134,-93.582024]` | Dry, 0.1 beyond local +Y face |
| `[-1233.036144,-137.548630,-53.482024]` | Dry above |
| `[-1233.036144,-137.548630,-133.682024]` | Dry below |

The dry points in these two tables were checked against all authored records in
their respective audited DAT, not just the named box. In particular, the Dead
Hills local +X face is a bad whole-zone dry fixture: the neighboring
`AWT_dh_pools_3D` overlaps it. This overlap must remain wet when crossing between
pools. These are expected results from the recovered contract, not live-client
observations or currently passing OpenEQ EQG-volume tests.

## September 29: binary EQGZ boundary was unresolved

The following records the initial investigation; the caller boundary is now
resolved in [EQGZ_NATIVE_REGIONS.md](EQGZ_NATIVE_REGIONS.md).

Binary ZON v1/v2 region records are 40 bytes: name-table offset, center XYZ,
three orientation/unknown words, and signed extents XYZ. Pinned
`eqg_loader.cpp` treats the first orientation float as a 512-unit Z rotation
and the other two words as flags. Quail reads all three as float orientation.

Original Anguish `AWT_water` has center
`[700.9059,2.677391,-256.8711]`, raw orientation `[-1.5707964,-0,0]`, and
extents `[125.7306,-123.3301,11.7559]`. Crescent has 57 water records and one
zone line, with long adjoining river boxes. Their layout does not justify
blindly interpreting the first float as a -90-degree rotation.

The native shared constructor `0x10021c50` consumes **512-unit Z/Y/X angles**
and signed half-extents, as proved above. However, this investigation has not
followed binary ZON's actual file reader into that constructor. It could convert,
permute, or otherwise transform the raw record. Finding the constructor is not
proof that binary file floats are passed directly. Binary EQGZ remains disabled
until that caller boundary, signed extent basis, and non-square boundary
fixtures are recovered. DAT evidence must not be transplanted into EQGZ.

## September 29: initial safe implementation boundary

1. Retain DAT region records as metadata with the exact grammar, preserving
   source order, names, type, repeated grid, raw Z, rotations, scale and full
   size. Validate bounds, counts, finite values, and truncated records before
   they can contribute a query volume. Do not use `water.dat` or render material
   names to fill missing metadata.
2. Add CPU-only tests for the native DAT anchoring and angle contract, including
   both quad diagonals, all six finite faces, swept crossings with dry endpoints,
   the original fixtures above, adjacent-pool overlap, and explicit type-zero
   `ATP`, `APK`, `ASL`, `APV`, and unknown-name records. Test type preservation
   for modifiers, case-sensitive/short names, native registration order, dry
   overrides and generic `APV` exclusion. Synthetic `ALV` and `AVW` must stay
   distinct kinds; original lava/freezing fixtures are still desirable.
3. Before enabling volumes, review the intended compatibility target explicitly:
   native-authored geometry versus the generated EQEmu WTR map. Do not combine
   the native angle/scale interpretation with an unlabelled server-derived
   anchor. Start with audited top-level DAT regions, unit stored scale,
   yaw-only rotations and strict interior anchors if narrower support is wanted.
4. Preserve the recovered region precedence before converting anything to a
   liquid-only query. The current first-liquid-box API cannot automatically
   reproduce a first-region query where an overlapping dry region wins. An
   initial supported subset can require only top-level records, no embedded
   regions, matching enclosing/repeated grids, unique tiles, unit stored scale,
   yaw-only rotation, strictly interior anchors, and positive finite dimensions.
   Retain unsupported metadata and report its limits; do not silently drop a
   potentially winning unknown region and expose liquid behind it.
5. Keep binary EQGZ volumes unsupported until its reader-to-constructor path is
   verified. Handle instanced regions, nonunit stored scale, tilted boxes,
   mismatched repeated grids and exact tile-edge anchors as separately evidenced
   extensions rather than extrapolating from the original 66 records.

## September 29: initial CPU checkpoint

`Heightmap::regions` now retains each top-level DAT record, its original source
offset, tile/index order, names, raw type, repeated biased grid and four authored
vec3s. The parser rejects truncation, invalid strings and non-finite transforms;
finite unsupported transforms remain available as metadata.

The separate `terrain::regions` module provides `native_type_word`,
`top_level_registration_order`, `native_anchor_height`, `NativeRegionBox`, and
`NativeTopLevelRegions`. `LiquidRegions` now uses the whole-set helper for the
bounded runtime subset below. These helpers do not alter visible terrain,
object placement or collision geometry.
The box helper accepts original-fixture DAT versions 20/21, matching grids,
strict interior anchors, unit stored scale, yaw-only rotation and positive
dimensions. It rounds the sampled height and world center to their native
float32 storage points. Table trig values are approximated using float32
samples; CPU results are not promised bit-identical to the old x87 process.

The set helper retains every region in the recovered order and selects it
before interpreting its type. Unrecognized classic names yield an explicit
unsupported classification (`None`) while still occupying their precedence
position. Unknown uppercase `A...` names preserve their raw type, as native does.
Unsupported records fail construction. The helper also rejects duplicate tiles
and any object-group placements: their embedded region content is unresolved,
so their absence cannot be assumed. Individual top-level boxes can still be
studied in such zones without claiming a complete zone environment query.

`tests/terrain_regions.rs` checks all four height planes, grammar/truncation,
name/type behavior, native ordering, APV preference/fallback, dry and unknown
overrides, six finite faces, swept crossings, quantized nonsquare rotation,
unsupported paths and all 66 original records. The Feerrott2/Dead Hills fixtures
above now pass those isolated box tests, including the adjacent-pool overlap.
The unsupported original zones retain their empty gameplay-volume behavior.

## September 29: initial bounded runtime integration

`LiquidRegions::from_heightmap` retains the complete native ordered set when
there is explicit liquid evidence. Point queries use generic native selection
(no preferred prefix, skipping APV), then classify only case-sensitive names
of at least four bytes starting `AWT`, `ALV`, or `AVW`. These select water,
lava and freezing water respectively. Numeric unnamed types, classic names,
unknown `A...` names, and modifier-only `ATP`/`APK`/`ASL` records do not establish
liquid evidence. They still occupy their native precedence position. A winning
unsupported record suppresses liquid evidence; this is not a claim that the
native environment type is dry. For example, an `ATP` with low byte 5 remains
unsupported rather than being labelled water or intrinsically dry.

Segment queries intersect every non-APV region and partition the path at all
entry/exit fractions before choosing its first region. Thus a known dry or
unsupported winner subtracts its span from underlying water, and overlapping
liquid kinds retain native order. The swept path is OpenEQ movement geometry,
not a reverse-engineered original-client movement algorithm. Finite faces are
inclusive in point queries; zero-length and point-only tangencies produce no
positive-length liquid span. Public f32 query coordinates may round a computed
face outwards; the box itself is not expanded with an epsilon.

The archive loader shares exact declaration and DAT selection with rendering:
EQG precedes S3D, exact archived ZON precedes the case-insensitive loose ZON,
and a renamed/internal fallback requires an unambiguous actual archived
declaration naming its own DAT. It never guesses a DAT filename or falls back
to an obsolete S3D after finding EQG. Unsupported native transform/group sets
return empty volumes with a warning; malformed archive/declaration/DAT data
remain load errors. At this checkpoint binary EQGZ returned empty volumes with a
diagnostic; the October 1 implementation enables its verified boxes.

Tests cover rotation, liquid kinds, dry/unsupported overlap winners, ATP/grid
order, swept crossings, declaration precedence, ambiguity and absence of S3D
fallback. The original Maiden's Grave fixture checks its finite bounds and
depth through the public runtime API. Offline movement tests use both its
actual scene collision for idle/swimming/surfacing and an isolated collision
world for entering/leaving its finite side at 10, 30 and 120 FPS.

This compatibility target is the native startup-authored DAT volume, not the
generated EQEmu WTR interpretation. Server-map equality, damage/drowning,
region-trigger side effects, later height edits, embedded group areas and
binary EQGZ transforms are not established by this slice.

No server state, live sessions, renderer/GPU checks, or audio were changed for
this investigation.

## October 1: region-free object groups

The native TOG reader at `0x100f9fa0` dispatches two distinct record types:
`*BEGIN_OBJECT` at `0x100fa0c0..0x100fa151` parses through `0x100f9b70` and
appends to the group's object list at `+0x10`; `*BEGIN_AREA` at
`0x100fa159..0x100fa1fd` parses through `0x100f9530` and appends to its area
list at `+0x18`. Object `*FILE` records are typed file attachments in a separate
object list (`0x100f9e7f..0x100f9ee2`), not area constructors. The installed
subset uses `*FILE LIT filename` for lighting. Registration at
`0x100a6e75..0x100a6fd9` enumerates only the group's area list at `+0x18`, after
that tile's top-level non-ATP regions. With no area records, the previously
recovered top-level order is therefore complete regardless of group transforms.

`from_heightmap_with_groups` checks every distinct referenced group before
enabling top-level regions. It accepts complete `BEGIN/END_OBJECTGROUP` and
`BEGIN/END_OBJECT` blocks with known, nonduplicated name/position/rotation/scale
and LIT attachment fields. This is a conservative proof subset, not a claim to
parse the entire TOG grammar. Embedded areas (including dry/unknown ones),
unknown fields, malformed/truncated text and missing files reject the whole
set. Merely searching for an absent `BEGIN_AREA` string would not be sufficient.
Rendering and region validation share archived-first, case-insensitive loose
group resolution. An archive read failure cannot be hidden by a loose fallback.

After the conservative AFG guard, the 56-zone heightmap subset has 27 supported
wet top-level sets (previously 1), 8 complete sets with no supported liquid and
21 unresolved sets. This enables 26 additional zones, including original
Feerrott2 and Loping Plains, without
guessing any parent transform. Some unresolved group files are absent from the
installation; Oceangreen Hills, Old Dranik and Shard's Landing have real embedded
areas that still need their parent transform recovered. Other failures retain
unsupported anchor/version/grammar diagnostics. Final full survey:
`/tmp/openeq-native-regions-survey-final/`.

The initial `/tmp/openeq-region-free-group-survey/` predates the AFG guard
and counts Arelis as supported. The final survey above excludes it. A scan of all
56 original heightmap zones found only `AFG_arelis`, with full size
`[3510,3510,700]`; its equal horizontal extents make this particular native
adjustment a no-op, so no original Arelis mismatch was demonstrated. It remains
excluded to keep the supported constructor boundary consistent. A synthetic
`AFG [6,2,2]` followed by water demonstrates why unsupported records cannot be
accepted as ordinary boxes or discarded from precedence.

New tests preserve dry-region precedence through point and swept queries,
deduplicate repeated group references, reject malformed/missing/area-bearing
definitions and verify the original Feerrott pond's side/top/bottom boundaries
and finite vertical crossing. Original Loping Plains river containment also
passes through the runtime loader. No live character or server state changed.
