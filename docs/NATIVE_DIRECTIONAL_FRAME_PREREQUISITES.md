# Bounded normal host-frame prerequisite research

Result: no complete normal-frame directional-light witness was obtained. The three
probes advance the original normal host frame without replacing game owner,
clock, directional-light, or sky-update methods. All stop before host tick
publication, engine clock publication, directional update/transfer, and rendering.
This is new prerequisite evidence, not a basis for a production direction change.

## Executed boundaries

1. `openeq-direction-complete-frame-attempt.py` enters `0x004c0420`, follows its
   jump into `0x004bf320`, executes the native particle-manager virtual call and
   lazy singleton construction, then faults at `0x004ac1d3`. The inherited fixture
   has no player in global `0x01017210`; the instruction reads player `+0x10dc`.
   The containing method `0x004ac1b0` later also reads calendar `+4`.
2. `openeq-direction-complete-frame-player-attempt.py` additionally executes the
   original player constructor `0x005cbbd0` on a zeroed `0x1fe4` allocation, checks
   its returned pointer, publishes that object and the host, and supplies a
   controlled noon calendar. It reaches frame `0x004bf3f2`, which invokes the
   AudioTrigger lazy singleton `0x0042d7a0`. Native construction `0x0042d650`
   calls directory setup `0x0042cb20`, string formatting, then native CRT
   locale/thread-state acquisition. This probe intentionally stops before the
   first unprovided Windows import at `0x00943694`, `GetLastError` through IAT
   `0x00ab1238`, avoiding execution of an unresolved import value as code.
3. `openeq-direction-complete-frame-platform-attempt.py` adds explicit controlled
   Windows services and runs native CRT thread initialization `0x009438c0`, whose
   return is asserted to be 1. The original thread/locale initialization,
   AudioTrigger construction and directory-pattern formatting then execute.
   The precise remaining boundary is `FindFirstFileA` at `0x0089b148`, IAT
   `0x00ab1218`, with native-produced pattern `AudioTriggers\*` (one backslash
   in the actual string). The pending output buffer and import are in JSON.

Every probe asserts incomplete execution and unchanged host/engine clocks (both
initially zero in this setup), no CPU engine-clock stores, and zero entries at
`0x004bf42f`, `0x004bf434`, `0x004bf557`, `0x004b9970`, `0x004b9440`,
`0x10097de0`, and normal-frame return `0x004bfd04`. Thus the missing vector
witness has not been silently replaced with an empty successful frame.

## Fixture and platform limits

All three import only the setup prefix of frozen
`/tmp/openeq-particle-frame-admission.py` (SHA-256
`c3a28b56d9b4f9dde6f018639e167c6428ed2cf9fafc3c23d552552a995e265c`).
That setup has native cameras and controlled renderer/device/resource/DPVS
interfaces, a placed lamp particle fixture, no active sky, zero host directional
active field, disabled sound, and empty optional subsystems. The frame probes
publish its particle-manager interface with constructor-proven native vtable
`0x1013c31c` and supply ordinary host C++ allocation at `0x009351a9`; lazy game
singletons still execute native construction. QPC is held at
`epoch + 2,000,000 * 10,000`, with no per-read advance; native timer initialization
and conversion remain intact.

The constructed player has not undergone full spawn/configuration method
`0x005d3400`; its constructor completion is a narrower fact than a valid live
player/zone owner. The complete native spawn allocator `0x005d7c30` allocates
`0x1fe4`, calls `0x005cbbd0`, then calls `0x005d3400` with additional inputs.
No claim of valid full gameplay ownership follows from this fixture.

The third probe uses a single controlled OS thread: thread ID 1, mutable last
error initially 0, dictionary-backed TLS with monotonically allocated slots,
identity pointer encoding, a zeroed allocation heap, and uncontended critical
sections. Module handles are 1; GetProcAddress returns 0, causing native CRT to
select its TLS fallback. Reference increment/decrement interfaces perform actual
memory updates. HeapFree is a no-op in this bounded fixture. The complete list
of bound APIs and final TLS/error state is saved in its JSON. These are explicit
platform controls, not evidence for every Windows locale/thread mode.

All remaining unprovided KERNEL32 direct-IAT calls are stopped before execution
in the third probe. No file-enumeration or game method was replaced to advance
past the final boundary. A read-only metadata snapshot records the actual local
AudioTriggers tree (default/shared directories), but that snapshot is not yet
connected to FindFirstFileA/FindNextFileA or native config loading.

Further work must supply a documented filesystem/configuration interface, finish
normal-frame owner setup, and connect active native calendar/sky/light state.
Then capture every angle/vector CPU write and final renderer direction binding
across the original deadline. Existing bounded lifecycle/rollover evidence and
complete alternative-frame particle evidence retain their previous scope.

## Verification

`openeq-direction-normal-frame-replay.py` reruns all three probes and compares
JSON and stdout bytes exactly with the saved outputs. No Cargo/GPU work, game
session, live character, network interaction, production source edit, or commit
was performed. The accompanying manifest hashes the scripts, outputs, static
native disassembly, metadata snapshot, and this note. Static disassembly uses
only explicitly selected instruction-aligned spans; it is not a complete caller
or owner-state proof.

Replay command:

    PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-direction-normal-frame-replay.py

Frozen manifest: `/tmp/openeq-direction-normal-frame-manifest.json`, SHA256
`69c5c279ee9f2fb4f56bf8df4a2663dace559ebe386fb3760323c9344061a161`.

Root verified all16 manifest artifact hashes and independently replayed all
three probes into separate outputs. JSON and stdout match exactly. This confirms
the recorded incomplete boundaries; it does not upgrade them to a successful
normal-frame or directional-light witness.
