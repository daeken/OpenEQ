# Native WLD particle placement and preview contexts

Evidence checkpoint: 2026-10-01. A bounded original-x86 execution witness now
connects two real Plane of Knowledge placements to their particle origins and
owner bases. It also identifies manager `+0xbc` as the object-preview drawing
context: emitters capture it at construction, and the update routine emits
triangles only for matching contexts. A mismatch still advances particle life.
This is research; no production particle implementation changes here.

This extends [WLD_PARTICLE_RUNTIME.md](WLD_PARTICLE_RUNTIME.md), whose original
runtime witness used controlled owner matrices. Source attachment identities
remain documented in [WLD_PARTICLE_ACTORS.md](WLD_PARTICLE_ACTORS.md). The drawing
context here is separate from the loader's named-resource cache context.

## Provenance and execution boundaries

All graphics addresses below refer to the installed PE32 `EQGraphicsDX9.dll`,
image base `0x10000000`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
The matching Microsoft x86 `d3dx9_30.dll`, base `0x00400000`, has SHA-256
`5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532`.
Its source and scalar dispatch selection are documented in
[WLD_OBJECT_ANIMATION.md](WLD_OBJECT_ANIMATION.md).

The original WLD members are:

| Archive/member | SHA-256 |
| --- | --- |
| `poknowledge_obj.s3d/poknowledge_obj.wld` | `e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3` |
| `poknowledge.s3d/objects.wld` | `f872a141e7600399ea00f8f4c6eb965fe03002251ca4be3c6adcb4fd5c3a11d2` |

The temporary harness maps original code/data in Unicorn 2.1.4. It executes
the native track reader, frame-zero quaternion adjustment, node constructors,
particle actor constructor, descriptor converter and emitter factory. It then
executes placement position/angle routines, native D3DX matrix operations, the
active model-render block that invokes recursive node composition, and the
particle CPU update through projected triangle generation.

The harness supplies parsed original fragment records to a minimal loader
table. It builds typed attachment wrappers and links converted descriptors to
manager records directly. Node child counts and links come from the original
skeleton; the complete skeleton/resource loader does not execute. Mesh-only
attachments are omitted. Inverse-bind matrices retain their native constructor
identity values; the particle world matrix does not depend on those matrices.

The enclosing owner uses original base constructors, hierarchical actor/data
vtables and native owner setters, with unit scale/alpha, zero auxiliary angles
and controlled engine state. The complete placed-actor constructor and world
registration do not execute. Particle actor registration is intercepted, but
its constructor and emitter creation execute. Allocations, ASCII character
classification, deterministic random input and vertex-buffer Lock/Unlock are
controlled as in the preceding runtime witness. The camera sits 60 units
behind each placement on Y and looks toward it. No running original client,
GPU draw, material resolution, texture upload or arbitrary animated skeleton
is claimed.

## Placement and skeleton composition

The fragment `0x15` reader at `0x1001c890` passes the authored position pointer
and the following three angle floats to hierarchical actor constructor
`0x10044660`, at `0x1001ca70..0x1001ca87`. The constructor calls position setter
`0x100443d0` at `0x10044bfe` and rotation setter `0x10042d90` at `0x10044c0d`.

Position setter `0x100c4710` copies XYZ into the translation row of the actor
data matrix at data `+4`. Rotation routine `0x100c48a0` consumes the source
angles through the native 512-unit trigonometric tables. The enclosing rotation
setter creates a uniform scale matrix and computes `scale * actor_matrix`
before calling model virtual slot `+0x10`, `0x1004a880`, which copies the result
to model `+0xc`. Both tested placements have scale 1 and auxiliary angles 0.
The witness executes the position/rotation routines and model matrix setter;
the enclosing world-manager notifications are outside it.

The existing packed-track evidence applies unchanged: `0x1001ae20` initially
decodes W with a negative sign; skeleton-reader instructions
`0x1001e122..0x1001e12a` negate frame zero's W again. All nine tested tracks have
one packed frame, reference flags 0 and scale 1. Node-definition constructor
`0x1003ec20` builds the initial local matrix using original D3DX quaternion,
translation and scale operations. Instance constructor `0x1003e5e0` copies it
to node `+0x20`, with node `+0x10` pointing there. Particle attachments take its
real `0x10052630` constructor call at `0x1003e756`.

Recursive composition is `0x10049310`, model virtual slot `+0x88`. For the
ordinary unaliased nodes in this witness, it computes at `0x10049497..0x100494ac`:

```text
node.world       = node.local * parent.world
node.skin_matrix = node_definition.inverse_bind * node.world
```

The root uses model `+0xc` as its parent matrix. `node.world` is stored at node
`+0xa0`; `node.skin_matrix` is at `+0x60`. Child recursion passes the parent's
`+0xa0` matrix through the same virtual slot. These are row-vector matrices.

The active model-render routine `0x10049c90` looks up root node 0 and invokes
that virtual slot with a null parent at `0x10049cb2..0x10049cca`. The witness
executes this original caller block through return at `0x10049ccc`, including
the complete recursion. It stops before subsequent clock/mesh submission.
Independent matrix multiplication checks every captured node result.

## Original placement witnesses

Fragment numbers are one-based within their respective WLD members. Values
below are in authored/native world coordinates, before any OpenEQ coordinate
conversion.

| Actor | Skeleton | Placement | Position XYZ | Raw angles |
| --- | ---: | ---: | --- | --- |
| `FTORCH301` | 424 | 18 | `(142, 1375.0556640625, -108.49897766113281)` | `(384, 0, 0)` |
| `POKLAMP500` | 1389 | 571 | `(925.6531982421875, 150.05615234375, -151.99896240234375)` | `(128, 0, 0)` |

Their native placement rotation rows are respectively:

```text
FTORCH301:  (0,-1,0), ( 1,0,0), (0,0,1)
POKLAMP500: (0, 1,0), (-1,0,0), (0,0,1)
```

The torch's chain is `0 -> 1 -> 2 -> 3 -> 4`. Track 1 translates by
`(0,-370/256,1792/256)`, track 2 by `(0,0,64/256)`, and flame track 3 by
`(0,0,104/256)`, with raw quaternion WXYZ `(8192,8192,8192,8192)`.
Smoke track 4 translates by `(0,105/256,0)` in its parent's rotated basis.
All other torch quaternions are identity. The resulting smoke origin is
therefore `105/256 = 0.41015625` world Z above the flame.

The lamp's chain is `0 -> 1 -> 2 -> 3`. Its translations are
`(-1548/256,11/256,6912/256)`, `(0,0,40/256)` and `(0,1/256,97/256)`.
The final raw quaternion has all four components `-8192`; the preceding
identity quaternions have raw W `-16384`. The final track name is
`" PKPAR500_DAG"`, including its original leading space.

| Attachment | Native world origin / first spawned particle XYZ |
| --- | --- |
| Torch flame, cloud 411 `L301_PCD` | `(140.5546875, 1375.0556640625, -100.84272766113281)` |
| Torch smoke, cloud 406 `CSMOKE_PCD` | `(140.5546875, 1375.0556640625, -100.43257141113281)` |
| Lamp flame, cloud 20 `L500_PCD` | `(925.6063232421875, 144.00927734375, -124.46380615234375)` |

The CPU update reads translation from node `+0xd0/+0xd4/+0xd8` at
`0x10072f92..0x10072fb2`. With the PoK owner-basis flag, emitter axes are world
matrix row 2, row 0 and negative row 1, as established by the prior witness.
The real placements produce these axes:

```text
Torch flame and smoke: (0,-1,0), ( 1,0,0), (0,0,-1)
Lamp flame:            (0, 1,0), (-1,0,0), (0,0,-1)
```

These are the intermediate emitter axes. Mode 3's -128 quarter-turn makes the
final particle birth axes normalized world rows `[1, 0, 2]`:
`(0,0,1), (1,0,0), (0,-1,0)` for the torch, and
`(0,0,1), (-1,0,0), (0,1,0)` for the lamp. The
CPU sampler tests these final axes directly against the placement witness.

All three first particles exactly inherit their computed node translation.
Each produces two triangles under the controlled camera and matching context.
This closes the owner-composition gap for these two static PoK placements;
it does not establish every skeleton's alias, animation or inverse-bind path.

## Object-preview drawing-context lifecycle

The render routine at `0x10017860` includes the original UTF-16 marker
`ObjectPreviewView::Render`, referenced at `0x10017980` from `0x10135618`.
Renderer initialization `0x10098b40..0x10098b88` constructs two preview objects,
indices 0 and 1, stored at renderer `+0xd7e8` and `+0xd7ec`. Constructor
`0x10017820` stores the index at preview `+0xc`. The preview renderer bounds
that index to 0 or 1 and selects the corresponding engine camera.

The preview-render block `0x10017a87..0x10017ab2` performs this sequence:

1. Copy preview `+0xc` to particle manager `+0xbc`.
2. Call manager update `0x100762f0` with the preview camera.
3. Call draw dispatch `0x10072a60`.
4. Restore manager `+0xbc` to -1.

The witness executes that exact block for both indices, intercepting only its
update/draw calls to record arguments. Both calls observe the selected index;
the final value is -1. The central frame path renders enabled previews at
`0x10097e84..0x10097eab`, before normal particle update/draw at
`0x1009812c` and `0x1009813b`.

Setter `0x10086c00` writes its argument to global manager `+0xbc`. It is renderer
virtual slot `+0x120`, table entry `0x1013df64`. The installed `eqgame.exe`
(SHA-256 `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`)
uses that slot around preview object work: at `0x005b1a88..0x005b1a9d` it passes
the object's `+0x1e0` index, and at `0x005b1b42..0x005b1b52` it restores -1.
This host bracket is static evidence; the host executable is not emulated.

Particle actor constructor `0x10052630` captures manager `+0xbc` at
`0x100526ce` and passes it through the native factory to emitter `+0x98`.
The update routine compares the current manager context with this saved value
at `0x10075265..0x10075276`, in the triangle-generation path.

For each of the nine combinations below, the witness executes the original
setter, creates a fresh torch flame through the native particle actor
constructor, changes the manager context, then runs updates at delta 0 and
0.1 seconds. Both updates produce the shown triangle count:

| Captured emitter context | Manager -1 | Manager 0 | Manager 1 |
| ---: | ---: | ---: | ---: |
| -1 | 2 | 0 | 0 |
| 0 | 0 | 2 | 0 |
| 1 | 0 | 0 | 2 |

Every combination creates its first particle. Every combination reduces its
life from `0.6500000357627869` to `0.550000011920929` seconds on the second
update. The context mismatch suppresses drawing at this gate; it does not
pause the whole update. Preview/main frame scheduling and the manager's shared
clock must therefore be considered together when implementing native parity.

## Reproduction and remaining scope

The temporary artifacts are `/tmp/openeq-native-particle-placement.py`,
`/tmp/openeq-native-particle-placement.json` and the matching `.log`. They are
local research outputs, not committed fixtures. Run with:

```text
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-native-particle-placement.py
```

The script reuses only the setup prefix of the frozen runtime harness and the
reader prefix of `/tmp/openeq-wld-particle-probe.py`; it does not execute their
output-writing bodies. All assertions pass. Frozen artifact SHA-256 values:

```text
script abf53f5e09ae584933b7b172c2b92b86c1cc3a7866a6f56c529c168cefbdb164
JSON   cf45ba930a6d2fbc323a4445b8a61b00862280c14ee7cddede7620b65a843b1e
```

Native resource registration, scope visibility, loader selection and bounded
full-WLD assembly are now documented in
[WLD_PARTICLE_LOAD_CONTEXT.md](WLD_PARTICLE_LOAD_CONTEXT.md). Remaining work
includes complete startup/import cache history, broader placement and animation
modes, and integration with an actual renderer.
The material/blend path is documented separately in
[WLD_PARTICLE_MATERIALS.md](WLD_PARTICLE_MATERIALS.md). These results do not
authorize guessed emission parameters, generated lights or visual parity claims
based solely on attachment names.
