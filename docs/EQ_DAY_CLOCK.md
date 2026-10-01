# EQ packet clock, elapsed time and native sky hour

2026-10-01. Protocol and original-instruction evidence, with the bounded live
implementation recorded below. The wire clock is one-based (`1..=24`), but different
native consumers apply different offsets: displayed hour subtracts one;
the sky-time path uses the raw hour. Preserve that distinction when advancing
OpenEQ's clock between packets.

## EQEmu packet path

Reviewed EQEmu checkout `4aceae18b94ffaafc08e2b17bc41cd72c77f795d`:

- `common/eqtime.cpp:60` computes elapsed real seconds from the saved anchor,
  divides by three, adds saved minute and zone offset, and carries into hour
  using `(saved_hour - 1 + carry) % 24 + 1`. Minutes are `0..59`, hours `1..24`,
  days `1..28`, months `1..12`.
- `zone/client_packet.cpp:1769` sends `OP_TimeOfDay` during zone entry by calling
  `zone_time.GetCurrentEQTimeOfDay(time(0), packet_payload)` directly.
- `world/zoneserver.cpp:1127` handles `ServerOP_GetWorldTime` by sending the
  world's saved EQ date/time and real anchor to the zone. Its
  `ServerOP_SetWorldTime` handling replaces that anchor and broadcasts a sync.
- `zone/worldserver.cpp:976` handles that sync by setting the zone anchor,
  computing current time and broadcasting a new `OP_TimeOfDay` to clients.
  Localized zone clocks skip world synchronization.
- `zone/zone.cpp:1991` implements `SetTime`: a caller-supplied displayed hour
  becomes `hour + 1` in the stored anchor. Its local-only branch immediately
  broadcasts the new time. `zone/gm_commands/set/set_time.cpp` displays the
  current clock with `world_time.hour - 1`; worldserver's sync log does likewise.

The common packet is eight bytes: hour, minute, day, month, then a little-endian
32-bit year. The RoF2 descriptor declares a 16-bit year plus two placeholder
bytes, still eight bytes total; its first four bytes have the same layout.
`common/patches/rof2_ops.h` has no TimeOfDay override. `StructStrategy` defaults
to `PassEncoder`, so no hour conversion is applied there. RoF2's configured
opcode is `0x5070`, matching OpenEQ's `ZoneOp::TimeOfDay`; OpenEQ currently
preserves the first two payload bytes as `ZoneEvent::Time`.

The comment in `common/eqtime.cpp` saying "1 = 1am" is inconsistent with
the explicit display/setter paths above. The one-based arithmetic alone does
not decide the displayed hour; the setter and display consumers do. More
importantly, this does not authorize subtracting one for every consumer.

The server rate is **one EQ minute per three real seconds**, one EQ hour per
three real minutes, and one EQ day per 72 real minutes. No subminute phase,
rate field or real timestamp is transmitted in the client TimeOfDay payload.
The shown server paths send initial time and authoritative corrections; they
do not provide a packet every in-game minute. A client must advance locally.

For a bounded source execution, `/tmp/openeq-eqtime-source-check.cpp` contains
the exact current `GetCurrentEQTimeOfDay` function body with minimal struct
declarations and a standalone test main. Its 505 assertions cover every hour,
three minute values, seven elapsed intervals, and year rollover. This is an
extracted-function execution, not a full EQEmu build or live packet capture.

## Original-client receipt and elapsed update

Installed `eqgame.exe` SHA-256:
`bab4ee0bd724b80c85de7df7020e7049a2bedaa1eca65b1abc59294c472cd593`.
This installed client's timestamp dispatcher uses opcode `0x5ed5`, not
RoF2's `0x5070`. Native string `MSG_TIME_STAMP received.` identifies the
branch at `0x004fdad2`. Its payload is the same first four bytes plus a
32-bit year. This research establishes that installed build's behavior and
the independently inspected EQEmu/OpenEQ protocol; it does not claim that
their complete native network protocols are identical.

At `0x004fdad2..0x004fdbc5`, the original handler copies the payload unchanged
into the object at global `0x010171e0`:

| Object offset | Meaning established by receipt and update |
| --- | --- |
| `+4..+8` | Current hour, minute, day, month, year |
| `+0xc..+0x10` | Received date/time anchor, same field layout |
| `+0x14` | Millisecond clock at last update/receipt |
| `+0x18` | Wall-clock seconds at packet receipt |

The handler then calls environment refresh `0x004b96d0`. Receipt replaces both
clock anchors unconditionally, including an authoritative time earlier than
the previously calculated time; it does not smooth a correction.

The main-loop call at `0x0057c99b..0x0057c9a7` passes milliseconds from
`0x00897d90` into clock updater `0x00869670`. The updater returns unchanged
until unsigned `(now_ms - last_update_ms) >= 3000`. Once admitted, it obtains
wall-clock seconds from `0x008978a0`, subtracts the receipt wall anchor, and
multiplies by 20 to get elapsed EQ seconds. It splits that elapsed value into
calendar carries, recomputes the clock from the received anchor, and records
the new millisecond gate time. It does not repeatedly add one minute per call,
so missed updates catch up.

`0x008978a0` calls native CRT routine `0x00936fa5`, whose original code reads
`GetSystemTimeAsFileTime` and converts to whole Unix seconds. The millisecond
wrapper dispatches through `0x00e04c84`; its QPC path at `0x00897d20` uses
`QueryPerformanceCounter` and a counts-per-millisecond divisor. Other startup
fallbacks exist. Therefore the native implementation is **not purely
monotonic**: it uses milliseconds to gate an update but wall seconds to compute
the date/time. Wall-clock jumps and all fallback startup selection are outside
the bounded execution cases.

## Raw hour through sky and display consumers

Clock getter `0x004add30` first considers override global `0x0102481c`, falling
back to `0x01024780` if that value exceeds 24. An override in `1..24` returns
that hour and zero minutes. Without one, zone types 1, 2 and 5 return object
bytes `+4` and `+5` unchanged. Other types begin with zero hour/minute. There
are additional zone-ID-specific fixed-time cases in this getter; the witness
uses ordinary zone ID zero rather than claiming every zone follows the clock.

Host sky updater `0x004b9440` calls that getter and passes
`f32((raw_hour * 60 + minute) * native_f32(1/1440))` to sky slot `+0x48`.
There is no hour-minus-one or normalization in this host arithmetic. Native
display preparation `0x008697b0`, by contrast, decrements the current hour
before placing it into the formatting routine's `tm_hour`.

| Received hour/minute | Getter output | Display `tm_hour` | Native sky fraction |
| --- | --- | --- | --- |
| `1:00` | `1:00` | 0 | about `0.04166667` |
| `12:00` | `12:00` | 11 | `0.5` |
| `13:15` | `13:15` | 12 | about `0.55208337` |
| `24:00` | `24:00` | 23 | `1.0` |
| `24:59` | `24:59` | 23 | about `1.04097223` |

Thus the native sky's time convention is one hour ahead of the displayed
clock. This is observed behavior, not a reason to change all raw-hour users.
The audio schedule already intentionally uses raw server-hour thresholds.
Color-table sampling at fractions above one is a separate boundary; the host
does pass those values. See [sky-color sampling](SKY_LIGHT_COLOR_INPUTS.md).

## Executable witness and implementation guidance

`/tmp/openeq-day-clock.py` executes the original timestamp dispatch branch and
receipt, original wall-second and QPC-millisecond wrappers, complete elapsed
updater, hour/minute getter, and sky host through its time-setter call. It also
executes display preparation through the `tm_hour` assignment. OS clock calls
return controlled FILETIME/QPC values; native conversion arithmetic executes.
Logging and final environment refresh are intercepted, and the sky setter
records its argument. No network transport, complete formatting, native game
process or GPU executes.

All 23 cases pass, including 20 native receipts and 16 native elapsed updates.
An independent calendar oracle checks valid ordinary dates and tested elapsed
spans. Cases include:

- The raw-hour/display/sky distinctions in the table above.
- Millisecond gate 2999 versus 3000; independent wall elapsed 2, 3 or 6 seconds.
- Hour, day, 28-day month and year rollover; missed updates; one real day.
- Successive updates without cumulative minute drift; 32-bit millisecond wrap.
- A newer packet resetting the date/time backwards and resetting both anchors.
- Outdoor types 2/5, an indoor getter, explicit hour override and fallback.

For OpenEQ, a receipt-anchored monotonic clock is a reasonable deliberate
implementation choice: advance by `floor(elapsed_real_seconds / 3)`, retain
raw hours `1..24`, and replace the anchor on each valid TimeOfDay packet.
This keeps the established rate without copying sensitivity to host wall-clock
changes. Reject malformed hour/minute samples instead of manufacturing a new
anchor. A packet has no subminute phase, so exact server tick alignment cannot
be inferred; receipt anchoring can differ by up to the remaining three-second
tick plus network delay. Do not claim native wall-clock parity for this choice.

Refresh minute-dependent sky sampling when the current raw `(hour, minute)`
changes, including local elapsed updates and packet corrections; caching only
the hour misses native transitions. Keep the raw-hour fraction for sky and
audio users. A separate user-facing clock can explicitly display `hour - 1`.
This research does not establish continuous fractional-minute sky sampling,
live weather transitions, server clock drift correction, or update ordering
against every native environment subsystem.

Run `PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-day-clock.py`.
The harness reuses only the setup prefixes documented in
[native ambient research](NATIVE_HOST_AMBIENT.md). Local evidence hashes:

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-day-clock.py` | `e89807739642f089ffc405e3cf53d7363896102f3ec0095d4551be9761cecbc5` |
| `/tmp/openeq-day-clock.json` | `1a68e8fd087b179395433965e1685019512314c0c67b812b8dbaf3f2edb4553f` |
| `/tmp/openeq-day-clock-native.txt` | `6d96fe5660d186968101de905bce9f8238226123068ef9682ab279fd5763be41` |
| `/tmp/openeq-eqtime-source-check.cpp` | `3d99c8daac13b66c39f6c1897f0e467ca4d26328a94733e36ac6ac6bace861c8` |
| `/tmp/openeq-eqtime-source-check` | `93d959f55ec914a3a32e3fbdd396403db2fc53c581d59b0e5d67a033fcbe629c` |
| EQEmu `common/eqtime.cpp` | `ba5b67ace7e5e0c41b6039ff78e513c146c0b848924427d02efb142c22e23983` |

The native elapsed routine and calendar checks are bounded to valid dates and
ordinary spans. Malformed fields, backward wall time, native year overflow,
all fixed-zone overrides, and very long integer-overflow behavior are not
asserted as supported behavior by these tests.


## Live implementation and background sampling

OpenEQ now advances the public raw hour/minute from a monotonic receipt anchor,
one EQ minute per three real seconds. Every valid packet replaces that anchor,
including backwards and same-minute corrections; invalid fields preserve the
previous anchor. Before the first valid packet, the existing noon fallback
stays fixed. The network worker timestamps receipt before outgoing work or
foreground queue delays. Zone loading does not freeze elapsed progression;
the destination's authoritative packet resynchronizes it.

Sky sampling runs in a background worker when the raw minute or zone visit
changes. At most one worker is retained. Results from stale minutes, departed
visits and invalidated account sessions are discarded, even when a new session
happens to request the identical zone/minute stamp. Reset retains and drains
pending work before starting a replacement. A failure is terminal for its
stamp, preserving the existing unsupported-sky fallback without retrying each
rendered frame. Applying GPU resources remains on the event loop; texture
reads and decoding do not.

Twelve clock tests cover all 1,440 minute transitions, exact elapsed boundaries,
long gaps, corrections, delayed delivery and malformed samples. Four background
refresh tests cover stale minutes/visits, failures and repeated account resets
while work is blocked. Independent code review cleared both components after
the reset test exposed and repaired overlapping loader work. Full integrated
verification is recorded in the overnight checkpoint.

This does not implement native wall-clock discontinuities, full calendar or
zone-specific clock overrides, weather simulation, fractional-minute sky
sampling or exact server subminute phase. Existing sky fraction normalization
and raw-hour audio thresholds are unchanged.

### Bounded refresh performance audit

A local 2026-10-01 probe used the installed original PoKnowledge and Greater
Faydark sky assets on Apple M4 / arm64 / macOS 27.0. Both zones resolve
`DefaultClear`, with a 256x256 `cloud-DefaultOrographic.dds` sprite, a 32x32
main table and a 32x32 cloud-color table. The standalone probe linked the
existing dev-profile libraries (`opt-level=1` for workspace code) and made no
production edits or live-account connections. It ran alongside workspace
verification, so these are bounded warm-cache observations, not isolated
hardware benchmarks or guarantees for other machines.

Each loader distribution below contains 200 samples after 20 warmups, cycling
15 adjacent minutes at noon or within the dawn blend. Each renderer distribution
contains 100 samples after 20 warmups. All timings are elapsed wall time.

| Operation | PoKnowledge median / p95 | Greater Faydark median / p95 |
| --- | --- | --- |
| `load_sky`, noon | 1.433 / 1.549 ms | 1.445 / 1.623 ms |
| `load_sky`, dawn | 1.471 / 1.611 ms | 1.484 / 1.757 ms |
| `set_environment`, caller return | 0.090 / 0.135 ms | 0.094 / 0.124 ms |
| `set_environment` + empty submit + device wait | 1.609 / 1.662 ms | 1.612 / 1.703 ms |

The last row deliberately drains pending GPU work and includes driver submission
and synchronization; it is not GPU execution time or the normal event-loop
cost. Normal `set_environment` only queues uploads. Headless rendering used an
empty 96x96 scene for the separate phase check. This does not measure full-zone
frame-time impact, cold storage, worker startup, memory pressure or a live
72-minute day. Background loading is about 1.5 ms per three real seconds in
these samples; the observed caller-side upload setup is about 0.1 ms.

There is avoidable repeated work: `load_sky` rereads and parses both INIs
(107,100 bytes combined), resolves keys and decodes source textures on every
minute. `SkyResources::new` recreates three textures and views, a bind-group
layout, sampler and bind group. The two native tables upload cropped 31x30
regions (7,440 bytes total); the unchanged cloud sprite contributes another
262,144 bytes, for **269,584 bytes per refresh**. No new pipeline is compiled by
`set_environment`. Retaining parsed definitions/source textures and updating
only changing GPU tables could reduce this work, but the measured cost does
not justify a cache rewrite in this checkpoint.

A complete 1,440-minute sweep of the installed `DefaultClear` using explicit
`minute / 1440` samples found 157 adjacent minute boundaries with any texture
change: 153 changed the main table, 64 changed cloud tint, and none changed the
cloud sprite. Thus 1,283 minute refreshes per cycle reproduced identical texture
bytes. The two change counts overlap; their sum is not the number of unique
changing minutes. This is a static asset sweep, not live weather evidence.

Cloud motion does not restart during resource replacement. `set_environment`
does not reset `Renderer::start`; the shader derives drift from renderer elapsed
time and the authored velocity. At fixed elapsed time 90 seconds, the probe's
GPU readbacks before and after replacing identical sky resources were byte
identical for both zones. Advancing to 93 seconds changed 2,127 of 9,216 pixels
in each image, confirming that scrolling still progresses across replacement.
This verifies the present constant-velocity cloud path, not future weather or
velocity transitions.

Temporary reproducibility artifacts (no original texture bytes checked in):

| Artifact | SHA-256 |
| --- | --- |
| `/tmp/openeq-sky-refresh-bench.rs` | `3dc0985ca586de1842dba07b7c75b9c171d4c4eff283bdfb88a04d91ebcb1e5f` |
| `/tmp/openeq-sky-refresh-bench.log` | `b24249edccfb7da5df73d86a5622562f3d4cbf8de693188d7129b484ab348e12` |
| `/tmp/openeq-sky-refresh-day-sweep.log` | `dea548ac9c1623ea59af5c01dfc2358e6fb63232a7d32538e33ef8db6d6a4c11` |

The probe accepts `--day-sweep` for the static day pass; otherwise it measures
warm loading, resource replacement and fixed-elapsed GPU phase continuity.


## Raw hour 24 color-table boundary

The native host passes fractions from 1 through approximately 1.041 during
raw hour 24. The existing public loader wraps finite fractions to one day.
A follow-up executes the original sampler twice for every minute in hour 24
and every resolvable installed color set: once with the exact native host f32
multiply input, once with OpenEQ's existing f32 expression and normalization.
All **4,680 full-table comparisons across 78 sets** match, from 9,360 original
sampler executions. The missing PoDisease set is excluded explicitly.

This establishes that the advancing clock does not introduce a color-table
mismatch during hour 24 in the installed corpus. It does not make normalization
native behavior for arbitrary authored keys or establish dome orientation;
those are distinct inputs. No source/production behavior changes follow from
this corpus equivalence.

Reproducer: `PYTHONPATH=/tmp/openeq-re-tools python3 /tmp/openeq-sky-wire-midnight.py`.
The script SHA-256 is
`384fc9447ba52163ed5d2527953c21ed798c9bf2e42ad2943491144353f6bd4a`;
its `/tmp/openeq-sky-wire-midnight.json` result SHA-256 is
`91f273ef6baf78389a2ca17689a7864ecf9b63d60be10e01a7abd86b1aff537a`.
The result records source-witness hashes and per-set aggregated native BGRA
CRCs, without original pixel bytes. The log is
`/tmp/openeq-sky-wire-midnight.log`.
