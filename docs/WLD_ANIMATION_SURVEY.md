# Installed WLD object animation survey

This is the historical checkpoint inventory. Production now animates supported
actors whose collision ancestry is stationary. The later
[five-frame extension](WLD_LONG_OBJECT_ANIMATION.md) additionally admits nine
definitions with 2,424 original metadata placements, using native key reduction
and conservative numeric admission. The snapshot counts below remain unchanged.

The bounded sampler at checkpoint `f35062c` supports **115 actor definitions
across 26 installed zone families**. All use four authored frames at a
1000-millisecond interval, with a four-second closed loop. This is an asset
inventory and CPU sampling survey; production object animation remains disabled.

Support alone is **not a collision gate**. Of those 115 definitions, 29 move
collidable vertices at sampled keys or midpoints. The other 86 split into 63
with collision bound entirely to static tracks and ancestors, and 23 with no
collidable polygons. Future renderer integration can prioritize those 86 after
source-vertex bindings and animated bounds are implemented.

## Scope and reproducibility

On 2026-10-01, the temporary probe inspected all 293 top-level installed
`*_obj*.s3d` archives under `/Users/daeken/EverQuest`, including numbered,
`_2_obj`, global, extra and backup libraries. They contain 293 WLD members and
255 archive-family identifiers; 252 have a corresponding primary zone archive.
`backup_stonebrunt`, `global` and `potranquility_extra` do not. Counts below are
**actor occurrences in archives**, not deduplicated model bytes. Reused names
and zone aliases remain separate occurrences.

Each original archive was exposed by a temporary symlink as `survey_obj.s3d`
in an otherwise empty directory. The probe called the actual public
`load_object_library`, `animation_period()` and `sample_animation()` APIs, so
unrelated global and supplemental archives could not obscure actor ownership.
It separately parsed the raw actor/skeleton references to account for actors
that assembly rejects. Original archives were not modified.

The probe links a frozen copy of the already-built `openeq-assets` library from
the validated sampler checkpoint, avoiding concurrent source edits and Cargo
builds. HEAD at copy was `a9f7b61`, which differs from `f35062c` only in docs.
The copied library SHA256 is
`fe79d8754e9fcc0612d5f4ad45c8332635742309551288d03c54ce782f862f47`.
Temporary source, provenance and complete per-actor results are:

```text
/tmp/openeq-wld-animation-survey/survey.rs
/tmp/openeq-wld-animation-survey/provenance.json
/tmp/openeq-wld-animation-survey/results.json
/tmp/openeq-wld-animation-survey/summary.txt
/tmp/openeq-wld-animation-survey/supported.json
```

The source SHA256 is
`a312f884b6d8ed8ba8fd41c577a786e3e34b51e27386744bb98a3ae2658fbd3f`;
the final full result SHA256 is
`d42b08c69280dec3c4b819b3d239b38cf6ab025ad46ca8e53d18229bd7266745`.
The survey completed without archive, parser, loader or sample errors. All
collidable vertex indices and their skeletal bindings were checked before
classifying collision behavior.

## Coverage

| Inventory | Actor occurrences |
| --- | ---: |
| Raw ActorDefs | 12,388 |
| Direct MeshRef actors, retained as static models | 11,721 |
| Raw SkeletonRef actors | 667 |
| Skeletal actors retained by assembly | 290 |
| Retained skeletal actors with multiple-frame tracks | 288 |
| Supported by the bounded time sampler | 115 |
| Retained animated actors outside the sampler bounds | 173 |
| Retained all-static skeletal actors | 2 |
| Rejected during assembly because of fragment `0x34` links | 376 |
| Rejected during assembly because of unresolved mesh names | 1 |

The supported set contains 37 distinct actor keys. All 115 have placements in
their family’s original `objects.wld`; none of these 26 families is superseded
by an installed `.eqg` primary archive. Placement counts here come from those
classic metadata files, not a server session or a rendered-zone traversal.
They include separate aliases such as `mischiefplane`/`pomischief` and
`eastwastes`/`eastwastesshard`.

## Collision and first integration candidates

For every supported actor, the probe sampled 0, 500, 1000, 1500, 2000, 2500,
3000, 3500 and 4000 milliseconds. It compared each source vertex against time
zero and separately compared only vertices referenced by collidable polygons.
A displacement above `0.0001` object-local units counts as sampled movement.
All supported actors had visible-geometry movement. Displacements are before
placement scale and rotation.

| Supported category | Definitions | Distinct keys | Zone families | Metadata placements |
| --- | ---: | ---: | ---: | ---: |
| Collision bound only to one-frame tracks/ancestors | 63 | 15 | 23 | 38,088 |
| No collidable polygons | 23 | 5 | 10 | 15,852 |
| Collidable vertices move in sampled poses | 29 | 17 | 12 | 11,247 |

For the first category, sampled collision displacement was exactly zero, and
none of the collidable vertex bindings had a multiple-frame ancestor. That
structural check is stronger than testing a few moments alone. It does not
prove a collision policy for other actors, later animation families, particles,
placement motion, or every possible future interpolation scheme.

The static-collision candidates are:

| Actor key | Archive/zone occurrences | Metadata placements |
| --- | ---: | ---: |
| `date101` | 1 | 330 |
| `date102` | 1 | 444 |
| `icetree200` | 5 | 2,290 |
| `icetree203` | 4 | 2,466 |
| `jntree103` | 5 | 9,431 |
| `jntree104` | 7 | 5,416 |
| `jntree105` | 4 | 2,830 |
| `tree101` | 9 | 1,594 |
| `tree102` | 9 | 2,118 |
| `tree104` | 8 | 3,007 |
| `tree106` | 1 | 104 |
| `tree106b` | 1 | 134 |
| `tree107` | 4 | 788 |
| `wltree203` | 2 | 7,001 |
| `wltree205` | 2 | 135 |

The five keys without collision are `cmplant103`, `cmplant104`, `cmplant104b`,
`jngrass101` and `jngrass102`. Treat names as inventory labels, not a universal
allowlist: `tree106`, for example, is supported in Dreadlands but has five-frame
tracks in Growthplane and Mischief. Gate actual retained source data and its
collision bindings.

City of Mist is still the best small integration regression: `jntree103` has
50 placements with 42 static trunk triangles each, and `jntree104` adds two
placements with static collision. Its `cmplant103` and `cmplant104` contribute
37 and 64 placements without collision. **Do not also animate `jntree101` and
`jntree102` under a static-collision assumption**: each has one placement,
68 collidable triangles, and sampled collidable displacement of about 0.612
and 0.786 local units respectively.

The complete supported-zone inventory is:

| Zone family | Static-collision definitions | No-collision definitions | Moving-collision definitions | Static/no-collision placements |
| --- | ---: | ---: | ---: | ---: |
| `burningwood` | 3 | 0 | 0 | 10 |
| `citymist` | 2 | 2 | 2 | 153 |
| `dreadlands` | 5 | 0 | 0 | 549 |
| `eastwastes` | 2 | 0 | 0 | 2,053 |
| `eastwastesshard` | 2 | 0 | 0 | 2,053 |
| `emeraldjungle` | 6 | 4 | 3 | 16,001 |
| `fieldofbone` | 3 | 0 | 0 | 1,303 |
| `firiona` | 3 | 0 | 0 | 2,174 |
| `frontiermtns` | 1 | 0 | 1 | 270 |
| `greatdivide` | 2 | 0 | 0 | 353 |
| `growthplane` | 7 | 5 | 3 | 2,076 |
| `hole` | 0 | 1 | 5 | 3 |
| `lakeofillomen` | 4 | 0 | 0 | 583 |
| `mischiefplane` | 1 | 1 | 1 | 60 |
| `overthere` | 1 | 0 | 1 | 208 |
| `paineel` | 0 | 0 | 5 | 0 |
| `pomischief` | 1 | 1 | 1 | 60 |
| `sebilis` | 0 | 0 | 1 | 0 |
| `stonebrunt` | 2 | 2 | 0 | 527 |
| `swampofnohope` | 3 | 0 | 0 | 313 |
| `timorous` | 5 | 1 | 0 | 986 |
| `trakanon` | 3 | 4 | 3 | 12,103 |
| `velketor` | 1 | 0 | 0 | 13 |
| `wakening` | 2 | 2 | 3 | 11,714 |
| `warslikswood` | 2 | 0 | 0 | 91 |
| `westwastes` | 2 | 0 | 0 | 284 |

Moving-collision definitions also include `cmplant105`, `cmplant105b`,
`frncrock101`, `frnlog101`, `hblais200`, `hconfect200`, `hgaren200`, `hmush200`,
`pasign101/102/103/301/302`, `cbboard102` and `wlcrock200`. Sampled displacement
ranges from about 0.044 to 3.200 local units across that category. Small movement
still requires a deliberate runtime collision policy.

## Rejected frame and timing families

All 173 retained animated actors outside the sampler bounds have more than
four authored frames. They span 87 distinct keys and 64 archive families. All
have packed frame flags 8, animated reference flags 5, and a consistent animated
count/interval within their own skeleton; mixed timelines and float tracks are
not the next limiting factor in this retained corpus.

The API's first reported errors split into 88 track-layout/length rejections
and 85 static-reference rejections. The latter have static track references
with flags 0 instead of the proven flags 4, **and also have long animations**.
Two additional retained skeletal actors are wholly static and have those flags
0; they are not missing animated motion. Looking beyond the first error, seven
of the 173 animated actors change scale and 60 change translation. These counts
overlap; the optimizer contract remains necessary even after extending length.

| Interval (ms) | Animated actor occurrences rejected | Authored frame counts |
| ---: | ---: | --- |
| 33 | 78 | 21, 24, 26, 51, 61, 81, 91, 101, 121, 141, 151, 161, 201, 300, 401, 456, 491, 600, 801 |
| 100 | 28 | 8, 9, 14, 16, 17, 20, 101, 401 |
| 200 | 16 | 14, 20, 40, 400 |
| 250 | 2 | 15 |
| 333 | 26 | 5, 14, 15 |
| 500 | 1 | 14 |
| 1000 | 22 | 5, 7, 10 |

The closest extension is the 20 occurrences with five frames at 1000 ms,
including `tree105`, swamp trees, `purptree200`, some `tree106` definitions and
`electmonu201`. Five authored frames imply six keys after closure, so D3DX's
0.1-lossiness compressor may reduce a key; raising the current limit without
tracing that selection would lose the native-compatibility guarantee. The
14-frame/333-ms pine-tree family is another repeated target. Long machinery,
boats, fish and fluttering objects additionally exercise translation/scale
optimization and static reference flags 0.

## Particle-linked assembly gaps

Fragment `0x34` appears in 733 skeleton track mesh fields across **376 actor
occurrences, 97 actor keys and 119 archive families**. All 376 are rejected by
current actor assembly; they account for all but one of the 377 missing actor
definitions. Of these particle-linked actors, 372 have only one-frame tracks
and four have animated tracks. Extending the pose sampler by itself therefore
will not recover most of this gap.

The fragment names include `L300_PCD`, `L309_PCD`, `CSMOKE_PCD`, `SPOP500_PCD`
and similar particle-cloud references. Examples are torches, lamps, braziers
and fireplaces. There are 13,684 matching placements in the surveyed classic
`objects.wld` metadata, with 367 of the 376 occurrences actually placed.
This is a cross-reference count, not a claim about active server zones or
complete particle behavior.

Plane of Knowledge has eight such rejected actor definitions: `FTORCH301`,
`FTORCH302`, `FTORCH304`, `POKLAMP500`, `POKLAMP501`, `POKLAMP502`,
`POKSCONCE500` and `POKTORCH500`, totaling 366 metadata placements. The largest
individual repeated examples include `COMTORCH303` in Sanctus Seru (422),
`FTORCH301` in Plane of Justice (220), and `POKSCONCE500` in Halls of Honor A
(209). Current fallback raw mesh definitions do not establish correct actor
assembly, emitter ownership or particle playback for these placements.

The remaining assembly gap is `qeynos2_obj.s3d/TEMPLELIFEB_ACTORDEF`:
`BEAM_P2_DAG` and `BEAM_P1_DAG` use unresolved negative mesh references
`-5554` and `-5577`. That needs name/cross-library resolution evidence rather
than the short-track animation sampler.

The subsequent renderer integration preserves the first-pose fallback and adds
source-vertex bindings and animated bounds for the static/no-collision cohort
(see `WLD_OBJECT_ANIMATION.md`).
Particle-linked actors need their own `0x34` parser and native initialization
trace. Longer-key compression, flags-0 semantics and moving-collision behavior
remain separate compatibility work; this survey does not relax those gates.

## Live binding follow-up

After runtime integration, a separate probe reloaded all 293 archives with the
new loader and checked all 86 eligible definition occurrences. Their 8,604
packed vertices map exactly to the source position/normal at time zero. Eleven
sample times (including intermediate poses and loop closure) stay inside each
analytic animation radius. Zero load, binding or bounds failures occurred.
This checks the newly generated CPU-to-GPU binding contract across the corpus;
it does not constitute GPU captures of all placements. Temporary probe and
result: `/tmp/openeq-live-binding-survey/check.rs` and `result.txt`.
