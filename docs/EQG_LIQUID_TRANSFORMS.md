# Newer liquid regions: native transform evidence

Research checkpoint, 2026-09-29. **This does not enable EQG swimming volumes.**
`LiquidRegions::load` remains conservative for both binary EQGZ and heightmap
EQTZP zones. Water surfaces are not used to invent a volume or a lower bound.

The heightmap DAT path is now traced from the original client's reader through
terrain-height anchoring and registered box containment. It differs from the
pinned EQEmu map generator in height interpolation, angle quantization and scale
handling. Binary EQGZ file-to-box orientation still has an unresolved caller
boundary; the shared box constructor alone does not resolve that format.

## Reproducible sources and evidence limits

- Installed `EQGraphicsDX9.dll`, PE i386 preferred image base `0x10000000`,
  1,615,360 bytes, SHA-256
  `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
  Addresses below are virtual addresses at that image base. `.text` begins at
  VA `0x10001000` / file offset `0x400`; `.rdata` begins at VA `0x10133000` /
  file offset `0x131600`.
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

**The quad-diagonal finding below is proved for the native CPU height sampler.**
Visible terrain index generation has not been traced in this investigation.
Whether rendered native terrain uses the same diagonal remains unverified.
Do not silently change visible terrain triangulation on the strength of this
sampling evidence. OpenEQ's current fixed-diagonal `TerrainTile::height_at` and
terrain mesh construction need a separate review.

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
The constructor also has a special `AFG` name branch; it does not apply to
`AWT`, `ALV`, or `AVW`. Avoid treating generic regions as interchangeable with
liquid regions.

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
The constructor compares them to assign debug colors. That color dispatch alone
does not prove gameplay classification. Native region query `0x100bcf60` calls
box containment and performs prefix selection with a secondary non-`APV`
fallback. Full native overlap/type precedence is not yet specified here.

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

## Binary EQGZ boundary still unresolved

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

## Smallest safe implementation boundary

1. Retain DAT region records as metadata with the exact grammar, preserving
   source order, names, type, repeated grid, raw Z, rotations, scale and full
   size. Validate bounds, counts, finite values, and truncated records before
   they can contribute a query volume. Do not use `water.dat` or render material
   names to fill missing metadata.
2. Add CPU-only tests for the native DAT anchoring and angle contract, including
   both quad diagonals, all six finite faces, swept crossings with dry endpoints,
   the original fixtures above, adjacent-pool overlap, and explicit dry `ATP`,
   `APK`, `ASL`, `APV`, and unknown-name records. Synthetic `ALV` and `AVW` must
   stay distinct kinds; original lava/freezing fixtures are still desirable.
3. Before enabling volumes, review the intended compatibility target explicitly:
   native-authored geometry versus the generated EQEmu WTR map. Do not combine
   the native angle/scale interpretation with an unlabelled server-derived
   anchor. Start with audited top-level DAT regions, unit stored scale,
   yaw-only rotations and strict interior anchors if narrower support is wanted.
4. Resolve overlap precedence and non-liquid exclusions before converting to a
   liquid-only list. The current first-liquid-box API cannot automatically
   reproduce a first-region query where an overlapping dry region wins.
5. Keep binary EQGZ volumes unsupported until its reader-to-constructor path is
   verified. Handle instanced regions, nonunit stored scale, tilted boxes,
   mismatched repeated grids and exact tile-edge anchors as separately evidenced
   extensions rather than extrapolating from the original 66 records.

No runtime files, server state, live sessions, renderer/GPU checks, or audio were
changed for this investigation.
