# World and audio parity: active priority

The user reprioritized world geometry/movement and audio on September 30, 2026.
Other gameplay milestones remain in the backlog. Reprioritize only after broad
original-zone evidence supports these two areas; a successful load is not proof
that a zone looks, moves or sounds right.

## Current work

1. Survey installed zone declarations and independently report load/geometry,
   collision structure, supported liquid subset and border support. Preserve
   failures, absent formats and timeouts in the results. Add reproducible paths
   and fixed-camera checks for each discovered problem.
2. Verify native terrain diagonal selection; keep visible triangles, collision
   and object-height sampling consistent for the supported full-grid path.
3. Parse every installed XMI container with bounded synthetic regressions.
   Recover actual native selector semantics and independent day/night emitter
   types before connecting classic music to the runtime.
4. Add offline synthesis and deterministic note scheduling, then integrate
   bounded music streaming/cancellation. Test original audio silently; preserve
   separate limits for synthesis/timbres, acoustic fidelity and listening.
5. Continue remaining native region/group transforms, binary EQG volumes, border
   triggers, collision-deflected liquid crossings, movement-rate/environmental
   rules and event sounds. Each unknown requires source or measured evidence.

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

Discoveries to drive the next work, in priority order:

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

## Final structural survey and verification record

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
