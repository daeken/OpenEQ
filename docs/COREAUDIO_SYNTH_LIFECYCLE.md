# macOS DLSSynth native-call serialization

OpenEQ serializes every native call to its macOS DLSSynth units with one
process-wide mutex: creation, configuration, initialization, MIDI, SysEx,
rendering, uninitialization, and disposal. Each unit still belongs to one
synthesis worker. The native component shares bank references and mutable
waveform-loading state between units; a per-unit owner or a lifecycle-only
lock does not protect those shared objects. Buffer preparation and PCM copying
run outside the native render call's lock, and no audio device is opened.

The first failure below established the lifecycle race. A later full-suite
failure and a small independent reproducer demonstrated that the original
lifecycle-only correction was insufficient; the current boundary covers both.

## Matching crash and binary

Investigated on 2026-10-01, macOS 27.0 build 26A428:

- Crash: `openeq-6debd45368f48bd2-2026-10-01-051256.ips` in
  `~/Library/Logs/DiagnosticReports/`.
- Incident: `DC4186F8-E50C-4DB7-8759-EB9C0BD8EE53`.
- Component: `/System/Library/Components/CoreAudio.component/Contents/MacOS/CoreAudio`.
- Matching **arm64e** UUID: `C6E682F4-92B6-318F-877C-640AB5E1E026`.
- Crash image index 1: base `4631543808`, size `1343488`, component version 1.14.

The installed fat binary contains other architectures with different UUIDs.
All addresses below are offsets in the matching arm64e image, whose text VM
base is zero. Stack frame offsets are return PCs: `0x8682c`, for example,
identifies the call at `0x86828`, not execution of the function beginning at
`0x8682c`.

## Exact assertion and simultaneous release

Thread 20 aborted through `CAAssertRtn`. The component at `0x86814..0x86828`
supplies `DLSClient.cpp`, line **482**, expression
**`mDlsCollection == __null`**. Its acquire routine at `0x867c4` checks this
when the shared reference's count is zero:

```text
0x867d8  load reference.refcount               [reference + 0x10]
0x867dc  skip null check if count != 0
0x867e0  load reference.collection            [reference + 0x08]
0x867e4  assert if collection != null
0x867f4  load collection when first acquired
0x867f8  store reference.collection
0x86800  increment reference.refcount
0x86804  store reference.refcount
0x86828  call CAAssertRtn on the failed branch
```

The same crash captured thread 18 in the shared reference's final release:

```text
0x86848  load reference.refcount
0x86854  decrement count
0x86858  store count                           // already zero
0x8685c  skip destruction if count != 0
0x86860  load reference.collection             // still nonnull
0x86868  call collection destructor            // thread 18 is inside it
0x8687c  free collection allocation
0x86880  clear reference.collection            // not reached yet
```

Relevant frames, from native failure toward the Rust caller:

| Thread | Native frames | OpenEQ caller |
| --- | --- | --- |
| 20, triggered | `CAAssertRtn`, `+0x8682c`, `+0x86678`, `+0x1d780`, `+0x14424`, `+0xcf684`, `+0xd3ca0`, `AudioUnitInitialize` | `Synth::create`, then `offline_synthetic_note_and_cleanup` |
| 18 | `_xzm_free_main`, `+0x7fdb4`, `+0x7fe04`, `+0x5d83c`, `+0x5d9a8`, `+0x8686c`, `+0x1d8a0`, `+0xcf2d8`, `+0xd3db8`, `AudioUnitUninitialize` | `Synth::close`, then `offline_original_sysex_transports` |

This directly places the final release in the zero-count/non-null-pointer
window while another unit acquires the bank. Serializing initialization
alone would leave this initialization-versus-teardown race open.

## Shared data and native lock scope

`0x86598` searches a vector of bank references by their URL's filesystem path.
The `+0x86678` frame is the existing equal-URL entry's acquire call at
`0x86674`; it is not the new-reference allocation path.

The initializer loads the global vector pointer at `+0x1619e0` and stores it
in the unit at `+0x268` (`DLSClient + 0x28`). Both initialization and
uninitialization also update the global instance count at `+0x1619d8` with
ordinary load/add-or-subtract/store instructions. Their locks use
**unit + 0x280**. The lock helper `0x1dd8c` uses the passed object's owner,
recursion count, and semaphore mutex; it is per instance, not a lock around
the shared vector. The two crash threads therefore need an external common
lifecycle lock.

## Creation and property boundaries

The constructor at `0x160d0` zeroes the DLS client's reference, collection,
and vector fields before calling the default-bank URL setter at `0x16190`.
That setter (`0x863bc`) invokes release and acquire virtual methods even
during construction. The constructor's base vtable at `0x155418` resolves
those slots to `0x8682c` and `0x86598`. Release sees null ownership fields;
acquire sees a null vector and returns null at `0x86660`. Thus this inspected
constructor path does not acquire the shared collection. The vector is
attached later by initialization.

Property 37 (`kAudioUnitProperty_OfflineRender`) at `0x150b8..0x150f4` changes
the offline and bank-streaming flags. Its continuation at `0x153f4` checks
the initialized byte at unit `+0x54a`; a constructed unit has that byte zero.
Before initialization, this path does not reload the bank. On an initialized
unit, it **can** call the URL setter at `0x15470`, which releases and
reacquires the bank. Bank-URL/data properties also use the release/acquire
methods. OpenEQ currently sets only stream format (8), maximum frames (14),
and offline render (37) before initialization; this investigation did not
exhaustively trace the framework's generic handlers for properties 8 and 14.

Keeping the whole setup and disposal sequence under the common native gate
protects this boundary. Future sound-bank changes or changes to offline mode
after initialization must use that same gate. These observations do not imply
that arbitrary native property changes are safe during render.

## Lazy waveform loading during MIDI delivery

The later crash `openeq-6debd45368f48bd2-2026-10-01-111833.ips` uses the same
component UUID. Thread 27 (`openeq-xmi-synth`) aborts during
`MusicDeviceMIDIEvent`, while thread 26 renders The Deep through
`AudioUnitRender`. The triggered stack contains component return PCs:

```text
0x85bec -> 0xfe7ac -> 0x800ac -> 0x10b350 -> 0xdc7a8 -> 0xe0370 -> 0xd5ce8
```

The actual assertion call at `0x85be8` names `DlsFile.cpp`, line **456**,
**`GetHeader().IsListType()`**. This is a waveform-header assertion, distinct
from the shared reference-count assertion above. The neighboring assertion at
`0x85c00` checks line **457**,
**`GetHeader().GetSubType() == ChunkType(kChunkType_WaveFileChunk)`**.

The native path at `0xfe77c` checks cached waveform pointer `+0x1d0`. If it is
null, `0xfe798..0xfe7a8` follows the region's owning collection and calls the
waveform loader `0x84b9c`; `0xfe7ac` stores the result. That loader checks the
collection's waveform entry before parsing a missing waveform. The DLS path
at `0x84dac..0x84eac` obtains the bank reader, positions it using `0x18630`,
reads its chunk header using `0x1832c`, then validates the list/wave tags.
This establishes lazy bank reads during MIDI delivery, not merely during
unit initialization. It does not by itself identify every possible native
race or establish that SysEx resets reload the entire bank.

A standalone copy of the lifecycle-only implementation reproduces SIGTRAP
immediately with four workers, eight instances per worker, and all 128 General
MIDI programs per instance. Each unit receives a complete GM System On packet
(`F0 7E 7F 09 01 F7`), then bank/program changes, notes, memory renders,
note-offs and all-sound-off messages. Workers use different program/pitch
orders and alternate 44.1/48 kHz and explicit/drop cleanup. The sole barrier
is at worker startup. No original assets or playback devices are involved.

Both the eight-worker initial probe and the four-worker regression pattern
failed on their first runs, at the neighboring wave-subtype assertion
(`0x85c04` return PC), with the same remaining MIDI stack as the suite crash:

- `openeq-synth-cache-stress-before-2026-10-01-112033.ips`, incident
  `5ECE3E45-26D1-4561-9685-3BBBBA945AAC`.
- `openeq-synth-cache-stress-bounded-before-2026-10-01-112118.ips`, incident
  `42237C37-314F-499C-99E9-ED6B88A0C024`.

The same four-worker program passes with the common native gate. OpenEQ gates
rendering and SysEx as well as MIDI and lifecycle operations because those
calls can consume or change native bank/voice state. Narrowing the gate to
particular MIDI statuses or only the observed loading routine would leave
unverified cross-unit overlaps. Calls are synchronous and remain on workers;
the lock is never held across queue waits, sleeps, or output consumption.
Multiple native synth workers may consequently wait for one another. This
correctness boundary is not a guarantee about unrelated system components or
native code outside OpenEQ's calls.

## Rust cleanup and regression coverage

In `crates/openeq/src/audio/midi_synth.rs`, the owning `Synth` local is
declared before the lock guard. Rust drops locals in reverse declaration
order, so a setup error or unwind releases the guard before `Synth::drop`
calls `close` and reacquires the lock. Successful construction moves the
owner into the result and drops the guard. `close` holds the same lock
across both uninitialization and disposal, takes ownership only once, and
attempts disposal even if uninitialization reports an error.

The opt-in `concurrent_offline_synth_lifecycle` regression runs four workers
through 32 lifecycles each, alternating 44.1/48 kHz, rendering a note into
memory, and alternating explicit close and destruction. Its only barrier
is before the loops, so a failed worker cannot strand peers at a later
barrier. The initial pre-fix reproducer (with additional per-iteration barriers)
reproduced SIGSEGV, recorded in
`/tmp/openeq-coreaudio-concurrent-before.log`; that is a separate reproduction
from the fully explained SIGTRAP above. No audio device is opened by this test.
The initial lifecycle-only correction passed the parallel 61-test audio run
and 20 repeated stress batches (2,560 total synth lifecycles), but did not
exercise the instrument churn needed to reveal the later waveform failure.

The new opt-in `concurrent_offline_instrument_loading` regression preserves
the four-worker reproducer's 4,096 program changes and checks that PCM is
finite and not entirely silent. Together with the existing lifecycle,
synthetic-note, original-SysEx, validation and ABI tests, all seven focused
synth tests passed concurrently with the full native gate. This keeps real
cross-unit work in the regression rather than serializing tests externally.
The focused run is `/tmp/openeq-synth-cache-fixed-tests.log`.

For reproducibility, the matching native disassembly was generated with:

```sh
xcrun dwarfdump --uuid /System/Library/Components/CoreAudio.component/Contents/MacOS/CoreAudio
xcrun llvm-objdump --macho --arch=arm64e --disassemble /System/Library/Components/CoreAudio.component/Contents/MacOS/CoreAudio
xcrun llvm-objdump --macho --arch=arm64e --disassemble --section=__realtime /System/Library/Components/CoreAudio.component/Contents/MacOS/CoreAudio
```

The local full-suite log is `/tmp/openeq-lighting-five-frame-workspace.log`;
annotated disassembly is `/tmp/openeq-coreaudio-arm64e-disasm.txt`. The crash
and temporary logs are local investigation artifacts, not repository files.
Offsets and internal layouts are specific to the UUID above.

The lazy-loading investigation additionally uses
`/tmp/openeq-coreaudio-arm64e-audio-race-disasm.txt` and
`/tmp/openeq-coreaudio-arm64e-realtime-disasm.txt`: the default disassembly
omits the `__realtime` section containing several relevant stack frames.
Standalone before/after sources are
`/tmp/openeq-synth-cache-stress-bounded.rs` and
`/tmp/openeq-synth-cache-stress-fixed.rs`; the baseline implementation is
`/tmp/openeq-synth-lifecycle-only.rs`. Compile with `rustc --edition=2024 -O`.
The fixed program renders only to memory, and its successful output is in
`/tmp/openeq-synth-cache-stress-fixed.log`.

Root independently reran the final startup-only stress pattern against a temporary
copy of the same synth implementation with only the lifecycle guards removed.
The first batch reproduced SIGTRAP (return code -5); no shared checkout change
was needed. Witness `/tmp/openeq-synth-unlocked.rs` and result
`/tmp/openeq-synth-unlocked-result.log` remain local.

Root independently compiled both frozen before/after standalone harnesses. The
lifecycle-only version terminated with SIGTRAP (return code -5) in 0.10 seconds;
the production-gated version completed the same 4 × 8 × 128 program sweep in
0.15 seconds. Result: `/tmp/openeq-synth-cache-stress-root.json`. This independently
confirms the recorded reproduction and regression, not a throughput benchmark.
