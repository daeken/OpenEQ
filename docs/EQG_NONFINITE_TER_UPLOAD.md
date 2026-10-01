# Native TER attribute packing: Causeway

2026-10-01, research only. This follows
[the authored nonfinite-attribute survey](EQG_NONFINITE_ATTRIBUTES.md) through
the ordinary TER path for one original material. The installed renderer does
not submit that material's floating-point UVs directly: it packs them into
signed `SHORT2`, and the selected shader family divides those integers by 256.
An isolated instruction probe reproduces zero packed UVs for both an authored
NaN and its extremely large, **finite** neighbor. This is evidence for a render
conversion boundary, not permission to sanitize the source parser or delete
faces. No production code changed for this investigation.

## Original fixture

`causeway.eqg/ter_gorge.ter` is version 2, with 111,269 vertices, 104,761
polygons, and 17 materials. Its uncompressed SHA-256 is
`ac5973164156e159f40a717590143ddc254a57806e1feba977b8212d8580b857`.
Vertices begin at `0x17a7`, with stride 32; polygons begin at `0x366c47`.
The four trailing bytes are zero, so the legacy optional secondary-UV array
is absent.

The independently walked **material ordinal 6** also has stored ID 6. Its name
is `Material #6894`, its shader is `Opaque_MaxCB1.fx`, and its complete property
list is tag-2 diffuse `sp2_c.dds` and tag-2 normal `sp2_n.dds`. There is no channel
selection or alternate UV property. Its 879 faces reference 2,257 vertices.

All source positions and normals are finite. Ten vertices contain NaNs in
UV0, affecting ten faces, all with polygon flags zero and nonzero area in f64.
The first is source polygon 99,575, indices `(96771, 96772, 96773)`. Its squared
cross-product length is approximately `1.3416590932041617e-5`.

| Source vertex | UV word offsets | Raw UV words | Interpretation |
| --- | --- | --- | --- |
| 96,771 | `0x2f581f`, `0x2f5823` | `80000000 ffffffff` | negative zero, NaN |
| 96,772 | `0x2f583f`, `0x2f5843` | `638a681c e38a6804` | approximately `5.1063051e21`, `-5.1062916e21` |
| 96,773 | `0x2f585f`, `0x2f5863` | `00000000 80000000` | zero, negative zero |
| 97,517 | `0x2fb55f`, `0x2fb563` | `ffffffff 7fffffff` | two NaNs |

The oversized finite pair is consequential: a policy that only replaces
nonfinite inputs cannot reproduce the original upload for this triangle.

## Material, CPU copy, and upload route

Native addresses below use preferred image base `0x10000000` in the installed
`EQGraphicsDX9.dll`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
These are static call/data-flow findings, supplemented by the isolated probe
below; they are not a live D3D frame capture.

1. The material constructor `0x10012e70` clears both bytes at material `+8/+9`
   (`0x10012e98`). Parsing `Opaque_MaxCB1.fx` selects type 8 at `0x10014da0`.
   The final mapping at `0x10015b03..0x10015b13` writes runtime type 9 at `+4`
   and sets only the bump flag at `+8`. Thus this material uses the one-UV
   bump layout, not the separate two-UV branch.
2. TER reader `0x100643b0` expands version-2 records from 32 to 44 bytes.
   The primary pair at source `+24/+28` is copied to expanded `+28/+32`
   at `0x1006455b..0x10064573`; absent UV1 is initialized to zero. Position
   and normal components are copied through x87 float loads/stores.
3. `0x1001a600` constructs the terrain object through `0x1001f180`, whose
   vtable is `0x101358ac`. Its `+0x24` slot is the identity method
   `0x1001f890`, closing the previously unresolved virtual call in the TER
   reader. Reader call `0x100647ed` reaches the `+0x28` slot `0x10020970`;
   reader call `0x10064815` reaches `+0x18`, `0x10020b50`.
4. `0x10020970` separates positions into a 12-byte array and other attributes
   into 36-byte records. Normal XYZ go to `+24/+28/+32`
   (`0x100209e9..0x10020a01`), UV0 to `+0/+4`
   (`0x10020a07..0x10020a14`), and UV1 to `+8/+12`. It calls the tangent
   builder `0x10020030` at `0x10020b35`. That builder reads UV0 and normals
   but writes its tangent results to separate arrays.
5. Region partitioning in `0x10020b50` uses `0x1001f9b0` to remap referenced
   vertices. Its copy helper `0x1001efe0` copies positions and all nine
   attribute dwords (`rep movsd` at `0x1001f02f`), without UV arithmetic.
   `0x1001fea0` points each region at these copied arrays: region data `+0xc`
   is the 36-byte attributes and `+0x10` is the 12-byte positions
   (`0x1001ff16..0x1001ff65`). Tangent arrays occupy `+0x14/+0x18`.
6. Renderer call `0x1008640b` reaches terrain uploader `0x1001f8b0`, which
   visits the regions and calls `0x10019560`. For this material,
   `0x100196ed..0x1001970c` selects upload layout **1** from the bump/UV flags,
   and passes it, runtime type 9, the material, and the region arrays to
   `0x1008cf00` at `0x100197a0`.

The layout-1 loop writes this 32-byte GPU record:

| Offset | Native write | Declaration selector 5 |
| ---: | --- | --- |
| 0 | position XYZ, unchanged float components | `FLOAT3 POSITION0` |
| 12 | packed normal components | `D3DCOLOR NORMAL0` |
| 16 | CPU lighting color at attribute `+16` | `D3DCOLOR COLOR0` |
| 20 | primary UV, two low integer words | `SHORT2 TEXCOORD0` |
| 24 | one prepacked tangent-frame word | `D3DCOLOR TANGENT0` |
| 28 | the other prepacked tangent-frame word | `D3DCOLOR BINORMAL0` |

The UV instructions are `0x1008d2f1..0x1008d315`: load each float, multiply
by **256**, call `0x1010f280`, and store AX. The constant at `0x101357b4` is
exactly 256. For representable ordinary values this truncates toward zero,
keeps the low 16 bits, and lets the signed declaration interpret those bits.
It is not a saturating clamp and not a direct floating-point passthrough.

Normal packing at `0x1008d24e..0x1008d2e8` computes
`(component * 0.5 + 0.5) * 255`, forces x87 truncation for the integer store,
and keeps the low byte. The constants are at `0x10133c5c` and `0x10134ef4`.
This fixture does not establish how an authored nonfinite normal reaches this
path: it contains no such source normals.

## Actual declaration and shader-channel selection

The geometry keeps layout 1 and runtime type 9 at `+4/+8`
(`0x1008d4b5..0x1008d4ca`). The region switch in `0x1008f960`, table entry
`0x1008fe70 + 9*4`, targets `0x1008fbdf`, selecting render descriptor **6**.
The descriptor initialization record is:

| Role | Effect index | Original effect |
| --- | ---: | --- |
| single-pass | 6 | `SPL/RegionCB1.fxo` |
| multipass base | 94 | `MPL/Region_BaseB.fxo` |
| multipass light | 99 | `MPL/Region_LightB1.fxo` |
| multipass diffuse | 104 | `MPL/Region_TextureD1.fxo` |
| vertex declaration | 5 | table at `0x10175168` |

This mapping comes from the descriptor initialization data and constructor
`0x100a01a0`, not a resemblance between effect filenames. The effect filename
table is at `0x10175548`. Declaration initialization `0x100825a0` puts the
table at `0x10175168` into selector 5 and creates the D3D vertex declaration
through device slot `+0x158`. The six elements above and the END marker decode
directly from that table. Binder `0x10089240` selects the declaration at
renderer `+0x1d5c + 4*selector` and passes it to the cached setter.

The single-pass draw path reads the descriptor's effect at `+0xc`
(`0x1008c82c`), begins that effect (`0x1008ca55`), and invokes the declaration
binder (`0x1008ca83`). Effect loader `0x10087010` constructs `0x1000be30` and
calls its initialization slot `+4`, `0x1000bd10`. This is a real draw route;
it still does **not** prove which renderer configuration was active on a
particular original-client machine.

`RegionCB1.fxo` parses to exact EOF `0x34d8`. All three techniques use
`TEXCOORD0`, with the same signed-integer `/256` decoding:

| Technique | Vertex program offset | Primary-UV instruction(s) |
| --- | --- | --- |
| `RegionCB1_DX8_VS1_PS14` | `0x2884` | `0x3454` scales v3; `0x3464/0x3470` feed diffuse and normal sampling |
| `RegionCB1_DX8_VS1_PS11` | `0x1b30` | `0x2700` scales v3; `0x2710/0x271c` feed the texture coordinates |
| `RegionCB1_DX6_VS1_PS0_NoBump` | `0xe50` | `0x1998` writes `oT0.xy = v3 / 256` |

The last technique uses a vertex shader with fixed-function pixel processing;
it is not a raw-float-UV fallback. The alternate MPL programs also decode
`TEXCOORD0 / 256`: base at `0xf14`, light at `0xc54`, and diffuse at `0x6bc`.
Their shader input register numbers differ, but their semantic is the same.

Technique initialization `0x1000c0c0` enumerates device-valid techniques through
`ID3DXEffect::FindNextValidTechnique` (slot `+0xf4`, call `0x1000c134`) and gets
their descriptions through slot `+0x14`. `0x1000b970` then filters by renderer
settings and name substrings including `PS20`, `PS14`, and `PS11`.
`0x1000b810` calls `SetTechnique` through slot `+0xe8`. Without a live device
and its settings, the chosen technique remains conditional. The UV channel
conclusion does not depend on selecting one of these three variants.

## Isolated instruction probe

A temporary Unicorn x86-32 harness maps the original DLL sections, supplies
original Causeway records, and executes:

- the original CPU-copy loop `0x100209a7..0x10020ac7`;
- the complete tangent builder `0x10020030`, with only allocation/free calls
  replaced by isolated arena allocation;
- the original layout-1 upload loop `0x1008cffd..0x1008d42c`, without a D3D
  device, upload allocator, or draw call.

It ran both the bounded material subset and all 111,269 source vertices and
104,761 source faces. The whole-payload run retains the original polygon
material/flag words and native winding order. The CPU-copy input uses the
statically established version-2 expansion, with zero secondary UVs; region
partitioning itself is not emulated. The whole payload is supplied to the
same layout-1 loop as a diagnostic convenience; this does not assert that
every Causeway material selects that layout.

All copied CPU UV/normal words matched their source bits, including NaN
payloads. Running the tangent builder changed none of those CPU words.
The probe explicitly used x87 control word `0x037f` and MXCSR `0x1f80`, then
selected both branches of `0x1010f280` via its CPU-feature flag at
`0x101d6d0c`. The two runs produced identical packed records for the full
111,269-vertex diagnostic input.

| Vertex | Packed UV dword | Shader UV | Packed normal | Packed tangent / binormal |
| ---: | --- | --- | --- | --- |
| 0, finite control | `fffc0001` | `(1/256, -4/256)` | `007f7ffe` | `007fff7f / 00fe7f80` |
| 87,375, finite control | `000001ff` | `(511/256, 0)` | `00b50b7f` | `00124ca9 / 00a591f7` |
| 96,771 | `00000000` | `(0, 0)` | `00d3d3ac` | `00000000 / 00000000` |
| 96,772 | `00000000` | `(0, 0)` | `00d3d3ac` | `00000000 / 00000000` |
| 96,773 | `00000000` | `(0, 0)` | `00d3d3ac` | `00000000 / 00000000` |
| 97,517 | `00000000` | `(0, 0)` | `00d8d893` | `00000000 / 00000000` |

The zero tangent words are **packed colors**, not a claim that the shader
receives a zero direction. The programmable shaders unpack these inputs from
`[0,1]` to `[-1,1]`. Nor does this probe prove the affected thin faces are
visible from a particular camera.

The exceptional UV result agrees with the instruction-level analysis:
masked-invalid `cvttsd2si` (`0x1010f295`) produces `0x80000000`; its low word
is zero. The x87 branch uses `fistp qword` (`0x1010f2c5`) and its indefinite
zero-low-word return path (`0x1010f315..0x1010f325`). This fixture's huge finite
values exceed both conversion ranges. Values in other ranges can differ
between the helper's 32-bit SSE and 64-bit x87 conversion paths; do not infer
one universal numeric rule beyond the traced instructions.

This is isolated instruction execution with a specified floating-point
environment, not original-client hardware validation. The original process's
FPU/MXCSR state, device capabilities, and renderer settings remain unobserved.
No character was connected and no audio was played.

## Remaining compatibility boundary

The source parsing and primary-channel selection are now concrete for this
ordinary TER material. A future compatibility implementation should preserve
raw CPU/source data and reproduce proven conversion at the rendering boundary,
including finite quantization and overflow behavior. Replacing NaNs alone,
substituting UV1, or deleting these finite-position triangles is unsupported.
Tangent generation/packing and shader normal decoding matter alongside UVs.

This case does not close the MOD path, two-UV materials, nonfinite source
normals or positions, every shader family, or arbitrary floating-point state.
The structural survey's nonfinite counts should remain unchanged.

Effect SHA-256 values, in descriptor-role order above:

- `RegionCB1.fxo`: `ee5d7aec289ca8917ed900bacf7a28bd4341c0c9991b38270aadd8ef6e1bb87a`
- `Region_BaseB.fxo`: `c590001ec14ad0341d327cc643749e3d1d7714a7ba0e89811f881b2097781c70`
- `Region_LightB1.fxo`: `2d301ed2260b11d75f984ec146e974d574ecc2ad59861c9979e81b013579336c`
- `Region_TextureD1.fxo`: `ed2610e2cfa8c67187fad66c5982d7b5a9226f93303c52383dc25af6cb541c33`

Local probes: `/tmp/openeq-ter-upload-probe.py`, its material and `-whole`
JSON/log outputs, `/tmp/openeq-addalpha-regioncb1-shaders.txt`, and
`/tmp/openeq-nonfinite-region{baseb,lightb1}-{effect,shader}.txt`. The isolated
Python environment is `/tmp/openeq-ter-upload-venv`. No original binary data
or derived disassembly is included in this repository.
