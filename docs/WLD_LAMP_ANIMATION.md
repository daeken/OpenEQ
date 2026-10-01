# WLD hanging-lamp animation: bounded scalar reduction

`KRLAMP101` now swings through the existing placed-object animation path in
Swamp of No Hope. The original zone has 36 placements. Both mesh parts remain
present: the support pole stays fixed while the lamp rotates on its authored
1600-ms loop. All 185 triangles are noncollidable; the existing structural
moving-collision rejection is unchanged.

This is a bounded 16-frame, single-axis rotation extension with an explicit
scalar numerical reference. It neither implements a general long-track
compressor nor claims identical key selection across every native CPU path.
Existing short clips and the five-frame reduction/admission rules are unchanged.

## Original candidates and loader timing

The remaining stationary/no-collision entries from the historical unsupported
actor survey are three different motion families, not three translating actors:

| Actor / archive | Meshes / vertices / triangles | Frames / source interval | Motion | Metadata placements |
| --- | --- | --- | --- | ---: |
| `VSCRATE103`, `overthere_obj.s3d` | 1 / 20 / 12 | 20 / 200 ms | Rotation and translation | 2 |
| `TEMPLELIFE`, `qeynos2_obj.s3d` | 1 / 412 / 372 | 40 / 200 ms | Rotation; fixed translation and scale | 1 |
| `KRLAMP101`, `swampofnohope_obj.s3d` | 2 / 220 / 185 | 16 / 100 ms | Rotation; fixed translation and scale | 36 |

All three have zero collidable triangles. The lamp has three tracks: one-frame
root and support pole, and a 16-frame lamp track rotating around X. Its packed
frames have flag 8; track references use 4 for static tracks and 5 for animation.
There are no particle attachments.

| Original WLD member | SHA256 |
| --- | --- |
| `overthere_obj.wld` | `4ed3874f2d88933ac9aa5b404715df9378d57c2927cbca49e61f834402cc3775` |
| `qeynos2_obj.wld` | `c536ef29ff1231d63d05134fe41b4ed2847d3e4b901f85bdd409aa9d417163c2` |
| `swampofnohope_obj.wld` | `3c6ed1003868afed70627a2f156e5e1a2238b1287306d1582dcacb8b27e948c0` |

Timing was established by execution, rather than applying the known
greater-than-15-frame halving branch by assumption. The original `eqgame.exe`
object-zone-load block `0x004be442..0x004be837` and its full-WLD wrapper execute
for zone IDs 93, 2, and 83, using installed file existence. Each requests its
object WLD through graphics endpoint `0x100675f0` at resource scope 2 with flags
`[1, 0]`. Archive/path/progress/CRT calls and the final graphics endpoint are
controlled boundaries in this upstream witness.

A separate downstream witness executes original WLD setup `0x100c1030`, full
header/fragment/string decoding, and track handler `0x1001ae20` for every track
of the three actors. Original member bytes are supplied through a controlled
archive read interface; every native fragment kind, name and body is compared
against the independent reader. Decoded frame components match the original
packed records exactly.

Setup stores the **second** byte flag at loader `+0x30` (`0x100c10b7`) and the
first at `+0x31` (`0x100c10b4`). Thus these object loads have `+0x30 = 0` and do
not halve intervals. Controls with setup flags `[0, 0]` also preserve timing;
`[1, 1]` takes `shr interval` at `0x1001b05a` for these long tracks. Static
tracks retain their default 100-ms field in all three controls. The actual
object periods are therefore 4000, 8000, and 1600 ms. This does not establish
the timing policy for other loading contexts or character animations.

## Native compression evidence

Binary identities and the builder boundary match
[WLD_OBJECT_TRANSLATION.md](WLD_OBJECT_TRANSLATION.md):

| Binary | SHA256 |
| --- | --- |
| Installed `EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| Installed `eqgame.exe` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| Microsoft x86 `d3dx9_30.dll` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |

The verified decoded tracks feed original EQ builder `0x1003c150`, its
optimizer, native D3DX registration/ranking/compression, and compressed-set
construction. Execution stops at `0x1003c76d`, then calls native `GetSRT` and
normalization separately. The established skeleton-loader frame-zero W
adjustment is supplied explicitly here; native skeleton assembly, the live
controller, and the native renderer do not execute. Allocation/free and the
constructor's name/index sort are controlled; key-ranking math is native.

| Actor | Scalar x87 PC64 result | Other-mode result |
| --- | --- | --- |
| Crate | Rotation 21→18 keys; translation 20→17 | Translation retains 18 under PC24 |
| Temple | Rotation 41→36, removes 16, 22, 2, 7, 31 | Same removals in four tested modes |
| Lamp | Rotation 17→15, removes 1, 8 | Same removals in four tested modes |

The four modes are scalar and SSE2, each with x87 control words `0x037f` and
`0x007f`. Source indices include the appended closure. The crate's translation
channel has already lost one repeated key in EQ's optimizer. Its retained
count then sits at a precision-sensitive integer boundary: the wider product
of 20 and the stored `f32(1 - 0.1)` is below 18, whereas PC24 rounds it to 18
before truncation. The crate remains excluded. Temple requires five removals,
with tied and closely separated candidate errors, and also remains excluded.

For the lamp, native initial zero-error indices are `1, 6, 7, 8, 9, 14`.
The first removal is index 1. After the heap swaps its last node into the root
and restores heap order, the next selected zero is index 8, not the next
source index. Applying the five-frame earliest-tie rule repeatedly is wrong.
Native heap insertion/sift-up `0x0043ad75` and sift-down `0x0043adaf` use strict
less-than comparisons, with the left child considered before the right.

After a removal, native ranking recomputes errors at the surviving neighbors
and multiplies each by its new neighbor time span. The lamp's index-2 error
increases from about `9.34248e-5` to `0.0301993`; its positive category stays
unchanged. Endpoints remain protected and all retained keys keep their original
timestamps, including closure at 1600 ms.

## Implemented admission contract

The new helper accepts exactly 16 packed frames with constant translation and
scale and exactly one active XYZ rotation axis across the clip. Source squared
quaternion length must be within 0.01 of one. Existing shared positive timing,
track flags, finite positive scale, original/reduced hemisphere checks, and
stationary collision ancestry checks still apply. The total period remains
bounded by the existing exact-timestamp limit of `2^24` milliseconds.

OpenEQ uses the native **scalar x87 PC64 path as its numerical reference**,
implemented with wider `f64` arithmetic at the original `f32` store boundaries.
This is an explicit compatibility policy, not emulation of arbitrary CPU
math instructions or a proof of bit identity for all admitted inputs.

Each candidate is classified using the scalar midpoint/normalization rule:

- Midpoint squared length must lie in `[0.5, 1.5]`.
- Nonzero intermediate values within `1e-12` of an `f32` rounding midpoint are
  rejected. Normalization retains the native stored-squared-length near-unit
  fast path.
- A candidate is zero-error when the absolute normalized-midpoint/authored dot
  is at least `1 + 1e-6`, so the native clamp produces zero. An authored exact
  unit-axis key whose corresponding normalized component is exactly equal
  also has an exact scalar dot of one.
- It is positive-error when the absolute dot is at most `1 - 1e-6`. All other
  cases are rejected.

These margins are conservative admission policy; they are not a general
native floating-point error theorem. They preserve a clear scalar reference
and reject near-boundary classification choices.

The heap stores only zero/positive categories. This abstraction is sufficient
for two zero removals: swapping two positive nodes cannot move a zero; a
positive node cannot pass a zero during heap maintenance; equal zero nodes do
not swap. Insertion and removal therefore give the same zero-node positions
regardless of ordering among positive errors. After the first removal, both
surviving neighbors must retain their original categories when recomputed.
Their positive time-span factors preserve those categories too. This rules
out an update introducing a new zero whose location could depend on prior
positive ordering. Both selected removals must be zero. No general weighted
heap or additional removals are implemented.

The sampler brackets retained original timestamps, including consecutive
omissions and the closing segment, blends with wider arithmetic, and lets the
existing pose path normalize and compose the rotation. Rejected clips retain
their first pose and source metadata. Existing bounds remain valid because
translation and positive uniform scale stay constant in this family.

## CPU limits and validation

SSE normalizer `0x006117fd` uses `rsqrtss` followed by one Newton refinement.
An explicit sensitivity experiment replaces only the initial reciprocal-square-
root estimate, then runs the original refinement and builder/compressor.
Estimates with relative error less than `1.5 * 2^-12` can make the lamp remove
`[6, 8]` instead of `[1, 8]`. This is a controlled arithmetic sensitivity
experiment, **not an observation of another physical CPU**. It demonstrates
why matching the tested scalar/SSE modes does not justify universal native
CPU parity, and why the scalar reference is stated explicitly.

The original lamp plus 256 generated packed single-axis controls match the
model's first two removals under all four tested modes. The generated controls
cover every possible first removal index 1–14 and second index 2–15. They
exercise different axes, consecutive omissions, closing spans, intervals
1/17/100/333/1000, clamp exclusions, and changed-neighbor rejections.

A separate pure-model audit exhausts all 32,752 fifteen-node zero masks with
at least two zeros. Four positive-score orderings and arbitrary positive
neighbor updates give 131,008 comparisons with identical first two removals.
This audits the abstraction independently from numeric sampling; it does not
replace the executable native controls.

Independent review adds 513 admitted full-angle controls, generated from
17,154 candidates with sign changes and quaternion scales 0.998/1/1.002.
All omissions match the original native scalar path. This broadens validation
beyond the small angles in the first generated corpus. The reviewer also
compiles the actual Rust helper and compares 21,328 native key, midpoint and
pre-boundary samples. Worst quaternion-component error is `1.7881393e-7`.
The harness converts the native decoder's negative-W convention back to the
authored WLD convention before invoking the helper. Both the admission gate
and the strict-heap category argument passed independent review.

The focused WLD object suite passes 57 tests with original-asset opt-ins.
New ordinary cases cover native sampled lamp quaternions, exact loop period,
equivalent quaternion signs, consecutive omissions at either end, meaningful
negative gates, first-pose fallback, complete 1600-ms bounds sampling, and
unchanged moving-collision rejection. Original-asset checks admit the lamp
while keeping the crate and temple static.

The GPU regression first validates all 36 original placed instances, then
renders one isolated lamp with fixed camera, disabled sky, and one texture
frame per material. Pixel changes at 0/100/400/800/1400/1600 ms are
`0/1948/2391/2557/1948/0`; the loop closes exactly. The support pole's vertices
stay fixed and sampled vertices remain inside the existing GPU scene bounds.
Inspection of 0/800-ms captures shows the lamp tilting while its pole stays
fixed. This checks OpenEQ geometry upload and rendering, not native framebuffer
parity or a new frustum-culling system.

```sh
CARGO_INCREMENTAL=0 EQ_DIR="$HOME/EverQuest" cargo test -p openeq-assets --lib loader::wld_objects -- --include-ignored
CARGO_INCREMENTAL=0 EQ_DIR="$HOME/EverQuest" cargo test -p openeq-render --test wld_lamp_objects -- --include-ignored --nocapture
```

## Frozen local research artifacts

Research files remain outside the repository; original archives and binaries
are not committed. These sources and outputs are read-only. Replay against
separate output paths to preserve them. The native runtime is
`/tmp/openeq-native-animation-venv/bin/python` with Unicorn and `pefile`.

| `/tmp/` artifact | SHA256 |
| --- | --- |
| `openeq-native-long-object-upstream.py` | `333ba5ad2777913d9bb2e7a0115b5455534cd195b862162c519f5dfaceda3c42` |
| `openeq-native-long-object-upstream.json` | `6cd7ac14f89de2b8598c6b5c94b07960315dc89241192edb7bc61820032fd2bf` |
| `openeq-native-long-object-loader.py` | `ebf1eca6c8830676766c1a892745b2f4adf0dba579d3fd975f37b979ed24bce2` |
| `openeq-native-long-object-loader.json` | `8eebb12ec6736ba1a1d89ecd673e0c372b451386e62220970c032d9046785529` |
| `openeq-native-long-object-animation.py` | `5bb089c285788d9a66f5cf4dee1e8394bc3a5c135b91e4a00f5dd84a5f819462` |
| `openeq-native-long-object-animation-scalar-0x37f.json` | `c8f7c519870c7beab84f61c013f713a0858af77894f2cdd3ce2a485b0ded2f82` |
| `openeq-native-long-object-animation-scalar-0x7f.json` | `1132021c5024af2298da9f1919d56d63bf20bc5b2780b7fbdf531fbc94aa4608` |
| `openeq-native-long-object-animation-sse2-0x37f.json` | `dd4c75051ffd108e0a3a721fbeaad6ce244d7d097d75f594bbfcdb04a8c188e3` |
| `openeq-native-long-object-animation-sse2-0x7f.json` | `6907b3f05a60829aa6dc771ff6f3e4cfdf7616fc6e764932d28197d8c6ba2611` |
| `openeq-lamp-zero-model.py` | `a01d8e086960f823b083dfb17c64fc9f2ea52a6d39848872dee2b27889d4ff4a` |
| `openeq-native-lamp-zero-controls.py` | `dd8fded6e8c481e9749c31d98b14a3a09a7f35cffe8d3218dec3ac783f9cf3c3` |
| `openeq-native-lamp-zero-controls-scalar-0x37f.json` | `0e6d9cc729671d689591ed78323fdf5b5cc7dc24263e4e6bf1f50415183bab76` |
| `openeq-native-lamp-zero-controls-scalar-0x7f.json` | `20f6cd9192ab2d8e08cc187df93034afdc065b1cd289b04f95198bbf1b008272` |
| `openeq-native-lamp-zero-controls-sse2-0x37f.json` | `0d624355cfc8c176df98b8ca4a7b37238be778b1edabda0c8a3880d540648fa0` |
| `openeq-native-lamp-zero-controls-sse2-0x7f.json` | `2ee9cc764227c95bc04cc5e42a1dd7dc36884fdf3f30c40f701a786f7a578955` |
| `openeq-lamp-zero-heap-audit.py` | `fed9266be21530cf75584efd0b28fdb524848f18ca18b8c608feacfdf18058c1` |
| `openeq-lamp-zero-heap-audit.json` | `86804311a1fbccb697ce6ad1cc4f12949820435b0e3b4c797a22b0bbce3c7913` |
| `openeq-native-lamp-rsqrt-control.py` | `dc5949bf4db6d3b2ee52abe38ceb34a2b23143ef7a94f04aea0617b6450aeab0` |
| `openeq-native-lamp-rsqrt-control.json` | `edf46b2abeaf7a160c4cd45cc8ef01786b625b6f6be351c8663b6d4995519733` |
| `openeq-lamp-review-wide-controls.py` | `640e0295209154eec491064fe79e10f37ead764088cd05a8ab77b95cc94d618a` |
| `openeq-lamp-review-wide-controls-scalar-0x37f.json` | `fd1957a32a7908309dd083f75d425640c7e70a00a5828a445fa1872890c2d39b` |
| `openeq-lamp-review-wide-samples.py` | `1c84fa306a1c1f283a0486c802d06dbf40625250a044d4956d60c17f787b780d` |
| `openeq-lamp-review-wide-samples-scalar-0x37f.json` | `2eedcdceb16926805c872d7ff6562a2e2b556a19806cca402afeda5ba1028c37` |
| `openeq-lamp-review-rust.rs` | `308eacd92853ca151f0071a6201569a63e56dfc2f8d893e4ee81e253a27dc4d4` |
| `openeq-lamp-review-rust.log` | `f21538c2a2238e43a36acc6b947e7acce3bca4021700afb52e377b09572edd76` |
| `openeq-lamp-review-replay.py` | `56c985cb99aa5392e772cdd5e47fc60106f5f3d13fde6d50921c76b8b1c22beb` |
