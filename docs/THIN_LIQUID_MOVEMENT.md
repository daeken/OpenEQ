# Straight movement across thin liquids

## Reproduction

The original fixed-step controller sampled the player's center only at the
start of each 1/120-second tick. `LiquidRegions::segment` already detected
positive-length wet intervals between two dry endpoints, but movement did not
use those intervals.

A synthetic water box with X bounds `[0.1, 0.2]`, broad Y bounds, and Z bounds
`[0, 6]` demonstrates the problem on a flat floor. Starting at feet `[0, 0, 0]`
with 40-unit/s walking and volume input, the old controller moves to
`[0.33333334, 0, 0]` and never enters swimming. Both center endpoints are dry;
the segment query reports water at fractions `[0.3, 0.6]`.

Using the existing configured swim multiplier of 0.6, the tick should spend
`0.1 / 40` seconds reaching water, `0.1 / 24` seconds crossing it, and the
remaining time walking. The resulting X is `0.26666667`. This is an internal
consistency regression, not a measurement of the original client's speed.

## Implemented scope

`movement.rs` now splits the remaining tick time at wet/dry boundaries for
straight movement and flat supported walking. It does not replace collision:

- First compute the existing complete collision result. Its endpoint must
  equal collision's unobstructed substep arithmetic exactly, allowing the
  existing downward clamp onto the flat floor already supporting the player.
- Query the center segment for the first wet/dry boundary. Adjacent liquid
  kinds share the same movement response and do not create a dry interval.
- Find a representable point in the next interval with at most 24 metadata
  bisections. This handles WLD's dry split planes and inclusive box faces
  without choosing a fixed world-space offset.
- Run that prefix through normal static/dynamic body collision. Verify both
  its unmodified trajectory and its actual outgoing medium before accepting it.
- Entering liquid immediately adopts the existing swim direction and speed.
  Leaving restores walking XY and carries vertical momentum. Gravity and jump
  impulses still run once per fixed tick, preserving the existing dry and
  wholly submerged integration rules.

No boundary means the original complete movement and velocity result are used.
Dry/fully submerged fixtures retain exact endpoints, including grounded jumping.
Position is always returned by the collision solver, never assigned by
interpolating between collision endpoints.

## Bounds and limitations

There are at most **16 medium transitions per physics tick**. Opposing movement
directions at a boundary can repeatedly alternate, and deliberately dense
volumes can exceed that budget. In either case, movement stops at the last
processed collision-safe point for the remaining tick, with vertical velocity
zero. Unprocessed intervals are not crossed at walking speed. The outer fixed
step still consumes its normal elapsed time.

If a complete move or any proposed prefix slides, steps, or otherwise differs
from the supported straight trajectory, the tentative split is discarded and
the original complete-tick result is retained. The collision API does not expose
the route taken through those responses; a chord between its endpoints is not
treated as that route. Thin liquid crossings during such deflected movement
remain a follow-up. Boundaries with no representable outgoing sample likewise
retain the existing behavior. Oversized collision-capped moves are excluded
because they have no constant-speed mapping from distance to tick time.

This changes no liquid classification, damage/breath rules, EQG swimming-volume
support, or NPC terrain projection. A separate synthetic submerged-NPC probe
shows that terrain projection can move its anchor above a sloping pool surface,
but fly mode 3 also describes ordinary land movement. Without evidence that an
NPC is swimming rather than walking along the submerged floor, that observation
does not justify disabling its terrain projection.

## Verification

All **25 movement tests pass**, including the opt-in original Plane of Knowledge
pool and Greater Faydark/Kelethin lift fixtures. Seven added regressions cover:

- A thin slab crossed between dry endpoints, exact time allocation, and
  equivalent elapsed movement at 10/30/60/120/240 FPS.
- Swim direction changes during the remainder of a tick.
- Terminal-velocity falling into a thin layer, then holding depth.
- Exact dry/jump and wholly submerged movement preservation.
- Exact fallback on a shore slope, and a blocking/sliding dynamic wall before
  water with no phantom swimming.
- Opposing walk/swim directions at one boundary.
- Twenty thin slabs, stopping after the sixteenth transition.

Existing tests also cover shores, stairs, ceilings, static/dynamic blockers,
swim ascent/descent, server flight/float, levitation and suspension clamping.

Independent read-only review also exercised 528 box cases and 120 synthetic
WLD BSP cases: both directions, multiple intervals, wet/dry/exact-boundary
starts, speeds from 0.00001 to 15,000 units/s, and a 0.000001-unit slab. BSP
results matched analytic time allocation within 0.00002 units. The largest
box discrepancy was about 0.0004 units at the extreme 15,000-unit/s input,
consistent with repeated collision-substep rounding. No additional blocker was
found. Temporary probe: `/tmp/openeq-liquid-independent.rs` and `.txt`.

## Local cost

A standalone optimized CPU probe compared the previous source and current
source against the same collision/assets. Each median contains seven batches
of 10,000 fresh one-tick movements. These are fixture costs on this host, not a
zone-wide frame-time guarantee. Original-asset endpoints matched exactly.

| Fixture | Before | After |
| --- | ---: | ---: |
| PoK dry, scene X/Y `[0, 0]`, supported floor | 313.875 µs | 316.831 µs |
| PoK submerged, feet `[15, 1455, -134]` | 14.380 µs | 14.567 µs |
| Greater Faydark dry, scene X/Y `[137.463, 216]` | 3.697 µs | 3.619 µs |
| Synthetic thin-slab crossing above | 0.553 µs | 3.647 µs |

Small ordinary-path differences include timing noise; the additional work is
concentrated at actual transitions. GFay has no supported liquid regions, so its
empty-region path skips segment queries. Temporary reproduction and timing
programs are `/tmp/openeq-liquid-review.rs` and
`/tmp/openeq-movement-timing.rs`; no original asset bytes are checked in.
