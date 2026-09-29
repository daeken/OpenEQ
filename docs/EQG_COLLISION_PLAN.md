# Binary EQG collision: evidence and implementation plan

Initial read-only investigation, 2026-09-29, for the Bloodfields item in
[COMPATIBILITY_SWEEP_PLAN.md](COMPATIBILITY_SWEEP_PLAN.md). This work inspected
public source and the installed archive, then exercised temporary CPU collision
worlds. That investigation changed no runtime source, original assets or server
state. Subsequent implementation review is recorded below. No original-client
physics comparison or live route was performed for this investigation.

## Supported semantics and limits

The current EQEmu map pipeline establishes **`(polygon.flags & 1) == 0` as
collision eligibility**, independently of material and the other flag bits:

1. EQEmu/zone-utilities revision
   `b361e63dd067e8959f5bf2341579f481d2374fd5` defines a polygon as three indices,
   signed material and unsigned flags in
   [eqg_structs.h](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/common/eqg_structs.h#L122).
   Its [model reader](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/common/eqg_model_loader.cpp#L108)
   retains every polygon, including material -1.
2. [Map::CompileEQG](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/azone/map.cpp#L729)
   sends TER polygons with bit 0 set to `AddFace(..., false)` and all others to
   `AddFace(..., true)`. `AddFace` separates noncollision and collision buffers.
   There is no material or other flag test on this path.
3. The [MOD serialization path](https://github.com/EQEmu/zone-utilities/blob/b361e63dd067e8959f5bf2341579f481d2374fd5/src/azone/map.cpp#L211)
   writes a byte named `vis`, zero precisely when bit 0 is set. This variable's
   name alone would be ambiguous. Its actual consumer settles the question:
   EQEmu revision `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`,
   [Map::LoadV2](https://github.com/EQEmu/EQEmu/blob/4aceae18b94ffaafc08e2b17bc41cd72c77f795d/zone/map.cpp#L630),
   skips zero-`vis` model polygons and adds the others to the collision mesh
   after the placement transform. The same loader skips the serialized
   noncollision terrain buffers at lines 542–548.

This is authoritative for **current EQEmu map compatibility**, not proof of
all original-client physics or rendering behavior. In particular:

| Field | Evidence | Action justified now |
| --- | --- | --- |
| Bit 0 / `0x1` | Executed map-generator and server-consumer branches | Exclude from physical geometry when set; preserve drawing |
| Material `0xffffffff` / signed -1 | Current map reader retains it; collision eligibility is material-independent | Preserve its bit-0-clear polygons through material-free physical geometry |
| Bit 1 / `0x2` | Quail calls it `Transparent`; Bloodfields pairs it with material -1 | Do not change rendering from this bit alone |
| Bits 2–4 | Quail names `CollisionRequired`, `Culled`, `Degenerate` | Do not add override rules; EQEmu's collision test does not use them |
| Bits 5–31 | No original-client or behavioral interpretation established | Preserve raw values; use only bit 0 for collision eligibility |

[Quail's constants](https://github.com/xackery/quail/blob/776fc1acc7676c6984feb6fe0ff9353c4f9e566f/raw/mod_read.go#L86)
are corroborating terminology, not sufficient physics/rendering evidence. Its
WCE conversion reads/writes the named low bits; no consumer located establishes
how `Transparent` or `CollisionRequired` should change this client. Quail's
source is pinned at `776fc1acc7676c6984feb6fe0ff9353c4f9e566f`.

EQ Sage revision `d5dc8328c9e30a88e352ca8a81ccb0efb32bc986` retains signed material
and raw flags in its parser and [omits material -1 in visual export](https://github.com/knervous/eqsage/blob/d5dc8328c9e30a88e352ca8a81ccb0efb32bc986/sage/lib/eqg/gltf-export/common.js#L165).
That supports preserving OpenEQ's existing no-draw decision for this sentinel;
it does not independently establish bit 1's meaning. The older deprecated
EQEmu azone2 `ter.cpp` skips material -1 entirely, unlike the current pipeline.
The old C# OpenEQ reader preserves flags but does not use them when grouping
physical meshes. Neither older path should override the current source evidence.

## Installed Bloodfields inventory

Archive: `/Users/daeken/EverQuest/bloodfields.eqg`, SHA-256
`30d1dfb43fbdf96f27ba3f3f5f7aad78bf889c963e90ad5dc547edde937ac3f0`.
All 523 MOD payloads and one TER payload are version 2. The archive has
487,905 vertices and 311,023 polygons before repeated references/placements.
Every source polygon vertex index is within its payload's vertex array.

| Material resolution / flags | Unique archive polygons |
| --- | ---: |
| Resolved, exact `0x0` | 83,462 |
| Resolved, exact `0x1` | 9,644 |
| Material MAX, exact `0x2` | 14,059 |
| Resolved, nonzero bits 16–31 only | 203,858 |
| Other unresolved material indices | 0 |

Inclusive `flags & 1` and `flags & 2` counts are therefore exactly 9,644 and
14,059 respectively. No polygon combines these two bits, combines a high bit
with either low bit, or sets bits 2–15. Those combinations require synthetic
coverage; their absence is not permission to use equality instead of a mask.

All material-MAX faces are in `ter_ground.ter`. Their scene-space bounds are
`[-1215.933838, -2087.265625, -1060.285034]` to
`[2501.282227, 2155.876465, 276.263184]`. Several are broad zone-enclosure planes;
use the local wall below for the first movement regression, not an enclosure
floor as an arrival/spawn target.

High-bit exact counts are: `0x10000`: 25,330; `0x20000`: 30,908;
`0x40000`: 92,232; `0x80000`: 32,122; `0xa0000`: 360;
`0x100000`: 7,455; `0x200000`: 3,196; `0x400000`: 4,297;
`0x800000`: 657; `0x1000000`: 1,061; `0x2000000`: 793;
`0x4000000`: 1,068; `0x8000000`: 813; `0x10000000`: 1,489;
`0x20000000`: 617; `0x40000000`: 843; `0x80000000`: 617.
These values establish useful regression inputs, not high-bit semantics.

The ZON has 530 object references to those 524 payloads, 697 placements,
three regions and 134 lights. The four repeated filenames are
`obp_tree_marshe10.mod` (two references), `obp_tree_marshe13.mod` (four),
`obp_tree_marshd04.mod` (two) and `obp_tree_marsha31.mod` (two).
Keep archive counts, reference counts and expanded world counts distinct.

The current loaded scene has 971 meshes/material entries, 529 object
definitions and 297,388 drawable source triangles. Its collision world has
320,817 valid triangles. A temporary world applying the proposed classification
has **324,800**: 14,059 added material-free triangles and 10,076 removed
noncollision triangles after placements. It retains 301,379 physical source
triangles before instance expansion. Bloodfields has no water shader in this
inventory, so this query does not exercise the required water exclusion.

## Concrete source fixtures

All ordinals are zero-based. Coordinates are asset/scene XYZ, Z up. The current
loader and EQEmu map generator treat TER positions directly; **do not apply the
ZON TER placement to these already positioned vertices**. MODs use the same
scale/rotation/translation as their existing drawable instances. Server X/Y
conversion belongs at the protocol boundary, not in collision collection.

### Hidden hallway wall with visible starting support

`ter_ground.ter`, object reference 236, polygon **107681**, byte offset
5,256,805, indices `[64421, 64422, 64423]`, material `0xffffffff`, flags `0x2`:

| Vertex | X | Y | Z |
| --- | ---: | ---: | ---: |
| A | -809.419128 | -1206.012207 | -944.993225 |
| B | -760.623169 | -1275.877686 | -944.993225 |
| C | -760.623169 | -1275.877686 | -825.561462 |

Normal is approximately `[-0.81983715, -0.5725967, 0]`. With radius 1, height 6
and step 2, the existing movement query produces:

| Query | Scene XYZ |
| --- | --- |
| Supported starting feet | `[-780.9877, -1255.4523, -932.1253]` |
| Requested displacement | `[8.198372, 5.7259674, 0]` |
| Current result | `[-772.7894, -1249.7272, -931.43945]` |
| Proposed result | `[-777.7089, -1253.1626, -932.1253]` |

Adding **only this triangle** to the current physical world produces the same
blocked result, so the change is attributable to the named source face.
The starting floor is visible `obj_eghallway13.mod`, object 259, placement 422,
polygon 1, material 6, flags `0x80000`. Its world vertices are
`[-941.74194,-1323.8956,-932.1253]`,
`[-919.5297,-1356.2445,-932.1253]`, and
`[-750.78937,-1239.7545,-932.1253]`.

The reverse supported start `[-772.7893,-1249.7262,-931.43945]`, displacement
`[-8.198372,-5.7259674,0]`, currently reaches
`[-780.98755,-1255.4513,-932.1253]`; the proposed world stops at
`[-776.06805,-1252.0156,-932.1253]`. Use approximately 0.03-unit tolerance for
these rounded coordinates. This proves a supported local CPU walking case;
liquid classification, native-client intent, an arrival route and live NPC
navigation at this location have not been verified.

### Visible passable leaves and camera obstruction

`obp_tree_marsha06.mod`, object 147, polygon **0**, indices `[0,1,2]`, material 1
(`Leaves`, `Chroma_MaxCB1.fx`), flags `0x1`. All 184 polygons in this payload
have bit 0 set. Placement 287 has position
`[1103.276123,-664.983093,-923.558228]`, raw stored Z/Y/X angles
`[-1.57079637,0,0]` radians and unit scale. Its world triangle is:

`[1119.6532,-642.07855,-877.3079]`,
`[1075.5581,-646.9130,-886.95776]`,
`[1109.3385,-648.10333,-851.7874]`.

A zero-padding camera query from `[1101.2278,-643.74927,-871.67426]` toward
`[1101.8054,-647.64734,-872.3610]` currently stops on the face at
`[1101.5166,-645.6983,-872.01764]`. The proposed world reaches the requested
endpoint. Its vertices, material, alpha treatment and drawable indices remain
unchanged in the temporary scene. This is a camera-obstruction fixture, not
proof that an ordinary walking player reaches this elevated leaf.

### Ordinary and high-bit controls

- `obj_archway.mod`, object/placement 6, polygon **22**, material 4, flags zero,
  is an ordinary physical control. Polygon **0** uses that same material with
  flags `0x20000`. Both must stay drawable and physical. Their world centroids
  are approximately `[2251.30127,-1791.10820,-929.08232]` and
  `[2239.98877,-1741.78971,-931.59865]` respectively. Placement position is
  `[2248.472900,-1763.089355,-879.103333]`, Z yaw `-1.57079637`, scale 1.
- `obj_egtower04.mod`, object 244, placement 406, polygon **192**, material 5,
  flags `0xa0000`, has world centroid `[1320.99337,-446.86384,-761.79942]`.
  This tests more than one high bit without changing collision eligibility.
- `obj_tree_burnte23.mod`, object 200, placement 362, polygon **149**, material 0,
  flags `0x80000000`, has world centroid `[77.53897,-408.80608,-766.57682]`.
  This guards against signed-flag handling.
- `ter_ground.ter` polygon **107667** is a hidden enclosure floor at
  Z=-1060.285034. A ground query at centroid XY `[23.138184,-672.88495]`, feet
  Z=-1060.235034, step 0.1/drop 2, changes from `None` to that floor height.
  Retain as a structural test only; it is not a recommended gameplay location.

## Smallest implementation preserving drawing

Keep `TerMod::mesh_groups` and the existing material-based draw bake unchanged.
Splitting drawable batches by collision flags would unnecessarily alter draw
counts/order; equipment also uses this grouping API. Reuse the separate
`CollisionGeometry` channel instead:

1. Add a narrow loader child helper that collects original `(a,b,c)` indices
   only when bit 0 is clear and either the material resolves to an ordinary
   nonwater material or its raw index is **exactly `u32::MAX`**. Other unresolved
   material indices stay unavailable; missing textures on a resolved material
   do not make it unresolved. Do not infer anything from material names.
2. Preserve the existing water-material exclusion before physical collection.
   A material-free MAX face has no shader and follows the bit-0 rule. Do not
   invent liquid volumes or reinterpret `0x2` as water/transparency.
3. Once the replacement physical path is installed for an EQG object, set its
   drawable `Geometry::collidable` values false to prevent duplicate physics.
   Keep all drawable vertices, indices, normals, UVs, materials and order
   identical. This extends the separate channel from additional hidden faces
   to an independent EQG physical mesh; update comments that describe the
   channel as exclusively invisible.
4. Record physical mesh ownership on the same `SceneObject` as the draw meshes,
   including objects with no drawable material. TER physical geometry remains
   unowned/direct. Preserve object definitions with no instances but do not
   let them collide at origin. Static instances, extracted object models and
   dynamic door/lift worlds must use the existing transform path exactly once.
5. Validate index bounds, finite source vertices and exact nondegeneracy using
   f64 triangle area before publishing physical geometry. Do not apply a
   world-space area threshold before an instance transform. Keep the winding from
   `mesh::pack` (`a,b,c` for EQG); do not copy WLD's separate winding correction.
   `CollisionWorld` retains its f32 finite-area and `1e-10` minimum area-squared
   checks after the transform. Bounds consumers retain their own validity checks.

Do not change dynamic lighting, LIT decoding, shader detail blending, liquid
regions, rendering flags or high-bit interpretation in this slice. It should
require no protocol/server changes. Existing `CollisionWorld` and door/lift
consumers already support the physical ownership channel.

## Validation and ownership proposal

The next slice can assign `crates/openeq-assets/src/loader/eqg_collision.rs`
and a new `crates/openeq-assets/tests/eqg_collision.rs` to one owner; the root
integrates `append_eqg_object` and any channel comments after the current
heightmap/movement checkpoint. Coordinate edits to the shared loader; no
implementation was started by this investigation.

Required synthetic tests:

- Table-driven classification for each of bits 1–31 alone and ORed with bit 0,
  plus `0x80000000`, `0xfffffffe`, `0xffffffff`, `0xa0000`, and combinations of
  bits 0–4. Assert only bit 0 decides physics for a resolved nonwater material.
  Include mixed flags within one material to prove draw grouping stays intact.
- Exact MAX material with bit 0 clear/set, an arbitrary missing material ID,
  resolved water, resolved ordinary material with a missing texture, invalid
  indices, nonfinite positions and exactly zero source area. Verify tiny source
  faces scaled up and large source faces scaled down against the original
  transformed drawable-geometry collision path; invalid world area is still
  rejected after placement.
- Uninstanced physical-only MOD contributes no world triangles; two instances
  with translation/rotation/scale contribute at both intended positions and
  not the original local origin. Include negative/nonuniform instance scale,
  object extraction and original local winding. TER stays in direct scene space.
- Visible floor plus separate physical wall demonstrates body and camera
  blocking; a drawn bit-0 face is passable. No physical duplication. Exercise
  a hidden-only moving door/lift through existing extraction and bounds code;
  preserve animated support/carrying, removal of old pose and removal of object.

The explicit installed-asset fixture must fail or report a deliberate skip
when Bloodfields is unavailable, never silently count missing data as coverage.
Assert the source distributions, exact named faces/placements, unchanged
297,388 drawable triangles and 971 material/mesh entries, the expected physical
counts, the hallway forward/reverse results and the leaf camera result above.
Retain ordinary/high-bit controls. Snapshot drawable attributes/indices and
materials; run GPU A/B at the same camera/settings to verify pixels, GPU bounds,
color/depth/shadow input and draw inventory are unchanged. Inspect the capture.
Re-run Anguish water, PoK invisible rendering, Timorous collision, placed-object
orientation and dynamic-door regressions after integration. The A/B render and
those regressions are proposed work, not completed evidence in this document.

For performance, the recovered hidden set includes very large enclosure faces;
measure its spatial-grid/global-list impact and representative query/build
costs instead of asserting that the triangle-count delta predicts frame time.
Do not change collision broadphase or movement policy without a measured need.

Temporary audit source/output: `/tmp/openeq-bloodfields-flags.py`,
`/tmp/openeq-bloodfields-flags.json`, `/tmp/openeq-bloodfields-flags.txt`,
`/tmp/openeq-bloodfields-queries.rs`, and
`/tmp/openeq-bloodfields-queries.txt`. The CPU probe linked an ordinary successful
`openeq-assets` development build; it changed only an in-memory scene. Public
reference clones are under `/tmp/openeq-{zone-utilities,quail,eqsage}-collision`.
Original proprietary payloads were not copied into the repository.

## Implementation review: source area versus instance scale

The initial helper used CollisionWorld's f32 area cutoff in model coordinates.
Review reproduced two regressions before integration: a triangle with local
edges `1e-4` scaled by 100 has source area-squared about `1e-16`, while one with
edges `1e20` scaled by `1e-18` overflows f32 source area. Both produce one valid
physical triangle through the previous post-transform collision path. Rejecting
either in the collector would lose previously valid instanced geometry.

The collector now uses finite source positions and finite, nonzero f64 area;
the existing CollisionWorld insertion remains responsible for the numerical
threshold and overflow checks in actual world coordinates. A focused regression
compares the new physical channel with `add_geometry` for both scales, including
their matching ground query. Invalid indices, nonfinite vertices and exact
degeneracy remain excluded before physical vertices are published.

Existing dynamic-door local support and hidden-only extent queries still use
local f32 area thresholds. That preexisting scale limitation is not expanded
into a door/support redesign here. Visible door bounds continue to come from
their unchanged GPU geometry, and hidden-only bounds retain their current
overflow protection. Static placed collision uses the instance transform and
therefore benefits from the corrected collector validation.


## Implemented and targeted verification

The collector now supplies the separate physical channel for binary EQG terrain,
placed MODs, object libraries, and heightmap-zone props. Every original drawable
batch remains in the same order with unchanged attributes/materials; only its
old physics flag is disabled to avoid duplicate collision. Exact MAX material
is retained, bit 0 controls passability, and resolved water remains nonphysical.
Missing arbitrary materials are diagnosed, not turned into hidden barriers.

Seven helper tests, five public-loader integration tests (including installed
Bloodfields), and three serial GPU tests pass. The loader fixtures cover mixed
flags in one material, missing textures/materials, water, direct TER coordinates,
unplaced hidden-only MODs, extraction, negative/nonuniform transforms and source
scale extremes. The dynamic GPU fixture verifies hidden lift support, carrying,
old-pose removal, sliding bounds and complete removal through DoorRenderer.

Bloodfields retains the independently captured pre-change drawing fingerprints:
geometry `788b0e681cba32fa`, materials `40ac2b6644e185fb`, draw ownership/transforms
`8ac04774af24cb60`. It still draws 297,388 source triangles in 971 mesh/material
batches, with 529 objects and 697 instances. The valid collision world changes
from 320,817 to 324,800 triangles. Forward/reverse hidden-wall blocking, passable
leaves and ordinary/high-bit controls pass at the named source locations above.

GPU A/B includes the original lights and shadow pass with sky/texture animation
frozen. Draw inventory, buffers, bounds and every rendered pixel are equal.
Both inspected captures at `/tmp/openeq-bloodfields-collision/{before,after}.png`
have SHA-256 `4b1331c2883c3636a9aa37452c78600f0f57b336162eda228d6d25cbcd8295a0`.
Full workspace verification is tracked in `OVERNIGHT_2026-09-29.md`.

## Measured cost and wider load sweep

A temporary optimized CPU probe reconstructs the previous physics from unchanged
drawable meshes, then compares it with the new physical channel. Medians contain
seven batches: ten world builds, 1,000 movement queries or 10,000 camera queries
per batch. These are local fixture measurements, not whole-zone frame times.

| Bloodfields fixture | Before | After |
| --- | ---: | ---: |
| Collision-world build | 13.229 ms | 11.073 ms |
| Supported hallway move `[0.08,0.05,0]` | 29.717 µs | 53.319 µs |
| Ten-unit hallway wall crossing | 953.448 µs | 1366.446 µs |
| Archway small move | 51.522 µs | 78.037 µs |
| Leaf camera query | 4.332 µs | 6.849 µs |

Grid cells increase 2,546→2,839, indexed references 440,869→448,147, and large
triangles held outside the grid 0→16. The new hallway query does more work to
block the authored wall. A temporary cell filter for large triangles gave no
meaningful gain on this fixture and was not incorporated. No collision
broadphase or movement policy changed in this slice.

A separate explicit load/build sweep succeeded for all twelve installed zones:
Anguish, Bloodfields, Crescent Reach, Ashengate, Crystallos, West Freeport,
Feerrott2, Dead Hills, Loping Plains, Buried Sea, Old Commonlands and Nektulos.
This exercises both binary terrain and heightmap prop collection. Examples of
valid expanded physics counts are Feerrott2 3,138,384→824,400 and West Freeport
786,767→399,069, largely from excluding authored passable props. Reduced counts
are not proof of frame-time improvement, whole-zone traversal or NPC parity.

Temporary probes and full outputs: `/tmp/openeq-eqg-collision-bench.rs`/`.txt`
and `/tmp/openeq-eqg-compatibility-probe.rs`/`.txt`. They did not change original
assets or any live fixture/server state.
