# OpenEQ agent handoff — October 1, 2026

## Start here: work is intentionally stopped

The user is moving OpenEQ into a new agent runtime and requested this handoff.
**Do not resume autonomous feature work or restart scheduled development solely
because an older document says “continue.”** Wait for the user's instructions in
the new runtime. The old `openeq-overnight-development` automation is **PAUSED**;
its last authorized window ended at 13:30 America/Chicago on October 1.

The verified development checkpoint is **`220b410` on `master`**, pushed to
`https://github.com/daeken/OpenEQ`. The working tree was clean before this
handoff-only documentation change. No feature work is in progress. This handoff
will itself be committed after that checkpoint; use the actual current HEAD
when continuing. The old C# mainline is preserved on
`codex/legacy-csharp-mainline-2026-09-28`.

OpenEQ is a replacement EverQuest client: Rust, Bevy for application/ECS/input,
a custom wgpu renderer, original installed EQ assets and EQEmu's **RoF2** wire
protocol. This project began in 2003–2004 and has had several reboots. The user's
goal is original-client parity with room for modern improvements. Their latest
priority is **world appearance/traversal and audio**, ahead of unrelated gameplay
features. A main menu was once a future request; **it is now implemented**.

Recommended reading order:

1. This document: present status, environment, migration and next steps.
2. [Daytime final checkpoint](DAYTIME_2026-10-01.md): detailed last verification.
3. [World/audio parity](WORLD_AUDIO_PARITY.md): evidence and active backlog.
4. [README](../README.md): player controls and available applications.
5. Relevant focused document from the tables below; read its latest completed
   verification, not just an early proposal near the top.

The repository contains extensive chronological research. Old milestone text,
old test totals, and old “not implemented” statements are historical evidence,
not automatically the current state. Source plus the newest focused validation
record takes precedence. This handoff distinguishes production support,
controlled native evidence, and proposals.

## Working agreements and safety boundaries

- **Never log into or alter Explorer**, the user's regular development character.
  README and old server notes contain direct connection examples targeting it;
  those examples are for the user, not authorization for automated testing.
- Use dedicated fixtures only for live tests. Revalidate fixture identity,
  offline status, deployed source/opcodes/rules and the particular probe's
  snapshot/restoration contract. Several “smoke” tools deliberately mutate
  inventory, coins, spells, XP, binds or character state. Do not run all of them
  as a generic test suite.
- Keep original EQ assets/binaries, credentials, restoration snapshots, database
  backups and private derived captures outside Git. Pass credential **paths**
  to existing programs; never print their contents or log passwords/session keys.
- Native audio verification stays offline or digitally silent. Use `--no-audio`
  for unattended clients; do not open an audible device simply to test a parser.
- Preserve unknown/unsupported data and explicit fallback boundaries. Do not
  guess packets, material transforms, region meanings, animation timings or
  transaction success. Server gameplay outcomes remain authoritative.
- Do not reimport PEQ, change global server rules, restart occupied zones, or
  deploy experimental server patches as part of routine client verification.
- Use `CARGO_INCREMENTAL=0`. Run heavy Cargo/GPU work sequentially when possible.
  Coordinate ownership if several agents share a checkout; stage explicit
  verified scopes rather than sweeping another agent's edits into a commit.
- The prior user authorized cohesive tested commits and pushes to `master`.
  Worktrees/branches are available for isolation; preserve that publication
  history rather than rewriting it. New runtime instructions can refine this.
- The user likes autonomous concrete progress and dislikes needless confirmation.
  Communicate substantive outcomes, failures or necessary decisions. Do not
  claim research-only discoveries as shipped behavior.

## Environment and first session

| Resource | Existing location / state |
| --- | --- |
| Rust client | `/Users/daeken/projects/OpenEQ` |
| Pinned EQEmu source | `/Users/daeken/projects/EQEmu`, clean `master`, commit `4aceae18b94ffaafc08e2b17bc41cd72c77f795d` at handoff |
| Original assets/client | `/Users/daeken/EverQuest` |
| Legacy C# | Preserved Git branch above; user also has an `OpenEQ-csharp` directory (locate it rather than assuming a path) |
| Toolchain | `rust-toolchain.toml`: stable + rustfmt/Clippy; workspace edition 2024, declared Rust minimum 1.90; verified locally with Rust/Cargo 1.97.1 |
| GPU baseline | Apple M4 MacBook Air, Metal, 24 GB; this is not a cross-platform GPU certification |
| Python research | Python 3.14.7; `/tmp/openeq-re-tools` and `/tmp/openeq-native-animation-venv` (migration details below) |
| Private user configuration | `~/.config/openeq/`, or `$XDG_CONFIG_HOME/openeq/` |

Set the **asset variables** explicitly on a new host. Many ignored tests use
`EQ_DIR`; account/creation UI tests also use `EQ_CLIENT_DIR`, and UI asset tests
use `EQ_UI_DIR`. Production/default-loader tools use `OPENEQ_CLIENT_DIR`. The current
`default_client_dir()` checks the latter, then hardcoded
`/Users/daeken/EverQuest`. It does not implement a portable home-directory search.

```sh
cd /path/to/OpenEQ
export CARGO_INCREMENTAL=0
export OPENEQ_CLIENT_DIR=/path/to/EverQuest
export EQ_DIR="$OPENEQ_CLIENT_DIR"
export EQ_CLIENT_DIR="$OPENEQ_CLIENT_DIR"
export EQ_UI_DIR="$OPENEQ_CLIENT_DIR/uifiles/default"
git status --short --branch
```

For offline interactive exploration and offline rendering:

```sh
cargo run -p openeq -- gfaydark --dir "$OPENEQ_CLIENT_DIR" --no-audio
cargo run -p openeq-render --bin renderzone -- anguish --out /tmp/anguish-handoff.png
cargo run -p openeq-assets --bin zonescan -- poknowledge
```

For the **user's** interactive play flow, without selecting a character on their
behalf:

```sh
cargo run -p openeq -- --login storage2.daeken.dev --no-audio
```

No arguments opens the main menu using saved server preferences. Play supports
sign-in, server/character selection, rotating preview and bounded character
creation. Interactive `/camp` returns to a fresh roster after its countdown;
Escape cancels. Direct `--connect` does not acquire that retained account flow.
Passwords are not saved by interactive sign-in.

On Linux, default audio builds need ALSA development headers (`libasound2-dev`
on Debian/Ubuntu). `--no-default-features` omits the playback device dependency;
parsing/decoding remains. macOS DLSSynth is currently required for XMI synthesis.
On another OS, an unavailable synth/GPU/original installation is a verification
limitation, not permission to report the previous host's tests as newly passed.

## Verified baseline and reproduction

At the final production checkpoint: **1,238 workspace tests passed across 121
suites, zero failed/ignored**, with original assets, GPU and silent audio.
Strict workspace lint, client/audit builds, all-target no-default, formatting
and diff checks passed. A test-only lint cleanup was followed by all 60 focused
placed-object asset tests passing; production code did not change afterward.
The handoff edits are documentation only; this turn did not rerun all tests.

```sh
cargo fmt --all -- --check
cargo test --workspace -- --include-ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p openeq --bin openeq --bin zone_audit
cargo check --workspace --all-targets --no-default-features
git diff --check
```

Establish prerequisites before the full run. For a code change, start with the
relevant meaningful regression, then required broader checks. Do not repeatedly
run the entire suite when no source changed. Original screenshot tests using
`OPENEQ_UI_CAPTURE_DIR` may require the destination directory to exist first.

Final logs are `/tmp/openeq-temple-final-workspace.log`,
`openeq-temple-final-clippy-fixed.log`, `openeq-temple-final-build.log`,
`openeq-temple-final-no-default.log`, `openeq-temple-final-assets-followup.log`
and `openeq-temple-final-fmt.log` in the same directory. Copies are in the private
handoff evidence bundle. Failed earlier attempts are recorded separately in
[the daytime log](DAYTIME_2026-10-01.md); they are not counted as passes.

The latest broad CPU survey examined **523 installed zone declarations: 501
structural passes and the same 22 known cases** (21 authored nonfinite outliers
and Dranikcatacombsa's internal banner dependency). It does not certify every
zone's textures, GPU appearance, collision traversal, NPCs or audio. Missing
classic Freeport assets require an *expected-zone* survey: discovering installed
zones alone cannot discover absent files. `freportw`/`freporte` are not aliases
for unrelated revamped Freeport maps; never silently substitute geometry with
different coordinates. See [the world record](WORLD_AUDIO_PARITY.md).

```sh
# A fresh output directory is required; use a new path per run.
python3 tools/audit_zones.py --binary target/debug/zone_audit \
  --dir "$OPENEQ_CLIENT_DIR" --output /tmp/openeq-next-zone-survey --jobs 2
# A focused expected-zone check can use --zones freportw freporte gfaydark.
```

Frozen final audit: `/tmp/openeq-lava-zone-survey/` (manifest, `zones.jsonl`,
comparison, final EQGZ recheck). Only 20 additional lava texture references across
16 zones changed in the last broad comparison; structural results stayed fixed.
Do not rerun 523 zones merely for a UI edit or a fixed-pose profiling change.

## Architecture and source map

Bevy is built without its renderer. It owns the application loop/ECS/input;
`openeq-render` owns wgpu device, surface, resources and passes. Window-handle
access stays on the appropriate main thread. Network workers use Tokio and
transfer typed events/commands into foreground reducers. Original XML defines
appearance/layout; application code owns actions. XML cannot execute network
commands or scripts.

| Area | Useful entry points |
| --- | --- |
| Application/orchestration | `crates/openeq/src/main.rs`, `live.rs`, `loading.rs`, `zone_loading.rs` |
| Input, game state and UI actions | `game.rs`, `interaction.rs`, `chat.rs`, `hud.rs`, `ui_layout.rs` in `crates/openeq/src/` |
| Account lifecycle | `account.rs`, `account/selection.rs`, `account_creation.rs`, `account_creation/editor.rs`, `account_ui.rs`, `account_ui/{menu,creation,preview}.rs`, `account_preview.rs` |
| Movement and travel | `crates/openeq/src/{coordinates,movement,zone_travel}.rs`; `crates/openeq-assets/src/{collision,collision_ascending,liquid_regions,zone_lines}.rs` |
| Asset formats/assembly | `crates/openeq-assets/src/{pfs,wld,zone,terrain,loader}.rs`, `loader/`, `character/` |
| Placed WLD animation | `loader/wld_objects.rs`, `wld_object_animation.rs`, key/translation/zero/positive reduction modules; renderer `scene/placed_animation.rs` |
| Rendering | `crates/openeq-render/src/{lib,scene,environment,light_grid,actors,doors,profiling}.rs`, `scene/`, `shaders/` |
| Authored material paths | Asset `loader/{ter_uv,ter_secondary_uv,ter_lighting,ter_lighting_pack,lava}.rs`; render `terrain.rs`, `lava.rs`, `waterfall.rs`, `additive.rs`, `transparency.rs` |
| Particles | Application `spell_effects.rs`; renderer `particles.rs`, `projectiles.rs`, `wld_particles.rs`, `wld_particle_gpu.rs`; asset `loader/wld_particle_*` |
| Audio | `crates/openeq/src/audio/{schedule,service,decode,settings,midi_synth}.rs`, `audio/xmi/`; asset `audio/` |
| Protocol | `crates/openeq-net/src/{stream,login,world,zone,session,gameplay}.rs`, specialized account/creation/raid/guild/training/death modules |
| Original UI definitions | `crates/openeq-ui/` and its README; application-specific HUD/account/social/progression modules bind state |
| Diagnostics | `crates/openeq/src/bin/*_smoke.rs`, `zone_audit.rs`, `render_profile.rs`; net diagnostic binaries; render `bin/renderzone.rs` |
| EQEmu research patches | `tools/eqemu-patches/`, `tools/eqemu-tests/`; patch presence does not mean deployment |

Coordinate invariants matter: original assets are Z-up. The network boundary
swaps EQEmu X/Y and converts heading as `(128 - heading) mod 512`. The renderer
maps asset `(x,y,z)` to `(x,z,-y)`. Avoid a second swap in scene/audio code.
Placement rotations have their own normalized convention; do not reuse server
heading conversion. Collision-only geometry is separate from visible geometry.
Invisible authored barriers must not reappear as magenta drawable meshes.

GPU work now includes shadow, G-buffer, lighting, lava, transparency accumulation
and resolve, waterfall, additive, particles and UI (10 profiling slots). An old
“four passes” architecture overview is conceptual/historical. The forward lava
pass is opaque and runs after deferred lighting but before transparent families.
Empty raster work omits timestamps while mandatory target clears remain. Keep
the timing decoder strict: hiding invalid samples is not a profiling fix.

## What works, and what that does not establish

| System | Implemented / verified scope | Remaining boundary |
| --- | --- | --- |
| Account/menu | Main menu/settings; interactive login/world/roster; Classic/Luclin/Drakkin preview; creation UI with immutable same-socket name approval/create; camp/cancel/fresh roster/reentry | Account registration, deletion, tutorial/return-home/heroic flows; broader creation appearance and first-zone-entry proof |
| Creation proof | Ordinary human warrior Trailborn, Classic face 1, created once and visible on fresh roster | No live zone entry in that proof; do not expand its appearance certificate to arbitrary races/features |
| Core gameplay | NPC movement/appearance; chat; target/assist; autoattack/hits; inventory/equipment/bags/stacks; corpses/loot; spells/gems/buffs/scribing/item effects | General original-client parity, all skill/combat/event types and long-session stress |
| Commerce/social | Merchant and personal/shared bank transactions; player trade; groups; bounded raid membership/leadership/chat; receive-only guild state | NPC hand-ins, UCS, guild mutations/refresh, raid subgroup/loot administration |
| Progression | Skills/languages/normal XP; 12 saved single-command hotbuttons; trainer UI and dedicated purchase/persistence proof | AA purchases, augmentation and broader progression; trainer replies do not provide general transaction certainty |
| Recovery | Forced same/cross-zone bind entry and living resurrection; stale-generation guards and original UI | Hover/cross-zone resurrection and full item/XP recovery are not comprehensively live-proved |
| UI | Original XML includes/art/atlases/layout, edit/gauge controls, scrolling descriptions, DPI input, saved positions/stacking/map/hotbuttons | General list/tree/STML/resize/font/skin compatibility; not a complete original UI engine |
| World | WLD, binary EQGZ and EQTZP heightmaps; restored terrain, authored invisible collision, supported region families, travel, lifts/swimming/levitation | Remaining embedded group regions/borders, collision-deflected medium changes, environmental damage, all original movement rules |
| Graphics | Deferred lighting/spatial point-light lookup, stable local shadows, fog/sky, direct terrain textures, selected layered/waterfall/additive/lava shaders, bounded placed animation | Exact original normals/light selection/bounce/direction/color-space behavior, unsupported shader families, water refraction/weather, moving placed collision |
| Audio | WAV ambience, MP3, macOS XMI; native four-slot loops and complete-packet SysEx; classic base gain/cooldown construction; synth crash fix | Original timbres, cross-platform synthesis, remaining host controls/tempo/pause/branches, event sounds, full emitter/voice/device lifecycle |

XMI parsing covers **79 installed files / 389 sequences**, and native-based
scheduling now admits all 389. Generic “XMI loops/SysEx pending” statements in
older checklists are superseded by [AUDIO_RUNTIME](AUDIO_RUNTIME.md),
[XMI_NATIVE_LOOPS](XMI_NATIVE_LOOPS.md) and [XMI_NATIVE_SYSEX](XMI_NATIVE_SYSEX.md).
Admission and exact bounded scheduling tests are not original timbre parity.

## World/audio continuation, when the user resumes

### 1. Collision-deflected liquids and active region/border evidence

Read [THIN_LIQUID_CONTACT_TRACE_PLAN](THIN_LIQUID_CONTACT_TRACE_PLAN.md),
[THIN_LIQUID_DEFLECTED_REVIEW](THIN_LIQUID_DEFLECTED_REVIEW.md),
[THIN_LIQUID_PREFIX_ENCLOSURE](THIN_LIQUID_PREFIX_ENCLOSURE.md),
[EQG_GROUP_REGIONS](EQG_GROUP_REGIONS.md) and
[ANGUISH_BASIN_ROUTE](ANGUISH_BASIN_ROUTE.md).

Rounded wall-slide prefixes can be nonmonotone even for a simple wall and box.
An endpoint chord or ordinary binary search can miss the first wet island.
The conservative enclosure finds the fixed witness in 138 nodes; a bounded
exhausted search returns **Unknown**, not “dry.” General collision admission and
continuation after changing medium remain unimplemented. The Anguish entrance
route is correctly dry above retained water metadata at 10/30/120 FPS; do not
move authored liquid coordinates to manufacture a swim entry.

Next concrete slice: a reviewed timed-contact interface with conservative
fallback, then original collision/medium continuation tests. For embedded group
areas, establish an **active native consumer**, not just a plausible parser or
transform. Preserve dry/unknown precedence and complete-set rejection boundaries.

### 2. Placed actors, moving collision and particles

Read [WLD_TEMPLE_ANIMATION_RESEARCH](WLD_TEMPLE_ANIMATION_RESEARCH.md),
[WLD_CRATE_ANIMATION_RESEARCH](WLD_CRATE_ANIMATION_RESEARCH.md),
[WLD_PLACED_ANIMATION_PERFORMANCE](WLD_PLACED_ANIMATION_PERFORMANCE.md),
[WLD_PARTICLE_FRAME_ADMISSION](WLD_PARTICLE_FRAME_ADMISSION.md) and
[WLD_PARTICLE_PREVIEW_ADMISSION](WLD_PARTICLE_PREVIEW_ADMISSION.md).

Shipped bounded families include five-frame ELECTMONU translation with stationary
collision, all 36 KRLAMP101 placements on a 1600ms loop, and North Qeynos Temple
rotation on an 8000ms loop. These use explicit scalar-reference gates, not a
universal CPU-path claim. Temple certifies stored f32 scores with outward bounds,
strict heap comparisons and rejection when a saved native right-neighbor heap
slot becomes stale after a left sift. Do not replace that with an epsilon or a
hardcoded omission list.

The remaining VSCRATE103 has separate translation and rotation timelines.
Native optimization removes its repeated final translation key, leaving a final
200ms plateau; scalar PC64 retains 17 translation keys while PC24 retains 18.
Generate a mixed-motion corpus through the full builder and both rankers before
extending runtime support. Moving collision and automatic placed particles need
separate ownership/clock/culling/RNG evidence; spell particles being implemented
does not authorize these placed emitters.

CPU measurements: roughly 12.4µs per Temple pose and 2.7µs per lamp pose in a
fully optimized non-LTO harness. All 36 lamp instances share one controller per
loaded scene's object definition; separate GPU scenes can create separate
controllers. These figures do not establish a global shared cache.
Preparation could be cached in an immutable sampler, but public mutable sources
make an unqualified cache unsafe. These microbenchmarks exclude GPU/upload costs.

### 3. Remaining terrain/shader/lighting fidelity

Read [EQG_LAVA_RENDERING](EQG_LAVA_RENDERING.md),
[EQG_TER_LAYERED_COLOR](EQG_TER_LAYERED_COLOR.md),
[EQG_TER_DUAL_FRAME_CHANNELS](EQG_TER_DUAL_FRAME_CHANNELS.md),
[EQG_NONFINITE_TER_UPLOAD](EQG_NONFINITE_TER_UPLOAD.md),
[SKY_AMBIENT_RENDERING](SKY_AMBIENT_RENDERING.md),
[SKY_DIRECTIONAL_RENDERING](SKY_DIRECTIONAL_RENDERING.md),
[NATIVE_DIRECTIONAL_CLOCK_LIFECYCLE](NATIVE_DIRECTIONAL_CLOCK_LIFECYCLE.md),
[NATIVE_DIRECTIONAL_CLOCK_ROLLOVER](NATIVE_DIRECTIONAL_CLOCK_ROLLOVER.md), and
[NATIVE_DIRECTIONAL_FRAME_PREREQUISITES](NATIVE_DIRECTIONAL_FRAME_PREREQUISITES.md).

Authored ambient and directional **RGB** are integrated for native environment
types **1/2/5**, valid original sky-table inputs and the renderer's normal-vision
policy; direction/shadow policy is still fixed. Sky angle updates and publication
of the cached light vector
are separate native operations. Do not substitute an attractive guessed daily
orbit. Native direct unsigned deadlines misbehave at wrap; a modern monotonic
policy should be named as such rather than called exact native scheduling.

The full normal-frame attempt stops at `FindFirstFileA` requesting
`AudioTriggers\*`, after native player/CRT construction but **before** host/engine
clock publication, directional update or rendering. It needs filesystem/config,
complete owner setup and active sky/light state. All three incomplete probes
are frozen with explicit unreached milestones; they are not full-frame proof.

MaxLava supports complete static exact `Opaque_MaxLava.fx` recipes with recovered
scroll/blend math and opaque depth/shadows. MaxLava2, animated/ambiguous/rebound
recipes keep fallback. Modern normals, color conversion, lighting membership
and device technique remain separate limits. Original Nest fallback captures
match the pre-change baseline exactly when its recipe map is disabled.
Keep FPU/packed-UV exceptional-value evidence separate from normal/tangent or
framebuffer parity. Bazaar, The Nest and Thundercrest restored geometry are
useful full-scene GPU targets; see [RESTORED_EQG_GPU_AUDIT](RESTORED_EQG_GPU_AUDIT.md).

### 4. Audio instance/admission fidelity and remaining events

Read [CLASSIC_AMBIENT_CLOCK_LIFECYCLE](CLASSIC_AMBIENT_CLOCK_LIFECYCLE.md),
[CLASSIC_SAMPLE_GAIN_RAMPS](CLASSIC_SAMPLE_GAIN_RAMPS.md),
[CLASSIC_AMBIENT_COOLDOWNS](CLASSIC_AMBIENT_COOLDOWNS.md),
[COREAUDIO_SYNTH_LIFECYCLE](COREAUDIO_SYNTH_LIFECYCLE.md) and the XMI notes above.

Native positive-cooldown attempted play resets its timer even if playback fails;
continuous null playback can retry every eligible update. A one-shot emitter
need not retain its returned instance. Concrete 2D/3D fades now have independent
state/event witnesses: both reach zero volume before a subsequent still-playing
update requests stop; 2D requests end+init, 3D only end. All Miles endpoints and
sample status are controlled, so real device ownership/status changes, asset
class selection and manager cadence remain open.

Next: connect those boundaries, then explicitly design separate emitter, voice
and resource lifetimes, overlap/caps, failed-resource policy and random-stream
ownership. Do not replace the shared scheduler with one incomplete rule.
Keep the **process-wide gate around every native macOS DLSSynth entry point**;
locking only create/dispose allowed a reproduced shared lazy-instrument-cache
crash. PCM waits and output callbacks remain outside that gate.

## Gameplay/server work deferred behind world/audio

NPC hand-ins are not a UI-only task. Stock cursor packets cannot distinguish
all append/refresh/silent-tail histories. Serials are not universal identities:
clones can preserve them and reconnect regenerates ordinary serials. Never
infer an empty queue from silence or globally deduplicate by item contents.
Read [CURSOR_RECONCILIATION](CURSOR_RECONCILIATION.md),
[QUEST_HANDIN_PLAN](QUEST_HANDIN_PLAN.md),
[CURSOR_PERSISTENCE_REPAIR_PLAN](CURSOR_PERSISTENCE_REPAIR_PLAN.md) and
[OPENEQ_CURSOR_EXTENSION_V1](OPENEQ_CURSOR_EXTENSION_V1.md).
The proposed owned-state extension is **proposal only**, not a stock opcode or
implemented/deployed protocol. Do not send its proposed opcode to arbitrary
servers.

Six numbered server patches are retained in `tools/eqemu-patches/`: cursor-tail
ownership fixes, full skill-cache keys, inventory save-error propagation, and
ItemInstance task-delivery clone preservation. All six remain **undeployed**;
they are isolated reviewed patches, **not a complete cursor persistence solution**.
Inventory operation atomicity, caller error handling and full cursor serialization
remain open.
Read [CURSOR_PERSISTENCE_AUDIT](CURSOR_PERSISTENCE_AUDIT.md),
[SKILL_CAP_CACHE_REVIEW](SKILL_CAP_CACHE_REVIEW.md),
[INVENTORY_SAVE_ERRORS_REVIEW](INVENTORY_SAVE_ERRORS_REVIEW.md) and
[ITEM_CLONE_REVIEW](ITEM_CLONE_REVIEW.md) before applying or deploying anything.
Do not assume their temporary worktrees or build harnesses exist on a new host.

Other backlog: augmentation/AA, quest journal, richer XML widgets, guild/UCS and
raid administration, full recovery and long-session reliability. See
[DEEP_PARITY_ROADMAP](DEEP_PARITY_ROADMAP.md); its older inventory-first ordering
was superseded by the user's world/audio priority.

## EQEmu operations and dedicated fixtures

These are documented deployment facts, **not a fresh server-health inspection**
at handoff. SSH to `storage2.daeken.dev` with existing access/passwordless sudo.
Runtime `/srv/eqemu`, service user `eqemu`; bootstrap source/build under
`/home/daeken/eqemu-bootstrap`. MariaDB database `peq` is localhost-only. Recorded
source revision is the pinned EQEmu commit above; initial PEQ schema 9328.

| Service | Endpoint |
| --- | --- |
| RoF2 login | UDP 5999 |
| Titanium login | UDP 5998 |
| World | UDP 9000, advertised “OpenEQ Storage2”, ID 1 |
| Zones | UDP 7000–7040; honor the returned zone address |
| Chat/mail | UDP 7778 |

`eqemu.target` owns login/world/chat, static PoK/Anguish, six dynamic workers and
network guards. Preserve dynamic-worker capacity and `Restart=always` on normal
empty-zone exit. Read-only status commands are in [SERVER_SETUP](SERVER_SETUP.md).
Run `sudo -u eqemu` commands with `/srv/eqemu` as cwd. The backup helper is
`/srv/eqemu/private/backup.sh`; backups/configuration are sensitive. Never rerun
the initial destructive PEQ import over player progress. Preserve the deployed
social opcode correction `OP_GroupCancelInvite=0x2a50` when inspecting/upgrading
opcode files. Source/config/binary hashes should be freshly compared before a
new live fixture or deployment; do not trust old running-service counts.

| Dedicated character(s) | Account(s) / purpose |
| --- | --- |
| Mechanic | `openeq_gameplay`: core gameplay, movement, travel |
| Arcanist | `openeq_spells`: spells and effects |
| Broker | `openeq_commerce`: commerce and rendering profile |
| Fellowship / Companion | `openeq_social1` / `openeq_social2`: groups/raid/guild; progression on Fellowship |
| Barterer / Swapper | `openeq_trade1` / `openeq_trade2`: trade; Barterer trainer/catalog/camp checks |
| Artificer | `openeq_itemuse`: item-use fixture with deliberately consumed/learned persistent state |
| Reviver / Rezzer | `openeq_recovery` / `openeq_resurrector`: recovery/resurrection; Reviver account/camp/progression |
| Trailborn | `openeq_create1`: retained ordinary creation fixture, no zone-entry proof |

Private connection files use descriptive `storage2-*-credentials.json` names
under the user's config directory. Discover **filenames**, not contents; consult
the matching focused proof document and binary guard before use. Old setup
coordinates/levels are snapshots, not current permission to restore a character
to those values. Nothing in this handoff authorizes logging into Explorer.

## Native research and evidence migration

A private persistent bundle was created specifically for this handoff:

**`/Users/daeken/.local/share/openeq/handoffs/2026-10-01/`**

It contains **537 selected text files, 153,015,233 bytes**, with source paths,
copied relative paths, sizes and SHA-256 in `manifest.json`. Every copy was
rehash-verified. Manifest SHA-256:
`722d95c3a93afa6b69db4d8bc2dad7c94ca47dcd66d3b6ad9462b6fa711a23b6`.
This directory is private and is **not committed**. It preserves key world/audio
scripts/results/logs, transitive literal helper references, final test/audit
records and the placed-animation source/benchmark text.

**It is not a complete backup.** The manifest explicitly lists exclusions and
unresolved literal references (some are document prefixes or example output
names). It excludes original DLLs/assets, compiled executables/rlibs, image/RGBA
captures, Python environments, large disassemblies and live/private restoration
fixtures. Keep the original machine's relevant `/tmp` directories until the new
runtime can replay its chosen witness. Do not publish the bundle wholesale;
JSON/results may contain original asset-derived values even without archives.

Migration checklist:

1. Clone/fetch OpenEQ with its branch history and handoff commit; retain the
   pinned EQEmu tree plus numbered patches. No running Codex/subagent session is
   required to continue the code.
2. Retain the private evidence bundle and selected original captures/compiled
   helper sources as needed. Copy the original EverQuest installation separately,
   under the user's existing asset rights. Retain the D3DX reference DLL below.
3. Transfer access/credential files separately through private storage with
   appropriate permissions. Do not place them in the handoff, prompts or Git.
   Live restoration snapshots and server backups also require separate handling.
4. Rebuild Rust/Python/native helper environments for the destination platform.
   Do not assume copied `.rlib`, executables or virtualenvs are portable.
5. Many probes contain absolute `/tmp` and `/Users/daeken` paths and assert hashes
   of imported helper source. Preserve the old mapping when practical, or adapt
   reviewed copies and record new hashes. Restoring only the top-level script is
   insufficient. Do not overwrite existing files blindly from the manifest.
6. Replay to **fresh output paths** and compare the frozen result hash. Keep
   original frozen inputs/results intact. A changed CPU mode/binary/environment
   requires new evidence rather than relabeling a prior result.

Current native tooling: Python 3.14.7 with `unicorn==2.1.4`, `capstone==5.0.9`,
`pefile==2024.8.26` available through `PYTHONPATH=/tmp/openeq-re-tools`.
Animation probes often use `/tmp/openeq-native-animation-venv/bin/python`.
Some shader/preshader probes additionally compile local C/Rust helpers; follow
their recorded commands/manifests. These scripts run original x86 instructions
under controlled emulation; they do not require logging into a game character.

Verified binary identities (retain the files outside Git):

| Binary | SHA-256 |
| --- | --- |
| `EverQuest/eqgame.exe` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| `EverQuest/EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |
| `EverQuest/mss32.dll` (XMI witness) | `fa78565a1e07df215532c611a5089256fcc1d81c1ae808eca66614c3ab77f9e0` |
| `/tmp/openeq-d3dx9-native/d3dx9_30.dll` | `5edeed79f2359527a55b8189cfa8b9b121cd608d44eead905a0f3436938ad532` |

Representative fresh-output replays:

These assume the original frozen inputs/dependencies have been restored to
their expected paths. In particular, the 3D fade probe reads the frozen
`/tmp/openeq-classic-sample-fade.json`, **not** the new 2D output below. The crate
probe requires `/tmp/openeq-native-long-object-loader.json`. Preserve exact
filename case, including `EQGraphicsDX9.dll`, on case-sensitive filesystems.

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-direction-rollover.py /tmp/new-direction-result.json
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-classic-sample-fade.py /tmp/new-2d-fade-result.json
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-classic-3d-sample-fade.py /tmp/new-3d-fade-result.json
/tmp/openeq-native-animation-venv/bin/python /tmp/openeq-native-crate-lifecycle.py scalar 0x37f /tmp/new-crate-result.json
```

Dependency examples: normal-frame → particle-frame-admission → lamp-cadence →
native-particle-placement; directional lifecycle → host-sky-light-native-math →
ter-light-binding. Preserve FPU precision, instruction addresses, clock inputs,
output interception and asset/binary hashes. Exact controlled traces are not
native live-frame/GPU/device parity. Independent review and explicit negative
controls have been important to keeping these boundaries honest.

## Documentation traps and a sensible first resumed task

Known superseded statements include old `ARCHITECTURE.md` paragraphs claiming
no creation or invisible collision, early `ACCOUNT_FLOW_PLAN.md` statements that
creation/camp are not connected, and old milestones deferring hotbuttons,
trainers, player trading, raids or all XMI. Consult `ACCOUNT_UI.md`,
`CHARACTER_CREATION_PROOF_PLAN.md`, the later camp section, `TRAINER_LIVE_PROOF_PLAN.md`,
`SOCIAL_PARITY_PLAN.md` and `AUDIO_RUNTIME.md` for current scope. Historical plans
may contain proposed commands and tests that were never executed.

After the user resumes work: establish asset/tool availability, read the latest
checkpoint, check Git for new edits, and pick one bounded task from the ordered
world/audio list. A useful independent research slice is the generated crate
mixed-motion corpus; the deepest active priority is general timed liquid
contact/continuation. Review and test a concrete slice before enabling it.
Record discoveries, precise limitations and verification alongside the change.
Do not recreate the old agent tree or scheduling arrangement as a prerequisite.
