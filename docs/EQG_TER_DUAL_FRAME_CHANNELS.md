# Ordinary and two-UV TER normal/tangent channels

2026-10-01, research only. The original two-UV uploader uses the same
normal quantization and prepacked tangent-frame convention as the ordinary
`Opaque_MaxCB1.fx` path. **UV0 constructs the tangent frame; UV1 does not.**
The CB and CB1_2UV vertex shaders decode the same frame and project the same
directional-light vector through it. CBSG uses that frame for additional
point/eye projections, with different pixel lighting.

This extends [ordinary vertex channels](EQG_TER_VERTEX_CHANNELS.md),
[two-UV material/shader routing](EQG_TER_SECONDARY_UV_SHADERS.md), and
[the Causeway malformed-UV witness](EQG_NONFINITE_TER_UPLOAD.md).
It changes no renderer or asset code. Native CPU instructions execute in
Unicorn; shader algebra is checked separately with a deliberately non-GPU
token interpreter. Neither is an original-client frame capture.

## Executed construction and upload

The original DLL is `EQGraphicsDX9.dll`, preferred base `0x10000000`,
SHA-256 `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.

The new probe supplies the established 44-byte expanded TER records and
preallocated object arrays. It executes the CPU-copy tail beginning at
`0x100209a7`, including the **original polygon-copy loop** and the actual
call at `0x10020b35` into the complete `0x10020030` tangent builder. It
checks the copied native polygon records: source `(a,b,c)` becomes `(a,c,b)`,
with the original low-16-bit flag/material fields in their native positions.
The builder leaves copied position and attribute arrays unchanged.

It then executes both upload loops against those same arrays:

| Channel | Ordinary layout 1 | Two-UV layout 3 |
| --- | ---: | ---: |
| Position | 0 | 0 |
| Packed normal | 12 | 12 |
| Lighting color | 16 | 16 |
| Packed UV0 | 20 | 20 |
| Packed UV1 | absent | 24 |
| Packed TANGENT0 | 24 | 28 |
| Packed BINORMAL0 | 28 | 32 |
| Vertex stride | 32 | 36 |

All common bytes match for every vertex in both original full payloads,
under all four construction/upload control states below. The normal paths
are `0x1008d24e..0x1008d2e8` and `0x1008d8ce..0x1008d968`. The latter
stores the first region tangent array at `0x1008d9c4` and the second at
`0x1008d9c9`. These correspond to object `+0xc0` → TANGENT0 and object
`+0xbc` → BINORMAL0, as in the ordinary uploader.

Allocation/free and the starting object/copy-stack state are controlled.
The raw version-2 expansion is supplied, rather than executing the complete
TER file reader. Archive `.lit` words are supplied for the color channel;
this is not a new test of embedded-ZON/LIT precedence. Region partitioning,
buffer-device allocation and drawing do not execute. Uploading every vertex
through both layouts is diagnostic outside each layout's own material family.

## Numeric and malformed-input checks

For each normal component, the two-UV instructions perform:

```text
scaled = (component * 0.5 + 0.5) * 255
byte = low8(truncate_to_signed_qword(scaled))
normal_word = (byte_x << 16) | (byte_y << 8) | byte_z
```

Arithmetic uses the caller's x87 precision/rounding state. Only the qword
integer store temporarily forces truncation; the caller's control word is
restored. There is no component clamp or source-normal normalization. This
is separate from the UV helper's SSE2 conversion route.

The new layout-3 probe matches both a fresh layout-1 execution and every
result in the frozen ordinary normal oracle: **5,026 input words × 12 x87
precision/rounding combinations**. The corpus includes signed zeros,
subnormals, values just around ±1, large finite values, NaNs and infinities.
Asymmetric color/tangent sentinel words are copied unchanged. For example,
normal `3f7fffff` quantizes to byte 255 at PC24 nearest but 254 at PC53/PC64
nearest. Masked-invalid NaN/infinity conversion has a zero low byte.

A further **56 cases** execute the copy tail, actual tangent-builder call,
and both upload loops: 14 controlled inputs under each of CW `007f`,
`027f`, `037f`, and `0e7f`. Representative PC64-nearest results are:

| Controlled input | Normal word | Tangent word | Binormal word |
| --- | --- | --- | --- |
| XY triangle, UV0 `(0,0),(1,0),(0,1)`, normal `(0,0,1)` | `007f7fff` | `00007f7f` | `007fff7f` |
| Same, nonunit normal `(.5,0,.5)` | `00bf7fbf` | `00067fa7` | `007fff7f` |
| All three normal components NaN | `00000000` | `00000000` | `00000000` |
| Degenerate UV0, finite normal | `007f7fff` | `00000000` | `00000000` |
| One UV0 component NaN, finite normal | `007f7fff` | `00000000` | `00000000` |
| UV0 vertex 0 uses finite words `638a681c e38a6804` | `007f7fff` | `007f7f7f` | `007f7f7f` |
| Only UV1 changed to NaN/+infinity | `007f7fff` | `00007f7f` | `007fff7f` |

The huge-finite UV row is a constructed triangle and differs from the
original Causeway triangle below. It must not be generalized to all large
UVs. Changing only UV1 to unrelated finite coordinates or NaN/+infinity
leaves the complete ordinary-layout output and both packed tangent arrays
byte-identical to the finite control, in every tested mode. Nonfinite UV1
still packs to zero in its own tested SSE2 upload channel.

The original builder operates on raw UV0 before SHORT2 upload, with
order-dependent direction seeds and its own orthogonalization/cross-product
sequence. A conventional tangent generator or a generator using quantized
UVs is not established as equivalent. Packed zero is also not a zero
direction: D3DCOLOR decode followed by `2*x-1` produces `(-1,-1,-1)`.
Packed `007f7f7f` decodes to `(-1/255,-1/255,-1/255)`.

NaN source cases are constructed expanded records; neither original payload
has nonfinite normals. Exact retained NaN payload/sign behavior is an
emulator observation, not a physical-x87 guarantee or a complete file-reader
claim. Nonfinite source positions are outside these cases.

## Original Nest and Causeway results

The full native construction preserves original polygon order across
materials, rather than constructing a separate frame for each draw group.

| Source | Vertices / polygons | SHA-256 |
| --- | --- | --- |
| `thenest.eqg/ter_abyss01.ter` | 384,042 / 320,324 | `238b0f41bd5133de9e9a5f04cc2502f090b120028bf7473b4f247340e3c2a176` |
| `causeway.eqg/ter_gorge.ter` | 111,269 / 104,761 | `ac5973164156e159f40a717590143ddc254a57806e1feba977b8212d8580b857` |

Every normal component in both payloads is finite and within [-1,1]. The
referenced source-vertex subsets are 261,870 for Nest CB1_2UV, 112,073 for
Nest CBSG1_2UV, and 69,352 for Causeway CB1. These are per-family sets;
vertices shared between families need not be counted once overall.

Relative to PC64 nearest (`037f`), the number of changed packed words is:

| Source subset | Construction/upload mode | Normal | Tangent | Binormal |
| --- | --- | ---: | ---: | ---: |
| Nest CB1_2UV | PC24 nearest (`007f`) | 1 | 38 | 80 |
| Nest CB1_2UV | PC53 toward-zero (`0e7f`) | 0 | 326 | 239 |
| Nest CBSG1_2UV | PC24 nearest | 0 | 6 | 37 |
| Nest CBSG1_2UV | PC53 toward-zero | 0 | 32 | 87 |
| Causeway CB1 | PC24 nearest | 728 | 409 | 814 |
| Causeway CB1 | PC53 toward-zero | 0 | 32,886 | 51,451 |

PC53 nearest (`027f`) equals the PC64-nearest results throughout both full
payloads. The full-payload Causeway counts and all shared retained witnesses
also exactly reproduce the earlier independent construction probe.

No Nest vertex has a zero packed tangent or binormal word in any tested
mode. Causeway has 1,832 such vertices overall, including **1,641 referenced
by CB1**; those counts are unchanged across the four modes. This counts
packed output, not visible faces or a classification of every degenerate
input. Original Causeway vertices 96,771–96,773, including the NaN/huge-finite
UV triangle, reproduce normal `00d3d3ac` with zero tangent and binormal.

## Shader frame and sign convention

Declarations 5 and 7 supply normal/tangent/binormal as normalized D3DCOLOR
channels. Decode each packed word into RGB/255, then apply `2*x-1`; call
the resulting vectors **N**, **T**, **B**. The examined vertex programs do
not normalize those three vectors individually.

For a world-space vector `v`, define the frame projection:

```text
Q(v) = (-dot(T,v), -dot(B,v), dot(N,v))
```

The minus signs on T/B are consequential. With the finite XY witness,
the packed tangent represents approximately negative X, not positive X.

| Program | Relevant original FXO offsets | Frame outputs |
| --- | --- | --- |
| CB1 PS1.4 vertex program `2884` | N `3124`; T/B `337c/3390`; projection/normalize `335c..3434` | `oT2 = 0.5*normalize(Q(-D))+0.5` |
| CB1_2UV PS1.4 vertex program `3818` | N `40d4`; T/B `432c/4340`; projection/normalize `430c..43d4` | `oT3 = 0.5*normalize(Q(-D))+0.5` |
| CBSG1_2UV PS2.0-technique vertex program `3be4` | N `44a0`; T/B `467c/4690`; projections `466c..47e8` | Outputs described below |

Here D is the bound world directional-light normal. Its incoming magnitude
is not changed by the CPU binder. CB1 and CB1_2UV normalize the assembled
tangent-space direction at the vertex, then encode it for interpolation.
Their pixel programs decode the interpolated value and sampled normal,
take a saturated dot product, and **do not normalize either decoded vector
again per pixel**. Their ordinary geometric-normal point/bounce terms use
the unnormalized decoded N as documented in the lighting note.

For CBSG1_2UV, with point-0 and eye displacement vectors `p` and `e`:

```text
P = normalize(Q(p))
E = normalize(Q(e))
oT2 = E + P                    // normalized later in the pixel shader
oT3 = P
oT5 = normalize(Q(-D))
```

Its point-0 attenuation still uses world-space squared distance/radius²;
normalizing the projected displacement does not change that range rule.
Point slots 1/2 contribute to vertex base lighting, while point 0 is a
separate pixel-light term. The existing two-UV shader note gives CBSG's
normal-texture R/G, glow-B and specular-A interpretation. Its equations
must not be substituted for CB merely because both use two color textures.

A small independent interpreter reads the three original vertex token
streams and compares their outputs to the equations above and the separate
base-light formulas. **124 original/synthetic packed fixtures × three
programs agree.** This interpreter uses Python float64, supplied frame/point
constants and explicit D3DCOLOR decoding; it is an algebra check, not native
shader execution. Legacy GPU precision, register-range clamping, interpolation
and texture sampling are intentionally outside that result.

## Reproduction and remaining boundary

The native helper accepts `--zone thenest|causeway`, `--cw` in hexadecimal,
and a distinct output path. Run the four states `007f`, `027f`, `037f`,
`0e7f` for each zone; summary and algebra probes read those named outputs.

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-dual-frame-native.py --zone thenest --cw 037f --output /tmp/openeq-ter-dual-frame-thenest-037f.json
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-dual-frame-boundaries.py
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-dual-frame-summary.py
python3 /tmp/openeq-ter-frame-shader-algebra.py
```

All inputs and derived upload buffers remain outside the repository. The
summary records the SHA-256 of each of the eight native result JSON files;
those results retain hashes of their complete uploaded arrays.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-dual-frame-native.py` | `504abbc938316717961fdb7932dad8bd0e7bc2c17cca7539872a39d643f2edec` |
| `/tmp/openeq-ter-dual-frame-boundaries.py` | `d62a17a739f586109cd80fb54395accf3461f37f1887c3c9becda496aedbb7eb` |
| `/tmp/openeq-ter-dual-frame-boundaries.json` | `74f8143042ee959bb4260228c05e5d477d6939ba3f1c6b35e474f5f477d35fd9` |
| `/tmp/openeq-ter-dual-frame-summary.py` | `919c60efc7a4198ba5b189bc791888abc6daa0d259c28cb640ee15593a0f86fe` |
| `/tmp/openeq-ter-dual-frame-summary.json` | `e7f01b4056971305598d19238ab2860b80874728778dea497a8812fa4e134b04` |
| `/tmp/openeq-ter-frame-shader-algebra.py` | `7ed56a70761c9de893b08de11585f5a855c4b5c2230a1639065137c9ac36fdad` |
| `/tmp/openeq-ter-frame-shader-algebra.json` | `b898e19ae9baea9a76587e834d8651f77e1fab76a920216c20859cdac50d37f0` |
| Required `/tmp/openeq-pfsinspect.py` | `64fedc715bae14a56b0c10970914a003eab6165d282b71052a935906fd1ee0cd` |
| Frozen ordinary normal oracle JSON | `3824b4c9072092022bc8a2990eab7f446da2e5a4a64ba43c1d09e5a030392112` |

The [loading-loop FPU investigation](EQG_TER_FPU_LIFECYCLE.md) establishes
an explicit PC53/toward-zero request but still does not prove preservation
through every intervening callback. These additional mode-dependent results
do not close that lifecycle gap or select a universal production mode.
They also do not establish live light membership, frame constants, SPL/MPL
choice, device-valid techniques or complete native lighting pixels.
