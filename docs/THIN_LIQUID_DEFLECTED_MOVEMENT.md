# Thin-liquid crossings during collision-deflected movement

2026-10-01, research followed by an optional collision certificate. This follows
[the straight-path implementation](THIN_LIQUID_MOVEMENT.md). The frozen prototype
evidence below predates the production certificate described in its own section;
no original-client physics is inferred. The first supported extension is
**certified ascending motion on one support plane**. General slides, descending
support, stairs and contact-correction attempts do not yet have suitable
travel/time semantics.

## Reproduced misses at ordinary walking speed

Both fixtures start at feet `[0,0,0]`, with radius 1, height 6, ordinary
gravity, no jump and one 1/120-second tick. The liquid sample is the body center,
three units above feet. Walking input is 40 units/s per requested horizontal
axis, and the existing swim multiplier is 0.6. These speeds are OpenEQ tuning,
not measurements of the original client.

The probe includes an instrumented copy of the current collision source. It
records requested substeps, support projections, attempted lateral corrections
and accepted positions without altering the solver's expressions or control
flow. It also runs the unmodified movement/collision implementation for parity.

### Ascending planar ramp

One walkable triangle has vertices `[-100,-100,-50]`, `[100,-100,50]`, and
`[0,100,0]`, giving support plane `z = x/2`. The requested walk velocity is
`[40,0]`. A water box uses min `[0.1,-10,0]`, max `[0.2,10,6]`, converted to
center and half-extents with the normal `LiquidBox` constructor arithmetic.

The current movement result is **`[0.33333334,0,0.16666794]`, Ground**, both
with and without liquid metadata. The solver labels the move `Deflected`.
Both center endpoints are dry. The center-line liquid query reports fractions
0.3–0.6, but the existing guard correctly refuses to interpret an arbitrary
deflected endpoint line as travel.

The accepted solver operation first requests
`[0.33333334,0,-0.00888889]`, projects Z to `0.16666794`, then accepts that
position without lateral corrections or step attempts. Independent collision
prefix calls establish the missed interval:

| Requested elapsed time | Accepted feet | Wet center |
| ---: | --- | --- |
| 2.0 ms | `[0.080000006,0,0.040000916]` | No |
| 3.0 ms | `[0.120000005,0,0.060001373]` | Yes |
| 4.0 ms | `[0.16000001,0,0.08000183]` | Yes |
| 8.333334 ms | `[0.33333334,0,0.16666794]` | No |

For this single-plane fixture, all **1,001 tested prefix results** agree
bit-for-bit with evaluation of the original supporting triangle's plane at
the requested XY. This uses the source triangle's `plane_z` calculation,
not interpolation between corrected endpoints. The finite sweep verifies the
fixture; it is not a general certificate for arbitrary collision geometry.

A fixture-specific event replay uses the known monotone X boundaries, then
bisects actual collision-prefix results to find representable outgoing points.
It reaches wet feet `[0.10000001,0,0.049999237]`, exits at
`[0.20000002,0,0.099998474]`, and finishes at
**`[0.26666668,0,0.13333511]`**. The ideal internal time allocation is 2.5 ms
walking, 4.166667 ms swimming, and 1.666667 ms walking. No position is assigned
from a corrected-endpoint line. The replay is not a generic boundary detector.

### Wall slide whose endpoint line misses water

A flat floor spans XY `[-50,50]` at Z=0. A vertical wall at X=1.1 spans
Y `[-20,20]`, Z `[0,20]`. Walking input is `[40,40]`. A water box uses
min `[0.07,0.13,0]`, max `[0.11,0.2,6]`.

With or without liquid metadata the current tick ends at
**`[0.09899999,0.33333316,0]`, Ground**. The direct line between starting and
ending centers has **no liquid spans**. Nevertheless, actual collision-prefix
responses enter the box:

| Requested elapsed time | Accepted feet | Wet center |
| ---: | --- | --- |
| 2.0 ms | `[0.080000006,0.080000006,0]` | No |
| 3.25 ms | `[0.09900002,0.13000003,0]` | Yes |
| 4.0 ms | `[0.09900001,0.16000015,0]` | Yes |
| 5.0 ms | `[0.09900002,0.1999999,0]` | Yes |
| 8.333334 ms | `[0.09899999,0.33333316,0]` | No |

The full solve requests `[0.33333334,0.33333334,-0.00888889]`, clamps to the
floor, then applies a lateral correction to approximately
`[0.099,0.33333316,0]`. That correction starts at an obstructed candidate;
it is not an accepted segment traveled through the wall.

A diagnostic replay using the known Y interval and collision-prefix bisection
ends at `[0.09899999,0.28666675,0]`. This illustrates the cost of the missed
medium change, but does **not** establish a general slide-time model. Prefix
queries describe responses to different requested durations; they are not a
recorded continuous route inside the original complete call.

## Why accepted endpoints and correction logs are insufficient

The collision solver has no velocity or elapsed-time argument. It receives a
displacement, splits its **3D length** into at most 256 equal requested
displacements, and accepts one position per `move_step`. At normal speeds
both witnesses above use only one substep.

`move_step` may project support, clamp a ceiling, try lateral depenetration,
try raised supports, or resnap support after a slide. `slide` iterates endpoint
overlap corrections up to eight times. Neither routine computes contact
times. Correction length therefore cannot be divided by velocity to recover
time. Failed or obstructed candidates must never become liquid travel segments.

Even the family of independently replayed prefixes can be discontinuous:

- At the wall, a 2.5-ms prefix ends at X=`0.099999994`; 2.5001 ms ends at
  X=`0.09900001`. The extra `SKIN*0.1` push introduces a backward jump when
  contact first overlaps. This is endpoint clearance behavior, not a timed
  backwards movement.
- Descending the same ramp with input `[-40,0]`, 0.49 ms gives
  Z=`-0.009799957`, 0.51 and 0.75 ms give Z=0, and 1.1 ms gives
  Z=`-0.02199936`. `movement_ground`'s footprint-retention and support
  tolerances cause this nonmonotone prefix response.

Consequently, exposing a list of accepted endpoints and querying each adjacent
line would not solve this problem safely. A complete discrete collision result
needs an independently justified trajectory certificate before movement can
assign travel times within it.

## Proposed smallest implementation boundary

Start with a new optional certificate for **ascending grounded travel on one
known support plane**, preserving the existing `Deflected` fallback elsewhere.
The original proposed contract is recorded here; the smaller single-substep
implementation boundary is detailed below:

1. Preserve the original complete collision solve and its exact result. Record
   its actual substep partition, requested motion and selected support primitive
   without changing collision arithmetic, ordering, support selection or pushes.
2. Certify a substep only when the starting feet lie on its support plane,
   requested Z is nonpositive, support height is nondecreasing along requested
   XY, and the whole body footprint stays strictly within one walkable support
   triangle. One triangle is a useful initial restriction; combining a patch
   of coplanar triangles can follow with separate coverage evidence.
3. Inspect the entire swept body bound and the **full step/drop support-query
   envelope below the feet** in both static and dynamic worlds. A lower floor
   outside the body may still win closest-height support selection. Reject
   competing support, walls, ceilings, support edges, discontinuous snaps, step
   attempts or lateral correction attempts. Checking only endpoint candidates
   is insufficient. A conservative rejection of additional nearby geometry is
   acceptable for this first boundary.
4. Retain the source plane evaluator, not a plane reconstructed from the two
   accepted endpoints. Require finite, well-conditioned evaluation and margin
   from support/clearance thresholds. Large-coordinate rounding or an inability
   to bound the complete interval makes the certificate unavailable.
5. For substep `i` of `N`, with original accepted start `p_i` and original
   requested motion `m`, define a prefix by requested parameter `u`:

   ```text
   xy(u) = p_i.xy + m.xy * u
   z(u)  = selected_source_triangle.plane_z(xy(u))
   spent_time(u) = remaining_tick_time * (i + u) / N
   ```

   `u` measures requested integration time, **not corrected spatial distance**.
   The source solver must expose a prefix operation using that same stored
   substep motion and floating-point order. Recalling the whole solver with a
   shorter displacement can change `N` and is not an equivalent API.
6. Use the certified parameterized support motion to locate a candidate liquid
   interval. Apply the existing representable-outgoing-point rule and run the
   candidate prefix through the actual collision operation. Accept only when
   its support certificate and outgoing medium agree. A numerical ambiguity
   must fall back, rather than justify a fixed world-space offset.
7. Recompute the remaining movement under the existing swim/walk policy. Keep
   gravity/jump once per tick and the 16-transition limit. Any unsupported
   subsequent segment discards the tentative split and returns the saved
   original whole-tick result, as the current straight-path implementation does.

The certificate is the required new work. Matching sampled prefixes or knowing
that the full accepted endpoint lies on a ramp cannot substitute for the swept
support/clearance proof. The implementation can be staged as metadata-only
collision reporting and tests before connecting it to medium transitions.
General wall slides, descending support, triangle-edge changes, stairs and
moving geometry during the tick remain outside this first extension.

## Implemented optional collision certificate

[`collision_ascending.rs`](../crates/openeq-assets/src/collision_ascending.rs)
adds `CollisionWorld::certify_ascending_support` without changing the existing
solver, `PlayerMove`, or `PlayerMovePath` classifications. Its `AscendingSupport`
result borrows the static and optional dynamic worlds, preserving an immutable
geometry snapshot. Ordinary collision calls do not run the additional proof.

The initial implementation accepts **one uncapped substep only**. It applies the
existing radius and step clamps and checks the same 3D displacement length. It
requires exact initial source-plane support, nonpositive requested Z, a strictly
higher final support height, and nondecreasing source-plane height for **each**
requested XY component. Opposing uphill/downhill components are rejected even
when their combined movement would ascend. This stronger restriction makes the
original sequence of floating-point plane operations monotone across the whole
component-bounded prefix rectangle.

Outward f32 interval calculations prove that the complete prefix XY rectangle,
expanded by body radius plus skin, stays strictly inside the source triangle's
actual `ground_at` barycentric domain. The square footprint bound is stronger
than circular coverage. Every static and dynamic triangle returned by the swept
spatial query is checked against both body clearance and the support interval:

```text
max_drop = max(max_step * 1.5, abs(delta.z) + SKIN)
support_low = next_down(start.z - max_drop - SKIN)
support_high = next_up(start.z + max_step + SKIN)
body_high = next_up(support_high + height + SKIN)
```

A vertically distant triangle is ignored only if an outward bound of its
**computed source plane** also excludes the support interval. Vertex Z bounds
alone are insufficient for ill-conditioned plane evaluation. Other triangles
conservatively reject the certificate, including harmless extra geometry in
the swept grid buckets. No contact ordering or collision arithmetic is changed.

`AscendingSupport::position_for_delta(prefix_delta)` receives the caller's exact
prefix request, such as `velocity * (remaining_time * fraction)`. It does not
reconstruct that request by multiplying the already rounded full displacement.
Each component must remain between zero and the original component. Evaluation
uses the source triangle's existing `plane_z` expression at the requested XY;
the existing `1e-7` no-motion cutoff remains intact. Single-substep membership
ensures that a shortened solver call cannot change the original partition.

`AscendingSupport::resolve_prefix(prefix_delta)` calls the unchanged solver and
requires bit-exact agreement with the predicted position. Certificate creation
also validates the complete request this way. A caller that already performed
the collision call can compare its result with `position_for_delta` instead.
The API certifies collision responses to specified prefix displacements; the
caller owns time accounting, liquid-boundary discovery and outgoing-medium
validation. It is not a continuous native collision trace.

The focused certificate tests include 1,001 prefix requests for each of static
and dynamic copies of the ramp, preserving actual multiplication order and the
existing `Deflected` result. They also cover opposite winding and motion signs,
all 3,872 sampled combinations in translated two-axis prefix rectangles,
prefix-domain rejection, the no-motion cutoff, clamped body parameters, lower
support competitors, ceilings, walls, narrow support, descents, airborne starts,
uncapped substep limits, and uncertain arithmetic. The broader original
collision tests remain unchanged.

## Dry behavior and next verification

The instrumented copy and the unmodified solver produced bit-identical feet
and vertical velocities, with equal modes, in **1,344 dry cases**. These cover
flat floor, ascending/descending ramp, wall, and step fixtures; signed walking
speeds up to 1,200 units/s; grounded/airborne starts; jumping; and one/four ticks.
This proves the temporary observation hooks did not change those results. It
does not claim that a future certificate implementation is already verified.

A production extension should retain exact dry and fully submerged results by
returning the saved original result whenever there is no accepted boundary.
Required follow-up checks include original dry-zone fixtures, static/dynamic
copies of the same ramp, a nearby competing floor, walls/ceilings, narrow
support edges, the existing raised-platform false-line case, the descending
discontinuity above, capped requests, and medium-direction reversal. The wall
slide witness should remain explicitly unsupported until timed contact
semantics are established. No collision retuning follows from this note.

## Frozen local evidence

Source checkpoint observed: `a5766b98b09492a1f3ba55b2463fecbe911be08b`.
The relevant source SHA-256 values are:

- `crates/openeq-assets/src/collision.rs`:
  `f7e8c5b121ac89b6897b7e8356955912a284e0983c881434da3cea4a7420f02d`
- `crates/openeq/src/movement.rs`:
  `03e6e5dbefb6ccd77d68258241f3c8bd677e7139d195fce8e02169e9b2c6aa28`

Run `python3 /tmp/openeq-liquid-deflection-review.py` after building the assets
crate. The script checks the source hashes and links the available local debug
libraries. The collision copy adds observation hooks and a fixture-only
source-plane evaluator; the movement copy only redirects its collision import.
These temporary paths are local research artifacts, not permanent fixtures.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-liquid-deflection-review.rs` | `a08ace9b82356c7900566cc6cd1f75d694a918f074203d9b504699bca56b29a0` |
| `/tmp/openeq-liquid-deflection-review.py` | `212ef33eab4acd6169a3f2e94f11aa1e18fb967c1219044be3549861462c4e76` |
| `/tmp/openeq-liquid-deflection-review.log` | `31a68af9129ab98a46f4e2a3913b4bb8aeb0c300497b2e1bcf64160c838f2ce5` |
| `/tmp/openeq-liquid-deflection-collision.rs` | `05972d700585a8ae93dbf293910cb7fb4bbde8fcd4b07630898909eeb68647f9` |
| `/tmp/openeq-liquid-deflection-movement.rs` | `ded60a48010aefe50192481e63ab5fe985c879001b451026fe65b7482c44e383` |


## Integrated medium response and numerical restrictions

The production movement caller now uses this certificate only for a grounded,
nonpositive-Z request with exactly one moving horizontal axis. It additionally
requires `LiquidRegions::height_invariant_in_bounds` over the entire swept
body-center AABB. The certificate's monotone source-plane heights make endpoint
min/max sufficient to enclose every actual rounded center height. This is a
proof of independence from Z, not a sample of the endpoints' liquid kinds.

The height proof supports WLD BSP and identity-rotation boxes. BSP dry subtrees
are pruned; uncertain XY-only splits inspect both children, while a split with
nonzero Z requires an outward-rounded f64 distance interval strictly on one
side. Touching or crossing such a plane rejects proof. Each potentially active
box must contain the whole queried Z interval, preserving inclusive faces and
source precedence. Native binary/terrain volumes and rotated boxes deliberately
remain outside this first proof. No authored region or collision is changed.

These restrictions address the independently reproduced rounded-height and
rounded-diagonal counterexamples in `THIN_LIQUID_DEFLECTED_REVIEW.md`. A chord
can miss water reached by the actual source-plane staircase; height independence
and one-axis XY travel remove those new coverage ambiguities. This is a narrow
extension, not completed arbitrary ramp or slide support.

Even one-axis endpoint distance can differ from requested distance at large
coordinates. The caller therefore finds the next geometric span's midpoint in
directed scalar progress, then inverts **actual certified prefix positions**
to find a representable time inside that span. It rejects a rounded jump that
skips the span. From zero to that verified interior time it locates the first
outgoing-medium fraction, searching nonnegative f32 bit order to include tiny
fractions that 24 arithmetic halvings would miss. Actual prefix collision and
medium queries must agree before applying a mode change. Existing adjacent-span
and zero-length-boundary semantics remain unchanged.

At X=1,000,000, the regression's first wet fraction is exactly `0.09375`; the
old corrected-endpoint fraction would start at approximately `0.168`. The test
requires the immediately preceding representable fraction to remain dry. The
ordinary ramp's X[0.1,0.2] water interval now produces final X approximately
0.26666668 instead of 0.33333334, including the existing 0.6 swimming multiplier.
Both static and dynamic copies pass, and the final feet remain on the source
plane. This does not retune speeds or recover original-client movement physics.

The saved original complete-tick solve remains the fallback for unsupported
certificates, missed/discontinuous brackets, later unsupported segments or failed
medium revalidation. Dry and fully submerged requests preserve their exact
original solve; an initial deflected request without a candidate transition
avoids the extra swept proof. The 16-transition bound remains in force. All
32 movement tests pass, including original PoK swimming and Kelethin lift landings;
logs: `/tmp/openeq-planar-medium-movement-final.log`. The integrated full workspace
checkpoint is recorded separately when complete.
