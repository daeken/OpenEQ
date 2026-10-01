# TER floating-point state: progress and buffer callbacks

2026-10-01, research only. This extends
[the executable loading-loop evidence](EQG_TER_FPU_LIFECYCLE.md) without changing
its conclusion: **the effective live control word at tangent construction and
vertex packing remains unresolved**. The loading-screen presentation callback
is different from the frame callbacks that explicitly request PC53/toward-zero.
The immediate pre-upload path also calls Direct3D buffer `Lock` without a local
control reset afterward.

Eight bounded CPU executions establish the actual nested callback paths and
show that an injected endpoint state change survives to their exits. They do
not observe a real Direct3D implementation changing the state. The executable
reset, progress renderer, and post-load caller are separate entry segments in
the same emulator; the omitted loader between those segments is not executed.
This is a callback-preservation/sensitivity witness, not a connected full-load
witness or a basis for choosing a production numeric mode.

## Images and original interface slots

| Image | Base | SHA-256 |
| --- | --- | --- |
| `eqgame.exe` | `0x00400000` | `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593` |
| `EQGraphicsDX9.dll` | `0x10000000` | `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383` |

The resource interface table is `0x1013b2ec`; its first slot is the known zone
loader `0x10066700`, and `+0x24` is the known upload wrapper `0x1005a450`.
Renderer construction writes table `0x1013de44` at `0x100872b4`. The relevant
original slots are:

| Interface and slot | Destination | Operation relevant here |
| --- | --- | --- |
| Resource `+0x64` | `0x10061030` | Copies image dimensions; null image returns immediately |
| Renderer `+0x88` | `0x10089160` | Queues line geometry |
| Renderer `+0x98` | `0x10088e60` | Queues textured geometry |
| Renderer `+0xa0` | `0x10088e30` | Queues rectangle geometry |
| Renderer `+0xa8` | `0x10092990` | Viewport save/change, clear, viewport restoration |
| Renderer `+0xac` | `0x10097de0` | Frame routine with explicit control-helper calls |
| Renderer `+0xb0` | `0x10092a20` | Another frame routine with explicit control-helper calls |
| Renderer `+0xb4` | `0x10092b70` | Cooperative-level check and presentation; may reset lost device |

The slots used below are read from the original tables. No virtual method
address is replaced with the address of an unrelated original routine.

## Loading-screen path is presentation, not the resetting frame routine

Progress drawing `0x004c8230` loops twice. It queries resource `+0x64`, calls
background drawing `0x004c8060`, conditionally queues the bar and text, then
calls renderer `+0xb4` at `0x004c8707` when game global `0x01017240` is nonzero.
That last call reaches **`0x10092b70`**, not `0x10097de0` or `0x10092a20`.

`0x10092b70` calls device `TestCooperativeLevel` (`+0x0c`). On success it may
flush queued UI drawing, then either obtains a swap chain and calls its
`Present`, or directly calls device `Present` (`+0x44`). The direct path is
`0x10092cc4..0x10092cdc`. There is no explicit control-helper call around the
presentation routine.

On `D3DERR_DEVICENOTRESET` (`0x88760869`), it calls renderer `+0x64`,
`0x10099ba0`, at `0x10092ba1`. That routine reaches device `Reset` (`+0x40`) at
`0x10099d3b`, with possible retries at `0x10099e06`. These reset/retry and
resource-recreation paths are static findings only; they were not executed by
the bounded witness. They must not be silently treated as preserving state.

By comparison, `0x10092a20` requests `value=0x9031f, mask=0xb031f` at
`0x10092a4f`, saves the resulting translated state, and requests that saved
state again on failure at `0x10092a7b` and on normal completion at
`0x10092b50`. `0x10097de0` uses the same pattern at `0x10097e0f`,
`0x10097e6a`, and `0x10098314`. The saved result is the requested resulting
mode, not arbitrary pre-entry state. Observing either of these resetting
routines does not establish that loading-screen presentation resets state.

## Immediate pre-upload calls are buffer locks

The witness executes the original post-load callsite beginning at
`0x004bf2b6`, including its resource `+0x24` call at `0x004bf2c4`:

```text
004bf2c4 -> resource +24 -> 1005a450
         -> 10086490(type=2) -> 100863b0
             -> 10085fd0                  pools 0..7
             -> 100860f0(type=2)          pools 24..31
             -> 10002fc0(renderer+17d4)  pool 32
             -> 100bb420(engine+4)
             -> terrain +24 -> 1001f890  identity
             -> 1001f8b0                  terrain upload entry
```

Each preparation call reaches `0x10002fc0`, which calls `0x10002ea0` and
then tail-calls `0x10002f10`. Those routines lock the pool's vertex buffers
and its two index-buffer groups through COM slot `+0x2c`. Their success paths
set pool flags `+5/+6`. The original local code between these buffer locks
and `0x1001f8b0` does not establish a fresh floating-point mode.

The fixture supplies one buffer in each of the three groups of each pool:
17 original preparation calls and **51 controlled `Lock` endpoints** execute.
After the final lock, the original terrain identity method and actual call
into `0x1001f8b0` execute with the endpoint's resulting state. Execution stops
at that entry: no region traversal or packed-vertex arithmetic is claimed.

## Controlled endpoint results

Each case begins at x87 CW `0x037f`, MXCSR `0x1f80`, and executes the original
game reset `0x0057db4b..0x0057db5a`. The executable and DLL SSE feature flags
are both tested off and on. The reset yields CW `0x0e3f` in all cases and
MXCSR `0x1f80` or `0x7f80`, respectively.

The progress fixture has a null background image, zero progress, no text,
and an empty queued UI list. It selects the successful direct-device
presentation path. Consequently its background/bar/font/queued-UI work is
skipped by original branches. The complete `0x004c8230` function and its two
nested `0x10092b70` calls execute. Only COM endpoints are controlled in those
calls: `TestCooperativeLevel` returns success and `Present` returns success.

For sensitivity cases the endpoint hook deliberately writes CW `0x007f`
(PC24/nearest) and MXCSR `0x1f80`. This is a test input, **not a measured driver
behavior**. No state reset is inserted between the three entry segments.

| Endpoint scenario, each with SSE off/on | After progress | At terrain-upload entry |
| --- | --- | --- |
| All endpoints preserve state | `CW=0e3f`; reset's MXCSR | `CW=0e3f`; reset's MXCSR |
| `Present` changes state | `CW=007f, MXCSR=1f80` | `CW=007f, MXCSR=1f80` |
| Final `Lock` changes state | `CW=0e3f`; reset's MXCSR | `CW=007f, MXCSR=1f80` |

Two additional cases call original frame routine `0x10092a20` after the reset.
Its first `TestCooperativeLevel` endpoint changes state to PC24/nearest and
returns failure. The original failure path restores `CW=0e3f`; SSE-enabled
execution also restores `MXCSR=7f80`. SSE-disabled execution leaves MXCSR
`1f80`. Original SEH setup/teardown executes against a mapped `fs:[0]`, whose
chain is verified afterward; OS exception dispatch is not executed.

An instruction trace watches control-changing instructions on every executed
path. The progress/pre-upload cases execute the original reset's `fldcw` at
`0x0093a917` and, with SSE enabled, `ldmxcsr` at `0x0094ea8b`. No further
control-changing instruction executes in those original callback paths.
The frame-failure cases additionally execute `fldcw` at `0x1010f858` and,
with SSE enabled, `ldmxcsr` at `0x1011d01b`. Endpoint injections are recorded
separately from these original instructions in the JSON.

## Remaining boundary

This narrows the open issue but does not close it. The selected empty progress
branches do not audit active background textures, font callbacks, UI flushes,
swap-chain presentation, lost-device reset/recreation, or archive/material
callbacks. The game loader between the reset, progress, construction, and
post-load caller is omitted. The real driver and live tangent construction
are not executed. The post-load witness stops before actual region packing.

The decisive evidence remains a live same-thread trace at the reset,
`0x10020970`, `0x10020030`, `0x1001f8b0`, and the actual packing arithmetic,
including control changes inside device calls. Neither a preserved-state
COM stub nor an injected PC24 endpoint proves a live production mode. No
production numeric policy or code is changed by this note.

## Frozen reproduction

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-ter-fpu-callback-witness.py
```

All eight cases pass. The original images and generated probe artifacts remain
outside the repository. The probe outputs explicit entry-segment boundaries,
endpoint injections, control-changing instructions, and original callback hit
counts. Prior frozen FPU probes and notes were not modified.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-ter-fpu-callback-witness.py` | `9b9c8b9d849b3a9356c63a38f05cac0dcadbbe70084eeb9d87d0e1adf5aeaac4` |
| `/tmp/openeq-ter-fpu-callback-witness.json` | `d8785c50e65aea7256860eaa0d1634fdf41af5d15b5dcf12ead7e12ce5756888` |
| `/tmp/openeq-ter-fpu-callback-witness.log` | `3c62e6ca6228cb92338578dcbcabb15910e5cb078305a5b9d5fab910db96eb11` |

Root independently replayed all eight cases using a separate output path;
the result JSON exactly matches the frozen hash above.
