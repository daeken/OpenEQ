# XMI music: verified structure and synthesis plan

Research snapshot: 2026-09-29, using the original client installed at
`/Users/daeken/EverQuest`, its Miles DLL, and the local macOS SDK. This was static,
silent research: no DLL execution, synthesizer initialization, audio device,
playback, bank download, or proprietary asset copied into the repository.

The container and event grammar are now sufficiently understood to implement a
bounded asset parser. The installed Miles library establishes a default clock of
120 ticks per second. **EverQuest's music selector mapping is still unresolved**;
do not enable automatic zone XMI playback by guessing a sequence offset.

Current audio metadata retains `AudioReference::XmiSequence { file, sequence }`.
The `sequence` value is an authored selector, not a verified zero-based container
ordinal. The runtime deliberately skips XMI. See [audio assets](AUDIO_PLAN.md)
and [current runtime](AUDIO_RUNTIME.md).

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

All pending note end ticks were at or before EOT. There are 83 zero-duration
notes, so a renderer must handle immediate releases. The longest duration is
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

## Selectors, branches, and controllers still need work

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

Controller 120 values correlate with RBRN marker IDs. Blindly forwarding it as
GM's “all sound off” would lose this distinction. Controllers 116/117 occur in
`templeveeshan.xmi` ordinal 0, `thurgadina.xmi` ordinals 0 and 5, and
`thurgadinb.xmi` ordinal 0. Preserve these commands and branch tables; establish
their control-flow semantics before implementing looping or branching. Until
then, report unsupported control sequences rather than flattening them silently.
Any eventual loop interpreter needs a per-render event-work limit and a finite
zero-time-jump limit, even when authored playback intentionally repeats forever.

The 22 SysEx events are in `thedeep.xmi`, with Roland manufacturer `0x41`
messages including GS-like initialization. Keep payloads and diagnose unsupported
ones; neither generic GM playback nor the available macOS bank guarantees the
same native instrument setup.

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
