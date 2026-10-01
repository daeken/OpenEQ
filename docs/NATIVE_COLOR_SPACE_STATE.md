# Native color-space initialization and remaining frame boundary

2026-10-01. The native device-creation/reset path is consistent with **disabled
texture sRGB decoding and disabled framebuffer sRGB encoding**. Direct3D 9
documents zero defaults for both controls, and the executed client setup does
not change them. All 148 installed compiled effects also omit both controls.
The client separately computes and installs a display gamma ramp.

This sharpens the inherited-state boundary in
[SKY_DOME_DRAW_STATE.md](SKY_DOME_DRAW_STATE.md),
[EQG_WATERFALL_UPLOAD.md](EQG_WATERFALL_UPLOAD.md) and the terrain lighting notes.
It does **not** prove every live frame's final color transfer: the D3D device,
complete effect/scene lifecycle, OS gamma application and display are not
executed. Production rendering is unchanged.

## API defaults and native creation/reset context

Microsoft's [D3DRENDERSTATETYPE documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3drenderstatetype)
states that `D3DRS_SRGBWRITEENABLE` (194) defaults to zero. Its
[D3DSAMPLERSTATETYPE documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dsamplerstatetype)
states that `D3DSAMP_SRGBTEXTURE` (11) defaults to zero, with no gamma correction
on sampling. The [Reset contract](https://learn.microsoft.com/en-us/windows/win32/api/d3d9/nf-d3d9-idirect3ddevice9-reset)
states that all state information is lost on reset. These are external API
contracts, not observations of a native driver in this witness.

The graphics DLL has SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
All following addresses use image base `0x10000000`.

The native caller suffix beginning at `0x10099990` selects device flags and
calls `IDirect3D9::CreateDevice` through slot `+0x40` at `0x100999e1`, writing
the resulting device into renderer `+0xf08`. On the executed successful branch,
it calls shared setup `0x100987d0` at `0x10099b10`.

The reset suffix beginning at `0x10099df3` calls `IDirect3DDevice9::Reset`
through device slot `+0x40` at `0x10099e06`. Success reaches the same shared
setup at `0x10099e21`. That setup queries display/capability/backbuffer data,
performs initial clear/present/viewport work, and calls `0x10092050` at
`0x10098a5d`.

Both suffixes execute through `0x10098a62`. The complete `0x10092050` routine
executes, including its previously unexecuted tail:

- Renderer vtable `0x1013de44`, slot `+0x7c`, calls native gamma setter
  `0x10091e00`.
- Slot `+0x78` calls native fog setter `0x10097010`.
- Tail call `0x1009e9d0` snapshots the native state cache.

The controlled `CreateDevice`/`Reset` endpoints explicitly install the documented
zero sRGB defaults, after first giving the old synthetic device values of one.
This makes the supplied default transition visible and avoids inferring device
state from zero-initialized memory. Neither suffix subsequently issues an sRGB
state write. Both finish with framebuffer state 194 and sampler state 11 on
samplers 0–15 equal to zero. The corresponding fresh cache values remain the
unknown sentinel `0x7fffffff`, rather than becoming inferred device defaults.

This is a bounded initialization path: execution stops before subsequent
resource/effect loading. Error/retry branches, complete native device creation,
device reset implementation and later scene draws are outside it.

## Complete renderer reset still inherits sRGB

Two additional cases seed actual controlled device state through the native
cache setters and flush: one has both controls zero, the other one. They then
execute **all** of `0x10092050` and flush again. Both preserve their prior
framebuffer value and all 16 sampler values.

Thus the complete renderer reset, not merely its fixed prefix, leaves these
controls inherited. The distinction matters: a fresh D3D device supplies the
documented defaults, while calling the renderer reset routine on an existing
device does not force those defaults back into place.

## Native writers and compiled-effect inventory

A direct-call inventory finds 198 calls to render-state cache setter
`0x1009f2d0` and 49 calls to sampler-state setter `0x1009f350`. Backward
control-flow/stack tracing resolves the state argument at 245 sites to literal
values. None selects render state 194 or sampler state 11. The remaining two
sites are generic effect-state forwarding callbacks:

| Entry | Behavior |
| --- | --- |
| `0x10088940` | Forwards supplied render-state ID/value to `0x1009f2d0` |
| `0x100889a0` | Forwards supplied sampler/state/value to `0x1009f350` |

Their native state-manager object is constructed at `0x100871e0`, using vtable
`0x1013dc84` and the cache pointer at object `+8`. Both callbacks execute in
the witness with explicitly supplied sRGB values zero and one. The real cache
flush forwards them unchanged to controlled device calls. They **can** alter
sRGB settings; the cache does not filter or ignore those state IDs.

The static compiled-effect inventory parses all 148 `.fxo` files beneath the
installed `RenderEffects` directory to exact EOF. It examines 1,732 sampler
state declarations and 1,552 pass state declarations. None explicitly authors
`SRGBTexture` or `D3DRS_SRGBWRITEENABLE`. This includes `SPL/RegionCB1.fxo`,
`SPL/RegionCB1_2UV.fxo`, `SPL/RegionWater.fxo`, `SPL/RegionWaterFall.fxo`,
`SPL/SModelWater.fxo` and `SPL/SModelWaterFall.fxo`.

These are bounded source findings. The direct-call audit is not a proof against
indirect calls, memory/table-based state changes, state restoration, other
modules, dynamically supplied effects or direct device calls outside those
cache setters. The effect inventory does not execute D3DX's state application
or state-block behavior. Absence of an explicit declaration is not a complete
frame-state observation.

## Separate display gamma ramp

Renderer construction stores float32 1 at `+0x1058` (`0x100874a7`). The full
reset passes the stored value to gamma setter `0x10091e00`. Its original loop
constructs matching 256-entry red, green and blue ramps, approximately:

```text
exponent = float32(1 / gamma)
ramp[i] = uint16(truncate(pow(i / 256, exponent) * 65535))
```

The original constants are double `1/256` at `0x1013e948` and float32 `65535`
at `0x1013e944`. Native power/conversion instructions execute in this witness;
the formula above is descriptive, not a separately proven replacement for
every possible gamma value.

With renderer byte `+0xea4` set, native virtual dispatch reaches `0x10091f50`
and `IDirect3DDevice9::SetGammaRamp` at device slot `+0x54`. The witness captures
the exact submitted ramps without applying them to the OS/display. Capability
word renderer `+0xf34`, bit `0x100000`, selects flags zero when set, one when
clear. Both branches execute. Other routing branches can use Win32 display
gamma APIs; those branches are statically observed but not executed here.

| Gamma input | Ramp at indices 0, 1, 64, 128, 192, 255 |
| ---: | --- |
| 1.0 | 0, 255, 16383, 32767, 49151, 65279 |
| 1.5 | 0, 1625, 26007, 41284, 54097, 65364 |
| 2.2 | 0, 5270, 34898, 47823, 57502, 65418 |

Even gamma 1 does not submit an exact full-range identity ramp: its last entry
is 65279, because the sample divisor is 256. This gamma ramp is a separate
display operation, not evidence that texture sampling or render-target writes
use the sRGB transfer curve. The live user's gamma setting and actual platform
application of the ramp remain unobserved.

## Consequence for sky and lighting work

A controlled native comparison beginning from documented device defaults can
now explicitly use disabled sRGB sampler/framebuffer state. That choice has
initialization, ordinary native writer and installed effect-corpus support.
The existing modern linear/sRGB renderer should still describe its color
transfer as an approximation until a connected scene/effect draw-state witness
and display-gamma boundary are addressed. A screenshot cannot distinguish
shader lighting changes from display gamma without those settings.

## Frozen reproduction

Run the probes with `PYTHONPATH=/tmp/openeq-re-tools`. Original files remain
outside the repository; JSON contains hashes, metadata and captured calls.
The initialization probe executes two creation/reset suffix cases, two full
renderer resets with distinct seeds, two effect callback cases and three
additional gamma cases. All assertions pass.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-srgb-initialization.py` | `04e1b9b6f64358dca02434aaaa6fd42651502778ea3be43c3b1d4cd42ecd15da` |
| `/tmp/openeq-srgb-initialization.json` | `a96161b4103c190a0009248ece0f494dd336941ba3b16e743cfa7c1fa14a4bd6` |
| `/tmp/openeq-srgb-initialization.log` | `1ebafebdd98ef72bf10e76aa36aae585009000f72b3ce40890711b03564391e6` |
| `/tmp/openeq-color-state-static.py` | `830c98198a0c0af61deadd6d8c489559287570b2685cd905765e334ce75c1694` |
| `/tmp/openeq-color-state-static.json` | `cabbc36b078b74b249bebd002d26ca2fff633e69db2f5697fabc420e30182435` |
| `/tmp/openeq-srgb-cfg-arguments.py` | `9db53c94fb192cb3e913cc1bbfc5f2b886bc8507aeae1cb7bb3e059b71a4fd3e` |
| `/tmp/openeq-srgb-cfg-arguments.json` | `78c72d035d782d6e7c334f1da86f37678e8fe0958cebba53927c571a20e87995` |

The native probe verifies the shared emulator prefix described in
`EQG_MATERIAL_PARAMETER_BINDING.md`. The effect-state-name mapping comes from
local Wine reference `/tmp/openeq-effect.c`, SHA-256
`bbdd73f91d07565b7070f9de895ed252dc8e1af2959a07d5b1715d208ce612ca`.
