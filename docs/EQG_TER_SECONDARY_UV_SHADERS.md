# Native TER secondary-UV shader families

2026-10-01. The Nest's `Opaque_MaxCB1_2UV.fx` and
`Opaque_MaxCBSG1_2UV.fx` use **UV0 for diffuse and normal textures, and UV1
for a second color texture**. Both coordinate pairs are independently packed
to signed 16-bit values at 256 units per tile, then divided by 256 in the
vertex program. Their programmable pixel shaders multiply the two color
samples; UV1 is not a second normal coordinate, light-map selector, or a copy
of UV0.

This research changes no renderer behavior. It connects the native optional
TER UV stream to an executed material/upload/binding route and inspects the
original compiled shader programs. It does not execute a native graphics
device, establish device-valid techniques, or certify complete original GPU
pixels. The separately retained UV metadata is not itself a rendering fix.

## Original Nest material scope

The independent PFS/TER walker reads `thenest.eqg/ter_abyss01.ter`, version 2,
SHA-256 `238b0f41bd5133de9e9a5f04cc2502f090b120028bf7473b4f247340e3c2a176`.
It resolves polygon material ordinals through the first exact material name,
following [native material identity](EQG_MATERIAL_IDENTITY.md).

| Exact family | Resolved definitions | Total triangles | Triangles restored by material-identity correction |
| --- | ---: | ---: | ---: |
| `Opaque_MaxCB1_2UV.fx` | 19 | 188,101 | 121,401 |
| `Opaque_MaxCBSG1_2UV.fx` | 8 | 117,421 | 117,421 |

Every definition supplies `e_TextureDiffuse0`, `e_TextureNormal0`, and
`e_TextureSecond0`. The eight CBSG definitions also supply `e_fShininess0 = 12`.
None supplies a `mapChannel` property. Representative first-name records are:

| Ordinal / name | Diffuse | Normal | Second color |
| --- | --- | --- | --- |
| 0 / `ground` | `rc_RCDFlr_c.dds` | `Di_nest_abyss_floor_8_n.dds` | `Di_nest_desert_ov02_c.dds` |
| 12 / `firegrnd` | `Di_nest_fire_floor_c.dds` | `Di_nest_fire_floor_8sg_n.dds` | `Di_nest_desert_ov02_c.dds` |
| 19 / `metalgrnd` | `Di_nest_metal_floor_c.dds` | `Di_nest_metal_floor_8sg_n.dds` | `Di_nest_metal_ov_c.dds` |

The payload's 3,072,340-byte tail is exactly a tag word of 1 plus 384,042
float pairs. The separate
[native tail-reader witness](EQG_TER_TRAILING_CHANNELS.md) establishes the
version-2 tag-1/tag-2 expansion into UV1; this investigation does not infer it
from file length.
Among vertices referenced by these families, all 261,870 CB vertices and
110,561 of 112,073 CBSG vertices have a nonzero UV1 pair. Neither family has
a nonfinite referenced UV1 component in this payload. These are distinct
source vertices per family, not triangle-corner or baked-vertex counts.

## Executed material and upload route

Addresses use `EQGraphicsDX9.dll` at base `0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The probe executes the **whole** material parser `0x10014450` for all 27
resolved definitions, including their authored property records. Renderer
byte `+0xb0ba` is explicitly set to 1 so the parser admits normal textures;
the guard at `0x100154e1` can otherwise skip their resource loads.

| Family suffix | Parser type | Runtime type | Upload layout | TER descriptor | SPL effect index | Declaration selector |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `MaxCB1_2UV` | `0x0f` | `0x10` | 3 | `0x0b` | `0x0b` | 7 |
| `MaxCBSG1_2UV` | `0x10` | `0x11` | 3 | `0x0c` | `0x0c` | 7 |

**Upload layout 3 and declaration selector 7 are different numbering systems.**
Material `+8` is 1 and `+9` is 0 for both families. The family comparisons at
`0x1001961c..0x10019679` select the two-UV branch. Executed selection and
argument construction beginning at `0x1001960c` reach the real call at
`0x10019860` to `0x1008d540`, with layout 3 and the material's runtime type.
The ordinary one-UV uploader is the separate `0x1008cf00` function.

The complete two-UV uploader runs against controlled buffer allocations. It
returns a geometry record preserving layout 3 and runtime type. The native
TER descriptor switch `0x1008fb7c..0x1008fd02` consumes that geometry;
complete descriptor initialization `0x100826d0` supplies the matching effect
and declaration records. The SPL filenames are
`SPL/RegionCB1_2UV.fxo` and `SPL/RegionCBSG1_2UV.fxo`.
Their other effect routes remain separate: CB uses MPL base/light/diffuse
indices 94/99/106; CBSG uses 97/101/106. Those MPL shaders are outside this
investigation.

Declaration initialization `0x100825a0` passes original table `0x10175248`
to a recording device at slot `+0x158`, creating selector 7. Complete binder
`0x10089240` then selects that handle. Its 36-byte vertex is:

| Offset | Original input | D3D declaration | Vertex shader input |
| ---: | --- | --- | --- |
| 0 | Separate position XYZ array | `FLOAT3 POSITION0` | `v0` |
| 12 | Packed normal from attribute `+24/+28/+32` | `D3DCOLOR NORMAL0` | `v1` |
| 16 | Lighting color at attribute `+16` | `D3DCOLOR COLOR0` | `v2` |
| 20 | UV0 at attribute `+0/+4` | `SHORT2 TEXCOORD0` | `v3` |
| 24 | UV1 at attribute `+8/+12` | `SHORT2 TEXCOORD1` | `v4` |
| 28 | First region tangent-frame array | `D3DCOLOR TANGENT0` | `v5` |
| 32 | Second region tangent-frame array | `D3DCOLOR BINORMAL0` | `v6` |

This layout does not consume the separate source color at attribute `+20`.
The `+9` material flag selects variants with an extra color word; it is not
the switch between one and two UV streams.

Instructions `0x1008d971..0x1008d9b9` independently multiply all four UV
components by 256, call conversion helper `0x1010f280`, and store AX.
The witness selects its masked SSE2 route, as in
[the existing UV packing investigation](EQG_TER_UV_PACKING.md).
An asymmetric input `(UV0, UV1) = ((0.125,-0.25),(1.75,-2.5))` produces
signed words `(32,-64,448,-640)`. Another input with UV0 `(128.25,-128.5)`
wraps to `(-32704,32640)`, rather than saturating. Six independently read
original Nest vertices also pass through the same native uploader with exact
expected packed UV words. Other attributes in that six-vertex check remain
constructed witnesses; it is not an original scene upload.

## Active texture binding and integer channel properties

The complete programmable material binder `0x1008a110` consumes the original
parser's property table and texture-set indirection. A recording effect
receives all three texture writes by their exact parameter names. Every CBSG
definition additionally writes float bits `41400000` to `e_fShininess0`.
Texture creation and resolution return controlled resource identities; actual
DDS creation/decoding is outside this execution.

This is also an executed caller relationship. For every original definition,
the probe enters the SPL draw block at `0x1008c812`, resolves its descriptor
and effect wrapper, executes Begin, the native frame/declaration binders, and
the actual call at `0x1008ca96` to the material binder. It stops immediately
after that call at `0x1008ca9b`, before geometry draw. Its texture/float writes
exactly equal the independently called binder. The initial descriptor state
is supplied consistently, the effect interface reports one pass, and no
device technique-selection result is implied.

The integer properties described in
[EQG_MATERIAL_PROPERTIES.md](EQG_MATERIAL_PROPERTIES.md) have a narrower role
than their names might suggest. Synthetic variants of both exact families add:

```text
e_TextureDiffuse0mapChannel
e_TextureNormal0mapChannel
e_TextureSecond0mapChannel
```

The tested value triplets are `(1,1,2)`, reversed `(2,2,1)`, and
`(0,0xffffffff,7)`. Whole material parsing preserves each tag-1 word as runtime
kind 2. All six variants still execute uploader selection as layout 3.
At `0x1008a150..0x1008a166`, the programmable binder handles runtime kinds
0, 1 and 3, while kind 2 branches directly to the next property. It makes
**no effect-parameter lookup or write** for these channel names. Neither
compiled effect contains those names. Thus these retained words do not
remap UVs in the executed route; no conclusion about every material family,
fixed-function path, or external authoring tool is intended.

## Compiled single-pass UV and pixel expressions

Both effect containers parse to exact EOF: CB `0x44b4`, CBSG `0x48b8`.
This section decodes original FXO shader tokens and pass declarations; it is
static shader evidence, separate from the CPU instruction executions above.
The algebra below expresses channel use and factors, not certified GPU
rounding or all legacy shader-register range behavior.

The CB PS1.4 vertex program at `0x3818` uses `v3 / 256` for diffuse and
normal sampling (`0x4404..0x4420`) and `v4 / 256` for the second texture
(`0x43e8`). Its PS1.1 technique has the same mapping. There is no channel
property, slide, or effect-clock term in these expressions.

For CB, write `D = Diffuse(UV0)`, `M = Normal(UV0)`, `S = Second(UV1)`.
Let `B` and `L` denote interpolated base and directional light colors and
`T` the interpolated encoded tangent-space directional vector. The PS1.4
program at `0x3698` and PS1.1 program at `0x28f8` perform:

```text
q = saturate(dot(2*M.rgb - 1, 2*T - 1))
rgb = 2 * D.rgb * S.rgb * (B + q*L)
alpha = 2 * D.a * S.a
```

The normal sample is not normalized in those CB pixel programs. The color
product is explicit at `0x37bc` / `0x2a0c`; the final multiply has the legacy
`_x2` modifier at `0x37ec` / `0x2a3c`. The vertex program supplies the base
color alpha as one. These opaque descriptors request depth writes and do not
request alpha blending or alpha testing; the alpha expression should not be
mistaken for proof of transparent surface rendering.

CBSG's PS2.0 vertex program at `0x3be4` writes `oT0 = v3 / 256` and
`oT1 = v4 / 256` at `0x47f8..0x4808`. Pixel instructions
`0x3a04..0x3a24` sample normal and diffuse at the first pair and second color
at the second pair. Its normal texture has different channel semantics:

- R/G construct `N = normalize((2*M.r-1, 2*M.g-1, 0.9))`.
- B adds a scalar glow term to the base lighting before multiplication by
  both color textures.
- A scales the specular term, using `e_fShininess0` as the exponent.

More precisely, let `H = normalize(t2)`, `P = t3`, `Ld = t5`,
`Cp = v1.rgb`, `Cd = t4.rgb`, and `B = v0.rgb` at the pixel shader.
The vertex program provides a point-light half vector, point-light direction,
directional direction, attenuated point color and weighted directional color
in these respective inputs. With `p = max(dot(N,P),0)`,
`q = saturate(dot(N,Ld))`, and `h = dot(N,H)`, the token sequence is:

```text
spec = saturate(M.a * pow(h, shininess) * p * Cp)
       when h > 0 and dot(N,P) > 0; otherwise zero
rgb = 2 * D.rgb * S.rgb * (B + M.b + p*Cp + q*Cd) + spec
alpha = 2 * D.a * S.a
```

The positive-dot masks, including zero rejection, occur at `0x3a7c` and
`0x3ab0`; the second-color multiply is at `0x3b48`, and final color/alpha
factors at `0x3b98..0x3ba8`. Glow is therefore textured illumination, not a
separate untextured final emission value. Specular strength and glow come
from the **normal** sample; the second texture remains a color multiplier.

Technique fallbacks are material differences, not interchangeable aliases:

| Effect technique | Relevant behavior in its own program/pass |
| --- | --- |
| CB PS1.4 / PS1.1 | Both color textures and three-channel normal dot above |
| CB `NoBump` | UV0/UV1 remain; texture stage 1 uses second texture and `MODULATE2X` |
| CB `NoBumpNoSecondUV` | Only diffuse texture is referenced |
| CBSG PS2.0 | Two color textures plus normal R/G, B glow and A specular |
| CBSG PS1.4 / PS1.1 `NoShineNoGlow` | Two color textures; normal `(2*R-1,2*G-1,1)` without normalization, no B/A glow/specular |
| CBSG PS0 `NoBumpNoShineNoGlow` | Only diffuse is referenced by its program/pass |

The fixed-function fallback descriptions do not establish inherited texture
stage state or complete pixels. Native device admission/technique choice,
MPL lighting, full draw state, filtering/color space and complete GPU output
remain outside this checkpoint.

## Reproduction

```sh
python3 /tmp/openeq-ter-secondary-uv-assets.py
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-secondary-uv-native.py
python3 /tmp/openeq-terrain-effect-audit.py /Users/daeken/EverQuest/RenderEffects/SPL/RegionCB1_2UV.fxo
python3 /tmp/openeq-terrain-shader-audit.py /Users/daeken/EverQuest/RenderEffects/SPL/RegionCB1_2UV.fxo
```

Repeat the last two commands for `RegionCBSG1_2UV.fxo`. Decoded local reports
are `/tmp/openeq-ter-secondary-{cb,cbsg}-{effect,shader}.txt`.
The native probe reuses the initialization/helper prefix before `renderer=alloc`
in `/tmp/openeq-ter-light-binding.py`, SHA-256
`801c7c490d8d319d964fe43c975912d52001cc3c0bcb59f4aa11db86a813aa21`.
It uses x87 control word `037f`, MXCSR `1f80`, controlled allocation and
buffer interfaces, absent optional texture-animation sidecar files, and
recording effect/device interfaces. Original binaries, effect bytes and
proprietary textures are not added to the repository.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-secondary-uv-assets.py` | `6ef07f12c683143e220fc700493b8a38eb4d6756d98915705b83b2ff890f04cb` |
| `/tmp/openeq-ter-secondary-uv-assets.json` | `1656364a7fcf742c0885030ee15389469aa9e8c2e62cd41d2f9908f1e801a39a` |
| `/tmp/openeq-ter-secondary-uv-native.py` | `5541177d3c73793b44905e4c195b34fd5bed13a64d89930962d70a331f6afca8` |
| `/tmp/openeq-ter-secondary-uv-native.json` | `72f4c19d78ac1c50103aee0f13aa464abbde416f7c348a79aa1a3506dbfa9dfb` |
| `/tmp/openeq-terrain-effect-audit.py` | `af65996fa8947fd6277fd20d8ffe9f9e168189ba95fab06b2348f194f3f08a8d` |
| `/tmp/openeq-terrain-shader-audit.py` | `03e9473247e1d4a1cfdf3f0f518e1d525f56e81dd9f536fdb654599de0652007` |
| `SPL/RegionCB1_2UV.fxo` | `e5473824c9c061be2aebc6a599a62e4bbd98ebba143b3cb7c0ea22baa3402907` |
| `SPL/RegionCBSG1_2UV.fxo` | `457ed6a19f93bb7d7184799ef980216088baf7e7caa1446c42534f99e0941943` |
