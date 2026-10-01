# Partial WLD particle-actor restoration: corpus reconciliation

Audit date: 2026-10-01. The particle-actor change accounts exactly for the
full-zone survey's **128,093 additional definition triangles, 3,241,450
additional collision triangles and 13,099 newly resolved placements**.
All changes occur in 113 classic WLD zones. No unexplained geometry, terrain,
placement-transform or collision-count differences remained in the comparison.

These are restored ordinary mesh portions of particle-linked actors. Particle
playback remains unsupported and is explicitly retained in source metadata and
diagnostics. Native ownership and record evidence are documented in
[WLD_PARTICLE_ACTORS.md](WLD_PARTICLE_ACTORS.md).

## Inventory versus active zone selection

The isolated object-archive survey opened all 293 installed `*_obj*.s3d`
archives and loaded each through `load_object_library` in a temporary directory
containing only a symlink to that original archive. All 12,388 ActorDefs were
inspected; 12,383 assembled successfully, without parser or library-load errors.

Exactly 372 previously rejected actors now retain their static ordinary meshes
and unsupported particle metadata. They span 116 archive families and 94 actor
keys, with 517 source parts, 174,652 source vertices, 131,585 source polygons
and 131,529 collidable source polygons. Their corresponding classic metadata
contains 13,385 placements. These are archive occurrences, not deduplicated
models or a count of currently selected runtime assets.

The remaining five rejected definitions are unchanged in scope:

| Archive | Actor | Remaining limitation |
| --- | --- | --- |
| `codecay_obj.s3d` | `CDTORCH502` | Multiple-frame skeleton |
| `codecayb_obj.s3d` | `CDTORCH502` | Multiple-frame skeleton |
| `poeartha_obj.s3d` | `POEDMND500` | Multiple-frame skeleton |
| `postorms_obj.s3d` | `POSTRMTRNDO500` | Multiple-frame skeleton |
| `qeynos2_obj.s3d` | `TEMPLELIFEB` | Unresolved named mesh references |

The full installed-zone audit attempts 523 zones. It selects WLD terrain for
246 of them; the others select 220 EQGZ zones, 56 heightmap zones, and one zone
whose metadata load fails. Its result remains 501 structural passes, the same
21 zones with nonfinite mesh diagnostics, and the same Dranikcatacombsa banner dependency. There are no new invalid-geometry diagnostics.

Nine of the 372 restored archive definitions are not used by the active WLD
zone selection:

| Archive family | Restored definitions excluded from active WLD selection | Classic metadata placements excluded | Active format |
| --- | ---: | ---: | --- |
| `powar` | 7 | 0 | EQGZ |
| `tutoriala` | 1 (`YKFLAME400`) | 96 | EQGZ |
| `tutorialb` | 1 (`YKFLAME400`) | 92 | EQGZ |

Thus the active WLD subset has **363 partial actor definitions in 113 zones**
and **13,197 matching placements**. No additional partial actors appeared from
unaccounted archive selection or cross-library fallback.

## Independent old/new comparison

A temporary Rust probe was compiled twice against pinned asset libraries:
the pre-restoration sampler library and the candidate containing the bounded
particle parser and actor gate. Each binary independently called `load_zone`
for all 246 actively selected WLD zones. The resulting triangle, collision,
instance and unresolved-placement counts matched the corresponding earlier
and candidate full-zone audit rows exactly.

For each object, the probe retained its actor/source identity, definition
triangle count, placement count and a canonical geometry digest. Digests
include each ordered triangle's full eight-component vertex attributes,
material flags, texture names, normal-map reference, animation interval,
mask/blend/emissive state, water classification and collision eligibility.
Hidden collision triangles are also included. Triangle digests are sorted
before aggregation, so changes in buffer batching alone do not appear as
geometry changes. They are deterministic 64-bit comparison digests, not
cryptographic identity claims.

The comparison also checks unowned terrain/hidden-collision digests and sorted
placement records, including every position, scale and quaternion bit pattern.
For each partial actor, a separate scene containing only its ordinary geometry
and its actual zone placements measures its collision contribution. This uses
the collision builder's existing material and degenerate-face rules, rather
than multiplying source polygon counts and assuming every face survives.

The reconciler requires, for every zone:

```text
definition delta = sum(candidate partial actor triangles)
                 - sum(old same-key object triangles)

collision delta  = sum(candidate partial actors' placed collision triangles)
                 - sum(old same-key objects' placed collision triangles)

newly resolved placements = placements of partial actors with no old same-key object
```

Every equation matched. All **33,536 nonpartial object occurrences** retained
their geometry/material digests and counts. All 246 terrain digests and all
246 placement-record digests remained unchanged. No old object keys disappeared.
The remaining 277 full-zone audit rows had no reported geometry changes.

## Existing raw-mesh aliases

Three restored definitions already had a same-key raw mesh fallback:

| Zone | Actor | Placements already resolved | Old and new definition triangles | Old and new placed collision triangles |
| --- | --- | ---: | ---: | ---: |
| `acrylia` | `ACTORCH302` | 42 | 178 | 7,476 |
| `mirh` | `ACTORCH302` | 13 | 178 | 2,314 |
| `pojustice` | `ACTORCH302` | 43 | 178 | 7,654 |
| **Total** | | **98** | **534** | **17,444** |

Those 98 placements were not previously unresolved, so they must not be counted
as newly resolved by this change. Their geometry digests do change: all 235
vertices of each `ACTORCH302_DMSPRITEDEF` bind to track 1, whose authored
translation is `(0,0,626/256)`, or **+2.4453125 Z**. The root transform is the
identity and the source mesh center is zero. Actor assembly applies that offset;
the old raw lookup did not. The triangle counts remain identical while the
authored pose and ownership are restored.

The placement reconciliation is therefore:

```text
13,385 isolated-archive metadata placements
 - 188 tutorial placements whose active zones select EQGZ
=13,197 active WLD placements using partial actors
 -  98 placements already resolved through ACTORCH302 raw aliases
=13,099 newly resolved placement keys
```

There are 360 newly introduced object keys plus the three corrected raw aliases.

## Definition and collision triangle accounting

The 363 active partial definitions contain 508 parts, 170,266 source vertices
and 128,639 source polygons. Twelve polygons are intentionally invisible and
non-collidable: `potorment_obj.wld` mesh 558, `POTWF501_DMSPRITEDEF`, belonging
to `POTORMWATFAL501`. Material list 557 points to material 556, whose render
method is zero. The regular bake emits neither drawable nor collision geometry
for those faces.

That actor has eight metadata placements. Its retained source/particle ownership
is newly resolved, but it has **no restored visible body or collider**. The
13,099 newly resolved placement keys must not be described as 13,099 newly
visible fixtures: eight are this invisible scaffold. No waterfall particles
were fabricated.

Definition triangle accounting:

```text
128,639 source polygons in active partial definitions
 -   12 invisible, non-collidable scaffold polygons
=128,627 drawable partial-definition triangles
 -  534 triangles already supplied by the three raw aliases
=128,093 additional definition triangles
```

Collision accounting uses actual placements and collision filtering:

```text
3,258,894 collision triangles from all active partial actors at their placements
 - 17,444 collision triangles already supplied by the three raw aliases
=3,241,450 additional collision triangles
```

The source collidable-polygon count multiplied by those placements is
3,276,726, which is not the usable collision result. Collision construction
filters 17,832 of those source faces. Multiplying raw polygon counts would
overstate the effect of the change.

Two restored definitions, `COMTORCH304` in `blackburrow` and `qrg`, have no
placements. Each adds 160 definition triangles and zero world collision or
resolved placements. This explains why a changed library inventory does not
necessarily imply changed visible world geometry.

### Plane of Knowledge witness

The eight PoK definitions retain 39 source parts, 4,993 source vertices and
3,130 collidable source polygons. All 366 placements were previously unresolved.
All 3,130 definition polygons remain in the drawable geometry.

| Actor | Placements | Definition triangles | Usable collision triangles per model | Placed collision triangles |
| --- | ---: | ---: | ---: | ---: |
| `FTORCH301` | 60 | 168 | 168 | 10,080 |
| `FTORCH302` | 13 | 128 | 128 | 1,664 |
| `FTORCH304` | 17 | 160 | 160 | 2,720 |
| `POKLAMP500` | 16 | 482 | 458 | 7,328 |
| `POKLAMP501` | 13 | 1,000 | 1,000 | 13,000 |
| `POKLAMP502` | 30 | 918 | 894 | 26,820 |
| `POKSCONCE500` | 49 | 90 | 90 | 4,410 |
| `POKTORCH500` | 168 | 184 | 184 | 30,912 |
| **Total** | **366** | **3,130** | **3,082** | **96,934** |

Both affected lamp models have 24 source faces excluded by the collision
builder's degenerate-face filter. Their placements account for the difference
between 98,038 placed collidable source polygons and 96,934 usable collision
triangles: `24 × (16 + 30) = 1,104`. Existing PoK collision geometry is unchanged.

## Reproduction and limits

Temporary evidence artifacts:

```text
/tmp/openeq-wld-particle-survey/survey.rs
/tmp/openeq-wld-particle-survey/results.json
/tmp/openeq-material-bounds-survey-final/zones.jsonl
/tmp/openeq-particle-zone-survey/zones.jsonl
/tmp/openeq-particle-zone-survey/comparison.json
/tmp/openeq-particle-reconciliation/reconcile.rs
/tmp/openeq-particle-reconciliation/compare.py
/tmp/openeq-particle-reconciliation/old.json
/tmp/openeq-particle-reconciliation/candidate.json
/tmp/openeq-particle-reconciliation/reconciliation.json
```

The old pinned asset library SHA-256 is
`fe79d8754e9fcc0612d5f4ad45c8332635742309551288d03c54ce782f862f47`.
The candidate library SHA-256 is
`4da1a7442747cc167d392c48e1d7f8e5f926111231c386cabca9a167d5f2203d`.
The candidate contains positive-only particle attachment and texture metadata
gates; it predates the separate generic signed-reference resolver follow-up.

The reconciliation probe source SHA-256 is
`398e5442810163923e3027e6965959586387e9c6a6145417099198b314f60d8f`;
the comparison source SHA-256 is
`877903fa06455a5541ee36265ab5697b93d1fd6580d24b26eb1a864ad4a31495`.
The final reconciliation JSON SHA-256 is
`1a5cb577d1806e49056c7246b6a9396ccb779ae818a168303d0bcb7e91925632`.
It retains all 246 per-zone equations and all 363 per-actor contributions.

The full asset suite at this particle checkpoint passed 293 tests, including
the original assets, with zero failures or ignored tests; assets all-target
Clippy passed with warnings denied. Focused regressions cover lossless particle
records, fragment truncation, positive full-width reference identity, duplicate
definitions, retained owning tracks, rigid/weighted and hidden-collision ancestry
rejection, unsupported animation/layout rejection, PoK source geometry and the
historical raw-lookup inventory.

This reconciliation is CPU structure, ownership and collision inventory
verification. It does not certify native particle playback, dynamic light,
traversability around every restored fixture, GPU appearance in all 113 zones,
or complete actor fidelity. Original assets were neither modified nor added to
the repository. No production changes or golden-count updates were made for
this reconciliation.


The final root audit includes the separate signed-reference fix and an explicit
`unsupported_particle_placements` map. All earlier geometry/metadata/count
fields match the reconciled candidate exactly. The new map reports 13,197
placements in 111 zones; two of the 113 zones with changed definitions do not
place their partial models. PoK lists the same eight names and 366 placements.
Final reports: `/tmp/openeq-particle-zone-survey-final/`, including
`final-comparison.json`. The complete workspace/GPU verification is recorded in
`WORLD_AUDIO_PARITY.md`; all eight PoK model bodies have isolated GPU captures.
