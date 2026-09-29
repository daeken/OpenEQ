# Native terrain material checkpoint

2026-09-29. Static analysis of the installed original client establishes that
ECO blend and layering images participate in CPU preprocessing, while
the audited terrain pixel shader samples a tile color image, an RGB detail
mask, three detail images and three RGB normal images. This is a bounded research
checkpoint, not a recovered end-to-end terrain renderer. It does not change the
compatibility blend or asset interface in [GPU_TERRAIN_PLAN.md](GPU_TERRAIN_PLAN.md).
No original client code was executed and no proprietary binary payload is
included here.

## Inputs and reproduction

| Original file | Bytes | SHA-256 |
| --- | ---: | --- |
| `EQGraphicsDX9.dll` | 1,615,360 | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| `RenderEffects/SPL/Terrain_Bump3Detail.fxo` | 22,632 | `795c1076d7fadb69800dfec1fa675053aa54e3fc2a5c21ff3bf286c3f2f439cc` |

Both paths are beneath `/Users/daeken/EverQuest`. DLL addresses below are
**preferred virtual addresses**, with image base `0x10000000`; effect offsets
are **file byte offsets**. The DLL's `.text` starts at VA `0x10001000`, file
offset `0x400`; `.rdata` starts at VA `0x10133000`, offset `0x131600`; `.data`
starts at VA `0x1015f000`, offset `0x15d000`. Use the correct section mapping
when locating RTTI strings in `.data`.

```sh
objdump --disassemble --x86-asm-syntax=intel --no-show-raw-insn \
  --start-address=0x10089960 --stop-address=0x10089e00 \
  /Users/daeken/EverQuest/EQGraphicsDX9.dll
```

The effect uses D3DX effect format `0xfeff0901`. A static reader following the
Wine D3DX9 effect/token descriptions walked its parameters, techniques, pass
states and resources, reaching exact EOF `0x5868`. The shader reader checked
instruction lengths and constant tables. Temporary research readers and reports
were `/tmp/openeq-terrain-{effect,shader}-audit.py` and
`/tmp/openeq-terrain-bump3-{effect,shaders}.txt`; OpenEQ does not depend on these
files. The original effect plus the offsets below are the reproducible evidence.

## ECO fields and their native consumers

The texture-layer text reader is `0x100f0360`, called by the texture-part reader
at `0x100f0b7f`. The latter appends layers in declaration order. Relative native
layer offsets and assignment sites are:

| ECO field | Native field | Evidence VA |
| --- | --- | --- |
| `COVERMAP`, alias `COLORMAP` | filename `+0x20`, image `+0x24` | shared assignment `0x100f09f2..0x100f0a0b`; image stored at `0x100f02ef`/`0x100f0d3a` |
| `BLENDMAP` | filename `+0x28`, image `+0x2c` | `0x100f06b3..0x100f0702` |
| `BLENDSOFTNESS` | integer `+0x5c` | `0x100f073c` |
| `LAYERINGMAP`, alias `COVERAGEMAP` | filename `+0x50`, image `+0x54` | shared assignment `0x100f09d4..0x100f09ed` |
| `LAYERINGAREA` | integer `+0x58` | `0x100f07e9..0x100f07f2` |
| `DETAILMAP` | filename `+0x30`, image getter reads `+0x34` | `0x100f083d..0x100f0855`; getter `0x100ee150` |
| `DETAILREPEAT` | integer `+0x48` | `0x100f0889..0x100f0892` |
| `NORMALMAP` | filename `+0x3c`, image getter reads `+0x40` | `0x100f08d9..0x100f08f5`; getter `0x100ee160` |
| `NORMALREPEAT` | integer `+0x4c` | `0x100f092c..0x100f0935` |
| Height min/max/tolerance | `+0x60/+0x64/+0x68` | consumed by CPU mask generation |
| Slope min/max/tolerance | `+0x6c/+0x70/+0x74` | consumed by CPU mask generation |

The native saver uses `%i` for detail/normal repeats, layering area and softness
(format strings at `0x10143cdc`, `0x10143cc8`, `0x10143d04`, `0x10143bfc`).
Setter methods `0x100ee090` and `0x100ee0b0` clamp the respective repeat integer
to 0–100; this does not by itself establish that the text parser invokes those
setters. A child-layer list exists at `+0x78`; OpenEQ's current ECO parser does
not reproduce that hierarchy.

RTTI identifies `CEQEcoTextureLayer`'s vtable at `0x1013fe14` and the base
`CEcoTextureLayer` vtable at `0x10143ab4`. Their complete-object locators are
`0x10151b04` and `0x10153d90`; derived constructor references include
`0x100a0440` and `0x100a04af`. These provide a route for further tracing without
relying on objdump's generic export-relative function labels.

### Blend and layering maps are active preprocessing inputs

CPU function `0x100ef030` selects/clears a per-layer mask using the layer index
at `+0x14`, evaluates parent/child masks and height/slope inputs, and recursively
processes child layers at `0x100ef6e7`. Its caller `0x100ef770` invokes it at
`0x100efdab`; terrain texture generation calls that wrapper at `0x100f4d80`.

* `0x100ef0ec..0x100ef10d` tests/reads the blend image at `+0x2c`, excluding
  the `default.bmp` sentinel at `0x10143568`.
* `0x100ef119..0x100ef138` reads softness and forms
  `(100 - softness) * 0.001` as an intermediate value. This is **not** a
  complete opacity formula.
* `0x100ef121..0x100ef140` reads layering area and multiplies it by the output
  mask dimension. `0x100ef161..0x100ef1e6` checks the layering image, excludes
  `default.bmp` and chooses source image data. Later sampling/interpolation is
  visible at `0x100ef37b..0x100ef4ab`.
* `0x100ef4ad..0x100ef560` applies blend/softness operations using byte values
  and constants including 0.02 and 0.98. Parent-mask multiplication follows at
  `0x100ef560..0x100ef58b`, then opacity accumulation at
  `0x100ef58b..0x100ef610`.

This disproves treating blend/layering/softness as unused labels or independent samplers
in the effect below. The complete CPU mask equation and the channel-packing
bridge to that effect's RGB detail mask remain untraced. In particular, a DAT
ecosystem application mask is not established to be the shader's RGB mask.
Coverage-image loading is established above, but its complete composition into
the tile color image has not been traced in this checkpoint.

## Material-to-effect bindings

Function `0x10089960` finds effect parameters by semantic (effect vtable
`+0x28`), sets textures (`+0xd0`) and sets scalar floats (`+0x78`). With EBP
pointing at its material record:

| Semantic | Source | Native bind evidence |
| --- | --- | --- |
| `ColorMap` | material `+0x24`, texture handle `+0x08` | `0x10089981..0x10089997` |
| `DetailMaskMap` | material `+0x24`, texture handle `+0x10` | `0x100899ae..0x100899c4` |
| `DetailColorMap0` | first descriptor from material `+0x28`, handle `+0x08` | `0x100899db..0x100899f8` |
| `DetailNormalMap0` | same descriptor, handle `+0x10` | `0x10089a1f..0x10089a3c` |
| `DetailScale0` | descriptor `+0x04`, converted unsigned integer to float | `0x10089a61..0x10089a93` |

Layers 1/2 have analogous bindings later in the function. This is positive
evidence that normal-map resources are bound; a filename alone would not prove
use. The complete path from ECO `NORMALREPEAT` to descriptor values has **not**
been established. Do not infer a second runtime normal scale from its existence
in the ECO grammar.

## Compiled three-detail bump path

The audited effect includes four techniques. Only
`TerrainBump3Detail_DX9_VS1_PS20` was fully analyzed here. The other names
explicitly identify PS1.4 no-bump, PS1.1 no-bump/two-layer and DX6
no-bump/one-layer paths. Native technique selection is still an open question;
the presence of PS2 bytecode does not prove that every client uses it.

The PS2 pixel shader starts at file offset `0x45a4` (1,184 bytes), and its
paired VS1.1 shader starts at `0x4a5c` (3,192 bytes). PS constant/sampler table:

| Register | Meaning |
| --- | --- |
| `c0` | `a_v4ColorPointLight0` |
| `s0` | `s_SamplerDiffuse0` / `ColorMap` |
| `s1` | `s_SamplerDetailMask0` / `DetailMaskMap` |
| `s2/s3/s4` | Detail color 0/1/2 |
| `s5/s6/s7` | Detail normal 0/1/2 |
| literal `c1` | `(2, -1, 1, 0)` |

At `0x4874..0x48e4`, the shader samples mask and color at `t0`, and corresponding
detail/normal pairs at **identical** coordinates `t1`, `t2`, `t3`. VS instructions
at `0x5234..0x52dc` set `t0 = inputTexcoord0 / 256`, then those three coordinates
to `(inputTexcoord1 / 256) * DetailScale0/1/2`. There is no independent normal
repeat or normal-strength effect parameter in this effect.

Using mask RGB as `m0,m1,m2`, sampled detail RGB as `d0,d1,d2` and sampled normal
RGB as `n0,n1,n2`, its pixel RGB arithmetic is:

```text
n = m0 * (2*n0 - 1) + m1 * (2*n1 - 1) + m2 * (2*n2 - 1)
point0 = saturate(dot(n, t4)) * PointLight0Color
attenuation0 = 1 - saturate(dot(t5, t5))
lighting = vertexColor0 + point0 * attenuation0
detail = m0*d0 + m1*d1 + m2*d2
rgb = ColorMap.rgb * detail * vertexColor1.rgb * 2 * lighting
```

Normal decode occurs at `0x48f4..0x491c`; normal weighting/dot at
`0x4930..0x4968`; attenuation/lighting at `0x4978..0x49a8`; detail/color multiply
at `0x49bc..0x4a24`. The shader does **not** renormalize the weighted normal
before the dot, reconstruct Z from two channels, use alpha/green normal packing,
or explicitly invert green. Mask sums are not normalized here either.

The normal-map contribution in this path affects **point light 0**. Other
directional/bounce and point-light terms arrive through `vertexColor0`, computed
from the geometric normal in the VS around `0x5468..0x5594`. The VS decodes the
geometric input normal from `[0,1]` at `0x5448`, builds a geometry-derived basis
and projects the point-light-0 direction into it at `0x55a8..0x56b4`. Exact basis
signs and correspondence to OpenEQ world axes still need a separate trace of
vertex packing; the RGB decode is not sufficient to guess that transform.

All nine declared samplers set minification, magnification and mip filtering
to numeric 2 (linear). Only detail sampler 2 explicitly assigns address U/V
numeric 1 (wrap). Other address modes and any state inherited from outside this
effect remain unresolved. No explicit sRGB sampler state was found in this
effect; that does not prove the state of the surrounding renderer.

## Original asset witness

`oldcommons.eqg`, `base.eco`, first layer `grass` declares:

```text
COVERMAP coverage_lightd.dds
BLENDMAP default.bmp
BLENDSOFTNESS 0
LAYERINGMAP blend-grayscale.bmp
LAYERINGAREA 16
DETAILMAP grass_sand.dds
NORMALMAP Di_ro_sand_grass2n.dds
DETAILREPEAT 10
NORMALREPEAT 10
```

Its height bounds/tolerance are `-10000, 10000, 1`; slope bounds/tolerance are
`0, 90, 1`. In the same archive, `trail.eco` uses `coverage_desertb.dds`, the
nondefault blend image `grassyblend.bmp`, softness 50, default layering image,
detail `Di_ro_sand_loose.dds` and normal `Di_ro_sand_loose_n.dds`, both repeats 10.

`di_ro_sand_grass2n.dds` is 262,272 bytes, SHA-256
`a2a6548f71eb59091e461d261c223665b0a5718525cf2ac6dadcb208daa48fe6`.
It is an uncompressed 256×256 32-bit ARGB DDS, flags `0x41`, masks
R=`0x00ff0000`, G=`0x0000ff00`, B=`0x000000ff`, A=`0xff000000`.
Base-level channel ranges are R 44–204, G 49–202, B 224–255 and A exactly 255.
Its first three decoded RGBA pixels are `(144,104,252,255)`, `(122,107,253,255)`
and `(86,115,247,255)`. This is an original RGB normal witness consistent with
the shader's three-channel decode, not an assumption based on a `_n` suffix.

`grassyblend.bmp` is present (66,616 bytes, SHA-256
`839ed7564576d0950a18573847d5c8b09c16c83b2c3b150226ad0dc146dd5f5f`).
`blend-grayscale.bmp` and `default.bmp` are absent from this archive. Shared or
loose resolution of the nondefault image was not audited; the native sentinel
check is the evidence for special treatment of `default.bmp`.

## Safe implementation boundary

Native fidelity needs the remaining CPU generation and material-packing trace
before adding coverage/softness/layering behavior to OpenEQ. Normal maps have a
proven RGB decode and shared detail UVs in the specific compiled path, but
world-basis orientation, actual effect selection and independent normal-repeat
handling remain unresolved. No speculative blend formula, strength, green flip
or sampler state should be promoted into runtime behavior from this checkpoint.
The existing direct-texture compatibility renderer can proceed independently;
deferred fallback painting is likewise independent of these unresolved native
shading semantics.
