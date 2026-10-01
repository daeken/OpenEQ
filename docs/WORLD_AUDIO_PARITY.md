# World and audio parity: active priority

The user reprioritized world geometry/movement and audio on September 30, 2026.
Other gameplay milestones remain in the backlog. Reprioritize only after broad
original-zone evidence supports these two areas; a successful load is not proof
that a zone looks, moves or sounds right.

## Current work

1. Resolve the remaining native embedded region/group transforms and border
   triggers, then verify thin/deflected liquid crossings. Registered
   binary boxes can extend below floors or differ from visible water surfaces;
   Crescent now has an original-floor movement fixture, while Anguish's authored
   basin/box relationship still needs a playable-route check.
   Straight-motion eligibility now checks every accepted collision substep;
   a stair excursion cannot regain eligibility merely by returning to the
   expected endpoint. General deflected crossing integration is still pending.
2. Complete WLD placed-actor behavior: first-pose geometry now resolves supported
   skeletons, but branch animation timing, collision policy and particle-linked
   fragment families still need evidence.
3. Follow authored nonfinite mesh attributes through native vertex upload and
   material/channel selection. Preserve the 21 original outliers; do not invent
   normals/UVs or remove geometry without recovering native behavior.
4. Verify shader channels, fog/sky/light metadata and rendering across formats
   with fixed-camera GPU checks and controlled traversal. Material table
   identity and startup scene bounds now have dedicated regression coverage.
   The restored Bazaar, The Nest and Thundercrest terrain substantially raises
   submitted geometry; add fixed-camera appearance and GPU timing checks there.
   The first overnight captures identify ordinary-texture mip-zero aliasing,
   unimplemented MaxLava/MaxWaterFall/additive shader handling and Thundercrest's
   missing `clz-0` sky selection as concrete follow-ups. Track ongoing work in
   `OVERNIGHT_2026-10-01.md`.
   Native adaptive terrain tessellation/normal generation remains separate from
   the supported full-grid topology.
5. Finish native XMI loop/SysEx behavior, original-compatible timbres and audio
   event bindings, then compare timing/attenuation/fades against native output.
   Automated original-audio checks remain offline or digital silence.

The October 1 checkpoint below records the completed native-volume and placed
object work. Gameplay milestones remain behind these world/audio priorities.

## Survey tooling

`zone_audit` discovers WLD zones by their matching world BSP and EQG candidates
by archived/loose ZON declarations. It selects active metadata using the same
precedence as the live loader. EQG swimming/border limitations are explicit;
an empty supported set never means a zone is proven dry or has no exits.

```sh
cargo build -p openeq --bin zone_audit
python3 tools/audit_zones.py --dir /path/to/EverQuest --output /tmp/new-zone-survey
```

Each zone runs in its own process with a timeout. The report separates geometry
structure from all untested texture, GPU, traversal, NPC and acoustic behavior.
Use `--metadata-only` for a faster format/region survey or `--zones gfaydark
poknowledge feerrott2` for a focused rerun. Original assets are never exported.

Installed discovery cannot find wholly absent zone files. For server coverage,
use `--zones-file /path/to/server-zone-names.txt` with one expected short name per
line (blank lines and `#` comments are allowed). The manifest records the exact
normalized selection; the summary reports missing primary archives separately
from corrupt/unsupported content. Database zone names can include unused or
unreleased entries, so this inventory is not a list of confirmed travel routes.

## September 30: first broad evidence checkpoint

Implemented in this pass:

- Terrain quad flag 0x80 selects the authored alternate diagonal. Render indices,
  height sampling, collision, prop/light anchors and height-driven painting now
  agree. The original Feerrott seam witness changes by 4.220995 units. Both direct
  and baked GPU paths match independent triangle references. This establishes
  cached full-grid topology, not native adaptive tessellation/LOD or normals.
- Classic EFF byte57 is the independent night kind. Native all-day collapse and
  raw-hour day selection are implemented; music selector 0 selects ordinal 0.
- All 79 installed XMI files parse (389 sequences/548,617 events). Native note-slot
  scheduling supports 384 sequences (927,156 scheduled outputs,31 peak active notes).
  Four sequences need loop controls; The Deep ordinal 0 needs SysEx. Unsupported
  controls fail preflight rather than being forwarded as different MIDI commands.
- macOS classic music uses the installed offline DLSSynth and bounded PCM queues.
  File+ordinal identity prevents two songs in one XMI from becoming one track.
  Original GFay ordinals 0/2/5 produce finite, nonzero samples silently. Native
  note scheduling is verified; the OS default bank does not prove original
  instrument/timbre fidelity. Other platforms still need a synthesis backend.
- Thirteen renamed binary dungeon declarations load their archived meshes,
  placements and collision without renaming user assets. A fourteenth,
  Dranikcatacombsa, has two actual banner-reference/filename mismatches. The
  error names the internal declaration and missing mesh; no fuzzy name repair.
- Housegarden now loads after retaining native tag 1 material-property words
  distinctly. Its 101 models and 6,267 placements load with collision. UV-channel
  binding and repeated material-ID interpretation remain separate unresolved
  work; see `EQG_MATERIAL_PROPERTIES.md`.

The first completed structural sweep examined 3,246 archives and discovered 523 zone
candidates. It retained three archive-directory inconsistencies in character
archives. The run is a CPU structure/metadata survey, not a visual certificate.
Local reports: `/tmp/openeq-world-audio-zone-survey-v2/`; initial interrupted
run is preserved separately. The driver records the executable SHA256 in new
manifests so subsequent runs can be distinguished from this baseline.

September 30 discovery order (status superseded by the October 1 checkpoint):

1. Complete source-backed resolution of grouped heightmap regions and binary
   EQG liquid/border volumes. The survey found 56 heightmap declarations, not just
   the six previously sampled. 55 currently report unsupported embedded regions;
   only one has the supported top-level subset. 220 binary EQG declarations report
   unsupported volumes. Empty supported sets must never be called proven dry.
2. Resolve placed WLD actor definitions. 98,661 placements across the survey have
   names absent from the current mesh-name lookup; many resemble trees/grass,
   while others are known EQG terrain markers. This count is diagnostic, not a
   count of proven missing visible objects. City of Mist has 50 unresolved
   `jntree103` placements whose actor definition references an 8-track animated
   skeleton; the loader currently registers only the component meshes.
   The placements are in `citymist.s3d/objects.wld`; their `JNTREE103_ACTORDEF`
   is present in `citymist_obj.s3d/citymist_obj.wld`. Its six branch meshes and
   trunk total 106 vertices/72 polygons; only the 42 trunk polygons collide.
   All parts have empty vertex-piece tables and must inherit their referencing
   track, not the character path's default root bone. Six branch tracks contain
   four differing frames at speed 1000, with nonidentity pose transforms.
   Next implement a bounded ActorDef→Skeleton→track/mesh object builder with
   actor-owned render/collision groups, then verify the sampled pose and trunk
   collision on this fixture before animation integration. Preserve those
   tracks; name aliases alone cannot recover the object, shared mesh ownership
   can be overwritten, and decoded vertex centers must not be added twice.
3. Investigate 21 zones with nonfinite vertex attributes. The refined audit
   distinguishes position, normal and UV faults. Bloodfields/The Void/Direwind/
   Valdeholm samples have nonfinite UVs; ArxMentis also has nonfinite positions
   and normals. Trace source mesh version/indices before inventing replacements.
4. Verify textures/shaders, fog/sky/light metadata and fixed-camera GPU captures
   across each format and the discovered failures. Successful CPU load is only
   the starting point; controlled traversal must cover ledges, seams, ramps,
   lifts, thin/deflected liquid crossings and zone transitions.
5. Verify material table identity: Housegarden terrain contains 33 records but
   only 23 distinct stored IDs, so the current keyed map loses records. Trace
   polygon/material references before replacing the map. Also normalize the
   raw 1–24 clock at sky-selection call sites; audio deliberately keeps raw hours.
6. Finish native XMI loop/SysEx behavior, original-compatible timbres and audio
   event bindings, then compare timing/attenuation/fades against native output.
   All automated audio checks remain offline or digital silence.

Remaining foundations include native adaptive terrain tessellation/normal
construction, grouped placement/region transforms and movement environmental
rules. Gameplay roadmap work stays behind these world/audio priorities.

## September 30 structural survey and verification record

The final survey is `/tmp/openeq-world-audio-zone-survey-final/`, using a frozen
executable with SHA256 `36f04c17773e00d23bfc9fb84ea60beb269f66ab552f6ba59078e2696bf9201d`. All 523 candidates completed, with no timeout.
501 passed CPU structure checks; 21 retained nonfinite mesh-attribute diagnostics;
Dranikcatacombsa retained its explicit authored banner-reference failure.
Housegarden now passes loading/structure. None of these counts certify textures,
GPU appearance, NPCs, traversal or audio fidelity.

The 21 mesh outliers are: arxmentis, bloodfields, breedinggrounds, causeway, chapterhouse, direwind, dranik, draniksscar, kattacastrum, kattacastrumb, thevoida, thevoidb, thevoidc, thevoidd, thevoide, thevoidf, thevoidg, thevoidh, valdeholm, wallofslaughter, zhisza.

Indexed-water tests retain their former collision counts through a test-only
replay of original DAT data with only 0x80 diagonal flags cleared. Removing all
water surfaces changes no physical triangles. Restoring legacy terrain triangles
alone also changes no counts. Corrected prop heights explain every remaining
f32 near-collinear filtering change: Feerrott −1, BuriedSea +4, Oldcommons +2. This
preserves the original regression evidence instead of replacing its goldens.

Original assets remained local. No live character or server state changed. All
original audio verification used in-memory/offline synthesis; the hardware
initialization test produced only digital silence.

Final verification: **920 workspace tests passed**, zero failures and zero
ignored tests, including original assets, GPU captures and silent audio.
Strict workspace Clippy, formatting, normal client/audit builds and all-target
`--no-default-features` checks passed. Evidence logs are
`/tmp/openeq-world-audio-{workspace,clippy,build,no-default}-final.log`;
terrain captures are `/tmp/openeq-terrain-topology/`. The broad test run used
`EQ_DIR` and `EQ_CLIENT_DIR` pointing at the local installation and
`OPENEQ_UI_CAPTURE_DIR=/tmp/openeq-world-audio-checkpoint-ui`.

## September 30: classic Freeport missing-assets report

PoK travel selected classic West Freeport (`freportw`, zone 9), but the installed
directory has neither `freportw.s3d` nor `freportw_obj.s3d`. Its character archive
alone is insufficient. `freeportwest.eqg` is the redesigned zone 383, with different
geometry and server coordinates; it must not substitute for classic Freeport.
East Freeport's classic terrain/props archives are also absent. Restoring the
matching original archives from an older installation remains necessary; this
checkpoint does not claim that classic Freeport is playable.

The loader now requires the primary zone archive and its matching terrain WLD,
allows classic zones without optional prop archives, and reports missing assets
with their expected names and installation directory. Props-only archives and
matching WLDs hidden in supplemental archives cannot masquerade as terrain.
Primary archive selection handles filename case consistently in loading and
metadata checks. Three synthetic regressions cover these cases, including a
primary-only zone with verified physical ground and a classic/modern Freeport
name collision.

A read-only server database inventory supplied 482 distinct expected zone names
to the new `--zones-file` survey. 458 metadata reads passed; 23 have no matching
primary archive; Dranikcatacombsa retains its known internal banner mismatch.
No survey timed out. The 23 names include unused/test entries; no reachability
claim is implied. They are: apprentice, arttest, aviak, barter, befallenb, commons,
cshome, ecommons, erudsxing2, freeporttemple, freporte, freportw, highpass,
highpasskeep, kithforest, misty, nektropos, oasis, oldhighpass, oot, qvicb, sro, tox.
Reports: `/tmp/openeq-freeport-server-zone-survey/`. A separate geometry/collision
survey passed freportn, freeportwest, gfaydark and poknowledge, and correctly
reported freporte/freportw missing: `/tmp/openeq-freeport-focused-survey/`.

Verification: all 228 asset tests passed, including original fixtures and all
three new regressions, with no ignored tests. Original PoK/GFay/Chardok/Abysmal
GPU uploads passed. The zone-list driver passed checks for comments, blank lines,
case normalization, deduplication, missing-assets reporting and invalid/empty-list
rejection. No character state or client assets were changed.
Strict workspace Clippy, formatting and the normal client/audit builds passed.
Logs: `/tmp/openeq-freeport-{assets-tests,gpu,clippy,client-build,build}.log`.

## October 1: native regions and classic placed actors

- Recovered binary EQGZ v1/v2 reader-to-active-factory and query paths. Native
  centers and signed half-extents pass unchanged; raw Z/Y/X rotations use
  truncated 512-unit turns. The original -1.5707964 value becomes -1 unit,
  not -90 degrees. Runtime point/swept queries retain source-order dry/unknown
  winners and APV exclusion. AFG and malformed/unsupported records reject the
  complete set. See `EQGZ_NATIVE_REGIONS.md`.
- Heightmap top-level regions are enabled when every referenced TOG has a
  complete validated region-free definition. Missing files, embedded areas,
  unknown grammar and unsupported transforms still reject the complete set.
  Renderer and liquid loader share archive-first group resolution. This does
  not recover parent transforms for embedded areas. See `EQG_LIQUID_TRANSFORMS.md`.
- Classic ActorDef→MeshRef/SkeletonRef assembly now bakes actor-owned initial
  poses for rendering and collision. City of Mist resolves all 50 trees with
  seven parts, 72 visible triangles and 42 trunk collision triangles each.
  Timorous resolves 1,996 previously missing placements. The full survey resolves
  85,028 formerly unmatched placements across 65 zones. Source tracks, frames,
  speed fields and hierarchy are preserved; playback remains future work.
  Original global/GFay/PoK raw keys and lift models stay compatible. See
  `WLD_PLACED_OBJECTS.md`.
- A bit-level audit of 4,572 original TER/MOD payloads established that all 21
  mesh-attribute outliers contain authored NaNs. There are 192 source faces
  with bad positions and 404 finite-area faces with bad normal/UV values. No
  production sanitization or face removal was introduced. See
  `EQG_NONFINITE_ATTRIBUTES.md` for the native upload lead and evidence limits.

Review caught the native AFG constructor's special horizontal expansion: it
uses max(X,Y) for both horizontal half-extents. A dry AFG handled as an ordinary
box can incorrectly reveal later water. Both binary and DAT now conservatively
reject these whole sets. Arelis's sole installed DAT example is square, so no
original Arelis mismatch was demonstrated; it remains outside this constructor
subset.

Additional discoveries: GPU scene bounds currently use raw local definition
vertices, including unplaced definitions, rather than all instance transforms.
This predates actor assembly and needs a separate rendered-bounds regression.
Native TER registration and environment queries preserve XYZ; Crescent's
terrain placement record is deliberately skipped by the original loader. The
initial depth probes were not a reason to add a coordinate transform. Anguish
has a lower floor 105 units beneath its box center (beyond the initial 100-unit
search), other floors above it, and a visible water plane at a different height.
Crescent's river box center is below ground, while a nearby upper portion lies
above the river floor; a new offline movement fixture checks that shallow slice.
This does not establish Anguish reachability or native/server surface agreement.

Final CPU survey: `/tmp/openeq-native-regions-survey-final/`, executable SHA256
`b9389850a3cd45132efcd9922e43c8007878751a9fa8042afb009756765ba51c`.
All 523 candidates completed without timeout: 501 passed structure checks, the
same 21 authored nonfinite-attribute zones retained their diagnostics, and
Dranikcatacombsa retained its authored banner-reference failure. No new structural
failure appeared. Unresolved placement references decreased from 98,662 to
13,634 across 65 changed zones; this includes marker/unsupported references and
is not an exact missing-visible-object count.

Liquid coverage: 177 classic WLD, 92 binary EQGZ and 27 heightmap supported wet
sets. Another 204 sets have no supported liquid; 21 heightmaps and one binary
zone retain explicit unsupported diagnostics. The one metadata failure is the
known Dranikcatacombsa dependency. The binary exclusion is Pohealth's AFG;
heightmap exclusions include Arelis AFG, real embedded areas, missing groups,
unsupported group grammar and anchor/version limits. This is metadata/CPU
coverage, not certification of every zone's rendered or playable environment.

Movement tests cover original Maiden's Grave collision and Crescent's shallow
river slice at 10/30/120 FPS, plus Anguish's finite box in an explicitly isolated
collision world. Original City of Mist GPU captures verify assembled tree parts
and all 50 instance draws. Timorous's old triangle goldens are independently
replayed with only new actor-owned buffers disabled, then all runtime barrier
and GPU comparisons run with those actors restored. The additions contribute
67,192 physical triangles; hidden collision remains 12,438. GFay, PoK, North
Freeport and Qeynos retain their prior physical triangle counts.

All **947 current workspace tests are verified passing**, including original
assets, GPU checks and silent audio. The full run executed 946 tests; five early
screenshot saves failed because the newly named capture directory did not yet
exist. The complete affected 463-test client library passed on rerun after the
directory existed. The movement suite then passed all four tests, including the
one added Crescent floor fixture. No test was left ignored or failing. Evidence:
`/tmp/openeq-native-regions-workspace.log`,
`/tmp/openeq-native-regions-ui-rerun.log`, and
`/tmp/openeq-native-regions-movement-final.log`.
Strict workspace Clippy, normal client/audit builds, all-target no-default-feature
checks and formatting passed. Logs use `/tmp/openeq-native-regions-*.log`;
UI captures are `/tmp/openeq-native-regions-ui/`. Create the capture directory
before setting `OPENEQ_UI_CAPTURE_DIR`; several existing screenshot tests expect
it to exist. No original assets were committed, no live character/server state
changed, and original audio verification remained offline/digital silence.

## October 1 follow-up: material identity, AFG and rendered bounds

- TER/MOD polygon material references are source-table ordinals, followed by
  first exact-name canonicalization. Stored IDs are retained as metadata rather
  than used as map keys. This restores Housegarden's 177 omitted triangles and
  gives 2,210 stonewall triangles their authored stone material rather than a
  later branch material. Anguish also resolves the native first-name normal-map
  binding for 6,403 triangles sharing a duplicate material name. All source
  records and their property words survive.
  Draw batches, equipped models and collision water classification use the same
  lookup; explicit material-free and invalid-reference policies remain distinct.
  See `EQG_MATERIAL_IDENTITY.md` and `EQG_MATERIAL_PROPERTIES.md`.
- AFG constructor support now uses the larger signed horizontal half-extent for
  both horizontal axes before ordinary rotation. Original Arelis's water set
  works; Pohealth's 19 records parse as a set with no supported liquid. Point and
  swept queries preserve AFG dry precedence. This implements region geometry,
  not the native fog transition behavior. See `EQG_AFG_REGIONS.md`.
- GPU startup bounds now follow submitted finite indexed geometry and its actual
  instance transforms. Unused models, unused vertices and physical-only geometry
  no longer distort the offline camera center or extracted model dimensions.
  Bounds conservatively transform each mesh box and do not refresh on dynamic
  pose updates. Original tree placements, reflected/nonuniform transforms and
  unchanged hidden-collision rendering have regressions. See `SCENE_BOUNDS.md`.
- Embedded TOG area research recovered a candidate transform and original
  fixture but could not establish that the native runtime calls it. The current
  area-bearing-group rejection remains; native placement setters update only
  object lists in the traced path. See `EQG_GROUP_REGIONS.md`.

An original Housegarden GPU fixture uploads the entire corrected zone, then
renders the first four stonewall source polygons with a fixed camera. Replaying
the former stored-ID lookup produces transparent branch speckles where the
corrected frame shows solid stone. Captures:
`/tmp/openeq-housegarden-materials/{corrected,former-id-lookup}.png`.
These verify the material identity change, not full original shader/UV fidelity.

The frozen full CPU survey again covers **523 zones**: 501 pass structural
checks; the same 21 authored nonfinite outliers and Dranikcatacombsa's known
banner dependency remain. No new failure, invalid mesh/instance/light/reference,
unresolved-object, or mesh-problem diagnostic appears. Geometry reports change
in 102 EQG zones (89 binary, 13 heightmap), with 93 zones gaining a total of
**1,819,692 drawable mesh-definition triangles**. These counts include reusable
models once, not once per placement. The three largest recovered terrain
examples have independently verified source counts: Bazaar +142,594, The Nest
+250,988, Thundercrest +270,445. Their additions are direct TER geometry;
hidden material sentinels remain excluded.

An independent raw-record audit also reconciles every draw-count delta across
all **276 EQG zones** (220 binary plus 56 heightmap). Binary declarations match
both absolute old/new triangle totals; heightmap checks use the actually loaded
MOD definitions and match their whole-scene triangle deltas. Every one of the
1,819,692 additions is accounted for by a formerly absent source-ordinal group;
there are no removed drawable groups. Reports:
`/tmp/openeq-material-source-delta.json` and
`/tmp/openeq-material-source-delta.log`. This comparison checks material-linked
geometry counts, not all pixels, collision outcomes or native shader behavior.

The two initial indexed-water test failures were frozen whole-scene counts,
not changes to water. Exact source-face/instance replay proves Feerrott2's
+936 drawable/+2,172 physical fence triangles, Buried Sea's +8/+32 mast
triangles and Arelis's +16/+16 building triangles. Removing only those recovered
faces recreates every previous total. Water surface vertices, indices, selector
bindings and terrain-anchor rounding deltas remain unchanged. See
`EQG_MATERIAL_SURVEY.md` for the source witnesses and original regressions.

Liquid metadata now reports 177 supported WLD wet sets, 92 binary EQGZ wet sets,
28 heightmap wet sets, 205 sets with no supported liquid, 20 explicitly
unsupported heightmap sets and the known one metadata failure. There are no
remaining unsupported binary-region sets in this installed survey. This does
not certify all native region side effects, embedded areas, traversal, textures
or GPU appearance.

Survey: `/tmp/openeq-material-bounds-survey-final/`.
Frozen audit binary SHA-256:
`6709ff97c3f23f9844e6c4d99790a207eedef324ad616ee25dbad8bcd41d6cc4`.

All **961 current workspace tests are verified passing**, including original
assets, GPU checks and silent audio. The full run executed 960 tests: 958 passed
and the two indexed-water tests stopped at their old whole-scene counts. After
source/instance reconciliation, both complete affected suites passed all eight
tests, including the newly added regression. No test remains ignored or failing.
Evidence: `/tmp/openeq-material-bounds-workspace.log` and
`/tmp/openeq-material-bounds-reconciled-tests.log`.

Strict workspace Clippy passed again after the final test addition; normal
client/audit builds, all-target no-default-feature checks, formatting and diff
checks also passed. Logs use `/tmp/openeq-material-bounds-*.log`; UI captures
are `/tmp/openeq-material-bounds-ui/`. The original assets remain outside the
repository, no live character/server state changed, and original audio checks
remained offline/digital silence.
