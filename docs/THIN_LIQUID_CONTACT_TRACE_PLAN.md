# Thin-liquid transitions on an explicit contact trace

2026-10-01. Research proposal following
[the deflected-movement investigation](THIN_LIQUID_DEFLECTED_MOVEMENT.md) and
[the wall-prefix review](THIN_LIQUID_DEFLECTED_REVIEW.md). No production
collision or movement implementation is changed by this plan.

## What a trace would establish

A solver that advances to contacts and records accepted, timed travel can
support liquid transitions without assuming that its complete path or liquid
membership is monotone. Adding timestamps to the current solver's correction
log does not create such a trace. `move_step` starts with a requested endpoint,
projects support, tries overlap corrections and raised supports, and accepts an
endpoint. It does not compute when a wall was first reached. At ordinary speed,
the reproduced wall witness uses only one substep.

The legacy response family also has nonmonotone tangent rounding: adjacent
requested time fractions can produce wet, then dry, then wet points within one
ordinary water box. Neither an endpoint chord nor binary search over freshly
solved prefixes establishes its first crossing. Recording more failed or
obstructed candidates does not remedy that missing contract.

There are two distinct implementation directions:

- Preserve the existing solver and develop conservative bounds on its actual
  prefix responses. This remains an enclosure problem, even with observation
  hooks added to the solver.
- Define a new swept-contact movement policy that produces timed travel as it
  resolves collision. The search prototype below applies to this second
  direction once such a solver supplies valid segments.

**The proposed swept-contact policy is revised OpenEQ behavior, not recovered
native-client physics and not a behavior-preserving liquid fix.** A constructed
flat-wall trace ends dry at approximately `[0.09900002,0.33333337,0]`; the current
depenetration solver gives `[0.09899999,0.33333316,0]`. These differences cannot
be hidden by calling the new path an exact trace of the old solver. If exact
existing dry movement is mandatory, retain the legacy solver and pursue the
first direction. Do not silently splice a newly constructed path into a legacy
endpoint or select different collision policies merely because liquid metadata
was loaded.

## Recommended first implementation boundary

Start with a diagnostic solver for one uncapped request, grounded on one flat
support triangle beside one isolated axis-aligned wall. Prove full swept
footprint coverage on the floor and full tangent/body-height coverage on the
wall. The initial support must be exact, the body initially clear, and the
static and optional dynamic collision worlds immutable for the request.

Check the full support step/drop envelope and body clearance in both worlds.
Conservatively reject competing support, another wall, a corner, a ceiling,
triangle edges, a raised step, a slope, an initial overlap, upward motion,
displacement caps and geometry that changes during the tick. This intentionally
does not certify arbitrary coplanar floor patches or every triangulation of the
earlier wall fixture; those need separate coverage proofs.

The new solver must explicitly choose its clearance plane and contact-time
rounding. It moves to that plane, suppresses only inward normal velocity, and
spends the remaining time sliding. Moving away must release the constraint.
After a liquid event changes velocity, discard the planned remainder and solve
from the accepted event state. Gravity and jump impulses remain once per fixed
tick. Contact, liquid and time accounting must share one authoritative state;
calling the old whole solver with a shorter displacement is not continuation
of the new trace.

Suggested trace records are:

| Record | Required meaning |
| --- | --- |
| `Travel` | Time interval, authoritative position evaluator, conservative bounds for any subinterval, and accepted collision state at its end |
| `Contact` | Zero-time update of active constraints at an accepted position |
| `Correction` | Explicit zero-time relocation; its connecting line contributes no traveled liquid distance or elapsed time |
| `Unsupported` | No valid trace for the complete tentative request |

The first family should avoid corrections by admission rather than trying to
assign time to them. Failed candidates never become `Travel`. Later support
snaps or separation corrections must query the resulting medium at their
destination, without treating the correction chord as a swim interval.

Choose and document the exact time/position arithmetic before integration.
The reference search uses nonnegative representable f32 fractions and an
explicit rounded affine position evaluator. Its results concern reachable
sample positions, not every hypothetical continuous point between them.
Absolute tick-time endpoints should own accounting so that splitting neither
duplicates nor loses elapsed time. Nonadvancing representable events need a
hard transition limit and a last-safe-state result.

## Search without monotone liquid membership

Initially admit only identity-rotation authored boxes. Prepare the boxes that
can intersect the complete trace bounds. Native-format precedence, rotated
boxes and BSP split-plane semantics remain unsupported until separately
bounded; do not use merged `LiquidRegions::segment` spans as an oracle for this
search.

For each accepted travel segment, visit parameter intervals chronologically:

1. Enclose every actual rounded body-center position in the interval.
2. Prove it dry if every box is disjoint from that enclosure. Prove it wet if
   one box contains the whole enclosure. Otherwise report unknown membership;
   failure to prove coverage by a union is a conservative rejection.
3. Skip an interval only when its proven membership equals the current medium.
4. Split uncertain intervals, processing the earlier half before the later
   half. At a single representable fraction, query the actual point.
5. Return the first opposite-medium point only after every earlier interval
   was processed or proved unchanged. A verified later point cannot justify
   skipping an unresolved earlier interval.

Return separate `Found`, `NoEvent` and `Unknown` outcomes. A fixed work budget
must produce `Unknown` when exhausted. Keep the existing 16-transition cap
separate from the interval-search budget. Any fallback must discard the whole
tentative integration; it must not leave an earlier partial medium change
committed. The diagnostic prototype uses 256 search nodes per query; the final
runtime budget and candidate-volume limit still need measurement.

For the prototype's finite constant-velocity evaluator, each rounded coordinate
is monotone by construction, so evaluated endpoint minima/maxima enclose its
whole subinterval, including rounded body-center addition. This establishes
bounds, not monotone liquid membership: multiple boxes can still produce many
wet and dry intervals, and successive travel segments can reverse direction.
This endpoint-bound argument is explicitly invalid for the legacy wall-prefix
function. More complicated source evaluators require their own outward
arithmetic or geometric bounds.

## Required adversarial checks

- The dry-endpoint wall witness, water behind the wall, and water intersecting
  only a rejected correction attempt. Only accepted travel may consume wet
  time.
- A liquid entered before wall contact, including swimming that reverses
  direction. Recompute both contact time and the remaining path.
- Coincident contact/liquid events, motion away from contact, stationary
  intervals and repeated zero-time changes. Reapply the new velocity against
  current constraints without spending time twice.
- Thin boxes beside the clearance plane and correction destinations. No
  arbitrary spatial epsilon or invented travel through a separation segment.
- Multiple wet intervals, backtracking paths, rounded diagonal coordinates at
  large positions, and the representable dry gap between boxes whose public
  segment fractions appear to touch.
- Fully dry and fully submerged trace bounds, plus bounded search exhaustion.
  `Unknown` must remain distinguishable from proof of no event.
- Competing floors, wall corners, triangle edges, stairs, slopes, ceilings,
  capped requests and changed substep partitions. Unsupported later work must
  discard the complete tentative integration.
- Equivalent static/dynamic geometry arrangements, frozen snapshot ownership,
  once-per-tick gravity, exact time accounting and the transition cap.

Use an independent contact-time oracle for the new solver's simple flat-wall
family, and exhaustive finite time windows for the boundary search. Compare
new dry outcomes with legacy results to measure the policy change; do not
mistake those comparisons for an expectation of bit-identical movement.

## Frozen search prototype

`/tmp/openeq-contact-trace-policy.py` is a standalone research program over
supplied timed affine segments. It **does not construct or certify collision
contacts**, execute the native client, implement a live movement policy, or
prove coverage for arbitrary geometry.

It locates a supplied wall-tangent entry, a rounded diagonal thin-box entry,
the representable dry gap between two boxes, and entries in both directions of
a backtracking trace. Full-range searches visit 41–54 nodes. Uniform dry/wet
bounds finish in one node, and explicit budget exhaustion returns `Unknown`.
It also agrees with exhaustive membership checks over 384 narrow f32-word
windows, totaling 74,112 points, with multiple tiny boxes, both X travel signs
and large Y offsets. Those local searches use at most 14 nodes. This finite
validation supports the search implementation; it is not a collision proof.

Reproduce with `python3 /tmp/openeq-contact-trace-policy.py` and compare stdout
with `/tmp/openeq-contact-trace-policy.json`. These artifacts are frozen; later
experiments should use different filenames.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-contact-trace-policy.py` | `fff2d8acda1d00b83f256c2890f95b706a66e9c8c696fdaf403309c9256947e9` |
| `/tmp/openeq-contact-trace-policy.json` | `edf4c683b5cd4acca33a74cf3ca3e4be3601b0bae4e2cd8b460001b5b5d23004` |

Root independently replayed the supplied-segment search prototype; the result
JSON matches the frozen hash above. No collision solver is enabled by this replay.
