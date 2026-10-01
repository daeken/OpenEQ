# Authored EQG waterfall rendering

The exact TER `Opaque_MaxWaterFall.fx` family now uses independently scrolling
color and opacity from the same diffuse texture. Previously these surfaces
entered the ordinary opaque pass, with a stationary texture and incorrect
opacity. The new pass uses source-alpha blending, filtered alpha cutoff at
16/255, and opaque depth testing without depth writes. Existing geometry and
physical collision are unchanged.

This is a bounded material improvement, not complete original lighting parity.
Native baked-light contributions, packed normals, three-slot point-light
membership, original fog transfer, inherited sRGB state and original scene
sorting remain separate. The new pass uses OpenEQ's current geometric-normal
lighting, fog and linear/sRGB color handling. It draws after the existing
weighted transparency pass and before additive surfaces, in existing scene
batch order; this is not a reconstruction of native visibility/sort order.
The original client framebuffer has not been compared.

## Admission and source routing

Admission requires TER version 1–3, the exact shader name, an authored nonempty
`e_TextureDiffuse0` string other than `none`, and all four finite float slide
properties. Missing properties do not receive guessed defaults: the native
shared effect can retain previous material values. MOD, near-name families,
incomplete state and nonfinite rates retain their previous handling.

Primary source UVs remain unchanged in baked geometry. The renderer performs
the established masked-SSE2 signed SHORT2 conversion and divides by 256, as
proven for this non-bump upload layout. Both scrolling coordinates originate
from this primary pair. The TER secondary channel is preserved by its parser
but is not used by the waterfall shader. CPU attribute deduplication remains
safe because this shader does not consume UV1.

The ordinary-bump native-light metadata gate stays confined to its existing
`Opaque_MaxCB1.fx` family. Enabling the same UV encoding for waterfalls does not
silently enable another family's lighting source/packing policy.

See [material binding](EQG_MATERIAL_PARAMETER_BINDING.md),
[waterfall upload/state](EQG_WATERFALL_UPLOAD.md), and
[compiled shader evidence](EQG_LAVA_WATERFALL.md).

## Timing and render state

Every admitted material retains the four authored rates. Each frame computes
its four offsets on the CPU from the renderer's shared elapsed clock:

1. Truncate to milliseconds, retain low unsigned 32 bits, reduce modulo 100,000.
2. Convert to float32 and multiply by original float32 `0.001`.
3. Expand that time to float64; evaluate `(time * 0.01).fract() * 100`.
4. Multiply each expanded authored rate in float64, then convert its offset
   to float32 for the GPU.

The effect time is nonnegative, so the positive fractional branch suffices.
Native D3DX uses qword intermediate storage. Replacing this with a single
float32 `time * rate` changes real outputs; cached millisecond 237 with rates
0.75, 3 and -1.5 is a saved regression. Renderer startup remains OpenEQ's epoch,
not a claim to reproduce the native process timer's origin. See
[executed preshader precision](EQG_WATERFALL_PRESHADER_PRECISION.md).

The storage record grows from 80 to 96 bytes, with matching CPU and shader
layouts. Sixteen bytes per waterfall material are updated before rendering;
vertex buffers, textures and pipelines are not rebuilt. Zone and actor scene
updates share the same elapsed clock. GPU profiling accounts for the new pass
separately, including CLI, interactive and restored-zone audit reports.

Texture samples use the existing repeating, filtered atlas. RGB comes from
the color coordinate; alpha comes only from the independently scrolling opacity
coordinate. Alpha below 16/255 discards the fragment. The pass uses SRCALPHA /
INVSRCALPHA RGB blending, read-only LESSEQUAL opaque depth and backface culling.
It preserves compositor alpha as an explicit OpenEQ policy. It never enters
the opaque G-buffer, shadow depth or weighted transparency passes, even where
the sampled opacity is one.

## Original corpus and verification

An independent raw archive/TER walk covers all 228 installed TER payloads.
It finds 138 first-name material definitions in 74 archives with this exact
shader, 117,568 source polygon references and 113,000 distinct referenced vertices
summed per material. All definitions contain all four finite slide rates.
Four omit a diffuse property and have zero polygon references; the remaining
134 meet the source admission shape. All referenced primary UVs are finite
and avoid the native SSE2/x87 integer-overflow disagreement domain. These are
source counts, not a visibility certificate or an assertion that every
polygon passes the renderer's independent drawable-flag policy.

Corpus evidence:
`/tmp/openeq-waterfall-corpus.py` and `/tmp/openeq-waterfall-corpus.json`.
The script SHA-256 is
`9393765f0efb9435db78fb5eaa30f8e07987c64504a8b6ea0dbf9a1ddf648f13`.
The JSON SHA-256 is
`f263478b03c37e79e5dac5d3b131ab69f10fee918ff928135490650d76210718`.
It records per-file hashes, property words, polygon flags and UV digests without
including original assets. Four missing-diffuse definitions are unreferenced:
Ethernere `river_flow`, Plhbixieint/Plhbixieintb `waterfall`, Westkorlacha
`Material #6`.

Focused GPU checks cover source-alpha weighting, cutoff boundaries, opaque
occlusion, no waterfall depth writes, independent color/alpha movement,
100-second period and unsigned clock wrap, current fog behavior and profiler
accounting. Original Nest checks reconcile 916 triangles and exact authored
rates, compare samples against an independent wrapped source-texture reference,
and render its actual waterfall geometry at 0, 2 and 100 seconds. The first and
last images match exactly and more than 1,000 pixels change between 0 and 2.
Captures are under `/tmp/openeq-waterfall-gpu/`; they isolate waterfall geometry
and are not full native-client screenshots.

Independent review found no blockers. It compiled the actual Rust offset
function separately and matched all 390 frozen native corpus batches at base
and wrapped clocks, plus the complete 400,000-word Nest output digest
`8208b73dd2d5877d67cdfb3518074fe93f85cefbcc5d71b98f5d23ce6b911db7`.
Root also replayed the original property, upload, draw-state and preshader
witnesses and reproduced their frozen hashes. Complete workspace results are
recorded in `WORLD_AUDIO_PARITY.md` at integration.
