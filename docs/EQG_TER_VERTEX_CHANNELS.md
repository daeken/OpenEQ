# Native TER normal, tangent and lighting-color channels

2026-10-01, research only. This extends
[the executed TER upload](EQG_NONFINITE_TER_UPLOAD.md) and
[UV compatibility](EQG_TER_UV_PACKING.md) for ordinary TER versions 1–3 with
the exact `Opaque_MaxCB1.fx` family. No production code changes follow from
this note. The clearest next compatibility boundary is the separate per-vertex
lighting stream. Normal and tangent conversion also depend on an effective
floating-point mode that device-creation flags alone do not settle.

Native addresses use the installed `EQGraphicsDX9.dll` image base `0x10000000`,
SHA-256 `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The isolated probes execute original CPU-copy, tangent-builder and layout-1
upload instructions in Unicorn. They create no graphics device, original-client
process or character session. Shader findings are decoded original bytecode,
not a capture of a particular device's technique selection or output pixels.

## Executed layout and normal conversion

The established material route is type 8 → runtime type 9 → layout 1 → region
descriptor 6 / declaration selector 5. Table `0x10175168` contains:

| GPU byte offset | Declaration | SPL vertex input |
| ---: | --- | --- |
| 0 | FLOAT3 POSITION0 | v0 |
| 12 | D3DCOLOR NORMAL0 | v1 |
| 16 | D3DCOLOR COLOR0 | v2 |
| 20 | SHORT2 TEXCOORD0 | v3 |
| 24 | D3DCOLOR TANGENT0 | v4 |
| 28 | D3DCOLOR BINORMAL0 | v5 |

At `0x1008d24e..0x1008d2e8`, normal XYZ from attribute offsets +24/+28/+32
become the R/G/B bytes of a word `0x00RRGGBB`. Each component is multiplied
by 0.5, then has 0.5 added, then is multiplied by 255. The original sequence
forces truncation only for `FISTP qword`, restores the caller's control word,
and retains the integer's low byte. Unlike the UV conversion, this directly
uses a signed **64-bit x87** integer conversion and does not call the SSE2
conversion helper. Masked-invalid NaN, infinity or signed-64 overflow produces
integer indefinite, whose low byte is zero. Valid large integers wrap; there
is no saturating clamp and no input normalization.

A layout-1 probe ran 5,026 source words in all 12 combinations of x87 precision
24/53/64 and four rounding modes, with exceptions masked. It separately
verified unchanged color and prepacked tangent-word copies with asymmetric
sentinels. At PC24 round-nearest, explicit f32 rounding after each of the three
arithmetic operations reproduces every result; at PC64, exact rational
evaluation followed by the signed-64/truncated-byte rule reproduces every
result. These are explicit numeric targets, not a claim that one mode always
governs terrain loading.

| Repeated source component | PC24 nearest byte | PC53 nearest byte | PC64 nearest byte |
| --- | ---: | ---: | ---: |
| 0 or negative zero | 127 | 127 | 127 |
| 0.5 | 191 | 191 | 191 |
| -0.5 | 63 | 63 | 63 |
| `3f7fffff`, just below 1 | 255 | 254 | 254 |
| 1 | 255 | 255 | 255 |
| -1 | 0 | 0 | 0 |
| 2 | 126 | 126 | 126 |
| -2 | 129 | 129 | 129 |
| `4b000001`, 8,388,609 | 0 | 255 | 255 |
| `5b000000`, 2^55 | 0 | 0 | 127 |
| `db000000`, -2^55 | 0 | 0 | 128 |
| NaN, infinity, largest finite f32 | 0 | 0 | 0 |

The declaration supplies normalized RGBA to the shader. `2*v1.xyz - 1`
decodes the normal; the examined shader does not renormalize that geometric
normal before its point/bounce-light dot products. Thus packed zero means
(-1,-1,-1), while a packed component 127 decodes to -1/255, not exactly zero.
Replacing malformed normals with (0,0,1), or normalizing all decoded normals,
would be a separate behavior change.

An independent exact-family corpus walk covers 24 TERs and 278 referenced
material records. Its 8,065,287 referenced normal components are all finite
and within [-1,1]. Nonfinite/huge-normal witnesses here are synthetic; the
known original bad-normal MODs use other families. Under the expanded
quantization rule, 7,219,221 ordinary finite components change numerically.
The corpus counts retain each material's referenced source vertices, so a
shared source vertex can contribute to more than one material count.

## Device flags and later FPU setup

The actual Direct3D9 object from `Direct3DCreate9` is stored at renderer +0xf04
at `0x10099532`. At `0x10099990..0x100999ac`, creation chooses behavior flags
0x24, 0x84 or 0x44; `0x100999cd` ORs in 4. The call at `0x100999e1` uses that
same object's vtable slot +0x40, with presentation parameters at renderer
+0xea8 and the output device at +0xf08. An instruction probe confirms all
three argument combinations. Retries return through the same selector.
None includes `D3DCREATE_FPU_PRESERVE` (2).

[Microsoft's D3DCREATE documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dcreate)
says that omitting FPU_PRESERVE defaults to single precision, round-nearest.
However, this DLL also changes the mode explicitly later. Renderer routines
`0x10097de0` and `0x10092a20` call CRT helper `0x1010f787` at `0x10097e0f` and
`0x10092a4f` with value `0x9031f` and mask `0xb031f`. The helper's own bit
translation maps the precision field to **PC53** and rounding to **toward
zero**. Executing its complete x87-only branch from each of CW 0x007f, 0x027f
and 0x037f produces CW 0x0e3f and return value 0x9031f. This masks exceptions;
bit 6 differs from the otherwise equivalent probe CW 0x0e7f.

The renderer passes the helper's returned value to later calls at
`0x10097e6a`, `0x10098314`, and `0x10092a7b`. That returned value is the
updated state, so these calls do not prove restoration of a pre-entry mode.
The isolated full-helper witness disables its SSE feature branch; it proves
x87 conversion and return semantics, not the helper's MXCSR/TLS path.

The remaining lifecycle gap is the effective thread/control state during TER
copy/tangent construction and the terrain resource upload through
`0x10086490` → `0x100863b0` → `0x1008640b` → `0x1001f8b0`.
Neither observing CreateDevice's flags nor observing a rendering wrapper
establishes that entire ordering. No universal PC24 or PC64 normal/tangent
policy should be introduced from this note.

## Tangent-frame construction and packing

Original `0x10020030` creates two temporary float-vector arrays and two packed
word arrays. It reads source positions, **raw UV0**, source normals and source
polygon order. The TER-copy path reverses each source triangle's last two
indices before this routine. Packing UVs first and generating tangents from
the decoded SHORT2 values would therefore use the wrong inputs.

The face stage at `0x10020194..0x10020544` computes reciprocal UV-determinant
directions. Per-vertex direction storage is filled when its components are
within the small-zero tests using constants ±2^-23 (`0x10133c54/58`). It is
not a sum/average of every adjacent face. Two finite triangles sharing a vertex
produce different packed bases when their order is reversed. A degenerate-UV
first triangle leaves NaN direction seeds; a later valid triangle does not
repair them in the executed witness. Polygon flag 4 on the first triangle
also does not skip its contribution in this builder.

The second stage starts at `0x1002056e`. It uses the supplied normal in
projection/orthogonalization, normalizes temporary directions, reconstructs a
basis with cross products (`0x100206aa..0x10020787`) and adjusts orientation
using its dot test (`0x100207aa..0x100207df`). It does not normalize or rewrite
the source normal. These operations and their intermediate f32 stores are
material to parity; a conventional tangent-library replacement is not an
established equivalent.

At `0x100207e1..0x1002093e`, each resulting vector is packed with the same
0.5/0.5/255, truncated-qword, low-byte procedure. Object +0xc0 feeds GPU
TANGENT0 and object +0xbc feeds BINORMAL0 through the region copies. The
layout-1 uploader only copies these two prepacked words at
`0x1008d319..0x1008d325`; it performs no tangent arithmetic itself.

A synthetic XY triangle with UVs (0,0),(1,0),(0,1), normal (0,0,1), and native
winding produces geometric normal word `007f7fff`, tangent `00007f7f` and
binormal `007fff7f`. The prepacked float vectors are (-1,0,0) and (0,1,0).
Mirroring U changes them to (1,0,0) and (0,-1,0). Degenerate UVs or all-NaN
normals produce **zero packed tangent words**, which shader decoding turns
into (-1,-1,-1), not a zero vector. These are executed CPU results under
explicit controls, not proof of a malformed triangle's eventual visibility.

The complete original Causeway payload, 111,269 vertices / 104,761 polygons,
was also copied and passed through the original tangent builder. Every source
UV/normal word and both CPU color words survived that builder unchanged. Its
layout-1 diagnostic upload gives these differences relative to PC64 nearest:

| Entire CPU construction/upload mode | Normal words | Tangent words | Binormal words | Color words |
| --- | ---: | ---: | ---: | ---: |
| PC24 nearest | 855 | 663 | 1,039 | 0 |
| PC53 nearest | 0 | 0 | 0 | 0 |
| PC53 toward-zero | 0 | 50,091 | 69,325 | 0 |

Only supported materials actually select layout 1; applying it to the whole
payload is a diagnostic convenience. The bounded material-6 run contains
2,257 vertices / 879 faces. Its PC24 differences are 28/22/45 normal/tangent/
binormal words; its PC53 toward-zero differences are 0/95/951. Tangent seeds
in a material subset need not equal the full-payload seeds when vertices are
shared across materials; full-payload construction preserves that dependency.

## Lighting color is a separate indexed stream

The CPU attribute record distinguishes two color words:

- **+16:** per-vertex lighting word supplied as the second argument to
  `0x10020970`; absent data becomes literal `0x001f1f1f` at `0x10020a3c`.
- **+20:** the expanded TER vertex's word at +24, copied at
  `0x10020a46..0x10020a4a`. For v3 this is authored vertex color; legacy
  expansion supplies its older default.

The layout-1 uploader copies **+16**, unchanged, into GPU COLOR0 at
`0x1008d2eb..0x1008d2ee`. It does not read +20. An asymmetric synthetic
source-color `a1b2c3d4` plus lighting `11223344` confirms the distinction;
without lighting the output is `001f1f1f`, not the source color.

TER reader `0x100643b0` first checks global lighting pointer `0x1017c200`
and count `0x1017c204` (`0x100645b9..0x100645d6`). If absent, it constructs
the `.LIT` name using literal `0x1013b820`, accepts magic `EQGP`
(`0x1013b818`), reads a u32 count and that many packed words. The count is
compared to the terrain vertex count at `0x10064784..0x100647b4`; a mismatch
discards the supplied lighting pointer. The embedded-pointer branch jumps
directly to that comparison at `0x100645d6`: an embedded-count mismatch does
**not** retry `.lit`. Missing/unrecognized LIT also leaves the lighting pointer
null, giving the CPU-copy default. The final call at `0x100647ed` passes the
selected stream to the CPU-copy method. These native branches establish source
selection and count handling, not safe bounds handling for a truncated file;
a new parser must independently bound its reads and allocations.

There is a concrete source for that higher-priority global stream. Function
`0x100649f0`, called at `0x10066953`, recognizes loose `EQGZ` version 2.
Starting after its 28-byte header, string block and model-name-offset table,
it reads the terrain placement's count at +36 and sets the lighting pointer
to +40 (`0x10064a68..0x10064a76`). The global is cleared at `0x1006676f`
before zone loading. This path must be considered before assuming a missing
archive `.lit` means default lighting.

Both sources index colors by the **original TER vertex index**. The copy loop
uses `lighting[vertex_index]` at `0x10020a33`; later region remapping copies
all nine attribute dwords at `0x1001f02f`. A compatibility implementation must
preserve this association through material packing and vertex deduplication.
Using a packed/deduplicated vertex index to read the original stream is wrong.

Original Causeway `ter_gorge.lit` is exactly 8 + 4*111,269 bytes, SHA-256
`f9b94aabcf91fe4dfec1b88d2fc0b9e159b30cd238e827831009d47eb923e753`.
All 111,269 native diagnostic outputs equal their corresponding original
lighting words, across the four FPU modes above. Material 6 has 1,943 vertices
with alpha zero and 314 with nonzero alpha, reaching 229.

| Original Causeway vertex | LIT byte offset | COLOR0 word | Significance |
| ---: | --- | --- | --- |
| 87,375 | `0x55544` | `001b1202` | baked RGB, zero ambient/directional weight |
| 87,468 | `0x556b8` | `4c0d0901` | weight 76/255 plus nonzero baked RGB |
| 96,771 | `0x5e814` | `00000000` | original malformed-UV witness has no baked light |
| 103,109 | `0x64b1c` | `e5000000` | weight 229/255, zero baked RGB |

The three supported v3 original TERs use loose ZON-v2 embedded lighting:

| Zone | ZON stream offset | Colors = TER vertices | Selected unique vertices checked |
| --- | --- | ---: | ---: |
| guildhall | `0x0c4b` | 28,584 | 192 |
| guildlobby | `0x241c` | 57,912 | 8,870 |
| roost | `0xa536` | 73,847 | 184 |

All 9,246 selected vertices' uploaded colors exactly match those indexed
streams, and all differ from the TER-stored color. For example, Guild Hall
vertex 16,302 has TER word `ff232727` at `0xb08e2` but uploads embedded
`32a6a6a6` from ZON `0x10b03`. Guild Lobby vertex 14 uploads `00686456`
from ZON `0x2454` instead of TER `ff808080`; Roost vertex 72,880 uploads
`00020202` from ZON `0x517f6` instead of TER `ff808080`.

Their loose ZON SHA-256 values are respectively:

- `b789663e437b323aaef44a35568fe5c0683b431f35bc706407da92d963466d9e`
- `f1f47023efc0bff8e603974c7b67899fb68e5217e8fe4ae499b5070e88846f1b`
- `d7f68ff594d3abebfa03b8ebd99e359503567d2a95ca08739a53dd855491c0b6`

## Shader decoding, interpolation and light interaction

The original single-pass `SPL/RegionCB1.fxo` hash and descriptor binding are
recorded in the earlier upload note. Its PS1.4 vertex program at file offset
`0x2884` declares the table inputs above at `0x2f18..0x2f54`; the PS1.1
variant at `0x1b30` uses the same meanings. D3DCOLOR `0xAARRGGBB` becomes
RGBA divided by 255. No sRGB decoding of this vertex input is requested by
the declaration. Call the decoded lighting color C, geometric normal N,
tangent T, binormal B, and directional-light normal D.

- `0x30b4/0x30d8`: base RGB starts with `C.rgb + C.a*ambient + specialAmbient`.
- `0x3124`: N is `2*v1.xyz - 1`. Three point lights add vertex-computed
  `color * clamp(dot(N, directionToLight),0,1) * attenuation`. Attenuation is
  one minus the squared scaled distance, clamped at one. Its inverse-range
  scalar comes from the program's preshader constants; actual runtime light
  selection/range binding is outside this probe.
- `0x3268..0x32bc`: bounce adds `C.a*bounceColor*max(dot(N,D),0)`.
- `0x337c/0x3390`: T and B likewise decode as `2*input - 1`.
- `0x335c/0x33a4/0x33b4` form directional light in tangent space as
  `(dot(T,D), dot(B,D), dot(N,-D))`; `0x33d4/0x33f4/0x3414` normalize that
  assembled vector, and `0x3434` encodes it back into [0,1].
- `0x3400` emits the accumulated base/point/bounce RGB as oD0; `0x3424`
  emits `C.a*directionalColor` as oD1. `0x3448` explicitly sets oD0 alpha to
  **1**, so the original lighting alpha is a light weight, not surface opacity.

These oD0/oD1 colors and the encoded tangent-light coordinate are interpolated
by the rasterizer before the pixel program. At `0x2828`, that program dots
the decoded sampled normal with decoded interpolated tangent light, saturates
the dot, then computes `diffuse.rgb * (base.rgb + dot*directional.rgb)` at
`0x2838/0x2858`. It does not renormalize either vector per pixel. Exact legacy
color-output clamping, interpolation precision and sampling require device
validation; recomputing all three point lights per pixel would not reproduce
this program's vertex-light interpolation.

The no-bump technique also decodes the packed geometric normal, uses lighting
RGB/alpha in the same base/bounce/point terms, and computes directional light
from that geometric normal. It does not bypass the packed normal or substitute
the v3 TER color. Consequently lighting alpha zero suppresses ambient, bounce
and directional terms, but does not suppress supplied baked RGB, special
ambient or point-light terms.

Multipass is a distinct boundary. `MPL/Region_BaseB.fxo` starts its base with
half of C.rgb at `0x108c`, adds weighted ambient/bounce and special ambient,
then clamps base RGB to [0,0.5] at `0x10f8/0x1120`. Its pixel program uses a
normalization cube texture. `Region_LightB1.fxo` performs bump point-light
evaluation in the pixel stage. These program facts do not establish the full
multipass blend/fog equivalence; do not silently apply the SPL formula to all
renderer modes.

## Next implementation boundary and remaining evidence

Preserving the distinct original-index lighting word through source loading,
material grouping and GPU upload is a concrete candidate for the next scoped
change. It needs both matching `EQGP` LIT and ZON-v2 embedded sources, the
observed precedence/count checks, and the tested missing-lighting default.
The GPU channel must retain alpha's lighting role separately from material
opacity. Merely multiplying diffuse by TER v3 color would miss the actual
input and the ambient/directional weighting.

Normal packing, tangent generation and shader equations should remain separate
reviewable steps. Native effective FPU state at each load/upload stage, precise
tangent arithmetic under that state, runtime technique selection, actual light
constant binding and render-state behavior remain unclosed. This note does not
authorize deleting malformed faces, changing collision, normalizing source
data, substituting reconstructed normals, or declaring original lighting parity.

## Reproducible temporary evidence

Probes use `/tmp/openeq-ter-upload-venv`, the same PE/PFS helpers as the UV
investigation, and read installed assets in place. The complete payload probe
is run with `--whole`; its layout-1 upload is diagnostic for materials outside
the supported family. The following artifacts contain probe code, summaries,
raw-word witnesses and their original indices; no original assets or native
disassembly are committed.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-normal-range.py` | `d60faf0a4bda215cfb1987fd2af034495812052e5d8619881ecb8f2c45d5712d` |
| `/tmp/openeq-ter-normal-range.json` | `3824b4c9072092022bc8a2990eab7f446da2e5a4a64ba43c1d09e5a030392112` |
| `/tmp/openeq-ter-tangent-channels.py` | `7d35ca34965dfd35b0275703932fd4ba33fa10536cf72c9289344636845089dc` |
| `/tmp/openeq-ter-tangent-channels.json` | `e0453acf5503f6b88871d9fbd6bfef38931c9a9dba00e3cec0743a12269d12a1` |
| `/tmp/openeq-ter-tangent-seeds.py` | `051883e14fdda04e4390ead40ad7dffc3e34509496b4c65d6e2f95456b1c1d26` |
| `/tmp/openeq-ter-tangent-seeds.json` | `c3f2dd763c6c94a2935c808e25114d291875b38dbbb41457212a1cef3009e372` |
| `/tmp/openeq-ter-original-channels.py` | `58d8c6e0d973023a510547631daa5e084e4170960d0e52b4286e379ca246b293` |
| `/tmp/openeq-ter-original-channels.json` | `6963896449fadedaeae82b649d3a3f1c93f3d045258b2b4ad23f29a6051e5312` |
| `/tmp/openeq-ter-original-channels-whole.json` | `f196bd5c29f409142822f05ed7fe2bcd2da8eb4a8b4f548c05ca0d4b0455b1b2` |
| `/tmp/openeq-ter-embedded-colors.py` | `4f107a03618f480d1c7f7e025ddf6050c8b8efed64cdcd45d009c0a078c540c0` |
| `/tmp/openeq-ter-embedded-colors.json` | `0178bd83b9319113740597d90faf5be54518dae08f837681a4320253d04f9851` |
| `/tmp/openeq-ter-fpu-state.py` | `cb8588098b88e77312663898811aec9fe9a1cb28cd85f0a426919d5e93d3ef54` |
| `/tmp/openeq-ter-fpu-state.json` | `82546e37bcf882a02c3415ac9ad02d8adfdb6a0f9a2b2fa36ec161a3d5c5fe87` |
| `/tmp/openeq-ter-vertex-corpus.py` | `9100037189d4fc6203ce0af47185755802d840397e88d7cf45a7ea2460c97494` |
| `/tmp/openeq-ter-vertex-corpus.json` | `69da51de7f7e7e8bf2e83e20cbe45144b12b6ca6bfc2faaa03bbcf934191a341` |

## Source-retention implementation checkpoint

The asset parser now retains the raw channels without applying native lighting
or modifying geometry: `TerMod.vertex_colors` and `secondary_tex_coords` hold
version-3 source data in original vertex order; `Placeable.vertex_lighting`
holds version-2 placement lighting. `None` distinguishes older versions from
an authored empty stream. `VertexLighting::parse` reads an independent EQGP
stream, preserves its packed words and opaque suffix, and verifies the complete
declared byte range before allocating the color vector.

Four regressions cover every truncated prefix, invalid counts/magic, nonfinite
secondary UV words, signed zero, independent source/lighting colors, alignment
across consecutive placements, and original streams. Causeway's four distinct
lighting witnesses and the complete Guild Hall/Guild Lobby/Roost embedded
streams agree with independent raw offsets. An independent review found no
issue. Focused evidence: `/tmp/openeq-vertex-lighting-focused.log`.

These channels are not yet selected by the scene loader or uploaded to a GPU.
Original-index remapping, embedded-versus-LIT precedence, mismatch fallback,
shader lighting and native color-space behavior remain separate integration
work. Keeping this boundary explicit avoids treating TER color as opacity or
claiming a visible lighting repair from parser preservation alone.

Integrated source verification passes all 319 assets tests, zero failures or
ignored tests, and strict assets all-target Clippy. Logs:
`/tmp/openeq-lighting-source-assets-final.log` and
`/tmp/openeq-source-metadata-clippy.log`. That run includes the separate WLD
texture-chain metadata change; it is not a full workspace/GPU checkpoint.
