# Native sky directional-color selection

2026-10-01. This connects the original host's clock calculation, concrete sky
sun/moon switch, published-table getters and packed directional-light setter.
It supports a narrow renderer change: select the authored directional RGB from
the same explicit float time used to sample the sky table. It does not recover
live update ordering, celestial motion, native display transfer or frame parity.

## Executed boundary

The complete original host routine `eqgame.exe:0x004b9440` runs with its original
directional-light and definition objects. The sky uses concrete original vtable
`0x101374dc`, rather than replacement selection or color methods. Its manager
and published color-table pointer are explicitly supplied. The sky angle fields
are fixed controls, and original 512-entry trigonometry executes. Clock output,
allocation/free/memcpy and the separate trailing ambient call are controlled
interfaces. No game process, audio, network session or GPU is started.

The connected calls are:

1. Host clock helper `0x004add30` supplies the raw hour and minute. Original
   host instructions calculate `(hour * 60 + minute) * f32(1/1440)` and store
   the result as an f32 argument to the original sky time setter `0x100c2c40`.
2. The concrete sky switch `0x100c2c70` reads that stored float. For finite
   inputs, it chooses sun within the inclusive interval below, moon outside.
3. Sun getter `0x1002c7d0` reads table offset `0x07c`; moon getter `0x1002c810`
   reads `0x0fc`. The original host passes the selected packed word to
   directional setter `0x10012560` without changing it.
4. The setter extracts packed bytes 2, 1 and 0 as RGB and multiplies each by
   the stored f32 reciprocal of 255 at `0x10135024`. Source alpha is ignored.
   There is no ambient floor, alpha multiplication or sRGB decoding here.
5. The corresponding sun/moon bounce getter and original bounce setter execute
   separately. Their presence in the witness does not require adding bounce to
   OpenEQ's current shading model.

| Switch limit | Original f32 bits | Value |
| --- | --- | --- |
| Sun begins, inclusive | `0x3e7c71c7` | `0.2465277761220932` |
| Sun ends, inclusive | `0x3f438e39` | `0.7638888955116272` |

The switch's finite-input comparison is stable across all 12 tested x87
precision/rounding combinations: 24/53/64-bit precision, each with nearest,
downward, upward and toward-zero rounding. This does not establish the control
word of a live original client.

## Keep the sampled float, not only its quantized tick

The lower limit and the immediately preceding f32 both truncate to sky-table
tick **16156**, but choose sun and moon respectively. The upper limit and the
immediately following f32 both truncate to tick **50062**, but choose sun and
moon respectively. Reconstructing time as `day_tick / 65536` therefore loses
information needed by the native switch.

The useful implementation boundary is to retain the exact normalized input
float, for example as `day_fraction_bits: u32`, alongside existing sky-map
provenance. Lighting can use that fraction and the table produced from it as
one snapshot. A fresh server clock combined with an older asynchronously loaded
table would not preserve this relationship.

The original host's own hour-to-float arithmetic is a distinct boundary. A full
1,440-minute sweep using raw hours 1 through 24 under each of the 12 x87 modes
finds exactly one minute whose selected branch changes with rounding:

| Raw clock | Rounding | Stored fraction bits | Branch |
| --- | --- | --- | --- |
| 18:20 | Nearest, downward, toward zero | `0x3f438e39` | Sun |
| 18:20 | Upward | `0x3f438e3a` | Moon |

All three precision settings exhibit that same distinction. At 05:55 every
tested mode selects sun, although its stored fraction can differ by one ULP.
An hour-only switch described as universally including 18:20 would overstate
the evidence. OpenEQ can keep its existing time normalization and clock
construction as explicit modern input policy, then apply the recovered switch
to the exact resulting sample fraction. Native raw hour 24 can produce a
fraction at or above one; this probe does not assert that OpenEQ's modulo-one
normalization reproduces that original host input.

## Zone gates and fallback lifecycle

The witness executes both day and night controls for every possible zone-type
byte. Only **1, 2 and 5** take the sky directional path. The host active flag
independently gates the entire routine.

For other zone types, the original host blackens the light **definition**,
immediately clears bounce and disables the sky if present. It leaves the
directional object's cached RGB and vector untouched during this routine.
For admitted types without a sky, it writes the hourly grayscale to the
definition and likewise retains cached RGB/vector and bounce. Missing and
inactive controls assert those distinctions explicitly.

Consequently, retaining OpenEQ's existing default directional color when the
zone type, original table or sample provenance is unavailable is an explicit
fallback policy. Neither this default nor an immediate reset to black should
be described as recovered native cache lifecycle. A valid `OriginalDome` table,
valid sample fraction and exact zone-type gate form the narrow supported path.

## Authored colors and byte transfer

The original installed default day/night/dawn/dusk DDS tables are supplied as
published-table fixtures, and both sun and moon branches execute for each.
These controls deliberately decouple the chosen table from the clock; native
table sampling is established separately in `SKY_LIGHT_COLOR_INPUTS.md`.

| Published table | Sun packed AARRGGBB | Moon packed AARRGGBB |
| --- | --- | --- |
| DefaultDay | `ffffffff` | `ff000000` |
| DefaultNight | `ff000000` | `ff0d56e7` |
| DefaultDawn | `00feae51` | `00444e75` |
| DefaultDusk | `00ff9a24` | `00000000` |

The zero-alpha dawn and dusk sun colors still contribute their complete RGB.
Asymmetric control colors and 96 alpha-variation executions verify that
changing alpha alone leaves directional and bounce RGB bits unchanged within
each control-word mode. A source red byte of 18 remains below the ambient
floor of 20; directional transfer does not reuse that ambient adjustment.

Native reciprocal multiplication can differ by one ULP from Rust `byte / 255`
and across rounding modes. Keeping normalized bytes in the current linear
renderer is an explicit approximation to the original encoded-color pipeline.
Retaining the existing fixed direction, shadows and absence of bounce remains
a separate visible limitation. This evidence supports authored directional
color selection, not a claim of complete native sunlight behavior.

## Reproduction and frozen artifacts

Run `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-native-directional-selection.py`. The original images and generated
buffers remain outside the repository. All assertions pass: 17,280 complete
host minute cases, 96 direct switch cases, 512 zone-gate cases, 15 missing or
inactive controls, eight authored-table cases and 96 alpha cases.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-native-directional-selection.py` | `eded42c32c5423e4547f17fc80d9dc633973b42eede644e16aed695e2ae22843` |
| `/tmp/openeq-native-directional-selection.json` | `3f19a31fb68036e0224b09334d05101924049280c6a05420daf4e3b5064359eb` |
| `/tmp/openeq-native-directional-selection.log` | `4da9a0b3a61752361908240b431950af5e23bef162ec8d48ea4bc8e588268308` |
| Original `eqgame.exe` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| Original `EQGraphicsDX9.dll` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |

The probe verifies frozen setup sources before executing their initialization
prefixes. Their hashes, each installed DDS hash, exact trace records, RGB float
bits and original entry counts are in the JSON. It executes 17,911 host calls;
17,390 of them reach the original directional packed setter. Existing probes
and their result files are unchanged.

## Independent integration and full-scene GPU review

The corresponding Rust changes were independently reviewed: exact sample bits
are retained before tick truncation; the selector uses both inclusive original
thresholds, exact zone-type gates and validated original-table input; RGB has
neither alpha scaling nor an ambient floor. The renderer recomputes directional
color on every `set_environment` call, including `None`, and supplies that state
to the existing surface-lighting uniform. No blocking integration issue was
found. The current linear pipeline, fixed direction and shadow model remain
explicit limitations.

A separate temporary Rust tool renders original Plane of Knowledge geometry
offline at a fixed 1,280×720 resolution and zero elapsed animation time. The
scene contains 633 materials/meshes, 146,906 mesh triangles, 1,249 instances and
620 point lights. Camera position is `[200,200,-110]`, yaw −45 degrees, pitch
−0.15 radians and vertical FOV 70 degrees. Original zone fog, sampled sky,
clouds and point lights are enabled. No client, character, audio or network
session runs.

For each time, the comparison removes only the main sky's sample provenance
from a cloned `SkyAssets`, retaining identical authored ambient, sky pixels,
clouds and fog while selecting the old fixed directional RGB. Both control
images are **byte-identical to independently frozen captures made before the
directional change**. The authored result therefore isolates this directional
integration rather than combining it with the earlier ambient improvement.

| Time | Authored directional RGB | Changed pixels | RGB-sum brighter / darker | Largest channel change |
| --- | --- | --- | --- | --- |
| Day, fraction 0.5 | `[1,1,1]` | 282,434 | 282,434 / 0 | 7 |
| Night, fraction 0 | `[13,86,231]/255` | 310,548 | 12 / 310,507 | 100 |

The remaining 29 changed night pixels keep the same RGB sum. The small number
whose RGB sum increases is expected: authored blue `231/255` exceeds the old
fixed blue `0.86`, while red and green decrease. The capture checks that no
red or green channel increases; 231,249 blue channels increase and none
decrease. The RGB channel-sum changes over the whole night image are
`[-8792031,-5198336,+278452]`.

Additional whole-image assertions pass:

- Empty-scene sky/cloud controls are byte-identical for old and authored
  directional light at both times.
- Setting the sky to `None` after authored moonlight exactly matches a fresh
  no-sky render, including default ambient and directional fallback.
- Reapplying day and night after that fallback exactly reproduces their first
  authored captures, with no stale selection.

The authored day and night images and old night control were visually inspected.
Daytime surfaces become slightly more neutral; nighttime exposed roofs, grass
and paving pick up authored blue moonlight. Existing warm point-light patches
remain visible. This is an OpenEQ GPU regression comparison, not an original
client framebuffer comparison.

Artifacts are outside the repository in `/tmp/openeq-directional-scene-review/`.
The temporary tool was compiled with `CARGO_INCREMENTAL=0` against the current
assets/render libraries; no production files were changed by the review.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-directional-scene-review.rs` | `beb9121ed7bb2ac484cf21e0300648a8f03af650fe599f361c09316b6d61d366` |
| `/tmp/openeq-directional-scene-review.log` | `e339ebfd032b138117a8f82cf0331a5fd0df09388e6cdb45300b884c34840098` |
| `day-ambient-only.png` | `64c539e18457aaf1d98e7ee1bb8552f9e6febef7bbfae70e1637ce01fbe3fe33` |
| `day-authored-directional.png` | `bebf01ffade9abc82bef659e25085ee4cca033cfa97cd41924cfb32d739cb5d5` |
| `night-ambient-only.png` | `19172cf590c73a0274c7197d9b4b4cc8a8dcf36e682329ac21e9b3662a48c58d` |
| `night-authored-directional.png` | `2a890040f352c7f7364bdcdf10feff531d020078cb7d509414444683bbe85157` |
| `missing-sky-reference.png` and `missing-sky-reset.png` | `a951d9c849f938987ad793a29f9cd71e2ebf49e040f4dae047abb982360900bc` |
