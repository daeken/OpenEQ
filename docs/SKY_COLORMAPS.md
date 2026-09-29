# Original sky color tables

The original `Resources/sky/colormap-*.dds` images are **32×32 vertex-color tables**,
not ordinary dome textures or cubemap faces. A subset of their texels colors the
sky mesh; other entries must never be sampled as sky. Their raw contents are
preserved by the asset loader.

`SkyAssets.color_map_layout` and `cloud_color_map_layout` explicitly distinguish:

- `SkyColorMapLayout::FullTexture`: a generic/synthetic lookup; all texels usable,
  no declared pole entries. This is the enum's default.
- `SkyColorMapLayout::OriginalDome`: requires a 32×32 source, usable rectangle
  **X 0..=30, Y 0..=29**, and native pole colors at **[0,0] and [0,29]**.

`usable_size(&texture)` returns `[31,30]` for an original table;
`pole_texels()` returns its two pole source positions. The loader rejects a native
colormap of any other dimensions before assigning this layout. Cloud sprites are
ordinary textures; only cloud *color maps* use this table convention. Cloud maps
are validated separately, and an absent map keeps the `FullTexture` default.

The renderer makes a separate 31×30 GPU lookup of the usable domain, without
changing the source bytes, and uses texel-center bounds for filtered sampling.
When adapting this table to a continuous spherical lookup, each native pole must
have one color: its column-zero source entry. Keeping azimuth-dependent colors
around a pole produces a pinwheel as the camera looks directly at it.

GPU regressions poison every excluded texel and unused pole-ring entry with
magenta, then render 15 pitch/yaw combinations with and without cloud tinting.
No poisoned color reaches a pixel. An original PoK sky regression covers 12
upward views; a full-zone capture at scene `[15,1455,-120]`, pitch 85°, confirms
the reported neon wedge and central pinwheel disappear. Ordinary texture lookups,
camera-translation invariance, zone fog and underwater entry/exit also pass.

## Plane of Knowledge rainbow diagnosis

PoK has no sky override in the installed `Resources/sky/sky.ini`. It inherits
`SkySetting-default -> DefaultWeather=DefaultClear`. At day fraction `.5`,
`weather.ini` selects `ColorSet-DefaultClear -> DefaultClearDay -> File=DefaultDay`.
The resulting `colormap-defaultday.dds` is 32×32, uncompressed 32-bit BGRA with the
usual RGB masks. Its decoder is reading the original pixels correctly.

The rightmost column contains highly saturated non-dome entries. In particular,
source pixel **[31,23] is RGBA [0,255,30,255]**, and nearby entries include bright
red, white and black. Sampling the full 0..1 texture width spreads those entries
into the sky as a neon wedge. At [0,0] the source is [206,209,233,0], and at [0,29]
it is [204,208,233,0]. Source alpha is preserved; these are RGB color-table values,
not a command to make the sky transparent.

The bottom two rows also have other values. They are excluded because the native
mesh never indexes them, not because their colors merely look unusual. The
original-asset test records the green entry and domain/pole values to distinguish
correct decoding from correct interpretation. A synthetic regression also
checks that layout tagging leaves all source RGBA bytes intact and generic
textures retain their full domain.

## Evidence from the installed original client

Read-only inspection of `EQGraphicsDX9.dll` established the domain directly. The
addresses below are virtual addresses for this 32-bit client build (image base
`0x10000000`), not portable symbols:

- `0x1002dc90` loads `Resources\\Sky\\ColorMap-%s.dds`. At `0x1002dd4e` it checks
  both dimensions against 32. The loop at `0x1002dd95` copies all 1,024 color words
  unchanged. Error text explicitly says the map must be 32×32.
- `0x1002f080` constructs the sky sphere. Its sector/step constants at
  `0x10170a30` and `0x10170a34` are 31 and 29.
- The first pole receives color-table index zero (`0x1002f19c`); the opposite pole
  receives `0x3a0 = 29*32` (`0x1002f4bb`). Ring color indices advance by a row
  stride of 32 (`0x1002f450`). Dome indices do not use column 31 or rows 30/31.
- `0x1002f550` updates the actual vertex diffuse colors by indexing that CPU
  table (`0x1002f600`), instead of binding the whole DDS as a sky texture.

The original mapping also includes a celestial orientation that is **not yet
ported in full**. At `0x1002f630`, the client builds a `D3DXMatrixLookAtRH` rotation
with eye zero, the direction obtained from `CSky+0x10`, and world up `[0,0,1]`,
then transposes it. The direction setter at `0x100c2c40` builds
`[0, sin(2*pi*t), cos(2*pi*t)]`. The map's pole axis therefore follows the original
sky orientation; its two poles are not intrinsically the world zenith/nadir.
Determining the exact time parameter, interpolation and complete native dome
construction remains follow-up work.

OpenEQ's angular sky lookup is a simplified presentation. Restricting its lookup
to the proven domain and maintaining continuous poles fixes the visible edge
swatches/pinwheel; it does not claim to reproduce the entire native sky system.
Sun/moon satellites, transitions and multiple cloud populations remain separate
features. A map that looks blue in its middle is not sufficient evidence to
relabel that row as the original zenith.

Inspected source SHA-256:

- `EQGraphicsDX9.dll`:
  `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`
- `colormap-defaultday.dds`:
  `a2356f59a1003f6c1f050268bdac6e5ae4a7e1e40754fed67820a476b95ffcbc`

No original binary, texture, or disassembly is checked into the repository.
