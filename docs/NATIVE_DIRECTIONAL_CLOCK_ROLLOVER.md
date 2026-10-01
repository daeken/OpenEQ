# Native directional clock rollover

October 1, 2026. This research extends the connected
[calendar/directional lifecycle](NATIVE_DIRECTIONAL_CLOCK_LIFECYCLE.md)
with explicit unsigned 32-bit millisecond rollover. It changes no production
clock, lighting, shadow or calendar behavior.

## Executed boundary

The frozen lifecycle harness supplies the same original calendar, clock getter,
directional initializer/updater/transfer, native angle/trigonometric methods,
and bounded host tick/outdoor dispatch blocks. The new witness changes initial
millisecond ticks and advances from each explicit origin. Wall seconds advance
normally, or the calendar update is deliberately omitted as a separate control.
Original executable/DLL hashes and the prior harness hash are asserted.

There are **24 scenarios and 408 complete directional updates**: initial hours
6/12/18/24, tick origins 2,000,000 / `0xffff0000` / `0xffffff00`, with both frozen
and advancing calendar. Seventeen chronological offsets bracket wrap and the
ordinary transfer boundaries, ending 1,080,000 ms after initialization.
All 408 observed transfer decisions match an independent comparison of the
current unsigned tick with the previous stored deadline. Every calendar sequence
matches its ordinary-origin control, including the advancing cases. There are
126 transfers across the matrix.

## Observed deadline behavior

The original direct unsigned deadline comparison is not wrap-safe:

- At 06:00 with origin `0xffff0000`, the ordinary first transfer at elapsed
  36,000 sets a wrapped deadline `0x1940`. The next update at 36,001 still has
  a high pre-wrap tick and transfers again, consuming another countdown step.
  After wrap it waits for the currently retained low deadline.
- At 12:00 with origin `0xffffff00`, the initial future deadline has already
  wrapped. Elapsed 0 transfers immediately; elapsed 1 reinitializes. At elapsed
  255, tick `0xffffffff`, the zero transfer interval leaves deadline
  `0xffffffff` and countdown zero. All tested later updates, from elapsed 256
  through 1,080,000, skip transfer and retain the cached light direction even
  while the sky angle changes. The frozen-calendar sun reaches approximately
  127.999878 at the last sample. The same deadline pattern occurs in the
  advancing-calendar control; advancing the calendar alone does not repair it
  within this executed chain.

This distinguishes smooth angle progression and calendar arithmetic from cached
light publication. The witness does **not** establish that a live client keeps
that stale direction indefinitely: other frame, zone, time receipt or explicit
transfer callers may reset it. Nor does it establish how the actual millisecond
clock is anchored in a process surviving this duration. It executes controlled
near-wrap inputs, not a 49-day live session.

## Implementation boundary

A future monotonic OpenEQ lighting clock should explicitly document its rollover
policy. Copying this direct comparison would preserve a demonstrated failure;
changing it should be identified as a modern policy rather than claimed as
exact native scheduling. The existing fixed direction remains unchanged.
Full-frame ordering, remaining direct transfer callers and renderer uniform
consumption still require their own evidence.

## Frozen local reproduction

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-direction-rollover.py /tmp/your-rollover-result.json
```

The output argument preserves the frozen evidence. No original binaries or
asset-derived outputs are added to the repository.

| Local artifact | SHA256 |
| --- | --- |
| `/tmp/openeq-direction-rollover.py` | `d1f2cc127b2d3efb83686c8ce3256fbe49db5aad055f4e62e3c22788daec9b09` |
| `/tmp/openeq-direction-rollover.json` | `3dd149a2b8b133081117cabfc47e5343a2d8cb64128218a87bdea056ca9d8d3a` |
| `/tmp/openeq-direction-rollover.log` | `cda9769d016708b269b51735b7b7643a69301cadbe44db4d2a532a9928a543f1` |

Independent review replayed the script into a separate output file and reproduced
both JSON and log hashes exactly. It checked origin-relative stepping, reset
anchors, prior-deadline prediction and the limited post-wrap invocation history.
