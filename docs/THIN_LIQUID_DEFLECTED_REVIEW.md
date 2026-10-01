# Review of ascending-support liquid transitions

2026-10-01. Independent review of the draft ascending-support collision
certificate and its proposed movement integration. The certificate accurately
predicted the tested collision prefixes. The first integration draft still
used corrected endpoint chords to discover liquid spans and elapsed times;
four counterexamples below show why additional restrictions are necessary.

This is evidence for a bounded OpenEQ change, not recovered original-client
physics. It extends [the initial deflected-movement investigation](THIN_LIQUID_DEFLECTED_MOVEMENT.md).
The rejected draft is not an approved general planar trajectory model.

## Collision certificate checks

Source inspection found no incorrect admission in
`CollisionWorld::certify_ascending_support`. The proof checks the entire
component-bounded XY rectangle, strict footprint containment, both collision
worlds and their complete support-query envelope. Per-axis nonnegative rise
makes the actual source-plane expression monotone, including rounding.
The stored source evaluator is necessary; endpoint interpolation is not the
same function.

A separate probe admitted **6,912 isolated ramp requests** and compared
**691,200 independently varied component prefixes** with fresh full collision
solves. Every position matched bit-for-bit. It covers both triangle windings,
positive and negative slopes, coordinates up to one million, and triangle
half-widths from 10 to one million. This finite stress test supports the code
review; it is not a substitute for the geometric proof or a test of competing
geometry. The portable collision tests cover competing supports and obstacles.

## Endpoint-chord coverage counterexamples

All three cases use radius 1, height 6, grounded non-jumping motion, one
`f32(1/120)` tick, and requested vertical velocity `-128 * tick_time`.
Liquid sampling uses feet plus three units on Z. Boxes have identity rotation.
For each case, the collision certificate is admitted, the source-plane prefix
equals a real collision prefix, and that prefix is inside the liquid. Yet
`LiquidRegions::segment` between the two complete-tick centers returns no spans.

### Ordinary ramp, source-plane height rounding

Triangle: `[-100,-100,-50]`, `[100,-100,50]`, `[0,100,0]`.
Start: `[0,0,0]`; horizontal velocity: `[40,0]`.

- Prefix time `8.333333e-6` seconds yields feet
  `[0.00033333333,0,0.00016784668]`.
- Box center `[0.00033333333,0,3.0001678]`, half-extents
  `[1e-7,10,1e-7]`, contains that prefix center.
- Complete dry result is `[0.33333334,0,0.16666794]`.

The slight height deviation from the chord is enough to miss this thin volume.
The draft returned the complete dry result without discovering a transition.

### Large triangle, a longer reachable height interval

Triangle: `[-1000000,-100,-500000]`, `[1000000,-100,500000]`,
`[0,100,0]`. Start and horizontal velocity are unchanged.

- Prefix time `3.3333334e-5` seconds yields feet `[0.0013333333,0,0]`.
- Box center `[0.0013333333,0,3]`, half-extents `[0.001,10,0.0001]`.
- Complete dry result is `[0.33333334,0,0.15625]`.

Cancellation in the source plane produces height plateaus. This box contains
roughly 50 microseconds of the requested walking trajectory, so the missed
interval is not merely a nonexistent representable point. A height-invariance
gate must reject it before endpoint spans can authorize a split.

### Diagonal XY rounding survives a height-only restriction

Triangle: `[-100,999900,-50]`, `[100,999900,50]`, `[0,1000100,0]`.
Start: `[0,1000000,0]`; horizontal velocity: `[40,40]`.

- Prefix time `5.833334e-5` seconds yields
  `[0.0023333335,1000000,0.0011672974]`.
- Box center `[0.0023333335,1000000,3.0011673]`, half-extents
  `[0.001,0.001,100]`.
- Complete dry result is `[0.33333334,1000000.3125,0.16666794]`.

This box is height invariant across the whole movement range. Independently
rounded X and Y still leave the endpoint chord. The new ascending integration
must initially allow only one nonzero requested horizontal component; the
collision diagnostic can retain its broader component-rectangle contract.
The existing straight-path behavior is separate preexisting scope.

## Spatial fractions are not elapsed-time lower bounds

Even single-axis movement with a height-invariant box needs a separate time
inversion. Translate the ordinary ramp by one million on X. Start at
`[1000000,0,0]` and request horizontal velocity `[40,0]`.

The complete result is `[1000000.3125,0,0.15624619]`. Use a box centered at
`[1000000.0625,0,3]`, half-extents `[0.01,10,100]`. Endpoint segment fractions
are `[0.168,0.232]`, but they parameterize the rounded endpoint distance.
The first reachable wet prefix has elapsed fraction **0.09375**.

The original `boundary_fraction` helper initializes its lower bracket at the
chord fraction. It consequently selects **0.16800001** here:

| Query | Elapsed seconds | Accepted feet |
| --- | ---: | --- |
| First reachable wet prefix | `0.00078125007` | `[1000000.0625,0,0.03125]` |
| Original chord-lower-bound helper | `0.0014000002` | `[1000000.0625,0,0.03125]` |

The endpoint and medium checks alone cannot distinguish these different
elapsed times: both prefixes produce identical feet. For the new ascending
subset, first invert a target coordinate inside the outgoing spatial span
through the exact monotone source-prefix operation. Verify the reached
coordinate remains strictly inside that span and in the intended medium.
Then search the first transition over elapsed fractions from zero to that
verified high bracket. If coordinate rounding skips the entire span, retain
the original whole-tick result rather than inventing occupancy.

## Height-invariance review

Independent inspection of `LiquidRegions::height_invariant_in_bounds` found
no incorrect admission. The contract requires the supplied inclusive AABB to
enclose every actual body-center prefix. The ascending certificate's monotone
position components and monotone center-height addition permit endpoint bounds
for this purpose.

For identity boxes, exact face inclusion is valid if the whole Z range stays
inside; a partly outside range touching the face must reject. The implementation
retains this distinction. For BSP metadata, outward f64 interval operations
follow the actual dot-product order. Nonzero-Z planes require strict one-sided
bounds, preserving the dry split-plane boundary. Zero-Z splits can inspect both
children, and fully dry subtrees can be pruned. Unsupported native and rotated
volumes decline proof. This certifies height independence, not XY trajectory or
elapsed-time mapping.

## Frozen local evidence

The temporary probes are separate from production files. Three coverage probes
import workspace movement source at compile time; their frozen executables
preserve the reviewed draft even after that source changes. Rebuilding them
against later source tests that later version. The timing probe includes a
verbatim frozen copy of the relevant original boundary helpers. No original
EverQuest files or native binaries are involved.

`/tmp/openeq-ascending-review-provenance.json` records source, executable and log
hashes for all five probes. Its SHA-256 is
`9116e944c43af63be8621193e4bf54e428770002bd5ec5f3ec79aefdf5f5fed1`.
The following names are relative to `/tmp`:

| Artifact | SHA-256 |
| --- | --- |
| `openeq-ascending-review.rs` | `9595767fd489569ef6ab166322fbe05ba90c1905578bfd423247b524ed1ea8b2` |
| `openeq-ascending-review.log` | `5fb8654fecbc2dcbe579a0718e164de04c97ebebc20c70dfcd00c316d65480d6` |
| `openeq-ascending-review-large.rs` | `b689d39a78885e5799ff276ce825e6a2c5c9704f52aa10bd3e358fc0effdef78` |
| `openeq-ascending-review-large.log` | `c1a003a2602e94f403bf4581021a23662205de483fa58ace9a5f6ce3105a9a72` |
| `openeq-ascending-review-xy.rs` | `31a5e7211506f694b7de4af95f250a02680b6d7bf5ea5b10e038ac6972ff796f` |
| `openeq-ascending-review-xy.log` | `6fda4bf997594efc164386773aaddc44b3499f95669c415bb1b90516fad6b828` |
| `openeq-ascending-time-review.rs` | `6b7e48c5961fce953ad7e208d33f8a4bd64bd24696baa57483abdc193632de3f` |
| `openeq-ascending-time-review.log` | `219ab40a57dcb0f9d51474d248615ce1ff827bc4e986e3dc68fee8dfb137f6cc` |
| `openeq-ascending-certificate-review.rs` | `575b1eb5fd2a5886a9ba1d25b94eca1a57ba27651dea8b4d5b6fc7343f94f7cd` |
| `openeq-ascending-certificate-review.log` | `2a73b25aab320932290495e1c26aeb0b270bc8b28cc0fbb9528ee3c4ca43181c` |

The artifacts above are frozen; later verification should use separate paths.

## Final point-membership monotonicity review

A subsequent independent review found two gaps in the binary-search precondition:

- A WLD split at X=0 enters water, then a split at
  `f32::from_bits(0x36555556)` has wet leaves on both sides. The segment API
  merges them, while the exact internal plane is dry. Rounded prefixes dwell
  there; search found fraction 9.536744073557202e-6 although the first wet
  prefix was 2.9989340077918314e-7. Height invariance alone correctly admits
  this structure, so it cannot serve as a single-interval proof.
- Box centers X=.09375/.10625 with half-extents .00625 and
  `f32::from_bits(0x3bcccccf)` leave X=.1 dry while adjacent representable Xs
  are wet. Over travel X=0..=.33333334, their reported fraction boundaries
  both round to .29999998211860657, hiding the gap from span merging.

The separate `has_single_liquid_interval_in_bounds` helper excludes more than
one potentially reachable wet BSP leaf or intersecting identity box, using
conservative bounds. Combined with height invariance and one moving horizontal
axis, this establishes the scalar membership contract needed by first_fraction.
Both failures are permanent regressions, and the movement tests require exact
saved-move fallback. A second independent inspection found no issue in the
stronger gate. This does not change the public segment API or claim that its
rounded fractions retain all point-sized gaps for other callers.
