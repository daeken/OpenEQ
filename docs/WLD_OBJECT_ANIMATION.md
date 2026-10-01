# WLD placed-object animation: native evidence and diagnostic poses

Production placed actors still use their first authored pose. This milestone
retains fragment `0x13` flags and exposes exact-frame and bounded-time sampling; it
does not enable autonomous animation. The native clock, quaternion-to-node
conversion, interpolation and City of Mist shared-controller selection are now
established below. A bounded time sampler can use this evidence without
claiming compatibility with every WLD animation.

## Reproducible source

The native evidence is from the installed 32-bit `EQGraphicsDX9.dll`:

```text
SHA256 615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383
Image base 0x10000000
.text:  RVA 0x1000,   file offset 0x400,    raw size 0x131200
.rdata: RVA 0x133000, file offset 0x131600, raw size 0x2ba00
.data:  RVA 0x15f000, file offset 0x15d000, raw size 0x1ca00
```

Unqualified `0x100...` addresses are virtual addresses for that exact binary. The
local disassembly used for investigation is `/tmp/eqgraphics-disassembly.txt`;
the binary and that temporary file are not repository fixtures. Import names
were checked against the PE import table rather than inferred from tool labels.

The host-clock evidence uses the installed 32-bit `eqgame.exe`, image base
`0x00400000`, SHA256
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
Its temporary disassembly is `/tmp/openeq-eqgame-disassembly.txt`.

The graphics DLL imports `d3dx9_30.dll`. The matching Microsoft x86 library was
extracted, without installation, from `Apr2006_d3dx9_30_x86.cab` in the official
[June 2010 DirectX redistributable](https://download.microsoft.com/download/8/4/A/84A35BF1-DAFE-4AE8-82AF-AD2AE20B6B14/directx_Jun2010_redist.exe).
The installer SHA256 is
`053f76dcbb28802e23341b6a787e3b0791c0fa5c8d4d011b1044172dbf89c73b`;
the DLL SHA256 is
`5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532`.
Its image base is also `0x00400000`, so addresses below explicitly distinguish
D3DX from the host. Local research files are
`/tmp/openeq-d3dx9-native/d3dx9_30.dll` and
`/tmp/openeq-d3dx9-30-disassembly.txt`; neither is a repository fixture.

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

Scale is read as an unsigned 16-bit word at `0x1001b02a`. The Rust parser now
matches that unsigned read after an independent native check and full installed
WLD corpus audit; see [packed scale evidence](WLD_PACKED_SCALE.md). The audit
found 63 high-bit frames in three copies of one character track and none in
object archives. All Citymist tree scales remain unchanged within the signed
range. Quaternion and translation words remain signed.

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
`q -> -q`. The missing inverse is in D3DX: both the keyframed-set vtable
`0x00401e18` and compressed-set vtable `0x00401dd8` use the same `GetSRT`
implementation at `0x0043bb8d`, slot `+0x24`. It invokes quaternion sampler
`0x0043ae1d` at `0x0043bc20`. This sampler **conjugates both selected keys**,
negating XYZ and retaining W (`0x0043af2a..0x0043af69`). Thus an EQ key
`[rawX, rawY, rawZ, -rawW]` becomes `-rawQuaternion`, which has the same
rotation as the raw quaternion used by the initial pose.

D3DX linearly blends those conjugated quaternion components and normalizes
the result at `0x0043afaf..0x0043afed`: this is normalized linear interpolation,
not spherical interpolation. A static/clamped endpoint is returned without
that sampler normalization at `0x0043aff4..0x0043b00a`. The controller's output
helper `0x00437d82`, called by `AdvanceTime` at `0x00438e6a`, accumulates track
outputs with quaternion-sign handling and normalizes again at
`0x00438309..0x0043830e`. Its inline SRT matrix write at
`0x00438349..0x0043843e` uses the standard D3DX row-vector rotation convention;
there is no additional inverse. EQGraphics registers each node's `+0x10`
matrix pointer directly with the controller at `0x1003f0a3..0x1003f0bb`.

An isolated Unicorn 2.1.4 x86 execution of the original D3DX `GetSRT`, scalar
normalizer and `D3DXMatrixRotationQuaternion` checked BR1 at 0, 500, 1000,
1500, 2000, 2500, 3000, 3500, 3999 and 4000 milliseconds. Normalized
quaternions agreed with conjugated component interpolation to less than
`3e-8` per component; the closing matrix matched the first. This executes
native math, not the running EQ client or its complete actor update. It
selects D3DX's scalar fallback table, equivalent to
`D3DXCpuOptimizations(FALSE)`, and needs no Win32 services. The temporary
reproducer and results are `/tmp/openeq-native-animation-witness.py` and
`/tmp/openeq-native-animation-witness.json`.

The exact-frame diagnostic still uses the existing first-pose convention for
each explicitly selected source frame. Its captures validate geometry
sampling and do not claim to be captures of native playback.

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
closing key at 4000 milliseconds: a four-second loop. The host-clock proof is
below.

Tracks belong to one animation set and share its period. Independently looping
every track would not reproduce mixed-count or mixed-interval sets. Allocation
uses the first animated track's count plus one; longer later tracks can be
clamped. General mixed-length compatibility needs additional coverage.

Loop wrapping occurs before `GetSRT`. D3DX `AdvanceTime` calls the set's
`GetPeriodicPosition`, vtable `+0x14`, at `0x00438a0b`, storing the result at
controller-track `+0x28`. Both set vtables point to `0x0043a787`; for playback
type zero it computes the remainder of time divided by set period through
`_CIfmod` at `0x0043a79f` (IAT `0x004012a4`), with negative-remainder handling.
The output helper reads that stored periodic position at `0x00437e82` and
passes it to `GetSRT`. The isolated witness's direct `GetSRT(4000)` sample
checks the closing endpoint; this separate controller trace proves wrapping.

The builder optimizes keys via `0x1003b650`, registers SRT keys through the
D3DX set's vtable `+0x80` at `0x1003c696..0x1003c6a2`, and then calls the set's
compression path (`+0x84`) with flags zero, lossiness **0.1** (constant
`0x101357bc`), and null hierarchy at `0x1003c70d..0x1003c734`. A returned
buffer becomes a compressed set at `0x1003c768`; otherwise the uncompressed
set is retained at `0x1003c825..0x1003c829`. The PE
imports identify these thunks:

| Thunk | IAT | Import |
| --- | --- | --- |
| `0x100b9b46` | `0x10133280` | `D3DXCreateCompressedAnimationSet` |
| `0x100b9b4c` | `0x10133284` | `D3DXCreateKeyframedAnimationSet` |
| `0x100b9b52` | `0x10133288` | `D3DXMatrixRotationQuaternion` |

The native D3DX compressor is `0x0043c4ff`. Its retained rotation-key count is
clamped to at least five, or the original count when shorter
(`0x0043c7fe..0x0043c829`). It copies each retained time/XYZW tuple as five
unchanged float words (`0x0043c935..0x0043c94a`), then may flip all four
components together for hemisphere continuity (`0x0043c95d..0x0043ca1f`).
The compressed constructor points its rotation array directly at those
20-byte keys (`0x0043a149..0x0043a163`, `0x0043a208..0x0043a214`). There is no
quaternion quantization or second orientation conversion on this path.

Consequently Citymist's four authored rotation keys plus closing key are all
retained. The earlier EQ optimizer `0x1003b650` processes uniform-scale and
translation runs, leaving rotation keys alone. Citymist has constant scale
and translation within each track; collapsing those to one key does not
change its motion. Longer rotations and changing scale/translation require
their own reduction/optimization coverage. The Wine `d3dx9_36/animation.c`
stubs do not supply that evidence.

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

The selection call at `0x10044941..0x1004495a` is actor-data virtual `+0x48`.
The hierarchical actor constructor `0x10044660` installs actor-data vtable
`0x10139024` (`CHierarchicalActorDataClient`) at actor `+0xe0`; its `+0x48`
entry `0x100460a0` follows data `+0xa4` to the model, model virtual `+8`
(`0x10024360`) to the definition, then reads **definition byte `+0x18`**.
Clear selects shared grouping; set selects a private random-phase controller.

The classic WLD skeleton loader constructs that definition through
`0x1004adf0` at `0x1001e581`; the constructor clears byte `+0x18` at
`0x1004adfc`. The subsequent WLD initialization chain
`0x1004ae60 -> 0x100bffc0 -> 0x1004c8f0/0x1004afa0` leaves it clear, as do
the bounds pass and `0x1001a5d0` metadata setup. A separate EQG initializer
sets it for `ROOT_BONE` at `0x1004d5a9`; Citymist does not take that path.
This establishes the classic WLD tree's **shared controller, speed one,
initial phase zero**. In the shared path, `0x1003d610` aliases later actors'
node `+0x10` matrix pointers to the first actor's matching nodes. This proves
synchronized model-space motion for that animation definition, without
claiming untraced lifecycle resets or dynamic phase changes.

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

`CreateGraphicsEngine` returns the global engine in output-structure `+8`
at `0x10012483..0x1001248b`. In **eqgame.exe**, the host call is
`0x0093369e`, and `0x009336ca..0x009336cd` stores that output pointer at
`0x01822678`. The per-frame path `0x004bfd57..0x004bfd6e` calls host clock
`0x00897d90` and passes its returned value unchanged to graphics virtual
`+0x58`, the setter above. `0x004bef96..0x004befad` does the same; another
path passes the saved object `+0x154` clock at `0x004bf545..0x004bf557`.

Host `0x00897d90` dispatches through `0x00e04c84`. The on-disk default,
`0x00897d80`, returns `GetTickCount() - origin`, in milliseconds. Clock init
`0x00897df0` normally replaces this with `0x00897d20`: it reads
`QueryPerformanceFrequency`, divides it by **1000** at
`0x00897e14..0x00897e24`, and captures the QPC origin. The replacement
subtracts that origin and divides elapsed ticks by the stored
`frequency / 1000`. Both established sources therefore deliver milliseconds;
the QPC divisor has integer truncation. A separately calibrated RDTSC fallback
exists, but its precision was not established. The normal host clock plus the
unscaled graphics delta proves Citymist's four-second loop.

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

`ObjectSource::animation_period()` validates a deliberately narrow native
animation family, and `sample_animation(Duration)` returns fresh posed meshes
at an explicit shared time. The supported bounds are:

- Packed definition flags exactly 8; reference flags 4 without timing for
  one-frame tracks, or flags 5 with an explicit positive interval for animation.
- Two to four authored frames on every animated track, all with the same
  count and interval. The closing key fits D3DX's minimum-five-key retention.
- Constant scale and translation within each track; all poses finite with
  positive scale and nondegenerate quaternions. Adjacent/closing quaternion
  hemispheres must be unambiguous (`abs(normalized dot) >= 1e-6`).
- The closing timestamp is at most `2^24` milliseconds, so the native float
  key times are exact integers. Unknown timing/flags, long or mixed clips,
  float frame layouts, and moving scale/translation are rejected.

Time truncates to whole milliseconds, matching the host clock, then wraps by
the one animation-set period. Rotation blends the source components before
normalizing; normalizing each packed key before interpolation would subtly
change the native result. Pairwise quaternion signs include the closing seam.
This API does not select per-instance phases, advance a runtime controller,
upload vertices, alter bounds or rebuild collision. Production actors continue
to use their first pose.

The 16 focused CPU checks include synthetic rigid and weighted parts,
parent/local transforms, immutable independent actor bakes, bad selections,
bad later poses that leave frame zero usable, and two original-asset tests.
For the original tree, all four selected poses change branches while leaving
every trunk vertex and its 42 collision triangles unchanged.
The time tests cover key boundaries and closure, intermediate nlerp, equivalent
quaternion signs, shared periods, millisecond truncation, unsupported inputs,
source/binding immutability, and a matrix produced by the isolated native BR1
witness. The original Citymist test verifies a four-second period, matching
whole-second authored poses and intermediate motion with a static trunk.

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

Autonomous playback remains disabled. Future renderer integration needs
explicit source-vertex bindings through material baking, animated bounds and
a collision policy for actors whose
collidable parts move; the static Citymist trunk does not establish a general
collision policy.
