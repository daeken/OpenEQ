# Rendering performance

## Plane of Knowledge investigation — 2026-09-28

The main slowdown was the shared surface-lighting shader. Every opaque pixel
and translucent fragment checked all **620 authored PoK lights**, including
lights in distant buildings. At Retina resolutions this dominated GPU time.
The HUD, network updates and terrain alignment were much smaller costs.

The renderer now builds a conservative spatial index when uploading a zone.
Each fragment looks up nearby light candidates, then applies the original 3D
radius test, attenuation, shading and fog. The index preserves source order,
duplicate lights and every original light value. It serves both deferred
opaque surfaces and forward translucent surfaces; resolution and shadow
quality are unchanged.

Cells start at 32 EQ units and grow for large zones. Dimensions are capped at
128×128 and references at 1,048,576 (about 4.13 MiB maximum buffer size).
Invalid or oversized inputs use the original complete light loop. Empty zones
have a valid fallback buffer too. Padding conservatively covers floating-point
cell/radius boundaries, and out-of-grid surfaces have no candidate lights.

## Measurements

Hardware: Apple M4 MacBook Air, 10-core GPU, 24 GB, Metal. The user's ordinary
`cargo run` development profile was used (workspace optimization level 1,
dependency optimization level 3). Classic models are the default.

Although the panel is 2880×1864, the macOS desktop is scaled to 1710×1107
logical pixels. A maximized test window produced **3420×2074 physical pixels**
(7.09 million), excluding its title bar/menu area. Measuring only a 1280×720
headless frame would miss most of the original lighting cost.

The live fixture uses Broker in PoK, leaving Explorer untouched. The diagnostic
camera is `[200, 200, -110]`, yaw −45°, pitch −8.594°. The scene contains 322
entities, 318 rendered actors, 142 doors, 415 static draw descriptors and 620
lights. The camera does not change outgoing character coordinates. NPCs remain
live, so these are repeatable fixed-view measurements, not deterministic replays.

Each run warms 60 frames and measures 240, with inventory/map closed and
logical UI dimensions half the physical dimensions. Loading, screenshot
readback and final timestamp draining are outside the measured interval.
The headless probe waits for each submitted frame to finish, exposing CPU
work and the remaining GPU wait separately. Its total is a **serialized test
frame time, not the normal windowed client's pipelined FPS**. CPU submit time
can overlap GPU execution; never add GPU timestamps to CPU/wait totals.

Initial 2880×1800 measurements isolated the cause: complete lights took
75.7–92.0 ms per serialized frame across runs, while removing zone lights as
a diagnostic took 18.3 ms. With the full light loop, lighting accounted for
58.1–74.2 ms of a 61.5–77.9 ms GPU frame. No lights are removed by the fix.

Final same-binary comparison at **3420×2074, classic models** (milliseconds;
each column is median / p95):

| Work | Full light loop | Spatial lookup |
| --- | ---: | ---: |
| GPU lighting attribution | 102.851 / 104.359 | 2.827 / 4.905 |
| Entire GPU frame span | 107.021 / 109.069 | 6.180 / 8.471 |
| CPU actor animation/uploads | 6.315 / 6.609 | 6.172 / 6.541 |
| CPU render/submit | 4.743 / 4.999 | 4.657 / 4.849 |
| Entire serialized test frame | 121.343 / 123.585 | 19.977 / 22.604 |

This is about **17× less GPU time** and **6× less serialized frame time** in
this view. The optimized GPU breakdown was shadow 1.438 ms, G-buffer 1.104 ms,
lighting 2.827 ms, transparency 0.674 ms and HUD 0.061 ms (medians are not
additive). The idle scene had no active spell particles, so their measured zero
is not a spell-load benchmark. Every one of the 240 GPU samples was valid in
both runs; none were dropped or left pending after draining.

The PoK grid has 66×67 cells, 32,920 light references and a 167,088-byte buffer.
The largest cell contains 77 candidates instead of 620; the mean across the
entire grid footprint, including empty cells, is 7.44. That mean is not a
pixel-weighted count for this particular view.

Further final-grid comparisons, also 240 frames:

| Resolution/models | CPU actors | CPU submit | GPU span | Serialized frame |
| --- | ---: | ---: | ---: | ---: |
| 1280×720, classic | 6.112 | 4.687 | 3.234 | 17.074 |
| 3420×2074, classic | 6.172 | 4.657 | 6.180 | 19.977 |
| 3420×2074, Luclin | 13.218 | 7.273 | 8.210 | 33.116 |

All times are medians in milliseconds. Luclin now makes the remaining animation
and draw-submission costs especially visible. Different camera views, active
effects, open panels and laptop clock/thermal behavior can change these times.
Raw local artifacts use `/tmp/openeq-pok-native-{brute,grid,luclin}` and
`/tmp/openeq-pok-small-grid` prefixes; proprietary screenshots are not bundled.

The normal windowed classic client was also checked at 3420×2074, using
Broker's actual saved view with normal FIFO presentation and profiling enabled.
Steady three-second windows measured **16.66–16.67 ms median frame intervals**
(about 60 FPS), with representative p95 intervals of 16.86–16.88 ms and no
failed/dropped GPU samples. This was a stationary view, not a walking-route or
worst-case combat benchmark. Its CPU `render_submit` includes surface/driver
pacing; it should not be compared directly to the headless submit column.
The local log is `/tmp/openeq-pok-windowed-final.log`.

## Profiling the normal client

```sh
RUST_LOG=warn,openeq::profiling=info OPENEQ_PROFILE=1 \
  cargo run -p openeq -- \
  --connect "$HOME/.config/openeq/storage2-credentials.json"
```

Every three seconds this reports physical resolution, CPU stage median/p95,
frame intervals (including normal pacing), GPU attribution and failed/dropped
timestamp counts. Profiling is off by default. A normal profiled frame never
waits explicitly for timestamp readbacks; four asynchronous slots bound memory
and drop measurements when busy. Unsupported GPUs still report CPU timing.

Apple tile GPUs overlap vertex and fragment stages of successive passes.
Raw pass intervals therefore cannot be added together. Reports retain those
raw intervals but also attribute each pass's completion after prior passes
complete. This attribution identifies the critical path; it is not an isolated
shader-cycle measurement. `frame_span` is the first start through last end.
Metal timestamp resolution is deferred until submission completion to avoid
reading incomplete or stale end counters. Frame IDs prevent averaging a stale
sample repeatedly; the live probe associates delayed samples with their source
CPU frames and explicitly reports missing/failed samples.

## Reproducing the fixed-view comparison

Use the dedicated Broker fixture, already in PoK. The probe refuses Explorer
and does not travel, cast, trade or intentionally move the fixture. Ordinary
server login may lift the saved Z by 0.75, as documented in the movement notes.
Credentials stay outside the repository and are not written to reports.

```sh
cargo build -p openeq --bin render_profile
OPENEQ_PROFILE_POS=200,200,-110 OPENEQ_PROFILE_YAW=-45 \
  target/debug/render_profile \
  "$HOME/.config/openeq/storage2-commerce-credentials.json" \
  /tmp/pok-grid 3420 2074 240 classic

OPENEQ_PROFILE_BRUTE_LIGHTS=1 \
  OPENEQ_PROFILE_POS=200,200,-110 OPENEQ_PROFILE_YAW=-45 \
  target/debug/render_profile \
  "$HOME/.config/openeq/storage2-commerce-credentials.json" \
  /tmp/pok-brute 3420 2074 240 classic
```

Each prefix produces a `.txt` report, per-frame `.csv`, and final `.png`.
`OPENEQ_PROFILE_NO_LIGHTS=1` is a diagnostic control only; omit it for normal
comparisons. `OPENEQ_PROFILE_PITCH` overrides camera pitch in degrees. Use
`luclin` in the final argument to compare the optional model family. Do not
run benchmarks concurrently with each other, GPU tests, or the windowed client.

An offline zone-only probe is also available:

```sh
cargo run -p openeq-render --bin renderzone -- poknowledge \
  --pos 200,200,-110 --yaw -45 --pitch -8.594 \
  --width 3420 --height 2074 --profile 240 --out /tmp/pok-static.png
```

Add `--brute-lights` for the original light loop or `--no-lights` for the
diagnostic zero-light control. This probe excludes NPCs, doors, HUD and gameplay.

## Remaining work and verification

- NPC animation still samples and uploads poses every rendered frame, even
  when a quantized 30 Hz pose is unchanged. Classic animation/uploads measured
  about 6.2 ms; Luclin about 13.2 ms in the maximized development-build probe.
  A pose cache and dirty upload ranges are the next CPU optimization candidates.
- Draw submission measured about 4.7 ms with classic models and 7.3 ms with
  Luclin. Those scenes had 5,604 and 12,168 actor/door draw descriptors,
  respectively, in addition to the zone. Counts include allocated pose slots;
  they are not a hardware count of nonempty draws per pass.
- Actor states have a distance cutoff, but rendering lacks view-frustum culling.
  Zone geometry is merged by material across WLD fragments, so effective terrain
  culling needs spatial batches. Shadow casters need their own visibility test.
- Default HUD construction/upload was roughly 0.25 ms combined. Long chat
  histories and open inventory/spellbook windows are separate workloads; this
  result does not establish their worst-case cost.

Grid tests compare candidate results against brute force at 10,000 deterministic
points, cell/radius boundaries, negative coordinates, huge radii and bounded
fallbacks. GPU tests compare every RGBA channel with the original loop for six
synthetic views each of opaque and soft-transparent geometry and three original
PoK views. All 15 comparisons were pixel-exact.
Profiling tests verify optional features, asynchronous bounded readbacks, valid
pass conversion, absent passes and identical pixels with profiling on/off.

Final validation: **340 workspace tests passed, zero failed or ignored**,
including original-asset/GPU tests; strict workspace Clippy, formatting and
client/probe builds passed. The asset tests require their explicit `EQ_DIR`:

```sh
EQ_DIR="$HOME/EverQuest" cargo test --workspace -- --include-ignored --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

Local verification logs are `/tmp/openeq-performance-tests.log`,
`/tmp/openeq-performance-clippy.log` and `/tmp/openeq-light-grid-pixels.log`.
