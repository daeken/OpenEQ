# A conservative enclosure of the existing wall-prefix response

2026-10-01. Standalone research following
[the contact-trace plan](THIN_LIQUID_CONTACT_TRACE_PLAN.md) and
[the nonmonotone wall-prefix witness](THIN_LIQUID_DEFLECTED_REVIEW.md).
No production collision or movement behavior is changed.

The existing solver can be enclosed for the fixed floor/wall fixture without
assuming monotone tangent motion or constructing a different movement policy.
The prototype finds the known first wet fraction, word **1053270553**, in
**138 chronological search nodes**, including one singleton evaluation. The
ordinary thin box beginning near Y=0.13 takes 134 nodes. A behind-wall box is
proved unreachable in 737 nodes; a 256-node budget returns `Unknown` for that
query. These counts support further investigation, not a production performance
claim or an admission rule for arbitrary geometry.

## Exact scope

The world is exactly the two quads from the frozen witness, triangulated
`[0,1,2, 0,2,3]` in this order:

```text
floor = [(-50,-50,0), (50,-50,0), (50,50,0), (-50,50,0)]
wall  = [(1.1,-20,0), (1.1,20,0), (1.1,20,20), (1.1,-20,20)]
```

All geometry is static. Start feet are `[0,0,0]`, radius 1, height 6, maximum
step 2, and `DT = 1f32/120`. For each nonnegative representable fraction `f`
in `[0,1]`, the actual request uses this arithmetic:

```rust
let dt = DT * f;
let delta = [40., 40., -128. * DT].map(|v| v * dt);
world.move_player_with_path(None, [0.; 3], delta, 1., 6., 2.)
```

Replacing that expression with `complete_delta * f` is outside this result.
The certificate concerns the accepted response to each independently solved
prefix. It does not recover continuous native physics, certify the solver's
correction chords as traveled time, or establish arbitrary continuation after
a medium changes velocity.

The program reads clipped polygons from a copy of the current collision source.
The replay verifies that copy is identical except for an absolute submodule
path and an appended read-only inspection hook. The hook checks the complete
four-triangle geometry and normals. Exact query results are also compared with
the existing compiled `openeq_assets::collision::CollisionWorld`.

## Why this fixture reduces to one possible correction

The following facts are part of the fixed-fixture proof, not heuristics inferred
from sampling:

1. Every requested displacement has length at most 0.5, so all moving prefixes
   use one uncapped substep. The no-motion cutoff is handled separately; an
   interval straddling it includes the starting point in its output enclosure.
2. The floor gives exact Z=0 throughout the admitted XY square `[-1,1]^2`.
   For X>=Y the first floor triangle has a nonnegative cancellation numerator
   in `ground_at`; for Y>=X the second does. This follows from monotonicity of
   the same rounded addition and multiplication on each side. The other
   barycentric coordinate is in approximately `[0.49,0.51]`, and even the loose
   independent bound on their sum stays below 1. Both triangles have exact
   normal `[0,0,1]`, so the returned plane height is exactly zero. The program
   checks these numerical bounds and rejects any corrected enclosure leaving
   that square.
3. The source broad phase includes the required floor and wall triangles. The
   floor triangles occupy all four XY grid cells meeting the origin. The wall
   occupies X cell 0 and Y cells -1 and 0. Every admitted center's radius-one
   candidate rectangle includes an occupied wall cell. There are no other
   triangles, and candidate order is stable.
4. Both floor triangles are skipped by `slide` after grounding. Exact source
   clipping of the first wall triangle produces Y coordinates
   `[-8.02,-19.98,20,20]`; the second produces
   `[-20,-20,-19.98,-8.020002]`. Every clipped X has word 1066192077 (1.1f32),
   and each polygon's rounded signed area is exactly zero. This is checked,
   not assumed from geometric collinearity: `Vec3::lerp` can otherwise round
   even an ostensibly constant coordinate. Thus `inside` is false for both.
5. The second wall polygon is proved clear over every admitted input and
   corrected enclosure by bounding all its edge distances.
6. The raised-support branch is excluded explicitly. Floor contributions to
   `max_z_in_footprint` are zero. The vertical wall has no `ground_at` value
   and no finite normalized uphill gradient. The prototype bounds every
   possible wall-edge height contribution using the source projection and
   extent arithmetic. A contribution is either at/below `SKIN` or above
   `max_step + SKIN`; their maximum therefore cannot be an admitted raised
   support. This matters because that source function also examines vertical
   wall edges. Merely calling the wall non-walkable would not exclude steps.
7. Whenever the first wall produces an applied push, the interval certificate
   proves its length is at most 2, the correction remains within the source
   distance limit of 4, and the corrected enclosure is clear of both wall
   polygons. Subsequent iterations and the post-grounding second `slide` are
   therefore unchanged. If a push is absent or below the source threshold,
   no triangle changes the point; repeating that same pass is also unchanged.

If an interval cannot establish this branch family, the enclosure routine
returns no certificate for that interval. The search subdivides it; it does
not assume the family holds or treat uncertainty as dryness.

## Rounded arithmetic and branch coverage

The interval evaluator encloses **rounded f32 operations**. For addition,
subtraction and square root it executes the source operation at the appropriate
f32 endpoints. Multiplication and division take all four corners; division
rejects a denominator spanning zero. Squaring includes zero when needed.
Round-to-nearest is monotone, so these bounds enclose all rounded outputs of
that operation. An extra arbitrary epsilon or a real-arithmetic formula is
unnecessary. The replay uses glam 0.30.10, checks that `fast-math` is disabled,
and relies on the inspected scalar Vec2/Vec3 operation order.

For every clipped edge, it bounds the projection, clamping, nearest point and
rounded squared distance. An edge remains a possible winner whenever its
distance lower bound is no greater than the smallest candidate upper bound.
Every retained winner is evaluated; the prototype never guesses between the
two nearly identical long-edge projections. The winning distance is intersected
with that common upper bound.

The response enclosure unions the branches for no push, a threshold-skipped
push, and an applied push when each is possible. The applied branch uses the
actual subtraction, square root, division, penetration, skin multiplication
and coordinate-addition order. Distance is proved above the fallback threshold.
The one-correction clearance check then establishes that no later push can
change the result. This preserves the tiny tangent reversals rather than
assuming they disappear at a smaller time scale.

## Chronological search and checks

The search visits f32-word intervals from earliest to latest. It skips only
enclosures proved dry, subdivides unresolved intervals, and calls the original
point solver for a singleton. An enclosure entirely inside the authored box
can return its first word after checking that point. Every earlier interval
has already been proved dry or inspected. Budget exhaustion returns `Unknown`
with the first unresolved interval; a known later wet point cannot bypass it.

The boxes use identity rotation. Their centers and half-extents are first
constructed in f32 exactly as in the earlier witness, then interpreted by the
source f64 membership query. The interval classifier uses those same exact
dyadic bounds. No rotated-box, BSP or native terrain precedence is certified.

| Query over all f32 fractions in [0,1] | Result | Nodes |
| --- | --- | ---: |
| Known tangent reversal, low Y=0.12996963 | First wet word 1053270553 | 138 |
| Thin box, low Y=0.13 | First wet word 1053273616 | 134 |
| Behind wall: X in approximately [0.19,0.21] | No event | 737 |
| Box wholly outside on negative X | No event | 737 |
| Entry before wall contact | First wet word 1041865114 | 49 |

A complete partition of the full fraction domain contains 369 admitted
enclosures. For every enclosure the program compares endpoints, midpoint and
128 deterministic interior samples against both the source copy and compiled
production solver: 48,339 comparisons. It separately exhausts 160 local
257-word windows around contact and tangent reversals: 41,120 points. Across
those windows, 1,600 thin/wide-box searches agree with exhaustive first-wet
checks and use at most 63 nodes. These finite checks validate the implementation;
the operation/branch argument supplies the reason an admitted interval encloses
all its values.

## Recommended production boundary

Retain this as a standalone result this window. A fixture-specific predicate
must not become a general `Deflected` path certification. A future diagnostic
implementation can be bounded as follows:

- Prepare an opaque certificate borrowing immutable static and optional dynamic
  collision worlds. Carry the exact requested-prefix evaluator and its rounded
  displacement bounds; do not derive it by multiplying an already rounded full
  displacement by the fraction.
- Require exact initial flat support, downward motion, one uncapped substep for
  the whole prefix family, and fixed body parameters. Initially require one
  flat triangle whose interval barycentric test contains the whole possible
  center region; other coplanar floor triangles may be harmless candidates.
  Supporting a union across a floor seam needs its own coverage proof, as the
  specific two-triangle proof above demonstrates.
- Gather all potentially observed triangles in both worlds. The candidate
  envelope must cover requested points, every still-admissible correction
  within the source four-radius limit, the body footprint, and support checks.
  A candidate list near the complete endpoint alone is insufficient. Reject
  competing geometry until its branch effects are independently bounded.
- Clip the fixed-height axis wall using the existing source arithmetic and
  validate the resulting polygons. Initially admit one potentially pushing
  polygon and require every other wall polygon to be proved clear before and
  after correction. Prove raised-support exclusion with the actual edge-height
  arithmetic; reject ceilings, slopes, corners and unresolved supports.
- Expose conservative position bounds or `Unsupported` for each interval.
  Ambiguous nearest edges and push thresholds must remain unions. Split a
  failed interval certificate chronologically, returning `Unknown` if the
  shared work budget cannot resolve the earliest pending range.

Before enabling liquid integration, that general admission needs separate
adversarial tests for candidate closure, translations and axis swaps, static/
dynamic ownership and ordering, floor edges, failed-clearance branches and
changed motion after a liquid event. Any unresolved later work must discard
the whole tentative integration. A successful prefix search does not itself
establish time accounting or continuation behavior.

## Frozen artifacts

Replay with `python3 /tmp/openeq-wall-enclosure.py`. It checks source identities,
the read-only source-copy construction, matching glam dependency, and the exact
stdout hash. It does not overwrite frozen evidence or repository files.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-wall-enclosure.rs` | `b424b96c369903f42f37d89e70805727b0a87235214f32eeb953c96e0c2bd03c` |
| `/tmp/openeq-wall-enclosure-hooks.rs` | `fa0069d5547ff97a1cd45de5a8efd3a533092d7543669874235de73bdaa302e9` |
| `/tmp/openeq-wall-enclosure-collision.rs` | `a4dc17210ba5dbe9fd86eea6a7baa42a778187afdd62b92286dde8958fbd3d5f` |
| `/tmp/openeq-wall-enclosure.py` | `11ea2ab4a1d37116eefaac132ae6393948311d7c479adc563d504623acaa43f2` |
| `/tmp/openeq-wall-enclosure.log` | `be821296d56bd177fe426532481fbd03ea121ceecbd783a39d4c8f2ecd581f27` |
| `crates/openeq-assets/src/collision.rs` | `8fe9ff9da1224b74c31578f077158f598a21f132b21b52e6ba41c5149f27a5e4` |
| `crates/openeq-assets/src/collision_ascending.rs` | `d706646e69b9918120bfcd754ddc517081f2ae3ee012c2d6ef06e482501be034` |
| `crates/openeq-assets/src/liquid_regions.rs` | `c12d86fffe2c5b7d2dca41d608d2ba93bd502093a9595b73425878378384adbe` |
