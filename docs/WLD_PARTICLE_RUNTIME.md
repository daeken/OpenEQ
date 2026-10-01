# Native WLD particle runtime follow-up

Evidence checkpoint: 2026-10-01. Executing the installed original particle
converter, emitter factory and CPU update routine establishes a bounded runtime
model for the four distinct Plane of Knowledge particle definitions. It also
demonstrates how named-resource reuse can bypass the native low-byte texture
reference read. This is research for a future implementation; it adds no
production particle playback or effect-generated light.

The attachment and original record inventory are in
[WLD_PARTICLE_ACTORS.md](WLD_PARTICLE_ACTORS.md). Static geometry recovery and
the full corpus reconciliation are in
[WLD_PARTICLE_SURVEY.md](WLD_PARTICLE_SURVEY.md). This follow-up does not repeat
those audits or extend its runtime conclusions to other particle modes.

## Provenance and execution limits

The source is the installed PE32 `EQGraphicsDX9.dll`, preferred image base
`0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below are preferred virtual addresses; role names are descriptions,
not recovered symbols. The original `poknowledge_obj.wld` member of
`poknowledge_obj.s3d` has SHA-256
`e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3`.
Body offsets exclude the common four-byte fragment name reference. Descriptor,
emitter, manager and particle offsets refer to distinct native structures.

A temporary Python harness maps original PE code and data in Unicorn 2.1.4 and
executes the native math-table initialization, random-table construction,
descriptor conversion, emitter allocation, update and projected-vertex
generation. It separately executes the clock wrapper and particle readers.
This is **not a running original client**. Its controlled boundaries are:

- Heap allocation uses a mapped-memory allocator. ASCII name comparison uses a
  bytewise `strnicmp` substitute.
- `rand` uses a deterministic MSVC-style substitute seeded with 1. The original
  table construction, shuffling and selection execute unchanged. The actual
  client's seed and random sequence are not established.
- Vertex-buffer Lock/Unlock use a memory-backed substitute. The original CPU
  routine writes vertices and colors, but no GPU submission, final blend state,
  sampler state or texture upload executes.
- Owners use original native vtables/getters, unit scale and alpha, controlled
  node matrices and a controlled camera. Actual PoK placement matrices and
  complete client skeleton playback are not executed.
- The update witness links a native definition record to the converted
  descriptor directly. Full manager registration and material integration are
  outside that witness. Its texture descriptor has one frame, timing 100 and
  material word 0; the actual negative material handle is not resolved.
- The reader experiment supplies controlled resource lookup, texture resolver
  and registration interfaces. Native traversal, cache checks and byte reads
  execute. This proves conditional behavior, not every real load context.

## Active frame path and clock

The central frame path calls manager update `0x100762f0` at `0x1009812c`, then
draw dispatch `0x10072a60` at `0x1009813b`. The earlier lead
`0x10097c23 -> 0x10070880` releases buffers; it is not the particle update.

The clock wrapper reads the engine at global `0x1017c0a8`, through virtual slot
`+0x54`, getter `0x10068fa0` and `0x100bb4f0`. The value is an engine millisecond
tick. Its host clock origin is not established. The wrapper subtracts the
previous manager `+0x04` value using unsigned 32-bit wrapping arithmetic,
converts elapsed milliseconds to seconds, and caps the result at 1 second. It
stores the new tick before dispatching the four particle lists.

| Previous tick | Current tick | Captured update delta |
| ---: | ---: | ---: |
| 1000 | 1016 | approximately 0.016 s |
| 1000 | 6000 | 1 s |
| `0xfffffff0` | `0x10` | approximately 0.032 s |
| 200 | 100 | 1 s, from unsigned wrap followed by the cap |

The native calls target manager lists `+0x14`, `+0x5c`, `+0x80`, `+0x38`, in that
order. Each receives the same delta. The WLD list at `+0x38` receives flag 1;
the other lists receive flag 0. The final update calls were intercepted for
this clock test; separate witnesses execute the full update routine.

## Definition conversion and emitter creation

The WLD emitter path is manager `0x10071080 -> 0x10070d90 -> 0x1006d2a0`, with
constructor `0x1006ce50`. The definition record points to its descriptor at
`+0x14` and heads its emitter list at `+0x10`; emitter `+0x84` links the next
emitter. The constructor allocates a `0x9c`-byte emitter and a particle ring of
`(capacity + 1) * 0x80` bytes. Capacity bounds occupancy without per-particle
allocation. Its ring pointers are at `+0x74..+0x80`.

For the four original PoK definitions, body `+0x10` supplies capacity, body
`+0x30` supplies particle life in milliseconds, body `+0x44` supplies the
emission interval, body `+0x34` supplies axial speed and body `+0x48` supplies
quad size. All have mode 3, bitfield `0x30500` and no optional vector blocks.

| Definition | Capacity | Life, seconds | Interval, ms | Nominal rate, /s | Axial speed | Maximum radial speed | Quad size | RGB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `CSMOKE_PCD` | 50 | 3.000000238 | 160 | 6.25 | 1 | 0.906347215 | 1 | 100, 100, 100 |
| `L301_PCD` | 40 | 0.650000036 | 60 | 16.66666603 | 3 | 0.558556199 | 1 | 255, 255, 255 |
| `L308_PCD` | 10 | 0.750000060 | 90 | 11.111110687 | 0.7 | 0.175340876 | 0.25 | 255, 255, 255 |
| `L500_PCD` | 40 | 0.650000036 | 60 | 16.66666603 | 2 | 0.372370809 | 1.2 | 255, 255, 255 |

The routine at `0x100b9e30` is a **tangent lookup**, not a cosine lookup. The
native math initialization uses 512 units per turn and builds a sine/cosine
ratio table. This lookup truncates its input to an integer and masks it with
`0x7f`. For these integer PoK angles, the converted maximum radial speed is
`body[0x34] * tan(body[0x2c] * 2π / 512)`. The angles are respectively 60, 15,
20 and 15. This description does not generalize the lookup to arbitrary inputs.

Mode 3 converts to runtime shape 4, with descriptor `+0xc8 = -128` and
`+0xc4 = 0`. Its radius at `+0xac` is zero unless source bit `0x80` is set;
none of these definitions sets it. **Body `+0x28` is ignored by this mode's
conversion**, despite looking like a plausible radius in the source records.
The source packed color's high byte is also ignored here; the three low bytes
supply RGB. Changing L500's body `+0x28` to 99, or its packed color high byte
to zero, leaves converted descriptor bytes `+0x60..+0x19f` identical to the
original. This is a mode-3 mutation control, not a claim about other modes.

Other relevant converted fields are:

| Descriptor offset | PoK value / demonstrated role |
| --- | --- |
| `+0x60` | 1, use owner basis |
| `+0x6c` | 1, owner scale handling |
| `+0x74` | 0, existing particles retain their birth transform |
| `+0x78` | -9999 when source body `+0x24` is zero |
| `+0x7c` | particle life |
| `+0x80`, `+0x84` | 1, initial and batch counts |
| `+0x88` | nominal rate, `1000 / interval` |
| `+0x94` | particle life, used for the fade-out interval |
| `+0x90`, `+0x98`, `+0x9c` | 0 |
| `+0xa0` | 80, distance scaling factor |
| `+0xa4` | 1 |
| `+0x16c`, `+0x170` | quad size, 1 |

PoK acceleration fields are zero. The emitter starts with elapsed counter
`+0x10 = -1`, total spawn counter `+0x14 = 0`, initial count `+0x18 = 1`, batch
count `+0x1c = 1`, and rate at `+0x20`. Both initial duration `+0x24` and
remaining duration `+0x28` start at 9999. The factory can return zero for these
indefinite emitters while writing a valid output pointer; zero alone is not a
failure indication on this path.

The particle actor is emitter `+0x68`; its runtime owner node is `+0x6c`.
The actor constructor also captures manager `+0xbc` in emitter `+0x98`.
Manager construction defaults this value to -1. Update compares the two at
`0x10075265..0x10075276`, and a mismatch later suppresses drawing. The semantic
name of this value is not established. It is distinct from the loader's named
resource context described below.

## Spawn, motion and retirement ordering

`0x10072ab0` combines simulation and projected quad construction. It walks the
definition list and emitter lists, requires a nonzero list vertex buffer at
`+0x20`, and evaluates owner methods and node transforms on each update.

The relevant ordering for the exercised path is:

1. Retire expired records from the ring front at `0x100733c1..0x100733f7`.
2. Evaluate the emission schedule at `0x10073410`, using
   `(initial_duration - remaining_duration - descriptor_delay) * rate` and
   truncating to integer at `0x10073428`. The initial -1 counter produces one
   particle at zero elapsed time.
3. Initialize new particle origins, bases, direction, speed and life; update
   particle motion and construct visible quads.
4. Decrement particle life at `0x10076121..0x1007612b` and emitter remaining
   duration at `0x100761dd..0x100761e7`.

Thus emission and color evaluation use time **before subtracting the current
frame's delta**. The ring can contain records that became expired during this
call until the next retirement pass. The witness JSON's `live` array is ring
occupancy, not a promise that every stored particle remains visibly alive.

The nominal rate is subject to distance scaling. Descriptor `+0xa0 = 80`
contributes `min(1, 80 / distance)`, with constructor floors for initial and
batch counts. The initial floor at emitter `+0x30` is 1; the batch floor at
`+0x34` is `min(1, 16 / capacity)`: 0.32 for smoke, 0.4 for L301/L500 and 1 for
L308. The table above must not be interpreted as a fixed rate at all distances.

For the near-camera L301 witness, deltas
`[0, .05, .05, .1, .25, .5, 1, 1, 1]` yield total spawn counters
`[1, 1, 1, 2, 4, 8, 16, 33, 50]`. These counters are not ring occupancy.

Runtime shape 4 reaches `0x10073e67`. It samples two coordinates from the
native shuffled signed table and rejects pairs outside the unit disk. A native
angle helper derives azimuth. The zero radius makes each PoK birth a point
origin, while azimuth remains random. Radial speed samples the converted
range from zero to the table's maximum using the shuffled 0..999 table; its
discrete fraction does not reach 1. The equal axial speed range endpoints make
axial speed constant for each definition.

Relevant 128-byte particle fields are birth origin at `+0x00`, birth basis at
`+0x0c..+0x2c`, azimuth at `+0x30`, radial and axial distances at `+0x34` and
`+0x38`, direction coefficients at `+0x3c/+0x40`, axial speed at `+0x44`, radial
speed at `+0x50`, total life at `+0x5c` and remaining life at `+0x60`.
Initialization at `0x100747d1..0x100747de` sets both life fields. Axial motion
includes speed times delta at `0x10074cbb`; radial motion at
`0x10074dc1..0x10074dd8` is visibility-dependent.

Fresh visible and behind-camera L301 instances were stepped with
`[0, .1, .1, .1]`. The visible instance produced `[2, 2, 4, 8]` triangles; the
behind-camera instance produced none. In both, the first particle reached
approximately 0.9 axial distance and 0.35 remaining life. Visible radial
distance advanced; behind-camera radial distance stayed zero. Random state
was not reset between controls, so their exact random speeds are not compared.
This proves partial simulation culling: neither a complete pause nor wholly
view-independent motion.

## Owner basis and birth transform

The previously traced skeleton attachment path supplies a runtime node to the
particle actor. At `0x10072f92`, update takes emitter origin from node
`+0xd0/+0xd4/+0xd8`. At `0x10073177`, it takes basis vectors from the node world
matrix at `+0xa0`, in this order:

- emitter `+0x44..+0x4c`: matrix third row, `+0xc0/+0xc4/+0xc8`;
- emitter `+0x50..+0x58`: matrix first row, `+0xa0/+0xa4/+0xa8`;
- emitter `+0x5c..+0x64`: negative matrix second row, `+0xb0/+0xb4/+0xb8`.

Owner scale getter `0x1003d820` supplies the first-row norm for normalization.
A controlled identity matrix translated to `(10, 20, 30)` gives emitter basis
`(0,0,1), (1,0,0), (0,-1,0)`. Mode 3's -128 quarter-turn subsequently modifies
the particle birth basis. The emitter basis is therefore not itself the final
travel-axis basis.

In the moving-owner control, after a 0.1-second step the node changed to a
cyclic rotation and translation `(11,22,33)`. The next update immediately
changed the emitter basis. The existing particle retained birth origin
`(10,20,30)` and its old basis; the newly spawned particle used `(11,22,33)`
and the new basis. Descriptor `+0x74 = 0` selects this retention behavior.

The original FTORCH301 and POKLAMP500 track chains and packed rotations remain
recorded in the attachment checkpoint. This controlled matrix test does not
claim that their complete native skeleton/world transform composition ran.

## Quad and color output

The native CPU routine writes four 32-byte vertices and two triangles per
visible particle. Each vertex contains projected `x, y, z, rhw`, diffuse and
specular words, then UV. Captured UVs are `(0,1), (1,1), (1,0), (0,0)`.

With controlled projection factor 400 and depth 30, zero-age quad widths are
approximately 13.3333 pixels for CSMOKE/L301, 3.3333 for L308 and 16 for L500,
matching their source sizes 1, 0.25 and 1.2. Initial diffuse colors are
`0xff646464` for smoke and `0xffffffff` for the three flame definitions.

For the first L301 particle, the update delta sequence
`[0, .05, .05, .1, .25, .5]` produces diffuse alpha
`[255, 255, 235, 216, 177, 78]`. This matches life-based fading evaluated before
the current update's life subtraction. The source packed high bytes `0x3f` and
`0x3e` are not used as alpha by this conversion. These CPU results do not prove
the final material, texture sampling or GPU blend equation.

## Named resource reuse and texture reference width

Native loader entries have stride 20: kind at `+4`, name pointer at `+8`, body
pointer at `+0x0c`, runtime pointer at `+0x10`. Loader `+0x34` holds the entries;
`+0x2c` supplies the resource context. Cache helper `0x100c0520` uses the
loader's virtual lookup, accepts resource type `0x1002`, and copies the result
to the fragment runtime pointer at `0x100c0574`.

Both readers call this helper before decoding: bulk cache call `0x1001e6dd`,
on-demand cache call `0x1001d9ee`. On a miss, the actual texture reads at
`0x1001e7ae` and `0x1001dac3` are one byte wide.

The controlled bulk witness supplies all eight original cloud records at
their original fragment indices, other entry kinds zero, scan bounds 5..471,
no name filter and resource context `0x1234`. It registers four definitions
and calls the texture resolver only with `[4, 9, 14, 19]`. The later runtime
pointers alias the earlier same-name definitions:

| Later fragment | Reused earlier fragment | Name |
| ---: | ---: | --- |
| 406 | 5 | `CSMOKE_PCD` |
| 411 | 10 | `L301_PCD` |
| 438 | 15 | `L308_PCD` |
| 471 | 10 | `L301_PCD` |

On-demand reads of fragment 411 distinguish the conditions:

| Controlled lookup result | Native result |
| --- | --- |
| Same name/context, correct type | Returns 0, reuses fragment 10's runtime pointer, no texture call |
| Context changed to `0x5678`, cache miss | Attempts texture 154 (`410 & 255`); mocked resolver rejects; returns -1 |
| Same name/context, wrong resource type `0x9999` | Rejects cached object, attempts texture 154; returns -1 |

Named reuse therefore can avoid the narrow texture read, but it cannot justify
assuming that arbitrary references above 255 decode correctly. The real
client's complete load order, filters and context lifetime are not established
by the controlled resource dictionary. Asset metadata should continue to
retain the **full original file reference** rather than replacing it with a
low byte or with an inferred same-name identity.

## Evidence artifacts and remaining work

The temporary harness and output are not committed or required by production:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-native-particle-witness.py` | `9e7b3158d9022e3f99c9939e45fbf7618e4c143a25ab2813f919a760e21efcd0` |
| `/tmp/openeq-native-particle-witness.json` | `607dbfc25049cbbe3982bce3ff987085692c8740daccf88b170d85823a9156f1` |

Run with `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-native-particle-witness.py`. The latest run passes its descriptor,
initial spawn/quad, lifetime, clock wrap/cap/list order, converter mutation,
visibility, owner movement and cache hit/miss assertions. The three L500
converter controls share descriptor-tail SHA-256
`315e2a9c61c2ec98bae1d23900b1228e5ecd0cbac6d4d113f0b3312f43a90d9c`.

Before enabling production effects, the unresolved integration work includes
the actual material/texture and blend path, manager registration and bounds,
the meaning and lifecycle of the manager `+0xbc` drawing gate, native owner
transform composition at real placements, and which load contexts permit
definition reuse. Exact client random initialization and unsupported source
selectors/modes also remain unproven. No light behavior follows from the
particle colors or torch names alone.
