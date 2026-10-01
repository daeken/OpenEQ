# Authored sky ambient in live rendering

October 1, 2026. Eligible outdoor zones now use the original sky table's ambient
color in the shared surface-lighting uniform. Previously every zone/day used
OpenEQ's fixed `[0.22,0.24,0.30]` ambient. The existing minute-based sky refresh
updates this input together with the sampled sky; no new clock is introduced.

## Bounded transfer

`EnvironmentSettings.zone_type` retains the native environment/time type:
NewZone byte 520, supplied by PEQ `time_type`. This is **not** NewZone `ztype`
at byte 470 or the separate PEQ `ztype` column. The main client, offline snapshot,
live smoke tool and rendering profiler all pass the same field. Unspecified
standalone settings leave this field absent and retain the prior ambient.

Only types 1, 2 and 5 plus a structurally valid `OriginalDome` table enable
this transfer. `raw_light_colors()` reads RGB from reserved entry `(31,3)`;
alpha is ignored. Each byte receives the recovered normal-vision floor of 20,
then is divided by 255 for the existing renderer. Native construction and the
3,144-case arithmetic witness are in `NATIVE_HOST_AMBIENT_BYTE_FLOOR.md`.
The packed-byte result is independent of the twelve tested x87 modes. Native
normalized float results can vary by one ULP; Rust normalization remains an
explicit rendering policy rather than a universal original-CPU bit claim.

The renderer recomputes its private ambient value on **every** environment
application, including absent or malformed sky data. Unsupported inputs use
the prior OpenEQ fallback; they cannot retain the previous zone's ambient.
This fallback does not reproduce the original cached-sky getter or missing-sky
hourly tables. Camera-liquid presentation changes sky/fog without reselecting
zone lighting, and resurfacing preserves the authored ambient.

No GPU layout, pass, texture lookup or new per-frame allocation is added. All
ordinary shared lighting passes receive the uniform; emissive surfaces remain
unlit. Lighting swatches remain excluded from visible sky sampling.

## Verification

Five tests in `crates/openeq-render/tests/sky_ambient.rs` cover:

- Every environment-type byte, every ambient byte, asymmetric RGB, zero/opaque
  alpha, liquid presentation and offline snapshot field mapping.
- Missing table, wrong layout/size and malformed pixel payload fallbacks.
- GPU pixel checks on an isolated white receiver with a downward normal, so
  fixed sunlight contributes zero: refresh, missing sky, unsupported zone and
  a liquid round trip all follow the expected ambient/reset contract.
- Emissive geometry and complete empty-sky images stay unchanged when only
  ambient swatches change.
- Original PoK night/dawn/day/dusk tables drive the isolated surface with the
  independently read source RGB; daylight and night captures differ.

Original-asset checks are opt-in and run offline. No live player or sound output
is involved. Focused GPU tests pass; final workspace verification is recorded
in `DAYTIME_2026-10-01.md` (an unrelated native synth concurrency crash during
its first broader run is investigated separately, rather than counted as a pass).

## Remaining fidelity

The current renderer still treats character lighting as normal vision.
Infravision/ultravision selection, special ambient A/B, native directional color,
angle/bounce lifecycle, point-light membership, native normal/tangent shading,
encoded-color arithmetic and display transfer remain separate. Existing linear
lighting, fixed sun, shadowing and fog policies are retained. Consequently this
is authored ambient input support, not complete native daytime/nighttime pixels
or a claim that every zone's illumination now matches the original client.

## Original-zone visual review

An independent offline full-scene PoK comparison holds camera, time, geometry,
sky, fog, sun and shadows fixed and changes only ambient admission. Camera:
`[200,200,-110]`, yaw -45 degrees, pitch -0.15, 1280×720, elapsed zero.
The loaded scene has 633 meshes/materials, 146,906 mesh triangles, 1,249
instances and 620 lights. Authored daylight changes 855,314 pixels, all brighter
than the old ambient; night changes 855,117 pixels, all darker. Empty-sky
controls stay byte-identical. After night → missing sky, the complete frame
matches an independently configured no-sky fallback.

Root visually inspected the authored day/night captures: building facades,
paving and vegetation retain detail while ambient shade changes. Fixed sunlight
still brightens some night-facing surfaces, making the remaining directional
approximation visible. No original-client framebuffer or live NPC/character,
weather-transition or clock synchronization comparison is claimed.

Artifacts: `/tmp/openeq-ambient-scene-review/{day-legacy,day-authored,
night-legacy,night-authored,missing-sky-reference,missing-sky-reset}.png`.
Temporary source SHA-256:
`d4512bfa5da01500d6ad9f4fbdbae1619597c23a274d657f43dab8395aba981c`.
Log SHA-256: `48ed9117a4bf3bb45401d6fd1db790a929c1e2d66e7285ae39505bca8019ab6c`.
The two missing-sky PNGs share SHA-256
`a951d9c849f938987ad793a29f9cd71e2ebf49e040f4dae047abb982360900bc`.
