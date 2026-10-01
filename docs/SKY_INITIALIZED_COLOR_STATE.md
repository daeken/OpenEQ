# Connected native initialization-to-sky color state

October 1, 2026. A controlled fresh-device path now reaches the original sky
`DrawIndexedPrimitive` call with **framebuffer sRGB writes disabled and all16
sampler sRGB controls disabled**. Both native device-creation and device-reset
suffixes pass. This joins the initialization evidence in
[NATIVE_COLOR_SPACE_STATE.md](NATIVE_COLOR_SPACE_STATE.md) to the sky wrapper
in [SKY_DOME_DRAW_STATE.md](SKY_DOME_DRAW_STATE.md), using the same renderer,
device, cache and captured device-state dictionaries throughout each case.

This is a connected state witness for a deliberately composed dome draw,
not a complete native frame or GPU/display observation. Device defaults are
supplied from the documented D3D9 contract. The normal resource-reload segment
is replaced by controlled dome resources and an explicit native sky invocation;
there are no intervening scene/effect draws.

## Executed connection

Each case begins with explicitly supplied nondefault device values of one for
render state194 (`SRGBWRITEENABLE`) and sampler state11 (`SRGBTEXTURE`) on
samplers0–15. The native cache initializer `0x1009f520` sets its separate unknown
sentinels. The successful controlled COM `CreateDevice` or `Reset` callback
then installs documented zero defaults. State is not inferred from blank memory.

The native caller suffix executes from `0x10099990` (creation) or `0x10099df3`
(reset), through shared setup `0x100987d0`, stopping at `0x10098a62`. The complete
renderer reset `0x10092050` runs, including native gamma computation/submission,
fog setup and cache snapshot. All these original entries are counted and asserted
in each case; the probe inherits the initialization harness's controlled
capability, display-mode, backbuffer and device callbacks.

Without resetting or replacing the renderer, device, cache or state dictionaries,
the probe creates synthetic lockable buffers and executes original sphere
generation `0x1002f080` and diffuse update `0x1002f550`. The asymmetric supplied
palette is copied into every original vertex correctly. These operations make
no device render/sampler-state calls.

The original renderer vtable remains `0x1013de44`, including native radius
getter `0x10088d50`. This replaces the earlier geometry probe's controlled
radius-getter callback with the native getter reading the supplied renderer
radius field. The camera constructor, far/near setters and getters remain native.

The probe then calls full sky wrapper `0x1002d0f0` with argument1, which selects
the existing dome-only path and skips secondary clouds/satellites. It executes
preset8, `0x1002cb80`, dome draw `0x1002f630`, native cache flush and preset0
cleanup. The device callback records the actual call at `0x1002f7cd`, with return
address `0x1002f7cf`:

```text
DrawIndexedPrimitive(TRIANGLELIST, baseVertex=0, minVertex=0,
                     vertexCount=962, startIndex=0, primitiveCount=1861)
```

The imported D3DX LookAtRH numerical result remains a supplied asymmetric matrix,
while its native arguments, transpose and eye translation execute. That boundary
does not mutate render/sampler state, and no GPU matrix/pixel claim is made.

## Captured state at the draw

Both cases capture144 device/math events and the same relevant state:

| State | At the original sky draw |
| --- | --- |
| `SRGBWRITEENABLE` | 0 |
| `SRGBTEXTURE`, samplers0–15 | 0 on every sampler |
| Corresponding native cache entries | Still unknown `0x7fffffff` |
| Lighting / ambient | Disabled / zero |
| Stage0 color operation / argument2 | SELECTARG2 / DIFFUSE |
| Fog / alpha blending / alpha test | Disabled |
| Depth test / comparison / writes | Enabled / LESSEQUAL / enabled |
| Culling | NONE |

No sRGB render/sampler-state write occurs between the supplied device default
transition and the captured draw. The unknown cache entries are intentional:
the observable zero comes from the explicit fresh-device contract and remains
in the captured device state, rather than being fabricated from a cache value.

This dome uses vertex diffuse color rather than a sampled texture. Sampler sRGB
state is captured for completeness; framebuffer sRGB state directly concerns
this pass's color output. The native wrapper subsequently restores CCW culling
in the pending cache. It does not retroactively change the captured draw.

## Remaining boundaries

The witness does not execute a real D3D9 driver, full device creation/reset,
ordinary resource/effect reloading, other scene draws, clouds, satellites,
D3DX's matrix implementation, GPU rasterization, swap-chain presentation of the
sky, OS gamma application or a physical display. The gamma ramp is computed and
captured during initialization but never applied to a display. Controlled zero
capabilities cause the expected native “Mipmapping unavailable” diagnostic; the
probe records these diagnostics instead of silently treating them as GPU support.

Thus a comparison explicitly beginning from documented fresh-device defaults may
use disabled sRGB controls for this dome. This result does not establish every
later live frame's color state or justify claiming display-transfer equivalence.
The existing distinct-prior-state sky tests remain relevant: the sky path itself
inherits these controls and does not force zero after arbitrary prior mutations.

## Frozen reproduction

Run `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-sky-initialized-color-state.py`. The probe verifies both parent
harness hashes and reuses only selected setup sections; it never executes their
output writers. Earlier initialization, geometry and draw-state JSON hashes
remain unchanged.

| Local evidence | SHA-256 |
| --- | --- |
| `/tmp/openeq-sky-initialized-color-state.py` | `fe811aadb48a11d1448da01fb7595c8941516ef58c021e2ce6dc740dfe5fcc19` |
| `/tmp/openeq-sky-initialized-color-state.json` | `32db321e97829899855025bd84d39f6bec076e40c7b8623fc508932fe1212219` |
| `/tmp/openeq-sky-initialized-color-state.log` | `60fd8dec13c0857329e07df3f264e81d72442369baffffa5bbf3ccfa5098ea1a` |

Original DLL SHA-256 remains
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
No production rendering change is included.
