# Classic ambient cooldowns

October 1, 2026. Native execution confirms two constructor differences in
OpenEQ's supported classic kind-0 ambience: a nonpositive base cooldown ignores
the random field and selects continuous playback; positive base and random
values add 500 ms to both delay bounds. The native timer consumer also exposes
broader scheduling differences. This is a frozen research follow-up, with **no
production scheduler change**. It is separate from the implemented
[base-level correction](CLASSIC_AMBIENT_LEVELS.md).

## Original constructor and timer

The installed `eqgame.exe` has SHA-256
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
Addresses below are preferred virtual addresses at base `0x00400000`.
The probe executes the complete `CreateOldEmitter` at `0x004db3d0`, its
`0x005fefc0` constructor and base initializer `0x005fdd80`, with allocation,
asset-bank contents and owner/listener state supplied by the harness.

EFF stores signed day/night cooldowns at bytes 32/36 and signed random delay at
byte 40. The constructor receives one period's base and the shared random
value. Its branch at `0x004db6a3..0x004db6db` writes the emitter's loop count at
`+0x20` and delay bounds at `+0x44/+0x48`:

| Signed source values | Loop count | Stored lower/upper bounds |
| --- | --- | --- |
| `base <= 0`, any random | 0 | `[0, 0]` |
| `base > 0`, `random <= 0` | 1 | `[base, base]` |
| `base > 0`, `random > 0` | 1 | `[base + 500, base + random + 500]` |

Those additions are native wrapping 32-bit arithmetic, not saturation. For
example, `base=2,147,483,147, random=1` produces signed bounds
`[2,147,483,647, -2,147,483,648]`. Do not interpret overflowed bounds as an
ordinary positive interval or silently claim parity for a capped Rust value.

The timer setter at `0x005fdfa0` stores `GetTickCount()` in `+0x160` and copies
the lower bound to selected delay `+0x164`. Only when the upper bound is
**signed-greater** than the lower bound does it consume `rand()` and add
`rand() % (upper - lower)`. For ordinary nonoverflowing positive inputs, the
upper bound is therefore **exclusive**. Equal or inverted signed bounds use
the lower value without consuming random state.

The original CRT routine at `0x00937111` updates a thread-local 32-bit state as
`state = state * 214013 + 2531011` with wrapping arithmetic and returns
`(state >> 16) & 32767`. Twelve original calls with a controlled thread-local
block reproduce the seed-1 sequence beginning `41, 18467, 6334, 26500`.
Consequently, a random span greater than 32,768 cannot reach its full authored
range through this consumer, and modulo introduces bias for many smaller
spans. The native interval formula does not imply uniform random selection.

## Native runtime consumers

The witness executes the complete ordinary emitter update `0x00600320` for
kind 0. GetTickCount and the asset play function are controlled boundaries;
native initialization, range/time admission, timer reset, playback-parameter
construction and retained-instance decisions all execute.

- When the timer anchor is zero, `0x00600326..0x00600364` initializes the clock
  and selected delay before checking whether the emitter is enabled or the
  listener is in range. A positive cooldown therefore delays first playback
  from the first update, rather than from first range entry. The initial clock
  is verified while outside range; later entry after expiry plays immediately.
- Within range, `0x00600411..0x0060042e` subtracts the anchor from the current
  32-bit tick count and tests signed `elapsed > selected_delay`. Equality does
  not admit playback. A selected delay of zero bypasses that comparison.
  The separate eligibility helper `0x005fde90` has the same timer predicate;
  that helper was inspected, not separately executed by this probe.
- At `0x0060060f`, an admitted emitter resets its timer through `0x005fdfa0`
  **before** checking its retained instance or calling play. Timing is anchored
  to admission, including an attempted play, rather than sound completion.
- Delayed classic one-shots pass loop count 1 to `0x005fee40` and do not retain
  the returned instance. The emitter can issue another play without receiving
  a completion callback. This proves the scheduling decision, not what the
  device would do when simultaneous voices compete for resources.
- The zero-bounds/loop-zero branch passes loop count 0 and retains the returned
  instance in `+0x16c`, incrementing its reference count. Subsequent admitted
  updates reset the timer but do not issue another play while it is retained.

The downstream 2D sample method `0x005fec10` forwards playback parameter `+8`
to imported `_AIL_set_sample_loop_count@8` at `0x005fecab`, then calls
`_AIL_start_sample@4`. That forwarding is disassembly evidence; this probe
intercepts `0x005fee40` and does not execute Miles, a decoder or an audio device.
The music/sequence bypass involving emitter `+0x178` is outside this kind-0
runtime witness.

Two exact controlled-clock examples distinguish the policies:

| Source and supplied random values | First update | First play | Next play |
| --- | --- | --- | --- |
| `base=1000, random=0` | 1000 ms | 2001 ms; 2000 does not play | 3002 ms; 3001 does not play |
| `base=1000, random=100`, draws `0,99,100` | 1000 ms; selected delay 1500 | 2501 ms; next delay 1599 | 4101 ms; next delay 1500 |
| `base=0, random=100` | 1000 ms; plays with loop 0 | Same update | No further play at 1001 or 900000 while retained |

## Current OpenEQ policy differences

These observations refer to `crates/openeq/src/audio/schedule.rs` after the
base-gain fix in `3b89b11`, before any cooldown integration:

| Behavior | Current scheduler | Native kind-0 evidence |
| --- | --- | --- |
| Nonpositive base, positive random | Delay `[0, random]`; not continuous | Ignore random; continuous |
| Positive base and random | Bounds start at base | Add 500 to both bounds |
| Random upper endpoint | Inclusive (`max - min + 1`) | Exclusive when upper > lower |
| Random source | Scheduler xorshift64; advances even for a fixed delay | Shared thread-local 15-bit CRT result; no draw for fixed/inverted bounds |
| Initial playback | Newly eligible state has `next=0` | Initial update starts cooldown before range/enable checks |
| Repeat timing | Completion callback plus selected delay | Admission time plus selected delay |
| Deadline equality | Eligible at equality | Requires strictly greater elapsed time for nonzero delay |
| Active periodic sound | Retains its token until completion; no concurrent repeat | Delayed emitter does not retain returned instance |
| Large/invalid source values | `millis` clamps each signed field to 0..86,400,000; addition can yield a 48-hour upper bound | Signed tests and wrapping 32-bit fields/tick arithmetic |

Classic music is currently forced continuous independently of delay bounds.
EMT shares the scheduler and its inclusive interval representation. Neither
should acquire incidental changes from correcting classic ambient construction.
Current eligibility, voice caps, failure suppression, state retention, day/night
handoff and actual audio mixing remain OpenEQ policy; this witness establishes
no universal native scheduling or acoustic parity.

## Practical next integration boundary

Make the next bounded audio change a **kind-0 constructor interpretation and
delay-endpoint correction**. Preserve the currently documented clock and voice
lifecycle policies for that change:

1. Add one explicit classic ambient conversion for each period. Nonpositive
   base selects continuous with zero delay, regardless of random. Positive
   base with nonpositive random stays a fixed delay. Positive base and random
   acquire the native 500 ms offset and exclusive upper endpoint.
2. Keep the shared scheduler's existing inclusive representation, if desired,
   by representing an ordinary native interval `[lo, hi)` as `[lo, hi - 1]`.
   For example, source `(1000, 1)` becomes the single value 1500, and
   `(1000, 100)` becomes inclusive `[1500, 1599]`. This corrects reachable
   endpoints without silently changing EMT. It does not reproduce the native
   random distribution or its consumption of shared random state.
3. Choose and document large-value normalization explicitly. Use checked or
   wider arithmetic before conversion to durations, and preserve a stated
   OpenEQ safety policy rather than accidentally reproducing negative wrapped
   delays. Specify whether a cap applies to each source field or the final
   delay. The existing per-field cap is not a 24-hour cap on the total delay.
4. Cover both parsed day/night sides, all-day coalescing, fixed delay,
   zero/negative base with positive random, random values -1/0/1, the 500 ms
   offset, upper exclusion, and cap/overflow boundaries. Preserve the existing
   music and EMT regression expectations. Tests should remain device-free or
   digitally silent.

Document first-play and repeat clocks as remaining differences after that
bounded correction. A separate native-timing implementation would require an
explicit per-emitter timer model, pre-eligibility initialization, admission-time
rescheduling, a decision about overlapping periodic voices and resource limits,
random-stream ownership, failed play attempts, clock wrap, range exit/reentry,
and day/night enable changes. The controlled witness does not fully establish
all of those lifecycle policies. Do not fold that redesign into a gain fix or
describe a constructor correction as complete native scheduling parity.

## Frozen local reproduction

Run `PYTHONPATH=/tmp/openeq-re-tools python3
/tmp/openeq-classic-cooldown-native.py`. The script writes the JSON artifact and
prints the compact result captured in the log. Original binaries and derived
outputs remain local; no audio opens or plays. Preserve these frozen artifacts
and use different filenames for further experiments.

The shared emulation helper is the prefix before `renderer=alloc` in
`/tmp/openeq-ter-light-binding.py`. Its SHA-256 is asserted as
`801c7c490d8d319d964fe43c975912d52001cc3c0bcb59f4aa11db86a813aa21`.
The helper additionally verifies its original graphics DLL/effect dependencies,
which are mapping/setup dependencies rather than audio evidence.

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-classic-cooldown-native.py` | `02ee386d62405440d2abf7c4afd0bd5ebfd07f1ff78f51b092bae721cea39f1d` |
| `/tmp/openeq-classic-cooldown-native.json` | `89df8273efed3f51b2bf7450c7bb5791f0fce0932fcf80fe1354f9af38365a03` |
| `/tmp/openeq-classic-cooldown-native.log` | `273a1f7285b17d088ef1bcd13b5458741a00fead3feca5505b359e6b1dbbc52f` |

Coverage is 60 constructor boundary cases, 50 timer selections, 12 original CRT
calls and 17 original whole-emitter updates across five runtime scenarios.
Constructor cases include signed extremes and additions crossing `i32::MAX`.
Timer selections use controlled legal CRT outputs `0,1,99,100,32767`, including
a 40,000 ms random span and signed-overflow bounds. Runtime samples establish
strict deadline equality, repeated plays without completion, retained
continuous playback and initial timing outside range. They do not cover every
32-bit tick-wrap state or every native voice/resource failure.

Root independently replayed the native cooldown witness to separate output
paths; the result JSON matches the frozen hash above.
