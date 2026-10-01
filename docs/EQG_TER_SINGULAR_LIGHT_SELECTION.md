# Native terrain light ranking at singular inputs

The original terrain selector does not discard nonfinite scores. With finite
light inputs, exact coincidence with the receiver center can produce positive
infinity or NaN. The partial descending sort uses ordinary floating comparisons;
unordered comparisons do not swap. A NaN-scored light can consequently remain
in one of the three selected slots. A future implementation must not replace
this with a total float ordering, a finite-only filter, or distance clamping
while claiming native parity.

This is a bounded extension of [the original light-list witness](EQG_TER_LIGHT_LISTS.md).
It changes no production renderer. DPVS membership, event ordering and actual
region partitioning remain unresolved, so it does not justify substituting a
zone-wide light list.

## Executed boundary

The probe uses the same original DLL, constructors, receiver, score producer
and command handlers as the earlier witness. It checks the parent probe's
SHA-256 before reusing only its setup/helper prefix; the old probe and output
are not changed. Controlled DPVS interfaces deliver an explicit candidate
sequence. All six light definitions contain finite position, color and radius
values. No scratch score is manually injected.

The original point-light constructors and weight producer execute, followed
by command 0x40 to enter each light and command 0x30 to select the region's
lights. The region center is the same finite value used by the parent probe.
Execution uses masked x87 control word 0x037f and MXCSR 0x1f80. No native GPU,
real DPVS traversal, live player or actual in-game occurrence is claimed.

The source/result cases are:

| Controlled input | Stored weight | Squared distance | Observed score |
| --- | ---: | ---: | --- |
| Radius 4, RGB (.25,.5,.75), distance 2 | 8 | 4 | 4 |
| Same light exactly at receiver center | 8 | 0 | +infinity |
| Radius 0, RGB (.25,.5,.75), distance 2 | 0 | 4 | +0 |
| Radius 0 at receiver center | 0 | 0 | NaN |
| Radius 4, black RGB, at receiver center | 0 | 0 | NaN |
| Radius 4, RGB (-.25,-.5,-.75), distance 2 | -8 | 4 | -4 |

The negative-color row is a controlled input, not an observed original zone
light. The exact NaN word 0xffc00000 is an emulator observation, not a promise
of payload/sign equivalence on every physical x87 implementation. The relevant
selection property is unordered comparison behavior.

## Original arithmetic and branches

In selector 0x1000fcb0, the point-to-region differences and squared sum execute
at 0x1000fe81..0x1000feb0. The squared distance is stored as float32 at
0x1000feb0. The selector doubles the stored influence weight at 0x1000fee3,
divides by that squared distance at 0x1000fee6, multiplies the selected priority
at 0x1000feea and stores the score as float32 at 0x1000feec. These cases use
ordinary priority 1; the earlier witness covers priority 10/100 separately.
There is no zero-distance clamp or finite-score rejection in this slice.

Both the unrolled comparison loop beginning 0x1000ff58 and scalar tail at
0x10010006 execute `FCOMP`, `FNSTSW AX`, `TEST AH,5`, and a parity branch
that skips the swap. Mask 5 checks C0/C2; an ordered less-than result has odd
parity and swaps, while unordered sets both bits and skips. This differs from
sorting float bit patterns or assigning NaN a preferred position.

The witness checks all six permutations of a finite, infinite and NaN-scored
candidate. Three additional six-candidate cases exercise the unrolled loop
with zero, negative and two distinct NaN candidates. An independent ordinary
float-comparison oracle checks every scratch-list identity and the first
three selected slots. Together with six one-light arithmetic cases, all
**15 executed cases pass**.

## Reproduction

Run `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-ter-light-singular.py`. Its output retains finite source inputs,
receiver center, selected identities and raw score words, avoiding JSON NaN
numbers. It does not include original asset payloads.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-light-singular.py` | `0d33a2bac548241ee087196f12a207def91db683188db66e202d3f6a5740ff21` |
| `/tmp/openeq-ter-light-singular.json` | `a862f141f530d36bd6ce58da9a66252cc8db48cc5e5fecbd08e9c607c7ddcb53` |
| Required parent probe | `2abda543b71f540ac790d108f0e0f9d2f2eb840ee55c0e1840f8a4e9215a4006` |

The previous native-input precision, visibility and caller-lifecycle limits
remain in force. This does not cover arbitrary nonfinite source attributes,
all FPU control modes or full native lighting pixels.

Independent review replays all 15 cases to a separate output with the identical
JSON hash. Additional instrumentation observes 15 selector entries, 42 divides,
six visits to each unrolled comparison site and 36 scalar comparisons. No
probe or documentation blockers were found.
