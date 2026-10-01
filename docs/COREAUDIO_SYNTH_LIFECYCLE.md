# macOS DLSSynth lifecycle serialization

OpenEQ serializes creation, configuration, initialization, uninitialization,
and disposal of its macOS DLSSynth units with one process-wide mutex. The
native component shares its DLS sound-bank cache between units, but the
observed cache acquire and final release are not protected by a common lock.
Rendering and MIDI delivery remain on each unit's owning worker.

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

Keeping the whole setup and disposal sequence under the lifecycle lock is a
simple conservative boundary. Future sound-bank changes or changes to
offline mode after initialization must use that same lock. These observations
do not imply that arbitrary native property changes are safe during render.

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
After serialization, the parallel 61-test audio run and 20 repeated stress
batches (2,560 total synth lifecycles) passed. This is regression evidence
for the OpenEQ lifecycle boundary, not a guarantee about all native APIs.

For reproducibility, the matching native disassembly was generated with:

```sh
xcrun dwarfdump --uuid /System/Library/Components/CoreAudio.component/Contents/MacOS/CoreAudio
xcrun llvm-objdump --macho --arch=arm64e --disassemble /System/Library/Components/CoreAudio.component/Contents/MacOS/CoreAudio
```

The local full-suite log is `/tmp/openeq-lighting-five-frame-workspace.log`;
annotated disassembly is `/tmp/openeq-coreaudio-arm64e-disasm.txt`. The crash
and temporary logs are local investigation artifacts, not repository files.
Offsets and internal layouts are specific to the UUID above.

Root independently reran the final startup-only stress pattern against a temporary
copy of the same synth implementation with only the lifecycle guards removed.
The first batch reproduced SIGTRAP (return code -5); no shared checkout change
was needed. Witness `/tmp/openeq-synth-unlocked.rs` and result
`/tmp/openeq-synth-unlocked-result.log` remain local.
