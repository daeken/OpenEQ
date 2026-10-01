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
   skeletons, with a native-time sampler for verified short packed tracks.
   Source-bound runtime playback now covers supported actors whose collision
   ancestry stays static. Static ordinary meshes of particle-linked actors are
   now restored with explicit unsupported emitter metadata. Moving collision,
   longer clips, four animated particle-linked definitions and actual particle
   playback remain unfinished. See `WLD_OBJECT_ANIMATION.md` and
   `WLD_PARTICLE_ACTORS.md`.
3. Follow authored nonfinite mesh attributes through native vertex upload and
   material/channel selection. Preserve the 21 original outliers; do not invent
   normals/UVs or remove geometry without recovering native behavior.
   Exact TER Opaque_MaxCB1 v1–3 UV upload now matches masked-SSE2 SHORT2
   conversion, with source words and diagnostics kept intact. Native packed
   normals/tangents/colors and other shader families remain under investigation.
   See `EQG_NONFINITE_TER_UPLOAD.md` and `EQG_TER_UV_PACKING.md`.
4. Verify shader channels, fog/sky/light metadata and rendering across formats
   with fixed-camera GPU checks and controlled traversal. Material table
   identity and startup scene bounds now have dedicated regression coverage.
   Fixed-camera GPU appearance and timing checks now cover restored Bazaar,
   The Nest and Thundercrest terrain. Opaque repeating diffuse textures now use
   the uploaded mip levels, reducing distant shimmer. Unimplemented
   MaxLava/MaxWaterFall and other additive shader families, normal mapping, alpha-safe/clamped
   filtering, linear/periodic mip construction and Thundercrest's missing
   `clz-0` sky selection remain follow-ups. See `RESTORED_EQG_GPU_AUDIT.md`,
   `EQG_ADDITIVE_SHADER.md` and `SKY_PATTERN_FOLLOWUP.md`. Failed native weather
   selection preserves manager state, but host transition/reset behavior is
   still unresolved. Track ongoing work in `OVERNIGHT_2026-10-01.md`.
   Native adaptive terrain tessellation/normal generation remains separate from
   the supported full-grid topology.
5. Finish native XMI loop/SysEx behavior, original-compatible timbres and audio
   event bindings, then compare timing/attenuation/fades against native output.
   Native four-slot loops and complete-packet SysEx transport are implemented
   and independently replayed; all 389 installed sequences pass admission.
   Streaming retains a 30-minute policy cap. Original timbres, unsupported
   branches/host controls and event bindings remain separate.
   See `XMI_NATIVE_LOOPS.md` and `XMI_NATIVE_SYSEX.md`.
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


## October 1 overnight: opaque filtering and placed-actor diagnostics

Published changes `c0aa4cb` and `ce5044f` retain WLD track reference flags,
add checked immutable authored-frame inspection, and enable derivative-based
sampling for ordinary opaque repeating textures. Masked/blended/clamped paths
retain their prior coverage; water and direct terrain remain specialized.
Citymist's four diagnostic tree poses preserve the static trunk. These are
source-frame diagnostics, not native animated output or enabled playback.
See `WLD_OBJECT_ANIMATION.md` for the recovered native timing, quaternion
conventions and remaining controller/output questions.

The original GPU audit independently reconciles restored terrain batches in
Bazaar, The Nest and Thundercrest and freezes baseline/filtered captures.
The Nest's distant speckling visibly improves. A synthetic subpixel-camera
regression reduces mean brightness change from 115.422 to 0.531–0.562 steps;
nearby details remain resolved. No performance claim follows from the noisy
serialized GPU timings. Full evidence and unsupported shader findings are in
`RESTORED_EQG_GPU_AUDIT.md`.

**969 workspace tests passed, zero failed or ignored** in the complete
original-asset/GPU/silent-audio run, including the earlier collision-substep
regressions. Evidence: `/tmp/openeq-overnight-mips-workspace.log`; UI captures:
`/tmp/openeq-overnight-mips-ui/`. Strict workspace Clippy, client build,
all-target no-default-feature check, formatting and diff checks passed;
logs use `/tmp/openeq-overnight-mips-{clippy,build,no-default}.log`.
Independent review found no actionable issues in the authored-frame or
shader change. No original assets were committed and no live character state
changed. Packed WLD unsigned scale, native controller output and unresolved
sky/shader definitions remain under separate investigation.


Further overnight corrections: packed WLD track scale now uses native unsigned
u16 / 256 (`2769037`). The independent installed-corpus audit covers 27,118,476
packed frames and finds 63 affected frames in three duplicate character tracks;
none are in object archives. All 267 asset tests, workspace lint and client
build pass. See `WLD_PACKED_SCALE.md`; this is numeric parser compatibility,
not a claimed visible character repair.

WLD shared-material merging now preserves source group order (`587f052`).
Previously, hash-map iteration randomized triangle/packed-vertex order across
identical bakes. The new regression fails on the old code; all 10 collision
fixtures, 11 object fixtures, original Citymist GPU rendering and strict lint
pass. See `WLD_PLACED_OBJECTS.md`. Stable source bindings and separate motion
ownership are still required before fixed-topology runtime animation.


## October 1 02:30 checkpoint: native short-track sampler

`f35062c` adds `ObjectSource::animation_period()` and
`sample_animation(Duration)` for verified short packed tracks. Read-only native
host/graphics traces establish milliseconds, Citymist shared controllers and
loop timing. The matching Microsoft D3DX library establishes quaternion
conversion, normalized component interpolation and short-clip compression
bounds. Isolated native math execution checks ten BR1 sample times; this is
not a full running-client comparison. Independent review found no actionable
issue. See `WLD_OBJECT_ANIMATION.md` for provenance and exact support bounds.

All **978 workspace tests passed, zero failed or ignored**, including originals,
GPU and offline/silent audio, at this checkpoint. Strict workspace Clippy,
normal client build, all-target no-default-feature check, formatting and diff
checks pass. Evidence: `/tmp/openeq-native-animation-{workspace,clippy,build,no-default}.log`;
UI captures: `/tmp/openeq-native-animation-ui/`.

Production placed actors still use the initial pose. The sampler accepts an
explicit shared time and preserves source geometry/collision; renderer motion
is not enabled. Next integration needs stable source-vertex bindings through
material baking, ownership-aware deduplication, animated bounds and an explicit
collision policy. A corpus survey is in progress to identify additional
supported actors. Dedicated additive-glass rendering is also in progress from
`EQG_ADDITIVE_SHADER.md`; these new uncommitted changes are not covered by the
978-test checkpoint.

## October 1 live-animation and additive-glass checkpoint

Supported classic placed actors now animate through stable source-vertex
bindings, with a shared scene clock and conservative all-pose instance bounds.
The collision gate checks every physical source vertex, including hidden faces
and animated ancestors. It enables only static-collision or collision-free
actors; Citymist JNTREE101/102 remain static while JNTREE103 animates. Extracted
models, unplaced definitions and objects without submitted triangles do not gain
controllers. See `WLD_OBJECT_ANIMATION.md` and `WLD_ANIMATION_SURVEY.md`.

The independent corpus probe checks 293 archives, 86 eligible definition
occurrences and 8,604 packed vertex bindings, with zero binding/bounds failures.
The corresponding 53,940 metadata placements are survey coverage, not individual
GPU validation. Original Citymist GPU coverage includes its 50 instances,
visible intermediate branch motion, exact four-second image closure, stationary
trunk collision and reflected/nonuniform instance bounds.

The proven TER `AddAlpha_MaxCB1.fx` family now uses its own forward glass pass:
ONE/ONE RGB addition, sampled alpha cutoff at 16/255, read-only depth and no fog.
Thundercrest's original 72 glass triangles are covered by GPU regression; changing
its texture alpha from 102 to 255 leaves the image identical, confirming full
RGB contribution. Four synthetic GPU tests cover the surrounding rendering
contract. Other shader families and native lighting/color-space agreement remain
outside this implementation. GPU profiling includes the new pass in every
consumer. See `EQG_ADDITIVE_SHADER.md`.

The full original-asset/GPU/silent-audio workspace run passed **989 tests, zero
failed or ignored**. Evidence: `/tmp/openeq-placed-additive-workspace-final.log`;
UI captures: `/tmp/openeq-placed-additive-ui/`. An earlier invocation omitted
`EQ_DIR` and stopped at the original-XMI fixture's explicit configuration check;
the corrected complete run passed. The subsequent no-visible-triangle animation
bound guard is covered by a focused rerun recorded separately in
`/tmp/openeq-placed-additive-bounds-final.log`. No asset bytes or captures are
committed, and no live character/server state changed.

The final affected GPU suites, strict workspace Clippy, normal client build,
all-target no-default-feature checks, formatting and diff checks pass. Logs:
`/tmp/openeq-placed-additive-{bounds-final,clippy-final,build,no-default}.log`.
Independent review cleared the animation identity/collision/bounds/clock logic
and the additive render contract, including the final no-triangle guard.


## October 1 03:20 checkpoint: particle-linked static meshes

The loader now retains typed `0x34` particle-cloud records and each attachment's
exact source identity/owning track, while restoring the independently supported
static mesh siblings. The support gate requires the proven record/static-track
family and no physical or visible vertex depending on particle-node ancestry.
Unknown/dynamic cases still fail explicitly; no flame, smoke, particle light or
emitter motion is fabricated.

The 523-zone structural survey remains **501 passes**, the same 21 authored
nonfinite cases and Dranikcatacombsa's banner dependency, with no new invalid
geometry diagnostics. Independent replay reconciles every change in the 246
active WLD zones: +128,093 definition triangles, +3,241,450 collision triangles
and +13,099 resolved placements. Of those placements, eight are an invisible
scaffold, leaving 13,091 newly visible mesh placements. Another 98 previously
resolved raw torch aliases gain their authored vertical offset. All 33,536
nonpartial object digests, terrain and placement transforms are unchanged.

There are 363 partial definitions in 113 active zones; their 13,197 placements
occur in 111 zones. The audit now reports `unsupported_particle_placements`
separately so restored bodies do not conceal the remaining effect gap. The
separate 293-archive inventory recovers 372 actors; nine are superseded by active
EQG zone selection. See `WLD_PARTICLE_SURVEY.md` for exact equations and caveats.

PoK's eight restored actors account for 366 placements, 3,130 definition faces
and 96,934 usable collision triangles. All eight pass original GPU checks,
including instance counts and visible isolated bodies. Captures are under
`/tmp/openeq-pok-particle-bodies/`. Source collision flags and independent
nondegenerate-face counts explain the physical totals; no old collision was
removed. These checks do not certify every restored fixture's traversal.

Malformed signed WLD references now reject missing/empty names instead of
aliasing an unnamed fragment, and minimum signed offsets cannot overflow.
`WLD_REFERENCE_RESOLUTION.md` records the regression. A separate UTF-8/source
byte-offset issue is documented in `WLD_STRING_OFFSETS.md`; all observed original
high bytes occur in trailing padding, with no shifted referenced names found.
String decoding remains unchanged in this checkpoint.

**1,008 workspace tests passed, zero failed or ignored**, with originals, GPU
and offline/silent audio. An earlier full run exposed a test-only temporary
folder name collision between parallel fixtures; adding an atomic serial fixed
it, and the complete corrected run passed. Evidence:
`/tmp/openeq-particle-bodies-workspace-final.log`. Strict workspace Clippy,
normal client build, all-target no-default checks, formatting and diff checks
pass; logs use `/tmp/openeq-particle-bodies-{clippy,build,no-default}.log`.

The final frozen zone audit includes the signed-reference fix and explicit
partial-effect diagnostics. All previous geometry/metadata/count fields match
the independently reconciled candidate. Its SHA-256 is
`5953f14903676433f9fbf85731a15582cd053060232a89e02dd0a006afd25d03`;
reports are `/tmp/openeq-particle-zone-survey-final/`. No original assets were
committed, live character/server state was untouched, and audio stayed silent.


The subsequent WLD source-address repair separates file byte offsets from the
existing UTF-8 display representation. Six focused regressions pass; independent
review and strict assets lint pass. The raw-byte oracle matches all 6,337,095
fragment and 154,518 skeleton names in 1,804 parsed original WLDs, with no
original name change. See `WLD_STRING_OFFSETS.md`; this is a parser correctness
fix, not a claimed visible original-asset repair.


Native TER UV upload now covers the exact `Opaque_MaxCB1.fx` v1–3 family, with
material provenance retained through cloning/remapping. Quantization, signed
wrapping and nonfinite conversion occur only during upload; raw source words
and structural diagnostics remain intact. The24original affected TERs have no
UVs in the SSE2/x87 disagreement set. Independent numeric/native review and
original Causeway/v3/GPU tests pass. See `EQG_TER_UV_PACKING.md`.

The integrated checkpoint passes **1,031 workspace tests, zero failed/ignored**,
including original assets, GPU and silent audio. Strict lint, client build,
all-target no-default and formatting pass. Logs `/tmp/openeq-uv-loops-*`.
This test count includes the separate XMI loop and complete-packet synth API
work; it does not certify original timbre, particle rendering, or other shaders.


The XMI scheduler now executes the original four-slot loop behavior, including
re-entering CC116 itself, while preserving a monotonic clock and active notes.
All four original looping sequences match native trace digests. The scheduler
can complete the full 236-minute finite witness offline; streaming retains the
explicit 30-minute safety cutoff and bounded cancellation/release tail. Original
sequence admission rises to 388/389; The Deep remains gated for SysEx integration.
Independent code review and the 1,031-test integrated checkpoint pass. See
`XMI_NATIVE_LOOPS.md` and `XMI_PLAN.md`; original timbre is still separate.


## October 1 complete-packet XMI checkpoint

The Deep's 22 complete SysEx packets now preserve native byte/tick/source order
through the scheduler and offline synthesizer. Independent original native
MIDI/packet/combined digests match at batch sizes 1/7/512. Synthetic tests cover
notes expiring before packets, mixed source order, loop reentry, rejection,
cancellation and cleanup on synth failure. The full original sequence and
release tail render finite nonzero PCM to memory. See `XMI_NATIVE_SYSEX.md`.

All **389 installed sequences** pass scheduler admission: 385 straight-through
sequences yield 929,693 scheduled events, and four loops retain their independent
native trace checks. This does not certify original instrument timbre, Roland
GS semantic effects, unimplemented branch/host controls or cross-platform audio.
Streaming retains the explicit 30-minute cap; F7 continuations and malformed
or oversized SysEx records remain unsupported.

**1,037 workspace tests passed, zero failed/ignored**, including original assets,
GPU and offline/digitally silent audio. Strict lint, normal client build,
all-target no-default, formatting and diff checks pass. A test-only nested-if
lint correction has its three affected tests rerun successfully. Evidence:
`/tmp/openeq-sysex-workspace.log`, `clippy-final.log`, `build.log`,
`no-default.log`, and `final-tests.log`, all using the same sysex prefix.
Independent code/native review found no blockers. Original assets remain
outside git, and no live character/server state changed.

## October 1 source-channel and particle-texture checkpoint

Particle attachments now retain bounded full authored texture chains, preserving
signed references, duplicate identities, flags, intervals, filenames and opaque
record suffixes. Broken/unsupported effect chains remain explicit issues without
hiding independently validated static meshes or collision. All 623 installed
object-cloud chains resolve as source metadata; this is not native cache emulation
or enabled playback. See `WLD_PARTICLE_TEXTURE_METADATA.md`.

Native terrain research distinguishes original-index baked-light words from
TER-v3 stored colors. The parser now retains ZON-v2 lighting, TER-v3 color/UV1,
and separate EQGP streams without changing render behavior. Causeway's original
LIT and all embedded lighting words in Guild Hall, Guild Lobby and The Nest
match independent raw offsets. Native source selection/remapping, light-weight
shading, normal/tangent precision and GPU parity remain separate work. See
`EQG_TER_VERTEX_CHANNELS.md`.

All **319 assets tests pass**, none failed/ignored, with strict assets all-target
Clippy and independent code/native review. Logs:
`/tmp/openeq-lighting-source-assets-final.log` and
`/tmp/openeq-source-metadata-clippy.log`. This narrower checkpoint follows the
1,037-test workspace run; it does not imply a new complete workspace count.
