# Native waterfall terrain coordinates and draw-state inheritance

October 1, 2026. This closes the upload ambiguity left by
`EQG_LAVA_WATERFALL.md`. Exact `Opaque_MaxWaterFall.fx` terrain uses the primary
UV pair, packed into signed 16-bit coordinates at 1/256 precision. Both scrolling
shader coordinates derive from that same pair. The TER version-2 trailing float2
stream does not supply the waterfall's second texture sample.

## Upload and declaration are different selectors

The complete native material reader `0x10014450` gives the original Nest
`waterfalls` material parser/runtime type 54, bump byte `+8 = 0` and extra-color
byte `+9 = 0`. The executed ordinary region selector
`0x1001960c..0x10019732` chooses **upload layout 0**, passed to `0x1008cf00`.
Byte `+9` controls an extra color, not the number of source UV pairs.

Descriptor 191 chooses effect 45 (`SPL/RegionWaterFall.fxo`) and **declaration
selector 1**. The earlier note's column called “Vertex layout selector” is this
last declaration selector; it must not be confused with upload layout 1,
which has a different stride and tangent fields.

Complete declaration initialization `0x100825a0`, followed by native declaration
binding `0x10089240` and `0x1009f490`, selects the original table at `0x10175110`:

| Byte offset | D3D9 declaration | Meaning |
| ---: | --- | --- |
| 0 | `FLOAT3 POSITION0` | Position |
| 12 | `D3DCOLOR NORMAL0` | Packed normal |
| 16 | `D3DCOLOR COLOR0` | Vertex lighting |
| 20 | `SHORT2 TEXCOORD0` | Packed primary UV |

The stride is 24 bytes. No second coordinate declaration exists in this layout.
Controlled `CreateVertexDeclaration` and `SetVertexDeclaration` callbacks verify
that the initialized selector-1 handle is the handle actually bound.

The original expanded vertex copy `0x100209a7..0x10020ac7` retains UV0 in runtime
attribute bytes 0/4 and the trailing UV1 in bytes 8/12. The **complete** original
uploader `0x1008cf00` then executes its layout-0 vertex body at
`0x1008d334..0x1008d408`. Its two conversions read only attribute bytes 0/4,
multiply by 256, call `0x1010f280`, and store the low 16 bits. Attribute byte 16
supplies `COLOR0`; attribute byte 20 is not an extra waterfall color input.
The probe deliberately assigns different words to those two color fields.

For ordinary finite input within signed-i32 conversion range, this is:

```text
packed_uv = signed16(low16(truncate(source_uv * 256)))
shader_base_uv = float(packed_uv) / 256
```

Packing happens per vertex, before interpolation. Signed wrapping matters:
source coordinate 128 becomes shader coordinate -128. This note does not
extend the formula to nonfinite or signed-i32-overflow inputs. Both native
conversion modes agree for the chosen synthetic cases and all original Nest
waterfall vertices.

## Executed witnesses and mapChannel

The probe exercises six material/region selections: the actual Nest material
and integer `e_TextureDiffuse0mapChannel` values 0, 1, 2, 3 and `0xffffffff`,
with additional normal/second mapChannel properties. All retain runtime54,
no bump/extra-color, and upload layout0. Those properties do not redirect the
layout-0 vertex loop. Generic integer-property exclusion from programmable
binding is separately established in `EQG_MATERIAL_PARAMETER_BINDING.md` and
`EQG_TER_SECONDARY_UV_SHADERS.md`.

Three asymmetric synthetic vertices cover negative values, truncation, signed
wrapping and a nonfinite value in the unused UV1 pair. Changing **only UV1**
leaves the complete packed vertex buffer SHA-256 unchanged. Each case runs
with both native conversion-mode settings.

The original Nest material resolves through first exact-name record3788,
referenced by 916 polygons and **972 distinct source vertices**. Every one of
those vertices has UV1 different from UV0. All972 pass the same original CPU
copy and complete native upload, with every packed primary coordinate compared
against the independent conversion oracle. Original source-index witnesses are
382802, 383288 and 383805. The sorted source-index list SHA-256 is
`7e2596b91e14ab4c3aef6dc8e2684b8b53d18b5817f7a035653ae19d517be9aa`.

The complete uploader also emits a controlled triangle's indices and geometry
record. Index/vertex buffer allocation endpoints are synthetic; source material
reading, declaration selection, vertex packing, index output and geometry-record
assembly execute original DLL instructions. Native region partitioning and a
complete effect draw are outside this probe.

## Shader connection and states

Static decoding of `RegionWaterFall.fxo` gives `v3 = TEXCOORD0` and
`c30.x = 1/256`. The PS1.1 vertex program's instructions at effect offsets
`0x306c`, `0x3080`, `0x3094` and `0x30a8` produce:

```text
UV_color = shader_base_uv + (slide1_x, slide1_y)
UV_alpha = shader_base_uv + (slide2_x, slide2_y)
```

Both use `v3.xy`. The fallback vertex program has the same coordinate source
at offsets `0x1ee8..0x1f24`. The slide offsets are preshader results; their clock
and authored-parameter binding are documented separately. Pixel RGB uses the
color sample and interpolated lighting, while pixel alpha comes from the second
sample. Original D3DX evaluation and GPU pixels are not executed here.

A separate six-case state probe executes the original descriptor constructor,
waterfall binder and cached setters, with two distinct prior state sets and
optional original fixed reset prefix `0x10092050..0x10092512`:

| State | Waterfall binder | With that native reset prefix |
| --- | --- | --- |
| Alpha blending | Enabled; SRCALPHA / INVSRCALPHA | Same |
| Alpha test | Enabled; reference16 | Comparison GREATEREQUAL |
| Depth writes | Disabled | Disabled |
| Depth test / comparison | Inherited | Enabled / LESSEQUAL |
| Culling | Inherited | CCW |
| Fog enable | Inherited | Still inherited |
| Sampler address U/V/W | Inherited | WRAP on stages0–7 |
| Sampler min/mag | Inherited | LINEAR on stages0–7 |
| Sampler mip | Inherited | POINT or LINEAR by renderer setting |
| Sampler LOD bias | Inherited | -1 only with the tested capability flag |
| Texture/framebuffer sRGB | Inherited | Still inherited |
| Blend operation / separate alpha | Inherited | Still inherited |

The two distinct seeds demonstrate inheritance rather than merely observing
zeroed memory. The compiled effect declares LINEAR min/mag/mip filtering for
`s_SamplerDiffuse0`; it does not explicitly author address mode or sRGB in the
examined effect metadata. This is **not** proof of every live draw state: the
state probe does not execute a complete runtime reset, intervening scene draws,
D3DX effect-state application, device flush or GPU. A normal waterfall path can
use the established reset-plus-descriptor contract, but its remaining inherited
and color-space assumptions must remain explicit.

## Reproduction

Run with `PYTHONPATH=/tmp/openeq-re-tools`. The probes validate the installed
DLL SHA-256 `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`
and the original Nest TER hash recorded in `EQG_LAVA_WATERFALL.md`.

| Local evidence | SHA-256 |
| --- | --- |
| `/tmp/openeq-waterfall-upload.py` | `1cda002930c94810faaaf9acdb69ac9ef842bc849f5a2fb1689c94012164ada2` |
| `/tmp/openeq-waterfall-upload.json` | `d6471f922103154cf4234b6deb035871ce6019ceefbc9e4f2ce810653636dd0b` |
| `/tmp/openeq-waterfall-state-inheritance.py` | `5ae337f80dc054d8d686e62639fd3583ac0f5c2408b23e41d9089a078b7c4bef` |
| `/tmp/openeq-waterfall-state-inheritance.json` | `dc9b2e6fb65a54ef6133be4a26875feedec545d1952c374c6c1c0128e7ba2b7b` |

The upload probe reuses only a hash-checked prefix of the material-binding
harness; the state probe reuses only the hash-checked upload setup. Neither
rewrites another probe's frozen output. No production renderer change is made
by this research.
