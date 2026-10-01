# Native EQG effect clock

2026-10-01. The native `a_fTime : Time` binding is **real seconds within a
100-second cycle**, derived from the host's cached unsigned millisecond clock.
It is separate from EverQuest's accelerated day clock and has no material-local
or zone-entry origin in the traced paths.

The executed calculation is:

```text
frame_ms = low_u32(host_timer_milliseconds)
remainder_ms = frame_ms % 100000
a_fTime = float32(remainder_ms * float32(0.001))
```

The multiplication uses native x87 arithmetic followed by a float32 store for
`SetFloat`. The original float32 multiplier is `0x3a83126f`, approximately
`0.0010000000474974513`; this is not bit-identical to division by exact 1000
for every input. The modulo occurs before conversion to floating point.

This resolves the previously untraced time-provider bridge in
[heightmap water research](HEIGHTMAP_WATER_REVERSE_ENGINEERING.md) and
[heightmap water surfaces](HEIGHTMAP_WATER_SURFACES.md). It does not change
production rendering or establish complete water/waterfall pixels.

## Effect parameter and binder

Original `EQGraphicsDX9.dll` SHA-256:
`615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`.

Effect-wrapper initialization at `0x1000b787..0x1000b79e` asks the effect's
`GetParameterBySemantic` for `Time` (literal `0x1013440c`) and stores its handle
at wrapper `+0xec`. That exact discovery slice executes in the witness.
Independent parsing of the installed compiled effect parameter descriptors
finds one scalar float parameter named `a_fTime`, semantic `Time`, in each:

| Effect | SHA-256 |
| --- | --- |
| `SPL/SModelWater.fxo` | `1de5e6c57a6fa788b7cd68d552a5e1aebc0af84f078e22c1c08c962046ac47f1` |
| `SPL/RegionWaterFall.fxo` | `65aa785ab770b621669e65f7e5b99e2397e8eeaac7efc4d398019cdf916514fd` |
| `SPL/RegionWater.fxo` | `d61baba5a8d01ab292317d957ee98d2fce0d401ea0fd4a23f6cbd3c206a95471` |
| `SPL/RegionLava.fxo` | `c1dec3fe94eed577ae318a87d2954e6f920d152ee9ed55a76b6c3bc063eebace` |
| `SPL/RegionLava2.fxo` | `a6893a7176b122f17b2ca52c331fed7e4f1e552fcd3441a992dc873845e5301f` |

Global binder `0x10089590` skips its parameter-binding body when wrapper byte
`+6` is set. Within the active body, `0x100896c4` tests the Time handle; a
missing handle skips this parameter. With a handle, the native instructions:

1. Read engine global `0x1017c0a8`, then call engine vtable slot `+0x54`.
2. Divide the returned unsigned 32-bit value by `0x186a0` (100,000), retaining
   EDX, the integer remainder.
3. Convert that remainder to x87, multiply by float32 at `0x10134ec0`, and
   store a float32 argument on the stack.
4. Call effect vtable slot `+0x78` (`SetFloat`) with the Time handle.

The witness executes the complete global binder with this parameter active.
Other handles are absent, fog-name lookup returns no handle, and graphics-only
flags are clear. It does not execute a complete effect constructor, choose a
zone's technique, run a preshader, or execute a GPU shader. Declaring a semantic
in an effect establishes its available input, not which technique is selected
for every material.

## Engine field and host frame transfer

The engine uses original vtable `0x1013becc`:

| Slot | Original entry | Behavior |
| --- | --- | --- |
| `+0x54` | `0x10068fa0` → `0x100bb4f0` | Return engine `+0x10` as u32 |
| `+0x58` | `0x10068fb0` → `0x100bb500` | Copy supplied u32 into engine `+0x10` |

These are a getter and setter for a cached value; neither reads an OS clock,
subtracts a material/zone origin, or accumulates a frame delta. Both execute
through their original addresses in the witness.

Installed `eqgame.exe` SHA-256:
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
The host engine pointer is global `0x01822678`. Three executed transfer slices
establish how the field is supplied:

- `0x004bfd57..0x004bfd70` calls host timer `0x00897d90`, stores its return in
  host `+0x154`, and passes that same value through engine slot `+0x58`.
- `0x004bf545..0x004bf559` forwards already-cached host `+0x154` through that
  same setter, without subtraction or scaling.
- `0x004bef96..0x004befaf`, in the host entry/setup path, calls the same timer
  and forwards it unchanged. It does not zero the effect clock at entry.

The first path followed by the complete DLL binder is tested as a connected
chain. Advancing the controlled OS counter by five seconds without running
another host setter leaves the effect value unchanged, and the binder makes
no OS calls. Effect time therefore follows the published frame timestamp.

## Timer unit and epoch

`0x00897d90` dispatches through global `0x00e04c84`. Timer initializer
`0x00897df0` selects `0x00897d20`. Its QPC-success path executes:

```text
counts_per_millisecond = integer(QueryPerformanceFrequency() / 1000)
counter_origin = QueryPerformanceCounter()
timer = integer((QueryPerformanceCounter() - counter_origin)
                / counts_per_millisecond)
```

The original 64-bit integer division helper `0x00935270` also executes. The
initializer stores the divisor at `0x01821238..3c`, the origin at
`0x01821230..34`, and enables QPC via byte `0x01821240`. The host caches only
the returned low 32 bits. The witness supplies a frequency of 10,000,000 counts
per second, giving exactly 10,000 counts per millisecond. Frequencies not
divisible by 1000 retain the initializer's integer divisor rounding; no claim
of correcting that native rounding is made.

The initializer's direct host caller at `0x0063a47f` is immediately preceded
by the log string **“Initializing timers.”** This makes the verified QPC epoch
the counter sample at client timer initialization, not Unix time, OS uptime,
server time, scene load, or material creation. Executing initialization again
with a new counter origin explicitly resets the resulting effect phase to
zero. This is a controlled proof of initializer behavior, not evidence that
normal zone transitions invoke it.

Before that initialization, the image's default dispatch target is
`0x00897d80`: `GetTickCount() - baseline`, using 32-bit wrapping subtraction.
The baseline writer `0x00aa83b0` calls `GetTickCount` and stores it at
`0x01821244`. Both the baseline writer and provider execute in the witness,
including an OS tick rollover. QPC-unavailable initialization has an alternate
RDTSC/calibrated-divisor branch; its calibration, startup selection and
cross-core behavior are outside this execution scope.

The native Time binding resets every 100,000 cached milliseconds. It also
inherits the low-u32 host timestamp rollover after approximately 49.7 days;
that rollover is not a continuation of the same 100-second phase. OpenEQ's
renderer-start elapsed time is a reasonable distinct origin for local
animation, but exact original startup phase cannot be inferred from zone
assets or an EQEmu TimeOfDay packet.

## Executed checks and limits

The frozen witness passes **17 QPC elapsed-time cases**, **seven GetTickCount
cases**, cached-frame and epoch-reset checks, and 29 complete global-binder
calls. The QPC cases include zero, millisecond increments, second boundaries,
100-second boundaries, large timestamps across the signed-u32 boundary, and
the low-u32 rollover. Selected original `SetFloat` outputs:

| Host elapsed ms | Cached u32 ms | Bound float32 `a_fTime` | Float bits |
| --- | --- | --- | --- |
| 0 | 0 | 0 | `00000000` |
| 1 | 1 | 0.0010000000474974513 | `3a83126f` |
| 999 | 999 | 0.999000072479248 | `3f7fbe78` |
| 1,000 | 1,000 | 1 | `3f800000` |
| 12,345 | 12,345 | 12.345000267028809 | `4145851f` |
| 99,999 | 99,999 | 99.99900817871094 | `42c7ff7e` |
| 100,000 | 100,000 | 0 | `00000000` |
| 100,001 | 100,001 | 0.0010000000474974513 | `3a83126f` |
| 4,294,967,295 | 4,294,967,295 | 67.29500579833984 | `4286970b` |
| 4,294,967,296 | 0 | 0 | `00000000` |

This research verifies the seconds unit and 100-second period assumed by
OpenEQ's indexed-water animation. It does not establish identical floating
point rounding or phase: before this investigation, the shader divided renderer elapsed
milliseconds by 1000 after floating-point modulo, while the native binder
uses integer milliseconds and multiplication by a rounded float32 reciprocal.
The existing water preshader's signed `fmod(a_fTime, 100)` receives an already
bounded, nonnegative value on the witnessed path. Day-clock corrections do
not enter any of these traced instructions.

OS clock/frequency and effect interfaces are controlled callbacks. Original
clock conversion, frame transfer, engine getter/setter, semantic discovery and
binding arithmetic execute with x87 control word `0x037f` and MXCSR `0x1f80`.
No complete game process, live frame loop, sleep/resume behavior, negative QPC
delta, RDTSC fallback, effect override by untraced code, or GPU output is claimed.

## Reproduction

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-effect-clock.py
```

The harness reuses the initialization/helper prefix of
`/tmp/openeq-ter-light-binding.py` before `renderer=alloc`, whose SHA-256 is
`801c7c490d8d319d964fe43c975912d52001cc3c0bcb59f4aa11db86a813aa21`.
It does not import or modify the separate lava/waterfall binding probe.
No original binary or effect bytes are checked into the repository.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-effect-clock.py` | `8b9faf32b6a69caa5b32e619bfedd27da75a92d5de5f1970b85dda600101195c` |
| `/tmp/openeq-effect-clock.json` | `d0856f96d273722b34748ffb67f9fbaf1eaf5810925ce9a40a8ff9b224f85362` |
| `/tmp/openeq-effect-clock.log` | `dda38099e86d89fe600dd7fa8522d9e7c7430d5ea65cc51afc61241da5876f42` |

## Separate implementation follow-up

After freezing the native evidence above, the renderer now uses this clock
for indexed water only. `EnvironmentSettings::uniform_at(Duration)`
reduces elapsed time in integer milliseconds, retaining low u32 and modulo
100,000 before multiplying by float32 `0.001`. It supplies the existing reserved
`sky_params.w`; indexed-water shading reads that precomputed phase. This avoids
losing millisecond precision by first converting a long renderer uptime to
float32. Other shader clocks are outside that change.

This implementation keeps the renderer's elapsed-time origin, rather than
claiming to reproduce the original client's timer-initialization epoch. The CPU
test matches all 17 native QPC-output bit witnesses and millisecond truncation.
A GPU regression failed before the change at 100,037,777 ms uptime; after the
change it matches the 37.777-second frame exactly across long uptimes and two
unsigned-clock wraps. All three indexed-water GPU tests pass, including the
original Feerrott fixture. Root and an independent reviewer reproduced the
native witness; independent code review found no blockers.

The complete checkpoint passes 1,157 tests with zero failed/ignored across
109 suites, including original assets, GPU and digitally silent audio. Strict
workspace Clippy, client build, all-target no-default, formatting and diff
checks pass. Logs use `/tmp/openeq-effect-clock-`: `workspace.log`,
`clippy.log`, `build.log`, `no-default.log`, `fmt.log`, `cpu.log` and `gpu.log`.
The deliberately failing pre-fix regression is `gpu-before.log`.
