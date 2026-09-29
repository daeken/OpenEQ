# Zone audio runtime

The client now plays original zone WAV ambience and MP3 music. Asset details and
evidence are in [AUDIO_PLAN.md](AUDIO_PLAN.md). No EverQuest audio is redistributed.

## User controls

Playback is enabled by default. The default master level is 50%, with music and
ambience each at 60% of master. `/audio` shows current levels and output status.

| Command | Result |
| --- | --- |
| `/audio master 0..100` | Overall volume |
| `/audio music 0..100` | Zone music volume |
| `/audio ambience 0..100` | Authored ambient sound volume |
| `/audio effects 0..100` | Saved setting reserved for future event sounds |
| `/audio mute`, `/audio unmute` | Silence/restore without changing channel levels |
| `/audio environment on`, `/audio environment off` | Enable/disable environment-controlled emitters |

Settings persist in `$XDG_CONFIG_HOME/openeq/audio.json`, falling back to
`~/.config/openeq/audio.json`. Writes replace the file atomically. Invalid/future
settings are preserved, and the session uses defaults without overwriting them.
Values must be finite and in range. Muting or setting master to zero takes effect
on the next service update; ordinary volume changes are smoothed.

`--no-audio` never opens an output device. Metadata still loads for silent
scheduling checks. A missing device is nonfatal. Linux playback builds need ALSA
development headers (Debian/Ubuntu: `libasound2-dev`); `cargo build -p openeq
--no-default-features` keeps parsing/decoding but omits the device dependency.

## Current coverage and policies

- Classic EFF kinds 0 (day/night ambience) and 1 (MP3 music). Legacy effect kinds
  2/3 and XMI selections are retained by the parser but omitted from playback.
- EMT WAV/MP3 emitters with known day/night and continuous/delayed-repeat modes.
  Unknown loop modes do not create repeated bursts. Zero-range MP3s such as
  Anguish are zone music; zero-range effects are not turned into global sounds.
- Day is currently 06:00–18:00 from the live zone clock (viewer defaults to noon).
  Exact native transition boundaries and opaque EMT flags remain unverified.
- Source/scene positions are used directly; network poses are not swapped again.
  The listener follows the player's first-person camera even in third person.
- EMT ambient distance gain is linear between full and maximum radii. Classic
  kind-0 ambience is two-dimensional and retains its level inside its trigger
  radius. Music regions
  retain full volume within range, and a small boundary margin avoids chatter.
  These curves are client tuning, not measured native attenuation.
- Classic nonzero effect levels use the documented hundredths-of-decibel
  interpretation. Zero uses unity until EAL defaults are implemented. Authored
  EMT gains are capped at unity separately from user volume, and the summed
  output has a limiter. This avoids unbounded boosts in original EMT files.
- One current music track is selected. Audible current music wins overlapping
  regions, and regions with the same file reuse its playback token. Region
  activation remains separate from shared track identity. At most one outgoing
  track fades alongside it. Crossing a zone boundary stops both immediately.
- Authored EMT fade-in/out applies to music and WAVs. Classic ambient exit uses
  a short client fade. Fading effects count toward the 32-voice bound; the oldest
  fade can end early to make room for a newly audible voice.
- Delayed repeats wait from completion, never accumulate a catch-up queue after
  stalls. Continuous WAV loops reuse decoded buffers. MP3 continuous playback
  reopens after completion, so gapless music looping is not claimed.

## Threads and bounds

The game thread publishes a coalesced desired state: destination generation,
listener, live hour and levels. A dedicated service owns the device and updates
every 20 ms. Loading, disconnect and character/zone changes publish no destination
or a new generation, clearing voices. Every scheduled occurrence gets a unique
token, including same-zone re-entry, and late effect decodes cannot start an old
voice. The service releases players before device shutdown.

Metadata indexing and file/decompression work never run on the render or output
callback. Up to two effect workers and two music workers are allowed. Effects
are decoded once and shared; music workers feed eight 4,096-frame blocks through
a bounded queue. The callback takes available samples without waiting; underruns
insert whole silent frames to preserve stereo alignment. Dropping a source
disconnects a back-pressured worker. Source format is limited to mono/stereo,
8–192 kHz, with WAV PCM 8/16/24-bit and MP3 supported by Rodio/Symphonia.

Bounds: 32 effect voices, two music voices, 64 MiB effect cache, 16 MiB per decoded
effect, 32 MiB runtime source limit, 30 minutes per streamed occurrence. Cached
active effects are not evicted. At most two pending decoded effects can add
another 32 MiB transiently, plus bounded compressed archive/source buffers.
Each music stream uses up to 320 KiB of prepared stereo samples across its queue
and producer/consumer blocks, plus the codec's internal buffers.
Only loose MP3 music currently streams; archived music is skipped. Source/decode
failures are suppressed until the next zone load rather than retried every frame.

Rodio 0.22.2 and CPAL 0.17.3 use MIT/Apache-2.0 licenses; Symphonia 0.5.5 uses
MPL-2.0. Their package license/source notices remain in Cargo's dependencies.
No proprietary Miles runtime or synthesis bank is bundled.

## Verification

All audio verification is silent. Native device initialization/closure was also
verified with only digital silence. Tests cover PCM depths and unusual 22,000 Hz
rates; archive-only PoK/GFay effects; full MP3 decoding for PoK and Anguish;
stereo underruns; offline resampling/mixing, volume and stop; day/night changes;
region activation; same-track region handoff; cooldown timing; voice starvation;
unknown modes; generation cleanup; and original-zone service transitions.
The original-asset tests use `openeq_assets::loader::default_client_dir()`.

```sh
cargo test -p openeq --lib audio:: -- --include-ignored --test-threads=1
cargo test -p openeq-assets audio -- --include-ignored --test-threads=1
cargo check -p openeq --no-default-features --all-targets
```

Speaker listening and native-client acoustic comparison remain manual checks.
Other follow-ups: XMI parsing/synthesis with a licensed user-supplied bank,
original animation/spell/combat sound events, stereo panning, reverb/occlusion,
EAL default levels, native opaque emitter flags, exact fade/priority rules,
device hotplug recovery, and graphical options controls.
