# TEMPLELIFE rotation: certified scalar reduction

October 1, 2026. `TEMPLELIFE` now rotates on its original eight-second loop in
North Qeynos. Its native reduction needs ordering among positive errors, so
the lamp's zero/positive-category argument does not extend to this clip.
A separate bounded forty-frame helper instead certifies the stored scores of
an explicit scalar-PC64 numerical reference before making heap comparisons.

## Source and execution

The original `qeynos2_obj.wld` member SHA256 is
`c536ef29ff1231d63d05134fe41b4ed2847d3e4b901f85bdd409aa9d417163c2`.
The actor has one static root and a 40-frame rotating track, `TEMPLIFE_DAG`.
Translation and scale remain fixed, but all three XYZ quaternion components
vary. Its 200-ms authored interval gives an 8000-ms loop. There is one original
placement and no collidable triangles. Source decode and actual object-load
timing were established by the earlier
[lamp investigation](WLD_LAMP_ANIMATION.md).

The follow-up reuses those verified decoded records and executes original EQ
builder `0x1003c150`, its optimizer, native D3DX registration, rotation ranking
`0x0043bed1`, compression and compressed-set construction. It uses the scalar
path with x87 control word `0x037f`. Allocation/free and name sorting are
controlled, and the established skeleton frame-zero W adjustment is supplied
explicitly. It does not execute the original archive loader, skeleton assembly,
live controller or rendering. The installed graphics DLL and Microsoft D3DX
binary hashes are asserted by the frozen helper and recorded in the result.

The trace records candidate helper `0x0043b27a` inputs, stored normalized
midpoints, initial `f32` errors, neighboring raw and weighted errors, linked
neighbors, and every heap state before extraction. It covers all **39 initial
candidates, 107 score calls and 39 heap extractions**. An independent strict
heap/link model reproduces every complete heap snapshot using the observed
native numeric scores. That replay checks heap and link semantics; it is not
an independent implementation of the floating-point error calculation.

## Positive scores determine the omissions

All 39 initial errors are positive, ranging from `3.8880731153767556e-5` to
`1.6883820353541523e-4`. The appended closing key gives **41 keys, of which 36
are retained**: five omissions, not six.

| Extraction | Source index | Native stored score | Compressed result |
| --- | ---: | ---: | --- |
| 1 | 16 | `3.8880731153767556e-5` | Omitted |
| 2 | 22 | `3.8880731153767556e-5` | Omitted |
| 3 | 2 | `4.838977838517167e-5` | Omitted |
| 4 | 7 | `6.01904139330145e-5` | Omitted |
| 5 | 31 | `6.01904139330145e-5` | Omitted |
| 6 | 24 | `6.141891208244488e-5` | Retained |

Indices 16/22 and 7/31 are exact ties in stored native scores. Native strict
heap comparisons determine their extraction order. The fifth/sixth score gap
is about `1.22849814943e-6`; a zero/positive category does not preserve this
ordering. Collapsing every positive score to the same category would select
source index 1 first, already disagreeing with the native index 16.

After each of the first five omissions, both surviving neighbors are
recalculated across a 600-ms span. Native weighting multiplies the wider raw
error by that span before its final `f32` store; the intermediate raw `f32`
record is diagnostic and is not a replacement for that wider value.

| Removed index | Recomputed left / right indices | New weighted left / right scores |
| ---: | --- | --- |
| 16 | 15 / 17 | `0.06964275240898132` / `0.04504149779677391` |
| 22 | 21 / 23 | `0.04504149779677391` / `0.06964275240898132` |
| 2 | 1 / 3 | `0.04504149779677391` / `0.06985297054052353` |
| 7 | 6 / 8 | `0.058306191116571426` / `0.06127072125673294` |
| 31 | 30 / 32 | `0.06127072125673294` / `0.058306191116571426` |

These neighbors remain positive, but preserving that category alone says
nothing about their relative position among other positive candidates.

## Certified positive-score admission

The new helper accepts exactly forty packed near-unit quaternion frames with
constant translation and scale, positive exact timestamps, and the existing
reference flags, shared timeline and hemisphere restrictions. Moving physical
geometry remains rejected by the unchanged collision-ancestry gate.

Each arithmetic operation has an interval enclosed by the adjacent binary64
values outside the calculated endpoints. With IEEE round-to-nearest arithmetic,
these binary53-significand endpoints also bound the finer x87 PC64 result:
they are representable in the binary64-significand reference grid. This covers
addition, subtraction, multiplication, division and square root. Stored `f32`
endpoints follow monotonic rounding. Squared midpoint length must remain within
`[0.5, 1.5]`, and the entire interval must choose one native near-unit branch.

A score is admitted only when both final endpoints produce **identical `f32`
bits**, both are finite, and the score is nonnegative. Uncertain stores or
normalizer branches reject the clip. This uses no tuned numerical margin.
Equal certified score words are exact reference ties, including the two
sign-reflected pairs in this actor. The native wider raw error is retained
through time-span weighting before the final score store. The five-frame
helper's formula informs this implementation, but its separation threshold
and first-key tie rule are not reused for longer tracks.

Native retained-count arithmetic puts `41 * f32(1 - 0.1)` strictly between
36 and 37, so five keys are removed. A strict heap preserves native tie order.
Original timestamps, closing period, and retained-key hemisphere continuity
remain intact. Sampling uses the established wider interpolation convention.
This is a scalar-PC64 reference policy, not a claim of identical results on
all native CPU paths or support for arbitrary longer tracks.

## Native saved-neighbor restriction

Generated controls exposed an additional native constraint. The ranker obtains
both neighbor heap addresses after root removal. Updating the left neighbor
can move the right neighbor, but the native code retains its old address until
the right update. In the preserved synthetic case 13, removing source key 14
makes the left update move source key 13 into the saved slot for source key 15.
The original ranker then recalculates key 13 twice and leaves key 15's score
unchanged. A straightforward source-neighbor algorithm would disagree.

The new helper rejects any removal whose left update changes the identity in
the saved right slot. It does not reproduce that stale-address behavior or
assume it away. Root removal finishes before the neighbor addresses are taken;
the right update is the only remaining neighbor use after a mutating left
sift. The check applies to all five removals, including the last one's updates.
A compact generated witness is an ordinary rejection regression.

## Validation

The original clip plus 128 seeded multi-axis packed controls use intervals
1/17/100/333/1000 ms, quaternion sign changes and varied axes, angles and
magnitudes. Of 129 cases, 109 pass the proposed gate; 15 reject saved-slot
movement and five reject uncertain score stores. All **5,318 admitted native
score calls and 545 omissions** match, including complete first-five heap
snapshots. Rejected cases remain diagnostic evidence, not admitted controls.

An external Rust wrapper includes the actual production helper. It matches
all 109 admitted omission arrays and **1,198 original native `GetSRT` samples**
at discarded keys, neighboring midpoints and the loop endpoints. Maximum
quaternion-component error is `1.1920929e-7`. The wrapper reverses the native
decoder's W convention before constructing public WLD frames.

Independent review checked the interval argument against the original scalar
instruction order, fast-path and store guards, strict heap ties, saved-neighbor
restriction, retained count, sampling bounds and collision integration. Its
fresh compilation of the actual helper reproduced the 109-case / 1,198-sample
result, and its separate enclosure replay byte-matched the frozen output.

The focused asset suite passes 60 tests with original assets enabled. It covers
the native original omissions, a generated positive-score witness, loop closure,
sign equivalence, bounds, uncertain scores, stale slots, invalid layout/timing,
and unchanged moving-collision rejection. The isolated GPU regression validates
the original placement and all 372 noncollidable triangles, freezes texture
animation, disables sky and checks bounds. At 0/200/1600/4000/7600/8000 ms its
changed-pixel counts are `0/8550/8828/8429/8713/0`. Captures show the textured
model rotating and the loop closes exactly. This checks OpenEQ's geometry
upload and rendering, not native framebuffer parity. Scoped Clippy passes.

```sh
CARGO_INCREMENTAL=0 EQ_DIR="$HOME/EverQuest" cargo test -p openeq-assets --lib loader::wld_objects -- --include-ignored
CARGO_INCREMENTAL=0 EQ_DIR="$HOME/EverQuest" cargo test -p openeq-render --test wld_temple_objects -- --include-ignored --nocapture
```

## Local reproduction

Original assets and derived research outputs remain outside the repository.
The new script and result are frozen; use a separate output path for replay:

```sh
/tmp/openeq-native-animation-venv/bin/python /tmp/openeq-native-temple-rank-trace.py scalar 0x37f /tmp/temple-rank-review.json
```

| Local artifact | SHA256 |
| --- | --- |
| `/tmp/openeq-native-temple-rank-trace.py` | `44be37aa0a79e5e6a6ac38bfe9c7d41920fa69cc820c1514d40828727285155a` |
| `/tmp/openeq-native-temple-rank-trace.json` | `4089ac13c7f3d37a429ddd3f2f51836ff268dd4017acdf891727ddcaf9776336` |
| `/tmp/openeq-native-temple-rank-trace.log` | `12b1b1b96fe59482ee4f40650b6541cb9a8563aefe5b2659029da8d44ff11377` |
| `/tmp/openeq-temple-score-enclosure.py` | `a44abcacc4b43dbcab84a964446650957d7ff1c90eda960683abbebd5ca722a8` |
| `/tmp/openeq-temple-score-enclosure.json` | `8387c80b4588b6c5549110a69692093b04c07ca1d474afbce7c8bc77cceafaa6` |
| `/tmp/openeq-temple-enclosure-controls.py` | `96bac549e984cbbd17234286a7ac2db36f422799fd12da0fc244b4f3682b851d` |
| `/tmp/openeq-temple-enclosure-controls.json` | `58e4129bdf7456a944585c033851939ef508d289a6f336c389c668cfed316dcd` |
| `/tmp/openeq-temple-enclosure-controls.log` | `bf412164635754bcffbeeed0515fb764dbbae0ab1a51b8925d59d6d1b8e82cb5` |
| `/tmp/openeq-temple-control13-trace.json` | `817390fbfd96ef26af61eb88f46edae6de37e22e71aa4db07fbf0d61f03f5311` |
| `/tmp/openeq-temple-review-rust.rs` | `b4b6c2ef767cecb4ee8b6f2ef49153df99d264ca86683940b46a081eca4b80ad` |
| `/tmp/openeq-temple-review-rust.log` | `4b7f454ee056077ad4778216e1ba373ce264ef83c0697f1998c8cecd817af7a5` |

The pure enclosure replay takes its output path as argument one. The broader
native control replay needs **both** separate result paths, including the
preserved stale-slot diagnostic:

```sh
python3 /tmp/openeq-temple-score-enclosure.py /tmp/temple-enclosure-review.json
/tmp/openeq-native-animation-venv/bin/python /tmp/openeq-temple-enclosure-controls.py scalar 0x37f /tmp/temple-controls-review.json /tmp/temple-stale-slot-review.json
```

Separate native trace, control and stale-slot replays byte-match their frozen
results. The scripts verify the earlier helper/decoder identities; none of the
original archive payloads or binary files is committed.
