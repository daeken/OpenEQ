# WLD particle attachments and recoverable PoK object geometry

Evidence checkpoint: 2026-10-01. The eight rejected Plane of Knowledge torch,
lamp and sconce actors contain **39 ordinary mesh parts, 4,993 source vertices
and 3,130 collidable polygons**, in addition to their particle attachments.
They account for **366 placements** in the original zone metadata. Native
assembly treats a particle attachment as a separate typed child of a skeleton
node. It does not treat fragment `0x34` as an ordinary mesh.

This establishes a bounded path to restoring these actors' static mesh geometry
while retaining explicit unsupported particle attachments. It does **not**
establish complete actor fidelity, particle playback or effect-generated light.
This investigation changes no production code.

## Sources and scope

The installed PE32 `EQGraphicsDX9.dll` has preferred image base `0x10000000`
and SHA-256
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.
Addresses below are preferred virtual addresses. The binary was disassembled,
not executed. Function names below describe observed roles, not recovered
symbols.

Original source data:

| Archive / member | Uncompressed member SHA-256 |
| --- | --- |
| `poknowledge_obj.s3d` / `poknowledge_obj.wld` | `e7fbed560a7b81bbfe7495440418de0020f98ef84f63b3f5e4850fcfc2af10e3` |
| `poknowledge.s3d` / `objects.wld` | `f872a141e7600399ea00f8f4c6eb965fe03002251ca4be3c6adcb4fd5c3a11d2` |

An independent temporary PFS/WLD reader followed exact fragment references,
source mesh vertex-piece runs, polygon collision flags and packed frame words.
It did not select meshes by similar names. Fragment numbers below are one-based;
track indices and byte offsets are zero-based. Original assets were not changed
or added to the repository.

The broader rejected-actor inventory is in
[WLD_ANIMATION_SURVEY.md](WLD_ANIMATION_SURVEY.md). This checkpoint investigates
the native reader and the eight PoK actors; it does not certify all 376 rejected
particle-linked actor occurrences in that survey.

## Fragment `0x34` record structure

The bulk reader is `0x1001e640`; the on-demand reader is `0x1001d9b0`.
The loader scans for `0x34` at `0x1001a823` and calls the bulk handler through
virtual slot `+0x28` at `0x1001a882..0x1001a894`, before later actor assembly.
The bulk handler verifies fragment type at `0x1001e693`.

Both readers take a fixed **80-byte body**, followed by flag-controlled fields.
Here, **body offsets exclude the common four-byte name reference**. The native
source pointer includes that name reference, so its disassembly offsets are
four bytes greater. For example, flags are read at native `[esi+4]`, and the
optional-field cursor starts at `[esi+0x54]`.

The fixed fields below describe their observed representation, without inventing
simulation names for scalar parameters. All four distinct PoK definitions have
the same flags and selector words. The later duplicate definitions differ only
in their trailing texture-reference word.

| Body offset | Representation / demonstrated use | `L301_PCD` | `CSMOKE_PCD` | `L308_PCD` | `L500_PCD` |
| --- | --- | ---: | ---: | ---: | ---: |
| `0x00` | `u32`, optional-field flags | 4 | 4 | 4 | 4 |
| `0x04` | copied 32-bit selector | 3 | 3 | 3 | 3 |
| `0x08` | 32-bit mode, converter switches over 0–5 | 3 | 3 | 3 | 3 |
| `0x0c` | 32-bit bitfield, tested by converter | `0x30500` | `0x30500` | `0x30500` | `0x30500` |
| `0x10` | copied 32-bit integer | 40 | 50 | 10 | 40 |
| `0x14..0x20` | four `f32` values | all 0 | all 0 | all 0 | all 0 |
| `0x24` | `u32`, converted numerically by converter | 0 | 0 | 0 | 0 |
| `0x28` | `f32` | 2 | 0.4 | 0.3 | 1 |
| `0x2c` | `f32` | 15 | 60 | 20 | 15 |
| `0x30` | `u32`, converted numerically by converter | 650 | 3000 | 750 | 650 |
| `0x34` | `f32` | 3 | 1 | 0.7 | 2 |
| `0x38` | `f32` | 0 | 0 | 0 | 0 |
| `0x3c` | `f32` | 1 | 1 | 1 | 1 |
| `0x40` | `f32` | 0 | 0 | 0 | 0 |
| `0x44` | `u32`, converted numerically by converter | 60 | 160 | 90 | 60 |
| `0x48` | `f32` | 1 | 1 | 0.25 | 1.2 |
| `0x4c` | packed word; low three bytes used individually | `0x3fffffff` | `0x3f646464` | `0x3effffff` | `0x3fffffff` |

The cursor processes optional fields in this order:

| Flag | Bytes | Native evidence |
| --- | ---: | --- |
| `0x01` | 24 | Six floats are read as two triples at `0x1001e75c..0x1001e791`; they populate two ranges in the intermediate descriptor. |
| `0x02` | 24 | Cursor advances at `0x1001e7a1..0x1001e7a6`. This reader does not interpret the skipped block. |
| `0x04` | 4 in every PoK witness | Texture/animation reference slot; important native read-width caveat below. |

Every PoK cloud has an 84-byte body: flags 4, the 80-byte fixed portion and one
four-byte trailing slot. A parser should retain opaque fields and raw reference
identity instead of assigning guessed meanings or silently discarding them.
The optional-block interpretation for flags 1 and 2 comes from native code;
those flags are not exercised by these PoK records.

### Descriptor conversion

Fixed-field transfer occurs at `0x1001e7f4..0x1001e901`, then descriptor
initializer `0x10071c20` and converter `0x1001c410` are called. In particular,
body `0x24` is an integer, despite its zero value also looking like a float in
these files. The converter uses unsigned numeric conversion and calculates:

- body `0x24`: `-value * 0.001`, or `-9999` when zero, into descriptor `+0x78`;
- body `0x30`: `value * 0.001`, into descriptor `+0x7c`;
- body `0x44`: `1000 / value`, into descriptor `+0x88`.

These conversions are visible at `0x1001c6b0..0x1001c76d`; the constants were
read directly from the PE. They suggest timing parameters, but their complete
simulation semantics are outside this checkpoint. The low three bytes of body
`0x4c` are copied into paired descriptor slots at `0x1001c77c..0x1001c7b2`;
treating the whole word as a float would lose that structure. The high byte's
meaning, including any alpha interpretation, is not established here.

The converter reads frame and material information from the resolved texture
descriptor at `0x1001c7b8..0x1001c821`. It also has a six-character `TFIRE2`
name-prefix override at `0x1001c824..0x1001c866`; none of these PoK cloud names
matches. No generic blend or lifetime rule is inferred from the torch names.

### Texture reference width and duplicate names

The native bulk reader executes **`movzx eax, byte ptr [eax]`** at
`0x1001e7ae`, and the on-demand reader does the same at `0x1001dac3`.
It passes this byte to texture resolver `0x1001d520`. It does **not** load the
full four-byte reference stored in the file. This cannot be rewritten as an
ordinary native `u32` read in a compatibility account.

The early PoK definitions use small references and have these exact chains:

| Cloud `0x34` | Texture-bearing `0x26` | `0x05` | `0x04` | Bitmap `0x03` |
| --- | --- | ---: | --- | --- |
| 5 `CSMOKE_PCD` | 4 `I_CSMOKE_SPB` | 3 | 2 `I_CSMOKE` | 1 `CSMOKE.DDS` |
| 10 `L301_PCD` | 9 `I_L301_SPB` | 8 | 7 `L301_SPRITE` | 6 `GENG00.DDS` |
| 15 `L308_PCD` | 14 `I_L308_SPB` | 13 | 12 `L308_SPRITE` | 11 `GENG00.DDS` |
| 20 `L500_PCD` | 19 `I_L500_SPB` | 18 | 17 `L500_SPRITE` | 16 `GENG00.DDS` |

The `0x26` bodies contain flags 0, the `0x05` reference and word `0x80000017`.
The `0x04` records contain flags `0x18`, one bitmap reference and timing word
100. The native resolver explicitly handles kind `0x26` at `0x1001d576` as
well as kinds `0x03`, `0x04` and `0x05`; particle texture ownership is not an
ordinary mesh-material link.

Later cloud fragments 406, 411, 438 and 471 contain trailing reference words
405, 410, 437 and 470 respectively. Their first 80 body bytes are identical to
the earlier same-name cloud. Native helper `0x100c0520` checks a named resource
of type `0x1002` before either reader decodes it. A matching existing resource
is written to the current fragment's runtime pointer at `0x100c0574`, and the
reader then skips decoding. Thus native named-resource reuse can bypass the
byte-width issue for duplicates. This static trace does not establish every
load context or name-filter path, so it is not proof that arbitrary references
above 255 decode correctly. Preserve exact file references in asset metadata.

## Native attachment and instance ownership

The relevant path is reachable from the normal WLD skeleton and actor loaders,
not merely an unused particle utility:

1. The particle reader converts and registers the descriptor with the manager
   at global `0x1017c0b0`, calling `0x10070ce0` at `0x1001e952`. Wrapper
   constructor `0x1006f2f0` stores the returned definition index at wrapper `+4`
   and the resource context at `+8`. It registers resource type `0x1002` and
   saves the wrapper on the WLD fragment entry.
2. Skeleton reader `0x1001dd10` resolves each track's attachment reference at
   `0x1001e182`. It branches separately for `0x2d` ordinary meshes, `0x11`
   nested skeletons, and `0x34` particles. The `0x34` branch starts at
   `0x1001e275`, obtains frame-zero scale and calls the on-demand particle
   reader at `0x1001e292`. Bulk conversion supplies scale 1; on-demand
   conversion can supply this track scale. Cached named definitions can skip
   conversion, so this is not proof of arbitrary per-instance scaling policy.
3. That branch allocates a 48-byte typed attachment using `0x1003b380`.
   Its particle wrapper lives at attachment `+0x28`; ordinary mesh and nested
   skeleton slots remain zero. The decoded skeleton track retains the typed
   attachment at track `+4` (`0x1001e2e3`). Child links remain separate.
4. Node-definition constructor `0x1003ec20` copies the track attachment to
   node definition `+0x80`. There is a particle suppression condition beginning
   at `0x1003ec63`, requiring scale below approximately `0.05` plus further
   translation checks. All PoK witness scales are 1, so that condition cannot
   suppress their attachments.
5. Runtime skeleton construction calls node-instance constructor
   `0x1003e5e0` at `0x10047c9d`. It reads definition `+0x80` and selects mesh,
   nested skeleton or particle construction from the separate attachment
   slots. The particle path allocates a 400-byte actor, passes the current
   runtime node as its seventh argument and calls `0x10052630` at
   `0x1003e756`. The child actor is retained at runtime node `+4`.
6. Particle actor constructor `0x10052630` stores its typed attachment at actor
   `+0x178`, obtains the particle definition through `0x1006f310`, and calls
   manager helper `0x10071080` at `0x10052718`. Arguments include the
   definition index, this particle actor, a pointer into its actor data and
   the owning runtime node. The returned emitter handle/pointer is retained
   at actor `+0x184`. This is per-instance ownership, not one emitter per
   shared WLD definition.

The particle actor destructor invokes a manager removal operation with the
actor pointer at `0x100525a4..0x100525bf`. This supports ownership/lifetime
separation, but does not establish the update clock, spawn distribution,
billboarding, randomization, light emission, culling or full destruction
behavior. Those need their own playback trace before implementation.

Separately, the skeleton's flags-`0x200` mesh list is processed at
`0x1001e30c..0x1001e55a`, resolving ordinary `0x2d` mesh references. It is not
replaced by the particle attachment branch.

## PoK actor witnesses

All eight skeletons have flags `0x202`. Every track has one packed frame
(definition flags 8), reference flags 0 and scale 1. Every ordinary mesh's
vertex-piece runs bind **all its vertices to track 1**. Track 1's only ancestor
is root track 0. Every particle attachment is on track 3 or 4 below track 1;
none is on a mesh vertex's ancestry. All ordinary source polygon flags are
zero (collidable), and all referenced ordinary material render-method words
are nonzero. Counts below are source geometry, before material batching.

| Actor | Placements | Skeleton | Ordinary mesh fragments `0x36` | Parts | Vertices | Collidable polygons | Particle fragments by track |
| --- | ---: | ---: | --- | ---: | ---: | ---: | --- |
| `FTORCH301` | 60 | 424 | 412 | 1 | 307 | 168 | 3 → 411 `L301_PCD`; 4 → 406 `CSMOKE_PCD` |
| `FTORCH302` | 13 | 449 | 439 | 1 | 183 | 128 | 3 → 438 `L308_PCD` |
| `FTORCH304` | 17 | 482 | 472 | 1 | 273 | 160 | 3 → 471 `L301_PCD` |
| `POKLAMP500` | 16 | 1389 | 1359–1369 | 11 | 831 | 482 | 3 → 20 `L500_PCD` |
| `POKLAMP501` | 13 | 1437 | 1411–1419 | 9 | 1376 | 1000 | 3 → 20 `L500_PCD` |
| `POKLAMP502` | 30 | 1483 | 1451–1462 | 12 | 1660 | 918 | 3 → 20 `L500_PCD` |
| `POKSCONCE500` | 49 | 1733 | 1721 | 1 | 94 | 90 | 3 → 20 `L500_PCD`; 4 → 406 `CSMOKE_PCD` |
| `POKTORCH500` | 168 | 1915 | 1901–1903 | 3 | 269 | 184 | 3 → 20 `L500_PCD` |

### Torch with both flame and smoke

ActorDef 2603 `FTORCH301_ACTORDEF` points to SkeletonRef 2602, which points to
skeleton 424 `FTORCH301_HS_DEF`. Its explicit skeleton mesh reference 423
points to mesh 412 `FTOR301_DMSPRITEDEF`. The mesh has center `(0,0,0)` and
one vertex-piece run `(307 vertices, track 1)`.

| Track | Name | Parent | Local translation | Raw packed quaternion `(w,x,y,z)` | Attachment |
| --- | --- | ---: | --- | --- | --- |
| 0 | `FTORCH301_DAG` | — | `(0,0,0)` | `(16384,0,0,0)` | none |
| 1 | `FTO301B1_DAG` | 0 | `(0,-1.4453125,7)` | `(16384,0,0,0)` | ordinary mesh vertex binding |
| 2 | `FTO301B2_DAG` | 1 | `(0,0,0.25)` | `(16384,0,0,0)` | none |
| 3 | `FTO301P1F_DAG` | 2 | `(0,0,0.40625)` | `(8192,8192,8192,8192)` | 411 `L301_PCD` |
| 4 | `FTO301P2S_DAG` | 3 | `(0,0.41015625,0)` | `(16384,0,0,0)` | 406 `CSMOKE_PCD` |

The smoke node is a child of the flame-bearing node, so discarding emitter
nodes from the hierarchy would discard meaningful transform ownership even
when their generated particles are unsupported. The mesh itself depends only
on tracks 0 and 1. The raw standalone mesh misses its authored track-1 offset.

### Lamp with eleven mesh parts

ActorDef 2695 `POKLAMP500_ACTORDEF` → SkeletonRef 2694 → skeleton 1389.
Its mesh-list references 1378–1388 resolve to meshes 1359–1369; all mesh centers
are zero. The track chain is `0 → 1 → 2 → 3`. Track 1
`PKLMPBN500A_DAG` translates by `(-6.046875,0.04296875,27)`; track 2
`PKLMPBN500B_DAG` translates by `(0,0,0.15625)`; track 3 translates by
`(0,0.00390625,0.37890625)` and attaches `L500_PCD`. Track 3's source name is
**` PKPAR500_DAG`**, including its leading space. Tracks 0–2 have raw quaternion
`(-16384,0,0,0)`; track 3 has `(-8192,-8192,-8192,-8192)`.

All eleven mesh parts bind to track 1. The lower particle nodes add no transform
to those vertices. Retaining the actor/skeleton chain is sufficient to recover
the mesh's authored static offset; attaching raw mesh fallbacks at the placement
origin is not equivalent.

## Boundary for a later implementation

At this checkpoint, `wld.rs` retains `0x34` only as `Fragment::Ignored(0x34)`.
`actor_source` sends every nonzero track attachment to `resolve_mesh`, so the
first particle node aborts the entire actor before the independent skeleton
mesh list is assembled.

A bounded restoration can distinguish the supported particle record from an
invalid mesh reference, retain its exact fragment identity and owning track,
and continue assembling the ordinary mesh list. It must retain the whole
validated hierarchy, first-frame transforms, vertex-piece bindings, material
visibility and source collision flags. Unsupported particle playback should
remain explicit in the retained source metadata and diagnostics. Arbitrary
unknown attachment types or malformed references should not become a blanket
ignore rule.

For these eight actors, static mesh pose and authored mesh collision need only
tracks 0 and 1; the particle-bearing descendants are not inputs to either.
The native branches likewise construct particle children separately from the
ordinary mesh list. No dependency requiring the emitter to generate the
ordinary geometry was found in this bounded path. This is a claim about that
geometry and its source collision, not proof that all native visibility,
lighting, effect bounds or scene-culling decisions are independent of particles.
Restored fixture bodies without native flames/smoke remain partially supported
actors and should be described that way.

One-frame reference flags 0 do not require extending the bounded animation
sampler: these witnesses can use existing first-pose assembly. The four animated
particle-linked actors in the wider survey need separate evaluation. A general
implementation must also preserve mesh descendants of unsupported attachment
nodes and validate their complete ancestry rather than assuming every particle
node is a leaf or that all mesh parts bind to track 1.

Temporary reproduction evidence is in
`/tmp/openeq-wld-particle-probe.py`, `/tmp/openeq-wld-particle-pok.json` and
`/tmp/eqgraphics-disassembly.txt`; the probe uses
`/tmp/openeq-group-region-audit.py` only for PFS decompression. Placement counts
come from the independently retained survey results described in
`WLD_ANIMATION_SURVEY.md`. No production tests were needed for this read-only
checkpoint; no rendering or native particle playback comparison was performed.


## Implemented partial restoration

The bounded static mesh path described above is now implemented. Particle
records retain all fixed words, optional blocks, full file texture references
and trailing bytes; owning tracks and exact duplicate definition identity are
preserved. Positive source references, the proven flags-4/no-tail layout,
packed one-frame flags-0 tracks and independent mesh ancestry are required.
Native particle behavior stays unsupported and is recorded in loader warnings,
source metadata and zone-audit placement diagnostics. See
`WLD_PARTICLE_SURVEY.md` for full corpus reconciliation and
`WORLD_AUDIO_PARITY.md` for original GPU and workspace verification.
