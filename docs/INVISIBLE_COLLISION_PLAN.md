# Invisible WLD collision: implementation and fixture plan

Read-only CPU investigation, 2026-09-29. No runtime source, original assets,
server state or GPU resources were changed. The audit loaded the installed
Timorous assets, built the existing collision world, then supplied selected
hidden triangles to temporary collision queries. This is a reproduced physical
query difference, not an in-game visual or live-server claim.

## Source semantics

The independent visibility and collision decisions already exist in both code
generations:

- `OpenEQ-csharp/LegacyFileReader/Wld.cs`, `Read36`, decodes a polygon as
  collidable when its leading `u16` is zero. Rust `wld::read_mesh` preserves
  the same rule. Do not reinterpret material flags as collision flags.
- `OpenEQ-csharp/ConverterCore/Converter.cs`, `CreateMeshAndSkin`, line 376,
  skips `texture.Flags == 0` with the explicit comment:
  `TODO: Bake this in, but non-renderable. Collision mesh type?`
- Rust `mesh::bake_wld_meshes` preserves material-list slots, groups triangles
  by material and collision flag, flips `(a,b,c)` to `(a,c,b)`, and then drops
  zero-render groups. Its visibility decision is correct; the discarded
  collidable triangles need a separate destination.
- EQEmu's historical `utils/deprecated/azone2/wld.cpp`, `Data30`, also identifies
  absent render/texture data as `collide.dds`. That is corroboration for the
  collision-only material convention, not a reason to classify by filename or
  to copy that old decoder's different field handling.

These are source-backed semantics from the prior client and current parser;
this investigation did not reverse-engineer the original executable's physics.
Keep the existing PoK zero-render/COLLIDE.DDS drawing regression unchanged.

## Installed Timorous inventory

The main `timorous.wld` contains 12,438 invisible **and collidable** triangles.
All 12,438 survive finite/nondegenerate validation in `CollisionWorld`.
Their overall scene bounds are:

`[-13093.21875, -7721.9375, -618.59375]` to
`[10032.59375, 5548.65625, 811.71875]`.

Of these, 8,658 have a walkable geometric slope under the existing 45-degree
rule, and 3,780 have `abs(normal.z) < 0.1`. Large portions are zone enclosures:
4,012 triangles lie at Z=-618.59375 and another 4,012 at Z=811.71875. Those
extreme planes are poor first player-facing fixtures. Do not use their height
to snap ordinary spawn positions or treat a ceiling as an arrival floor.

The current loaded scene has 227,791 source drawable triangles, 139 object
definitions and 3,298 static instances. The existing collision world has
259,201 nondegenerate triangles after instance expansion. Source drawable
counts and expanded collision counts are intentionally different quantities.
Auditing the zone's non-character supplemental S3D/WLD object archives found
zero additional hidden collidable object triangles. For this installation,
adding the main WLD channel should therefore produce **271,639** static
collision triangles without changing the drawable inventory. Synthetic object
fixtures remain necessary to prove ownership and instance transforms.

## Primary fixture: dry barrier near the Portik map label

All values below use **asset/scene XYZ**, with Z up. Polygon ordinals are
zero-based in the original mesh fragment; vertices below use baked winding.

`timorous.wld`, mesh `R10684_DMSPRITEDEF`, polygon **12**, material `M0000_MDF`:

| Vertex | X | Y | Z |
| --- | ---: | ---: | ---: |
| A | -11358.0 | -2382.03125 | -4.09375 |
| B | -11317.65625 | -2232.09375 | 96.5625 |
| C | -11317.65625 | -2232.09375 | -4.09375 |

Its material render method is zero and its polygon collision flag is zero.
The baked normal is approximately `[-0.9656546, 0.2598291, 0]`.

Use the current player's radius 1, height 6 and step height 2:

| Query | Scene XYZ |
| --- | --- |
| Starting feet | `[-11326.276, -2283.372, 10.0]` |
| Requested displacement | `[-9.65625, 2.5981445, 0.0]` |
| Current visible-only result | `[-11335.925, -2280.7744, 10.0]` |
| Result with only polygon 12 added | `[-11330.137, -2282.3325, 10.0]` |
| Tangential displacement | `[2.0786328, 7.7252368, 0.0]` |
| Tangential result with all hidden triangles | `[-11324.198, -2275.6455, 10.0]` |

The existing world reports the start supported. Its authored liquid query at
body center `[-11326.276,-2283.372,13]` returns `None`. Visible, collidable
polygon **117 in the same fragment** supports the start at Z=10:

`[-11302.6875,-2232.09375,10]`,
`[-11432.59375,-2298.96875,10]`,
`[-11323.40625,-2382.4375,10]`.

Thus this is a dry, locally walkable barrier fixture: walking across currently
passes through; adding the single authored wall stops the capsule about one
radius from the plane; walking along it still works. The reverse crossing was
also blocked by the full hidden-triangle set. An extended route from the zone
arrival point was not traversed, so do not describe that as verified.

For orientation, the installed map label `Portik_(Bar)` is
`P 2276,11594,71.1716`; the existing map conversion places it at scene
`[-11594,-2276,71.1716]`, about 268 horizontal units from the start. This is a
map landmark, not proof of a matching current NPC spawn. Server XYZ swaps the
first two scene components: the fixture's server position is approximately
`[-2283.372,-11326.276,10]`. Do not send scene coordinates directly to EQEmu.

## Secondary fixture: underwater barrier and hidden floor

A simple axis-aligned wall is `R12142_DMSPRITEDEF`, polygon 2, `M0000_MDF`:

`[-2088.1875,-2364.0625,-204.34375]`,
`[-2088.1875,-2232.09375,-4.34375]`,
`[-2088.1875,-2232.09375,-204.34375]`.

Starting at `[-2083.1875,-2276.0833,-140.90625]` and requesting `[-10,0,0]`
currently reaches X=-2093.1875. With the hidden set added it stops at
approximately X=-2087.1865. Both sides' body centers are in authored WLD water;
this must be a swimming collision fixture, not a dry-walking claim. A vertical
point query from Z=3 to body-center Z=-137.90625 at the start XY is unobstructed
in the visible world. A full swimming route was not simulated.

For a floor-only structural check, `R10698_DMSPRITEDEF`, polygon 7, has hidden
vertices `[-10795.21875,-2631.09375,-29.6875]`,
`[-10809.8125,-2232.09375,-29.6875]`,
`[-10764.09375,-2506.25,-29.6875]`. At centroid approximately
`[-10789.708,-2456.4792]`, a ground query near Z=-29.6375 with step0.1/drop2
finds no visible floor but finds the hidden floor at Z=-29.6875. This is a
structural floor fixture; its gameplay access and intended use are unverified.

## Minimal additional geometry channel

Keep `Scene::meshes` drawable and preserve `Geometry::collidable` for its
existing visible collision surfaces. Add an independent positions/indices
type, for example `CollisionGeometry`, with no material or texture requirement:

- `Scene::collision_meshes: Vec<CollisionGeometry>` stores **additional hidden
  geometry only**. Do not duplicate visible collidable meshes here.
- `SceneObject::collision_meshes: Vec<usize>` owns indices in that separate
  array, just as `SceneObject::meshes` owns drawable indices. Unowned hidden
  meshes are terrain; owned meshes collide only for matching instances.
- Keep `bake_wld_meshes`' existing return type and drawing behavior for character
  and appearance callers. Add a narrowly scoped collector for collidable
  polygons in zero-render material runs. Preserve run positions, original
  source indices and the same `(a,c,b)` winding. Warn/skip invalid references;
  do not invent materials or infer invisibility from a COLLIDE filename.
- Initialize the extra arrays empty in existing non-WLD scene constructors.
  Populate them for main WLD terrain, placed WLD objects and
  `load_object_library`. Keep object ownership even when drawable geometry is
  empty. `object_model` must copy the selected object's collision geometry into
  the extracted local model and must not discard a collision-only definition
  merely because it has no visible meshes.
- Extend `CollisionWorld::build` with the same object lookup and transform list
  for both channels. Each hidden object vertex receives exactly
  `Mat4::from_scale_rotation_translation(instance.scale, instance.rotation,
  instance.position)`. Parsed WLD vertices already include fragment center and
  quantization scale; do not apply either again, or swap scene X/Y here.
- Add a positions/indices insertion method sharing the existing finite,
  degenerate, bounds and spatial-grid checks in `add_geometry`. Preserve
  visible-water exclusion and existing two-sided body/camera queries.

The renderer continues to read only the drawable array, so these triangles
cannot enter color, depth, shadow, picking or GPU scene bounds through this
change. A material marked invisible must never be put back in the drawable
array with an artificial transparent texture.

Dynamic doors/lifts are a required follow-through touchpoint, not covered by
static instance expansion. `openeq-render/src/doors.rs` currently derives both
local and moving collision solely from `object_model().meshes`; it must also
retain the new collision channel and apply the **same current door/platform
matrix** used for the visible geometry. Preserve the existing open-type
exclusions. Collision-only models must keep physical bounds even when the GPU
scene has no draws; do not derive their movement extent solely from empty GPU
bounds. This can be a coordinated renderer slice after the assets API lands.

## Paired regression checks and file ownership

The first implementation slice should own only:

- `crates/openeq-assets/src/mesh.rs`: additional geometry type/collector.
- `crates/openeq-assets/src/loader.rs`: scene/object storage, WLD loaders and
  object extraction.
- `crates/openeq-assets/src/collision.rs`: additional-channel build/insertion.
- New `crates/openeq-assets/tests/invisible_collision.rs`: paired CPU fixtures.

Use a synthetic visible floor plus invisible collidable wall, a second
invisible **noncollidable** wall and an ordinary visible collidable wall.
Assert that the hidden walls add no drawable materials/triangles, only the
first blocks movement, and visible collision remains unchanged. Add a hidden
floor support query and a camera segment test. Include malformed/degenerate
geometry rejection without broadening material fallback behavior.

For transforms, a collision-only object with no instance must contribute zero
world triangles. Instantiate it twice with nonuniform scale, rotation and
translation; query both world locations and assert no collision remains at the
unplaced origin. Extract it through `object_model` and check local-space
geometry before separately applying a dynamic transform. Negative scaling
should remain finite and two-sided, with the existing slope policy unchanged.

The installed Timorous fixture should explicitly require its archives instead
of silently passing when absent. Assert the named source material/polygon,
start support, dry body center, crossing and tangential behavior above, using
roughly 0.03 world-unit tolerance for rounded fixture coordinates. Confirm the
drawable triangle/material inventory is unchanged while hidden collision count
increases. Keep original bytes out of the repository; small derived positions
and counts are sufficient. Re-run the existing PoK invisibility and object
transform tests alongside it.

Renderer/door source ownership should stay with its current owner until a
separate handoff. No runtime implementation is authorized by this document;
the root task will start that slice after the raid checkpoint. The temporary
CPU audit source and detailed output remain in `/tmp/timorous-hidden-audit.rs`
and `/tmp/timorous-hidden-fixture.txt` for local reproduction.

## Implemented slice and validation

The planned separate channel is now implemented in `mesh`, `loader` and
`collision`, with renderer door/lift follow-through. Five synthetic asset tests
cover independent render/collision flags, resolved-material requirements,
malformed geometry, hidden floor/camera queries, object extraction/ownership,
negative/nonuniform scale, rotation, fragment-center/quantization and archive
loading. The explicit original Timorous test verifies the named material, dry
visible-only starting support, forward/reverse blocking and tangential walking.

Targeted checks also passed the 14 existing collision unit tests, PoK hidden
material regression and placed-object orientation. Three GPU tests passed:
synthetic invisible-wall pixel/bounds equality, original Timorous A/B equality,
and hidden-only lift/slide support, passenger carrying and old-pose removal.
A review caught invalid finite vertices overflowing triangle area in the hidden
model extent calculation; the bounds path now matches CollisionWorld validation
and its focused regression passes. The original Timorous capture was inspected.

Temporary CPU performance audit (local optimized development profile, no timing
assertions) alternated seven measured build rounds and seven rounds of 1,000
queries per case. The 12,438 extra triangles add zero entries to the global
oversized-triangle list; largest XY span is 64 grid cells (fallback threshold256).

| Median | Visible-only | With hidden collision |
| --- | ---: | ---: |
| Collision build | 32.85 ms | 44.86 ms |
| Short per-frame movement | 0.497 µs | 1.035 µs |
| 10-unit barrier crossing | 11.73 µs | 30.19 µs |
| Tangential movement | 7.53 µs | 16.33 µs |
| Reverse crossing | 11.81 µs | 24.30 µs |

These are fixture measurements, not a frame-rate guarantee for every zone.
Audit source remains `/tmp/timorous-hidden-perf.rs`; GPU captures are under
`/tmp/openeq-timorous-collision`. Combined release checks are recorded in the
overnight log. No live route from Timorous arrival or native-client physics
comparison was performed, and ordinary hinged doors retain their existing
final-state collision policy while lifts use the animated pose.
