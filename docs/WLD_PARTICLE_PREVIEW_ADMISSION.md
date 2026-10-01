# Particle preview admission and cached clock publication

This addendum narrows the remaining caller boundary from
[the stationary lamp cadence witness](WLD_PARTICLE_LAMP_CADENCE.md). Both
native preview slots start disabled. An explicit host widget controller can
enable or disable them through the renderer interface. An enabled request
alone does not guarantee a particle update: preview resources, camera and
other render-entry gates must also admit the preview body.

The engine clock is a published cached u32 timestamp, already established by
[the effect-clock witness](EQG_EFFECT_CLOCK.md). The new probe connects its
cached host transfer to the renderer's frame-entry read and identifies an
additional native terrain-system writer. It does not claim that a live PoK
frame always has previews, or that all admitted preview/main calls always see
an unchanged timestamp. No production rendering is changed.

## Preview flag writers and renderer interface

The original graphics DLL is `EQGraphicsDX9.dll`, SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.

Preview constructor `0x10017820` initializes its 28-byte object with device at
`+0`, null actor at `+4`, null render target at `+8`, supplied index at `+0xc`,
and disabled byte at `+0x10`. Renderer initialization constructs two such
objects, indexed 0 and 1, at renderer `+0xd7e8/+0xd7ec`.

Renderer vtable `0x1013de44` exposes:

| Slot | Entry | Behavior |
| --- | --- | --- |
| `+0x10c` | `0x10086b00` | Accept enable byte and index; dispatch to `0x10017d90` |
| `+0x110` | `0x10086b30` | Return the indexed preview's enabled byte |
| `+0x114` | `0x10086b50` | Assign the indexed preview actor |
| `+0x11c` | `0x10086bd0` | Resize the indexed preview target |

Both enable setter and getter reject unsigned indices greater than 1 and
null preview pointers. Invalid getter indices return false. The setter's
callee writes the supplied low byte at `0x10017dae` before managing resources.
A nonzero enable creates a missing target, using width/height each capped at
1024; a zero enable releases preview-associated resources and clears the
target pointer. Re-enabling an existing target does not recreate it.

The probe executes the original constructors, wrappers and setter. A controlled
render-target constructor/destructor substitutes only the graphics resource
endpoint. With original width/height getters returning 1920/1080, the native
setter requests target dimensions `[1024,1024,1024,1024]`. Invalid indices 2
and `0xffffffff` leave both preview objects unchanged.

## Connected host controller

The installed `eqgame.exe` has SHA-256
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
Its renderer pointer is global `0x01822674`.

Host controller `0x005b1230` takes an enable byte. The widget's `+0x1e0` is its
preview index and byte `+0x1e4` is its active latch. On a nonzero request with
the latch clear, it calls renderer slot `+0x10c` with `(1, index)` at
`0x005b1261`, configures its rectangle/camera and target size, then sets the
latch to 1. A repeated enable with that latch already set returns without
another setter/resource call. A zero request clears the widget's preview
actor through `0x005b0ea0`, calls renderer `+0x10c` with `(0, index)` at
`0x005b12ce`, and clears the latch.

The complete controller and its native renderer calls execute for each index.
Only widget rectangle/camera setup and the graphics target endpoints are
controlled; actor slots are empty in this control. Observed results:

| Action | Native enabled bytes | Native target presence | Host latch |
| --- | --- | --- | ---: |
| Native preview construction | 0, 0 | absent, absent | controlled 0 |
| Host enable index 0 | 1, 0 | present, absent | 1 |
| Host disable index 0 | 0, 0 | absent, absent | 0 |
| Host enable index 1 | 0, 1 | absent, present | 1 |
| Host disable index 1 | 0, 0 | absent, absent | 0 |
| Direct renderer enable both | 1, 1 | present, present | unchanged |

Static host evidence connects this controller to an object-preview window:
constructor `0x007414e0` supplies the original string `ObjectPreviewWnd` at
`0x00aecdbc`. Its child at parent `+0x230` is passed to controller
`0x005b1230` by `0x007408ac` with false, `0x007414d5` with true, and
`0x00740953` with a supplied byte. Another caller family uses parent `+0x4fc`.
These call sites establish explicit UI control, but their complete window
lifecycle, persistence and live user state are not executed here. They are not
evidence that opening a zone automatically enables either preview.

## Requested enable versus actual particle invocation

The original frame loop `0x10097e84..0x10097ead` walks both preview pointers
in index order. With synthetic render endpoints only for observing dispatch,
the native loop calls exactly no previews, index 0, index 1, or indices 0 then
1, according to the tested enable combinations.

Resource release `0x10017ed0` clears target `+8` but preserves enabled byte
`+0x10`. Restore `0x10017ea0` reads that byte and invokes the enable setter
again to recreate a target when requested. Both behaviors execute. Thus the
frame loop can dispatch an enabled preview whose render function immediately
returns without running the particle block.

Original render entry `0x10017860` separately requires:

- Nonnull preview target `+8`.
- Nonnull renderer context pointer `+0x17d8`.
- Preview index at most 1 and nonnull engine camera at `+0x38 + index*4`.
- Successful camera preparation through camera slot `+0x6c`, whose negative
  return branches out at `0x1001797a`.

The first three missing-resource/context/camera return paths execute in the
probe without intercepting the preview render routine. The camera preparation
failure branch is static evidence. Other draw/owner/resource work between
entry and the particle block is not fully emulated here.

When the body reaches `0x10017a87`, the earlier cadence witness applies:
context set, shared particle manager update, draw dispatch, then restore -1.
A live caller must distinguish an enable request, admission by the outer loop,
and actually reaching this particle block. Inferring all three from one
boolean would exceed the evidence.

## Cached clock connection and additional writer

The earlier effect-clock probe executes the original host timer conversion,
engine setter/getter and three host publication slices. Those frozen artifacts
remain unchanged. In particular, engine getter `0x10068fa0 -> 0x100bb4f0`
returns engine `+0x10`; it never samples the OS clock. Particle manager update
uses that same getter.

This addendum executes host cached transfer `0x004bf545..0x004bf559`, which
passes host `+0x154` unchanged through engine slot `+0x58`. It then executes
the original renderer frame-entry read `0x10097e33..0x10097e46`, which reads
the getter and stores it at renderer `+0xaff4`. Values 1000, 1016,
`0xffffffff`, and 0 remain identical at host, engine, renderer and a repeated
native getter. The controls do not need or invoke an OS clock endpoint.

Static host frame paths call renderer slot `+0xac`, target `0x10097de0`, at
`0x004bfc38` or `0x004bfeb4`. The latter lies in the host function beginning
`0x004bfd10`, after its known fresh timer publication at
`0x004bfd57..0x004bfd70`. The former path can alternatively select renderer
slot `+0xb0` before that call. These are surrounding caller observations;
intervening host calls and the full renderer function are not executed by this
addendum.

A further writer prevents claiming that the three known host slices are the
only publication paths. Terrain vtable `0x10140644`, identified by native RTTI
as `CEQTerrainSystem`, has slot `+0xe0 = 0x100a5110`. At `0x100a5126`, that
routine forwards its supplied u32 time through the same global engine setter,
before checking optional terrain controller `+0x100`. The complete null-
controller path executes with 5555 and `0xffffffff`, and the native engine
getter returns each supplied value unchanged. Its live call sites and timing
relative to preview/main rendering are not established here; the routine's
existence does not show that it runs between those particle invocations.

Consequently an OS clock advancing while no publisher runs cannot itself
change particle delta. Conversely, elapsed wall-clock time or a preview flag
alone cannot establish an unchanged cached tick throughout every live frame.
The remaining clock investigation should trace admission/order of these
publishers and any indirect writers in the selected host frame path, while
recording the cached value at each actual particle invocation.

## Remaining live admission boundary

This closes the explicit preview enable API, its connected host controller,
resource persistence behavior, and the cached host-to-renderer time transfer.
It does not close the complete PoK UI/frame lifecycle. Main particle work
also remains conditional on its surrounding native scene gates, including
returns from `0x100891c0` and `0x1008bd90` before `0x10098125`.

The next focused witness should start at the actual chosen host frame entry
with known preview-window/resource state, record every reached particle block
and engine-clock publication, and preserve each invocation's camera and
context. The stationary lamp's owner suppression, full culling inputs, live
creation order and random stream remain separate requirements. No global
random seed or live particle enablement follows from this addendum.

## Frozen reproduction

```text
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-particle-preview-admission.py
```

The probe passes connected enable/disable/repeated-enable controls for both
indices, invalid-index preservation, both-slot traversal, target teardown and
restore, original render-entry early returns, four cached frame transfers and
two terrain publication controls. It reuses only the hash-checked setup prefix
of the existing runtime witness, before any frozen output writes. Render-target
and widget endpoints are explicit controls; no native GPU or complete live
frame loop runs.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-particle-preview-admission.py` | `b12515a08033ff3889bb2c6dbca32e3646ad9517df9712e84a21faee61382a3b` |
| `/tmp/openeq-particle-preview-admission.json` | `631636d4c73f9c0b4e867a4d6409faecb18eda1c82e4d49a7c019d3d139b83f6` |
| `/tmp/openeq-particle-preview-admission.log` | `b975fae1071507d50656687ee35480e850c5b686c06f959bcaf9d3046c5834c7` |
| Reused runtime setup script | `9e7b3158d9022e3f99c9939e45fbf7618e4c143a25ab2813f919a760e21efcd0` |
