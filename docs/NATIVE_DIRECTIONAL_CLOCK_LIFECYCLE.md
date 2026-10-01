# Native directional-light clock lifecycle

October 1, 2026. Research only. This extends
`NATIVE_HOST_DIRECTIONAL_LIGHT.md` from known angle writers to the complete
clock-driven update routine and bounded native callers. No renderer direction,
shadow, clock, or sky policy changes are made.

The native sun/moon angle fields advance separately from the cached directional
light vector. A timed branch in `eqgame.exe:0x004b9970` decides when to transfer
the current selected angle to the light. Fixed-time overrides take a different
branch that preserves the initialized angles and transfers on every call.
These statements describe the executed routines under controlled inputs; they
do not establish the cadence of a complete live rendered frame.

## Executed chain and controlled boundaries

The new probe `/tmp/openeq-direction-lifecycle.py` executes:

- Complete calendar advancement `0x00869670`, clock selection `0x004add30`,
  angle initialization `0x004b96d0`, angle update `0x004b9970`, and light
  transfer `0x004b9440`.
- Original sky angle getters/setters, time setter, and sun/moon selector,
  installed at their observed interface slots. Original math-table construction,
  512-unit trigonometry, directional-light constructor/setters, and engine
  registration execute as in the preceding host witness.
- Original frame tick write `0x004bf42f..0x004bf43a`, followed by the separate
  original outdoor dispatch block `0x004bf8dd..0x004bf925`. These are explicitly
  bounded portions of frame routine `0x004bf320`, not its complete execution.
- Complete local time setter `0x004bff20` and packet receipt block
  `0x004fdad2..0x00500ef7`, including their calls to the original initializer.
  Packet dispatch and receipt from a network session are not executed.

The host, calendar, sky interface, and process globals are controlled objects.
Wall seconds (`0x008978a0`) and milliseconds (`0x00897d90`) are supplied by the
probe. Calendar fields subsequently change through the actual calendar routine
except in the deliberately frozen-calendar threshold control. The sky's packed
RGB getters, allocation/free/memcpy, logging, and trailing ambient routine are
controlled interfaces. No process, character, session, native device, or GPU
frame is started. The probe uses x87 control word `0x037f` and does not prove the
control word or every arithmetic bit of a live original CPU.

Direction is kept separate from sky appearance. Sky time writes the dome axis
at sky +0x10; directional light uses sun/moon angle fields at +8/+0xc. The host
passes `512 - selected_angle` to the original math interface and writes
`[0, sine, -cosine]` through the native directional setter. Existing renderer
binding evidence copies that stored vector without normalization or negation.
The new probe stops at the light state; it does not repeat the GPU binding test.

## Calendar and zone selection

Calendar object global `0x010171e0` contains current hour/minute at +4/+5,
anchor hour/minute at +0xc/+0xd, last update milliseconds at +0x14, and anchor
wall seconds at +0x18. The full native calendar updater returns until unsigned
`now_ms - last_update_ms >= 3000`. It then calculates calendar time from the
wall-clock anchor at twenty game seconds per real second and stores the new
update tick. Valid calendar hours are 1 through 24; 24 represents midnight.
The connected day run uses hour 24, not synthetic hour 0.

Clock selection first considers `0x0102481c`. Values 1 through 24 force that
hour and minute zero. A value above 24, using the native unsigned comparison,
selects fallback `0x01024780`, which likewise forces time only in 1 through 24.
Otherwise zone types 1, 2, and 5 read the calendar; other types initially produce
zero hour/minute. The following zone IDs then override time independently of
that type check:

| Zone ID | Forced hour:minute |
| --- | --- |
| 63, 76 | 06:00 |
| 71 | 18:00 |
| 123 | 10:00 |
| 190 | 12:00 |

The clock helper returns one for forced time and zero for ordinary calendar
time. All 256 zone-type bytes execute through both the actual getter and the
bounded outdoor frame dispatch. Only types 1, 2, and 5 call the updater from
that frame block. IDs 0 through 255 execute through the original getter and
confirm the five switch entries above. Other caller paths are not thereby
proven to use this same outer type gate.

## Normal update and transfer gate

The complete updater performs these steps in order:

1. Read the original clock helper. If its result is nonzero, call ambient with
   the existing table index, write the forced-hour sky time, and tail-call the
   light transfer. This branch does not write sun/moon angles.
2. For ordinary calendar time, read milliseconds. Globals `0x00f63fc0`,
   `0x00f63fbc`, and `0x00f63fb8` retain the last hour, minute, and observation
   tick. A changed hour/minute resets this observation tick. An unchanged
   minute adds unsigned elapsed milliseconds times stored f32
   `0.00033333332976326346` before calculating the sky day fraction.
3. If a sky exists and duration `0x00f63ebc` is nonzero, update both angle fields
   from host tick +0x154 and the saved base/delta/origin/duration. The original
   wrap helper `0x0092c450` executes, including the moon's 256-unit offset.
4. Compare host tick directly, unsigned, with deadline `0x00f63ec8`. Before
   the deadline, call ambient with scalar 1 and return without transferring
   the directional vector.
5. At or after the deadline, set deadline to `now + interval` (`0x00f63ec4`),
   increment the ambient index and wrap it after 59, and decrement countdown
   `0x00c18ce0`. If its old value was nonzero, tail-call light transfer. If its
   old value was zero, tail-call initialization, which also transfers.

At transfer, `0x004b9440` writes sky time again using the unsmoothed raw
hour/minute. Thus a transfer call can replace the fractional-minute time stored
earlier in the updater. The selection at that transfer follows the raw-time
write and the existing original sun/moon switch. Neither sky dome rotation nor
the most recent smoothly interpolated time alone defines the light vector.

Across all 1,440 valid independently initialized calendar minutes, the native
initializer chooses either interval zero (720 cases) or 36,000 milliseconds
(720 cases), with delta 128 and duration 1,080,000 milliseconds. An interval of
zero is not a zero-duration angle interpolation: initialization also sets a
future first deadline and a countdown. In the 12:00 control that first deadline
is 540,000 milliseconds after initialization; the next update after its first
transfer reinitializes because the old countdown is now zero.

## Controlled boundary witnesses

The irregular `noon` scenario initializes calendar 12:00 at milliseconds
2,000,000 and wall seconds 1,700,000,000. Every step supplies milliseconds
`2,000,000 + elapsed` and wall seconds `1,700,000,000 + floor(elapsed/1000)`,
calls the complete native calendar updater, then executes the bounded frame
tick and outdoor-dispatch blocks.

| Elapsed ms | Native calendar | Sun angle | Stored light direction | Transfer |
| --- | --- | --- | --- | --- |
| 45,000 | 12:15 | 5.33331298828125 | `[0, 0, -1]` | No |
| 539,999 | 14:59 | 63.9998779296875 | `[0, 0, -1]` | No |
| 540,000 | 14:59 | 64 | `[0, -0.7071067691, -0.7071067691]` | Yes |
| 540,001 | 14:59 | 63.64447021484375 | `[0, -0.7040027976, -0.7101728916]` | Reinitialize, then transfer |

At 539,999 the original calendar advances to 14:59. The next two calendar
calls are only one and two milliseconds later, so its 3,000-millisecond gate
retains 14:59. That explains the next-call reinitialization angle. The probe
does not substitute a manually calculated 15:00 at the boundary.

A separate `threshold` scenario deliberately omits calendar advancement and
holds calendar 12:00. It has the same first transfer threshold and angle, but
the 540,001-millisecond reinitialization resets sun angle to zero. These two
controls must not be conflated.

The regular complete-day control starts at 24:00 and makes 1,441 updates spaced
3,000 milliseconds apart. The actual calendar reaches 24:00 on the next day.
There are 66 light transfers and four reinitializations during those update
calls, excluding the initial setup transfer. This is one supplied call schedule;
it is not a recovered normal frame rate or proof of native shadow animation.

Other executed controls cover dawn/dusk, seven forced-time configurations,
inactive transfer, absent sky, local time assignment, and packet time receipt.
Forced time keeps the initialized sun/moon fields unchanged even when supplied
elapsed time reaches 1,080,000 milliseconds, while transferring once on each
updater call. Inactive transfer still permits angle updates in the outer
updater but preserves the light vector; absent sky skips the angle setters.
Both time-assignment paths update calendar anchors and call initialization and
transfer exactly once in the executed boundary.

## Static callers and remaining limits

Direct update callers are `0x004bf920` in frame routine `0x004bf320` and
`0x004bfe74` in `0x004bfd10`. Their respective host tick writes are
`0x004bf434` and `0x004bfd5c`. Only the bounded first caller blocks described
above execute in this probe. Complete frame processing and the second caller's
preceding engine/sky work are not executed.

Initialization also has direct callers at `0x004bf205`, `0x004bffa6`,
`0x004fd12f`, `0x004fd2a9`, `0x004fdbb3`, and `0x00578b12`. Separate direct
light-transfer call sites exist outside the updater. Their triggers, weather
ordering, zone transitions, full device/render scheduling, and all paths that
could refresh the light between these calls remain outside this witness.
The unsigned deadline comparison is recovered. A separate
[rollover witness](NATIVE_DIRECTIONAL_CLOCK_ROLLOVER.md) now executes controlled
near-wrap inputs; live process uptime and other reset callers remain outside
that scope. At this original checkpoint, clock rollover behavior was
not exercised here. A runtime implementation needs an explicit state/lifetime
design and additional caller validation; replacing the fixed renderer vector
with a guessed daily orbit is not supported by these results.

## Frozen artifacts and replay

Original binaries remain those recorded by the preceding host witness:

- `eqgame.exe`: `bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`
- `EQGraphicsDX9.dll`: `615e7ba9e20745ec03a908cf47c038cc764358ad820d9d4d3232734fadff5383`

New frozen files:

- `/tmp/openeq-direction-lifecycle.py`:
  `640ea2bb019a51714639f15254668108ffff3a32efb91c122ad9787af7d80ae7`
- `/tmp/openeq-direction-lifecycle.json`:
  `a32d15c12381b1b92b4ccef5dda551b301c683bc82cd01be206aff85857f8c37`
- `/tmp/openeq-direction-lifecycle.stdout`:
  `fa83920b0d78443c73bdb7a26384fe1deb631eeb9ec0ea0fef0c6d0bc8d3e1e8`

The source reuses only initialization/helper prefixes from the frozen
`openeq-host-sky-light-native-math.py` and `openeq-ter-light-binding.py`.
`/tmp/openeq-direction-lifecycle-manifest.json` records all source, binary,
supporting effect, and generated disassembly hashes. Disassembly range dumps
can include data or partial instructions outside the explicitly described
boundaries; those lines are not execution evidence.

Replay without overwriting the frozen result:

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-direction-lifecycle.py \
  /tmp/openeq-direction-lifecycle-review.json
```

The review JSON should have the same SHA-256 as the frozen JSON above. Original
binaries, generated disassembly, and large native state tables remain outside
Git.

Root independently replayed the complete probe to a separate output path;
the JSON matches `a32d15c12381b1b92b4ccef5dda551b301c683bc82cd01be206aff85857f8c37`.
No direction or clock implementation is enabled by this research checkpoint.

The later [normal-frame prerequisite attempt](NATIVE_DIRECTIONAL_FRAME_PREREQUISITES.md)
executes original player construction and native CRT initialization, but stops
at the AudioTrigger directory-enumeration boundary before clock publication.
It documents the next fixture requirements without claiming a full-frame result.
