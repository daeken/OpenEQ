# Original audio: formats and first integration

Research snapshot: 2026-09-29, using the installed client at
`/Users/daeken/EverQuest`. This is an implementation plan, not a claim that audio
playback is implemented. Research inspected bytes and source only; no sound was
played and no original asset was copied into the repository.

The first useful slice is authored zone ambience plus MP3 zone music, with a
silent backend for tests and machines without audio devices. This reaches Plane
of Knowledge and Anguish without requiring a MIDI synthesizer. Keep XMI music,
animation/footstep sounds, combat events, reverb, and exact legacy attenuation
as explicit follow-up work.

### Implemented asset slice

`openeq_assets::audio` now implements the metadata/index portion, without any
audio device or codec dependency. `AudioCatalog::load(client_directory)` indexes
loose WAV/MP3/XMI files and numbered PFS banks, loads the global sound/MP3 tables,
and returns bounded diagnostics. `load_zone(short_name)` returns `ZoneAudio`
with typed `AudioEmitter::Classic`/`Emt` records and preserved raw fields.
`asset(name)`/`asset_for(reference)` expose `AudioAssetLocation::Loose(path)` or
`Archive { path, member }`; `read(name)` and `AudioAsset::read()` retrieve bounded
source bytes lazily. XMI references retain their original sequence number.

Limits enforced here: 4 MiB metadata, 4,096 emitters/zone, 65,536 table/directory
entries, 256 diagnostics plus a suppressed count, 256 sound archives, 128 MiB
compressed per archive, 64 MiB per source/member and 8 MiB per archive directory.
Archive member/block lengths are validated before using the shared PFS reader.
No decoded audio cache, timers, synthesis or output is part of this asset API.

Initial validation passed 11 synthetic parser/index tests and five silent
original-asset tests, plus strict assets Clippy. The tests cover archive
overlays, damaged lengths, metadata boundaries, unknown values, missing banks,
positive XMI references, the MP3 index correction and the malformed WestKorlach
row. Original tests remain opt-in and do not bundle proprietary bytes.
The broad audit loaded 460 zones and 48,017 emitters. Negative legacy radius
sentinels are retained
as unsupported; eight EMT rows with negative ranges and the two malformed
WestKorlach rows are rejected with diagnostics rather than shifted/repaired.

## What is available

At the start of this audit the client had no audio dependency or output backend.
Bevy 0.19 is built with default features disabled; `bevy_audio` was not enabled.
The old C# tree has
an opcode named `Sound` but no audio implementation found by the source scan.
The assets crate already retains spell-stage sound IDs; see
[spell effect notes](SPELL_EFFECT_RENDERING.md).

Installed files:

| Asset | Count | Purpose |
| --- | ---: | --- |
| `*_sounds.eff` | 134 | Classic binary emitter records |
| `*_sndbnk.eff` | 135 | Classic per-zone text sound banks |
| `*.emt` | 330 | Later text emitter lists |
| Root `*.mp3` | 257 | Mostly music |
| Root `*.xmi` | 79 | Miles/XMIDI sequence containers |
| `sounds/*` | 2,395 | 2,320 PCM WAVs and 75 ID3-prefixed MP3s |
| `snd1.pfs` through `snd17.pfs` | 17 | 2,127 WAV members, 2,088 distinct names |
| `soundassets.txt` | 1 | Global numeric sound ID to filename table |
| `animationsounds.txt` | 1 | Animation key to numeric sound ID table |
| `mp3index.txt` | 1 | Classic music ID to root MP3 filename table |
| `defaults.eal` | 1 | RIFF environmental audio defaults, including sound levels |

The numbered archives total 125,219,325 compressed bytes. They are necessary:
16 of the 18 distinct sound names in PoK's bank are absent from the loose
`sounds` directory. For example, `page_turn01.wav`, `flag_flap_lite_lp.wav`,
`nightime_background02_lp.wav`, and `wind_magical.wav` are in `snd11.pfs`;
`cough_01.wav` and `glass_shattering.wav` are in `snd14.pfs`. All the classic
Greater Faydark bank sounds are in `snd6.pfs`.

Reuse `openeq_assets::pfs::Archive::{open,names,contains,read}`. Its filename
lookup is already case insensitive and its directory handling is correct for
these archives. Build a name/location index, then open/decompress only assets
needed by nearby emitters. Do not eagerly decode every archive member.

Suggested deterministic lookup: case-insensitive loose `sounds` files, root
files, then numbered sound archives. Loose files should override archived
versions. For archive collisions, choose and document a numeric precedence
(suggest later-numbered archive first); **native archive precedence has not
been established**. There are 39 duplicate member names, including differently
sized `snd1`/`snd7` door sounds, so hash-map iteration order is not acceptable.
Restrict authored names to local asset names; do not interpret them as arbitrary
paths or URLs. A known `sounds\` prefix occurs in 1,214 EMT rows; strip that
single local prefix (also accept `sounds/`) before basename lookup, with normal
case folding. Reject nested paths, traversal and drive/URL prefixes.

## Classic EFF files

`{zone}_sounds.eff` has no header: it is an array of little-endian **84-byte**
records. Every installed file has an integral record count. Read known fields
with bounds checks and retain opaque fields for later comparison.

| Offset | Encoding | Meaning / confidence |
| ---: | --- | --- |
| 0–15 | Four 32-bit words | Opaque; some PoK values resemble saved pointers, not IDs |
| 16, 20, 24 | `f32` | Asset-space X, Y, Z; Z is up |
| 28 | `f32` | Radius |
| 32, 36 | `i32` | First/second repeat cooldown, milliseconds |
| 40 | `i32` | Additional randomized delay, milliseconds |
| 44 | `i32` | Opaque |
| 48, 52 | `i32` | First/second sound or music selection |
| 56 | `u8` | Legacy emitter kind; 57–59 are padding |
| 60, 64 | `i32` | Effect sound levels, or music loop counts, depending on kind |
| 68 | `i32` | Music fade-out milliseconds; effect interpretation uncertain |
| 72, 76, 80 | `i32` | Type-dependent parameters; retain raw |

Public readers agree on kind 0 as day/night 2D ambience, kind 1 as music, and
kinds 2/3 as other effect emitters. They disagree about how the latter kinds
use their second selection and trailing words. EQ Sage treats offset 76 as a
full-volume radius and offset 68 as a fade for all kinds; LanternExtractor
does not. Do not declare those fields decoded for all kinds.

For kind 0, choose the first selection by day and second by night; zero means
silence, and equal selections represent one voice, not two. Use the live world
clock, not the host clock. Kind 1 has analogous day/night selections. Exact
day/night boundaries and the native overlap/priority behavior need comparison.

`{zone}_sndbnk.eff` is a newline-separated list with `EMIT` and `LOOP` sections.
Handle CRLF/LF and surrounding whitespace; preserve index positions rather than
silently reordering or deduplicating names. Resolution is contextual:

| ID / context | Resolution |
| --- | --- |
| 0 | No sound |
| Effect ID 1–31 | `EMIT[id - 1]`, add `.wav` if absent |
| Effect ID 32–161 | Global `soundassets.txt` mapping |
| Effect ID 162+ | `LOOP[id - 162]`, add `.wav` if absent |
| Negative music ID | `mp3index[(-id) - 1]` |
| Positive music ID | A sequence in `{zone}.xmi`; preserve raw index until XMI support |

Use checked negation for negative indices. Bank IDs and global sound IDs are
different namespaces: global spell sound 162 must not be resolved through a
zone's `LOOP` section.

The negative MP3 mapping is **one based**. PoK's two music emitters both specify
`-14`; line 14 of `mp3index.txt` is `poknowledge.mp3` (zero-based element 13).
`bothunder` uses `-1`, `codecay` uses `-2`, and 23 installed EFF files with
negative music references consistently match `(-id)-1`. EQ Sage's `[-id]`
lookup is off by one for these assets.

Do not use the small hardcoded internal sound table from that reader. The
installed `soundassets.txt` uses `id^filename^`, contains sparse IDs through
4659, and identifies ID 39 as `Death_M.WAV`; that reader instead says
`death_me`. Treat `Unknown` and absent entries as missing audio, not filenames.

### Levels and scheduling

The following third-party level interpretation was superseded for supported
kind-0 base gain by original-instruction execution on October 1; see
[CLASSIC_AMBIENT_LEVELS.md](CLASSIC_AMBIENT_LEVELS.md). Ordinary nonpositive
levels use 0.2, positive values attenuate through 10,000, and larger values
plus the signed-minimum edge are silent. No native EAL lookup participates in
this legacy emitter-construction path. Downstream EAL behavior remains separate.

Historical source interpretation: LanternExtractor interprets nonzero legacy effect levels as negative absolute
DirectSound/EAX hundredths of a decibel, with linear gain
`10 ^ (-abs(raw) / 2000)`. A zero level asks for the sound's default level from
`defaults.eal`, falling back to unity if unavailable. This is a credible legacy
interpretation, but it is not verified against native playback here.

The EQ Sage expression `3000 - raw` is not a normalized gain and should not be
copied. `defaults.eal` is present (22,180 bytes, `RIFF ... eal `); Lantern's
reader gets `SourceModels[].SourceAttributes.EaxAttributes.DirectPathLevel`
from it. EAL parsing and exact legacy level parity can follow the first slice.
Preserve raw values; the earlier proposed unity fallback is no longer used
for supported classic ambience.

For supported effect kinds, a sensible initial schedule is continuous when
cooldown and random delay are both nonpositive; otherwise wait the nonnegative
cooldown plus a sampled nonnegative random delay. Native timer anchoring
(start-to-start versus end-to-next-start) remains unverified. Start a new
occurrence only after the previous one ends, and never enqueue an accumulated
burst after a stall or zone transition.

Type 2/3 spatial behavior needs a source check before claiming fidelity. It is
better to explicitly omit unsupported records initially than invent meanings
for trailing fields. In particular, PoK record 5 has radius 0 and a trailing
value 50; treating all zero radii as global sounds would turn this horse sound
into zone-wide ambience.
Other authored records use radius -1, -2 or -4 (for example Jaggedpine and
Plane of Water). These are likely conditional/global sentinels, but their
meaning is not yet decoded. The parser retains and diagnoses them; the first
runtime must omit them instead of treating their magnitude as a radius.

### Concrete original fixtures

Indices below are zero based; coordinates are source/scene coordinates.

| Fixture | Expected metadata |
| --- | --- |
| PoK EFF | 3,360 bytes, 40 records |
| PoK record 0 | Kind 0; `(-782.696, 890.960, -147.947)`, radius 850; day silent, night ID 165 → `nightime_background02_lp.wav` |
| PoK record 1 | Kind 0; `(-865.476, 280.969, -155.097)`, radius 75; both IDs 166 → one `scientist_lab_lp.wav` voice |
| PoK record 38 | Kind 1; `(-0.451, 1489.518, -124.249)`, radius 500; both IDs -14 → `poknowledge.mp3`; fade-out 2000 ms |
| PoK record 39 | Same track, another region; moving between them should not restart it |
| Greater Faydark EFF | 1,596 bytes, 19 records; 4 XMI music records plus effect/ambient records |
| Greater Faydark record 12 | Kind 0; day ID 162 → `wind_lp2.wav`, night ID 164 → `darkwds1.wav` |

## Later EMT files

EMT is comma-separated text. The installed set contains **39,614 rows**, not
all the same length:

| Fields | Rows | First-field values |
| ---: | ---: | --- |
| 19 | 985 | 984 begin with `1`; one malformed row begins with `2` |
| 20 | 8,556 | `2` |
| 21 | 15,375 | `2` |
| 22 | 14,698 | `2` |

The first field appears revision-like; it is **not the classic EFF kind**.
For example, Anguish's only row starts with `2` but plays MP3 music. Keep it
opaque until native parsing is checked. The first 19 fields otherwise share
this source-documented layout; later fields are extensions:

| Index | Field |
| ---: | --- |
| 0 | Opaque revision/type |
| 1 | Filename (`.wav` effect; `.mp3`/`.xmi` music) |
| 2 | Opaque flags/selection |
| 3 | Active period: 0 always, 1 day, 2 night |
| 4 | Authored gain |
| 5, 6 | Fade-in/out milliseconds |
| 7 | Loop mode: documented 0 continuous, 1 delayed repeat; other values exist |
| 8, 9, 10 | X, Y, Z |
| 11 | Full-volume radius |
| 12 | Maximum audible distance |
| 13 | Randomized-location distance |
| 14 | Activation range |
| 15, 16 | Minimum/maximum repeat delay, milliseconds |
| 17 | XMI sequence index |
| 18 | Echo/effects level |
| 19 | Subject to the environment-sounds option; absent in older rows |
| 20, 21 | Unknown extensions; preserve |

EQEmu documentation and EQ Sage agree on these core positions. They do not
establish the exact echo scale, trailing flags, or all loop-mode values.
The installed rows include loop values such as 2, 3, 10, 100, -1, and apparent
garbage integers; unknown modes must not silently become rapid repeat loops.

Authored gains exceed 1 in 2,331 rows (maximum 75). Keep the raw value for
diagnostics; user volume sliders are a separate 0–1 range. The first backend
should apply a documented gain cap/limiter rather than let malformed or heavily
boosted emitters clip an unbounded sum. This cap is a compatibility compromise.

One known malformed row is `westkorlach.emt:150`, which has
`88.50.70.00` in its coordinate fields. Reject that row with a bounded diagnostic
and continue loading the zone. Do not repair it by shifting columns.
Line 107 of the same file also contains an invalid `^-1556.00` coordinate.

Anguish's sole row is a useful exact music fixture:

```text
2,anguish.mp3,0,0,1.00,0,0,1,7.42,-14.08,6.84,0.00,0.00,0.00,0.00,5000,5000,0,1.00,0
```

The root MP3 exists (2,013,184 bytes). With all ranges zero, this is a
nonspatial zone music selection, not a zero-radius positional effect. Repeat
after the track completes and the authored delay; do not restart every frame.
The first integration should validate this behavior separately from spatial
WAV emitters, whose zero-range semantics may differ.

`growthplane`, `lavastorm`, `nektulos`, and `powar` have both EMT and legacy EFF
files. Initial policy should prefer a valid EMT list and fall back to EFF only
when EMT is absent, rather than play both lists and duplicate ambience. This is
a proposed client policy, not yet a confirmed native precedence rule. An
existing malformed EMT should produce diagnostics, not silently select stale
legacy content.

## Decoding and platform choice

All inspected WAVs are PCM format 1. Support unsigned 8-bit and signed 16/24-bit
samples, mono and stereo, and arbitrary valid sample rates. Loose WAVs include
the unusual **22,000 Hz** rate, plus 11,025/22,050/44,100/48,000 Hz. Archived
WAVs add 8,000 and 16,000 Hz and many 8-bit files. Never assume 44.1 kHz stereo.
Decode on a worker, resample through the audio backend, and keep source duration
independent of the device sample rate.

No registry source/archive was cached for `rodio`, `cpal`, `symphonia`, `kira`,
`hound`, `lewton`, `rustysynth`, or `bevy_audio` during this audit. Dependencies
will need fetching. Nothing was installed or added to Cargo for this research.

Recommended minimal backend: **Rodio 0.22.2**, defaults disabled, with
`playback`, `wav`, `mp3`, and optionally `tracing`. The inspected version requires
Rust 1.87, below this workspace's 1.90 requirement; it uses CPAL 0.17 and
Symphonia 0.5.5. WAV enables both the RIFF demuxer and PCM decoder. Do not enable
the default recording feature or unrelated formats. Use its current
`DeviceSinkBuilder`/`MixerDeviceSink`/`Player` API; older examples use renamed
`OutputStream`/`Sink` types.

Rodio provides decoding, mixing, playback handles, looping, seeking and volume.
Keep EQ emitter selection/timers and authored distance gain in OpenEQ so they
can be tested without a device. Its `SpatialPlayer` applies its own attenuation;
do not feed EQ distances into it and then accidentally attenuate a second time.
For the bounded first slice, centered ambience and explicitly computed
full-radius/max-radius gain are adequate; directional panning is a subsequent
step unless a small stereo-source adapter is included and verified.

Alternatives reviewed:

| Library | Tradeoff |
| --- | --- |
| Kira 0.12.5 | Game-oriented tracks, fades and spatial audio; uses CPAL 0.18 and Symphonia 0.6, adds a second glam version. Good if spatial tracks are required immediately. Disable default realtime/D-Bus and unrelated codecs; WAV needs both `wav` and `pcm`. |
| Direct CPAL + Symphonia | Maximum mixer control, but requires writing the voice/mixing/resampling layer before ambience works. |
| Bevy audio | Not currently enabled; adopting it ties audio asset loading and lifetime to Bevy's audio ECS. Check the exact 0.19 dependency graph rather than assuming latest `bevy_audio` is compatible. |
| Hound | Simple PCM WAV decode; no MP3, mixer or device output. |

CPAL's native paths cover CoreAudio on macOS, WASAPI on Windows, and ALSA on
Linux. Linux playback builds need ALSA development headers (`libasound2-dev` on
Debian/Ubuntu); a headless/no-playback feature should avoid that dependency.
Kira's current CPAL 0.18 branch documents macOS 14.2 as its minimum; do not
assume all backend versions have identical platform floors. A missing/default
device failure must log once and retain a functioning silent client.

Rodio and CPAL are MIT/Apache-compatible; Symphonia is MPL-2.0. Keep dependency
license notices. Do not add the original client's Windows Miles DLLs as a
runtime dependency. FFmpeg and `afinfo` are installed here and useful for silent
format inspection, but should not be required to play the game.

### XMI is a separate synthesis problem

Update2026-09-30: this section is the original design research. The bounded
parser, native ordinal selection,384/389-sequence scheduler and macOS offline
synthesis backend are now implemented; see [AUDIO_RUNTIME.md](AUDIO_RUNTIME.md)
and [XMI_NATIVE_SELECTION.md](XMI_NATIVE_SELECTION.md) for current behavior.
Loop/SysEx support and original timbre fidelity remain open.

`gfaydark.xmi` is a 28,062-byte IFF container beginning `FORM XDIR`, followed by
`CAT XMID`; it contains six `EVNT` sequences. `qeynos.xmi` is 107,184 bytes and
contains thirteen. These are event sequences, not compressed waveforms.
Standard WAV/MP3 decoders cannot play them.

A later XMI implementation needs bounded IFF chunk parsing, XMIDI event timing,
note durations, tempo, controllers/loops and sequence selection, then synthesis.
EQ Sage calls a WASM XMI-to-MIDI converter and separately synthesizes the MIDI;
the converter alone is not an audio backend. Verify XMI sequence numbering
against original EFF selections before resolving positive IDs.

RustySynth 1.3.6 is a plausible pure-Rust synthesizer: MIT licensed, no external
dependencies, MIDI input plus SoundFont2, offline or live rendering. **No SF2,
SFZ, DLS, MLS, WSF or BNK files were found in the installed tree.** A synthesis
bank must therefore be supplied separately under an appropriate license, or
selected by the user. Do not fetch/bundle a random soundfont or claim native
instrument parity. Until then, skip XMI music with one diagnostic per track
while continuing WAV effects. Do not replace it with an unrelated MP3.

## Bounded implementation contract

1. Add asset-only loaders for EFF banks/records, EMT rows, the two filename
   tables, and indexed WAV/MP3 lookup. Return normalized supported emitters plus
   explicit unsupported/malformed diagnostics. Preserve source file, record
   number, raw kind/flags and unresolved IDs. Parse metadata even with audio off.
2. Add a small audio service owned by the game thread, with a real backend and
   a null backend. Its commands are `set_zone(generation, emitters)`,
   `set_listener(scene_position, orientation)`, `set_world_time(...)`,
   `set_levels(...)`, `stop_zone(...)`, and later `play_event(...)`. The output
   handle must outlive all voices. If it is not `Send`, use Bevy's non-send
   resource path rather than forcing it across threads.
3. First enable confirmed classic kind-0 ambience, kind-1 MP3 selections, and
   well-formed EMT WAV/MP3 emitters with known modes. This provides a useful
   running slice while legacy kinds 2/3, XMI, opaque flags and reverb stay
   visible as unsupported. Expand each through evidence, not inferred enum
   equivalence. Do not label the milestone full original-client audio parity.
4. Use scene-space coordinates consistently. EFF positions already correspond
   to source geometry; convert network listener positions once through
   `server_point_to_scene`. Verify a known asymmetric EMT position before
   applying the same convention there. Viewer mode can use the camera; live
   gameplay should use the player listener position and camera orientation.
5. Keep one music selection active, plus at most one outgoing track during a
   crossfade. Equal tracks in adjacent regions reuse the current player. Use
   stable emitter IDs and hysteresis at region boundaries. Positional loops
   stop/fade outside range; delayed one-shots have at most one outstanding
   occurrence. Compute distance gain once. Linear gain from full-volume radius
   to max distance is an explicit first-pass curve, not verified native rolloff.
6. Start with explicit bounds: 4,096 rows/zone, 32 effect voices, 2 music voices,
   64 MiB decoded-effect cache, and 4 decode jobs. Stream music. Bound individual
   source size/decoded duration and archive residency; do not retain all 125 MB
   of sound banks merely to build an index. Prioritize nearby audible effects
   and release least-recently-used inactive buffers. Never read/decompress files
   on the real-time audio callback.
7. Tag loads, timers and voices with the scene generation. Zoning, same-zone
   re-entry, disconnect, character switch and app shutdown stop old voices and
   invalidate pending decodes. A late completion must not start sound in the
   next zone. Despawn later cancels sounds attached to that entity.

Volume API: persistent `master`, `music`, `effects`, and `ambience` values in
`[0,1]`, plus `muted` and `environment_enabled`. Keep authored gains separate
from user values. EMT `isEnvSound` controls whether the environment option can
disable that emitter; it does not identify music. Smooth ordinary level changes;
mute should immediately reach zero. Both settings and backend status should be
available to the options UI without exposing decoder details in gameplay.

Provide an explicit no-audio switch for smoke runs and a null backend that never
opens a device. No-device/decode/missing-file failures are nonfatal and counted
once per source/zone, rather than logged every frame. Decoding and scheduling
can be verified silently; actual listening is a separate manual check.

## Verification and remaining work

Portable tests should use synthetic metadata and generated tiny PCM samples:
84-byte boundaries, finite positions/ranges, sparse/global versus bank IDs,
negative MP3 off-by-one, LF/CRLF tables, all supported EMT row lengths, malformed
rows, unknown modes, range enter/leave, day/night selection, same-track region
handoff, repeat deadlines, voice limits and stale generation cleanup. Check
8/16/24-bit PCM and 22,000 Hz input; WAV and MP3 decode tests do not need output.

Optional original-asset tests should assert the PoK/GFay/Anguish fixtures above,
resolve archive-only ambience, enumerate XMI sequence counts without playing,
and use silent backend snapshots to prove correct track selection and cleanup.
An offline render can measure duration/channel energy/fades without speakers;
do not commit its proprietary audio output. Device smoke tests can initialize
and immediately close a stream at zero master gain, only when playback testing
is intended. No such test was run during this research.

Later event integration has two existing inputs: spell definitions retain
sound IDs, and `animationsounds.txt` maps keys such as `STND_BA_1_DOG` to IDs
through the global sound table. Trigger sounds from discrete accepted server
events or actual animation phase crossings, never once per pose packet/frame.
Footsteps need movement/animation/material policy; combat sounds need duplicate
event suppression; persistent spell sounds need cancellation/despawn cleanup.
These are separate from ambience loading and should have independent tests.

Open compatibility questions: native EMT revision/extension semantics; legacy
kinds 2/3 and attenuation; EAL defaults; exact loop counts/timer anchoring;
priority among overlapping music regions; weather/indoor flags; XMI sequence
numbering/controllers and synthesis bank; stereo positional behavior, echo and
occlusion; native archive/EMT-versus-EFF precedence.

## Sources and reproducibility

The format tables distinguish original-file observations from public-reader
interpretations. Public implementations are references, not proof where they
conflict with installed assets:

- [EQEmu emitter guide](https://github.com/EQEmu/eqemu-docs-v2/blob/main/docs/client/eqgzi/sound-howto.md)
- [EQEmu sound/archive list](https://github.com/EQEmu/eqemu-docs-v2/blob/main/docs/client/guides/sounds.md)
- [LanternExtractor EFF reader](https://github.com/LanternEQ/LanternExtractor/blob/main/LanternExtractor/EQ/Sound/EffSounds.cs)
  and [environment levels](https://github.com/LanternEQ/LanternExtractor/blob/main/LanternExtractor/EQ/Sound/EnvAudio.cs)
- [EQ Sage EFF/EMT model](https://github.com/knervous/eqsage/blob/master/sage/lib/s3d/sound/sound.js)
  and [audio/archive/XMI pipeline](https://github.com/knervous/eqsage/blob/master/src/components/sound/AudioController.js)
- [Quail archive audio inventory](https://github.com/xackery/quail/blob/main/scripts/audiodump/main.go)
- [EQLib sound manager declarations](https://github.com/macroquest/eqlib/blob/emu-rof2/include/eqlib/game/EQClasses.h)
- [Rodio 0.22.2 manifest](https://github.com/RustAudio/rodio/blob/v0.22.2/Cargo.toml),
  [CPAL 0.17.3 platforms](https://github.com/RustAudio/cpal/blob/v0.17.3/README.md),
  [Kira 0.12.5 manifest](https://github.com/tesselode/kira/blob/v0.12.5/crates/kira/Cargo.toml),
  [RustySynth](https://github.com/sinshu/rustysynth)

Original-file SHA-256 fingerprints (assets remain outside the repository):

| File | SHA-256 |
| --- | --- |
| `poknowledge_sounds.eff` | `b7fbef5d2b5fb72e68c7304e936ac02bd7d16a4e8da16becdbe1fc6aef6f35a8` |
| `poknowledge_sndbnk.eff` | `c79f27c572c623e4c85e3c5f3634f58351c2d4de65d7f6dd1fcc3af19183942d` |
| `gfaydark_sounds.eff` | `b4eeed6c6c802d04f80d1d9aba147077c4dac12e8bdee25dac12e654b821dc30` |
| `anguish.emt` | `31eec7b4b0f27ac51f971ff29baf8b309b7251c9dd14636a60a673cabfdf065d` |
| `mp3index.txt` | `408c6e9ede30f8aedc3be358348dc55cab76bb46da4ef6e56e48a02323d0ba9f` |
| `defaults.eal` | `46fa29f4d83c229124ed99bf594ee946ad6acf7064513ad4c6b6aa95f2778564` |
