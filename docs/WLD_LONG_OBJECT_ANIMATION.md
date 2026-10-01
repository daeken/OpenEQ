# WLD five-frame object animation: native key reduction

Five authored rotation frames are the smallest extension beyond the original
short-clip sampler. The native client appends a closing key, then compresses
those six keys to five. It does **not** always remove a repeated first pose.
This document establishes the one-key reduction independently of renderer
integration. Longer tracks and changing scale or translation remain outside
this evidence. The structural stationary-collision gate remains required.

## Original binaries and execution boundary

The binaries and decoded-track conventions are the same as
[WLD_OBJECT_ANIMATION.md](WLD_OBJECT_ANIMATION.md):

| Binary | SHA256 |
| --- | --- |
| Installed `EQGraphicsDX9.dll`, base `0x10000000` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| Microsoft x86 `d3dx9_30.dll`, base `0x00400000` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |

Unicorn 2.1.4 executes the original EQ key builder at `0x1003c150`, including
the EQ scale/translation optimizer at `0x1003b650`, native D3DX registration,
compression, and compressed-set construction. Execution stops at
`0x1003c76d`, immediately after successful compressed-set creation. Native
`GetSRT` and quaternion normalization are then called separately.

Inputs are decoded tracks constructed from the original packed WLD records,
with the already-established frame-zero W sign adjustment. This experiment
does not execute the archive loader, live host controller, renderer, or GPU.
Heap allocation/free are controlled host substitutes. The compressed-set
constructor's CRT `qsort` import is replaced only for its name/index table,
using the byte-string comparison proven by comparator `0x00556a3b`; no motion
key ranking or math routine is replaced. The two graphics imports are bound
to the original Microsoft D3DX exports.

## Concrete witness: Dreadlands TREE105

The source is `dreadlands_obj.s3d/dreadlands_obj.wld`, WLD SHA256
`b960e25f53aac822c8216ad41238c7172878134c036f5958c0848e14e63d1f65`,
skeleton `TREE105_HS_DEF`, actor `TREE105_ACTORDEF`. Its original zone
metadata contains 34 placements.

It has eight tracks, seven rigid meshes, 71 vertices, and 48 triangles.
Root and trunk have one frame, reference flags 4, and no timing word.
Each of six branches has five frames, reference flags 5, and interval 1000.
Every definition uses packed-frame flags 8 and constant scale/translation.
Only the trunk's 18 triangles are collidable. Its parent is the one-frame
root; no collidable vertex has an animated ancestor.

The original builder produces rotation keys at 0, 1000, 2000, 3000, 4000 and
5000 ms. Native optimization reduces each constant scale/translation channel
to one key. The shared period remains 5000 ms after compression.

| Branch | Original key ranks, including closure | Removed key time |
| --- | --- | ---: |
| `TR5MBR1_DAG` | 0, 5, 3, 4, 2, 1 | 1000 ms |
| `TR5MBR2_DAG` | 0, 4, 3, 5, 2, 1 | 3000 ms |
| `TR5MBR6_DAG` | 0, 5, 2, 3, 4, 1 | 1000 ms |
| `TR5MBR5_DAG` | 0, 3, 5, 2, 4, 1 | 2000 ms |
| `TR5MBR4_DAG` | 0, 3, 5, 2, 4, 1 | 2000 ms |
| `TR5MBR3_DAG` | 0, 5, 3, 2, 4, 1 | 1000 ms |

The first two authored poses are identical on every branch, but BR2, BR5 and
BR4 retain that pause and remove a later key. Native compressed sampling was
checked at 125-ms intervals, including the closing endpoint, for all branches;
the static tracks were checked at both endpoints. Across 250 samples, native
normalized quaternions match interpolation between the retained keys to less
than `4.2e-8` per component. Ignoring compression differs by as much as
`0.0043011` per quaternion component on this actor.

## The bounded six-to-five rule

EQ calls D3DX compressor `0x0043c4ff` with flags 0, lossiness 0.1 and null
hierarchy. The threshold at `0x0040141c` is 0.5. At lossiness 0.1, the branch
at `0x0043c574..0x0043c58b` selects algorithm 1, whose rotation rank function
is `0x0043bed1`. This is the **remove-lowest-error** algorithm; algorithm 0's
split-largest-error routine `0x0043c379` is a different path.

For each channel, the retained count is the integer truncation of
`original_count * f32(1 - lossiness)`, clamped to at least five and at most the
original count. Six rotation keys therefore retain five. Endpoint ranks are
fixed at 0 and 1. The first removed interior key gets rank 5. Later ranking
iterations cannot change which key is omitted when retaining five.

The initial interior candidate errors come from `0x0043b27a`:

1. Interpolate the candidate's two immediate neighbor quaternions at the
   candidate timestamp. Store each interpolated component as `f32`.
2. Normalize that interpolated quaternion with D3DX's normalizer.
3. Compute its dot product with the **un-normalized authored candidate**,
   clamp the dot to [-1, 1], and compute `1 - dot * dot`.
4. Store this error as `f32` before comparing candidates.

All timestamps in the bounded family are equally spaced, so the initial
interpolation fraction is exactly 0.5. No time-span weighting is applied to
these initial candidates. The weighting at `0x0043c00c..0x0043c035` and
`0x0043c06e..0x0043c091` belongs to subsequent updates after a removal; it is
irrelevant to the six-to-five selection and must not be generalized from this
milestone to larger key arrays.

Candidates enter the min-heap in ascending source index. The sift-up at
`0x0043ad75` swaps only on strict less-than. Thus, for this first removal,
an equal minimum keeps the **earliest source index**. TREE105 BR4 has an exact
tie at indices 2 and 4 and removes index 2. The corpus contains 14 tied tracks.

Retained keys keep their original timestamps and quaternion magnitudes.
Compression copies each 20-byte time/XYZW tuple, then reapplies adjacent
hemisphere continuity at `0x0043c95d..0x0043ca1f`. Interpolation must bracket
the retained timestamps: the interval crossing a removed key spans two
original intervals. The closing key remains at five intervals; shortening the
period to four intervals is incorrect.

### Floating-point details

The scalar normalizer is `0x004451e3`. It stores squared length as `f32` and
returns the vector unchanged when that squared length lies within inclusive
`1 +/- 2^-23`. Otherwise it computes inverse square root and rounds each
result component to `f32`. Squared lengths at or below `2^-126` produce zero.
The error function evaluates its dot in W, Z, Y, X order and only rounds the
final error to `f32`. Prematurely normalizing the authored candidate, computing
everything with `f32` vector helpers, or ignoring the near-unit fast path can
alter the ordering of very small errors.

A separate scalar model, using wider intermediates at the native store
boundaries, matches all **432 initial error values bit for bit** in the
installed five-frame constant-scale/translation corpus. This is concrete
corpus evidence, not a proof that every possible quaternion rounds identically
on every processor.

The corpus was also executed using D3DX's original SSE table installer
`0x00610c5d` and SSE2 installer `0x006132ef`; both select normalizer
`0x006117fd`. Their errors differ from the scalar path by up to `2.56e-7`,
but **all 108 removed-key selections are identical**. Additional scalar
x87 control words `0x027f` and `0x007f`, and SSE2 with `0x007f`, also preserve
every selection. The primary scalar witness uses `0x037f`. The experiment
does not establish which CPU mode the complete running client chooses, or
cover AMD 3DNow, allocation failure, or non-default rounding modes.

## Installed extension candidates and collision

The original survey's 173 unsupported animated actors contain 23 occurrences
whose animated tracks have five frames. Five of these change scale or
translation and remain excluded. The remaining 18 all have interval 1000,
packed flags 8, static reference flags 4 and animated reference flags 5.
Original native build/compression succeeds for all 18, covering 108 animated
tracks. Every retained rotation array has five keys and every set retains a
five-second period.

| Actor key | Archive occurrences | Metadata placements |
| --- | ---: | ---: |
| `tree105` | 4 | 193 |
| `swamptree100` | 4 | 1,379 |
| `swamptree101` | 2 | 2,007 |
| `swamptree102` | 2 | 2,028 |
| `purptree200` | 3 | 224 |
| `tree106` | 3 | 178 |
| **Total** | **18** | **6,009** |

These occurrences are in Dreadlands, Firiona, Growthplane, Mischiefplane,
Pomischief, and Swamp of No Hope. They use rigid node meshes with no
vertex-piece rebinding. An independent topology audit found no collidable
triangle with a multiple-frame track anywhere in its binding ancestry.
Counts are archive occurrences and original metadata placements, including
aliases; they are not unique content or observed live render instances.

The smallest defensible extension is therefore five authored frames, with
the existing uniform shared timeline, packed layout, reference flags,
constant scale/translation, quaternion validity, and exact timestamp limits,
plus the one-key reduction above. Keep the existing structural collision gate
unchanged. Static flags 0, six or more authored frames, mixed timelines,
changing scale/translation, and moving collision remain separate work.

## Conservative production admission

The implemented extension deliberately admits a subset of those 18 research
candidates. Close but unrelated errors can change order with CPU precision.
An independent native ranker replay found this concrete packed XYZW clip:

```text
[20, -16, 4, 16383]
[21, -14, 5, 16383]
[21, -15, 5, 16383]
[21, -16, 5, 16383]
[20, -15, 5, 16383]
```

After division by 16384, sign handling and closure, scalar x87 PC64/PC53 and
SSE2 with PC64 remove index 3. Scalar PC24 and SSE2 with PC24 remove index 2.
The original ranker itself was executed; this is not an inference from scores.
Thus, even packed, near-unit, small-motion quaternions need an admission limit.

Five-frame tracks now additionally require:

- Each source component lies on the signed 16-bit packed grid; squared
  quaternion length is within 0.01 of one.
- Every initial neighbor midpoint has squared length at least 0.5, keeping
  normalization away from cancellation and denormal cases.
- Each competing error exceeds the selected minimum by more than `1e-5`,
  unless its authored quaternion and pre-normalization midpoint are both
  identical to the selected candidate's, allowing one common sign flip.
- Newly adjacent retained keys pass the same quaternion-hemisphere ambiguity
  check as authored neighbors. Hemisphere continuity is reapplied afterward.

The separation threshold is a deliberately conservative compatibility policy,
not a universal floating-point error proof. The native scalar ordering is
the implemented reference inside these bounds. Equal scalar scores alone do
not establish a safe tie; identical error inputs do.

The public source APIs and original-asset regression admit **9 definitions
covering 2,424 metadata placements**: `tree105` in four archives,
`purptree200` in three, and `swamptree101` in two. `swamptree100`,
`swamptree102`, and these five-frame `tree106` occurrences stay at their first
pose because at least one branch has insufficient score separation. Existing
short-clip support is unchanged. The collision/bounds implementation was not
modified; moving physical vertices still prevent live animation.

Four new ordinary regressions cover native sampled outputs for all six
TREE105 branches, exact retained key times, the BR4 structural tie, equivalent
quaternion signs, five-second closure, the native precision counterexample,
unproven source data, a newly adjacent half-turn, and unchanged collision
policy. All 34 runnable WLD object tests pass. The explicit installed-asset
test checks all 18 candidate definitions, their final admission decisions,
stationary physical vertices, and animation bounds.

The separate renderer regression
`crates/openeq-render/tests/wld_five_frame_objects.rs` also passes using original
Dreadlands assets: 34 placed TREE105 actors, 48 triangles each, all 18 physical
triangles per actor unchanged at every checked pose, bounded animated vertices,
and pixel-exact closure after five seconds. Intermediate samples change
2,842 / 3,363 / 3,373 image pixels at 1,250 / 2,500 / 3,750 ms. The local GPU
log is `/tmp/openeq-five-frame-gpu.log`; reviewed captures are under
`/tmp/openeq-dreadlands-five-frame-tree/`. This is an isolated original-asset
render regression, not a claim of a complete live-client traversal.

## Frozen temporary evidence

The files below are local research artifacts, not repository fixtures. They
contain no copied archive members or original executable binaries.

| File under `/tmp/` | SHA256 |
| --- | --- |
| `openeq-native-long-animation.py` | `4e1a0f4d323122547949343cbf5e90a2ffdcafaecc19835ad88367ddf3eefd5e` |
| `openeq-native-long-animation.json` | `eb0c414b31c7188d315c258b771d0d8b88085bc53dea9d71e879b0ba9a633d3b` |
| `openeq-native-long-animation-corpus.py` | `94bd7b68454d63297cb2b47492fba635f5733b49a9fd0f42c063c10a6c5dcfb7` |
| `openeq-native-long-animation-corpus.json` | `cc23b473ca90e4266c8ab2f05680ed9bb4b8b48dc0dd379f2cb647e5cbb1154e` |
| `openeq-native-long-animation-corpus-sse.json` | `6bc11ffdcd808f879c62e6928cd08c5eef7ea2c8e2b950f78db90a5483bb3647` |
| `openeq-native-long-animation-corpus-sse2.json` | `fdf86dbed20f3012c48ead4294e1835aa1f6144921b83844de9358477196cbad` |
| `openeq-native-long-animation-corpus-0x27f.json` | `38864abb4ce60e8fdf76bc3a8ade12076baca915b1794f405c88da783cae7557` |
| `openeq-native-long-animation-corpus-0x7f.json` | `b7e0ace6b6a151c2c6e97cca536984996b58c3c86f085ad8ea08cae46c8a9d2e` |
| `openeq-native-long-animation-corpus-sse2-0x7f.json` | `9de13ad0efcc1faabe55fd180db9619616fca2644b488ff0c1f3dfcb70fb347c` |
| `openeq-native-five-frame-rounding-review.py` | `dcaadbe2db499ec984c9b0e0fad594157aae38b32eaf497236260cd450bd219b` |
| `openeq-native-five-frame-rounding-review.json` | `a665f4f143c4b9190c534251ba9947d9ee05037b190900392952fe6fbeb5f383` |

The corpus script reuses only the emulator/reader definitions before the
concrete witness body; it does not execute or overwrite that witness. Original
archive ownership and placement counts are anchored to the frozen
[WLD animation survey](WLD_ANIMATION_SURVEY.md).
