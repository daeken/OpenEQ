# Native EQG lava and waterfall: binding follow-up

October 1, 2026. This is source and original-instruction research, with no
renderer change. The Nest has 512 restored lava triangles and 916 waterfall
triangles that still use ordinary diffuse rendering. Waterfall transparency
and independently scrolling color/alpha are missing; lava uses two diffuse
layers, a scrolling normal map and a different light expression.

## Original materials

The independently parsed `thenest.eqg/ter_abyss01.ter` is version 2, with 3,813
materials, 384,042 vertices and 320,324 polygons. Its SHA-256 is
`238b0f41bd5133de9e9a5f04cc2502f090b120028bf7473b4f247340e3c2a176`.
First exact-name resolution agrees with `EQG_MATERIAL_IDENTITY.md`:

| Family | First-name record | Referencing ordinals | Triangles | Authored slide 1 / slide 2 |
| --- | --- | --- | ---: | --- |
| `Opaque_MaxLava.fx` | 53, `lavaflow` | 2267, 2291, 3251, 3275 | 512 | `(0.3,0)` / `(0.2,0)` |
| `Opaque_MaxWaterFall.fx` | 3788, `waterfalls` | 3790, 3792, 3794, 3798, 3800 | 916 | `(-0.12,-0.32)` / `(0,-0.5)` |

The decimals above describe authored f32 values; the JSON retains their exact
expanded values. Both materials author slide values different from the effect defaults; a
default-only implementation would omit that source data. Native property
upload remains outside the executed binding slice below.

Lava binds `e_TextureDiffuse0=kl_lavaTop_c.dds`,
`e_TextureDiffuse1=kl_lavaBottom_c.dds` and
`e_TextureNormal0=kl_lava_n.dds`. The top/bottom maps are 256-square DXT3;
the normal is 128-square DXT3. Their alpha data is not uniformly opaque.
The waterfall binds only `e_TextureDiffuse0=wtr_waterfall_tile.dds`, a
256-square uncompressed ARGB texture with all 256 alpha values represented:
2,416 texels are fully transparent, 8,281 fully opaque and 5,853 below 16/255.
These are level-zero texel counts, not filtered pixel coverage.

After the declared polygons, this TER has **3,072,340 additional bytes**,
exactly `4 + vertex_count * 8`, beginning with word 1. Their interpretation is
not established by this material audit. They are not discarded from the source
file or interpreted as a guessed UV channel. Active native tail-reader tracing
is a separate follow-up, especially for this zone's `_2UV` shader families.

## Executed native binding

The installed graphics DLL is the same SHA-256 recorded in
`EQG_ADDITIVE_SHADER.md`. `/tmp/openeq-lava-waterfall-binding.py` executes:

- Classifier `0x10014450` through the selected-name branch at `0x10014de5`,
  supplying controlled material/string records and substring lookup.
- Final runtime remapping `0x10015a4c..0x10015eff` as a **separate slice**;
  property ingestion between these slices is not executed.
- TER descriptor switch `0x1008fb7c..0x1008fd02` with that runtime type.
- Complete descriptor initialization `0x100826d0`, including original
  descriptor constructors, with controlled process allocation/copy interfaces.
- Descriptor state binder `0x10089370` and the real cached state setters.

| Name suffix | Parser / runtime type | TER descriptor | Single-pass effect index / filename | Vertex layout selector |
| --- | --- | --- | --- | --- |
| `MaxLava` | `0x13 / 0x14` | `0x0e` | `0x0e`, `SPL/RegionLava.fxo` | 5 |
| `MaxLava2` | `0x14 / 0x15` | `0x0f` | `0x0f`, `SPL/RegionLava2.fxo` | 5 |
| `MaxWaterFall` | `0x36 / 0x36` | `0xbf` | `0x2d`, `SPL/RegionWaterFall.fxo` | 1 |

The two lava descriptors request depth writes, with no alpha/additive flags;
their binder emits no state writes and relies on the surrounding opaque state.
Waterfall requests ordinary source-alpha blending and low-reference alpha test,
with no depth writes. Its executed render-state writes are exactly:
`ALPHABLENDENABLE=1`, `SRCBLEND=SRCALPHA`, `DESTBLEND=INVSRCALPHA`,
`ALPHATESTENABLE=1`, `ALPHAREF=16`, `ZWRITEENABLE=0`.
Fog is not disabled by this waterfall binder. Alpha comparison/depth/culling
still depend on the surrounding initialization described in the additive note.
This isolated binder execution does not establish every inherited draw state.

## Compiled effect observations

The effect parser reaches exact EOF for both files. Waterfall has a PS1.1
technique and a fixed-function fallback; Lava has PS1.4, PS1.1 and a one-layer
no-bump fallback. Actual device technique admission/selection for these families
and original GPU pixels are not executed here.

The waterfall PS1.1 program at `0x1fb8` samples the same diffuse texture twice:
color from the first UV, alpha from the second. Instructions at `0x2078` and
`0x2088` produce `rgb = texture(UV0).rgb * interpolated_light.rgb` and
`alpha = texture(UV1).a`. Its vertex program generates both UVs from packed
primary coordinates / 256 plus separate slide offsets. Thus using color's
sampled alpha, or vertex alpha as final opacity, would be incorrect.

The Lava PS1.4 program at `0x3360` samples two scrolling normal inputs and
two diffuse inputs. At `0x34ec` it sums the signed normals; `0x34fc` takes a
saturated dot with the signed interpolated light direction. It combines that
term with point-light color and interpolated base light, multiplies the first
diffuse RGB, then interpolates between that result and **twice** the second
layer's RGB using the first layer's alpha (`0x3530`). Final alpha is one.
This is not a generic emissive surface or ordinary transparent texture.
Full vertex lighting, normal upload and technique equivalence remain separate.

Effect defaults are slide 1 `(0.02,0.02)` and slide 2 `(0.03,0.03)`.
Decoded preshaders compute a signed remainder with period 100, then multiply
that phase by each slide component. The independent native Time provider is
established in `EQG_EFFECT_CLOCK.md`; it is a frame-cached millisecond clock,
not the server's accelerated day clock. Preshader decoding here is static
inspection, not original D3DX preshader/GPU execution.

## Reproduction and next implementation boundary

Run the two local Python probes below. The assets probe independently inflates
PFS entries, walks original material/polygon records and counts texture alpha;
it does not use OpenEQ's loader. Results contain metadata/histograms, not
original pixels. The binding probe requires `PYTHONPATH=/tmp/openeq-re-tools`.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-lava-waterfall-binding.py` | `c71ce0d0bff4086264a9c9a9f168a1d14414936ddfc5fd03e26b89e2df9cfbb0` |
| `/tmp/openeq-lava-waterfall-binding.json` | `8bdfbf887b357432e6a12cdaf197ec1cb3f82745db0751011830122e5209fca2` |
| `/tmp/openeq-lava-waterfall-assets.py` | `48e116cb13cc55df7fe44618e7361b0f5a4236647e919b459fa92154d38b1ff1` |
| `/tmp/openeq-lava-waterfall-assets.json` | `e111f7cac33dcb0f63dfcf19746660b3641592e8e37430b7332899ad10a65683` |
| `SPL/RegionWaterFall.fxo` | `65aa785ab770b621669e65f7e5b99e2397e8eeaac7efc4d398019cdf916514fd` |
| `SPL/RegionLava.fxo` | `c1dec3fe94eed577ae318a87d2954e6f920d152ee9ed55a76b6c3bc063eebace` |

The decoded local reports are `/tmp/openeq-{waterfall,lava}-{effect,shader,preshader}.txt`.
A future implementation must retain authored slide properties and texture
identities, prove the relevant upload/parameter binding, test independent
color/alpha UVs and cutoff/depth/fog behavior, and distinguish its remaining
lighting/color-space approximations. No live material change is enabled here.
