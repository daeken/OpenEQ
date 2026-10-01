# Executed opaque TER single-pass light binding

2026-10-01, research only. This extends
[lighting-source selection and packing](EQG_TER_LIGHTING_SELECTION.md) and
[normal, tangent and color channels](EQG_TER_VERTEX_CHANNELS.md) for ordinary
TER versions 1–3 using exactly `Opaque_MaxCB1.fx`.

The native single-pass draw binds five frame vectors and three ordered
per-batch point-light slots. Selected baked RGB is a separate vertex input;
its alpha weights ambient, bounce and directional light, rather than
controlling surface opacity. The next useful rendering change is an explicit
single-pass material path that consumes those channels with their distinct
roles. Applying the newly retained lighting word as an ordinary vertex tint
inside the shared deferred-lighting function would not reproduce this route.

This note changes no loader or GPU behavior. It executes original CPU
instructions against constructed process state and effect interfaces. It does
not run the original client, create a native graphics device, establish actual
device support, populate a region's light list, or compare rendered pixels.

## Exact material and effect route

The established material route is parse type 8 → runtime type 9 → layout 1 →
region descriptor 6 / declaration 5. Descriptor 6 uses SPL effect index 6,
`RenderEffects/SPL/RegionCB1.fxo`. Its original hash is
`ee5d7aec289ca8917ed900bacf7a28bd4341c0c9991b38270aadd8ef6e1bb87a`.
Its other routes are MPL base 94, light 99 and diffuse 104; this research does
not collapse those routes into SPL.

Addresses refer to `EQGraphicsDX9.dll`, base `0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
At `0x1008c812`, the draw block reads the material batch's descriptor index
(+0x2c), looks up renderer +0x1a20[index], obtains its SPL effect index at
+0x0c, then resolves renderer +0x17dc[effect index] to the effect wrapper.
After `ID3DXEffect::Begin`, the concrete call at `0x1008ca5d` invokes the
frame binder `0x10089590`. Material properties are applied by `0x1008a110`.
The ordinary batch-list call at `0x1008cb0b` invokes `0x1008a9f0`, whose call
at `0x1008aac8` invokes the per-batch light binder `0x1008a380`.

The witness executes that whole bounded draw block through the return from
point binding, using descriptor/effect index 6 and one ordinary batch. Its
five global and six point-vector writes equal the independently executed
binder results. Effect Begin, declaration setup and unrelated render-state
interfaces are controlled boundaries; execution stops before drawing.

## Technique choice is conditional

The original effect contains these techniques in order:

| Technique | Vertex-program file offset |
| --- | ---: |
| `RegionCB1_DX8_VS1_PS14` | `0x2884` |
| `RegionCB1_DX8_VS1_PS11` | `0x1b30` |
| `RegionCB1_DX6_VS1_PS0_NoBump` | `0x0e50` |

`0x1000c0c0` enumerates `FindNextValidTechnique` and gets each technique's
description. `0x1000b970` then scans that list in order, applying renderer
settings and name tests, before `0x1000b810` calls SetTechnique. The witness
executes enumeration through `0x1000c1f2` and the complete selector/binder,
with **explicitly supplied device-valid lists**. These are conditional
selection results, not evidence that a particular real GPU accepts all three.

| Supplied valid list | Other condition | Selected technique |
| --- | --- | --- |
| PS14, PS11, PS0 | Pixel shaders enabled | PS14 |
| PS14, PS11, PS0 | Disable PS14 | PS11 |
| PS14, PS11, PS0 | Disable PS14 and PS11 | PS0 NoBump |
| PS14, PS11, PS0 | Renderer pixel-shader flag off | PS0 NoBump |
| PS11, PS0 | Pixel shaders enabled | PS11 |
| PS0 | Pixel shaders enabled | PS0 NoBump |
| PS14 only | Disable PS14 | No SetTechnique call; previous index restored |

The relevant settings bytes are renderer-settings +0xc6 for disabling PS14
and +0xc7 for disabling PS11; renderer +0xef5 gates pixel shaders. The
selector also recognizes PS20, Blend, IndexBlend and 1Pass in other effect
names. None appears in these three names. The no-bump technique is **still a
vertex-shader route**: its name contains neither `_FF` nor `Disabled`, so the
wrapper's corresponding +6/+5 flags remain zero. Pixel-shader availability
does not imply that this material switches to the fixed-function binder.

## Executed frame constants and semantic lookup

The frame prefix `0x1008bd90..0x1008c120` reads the engine at global
`0x1017c0a8`. Original getter methods execute through engine vtable
`0x1013becc`; the input fields contain asymmetric binary-exact float values.
Only the camera getter is replaced. Shader handles are discovered by the
original semantic-lookup block `0x1000b5d2..0x1000b6cf` against a recording
effect interface, and are checked against the original FXO parameter table.

| Effect semantic | Native input | Renderer vector | Wrapper handle |
| --- | --- | ---: | ---: |
| Ambient | engine +0x240 through `0x1006b0d0` | +0xb0d0 | +0xa0 |
| SpecialAmbient | engine +0x24c plus +0x258, each component upper-clamped to 1 | +0xb0e0 | +0xa4 |
| DirectionalColor | object at engine +0x230, RGB at +0x5c | +0xb0f0 | +0xa8 |
| BounceColor | engine +0x268 through `0x1006b0e0` | +0xb100 | +0xb0 |
| DirectionalNormal | same directional object, XYZ at +0x50 | +0xb110 | +0xac |

Special-ambient getters are `0x1006b100` and `0x1006b120`. All five vectors'
W components are set to zero. No lower clamp is applied to special ambient.
The executed inputs `(0.25, 0.75, -0.5)` and `(0.125, 0.625, 0.25)` produce
`(0.375, 1, -0.25, 0)`. Direction `(2,-3,4)` remains unchanged; this prefix
neither normalizes nor negates it. These deliberately unusual finite values
expose transformations; they are not asserted to occur in a particular zone.

The same prefix also prepares half-valued copies at +0xb120, +0xb130,
+0xb140 and +0xb150. **The SPL binder reads the full vectors above.** It does
not multiply them by 0.5 or by vertex lighting alpha. The full native binder
`0x10089590` calls SetVector (vtable +0x88) once for each nonnull light handle.
Lighting alpha is applied later by the vertex program. Shader-file default
ambient values are therefore not the runtime zone ambient when this binder
supplies the parameter.

## Three point slots, radius and missing lights

The point binder `0x1008a380` receives a prepared light-list reference from
one of three batch forms. In the ordinary batch route executed here, the
second argument's +0x10 points to a pointer to that list. The list stores its
count at +0x1c and light pointers beginning at +0x20. The other two argument
forms access +0x34 and +0x24 respectively. Its programmable branch iterates
slots 0–2 in existing list order; it does not calculate nearest lights.

The witness creates four RGB definitions with the original `0x10012bb0`
constructor and four point lights with the original `0x100126e0`
constructor. Only allocation and memcpy are replaced. Actual vtables
`0x101350b8` and `0x1013502c` then dispatch the original getter chain:

- `0x100129b0`: point-light radius at +0x5c, returned in x87 ST0.
- `0x10012990`: RGB-definition pointer at +0x0c.
- `0x10012a80`: definition's RGB-array pointer at +0x5c.
- `0x10012750`: point-light XYZ at +0x50, copied to the output vector.

Each populated shader slot receives **color = (R,G,B,radius)** and
**position = (X,Y,Z,1)**. No intensity multiplier, gamma conversion,
coordinate conversion, radius clamp or inverse-radius calculation occurs
in this binder. Coordinates are the already-constructed light object's
coordinates. The point-0 position semantic is literally `Position` in both
the DLL and original effect, while later slots use `PointLight1Position`
and `PointLight2Position`.

| Slot data | Bound color | Bound position |
| --- | --- | --- |
| First asymmetric witness | `(0.125,0.25,0.375,7.5)` | `(100,-20,3,1)` |
| Second witness | `(0.25,0.5,0.75,15)` | `(200,-40,6,1)` |
| Third witness | `(0.375,0.75,1.125,22.5)` | `(300,-60,9,1)` |
| Absent slot or null light pointer | `(0,0,0,0.01)` | `(0,0,-10000,1)` |

Tests cover counts 0, 1, 2, 3 and 4, plus a null middle slot. Four entries
produce exactly the first three. A null middle slot clears that slot while
the third retains the third light; lights are not compacted. Successive calls
verify that missing lights overwrite previous constants. The finite RGB
value 1.125 is copied unchanged. `0.01` means the stored f32 value
`0x3c23d70a`.

The PS14 preshader's decoded operations at FXO offsets `0x2e7c`, `0x2e9c`
and `0x2ebc` take reciprocals of the point color W components (order 1,2,0)
into VS c5.x/c6.x/c7.x. Combining that decoding with the executed radius
binding closes the prior radius meaning: for ordinary positive finite
radius, the shader's attenuation is `1 - min(distance² / radius², 1)`,
multiplied by the clamped geometric-normal/light-direction dot product and
RGB. This is **vertex lighting** before interpolation. The preshader was
decoded, not executed through native D3DX in this probe; zero/nonfinite
radius and precise GPU arithmetic are not new compatibility claims.

## What this permits next

A defensible next renderer change is an explicitly scoped SPL terrain path
for the exact retained material family. Its first visible behavior should
consume native baked `C.rgb` as a direct base-light contribution and `C.a`
as a light weight. Supply separate ambient, special-ambient, bounce,
directional and ordered point-slot inputs, instead of treating the entire
word as a tint or putting all zone lights through the shared deferred loop.
For the chosen PS14 profile, compute the native base/bounce/three point terms
at vertices and interpolate them; apply the directional normal-map term in
the fragment stage as documented in the channel note. A no-bump profile must
be a separate explicit branch. It must not silently claim to reproduce
original device-dependent selection.

This provides a concrete way to render retained baked lighting with the
right contribution and alpha role. A staged implementation can validate
controlled zero-point-light/frame-constant scenes first, followed by the
asymmetric populated-slot fixtures above. It should use original-index
mapped lighting words, including zero/nonzero-alpha witnesses from Guild
Hall and Causeway, and must keep lighting alpha separate from surface alpha.
The current shared `shade_surface_unfogged` path normalizes a fragment
normal, shades all affecting lights per pixel with cubic falloff, and
multiplies ambient into albedo. That is a distinct model; changing its falloff
or multiplying its albedo by native C would not implement the witnessed SPL
path.

Remaining prerequisites for claiming live native lighting parity are specific:

- Where zone/time-of-day state supplies the engine's ambient, special,
  bounce and directional fields. The probe starts from those fields and
  does not establish OpenEQ's environment values as equivalents.
- How each terrain/object light list is populated, filtered and ordered.
  This note does not justify choosing three nearest lights, choosing the
  first three archive lights, or adding every static zone light again on top
  of baked RGB.
- The native normal/tangent packing and runtime arithmetic state recorded in
  the channel/FPU notes; current normalized float normals are not proven
  equivalent inputs.
- Actual SPL-versus-MPL host selection, final legacy color clamping,
  interpolation, sampling, render-target color space and fog/blend behavior.

The controlled fixture route and the live environment/list-selection problem
are distinct acceptance targets. Until the latter inputs are established,
an explicit compatibility/diagnostic profile can demonstrate the witnessed
lighting, but should not replace all live material lighting as a parity fix.

## Reproduction and frozen results

`/tmp/openeq-ter-light-binding.py` is self-contained apart from `pefile`,
Unicorn and the two original installed binaries. It checks both input hashes,
parses original effect parameters and technique names, and writes
`/tmp/openeq-ter-light-binding.json`. Run with
`PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-light-binding.py`.
All assertions pass under Unicorn 2.1.4, x87 CW 0x037f, MXCSR 0x1f80.

The result contains ten assembled frame vectors, eleven semantic lookups,
five global bindings, six point-list cases, eight conditional technique cases
(including the final draw-chain setup), separate ordinary-batch binding, and
the combined descriptor → effect → global binder → batch → point binder
writes. Native getter counts also prove execution through original point-light
and engine vtables. Allocation, memcpy, camera getter, CRT strstr,
device-valid effect enumeration and effect/GPU interfaces are controlled.
No shader math is replaced by a native-execution claim.

| Artifact | SHA-256 |
| --- | --- |
| Probe `.py` | `9c278977e3c7d9c8b983489f34f7aef8a3a49c51dea5a5b6ddcd816a787e1fb6` |
| Result `.json` | `c89d039b6183d764df989e1fc2b8bc3c40e8857b3a86d82a69674e13b534da59` |
