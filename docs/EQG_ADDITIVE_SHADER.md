# Native EQG additive glass

2026-10-01. Frozen research checkpoint; this document changes no rendering code.
Thundercrest's 72 glass triangles use `AddAlpha_MaxCB1.fx`. The native region
path selects **ONE/ONE blending**, enables an alpha test with reference 16 and
comparison GREATEREQUAL, disables fog, and disables depth writes. Its diffuse
texture has alpha **102/255 in every texel**. The programmable shader retains
that alpha but does not multiply RGB by it. Routing this material through
source-alpha-weighted transparency would therefore reduce its contribution
to 40% of the native value.

This is static evidence from the installed assets and graphics DLL, not a live
original-client frame comparison. It establishes a bounded implementation
target without claiming parity for every additive shader or rendering mode.

## Source witness

`thundercrest.eqg/ter_stormtower01.ter` is version 2, with 295,380 string bytes,
2,340 material records, 762,815 vertices and 290,120 polygons. Source material
ordinal 227 is referenced by the 72 glass polygons and resolves to first exact
name ordinal 32 under the native identity rules in
[EQG_MATERIAL_IDENTITY.md](EQG_MATERIAL_IDENTITY.md). Both records have stored ID
32, name `STORMTOWER_33`, shader `AddAlpha_MaxCB1.fx`, and exactly these properties:

| Property | Tag | Value |
| --- | ---: | --- |
| `e_TextureDiffuse0` | 2 | `rc_ST_Gcglass_c.dds` |
| `e_TextureNormal0` | 2 | `test2_n.dds` |

There is no authored opacity, color, or channel-selection property here.
The first affected source polygon is ordinal 19,671, with words
`(57396, 54999, 55001, 227, 0)`. Its first position is approximately
`(3228.3694, -3507.4661, 290.3383)` in original coordinates.

The diffuse is a 128 by 128 DXT3 DDS of 16,512 bytes, containing one stored
level (header mip count zero). Independently unpacking all 16,384 explicit
four-bit alpha values gives this complete histogram:

| Decoded alpha | Texels |
| ---: | ---: |
| 102/255 = 0.4 | 16,384 |

Every texel passes the native cutoff. The normal texture is a 256 by 256,
262,272-byte, uncompressed ARGB DDS. Every pixel is RGBA `(128,128,255,255)`:
a flat tangent-space normal, with no alpha pattern to explain opacity.

Payload SHA-256 values:

| Payload | SHA-256 |
| --- | --- |
| `ter_stormtower01.ter` | `9b2f75fe159d1a1956d7efc95e480d6ffd615b36de2e5cd3cfb14c3c90bb204f` |
| `rc_st_gcglass_c.dds` | `6be103e86690db4837b4fd86a7b9fc823d3c6951ec3fa1b0a2833aba4fadd36d` |
| `test2_n.dds` | `3588a9da14175a7b206f6b42aa2cf064ec535d9231af1b87ec9e8a62325a34a6` |

## Material, descriptor and effect binding

The installed PE32 `EQGraphicsDX9.dll` has SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below are preferred virtual addresses at image base `0x10000000`.
For the cited code in `.text`, file offset is VA minus `0x10000c00`.

1. The common TER/MOD material processor calls `0x10014450` at `0x1006289d`.
   The parser matches `AddAlpha` at `0x100145db..0x100145f3`, then `CB1` at
   `0x1001468a..0x100146a7`, selecting material type `0x3b`. Its final
   conversion at `0x10015eb3..0x10015ebf` preserves runtime type `0x3b` in
   material `+4` and sets its tangent/bump flag at `+8`.
2. The region upload bridge `0x10019560` resolves a material through
   `0x10016d20`. For this type, `0x100196ed..0x1001970c` copies material `+4`
   into the type argument and selects a region upload layout. The arguments
   passed at `0x1001978d..0x100197a0` feed `0x1008cf00`; that function stores
   layout at geometry `+4` and runtime type at geometry `+8`
   (`0x1008d4b5..0x1008d4ca`). The material's byte flag at `+8` and the
   geometry's type DWORD at `+8` are different fields in different objects.
3. The uploaded geometry is passed to `0x1008f960` at `0x10019891`.
   Geometry layouts 0 through 7 take the region switch at `0x1008fb7c`.
   Jump table `0x1008fe70`, entry `0x3b`, leads to `0x1008fcfd`, selecting
   render descriptor **`0xc7`**. The new render record stores that descriptor
   index at `+8` (`0x1008fd28..0x1008fd2b`).
4. Descriptor initialization `0x100826d0` uses 28-byte source records. Record
   `0xc7` at stack `+0x15e0` has DWORDs
   `[-1, -1, -1, 6, 5, 0x00010100]` and trailing byte zero
   (`0x1008553e..0x10085570`). Constructor `0x100a01a0` maps these into a
   32-byte descriptor: normal-alpha flag zero, additive flag one, low-reference
   alpha-test flag one, high-reference flag zero, depth-write flag zero,
   single-pass effect index 6, and shader vertex-layout selector 5.
5. Effect filename table `0x10175548`, index 6, points to
   `SPL/RegionCB1.fxo` at `0x1013dae8`. Loader `0x10087010` reads that table
   at `0x10087089` and loads the effect through `0x1000be30`. Descriptor
   construction stores records under renderer `+0x1a20`; effect objects live
   under renderer `+0x17dc`.
6. The ordinary draw loop reads the descriptor at `0x1008c819`, takes its
   single-pass effect index at `0x1008c82c`, applies descriptor state at
   `0x1008c95f`, and begins the effect at `0x1008ca55`. Its region geometry
   list is drawn through `0x1008a9f0` at `0x1008cb0b`. This draw helper begins
   the effect pass and flushes cached D3D state immediately before drawing
   (`0x1008ad20..0x1008ad3a`).

The other geometry-layout switch in `0x1008f960` and the alternate-layout
function's switch at `0x100903c8` do not support type `0x3b`. This evidence
must not be generalized into an all-model additive-material rule.

## State contract

Descriptor binder `0x10089370` sets the following states for descriptor `0xc7`:

| State | Native value | Evidence |
| --- | --- | --- |
| Alpha blending | Enabled | `0x100893e5..0x100893ef` |
| Source blend | `D3DBLEND_ONE` (2) | `0x100893f4..0x100893fe` |
| Destination blend | `D3DBLEND_ONE` (2) | `0x10089403..0x1008940d` |
| Fog | Disabled | `0x10089423..0x1008942d` |
| Alpha test | Enabled | `0x10089432..0x10089448` |
| Alpha reference | 16 | `0x1008944d..0x10089464` |
| Depth write | Disabled | `0x10089469..0x10089479` |

The common state initialization sets `ALPHAFUNC=GREATEREQUAL` (7) at
`0x10092124..0x1009212e`, giving acceptance of sampled alpha >= 16/255.
It also enables depth testing at `0x1009208e..0x10092098`, selects
`ZFUNC=LESSEQUAL` (4) at `0x10092106..0x10092110`, and sets
`CULLMODE=CCW` (3) at `0x100920f7..0x10092101`. The draw loop also restores
CCW culling at `0x1008c7d0..0x1008c7da`. The additive descriptor does not
disable depth testing or culling. The normal upload path can duplicate reverse
indices for a separately supplied two-sided flag (`0x1008d470..0x1008d49d`);
the additive shader name by itself does not imply two-sided geometry.

These calls target cached setter `0x1009f2d0`. The cache flush issues actual
`IDirect3DDevice9::SetRenderState` calls through vtable slot `+0xe4` at
`0x1009ef20..0x1009ef29`. Descriptor cleanup `0x100894a0` disables blending
and alpha testing, restores ONE/ZERO factors and depth writes, and re-enables
fog if the zone has it enabled.

Neither this descriptor binder nor the effect passes set `BLENDOP` or enable
separate alpha blending. The RGB sum below uses D3D9's default `BLENDOP=ADD`;
no override was found in the examined path. Destination-alpha behavior, other
render-target configurations and all possible earlier state mutations were
not independently audited. Do not treat this as a measured framebuffer-alpha
contract.

## Texture channels and lighting

`RenderEffects/SPL/RegionCB1.fxo` is 13,528 bytes, SHA-256
`ee5d7aec289ca8917ed900bacf7a28bd4341c0c9991b38270aadd8ef6e1bb87a`.
The D3DX container parses to exact EOF and exposes three one-pass techniques:
`RegionCB1_DX8_VS1_PS14`, `RegionCB1_DX8_VS1_PS11`, and
`RegionCB1_DX6_VS1_PS0_NoBump`. Their pass declarations bind vertex/pixel
shaders; the fallback also binds a texture. They contain no blend, alpha-test,
depth, or fog render-state override. Both texture samplers specify linear
minification, magnification and mip filtering.

The PS1.4 program at file offset `0x2744` samples normal RGB and diffuse RGBA
(`0x2804/0x2810`). It decodes the normal and tangent light vector as
`2 * value - 1`, computes their saturated dot product (`0x2828`), then computes
lighting as `v0.rgb + dot * v1.rgb` (`0x2838`). It copies `v0.a` to the lighting
alpha (`0x284c`) and multiplies by the diffuse RGBA (`0x2858`). The associated
vertex shader writes `v0.a = 1` at `0x3448`. Consequently:

```text
source.rgb = diffuse.rgb * (base_light.rgb + normal_dot * directional_light.rgb)
source.a   = diffuse.a
keep       = source.a >= 16/255
result.rgb = destination.rgb + source.rgb  (for a kept fragment)
```

The PS1.1 technique independently agrees: lighting at `0x1ae4`, alpha copy
at `0x1af8`, final diffuse multiply at `0x1b04`, and vertex output alpha one
at `0x26f4`. Input vertex COLOR0 alpha participates in lighting weights; it
is not forwarded as output opacity. The vertex program includes ambient,
bounce, directional, point and baked vertex-light terms. Additive blending
therefore does **not** imply that this shader is unlit or fully emissive.
Normal-map alpha is not used as opacity in these programs.

Diffuse and normal coordinates come from the same authored primary UV input,
divided by 256 (`0x3454..0x3470`) after native packed-vertex upload. There is
no evidence for a second UV channel or generated world-coordinate texture
lookup on this material. The fallback vertex shader also outputs alpha one,
but its complete fixed-function texture-stage behavior and the device's
technique choice remain outside this checkpoint.

## Opaque binding lead retained for nonfinite-UV research

The same tables refine the open binding question in
[EQG_NONFINITE_ATTRIBUTES.md](EQG_NONFINITE_ATTRIBUTES.md). Parser types are
not always identical to the runtime type consumed by the draw switches:

| Source shader | Parser -> runtime type | Simple-model descriptors | Multipass texture effect | Single-pass effect | Shader layouts |
| --- | --- | --- | --- | --- | --- |
| `Opaque_MaxC1.fx` | 5 -> 6 | `0x24 / 0x25` | `MPL/SModel_TextureD1.fxo` (`0x7c`) | `SPL/SModelC1.fxo` (`0x10`) | 1 / 9 |
| `Opaque_MPLBasic.fx` | `0x15 -> 0x16` | `0x3c / 0x3d` | `MPL/SModel_TextureD1T.fxo` (`0x7d`) | `SPL/SModel_Basic.fxo` (`0x3f`) | 2 / 10 |

The `MaxC1` default parser branch is `0x10014dc7..0x10014de3`, followed by
runtime remap `0x10015ad0..0x10015adc`. The `MPLBasic` match selects `0x15`
at `0x100149e1`, then remaps it at `0x10015c16..0x10015c26`.
The simple-model switch `0x1008fd78` selects `0x24` at `0x1008f9ce` for
runtime type 6 and `0x3c` at `0x1008fa64` for runtime type `0x16`.
Alternate-layout switch `0x100903c8` selects `0x25` at `0x10090029` and
`0x3d` at `0x100900ec`. All four descriptors have depth writes enabled and
no alpha/additive flags.

In particular, `MPLBasic` points to **TextureD1T**, not TextureD1. These are
descriptor alternatives; this work does not establish the active single-pass
versus multipass choice for the affected NaN fixtures, or finish their packed
vertex-layout bridge. It does not justify changing raw parser attributes.

## Implementation boundary

A follow-up should preserve the established region material binding and add a
dedicated additive path with the RGB blend factors, cutoff, depth-write and
fog behavior above. It should verify background preservation, full-strength
addition for alpha 0.4, alpha-cutoff rejection, depth occlusion, and the absence
of depth writes with GPU regressions. The existing weighted transparency path
cannot express this contract merely by assigning opacity 0.4. Exact light
intensity, native color-space behavior, other AddAlpha variants, and original
client frame parity require separate validation.

Research utilities and full diagnostic output remain local under
`/tmp/openeq-addalpha-*`, `/tmp/openeq-terrain-{effect,shader}-audit.py`, and
`/tmp/eqgraphics-disassembly.txt`. Only installed original assets were read.
No original textures, binary assets or disassembly dumps are added to the repo.
