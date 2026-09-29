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
OpenEQ layout. In particular, OpenEQ's object-placement extra-word condition
currently uses `header[0] & 2 != 0`; now that the header word is known to be a
version, that condition needs a separate native trace. Versions below 12 omit
water entirely in the original reader and are not claimed to work in OpenEQ.

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
does not clip individual cells against terrain heights. It does not establish
whether later vertex/pixel shaders change or discard parts of the surface, nor
does it by itself prove final shoreline appearance or underwater behavior.

## UV and material evidence, with remaining limits

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
not the native indexed CPU mapping. The complete effective UV formula still
requires decoding subsequent shader scaling, scrolling, and seam behavior.

Material builder `0x100aa630` selects material type `0x35` and supplies the
diffuse water texture, authored normal/environment maps, Fresnel bias/power,
reflection amount/color, two water colors, and UV scale. No terrain-height or
terrain-depth sampler was found in that examined setup. This limited observation
is not evidence of a global absence of later clipping.

The installed shader `RenderEffects/SPL/SModelWater.fxo` is 26,920 bytes. Its
metadata includes shader-model 1.1/1.4/2.0 techniques, world matrices, fog/light
parameters, UV scale, time/slide parameters and normal/environment samplers.
Its full shader bytecode has not been decoded for this checkpoint. No exact
shader clipping, displacement or shading claim follows from those strings.

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

Before compatible surface rendering is claimed, resolve shader UV/clip behavior
and compare fixed cameras at Feerrott2 pond tile 84's local Y=224 edge, tile
271's entirely above-water terrain, and the pond seam at world X=-768. Buried
Sea selector 2 provides a separate material-appearance check. DAT region records
and liquid movement remain separate investigations; neither rectangle extents
nor the unknown trailing float establish a liquid volume.
