# Bounded native MaxLava surface rendering

October 1, 2026. The renderer now admits complete static `Opaque_MaxLava.fx`
TER materials. The two diffuse layers scroll independently, and the top
layer's alpha mixes its lit RGB with twice the bottom RGB. This is an opaque,
depth-writing surface. Current OpenEQ geometric-normal lighting, shadows,
fog, texture resizing/mips and linear/sRGB policy remain explicit modern
choices. Native bump lighting, baked vertex lighting, original point-light
membership, device technique selection and native GPU pixel parity are **not**
claimed.

This closes the material/upload/timing prerequisites left open in
[EQG_LAVA_WATERFALL.md](EQG_LAVA_WATERFALL.md). The waterfall implementation is
separately documented in [EQG_WATERFALL_RENDERING.md](EQG_WATERFALL_RENDERING.md).

## Connected original material and upload execution

The original `EQGraphicsDX9.dll` SHA-256 is
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The upload witness executes complete material reader `0x10014450`, original
region selection/argument construction beginning `0x1001960c`, uploader
`0x1008cf00`, descriptor switch `0x1008fb7c..0x1008fd02`, complete descriptor
initialization `0x100826d0`, declaration creation `0x100825a0`, declaration
binding `0x10089240` and generic material-property binder `0x1008a110`.

Eight cases include the exact Nest `lavaflow` material record, synthetic
MaxLava2 with its fourth texture, and six asymmetric `mapChannel` controls.
Texture identities survive through controlled loader/effect callbacks; each
original slide word reaches its effect float parameter unchanged. Whole native
material ingestion and the draw caller's ordering are additionally established
in [EQG_MATERIAL_PARAMETER_BINDING.md](EQG_MATERIAL_PARAMETER_BINDING.md).
Missing effect parameters retain prior shared-effect state, so no defaults are
invented for incomplete materials.

Both lava families choose **upload layout 1 and declaration selector 5**.
These are different selectors. The bump byte is one and extra-color byte zero;
the native branch at `0x100196ed..0x1001970c` selects ordinary upload layout 1.
Its 32-byte output is:

| Offset | Declaration |
| ---: | --- |
| 0 | float3 position |
| 12 | D3DCOLOR normal |
| 16 | D3DCOLOR color0 |
| 20 | SHORT2 texcoord0 |
| 24 | D3DCOLOR tangent |
| 28 | D3DCOLOR binormal |

The UV instructions at `0x1008d2f1..0x1008d315` read the primary attribute
pair, multiply by 256, invoke `0x1010f280` and retain each low word. Original
whole-uploader execution checks three asymmetric synthetic vertices, colors,
tangents and binormals, including signed wrapping and fractional truncation.
Changing only secondary source coordinates leaves every uploaded byte identical.
Map-channel integer properties are not float/texture effect bindings and do not
change this upload route.

The renderer therefore reuses the existing masked-SSE2 SHORT2 conversion target
from [EQG_TER_UV_PACKING.md](EQG_TER_UV_PACKING.md), including its separately
executed exceptional-value evidence: NaN/infinity/i32 overflow yield integer
indefinite with low word zero; valid coordinates truncate and wrap. These are
shared helper semantics, not new exceptional-value corpus cases. The installed
lava corpus contains no nonfinite or scaled-i32-overflow referenced UVs. Raw
baked vertices and collision data remain unchanged; conversion happens only
after the renderer admits the complete lava recipe.

The witness controls process allocation, CRT string interfaces, archive
sidecar absence, texture handles, vertex/index allocation, and effect/device
interfaces. It executes original instructions with synthetic asymmetric vertex
inputs, not native texture decoding or native GPU rendering. It does not claim
a whole running original client.

## Compiled surface expression and timing

`SPL/RegionLava.fxo` SHA-256 is
`c1dec3fe94eed577ae318a87d2954e6f920d152ee9ed55a76b6c3bc063eebace`.
The PS1.4 pixel program starts at `0x3360`. It samples Normal0 at both scrolling
UVs and samples Diffuse0/Diffuse1 independently. At `0x34ec..0x3520`, signed
normals and interpolated point-light direction produce a saturated dot term,
which combines with point-light color and interpolated base light before
multiplying top RGB. `lrp` at `0x3530` is:

```
rgb = top.a * lit_top + (1 - top.a) * (2 * bottom.rgb)
alpha = 1
```

The PS1.1 technique has the same final layer expression. The one-layer no-bump
fallback differs; no original device technique selection is claimed. The
PS1.4 vertex program begins `0x356c`, reads declaration input v3 and scales by
`1/256`; `0x4490..0x44b8` supplies slide1 to normal/top and
`0x44d0..0x44f8` supplies slide2 to normal/bottom. UV1 is not consumed.

`SPL/RegionLava2.fxo` SHA-256 is
`a6893a7176b122f17b2ca52c331fed7e4f1e552fcd3441a992dc873845e5301f`.
It is deliberately excluded. Its PS2 program at `0x2fa4` has stationary
Diffuse0/Normal0, scrolling Diffuse1 and independently scrolling Normal1;
normal, glow and specular terms differ. Sharing a declaration or slide names
does not establish the same color expression. No exact MaxLava2 TER definitions
occur in the installed corpus.

The original D3DX30 parser (`0x4eac2d`), runtime caller (`0x5053ee`), interpreter
(`0x4ea21f`) and qword-to-f32 output conversion (`0x50557e..0x505583`) execute
five original scrolling PRES programs: RegionLava `0x1504`, `0x2758`, `0x3924`
and RegionLava2 `0x23ec`, `0x3840`. D3DX DLL SHA-256 is
`5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532`.
The witness supplies controlled exact f32-expanded input banks, ID3DXBuffer
interfaces and mathematical CRT floor. It does not construct a native effect
object or execute a GPU.

All **2,880 samples** (36 authored four-rate sets, 16 positive/negative and
beyond-period times, five programs) match signed remainder/phase arithmetic
with qword intermediates, followed by one final f32 conversion. The actual
frame-clock domain additionally matches for **all 100,000 millisecond
remainders** with the Nest's exact `0.3,0,0.2,0` f32 rate words. Its output digest
is `f5bec79ff67b3cd9c55e9dfdf2ec96c4d7d7cd6217d59f046f1011403037f6f4`.
Those Nest rates alone produce no difference from the single-multiply shortcut
in that sweep; the requirement to retain the complete arithmetic is established
by the other authored rates in
[EQG_WATERFALL_PRESHADER_PRECISION.md](EQG_WATERFALL_PRESHADER_PRECISION.md).
The existing shared CPU offset helper is reused, including unsigned millisecond
clock wrapping established in [EQG_EFFECT_CLOCK.md](EQG_EFFECT_CLOCK.md).

## Installed source inventory and admission

The independent reader walks 228 TER payloads and first exact-name material
resolution. It finds 77 exact MaxLava definitions across 46 archives, referencing
78,754 triangles and 66,129 distinct vertices summed per material. Seventy
records are TER3 and seven TER2; all have four finite explicit slide properties,
with 36 distinct rate tuples. No `mapChannel` properties occur in this corpus.

All 76 referenced definitions have explicit Diffuse0/Diffuse1/Normal0 names.
The remaining definition, `takishruinsa.eqg` material `lava_flow`, has zero
triangles and no Diffuse1. Of 230 texture references, 228 resolve inside their
archive. Ethernere's `lava` material references absent `moltensteel.dds` and
`adl_moltensteel_n.dds`; its 2,782 triangles retain the ordinary fallback when
these bindings remain unresolved in the loaded scene. No
referenced texture has an archived or loose `.txt` animation sidecar in the
client root, Resources or Resources/waterswap directories examined. This is a
source inventory, not a claim that every material in every archive was rendered.

The asset loader retains a separate `Scene::ter_lava` material-indexed recipe
only for exact `Opaque_MaxLava.fx`, TER1–3, explicit nonempty/non-`none` three
texture names, four correctly typed finite floats, and no detected texture
animation sidecar in any candidate archive or its known loose texture roots.
This is conservative: a sidecar in a fallback archive rejects the recipe even
when a nearer static texture would win. A two-archive regression checks top,
bottom and normal sidecars while proving fallback texture provenance.
It registers the second diffuse independently, never as a
flipbook frame. The renderer revalidates primary/normal bindings, static frame
state, nonconflicting material modes and resolvable texture data before opting
in. The common decode-error sentinel is rejected. Missing textures, incomplete
or rebound recipes, MaxLava2, MOD and other shader spellings keep the existing
ordinary material and UV behavior. Sidecar indices must be cleared/remapped
when materials are replaced; object extraction starts with an empty recipe map.

## Renderer boundary and verification

Root independently replayed both connected upload and native preshader probes;
their complete result hashes match the frozen artifacts below. Two full-Nest
basin cameras at 960x540 were captured at 0, 1000, 2300 and 100000 ms before
and after this change. The new surface changes 24,799 and 62,351 pixels,
respectively, and both 100-second captures close exactly to time zero. Both
views were visually inspected. Clearing only the new lava recipe map produces
pixel-identical captures to the frozen pre-change renderer at all eight
camera/time pairs, demonstrating exact ordinary fallback restoration in these
scenes. This is an OpenEQ before/after comparison, not a native image golden.
Local sources/captures and comparison log are `/tmp/openeq-lava-scene-*`.

Admitted lava skips the G-buffer, participates in the existing opaque shadow
pass without alpha cutout, and renders after deferred lighting but before all
transparency, waterfall, additive and particle passes. Its forward pass loads
existing target/depth, uses backface culling and `LessEqual`, writes opaque
depth, has no blending/discard, and writes output alpha one. Thus a top-alpha-zero
lava surface still occludes later geometry and casts an opaque shadow.

The shader samples top/bottom from the sRGB atlas at packed primary UV plus
independent native CPU offsets. It substitutes current geometric-normal ambient,
directional/shadow and zone point lighting for the native top-light expression,
keeps doubled bottom RGB independent of lighting, then applies current fog to
the combined RGB exactly once. The normalized G-buffer cannot hold the two
independent contributions without losing this expression; the dedicated pass
avoids modifying unrelated material semantics. Atlas binding 8 and the opaque
terrain clamp sampler remain unchanged. Timing query order, frame attribution,
in-app profiling labels and offline CSV fields include the lava pass.

Lava-only and completely empty scenes exposed invalid Metal timestamp pairs
for clear-only geometry passes. Shadow/G-buffer clears remain unconditional;
their timestamp pairs now use the same draw iterator as rendering and require
nonzero index and instance counts. Shadow eligibility includes lava; G-buffer
eligibility excludes it. The lava pass also requires nonzero counts. No timing
decoder acceptance rule changed. GPU regressions cover lava-only, empty,
zero-index and zero-instance scenes, each with successful timestamp completion.

Synthetic GPU regressions independently cover the layer formula at seven top
alpha values and two bottom alpha values, factor-two bottom emission, fog,
independent scrolling, signed UV wrapping/truncation, 100-second and 32-bit
clock closure, zone-light influence only on top, unchanged rejected fallbacks,
opaque depth against later lava/transparent/waterfall/additive/particle draws,
opaque foreground occlusion, UI order, and shadow equality with an ordinary
solid caster. The shadow fixture counts darkened nonblack floor pixels to
exclude merely drawing the black caster. The original Nest integration test
checks all four material groups, exact bindings/rates and 512 opaque triangles.
These are OpenEQ GPU regressions against independently computed expressions,
not original-client golden screenshots.

Validation passed: seven synthetic lava GPU tests, three loader tests, the
original Nest binding/512-triangle test, and existing waterfall/profiling tests.
Strict Clippy across all targets in assets, renderer and main client also passed.

The combined workspace passes 1,233 tests, zero failed/ignored across 120
suites, with original assets and offline/digitally silent audio. Strict
workspace lint, client/audit builds, all-target no-default, formatting and
diff checks pass. Logs are `/tmp/openeq-lava-lamp-final-{workspace,clippy,build,
no-default,fmt}.log`.

The 523-zone structural survey retains 501 passes and the same 22 known cases.
Only 20 additional second-diffuse texture references across 16 zones change;
all other non-timing fields match the preceding layered-color survey. After
the animation-sidecar review fix, all 220 EQGZ declarations were rechecked
against a pinned final audit executable with identical non-timing results.
Reports are `/tmp/openeq-lava-zone-survey/`; this is CPU metadata/structure
coverage, not a claim of 523-zone visual or gameplay parity.

## Frozen local reproduction

Original assets and generated buffers remain outside git. Probe scripts, JSON
and logs are read-only. Run native probes with `PYTHONPATH=/tmp/openeq-re-tools`;
the upload, preshader and residency scripts accept a separate JSON output path
as their first argument. Do not overwrite frozen outputs.

| Local artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-lava-upload.py` | `d3799d2989af340a5757c2f3b9cf7e0d58ab6f8c0fa7d3996c6f44fb0fe770f4` |
| `/tmp/openeq-lava-upload.json` | `10f4335c5c91e09db736c0f2972e93d70d001c8884d93bd236ae119f516d9118` |
| `/tmp/openeq-lava-native-preshader.py` | `b724b74b84c307bd59b2cdf7373d81507fa4bae4f55fec221c857cac56bd04a9` |
| `/tmp/openeq-lava-native-preshader.json` | `bef925e87dbcc0d7c79fbdf5846e71b25b8567e303fc175376b46c9756c234b9` |
| `/tmp/openeq-lava-corpus.py` | `10f5ac8d0e7e4978a651252f68559f7ce87e91a0aced65d4326608a6ae048615` |
| `/tmp/openeq-lava-corpus.json` | `60489a9e0b2a165f9e2319600d8c90ef39dac72f87a15062d9396dd3b9645059` |
| `/tmp/openeq-lava-texture-residency.py` | `0f01c2f2e8989f228dcaf38dff6d7b27ae603b98a36d7de0dc9c781b138a0c99` |
| `/tmp/openeq-lava-texture-residency.json` | `39c945eaa683441ee3acbded5109ba78a64f6cf52576a367d0506f92cc73c840` |

An independent root replay produced identical upload and preshader JSON hashes,
including all 102,880 original interpreter/caller executions. The frozen setup
prefixes and original source hashes are checked by the probes themselves.
