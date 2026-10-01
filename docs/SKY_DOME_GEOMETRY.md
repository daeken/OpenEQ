# Native sky dome geometry and draw contract

The original default sky dome contains **962 vertices and 1,861 triangle-list
primitives**. Its source-color lookup is not a regular 31-by-30 texture grid:
the main rings wrap their source columns modulo **29**. The bottom fan also
emits one extra triangle. Both behaviors come from executed original
instructions, rather than a proposed replacement mesh.

This extends [the color-table domain research](SKY_COLORMAPS.md) and
[the day-key/light-input research](SKY_LIGHT_COLOR_INPUTS.md). It does not change
the renderer or claim complete native sky pixels. In particular, the current
renderer’s pole-ring suppression is an approximation: the original dome does
consume off-column-zero entries in rows 0 and 29 on rings adjacent to the poles.
Column 31 and rows 30/31 remain outside the original dome’s color domain.

## Executed scope

The original `EQGraphicsDX9.dll` has SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The bounded x86 witness executes these original functions:

| Entry | Proven behavior |
| --- | --- |
| `0x1002f080` | Sphere vertices, indices and source-color indices |
| `0x1002f550` | Existing-buffer diffuse update with unchanged radius |
| `0x1002f630` | Dome draw, matrix arguments, transpose and camera translation |
| `0x1002d0f0`, `0x1002cb80` | Outer sky pass and immediate draw wrapper |
| `0x1009f970`, `0x1009f6f0` | Sky render preset and reset |
| `0x10006830` | Camera construction and its original vtable |
| `0x10006a00`, `0x10005e00` | Camera far/near setters |
| `0x10005ee0`, `0x100069a0` | Radius and camera-position getters |

Allocation and graphics buffer/device interfaces are controlled. Buffer locks
return writable storage; device calls record their arguments. Native render
cache setters and flushes execute. The camera uses its original vtable and
methods, with explicit near distance 1, far distance 800 and position
`[10,-20,30]`. The outer sky call uses its argument that skips secondary
cloud/satellite objects while retaining the dome and render presets.

The imported `D3DXMatrixLookAtRH` numerical implementation is **not executed**.
Its trampoline records the actual input vectors and returns an asymmetric
matrix with entries 1 through 16. The native transpose and translation then
execute, so their behavior is separately testable. No original D3DX runtime,
device rasterization, live host camera or sky clock is claimed by this probe.
Execution uses x87 control word `0x037f` and MXCSR `0x1f80`.

## Radius and storage

The generator reads the camera at engine `+0x34`, through camera virtual slot
`+0x10`. Constructor `0x10006830` installs vtable `0x10133c7c`; this slot points
to `0x10005ee0`, which returns camera `+0x14`. This is the camera far-distance
field: setter `0x10006a00` stores it and recomputes the projection coefficient
`far / (far - near)`. Native frustum construction `0x10006a30` uses the same
field for the far corners. The executed near/far inputs yield coefficient
`1.0012515783309937`.

The two original globals are sectors `0x10170a30 = 31` and steps
`0x10170a34 = 29`. The allocation path at `0x1002f820` computes vertex capacity
`(sectors+1)*(steps+1)+2 = 962`. Relevant dome fields are:

| Offset | Meaning in this slice |
| --- | --- |
| `+0x00`, `+0x04` | Vertex buffer and vertex capacity |
| `+0x08` | Index buffer |
| `+0x10`, `+0x12` | Rebuild flag and enabled flag |
| `+0x18` | Ordinary ring angular margin, `0.10000000149011612f` |
| `+0x1c` | First ring polar angle, `0.009999999776482582f` |
| `+0x48` | Owning sky |
| `+0x4c`, `+0x50` | Emitted index count and vertex count |
| `+0x54` | Array of one source-table word index per vertex |

Vertices have stride 16: three float32 coordinates followed by packed diffuse
color. Indices are unsigned 16-bit values. The generator emits 5,583 indices.
Initial diffuse values are zero except for the final pole’s `0xffff00ff`;
the subsequent color update overwrites every vertex’s diffuse word.

## Vertex positions

There is a positive-Z pole at index 0, then 30 rings of 32 vertices, then a
negative-Z pole at index 961. Ring `r` begins at `1 + 32*r`. Each ring includes
a seam vertex. For radius `R`, the mathematical shape is:

```text
dphi   = float32(2*pi / 31)                  = 0.20268340408802032
dtheta = float32((pi - 2*float32(.1)) / 29)  = 0.10143423080444336

theta(0) = float32(.01)
theta(r) = float32(.1) + (r-1)*dtheta        for r = 1..29
phi(k)   = (31-k)*dphi                       for k = 0..31

vertex(1 + 32*r + k) =
  [R*sin(theta(r))*sin(phi(k)),
   R*sin(theta(r))*cos(phi(k)),
   R*cos(theta(r))]

vertex(0)   = [0,0,R]
vertex(961) = [0,0,-R]
```

This expression describes the geometry; it is not a claim that a language’s
ordinary trig functions reproduce every x87 bit. The angle products and sums
retain x87 precision before `fsin`/`fcos`, with float32 stores at the locations
shown by the original code. At `R=800`, all 960 ring vertices agree with the
independent double-precision expression within `0.000030461` world units.
The exact original-instruction output is retained separately in the witness.

The first ring is near the positive pole at angle `.01`; subsequent rings
start at `.1`. The ordinary ring loop stops after its 29th ring, without
emitting a ring at `pi-.1`. Consequently the two pole caps are asymmetric.
Because float32 `dphi * 31` differs slightly from `2*pi`, seam endpoint
positions are also slightly different. Do not silently regularize these
details in a port intended to match the original mesh.

## Exact topology

These loops independently reproduce all 5,583 emitted indices:

```text
for k = 0..30:
  emit [0, 1+k, 2+k]

for r = 1..29:
  for k = 0..30:
    v = 1 + 32*r + k
    emit [v,   v+1,  v-32]
    emit [v+1, v-31, v-32]

for k = 0..31:
  emit [961, 960-k, 959-k]
```

The final loop contains **32** triangles. Its final triangle is
`[961,929,928]`, which reaches the previous ring’s last seam vertex. At radius
800, the cross-product normal dotted with the triangle center is positive
for 1,860 triangles and negative for this final extra triangle. The latter
has a small nonzero area from seam rounding. This is a native topology quirk,
not a reason to reduce the emitted primitive count to 1,860.

## Exact source-color addressing

Source indices count packed words in the original 32-by-32 table:

```text
source(0) = 0
source(1+k) = max(30-k, 0)                 for k = 0..31
source(1 + 32*r + k) = 32*r + (k % 29)    for r = 1..29, k = 0..31
source(961) = 928                         # row29, column0
```

The first ring therefore uses columns `30,29,...,0,0`. Every later ring uses
columns `0,1,...,28,0,1,2`. The original division at `0x1002f401` uses the
**steps** global 29; it does not use sectors 31. Row advances add 32 at
`0x1002f450`. The first-ring reverse addressing is at `0x1002f26d..0x1002f282`;
the negative pole gets literal `0x3a0` at `0x1002f4bb`.

There are **872 unique source words**: row 0 columns 0 through 30, plus rows
1 through 29 columns 0 through 28. A rectangular “usable size” describes an
outer bound, not the native addressing function. In particular, forcing all
of row 0 or 29 to the pole color changes the original near-pole rings.

The executed updater `0x1002f550` obtains sky manager `+0x13c4` through original
getter `0x10032b60`. It copies `table[source(vertex)]` to every diffuse word,
traversing dome vertex capacity `+0x04`. The asymmetric 1,024-word sentinel
palette verifies all 962 copies, including every alpha bit, and verifies that
all XYZ bytes are unchanged. This case holds radius constant and does not
exercise the updater’s rebuild branch.

## World transform and draw

The dome reads sky virtual slot `+0x54`, which resolves to original getter
`0x100242a0` returning sky `+0x10`. The original time setter writes this axis
as `[0,sin(2*pi*t),cos(2*pi*t)]`, as documented in the light-input research.
The draw supplies `D3DXMatrixLookAtRH` with:

```text
eye = [0,0,0]
at  = sky axis
up  = [0,0,1]
```

It transposes the returned matrix using native `0x1002bc60`, then replaces
matrix elements 12, 13 and 14 with the camera position from virtual slot
`+0x40` (`0x100069a0`, camera `+0xbc..+0xc4`). It sets D3D transform
`0x100` (WORLD). Thus camera translation follows the dome while the sky axis
controls its orientation; camera yaw/pitch are not used to build this world
matrix.

Under the documented nonsingular right-handed look-at convention, the basis
Z direction is `normalize(eye-at)`, so the local positive-Z pole would map to
the **negative** normalized sky axis after this transpose. This is a
consequence of the API convention, not an executed D3DX numerical result.
At exact `t=0`, the supplied axis and up vector are parallel; the imported
routine’s singular-case behavior remains unproven. An exact implementation
must settle that behavior rather than inventing a fallback orientation.

The draw sets FVF `0x42` (`XYZ | DIFFUSE`), binds stream 0 with stride 16 and
issues exactly:

```text
DrawIndexedPrimitive(TRIANGLELIST, baseVertex=0, minVertex=0,
                     vertexCount=962, startIndex=0, primitiveCount=1861)
```

## Culling and render state

The dome itself inherits culling. The outer sky wrapper `0x1002d0f0` selects
preset 8 via `0x1009f970`, which calls `0x1009f6f0(true)`. This sets render
state 22 to **1, D3DCULL_NONE**. Executing the original outer wrapper and cache
flush records that device state before the dome draw. The same preset is
selected by the cubemap-producing path at `0x1002cfce..0x1002cfd6`.

The dome temporarily sets texture-stage-0 COLOROP to SELECTARG2 (3), disables
alpha blending and enables depth writes. After drawing it caches COLOROP
MODULATE (4), alpha blending enabled and depth writes disabled. The outer
wrapper then resets its world transform to identity and applies preset 0,
which caches CULLMODE **3, D3DCULL_CCW**. That final cached cull change is not
flushed to the device within this outer call. These facts do not establish
every inherited texture argument, fog, lighting or depth-test state.

## Reproduction and frozen evidence

Run:

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-sky-dome-geometry.py
```

The probe reuses the initialization/helper prefix of
`/tmp/openeq-ter-light-binding.py` before `renderer=alloc`; that prefix has
SHA-256 `801c7c490d8d319d964fe43c975912d52001cc3c0bcb59f4aa11db86a813aa21`.
The JSON retains all vertices, indices, source-color indices and captured
device calls. Assertions cover the full independent index/color formulas,
all positions within the stated tolerance, diffuse copies, matrix inputs,
transpose, camera translation, primitive counts and native cull transition.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-sky-dome-geometry.py` | `aabc0f34cdbcfa5d69f97dc16c82363f7d95761495118b445319fbbfa092afac` |
| `/tmp/openeq-sky-dome-geometry.json` | `cc9590d5bef8a8e1d774064c8c3593c287ec55f6da7e7793702536fb08b0a74c` |
| Initial emitted vertex bytes (`<fffI`) | `365421c1ef1875397ecb000cc5ef05355d53f30ff0ac24ccf9d156abb15cd6c5` |
| Emitted index bytes (little-endian u16) | `fdf35e7dbae8b7ddcc8b636d20175aae514267d1d2224952b7b62afc7dde3ae4` |
| Source-color index bytes (little-endian u32) | `5ded250ddc5504d8dc2ebdc41837ddedb39858bc357c8825232041a3ae13a6bf` |


## Opt-in CPU geometry diagnostic

`openeq_assets::environment::dome::NativeSkyDome::build` now exposes the proven
mesh in original EQ Z-up coordinates, with supplied positive finite radius,
exact triangle/source-index order and packed AARRGGBB diffuse words. It accepts
only an intact `OriginalDome` 32-by-32 table. It applies no celestial transform,
scene-axis conversion, camera translation or rendering. The live sky remains
unchanged pending its complete draw/color-transfer contract.

Seven tests compare independent native CRCs for every triangle index, every
source index and the synthetic diffuse witness. They preserve the irregular
seam/cap geometry, near-pole colors and alpha bits, reject malformed inputs,
and compare 17 sparse original position snapshots within 0.000062 units at
radius 800. The implementation uses original stored f32 angle constants with
f64 trigonometry and final f32 stores, explicitly without promising x87 bit
identity across platforms.

A standalone cross-check compares all 962 positions and both complete index
streams to the frozen original JSON. On this Apple M4 run all positions are
bit-identical, and the source/index SHA-256 values match the table above.
Four original PoK sampled tables additionally verify 3,848 packed color copies.
The local probe is `/tmp/openeq-sky-dome-port-check.rs`, SHA-256
`9c054d595ce9b92a331403d4030ffff42ed2b83b8a12e63d7ebe61cee43712fe`;
its log is `/tmp/openeq-sky-dome-port-check.log`. No source asset bytes are
repository fixtures.

All **374 assets tests pass**, zero failed/ignored, including original assets.
Strict workspace Clippy, formatting and diff checks pass. A test-expression
precedence lint was corrected without changing the expression's value, and
all seven dome tests passed again. Logs:
`/tmp/openeq-sky-dome-port-{assets,tests-final,clippy-final}.log`.
This is an assets-only follow-up to the 1,148-test complete workspace checkpoint;
it adds no GPU or live-sky parity claim.
