# WLD placed-object animation: native evidence and diagnostic poses

Production placed actors still use their first authored pose. This milestone
retains fragment `0x13` flags and exposes exact-frame diagnostic sampling; it
does not enable autonomous animation. The native key timing and loop closure
are established below, but the animated quaternion-to-node contract, clock
units, and City of Mist controller ownership still need proof.

## Reproducible source

The native evidence is from the installed 32-bit `EQGraphicsDX9.dll`:

```text
SHA256 615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383
Image base 0x10000000
.text:  RVA 0x1000,   file offset 0x400,    raw size 0x131200
.rdata: RVA 0x133000, file offset 0x131600, raw size 0x2ba00
.data:  RVA 0x15f000, file offset 0x15d000, raw size 0x1ca00
```

Addresses in this document are virtual addresses for that exact binary. The
local disassembly used for investigation is `/tmp/eqgraphics-disassembly.txt`;
the binary and that temporary file are not repository fixtures. Import names
were checked against the PE import table rather than inferred from tool labels.

The original regression fixture is
`citymist_obj.s3d/citymist_obj.wld`, actor `JNTREE103_ACTORDEF`. City of Mist has
50 placements of this actor. Its eight tracks own seven meshes, 106 vertices
and 72 triangles. Six branch tracks have four frames and timing word 1000;
root and trunk each have one frame and no timing word. Track-reference flags
are 5 on branches and 4 on the static tracks. Every track definition has the
packed-frame flag 8. Only the trunk's 42 polygons are collidable.

## Track reference and packed frame decode

The WLD loader wrapper at `0x10067570` installs vtable `0x1013b8fc`. Its slot
`+0x30` points to the fragment `0x13` handler at `0x1001ae20`; the dispatch is
visible at `0x1001a8b7..0x1001a8c4`. That handler allocates a decoded track with
this layout:

| Offset | Observed field |
| --- | --- |
| `+0x00` | Name pointer |
| `+0x04` | Timing multiplier, initialized to float 1.0 |
| `+0x08` | Byte copied from reference flag bit 1 (`flags & 2`) |
| `+0x09` | Byte copied from reference flag bit 2 (`flags & 4`) |
| `+0x0c` | Timing interval, initialized to 100 |
| `+0x10` | Frame count |
| `+0x14` | Decoded frame-array pointer |
| `+0x18` | Whether the name lacks a letter/digit/digit clip prefix |

If `flags & 1`, the optional source timing word replaces the default interval
at `0x1001aef6..0x1001aeff`. The other flag bytes are copied at
`0x1001af02..0x1001af12`; their playback semantics have not been established.
They must not be renamed to guessed loop or interpolation flags.

Each decoded frame is 32 bytes: quaternion XYZ at offsets 0, 4 and 8, W at
12, translation at 16, 20 and 24, and scale at 28. The packed conversion at
`0x1001af90..0x1001b03f` uses these constants:

| Component | Constant address | Multiplier |
| --- | --- | --- |
| Quaternion XYZ | `0x10135730` | `1 / 16384` |
| Quaternion W | `0x10135734` | **`-1 / 16384`** |
| Translation and scale | `0x1013572c` | `1 / 256` |

Scale is read as an unsigned 16-bit word at `0x1001b02a`. The current Rust
parser reads that packed field as signed; changing it requires a separate
corpus audit and is not part of this diagnostic milestone. All Citymist tree
scales are positive within the signed range.

If loader byte `+0x30` equals 1 and the track has more than 15 frames, the
interval is halved at `0x1001b04e..0x1001b05a`. The four-frame tree tracks do
not take that branch.

## Initial node pose and animation key quaternions differ

The skeleton loader at `0x1001dd10` resolves a decoded track into its entry
`+8`. At `0x1001e122..0x1001e12a` it negates **only frame zero's W in place**.
Consequently frame zero again has the raw source W sign, while later decoded
frames retain the negative-W convention from the decoder.

Node initialization at `0x1003ec20`, called from `0x1004c97f`, takes decoded
frame zero. It passes XYZW unchanged to `D3DXMatrixRotationQuaternion` at
`0x1003ed00`, then applies translation and uniform scale. This supports the
current raw-W first-pose convention.

The animation-key builder at `0x1003c150` negates W for key zero again at
`0x1003c387..0x1003c397`; later keys copy decoded W unchanged at
`0x1003c39c..0x1003c3a3`. Thus all authored animation keys start from
**W = -raw W**. It then selects equivalent quaternion signs for continuity
between keys (`0x1003c3c2..0x1003c498`) and across the closing seam.

Negating only W is generally not the equivalent quaternion sign change
`q -> -q`. Reusing the first-pose `glam` transform for native animated keys
would therefore be unjustified. The remaining requirement is to trace how
controller/compressed-set output becomes node matrices, or obtain controlled
native output. No native animated matrix or frame capture has yet verified
that step. The diagnostic sampler deliberately uses the existing first-pose
convention for every explicitly selected source frame and does not claim that
those pictures are native playback frames.

## Key timing, closure, and controller ownership

The hierarchical model-definition initializer `0x1004c8f0` detects tracks with
more than one frame at `0x1004ca06..0x1004ca0f`, creates a `CAnimation` at
definition offset `+0x84`, and invokes the key builder at `0x1004ca6d`.

The builder creates a D3DX keyframed animation set with ticks-per-second 1.0
and playback type 0 (loop) at `0x1003c199`. A source key's timestamp is:

```text
key_time = frame_index * decoded_track.interval * decoded_track.multiplier
```

The multiplication is visible at `0x1003c2ca..0x1003c2f5`. For a track with
more than one frame, the builder appends the first scale/rotation/translation
again at `frame_count * interval * multiplier`
(`0x1003c4a8..0x1003c650`). Single-frame tracks keep one key. For the tree's
four-frame branches, the key times are therefore 0, 1000, 2000, 3000 and a
closing key at 4000 in native clock units. Calling these seconds or
milliseconds still requires proof of the host clock described below.

Tracks belong to one animation set and share its period. Independently looping
every track would not reproduce mixed-count or mixed-interval sets. Allocation
uses the first animated track's count plus one; longer later tracks can be
clamped. General mixed-length compatibility needs additional coverage.

The builder optimizes keys via `0x1003b650`, registers SRT keys through the
D3DX set's vtable `+0x80` at `0x1003c696..0x1003c6a2`, and then calls the set's
compression path (`+0x84`). It may create a compressed animation set. The PE
imports identify these thunks:

| Thunk | IAT | Import |
| --- | --- | --- |
| `0x100b9b46` | `0x10133280` | `D3DXCreateCompressedAnimationSet` |
| `0x100b9b4c` | `0x10133284` | `D3DXCreateKeyframedAnimationSet` |
| `0x100b9b52` | `0x10133288` | `D3DXMatrixRotationQuaternion` |

Interpolation and compression parity should not be inferred merely from
authored key values. The investigated Wine `d3dx9_36/animation.c` implementation
has stubs for the relevant `GetSRT`, `RegisterAnimationSRTKeys`, and
`AdvanceTime` methods, so it does not settle this question.

Actor setup at `0x10044935` detects the definition's animation. It has two
ownership paths:

- `0x10044960..0x10044a23` creates a per-actor controller, starts at speed 1,
  and chooses an initial phase using CRT `rand * (1 / 32767) * duration`.
  The random scale is constant `0x10136660`; phase is set via `0x1003f110`.
- `0x10044a28..0x10044a43` enters shared group management at `0x10052b40`.
  Groups are keyed by animation pointer at `+0x68`. Constructor `0x10052970`
  starts at speed 1 and phase zero through `0x1003fdf0`. Later actors bind
  matching node names to the first actor's matrices at `0x10052a40`, using
  `0x1003d610`.

Which branch Citymist's placed trees actually select remains unproven. Neither
per-instance random phase nor globally synchronized trees should be chosen
without resolving that condition.

## Engine clock boundary

The base controller constructor is `0x1003ff50`, vtable `0x10138d04`; the
shared derived controller uses vtable `0x1013a404`. Both update through slot
`+0x18`, function `0x1003f640`.

That update obtains a clock through global graphics engine pointer
`0x1017c0a8`, virtual slot `+0x54`. It computes an unsigned delta at
`0x1003f6e7..0x1003f6fd` and passes that delta, converted to double without
rescaling, to D3DX `AdvanceTime` at `0x1003f741..0x1003f75b` (vtable `+0x34`).
Animation key times and this engine clock therefore share units.

`CreateGraphicsEngine` constructs the engine via `0x1006a640` and stores the
pointer at `0x10012356`. Its vtable is `0x1013becc`. Slot `+0x54` is
`0x10068fa0`, which adjusts `this` by 4 and enters `0x100bb4f0`, reading engine
offset `+0x10`. The neighboring setter `0x10068fb0 -> 0x100bb500` writes its
argument unchanged to that field. Construction initializes it to zero at
`0x1006a8b1..0x1006a8c5`.

The next clock investigation is the host `eqgame.exe` virtual setter call and
the source of its time argument. The graphics-side trace alone does not prove
wall-clock milliseconds. A local host disassembly was prepared at
`/tmp/openeq-eqgame-disassembly.txt` but that call chain is not established.

## Implemented diagnostic and verification

`PieceTrackRef::flags` and `ObjectTrack::reference_flags` now retain the source
reference flags independently from the frame-definition flags. Raw optional
timing is preserved, including zero and absent values.

`ObjectSource::sample_authored_frames(&[usize])` takes an explicit index per
track, or an empty slice for a static object. It copies source meshes and
applies the existing first-pose transform convention. It does not infer a
clock, select a phase, loop, interpolate, update instances, or mutate already
baked rendering/collision. It validates the current hierarchy and selection,
rejects nonfinite/degenerate poses, and preserves material references, UVs,
topology, collision flags, and rigid/weighted binding ownership.

The 11 focused CPU checks include synthetic rigid and weighted parts,
parent/local transforms, immutable independent actor bakes, bad selections,
bad later poses that leave frame zero usable, and two original-asset tests.
For the original tree, all four selected poses change branches while leaving
every trunk vertex and its 42 collision triangles unchanged.

The original Citymist GPU regression verifies all 50 placements, then captures
an isolated textured tree at each of the four explicit source poses. The
diagnostic frame zero is pixel-identical to the ordinary extracted actor at a
fixed renderer time. Each later pose visibly differs; all have 72 drawn and
42 collision triangles. Palette-masked texture aliases from object extraction
are retained. Output is `/tmp/openeq-citymist-objects/assembled-tree.png` and
`authored-frame-0.png` through `authored-frame-3.png` in the same directory.
These captures validate the diagnostic, not native motion parity.

```sh
CARGO_INCREMENTAL=0 EQ_DIR=/path/to/EverQuest cargo test -p openeq-assets --lib loader::wld_objects -- --include-ignored
CARGO_INCREMENTAL=0 EQ_DIR=/path/to/EverQuest cargo test -p openeq-render --test wld_objects -- --include-ignored
```

Autonomous playback remains gated on the quaternion/output-matrix contract,
host clock units, and placed-tree controller ownership. Future integration
also needs explicit animated bounds and a collision policy for actors whose
collidable parts move; the static Citymist trunk does not establish a general
collision policy.
