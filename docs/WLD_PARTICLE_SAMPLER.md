# Bounded WLD particle CPU sampler

Checkpoint: 2026-10-01. `openeq_render::wld_particles` reproduces the recovered
CPU behavior of four exact Plane of Knowledge particle definitions. It does
not register emitters in the scene, upload textures, or draw gameplay particles.
The source bodies, runtime investigation, and placement evidence are recorded
in [WLD_PARTICLE_RUNTIME.md](WLD_PARTICLE_RUNTIME.md) and
[WLD_PARTICLE_PLACEMENT.md](WLD_PARTICLE_PLACEMENT.md).

## Admission and API

`PokDefinition::from_cloud` compares all twenty authored fixed words against
the four witnessed bodies, requires a texture reference, and rejects optional
vectors, optional blocks, and trailing bytes. It does not match by resource
name or emulate the original same-name cache. The explicit enum is also useful
for diagnostics which have already selected a witnessed body.

| Definition | Source name | Capacity | Source lifetime, ms | Source size | RGB |
| --- | --- | ---: | ---: | ---: | --- |
| `Smoke` | `CSMOKE_PCD` | 50 | 3000 | 1 | 100,100,100 |
| `Flame301` | `L301_PCD` | 40 | 650 | 1 | 255,255,255 |
| `Flame308` | `L308_PCD` | 10 | 750 | 0.25 | 255,255,255 |
| `Flame500` | `L500_PCD` | 40 | 650 | 1.2 | 255,255,255 |

Lifetime conversion retains the native float value of 0.001 before multiplying;
for example, the native 650 ms lifetime is `0.6500000357627869` seconds.
Source color high bytes are not alpha. Full texture-chain resolution is a
separate loader concern; see
[WLD_PARTICLE_TEXTURE_METADATA.md](WLD_PARTICLE_TEXTURE_METADATA.md).

`Sampler::new(definition, captured_context)` creates a fixed-capacity emitter.
`Sampler::update` takes a `FrameInput`, a mutable `DiagnosticRandom`, and a
per-particle drawing predicate. It returns ordered `ParticleSample` candidates
with native-world position, full square edge size, RGBA, and a drawing flag.
Read-only particle, total-spawn, and remaining-duration accessors support
inspection. The caller should share a random stream across emitters in an
explicit update order.

The sampler stores at most 50 particles, attempts no more births than its free
capacity, and emits at most 50 candidates per call. Random disk rejection has
a bounded full-table search. A delta is finite and nonnegative, capped at one
second. There are no implicit substeps or catch-up calls. Camera coordinates
must be finite and owner alpha must be in `[0,1]`. Invalid frame input is
rejected before sampler state or random counters change.

## Coordinates and owner state

`OwnerPose::from_native_world` accepts the original node's row-major,
row-vector affine world matrix, before OpenEQ coordinate conversion. The
translation is row 3. It admits a positive uniform scale with orthogonal
axes, finite components, and a finite f32 reciprocal. Nonuniform scale,
shear, perspective, singular transforms, and nonfinite values are rejected.
The scale ceiling of 1000 is an explicit diagnostic admission bound, not a
recovered native engine limit.

There are two distinct native basis stages. If normalized world rows are
`r0`, `r1`, and `r2`, update first constructs emitter axes `[r2,r0,-r1]`.
Mode 3 then applies its -128 quarter-turn, producing final particle birth
axes `[r1,r0,r2]`. `OwnerPose` stores these final axes, ordered axial, radial
cosine, radial sine. The caller supplies the node world matrix without
preapplying either transformation.

The actual FTORCH301 cloud 411/406 world matrices have rows
`(1,0,0),(0,0,1),(0,-1,0)`, so final particle axes are
`(0,0,1),(1,0,0),(0,-1,0)`. POKLAMP500 cloud 20 has world rows
`(-1,0,0),(0,0,1),(0,1,0)`, so its final axes are
`(0,0,1),(-1,0,0),(0,1,0)`. A direct regression checks these original
matrices and their exact flame/smoke origins against the frozen placement
witness, independently of the synthetic identity and moving-owner controls.

Existing particles retain their birth origin and axes when the owner moves.
Current owner scale still multiplies axial/radial motion increments and
current quad size. The moving-owner snapshots distinguish retained particles
from particles subsequently born at the new position and orientation.

## Update ordering

The implemented order follows original update `0x10072ab0`:

1. Clamp previously expired emitter duration to zero. Owner suppression clears
   occupancy; otherwise retire leading particles whose remaining life is
   already nonpositive.
2. Determine births from elapsed emitter duration before subtracting this
   call's delta. The initial schedule counter of -1 creates one zero-time
   particle. The initial emitter duration is the native f32 value 9999.
3. Apply distance batch factor
   `max(min(1,80/distance),min(1,16/capacity))`. A batch truncating to zero
   retains its unconsumed schedule difference. The native f32 remaining
   duration quantization and intermediate rounding points are preserved.
4. Count scheduled births even when capacity is full. Only admitted births
   consume random values; a full ring does not overwrite live particles.
   New life is limited by the emitter's remaining duration.
5. Advance axial distance for every live particle and radial distance only
   when the caller's emitter view gate is true. Derive position from retained
   birth origin/axes, native angle tables, and updated motion distances.
6. Evaluate fade using life before this call's life subtraction, multiply
   by current owner alpha, and apply native nearest-even byte rounding.
   Evaluate drawing gates, then subtract particle life and emitter duration.

In particular, a particle reaching exactly zero life remains in this call's
output and occupancy; the following call retires it. A native exact-expiry
control verifies that ordering. Suppression prevents births for that call
without resetting the schedule counter or stopping emitter duration.

## Visibility and context are explicit

`owner_suppressed` represents the original owner's suppression policy and
clears the ring. `radial_motion_visible` represents the native emitter view
gate: false prevents drawing and radial motion while axial motion and life
continue. These inputs are different and must not be conflated.

`captured_context` and `FrameInput::draw_context` correspond to the original
emitter/manager context comparison. A mismatch prevents drawing but does not
pause life or radial motion. Original normal-scene context is -1; object
previews use 0 and 1. The sampler accepts explicit integer identities without
inferring preview lifecycle or camera selection.

The caller's drawing predicate supplies per-particle clipping and runs only
when the emitter view and context gates pass. Its candidate has `draw=false`
until the predicate returns. The sampler's RGBA/position for invisible
candidates are diagnostic mathematical values: the native routine skips
writing vertex/color output for culled particles. No full original frustum,
owner-suppression policy, or engine visibility policy is claimed here.

## Random and math recovery

`DiagnosticRandom::seed_one()` reproduces the shuffled-table algorithm with
an explicitly controlled MSVC-style `rand` seed of 1. Initialization advances
`state = state*214013+2531011` modulo 2^32 and returns
`(state>>16)&32767`. Native constructor `0x100706f0` initializes and shuffles,
in order, 512 angle entries, 2000 signed entries, and 1000 unit entries.
The angle table is not sampled by these four bodies, but its initialization
draws affect the subsequent shuffles.

Each native shuffle performs one full-range swap per table entry, with index
`trunc(rand * f32::from_bits(0x38000100) * table_length)`. Seed 1 produces
no endpoint overflow. The API intentionally does not generalize arbitrary
seeds around the original shuffle's endpoint behavior. Signed entries use
`f32(i*f64(0.001f32)-1)` and unit entries use `f32(i*f64(0.001f32))`.
Counters increment as wrapping u32 values; reads index their table modulo
its length, matching the original instructions.

Every admitted birth consumes seven unit draws, including collapsed ranges.
Radial speed uses the sixth value. Signed pairs are rejected outside the
unit disk and passed through the native quadrant/threshold angle lookup;
the zero source radius still consumes these azimuth draws. Unit fractions
never reach 1. Cosine uses the DLL literal `3.1415926535`, 512 entries,
and explicitly forced cardinal values; sine is shifted by 384 entries.

The independent math probe matched all initialized angle, signed, unit,
cosine, and sine table bytes against native execution. It also matched all
783 accepted even-parity signed-table pairs against original angle helper
`0x100b9e70`. The Rust regression snapshots verify the resulting azimuth,
speed, and state sequence across emitters. Float comparisons allow 2e-6
relative/absolute tolerance; this is not a promise of bit-exact host libm
behavior on every target.

This is a deterministic diagnostic stream. The original live client's seed,
prior random consumers, emitter creation order, and unrelated effects remain
unknown. Matching the controlled global stream does not establish live-client
random parity.

## Geometry boundary

The native routine writes four 32-byte vertices per drawn particle, with
`x,y,z,rhw,diffuse_u32,specular_u32,u,v`, and reports two triangles. Under
the controlled camera, screen Y points downward and corner order is:

| Corner | UV |
| --- | --- |
| Top left | (0,1) |
| Top right | (1,1) |
| Bottom right | (1,0) |
| Bottom left | (0,0) |

The quad is a centered screen-aligned square; size is the full edge length,
so half-size offsets are used. With projection factor 400, its pixel width
is `400*size/depth`. Tests compare native width, alpha, and the world center
recovered from projected x/y and reciprocal depth. Subtracting native f32
pixel coordinates near 500 loses precision, so width tolerance is 0.0002
pixels rather than the tighter state tolerance.

The sampler returns world centers and sizes, not projected XYZRHW vertices.
The controlled projection evidence does not replace recovery or validation
of the full original camera/frustum and clipping path. Texture uploads,
quad/index construction, coordinate conversion, material/blend selection,
depth policy, and production update ordering remain integration work. No
generic spell renderer is used or implied by this module.

## Portable fixtures and provenance

The tests embed facts captured from original x86 instruction execution,
not executable bytes and not expected state generated by the Rust sampler.
The original DLL is `/Users/daeken/EverQuest/EQGraphicsDX9.dll`, base
`0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The placement witness additionally uses original `d3dx9_30.dll`, SHA-256
`5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532`.

Local evidence sources, kept separate from the portable tests:

| File under `/tmp` | SHA-256 |
| --- | --- |
| `openeq-native-particle-witness.py` | `9e7b3158d9022e3f99c9939e45fbf7618e4c143a25ab2813f919a760e21efcd0` |
| `openeq-native-particle-witness.json` | `607dbfc25049cbbe3982bce3ff987085692c8740daccf88b170d85823a9156f1` |
| `openeq-native-particle-placement.py` | `abf53f5e09ae584933b7b172c2b92b86c1cc3a7866a6f56c529c168cefbdb164` |
| `openeq-native-particle-placement.json` | `cf45ba930a6d2fbc323a4445b8a61b00862280c14ee7cddede7620b65a843b1e` |
| `openeq-particle-sampler-math.py` | `c811d4562fabcf8118aa7962ab17904ecea82524ab55085c713b6642f045f119` |
| `openeq-particle-sampler-math.json` | `9b97637271c8fa787d0e267bb2d535dd18113d5e32146a3b6c794d20eee86c69` |
| `openeq-native-particle-sampler-controls.py` | `7c753b4c232f04b80d54faf67dcdd943f8f14327e2054395ba3e8158fccc2dad` |
| `openeq-native-particle-sampler-controls.json` | `1fcf7e8d3233130522ea2ede28049457c44a9190c2f40b1d2c156154d7fab369` |

`native_witness.bin` is 37788 bytes, SHA-256
`650c60e5f2bdf67dbd7fce4f1ce918a1a3bc5c932eba2ef2f72aebb3560a7d3a`.
Its little-endian layout is magic `WLP1`, u32 group count, then seven groups:
u32 definition index, u32 case, u32 frame count, followed by frames. Each frame
is f32 delta, f32 emitter duration, u32 total spawn count, u32 occupancy,
u32 triangles, 128 native vertex bytes, and `occupancy` native 128-byte
particle records. Definitions are Smoke/L301/L308/L500 indices 0..3; cases
are baseline/visible/behind/moving indices 0..3. The first four baseline
groups and later controls share one continuous diagnostic random stream.
Original control snapshots lacked emitter duration/spawn counters; their
fixture fields are zero placeholders and are not asserted.

`native_controls.bin` is 10088 bytes, SHA-256
`ede0f79fcd25f07a1ec8b8c482650507197d2dba45b101677a9247d4a44d9e53`.
Layout is magic `WLC1`, u32 group count, then five groups: three f32 origin
coordinates, f32 scale, f32 alpha, i32 captured context, i32 current context,
u32 frame count, followed by frames. Each frame is f32 delta, u32 suppressed,
u32 emitter view gate, f32 duration, f32 reserved zero, u32 total spawn count,
u32 occupancy, u32 triangles, 128 vertex bytes, and native particle records.
Each group resets native random counters to zero while retaining the original
seed-1 tables. Groups cover distance 1000, scale 2/alpha 0.5, suppression,
context mismatch, and exact expiry, all using L301. The suppression control
overrides the already-computed native suppression byte at `0x100733c1`; it
proves the resulting clear/no-birth behavior, not the full engine policy.

## Validation and remaining scope

`cargo test -p openeq-render wld_particles:: --lib` runs seven tests. They
compare every stored particle's birth transform, azimuth, radial/axial state,
and life across all four native families, including full-capacity frames,
moving and culled owners. Additional native controls verify counts, duration,
alpha, projected size/center, distance, scale, suppression, context, and exact
expiry. Admission, invalid input atomicity, subnormal transform rejection,
delta capping, all nine preview/scene context pairs, and actual torch/lamp
world matrices are also covered.

This closes the bounded CPU sampling task. General WLD particle families,
live global randomness, owner animation integration, full camera/visibility
policy, and scene/GPU activation require separate evidence and review.
