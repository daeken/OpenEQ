# XMI music: verified structure and synthesis plan

Original research snapshot: 2026-09-29, using the original client installed at
`/Users/daeken/EverQuest`, its Miles DLL, and the local macOS SDK. This was static,
silent research: no DLL execution, synthesizer initialization, audio device,
playback, bank download, or proprietary asset copied into the repository.

The bounded container/event parser is now implemented. The installed Miles
library establishes a default clock of 120 ticks per second. Subsequent
[native lookup research](XMI_NATIVE_SELECTION.md) establishes that nonnegative
EFF/EMT music selectors map directly to zero-based container ordinals, including
zero. The native four-slot loop interpreter is implemented; arbitrary branch
execution remains gated.

Current audio metadata retains `AudioReference::XmiSequence { file, sequence }`.
The `sequence` value is an authored selector; native lookup passes it unchanged
as the selected file's zero-based container ordinal. See [audio assets](AUDIO_PLAN.md)
and [current runtime](AUDIO_RUNTIME.md).

The pure parser was implemented and checked on 2026-09-30; executed facts are
recorded below. Selector statements in the original research are historical;
subsequent native lookup research is separate from this parser's container
ordinal API. Parsing does not enable playback or select a zone's music.

## Implemented parser and executed validation, 2026-09-30

`openeq_assets::audio::xmi::XmiFile::parse` is a pure owned-data decoder. It
returns source-ordered sequences with a distinct `XmiSequenceOrdinal`, raw
optional TIMB pairs, optional RBRN records and validated target event indices,
absolute EVNT file ranges, and events with EVNT-relative start/status offsets,
additive delays, absolute u64 ticks and note-on duration. `note_end_tick()`
exposes checked authored tick+duration without creating a note-off, choosing an
overlap policy or moving the next event's time. Equal-tick events remain in
source order. EOT is retained and its one optional internal zero is recorded.

Every channel shape is represented independently. Controllers, tempo/other
meta events and both SysEx statuses preserve their exact payload values;
extended control flow is not interpreted. Raw timbre bytes and duplicate branch
marker IDs are retained without assigning instrument or branch semantics.
Unknown leaf chunks retain scope, tag, absolute header offset and payload.
The parser never searches their contents for chunk signatures.

The reader enforces the documented FORM XDIR / CAT XMID / FORM XMID nesting
without generic recursion. Required chunks, optional table duplicates,
sequence counts, parent extents, table extents, four-byte VLQs, data-byte status
bits and EOT tails are checked. Odd IFF padding must exist inside the declared
parent, but its value is opaque. Errors report an absolute file offset and the
sequence ordinal when available. Limits are 4 MiB source, 256 sequences,
65,536 events per sequence, 262,144 events per file, 4,096 records per table and
16,384 chunks per file; tick additions and note end times use checked u64
arithmetic. Unknown nested container types are rejected rather than recursively
interpreted or flattened.

Eleven generated-byte portable tests pass. They cover source order, additive
`7F 01` delay 128, overlap/zero duration without generated releases, all channel
shapes, raw tempo/SysEx/controller values, exact table widths, branch targets
at preceding delay bytes, invalid interior/status/padding targets, EOT cases,
maximum/unterminated VLQs, every truncated file prefix, parent length failures,
duplicate/missing chunks, count mismatch, unknown leaves and all resource
limits, including explicit zero-delay provenance without changing additive ticks.
Notes whose durations extend beyond EOT remain represented; cleanup is
a scheduler concern, not a silently truncated parser duration.

The opt-in test `audio::xmi::tests::original_xmi_container_and_sequence_sweep`
passed against `EQ_DIR=/Users/daeken/EverQuest`. It independently reproduced all
79 containers, 389 sequences, 548,617 events, 387 TIMB chunks/4,392 pairs,
51 RBRN chunks/58 records and 205 internal EOT padding bytes. Event-type totals,
83 zero-duration notes, 1,772 tempo events, 22 SysEx events, maximum ticks/duration/
event count/EVNT size, and the named fixture timings below match the research.
The Befallen ordinal 11 branch at offset 765 resolves to delay 8 followed by the
pitch-bend status at 766. All observed note end ticks are at/before EOT.

Strict Clippy passed for the assets library and tests. No original asset bytes
were copied into the repository. Tests neither open an
audio device nor initialize a synthesizer. Native control interpretation,
scheduling, synthesis and runtime playback are subsequent work.

## Implemented bounded scheduler, 2026-09-30

`openeq::audio::xmi::schedule` provides a pure `XmiScheduler`, explicit
`SampleClock`, and bounded `preflight` report before any output. Source metadata
remains observable by source index; MIDI messages expose status/data bytes for
a separate synthesizer adapter. A scheduler owns only the immutable selected
sequence, 32 active slots, a release heap bounded by those slots and 16 remembered
sustain values. Pull batches have a 512-event maximum. Source-event count and
per-call work stay bounded; repeats are never expanded into an event vector.

The scheduler follows [native note-walker evidence](XMI_NATIVE_SELECTION.md):
duration expiries precede authored events at equal ticks, simultaneous expiries
follow ascending active-slot order, and each note-on uses the first free slot.
Overlapping same-key notes retain separate identities and emit every native
note-off rather than suppressing an older release. Zero-duration notes release
at the next tick, except when EOT stops them sooner. Notes beyond EOT are stopped
at EOT. Cleanup releases active slots in ascending order, then emits CC64=0 on
channels whose remembered sustain is at least 64; it does not invent a blanket
CC120 reset. Cancellation drops all pending source/releases and returns at most
48 cleanup messages for the caller's current render frame, then remains inert.

Sample frames are computed as checked `floor(tick * sample_rate / clock_rate)`
using a u128 intermediate. This avoids repeated rounding drift at 44.1 kHz and
48 kHz. Same-frame events retain their tick/source order. The default clock is
the observed Miles 120 Hz; an explicit nonzero clock rate can be supplied.

Controller 120 follows common MIDI output, matching the new native dispatch
evidence. Controller 108 also follows common output under EQ's observed lack of
a prefix callback. The unimplemented native controls 106, 109, 110, 111, 115, 118, 119 and
unsupported SysEx framing are rejected before emitting any events. Complete
F0..F7 packets up to 1536 bytes are forwarded unchanged to the offline synth. Passive RBRN metadata alone
does not imply a branch; executing controller 109 remains unsupported. Rejected
controls are not relabelled as ordinary MIDI or silently skipped. Preflight
reports the first source index and occurrence count for each bounded issue kind.

The initial nine portable tests passed for clock drift/overflow, native release ordering,
slot reuse, same-key overlap, zero duration, EOT/cancellation/sustain cleanup,
source metadata/channel shapes, unsupported controls and explicit zero delays,
capacity/work guards and
batch partition independence. The ignored `original_xmi_scheduler_coverage`
audit passed against the installed 79 files/389 sequences without a synthesizer
or device. It fully scheduled 384 straight-through sequences into 927,156
ordered outputs; peak linear active-note occupancy was 31 of 32 slots. The
loop extension below admits four more sequences, and the later complete-packet
SysEx integration admits `thedeep.xmi` ordinal 0, reaching **389/389**. The
current corpus audit schedules 385 straight-through sequences into 929,693
ordered outputs, with the same peak 31/32 linear occupancy. Loop sequences use
separate bounded/native comparisons instead of blindly draining an infinite
sequence in the corpus sweep.

These are scheduling checks, not a claim of original timbre parity or listening
validation. The renderer/stream integration and system synthesizer remain
separate modules and carry their own executed evidence.
Strict Clippy passed for the assets/application libraries and tests, and an
independent comparison with the native note/controller traces found no blocking
scheduler discrepancy. A remaining non-corpus timing question is whether an
explicit zero delay byte causes a native one-tick wait. The parser preserves
additive ticks and records any literal zero among delay bytes, including zeros
mixed with positive delays. Preflight rejects these sequences with
`UnsupportedZeroDelay`; zero-duration notes still use the verified one-tick
minimum. The original parser sweep asserts that none of the 79 installed files
contains a zero delay byte, so this gate does not reduce original coverage.

## Implemented native loop execution, 2026-10-01

The interpreter consumes CC116/117 using the four sequence-wide slots proven in
[XMI_NATIVE_LOOPS.md](XMI_NATIVE_LOOPS.md). It restarts at CC116 itself, chooses
the first free slot on start and highest occupied slot on NEXT, ignores a fifth
start, and preserves native count-zero infinity and the 64 NEXT threshold.
Unmatched NEXT values below 64 fail safely instead of reproducing the native
out-of-bounds write. Every other unknown controller/callback/branch path remains
under its existing gate; passive RBRN records remain immutable metadata.

Source records, their authored ticks and offsets never change. A separate
checked execution tick advances across repeats; restart skips only the delay
before CC116. Note expirations continue across jumps and retain release-before-
source ties and first-free-slot identities. Every note occurrence has a distinct
identity even when its source index and active slot are reused. Runtime always
enforces 32 active notes; preflight's linear occupancy cannot prove a loop's
peak or endpoint. Consequently loop duration metadata is unknown.

Each scheduler pull executes at most one source command, release or cleanup.
Consumed controls remain visible as source events without MIDI output. At most
65,536 source events may execute at one tick, persisting across pull batches;
the PCM worker also limits all scheduled events to 131,072 per output block.
These explicit errors bound control-only loops and dense advancing loops without
expanding repeats. Cancellation clears loop state and pending releases and
attempts all note/pedal cleanup messages, including after render/guard failure
or receiver cancellation.

The worker keeps its existing 30-minute playback policy: known longer linear
sequences are rejected before rendering; loops stop and log at that runtime cap.
Natural finite EOT or the policy cap receives the existing two-second release
tail. Pure scheduler conformance has no playback-duration cap, so it verifies
the full roughly 236.66-minute finite original trace offline.

The focused synthetic scheduler/stream suite passes 28 tests, including all
positive native counts 1–127, zero-count infinity, cross-channel nesting, four-
slot capacity, delay placement, overlapping releases, unique identities,
batch independence, malformed breaks, clock overflow and bounded cancellation.
Fake-synth tests cover EOT beyond the source duration, unknown stream duration,
runtime cutoff, control-only/per-block work limits, cleanup failures and receiver
drop; none opens an audio device. The two ignored original scheduler audits
also pass: both finite original copies reproduce 261,313 MIDI outputs through
tick 1,703,932, and both infinite copies reproduce 14,195 outputs through tick
69,686, including complete same-tick restarted events. Canonical trace digests
and source/control counts match the isolated native execution. This establishes
the captured scheduling behavior, not original timbre parity or listening quality.

## Container structure

All 79 installed root XMI files walked successfully and contain 389 sequences:

```text
FORM XDIR
  INFO: u16 little-endian sequence count
CAT  XMID
  FORM XMID
    TIMB (optional)
    RBRN (optional)
    EVNT
  FORM XMID ...
```

Each chunk begins with a four-byte tag and a **big-endian u32 payload length**.
The length excludes the eight-byte header. Container payloads start with their
four-byte type. Walk children inside their parent's declared end, accounting for
IFF even-byte padding separately from payload length. Preserve/skip unknown
chunks by validated lengths. Never scan for magic strings inside arbitrary data.

There are 79 `INFO`, 389 `EVNT`, 387 `TIMB`, and 51 `RBRN` chunks. Every `INFO`
count matches the number of nested sequence forms. Installed containers have
1–24 sequences; `gl.xmi` has 24. `gfaydark.xmi` has six and `qeynos.xmi` has 13.
Reject duplicate required chunks, impossible nesting, overlapping/truncated
lengths, and count mismatches with contextual errors.

`TIMB` has a little-endian u16 count followed by that many two-byte entries.
All observed payload sizes equal `2 + count * 2`. The 4,392 entries have a first
byte in 0–127 and a second byte of 0 (3,606 entries) or 127 (786 entries).
Preserve these as raw timbre pairs. A program/bank interpretation is plausible;
the complete instrument/percussion mapping is not established by this audit.

`RBRN` has a little-endian u16 count followed by six-byte records:
`u16 marker_id, u32 EVNT_payload_offset`, both little endian. All 58 records fit
that layout and point to valid event boundaries. **A boundary may begin with
delay bytes**, not a status byte. For example, `befallen.xmi` ordinal 11 has
marker 0 at offset 765, beginning `08 E1 7F 7F`: delay 8, then pitch bend.
Validate against parsed event-start offsets, including the preceding delay.

## EVNT grammar

A temporary decoder parsed all **548,617 events in 389 sequences** without an
event error, using this grammar:

1. Sum consecutive bytes below `0x80` as the delay before the next explicit
   status byte. This is additive tick encoding, **not MIDI delta-time VLQ**.
   There is no running-status interpretation at this boundary.
2. Channel messages `8n`, `9n`, `An`, `Bn`, and `En` have two data bytes; `Cn`
   and `Dn` have one. Require data bytes below `0x80`.
3. Every `9n` note-on additionally carries a base-128, most-significant-group-
   first VLQ duration, after note and velocity. Duration does not advance the
   event cursor's time; it establishes that note's future release tick.
4. `FF` has a meta subtype, VLQ payload length, and payload. `F0`/`F7` have a
   VLQ payload length and payload. Bound lengths before slicing or allocating.
5. `FF 2F 00` ends the sequence. In 205 originals, exactly one zero byte remains
   **inside the EVNT payload**; in 184, no bytes remain. Accept that single
   internal zero padding byte. Do not interpret it as an unfinished delay, and
   do not accept arbitrary trailing bytes after EOT.

Observed channel events: 392,655 note-ons, 82,102 pitch bends, 60,318 controller
changes, and 4,546 program changes. There are 8,996 meta/SysEx events. No explicit
note-off, polyphonic pressure, or channel pressure events occurred in this set;
the parser can still represent those standard channel message shapes.

All pending authored note end ticks were at or before EOT. There are 83
zero-duration notes; subsequent native tracing establishes a one-tick minimum
before expiry, unless EOT stops the note sooner. The longest duration is
6,442 ticks, longest sequence is 39,431 ticks, largest EVNT is 29,918 bytes, and
largest event count is 6,896 (`kaladima.xmi`, ordinal 7). Observed VLQs require
at most two bytes; a bounded four-byte decoder accommodates normal MIDI-style
length/duration encoding without allowing overflow or unterminated reads.

Keep source order for same-tick events and generate releases using stable note
identities. Repeated/overlapping notes on the same channel and key need an
explicit policy: a naive late release can silence a newer note. Zero-duration
note-on/off order, sustain, controller resets, end-of-track cleanup, and any
native note-overlap behavior need dedicated scheduler tests.

## Timing evidence from the native Miles library

The installed `mss32.dll` is PE32, image base `0x21100000`, 349,696 bytes,
SHA-256 `fa78565a1e07df215532c611a5089256fcc1d81c1ae808eca66614c3ab77f9e0`.
Addresses below are absolute virtual addresses; subtract the image base for an
RVA. Inspection used LLVM's disassembler without executing the library.

| Native location | Observation |
| --- | --- |
| `0x211010a0`, exported `AIL_get_preference` | Returns the indexed u32 from table `0x21151648` |
| `0x211012e0`, preference initialization | At `0x2110136c`, sets table element 3 (`0x21151654`) to `0x78`, decimal **120** |
| `0x21117fbc`–`0x21117fc7` | Passes that element to `AIL_set_timer_frequency` for the MIDI driver's timer |
| `0x2110b5b0`, exported `AIL_sequence_ms_position` | Calls internal routine `0x211192b0` |
| `0x211192e3`–`0x211192ee` | Duration walker adds bytes below `0x80` to its tick count |
| `0x2111932d`–`0x2111934f` | Skips meta payloads by length; only EOT changes this walk's control flow |
| `0x21119376`–`0x2111938a` | Converts total ticks to nominal milliseconds as `ticks * 1000 / preference[3]` |

Thus, the default nominal time is `tick / 120` seconds. For synthesis, compute
sample positions using a checked integer rational accumulator, rather than
rounding each tick to 8 ms. At 48 kHz, a tick is exactly 400 sample frames; at
44.1 kHz it is 367.5. Preserve long-run timing at either output rate.

There are 1,772 tempo meta events and 81 sequences with multiple tempo events.
Retain these metadata values, but **do not apply a normal SMF tempo map to the
already timed XMI delays**. The Miles duration walker does not do that. This
does not establish whether EverQuest overrides the Miles timer preference or
uses the separate sequence-tempo API, nor does it fully explain native beat/bar
callbacks. Native elapsed playback under those overrides remains follow-up work.

Useful fixtures, with zero-based **container ordinals**, not EQ selectors:

| File / ordinal | EOT tick | Default nominal seconds | Tempo event count |
| --- | ---: | ---: | ---: |
| `gfaydark.xmi` / 0 | 1,767 | 14.725 | 7 |
| `gfaydark.xmi` / 1 and 2 | 2,351 | 19.591666… | 14 each |
| `gfaydark.xmi` / 3 | 1,767 | 14.725 | 7 |
| `gfaydark.xmi` / 4 | 15,188 | 126.566666… | 68 |
| `gfaydark.xmi` / 5 | 4,596 | 38.3 | 1 |
| `griegsend.xmi` / 0 | 39,431 | 328.591666… | 1 |

## Original selector audit and subsequent native resolution

The mismatched selector observations below motivated native tracing. The
subsequent [native selection evidence](XMI_NATIVE_SELECTION.md) resolves the
mapping directly: raw nonnegative music selectors are zero-based, separate
EFF day/night kinds matter, and out-of-range ordinals can be authored requests
that native initialization rejects. The original absence of a mapping was a
research limit, not permission to invent an offset or fallback.

The Miles sequence finder at `0x2111acb0`, called by the internal implementation
of `AIL_init_sequence`, selects nested `FORM XMID` records using a **zero-based
ordinal**. At `0x2111ad68`–`0x2111ad6d`, it returns the current form when the
pre-decrement index is zero. This proves the Miles API's indexing convention,
not the mapping used by EverQuest's EFF or EMT data.

The asset set rejects a universal guessed mapping:

| Zone | Container count | Positive EFF selectors observed |
| --- | ---: | --- |
| Greater Faydark | 6 | 2, 3, 4, 5 |
| Blackburrow | 12 | 1, 5, 10, 11, 12 |
| Nektulos | 1 | 1 |
| Butcherblock | 2 | **1 through 7** |
| Bothunder | No named XMI present | 163 |

Other zones have positive selectors but no correspondingly named XMI. Preserve
the raw selector and investigate native file/sequence lookup, including any
fallbacks or aliases, before automatic selection. The mere presence of
`gl.xmi` is not evidence for a particular global mapping. A diagnostic tool can
explicitly select a file and zero-based ordinal without claiming zone fidelity.

Controller values in the originals include these extended cases:

| Controller | Values (occurrence counts) |
| ---: | --- |
| 108 | 0 (59), 2 (3) |
| 116 | 0 (2), 127 (2) |
| 117 | 127 (4) |
| 120 | 0 (51), 3 (3), 64 (4) |

Controller 120 values correlate with RBRN marker IDs, but subsequent native
dispatch tracing disproves that correlation as a branch rule: controller 120
uses common MIDI output; controller 109 executes `AIL_branch_index`.
Controllers 116/117 occur in
`templeveeshan.xmi` ordinal 0, `thurgadina.xmi` ordinals 0 and 5, and
`thurgadinb.xmi` ordinal 0. The later native loop evidence and bounded interpreter
above resolve those controls. Branch tables remain preserved; unimplemented
branch execution is still rejected explicitly.

The 22 SysEx events are in `thedeep.xmi`, with Roland manufacturer `0x41`
messages including GS-like initialization. Keep payloads and diagnose unsupported
ones; neither generic GM playback nor the available macOS bank guarantees the
same native instrument setup.

## Complete-packet SysEx integration, October 1

`XMI_NATIVE_SYSEX.md` establishes native packet bytes, tick ordering, source
offsets and the 1536-byte output preference. The scheduler admits only complete
F0 packets ending in F7 with seven-bit interior bytes, retains immutable source
payloads, and emits an explicit SysEx event at the authored position. The stream
reuses fixed bounded packet storage and calls the worker-owned synth before
rendering the next samples. F7 continuations, malformed and oversized records
remain rejected before construction; packets are not truncated or rewritten.

Synthetic tests cover mixed source order, expiring notes, native loop reentry,
small/large batches, cancellation and cleanup after a failed SysEx call. The Deep
passes native ordinary, packet and combined trace digests at all three batch
sizes, and its full 199.375-second sequence plus release tail renders finite
nonzero PCM entirely to memory. API acceptance does not prove Roland GS semantic
effects or original instrument timbres in the installed macOS bank.

## Already-installed synthesis option

This Mac contains:

```text
/System/Library/Components/CoreAudio.component/Contents/Resources/gs_instruments.dls
```

It is a 1,996,068-byte RIFF `DLS ` bank with 235 instruments in `colh` and
copyright metadata naming Roland Corporation (1997). SHA-256:
`739d277474bddeb120372625b70c83c653faaee44de26c3191848cc6c64bfb74`.
No synthesis bank was found in the original EverQuest tree. No other SF2/SF3/
SFZ/DLS bank was found in the scoped system audio and package-share directories.
This is an inventory of this machine, not a guarantee about other installations.

The useful Mac route is the **installed OS synthesizer API**, leaving its bank
in place. Do not bundle, convert, upload, or redistribute the system bank.
The local AudioToolbox SDK documents:

- `kAudioUnitSubType_DLSSynth` (`'dls '`): a desktop multi-timbral music device
  supporting DLS/SoundFont banks, GM, and basic GS extensions.
- `MusicDeviceMIDIEvent`: channel messages with sample-frame offsets when sent
  from the unit's render thread. Otherwise the offset must be zero.
- `AudioUnitRender`: rendering into caller-supplied buffers with a valid,
  sequential sample-time timestamp. A dedicated synthesis worker can render
  PCM without connecting an output unit or opening a hardware device.
- `kMusicDeviceProperty_SoundBankURL`: **read-only for DLSMusicDevice**, read/write
  for AUMIDISynth (`'msyn'`). Do not attempt to configure DLSSynth by writing this
  property. Prefer its installed default bank, and query/report availability.

No AudioUnit was instantiated in this research. First integration should verify
component availability, output format and bounded rendering using an opt-in
offline check. Split blocks at event boundaries if sample-offset scheduling is
not supported by the chosen unit. Dispose the unit on its owning worker.

For cross-platform synthesis, RustySynth is an SF2-only option; it does **not**
read the installed DLS directly. A user-provided, appropriately licensed SF2 or
a separately reviewed redistributable bank is needed for that route. Bank
licensing is independent of synthesizer licensing. No bank download is required
for the Mac API experiment, and no cross-platform or original timbre parity is
claimed.

## Implementation order and validation

1. Add a pure asset parser returning sequences, timbre pairs, branch metadata,
   and source-ordered events with absolute integer ticks and note durations.
   Keep authored music selectors in a separate type from container ordinals.
   Proposed initial bounds: 4 MiB XMI source, 256 sequences, 65,536 events per
   sequence and 262,144 per file, 4,096 timbres/branch records, four-byte VLQs,
   bounded chunk nesting, and checked u64 tick arithmetic. These exceed the
   observed originals while bounding allocation and work.
2. Add a deterministic scheduler with generated note releases, rational
   tick-to-sample conversion, stable same-tick ordering, sustain/reset handling,
   and a bounded active-note/event queue. Preserve unsupported control-flow
   metadata; reject unsupported playback explicitly. Do not expand repeats into
   an unbounded event list.
3. Implement an optional synthesis adapter on its own worker. Reuse the runtime's
   two-music-voice and bounded PCM-stream model, volume/fades, cancellation tokens,
   and destination generations. Include source/events, synthesis state, producer
   and consumer blocks in memory accounting. Synthesis and bank loading must not
   occur on the output callback or game thread. Underruns remain silent frames.
4. Verify EQ's selector lookup and extended controller behavior, then connect
   XMI to authored zone music selection. Track identity must include the resolved
   sequence, not just the filename. On zone transitions, cancel work, clear
   queued blocks and notes, and prevent an old render from entering a new stream.

Portable tests should use generated bytes: mixed-endian chunks and count errors,
unknown chunks, truncated/overflowing lengths and VLQs, additive delays such as
`7F 01` (=128 ticks), duration releases that do not advance event time, zero
duration, same-note overlap, missing EOT, accepted single EOT padding, invalid
branch targets, and explicit rejection of unimplemented control flow. A fake
synth/event sink tests scheduling, cancellation and fades without a bank/device.
Parser tests must not initialize AudioUnit or any output backend.

Original-asset tests remain ignored/opt-in and read the user's installation.
Check all 79 containers/389 sequences, the fixture ticks above, and bounds without
committing original bytes. An opt-in offline Mac test can render synthetic notes
to memory and check finite samples, timing and release; it must never construct
a hardware output unit. Actual listening is a separate manual check.

## Reproducible fixture identity and sources

| Installed file | Bytes | SHA-256 |
| --- | ---: | --- |
| `gfaydark.xmi` | 28,062 | `c0d73a20701bd6f0d4d709b4f688f09d3c8cafa7d7c8801c913e25ce776b9e41` |
| `qeynos.xmi` | 107,184 | `12aae448dc2553fc105f6c8cf124965bc7e7c58346aef2bc32732b57de15e184` |
| `butcher.xmi` | 7,942 | `cdd1f15c05c150c3d477245d3529cf4fdc508bd2779c6bba5ef638169b31539d` |
| `gl.xmi` | 121,826 | `a14fd41406d6b776a4b85f8a6f7d4986f1302c5e2ede78cfd9fc634c44b8e76c` |
| `griegsend.xmi` | 8,224 | `940b86d89a8197a7d8a9b82b9c7ebde3cf0ca2432fe8281fad336b8e23475403` |

Container/event facts come from the installed bytes; timing and native ordinal
facts come from the fingerprinted Miles DLL above. Synth API facts come from
`AudioToolbox.framework/Headers/{AUComponent.h,MusicDevice.h,AudioUnitProperties.h}`
in the local Command Line Tools macOS SDK (DLSSynth docs around AUComponent:339,
render docs around 1543, MIDI event docs around MusicDevice:185, and bank URL
access around AudioUnitProperties:3722). Earlier public audio references are
linked in [AUDIO_PLAN.md](AUDIO_PLAN.md); none establishes the unresolved EQ
selector mapping described here.
