# TER floating-point state: executable loading loop

2026-10-01, research only. This narrows the lifecycle gap in
[TER vertex channels](EQG_TER_VERTEX_CHANNELS.md#device-flags-and-later-fpu-setup).
The executable explicitly requests **x87 PC53, round toward zero, exceptions
masked** on the thread that calls normal zone loading. Terrain construction
and post-load upload have synchronous call paths from that loader. The
remaining gap is preservation across the intervening game, archive and
graphics calls; this note does not establish one universal production
normal/tangent policy.

The installed images are:

| Image | Base | SHA-256 |
| --- | --- | --- |
| `eqgame.exe` | `0x00400000` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| `EQGraphicsDX9.dll` | `0x10000000` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |

## Explicit loading-loop reset

Function `0x0057d9d0` sets the mode at its loop head:

```text
0x0057db4b  push 0x000f031f       ; mask
0x0057db50  push 0x0009031f       ; requested state
0x0057db55  call 0x0093a846
```

The loop back-edge at `0x0057f62a` returns to `0x0057db44`, before this
reset. Two routes later call the normal zone loader `0x004bdca0`:

| Caller in the loop | Intermediate function | Zone-load call |
| --- | --- | --- |
| `0x0057ef28` | `0x00569670` | `0x00569738`, after `Initializing world.` |
| `0x0057dc59` or `0x0057e39b` | `0x00579d50` | `0x00579eee`, after `Starting load.` |

The second route sets a character zone field to `0xbe` before loading.
The main gameplay loop has another identical reset at
`0x0057bd70..0x0057bd7a`. These are explicit executable call sites, so the
initial value selected by Direct3D device creation does not by itself describe
the loading-loop state.

The static thread ancestry is a series of ordinary calls from the executable
entry point, not a resource-worker dispatch:

| Call or jump site | Destination |
| --- | --- |
| PE entry `0x0093de99`, then jump `0x0093de9e` | CRT startup `0x0093dd2c` |
| `0x0093de41` | WinMain `0x0063a0d0` |
| `0x0063a791` | `0x00639b00` |
| `0x00639fa3` | `0x006398f0` |
| `0x00639985` | `0x00634590` |
| `0x006345d0` | Loading loop `0x0057d9d0` |

This identifies the process-entry thread for the shown load route. It is not
a capture of a running client or a claim about every possible resource load.

## Executed original reset and helper

The isolated witness executes the original call site
`0x0057db4b..0x0057db5a` and complete helper `0x0093a846`, including its
normal SSE-enabled path. It supplies 24 combinations: x87 precision 24/53/64,
each of four rounding modes, and executable SSE feature flag `0x018265e0`
off/on. Initial exceptions are masked and MXCSR is `0x1f80`.

All cases return `0x0009031f` and leave x87 at PC53/toward-zero with exceptions
masked. The helper writes CW `0x0e3f` when changing translated state. If the
initial CW is already `0x0e7f`, it skips the x87 write and retains that word;
the difference is reserved bit 6, not precision or rounding. With SSE disabled,
MXCSR remains `0x1f80`; with SSE enabled, it becomes `0x7f80`, also with
toward-zero rounding and masked exceptions.

The x87 update is `fldcw` at `0x0093a917`. The SSE path calls `0x0094ea23`,
whose original SEH setup/teardown instructions execute against a synthetic
mapped `fs:[0]` chain. The test follows the no-exception path and verifies
restoration of that chain. It does not execute operating-system exception
dispatch, create a graphics device, or run the complete loading loop.

As in the DLL helper, the return is the resulting translated mode, not the
pre-entry value. Other executable calls are `0x004bf92f`, `0x004bfe6a` and
`0x004bfe83`. The latter pair requests `0x000a031f`, calls `0x004b9970`, then
explicitly requests `0x0009031f` again. It must not be read as arbitrary
previous-state restoration.

## Synchronous TER construction and upload

Within `0x004bdca0`, the zone EQG call at `0x004bdf3c` invokes resource-manager
vtable `+0`, `0x10066700`, with scope 2. EQG loading calls member parser
`0x100660c0`; its TER branch calls `0x100643b0` at `0x10066499`. The TER
reader calls manager `+0x28`, `0x10020970`, at `0x100647ed`. The CPU copy
routine directly calls tangent builder `0x10020030` at `0x10020b35`.
Those steps are nested calls on the loading thread, not queued CPU work.

Later in the same executable zone-loader function, `0x004bf2c4` invokes
resource-manager `+0x24`, between the post-load and precipitation log messages:

```text
0x004bf2c4 -> 0x1005a450
             -> 0x10086490(type 2)
                -> 0x100863b0
                   -> call at 0x1008640b -> 0x1001f8b0
```

`0x1005a450` supplies literal type 2. At `0x1008649d`, that type directly
enters `0x100863b0`. There is no thread-dispatch boundary in these shown
edges. Earlier [upload witnesses](EQG_NONFINITE_TER_UPLOAD.md) establish the
downstream region packing route.

## Boundary still open

The reset, synchronous thread ancestry and helper semantics are established.
They do not prove that every nested call preserves this mode. Concrete
remaining boundaries include loading-progress rendering
`0x005585e0 -> 0x004c8730 -> 0x004c8230`, archive/material callbacks before
the TER copy, the intervening calls within `0x004bdca0`, and graphics-buffer
preparation within `0x100863b0` before `0x1001f8b0`. The progress renderer
itself makes indirect graphics calls. The bounded witness does not execute
them or exclude device recreation and control changes inside their callees.

The next decisive observation is a same-thread trace of CW at
`0x0057db5a`, `0x10020970`, `0x10020030`, `0x1001f8b0` and the actual normal
arithmetic at `0x1008d24e`, recording any intervening control-word writes or
device creation. An instrumented full load or a complete preservation audit
across the remaining callbacks can close that gap. Merely feeding PC53 into
an isolated tangent probe cannot.

## Reproduction artifacts

Run with the previously installed temporary research dependencies:

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-eqgame-fpu-lifecycle.py
```

All 24 cases pass. No production files or numeric policies were changed.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-eqgame-fpu-lifecycle.py` | `45d7f92ae3992e6e89b4f088c54f61d3a17882b2763516f37115600d1b7217a3` |
| `/tmp/openeq-eqgame-fpu-lifecycle.json` | `45ba2b1e52bad8c8bdc1e7c4fd8b548048d7ed27b0fc6319ddf2e1577d066283` |
| `/tmp/openeq-eqgame-fpu-lifecycle.log` | `45ba2b1e52bad8c8bdc1e7c4fd8b548048d7ed27b0fc6319ddf2e1577d066283` |
