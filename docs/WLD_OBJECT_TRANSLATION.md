# WLD five-frame object translation

The lightning between the two electric monument obelisks now rises and falls
through the existing placed-object animation path. All three original mesh
parts remain present, and the two obelisks' 200 collidable triangles stay fixed.
This extends the bounded five-frame animation family; it does not relax the
stationary-collision gate or admit general moving WLD actors.

## Original sources

The installed inventory contains these five actor definitions. WLD digests
identify the original decompressed member, not a committed asset fixture.

| Archive | Actor | Period | WLD SHA256 |
| --- | --- | ---: | --- |
| `eastwastes_obj.s3d` | `electmonu200` | 1665 ms | `69f4724846852b13e2640ab4cc1109367b1d0b5bc6fbe327d80360dce55c6ba5` |
| `eastwastesshard_obj.s3d` | `electmonu200` | 1665 ms | `69f4724846852b13e2640ab4cc1109367b1d0b5bc6fbe327d80360dce55c6ba5` |
| `westwastes_obj.s3d` | `electmonu200` | 1665 ms | `bda614c9a34f6c7cf09751c0b22828b5220f31cfc50d93bf3aeaf0ad7adf62b4` |
| `necropolis_2_obj.s3d` | `electmonu201` | 5000 ms | `5425521902ead668485b4f8ed74246581c645291d4568b10b500ef3580e6439f` |
| `sleeper_2_obj.s3d` | `electmonu201` | 5000 ms | `356b19d616a92ddb63b8d1cab0aa948c1a39e2585ba3cfac8d7dad3defa9f698` |

Each has a root, two static obelisk tracks, and one translating lightning
track. Three rigid meshes contain 204 triangles: 100 collidable triangles per
obelisk and four noncollidable lightning triangles. No particle attachments
are involved. The classic placement survey finds three `electmonu200`
placements, one in each matching zone. It finds no same-family placements for
the supplemental `electmonu201` definitions; that is not evidence that no
runtime or cross-library caller can use them.

The lightning track's five packed positions are `(0, 74, z) / 256`, with
`z = [8179, 9423, 11033, 11959, 10156]`. Its raw packed WXYZ quaternion is
constant `[-8192, -8192, 8192, 8192]`, and scale is one. Authored intervals are
333 ms for `electmonu200` and 1000 ms for `electmonu201`.

## Executed native path

The execution setup follows
[WLD_LONG_OBJECT_ANIMATION.md](WLD_LONG_OBJECT_ANIMATION.md). Unicorn executes
the original EQ key builder `0x1003c150`, including the scale/translation
optimizer `0x1003b650`, native D3DX registration, compression, and compressed
set construction. It stops at `0x1003c76d`; native `GetSRT` at `0x0043bb8d`
and quaternion normalization are then called separately.

| Binary | Image base | SHA256 |
| --- | --- | --- |
| Installed `EQGraphicsDX9.dll` | `0x10000000` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| Microsoft x86 `d3dx9_30.dll` | `0x00400000` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |

Inputs are controlled decoded original packed tracks, using the established
frame-zero W convention. Allocation/free and the compressed constructor's
name/index `qsort` are host substitutes; motion math and key ranking execute
original instructions. The archive loader, live animation controller, and
native GPU pipeline are outside this experiment. These short clips also avoid
the loader's conditional interval halving for tracks longer than 15 frames.

The EQ optimizer compares neighboring translation components against constants
`-0.0001` and `+0.0001` at `0x10138c10` and `0x10138c14`. It can remove keys
from nearly equal runs before D3DX receives them. This implementation excludes
such runs instead of approximating the general optimizer.

D3DX ranks translation keys with `0x0043bcd0`, separately from the rotation
ranker `0x0043bed1`. Candidate error helper `0x0043b1c5` interpolates the two
immediate neighbors at the candidate time, stores that position as `f32`, then
computes squared position error in Z, Y, X order using x87 intermediates. The
ranker stores each initial score as `f32`. The translation rank pointer is at
native track offset `+0x30`; rotation ranks are at `+0x28`.

As with five-frame rotation, the appended closing frame gives six keys.
Lossiness 0.1 retains five, so only the first lowest-error interior key is
discarded. Endpoints retain their original times. Subsequent neighbor-weight
updates cannot change this single omission.

For the monuments, the four initial translation errors are:

```text
0.5110015869140625
1.78472900390625
28.409732818603516
0.1154937744140625
```

Native translation ranks are `[0, 4, 3, 2, 5, 1]`: authored index 4 is removed.
The constant rotation channel independently removes index 1. Reusing the
rotation omission for translation gives the wrong motion. At 1332 ms in the
333-ms clip, native height is `39.33203125`, which differs from the discarded
authored height by `0.33984375`.

Native vector interpolation `0x0043b017` keeps the fraction in wider arithmetic
and stores each final component as `f32`. The production sampler uses `f64`
arithmetic at these boundaries and preserves the five-interval loop period.

## Conservative admission and bounds

Existing packed layout, reference flags, shared timeline, valid quaternion,
positive scale, and exact timestamp limits remain required. A track with
changing translation additionally requires:

- Exactly five frames, with identical rotation and scale in every frame.
- Each translation component lies exactly on the signed packed16 grid.
- Every neighboring position differs, including the last-to-first closure.
- Each candidate's squared component error and each Z/Y/X partial sum is
  exactly representable as `f32`.
- A unique smallest initial translation error.

Distinct packed positions differ by at least `1/256` in one component, which
exceeds EQ's near-equality threshold. Thus all six translation keys survive
the pre-optimizer. Packed positions, initial midpoints, and differences are
already exact `f32`; requiring exact squares and sums makes this admitted
first-removal ranking independent of x87 precision. This deliberately rejects
many otherwise valid translations rather than generalizing from a close-score
heuristic. It does not claim bit-identical interpolation in every CPU mode.

Animation bounds use the maximum translation norm across all authored frames
at every ancestry level. Linear interpolation stays inside their convex hull,
including the span across an omitted key and the closing segment. Existing
positive-scale composition and roundoff slack remain in place. Collidable
vertices still reject live animation if any track in their binding ancestry
has multiple frames, including invisible collision faces.

Rejected motion stays at its existing first pose. Longer clips, adjacent
repetitions, tied or inexact translation errors, changing rotation together
with translation, changing scale, and moving collision remain outside scope.

## Validation

The full native monument probe covers five definitions and 20 tracks, with
1756 samples per mode. Scalar x87 control word `0x037f`, SSE/`0x037f`, and
SSE2/`0x037f` translations match the wider scalar model bit for bit. Scalar
`0x007f` has maximum component error `3.814697265625e-6`; retained arrays and
rank choices stay identical. This does not establish which mode the complete
client chooses, AMD 3DNow behavior, or nondefault rounding modes.

An additional seeded probe generates 128 accepted three-axis packed tracks,
32 for each possible omitted index 1 through 4. Each executes the complete
native builder/compressor under scalar and SSE2 with both `0x037f` and
`0x007f`. All candidate scores, first omissions, and retained timestamps match
the independent model. Four compact synthetic witnesses are ordinary Rust
regressions. Root independently replayed the full-monument and 128-control
probes under scalar/`0x037f` and SSE2/`0x037f`; all four JSON results byte-match.

Independent review compiled the actual production translation helper through
an out-of-repository Rust wrapper. It matched all 128 native omitted indices
and executed native build/compression/`GetSRT` for a controlled original-style
translation channel at every millisecond of both monument periods. This
single-track witness uses the original translation values with identity
rotation and scale one; the five-actor probe separately covers the authored
quaternion. All 6667 samples, including closing endpoints, match Rust bit for
bit under scalar x87 `0x037f`.

The focused WLD object asset suite passes 52 tests with original-asset opt-ins
enabled. New coverage includes independent channel omissions, original sample
bits, all four omission positions, admission exclusions, the complete
1665-ms synthetic bound sweep, unchanged moving-collision rejection, and all
five original definitions with 200 stationary physical triangles each.

The original Eastwastes GPU regression validates the real placed binding,
then isolates the monument at the origin with a fixed camera and disabled sky.
It truncates material texture lists to one frame, so texture animation cannot
produce a false geometry-motion pass. At 0/333/666/999/1332/1665 ms the changed
pixel counts are `0/1617/1920/1925/1828/0`. The loop closes exactly, source
collision vertices remain unchanged, and all sampled vertices lie inside GPU
bounds. Inspection of the 0-ms and 999-ms captures shows the same lightning
texture rising while the pillars stay fixed. This establishes OpenEQ geometry
upload and rendering, not native framebuffer parity.

```sh
CARGO_INCREMENTAL=0 EQ_DIR="$HOME/EverQuest" cargo test -p openeq-assets --lib loader::wld_objects -- --include-ignored
CARGO_INCREMENTAL=0 EQ_DIR="$HOME/EverQuest" cargo test -p openeq-render --test wld_translating_objects -- --include-ignored --nocapture
```

## Frozen local evidence

The following research artifacts are outside the repository and read-only.
They reference installed assets and the earlier native helper; no original
archive payloads or binaries are committed. Independent replay should use
separate output paths rather than overwrite frozen evidence.

| `/tmp/` artifact | SHA256 |
| --- | --- |
| `openeq-native-long-animation.py` (reused helper) | `4e1a0f4d323122547949343cbf5e90a2ffdcafaecc19835ad88367ddf3eefd5e` |
| `openeq-native-translating-object.py` | `121d978f80491083ec8815cfae2b6251543e6a63a21ce9004fb3ff280e4618bc` |
| `openeq-native-translating-object-scalar-0x37f.json` | `a4b0577a44636a3a76f49349a8d26640446736ef209d103a7bb5303f4ab56d1f` |
| `openeq-native-translating-object-scalar-0x7f.json` | `b905c6ce2742933b7903c063e5efe1aa6bcb1bb8e0e4dad99e769d83067080c4` |
| `openeq-native-translating-object-sse-0x37f.json` | `64f21d27734424af36fcf2f900281d0194a2992ae48733e7aa0e4dbc95a0fdf3` |
| `openeq-native-translating-object-sse2-0x37f.json` | `694de7952255c8c9c22301078b1554fc1bcac876745ddab5d6aa148558903ca4` |
| `openeq-native-translation-controls.py` | `d34fba7bb970747d1831ec64fe48e943636b94af8b785e8551ef7233f45774f2` |
| `openeq-native-translation-controls-scalar-0x37f.json` | `d39cf3963c07cbb853404310211d13a4ed2633aa407a57a07bae02323514c2f3` |
| `openeq-native-translation-controls-scalar-0x7f.json` | `ea5581fa19e6bab51af0a6449d8696d1c8b8d489b18cfab25c98cf9e7c8c0d1c` |
| `openeq-native-translation-controls-sse2-0x37f.json` | `a2981fe235b6426007df14ac81b47edb9f8585ed8ca16f4d4593ce282398b2ec` |
| `openeq-native-translation-controls-sse2-0x7f.json` | `5e4788958d6b0b18859ffba8e5c850f758adc9e578d364941ec8a6ab17117551` |
| `openeq-translation-review-native.py` | `000414ced510528acf9490fb1274a1dc2e20f66e2eea682572c11653b4d9796e` |
| `openeq-translation-review-native.json` | `0a0bf07cf3be05c8884a1358d0d512d9d9a70aea86818f66f65dd44094450822` |
| `openeq-translation-review.rs` | `971973b5e47ec0ad116f40a4cddaeb42eb80d9ceead45c396ffc493df99c8db1` |
| `openeq-translation-review-rust.txt` | `f1f687330fda641b6f68166afe735d381267edf31b52446f9d950687bb7e5e54` |

## Other candidates examined

The ancestry audit of the historical 173 unsupported animated actors found
26 candidates with stationary collision or no collision: the previously
studied 18 five-frame rotation definitions, these five translating monuments,
and three longer clips (`VSCRATE103`, `TEMPLELIFE`, `KRLAMP101`). Only the crate
changes translation; the other two change rotation. The later bounded lamp
extension is documented in [WLD_LAMP_ANIMATION.md](WLD_LAMP_ANIMATION.md).

The 14-frame `TREEPINE103`/`WARDPINE101` family initially looked promising, but
each has 32 collidable animated branch triangles as well as ten static trunk
triangles. All 21 inventoried pine occurrences fail the unchanged collision
gate. Native 15-to-13 key reduction was examined but was not integrated.

Likewise, `CDTORCH502` in Codecay/Codecayb (135 placements each), `POEDMND500`
in Poeartha (28), and `POSTRMTRNDO500` in Postorms (one) have collidable vertices
under animated tracks. Their respective clips have 21, 1281, and 821 frames.
The latter two also carry four and five particle attachments. Relaxing only
particle admission would not establish safe live animation for these actors.

| `/tmp/` audit artifact | SHA256 |
| --- | --- |
| `openeq-animated-particle-actor-audit.py` | `ac967a9869f6f0759c5573d896e91d39dc6df0180cb034c5b11e11f0391536ec` |
| `openeq-animated-particle-actor-audit.json` | `e1e0920b8143c8f8dfd95ee39bcfb2702f59f7bd6a8a92c6bc3e403204cd3bd0` |
| `openeq-long-actor-ancestry-audit.py` | `dafb2d4773eb2a19d0fb0d34dcafd4179f67eab3c23d90bfb8681e38ce300707` |
| `openeq-long-actor-ancestry-audit.json` | `07c8c58b344f2da8fc90cc80d90821de32e1ca876b66f73c3cb751db7ebfe48e` |
| `openeq-native-pine-animation.py` | `4b86f255b853dfd467f4ca73611168e8102b21ef180a0231b842d27dd3459842` |
| `openeq-native-pine-animation-scalar-0x37f.json` | `2e08349c20c56a907309589db4f00785ac7a30a21c55909b99455cda9eb779c0` |
