# Native sky dome draw state

The original outer sky pass explicitly disables fog and alpha blending,
enables depth writes, disables culling, and selects texture-stage-0 argument 2
for RGB. The original renderer's fixed reset prefix supplies unlit vertex
diffuse as that argument and enables `LESSEQUAL` depth testing. These are
executed native state changes, not assumed D3D defaults.

Two requested color-space controls remain inherited in both paths:
`D3DSAMP_SRGBTEXTURE` and `D3DRS_SRGBWRITEENABLE`. This witness therefore does
not establish a native framebuffer color-transfer function. It also does not
establish a normal live value for the user clip-plane mask.

This extends [the dome geometry and transform witness](SKY_DOME_GEOMETRY.md).
It does not change production rendering.

## Executed scope

The original DLL is `EQGraphicsDX9.dll`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The harness reuses the frozen geometry witness, including its original camera,
dome mesh, diffuse update and controlled D3DX endpoint. It executes:

| Entry | Behavior |
| --- | --- |
| `0x1002d0f0` | Outer sky pass; preset 8, dome wrapper, world reset, preset 0 |
| `0x1002cb80`, `0x1002f630` | Immediate wrapper and actual dome draw |
| `0x1009f970`, `0x1009f6f0` | Preset selection and shared state changes |
| `0x1009f2d0` | Render-state cache setter |
| `0x1009f350` | Sampler-state cache setter |
| `0x1009f3d0` | Texture-stage-state cache setter |
| `0x1009edb0` | Device-state flush |
| `0x10092050` through stop `0x10092512` | Optional fixed renderer reset prefix |

Synthetic device callbacks retain the state actually submitted by the native
cache flush and capture it at `DrawIndexedPrimitive`. Each case explicitly
seeds 42 render states, nine texture-stage states on each of stages 0–7, and
eight sampler states on each of samplers 0–7: **178 selected states**.
Two distinct sets of prior values test both changed and inherited states.
The native renderer's mip-filter setting is tested both zero and nonzero.
The optional reset is tested with both mip-filter branches and with the
LOD-bias capability bit absent and present, for **eight draw cases** total.

Before the requested seeds, complementary cache values are flushed, ensuring
that zero-valued requested seeds reach the synthetic device as well. The
complementary values are setup only, are never drawn, and are not asserted to
be device-valid. The subsequent seed flush and all selected state snapshots
are checked explicitly. No state is inferred from zero-filled allocation.

The fixed reset prefix is called during device setup at `0x10098a5d`. The
probe stops before `0x10092512`, which begins two runtime renderer-vtable
setting calls, followed by a cache snapshot. This is **not complete native
device creation or a full scene/effect lifecycle**. The outer sky argument is
1, retaining dome/preset behavior while skipping secondary sky objects.
There is no native GPU, fixed-function rasterization, pixel-output or D3DX
numerical claim.

## State forced by the outer sky pass

Every draw case has these values regardless of its prior seeds:

| State | Value at dome draw |
| --- | --- |
| `ZWRITEENABLE` (14) | 1 |
| `SRCBLEND` (19) | 5, `SRCALPHA` |
| `DESTBLEND` (20) | 6, `INVSRCALPHA` |
| `CULLMODE` (22) | 1, `NONE` |
| `ALPHABLENDENABLE` (27) | 0 |
| `FOGENABLE` (28) | 0 |
| Stage 0 `COLOROP` (1) | 3, `SELECTARG2` |
| Stage 0 `ALPHAOP` (4) | 4, `MODULATE` |

The blend factors are installed even though blending is disabled for this
draw. `COLORARG2` itself is inherited by the outer pass. So is the alpha-test
enable flag: the pass alone does not guarantee alpha testing is off.

There is one conditional sampler change. When renderer byte `+0xee5` is
nonzero, preset 8 changes sampler 0 `MIPFILTER` (7) to **1, POINT**, at
`0x1009fccd..0x1009fcd1`. When that byte is zero, the sampler is inherited.
Other selected sampler states are inherited by the outer pass in all cases.

## Explicit fixed reset plus outer pass

The reset prefix explicitly installs the following additional values, which
survive to the dome draw in the combined cases:

| State | Value at dome draw |
| --- | --- |
| `ZENABLE` (7) | 1, enabled |
| `FILLMODE` (8) | 3, `SOLID` |
| `SHADEMODE` (9) | 2, `GOURAUD` |
| `ALPHATESTENABLE` (15) | 0 |
| `ZFUNC` (23) | 4, `LESSEQUAL` |
| `ALPHAREF` (24) | 0 |
| `ALPHAFUNC` (25) | 7, `GREATEREQUAL`; inactive with alpha testing off |
| `DITHERENABLE` (26) | 1 |
| `SPECULARENABLE` (29) | 0 |
| `RANGEFOGENABLE` (48) | 0 |
| `STENCILENABLE` (52) | 0 |
| `TEXTUREFACTOR` (60) | `0xffffffff` |
| `CLIPPING` (136) | 1 |
| `LIGHTING` (137) | 0 |
| `AMBIENT` (139) | 0 |
| Stage 0 `COLORARG1`, `ALPHAARG1` | 2, `TEXTURE` |
| Stage 0 `COLORARG2`, `ALPHAARG2` | 0, `DIFFUSE` |
| Stage 0 `TEXCOORDINDEX` | 0 |
| Stage 0 `RESULTARG` | 1, `CURRENT` |
| Stages 1–7 `COLOROP`, `ALPHAOP` | 1, `DISABLE` |
| Stages 1–7 `COLORARG1`, `ALPHAARG1` | 2, `TEXTURE` |
| Stages 1–7 `COLORARG2`, `ALPHAARG2` | 1, `CURRENT` |
| Stages 1–7 `TEXCOORDINDEX` | Corresponding stage number |
| Samplers 0–7 `ADDRESSU/V/W` | 1, `WRAP` |
| Samplers 0–7 `MAGFILTER` (5), `MINFILTER` (6) | 2, `LINEAR` |

The prefix sets all eight `MIPFILTER` values to POINT when renderer `+0xee5`
is zero, otherwise LINEAR (2). The outer pass conditionally overrides sampler
0 to POINT as described above; stages 1–7 retain the reset's choice. When
renderer capability dword `+0xf4c` has bit `0x2000`, the prefix sets all eight
`MIPMAPLODBIAS` values to float32 **-1**, bits `0xbf800000`. Otherwise those
values remain inherited.

With these explicit reset values, stage-0 RGB selects the mesh's diffuse
color while fixed-function lighting is disabled and later stages are
disabled. The mesh format `XYZ | DIFFUSE` and native diffuse table copies
are established separately in the geometry witness. This identifies the
color input and state configuration; it does not execute their rasterized
or final display result.

## Inherited settings and remaining limits

The following selected states retain distinct prior values even after the
fixed reset prefix and outer pass:

- `SRGBWRITEENABLE` (194) and every sampler's `SRGBTEXTURE` (11).
- `CLIPPLANEENABLE` (152), despite `CLIPPING` being explicitly enabled.
- `COLORVERTEX`, `LOCALVIEWER`, `NORMALIZENORMALS` and the four material-source
  selections. Fixed-function lighting is off in the combined cases.
- `COLORWRITEENABLE`, `BLENDOP`, and the separate-alpha blend enable/factors/
  operation. Alpha blending is off in this draw.
- Fog color, mode, start, end and density; fog itself is off.
- Texture transform flags on stages 0–7 and `RESULTARG` on stages 1–7.

The outer pass alone also inherits every explicit-reset-only item in the
preceding table. In particular, it does not itself disable stage 1, set
`COLORARG2` to diffuse, or establish lighting/depth/alpha-test defaults.

The cache initializer `0x1009f520` fills render, stage and sampler arrays with
an unknown sentinel `0x7fffffff`; it is not evidence for device defaults.
Native effect termination, device creation defaults, state blocks and any
intervening scene draws must be traced separately before claiming that the
combined reset slice describes every live sky draw. The sRGB and clip-plane
values cannot be filled in from this probe.

## State after return

For all eight cases the selected **actual device state** after the outer
wrapper returns equals the state captured at the dome draw. Teardown changes
the pending cache without another selected-state flush in this call:

| Cached state | Value after outer wrapper |
| --- | --- |
| `ALPHATESTENABLE` | 0 |
| `SRCBLEND`, `DESTBLEND` | 2 (`ONE`), 1 (`ZERO`) |
| `CULLMODE` | 3, `CCW` |
| Stage 0 `COLOROP`, `ALPHAOP` | 4 (`MODULATE`), 2 (`SELECTARG1`) |
| Sampler 0 `MIPFILTER`, when renderer `+0xee5` nonzero | 2, `LINEAR` |

Other selected cached values equal their captured draw values. In particular,
the dome's temporary post-draw changes to depth-write/blend enable are
superseded by preset 0. World transform restoration is separate from these
selected state dictionaries and is documented in the geometry witness.

## Reproduction and evidence

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-sky-dome-draw-state.py
```

All eight cases assert all 178 selected states at the draw, the actual device
state after return and the complete selected pending cache after return.
The JSON retains prior values, draw snapshots, changed values, final device/
cache snapshots and native device-call events. The geometry setup is written
to a separate output path so the original frozen geometry evidence remains
untouched.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-sky-dome-draw-state.py` | `09ed4d4da9ed02a97cf81e35b1024c8211aad0bff35fa3e0108ce8511a8f4745` |
| `/tmp/openeq-sky-dome-draw-state.json` | `a8b9d8150941296628f326e5553e4c0fc2346d78711bb160118dac59f1806c2c` |
| Reused `/tmp/openeq-sky-dome-geometry.py` | `aabc0f34cdbcfa5d69f97dc16c82363f7d95761495118b445319fbbfa092afac` |
| `/tmp/openeq-sky-dome-draw-state-setup.json` | `cc9590d5bef8a8e1d774064c8c3593c287ec55f6da7e7793702536fb08b0a74c` |
