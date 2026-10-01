# WLD placed actors: initial skeletal pose

The classic object loader now resolves authored ActorDef references before
creating reusable object groups. A direct ActorDef→MeshRef→Mesh remains a
static model. ActorDef→SkeletonRef→Skeleton resolves each track's mesh and
first authored frame, then composes parent × local scale/rotation/translation.
Rigid meshes with no vertex-piece runs inherit the track referencing their
MeshRef. Weighted pieces use their stored vertex-to-track runs. There is no
fallback to bone zero for an unweighted skeletal part.

The same posed source meshes feed the drawable bake and the separate invisible
collision bake. Each actor owns fresh render and collision indices even when
another actor references the same source mesh. Historical raw mesh keys remain
available through independent bakes when they differ from an actor key. When
the keys match, the actor's assembled pose supplies that object. Unplaced
definitions never become terrain or standalone collision.

`Scene::wld_object_sources` retains the actor references, original decoded
meshes, original skeleton names/flags/references/children, all frame values,
track-reference flags and raw optional track speeds. Object extraction retains
its source entry. This
is an initial-pose geometry implementation, not foliage animation: it does not
choose a default speed, infer a loop, or claim native playback timing.

## Evidence and boundaries

- City of Mist's `citymist.s3d/objects.wld` has 50 `JNTREE103_ACTORDEF`
  placements. The definition exists in `citymist_obj.s3d/citymist_obj.wld` and
  links an eight-track skeleton. Its seven meshes contain 106 vertices and
  72 polygons. Six branch tracks have four differing frames with raw speed
  1000. Only the trunk's 42 polygons are collidable.
- The decoded vertices already contain their WLD fragment centers. Adding a
  center during skeletal assembly would apply it twice. The original BR3
  translation is `[0.125, -0.421875, 85.53125]`; the trunk has uniform scale
  0.83984375 and translation `[0, 0.3671875, 0]`. The original regression also
  checks BR1's nonidentity quaternion against independent scalar arithmetic.
- Existing Rust character sampling composes the same parent/local transforms.
  The old C# character composer corroborates hierarchy and quaternion order,
  but its packed translation-divisor decoding is obsolete; the Rust parser's
  fixed /256 translation and separate /256 scale are covered by existing
  packed-versus-floating-frame regressions. The C# zone path had the same
  missing ActorDef assembly as the old Rust object path.
- EQEmu zone-utilities source links rigid MeshRefs to their referencing track,
  independently corroborating ownership. Its legacy Euler/translation-divisor
  calculations are not used as numeric authority. No new native animation
  timeline or runtime capture is claimed by this change.
- An installed object-archive survey found 11,721 direct MeshRef actors and
  667 SkeletonRef actors, all with exactly one actor reference. Some skeletal
  track fields reference unsupported fragment 0x34; those actor variants remain
  explicitly diagnosed rather than partially assembled. Multiple actor
  references, missing track/mesh links, ambiguous unweighted mesh ownership,
  malformed hierarchies/runs and degenerate first poses are also unsupported.
  Failure of one actor preserves unrelated static objects and historical raw
  mesh lookup. References remain local to their source WLD.
- Assembly is bounded to 4,096 tracks and parts, one million retained frames
  and one million source vertices per actor. Hierarchies are traversed
  iteratively after rejecting cycles, invalid child indices and shared parents.

## Verification

The module's synthetic tests cover root/child rotation, translation and scale;
center application exactly once; matching visible/hidden collision poses;
weighted-run bounds; repeated rigid references; independent ownership across
actors and raw lookups; unsupported variants; and malformed hierarchies/poses.

The original City of Mist test resolves all 50 trees, keeps all seven parts and
six animated tracks, verifies exact initial-pose witnesses, and builds 42 trunk
collision triangles per tree (2,100 across the 50 placements). It also checks
object-library loading and source preservation during extraction.

A second original regression preserves all raw lookups and their exact triangle
attributes/materials/collision in global_obj (1), gfaydark_obj (84), and
poknowledge_obj (131), with unique ownership of every baked geometry index.
It additionally resolves the actual Kelethin `FAYLEVATOR` and `FELE2` models.
Plane of Knowledge previously retained 18 later duplicate raw definitions,
totalling 984 triangles, under generic box/cylinder/geosphere keys. Both the
renderer and collision choose the first matching object name; those later
copies had no placements. Their omission reduces the total geometry inventory
by 984 while preserving every first-name model and all instanced collision.
The regression asserts that exact original duplicate count and triangle sum.
These are CPU geometry checks, not visual, traversal or native-motion parity.

Timorous's hidden collision inventory remains exactly 12,438 triangles. The
eight newly resolved actors (`cbbarrel103`, `cbcrate103`, `date101`, `date102`,
`jngrass101`, `jntree103`, `jntree104`, `jntree105`) contribute 620 drawable
definition triangles and 1,996 placed instances, with no added hidden geometry.
Their placed drawable collision adds 67,192 physical triangles: visible
collision is 326,393 and total collision is 338,831. The asset and GPU hidden
collision regressions derive these actor keys from original raw mesh names and
temporarily empty only the actors' owned index buffers. This test-only replay
restores the earlier 227,791 drawable-inventory, 259,201 visible-collision and
271,639 total-collision goldens exactly. Every full-scene barrier motion and
pixel/upload comparison runs after restoring all actor geometry; the hidden
collision A/B delta remains 12,438.

```sh
CARGO_INCREMENTAL=0 EQ_DIR=/path/to/EverQuest cargo test -p openeq-assets --lib loader::wld_objects -- --include-ignored
```

The original City of Mist GPU regression uploads all 50 tree instances and
verifies 72 triangles per instance. An isolated fixed-camera capture shows the
assembled textured trunk and branches; clearing all placements removes every
draw call. Capture: `/tmp/openeq-citymist-objects/assembled-tree.png`. This checks
assembly/upload/visibility, not original-client visual equivalence.

An explicit authored-frame diagnostic now samples retained tracks without
mutating source meshes or baked scenes. Its frame zero is pixel-identical to
the loaded tree; the other three branch poses are visibly different while
the trunk and its collision stay fixed. Production remains at the first pose.
See [native animation research](WLD_OBJECT_ANIMATION.md) for the proven key
timing and loop closure, the quaternion and clock boundaries that still need
proof, exact binary addresses, and the expanded original-asset regressions.

Next, resolve native animated quaternion output, host clock units and placed
controller ownership before integrating runtime motion and collision policy.
Particle-linked actors and other unsupported fragment families remain separate
work; this implementation must not silently turn them into static mesh aliases.
