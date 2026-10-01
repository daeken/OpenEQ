# Connected native particle frame admission

This witness executes the complete original host entry `0x004bfd10`, its
renderer frame `0x10097de0`, and every admitted preview body `0x10017860`,
through their normal returns. The native particle manager, four list updates,
camera conversion, and stationary placed PoK lamp execute inside that call
chain. The scene and graphics resources are explicit controlled inputs.

It closes the disconnected caller slices in
[preview admission](WLD_PARTICLE_PREVIEW_ADMISSION.md) and
[lamp cadence](WLD_PARTICLE_LAMP_CADENCE.md) for this chosen host entry and
resource configuration. It does not establish a complete live client session,
the visibility callbacks of a populated zone, or the global random stream.
Production rendering is unchanged.

## Executed caller chain

Before rendering, the probe constructs both native preview objects and calls
the complete host widget controller `0x005b1230` for each requested index.
That controller reaches renderer setter `0x10086b00` and preview setter
`0x10017d90`, creates/resizes the supplied target, and sets the widget latch.
The preview actor pointers remain null. Widget rectangles are supplied as
400 by 300; widget camera configuration is a controlled endpoint, with the
actual native camera objects supplied separately. The complete UI window
creation/open/close lifecycle is not executed.

Each frame then follows the original calls:

```text
eqgame 0x004bfd10
  native timer 0x00897d90 -> host+0x154
  engine +0x58 -> 0x10068fb0 -> 0x100bb500
  host object/UI/camera/ambient work
  renderer +0xac -> 0x10097de0
    engine getter -> renderer+0xaff4
    device availability check
    enabled preview slots, in index order
      0x10017860: target/context/index/camera checks
        native camera preparation and projection upload
        native empty-scene pass 0x1008bd90
        context = preview index
        0x100762f0 -> four 0x10072ab0 calls
        particle draw dispatch; context = -1
    BeginScene and main camera preparation
    scene/resource checks
    native 0x1000ec40, 0x1000f250, 0x100891c0, 0x1008bd90
    0x100762f0 -> four 0x10072ab0 calls
    particle draw dispatch
    native frame cleanup and return
  native host cleanup and return
```

The host work functions `0x004af4d0` and `0x004b9970` execute rather than being
replaced with assumed return values. Their supplied state has empty tracked
object and UI-root lists, no host sky or player actor, paused native sound
manager, and unset host ambient-resource field `+0x2d04`. This takes real
empty/disabled paths through those systems.

The cameras use native constructor `0x10006830`, real vtable `0x10133c7c`,
native getters/setters, and complete preparation `0x10006080`. The main
camera is distinct from the preview cameras. Supplied heading is 128 native
angular units; the near and behind cameras are 60 units on opposite sides of
the actual lamp origin. A far preview is 1000 units away. Successful main
camera dimensions are 800 by 600. No camera-preparation return value is
intercepted.

The native main-scene setup and renderer routine `0x1008bd90` also execute
completely, including their original return-value checks. Geometry queues are
empty and DPVS visibility emits no geometry callbacks. The scene's virtual
`+0x24` is an explicit resource interface returning an allocated empty zone
resource or null; the original implementation of that getter is not claimed.

## Admission controls

The table lists actual manager-update contexts, in invocation order. All
ordinary rows publish tick 1100 with the particle manager initially at 1000.

| Supplied state | Reached update contexts |
| --- | --- |
| Both widgets disabled | `-1` |
| Widget 0 enabled | `0, -1` |
| Widget 1 enabled | `1, -1` |
| Both widgets enabled | `0, 1, -1` |
| Enabled preview 0 uses far camera | `0, -1` |
| Both enabled, native target 0 teardown | `1, -1` |
| Both enabled, renderer context missing | `-1` |
| Both enabled, preview camera 0 missing | `1, -1` |
| Both enabled, target 0 width zero | `1, -1` |
| Both enabled, preview object 0 index set to 2 | `1, -1` |
| Both enabled, scene missing | `0, 1` |
| Both enabled, scene resource getter returns null | `0, 1` |
| Both enabled, main camera width zero | `0, 1` |
| Both enabled, device BeginScene fails | `0, 1` |
| Both enabled, then widget 0 disabled through controller | `1, -1` |
| Both enabled, initial device availability check fails | none |
| Both enabled, OS counter advances between reads | `0, 1, -1` |

Zero target width reaches the original failed preview camera-preparation
branch; zero main width reaches the separate main failure branch. The device
failures are supplied HRESULTs from the controlled device interface. The
invalid preview index is a direct object-state control, distinct from the
API's invalid-index rejection already established by the earlier witness.

Native teardown `0x10017ed0` leaves both enabled bytes set but clears target
0, so the outer loop still dispatches that preview and its body returns
before updating particles. Disabling through the host controller instead
clears widget 0's latch and preview enabled byte before the frame.

A null preview actor does **not** prevent the particle call. The actor work
is guarded at `0x100179fe`, but the particle block is after that guard. Thus
even the two explicitly blank preview resources in this experiment advance
the shared world lamp's state. A live implementation cannot derive particle
admission solely from whether an actor is displayed in the preview.

Main BeginScene, camera, and scene failures occur after preview processing.
They do not reverse earlier preview simulation. Device unavailability is
checked before preview processing and skips every update. Existing particle
buffer contents in that last case remain unchanged; they are not evidence
that a draw occurred.

## Clock publications in the connected frame

Native timer initialization `0x00897df0` selects the original QPC timer
implementation. The controlled OS frequency is 10,000,000 counts per second,
with initial counter 100,000,000; native elapsed-millisecond conversion runs.
Engine getter/setter and all admitted particle updates remain original code.

A CPU memory-write hook observes every store overlapping engine `+0x10`.
Every one of the 17 admission cases records exactly one store, at
`0x100bb504`, called through `0x10068fb0` from host return address
`0x004bfd70`. Host `+0x154`, engine `+0x10`, and renderer `+0xaff4` agree.
Every particle pass consumes that same published value.

The advancing-clock case supplies a counter increment of 170,000 counts
(17 ms) after each QPC read. The cold host-frame calls observe:

| Native timer use | QPC counter | Elapsed ms |
| --- | ---: | ---: |
| First-entry host initialization | 111,000,000 | 1100 |
| Host frame publication | 111,170,000 | 1117 |
| First subsequent host object-work read | 111,340,000 | 1134 |
| Second subsequent host object-work read | 111,510,000 | 1151 |
| Host ambient-work read | 111,680,000 | 1168 |

Despite those later OS reads, the renderer and all three particle passes use
1117. The first list batch receives the original float32 delta
`0.11700000613927841`; subsequent batches receive zero. Every batch retains
list order `+0x14, +0x5c, +0x80, +0x38`. Native preview cleanup restores
manager context to -1.

This establishes absence of an intervening engine-clock publication on the
executed path with these inputs. It does not establish that no publisher can
run with populated scene/UI/actor state or other host frame entries. In
particular, terrain writer `0x100a5110` is never reached in this empty-scene
witness. Its previously proven setter behavior remains valid, and its live
caller timing remains unresolved.

## Connected stationary-lamp results

The particle fixture reuses the actual native placement of `POKLAMP500`,
placement index 571, at approximately
`(925.606323, 144.009277, -124.463806)`. It imports only the hash-checked setup
prefix of the earlier cadence witness, before any of that witness's output
writes or scenario execution. Descriptor, owner/node linkage, initial
particle, and diagnostic seed-1 native tables are inherited unchanged.

At tick 1100 the world-only frame retains one particle and produces two
triangles. Either admitted preview causes the final world pass to contain
two particles and four triangles. Preview contexts produce zero world-lamp
triangles but can change its simulation state. A first update using the
behind preview leaves the first particle's radial term at zero; a near first
update changes it to about `0.0028300183`. The lifetime and axial advance are
identical, confirming the first admitted camera's effect inside this connected
caller chain.

Three cases continue through four additional complete host frames without
resetting native state. These warm frames have four QPC reads, rather than
the cold frame's five, because the original first-entry host flag is already
set. Every warm frame still records one clock store and the same update order.

| Connected sequence | Particle counts at ticks 1100, 1200, 1300, 1400, 1500 |
| --- | --- |
| Main only | `1, 2, 4, 5, 7` |
| Preview 1 behind camera, then main | `2, 4, 5, 7, 9` |
| Preview 0 near, preview 1 behind, then main | `2, 4, 5, 7, 9` |

These counts match the earlier sliced cadence witness, now reached through
the chosen complete host frame and explicit controller/resource state.

## Controlled interfaces and limits

The renderer executes in Unicorn, with these external boundaries supplied:

- GPU device, render-target construction/destruction/resize, and preview
  surface begin/end interfaces. Actual GPU rendering is not performed.
- Widget rectangle and widget camera setup. Native controller latches,
  renderer enable/resize wrappers, and preview resource checks execute.
- Empty DPVS frustum/camera/visibility endpoints and scene resource getter.
  No populated-zone visibility, owner admission, or terrain callbacks run.
- Auxiliary-effects option disabled, debug geometry unavailable, and empty
  post-render world/resource callbacks. These are explicit resource controls.
- Native D3DX runs at relocated base `0x20000000` to avoid overlapping the
  host EXE. Its CRT allocation, absent registry override, and Windows XP / no
  optional processor-feature query responses are supplied. Graphics DLL
  imports are rebound to the relocated original exports.
- QPC/QPF and debug/performance-marker interfaces. The two CRT floating-point
  control wrappers return a controlled cookie; the harness uses x87 control
  word `0x037f`.
- The final particle draw dispatcher `0x10072a60` is intercepted after native
  simulation and vertex-buffer generation, as in the prior cadence witness.
  There is no GPU pixel claim.

The RNG initialization is a controlled diagnostic state, not a recovered
session seed or proven complete random-consumer sequence. Static world-lamp
placement is established; a populated world's creation order, owner changes,
visibility callbacks, live UI states, and other clock publishers are still
separate requirements. No live particle enablement follows from this probe.

## Frozen evidence

There are 17 independent admission controls and 12 warm continuation frames:
29 complete host-frame invocations, all assertions passing. Prior preview
and cadence JSON hashes remain unchanged. The new script writes only its own
new output paths; use a copy with new output paths for future experiments.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-particle-frame-admission.py` | `c3a28b56d9b4f9dde6f018639e167c6428ed2cf9fafc3c23d552552a995e265c` |
| `/tmp/openeq-particle-frame-admission.json` | `b8d96cde7473cc6235806d18ce9387d26fc052ebd405ea44dd1867e40eb29a72` |
| `/tmp/openeq-particle-frame-admission.log` | `a816044ff0de5e207b36c09eacf2d154155658e8823ea6cb005a7b7a6d23c9d0` |

The JSON records source hashes, every case's controller/resource setup,
native entry counts, interface calls, clock writes, complete converted
camera inputs, per-list deltas, particle draws, and particle snapshots. It
also records each warm continuation frame. Original source-image hashes are
unchanged from the linked preview/cadence witnesses.

Root independently replayed the complete 29-frame probe using separate output
paths; JSON and log hashes exactly match the frozen results above.
