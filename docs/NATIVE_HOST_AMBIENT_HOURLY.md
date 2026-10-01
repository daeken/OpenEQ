# Missing-sky hourly ambient sweep

October 1, 2026. This addendum extends the single 12:30 fallback witness in
[NATIVE_HOST_AMBIENT.md](NATIVE_HOST_AMBIENT.md). The original missing-sky
ambient path now passes every valid hour 1–24 and minute 0–59 under normal vision,
infravision and ultravision: **4,320 independently checked executions**.
Production lighting is unchanged.

The separate probe reuses only the frozen parent harness's setup and `run`
function. It executes complete host ambient routine `0x004b6b20`, original
vision initialization `0x005523b0` and native ordinary/special-A engine setters.
The clock, character/profile/effect/scalar interfaces and final host notification
retain the parent's controlled boundaries. No sky getter executes in this sweep.

## Table, arithmetic and seam checks

The oracle independently reads the 24 original f32 values in each R/G/B table
at `0x00ca59b0`, `0x00ca5a10` and `0x00ca5a70`. It computes:

```text
current = table[hour - 1]
next = table[hour % 24]
base = f32((next - current) * minute * native_reciprocal_60 + current)
```

The native reciprocal at `0x00abf130` has f32 bits `3c888889`. Wider unspilled
products precede the explicit source f32 stores; the oracle does not round every
intermediate to f32. A code observer at `0x004b6dc5` captures all three stored
base components before vision floors. Their bits match the oracle in all cases.
Final packed ambient and normalized engine RGB also match independently computed
floor/cap/truncation results. With scalar 1, special A is always `ff000000` and
special B remains untouched.

A separate memory-read observer checks **every hourly-table read address and
size in all 4,320 cases**. Each channel reads its next entry followed by its
current entry twice. Hour 24 reads index 0 and index 23, never index 24. This proves
the wrap even though the installed midnight table entries have equal values,
which could conceal an indexing error in a color-only comparison.

The following packed ordinary outputs hold at 23:59, 24:00, 24:59 and 1:00:

| Vision mode | Ordinary ambient |
| --- | --- |
| Normal | `ff14144c` |
| Infravision | `ff41324c` |
| Ultravision | `ff505078` |

The floor raises 790 channel/time combinations in normal mode, 1,030 in
infravision and 2,065 in ultravision. These counts cover 1,440 times and three
RGB channels per mode. Separate scalar 0 cases at 24:59 retain those ordinary
outputs while producing special A `ff141414`, `ff413232` and `ff505078`,
respectively. The vision minimum is a separate ambient contribution rather
than a multiplier on the hourly base color.

Zone types 2 and 5 receive 48 additional checks: minute 59 at every hour for each
type. Their ordinary outputs equal the corresponding type 1 results. Together
with three scalar 0 seam checks, the probe executes the host, vision initializer
and each ambient setter **4,371 times**. Native sky/manager getter counts are zero.

## Boundaries and reproduction

The sweep accepts only valid clock inputs. It establishes no policy for hour 0,
hour 25, minute 60, negative values or a malformed server clock. It does not execute
real character logic, sky loading/interpolation, a native graphics device,
GPU/display transfer or complete fog behavior. It retains the parent's x87
control settings and emulator/CPU-parity limits. Existing vision-route and
smoothing coverage remains in the parent probe rather than being broadened here.

```sh
PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-host-ambient-hourly-sweep.py
```

| Local evidence | SHA-256 |
| --- | --- |
| `/tmp/openeq-host-ambient-hourly-sweep.py` | `831992a16adb9bcf0b765ec3a90b28c913298a5d9f122c2f71ab71cf092fdb13` |
| `/tmp/openeq-host-ambient-hourly-sweep.json` | `b5afebd2c4f90c5e18134f5200fae86b9b0b6375352c6a605b61ccfa8e10d903` |
| `/tmp/openeq-host-ambient-hourly-sweep.log` | `246c423c95929ce195d225beeed62d66629a5d27d908168f9168ece73d5c3b96` |

The parent script and JSON remain unchanged at hashes
`517716fdebadd2225eeea6e0b3a097bcc01b39b1f18857dd441112f209ce7669` and
`3662580390d5c427739dc9badeb8121d4bc9d5a19d7521ba4d0a21725db4c24d`.
Original binary hashes remain those in the parent note; no original assets are
added to the repository.
