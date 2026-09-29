# Native heightmap water reader and mesh construction

This is static analysis of the installed original client's terrain-water path,
performed on 2026-09-29. No client code was executed, no original files were
modified, and no binary or disassembly payload is included in this repository.
The evidence corrects the earlier signed-presence inference in
[HEIGHTMAP_WATER_PLAN.md](HEIGHTMAP_WATER_PLAN.md). It does not establish complete
support for every DAT version, final water shading, or liquid movement behavior.

## Binary identity and reproduction

The analyzed file is `/Users/daeken/EverQuest/EQGraphicsDX9.dll`:

- Size: 1,615,360 bytes.
- SHA-256: `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
- PE i386, image base `0x10000000`.

| Section | Virtual address | File offset | Raw size |
| --- | --- | --- | --- |
| `.text` | `0x10001000` | `0x400` | `0x131200` |
| `.rdata` | `0x10133000` | `0x131600` | `0x2ba00` |
| `.data` | `0x1015f000` | `0x15d000` | `0x1ca00` |

Addresses below are virtual addresses in this image. LLVM objdump's displayed
`CreateGraphicsEngine+...` labels are export-relative labels, not function names.
Class names used here are supported by RTTI/vtable and constructor references.

```sh
shasum -a 256 /Users/daeken/EverQuest/EQGraphicsDX9.dll
objdump --disassemble --x86-asm-syntax=intel --no-show-raw-insn \
  --start-address=0x10100dd0 --stop-address=0x10101040 \
  /Users/daeken/EverQuest/EQGraphicsDX9.dll
```

The same disassembly command can select the other address ranges below. The
temporary full disassembly used during investigation was
`/tmp/eqgraphics-disassembly.txt`; it is not required as an input to OpenEQ.

## The first DAT word is the version

The full file loader at `0x1010caf0` forms `<terrain-name>.dat` using the `.dat`
string at `0x10145140`. At `0x1010cb92..0x1010cb98` it reads the first word of
the returned file data and saves that value as the version. It compares the
version against the global at `0x10145148`, whose value is 22. After skipping the
first eight bytes, versions at least 19 read the third header word and the
base-texture NUL string, then the tile count. The call at
`0x1010cc2a..0x1010cc3a` passes the saved first header word and the current cursor
to the full tile reader at `0x10100dd0`.

The chunk loader at `0x1010d720` reads `%d_%d.dat`, likewise takes the first word
at `0x1010d7d1..0x1010d7de`, and passes it at `0x1010d810..0x1010d81b`. This
independent caller also rejects versions above 22.

These call sites establish the source of the tile reader's argument; it is not
an unrelated flag inferred from the fixtures' values 20 and 21.

## Water-field grammar

After tile heights, colors and quad flags, the full reader follows this shape:

```text
if version >= 12:
    base_water_elevation = read_f32()
    if version >= 21:
        material_index = read_i32()
        presence_byte = read_u8()
        if presence_byte != 0:
            local_bounds = read_4_f32()
            construct_sheet(base_water_elevation, material_index, local_bounds)
    trailing_or_legacy_value = read_f32()
layer_count = read_u32()
```

| Address | Evidence |
| --- | --- |
| `0x10100e55` | Restores the version argument to EBP after height copying. |
| `0x10100ef8` | Compares version with 12; lower versions skip the water fields. |
| `0x10100f01..0x10100f06` | Reads base elevation to runtime tile `+0x28`; advances four bytes. |
| `0x10100f09` | Compares version with 21; lower versions jump to `0x10101024`. |
| `0x10100f12..0x10100f1d` | Unconditionally reads the word to `+0x2c` and byte to `+0x30`; advances five bytes. |
| `0x10100f20..0x10100f22` | Tests the byte against zero with `test cl,cl` / `je`; no signed-positive test. |
| `0x10100f2d..0x10100f56` | Nonzero byte reads four floats to `+0x34,+0x38,+0x3c,+0x40`; advances 16 bytes. |
| `0x10101024..0x10101029` | Reads the final/legacy float to `+0x24` for every version at least 12. |
| `0x1010102c` | Reads the next layer count. |

Thus modern selectors zero and negative still consume the byte and final float.
Byte values `0x80` and `0xff` still consume the quartet. Version 20 has only two
floats, even if the second float has positive signed raw bits. The earlier
signed-positive gates matched the six audited archives accidentally and could
misalign these cases.

OpenEQ now preserves the raw word bits and uses the version gate. An extension
is present only for modern indexed records, and `material_index()` returns its
signed selector including zero/negative values. Legacy second-float bits do not
become selectors. The byte is stored as `i8` for API continuity; the predicate
is `!= 0`. Float payload bits are preserved without assigning a bottom depth or
other meaning to the trailing value.

The complete original fixtures cover DAT versions 20 and 21. Native evidence
for the water branch extends to 22, but this does not validate every surrounding
OpenEQ layout. The object-placement extra-word condition is now independently
traced below; its native predicate is version at least 22. Versions below 12
omit water entirely in the original reader and are not claimed to work in OpenEQ.

A second sparse/update reader at `0x10100070` has the same water version branch
but a different sheet-creation fall-through. It must not be used interchangeably
with the full file-load path to infer whether an inactive tile draws a sheet.

## Selector and rectangle data flow

The terrain system vtable at `0x10140644` has these relevant slots:

| Slot | Target | Role |
| --- | --- | --- |
| `+0xa8` | `0x100ebb80` | Grid/local coordinates to world coordinates. |
| `+0x1a0` | `0x100ea9d0` | Vertices per side, options `+0x18`. |
| `+0x1a4` | `0x100ea9e0` | Quads per side, options `+0x1c`. |
| `+0x1a8` | `0x100ea9f0` | Tile width, options `+0x20`. |
| `+0x1ac` | `0x100ea9c0` | Vertex spacing, options `+0x14`. |
| `+0x230` | `0x100a4380` | Water-sheet factory. |
| `+0x234` | `0x100a4490` | Water-material-data factory. |
| `+0x238` | `0x100eb360` | Indexed lookup wrapper leading to `0x100fae00`. |

The full reader passes the word at tile `+0x2c` to lookup slot `+0x238` at
`0x10100fbc..0x10100fd2`. The lookup at `0x100fae00` walks the linked material
list and compares each node's index at `+0x10` exactly. This is a selector, not
a count of water records.

At `0x10100fd4..0x1010101a`, all four bounds are translated by the tile's world
XY and passed with the base elevation to factory slot `+0x230`. The coordinate
helper `0x100ebb80` subtracts 100000 from biased grid coordinates, multiplies by
tile width, and adds local offsets. There is no EQEmu X/Y swap in this path.

The factory calls `CEQWaterSheet` constructor `0x100ac120`, which calls
`CWaterSheet` constructor `0x100fab70`. The translated bounds become object
fields `+0x10,+0x14,+0x18,+0x1c`, elevation becomes `+0x20`, and the material
pointer becomes `+0x3c` (copied to derived field `+0x58`). A null material pointer
causes the native base constructor to request a default material. OpenEQ's
metadata resolver intentionally reports `Missing`; this discovery alone does
not authorize synthesizing a surface or adopting that runtime fallback.

The derived constructor builds a cache key from material ID, tile-relative
start position and extents at `0x100ac217..0x100ac24a`. On a cache miss,
`0x100ac284..0x100ac2a5` calls geometry generator `0x100ab0f0`, material/mesh
builder `0x100aaf40`, and actor-definition builder `0x100aae90`. It then creates
the actor through `0x100abd40`, using the rectangle midpoint and authored water
elevation. Therefore a cache key is only one consumer of these bounds.

## What the native geometry proves

The indexed branch of `0x100ab0f0` is selected for material IDs 1 through 9999.
IDs zero or outside that range take another branch beginning at `0x100ab6e4`,
which is not decoded here.

For the indexed branch:

1. `0x100ab131..0x100ab163` computes width/height from the rectangle and their
   half extents.
2. `0x100ab167..0x100ab1f1` takes subdivision counts from the truncated extent
   divided by terrain spacing, with a minimum of one subdivision per axis.
3. `0x100ab1f1..0x100ab227` allocates two sets of `(nx+1)*(ny+1)` vertices,
   44 bytes per vertex.
4. `0x100ab2e0..0x100ab4f2` fills the full rectangle grid centered on the origin:
   `x = i/nx*width - width/2`, `y = j/ny*height - height/2`, `z = 0`. The two
   sets have opposing Z normals.
5. `0x100ab4f8..0x100ab6e3` emits four triangles per grid cell, covering both
   sides. No terrain height or quad-flag query appears in this examined branch.

The material/mesh builder submits those vertex and triangle arrays to mesh
creation at `0x100ab004..0x100ab02c`. The actor midpoint transform places the
rectangle at its translated bounds and elevation.

This establishes that the authored rectangle bounds the generated planar mesh
in XY. It is not merely editor or cache metadata. The examined CPU construction
does not clip individual cells against terrain heights. By itself this CPU
evidence does not establish later shader behavior; the subsequent shader audit
below resolves displacement and discard. Complete final appearance and
underwater behavior remain separate questions.

## CPU UVs, vertex packing, and effect selection

The helper at `0x100ab060` builds a global float lookup table using
`i/(vertices_per_side-1)`. The indexed mesh code at
`0x100ab22d..0x100ab2bf` takes the starting world X/Y modulo tile width and
adjusts negative remainders to positive. At `0x100ab3a0..0x100ab3ed` it rounds
grid positions divided by spacing using `+0.5` and integer conversion and
indexes that table. The resulting pair is stored in both UV channels at vertex
`+0x1c,+0x20` and `+0x24,+0x28`.

The material field parser is `0x100aa250` on `CEQWaterSheetData`, whose
constructor is `0x100abb40`. `*UVSCALE` is stored at material `+0x20` and sent
separately as shader parameter `e_fUVScale` at `0x100aa837..0x100aa850`.
Consequently the existing finite-sheet world-coordinate/UV-scale formula is
not the native indexed CPU mapping.

There is an essential fixed-point conversion between those CPU UVs and the
shader. `0x100aaf40` creates a `CSimpleModelDefinition`; its RTTI-backed vtable
is `0x1013a968`, installed by constructor `0x10057180`. The vertex-copy slot
`+0x18` calls `0x100579f0`. At `0x10057a44..0x10057a67`, that routine copies both
CPU UV pairs unchanged from input vertex `+0x1c..+0x28` into the model's auxiliary
vertex array at object `+0x70`.

The model upload path passes that array to `0x1008ea30` at `0x10056ee4`. Upload
loads constant `0x101357b4`, whose float value is **256**, and multiplies each UV
component by it before calling integer conversion `0x1010f280` and storing a
16-bit word. Examples are `0x1008ed6b..0x1008ed87` and
`0x1008ef57..0x1008ef73`; the other vertex layouts use the same conversion.
The helper's SSE branch uses `cvttsd2si` at `0x1010f295`, establishing truncation
toward zero rather than rounding to nearest. Vertex declarations including
`0x10175168` and `0x10175338` declare `TEXCOORD0` as **SHORT2**, not SHORT2N;
the declaration entries are at `0x10175180` and `0x10175350`, respectively.
They are registered from `0x100825a0`. The GPU therefore receives integer UV
values as floats, with a scale of 256 still applied.

The render descriptor produced by upload retains material type `0x35` at
`+0x8` (`0x1008f0a8..0x1008f0c4`). The non-region dispatch table at
`0x1008fd78` maps this type to `0x1008fb5e`, which selects pass descriptor
`0xc1`. The descriptor array built at `0x100826d0` starts at stack `+0x1c`
with 28-byte records; descriptor `0xc1` starts
at `+0x1538`. Its shader field at `+0x1544` is set to `0x2f` at `0x1008541e`.
Shader-name table `0x10175548`, index `0x2f`, points to string `0x1013d758`,
`SPL/SModelWater.fxo`. `0x10087089` resolves this name table when loading an
effect. This connects the constructed simple-model water sheet to the shader
analyzed below; the presence of similarly named RegionWater/SkinMeshWater
files alone would not have established that connection.

Material builder `0x100aa630` selects material type `0x35` and supplies the
diffuse water texture, authored normal/environment maps, Fresnel bias/power,
reflection amount/color, two water colors, and UV scale. No terrain-height or
terrain-depth sampler was found in that examined setup. This limited observation
is not evidence of a global absence of later clipping.

## Decoded SModelWater shader contract

`/Users/daeken/EverQuest/RenderEffects/SPL/SModelWater.fxo` is 26,920 bytes,
SHA-256 `1de5e6c57a6fa788b7cd68d552a5e1aebc0af84f078e22c1c08c962046ac47f1`.
All offsets in this section are **file byte offsets**, not DLL virtual addresses.
The compiled effect container was parsed through exact EOF `0x6928`: 40
parameters, four techniques, 14 object slots, three empty string objects and
11 resource records. Resource records explicitly associate each technique with
its shader blobs:

| Technique suffix | Vertex program | Pixel program |
| --- | --- | --- |
| `DX9_VS1_PS20` | `0x5628`, 4,728 bytes | `0x4e1c`, 2,036 bytes |
| `DX8_VS1_PS14` | `0x3a38`, 5,068 bytes | `0x37a0`, 640 bytes |
| `DX8_VS1_PS11` | `0x2518`, 4,720 bytes | `0x22b4`, 588 bytes |
| `DX6_VS1_PS0_NoEnvNoShineNoGlowNoBump` | `0x14e0`, 3,164 bytes | None |

All four vertex programs and all three pixel programs were decoded to their
END tokens, skipping sized CTAB/PRES comments. The programmable passes set only
their vertex and pixel shader; they do **not** establish depth, blend, culling,
alpha-test or normal-map address modes. Those states belong to the surrounding
renderer and cannot be inferred from this effect container.

### Flat geometry and absence of shader clipping

The DX9 vertex program's CTAB identifies world matrix registers `c10..c13` and
view-projection registers `c14..c17`. Position input `v0` is transformed into
world XYZ at `0x6238..0x6258`, its W is computed at `0x6314`, and clip position
is written at `0x6338`, `0x6358`, `0x6378`, `0x6398`/`0x6808`. There are no wave,
time, normal-map, UV, or terrain-height terms in this position data flow.
The older vertex variants likewise use only the world/view-projection transforms
for position. The generated rectangle stays planar in these shaders.

No pixel program contains `TEXKILL`, a depth output, a terrain/depth sampler,
or a conditional discard equivalent. In the DX9 program, normal-map reads occur
at `0x535c` and `0x536c`, and the environment read at `0x54d0`; its only declared
samplers are the authored 2D normal map and environment cube. It writes color
at `0x5600`. It adjusts shading normals using distance and combines Fresnel,
reflection and lighting; that does not change mesh geometry or add a terrain
shoreline clip.

Together with the CPU mesh evidence, this is sufficient to render the authored
rectangle with ordinary depth occlusion as a bounded surface implementation.
It is not a complete audit of external render states, offscreen passes, global
clip planes, translucency ordering or original-client screenshots.

### Effective normal-map coordinates

In the DX9 vertex program, CTAB identifies `e_fUVScale` as `c40.x`. Constants
at `0x61d8` are `1/256` and `1/128`. The relevant operations at
`0x6814..0x687c` implement:

```text
packed_uv = v3.xy
uv1 = packed_uv * UVScale / 256 - [c6.x, c7.x]
uv2 = packed_uv * UVScale / 128 + [c8.x, c9.x]
```

The PRES block at `0x5abc` includes its own CTAB, CLIT and FXLC sections. Its
instructions at `0x5f58..0x60c0` compute a signed remainder using absolute value,
fractional part and sign restoration: `phase = fmod(a_fTime, 100)`. The four
outputs at `0x60ec..0x6170` are `phase * Slide1X`, `phase * Slide1Y`,
`phase * Slide2X`, and `phase * Slide2Y`, in GPU `c6..c9.x`.
The effect's defaults are Slide1=`[0.02,0.02]`, Slide2=`[0.03,0.03]`.
The native material constructor sets ten texture/material values, including UV
scale, but does not set these slide parameters. Engine time's exact unit/epoch
and possible external overrides have not been traced; animation timing in
OpenEQ should identify its seconds-based convention as an approximation until
that remaining bridge is verified.

For the audited grid-aligned rectangles, let `q` be quads per tile and `(gx,gy)`
the rounded local grid coordinates of a generated vertex. Then:

```text
cpu_uv = f32([gx,gy]) / f32(q)
packed_uv = trunc(256 * cpu_uv)             // signed SHORT2
asset_uv = packed_uv / 256
uv1 = asset_uv * UVScale - phase * [0.02,0.02]
uv2 = 2 * asset_uv * UVScale + phase * [0.03,0.03]
```

The quantization happens **per vertex**, before interpolation. Collapsing a
subdivided rectangle to two triangles can change interpolation where `q` does
not divide 256; preserving the native grid avoids that difference.
At tile boundaries, endpoints remain 0 and 1, rather than being wrapped to zero
in the mesh. Texture repetition makes the opposite endpoints equivalent for
integer UV scales. The active materials in Feerrott2, Dead Hills, Loping Plains
(both indices), and Buried Sea (both indices) all author scale **1.000**.
Old Commonlands's 83 inactive index-zero definitions author **3.100**; it has no
active indexed rectangles and is not evidence for continuity with noninteger
scales. Do not force a noninteger scale to an integer or promise a seamless join
for all possible material records.

The time reset shifts the default two layers by exactly two and three periods,
respectively, under repeat addressing. The effect sets normal-map min/mag/mip
filters to linear but leaves addressing unstated; repeat is the conservative
OpenEQ choice to reproduce the audited tile-edge contract. The environment
sampler explicitly uses clamp on all three axes and disables mip filtering.

The existing OpenEQ water shader uses a world-plane `/80` mapping, unrelated
scroll rates and a second-layer scale of 1.37. Indexed water needs an explicit
material mode using asset UVs and the contract above; changing that shared
default would alter existing EQG and finite-sheet behavior.

### Reproducing shader interpretation

Temporary, authored audit programs were `/tmp/openeq-shader-audit.py`,
`/tmp/openeq-effect-audit.py`, and `/tmp/openeq-preshader-audit.py`; their outputs
are temporary derived disassembly. They parsed the effect offsets, resource
sizes, shader END tokens and CTAB register assignments, rather than searching
arbitrary binary words for opcodes. Format references used were Wine 10.0's
[D3D9 tokens](https://github.com/wine-mirror/wine/blob/wine-10.0/include/d3d9types.h),
[effect parser](https://github.com/wine-mirror/wine/blob/wine-10.0/dlls/d3dx9_36/effect.c),
and [preshader parser](https://github.com/wine-mirror/wine/blob/wine-10.0/dlls/d3dx9_36/preshader.c).
No Wine runtime or original shader execution was needed.

## Version-22 object-placement word

The full DAT reader's placement block starts after layer parsing at
`0x10101236`. It reloads the saved DAT version into EBP at `0x1010127d`, reads
the model string, reads the ecosystem string for versions at least 6, and then
consumes repeated grid coordinates plus position/rotation/scale as 44 bytes at
`0x101012d3..0x10101323`.

- `0x10101327..0x10101332`: version at least 16 consumes the following byte.
- `0x10101333`: initializes EAX to -1.
- `0x10101336..0x10101339`: compares version with **22** and skips the extra word
  if it is lower.
- `0x1010133b..0x1010133d`: reads the extra 32-bit word and advances four bytes.
- `0x1010134f..0x10101358`: passes that value and the model name to terrain-system
  vtable slot `+0x48`.

Thus the word is present for version **>=22**, not according to header bit 1.
Its argument role is preserved here without guessing an application-level name.
The existing OpenEQ expression `header[0] & 2 != 0` happens to match versions
20, 21 and 22, but is not the native grammar. This research is read-only;
correcting that predicate and validating a complete version-22 fixture are
separate implementation work. Older placement/layout gates above likewise do
not imply that all older files are supported by OpenEQ.

## Regression coverage and next verification

Synthetic tests cover explicit version 20/21 streams, selectors 1/2/0/-1 and
`i32::MIN`, byte values 0/1/127/128/255, multiple-record alignment, positive and
negative legacy second floats, exact NaN/negative-zero bits, every truncated
water-field byte, truncated complete streams, and extra trailing bytes.
Indexed-text tests preserve finite-only API behavior and reject finite/indexed
blocks nested in either direction.

The separately invoked original-data regression verifies exact EOF and water
distributions for Feerrott2, Dead Hills, Loping Plains, Buried Sea, Old Commonlands
and Nektulos:

```sh
cargo test -p openeq-assets --test terrain water -- --nocapture
cargo test -p openeq-assets --test terrain \
  original_six_zones_preserve_indexed_water_metadata_and_lookup -- --ignored --nocapture
```

The shader UV/clip/displacement contract is now sufficient for the bounded
[surface implementation plan](HEIGHTMAP_WATER_SURFACES.md). Before complete
appearance compatibility is claimed, compare fixed cameras at Feerrott2 pond tile 84's local Y=224 edge, tile
271's entirely above-water terrain, and the pond seam at world X=-768. Buried
Sea selector 2 provides a separate material-appearance check. DAT region records
and liquid movement remain separate investigations; neither rectangle extents
nor the unknown trailing float establish a liquid volume.
