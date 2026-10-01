# Crate mixed-motion animation: remaining boundary

October 1, 2026. This follow-up narrows the remaining `VSCRATE103` obstacle
without changing production admission. The crate still stays at its first
pose. Original native execution now separates its translation optimizer,
precision-sensitive retained count, two channel rankings and endpoint sampling.

## Original source and execution scope

`overthere_obj.s3d/overthere_obj.wld`, actor `VSCRATE103`, contains a static
root and a twenty-frame `VSCRATE3_DAG` track. It changes rotation and translation
while scale stays one. Its interval is 200 ms and period 4000 ms. There are two
original placements and twelve noncollidable triangles. The WLD SHA256 is
`4ed3874f2d88933ac9aa5b404715df9378d57c2927cbca49e61f834402cc3775`.
The earlier [lamp investigation](WLD_LAMP_ANIMATION.md) established original
decode and the actual object-loader timing flags.

The new witness supplies those verified decoded records to original EQ builder
`0x1003c150`. It records both sides of EQ optimizer `0x1003b650`, then executes
D3DX registration, rotation ranker `0x0043bed1`, translation ranker
`0x0043bcd0`, compression and compressed-set construction. Native `GetSRT` and
normalization are called separately at 84 times per mode. Scalar and SSE2 are
each exercised with x87 PC64 (`0x037f`) and PC24 (`0x007f`).

The frozen helper asserts original graphics/D3DX binary identities. Allocation,
free and name sorting are controlled; the established frame-zero skeleton W
adjustment is supplied explicitly. This does not execute original skeleton
assembly, live controller looping, collision runtime or GPU rendering.

## Independent channel counts and keys

The builder appends a closing key at 4000 ms. EQ's optimizer keeps all 21
rotation keys, reduces constant scale to one key, and removes the repeated
translation closure. Translation therefore enters D3DX with **20 keys ending
at 3800 ms**, while the complete animation period remains **4000 ms**.

| Reference mode | Rotation | Rotation omissions | Translation | Translation omissions |
| --- | --- | --- | --- | --- |
| Scalar or SSE2, PC64 | 21 → 18 | 2, 11, 7 | 20 → 17 | 5, 17, 12 |
| Scalar or SSE2, PC24 | 21 → 18 | 2, 11, 7 | 20 → 18 | 5, 17 |

Indices refer to the post-optimizer arrays. The retained-count difference is
real: the stored factor is `f32(1 - 0.1) = 0.8999999761581421`. Its PC64 product
with 20 is `17.999999523162842`, which truncates to 17. PC24 rounds the product
to 18 before truncation. At 2400 ms, native PC64 compressed translation Z is
`-0.177734375`; PC24 retains the authored `-0.1796875`. This is a motion
difference, not merely different metadata.

Native direct samples at 3800, 3801, 3900, 3999 and 4000 ms all hold the last
translation `[0.0234375, 0.04296875, 0.0625]`. The final 200 ms is a plateau.
Direct calls beyond 4000 ms also hold the endpoint; the witness does not execute
the separate controller operation that supplies periodic animation time.

## Bounded scalar score certification

A research-only extension of the
[Temple enclosure model](WLD_TEMPLE_ANIMATION_RESEARCH.md) certifies all
**25 rotation and 24 translation score calls** needed for the original first
three removals, including their neighbor updates. It independently reconstructs
the complete first-three heap states and confirms that both channels preserve
the saved-right-slot restriction. No native score values or omission indices
are supplied to that independent selection model.

Rotation's first two candidates, indices 2 and 11, have clamped zero error;
index 7 has `2.3414560928358696e-5`. Temple's positive-score implementation
deliberately rejects zero-store uncertainty. The research model adds a symbolic
case: if the entire dot interval clamps to the singleton +1 or -1, native
`1 - dot²` is exactly positive zero, as is multiplication by a finite time
span. This is an exact algebraic case, not a tuned tolerance. It has not been
added to the production helper.

Translation uses native `f32` midpoint stores, squared error in Z/Y/X order,
and the wider raw error through neighbor-span weighting. Its first three
selected initial errors are all exactly `3.814697265625e-6`. The strict heap,
not an earliest-index tie shortcut, chooses 5, 17 and 12. Both channel models
retain their own keys and original timestamps.

The same outward bounds certify the PC64 count: the twenty-key product lies
within `[17.99999952316284, 17.999999523162845]`, wholly below 18. An explicit
PC64 compatibility policy can therefore select 17 without an epsilon. This
does not establish which precision the complete original running client uses.

## Optimizer run and endpoint controls

Eight controlled variants execute the complete original builder in both
scalar precision modes. Only decoded translation values are changed; source
rotation stays intact. The two near-threshold variants intentionally leave the
packed grid and are diagnostics, not proposed admission candidates.

| Controlled translation change | Optimizer keys | Removed source times (ms) | PC64 / PC24 retained |
| --- | ---: | --- | --- |
| Original | 20 | 4000 | 17 / 18 |
| Last authored Z increased by one packed step | 21 | None | 18 / 18 |
| Last three values, including closure, equal | 19 | 3800, 4000 | 17 / 17 |
| All values constant | 1 | All after zero | 1 / 1 |
| Two equal interior values at 1800/2000 | 20 | 4000 | 17 / 18 |
| Three equal interior values at 1800/2000/2200 | 19 | 2000, 4000 | 17 / 17 |
| Last authored Z offset by about `0.00009` | 20 | 4000 | 17 / 18 |
| Last authored Z offset by about `0.00011` | 21 | None | 18 / 18 |

Interior runs preserve their first and last time; terminal runs retain their
first time and discard redundant later endpoints. The near-equality constants
are the native stored `f32` values around ±0.0001. Distinct packed positions
are separated by at least 1/256, so exact repeated positions offer a narrower
future policy than accepting arbitrary near-equal floating-point runs.

## Strongest next witness

The count boundary is now understood under an explicit scalar-PC64 reference,
but this evidence does not justify a hurried runtime extension. The next useful
step is a generated twenty-frame mixed-motion corpus that independently models
packed translation runs, then executes the whole original builder and both
rankers. It should vary run location/length, closure retention, channel key
counts, clamp-zero rotations, exact score ties and saved-neighbor movement,
then compare native samples across omitted keys and the terminal plateau.

A later implementation needs separate optimized translation timestamps and
rotation keys, a reviewed symbolic zero gate, channel-specific retained counts,
the stable-neighbor restriction, and existing bounds/collision guarantees.
Repeating original omission indices or reusing the rotation omission for
translation would be wrong. No production files, GPU runs or Cargo checks
were part of this research follow-up.

## Frozen local reproduction

All listed probes and results are read-only and remain outside the repository.
Original assets and binaries are not committed. Supply a new output path:

```sh
/tmp/openeq-native-animation-venv/bin/python /tmp/openeq-native-crate-lifecycle.py scalar 0x37f /tmp/crate-native-review.json
python3 /tmp/openeq-crate-score-enclosure.py /tmp/crate-enclosure-review.json
/tmp/openeq-native-animation-venv/bin/python /tmp/openeq-native-crate-optimizer-controls.py scalar 0x37f /tmp/crate-optimizer-review.json
```

All three separate scalar-PC64 replays byte-match the frozen results.

| Local artifact | SHA256 |
| --- | --- |
| `/tmp/openeq-native-crate-lifecycle.py` | `650fb7090e0da521ec2519f5cec9fee61465ae24b69917a59a805e292b0fa607` |
| `/tmp/openeq-native-crate-lifecycle-scalar-0x37f.json` | `4aee32fb551bfbddd3864607d1fea63bd908fa331fea3874f8afa162e9fbb843` |
| `/tmp/openeq-native-crate-lifecycle-scalar-0x7f.json` | `db42b3727d8a02a9e3d21a4ed48afc6fa2f8ef9c399d0a926e0e8e35b47715e9` |
| `/tmp/openeq-native-crate-lifecycle-sse2-0x37f.json` | `0297e96a48bd772706557673f93f3c4d267f7a2dcb1db6b4340d0e83ca64a447` |
| `/tmp/openeq-native-crate-lifecycle-sse2-0x7f.json` | `0cff90105408248972f5171a95114c70f56e986764c9d25013191b228d20656b` |
| `/tmp/openeq-crate-score-enclosure.py` | `bdd36c977765504cfb73eae28cbc3f7ab7d7d43c13703f525a5556141c155009` |
| `/tmp/openeq-crate-score-enclosure.json` | `b0670ec45fe84490dcfce7dd71db363ebf53f81715473ea142dbdd65aaffac52` |
| `/tmp/openeq-native-crate-optimizer-controls.py` | `4f21a374fa403304602eca2e52814a8c74975387e9dfc6d3928c0db8f3380469` |
| `/tmp/openeq-native-crate-optimizer-controls-scalar-0x37f.json` | `6c7367b97e1cd068be52d5639bdf21b1b26445904fac67a12996ef83c06d4fe3` |
| `/tmp/openeq-native-crate-optimizer-controls-scalar-0x7f.json` | `1de39b651a12023bafc700d7778b02dfdc83c4f66c820b618cef677c5f931c04` |

Root independently reran all three scalar-PC64 commands into separate outputs;
all result hashes above matched exactly. Source review confirms the native
channel counts and stored timestamps are retained separately in the witness.
